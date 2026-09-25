# PLAN — TASK-016 (GDD T15: traffic and police cars)

Cost of error: HIGH, silent class. Kinematic bodies push with infinite mass, two kinematic cars pass through each
other without any solver response (probe below), an intersection that grants two conflicting movements, a despawn
in view, a unit cap that grows past the GDD budget, and frame cost for the largest population of the game all break
without a visible error. Full evidence layer for the traffic core, the dispatcher caps and the binding notes. Feel
(traffic density, turn look, chase handling, sirens, police model) goes to the owner's run.

Scope law: GDD §5.2, §5.3, §6.1, §6.4, §6.5, §7, §8, §11, §12, §13 T15, plus the three binding TASK-015 notes (cabin
wound + pull-out arrest at 1 star, cars in the fire line, forced-eject feet height). Pull-out is a deliberate
extension of GDD §6.4 "вне MVP" (authorised by the task); step 30 amends the GDD by one line.

Evidence written by this planner (all under `scratch/`):
- `scratch/probe_kinematic/` — avian3d 0.7.0 probe (own workspace, `Cargo.lock` copied from the repo, built with
  `CARGO_TARGET_DIR=D:/test-gta-like/target --offline --release -j 4`). Output (verbatim):
  - Q1 kinematic body moved `(10.000001, 0, 0)` in 64 ticks at `LinearVelocity 10` — kinematic motion by velocity is exact.
  - Q2a `CollisionStart` kinematic vs ground at tick 1 with the box 0.1 m ABOVE the ground (speculative contact).
  - Q2b two kinematic boxes: `CollisionStart` at tick 72 and `kin_1` ends at x 9.92 inside `kin_2` — kinematic pairs
    get events and NO solver response.
  - Q3 `insert(RigidBody::Dynamic)` from `FixedPostUpdate.before(PhysicsSystems::First)` at tick 5: tick 5 already
    `Dynamic`, `vel.y -0.1533` (= -9.81/64); `ComputedMass` exists while kinematic.
  - Q4 dynamic box at 10 m/s into a resting kinematic box: no switch → dynamic stops dead (vel ≈ 0), kinematic 0
    (wall). Switch one tick before contact → both ≈ -2.1 m/s (momentum shared).
- `scratch/carkit/glb_probe.py` — Kenney car-kit zip (cached at `maw/tasks/done/TASK-015/scratch/carkit/kenney_car-kit.zip`,
  sha256 `fac7dac…d0c4` = manifest `archive_sha256`): `police.glb` sha256
  `a617b880594f56239b7ac4cf6fd4d74ebdae3ba6c5d842e77c76a5906eff352a`, nodes `wheel-front-left/right`,
  `wheel-back-left/right`, `body` (+ child `grill`), body bounds x ±0.75, y 0..1.10, z -1.525..1.375 (model units);
  same texture `Textures/colormap.png` as the sedan.
- No new crate. Every dependency (bevy 0.19.1, avian3d 0.7.0, pathfinding 4.16.0, rand_chacha 0.10.0) is already in
  `Cargo.lock`; no lock change is needed.

---

## 1. Understanding (today's code)

### Lane graph (citygen, untouched by this plan)
- `crates/citygen/src/layout.rs:104-120` — `LaneGraph { lanes: Vec<Lane{edge, from, to}>, connectors:
  Vec<Connector{from, to, intersection}> }`. No lane slot, no curve, no conflict data.
- `crates/citygen/src/graphs.rs:83-135` — lanes per road edge per direction, `i in 0..lanes_per_direction`, shifted
  `right * (i + 0.5) * lane_width` from the road centre line (`right = d.perp()`, right-hand traffic); trimmed by the
  largest half carriageway at each node. Slot 0 = innermost lane. Connectors: every incoming × outgoing lane of a
  node on a DIFFERENT edge (no U-turns). Alleys have no lanes.
- `crates/citygen/src/parking.rs:6-35` — parked spots on the avenue CURB lane (`from_centre = half_carriageway -
  curb_offset` = 6.5 − 1.625 = 4.875 m), from 15 m to len−15 m of each edge, heading = lane direction.
- Geometry check: car half width 1.2 (`sedan.ron chassis_half_extents`), lane width 3.25. A car on the inner lane of
  an avenue and a parked car on the curb lane are 3.25 m apart centre to centre → 0.85 m clear. Opposite inner lanes
  are 3.25 m apart → 0.85 m clear. Street: car edge 0.425 m from the 3.25 m curb.
- `crates/citygen/tests/properties.rs:63` — the full lane graph is strongly connected (seed sweep).

### Vehicle domain (T14)
- `crates/gta_sim/src/vehicle/mod.rs:36-48` `Vehicle { driver: Option<Entity>, steer, on_sidewalk, taken, wheels }`
  requires `VehicleHealth, PreStepVelocity, CityScoped`; `vehicle_bundle` (`:135-159`) = `RigidBody::Dynamic`,
  convex-hull chassis, density, CoM, layers `Vehicle` vs `World|Character|Vehicle`, `CollisionEventsEnabled`.
  `spawn_parked_cars` (`:161-174`) on `OnTransition{Loading→Playing}`. Sets: `Enter` (before `HealthSystems::Damage`),
  `Impact` (in Damage), `Bullets` (after Damage, unordered vs `Death`), `Drive` (after Enter/Bullets/Impact);
  FixedPostUpdate `Record` before `PhysicsSystems::First`, `Seat` after `Last`. `VehiclePlugin` is added outside the
  15-plugin tuple in `lib.rs:185-186`.
- `chassis.rs:80-194 drive_vehicles` — intent comes ONLY from `vehicle.driver`'s `DriveIntent` (`:101-105`), casts 4
  wheel rays for every awake car (kinematic ones too), forces via `non_waking` when no driver.
- `seat.rs:281-349 exit_spot` — candidates left door / right door / roof with expected feet level, valid when the
  feet ray is within `step = float_height − capsule_height/2` of it, the capsule is free and the way from the seat is
  free. `None if forced` (`:339-342`) falls back to the LEFT door ray with NO level check → the TASK-015 wall-top bug.
  `enter_exit` (`:384-478`): entry to the nearest car with `driver == None` and speed ≤ `exit_max_speed` (3 m/s)
  within `enter_radius` (2.5 m) of the left door; inserts `Driving`, `RigidBodyDisabled`, `ColliderDisabled` (+ head
  hitboxes), `TnuaToggle::Disabled`, `SleepingDisabled` on the car; writes `VehicleEntered{first: !taken}`.
  `eject_all` (`:482-511`) on `OnEnter(Wasted|Busted)` calls `exit_spot(.., forced = true)`.
- `impact.rs:329-433 apply_impacts` — `CollisionStart` + pre-step velocities; character contact → pedestrian damage,
  `VehicleHit`; else car health loss unless an underbody scrape. `apply_bullet_hits` (`:435-447`) only lowers
  `VehicleHealth` → a seated driver is never hurt (premise challenge confirmed).
- `combat/hitscan.rs:103-111, 286-301` — a pellet whose target is not a `Health` body and whose collider is on the
  `Vehicle` layer writes `BulletHitVehicle{shooter, attack, vehicle, point, damage}` (damage = base × falloff, no
  variance roll).

### Police and wanted (T10/T11)
- `police/mod.rs:37-49` `EscalationConfig` (rows `units, swat, reinforce_seconds, arrest, surround`; no cars),
  `:303-320` `PoliceUnit`, `:351-356` `PoliceDispatcher{units, swat, reinforce_left}` (reflected, read by t11.py and
  `new_city.rs:301` as a struct literal), `PoliceRng` stream 3 (`:329`). Streams in use: NpcRng 1, gangs 2, police 3.
- `police/dispatch.rs:539-641 dispatch_police` — foot units up to the row on sidewalk spawn points in the
  `spawn_ring`, hidden from the camera, `pick_spawn` near the last known position.
- `police/behavior.rs:114-404 police_fsm` — Arrest seeks the player's position (`:311-321`); Attack uses
  `hold_fire` (`:337-354`) with bodies only.
- `police/arrest.rs:409-478 arrest_player` — query `Without<Driving>` (`:419`): a driver is never arrested.
- `tactics/mod.rs:168-227 hold_fire`, `tactics/fire_line.rs:283-303 FireLine::blocked/blockers` — flat 2D line test
  against POINT bodies with one `clearance` (= capsule radius 0.3 + `fire_line_margin` 0.2 = 0.5 m) plus the spread
  cone; `Blocked{shields, yielding, bodies}` drives reposition. Cars are not in any list (note O1).
- `wanted/search.rs:420-455 track_search` — "seen" only by `Faction::Police` characters with
  `cop_view_distance` 35 m; GDD §6.4 wants 50 m from a car. `crimes.rs:298-301` witnesses = police characters.
