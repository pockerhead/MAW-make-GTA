# PLAN_V2 — TASK-012 (GDD T11): police on foot and arrest

Reviewer: plan-reviewer-1. Base: `PLAN.md` (planner). This is a full plan, not a diff. The implementer follows this file;
where it is silent, PLAN.md's detail applies unchanged.

Cost of error: **mixed**. Silent class (full evidence layer): units over the table, arrest firing early or never, a
busted player keeping guns or wanted, cop crimes without heat, cops chasing the real position instead of
`LastKnownPosition`, a gang regression from the shared fire-line move, the corridor starvation (TASK-010 QA Bug 1),
non-deterministic system order, frame budget at 12 cops. Owner class (mechanism gate + owner run): how cops look, how
the arrest reads, whether 1.5 s is fair, BUSTED colour, SWAT readability.

Pinned (checked in `Cargo.lock` and `~/.cargo/registry/src/index.crates.io-*/`): bevy / bevy_ecs / bevy_state /
bevy_time / bevy_app 0.19.1, avian3d 0.7.0, bevy-tnua 0.32.0, pathfinding 4.16.0, rand_chacha 0.10.0, ron 0.12.2.
No new crate, no Cargo.toml change. `citygen` source is not changed; one test is added to `crates/citygen/tests/
properties.rs` (so `cargo test -p citygen` runs).

Builds: `CARGO_TARGET_DIR=D:/test-gta-like/target`, `-j 4`, one cargo command at a time, in place.

---

## 1. Review notes (issues found in PLAN.md, with evidence)

**Disconfirmation test (done first).** Most concrete counter-example I chose: "QA/tests set `WantedLevel.heat`
directly, so `last_known` stays `None` and the dispatcher (which returns while `last_known` is `None`) never spawns a
cop" (would break AC 1, AC 2 and the whole t11.py). Checked `wanted/search.rs:80`: `search_step` does
`w.last_known.get_or_insert(player)` on every tick with `heat > 0`. **The counter-example did not hold.** The search
for it surfaced the real defects below.

Major:

