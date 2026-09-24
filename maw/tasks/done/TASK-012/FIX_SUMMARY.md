# FIX_SUMMARY — TASK-012 (GDD T11), fixer round 2 (after QA)

Fixer: claude/opus, medium. Tree `feature/t11-police` @ `0ce78ac` plus the working-tree edits below. Binding input:
the orchestrator note for round 2 and the last OPEN_DECISIONS.md entry. That means fixing QA Bug 1 (fire-line range
boundary, shared code) with two gates, and making the t11 tracker count arrival by real distance. Bugs 2 and 3 and
the Busted camera stay out of scope, as that entry says. Round 1 is in `FIX_SUMMARY.prev-1.md`.

## Pre-flight

- I listed `scratch/` and read it as a coverage map: QA's `scratch/qa/zz_qa_probe.rs` (12-SWAT street),
  `probe_output.txt` and the round-1 fixer flips. I did not count any of it as evidence. Every number below comes from
  my own runs.
- I read from disk: the review (`IMPL_REVIEW.md`, handled in round 1), `QA_REPORT.prev-1.md`, `FIX_SUMMARY.prev-1.md`,
  `OPEN_DECISIONS.md` and `log.jsonl`.
- **The claim I checked first** is the one most likely to break correct code if applied as written. QA prescribed "a
  body counts while `along - radius < range + muzzle_offset`, or the line is measured from the muzzle", and the
  orchestrator named the "+1.0 hack". I checked it in `combat/hitscan.rs` (the ray is cast `stats.range` from
  `muzzle(position, dir, aim.muzzle_offset)` against the World/Character/Hitbox layers), in `aim.ron`
  (`muzzle_offset: (0.25, 0.35, -0.45)`) and in `locomotion.ron` (`capsule_radius 0.3`, **`head_radius 0.35`**, head
  sphere on the Hitbox layer via `character::head_hitbox`). Result:
  - With `radius` = capsule radius and `muzzle_offset` = 0.45 forward, the prescription is unsound. The head sphere is
    wider (0.35 m), and the muzzle also sits 0.25 m to the side.
  - "Measure the line from the muzzle" would move the origin of `spots`, `pinned`, `usable` and `queue_slot` for
    every gang and police gate, only to fix a range-only defect. I rejected it (see `log.jsonl`).
  - The diagnosis was right. QA's logged hit, shooter (33.82, 33.36) to the cop at (-11.32, 28.70) with the player at
    (0, 32), gives `along` = 45.29 m > 45 in the old predicate.

## Fixed

### 1. QA Bug 1 (major): cops shoot each other across the player at the edge of weapon range

**Derivation.** A bullet point is at most `|muzzle_flat|` + `range` from the shooter's centre in the ground plane,
by the triangle inequality: the ray starts at the muzzle and its flat length is at most `range`. The bullet can touch
a body only if that body's centre is within the widest hittable horizontal radius of such a point. So a body can be
hit only while `along < range + overreach`, where

`overreach = |flat muzzle offset| + max(capsule_radius, head_radius) = 0.515 + 0.35 = 0.865 m` (shipped data).

A body past that is out of reach whatever the aim and the spread. The bound is conservative: it may hold fire up to
about 0.07 m early, because the forward muzzle offset along the line is about 0.45 m and not 0.515 m. Nothing new is
tuned. The number is derived from `aim.ron` and `locomotion.ron`, not from a `const`.

**Code** (the shared `tactics` layer, so it covers gangs and police):
- `crates/gta_sim/src/tactics/fire_line.rs`: new `pub(crate) fn overreach(aim: &AimConfig, loco:
  &LocomotionConfig) -> f32` with a doc comment explaining why. `FireLine.range` becomes `reach` (= `stats.range +
  overreach`), and `FireLine::of` takes `overreach`. `blockers` compares `along < reach`. The `blocked`, `usable`,
  `pinned` and `queue_slot` paths all go through `blockers`, so they stay consistent.
- `crates/gta_sim/src/tactics/mod.rs`: re-exports `overreach`.
- `crates/gta_sim/src/gang/behavior.rs::gang_fsm` and `crates/gta_sim/src/police/behavior.rs::police_fsm`:
  `Res<AimConfig>` is added to the `configs` tuple, `overreach` is computed once per run, and it is passed to all four
  `FireLine::of` calls. No other behaviour changed.

