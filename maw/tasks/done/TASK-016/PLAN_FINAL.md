# PLAN_FINAL — TASK-016 (GDD T15: traffic and police cars)

Reviewer: plan-reviewer-2. Sources: `PLAN.md` (planner, detail), `PLAN_V2.md` (plan-reviewer-1, corrections),
`TASK_FINAL.md` (Q-А a, Q-Б b, Q-В b, Q-Г a; binding notes O1, O2, forced eject). V2 corrections win over PLAN.md;
this review's corrections (section 5, F1..F14) win over both. This file is the whole plan: an implementer does not
need PLAN.md or PLAN_V2.md.

Cost of error: HIGH, silent class. Kinematic bodies push with infinite mass; two kinematic cars pass through each
other with no solver response (probe Q2b); an intersection can grant two conflicting movements; a car can despawn in
view; unit and car caps can drift; car entities can leak; the largest population in the game costs frame time. Full
evidence layer for the traffic core, dispatcher caps, cabin wound and bail-out, pull-out, fire line and forced
eject. Feel goes to the owner's run: density, turn look, chase handling, sirens, models.

---

## 1. Summary

A new headless gameplay domain `crates/gta_sim/src/traffic/` (one `TrafficPlugin`, data `assets/traffic/traffic.ron`)
builds a `TrafficGraph` from the city's slot-0 (inner) lanes with Bézier connectors and per-intersection conflict
tables. Traffic cars are the T14 sedan body (`vehicle_bundle`) switched to `RigidBody::Kinematic`. They drive a lane
cursor with IDM and a ballistic integrator, reserve whole conflicting connectors first-come-first-served, and spawn
and despawn by the Vermeij bubble (70/90 m in frame, 15/25 m off frame, ≥ 2 s off frame, cap 24). A time-to-contact
test switches a kinematic car to dynamic before any contact with a dynamic body; a `CollisionStart` backstop covers
the rest. A dynamic car follows its lane path through a shared pure-pursuit `Autopilot` in `vehicle/`. Hijacking
throws the data-only driver out as a fleeing civilian. A cabin shot makes the driver brake, get out and flee (Q-Б),
and a new `DriverScared` message turns that shot into a `Shooting` incident. Every car the AI gives up becomes
`Abandoned` and follows the bubble rule, so no car entity leaks. In `police/`, `dispatch_police_cars` keeps
`row.cars` police cars (1..5); their crews count toward `row.units`, and the foot dispatcher reserves the seats of
missing cars. Police cars route by A* over lanes, chase a driver directly within 40 m in sight, dismount the crew
within 20 m, and re-board when the player drives off (Q-А). The TASK-015 notes land too: the cabin wounds a seated
driver, a cop pulls a stopped driver out at 1 star through the left door, cars in the shooter→target segment block
fire, and the forced eject never lands on a wall top. Client: police and taxi models (Q-В), sirens on police cars, a
minimap marker. QA: `tools/qa/scenarios/t15.py`.

---

## 2. Implementation steps

Ticks are 64 Hz (`Time::<Fixed>::default()`; `lib.rs:133` already derives `tick` from it). Every tuning number
lives in the named RON file; the only new `const`s are laws, named where added. Before coding read
`maw/project-context/domains/{bevy-ecs,gates,game-design}.md`. Navigate by symbol; line numbers below were
re-verified in this review where given.

### Phase A — data and graph

**Step 1. `assets/traffic/traffic.ron` (new) + `crates/gta_sim/src/traffic/config.rs` `TrafficConfig`**
(`#[serde(deny_unknown_fields)]`, `validate()`, loaded in `compose_sim` like the others with
`pub const TRAFFIC_CONFIG: &str = "traffic/traffic.ron"`).
```
(
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
    switch: (reach: 5.0, horizon_seconds: 0.1, skin: 0.1),   // time-to-contact switch (step 8)
    lost: (distance: 4.0, angle_deg: 60.0),
)
```
Comments in the file state the GDD source of each number (§5.2 IDM start values, Vermeij bands, cap 24).
`validate`: all finite; positive where physical; `min_gap > 0`; `max_deceleration >= comfortable_deceleration`;
`off_view.spawn < off_view.despawn <= in_view.spawn < in_view.despawn`; `max_cars >= 1`; `spawns_per_tick >= 1`,
`initial_spawns_per_tick >= 1`; `connector_samples >= 3`; `0 < lost.angle_deg < 180`; `switch.skin >= 0`;
`switch.horizon_seconds >= 2 * tick` (tick passed in; law: the sweep must cover at least two steps).
Cross-config in `compose_sim` (next to `validate_ring`): `switch.reach >= (vehicle.max_speed + max(desired_speed.avenue,
desired_speed.street)) * horizon_seconds` (shipped: (28 + 16) · 0.1 = 4.4 <= 5.0; the broadphase must contain every
body the sweep can reach) and `bubble.in_view.despawn < population.despawn_distance` (90 < 150).
`pub const IDM_DELTA: i32 = 4` in `traffic/idm.rs` (law: part of the GDD formula).
Check: step 26 gates.

**Step 2. Other data files.** Append new fields at the END of existing tuples/structs.
- `assets/police/escalation.ron`: each star row gets `cars:` 1 / 2 / 3 / 4 / 5 at the end of the row (GDD §6.4 table);
  `arrest` gets `pull_out_seconds: 1.0`; new block after `spawns_per_tick`:
  `car: (crew: 2, spawn_ring: (60.0, 120.0), spawns_per_tick: 1, pursuit_speed: 20.0, turn_speed: 8.0,
  dismount_distance: 20.0, direct_chase_distance: 40.0, stopped_seconds: 1.0, reboard_distance: 25.0,
  reboard_timeout_seconds: 10.0, route_refresh_seconds: 1.0, routes_per_tick: 1)`.
  Types in `police/mod.rs`: `EscalationRow.cars: u32`, `ArrestConfig.pull_out_seconds: f32`, `PoliceCarConfig`,
  `EscalationConfig.car`. Validate: row `cars >= 1` and non-decreasing over the rows; `crew >= 1`;
  `car.spawn_ring` is a band (`0 < inner < outer`) and `outer < population.despawn_distance` (extend `validate_ring`);
  `dismount_distance < direct_chase_distance`; everything else positive; `routes_per_tick >= 1`.
  **Re-anchor `tests/config_police.rs every_star_has_a_row` (line 98):** its whole-row literal becomes
  `"(units: 12, swat: 12, reinforce_seconds: 3.0,  arrest: false, surround: true, cars: 5),"`, exactly as written in
  the file (same spacing); re-run its flip (row deleted → error contains "length 5"). The other row sabotages
  (`"(units: 8,  swat: 4,"`, `"(units: 6,  swat: 0,"`) are prefixes and still match.
- `assets/wanted/wanted.ron`: `cop_car_view_distance: 50.0` (GDD §6.4 "50 м из машины"); validated positive and
  `>= cop_view_distance`.
- `assets/vehicle/sedan.ron`: `cabin: (centre: (0.0, 0.45, 0.1), half_extents: (1.25, 0.5, 1.0))` (body frame; the
  x half extent and the top reach past the chassis 1.2 / 0.92, so surface hit points count) and
  `autopilot: (lookahead_min: 4.0, lookahead_per_mps: 0.5, speed_gain: 0.5, stuck_speed: 0.5, stuck_seconds: 2.0,
  reverse_seconds: 1.5)`. `VehicleConfig.cabin`, `.autopilot`; validate half extents > 0, `lookahead_min > 0`,
  every other field positive.
- `assets/vehicle/damage.ron`: `vehicle.cabin_driver_share: 0.5`, validated in [0, 1].
- `assets/world/render.ron` (client): `police_vehicle` and `taxi_vehicle` with the `vehicle` struct, plus `taxi_share`:
  ```
  police_vehicle: (model: "third_party/car-kit/police.glb", scale: 1.4, offset: (0.0, -1.16, -0.14),
                   wheels: ("wheel-front-left", "wheel-front-right", "wheel-back-left", "wheel-back-right")),
  taxi_vehicle:   (model: "third_party/car-kit/taxi.glb", scale: 1.48, offset: (0.0, -1.16, -0.037),
                   wheels: ("wheel-front-left", "wheel-front-right", "wheel-back-left", "wheel-back-right")),
  taxi_share: 0.25,
  ```
  Derivation for the comment (verified by `scratch/carkit/glb_probe.py sedan taxi police` in this review):
  scale = 4.08 m chassis / body length (sedan 2.55 → 1.6 as shipped, taxi 2.75 → 1.48, police 2.90 → 1.4);
  `offset.y = -1.16` puts the wheel bottoms (model origin) at the road under a chassis centre at rest height 1.1596;
  `offset.z = scale × (body node z + body mesh centre z)` (the 180° model yaw mirrors it; sedan 1.6 × −0.025 = −0.04
  as shipped, taxi 1.48 × (−0.025 + 0) = −0.037, police 1.4 × (−0.025 − 0.075) = −0.14). `taxi_share` validated in
  [0, 1]. `vehicle_asset_paths` (`src/visuals/config.rs:240`) yields all three model paths.
- `assets/audio/mix.ron`: `siren.car_height: 1.0` (m above the chassis centre, roof light bar); validated positive.
- `assets/third_party/manifest.ron`, pack `car-kit`: add
  `(archive: "Models/GLB format/police.glb", path: "police.glb", sha256: "a617b880594f56239b7ac4cf6fd4d74ebdae3ba6c5d842e77c76a5906eff352a")`
  and `(archive: "Models/GLB format/taxi.glb", path: "taxi.glb", sha256: "3803539718ffd3b84b515dbce8ed6b489f1ff5be58edb1903d2c5db5c584bdc7")`
  (both re-hashed in this review from the cached zip, sha256 `fac7dac…d0c4` = manifest `archive_sha256`). Fetch with
  `python tools/fetch_assets.py --cache maw/tasks/done/TASK-015/scratch/carkit`.
