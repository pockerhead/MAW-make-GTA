# TASK-039: Universal traffic progress guarantee (wait-for detector + pass with collision relaxed against the blocker)

Type: feature
Mode: full
Priority: high
Branch: feature/box-uturn (name kept from task.md; the U-turn is no longer the main mechanism)
Domains: bevy-ecs, gates, game-design

Cost of error: silent. The class is gridlock that only shows over minutes, in front of the player. Full
evidence layer: headless rows with flip-RED, Windows and Linux, runtime QA on the TASK-040 repros.

## How this spec was assembled

`task.md` has layers written at different times: the original class E / class D and U-turn text, the
"Added by TASK-037 QA" and "Added by TASK-036 fixer" notes, the "Redesign direction" (2026-09-27) and
"Added by TASK-040 playtest (binding scope)". Per the orchestrator's binding note, the last two win.
This file is the single spec. Where an older layer conflicts with it (the U-turn as the main mechanic,
"physical push-through and despawn in view stay out", "decide fixture vs recovery rule" for class D),
the resolution is written under "Decisions".

## Problem

Almost every traffic fix in TASK-032..038 sits on the logic-vs-physics seam. A car has two body modes
(`Kinematic`, `Dynamic`), and each lock geometry got its own local rule: the lane pass (032, `pass.rs`),
the lease (033), repick and the box pass (037, `box_rules.rs` / `junction.rs`), the recover skin (037,
`recover.rs`). There is no general progress guarantee and no wait-for cycle detector (TASK-033 found a
real cyclic wait at node 83). The only universal escape so far is the stuck cheat (`bubble.stuck_*`),
which is forbidden in frame near the player, so the residue collects exactly where the player looks.

Known residue that this task must end, all in frame:

1. **M1 residue (TASK-040, MAJOR, 2 of 2 on seed 1).** The player's car left on a street at about
   (34.9, -76.9) and a bumped traffic car on the centre line at about (34.0, -79.2) block both lanes. Cars
   on both sides, (27.9, -77.5) and (40.2, -80.4), stand 88-95 s, manoeuvre `None`, 0 passes. GDD §5.2
   allows a go-around only over a free opposite/curb lane, so there is no way out.
   Evidence: `maw/tasks/done/TASK-040/PLAYTEST_REPORT.md` (M1), sessions `m1_s1` (68-158 s),
   `repro_m1_centreline`; script `scratch/tools/m1_fixed.py <out> 34.6 -79.1`.
2. **N1 (TASK-040, minor, confirmed).** A pedestrian (the player) standing in the junction box holds the
   approach while standing there: 74.2 s in the seed-1 repro (box (7.9, -81.3)); worst case s42 tourist,
   19 of 24 traffic cars standing up to 118 s. Evidence: sessions `repro_n1_player_in_junction`,
   `s42_tourist`, `s7_reckless`; script `scratch/tools/repro_player_in_junction.py`.
3. **Class E: every exit of an approach crosses a car left in the box, no box pass fits.**
   - `traffic_junction_box::seed_7_box_keeps_moving_liveness`: lane-299 queue stands 61.3 s (bound 40 s),
     Windows and Linux. The swept body of each exit meets the left car at s 1.4-2.3; `repick` finds no
     clear exit; `plan_box_pass` finds no side (+1 needs 5.71 m > pitch + slack 3.67 m; -1 runs into the
     oncoming approach's queue head at its stop line and walkers on the exit crosswalk); the lease lapses
     and the box rotates.
   - `traffic_causes::r1_car_left_in_the_box_seed_7` (re-ignored by the TASK-036 fixer): the real-body
     path check (`box_rules::connector_body`) blocks all three exits of box 84's east approach (lane 297);
     the queue stands 50.9 s Windows / 57.8 s Linux (bound 30 s). Evidence:
     `maw/tasks/done/TASK-036/scratch/stage4/r1_trace_fixed.txt`, `r1_trace_head.txt`.
4. **Class D: a `Dynamic` car pinned against a vehicle at rest.**
   `traffic_causes::r1_car_left_in_the_box_seed_1`: 1853v0, granted and driving through the box at
   2.8 m/s, is switched by the left car teleported 1.4 m ahead of it, then stands 0.06-0.26 m from it
   (inside `recover.skin` 0.4) in `Dynamic` for 113 s. The TASK-037 QA "second shape" is the same trigger:
   the player's car left at a box entry, pressed against the approach head, froze the approach (15 cars
   > 30 s; once 4 `Dynamic` cars at 149 s). Evidence: `maw/tasks/done/TASK-037/QA_REPORT.md`,
   `scratch/qa/r1/A_*`, `scratch/stage4/trace_r1_seed_1.txt`, `scratch/stage4/causes_2.txt`.

