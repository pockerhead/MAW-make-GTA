# Traffic and police cars (T15)

How the traffic (`crates/gta_sim/src/traffic/`) and the police cars (`police/cars.rs`, `police/car_route.rs`)
work. Design law: GDD §5.2, §5.3, §6.4. Tuning: `assets/traffic/traffic.ron`, `assets/police/escalation.ron`
(`car` block, `stars[].cars`), `assets/vehicle/sedan.ron` (`cabin`, `autopilot`).

## Lane graph

`TrafficGraph` is built once per city (`OnTransition Loading -> Playing`) from citygen's `LaneGraph`:
only the inner lane of every road edge (slot 0, derived from the lateral offset to the edge line) is kept,
so the avenue curb lanes stay free for parked cars. Connectors are quadratic Béziers (control point where
the two lane lines meet) sampled into `connector_samples` points; the pose tangent comes from the
analytic derivative, so a car's yaw turns smoothly. Two connectors of one intersection conflict when
they share a source or destination lane or their centre lines pass closer than `2 x half width +
conflict_margin`. A lane without an out connector is a build error (the game exits). The seeds 1..8
gate (`tests/traffic_graph.rs`) keeps a car on any connector off every block.

## Road occupancy (`occupancy/`, TASK-032)

One flat snapshot per fixed tick (`snapshot_road`, `OccupancySystems`: after the hijack and bail sets,
before traffic drives or spawns) of every vehicle body and every enabled character within
`in_view.despawn + look_ahead` of the player: a rectangle (vehicles) or a circle (characters, dead ones
included), flat velocity, `standing` (seconds at or below the vehicle `hold_speed`), `dynamic`, and the
siren flag (`police::sirens_on`: Respond or Chase). Seated drivers and the player in a car
(`ColliderDisabled` / `RigidBodyDisabled`) are skipped, like avian skips them. Kinds:

- `OnPathTraffic`: a kinematic AI car on its path line with no manoeuvre (exactly the cars the path
  occupancy of `advance_traffic` holds);
- `Traffic`: any other AI car (off its line: yielding, passing, rejoining; dynamic; bailing);
- `Vehicle`: any other car (abandoned, taken, parked, the player's, police);
- `Character`.

Claims: the road a passer will drive through (`pass::derived_claim`, from the car's rear to the end of
its pass, on the pass side). They are derived from `TrafficCar.manoeuvre` of the cars the AI drives
(a hijacked or abandoned car claims nothing; `clear_ai_state` also resets its fields) in every snapshot and inserted
at once when `advance_traffic` commits a pass, so later cars of the same tick, the spawner and the police
see them. A claim is seen only by strips pointing the other way (oncoming cars stop before it).

Queries (`query.rs`): `first_along(strip, skip)` (the nearest body or opposite claim in a band ahead of an
origin: gap, speed along, standing, kind; a body straddling the origin has gap 0, one behind it is not
seen), `blocked(rect, skip, claims)` (the first body or claim overlapping a rectangle, flat SAT, 1 cm
shrink) and `world_clear` (the static road edge stays an avian box query).

Consumers on it: traffic sensing (two strips, below), the lane-start room check of the junction grants,
the junction box rules (a connector path free of bodies), the traffic spawner (a spawn spot free of
every body and claim), the siren yield and the police lane choice. Pending in TASK-036: NPC walk
avoidance (`navigation::avoid_offset`, `perception::wall_blocked`, walls only), the fire line
(`tactics::nearby_cars` / `car_blocks`), sight (`perception::sight_blocked`), and a conflict-point
junction reservation instead of the whole connector.

## Modes

`TrafficCar.mode`: `Kinematic` (moved by velocity along the path), `Dynamic` (after a contact: the
shared pure-pursuit `vehicle::Autopilot` drives the car along its path), `Bailing` (brakes; the driver
gets out when it stands), `Taken` (the player drives it), `Abandoned` (a dynamic body with no AI under
the bubble rule). The driver is data: a civilian entity exists only once thrown out (hijack) or bailed
out (cabin shot, wreck).

The switch has a way back (`recover.rs`): a `Dynamic` car becomes `Kinematic` again once for
`recover.seconds` it has been upright (tilt under `max_tilt_deg`), within the `lost` thresholds, at rest,
with no dynamic body whose relative sweep over `recover.horizon_seconds` reaches its footprint grown by
`recover.skin` (four and five times the switch's skin and horizon: a body that would switch it again
within a tick never lets it recover), and with its rejoin corridor to the lane line clear (sampled
rectangles 7.5 deg / 0.3 m apart). It then rejoins its line at the lateral rate (`Manoeuvre::Rejoin`).
A car that stood `give_up_seconds` in `Dynamic` without recovering, out of its lane band (half the lane
pitch from the path line) and with nothing on its lane line within twice the jam gap ahead, bails out, or
is abandoned at once when no door is free (a bailing car with no exit would stand forever). A car in a
queue or behind a standing body never gives up, it waits: given up there it would be one more standing
body, the car behind it the next to be bumped (TASK-032 QA, a column of 8 `Abandoned`). A bailing car
claims no road (the snapshot derives claims for `Kinematic`/`Dynamic` cars only).

`TrafficCar.lateral` and `manoeuvre` (`None`, `Rejoin`, `Pass`, `Yield`) move a kinematic car off its line
under one law (`lateral.rs`): rate `rate_at_rest + slope x speed`, the heading turned into the move at
most `yaw_rate_deg` per second; on a connector the offset decays linearly to its end. A car on its line
keeps the one-tick yaw snap.

## One tick (`advance_traffic`, entity order)

Order in `FixedUpdate` (`NpcSystems`): `TrafficSystems::Hijack` and `Bail`, then `OccupancySystems`
(the snapshot), then `TrafficSystems::Drive`, then `Bubble` (stuck cheat, despawn, spawn) and
`PoliceSystems` (after `Drive`: the police lane choice reads the claims `Drive` inserted this tick).

1. Dynamic cars are re-projected onto their path; a car farther than `lost.distance`, heading off by
   more than `lost.angle_deg` or upside down is abandoned; the others may recover (Modes).
2. A wreck (car health 0) bails out.
3. Occupancy per segment; a car on a connector also counts on its source lane past the lane end.
4. Intersections (`junction.rs`): a car within `v^2/2b + 2 s0 + half length` of its stop line picks a
   random out connector and queues (tick stamp). Grants go first come first served when no granted and
   no earlier queued connector conflicts and the destination lane has room for the cars already heading
   into it plus this one ("don't block the box"): behind the last AI car there, and no other vehicle
   (abandoned, taken, police, an off-path AI car) or pass claim overlaps that stretch of the lane (road occupancy). A car that
   waits only for that room does not block later conflicting connectors (one car behind a standing one
   would lock the node). A grant is held until the car's rear leaves the connector. (The PLAN's `v^2/2b + s0 + half length` equals the IDM rest position: queue heads crept to
   the line before asking.)
