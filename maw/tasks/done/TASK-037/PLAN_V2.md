# PLAN_V2 — TASK-037: in-view box lock (Dynamic recovery on connectors, connector sensing, walkers around cars)

Reviewer: plan-reviewer-1. Base: PLAN.md at 85d08eb, TASK_FINAL.md (binding rescope + Q1-Q3 resolved).
Cost of error: silent (minutes-long gridlock in front of the player; G1 pass-through if a kinematic body
sits where the conflict table does not model it). Full evidence layer stays.

## 0. Disconfirmation (done before the review)

Counter-examples tested against the code, in order:

1. **"The in-band recovery rule (1.1) is degenerate on connectors from lanes with no `left_gap`"**
   (`recover.rs:99`: pitch = `left_gap.unwrap_or(2 * half.x)`, so the slack would be 0 and no real car
   could ever be "in band"). Probe: `scratch/probe/ws/probe/tests/pr1_band.rs`, output
   `scratch/pr1_band.txt`. **Did not hold**: 0 of 1488 (seed 1) and 0 of 1468 (seed 7) connectors start
   on a lane without a left gap; every traced connector gets slack 0.425 m. The probe also showed the
   real problem with the band: it is **0.425 m, while the conflict table models only 0.15 m**
   (`graph.rs:301-305`, `grown = half + conflict_margin / 2`). See R1 below.
2. **"Stage 3 breaks an existing gate that needs a walker pressing a car"**. **Held**:
   `tests/traffic_pedestrian.rs::a_walker_at_the_flank_does_not_hold_the_car` asserts
   `GATE BROKEN: the walker never pressed against the flank` (a production civilian walking its sidewalk
   run into the car flank for 64 ticks). `traffic_floor` inserts a `TrafficGraph` and a player, so
   `RoadOccupancy` fills and the walker with `around_cars` walks around the car: the gate goes RED on its
   own precondition. PLAN.md lists this file only as "stays green". See R3.
3. **"The new f-row flip (1.7) stays green"**. **Held** (by code reading): PLAN.md places the car "with
   its grant". A granted `Dynamic` car on a connector is not `held` (`drive.rs:437-447`), its autopilot
   drives it along the U connector (`drive.rs:459-471`), `reproject` moves it onto the exit lane
   (`drive.rs:154-157`) and the OLD rule recovers it there. The "never kinematic" flip is not
   falsifiable as written. See R2.

## 1. Review notes (issues in PLAN.md, with evidence)

