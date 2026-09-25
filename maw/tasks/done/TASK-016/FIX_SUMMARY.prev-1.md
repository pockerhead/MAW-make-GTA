# FIX_SUMMARY — TASK-016 fixer round 1 (GDD T15: traffic and police cars)

Branch `feature/t15-traffic`, commits `b46dacb` and `47072c0` (pushed). Input: `IMPL_REVIEW.md` (NEEDS_WORK)
plus the binding orchestrator note (items 1-4). Cost of error: HIGH for the box lock and the police FSM
(silent jams, a car flipping state every tick); police response time and pull-over are owner-visible.

## 0. Preflight

- Scratch read as a coverage map: `scratch/cr_probe` (reviewer probes), `scratch/flip_*.py`, `scratch/qa_*`.
  My own probes and logs are in `scratch/fixer/` and `scratch/cr_probe/tests/fix_*.rs`.
- The claim I checked first, as the one most likely to break correct code if applied verbatim: issue 3's fix
  ("test the first `(queued + 1)·spacing` m of `to_lane` with `shape_intersections`, excluding kinematic AI
  cars"). The diagnosis is right. The fix alone is not enough. With the room check in place, a car stuck at
  the stop line waiting for that room was still pushed into `blocking` (`junction.rs:128-129`, FCFS). Every
  movement that crosses its connector then stayed locked. My gate measured this directly: 2 cars crossed the
  stuck car's path in 100 s, against 6..12 once room-only waiters stop blocking. Applied verbatim, the fix
  would have moved "the whole box locked" from inside the box to the stop line.
- Other claims checked in the code: issue 7's `k == 2` is the candidate index (`seat.rs candidates`: left,
  right, roof), so that part is correct. Issue 2's "blocked counts from spawn at v = 0" is true
  (`car_route.rs:235`). Issue 8's "hold until a route exists" would not strand the car forever: `blocked`
  would dismount it after `blocked_seconds`. Per the orchestrator, a missing route now dismounts at once.

## 1. Fixed

**Issue 1 (A* goal radius loops blocks) and note 1.**
- `car_route.rs`: the radius is now `nearest + car.goal_margin`. The new data field is in `escalation.ron`
  (`goal_margin: 10.0`, validated > 0), and the inline `0.75` is gone.
- Found while re-running the gates: with the wider radius, a car that spawned past the goal point on a lane
  that qualified counted as "arrived" at once. It dismounted 44.7 m out, and `car_closes_in_and_dismounts`
  went RED. `find_lane_route` now takes `from_s`, so on the car's own start lane only the stretch ahead
  counts. The route end is also clamped to `half_length + s0` from both lane ends, so it never ends inside a
  box.

**Issue 2 and note 1.** In `escalation.ron`: `blocked_seconds: 2.0` and car `spawn_ring: (30.0, 60.0)`.
Spawns must still be hidden, so the ring is out of view.

Police response, seed 1, 2 stars, with the `police_city` setup. Probe:
`scratch/cr_probe/tests/police_response.rs`, rows `*_shipped`. Figures are first reach (unit sees the player
or is within the keep band) / first cop within 18 m, in seconds.

| spot | before (`318936a`, re-measured) | after (`47072c0`) |
|---|---|---|
| park (43 m from any lane) | 63.08 / 69.72 | 6.17 / 12.61 |
| hospital | 6.45 / 6.45 | 3.02 / 3.02 |
| plaza | 8.38 / 8.38 | 2.03 / 4.73 |

All three spots meet the "about 15 s" target. The GDD car table is unchanged.

**Issue 5 (window).** `police_city.rs` is back to `SPOT_TICKS` (40 s) for both variants, and
`CAR_SPOT_TICKS` is removed. The doc comment ("40 s") is true again. Every car-variant cop now reaches the
player in 2.0-9.6 s. Flip: the old radius `nearest.max(0.75 × dismount)` plus `blocked_seconds: 6.0` makes the
park row RED ("no cop reached the player", all 4 stuck at 42-75 m).

**Issue 3 and note 2a (room past the box).**
- `drive.rs` builds `lane_start_free`: a box the height of the chassis over the needed stretch of the
  destination lane, on the Vehicle layer, excluding this tick's AI cars (they are in the occupancy).
- `junction.rs` grants only when `room()` holds and that stretch is free. Abandoned, taken and police cars
  now count.
- A waiter denied only for room no longer blocks later conflicting connectors. Waiters denied for a conflict
  still block, so the FCFS anti-starvation rule is kept.
