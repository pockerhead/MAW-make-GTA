# PLAN — TASK-037: in-view box lock (rescoped: Dynamic-car recovery on connectors, explain first)

Cost of error: silent (gridlock over minutes in front of the player, plus G1 pass-through if a kinematic
body ends up where the conflict table does not expect it). Full evidence layer: cause table, flip-RED per
mechanism, G1 in every touched city gate, WSL run, runtime R1.

## 0. Headline: stage 0 was run by the planner, and it changes the premise

I ran the "explain first" trace on the current code (HEAD 6f31688) before planning, with a read-only probe
crate in scratch that reuses the gates' own fixtures (`tests/common`, `tests/traffic_support` via
`#[path]`) and recomputes the `recover.rs` predicates from public state (`RoadOccupancy`, `TrafficCar`,
`FlatRect`):

- probe: `scratch/probe/ws/probe/tests/trace.rs` (build: `cd scratch/probe/ws/probe &&
  CARGO_TARGET_DIR=D:/test-gta-like/target cargo test --offline --test trace -- --exact <row> --nocapture`;
  `scratch/probe/assets` is a junction to `assets/` so `common::assets_root()` resolves),
- outputs: `scratch/trace/{g4_seed_1,g4_seed_7,g4_extra_seed_1,r1_seed_1,r1_seed_7,rb_far_seed_1,c_node_141}.txt`
  and `r1_seed_7.kin.txt` / `g4_seed_7.kin.txt` (kinematic heads). Each row takes 15-20 s.

The seed-1 G4 run reproduces the premise challenge's numbers exactly (2109v0 95.5 s `Dynamic` at
(-4.43, -86.73); 2021v0 93.9 s), so the probe measures the same scene.

### Cause table (current code)

| Row | Head | Where | Switched by | Why it never recovers |
|---|---|---|---|---|
| G4 seed 1 | 2109v0 | **`Lane(294)` s 77.40 / 77.61** (lane end, in the crosswalk), no grant | walker, gap 0.28 | walkers pinned at gap 0.00 for 79 s (`nobody_coming` and corridor sample 0 both hit the walker); led by the walker, so no give-up |
| G4 seed 1 | 2021v0 | **`Lane(299)` s 66.27 / 66.95**, **granted** 727 | walker, gap 0.28 | walker pinned at gap 0.00 for 90 s; its grant is kept (walkers are excluded from `body_blocked`, TASK-033), so the conflicting approaches (east queue x 11-48) stand 100 s |
| G4 seed 7 | 2017v0 | `Connector(714)` s 0.43, no grant, not a waiter | walker, gap 0.26 | **only `on_lane`**: `coming: []`, corridor clear |
| G4 extra seed 1 | 2140v0 (the extra car) | `Connector(720)` s 1.0, waiter | walker, gap 0.28 | walkers pinned (gap 0.00, up to 90 s) **and** `on_lane` |
| R1 seed 1 | 1853v0 | `Connector(726)` s 2.1, no grant | **the left car**, gap 0.31 (the fixture parks it 0.31 m in front of a car moving at 2.8 m/s) | left car at gap 0.06-0.14 (inside `switch.skin` 0.1 and `recover.skin` 0.4) **and** `on_lane`; walkers pinned later |
| R1 seed 7 | none `Dynamic` | holders 1930v0 `Connector(709)` s 1.56, 1927v0 `Connector(717)` s 1.40, both granted and **kinematic** | — | each stands at exactly 2.00 m (IDM rest `s0`) behind the left car seen by the **straight** 6 m connector strip (`manoeuvre::sense`), while the true body sweep of 709 / 717 passes the left car at **0.39 / 0.37 m**; `connector_clear` also says clear, so no demotion, and neither is `alone_in_box`, so no box pass. 14 cars stand 128-146 s on all four approaches |
| rb far seed 1 | 1922v0 | switched on `Connector(719)` s 3.5, stands on `Lane(249)` s 0.6 (exit lane start) | walker, gap 0.25 | 3-7 walkers pinned at gap 0.00 for up to 85 s |
| (c) node 141 | 2002v0 | `Lane(509)` s 99.9 / 107.35, later granted 1300 | a `Dynamic` traffic car 1919v1, gap 0.30 | walker pinned at gap 0.00 **and** 1919v1, since given up (`Vehicle` kind), standing at 0.23-0.34 m (inside `recover.skin`) |