## Goal

ONE universal progress rule for AI traffic, covering every blocker kind (vehicles, characters, bodies in
the junction box) instead of one more per-geometry rule:

1. **Wait-for detector.** An explicit record of who waits for whom: car -> car, car -> character,
   car -> box blocker (a body on its connector path), grant waiter -> grant holder. Progress triggers
   when (a) the wait-for relation has a cycle, or (b) a car has waited on a stationary body longer than
   a threshold T. A cycle is broken deterministically (the same seed on the same platform gives the same
   result; the planner names the key, e.g. the lower key yields or passes).
2. **Progress pass.** The triggered car passes the blocker with collision relaxed against THAT body only,
   for the duration of the manoeuvre, on a curb, oncoming or in-box path. It never passes through a third
   body: every other vehicle, every other character and static world geometry are respected as today
   (the G1 oracle holds for every pair except the relaxed pair).
3. The same rule covers M1 residue, N1, class E and class D. The U-turn is a fallback only (see Decisions).

## Decisions (resolved by the clarifier under the orchestrator's delegation; recorded as binding)

- **D1. In-frame drama cheat accepted.** The orchestrator, holding the owner's delegation, accepts a small
  visual imperfection (a car brushing past or slightly into the blocker) in exchange for guaranteed
  progress. This withdraws the "no in-frame cheat" stance of TASK-032 / TASK-037 for this rule only.
  Everything else in that stance stays: no despawn of a car in view near the player (the `rb_*` rows stay
  green), no physical push-through by traffic (a braked car cannot be shoved, TASK-032 lesson), and no
  pass-through against any third body. The decision is written into GDD §5.2 (see Acceptance).
- **D2. What "collision relaxed against the blocker" means.** For the pair (passer, blocker) only, and only
  while the progress manoeuvre runs: the passer's sensing, path checks (`connector_clear`-like), pass
  planning and the kinematic -> dynamic switch (`contact.rs`) ignore that blocker, and the physics contact
  between the two does not push either body (a kinematic car must not shove the blocker with infinite
  mass). The relaxation ends when the manoeuvre ends; a stale relaxation must not outlive it. Paths that
  clear the blocker are preferred; overlap with the blocker is allowed only when no clear path exists.
  The worst overlap depth with the blocker is measured and reported by every new row (report, not a pass
  bound); its look is judged by the owner run.
- **D3. Character blockers.** A relaxed character (N1: the player on foot) is not damaged, not knocked
  down and not launched by the passer during the manoeuvre (`apply_impacts` / `VehicleHit` must not fire
  for the relaxed pair). Whether the car's body visually brushes the capsule or clears it is the planner's
  choice within D2's "prefer clear paths". Other characters (walkers on the curb or crosswalk) are third
  bodies and are never passed through.
- **D4. Class D is fixed by the rule, not by the fixture.** `r1_car_left_in_the_box_seed_1` keeps its
  fixture (two fixture attempts already failed in TASK-037). A `Dynamic` car whose obstacle is a body at
  rest is a waiter in the detector like any other, and progress must also end its `Dynamic` stand
  (recovery or the pass, the planner decides how), with the city-wide bound "no AI car stands > 30 s in
  `Dynamic`" kept.
