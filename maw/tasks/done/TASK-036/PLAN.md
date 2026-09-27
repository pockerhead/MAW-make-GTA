# PLAN — TASK-036 (rescoped): walker pile-up at a car, `connector_rects` overhang, residual lateral at connector entry

Binding scope (TASK_FINAL "Orchestrator rescope", orchestrator note): three causal-first items A, B, C. Fire
line, sight and conflict-point reservation are out (conflict points get one line in traffic.md as the known
cost of swept conflicts). A cause that does not reproduce headless is dropped with evidence, not built.

Cost of error per item:
- A: the owner sees it on the first minute near a left car. Mechanism plus one honest city gate.
- B: silent. Kinematic cars drive through each other (G1), or a grant holder and a waiter wait on each
  other. Full evidence layer.
- C: silent freeze in view. Full evidence layer for what reproduced.

## 0. What the planner already reproduced (read-only probes, all under `scratch/`)

Probe crate: `scratch/probe/ws/probe` (a copy of the TASK-037 probe crate, renamed `probe036`, with its
own `scratch/probe/assets` copy). Build and run with
`CARGO_TARGET_DIR=D:/test-gta-like/target cargo test --offline --test <name> -- --nocapture`.
Do not use `--release`: it rebuilds all of bevy.

| Probe | Output | Result |
|---|---|---|
| `tests/walkers.rs` (A trace, seed 1, TASK-037 spot B, 150 s, plus a run without the car) | `scratch/walkers.txt`, `scratch/walkers_dump.txt` | Reproduced: 13 walkers stall up to 94 s (car heading 0) and 11 up to 118 s (heading 90). The control run without the car has 0 stalls. **The cause is not counter-flow.** 0 of 6361 stalled samples have a counter-flowing nearest walker. 72 % / 58 % have a same-direction neighbour. 100 % steer to a car corner. The parked car (centre (5.30, -84.50)) covers sidewalk node 312 at (5.80, -84.43). Every stalled walker is on an edge whose `to` is node 312: (311→312), (314→312), (259→312), (307→312). Its lane target (5.81, -84.93) lies inside the car. |
| `tests/geometry_b.rs` (B static, seeds 1..8 and the + floor) | `scratch/geometry_b.txt` | On every seed ~2200 (c1, c2) pairs have a body X standing on c1 outside c2's band but inside c2's swept body. The exit-lane room check blocks none of them. All contacts are with Y's centre on its connector. The worst depth is 0.71 m. From rest, Y cannot stop in time on 276-334 pairs per seed. Dominant row: X on a near-straight connector (len ~6.2 m) about 1.45 m in, Y on an adjacent right turn (len 2.0-2.5 m), both into the same exit lane. |
| `tests/b_fixture.rs` (B in the App) | `scratch/b_fixture.txt` | **Reproduced.** X is a kinematic traffic car on c1 with no grant. Y's grant is decided while X stands there (spawn order). Results: + floor with Y at 6 m/s: G1 0.53 m (kinematic×kinematic, 12 ticks). Seed 1 node 69 (c1 590 s1 1.47, c2 587) from rest: G1 0.68 m. Seed 1 node 92 (c1 804, c2 801) from rest: G1 0.61 m. + floor from rest: no G1, but Y stops at s 0.47 on c2 still holding its grant, and X waits for c1, which conflicts with Y's grant. Nothing changed for the remaining ≥ 17.75 s: a mutual wait. In the city rows Y never braked: the contact is Y's rear swing, which sensing skips by design (`manoeuvre.rs:51-54, 76-85`). |
| `tests/geometry.rs` `c_entry_offset`, `c_threshold`, `c_block_clip` (C static) | `scratch/geometry.txt`, `scratch/geometry_c.txt`, `scratch/geometry_clip.txt` | Co-granted pairs touch only when car A enters with a right (yield-side) offset of ≥ 2.29-2.46 m (Yield decay law) or ≥ 2.74-2.89 m (stepped Rejoin law), on seeds 1..8. Street slack yields (0.425 m) and 1.0 m touch no co-granted pair. Only the avenue curb-lane yield (3.25 m) exceeds the threshold (64-102 ordered pairs per seed). Side finding: curb-yield entries of ≥ 2 m clip a city block's curb prism on 16-24 connectors per seed (2 m) and on 138-149 (3.0-3.25 m), up to 0.51 m. |
| `tests/c_fixture.rs` (C in the App) | `scratch/c_fixture.txt`, `scratch/c_fixture2.txt` | **G1 not reproduced** in 5 timings of the seed 1 curb pair 1091/1089: B's sensing slowed B in time. **Reproduced instead, a lock:** a granted car whose yield brings it to its full offset at or past the lane end (slack yield begun 1 m before the stop line on the + floor; curb yield begun 10 m before on seed 1, lane 437) enters the connector, and the yield stop (`drive.rs:399-401`) halts it at s 0.33-0.75. It then stands there **with its grant** for the rest of the run (30 s and 9 s observed). |

