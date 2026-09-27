# IMPL_SUMMARY — TASK-036

**Verdict: IMPLEMENTED, THREE OPEN DECISIONS FOR THE ORCHESTRATOR.** Every item has a fix with a
flip-RED gate. Each fix also exposed a new failure that the plan did not cover. I did not patch any of the
three, for these reasons: plan 3.3 says stop on a red C cell; the B side effect is TASK-039's class E; the A
Linux red needs a change to `around_cars`, which the plan said to keep as is.

| Item | Fix | Own gates | New failure it exposed |
|---|---|---|---|
| A: walkers orbit a car covering their node | `civilian::reached` | green on Windows, **red on Linux** (A2 heading 90) | walkers meet head-on at a car corner (Linux trajectory, 2 walkers, 89 s) |
| B: box path check misses the rear swing | `box_rules::connector_body` | green (Win + Linux) | `traffic_causes::r1_car_left_in_the_box_seed_7` red (50.9 s Win, 57.8 s Linux; bound 30 s) |
| C: yield locks a car in the box | `sirens::update` ends a Yield on a connector | green (Win + Linux) | post-fix App sweep: C-G1 up to 0.479 m (HEAD sweep: 0) |

## 1. What was implemented

Production (`crates/gta_sim/src`, +137 / -44):
- `civilian/mod.rs` (+64/-5): new `reached` helper. A lane target inside a standing car (grown by
  `capsule_radius`) counts as reached once the walker is inside the same car grown by `corner + arrive_radius`
  (1.3 m). `arrive` takes `cars` and `(clearance, corner)`. `standing_cars` is computed once per civilian
  and shared by `arrive` and `around_cars`. Added unit rows (a)-(f) (`reached_rows`). No new tuning value.
- `traffic/box_rules.rs` (+42/-24): `connector_rects` deleted. New `connector_body(graph, c, from_s, half)`
  sweeps the real body from `from_s` to the connector end, sampled by the corridor law
  (`CORRIDOR_LATERAL_STEP`, `CORRIDOR_YAW_STEP_DEG`). `connector_clear` and `repick` take `from_s` and `half: Vec2`.
- `traffic/junction.rs` (+23/-10): `BoxInputs.body: Vec2` replaces `half_width`. New `from_s(snap, c)` helper.
  All three callers (holders, queue head / `at_start` + `repick`, waiters) pass where the car stands. Module doc
  updated.
- `traffic/drive.rs` (+1/-1): `body: Vec2::new(half.x, half.z)`.
- `traffic/sirens.rs` (+7/-4): off a lane, a `Yield` returns `(Manoeuvre::None, 0.0)`. Module and `update`
  docs updated.

Gates:
- New `tests/walk_arrival.rs` (282 lines): A1 (floor hub, correctness) and A2 headings 0/90 (seed 1 spot B,
  liveness, bound 20 s).
- New `tests/traffic_box_overhang.rs` (512 lines): B1-B5 and C1-C2.
- `tests/traffic_junction_box.rs` (+40/-24): per the plan 2.4, the static `through` band is replaced by
  `on_path`, checked each tick from the holder's current `s`. The `GATE BROKEN` check that the body's own
  connector passes from s 0 is kept. Header note updated.

Docs:
- `docs/architecture/traffic.md`: capacity deferral, the rescope record in the consumers paragraph, the
  walker arrival rule and the Q4 refutation (qualified by the open Linux case), the box path check (why real
  body, why from the car, spillback numbers, open R1 seed 7), and sirens (yield ends in the box, open C-G1).
- `docs/narrative-graph.md`: the Q3 curb-clip observation added to the backlog line (Russian, like the file).
- `PCTX_PROPOSALS.md`: two implementer entries appended.

Probes (all under `scratch/`, left in place): `probe/ws/probe/tests/c_sweep.rs` (Stage 0.2/3.3 sweep and
`trace_red_cell`), `side_effects.rs` (0.3/2.5), `r1_trace.rs` (R1 seed 7 trace); `wsl/*.sh`;
`qa/run_r1_walkers.py` (R1 runtime plus a walker stall count).

## 2. Stage results and flips

**Stage 0** (`scratch/stage0/`): every reproduced row of PLAN_FINAL §2 reproduces on HEAD. A stalls 94.2 s
(heading 0) and 118.5 s (heading 90). B G1 0.533 / 0.683 / 0.612 m, plus the + floor mutual wait from
t 2.25 s to the end. C locks at `Connector(1091)` s 0.35 and `Connector(1)` s 0.75. 0.2 sweep on HEAD:
546 cells, G1 0.000 everywhere. 0.3 baseline: 0 demotions, 0 whole-box grants, 0 re-picks; worst stands
31.0 / 25.9 / 17.6 / 23.5 s on seeds 1/2/7/42.