- `tests/new_city.rs` literals (line ~301 and ~306): `PoliceDispatcher { units: 3, swat: 1, reinforce_left: 2.0, cars: 2 }`
  and `ArrestAttempt { cop: Some(unit), hold: 1.0, pull: 0.5 }` (non-zero, so the reset is really tested; see step 27).
Check: `cargo test -p gta_sim --test config --test config_police --test config_vehicle --test asset_manifest -j 4`.

**Step 3. `traffic/graph.rs` — `TrafficGraph` (Resource, `Debug`).**
```rust
#[derive(Reflect, Clone, Copy, Debug, PartialEq, Eq)]
pub enum Segment { Lane(u32), Connector(u32) }
pub struct TrafficLane { pub from: Vec3, pub to: Vec3, pub dir: Vec3, pub length: f32, pub v0: f32,
                         pub end_node: u32, pub out: Vec<u32 /*connector*/> }
pub struct TrafficConnector { pub from_lane: u32, pub to_lane: u32, pub node: u32, pub points: Vec<Vec3>,
                              pub cumulative: Vec<f32>, pub length: f32, pub conflicts: Vec<u32> }
pub struct TrafficGraph { lanes: Vec<TrafficLane>, connectors: Vec<TrafficConnector>,
                          spawn_points: Vec<(u32 /*lane*/, f32 /*s*/)> }
impl TrafficGraph {
  pub fn new(lanes: Vec<(Vec3, Vec3, f32 /*v0*/, u32 /*end node*/)>, connectors: &[(u32, u32, u32 /*node*/)],
             cfg: &TrafficConfig, half_width: f32) -> Result<Self, String>;   // synthetic graphs in tests
  pub fn from_layout(layout: &CityLayout, params: &CityParams, cfg: &TrafficConfig, half_width: f32)
             -> Result<Self, String>;
  pub fn pose(&self, seg: Segment, s: f32) -> (Vec3 /*point, y = 0*/, Vec3 /*unit tangent*/);
  pub fn length(&self, seg: Segment) -> f32;
  pub fn nearest(&self, p: Vec3) -> Option<(Segment, f32 /*s*/, f32 /*distance*/)>;   // police routing goal
}
```
- `from_layout` (citygen untouched; `LaneGraph` in `crates/citygen/src/layout.rs`, lanes built in `graphs.rs`): keep
  lane k iff `slot(k) == 0`, `slot = round(|cross(d_edge, lane.from − node_a)| / lane_width − 0.5)` (worked: inner
  avenue lane offset 1.625 → 0; curb lane 4.875 → 1; street lane 1.625 → 0). v0 from the edge class
  (`desired_speed`). Connectors = citygen connectors whose both lanes are kept. y = 0 (road top). The curb lane stays
  for parked cars.
- Curve: p0 = from-lane end, p2 = to-lane start; control = intersection of the lines `p0 + t·d_in` and `p2 − u·d_out`,
  midpoint of p0/p2 when `|d_in × d_out| < 1e-3`; `connector_samples` points; cumulative arc length.
- Conflicts (only connectors of the same node): `i != j` conflict iff same `from_lane`, or same `to_lane`, or the
  minimum segment-segment distance between their polylines `< 2·half_width + conflict_margin`.
- Spawn points every `bubble.spawn_spacing` along each lane (s from `spawn_spacing / 2`).
- Errors: a kept lane with no outgoing connector (a dead end would park cars forever); NaN or zero-length lane.
- System `build_traffic_graph` on `OnTransition { exited: Loading, entered: Playing }` `.run_if(resource_exists::<City>)`
  (reads `City`, `CityParamsRes`, `TrafficConfig`, `VehicleConfig.half_extents().x`); on error log and write
  `AppExit::error()` like `build_sidewalk_graph`. `NEW_CITY` removes the resource.
Unit tests in `graph.rs`: slot rows (inner / curb / street, both directions); right-angle connector: p0 (0,0,0),
d_in −Z, p2 (5,0,−5), d_out +X → control (0,0,−5), Bézier midpoint (1.25, 0, −3.75), curve starts at p0, ends at p2,
end tangents equal the lane directions; a + intersection (4 in, 4 out, 12 connectors): straight N→S conflicts with
straight E→W, not with itself; a same-source pair conflicts; a same-destination pair conflicts.
Seed property gate: step 26.

**Step 4. `traffic/idm.rs` — pure kernel.**
```rust
pub fn idm_acceleration(v: f32, v0: f32, gap: Option<(f32 /*s*/, f32 /*dv = v − v_leader*/)>, c: &IdmConfig) -> f32
// free road when gap is None; s clamped at >= 1e-3; result clamped to >= -max_deceleration
pub fn ballistic_step(s: f32, v: f32, a: f32, dt: f32) -> (f32, f32)
// v' = v + a·dt; if v' < 0 { (s − v²/(2a), 0.0) } else { (s + v·dt + a·dt²/2, v') }
```
Worked rows (shipped a 0.73, b 1.67, T 1.5, s0 2; all re-derived in this review): free v=0 → 0.73; free v=v0=12 → 0;
v=0, s=2, dv=0 → s*=2 → 0; v=0, s=1 → 0.73·(1−4) = −2.19; v=12, s=30, dv=0 → s* = 2 + 18 = 20,
a = 0.73·(1 − 1 − 0.4444) = −0.3244; `ballistic_step(0, 1, −8, 1/64)` → (0.0146484, 0.875);
`ballistic_step(0, 0.1, −8, 1/64)` → (0.000625, 0.0); `ballistic_step(5, 0, −2.19, 1/64)` → (5, 0).
Sources: Treiber & Kanagaraj 2015 (ballistic scheme with stops); traffic-simulation.de/info/info_IDM.html.
Check: `cargo test -p gta_sim traffic::idm`.

### Phase B — the traffic loop

**Step 5. `traffic/mod.rs` — components, resources, messages, plugin.**
- `TrafficCar { segment: Segment, s: f32, speed: f32, next: Option<u32>, mode: TrafficMode, waiting: Option<u64> }`
  (`Component`, `Reflect`, `#[reflect(Component)]`, `#[require(Offscreen)]`).
  `TrafficMode { Kinematic, Dynamic, Bailing { attack: Option<u32> }, Taken, Abandoned }` (one enum, no marker
  churn). Meaning:
  - `Kinematic` / `Dynamic`: AI-driven, the data driver is aboard.
  - `Bailing { attack }`: the driver is scared (cabin shot, `attack: Some`) or the car is wrecked (health 0,
    `attack: None`, F3); it brakes to a stop and the driver gets out (step 7).
  - `Taken`: the player sits in it. Never despawned, not counted toward `max_cars`.
  - `Abandoned`: dynamic, no AI, no `Autopilot`, no own `DriveIntent`, no `SleepingDisabled`, no reservations; counted
    toward `max_cars`; bubble despawn rule. Entered when the player leaves a `Taken` car (incl. forced eject), when a
    bail-out finishes, and on the `lost` rule. PLAN's `Stalled` is merged into it.
  Parked cars stay plain `Vehicle`s (city lifetime).
- `TrafficRng(pub ChaCha8Rng)` (`seed_from_u64(seed)`, `set_stream(4)`; `unit()` via `unit_f32`, `next_u32()`),
  reseeded `OnEnter(Loading)` from `CitySeed` like `reseed_police`. Streams 1-3 are taken (NpcRng, gangs, police), 5
  goes to `PoliceCarRng`.
- `TrafficIntersections` (Resource): per node `occupants: Vec<(u32 connector, Entity)>`,
  `waiters: Vec<(u64 tick, Entity, u32 connector)>`; cleared on `NEW_CITY`.
- `TrafficStats` (Resource, Reflect, read by QA and the bench): `cars, kinematic, dynamic, bailing, abandoned, taken,
  spawned, despawned, casts, switches_by_cause: [u32; 4]` (causes Character, Vehicle, Backstop, Hijack); `casts` is
  the fixture liveness counter (F2). Reset on `NEW_CITY`.
- `TrafficPhase { InitialFill, Steady }` (Resource, reset to `InitialFill` on `NEW_CITY`).
- Message `DriverScared { shooter: Entity, attack: u32, vehicle: Entity }` (Reflect, registered here, cleared on
  `NEW_CITY`).
- Sets (FixedUpdate), all `.in_set(NpcSystems).run_if(resource_exists::<TrafficGraph>)`:
  `TrafficSystems::Hijack.after(VehicleSystems::Enter)`;
  `TrafficSystems::Bail.after(VehicleSystems::Bullets).before(WantedSystems)`;
  `TrafficSystems::Drive.after(TrafficSystems::Hijack).after(TrafficSystems::Bail).before(VehicleSystems::Drive)`;
  `TrafficSystems::Bubble.after(TrafficSystems::Drive)`.
  Changes in `vehicle/mod.rs configure_sets`: `VehicleSystems::Bullets.after(HealthSystems::Damage).before(HealthSystems::Death)`
  and `VehicleSystems::Drive.after(PoliceSystems)` (added in `VehiclePlugin`, which imports `PoliceSystems`).
  Verified in this review: no existing edge closes a cycle (`NpcSystems` itself is unordered; only the
  `(Perceive, Decide, PopulationSystems)` chain is `.after(HealthSystems::Death)`; `WantedSystems` is after
  `Decide`; `PoliceSystems` after `PopulationSystems` and `WantedSystems`; nothing is ordered after
  `VehicleSystems::Drive`). A cycle panics at app build in every test.
- FixedPostUpdate: `contact::switch_to_dynamic.before(PhysicsSystems::First)` (no state gate: FixedPostUpdate does not
  run while paused; it must run while Wasted/Busted, traffic keeps moving).
- `lib.rs compose_sim`: load `TrafficConfig`, validate (+ cross-config), `insert_resource`, and
  `app.add_plugins(TrafficPlugin)` right after `app.add_plugins(VehiclePlugin)` (outside the 15-tuple).