Verified correct (kept): the cause table matches the traces in `scratch/trace/*.txt`
(g4_seed_7 2017v0 only `on_lane` false, coming [] and corridor clear; r1_seed_7 holders 1930v0 /
1927v0 kinematic at `LEFT_CAR gap 2.00` with sweep min gap 0.39 / 0.37; r1_seed_1 1853v0 LEFT_CAR at
0.06-0.14; c_node_141 2002v0 with 1919v1 at 0.23-0.34). Line references in `recover.rs`, `drive.rs`,
`manoeuvre.rs`, `junction.rs`, `box_rules.rs`, `lateral.rs` are right. The `holds_offset` change also
removes a hidden jump: today a car recovered on a connector gets `lateral = measured` but is posed at
`lateral * (1 - s/L)` (`lateral.rs:41-55`), a sideways teleport of `lateral * s/L` on the recovery tick.
The SUMO reference is right (`jamtime.crossing` default 10 s, a jammed pedestrian moves on at 1/4
speed through obstacles; https://sumo.dlr.de/docs/Simulation/Pedestrians.html).

**R1 (major, G1). The connector recovery band is wider than the conflict table.** PLAN.md 1.1 recovers
on a connector when `!off_lane` (slack `left_gap/2 - half.x` = 0.425 m) and argues "inside the band the
body sits where the table puts it". It does not: the table sweeps the body grown by `conflict_margin/2`
= 0.15 m (`graph.rs:303`), and pairs allowed to hold grants together are only guaranteed 0.3 m apart
(grown 0.15 each). A granted car recovered 0.4 m off its line can touch a car on a non-conflicting
granted connector, kinematic vs kinematic: the switch never fires between two kinematic bodies
(`contact.rs`: `if !rb.is_dynamic() return None`), so it is a pass-through. The `REPICK_WITHIN` 0.425 m
precedent (`junction.rs:51-55`) applies only within 1 m of the lane end, where the rear is still on
the lane. Fix: on a connector, recover only when the footprint fits the table's own body model at `s`
(reach across the path line <= `half.x + conflict_margin/2`, i.e. 1.35 m). The traced heads fit
(worst: 1853v0 lateral 0.02, yaw -1.8 deg -> 0.02 + 1.2 cos + 2.04 sin = 1.283 m). `off_lane` stays
the give-up rule, unchanged.

**R2 (major, gate). The stage-1 flip row is not falsifiable as specified** (disconfirmation 3). The car
must be held (grant released, like `e_off_path_car_with_nothing_ahead_gives_up`), the assertion must be
"Kinematic while `segment == Connector(0)` and `s` within 0.05 m of where it stood", and the shove must
be inside the new band: 0.2 m + 1.2 = 1.4 > 1.35 would now be out of band. Use 0.10 m (reach 1.30),
and add the out-of-band partner strictly on the failing side (0.25 m, reach 1.45).

**R3 (major, regression). Stage 3 turns `traffic_pedestrian::a_walker_at_the_flank_does_not_hold_the_car`
RED** (disconfirmation 2). The TASK-016 property it guards (the forward cast starts at the nose, so a
body at the flank does not hold the car) still needs a body pressing the flank. Re-anchor it on a body
the new avoidance does not steer (the player driven by `set_intent` into the flank, or a dummy placed
touching it), flip it RED again, and reuse the same floor for the new stage-3 civilian row (the run
already crosses the lane at the car flank). No new `civilian_cars.rs` file.

**R4 (medium, gate numbers). Stage 2 gap definition and the "on the curve" tolerance are not derived.**
PLAN.md: "gap = sample distance along the path minus half.z". If samples pose the car body (centre) at
`s + d_k`, the free travel before contact is between `d_{k-1}` and `d_k`, not `d_k - half.z` (that
would stop the car ~2 m early and the "min_gap +- 0.05" row goes RED on correct code). And with a
0.3 m step the gap is quantised to 0.3 m, so "+- 0.05" is not reachable without refinement. Fix: gap =
travel to first contact, refined by bisection between the last free and the first hit sample to
0.01 m (a few extra overlap tests, only on a hit); then the +- 0.05 tolerance holds.

**R5 (medium). The step-law derivation is copied to a use it does not cover.** The recover.rs comment
(2.37 m x sin 7.5 deg = 0.31 m) is a rotation about the car centre. Along a turn of radius r the far
corner moves `dtheta * sqrt((r + half.x)^2 + half.z^2)`: at r = 1.7 m (citygen right turns, TASK-038)
and 7.5 deg that is 0.46 m. What the path sweep needs is that no road body fits between two samples:
the thinnest body is a walker, 0.6 m across (`capsule_radius` 0.3), so a corner chord < 0.6 m cannot
skip a body entirely; the step only bounds the gap error, which the bisection removes. Move the two
constants to one place as PLAN.md says, but give the path use its own one-line derivation (walker
diameter), not the corridor's.

**R6 (medium, fixture). `box_scene` is shared by four rows.** PLAN.md 4.2 changes
`traffic_causes::box_scene`'s clearing for R1. It also feeds `rb_a_box_car_seen_from_nearby_stays`,
`rb_a_box_car_seen_from_afar_is_cleared` and `spot_c_queue_behind_a_box_car_is_never_given_up`. The
fixture defect (a body teleported 0.31 m in front of a moving car) is the same for all, so apply it to
all four, name it in the `box_scene` doc, and re-run all four (3x) with before/after numbers.

**R7 (medium, acceptance risk). Class D in (c) is predicted RED by arithmetic, not "maybe".** 1919v1
(given up, `Vehicle`, 0.23-0.34 m from 2002v0, inside `recover.skin` 0.4) blocks both `nobody_coming`
(at rest, relative sweep 0, grown own rect overlaps) and `corridor_clear` sample 0. The only thing that
removes it is the stuck cheat at `stuck_despawn_seconds` = 45 s out of frame. 2002v0 switched at
t 57.0, 1919v1 standing since about t 57 -> removed about t 102 -> 2002v0 `Dynamic` >= 45 + 1.5 s > 30 s.
PLAN.md's Q3-A recommendation ("falls to the stuck cheat") does not meet the bound. Whether 1919v1
exists at all after stage 3 is unknown (the trace does not say what switched it). So: stage 0 must
record 1919v1's switch cause, and the class-D decision goes to the orchestrator as an open question
now (Q4 below), not after stage 3.

**R8 (minor). Wrong data paths and GDD section.** Civilian/navigation data live in
`assets/npc/navigation.ron` and `assets/npc/civilian.ron` (not `assets/navigation/*.ron`,
`assets/civilian/*.ron`). The walker rule is a civilian behaviour: its one-line amendment goes to GDD
§6.2 "Мирные" (in Russian), not §5.2 Трафик.

**R9 (minor). Stage 2 claim filtering.** `first_along` ignores an oncoming claim the car is already
inside (`span.0 > 0`, query.rs). The new `first_in` over rectangles must keep that rule (skip a claim
that overlaps sample 0), else a car inside an oncoming passer's claim stops dead in the box.

**R10 (note, not a change). The lane arm of `sense` still looks straight into the box.** A granted car
at its lane end whose path turns away still sees a body straight ahead through the 25 m lane strip; if
that body is within `min_gap` of the lane end the car never reaches its connector. Not seen in any
trace (no granted lane car stands at `LEFT_CAR gap 2.00`); watch for it in the stage-2 re-trace, do not
build for it.

## 2. Updated understanding (corrections only; PLAN.md §1 otherwise holds)

- `off_lane` (`recover.rs:94-107`) band = half of `left_gap` of the lane (connector: its `from_lane`).
  On seeds 1/7 every traffic connector gets 0.425 m slack (probe). It is the give-up rule only;
  recovery never consulted it.
- The conflict table (`graph.rs:301-316`) = centre-line distance < `2*half.x + conflict_margin` or
  body sweeps (grown by `conflict_margin/2` = 0.15 m, 0.2 m samples) touching. `connector_clear`
  (`box_rules.rs:31-49`) skips requester, characters and AI cars granted at the node; an ungranted
  kinematic car in the box IS a blocking body for other grants (minus the known corner overhang,
  TASK-036 item 4).
- A `Dynamic` car on a connector keeps its grant (`junction.rs:158-162` releases only `stuck`); with a
  grant its autopilot drives it through (`drive.rs:459-471`); without one it is `held` (speed 0).
  After recovery it queues on its connector (`junction.rs:200-207`), may repick at `s <= 1.0`.
- `RoadOccupancy` is filled only with a `TrafficGraph` (`OccupancySystems.run_if(resource_exists::
  <TrafficGraph>)`) and only within `bubble.in_view.despawn + look_ahead` of the player (no player: no
  distance filter). `civilian_fsm` (`civilian/mod.rs:456-517`, file 517 lines) steers with
  `navigation::steer(position, lane_target(..))`, no avoidance. `tactics::around_cars(from, to, cars,
  clearance, corner)` and `CarRect::of(position, rotation, half)` already exist (`tactics/fire_line.rs`);
  no `From<FlatRect>` is needed (`CarRect { centre, axis, half }` maps 1:1 from `FlatRect` inline).
- `traffic_junction_box.rs`: the three `_liveness` rows re-run the same scenes as the three non-liveness
  rows with extra assertions (`check(.., liveness)`).
- Data: `assets/npc/navigation.ron` (`arrive_radius` 0.5, `avoid_distance` 2.0),
  `assets/character/locomotion.ron` (`capsule_radius` 0.3), `assets/traffic/traffic.ron`
  (`conflict_margin` 0.3, `switch` skin 0.1 / horizon 0.1, `recover` skin 0.4 / horizon 0.5 / seconds
  1.5 / give_up 10, `turn_sense_distance` 6, `lateral.rate_at_rest` 0.8, `idm.min_gap` 2.0,
  `bubble.stuck_despawn_seconds` 45).

## 3. Revised approach

Unchanged in shape (the binding rescope): build the shared rule "a bumped car returns to the kinematic
world wherever it stands, inside the model the conflict table keeps", then fix each existing rule the
trace shows misfiring, re-tracing after every stage. Changes against PLAN.md:

1. Stage 1 recovers on a connector inside the **table band** (`half.x + conflict_margin/2`), not the
   lane band. No new tuning value: `conflict_margin` is existing data.
2. Stage 2 senses along the path with body samples, **gap by bisection**, its own step derivation,
   oncoming-claim rule kept.
3. Stage 3 walkers go around cars (Q1/Q2 resolved), **with the flank gate re-anchored** and the
   civilian row built on that floor.
4. Stage 4 class D: the (c) outcome is predicted RED by arithmetic; the decision is asked now (Q4),
   the R1 teleport fixture fix (Q3) applies to all `box_scene` rows.

## 4. Revised steps

### Stage 0 — baseline and cause table (no code change)

0.1 On the branch point, run and store verbatim under `scratch/stage0/`:
`cargo test -p gta_sim --test traffic_junction_box -- --ignored --nocapture`,
`cargo test -p gta_sim --test traffic_causes -- --include-ignored --nocapture` (r1 rows + (c) + rb +
spot_c: their "reported" lines hold the off-approach `Dynamic` stands),
`cargo test -p gta_sim --test traffic_pedestrian -- --nocapture`.
0.2 Rerun the planner probe rows (`scratch/probe`). Extend it: (a) compute the table-band reach
(`|lateral| + half.x|cos| + half.z|sin|` against the path at `s`) for every `Dynamic` head on a
connector; (b) for (c), print 1919v1's switch cause and time and whether it is in frame; (c) for every
row, the longest walker stand at gap <= 0.05 m to a car. Keep the cause table in
`scratch/stage0/causes.md`.
Check: every red row's longest stand has a class (A-D) and a named blocker; a cause that differs from
the table goes to OPEN_DECISIONS before code.

### Stage 1 — recovery on connectors (class B)

1.1 `crates/gta_sim/src/traffic/recover.rs`: extract the reach computation of `off_lane` (`:100-105`)
into `fn reach_across(graph, snap, half) -> f32`; `off_lane` = `reach_across > pitch / 2` (behaviour
unchanged). Replace `:144` with
`let on_path = match snap.car.segment { Segment::Lane(_) => true, Segment::Connector(_) =>
reach_across(..) <= half.x + cfg.conflict_margin / 2.0 };` (`cfg.conflict_margin` is already in
`TrafficConfig`). Module doc `:1-6`: "on its path (on a connector only where the conflict table's body
model holds it)". Doc of the new condition: one line, why (the table sweeps bodies grown by half the
margin).
1.2 `manoeuvre.rs`: `pub(super) fn holds_offset(car) -> bool { passing(car) || car.manoeuvre ==
Manoeuvre::Rejoin }` beside `passing` (`:38-40`), one-line doc.
1.3 `drive.rs`: `:477` and `:538` `passing(car)` -> `holds_offset(car)` in `effective_lateral`; `:534`
`|| passing(car)` -> `|| holds_offset(car)`; `:511-515`: `if passing(car) { shift_pass(..) } else if
car.manoeuvre != Manoeuvre::Rejoin { car.lateral = 0.0; }`, comment `:509-510` updated (a rejoin still
offset carries on along the exit lane).
1.4 `manoeuvre.rs:59` and `:63`: `passing(car)` -> `holds_offset(car)`.
1.5 `lateral.rs:1-3`, `:39-40`: doc — decays on a connector unless the car passes or rejoins.
1.6 Side effect to measure, not design around: lane Rejoins and siren-yield Rejoins (`sirens.rs:74`)
that reach a connector now step. G1 rows of `traffic_recovery`, `traffic_go_around`,
`traffic_intersection`, `traffic_graph` stay clean.
1.7 Gates, `crates/gta_sim/tests/traffic_recovery.rs` (floor `two_way_street(70.0)`, U `Connector(0)`,
radius 1.625 m; the file is 468 lines, stays < 750):
- `f_connector_car_recovers_in_the_table_band`: car on `Connector(0)` at `s = 1.0`, `next = Some(0)`,
  `switch_by_hand`, **grant released** (as the e-row) so it is held, shoved 0.10 m right of the line
  at the path yaw. Compute in the test: `reach = 0.10 + half.x` and `limit = half.x +
  conflict_margin/2`; `GATE BROKEN` unless `reach < limit - 0.02`. Assert: `Kinematic` within
  `recover.seconds + 0.5 s` while `segment == Connector(0)` and `|s - s0| < 0.05`; the Rejoin ends in
  place (`|lateral| < 0.01`, `manoeuvre == None`) within `0.10 / rate_at_rest` (0.125 s) + 0.25 s of
  the recovery, still on `Connector(0)`; then it is granted (a waiter on an empty node) and reaches
  lane 1 within 5 s; G1 clean. Flip A: `on_path = matches!(.., Lane(_))` -> RED (stays `Dynamic`).
  Flip B: revert the `:534` change only -> RED (the Rejoin does not end on the connector).
