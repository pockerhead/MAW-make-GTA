# TASK-036: Shared road occupancy — remaining consumers

Type: refactor
Mode: full
Priority: medium
Branch: refactor/occupancy-consumers
Domains: bevy-ecs, gates, game-design

## Description
TASK-032 built one spatial truth for "what occupies the road" (`crates/gta_sim/src/occupancy/`,
`RoadOccupancy`: every vehicle body and every enabled character near the player as a flat footprint with
velocity, standing time, siren flag and pass claims; queries `first_along`, `blocked`) and moved the
traffic-side consumers onto it: traffic sensing, the lane-start room check and connector path of the
junction grants, the traffic spawner, the siren yield and the police lane choice
(`docs/architecture/traffic.md`, "Road occupancy"). The other consumers still see cars through their own
code. This task moves them onto the shared API, one row per consumer:

1. **NPC walk avoidance**: `navigation::avoid_offset` / `perception::wall_blocked` see walls only;
   civilians have no `around_cars` (police do, `tactics::around_cars`). Includes the unconfirmed
   TASK-031 M1 side symptom "civilians stand against an abandoned car" (s42 tourist: 10 wanderers for
   90-152 s; the dedicated TASK-031 repro `repro_r1_sidewalk` did not reproduce it; TASK-032 R1 reports
   the civilians within 8 m of the left car). Reproduce it headless first (premise rule: a cause that
   does not reproduce is dropped and reported).
2. **Fire line**: `tactics::nearby_cars` / `car_blocks` onto `RoadOccupancy`. Unification only: it
   already sees cars, no observed bug.
3. **Sight**: `perception::sight_blocked` (the Vehicle mask) onto the shared footprints where it pays;
   unification only, no observed bug.
4. **Conflict-point junction reservation** instead of the whole connector (GDD §5.2 wording, deferred by
   TASK-016 and TASK-033). Include the TASK-032 G1 finding: the conflict table (`traffic/graph.rs`,
   centre-line distance < 2 x half width + margin) misses the corners of a car swinging out on a curve;
   the G1 oracle caught two granted left turns from adjacent approaches interpenetrating by 0.34 m
   (TASK-032 `REDESIGN_NOTE.md`, `scratch/g6/g1_flip_trace.txt`, seed 5). Conflicts from swept car
   rectangles, gated against `traffic_gridlock` throughput.

Traffic lights (P1) stay outside the GDD and this task.

Related: TASK-037 (in-view junction box lock) owns the box side of the node-141 case; the walkers pinned
around that grant holder are walk avoidance, item 1 here.

## Acceptance criteria
- [ ] Each migrated consumer has a flip-RED row that goes RED when its occupancy input is removed
  (the TASK-032 `traffic_occupancy.rs` pattern).
- [ ] Walk avoidance: the M1 side symptom reproduced headless and fixed (no civilian stands against an
  abandoned car longer than a derived bound), or dropped and reported if it does not reproduce.
- [ ] Junction: the swept-corner conflict case has a G1 row (the TASK-032 `Footprints` oracle, 0 kinematic
  interpenetrations) and `traffic_gridlock` stays within its 40 s bound on seeds 1/2/7/42.
- [ ] No regressions: `cargo test -p gta_sim -p citygen`, `traffic_bench` / `police_bench` under their
  `MEAN_LIMIT`, clippy, client tests, `tools/qa/tree_check.py`, the 5 CI workflows.
- [ ] `docs/architecture/traffic.md` lists every consumer as migrated.

## Dependencies
- blocked by TASK-032