- **D5. Existing per-case rules stay unless proven subsumed.** The lane pass, the lease, repick, the box
  pass, recover / give-up and the stuck cheat are not required to be removed. The detector must replace
  per-case timers only where the plan shows the old rule is subsumed, and then only after rerunning every
  city gate's stand bounds (lesson TASK-032: narrowing a give-up timeout turned four green gates into
  53-100 s stands). Removing a rule is optional, not an acceptance item.
- **D6. Scope of waiters.** The waiters are AI traffic cars (`TrafficCar` in `Kinematic` or `Dynamic`).
  Blockers are any body in the road occupancy: vehicles of every kind (abandoned, player's, parked,
  police, AI traffic), characters (player and NPC, alive or dead), and bodies in the junction box. Police
  cars as waiters are out of scope (they have their own drive and siren lane logic); they must not regress.
  Walkers' own waiting is out of scope (they already walk around standing cars).
- **D7. Threshold T is data.** T (and any per-kind variant) lives in `assets/traffic/traffic.ron` with the
  strict loader; it must leave margin under the 30 s acceptance bound. No new tuning `const`.
- **D8. Curb path.** A curb pass for this rule may use the curb lane or mount the sidewalk edge, subject to
  static world clearance (`world_clear`-like) and to walkers and parked cars as third bodies.
- **D9. U-turn fallback.** Not built unless the universal rule is shown (by the headless rows) not to clear
  class E. If built: used only when every exit is blocked, never part of the random exit draw
  (`junction.rs` picks `out[rng]`), and it goes through the same detector trigger.
- **D10. "One body owner instead of Kinematic/Dynamic".** Evaluated in the plan only as a named option
  with a cost estimate (files touched, gates re-anchored, risk to the switch/recover invariants). Not
  required and not built in this task.

## Out of scope

- Despawning a car in view near the player; physical push-through by traffic (TASK-032).
- The conflict-point (AIM-style) reservation redesign; the capacity cost of swept conflicts.
- Police car progress logic, walker-walker avoidance, N2 (player car on top of another), P1/P3.
- Cross-platform determinism (Windows and Linux trajectories differ; each must be green on its own).

## Acceptance criteria

- [ ] **New headless rows** (production city, stationary player facing the scene, production population,
  no input; each row collects all violations before panicking):
  - M1 residue: seed 1, the player's car left on the street near (34.9, -76.9) and a traffic car bumped
    onto the centre line near (34.0, -79.2) so both lanes are blocked, player on the sidewalk looking at it.
  - N1: seed 1, the player character standing in the junction box at (7.9, -81.3) for at least 75 s.
  - Class E and class D: the existing rows below, un-ignored.
  - Each row asserts: no AI car stands > 30 s behind a stationary body in frame; 0 pass-through against any
    third body (the G1 oracle, which today covers vehicle pairs only, is extended so characters and
    static geometry also count as third bodies; the only exempt pair is (passer, its relaxed blocker)
    while the manoeuvre runs); no AI car > 30 s in `Dynamic`; the relaxed character in N1 takes no damage
    and is not knocked down. Each row reports the worst overlap depth with the relaxed blocker.
  - Fixture positions are verified against the real scene (a displaced fixture is `GATE BROKEN`, not a pass).
- [ ] **Un-ignore** `traffic_junction_box::seed_7_box_keeps_moving_liveness`,
  `traffic_causes::r1_car_left_in_the_box_seed_1` and `traffic_causes::r1_car_left_in_the_box_seed_7`;
  green on Windows and on Linux (WSL recipe: TASK-037 `scratch/wsl/`, `scratch/fixer/linux_run.sh`).
- [ ] **Flip-RED:** the detector's stationary-wait trigger, its cycle break, and the relaxation (disable
  each in turn) each turn at least one new or un-ignored row RED; a sabotage that relaxes against every
  body (not only the blocker) turns the third-body oracle RED. Recorded in the stage summaries.
- [ ] **No regressions:** `cargo test -p gta_sim -p citygen` (including `traffic_gridlock` seeds 1/2/7/42
  and every G1-asserting gate), `traffic_bench` / `police_bench` / `civilian_bench` under their
  `MEAN_LIMIT`, clippy, `cargo test -p gta_like --bin gta_like`, `python tools/qa/tree_check.py`, the 5 CI
  workflows green on the merge commit.