- `f_connector_car_out_of_the_table_band_stays_dynamic`: same with a 0.25 m shove (reach 1.45, strictly
  past 1.35: `GATE BROKEN` unless `reach > limit + 0.05`): not `Kinematic` for `recover.seconds + 2 s`.
  Flip: `on_path = true` on connectors -> RED.
- `e_off_path_car_with_nothing_ahead_gives_up`: behaviour unchanged (1.5 m, 30 deg); reword doc and the
  `GATE BROKEN` message to "recovered out of the table band on a connector".
- `a_nudged_car_recovers`, `c_pressed_car_does_not_flip_flop`,
  `e_blocked_corridor_waits_in_the_queue_and_clear_corridor_recovers`: stay green.
1.8 Re-trace G4 seed 7 and the extra row. Expected: 2017v0 turns kinematic on `Connector(714)` s 0.43,
queues there, repicks (s <= 1.0) or waits for the whole-box pass. Record its `Dynamic` stand and every
head whose reach falls between 1.35 and 1.625 m (the residual R1 leaves; expected none).

### Stage 2 — connector sensing along the path (class C)

2.1 `manoeuvre.rs` `sense` (`:46-87`), connector arm only: replace the straight strip at the target
offset by body samples along the path: car body (`half.x`, `half.z`, not grown) posed with
`box_rules::path_pose` (make it `pub(super)`) at travel `d = 0, step, 2*step, .. <= turn_sense_distance`
past the current `s`, at the target offset (`right_of(tangent) * target`), yaw of the path tangent.
First sample that overlaps a body not dropped by `skip` (rule unchanged) or an oncoming claim (claim
direction against that sample's tangent, and not a claim already overlapping sample 0: R9) is the hit.
`gap` = travel to first contact, bisected between the last free and the first hit sample to 0.01 m
(sample 0 hit: gap 0). `speed_along` = body velocity along that sample's tangent; `standing`, `kind`,
`dynamic` from the body. Keep the second (current-offset) strip straight as today.
Add `RoadOccupancy::first_in(&self, rects: &[FlatRect], skip, claims_against: impl Fn(usize) -> Vec2)
-> Option<(usize, Hit)>` (or equivalent) in `occupancy/query.rs` next to `blocked`; `query.rs` is 382
lines.
Step: one shared place for the two recover constants (`lateral.rs`, `pub(super)`), keeping the
corridor derivation for the corridor; the path sweep uses 0.3 m of travel with its own one-line law
(R5: a turn of radius >= 1.7 m moves a corner < 0.6 m, the walker diameter, so no body fits between
two samples; the bisection sets the gap).
2.2 `plan_box_pass` unchanged (gets `hit.entity` from `sense`).
2.3 Gates (`traffic_intersection.rs`, 393 lines, or a new file if it would pass 750):
- "beside the curve": parked sleeping car whose footprint is >= 0.35 m outside the car's unmargined
  sweep of a 90-degree connector but inside the old straight 6 m strip from the entry (derive both in
  the test from `graph.pose` + `FlatRect`; `GATE BROKEN` otherwise). A granted kinematic car passes it
  without stopping (speed > 0 throughout the connector, exit lane within `length / turn_speed + 2 s`),
  `switches_by_cause` unchanged, G1 clean. Use a turn where the parked car is on the outside; record the
  closest approach and check it stays above `switch.skin` plus the straight-extrapolation drift
  (`(turn_speed * switch.horizon)^2 / (2r)`) so the predictive switch cannot fire on correct code.
- "on the curve": the same car moved onto the sweep: the car stops with nose-to-body travel gap within
  `min_gap +- 0.05` (derivable only because of the bisection).
- Flips: straight strip restored -> "beside" RED (stands at 2.00 m); bisection removed (gap = first hit
  sample travel) -> "on the curve" RED if the 0.3 m quantisation moves the rest point out of +- 0.05
  (compute the expected quantisation for the chosen placement first; if it lands within tolerance, pick
  an off-grid placement so it does not: gates lesson "include one off-grid start value").
- `traffic_bench` mean tick under `MEAN_LIMIT`, before/after recorded.
2.4 Re-trace R1 seed 7. Expected: 1930v0/1927v0 drive past the left car, approaches drain. If the next
blocker is `alone_in_box` or the lane-arm strip (R10), record it in OPEN_DECISIONS before touching it.

### Stage 3 — walkers go around car bodies (class A; Q1 = fold in, Q2 = around)

3.1 `civilian/mod.rs` `civilian_fsm`: add `road: Res<RoadOccupancy>`, `loco: Res<LocomotionConfig>`
(capsule radius). For Wander / Flee, before `steer`: `cars` = `Footprint::Rect` bodies of
`road.bodies()` (vehicles only) whose centre is within `nav.avoid_distance + half diagonal` of the
walker, mapped inline to `CarRect { centre, axis, half }`; target =
`around_cars(position, lane_target(..), &cars, radius, radius + nav.arrive_radius)` — the police arrest
values (`police/behavior.rs:407`). No new tuning number; if one proves necessary it goes into
`assets/npc/navigation.ron` or `assets/npc/civilian.ron` with a strict loader field.
3.2 API change: `civilian_fsm` is registered only through `CivilianPlugin` (grep confirmed: no test
registers it directly); `RoadOccupancy` is `init_resource`d by `OccupancyPlugin` in `compose_sim`
(`lib.rs:209`). Run `civilians`, `civilian_city`, `witness_city`, `street_spawn`, `crossing_run`,
`route_walk`, `police_*` that spawn civilians.
3.3 Gates, `crates/gta_sim/tests/traffic_pedestrian.rs` (225 lines):
- Re-anchor `a_walker_at_the_flank_does_not_hold_the_car` (R3): the pressing body is one the civilian
  avoidance does not steer (the player pushed into the flank via `set_intent`, or a dummy placed
  touching it), keeping the `pressed >= 64` precondition. Flip-RED again: start the forward cast at the
  car centre instead of the nose -> RED; record it.
- New `a_walker_goes_around_a_car_across_its_run`: the same floor and held car, the production
  civilian on the run: it reaches the far node within a bound derived from the run length plus the
  detour around the corner at `radius + arrive_radius`, divided by the wander speed from
  `assets/npc/civilian.ron` (+ 2 s), and never stands (speed < 0.1 m/s) within `radius + 0.1 m` of the
  car footprint longer than 0.5 s. Flip: empty car list -> RED (it presses until the timeout).
- City evidence for TASK-036 item 1 ("no civilian stands against a car longer than a derived bound"):
  from the stage-0 probe column (c), longest walker stand at gap <= 0.05 m before (90 s on G4 seed 1)
  and after; derive the bound from the fixed runs, record it; it is carried by the G4/R1 liveness rows,
  no separate city gate.
- `witness_city` (100 seeds, TASK-026) and civilian density gates stay green.
3.4 Re-trace G4 seed 1, extra, rb far, (c), R1 seed 1. Expected: heads recover within seconds of the
last walker leaving the skin; 2021v0's grant turns over. Count `SwitchCause::Character` switches near
the box before/after (walkers stepping around a nose into the lane). If they rise and create new
`Dynamic` heads, turn-back is the named fallback (Q2), only after two failures of that class.

### Stage 4 — class D (Q3 resolved for R1, Q4 open for (c))

4.1 R1 fixture (Q3): if the stage-0/3 trace confirms 1853v0's switch is the teleport artefact (body
placed 0.31 m in front of a car at 2.8 m/s), change `traffic_causes::box_scene` to wait until no
vehicle or character centre is within `2 * half.z + 2` of the hub before `park_car` (the G4 rule,
`traffic_junction_box.rs:99-118`), with a `GATE BROKEN` if no free moment within 5 s; name it in the
`box_scene` doc. It feeds r1 seeds 1/7, rb near, rb far and spot_c (R6): re-run all five rows 3x and
record before/after.
4.2 (c): apply the Q4 answer. If Q4 is "wait for stage 3", rerun (c) after stage 3; if 1919v1 still sits
inside `recover.skin` of 2002v0, the row is RED by the 45 s arithmetic (R7) and the chosen Q4 option is
built with its own flip row on the `traffic_recovery` floor.

