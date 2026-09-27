# PLAN_FINAL — TASK-036 (rescoped): walker pile-up at a car, `connector_rects` overhang, residual lateral at connector entry

Reviewer: plan-reviewer-2. Base: PLAN.md (`d6a5351`) and PLAN_V2.md (`2b63bdb`); no production code changed since
`7e15814`. Binding scope: TASK_FINAL "Orchestrator rescope" items A, B, C, and Resolved questions Q1-Q4
(Q1: fix the siren-yield lock on a connector here; Q2: lane-end guard only if the Stage 0.2 sweep finds a G1;
Q3: the curb-clip observation goes to the `docs/narrative-graph.md` backlog; Q4: traffic.md records that the
"counter-flow" hypothesis is refuted). Fire line, sight and conflict-point reservation stay out.

Cost of error per item:
- A: the owner sees it in the first minute near a left car. Mechanism rule plus one honest city row.
- B: silent. Kinematic cars drive through each other (G1), or a holder and a waiter wait on each other. Full
  evidence layer.
- C: a silent freeze in view. Full evidence layer for what reproduced.

## 1. Summary

Three causal-first fixes, each with a flip-RED gate. **A:** a sidewalk waypoint whose lane target lies under a
standing car is unreachable (`around_cars` never returns a target inside a car), so walkers orbit the car's
corners forever. `arrive` in `civilian/mod.rs` counts such a waypoint as reached once the walker is within
`corner + arrive_radius` of the covering car, and the walker takes its next edge. **B:** the junction grant/box
check (`box_rules::connector_clear`) replaces the capless centre-line band with the requester's **real** body
(chassis half extents, no margin) swept along its connector **from where the car stands now** (`from_s`), sampled
by the corridor law. This closes the reproduced rear-swing G1 (0.53-0.68 m) and the holder/waiter mutual wait,
and neither demotes waiting holders over their own lane follower (the V2 rear-cap finding) nor treats other lanes'
queue heads standing at their stop lines as bodies on the path (this review's finding against V2's grown body).
**C:** `sirens::update` ends a `Yield` when the car is on a connector (`Manoeuvre::None`), so a car that stopped
for its yield inside the box drives out instead of standing there with its grant; a C-G1 (co-granted contact
from a residual offset) is built only if the Stage 0.2 timing sweep finds one. Docs and full Windows + WSL
verification close the task.

## 2. Evidence already on disk (read-only probes, all under `scratch/`)

Probe crate: `scratch/probe/ws/probe` (package `probe036`, its own `scratch/probe/assets`). Build and run with
`CARGO_TARGET_DIR=D:/test-gta-like/target cargo test --offline --test <name> -- --nocapture` (debug profile; do not
use `--release`, it rebuilds all of bevy). Run from `D:/test-gta-like/maw/tasks/in_progress/TASK-036/scratch/probe/ws/probe`.

