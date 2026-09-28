# PLAN V2 — TASK-039: universal traffic progress guarantee

Reviewer: plan-reviewer-1. Base: `PLAN.md` (commit 48af323). Cost of error: silent (gridlock that shows
over minutes, in front of the player). Full evidence layer: headless rows with flip-RED on Windows and
Linux, the third-body oracle, runtime QA on the TASK-040 repros.

## 0. Disconfirmation (done first)

Counter-example tested: "the physics relaxation (`modify_contacts` returning `false`) does not reach a pair
that is ALREADY touching when the relaxation starts (class D: 0.06-0.26 m from the left car; b3: the
dummy pressed at the bumper), or `ActiveCollisionHooks` read only at pair creation cannot be on the car
in time." Checked in `avian3d-0.7.0/src/collision/narrow_phase/system_param.rs:771-783`: the hook runs
every step for every touching pair whose `ContactPairFlags::MODIFY_CONTACTS` is set, and `false` clears
the manifolds, so a pair touching before gets `STOPPED_TOUCHING`. The flag comes from
`ColliderTreeProxyFlags::MODIFY_CONTACTS` (`collider_tree/tree.rs:93`, also refreshed by an
`On<Insert, ActiveCollisionHooks>` observer, `collider_tree/update.rs:347`). So the counter-example does
NOT hold, and the plan's physics mechanism is sound. Caveat from `hooks.rs:180`: the hook is "only called
if at least one entity in the contact pair is not Static and not Sleeping". Row 2.12 must therefore
start with a sleeping parked car (see Stage 2).

The review found three bigger problems elsewhere: the plan ignores the orchestrator's binding resolved
questions, its relaxation end rule is unsafe, and its trigger makes waits compound in N1.

## 1. Review notes (issues in PLAN.md, with evidence)

**R1. Binding resolved question 1 is ignored (MAJOR).** `TASK_FINAL.md` "Resolved questions" (added
after PLAN.md §5): "in a wait chain, relax the car whose pass has the smallest overlap with its blocker,
not the chain's end car by default". PLAN §2 ("Sink-only triggers ... in M1 D is the chain end and drives
through P with 1.3-1.8 m") and §5 Q1 still propose the chain end. The detector's choice rule has to
change (see §3 A2). Worked M1 geometry (lane pitch ~3.25 m, car half width ~0.9 m; D at eastbound line
-1.85 m, P at +0.6 m):
- S (westbound, on its line at -3.25): body -4.15..-2.35 vs D -2.75..-0.95, overlap ~0.4 m;
- D rejoining its line through P: D -0.9..0.9 vs P -0.3..1.5, overlap ~1.2 m;
- D driving on at its current offset -1.85: 0 m against P, but ~0.4 m into the westbound band (third
  bodies while any westbound car comes).
Consequence the plan must state: the least-overlap rule changes the ORDER in M1 (the westbound queue
passes D first), but D itself must still leave. D4 and the stand bound forbid D standing > 30 s
(`Dynamic` or not), and every path out of D's spot crosses P or the westbound band. So the plan must
also give D a least-overlap motion: its current offset past P while the westbound band is clear, else
the rejoin through P. Stage 0 measures the real numbers; the ones above are estimates.

