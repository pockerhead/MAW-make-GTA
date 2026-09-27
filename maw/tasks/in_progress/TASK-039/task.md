# TASK-039: Box U-turn — an approach whose every exit crosses a car left in the box

Type: feature
Mode: full
Priority: high
Branch: feature/box-uturn
Domains: bevy-ecs, gates, game-design

Cost of error: silent. The class is gridlock that only shows over minutes, in front of the player.

## Description
TASK-037 resolved the in-view box lock where it was not geometry: `Dynamic` cars on connectors now
recover, connector sensing follows the path, walkers go around standing cars. Two rows stay red and
ignored; each ignore reason points here. (Two more, R1 seed 7 and the G4 extra row, were red at the
TASK-037 review and turned green in its fixer round: walkers stopped dithering at a corner past a car,
and crossing cars now see a car held on its connector without a grant. They are un-ignored.)

### Class E: every exit of an approach crosses the left car, no box pass fits
- G4 seed 7 liveness (`traffic_junction_box::seed_7_box_keeps_moving_liveness`): the lane-299 queue
  stands 61.3 s (bound 40 s) on Windows and Linux. (R1 seed 7 showed the same class, 100-106 s, until
  the TASK-037 fixer round.)
- The true body sweep of each exit meets the left car at s 1.4-2.3. `repick` (the re-route) runs and
  finds no clear exit. The whole box goes to the head in turn, and `plan_box_pass` finds no side: side +1
  needs a 5.71 m shift (more than pitch + slack, 3.67 m); side -1 runs into the oncoming approach's own
  queue head at its stop line and into walkers on the exit crosswalk. The lease lapses and the box
  rotates to other waiters.
- In R1 seed 7 the head was also granted into a blocked path: `connector_rects` has no corner overhang
  and found exit 713 clear where the swept body meets the left car (TASK-036 item 4). TASK-036 item 4
  may shrink this class; take this task after it.
- Named fallback from TASK-037 (two failures of the class): a U-turn, or reverse-out connectors, used
  only when every exit is blocked and kept out of the random exit choice (`junction.rs` picks
  `out[rng]`). Physical push-through and despawning a car in view stay out (TASK-032).

