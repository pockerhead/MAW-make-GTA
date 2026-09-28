# PLAN — TASK-039: universal traffic progress guarantee

Cost of error: silent (gridlock that shows over minutes, in front of the player). Full evidence layer:
headless rows with flip-RED on Windows and Linux, the third-body oracle, runtime QA on the TASK-040 repros.

## 1. Understanding

### What stands today and why (code, not comments)

- `traffic/drive.rs::advance_traffic` (lines 202-662) is the whole tick: snapshot of AI cars sorted by
  entity bits (245-268), `recover_dynamic` for `Dynamic` cars (270-304), path occupancy (307), junctions
  (`junction::update`, 328-344), then per car IDM on the path leader (`leader`, 102-129, returns gap and
  speed only), the stop line, the pass hold and the two sensing strips (`manoeuvre::sense`, 124-169),
  then `manoeuvre::plan` (181-264), then kinematic motion or autopilot targets (444-593). The file is 662
  lines (warning at 750).
- `recover.rs::recover_dynamic` (193-242) recovers only when `corridor_clear` (113-148) and
  `nobody_coming` (79-108) hold; it gives up only when `!led` (173-190). A `Dynamic` car pinned against a
  body at rest waits forever by design (PREMISE_CHALLENGE §2, executed class D: 113 s in `Dynamic`).
- `pass.rs::passable` (292-303) + `plan_pass` (455-548): the lane pass needs a side whose claim holds no
  body but the obstacle; `manoeuvre::plan` runs it only for an `idle` kinematic car (197-200).
- `box_rules.rs::connector_clear` (46-64) skips characters and granted AI cars; `repick` (69-84) takes
  the next clear exit; `plan_box_pass` (122-238) needs `exit.left_gap` and a clear side; `plan` refuses a
  box pass around a character (`manoeuvre.rs:253`).
- `junction.rs::update` (314-602): lease/demotion (348-449), requests and repick (450-526), the per-waiter
  path check map `blocked: HashMap<(Entity,u32), Option<f32>>` (539-547, keeps only the standing time,
  not the body), grants with the whole-box rule (548-601). A holder stopped by a walker keeps its grant
  (connector_clear skips characters: N1).
- `contact.rs::switch_to_dynamic` (412-527): predictive sweep over dynamic bodies (458-489) and the
  `CollisionStart` backstop (490-505).
- `vehicle/impact.rs::apply_impacts` (446-552): every `CollisionStart` between a car and a live character
  charges damage and may knock down (`HitReaction::KnockedDown`, `combat/melee.rs:251`).
- `stuck.rs::despawn_stuck` (623-678): the only universal escape, off in frame near the player.
- `occupancy/mod.rs::snapshot_road` (105-190): `RoadBody.standing` resets on motion and on displacement
  (the TASK-032 teleport lesson); bodies only within `in_view.despawn + look_ahead` of the player.
- `TrafficCar` (`traffic/mod.rs:67-90`) is constructed literally only in `spawn.rs:63-75`;
  `clear_ai_state` (`mod.rs:256-264`) resets AI fields.
- Physics composition: `lib.rs:194` `PhysicsPlugins::default()`; vehicles carry one convex-hull chassis
  collider on the body entity (`vehicle/mod.rs:153-177`).
- G1 oracle: `tests/traffic_support/mod.rs:255-351` (`Footprints`) checks vehicle pairs with at least one
  kinematic side only; `StandClock` (358-465) measures continuous stands (< `hold_speed`) and `Dynamic`
  stands.

### The four residues mapped to one missing relation