**R2. Binding resolved question 2 is ignored (MAJOR).** "N1: a clean path (curb, in-box offset where the
conflict table allows) is tried first ... `TnuaNotPlatform` (or an equivalent) is mandatory during it."
PLAN 2.11 makes `TnuaNotPlatform` conditional ("if the height assertion fails"), and the plan keeps the
box-pass refusal around characters (`manoeuvre.rs:253`
`if hit.kind == BodyKind::Character || !alone_in_box(..) { return None; }`). So no clean in-box path
around the player is ever tried. `TnuaNotPlatform` exists in the vendored crate
(`vendor/bevy-tnua-avian3d-0.12.1/src/lib.rs:20,160,256-258`: a hit on an entity, or on the body of a
collider, carrying it is skipped by every character's ground sensor).

**R3. Unsafe end rule: "B moved (`standing == 0`)" ends the relaxation even mid-overlap (MAJOR, D3).**
Once the relaxation ends, the hook returns `true` again. For a pair still overlapping, the narrow phase
reports `STARTED_TOUCHING` → `CollisionStart` → `apply_impacts` (`vehicle/impact.rs:65-136`) charges
pedestrian damage and knocks down at `closing >= knockdown_speed`. The solver then pushes a character
out of a kinematic (infinite-mass) body. So the player stepping away while a car is inside him breaks D3
(no damage, no knock-down, no launch). The same holds for `clear_ai_state` clearing `relaxed` on
hijack or bail mid-overlap. Fix: the physics exemption lasts until the footprints (grown by
`recover.skin`) separate, whatever ended the manoeuvre (§3 B4).

**R4. `min(standing(W), standing(B)) >= T` makes waits compound (MAJOR, N1).** Worked N1 example,
T = 18 s: holder H1 stops at the player and relaxes after 18 s. Conflicting waiter W2 (older stamp) gets
the grant next. Its connector also crosses the player (`connector_clear` skips characters,
`box_rules.rs:58`), so it enters, stops, its `standing` restarts at 0 (it moved), and it waits another
18 s. A car W4 at a third approach that arrived during H1's wait stands ≥ 36 s > 30 s. This is the s42
tourist shape (19 of 24 cars up to 118 s). The project already keys pass eligibility on the blocker's
standing time (`pass.rs::passable` 28-40: `hit.standing >= p.vehicle_seconds / character_seconds`).
Fix: trigger on the blocker's standing ≥ T plus a short waiter grace (data), §3 A3. The SUMO citation
does not support PLAN's claim that "the time is the BLOCKER's standing time": the SUMO page wording is
ambiguous. This is not load-bearing; the local precedent and the worked example are.

**R5. The `Grant` relaxation (PLAN 2.7) is dangerous and not needed.** Letting a waiter's grant ignore a
conflicting holder breaks the conflict table's premise. If the holder moves later (its own relaxation
ends its stand), two MOVING cars on conflicting connectors are an exempt pair, a ghosting that the end
rule "B moved" then drops mid-overlap (R3). Pure grant stand-offs are already ended by the lease
(`junction.rs:165-174`, `reservation_timeout` 5 s). Fix: `Grant` edges stay in the graph for chain and
cycle detection and reporting, but are never relaxed. A cycle is broken on one of its `Body` edges.

**R6. `leader` is not a relaxation consumer.** `drive.rs::leader` (102-129) brakes for any AI car ahead in
the path occupancy (`Dynamic` ones included, `occupancy` 52-68). A `Body` edge to an off-line or
`Dynamic` AI car on the waiter's own path (sensing gap < leader gap when that car is yawed or offset:
"nearest gap wins") would relax sensing, but `leader` would still hold the car, so the relaxation would do
nothing and the stand bound would go RED. Fix: `leader` skips the relaxed blocker (§3 B2).

**R7. Wrong line numbers in PLAN.md (the functions exist; the numbers look like offsets into concatenated
files).** Actual locations: `junction.rs::update` 76-364 (file 364 lines, not 314-602); `body_blocked`
115-134; head stuck check and repick 241-273; `blocked` map 301-309; grant loop 310-363.
`contact.rs::switch_to_dynamic` 131-246 (predictive loop 177-208, backstop 209-224); `rects_overlap` 116.
`pass.rs::passable` 28-40, `plan_pass` 191. `vehicle/impact.rs::apply_impacts` 50-156.
`stuck.rs::despawn_stuck` 20. The citations for `drive.rs`, `recover.rs`, `box_rules.rs`,
`manoeuvre.rs`, `occupancy/mod.rs` and `traffic/mod.rs` are correct. Implementers: locate by function
name.

**R8. `TrafficStats` accounting.** `advance_traffic` rebuilds `TrafficStats` every tick from `..default()`
and carries only `spawned/despawned/casts/switches_by_cause` (`drive.rs:237-243`, `*stats = counts` at
661). A new cumulative `progress_by_trigger` has to be carried the same way, or it reads 0 every tick
and every liveness assertion on it is vacuous. `unexplained` is a per-tick count, so name it that way
(or add a cumulative `unexplained_max`).

**R9. Cycle-row fixture.** In PLAN's version X is granted and Y is held. The lease demotes or lapses X
(contested, stale), and `whole` then rotates, so the fixture may not stand > 30 s on HEAD. PLAN hedges
this, but it gives no fallback. A pure persistent 2-cycle is two cars both demoted on crossing
connectors, each body on the other's swept path, on a one-way crossing (`left_gap = None` →
`plan_box_pass` `None`, `box_rules.rs:137`). Each is a `Body` edge through the `blocked` map, and the
`whole` grant rotates between them forever. Stage 0 must show it RED on HEAD. Without a HEAD-RED cycle
row the spec's flip "cycle break off → RED" cannot be met.

**R10. The b3 re-anchor touches a shared helper.** `bumped` (`traffic_causes.rs:141-249`) is shared by
b1..b4 and uses the const `PRESSED_S`. The pressed time needs a parameter for b3 only. b1/b2/b4 and
`spot_c` (which also uses `PRESSED_S`) stay unchanged.

