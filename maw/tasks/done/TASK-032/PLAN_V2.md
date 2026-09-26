# PLAN V2 — TASK-032: the street flows around the player's mess; sirens part traffic

Reviewer: plan-reviewer-1. Base: `PLAN.md` (planner) checked against `TASK_FINAL.md` and the code on
`feature/oncoming-lane` @ 7302ec5. Cost of error: silent (kinematic pass-through, minutes-long gridlock,
frame budget) → full evidence layer. Glide/snap, pull-over, pass and chase feel go to the owner run.

## 0. Disconfirmation (done before the review)

Counter-example tested: "an oncoming AI car never sees a pass claim, so a passer and an oncoming car meet
head-on in the oncoming lane". PLAN.md §2.1 says claims are written by `set_claim` "at the end of
`advance_traffic`" into `RoadOccupancy`, and that `snapshot_road` rebuilds the resource once per tick before
`TrafficSystems::Drive`. Sensing runs in the middle of `advance_traffic` (`drive.rs:313-381`). So in tick N the
claim appears only after every car has sensed. In tick N+1 the rebuild wipes it unless the snapshot rebuilds
claims from `TrafficCar` state, and the plan never says it does. Result: the spawner (`Bubble`) sees claims,
but no oncoming car's sensing ever does. **The counter-example holds.** Fix: R1 below.

## 1. Review notes (issues found, with evidence)

**R1. Claims are invisible to sensing (BLOCKER).** See §0. A claim has to be (a) derived statelessly in
`snapshot_road` from `TrafficCar.manoeuvre == Pass{..}` plus the car's pose, so every consumer sees last tick's
passes from the start of the tick, and (b) inserted into the resource at once when a pass is committed inside
the `advance_traffic` loop, so a car later in the same entity-ordered loop sees it. Without (b), two cars on
opposite lanes can commit to conflicting passes in the same tick.

**R2. Seated and disabled characters would enter the snapshot.** A seated driver keeps its `Character`
entity with `RigidBodyDisabled` + `ColliderDisabled` (`vehicle/seat.rs:280-281`). The avian casts it replaces
never see such bodies, and the switch skips disabled ones (`contact.rs:179`). The plan's query (`Has<Character>`
with no filter) would put a circle inside the player's car and inside police cars. The snapshot must skip
`ColliderDisabled` / `RigidBodyDisabled`, the same way avian does.

**R3. Curb-side feasibility read from "no World collider" breaks existing gates (BLOCKER for "unchanged-green").**
PLAN.md §2.5 step 5 and §2.7 E2 treat the curb side as free when `world_clear` finds no World collider. Test
floors have no sidewalk geometry. `traffic_pedestrian.rs::a_walker_ahead_of_the_bumper_still_holds_the_car`
(`:103-123`) holds a car behind a dummy for 640 ticks (10 s) and asserts `moved < 0.1`. With
`pass.character_seconds = 6` and a "free" curb side, the car drives off the one-way floor lane around the dummy,
and the gate goes RED. `a_walker_at_the_flank_does_not_hold_the_car` (`:33-99`) holds the car for up to
1920 + 1280 ticks, so the same thing happens. Only slot-0 lanes are in `TrafficGraph` (`graph.rs:287`), so the
graph has no idea whether a curb lane exists. Fix: a data-derived `TrafficLane.curb_lane: bool` (true only when
the layout has a same-direction slot-1 lane on the edge, i.e. an avenue). `TrafficGraph::new` sets it false, so
floors keep today's behaviour. `world_clear` stays as an extra check, not the source of truth.

**R4. The police lane choice misses `Chase`, and the heading cast undoes it.**
- `drive_police_cars` Chase targets the player directly (`car_route.rs:543`). E3 in PLAN.md changes only the
  Respond branch. In G6 a police car within `direct_chase_distance` 40 m that sees the driver chases straight
  down the lane into yielded cars. Sirens on = Respond | Chase (spec E1), so the lane choice has to cover both.
- The police IDM obstacle is an avian slab cast along the car's HEADING, 25 m long (`car_route.rs:572-597`).
  PLAN.md keeps it. At a 0.30 m clearance, a heading error above atan(0.3/25) ≈ 0.7° makes the cast hit a
  yielded car. A pure-pursuit autopilot changing lanes goes far past that, so the police brakes, stands
  `blocked_seconds` 2.0 (`escalation.ron`) and dismounts (`car_route.rs:355`, `cars.rs:165-174`). Fix: while
  sirens are on, the police IDM obstacle comes from `first_along` on the chosen corridor (lane tangent at the
  chosen offset). That is spec A4 "police lane choice reads the same rule".
