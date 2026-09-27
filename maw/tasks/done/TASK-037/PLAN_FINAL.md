# PLAN_FINAL — TASK-037: in-view box lock (Dynamic recovery on connectors, connector sensing along the path, walkers around standing cars)

Reviewer: plan-reviewer-2. Base: HEAD a230ff1 (`PLAN.md` 85d08eb, `PLAN_V2.md` a230ff1), `TASK_FINAL.md`
(binding rescope, Q1-Q4 resolved). V2 corrections win over PLAN.md; PLAN.md detail restored where V2
dropped it without correcting it; this review's own corrections are marked **[PR2]** and listed in §5.

Cost of error: silent (minutes-long gridlock in front of the player; G1 pass-through if a kinematic
body sits where the conflict table does not model it). Full evidence layer: cause table, flip-RED per
mechanism, G1 in every touched city gate, WSL run, runtime R1.

## 0. Disconfirmation (done before the review)

Counter-example chosen as the most concrete way this plan is wrong: **stage 2's body-sample sweep
starts at travel 0 (the car's own current footprint) and, on a curve, poses the whole rigid body
ahead, so a body at the flank or at the outer rear of a car on a connector becomes a hit and holds
the car** — the TASK-016 property ("the forward cast starts at the nose: only bodies ahead of the
bumper stop a car", `tests/traffic_pedestrian.rs:1-11`) regressed on connectors, where no gate looks
(the flank gate runs on `Lane(0)`). **Held (by code reading + arithmetic):** V2 2.1 poses samples at
`d = 0, step, ..` with skip "rule unchanged" (`manoeuvre.rs:70-72`: only `me` and plain
`OnPathTraffic`), and "sample 0 hit: gap 0". A dummy 0.15 m off the outer rear flank of a car on the
`plus()` right turn (nominal r 1.625 m, Bezier minimum radius 0.707 x 1.625 = 1.15 m) is overlapped by
the body posed 0.3 m further on (rear flank 1.5 m behind the centre swings out about 1.5 x 0.3/1.4 -
0.03 = 0.29 m). Also held: V2's step law R5 is numerically false (below). Both corrected in §3 stage 2.

Second check (V2's step law, R5: "0.3 m of travel on a turn of radius >= 1.7 m moves a corner < 0.6 m"):
**held as wrong.** Far corner at `sqrt((r + half.x)^2 + half.z^2)` = 3.55 m from the turn centre at
r 1.7 → 0.3/1.7 x 3.55 = 0.63 m; connectors are quadratic Beziers (`graph.rs:189-243`) whose minimum
radius is 0.707 x the leg, and TASK-038 measured 0.83-0.98 m corner moves between 0.2 m samples.

Counter-examples that did NOT hold (kept from V2 as verified): the table band admits every traced
connector head (2017v0 reach 1.200, 2140v0 1.257, 1853v0 1.283 m, all < 1.35); `civilian_fsm` has 7
params, +2 stays far under Bevy's 16; no test registers `civilian_fsm` outside `CivilianPlugin`.

## 1. Cause table and understanding

### 1.1 Cause table (planner's stage-0 trace at 6f31688, `scratch/trace/*.txt`, verified by PR1/PR2)

Probe: `scratch/probe/ws/probe/tests/trace.rs` (build: `cd scratch/probe/ws/probe &&
CARGO_TARGET_DIR=D:/test-gta-like/target cargo test --offline --test trace -- --exact <row> --nocapture`;
`scratch/probe/assets` is a junction to `assets/`). Rows take 15-20 s each.

| Row | Head | Where | Switched by | Why it never recovers | Class |
|---|---|---|---|---|---|
| G4 seed 1 | 2109v0 | `Lane(294)` s 77.40/77.61 (lane end, crosswalk), no grant | walker, gap 0.28 | walkers pinned at gap 0.00 for 79 s (`nobody_coming` and corridor sample 0); led by the walker, no give-up | A |
| G4 seed 1 | 2021v0 | `Lane(299)` s 66.27/66.95, granted 727 | walker, gap 0.28 | walker pinned 90 s; grant kept (walkers excluded from `body_blocked`), east queue x 11-48 stands 100 s | A |
| G4 seed 7 | 2017v0 | `Connector(714)` s 0.43, no grant, not a waiter, lateral 0.00, yaw 0.0 | walker, gap 0.26 | **only `on_lane`**: `coming: []`, corridor clear | B |
| G4 extra seed 1 | 2140v0 | `Connector(720)` s 0.86-1.00, waiter, lateral 0.01-0.05 | walker, gap 0.28 | walkers pinned (up to 90 s) **and** `on_lane` | A+B |
| R1 seed 1 | 1853v0 | `Connector(726)` s 2.1, no grant, lateral 0.00-0.02, yaw -0.4..-1.8 deg | the left car, gap 0.31 (fixture teleport 0.31 m in front of a car at 2.8 m/s) | left car at 0.06-0.14 (inside both skins) **and** `on_lane`; walkers later | D (+A,B) |
| R1 seed 7 | none `Dynamic` | 1930v0 `Connector(709)` s 1.56, 1927v0 `Connector(717)` s 1.40, granted, kinematic | — | each stands at 2.00 m (IDM `s0`) behind the left car seen by the **straight** 6 m connector strip; the true sweep passes it at 0.39/0.37 m; `connector_clear` clear (no demotion), not `alone_in_box` (no box pass); 14 cars stand 128-146 s | C |
| rb far seed 1 | 1922v0 | switched on `Connector(719)` s 3.5, stands `Lane(249)` s 0.6 | walker, gap 0.25 | 3-7 walkers pinned up to 85 s | A |
| (c) node 141 | 2002v0 | `Lane(509)` s 99.9/107.35, later granted 1300 | `Dynamic` car 1919v1, gap 0.30 | walker pinned **and** 1919v1 (given up, `Vehicle`) at 0.23-0.34 m (inside `recover.skin` 0.4) | A+D |

Classes: **A** walkers pinned at a `Dynamic` head (mutual wait; SUMO resolves the same deadlock from the
pedestrian side, `jamtime.crossing` 10 s, https://sumo.dlr.de/docs/Simulation/Pedestrians.html,
https://github.com/eclipse-sumo/sumo/issues/5662); **B** `Dynamic` on a connector, nothing in the way;
**C** a kinematic holder held by a sensing false positive; **D** a `Dynamic` car inside `recover.skin`
of a standing non-walker body. Re-route/U-turn fix none of A-D (every head has a free exit or is held
by something other than the exit choice); they stay the named fallback after two failures of a class.

### 1.2 Code facts the steps rely on (all verified at a230ff1)

- `traffic/recover.rs` (167 lines): `recover_dynamic` `:131-167` needs upright, `at_rest`, **`on_lane`
  (`:144`, `Segment::Lane` only)**, `nobody_coming` (`:38-59`, dynamic bodies' relative sweep over
  `recover.horizon_seconds` vs the footprint grown by `recover.skin`) and `corridor_clear` (`:63-90`,
  current pose → line pose at the same `s`, `CORRIDOR_YAW_STEP_DEG` 7.5 / `CORRIDOR_LATERAL_STEP` 0.3,
  `:27-30`), for `recover.seconds`. Give-up `:156` only when `off_lane` (`:94-107`, reach across >
  half the pitch; connector: `from_lane`'s `left_gap`, 3.25 m on every traced connector → slack
  0.425 m, probe `scratch/pr1_band.txt`) and not `led` (`:111-128`).
- `traffic/drive.rs` (638): `:255-289` recovery pass; `:612-632` applies it (`RigidBody::Kinematic`,
  removes `Autopilot`/`DriveIntent`/`SleepingDisabled`, `lateral = measured`, `Rejoin`). Dynamic
  cars: `held` (`:437-447`, connector without grant/whole box) → autopilot speed 0; granted → drive
  through (`:459-471`). Kinematic motion `:475-569`: `before`/`lateral` via `effective_lateral(..,
  passing(car))` (`:477`, `:538`) decays any non-pass offset on a connector as `lateral * (1 - s/L)`
  (`lateral.rs:41-55`); stepping only `if manoeuvring && (Lane || passing)` (`:534`); connector end:
  non-passing `lateral = 0.0` (`:509-515`); heading while manoeuvring = `heading_yaw(tangent, v,
  (lateral - before)/dt)` turned at `yaw_rate_deg` 60 deg/s (`:546-551`; `heading_yaw` uses
  `max(v, 1)`, so at rest a 0.8 m/s sideways move aims 38.7 deg off the path); Rejoin ends at
  `|lateral| < 0.01` and path error < 1 deg (`:555-561`).
- `traffic/manoeuvre.rs` (182): `passing` `:38-40`; `sense` `:46-87` — connector arm: a **straight**
  strip from the nose (`origin = point + right*lateral + tangent*half.z`, `half_width = half.x`) of
  `turn_sense_distance` (6 m); `plain` skip of `OnPathTraffic` `:68-72`; second strip at the current
  offset while target ≠ current. `plan` `:99-182`: passes only for `idle` kinematic cars; box pass
  needs `alone_in_box` and a non-`Character` hit.
- `traffic/junction.rs`: `REPICK_WITHIN` 1.0 (`:51-55`); a kinematic car on a connector queues there,
  may repick at `s <= 1.0`; a `Dynamic` car on a connector is skipped (`:204-205`); holders on their
  connector are demoted only when `connector_clear` finds a body standing `pass.vehicle_seconds`.
- `traffic/box_rules.rs`: `connector_rects`/`connector_clear` `:13-49` (band `half.x +
  conflict_margin/2` = 1.35 m, no corner overhang); `path_pose` `:70-81` (private today).
- `traffic/graph.rs:301-316` conflict table: centre-line distance < `2*half.x + conflict_margin` or
  body sweeps grown by `conflict_margin/2` (0.15 m) touching. Connectors are quadratic Beziers,
  point on the polyline, tangent from the Bezier derivative (continuous) (`:189-243`).
- `occupancy/query.rs` (382): `Strip`, `Hit`, `first_along` (oncoming claim skipped when the car is
  already inside it: `span.0 > 0`, `:159-177`), `blocked`, private `overlaps` (shrinks both by 0.01).
  `occupancy/mod.rs`: bodies = every `Vehicle` (Rect) and `Character` (Circle, `capsule_radius`)
  within `bubble.in_view.despawn + look_ahead` of the player, `standing` = seconds at/below
  `hold_speed`; filled only when a `TrafficGraph` exists; `init_resource` in `OccupancyPlugin`,
  composed in `compose_sim` (`lib.rs:209`).
- `civilian/mod.rs` (517): `civilian_fsm` `:456-517` (7 params, `AiSystems::Decide`, registered only by
  `CivilianPlugin` `:260`) steers with `steer(position, lane_target(..))` (`:513`), no avoidance.
  `tactics/fire_line.rs:144-172` `around_cars(from, to, cars, clearance, corner)`; `CarRect { centre,
  axis, half }` (`pub(crate)`, same semantics as `FlatRect`: unit right axis, half (right, forward)).
  Police call it with `radius`, `radius + nav.arrive_radius` (`police/behavior.rs:407`).
- Data: `assets/traffic/traffic.ron` (`conflict_margin` 0.3, `turn_sense_distance` 6, `turn_speed` 6,
  `idm.min_gap` 2.0, `switch` skin 0.1 / horizon 0.1, `recover` skin 0.4 / horizon 0.5 / seconds 1.5 /
  give_up 10, `lateral` rate_at_rest 0.8 / slope 0.15 / yaw_rate_deg 60, `pass.vehicle_seconds` 3,
  `bubble.stuck_despawn_seconds` 45); `assets/vehicle/sedan.ron` (`chassis_half_extents` (1.2, 0.92,
  2.04), `hold_speed` 0.5); `assets/npc/navigation.ron` (`arrive_radius` 0.5, `keep_right` 0.5,
  `avoid_distance` 2.0); `assets/npc/civilian.ron` (`wander_gait: Walk`);
  `assets/character/locomotion.ron` (`walk_speed` 1.8, `capsule_radius` 0.3).
- Pinned: bevy 0.19.1, avian3d 0.7.0 (`Cargo.lock`). No new Bevy or crate API is used: only `Res<T>`
  of project resources.

## 2. Approach

Build the missing shared rule ("a bumped car returns to the kinematic world wherever it stands,
inside the model the conflict table keeps"), then fix each existing rule the trace shows misfiring,
in cause order, re-tracing after every stage. No new parallel box mechanism. Stage 0 baseline; stage 1
(B) connector recovery in the table band with the Rejoin stepped **and sliding** on connectors;
stage 2 (C) connector sensing along the path (swept body, nose rule, bisected gap); stage 3 (A)
walkers go around standing cars (Q1/Q2); stage 4 (D) R1 fixture fix (Q3) and, only if the re-trace
needs it, the at-rest skin for vehicles (Q4 option B); stage 5 restore gates, regressions, docs.

## 3. Steps

### Stage 0 — baseline and cause table (no code change)

0.1 On the branch point, run and store verbatim under `scratch/stage0/`:
- `cargo test -p gta_sim --test traffic_junction_box -- --include-ignored --nocapture`
- `cargo test -p gta_sim --test traffic_causes -- --include-ignored --nocapture` (r1 rows, (c), rb
  near/far, spot_c: their "reported" lines hold the off-approach `Dynamic` stands)
- `cargo test -p gta_sim --test traffic_pedestrian -- --nocapture`
- `cargo test -p gta_sim --test traffic_bench --test police_bench --test civilian_bench --release -- --nocapture`
  (record the means; `MEAN_LIMIT` 19 / 11 / 8 ms).
0.2 Rerun the planner probe rows and extend `trace.rs`: (a) the table-band reach
(`|lateral| + half.x|cos| + half.z|sin|` against the path at `s`) for every `Dynamic` head on a
connector; (b) for (c), 1919v1's switch cause, time and in-frame flag; (c) per row, the longest walker
stand at gap <= 0.05 m to a car; (d) for the flank scene of `traffic_pedestrian`, the minimum signed
distance walker capsule → car rectangle while pressed (penetration depth; needed by 3.3). Cause table
in `scratch/stage0/causes.md`.
Check: every red row's longest stand has a class (A-D) and a named blocker; a cause that differs from
§1.1 goes to `OPEN_DECISIONS.md` before any code.

### Stage 1 — recovery on connectors (class B)

1.1 `crates/gta_sim/src/traffic/recover.rs`:
- Extract the reach computation of `off_lane` (`:100-105`) into
  `fn reach_across(graph: &TrafficGraph, snap: &Snap, half: Vec2) -> f32` (reach of the footprint
  across the path line at `snap.car.s`); `off_lane` becomes `reach_across(..) > pitch / 2.0`
  (behaviour unchanged).
- Replace `:144` with
  `let on_path = match snap.car.segment { Segment::Lane(_) => true, Segment::Connector(_) => reach_across(graph, snap, half) <= half.x + cfg.conflict_margin / 2.0 };`
  and use `on_path` in `:145`. One-line doc on it: on a connector only where the conflict table's
  body model (half width + half the margin) holds the car. No new tuning value (`conflict_margin` is
  existing data in `TrafficConfig`).
- Module doc `:1-6`: "on its lane" → "on its path (on a connector only inside the conflict table's
  body band)".
1.2 `crates/gta_sim/src/traffic/manoeuvre.rs`: next to `passing` (`:38-40`) add
`pub(super) fn holds_offset(car: &TrafficCar) -> bool { passing(car) || car.manoeuvre == Manoeuvre::Rejoin }`
with a one-line doc (an offset the lateral law steers on a connector instead of decaying).
1.3 `crates/gta_sim/src/traffic/drive.rs`:
- `:477` and `:538`: `effective_lateral(.., passing(car))` → `holds_offset(car)`;
- `:534`: `(matches!(seg, Segment::Lane(_)) || passing(car))` → `|| holds_offset(car)`;
- `:509-515`: `if passing(car) { shift_pass(..) } else if car.manoeuvre != Manoeuvre::Rejoin { car.lateral = 0.0; }`;
  comment `:509-510`: a pass goes on along the exit lane, so does a rejoin still offset; any other
  offset has decayed to 0.
- **[PR2] `:546-551`: a Rejoin on a connector slides.** Compute the heading with rate 0 when
  `matches!(seg, Segment::Connector(_)) && car.manoeuvre == Manoeuvre::Rejoin` (the car keeps the path
  tangent while its offset steps), else as today. One-line comment: a yaw swing in the box takes the
  body out of the conflict table's band. Reason: at rest `heading_yaw` aims 38.7 deg off the path;
  a 0.10 m rejoin turns the car 7.5 deg in 0.125 s, reach 1.2 cos 7.5 + 2.04 sin 7.5 = 1.46 m > 1.35 m,
  so the band of 1.1 would not hold during the move.
1.4 `manoeuvre.rs:59` and `:63`: `passing(car)` → `holds_offset(car)` (a rejoining car on a connector
senses at its target 0 and at its current offset, like on a lane).
1.5 `crates/gta_sim/src/traffic/lateral.rs:1-3` and `:39-40`: docs — on a connector the offset decays
linearly to 0 at the connector end unless the car passes or rejoins.
1.6 Side effect to measure, not design around: lane Rejoins and siren-yield Rejoins (`sirens.rs:74`)
that reach a connector now step (and slide) there. G1 rows of `traffic_recovery`, `traffic_go_around`,
`traffic_intersection`, `traffic_graph` stay clean.
1.7 Gates, `crates/gta_sim/tests/traffic_recovery.rs` (468 lines; floor `two_way_street(70.0)` as the
e-row: lane 0 +X at z 30, U `Connector(0)` to lane 1, radius 1.625 m; not the loop, whose lanes have no
neighbour):
- `f_connector_car_recovers_in_the_table_band`: `spawn_traffic_car(Connector(0), 1.0, 0.0)`,
  `next = Some(0)`, `switch_by_hand`, grant released (`TrafficIntersections::release`, as the e-row),
  teleported 0.10 m right of the line at the path yaw (set `Rotation` and `Transform.rotation`). After
  one tick compute in the test `reach` (the `reach_across` formula from `Position`/`Rotation` against
  `graph.pose(Connector(0), project(..))`) and `limit = half.x + conflict_margin / 2`;
  `GATE BROKEN` unless `reach < limit - 0.02`. Assert: `Kinematic` within `recover.seconds + 1.0 s`
  of the shove while `segment == Connector(0)` and `|s - 1.0| < 0.05`; **every tick from recovery to
  the Rejoin end the measured reach stays <= limit** [PR2]; the Rejoin ends in place (`|lateral| < 0.01`,
  `manoeuvre == None`, still `Connector(0)`) within `|lateral at recovery| / rate_at_rest + 0.1 s`
  (derived: rate 0.8 m/s at rest, slide keeps path error ≈ 0); then it is granted (waiter on an empty
  node) and reaches lane 1 within 5 s (U length ≈ 5.1 m, IDM from rest ≈ 2.3 s); G1 oracle clean.
  Flip A: `on_path = matches!(.., Lane(_))` → RED (stays `Dynamic`). Flip B: revert only the `:534`
  change → RED (the Rejoin never ends while held on the connector). Flip C [PR2]: remove the slide →
  RED on the reach assertion (≈ 1.46 m).
- `f_connector_car_out_of_the_table_band_stays_dynamic`: same with a 0.25 m shove (`GATE BROKEN`
  unless `reach > limit + 0.05`, expected 1.45): not `Kinematic` for `recover.seconds + 2 s`. Flip:
  `on_path = true` on connectors → RED.
- `e_off_path_car_with_nothing_ahead_gives_up` (`:418-468`): behaviour unchanged (1.5 m, 30 deg);
  reword doc and the `GATE BROKEN` message to "recovered out of the table band on a connector".
- `a_nudged_car_recovers`, `c_pressed_car_does_not_flip_flop`,
  `e_blocked_corridor_waits_in_the_queue_and_clear_corridor_recovers`: unchanged, stay green.
- Module doc: add (f).
1.8 Re-trace G4 seed 7 and the extra row. Expected: 2017v0 kinematic on `Connector(714)` s 0.43,
queues there, repicks (`s <= 1.0`) or waits for the whole-box pass. Record its `Dynamic` stand and
every head whose reach falls between 1.35 and 1.625 m (the residual band: neither recovers nor gives
up, as today on every connector). Stop rule: if a gated row is red on a residual-band head, do not widen
the band (G1); record it in `OPEN_DECISIONS.md`.

### Stage 2 — connector sensing along the path (class C)

2.1 `crates/gta_sim/src/traffic/manoeuvre.rs` `sense` (`:46-87`), connector arm only; the lane arm and
the second (current-offset) strip stay straight as today.
- Samples: the car body (`half.x`, `half.z`, not grown) posed with `box_rules::path_pose` (make it
  `pub(super)`) at travel `d_0 = 0 < d_1 < .. <= turn_sense_distance` past the current `s`, at the target
  offset (`right_of(tangent_d) * target`), yaw of the path tangent at `s + d`. **[PR2] Step law = the
  recover corridor law:** move `CORRIDOR_YAW_STEP_DEG` / `CORRIDOR_LATERAL_STEP` from `recover.rs:27-30`
  to `lateral.rs` as `pub(super)` consts; per 0.3 m of travel, `n = ceil(path yaw change / 7.5 deg).max(1)`
  sub-steps of `0.3 / n`. Comment (one line each use): the corridor keeps the chord under the skin;
  the path sweep only bounds rotation + travel between samples (≤ 0.31 + 0.3 m), a body skipped
  between samples is caught on a later tick as the grid moves with the car, and the no-touch
  guarantee stays with the unmargined oracle (TASK-038 lesson). Do not claim a walker-diameter bound.
- **[PR2] Nose rule (TASK-016 kept):** a body counts only if some part of it lies ahead of the car's
  current nose line (nose = `point + right*target + tangent*half.z` at `s`, `fwd = tangent`): Circle
  `(c - nose)·fwd + radius > 0`; Rect `(c - nose)·fwd + half.x|axis·fwd| + half.y|axis.perp()·fwd| > 0`.
  Add it to the existing `skip` (`b.entity == me || (plain && OnPathTraffic) || !ahead_of_nose(b)`).
- Claims: an oncoming claim counts at sample k when `claim.dir · dir_k < 0`, except a claim that
  already overlaps sample 0 (the car is inside it: `first_along`'s rule, R9).
- New query in `crates/gta_sim/src/occupancy/query.rs` next to `blocked`:
  `pub fn first_in(&self, rects: &[FlatRect], dirs: &[Vec2], skip: impl Fn(&RoadBody) -> bool) -> Option<(usize, Hit)>`
  — the smallest index whose rect overlaps (the existing `overlaps`) a non-skipped body, or a claim
  against `dirs[k]` that does not overlap `rects[0]`; `Hit.speed_along = velocity · dirs[k]`,
  `standing`/`kind`/`dynamic`/`claim` from the body/claim, `gap` filled by the caller. Cull bodies once
  by centre distance to the car (sensing reach + the car's half diagonal + 4 m, as `first_along`).
- Gap: `k == 0` → 0; else bisect travel between `d_{k-1}` (free) and `d_k` (hit) with single-rect
  `first_in` calls until the interval is <= 0.01 m; `gap` = the free end. Hit entity = the one found at
  the hit end.
2.2 `crates/gta_sim/src/traffic/box_rules.rs` `plan_box_pass`: unchanged (takes `hit.entity` from
`sense`).
2.3 Gates in `crates/gta_sim/tests/traffic_intersection.rs` (393 lines; reuse its private `plus()`:
half box 3.25, lane offset 1.625, right-turn nominal radius 1.625 m; move to a new file only if it would
pass 750). Place the car with the `car_left_on_a_connector` pattern (`traffic_causes.rs:333-367`: spawn on the
connector at `s0 = 0.5`, speed 0, `next = Some(c)`, push `(c, car)` into the node's occupants). Every placement is derived in the test from
`graph.pose` + `FlatRect`/`obb_overlap` with `GATE BROKEN` when it does not hold.
- `a_car_beside_the_curve_does_not_stop_a_turn`: a parked sleeping car (`park_car`) on the outside of a
  right turn, footprint >= 0.35 m outside the unmargined body sweep of that connector (fine 0.05 m
  sampling in the test) and inside the old straight strip at `s0` (nose, tangent, `half.x`, 6 m).
  The granted car starts from rest at `s0`; once moving its speed never returns to 0 on the
  connector; it reaches the exit lane within `length / turn_speed + 2 s`; `switches_by_cause`
  unchanged; G1 clean; the closest approach body-to-parked car > `switch.skin + (turn_speed *
  switch.horizon_seconds)^2 / (2 * 1.15)` (straight-extrapolation drift at the Bezier minimum radius:
  0.1 + 0.16 = 0.26 < 0.35). Flip: straight strip restored → RED (stands at 2.00 m).
- `a_car_on_the_curve_stops_at_the_jam_gap`: named fixture mutation `set_traffic(|c|
  c.pass.vehicle_seconds = 1000.0)` (no demotion, no box pass during the measurement); the parked car
  on the sweep 3.5 m of travel ahead of the car at rest at `s0`. When the car has stood 0.5 s, the
  test's own oracle measures the travel gap: body posed along `graph.pose` in 0.01 m steps to the first
  `obb_overlap` with the parked footprint. Expected `min_gap ± tol`; **derive `tol` from two runs** (fixed
  code and bisection removed, i.e. `gap = d_k`: the rest gap then sits ≈ 0.2 m short); `tol` must leave
  >= 0.05 m on both sides, else record that the 0.3 m quantisation is not gated and drop the flip claim.
- **[PR2] `a_body_at_the_outer_rear_flank_does_not_hold_a_turn`**: the granted car at rest at `s0`
  of the right turn, a dummy 0.15 m off its outer flank 1.5 m behind the centre (`GATE BROKEN` unless
  the dummy has no part ahead of the nose line and some body sample within the first 0.6 m of travel
  overlaps it). Assert the car moves >= 1.5 m along the path within 3 s. Flip: drop the nose rule →
  RED (held by the rear swing). Record whether the dummy switched it to `Dynamic` (allowed).
- `traffic_bench` mean tick under `MEAN_LIMIT` (19 ms); before/after recorded.
2.4 Re-trace R1 seed 7. Expected: 1930v0/1927v0 drive past the left car, approaches drain. If the next
blocker is `alone_in_box` or the lane-arm strip into the box (V2 R10: not seen in any trace), record it
in `OPEN_DECISIONS.md` before touching it.

### Stage 3 — walkers go around standing cars (class A; Q1 fold in, Q2 around)

3.1 `crates/gta_sim/src/civilian/mod.rs`:
- New pure fn `fn standing_cars(bodies: &[RoadBody], at: Vec3, reach: f32) -> Vec<CarRect>`: bodies
  with `Footprint::Rect(r)` **and `standing > 0.0` [PR2]** whose centre is within `reach +
  r.half.length()` of `at`, mapped inline to `CarRect { centre: r.centre, axis: r.axis, half: r.half }`.
  Reason for the standing filter: the lock is walkers pressed against standing cars; steering at the
  corners of a moving car is new behaviour nobody asked for and can aim a walker at a moving car's
  front corner.
- `civilian_fsm`: add `road: Res<RoadOccupancy>`, `loco: Res<LocomotionConfig>`. For Wander/Flee, the
  steer target becomes `around_cars(position.0, lane_target(&graph, *walker, nav.keep_right),
  &standing_cars(road.bodies(), position.0, nav.avoid_distance), loco.capsule_radius,
  loco.capsule_radius + nav.arrive_radius)` (the police arrest values, `police/behavior.rs:407`).
  `arrive` keeps using `lane_target` (unchanged). No new tuning number; if one proves necessary it goes
  into `assets/npc/navigation.ron` or `assets/npc/civilian.ron` with a strict loader field.
- Unit test in the same file: `standing_cars` keeps a standing rect within reach, drops a moving rect,
  a circle and a far rect. Flip: remove the `standing > 0.0` filter → RED.
- File stays < 750 (517 + ~60).
3.2 API change check: `civilian_fsm` is registered only through `CivilianPlugin` (`civilian/mod.rs:260`);
`RoadOccupancy` is `init_resource`d by `OccupancyPlugin` in `compose_sim` (`lib.rs:209`); floors without a
`TrafficGraph` never fill it (no avoidance there, unchanged). Run `civilians`, `civilian_city`,
`witness_city`, `street_spawn`, `crossing_run`, `route_walk`, `civilian_bench`, `police_*` that spawn
civilians.
3.3 Gates, `crates/gta_sim/tests/traffic_pedestrian.rs` (225 lines):
- Re-anchor `a_walker_at_the_flank_does_not_hold_the_car` (V2 R3: the civilian now walks around the
  held car and the `pressed >= 64` precondition goes RED). The pressing body is **the player** [PR2]
  (not a civilian, so no avoidance): `place_player` beside the flank at the old QA offset (0.9 m ahead
  of the centre, left side) and `set_intent` walking into the flank for the whole press. Precondition
  (`GATE BROKEN`): for >= 64 consecutive ticks the player's centre is < `half.x + capsule_radius - 0.02`
  across the car axis (strictly inside the nose strip's band, so the flip below can fire) and behind
  the nose line. Keep "the car drives > 3 m in 5 s after the dummy ahead is despawned" and the health
  assertion (the player's). Flip: forward cast from the car centre (strip origin `point + right*lateral`,
  length + `half.z`) → RED; record it. If stage 0.2 (d) shows the press never reaches 0.02 m inside,
  record that finding and use the sabotage "strip from the centre with `half_width = half.x +
  capsule_radius`" instead; a placed dummy is not used (it rests at 1.55 m across, outside the 1.5 m band:
  the flip could not fire).
- New `a_walker_goes_around_a_car_across_its_run`: the same floor (lane 0 +X at z 30, the car held at
  x -12.30 by the dummy at x -7.96, the run at x -11.15 from z 22 to 38), the production civilian on
  edge 0 → 1 after the car stands. Bound: path via the nose corners at `radius + arrive_radius` = 0.8 m
  out (x -9.46, z 28 then 32) ≈ 16.8 m vs 16 m straight → 16.8 / `walk_speed` 1.8 + 2 s = 11.3 s to arrive
  within `arrive_radius` of node 1 (recompute from `keep_right` side and the real start in the test,
  print it). It never stands (speed < 0.1 m/s) within `capsule_radius + 0.1` of the car footprint for
  more than 0.5 s. Flip: empty car list → RED (presses until the timeout).
- `a_walker_ahead_of_the_bumper_still_holds_the_car`: unchanged (dummy), stays green.
- City evidence for TASK-036 item 1 ("no civilian stands against a car longer than a derived bound"):
  stage-0.2 column (c) before (90 s on G4 seed 1) and after; derive and record the bound from the fixed
  runs; carried by the G4/R1 liveness rows, no separate city gate.
- `witness_city` (100 seeds) and civilian density gates stay green; `civilian_bench` under 8 ms.
3.4 Re-trace G4 seed 1, extra, rb far, (c), R1 seed 1. Expected: heads recover within seconds of the
last walker leaving `recover.skin`; 2021v0's grant turns over. Count `SwitchCause::Character`
switches near the box before/after (`TrafficStats::switches_by_cause`). If they rise and create new
`Dynamic` heads: turn-back is the named fallback (Q2), only after two failures of that class.

### Stage 4 — class D (Q3 for R1, Q4 for (c))

4.1 R1 fixture (Q3): if the stage-0/3 trace confirms 1853v0's switch is the teleport artefact (a body
placed 0.31 m in front of a car at 2.8 m/s), change `traffic_causes::box_scene` (`:421-445`): replace
`clear_spot(hub, 6.0)` with the G4 rule (`traffic_junction_box.rs:99-118`): tick until no `Vehicle` or
`Character` centre is within `2 * half.z + 2` of the hub, `GATE BROKEN` if no free moment within 5 s;
then `park_car`. Name the mutation and its reason in the `box_scene` doc. It feeds r1 seeds 1/7, rb near,
rb far and spot_c (V2 R6): re-run all five rows 3x, record before/after. If D persists after that: the
two-failures rule, re-route/U-turn is the named fallback.
4.2 (c) (Q4 resolved: re-trace after stage 3 first). If 1919v1 is gone or outside `recover.skin` of
2002v0, build nothing and record it. Else build option B **[PR2 derivation]**:
- `recover.rs`: `rest_skin = cfg.switch.skin + cfg.lateral.rate_at_rest * cfg.switch.horizon_seconds`
  (0.1 + 0.08 = 0.18 m; derived: a body at rest cannot close a gap within the switch horizon, and the
  car's own rejoin moves it at most `rate_at_rest * horizon` towards it, so a recovered car is not
  re-switched at once). Applies to **vehicle bodies at rest** (`Footprint::Rect`, `standing > 0.0`)
  in both `nobody_coming` and `corridor_clear`; characters keep `recover.skin` (a standing walker
  reaches walk speed within ticks: the anti-flip-flop purpose). Corridor: per sample two `blocked`
  queries (grown by `recover.skin` skipping resting vehicles; grown by `rest_skin` for resting vehicles
  only, `ClaimFilter::None`). `config.rs:259-270` validation: add `rest_skin < recover.skin`. No new data.
- Gate rows in `traffic_recovery.rs`: a `Dynamic` car on lane 0 of `two_way_street` on its line, a
  parked sleeping car 0.25 m beside it (strictly between 0.18 + 0.05 and 0.4 - 0.05), nothing else:
  `Kinematic` within `recover.seconds + 1 s`, no re-switch for 2 s, G1 clean; flip: `rest_skin =
  recover.skin` → RED. Second row: a standing dummy at 0.25 m instead → never `Kinematic` for
  `recover.seconds + 2 s`; flip: apply `rest_skin` to characters → RED.

### Stage 5 — restore the gates, regressions, docs

5.1 `crates/gta_sim/tests/traffic_junction_box.rs`: remove `#[ignore]` from the three `_liveness` rows
(`:302-318`); module doc `:12-19` (no longer "ignored: the in-view box lock is open"); the
non-liveness arm `Some(dynamic) => eprintln!("seed {seed} (reported, TASK-037): ..")` (`:281`) gets the
label "(asserted in the _liveness twin)". Keep the non-liveness twins; note their duplicate runtime in
the stage summary for the owner.
5.2 `crates/gta_sim/tests/traffic_causes.rs`: remove `#[ignore]` from `r1_car_left_in_the_box_seed_{1,7}`
(`:501-511`), update the r1 doc (`:484-487`); in `c_character_in_the_lane` (`:294`) and
`rb_a_box_car_seen_from_afar_is_cleared` (`:531`) replace `dynamic_bound_on(..)` with
`clock.dynamic_violation()` (city-wide); delete `dynamic_bound_on` (`:305-328`) when unused; module doc
`:9-11`, `:18-24`.
5.3 Flip evidence for the restored rows: revert 1.1 → G4 seed 7 liveness RED; revert 2.1 → R1 seed 7
RED; revert 3.1 → G4 seed 1 liveness RED; revert 4.1 (if built) → R1 seed 1 RED; revert 4.2 (if built) →
(c) RED. Record which input was perturbed.
5.4 Full suite: `cargo test -p gta_sim -p citygen` (incl. `traffic_gridlock` seeds 1/2/7/42 within 40 s),
`cargo test -p gta_like --bin gta_like`, `cargo clippy --workspace --all-targets -- -D warnings`,
`python tools/qa/tree_check.py`, `traffic_bench` / `police_bench` / `civilian_bench` under
`MEAN_LIMIT` (release, before/after). Every touched city gate reports G1 max depth (0). Touched city
gates 3x each (phase-sensitive).
5.5 WSL (TASK-038 recipe, `maw/tasks/done/TASK-038/IMPL_SUMMARY.md:15-20`: sources synced to the WSL
filesystem, `CARGO_TARGET_DIR` there, Ubuntu-22.04, toolchain 1.95.0, `-j 2`; write
`scratch/wsl_sync.sh`, `scratch/linux_run.sh`): `traffic_junction_box`, `traffic_causes`,
`traffic_gridlock`, `traffic_recovery`, `traffic_intersection`, `traffic_go_around`,
`traffic_pedestrian`, `witness_city`. A Linux-only red is a real bug.
5.6 Docs: `docs/architecture/traffic.md` "Modes" (`:58-83`: connector recovery inside the table band,
Rejoin stepped and sliding on connectors), "One tick" item 5 (`:84-120`: connector sensing along the
path, nose rule, bisected gap), "Junction box" (`:136-148`: drop the "Open (TASK-032 R1)" sentence
at `:145`, state what resolves the in-view lock and what remains: the residual band, the
`connector_rects` overhang of TASK-036 item 4), a line on civilians going around standing cars. GDD
§6.2 "Мирные" (`docs/design/GDD.md:274`, Russian, one line): "Пешеход обходит машину, стоящую у него на
пути." Recovery and sensing are not player-visible: no §5.2 change.
5.7 Runtime R1 (QA): `maw/tasks/done/TASK-032/scratch/qa/run_r1.py`, spots A/B/C + control, branch and
main: spot A median <= main's 1 car over 30 s, B and C 0, control 0. Owner run (not gated): how the
recovered cars, the sliding rejoin and the walkers around cars look.

## 4. Test plan (summary)

| Gate | Class | Proves | Flip |
|---|---|---|---|
| `traffic_recovery::f_connector_car_recovers_in_the_table_band` | correctness | connector recovery in band; Rejoin ends in place; body stays in the table band while rejoining | A `on_path` lane-only; B revert `:534`; C remove slide |
| `traffic_recovery::f_connector_car_out_of_the_table_band_stays_dynamic` | correctness | band boundary (G1 side) | `on_path = true` on connectors |
| `traffic_intersection::a_car_beside_the_curve_does_not_stop_a_turn` | correctness | no false positive beside the curve | straight strip restored |
| `traffic_intersection::a_car_on_the_curve_stops_at_the_jam_gap` | correctness | real hits on the curve, gap precision | `gap = d_k` (tolerance derived; claim dropped if not separable) |
| `traffic_intersection::a_body_at_the_outer_rear_flank_does_not_hold_a_turn` | correctness | nose rule (TASK-016) on connectors | drop nose rule |
| `traffic_pedestrian::a_walker_at_the_flank_does_not_hold_the_car` (re-anchored) | correctness | forward cast from the nose | cast from the centre |
| `traffic_pedestrian::a_walker_goes_around_a_car_across_its_run` | correctness | walker avoidance | empty car list |
| `civilian::tests` `standing_cars` unit | correctness | only standing vehicle bodies steer walkers | drop standing filter |
| stage 4.2 rows (only if built) | correctness | at-rest vehicle skin; walkers keep `recover.skin` | `rest_skin = recover.skin`; rest skin on characters |
| G4 `*_liveness` x3, r1 seeds 1/7, (c), rb far | liveness + G1 | the in-view lock resolved, city-wide `Dynamic` 30 s | 5.3 |
| `traffic_gridlock` 1/2/7/42, benches, clippy, client, tree_check, WSL, runtime R1 | regression | no regressions | — |

Test numbers above are derived in §3; the implementer recomputes every placement in the test and fails
with `GATE BROKEN` naming the gate when a fixture does not hold. Presentation-free: no client gate is
touched. Every new or touched city gate runs 3x.

## 5. Rollout notes

- No migration, no new env var, no feature flag, no new crate, no new Bevy API, no new data field
  (the at-rest skin is derived from existing `traffic.ron` values; `conflict_margin` reused).
- Behaviour visible to the owner: walkers step around standing cars (GDD §6.2 line), recovered cars slide
  back onto a connector line, fewer stops in the box.
- CI time grows: five heavy city rows un-ignored (3 G4 liveness, 2 r1, 150 s scenes each); the G4
  non-liveness twins duplicate three of them. Report the step-summary times.
- After merge: all 5 workflows green on the merge commit (`gh run list --repo pockerhead/MAW-make-GTA
  --branch main --limit 10`); README test counts from the step summary.
- Probes in `scratch/probe` mirror `recover.rs`; after stage 1 they must use the new `on_path`. They
  stay in scratch; they are not gates.
- File sizes to watch (750 warning): `drive.rs` 638, `traffic_recovery.rs` 468 (+f and 4.2 rows ≈ 680),
  `traffic_intersection.rs` 393 (+3 rows ≈ 620), `civilian/mod.rs` 517, `query.rs` 382,
  `manoeuvre.rs` 182.

## 6. Review notes (what changed from PLAN_V2 / PLAN.md and why)

Kept from V2 (verified against code): table band instead of lane band for connector recovery (R1);
held car + in-band/out-of-band f-rows (R2); flank gate re-anchor need (R3); gap by bisection (R4); the
step derivation needed its own reasoning (R5, diagnosis only); `box_scene` shared by five rows (R6);
class-D arithmetic for (c) (R7, now resolved by Q4); data paths and GDD §6.2 (R8); oncoming-claim rule
(R9); lane-arm note (R10). Restored from PLAN.md: the full cause table, line-level understanding, probe
build command, expected re-trace outcomes, file list for docs.

Changed by this review:
1. **Stage 2 nose rule** (new). V2 posed full-body samples from travel 0: flank bodies and bodies the
   rear swings into on a curve would hold a car on a connector (TASK-016 regression where no gate looks).
   Bodies now count only with a part ahead of the current nose line; a connector flank row gates it.
2. **Step law** replaced. V2's "0.3 m, r >= 1.7 m → corner < 0.6 m" is false (0.63 m at r 1.7; Bezier
   minimum radius 0.707 x leg; TASK-038 measured 0.83-0.98 m at 0.2 m). The path sweep reuses the
   corridor law (0.3 m / 7.5 deg, constants moved to `lateral.rs`) and claims no bound.
3. **Rejoin slides on connectors** (new, 1.3). V2's G1 argument needs the body inside the 1.35 m table
   band, but the existing heading law swings a car at rest 7.5 deg for a 0.10 m rejoin (reach 1.46 m).
   The f-row asserts the band every tick; flip C.
4. **Walkers avoid standing vehicles only** (3.1) with a unit flip; V2 steered at every vehicle.
5. **Flank gate re-anchor on the player** with a penetration precondition; V2's "or a dummy placed
   touching it" cannot flip (a resting dummy sits 1.55 m across, outside the 1.5 m band).
6. **Q4 option B specified** (resolved by the orchestrator after V2): skin derived from existing data
   (0.18 m), vehicles only, own flip rows; no new tuning number (V2 had "+ a data margin in traffic.ron").
7. **"on the curve" row made measurable**: named `pass.vehicle_seconds` fixture mutation (else demotion
   at 3 s and a box pass move the car before IDM settles), an independent 0.01 m travel oracle, and a
   tolerance derived from the fixed and flipped runs (IDM rest precision is unmeasured; the lane gate
   uses ±0.3).
8. Wander speed source corrected: `wander_gait: Walk` → `walk_speed` 1.8 in
   `assets/character/locomotion.ron`, not `civilian.ron`. `civilian_bench` (8 ms) added to the
   benches; stage 0 records bench baselines. The G4 non-liveness "(reported, TASK-037)" label updated.
9. Stage 0.2 (d) added: penetration depth of a pressing character, needed by the flank re-anchor.

Open concerns (not changed, documented): residual band 1.35-1.625 m on connectors (neither recover nor
give up, as today; stop rule in 1.8); `connector_rects` overhang for ungranted kinematic cars in the box
(TASK-036 item 4) now applies to more cars — the G1 oracle in G4/R1/rb/(d) is the check; a walker pinned
between two cars gets `to` back from `around_cars` and keeps pressing (probe column (c) shows it).

Disconfirmation result: the tested counter-example (stage-2 flank/rear-swing hits, and the step law)
held; both are corrected above. children: 0 launched / 0 reported.
