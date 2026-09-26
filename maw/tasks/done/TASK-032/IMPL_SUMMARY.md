# IMPL_SUMMARY — TASK-032 (stages 1-7)

Two implementer runs. Run 1 closed stages 1-5 (committed as f95461d, wip; stage-6 code was written but
untested when the session ended). Run 2 (this one) verified stages 1-5, built stage 5b (orchestrator
redesign note), finished and gated stage 6, and did stage 7. Run 2 is committed on
`feature/oncoming-lane` as 9b5c045 (on top of 193ca54).

**Headline for the orchestrator:**
- G1, G2, G3, G4, G5, G7, G8 green with flips. Stage 5b (stuck cheat) green with flips.
- **G6 is not met** (0/10 streets, 2/10 avenues). The pre-decided fallback chain ran out, so the stop
  rule applies: `REDESIGN_NOTE.md` has the numbers and options. Both G6 rows are `#[ignore]`d with the note
  as the reason.
- **R1 runtime is RED** (3/3 leave runs, 11-15 cars stand ~150 s; control 0). The car left in the box
  in view is the stage-5 residue the orchestrator accepted, but R1 is exactly that case. At the
  baseline spot it is worse than pre-TASK-032. Details and options are in `REDESIGN_NOTE.md`.
- t15 runtime: pressure in **5/6** runs (criterion ≥ 5/6). Hijack passes 6/6, at most 2 active cars.
- Suite, clippy, client tests and tree_check are green. The 5 CI workflows run after the merge (not
  run here).

## 1. What was implemented

### Stages 1-5 (run 1, verified green at the start of run 2)
All six stage-1..5 test files were green before any change (`traffic_causes` 7 + 1 ignored,
`traffic_occupancy` 7, `traffic_recovery` 5, `traffic_go_around` 10, `traffic_junction_box` 3,
`traffic_gridlock` 5). Stage decisions and flips are in `log.jsonl` (entries of 09:01-12:19Z). In short:
- Stage 1 causes: (a) left car, (b3) dummy at the bumper of a Dynamic car, (c) character in the lane
  and (d) car on a connector reproduced RED. (b1) nudge, (b2) shove and (b4) yaw did not reproduce and
  stayed as regression rows. G1 baseline on gridlock: 0 violations.
- Stage 2: `occupancy/` (`RoadOccupancy`, `first_along`, `blocked`, `world_clear`). Sensing, lane
  start and spawner moved onto it. G8 rows (flip RED). Gridlock drift reported.
- Stage 3: `recover.rs`. G3 a/c/d/e plus flips.
- Stage 4: `pass.rs`, `manoeuvre.rs`, `lanes.rs`, `lateral.rs`. G2 has 10 rows: the first 8 queued cars
  pass in 7.7-49 s and the worst stand is ≤ 21.9 s. Flips in the log. A pass that reaches the lane end
  is refused (plan step 6 not built).
- Stage 5: `box_rules.rs` and junction D1/D2/D3/D5. The stop rule was hit on cause (d); the orchestrator
  wrote a redesign note, which became stage 5b.

### Stage 5b (run 2): out-of-view stuck cheat
- `crates/gta_sim/src/traffic/stuck.rs` (new, 62 lines): `despawn_stuck` is a `Bubble` system chained
  before `despawn_traffic`. It despawns two kinds of body once they have stood out of frame (the
  spawner's `in_frame` predicate at `in_view.despawn`) for `bubble.stuck_despawn_seconds`: a traffic
  car (not `Taken`), and a driverless non-police car in a junction box (`graph.in_junction(p, 0)`). The
  clock is standing (the occupancy `standing > 0`) and out of frame, counted in a pruned `Local` map.
- `traffic/config.rs` (+4) and `traffic.ron`: `bubble.stuck_despawn_seconds: 45.0`. The comment
  explains why 45: it is above the 40 s TASK-033 stand bound, so no liveness gate is masked. There is a
  validate rule and a config row.
- `tests/traffic_causes.rs`: cause (d) is now two rows, 12 m from the connector entry:
  - `d_car_left_on_a_connector_out_of_view`: nothing near the box stands > 46 s and the left car is
    gone. Flip (`out < limit * 1000`) → RED, stands 120 s and the car stays.
  - `d_car_left_on_a_connector_in_view_is_the_residue`: the cheat never despawns the car the player
    looks at. Stands are printed: 13 cars at 47-108 s. Flip (`in_frame` dropped) → RED, the car is
    despawned in frame.

### Stage 6 (run 2 finished the code run 1 wrote, gated it, fixed what the gates found)
- `police/siren.rs` (151): `sirens_on`, `SirenConfig`, `SirenLane`, `lane_frame`, `strip_at`,
  `choose_lane`, `corridor_obstacle`. **Changed from the plan:**
  - `lane_frame` takes the nearest lane pointing the car's way within 1.5 pitches (with the plain
    nearest lane, a car in the opposite lane lost its frame and swerved back).
  - Box margin is 0.