- Arithmetic: offset −0.7 w puts the police centre at 1.625 − 2.275 = −0.65 m from the centre line, so its
  right edge is at 0.55 m. A street car yielded by its 0.425 m slack has its left edge at 1.625 + 0.425 − 1.2 =
  0.85 m. Clearance is **0.30 m**, not 0.35. The −0.7 w strip also covers −1.85..0.55 m, which overlaps any
  oncoming car (−2.825..−0.425 m), yielded or not. So −0.7 w never beats −1.0 w, and −1.0 w leaves 1.275 m to the
  yielded row. Use `lane_offsets: [-1.0]` (plus the implicit 0).
- Physical limit to record, not engineer away: a street is 6.5 m of asphalt. With both directions pulled to
  their curbs, the gap is 0.85 − (−0.85) = 1.7 m, less than a 2.4 m car. A police car passes a street queue
  only through a gap in oncoming traffic. G6's 8/10 depends on that, so stage 6 measures it first (§3, 6.0).

**R5. Offset junction entry (D4) is the riskiest geometry, and a simpler safe form exists.** SUMO lets vehicles
drive in the opposite direction only across *straight prioritized links* (sumo.dlr.de/docs/Simulation/
OppositeDirectionDriving.html, re-read for this review). PLAN.md cites that page but D4 allows any connector
and computes geometric conflicts on a shifted polyline (its own risk 3: a `right(s)` sign error). A left turn
from an offset of −3.25 m into the inside of the curve can cross the corner. Replacement: an offset entry is
allowed only onto the lane's straight connector (`dir_in · dir_out > 0.99`), and it conflicts with **every**
connector of the node (whole-box grant, no geometry). FCFS order still guarantees progress. If the lane has no
straight out connector, the pass is not committed (the car queues; the named fallback applies).

