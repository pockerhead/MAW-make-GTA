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

## Modes

`TrafficCar.mode`: `Kinematic` (moved by velocity along the path), `Dynamic` (after a contact: the
shared pure-pursuit `vehicle::Autopilot` drives the car along its path), `Bailing` (brakes; the driver
gets out when it stands), `Taken` (the player drives it), `Abandoned` (a dynamic body with no AI under
the bubble rule). The driver is data: a civilian entity exists only once thrown out (hijack) or bailed
out (cabin shot, wreck).

## One tick (`advance_traffic`, entity order)

1. Dynamic cars are re-projected onto their path; a car farther than `lost.distance`, heading off by
   more than `lost.angle_deg` or upside down is abandoned.
2. A wreck (car health 0) bails out.
3. Occupancy per segment; a car on a connector also counts on its source lane past the lane end.
4. Intersections (`junction.rs`): a car within `v^2/2b + 2 s0 + half length` of its stop line picks a
   random out connector and queues (tick stamp). Grants go first come first served when no granted and
   no earlier queued connector conflicts and the destination lane has room for the cars already heading
   into it plus this one ("don't block the box"): behind the last AI car there, and no other vehicle
   (abandoned, taken, police) overlaps that stretch of the lane (a chassis-high box query). A car that
   waits only for that room does not block later conflicting connectors (one car behind a standing one
   would lock the node). A grant is held until the car's rear leaves the connector. (The PLAN's `v^2/2b + s0 + half length` equals the IDM rest position: queue heads crept to
   the line before asking.)
5. IDM (`idm.rs`, ballistic update with stops) on the nearest of: the leader along the path within
   `look_ahead`, the stop line when not granted, a forward shape cast (`sense_distance` on lanes,
   `turn_sense_distance` on connectors) that skips kinematic traffic (it is in the occupancy). The cast
   starts at the nose: a walker pressed against the flank is not in the way (a car and a walker used to
   wait on each other inside intersections for 3-13 s). A chosen turn caps the lane speed so the
   connector is reached at `turn_speed`.
6. Kinematic motion: `LinearVelocity = (pose(s') - position) / dt`, yaw closed loop through
   `AngularVelocity`; a lane end without a grant clamps the car at the stop line. Dynamic cars get an
   autopilot target ahead on the path and a target speed `v + a / (acceleration x speed_gain)` while
   IDM speeds up, so the autopilot opens the throttle by `a / acceleration` (with `v + a·dt` a dynamic car
   crawled from rest at about 0.03 m/s^2); braking keeps `v + a·dt` (any shortfall brakes).
7. A stopped bailing car lets the driver out at the first clear exit (`vehicle::exit_spots`); the civilian
   flees from the shooter about the shot, so its call reports the shooting.

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
from the cars on that lane and need a free chassis box. Abandoned and bailing cars follow the same rule,
so no car entity leaks (`tests/sensor_leak.rs`).

## Hijack, bail-out, cabin

Entering an AI traffic car throws the driver out as a fleeing civilian at the player's feet (about the
entry attack; the theft is recorded by `record_crimes`); an abandoned car has nobody inside. A bullet
inside the cabin zone (`sedan.ron cabin`, body frame) wounds a seated driver by `cabin_driver_share`
(the player), scares a traffic driver (`DriverScared` -> a `Shooting` incident) or does nothing more
(police crews are data).

## Police cars

`dispatch_police_cars` keeps `row.cars` active cars (Respond, Chase, Dismounted) while the row's units
(foot cops plus crews aboard) allow one more cop; crews are `min(crew, units left)` of the row's kinds
(SWAT first). While a lane graph exists the foot dispatcher keeps the seats of the missing cars
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
reserve intersections and wait behind traffic (IDM on a forward cast). Behind a leader they speed up
at the IDM rate through the same autopilot target as dynamic traffic (`vehicle::follow_speed`), not at
`a·dt·gain` (a crawl from rest). They cannot pass traffic: a car spawned behind the queue in the
player's lane follows it at traffic speed (see FIX_SUMMARY round 3, STOP).

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
