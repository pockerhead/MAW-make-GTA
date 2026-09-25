# PLAN_V2 — TASK-016 (GDD T15: traffic and police cars)

Reviewer: plan-reviewer-1. Base: `PLAN.md` (planner, commit cca4b69) + `TASK_FINAL.md` orchestrator decisions
(Q-А a, Q-Б b, Q-В b, Q-Г a). This file is the complete plan; it replaces PLAN.md.

Cost of error: HIGH, silent class (infinite-mass pushes, kinematic pairs without a solver response, two cars
granted one conflict point, despawn in view, unit/car caps drifting, a car-entity leak, frame cost of the largest
population). Full evidence layer for the traffic core, dispatcher caps, cabin/bail-out, pull-out, fire line and
forced eject. Feel (density, turn look, chase handling, sirens, models) goes to the owner's run.

---

## 0. Disconfirmation (done before the review)

Counter-example chosen: "the headless traffic gates on the test area (steps 21, 22, 23, 25 of PLAN.md) insert only a
synthetic `TrafficGraph`; the traffic systems never run there, so the gates are vacuous or fail on plumbing".

Checked in code: `navigation/mod.rs:347-350` configures `NpcSystems.run_if(resource_exists::<SidewalkGraph>)`;
`flow/mod.rs:107-114` adds the state condition. The test area has no `SidewalkGraph` unless a test inserts one
(`tests/common/mod.rs:373 test_graph`, used by `gang_floor`). PLAN.md puts `TrafficSystems` `.in_set(NpcSystems)` and
its fixture `test_lanes(app, TrafficGraph)` inserts only the traffic graph. **The counter-example HOLDS**: every
test-area traffic gate would see cars that never move (and the hijack gate additionally needs a sidewalk graph to
spawn the fleeing driver). Fixed in §3 (fixture inserts both graphs + a `GATE BROKEN` liveness check).

---

## 1. Review notes (issues found in PLAN.md, with evidence)

R1. **Traffic sets gated by `SidewalkGraph` (above).** Test fixtures must insert a sidewalk graph; every traffic gate
asserts `GATE BROKEN` when no car moved in the first 64 ticks.

R2. **Line references in PLAN.md are wrong almost everywhere** (they look like offsets into a concatenation of
files): `seat.rs` has 299 lines, PLAN cites `seat.rs:281-349 / 384-478 / 482-511`; `impact.rs` has 205 lines, PLAN
cites `:329-433`; `police/dispatch.rs` 164 lines vs `:539-641`; `police/arrest.rs` 99 lines vs `:409-478`. The
described CONTENT is mostly right (verified: `exit_spot` forced fallback = left door ray without level check,
`seat.rs:83-88`; `apply_bullet_hits` only lowers `VehicleHealth`, `impact.rs:148-160`; `arrest_player` query
`Without<Driving>`, `arrest.rs:35`). Implementers navigate by symbol, not by these numbers.

R3. **"Append fields at the END of tuples keeps the sabotage strings" is false for one gate.**
`tests/config_police.rs every_star_has_a_row` matches the whole row literal
`"(units: 12, swat: 12, reinforce_seconds: 3.0,  arrest: false, surround: true),"`; adding `cars: 5` inside the row
changes that substring and the helper panics `GATE BROKEN: shipped ... has no`. Re-anchor that string (same gate,
re-run its flip). Also `tests/new_city.rs:306` builds an `ArrestAttempt { .. }` literal — PLAN mentions only the
`PoliceDispatcher` literal at `:301`; both need the new fields.

R4. **Leak: cars that stop being `TrafficCar` are never despawned.** PLAN's hijack removes `TrafficCar`; a Stalled
car is bubble-despawned but a hijacked-then-abandoned car, a bailed-out car (Q-Б) and a Taken police car ("never
despawned by police") become permanent city entities (`spawn_parked_cars` is the only other car source and is
one-shot). Shooting 20 cabins = 20 dynamic cars forever. Fix: every AI car keeps a bubble marker; a car the player
leaves becomes `Abandoned` and follows the traffic despawn rule. Gate in the bubble/leak tests.

R5. **Police-car state machine contradiction.** Step 12: "Dismounted → Respond when no live `CrewOf` cop is
outside"; step 13: an all-dead crew car is bubble-despawned. With the whole crew dead there is no live `CrewOf`
outside → the EMPTY car goes `Respond`, keeps counting toward `row.cars` and blocks a replacement car. Also a crew
member stuck far away (Search) holds its car in `Dismounted` forever. Fix: `Dismounted → Respond` only with crew
aboard ≥ 1 and no live `CrewOf` outside, or after `reboard_timeout_seconds` (stragglers lose `CrewOf` and become
ordinary foot units); crew aboard 0 and no live `CrewOf` → `Abandoned` (not counted, bubble despawn).

R6. **Predictive switch churn.** PLAN's rule (box inflated by a flat 1.0 m + closing ≥ 0.5 m/s) fires far too early.
Worked: a pedestrian crossing 1.2 m in front of a waiting car's bumper (the sidewalk graph crosses streets at block
corners, i.e. right at the lane end = the stop line) at walk 1.4 m/s has a component toward the car centre of
1.4·1.2/√(1.2²+3.0²) ≈ 0.52 m/s ≥ 0.5 → switch, although no contact follows. Dynamic cars never go back to
kinematic (PLAN YAGNI), so intersections fill up with dynamic cars (4 rays + solver + autopilot each). Probe Q4
shows what is actually needed: the switch must happen BEFORE the first contact step. Replace the flat margin by a
time-to-contact test (§2.1, step 8) with its own "crossing pedestrian → no switch" gate row.