| Probe | Output | Result |
|---|---|---|
| `tests/walkers.rs` (A trace, seed 1, TASK-037 spot B, 150 s, plus a run without the car) | `scratch/walkers.txt`, `scratch/walkers_dump.txt` | Reproduced: 13 walkers stall up to 94 s (car heading 0), 11 up to 118 s (heading 90). Control without the car: 0 stalls. **Not counter-flow:** 0 of 6361 stalled samples have a counter-flowing nearest walker; 72 % / 58 % have a same-direction neighbour; 100 % steer to a car corner. The parked car (centre (5.30, -84.50)) covers sidewalk node 312 at (5.80, -84.43); its lane target (5.81, -84.93) lies inside the car. Every stalled walker is on an edge into 312: (311→312), (314→312), (259→312), (307→312). Stalled walker 2016 at (6.79, -81.52) is 0.94 m from the car's end. |
| `tests/geometry_b.rs` (B static, seeds 1..8, + floor) | `scratch/geometry_b.txt` | ~2200 (c1, c2) pairs per seed with a body X on c1 outside c2's band but inside c2's unmargined swept body; the exit-lane room check blocks none; every contact has Y's centre on its connector; worst depth 0.71 m. Dominant row: X on a near-straight connector (~6.2 m) about 1.45 m in, Y on an adjacent right turn (2.0-2.5 m), same exit lane. |
| `tests/b_fixture.rs` (B in the App) | `scratch/b_fixture.txt` | **Reproduced.** + floor, Y at 6 m/s: G1 0.53 m (c1 5 s 1.50, c2 2). Seed 1 node 69 (c1 590 s 1.47, c2 587) from rest: G1 0.68 m. Seed 1 node 92 (c1 804, c2 801): G1 0.61 m. + floor from rest: no G1, but Y stops at s 0.47 on c2 holding its grant while X waits for c1 (conflicts with Y's grant): mutual wait for the remaining >= 17.75 s. Y never braked in the city rows: the contact is Y's rear swing, which sensing skips by design (`manoeuvre.rs:51-54`, `:76-85`). |
| `tests/geometry.rs` `c_entry_offset`, `c_threshold`, `c_block_clip` (C static) | `scratch/geometry.txt`, `scratch/geometry_c.txt`, `scratch/geometry_clip.txt` | Co-granted pairs touch only when car A enters with a right offset >= 2.29-2.46 m (Yield decay law) or >= 2.74-2.89 m (stepped Rejoin), seeds 1..8. Street slack yields (0.425 m) and 1.0 m touch nothing; only the avenue curb yield (3.25 m) exceeds the threshold (64-102 ordered pairs per seed). Side finding (Q3): curb-yield entries >= 2 m clip a block's curb prism on 16-24 connectors per seed (2 m) and 138-149 (3.0-3.25 m), up to 0.51 m. |
| `tests/c_fixture.rs` (C in the App) | `scratch/c_fixture.txt`, `scratch/c_fixture2.txt` | **C-G1 not reproduced** in 5 timings of seed 1 curb pair 1091/1089. **Reproduced instead, a lock:** a granted car whose yield reaches its full offset at or past the lane end enters the connector and the yield stop (`drive.rs:397-399`) halts it at s 0.33-0.75 **with its grant**, for the rest of the run (+ floor slack yield begun 1 m before the stop line: `Connector(1)` s 0.75; seed 1 lane 437 curb yield begun 10 m before: `Connector(1091)` s 0.35). |
| `tests/rv1_rear_cap.rs` (plan-reviewer-1) | `scratch/rv1/rear_cap.txt` | 238-240 of ~580 lanes per seed (1/2/7/42) have the stop line within 2.18 m of the lane end; 217-227 have `stop == length`; every + floor lane has `stop == length`. A sweep started at connector s 0 reaches the holder's own lane follower. |
| `tests/rv2_neighbour.rs` `neighbour_queue_heads` (this review) | `scratch/rv2/neighbour.txt` | The B sweep from s 0 against (Q) the queue head of every OTHER lane ending at the node, nose at its stop line, and (E) a car with its rear at s 0 on another exit lane of the node. **Grown body (V2, +0.15):** Q hits on 4 of 16 + floor connectors and 5-8 turning connectors per city seed (worst 0.12 m), old band 0. **Grow 0.05 and 0 (real body): Q hits 0** everywhere. E (a real spillback contact): 29-42 per seed with the real body, 6-11 with the old band, worst real depth 0.33-0.42 m. |
| `tests/rv2_neighbour.rs` `b_rows_static`, `b5_plus_left_turn_vs_opposite_head` | `scratch/rv2/b_rows_static.txt`, `scratch/rv2/b5_plus.txt` | Real-body sweep catches B1 (0.54 m) and B3 (0.69 m); Y at its stop line is outside X's c5 sweep from s 1.5 (-0.42 m). Rear-cap flip geometry on the + floor straight c1: follower F behind a holder at s 1.0 is NOT in the from-0 sweep (nose 3.04 m back); at s >= 2.5 it is. Grown body: every + floor turn meets some queue head (c0/c4/c8/c9 with corridor sampling, c2/c3/c7/c11 with a 0.01 m sweep), e.g. c0 (lane 0 → 5) against lane 2's head; real body: none. |

## 3. Understanding (verified against code)

### A: walkers around a standing car
- `civilian_fsm` (`civilian/mod.rs:479-550`): `next_state`, then `arrive` (`:427-455`), then
  `around_cars(position, lane_target(&graph, *walker, nav.keep_right), &standing_cars(road.bodies(), position.0,
  nav.avoid_distance), loco.capsule_radius, loco.capsule_radius + nav.arrive_radius)` (`:538-544`).
- `arrive` computes `lane_target` itself and takes the next edge only when `flat_distance(position, target) <=
  nav.arrive_radius` (0.5 m, `assets/npc/navigation.ron`). When it fires it replaces `*walker`
  (`civilian/mod.rs:445-448`) and draws `NpcRng` (`wander_next` and the idle roll, `:441-452`), so `around_cars`
  must read the NEW edge's target after `arrive` (it already does).
- `around_cars` (`tactics/fire_line.rs:141-199`) returns a car corner `corner` = 0.8 m out while the straight line
  to `to` enters a car grown by `clearance` (0.3 m). A target inside a car grown by 0.3 m blocks every line, so
  `around_cars` never returns it, `arrive` never fires, and the walker orbits the four corners; walkers from all
  edges into that node share those corners and press against each other there.
- `standing_cars` (`:457-476`) keeps rect bodies with `standing > 0` whose centre is within `avoid_distance` (2.0)
  + half diagonal. The list comes from `RoadOccupancy`, which is filled by `snapshot_road` only when a
  `TrafficGraph` exists (`occupancy/mod.rs:207`) and only for bodies within `bubble.in_view.despawn + look_ahead`
  (150 m) of the `Player` (`:134-145`).
- `car_blocks(p, p, car, r)` (`fire_line.rs:118-122`): the degenerate segment makes `car_entry` return `Some` iff
  `p` lies inside the car rect grown by `r` on both axes (`:124-140`). `car_blocks` is re-exported
  `pub(crate)` from `tactics/mod.rs:7-9` but is **not** imported in `civilian/mod.rs` (`:20` imports `CarRect,
  around_cars`).
- Values: `capsule_radius` 0.3 (`assets/character/locomotion.ron`), `walk_speed` 1.8, `arrive_radius` 0.5,
  `keep_right` 0.5, `idle_chance` 0.15 (`assets/npc/civilian.ron`).

### B: `connector_rects` overhang
- `box_rules.rs:13-28` `connector_rects`: the connector polyline as rects `half_width` (= `half.x +
  conflict_margin/2` = 1.2 + 0.15 = 1.35, `drive.rs:339-343`) either side, no end caps, no body length.
- `connector_clear` (`box_rules.rs:32-49`) tests them with `RoadOccupancy::blocked` (`occupancy/query.rs:182-212`;
  both rects shrunk 0.01 m, `query.rs:109-126`), skipping the requester, characters, and AI cars granted at the
  node. `repick` (`:52-66`) tries the lane's other exits with it. `connector_rects` has no other user in `src/` or
  `tests/`.
- Three callers in `junction.rs`, each with a different requester position:
  - `:106-116` holder demotion `body_blocked`: every occupant, anywhere on its source lane, connector or exit lane;
    consumed only for holders on the source lane past the stop line or on the connector (`:147-163`), and only
    when `stale` (contested and not moved for the 5 s lease).
  - `:223-252` queue head on its lane, or a kinematic car at `s <= REPICK_WITHIN` (1.0) on its connector
    (`at_start`): a blocker standing >= `pass.vehicle_seconds` (3 s) makes it `repick`.
  - `:281-288` every waiter (incl. demoted holders mid-connector): feeds `:316-327` (a waiter blocked by a body
    does not block the node; a body standing >= 3 s with no occupants gives a whole-box grant).
- The conflict table uses `graph.rs:113-139` `body_sweep` (half grown by margin/2, 0.2 m steps, centre from -half.z
  to length + half.z) between two connectors. The box check is a different question: the requester's path against
  **real** standing footprints. The G1 oracle (`tests/traffic_support/mod.rs:255-276` `Footprints`) is unmargined
  with `kinematic_tolerance` 0.02 m, which equals the two 0.01 m shrinks of `blocked`.
- Sensing does not cover the gap: on a connector it skips bodies behind the nose line (`manoeuvre.rs:75-85`), where
  a right-turn pivot's rear swings.
- Queue heads stop with the nose at `lane.stop` (`drive.rs:506-510`); a lane follower keeps the IDM jam gap
  `min_gap` 2.0 m behind its leader's rear (`assets/traffic/traffic.ron` idm).
- Waiter order: `junction.rs:264` `let stamp = *snap.car.waiting.get_or_insert(tick)` and `:265-267` push the
  `(stamp, e, c)` tuple once; `:297-299` sorts by `(stamp, e.to_bits())`. Editing `TrafficCar.waiting` after the
  tuple is stored does NOT change the order. `tick = Time<Fixed>.elapsed / timestep` (`drive.rs:231-232`) is >= 1
  on every fixed run.

### C: residual lateral at connector entry
- `lateral.rs:47-60` `effective_lateral`: on a connector a non-holding offset decays linearly from its entry value;
  `CORRIDOR_YAW_STEP_DEG` 7.5 / `CORRIDOR_LATERAL_STEP` 0.3 at `lateral.rs:11-12`. Pass and Rejoin step it
  (`holds_offset`, `manoeuvre.rs:46-49`).
- A pass cannot reach the box (`pass.rs:213-216`). A siren yield can: `sirens.rs:77-81` has a lane-start guard only.
- `sirens::update` returns `None` ("no change") off a lane (`sirens.rs:44-46`) before its passed/timed-out check
  (`:54-76`, which ends a lane yield with `Rejoin`), so nothing ends a Yield on a connector. `drive.rs:546-549`
  steps the lateral only on lanes or for holding manoeuvres, so a Yield's `car.lateral` is frozen at its entry
  value on the connector; the yield stop (`drive.rs:397-399`) fires while `|lateral - offset| < 0.05`.
- `junction.rs:158-163`: a holder on its connector loses its grant only when `stuck` (a non-walker body blocks
  it), so a car stopped for its yield in the box keeps its grant indefinitely; the stuck cheat only acts off frame.
- `Rejoin` is the wrong end state in the box: it holds the offset (`holds_offset`), so `effective_lateral` would
  jump from the decayed value back to the frozen entry value. `Manoeuvre::None` keeps the decay (no pose jump), and
  `drive.rs:513-524` zeroes `lateral` at the connector end for anything but Rejoin/Pass.
- `sirens.timeout_seconds` 8.0, `yield_distance` 40.0 (`traffic.ron`).

## 4. Implementation steps

### Stage 0: baseline on the current code (no production change)

0.1 Re-run the probes on HEAD (production code identical to `7e15814`) and record outputs under
    `scratch/stage0/`: `walkers` (3 rows), `b_fixture` (4 rows), `c_fixture` (all rows), `geometry_b`,
    `geometry c_threshold`, `rv1_rear_cap`, `rv2_neighbour` (all three tests).
    - Check: each "reproduced" row of §2 reproduces (walkers stall 94/118 s; B G1 0.53/0.68/0.61 m and the + floor
      mutual wait; C lock at s 0.33-0.75). A row that does not reproduce drops its item: append a `dead_end` to
      `log.jsonl` and stop that item.

0.2 C-G1 timing sweep (decides Q2). Extend a copy of `c_fixture.rs` in the probe crate: A is a curb yield
    (offset = lane pitch, begun at lateral 0) on the worst 3 touching curb pairs per seed from `c_threshold`, seeds 1
    and 7; B starts from rest at its stop line. Sweep yield start 6..12 m before the stop line in 0.5 m steps and
    B delay 0..96 ticks in 16-tick steps. Write the G1 max depth per cell to `scratch/stage0/c_sweep.txt`. Any cell
    > 0.02 m = C-G1 reproduced. Run it on HEAD now **and again after Stage 3** (the Stage 3 fix changes what a
    yielding car does in the box; the post-fix sweep decides).

0.3 Side-effect baseline for B. A read-only probe that `#[path]`s the `traffic_gridlock` fixture, seeds 1/2/7/42,
    diffing `TrafficIntersections` and `TrafficCar` each tick. Record per seed:
    - worst stand, and every stand > 20 s;
    - whole-box grants (`whole` None→Some), demotions (occupant → waiter of the same car at a node), re-picks
      (`next` changes while on the source lane, or a connector switch at `s <= 1.0`);
    - per demotion, the blocker's class: same source lane behind the holder / exit lane / on the box / other;
    - `traffic_bench`, `police_bench`, `civilian_bench` means.
    Output: `scratch/stage0/b_side_effects_before.txt`.

### Stage 1: A (walker arrival at a covered waypoint)

1.1 `crates/gta_sim/src/civilian/mod.rs`.
    - In `civilian_fsm`, compute `let cars = standing_cars(road.bodies(), position.0, nav.avoid_distance);` once,
      before the `arrive` call, and pass `&cars` to both `arrive` and `around_cars` (replacing the inline
      `&standing_cars(..)` at `:541`).
    - `arrive` gains two parameters: `cars: &[CarRect]` and `(clearance, corner): (f32, f32)`. It **keeps computing
      `lane_target(ctx.graph, *walker, nav.keep_right)` itself** and replaces the distance test with
      `if !reached(position, target, cars, clearance, corner, nav.arrive_radius) { return state; }`. The
      `around_cars` call keeps reading `lane_target(&graph, *walker, nav.keep_right)` after `arrive`, so a walker
      that just took its next edge steers to the new target in the same tick.
    - New private helper:
      ```rust
      /// Within `arrive_radius` of `target`, or `target` lies under a standing car (grown by `clearance`) and the
      /// walker is within `corner + arrive_radius` of that same car: a waypoint under a car is reached beside it.
      fn reached(position: Vec3, target: Vec3, cars: &[CarRect], clearance: f32, corner: f32, arrive_radius: f32) -> bool {
          flat_distance(position, target) <= arrive_radius
              || cars.iter().any(|car| {
                  car_blocks(target, target, car, clearance)
                      && car_blocks(position, position, car, corner + arrive_radius)
              })
      }
      ```
    - Call site: `arrive(state, &mut walker, position.0, &nav, &ctx, &mut rng, &cars, (loco.capsule_radius,
      loco.capsule_radius + nav.arrive_radius))`. `clearance` 0.3 and `corner` 0.8 are exactly the values
      `around_cars` gets at `:543-544`. No new tuning value.
    - The reach `corner + arrive_radius` = 1.3 m: the implementer re-derives it from `scratch/stage0/walkers_dump`
      (the Stage 0.1 rerun) and names the worst stalled sample's distance to the car rect in the stage summary
      (PLAN names walker 2016 at 0.94 m). If any stalled sample sits farther than 1.3 m from the car, stop and
      report instead of widening.
    - Add `car_blocks` to the import at `:20`: `use crate::tactics::{CarRect, around_cars, car_blocks};`.
    - `arrive`'s doc gets one line: a waypoint under a standing car is reached beside that car.
    - `around_cars` is unchanged.
    - Check: `cargo test -p gta_sim --lib civilian`.

1.2 Unit rows in `civilian/mod.rs` `mod tests`, one per branch of `reached`, numbers worked through the helper
    (car: `CarRect { centre: Vec2::ZERO, axis: Vec2::X, half: Vec2::new(1.2, 2.04) }`, clearance 0.3, corner 0.8,
    arrive_radius 0.5; `target`/`position` built with `Vec3::new(x, 0.0, z)`):
    - (a) no car, target 0.4 m away: reached.
    - (b) no car, target 0.6 m away: not reached.
    - (c) target (0.5, 0, 0) inside the car; walker at (2.2, 0, 0) (1.0 m off the flank, inside 1.2 + 1.3): reached.
    - (d) same target; walker at (4.2, 0, 0) (3.0 m off): not reached.
    - (e) target (2.0, 0, 0) (0.8 m from the flank, 0.5 m outside the rect grown by 0.3: not covered); walker at
      (2.2, 0, 2.0) (1.0 m off the flank, 2.01 m from the target): not reached.
    - (f) two cars: car 1 as above, car 2 centred at (10, 0); target under car 1 at (0.5, 0, 0); walker at
      (7.8, 0, 0) (1.0 m off car 2's flank, 7.3 m from car 1): not reached (pins "the same car").

1.3 New gate file `crates/gta_sim/tests/walk_arrival.rs` (header doc: gate class, derivation, flip).
    - **Row A1 (floor, correctness of the arrival rule).**
      - App: `traffic_floor(lanes, &connectors, &[])` with `let (lanes, connectors) = loop_lanes(6.0);` (so a
        `TrafficGraph` exists and `snapshot_road` runs; no camera view, so the bubble spawns no cars), then
        `test_graph(&mut app, nodes, &edges)` to replace the stub sidewalk graph with the hub: C = (24, 0, 0), arms
        to (24, 0, 12), (24, 0, -12), (12, 0, 0), edges (C, each arm end). These points are clear of every
        `world/test_area.rs` block and of the loop lanes (|x|, |z| = 34..37); the implementer asserts it.
      - `park_car(&mut app, Vec3::new(24.5, 0.0, 0.0), Vec3::X)` (centre 0.5 m from C, as spot B).
      - Preconditions (`GATE BROKEN` with a message naming the gate): after a 1 s warm-up the car is in
        `RoadOccupancy::bodies()` with `standing > 0`; for each arm's edge (arm_end → C) the lane target of C is
        inside the car rect grown by `capsule_radius` (use `lane_target` and the car's footprint); the car has not
        moved (> 0.05 m) during the run.
      - Spawn 6 calm civilians with `spawn_civilian(&mut app, GraphWalker { from: arm_end, to: C }, t, calm())`,
        t 0.2 and 0.6 per arm.
      - Assert every walker's `GraphWalker.from` becomes C within `bound = (d_start + ring / 2) / walk_speed +
        slack`, with `d_start = 12 m x (1 - 0.2)` = 9.6 m, ring perimeter `2 x (2 x (half.x + corner) + 2 x (half.z
        + corner))` = 2 x (4.0 + 5.68) = 19.36 m, so (9.6 + 9.68) / 1.8 = 10.7 s + slack. `slack` comes from the
        first 3 green runs (report the spread). Also assert no walker moves less than 0.7 m over any 10 s window
        before its first arrival. On HEAD it is RED: the walkers orbit.
    - **Row A2 (city, the QA scene; liveness as the player sees it).** The `walkers.rs` probe condensed: seed 1,
      player at the R1 stand point, a car parked at spot B, one run per heading 0 and 90, 150 s. Stall metric:
      a Wander/Flee civilian within 8 m of the car that moved < 0.7 m over a 10 s window; report the longest stall
      per walker.
      - Bound: 20 s (2 x the metric window). Derivation: control without the car 0 walkers >= 10 s; broken 70-118 s.
        Run the fixed code 3 times per heading, report the worst stall next to the bound; if a fixed run exceeds
        10 s, trace that walker before accepting the bound.
      - `GATE BROKEN` if the car moved or is gone, or no walker ever targeted the covered node (found as the
        sidewalk node whose lane target is inside the car's grown footprint, not hard-coded as 312).
      - Print the stall list, as the gridlock gates do.
    - Flip: make the covered branch of `reached` return `false`. A1 and A2 go RED; restore, GREEN. Record it in the
      stage summary.
    - Check: `cargo test -p gta_sim --test walk_arrival` 3 runs; then `traffic_pedestrian`, `civilians`,
      `civilian_city`, `witness_city` **green**. Not "unchanged": the arrival rule changes when walkers draw
      `NpcRng`, which shifts every later civilian roll (TASK-010). Trace any red to a walker and trajectory before
      touching a bound; `witness_city` gates 94/100 seeds, so its margin is thin.

### Stage 2: B (grant check with the requester's real body, swept from where it stands)

2.1 `crates/gta_sim/src/traffic/box_rules.rs`.
    - Delete `connector_rects` (grep first: its only user is `connector_clear`).
    - Add `pub(super) fn connector_body(graph: &TrafficGraph, c: u32, from_s: f32, half: Vec2) -> Vec<FlatRect>`:
      body rects (axis = `flat(right_of(tangent)).normalize_or(Vec2::X)`, `half` = (across, along)) at centre poses
      `graph.pose(Segment::Connector(c), s)` for `s` from `from_s.clamp(0.0, length)` to `length`, both ends
      included. Sampling: the `sweep_ahead` loop shape (`manoeuvre.rs:86-96`): steps of at most
      `CORRIDOR_LATERAL_STEP`, each subdivided so the path yaw between samples is at most
      `CORRIDOR_YAW_STEP_DEG` (both imported from `super::lateral`; compare flat tangents with `Vec2::angle_to`).
      Doc: "The requester's body driven on along connector `c` from `from_s` to its end (centre on the connector;
      the corridor steps bound the corner chord)."
    - `connector_clear(road, graph, junction, c, from_s: f32, requester, half: Vec2)` and
      `repick(road, graph, junction, lane, current, from_s: f32, requester, half: Vec2)`: call
      `connector_body(graph, c, from_s, half)`. `repick` passes the same `from_s` to every other exit (exits part by
      <= 0.42 m within `REPICK_WITHIN`, per the const doc at `junction.rs:51-55`). Doc of `connector_clear`: "the
      requester's body driven on along the connector from where it stands".
    - Module doc (`:1-3`): unchanged in meaning; add nothing beyond what 2.2 states.
    - Why the real body and not the conflict table's grown body: the check compares one path with real standing
      footprints; the G1 oracle is unmargined with 0.02 m tolerance (= the two 0.01 m shrinks of `blocked`); the
      grown body makes other lanes' queue heads at their stop lines "bodies on the path" with no physical contact
      (`scratch/rv2/neighbour.txt`), which would feed repick, whole-box grants and holder demotion.

2.2 `crates/gta_sim/src/traffic/junction.rs` and `drive.rs`.
    - `BoxInputs.half_width: f32` becomes `pub body: Vec2`, doc "Car half extents (across, along), m: the body the
      path check drives along a connector". `drive.rs:339-343`: `body: Vec2::new(half.x, half.z),` (`half` is
      already `vcfg.half_extents()` there; `cfg.conflict_margin` is no longer read at this site).
    - Local helper in `junction.rs`:
      ```rust
      /// Where the car stands along connector `c`: its `s` on it, else 0 (on its source lane).
      fn from_s(snap: &Snap, c: u32) -> f32 {
          if snap.car.segment == Segment::Connector(c) { snap.car.s } else { 0.0 }
      }
      ```
    - `:110` (holders): `connector_clear(boxes.road, graph, Some(junction), c, index.get(&e).map_or(0.0, |&k|
      from_s(&snaps[k], c)), e, boxes.body)`.
    - `:225` (queue head / `at_start`): `from_s(snap, c)`; `:229-237` `repick(.., c, from_s(snap, c), snap.entity,
      boxes.body)`.
    - `:284` (waiters): through `index`, as for holders.
    - Module doc (`:9-13`): one clause, "the path check drives the requester's body on from where it stands".
    - Check: `cargo test -p gta_sim --lib traffic`, `cargo clippy` as in 4.2.

2.3 New gate file `crates/gta_sim/tests/traffic_box_overhang.rs` (header doc: gate classes, derivation, flips).
    - Queue order (all B rows with X and Y): spawn X and Y in the same update; before the first tick in which both
      queue, `set_car(Y, |t| t.waiting = Some(0))` and leave `X.waiting = None` (X gets the current tick, >= 1).
      Precondition (`GATE BROKEN`): before that tick `Y.waiting == Some(0)`, `X.waiting == None` and
      `Time<Fixed>.elapsed / timestep >= 1`; after it, Y is an occupant at the node (HEAD) or a waiter with stored
      stamp 0 (fixed code). Never use who gets the grant as the precondition: on the fixed code X is granted first
      by design (Y waits for a body on its path, which does not block the node).
    - Rows:
      - **B1** + floor (`traffic_occupancy.rs:391-459` geometry): X a kinematic traffic car on c1 = 5 (lane 1 → 7,
        straight, 6.50 m) at s 1.50, speed 0, `next = Some(5)`, no grant; Y on lane 0 at s 2.5, 6 m/s, `next =
        Some(2)` (the right turn lane 0 → 7, 2.63 m). HEAD: G1 0.533 m.
      - **B2** same geometry, Y from rest at its stop line. HEAD: mutual wait >= 17.75 s. Y at its stop line is
        outside X's sweep from s 1.5 (`b_rows_static.txt`: -0.42 m), so on the fixed code X is granted first and
        leaves; report the path taken.
      - **B3** seed 1 node 69 (c1 590 at s 1.47, c2 587), Y from rest; player at `sidewalk_at(hub, 25)` facing the
        box (the camera mutation named, TASK-032 lesson); `bubble.max_cars = 0` via `set_traffic`; every other
        traffic car despawned. `GATE BROKEN` if c2 is not a right turn or does not conflict with c1. HEAD: G1 0.683 m.
      - **B4 (rear-cap regression guard)** + floor: holder H spawned on the straight c1 (lane 0 → 6, 6.50 m) at
        **s 3.5**, speed 0, `next = Some(1)`, pushed into the node's `occupants` (the `traffic_junction_box.rs`
        pattern), and held by a character dummy on c1 just ahead of its nose. Its lane follower F on lane 0,
        `next = Some(1)`, speed 0, nose `min_gap` (2.0 m) behind H's rear (nose ~0.54 m before the connector start).
        A conflicting waiter W on lane 1 at its stop line, `next = Some(5)`, so the lease is contested.
        Preconditions (`GATE BROKEN`): F's nose is within `half.z` of the connector start (so F lies in a from-0
        sweep, `b_rows_static.txt`); F is never an occupant during the window (lane 6 is 12 m; with H counted,
        the room need is 2 x 6.08 = 12.16 m > 12); `reservation_timeout + 0.5 < pass.character_seconds` (5.5 < 6.0:
        no pass around the dummy starts inside the window); c5 is in `graph.connector(1).conflicts` (W contests
        H's lease); H stands.
        Assert over a window of `reservation_timeout + 0.5` s (5.5 s): H stays an occupant (not demoted: only a
        character holds it, TASK-033) and `Footprints` is clean.
        Flip: pass `from_s = 0.0` for holders at `:110`; F (standing >= 3 s) is on H's path, H is demoted at the
        lease (5 s): RED. Restore, GREEN.
      - **B5 (no false block by a queue head)** + floor: Y on lane 0 at its stop line, speed 0, `next = Some(0)`
        (left turn lane 0 → 5), `Y.waiting = Some(0)`; Q on lane 2 at its stop line (nose at `lane.stop`), speed 0,
        `next = Some(6)` (straight lane 2 → 4, conflicts with c0). Precondition: Q is not an occupant before the
        tick. Assert: Y is an occupant at the node after the first tick in which both queue. HEAD: GREEN (band).
        Flip: grow the body by `conflict_margin / 2` in `drive.rs` (`Vec2::new(half.x, half.z) + Vec2::splat(0.15)`):
        Y is refused for Q (c0's corridor sweep meets Q, `b5_plus.txt`), Q is granted first: RED. Restore, GREEN.
    - Assertions B1-B3: `Footprints` clean over the run; Y is never an occupant while X stands at its fixture pose
      (X on c1, `s <= s1 + 0.05`); both cars leave the box (rear past the connector end) within a bound derived
      from the first green run (connector length / `turn_speed` + the IDM start from rest), numbers printed.
    - Flip for B1-B3: swap `connector_body` for the old capless band (`connector_rects` with half width `body.x +
      0.15`) in `connector_clear`. B1/B3 go RED on G1, B2 on liveness and the "Y never granted while X stands"
      assertion. Restore, GREEN.
    - Check: 3 runs. File < 750 lines (if the C rows go here too, count first).

2.4 `crates/gta_sim/tests/traffic_junction_box.rs:136-152` (`through`). The lease gate watches holders "whose
    connector runs through the body"; with the new rule that means "the body lies on the holder's path from where it
    stands". Replace the static band with a per-occupant check at each tick: body rects `(half.x, half.z)` at 0.1 m
    steps along connector k from the holder's `s` on k (0 while on its source lane) to k's end, `obb_overlap`
    against `footprint(&app, body)`. Keep the `GATE BROKEN` that the body's own connector passes the check from s 0.
    Header note: stricter than the old band (it adds rear-swing holders) and exact about holders already past the body.

2.5 Side-effect measurement: re-run the 0.3 probe after 2.1-2.2 into `scratch/stage2/b_side_effects_after.txt`, a
    before/after table per seed (stands, whole-box grants, demotions by blocker class, re-picks, bench means).
    - Expected: a few more waits for bodies and re-picks; no stand > 40 s; ~0 demotions with a blocker "same source
      lane behind the holder" and ~0 "other lane's queue head".
    - "Exit lane" demotions (spillback; the real-body sweep sees 29-42 real E contacts per seed against 6-11 before):
      report them, do not patch without a trace.
    - Any new stand > 20 s is traced by segment (TASK-037 `trace.rs` pattern) before anything changes.

### Stage 3: C (a yield ends in the box)

3.1 `crates/gta_sim/src/traffic/sirens.rs:44-46`:
    ```rust
    let Segment::Lane(l) = car.segment else {
        // A yield is a lane manoeuvre: in the box the car drives out, its offset decaying to the connector end.
        return matches!(car.manoeuvre, Manoeuvre::Yield { .. }).then_some((Manoeuvre::None, 0.0));
    };
    ```
    - `manoeuvre::plan` applies it the same tick (`manoeuvre.rs:200-211`). `holds_offset` is false for `None`, so
      `effective_lateral` keeps the decay the Yield already used (no pose jump); `drive.rs:513-524` zeroes `lateral`
      at the connector end; `v0` is no longer capped at `pass.speed` (`drive.rs:381-384`).
    - One tick of yield braking remains (the obstacle at `drive.rs:397-399` is evaluated before `plan`). Name it in
      the stage summary, do not patch it.
    - Docs: module doc (`:1-2`) one clause ("... and drives out of the box if the yield reached it there"); the
      `update` doc (`:29-30`) "the end of its current yield (also on entering the box)".

3.2 Gate rows: a "C" section in `traffic_box_overhang.rs` or in `police_sirens.rs` (293 lines); both files stay
    < 750 lines.
    - **C1** + floor: A granted on lane 0 → the straight c1, 6 m/s, 1.0 m before its stop line. Named mutation:
      `Manoeuvre::Yield { siren: Entity::PLACEHOLDER, since: <current tick>, offset: slack }` at lateral 0
      (`slack = pitch / 2 - half.x`, 0.425 m), what `sirens::update` sets with no lane-end guard. Timing check:
      `sirens.timeout_seconds` (8 s) is far longer than the < 1 s to the connector. Assert A's rear leaves the
      connector within a bound derived from the first green run and A holds no grant afterwards. HEAD: stands at
      s 0.75 for 30 s.
    - **C2** seed 1 curb scene (lane 437 → connector 1091, curb yield begun 10 m before the stop line; `GATE
      BROKEN` if lane 437 is not a curb lane with `left_gap` 3.25): A leaves the box. HEAD: stands at s 0.35.
    - `Footprints` clean on both.
    - Flip: restore the plain `return None`; both RED. Restore, GREEN.

3.3 C-G1 decision: re-run the 0.2 sweep on the fixed code (`scratch/stage3/c_sweep_after.txt`).
    - All cells clean: drop C-G1; append a `dead_end` citing both sweeps and `geometry_c.txt`. Leave the oracle doc
      in `tests/traffic_graph.rs:4-9` as is (it already names the gap).
    - Any red cell: stop and report to the orchestrator (Q2: a lane-end guard changes siren behaviour).

### Stage 4: docs and full verification

4.1 `docs/architecture/traffic.md`:
    - Lane graph paragraph (`:21-23`): replace "TASK-036 item 4 owns getting that capacity back" with the deferral:
      conflict-point reservation is deferred (no observed capacity problem, `traffic_gridlock` holds its bound); the
      -43 % co-granted pairs on seed 1 is the known cost of swept conflicts.
    - Road occupancy consumers (`:44-47`): replace the "Pending in TASK-036" sentence with the rescope record: fire
      line and sight stay on their own car code (dropped: unification with no observed bug); walk avoidance is on
      `RoadOccupancy` since TASK-037; TASK-036 adds the covered-waypoint arrival.
    - Junction box (`:156-172`): `connector_clear` drives the requester's real body along its connector from where it
      stands (corridor steps), replacing the "Open (TASK-036 item 4)" sentence. Name what it closed (the rear-swing
      G1, the holder/waiter mutual wait) and why it starts at the car (the TASK-033 lease: a sweep from s 0 reaches
      the holder's own lane follower) and why it has no margin (a grown body meets other lanes' queue heads).
    - Walkers (the `around_cars` sentence): a sidewalk node under a standing car counts as reached beside the car;
      the TASK-037 QA "counter-flow" hypothesis is refuted, the cause is an unreachable walk target under a standing
      car (Q4).
    - Sirens (`:174-182`): a yield ends when the car is in the box, and it drives out; the C-G1 result (thresholds
      2.3/2.7 m, only the curb yield reaches them, the App sweep before/after).
    - Q3: add the curb-clip observation (up to 0.51 m on ~140 connectors per seed at a 3.0-3.25 m entry) to the
      `docs/narrative-graph.md` backlog as an observation, not a task.

4.2 Full verification:
    - Windows: `cargo test -p gta_sim -p citygen`; clippy exactly as `.github/workflows/clippy.yml` runs it;
      `cargo test -p gta_like --bin gta_like`; `python tools/qa/tree_check.py`.
    - `traffic_gridlock` seeds 1/2/7/42 within 40 s; `traffic_bench` / `police_bench` / `civilian_bench` under
      `MEAN_LIMIT`, compared with the 0.3 baseline (`connector_body` has ~10-45 rects per call against ~7 before).
    - WSL Ubuntu 22.04, toolchain 1.95.0: the full `gta_sim` + `citygen` suite in the mirror, with
      `maw/tasks/done/TASK-037/scratch/wsl/wsl_sync.sh` and `linux_run.sh` patterned into this task's scratch
      (`$HOME/gta036`, `CARGO_TARGET_DIR` in the WSL filesystem). A Linux-only red is a real bug: trace it.
    - After merge: CI 5/5 on the merge commit.

## 5. Test plan

| Gate | Class | What goes RED on HEAD | Flip (perturbed input) |
|---|---|---|---|
| `civilian` unit rows (a)-(f) | correctness of `reached`, per branch | (c) | n/a (pure helper; each row pins a branch) |
| `walk_arrival` A1 | correctness of the arrival rule | walkers never reach C | covered branch of `reached` → `false` |
| `walk_arrival` A2 | liveness as seen by the player | stalls 70-118 s | same |
| `traffic_box_overhang` B1, B3 | correctness (G1 oracle) + grant order | G1 0.533 / 0.683 m | old capless band in `connector_clear` |
| B2 | liveness (no mutual wait) | wait >= 17.75 s | same |
| B4 | regression guard (TASK-033 lease vs rear cap) | green on HEAD | holders' `from_s` = 0 |
| B5 | regression guard (no queue-head false block) | green on HEAD | body grown by margin/2 |
| C1, C2 | liveness (yield lock in the box) + G1 | stand at s 0.75 / 0.35 | plain `return None` in `sirens::update` |
| `traffic_junction_box` lease | correctness, stricter `through` | n/a | existing |

Expected outcomes: all new rows GREEN on the fixed code and RED under their flip (recorded per row in the stage
summaries, with the perturbed input named); the A2 and B rows run 3 times; `traffic_pedestrian`, `civilians`,
`civilian_city`, `witness_city`, `traffic_junction_box`, `traffic_go_around`, `police_sirens` and the whole suite
green on Windows and in WSL; `traffic_gridlock` within 40 s on seeds 1/2/7/42; bench means under `MEAN_LIMIT`.

## 6. Rollout notes

- No migrations, env vars, feature flags or data-file changes. No new tuning value (A reuses `capsule_radius` and
  `arrive_radius`; B uses the chassis half extents and the existing corridor law consts; C is a match arm).
- Behaviour changes the owner can see: walkers no longer circle a car parked over a sidewalk node; cars wait for a
  standing car their turning body would hit; a car yielding into the box drives out. City trajectories move (one
  traffic rule change moves every trajectory, TASK-037), and civilian rolls shift (NpcRng draw order, TASK-010).
- Only `gta_sim` changes; no `bevy_render` in `gta_sim` (no new dependency). Probe crates stay in `scratch/`.

## 7. Risk areas

- **B, body model.** The real body with 0.01 m shrinks on both sides can miss an overlap of up to 0.02 m (the
  oracle tolerance) plus the corridor corner sagitta (~0.007 m on a 1.3 m-radius right turn at 7.5° steps). The
  guarantee is the `Footprints` oracle in B1-B5 and the city gates, not the step; do not coarsen sampling or add a
  margin without re-running B4/B5 and `rv2_neighbour`.
- **B, start point.** A sweep from s 0 reaches the holder's own follower (B4). If a later change needs the full
  `body_sweep` (source/exit-lane poses), re-run B4 and the 2.5 demotion classes.
- **B moves every city trajectory.** A red elsewhere is usually a latent lock the new trajectory reaches; trace it
  before reverting. Watch `traffic_junction_box` seed 7 (ignored liveness, TASK-039 class) and `traffic_go_around`.
- **B, exit-lane spillback.** The nose cap (2.04 m past the connector end) and the pivot now see real spillback
  contacts on other exit lanes; the room check keeps AI cars off a granted car's own exit lane. Measured in 2.5.
- **B, transient blocks.** A car just released onto its exit lane can overlap the nose cap for a few ticks: that
  delays a grant only; demotion and repick need 3 s standing.
- **B, requester lateral.** The sweep is on the path line; a requester entering with a residual offset is item C
  (and the Stage 0.2 sweep), not modelled here.
- **A, NpcRng shift** and **A2 phase sensitivity / platform divergence**: gate the longest stall, 3 runs, re-run in
  WSL (TASK-022, TASK-038).
- **A, reach 1.3 m.** A walker at the far end of the car may take the next edge early: harmless, `around_cars`
  routes the next target. Two nodes under one car (4.73 m diagonal > 4.5 m crossing spacing): both are passed;
  `wander_next` returns `from` only at a dead end.
- **C, one tick of braking** remains (3.1).

## 8. Review notes (what changed and why)

Disconfirmation (tested before the review): "V2's B body, grown by `conflict_margin / 2`, makes a queue head
standing at its stop line on ANOTHER lane of the same node a body on the requester's path." **It held.** Probe
`scratch/probe/ws/probe/tests/rv2_neighbour.rs` (`scratch/rv2/neighbour.txt`, `b5_plus.txt`): with the grown body
every turning connector of the + floor meets some queue head (4 with corridor sampling), and 5-8 turns per city
seed do (worst 0.12 m, less than the 0.15 m grow, so none is a physical contact); with grow 0.05 or 0 there are none.
Consequences in code: `junction.rs:316-327` lets the opposite queue head overtake the turner's FCFS slot,
`:223-240` makes the turner re-pick once the head has stood 3 s, and `:147-163` can demote a holder waiting for
walkers (the TASK-033 lease) over that head.

Changes against PLAN_V2 (V2's corrections to PLAN.md are kept):
1. **B uses the real body** (`BoxInputs.body = Vec2::new(half.x, half.z)`), not the grown one; docs no longer claim
   "the conflict table's body". Reason above; the reproduced G1 depths (0.53-0.68 m) are caught either way
   (`b_rows_static.txt`).
2. **B4 fixture geometry fixed.** V2 put H at s ~1, where F's nose is 3.04 m behind the connector start and the
   `from_s = 0` flip stays GREEN (static check). H is now at s 3.5 with a precondition that F lies within `half.z`
   of the connector start, plus a window shorter than `pass.character_seconds` and a room precondition that keeps F
   ungranted.
3. **Queue-order mutation fixed.** V2's "run one tick, read X's stamp, set `Y.waiting = x_stamp - 1`" does not work:
   the waiter tuple keeps the stamp it was pushed with (`junction.rs:264-267`), and Y may be granted on that first
   tick. Pre-set `Y.waiting = Some(0)` before the first shared tick; X is stamped with the current tick, which is
   >= 1 on every fixed run (`drive.rs:231-232`). The precondition pins the stamps, not who is granted (V2's point).
4. **New row B5** (no false block by an opposite queue head), flipped by the grown body.
5. **B rows assert the rule directly:** Y is never an occupant while X stands at its fixture pose (V2 had this in a
   form that needed the production predicate in the test).
6. **2.4 `through` set** is evaluated per holder from its current `s`, matching the new rule; a static from-0 set
   would flag holders already past the body.
7. **A1 fixture made concrete:** `traffic_floor` + `loop_lanes(6.0)`, then `test_graph` replaces the stub sidewalk
   graph (it cannot share nodes through `sidewalk_runs`); hub coordinates, car pose and the worked bound
   (10.7 s + slack) are given. A2 bound given as 20 s with its derivation instead of "derived later".
8. **A unit rows** get concrete coordinates, each worked through `car_entry`.
9. **C:** stated why `Manoeuvre::None` and not `Rejoin` (Rejoin holds the offset and would jump the pose); C1's
   timing against `sirens.timeout_seconds`; C2 gets a layout precondition.
10. Restored from PLAN.md where V2 compressed it: the evidence table, the traffic.md line-level edits, the WSL
    script paths, the rejected alternatives below.

Rejected alternatives (kept from PLAN.md, still valid): projecting the waypoint onto the car's corner ring (all flows
converge on one point); walker-walker or counter-flow rules (the trace shows none); widening the band by the 0.91 m
overhang (blocks straight connectors for nothing); the full `body_sweep` with source/exit-lane poses (task text
warns about repick/stuck behaviour; every probed contact had Y's centre on its connector). New: the grown body (above);
skipping AI cars on a lane before their stop line (a semantic hole for yielding or passing cars; geometry already
answers it with the real body).

Research check: no new crate or Bevy API is used. Rust/glam calls in the plan (`bool::then_some`, `Vec2::angle_to`,
`Vec2::normalize_or`) are already used in this crate (`manoeuvre.rs:94`, `box_rules.rs:23`). The AIM reference
(Dresner & Stone, JAIR 2008: reservations simulate the vehicle's footprint from its current state through the box)
supports sweeping from where the car stands.

## 9. Open questions

None new. Q1-Q4 are resolved in TASK_FINAL; the review changes stay inside that scope.

children: 0 launched / 0 reported