5. IDM (`idm.rs`, ballistic update with stops) on the nearest of: the leader along the path within
   `look_ahead`, the stop line when not granted, and the road occupancy on two strips from the nose
   (`manoeuvre::sense`): one at the target lateral over `sense_distance` on lanes / `turn_sense_distance`
   on connectors, one at the current lateral over the rest of the sideways move plus twice the jam gap
   (a strip as long as IDM's rest gap would drop the body the car stands behind). A car on its line with
   no manoeuvre skips `OnPathTraffic` (the path occupancy holds those; a straight strip into a box must
   not brake for crossing cars); every other body, and every oncoming claim, is seen. The strips start at
   the nose: a walker pressed against the flank is not in the way (a car and a walker used to wait on
   each other inside intersections for 3-13 s). A chosen turn caps the lane speed so the connector is
   reached at `turn_speed`. Then the manoeuvre step (`manoeuvre::plan`): the siren yield, a published
   pass going out, a new pass, a box pass.
6. Kinematic motion: `LinearVelocity = (pose(s') - position) / dt`, yaw closed loop through
   `AngularVelocity`; a lane end without a grant clamps the car at the stop line. Dynamic cars get an
   autopilot target ahead on the path and a target speed `v + a / (acceleration x speed_gain)` while
   IDM speeds up, so the autopilot opens the throttle by `a / acceleration` (with `v + a·dt` a dynamic car
   crawled from rest at about 0.03 m/s^2); braking keeps `v + a·dt` (any shortfall brakes).
7. A stopped bailing car lets the driver out at the first clear exit (`vehicle::exit_spots`); the civilian
   flees from the shooter about the shot, so its call reports the shooting.

## Going around a standing body (`pass.rs`, TASK-032)

The queue head whose nearest strip hit is within `pass.trigger_gap` and has stood `pass.vehicle_seconds`
(a vehicle, a dynamic AI car) or `pass.character_seconds` (a person) passes it: on the curb lane of an
avenue (when the layout has one, `TrafficLane::curb_lane`, and `world_clear`), else on the opposite inner
lane (`TrafficLane::left_gap`). The claim is published at the commit while the car still stands on its
line (`Pass { go: false }`): oncoming cars not yet inside it stop before it, one already inside drives
out. The car goes when its claim holds no body and no oncoming car beyond the claim end is closer than
its hard-braking stop; a new pass does not commit while an oncoming AI car waits beyond the claim
(passes alternate with the oncoming flow). The nose holds at `hold_s` (behind the obstacle) until the
car is out beside it (`|lateral| >= need`), so its clearance to the obstacle is `pass.clearance`, above
the switch skin. It merges back past the obstacle and the manoeuvre ends on its line. A pass that would
reach the lane end is refused (the plan's straight-connector entry is not built). Moving traffic is
never overtaken.

## Junction box (`box_rules.rs`, `junction.rs`, TASK-032)

A grant also needs the connector path free of bodies the conflict table does not know
(`connector_clear`: a car left in the box, a demoted holder, a claim; walkers stay the strips' job, and
AI cars granted at the node are the conflict table's). A waiter that fails it does not block the node.
A holder whose path a standing non-walker body has blocked for the lease is demoted to a waiter where
it stands. A queue head (or a car at the very start of its connector) whose path a body standing
`pass.vehicle_seconds` blocks takes another exit (no RNG draw). When a body stands on every way out, the
head enters alone with a whole-box grant and goes around it (`plan_box_pass`). A lock that no pass
fits around (two bodies in the box) is left to the stuck cheat (Bubble). Open (TASK-032 R1): a car
left in a box the player watches from within `bubble.stuck_in_view_distance` is neither passed nor
removed and can lock the box; a push-through by the traffic (a braked car cannot be shoved sideways
by the autopilot, and walkers get pinned between the cars) was tried and dropped.

## Sirens (`traffic/sirens.rs`, `police/siren.rs`, TASK-032)

An idle kinematic car on a lane yields to a siren body (Respond or Chase) coming up behind it within
`sirens.yield_distance` (heading along the lane, within a lane pitch) or driving at it: it shifts to the
free curb lane of an avenue, else the 0.425 m slack of its own lane, at most at `pass.speed`, then stops
(IDM on a virtual obstacle at its comfortable stop `v^2/2b`, which ends at 0: a gap ending at the jam gap
is IDM's rest point, and the car crept forever). It does not yield when the siren car cannot get by (no
free curb lane and no opposite lane) or within a car length and a jam gap of its lane start (the siren
car would stand in the box behind it, where it cannot change lanes). It drives on once the siren car is
past or after `sirens.timeout_seconds`, then ignores sirens that long.

A police car with sirens on picks a lateral position on its lane (`SirenLane`, `car.sirens.lane_offsets`
in lane pitches, the opposite lane at -1): leaving its own lane needs `lane_gain` m more clear road
there, coming back only as much as where it is, each after `lane_hold_seconds`. Lanes are compared from
its tail (a car still alongside keeps a lane taken) over `max(sense_distance, v x lane_hold_seconds +
v^2/2b)`. The reference is the nearest lane pointing its way within 1.5 pitches (so a car out in the
opposite lane keeps its own); in a junction box the offset is 0. IDM runs on the occupancy strip of the
chosen lane instead of the heading cast. On a street (6.5 m) a car yielded to each curb leaves 1.7 m, less
than a police car: it passes only where the rows are staggered (G6 numbers in TASK-032
`REDESIGN_NOTE.md`).

## Kinematic -> dynamic switch (`contact.rs`, FixedPostUpdate before the physics step)

A kinematic body pushes with infinite mass and passes through other kinematic bodies (probe in
`maw/tasks/.../TASK-016/scratch/probe_kinematic`). The switch is predictive: every dynamic body in a
broadphase box grown by `switch.reach` is swept over `horizon_seconds` by the relative velocity (sleeping
bodies count at rest) against the car's footprint grown by `skin` (circle vs rectangle for characters,
SAT on the swept hull for cars). A `CollisionStart` with a dynamic body is the backstop. An oncoming car
0.85 m away in the opposite inner lane or a pedestrian crossing 1.2 m in front never switches.

## Bubble (`spawn.rs`)

Vermeij rules: in frame (a chassis top corner or the centre line in the view cone, within
`in_view.despawn`) cars spawn at 70-90 m and go past 90 m; out of frame they spawn at 15-25 m and go past
25 m, never before `offscreen_seconds` out of frame. Cap `max_cars` (taken cars do not count). A spawn
near a lane end starts slow enough to stop at the line; spawn points are spaced by `s0 + v0 T + length`
from the cars on that lane and a chassis free of every body and claim in the road occupancy. Abandoned
and bailing cars follow the same rule, so no car entity leaks (`tests/sensor_leak.rs`).

Stuck cheat (`stuck.rs`, before the bubble despawn): a traffic car that has stood out of frame for
`bubble.stuck_despawn_seconds` (45 s, above the 40 s saturated-junction stand bound, so no liveness gate
is masked) despawns; the bubble refills the street. A car nobody drives (a driverless non-police car, an
abandoned traffic car) standing that long in a junction box or within a car length of it despawns when
it is out of frame, farther than `bubble.stuck_in_view_distance` (40 m) from the player, or only a
corner of it is in frame. A car at a box the player looks at from nearer stays (see the box section).

## Hijack, bail-out, cabin

Entering an AI traffic car throws the driver out as a fleeing civilian at the player's feet (about the
entry attack; the theft is recorded by `record_crimes`); an abandoned car has nobody inside. A bullet
inside the cabin zone (`sedan.ron cabin`, body frame) wounds a seated driver by `cabin_driver_share`
(the player), scares a traffic driver (`DriverScared` -> a `Shooting` incident) or does nothing more
(police crews are data).

## Police cars

`dispatch_police_cars` keeps `row.cars` active cars (Respond, Chase, Dismounted) while the row's units
(foot cops plus crews aboard) allow one more cop; crews are `min(crew, units left)` of the row's kinds
(SWAT first). While the player drives a moving car the spawn points are drawn by sector around his
heading (`car.spawn_sectors`: ahead, beside, behind; the sector most behind its share that still has
points, no random draw; a deliberate cheat). A spawn is hidden when every top corner of the chassis is
outside the view cone or world geometry blocks the ray to every one (rays across the width alone let a
corner of a car seen at an angle show). While a lane graph exists the foot dispatcher keeps the seats of the missing cars
(`(row.cars - active cars) x crew`), else row 5 would fill as 4 cars + 4 foot. Cars spawn only on lanes
heading towards the last known position (without U-turns a car facing away loops a block first). States (`next_car_state`):
Respond (A* over lane ends to the first lane passing within `nearest + goal_margin` of the target; on
the car's own lane only the stretch ahead counts), Chase (a seen driver within `direct_chase_distance`: straight at the player at
`pursuit_speed`), Dismounted (slow and near a player on foot or a stopped driver, at the end of the
route, held up for `blocked_seconds`, or no lane route at all; inside an intersection only once held
up there for `blocked_seconds x junction_factor` (else a car there keeps at least `turn_speed` until it
is out); a car with no route waits where it is instead of driving at the target; for a driver, whatever
the reason, only once his car has stood `stopped_seconds`), Leave (0 stars: wander the lanes), Taken,
Abandoned (nobody aboard and nobody alive outside). A dismounted crew walks back to the nearer door and
re-boards when the player drives away, `PoliceCar::driven_off`: his car has moved (above exit speed) for
`moving_seconds` and is farther than `reboard_distance`. The two windows are the hysteresis that keeps a
stop-and-go driver from making crews hop out and in (QA round 2, seed 3: 7 dismounts, 4 re-boards in 25 s;
now at most one each). After `reboard_timeout_seconds` the stragglers stay foot cops. Crews get out at the doors only, never onto the roof; a cop without a free door stays aboard
and gets out once one clears, and a car nobody can leave stays on the job (Respond) until a door clears. A car stopping from speed steers `pull_over` m to the right when a chassis
there is free of cars, walls and the raised sidewalk (seed 1: rarely possible, 0-0.45 m in practice).
Police cars slow to `turn_speed` before a turn and brake at `idm.max_deceleration`; they do not
reserve intersections. Behind a leader they speed up at the IDM rate through the same autopilot target
as dynamic traffic (`vehicle::follow_speed`), not at `a·dt·gain` (a crawl from rest). With sirens off
they wait behind traffic (IDM on a heading cast); with sirens on traffic yields and they take any lane
(Sirens above), so the TASK-016 spawn filter "no traffic on the approach" is gone.

At 1 star a cop within `arrest.approach_distance` of the left door of the player's car goes to `Arrest`
without needing sight (a car queued behind the player hides him), faces the door and walks to it around
the cars in the way (`tactics::around_cars`: the corner of the first car within the capsule radius of the
straight line, in plain view and shortest to go via; the wall avoidance does not see cars). The same
walk serves an arrest on foot. A cop in `Arrest` within `arrest.distance` of the door of the stopped car
pulls the player out after `pull_out_seconds`, only through a clear left door (the right door or the roof
would put the player out of reach: a break-free star); the arrest then runs on foot. The arresting cops never
block that door (the whole crew goes to `Arrest` and one stood on the exit spot, so the pull retried
forever). A door blocked by anything else holds the pull for `arrest.pull_give_up_seconds`; then the
driver is pulled out at either clear door (never onto the roof) and the attempt is reset, so the foot arrest starts over and never
counts a cop at the far door as a break-free. Cars in the segment from a
shooter to its target block fire like spared bodies (plain clearance, no spread cone, nothing past the
target, never the car the target drives).

## Probe facts (avian3d 0.7.0)

A kinematic body moves exactly by its velocity; kinematic pairs report `CollisionStart` but get no solver
response; `insert(RigidBody::Dynamic)` before `PhysicsSystems::First` acts in the same step; a
`CollisionStart`-driven switch is one step late (the dynamic body stops dead against the kinematic one).