## 1. Understanding

### A: walkers around a standing car
- `civilian/mod.rs:479-550` `civilian_fsm` computes `next_state`, then `arrive` (`:428-455`), then steers to
  `around_cars(position, lane_target, standing_cars(...), capsule_radius, capsule_radius + arrive_radius)`
  (`:538-548`).
- `arrive` takes the next sidewalk edge only when `flat_distance(position, lane_target) <= arrive_radius`
  (0.5 m, `assets/npc/navigation.ron`).
- `tactics/fire_line.rs:141-199` `around_cars` returns a car corner `corner` = 0.8 m out while the straight
  line to `to` is blocked. A corner within `corner - clearance` (0.5 m) counts as reached for the corner
  choice only.
- When `to` lies inside a standing car (grown by the 0.3 m clearance), the line from any point enters the
  car, so `around_cars` never returns `to`. The walker never gets within 0.5 m of it, so `arrive` never
  fires. The walker orbits the car from corner to corner. Walkers from all edges into that node share the
  same four corner points and push against each other at them. The QA intents "towards and away from the
  car" are this orbit.
- `standing_cars` (`:460-476`) gives the car footprints (from `RoadOccupancy`, standing > 0).
  `tactics::car_blocks(p, p, car, r)` (`fire_line.rs:120-122`, the degenerate segment in `car_entry`) is
  "p within r of the car rect".

### B: `connector_rects` overhang
- `traffic/box_rules.rs:12-28` `connector_rects` models the requester's connector as the polyline band
  ± `half_width` (= `half.x + conflict_margin/2` = 1.35 m, `drive.rs:339-343`), with no end caps and no
  body length.
- `connector_clear` (`:32-49`) tests those rects with `RoadOccupancy::blocked` (`occupancy/query.rs:183-212`,
  real footprints, 1 cm shrink). It skips the requester, characters and AI cars granted at the node.
- Call sites, all in `junction.rs`: holder demotion `body_blocked` (`:106-116`), the queue-head re-pick
  (`:224-240`, via `repick` `box_rules.rs:52-66`), and the grant pass `blocked` map (`:281-288`), which
  feeds `:316-327` (a waiter blocked by a body does not block the node; a body standing
  `pass.vehicle_seconds` on every way out gives a whole-box grant).
- The conflict table models bodies with `graph.rs:113-139` `body_sweep` (half grown by margin/2, 0.2 m
  steps, nose-in to rear-out). So the grant check and the table use different body models, and a body the
  table does not know (a demoted holder `junction.rs:158-161`, a car recovered or re-picked on its
  connector, a left car) in the pivot overhang is invisible at grant time.
- Sensing does not cover the gap. On a lane it is a straight strip (`manoeuvre.rs:158-161`). On a connector
  it skips every body behind the nose line (`:75-85`), which is where a right-turn pivot's rear swings.
- `held_in_box` (`drive.rs:137-149`): a car on a connector with no grant does not move.