**R6. A car already on a connector when a body lands in its path is not covered.** Passes run only on lanes
(`left_gap` is a lane property), and D3 re-pick is only for queue heads before the line. A kinematic car on the
connector behind a body left in the box brakes (turn-sense strip) and stands forever. D2 only demotes its grant.
This is exactly the s1-tourist shape (car left about 7 m from the junction centre). Spec D and G4 ("no AI car on
any approach stands > 40 s") need a rule and a G4 row for it (D5 below).

**R7. Recovery can cause its own re-switch.** The recovery predicate checks only OTHER bodies' sweeps. A
recovered car then glides/rotates (Rejoin) toward its path, and its corners sweep up to
sqrt(1.2² + 2.04²) = 2.37 m while it turns in place. That can drive it into a sleeping parked car or the
player's car and re-trigger `switch_to_dynamic`. The hysteresis band does not cover self-motion. Fix: recovery
also needs `blocked(hull(current rect, on-path rect) grown by recover.skin) == None`. Also, give-up via
`Bailing` stays Dynamic and standing forever when `exit_spots` finds no door (`drive.rs:480-489`). Give-up goes
to `Bailing` only when an exit spot exists that tick; otherwise it calls `abandon()` directly (spec B: "becomes
Abandoned").

**R8. G1 has no baseline on current code.** Stage 1 builds the oracle but never runs it on untouched code. If
today's traffic already has kinematic overlaps (turning cars on non-conflicting connectors: the conflict rule is
centre-line distance < 2·half.x + 0.3 = 2.7 m, `graph.rs:233-241`, and it ignores corner sweep on curves), G1 is
RED before any change. Stage 1 runs the oracle on the four `traffic_gridlock` seeds and records the result.
Pre-existing violations are classified before any tolerance is set.

**R9. The spawner change shifts every seeded gate.** PLAN.md 2.7 extends the spawn check rect forward by
`min_gap + v0·T` (2 + 12·1.5 = 20 m on a street). Candidate acceptance feeds `rng.next_u32()` draws
(`spawn.rs:198-230`), so any change in acceptance moves the `TrafficRng` stream and every seeded gate (TASK-010
lesson). The spec asks only "never a spot another body occupies, a passing car included". Keep today's
chassis rect and today's body set (Character + Vehicle), and add claims. With no claim alive the result matches
today up to 2D-vs-3D shape differences.

**R10. G7's tolerance is a tautology.** "Count within ±1 of share·N minus reported fallbacks" moves with the
defect: if every ahead pick falls back, 0 ≈ 0.3N − 0.3N passes. Use a fixed per-sector floor taken from a
measured run (ahead ≥ ⌈k⌉ of N), report the fallbacks, and flip with `ahead: 0.0` → RED.

**R11. Schedule nondeterminism.** `TrafficSystems::Drive` and `PoliceSystems` are both only `.before
(VehicleSystems::Drive)` (`traffic/mod.rs:230-233`, `vehicle/mod.rs:239-243`) and are unordered with each other.
Once Drive writes claims and police reads them, the executor's pick decides what police sees. Add
`PoliceSystems.after(TrafficSystems::Drive)`. No cycle: Drive depends on Hijack/Bail only, and Police on
Population/Wanted.

**R12. Missing data values.** `LateralConfig { rate_at_rest, slope, yaw_rate_deg }` is declared, but no
`traffic.ron` values are given, and `pass.stuck_despawn_seconds` (fallback) has none. Every value needs a
number with a "why" comment in the stage that adds it, derived from a worked example: lateral 3.25 m completes
within the pass `end_s` stretch at `pass.speed`.

**R13. Yield trigger uses velocity.** "velocity·tangent > 0" drops the yield as soon as the siren car stands
behind the queue. That is when the police needs it most: blocked 2 s → dismount. Use the siren car's heading
(`forward · tangent > cos 45°`) and ignore its speed.

**R14. The G5 fixture can end before the police passes.** The Respond route holds at `dismount_distance` 20 m
of an on-foot player (`car_route.rs:503-504`). 6 cars × (4.08 + 2.0) ≈ 36.5 m of queue. The player has to stand
more than queue length + 20 m + the police stopping distance past the queue head. Derive the position in the
stage summary.

**R15. Occupancy lifecycle.** The `standing` map lives in the resource. Clear it on `NEW_CITY` (as
`reset_traffic` does). `snapshot_road` needs `Option<Res<TrafficGraph>>` to derive claims (R1). Gate the set
like the traffic sets (`run_if(resource_exists::<TrafficGraph>)`).

Verified correct in PLAN.md: the drive.rs facts (cast from the actual position along the path tangent,
`!kinematic.contains(&e)` at `:371`, no Dynamic→Kinematic path, `lost` at `:160`); the junction lease rule
(`junction.rs:105-117`); switch geometry; `SolverConfig.max_overlap_solve_speed` 4.0 (avian3d-0.7.0
`dynamics/solver/plugin.rs`, default at the Default impl); Bevy 0.19.1 / avian 0.7.0 from `Cargo.lock`;
`step_cost` and `lane_costs_to` are used only by `approach_clear`, so removing them is safe; every test helper
PLAN.md names exists (`chase_view`, `loop_lanes`, `no_police_cars`, `spawn_police_car`, `composed_app`,
`spawn_dummy`, `traffic_floor`, `spawn_car`, `raise_heat`). The SUMO bluelight page confirms a 25 m default
reaction distance and "resume previous lateral alignment after the pass".

## 2. Updated understanding (corrections only; the rest of PLAN.md §1 stands)

- Seated characters: `Driving` + `RigidBodyDisabled` + `ColliderDisabled`. Avian queries skip them.
- Police: Respond follows the A* lane route with `pull_over` only below `turn_speed`. Chase drives straight at the
  player (`at`) with the player's car ignored in its cast. Both use a HEADING slab cast for IDM. A Respond car at
  or below `exit_max_speed` for `blocked_seconds` 2.0 s dismounts.
- Dispatch candidates are already limited to lanes heading toward `last_known` (`car_dispatch.rs:147-149`), so
  an "ahead" spawn is an oncoming-direction lane point facing the player. `pick_spawn` draws no RNG.
- Schedule: `TrafficSystems::Drive` ∥ `PoliceSystems`, both before `VehicleSystems::Drive`. Physics runs in
  `FixedPostUpdate`, so `Position` is constant through `FixedUpdate` and one snapshot per tick is exact.
- Street geometry: 6.5 m asphalt, inner lane centres at ±1.625 m, curb at ±3.25 m, raised sidewalk beyond.
  Avenue: inner ±1.625 m (graph), curb lanes ±4.875 m (parked cars, not in graph), no median.

## 3. Revised approach (deltas to PLAN.md §2; unchanged parts apply as written)

2.1 Occupancy: as PLAN.md, plus R1 (claims derived in the snapshot from `Manoeuvre::Pass`, and an immediate
`insert_claim` on commit inside the loop), R2 (skip disabled bodies), R15. `RoadBody.kind` as PLAN.md.
`set_claim` at the end of the loop is dropped.

2.2 Lateral state: as PLAN.md. Values in `traffic.ron` `lateral:` derived in stage 2 (R12).

2.3 Traffic sensing: as PLAN.md (two strips, on-path skip only for non-manoeuvring cars).

2.4 Recovery: as PLAN.md plus the R7 corridor check and the R7 give-up rule.

2.5 Go-around: as PLAN.md with these changes:
- step 5: the curb side is allowed only when `lane.curb_lane` (data, R3) AND `world_clear` AND `blocked == None`;
- step 6: when `end_s > lane.stop − half_length`, the pass commits only if the lane has a straight out connector.
  `next` is set to it (no RNG draw), and the lane-end request is a whole-box request (R5);
- the stuck-despawn fallback stays named, not built.

2.6 Junction:
- D1, D2, D3 as PLAN.md.
- **D4 (replaced)**: an offset entry requests the straight connector with the conflict set = every connector of
  the node. Its pose on the connector decays laterally `l0·(1 − s/len)`, and `world_clear` over the offset path
  rects is checked at commit (in 2.5).
- **D5 (new, R6)**: a car on a connector whose strip-T hit is a standing non-AI body (≥ `pass.vehicle_seconds`)
  tries a connector pass: offset ±`w` decaying to 0 at the connector end, claim = offset path rects, requiring
  `blocked == None`, `world_clear`, and that no other grant is live at the node (it re-requests as a whole-box
  waiter, like D4). If neither side is clear it holds, and its stand shows in the G4 row. A second failure of this
  class triggers the named fallback (off-frame stuck despawn) through `REDESIGN_NOTE.md`.

2.7 Sirens:
- E1 as PLAN.md.
- E2 yield: trigger by siren heading (R13). Avenue: the curb lane when `curb_lane && free`. Street: the in-lane
  slack.
- E3 any lane: applies in Respond **and** Chase (R4). Candidates `[0] ∪ lane_offsets` with shipped
  `lane_offsets: [-1.0]`. While sirens are on, the police IDM obstacle = `first_along` on the chosen corridor
  strip (lane tangent at the offset, from the nose, `traffic.sense_distance`), skipping the player's car in
  Chase as today. The heading cast stays for sirens-off states.
- E4 spawn sectors as PLAN.md; `approach_clear`, `lane_costs_to`, `step_cost`, `approach_clear_rows` removed.

2.8 Graph: `TrafficLane.left_gap: Option<f32>` (PLAN.md) and `curb_lane: bool` (R3), both set in
`traffic/lanes.rs`. `left_gap` is computed in `TrafficGraph::new`; `curb_lane` in `from_layout` (a same-direction
slot-1 lane exists on the edge). Floors: false.

Schedule: `OccupancySystems` in `NpcSystems`, after `TrafficSystems::{Hijack, Bail}`, before
`TrafficSystems::{Drive, Bubble}` and `PoliceSystems`; plus `PoliceSystems.after(TrafficSystems::Drive)` (R11).
Messages/observers: none new. All cross-system data goes through the `RoadOccupancy` resource and components.

## 4. Revised steps (complete)

Every stage ends green: `cargo test -p gta_sim -p citygen`, `cargo clippy --workspace --all-targets -- -D
warnings`, `cargo test -p gta_like --bin gta_like`, `python tools/qa/tree_check.py`. Stop rule: the second
failure of one class (same gate, same mechanism) → `REDESIGN_NOTE.md` (fantasy, missing shared rule, cheat,
rescope), no third patch. Before trusting RED from a shared `target/`, `touch crates/*/src/lib.rs` (gates lesson).

### Stage 1 — causal evidence, baselines, tooling (no production code)
1.1 `tests/traffic_support/mod.rs`: the G1 oracle `Footprints`, an independent flat SAT with its own projection
   code. It covers every `Vehicle` `Position`/`Rotation` within 150 m of the player, over pairs with at least one
   `RigidBody::Kinematic`. Tolerances: kinematic-kinematic 0.02 m; kinematic-dynamic `max_overlap_solve_speed ·
   dt` read from the `SolverConfig` resource (0.0625 m at 64 Hz). `record` collects, `assert_clean(label)` panics
   with all violations. Also `StandClock`: per-entity stands below `hold_speed`, tagged by mode, with Dynamic
   stands kept separately.
1.2 **Baseline G1 (R8)**: a probe (ignored test or `scratch/` harness) runs the oracle on current code, gridlock
   pose, seeds 1/2/7/42, 120 s. Record the violation count and class. If any exist, report to the orchestrator
   before stage 2 with the pairs (connector ids, overlap depth). The tolerance is never widened to hide them.
1.3 `tests/traffic_causes.rs` (production city, stationary player on the sidewalk off the carriageway facing the
   scene, all violations collected). Rows as PLAN.md 1.2: (a) abandoned car mid-lane, street lane ≥ 60 m before
   its stop line, seeds 1 and 7; (b1)-(b4) bumped car left Dynamic; (c) character standing in a street lane;
   plus **(d) a non-AI car left on a connector path with an AI car already on that connector behind it** (R6).
   Each RED row gets `#[ignore = "TASK-032 stage N"]` with its RED numbers in the stage summary. A cause that does
   not reproduce in any variant is dropped from scope and reported (premise amendment 1).
1.4 `tools/qa/scenarios/t15.py --seed` (default 1), seed in `summary.json`.
1.5 R1 script at `maw/tasks/in_progress/TASK-032/scratch/tools/repro_abandoned_car.py` as PLAN.md 1.4.
   Baselines: R1 leave=1/leave=0 seed 1; t15 seeds 1/2/3 one run each; the `traffic_bench` and `police_bench`
   means. Outputs under `scratch/baseline/`.
Check: ignored rows RED under `--ignored`; normal suite green; baselines recorded.

### Stage 2 — occupancy, lateral state, sensing, lane start, spawner (G1, G8)
2.1 `occupancy/mod.rs`, `occupancy/query.rs` as PLAN.md 2.1, with: `snapshot_road` skips entities with
   `ColliderDisabled` or `RigidBodyDisabled` (R2); claims derived from `TrafficCar.manoeuvre` using
   `Option<Res<TrafficGraph>>` (R1); `insert_claim(entity, rect)` API for same-tick commits; the `standing` map is
   cleared on `NEW_CITY` (R15). `lib.rs`: `pub mod occupancy;`, `OccupancyPlugin` in the
   `(VehiclePlugin, TrafficPlugin)` tuple.
2.2 `traffic/mod.rs`: `TrafficCar.{lateral, manoeuvre, calm}` + `Manoeuvre` (Reflect, registered); the
   `spawn.rs:63` literal; `abandon()` resets them. Sets: Drive/Bubble after `OccupancySystems`. `police/mod.rs`:
   `PoliceSystems.after(OccupancySystems).after(TrafficSystems::Drive)` (R11).
2.3 `traffic/lateral.rs` (PLAN.md 2.3), with `LateralConfig` added to `config.rs` and `traffic.ron` in this
   stage. Values: `rate_at_rest`, `slope`, `yaw_rate_deg`, each with a "why" comment and a worked example in the
   summary (a 3.25 m shift at `pass.speed` completes within the `end_s` stretch). Config rows in
   `tests/config_traffic.rs`.
2.4 `traffic/lanes.rs`: `left_gap` (in `TrafficGraph::new`) and `curb_lane` (in `from_layout`; floors false)
   (R3). `graph.rs` grows by less than 10 lines. Unit rows: a street edge (left_gap 3.25, curb false), an avenue
   edge (curb true), a one-way floor (None, false).
2.5 `drive.rs` sensing → two `first_along` strips (PLAN.md 2.5); `TrafficStats.casts` still counts.
2.6 `lane_start_free` → `occupancy.blocked` (Vehicle, Traffic, claims; no characters, no on-path traffic).
2.7 `spawn.rs` chassis check → `occupancy.blocked` over **today's chassis rect** with today's kinds (Character,
   Vehicle and all traffic) **plus claims**. No forward extension (R9). The `on_lane` spacing stays.
2.8 Gates: `traffic_gridlock.rs` runs the G1 oracle every tick on its four seeds. `tests/traffic_occupancy.rs`
   (G8) rows as PLAN.md 2.8. The spawner row sets `Manoeuvre::Pass` on a car so the derived claim covers a spawn
   point, which also exercises R1. Each row is flipped by removing that consumer's occupancy input, recorded.
   The yield-based sensing row needs its dummy siren entity kept alive and "not passed" by the resume rule, or it
   uses a `Pass` manoeuvre instead; the fixture states which.
Check: existing `traffic_*`, `police_*`, `vehicle_*` green; gridlock seeds unchanged within the printed worst
stop (seed drift is reported if any: it can only come from 2D-vs-3D shape differences, R9); `traffic_bench` mean
compared with the stage-1 baseline.

### Stage 3 — bumped-car recovery (B, G3)
3.1 `traffic/recover.rs`: `recover_dynamic` → `Stay | Recover { lateral } | GiveUp`, applied in the write-back
   loop. The predicate is PLAN.md 2.4 **plus** the rejoin corridor clear (R7). `GiveUp` → `Bailing` only if
   `exit_spots` returns a spot this tick, else `abandon()` (R7).
3.2 `RecoverConfig { seconds, skin, horizon_seconds, max_tilt_deg, give_up_seconds }` with validation, including
   the laws `recover.skin > switch.skin` and `recover.horizon_seconds >= switch.horizon_seconds`.
3.3 `traffic.ron` `recover: (seconds: 1.5, skin: 0.4, horizon_seconds: 0.5, max_tilt_deg: 10.0,
   give_up_seconds: 10.0)` with a "why" comment.
3.4 `tests/traffic_recovery.rs` (G3) rows (a), (c), (d) as PLAN.md 3.4, plus **(e) a car shoved 1.5 m
   sideways next to a sleeping parked car on an avenue floor: it recovers without a re-switch**. This is the R7
   flip: remove the corridor check → RED by a counted re-switch. (b) comes in stage 4. Flips: `recover.seconds`
   1e6 → (a) RED; recovery on the switch's own skin/horizon → (c) RED.
3.5 Un-ignore the stage-1 (b) rows that reproduced. Config rows for every new key.

### Stage 4 — go-around of standing vehicles and characters (C, G2)
4.1 `traffic/pass.rs` as PLAN.md 4.1, with: curb side only for `curb_lane` (R3); commit inserts the claim at
   once (R1); step 6 as §3 2.5 (a straight connector or no commit, R5).
4.2 `pass: (vehicle_seconds: 3.0, character_seconds: 6.0, trigger_gap: 6.0, clearance: 0.5, speed: 6.0)`,
   validation as PLAN.md 4.2.
4.3 A worked example in the stage summary. Oncoming car standing at the claim end; passer rear at `merge_s`:
   gap = `w/slope + s0`; show that strip C at the current lateral does not hold the passer at that gap with the
   stage-2 lateral values. Also the three directions for the offset pose sign (north lane, east lane, the straight
   connector).
4.4 `tests/traffic_go_around.rs` (G2) as PLAN.md 4.3 (street and avenue rows × seeds 1/2/7/42, character rows
   seeds 1/7, collect all, G1 oracle). Flips as PLAN.md.
4.5 Regression check named explicitly: both `traffic_pedestrian.rs` hold gates stay green, because their floor
   has no antiparallel lane and no curb lane. Record that removing the `curb_lane` guard turns
   `a_walker_ahead_of_the_bumper_still_holds_the_car` RED (it doubles as the R3 flip).
4.6 G3 (b) row; un-ignore stage-1 (a) and (c).
4.7 Fallback trigger as PLAN.md 4.5.

### Stage 5 — junction box (D, G4)
5.1 `traffic/box_rules.rs`: `connector_clear`, `straight_out(lane)`, whole-box request helper, `repick`,
   connector pass (D5). `graph.rs` `polyline_distance` stays private unless `connector_clear` needs it.
5.2 `junction.rs`: D1 (grants add `connector_clear`), D2 (demote stale holders on connectors or past the line
   into waiters in place), D3 (re-pick, no RNG), D4-replaced (a whole-box waiter blocks and is blocked by every
   connector of the node), D5. `junction::update` gains `&RoadOccupancy`.
5.3 `drive.rs`: a car on a connector without its grant holds; lateral decays on the connector.
5.4 `tests/traffic_junction_box.rs` (G4) as PLAN.md 5.4, plus **a row with an AI car already on the blocked
   connector** (R6): its stand is bounded by 40 s. Flips: D1 removed → RED; D2 removed → stale-grant RED; D5
   removed → the new row RED. G8 junction row in `traffic_occupancy.rs`.
5.5 `traffic_intersection.rs` stays green (conflict-table regression). The stage-1 (d) row is un-ignored.

### Stage 6 — sirens, any lane, spawn sectors (E, G5-G7)
6.0 **Feasibility probe first (R4)**: on seeds 1..10 in the G6 pose, measure the share of 25 s windows in which
   the oncoming street lane next to the queue has a gap of at least the queue length plus 2·`sense_distance`.
   If it is below 8/10, report to the orchestrator before building G6. Options: take G6 on an avenue lane, or
   add an oncoming yield that stops oncoming cars before the police corridor. Do not tune around it silently.
6.1 `police/siren.rs`: `sirens_on`, `SirenConfig { lane_offsets, lane_hold_seconds, lane_gain }`,
   `choose_lane`, `corridor_obstacle` (the `first_along` IDM input, R4). Call sites in both the Respond and the
   Chase arms of `drive_police_cars` (a few lines each; `car_route.rs` must stay < 750; it shrinks by the removed
   `approach_clear` / `lane_costs_to` / `step_cost` / test). The `SirenLane { offset, held }` component is required
   by `PoliceCar`.
6.2 `traffic/sirens.rs` (E2), heading-based trigger (R13); `traffic.ron` `sirens: (yield_distance: 40.0,
   timeout_seconds: 8.0)` (`yield_distance` 0 = never).
6.3 `police/spawn_sector.rs` as PLAN.md 6.3; `escalation.ron` `car.spawn_sectors` as PLAN.md and
   `car.sirens: (lane_offsets: [-1.0], lane_hold_seconds: 1.0, lane_gain: 5.0)`. The comment gives the R4
   arithmetic (−1.0 w leaves 1.275 m to a yielded street row; −0.7 w leaves 0.30 m and still overlaps the
   oncoming lane).
6.4 Gates:
   - `tests/police_sirens.rs` (G5) as PLAN.md, with the player position derived per R14: beyond the queue head by
     at least `dismount_distance` + the police stopping distance at `pursuit_speed` + one car length, value in
     the summary. The police car's `blocked` timer never reaches `blocked_seconds` during the pass (asserted, so a
     dismount cannot fake a pass). Rows: yield, pass, resume, `Leave` (nobody yields).
   - `tests/police_close_in.rs` (G6) as PLAN.md, with a Chase row: the police car within
     `direct_chase_distance` sees the driver from the start. Flip: `yield_distance = 0` and `lane_offsets = []`
     → RED (count recorded).
   - `tests/police_spawn_sectors.rs` (G7): one row per sector with a fixed floor from a measured run (R10), every
     spawn hidden and overlapping no body, `dispatcher.cars <= row.cars` every tick. Flip: `ahead: 0.0` → the ahead
     row RED.
