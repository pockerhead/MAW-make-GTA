# PLAN FINAL — TASK-032: the street flows around the player's mess; sirens part traffic

Reviewer: plan-reviewer-2. Inputs: `TASK_FINAL.md` (spec, premise resolution, resolved questions), `PLAN.md`
(planner, detail source), `PLAN_V2.md` (plan-reviewer-1, corrections win over PLAN.md). Every code fact below
was re-checked on `feature/oncoming-lane` @ 1bbb1b1. Where this plan differs from V2 or PLAN.md, §5 says why.

Cost of error: silent (kinematic pass-through, minutes-long gridlock, frame budget) → full evidence layer. The
look of the moves (glide vs snap, rotation of a recovered car, pull-over, pass, chase aggression) is judged in
the owner run, not by gates.

---

## 1. Summary

A new `occupancy` domain builds one `RoadOccupancy` snapshot per fixed tick: every vehicle and every enabled
character near the player as a flat footprint with velocity, standing time, siren flag and derived pass claims.
Traffic sensing (two strips instead of the avian slab cast that skipped kinematic cars), lane-start checks,
junction grants, the traffic spawner and the police lane choice all query it through one API
(`first_along`, `blocked`). On top of it: a kinematic lateral offset per traffic car (one rate-limited law for
rejoin, pass and yield); recovery of bumped `Dynamic` traffic cars back to `Kinematic` with data hysteresis;
go-around of standing vehicles and characters (curb lane on avenues, opposite inner lane on streets, committed
only when the whole claim is clear, the claim visible to everyone as an occupancy footprint); junction-box rules
(grants check the connector path, stale holders past the line are demoted, blocked approaches re-pick, a car on
a connector passes a body in the box); sirens (`Respond`/`Chase`) make traffic ahead yield to the curb, let the
police car choose the oncoming lane, and the dispatcher spawns cars ahead of and beside a fleeing driver by data
sector shares. Work is staged: causal repros and baselines first, each stage ends green, with the stop rule and
the named fallback (footprint-only passers plus the out-of-view stuck-despawn cheat).

---

## 2. Implementation steps

### 2.0 Verified code facts the steps rely on

- `traffic/drive.rs` (514 lines) `advance_traffic`: AI snaps (`is_ai`: Kinematic, Dynamic, Bailing) sorted by
  entity bits (`:219-243`); Dynamic cars re-projected, `reproject` returns lost for > 4 m, > 60°, upside down
  (`:128-161`); path `Occupancy`/`leader()` (`:40-119`) holds every non-abandoned AI car by `s`, Dynamic ones
  included; `lane_start_free` (`:266-280`) is an avian box over the first `need` m of the destination lane that
  excludes ALL AI snaps; sensing (`:296-379`) casts a 0.1 m slab from the nose along the PATH tangent with
  `e != me && !kinematic.contains(&e)` (`:371`): every kinematic AI car is invisible, wherever it is; kinematic
  motion `velocity = (pose(s') − position)/dt`, yaw snapped in one tick (`:414-461`); nothing switches a car
  back to Kinematic; bailing driver exit (`:464-503`) skips a car with no exit spot, forever.
- `traffic/contact.rs`: `switch_to_dynamic` in `FixedPostUpdate` before `PhysicsSystems::First`; skips
  `RigidBodyDisabled` bodies, uses zero velocity for `Sleeping` ones; `FlatRect::of`, `swept_rect_hits_rect`,
  `swept_circle_hits_rect` are public (`traffic/mod.rs:19`).
- `traffic/junction.rs`: FCFS grants; a waiter that fails `room`/`lane_start_free` is NOT pushed to `blocking`
  (`:175-181`), a conflict failure is (`:171-173`); the lease lapses only for a holder before its stop line
  (`:108-114`); a holder on its connector keeps its grant forever (`:115`); `waiting` is cleared on grant
  (`:191-195`); the `next` connector is drawn from `TrafficRng` at request time (`:135-138`).
- `traffic/spawn.rs`: candidates are drawn by `rng.next_u32()` (`:198`) and a rejected avian chassis check
  (`:208-214`) consumes a `check`, so any change in acceptance moves the `TrafficRng` stream.
- `traffic/graph.rs`: only slot-0 lanes (`from_layout` `:287`), Alleys skipped (`:293`); `TrafficLane` has one
  struct literal (`:202`); `polyline_distance` private (`:87`).
- Schedule: `TrafficSystems::{Hijack, Bail, Drive, Bubble}` in `NpcSystems`, `run_if(resource_exists::
  <TrafficGraph>)` (`traffic/mod.rs:223-238`); `PoliceSystems` after `PopulationSystems` and `WantedSystems`
  (`police/mod.rs:590-596`); `VehicleSystems::Drive` after `PoliceSystems` (`vehicle/mod.rs:239-243`).
  `TrafficSystems::Drive` and `PoliceSystems` are unordered. Nothing outside `traffic/` references
  `TrafficSystems`, so `PoliceSystems.after(TrafficSystems::Drive)` creates no cycle. Physics runs in
  `FixedPostUpdate`: `Position` is constant through `FixedUpdate`.
- Seated characters carry `RigidBodyDisabled` + `ColliderDisabled` (`vehicle/seat.rs:278-283`).
- Police (`police/car_route.rs` 659 lines): Respond follows an A* lane route, `pull_over` only below
  `turn_speed`; Chase targets the player directly (`:543`) with the player's car ignored in the IDM cast; the
  IDM obstacle is an avian slab cast along the car HEADING from the car centre (`:572-600`). `car.blocked`
  grows only in `Respond` at `|v| <= exit_max_speed` (`:355`). A blocked car dismounts only when the player is
  on foot or has stood `stopped_seconds` (`cars.rs:167-171`, `may_stop`): with the player driving, a blocked
  police car stands, it does not dismount.
- Dispatch (`car_dispatch.rs`): candidates are spawn points of lanes heading toward `last_known`
  (`:139-148`); `pick_spawn` (`fsm.rs:87`) draws no RNG; `approach_clear`/`lane_costs_to`/`step_cost` are used
  only by the pursuit filter (`:172-175`) and the `approach_clear_rows` unit test.
- Data: `traffic.ron` `reservation_timeout: 5.0`, `hold_speed` 0.5 (vehicle), `sense_distance 25`,
  `turn_sense_distance 6`, `switch: (reach 5, horizon 0.1, skin 0.1)`, `lost: (4.0, 60)`; `escalation.ron`
  `car: (pursuit_speed 20, dismount_distance 20, direct_chase_distance 40, blocked_seconds 2, pull_over 3.25,
  spawn_ring (30, 60))`. Sedan chassis half extents (1.2, 0.92, 2.04); its hull projects exactly onto the
  2.4 × 4.08 rectangle (top points at ±h.z, `vehicle/config.rs:170-188`).
- Road geometry (`city.ron`): lane width `w` = 3.25. Street: inner lane centres at ±1.625 m, curb at ±3.25,
  raised sidewalk beyond, in-lane slack (w/2 − half.x) = 0.425 m. Avenue: inner ±1.625 (in graph), curb lanes
  ±4.875 (parked cars, not in graph), no median.
- Existing floors with an antiparallel neighbour 3.25 m to the left (so `left_gap` will be `Some(3.25)`):
  `traffic_intersection.rs::plus()` and `police_car_floor.rs::plus()` (in lane vs out lane of one arm). All
  other floors (`traffic_pedestrian`, `traffic_contact`, `traffic_hijack`, `traffic_bubble`, `sensor_leak`,
  `loop_lanes`, `square_loop`) have none.