- `wanted/crimes.rs:248-262` — `VehicleEntered{first}` records `Crime::CarTheft`; reported when a cop witnesses or a
  civilian calls about `Cause::Attack(theft.attack)` (`take_calls` → `Crimes::resolve`).

### Population / NPC
- `population/mod.rs:146-180` `ViewCone`, `CameraView(Option<ViewCone>)`, `Offscreen(f32)`; `outside_cone`
  (`:246-249`), `occluded` (`:257-278`); despawn rule of civilians = off-frame ≥ `despawn_offscreen_seconds` and
  past `despawn_distance` (`:353-410`). No spawn without a published `CameraView`.
- `civilian/mod.rs:201-222 civilian_bundle` (wandering on a `GraphWalker`), `CivilianState::Flee{from, left, about}`;
  `call_later` makes a fleeing witness with `about: Some(cause)` phone it in with `call_after_flee` 0.8.
- `navigation/mod.rs:221-253` A* with `pathfinding::astar` and integer centimetre costs; `NpcSystems` requires
  `SidewalkGraph` (`:399-402`).
- `character/mod.rs:110,125-134` — an observer despawns Tnua sensors of any despawned character (TASK-025): every
  cop/civilian despawn path in this plan is safe.

### Client
- `src/visuals/vehicle.rs` — one model (`render.ron vehicle`, sedan) on every `Vehicle` (`spawn_vehicle_model`
  observer `On<Add, Vehicle>`); wheels follow `vehicle.wheels[i]` (grounded + compression) → a car whose wheel state is
  never written shows hubs sunk by the static compression (0.11 m).
- `src/audio/loops.rs:195-268 update_sirens` — sirens ride the nearest live `PoliceUnit`s (`pick_sirens`, 2 emitters);
  gate `src/audio/event_gate.rs:179 sirens_ride_live_cops`, and `event_gate.rs:161` builds a `PoliceUnit` literal.
- `src/minimap/markers.rs:98-170 sync_markers` — cops `Police`, cars `Vehicle` (grey), no police car kind.
- Runtime scenarios that will see traffic: `t14.py:79-84` takes the nearest `Vehicle` (could be a moving traffic car),
  `t13.py:364-384` requires every siren parent to be a live cop, `t11.py:171-181` checks `dispatcher.units <= row`.

### Tests that the new world touches
- Test area (`world/test_area.rs`, floor ±40 m, obstacles listed there) has no lane graph → no traffic, no police
  cars; all test-area gates stay as they are.
- City gates with a `CameraView` (e.g. `police_city.rs`, `police_bench.rs`, `civilian_city.rs`, `gang_city.rs`,
  `witness_city.rs`, `street_spawn.rs`, `vehicle_city.rs`) will now run WITH traffic, and at stars WITH police cars.
  `police_bench.rs:30-38` waits until `active == (row.units, row.swat)` counting foot entities: with crew aboard cars
  this never happens in 640 ticks → must be re-anchored (step 27).

---

## 2. Approach

### 2.1 Traffic = a lane-cursor state per car, two actuation modes
- New gameplay domain `crates/gta_sim/src/traffic/` (GDD §12 table: `traffic/` owns lanes, IDM, intersections,
  spawn/despawn; data `assets/traffic/traffic.ron`). One `TrafficPlugin`.
- `TrafficGraph` resource (built from `City` on `OnTransition{Loading→Playing}`, dropped on `NEW_CITY`, like
  `SidewalkGraph`): only slot-0 lanes (inner lane; slot derived from the lateral offset to the road edge centre line,
  so citygen and its golden hashes stay untouched — the curb lane is left to parked cars, orchestrator note 4);
  connectors between slot-0 lanes with a sampled quadratic Bézier (control point = intersection of the two lane
  lines, midpoint when parallel); a conflict table per connector; spawn points every `spawn_spacing` along lanes.
- Every traffic car is the T14 car (`vehicle_bundle`) plus `TrafficCar { segment: Lane|Connector, s, speed, next,
  mode: Kinematic|Dynamic|Stalled }`. The AI driver is data, not an entity (decision logged): no seated character,
  so no change to civilian caps, FSMs or perception; a civilian is spawned only when the car is hijacked.
- Longitudinal model: IDM exactly as GDD §5.2, `a·(1 − (v/v0)^4 − (s*/s)²)`, `s* = s0 + vT + vΔv/(2√(ab))`,
  integrated with the ballistic update and the stop-within-step rule (Treiber & Kanagaraj 2015, "Comparing
  numerical integration schemes for time-continuous car-following models", Physica A 419: ballistic beats Euler
  with stops; traffic-simulation.de/info/info_IDM.html: "decelerate at constant deceleration to a complete stop and
  remain at standstill"). Speed is clamped at 0 (GDD). Decelerations are clamped at `max_deceleration` (a car
  cannot stop in one tick; an unforeseen obstacle can still be hit, which the contact switch handles).
- Leader = the next car along the car's own path (current segment, then its chosen connector, then the connector's
  lane, up to `look_ahead` m), from per-segment sorted occupancy lists rebuilt once per tick — O(N log N), N ≤ 29.
  Plus a stop-line obstacle at the lane end while the connector is not granted, plus one forward shape cast per car
  per tick (mask `Character|Vehicle`, excluding kinematic traffic, which is already in the occupancy) for the
  player, pedestrians in the road, the player's car, police cars, stalled cars. Gap = min of the three.
- Intersections: first-come-first-served reservation of connectors (simplified Dresner & Stone 2008 AIM, JAIR 31:
  "determine whether any of the reservation tiles are already reserved"; we reserve whole conflicting connectors
  instead of space-time tiles — GDD R7 "одна резервация на точку конфликта, без светофоров"). Two connectors conflict
  when they share the source lane, share the destination lane, or their centre polylines come within car width +
  `conflict_margin`; a connector does not conflict with itself (a platoon follows by IDM). A car at the stop line is
  granted when no occupant's connector and no EARLIER waiter's connector conflicts with its own and the destination
  lane has room for it ("don't block the box"). Released when its rear leaves the connector.
- Kinematic actuation (default): `LinearVelocity = (pose(s') − position)/dt`, `AngularVelocity.y = wrap(yaw' −
  yaw)/dt` (probe Q1: exact; closed-loop, no drift). Chassis at `rest_height` over the flat road; wheel states set
  to rest so the client shows them grounded.
- Kinematic → dynamic (GDD §5.2, mandatory): predictive, in `FixedPostUpdate.before(PhysicsSystems::First)`: a
  kinematic traffic car whose chassis box inflated by `switch.margin` intersects a dynamic, awake body (Character or
  Vehicle layer) closing at ≥ `switch.closing_speed` gets `insert(RigidBody::Dynamic)` this step (probe Q3: same-step
  effect; probe Q4: a CollisionStart-driven switch is one step late and the solver treats the car as a wall). Backstop:
  any `CollisionStart` between a kinematic traffic car and a dynamic body; and the hijack.
- Dynamic actuation: the same lane-cursor logic (s projected from the position) produces a target point and a
  target speed for a shared pure-pursuit autopilot (Coulter 1992, CMU-RI-TR-92-01: curvature `κ = 2 sin α / L`),
  which writes the car's own `DriveIntent`; the T14 chassis reads it. A dynamic car farther than `lost.distance` from
  its path, turned past `lost.angle_deg`, or upside down becomes `Stalled` (handbrake, leaves the lane system) and
  is removed by the bubble rules. No return to kinematic (YAGNI; cap bounds the dynamic count; bench measures).