Check: any test builds the app.

**Step 6. `vehicle/autopilot.rs` — shared AI driving (vehicle domain, `VehicleConfig.autopilot`).**
- `Autopilot { target: Vec3, speed: f32, stuck: f32, reverse_left: f32 }` (Component, Reflect). The car carries its
  own `DriveIntent` next to it. System `steer_autopilots` in `VehicleSystems::Drive`, `(steer_autopilots,
  drive_vehicles).chain()`:
  - forward `f = rot·(−Z)`, right `r = f × Y`, `d = flat(target − position)`, `α = atan2(d·r, d·f)`,
    `L = max(|d|, lookahead_min)`, `κ = 2 sin α / L` (Coulter 1992), `δ = atan(κ · 2·half_wheelbase)`,
    `intent.steer = clamp(δ / steer_limit(cfg, v_fwd), −1, 1)`.
  - throttle: `err = speed − v_fwd`; `err > 0` → `min(err·speed_gain, 1)`; else if `v_fwd > hold_speed` →
    `max(err·speed_gain, −1)` (brake); else 0 (hold). Never reverse by accident: `drive_force` reverses on a negative
    throttle below `hold_speed`.
  - stuck: throttle > 0.5 and `|v| < stuck_speed` for `stuck_seconds` → `reverse_left = reverse_seconds`; while
    `reverse_left > 0`: throttle −1, steer negated.
  Directional rows (re-derived here; sign errors pass magnitude tests), car at the origin, target 10 m ahead and 5 m
  right: yaw 0 (f −Z, r +X): target (5,0,−10) → d·r = 5 > 0 → steer > 0 (right); yaw +90° (f −X, r −Z): target
  (−10,0,−5) → steer > 0; yaw 180° (f +Z, r −X): target (−5,0,10) → steer > 0; the mirrored targets give steer < 0.
  `wheel_forward(+δ)` has +x = right, consistent with `chassis.rs`.
- `chassis.rs drive_vehicles`: add `&RigidBody` to the vehicle query; `!rb.is_dynamic()` → `continue` before rays and
  forces (kinematic cars get their wheel state from traffic). Intent = the driver's `DriveIntent`, else the car's own
  (`.or_else(|| intents.get(entity).ok())`), else default.
Check: unit rows in `autopilot.rs`; `tests/vehicle.rs` stays green.

**Step 7. `traffic/drive.rs` — `advance_traffic` (TrafficSystems::Drive), one bounded system.**
Per tick, deterministic order: cars sorted by `Entity::to_bits()` (query order is not spawn order, TASK-015). Skip
`Taken` and `Abandoned` cars.
1. Dynamic and dynamic-`Bailing` cars: re-project `(segment, s)` from `Position` onto their own path (current segment,
   advance to `next` connector / destination lane when `s >= length`); `lost` rule (farther than `lost.distance` from
   the path, heading off the tangent by more than `lost.angle_deg`, or upside down) → `abandon(car)`.
2. Wreck rule (F3): `VehicleHealth.current <= 0` on a `Kinematic` or `Dynamic` car → `Bailing { attack: None }`.
3. Occupancy: per segment a sorted `Vec<(s, Entity, speed)>` of the AI cars (Kinematic, Dynamic, Bailing).
4. Intersections: release every occupant whose rear (`s − half_length`) has left its connector; for each car on a
   lane within `request_distance = speed²/(2·comfortable_deceleration) + min_gap + half_length` of the lane end:
   choose `next` if unset (uniform over `lane.out`, `TrafficRng`), queue as waiter (tick stamp once), grant in FCFS
   order (tick, then entity bits) when no occupant's and no earlier waiter's connector conflicts with its own and the
   destination lane has room (its last car has `s − half_length >= length_of_it + min_gap`, "don't block the box").
   A `Bailing` car keeps its grant until it leaves the connector or turns `Abandoned`.