6.5 `tests/config_police.rs` rows for `sirens` and `spawn_sectors`. Replace
   `police_car_floor.rs::police_car_never_spawns_behind_a_traffic_queue` by a floor row: a police car spawned
   behind a queue closes in.

### Stage 7 — docs, data, runtime QA, follow-up task
7.1 GDD §5.2/§5.3 one-line amendments in Russian (go-around incl. characters after a longer wait, recovery,
   siren yield, any lane with sirens, spawn ahead/beside by sector shares).
7.2 `docs/architecture/traffic.md`: occupancy (snapshot, kinds, strip/overlap API, claims derived from
   manoeuvre state), modes incl. the switch and its return, the lateral law, tick order (incl.
   `PoliceSystems.after(Drive)`), pass (straight-connector rule), junction D1-D5, sirens; migrated consumers vs
   TASK-036 pending ones. Remove the "cannot pass traffic" paragraph.
7.3 TASK-036 drafted for `maw-tasks` (blocked by TASK-032).
7.4 Runtime QA: R1 (leave=1: no traffic car within 45 m stands > 30 s; control: 0 over 20 s; civilians within
   8 m reported); t15 seeds 1/2/3 × 2 (pressure ≥ 5/6; hijack and "≤ 2 active police cars at 2★" green).
   Owner-run notes: recovery look (incl. the in-place rotation of a recovered car), pull-over, pass glide, chase.