- Bubble (GDD §5.2 from Vermeij via libertycity.net, verbatim: "places cars at about 70m from the player if they would
  be in view of the camera. They get removed at around 90m. Cars that are 'off screen' get placed at around 15m and
  removed at 25m. For SA … off-screen for at least 2 seconds to be removed"): `in_frame = inside the camera cone AND
  within in_view.despawn (90)`; spawn in frame at `[in_view.spawn, in_view.despawn)` = [70, 90), off frame at
  `[off_view.spawn, off_view.despawn)` = [15, 25); despawn iff `Offscreen ≥ offscreen_seconds (2)` AND distance >
  `off_view.despawn (25)`. A car in frame is never removed; a car beyond 90 m counts as out of frame. Cap
  `max_cars` 24 (GDD §6.1). A load-time fill (like `PopulationPhase::InitialFill`) fills [15, 90) at once.
- Hijack (GDD §5.2 MVP): the existing `enter_exit` already lets the player enter any car with `driver == None` at
  ≤ 3 m/s (a traffic car stops for a pedestrian in its lane and at stop lines). A traffic system reacting to
  `VehicleEntered` for a `TrafficCar`: the car becomes dynamic, loses `TrafficCar`/`Autopilot`/its own `DriveIntent`,
  and a civilian driver appears where the player stood (a spot the player occupied, so it is walkable) in
  `Flee{about: Some(Cause::Attack(theft.attack))}` — the existing `call_later` makes it phone in the CarTheft.

### 2.2 Police cars
- In `police/` (GDD §12: police owns "полицейские машины"). Row column `cars` 1..5 (GDD §6.4 table). Crew
  (`car.crew`, 2) counts toward the row's `units`, so humans stay within the GDD cap of 12 and cars within 5
  (decision logged); the foot dispatcher fills only what cars do not carry.
- Spawn off frame on traffic spawn points in `car.spawn_ring` near the last known position (reuse `pick_spawn`).
  Dynamic from spawn, driven by the shared autopilot. Route = A* over road nodes with directed slot-0 lanes as edges
  (`pathfinding::astar`, cm costs like `navigation::find_route`), at most `car.routes_per_tick` searches per tick,
  refreshed every `car.route_refresh_seconds`.
- State (pure fn + table gate): `Respond` (route to the node nearest the target: the player when a car/cop sees him,
  else `last_known`), `Chase` (player driving, seen, within `direct_chase_distance` 40 → target = the player
  directly, rams), `Dismounted` (stopped within `dismount_distance` 20 m of a player on foot, or of a player whose
  car stands still for `stopped_seconds`, or at the route end → the crew becomes `PoliceUnit`s at the door exit
  spots, tagged `CrewOf{car}`), `Leave` (0 stars: wander lanes, bubble-despawn), `Taken` (the player drove it).
- Re-boarding: while the player drives and is farther than `reboard_distance` from a dismounted cop, that cop
  (not Dead/Leave) runs to its car's door; within `enter_radius` it is despawned and its kind goes back into
  `car.crew`; when no live `CrewOf` cop of the car is outside, the car goes `Respond`. This keeps the car chase alive
  after a foot fight without growing the unit count.
- Police cars see like a cop from the car (GDD §6.4: 50 m) in `track_search` and witness crimes in `record_crimes`.
- Sirens (GDD §8) ride active police cars; foot cops carry them only when no active police car is audible (keeps
  the test-area behaviour and the T13 gate).

### 2.3 Binding TASK-015 notes
- Cabin wound: `apply_bullet_hits` → a pellet whose hit point (body frame) lies in the cabin box (`sedan.ron`) of a
  car with a `driver` wounds the driver by `damage × cabin_driver_share` (`damage.ron`) through `Health::take`
  (armour first) with a `DamageDealt`; `VehicleSystems::Bullets` gets `.before(HealthSystems::Death)` so a killing
  shot is handled in the same tick.
- Pull-out at 1 star: a cop in `Arrest` within `arrest.distance` of the driver's door point of a car at ≤
  `exit_max_speed` for `arrest.pull_out_seconds` pulls the player out through the forced exit path; the normal arrest
  then runs (passive 1.5 s → Busted; running breaks free, +1 star). Arrest cops walk to the door point, not the seat.
- Cars in the fire line: non-target cars (every `Vehicle` except the one the target drives) become spared-body point
  sets on their flat chassis rectangle (perimeter samples every `2 × clearance`, so a line through the rectangle
  always passes within `clearance` of a sample), never `yielding`; shared `hold_fire` for police and gangs.
- Forced eject: same candidates, level rule kept, capsule/path tests dropped; last resort the roof point at the
  car's top (never a ray result).

Rejected alternatives (with reasons, see `log.jsonl`): CollisionStart-only switch (probe Q4); seated AI driver
entities; kinematic police cars; crew outside the unit cap; a citygen `slot` field; a return-to-kinematic mode
(YAGNI); traffic lights (GDD: none in MVP).

---

## 3. Steps

Implementation order is by dependency, with a check per phase. Ticks are 64 Hz (`Time<Fixed>` default, law).
Every tuning number below lives in the named RON file; the only new `const`s are laws (named in the step).

### Phase A — data and graph

**Step 1. `assets/traffic/traffic.ron` (new) + `traffic/config.rs` `TrafficConfig` (`deny_unknown_fields`,
`validate()`, loaded in `compose_sim` like the others, `TRAFFIC_CONFIG = "traffic/traffic.ron"`).**
Fields and start values (GDD §5.2 where given):
```
idm: (time_headway: 1.5, acceleration: 0.73, comfortable_deceleration: 1.67, min_gap: 2.0, max_deceleration: 8.0),
desired_speed: (avenue: 16.0, street: 12.0),   // m/s by road class (GDD §5.2)
turn_speed: 6.0,                  // v0 on a connector, m/s
look_ahead: 60.0,                 // leader search along the path, m
sense_distance: 25.0,             // forward cast on a lane, m past the bumper
turn_sense_distance: 6.0,         // forward cast on a connector (a straight cast would see corner sidewalks)
connector_samples: 8,             // polyline points per connector curve
conflict_margin: 0.3,             // m added to the car width when two connector lines count as crossing
bubble: (max_cars: 24, in_view: (spawn: 70.0, despawn: 90.0), off_view: (spawn: 15.0, despawn: 25.0),
         offscreen_seconds: 2.0, spawns_per_tick: 1, initial_spawns_per_tick: 4, spawn_spacing: 10.0),
switch: (margin: 1.0, closing_speed: 0.5),
lost: (distance: 4.0, angle_deg: 60.0),
```
`validate`: all finite; positive where physical; `min_gap > 0`; `max_deceleration >= comfortable_deceleration`;
`off_view.spawn < off_view.despawn <= in_view.spawn < in_view.despawn`; `max_cars >= 1`, `spawns_per_tick >= 1`,
`connector_samples >= 3`; `0 < lost.angle_deg < 180`. Cross-config in `compose_sim` (like `validate_ring`):
`switch.margin > (vehicle.max_speed + max(desired_speed)) / 64` (the closing distance of one tick; 44/64 = 0.69 < 1.0),
and `in_view.despawn < population.despawn_distance`. IDM exponent δ = 4 is part of the GDD formula → `const IDM_DELTA:
i32 = 4` (law).
Check: `tests/config_traffic.rs` rows (step 26).

**Step 2. Other data files (append fields at the END of existing tuples so the sabotage strings of
`tests/config_police.rs`/`config_vehicle.rs` still match).**
- `assets/police/escalation.ron`: each star row gets `cars: 1|2|3|4|5` (GDD §6.4); `arrest` gets
  `pull_out_seconds: 1.0`; new block
  `car: (crew: 2, spawn_ring: (60.0, 120.0), spawns_per_tick: 1, pursuit_speed: 20.0, turn_speed: 8.0,
  dismount_distance: 20.0, direct_chase_distance: 40.0, stopped_seconds: 1.0, reboard_distance: 25.0,
  route_refresh_seconds: 1.0, routes_per_tick: 1)`. `EscalationRow.cars: u32`, `ArrestConfig.pull_out_seconds`,
  `PoliceCarConfig` in `police/mod.rs`; validate: rows `cars` non-decreasing, `crew >= 1`, `car.spawn_ring` band and
  `< population.despawn_distance` (extend `validate_ring`), `dismount_distance < direct_chase_distance`, positives.
  Update the `fsm.rs` shipped-rows test only if it compares whole rows (it compares `(units, swat, arrest)` — no change).
- `assets/wanted/wanted.ron`: `cop_car_view_distance: 50.0` (GDD §6.4 "50 м из машины"), validated positive and ≥
  `cop_view_distance`.
- `assets/vehicle/sedan.ron`: `cabin: (centre: (0.0, 0.45, 0.1), half_extents: (1.25, 0.5, 1.0))` (body frame; the
  x and top half extents exceed the chassis 1.2 / 0.92 so surface hit points count) and
  `autopilot: (lookahead_min: 4.0, lookahead_per_mps: 0.5, speed_gain: 0.5, stuck_speed: 0.5, stuck_seconds: 2.0,
  reverse_seconds: 1.5)`. `VehicleConfig.cabin`, `.autopilot` + validate (half extents > 0, lookahead_min > 0).
- `assets/vehicle/damage.ron`: `vehicle.cabin_driver_share: 0.5` in [0, 1].
- `assets/world/render.ron`: `police_vehicle: (model: "third_party/car-kit/police.glb", scale: 1.4, offset: (0.0,
  -1.16, <derived>), wheels: (…4 names…))` — scale 1.4 makes the police body 2.9 × 1.4 = 4.06 m long against the
  4.08 m chassis; `offset.y = -rest_height` like the sedan (model origin = wheel bottoms); `offset.z` derived from the
  body node z (-0.025) and the body mesh centre z (-0.075) mirrored by the 180° model yaw, as the sedan comment does.
- `assets/third_party/manifest.ron` car-kit pack: add `(archive: "Models/GLB format/police.glb", path: "police.glb",
  sha256: "a617b880594f56239b7ac4cf6fd4d74ebdae3ba6c5d842e77c76a5906eff352a")`; fetch with
  `python tools/fetch_assets.py --cache maw/tasks/done/TASK-015/scratch/carkit` (zip sha verified by the planner).
  `src/visuals/config.rs vehicle_asset_paths` yields the police model too.
Check: `cargo test -p gta_sim --test config --test config_police --test config_vehicle --test asset_manifest -j 4`.

**Step 3. `traffic/graph.rs` — `TrafficGraph` (Resource, `Debug`).**
```rust
pub enum Segment { Lane(u32), Connector(u32) }            // Reflect, Copy
pub struct TrafficLane { from: Vec3, to: Vec3, dir: Vec3, length: f32, v0: f32, end_node: u32, out: Vec<u32> }
pub struct TrafficConnector { from_lane: u32, to_lane: u32, node: u32, points: Vec<Vec3>, cumulative: Vec<f32>,
                              length: f32, conflicts: Vec<u32> }
pub struct TrafficGraph { lanes, connectors, spawn_points: Vec<(u32 /*lane*/, f32 /*s*/)> }
impl TrafficGraph {
  pub fn new(lanes: Vec<(Vec3, Vec3, f32 /*v0*/)>, connectors: &[(u32, u32, u32 /*node*/)], cfg: &TrafficConfig,
             half_width: f32) -> Result<Self, String>;   // tests build synthetic graphs with this
  pub fn from_layout(layout: &CityLayout, params: &CityParams, cfg: &TrafficConfig, half_width: f32) -> Result<Self, String>;
  pub fn pose(&self, seg: Segment, s: f32) -> (Vec3 /*point, y = 0*/, Vec3 /*unit tangent*/);
  pub fn length(&self, seg: Segment) -> f32;
  pub fn nearest(&self, p: Vec3) -> Option<(Segment, f32, f32 /*distance*/)>;   // for dynamic cars / routing
}
```
- `from_layout`: keep lane k iff `slot(k) == 0` where `slot = round(|cross(d_edge, lane.from − node_a)| /
  lane_width − 0.5)` (worked: inner avenue lane offset 1.625 → 0; curb 4.875 → 1; street 1.625 → 0). v0 from the
  edge class. Connectors = citygen connectors whose both lanes are kept. y = 0 (road top, `world/city.rs:12`).
- Curve: p0 = from-lane end, p2 = to-lane start; control = intersection of the lines (p0 + t·d_in, p2 − u·d_out),
  midpoint when `|d_in × d_out| < 1e-3`; `connector_samples` points; cumulative arc length.
- Conflicts: `i≠j` conflict iff same `from_lane`, or same `to_lane`, or min distance between their polylines
  (segment-segment) < `2·half_width + conflict_margin`. Only connectors of the same node are compared.
- Errors: a kept lane with no outgoing connector (a dead end would park cars forever), NaN/zero-length lane.
- `NEW_CITY` drops the resource; built in `OnTransition{Loading→Playing}` `.run_if(resource_exists::<City>)`; on
  error `AppExit::error()` like `build_sidewalk_graph`.
Check: unit tests in `graph.rs`: slot rows (inner/curb/street, both directions); a right-angle connector's curve
starts at p0, ends at p2, tangent at ends equals the lane directions (worked: p0 (0,0,0) d_in −Z, p2 (5,0,−5) d_out
+X → control (0,0,−5), midpoint of the Bézier (1.25,0,−3.75)); a + intersection (4 in, 4 out, 12 connectors):
straight N→S conflicts with straight E→W, not with its own; a same-source pair conflicts; a same-destination pair
conflicts. Property gate over seeds 1..=8 via `citygen::generate` + `from_layout` (no App): no error, every lane has
out connectors (step 26).

**Step 4. `traffic/idm.rs` — pure kernel.**
```rust
pub fn idm_acceleration(v: f32, v0: f32, gap: Option<(f32 /*s*/, f32 /*dv = v − v_leader*/)>, c: &IdmConfig) -> f32
// free road when gap is None; s clamped at 1e-3; result clamped to >= -max_deceleration
pub fn ballistic_step(s: f32, v: f32, a: f32, dt: f32) -> (f32, f32)
// v' = v + a·dt; if v' < 0 { (s − v²/(2a), 0) } else { (s + v·dt + a·dt²/2, v') }
```
Worked rows (shipped IDM: a 0.73, b 1.67, T 1.5, s0 2): free v=0 → 0.73; free v=v0=12 → 0; v=0, s=2, dv=0 → s*=2 →
0; v=0, s=1 → 0.73·(1−4) = −2.19; v=12, s=30, dv=0 → s* = 2 + 18 = 20, a = 0.73·(1 − 1 − 0.444) = −0.3244;
`ballistic_step(0, 1, −8, 1/64)` → (0.0146484, 0.875); `ballistic_step(0, 0.1, −8, 1/64)` → (0.000625, 0);
`ballistic_step(5, 0, −2.19, 1/64)` → (5, 0) (a stopped car never goes backwards).
Check: `cargo test -p gta_sim traffic::idm` (unit tests with these rows).

### Phase B — the traffic loop

**Step 5. `traffic/mod.rs` — components, resources, plugin.**
- `TrafficCar { segment: Segment, s: f32, speed: f32, next: Option<u32>, mode: TrafficMode, waiting: Option<u64> }`
  (`Reflect`, `#[reflect(Component)]`, `#[require(Offscreen)]`); `TrafficMode { Kinematic, Dynamic, Stalled }`.
- `TrafficRng` (ChaCha8, `set_stream(4)`; reseeded on `OnEnter(Loading)` from `CitySeed` like `reseed_police`).
- `TrafficIntersections` resource: per node `occupants: Vec<(u32 connector, Entity)>`, `waiters: Vec<(u64 tick,
  Entity, u32 connector)>`; cleared on `NEW_CITY`.
- `TrafficStats` (Reflect Resource, read by QA and the bench): `cars, kinematic, dynamic, stalled, spawned,
  despawned, casts, switches`.
- `TrafficPhase { InitialFill, Steady }` (reset on `NEW_CITY`).
- Sets: `TrafficSystems::{Hijack, Drive, Bubble}` in `FixedUpdate`, `.in_set(NpcSystems)`,
  `.run_if(resource_exists::<TrafficGraph>)`; `Hijack.after(VehicleSystems::Enter)`, `Drive.after(Hijack)
  .before(VehicleSystems::Drive)`, `Bubble.after(Drive)`. `VehicleSystems::Drive` additionally `.after(PoliceSystems)`
  (police car autopilot targets written first). Verify no schedule cycle (Bevy panics at build; any test catches it).
- `FixedPostUpdate`: `contact::switch_to_dynamic.before(PhysicsSystems::First)`.
- `lib.rs`: load `TrafficConfig`, insert, `app.add_plugins(TrafficPlugin)` next to `VehiclePlugin` (outside the tuple).

**Step 6. `vehicle/autopilot.rs` — shared AI driving (vehicle domain, `VehicleConfig.autopilot`).**
- `Autopilot { target: Vec3, speed: f32, stuck: f32, reverse_left: f32 }` (Reflect component; the car also carries its
  own `DriveIntent`). System `steer_autopilots` in `VehicleSystems::Drive` before `drive_vehicles` (`.chain()`):
  - forward `f = rot·(−Z)`, right `r = f × Y`, `d = flat(target − position)`, `α = atan2(d·r, d·f)`, `L = |d|`
    (≥ lookahead_min), `κ = 2 sin α / L`, `δ = atan(κ · 2·half_wheelbase)`, `intent.steer = clamp(δ / steer_limit(cfg,
    v_fwd), −1, 1)`.
  - throttle: `err = speed − v_fwd`; `err > 0` → `min(err·speed_gain, 1)`; else if `v_fwd > hold_speed` →
    `max(err·speed_gain, −1)` (brake); else 0 (hold; never reverse by accident — `drive_force` reverses on negative
    throttle below `hold_speed`).
  - stuck: throttle > 0.5 and `|v| < stuck_speed` for `stuck_seconds` → `reverse_left = reverse_seconds`, then throttle
    −1 and steer negated.
  Three directional rows (sign errors pass magnitude tests), car at the origin, target 10 m ahead 5 m right:
  yaw 0 (f −Z, r +X): target (5,0,−10) → d·r 5 > 0 → steer > 0 (right); yaw +90° (f −X, r −Z): target (−10,0,−5) →
  d·r 5 → steer > 0; yaw 180° (f +Z, r −X): target (−5,0,10) → steer > 0; mirrored targets give steer < 0.
  `wheel_forward(+δ)` has +x = right, consistent with `chassis.rs:8`.
- `chassis.rs drive_vehicles`: intent = driver's `DriveIntent`, else the car's own `DriveIntent` (`.or_else(||
  intents.get(entity).ok())`); skip non-dynamic bodies (`&RigidBody`, `!rb.is_dynamic()` → continue, no rays).
Check: unit rows above in `autopilot.rs`; `tests/vehicle.rs` stays green.

**Step 7. `traffic/drive.rs` — `advance_traffic` (TrafficSystems::Drive), one system, bounded per tick.**
Per tick, deterministic order (cars sorted by `Entity` bits — query order is not spawn order, TASK-015 lesson):
1. Dynamic cars: re-project `(segment, s)` onto their path from `Position` (segment advance when `s ≥ length`);
   lost rule → `Stalled` (remove `Autopilot`, own `DriveIntent{handbrake: true}`, release reservations).
2. Occupancy: per segment a sorted `Vec<(s, Entity, speed)>` of non-stalled cars.
3. Intersections: release every occupant whose rear (`s − half_length`) is past its connector; for each car on a lane
   within `request_distance = speed²/(2·b) + s0 + half_length` of the lane end, choose `next` if unset (uniform over
   `lane.out`, `TrafficRng`), queue as waiter (tick stamp once), grant in FCFS order (tick, then entity bits) when no
   occupant/earlier waiter conflicts and the destination lane's last car has `s − half_length ≥ length + s0`.
4. For each non-stalled car: gap = min(leader along the path within `look_ahead`, stop line (lane end − half_length)
   if `next` is not granted, forward cast hit − half_length). Forward cast: `cast_shape_predicate` with a box of the
   chassis half width/height, from the car centre along its tangent, `half_length + sense_distance` on a lane,
   `half_length + turn_sense_distance` on a connector, mask `[Character, Vehicle]`, excluding itself and kinematic
   traffic cars; leader speed of a cast hit = its `LinearVelocity` along my tangent. `TrafficStats.casts += 1`.
5. IDM with v0 = lane v0 (connector: `turn_speed`); kinematic: `ballistic_step`; on crossing a lane end with a grant
   → move into the connector (carry the excess s); connector end → the destination lane; on a lane end WITHOUT a
   grant clamp `s` at the stop line with `v = 0` (a driver never runs the line; the conflict gate then tests the
   reservation, not the brakes).
6. Kinematic actuation: `pose(s')` → `LinearVelocity`, `AngularVelocity`, `vehicle.steer = atan(2·half_wheelbase ·
   yaw_rate / max(v, 1))` clamped to `steer.max_deg` (visual only). Dynamic: `Autopilot{target: pose(s + L), speed:
   max(0, v + a·dt)}`.
Budget: O(N log N) + N casts, N ≤ 24 + intersections ≤ waiters × occupants (≤ 8 × 8 per node).
Check: step 21/22 gates; clippy.

**Step 8. `traffic/contact.rs` — the switch (FixedPostUpdate, before `PhysicsSystems::First`).**
For each `TrafficCar` with `mode == Kinematic`: `shape_intersections(Collider::cuboid(2·(half + margin)), position,
rotation, mask [Character, Vehicle])`, excluding itself; a hit counts when its body is `RigidBody::Dynamic`, not
`Sleeping`, not `RigidBodyDisabled`, and `(v_other − v_car)·normalize(car − other) ≥ switch.closing_speed`. Plus a
`MessageReader<CollisionStart>` backstop (kinematic traffic car vs a dynamic body). Switch = `commands.insert
((RigidBody::Dynamic, SleepingDisabled, Autopilot{..}, DriveIntent::default()))`, `mode = Dynamic`,
`TrafficStats.switches += 1`. Parked cars asleep and cars in the next lane at equal speed never switch (closing 0).
Check: step 23 gate.

**Step 9. `traffic/spawn.rs` — `despawn_traffic`, `spawn_traffic` (TrafficSystems::Bubble, chained).**
- Needs `CameraView` and the player (like `despawn_far`/`spawn_civilians`). Frame test per car:
  `in_frame = !outside_cone(view, feet, car_height = 2·half_y, 0) && flat_distance ≤ in_view.despawn`;
  `Offscreen += dt` when not in frame, else 0; despawn iff `Offscreen ≥ offscreen_seconds && distance >
  off_view.despawn` (stalled cars included; release their reservations). Cars driven by the player are never
  traffic (hijack removes `TrafficCar`).
- Spawn while `cars < max_cars`, `spawns_per_tick` (InitialFill: `initial_spawns_per_tick` in [off_view.spawn,
  in_view.despawn) regardless of view, then Steady): candidates from `spawn_points` with band = in frame ? [70, 90)
  : [15, 25) by the same frame test; skip points closer than `v0²/(2b) + s0 + half_length` to the lane end; skip
  when the nearest car on that lane (occupancy) is within `s0 + v0·T + length` behind or ahead; skip when
  `shape_intersections` of the chassis box finds anything on `Character|Vehicle` (the player, a parked car on a
  test lane, a pedestrian). Spawn `vehicle_bundle` + `insert(RigidBody::Kinematic)` + `TrafficCar{speed: v0}` +
  `Name("Traffic car")`, pose from the graph at `rest_height`, wheels set to rest (`compression =
  static_compression`, `grounded`). Random choice with `TrafficRng`.
Check: step 24 gate.

**Step 10. `traffic/hijack.rs` — `on_hijack` (TrafficSystems::Hijack).**
Reads `VehicleEntered`; for a car with `TrafficCar`: remove `TrafficCar, Autopilot, DriveIntent`, insert
`RigidBody::Dynamic` (probe Q3), release reservations; spawn `civilian_bundle` on the nearest sidewalk edge (walker
from `nearest_node` and its first neighbour), then override `Transform` to the player's pre-entry feet (read from
the message tick: `enter_exit` inserts the seat pose later in the same tick via commands, so read the player's
`Position` in this system before those commands apply — ordering `.after(VehicleSystems::Enter)` still sees the old
`Position` because `leave`/entry use inserts, `seat.rs:359-365`), state `Flee{from: player, left: roll(flee_distance),
about: Some(Cause::Attack(msg.attack))}`, `flee_start` walker; temperament and rolls from `TrafficRng` (never
`NpcRng`, TASK-010 lesson). The police car variant is step 14.
Check: step 25 gate.

