# TASK-018 implementation review

## Verdict

**NEEDS_WORK**: both acceptance gates exist, pass, and were shown RED. But the new "keep the launch anchor" logic (`remaining` refresh plus the `remaining <= 0.0` guard) adds a reproducible regression: a step you can climb stays blocked for as long as the player keeps pushing toward it.

## Disconfirmation (done before the rest of the review)

Counter-example I wrote down first: *the barrier now refreshes `assist.remaining` every tick it fires, and a new jump no longer re-anchors `launch_feet_y` while `remaining > 0`. So a player who reached an intermediate surface (a crate) inside the window, next to a wall whose top is more than `ledge_assist_max_height` above the ORIGINAL launch but an easy step from the crate, gets pushed back every tick and can never climb it.*

I checked it with an executable probe, not by reading code alone. The probe is in `scratch/probe_stale_anchor/` (HEAD code) and `scratch/probe_old/`, which links against `scratch/gta_sim_old/` (the pre-task `gta_sim` from `git archive 2ade7af`). Both reuse the main `target/` via `CARGO_TARGET_DIR`, so no second target dir exists and no project files were touched. Layout: a 0.8 m crate at z -1..-3, and a 1.5 m wall behind it at z < -3. That leaves a 0.7 m step from the crate top, below `jump_height` 1.0. The character runs -z and jumps from the ground. After tick 60 it re-taps jump every 40 ticks. The run lasts 640 fixed ticks (10 s). Output is in `probe_out.txt` in each probe dir.

| start | HEAD (c03d819) | pre-task (2ade7af) |
|---|---|---|
| spawned on crate (control) | climbed at tick 28 | climbed at tick 28 |
| ground, start_z 2.0 | climbed at tick 104 | climbed at tick 104 |
| ground, start_z 1.0 / 0.5 / 0.0 / -0.3 / -0.6 | **never climbs in 640 ticks**, ends at y=2.57 z=-2.65 (on the crate, against the wall) | climbs at ticks 100..103 |
| crate 0.9 / wall 1.6, start_z 1.0 / 0.5 / 0.0 / -0.3 | **never climbs** | climbs at ticks 99..104 |

**Counter-example held: this is a regression caused by this diff.**

## Confirmed correct

- `crates/gta_sim/src/character/ledge.rs:89-103`: the over-limit barrier now runs before the approach-angle filter, so it applies at every angle. The pull-up alone stays angle-gated (`:104`). This matches spec item 1.
- `ledge.rs:94-99`: the barrier push uses the wall normal and the perpendicular distance (`distance * incidence`), so the capsule ends up exactly `radius + clearance` from the wall plane at any angle. For a frontal approach it matches the old `-direction * keep_out`.
- `ledge.rs:108-120`: the free-space check builds the same capsule as the body (`mod.rs:86-89`: `Collider::capsule(r, h - 2r)`, rotation locked except Y, so `Quat::IDENTITY` is right). It runs at the real snap target and excludes self. Target center = `top_y + 1.05` and half-height is 0.75, so the capsule bottom sits 0.3 m above the ledge top and the ledge itself cannot trigger a false block. The API matches pinned avian3d 0.7.0 `SpatialQuery::shape_intersections(&Collider, Vector, RotationValue, &SpatialQueryFilter) -> Vec<Entity>` (`system_param.rs:1173`).
- B7 one-tick snap (`apply_ledge`) is unchanged.
- No new `const`; no tuning value hard-coded in sim code.
- I verified in the tree myself: `cargo test -p gta_sim` gives 16/16 green (14 pre-existing + 2 new). `cargo clippy -p gta_sim --all-targets -- -D warnings` is green. `scratch/t1/summary.json` shows t1 passed (movement, fps 30, shutdown).
- Flip-RED is real and hits the mechanism (`scratch/flip_red_ledge.txt`). Restoring the angle gate fails the 62-degree row. Deleting the intersection query fails the ceiling test. The working tree is clean after the probe (`git status` clean at HEAD a4f2891).

## Issues

### 1. major: stale launch anchor plus self-refreshing window blocks climbable steps indefinitely
`crates/gta_sim/src/character/ledge.rs:39` (`assist.remaining <= 0.0 &&` guard) together with `ledge.rs:93` (`assist.remaining = cfg.ledge_assist_window;` inside the barrier).