- Engine APIs used, all confirmed in the pinned sources (`Cargo.lock`: bevy 0.19.1, avian3d 0.7.0) and already
  used in these files: `SpatialQuery::shape_intersections`, `try_insert`/`try_remove`, `RigidBody::Kinematic`,
  `SleepingDisabled`, `Has<ColliderDisabled>`/`Has<RigidBodyDisabled>`, `MessageReader<CollisionStart>`,
  `#[require(..)]` (pattern: `TrafficCar #[require(Offscreen)]`), `SolverConfig.max_overlap_solve_speed`
  (avian3d-0.7.0 `dynamics/solver/plugin.rs:250`, default 4.0 at `:296`, "maximum speed at which overlapping
  bodies are pushed apart", scaled by `PhysicsLengthUnit`, which this project leaves at 1). No new crates.

### 2.1 Common rules for every stage

- Every stage ends green: `cargo test -p gta_sim -p citygen`, `cargo clippy --workspace --all-targets -- -D
  warnings`, `cargo test -p gta_like --bin gta_like`, `python tools/qa/tree_check.py`. Before trusting RED from
  the shared `target/`, `touch crates/*/src/lib.rs` and rebuild (gates lesson, TASK-009).
- Stop rule: the second failure of one class (same gate, same mechanism) → write `REDESIGN_NOTE.md` in the task
  dir (fantasy, missing shared rule, cheat, rescope) for the orchestrator. No third patch.
- Every new tuning value is data with a strict loader (`deny_unknown_fields`), a `validate` rule, a "why"
  comment in the `.ron`, and a failing-side config-test row whose sabotage value sits strictly off the boundary.
- Every new gate is flip-RED: break the mechanism, observe RED, restore, observe GREEN, record the perturbed
  input in the stage summary. Every new or touched gate over one deterministic walk reports its spread; a
  touched city gate runs at least twice.
- New code goes into new files: `occupancy/{mod,query}.rs`,
  `traffic/{lateral,lanes,recover,pass,box_rules,sirens}.rs`, `police/{siren,spawn_sector}.rs`. Existing files
  get call sites only. Limits: `drive.rs` < 750 (target ≈ 600), `car_route.rs` shrinks, `police/mod.rs` +≤ 8,
  `cars.rs` +≤ 2, `graph.rs` +< 10, `junction.rs` < 400.
- A new `Res`/`ResMut<RoadOccupancy>` on `advance_traffic`, `spawn_traffic`, `drive_police_cars`,
  `dispatch_police_cars`: all harnesses go through `composed_app`/`headless_app`; before each merge grep
  `crates/gta_sim/tests` and `src/` for hand-registered copies of these systems (none today).

### Stage 1 — causal evidence, baselines, tooling (no production code)

1.1 `crates/gta_sim/tests/traffic_support/mod.rs` — add:
   - `pub struct Footprints` (the G1 oracle): its OWN flat SAT (4-axis projection of two oriented rectangles
     written in this file; it must not call `swept_rect_hits_rect`, `FlatRect` methods beyond construction, or any
     occupancy code). `record(&mut self, app: &mut App, tick: u32)` takes every entity with `Vehicle` +
     `Position` + `Rotation` + `RigidBody` within 150 m of the player (all when there is no player), builds
     rectangles from `VehicleConfig::half_extents()` (x, z), and for every pair with at least one
     `RigidBody::Kinematic` computes the penetration depth (minimum axis overlap). Tolerances: kinematic ×
     kinematic 0.02 m (1 cm per side, the `obb_overlap` precedent; kinematic pairs get no solver response);
     kinematic × dynamic `SolverConfig.max_overlap_solve_speed · dt` read from the resources (4.0 / 64 = 0.0625 m:
     the overlap the solver removes in one step at its maximum push-out speed; a deeper overlap means a body moved
     into another faster than contact resolution allows). A violation stores (tick, pair, kinds, depth, positions).
     `assert_clean(&self, label: &str)` panics with ALL violations. `max_depth()` returns the deepest seen.
   - `pub struct StandClock`: per entity, current and longest continuous stand below `hold_speed` (0.5 m/s),
     tagged with the `TrafficMode` at the time; Dynamic stands are kept in a separate max. `record(app)`,
     `worst()`, `worst_dynamic()`, `longer_than(s) -> Vec<(Entity, f32, Vec3)>`.
   - `pub fn two_way_street(length: f32) -> (Vec<LaneSpec>, Vec<(u32,u32,u32)>)`: a straight two-lane street
     (lanes 3.25 m apart, antiparallel, v0 12) closed at both ends by U connectors, laid inside the test floor
     clear of every `world/test_area.rs` block (check each point; `GATE BROKEN` if a lane rect meets the test area).
   - `pub fn avenue_floor(...)`: the same with a parked (sleeping, dynamic, `Vehicle`) car row option in a curb
     lane 3.25 m right of each traffic lane. `curb_lane` is data from the city layout, so the floor helper sets it
     on the built graph through a test-only setter `TrafficGraph::set_curb_lane(lane, bool)` (`#[doc(hidden)] pub`),
     named in the helper doc.
1.2 Baseline G1 on current code: an `#[ignore]`d test `tests/traffic_g1_baseline.rs` (or a `scratch/` harness)
   runs the oracle every tick on the `traffic_gridlock` pose, seeds 1/2/7/42, 120 s, and prints violation count,
   classes (kin×kin on connectors, kin×dyn), max depth and the connector ids of kinematic pairs. If any violation
   exists, report to the orchestrator before stage 2 with the pairs. The tolerance is never widened to hide them.
1.3 New `crates/gta_sim/tests/traffic_causes.rs`: production city (`city_app(seed)`), stationary player on the
   sidewalk OFF the carriageway, facing the scene (`set_view(chase_view(feet, dir))`), `StandClock`, all
   violations collected before panicking. Rows (each gets `#[ignore = "TASK-032 stage N"]` once its RED is
   recorded, with the RED numbers in the stage summary):
   - (a) abandoned player car (`vehicle_support::spawn_car`) mid-lane on a street lane ≥ 60 m before its stop line,
     seeds 1 and 7, 120 s. RED = an AI car behind it stands > 30 s.
   - (b1) rear nudge at 2 m/s by a kicked player car on a free lane; (b2) the same car shoved 0.6 m (street) /
     1.0 m (avenue) toward the curb with a dummy on the sidewalk 10 m ahead (street) or a parked car ahead
     (avenue); (b3) a dummy pressed at its bumper; (b4) yawed 45° within `lost`. RED = still `Dynamic` and
     standing after 60 s.
   - (c) a character (`spawn_dummy`) standing in a street lane. RED = the car behind stands > 30 s.
   - (d) a non-AI car left on a connector path with an AI car already on that connector behind it. RED = that AI
     car stands > 40 s.
   A cause that does not reproduce in any variant is dropped from scope and reported (premise amendment 1). If
   all of (b) is green on current code, stage 3 shrinks to the rows that went RED; G3 rows without a RED cause
   are reported, not built. Mine positions from `maw/tasks/done/TASK-031/scratch/sessions/` and the task's
   `scratch/mine_stands.txt`.
1.4 `tools/qa/scenarios/t15.py`: `--seed` argument (int, default 1) replaces the `SEED` global; the seed is
   written into `summary.json`. Script-only.
1.5 R1 script: `maw/tasks/in_progress/TASK-032/scratch/tools/repro_abandoned_car.py`, a copy of the TASK-031
   script with `sys.path` pointing at `maw/tasks/done/TASK-031/scratch/tools`; the player walks to one fixed
   sidewalk point OFF the carriageway (same point in the leave and the control run, within 45 m of the junction,
   in view); per-car stand durations logged (max per entity, as `scratch/mine_stands.py` does); civilians within
   8 m of the left car reported.
1.6 Baselines on current code, outputs under `maw/tasks/in_progress/TASK-032/scratch/baseline/`: R1 leave=1 and
   leave=0 (seed 1); t15 seeds 1/2/3 one run each (pressure count); `traffic_bench` and `police_bench` means
   (3 runs each, spread reported).
Check: `cargo test -p gta_sim --test traffic_causes -- --ignored` shows each reproduced row RED; the normal suite
is green; baselines recorded.

### Stage 2 — shared occupancy, lateral state, migrated sensing / lane start / spawner (G1, G8)

2.1 New `crates/gta_sim/src/occupancy/mod.rs`:
```rust
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BodyKind { OnPathTraffic, Traffic, Vehicle, Character }
#[derive(Clone, Copy, Debug)]
pub enum Footprint { Rect(FlatRect), Circle { centre: Vec2, radius: f32 } }
#[derive(Clone, Copy, Debug)]
pub struct RoadBody {
    pub entity: Entity, pub kind: BodyKind, pub shape: Footprint,
    pub velocity: Vec2, pub dynamic: bool,
    /// Seconds at or below `hold_speed`.
    pub standing: f32,
    pub siren: bool,
}
pub struct Claim { pub owner: Entity, pub rect: FlatRect, pub dir: Vec2 }
#[derive(Resource, Default)]
pub struct RoadOccupancy { bodies: Vec<RoadBody>, index: HashMap<Entity, usize>,
                           claims: Vec<Claim>, standing: HashMap<Entity, f32> }
#[derive(SystemSet, Debug, Clone, PartialEq, Eq, Hash)]
pub struct OccupancySystems;
pub struct OccupancyPlugin;
```
   - Kinds: `OnPathTraffic` = a `TrafficCar` with `RigidBody::Kinematic`, mode Kinematic or Bailing,
     `lateral == 0.0` and `manoeuvre == Manoeuvre::None` (exactly the cars the path `Occupancy`/`leader()`
     covers). `Traffic` = every other AI car (off-path kinematic, Dynamic, Bailing-dynamic). `Vehicle` = every
     other `Vehicle` (Abandoned/Taken traffic, parked, the player's car, police). `Character` = `Character`
     entity, circle of `LocomotionConfig.capsule_radius` at `Position`.
   - `snapshot_road` (in `OccupancySystems`) reads `Query<(Entity, &Position, &Rotation, &LinearVelocity,
     &RigidBody, Option<&TrafficCar>, Option<&PoliceCar>, Has<Character>, Has<Vehicle>, Has<ColliderDisabled>,
     Has<RigidBodyDisabled>)>`, `Res<VehicleConfig>`, `Res<LocomotionConfig>`, `Res<TrafficConfig>`,
     `Res<TrafficGraph>` (the set runs only with a graph), `Res<Time<Fixed>>`, the player `Position`.
     Skips entities with neither marker, and any with `ColliderDisabled` or `RigidBodyDisabled` (seated drivers,
     the player in a car — avian queries skip them too). Keeps `Dead` characters (a body on the road is a
     standing character). Radius: bodies within `bubble.in_view.despawn + look_ahead` (90 + 60 = 150 m, derived)
     of the player; all bodies when there is no player. `standing` persists in the map (pruned to live bodies each
     rebuild, `+= dt` when speed ≤ `hold_speed`, else 0). `siren = police::sirens_on(state)`.
   - Claims are DERIVED here every tick from `TrafficCar.manoeuvre == Pass{..}` plus the car's pose and the graph
     (`pass::claim_rect`, stage 4; until then the list is empty), so every consumer sees last tick's passes from the
     start of the tick. `insert_claim(&mut self, claim)` adds a same-tick claim when `advance_traffic` commits a
     pass, so a car later in the entity-ordered loop, the spawner and the police see it at once.
   - `reset_occupancy` on `NEW_CITY` clears everything. `init_resource::<RoadOccupancy>()`; not reflected (QA reads
     `TrafficCar.manoeuvre`/`lateral`).
   - Plugin: `configure_sets(FixedUpdate, OccupancySystems.after(TrafficSystems::Hijack).after(TrafficSystems::Bail)
     .before(TrafficSystems::Drive).before(TrafficSystems::Bubble).in_set(NpcSystems).run_if(resource_exists::
     <TrafficGraph>))`.
2.2 New `crates/gta_sim/src/occupancy/query.rs`:
```rust
pub struct Strip { pub origin: Vec2, pub dir: Vec2, pub length: f32, pub half_width: f32 }
pub struct Hit { pub entity: Entity, pub gap: f32, pub speed_along: f32, pub standing: f32,
                 pub kind: BodyKind, pub dynamic: bool, pub claim: bool }
impl RoadOccupancy {
    pub fn first_along(&self, strip: &Strip, skip: impl Fn(&RoadBody) -> bool) -> Option<Hit>;
    pub fn blocked(&self, rect: &FlatRect, skip: impl Fn(&RoadBody) -> bool,
                   claims: ClaimFilter) -> Option<Entity>;
    pub fn body(&self, e: Entity) -> Option<&RoadBody>;
    pub fn sirens(&self) -> impl Iterator<Item = &RoadBody>;
    pub fn insert_claim(&mut self, claim: Claim);
}
pub enum ClaimFilter { None, All, Except(Entity) }
pub fn world_clear(spatial: &SpatialQuery, rect: &FlatRect, bottom: f32, top: f32) -> bool;
```
   - `first_along`: coordinates along `dir` (a) and across (c) relative to `origin`. Rect bodies are clipped to the
     band `|c| <= half_width` (Sutherland-Hodgman on the two band lines); circles use `a − sqrt(r² − max(0,
     |c| − hw)²)` when `|c| < hw + r`. A body counts only if some clipped point has `a` in `[0, length]`; bodies
     entirely behind the origin (max `a` < 0) are excluded; a body straddling the origin gets `gap = 0`
     (clamped), never negative. Claims are returned only when `claim.dir · strip.dir < 0` (opposite direction),
     with `speed_along = 0`, `standing = f32::MAX`, `claim = true`. `speed_along = velocity · dir`. AABB prefilter,
     brute force.
   - `blocked`: first body (flat SAT, rectangles shrunk 1 cm per side like `obb_overlap`, circles by distance)
     or claim (by `ClaimFilter`) overlapping `rect`, except `skip`.
   - `world_clear`: an avian `shape_intersections` with a box over `rect` between `bottom` and `top`, mask
     `GameLayer::World`; empty = clear. The static road edge stays in avian.
   - Unit tests in `query.rs`: a rect straight ahead, one straddling the band edge, one behind the origin (excluded),
     one straddling the origin (gap 0), a circle at the band edge (the `traffic_pedestrian` flank case: centre
     1.14 m behind the nose, |c| = 1.5, r 0.3 → excluded), a same-direction claim (ignored), an opposite claim.
2.3 `crates/gta_sim/src/lib.rs`: `pub mod occupancy;`, `OccupancyPlugin` in the `(VehiclePlugin, TrafficPlugin)`
   tuple (`:207`).
2.4 `traffic/mod.rs`: `TrafficCar` gains `pub lateral: f32` (m, + = right of the path tangent, right of `t` is
   `(-t.z, 0, t.x)`), `pub manoeuvre: Manoeuvre`, `pub calm: f32`:
```rust
#[derive(Reflect, Clone, Copy, Debug, PartialEq, Default)]
pub enum Manoeuvre {
    #[default] None,
    Rejoin,
    Pass { obstacle: Entity, offset: f32, need: f32, hold_s: f32, merge_s: f32, end_s: f32 },
    Yield { siren: Entity, since: u64, offset: f32 },
}
```
   `need` = the lateral magnitude that clears the obstacle (§ stage 4), `hold_s` = the path `s` of the obstacle's
   rear minus `half_length` minus `idm.min_gap` (where the nose must stop while `|lateral| < need`).
   `register_type::<Manoeuvre>()`. Update the only literal (`spawn.rs:63`: `lateral: 0.0, manoeuvre:
   Manoeuvre::None, calm: 0.0`). `abandon()` resets `lateral`, `manoeuvre`, `calm`. `configure_sets`:
   `TrafficSystems::Drive.after(OccupancySystems)`, `TrafficSystems::Bubble.after(OccupancySystems)`.
2.5 `police/mod.rs` `configure_sets`: `PoliceSystems.after(OccupancySystems).after(TrafficSystems::Drive)`
   (Drive writes same-tick claims that the police lane choice reads; without the order the executor decides).
2.6 New `traffic/lateral.rs`:
   - `LateralConfig { rate_at_rest: f32, slope: f32, yaw_rate_deg: f32 }` in `traffic/config.rs` (+ validate:
     all > 0) and `traffic.ron` `lateral:`. Rate law `r(v) = rate_at_rest + slope · v` (m/s).
     Starting values `lateral: (rate_at_rest: 0.8, slope: 0.15, yaw_rate_deg: 60.0)` with this "why" comment
     and worked example in the stage summary: at rest a pass shifts `need` = 2.9 m (aligned car: 1.2 + 0.5 +
     1.2) in 2.9 / 0.8 = 3.6 s; the G2 floor is 60 s / 8 cars = 7.5 s per car, so the at-rest shift must stay
     below ≈ 4 s; at `pass.speed` 6 m/s r = 1.7 m/s, a 3.25 m shift takes 1.9 s = 11.5 m of travel; yaw offset
     atan(1.7/6) = 15.8°. The owner judges the look; the implementer may retune within the worked example.
   - `target_lateral(car, rear_s) -> f32`: `Pass` → `offset` until the rear passes `merge_s`, then 0;
     `Yield` → `offset`; else 0.
   - `step_lateral(lateral, target, v, dt, cfg) -> f32`: `lateral + clamp(target − lateral, ±r(v)·dt)`.
   - `offset_pose(graph, seg, s, lateral) -> (Vec3, Vec3)`: `pose(seg, s)` + right(tangent) · lateral. On a
     connector the caller passes `l0 · (1 − s/len)` (linear decay, `l0` = lateral at connector entry).
   - `yaw_target(path_yaw, rate, v) = path_yaw + atan(rate / max(v, 1.0))`, reached at most at `yaw_rate_deg`,
     applied only while `lateral != 0 || manoeuvre != None`. Angular velocity = scaled axis of
     `Quat::from_rotation_y(target_yaw) * rotation.inverse()` over `dt`, clamped to `yaw_rate_deg` (this also levels
     a recovered car's residual roll/pitch). An on-path car keeps today's one-tick yaw snap, so every existing
     traffic gate keeps its behaviour.
   - `drive.rs` kinematic motion (`:447-455`): uses `offset_pose` + the yaw law only when `lateral != 0 ||
     manoeuvre != None`; the lateral advances by `step_lateral` every tick. Dynamic autopilot target (`:406-410`)
     adds `right · target_lateral`.
   - Config rows in `tests/config_traffic.rs` (each key's failing side with its own keyword).
2.7 New `traffic/lanes.rs` + `graph.rs` (+< 10 lines):
   - `TrafficLane.left_gap: Option<f32>`: distance to the nearest antiparallel lane (dir · dir' < −0.99) whose line
     lies to the LEFT at a lateral distance in `(2·half.x, 4·half.x]` and overlaps longitudinally. Computed in
     `TrafficGraph::new` via `lanes::left_gaps(&built, half_width)` (floors get it when they build a two-way road).
   - `TrafficLane.curb_lane: bool`: true only when the layout has a same-direction slot-1 lane on the same edge
     (an avenue). Set in `from_layout` from `layout.lanes` (`lanes::curb_lanes(...)`); `new` sets false, so floors
     keep today's behaviour unless a test calls the `set_curb_lane` setter (step 1.1).
   - Unit rows: street edge (left_gap 3.25, curb false), avenue edge (left_gap 3.25, curb true), one-way floor
     (None, false), the `plus()` in lane (left_gap 3.25).
2.8 `drive.rs` sensing (`:295-381`) → two `first_along` strips from the nose (`origin` = pose at `s + half_length`
   shifted by the lateral), direction = path tangent, `half_width = half.x`:
   - strip T at the TARGET lateral, length `sense_distance` (lanes) / `turn_sense_distance` (connectors);
   - strip C at the CURRENT lateral (kinematic: `lateral`; Dynamic: signed re-projection offset), length
     `min(reach, v · |target − lateral| / r(v) + 2 · idm.min_gap)` — the lateral move still to go plus twice the
     jam gap (a length equal to the IDM rest gap would drop a body the car stands behind: TASK-016 lesson). When
     `target == lateral` strip C is not cast (identical to T).
   - Skip: self; `OnPathTraffic` only when the sensing car has `target == lateral == 0` and `manoeuvre == None`
     (today's behaviour: a queue head's straight strip into a box never brakes for cars on non-conflicting
     connectors). A manoeuvring car sees on-path traffic too (a passer sees oncoming cars). `Traffic`, `Vehicle`,
     `Character` and opposite claims are always seen. IDM obstacle per hit: `obstacle(hit.gap, hit.speed_along)`.
   - Removed: the `kinematic` HashSet, `slab`, `filter`, `speed_of`. `others` stays (bailing shooter lookup).
     `advance_traffic` takes `ResMut<RoadOccupancy>` (claims from stage 4). `TrafficStats.casts` counts sensing
     calls (liveness of every gate that uses it).
2.9 `drive.rs` `lane_start_free` → `occupancy.blocked(rect over the first need m of the lane, skip, ClaimFilter::All)`
   with skip = characters, `OnPathTraffic`, and `Traffic` bodies that are `dynamic` (today's semantics: every AI car
   on the path is counted by `room()`; the new inputs are off-path kinematic AI cars and claims).
2.10 `spawn.rs` chassis check (`:205-214`) → `occupancy.blocked(today's chassis rect, skip = none, ClaimFilter::All)`:
   today's body set (every Character and Vehicle, AI cars included) plus claims. No forward extension (it would move
   the `TrafficRng` stream and every seeded gate). The `on_lane` spacing stays. `spawn_traffic` takes
   `Res<RoadOccupancy>`; `spatial` and the chassis collider go away if unused.
2.11 Gates:
   - `tests/traffic_gridlock.rs`: `Footprints::record` every tick on all four seeds, `assert_clean` at the end (the
     "≥ 8 seeded runs" count is completed by G2-G6).
   - New `tests/traffic_occupancy.rs` (G8, floors from `traffic_support` with same-direction lanes only, so no pass
     can start), each row flipped by removing that consumer's occupancy input in code, recorded:
     - sensing (a): lanes L1 (+X, z 30) and L2 (+X, z 26.75, 3.25 m to L1's left). Car A on L1 standing, set to
       `lateral = −1.4`, `manoeuvre = Yield { siren: Entity::PLACEHOLDER, since: <tick>, offset: −1.4 }` (it holds:
       the resume rule of stage 6 only times out when the siren body is absent, and the row lasts ≤ 320 ticks
       < `sirens.timeout_seconds`); car B on L2 40 m behind at 12 m/s. B stops with bumper gap ≥ `min_gap − 0.1`;
       oracle clean; `GATE BROKEN` if A's lateral drifts > 0.05. Flip: skip `Traffic` kind in B's strips → oracle RED.
     - sensing (b): an Abandoned traffic car, a Taken car and a police car (`PoliceCarState::Leave`, sirens off) in
       L1; the approaching car stops. Flip: drop `Vehicle` from the snapshot.
     - sensing (c): a dummy ahead of the bumper holds the car; RED when characters are dropped from the snapshot.
     - lane start: an off-path kinematic car at the start of a destination lane blocks the grant; flip: skip it.
     - spawner (stage 4 row, needs claims): a traffic floor with a `CameraView`, a car with `Manoeuvre::Pass` whose
       derived claim covers a spawn point in the band for every tick of a 64-tick window (`GATE BROKEN` otherwise);
       no car spawns on that point; liveness: a spawn happens elsewhere. Flip: `ClaimFilter::None` in the spawner.
     - junction row: stage 5.
Check: existing `traffic_*`, `police_*`, `vehicle_*` gates unchanged-green; gridlock worst stops printed next to the
stage-1 numbers (drift can only come from 2D-vs-3D shape differences and is reported, never absorbed); oracle
clean on the four seeds; `traffic_bench` mean vs baseline.

### Stage 3 — bumped-car recovery (B, G3)

3.1 New `traffic/recover.rs`: `recover_dynamic(snap, occupancy, graph, cfg, vcfg, dt) -> Recovery` with
   `enum Recovery { Stay, Recover { lateral: f32 }, GiveUp }`, called from `advance_traffic` step 1 for Dynamic
   snaps after `reproject` (a lost car stays on the existing abandon rule). `calm += dt` while ALL hold, else
   `calm = 0`:
   - upright: `(rot · Y).y >= cos(recover.max_tilt_deg)`; not lost; at rest: speed ≤ `hold_speed`;
   - hysteresis: no dynamic body in the occupancy whose relative sweep over `recover.horizon_seconds` hits the
     footprint grown by `recover.skin` (the switch predicate `swept_rect_hits_rect`/`swept_circle_hits_rect` with a
     wider skin and longer horizon);
   - rejoin corridor clear: the motion from the current pose to the on-path pose (lateral 0, path yaw) sampled at
     `n = max(1, ceil(max(|Δyaw| / 7.5°, |Δlateral| / 0.3)))` + 1 rectangles (linear in lateral and yaw), each grown
     by `recover.skin`, all `blocked(.., skip self, ClaimFilter::All) == None`. The 7.5° / 0.3 m steps keep the corner
     chord between samples (2.37 m · sin 7.5° ≈ 0.31 m) below the 0.4 m skin: geometry law, not tuning.
   At `calm >= recover.seconds` → `Recover`. A Dynamic car that stood `recover.give_up_seconds` without recovering →
   `GiveUp`.
3.2 Apply in the write-back loop (`drive.rs:505-512`): `Recover` → `try_insert(RigidBody::Kinematic)`,
   `try_remove::<(Autopilot, DriveIntent, SleepingDisabled)>()`, `mode = Kinematic`, `lateral` = measured signed
   offset, `manoeuvre = Rejoin` (cleared when `|lateral| < 0.01` and yaw within 1° of the path), `calm = 0`.
   `GiveUp` → `Bailing { attack: None, shooter: None }` only if `exit_spots` returns a spot this tick, else
   `abandon()` directly (a Bailing car with no door would stay Dynamic and standing forever, `drive.rs:487-489`).
   A car switched mid-pass keeps its `Pass` target (its autopilot target includes the lateral).
3.3 `traffic/config.rs`: `RecoverConfig { seconds, skin, horizon_seconds, max_tilt_deg, give_up_seconds }`;
   validate: all > 0, `max_tilt_deg` in (0, 90), laws `recover.skin > switch.skin`, `recover.horizon_seconds >=
   switch.horizon_seconds`. `traffic.ron` `recover: (seconds: 1.5, skin: 0.4, horizon_seconds: 0.5,
   max_tilt_deg: 10.0, give_up_seconds: 10.0)`, comment: skin/horizon four and five times the switch's, so a body
   that would re-switch the car within a tick never lets it recover.
3.4 New `tests/traffic_recovery.rs` (G3; `loop_lanes` for (a)(c)(d), `two_way_street` for (b), `avenue_floor`
   for (e); G1 oracle in every row):
   - (a) nudge at 2 m/s from behind by a kicked car: `Kinematic` and speed > 0 along its lane within
     `recover.seconds + 1 s` after it came to rest. Flip: `recover.seconds = 1e6` → RED.
   - (c) the player's car held pressed (throttle into its rear) for 10 s: switches from
     `TrafficStats.switches_by_cause` and mode transitions; at most one re-switch per `recover.seconds`; the car
     ends Bailing/Abandoned only after `give_up_seconds`. Flip: recovery on the switch's own skin/horizon → RED
     (counted flip-flop).
   - (d) rotation set upside down: `Abandoned`.
   - (e) corridor: a car shoved 1.5 m sideways and yawed 30°, a sleeping dynamic car placed so that it overlaps the
     rejoin corridor but stays more than `recover.skin` from the shoved car's current rect (both asserted at setup,
     `GATE BROKEN` otherwise). Expected: no recovery while the corridor is blocked, 0 re-switches, the car ends
     Bailing/Abandoned after `give_up_seconds`. A second setup with the sleeping car moved clear: it recovers within
     `recover.seconds + 1 s`. Flip: remove the corridor check → the first setup recovers and re-switches (≥ 1) → RED.
   - (b) comes in stage 4.
3.5 Un-ignore the stage-1 (b) rows that reproduced; they must now be green. Config rows for every new key.

### Stage 4 — go-around of standing vehicles and characters (C, G2)

4.1 `traffic/config.rs` + `traffic.ron`: `pass: (vehicle_seconds: 3.0, character_seconds: 6.0, trigger_gap: 6.0,
   clearance: 0.5, speed: 6.0)`. Comments: a character waits longer than a car (premise amendment 2; this is the
   spec's `go_around_wait_s`); trigger gap = s0 + 4 m so only the queue head behind the obstacle evaluates;
   `character_seconds` 6 > `reservation_timeout` 5 keeps `traffic_intersection::contested_lease_lapses_to_the_waiter`
   (a holder behind a dummy on the `plus()` floor, which has antiparallel lanes) standing for its whole lease.
   Validate: all > 0, `trigger_gap > idm.min_gap` (a threshold at IDM's rest point never fires),
   `character_seconds >= vehicle_seconds`.
4.2 New `traffic/pass.rs`: `passable(hit, cfg) -> bool`, `plan_pass(occupancy, spatial, graph, junctions, snap,
   hit, cfg) -> Option<Manoeuvre>`, `claim_rect(graph, car, pass, half) -> Claim`, `pass_done(car, pass) -> bool`.
   Called from `advance_traffic` after sensing, for cars with `manoeuvre == None` on a lane.
   - Passable: the strip-T first hit, `gap <= pass.trigger_gap`, kind `Vehicle`, `Character`, or `Traffic` with
     `dynamic`, `standing >= pass.vehicle_seconds` (vehicles) / `pass.character_seconds` (characters). Never
     `OnPathTraffic`, never a kinematic `Traffic` car (yielding/rejoining cars are temporary), never a claim.
   - Commit, per side in order curb (+) then oncoming (−):
     1. `w` = `lane.left_gap` (oncoming side; `None` → side unavailable) or 3.25 m lane pitch on the curb side, which
        is available only when `lane.curb_lane`. `need` = the obstacle footprint's lateral extent on that side (from
        the lane centre line) + `pass.clearance` + `half.x`; `offset = ±max(w, need)`; infeasible when
        `max(w, need) > w + (w/2 − half.x)` (the passer would leave the asphalt).
     2. `hold_s` = obstacle rear (min along-coordinate of its footprint on the path) − `half_length` − `idm.min_gap`;
        `merge_s` = obstacle front (max along-coordinate) + `pass.clearance` + `half_length`;
        `end_s = merge_s + half_length + w · pass.speed / r(pass.speed)`.
     3. Claim rect: from the car's rear to `end_s + half_length` along the path, centred at `offset`, half width
        `half.x + pass.clearance`. Requires `blocked(claim, skip = self and same-direction AI passers ahead,
        ClaimFilter::All) == None` (bodies AND claims; characters included, so the manoeuvre never runs anyone over).
     4. Oncoming side only: the stretch beyond the claim end, length `v0'² / (2·b) + s0` of the opposite lane's v0
        (the comfortable stop distance, derived from IDM data; 45 m on a street), holds no vehicle.
     5. Curb side only: `world_clear` over the claim rect.
     6. If `end_s > lane.stop − half_length` (the pass reaches the lane end): the lane must have a straight out
        connector (`dir_in · dir_out > 0.99`); `next` is set to it (no RNG draw); the claim extends along the offset
        path through the box to the connector end; AND no live grant at that node may have a connector whose swept
        rects (`box_rules::connector_rects`) overlap the claim (a car granted before the commit would otherwise meet
        the passer head-on at the start of the opposite lane and both would hold). If any fails, no commit on that
        side.
   - On commit: `manoeuvre = Pass{..}`, v0 capped at `min(lane v0, pass.speed)`, `occupancy.insert_claim(..)` at
     once (R1b). The claim is re-derived in `snapshot_road` every tick while the manoeuvre lasts, shrinking with the
     car. `pass_done` when the rear passes `merge_s` and `|lateral| < 0.01` → `manoeuvre = None`.
   - Hold behind the committed obstacle: while `|lateral| < need` the nose never passes `hold_s` (IDM obstacle at
     `hold_s + half_length − (s + half_length)` with speed 0, and the kinematic step clamps `s <= hold_s`). The
     passer shifts at rest first, then drives; its lateral clearance to the obstacle is `pass.clearance` (0.5 m >
     `switch.skin` 0.1), so the switch never fires on the obstacle.
   - A pass whose obstacle entity is gone keeps its geometry to `end_s` (the claim stays valid).
   - Followers are not special-cased: they follow the passer through `leader()` (it stays on their path by `s`),
     then evaluate their own pass when the obstacle becomes their strip-T hit.
4.3 Worked example in the stage summary (numbers through the real code path):
   - street, obstacle aligned in the lane: need 2.9, offset −3.25, `hold_s`, `merge_s`, `end_s` for `pass.speed` 6,
     claim length, the 45 m stretch; the at-rest shift time and the per-car cycle vs the 7.5 s floor;
   - oncoming car standing at the claim end, passer rear at `merge_s`: show strip C at the current lateral does
     not hold the passer at that gap with the stage-2 lateral values;
   - the offset pose sign on three directions (a +Z lane, a +X lane, the straight connector with decaying lateral).
4.4 New `tests/traffic_go_around.rs` (G2): production city, seeds 1, 2, 7, 42; per seed one street row and one
   avenue row (lane found per seed: the longest lane of the class in the bubble with ≥ 70 m before its stop line;
   `GATE BROKEN` if none), plus character rows (dummy standing in a street lane, seeds 1 and 7). An abandoned car
   (`spawn_car`, no driver) stands mid-lane; the player stands on the sidewalk facing it 20-30 m away. The fixture
   feeds cars with `spawn_traffic_car` at the lane start every 4 s until 8 have queued (named fixture; natural
   traffic also counts). Asserts (collect all): the first 8 AI cars that queue pass (rear past the obstacle's front)
   within 60 s of joining the queue; no AI car stands > 30 s; no Dynamic AI car stands > 30 s; 0 `CollisionStart`
   between a car in `Manoeuvre::Pass` and any `Vehicle` (`MessageCursor`, as in `traffic_contact.rs`); G1 oracle
   clean. Tighten 60 s from the worked example if it allows. Flips: `pass.vehicle_seconds = 1e6` → G2 RED; the
   passer typed `OnPathTraffic` (hidden from oncoming strips) and its claim hidden from `first_along` → G1 RED on at
   least one row.
4.5 Regression, named: both `traffic_pedestrian.rs` hold gates stay green (their floor has no antiparallel lane and
   no curb lane); record that setting `curb_lane = true` on that floor turns
   `a_walker_ahead_of_the_bumper_still_holds_the_car` RED (the R3 flip). `traffic_intersection.rs` stays green
   (run it 3 times: the `plus()` floor has `left_gap`).
4.6 G3 (b) row: shoved beyond `lost.distance` on `two_way_street` → `Abandoned`, then a following car passes it.
   Un-ignore stage-1 (a) and (c).
4.7 Fallback: a second pass-through or head-on failure of the same class → `REDESIGN_NOTE.md` proposing the named
   fallback: footprint-only passers (no oncoming side) plus "a car stuck behind a standing obstacle for
   `pass.stuck_despawn_seconds` while off frame despawns (the bubble respawns elsewhere)". The value and its data row
   are added only if the fallback is taken (not built now).

### Stage 5 — junction box (D, G4)

5.1 New `traffic/box_rules.rs`:
   - `connector_rects(graph, c, half_width) -> Vec<FlatRect>`: one rect per polyline segment, no end caps, half width
     `half.x + conflict_margin / 2`.
   - `connector_clear(occupancy, graph, junctions, c, requester) -> Option<Entity>` (the first blocker): sees every
     body and claim overlapping `connector_rects(c)` EXCEPT the requester, characters (walkers are the strips' job;
     adding them to grants is the TASK-033 crosswalk-wait class) and AI cars that hold a grant at this node (the
     conflict table covers them). On-path kinematic AI cars WITHOUT a grant at this node are seen: a demoted holder
     (D2) standing on its connector must block crossing grants.
   - `straight_out(graph, lane) -> Option<u32>`, `repick(graph, occupancy, junctions, lane, current) -> Option<u32>`
     (first out connector after `current` in `out` order whose `connector_clear` is None; no RNG).
   - `connector_pass(..)` (D5 below).
   `graph.rs`: `polyline_distance` stays private unless needed.
5.2 `junction.rs`, `junction::update` gains `&RoadOccupancy`:
   - D1: a grant also needs `connector_clear(c) == None`. A waiter that fails it is NOT pushed to `blocking` (same as a
     `room` failure: a car waiting on a body in its path must not lock the node).
   - D2 lease tail: a holder on its connector, or on its from-lane with the nose past the stop line, that has not
     moved for `lease_ticks` while contested is demoted: its grant is removed (collect in a Vec inside the
     `occupants.retain`, push after it) and it becomes a waiter IN PLACE with stamp = the current tick. The waiter
     `retain` accepts `Segment::Connector(c)` for its own `c` (skipping the `heads` and from-lane checks for it).
     Re-grant uses the normal rules plus `connector_clear` of its connector.
   - D3 re-pick: a queue head whose chosen connector's `connector_clear` returns a blocker with `standing >=
     pass.vehicle_seconds` takes `repick(..)`, keeping its `waiting` stamp; no extra `TrafficRng` draw.
   - D4 (offset entry): a car with `lateral != 0` at its lane end (only possible through stage-4 step 6, on its
     straight connector) requests a whole-box grant: it conflicts with every connector of the node (blocks and is
     blocked by all), FCFS as usual.
   - D5: a car on a connector whose strip-T hit is a standing non-AI body (`Vehicle`, or `Traffic` with `dynamic`,
     `standing >= pass.vehicle_seconds`) tries a connector pass: offset ±`w` (w = 3.25 lane pitch) decaying to 0 at
     the connector end, claim = offset path rects, requiring `blocked == None`, `world_clear`, and no other live
     grant at the node; it then holds a whole-box grant like D4. If neither side is clear it holds; its stand shows
     in G4. A second failure of this class triggers the named fallback through `REDESIGN_NOTE.md`.
5.3 `drive.rs` motion: a car on a connector without its grant holds (kinematic: `v = 0`, `s` unchanged; Dynamic:
   `pilot.speed = 0`); lateral decays linearly on connectors (`l0 · (1 − s/len)`).
5.4 New `tests/traffic_junction_box.rs` (G4): production city seeds 1 and 7, the gridlock pose (player at the spawn
   looking north), a non-AI car placed at t = 20 s on a free spot of a connector path inside the busiest box in view
   (`GATE BROKEN` if the spot overlaps a body), 120 s. Asserts: no grant holder that has not moved keeps its grant
   against a conflicting waiter longer than `reservation_timeout + 1 s` (the gridlock stale metric extended to
   holders on connectors and past the line); no AI car on any approach stands > 40 s; G1 clean. Extra row: an AI car
   already on the blocked connector behind the body; its stand is bounded by 40 s. Flips: D1 removed → RED (a grant
   into the blocked path: contact or oracle); D2 removed → stale-grant RED; D5 removed → the extra row RED.
   G8 junction row in `traffic_occupancy.rs` (the `plus()` floor from `traffic_intersection.rs`): a parked car on a
   connector path; that connector is never granted while it stands. Flip: `connector_clear` input removed.
5.5 `traffic_intersection.rs` existing rows stay green (conflict-table regression; run 3 times). Un-ignore the
   stage-1 (d) row.

### Stage 6 — sirens, any lane, spawn sectors (E, G5-G7)

6.0 Feasibility probe first: on seeds 1..10 in the G6 pose (step 6.5), measure the share of 25 s windows in which the
   oncoming street lane next to the queue has a gap of at least the queue length + 2 · `sense_distance`. Physical
   limit: a street is 6.5 m of asphalt; with both directions at their curbs the gap between rows is 0.85 − (−0.85) =
   1.7 m < 2.4 m, so a police car passes a street queue only through a gap in oncoming traffic. If the share is below
   8/10, report to the orchestrator before building G6 (options: G6 on an avenue lane, or an oncoming yield that stops
   oncoming cars before the police corridor). Do not tune around it silently.
6.1 New `police/siren.rs`:
   - `pub fn sirens_on(state: PoliceCarState) -> bool` (Respond | Chase).
   - `SirenConfig { lane_offsets: Vec<f32>, lane_hold_seconds: f32, lane_gain: f32 }` + validate (offsets finite,
     |offset| ≤ 1.0, hold ≥ 0, gain ≥ 0); `PoliceCarConfig.sirens: SirenConfig` (`police/mod.rs`, validation
     delegated).
   - `#[derive(Component, Reflect, Default)] #[reflect(Component)] pub struct SirenLane { pub offset: f32, pub held: f32 }`,
     registered; `#[require(SirenLane)]` on `PoliceCar` (`cars.rs`, +1 line).
   - Reference lane: `graph.nearest(position)`; offsets apply only on a `Segment::Lane` whose `dir · forward >
     cos 45°`; on a connector, in a box (`graph.in_junction`) or with no such lane the offset is forced to 0.
   - `choose_lane(occupancy, lane, s, current, cfg, ignore) -> f32`: candidates `[0] ∪ lane_offsets` (in units of
     `w` = 3.25); each gets a strip from the nose along the lane tangent at that offset, length `sense_distance`,
     half width `half.x`, skipping self and `ignore` (the player's car in Chase); clear length = hit gap or the full
     length. Leaving 0 for an offset needs `clear(offset) − clear(0) > lane_gain` and `held >= lane_hold_seconds`;
     returning to 0 needs only `clear(0) >= clear(current)` and `held >= lane_hold_seconds` (home lane wins ties).
   - `corridor_obstacle(occupancy, strip, ignore) -> Option<(f32, f32)>`: the IDM input while sirens are on.
   - `car_route.rs` call sites (few lines each): Respond: `target + right(tangent) · offset · w` (like `pull_over`);
     Chase: `at + right(tangent) · offset · w`. While sirens are on, the IDM obstacle is `corridor_obstacle` on the
     chosen strip instead of the heading cast (a heading error above atan(0.3/25) ≈ 0.7° made the heading cast hit a
     yielded car); the heading cast stays for sirens-off states. `drive_police_cars` takes `Res<RoadOccupancy>`.
6.2 New `traffic/sirens.rs` (E2), called from `advance_traffic` before pass evaluation:
   - Trigger: a kinematic AI car on a lane with `manoeuvre == None` yields to a siren body whose along-offset is in
     `[−sirens.yield_distance, −half_length]` behind it, lateral offset ≤ `w` from its path, and heading
     `forward · tangent > cos 45°` (heading, not velocity: a siren car standing behind the queue still gets the yield).
   - Offset: avenue `+w` when `lane.curb_lane` and the curb-lane rect passes `blocked` + `world_clear`; else the in-lane
     slack `+(w/2 − half.x)`. IDM gets a virtual standing obstacle at `v²/(2b) + s0`.
   - Resume (target 0, IDM free): when the siren body is in the snapshot and its rear is past the car's nose, or after
     `sirens.timeout_seconds` since `since` (a missing siren body resumes only by timeout). After a timeout the car
     ignores sirens for another `timeout_seconds` (hysteresis).
   - `traffic.ron` `sirens: (yield_distance: 40.0, timeout_seconds: 8.0)`; `yield_distance >= 0` (0 = never, the
     `pull_over` precedent, needed for the G6 flip).
6.3 New `police/spawn_sector.rs`:
   - `SectorConfig { ahead, beside, behind, ahead_deg, behind_deg }` (shares ≥ 0, sum 1 within 1e-3,
     `0 < ahead_deg < behind_deg < 180`); `PoliceCarConfig.spawn_sectors`; `escalation.ron`
     `car.spawn_sectors: (ahead: 0.3, beside: 0.3, behind: 0.4, ahead_deg: 45.0, behind_deg: 135.0)` and
     `car.sirens: (lane_offsets: [-1.0], lane_hold_seconds: 1.0, lane_gain: 5.0)`, comment: −1.0 w (police centre
     at the opposite lane centre) leaves 1.275 m to a street car yielded by its 0.425 m slack; −0.7 w leaves 0.30 m
     and still overlaps the oncoming lane.
   - Sector of a candidate = the bearing of `(candidate − player)` relative to the player's heading (the driven car's
     velocity direction while pursuing): ahead ≤ `ahead_deg`, behind > `behind_deg`, beside between.
   - `car_dispatch.rs` while pursuing: next sector = largest `share · (n + 1) − count` (quota, deterministic, no RNG);
     `pick_spawn` over that sector's candidates; a candidate failing hidden/overlap is removed and the pick repeats
     within the sector; an empty sector falls back to the next by deficit (counted as a fallback). Hidden rule and
     per-star caps untouched. `PoliceDispatcher` gains `sector_spawns: [u32; 3]` and `sector_fallbacks: u32`
     (reflected; reset in `reset_dispatcher`). The overlap check stays avian (today's).
   - Removed (dead after this): `approach_clear`, `lane_costs_to`, `step_cost` and the `approach_clear_rows` unit test
     in `car_route.rs`; in `car_dispatch.rs` the `traffic: Query<&TrafficCar>` param, `queue`, `radius`, `costs`.
     Decision: the pursuit filter goes (spec lets the planner decide; G6 checks it).
6.4 `tests/config_police.rs` rows for `sirens` and `spawn_sectors`; `tests/config_traffic.rs` rows for `sirens`.
   Replace `police_car_floor.rs::police_car_never_spawns_behind_a_traffic_queue` by a floor row: a police car spawned
   behind a queue closes in. Move `cruise` from `tests/traffic_bench.rs` into `tests/vehicle_support/mod.rs`
   (`pub fn cruise`), `traffic_bench` imports it.
6.5 Gates:
   - `tests/police_sirens.rs` (G5): production city seeds 1 and 7, `traffic.bubble.max_cars = 0` (named: the
     oncoming lane is empty, so the gate is about the yield mechanism; the oncoming interplay is G6's), a street lane
     ≥ 90 m, 6 AI cars queued through `spawn_traffic_car` (rows: moving at street v0; standing behind the stop line),
     a police car with the production component set (`police_car_floor.rs::spawn_police_car` pattern) 50 m behind in
     `Respond`, dispatcher off (`no_police_cars`), the player on foot on the sidewalk beyond the queue head by at
     least `dismount_distance` + the police stopping distance at `pursuit_speed` (v²/(2·max_deceleration)) + one car
     length (value derived in the summary). Rows: every queued car whose nose is within `yield_distance` shifts
     > 0.3 m toward the curb and stops; the police car passes all 6; each yielded car is Kinematic, back to
     `|lateral| < 0.05` and moving within `timeout_seconds` after the police rear passes its nose; the police
     `blocked` timer stays below `blocked_seconds` during the pass (a dismount cannot fake a pass); `Leave` row: no
     car's `manoeuvre` becomes `Yield`. G1 clean. Flip: `yield_distance = 0` → the yield row RED.
   - `tests/police_close_in.rs` (G6): seeds 1..=10, production population and traffic; the player's car cruising a
     street at 12 m/s (`cruise`), 3 AI cars queued between it and a police car 40 m behind (so it starts in Respond or
     Chase depending on sight; a Chase row places it within `direct_chase_distance` with the driver in sight), dispatcher
     off (named). A police unit within 18 m within 25 s in ≥ 8/10 seeds. Note: with the player driving a blocked police
     car stands (no dismount), so a RED here means standing. Flip (named): `sirens.yield_distance = 0` and
     `lane_offsets = []` → RED (count recorded). G1 clean.
   - `tests/police_spawn_sectors.rs` (G7): seed 1, 2 and 3 stars, the player driving a loop at 12 m/s (`cruise`), the
     view updated every tick (`set_view(chase_view(car position, car forward))`). Sector rows: the test despawns each
     police car one tick after it appears (named mutation, to sample the dispatcher ≥ 12 times); one row per sector
     with a fixed floor per sector taken from a measured run (ahead ≥ ⌈k⌉ of N, recorded in the summary), fallbacks
     reported; every spawn hidden (corners off the view cone or occluded) and overlapping no body. Flip: `ahead: 0.0`
     → the ahead row RED. Cap row (separate run, NO despawn mutation, 60 s): `dispatcher.cars <= row.cars` every tick.

### Stage 7 — docs, data, runtime QA, follow-up task

7.1 `docs/design/GDD.md` §5.2: one line each in Russian for the go-around of standing bodies (characters after a
   longer wait), bumped-car recovery, siren yield; §5.3: any lane with sirens, spawn ahead/beside by sector shares.
7.2 `docs/architecture/traffic.md`: occupancy (snapshot, kinds, strip/overlap API, claims derived from manoeuvre
   state plus same-tick insert), modes incl. the switch and its return, the lateral law, tick order (incl.
   `PoliceSystems.after(TrafficSystems::Drive)`), pass (hold behind the obstacle, straight-connector rule, live-grant
   check), junction D1-D5, sirens; migrated consumers (traffic sensing, lane start, junction grants, spawner, police
   lane choice) vs those pending in TASK-036 (NPC walk avoidance, fire line `tactics::nearby_cars`/`car_blocks`, sight
   `perception::sight_blocked`, conflict-point reservation). Replace the "cannot pass traffic" paragraph (`:104`).
7.3 TASK-036 text drafted in the stage summary for the orchestrator's `maw-tasks` (blocked by TASK-032).
7.4 Runtime QA: R1 (leave=1: no traffic car within 45 m stands > 30 s; control: 0 over 20 s; civilians within 8 m
   reported); t15 seeds 1/2/3 × 2 (pressure ≥ 5/6; hijack and "≤ 2 active police cars at 2★" green). Owner-run
   notes: recovery look (incl. the in-place rotation), pull-over, the at-rest sideways shift before a pass, pass glide,
   chase aggression.
7.5 G9: full suite, both benches under `MEAN_LIMIT`, clippy, client tests, tree_check, 5 CI workflows on the merge
   commit.

---

## 3. Test plan

| Gate | File | Class | What proves it | Flip |
|---|---|---|---|---|
| Causes | `traffic_causes.rs` | repro | RED on current code per cause (a)-(d) | (current code is the flip) |
| G1 | `traffic_support::Footprints` in `traffic_gridlock.rs` + every G2-G6 run | correctness | 0 kinematic interpenetrations over ≥ 8 seeded runs | hide a passer/yielder from the occupancy |
| G8 | `traffic_occupancy.rs` | correctness, one row per consumer | sensing a/b/c, lane start, spawner, junction | remove that consumer's input |
| G3 | `traffic_recovery.rs` | correctness | (a) recover in time, (b) abandoned + passed, (c) ≤ 1 re-switch per window, (d) upside down, (e) corridor | `recover.seconds` 1e6; switch's own skin; no corridor check |
| G2 | `traffic_go_around.rs` | liveness + correctness | 8 cars pass ≤ 60 s, no stand > 30 s, 0 passer contacts, G1 | `vehicle_seconds` 1e6; passer hidden |
| G4 | `traffic_junction_box.rs` | liveness + correctness | stale grant ≤ timeout + 1 s, stands ≤ 40 s, G1 | D1, D2, D5 removed |
| G5 | `police_sirens.rs` | correctness | yield, pass, resume, `Leave` | `yield_distance` 0 |
| G6 | `police_close_in.rs` | liveness | ≥ 8/10 seeds within 18 m in 25 s | yield and any-lane off |
| G7 | `police_spawn_sectors.rs` | correctness | per-sector floors, hidden, no overlap, caps | `ahead: 0.0` |
| G9 | full suite, benches, clippy, client, tree_check, CI | regression | all green, `MEAN_LIMIT` | — |
| R1, t15 | QA scripts | runtime | per-car stands; pressure ≥ 5/6 | baseline runs |

Named regressions to watch: `traffic_pedestrian` (both hold gates), `traffic_intersection` (3 runs; `plus()` has
`left_gap`), `traffic_gridlock` (4 seeds, printed drift), `police_car_floor` (the `plus()` floor gets a lane choice
for sirens-on cars), `traffic_bubble`, `traffic_contact`, `vehicle_*`. Config rows for every new key
(`lateral`, `recover`, `pass`, `sirens` in traffic; `car.sirens`, `car.spawn_sectors` in police).

Every gate collects all violations before it panics; every city gate uses the production population with a
stationary player facing the scene unless the row names a mutation; presentation is untouched (no client gate
changes expected; `cargo test -p gta_like --bin gta_like` stays green).

## 4. Rollout notes

- No migrations, no env vars, no feature flags. Save data: none in this project.
- New data keys (strict loaders, so an old `.ron` fails loudly): `traffic.ron` `lateral`, `recover`, `pass`,
  `sirens`; `escalation.ron` `car.sirens`, `car.spawn_sectors`. Both files ship in the same commit as the code.
- New reflected state for QA/BRP: `TrafficCar.{lateral, manoeuvre, calm}`, `Manoeuvre`, `SirenLane`,
  `PoliceDispatcher.{sector_spawns, sector_fallbacks}`. `t15.py`/`t13.py`-style scripts that deserialize
  `TrafficCar` must tolerate the new fields (QA checks the scenarios that read it).
- Seed drift: spawner acceptance is kept identical to today (plus claims), and re-pick, sectors, pass and yield draw
  no RNG. Any drift in gridlock numbers is reported, not absorbed.
- Frame budget: brute force (~100-150 bodies × ~24 cars × 2-3 strips, plus police strips and D1/D5 rects). Compare
  `traffic_bench`/`police_bench` with the stage-1 baseline at each stage; a uniform grid is the named fallback when the
  mean grows by more than 0.5 ms.
- Known physical limits (reported, not engineered): a wreck turned more than ~50° on a street needs more than 3.675 m
  of offset and cannot be passed; a police car cannot fit between two curb-pulled street rows (1.7 m < 2.4 m).

## 5. Review notes (changes from PLAN_V2 / PLAN.md and why)

Disconfirmation (before the review): counter-example "a floor used by an existing hold gate has an antiparallel lane
3.25 m to the left, so `left_gap` is `Some` and the held car goes around the dummy after `character_seconds`, turning
the gate RED". Searched every `traffic_floor`/`TrafficGraph::new` call. `traffic_pedestrian.rs` (V2's R3 case) uses
two same-direction lanes, so it did not hold there. It does apply to `traffic_intersection.rs::plus()` and
`police_car_floor.rs::plus()` (in lane vs out lane, 3.25 m apart): `contested_lease_lapses_to_the_waiter` holds a car
behind a dummy for the 5 s lease, which is shorter than `character_seconds` 6 s, so it stays green with a 1 s
margin. Recorded as a coupling in the `pass` comment and a named 3-run regression (4.1, 4.5).

Changes, each verified against code:
1. D1 body set (5.1, 5.2). V2/PLAN had `connector_clear` see "Vehicle, Traffic, claims". A D2-demoted holder is an
   on-path kinematic car standing on its connector; that set hides it, and a conflicting waiter would be granted
   through it (kinematic × kinematic pass-through). `connector_clear` now sees every AI car without a grant at the
   node. A `connector_clear` failure does not push to `blocking`, like a `room` failure (`junction.rs:175-181`).
2. D2 mechanics (5.2). The waiting stamp is cleared on grant (`junction.rs:191-195`), so "keeps its waiting stamp"
   was impossible. A demoted holder gets the current tick as its stamp. The waiter `retain` needs an explicit connector
   branch, because a connector car is never in `heads`.
3. Pass near the lane end (4.2 step 6). A whole-box request alone does not stop a car granted BEFORE the commit from
   leaving the box into the opposite lane and meeting the passer, who stands at that lane's start. Both would hold.
   The commit now also needs no live grant whose connector rects overlap the claim.
4. Strip C and the committed obstacle (2.8, 4.2). V2 used strip C length `v·|Δ|/r + min_gap`. At rest that equals
   IDM's rest gap, so the obstacle the car stands behind can drop out of strip C, and the passer drives into it. That
   is the TASK-016 threshold-at-rest-point class. Fixes: `+ 2·min_gap`, and an explicit hold at `hold_s` while
   `|lateral| < need`. The second also gives the passer its 0.5 m lateral clearance, above `switch.skin`, before it
   moves forward, so the TTC switch does not fire on the obstacle.
5. Police lane return (6.1). Under PLAN's symmetric gain rule a siren car stays in the oncoming lane whenever both
   strips are clear. Now the home lane wins ties.
6. Chase reference lane (6.1). V2 put the offset in the Chase arm but never defined which lane the offset is relative
   to. Chase has no lane route (`car_route.rs:543`). It now uses the nearest aligned lane, and offset 0 on connectors
   and in boxes.
7. V2 R4 said "blocked 2 s → dismount". That holds only when the player is on foot or stopped (`cars.rs:167-171`),
   and `blocked` grows only in Respond. In G6 (player driving) the failure mode is standing, not dismount (6.5 note).
8. G5 needs `bubble.max_cars = 0` (6.5). Otherwise oncoming production traffic can stall the police in the opposite
   lane, the on-foot player lets it dismount after 2 s, and the gate goes RED for a reason outside the yield mechanism.
9. G7 caps (6.5). Under the "despawn each car after a tick" mutation the cap holds trivially, so the cap has its own
   un-mutated run.
10. G1 tolerance wording (1.1). `max_overlap_solve_speed` is a push-apart speed, not an overlap bound (avian doc,
    `plugin.rs:243-250`). The number stays and its meaning is stated. The baseline (1.2) reports max depth.
11. Recovery corridor (3.1). `blocked` takes a `FlatRect`, not a hull, so the corridor is sampled rectangles with a
    geometry-derived step. G3(e) expected outcome corrected: while the corridor is blocked the car does not recover,
    and a second setup covers recovery.
12. Recovery predicate vs spec wording (3.1, logged decision). The spec says "no dynamic body within switch reach"
    (5 m). Literally, that never recovers a car whose nudging car or on-foot player stays within 5 m, so G3(a) could not
    pass. The hysteresis sweep predicate (skin 0.4, horizon 0.5) keeps the intent: no body that would re-switch it.
13. `lane_start_free` (2.9). V2's "Vehicle, Traffic, claims" would newly count Dynamic AI cars, which today are
    excluded and counted by `room()`. Kept today's semantics plus off-path kinematic cars and claims, to avoid silent
    gridlock drift.
14. Concrete values for `lateral` (2.6), derived from the G2 per-car floor (7.5 s): an at-rest shift of 2.9 m must stay
    under ~4 s. V2 R12 asked for values but gave none.
15. `snapshot_road` takes `Res<TrafficGraph>` under the set's `run_if` (V2 had `Option` plus `run_if`, which is
    redundant). `first_along` edge cases are specified: bodies behind the nose are excluded, straddling bodies get gap 0.
16. G8 (a) fixture made concrete: Yield with a placeholder siren, the stage-6 resume rule resumes only by timeout when
    the siren body is absent, and the row is shorter than the timeout.
17. `cruise` is private to `traffic_bench.rs`, so it moves to `vehicle_support` for G6/G7. The dispatch params that
    become dead after removing `approach_clear` are removed.
18. Detail restored from PLAN.md where V2 dropped it without correcting it: types and fields, snapshot query, the
    kind rules, the strip clipping math, stage-1 row variants, config values and comments, G2/G5/G6/G7 fixtures,
    flips, the D3 no-RNG re-pick, file-size targets.

Kept from V2 (verified): claims derived in the snapshot plus same-tick insert (R1); skip disabled bodies (R2);
`curb_lane` from data (R3); police lane choice in Respond and Chase with corridor-strip IDM and offset −1.0 w (R4); the
straight-connector whole-box entry (R5); D5 connector pass (R6); give-up to `abandon()` without a door (R7); G1 baseline
first (R8); spawner acceptance unchanged plus claims (R9); fixed G7 floors (R10); `PoliceSystems.after(Drive)` (R11,
no cycle: nothing outside `traffic/` orders against `TrafficSystems`); heading-based yield trigger (R13); the G5 player
position (R14); occupancy lifecycle on `NEW_CITY` (R15); the stage-6.0 feasibility probe.

Open risks, not decided here: G6 feasibility on streets (6.0 decides, the orchestrator picks the option); busy oncoming
traffic can starve oncoming-side passes (step 4's 45 m stretch alternates passers and oncoming flow; G2's 30 s bound
measures it). children: 0 launched / 0 reported.
