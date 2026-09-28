# IMPL_SUMMARY — TASK-039 (universal traffic progress rule)

Plan: `PLAN_FINAL.md` with the binding "Orchestrator amendment after PLAN_BLOCKED" (the cycle trigger
dropped, N1 re-derived on the real geometry). Evidence under `scratch/stage0..4` and `scratch/wsl`.

**Status: stopped at the Windows-green boundary (budget and second-failure rules).**
- Windows: the whole `gta_sim` + `citygen` suite is green (630 passed, 0 failed, 8 ignored), with clippy,
  client gates and the tree check clean. The new rows (M1, N1, plumbing, oracle) pass, and so do the
  un-ignored class-E rows (G4 seed 7 liveness, R1 seed 7) and the re-anchored b3.
- Not met:
  - `r1_car_left_in_the_box_seed_1` stays ignored. Its residual after class D is 33 s of grant queueing.
  - Linux has two regressions that HEAD does not have: `traffic_causes::c_character_in_the_lane` (a
    `Dynamic` car stands 47.3 s) and `traffic_go_around::dummy_street_seed_7` (30.1 s against the 30 s
    bound).
  - Runtime QA (stage 5) was not run.
  - Section 5 has the details and a redesign note.

## 1. What was implemented

Production (`crates/gta_sim/src`):

