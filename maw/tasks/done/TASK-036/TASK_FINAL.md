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
   Added by TASK-038 (the swept-rectangle conflicts shipped there, `graph.rs` `body_sweep`):
   - Cost: connector pairs allowed to hold grants together fell 2340 -> 1323 on seed 1 (-43 %, 52-58 %
     kept on seeds 1..8), every lost pair involving a right turn; opposite straights unaffected. Root:
     citygen right-turn connectors are ~2-2.6 m long (radius ~1.7 m), so a 4.08 m body pivots almost in
     place and its nose and rear sweep the neighbouring lanes. Conflict points (or a wider right-turn
     radius in citygen) are where that capacity comes back.
   - `box_rules::connector_rects` (used by `connector_clear`, `junction.rs:109`) still models a body on
     a connector as centre line +- half width; the unmargined body corners reach 0.87-0.91 m past that
     band on every seed (right-turn pivot). A body the conflict table does not know (a stuck holder
     demoted while it stands on its connector, `junction.rs:149-153`, still kinematic; a box passer; a
     left car) in that overhang does not stop a grant on a conflicting connector: a kinematic car drives
     through it. Not reproduced in a scene. Add a G1 fixture: a kinematic car demoted on a right-turn
     connector, a conflicting connector granted. Swapping in `body_sweep` shapes changes repick/stuck
     behaviour (it reaches 4 m onto the exit lane) and needs its own measurement.
   - Neither the table nor the `traffic_graph` oracle models a car entering a connector with a residual
     lateral offset (`lateral.rs` `effective_lateral`, a yield or lane pass ending near the box).

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

## Moved by TASK-037 (orchestrator, 2026-09-27)
- Item 1 (walkers avoid cars, via `tactics::around_cars` over `RoadOccupancy`) is done inside TASK-037, because it is the dominant cause of the in-view box lock. This task keeps the remaining items.

## Added by TASK-037 QA
- Walkers still pile up at a car on a crosswalk in counter-flow (runtime R1 spot B: 9 and 6 walkers up to 103/150 s; main 10 and 17 up to 153 s). Headless probe: `maw/tasks/done/TASK-037/scratch/probe/ws/probe/tests/qa_walkers.rs` (11-13 walkers 70-118 s, capsules touching at 0.60 m, 0.1-1.3 m from the car). Likely walker-walker counter-flow in a narrowed corridor.

## Orchestrator rescope (2026-09-27, binding; supersedes the item list above)
Project rule: every abstraction must solve a real problem (YAGNI). Items 2 (fire line) and 3 (sight) are "unification only, no observed bug", so they are dropped. The conflict-point reservation (item 4, capacity) is deferred: nobody has seen a capacity problem, and `traffic_gridlock` holds its bound. It is noted in traffic.md as the known cost of swept conflicts. The task is now three items, each causal-first: a cause that does not reproduce headless is dropped and reported, not built.
A. **Walker counter-flow pile-up at a car on a crosswalk** (TASK-037 QA; runtime R1 spot B: 9 and 6 walkers up to 103/150 s; headless probe `maw/tasks/done/TASK-037/scratch/probe/ws/probe/tests/qa_walkers.rs`, 11-13 walkers 70-118 s). The player can see it. Reproduce it as a headless row, find the rule that makes it (walker-walker avoidance in a narrowed corridor? around_cars corner shared by opposite flows?) and fix the rule. Gate: no walker stands > a derived bound near a standing car on a crosswalk, with a flip.
B. **`connector_rects` corner overhang** (item 4 bullet 2): build the G1 fixture first (a kinematic car demoted on a right-turn connector, a conflicting connector granted). If it interpenetrates, fix it with the body shapes, and measure the repick and stuck side effects. If not, drop it and report.
C. **Residual lateral offset at connector entry** (item 4 bullet 3): a G1 fixture with a car entering a connector with a yield or pass offset. Fix only if it reproduces.
Acceptance: each item either fixed with a flip-RED gate, or dropped with evidence. No regressions (full suite on Windows AND Linux via WSL, traffic_gridlock seeds 1/2/7/42 within 40 s, benches, clippy, client tests, CI 5/5). traffic.md records the rescope.

### Resolved questions (orchestrator, after PLAN.md)
- Q1: yes, fix the siren-yield lock on a connector (Yield becomes None on a connector) in this task. It is a real stuck-forever grant holder that stage 0 reproduced.
- Q2: add a lane-end guard for the siren yield only if the stage-0 timing sweep shows a G1 contact.
- Q3: the curb-offset entry clipping the block (up to 0.51 m on about 140 connectors per seed) goes to the narrative-graph backlog as an observation. Nobody has seen it in play, so it gets no task yet.
- Q4: yes, record in traffic.md that the TASK-037 QA "counter-flow" hypothesis is refuted (the cause is an unreachable walk target under a standing car).
