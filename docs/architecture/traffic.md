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
they share a source or destination lane, their centre lines pass closer than `2 x half width +
conflict_margin`, or the car bodies driven along them touch (rectangles grown by half the margin every
0.2 m, from the nose at the connector start to the rear at its end: the corners of a turning car reach
past its centre line, TASK-038). A lane without an out connector is a build error (the game exits). The
seeds 1..8 gate (`tests/traffic_graph.rs`) keeps a car on any connector off every block, and two cars on
connectors granted together apart (an unmargined oracle every 0.1 m). Right turns conflict with almost
everything at their node: citygen's right-turn connectors are ~2-2.6 m long (radius ~1.7 m), so a 4.08 m
body pivots nearly in place and its nose and rear sweep the neighbouring lanes. The swept rule cut the
connector pairs that may hold grants together from 2340 to 1323 on seed 1 (52-58 % kept on seeds 1..8;
opposite straights unaffected). That is the known cost of swept conflicts; a conflict-point reservation
that would bring it back is deferred (TASK-036 rescope: nobody has seen a capacity problem, and
`traffic_gridlock` holds its 40 s bound).

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
every body and claim), the siren yield, the police lane choice, and walkers (civilians walk around the
standing cars in it since TASK-037, `tactics::around_cars`; since TASK-036 a sidewalk node under a
standing car counts as reached beside that car). The fire line (`tactics::nearby_cars` / `car_blocks`)
and sight (`perception::sight_blocked`) keep their own car code: moving them was unification with no
observed bug, dropped in the TASK-036 rescope.

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
within a tick never lets it recover), and with its rejoin corridor to the path line clear (sampled
rectangles 7.5 deg / 0.3 m apart). Against a vehicle at rest the skin shrinks to `switch.skin` plus the
farthest the car's own rejoin moves any part of it (offset, corner arc of its turn and, off a
connector, of the heading swing at rest), at most `recover.skin`; the corridor meets that vehicle
where its own velocity takes it over `recover.horizon_seconds` (`standing` also counts a body creeping
at up to `hold_speed`). TASK-037: a car held inside the full skin of a given-up car stood 88 s; walkers
keep the full skin. On a connector it recovers only while its footprint stays inside the conflict
table's body band (`half.x + conflict_margin / 2` across the path line, and its centre within
`conflict_margin / 2` along it: the table keeps co-granted bodies apart by that model only, TASK-037);
a car between that band and half the lane pitch neither recovers nor gives up (the residual band). It
then rejoins its line at the lateral rate (`Manoeuvre::Rejoin`). A `Dynamic` car relaxed against its
blocker (Progress) ignores that body in recovery and drives through it under the autopilot.
A car that stood `give_up_seconds` in `Dynamic` without recovering, out of its lane band (half the lane
pitch from the path line) and with nothing on its lane line within twice the jam gap ahead, bails out, or
is abandoned at once when no door is free (a bailing car with no exit would stand forever). A car in a
queue or behind a standing body never gives up, it waits: given up there it would be one more standing
body, the car behind it the next to be bumped (TASK-032 QA, a column of 8 `Abandoned`). A bailing car
claims no road (the snapshot derives claims for `Kinematic`/`Dynamic` cars only).