| File | +/- | What |
|---|---|---|
| `traffic/progress.rs` | new, 564 | `TrafficHooks` (the app's `CollisionHooks`: `modify_contacts` drops a relaxed pair), `exempt_pair` / `planning_blocker` / `sensing_blocker`, `upkeep` (end rules + `PassingThrough` / `TnuaNotPlatform` markers), the wait-for record (`EdgeKind`, `Edge`, `edge_of`), `roots` (pointer walk), `overlap`, `detect` |
| `traffic/mod.rs` | +24 -2 | `Relax`, `TrafficCar.relaxed`, `TrafficStats.{progress_relaxations, progress_recoveries, unexplained}`, `(progress::upkeep, drive::advance_traffic).chain()` |
| `traffic/config.rs` | +39 | `ProgressConfig { wait_seconds, grace_seconds, max_seconds, speed }` + validator rules |
| `traffic/drive.rs` | +59 -17 (704 lines) | `Snap: Clone + Copy`, `leader` skips the relaxed blocker and returns the index, squeeze speed cap, per-car edges, `detect` after the per-car loop, `stay` blockers, stats carried |
| `traffic/manoeuvre.rs` | +80 -15 | `Sensed { ahead, beside, relaxed_hit }`, sensing skips the blocker, relaxed branch in `plan` (a lane pass that can go at once, else through), `box_pass` split out |
| `traffic/junction.rs` | +82 -37 | returns the waits map (`Grant` / `Follow` / `Body` per ungranted waiter), `lane_start_free` returns the body, `ignore` on the path checks, `repick_relaxed` for a relaxed head, in-box waiters granted first |
| `traffic/box_rules.rs` | +54 -5 | `ignore` on `connector_clear` / `repick`, `repick_relaxed` |
| `traffic/recover.rs` | +53 -37 | `nobody_coming` / `corridor_clear` return the blocking body and skip the relaxed blocker; `Recovery::Stay { blocker }` |
| `traffic/contact.rs` | +47 -18 | `penetration`, switch skips (predictive and backstop) for the relaxed pair |
| `traffic/spawn.rs` | +2 | `relaxed: None`, `ActiveCollisionHooks::MODIFY_CONTACTS` |
| `vehicle/mod.rs`, `vehicle/chassis.rs` | +5, +10 -3 | `PassingThrough(Entity)`; wheel rays `cast_ray_predicate` skip the relaxed partner |
| `lib.rs` | +1 -1 | `PhysicsPlugins::default().with_collision_hooks::<traffic::TrafficHooks>()` |
| `assets/traffic/traffic.ron` | +8 | `progress: (wait_seconds: 16.0, grace_seconds: 6.0, max_seconds: 30.0, speed: 3.0)` |

Gates (`crates/gta_sim/tests`):

| File | What |
|---|---|
| `traffic_progress.rs` (new, 838 lines: over the 750 warning, under 950) | `oracle_sees_a_car_through_a_dummy`; plumbing `relaxed_car_drives_through_a_standing_player`, `relaxed_pair_ends_only_when_separated`, `relaxed_pair_has_no_contact`, `relaxed_car_passes_an_awake_parked_car`; city `m1_two_bodies_block_both_lanes_seed_1`, `n1_player_standing_on_the_crosswalk_seed_1` |
| `traffic_support/third_body.rs` (new, 173) + `mod.rs` (+12, 747 lines) | third-body oracle (opt in `with_third_bodies`: kinematic car x character, x static world), the relaxed-pair exemption with `relaxed_max_depth`, `StandClock::current` |
| `traffic_causes.rs` | b3 re-anchored (`bumped(label, pressed_s, ..)`, pressed `wait_seconds + 12`, asserts: never given up, stand <= 30 s, rear past the dummy before removal, dummy unhurt); R1 rows with third bodies; `r1_car_left_in_the_box_seed_7` un-ignored; seed 1 re-ignored with its residual (section 2) |
| `traffic_junction_box.rs` | third bodies, relaxed depth printed, lease clause excludes a holder relaxed against the body, `seed_7_box_keeps_moving_liveness` un-ignored |
| `traffic_occupancy.rs` | `stops_short` watched `wait_seconds - 1` s (the progress rule squeezes the car past the dummy after it) |
| `traffic_gridlock.rs` | prints progress relaxations / recoveries per seed (plan 4.2) |
| `config_traffic.rs` | 4 sabotage rows (wait <= grace, grace < character_seconds, speed > pass.speed, max < 0) |

Docs: `docs/architecture/traffic.md` new section "Progress: wait-for record and relaxed pass" (edges,
eligibility, order, consumers by phase, hook, wheel rays, Tnua, end rules, values, D10 estimate, exit-lane
observation), box section "Open (TASK-039)" resolved, Modes line; `docs/design/GDD.md` §5.2 new bullet
(D1 recorded) and the R1 sentence updated.

## 2. Deviations from the plan (each logged in `log.jsonl`)

1. **Amendment applied**: no cycle trigger, `cycle_seconds`, example 2 or flips (b)/(h). N1 is the lane
   head of lane 297 (player 0.5 m before its stop line), row `n1_player_standing_on_the_crosswalk_seed_1`
   gates on the real geometry. Example 3 from the stage-0 trace: head stands at ~5.5 s behind the player
   (bumper gap 2.0 m); at T 16 it is relaxed (stood >= grace), drives past at 3 m/s; each next head stands
   grace 6 s + ~3 s. Measured: worst N1 stand 19.8 s, 5 relaxations against the player in 100 s.
2. **Dynamic relaxed cars skip their blocker in sensing** (the plan kept them braking to recover first).
   b3 showed a car pinned between the dummy and the pusher behind it (inside the rest skin): with one
   relaxation it could never recover. Now the autopilot drives it through at the squeeze speed; "past B"
   applies to it too.
3. **Eligibility**: sink stood `wait_seconds` OR the waiter itself stood `wait_seconds` and the sink the
   grace (R1 seed 1: a nudge of the left car at 12 s restarted the sink clock).
4. **"Being passed" exclusion counts only passing relaxations**: a relaxed car whose own edge is a Body
   edge to another body is stalled (M1: the eastbound head relaxed against D's corner stood behind P and
   kept D waiting 120 s).
5. **A relaxed car never takes a box pass**; accepting drops any pass around its blocker (G4 seed 7: a
   whole-box pass relaxed the same tick left its claim over the oncoming exit and waited for that car's
   grant, a claim/grant cycle). This matches the planner's own log decision "never an offset in the box"
   and the amendment ("drop the in-box clean-first branch").
6. **Follow -> Body post-pass only for a leader not driven by the AI** (a bailing car), and the room edge
   falls back to a car granted into the lane. Found by the plan 4.2 gridlock trace: followers were being
   relaxed through their own queue head (5-11 per 120 s with nobody playing); now 0.
7. **Junction: an ungranted waiter already on its connector is granted first** (R1 seed 1: 41 s -> 33 s).
8. **`wait_seconds` 16** (plan 3.6 latency rule: a row stood > 27 s).
9. Plumbing row b): the plan's "the car then stops inside the moved dummy" is false (the dummy is behind
   the nose); the row asserts the exemption holds while the car drives out and ends only on separation.
   The row a) control (a second unrelaxed car switching) is not built: an unrelaxed kinematic car brakes
   for the player and never touches him; the hook flip covers the mechanism instead.
10. Stage-0 probe crate not built: fixtures and traces were run in-tree (`scratch/stage0/`), which is the
    same evidence without a second bevy build.
11. Plan 1.5 baseline ran through a temporary env switch in `Footprints` (removed): 27 of 27 city oracles
    had 0 third-body violations on HEAD (`scratch/stage1/baseline.txt`).