### Phase C — police cars

**Step 11. `police/cars.rs` (new) — components, dispatch.**
- `PoliceCar { state: PoliceCarState, crew: Vec<UnitKind>, stopped: f32 }`, `PoliceCarState { Respond, Chase,
  Dismounted, Leave, Taken }` (Reflect), `PoliceCarRoute { lanes: Vec<u32>, next: usize, goal: Option<u32>, age: f32 }`,
  `CrewOf { car: Entity }` on dismounted cops (a separate component keeps the `PoliceUnit` literal at
  `src/audio/event_gate.rs:161` compiling). `PoliceCarRng` (`set_stream(5)`, reseeded on Loading).
- `PoliceDispatcher` gets `cars: u32` (update the literal in `tests/new_city.rs:301` to `..default()`/`cars: 0`).
- `dispatch_police_cars` in the `PoliceSystems` chain BEFORE `dispatch_police`: count active cars (state Respond,
  Chase, Dismounted; not Taken/Leave) and crew aboard; while `cars < row.cars` and `units + 1 <= row.units`, spawn a
  car with `min(car.crew, row.units − units)` crew, kinds by `spawn_kind` (SWAT share first) — at most
  `car.spawns_per_tick`; spawn point: traffic `spawn_points` in `car.spawn_ring`, hidden (`outside_cone` or
  `occluded` within the shared `PopulationLoad` ray budget), free box, `pick_spawn` near `last_known` (surround rows
  spread). Car = `vehicle_bundle` + `PoliceCar` + `PoliceCarRoute` + `Autopilot` + own `DriveIntent` +
  `SleepingDisabled`, dynamic, heading = lane direction.