**R11. Correct and kept:** the avian hook API (`CollisionHooks: ReadOnlySystemParam + Send + Sync`,
`modify_contacts(&self, &mut ContactPair, &mut Commands) -> bool`, `PhysicsPlugins::with_collision_hooks`
`lib.rs:701`, `ContactPair.body1/body2: Option<Entity>`); `filter_pairs` rejected (new pairs only);
`TrafficCar` built literally only in `spawn.rs:63`; the vehicle chassis collider on the body entity
(`vehicle/mod.rs:158-176`, `CollisionEventsEnabled`); `recover_dynamic` gates recovery on the
conflict-table band (`recover.rs:209-217`), so PLAN 0.4's stop condition is right; `traffic_support/mod.rs`
is 735 lines (submodule needed); `drive.rs` 662. No BRP/QA script writes a `TrafficCar` (t14/t15/t16 read
only), so the new field breaks no script. No new crate, `Cargo.lock` unchanged.

## 2. Updated understanding (corrected)

- `advance_traffic` (`drive.rs:202-662`) runs in this order: snapshot sorted by entity bits (244-268);
  1. `recover_dynamic` for `Dynamic` cars, before everything else (270-304); path occupancy (307);
  `junction::update` (328-344); per car IDM (`leader`, stop line, pass hold, `sense`) and then
  `manoeuvre::plan` (353-442); motion (444-593); bailing (595-634); write back (636-660). A relaxation
  set after the per-car loop is first seen by recovery on the NEXT tick. That is fine, but the latency
  counts toward the 30 s budget.
- `switch_to_dynamic` runs in `FixedPostUpdate` before `PhysicsSystems::First` (`mod.rs:320-325`). It
  switches on a predicted contact with any DYNAMIC character or vehicle (`contact.rs:188-204`) or a
  `CollisionStart` backstop (209-224). Physics (and the hook) runs after it, so the hook sees this
  tick's `TrafficCar.relaxed`.
- `RoadOccupancy` holds only bodies within `in_view.despawn + look_ahead` = 150 m of the player
  (`occupancy/mod.rs:134-146`). `standing` resets on motion or displacement.
- The lease already ends grant stand-offs (`junction.rs:160-184`). A holder blocked by a WALKER keeps its
  grant, because `connector_clear` skips characters (`box_rules.rs:56-60`). That is the N1 mechanism.
- `plan` only lane-passes an `idle` kinematic car (`manoeuvre.rs:197-200, 234`) and never box-passes
  around a character (253).
- Existing clean-pass eligibility is keyed on the blocker's standing time (`passable`).

## 3. Revised approach

One mechanism, in a new file `crates/gta_sim/src/traffic/progress.rs`: a wait-for record, a
deterministic choice of which pair to relax, and a relaxation with a safe end.

**A1. Edges** (at most one per AI car in `Kinematic`/`Dynamic` whose road body stands; nearest gap wins,
a tie goes to `Body`):

| Edge | Source | Kind |
|---|---|---|
| path leader (`leader` now also returns its index) within `pass.trigger_gap` | lane queue | `Follow` |
| oncoming claim hit | waits for a passer | `Follow` |
| nearest sensing hit (not a claim) within `pass.trigger_gap`; the pass hold's obstacle | body on its path | `Body` |
| at stop line / on connector: body on its connector path (`blocked` map, now with the entity) | box blocker | `Body` |
| at stop line: holder or earlier waiter of a conflicting connector | grant | `Grant` (never relaxed, R5) |
| at stop line: waiting for room (exit-lane tail, `lane_start_free` body) | room | `Follow` |
| `Dynamic`: first body failing `corridor_clear` / `nobody_coming` | recovery blocker | `Body` |
| `Dynamic`: path leader within `2 * idm.min_gap` | queue | `Follow` |

A path leader that is itself a sink (an AI car with no out-edge, standing ≥ T) gives a `Body` edge, not
`Follow`. Bailing, abandoned, taken and police cars, and characters, have no out-edges (sinks).

**A2. Choice (resolved question 1).** Chains are walked over the functional graph (pointer walk,
visited/in-stack marks, O(n)).
- *Sink chain*: a chain that ends at a sink B. Candidates are the chain's `Body` edges (W → X) whose
  target X has stood ≥ `progress.wait_seconds` (T) and whose waiter has stood ≥
  `progress.grace_seconds`. The pick is the one of least `overlap(W, X)`: the worst penetration of W's
  footprint swept along the path it will drive relaxed, from its pose until past X (lane line, or its
  current offset for a car in `Rejoin`/`Pass`, or the connector line), against X's footprint. Ties go
  to the lower `Entity::to_bits()`.