### Stage 5 — restore the gates, regressions, docs

5.1 `traffic_junction_box.rs`: remove `#[ignore]` from the three `_liveness` rows; module doc `:12-19`
updated. (The non-liveness twins now repeat the same scenes; leave them, note the duplicate runtime in
the stage summary for the owner.)
5.2 `traffic_causes.rs`: un-ignore `r1_car_left_in_the_box_seed_{1,7}`, update the r1 doc; in (c) and
rb far replace `dynamic_bound_on(..)` with `clock.dynamic_violation()`; delete `dynamic_bound_on` if
unused; module doc updated.
5.3 Flip evidence for the restored rows: revert 1.1 -> G4 seed 7 liveness RED; revert 2.1 -> R1 seed 7
RED; revert 3.1 -> G4 seed 1 liveness RED. Record which input was perturbed.
5.4 Full suite: `cargo test -p gta_sim -p citygen` (incl. `traffic_gridlock` seeds 1/2/7/42 within
40 s), `cargo test -p gta_like --bin gta_like`, `cargo clippy --workspace --all-targets -- -D warnings`,
`python tools/qa/tree_check.py`, `traffic_bench` / `police_bench` under `MEAN_LIMIT`. Every touched
city gate reports G1 max depth (0). Touched gates 3x each (city scenes are phase-sensitive).
5.5 WSL (TASK-038 recipe, `maw/tasks/done/TASK-038/IMPL_SUMMARY.md:15-20`; write `scratch/wsl_sync.sh`,
`scratch/linux_run.sh`): `traffic_junction_box`, `traffic_causes`, `traffic_gridlock`,
`traffic_recovery`, `traffic_intersection`, `traffic_go_around`, `traffic_pedestrian`,
`witness_city`. A Linux-only red is a real bug.
5.6 Docs: `docs/architecture/traffic.md` "Modes" (connector recovery inside the table band, Rejoin
stepped on connectors), "One tick" item 5 (connector sensing along the path), "Junction box" (drop the
"Open (TASK-032 R1)" sentence, `:145`; state what resolves the in-view lock and what remains), a line on
civilians going around cars. GDD §6.2 (Russian, one line): walkers go around cars standing across their
way. Recovery and sensing are not player-visible: no §5.2 change.
5.7 Runtime R1 (QA): `maw/tasks/done/TASK-032/scratch/qa/run_r1.py`, spots A/B/C + control, branch and
main: spot A median <= main's 1 car over 30 s, B and C 0, control 0. Owner run (not gated): how the
recovered cars and the walkers around them look.