- `dispatch_police` (foot): `active` also contains each crew member aboard (at its car's position) so
  `(dispatcher.units, dispatcher.swat)` = foot + aboard; everything else unchanged. With no `TrafficGraph` (test area)
  no car ever spawns → every test-area police gate is unchanged.
Check: step 28 gates rows 1..5.

**Step 12. `police/cars.rs` — `drive_police_cars` (PoliceSystems, after dispatch).**
- Senses: `sees = in_view(car_eye = position + rot·seat, rot·(−Z), player_eye, cop_view_cone_deg,
  cop_car_view_distance) && !sight_blocked(..)`; `driving` = player has `Driving`; player car speed; distance.
- Pure `next_car_state(state, &CarSenses) -> PoliceCarState` (table-gated like `fsm::next_state`): any → `Leave` at 0
  stars (except Taken); Respond → Chase if driving && sees && d ≤ direct; Chase → Respond if !(driving && sees && d ≤
  direct); Respond → Dismounted when the car is at ≤ `exit_max_speed` and (on foot && d ≤ dismount) or (driving &&
  player car ≤ exit speed for `stopped_seconds` && d ≤ dismount) or (route done); Dismounted → Respond when no live
  `CrewOf` cop of this car is outside; Leave, Taken terminal.
- Motion: Respond → route (A* over road nodes, lanes as edges, `astar`, cm costs, ≤ `routes_per_tick` searches per
  tick for all cars, refresh by age) to the node nearest the target (`last_known`, or the player while seen);
  autopilot target = the path point `L` ahead along lane/connector polylines; speed `pursuit_speed` on lanes,
  `turn_speed` on connectors, `min(.., sqrt(2·b·distance_to_goal))` near the goal, and IDM on the forward cast
  (excluding the player's car when `Chase`). Chase → target = player position, speed `pursuit_speed` (rams).
  Dismounted/stopped → speed 0. Leave → wander lanes like traffic (random next), bubble-despawn (step 13).
- Dismount (on entering Dismounted): for each crew kind spawn `police_unit_bundle` at `vehicle::exit_spots` (step 16:
  left door, right door, roof, level-checked, free) — a crew member without a free spot stays aboard;
  `insert(CrewOf{car})`; appearance from `PoliceCarRng`; state `Respond` (the FSM takes over next tick).
- Re-board (`board_police_cars`, after `police_fsm`): a `CrewOf` cop within `enter_radius` of its car's door while the
  car is `Dismounted` and the player drives farther than `reboard_distance` → despawn (sensor observer cleans up),
  push its kind to `crew`. In `police_fsm` (`behavior.rs`, before the state match): such a cop's motion is
  `Seek(door, chase_gait, direct)`, aim off, no trigger pull (≈20 lines).
Check: step 28/29 gates.

**Step 13. `police/cars.rs` — `despawn_police_cars`.** Leave cars and cars whose crew is all gone (Dismounted with
no crew aboard and no live `CrewOf`) follow the traffic bubble rule (step 9 frame test, 2 s, > 25 m); engaged cars
only when off frame ≥ 2 s and beyond `population.despawn_distance` (like foot cops). Taken cars are never despawned
by police. `NEW_CITY`: cars are `CityScoped`; reset `PoliceDispatcher.cars`.

**Step 14. Hijacking a police car.** `on_police_car_entered` (reads `VehicleEntered` for a `PoliceCar`): crew aboard
dismount at once (step 12 spawn, excluding the spot the player stood on), state `Taken`, remove `Autopilot`,
`DriveIntent`, `PoliceCarRoute`. CarTheft is recorded by the existing `record_crimes` (`first`).

**Step 15. Wanted: cars see and witness.** `wanted/search.rs track_search`: add a query of active `PoliceCar`s with
crew aboard, `cop_sees` with `cop_car_view_distance` from `car_eye`. `wanted/crimes.rs record_crimes`: witnesses also
from crewed police cars (`witnesses(&spatial, car_eye, ..)`). Gate rows in step 28.

### Phase D — binding notes

**Step 16. `vehicle/seat.rs` exit spots.** Split `exit_spot` into `fn candidates(..) -> [(Vec3, f32); 3]`, `fn
clear_spots(spatial, cfg, loco, car, exclude: &[Entity]) -> Vec<Spot>` (every candidate passing level + capsule +
path, in order) and `exit_spot(.., forced)` = first clear spot; `forced` fallback = first candidate whose feet ray
passes the LEVEL rule only; last resort = the roof point `roof` with feet at `top` (no ray). `pub(crate) fn
pull_out(commands, spatial, cfg, loco, player, children, heads, car, vehicle)` = `exit_spot(forced) + leave +
free_car` for step 18. `pub(crate) fn exit_spots` for step 12.
Check: `tests/vehicle_seat.rs` stays green + step 31 rows.

**Step 17. Cabin wound (`vehicle/impact.rs apply_bullet_hits`).** For a hit on a car with `driver: Some(d)`: `local =
rot⁻¹·(point − position) − cabin.centre`, inside iff `|local| ≤ half_extents` per axis; then `wound =
round(hit.damage × cabin_driver_share) as u32`, `killed = health.take(wound)`, write `DamageDealt{shooter,
shot: attack, target: d, point, damage: wound, headshot: false, killed}` (juice arc, vignette, wanted all react
through their existing readers). `VehicleSystems::Bullets.after(HealthSystems::Damage).before(HealthSystems::Death)`.
Worked (pistol 25, share 0.5): side window hit at body (1.2, 0.5, 0.0) → wound 13 (12.5 rounds half-away to 13 —
use `f32::round`), armour 0 → health 100 → 87; hood (0, 0.3, −2.04) → z out of [−0.9, 1.1] → 0; low door (1.2,
−0.3, 0) → y below −0.05 → 0; roof (0, 0.92, 0.3) → wound 13.

**Step 18. Pull-out (`police/arrest.rs`).** `ArrestAttempt` gets `pull: f32` (reflected). New `pull_out_driver`
(same set as `arrest_player`, before it): player `With<Driving>`, arrest row (row.arrest && !hostile); nearest cop in
`Arrest` with `flat_distance(cop, door_point(car)) ≤ arrest.distance`, car speed ≤ `exit_max_speed` → `pull += dt`,
else `pull = 0`; at `pull ≥ pull_out_seconds` → `vehicle::pull_out(..)`, `attempt = {cop: Some(cop), hold: 0, pull:
0}`. Reset on Wasted/Busted/NEW_CITY (existing `reset_arrest`). `police_fsm` Arrest: when the player drives, `at` =
the car's door point (the cop walks to the door, stands at `stand_distance` from it).

**Step 19. Cars in the fire line (`tactics`).** `fire_line.rs`: `pub(crate) fn car_points(cars: &[(Entity, Vec3,
Quat)], half: Vec2 /*x, z*/, spacing: f32, exclude: Option<Entity>, near: (Vec3, f32)) -> Vec<Vec3>` — perimeter
samples of each flat chassis rectangle (corners + every ≤ `spacing` along edges), only for cars whose centre is
within `reach + |half|` of the shooter. `hold_fire` gets `cars: &[Vec3]`, appended to `shields` with `yielding =
false` and to `bodies`. Police (`behavior.rs:337`) and gangs (`gang/behavior.rs:402`) build the list once per system
run with `spacing = 2 × clearance` (derived, no const) and `exclude = the car the target drives`. Cost: prefilter keeps
it at the cars near each shooter.

### Phase E — presentation and QA

**Step 20. Client.**
- `src/visuals/vehicle.rs`: `spawn_vehicle_model` picks `police_vehicle` when the entity has `PoliceCar` (observer
  `On<Add, Vehicle>` + `Has<PoliceCar>`; police cars are spawned with both in one bundle), else the sedan. No other
  change (kinematic wheel states come from the sim).
- `src/audio/loops.rs update_sirens`: candidates = active `PoliceCar`s (not Taken/Leave) within `audible`; if none,
  live foot cops as today. Siren child `Transform` height: `siren.height` on cops, a new `mix.ron siren.car_height`
  on cars (roof light bar ≈ 1.0 m above the chassis centre). New gate `sirens_ride_police_cars` in
  `event_gate.rs`: car + 2 cops at 2 stars → one siren parent is the car; car despawned → sirens move to cops.
- `src/minimap/markers.rs`: `MarkerKind::PoliceCar` (police tint), police cars no longer get the grey `Vehicle` dot;
  traffic cars keep the grey dot (as T14 decided for all cars).
- `tools/qa/scenarios/t14.py`: `cars(game)` keeps only entities without `TrafficCar`/`PoliceCar` (parked cars).
  `t13.py`: a siren parent may be a live cop or an active `PoliceCar`. Rerun t8…t14 (PCTX TASK-015 lesson).
Check: `cargo test -p gta_like --bin gta_like -j 4` three times (presentation gates, TASK-022 lesson).

**Step 21-31 (gates) are listed in §3.1.** **Step 32. `tools/qa/scenarios/t15.py`** (below). **Step 33. docs**: GDD
§6.4 arrest paragraph: one line "Исключение (T15): на 1 звезде коп вытаскивает игрока из остановленной машины
(≤ exit_max_speed) и арестовывает"; §5.3 one line "высаженные копы возвращаются в машину, если игрок уехал"
(orchestrator's call, see Q-А). `docs/architecture/traffic.md` design note (graph, modes, switch, reservation,
bubble, police car states, with the probe facts).

### 3.1 Gates (headless unless noted; each names its class and its flip)

Common fixtures: `tests/traffic_support/mod.rs` — `traffic_cfg(app)`, `set_traffic(app, |t| ..)`, `test_lanes(app,
TrafficGraph)` (insert a synthetic graph on the test area, like `test_graph`), `spawn_traffic_car(app, seg, s,
speed)`, `cars(app)`, `obb_overlap(a, b)` (2D SAT on chassis rectangles from `Position`/`Rotation`, independent of
the path parameters). Synthetic layouts avoid the test-area fixtures (`world/test_area.rs`): lanes run in z ∈ [20,
38], x ∈ [−38, 38] (clear of the wall at z 14 and the boxes at z 10); the test asserts `GATE BROKEN` if any car
collider touches a `Block` at spawn.

- **Step 21 `tests/traffic_idm.rs` — AC1 (correctness).** Loop of 4 lanes (x −34→34 at z 36, down to z 22, back,
  up) + 4 corner connectors (each node has one in/one out: no conflicts) — 172 m. 10 kinematic cars at rest; gaps
  drawn per seed from [1.0, 8.0] m with at least one gap of 1.0 m (< s0, IDM gives −2.19 at rest), 8 seeds. 6400
  ticks, every tick: every `TrafficCar.speed ≥ 0` and `LinearVelocity·tangent ≥ −1e-4`; no pair of cars with
  `obb_overlap`; liveness: each car travelled ≥ 172 m (a full loop; free speed 12 reached between). Flips: (a) drop the
  `v' < 0` branch in `ballistic_step` → negative speed at the 1.0 m gap (RED); (b) leader search limited to the own
  segment → overlap after a corner (RED).
- **Step 22 `tests/traffic_intersection.rs` — AC2 (correctness).** A + intersection centred (−20, 0, 29) with arms of
  8 m (lanes x −36…−24 / −16…−4, z 21…37 inside the floor), 4 in / 4 out lanes, 12 connectors; the test re-injects a
  car at each approach start when the first 12 m are free; turn choice from `TrafficRng`, 6 seeds × 6400 ticks.
  Conflict points computed BY THE TEST: for every pair of connectors of different source and destination, sample
  both `pose` functions at 0.1 m and keep points where the centre lines cross (distance < 0.05); every tick, for each
  point, count traffic cars whose chassis rectangle (from `Position`/`Rotation`) contains it → ≤ 1. Liveness: each
  approach delivered ≥ 20 cars; every crossing point was covered at some tick. Flip: grant always (skip the conflict
  test) → two cars on one point (RED). Second flip: conflict table built without the geometric test → RED (proves the
  table, not only the grant).
- **Step 23 `tests/traffic_contact.rs` — AC4 (correctness).** Rows, each its own test: (a) the player drives a car
  (T14 `drive_in`, throttle 1) at the rear of a kinematic traffic car stopped at a stop line (car ahead of it in
  lane, `next` not granted by an occupant): the traffic car is `RigidBody::Dynamic` in the tick BEFORE the first
  `CollisionStart` between them, and 16 ticks after contact its speed ≥ 1.5 m/s while the player car's velocity never
  reverses (flip: disable the proximity switch, keep the backstop → traffic car speed < 0.2 and the player car stops
  dead or bounces: RED); (b) the player on foot walks into the side of a stopped traffic car → Dynamic before
  contact; (c) a sleeping parked car 0.85 m beside a passing kinematic car and a car in the next lane at equal speed
  → no switch in 640 ticks (liveness of the closing-speed rule; flip: `closing_speed` 0 → switch RED).
- **Step 24 `tests/traffic_bubble.rs` — AC3 (correctness) + spawn bands.** Seed-1 city (`city_app(1)`), civilians
  0, gangs 0, player on the hospital sidewalk, view turns 90° every 3 s (so cars leave and enter the frame), 40 s.
  The test records per car per tick its position and computes its OWN frame test from the published `CameraView`
  (cone + 90 m); when a car disappears: the last 128 ticks (2 s) it was out of frame by the test's computation and its
  last distance > 25 m. Rows as separate tests: `despawn_needs_two_seconds_off_frame`, `in_frame_car_is_kept`
  (a car pinned in the cone at 60 m via the view never despawns in 640 ticks), `far_in_cone_car_goes`
  (a car in the cone at 100 m despawns after ≥ 128 ticks), `spawn_bands` (every spawn in Steady: in frame → [70, 90),
  out → [15, 25)). Liveness: ≥ 5 despawns and ≥ 5 spawns of each band. Flips: drop the Offscreen condition → RED;
  swap the bands → `spawn_bands` RED.
- **Step 25 `tests/traffic_hijack.rs` (correctness).** Synthetic straight lane on the floor, a traffic car stopped by
  a character standing in the lane 10 m ahead; the player placed 1.5 m from its door; `request_vehicle` → `Driving`
  = that car, car `Dynamic`, no `TrafficCar`/`Autopilot`/own `DriveIntent`; exactly one new `Civilian` within 1 m of
  the player's pre-entry feet, state `Flee{about: Some(Attack(entered.attack))}`; `Crimes` has a CarTheft incident with
  that attack. Then F out: the empty car stays within 0.2 m over 128 ticks (proves the stale AI intent is gone).
  Flip: skip the `DriveIntent` removal → the exited car drives off (RED).
- **Step 26 `tests/config_traffic.rs` + graph property.** Unknown field names file and field; one sabotage per
  `validate` rule strictly on the failing side (TASK-007 lesson), each with a distinct error text; cross-config
  (`switch.margin` 0.6 < 0.69 → error). `tests/traffic_graph.rs`: seeds 1..=8, `from_layout` Ok, every lane has out
  connectors, no lane is a curb lane (offset check vs `parking.curb_offset`), connector count > 0 per intersection
  with ≥ 2 edges.
- **Step 27 existing gates re-anchored (named, with reasons).** `police_bench.rs`: pin `cars = 0` in every row
  (fixture helper `no_police_cars(app)`) — it measures foot SWAT; cars are measured by step 30. `police_city.rs`:
  run twice — as is (cars deliver the cops; liveness "a cop reaches the player") and with `no_police_cars` (the
  sidewalk-routing evidence it was written for). `new_city.rs:301` literal gets `cars`; its "state reset" list gains
  `TrafficIntersections`, `TrafficPhase`, `TrafficStats`, `PoliceDispatcher.cars`, and "no traffic car survives a new
  city". `sensor_leak.rs`: the all-entities diff runs with traffic and a hijack in the window. Any other city gate
  that fails because of traffic: record the failure first; if its subject is not traffic, add `set_traffic(|t|
  t.bubble.max_cars = 0)` to its fixture with a one-line reason; never loosen a threshold.
- **Step 28 `tests/police_cars.rs` — AC5 (correctness, one test per row) + behaviour.** `cars_follow_row_1..5`:
  seed-1 city, player on a sidewalk, heat = row threshold (then one tick, TASK-012 lesson), 1920 ticks: every tick
  active police cars ≤ `row.cars`, `dispatcher.units ≤ row.units`, foot + aboard == `dispatcher.units`; liveness:
  `row.cars` reached. Rows 2 is the AC. `car_closes_in_and_dismounts` (player on foot; nearest police car distance at
  t=20 s < at spawn; a Dismounted car within 20 m + lane offset; its crew are `PoliceUnit`s with `CrewOf`).
  `car_chases_a_driver` (player drives a straight avenue at 10 m/s via `set_drive`: distance shrinks; within 40 m
  with LOS the car is `Chase` and its `Autopilot.target` equals the player position). `crew_reboards`: after a
  dismount the player drives away 60 m → the crew is aboard again and the car is `Respond`. `next_car_state_table`:
  unit table over every state × senses. `car_sees_at_50_m` / `crewed_car_witnesses_theft` (wanted rows). Flip for
  the row gate: count only foot units in the car dispatcher → row 2 overruns (RED).
- **Step 29 `tests/police_pull_out.rs` (correctness).** 1 star, player in a stopped car on the floor, a cop set to
  `Arrest` 1.2 m from the door point → after `pull_out_seconds` (64 ticks) the player has no `Driving`, stands at an
  exit spot (feet within 0.05 of ground level), and after 96 more passive ticks `GameState::Busted`. Rows: car at 5
  m/s → never pulled (640 ticks); 2 stars (no arrest row) → never; cop 2.0 m from the door → never. Flip: drop the
  speed check → row 1 RED.
- **Step 30 `tests/vehicle_hits.rs` additions — cabin (correctness).** Rows from step 17 through a real pistol ray
  (test shooter via `set_aim`/`set_action`) at a car the player drives: side window → health −13; hood → 0; low door
  → 0; roof → −13; SMG burst into the cabin kills the driver → `Wasted` and `eject_all` puts him at ground level.
  Flip: share 0 → rows 1/4 RED; zone check removed → row 2/3 RED.
- **Step 31 `tests/vehicle_seat.rs` additions — forced eject (correctness).** `forced_eject_never_on_a_wall_top`:
  car with a 1 m wall 0.1 m off the left door, a 4 m wall off the right door, a slab 2.5 m above the roof; Busted →
  the player's feet at the car top ± 0.05 (roof), never at 1 m + ground. On today's code → wall top (RED; the flip
  is the old fallback). `forced_eject_takes_a_level_door_despite_a_body`: a dummy standing at the right door, low wall
  left → right door at ground level.
- **Step 30b `tests/police_fire_lines.rs` additions — O1 (correctness + liveness).** Rows: (a) cop in Attack at 2
  stars, player 20 m away, a parked car whose corner is 0.4 m off the line at 10 m: no `BulletHitVehicle` on that car
  in 1920 ticks and ≥ 1 `DamageDealt` cop → player (it repositions); mirrored and at 3 distances (6 cases); (b) the
  player drives car X with the same geometry through X: the cop fires and hits X (target car excluded); (c) gang
  gunman row (a) through `gang/behavior.rs`. Flips: empty car list → (a) RED; exclude nothing → (b) RED.
- **Step 30c `tests/traffic_bench.rs` (liveness + order-of-magnitude, not a CI budget).** Seed-1 city, 40 civilians,
  gang cap, player driving on an avenue at 5 stars (12 units incl. crew, 5 cars chasing), traffic 24: wait until
  `TrafficStats.cars ≥ 20` and 5 police cars (GATE BROKEN otherwise), then 640 ticks: print mean/p50/p95/max and
  `TrafficStats`, `VehicleLoad`, `RouteLoad`; assert mean < 10 × the implementer's measured probe mean (TASK-009
  lesson: gate the mean only). Record the per-tick mean against GDD §11 (physics + AI ≤ 4 ms) in the summary.
- **Step 30d `tests/traffic_parked.rs` (correctness, orchestrator note 4).** Seeds 1, 2, 3, player on an avenue
  sidewalk with parked cars, traffic full, 4096 ticks: zero `CollisionStart` between a traffic car and a parked car,
  zero switches caused by a parked car (the switch records its cause in a test-visible way: `TrafficStats` counts by
  cause), traffic OBBs never overlap parked OBBs. Flip: traffic on slot 1 → RED.

**Step 32. `tools/qa/scenarios/t15.py` (runtime AC, via `tools/qa/brp.py`).**
Seed 1, `--features dev`, release. 1) wait `Playing`, then until ≥ 12 `TrafficCar`s; read their speeds (mean > 3
m/s, max ≤ 16.5), screenshot the street. 2) Hijack: nearest kinematic traffic car; put the player on its lane 12 m
ahead (it stops: speed ≤ 0.5), then 1.5 m from its door point, `send_keys F` → `Driving` = that car within 1 s; a
`Civilian` in `Flee` within 3 m; screenshot. 3) `mutate_resources WantedLevel.heat = 180`; hold W on a straight
avenue in bursts; every 0.5 s read the flat distance from each `PoliceCar` to the player: at most 2 active police
cars ever; the nearest distance decreases (first value vs the minimum over 25 s); screenshots every 1 s (chase).
4) stop, F out: within 20 s a Dismounted car and cops with `CrewOf` near. 5) `get_diagnostics` and
`Game.frame_report()` (TASK-010 lesson: FPS only there). 6) shutdown; no `ERROR_WORDS` in the log. Scenario derives
enum names from source (TASK-015 lesson), never hard-codes lists.
Owner checklist (for QA_REPORT): streets have moving cars that queue and yield at crossings; a car jacked from the
flow, its driver runs; survived a car chase (police cars follow, ram, cops get out); sirens on cars; police model
and traffic look.