- *Cycle*: when every member has stood ≥ `progress.cycle_seconds`, the member with the least-overlap
  `Body` edge is relaxed (tie: lower entity). A cycle with no `Body` edge is left to the lease and counted.
- At most one new relaxation per chain per tick. Re-evaluated every tick, so after a pass ends the next
  pick follows.
- *D4 guard*: a `Dynamic` waiter whose recovery blocker has stood ≥ T is also given a RECOVERY
  relaxation (only its corridor / `nobody_coming` skip B), whatever the chain pick is. Its `Dynamic`
  stand ends by recovery. What it then drives is the least-overlap motion (A4).

**A3. Trigger timing (R4).** The blocker's `standing ≥ T` AND the waiter's `standing ≥ grace`. Both are
data. `grace` gives the clean planners (lane pass, box pass, repick) their chance for each new waiter.
`grace ≥ pass.character_seconds` (6 s) is the natural floor; stage 0 derives the value from the N1 trace
(target: the worst N1 stand ≤ 27 s at the chosen T).

**A4. Clean first (D2, resolved question 2).** In this order for a triggered waiter:
1. Existing lane pass (`plan_pass`) around B. The `idle` gate is widened for a relaxed car in `Rejoin`,
   so D in M1 can pass P from its current offset when the far band is clear.
2. A head that may still re-pick (at its stop line, or `at_start`): the exit of least overlap with B
   among exits clear of every other body (`repick_relaxed`).
3. A box pass (`plan_box_pass`) around a CHARACTER blocker, when alone in the box. The refusal at
   `manoeuvre.rs:253` is lifted only for a car whose progress trigger fired.
4. Only then the relaxed drive on its own line (or current offset) through B.

**B. Relaxation.** `TrafficCar.relaxed: Option<Relax { blocker, since, physics_only: bool }>`.
- B1 consumers skip B while the relaxation is active: `manoeuvre::sense`, `box_rules::connector_clear` /
  `repick` (new `ignore: Option<Entity>`), the lease's `body_blocked`, `recover::corridor_clear` +
  `nobody_coming` (`led` unchanged: a relaxed car never gives up), and `switch_to_dynamic` (predictive
  and backstop).
- B2 also `drive.rs::leader` (R6).
- B3 physics: a `TrafficHooks` `modify_contacts` returns `false` for (car, relaxed.blocker) in both
  directions (by `body1.unwrap_or(collider1)`), so there is no push, no `CollisionStart`, and so no
  `apply_impacts` and no switch. `ActiveCollisionHooks::MODIFY_CONTACTS` goes on every traffic car at
  spawn. `TnuaNotPlatform` is inserted on the passer whenever its blocker is a character, is MANDATORY
  (resolved question 2), and is removed with `try_remove` when the relaxation ends (rare, no per-frame
  churn).
- B4 end rule (R3): the manoeuvre part ends when B has gone from the snapshot, B moved, the car is past
  B, the car left AI, or `max_seconds` elapsed. On that, the planning skips stop (`physics_only = true`),
  but the hook exemption and `TnuaNotPlatform` last until the footprints grown by `recover.skin` no
  longer touch, or B no longer exists. `clear_ai_state` does NOT clear `relaxed`. The separation check
  runs over every `TrafficCar` with `relaxed.is_some()`, whatever its mode: a small loop at the top of
  `advance_traffic`, before the AI filter. A car whose B moved while overlapped stops in B's way (its
  sensing sees B again) until B walks out. The contact stays off, so B is neither pushed nor hurt.

**T values (data, `assets/traffic/traffic.ron`, new `progress` block):** `wait_seconds` (T),
`grace_seconds`, `cycle_seconds`, `max_seconds`. The starting point, kept from PLAN §5: T = 18 s with b3
re-anchored (floor 14 s + 4 for the `traffic_recovery` c/e windows). `grace`, `cycle` and `max` are
derived in stage 0 and stage 3 from traces, not from design. Rules in `validate`:
`wait_seconds > 14`-class windows are covered by tests, not by a validator literal;
`grace_seconds >= pass.character_seconds`; `cycle_seconds > reservation_timeout`; `max_seconds > 0`;
all positive.

**D9 U-turn:** not built unless stage 3 shows class E not cleared. **D10 "one body owner":** named option,
not built. PLAN §2's estimate stands (about 12-15 source files, about 25 gate files, 3-5 tasks, and it
still needs a progress rule because traffic cannot shove a parked car).