`TrafficCar.lateral` and `manoeuvre` (`None`, `Rejoin`, `Pass`, `Yield`) move a kinematic car off its line
under one law (`lateral.rs`): rate `rate_at_rest + slope x speed`, the heading turned into the move at
most `yaw_rate_deg` per second; on a connector the offset decays linearly to its end, except for a pass
and a rejoin, which step it there too. A rejoin on a connector slides (the heading keeps the path
tangent): a yaw swing from rest would take the body out of the conflict table's band. On a lane the
swing is capped so the body stays in the band the manoeuvre may use: half the lane pitch either side of
the line, widened on a pass's side to its claim and on a yield's side to the car at its offset (the body
reaches `|half| sin(yaw error + atan2(half.x, half.z))` across, both ways; the room is taken at the
current offset and where the offset will be when the yaw can turn back). Without it a pass swung 31-39
deg from rest and put a corner into the next lane (TASK-037, avenue seed 2). A car on its line keeps the
one-tick yaw snap.

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
   (`manoeuvre::sense`): one at the target lateral over `sense_distance` on lanes, one at the current
   lateral over the rest of the sideways move plus twice the jam gap (a strip as long as IDM's rest gap
   would drop the body the car stands behind). On a connector the first one is the car body swept along
   the path over `turn_sense_distance` (onto the exit lane past the end; samples 0.3 m or 7.5 deg of
   path yaw apart, `RoadOccupancy::first_in`), with the gap bisected to 0.01 m of travel; only bodies
   with a part ahead of the nose line count (TASK-037: the straight strip stopped turning cars 2 m
   behind a car their body passes, and missed bodies on the curve). A car on its line with
   no manoeuvre skips `OnPathTraffic` (the path occupancy holds those; a straight strip into a box must
   not brake for crossing cars), except a car held on a connector without a grant (demoted, recovered
   there): no grant keeps crossing cars off it. Every other body, and every oncoming claim, is seen. The strips start at
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
fits around (two bodies in the box) is left to the stuck cheat (Bubble). A car left in a box the player
watches from within `bubble.stuck_in_view_distance` is never removed (TASK-032 R1). TASK-037 resolved the
locks it caused that were not geometry: cars bumped on a connector stood `Dynamic` forever (now they
recover there), turning cars stood behind a straight-strip false positive (now the swept body), and
walkers pinned at the nose of a `Dynamic` head held it (now civilians walk around standing cars,
`civilian_fsm` via `tactics::around_cars`, which scores a corner by the shortest way round the car). A
walk target under a standing car is unreachable (`around_cars` never returns a point inside a car), so
walkers bound for a sidewalk node a left car covers orbited its corners and pressed against each other
there for 70-118 s (TASK-037 QA R1 spot B). `civilian::arrive` counts such a waypoint as reached once the
walker is within `corner + arrive_radius` of the covering car (`tests/walk_arrival.rs`). The TASK-037 QA
hypothesis "walker counter-flow in a narrowed corridor" is refuted for those stalls: 0 of 6361 stalled
samples had a counter-flowing neighbour. On the Linux trajectory of the same scene two walkers leaving that
node met head-on at a car corner (one heading for an `around_cars` corner, one for its lane target, intents
exactly opposite) and stood 89 s: lane targets keep right, corners did not. The civilian caller moves a
corner target `keep_right` across the walker's travel, away from the standing cars (`civilian::off_corner`;
never towards a car, where 0.8 - 0.5 m would put the capsule against the body); `around_cars` itself and the
police arrest path are unchanged (`tests/walk_arrival.rs` A3). Two opposing walkers squeezed between two
standing cars can still press against each other for a while (16 s in the seed-1 spot-B scene, heading 0);
walkers have no walker-walker avoidance.
Resolved by TASK-039 (Progress, below): an approach whose every exit crosses the left car and where no
box pass fits (class E, G4 seed 7) and the R1 seed-1 car switched by the left car appearing in its path
(class D, 113 s in `Dynamic`) now squeeze past the left car with collision relaxed against it. The path check
(`connector_clear`, TASK-036) drives the requester's real body (chassis half extents, no margin) along its
connector from where it stands, sampled by the corridor law (`connector_body`). The old check, the
centre line +- half width with no body length, missed a right-turn pivot's rear swinging 0.87-0.91 m past
it: a car was granted into a kinematic car standing on a conflicting connector (G1 0.53-0.68 m) or
stopped by its own sensing beside it with the grant while that car waited for it (a mutual wait). The
sweep starts at the car, not at connector s 0: from s 0 the rear cap reaches the holder's own lane
follower (the stop line lies within 2.18 m of the lane end on ~240 of ~580 lanes per seed) and would
demote a holder waiting for walkers (the TASK-033 lease). It has no margin: a body grown by half the
conflict margin makes other lanes' queue heads at their stop lines bodies on a turner's path with no
physical contact. The real body also sees cars spilling back onto another exit lane of the node (353-590
waiter ticks per seed in `traffic_gridlock`, all refused by the room check too; stands, demotions,
whole-box grants and re-picks unchanged). The check is guaranteed by the G1 oracle gates, not by the step:
the 0.01 m shrinks of `blocked` plus the corner sagitta between samples can hide ~0.03 m.
`tests/traffic_box_overhang.rs` B1-B5. In `traffic_causes` R1 seed 7 the sweep finds every exit of the east
approach blocked by the left car (the old band left the right turn clear), so each car there needed a
whole-box grant, which waits for an empty box: stands 50.9 s (Linux 57.8 s) against the 30 s bound (TASK-039
class E, now ended by the progress rule; reverting the check would bring back the kinematic pass-through). A push-through by the traffic (a braked car cannot be shoved sideways by the
autopilot, and walkers get pinned between the cars) was tried and dropped.