### 3.2 Final checks (implementer)
`cargo build -j 4`; `cargo clippy -j 4 -- -D warnings`; `cargo clippy --workspace --all-targets -j 4 -- -D warnings`;
`cargo test -p gta_sim -j 4`; `cargo test -p citygen -j 4` (untouched, must stay green); `cargo test -p gta_like --bin
gta_like -j 4` ×3; `python tools/qa/tree_check.py`; `cargo tree -p gta_sim -e normal -i bevy_render` empty;
`python tools/qa/scenarios/t15.py --out <dir>` and reruns of t8…t14. Files stay < 750 lines (split
`police/cars.rs` into `cars.rs` + `car_route.rs` if needed; `traffic/drive.rs` is the likely largest).

---

## 4. Risk areas

1. **Schedule cycles / ordering.** `VehicleSystems::Drive.after(PoliceSystems)` and the traffic sets add edges
   across `HealthSystems`, `NpcSystems`, `AiSystems`. A cycle panics at app build (every test catches it). If it
   happens, drop the `after(PoliceSystems)` edge and accept one tick of autopilot latency.
2. **Dynamic traffic quality.** Pure pursuit + P speed control on the T14 chassis may oscillate or clip curbs on
   connectors; mitigations are data (`lookahead_*`, `speed_gain`, `turn_speed`) and the `lost` → Stalled rule.
   Owner-run.