## 4. Revised steps

Every stage ends green, and each stage summary records its flips (input perturbed, RED seen, restored,
GREEN). Probe builds use `CARGO_TARGET_DIR=D:/test-gta-like/target`. Run `touch crates/*/src/lib.rs`
before trusting a red built from the probe (TASK-009 lesson). Work in place, no second checkout.

### Stage 0 — traces on HEAD (no production change)

0.1 Probe crate `maw/tasks/in_progress/TASK-039/scratch/probe/ws/probe`, copied from TASK-037's
(`#[path]` to `tests/common/mod.rs`, `tests/traffic_support/mod.rs`). Read-only, recomputed from public
state.

0.2 Traces, once per second, for every AI car standing > 5 s within 45 m of the scene: segment, s, mode,
manoeuvre, lateral, grant/waiter/whole, speed, `standing`; its would-be edge (A1); the chain (sink or
cycle); for each `Body` candidate, `overlap(W, X)` (A2); for heads, per exit: clear of third bodies and
overlap with the chain end. Rows: class E (G4 seed 7, R1 seed 7), class D (R1 seed 1), M1, N1, b3, the
cycle fixture. Output under `scratch/stage0/`.

0.3 Fixtures in the probe first, each RED on HEAD:
- M1 (seed 1): as PLAN 0.3. Lanes via `TrafficGraph::nearest` at (34.4, -77.5) and (40.2, -80.4), which
  must be opposite (`GATE BROKEN`). D is switched through a real contact (the `bumped` pusher pattern),
  then `teleport` (named mutation) to the eastbound line -1.85 m. P is `park_car` at the line +0.6 m at
  x 34.9. The player is on the north sidewalk facing the scene, with feeders on both lanes, 120 s.
  `GATE BROKEN` unless D is `Dynamic`, both bodies are within 0.5 m of their targets, P and D do not
  overlap, and both lanes are blocked.
- N1 (seed 1): the player at (7.9, -81.3), `graph.in_junction` true, at least one connector body sweep
  overlapping the capsule, held ≥ 75 s, run 100 s.
- Cycle (R9): a one-way crossing floor; X and Y both on crossing straights, both demoted holders
  (`next = Some(own)`, no grant, the named `standoff` pose), each body on the other's swept path.
  `GATE BROKEN` unless both `blocked` entries name the other. It must stand > 30 s on HEAD. If no pose
  does, stop and write a note: the cycle flip cannot be met.