### Class D residue: R1 seed 1
- `traffic_causes::r1_car_left_in_the_box_seed_1`: 1853v0, granted and driving through the box at
  2.8 m/s, is switched by the left car teleported 1.4 m ahead of it, then stands 0.06-0.26 m from it
  (inside `recover.skin` 0.4) in `Dynamic` for 113 s (149 s before TASK-037's rest skin).
- Two fixture attempts (the G4 free-hub rule; also no grant at the node) failed and were reverted.
  Decide: a fixture that places the car where no granted car is inside its switch reach, or a recovery
  rule for a `Dynamic` car pinned against a vehicle at rest.

### Evidence (TASK-037, `maw/tasks/done/TASK-037/`)
- `OPEN_DECISIONS.md` (implementer entries 2026-09-27 and the orchestrator's resolution).
- `scratch/stage1/watch_g4_seed_7.txt`, `scratch/stage1/boxpass_g4_seed_7.txt`: class E in G4 seed 7.
- `scratch/stage4/trace_r1_seed_1.txt`, `scratch/stage4/causes_2.txt`: class D in R1 seed 1 and the
  fixture attempts.
- `scratch/stage5/causes_box.txt`: the ignored rows' stands with `--include-ignored`.
- Probe: `scratch/probe/ws/probe` (`PROBE_WATCH=<entity,...>`), `scratch/wsl/` for the Linux runs.

## Acceptance criteria
- [ ] Un-ignore `traffic_junction_box::seed_7_box_keeps_moving_liveness` and
  `traffic_causes::r1_car_left_in_the_box_seed_1`; all green on Windows and Linux (WSL recipe in
  TASK-037 `scratch/wsl/`, `scratch/fixer/linux_run.sh`).
- [ ] Every new mechanism flips RED under a named sabotage; G1 oracle clean in every touched city gate.
- [ ] No regressions: `cargo test -p gta_sim -p citygen` (including `traffic_gridlock` seeds 1/2/7/42),
  `traffic_bench` / `police_bench` / `civilian_bench` under their `MEAN_LIMIT`, clippy, client tests,
  `tools/qa/tree_check.py`, the 5 CI workflows.
- [ ] `docs/architecture/traffic.md` box section updated; GDD §5.2 one-line amendment if the U-turn is
  player-visible.
- [ ] Owner run (not gated): how a U-turn out of a blocked approach looks.

## Dependencies
- blocked by TASK-037
- prefer after TASK-036 (item 4, `connector_rects` corner overhang)

## Added by TASK-037 QA
- The "second shape": the player's car left at a box entry, pressed against the approach head, freezes that approach (twice in runtime: 15 cars > 30 s, once with 4 `Dynamic` cars at 149 s). It is the same class D trigger (a `Vehicle` switch at contact with a standing body). Evidence: `maw/tasks/done/TASK-037/QA_REPORT.md`, `scratch/qa/r1/A_*`.

## Added by TASK-036 fixer
- `traffic_causes::r1_car_left_in_the_box_seed_7` is ignored again, class E: TASK-036's real-body path check (`box_rules::connector_body`) blocks all three exits of box 84's east approach (lane 297; the old band left right turn 713 clear), so the queue stands 50.9 s Windows / 57.8 s Linux (bound 30 s). Un-ignore it here. Evidence: `maw/tasks/in_progress/TASK-036/scratch/stage4/r1_trace_fixed.txt` (and `r1_trace_head.txt`).

## Redesign direction (orchestrator, 2026-09-27, after a hint relayed from the owner's other session; decide after TASK-040)
Observation: almost every fix in TASK-032..038 sits on the seam "logic vs physics". A car has two owners (Kinematic and Dynamic), and each lock geometry got its own local rule: the lease (033), the lane pass (032), repick and the box pass (037), and now a U-turn (039). The system has no general progress guarantee and no wait-for cycle detector (TASK-033 found a real cyclic deadlock at node 83). The only universal escape, a cheat, is forbidden in frame, so the residue collects in front of the player. Hypothesis: "zero overlaps and zero in-frame cheats" is incompatible with a two-mode body unless a separate progress rule exists.
Industry answers:
- SUMO `--time-to-teleport`: stuck vehicles teleport ahead or are removed, with classified causes. https://sumo.dlr.de/docs/Simulation/Why_Vehicles_are_teleporting.html
- SUMO `--ignore-junction-blocker`: after T, a vehicle stops treating a car standing in the junction as an obstacle and drives around it. This is exactly class E. https://sumo.dlr.de/docs/Simulation/Intersections.html
- AIM space-time cell reservation (Dresner & Stone). https://www.cs.utexas.edu/~aim/
- A wait-for-graph deadlock detector: a cycle is broken deterministically (the lower key yields or passes).
- Cities: Skylines TM:PE: stuck vehicles despawn.
Preferred direction to evaluate FIRST, instead of the U-turn:
1. An explicit wait-for detector that replaces per-case timers.
2. An in-frame "ignore the blocker after T" pass: a curb or oncoming pass with collision relaxed against the blocker only for the manoeuvre. This is a drama cheat in the same spirit as the police drama system. It accepts a little visual imperfection for guaranteed progress. The orchestrator has the owner's delegation to decide this.
3. In physics, one body owner with one return rule instead of two modes. This is a larger refactor; evaluate its cost.
The U-turn stays as a fallback only.

## Added by TASK-040 playtest (orchestrator, binding scope)
- M1 residue (MAJOR, 2 of 2 on seed 1): the player's car plus a bumped traffic car block both lanes, and the street stands >= 88 s with nobody passing. Evidence: maw/tasks/done/TASK-040/PLAYTEST_REPORT.md and scratch/sessions.
- N1 (minor, confirmed): a pedestrian (the player) standing in the junction box holds the approach for 74 s in the repro, and in the worst case (s42 tourist) 19 of 24 cars stood 110 s.
- Both share the root named in "Redesign direction": no progress guarantee. Scope this task around ONE universal progress rule covering vehicles, characters and box blockers, not per-geometry rules. The U-turn is a fallback only. Acceptance adds: the TASK-040 M1-residue repro and the N1 repro become headless rows (no car stands > 30 s behind a stationary body, in frame, with no pass-through against any third body), plus class E and class D.
