# PLAN — TASK-032: the street flows around the player's mess; sirens part traffic

Cost of error: silent (kinematic pass-through, minutes-long gridlock, frame budget). Full evidence layer.
The look of the moves (glide vs snap, pull-over, pass, chase feel) is judged in the owner run, not by gates.

Binding inputs followed: `TASK_FINAL.md` (Scope A-F, G1-G9, R1, t15), its "Premise challenge resolution"
(causal repros first, a standing character is an obstacle kind, R1 fixed), and the orchestrator note (one
shared occupancy API, per-consumer gates, staged plan that ships green, stop after the second failure of one
class, named rescope fallback, new code in new files).

---

## 1. Understanding (what the code does today)

### Traffic tick (`crates/gta_sim/src/traffic/drive.rs`, `advance_traffic`, 514 lines)
- Snapshots of AI cars only (`is_ai`: Kinematic, Dynamic, Bailing), sorted by entity bits (`:219-243`).
- Dynamic cars are re-projected on their path; `reproject` (`:128-161`) returns "lost" for > `lost.distance`
  4 m off, > `lost.angle_deg` 60 deg off, or upside down; lost cars are abandoned (`:505-512`).
- Per-segment path `Occupancy` (`:40-58`): kinematic cars see each other only through `leader()` (`:92-119`).
- Junctions (`junction::update`, `:282-293`) with `lane_start_free` (`:266-280`): an avian box over the first
  `need` m of the destination lane, Vehicle layer, AI snaps excluded.
- Forward sensing (`:296-379`): a 0.1 m slab cast from the nose along the PATH tangent, `sense_distance` 25 m
  on lanes, `turn_sense_distance` 6 m on connectors, mask Character + Vehicle, predicate
  `e != me && !kinematic.contains(&e)` (`:371`): every kinematic AI car is invisible to it, wherever it is.
- Kinematic motion (`:414-461`): `velocity = (pose(s') - position)/dt`, yaw snapped in one tick through
  `AngularVelocity`; the stop-line clamp when not granted (`:429-434`). Dynamic cars get an autopilot target on
  the path (`:400-412`). Nothing ever switches a car back to `Kinematic`.
- Bailing: a stopped bailing car lets the driver out (`:464-503`) and is abandoned.

### Switch (`traffic/contact.rs`, 311 lines)
`switch_to_dynamic` in `FixedPostUpdate` before `PhysicsSystems::First`: time-to-contact sweep of every
dynamic, non-disabled body in a broadphase box (`switch.reach` 5 m) against the car footprint grown by
`switch.skin` 0.1 m over `horizon_seconds` 0.1 s; `CollisionStart` backstop. Inserts `RigidBody::Dynamic`,
`SleepingDisabled`, `Autopilot`, `DriveIntent`. `FlatRect`, `swept_rect_hits_rect`, `swept_circle_hits_rect`
are the reusable geometry.

### Junctions (`traffic/junction.rs`, 197 lines)
Only lane heads queue; FCFS grants when no granted/earlier-queued conflicting connector (`conflicts` from the
centreline polyline distance, `graph.rs:233-246`) and the destination lane has `room` plus `lane_start_free`.
Lease lapse only for a holder standing BEFORE its stop line while contested (`:108-114`); a holder on its
connector keeps the grant forever (`:115`), a nose past the stop line keeps it (`:113`). No check of the
connector path itself (TASK-033 residue).

### Bubble (`traffic/spawn.rs`, 237 lines)
Spawn candidates filtered by band, stop-line room and same-lane spacing to AI cars by `s` (`:158-193`), then an
avian chassis box (Character + Vehicle, `:194-216`). A car passing on another lane is not in `on_lane`.

### Police cars
- `police/car_route.rs` (659 lines) `drive_police_cars`: A* lane route, `follow()` target, IDM on an avian
  slab cast that DOES see kinematic traffic (`:572-598`), `pull_over` (`:253-284`, World + Vehicle box
  `pull_over` m right). Chase drives straight at the player. Police are dynamic bodies under `Autopilot`.
- `police/car_dispatch.rs` (228 lines): candidates on hidden lane points of `car.spawn_ring` heading toward the
  last known position, `pick_spawn` nearest/spread, and while pursuing, `approach_clear` (`car_route.rs:105-144`)
  rejects any point with AI traffic on its route to the player (the TASK-016 "cannot pass traffic" workaround).
- `police/cars.rs` (716) state table; `PoliceCar.state` in Respond/Chase is what the task calls "sirens on".
- `police/mod.rs` (643): `EscalationConfig`/`PoliceCarConfig`, `PoliceSystems` set after Population and Wanted.

### Schedule
`TrafficSystems::{Hijack, Bail, Drive, Bubble}` in `NpcSystems` (`traffic/mod.rs:223-238`); Bail is before
`WantedSystems`, `PoliceSystems` after it; `TrafficSystems::Drive` and `PoliceSystems` are not ordered against
each other. `VehicleSystems::Drive` runs after both. Physics runs in `FixedPostUpdate`, so `Position` is
constant through `FixedUpdate`.

### Road facts (`assets/world/city.ron`, `vehicle/sedan.ron`)
Lane 3.25 m, car 2.4 x 4.08 m (half 1.2 x 2.04), street 1 lane per direction (in-lane slack (3.25-2.4)/2 =
0.425 m per side; one lane right of the traffic lane is the sidewalk, World layer, 0.15 m curb), avenue 2 per
direction with the curb lane holding parked cars (curb lane centre 3.25 m right of the inner lane). Only slot-0
lanes are in `TrafficGraph`. `hold_speed` 0.5 m/s.

### Evidence mined for this plan (probe `scratch/mine_stands.py`, output `scratch/mine_stands.txt`)
In s1_tourist, s7_tourist, s7_reckless and s42_reckless every car that stood > 60 s did so at the END of the
session, clustered at one junction 9-25 m from a player standing on foot, with 3-6 `Dynamic` traffic cars
alive. Stands are on several approaches, some with a `waiting` stamp. The s7 screenshot
`s7_tourist/shots/0116_0562s_auto.jpg` shows a traffic car turned ~60 deg across the road with a civilian
pressed against it and the player on the asphalt. The causes are therefore mixed (character in the lane,
bumped Dynamic car, body in the box); stage 1 attributes them.