## Progress: wait-for record and relaxed pass (`progress.rs`, TASK-039)

One universal way out for an AI car stuck behind a standing body, where no per-case rule (lane pass,
lease, repick, box pass, recovery) ends the stand. The per-case rules all stay (none was proven subsumed).

Wait-for record. Each tick every AI car (`Kinematic`/`Dynamic`, road `standing > 0`) gets at most one
edge, the reason it stands, the nearest one winning (ties `Body` > `Grant` > `Follow`):

| Car | Condition | Target | Kind |
|---|---|---|---|
| kinematic | path leader within `pass.trigger_gap` | leader | `Follow` |
| kinematic | nearest sense hit within `pass.trigger_gap`: a claim / a body | claim owner / body | `Follow` / `Body` |
| kinematic | `Pass { go: false }` waiting for moving traffic (a car in its claim, a claim against it, an oncoming car too close to stop: `pass::waits_for_traffic`) | that car or claim owner | `Follow` |
| kinematic | `Pass { go: false }` otherwise | its obstacle | `Body` |
| kinematic waiter left ungranted (`junction::update`) | a conflicting occupant or earlier waiter, the whole-box holder | that car | `Grant` (never relaxed) |
| same | no room past the box / the exit lane start taken | the exit lane's first car (else a car granted into it) / the body or claim owner | `Follow` |
| same | a body on its connector path | the body | `Body` |
| dynamic | `Recovery::Stay { blocker }` (the corridor body, else the body about to reach it; never one wholly behind the car's middle) | the body | `Body` |
| dynamic | else the path leader within twice the jam gap | leader | `Follow` |

A `Follow` edge to a car with no edge of its own that has stood `wait_seconds` counts as `Body` only when
that car is not driven by the AI (a bailing car); a driven car with no recorded reason is never passed
through (it counts as `unexplained`). Before that rule, followers at busy nodes with nobody playing were
relaxed through their own queue head waiting for room (5-11 per 120 s in `traffic_gridlock`; now 0). Edges
form a functional graph; a pointer walk gives each car its chain's sink (a body with no edge: a left
car, a person, a bailing car) or a cycle. No cycle trigger is built (TASK-039 amendment: a nose-to-tail
`Dynamic` pair is not a cycle, the front car's IDM drives it off; the cycles seen were removed at their
cause: in G4 seed 7 a whole-box pass claim over the oncoming exit, below; in the Linux
`c_character_in_the_lane` pile `Dynamic` cars whose `Stay` edge pointed at the car pressed behind them,
which `Stay` no longer reports).

