# IMPL_SUMMARY — TASK-037 (in-view box lock)

**Verdict: PARTIAL.** Stages 0-4 are built and gated, and stage 5 is done for the rows that pass.

Green now:
- G4 seed 1 liveness (un-ignored).
- (c) and rb far, with the city-wide `Dynamic` bound, on Windows and Linux.
- rb near and spot C.
- In every traced row, walkers pinned at a car went from 52-94 s to 0 s.

Open, handed to the orchestrator (`OPEN_DECISIONS.md`, entries dated 2026-09-27, implementer):

1. **Class E** (not in the plan's cause table): an approach whose every exit crosses the left car, and
   no box pass fits. G4 seed 7 liveness (61 s) and R1 seed 7 (100-106 s) stay red and ignored, with
   that reason.
   - Why the box pass fails: side +1 needs a 5.71 m shift, more than pitch + slack (3.67 m). Side -1
     runs into the oncoming approach's own queue head at its stop line, and into walkers on the exit
     crosswalk.
   - The only rule that misfires is `connector_rects`, which has no corner overhang, so a grant can be
     given into a blocked path. That band belongs to TASK-036 item 4.
   - Named fallback, not built: re-route already runs (`repick`) and finds no clear exit. A U-turn needs
     new graph connectors kept out of the random exit choice. **This needs a decision.**
2. **Class D, R1 seed 1**: 113 s in `Dynamic` (was 149 s). Two fixture attempts (Q3) failed and were
   reverted. Option B (Q4) is built, which fixed class D in (c) on Linux, but it is not enough here.
   It stays ignored.
3. **G4 extra row** (55 s > 40 s): not a lock. The lane car behind the extra car waits out two grant
   cycles at the box the left car narrows. It stays ignored with that reason.
4. **Regression: `traffic_go_around::avenue_seed_2` is red on Windows only** (not ignored; green on
   Linux). It is a latent contact in TASK-032's pass law:
   - A lane passer merging back at 1.6 m/s yaws 31 deg.
   - Its front corner reaches 0.18 m into the adjacent oncoming lane, where a `Dynamic` car at 9.6 m/s
     touches it.
   - G1 is clean. The connector sensing change moved the trajectories so the two cars meet.
   - Options are in OPEN_DECISIONS. **It blocks a merge until someone decides.**

## 1. What was implemented

| File | +/- | What |
|---|---|---|
| `crates/gta_sim/src/traffic/recover.rs` | +83/-30 | `reach_across` extracted from `off_lane`. `on_path`: a connector car recovers only while its footprint is within the conflict table's band (`half.x + conflict_margin/2`). `rejoin()` shared by the predicates. **Rest skin (Q4 option B):** against a vehicle body at rest, the skin is `switch.skin` + the car's own rejoin excursion (offset + corner arc of its turn and of the heading swing at rest), capped at `recover.skin`. It applies in `nobody_coming` and `corridor_clear`. Walkers keep the full skin. |
| `crates/gta_sim/src/traffic/manoeuvre.rs` | +91/-14 | `holds_offset` (a pass or a rejoin). `sweep_ahead`: on a connector the car body is swept along the path, onto the exit lane, over `turn_sense_distance`. Samples follow the corridor law (0.3 m or 7.5 deg). Only bodies with a part ahead of the nose line count. The gap is bisected to 0.01 m. Oncoming claims count unless they overlap sample 0. |
| `crates/gta_sim/src/traffic/drive.rs` | +11/-8 | A rejoin offset steps on a connector and survives the connector end. A rejoin on a connector slides (heading = path tangent). |
| `crates/gta_sim/src/traffic/lateral.rs` | +10/-4 | `CORRIDOR_*` consts moved here (`pub(super)`). The `effective_lateral` flag is renamed `holds`. Docs updated. |
| `crates/gta_sim/src/traffic/box_rules.rs` | +1/-1 | `path_pose` is now `pub(super)`. |
| `crates/gta_sim/src/occupancy/query.rs` | +73/-2 | `RoadOccupancy::first_in(rects, dirs, skip)`. |
| `crates/gta_sim/src/civilian/mod.rs` | +81/-1 | `standing_cars`: standing `Rect` bodies within `avoid_distance` + half diagonal. `civilian_fsm` steers through `tactics::around_cars` with the police arrest values and gains `Res<RoadOccupancy>` and `Res<LocomotionConfig>`. Unit row `standing_cars_rows`. |
| `crates/gta_sim/src/tactics/fire_line.rs` | +27/-13 | **Deviation:** `around_cars` fixed in the rule itself. (a) It now prefers a corner that the walker's body clears by `clearance` (it used to take a line only the centre clears, a graze), and falls back to 0. (b) A corner within `corner - clearance` counts as reached; it used to re-pick the corner the walker stood on. A new assert in `around_cars_rows`. |
| `crates/gta_sim/tests/traffic_recovery.rs` | +243/-4 | Rows (f) in-band / out-of-band on a connector and (g) a car / a walker at rest 0.25 m beside. The e-row doc and message are reworded. |
| `crates/gta_sim/tests/traffic_intersection.rs` | +288/-1 | `a_car_beside_the_curve_does_not_stop_a_turn`, `a_car_on_the_curve_stops_at_the_jam_gap` (`JAM_TOLERANCE` 0.1, derived), `a_body_at_the_outer_rear_flank_does_not_hold_a_turn`. |
| `crates/gta_sim/tests/traffic_pedestrian.rs` | +117/-26 | The flank row is re-anchored on the player (`held_scene`). New `a_walker_goes_around_a_car_across_its_run`. |
| `crates/gta_sim/tests/traffic_junction_box.rs` | +8/-7 | `seed_1_box_keeps_moving_liveness` is un-ignored. The other ignores name their cause. The non-liveness label reads "(asserted in the _liveness twin)". |
| `crates/gta_sim/tests/traffic_causes.rs` | +18/-58 | (c) and rb assert `dynamic_violation()` city-wide. `dynamic_bound_on` and `LeftInBox::approaches` are deleted. The R1 ignores name D/E. **Spot C fixture fix:** the fill now spawns behind the last AI car on the lane, moving or standing. The old fill spawned into a rolling car's spot (G1 2.4 m), which new trajectories exposed. |
| `docs/architecture/traffic.md` | +30/-10 | Modes: connector recovery band, residual band, rest skin, sliding rejoin. One tick item 5: connector sweep, nose rule, bisection. Junction box: what TASK-037 resolved and what stays open. |
| `docs/design/GDD.md` | +1 | §6.2: "Пешеход обходит машину, стоящую у него на пути." |

## 2. Deviations from the plan

- **Class E** was found at the 1.8 re-trace and survived stages 2-3. It went to OPEN_DECISIONS before
  any code. No patch was tried and the fallback was not built (see above).
- **3.1:** `around_cars` needed the two fixes above.
  - Without (a), the new walk-around row is RED (the walker stands 2.95 s grazing the car).
  - Without (b), in G4 seed 1 walker 2049v0 stood 75 s on a corner of 2109v0.
- **3.3:** the pressed capsule sinks only 0.003 m (deepest 1.497 vs contact 1.500), so it never gets
  0.02 m inside. As the plan's fallback allows, the precondition is now contact
  (`< half.x + r + 0.01`), and the sabotage is "cast from the centre, half width + capsule radius, gap
  minus half length".
- **4.1** was tried and reverted.
  - The G4 distance rule was a no-op: the hub was free at placement.
  - Adding "no grant at the node" was never satisfiable within 5 s, so all five `box_scene` rows went
    GATE BROKEN.
- **4.2 was built**, but differently from the plan. The plan's fixed 0.18 m skin (`switch.skin +
  rate_at_rest x horizon`) failed `e_blocked_corridor`: a lane rejoin's yaw swing moves the corners
  farther than that (flip s4_C). So the skin is derived per car from the car's own rejoin excursion.
  - There is no config field and no validation change: the cap at `recover.skin` makes it safe by
    construction.
  - It was built because the Linux run of (c), with the city-wide bound, reproduced class D (Q4's own
    trigger) on a different pair of cars.
- **5.5:** TASK-038's WSL scripts no longer exist, so they were rewritten in `scratch/wsl/`
  (`wsl_sync.sh`, `linux_run.sh`, `probe_linux.sh`).
- **5.7:** runtime R1 was not run, because classes D/E are still open and a QA run now would measure an
  intermediate state. It is left for QA after the decisions.

## 3. Test results

- **Stage 0 baseline** (`scratch/stage0/`): the 3 liveness rows red (95.5 s Dynamic), the r1 rows red.
  Benches: traffic 1.52 ms, police 1.51 ms, civilian 1.42 ms.
- **Flip-RED** (runner `scratch/flip.py`, specs `scratch/flips_stage{1,2,3,4}.json`, outputs
  `scratch/flips/`). Every flip below went RED:
  - s1_A `on_path` lane-only: the in-band car never goes kinematic.
  - s1_B connector step reverted: the rejoin never ends.
  - s1_C slide removed: widest reach 1.456 > 1.350.
  - s1_D `on_path` everywhere: the out-of-band car recovers.
  - s2_A straight strip: the beside-the-curve car never leaves; on the curve the gap is 0.81.
  - s2_B `gap = d_k`: 1.790 vs 2.000 ± 0.1.
  - s2_C no nose rule: the outer-rear-flank car moves 0.00 m.
  - s3_A cast from the centre: the car moves 0.00 m.
  - s3_B empty car list: the walker stands 8.6 s against the car.
  - s3_C standing filter dropped: the unit row fails.
  - s3_D graze fix reverted: the walker stands 2.95 s.
  - s4_A rest skin = `recover.skin`: the g car row fails.
  - s4_B rest skin on walkers: the g walker row fails.
  - s4_C fixed 0.18 m skin: the e row fails.
  - s3_E reached-corner rule removed: `around_cars_rows` fails (the walker's own corner has the
    smallest via-distance).
- **Windows** `cargo test -p gta_sim -p citygen --no-fail-fast` (`scratch/stage5/full_suite_2.txt`):
  599 passed, 1 failed (`traffic_go_around::avenue_seed_2`, above), 11 ignored.
  - `traffic_gridlock` 1/2/7/42: green.
  - citygen first showed a phantom red: its binaries were linked from another worktree sharing
    `target/` (the TASK-009 lesson). After `touch` and a rebuild it is green.
- **Linux** (WSL Ubuntu-22.04, rustc 1.95.0, CI env, `scratch/wsl/*.log`): all green.
  - `traffic_junction_box`: 4 passed, 2 ignored.
  - `traffic_causes`: 11 passed, 2 ignored.
  - `traffic_gridlock`: 5 passed.
  - `traffic_recovery`: 10 passed.
  - `traffic_intersection`: 6 passed.
  - `traffic_go_around`: 10 passed.
  - `traffic_pedestrian`: 5 passed.
  - `witness_city`: 1 passed.
  - An earlier Linux run, before option B, had (c) red with a Linux-only class-D stand of 88 s. That is
    what triggered 4.2.
- **G1:** max depth 0.000 in every touched city gate.
- **Benches** (release, `scratch/stage5/benches.txt`, before option B): traffic 1.48 ms (limit 19),
  police 1.46 ms (11), civilian 1.41 ms (8). Option B only touches the recovery of `Dynamic` cars.
- **Other checks:**
  - `cargo clippy --workspace --all-targets -D warnings`: clean (re-run after option B).
  - `cargo test -p gta_like --bin gta_like`: 82 passed.
  - `tools/qa/tree_check.py`: passed.

## 4. How to verify manually

- Run `cargo test -p gta_sim --test traffic_recovery --test traffic_intersection --test traffic_pedestrian -- --nocapture`.
  The new rows print reach, gaps, rest skins and walk-around times.
- Run `cargo test -p gta_sim --test traffic_junction_box --test traffic_causes -- --include-ignored --nocapture`.
  It shows the open D/E rows with their stands.
- The read-only probe is `scratch/probe/ws/probe`. Build it with
  `CARGO_TARGET_DIR=D:/test-gta-like/target cargo test --offline --test trace -- --exact <row> --nocapture`.
  - `PROBE_WATCH=<entity,...>` logs grant and state changes and walker positions.
  - `tests/go_around.rs` traces avenue seed 2 (`PROBE_SENSE`, `PROBE_NEAR`, `PROBE_TICKS`).
  - On Linux: `scratch/wsl/probe_linux.sh`.
- Owner run (not gated), what to look at:
  - Walkers step around a car standing on a crosswalk.
  - A bumped car in the box slides back onto its line.
  - Turning cars no longer stop 2 m short of a car beside the curve.

children: 0 launched / 0 reported