**Stage 1 (A)** (`scratch/stage1/`):
- Reach: HEAD stalled samples sit up to 1.37 m from the car (box metric, walker 2001v0). Those walkers were
  pressed behind other walkers, 0.4-0.6 m from their nearest neighbour in 3050 of 3080 samples. The PLAN
  example, walker 2016, is at 0.94 m. I kept the reach at 1.3 m because with the fix no walker stalls on
  Windows. This departs from the plan's literal "stop if any stalled sample > 1.3 m" check; it is logged as a
  decision in log.jsonl.
- A1: arrivals at 1.5-4.9 s against a bound of 11.71 s (geometric 10.71 s plus 1 s slack). Same result in
  3 runs.
- A2: 0 walkers stall for 10 s or more on either heading. Same result in 3 runs.
- **Flip** (the covered branch of `reached` returns `false`): A1 gets 0 arrivals; A2 stalls 94.25 s and
  118.5 s. All RED.
- `traffic_pedestrian`, `civilians`, `civilian_city` and `witness_city` are green. `witness_city` gives 94/100
  with and without the fix (it needs 85).

**Stage 2 (B)** (`scratch/stage2/`):
- B1-B5 green: G1 0.000. X leaves at 3.0-3.2 s and Y at 7.9-8.8 s; Y waits for room behind X on the shared
  exit lane.
- **Flips:**
  - Old capless band: B1 G1 0.533; B3 G1 0.683; B2 Y granted at 0 s and neither car leaves. RED.
  - Holders' `from_s` = 0: B4 H demoted at 5.0 s. RED.
  - Body grown by `conflict_margin/2`: B5 Q granted over Y. RED.
  - All restored; GREEN.
- 2.5 side effects on seeds 1/2/7/42: stands, demotions, whole-box grants and re-picks are identical to the
  baseline. The swept body blocks 353-590 more waiter ticks per seed. All of these are cars on an exit lane
  (spillback) that the room check already refused.
- `traffic_junction_box`, `traffic_go_around`, `traffic_gridlock`, `traffic_occupancy` and
  `traffic_intersection` are green.

**Stage 3 (C)** (`scratch/stage3/`):
- C1 leaves the box at 2.55 s and C2 at 5.83 s, with G1 clean.
- **Flip** (plain `return None`): C1 stands at s 0.75 and C2 at s 0.19. RED; restored, GREEN.
- One tick of yield braking is left (the obstacle is evaluated before `plan`). Named here as the plan asks,
  not patched.
- **3.3: red cells, so I stopped here.** See open decision 3.

## 3. Open decisions for the orchestrator

1. **B turns R1 seed 7 red: TASK-039 class E.**
   - Bisected to B alone. With A only, the row matches HEAD exactly.
   - Trace (`scratch/stage4/r1_trace_fixed.txt` against `r1_trace_head.txt`): on box 84's east approach
     (lane 297), the real body of the right turn 713 meets the left car, where the old band left 713 clear.
     All three exits are now blocked, so every car on that approach needs a whole-box grant.
   - A whole-box grant is only given to an empty box. A waiter blocked by a body does not block the node,
     so the other approaches keep the box busy. The queue stands 50.9 s (Linux 57.8 s) against a 30 s bound;
     HEAD was 23.6 s.
   - On HEAD the same cars were granted 713 and then box-passed around the left car while holding an
     ordinary grant.
   - TASK-039 predicted this: "TASK-036 item 4 may shrink this class; take this task after it".
   - Options: (a) mark the row `#[ignore = "TASK-039 class E"]` and let TASK-039 un-ignore it; (b) add a
     box reservation for a long-blocked whole-box waiter here (a new junction rule, TASK-039 scope);
     (c) revert B.
2. **A2 heading 90 is red on Linux only: walker head-on at a car corner.**
   - Linux run: 2 walkers stand 89.25 s. Windows: 0.
   - WSL probe trace (`scratch/wsl/probe_spot_b_heading_90.log`): both walkers take their next edge beside the
     car and then meet head-on at its corner (2.07..2.67, -86.2).
   - One walker is heading for an `around_cars` corner target, the other for its lane target. Their intents
     are exactly opposite (-1.00) and their capsules touch (0.60 m).
   - Corner targets carry no `keep_right` offset, so opposite flows share one line. This is a real counter-flow
     case, a new class the fix exposes. The HEAD stalls were different: they were orbits.
   - Candidate: shift a corner target `keep_right` to the right of the walker's travel. `around_cars` is shared
     with police arrests, so this needs its own gate on the arrest rows.
