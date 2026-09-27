# PLAN_V2 — TASK-036 (rescoped): walker pile-up at a car, `connector_rects` overhang, residual lateral at connector entry

Reviewer: plan-reviewer-1. Base: PLAN.md at `d6a5351` (the only commit after `7e15814` is the plan itself; no
code changed). Binding scope is unchanged: TASK_FINAL "Orchestrator rescope" items A, B, C, plus Resolved
questions Q1-Q4 (Q1: fix the yield lock on a connector here; Q2: lane-end guard only if the Stage 0.2
sweep finds a G1; Q3: curb clip to the narrative-graph backlog; Q4: traffic.md records that "counter-flow"
is refuted).

## Disconfirmation (done before the review)

Counter-example I set out to find: **the new B grant shape (body swept along the connector from s 0) has a
rear cap of `half.z + margin/2` = 2.19 m behind the connector start. A holder's own lane queue head,
standing at its stop line behind it, sits in that cap. Then `body_blocked` sees a standing AI car on the
holder's path and demotes a holder that is only waiting (for walkers, for its leader) after the lease.**

Search result: **it holds.**
- `junction.rs:106-116`: `body_blocked` runs `connector_clear` for every occupant over its whole connector
  and marks it when the blocker has stood >= `pass.vehicle_seconds` (3 s). `connector_clear` skips only
  the requester, characters and AI cars granted at the node (`box_rules.rs:40-45`). The next queue head is
  not granted, so it is not skipped.
- `junction.rs:147-162`: a holder on its connector is demoted when `stale && body_blocked` (contested, and
  it has not moved for the 5 s lease). Today TASK-033 keeps a holder that waits for walkers on its lease,
  because walkers are not "bodies"; the rear cap would turn the car behind it into such a body.
- A follower's nose stops at `lane.stop` (`drive.rs:506-510`). Probe
  `scratch/probe/ws/probe/tests/rv1_rear_cap.rs`, output `scratch/rv1/rear_cap.txt`: on seeds 1/2/7/42,
  238-240 of ~580 lanes have their stop line within 2.18 m of the lane end (217-227 have
  `stop == length`: no crosswalk crosses them). The + test floor has `stop == length` on every lane
  (`graph.rs:268`, no sidewalk crossings).
- The old band had no end caps (`box_rules.rs:11-28`), so this demotion cannot happen on HEAD.

Consequence: PLAN.md step 2.1 ("centre poses along connector c from s 0 to its length" for every caller)
would break the TASK-033 lease rule on about 40 % of lanes. Fixed below: the sweep starts at the
requester's current position on the connector.

## 1. Review notes (issues in PLAN.md, with evidence)

**Major**