R7. **Fire-line rule for cars would starve cops (TASK-010 lesson class).** PLAN adds car perimeter samples to the
`shields` of `FireLine::blocked`, which counts anything up to weapon `reach` PAST the target and widens by the
spread cone (`fire_line.rs:61-76`). Streets are lined with parked cars and traffic behind/beside the player; at 30 m
the cone adds metres of width, so cops/gunmen hold fire in most street fights. O1 asks for "a line that grazes a
car's corner" — a car BETWEEN shooter and target. Fix: cars block only the segment shooter→target, with the plain
`clearance` (no cone widening), tested analytically against the car rectangle (no samples); gate rows for "car
behind the target → fires" and "car 1 m beside the line → fires".

R8. **Q-Б crime path is missing.** A traffic driver is data. The bail-out civilian flees with
`about: Some(Cause::Attack(shot))`, but `Crimes::resolve(Cause::Attack(a))` (`crimes.rs:120-131`) only finds an
incident that already holds that attack id. `record_crimes` records `Crime::Shooting` only when a PERSON is within
`shooting_radius` (15 m) of the muzzle (`crimes.rs:268-289`); the driver is no person at that tick. A cabin shot
from 20 m with nobody near → the call resolves nothing → the orchestrator's "the flee-then-call rule then reports
it" does not happen. Fix: the bail-out writes a message that `record_crimes` turns into a `Shooting` incident for
that attack (step 18b).

R9. **Q-Б mechanics: bail from a moving car.** A kinematic car at 12-16 m/s switched to dynamic with a default
intent coasts at 1 m/s² (`drive_force` idle = `coast_deceleration`) ~100 m; a civilian spawned at its door at speed
teleports. Fix: `Bailing` = the car brakes along its path (kinematic: `max_deceleration`), the driver gets out when
it stands, then the car turns dynamic `Abandoned` (a normal pushable car).

R10. **Client multi-model gaps.** `src/visuals/vehicle.rs` loads ONE scene in `VehicleVisualAssets`
(`:39-47`) and `animate_wheels` divides the wheel lift by `render.vehicle.scale` for every car (`:125, :150`). A
police (and taxi) model with another scale needs per-model scene handles and the scale stored on `VehicleModel`.