5. Gap for each AI car = min of: the leader along its path within `look_ahead` (current segment, then its chosen
   connector, then that connector's lane); the stop line (lane end − half_length) if `next` is not granted; the
   forward cast hit − half_length. Forward cast: `SpatialQuery::cast_shape_predicate` with a cuboid of the chassis
   half width/height, from the car centre along its tangent, max distance `half_length + sense_distance` on a lane,
   `half_length + turn_sense_distance` on a connector, mask `[Character, Vehicle]`, predicate excludes itself and every
   `Kinematic` traffic car (they are in the occupancy); leader speed of a cast hit = its `LinearVelocity` along my
   tangent. `TrafficStats.casts += 1` per cast.
6. Acceleration: IDM with v0 = lane v0 (connector: `turn_speed`); a `Bailing` car uses `a = −max_deceleration` (brakes
   along its path, keeps its reservation). Kinematic: `ballistic_step`; crossing a lane end with a grant → move into
   the connector (carry the excess s), connector end → destination lane; a lane end WITHOUT a grant clamps `s` at the
   stop line with `v = 0` (a driver never runs the line; the AC2 gate then tests the reservation, not the brakes).
7. Kinematic actuation: `(p', t') = pose(s')`; `LinearVelocity = ((p'.x, rest_height, p'.z) − position)/dt` with
   `y` component `(rest_height − y)/dt`; `AngularVelocity = (0, wrap(yaw(t') − yaw)/dt, 0)` (closed loop, probe Q1);
   `vehicle.steer = clamp(atan(2·half_wheelbase · yaw_rate / max(v, 1)), ±steer.max_deg)` (visual only).
   Dynamic: `Autopilot { target: pose(s + max(lookahead_min, lookahead_per_mps·v)).0, speed: max(0, v + a·dt), .. }`.
8. Bail-out finish: a `Bailing` car that stands (kinematic: `speed == 0`; dynamic: `|v| <= hold_speed`) asks
   `vehicle::exit_spots` (step 16) for the first clear spot (left door first); none clear → retry next tick. On a spot:
   spawn the driver civilian (step 10 helper) there in `Flee { from: shooter position if attack is Some and the
   shooter still exists, else the car position, left: roll(flee_distance), about: attack.map(Cause::Attack) }`, then
   `abandon(car)`.
`abandon(car)`: `insert(RigidBody::Dynamic)` (no-op if already), `remove::<(Autopilot, DriveIntent, SleepingDisabled)>`,
release its reservations, mode `Abandoned`.
Budget: O(N log N) + N casts, N ≤ 24; intersections ≤ waiters × occupants per node.
Check: gates 21, 22.

**Step 8. `traffic/contact.rs` — time-to-contact switch (FixedPostUpdate, before `PhysicsSystems::First`).**
For each `TrafficCar` with `mode == Kinematic` or a kinematic `Bailing` car:
- broadphase `shape_intersections` with the chassis cuboid grown by `switch.reach` (half extents + reach), mask
  `[Character, Vehicle]`, excluding itself;
- a hit counts when its body is `RigidBody::Dynamic` and has no `RigidBodyDisabled`. Sleeping bodies COUNT with
  `v_other = 0` (F4);
- flat relative displacement over the horizon `d = (v_other − v_car)·horizon_seconds`; the car's flat rectangle
  (from `Position`/`Rotation`, chassis half extents x/z grown by `skin`) against the other footprint swept by `d`
  (character: circle of `capsule_radius`; car: its flat rectangle) — overlap at any point of the sweep, including
  t = 0 → switch.
Pure fns in `contact.rs` with unit rows: `swept_circle_hits_rect(centre, radius, d, rect) -> bool`,
`swept_rect_hits_rect(other, d, rect) -> bool` (2D SAT on the Minkowski sweep). Rows (re-derived here):
- player car 10 m/s into the rear of a stopped car, gap 0.5 m: sweep 1.0 m > 0.5 → true (the switch fires once the gap
  is ≤ 1.1 m, ~7 ticks before contact; probe Q4 needs ≥ 1);
- pedestrian 1.2 m ahead of the bumper walking across at 1.4 m/s: sweep 0.14 m sideways, the circle stays 0.8 m off the
  grown bumper → false;
- oncoming car in the opposite inner lane, 3.25 m centre to centre (0.85 m clear), closing 28 m/s: sweep 2.8 m along
  the axis, lateral gap 0.85 > skin → false;
- sleeping parked car 0.85 m beside a passing car at 16 m/s: false;
- sleeping car 1.0 m ahead of a kinematic car at 12 m/s: sweep 1.2 m → true.
Backstop: `MessageReader<CollisionStart>`: a kinematic traffic car vs a `RigidBody::Dynamic` body → switch.
Switch = `commands.entity(car).insert((RigidBody::Dynamic, SleepingDisabled, Autopilot { target: position +
forward·lookahead_min, speed: v, stuck: 0.0, reverse_left: 0.0 }, DriveIntent::default()))`; mode `Dynamic`
(`Bailing` stays `Bailing`, and its dynamic branch applies); `switches_by_cause[cause] += 1`. The insert takes
effect in the same step (probe Q3).
Check: gate 23.

**Step 9. `traffic/spawn.rs` — `despawn_traffic`, `spawn_traffic` (TrafficSystems::Bubble, `.chain()`).**
- Needs `CameraView` (`Some`) and the player, like `despawn_far` / `spawn_civilians`; without a view both return (so
  synthetic-lane gates without a view never spawn or despawn).
- Frame test per car (F: R12): `in_frame = (any of the four chassis-top corners or the centre vertical line is inside
  the view cone) && flat_distance(car, player) <= in_view.despawn`. `Offscreen += dt` while not in frame, else 0.
- Despawn iff mode != `Taken` && `Offscreen >= offscreen_seconds` && flat distance > `off_view.despawn`
  (Abandoned and Bailing included); release reservations; `try_despawn`.
- Spawn while `cars < max_cars` (cars = every non-`Taken` `TrafficCar`), at most `spawns_per_tick`
  (`initial_spawns_per_tick` in `InitialFill`, band [off_view.spawn, in_view.despawn) regardless of view, then
  `Steady`). Steady candidates from `spawn_points`: band = [in_view.spawn, in_view.despawn) if the point is in frame
  by the same test, else [off_view.spawn, off_view.despawn); skip points closer than `v0²/(2b) + s0 + half_length` to
  the lane end; skip when the nearest car on that lane (occupancy) is within `s0 + v0·T + length` behind or ahead;
  skip when `shape_intersections` of the chassis box finds anything on `Character | Vehicle`. Choice by `TrafficRng`.
- Spawn (F1, bundle duplicates panic in bevy_ecs 0.19.1 `bundle/info.rs:118`):
  `commands.spawn((vehicle_bundle(&cfg, &dmg, transform), TrafficCar { segment, s, speed: v0, next: None, mode:
  Kinematic, waiting: None }, Appearance(rng.next_u32())))` then `.insert((RigidBody::Kinematic, Name::new("Traffic
  car")))`. `vehicle_bundle` already holds `RigidBody::Dynamic` and `Name`, so both go in the follow-up insert; the
  client's `On<Add, Vehicle>` observer sees `TrafficCar` and `Appearance` because they are in the spawn bundle.
  Pose from the graph at `rest_height`, yaw from the tangent, wheel states at rest (`compression =
  static_compression`, `grounded: true`).
Check: gate 24.

**Step 10. `traffic/hijack.rs` — `on_hijack`, `on_leave` (TrafficSystems::Hijack).**
- `on_hijack` reads `VehicleEntered` for cars with `TrafficCar`:
  - mode `Kinematic | Dynamic | Bailing`: spawn the driver civilian (helper below) at the player's pre-entry feet
    (the player's `Position` is still the standing spot: `enter_exit` only inserts `Driving` etc., the seat pose is
    written by `sync_seats` in FixedPostUpdate), `Flee { from: player position, left: roll(flee_distance), about:
    Some(Cause::Attack(entered.attack)) }`; `switches_by_cause[Hijack] += 1` if it was kinematic.
  - mode `Abandoned` (re-entry of a car the player left or a bailed-out car): NO civilian (F5).
  - every case: `insert(RigidBody::Dynamic)`, `remove::<(Autopilot, DriveIntent)>`, release reservations, mode `Taken`.
  CarTheft is recorded by the existing `record_crimes` (`VehicleEntered.first`).
- `on_leave`: a `Taken` car whose `Vehicle.driver` is `None` → `abandon(car)` (covers F out and `eject_all`).
- Driver helper `spawn_driver(commands, .., feet, state)`: `civilian_bundle` with a walker on the nearest sidewalk
  edge (`nearest_node` + its first neighbour, `t = 0`), then `.insert(Transform::from_translation(feet + Y·float_height))`
  (a Transform, not only a Position: avian copies GlobalTransform into Position, TASK-011), then set the state and
  `flee_start(graph, walker, from)`. Temperament and the `left` roll come from `TrafficRng`, never `NpcRng`
  (TASK-010). Generalise `civilian::roll_temperament(rng: &mut ChaCha8Rng, spread)` using `unit_f32`; the two
  existing callers pass `&mut rng.0` (same draws in the same order, so civilian density gates do not move).
- The police car variant is step 14.
Check: gate 25.

**Step 10b. `traffic/bail.rs` — `bail_out` (TrafficSystems::Bail).** Reads `CabinHit` (step 17) for cars in
`Kinematic | Dynamic`: mode `Bailing { attack: Some(hit.attack) }` (first hit wins; a car already `Bailing` ignores
later hits), writes `DriverScared { shooter, attack, vehicle }`. The stop-and-exit transition runs in `advance_traffic`
(step 7.8).

### Phase C — police cars (`police/cars.rs`; split `police/car_route.rs` if > 750 lines)

**Step 11. Components and dispatch.**
- `PoliceCar { state: PoliceCarState, crew: Vec<UnitKind>, stopped: f32, reboard_left: f32 }` (Reflect,
  `#[require(Offscreen)]`), `PoliceCarState { Respond, Chase, Dismounted, Leave, Taken, Abandoned }` (Reflect),
  `PoliceCarRoute { lanes: Vec<u32>, next: usize, goal: Option<u32>, age: f32 }`, `CrewOf { car: Entity }` on dismounted
  cops (separate component: the `PoliceUnit` literal in `src/audio/event_gate.rs` keeps compiling),
  `PoliceCarRng` (`set_stream(5)`, reseeded on `OnEnter(Loading)`).
- `PoliceDispatcher` gets `cars: u32` (active cars: Respond, Chase, Dismounted).
- `dispatch_police_cars` (`.in_set(PlayingSystems)`, `.run_if(resource_exists::<TrafficGraph>)`), in the
  `PoliceSystems` chain before `dispatch_police`: count active cars and crew aboard; while `cars < row.cars` and
  `units + 1 <= row.units` (units = foot + aboard), spawn a car with `min(car.crew, row.units − units)` crew, kinds by
  `spawn_kind` (SWAT share first), at most `car.spawns_per_tick` per tick. Spawn point: traffic `spawn_points` within
  `car.spawn_ring` of the player, hidden (all four roof corners `outside_cone`, or `occluded` within the shared
  `PopulationLoad` ray budget), chassis box free (`shape_intersections` on `Character | Vehicle`), chosen by `pick_spawn`
  near `last_known` (surround rows spread). Needs `CameraView`, `last_known` and a star row, like `dispatch_police`.
  Spawn (F1): `commands.spawn((vehicle_bundle(..), PoliceCar {..}, PoliceCarRoute::default(), Autopilot {..},
  DriveIntent::default(), SleepingDisabled))` then `.insert(Name::new("Police car"))`; dynamic, heading = lane
  direction.
- `dispatch_police` (foot): `active` also holds every crew member aboard (at its car's position), so
  `(dispatcher.units, dispatcher.swat)` = foot + aboard. **Seat reservation (F6):** add `Option<Res<TrafficGraph>>`
  and `Query<&PoliceCar>`; while a `TrafficGraph` exists, the foot dispatcher spawns only while
  `units + reserved < row.units`, `reserved = row.cars.saturating_sub(active_cars) × car.crew`. Result at full rows:
  1★ 2 by car, 0 on foot; 2★ 4/0; 3★ 6/0; 4★ 8/0; 5★ 10/2. Without it the 1-per-tick race of the two dispatchers fills
  row 5 as 4 cars + 4 foot and the fifth car never comes. No `TrafficGraph` (test area) → `reserved = 0`, every
  test-area police gate unchanged.
Check: gate 28 rows 1..5.

**Step 12. `drive_police_cars` (PoliceSystems, after `dispatch_police_cars`, before `dispatch_police`).**
- Senses: `sees = cop_sees(spatial, car_eye, rot·(−Z), eye(player), cfg, cfg.cop_car_view_distance)` with
  `car_eye = position + rot·seat` (inside the hull, so `sight_blocked` skips the own car). `cop_sees` gets a `distance`
  parameter; `police_fsm` and `track_search` pass `cfg.cop_view_distance` (R12). `driving` = player has `Driving`;
  player car speed; flat distance.
- Pure `next_car_state(state, &CarSenses) -> PoliceCarState`, table-gated one case per row:
  - any of Respond/Chase/Dismounted → `Leave` at 0 stars;
  - Respond → Chase iff driving && sees && d <= `direct_chase_distance`; Chase → Respond iff not that;
  - Respond → Dismounted when the car is at <= `exit_max_speed` and ((on foot && d <= `dismount_distance`) or
    (driving && the player's car <= `exit_max_speed` for `stopped_seconds` && d <= `dismount_distance`) or route done);
  - Dismounted → Respond iff crew aboard >= 1 and (no live `CrewOf` cop of this car outside, or `reboard_left <= 0`);
  - Dismounted with crew aboard 0 and no live `CrewOf` outside → `Abandoned`;
  - Leave, Taken, Abandoned terminal.
- `reboard_left` starts at `reboard_timeout_seconds` when the player drives farther than `reboard_distance` from the
  car; at 0 the stragglers lose `CrewOf` and stay ordinary foot units.
- Motion: Respond → route (A* in `car_route.rs`: state = lane id, successors = `lane.out` connectors' `to_lane`, cost
  = integer centimetres of connector + next lane length, heuristic = straight-line centimetres from the lane end to
  the goal; `pathfinding::astar`, at most `routes_per_tick` searches per tick for all cars, refresh by `age >=
  route_refresh_seconds`) to the lane `nearest()` to the target (`last_known`, or the player while seen); autopilot
  target = the path point `L = max(lookahead_min, lookahead_per_mps·v)` ahead along lane/connector polylines; speed
  `pursuit_speed` on lanes, `turn_speed` on connectors, `min(.., sqrt(2·comfortable_deceleration·distance_to_goal))`
  near the goal, and IDM on a forward cast (excluding the player's car in `Chase`). Chase → target = player position,
  speed `pursuit_speed` (rams). Dismounted / stopped → speed 0. Leave → wander lanes like traffic (random `next` by
  `PoliceCarRng`). Taken / Abandoned → no autopilot.
- Dismount (on entering Dismounted): each crew kind spawns `police_unit_bundle` at a `vehicle::exit_spots` spot
  (step 16), `insert(CrewOf { car })`, appearance from `PoliceCarRng`, cop state `Respond`; a crew member without a
  free spot stays aboard.
- Re-board (`board_police_cars`, `.in_set(AiSystems::Decide).after(police_fsm)`): a `CrewOf` cop within `enter_radius`
  of its car's door while the car is `Dismounted` and the player drives farther than `reboard_distance` → despawn (the
  Tnua sensor observer cleans up), push its kind to `crew`. A `CrewOf` whose car no longer exists or is Taken/Abandoned
  → remove `CrewOf`. In `police_fsm` (before the state match, ~20 lines): such a cop's motion is `Seek(door,
  chase_gait, direct)`, aim off, no trigger pull.
Check: gate 28.

**Step 13. `despawn_police_cars` (PoliceSystems chain, first).** Leave and Abandoned cars: traffic bubble rule (step 9
frame test, `offscreen_seconds`, > `off_view.despawn`); engaged cars (Respond/Chase/Dismounted): off frame >=
`despawn_offscreen_seconds` and beyond `population.despawn_distance` (like foot cops); Taken: never, becomes
Abandoned when the player leaves (`Vehicle.driver == None`). Cars are `CityScoped` (via `Vehicle`); `NEW_CITY` resets
`PoliceDispatcher` (existing `reset_dispatcher` covers `cars`).

**Step 14. Hijacking a police car** — `on_police_car_entered` (PoliceSystems chain, before `dispatch_police_cars`):
for a `PoliceCar` in Respond/Chase/Dismounted/Leave, the crew aboard dismounts at once (step 12 spawn, excluding the
spot the player stood on); state `Taken`; remove `Autopilot`, `DriveIntent`, `PoliceCarRoute`. Taken/Abandoned re-entry
→ Taken, nothing spawns. CarTheft is recorded by `record_crimes` (`first`).
PoliceSystems chain order: `(on_police_car_entered, despawn_police_cars, dispatch_police_cars, drive_police_cars,
despawn_police, dispatch_police).chain()`, with `dispatch_police_cars` and `dispatch_police` also `.in_set(PlayingSystems)`
and the car systems `.run_if(resource_exists::<TrafficGraph>)`.

**Step 15. Wanted: cars see and witness.** `wanted/search.rs track_search`: an extra query of active `PoliceCar`s
with crew aboard >= 1; `seen |= cop_sees(.., car_eye, .., cop_car_view_distance)`. `wanted/crimes.rs
record_crimes`: witnesses also from crewed active police cars (`witnesses(&spatial, car_eye, offender_eye, cfg)`).
Gate rows in 28.

### Phase D — binding notes and the Q-Б crime

**Step 16. `vehicle/seat.rs` exit spots.** Split `exit_spot` into:
`fn candidates(cfg, loco, position, rotation) -> [(Vec3, f32); 3]` (left door, right door, roof with expected feet
level, as today); `pub(crate) fn exit_spots(spatial, cfg, loco, car: (Entity, Vec3, Quat), exclude: &[Entity]) ->
Vec<(usize /*candidate index*/, Spot)>` (every candidate passing level + capsule + path, in order);
`exit_spot(.., forced)` = first of `exit_spots`; `forced` fallback = first candidate whose feet ray passes the LEVEL
rule only; last resort the roof point with feet at `top` (no ray). `Spot` becomes `pub(crate)`.
`pub(crate) fn pull_out(commands, spatial, cfg, loco, player, children, heads, car, vehicle) -> bool`: takes
`exit_spots` and uses the LEFT-door spot (index 0) only; if the left door is not clear it returns `false` and does
nothing (F7); else `leave` + `free_car`, returns `true`.
Check: `tests/vehicle_seat.rs` green + gate 31 rows.

**Step 17. Cabin (`vehicle/impact.rs apply_bullet_hits`).** For each `BulletHitVehicle` (after the existing car-health
loss): `local = rot⁻¹·(point − position) − cabin.centre`, inside iff `|local| <= half_extents` per axis. For a cabin hit:
write `CabinHit { shooter, attack, vehicle, point, damage }` (new message in `vehicle/mod.rs`, registered in
`VehiclePlugin`, cleared in `clear_vehicle_messages`); if the car has `driver: Some(d)` and `d` is alive: `wound =
(hit.damage × cabin_driver_share).round()`, `killed = health.take(wound)` (`Health::take(f32)`), write `DamageDealt {
shooter, shot: attack, target: d, point, damage: wound as u32, headshot: false, killed }`. System params: the car query becomes
`Query<(&mut VehicleHealth, &Vehicle, &Position, &Rotation)>`; new `Query<&mut Health, Without<Dead>>` (drivers),
`MessageWriter<CabinHit>`, `MessageWriter<DamageDealt>`, `Res<VehicleConfig>`.
Worked (pistol 25, share 0.5, falloff 1; re-derived): side window, body (1.2, 0.5, 0.0) → local (1.2, 0.05, −0.1)
inside → wound 12.5 → 13 (`f32::round` rounds half away from zero), armour 0 → health 100 → 87; hood (0, 0.3, −2.04)
→ z −2.14 outside ±1.0 → 0; low door (1.2, −0.3, 0) → y −0.75 outside ±0.5 → 0; roof (0, 0.92, 0.3) → local (0, 0.47,
0.2) inside → 13.

**Step 18. Pull-out (`police/arrest.rs`).** `ArrestAttempt` gets `pull: f32` (Reflect). New `pull_out_driver`
(`.after(AiSystems::Decide).before(arrest_player)`, `PlayingSystems`): player `With<Driving>`, alive, current row
`arrest` and `!hostile` (`PoliceAlert.hostile_left <= 0`); nearest cop in `CopState::Arrest` with
`flat_distance(cop, door_point(car)) <= arrest.distance` and car speed `<= exit_max_speed` → `pull += dt`, else
`pull = 0`. At `pull >= pull_out_seconds`: `vehicle::pull_out(..)`; if it returns true → `*attempt = ArrestAttempt {
cop: Some(cop), hold: 0.0, pull: 0.0 }`, else keep `pull` (retried next tick). The player then stands at the door
point, the cop within `arrest.distance`, so `arrest_player` holds → Busted after `arrest.seconds` (never BrokeFree:
`arrest_step` gives BrokeFree only beyond `break_free_distance` 3.0). `police_fsm` Arrest: when the player drives,
`at` = the car's door point (the cop walks to the door and stops at `stand_distance` from it). `reset_arrest` already
resets on Wasted/Busted/NEW_CITY.

**Step 18b. `wanted/crimes.rs record_crimes`** gets `MessageReader<DriverScared>`: for a message whose `shooter` is the
player, `let id = crimes.record(Crime::Shooting, shooter, None, attack, player_position, now, merge); touched.push(id)`
(witnessed by a cop in LOS in the same tick; else the fleeing driver's call resolves `Cause::Attack(attack)` to it).
Nothing new in `wanted.ron`.

**Step 19. Cars in the fire line (`tactics`).** In `tactics/fire_line.rs`:
- `pub(crate) struct CarRect { centre: Vec2, yaw: f32, half: Vec2 /*x, z*/ }` and
  `pub(crate) fn car_blocks(from: Vec3, to: Vec3, car: &CarRect, clearance: f32) -> bool`: the flat SEGMENT from→to
  passes within `clearance` of the rectangle (segment vs rectangle grown by `clearance`; no spread cone, nothing past
  the target).
- `Blocked` gets `cars: &'a [CarRect]`; `FireLine` exposes `clearance` to the module. `usable` rejects a spot whose
  segment to the target hits a car; `pinned` is false while any car blocks (cars never yield); `queue_slot` keeps
  using shields only (a slot "beside" a car centre would be inside it).
- `hold_fire(.., cars: &[CarRect], ..)`: the line is blocked if `line.blocked(chest, at.chest, &shields)` or any
  `car_blocks(chest, at.chest, car, line.clearance)`.
- Police (`behavior.rs`, Attack branch) and gangs (`gang/behavior.rs`) build the list once per system run: every
  `Vehicle` whose centre is within weapon reach + the car's half diagonal of any shooter, excluding the car the target
  drives (`Driving.vehicle` of the player; NPC targets never drive).

### Phase E — presentation and QA

**Step 20. Client.**
- `src/visuals/vehicle.rs`: `VehicleVisualAssets` holds three scene handles (sedan, police, taxi). `spawn_vehicle_model`
  gets `Query<(Has<PoliceCar>, Has<TrafficCar>, Option<&Appearance>)>` for `event.entity`: `PoliceCar` → police;
  `TrafficCar` with `a.0 as f32 / 4294967296.0 < taxi_share` → taxi; else sedan. `VehicleModel` stores its `scale`;
  `animate_wheels` divides the lift by the model's scale (today `render.vehicle.scale` for every car, R10). Wheel node
  names are the same four in all three models (verified). Kinematic cars' wheel states come from the sim.
- `src/audio/loops.rs update_sirens`: candidates = active `PoliceCar`s (Respond/Chase/Dismounted) within `audible`;
  if none, live foot cops as today. Siren child height `siren.car_height` on cars. New gate
  `sirens_ride_police_cars` in `src/audio/event_gate.rs`: a car + 2 cops at 2 stars → one siren parent is the car;
  car despawned → the sirens move to cops.
- `src/minimap/markers.rs`: `MarkerKind::PoliceCar` (police tint); police cars lose the grey `Vehicle` dot; traffic
  cars keep it.
- `tools/qa/scenarios/t14.py`: `cars(game)` keeps entities without `TrafficCar`/`PoliceCar` (parked cars).
  `t13.py`: a siren parent may be a live cop or an active `PoliceCar`. Rerun t8…t14.
- New client gates must follow the existing pattern for GLB-backed gates (CI does not fetch assets; check how
  `civilian_gate` handles that before adding a gate that loads `police.glb`/`taxi.glb`).
Check: `cargo test -p gta_like --bin gta_like -j 4` three times.

**Step 21-31: gates, section 3.** **Step 32: `t15.py`, section 3.3.**

**Step 33. Docs.** GDD §6.4 arrest paragraph, one line: "Исключение (T15): на 1 звезде коп вытаскивает игрока из
остановленной машины (≤ exit_max_speed) через левую дверь и арестовывает". GDD §5.3, one line: "высаженные копы
возвращаются в машину, если игрок уехал (Q-А)"; one line: "экипаж машины входит в лимит пеших юнитов строки,
диспетчер резервирует места недостающих машин". GDD §5.2, one line: "выстрел в салон машины трафика: водитель
тормозит, выходит и убегает (Q-Б); такси в потоке по броску внешности (Q-В)". `docs/architecture/traffic.md`: graph,
modes, TTC switch, reservation, bubble, bail-out, police car states, seat reservation, probe facts. After merge:
`docs/narrative-graph.md`, README, AGENTS.md "Проект" per the project rule.

### 2.1 Final checks (implementer)
`cargo build -j 4`; `cargo clippy -j 4 -- -D warnings`; `cargo clippy --workspace --all-targets -j 4 -- -D warnings`;
`cargo test -p gta_sim -j 4`; `cargo test -p citygen -j 4` (untouched, stays green);
`cargo test -p gta_like --bin gta_like -j 4` ×3; `python tools/qa/tree_check.py`;
`cargo tree -p gta_sim -e normal -i bevy_render` empty; `python tools/qa/scenarios/t15.py --out <dir>` and reruns of
t8…t14. Files < 750 lines (`traffic/drive.rs`, `police/cars.rs` are the likely splits). Before trusting a red from the
shared `target/`, `touch crates/*/src/lib.rs` and rebuild (TASK-009).

---

## 3. Test plan

### 3.0 Common fixture `crates/gta_sim/tests/traffic_support/mod.rs`
- `traffic_floor(lanes, connectors, sidewalk_runs: &[(Vec3, Vec3)]) -> App`: `headless_app()` + `settle` + `test_graph`
  built from `sidewalk_runs` (default: the stub `(30,0,−30)-(35,0,−30)`, far from the lanes) + the synthetic
  `TrafficGraph::new(..)` inserted as a resource. `NpcSystems` needs `SidewalkGraph`
  (`navigation/mod.rs:347-350`) and traffic sets need `TrafficGraph`: both are inserted (R1).
- Liveness (F2): `assert_traffic_ran(app, cars, ticks)` = `TrafficStats.casts` grew by >= cars × ticks / 2 over the
  window, else `GATE BROKEN: traffic systems did not run`. Every synthetic gate calls it after its first 64 ticks. (Not
  "a car moved": contact and hijack fixtures hold cars stopped by design.)
- No `CameraView` on synthetic floors unless the row says so: the bubble then never spawns or despawns; gates that
  expect a fixed car set assert the count stays constant (`GATE BROKEN` otherwise).
- Lanes in z ∈ [20, 38], x ∈ [−38, 38] (clear of the z 14 wall and the z 10 boxes of `world/test_area.rs`; the police
  station respawn point (−20, 0, 20) sits on that band, so gates that can reach Busted keep their lanes elsewhere);
  `GATE BROKEN` if a car's chassis collider intersects a `Block` at spawn (the floor top is y 0; chassis clearance 0.44).
- `spawn_traffic_car(app, seg, s, speed)` (production spawn path: the same helper step 9 uses), `cars(app)`,
  `obb_overlap(a, b)` (2D SAT on chassis rectangles from `Position`/`Rotation`, independent of the lane cursor),
  `set_traffic(app, |t| ..)`.

### 3.1 Traffic gates (headless)
- **21 `tests/traffic_idm.rs` — AC1 (correctness).** Loop of 4 lanes (x −34→34 at z 36, down to z 22, back, up) + 4
  corner connectors (one in / one out per node, no conflicts), 172 m. 10 kinematic cars at rest; gaps per seed from
  [1.0, 8.0] m with at least one gap of 1.0 m (< s0: IDM gives −2.19 at rest); 8 seeds × 6400 ticks. Every tick: every
  `TrafficCar.speed >= 0`, `LinearVelocity·tangent >= −1e-4`, no pair `obb_overlap`, |yaw − path tangent yaw| < 2°;
  car count stays 10. Liveness: each car travelled >= 172 m. Flips: drop the `v' < 0` branch of `ballistic_step` →
  negative speed at the 1.0 m gap (RED); leader search limited to the own segment → overlap after a corner (RED).
- **22 `tests/traffic_intersection.rs` — AC2 (correctness).** + intersection centred (−20, 0, 29), arms of 8 m (lanes
  x −36…−24 / −16…−4, z 21…37), 4 in / 4 out lanes, 12 connectors; the test re-injects a car at each approach start
  when its first 12 m are free; turns from `TrafficRng`; 6 seeds × 6400 ticks. Conflict points computed BY THE TEST:
  for every pair of connectors with different source and destination, sample both `pose` functions at 0.1 m and keep
  points where the centre lines cross (distance < 0.05). Every tick, for each point, count cars whose chassis
  rectangle (from `Position`/`Rotation`) contains it → <= 1. Liveness: each approach delivered >= 20 cars; every
  crossing point was covered at some tick. Flips: grant always (skip the conflict test) → RED; conflict table built
  without the geometric test → RED.
- **23 `tests/traffic_contact.rs` — AC4 (correctness).** Separate tests:
  (a) the player drives a car (T14 `drive_in`, throttle 1) into the rear of a kinematic car held at a stop line (a car
  on its destination lane / an occupant keeps `next` ungranted): the traffic car is `RigidBody::Dynamic` in a tick
  BEFORE the first `CollisionStart` of the pair; 16 ticks after contact its speed >= 1.5 m/s; the player car's
  forward velocity never reverses. Flip: disable the predictive switch, keep the backstop → RED.
  (b) the player on foot walks into the side of a stopped kinematic car → Dynamic before contact.
  (c) no switch in 640 ticks for: a sleeping parked car 0.85 m beside a passing kinematic car; an oncoming kinematic
  car in the opposite inner lane (3.25 m centre to centre) passing at full speed. Flip: replace the sweep by PLAN's
  closing-speed-along-the-centre-line rule → the oncoming row switches (RED).
  (d) a dummy/civilian walking across 1.2 m in front of a stopped car's bumper at 1.4 m/s → no switch. Flip: horizon
  test replaced by "inside `reach`" → RED.
  (e) a kinematic car at 12 m/s whose forward cast is disabled for the test (named mutation `sense_distance = 0.01`)
  approaching a `spawn_car` 20 m ahead in its lane that carries `Sleeping` when the run starts (`GATE BROKEN` if it
  never falls asleep) → Dynamic before the first `CollisionStart` (F4). Flip: skip `Sleeping` bodies → RED.
- **24 `tests/traffic_bubble.rs` — AC3 (correctness) + bands.** Seed-1 city (`city_app(1)`), civilians 0, gangs 0,
  player on the hospital sidewalk, `set_view(chase_view(..))` turning 90° every 3 s, 40 s. The test records per car
  per tick its position and computes its OWN frame test from the published `CameraView` (cone with the four roof
  corners + 90 m); when a car disappears: the last 128 ticks it was out of frame by the test's computation and its last
  distance > 25 m. Rows: `despawn_needs_two_seconds_off_frame`; `in_frame_car_is_kept` (a car pinned in the cone at
  60 m via the view never despawns in 640 ticks); `far_in_cone_car_goes` (a car in the cone at 100 m despawns after >=
  128 ticks); `spawn_bands` (every Steady spawn: in frame → [70, 90), out → [15, 25)). Liveness: >= 5 despawns and >= 5
  spawns of each band. Flips: drop the Offscreen condition → RED; swap the bands → `spawn_bands` RED.
  `abandoned_cars_are_despawned` (R4, on `traffic_floor` with a view): hijack a stopped car, drive 10 m, get out, move
  the player 40 m away and turn the view off the car → the car entity is gone after >= 128 ticks off frame; before
  that it exists (mode Abandoned). Flip: exclude Abandoned from despawn → RED.
- **25 `tests/traffic_hijack.rs` (correctness).** Synthetic straight lane; a dummy stands in the lane 10 m ahead of a
  traffic car (the forward cast stops it); the player 1.5 m from its door point; `request_vehicle` → `Driving` = that
  car, `RigidBody::Dynamic`, mode `Taken`, no `Autopilot`/own `DriveIntent`; exactly one new `Civilian` within 1 m of
  the player's pre-entry feet in `Flee { about: Some(Attack(entered.attack)) }`; `Crimes` holds a CarTheft incident
  with that attack. Then F out: mode `Abandoned`; the empty car stays within 0.2 m over 128 ticks. Re-entry row (F5):
  get back in → mode `Taken`, the civilian count does not change. Flips: keep the own `DriveIntent` → the exited car
  drives off (RED); spawn a driver on every `VehicleEntered` → the re-entry row RED.
- **25b `tests/traffic_bailout.rs` (correctness, Q-Б).** `traffic_floor` with a straight eastbound lane and a dead-end
  sidewalk run within 5 m of the expected stop point on the driver's (left, −Z for an eastbound car) side, east of
  x = 20 (TASK-026: stuck fleers never end their flight; `tests/wanted.rs delayed_calls_about_one_kill_count_once`
  precedent). A kinematic car at 12 m/s; the player (pistol, `arm`) shoots from 20 m to the side (beyond
  `shooting_radius` 15 m, no person near), aim recomputed each tick at the window point.
  (a) side window → mode `Bailing`; the car stops within `12²/(2·8) + 1 = 10 m` of the hit point; then exactly one
  `Civilian` within 1 m of the left or right door exit spot in `Flee { about: Some(Attack(shot)) }`; mode `Abandoned`,
  `RigidBody::Dynamic`; `Crimes` holds a `Shooting` incident with that attack; with the named mutation
  `CivilianConfig.call_after_flee = 1.0` (F8), wait up to `longest_call_ticks` for the call: a `PoliceCall` about
  `Cause::Attack(shot)` resolves to that incident and heat rises by `heat.shooting_near_people` (10).
  (b) hood hit → no Bailing, no civilian. (c) a second cabin hit while Bailing → still one civilian.
  Flips: cabin test removed → (b) RED; `DriverScared` not read by `record_crimes` → (a) crime/heat RED.
  Tick counts are derived by the implementer through the real call path before asserting.
- **26 `tests/config_traffic.rs` + `tests/traffic_graph.rs`.** Unknown field names the file and the field; one
  sabotage per `validate` rule strictly on the failing side (TASK-007), each with a distinct error text; cross-config:
  `switch.reach: 4.0` < 4.4 → error; `horizon_seconds: 0.02` < 2/64 → error; `in_view.despawn: 160.0` → error.
  Graph gate, seeds 1..=8 via `citygen::generate` + `from_layout` (no App): Ok; every lane has out connectors; no kept
  lane is a curb lane (offset check against `parking.curb_offset`); connectors > 0 at every intersection with >= 2
  edges; every connector polyline grown by the car half width (and the half length at the ends) stays off every city
  block polygon (a kinematic car does not collide with static geometry; a corner cut is a silent clip). Worked check
  done in this review for a street-street right turn (curb 3.25 m): the rectangle at t = 0.25 has its front-right corner
  at (3.51, 0.98) and at the midpoint the right side is 0.52 m from the block corner → expected GREEN; if RED, fix the
  curve, never the tolerance.
- **30c `tests/traffic_bench.rs` (liveness + order of magnitude, not a CI budget).** Seed-1 city, 40 civilians, gang
  cap, player driving on an avenue at 5 stars (12 units incl. crew, 5 cars chasing), view set, traffic 24: wait until
  `TrafficStats.cars >= 20` and 5 active police cars (`GATE BROKEN` otherwise; the seat reservation of step 11 makes
  the fifth car reachable), then 640 ticks: print mean/p50/p95/max, `TrafficStats` (incl. `switches_by_cause`),
  `VehicleLoad`, `RouteLoad`; assert mean < 10 × the implementer's measured probe mean (TASK-009: mean only). Record
  the per-tick mean against GDD §11 (physics + AI <= 4 ms) in the summary.
- **30d `tests/traffic_parked.rs` (correctness).** Seeds 1, 2, 3, player on an avenue sidewalk with parked cars, view
  set, traffic full (`GATE BROKEN` if `TrafficStats.cars < 12`), 4096 ticks: zero `CollisionStart` between a traffic
  car and a parked car; zero switches caused by a parked car (`switches_by_cause` + the test's own attribution); traffic
  OBBs never overlap parked OBBs. Flip: traffic on slot 1 → RED.

### 3.2 Police, binding-note and re-anchored gates (headless)
- **27 existing gates re-anchored (named, with reasons).**
  - `police_bench.rs`: pin `cars = 0` in every row (helper `no_police_cars(app)`); it measures foot SWAT. With
    `row.cars = 0` the seat reservation is 0, so the row still fills on foot.
  - `police_city.rs`: run twice: as is (cars deliver the cops; liveness "a cop reaches the player") and with
    `no_police_cars` (the sidewalk-routing evidence it was written for).
  - `new_city.rs`: literals of step 2 with non-zero `cars: 2` / `pull: 0.5`; the `GATE BROKEN` pre-checks gain
    `dispatcher.cars == 2`, `attempt.pull == 0.5`, `TrafficStats.cars > 0` (traffic existed in the first city),
    `TrafficGraph` present, and one `DriverScared` + one `CabinHit` message written; the A-rows gain `A9` with
    `cars == 0`, `ArrestAttempt::default()` (covers `pull`), no `TrafficGraph`, `TrafficIntersections` empty,
    `TrafficPhase::InitialFill`, `TrafficStats` default, both new message buffers empty, "no `TrafficCar` or `PoliceCar`
    entity survives".
  - `vehicle_hits.rs driver_is_not_hit_over_the_roof` (F9, would go RED): the roof shots now land in the cabin and
    wound the driver by design. Keep its purpose (the head sensor is off while driving): assert no `DamageDealt` on the
    driver has `headshot: true`, every one has `damage == round(pistol.damage × falloff × cabin_driver_share)`, the
    health drop equals their sum, and the 4 car hits and the car health loss stay as today. Run its old flip (head
    sensor enabled while driving → a headshot on the driver → RED).
  - `vehicle_hits.rs a_cop_sees_the_driver_and_shoots_the_car`: expected green (3 pistol wounds of 13 in 128 ticks do
    not kill); if a wound makes it flaky, give the player armour like `cop_across` does, with a one-line reason.
  - `config_police.rs every_star_has_a_row`: string of step 2.
  - `sensor_leak.rs`: the all-entities diff runs with traffic, a hijack and a bail-out in the window.
  - Other city gates with a view (`civilian_bench`, `civilian_city`, `gang_city`, `street_spawn`, `witness_city`) now
    run with traffic (and `sight_blocked` sees cars): record the first failure and the measured numbers; if the gate's
    subject is not traffic, add `set_traffic(|t| t.bubble.max_cars = 0)` to its fixture with a one-line reason, and
    report the with-traffic numbers in the summary (the owner sees the effect, e.g. on the witness rate). Never loosen
    a threshold. `vehicle_city.rs` sets no view: no traffic there.
- **28 `tests/police_cars.rs` — AC5 (correctness, one test per row) + behaviour.** Seed-1 city, player on a sidewalk,
  `set_view(chase_view(..))`, `set_player_armor(1e6)`, heat = row threshold then one tick (TASK-012), 1920 ticks;
  every tick `wanted.stars == row` (`GATE BROKEN` otherwise).
  `cars_follow_row_1..5`: every tick active police cars <= `row.cars`, `dispatcher.units <= row.units`, foot + aboard
  == `dispatcher.units`; liveness: `row.cars` reached. Row 2 is the AC. Flip: count only foot units in the car
  dispatcher → row 2 overruns (RED). Flip of the seat reservation: remove it → row 5 never reaches 5 cars (RED).
  `car_closes_in_and_dismounts` (player on foot): the nearest police car's distance at t = 20 s < at spawn; a
  Dismounted car within `dismount_distance` + lane offset; its crew are `PoliceUnit`s with `CrewOf`.
  `car_chases_a_driver` (player drives a straight avenue at 10 m/s via `set_drive`): distance shrinks; within 40 m with
  LOS the car is `Chase` and its `Autopilot.target` equals the player position.
  `crew_reboards`: after a dismount the player drives 60 m away → the crew is aboard again and the car is `Respond`.
  `dead_crew_frees_the_slot`: kill both dismounted cops → the car turns `Abandoned` and a new car is dispatched within
  the row. Flip: PLAN's "Dismounted → Respond when no CrewOf outside" → RED.
  `next_car_state_table`: unit table, one case per row of step 12 (incl. Abandoned and the re-board timeout).
  `car_sees_at_50_m` / `crewed_car_witnesses_theft` (wanted rows via `track_search` / `record_crimes`).
- **29 `tests/police_pull_out.rs` (correctness).** 1 star (`raise_heat`), player in a stopped car on the floor, a cop
  set to `Arrest` 1.2 m from the door point → after `pull_out_seconds` (64 ticks) the player has no `Driving`, stands
  at the left door spot (feet within 0.05 of ground level), and after 96 more passive ticks `GameState::Busted`. Rows:
  car at 5 m/s → never pulled in 640 ticks; 2 stars (no arrest row) → never; cop 2.0 m from the door → never; left door
  blocked by a wall 0.1 m off it (F7) → never pulled, and never `BrokeFree` (heat unchanged). Flips: drop the speed
  check → row 1 RED; pull through `exit_spot(forced)` → the blocked-door row RED (roof/right exit, BrokeFree).
- **30 `tests/vehicle_hits.rs` additions — cabin (correctness).** Through a real pistol ray (a dummy shooter with a
  loadout, as `driver_is_not_hit_over_the_roof` does) at a car the player drives: side window → health −13; hood → 0;
  low door → 0; roof → −13; an SMG burst into the cabin kills the driver → `Wasted`, and `eject_all` puts him at ground
  level. Flips: share 0 → rows 1/4 RED; zone check removed → rows 2/3 RED.
- **30b `tests/police_fire_lines.rs` additions — O1 (correctness + liveness).** (a) cop in Attack at 2 stars, player 20 m
  away, a parked car whose corner is 0.4 m off the line at 10 m: no `BulletHitVehicle` on that car in 1920 ticks and >=
  1 `DamageDealt` cop → player (it repositions); mirrored and at 3 distances (6 cases). (b) the player drives car X,
  same geometry through X: the cop fires and hits X (target car excluded). (c) gang gunman version of (a) through
  `gang/behavior.rs`. (d) a car 1.0 m beside the segment at 10 m → the cop fires without repositioning; check from the
  shipped numbers that `clearance (0.5) < 1.0 < clearance + 10·tan(cone)` holds, else pick the offset inside that
  band (`GATE BROKEN` if the band is empty). Flip: add the spread cone to the car test → RED. (e) a car 3 m behind the
  player on the line → the cop fires. Flip: test cars up to `reach` → RED. Flips of (a)/(b): empty car list; exclude
  nothing.
- **31 `tests/vehicle_seat.rs` additions — forced eject (correctness).** `forced_eject_never_on_a_wall_top`: a car with
  a 1 m wall 0.1 m off the left door, a 4 m wall off the right door, a slab 2.5 m above the roof; Busted → the player's
  feet at the car top ± 0.05 (roof), never at 1 m + ground. On today's code → wall top (RED; the flip is the old
  fallback). `forced_eject_takes_a_level_door_despite_a_body`: a dummy at the right door, a low wall left → right door
  at ground level.
- **Presentation (client, ×3 runs):** `sirens_ride_police_cars` (step 20); existing `sirens_ride_live_cops` stays green.

### 3.3 Runtime QA — `tools/qa/scenarios/t15.py` (via `tools/qa/brp.py`)
Seed 1, `--features dev`, release. 1) wait `Playing`, then until >= 12 `TrafficCar`s; read their speeds (mean > 3 m/s,
max <= 16.5), screenshot the street. 2) Hijack: the nearest kinematic traffic car; put the player on its lane 12 m
ahead (it stops: speed <= 0.5), then 1.5 m from its door point, `send_keys F` → `Driving` = that car within 1 s; a
`Civilian` in `Flee` within 3 m; screenshot. 3) `mutate_resources WantedLevel.heat = 180`; hold W on a straight
avenue in bursts; every 0.5 s read the flat distance from each `PoliceCar` to the player: at most 2 active police cars
ever; the nearest distance decreases (first value vs the minimum over 25 s); screenshots every 1 s (chase). 4) stop,
F out: within 20 s a Dismounted car and cops with `CrewOf` near. 5) `get_diagnostics` and `Game.frame_report()` (FPS
only there). 6) shutdown; no `ERROR_WORDS` in the log. Enum names derived from source, never hard-coded lists. The
taxi/sedan mix is not asserted.
Owner checklist (QA_REPORT.md): streets have moving cars that queue and yield at crossings; a car jacked from the
flow, its driver runs; a shot driver bails out and runs; taxis in the flow; survived a car chase (police cars follow,
ram, cops get out and re-board); sirens on cars; police/taxi model look and scale.