12. **Not met: `r1_car_left_in_the_box_seed_1` stays ignored.** Class D itself is ended (the car leaves
    `Dynamic` after 16.3 s, HEAD 113 s), but the north queue of box 83 then waits for grants serialized
    behind slow crossings (holders squeezing at 3 m/s, one crawling behind walkers): 33.0 s and 30.9 s
    against the 30 s bound (HEAD 143 s). Three changes aimed at it (items 7, 8 and a per-kind grace, the
    last reverted: no effect); stopped by the second-failure rule. Redesign note: the residual is junction
    throughput, not a stand behind a stationary body; options are a faster squeeze in the box (owner feel
    value `progress.speed`), a grant order that lets non-conflicting approaches through while a squeeze
    holds the box, or re-anchoring R1 to "stands behind a stationary body" (a gate change for the owner to
    decide).
13. G4 lease clause flip (lease off on a non-relaxed holder) cannot go RED on the fixed code: no holder is
    stale longer than 0.67 s in these scenes (`scratch/stage3/flips/lease.txt`); the clause is currently
    not exercised there.

## 3. Test results

Windows (all with `-j 2`):
- `cargo test -p gta_sim -p citygen --no-fail-fast`: 630 passed, 0 failed, 8 ignored
  (`scratch/stage4/full_suite_final.txt`). Benches: traffic mean 2.08 ms, 64 civilians 2.44 ms, SWAT
  2.26 ms (all under `MEAN_LIMIT`). `traffic_gridlock` seeds 1/2/7/42: 0 relaxations, worst stands 31.4 /
  25.9 / 17.6 / 23.5 s (bound 40).
- `cargo clippy --locked --workspace --all-targets -- -D warnings` and `-p gta_sim -p citygen`: clean.
- New rows: oracle row, 4 plumbing rows, M1, N1 green. M1: worst stand 19.0 s (D in `Dynamic` 19.0 s),
  8 / 13 cars past the scene per lane, relaxations in least-overlap order S->D and N->D (13.97 s), D->P
  (18.6 s), N->P; relaxed max depth 1.80 m (D through P). N1: worst stand 19.8 s, player unhurt, height
  change 0.018 m, relaxed depth 1.45 m. b3: rear past the dummy at 18.8 s (removed at 28 s), stand 16.4 s.
  R1 seed 7 green (worst 25.4 s), G4 seed 7 liveness green (worst 24.9 s, relaxed depth 0.92 m).

Flips (input perturbed -> RED; restored -> GREEN; files in `scratch/stage2/flips.txt`,
`scratch/stage3/flips/`, `scratch/stage4/flip_trigger_final.txt`):
- `TnuaNotPlatform` not inserted: player lifted 0.24 m (row a), dummy 0.25 m (row b).
- hook returns `true`: rows a, b, c, d RED (CollisionStart, parked car shoved 4.5 m).
- end rule "B moved -> None at once": row b RED (CollisionStart, switch, dummy pushed 1.7 m).
- wheel-ray predicate off: row d RED (awake parked car rolled 180 deg); row c stays green (asleep).
- sink trigger off: M1, N1, b3, G4 seed 7 RED (re-run on the final code). R1 seed 7 is green on Windows
  HEAD too (26.6 s), so its class-E evidence is G4 seed 7.
- relaxation off (consumers ignore it, contacts on): M1, N1, b3, G4 seed 7 RED.
- relax-all (every body skipped while relaxed): M1 G1 violation (the eastbound car through P while relaxed
  against D) and a G4 seed 7 third-body violation: RED.
- order reversed (deepest overlap first): M1 RED (D->P first, eastbound queue stands 113.8 s).
- grace replaced by min(standing W, standing B) >= T: M1 and G4 seed 7 RED; N1 stays green (22.6 s).
- `wait_seconds` 40: M1 (43.0 s) and N1 (39.4 s) RED.
- config sabotage rows each fire their own keyword.

Linux (WSL Ubuntu 22.04, toolchain 1.95.0): see section 5.

## 4. How to verify

- `cargo test -p gta_sim --test traffic_progress` (7 rows), `--test traffic_causes`, `--test
  traffic_junction_box`, `--test traffic_gridlock`, `--test config_traffic`.
- `TRAFFIC_TRACE=1 cargo test -p gta_sim --test traffic_progress -- --nocapture m1` prints per-second
  stands with each car's `relaxed`.
