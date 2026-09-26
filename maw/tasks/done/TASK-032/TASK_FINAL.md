# TASK-032: The street flows around the player's mess; sirens part traffic (shared occupancy rule, bumped-car recovery, police as a drama system)

Type: feature
Mode: full
Priority: high
Branch: feature/oncoming-lane
Domains: bevy-ecs, gates, game-design

Cost of error: silent. The failure classes are kinematic cars passing through each other, gridlock that only
shows over minutes, and frame budget. This task needs the full evidence layer. How the moves look (glide vs
snap, pull-over, chase feel) is judged in the owner run, not by gates.

## What this spec replaces

task.md was written in three layers. The top Description and Acceptance ask for a general oncoming-lane
reservation, and that layer is **superseded**. This spec follows the "Redesign direction" (orchestrator, after
the owner's "рескоуп и редизайн, а не стоп") and the TASK-031 playtest finding M1, which is MAJOR and meets the
run condition. Do NOT build a general oncoming-lane reservation. Normal AI traffic does not overtake moving
cars.

## Player fantasy

The city keeps flowing around the player's mess. A car left in a lane, a traffic car the player bumped, or a
police car whose crew got out does not freeze the street. Police chases feel aggressive: sirens part the
traffic, and police cars come from ahead and from the side as well as from behind.

## Evidence (read, not re-litigated)

- TASK-031 `PLAYTEST_REPORT.md` M1: an abandoned player car or a traffic car the player bumped freezes nearby
  traffic for 100-160 s, in columns of up to 16 cars. Traffic cars that a hit switched to `Dynamic` never
  resume: they stand for 87-155 s (s1/s7 tourist, s7/s42 reckless). Repro R1 is
  `maw/tasks/done/TASK-031/scratch/tools/repro_abandoned_car.py`, seed 1. The car is left at the tail of the
  queue at (2.4, -92.2) in front of junction (7.9, -81.3). The player steps 12 m away and stands for 150 s. A
  traffic car stands 146 s. In the control run (car driven 150 m on), no car stands longer than 20 s. In the s1
  tourist run the car was left about 7 m from the centre of junction (-5, 172), so the car can be in the
  junction box. Three approaches stood for 145-159 s.
- Code facts (verified):
  - `traffic/drive.rs` never switches a `Dynamic` car back to `Kinematic`. A dynamic car is only re-projected,
    or abandoned by the `lost` rule (4 m / 60 deg / upside down).
  - The forward cast skips kinematic traffic (`!kinematic.contains(&e)`). Kinematic cars see each other only
    through the per-segment `Occupancy`, so a car that leaves its lane path is invisible to other kinematic
    cars. This is the TASK-016 pass-through risk.
  - `junction.rs` keeps a grant forever for a holder on its connector (`Segment::Connector(k) => k == c`),
    and grants never check the connector path for non-AI bodies. This is the TASK-033 residue.
- TASK-016: police cars (dynamic bodies driven by `Autopilot`) get stuck behind queues, lose sight and dismount
  about 100 m away. t15 chase on seed 1 gave pressure in 3/5 runs and an escape in 2/5. The spawn filter
  `approach_clear` (pursuit only) is a workaround for "police cannot pass traffic".
- TASK-017 bench: the bench car sits in traffic queues for about 45 % of the measured window.
- City layout (`assets/world/city.ron`): lane width 3.25 m. A street has 1 lane per direction and no curb lane.
  An avenue has 2 lanes per direction: traffic drives only the inner (slot 0) lane, and the outer lane holds
  parked cars (spacing 40 m, chance 0.25). Police `pull_over: 3.25` already moves a stopping police car one
  lane to the right when that spot is free.

## Scope (this task)

### A. One shared occupancy rule (minimum that fixes M1 and the siren yield)
One spatial truth for "what occupies the road": every vehicle body (kinematic AI traffic wherever it is,
including off its lane path, dynamic traffic, abandoned, taken, player-parked, police) and every character,
behind one query API. In this task the traffic-side consumers move onto it:
1. The traffic obstacle sensing (IDM obstacle/forward cast) sees kinematic cars that are off their lane path.
   This removes the "kinematic casts skip kinematic traffic" hole.
2. Junction grants and leases see non-AI bodies on the connector path and in the box, not only on the start of
   the destination lane.
3. The traffic spawner never places a car on a spot another body occupies, a passing car included.
4. The police car lane choice (item E) reads the same rule.

The work per tick stays bounded (bevy-ecs "every system is the frame" invariant).

### B. A bumped traffic car recovers
A traffic car switched to `Dynamic` by contact goes back to `Kinematic` and drives on in its lane when all of
these hold: it is upright, within the existing `lost` thresholds of its path, has come to rest (at most
`hold_speed`) and has no dynamic body within switch reach for a data time. The return needs hysteresis in data,
so the switch and the recovery cannot flip each other every tick (TASK-016 lesson). If it cannot recover
(lost, upside down, wrecked) it becomes `Abandoned`, a plain obstacle that item C handles. Nothing stays
`Dynamic` and standing forever.

### C. Traffic passes a standing obstacle ("pull to curb + pass")
AI traffic behind a **standing** body in its lane passes it once the shared occupancy shows the pass is clear.
A standing body means one that has stood at most `hold_speed` for a data time: an abandoned or player-parked
car, a dismounted police car, an unrecovered bumped car, or a wreck. Both road classes must work. A street has
no curb lane, so the pass uses the opposite inner lane. The passer is visible to oncoming cars through the
shared occupancy, and that visibility replaces a separate reservation. An avenue has a curb lane that is
usually free. The planner picks the method for each road class. Passing moving traffic is out of scope.

### D. The junction box
A non-AI body left on a connector path or inside the box does not lock the junction. A grant holder blocked by
such a body does not keep its lease forever against conflicting waiters, and the approach whose path is blocked
does not stand forever: it passes the obstacle or takes another exit connector (planner's choice).

### E. Police as a drama system
1. **Sirens on** means a `PoliceCar` in state `Respond` or `Chase`. `Dismounted`, `Leave`, `Taken` and
   `Abandoned` mean sirens off. The sim has no siren flag; the audio emitter pick is client-only presentation.
2. **Yield**: an AI traffic car ahead of a siren car within a data distance, on the path the siren car
   approaches along, pulls to the curb side of its own lane and stops. This lateral move stays inside its own
   lane or onto a free curb lane, so it is kinematic-safe. It resumes after the siren car has passed, or after
   a data timeout. Cars do not yield to cars with sirens off.
3. **Any lane**: a police car with sirens on may drive in any lane, the oncoming lane included. Police cars
   are dynamic and collide physically. Oncoming kinematic cars must see them through the shared occupancy and
   brake. The existing time-to-contact switch covers a contact, and item B then recovers the traffic car.
4. **Spawn ahead / beside**: the dispatcher also spawns police cars ahead of and beside the fleeing player's
   heading, not only behind. This is a deliberate cheat, with the sector shares tuned in
   `assets/police/escalation.ron`. The existing hidden rule (off-frame or occluded, no overlap) and the
   per-star car caps still hold. The planner decides whether the TASK-016 `approach_clear` pursuit filter
   stays, and gate G6 checks the decision.

### F. Data, docs
- Every new tuning value (recovery time and hysteresis, standing time before a pass, pass clearance, yield
  distance and resume timeout, spawn sector shares) is data. Traffic values go in `assets/traffic/traffic.ron`,
  police values in `assets/police/escalation.ron`. Each has a strict loader and config-test rows.
- GDD §5.2 and §5.3 get one-line amendments for the pass, recovery, siren yield, any-lane and spawn-ahead
  rules. The orchestrator's delegation carries owner authority (game-design domain).
- Update `docs/architecture/traffic.md` (modes, the tick, the switch and its return, occupancy). It lists the
  migrated consumers and the ones pending in TASK-036.

### Files near the size limit
Put new code in new domain files, not these: `police/cars.rs` 716 lines, `police/car_route.rs` 659,
`police/mod.rs` 643, `population/mod.rs` 645, `traffic/graph.rs` 575.

## Out of scope: follow-up TASK-036 "Shared occupancy: remaining consumers"
Create it in `maw/tasks/pending/` when this task closes, blocked by TASK-032:
- NPC walk avoidance (`navigation::avoid_offset` / `perception::wall_blocked` see walls only; civilians have no
  `around_cars`). This includes the unconfirmed M1 side symptom where civilians stand against an abandoned car
  (s42 tourist: 10 wanderers for 90-152 s; `repro_r1_sidewalk` did not reproduce it).
- Fire line (`tactics::nearby_cars` / `car_blocks`) and sight (`perception::sight_blocked`) moved onto the
  shared API. Both already see cars through their own code, so this is unification only, with no observed bug.
- Conflict-point junction reservation instead of the whole connector (GDD §5.2 wording, deferred by TASK-016
  and TASK-033).

Traffic lights (P1) stay outside the GDD. They are not part of this task or of TASK-036.

## Acceptance criteria

Every gate goes RED under a named sabotage (flip-RED). Gates are row per case. City gates use the production
population with a stationary player facing the scene (TASK-033 lesson), and they collect all violations
before they panic.

- [ ] **G1 No kinematic pass-through (correctness).** An independent footprint oracle checks every tick of
  every G2-G6 run and of the four `traffic_gridlock` seeds, at least 8 seeded runs in total. The oracle is its
  own flat-rectangle SAT on `Position`/`Rotation`, not the occupancy code. Result: 0 ticks where two vehicle
  footprints interpenetrate while at least one of them is kinematic. The planner derives the tolerance from
  the contact margins. Flip: hide a passing or yielding car from the shared occupancy.
- [ ] **G2 Go-around.** An abandoned car stands in a traffic lane in view. One row for a street, one for an
  avenue, on seeds 1, 2, 7 and 42 (lanes found per seed). The first 8 AI cars that queue behind it all pass it
  within 60 s of joining the queue. No AI car stands longer than 30 s. There are 0 `CollisionStart` between a
  passing car and any vehicle.
- [ ] **G3 Bumped car recovery.** One row per case:
  - (a) Low-speed nudge: the car is `Kinematic` and moving along its lane within the data recovery time plus
    1 s after it comes to rest.
  - (b) Shoved beyond `lost`: the car is `Abandoned` and G2-passable.
  - (c) The player's car stays pressed against it: no switch/recover flip-flop (at most one re-switch per
    hysteresis window).
  - (d) Upside down: `Abandoned`.

  In every city gate run of this task, no AI car in `Dynamic` stands longer than 30 s (the M1 87-155 s class).
- [ ] **G4 Junction box.** A non-AI car is left on a connector path inside a busy junction. No grant holder
  that has not moved keeps its grant against a conflicting waiter for longer than `reservation_timeout` plus
  1 s. No AI car on any approach stands longer than 40 s (the TASK-033 saturation bound).
- [ ] **G5 Siren yield.** A siren car comes up behind a queue of 6 AI cars on a street lane.
  - Every queued car whose nose is within the yield distance shifts toward the curb and stops.
  - The police car passes all 6 cars.
  - Each yielded car is driving its lane again within the data resume time after the police car's rear passes.
  - Row with sirens off (`Leave`): nobody yields.
- [ ] **G6 Police close in.** The player's car drives away at 12 m/s (street v0) with an AI queue between it and
  a police car behind. A police unit (car or on foot) comes within 18 m (t15 `PRESSURE_M`) within 25 s (t15
  `CHASE_S`) in at least 8 of 10 seeds. Flip: yield and any-lane disabled gives RED.
- [ ] **G7 Spawn ahead/beside.** At 2★ or more with the player driving, the dispatched police cars cover the
  behind, ahead and beside sectors by the data shares (one row per sector). Every spawn is hidden and overlaps
  no body. The per-star car caps hold.
- [ ] **G8 Occupancy migration (one row per consumer migrated here).** Each row goes RED when that consumer's
  occupancy input is removed:
  - Traffic sensing sees (a) a kinematic AI car off its lane path, (b) an abandoned, taken or police car,
    (c) a character.
  - Junction grants see a non-AI body on the connector path.
  - The spawner rejects a spot a passing car overlaps.
- [ ] **G9 No regressions.** These stay green:
  - `cargo test -p gta_sim -p citygen`, including every `traffic_*`, `police_*` and `vehicle_*` gate, and
    `traffic_gridlock` on seeds 1/2/7/42 with its 40 s bound.
  - `traffic_bench` and `police_bench` under their `MEAN_LIMIT`.
  - `cargo clippy`, `cargo test -p gta_like --bin gta_like`, `python tools/qa/tree_check.py`.
  - All 5 CI workflows on the merge commit.
- [ ] **R1 runtime (QA).** Run the TASK-031 repro, seed 1, with `leave=1`. Over the 150 s stand, no traffic car
  within 45 m of the junction stands longer than 30 s (per-car stand durations). The control run (`leave=0`)
  keeps 0 cars over 20 s. Civilians stuck within 8 m of the car are reported, not asserted.
- [ ] **t15 runtime (QA).** `tools/qa/scenarios/t15.py` gets a `--seed` argument (default 1). Run seeds 1, 2
  and 3, two runs each. Chase pressure must show in at least 5 of 6 runs (TASK-016 baseline on seed 1: 3/5).
  The existing asserts stay green: hijack, and at most 2 active police cars at 2★.
- [ ] Data, GDD amendments and `docs/architecture/traffic.md` done as in item F. TASK-036 created.
- [ ] **Owner run** (not gated): the look of the recovery, the pull-over and the pass, and whether chases feel
  aggressive.

## Dependencies
- blocked by TASK-031 (done; M1 meets the run condition)

### Resolved questions
1. **Which layer of task.md wins?** The redesign direction and M1 win (binding orchestrator note), and the
   top-layer oncoming reservation is superseded. Reason: the owner said "рескоуп и редизайн, а не стоп", and
   TASK-016 stopped twice on that exact mechanism.
2. **Migrate every consumer now, or split?** Split. This task migrates only the traffic-side consumers:
   sensing, junction grants, spawner and police lane use. M1 and the siren yield need only those. Fire line
   and sight already see cars through their own code (`nearby_cars`/`car_blocks`, the Vehicle mask in
   `sight_blocked`), so migrating them now fixes no observed bug. Walk avoidance has only an unconfirmed
   symptom. All three go to TASK-036, so the task ships.
3. **What does "pass" mean without an oncoming reservation?** The shared occupancy includes kinematic cars
   anywhere, so a passer on the opposite lane is an ordinary obstacle to oncoming cars. That is the whole
   unification. Only standing obstacles are passed. Overtaking moving traffic stays out, which keeps the
   TASK-016 risk bounded. The road-class facts come from `city.ron`; the method per class is the planner's.
4. **When are sirens on?** When the `PoliceCar` state is `Respond` or `Chase`. The sim has no siren state, and
   the audio emitter pick is presentation (`src/audio`), which gameplay must not read.
5. **Recover or abandon?** Reuse the existing `lost` thresholds and the upright check. Add rest time and
   hysteresis as data. Anything that cannot recover becomes `Abandoned` and is passed by C. This follows the
   orchestrator note: "nothing stays Dynamic and blocking forever".
6. **Where do the numbers come from?**
   - 30 s stand bound: the orchestrator note and the R1 gate.
   - 40 s junction bound: TASK-033 saturated-junction bound.
   - 8 cars: task.md.
   - 60 s pass window: 8 cars at about 7.5 s each, a floor.
   - 18 m and 25 s: t15 `PRESSURE_M` and `CHASE_S`.
   - 5/6 runs: above the TASK-016 baseline of 3/5.

   The planner may tighten any of these from a worked example. Loosening one needs an OPEN_DECISIONS entry
   (gates rule: numbers are derived).
7. **Civilians against the abandoned car?** Unconfirmed: the dedicated repro got 0 stuck. It is reported in R1
   and fixed in TASK-036 if QA confirms it. No machinery for an unconfirmed symptom (evidence weight
   proportional to the cost of the error).
8. **t15 over seeds 1-3?** The script hard-codes `SEED = 1`, so a `--seed` argument is in scope. That is a
   script-only change.
9. **Spawn ahead vs the camera.** Ahead of a driving player is usually in frame, so ahead and beside spawns
   obey the existing hidden rule (off-frame or occluded). In practice they come from cross streets ahead. No
   pop-in.
10. **The R1 script.** It is gitignored scratch, and its `sys.path` still points to `in_progress/TASK-031`. QA
    points it at `done/TASK-031/scratch/tools`, adds per-car stand durations, and runs it locally. Results go
    to `maw/tasks/in_progress/TASK-032/scratch/`.
11. **Police hitting traffic in the oncoming lane.** This is allowed and GTA-like, because the contact is
    physical (dynamic). The TTC switch and B cover the traffic car. G1 counts only interpenetration with a
    kinematic participant. G2's zero-collision rule applies to traffic passers, not to police.
12. **GDD scope.** The pass, recovery, yield, any-lane and spawn-ahead rules are new mechanics. They enter via
    GDD §5.2/§5.3 one-line amendments under the orchestrator's delegated approval, as TASK-016 did for
    pull-out.

## Open questions
(none)

## Premise challenge resolution (orchestrator, 2026-09-26, binding)
PREMISE_CHALLENGE.md returned SUSPECT on the evidence, not on the class. The R1 repro's 146 s car stood behind the PLAYER standing on the carriageway, 4.5 m ahead, not behind the abandoned car. The control run moved the player beyond the 90 m despawn radius, so it did not isolate the cause. M1 as a class still holds: in s1_tourist, s7_tourist, s7_reckless and s42_reckless, 5-13 cars stood for over 60 s while 3-6 bumped `Dynamic` traffic cars were alive. Amendments:
1. **Causal evidence first.** Before building A-E, the implementer writes headless repros that reproduce each freeze cause in isolation and show RED on the current code: (a) an abandoned player car in a lane; (b) a bumped traffic car left `Dynamic`; (c) a character standing on the carriageway in a lane. Any cause that does not reproduce is dropped from scope and reported. The session data in `D:/test-gta-like/maw/tasks/done/TASK-031/scratch/sessions/` is the source to mine for positions and states.
2. **A standing character is an obstacle kind in the same shared rule.** The GTA fantasy is that traffic stops for a pedestrian in the road, then goes around after a short wait. It never stands behind them forever. Go-around (C) therefore treats "standing body" as a vehicle OR a character that has not moved for `go_around_wait_s` (data). The wait before going around a character may be longer than for a car. The player walking in front of a moving car is still yielded to, and no one gets run over by the go-around manoeuvre: the manoeuvre's clearance check sees characters.
3. **R1 is fixed before it is a criterion.** The player stands OFF the carriageway, at the same point in the leave run and the control run, within the in-view radius. Per-car stand durations are logged, not only counts. The script lives at the task's own scratch path (fix `sys.path`). R1 green means no traffic car within 45 m stands longer than 30 s in the leave run, with the control run as baseline.

### Resolved questions (orchestrator, after PLAN.md)
- The planner asked whether the pass claim is an oncoming reservation in disguise. No. The claim is a footprint in the same `RoadOccupancy` snapshot that every consumer already queries, not a second bookkeeping system with its own grant and expiry rules. Keep it. If the oncoming pass fails twice with the same class (head-on deadlock or pass-through), switch to the named fallback: footprint-only passers plus the stuck-despawn cheat out of view.
- (After PLAN_FINAL.) Recovery predicate: accept the sweep predicate with hysteresis (skin 0.4, horizon 0.5) instead of the literal "no dynamic body within switch reach 5 m". The literal version makes G3(a) unreachable, because the bumping car stands closer than 5 m. The intent is the same: no body will hit the car when it turns kinematic.
- (After PLAN_FINAL.) G6 feasibility, decided before probe 6.0 runs:
  - If the probe shows that a police car cannot pass between two cars yielded to the curb on a street, then on streets the siren car uses the oncoming lane (sirens on means any lane), and oncoming traffic yields to its own curb too.
  - If that is still infeasible on streets, G6 is scoped to avenue routes (≥ 8/10). Street routes are reported as numbers only, and the t15 runtime chase metric (≥ 5/6) remains the player-facing criterion.
  - Record the probe numbers in IMPL_SUMMARY.