---

## 4. Rollout notes
- No migrations, no save data, no new crate: bevy 0.19.1, avian3d 0.7.0, pathfinding 4.16.0, rand_chacha 0.10.0 are in
  `Cargo.lock`; no lock change. Avian APIs used were re-checked in `avian3d-0.7.0/src`:
  `SpatialQuery::cast_shape_predicate` (`spatial_query/system_param.rs:523`), `shape_intersections` (`:1173`),
  `RigidBody::is_dynamic/is_kinematic` (`dynamics/rigid_body/mod.rs:308/318`), `RigidBodyDisabled` (`:380`), `Sleeping`,
  `SleepingDisabled` (`sleeping.rs:61/70`), `CollisionStart` (`collision/collision_events.rs:171`).
- Assets: `police.glb`, `taxi.glb` added to the manifest; `python tools/fetch_assets.py` needed locally (only the
  manifest is in git).
- Data: new `assets/traffic/traffic.ron`; appended fields in `escalation.ron`, `wanted.ron`, `sedan.ron`,
  `damage.ron`, `render.ron`, `mix.ron`. All strict loaders: a stale local copy fails loudly, by design.
- Behaviour changes to existing play: cops now mostly arrive by car (crew within the row, seats reserved); a seated
  driver takes cabin wounds; cops pull a stopped driver out at 1 star; traffic cars block sight lines (existing
  `sight_blocked` includes the Vehicle layer).