1. **B: the sweep from s 0 demotes waiting holders (above).** Evidence: `junction.rs:106-162`,
   `box_rules.rs:40-49`, `scratch/rv1/rear_cap.txt`. Fix: `connector_body(graph, c, from_s, half)` with
   `from_s` = the car's `s` when it stands on `c`, else 0. The rear cap then stays inside the requester's
   own body plus 0.15 m. A follower's nose is at least the IDM jam gap (2 m) behind its rear, so it is
   clear by about 1.85 m. AIM does the same thing: it simulates the vehicle's trajectory from its current
   state through the box, not a fixed shape over the whole box
   ([Dresner & Stone, JAIR 2008](https://www.cs.utexas.edu/~aim/papers/JAIR08-dresner.pdf)).
   `connector_clear` / `repick` need the start `s`. All three `junction.rs` call sites can read it from
   `snaps` through `index` (`:81-86`, built before the loops).
2. **B, gate precondition contradicts the fixed behaviour.** PLAN 2.3 has `GATE BROKEN ... or X is
   granted before Y leaves`. After the fix, Y is refused while X stands in its sweep (the waiter "for a
   body on its path" does not block the node, `junction.rs:316-327`). X is then granted first, so the
   precondition fires on the green code. The precondition must pin what the fixture sets up, not what the
   rule under test decides: **Y is queued before X** (Y's `(stamp, bits)` sorts first in the node's
   `waiters` after tick 1). Also, `Y.waiting = Some(0)` does not guarantee this: X gets `tick` stamped at
   its first update (`junction.rs:264`). If that tick is also 0, the sort key `(stamp, e.to_bits())`
   (`:297-299`) falls back to entity order, and that is the problem the mutation meant to remove. Set
   Y's stamp strictly below X's: run one tick, read X's stamp, set `Y.waiting = Some(x_stamp - 1)`, or
   spawn X after a first tick with Y already queued. Then assert the order.
3. **A, "one computation" of the target is wrong.** PLAN 1.1 has the caller compute `lane_target` once
   and pass it to both `arrive` and `around_cars`. But `arrive` replaces `*walker` when it fires
   (`civilian/mod.rs:445-448`), and `around_cars` must steer to the NEW edge's target (today `:540`
   recomputes it after `arrive`). Passing the old value steers one tick towards the covered node. Keep
   `lane_target` inside `arrive`. Only add `cars: &[CarRect]` and the two radii as parameters, and leave
   the `around_cars` call reading `lane_target(&graph, *walker, ..)` after `arrive`.
4. **A1 fixture needs `RoadOccupancy` to run.** `snapshot_road` runs only with `resource_exists::<TrafficGraph>`
   (`occupancy/mod.rs:207`) and only for bodies near the `Player` (`:134-145`). A civilians-only app has an
   empty occupancy, so `standing_cars` is empty and the walkers have no car to steer around. That still
   fails on HEAD, but for the wrong reason. Build A1 on `traffic_floor(..)` (the `traffic_pedestrian.rs`
   pattern) with its sidewalk graph, and add `GATE BROKEN` unless `RoadOccupancy::bodies()` holds the parked
   car with `standing > 0` after warm-up.

**Minor (wrong citations or facts; the implementer would trip on them)**

5. `lateral.rs:308-325` / `:273-276` do not exist (the file has 200 lines). `effective_lateral` is at
   `lateral.rs:47-60`, and `CORRIDOR_YAW_STEP_DEG` / `CORRIDOR_LATERAL_STEP` at `:11-12`.
6. `car_blocks` is **not** imported in `civilian/mod.rs` (`:20` imports `CarRect, around_cars` only). It is
   re-exported `pub(crate)` from `tactics/mod.rs:7-9`, so it has to be added to the import.
7. The PLAN §2 B text says "use the model the conflict table already uses", but it samples by the corridor
   law, not the table's `BODY_SAMPLE_STEP` 0.2 m (`graph.rs:108-110`). The corridor law is the better
   choice (on a right-turn pivot the table's 0.2 m step moves corners ~1 m; 7.5° steps move a 2.4 m
   half-diagonal ~0.31 m). The doc comments must name the corridor steps, not claim that the shape is
   identical to the table's.
8. PLAN 1.3 check says `civilian_city` / `witness_city` stay "unchanged". A changes when walkers take
   their next edge. `wander_next` and the idle roll draw `NpcRng` (`civilian/mod.rs:441-452`), so every
   later civilian roll shifts (the TASK-010 lesson). The requirement is "green", and a red is traced, not
   assumed. `witness_city` gates 94/100 seeds, so its margin is thin.

**Verified correct** (spot-checked against code): the A mechanism (target under the car's grown rect,
`around_cars` never returns `to`, `arrive` needs 0.5 m, `navigation.ron`); the B call sites
(`junction.rs:110, 225, 236, 284`); `BoxInputs` built only at `drive.rs:339-343`; `half.x` 1.2,
`conflict_margin` 0.3, so the band half width is 1.35; the C mechanism (`sirens.rs:44-46` returns `None` off a
lane before the Yield end check at `:54-76`; `drive.rs:397-400` yield stop; `drive.rs:540-546` lateral
not stepped on a connector for Yield; `drive.rs:513-523` zeroes lateral at the connector end;
`junction.rs:158-162` keeps a non-stuck holder). The C lock is in `scratch/c_fixture2.txt` (A on
`Connector(1091)` s 0.35, v 0, `Yield(3.25)`, granted, to the end of the run). The B G1 rows are in
`scratch/b_fixture.txt` (0.533 / 0.683 / 0.612 m). Helpers exist: `test_graph`, `spawn_civilian`
(`tests/common`), `park_car`, `sidewalk_at`, `traffic_floor` (`tests/traffic_support`).

## 2. Updated understanding

### A (walkers around a standing car): as PLAN §1 A, with two corrections
- `arrive` (`civilian/mod.rs:428-455`) computes `lane_target` itself. After it fires, the walker's
  target changes, and `around_cars` (`:538-544`) reads the new one.
- `standing_cars` (`:460-476`) keeps rect bodies with `standing > 0` within `avoid_distance + half
  diagonal`. The list comes from `RoadOccupancy`, which exists only with a `TrafficGraph` and near the player.
- `car_blocks(p, p, car, r)`: a degenerate segment. `car_entry` then has `q == 0` on both axes and returns
  `Some` iff `p` is inside `half + r` (`fire_line.rs:118-140`). That is the "point within r of the rect"
  test the plan wants.

### B (`connector_rects` overhang): as PLAN §1 B, plus the caller context
- `connector_clear` is used for three things, each with a different requester position: a holder anywhere
  on its source lane or connector (`:106-116`), a queue head on its lane or a car at `s <= 1` on its
  connector (`:223-252`), and every waiter, including demoted holders standing mid-connector (`:281-288`).
  So a sweep "from s 0" has a different meaning per caller. Only "from where the car is now" is right for
  all three.
- Stop lines: 217-227 lanes per seed end at the lane end (no crossing), and every + floor lane does.
- The exit-lane nose cap (2.19 m past the connector end) is new too. For grants, the room check already
  requires `(queued + 1) * spacing` (>= 6.08 m) behind the last AI car there (`:308-315`), so an AI car on
  the exit lane cannot be within 2.19 m at grant time. For holder demotion it matters only in a spillback
  (the exit-lane car stands within 2.19 m of the lane start for 3 s while the holder is stale and
  contested). That is rare and arguably correct. Watch it in 2.5 (below), do not pre-patch it.

### C (residual lateral at connector entry): PLAN §1 C is correct (citations fixed in note 5).

## 3. Revised approach

- **A: the arrival rule, not steering.** Unchanged in substance. A waypoint whose lane target lies inside a
  standing car (grown by `capsule_radius`) counts as reached once the walker is within `corner +
  arrive_radius` of that car. No new tuning value: `capsule_radius`, `arrive_radius` and `corner` are the
  values `around_cars` already gets. The rejected alternatives in PLAN §2 stand (the trace shows 0 of 6361
  stalled samples with a counter-flowing neighbour, and 0 stalls without the car).
- **B: the requester's swept body from its current position.** Body rects (`half` = `(half.x, half.z) +
  margin/2`) at centre poses along `c` from `from_s` to the connector length. `from_s` is the car's `s` on
  `c`, else 0. Sampling follows the corridor law (the `sweep_ahead` loop, `manoeuvre.rs:88-96`), with
  both ends included. No `body_sweep` source-lane or exit-lane centre poses (the task text warns about
  repick/stuck behaviour). The band model goes away.
- **C: a Yield ends on a connector** (`Manoeuvre::None`, offset decays by `effective_lateral`). Q1
  resolved: in scope. C-G1 (co-granted contact from a residual offset) is built only if the Stage 0.2 sweep
  goes red (Q2).

## 4. Revised steps

### Stage 0: baseline on the current code (no production change)

0.1 Re-run the planner probes on HEAD (`d6a5351`; code identical to `7e15814`). Reuse
    `scratch/probe/ws/probe` read-only (build with
    `CARGO_TARGET_DIR=D:/test-gta-like/target cargo test --offline --test <name> -- --nocapture`, debug
    profile) and record under `scratch/stage0/`: `walkers` (3 rows), `b_fixture` (4 rows), `c_fixture` and
    `c_fixture2` (all rows), `geometry_b`, `geometry c_threshold`, `rv1_rear_cap`.
    - Check: each §0 "reproduced" row reproduces (walkers stall 94/118 s; B G1 0.53/0.68/0.61 m and the
      + floor mutual wait; C lock at s 0.33-0.75). If a row does not reproduce, drop that item and append a
      `dead_end` to the log.

0.2 C G1 timing sweep (decides Q2). As PLAN 0.2: curb yields (offset = pitch, begun at lateral 0) on the
    worst 3 touching curb pairs from `c_threshold` on seeds 1 and 7. Yield start 6..12 m before the stop
    line in 0.5 m steps, B delay 0..96 ticks in 16-tick steps. G1 max depth per cell goes to
    `scratch/stage0/c_sweep.txt`. Any cell > 0.02 m = C-G1 reproduced. Run this sweep on HEAD **and again after
    Stage 3** (the C fix changes what a yielding car does in the box, so the post-fix sweep is the one that
    decides).

0.3 Side-effect baselines for B (per seed 1/2/7/42 on `traffic_gridlock`, read-only probe that `#[path]`s
    the gridlock fixture and diffs `TrafficIntersections` and `TrafficCar` each tick):
    - worst stand, every stand > 20 s;
    - whole-box grants (`whole` None→Some), demotions (occupant → waiter of the same car at a node),
      re-picks (`next` changes while on the source lane, or a connector switch at `s <= 1.0`);
    - **new:** per demotion, the blocker's class: same source lane behind the holder / exit lane / on the
      box / other. On HEAD the first two should be ~0. Stage 2.5 compares against this.
    - `traffic_bench`, `police_bench` and `civilian_bench` means.
    - Output: `scratch/stage0/b_side_effects_before.txt`.

### Stage 1: A (walker arrival at a covered waypoint)

1.1 `crates/gta_sim/src/civilian/mod.rs`.
    - In `civilian_fsm`, compute `let cars = standing_cars(road.bodies(), position.0, nav.avoid_distance);`
      once, before `arrive`, and pass `&cars` to both `arrive` and `around_cars`.
    - `arrive` gains `cars: &[CarRect]` and `(clearance, corner)`. It **keeps computing `lane_target`
      itself**. The `around_cars` call keeps reading `lane_target(&graph, *walker, ..)` after `arrive`,
      so a walker that just took its next edge steers to the new target in the same tick.
    - New helper `fn reached(position: Vec3, target: Vec3, cars: &[CarRect], clearance: f32, corner: f32,
      arrive_radius: f32) -> bool`: `flat_distance(position, target) <= arrive_radius` OR some car in
      `cars` has `car_blocks(target, target, car, clearance) && car_blocks(position, position, car, corner +
      arrive_radius)`. The same car must satisfy both conditions.
    - `clearance` = `loco.capsule_radius` (0.3), `corner` = `capsule_radius + nav.arrive_radius` (0.8), as
      at `:543-544`. No new tuning value.
    - The `corner + arrive_radius` reach (1.3 m): the implementer derives it again from the Stage 0 dump and names the
      worst stalled sample's distance to the car rect (PLAN names walker 2016 at 0.94 m).
    - Add `car_blocks` to the `crate::tactics` import (`:20`); it is not imported today.
    - `arrive`'s doc: one line saying a waypoint under a standing car is reached beside that car.
    - Check: `cargo test -p gta_sim --lib civilian`.

1.2 Unit rows in `civilian/mod.rs` `mod tests`, one per branch of `reached`, numbers worked through the helper:
    (a) target 0.4 m away, no car: reached. (b) 0.6 m, no car: not reached. (c) target inside a car,
    walker 1.0 m off its flank: reached. (d) target inside a car, walker 3 m off: not reached. (e) target
    0.5 m outside the car's grown rect, walker 1.0 m off the flank and 2 m from the target: not reached.
    (f) two cars: target under car 1, walker 1.0 m from car 2 only: not reached. This pins "the same car".

1.3 New gate file `crates/gta_sim/tests/walk_arrival.rs` (header doc: class, derivation, flip).
    - **Row A1 (floor, mechanism).** Build it on `traffic_floor(..)` (so `TrafficGraph` exists and
      `snapshot_road` runs), spawned player included as the other floor gates do, and then `test_graph` for
      the hub: centre node C and three 12 m arms, placed clear of `world/test_area.rs` and of the floor's
      lanes (check every fixture point, TASK-012 lesson). `park_car` with its centre 0.5 m from C.
      Preconditions (`GATE BROKEN`): the car's chassis is clear of test-area geometry, the car is in
      `RoadOccupancy::bodies()` with `standing > 0` after warm-up, and C's lane target is inside the car
      rect grown by `capsule_radius`.
      Spawn 6 calm civilians (two per arm, `GraphWalker { from: arm_end, to: C }`, t 0.2 / 0.6). Assert
      each walker's `GraphWalker.from` becomes C within `bound = (longest start-to-ring distance + half the
      ring perimeter) / walk_speed + slack`, ring perimeter `2·(2·(half.x + corner) + 2·(half.z + corner))`,
      slack from the first 3 green runs (report the spread). Class: correctness of the arrival rule. On
      HEAD it is RED because the walkers orbit.
    - **Row A2 (city, the QA scene).** As PLAN 1.3 A2: seed 1, player at the R1 stand point, a car parked
      at spot B with headings 0 and 90, 150 s, stall metric (Wander/Flee within 8 m, moved < 0.7 m in a
      10 s window). Assert the longest stall <= a bound derived from the control (0 walkers >= 10 s), the
      broken runs (70-118 s) and the fixed runs' spread (3 runs). Report all three next to the bound.
      `GATE BROKEN` if the car moved or is gone, or no walker ever targeted the covered node (found as the
      sidewalk node whose lane target is inside the car's grown footprint, not hard-coded).
      Print the stall list.
    - Flip: return `false` from the covered branch of `reached`. A1 and A2 go RED; restore, GREEN. Record
      it in the stage summary.
    - Check: `walk_arrival` 3 runs, then `traffic_pedestrian`, `civilians`, `civilian_city`,
      `witness_city` **green** (not "unchanged": NpcRng draw order moves, TASK-010). Trace any red to a
      walker/trajectory before touching bounds.

### Stage 2: B (grant check with the requester's swept body)

2.1 `crates/gta_sim/src/traffic/box_rules.rs`.
    - Replace `connector_rects` with `connector_body(graph, c, from_s: f32, half: Vec2) -> Vec<FlatRect>`:
      body rects (axis = `right_of(tangent)`, `half` = (across, along)) at centre poses along `c` from
      `from_s.clamp(0, length)` to `length`, both ends included. Sampling follows the corridor law (steps <=
      `CORRIDOR_LATERAL_STEP`, subdivided so path yaw between samples <= `CORRIDOR_YAW_STEP_DEG`, the
      `manoeuvre.rs:88-96` loop shape; both constants imported from `lateral`).
    - `connector_clear(road, graph, junction, c, from_s, requester, half: Vec2)` and
      `repick(.., from_s, .., half: Vec2)`: doc says "the requester's body driven on along the connector
      from where it stands (the conflict table's grown body, corridor steps)".
    - Delete `connector_rects` (its only user is `connector_clear`; grep confirms no other reference in
      `src/` or `tests/`).

2.2 `crates/gta_sim/src/traffic/junction.rs`.
    - `BoxInputs.half_width: f32` becomes `body: Vec2` ("car half extents grown by half the conflict margin,
      the conflict table's body"). `drive.rs:339-343`:
      `body: Vec2::new(half.x, half.z) + Vec2::splat(cfg.conflict_margin / 2.0)`.
    - A local helper `fn from_s(snap: &Snap, c: u32) -> f32` returns `snap.car.s` if
      `snap.car.segment == Segment::Connector(c)`, else 0.0.
      - `:110` (holders): `index.get(&e).map_or(0.0, |&k| from_s(&snaps[k], c))`.
      - `:225` / `:236` (queue head / `at_start`): `from_s(snap, c)`. For `repick`'s other exits of an
        `at_start` car, use the same `snap.car.s` (exits part by <= 0.42 m within `REPICK_WITHIN`, per the
        existing const doc).
      - `:284` (waiters): through `index`, as for holders.
    - Module doc (`:9-13`): one clause, "the requester's body swept on from where it stands".

2.3 Gate file `crates/gta_sim/tests/traffic_box_overhang.rs` (new).
    - Rows are the `b_fixture` scenes without the entity-order loop.
    - Named mutation for queue order: queue Y strictly before X. After one tick, read X's `waiting`
      stamp, set Y's `waiting` to a smaller value (or spawn X only after Y is queued).
      **Precondition** (`GATE BROKEN`): after the next tick, Y sorts before X in the node's `waiters`, or
      Y is an occupant. Do NOT use who gets the grant as a precondition: on the fixed code X is granted
      first by design (Y waits for a body on its path, which does not block the node).
    - Rows:
      - **B1** + floor (`traffic_occupancy.rs` geometry): c1 = the worst static pick (c1 5 at s 1.50, c2 2),
        Y at 6 m/s from lane s 2.5. HEAD: G1 0.533 m.
      - **B2** same geometry, Y from rest at its stop line. HEAD: mutual wait for >= 17.75 s.
      - **B3** seed 1 node 69 (c1 590 at s 1.47, c2 587), Y from rest, player at `sidewalk_at(hub, 25)` facing the
        box (camera mutation named, TASK-032 lesson), `bubble.max_cars = 0`, other traffic despawned.
        `GATE BROKEN` if c2 is not a right turn or does not conflict with c1. HEAD: G1 0.683 m.
      - **B4 (new, rear-cap regression guard)** + floor: holder H granted on a straight connector, halted on
        it at s ~1 by a dummy (character) on its exit crosswalk/lane for longer than the lease. Its lane
        follower F stands at its stop line (= lane end on the floor). A conflicting waiter W is queued so
        the lease is contested. Assert H is **not** demoted while only a character holds it (TASK-033
        rule) and that `Footprints` stays clean. Flip: use `from_s = 0` for holders; H is demoted
        (RED). This row pins review note 1.
    - Assertions B1-B3: `Footprints` clean. Both cars leave the box (rear past the connector end) within a
      bound derived from the first green run (distance / `turn_speed` + IDM start), with the numbers printed.
      Also assert, through `TrafficIntersections`, that Y is never in `occupants` while X stands in Y's
      sweep.
    - Flip: swap `connector_body` for the old band (half width `body.x`, no caps). B1/B3 go RED on G1, B2 on
      liveness. Restore, GREEN.
    - Check: 3 runs. File < 750 lines (if C rows go here too, count first).

2.4 `tests/traffic_junction_box.rs:137-152` `through` set: recompute with body rects along each connector
    (the production grow, a 0.1 m step is enough in the test), so the lease gate watches every holder the
    new rule can demote. Header note: stricter, not looser.

2.5 Side-effect measurement: re-run the 0.3 probe after 2.1-2.2 into
    `scratch/stage2/b_side_effects_after.txt`, as a before/after table per seed (stands, whole-box grants,
    demotions by blocker class, re-picks, bench means).
    - Expected: a few more waits for bodies and re-picks, no stand > 40 s, and ~0 demotions with a blocker
      "same source lane behind the holder".
    - Report "exit lane" demotions (spillback), do not patch them without a trace.
    - Any new stand > 20 s is traced by segment (TASK-037 `trace.rs` pattern) before anything changes.

### Stage 3: C (a yield ends in the box)

3.1 `crates/gta_sim/src/traffic/sirens.rs:44-46`:
    ```rust
    let Segment::Lane(l) = car.segment else {
        // A yield is a lane manoeuvre: in the box the car drives out, its offset decaying to the connector end.
        return matches!(car.manoeuvre, Manoeuvre::Yield { .. }).then_some((Manoeuvre::None, 0.0));
    };
    ```
    - `manoeuvre::plan` applies it the same tick (`manoeuvre.rs:202-211`). `holds_offset` is false for
      `None`, so `effective_lateral` keeps the decay it already used for the Yield (no pose jump).
      `drive.rs:513-523` zeroes `lateral` at the connector end. `v0` is no longer capped at `pass.speed`
      (`drive.rs:382`).
    - One tick of yield braking remains (the obstacle at `drive.rs:397-400` is evaluated before `plan`).
      Name it in the summary, do not patch it.
    - Update the module doc (`:1-2`) with one clause, and the `update` doc (`:29-30`: "the end of its
      current yield", now also in the box).

3.2 Gate rows (a "C" section in `traffic_box_overhang.rs` or in `police_sirens.rs`; both files < 750 lines).
    - **C1** + floor: A is granted on lane 0 → the straight, at 6 m/s, 1.0 m before its stop line. Named
      mutation: `Manoeuvre::Yield { siren: Entity::PLACEHOLDER, since: now, offset: slack }` at lateral 0.
      Assert A's rear leaves the connector within a derived bound and A holds no grant after that.
      HEAD: stands at s 0.75 for 30 s.
    - **C2** seed 1 curb scene (lane 437 → connector 1091, curb yield begun 10 m before the stop line): A
      leaves the box. HEAD: stands at s 0.35.
    - `Footprints` clean on both.
    - Flip: restore the plain early return; both RED. Restore, GREEN.

3.3 C-G1 decision: re-run the 0.2 sweep on the fixed code.
    - All cells clean: drop C-G1 and append a `dead_end` citing both sweeps and `geometry_c.txt`.
    - Any red: stop and report to the orchestrator (Q2: the lane-end guard is a siren-behaviour change).

### Stage 4: docs and full verification

4.1 `docs/architecture/traffic.md` (sections as PLAN 4.1):
    - lane graph: conflict-point reservation deferred, -43 % co-granted pairs as the known cost;
    - consumers: the rescope record (fire line and sight dropped as unification with no bug; walk avoidance on
      `RoadOccupancy` since TASK-037, plus the covered-waypoint arrival);
    - junction box: `connector_clear` sweeps the requester's body from where it stands, what that closed
      (rear-swing G1, holder/waiter mutual wait), and why it starts at the car (the lease rule of TASK-033);
    - walkers: the TASK-037 QA "counter-flow" hypothesis is refuted, the cause is an unreachable walk target
      under a standing car (Q4);
    - sirens: a yield ends in the box; the C-G1 result.
    Q3: the curb-clip observation goes to the `docs/narrative-graph.md` backlog, not a task.

4.2 Full verification:
    - Windows: `cargo test -p gta_sim -p citygen`, clippy exactly as `.github/workflows/clippy.yml` runs it,
      `cargo test -p gta_like --bin gta_like`, `python tools/qa/tree_check.py`.
    - `traffic_gridlock` seeds 1/2/7/42 within 40 s; `traffic_bench` / `police_bench` / `civilian_bench`
      under `MEAN_LIMIT`, compared with the 0.3 baseline (`connector_body` has ~10-45 rects per call against 7).
    - WSL Ubuntu 22.04, toolchain 1.95.0: the full `gta_sim` + `citygen` suite in the mirror, with the
      TASK-037 `wsl_sync.sh` / `linux_run.sh` patterned into this task's scratch. A Linux-only red is a
      real bug: trace it.
    - After merge: CI 5/5 on the merge commit.

## 5. Risk areas

- **B, grant shape start point (review note 1).** A sweep starting behind the requester catches its own lane
  follower on ~40 % of city lanes and every + floor lane, and demotes waiting holders. Guarded by the
  `from_s` rule and row B4. If any later change needs the full `body_sweep`, re-run B4 and the 2.5
  demotion classes.
- **B moves every city trajectory.** More "blocked by a body" waits and re-picks. A red elsewhere is
  usually a latent lock the new trajectory reaches (TASK-037). Trace it before reverting. Watch
  `traffic_junction_box` seed 7 and `traffic_go_around`.
- **B, exit-lane nose cap.** A spillback car standing within 2.19 m of the exit lane start can now demote
  a stale holder. The room check makes this rare for AI cars. Measure (2.5), do not pre-patch.
- **B, transient blocks.** A car just released onto the exit lane can overlap the nose cap for a few ticks.
  That delays a grant only; demotion and repick need 3 s standing.
- **B, sampling.** The corners can slip between samples (TASK-038). The guarantee is the `Footprints`
  oracle in B1-B4, not the step. Do not coarsen the sampling for performance without re-running them.
- **B2 after the fix.** If Y at its stop line also lies in X's sweep, both wait until one has stood 3 s.
  Then the earliest waiter gets the whole box (`junction.rs:316-325`) and goes around. The row's liveness bound must
  cover that path. Report which path it took.
- **A, NpcRng shift.** City civilian gates move without a bug (TASK-010). The requirement is green, and
  any red is traced.
- **A, reach 1.3 m.** A walker at the far end of the car may take the next edge early. That is harmless:
  `around_cars` routes the next target. Two nodes under one car: both are passed.
- **A2 phase sensitivity and platform divergence.** Gate the longest stall, run it 3 times, and re-run in
  WSL (TASK-022, TASK-038).
- **C, one tick of braking** remains, as named in 3.1.

## 6. Open questions

None new. Q1-Q4 are resolved in TASK_FINAL. Review notes 1-4 are corrections within the resolved scope,
not new scope.

children: 0 launched / 0 reported