0.4 Record: where N1's waiters stand and how many holders pass the player per minute on HEAD; whether
1853v0 (class D) is inside the conflict-table band (outside → stop with a redesign note before stage 2);
the M1 overlaps S→D, D→P (rejoin), D→P (current offset), and whether D's offset path meets the westbound
band; the value of `grace` that keeps the worst N1 stand ≤ 27 s at T 18 (worked from the trace: holders
in a row × (grace + pass time) + a conflicting waiter's wait).

### Stage 1 — third-body oracle, before any behaviour change

1.1 `traffic/mod.rs`: `#[derive(Reflect, Clone, Copy, Debug, PartialEq)] pub struct Relax { pub
blocker: Entity, pub since: u64, pub physics_only: bool }`; field `pub relaxed: Option<Relax>` on
`TrafficCar`; `register_type::<Relax>()`. Add `relaxed: None` to the `spawn.rs` literal.
`clear_ai_state` leaves `relaxed` alone (B4). No writer yet.

1.2 `tests/traffic_support/third_body.rs` (new submodule; `mod.rs` stays < 750): `Footprints` exempts a
pair while either car's `relaxed.blocker` is the other (read every tick). Exempt pairs feed
`relaxed_max_depth`, never `violations`.

1.3 Opt-in `Footprints::with_third_bodies()`: kinematic vehicle × character (capsule circle into the
chassis rectangle, tolerance `dynamic_tolerance`) and kinematic vehicle × static world (the chassis box
above the 0.2 m underbody lift, shrunk 0.02 m, `SpatialQuery::shape_intersections` on
`GameLayer::World`). The exemption applies to characters too.

1.4 Floor row `oracle_sees_a_car_through_a_dummy` (in the new `tests/traffic_progress.rs`): drive a
kinematic car's `Position` by hand (named mutation) through a dummy → a character violation. With
`relaxed = Some(dummy)` → no violation and `relaxed_max_depth > 0`. With `relaxed` naming another entity
→ RED. The same through a test-area wall → a static violation. Check the fixture points against
`world/test_area.rs` (TASK-012 lesson).

1.5 Baseline on HEAD: `with_third_bodies()` in report-only mode over the city G1 gates through the probe
(`scratch/stage1/baseline.txt`). Every HEAD violation is traced by pair and tick. A real one goes to the
orchestrator as a finding BEFORE stage 2, because the spec's "0 against any third body" cannot then be
met by this task alone. Never add a tolerance.

### Stage 2 — relaxation plumbing (consumers only, no trigger yet)

2.1 `progress.rs`: `#[derive(SystemParam)] pub(crate) struct TrafficHooks<'w, 's> { cars: Query<'w, 's,
&'static TrafficCar> }`; `impl CollisionHooks for TrafficHooks<'_, '_>`, `modify_contacts` as in B3,
default `filter_pairs`. Helpers: `relaxed_blocker(&TrafficCar) -> Option<Entity>` (active planning skip,
`None` when `physics_only`) and `exempt(&TrafficCar, Entity) -> bool` (physics).
2.2 `lib.rs` (the one composition): `PhysicsPlugins::default().with_collision_hooks::<TrafficHooks>()`.
2.3 `spawn.rs::spawn_traffic_car`: add `ActiveCollisionHooks::MODIFY_CONTACTS` to the bundle.
2.4 Consumers with `relaxed_blocker`: `manoeuvre::sense` skip (and a `relaxed_hit` for A4.1);
`drive.rs::leader` skip (R6); `box_rules::connector_clear` / `repick` `ignore` parameter at the call
sites `junction.rs:127-129` (`body_blocked`), `:244` (head stuck), `:248-257` (repick), `:305`
(`blocked`); `recover::nobody_coming` / `corridor_clear` (both the `blocked` skip and `resting_in`);
`contact.rs::switch_to_dynamic` predictive (188-204) and backstop (209-224) use `exempt`.
2.5 `recover.rs`: `corridor_clear` / `nobody_coming` return the first failing body; `Recovery::Stay {
blocker: Option<Entity> }` (update the match at `drive.rs:279-294`).
2.6 `box_rules.rs::repick_relaxed` + `contact.rs::penetration(a: &FlatRect, b: &Footprint) -> f32` (SAT
minimum overlap for rectangles, radius minus distance for circles), next to `rects_overlap` (116). Also
used by the A2 overlap estimate.
2.7 (PLAN 2.7, Grant relaxation) is DROPPED (R5).
2.8 `progress.rs::end(...)` (B4), called at the top of `advance_traffic` over every `TrafficCar` with
`relaxed.is_some()`. It sets `physics_only` on manoeuvre end and `None` on separation (footprint +
`recover.skin`) or when B is gone. `TnuaNotPlatform` is inserted when a relaxation against a character
starts and removed with `try_remove` on `None`.
2.9 Floor row `relaxed_car_drives_through_a_standing_player` (named plumbing: `relaxed` set by hand). The
car passes the player's position; no `VehicleHit` / `DamageDealt` targets the player (read
`Messages<VehicleHit>` each tick); `Health` unchanged; `HitReaction` never `KnockedDown`/`Staggered`;
`Position.y` within 0.1 m of standing height; the car stays `Kinematic`. Flip: no `TnuaNotPlatform` →
the height assertion must go RED; if it stays GREEN, record that the sensor case was not exercised and
keep the component anyway (binding). Control: a second pass with `relaxed = None` switches the car.
2.10 Floor row `relaxed_pair_ends_only_when_separated` (R3): the relaxed car stops halfway through a
dummy, and the dummy then moves (a named mutation of its intent or position). While overlapped: no
`CollisionStart` for the pair, no damage, the car does not switch. After the dummy walks out: the
relaxation is `None`. Flip: end on "B moved" → damage or a switch (RED).
2.11 Floor row `relaxed_pair_has_no_contact`: relaxed against a parked car that is asleep at the start
(assert `Sleeping` present, `hooks.rs:180` caveat). The car drives through it: no `CollisionStart`, the
parked car moves < 0.02 m, no switch. Flip: the hook returns `true` → shoved and switched (RED).

### Stage 3 — detector, triggers, rows