- New gate `traffic_intersection.rs abandoned_car_past_the_box_holds_no_car_inside`, 3 seeds × 6400 ticks.
  An abandoned car stands 1 m past the box in the south out lane. A car at the north stop line is bound for
  that lane. Traffic comes from the other approaches. Checks every tick: no AI car stands on a connector
  (`speed > 0`). Liveness: at least 4 cars cross the stuck car's path (measured 6..12, locked node 2). A
  `GATE BROKEN` check fires if the north car is not waiting for the blocked lane.
- Flips: `lane_start_free` always true → RED ("stands on connector 1", tick 1121). Room-denied waiters
  blocking again → RED (seed 1: 2 crossings).

**Issue 4 and note 2b (police cars never stop in a box).**
- `TrafficGraph::in_junction(p, margin)` checks the flat bounds of each node's connector curves.
- `drive_police_cars`: inside a box, a Respond car keeps at least `turn_speed`, and `next_car_state` never
  enters Dismounted there (new sense `in_junction`).
- Pull over: a car stopping below turn speed steers `car.pull_over` m (new data, 3.25, validated ≥ 0) to its
  right, when a chassis there (from 0.05 m up, so the raised sidewalk counts) is free of World and Vehicle.
  Measured on seed 1 at hospital, plaza, park and a parked-car avenue (`scratch/cr_probe/tests/fix_pullover.rs`):
  the lateral offset at dismount is 0-0.45 m, because parked cars and sidewalks usually block the spot, and it
  adds no contacts compared with `pull_over: 0`. On the open test floor the car overshoots to 4.7 m against a
  3.25 m target. This goes to the owner run.