R11. **Police-car systems need `TrafficGraph`.** `dispatch_police_cars` with `Res<TrafficGraph>` on the test area
(no graph) must not run: gate it with `.run_if(resource_exists::<TrafficGraph>)` explicitly (PLAN only asserts "no
car ever spawns").

R12. **Small API facts PLAN glossed over.** `cop_sees` hard-codes `cfg.cop_view_distance` (`search.rs:39-53`): it
needs a distance parameter for the 50 m car view. `roll_temperament` takes `&mut NpcRng` (`civilian/mod.rs:230`):
generalise to a `FnMut() -> f32` (or `&mut ChaCha8Rng`) so traffic can roll from `TrafficRng` without touching
`NpcRng` (TASK-010 lesson). Frame-edge despawn: `outside_cone` tests one vertical line (`population/mod.rs:246-249`);
a 4 m car whose centre just left the cone is still visible — test the four roof corners.

Verified as correct in PLAN.md (kept): kinematic probe facts (source read; avian 0.7 `RigidBody::is_dynamic/
is_kinematic`, `SpatialQuery::cast_shape_predicate/shape_intersections` exist at `avian3d-0.7.0/src/...`);
IDM formula and ballistic stop rule (traffic-simulation.de: "decelerate at constant deceleration to a complete stop
and remain at standstill", `x − v²/(2·dv/dt)`); Vermeij distances 70/90, 15/25, SA ≥ 2 s (libertycity.net page
confirmed); all IDM/ballistic worked rows; Bézier worked row (midpoint (1.25, 0, −3.75)); cabin worked rows; the
three autopilot directional rows (f = rot·(−Z), r = f × Y, `wheel_forward(+δ)` has +x); lane geometry (inner avenue
lane 0.85 m from a parked car, street car edge 0.425 m from the curb); RNG streams 1-3 used, 4/5 free; police
`spawn_ring (60,120)` < `despawn_distance 150`; `VehicleEntered.first` → `Crime::CarTheft`; hijack position read (the
player's `Position` is only rewritten by `sync_seats` in FixedPostUpdate, so a FixedUpdate system after
`VehicleSystems::Enter` still reads the standing spot); schedule: `PoliceSystems` runs after `PopulationSystems` and
`WantedSystems`, nothing orders `VehicleSystems::Drive` before them, so `.after(PoliceSystems)` adds no cycle.
Kenney zip sha `fac7dac…d0c4`, `police.glb` `a617b880…352a`, `taxi.glb` `3803539718…bdc7` re-verified by running
`scratch/carkit/glb_probe.py sedan taxi police`.

---

## 2. Updated understanding (corrected)

- Schedule (FixedUpdate): `HealthSystems::{Damage, Regen, Pickup, Death}` chained; `AiSystems::{Perceive, Decide}`
  → `PopulationSystems` chained, `.in_set(NpcSystems).after(HealthSystems::Death)`; `WantedSystems` after
  `AiSystems::Decide`; `PoliceSystems` after `PopulationSystems` and `WantedSystems`, in `NpcSystems`;
  `arrest_player` after Decide, before `WantedSystems`, in `PlayingSystems`. Vehicle: `Enter` (PlayingSystems, before
  Damage), `Impact` (in Damage), `Bullets` (after Damage, NOT ordered vs Death today), `Drive` (after
  Enter/Bullets/Impact); FixedPostUpdate `Record` before `PhysicsSystems::First`, `Seat` after `Last`.
  `NpcSystems` = Playing|Wasted|Busted AND `SidewalkGraph` exists.
- `enter_exit` (seat.rs): entry to the nearest car with `driver == None` and `|v| ≤ exit_max_speed` (3 m/s) within
  `enter_radius` 2.5 m of the left door (`door (-1.7, 0, -0.3)` body frame); a traffic or police car (driver is data)
  qualifies. `exit_spot` candidates left door / right door / roof with level rule + capsule + path; forced → left
  door ray without level check (the wall-top bug).
- `drive_vehicles` (chassis.rs): intent only from `vehicle.driver`; every awake car casts 4 rays; `non_waking` forces
  without a driver. A driverless dynamic car with a default intent holds below `hold_speed`, coasts at 1 m/s² above.
- `apply_bullet_hits`: `VehicleHealth -= damage × bullet_scale`, nothing else. `BulletHitVehicle{shooter, attack,
  vehicle, point, damage}` has no variance roll.
- Police: `dispatch_police` counts active foot units, spawns on sidewalk points in `spawn_ring (40, 90)`;
  `EscalationRow{units, swat, reinforce_seconds, arrest, surround}`; `police_fsm` Arrest seeks the player position,
  stops at `stand_distance` 1.0; `arrest_player` holds within `distance` 1.5.
- Fire line: `FireLine::blocked` = ahead of the shooter up to `reach` (range + overreach, i.e. PAST the target),
  perpendicular ≤ `clearance + along·tan(cone)`; `hold_fire` builds `shields`/`yielding` from living bodies.
- Wanted: `record_crimes` witnesses = `Faction::Police` characters within `cop_witness_distance` 50 m LOS;
  `track_search` uses `cop_sees` (35 m foot view, hard-coded field).
- Client: one vehicle scene for every `Vehicle` (`On<Add, Vehicle>` observer), wheel lift uses the global sedan scale;
  sirens ride cops (`pick_sirens`); `Appearance(u32)` is the existing sim→client look roll (civilians map it by
  modulo in `body_look`).

---

## 3. Revised approach (deltas to PLAN.md §2; everything not named stays as PLAN.md §2 describes)

### 3.1 Traffic (unchanged core)
New domain `crates/gta_sim/src/traffic/` + `assets/traffic/traffic.ron`, one `TrafficPlugin`. `TrafficGraph` from the
city (slot-0 lanes, slot derived from the lateral offset; citygen untouched), Bézier connectors, conflict tables,
spawn points. Every AI car = `vehicle_bundle` + `TrafficCar`. IDM + ballistic stop. Leader along the own path +
stop line + one forward shape cast. FCFS reservation of whole conflicting connectors ("don't block the box").
Kinematic actuation by closed-loop velocities. Dynamic cars use a shared pure-pursuit `Autopilot` writing the car's
own `DriveIntent`. Bubble 70/90 + 15/25 + 2 s, cap 24, InitialFill. Traffic runs in `NpcSystems` (keeps moving while
Wasted/Busted), which requires `SidewalkGraph` — test fixtures insert one (R1).

`TrafficCar.mode`: `Kinematic | Dynamic | Stalled | Bailing { attack } | Taken | Abandoned` (one enum, no marker
churn):
- `Taken`: the player sits in it (hijack); never despawned; not counted toward `max_cars`.
- `Abandoned`: dynamic, no AI, no reservations, counted toward `max_cars` (it is a physics body in the bubble), bubble
  despawn rule. Entered on: the player leaves a `Taken` car (`Driving` removed), bail-out finished, stalled for
  good. `Stalled` is merged into `Abandoned` (PLAN's Stalled did the same thing).
- Parked cars stay plain `Vehicle`s (city lifetime, not bubble).

### 3.2 Predictive switch = time to contact (R6)
FixedPostUpdate, before `PhysicsSystems::First`. For each `Kinematic` traffic car: broadphase
`shape_intersections` with the chassis box grown by `switch.reach` (mask `Character|Vehicle`, self excluded); for
each hit body that is `RigidBody::Dynamic`, not `Sleeping`, not `RigidBodyDisabled`: flat relative displacement over
the horizon `d = (v_other − v_car)·switch.horizon_seconds`; the car's rectangle (from `Position`/`Rotation`, grown by
`switch.skin`) and the other footprint (character: circle of `capsule_radius`; car: its rectangle) swept by `d`
overlap → switch. 2D SAT/segment tests, pure functions in `traffic/contact.rs` with unit rows. Backstop
`CollisionStart` kinematic↔dynamic. Cross-config: `switch.reach ≥ (vehicle.max_speed + max desired_speed) ·
horizon_seconds` (the broadphase must contain every body the sweep can reach) and `horizon_seconds ≥ 2/64`.
Worked: player car 10 m/s into a stopped car, gap 0.5 m, horizon 0.1 s → sweep 1.0 m > 0.5 → switch 3-4 ticks before
contact (probe Q4 needs ≥ 1). Crossing pedestrian 1.2 m ahead, 1.4 m/s perpendicular → sweep 0.14 m sideways,
never enters the bumper rectangle (+0.1 skin) → no switch.

### 3.3 Hijack, bail-out (Q-Б), abandonment (R4, R8, R9)
- Hijack (`VehicleEntered` for a `TrafficCar`): `RigidBody::Dynamic`, mode `Taken`, remove `Autopilot` and the own
  `DriveIntent`, release reservations; spawn the driver as a civilian at the player's pre-entry feet in
  `Flee{about: Some(Cause::Attack(entered.attack))}` (CarTheft incident already recorded by `record_crimes`).
- Leaving (`Taken` car whose `Vehicle.driver` became `None`): mode `Abandoned`.
- Bail-out: `apply_bullet_hits` classifies the hit point against `sedan.ron cabin` (body frame) and writes
  `CabinHit{shooter, attack, vehicle, point, damage}` for every cabin hit (the vehicle domain owns the zone). Traffic
  `bail_out` reads `CabinHit` for cars in `Kinematic|Dynamic`: mode `Bailing{attack}` (first hit wins), writes
  `DriverScared{shooter, attack, vehicle}`. A `Bailing` car keeps its path and reservations, target speed 0 (kinematic:
  `a = −max_deceleration` through `ballistic_step`; dynamic: `Autopilot.speed = 0`); at `speed ≤ exit_max_speed` and
  kinematic stopped (v = 0) or dynamic ≤ `hold_speed`: the driver civilian spawns at the first clear exit spot
  (`exit_spots`, left door first; none clear → waits, retried each tick), `Flee{from: shooter position, about:
  Some(Cause::Attack(attack))}`, the car becomes Dynamic `Abandoned`, reservations released.
- Crime: `wanted::record_crimes` reads `DriverScared`; when `shooter` is the player it records `Crime::Shooting`
  (heat `shooting_near_people`) with that attack at the shooter's position, `touched` (needs a witness: a cop in LOS,
  or the fleeing driver's call via TASK-026 flee-then-call). Nothing new in `wanted.ron`.

### 3.4 Police cars (Q-А a) — as PLAN.md §2.2 with R5/R11 fixes
`PoliceCarState { Respond, Chase, Dismounted, Leave, Taken, Abandoned }`. Crew counts toward `row.units`.
Re-boarding per Q-А with `reboard_timeout_seconds`. Empty car → `Abandoned`. All police-car systems
`.run_if(resource_exists::<TrafficGraph>)`. Every crew mutation (dismount, board, hijack) runs in systems ordered
before `dispatch_police` so `(foot + aboard)` is exact in the same tick (auto sync points apply the spawns/despawns).

### 3.5 Binding TASK-015 notes
- Cabin wound (O2): `CabinHit` on a car with `driver: Some(d)` → `wound = round(damage × cabin_driver_share)`,
  `Health::take`, `DamageDealt`. `VehicleSystems::Bullets.before(HealthSystems::Death)`.
- Pull-out at 1 star (O2) — as PLAN step 18.
- Cars in the fire line (O1, R7): `hold_fire` gets `cars: &[CarRect]` (flat centre, yaw, half extents x/z);
  a car blocks when the SEGMENT shooter→target passes within `clearance` of its rectangle (no cone, nothing past the
  target); the car the target drives is excluded; cars never yield. `usable`, `pinned`, `queue_slot` test
  `shields ∪ cars`. One shared pure fn `car_blocks(from, to, car, clearance) -> bool` in `tactics/fire_line.rs`.
- Forced eject (note 3) — as PLAN step 16.

### 3.6 Models (Q-В b)
Sim: traffic cars carry `Appearance(TrafficRng.next_u32())` (the existing look-roll component; the sim never names an
asset). Client: `render.ron` gets `police_vehicle`, `taxi_vehicle` (same struct as `vehicle`) and
`taxi_share: 0.25`; a `TrafficCar` whose roll `a.0 as f32 / 2^32 < taxi_share` gets the taxi; `PoliceCar` → police;
else sedan. Scale by length match to the 4.08 m chassis: sedan 1.6 (unchanged), taxi `4.08/2.75 = 1.48`,
police `4.08/2.90 = 1.4`; `offset.y = −rest_height` (−1.16); `offset.z = −scale·(body node z + body mesh centre z)`:
taxi −1.48·(−0.025+0) → +0.037 after the 180° yaw → offset −0.037; police −1.4·(−0.025−0.075) → offset −0.14
(sedan check: 1.6·−0.025 → −0.04 = shipped). Owner-run for the look.

---

## 4. Revised steps (complete)

Ticks are 64 Hz. Every tuning number lives in the named RON file; the only new `const`s are laws, named where added.
Read `maw/project-context/domains/{bevy-ecs,gates,game-design}.md` before coding.

### Phase A — data and graph

**Step 1. `assets/traffic/traffic.ron` + `traffic/config.rs` `TrafficConfig`** (`deny_unknown_fields`, `validate()`,
loaded in `compose_sim`, `TRAFFIC_CONFIG = "traffic/traffic.ron"`).
```
idm: (time_headway: 1.5, acceleration: 0.73, comfortable_deceleration: 1.67, min_gap: 2.0, max_deceleration: 8.0),
desired_speed: (avenue: 16.0, street: 12.0),
turn_speed: 6.0,
look_ahead: 60.0,
sense_distance: 25.0,
turn_sense_distance: 6.0,
connector_samples: 8,
conflict_margin: 0.3,
bubble: (max_cars: 24, in_view: (spawn: 70.0, despawn: 90.0), off_view: (spawn: 15.0, despawn: 25.0),
         offscreen_seconds: 2.0, spawns_per_tick: 1, initial_spawns_per_tick: 4, spawn_spacing: 10.0),
switch: (reach: 5.0, horizon_seconds: 0.1, skin: 0.1),
lost: (distance: 4.0, angle_deg: 60.0),
```
`validate`: finite; positive where physical; `min_gap > 0`; `max_deceleration ≥ comfortable_deceleration`;
`off_view.spawn < off_view.despawn ≤ in_view.spawn < in_view.despawn`; `max_cars ≥ 1`; per-tick counts ≥ 1;
`connector_samples ≥ 3`; `0 < lost.angle_deg < 180`; `horizon_seconds ≥ 2/64` (law: tick from `Time<Fixed>`
default, as `compose_sim` already does); `skin ≥ 0`. Cross-config in `compose_sim`: `switch.reach ≥
(vehicle.max_speed + max(desired_speed)) · horizon_seconds` (44 · 0.1 = 4.4 ≤ 5.0) and `in_view.despawn <
population.despawn_distance`. `const IDM_DELTA: i32 = 4` (law: part of the GDD formula).
Check: step 26.

**Step 2. Other data files.**
- `assets/police/escalation.ron`: each star row gets `cars: 1..5` (GDD §6.4); `arrest.pull_out_seconds: 1.0`; new
  block `car: (crew: 2, spawn_ring: (60.0, 120.0), spawns_per_tick: 1, pursuit_speed: 20.0, turn_speed: 8.0,
  dismount_distance: 20.0, direct_chase_distance: 40.0, stopped_seconds: 1.0, reboard_distance: 25.0,
  reboard_timeout_seconds: 10.0, route_refresh_seconds: 1.0, routes_per_tick: 1)`. Types in `police/mod.rs`;
  validate rows `cars` non-decreasing and ≥ 1, `crew ≥ 1`, ring band and `< population.despawn_distance` (extend
  `validate_ring`), `dismount_distance < direct_chase_distance`, positives. **Re-anchor
  `tests/config_police.rs every_star_has_a_row`**: its literal gains `, cars: 5` exactly as written in the file
  (R3); re-run its flip (row deleted → "length 5").
- `assets/wanted/wanted.ron`: `cop_car_view_distance: 50.0`, validated positive and ≥ `cop_view_distance`.
- `assets/vehicle/sedan.ron`: `cabin: (centre: (0.0, 0.45, 0.1), half_extents: (1.25, 0.5, 1.0))`, `autopilot:
  (lookahead_min: 4.0, lookahead_per_mps: 0.5, speed_gain: 0.5, stuck_speed: 0.5, stuck_seconds: 2.0,
  reverse_seconds: 1.5)`; validate (half extents > 0, lookahead_min > 0, positives).
- `assets/vehicle/damage.ron`: `vehicle.cabin_driver_share: 0.5` in [0, 1].
- `assets/world/render.ron`: `police_vehicle: (model: "third_party/car-kit/police.glb", scale: 1.4, offset: (0.0,
  -1.16, -0.14), wheels: (4 names))`, `taxi_vehicle: (model: "third_party/car-kit/taxi.glb", scale: 1.48, offset:
  (0.0, -1.16, -0.037), wheels: (…))`, `taxi_share: 0.25` in [0, 1] (§3.6 derivation in the comment).
- `assets/audio/mix.ron`: `siren.car_height: 1.0` (m above the chassis centre).
- `assets/third_party/manifest.ron` car-kit pack: add `police.glb` (sha `a617b880594f56239b7ac4cf6fd4d74ebdae3ba6c5d842e77c76a5906eff352a`)
  and `taxi.glb` (sha `3803539718ffd3b84b515dbce8ed6b489f1ff5be58edb1903d2c5db5c584bdc7`); fetch with `python
  tools/fetch_assets.py --cache maw/tasks/done/TASK-015/scratch/carkit`; `src/visuals/config.rs vehicle_asset_paths`
  yields all three models.
- `tests/new_city.rs:301,306` literals: `PoliceDispatcher{.., cars: 0}`, `ArrestAttempt{.., pull: 0.0}` (R3).
Check: `cargo test -p gta_sim --test config --test config_police --test config_vehicle --test asset_manifest -j 4`.

**Step 3. `traffic/graph.rs` `TrafficGraph`** — as PLAN step 3 (API `new` / `from_layout` / `pose` / `length` /
`nearest`, slot rule `round(|cross(d_edge, lane.from − node_a)| / lane_width − 0.5) == 0`, Bézier connectors,
conflict = same source, same destination, or polylines closer than `2·half_width + conflict_margin`, only within
one node; errors for dead-end lanes / NaN; built on `OnTransition{Loading→Playing}` `.run_if(resource_exists::<City>)`,
`AppExit::error()` on error; dropped on `NEW_CITY`). Unit rows as PLAN (slot rows, right-angle Bézier with
midpoint (1.25, 0, −3.75), + intersection conflict rows).
Add to the seed property gate (step 26): every connector polyline, grown by the car half width, stays off every
city block polygon (sidewalk corners) — a kinematic car does not collide with static geometry, so a corner cut
would be a silent clip through the curb.

**Step 4. `traffic/idm.rs`** — as PLAN step 4 (`idm_acceleration`, `ballistic_step`, worked rows verified).

### Phase B — the traffic loop

**Step 5. `traffic/mod.rs`.**
- `TrafficCar { segment, s, speed, next: Option<u32>, mode: TrafficMode, waiting: Option<u64> }` (Reflect,
  `#[require(Offscreen)]`); `TrafficMode { Kinematic, Dynamic, Bailing { attack: u32 }, Taken, Abandoned }`.
- `TrafficRng` (ChaCha8, `set_stream(4)`, reseeded `OnEnter(Loading)` from `CitySeed`).
- `TrafficIntersections`, `TrafficStats` (Reflect: `cars, kinematic, dynamic, abandoned, taken, spawned, despawned,
  casts, switches_by_cause: [u32; N]` where cause ∈ {Character, Vehicle, Backstop, Hijack}), `TrafficPhase`.
- Messages `DriverScared { shooter, attack, vehicle }` (Reflect, cleared on `NEW_CITY`).
- Sets (FixedUpdate, `.in_set(NpcSystems)`, `.run_if(resource_exists::<TrafficGraph>)`):
  `TrafficSystems::Hijack.after(VehicleSystems::Enter)`, `Bail.after(VehicleSystems::Bullets).before(WantedSystems)`,
  `Drive.after(Hijack).after(Bail).before(VehicleSystems::Drive)`, `Bubble.after(Drive)`.
  `VehicleSystems::Drive.after(PoliceSystems)`; `VehicleSystems::Bullets.before(HealthSystems::Death)`.
  FixedPostUpdate: `contact::switch_to_dynamic.before(PhysicsSystems::First)`.
- `lib.rs`: load/validate `TrafficConfig` (+ cross-config), insert, `app.add_plugins(TrafficPlugin)` next to
  `VehiclePlugin` outside the tuple.
Check: any test builds the app (a schedule cycle panics at build).

**Step 6. `vehicle/autopilot.rs`** — as PLAN step 6 (pure pursuit, P speed control that never reverses by accident,
stuck/reverse; three directional rows + mirrored). `drive_vehicles`: intent = driver's else the car's own
`DriveIntent`; skip bodies with `!RigidBody::is_dynamic()` (kinematic cars get their wheel state from traffic).

**Step 7. `traffic/drive.rs advance_traffic`** — as PLAN step 7 (sorted by `Entity` bits; dynamic re-projection +
`lost` → `Abandoned`; occupancy; FCFS grants with don't-block-the-box; gap = min(leader, stop line, forward cast);
IDM; stop-line clamp; kinematic actuation incl. `LinearVelocity.y = (rest_height − y)/dt`; dynamic → Autopilot).
`Bailing` cars: target speed 0 as §3.3 (keep path, keep reservation until released by leaving the connector or on
turning Abandoned). `Taken`/`Abandoned` cars are skipped (the forward cast of others sees them: the cast excludes only
Kinematic traffic).

**Step 8. `traffic/contact.rs`** — time-to-contact switch §3.2 (pure fns `swept_circle_hits_rect`,
`swept_rect_hits_rect` with unit rows incl. the two worked rows above) + `CollisionStart` backstop. Switch =
`insert((RigidBody::Dynamic, SleepingDisabled, Autopilot{..}, DriveIntent::default()))`, mode `Dynamic`, stats by cause.

**Step 9. `traffic/spawn.rs`** — as PLAN step 9 with: frame test = any of the four chassis-top corners or the
centre line inside the cone (R12), `in_frame = that && flat_distance ≤ in_view.despawn`; despawn iff mode ∉ {Taken}
and `Offscreen ≥ offscreen_seconds` and distance > `off_view.despawn` (Abandoned included; release reservations);
count toward `max_cars` = every non-Taken `TrafficCar`. Spawn inserts `Appearance(TrafficRng.next_u32())`.

**Step 10. `traffic/hijack.rs`** — `on_hijack` (§3.3) + `on_leave` (Taken car with `driver == None` → Abandoned).
Civilian spawn: `civilian_bundle` on the nearest sidewalk edge then `Transform` override to the pre-entry feet;
`roll_temperament` generalised to take a unit-roll closure (R12), rolls from `TrafficRng`.

**Step 10b. `traffic/bail.rs bail_out`** (TrafficSystems::Bail) — §3.3: reads `CabinHit`, sets `Bailing`, writes
`DriverScared`; the stop-and-exit transition runs in `advance_traffic` (step 7) using `vehicle::exit_spots`.

### Phase C — police cars (`police/cars.rs`, split `car_route.rs` if > 750 lines)

**Step 11. Components, dispatch.** `PoliceCar { state, crew: Vec<UnitKind>, stopped: f32, reboard_left: f32 }`,
`PoliceCarRoute`, `CrewOf { car }`, `PoliceCarRng` (stream 5). `PoliceDispatcher.cars`. `dispatch_police_cars` in
the `PoliceSystems` chain before `dispatch_police`, `.run_if(resource_exists::<TrafficGraph>)`: active cars =
Respond|Chase|Dismounted; spawn while `cars < row.cars` and `units + 1 ≤ row.units`, crew `min(car.crew, row.units −
units)` (SWAT first by `spawn_kind`), at most `car.spawns_per_tick`, on traffic spawn points in `car.spawn_ring`,
hidden (`outside_cone` of all corners, or `occluded` within the shared `PopulationLoad` budget), free box,
`pick_spawn` near `last_known`. `dispatch_police` counts crew aboard at the car position.

**Step 12. `drive_police_cars`** — as PLAN step 12, state table corrected (R5):
any → Leave at 0 stars (except Taken/Abandoned); Respond ↔ Chase as PLAN; Respond → Dismounted as PLAN;
Dismounted → Respond iff crew aboard ≥ 1 and (no live `CrewOf` outside or `reboard_left` ≤ 0; stragglers lose
`CrewOf`); Dismounted with crew aboard 0 and no live `CrewOf` → Abandoned; Taken/Abandoned/Leave terminal.
`reboard_left` starts at `reboard_timeout_seconds` when the player drives farther than `reboard_distance`.
Senses use `cop_sees(.., distance)` with `cop_car_view_distance` (R12). Pure `next_car_state` table-gated (one case
per row, TASK-011 lesson).
Dismount/board/hijack all run before `dispatch_police` in the same tick (§3.4).

**Step 13. `despawn_police_cars`** — Leave and Abandoned cars: traffic bubble rule; engaged cars: off frame ≥ 2 s and
beyond `population.despawn_distance`; Taken: when the player leaves → Abandoned (R4). `CityScoped`; reset
`PoliceDispatcher.cars` on `NEW_CITY`.

**Step 14. Hijacking a police car** — as PLAN (crew dismounts, excluding the player's spot; `Taken`).

**Step 15. Wanted: cars see and witness** — as PLAN, via `cop_sees(.., cop_car_view_distance)` from `car_eye =
position + rot·seat` (inside the hull, so `sight_blocked` skips the own car).

### Phase D — binding notes and Q-Б crime

**Step 16. `vehicle/seat.rs` exit spots** — as PLAN step 16 (`candidates`, `clear_spots`, `exit_spot(forced)` with
level-only fallback then the roof point; `pull_out`, `exit_spots` pub(crate)).

**Step 17. Cabin (`vehicle/impact.rs`)** — `apply_bullet_hits`: body-frame test against `cabin`; for a cabin hit
write `CabinHit`; if `driver: Some(d)` wound as PLAN step 17 (worked rows kept: side window 13, hood 0, low door 0,
roof 13). `CabinHit` message registered in `VehiclePlugin`, cleared on `NEW_CITY` with the other vehicle messages.

**Step 18. Pull-out (`police/arrest.rs`)** — as PLAN step 18 (`ArrestAttempt.pull`, `pull_out_driver` before
`arrest_player`, cop walks to the door point). Risk: a cop approaching from the far side walks into the car body
(movement checks are walls-only) — gate places the cop on the door side; the far side is owner-run.

**Step 18b. `wanted/crimes.rs`** — read `DriverScared`; player shooter → `crimes.record(Crime::Shooting, shooter, None,
attack, shooter_pos, now, merge)`, pushed to `touched` (R8).

**Step 19. Cars in the fire line (`tactics`)** — §3.5 / R7: `CarRect`, `car_blocks`, `hold_fire(.., cars)`; police
(`behavior.rs`) and gangs (`gang/behavior.rs`) build the list once per system run (cars within weapon reach + car
half diagonal of any shooter), excluding the car the target drives.

### Phase E — presentation and QA

**Step 20. Client.**
- `src/visuals/vehicle.rs`: `VehicleVisualAssets` holds three scene handles; `spawn_vehicle_model` picks by
  `Has<PoliceCar>` / `Option<&Appearance>` + `Has<TrafficCar>` (all in the spawn bundle) — police, taxi by
  `taxi_share`, else sedan; `VehicleModel` stores its `scale` and wheel names; `animate_wheels` uses the model's
  scale (R10). Kinematic cars' wheel states come from the sim.
- `src/audio/loops.rs update_sirens`: candidates = active `PoliceCar`s (Respond/Chase/Dismounted) within `audible`,
  else foot cops as today; height `siren.car_height` on cars. New gate `sirens_ride_police_cars` in `event_gate.rs`.
- `src/minimap/markers.rs`: `MarkerKind::PoliceCar`.
- `tools/qa/scenarios/t14.py` keeps parked cars only (no `TrafficCar`/`PoliceCar`); `t13.py` accepts car siren
  parents. Rerun t8…t14.
Check: `cargo test -p gta_like --bin gta_like -j 4` three times.

### 4.1 Gates (headless unless noted; class + flip named)

Common fixture `tests/traffic_support/mod.rs`: `traffic_floor(lanes, connectors)` = `headless_app()` +
`test_graph` with a sidewalk graph far from the lanes (e.g. (30,0,−30)-(35,0,−30)) + the synthetic `TrafficGraph`;
asserts `GATE BROKEN: traffic systems did not run` if no car moved in the first 64 ticks (R1). Lanes in z ∈ [20, 38],
x ∈ [−38, 38] (clear of the z 14 wall and z 10 boxes); `GATE BROKEN` if a car collider touches a `Block` at spawn.
`obb_overlap` from `Position`/`Rotation` only.

- **21 `traffic_idm.rs` — AC1 (correctness)** — as PLAN (10 cars, loop, gaps from [1, 8] with one 1.0 m gap,
  8 seeds × 6400 ticks, speed ≥ 0, `LinearVelocity·tangent ≥ −1e-4`, no OBB overlap, each car ≥ one loop; also
  |yaw − path tangent yaw| < 2° every tick (kinematic angular integration)). Flips: drop the `v' < 0` branch; leader
  search limited to the own segment.
- **22 `traffic_intersection.rs` — AC2 (correctness)** — as PLAN (conflict points computed by the test, ≤ 1 car per
  point every tick, liveness per approach, flips: grant always; table without the geometric test).
- **23 `traffic_contact.rs` — AC4 (correctness)** — rows: (a) player car into the rear of a stopped kinematic car:
  Dynamic in a tick BEFORE the first `CollisionStart` of the pair, 16 ticks after contact speed ≥ 1.5 m/s, player car
  never reverses (flip: disable the predictive switch, keep the backstop → RED); (b) player on foot into the side of
  a stopped car → Dynamic before contact; (c) no switch in 640 ticks for a sleeping parked car 0.85 m beside a
  passing car and a car in the next lane at equal speed; (d) NEW: a dummy/civilian walking across 1.2 m in front of a
  stopped car's bumper at walk speed → no switch (flip: horizon test replaced by "inside reach" → RED).
- **24 `traffic_bubble.rs` — AC3 (correctness)** — as PLAN (seed 1, civilians/gangs 0, view turning, own frame
  computation from the published `CameraView` with the four roof corners; rows `despawn_needs_two_seconds_off_frame`,
  `in_frame_car_is_kept`, `far_in_cone_car_goes`, `spawn_bands`); NEW row `abandoned_cars_are_despawned`: hijack,
  drive 10 m, get out, walk away 40 m with the car off frame → the car entity is gone after ≥ 128 ticks off frame
  (flip: exclude Abandoned from despawn → RED) (R4).
- **25 `traffic_hijack.rs` (correctness)** — as PLAN (Taken, Dynamic, no Autopilot/own intent, one fleeing civilian
  with `about: Some(Attack(entered.attack))`, CarTheft incident, exited car stays within 0.2 m; flip: keep the own
  `DriveIntent`) + after exit mode `Abandoned`.
- **25b `traffic_bailout.rs` (correctness, Q-Б)** — synthetic straight lane, a kinematic car at 12 m/s, player
  pistol (`set_aim`/`set_action`) from 20 m to the side (beyond `shooting_radius` 15 m, no person near): (a) side
  window → mode Bailing, the car stops within `v²/(2·max_deceleration) + 1 m` = 10 m, then exactly one `Civilian`
  within 1 m of the left or right door exit spot in `Flee{about: Some(Attack(shot))}`, car Dynamic `Abandoned`,
  `Crimes` holds a `Shooting` incident with that attack; after the flight + call (TASK-026 fixture practice: calm
  temperament with `report` weight forced high) a `PoliceCall` resolves to that incident and heat rises by
  `shooting_near_people`; (b) hood hit → no Bailing, no civilian; (c) a second cabin hit while Bailing → still one
  civilian. Flips: cabin test removed → (b) RED; `DriverScared` not read by `record_crimes` → (a) crime/heat RED.
  Numbers derived by the implementer from the real call path before asserting (gates domain).
- **26 `config_traffic.rs` + `traffic_graph.rs`** — sabotage per rule strictly on the failing side, distinct error
  texts; cross-config (`switch.reach` 4.0 < 4.4 → error; `horizon_seconds` 0.02 < 2/64 → error). Graph gate seeds
  1..=8: `from_layout` Ok, every lane has out connectors, no curb lane, connectors per ≥ 2-edge intersection, no
  connector corridor enters a block polygon.
- **27 existing gates re-anchored** — as PLAN step 27 (`police_bench` pins `cars = 0`; `police_city` twice;
  `new_city` literals + state-reset list incl. `TrafficIntersections`, `TrafficPhase`, `TrafficStats`,
  `PoliceDispatcher.cars`, "no traffic car survives a new city"; `sensor_leak` with traffic, a hijack and a bail-out
  in the window; other city gates: record the failure, fixture `max_cars = 0` only if the subject is not traffic,
  never loosen a threshold) + `config_police every_star_has_a_row` string (R3).
- **28 `police_cars.rs` — AC5** — as PLAN (`cars_follow_row_1..5`, every tick `cars ≤ row.cars`, `units ≤ row.units`,
  foot + aboard == `dispatcher.units`; `car_closes_in_and_dismounts`; `car_chases_a_driver`; `crew_reboards`;
  `next_car_state_table` one case per row incl. the new Abandoned/timeout rows; `car_sees_at_50_m`;
  `crewed_car_witnesses_theft`) + NEW `dead_crew_frees_the_slot`: kill both dismounted cops → car Abandoned and a new
  car is dispatched within the row (flip: PLAN's "Dismounted → Respond when no CrewOf outside" → RED, the empty car
  keeps the slot). Row flip: count only foot units → row 2 overruns.
- **29 `police_pull_out.rs`** — as PLAN.
- **30 `vehicle_hits.rs` cabin** — as PLAN (13 / 0 / 0 / 13; SMG kill → Wasted, eject at ground level).
- **30b `police_fire_lines.rs` — O1** — rows: (a) parked car corner 0.4 m off the line at 10 m, player 20 m away: no
  `BulletHitVehicle` on it in 1920 ticks and ≥ 1 cop hit on the player (it repositions), mirrored and at 3
  distances; (b) player drives car X: the cop fires into X (target car excluded); (c) gang gunman version of (a);
  (d) NEW car 1.0 m beside the segment at 10 m → the cop fires without repositioning (flip: add the spread cone to
  the car test → RED; pick the offset so `clearance < offset < clearance + 10·tan(cone)` from the shipped
  numbers); (e) NEW car 3 m behind the player on the line → the cop fires (flip: test up to `reach` → RED).
  Flips of (a)/(b): empty car list; exclude nothing.
- **30c `traffic_bench.rs`** — as PLAN (liveness + order of magnitude; mean only; record vs GDD §11 4 ms).
- **30d `traffic_parked.rs`** — as PLAN (zero traffic↔parked `CollisionStart`, zero switches caused by parked cars
  via `switches_by_cause`, no OBB overlap; flip: traffic on slot 1).
- **31 `vehicle_seat.rs` forced eject** — as PLAN.

**Step 32. `tools/qa/scenarios/t15.py`** — as PLAN step 32 (≥ 12 traffic cars, speeds, hijack with `F`, heat to 2
stars via `mutate_resources`, ≤ 2 police cars, nearest distance decreases, screenshots every 1 s, dismount,
`get_diagnostics` + `Game.frame_report()`, no ERROR words; enum names derived from source) + read `Appearance`-based
model mix is not asserted (owner-run). Owner checklist adds: a shot driver bails out and runs; taxis in the flow.

**Step 33. Docs** — GDD §6.4 one line (pull-out at 1 star, T15 exception); GDD §5.3 one line (Q-А: dismounted cops
return to their car if the player drove off); GDD §5.2 one line (Q-Б: a shot into a traffic car's cabin makes the
driver stop, get out and flee; Q-В: taxis in the flow by a look roll). `docs/architecture/traffic.md` (graph, modes,
TTC switch, reservation, bubble, bail-out, police-car states, probe facts). After merge: narrative-graph, README,
AGENTS.md "Проект" per the project rule.

### 4.2 Final checks (implementer)
`cargo build -j 4`; `cargo clippy -j 4 -- -D warnings`; `cargo clippy --workspace --all-targets -j 4 -- -D warnings`;
`cargo test -p gta_sim -j 4`; `cargo test -p citygen -j 4`; `cargo test -p gta_like --bin gta_like -j 4` ×3;
`python tools/qa/tree_check.py`; `cargo tree -p gta_sim -e normal -i bevy_render` empty; `t15.py` + reruns of
t8…t14. Files < 750 lines (`traffic/drive.rs`, `police/cars.rs` are the likely splits).

---

## 5. Risk areas (updated)

1. **Schedule ordering.** New edges: `VehicleSystems::Drive.after(PoliceSystems)`, `Bullets.before(Death)`,
   `TrafficSystems::Bail.after(Bullets).before(WantedSystems)`. Verified no existing edge closes a cycle; a cycle
   would panic at build in every test.
2. **Test plumbing (R1).** Traffic in `NpcSystems` needs a `SidewalkGraph`; the fixture's liveness `GATE BROKEN` is
   the guard.
3. **Dynamic traffic quality** (pure pursuit on T14 chassis, connectors) — data + `lost` → Abandoned; owner-run.
   The TTC switch (R6) keeps the dynamic population small; `switches_by_cause` in the bench shows the rate.
4. **Jams.** An Abandoned/bailed car in a lane stops the queue behind it until the bubble removes it (never in
   frame). Owner-run; a lane-change/overtake is out of scope.
5. **Car-entity budget (R4).** Every AI car path ends in the bubble; leak gate rows 24/`sensor_leak`.
6. **Unit accounting.** Crew counted aboard; dismount/board/hijack before `dispatch_police`; row gates check
   foot + aboard == units every tick.
7. **Fire line (R7).** The segment-only car rule may let a bullet that misses the player hit a car behind him —
   accepted (cars are not bystanders); rows (d)/(e) pin the intent.
8. **Pull-out approach from the far side** — cop may push against the car body; owner-run.
9. **Bail-out timing.** A Bailing car blocks its lane and reservation for ~2 s while braking (by design; GTA-like).
10. **Performance.** N ≤ 24 casts + broadphase queries, one A* per tick for police, analytic car tests in hold-fire.
    Bench 30c records against GDD §11.
11. **Visuals** (police/taxi scale and offsets, siren height, no visible traffic driver) — owner-run.

## 6. Open questions

None new. Q-А/Б/В/Г are decided in TASK_FINAL.md and folded in above. Crew aboard a police car is not wounded by a
cabin shot (data crew; not required by O2/Q-Б) — owner-run note, not a question.

children: 0 launched / 0 reported.
