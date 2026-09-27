# TASK-037: In-view junction box lock — blocked approaches re-route, U-turn or pass through the box

Type: fix
Mode: full
Priority: high
Branch: feature/box-lock-in-view
Domains: bevy-ecs, gates, game-design

Cost of error: silent. The class is gridlock that only shows over minutes, in front of the player.

## Description
A car left inside a junction box that the player is looking at from nearby locks the junction. The
approaches queue behind it and stand for about 150 s. TASK-032 fixed the other freeze causes (a car left
in a lane, a bumped `Dynamic` car, a character in the lane, the box out of view via the stuck cheat), but
not this one. This lock predates TASK-032: it shows on main too.

Why it is still open:
- The stuck cheat (`traffic/stuck.rs`, fallback R-B) never removes a car the player sees within
  `bubble.stuck_in_view_distance` (40 m). That is correct: nothing pops in view.
- Push-through (R-A) failed twice in TASK-032 fixer round 1 (`maw/tasks/done/TASK-032/FIX_SUMMARY.prev-1.md`
  section 2). An unmanned sedan holds its wheels below `hold_speed`, and its tyres grip sideways at about
  µ·m·g ≈ 16 kN, while a sedan's drive force is m·a ≈ 6.9 kN. A pusher only moves the body by ramming: it
  gets stuck, reverses, rams again and reverses into the car behind it. Walkers on the crosswalk get
  pinned between the pusher and the left car at gap 0. A kinematic "bulldozer" pusher is rejected: it
  drives a pinned body through, the G1 pass-through class by construction.
- A third box-pass patch was refused by the stop rule (TASK-032 `OPEN_DECISIONS.md`, 2026-09-27, option A).

### Evidence (all from TASK-032, `maw/tasks/done/TASK-032/`)
- **Runtime R1, spot A** (seed 1, player at (7.2, -50), 31 m from junction (7.9, -81.3), car left at
  (8.2, -82.5), 150 s; count of traffic cars within 45 m standing > 30 s, longest stand in brackets):
  - main 861bd90 (before TASK-032): 1 (148.2 s), 1 (148.9 s). One car on the east approach.
  - TASK-032 branch, fixer round 2: 3 (149.1 s), 12 (149.0 s), 2 (144.9 s). The east approach queue
    (x 11-24, z -81), with one `Dynamic` car at the box entry; in the 12-car run the north and south
    queues too.
  - Spots B (5.3, -84.5) and C (0.7, -86.7) are 0/0/0 on the branch (main: B 0, C 3-5). Script:
    `scratch/qa/run_r1.py`, results `scratch/qa/r1/`, table in `FIX_SUMMARY.prev-2.md` section 3.
- **Headless R1** (`crates/gta_sim/tests/traffic_causes.rs`, `r1_car_left_in_the_box_seed_{1,7}`,
  ignored with a pointer here): the car at the hub of the box nearest the spawn, the player 31 m away
  facing it, 150 s. Seed 1: 149.4 s worst stand, one car 149.4 s in `Dynamic`, about 20 cars over 30 s
  within 45 m. Seed 7: 147.8 s worst, 122.4 s in `Dynamic`.
- **G4 liveness rows** (`crates/gta_sim/tests/traffic_junction_box.rs`, `*_liveness`, ignored with a
  pointer here): the car placed at 20 s on the busiest connector of the nearest box, the player 25 m
  away watching it. Seed 1: approach stands up to 97.7 s, 95.5 s in `Dynamic` (two cars at the box
  entries, `Connector(719)` s 0 at (-4, -86) and the end of lane 299 at (0, -72), both led, so they never
  give up or recover). Seed 7: 94.8 s, 77.0 s in `Dynamic`. Extra row (an AI car on the blocked
  connector right behind the body): stands 100 s. The lease assertion and G1 stay green in the
  non-ignored rows.
- **Node 141** (`traffic_causes::c_character_in_the_lane`, seed 1): a `Dynamic` grant holder on
  `Connector(1296)` (lane 462 → 510, s 2.4) in the box at the start of the scene lane stands 53.5 s. It is
  the sole occupant with 4 waiters. Pinned walkers stand around it 1.9-2.3 m away for 5-15 s: they block
  its recovery sweep and flicker in its `led` strip, so `stood` keeps resetting. The walker side is
  TASK-036 (walk avoidance); the box side is here. The row asserts the `Dynamic` bound on the scene lane
  only and prints this stand.

### Redesign direction to try
The lock is geometry: two bodies in a box leave no pass corridor, and nothing owns that case. Try, in
this order, and let the planner pick:
1. **The blocked approach queue re-routes.** A queue head whose connector path is blocked by a standing
   body re-picks another exit connector (`box_rules` already re-picks at the connector start within
   `REPICK_WITHIN`); extend it to cars further back in the queue, or let a car U-turn on a two-way street
   when every exit is blocked.
2. **Pass through the box.** A car on its connector goes around a standing body in the box with a claim
   in `RoadOccupancy`, as lane passes do, when the box geometry has room.