## 5. Risk areas

- **G1 from recovered kinematic cars in the box.** Closed to the table's own model by the table band
  (R1). Residual: a held `Dynamic` car 0.15-0.425 m off its connector line neither recovers nor gives up
  (same as today); stage 1.8 records whether any head lands there. The `connector_rects` corner overhang
  for an ungranted kinematic car in the box (TASK-036 item 4) now applies to more cars (recovered
  waiters); the G1 oracle in G4/R1/rb/(d) is the check.
- **Stage 2 changes every car's box sensing.** Fewer false stops; the sweep also sees curb walkers on
  the outside of right turns. Checks: `traffic_gridlock` (4 seeds), `traffic_pedestrian`,
  `traffic_bench`. Police cars keep their own sensing (`police/car_route.rs:540`).
- **Lane-arm straight strip into the box (R10).** Not seen; watch in the stage-2 re-trace.
- **Walkers around cars step into lanes** (new `Character` switches); measured in 3.4. A walker pinned
  between two cars gets `to` back from `around_cars` when every corner is blocked (`unwrap_or(to)`) and
  keeps pressing: expect it in the box with two bodies; the probe column (c) shows it.
- **(c) class D** is predicted RED unless 1919v1 disappears after stage 3 (R7, Q4).
- **Give-up timeouts**: none narrowed. Recovery on connectors takes cars off no give-up path (connector
  cars never gave up while led); list every city gate's stand and `Dynamic` bounds stage 0 vs after.