3. **Jams.** A stalled or knocked car in a lane stops everyone behind it until the bubble removes it (not while in
   frame). "Don't block the box" plus cyclic queues around a block could gridlock at high density (unlikely at 24).
   Owner-run; a `blocked_seconds` give-up is a follow-up, not in scope.
4. **Switch churn.** Pedestrians crossing close to cars and the player walking along them switch cars to dynamic;
   the closing-speed filter limits it. Bench and `TrafficStats.switches` show the rate.
5. **Existing city gates with traffic and police cars** (step 27). Traffic adds cars on roads in every city test
   with a camera view: flakes are possible in gates that put the player on a road. Handle per step 27, never by
   loosening thresholds.
6. **Unit accounting.** `dispatcher.units` now includes crew aboard; t11.py only checks `≤ row`, fine. A boarding or
   dismount that fails to spawn/despawn leaves the count wrong silently → row gates count foot + aboard every tick.
7. **Hijack position read.** Step 10 relies on the player's `Position` still being the standing spot when
   `on_hijack` runs; the entry pose is applied by commands (`seat.rs:451-457` inserts). If ordering proves otherwise,
   carry the spot in `VehicleEntered` (a new field) instead.
8. **Performance.** The largest population of the game. Bounded: N casts + N proximity queries (N ≤ 24), one A* per
   tick for police, occupancy sort, spawn scan only under the cap. Bench step 30c; GDD §11 budget is a measurement,
   not an assert.