So the binding rescope is right about one class and incomplete on the rest. The premise challenge read
"(-4.43, -86.73)" as `Connector(719)` s 0. The trace shows it is `Lane(294)` at its end. Four classes:

- **B, `Dynamic` on a connector, nothing else in the way** (G4 seed 7): connector recovery, binding item 1.
- **C, kinematic holders held by a sensing false positive** (R1 seed 7): a box rule that "does not fire for
  kinematic heads". Binding item 2 says fix that rule. The rule at fault is connector sensing.
- **A, walkers pinned at a `Dynamic` head** (G4 seed 1, extra, rb, (c), later R1 seed 1): a mutual wait.
  The walker walks into the car (civilians steer straight at `lane_target`, `civilian/mod.rs:552-556`, with
  no car avoidance at all), and the car cannot recover while a walker is inside its skin, and would wait
  for it in IDM anyway. No car-side rule breaks this. SUMO handles the same car-walker deadlock on
  crossings from the pedestrian side (after `jamtime.crossing`, 10 s, a jammed person moves on regardless,
  [SUMO pedestrians](https://sumo.dlr.de/docs/Simulation/Pedestrians.html); the deadlock itself:
  [sumo#5662](https://github.com/eclipse-sumo/sumo/issues/5662)). This is TASK-036 item 1, and the
  acceptance ("together with TASK-036's walk avoidance if the walkers keep it pinned") anticipated it.
  **Open question Q1.**
- **D, a `Dynamic` car inside the recovery skin of a standing non-walker body** (R1 seed 1 1853v0 at
  0.06 m, (c) 2002v0 at 0.23-0.34 m): the TASK-032 hysteresis (`recover.skin` 0.4 > `switch.skin` 0.1)
  never lets it recover, and nothing moves it away. **Decision point after stage 3, Q3.**

Re-route and U-turn (the task's fallback) would fix none of A-D: every one of these heads either already
has a free exit (repick exists) or is held by something other than the exit choice.

## 1. Understanding (current code)

- `traffic/recover.rs:131-167` `recover_dynamic`: recovery needs upright, `at_rest`, **`on_lane`
  (`:144`, `Segment::Lane` only)**, `nobody_coming` (`:38-59`, every dynamic body swept by the relative
  velocity over `recover.horizon_seconds`, against the footprint grown by `recover.skin`), and
  `corridor_clear` (`:63-90`, current pose to the line pose at the same `s`, 7.5 deg / 0.3 m samples, any
  body or claim), all for `recover.seconds`. Give-up (`:156`) only when `off_lane` (`:94-107`, footprint
  past half the lane pitch; for a connector the pitch of `from_lane`) and not `led` (`:111-128`).
- `traffic/drive.rs:255-289` runs recovery for `Dynamic` cars; `:612-632` applies it
  (`RigidBody::Kinematic`, removes `Autopilot`/`DriveIntent`/`SleepingDisabled`, `lateral = measured`,
  `manoeuvre = Rejoin`). Kinematic motion `:475-569`: the lateral offset is stepped only
  `if manoeuvring && (Lane || passing)` (`:534`); `effective_lateral(.., passing)` (`:477`, `:538`) decays
  any other offset linearly to 0 along a connector (`lateral.rs:41-55`); at the connector end a
  non-passing car gets `lateral = 0.0` (`:511-515`). Rejoin ends at `|lateral| < 0.01` and path error
  < 1 deg (`:555-561`).
- `traffic/manoeuvre.rs:46-87` `sense`: on a connector a **straight** strip from the nose along the tangent
  at `s`, `turn_sense_distance` (6 m) long, `half.x` wide (`:61-67`, `:73-79`). `plan` (`:99-182`): lane
  pass and box pass only for `idle` kinematic cars (`:115-118`, `lateral == 0.0`, `manoeuvre == None`);
  box pass needs `alone_in_box` (`:171`).
- `traffic/junction.rs:195-268`: a kinematic car on a connector queues there (at `s <= REPICK_WITHIN` it
  may repick); a `Dynamic` car on a connector is skipped (`:204-205`). Holders on their connector are
  demoted only when `connector_clear` finds a body standing `pass.vehicle_seconds` (`:106-116`,
  `:158-163`). Whole-box grant `:316-325`.
- `traffic/box_rules.rs:13-49` `connector_rects` / `connector_clear`: the connector polyline as bands of
  `half.x + conflict_margin/2`, no corner overhang (TASK-036 note: body corners reach 0.87-0.91 m past it on
  right turns). `path_pose` `:70-81` (connector, negative = source lane, past the end = exit lane).
- `traffic/contact.rs:131-246` the switch: any dynamic body whose relative sweep over
  `switch.horizon_seconds` reaches the footprint grown by `switch.skin`.
- `traffic/graph.rs:111-139` `body_sweep` (TASK-038): body rectangles along a connector at 0.2 m.
- `civilian/mod.rs:456-557` `civilian_fsm` (`AiSystems::Decide`): wander/flee walkers steer straight at
  `navigation::lane_target`; no wall or car avoidance. `tactics/fire_line.rs:144-172` `around_cars` (police
  use it at `police/behavior.rs:407` with `radius` and `radius + nav.arrive_radius`).
- Gates: `tests/traffic_junction_box.rs` (3 ignored `_liveness` rows, `check` `:238-285`),
  `tests/traffic_causes.rs` (ignored `r1_*` `:501-511`; `dynamic_bound_on` in (c) `:293-296` and rb
  `:531`), `tests/traffic_recovery.rs` (`e_off_path_car_with_nothing_ahead_gives_up` `:418-468` asserts
  a car never recovers on a connector, `GATE BROKEN: it recovered on the connector`).
- Data: `assets/traffic/traffic.ron` (`recover`, `switch`, `lateral`, `turn_sense_distance`, `pass`).
  `recover.skin > switch.skin` and `recover.horizon >= switch.horizon` are validated
  (`config.rs:259-270`).

## 2. Approach

Build the missing shared rule the orchestrator named ("a bumped car can always return to the kinematic
world wherever it stands"), then fix each existing rule the trace shows misfiring, in cause order, with
a re-trace after every stage. No new parallel box mechanism.

1. Stage 0: baseline with the gates' own measure + the cause table (done once here; the implementer
   reruns the probe on its branch point and after each stage).
2. Stage 1 (class B): recovery on connectors, inside the lane band, with the Rejoin offset stepped by the
   lateral law on connectors too.
3. Stage 2 (class C): connector sensing along the path with the swept body (the TASK-038 lesson: two-body
   predicates use swept bodies, not centre lines; sampled-rectangle sweeps are the standard way to check
   a curved path, e.g. [Laumond, Collision detection for motion planning](https://www.di.ens.fr/jean-paul.laumond/promotion/chap6.pdf),
   with the known caveat that a fixed step can miss thin obstacles, bounded here by the yaw/lateral step
   law already used by `recover.rs`).
4. Stage 3 (class A, per Q1): walkers go around car bodies (TASK-036 item 1, pulled in).
5. Stage 4 (class D, per Q3): only if a gated row is still red on class D after stages 1-3.
6. Stage 5: un-ignore rows, restore city-wide `Dynamic` bounds, full regression, WSL, runtime R1.

Why stage 1 recovers only inside the lane band: the conflict table (`graph.rs` `body_sweep`) and
`connector_clear` model a car on its connector line. A kinematic body up to `lost.distance` (4 m) off the
line inside a box is outside that model (TASK-036 notes the residual-offset gap), which is the G1 class.
Inside the band (`!off_lane`, footprint within half the lane pitch of the line) the body sits where the
table puts it. A car out of the band on a connector keeps today's behaviour (waits while led, gives up
otherwise), so `e_off_path_car_with_nothing_ahead_gives_up` stays valid unchanged. Every connector head
in the trace is in band (lateral 0.00-0.05 m).

Why the Rejoin law changes on connectors: with the decay law a recovered car keeps `lateral != 0` until
the connector end, so it is never `idle` there (`manoeuvre.rs:115-118`), and the box pass / repick of a
kinematic car on its connector would never engage. Stepping the offset at the lateral rate (as on lanes)
ends the Rejoin in place: 0.43 m at `rate_at_rest` 0.8 m/s takes 0.54 s.

## 3. Steps

### Stage 0 — baseline and cause table (no code change)

0.1 Run the five ignored rows with their own measure on the branch point and store verbatim output:
`cargo test -p gta_sim --test traffic_junction_box -- --ignored --nocapture` and
`cargo test -p gta_sim --test traffic_causes r1_ -- --ignored --nocapture`, plus the non-ignored (c) and
rb rows (their "reported" lines hold the city-wide `Dynamic` stands). Output under
`scratch/stage0/*.txt`.
0.2 Rerun the planner probe (`scratch/probe`, rows above) and extend it to R1 seed 7 heads that are
`Dynamic`, if any appear on the implementer's run. Keep the cause table in `scratch/stage0/causes.md`.
Check: every red row's longest stand has a named class (A-D) and a named blocker. A row whose cause
differs from the table above goes to OPEN_DECISIONS before any code.

### Stage 1 — recovery on connectors (class B)

1.1 `crates/gta_sim/src/traffic/recover.rs:144`: replace
`let on_lane = matches!(snap.car.segment, Segment::Lane(_));` with a band rule: on a lane as today; on a
connector only when `!off_lane(graph, snap, half)` (the existing fn, `:94-107`, already takes the pitch of
`from_lane` for connectors). Name it `on_path`. Update the module doc (`:1-6`) "on its lane" → "on its
path (a connector only within the lane band)". `corridor_clear` stays as it is (`graph.pose` handles a
connector); the in-band corridor on a connector is the sideways move to the connector line at the same
`s`, the same sweep and skin as on a lane.
1.2 `crates/gta_sim/src/traffic/manoeuvre.rs`: add `pub(super) fn holds_offset(car: &TrafficCar) -> bool
{ passing(car) || car.manoeuvre == Manoeuvre::Rejoin }` next to `passing` (`:38-40`), with a one-line doc:
an offset that is steered by the lateral law on a connector instead of decaying.
1.3 `crates/gta_sim/src/traffic/drive.rs`:
- `:477` and `:538`: `effective_lateral(.., passing(car))` → `holds_offset(car)`;
- `:534`: `(matches!(seg, Segment::Lane(_)) || passing(car))` → `|| holds_offset(car)`;
- `:509-515` connector end: `if passing(car) { shift_pass } else if car.manoeuvre != Manoeuvre::Rejoin
  { car.lateral = 0.0 }` (a Rejoin still offset at the connector end carries on along the exit lane; no
  snap). Fix the comment on `:509-510` to say so.
1.4 `crates/gta_sim/src/traffic/manoeuvre.rs:59` and `:63`: `passing(car)` → `holds_offset(car)` in the
`effective_lateral` call and in the connector arm of `sense`, so a rejoining car on a connector senses at
its target (0) and at its current offset, like on a lane.
1.5 `crates/gta_sim/src/traffic/lateral.rs:1-3` and `:39-40`: doc — "on a connector it decays linearly
to 0 at the connector end, unless the car passes or rejoins".
1.6 Side effect to check, not to design around: a lane Rejoin or a siren-yield Rejoin
(`sirens.rs:74`) that reaches a connector now steps instead of decaying. Both are continuous; the G1 rows
of `traffic_recovery.rs`, `traffic_go_around.rs`, `traffic_intersection.rs` and `traffic_graph.rs` must
stay clean.
1.7 Gates, `crates/gta_sim/tests/traffic_recovery.rs` (floor: `two_way_street(70.0)`, the one
`e_off_path_car_with_nothing_ahead_gives_up` uses; not the loop, whose lanes have no neighbour, so
`off_lane` takes the car's own width as pitch and any offset is out of band):
- new row `f_connector_car_recovers_in_band`: a traffic car placed on the U `Connector(0)` at `s = 1.0`
  with its grant (the `d_car_left_on_a_connector` fixture pattern: push `(c, car)` into the node's
  occupants), `switch_by_hand`, shoved 0.2 m right of the line (in band: lane 0 `left_gap` 3.25 m, slack
  `3.25/2 - half.x` = 0.425 m; compute it in the test and assert `GATE BROKEN` if the footprint reach is
  outside the band). Assert: kinematic
  within `recover.seconds + 1 s`; `lateral` reaches 0 on the connector (`|lateral| < 0.01` and
  `manoeuvre == None` before `s` reaches the connector end); moving on its path within 3 s; G1 clean.
  Derive the expected Rejoin time: `0.2 / rate_at_rest` = 0.25 s from rest.
  Flip: restore `on_lane = matches!(.., Lane(_))` → the row goes RED (never kinematic). Second flip:
  revert 1.3's `:534` change → RED (Rejoin never finishes on the connector).
- `e_off_path_car_with_nothing_ahead_gives_up` (`:418-468`): unchanged behaviour (1.5 m and 30 deg is
  off band). Re-word its doc and the `GATE BROKEN` message: "it recovered out of the lane band on a
  connector". Record in the stage summary that it is now also the out-of-band flip of 1.1 (sabotage:
  `on_path = true` on connectors → RED).
- `e_blocked_corridor_waits_in_the_queue_and_clear_corridor_recovers`, `a_nudged_car_recovers`,
  `c_pressed_car_does_not_flip_flop`: unchanged, must stay green.
1.8 Re-trace G4 seed 7 and the extra row. Expected: 2017v0 turns kinematic at `Connector(714)` s 0.43,
queues on its connector (`junction.rs:204`), may repick (`s <= REPICK_WITHIN`). Check: its `Dynamic`
stand ≤ `recover.seconds` + the time a walker stays inside its skin.

### Stage 2 — connector sensing along the path (class C)

2.1 `crates/gta_sim/src/traffic/manoeuvre.rs` `sense` (`:46-87`): on a connector, replace the straight
strip at the target offset with a path sweep: body rectangles (`half.x`, `half.z`, not grown) posed along
the path from `s` to `s + turn_sense_distance` with `box_rules::path_pose` (make it `pub(super)`), each at
the target offset (`offset_pose`), yaw of the path tangent. The hit is the first sample whose rectangle
overlaps a body or an oncoming claim not dropped by the existing `skip` (`plain` + `OnPathTraffic` rule
unchanged); `gap` = sample distance along the path minus `half.z` (nose to the body side; for sample 0 the
body straddles the nose: 0). Keep the second strip (current offset, only while target and current differ)
as a straight strip: it covers only the rest of a sideways move plus two jam gaps.
Sample step: a geometric law, not tuning — reuse the `recover.rs` corridor law (7.5 deg of yaw or 0.3 m
of travel, whichever is finer); move `CORRIDOR_YAW_STEP_DEG` / `CORRIDOR_LATERAL_STEP` to one place
(`lateral.rs`, `pub(super)`) with their existing derivation comment instead of a second copy.
To report `speed_along` and `standing` the hit needs the body: add
`RoadOccupancy::first_in(&self, rects: &[FlatRect], skip) -> Option<(usize, Hit)>` in
`occupancy/query.rs` next to `blocked`, returning the index of the first rectangle hit (the claim side
uses the same `ClaimFilter::Against(dir)` semantics as `first_along`: claims of cars travelling against
the sample's tangent).
2.2 `crates/gta_sim/src/traffic/box_rules.rs` `plan_box_pass`: unchanged; it consumes `hit.entity` from
`sense` and still gets the body when the body really is on the path.
2.3 Gates:
- `crates/gta_sim/tests/traffic_intersection.rs` (or a new `traffic_connector_sense.rs` if the file would
  pass 750 lines; it is 393): floor = a synthetic junction with a 90-degree connector (from the existing
  intersection fixtures). Row "beside the curve": a parked sleeping car whose footprint is 0.35 m outside
  the swept body of the connector but inside the straight 6 m strip from the connector entry; a kinematic
  car granted that connector passes it without stopping (`speed > 0` through the connector, reaches the
  exit lane within `length / turn_speed + 2 s`), no switch (`switches_by_cause` unchanged), G1 clean.
  Row "on the curve": the same car moved onto the path (sweep overlap): the car stops at the jam gap
  before it (IDM rest: nose-to-body gap within `min_gap ± 0.05` measured along the path).
  Derive both placements in the test from `graph.pose` + `FlatRect` and assert them (`GATE BROKEN` if the
  "beside" body is not in the straight strip or is in the sweep).
  Flip: restore the straight strip → "beside" RED (stands at 2.00 m). Second flip: sample step ×10 →
  "on the curve" RED if the body falls between samples (if it does not, say so and do not claim the
  step as gated; the fine guarantee stays with the `traffic_graph` oracle class).
- `traffic_bench` mean tick under `MEAN_LIMIT` (19 ms) with the path sweep; record before/after.
2.4 Re-trace R1 seed 7. Expected: 1930v0/1927v0 drive past the left car (0.37-0.39 m clearance), the
approaches drain. If they still stand, the next rule in line is `alone_in_box` (two holders each behind
the same body) — record it in OPEN_DECISIONS before touching it.

### Stage 3 — walkers go around car bodies (class A; only after Q1)

Default answer to Q1 assumed below: TASK-036 item 1, minimal slice, pulled in.
3.1 `crates/gta_sim/src/civilian/mod.rs` `civilian_fsm` (`:456-557`): add `road: Res<RoadOccupancy>` and
`Res<VehicleConfig>`/`Res<LocomotionConfig>` as needed. For a walking civilian (Wander, Flee), the steer
target becomes `tactics::around_cars(position, lane_target(..), &cars, radius, radius + nav.arrive_radius)`
— the same call and the same values the police arrest walk uses (`police/behavior.rs:407`), where
`radius` = `LocomotionConfig::capsule_radius`. `cars`: the vehicle footprints (`Footprint::Rect`) of
`road.bodies()` within `nav.avoid_distance + half diagonal` of the walker, converted to
`tactics::CarRect { centre, axis, half }` (add a `From<FlatRect>` or build it inline; `CarRect` fields are
`pub(crate)`). No new tuning value; if one turns out necessary it goes to `assets/navigation/*.ron` or
`assets/civilian/*.ron` with a strict loader field, never a `const`.
3.2 A new `Res` on `civilian_fsm` is an API change for every harness that registers it: grep
`civilian_fsm`/`CivilianPlugin` in `src/` and `tests/`. `RoadOccupancy` is `init_resource`d by
`OccupancyPlugin` in `compose_sim`; on the test floor without a `TrafficGraph` the snapshot never fills, so
civilian gates there see no cars (unchanged behaviour). Confirm by running `cargo test -p gta_sim --test
civilian*` and `population*`.
3.3 Gates:
- A floor row (new, `crates/gta_sim/tests/civilian_cars.rs` or in the existing civilian test file if it
  stays < 750 lines): a two-node sidewalk edge crossing a lane (a crosswalk), a parked car across the
  edge; a wandering civilian walking that edge passes the car (reaches the far node within
  `edge length / wander speed + 5 s`) and never stands within `capsule_radius + 0.1 m` of the car
  footprint longer than 2 s. Flip: pass an empty car list → RED (stands pressed until the timeout).
- TASK-036 item 1's criterion ("no civilian stands against a car longer than a derived bound") is carried
  by the G4 seed 1 re-trace: longest walker `standing` at gap ≤ 0.05 m to a car, from the probe. Derive
  the bound from the fixed run; broken run is 90 s.
- City gates that count civilians (`population*`, `civilian*`, `traffic_pedestrian.rs`) stay green; the
  witness gate on 100 seeds (TASK-026) stays green.
3.4 Re-trace G4 seed 1, extra, rb far, (c). Expected: walkers step around the heads; 2109v0/2021v0/
2140v0/1922v0 recover within seconds of the last walker leaving the skin; 2021v0's grant turns over.
Watch for the new risk: walkers going around a car's nose into the lane switch the next car to `Dynamic`
(count switches by `SwitchCause::Character` near the box before/after, from `TrafficStats`).

### Stage 4 — class D (decision point, Q3)

4.1 After stages 1-3, rerun the probe on R1 seed 1 and (c). If no gated row is red on class D, build
nothing and record it.
4.2 Else apply the Q3 answer. Default: if the only class-D stand is the R1 fixture artefact (a body
teleported 0.31 m in front of a car moving 2.8 m/s), make `traffic_causes::box_scene` clear the spot the
way G4 does (`traffic_junction_box.rs:99-118`: wait until no vehicle or character is within
`2 * half.z + 2` of the spot, instead of `clear_spot(hub, 6.0)` which only removes centres within 6 m),
as a named fixture mutation with that reason in the row doc; the runtime R1 (a car that arrives moving,
seen by the predictive switch) stays the check on real play. If class D shows on a non-teleport body
((c): a given-up car 0.23-0.34 m from a head), stop and write a redesign note with the two named options
of Q3 before building either.

### Stage 5 — restore the gates, regressions, docs

5.1 `crates/gta_sim/tests/traffic_junction_box.rs`: remove `#[ignore]` from the three `_liveness` rows
(`:302-318`); update the module doc (`:12-19`: no longer "ignored: the in-view box lock is open"). Keep
the non-liveness rows (lease + G1 only) as they are.
5.2 `crates/gta_sim/tests/traffic_causes.rs`: remove `#[ignore]` from `r1_car_left_in_the_box_seed_{1,7}`
(`:501-511`) and update the `r1` doc (`:484-487`); in `c_character_in_the_lane` (`:293-296`) and
`rb_a_box_car_seen_from_afar_is_cleared` (`:531`) replace `dynamic_bound_on(..)` with
`clock.dynamic_violation()` (city-wide), then delete `dynamic_bound_on` (`:305-328`) if unused; update the
module doc (`:9-11`, `:18-24`).
5.3 Flip evidence for the restored rows: revert stage 1.1 → G4 seed 7 liveness RED; revert stage 2.1 →
R1 seed 7 RED; revert stage 3.1 → G4 seed 1 liveness RED. Record which input was perturbed.
5.4 Full suite: `cargo test -p gta_sim -p citygen` (incl. `traffic_gridlock` seeds 1/2/7/42 within 40 s),
`cargo test -p gta_like --bin gta_like`, `cargo clippy --workspace --all-targets -- -D warnings` (as CI),
`python tools/qa/tree_check.py`, `traffic_bench` and `police_bench` under `MEAN_LIMIT`. Every touched city
gate reports G1 max depth.
5.5 WSL: rebuild the TASK-038 Linux setup (`maw/tasks/done/TASK-038/IMPL_SUMMARY.md:15-20`: sources
synced to the WSL filesystem, `CARGO_TARGET_DIR` there, Ubuntu-22.04, toolchain 1.95.0, `-j 2`; the
scripts were not archived, write `scratch/wsl_sync.sh` / `scratch/linux_run.sh` anew) and run every
touched city gate: `traffic_junction_box`, `traffic_causes`, `traffic_gridlock`, `traffic_recovery`,
`traffic_intersection`, `traffic_go_around`, `traffic_pedestrian`. A Linux-only red is a real bug.
5.6 `docs/architecture/traffic.md`: "Modes" (`:66-77`: recovery on a connector within the lane band;
Rejoin stepped on connectors), "One tick" item 5 (`:102-112`: connector sensing along the path), "Junction
box" (`:136-148`: remove the "Open (TASK-032 R1)" sentence, state what now resolves the in-view lock and
what remains), and a line on civilians going around cars (stage 3). GDD §5.2: a one-line amendment only
if stage 3 ships ("pedestrians walk around cars that stand across their way") — the walker behaviour is
player-visible; the recovery and sensing changes are not.
5.7 Runtime R1 (QA): `maw/tasks/done/TASK-032/scratch/qa/run_r1.py` at spots A/B/C + control on this
branch and on main (same script, same measure): spot A median ≤ main's 1 car over 30 s, B and C 0,
control 0. Owner run (not gated): how the recovered cars and the walkers around them look.

## 4. Risk areas

- **G1 from recovered kinematic cars in the box.** Mitigated by the in-band rule (1.1). Residual: a car
  in band but yawed so a corner reaches a conflicting connector's sweep; `off_lane` counts the footprint
  corners across the line, so the yaw is bounded by the band. The G1 oracle in G4/R1/(d) is the check.
- **Stage 2 changes every car's box sensing.** Fewer false stops, but the swept body also sees what the
  straight strip missed (the outside of a tight right turn, walkers at the curb): right turners may stop
  for curb walkers more often. `traffic_gridlock` (4 seeds, 40 s) and `traffic_pedestrian` are the checks;
  cost: `traffic_bench`. Police cars (`police/car_route.rs:540`) keep their own sensing.
- **A give-up timeout also ends locks it resolves silently** (game-design lesson, TASK-032): no give-up
  rule is narrowed here. Recovery on connectors removes some cars from the give-up path (they recover
  instead); list every city gate's `Dynamic` and stand bounds before/after stage 1 (stage 0 table vs
  stage 1 re-run) and name any lock that was only ended by a give-up.
- **Walkers around cars (stage 3) step into lanes.** New `SwitchCause::Character` switches near boxes
  could create new `Dynamic` heads. Measure switches by cause before/after on G4 seed 1.
- **Rejoin on connectors now steps instead of decaying** for lane recoveries and siren yields that reach
  a connector; small, continuous, gated by the existing G1 rows.
- **Cross-platform divergence**: every touched city gate on WSL (5.5).
- **The probe is not a gate.** Its predicate recomputation mirrors `recover.rs` at HEAD 6f31688; after
  stage 1 it must mirror the new `on_path`. It stays in scratch.
- **File sizes**: `drive.rs` 638, `manoeuvre.rs` 182, `traffic_recovery.rs` 468, `civilian/mod.rs` — check
  the 750 warning after each stage.

## 5. Open questions

**Q1 (blocks stage 3). The walker side.** Five of the eight traced rows are held by walkers pinned at gap 0
against a `Dynamic` head for 30-90 s; civilians have no car avoidance at all. The acceptance of this task
cannot be met without a walker-side rule.
- A (recommended): pull TASK-036 item 1 into TASK-037 as stage 3 (civilians go around car bodies with the
  existing `around_cars`, fed by `RoadOccupancy`). TASK-036 keeps items 2-4. The pinned-walker symptom
  TASK-036 asked to reproduce is reproduced here (`scratch/trace/g4_seed_1.txt`).
- B: block TASK-037 on TASK-036 item 1 and run that first. Same code, one more task cycle.
- C: ship stages 1-2 only; keep the walker-held rows ignored with a pointer to TASK-036. The acceptance
  would then change, which the orchestrator ruled out.

**Q2 (stage 3 variant).** Walkers go around (GTA-like, the police mechanism) or turn back after a short
stand (stays on the sidewalk, no lane entry)? Recommended: around, turn-back as the named fallback if
stage 3 raises `Character` switches near boxes.

**Q3 (stage 4, only if class D stays red).** A `Dynamic` car within `recover.skin` of a standing
non-walker body has no way back.
- A: the R1 fixture artefact only → clear the R1 spot like G4 (4.2). Recommended if (c)'s 2002v0 clears
  once the walkers are gone (its given-up neighbour is out of frame and falls to the stuck cheat).
- B: a controlled back-off (the `Dynamic` car reverses along its path at crawl until the body is past
  `recover.skin`, rear corridor clear). New autopilot mode (`speed_throttle` never reverses today).
- C: a static-pair hysteresis (a body at rest relative to the car counts within `switch.skin` + margin
  instead of `recover.skin`). Does not help a car inside `switch.skin` (1853v0 at 0.06 m).
Two failures of the same class → redesign note, then re-route / U-turn as the task's named fallback.

## Dependencies and environment

No new crate. The probe crate in `scratch/probe` builds offline with the workspace `Cargo.lock`
(copied), the same `[patch]` entries and the shared `D:/test-gta-like/target`.