- **Rejoin on connectors steps instead of decaying** for lane/siren Rejoins that reach a connector;
  G1 rows are the check.
- **Cross-platform divergence**: every touched city gate on WSL (5.5).
- **Probes are not gates**: the planner and reviewer probes mirror `recover.rs` at 85d08eb; after stage 1
  they must use the new `on_path`. They stay in scratch.
- **File sizes**: `drive.rs` 638, `manoeuvre.rs` 182, `recover.rs` 167, `query.rs` 382,
  `civilian/mod.rs` 517, `traffic_recovery.rs` 468, `traffic_pedestrian.rs` 225,
  `traffic_intersection.rs` 393; check the 750 warning after each stage.

## 6. Open questions (for the orchestrator)

**Q4 (new, blocks stage 4 for (c)). A `Dynamic` car inside `recover.skin` of a standing non-walker body
that the stuck cheat takes 45 s to remove.** Numbers in R7: (c) is RED by >= 16 s even with every walker
gone.
- A (recommended first step, costs nothing): decide after stage 3's re-trace, since 1919v1's own switch
  may have been walker-caused and disappear with stage 3. If it persists, go to B or C.
- B: static-body hysteresis — a body at rest relative to the car (both standing) is tested with
  `switch.skin` (+ a data margin in `traffic.ron` `recover`) instead of `recover.skin`, in both
  `nobody_coming` and the corridor. Fixes 2002v0 (0.23-0.34 m > 0.1), not a car inside `switch.skin`
  (1853v0 at 0.06, handled by the Q3 fixture fix). Keeps the anti-flip-flop purpose for moving bodies.
- C: controlled back-off (the `Dynamic` car reverses along its path at crawl until the body is past
  `recover.skin`, rear corridor clear). New autopilot mode (`speed_throttle` never reverses today).
- D: accept (c) scoped to its approaches for class D only, with a pointer; contradicts the acceptance.
Two failures of the same class -> redesign note, then re-route / U-turn (the task's named fallback).

## Dependencies and environment

No new crate, no Bevy API beyond what the code already uses (`Res<RoadOccupancy>` is a project
resource). Probe crates in `scratch/probe` build offline with the workspace `Cargo.lock`, the same
`[patch]` entries and the shared `D:/test-gta-like/target`.