9. **Sirens and visuals** (police model scale 1.4, offsets) — owner-run; t13 must accept car parents.
10. **Visual gap:** traffic cars have no visible driver model (the sedan windows are dark at the camera distance);
    owner-run (Q-В).

## 5. Open questions (для владельца / оркестратора; у каждого есть рекомендация по умолчанию)

**Q-А. Копы возвращаются в свою машину, если игрок уехал (шаг 12).** В GDD §5.3 этого нет, но без этого после
первой высадки погоня на машинах на 1-2 звёздах пропадает: высаженные копы держат лимит юнитов, новые машины не
приходят.
- (a) Возвращаются в машину и продолжают погоню (как в GTA IV/V). **Рекомендую.**
- (b) Не возвращаются; экипаж на борту не входит в лимит юнитов (до 22 копов на 5 звёздах, выше бюджета GDD §6.1).
- (c) Не возвращаются; высаженные копы исчезают, когда игрок уехал дальше 60 м (проще, но видно исчезновение).

**Q-Б. Раненый выстрелом водитель трафика.** Сейчас пули в салон машины трафика ничего не делают (водитель
не сущность). (a) Оставить так в T15 (рекомендую, вне скоупа задачи); (b) водитель выскакивает и убегает при
попадании в салон (+1 система, дешёвая).

**Q-В. Разнообразие трафика.** (a) Все машины трафика это седан, полиция своя модель (рекомендую для T15);
(b) добавить такси Kenney (sha256 `3803539718ffd3b84b515dbce8ed6b489f1ff5be58edb1903d2c5db5c584bdc7`, тот же
архив) по броску внешности.

**Q-Г. Удаление машины в кадре дальше 90 м через 2 с** (правило GTA III/VC + SA). Может быть видно на длинной
авеню. (a) Оставить 90 м в `traffic.ron` (рекомендую, крутится данными); (b) в кадре не удалять вовсе, только
вне кадра.

children: 0 launched / 0 reported.