7.5 G9: full suite, both benches under `MEAN_LIMIT`, clippy, client tests, tree_check, 5 CI workflows on the
   merge commit.

## 5. Risk areas

1. **Head-on in the oncoming lane (TASK-016 class).** Mitigated by commit-only-when-clear, claims visible from
   tick start (R1) and same-tick (R1b), the comfortable-stop extension, and the 4.3 worked example. A body that
   enters a claim after commit (player, police, walker) can still hold a passer mid-lane. Fallback named.
2. **G6 feasibility on streets (R4).** A police car cannot fit between two curb-pulled rows (1.7 m < 2.4 m).
   It passes only through gaps in oncoming traffic. Stage 6.0 measures this before G6 is built.
3. **Police lane change vs dismount.** `blocked_seconds` 2.0 turns any stall into a dismount (sirens off →
   yielders resume). The corridor-strip IDM (R4) and the G5 "blocked never reaches the limit" assertion carry
   it.
4. **Junction flow after D2/D4/D5.** Whole-box requests are conservative and can lengthen waits at busy nodes.
   `traffic_gridlock` 40 s on four seeds plus the stale-grant metric are the net. D5 is the likeliest to hit the
   stop rule.
5. **Recovery flip-flop.** Covered by the hysteresis laws in data (other bodies) and the corridor check (own
   motion, R7). G3 (c)/(e) count re-switches.