3.1 `traffic/config.rs`: `ProgressConfig { wait_seconds, grace_seconds, cycle_seconds, max_seconds }`
(`deny_unknown_fields`), with `validate` as in §3. Export it in `traffic/mod.rs:23-26`.
`tests/config_traffic.rs`: one sabotage per rule, strictly on the failing side (TASK-007 lesson).
3.2 `assets/traffic/traffic.ron` `progress: (...)` with the values derived in stage 0, and a comment
block saying what each value does and why it is set there.
3.3 Edges (A1): `leader` returns its index; `junction::update` returns the waiter edges (`blocked` value
becomes `Option<(Entity, f32)>`, `lane_start_free -> Option<Entity>`); the per-car loop collects
`edges`; the table logic lives in `progress::edge_of` (keeps `drive.rs` < 750).
3.4 `progress::detect` (A2, A3, D4 guard). It sets `relaxed` when `None`, and drops a `Pass { go:
false }` of the chosen car. A4 order: `plan` widens `idle` for a relaxed `Rejoin` car (A4.1), the
request loop uses `repick_relaxed` (A4.2), and the box-pass refusal is lifted for a triggered car against
a character (A4.3). `TrafficStats`: `progress_by_trigger: [u32; 2]` (sink, cycle) and
`progress_recoveries: u32`, both carried in `counts` like `switches_by_cause` (R8), plus a per-tick
`unexplained`.
3.5 Rows in `tests/traffic_progress.rs`: production city unless named, stationary player facing the
scene, production population, no input. Each collects all violations before panicking and reports the
worst relaxed overlap depth and the trigger counts.
- `m1_two_bodies_block_both_lanes_seed_1`: stands within 45 m ≤ 30 s; no `Dynamic` > 30 s; third-body
  oracle clean; at least 4 fed cars pass on each lane; a sink trigger fired. Report which pair was
  chosen first and D's path (offset or rejoin) with its depth.
- `n1_player_standing_in_the_box_seed_1`: stands ≤ 30 s; no `Dynamic` > 30 s; oracle clean; the player
  undamaged, never `KnockedDown`/`Staggered`, `Position.y` within 0.1 m. Liveness: a relaxation against
  the player occurred, or a clean box pass or repick around him did (report which).
- `cycle_two_held_cars_in_a_one_way_cross` (floor, fixture 0.3): both leave the box within 30 s;
  `progress_by_trigger[1] >= 1`, `[0] == 0`; the breaker is the least-overlap member (lower entity on a
  tie); G1 and third-body oracle clean.
- b3 re-anchored (R10): `bumped` takes the pressed time as a parameter; b3 passes `wait_seconds + 12 s`
  from the shipped config. Before `wait_seconds` it stays `Dynamic` and is not given up. After that it
  recovers (never `Abandoned`/`Bailing`), its rear is past the dummy within 30 s of the press, and the
  dummy is never hit or knocked down. b1/b2/b4/spot C keep `PRESSED_S`.
3.6 Un-ignore `traffic_junction_box::seed_7_box_keeps_moving_liveness` (`:326`) and
`traffic_causes::r1_car_left_in_the_box_seed_1` / `_seed_7` (`:462`, `:468`); update the module docs'
"ignored" lines (`traffic_causes.rs:15`, `traffic_junction_box.rs:18`). `r1()` and `check()` build the
oracle `with_third_bodies()` and print `relaxed_max_depth`. Re-anchor the G4 lease clause: a holder whose
active relaxation names the body is not "held by the body". Flip: the lease rule off on a non-relaxed
holder still turns the clause RED.
3.7 Flips, each recorded:
- (a) the sink trigger off → the M1, N1, class D and class E rows RED;
- (b) the cycle break off → the cycle row RED;
- (c) the relaxation off (consumers ignore `relaxed`, the hook returns `true`) → the rows RED;
- (d) relax-all (consumers skip every body while `relaxed` is set) → the third-body oracle RED;
- (e) T raised to 40 s → M1 RED on the 30 s bound;
- (f) least-overlap choice replaced by chain end → the M1 report shows the deeper first pick
  (report-level evidence for resolved question 1; the depth is not a pass bound);
- (g) `grace` rule replaced by `min(W, B)` → N1 RED (proves R4 is load-bearing).
3.8 Measure the start latency per row. If any longest stand is > 27 s: lower T (floor 16 s) or `grace`
and rerun. A second failure of the same class → stop with a redesign note.

### Stage 4 — regression, Linux, docs