### C: residual lateral at connector entry
- `lateral.rs:308-325` `effective_lateral`: on a connector a non-holding offset decays linearly from its
  entry value. Pass and Rejoin step it (`holds_offset`, `manoeuvre.rs:46-49`).
- A pass cannot reach the box: `pass.rs:213-216` refuses `end_s + half.z > lane.stop`, and the lateral
  reaches 0 by `end_s`.
- A siren yield can reach the box. `sirens.rs:77-81` has a lane-start guard but no lane-end guard. In
  `drive.rs:543-546` the lateral steps only on lanes or for holding manoeuvres, so on a connector a Yield's
  `car.lateral` is frozen at its entry value. The yield stop (`drive.rs:397-401`) fires while
  `|lateral - offset| < 0.05`.
- `sirens::update` returns `None` off a lane (`sirens.rs:44-46`) before its "passed or timed out" check
  (`:54-76`), so nothing ever ends a Yield on a connector. `manoeuvre::plan` (`:197-234`) makes no other
  change for a non-idle car.
- `junction.rs:158-163`: a holder on its connector loses its grant only when `stuck` (a non-walker body
  blocks its path). So a car that stopped for its yield in the box holds its grant indefinitely. The stuck
  cheat (`stuck.rs`) removes it only out of frame, so the lock stays in view.

## 2. Approach

**A: the arrival rule, not steering.** A sidewalk waypoint covered by a standing car is unreachable. Count
it as reached once the walker is next to the car that covers it, then take the next edge.
`around_cars` then routes the walker around the car to its next node.
- Rejected: projecting the waypoint onto the car's corner ring. Every flow through the node would converge
  on one shared point, a new bottleneck.
- Rejected: walker-walker avoidance or counter-flow rules. The trace shows no counter-flow, and without the
  car there are no stalls.