- `police/car_route.rs` (564, down from 659): IDM on the corridor strip while sirens are on. **Changed
  from the plan:** lanes are compared from the car's tail, over
  `max(sense_distance, v·lane_hold_seconds + v²/2b)`. `approach_clear`, `lane_costs_to`, `step_cost`
  and their unit test are removed.
- `police/spawn_sector.rs` (110): the sector quota as planned. `police/car_dispatch.rs`: sector picks,
  `sector_spawns` / `sector_fallbacks`, and the pursuit filter removed. **Changed:** occlusion rays go to
  the 4 top chassis corners (same 4-ray budget). G7 found corners in view under the old
  head/width/feet rays.
- `traffic/sirens.rs` (119): the yield. **Changed from the plan** (each change found by G5/G6/floor):
  - `yield_gap = v²/2b`, without `+ s0`: the gap ended at IDM's rest point and the car crept forever.
  - `drive.rs`: shift first at most at `pass.speed`, then stop.
  - No yield when the siren car has no way past (no free curb lane and no opposite lane). Without
    this, `police_car_pulls_away_behind_a_leader` went red on the one-way loop.
  - No yield within a car length + jam gap of the lane start (the siren car would stand in the box).
- `occupancy/mod.rs` (+10): standing time restarts when a body moved more than `hold_speed·dt` since
  the last tick (teleport, respawn). Found by t15 seed 1: the teleported player counted as a person
  standing for minutes, and the hijack target went around him at once.
- `police/mod.rs`: `sirens`, `spawn_sectors` config, `SirenLane` registered,
  `PoliceSystems.after(OccupancySystems).after(TrafficSystems::Drive)`. `cars.rs`:
  `#[require(SirenLane)]`.
- Data: `escalation.ron` `car.sirens: (lane_offsets: [-1.0], lane_hold_seconds: 1.0, lane_gain: 5.0)`
  and `car.spawn_sectors: (0.3, 0.3, 0.4, 45°, 135°)`. `traffic.ron` has
  `sirens: (yield_distance: 40.0, timeout_seconds: 8.0)`.

### Stage 7
- `docs/design/GDD.md` (+6 lines, Russian):
  - §5.2: go-around, recovery, siren yield, stuck cheat.
  - §5.3: any lane with sirens, spawn ahead/beside.
- `docs/architecture/traffic.md` (+139/-13): road occupancy (kinds, claims, queries, migrated vs
  TASK-036 consumers), recovery, the lateral law, tick order, go-around, junction box, sirens, stuck
  cheat, sector spawns and the hidden rule. It replaces the "cannot pass traffic" paragraph.
- `maw/tasks/pending/TASK-036/task.md`: walk avoidance (with the unconfirmed M1 civilian symptom), fire
  line, sight, conflict-point reservation. It includes the G1 junction-conflict finding (below).
  Blocked by TASK-032. `maw/ROADMAP.md` is not hand-edited (it is regenerated by the tasks skill).

### Tests (new or changed in run 2)
- New:
  - `tests/police_sirens.rs` (G5, 285)
  - `tests/police_close_in.rs` (G6, 197, ignored)
  - `tests/police_spawn_sectors.rs` (G7, 286)