1. **Nondeterministic order between the dispatcher and `WantedSystems`.** PLAN puts `PoliceSystems` `.after(
   PopulationSystems)`; `WantedSystems` is only `.after(AiSystems::Decide)` (`wanted/mod.rs:174-179`); the chain
   `Perceive -> Decide -> PopulationSystems` (`perception/mod.rs:146-151`) gives no edge between `PopulationSystems`
   and `WantedSystems`. `dispatch_police` reads `WantedLevel` (stars, `last_known`) that `track_search` writes: a
   Res/ResMut conflict with no order runs in an unspecified order. The exact-tick gates (D7 "exactly the derived tick
   count", P1 96 ticks) would be flaky by construction. Fix: `PoliceSystems.after(WantedSystems)` as well.
2. **`Leave -> Respond` breaks "units ≤ table".** With 12 SWAT walking off at 0 stars, one punch on a cop (45 heat,
   1 star) turns all 12 back to `Respond`, and SWAT would then try to arrest (row 1 `arrest: true`). `active` excludes
   `Leave` only for spawning, so nothing caps re-engaged units. Fix: `Leave` is terminal (a leaving cop never
   re-engages; it still counts as a police witness/spotter because `record_crimes` / `track_search` look at
   `Faction::Police`, which is correct). New gate D9.
3. **N1 as a hard RED gate contradicts the AC and the GDD.** GDD §6.6: navmesh only after a reproduced stuck chase,
   "решение в T11 по факту". A per-cop "must reach in 40 s, never loosen" assertion makes one stuck cop block
   `cargo test` and the whole task. The stuck evidence is data for a decision, not a gate. Fix: N1 asserts liveness
   (at least one cop reaches each spot), prints per-cop reach times/stuck list; the implementer and QA record the
   numbers and the navmesh decision.
4. **P6 fixture lets the searching cop re-see the player.** `graph_app(10)` has search points at z = 10; from
   (10, 10) the line to the hidden player (0, 22) crosses the wall plane z = 14 at x = 6.67, outside the wall
   (x ∈ [-6, 6], `world/test_area.rs:18`), 29.7 m < 35 m view distance. A cop that re-sees flips to `Attack`, so
   "≥ 2 search points in 20 s" is flaky. Worked fix below (graph on z ≤ 0, |x| ≤ 12 only: every line to (0, 22)
   crosses z = 14 at |x| ≤ 12·(8/22) = 4.36 < 6, so the wall hides the player from every point and every walk
   between them).
5. **N2 50-seed sweep duplicates an existing property harness.** `crates/citygen/tests/properties.rs:320`
   `hospital_anchor_on_sidewalk` already sweeps `common::layouts()` (SEEDS + SWEEP, cached `OnceLock`) with the same
   `sidewalk_anchor`. Put the station check there, for every `PoliceStation` (with `police_stations: 2` both must
   anchor, not only the first), not a new 50-city generation in `gta_sim`.
6. **Existing runtime QA scenarios are not re-run.** `t10.py` (wanted) raises heat over BRP; after T11 real cops
   spawn, arrest or shoot the player mid-scenario. PLAN's Step 13 runs only `t11.py`. Add `t10.py` and `t9.py` to
   verification; adapt only by named mutation (e.g. player armour, or heat reset) if cops interfere.
7. **Busted vs Wasted in the same tick.** `detect_player_death` (HealthSystems::Death) and `arrest_player` (after
   `Decide`) can both `NextState::set` in one tick; last writer wins, and `arrest_player` runs later. A player killed on
   the tick the hold completes (or knocked down by a lethal blow) would be Busted and respawned at the station with
   full health. Fix: `arrest_player` skips a player with `Health.current <= 0` (the `Without<Dead>` filter alone relies
   on the deferred `Dead` insert having been applied).

Minor / clarifications:

8. Step 3 wording "after `pinned` returns `(None, None)`, try `queue_slot`" reads as the pinned branch. Correct: when
   **not** pinned, try `queue_slot` before the old close-in seek; the pinned branch still returns `(None, None)`.
9. `hold_fire` extraction must preserve two gang details PLAN does not spell out: `plan = member.reposition.take()`
   runs **before** the block unconditionally (a clear line drops the kept spot), and in the `yields` branch
   `member.reposition` stays `None`. The returned `kept_spot` is `None` in both of those cases.
10. `police_models` reuses `character-male-c.glb`, which is already a civilian model (`visual.ron` civilian list), and
    civilian tint `(0.55, 0.75, 1.0)` is bluish: a tinted civilian male-c may read as a cop. Only `female-e` and
    `female-f` are unused. Keep the three-model list (the owner decides), but put this on the owner checklist.
11. `civilian_bench.rs` sets a view and fires `ShotFired` near civilians: calls can now raise heat and spawn cops (who
    may arrest the player → Busted) during the bench. Watch item: run it; if cops change its outcome, pin heat to 0 in
    the bench by a named mutation, do not loosen its assertions.
12. PLAN §5 "Open questions" are already resolved by the orchestrator (TASK_FINAL "Resolved questions"): drop them as
    open, cite the resolution.
13. `lib.rs` must declare `pub mod police;` (tests use `police_unit_bundle`, `PoliceUnit`); PLAN only says "police
    domain". `compose_sim`'s `add_plugins` tuple grows from 13 to 14 elements: Bevy 0.19.1 implements `Plugins` for
    tuples up to 15 (`bevy_app-0.19.1/src/plugin.rs:186-193`), so it compiles.
14. Crime gates C1..C4 run on the test floor **without** a sidewalk graph: `navigation/mod.rs:347-349` gates all of
    `NpcSystems` on `resource_exists::<SidewalkGraph>`, so the cop FSM stays off and the cop does not walk away
    (0 stars → `Leave`). `record_crimes` is `PlayingSystems` and runs. State it in the tests so nobody "fixes" it by
    adding a graph.
15. Busted respawn: GDD §3.4 says weapons are confiscated; GTA IV also strips body armour (search below).
    `Health::full` already resets the player's health; whether it zeroes armour is whatever `Health::full` does today
    for Wasted — keep one shared `respawn_at` and do not add an armour rule (not in GDD).

Verified as correct in PLAN (spot-checked in code): all cited line ranges in `gang/behavior.rs` (`Ctx` 121-127,
`head_for` 144-180, hold-fire 443-495, motion 599-606), `fire_line.rs` `unblock` 161-191; `crimes.rs` `classify`
141-149, `persons` 164, cop witness 234-237, `resolve` 118; `search.rs` `track_search` 97-133; `flow/mod.rs`
`NpcSystems` 53-57; `population::spawn_points` private (396-426), `outside_cone` / `occluded` /
`OCCLUSION_RAYS_PER_POINT` public, `PopulationLoad` reset at the start of `spawn_civilians`; `gang::fsm::band_move` is
already `pub`; `StateTransition` is inserted after `PreUpdate` (`bevy_state-0.19.1/src/app.rs:335`);
`FixedTimesteps(n)` advances `Time<Real>` by `n` timesteps (`bevy_time-0.19.1/src/lib.rs:181-183`);
`SystemCondition::or_else` (`bevy_ecs-0.19.1/src/schedule/condition.rs:508`); `DespawnOnExit<S: States>`
(`state_scoped.rs:149`); `hitscan` has no faction sparing (only `apply_strikes` spares), so cop→cop bullets hurt and
the hold-fire rule is the only protection; test floor 80×80 m around the origin; dispatch fixture: 19 spawn points
(A: 8 inner + 2 nodes, B: 7 + 2), 60.2-88.5 m from (-30, -30); corridor geometry in `scratch/corridor_geometry.txt`
(slot (-0.8, -10), rear-line perp 0.797 > limit ≤ 0.512, walk passes the front body at 0.784 m ≥ 0.6 m, slot body
edge at 1.1 m < wall face 1.2 m in the 2.4 m corridor).

## 2. Updated understanding (corrections only; PLAN §1 otherwise holds)

- `NpcSystems` runs only with `in_state(Playing | Wasted)` **and** `resource_exists::<SidewalkGraph>`
  (`navigation/mod.rs:347-349`). Police systems in `NpcSystems` need a graph in every test that expects cop AI.
- `GangSystems` additionally needs `GangTerritories`; the police do not join `GangSystems`.
- `WantedSystems` (`record_crimes → take_calls → track_search → forget_crimes`) is ordered only after
  `AiSystems::Decide`; nothing orders it against `PopulationSystems`.
- `heat` never decreases except to 0 (`search_step` clears the whole `WantedLevel`; nothing subtracts heat), so stars
  only rise or drop to 0. The only path to more units than the row is re-engaging old units after a reset.
- Hitscan damage does not consult factions; `GangConfig::spares` returns `false` for any non-gang attacker
  (`gang/mod.rs:349-354`), so police sparing lives only in the police hold-fire predicate.
- `drive_characters` runs in every state (no state gate), which is why `Cuffed` must zero the walk basis.
- `civilian_bench.rs`, `civilian_city.rs`, `gang_city.rs`, `street_spawn.rs` set a camera view; of these only the
  bench produces crimes (fake `ShotFired`).

## 3. Revised approach

Unchanged in substance from PLAN §2 (shared `tactics/` layer with a `Discipline` view, corridor queue slot, `police/`
domain with dispatcher + pure FSM + arrest, `GameState::Busted` mirroring Wasted, client looks and BUSTED screen,
navmesh decided by evidence, Q2=B off), with these changes:

- Schedule: `configure_sets(FixedUpdate, PoliceSystems.after(PopulationSystems).after(WantedSystems).in_set(
  NpcSystems))`. Every police system has a stated order against every writer of what it reads.
- Cop FSM: `Leave` is terminal (only `Dead` leaves it). New units come only from the dispatcher.
- Arrest never overrides a death in the same tick.
- Navmesh evidence is a report, not a gate.
- Station anchor property lives in the citygen property sweep.

Buffered signals: no new `Message`. Police read `ShotFired`, `MeleeHit`, `DamageDealt` with their own readers. No
observers. The flow change uses `NextState` (buffered by design, applied in `StateTransition`).

## 4. Revised steps

### Step 1 — data

`assets/police/escalation.ron` (new) exactly as PLAN Step 1 (tuple `stars` of 5 rows; `patrol` / `swat` UnitSpec;
`spawn_ring (40, 90)`; `spawns_per_tick 1`; `arrest (distance 1.5, stand_distance 1.0, seconds 1.5,
break_free_distance 3.0, hostile_seconds 5.0)`; `search_arrive_distance 3.0`; `combat` block). Comments one line each.
Also: `wanted.ron` heat `punch_cop: 45, wound_cop: 80, kill_cop: 150` (+ comment "cop rows are reported always");
`respawn.ron` `busted_arrest: 2.0, busted_screen: 3.0`; `strings.ron` `busted: "BUSTED"`, `busted_color`;
`visual.ron` `police_models` (male-c, female-e, female-f), `police_tint`, `swat_tint`. Every number is starting data
for the owner run; none becomes a `const`.

### Step 2 — shared gunfight layer `crates/gta_sim/src/tactics/` (pure move)

As PLAN Step 2 (`git mv` of `fire_line.rs`, `tactics/mod.rs` with `Ctx`, `Seek`, `Motion`, `head_for(avoid: &mut
f32)`, `walk`, `select`, `hold_fire`, `apply_motion`; `Discipline<'a>`; `GangCombatConfig::discipline()` with
`pressed_distance = melee_distance.1`; `Shooter.gang: u8 → faction: Faction`; `lib.rs` `pub(crate) mod tactics;`).
Preserve exactly:
- `let plan = member.reposition.take();` before the block, unconditionally, in `gang_fsm`;
- in the `yields` branch `clearing = Some(Motion::Stand)` and `member.reposition` stays `None`;
- `hold_fire` returns `(line_blocked, kept_spot, clearing)`; the gang writes `member.reposition = kept_spot` only when
  `unblock` ran (same as today).
Gate: after this step alone, `gang_fire_lines` (incl. `of_two_members_blocking_each_other_the_lower_index_moves`),
`gang_combat`, `gangs`, `gang_city` are green, run once and recorded. No behaviour change is intended; any red here is a
move bug, not a gate to adjust.

### Step 3 — corridor fix (both roles), `tactics/fire_line.rs`

`queue_slot(spatial, b, radius, rays) -> Option<Vec3>` exactly as PLAN Step 3 (nearest non-yielding blocker from
`b.line.blockers(b.chest, b.to, b.shields)`; `side = (-ahead.z, 0, ahead.x)`; offset `radius + b.line.clearance`;
nearer side first, tie `+side`; first that passes `usable`).
`unblock`, `plan == None` branch: `if pinned(b, d) { return (None, None); }` then `if let Some(slot) =
queue_slot(..) { return (Some(slot), Some(Motion::Yaw(steer(b.chest, slot), d.reposition_gait))); }` then the old
close-in seek. The pinned branch is unchanged. One-sentence doc update on `unblock`.

### Step 4 — wanted: cop crimes and re-exports

As PLAN Step 4 (`HeatTable` + 3 fields + `of` + `validate`; `Crime::{PunchCop, WoundCop, KillCop}`, `Victim::Cop`,
`classify` rows; `kinds` `(Has<Civilian>, Has<GangMember>, Has<PoliceUnit>)`; `persons` adds `With<PoliceUnit>`;
cop-crime ids reported before the witness early-return; `resolve(Body)` accepts `Kill | KillCop`; `pub(crate) use
search::{cop_sees, eye, witnesses}`; `crimes.rs` unit `HEAT` literal + 3 `classify_table` rows).
`WantedPlugin`: `.add_systems(OnExit(GameState::Busted), (reset_wanted, drop_queued_calls))`.

### Step 5 — flow: Busted

As PLAN Step 5 (`GameState::Busted`, `BustedPhase { Arrest, Screen }` sub-state, `BustedSystems` in `Update`,
`BustedClock`, `NpcSystems` condition `Playing.or_else(Wasted).or_else(Busted)` + doc, `enter_busted`,
`advance_busted` on `Time<Real>`, `OnExit(Busted)` → `respawn_at_station`, `drop_queued_damage`,
`drop_queued_input`; shared `respawn_at(...)` extracted from `respawn_player`; `RespawnConfig.busted_arrest /
busted_screen` validated finite ≥ 0; `Cuffed` marker in `character/`, `drive_characters` treats `Has<Cuffed>` like
`dead`; `reset_player_melee` also on `OnExit(Busted)`).
`respawn_at_station` resets the player `Loadout` to `Loadout::default()` and removes `Cuffed` with `try_remove`.

### Step 6 — world: police station spawn

As PLAN Step 6 (`PoliceStationSpawn { point, along }` Reflect resource; test-area fixture `(-20, 0, 20)`; city
`station_spawn` = hospital logic on the first `BuildingKind::PoliceStation`, same margin and error exit).
Property gate in `crates/citygen/tests/properties.rs`: `police_station_anchor_on_sidewalk` over `layouts()`, **every**
`PoliceStation` index gets `Some` from `sidewalk_anchor(layout, &params, idx, pickup_spacing())` and lies on a
sidewalk (reuse the hospital test's body through a small shared helper in that file, not a copy). Flip: pass an
out-of-range building index for one station → `None` → RED (the existing `sidewalk_anchor_rejects_bad_indices`
shows it returns `None`).

### Step 7 — population

`SpawnPoint` + fields and `spawn_points` become `pub(crate)`. Nothing else.

### Step 8 — police domain `crates/gta_sim/src/police/` (`lib.rs`: `pub mod police;`)

As PLAN Step 8 (config structs + `validate` + `validate_ring(despawn_distance)`; `UnitKind`, `CopState`,
`PoliceUnit` with `#[require(Character, Perception, Offscreen, Route)]`, `PoliceRng` stream 3, `PoliceDispatcher`,
`ArrestAttempt`, `PoliceAlert`, `police_unit_bundle`; `fsm.rs`, `behavior.rs`, `dispatch.rs`, `arrest.rs`), with
these changes:

- Sets: `configure_sets(FixedUpdate, PoliceSystems.after(PopulationSystems).after(WantedSystems).in_set(
  NpcSystems))`; `(dispatch::despawn_police, dispatch::dispatch_police.in_set(PlayingSystems)).chain().in_set(
  PoliceSystems)`; `police_death` in `HealthSystems::Death` + `NpcSystems`; `police_alert` in `AiSystems::Perceive`;
  `police_fsm` in `AiSystems::Decide`; `arrest_player.after(AiSystems::Decide).before(WantedSystems).in_set(
  PlayingSystems)`. Flat tuples, no nested `.chain()` (TASK-008 lesson). Record the resulting order of every reader
  and writer of `WantedLevel` in the implementer summary.
- `next_state`: `Dead → Dead`; `Leave → Leave` (terminal); `stars == 0 → Leave`; then PLAN's rows for `Respond`,
  `Search`, `Arrest`, `Attack` unchanged.
- `despawn_police`: skips `Dead` (corpses belong to `age_corpses`); despawns when `offscreen >=
  despawn_offscreen_seconds` and (`state == Leave` or flat distance > `despawn_distance`).
- `dispatch_police`: `active` = not `Dead`, not `Leave` (unchanged); `Leave` units never come back, so the table bound
  holds after a reset.
- `arrest_player`: player query adds `&Health`; return when `health.current <= 0` (a death in the same tick wins).
  The start tick of an attempt runs `arrest_step` with `hold = 0` (so it ends at `Hold(dt)`); the implementer derives
  the exact Busted tick from this and asserts it in P1.
- `compose_sim`: load + validate + `validate_ring(population.despawn_distance)`, insert, add `PolicePlugin { seed:
  combat_seed }` (plugin tuple 14 ≤ 15).

### Step 9 — client

As PLAN Step 9 (character config + preflight for `police_models`; `CharacterAnimations` chains police models;
`body_look` police branch; `LookQuery` + `Option<&PoliceUnit>`; `Cuffed` shows `Cower` (crouch); `civilian_gate.rs:66`
chains police models; generic title screen for WASTED/BUSTED; `menu/config.rs` `busted`, `busted_color` validated;
hud desaturate/restore on Busted; input `release_held_actions` on `OnEnter(Busted)`; camera `reset_pivot` on
`OnExit(Busted)`; `src/visuals/police_gate.rs` pose-range + tint gate, run 3 times).

### Step 10 — runtime QA `tools/qa/scenarios/t11.py`

As PLAN Step 10 (busted run with `busted.png`, loadout/position/heat checks, per-cop stuck log into `summary.json`;
SWAT run at `stars[3].heat` with `swat.png` and `frame_report()`; every constant read from RON; `log_errors` empty;
`shutdown`). The `stuck` list and reach times feed the navmesh decision written into QA_REPORT.md.

### Step 11 — config gates `crates/gta_sim/tests/config_police.rs`

As PLAN Step 11 (move `sabotaged` to `tests/common/mod.rs`; shipped loads; unknown field; `stars[3].swat` 9;
`stars[2].units` 3; `break_free_distance` 1.2; `spawn_ring` `(40.0, 160.0)`; 4 rows → `length 5`; `kill_cop: 0`;
`busted_screen: -1.0`). Each sabotage strictly on the failing side (TASK-007 lesson) and yields a different error
than its neighbours.

### Step 12 — headless gates

Fixtures `tests/police_support/mod.rs` as PLAN (`spawn_unit` through `police_unit_bundle`, position asserted after
one tick; `set_cop_state`; `cop`; `esc`; `assert_shipped` with `GATE BROKEN` on every shipped number the worked
examples use). Every app that needs cop AI has a sidewalk graph (see §2).

`tests/police_arrest.rs`:
- **P1..P5, P7, P8** as PLAN, with P1's exact tick derived from the start-tick rule of Step 8.
- **P6 `lost_player_is_searched_at_last_known`** (revised fixture): test graph square nodes (-12, 0, -24),
  (12, 0, -24), (12, 0, 0), (-12, 0, 0) (edges around), player at the origin, heat 180, armour 1e6, cop feet
  (-12, 0, 0) facing +X. After `Attack` + `seen`, `place_player` at (0, 0, 22). Worked: every point P = (x0, z0) with
  |x0| ≤ 12, z0 ≤ 0 → the line to (0, 22) crosses z = 14 at x = x0·(8/(22 − z0)) ⇒ |x| ≤ 12·8/22 = 4.36 < 6: behind
  the wall (height 4 m > eye). Assertions as PLAN (within 8 ticks `seen == false`, `last_known` within 0.05 m of L,
  cop `Respond`; `dest == last_known` every tick until `Search`; flat distance to L ≤ 3.0 within 5 s; search points
  within 70 m of L; ≥ 2 distinct points in 20 s) **plus** a `GATE BROKEN` assert that `WantedLevel.seen` stays false
  for the whole search phase (a re-sight is a fixture failure, not a pass). Flip: Respond heads for the player →
  `dest` assert RED.
- **P9 `death_beats_arrest_in_the_same_tick`** (new): P1 setup until the hold is one tick short of 1.5 s; named
  mutation `set_health(0)` so the death and the completing hold land in the same tick → `GameState::Wasted`, never
  `Busted`. Flip: drop the health check in `arrest_player` → `Busted` → RED. (Derive the tick from P1's arithmetic.)

`tests/police_dispatch.rs`: D1..D8 as PLAN (fixture: player (-30, 0, -30), edges A and B, `chase_view(feet,
(-1,0,-1).normalize())` with camera at feet − 3.8·dir, `max_civilians = 0`, armour 1e6). For D6 the implementer
computes which B points are in view with the 60° widened half-cone (x ≥ 7.5 on B from the player) and asserts only
"no unit on A". Plus:
- **D9 `reset_units_do_not_rejoin`** (new, AC 2): row 5 reached (12 active), heat → 0, all `Leave`; then heat → 40
  (row 1) while the old units are still alive: every tick for 320 ticks active ≤ 2 and no SWAT among units not in
  `Leave`. Flip: restore `Leave → Respond` → 12 active → RED.
- Unit tests in `police/fsm.rs` as PLAN; `next_state_table` gains `Leave` × every sense → `Leave`.

`tests/police_crimes.rs`: C1..C4 as PLAN, on the test floor **without** a graph (cop FSM off by design, comment says
why).

`tests/police_fire_lines.rs` and the gang corridor cases in `tests/gang_fire_lines.rs`: as PLAN (G-C1..G-C4,
P-C1..P-C4, P-B1); add one 18 m rear variant for the gang (QA3 V6 layout). Flip for the corridor cases: remove
`queue_slot` → rear 0 shots → RED.

`tests/police_city.rs`:
- **N1 `cops_reach_the_player_in_the_city`** (evidence, revised): three sidewalk spots, heat 180, view fixed,
  `max_civilians = 0`, 40 s each. Assert only liveness: at least one cop reaches `sees` or ≤ `keep_distance.1` at each
  spot. Print per-cop reach time or "stuck at d m". The implementer copies the printout into its summary; a stuck cop
  is a finding for the navmesh decision (new task), not a red build.

`tests/police_bench.rs`: as PLAN (mean only, 10× the recorded probe mean).

Existing tests kept green, touched only where needed: `crimes.rs` units, `respawn.rs`, `wanted*.rs`, all gang tests,
`civilian_bench.rs` (watch item, §1.11).

### Step 13 — verification

`cargo build -j 4`; `cargo build -p gta_like --features dev -j 4`; `cargo clippy --workspace --all-targets -j 4 -- -D
warnings`; `cargo test -p gta_sim -j 4`; `cargo test -p citygen -j 4` (a test is added there);
`cargo test -p gta_like --bin gta_like -j 4` ×3; `cargo tree -p gta_sim -e normal -i bevy_render` empty;
`python tools/qa/tree_check.py`; `python tools/qa/scenarios/t11.py --out <dir>`; **re-run `t10.py` and `t9.py`** (cops
now exist in those runs). Run each new exact-tick gate (P1, P3, P9, D7) 3 times to show determinism after the
`after(WantedSystems)` edge. Phantom-red rule: `touch crates/*/src/lib.rs` before trusting red in untouched code.
Record every flip (perturbed input, RED, GREEN).

### Owner checklist (QA writes it into QA_REPORT.md, in Russian)

PLAN's list, plus:
- [ ] Отличаются ли копы (синий тинт) от мирных, особенно от голубоватого мирного на модели male-c?
- [ ] После ареста или смерти старые копы уходят и не возвращаются; новая звезда приводит новых копов по таблице.

## 5. Risk areas

- **Shared-layer move changes gang behaviour.** Step 2 is a separate green checkpoint; preserved details in §1.9.
- **Queue slot reaches layouts that used to close in.** Only the non-pinned fallback changes. A gang gate red after
  Step 3 means a layout reached the fallback: diagnose it, do not widen the gate. Corridors < 2.2 m and three-in-a-file
  may still starve (owner line).
- **Order of `WantedLevel` readers/writers.** Now explicit; any new police system that reads `WantedLevel` must name its
  edge to `WantedSystems`.
- **Busted leaks** (TASK-006/007 class): damage, input, melee, calls, arrest state, pivot, saturation, input release
  all have Busted twins; P7/P8 gate the silent ones. NPC `fire_requested` set during Busted (gangs keep aiming at the
  cuffed, live player; `fire_weapons` is `PlayingSystems`) fires once on the first Playing tick at the old spot:
  harmless because the player was moved, owner-visible if not; not gated.
- **Death vs arrest in one tick** — P9.
- **Re-engaged units** — `Leave` terminal, D9.
- **At 5 stars after 4 stars the 4 patrol units stay** (spawn fills to 12 with SWAT): 8 SWAT + 4 patrol, not "12 SWAT"
  of GDD §6.4. Within "≤ table"; owner-visible; left as is.
- **Dispatcher in existing view tests** (`civilian_bench.rs`): watch item.
- **Station anchor** — citygen property sweep over all stations.
- **Frame budget at 12 cops** — bench on the mean only (TASK-009 lesson).
- **Navmesh** — N1 + t11 stuck log are evidence; a stuck chase opens its own task.
- **Feel** (1.5 m / 1.5 s arrest, SWAT tint, BUSTED colour, cops walking off) — owner run, knobs in data.

## 6. Resolved questions (orchestrator, owner delegation — not open)

Q2=B off; break free = farther than 3 m from the arresting cop during the hold; wanted and crimes reset on exit from
Busted; "fewer civilians at 5 stars" deferred (TASK_FINAL "Resolved questions").

## 7. Research notes

- GTA IV: arrest on foot only at one star; resisting by running away or fighting "immediately escalates a one-star
  wanted level to two-stars and causes the officer to open fire"; after arrest the player respawns at the nearest
  police station "stripped of weapons and body armour" (https://gta.fandom.com/wiki/Busted ,
  https://gta.wiki/w/Wanted_Level_in_GTA_IV , https://www.grandtheftwiki.com/Busted — community wikis, secondary).
  Matches the resolved break-free rule; armour stripping is not in our GDD, so not added.
- Tactical position selection (candidates + line-of-fire and claim filters): Game AI Pro ch. 26 (Jack); Killzone AI
  (Straatman et al.) — the queue slot is one more candidate of that kind.
- Engine behaviour: verified in pinned sources (cited in §1), which outrank web docs for this project.

children: 0 launched / 0 reported.