- [ ] **Runtime QA** (real game over BRP): the TASK-040 repros `repro_m1_centreline` (`m1_fixed.py`),
  `repro_player_in_junction.py` and `repro_r1_leave` (`repro_abandoned_car.py`), scripts in
  `maw/tasks/done/TASK-040/scratch/tools/`: no traffic car stands > 30 s behind the stationary body, and
  no car drives through a third body in the screenshots.
- [ ] **Docs:** `docs/architecture/traffic.md` gets a section on the wait-for detector and the progress
  pass, and its box section's "Open (TASK-039)" paragraph is resolved; GDD §5.2 amended: the progress pass
  as a player-visible rule (a car stuck behind a standing body squeezes past it after T, may brush it) and
  the line "Машина в коробке, на которую игрок смотрит вблизи, ... может запереть перекрёсток (открытый
  вопрос TASK-032, R1)" updated; D1 recorded there.
- [ ] **Plan artifact** names the "one body owner" option with a cost estimate (D10) and, if D9's U-turn is
  built, why the universal rule did not suffice.
- [ ] **Owner run (not gated):** how the progress pass looks when a car squeezes past a left car, past two
  cars blocking a street, and past the player standing in a junction; and the U-turn if one was built.

## Evidence index

- TASK-040: `maw/tasks/done/TASK-040/PLAYTEST_REPORT.md`, `OPEN_DECISIONS.md` (triage), `scratch/sessions/`,
  `scratch/tools/`.
- TASK-037: `maw/tasks/done/TASK-037/OPEN_DECISIONS.md`, `QA_REPORT.md`, `scratch/stage1/watch_g4_seed_7.txt`,
  `scratch/stage1/boxpass_g4_seed_7.txt`, `scratch/stage4/trace_r1_seed_1.txt`, `scratch/stage4/causes_2.txt`,
  `scratch/stage5/causes_box.txt`, probe `scratch/probe/ws/probe` (`PROBE_WATCH=<entity,...>`), `scratch/wsl/`.
- TASK-036: `maw/tasks/done/TASK-036/scratch/stage4/r1_trace_fixed.txt`, `r1_trace_head.txt`.
- Industry references (from the redesign direction): SUMO `--ignore-junction-blocker` and
  `--time-to-teleport`, AIM (Dresner & Stone), wait-for-graph deadlock detection, TM:PE despawn.
- Code: `crates/gta_sim/src/traffic/` (`junction.rs`, `box_rules.rs`, `pass.rs`, `recover.rs`,
  `contact.rs`, `manoeuvre.rs`, `stuck.rs`, `drive.rs`), `crates/gta_sim/src/occupancy/`,
  `crates/gta_sim/src/vehicle/impact.rs` (`apply_impacts`), `assets/traffic/traffic.ron`,
  G1 oracle `crates/gta_sim/tests/traffic_support/mod.rs` (`Footprints`).

## Dependencies

- TASK-037: done. TASK-036 (item 4, `connector_rects` corner overhang): done.

## Open questions

(none: resolved under the orchestrator's delegation, see Decisions)

### Resolved questions (orchestrator, after PLAN.md §5; the owner delegated these)
- M1 through the player's car: in a wait chain, relax the car whose pass has the smallest overlap with its blocker, not the chain's end car by default. A 1.3-1.8 m pass-through of the player's car reads badly. The overlap depth per row stays a reported number, and the owner's run judges the look.
- N1 through the player: a clean path (curb, in-box offset where the conflict table allows) is tried first. Relaxation against the player is the last resort, and `TnuaNotPlatform` (or an equivalent) is mandatory during it, so the player is never lifted onto the roof. Gate the Tnua case (plan row 2.11).
- A car at the start of an exit lane: out of scope. It is recorded in traffic.md as an observation.
- (After PLAN_V2.) M1: accept that car D brushes through the player's car by about 1.2 m at low speed after the minimum-overlap order has run. There is no other exit: push-through is physically impossible and despawning in frame is forbidden. The pass speed stays low (a data value), and the look is an owner-run item.