While the barrier fires, it keeps the window alive. While the window is alive, a new jump (`ActionStarted`) does not re-anchor `launch_feet_y`. So `rise` is always measured from the first launch (the ground), even after the player stands on a crate and jumps again. The barrier then pushes the player off a 0.7 m step and zeroes horizontal velocity on every tick they press toward it, and each push extends the window. There is no timeout. The table above reproduces it. Before this diff the same scenario climbs in about 100 ticks, because the window expired and the next jump re-anchored.

This is a silent gameplay-state bug (the player is "stuck" only in certain geometry). The T2 city will have crates, steps and low walls, so it will come up.

Suggested direction (diagnosis first; recompute the prescription):
- Re-anchor on every genuine new jump, i.e. when `ActionStarted` fires from the ground. Keep the guard only against whatever re-trigger it was added for (probably the held-jump re-start mid-air, see issue 3).
- Do not extend `remaining` from the barrier. If the barrier needs to outlive the window for the oblique case, key it on "still airborne since this launch" instead.
- Add the regression probe (crate plus 0.7 m step, re-jump from the crate) as a headless test. It must be RED on c03d819 and GREEN after the fix.

### 2. minor (owner-feel): oblique barrier zeroes tangential velocity too
`ledge.rs:98-101` → `apply_ledge` `:148-151`. Before, the barrier fired only for near-frontal approaches (dot <= -0.5). Now it fires at grazing angles too, and `apply_ledge` sets `velocity.x = velocity.z = 0`. A player who jumps along a too-high wall and brushes it near its top loses all horizontal speed, not just the into-wall part. Only the normal component needs removing (`v -= n * min(v·n, 0)`). This is visible on the first run, so the owner should check it in his run; I do not ask for a new gate.

### 3. minor: the table test shapes its input around the re-anchor problem
`crates/gta_sim/tests/ledge.rs:47,66-68`. Oblique rows start 0.55 m from the wall (`start_z = -1.45`), so the jump fires on tick 0 from a standstill. They also get `jump_held = false` at tick 30, which the frontal row does not. The released jump suggests a held jump re-triggered `ActionStarted` and moved the anchor. That is exactly the case the issue-1 guard patches in a way that is too broad. After fixing issue 1, run the table with the same approach (running start, jump held) for all angles, or write down in the test why the rows differ.

### 4. minor: flip-RED for the angle table only shows the 62-degree row
`scratch/flip_red_ledge.txt`: the loop panics on 62 degrees, so 75 degrees was never observed RED under the old gate. The old gate would not have applied the barrier at 75 degrees either, so it is expected to go RED, but that was not shown. Either assert per row after collecting all results, or run the sabotage per angle.

### 5. minor: free-space query also hits sensors
`ledge.rs:115-117`. Avian 0.7 spatial queries do not skip `Sensor` colliders (the filter tests only entity and layers, `system_param.rs:1254`). A future mission trigger or pickup volume at a ledge top will silently cancel the pull-up. Nothing in the current game produces this. Note it for T-mission work: when sensors appear, use a collision-layer mask in the filter.

## Missing coverage

- Re-jump from an intermediate surface inside the assist window (issue 1). This is the key missing gate.
- Barrier push at oblique angles: the character ends at `r + clearance` from the wall and is not pushed through or away more than that (only covered indirectly by `!on_top`).
- A per-row RED/GREEN record for the angle table (issue 4).
- A liveness check paired with the ceiling test: the same ledge without the slab still snaps. It is implied by `configured_reach_climbs_and_above_reach_blocks` at a different height, but not the same geometry.

## Nits

- `ledge.rs:111-114` duplicates the capsule construction from `character_components` (`mod.rs:86-89`). If one changes, the check drifts from the body. A small shared helper, or reading the entity's `Collider`, would keep one source.
- `shape_intersections` allocates a `Vec` only to test `is_empty()`. `shape_intersections_callback` with an early `false` return does the same with no allocation. It runs only on snap ticks, so the cost is negligible.
- `assist_ledge` is now about 90 lines with 8 early exits. Readable, but the barrier branch could be extracted.