6. **Seed drift.** The spawner acceptance is kept identical to today (R9), and re-pick, sectors, pass and yield
   draw no RNG. Any remaining drift is reported with the gridlock numbers, not absorbed into thresholds.
7. **G1 born RED (R8).** Pre-existing overlaps are found in stage 1 and routed to the orchestrator, never hidden
   by tolerance.
8. **Frame budget.** Brute force over about 100 bodies × about 24 cars × 2-3 strips, plus the police corridor
   and D1/D5 path rects. Compare `traffic_bench` / `police_bench` with the stage-1 baseline at each stage; a
   uniform grid is the named fallback (> 0.5 ms mean growth).
9. **Wrecks across the road.** An abandoned car turned more than about 50° on a street needs more than
   3.675 m of offset (the feasibility law), so it cannot be passed. The in-view cars behind it queue. This is
   physical and GTA-like; it is reported in R1/G2 summaries, not engineered around.
10. **New `Res<RoadOccupancy>` params** on `advance_traffic`, `spawn_traffic`, `drive_police_cars`,
    `dispatch_police_cars` (if used): every harness goes through `composed_app`; grep tests for hand-registered
    systems before merging.
11. **File limits.** New code goes to `occupancy/*`, `traffic/{lateral,lanes,recover,pass,box_rules,sirens}.rs`
    and `police/{siren,spawn_sector}.rs`. `drive.rs` (514 now) must stay < 750; `car_route.rs` shrinks;
    `police/mod.rs` +3 lines; `graph.rs` < 10 lines.
12. **Bevy/avian APIs.** No new engine APIs beyond those already in these files (`SpatialQuery::
    shape_intersections`, `try_insert`/`try_remove`, `RigidBody::Kinematic`, `SleepingDisabled`,
    `ColliderDisabled`/`RigidBodyDisabled` as `Has<>` filters, `MessageReader<CollisionStart>`, `SolverConfig`).
    Each is confirmed against avian3d-0.7.0 / bevy 0.19.1 sources at use.

## 6. Open questions
None blocking. Decisions made here (logged): claims derived from manoeuvre state; curb lane from data; police
lane choice in Chase too, with corridor-strip IDM and offset −1.0 w; the straight-connector whole-box entry
instead of geometric offset conflicts; D5 connector pass; spawner check kept identical plus claims. If stage 6.0
shows the street G6 infeasible, the orchestrator picks between an avenue G6 and an oncoming-yield extension.