- Genre reference: crowd systems pair global waypoints with local steering and guard against steering at an
  unreachable goal ([GameAIPro ch. 23, flow-field crowds](https://www.gameaipro.com/GameAIPro/GameAIPro_Chapter23_Crowd_Pathfinding_and_Steering_Using_Flow_Field_Tiles.pdf);
  [GameAIPro 2 ch. 17](https://www.gameaipro.com/GameAIPro2/GameAIPro2_Chapter17_Advanced_Techniques_for_Robust_Efficient_Crowds.pdf)).

**B: the grant check uses the requester's swept body.** Use the model the conflict table already uses:
half extents grown by `conflict_margin / 2`, sampled along the connector. The band stays out.
- This is the AIM reservation idea: the manager simulates the vehicle's footprint along its path through
  the box, not a centre line ([Dresner & Stone, JAIR 2008](https://www.cs.utexas.edu/~aim/papers/JAIR08-dresner.pdf)).
- Only the centre-on-connector part of the sweep is used (centre from s 0 to the connector length). The
  body still reaches `half.z + margin/2` past both ends.
- Not the full `body_sweep`, which puts the centre 2 m back on the source lane and 2 m onto the exit lane.
  The task text warns that this changes re-pick/stuck behaviour, and every probed contact had Y's centre on
  its connector (`geometry_b.txt`: `[lane, connector, exit] = [0..7, all, 0]`).
- Samples follow the recovery corridor law (`lateral.rs:273-276`: at most 0.3 m and 7.5° of path yaw
  apart, corner chord ≤ 0.31 m). The same law drives `sweep_ahead` (`manoeuvre.rs:86-96`). The margin/2
  grow (0.15 m) covers half that chord.
- Rejected: widening the band by the measured 0.91 m overhang. It blocks straight connectors for nothing
  and is not the table's model.

**C: fix the reproduced lock, drop the unreproduced G1.** A Yield is a lane manoeuvre. On a connector it
ends (`Manoeuvre::None`), so the offset decays to the connector end as it already does for a yield that is
still stepping, and the car drives out of the box.
- The co-granted G1 did not reproduce: the offset threshold is 2.3 m, only the curb yield reaches it, and
  the App timings were clean. Stage 0 re-runs the timing sweep. Only a red there builds the lane-end guard
  (Open question 2).

## 3. Steps

### Stage 0: baseline on the current code (no production change)
0.1 Build and run the planner probes on HEAD `7e15814`. Copy `scratch/probe` into the implementer's
    scratch or reuse it read-only, and record outputs under `scratch/stage0/`: `walkers` (all 3 rows),
    `b_fixture` (4 rows), `c_fixture` (all rows), `geometry_b`, `geometry c_threshold`.
    - Expected: the numbers in §0.
    - Check: files exist, and each §0 "reproduced" row is RED again. If a row does not reproduce, drop that
      item and log a `dead_end`.

0.2 C G1 timing sweep (decides Open question 2). Extend `c_fixture.rs`:
    - Cars: A is a curb yield (offset = lane pitch, begun with lateral 0) on each touching curb pair from
      `c_threshold` (take the worst 3 per seed on seeds 1 and 7). B starts from rest at its stop line.
    - Sweep: yield start 6..12 m before the stop line in 0.5 m steps, B delay 0..96 ticks in 16-tick steps.
    - Record: the G1 max depth per cell in `scratch/stage0/c_sweep.txt`.
    - Check: any cell with G1 > 0.02 reproduces C-G1. Otherwise C-G1 is dropped with this table.

0.3 Baselines the side-effect measurement of B needs.
    - Record per seed (1/2/7/42) on `traffic_gridlock`: the worst stand, every stand over 20 s, and counts
      of whole-box grants, demotions and re-picks.
    - How to count: a read-only probe that `#[path]`s the gridlock fixture and diffs `TrafficIntersections`
      and `TrafficCar.next` each tick. Whole-box grants: `whole` None→Some. Demotions: occupant→waiter of
      the same car at a node. Re-picks: `next` changes while `segment` is the source lane, or a connector
      switch at `s <= 1.0`.
    - Also record `traffic_bench` and `police_bench` means.
    - Output: `scratch/stage0/b_side_effects_before.txt`.

### Stage 1: A (walker arrival at a covered waypoint)
1.1 `crates/gta_sim/src/civilian/mod.rs`.
    - In `civilian_fsm`, compute `let cars = standing_cars(road.bodies(), position.0, nav.avoid_distance);`
      once, before `arrive`, and reuse it for `around_cars` (today it is computed inline at `:542`).
    - Change `arrive` to take the steering target it tests (`target: Vec3`, the `lane_target` value) plus
      `cars: &[CarRect]` and the two radii. It no longer calls `lane_target` itself; the caller passes it,
      one computation.
    - New reach rule (one helper, `fn reached(position, target, cars, clearance, corner, arrive_radius) -> bool`):
      `flat_distance(position, target) <= arrive_radius`, OR some car in `cars` covers the target and the
      walker is next to that car. Covers: `car_blocks(target, target, car, clearance)`. Next to it:
      `car_blocks(position, position, car, corner + arrive_radius)`.
      - `clearance` = `loco.capsule_radius` (0.3). `corner` = `capsule_radius + arrive_radius` (0.8), the
        same values `around_cars` gets at `:543-544`. No new tuning value.
      - The `corner + arrive_radius` reach (1.3 m) is derived from the trace. Walkers on the corner ring sit
        0.8 m from the car and are pushed out to about 1.0 m: stalled walker 2016 at (6.79, -81.52) is
        0.94 m from the car's end. The implementer re-derives this from the Stage 0 dump and names the
        worst sample.
    - `car_blocks` and `CarRect` are already imported (`:20`), and `car_blocks` is exported `pub(crate)`
      from `tactics/mod.rs:7`.
    - Doc comment on `arrive`: one line saying a waypoint under a standing car is reached beside the car.
    - Why: the only cause the trace shows. `around_cars` stays unchanged.
    - Check: `cargo test -p gta_sim --lib civilian`.

1.2 Unit rows in `civilian/mod.rs` `mod tests`, one per branch of `reached`, all numbers worked through the
    helper:
    (a) target 0.4 m away, no car: reached.
    (b) target 0.6 m away, no car: not reached.
    (c) target inside a car, walker 1.0 m off its side: reached.
    (d) target inside a car, walker 3 m away: not reached.
    (e) target 0.5 m outside the car's grown rect, walker beside the car but 2 m from the target: not
        reached (only a covered target is excused).

1.3 New gate file `crates/gta_sim/tests/walk_arrival.rs`.
    - Header doc: class, derivation, flip.
    - **Row A1 (floor, mechanism).** Build a sidewalk graph with `test_graph`: centre node C and three
      12 m arms. Place it clear of `world/test_area.rs`, e.g. around (-22, 22), the + floor area other
      gates use, and add a `GATE BROKEN` check that the parked car's chassis is clear of test-area
      geometry. `park_car` with its centre 0.5 m from C (the spot B offset). Spawn 6 calm civilians, two
      per arm, heading to C (`spawn_civilian(GraphWalker{from: arm_end, to: C}, t)` at t 0.2 and 0.6).
      Run for the bound below. Assert that every walker's `GraphWalker.from` becomes C (it took the next
      edge) within `bound = (longest start-to-ring distance + half the ring perimeter
      2·(2·(half.x+corner) + 2·(half.z+corner))/2) / walk_speed + slack`. The slack is derived from the
      first green run of 3 (report the spread). Also assert that no walker moves less than 0.7 m over
      any 10 s window (the probe's stall metric).
    - **Row A2 (city, the QA scene).** The `walkers.rs` probe condensed: seed 1, the player at the R1 stand
      point, a car parked at spot B headings 0 and 90, 150 s, the stall metric (Wander/Flee within 8 m,
      moved < 0.7 m). Assert the longest stall is ≤ 10 s. Derivation: control without the car 0 walkers
      ≥ 10 s, broken 70-118 s. Print the stall list, as the gridlock gates do. Add `GATE BROKEN` if the
      car moved or is gone, or if no walker ever targeted node 312 (the covered node, found as the sidewalk
      node inside the car footprint, not hard-coded).
    - Flip: return `false` for the new "covered" branch of `reached`. Both rows go RED: A1 on arrival, A2
      at ~70-118 s. Restore, both GREEN. Record in the stage summary.
    - Check: `cargo test -p gta_sim --test walk_arrival` 3 runs (walker phase varies), then
      `traffic_pedestrian`, `civilians`, `civilian_city`, `witness_city` unchanged.

### Stage 2: B (grant check with the requester's swept body)
2.1 `crates/gta_sim/src/traffic/box_rules.rs`.
    - Replace `connector_rects` with `connector_body(graph, c, half: Vec2) -> Vec<FlatRect>`: body rects
      (axis = `right_of(tangent)`, `half` = (across, along)) at centre poses along connector `c` from s 0
      to its length.
    - Sampling: the corridor law, the same loop shape as `manoeuvre.rs:88-96`: steps of at most
      `CORRIDOR_LATERAL_STEP`, subdivided so path yaw between samples is at most `CORRIDOR_YAW_STEP_DEG`.
      Both are imported from `lateral`. Always include both ends.
    - `connector_clear` and `repick` take `half: Vec2` instead of `half_width: f32`, and their doc says
      "the requester's body driven along the connector".
    - Delete `connector_rects`: its only user was `connector_clear` (grep before deleting).
    - Why: the observed G1 and the mutual wait both come from the band missing the pivot.

2.2 `traffic/junction.rs:42-49`.
    - `BoxInputs.half_width: f32` becomes `body: Vec2`, documented as "car half extents grown by half the
      conflict margin, the conflict table's body".
    - Update the four call sites (`:110`, `:225`, `:236`, `:284`) and the module doc (`:9-13`).
    - `traffic/drive.rs:339-343`: `body: Vec2::new(half.x, half.z) + Vec2::splat(cfg.conflict_margin / 2.0)`.

2.3 Gate file `crates/gta_sim/tests/traffic_box_overhang.rs` (new).
    - Rows are the §0 `b_fixture` scenes, rewritten without the entity-order loop.
    - Named mutation for grant order: after spawning X on c1 (at rest, `next = c1`) and Y (`next = c2`),
      set `Y.waiting = Some(0)` ("Y has queued since tick 0"; `junction.rs:264` keeps an existing stamp).
      Y then wins the first-come grant pass while X is a later waiter standing on c1: the state after a
      demotion. Add `GATE BROKEN` if after tick 1 Y is not a waiter or occupant, or X is granted before Y
      leaves.
    - Rows:
      - **B1** + floor (the `traffic_occupancy.rs:391-459` geometry): c1 = the straight whose static pick
        is the worst (`pick` in `b_fixture.rs`: c1 5 s1 1.50, c2 2 on HEAD). Y at 6 m/s from lane s 2.5.
      - **B2** same geometry, Y from rest at its stop line.
      - **B3** seed 1 node 69 (c1 590 s1 1.47, c2 587), Y from rest. Player at `sidewalk_at(hub, 25)` facing
        the box, `bubble.max_cars = 0`, every other traffic car despawned. Connector ids are
        seed-deterministic but asserted: `GATE BROKEN` if c2 is not a right turn or does not conflict with
        c1.
    - Assertions per row: `Footprints` clean (B1/B3 RED on HEAD at 0.53/0.68 m). Both cars leave the box
      (rear past the connector end) within a bound derived from the first green run (distance / turn_speed
      plus the IDM start). B2 is RED on HEAD: nothing moves for ≥ 17.75 s.
    - Keep a pure predicate row too: for B1's (X pose, c2), `connector_clear` would need
      `pub(crate)`/test access. Instead assert through the resource that Y is not in `occupants` while X
      stands in its sweep.
    - Flip: restore the band model (swap `connector_body` for the old `connector_rects` with half_width
      `body.x`). B1/B3 go RED on G1, B2 on liveness. Restore, GREEN.
    - Check: 3 runs.

2.4 `tests/traffic_junction_box.rs:137-152`.
    - The `through` set (which holders the lease gate watches) uses the old band. Recompute it with body
      rects along the connector (same grow, a 0.1 m step suffices in the test), so the lease gate watches
      every holder the new rule can demote.
    - Note in the header doc that this makes the gate stricter, not looser.

2.5 Side-effect measurement (task text: "measure the repick and stuck side effects").
    - Re-run the Stage 0.3 probe after 2.1-2.2. Output: `scratch/stage2/b_side_effects_after.txt`, as a
      before/after table per seed: stands, whole-box grants, demotions, re-picks.
    - Expected: more "waits for a body on its path" and a few more re-picks. No stand over 40 s.
    - Any new stand over 20 s is traced before anything is changed ("name segments from traces", TASK-037).
      Use the TASK-037 `trace.rs` pattern in the probe crate.

### Stage 3: C (a yield ends in the box)
3.1 `crates/gta_sim/src/traffic/sirens.rs:44-46`. Replace the early return with:
    ```rust
    let Segment::Lane(l) = car.segment else {
        // A yield is a lane manoeuvre: in the box the car drives out, its offset decaying to the connector end.
        return matches!(car.manoeuvre, Manoeuvre::Yield { .. }).then_some((Manoeuvre::None, 0.0));
    };
    ```
    - `plan` (`manoeuvre.rs:201-211`) applies it the same tick. `drive.rs:520-524` zeroes `lateral` at the
      connector end.
    - Pose continuity: Yield already uses the decay law on the connector, so `effective_lateral` does not
      jump.
    - Consequence: one tick of yield braking remains, because the yield obstacle is evaluated before `plan`
      (`drive.rs:397-401`). Named in the step summary, not patched.
    - Update the module doc (`:1-2`) with one clause.

3.2 Gate rows. Add a "C" section to `tests/traffic_box_overhang.rs`, or put them in
    `tests/police_sirens.rs` if the implementer prefers the sirens home. Both files stay < 750 lines.
    - **C1** + floor: A is granted on lane 0 → the straight, at 6 m/s, 1.0 m before its stop line. Named
      mutation: `Manoeuvre::Yield { siren: PLACEHOLDER, since: <now>, offset: slack }` at lateral 0, which
      is what `sirens::update` sets with no lane-end guard. Assert that A's rear leaves the connector
      within a derived bound and that A holds no grant after that. On HEAD, A stands at s 0.75 on the
      connector for 30 s.
    - **C2** seed 1 curb scene (A lane 437 → connector 1091, curb yield begun 10 m before the stop line): A
      leaves the box. RED on HEAD: stands at s 0.35.
    - `Footprints` on both.
    - Flip: restore the plain early return; both RED. Restore, GREEN.

3.3 C-G1 decision from Stage 0.2.
    - All cells clean: drop C-G1. Log a `dead_end` citing `c_sweep.txt` and `geometry_c.txt`. Leave the
      oracle doc in `tests/traffic_graph.rs:4-9` as is (it already names the gap).
    - Any cell red: stop and report under Open question 2 before building the lane-end guard. It is a
      behaviour change of the siren yield, and the orchestrator decides.

### Stage 4: docs and full verification
4.1 `docs/architecture/traffic.md`:
    - Lane graph paragraph (`:21-23`): replace "TASK-036 item 4 owns getting that capacity back" with the
      deferral. Conflict-point reservation is deferred (no observed capacity problem, `traffic_gridlock`
      holds its bound). The -43 % co-granted pairs on seed 1 is the known cost of swept conflicts.
    - Road occupancy consumers (`:44-47`): replace the "Pending in TASK-036" sentence with the rescope
      record. Fire line and sight stay on their own car code, dropped as unification with no bug. Walk
      avoidance is on `RoadOccupancy` since TASK-037, and TASK-036 adds the covered-waypoint arrival.
    - Junction box (`:156-172`): `connector_clear` tests the requester's body swept along the connector
      (the conflict table's body, corridor steps), replacing the "Open (TASK-036 item 4)" sentence. Name
      what that closed: the G1 through a rear swing, and the grant-holder/waiter mutual wait. Add the
      walker rule in the sentence about `around_cars`: a sidewalk node under a standing car counts as
      reached beside the car (TASK-036, the TASK-037 spot B pile-up; not counter-flow).
    - Sirens (`:174-182`): a yield ends when the car is in the box, and it then drives out. Record that
      residual-offset entry G1 was checked and dropped (thresholds 2.3/2.7 m, only the curb yield reaches
      them; App sweep clean) or, if 0.2 reproduced, what was decided. Also record the curb-clip side finding
      and where it is routed (Open question 3).

4.2 Full verification (orchestrator lesson: one traffic rule change moves every trajectory):
    - Windows: `cargo test -p gta_sim -p citygen` (full), `cargo clippy --workspace --all-targets -- -D
      warnings` (use the repo's clippy invocation from `.github/workflows/clippy.yml`),
      `cargo test -p gta_like --bin gta_like`, `python tools/qa/tree_check.py`.
    - `traffic_gridlock` seeds 1/2/7/42 within 40 s. `traffic_bench` / `police_bench` / `civilian_bench`
      under their `MEAN_LIMIT`: the new `connector_body` has ~10-45 rects per connector instead of 7, so
      report the mean against the Stage 0.3 baseline.
    - WSL Ubuntu 22.04, toolchain 1.95.0: the whole `gta_sim` + `citygen` suite in the mirror, with
      `maw/tasks/done/TASK-037/scratch/wsl/wsl_sync.sh` and `linux_run.sh` patterned into this task's
      scratch (`$HOME/gta036`). A Linux-only red is a real bug on another trajectory: trace it.
    - After merge: CI 5/5 on the merge commit.

## 4. Risk areas
- **B moves every city trajectory.** Wider grant shapes mean more "blocked by a body" waits, re-picks and
  whole-box grants. A city gate that goes red elsewhere is probably a latent lock the new trajectory
  reaches (TASK-037 lesson). Trace it before reverting. Watch `traffic_junction_box` seed 7 (TASK-039
  class E, ignored liveness) and `traffic_go_around`.
- **B, transient blocks.** An AI car just leaving the box onto the exit lane (OnPathTraffic, grant
  released) can overlap the nose-out part of the sweep for a few ticks. It only delays the grant: re-pick
  and demotion need a body standing ≥ `pass.vehicle_seconds`. If Stage 2.5 shows lease churn, report it;
  do not skip OnPathTraffic without evidence.
- **B, sampling.** Between two samples a corner can pass a thin body (the TASK-038 lesson). The guarantee is
  the `Footprints` oracle in the rows, not the step. Keep the corridor law, and do not coarsen it for
  performance without re-running B1-B3.
- **B2 after the fix.** X gets its grant first only if Y's wait "for a body on its path" does not block the
  node (`junction.rs:316-327`). If X's own path were blocked too, both would wait. That is the TASK-039
  class, not this row. Keep X's exit free in B2.
- **A, walker reach.** "Next to the car" at 1.3 m could let a walker on the far side take its next edge
  early. That is harmless: the next target is routed around the car. Two nodes under one car are possible:
  the car's 4.73 m diagonal exceeds the 4.5 m crossing spacing. The walker then passes both, and
  `wander_next` never returns `from` except at a dead end.
- **A, city row phase.** Walker counts are phase-sensitive (TASK-022 lesson). Gate the longest stall, not
  counts, and run it 3 times.
- **C, one tick of braking.** The step changes nothing else for a car still stepping into its yield. That
  car already used the decay law on the connector (seed 1, 8 m row, drove through).
- **Windows/Linux divergence (TASK-038).** Seed-specific connector ids and poses in B3/C2 are layout-only
  (citygen is deterministic across platforms per the golden hashes). Traffic motion is not. Rows use their
  own two cars with `max_cars = 0`, but civilians still walk: re-run in WSL.

## 5. Open questions
1. **Scope of the C lock fix.** The reproduced defect in C is a lock (a yielding car stops in the box and
   keeps its grant), not the G1 the rescope names. The plan fixes it here because the C fixture found it
   and it is the same entry mechanism. Options:
   - (a) fix here (Stage 3, one match arm plus 2 rows). **Recommended.**
   - (b) route to a new task and ship A/B only.
2. **If the Stage 0.2 sweep finds C-G1:** add a lane-end guard to the siren yield ("a car that cannot reach
   its yield offset and stop before its stop line drives on", mirroring the lane-start guard at
   `sirens.rs:77-81`)? It is a behaviour change: fewer cars pull over right before a junction, and the
   siren car follows them through. Recommended only on a red cell. Otherwise drop.
3. **Curb-yield entry clips city blocks** (static: up to 0.51 m on ~140 connectors per seed at a 3.0-3.25 m
   entry, `geometry_clip.txt`). It is visible (a car body over the sidewalk corner), not G1, and not in the
   rescope. Options:
   - (a) note in traffic.md and route to a follow-up task together with the lane-end guard of Q2, which
     removes it at the source. **Recommended.**
   - (b) build the guard now.
   The static 0.425 m street-yield figure (up to 0.14-0.20 m over the curb prism at corners) is the same
   class and existed before.
4. **TASK-037 QA F1's routing text ("walker-walker counter-flow") is refuted.** The QA report stays as it
   is (history). traffic.md records the actual cause. No action beyond 4.1.

children: 0 launched / 0 reported