Physical push-through is out (numbers above). Removing the car in view is out (pop-in).

## Acceptance criteria
- [ ] Un-ignore `traffic_junction_box::*_liveness` (3 rows) and `traffic_causes::r1_car_left_in_the_box_seed_{1,7}`;
  all green: no AI car on the box approaches stands > 40 s (G4) / no traffic car within 45 m stands > 30 s
  (R1), and no AI car stands > 30 s in `Dynamic`.
- [ ] `traffic_causes::c_character_in_the_lane` asserts the `Dynamic` bound city-wide again (the node-141
  holder, together with TASK-036's walk avoidance if the walkers keep it pinned).
- [ ] Every new mechanism flips RED under a named sabotage; G1 oracle clean in every touched city gate.
- [ ] Runtime R1 (QA): spot A median ≤ main's (1 car over 30 s), spots B and C stay 0, control 0.
- [ ] No regressions: `cargo test -p gta_sim -p citygen` (including `traffic_gridlock` seeds 1/2/7/42
  within 40 s), `traffic_bench` / `police_bench` under their `MEAN_LIMIT`, clippy, client tests,
  `tools/qa/tree_check.py`, the 5 CI workflows.
- [ ] `docs/architecture/traffic.md` box section updated; GDD §5.2 one-line amendment if a new rule
  (re-route, U-turn, box pass) is player-visible.
- [ ] Owner run (not gated): how the re-route or box pass looks.

## Dependencies
- blocked by TASK-032
- related: TASK-036 (walk avoidance: the walkers pinned at node 141)

## Added at TASK-032 closure (QA round 2)
- A second shape of the same lock: a car abandoned within a pass length of a stop line, in view. `pass.rs` refuses a pass whose manoeuvre reaches the junction, so 7 cars stood 64-89 s behind the player's car at the lane 302 stop line (`D:/test-gta-like/maw/tasks/done/TASK-032/scratch/qa2/tourist_s1_pressed/`). The same happens on main.

## Added by TASK-038 (orchestrator)
- The "Dynamic car at a box, walkers pinned at its nose" lock also shows in `traffic_causes::rb_a_box_car_seen_from_afar_is_cleared` (98.8 s, both platforms). TASK-038 scoped their `Dynamic` bound to the scene approaches, like (c) and the G4 liveness rows. Acceptance item for this task: restore the full-city `Dynamic` 30 s bound in causes (c), rb and the G4 liveness rows.

## Premise challenge resolution (orchestrator, 2026-09-27, binding; supersedes "Redesign direction to try")
PREMISE_CHALLENGE.md (SUSPECT) showed that the lock is not missing geometry. A box pass (`plan_box_pass` plus the whole-box grant) and `repick` already exist. They only act on `Kinematic` cars (`manoeuvre.rs:115`; `junction.rs:204-205` skips Dynamic cars on connectors). The approach heads stand in `Dynamic` at the box entry. A `Dynamic` car on a connector can never recover (`recover.rs:144-145` requires `Segment::Lane`), and since it is on its line it never gives up either (`recover.rs:156`). So re-route, U-turn and a new box pass would leave the gates red.

Rescoped task (the missing shared rule: a bumped car can always return to the kinematic world, wherever it stands):
1. **Recovery on connectors.** A `Dynamic` car on a connector recovers like a lane car: the path is the connector polyline, the rejoin corridor is along the connector, and the same sweep and hysteresis apply. After recovery it rejoins the existing box rules (grant, lease, repick, box pass).
2. **Explain first.** Before building, a headless trace per liveness row names why each head went `Dynamic` (contact with the left car, a walker, or a follower) and which existing box rule would have resolved it once kinematic. If a rule still does not fire for a kinematic head, fix that rule; do not add a parallel one.
3. Re-route to another exit or a U-turn only if (1)-(2) leave a row red. Name it as the fallback, used only after two failures of the same class.
4. The acceptance criteria above stand unchanged. They are the proof.

### Resolved questions (orchestrator, after PLAN.md)
- Q1: fold TASK-036 item 1 (walkers avoid cars through the existing `tactics::around_cars` over `RoadOccupancy`, with the police values) into this task. It is the dominant class A (G4 seed 1, G4 extra, rb, (c) node 141, R1 seed 1), and the car side cannot break that wait. TASK-036 keeps its other items (fire line/sight migration, `connector_rects` swept bodies, oracle lateral).
- Q2: the walker goes around the car. Turning back is the fallback, used only after two failures of the same class.
- Q3: if the stage-4 trace confirms class D in R1 seed 1 is the fixture teleport (a body placed 0.31 m in front of a car moving at 2.8 m/s), clear the spot before placing the body, as G4 does, and record it. If D persists after that, apply the two-failures rule, and re-route or U-turn is the named fallback.
- Q4 (plan review R7, class D in (c)): re-trace after stage 3 first. If 1919v1 still sits inside `recover.skin` of 2002v0, apply option B: a smaller recovery skin for bodies at rest (derived, not guessed: at-rest bodies cannot close the gap during the switch horizon). Other options go through the two-failures rule.