- In the game (dev build): leave a car across a street next to another stopped car, or stand still on
  the crosswalk at (7.9, -81.3) on seed 1: after ~16 s the waiting car squeezes past at 3 m/s.

Owner run (not gated): the look of a car squeezing past a left car, past two cars blocking a street (M1:
D rejoins through P, depth up to 1.8 m), and past the player on the crosswalk (1.45 m).

## 5. Linux, runtime QA, redesign note

Linux (WSL Ubuntu 22.04, toolchain 1.95.0, `scratch/wsl/linux_run.sh`, logs `scratch/wsl/*.log`):
- green: `traffic_progress` (7), `traffic_junction_box` (6, seed-7 liveness included), `traffic_gridlock`
  (5), `traffic_recovery` (10); `traffic_causes` R1 seed 7 and b3 green.
- RED, **regressions against HEAD** (HEAD on the same Linux mirror: both green, `head_c.log`,
  `head_go.log`):
  - `traffic_causes::c_character_in_the_lane`: an AI car stands 47.3 s in `Dynamic` at a junction exit
    (351.5, 490.7), 60 m from the scene (HEAD: 97 s stands there too, but none in `Dynamic` > 30 s). Trace
    (`c_dbg.log`): a pile of `Dynamic` cars at the lane-462 end. There was one real cycle (2088 <-> 2078,
    nose to tail, `root None`), then a stream of relaxations against 2078 (2124, 2080, ...). 2078 is
    either "being passed" or waiting on moving bodies, and it starves in `Dynamic`. A symmetric
    exemption (the car being passed ignores its passers in recovery and in the predictive switch) made it
    58.3 s (`c_sym.log`). Reverted.
  - `traffic_go_around::dummy_street_seed_7`: a queue car stands 30.1 s (HEAD 16.1 s) creeping behind a
    lane where 6 cars squeezed through the dummy at 3 m/s (`go_dbg.log`). On HEAD they took the clean lane
    pass at `pass.speed` 6 m/s.
- Tooling note: `rsync` keeps Windows mtimes, so the mirror scripts now `touch` the synced sources.
  Without that, a HEAD build in the same target made cargo run a stale binary (PCTX proposal).

Runtime QA (the TASK-040 repros over BRP): **not run**. Stage 4 is not green on Linux. Path-fixed copies
of the three scripts are ready in `scratch/qa/` (`m1_fixed.py <out> 34.6 -79.1`,
`repro_player_in_junction.py <out>`, `repro_abandoned_car.py <out> 1`). They import the TASK-040 harness
from `maw/tasks/done/TASK-040/scratch/tools`.

Redesign note. The failures that remain have one shape: a stream of relaxations against one body. Only
one relaxation per car is possible, and the plan's order rules (least overlap, "a car being passed is
never moved") starve the body's own turn. The squeeze runs at 3 m/s, so the box and the lane stay held
longer than a clean pass would hold them.
- R1 seed 1: the north queue waits for grants behind slow crossings.
- Linux c_character: the `Dynamic` car inside a pile keeps being passed.
- Linux go_around seed 7: the queue creeps behind 3 m/s squeezes.

Candidate directions for the orchestrator:
- (a) Limit relaxations against one blocker to one planning passer at a time, so the blocker's own turn
  comes between passers.
- (b) Prefer relaxing the blocker itself when it is an AI car that stands (`Dynamic` or queued) over
  relaxing a passer through it.
- (c) Raise `progress.speed` (the owner's feel value) toward `pass.speed`. The squeeze would then hold
  the lane and the box no longer than a clean pass.

None was tried: the second-failure rule applies.

## Final checks (on the final code, Windows)

- `cargo test -p gta_sim -p citygen --no-fail-fast`: 630 passed, 0 failed, 8 ignored.
- Traffic rows re-run after the last revert: all green (`traffic_progress` 7, `traffic_causes` 12 + 1
  ignored, `traffic_junction_box` 6, `traffic_gridlock` 5, `traffic_occupancy` 10, `config_traffic` 6,
  `traffic_recovery` 10).
- `cargo clippy --locked --workspace --all-targets -- -D warnings`: clean.
- `cargo test -p gta_like --bin gta_like`: 82 passed.
- `python tools/qa/tree_check.py`: passed.
- `cargo tree -p gta_sim -e normal -i bevy_render`: empty.
- `cargo fmt --check`: clean for the files touched.
- No new crate; `Cargo.lock` unchanged.
- `git status`: only the task's files. `metrics.md` was already modified before this stage.

## Children

children: 0 launched / 0 reported