Eligibility. A `Body` edge W -> X is a candidate when W is `Kinematic`/`Dynamic` with no relaxation (or
one in its physics phase against X: below), has stood `grace_seconds` (every new waiter first gets the
clean planners), X stood `grace_seconds` if it is not the sink, and either the sink stood `wait_seconds`
or, when neither X nor the sink is a person, W itself stood `wait_seconds` and the sink the grace (a
nudge that restarts a left car's clock does not restart the waiter's: R1 seed 1). A person keeps his own
clock: one pausing on a crosswalk in front of a car that waited long for its grant is not squeezed after
the grace. Dropped before ordering: W's path leader is relaxed against X or overlaps it (W is not the
car nearest X). Candidates go in order of least overlap with
their blocker (the car's body driven on along its path, at its offset and on its line, until its rear is
past the body; for a head that may still re-pick, the least over its exits clear of every other body;
key `(overlap, entity bits)`).

Accepting, in that order: a car that is being passed is never moved, and no car is relaxed against a car
that is itself squeezing past something (a relaxed car that stands for any reason, a grant too, is not
passing and does not count). Accepting drops any pass the car had around that blocker (its claim would
hold the oncoming car it then waits for). `TrafficStats` counts
`progress_relaxations`, `progress_recoveries` (relaxed `Dynamic` cars recovered) and, per tick,
`unexplained` (AI cars standing past `wait_seconds` with no edge) and `stalled_relaxed` (relaxed AI cars
standing past `wait_seconds`).

The relaxed pass. `TrafficCar.relaxed = Relax { blocker, since, physics_only }`. While planning
(`!physics_only`):
- sensing, the path leader and the IDM skip the blocker (a `Dynamic` car too: its autopilot drives it
  through; braking for it first, as the plan had it, pinned b3's car between the dummy and the pusher
  behind it), and the speed is capped at `pass.speed` (a squeeze is no faster than a clean pass);
- a clean lane pass that can go at once (`plan_pass` + `may_go`) is taken first; otherwise the car
  drives its own line through the body; never a box pass (in the box only the lines are kept apart by the
  conflict table, and a whole-box pass lapses under the lease);
- `connector_clear` / `repick` ignore the blocker; a relaxed head takes the exit of least overlap of
  those clear of every other body (`repick_relaxed`);
- recovery (`nobody_coming`, the corridor and `resting_in`) ignores it.

In both phases (physics consumers): the contact switch (predictive and backstop) skips the pair on
either side (a blocker is not switched by its passer), and the blocker's recovery ignores its passers
(their contacts with it are off: a passer stopped inside its blocker's nose at its stop line held the
blocker `Dynamic` 36.5 s on Linux);
`TrafficHooks::modify_contacts` (the app's one `CollisionHooks`, every traffic car carries
`ActiveCollisionHooks::MODIFY_CONTACTS`) drops the pair's contacts, so no push, no `CollisionStart`, no
damage (`apply_impacts` never sees the pair); `vehicle::PassingThrough(blocker)` makes both cars' wheel
rays skip each other (`cast_ray_predicate`: an awake blocker's suspension rays start inside the passer's
hull and read full travel, 180 deg roll in the flip); `TnuaNotPlatform` on the car keeps a relaxed person's
float sensor off its roof. `filter_pairs` would miss a pair already touching.

End (`progress::upkeep`, before `advance_traffic`): the blocker gone ends it; planning ends
(`physics_only`) when the car left AI, the blocker moved (road `standing == 0`), the car is past it
(every corner behind its rear; every edge points at a body ahead, `Stay` included, so for a `Dynamic` car
too) or after `max_seconds`;
the exemption lasts until the car's footprint grown by `recover.skin` no longer touches the blocker. A car
left standing in its blocker's way in the physics phase (the blocker moved and stopped again on its nose,
the squeeze went stale) is a candidate again against the same blocker and plans again with a fresh
`since`. Markers are inserted and removed only on change. The gates check each relaxation on its own terms
(`tests/traffic_support/third_body.rs`): it starts against a body that was standing when the tick began
and apart from it, and ends within `max_seconds` + 10 s.