**Gates**, in the new `crates/gta_sim/tests/police_range_edge.rs` (production composition via `gang_floor`):
- `cops_hold_fire_at_the_range_edge_across_the_player` is the deterministic boundary pair. Two SWAT (SMG, range 45,
  asserted `GATE BROKEN`) face each other across the player at the origin along x. Both are in `Attack`, and a named
  test mutation `Cuffed` holds them in place so the geometry stays at the boundary.
  - Gap 45.3 m: the centre is past the range. A premise check casts the real ray (`combat::muzzle` + the hitscan
    layers, a predicate on the victim's colliders only, length = range) and asserts it **does** reach the far cop
    (`GATE BROKEN` otherwise). Assert: neither cop fires in 320 ticks.
  - Gap 46.2 m: the premise ray must **not** reach the far cop. Assert: both fire at least once. This is the
    liveness side: the hold above comes from the boundary, not from a cop that never shoots.
  - Both runs assert `GATE BROKEN` if a cop leaves `Attack` or stops seeing the player.
- `surrounding_swat_never_hit_each_other_street_{8,10,11,14,11_from_respond}`: QA's 12-SWAT probe committed as a
  gate. The layout matches `zz_qa_probe.rs::swat_twelve`: player armour 1e6 at (0, 0, 32), 78 m walls, 6 + 6 SWAT at
  x = ±22..±37, 2560 ticks. Assert: 0 cop-to-cop `DamageDealt`. Liveness: at least `MIN_FIRING` = 9 of 12 cops fire
  and the player takes at least 1 hit (measured 10-11/12 after the fix; QA measured 11/12 before it). The 8 m street
  was clean before the fix too, and its comment marks it a safety case.

**Flip-RED** (`scratch/fixer2_flips.py`, output `scratch/fixer2_flips.txt`). The perturbed input is the mechanism
itself: in memory, `reach: stats.range + overreach` becomes `stats.range + 0.0 * overreach`, which is the pre-fix
predicate. The file was then restored, sha256 OK.

| Gate | Flip result | After restore |
|---|---|---|
| boundary pair | **RED**: "cops 45.3 m apart across the player fired" | GREEN |
| street 10 m | **RED**: 2 cop-to-cop hits (24, 25 dmg) | GREEN (0) |
| street 11 m | **RED**: 3 hits, the same shooter/victim/points as QA's log (tick 159 hit at (23.84, 0.45, 31.87)) | GREEN (0) |
| street 11 m from `Respond` | **RED**: 3 hits | GREEN (0) |
| street 14 m | **RED**: 1 hit | GREEN (0) |
| street 8 m | green in both (safety case, as QA measured) | GREEN |

All gang fire-line and corridor gates stay green (full suite below). Before the fix, QA had
`along < range + 1.0` → 0 hits in the 11 m layout. The derived 0.865 m does the same job and is not a guess.

### 2. `t11.py` tracker counted a cop as arrived by FSM state

`tools/qa/scenarios/t11.py`:
- `escalation()` also reads `keep_distance.1` per kind from `escalation.ron` (`arrive_m`: Patrol 18.0, Swat 12.0,
  parsed and checked).
- `Tracker(arrive_m)`: a cop has arrived once its flat distance to the player is at most its kind's `keep_distance.1`,
  whatever its state. `Dead`/`Leave` units are not tracked. That removes the far-away `Leave` patrols left over from
  the Busted run. `stuck` keeps the latest stall of a unit that has not arrived yet (no 1 m progress for 5 s), with its
  state: `Respond`/`Search` means navigation, `Attack` means a fire-discipline hold. `report()` adds `not_arrived`
  with the last kind, state and distance.
- The 4-star run no longer ends at the first SWAT within 30 m (QA: 2.6 s). It takes the screenshot then, keeps polling
  bounds and arrivals, and ends `SWAT_WINDOW_S` = 45 s after the heat write. That is 9 × `STUCK_S`, longer than QA's
  slowest 5-star arrival (33 s). The table bounds are checked on every poll during the whole window.

**Re-run** (release, `--features dev`, seed 1; `scratch/t11_fixer2/summary.json`, `busted.png`, `swat.png`):
**PASS**, `log_errors` empty, no `gta_like` process left afterwards.
- 1 star: arrival (<= 18 m) 6.5 / 7.4 s, a cop in arrest reach at 10.0 s, Busted at 11.5 s, BUSTED → Playing
  5.06 s. Respawn 0.0 m from the station, guns and bat confiscated, heat 0. `not_arrived {}`, `stuck {}`.
- 4 stars (8 units, 4 SWAT), 45 s window: all 8 arrived. Times: 12.2 / 12.2 / 15.3 / 25.1 / 25.1 / 26.1 / 30.2 /
  30.2 s. `not_arrived {}`. One stall before arrival: a unit in **`Attack`** at 13.7 m that held for 9.8 s (a
  fire-discipline hold, not navigation). The first SWAT came within 30 m at 2.5 s (28.9 m, screenshot).
- An earlier run of the same Rust build (tracker without the `Dead`/`Leave` filter, `not_arrived` polluted by
  the Busted-run patrols) gave: 1 star at 6.3 / 7.7 s, Busted 11.7 s. At 4 stars, 6 of 8 arrived at 20.4-44.2 s, and
  2 SWAT never arrived: in `Attack` at 16.4 m and 31.4 m. Every stall was in `Attack` (13-31 s), and none was in
  `Respond`/`Search`.
- **Navmesh input**: in both runs no cop stalled in `Respond`/`Search`. Every stall was a fire-discipline hold in
  `Attack` (QA Bug 3, an accepted risk). How long SWAT take to arrive varies between runs (all 8 by 30 s, or 6 of 8
  by 45 s).
- Frame: the first run reported 144 Hz Fifo, 144 FPS, no-vsync cost 2.50-2.54 ms. The second reported exactly
  60.0 FPS **both** with vsync and without it (16.67 ms), on the same Rust binary. A cap that holds without vsync
  points to a frame limiter, such as an unfocused window's low-power mode. It does not point to frame cost. The only
  difference between the two runs was the Python tracker. Say this to QA if it recurs.

## Skipped (per the binding OPEN_DECISIONS entry)

- QA Bug 2 (running at the cop's speed never breaks free): GDD-correct, owner checklist.
- QA Bug 3 (idle SWAT at about 22 m at 5 stars): accepted "single file" risk at scale, owner checklist. The new
  surround gate's liveness floor (9/12) records the current level. It does not fix it.
- Camera inside a cop body on the Busted screen: owner checklist, camera polish belongs to T13.
- `IMPL_REVIEW.md`: every item was handled in round 1 (`FIX_SUMMARY.prev-1.md`). Nothing new to act on.

## Test results

| Command | Result |
|---|---|
| `cargo test -p gta_sim -j 4 --test police_range_edge` | 6 passed (flip: 5 RED, see table) |
| `cargo test -p gta_sim -j 4` | **298 passed, 0 failed, 1 ignored** (292 + 6 new; `scratch/fixer2_sim_tests.txt`), incl. `gang_fire_lines`, `gang_combat`, `gangs`, `gang_city`, `police_fire_lines`, `police_bench` |
| `cargo test -p gta_like --bin gta_like -j 4` ×3 | 45 passed ×3 (`scratch/fixer2_client_x3.txt`) |
| `cargo clippy --workspace --all-targets -j 4 -- -D warnings` | clean |
| `cargo build -j 4`; `cargo build -p gta_like --features dev --release -j 4` | ok |
| `rustfmt --edition 2024 --check` on the 5 edited/new `.rs` files | clean |
| `python tools/qa/scenarios/t11.py --out scratch/t11_fixer2` | PASS (above) |

`citygen` is untouched (no source change there), so I did not re-run its suite. File sizes: `fire_line.rs` 232,
`gang/behavior.rs` 584, `police/behavior.rs` 442, `police_range_edge.rs` 228 (all < 750).

`git status --short`: `crates/gta_sim/src/{gang/behavior,police/behavior,tactics/fire_line,tactics/mod}.rs`, new
`crates/gta_sim/tests/police_range_edge.rs`, `tools/qa/scenarios/t11.py`,
`maw/tasks/in_progress/TASK-012/{log.jsonl,FIX_SUMMARY.md}` (+ scratch evidence). Every path is accounted for.

## Risks left

- The overreach bound covers the flat plane only. A bullet that drops or climbs steeply (a shooter on a ramp above
  the player) travels less flat distance, so the bound stays conservative. It never under-counts.
- Holding fire slightly earlier (bodies up to 0.865 m past range) can starve a far shooter a little more often at
  the range edge. The surround gate's liveness floor watches for this at the 12-SWAT scale.

children: 0 launched / 0 reported.