- Reflect/BRP: `TrafficCar`, `TrafficMode`, `TrafficStats`, `PoliceCar`, `PoliceCarState`, `CrewOf`, `Autopilot`,
  `DriverScared`, `CabinHit`, the new `PoliceDispatcher.cars` and `ArrestAttempt.pull` are registered (`t15.py` reads
  them). `t11.py` checks only `dispatcher.units <= row`: fine.
- No feature flag; traffic is off on the test area (no `TrafficGraph`) and on any app without a `CameraView`.

---

## 5. Review notes (changes from PLAN_V2 and why)

**Disconfirmation (done first).** Counter-example tested: "PR1's fixed traffic fixture is still vacuous or wrong for
some gates: its liveness 'no car moved in 64 ticks → GATE BROKEN' plus the far sidewalk stub cannot host the
stopped-car and fleeing-driver gates". Checked: `navigation/mod.rs:347-350` (NpcSystems needs `SidewalkGraph`),
`flow/mod.rs:107-114` (state), `police/mod.rs:434-459` (`dispatch_police` in `PlayingSystems`),
`civilian/mod.rs:355-362` (`call_later` rolls `call_after_flee` from `NpcRng`, temperament unused),
`tests/wanted.rs:126-140` (dead-end runs east of x = 20, `call_after_flee = 1.0`). **Held in part:** the systems now
run (PR1's graph insertion is right), but the liveness check would fire `GATE BROKEN` on gates 23 and 25, whose cars
stand still by design, and the far stub cannot host the bail-out call. Fixed by F2 and F8. Gate by gate I checked what
each fixture needs to run its systems: `SidewalkGraph`, `TrafficGraph`, `CameraView`, `Playing`, star row +
`last_known`. The needs are written into each gate above.

Findings (evidence, then change):
- **F1 Bundle duplicates panic.** `vehicle_bundle` (`vehicle/mod.rs:135-159`) already holds `RigidBody::Dynamic` and
  `Name`; PLAN spawns them again in one bundle; bevy_ecs 0.19.1 `bundle/info.rs:118` panics "Bundle … has duplicate
  components". The client observer `On<Add, Vehicle>` needs `TrafficCar`/`PoliceCar`/`Appearance` in the SAME spawn
  bundle. → steps 9 and 11 spell out spawn-tuple + follow-up insert.
- **F2 Fixture liveness.** PR1's "a car moved" check contradicts gates 23a and 25. → `TrafficStats.casts` growth.
- **F3 Wrecked traffic car.** GDD §5.2: at health 0 the car stalls; a kinematic car ignores `drive_vehicles` and would
  keep driving. → wreck rule, `Bailing { attack: Option<u32> }`.
- **F4 TTC skipped sleeping bodies.** The sweep uses relative displacement, so a parked car beside a lane never
  overlaps (0.85 m > skin), but a sleeping car ahead in the lane was never switched for → infinite-mass shove until
  the backstop. → filter dropped, row 23e added.
- **F5 Re-entry spawned a second driver.** `enter_exit` (`seat.rs:181-222`) lets the player re-enter any driverless
  car; V2's hijack reacts to every `VehicleEntered` on a `TrafficCar`. → only modes with a data driver spawn one; row in
  gate 25. Same for police cars (step 14).
- **F6 Dispatcher race.** Both dispatchers spawn 1 per tick; with crew counted in `row.units`, row 5 settles at 4
  cars + 4 foot and never reaches 5 cars (V2's gate 28 liveness and bench 30c wait would fail). → seat reservation in
  `dispatch_police`, gated on `TrafficGraph` so test-area gates stay unchanged; flip row in 28.
- **F7 Pull-out through the far side broke free.** `arrest_step` (`police/fsm.rs:126-152`) returns `BrokeFree` beyond
  `break_free_distance` 3.0; PLAN's `exit_spot(forced)` could exit right door (≈ 4.4 m from the cop) or roof → +1 star
  for being arrested. → pull only through a clear left door; blocked-door row in 29.
- **F8 Call determinism.** PR1's "report weight forced high" does not touch `call_later` (it rolls `call_after_flee`).
  → named mutation `call_after_flee = 1.0` + a dead-end run near the stop point.
- **F9 Existing gate goes RED.** `vehicle_hits.rs driver_is_not_hit_over_the_roof` asserts the driver's health is
  unchanged while 4 pistol pellets hit the roof over the seat, and the roof is now cabin (worked row: roof → 13). V2
  did not list it. → re-anchor in step 27 with its purpose kept.
- **F10 `new_city` literals with zeros test no reset.** V2 wrote `cars: 0`, `pull: 0.0`. → non-zero values +
  GATE BROKEN pre-checks and A-rows for all new state.
- **F11 queue_slot with cars.** V2 said `queue_slot` tests `shields ∪ cars`; its slot is placed beside the blocker
  centre at `radius + clearance` (0.8 m), which is inside a car (half width 1.2). → `queue_slot` stays shields-only;
  `usable` and `pinned` see cars.
- **F12 Render offset formula sign.** V2 wrote `offset.z = −scale·(…)` but its numbers follow `scale·(node z + mesh
  centre z)`. Re-measured all three GLBs: numbers kept, formula fixed; `offset.y = −1.16` explained as wheels on the
  road.
- **F13 Types.** `Health::take` takes `f32` (`character/health.rs:103`); `DamageDealt.damage` is `u32`. Step 17 says
  which is which. `roll_temperament` generalised to `&mut ChaCha8Rng` via `unit_f32` (same draws), not a closure.
- **F14 Opposite-lane row.** Slot-0 lanes have no same-direction neighbour; the real neighbour is the oncoming inner
  lane 0.85 m away, which PLAN's closing-speed rule would have switched. Row 23c now uses it, with a flip.
- Smaller: `dispatch_police_cars` goes in `PlayingSystems` like `dispatch_police`; the police chain order is written
  out (step 14); dangling `CrewOf` (car despawned or taken) is removed; `abandon()` removes `SleepingDisabled` so
  abandoned cars can sleep; `vehicle_city.rs` has no `CameraView`, so no traffic there (PLAN listed it); traffic gates
  keep clear of the station respawn point (−20, 0, 20).

Kept from V2 after re-checking: R1 (SidewalkGraph gating), R2 (line numbers), R3 (`every_star_has_a_row` literal),
R4 (Abandoned lifecycle), R5 (police car states), R6 (time-to-contact), R7 (segment-only car test), R8 (DriverScared
crime path: `record_crimes` records `Shooting` only with a person within 15 m of the muzzle, `crimes.rs`), R9 (braking
bail-out), R10 (per-model scale), R11 (`TrafficGraph` run condition), R12 (`cop_sees` distance, roof corners). The
IDM, ballistic, Bézier, slot, cabin and autopilot worked rows were recomputed here and hold. The schedule edges were
re-verified against `perception/mod.rs:173-179`, `police/mod.rs:434-459`, `vehicle/mod.rs:206-226`,
`wanted/mod.rs:209-214`: no cycle.

Open risks (owner-run or measured, no machinery): dynamic traffic quality on connectors; jams behind abandoned cars
(no lane change); police cars at intersections do not reserve (they rely on IDM + forward casts, possible stand-off);
a cop approaching the door from the far side walks into the car body; siren height and models; with the seat
reservation all 1-4★ cops arrive by car, so a city spot with no hidden lane point within 60-120 m gets no cops until
the player moves (the gate 28 liveness measures seed 1); no crew wound from cabin shots on police cars (data crew).

children: 0 launched / 0 reported.