- `police_car_floor.rs`: `police_car_never_spawns_behind_a_traffic_queue` is replaced by
  `police_car_gets_past_a_traffic_queue`. `spawn_police_car` moved to `police_support`.
- `police_cars.rs::car_chases_a_driver`: the chase target is the player shifted by the car's
  `SirenLane` offset (by design, sirens mean any lane).
- `traffic_occupancy.rs`: `standing_restarts_after_a_teleport`.
- `traffic_causes.rs`: the (d) rows above.
- `config_traffic.rs`: rows for `bubble.stuck_despawn_seconds`, `sirens.yield_distance`,
  `sirens.timeout_seconds`, `car.sirens.{lane_offsets, lane_hold_seconds, lane_gain}` and
  `car.spawn_sectors` (shares, degrees).
- `new_city.rs`: the dispatcher reset covers the sector fields.
- Moved without behaviour change: `cruise` to `vehicle_support`; `straightest_road` and `clear_ahead`
  in `traffic_support`.

## 2. What was not implemented, or deviates, and why

- **G6 not met** → `REDESIGN_NOTE.md`. Numbers (pressure ≤ 18 m within 25 s):

  | Road | Traffic | Result |
  |---|---|---|
  | street | production | 0/10 |
  | street | none (control) | 10/10 at ~6.8 s |
  | avenue | production | 2/10 |
  | avenue | none | 7/10 |

  Flip (no yield, no lanes): avenue 0/10. This number is the record for the "probe 6.0" the plan
  asked for. The probe was not run as a separate share-of-windows measurement; the G6 runs with
  no-traffic controls replaced it.

  Causes: on streets the 1.7 m gap between curb-yielded rows is smaller than the 2.4 m car. On
  avenues the police car's own route driving (turn speed in every box, no lane choice in a box) fails 3
  seeds even with no traffic. The fixture needs named mutations: camera looking back (the bubble drops
  traffic 25 m behind a forward-looking driver), followers going straight, the road ahead kept clear.
- **R1 RED** (runtime, in-view box lock) → `REDESIGN_NOTE.md` options R-A/R-B/R-C.
- **G1 finding outside this task's code**: in a G6 flip run (sirens off, street seed 5), two granted
  kinematic cars on connectors 126/127 interpenetrated by 0.34 m. Both were left turns from adjacent
  approaches. The conflict table, built from centre-line distance, misses a turning car's corners
  (TASK-016/033 rule). All production-config runs of this task are clean. Filed in TASK-036 item 4.
  Trace: `scratch/g6/g1_flip_trace.txt`.