A concrete, code-derived mechanism for "a Dynamic car never drives on": its forward slab is cast from its
ACTUAL position along the path tangent. A car shoved > 0.425 m toward the curb on a street has its 2.4 m slab
over the sidewalk (> 0.85 m on an avenue: over the parked-car lane), so a civilian standing on the sidewalk or
a parked car within 25 m holds it (IDM obstacle), and at 0 speed the pure-pursuit autopilot never re-centres
it. This is hypothesis (b2) for stage 1.

### Research (cited)
- SUMO opposite-direction overtaking (https://sumo.dlr.de/docs/Simulation/OppositeDirectionDriving.html):
  commits only after checking oncoming vehicles against the assumed overtaking duration and free space at the
  end of the column, look-ahead capped at 150 m; only straight priority links. We copy the "commit only with
  the whole manoeuvre clear" rule and pass only standing bodies (short, bounded duration).
- SUMO bluelight device (https://sumo.dlr.de/docs/Simulation/Emergency.html): traffic within a reaction
  distance (default 25 m) moves laterally inside its lane to form a rescue lane, makes no lane changes while
  doing so, and resumes its previous lateral alignment after the emergency vehicle passed. Our yield is the
  same shape: in-lane curb shift (or a free curb lane on an avenue), stop, resume on pass or timeout.
- SUMO sublane model (https://sumo.dlr.de/docs/Simulation/SublaneModel.html): lateral movement as a
  continuous, rate-limited offset rather than a discrete lane jump. Our kinematic `lateral` state is this idea.
- Hysteresis against flip-flop: TASK-016 lesson (1132 dismount/reboard flips); the recovery uses a wider
  skin/longer horizon and a hold time than the switch (Schmitt-trigger shape).

---

## 2. Approach

### 2.1 One shared occupancy (new domain module `crates/gta_sim/src/occupancy/`)
A `RoadOccupancy` resource rebuilt once per fixed tick by `snapshot_road` (set `OccupancySystems`, in
`NpcSystems`, after `TrafficSystems::Hijack`/`Bail`, before `TrafficSystems::Drive`, `TrafficSystems::Bubble`
and `PoliceSystems`; consumers order themselves after it). It holds one `RoadBody` per `Vehicle` and per
`Character` within `bubble.in_view.despawn + look_ahead` (90 + 60 = 150 m, derived from traffic data) of the
player (all bodies when there is no player):

```rust
pub enum BodyKind { OnPathTraffic, Traffic, Vehicle, Character }
pub enum Footprint { Rect(FlatRect), Circle { centre: Vec2, radius: f32 } }
pub struct RoadBody {
    pub entity: Entity, pub kind: BodyKind, pub shape: Footprint,
    pub velocity: Vec2, pub dynamic: bool, pub standing: f32 /* s at <= hold_speed */,
    pub siren: bool, pub claim: Option<FlatRect> /* committed pass envelope */,
}
```
- `OnPathTraffic`: a Kinematic or Bailing kinematic AI car with `lateral == 0` and no manoeuvre (the cars the
  path `Occupancy`/`leader()` already covers). `Traffic`: every other AI car (off-path kinematic, Dynamic).
  `Vehicle`: every other vehicle (abandoned/taken traffic, parked, the player's, police). `Character`: capsule
  circle (`LocomotionConfig.capsule_radius`).
- Vehicle footprint = `FlatRect::of(position, rotation, (half.x, half.z))` from `VehicleConfig`.
- `standing` persists across ticks in a `HashMap<Entity, f32>` inside the resource, pruned each rebuild.
- `siren = PoliceCar.state ∈ {Respond, Chase}` through `police::sirens_on(state)` (sim-only; the client audio
  pick stays untouched).
- Claims are set by the traffic drive for passing cars (`set_claim(entity, rect)` at the end of
  `advance_traffic`, so the spawner in `Bubble` sees this tick's commits).

Query API (`occupancy/query.rs`), the ONE entry every migrated consumer calls:
```rust
pub struct Strip { pub origin: Vec2, pub dir: Vec2, pub length: f32, pub half_width: f32 }
pub struct Hit { pub entity: Entity, pub gap: f32, pub speed_along: f32, pub standing: f32,
                 pub kind: BodyKind, pub claim: bool }
impl RoadOccupancy {
    /// Nearest body (or opposite-direction claim) inside the strip, gap along `dir` from `origin`.
    pub fn first_along(&self, strip: &Strip, skip: impl Fn(&RoadBody) -> bool) -> Option<Hit>;
    /// First body or claim overlapping `rect` (flat SAT), except `skip`.
    pub fn blocked(&self, rect: &FlatRect, skip: impl Fn(&RoadBody) -> bool) -> Option<Entity>;
    pub fn body(&self, e: Entity) -> Option<&RoadBody>;
    pub fn sirens(&self) -> impl Iterator<Item = &RoadBody>;
}
/// The static road edge: a chassis box at `rect` is clear of the World layer (raised sidewalk, walls).
pub fn world_clear(spatial: &SpatialQuery, rect: &FlatRect, bottom: f32, top: f32) -> bool;
```
`first_along` geometry: rect bodies are clipped to the strip's lateral band (Sutherland-Hodgman on the two
band lines) and the gap is the smallest along-coordinate of the clipped polygon; circles: `cx - r` when
`|cy| <= hw`, else `cx - sqrt(r^2 - (|cy|-hw)^2)`. Claims are returned only when `claim.dir · strip.dir < 0`
(opposite direction) and count as standing (speed 0). Cost: brute force with an AABB prefilter over ~100
bodies; a uniform grid is the named fallback if `traffic_bench` moves by more than 0.5 ms.

Why a snapshot resource and not a SystemParam over avian `SpatialQuery`: avian cannot carry standing time,
claims or the on-path rule, and a `Query<&LinearVelocity>` param conflicts with `advance_traffic`'s mutable car
query (the reason `speed_of`/`others` exist today). The static World stays in avian (`world_clear`).

### 2.2 Kinematic lateral state (one law for rejoin, pass and yield)
`TrafficCar` gains `lateral: f32` (m, + = right of the path tangent) and `manoeuvre: Manoeuvre`:
```rust
pub enum Manoeuvre {
    None,
    Rejoin,                                                   // back onto the path after a recovery
    Pass { obstacle: Entity, offset: f32, merge_s: f32, end_s: f32 }, // around a standing body
    Yield { siren: Entity, since: u64, offset: f32 },         // pulled over for a siren car
}
```
plus `calm: f32` (seconds the recovery conditions have held, Dynamic cars only). Target lateral: `Pass` →
`offset` until the car's rear passes `merge_s`, then 0; `Yield` → `offset`; else 0. Per tick
`lateral += clamp(target - lateral, ±(lateral.rate_at_rest + lateral.slope · v) · dt)`. Kinematic pose =
`pose(seg, s) + right(s) · lateral`; yaw target = path yaw + `atan(rate / max(v, 1 m/s))`, reached at most at
`lateral.yaw_rate_deg` while `lateral != 0` or the car is `Rejoin` (the unlimited one-tick yaw of on-path cars
is unchanged, so every existing traffic gate keeps its behaviour). The angular velocity is the scaled axis of
`Quat::from_rotation_y(target_yaw) * rotation.inverse()` over `dt`, clamped, which also levels a recovered
car's residual roll/pitch. Dynamic cars use the same target lateral in their autopilot target
(`pose + right · target_lateral`), so a car switched mid-pass finishes the pass physically. On connectors the
lateral decays linearly over the connector length (`l0 · (1 - s/len)`) so the path the junction checked is the
path driven (2.6 D4).

### 2.3 Traffic sensing (A1): two strips
Every AI car senses with `first_along` on two strips from its nose along the path tangent:
- strip T at its TARGET lateral, length `sense_distance` (lanes) / `turn_sense_distance` (connectors);
- strip C at its CURRENT lateral (kinematic: `lateral`; Dynamic: signed re-projection offset), length
  `min(reach, v · |target - lateral| / rate + min_gap)` (the distance the lateral move still needs).
Skip rule: self; `OnPathTraffic` only when the sensing car has `target == lateral == 0` (today's behaviour,
so a straight strip into a junction box never brakes a queue head for cars on non-conflicting connectors); a
manoeuvring car sees on-path traffic too (a passer sees oncoming cars). `Traffic`, `Vehicle`, `Character` and
opposite claims are always seen. For an on-path car strip C == strip T == today's cast. The strip starts at
the nose, so a walker at the flank still does not hold a car (`traffic_pedestrian.rs`).
This fixes hypothesis (b2): an offset Dynamic car senses its target corridor (lane centre) and drives back
instead of standing behind sidewalk walkers.

### 2.4 Bumped-car recovery (B) — `traffic/recover.rs`
A `Dynamic` car accumulates `calm += dt` while ALL hold, else `calm = 0`:
upright (`(rot·Y).y >= cos(recover.max_tilt_deg)`), not lost (`reproject`), at rest (speed <= `hold_speed`),
and no dynamic body in the occupancy whose relative sweep over `recover.horizon_seconds` hits the footprint
grown by `recover.skin` (the switch predicate with a wider skin and longer horizon: the hysteresis band; config
law `recover.skin > switch.skin`, `recover.horizon_seconds >= switch.horizon_seconds`).
At `calm >= recover.seconds`: `RigidBody::Kinematic`, remove `Autopilot`, `DriveIntent`, `SleepingDisabled`
(`try_insert`/`try_remove`), `mode = Kinematic`, `lateral` = measured offset, `manoeuvre = Rejoin` (cleared when
`|lateral| < 1 cm` and yaw within 1 deg). A Dynamic car that stood `recover.give_up_seconds` without
recovering becomes `Bailing { attack: None, shooter: None }` (the existing wreck path: the driver gets out, the
car is abandoned, then passable by C). Lost/upside down stay on the existing abandon rule. A car switched
mid-pass keeps its `Pass` target (2.2).

### 2.5 Go-around (C) — `traffic/pass.rs`
Passable: the strip-T hit of a car with no manoeuvre, `gap <= pass.trigger_gap`, kind `Vehicle` or
`Character` or `Traffic` (Dynamic only), standing >= `pass.vehicle_seconds` / `pass.character_seconds`.
Never an `OnPathTraffic` car (queues at stop lines are not passed; no overtaking of moving traffic).
Only the car whose first hit is the obstacle evaluates (its followers follow it through `leader()`).

Commit, per side in order curb (+) then oncoming (-):
1. `offset = ±max(w, need)` where `w` is the lane's `left_gap` (distance to the antiparallel inner lane,
   derived at graph build, 2.8) and `need` = obstacle's lateral extent on that side + `pass.clearance` +
   half width; infeasible if `max(w, need) > w + (w/2 - half.x)` (would leave the road; law from geometry).
2. Merge point `merge_s` = obstacle front (max along-coordinate of its footprint) + `pass.clearance` +
   `half_length`; `end_s = merge_s + half_length + w / lateral.slope_at(pass.speed)`.
3. Claim rect: from the car's rear to `end_s + half_length`, centred at `offset`, half width `half.x +
   pass.clearance`. It must be `blocked(...) == None` (bodies AND claims), skipping same-direction AI passers
   ahead (a platoon behind a passer is allowed; it follows it through `leader()`).
4. Oncoming side only: the stretch beyond the claim, length `v0² / (2·b) + s0` of the opposite lane's v0 (the
   comfortable-stop distance, derived from IDM data, no new number), holds no vehicle.
5. Curb side only: `world_clear` over the claim rect (a street's curb side is the sidewalk and fails).
6. If `end_s <= lane.stop - half_length` the pass merges back on the lane; otherwise the passer keeps the
   offset to the lane end and enters the junction with an offset entry (2.6 D4).
On commit: `manoeuvre = Pass`, v0 capped at `pass.speed`, claim published. The claim shrinks with the car and
disappears after `end_s`. Oncoming cars stop for it (they see opposite claims), the spawner and grants into the
opposite lane see it. Characters are in the claim test, so the manoeuvre never runs anybody over.
Named rescope fallback (orchestrator): if stage 4 fails twice with a pass-through or head-on class, replace the
oncoming side by "a car stuck behind a standing obstacle for > `pass.stuck_despawn_seconds` while off frame
despawns (the bubble respawns elsewhere); in view it passes only via the curb side".

### 2.6 Junction box (D) — `traffic/box_rules.rs`, small edits in `junction.rs`
- D1 grants see bodies on the path: a grant also needs `connector_clear(c)`: no `Vehicle`, `Traffic` or claim
  overlaps the connector polyline swept to `half.x + conflict_margin/2` (segment rects), skipping AI cars that
  hold grants at this node (the conflict table covers them) and characters (walkers are the strips' job; adding
  them to grants is the TASK-033 crosswalk wait class). `lane_start_free` moves onto `occupancy.blocked`
  (Vehicle, Traffic, claims; not characters, not on-path traffic, same as today plus claims).
- D2 lease tail: a holder on its connector or with its nose past the stop line that has not moved for
  `reservation_timeout` while contested is demoted: its grant is removed and it becomes a waiter IN PLACE
  (keeps its `waiting` stamp, holds position: a car on a connector without its grant does not advance). It no
  longer counts as a grant holder, so `connector_clear` of the conflicting waiters sees its body geometrically;
  re-grant uses the normal rules plus `connector_clear` of its remaining path.
- D3 re-pick: a queue head whose chosen connector fails `connector_clear` because of a body standing >=
  `pass.vehicle_seconds` picks the first out connector (in `out` order after the current one) whose path is
  clear, keeping its `waiting` stamp (no extra `TrafficRng` draw, so seeds do not drift).
- D4 offset entry: a car at its lane end with `lateral != 0` requests with its actual path (connector points
  shifted by `right · l0 · (1 - s/len)`); its conflict set is computed geometrically (`polyline_distance` <
  `2·half.x + conflict_margin` against every connector of the node, the table's own rule) instead of the table.

### 2.7 Sirens (E)
- E1 `police/siren.rs`: `pub fn sirens_on(state) -> bool` (Respond | Chase).
- E2 yield, `traffic/sirens.rs`: a kinematic AI car on a lane with no manoeuvre yields to a siren body with
  along-offset in `[-sirens.yield_distance, -half_length]` behind it, lateral offset <= `w` from its path and
  velocity·tangent > 0. Offset: avenue `+w` when the curb-lane rect passes `blocked` + `world_clear`, else the
  in-lane slack `+(w/2 - half.x)`; IDM gets a virtual standing obstacle at `v²/(2b) + s0`. Resume (target 0,
  IDM free) when the siren car's rear is past the car's nose or after `sirens.timeout_seconds`; after a timeout
  the car ignores sirens for another `timeout_seconds` (hysteresis). `Leave` cars: sirens off, nobody yields.
- E3 any lane, `police/siren.rs` called from `drive_police_cars` Respond branch (2-3 lines there): candidate
  lateral offsets `[0] ∪ sirens.lane_offsets` (in lane widths `w`, e.g. `[-0.7, -1.0, 1.0]`); each gets a strip
  of `traffic.sense_distance` via `first_along` (plus `world_clear` for the curb side); choose the farthest
  clear strip, switch only after `sirens.lane_hold_seconds` in the current choice and when the gain exceeds
  `sirens.lane_gain` m. The chosen offset is added to the autopilot target like `pull_over`. Police stay dynamic;
  oncoming kinematic cars see them (they are `Vehicle` bodies in every strip).
- E4 spawn sectors, `police/spawn_sector.rs` called from `dispatch_police_cars` while pursuing: bearing of a
  candidate relative to the player's heading: ahead `<= ahead_deg`, behind `> behind_deg`, beside between.
  Next sector = largest `share·(n+1) - count` (quota, deterministic, no RNG draw); fall back to the next sector
  by deficit when it has no hidden free candidate. Counts live in `PoliceDispatcher.sector_spawns: [u32; 3]`
  (reflected for QA). The hidden rule and per-star caps are untouched. `approach_clear` and `lane_costs_to`
  (and the `approach_clear_rows` unit test) are removed: they become dead code (decision logged; G6 carries it).

### 2.8 Graph helper
`TrafficLane` gains `left_gap: Option<f32>`: distance to the nearest antiparallel lane (dot < -0.99) whose line
lies to the left within `(2·half.x, 4·half.x]` and overlaps longitudinally (3.25 in the city; synthetic floors
get it when they build a two-way road). Computed in `TrafficGraph::new` by a function in `traffic/lanes.rs`
(`graph.rs` grows by < 10 lines). No `CityParams` dependency, so floors work.

---

## 3. Steps (staged; every stage ends green: `cargo test -p gta_sim -p citygen`, `cargo clippy --workspace
--all-targets -- -D warnings`, `cargo test -p gta_like --bin gta_like`, `python tools/qa/tree_check.py`)

Stop rule for every stage: if a stage fails twice with the same failure class (same gate, same mechanism),
stop patching and write `REDESIGN_NOTE.md` in the task dir (fantasy, missing shared rule, cheat, rescope) for
the orchestrator. No third patch.

### Stage 1 — causal evidence RED, tooling (no production code)
1.1 `crates/gta_sim/tests/traffic_support/mod.rs`: add the G1 oracle `Footprints` — independent flat SAT
   (its own 4-axis projection code, NOT `swept_rect_hits_rect` or occupancy code) over every `Vehicle`
   `Position`/`Rotation` within 150 m of the player, pairs with at least one `RigidBody::Kinematic`; per-pair
   tolerance derived from contact margins: kinematic-kinematic 0.02 m (the 1 cm per side shrink precedent of
   `obb_overlap`, kinematic pairs get no solver response); kinematic-dynamic `SolverConfig.max_overlap_solve_speed
   · dt` read from the resource (4.0 / 64 = 0.0625 m: the most overlap the solver may leave for one step).
   `record(&mut self, app, tick)` collects violations; `assert_clean(label)` panics with all of them.
   Also a `StandClock` (per-entity stand durations at < 0.5 m/s, mode-tagged: Dynamic stands tracked
   separately for the G3 "no Dynamic stands > 30 s" clause) reused by all city gates.
1.2 New `crates/gta_sim/tests/traffic_causes.rs` (production city, stationary player OFF the carriageway on
   the sidewalk, facing the scene with `chase_view`, collect all violations). Rows, each `#[ignore = "TASK-032
   stage N"]` once RED is recorded, with the RED numbers in the stage summary:
   - (a) abandoned player car: `vehicle_support::spawn_car` mid-lane on a street lane >= 60 m before its stop
     line, seeds 1 and 7; 120 s; RED = an AI car behind it stands > 30 s.
   - (b) bumped car left Dynamic, variants mined from the sessions: (b1) rear nudge at 2 m/s by a kicked
     player car on a free lane; (b2) the same car shoved 0.6 m (street) / 1.0 m (avenue) toward the curb with a
     dummy on the sidewalk 10 m ahead (street) or a parked car ahead (avenue); (b3) a dummy pressed at its
     bumper; (b4) yawed 45 deg within `lost`. RED = still `Dynamic` and standing after 60 s.
   - (c) a character (`spawn_dummy`) standing in a street lane: RED = the car behind stands > 30 s.
   Any cause that does not reproduce in any variant is dropped from scope and reported to the orchestrator
   (amendment 1); if all of (b) is green on current code, B shrinks to the (b)-rows that went RED, and G3 rows
   without a RED cause are reported, not built.
1.3 `tools/qa/scenarios/t15.py`: `--seed` argument (default 1), `SEED` global replaced by the argument, the
   seed written into `summary.json`. Script-only.
1.4 R1 script (QA-owned, prepared here): `maw/tasks/in_progress/TASK-032/scratch/tools/repro_abandoned_car.py`,
   copy of the TASK-031 script with `sys.path` pointing to `maw/tasks/done/TASK-031/scratch/tools`, the player
   walks to a fixed sidewalk point OFF the carriageway (same point in leave and control; within 45 m of the
   junction, in view), per-car stand durations logged (max per entity, as in `scratch/mine_stands.py`),
   civilians within 8 m of the car reported. Baseline runs on current code: leave=1 and leave=0, seed 1, one each;
   t15 seeds 1, 2, 3 one run each (baseline pressure count). Outputs under `scratch/baseline/`.
Check: `cargo test -p gta_sim --test traffic_causes -- --ignored` shows each reproduced row RED; normal suite green.

### Stage 2 — shared occupancy, lateral state, migrated sensing/spawner/lane start (G1, G8)
2.1 New `crates/gta_sim/src/occupancy/mod.rs` (`OccupancyPlugin`, `OccupancySystems`, `RoadOccupancy`,
   `RoadBody`, `BodyKind`, `Footprint`, `snapshot_road`) and `occupancy/query.rs` (`Strip`, `Hit`,
   `first_along`, `blocked`, `world_clear`, strip clipping). `lib.rs`: `pub mod occupancy;` and add
   `OccupancyPlugin` to the `(VehiclePlugin, TrafficPlugin)` tuple in `compose_sim`. `snapshot_road` reads
   `Query<(Entity, &Position, &Rotation, &LinearVelocity, &RigidBody, Option<&TrafficCar>, Option<&PoliceCar>,
   Has<Character>, Has<Vehicle>)>` (skip entities with neither; skip `Dead` characters? No: a corpse on the
   road is a standing character, keep it), `Res<VehicleConfig>`, `Res<LocomotionConfig>`, `Res<TrafficConfig>`,
   `Res<Time<Fixed>>`, player position. Register `RoadOccupancy` with `init_resource`; it is not reflected
   (QA reads `TrafficCar.manoeuvre`).
2.2 `traffic/mod.rs`: `TrafficCar` fields `lateral`, `manoeuvre`, `calm` (+ `Manoeuvre` enum, `register_type`);
   `configure_sets`: `TrafficSystems::Drive.after(OccupancySystems)`, `TrafficSystems::Bubble.after(...)`;
   `police/mod.rs`: `PoliceSystems.after(OccupancySystems)` (1 line). Update the only literal
   (`spawn.rs:63`). `abandon()` resets `lateral`/`manoeuvre`.
2.3 New `traffic/lateral.rs`: `target_lateral(&TrafficCar, rear_s)`, `step_lateral(...)`, `offset_pose(graph,
   seg, s, lateral)`, the yaw law of 2.2. `drive.rs` kinematic motion (`:447-455`) uses `offset_pose` and the yaw
   law only when `lateral != 0 || manoeuvre != None`; the dynamic autopilot target (`:406-410`) adds
   `right · target_lateral`.
2.4 New `traffic/lanes.rs` + `graph.rs`: `TrafficLane.left_gap` (2.8).
2.5 `drive.rs` sensing (`:295-381`): replace the avian slab cast with two `first_along` strips (2.3); the
   `kinematic` HashSet, `slab`, `filter`, `speed_of`, `others` for speed go away where unused (`others` stays for
   the bailing shooter lookup). `advance_traffic` takes `Res<RoadOccupancy>` (and `ResMut` for claims from
   stage 4). `TrafficStats.casts` keeps counting sensing calls (liveness of every gate using it).
2.6 `drive.rs` `lane_start_free` (`:266-280`) → `occupancy.blocked` over the same rect, skipping characters
   and `OnPathTraffic`.
2.7 `spawn.rs` (`:194-216`): the avian chassis check becomes `occupancy.blocked` over the chassis rect
   extended forward by `idm.min_gap + v0·T` in the travel direction (bodies, off-path traffic and claims).
   The same-lane `on_lane` spacing stays.
2.8 Gates: `tests/traffic_gridlock.rs` runs the G1 oracle every tick on all four seeds (the "at least 8 seeded
   runs" count includes G2-G6). New `tests/traffic_occupancy.rs` (G8, floors from `traffic_support`, each row
   flip-RED by removing that consumer's occupancy input in code and recorded):
   - sensing (a): a kinematic AI car set to `lateral = -1.4` (named mutation, `Manoeuvre::Yield` with a dummy
     siren entity so it holds) straddling into the path of a second kinematic car on a parallel lane 3.25 m
     away: the second car stops (oracle clean, gap >= s0 - 0.1);
   - sensing (b): an abandoned (`TrafficMode::Abandoned`), a taken, and a police car in the lane: the car stops;
   - sensing (c): a dummy ahead of the bumper holds the car (the existing
     `a_walker_ahead_of_the_bumper_still_holds_the_car` is the precedent; the new row asserts the hit came
     through `first_along`, i.e. RED when characters are dropped from the snapshot);
   - spawner: a traffic floor with a `CameraView`, a car in `Manoeuvre::Pass` whose claim covers a spawn point in
     the band: no car spawns on that point over 64 ticks (liveness: a spawn happens elsewhere / after the claim).
   - junction row is added in stage 5.
Check: existing `traffic_*`, `police_*`, `vehicle_*` gates unchanged-green (on-path behaviour is identical);
`traffic_bench` mean printed and compared with the TASK-017 figure.

### Stage 3 — bumped-car recovery (B, G3)
3.1 New `traffic/recover.rs`: `recover_dynamic(...)` called from `advance_traffic` step 1 for Dynamic snaps
   (after `reproject`), returning `Stay | Recover { lateral } | GiveUp`. The commands (`RigidBody::Kinematic`,
   removals) are applied in the final write-back loop (`drive.rs:505-512`) next to `abandon`.
3.2 `traffic/config.rs`: `RecoverConfig { seconds, skin, horizon_seconds, max_tilt_deg, give_up_seconds }`,
   `LateralConfig { rate_at_rest, slope, yaw_rate_deg }` (lateral also used in stage 2: add it there),
   `validate` rows (positive, tilt in (0, 90), and the hysteresis laws `recover.skin > switch.skin`,
   `recover.horizon_seconds >= switch.horizon_seconds`).
3.3 `assets/traffic/traffic.ron`: `recover: (seconds: 1.5, skin: 0.4, horizon_seconds: 0.5, max_tilt_deg: 10.0,
   give_up_seconds: 10.0)` with a "why" comment (skin/horizon four and five times the switch's, so a body that
   would re-switch within a tick never allows a recovery).
3.4 New `tests/traffic_recovery.rs` (G3; loop floor `loop_lanes` for (a)(c)(d), a two-way street floor helper
   `two_way_street()` in `traffic_support` for (b) passability, all with the G1 oracle):
   (a) nudge at 2 m/s from behind: `Kinematic` and speed > 0 along its lane within `recover.seconds + 1 s` of
   rest; (b) shove beyond `lost.distance`: `Abandoned`, then a following car passes it (needs stage 4: row added
   there); (c) player's car held pressed (throttle into its rear) for 10 s: switches counted from
   `TrafficStats.switches_by_cause` and mode transitions, at most one re-switch per `recover.seconds`; the car
   ends Dynamic-then-Bailing only after `give_up_seconds`; (d) rotation set upside down: `Abandoned`.
   Flip: `recover.seconds` to 1e6 (named) makes (a) RED; removing the skin/horizon margin (recovery on the
   switch's own predicate) makes (c) RED — record both.
3.5 Un-ignore the stage-1 (b) rows that reproduced; they must now be green.
3.6 Config rows in `tests/config_traffic.rs` for every new key (failing side, own keyword; sabotage values
   strictly off the boundary).

### Stage 4 — go-around of standing vehicles and characters (C, G2)
4.1 New `traffic/pass.rs`: `passable(hit, cfg) -> bool`, `plan_pass(occupancy, spatial, graph, snap, hit,
   cfg) -> Option<Manoeuvre>` (2.5 steps 1-6), `claim_rect(...)`, `pass_done(...)`. Called from `advance_traffic`
   after sensing for cars whose strip-T hit is passable; claims written through `ResMut<RoadOccupancy>`.
   v0 during `Pass` = `min(lane v0, pass.speed)`.
4.2 `traffic/config.rs` + `traffic.ron`: `pass: (vehicle_seconds: 3.0, character_seconds: 6.0, trigger_gap: 6.0,
   clearance: 0.5, speed: 6.0)` (character wait longer than a car's, premise amendment 2; trigger gap = s0 + 4 m
   so only the queue head behind the obstacle evaluates). Validation: all > 0, `trigger_gap > idm.min_gap`
   (a threshold at IDM's rest point never fires, TASK-016 lesson), `character_seconds >= vehicle_seconds`.
4.3 New `tests/traffic_go_around.rs` (G2): production city, seeds 1, 2, 7, 42, one street row and one avenue
   row per seed (lanes found per seed: longest lane of the class in the bubble with >= 70 m before the stop
   line; `GATE BROKEN` if none), plus character rows (dummy standing in a street lane, seeds 1 and 7). The
   player stands on the sidewalk facing the obstacle, 20-30 m away. Fixture feeds cars: `spawn_traffic_car`
   at the lane start every 4 s until 8 have queued (named fixture; natural traffic also counts). Asserts
   (collect all): first 8 queued AI cars pass (rear past the obstacle's front) within 60 s of joining the queue;
   no AI car stands > 30 s; 0 `CollisionStart` between a car in `Manoeuvre::Pass` and any `Vehicle`
   (`MessageCursor` as in `traffic_contact.rs`); no Dynamic AI car stands > 30 s; G1 oracle clean. Derive the
   60 s check from the worked example in the stage summary (8 x per-pass time measured on seed 1); tighten if
   the example allows.
   Flip-RED: G2 with `pass.vehicle_seconds` 1e6 (named) is RED; G1 with the claim hidden from
   `first_along` (and with the passer typed `OnPathTraffic`) is RED on at least one G2 row.
4.4 Add G3 (b) row (Abandoned then passed); un-ignore stage-1 (a) and (c) rows.
4.5 Rescope fallback trigger: two failures of the pass-through/head-on class → `REDESIGN_NOTE.md` proposing the
   orchestrator's stuck-despawn cheat for the oncoming side (2.5).

### Stage 5 — junction box (D, G4)
5.1 New `traffic/box_rules.rs`: `connector_clear(occupancy, graph, c, skip)`, `offset_path(graph, c, l0)`,
   `offset_conflicts(graph, node, path)`, `repick(graph, occupancy, lane, current)`. `graph.rs`:
   `polyline_distance` becomes `pub(crate)`.
5.2 `junction.rs`: grants add `connector_clear` (D1); the occupants `retain` (`:95-119`) demotes stale holders on
   connectors / past the stop line (D2) into waiters in place; the waiter `retain` (`:85-94`) accepts
   `Segment::Connector(c)` for its own `c`; the request loop re-picks (D3) and uses offset conflicts for
   `lateral != 0` (D4). `junction::update` gains `&RoadOccupancy`.
5.3 `drive.rs` motion: a car on a connector without its grant holds (`v = 0`, `s` unchanged); lateral decays
   linearly on connectors (2.2).
5.4 New `tests/traffic_junction_box.rs` (G4): production city seeds 1 and 7, the gridlock pose (player at the
   spawn looking north), a non-AI car placed on a free spot of a connector path inside the busiest box in view
   at t = 20 s (`GATE BROKEN` if the spot overlaps a body), 120 s. Asserts: no grant holder that has not moved
   keeps its grant against a conflicting waiter longer than `reservation_timeout + 1 s` (the gridlock stale
   metric extended to holders on connectors and past the line); no AI car on any approach stands > 40 s; G1
   clean. Flip: D1 removed → RED (a grant into the blocked path, contact or oracle); D2 removed → stale-grant RED.
   G8 junction row in `traffic_occupancy.rs` (plus floor from `traffic_intersection.rs`): a parked car on a
   connector path; that connector is never granted while it stands (flip: `connector_clear` input removed).
5.5 `traffic_intersection.rs` existing rows stay green (they are the conflict-table regression).

### Stage 6 — sirens, any lane, spawn sectors (E, G5-G7)
6.1 New `police/siren.rs`: `sirens_on`, `SirenConfig { lane_offsets: Vec<f32>, lane_hold_seconds, lane_gain }`,
   `choose_lane(occupancy, spatial, ...) -> f32`; `police/mod.rs` `PoliceCarConfig.sirens: SirenConfig` (+2 lines,
   validation delegated to `SirenConfig::validate`); `car_route.rs` Respond branch adds the chosen offset to the
   target (stays < 700 lines); `PoliceCar` needs the lane-hold state: a new small component `SirenLane { offset,
   held }` required by `PoliceCar` (keeps `cars.rs` under its limit).
6.2 New `traffic/sirens.rs` (E2) called from `advance_traffic` before pass evaluation; `traffic.ron`
   `sirens: (yield_distance: 40.0, timeout_seconds: 8.0)` (`yield_distance` >= 0, 0 = never, the `pull_over`
   precedent, needed for the G6 flip).
6.3 New `police/spawn_sector.rs`: `SectorConfig { ahead: f32, beside: f32, behind: f32, ahead_deg: f32,
   behind_deg: f32 }` (shares sum to 1 within 1e-3, `0 < ahead_deg < behind_deg < 180`) in
   `escalation.ron` `car.spawn_sectors: (ahead: 0.3, beside: 0.3, behind: 0.4, ahead_deg: 45.0, behind_deg:
   135.0)`; `car.sirens: (lane_offsets: [-0.7, -1.0, 1.0], lane_hold_seconds: 1.0, lane_gain: 5.0)` with comments
   (-0.7 w: 0.35 m clear of a street car yielded by its 0.425 m slack). `car_dispatch.rs`: sector pick while
   pursuing, `approach_clear` removed; `car_route.rs`: `approach_clear`, `lane_costs_to`, `step_cost`,
   `approach_clear_rows` removed; `police_car_floor.rs::police_car_never_spawns_behind_a_traffic_queue`
   replaced by a G6 floor row (a police car spawned behind the queue closes in). `PoliceDispatcher`
   gains `sector_spawns: [u32; 3]`.
6.4 Gates:
   - `tests/police_sirens.rs` (G5): production city seed 1 (and 7), a street lane >= 90 m, 6 AI cars queued
     through `spawn_traffic_car` (moving at street v0 or standing behind the stop line; both rows), a police
     car with the production component set (as `police_car_floor.rs::spawn_police_car`) 50 m behind in
     `Respond` toward the player standing on the sidewalk beyond the queue, dispatcher off (`no_police_cars`,
     named). Rows: every queued car whose nose is within `yield_distance` shifts > 0.3 m toward the curb and
     stops; the police car passes all 6; each yielded car is Kinematic, back to `lateral` 0 and moving within
     `timeout_seconds` after the police rear passes its nose; `Leave` row: no car's `manoeuvre` becomes Yield.
     G1 clean.
   - `tests/police_close_in.rs` (G6): seeds 1..=10, the player's car cruising a street at 12 m/s (the bench's
     `cruise`), 3 AI cars queued between it and a police car 40 m behind, dispatcher off (named); a police unit
     within 18 m within 25 s in >= 8/10 seeds. Flip (named): `sirens.yield_distance = 0` and
     `lane_offsets = []` → RED (record the count). G1 clean.
   - `tests/police_spawn_sectors.rs` (G7): seed 1, 2 and 3 stars, the player driving a loop at 12 m/s; the test
     despawns each police car one tick after it appears (named mutation, to sample the dispatcher >= 12 times).
     One row per sector: count within ±1 of `share · N` minus reported fallbacks (quota rule worked example in
     the summary); every spawn hidden (corners off the view cone or occluded) and overlapping no body;
     `dispatcher.cars <= row.cars` every tick.
6.5 `tests/config_police.rs` rows for `sirens`, `spawn_sectors`.

### Stage 7 — docs, data, runtime QA, follow-up task
7.1 `docs/design/GDD.md` §5.2: one line each for the go-around of standing bodies (and characters after a
   longer wait), bumped-car recovery, siren yield; §5.3: one line each for any-lane with sirens and spawn
   ahead/beside by sector shares. Written in Russian like the GDD.
7.2 `docs/architecture/traffic.md`: occupancy section (snapshot, kinds, strip/overlap API, claims), modes incl.
   the switch and its return, the lateral law, the tick order, the pass, junction D1-D4, sirens; a list of
   migrated consumers (traffic sensing, junction grants/lane start, spawner, police lane choice) and those
   pending in TASK-036 (NPC walk avoidance, fire line `tactics::nearby_cars`/`car_blocks`, sight
   `perception::sight_blocked`, conflict-point reservation). Remove the "cannot pass traffic" paragraph.
7.3 `maw/tasks/pending/TASK-036/task.md` via the `maw-tasks` format, blocked by TASK-032 (orchestrator runs
   `maw-tasks`; the implementer only drafts the text in the stage summary if the skill is not available).
7.4 Runtime QA (first-class): R1 with the stage-1 script, seed 1, leave=1 and leave=0: no traffic car within
   45 m of the junction stands > 30 s in the leave run, control keeps 0 over 20 s; civilians within 8 m of the
   car reported. t15 seeds 1, 2, 3, two runs each: pressure in >= 5/6, hijack and "<= 2 active police cars at
   2 stars" asserts green. Owner-run notes: recovery look, pull-over, pass glide, chase aggression.
7.5 G9: full suite, both benches under `MEAN_LIMIT`, clippy, client tests, tree_check, and the 5 CI workflows on
   the merge commit (orchestrator).

---

## 4. Risk areas

1. **Head-on deadlock in the oncoming lane** (TASK-016 class). Mitigated by commit-only-when-clear, claims seen by
   oncoming strips, spawner and grants, and the comfortable-stop extension. A body that enters the claim after
   commit (the player, police, a walker) can still hold the passer mid-lane; oncoming cars wait at the claim end.
   Watch the "no AI car stands > 30 s" rows. Fallback named (2.5).
2. **Junction regressions** from the sensing change: the on-path skip keeps today's behaviour for non-manoeuvring
   cars; `traffic_gridlock` 40 s bound on four seeds is the regression net. D2 demotion changes lease flow: the
   stale-grant metric must stay under `reservation_timeout + 1 s`.
3. **Offset junction entry (D4)** is the most novel geometry: conflicts computed on the actual offset path;
   a sign error in `right(s)` puts the path on the wrong side. Walk three worked directional examples (north,
   east, a left turn) in the stage-5 summary before the gate.
4. **Recovery flip-flop** (TASK-016 lesson): hysteresis is in data and validated as a law; G3 (c) counts flips.
5. **Snap visuals**: recovery from up to 60 deg rotates at `yaw_rate_deg` in place at rest; the lateral glide at
   rest. Owner-judged; not gated.
6. **Frame budget**: brute-force occupancy queries (~24 cars x 2-4 strips x ~100 bodies). `traffic_bench` and
   `police_bench` under `MEAN_LIMIT`; grid index is the fallback.
7. **Determinism of seeded gates**: D3 re-pick and sector quota draw no RNG; the pass and yield draw none. Any
   new `TrafficRng` draw would shift every seed's traffic (TASK-010 lesson).
8. **Occupancy radius**: bodies beyond 150 m of the player are absent; traffic and police live inside it. A
   headless test without a player sees everything.
9. **API renames in Bevy/avian**: no new engine APIs are needed beyond those already used in these files
   (`SpatialQuery::shape_intersections`, `try_insert/try_remove`, `RigidBody::Kinematic`, `SleepingDisabled`,
   `MessageReader<CollisionStart>`, `SolverConfig` resource: `avian3d-0.7.0/src/dynamics/solver/plugin.rs:214-216`,
   default `max_overlap_solve_speed: 4.0` at `:296`). No new crates.
10. **A new `Res<RoadOccupancy>` on `advance_traffic`, `drive_police_cars`, `spawn_traffic`,
   `dispatch_police_cars`**: every harness goes through the production plugins (`composed_app`), so the
   resource exists; grep test files for any hand-registered system before merging (bevy-ecs invariant).
11. **Files near limits**: new code goes to `occupancy/*`, `traffic/{lateral,lanes,recover,pass,box_rules,sirens}.rs`,
   `police/{siren,spawn_sector}.rs`. `drive.rs` must stay < 750 (expected ~560 after calls replace the cast);
   `car_route.rs` shrinks (approach_clear removed); `police/mod.rs` +3 lines; `graph.rs` < 10 lines.

## 5. Open questions
None blocking. Resolved here as planner decisions (logged in `log.jsonl`): the snapshot occupancy form, the pass
claim, the two-strip sensing, removal of `approach_clear`, the offset junction entry with the stuck-despawn cheat
as the named fallback. If the orchestrator rejects the claim as "an oncoming reservation in disguise", the
alternative is footprint-only passers plus the fallback cheat for the oncoming side from the start.