- New gate `police_car_floor.rs police_car_never_stops_in_an_intersection`. A police car crosses the AC2 box
  at 8 m/s with the player on foot 13 m away. It never stands in the box for 32 ticks, and it dismounts
  outside (dismounted at 89 ticks, 16.3 m from the player). Flip: `in_box = false` → RED ("crew got out in
  the box", tick 47).

**Issue 7 and note 4 (roof).**
- `dismount` filters out `ROOF_EXIT`, a new `pub(crate) const` in `seat.rs` (a law: the candidate index).
- A cop left aboard gets out when a door clears. The retry runs while Dismounted and the player has not
  driven off.
- When nobody can get out at all, the transition to Dismounted is undone, so the car stays in Respond and
  retries next tick. Without this the car flip-flops, see section 1b.
- Gates in `police_car_floor.rs`:
  - `crew_never_gets_out_on_the_roof`: right door walled, so one cop gets out at the left door, the other stays
    aboard, and he gets out once the first is moved away.
  - `crew_waits_aboard_when_no_door_is_free`: both doors walled, so the car stays Respond with crew 2 for
    128 ticks, and a cop gets out once a wall is removed.
- Flips:
  - Roof allowed → RED (feet at 2.08).
  - No retry → RED ("never got out at the freed door").
  - Transition not undone → RED ("a car nobody can leave is Dismounted", tick 0).

**Issue 8 and note 4 (no route).**
- With no walk, the car's target is straight ahead at speed 0, never the goal through blocks.
- A failed A* search sets the `no_route` sense, and the crew dismounts where the car stands (once slow and
  not in a box).
- Gate `car_without_a_route_lets_its_crew_out`: two unconnected loops, the car on one and the player inside
  the other. The car dismounts within 16 ticks and never moves 1 m.
- Flips: no `no_route` rule → RED (dismount only via `blocked`, too late). Old
  `None => (goal_point, pursuit_speed)` → RED (moved 1.02 m toward the player at tick 42).

**Issue 6 (frozen crew near the park).** Not reproduced after the fixes: all 4 park cops reach the player in
6.2-9.6 s (`police_city`). No follow-up needed.

**Note 3 (intersection throughput).** Not changed. Recorded in `OPEN_DECISIONS.md` as an owner-run item.

### 1b. Defects found while verifying (t15 runtime failed twice before these)
- **Spawn direction.**
  - With the (30, 60) ring, t15 failed in 2 of 2 runs with "the police cars never closed in" (first = closest =
    31-33 m). With the old ring it passed.
  - A BRP trace (`scratch/fixer/t15_diag.py`, `diag30/`) showed the near car facing away and looping a
    4-lane block, going from 30 m to 133 m.
  - `dispatch_police_cars` now keeps only spawn points whose lane tangent points at `last_known`.
  - Gate, added to `cars_follow_row_1..5`: every new police car faces its target. Flip: filter removed → rows
    2-5 RED.
- **Re-board flip-flop.**
  - A car at its route end while the player drove more than `reboard_distance` away went Dismounted, and its
    crew re-boarded the same tick. That repeated every tick: 1132 transitions in 25 s in the headless chase
    probe `scratch/cr_probe/tests/fix_chase.rs`.
  - This predates this round; the (30, 60) ring made it common.
  - New sense `driven_off`: Respond never goes to Dismounted for a driver already gone.
  - Gate: new rows in `next_car_state_table`. Flip: rule removed → RED.
- **`car_closes_in_and_dismounts` re-anchored.**
  - With blocked 2 s, a car held behind a slow traffic car can now get out 52 m away first. That is the
    orchestrator's trade-off: a hidden foot spawn.
  - The gate now follows the first car that gets out within `dismount_distance + 6` and asserts that it
    closed in.
  - Flip: `route_done: true` (every car gets out at spawn) → RED ("no police car dismounted near the player").

## 2. Skipped

- **Minimal go-around (note 2c): STOPPED per the orchestrator's rule.** It is bigger than one bounded
  system plus data. It needs:
  - an opposite-lane relation in `TrafficGraph`;
  - per-car overtake state in `TrafficCar` (a stopped timer and a lateral offset);
  - a reservation of the oncoming stretch that three existing consumers must honour: oncoming IDM in
    `advance_traffic`, junction `room()` for the opposite lane, and the spawner.

  The risk is silent. Forward casts skip kinematic traffic because it is in the occupancy, so a kinematic
  overtaker the oncoming kinematic cars do not see makes them pass through each other: a kinematic pair gets
  no solver response. My estimate is about 150-200 sim lines plus the gate. Remaining effect: a lane stays
  blocked behind an abandoned car, or behind a Dismounted police car the pull-over could not move, until the
  bubble despawns it. Logged as a `dead_end` and in `OPEN_DECISIONS.md`.
- **Review nits.** Not done: the duplicate `clear` in `spawn.rs` and a comment in `car_dispatch.rs`. Both are
  cosmetic, and the code is correct.
- **Reviewer's optional change** "count `blocked` only after the car first exceeds exit speed". Not done: a
  car that spawned behind a queue would then wait forever. The orchestrator kept blocked from spawn (a hidden
  foot spawn).

## 3. Test results (all `-j 4`)

- `cargo test -p gta_sim --lib` and all 54 test targets, in three batches: **all ok**. Logs:
  `scratch/fixer/tests_final{1,2,3}.log` (23 + 15 + lib + 16 `test result: ok`).
  - `traffic_bench`: mean 1.77 ms, p95 2.22, max 2.99 per tick (was 1.86). GDD §11 allows ≤ 4 ms.
  - `traffic_intersection one_car_per_conflict_point` (AC2) is still green after the FCFS change, delivering
    4-9 cars per approach.
- `cargo clippy --workspace --all-targets -- -D warnings` and `cargo clippy -p gta_sim -p citygen
  --all-targets -- -D warnings`: clean.
- `cargo test -p gta_like --bin gta_like`, ×3 after the last sim change: 79 passed in each run.
- Runtime, release, via `tools/qa/brp.py`:
  - **t15 PASS** (`scratch/fixer/qa_t15/`, `qa_t15.log`): 19 traffic cars, mean 8.3 m/s, max 12.2; hijack
    with 1 fleeing driver; chase 38.0 → 4.6 m, at most 2 active cars; dismount with 4 `CrewOf` cops; frame
    cost with no vsync 3.3 ms. Two earlier runs of the same code state stopped at "Driving the traffic car not
    reached in 1.0 s": the hijack step, before any police. That is a timing flake of the script's 1 s poll
    (2 of 5 runs), noted for QA.
  - **t11 PASS** (`scratch/fixer/qa_t11/`, `qa_t11.log`): busted in 10.1 s, no log errors.
- CI for `b46dacb`: all 5 jobs green. For `189797a` (the code of `47072c0` plus this summary): all 5 jobs
  green (sim gates run 36105534810, 12m50s). The `47072c0` sim run was cancelled because the next push
  superseded it.

Flip log (sabotage → RED → restored → GREEN). All are scripted in `scratch/fixer/flip.py`:
`room_ai_only`, `fcfs_room_blocks`, `box_guard_off`, `roof_allowed`, `no_retry`, `no_door_flipflop`,
`no_route_rule_off`, `no_route_old_drive`, `driven_off_ignored`, `dismount_anywhere`, `no_direction_filter`,
`old_goal_radius`. Every one was RED.

## 4. Owner checklist additions
- Police cars pull over to the curb where they can. Look for the car climbing the sidewalk or brushing
  parked cars: the open-floor overshoot is 4.7 m. `car.pull_over: 0` turns it off.
- Response: cops at the park within about 12 s; on the hospital and plaza sidewalks within 3-5 s.
- Crossings: a car stuck behind a standing car past the box no longer freezes the whole intersection. Its
  own approach still waits (no go-around).

children: 0 launched / 0 reported.