- G5 scene (derived; the plan's 50 m / moving-queue rows did not fit the bubble):
  - One row per seed with the queue at rest, the head 10 m before the stop line, the police car 12 m
    behind the last car.
  - The Leave row puts it 8 m behind, because a leaving car follows the traffic bubble rule and must
    start within 90 m in frame.
  - The player stands past the head by dismount 20 + v²/2b 25 + car 4.08 = 49.1 m, on the exit lane's
    sidewalk.
- G7: the 3-star sector row is dropped. Under the despawn mutation the cap never binds, and it sampled
  the same 1920 spawns as 2 stars. The cap row covers 2 and 3 stars.
- Config rows for police keys are in `config_traffic.rs` next to the other `car.*` rows, not in
  `config_police.rs`.
- `stuck_despawn_seconds` lives in `bubble`, not in `pass` (it is a bubble despawn rule).
- File budgets:
  - `police/mod.rs` +18 (plan +≤ 8).
  - `graph.rs` +20 (plan +< 10; stage 2).
  - `drive.rs` 628, `car_route.rs` 564. All files are < 750 lines.
- Stage 4 step 6 (a pass through the box on the straight connector) was not built (run 1): such a pass
  is refused.

## 3. Test results

- `cargo test -p gta_sim` (`scratch/suite_final_gta_sim.txt`):
  - 551 passed, 0 failed, 3 ignored (G6 ×2, plus pre-existing `city.rs`), plus `witness_city` 1 passed.
    The last binary was cut by my 598 s timeout and rerun alone.
  - `cargo test -p citygen`: all ok.
- `cargo clippy --workspace --all-targets -- -D warnings`: clean.
- `cargo test -p gta_like --bin gta_like`: 82 passed.
- `python tools/qa/tree_check.py`: passed.
- `traffic_intersection` ran 3+ times green (the `plus()` floor has `left_gap`).
- Benches (`scratch/benches_stage7.txt`, 6 runs each, noisy host; Defender was scanning):
  - `traffic_bench` mean 1.78-2.50 ms (min 1.78; baseline 1.62).
  - `police_bench` 1.95-2.44 ms (min 1.95; baseline 1.62-1.65).
  - Both are far under `MEAN_LIMIT` 19 / 11 ms. The growth at the minimum is +0.16 / +0.33 ms, under
    the plan's 0.5 ms grid-fallback threshold, but the spread between consecutive runs (±0.3 ms) is
    as large as the growth.
- New gates and their flips (restore → green in every case):

  | Gate | Result | Flip → RED |
  |---|---|---|
  | G5 `police_sirens` (seeds 1, 7) | Every queued car yields 0.425 m and stops. The police car passes all 6; each car is back on its line and moving 1.0-1.55 s after the police rear passes (≤ 8 s). Worst `blocked` 1.47 s < 2 s (start from rest). Leave row: nobody yields. G1 clean. | `yield_distance 0`: nobody yields, `blocked` 2.73 s. |
  | Floor `police_car_gets_past_a_traffic_queue` | Both yield. Worst `blocked` 0.02 s. The crew gets out at x 35.3, past the queue head at x 5.6. | `lane_offsets []`: `blocked` 2.00 s, crew out at x -16.2, behind the queue. |
  | G7 `police_spawn_sectors` | 1920 pursuit spawns: ahead 475, beside 612, behind 833 (fixed floors 288/288/384); dispatcher fallbacks 1405. All hidden and non-overlapping. Caps: 2★ max 2, 3★ max 3. | `ahead: 0.0` → ahead 33 of 1920. The old width-only occlusion rays → 18 spawns with a corner in view. |
  | `standing_restarts_after_a_teleport` | green | Displacement check removed → RED. |

- Runtime (`scratch/runtime/`):

  | Run | Result |
  |---|---|
  | t15, final code (`t15_s{1,2,3}_{c,d}`) | Pressure 5/6: s1 escaped + 15.4 s; s2 4.6 / 4.8 s; s3 11.5 / 11.2 s. Hijack 6/6. At most 2 active cars. |
  | t15, before the standing fix (`_a`, `_b`) | s1 failed the hijack step twice; s2/s3 4/4 pressure. |
  | R1 leave (3 runs) | RED: 15 / 12 / 11 cars over 30 s |
  | R1 control (2 runs) | 0 over 20 s |
  | Civilians within 8 m of the left car | 23 / 18 / 9 |

## 4. How to verify manually

- Gates:
  - `cargo test -p gta_sim --test police_sirens --test police_spawn_sectors --test police_car_floor --test traffic_causes --test traffic_occupancy -- --nocapture`
    (numbers print).
  - G6 numbers: `cargo test -p gta_sim --test police_close_in -- --ignored --nocapture`.
- Runtime:
  - `python tools/qa/scenarios/t15.py --seed N --out <dir>` for N = 1..3.
  - `python maw/tasks/in_progress/TASK-032/scratch/tools/repro_abandoned_car.py <dir> 1|0`.
- Owner run (not gated; the look):
  - Recovery: the in-place rotation back onto the lane.
  - Yield: the shift to the curb, then the stop. On avenues, a full move into the curb lane.
  - Go-around: the at-rest sideways shift, then the glide past the obstacle.
  - A police car in the opposite lane with sirens.
  - Whether chases feel aggressive: t15 seeds 2/3 put a car within 11-16 m in 5-12 s.
  - A car left in a junction you are looking at: the R1 lock.

children: 0 launched / 0 reported.
