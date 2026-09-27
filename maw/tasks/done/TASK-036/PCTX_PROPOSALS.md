# PCTX proposals — TASK-036

- 2026-09-27 (planner), domain game-design (or gates): **A waypoint under a standing car is unreachable, and
  walkers orbit the car.** `civilian::arrive` takes the next edge only within `arrive_radius` of the lane
  target. A car covering a sidewalk node (seed 1 spot B: car centre 0.5 m from node 312) kept 11-13 walkers
  circling its corners for 70-118 s. TASK-037 QA read it as counter-flow; the trace showed 0 counter-flow
  neighbours and 100 % corner targets, and a control run without the car had 0 stalls. Lesson: before
  naming a crowd rule, trace each stalled agent's target and whether that target is reachable. Trigger:
  `arrive_radius`, `lane_target`, `around_cars`, a walker stall near a parked car.
- 2026-09-27 (planner), domain bevy-ecs/traffic: **A manoeuvre managed only on lanes must end or be guarded
  at the box.** `sirens::update` returns early off a lane, so a Yield that crossed into a connector at its
  full offset stopped the car there with its grant held (reproduced: + floor and seed 1 curb yield). Trigger:
  any `Manoeuvre` whose start/end logic has `let Segment::Lane(..) = .. else { return }`.
- 2026-09-27 (implementer), domain gates/traffic: **A truer path check can turn a flowing approach into a
  whole-box queue.** The real-body box check (TASK-036 B) found that every exit of seed 7 box 84's east
  approach meets a car left in the box (the right turn's rear swing), where the old band found one clear.
  Each car then needs a whole-box grant, which is given only to an empty box, while a body-blocked waiter
  does not block the node: the other approaches keep the box busy and the queue stands 50.9 s (HEAD 23.6 s;
  `traffic_causes` R1 seed 7). Lesson: before tightening a "path clear" predicate, count per approach how
  many exits it leaves open next to a standing body, and run the left-car city rows. Trigger: `connector_clear`,
  `repick`, the whole-box grant in `junction.rs`.
- 2026-09-27 (implementer), domain gates/traffic: **Ending a lock can expose the contact it was hiding.** The
  C fix (a yield ends in the box) turned "stuck in the box with the grant" into "drives out at a 3.25 m offset
  and meets a co-granted car" (G1 up to 0.48 m in a timing sweep; HEAD sweep clean because the car never left).
  Run the contact oracle sweep on the fixed code, not only on HEAD. Trigger: a liveness fix for a car frozen
  inside a junction or with an offset.
- 2026-09-27 (fixer), domain gates: **A manoeuvre gate starts the manoeuvre through its real start rule.**
  The C rows and the C sweep first set `Manoeuvre::Yield` directly; once a start guard existed, they would
  have gated a state production can no longer create (red on an unreachable state, or vacuous). The fixed
  rows spawn a real responding police car behind the car, let `sirens::update` choose, remove the siren car,
  and name the chosen offset. Trigger: a gate that writes `manoeuvre = ...` into a `TrafficCar`.
- 2026-09-27 (fixer), domain gates/traffic: **Avenue curb lanes hold parking-spot cars in the seed cities.**
  A curb-yield row on seed 1 lane 437 stopped 4.8 m after its start for a parked car 11 m ahead in the curb
  lane (the curb-free check looks only beside the car); the row looked green for the wrong reason. Curb-lane
  fixtures remove parked cars on their run (a named mutation) and assert the car moved until its shift was
  done. Trigger: a traffic gate on a `curb_lane` in `city_app`.
- 2026-09-27 (fixer), domain game-design: **Walk-around corner targets keep right away from the car.** A
  walker heading for a bare `around_cars` corner met a walker leaving the car on the line through it, head-on,
  89 s (Linux trajectory). The civilian caller shifts a corner target `keep_right` across the travel to the
  side away from the standing cars (never towards: 0.8 - 0.5 m touches the body). Walkers still have no
  walker-walker avoidance: two opposing walkers squeezed between two standing cars pressed 16 s. Trigger:
  `around_cars` callers, `keep_right`.