3. **C: the post-fix sweep has C-G1 (plan 3.3 says stop; Q2 is the lane-end guard).**
   - `scratch/stage3/c_sweep_after.txt`: red cells on all 6 curb pairs, worst 0.479 m (seed 1, 1091/1089,
     yield begun 9.5 m before the stop line, delay 0), others 0.117-0.403 m. The HEAD sweep was clean because
     the car never left the box.
   - Trace (`c_red_cell_trace.txt`): A enters the box at the full 3.25 m curb offset, drives out with the
     offset decaying, and it and the co-granted B brake too late. Both then stand, interpenetrating, with their
     grants.
   - Options: (a) keep the fix and add a lane-end guard for the yield, then re-run the sweep; (b) revert
     `sirens.rs` and drop C1/C2 (the lock returns). The code and gates are in the tree as fix (a) needs them.

## 4. Test results

- Windows `cargo test -j 2 --no-fail-fast -p gta_sim -p citygen`: 617 passed, **1 failed**
  (`traffic_causes::r1_car_left_in_the_box_seed_7`), 9 ignored (all pre-existing). File:
  `scratch/stage4/full_suite_win2.txt`.
- Linux (WSL Ubuntu 22.04, toolchain 1.95.0, `~/gta036`, same command): 616 passed, **2 failed** (the same R1
  seed 7 row at 57.8 s, and `walk_arrival::a2_spot_b_heading_90`), 9 ignored. Files: `scratch/wsl/full_suite.log`;
  scripts `scratch/wsl/{wsl_sync,linux_suite,linux_run,probe_linux}.sh`.
- `traffic_gridlock` seeds 1/2/7/42: worst stands 31.0 / 25.9 / 17.6 / 23.5 s on Windows, identical to HEAD,
  all within 40 s.
- Clippy, exactly as `clippy.yml` (workspace `--all-targets`, and `-p gta_sim -p citygen`): clean.
- Client tests `cargo test -p gta_like --bin gta_like`: 82 passed. `python tools/qa/tree_check.py`: passed.
- Bench means on Windows (HEAD → after), all well under `MEAN_LIMIT`:
  - traffic 1.885 → 2.088 ms (limit 19)
  - police 1.863 → 1.838 ms (limit 11)
  - civilian 1.871 → 1.647 ms (limit 8)
- Runtime R1 spot B (`scratch/qa/r1/B_1`, `B_2`; harness `scratch/qa/run_r1_walkers.py`, `--settings-id .qa`
  through `pt.Session`, one game at a time; no game left running). Metric: Wander walkers within 6 m of the
  left car that moved < 0.7 m, 10 s or longer.
  - B_1: 0 walkers stalled, 0 traffic stands over 30 s.
  - B_2: 2 walkers stalled (76.6 s and 11.1 s). Traffic: one car stood `Dynamic` for 148.8 s at (6.8, -81.2),
    and several stood 40-49 s. In that run the harness snapped the player's car 7.4 m onto the spot while
    `switches_by_cause` rose during the drive-in. That is the TASK-037 snap caveat (the snap can land on a
    queue), so it is not attributed to the change without a trace.
  - TASK-037 QA for comparison: 9 and 6 walkers, up to 103 s and 150 s.
- CI 5/5: not run (the orchestrator merges).

## 5. How to verify manually

- `cargo test -p gta_sim --test walk_arrival -- --nocapture`: A1 prints arrivals; A2 prints the stalls per
  heading.
- `cargo test -p gta_sim --test traffic_box_overhang -- --nocapture`: B1-B5, C1-C2.
- `cargo test -p gta_sim --test traffic_causes r1_car_left_in_the_box_seed_7 -- --nocapture`: red (open
  decision 1).
- Probe sweeps: in `scratch/probe/ws/probe`, run
  `CARGO_TARGET_DIR=D:/test-gta-like/target cargo test --offline --test c_sweep -- --nocapture --test-threads=6`,
  and `--test side_effects` with `B_SHAPE=band|body`.
- Owner run: park a car across the spot B crosswalk (5.3, -84.5). Walkers should step past it instead of
  circling; watch for two walkers facing each other at a corner.

## Deviations from plan

- 0.3 probe: the plan asks to `#[path]` the `traffic_gridlock` fixture. Its `run` is a private test function,
  so I copied the setup (city, player at the spawn looking north, 120 s) and named the source in the probe
  doc. I also added side-by-side band/body blocked-waiter counts.
- `arrive` got `#[allow(clippy::too_many_arguments)]` (8 parameters), and so did `repick`.
- A1 slack is 1.0 s. The measured arrivals are all under the geometric bound alone, with no spread across 3
  runs.
- The A reach is kept despite HEAD stalled samples at up to 1.37 m (see Stage 1).
- WSL mirror `~/gta036` was seeded by copying `~/gta038`, including its target dir, to reuse the dependency
  build.

children: 0 launched / 0 reported