4.1 `cargo test -p gta_sim -p citygen` (all G1 gates, `traffic_gridlock` seeds 1/2/7/42);
`traffic_bench`/`police_bench`/`civilian_bench` under `MEAN_LIMIT`; `cargo clippy`;
`cargo test -p gta_like --bin gta_like`; `python tools/qa/tree_check.py`;
`cargo tree -p gta_sim -e normal -i bevy_render` empty.
4.2 Print `unexplained`, `progress_by_trigger` and `progress_recoveries` for every city gate. Trace and
justify any relaxation in `traffic_gridlock`, never silence it. Run every city gate's stand bounds (D5,
TASK-032 lesson).
4.3 Contact oracle sweep on the FIXED code with `with_third_bodies()` (TASK-036 lesson).
4.4 Linux: WSL Ubuntu 22.04, toolchain 1.95.0, TASK-037 `scratch/fixer/linux_run.sh` + `scratch/wsl/`.
Rows: the new rows, the three un-ignored rows, `traffic_gridlock`, `traffic_junction_box`,
`traffic_causes`, `traffic_go_around`. A Linux-only red is a real bug.
4.5 Docs:
- `docs/architecture/traffic.md`: a section "Progress: wait-for record and relaxed pass" (edges,
  least-overlap choice, grace, the D4 guard, consumers, the hook, the end rule and `physics_only`,
  `TnuaNotPlatform`, D10 one paragraph); resolve the "Open (TASK-039)" paragraph; record the exit-lane
  car observation (resolved question 3).
- GDD §5.2, in Russian: the progress bullet. The car that goes past is the one with the least overlap;
  a clean detour comes first; a pedestrian is not hurt. Also D1, and the updated "машина в коробке"
  sentence (as PLAN 4.5, with the least-overlap wording).
4.6 README/AGENTS/narrative-graph: the orchestrator's close-out.

### Stage 5 — runtime QA (QA role)

TASK-040 repros over BRP: `m1_fixed.py <out> 34.6 -79.1`, `repro_player_in_junction.py`,
`repro_abandoned_car.py`. No traffic car stands > 30 s behind the stationary body, no third body is
driven through in the screenshots, and the player is not lifted in N1. Read `TrafficStats` over BRP.
Owner run (not gated): the squeeze past a left car, M1 (which car went first, and D's path), and a car
past the player in a junction.

## 5. Risk areas

- **M1 still has one deep pass.** D must leave, and every way out crosses P or the westbound band. The
  least-overlap rule orders the passes but may not avoid D through P (~1.2 m). Reported and judged by
  the owner.
- **Clean-first can stall.** A4.1/A4.3 wait for a free band, and a `Pass { go: false }` stands. If a
  clean attempt is still waiting when `max_seconds` runs out, the relaxed drive is taken; stage 3.8
  measures this.
- **False sinks** in normal traffic: `unexplained` in every city gate; `Follow` never relaxes; `Grant`
  never relaxes.
- **Exemption outliving the manoeuvre** (by design, B4): a car stuck overlapped with a player who keeps
  standing inside it. It is bounded because B standing lets the car go on; row 2.10 covers the moving
  case.
- **`TnuaNotPlatform` on a car** hides it from every character's ground sensor while it is set: nobody
  can stand on its roof during the pass. Accepted.
- **Hook preconditions:** only cars spawned through `spawn_traffic_car` carry the flag (the only literal
  site, verified). Sleeping pairs are covered by row 2.11.
- **Trajectory shift** (TASK-037 lesson): trace the contact before blaming the change.
- **Pre-existing third-body violations on HEAD** (1.5) can block acceptance and must be escalated, not
  tolerated.
- **Class D band** (0.4 stop condition).
- **Performance:** the detector is O(n) per tick, and the overlap estimate is O(candidates × samples),
  only for standing chains. The hook costs one lookup per flagged touching pair. The benches gate the
  mean.
- **File sizes:** `drive.rs` 662 (logic goes to `progress.rs`); `traffic_support/mod.rs` 735 (new code
  in `third_body.rs`).

## 6. Decisions and open questions

Decisions changed from PLAN.md:
- chain end → least-overlap choice (binding resolved question 1);
- `min(W, B)` → blocker's standing + waiter grace (R4);
- `Grant` relaxation dropped (R5);
- end on separation only, with `physics_only` (R3);
- `TnuaNotPlatform` mandatory, and a clean box pass around a character is tried first (resolved
  question 2);
- `leader` is a consumer (R6).

Kept: `modify_contacts` over `filter_pairs`; own path line in the box; T 18 s with b3 re-anchored; D9
and D10 not built.

Вопросы к владельцу (не блокируют, решаются прогоном):
1. M1: даже с правилом наименьшего перекрытия машина D, скорее всего, пройдёт сквозь машину игрока
   (~1.2 м): другого выхода у неё нет, а стоять дольше 30 с ей запрещено. Варианты: (а) принять
   (рекомендую); (б) разрешить D бросить машину, водитель выходит (это противоречит D4: "восстановление
   или проезд"); (в) держать улицу закрытой (противоречит цели задачи).