`progress` values: `wait_seconds` 16 (18 in the plan, lowered by its latency rule after R1 seed 1),
`grace_seconds` 6 (>= `pass.character_seconds`), `max_seconds` 30. The squeeze runs at `pass.speed`: a
separate 3 m/s held the lane and the box twice as long as a clean pass (Linux `dummy_street_seed_7`, the
R1 seed 1 grant queue).

Also for this task (junction): an ungranted waiter already standing on its connector is granted first
(R1 seed 1: the recovered class-D car waited in the box while every other approach was relaxed through
it: 41 s without it, 27.8 s with it). A waiter that waits for room, the exit lane start or a body on its
path still holds nothing against a conflicting waiter
(`traffic_junction_box::a_waiter_in_the_box_behind_a_body_holds_nothing`).

"One body owner" (D10, not built): always-`Dynamic` traffic under the autopilot, or always-kinematic with
a custom contact response. It touches about 12-15 files (`drive.rs` motion, `contact.rs` and `recover.rs`
removed, `lateral.rs`, `manoeuvre.rs`, `pass.rs`, `box_rules.rs` targets, autopilot precision, the
occupancy kinds, `spawn.rs`, sirens and police) and re-anchors about 25 gate files (`traffic_contact`,
`traffic_recovery`, `traffic_idm`, `traffic_intersection`, the G1 kinematic tolerance, `traffic_bench`
`MEAN_LIMIT` with 24 wheel-raycast bodies, every city trajectory): 3-5 tasks. It would still need a progress
rule (traffic cannot shove a parked car). Not recommended now.

Observation (out of scope): a car standing at the start of an exit lane is not a box body; queues into it
wait on the room check (`Follow`).

## Sirens (`traffic/sirens.rs`, `police/siren.rs`, TASK-032)

An idle kinematic car on a lane yields to a siren body (Respond or Chase) coming up behind it within
`sirens.yield_distance` (heading along the lane, within a lane pitch) or driving at it: it shifts to the
free curb lane of an avenue, else the 0.425 m slack of its own lane, at most at `pass.speed`, then stops
(IDM on a virtual obstacle at its comfortable stop `v^2/2b`, which ends at 0: a gap ending at the jam gap
is IDM's rest point, and the car crept forever). It does not yield when the siren car cannot get by (no
free curb lane and no opposite lane) or within a car length and a jam gap of its lane start (the siren
car would stand in the box behind it, where it cannot change lanes). It drives on once the siren car is
past or after `sirens.timeout_seconds`, then ignores sirens that long. A yield that reached the box ends
there (`Manoeuvre::None`, TASK-036): the car drives out with its offset decaying to the connector end
(before, the yield stop held it in the box with its grant for good). A car must not reach the box at the
avenue curb offset: the conflict table and its oracle assume a car enters its connector on its line, and
bodies of connector pairs the table grants together touch from a 2.3-2.5 m entry offset with the decay
law, 2.7-2.9 m stepped (the 0.425 m slack yield touches nothing). Without a guard a curb yield begun 6-12 m
before the stop line met a co-granted car (G1 up to 0.48 m). So the curb lane is taken only when the nose
is at least `yield_reach` before the stop line: the shift at the yield's top speed `max(v, pass.speed)`
plus the stop from `pass.speed` (22.3 m from 6 m/s, 27.0 m from 16), else the slack. The start is the only
check: ending a curb yield mid-shift near the box would enter it as an offset Rejoin; a car standing at its
curb offset at the stop line travels at least half a car length before the connector (>= 1.65 s from
rest) and enters under 1.93 m. `sirens::tests` replays the tick law against the reach (8.5 m margin); `traffic_box_overhang` C1-C3
start the yield through a real siren car (C2: refused within the reach, the slack yield ends in the box;
C3: taken outside it, stands before the stop line). The real-start App sweep (seeds 1/7, three curb pairs
each, nose 6-12 m and 17.3-32.3 m before the stop line, the other car 0-96 ticks later; the curb cells
rejoin from their stand after the timeout) is clean; with the guard off it finds G1 0.25-0.40 m on the two
pairs tried.

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