| Row | Waiter | What it waits on | Why nothing ends it |
|---|---|---|---|
| M1 residue (TASK-040 `repro_m1_centreline`) | D (traffic car on the centre line, `Dynamic` 97 s); N (eastbound, 27.9,-77.5) and S (westbound, 40.2,-80.4) `Kinematic` 92.5 s | D: its rejoin corridor meets P (player's car), `led` by P, so no recovery and no give-up. N: path leader D. S: sensing hit D | N's and S's opposite-lane claims hold D/P/the other queue (`pass.rs` `taken`); D is never passable-around |
| N1 (`repro_n1_player_in_junction`) | a holder on its connector | the player (character) on its swept path (`sweep_ahead`) | box rules skip characters, the lease keeps a grant held by a walker, no box pass around characters |
| Class E (G4 seed 7, R1 seed 7) | queue head at its stop line | the left car on every exit's swept path (`blocked`) | repick none, whole-box pass fits no side, lease lapses and the box rotates |
| Class D (R1 seed 1) | 1853v0 `Dynamic` on its connector | the left car within the rest skin | `corridor_clear` fails at k 0, `led` holds give-up |

Every row is "a standing car whose chain of waits ends at a body that stands", which is exactly what an
explicit wait-for record can see and what a relaxed pass can end.

### Constraints that shape T (orchestrator side finding)

- `traffic_causes::b3_dummy_pressed_at_the_bumper` (`traffic_causes.rs:39` `PRESSED_S = 20`, 176-222,
  251-259) fails if the car leaves `Dynamic` while the dummy stands < 20 s.
- `traffic_recovery::c_pressed_car_does_not_flip_flop` (14 s pressed, `traffic_recovery.rs:203`) and
  `e_blocked_corridor_waits_in_the_queue...` (14 s, :314) must not see a recovery while pressed.
- Every new/un-ignored row bounds stands at 30 s; the G4 liveness row at 40 s (`MAX_STOP`).

### Verified engine facts (pinned sources)

- avian3d 0.7.0 `collision/hooks.rs:147-190`: `CollisionHooks: ReadOnlySystemParam` with
  `modify_contacts(&self, &mut ContactPair, &mut Commands) -> bool`; registered with
  `PhysicsPlugins::with_collision_hooks::<H>()` (`lib.rs:701`, `PhysicsPluginsWithHooks` is a
  `PluginGroup`, `lib.rs:830`). `ContactPair` has `body1`/`body2: Option<Entity>`
  (`contact_types/mod.rs:163-165`).
- `narrow_phase/system_param.rs:774-779`: `modify_contacts == false` clears the manifolds and the
  `TOUCHING` flag, so no `CollisionStart` (a pair touching before gets `STOPPED_TOUCHING`) and no solver
  constraint. It runs every step for pairs flagged `MODIFY_CONTACTS`; the flag is set at pair creation
  from either collider's `ActiveCollisionHooks` (`bvh_broad_phase.rs:194`, `collider_tree/tree.rs:93`),
  so the component must be on the traffic car from spawn. `filter_pairs` runs only for new pairs
  (`bvh_broad_phase.rs:291`) and would miss a pair already touching (class D, b3): not used.
- bevy-tnua-avian3d (vendored 0.12.1) `lib.rs:250-321`: the ground sensor shape-casts with only the owner
  excluded; spatial queries ignore hooks, so a car driving through a character could be seen as ground.
  An entity with `TnuaNotPlatform` (bevy-tnua 0.32 re-export of
  `physics-integration-layer 0.13.0 data_for_backends.rs:302`) is skipped (`lib.rs:250`).
- No new crate: nothing to fetch; `Cargo.lock` unchanged.

### Research (sources)

- SUMO `--ignore-junction-blocker <T>`: waiting vehicles ignore a vehicle that has stood T on an
  intersecting lane and drive on ("eventually finding a way around the offending vehicle"); the time is
  the BLOCKER's standing time. https://sumo.dlr.de/docs/Simulation/Intersections.html
- SUMO `--time-to-teleport` (default 300 s): the waiting counter counts steps below 0.1 m/s and resets
  when the vehicle moves; causes are classified (jam, yield, blocked, ...).
  https://sumo.dlr.de/docs/Simulation/Why_Vehicles_are_teleporting.html
- Wait-for graph: deadlock iff a cycle; with one outstanding wait per node the graph is functional
  (out-degree 1), so a pointer walk finds every cycle in O(n). https://en.wikipedia.org/wiki/Wait-for_graph
- Taken from these: SUMO's rule gives the grounding for "after T, ignore the blocker only"; the waiting
  counter reset-on-motion matches our `RoadBody.standing`; a single out-edge per car makes the detector
  O(n) and deterministic. Not taken: teleport/despawn (D1 forbids in view).

## 2. Approach

One mechanism in two parts, both in a new domain file `crates/gta_sim/src/traffic/progress.rs`:

**A. Wait-for record (every tick, inside `advance_traffic`).** Each AI car in `Kinematic`/`Dynamic`
whose road body stands (`RoadBody.standing > 0`) gets at most ONE out-edge, the reason it stands:

| Edge | Source | Kind |
|---|---|---|
| path leader within `pass.trigger_gap` (`leader`, returns its entity now) | lane queue | `Follow` |
| oncoming claim hit (`Hit.claim`) | waits for a passer | `Follow` |
| nearest sensing hit (`ahead`/`beside`, not a claim) within `pass.trigger_gap`; the pass hold's obstacle | body in its path | `Body` |
| at its stop line without a grant: body on its connector path (`blocked` map, now with the entity) | box blocker | `Body` |
| at its stop line without a grant: holder (or earlier queued waiter) of a conflicting connector | grant | `Grant` |
| at its stop line without a grant: waiting for room (exit-lane tail car or `lane_start_free` body) | room | `Follow` |
| `Dynamic`: path leader within `2 * idm.min_gap` | queue | `Follow` |
| `Dynamic`: first body that fails `corridor_clear` / `nobody_coming` / `led` | recovery blocker | `Body` |

Nearest gap wins; a tie goes to `Body`. Bailing, abandoned, taken, police, parked cars and characters
never have out-edges (sinks).

**Triggers** (both deterministic, entity order):
1. *Sink*: waiter W with a `Body`/`Grant` edge to B, where B has no out-edge, and
   `min(standing(W), standing(B)) >= progress.wait_seconds` (T). W relaxes against B.
2. *Cycle*: pointer walk over the functional graph; a cycle whose every member has stood
   `>= progress.cycle_seconds` is broken by the member with the lowest `Entity::to_bits()` among those
   with a `Body`/`Grant` out-edge; it relaxes against its successor.
`Follow` edges never relax: a car never ghosts into its queue leader or into the box for room.

**B. Relaxation (the progress pass).** `TrafficCar.relaxed: Option<Relax { blocker, since }>`. While set,
for that pair only:
- sensing (`manoeuvre::sense` skip), the box path check (`connector_clear`, `repick`), the lease's
  `body_blocked`, recovery (`corridor_clear`, `nobody_coming`, NOT `led`, so a relaxed car never gives
  up), the contact switch (predictive and backstop) ignore B;
- a `Grant` relaxation also lets the grant ignore B's conflicting connector;
- physics: the `TrafficHooks` collision hook returns `false` for the pair (no push either way, no
  `CollisionStart`, hence no `apply_impacts` damage/knock-down: D3);
- motion is the existing law: a relaxed car simply drives its own path line (lane line or connector
  line). "Prefer clear paths" (D2) is kept by order: the clear planners (lane pass, box pass, repick) run
  every tick for T seconds before any relaxation, a relaxed idle car on a lane still tries the clear lane
  pass around B first (a recovered `Dynamic` car could not before), and a head that may still re-pick (at its stop line, or
  within `REPICK_WITHIN` of its connector start) takes the exit of least overlap with B among exits clear
  of every other body (0 overlap first). A `Pass { go: false }` waiting for its claim is dropped at the
  relaxation (its hold would keep the car standing).
- End: B gone from the snapshot, or B moved (`standing == 0`), or the car is past B (every corner/rim
  point of B behind the car's rear line), or the car left `Kinematic`/`Dynamic` (`clear_ai_state`), or
  `progress.max_seconds` elapsed since `since` AND the car's footprint grown by `recover.skin` no longer
  touches B (never ends inside B, so no re-switch and no impact at the end).

Why this shape:
- In the box a relaxed car stays on its connector line: the conflict table (TASK-038 swept oracle) keeps
  co-granted bodies apart only on the lines; an offset needs the whole box, whose drain under the lease is
  the seam that failed in TASK-032/037. Third bodies stay safe through the unchanged rules (sensing skips
  only B; grants still check every other body; walkers are still seen).
- Sink-only triggers give one relaxation per chain (the car directly before the standing body), so a
  queue never ghosts into itself. Consequence (reported, owner-judged): in M1 D is the chain end and
  drives through P with an estimated 1.3-1.8 m lateral overlap (S's queue blocks every less-overlapping
  offset); N then passes P with the existing lane pass.
- T = 18 s, re-anchoring b3 (see §5 decision). Justification: floor = the longest existing "must wait"
  window (14 s, `traffic_recovery` c/e) + 4 s; ceiling = 30 s minus the start latency (recover 1.5 s,
  a grant, an oncoming queue clearing: estimated <= 8 s for M1-N, measured in stage 3). GTA drivers go
  around a pedestrian who blocks them, so b3 becomes a progress row (orchestrator note).
- U-turn (D9): not built; built only if stage 3 shows class E not cleared by the rule.

D10, "one body owner" (named option, not built): always-`Dynamic` traffic driven by the autopilot (or
always-kinematic with custom response). Touches `drive.rs` motion, removes `contact.rs` and `recover.rs`,
reworks `lateral.rs`, `manoeuvre.rs`, `pass.rs`, `box_rules.rs` targets, `vehicle/autopilot.rs` precision,
`occupancy` kinds, `spawn.rs`, sirens/police interplay: about 12-15 source files. Gates re-anchored:
`traffic_contact` (6 rows), `traffic_recovery` (9), `traffic_idm`, `traffic_intersection` (conflict
points with overshoot), `traffic_graph` oracle assumptions, the G1 kinematic tolerance, `traffic_bench`
`MEAN_LIMIT` (24 wheel-raycast bodies), and every city gate trajectory: about 25 gate files, 3-5 tasks.
It does not remove the need for a progress rule (TASK-032: traffic cannot shove a parked car). Not
recommended now.

## 3. Steps

Stages end green; each stage summary records its flips (input perturbed, RED seen, restored, GREEN).
All probe builds use `CARGO_TARGET_DIR=D:/test-gta-like/target`; `touch crates/*/src/lib.rs` before
trusting a red built from the probe workspace (TASK-009 lesson).

### Stage 0 — traces on HEAD (no production change)

0.1 Probe crate `maw/tasks/in_progress/TASK-039/scratch/probe/ws/probe` copied from
`maw/tasks/done/TASK-037/scratch/probe/ws/probe` (same `Cargo.toml` patches, `#[path]` to
`tests/common/mod.rs` and `tests/traffic_support/mod.rs`). Read-only: recompute from public state
(`RoadOccupancy::first_along`/`blocked`, `TrafficIntersections`, `TrafficCar`, positions).

0.2 Traces, one per row, once per second, for every AI car standing > 5 s within 45 m of the scene:
segment, s, mode, manoeuvre, lateral, grant/waiter/whole, speed, road `standing`; its would-be out-edge
by the table in §2 (kind and target entity); the chain end and whether it is a sink or a cycle; for a
head at a stop line, per exit: clear of third bodies (y/n) and overlap depth with the chain end. Rows:
class E (`traffic_junction_box` seed 7 scene, `traffic_causes` R1 seed 7), class D (R1 seed 1), M1 and
N1 fixtures (0.3), b3 (to time the trigger). Output under `scratch/stage0/`.

0.3 Build the two new fixtures in the probe first (they move into `tests/traffic_progress.rs` in
stage 3) and show both RED on HEAD:
- M1 (seed 1): lanes found through `TrafficGraph::nearest` at (34.4, -77.5) (eastbound) and (40.2,
  -80.4) (westbound), refuse if they are not opposite (`GATE BROKEN`). `clear_spot((34.5,-78.5), 10)`.
  D = `spawn_traffic_car` on the eastbound lane with its centre at the along-lane position of x 34.0,
  switched through a real contact (the `bumped` pusher pattern of `traffic_causes.rs:143-170`, rear
  nudge at 2 m/s, pusher despawned after the switch), then `teleport`ed (named mutation) to
  `eastbound line - 1.85 m` across (the centre line side) with its heading. P = `park_car` at
  `eastbound line + 0.6 m` across at the along position of x 34.9 (the curb side), heading along the
  lane. The player `stand_player` on the north sidewalk (`sidewalk_at` nearest to (35.0, -70.5)) facing
  (34.5, -78.5). After 2 s: `GATE BROKEN` unless D is `Dynamic` and both bodies are within 0.5 m of
  (34.0, -79.2) / (34.9, -76.9), P and D do not overlap, and both lanes are blocked (N's opposite-lane
  claim and S's opposite-lane claim both hold a body: recomputed with `blocked`). Feeders on both lanes
  (`Feeder::new(lane, 4*HZ, 8)`), 120 s.
- N1 (seed 1): `stand_player` at (7.9, -81.3) facing the box's busiest approach; `GATE BROKEN` unless
  `graph.in_junction(p, 0.0)` and at least one connector body sweep of that node (`connector_body`
  equivalent in the test, 0.1 m steps like `traffic_junction_box::on_path`) overlaps the capsule; the
  player is held still (no input) for 100 s.
- Cycle floor (see 3.6): confirm on HEAD that the two cars stand > 30 s with no sink in their chains.

0.4 Record in the stage-0 summary: where N1's waiting car stands (stop line or mid-connector), whether
1853v0 in class D is inside the conflict-table band (`recover_dynamic` `on_path`), which of M1's D/N/S
is the sink's waiter, and whether any row shows a cycle. If class D's car is outside the band, the
relaxed recovery cannot fire: stop and write a redesign note before stage 2 (the rule would need a
recovery path out of the residual band).

### Stage 1 — third-body oracle, before any behaviour change

1.1 `crates/gta_sim/src/traffic/mod.rs`: add `#[derive(Reflect, Clone, Copy, Debug, PartialEq)] pub struct
Relax { pub blocker: Entity, pub since: u64 }` and the field `pub relaxed: Option<Relax>` to
`TrafficCar` (doc: "the progress rule lets it past `blocker` since fixed tick `since`"); register the type;
`clear_ai_state` sets it `None`. `spawn.rs:63-75` literal gets `relaxed: None`. Nothing writes it yet
(no behaviour change).

1.2 `crates/gta_sim/tests/traffic_support/mod.rs` `Footprints::record` (279-322): a pair is exempt when
either car's `TrafficCar.relaxed.blocker` is the other entity (read each tick); exempt pairs feed
`relaxed_max_depth` (reported by every new row), never `violations`. Keep `mod.rs` under 750 lines: put
the new code in a submodule `tests/traffic_support/third_body.rs` (`mod third_body;` + re-export).

1.3 Opt-in `Footprints::with_third_bodies(mut self) -> Self` adds, every tick:
- kinematic vehicle x character: penetration of the capsule circle (`LocomotionConfig::capsule_radius`)
  into the chassis rectangle (own SAT: circle-to-OBB distance), tolerance `dynamic_tolerance` (the
  character is a dynamic body the solver pushes out);
- kinematic vehicle x static world: the chassis box above the 0.2 m underbody lift, shrunk 0.02 m
  (same box as the `spawn_traffic_car` `GATE BROKEN` check, `traffic_support/mod.rs:114-129`) against
  `GameLayer::World` via `SpatialQuery::shape_intersections`, every tick;
- the exemption of 1.2 applies to characters too.

1.4 Floor rows in a new `crates/gta_sim/tests/traffic_progress.rs` (plumbing of the oracle, flips):
- `oracle_sees_a_car_through_a_dummy`: a kinematic traffic car on the two-way street floor
  (`two_way_street(60.0)`), a dummy spawned on its line 10 m ahead with the car's contact switch left on:
  record; then drive the car's `Position` by hand through the dummy (named mutation, the oracle is the
  subject, not the car) -> a character violation; with `relaxed = Some(dummy)` set by hand -> no violation
  and `relaxed_max_depth > 0`; with `relaxed` naming another entity -> RED. The same with a wall of the
  test area (`world/test_area.rs` box) -> a static violation.
1.5 Baseline on HEAD: run `with_third_bodies()` report-only over the city G1 gates (`traffic_gridlock`
seeds 1/2/7/42, `traffic_junction_box`, `traffic_causes` R1/rb/spot C, `traffic_go_around`) through the
probe; record counts and depths in `scratch/stage1/baseline.txt`. Any HEAD violation is traced (which
pair, which tick) before stage 2; a real one is a finding for the orchestrator, not a tolerance.

### Stage 2 — relaxation plumbing (consumers only, no trigger yet)

2.1 `crates/gta_sim/src/traffic/progress.rs` (new): `pub struct TrafficHooks<'w, 's> { cars: Query<'w,
's, &'static TrafficCar> }` with `#[derive(SystemParam)]`, `impl CollisionHooks for TrafficHooks<'_, '_>`:
`modify_contacts` takes `body1.unwrap_or(collider1)`, `body2...`, returns `false` when either car's
`relaxed.blocker` is the other; `filter_pairs` keeps the default. Also here: `fn relaxed_blocker(car:
&TrafficCar) -> Option<Entity>` and `fn skip_relaxed(car, b: &RoadBody) -> bool`.
2.2 `crates/gta_sim/src/lib.rs:194`: `PhysicsPlugins::default().with_collision_hooks::<TrafficHooks>()`
(the one composition used by the game and the headless app).
2.3 `traffic/spawn.rs:60-79`: add `ActiveCollisionHooks::MODIFY_CONTACTS` to the spawned bundle (the pair
flag is set at pair creation, so it must be there from spawn).
2.4 `traffic/manoeuvre.rs::sense` (148-151): `skip` also drops `car.relaxed`'s blocker; `sense` also
returns the blocker's own hit on the target strip/sweep (`relaxed_hit`, computed with a skip that keeps
only the blocker). `manoeuvre::plan` (234-242): for an `idle` relaxed car, `relaxed_hit` filtered by
`passable` is tried with `plan_pass` first, so a clear lane pass is preferred over driving through
(D2: b3's recovered car goes around the dummy on the opposite lane when that is clear); the relaxation
stays until the car is past.
2.5 `traffic/box_rules.rs::connector_clear` (46-64) and `repick` (69-84): new parameter `ignore:
Option<Entity>` (the requester's relaxed blocker) added to `skip`. Call sites: `junction.rs:365-367`
(lease `body_blocked`), `:482` (head stuck check), `:486-495` (repick), `:543` (`blocked` map); each
passes the requester's `snaps[k].car.relaxed` blocker.
2.6 `box_rules.rs`: `pub(super) fn repick_relaxed(road, graph, junction, lane, current, from_s,
requester, half, blocker) -> Option<u32>`: over `graph.lane(lane).out` (current first, then `out`
order), keep exits whose `connector_clear(.., ignore = Some(blocker))` is `None`, and return the one of
least overlap with the blocker's footprint (max over `connector_body` rects of the penetration depth;
new `pub(super) fn penetration(a: &FlatRect, b: &Footprint) -> f32` in `contact.rs` next to
`rects_overlap` (397-405), SAT min overlap for rectangles, radius minus distance for circles); ties keep
`out` order. `junction.rs` request loop (479-511): a relaxed head (at its stop line, or `at_start`) uses
`repick_relaxed` when its current exit overlaps the blocker.
2.7 `junction.rs` grant loop (561-590): for a waiter whose relaxation's blocker holds (or waits for)
a conflicting connector at this node, that connector is not counted in `blocking` for it.
2.8 `recover.rs`: `nobody_coming` (79-108) and `corridor_clear` (113-148, both the `blocked` skip and
`resting_in`) skip the relaxed blocker; `led` unchanged (a relaxed car never gives up). Return the first
blocking body: `corridor_clear -> Result<f32, Option<Entity>>`, `nobody_coming -> Option<Entity>` (the
body that fails), `Recovery::Stay { blocker: Option<Entity> }` carrying corridor, then coming, then
`led`'s body (for the stage-3 edge).
2.9 `contact.rs::switch_to_dynamic`: predictive loop (469-485) skips `other == car.relaxed.blocker`;
backstop (490-505) skips the pair too (belt and braces; the hook already removes the start).
2.10 `progress.rs::keep(snap, road, half, cfg, tick) -> bool` (the end rule of §2) called in
`drive.rs` step 1 for every AI snap; `false` sets `relaxed = None`. Past test: every corner (or four rim
points) of B's footprint has `(q - rear) . forward < 0`, `rear = centre - forward * half.z`.
2.11 Tnua: floor row `relaxed_car_drives_through_a_standing_player` (in `traffic_progress.rs`): a
kinematic car on the two-way street, the player standing on its line 12 m ahead, `relaxed` set by hand
(plumbing row, named). Asserts: the car's centre passes the player's x; no `VehicleHit`/`DamageDealt`
targeting the player (read `Messages<VehicleHit>` each tick), `Health` unchanged, `HitReaction` never
`KnockedDown`/`Staggered`; the player's `Position.y` stays within 0.1 m of its standing height (not
launched); the car stays `Kinematic`; after the relaxation ends past the player, a `CollisionStart` no
longer being filtered is shown by a second pass with `relaxed = None` switching the car (control). If the
height assertion fails, add `TnuaNotPlatform` to the passer while its blocker is a character (inserted in
`drive.rs` when a character relaxation starts, removed with `try_remove` when it ends; rare, not
per-frame churn) and rerun. Record which in the stage summary.
2.12 Plumbing floor rows (mechanism, not behaviour): `relaxed_pair_has_no_contact` (a kinematic car set
relaxed against a parked car it drives through: no `CollisionStart` for the pair, the parked car's
position moves < 0.02 m, no switch to `Dynamic`); flip: hook returns `true` -> the parked car is shoved
and the car switches (RED).

### Stage 3 — detector, triggers, rows

3.1 `crates/gta_sim/src/traffic/config.rs`: `#[derive(Deserialize, Clone, Debug)]
#[serde(deny_unknown_fields)] pub struct ProgressConfig { pub wait_seconds: f32, pub cycle_seconds:
f32, pub max_seconds: f32 }`, field `progress` in `TrafficConfig`; `validate`: all positive;
`wait_seconds > pass.character_seconds` (clear passes get their chance first); `cycle_seconds >
reservation_timeout` (the lease resolves grant stand-offs first). Export in `traffic/mod.rs:23-26`.
`tests/config_traffic.rs`: one sabotage per rule, strictly on the failing side.
3.2 `assets/traffic/traffic.ron`: `progress: (wait_seconds: 18.0, cycle_seconds: 8.0, max_seconds:
15.0)` with a comment block (T: after this a car stuck behind a body that stands, with no way around,
squeezes past it, collision relaxed against that body only; above the 14 s "must still wait" windows
of the recovery gates, 12 s under the 30 s stand bound for the start; cycle: a cyclic wait held this
long is broken by the lowest entity; max: stale guard).
3.3 `drive.rs`: `leader` returns `(gap, speed, k)`; `junction::update` returns
`HashMap<Entity, (Entity, progress::Edge)>` for waiters at a stop line (box blocker from `blocked`,
whose value becomes `Option<(Entity, f32)>`; first conflicting holder or earlier waiter; room: the exit
lane's tail car from `occupancy`, or the body `lane_start_free` found, which becomes `-> Option<Entity>`);
in the per-car loop (353-442) collect one candidate edge per car into `edges: Vec<Option<(Entity,
Edge)>>` by the §2 table (nearest gap, `Body` on ties); for `Dynamic` cars from `Recovery::Stay`'s
blocker, or `Follow` to the leader within `2 * idm.min_gap`. Keep `drive.rs` under 750 lines: the table
logic lives in `progress::edge_of(...)`.
3.4 `progress.rs::detect(snaps, edges, road, cfg, tick) -> Vec<(usize, Entity, Trigger)>`: sink and
cycle triggers of §2 (pointer walk, visited/in-stack marks, members in entity order); called after the
per-car loop; sets `snaps[k].car.relaxed = Some(Relax { blocker, since: tick })` when `None`; drops a
`Pass { go: false }`. `TrafficStats` gains `progress_by_trigger: [u32; 2]` (sink, cycle) and
`unexplained: u32` (AI cars standing > `wait_seconds` with no out-edge: a false sink the gates must see).
3.5 Rows in `crates/gta_sim/tests/traffic_progress.rs` (production city unless named, stationary player
facing the scene, production population, no input; each collects all violations before panicking;
each reports worst relaxed overlap depth and `progress_by_trigger`):
- `m1_two_bodies_block_both_lanes_seed_1` (fixture 0.3): no AI car within 45 m stands > 30 s; no AI
  car > 30 s in `Dynamic`; `Footprints::with_third_bodies()` clean; liveness: `progress_by_trigger[0]
  >= 1` and at least 4 fed cars pass the scene on each lane.
- `n1_player_standing_in_the_box_seed_1` (fixture 0.3, player held >= 75 s, run 100 s): stands within
  45 m <= 30 s; no `Dynamic` > 30 s; third-body oracle clean; the player takes no damage (`Health`
  unchanged, no `VehicleHit` with target the player), is never `KnockedDown`/`Staggered`, `Position.y`
  within 0.1 m of standing height; liveness: a relaxation against the player occurred.
- `cycle_two_held_cars_in_a_one_way_cross` (floor): a one-way crossing (two one-way through roads,
  southbound and westbound, 12 m arms, so every lane has `left_gap = None` and `plan_box_pass` returns
  `None`); X is granted on its straight connector through the real start (queue head at its stop line
  from rest); Y is a held car on the crossing straight at the connector pose that puts its body on X's
  path (`spawn_traffic_car(Connector(cy), s_y, 0.0)`, `next = Some(cy)`, the named "demoted holder"
  pose of `traffic_box_overhang::standoff`); `s_y` chosen in stage 0 so that on HEAD both stand > 30 s.
  Asserts: both leave the box within 30 s; `progress_by_trigger[1] >= 1` and `[0] == 0` (the trigger
  was the cycle); the breaker is the lower entity; G1 + third-body oracle clean (the exempt pair only).
  No camera view: the stuck cheat never runs.
- `b3_dummy_pressed_at_the_bumper` re-anchored in `traffic_causes.rs` (see §5): pressed for
  `wait_seconds + 12 s` (derived from the shipped config, not a literal); before `wait_seconds` it stays
  `Dynamic` and is never given up (as today); afterwards it leaves `Dynamic` by recovery (never
  `Abandoned`/`Bailing`), passes the dummy (its rear past the dummy, the existing lane pass or the
  relaxed line) within 30 s of the press, the dummy is never hit/knocked down; `PRESSED_S` stays 20 for
  spot C.
3.6 Un-ignore `traffic_junction_box::seed_7_box_keeps_moving_liveness` (`:326`),
`traffic_causes::r1_car_left_in_the_box_seed_1` (`:462`) and `..._seed_7` (`:468`). `r1()` and
`check()` build their oracle with `with_third_bodies()` and print `relaxed_max_depth`. Re-anchor the
G4 lease clause (`traffic_junction_box.rs:211-213`): a holder whose `TrafficCar.relaxed.blocker` is the
body is not "held by the body" (its path is not blocked by it, D2); flip: the lease rule off on a
non-relaxed holder still turns the clause RED (record).
3.7 Flips (each recorded): (a) the sink trigger off (`detect` returns no sink triggers) -> M1, N1,
class D, class E rows RED; (b) the cycle break off -> the cycle row RED; (c) the relaxation off (all
consumers of 2.4-2.9 ignore `relaxed`; the hook returns `true`) -> the rows RED (stands, or G1 through
the unfiltered contact); (d) relax-all (consumers skip every body while `relaxed` is `Some`) -> the
third-body oracle RED in at least one row; (e) T raised to 40 s in the config -> M1 RED on the 30 s
bound (the threshold is load-bearing).
3.8 Measure and record the start latency per row (trigger tick to the end of the car's stand, and the
longest stand of any car in the scene). If any row's longest stand exceeds 27 s at T 18 s: lower T to
no less than 16 s (4 s over the 14 s recovery windows is the floor) and rerun; if that does not fit,
stop with a redesign note (second failure of the class rule).

### Stage 4 — regression, Linux, docs

4.1 `cargo test -p gta_sim -p citygen` (every G1-asserting gate, `traffic_gridlock` seeds 1/2/7/42),
`traffic_bench`/`police_bench`/`civilian_bench` under `MEAN_LIMIT` (the hook runs per touching pair
with a traffic car), `cargo clippy`, `cargo test -p gta_like --bin gta_like`,
`python tools/qa/tree_check.py`, `cargo tree -p gta_sim -e normal -i bevy_render` still empty.
4.2 Report `unexplained` and `progress_by_trigger` for every city gate (print lines); any relaxation in
`traffic_gridlock` is traced (which edge, which sink) and justified, never silenced.
4.3 Contact oracle sweep on the fixed code (TASK-036 lesson): run the city G1 gates with
`with_third_bodies()` through the probe; zero violations outside exempt pairs.
4.4 Linux: WSL Ubuntu 22.04, toolchain 1.95.0, recipe `maw/tasks/done/TASK-037/scratch/fixer/linux_run.sh`
and `scratch/wsl/`: the new rows, the three un-ignored rows, `traffic_gridlock`, `traffic_junction_box`,
`traffic_causes`, `traffic_go_around`. A Linux-only red is a real bug on another trajectory.
4.5 `docs/architecture/traffic.md`: new section "Progress: wait-for record and relaxed pass" (edges,
triggers, relaxation consumers, the hook, the end rule, T/cycle/max, the chain-end consequence in M1,
D10 estimate one paragraph); replace the "Open (TASK-039)" paragraph of the box section with the
resolution; the "Modes" paragraph notes that a relaxed `Dynamic` car recovers past its blocker.
`docs/design/GDD.md` §5.2: new bullet (Russian) "Продвижение (TASK-039): машина, простоявшая
`progress.wait_seconds` за стоящим телом (машина, человек, машина в коробке) без объезда, протискивается
мимо него, столкновение ослаблено только с этим телом (может задеть его, человеку не наносит урона и не
сбивает); циклическое ожидание ломается по младшему ключу. Решение D1: небольшое визуальное
несовершенство в кадре принято ради гарантированного движения; деспавн в кадре и продавливание
по-прежнему запрещены." and the last bullet's sentence "Машина в коробке, на которую игрок смотрит
вблизи, не исчезает и может запереть перекрёсток (открытый вопрос TASK-032, R1)" becomes "... не
исчезает; перекрёсток не запирается: машины проезжают её по правилу продвижения".
4.6 README "Статус"/AGENTS.md stage line and `docs/narrative-graph.md` are the orchestrator's close-out,
not this task's steps.

### Stage 5 — runtime QA (QA role)

TASK-040 repros over BRP: `m1_fixed.py <out> 34.6 -79.1`, `repro_player_in_junction.py`,
`repro_abandoned_car.py` (scripts in `maw/tasks/done/TASK-040/scratch/tools/`): no traffic car stands
> 30 s behind the stationary body; screenshots show no car through a third body; `TrafficStats`
`progress_by_trigger`/`unexplained` read over BRP (the resource is reflected). Owner run (not gated): a
car squeezing past a left car, past two cars blocking a street (M1: expect D through P), past the player
standing in a junction.

## 4. Risk areas

- **M1 look.** The chain end is D; the only path clear of S's queue goes 1.3-1.8 m through P (estimate
  from TASK-040 positions). Gated only as "no third body"; the depth is reported; the owner judges. If
  the owner rejects it, the named alternative is relaxing the waiter whose own relaxed path has the least
  overlap in the chain (S against D here: 0.2-0.8 m), a detector change, not a new rule.
- **N1 look and Tnua.** A mid-connector holder drives through the player; the sensor may lift him
  (2.11 decides `TnuaNotPlatform`). Every car whose path crosses a player who keeps standing waits T,
  so the junction runs at one car per ~T per approach; bounded, ugly, owner-judged.
- **False sinks in normal traffic.** An AI car standing for a reason the edge table misses becomes a
  sink and the car behind it (non-`Follow` edge) may ghost into it after T. Guarded by `unexplained`
  printed in every city gate and by `Follow` edges never relaxing. Watch `traffic_gridlock`.
- **Trajectory shift.** Hooks, the new field and new relaxations move every city trajectory (TASK-037
  lesson): expect latent reds elsewhere; trace the contact before blaming the change.
- **End of relaxation inside B.** The `max_seconds` end is refused while the footprint (+ `recover.skin`)
  touches B; the third-body oracle flags a relaxation that ended early (the pair is no longer exempt).
- **Stale relaxation vs spot C.** Queued `Dynamic` cars with flank dummies relax at 18 s (the dummies stay
  20 s): they recover into a queue (still `led`, never given up); spot C asserts only "never given up".
- **Hook scope.** `modify_contacts` runs only for pairs flagged at creation; a traffic car spawned before
  2.3 lands is not flagged (all are spawned through `spawn_traffic_car`; verify no other spawn path with
  `rg "TrafficCar \{"` — only `spawn.rs`).
- **Class D band.** If 1853v0 sits outside the conflict-table band, relaxed recovery never fires (0.4
  stop condition).
- **Performance.** Detector O(n) per tick; the hook one component lookup per flagged touching pair;
  benches gate the mean.
- **File sizes.** `drive.rs` 662 lines: new logic stays in `progress.rs`; `traffic_support/mod.rs` 735:
  new oracle code in `third_body.rs`.

## 5. Open questions and decisions

Decisions taken here (logged in `log.jsonl`):
- T = 18 s with b3 re-anchored into a progress row (vs T 21-24 s keeping b3: 2 s margin over the fixture
  and 6-9 s for the start latency).
- Sink/cycle-only triggers, `Follow` edges never relax (vs SUMO-style "ignore any body standing T").
- In the box: own connector line, least-overlap exit when re-picking is still allowed (vs a relaxed
  whole-box offset pass).
- Physics relaxation via `modify_contacts` (vs `filter_pairs`, which misses pairs already touching).
- D9 U-turn not built; D10 not built (cost above).

Вопросы к владельцу (не блокируют реализацию, решаются прогоном владельца):
1. M1: соседняя машина D проезжает сквозь машину игрока на 1.3-1.8 м (другого чистого пути нет, пока
   стоит встречная очередь). Варианты: (а) принять как есть (рекомендую, это правило D1); (б) выбирать в
   цепочке ожидания машину с наименьшим перекрытием (S мимо D, 0.2-0.8 м) — правка детектора, не новое
   правило; (в) держать улицу закрытой — противоречит цели задачи.
2. N1: машина проезжает сквозь стоящего игрока, урона нет. Варианты: (а) принять (рекомендую);
   (б) добавить объезд человека внутри коробки с захватом всей коробки — дороже и ломкое место
   TASK-032/037.
3. Машина, брошенная в начале выездной полосы за перекрёстком, держит подъезды, которые ждут места
   (ребро "место", `Follow`, не ослабляется). В четыре строки задачи не входит; если это нужно
   закрыть, отдельная задача (перевыбор выезда при ожидании места дольше T).
