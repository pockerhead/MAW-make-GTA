# PCTX proposals — TASK-016 (planner)

## 2026-09-25 — bevy-ecs risk lesson: avian3d 0.7 kinematic bodies
Probe `maw/tasks/in_progress/TASK-016/scratch/probe_kinematic` (avian3d 0.7.0, release):
- A kinematic body moves exactly by its `LinearVelocity` (10 m/s x 64 ticks = 10.000 m).
- Kinematic-static and kinematic-kinematic pairs are created by the broad phase
  (`collision/broad_phase/bvh_broad_phase.rs:90-160`: only static-static is skipped) and write
  `CollisionStart`; two kinematic boxes then pass through each other (no solver response). A kinematic
  box 0.1 m above the ground already reported `CollisionStart` with it (speculative contact).
- `RigidBody` is an immutable component (`dynamics/rigid_body/mod.rs:283`): change it with
  `insert`, never `&mut`. An `insert(RigidBody::Dynamic)` from `FixedPostUpdate` `.before(PhysicsSystems::First)`
  acts in the same step (gravity applied that tick); mass properties exist while kinematic.
- A dynamic body driven into a resting kinematic one stops dead (infinite mass) in the contact step; switching to
  dynamic one tick before contact exchanges momentum. A `CollisionStart`-driven switch is one step late.
Rule: a kinematic body that must become dynamic "on contact" switches predictively before the physics
step; never rely on the solver to separate two kinematic bodies. Trigger: `RigidBody::Kinematic`.

# PCTX proposals — TASK-016 (implementer)

## 2026-09-25 — gates: a threshold equal to a controller's rest point never fires
IDM stops a car with its bumper exactly `s0` before a stop line; the PLAN's intersection request distance
at rest (`v^2/2b + s0 + half_length`) was that same point, so queue heads crept towards the line
asymptotically before asking (loop gate: ~1 m/s, 197 m in 100 s instead of 290+). Rule: a trigger
defined on a controller's own equilibrium gets a margin (here one more `s0`); a liveness gate on distance
travelled finds it, an invariant gate does not.

## 2026-09-25 — gates: city gates now run with traffic
With a `CameraView` set, every seed-1 city gate has up to 24 traffic cars and police cars. A straight
player run on the sidewalk graph crosses streets and a queued car stops it (street_spawn 130 m of 270,
civilian turnover 46 < 48 deficit ticks); a car nudges a civilian spawned on a crosswalk in its first tick
(0.067 m off the graph, CI only). A gate whose subject is not traffic sets
`TrafficConfig.bubble.max_cars = 0` with a one-line reason. Runtime t9 counts a traffic run-over as
friendly fire unless it attributes health drops next to a car. Trigger: `city_app(` plus `set_view(`.

## 2026-09-25 — game-design: police cars in traffic
Police cars do not reserve intersections and follow IDM on a forward cast: behind a traffic queue they
wait. Without a fallback a car 60 m out never arrived (the park centre, 43 m from any lane). Held up for
`car.blocked_seconds` the crew goes on foot. Measured in `police_city` (seed 1, 2 stars): first cop at the
park by car 63 s, on foot only 1.3 s; hospital 6-27 s by car. Owner-run concern, not a gate.

# PCTX proposals — TASK-016 (code-reviewer)

## 2026-09-25 — game-design: a reservation's room check must see every body, not only its own agents
Traffic `junction::room()` counts only AI traffic cars; an abandoned, taken or police car standing just past a
crossing lets a granted car stop inside the box while it holds a connector that conflicts with ~67 % of the
node's movements (whole intersection locked until the obstacle despawns, never while in view). Rule: any
admission check ("don't block the box", spawn room, seat room) queries the physics world or a complete
occupancy, never the subset its own system manages. Trigger: `fn room(` / `occupancy` in `traffic/`.

## 2026-09-25 — game-design: police response by car is dominated by lane routing, measure it per spot
Seed 1, 2 stars, park centre (43 m from the nearest lane): goal radius max(nearest, 15 m) admitted one directed
lane, routes 4.1x the straight distance (median 376 m), first cop 63 s vs 1.3 s on foot. A liveness window that
was widened to 80 s hid it. Rule: police-by-car changes report first-reach time per landmark (park, hospital,
plaza) in the summary; widening a response window is a finding, not a fix.

# PCTX proposals — TASK-016 (fixer)

## 2026-09-25 — game-design: police cars on a lane graph without U-turns spawn facing their target
With the car spawn ring moved in to (30, 60) m, the nearest police car in the t15 chase often faced away:
A* then looped a 4-lane block (BRP trace: 30 m -> 133 m) and t15 "the police cars close in" failed 2/2;
with the old (60, 120) ring it passed. Rule: a vehicle spawner on the traffic graph keeps only lanes whose
tangent points at the target (`tangent . (target - point) > 0`). Trigger: `spawn_points()` in `police/`.

## 2026-09-25 — bevy-ecs: a transition that can fail must be undone, or its exit rule flips it back
Respond -> Dismounted with nobody able to get out (no free door), and Respond -> Dismounted while the player
is already past `reboard_distance` (the crew re-boards the same tick), both came straight back to Respond
the next tick: 1132 transitions in 25 s for one car, which never moved. Rule: when entering a state has an
action that can fail or be undone at once, check the action's result and stay in the old state; gate with
"state stays X for N ticks" plus the transition count. Trigger: `next_car_state` / any FSM with enter actions.

- 2026-09-25 (QA TASK-016, gates domain): a runtime step that teleports the player in front of a moving traffic car
  must wait for `HitReaction == Steady` before sending F: a knocked-down player's `vehicle_requested` is consumed and
  ignored (`seat.rs enter_exit`), which read as a 2/5 "Driving not reached" flake. Also: police-car behaviour gates
  need one row with the player sitting in a STOPPED car far from the route end, and pull-out gates need the full
  crew (two cops in Arrest), not one hand-placed cop — both defects were invisible to the shipped rows.

# PCTX proposals — TASK-016 (fixer round 3)

## 2026-09-25 — game-design: an NPC's walk avoidance is walls-only; a car body silently walls it off
`navigation::avoid_offset` probes the World layer only (TASK-015: "movement checks stay walls-only"). A cop
seeking a driver straight on pushed for 38 s against a traffic car queued behind the player's car (QA round 2,
seeds 2-3). Fix precedent: `tactics::around_cars` (corner of the first car within the capsule radius of the
line, in plain view). Rule: any NPC seek whose destination sits in or next to traffic (a driver's door, a car
crash site) routes around car rectangles; gate it on seeds 2-3, not seed 1 alone (seed 1 approached from the
side and hid it). Trigger: `Motion::Seek` with a destination derived from `Driving`/`door_point`.

## 2026-09-25 — game-design: aiming turns the body; an NPC aiming at a seated driver faces the car
`character` faces `AimIntent.direction` while aiming, and the view cone (110°) follows the body. An arresting cop
aimed at the seated driver, so the pulled-out player 1 m away at the door was outside its cone: Arrest -> Respond ->
Search, the arrest dropped (seed 3 hospital). Rule: an NPC waiting for a target to appear at a point aims at that
point. Trigger: `aim_from_eyes(` next to `player_door` / `door_point`.
