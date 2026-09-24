# PLAN FINAL — TASK-011 (GDD T10): wanted level core

Cost of error: **mixed, mostly silent.** Heat bookkeeping fails silently and shows up weeks later as "stars from
nowhere" or "stars never come": double counting of one corpse (binding orchestrator note), heat from an
unwitnessed crime, gang crimes charged to the player, stale calls or incidents leaking across `Wasted`, a search
timer that never or always clears. Those get headless gates with worked numbers. The owner class (star look, blink
feel, whether 10 s / 40 m feels right, whether the witness loop is fun) gets the owner checklist plus BRP
screenshots and one pure look-table test, nothing more.

Pinned (`Cargo.lock`, sources under `~/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/`): bevy / bevy_ecs /
bevy_time / bevy_state / bevy_remote 0.19.1, avian3d 0.7.0, bevy-tnua 0.32.0, ron 0.12.2. **No new crate.**
`citygen` is not touched. Fixed tick = `Time<Fixed>` default 15 625 µs = 64 Hz (`bevy_time-0.19.1/src/fixed.rs:76`,
nothing in the repo overrides it), so `dt = 1/64` is exact in f32.

Build/test everything with `CARGO_TARGET_DIR=D:/test-gta-like/target` and `-j 4` (host under memory pressure), one
build at a time, in-place (no worktree, no second target).

---

## 1. Summary

Rewrite the `wanted` stub (`crates/gta_sim/src/wanted/mod.rs`, 23 lines today: `WantedLevel { stars }` + reset on
`OnEnter(Wasted)`) into a headless wanted domain driven by buffered Bevy `Message`s. Player crimes (`Punch`,
`Shooting`, `Wound`, `Kill`) are recorded as **incidents** in a `Crimes` resource, keyed by attack id
(`AttackSerial`, already in `DamageDealt.shot`) and victim. An incident adds its GDD §6.4 heat **once**, on its
first report. A report comes from (a) a `Faction::Police` entity with LOS ≤ 50 m at record time (cop fixture until
T11) or (b) a completed civilian call: `CivilianState::Report` now carries the `Cause` of the threat it reacted to
(`Attack(id)` for gunshot/fight, `Body(entity)` for a corpse) and emits a `PoliceCall` message exactly once on
completion. `Attack(a)` resolves the incidents containing attack `a`; `Body(v)` resolves **only the player's
`Kill` of `v`**. Search (GDD §6.4 IV/V hybrid): `WantedLevel { heat, stars, last_known, seen, hidden }`; unseen and
strictly outside the star circle for `clear_seconds` continuously → wanted cleared; seen → timer reset, circle
moved. All tuning in new `assets/wanted/wanted.ron` (strict loader); the client draws 5 `★` under the ammo line
(numbers in `assets/ui/strings.ron`), blinking while unseen (GDD §6.4). Runtime QA `tools/qa/scenarios/t10.py`.

---

## 2. What exists today (verified in code)

- **Combat messages** (`FixedUpdate`, `HealthSystems::Damage`, `PlayingSystems`; `combat/mod.rs:82-108`):
  - `ShotFired { shooter, weapon, muzzle }` (`combat/hitscan.rs:56-62`), written at `hitscan.rs:221` **before** the
    pellet loop; `let shot = serial.next_id()` at `:190` is not carried in it.
  - `DamageDealt { shooter, shot, target, point, damage, headshot, killed }` (`hitscan.rs:75-87`), written per
    pellet at `hitscan.rs:~263` and per strike in `melee::apply_strikes` (`melee.rs:527-535`). `fire_weapons` skips
    targets with `health.current <= 0` (`hitscan.rs:249-251`), so only live targets get `DamageDealt`.
    The shotgun has `pellets: 10` (`assets/combat/weapons.ron`): one trigger pull can write several non-lethal
    `DamageDealt` then one `killed: true` for the same `(shot, target)`.
  - `MeleeHit { attacker, target, point, knockdown }` (`melee.rs:284-289`), written at `melee.rs:536` right after its
    `DamageDealt`; `Strike.attack` (`melee.rs:301`) has the id.
  - `AttackSerial` (`combat/mod.rs:29-38`): monotonic, first id 1, never 0 in practice.
- **Perception** (`perception/mod.rs`): `StimulusLog(Vec<(u64, ThreatKind, Vec3)>)` (`:89-91`); `collect_stimuli`
  (`:175-212`) logs Gunshot (`shot.muzzle`), Fight (`hit.point`), and sets `Hurt` pending on a hit civilian;
  `perceive` (`:215-289`) offers log sounds within radius, the nearest corpse in LOS ≤ `corpse_sight` (skipped while
  the civilian is in `Report`, `:259-266`), aimed guns; keeps the **nearest**. `Threat { kind, at, distance }`
  (`:62-68`) has no identity. `sight_blocked` (`:108-116`) casts a `GameLayer::World`-only ray (characters never
  block sight).
- **Civilians** (`civilian/mod.rs`): `Report { progress }` (`:126-128`); entered in `react` (`:258`) only when
  `choose_reaction` picks Report (reportable: Corpse always, Gunshot/Fight at ≥ `report_min_distance` 25 m,
  `reaction.rs:24-28`). `(Report, Some(threat))` → `react(.., allow_report=false)` → Flee/Cower only (`:283-285`);
  `(Report, None)` → progress; `progress >= 1.0` → `Wander` (`:300-307`) — the **only** Report→Wander edge.
  `civilian_death` (in `HealthSystems::Death`, before `AiSystems::Decide`) sets `Dead` first, and `civilian_fsm`
  `continue`s on `Dead` (`:378-380`). `arrive` (`:329-356`) can turn Wander into Idle after `next_state`.
  `civilian_fsm` is registered only in `CivilianPlugin` (`:201`). After completion the witness can see the same
  corpse again and call again (TASK-009 note: up to ~7 calls per corpse).
- **Corpses**: victim entity itself gets `corpse_components()` (`Dead`, `Corpse{age}`) for civilians
  (`civilian/mod.rs:224`) **and** gang members (`gang/behavior.rs:661`); despawned by age/cap (`population/mod.rs:313-334`).
  `Entity` includes its generation, so a recycled id never equals an old `Body(v)`.
- **Factions**: `Faction { Player, Gang(u8), Police }` component (`gang/mod.rs:31-37`); player has `Faction::Player`.
  `GangSystems` runs only with `GangTerritories` (`gang/mod.rs:509-514`) — never on the test floor.
- **Flow** (`flow/mod.rs`): `PlayingSystems` (fixed, `Playing` only), `NpcSystems` (fixed, `Playing` or `Wasted`).
  Its doc comment says a paused-state reader must "clear the NPC message readers' backlog on exit".
  `(AiSystems::Perceive, AiSystems::Decide, PopulationSystems).chain().in_set(NpcSystems).after(HealthSystems::Death)`
  (`perception/mod.rs:136-142`). `OnExit(Wasted)` already runs `respawn_player`, `drop_queued_damage`
  (`Messages<DebugDamage>::clear()`), `drop_queued_input` (`flow/wasted.rs`). `GameState` = Loading/Playing/Wasted
  (no Busted: T11).
- **Composition**: `compose_sim` (`lib.rs:38-139`) loads + validates every config, inserts it, adds plugins,
  `WantedPlugin` last. Test harness `tests/common/mod.rs`: `composed_app` → `compose_sim`, `finish()`, `cleanup()`,
  `TimeUpdateStrategy::FixedTimesteps(1)`; helpers `headless_app`, `run_ticks`, `settle`, `place_player`,
  `position`, `set_aim`, `set_action`, `set_loadout`, `spawn_civilian`, `set_civilian_state`, `civilian_state`,
  `position_of`, `set_health_of`, `test_graph`, `Shots`, `spawn_member`, `write_damage`, `game_state`, `calm()`.
- **Test floor** (`world/test_area.rs`): 80×80 floor; wall 12×4×0.5 centred (0, 2, 14) (x ∈ [−6, 6],
  z ∈ [13.75, 14.25]); boxes at (10,·,10), (13,·,10), (16,·,10); ramp/box near x = −10, z ∈ [−26, −11]; box
  (10,·,−17.4); steps x ∈ [8.5, 11.5], z ∈ [−14.4, −12.0].
- **HUD** (`src/hud/`): top-right bars (`mod.rs:61-97`), ammo text at `top = margin + 2*(bar_height+bar_gap)`
  (`weapon.rs:41-60`), witness bar reads `Report { progress }` (`witness.rs:41`), gate `witness_gate.rs:79,85,98`.
  HUD numbers live in `assets/ui/strings.ron` → `UiConfig.hud: HudLayout` (`src/menu/config.rs`, "no separate hud.ron").
- Existing users of the stub: `tests/respawn.rs:97` (`stars = 3`) and `:107` (expects 0 after death).
- Literal sites that the new fields break (grep-verified, complete list): `civilian/mod.rs:258`, `:283`, `:300`,
  `:305`; `civilian/reaction.rs:73`; `perception/mod.rs:206`, `:245`; `tests/civilians.rs:344,351,381,401,426`;
  `tests/civilian_bench.rs:107`; `src/visuals/character_gate.rs:488`; `src/hud/witness.rs:2,41`;
  `src/hud/witness_gate.rs:79,85,98`; `tests/respawn.rs:97,107`. `tools/qa` scripts do not reference
  `Report`/`ShotFired`/`MeleeHit`/`WantedLevel`.

Evidence already on disk: `scratch/ron_array_probe.txt` (ron 0.12.2 reads `[StarRow; 5]` only from tuple syntax;
4 rows → `ExpectedDifferentLength { expected: "an array of length 5", found: 4 }`, Display "Expected an array of
length 5 but found …"); `scratch/font_star_glyph.txt` (both shipped Inter fonts contain U+2605 `★`).

---

## 3. Design decisions (binding for the implementer)

### 3.1 Crime table (GDD §6.4, T10 rows only; offender must be the `Player` entity)

| Crime | Recorded when | Keyed / merged by | Heat (`wanted.ron`) |
|---|---|---|---|
| `Punch` | melee `DamageDealt` (its `shot` also appears in this tick's `MeleeHit`s), not killed, target has `Civilian` | same victim | `punch_civilian` 5 |
| `Wound` | gun `DamageDealt`, not killed, target has `Civilian`, **and no `killed` DamageDealt this tick with the same `(shot, target)`** | same victim | `wound_civilian` 30 |
| `Kill` | `DamageDealt.killed`, target has `Civilian` or `GangMember` | same victim | `kill_person` 40 |
| `Shooting` | player `ShotFired` with ≥ 1 person (Civilian or GangMember) within `shooting_radius` 15 m of `muzzle` **at the shot** (rule 3.3) | offender episode: joins the offender's last `Shooting` if `now - last <= shooting_merge_seconds` | `shooting_near_people` 10 |

Gang wound/punch = no crime (not in the table). Non-player offenders record nothing (heat is the player's). Cop rows,
car theft, run-over: T11/T14/T15 extend `HeatTable`/`Crime`.

### 3.2 Witness → incident identity

`Cause::Attack(u32) | Body(Entity)` in perception. `Threat.cause: Option<Cause>`: Gunshot/Fight/Hurt →
`Some(Attack(id))`, Corpse → `Some(Body(e))`, Aimed → `None`. `Report { progress, about: Option<Cause> }` captures
`threat.cause` on entry. On Report → Wander the civilian writes `PoliceCall { caller, about }` (only when `about` is
`Some`, which is always the case for a reportable threat).

Resolution in `wanted`:
- `Attack(a)` → every incident whose attack list contains `a` (the shooting episode and any wound/kill/punch done by
  that very trigger pull or swing).
- `Body(v)` → **only** an incident with `crime == Kill && victim == Some(v)` (offender is always the player since
  only player crimes are recorded). A body proves a death, not older unseen punches/wounds.

Rejected (logged): resolving by offender (reports unwitnessed crimes elsewhere); a wanted-owned geometric witness set
(duplicates perception radii, drifts); `Body(v)` → all incidents of `v` (worked case: private punch + gang/unattributed
kill → a corpse call charges the punch).

### 3.3 "Near people" is decided at the shot

`fire_weapons` writes `ShotFired` before applying damage; `record_crimes` runs later in the tick, when a victim
killed by this shot already has `Health.current <= 0`. Rule: a person counts as near for player shot `s` if within
`shooting_radius` of `s.muzzle` (3D distance of `Position`) **and** (`Health.current > 0` **or** a `DamageDealt`
this tick has `shot == s.attack && target == person && killed`). A body dead before the tick (no such DamageDealt,
health ≤ 0) does not count. Accepted edge: a person killed in the same tick by a *different* attack that fired later
in the `fire_weapons` iteration is treated as not near (conservative, sub-tick, documented in a one-line comment).

### 3.4 Report accounting and bounds

- `Crimes::report(id, &HeatTable) -> Option<(u32, Vec3)>` returns `Some` only on the first report (sets
  `reported = true` before returning). Repeat calls, and cop + civilian reports of one incident, add nothing and do
  not move the circle or reset the timer.
- `take_calls`: per `PoliceCall`, resolve → report each → sum the new heat; call `apply_report` once per call only if
  the sum > 0, with `at` = `at` of the reported incident with the highest id. (Heat values are validated > 0, so
  "some `Some`" ⇔ "sum > 0".)
- `apply_report(wanted, heat, at)`: `heat = heat.saturating_add(h); last_known = Some(at); hidden = 0.0`.
- Incident `at`: `Punch/Wound/Kill` = shooter `Position` at the crime; `Shooting` = `muzzle`. Extending sets `at`/`last`.
- **Bound:** `Incident.attacks: Vec<(u32, f64)>` (attack id, time). `forget_crimes` drops incidents with
  `now - last > incident_memory_seconds` **and** prunes `attacks` entries with `now - t > incident_memory_seconds`
  from the rest. Max retained attacks per incident = fire rate × memory (SMG 0.08 s × 60 s = 750). A call about a
  pruned/forgotten attack or a forgotten Kill resolves to `[]` → no-op. A late call can never target an expired
  cause except as a no-op, because a call completes ≤ `slots/64 + call_seconds` ≈ 4.06 s after its stimulus, far below
  the 60 s memory.
- `forget` keeps an incident while `now - last <= memory` (boundary inclusive: worked example 5 in 5.2).

### 3.5 Search (GDD §6.4)

```rust
pub struct WantedLevel {
    pub heat: u32,
    pub stars: u8,                // stars_for(heat), recomputed every tick (a BRP heat mutation shows next tick)
    pub last_known: Option<Vec3>, // GDD `LastKnownPosition`, centre of the search circle; None while heat == 0
    pub seen: bool,               // a cop sees the player this tick
    pub hidden: f32,              // s the player has been continuously unseen AND outside the circle
}
```

`search_step(w, player, seen, dt, cfg)`, once per fixed tick while the player is alive:
1. `heat == 0` → `*w = default()`, return.
2. `w.stars = stars_for(heat)`; `row = cfg.stars[max(stars, 1) - 1]` (sub-threshold heat clears by the 1-star rule,
   resolved Q1).
3. `last_known.get_or_insert(player)` (a BRP heat mutation gets a centre).
4. `seen` → `last_known = Some(player)`, `hidden = 0`, `seen = true`.
5. else `seen = false`; if `flat_distance(player, last_known) > row.search_radius` → `hidden += dt`; if
   `hidden >= row.clear_seconds` → `*w = default()`.
6. else (unseen, inside) → `hidden = 0`.

Cop sight (fixture now, T11 cops later): any live entity with `Faction::Police` + `Position` + `Rotation`. Eyes =
`position − Y·float_height + Y·head_height` (perception convention, `LocomotionConfig`).
- *Sees the player* (search): 3D distance eyes→eyes ≤ `cop_view_distance` (35 m), flat (XZ) angle between
  `rotation * NEG_Z` and the flat line to the player ≤ `cop_view_cone_deg / 2` (zero-length flat vector counts as in
  view), and `!sight_blocked(cop_eye, player_eye)`.
- *Witnesses a crime*: distance ≤ `cop_witness_distance` (50 m) and `!sight_blocked`, no cone (GDD "коп видит лично
  (LOS ≤ 50 м)"), checked in `record_crimes` for each incident recorded or extended this tick that is still unreported,
  against the player's eyes.
- Cost: ≤ 12 cops × 1 ray per tick while heat > 0, plus a few rays per crime tick (avian ray ~0.6 µs, TASK-009) —
  synchronous, no async task, no slicing.

### 3.6 Schedule and lifecycle

- `WantedSystems` set: `configure_sets(FixedUpdate, WantedSystems.after(AiSystems::Decide).in_set(PlayingSystems))`;
  `add_systems(FixedUpdate, (crimes::record_crimes, crimes::take_calls, search::track_search, crimes::forget_crimes).chain().in_set(WantedSystems))`
  — a flat tuple (TASK-008 nested-chain lesson). After `Decide` = this tick's combat messages and completed calls are
  both visible; `PlayingSystems` = nothing while the player is dead.
- Messages: `PoliceCall` is a buffered `Message` (cross-system signal read later in the tick), not an observer
  `Event`. `MessageWriter<PoliceCall>` in `civilian_fsm`, `MessageReader<PoliceCall>` in `take_calls`.
- `OnEnter(GameState::Wasted)`: `reset_wanted` → `*wanted = default(); crimes.clear();`.
- `OnExit(GameState::Wasted)`: `drop_queued_calls` → `Messages<PoliceCall>::clear()`. Required by the `NpcSystems`
  doc contract (a Playing-only reader of an NPC message clears the backlog on exit) and the TASK-006/007 lesson.
  With incidents cleared it has no observable effect today (a stale cause cannot match a post-respawn incident:
  attack ids are monotonic, a dead body cannot be killed again), so it has **no separate gate**; the leak gate is
  test B11 (crimes.clear).
- Clocks: `Time<Fixed>` in sim (`timestep().as_secs_f32()` for `hidden`, like `civilian_fsm`; `elapsed_secs_f64()`
  for incident times). HUD blink: `Time<Real>` in `Update`.

### 3.7 HUD look (GDD §6.4: "когда ни один коп не видит игрока … звёзды мигают")

Row hidden while `stars == 0`. Slot `i < stars`: `seen` → Lit; unseen → blinks Gray/Off every `blink_seconds`
(`Time<Real>`). Slot `i >= stars` → Off. Before T11 (no cops) the owner sees blinking stars after a call until
they clear. The pulse on raise (GDD §8) is T13, not this slice.

---

## 4. Implementation steps

### Step 1 — `assets/wanted/wanted.ron` (new)

```ron
(
    // Heat of a witnessed crime by the player (GDD §6.4). Cop rows come with T11, car rows with T14/T15.
    heat: (punch_civilian: 5, shooting_near_people: 10, wound_civilian: 30, kill_person: 40),
    // Stars 1..5: heat threshold, search circle radius (m), seconds unseen outside the circle to clear.
    stars: (
        (heat: 40,   search_radius: 40.0,  clear_seconds: 10.0),
        (heat: 180,  search_radius: 70.0,  clear_seconds: 15.0),
        (heat: 550,  search_radius: 100.0, clear_seconds: 20.0),
        (heat: 1200, search_radius: 140.0, clear_seconds: 25.0),
        (heat: 2400, search_radius: 180.0, clear_seconds: 30.0),
    ),
    shooting_radius: 15.0,          // m: a person this close to the muzzle at the shot makes it a crime
    shooting_merge_seconds: 5.0,    // a shot this soon after the offender's last one joins that shooting incident
    incident_memory_seconds: 60.0,  // an incident untouched this long is forgotten; keep well above call_seconds
    cop_witness_distance: 50.0,     // m, LOS, any direction: a cop witnesses a crime (GDD §6.4)
    cop_view_distance: 35.0,        // m on foot: a cop sees the player (search)
    cop_view_cone_deg: 110.0,       // full cone of that view
)
```
Tuple syntax for `stars` is mandatory (probe). The two merge/memory knobs are resolved Q2: data, tuned after the
owner run.

### Step 2 — `crates/gta_sim/src/wanted/mod.rs` (rewrite, ~200 lines)

- `pub const WANTED_CONFIG: &str = "wanted/wanted.ron";`
- `pub const STARS: usize = 5;` — law (GDD §6.4 "у нас 5 звёзд"), one-line comment.
- `WantedConfig` (`Resource, Deserialize, Clone, Debug`, `#[serde(deny_unknown_fields)]`): `heat: HeatTable`,
  `stars: [StarRow; STARS]`, `shooting_radius`, `shooting_merge_seconds`, `incident_memory_seconds`,
  `cop_witness_distance`, `cop_view_distance`, `cop_view_cone_deg` (all `f32`).
  `HeatTable { punch_civilian, shooting_near_people, wound_civilian, kill_person: u32 }` with
  `pub(crate) fn of(&self, crime: Crime) -> u32`; `StarRow { heat: u32, search_radius: f32, clear_seconds: f32 }`;
  both `deny_unknown_fields`, `pub`.
- `WantedConfig::validate() -> Result<(), String>` (style of `PerceptionConfig::validate`): each `heat.*` > 0 (error
  names `heat.<field>`); `stars[i].heat` > 0 and strictly increasing (error names `stars[i].heat`);
  `stars[i].search_radius`, `stars[i].clear_seconds` finite > 0; `shooting_radius`, `cop_witness_distance`,
  `cop_view_distance` finite > 0; `shooting_merge_seconds` finite ≥ 0; `incident_memory_seconds` finite and
  > `shooting_merge_seconds`; `cop_view_cone_deg` in (0, 360).
- `WantedLevel` as in 3.5: `#[derive(Resource, Reflect, Default, Debug, Clone, Copy, PartialEq)]`,
  `#[reflect(Resource)]`, `///` per field.
- `pub fn stars_for(heat: u32, rows: &[StarRow; STARS]) -> u8` = count of rows with `heat >= row.heat`.
- `#[derive(SystemSet, Debug, Clone, PartialEq, Eq, Hash)] pub struct WantedSystems;`
- `WantedPlugin::build`: `init_resource::<WantedLevel>()`, `init_resource::<Crimes>()`,
  `register_type::<WantedLevel>()`, the set + systems of 3.6, `OnEnter(Wasted)` → `reset_wanted`,
  `OnExit(Wasted)` → `drop_queued_calls`.
- `mod crimes; mod search; pub use crimes::{Crime, Crimes, Incident};` Everything else (`IncidentId`, `classify`,
  `Victim`, `in_view`, `cop_sees`, `witnesses`, `search_step`, `record/report/resolve/forget`) stays `pub(crate)` or
  private; T11 widens what it needs.
- `#[cfg(test)] stars_table`: parse `include_str!("../../../../assets/wanted/wanted.ron")` (panic "GATE BROKEN" if it
  fails or the row heats are not 40/180/550/1200/2400); heat → stars: 0→0, 39→0, 40→1, 179→1, 180→2, 549→2, 550→3,
  1199→3, 1200→4, 2399→4, 2400→5, `u32::MAX`→5.

### Step 3 — `crates/gta_sim/src/wanted/crimes.rs` (new, ~350 lines incl. tests)

```rust
#[derive(Clone, Copy, Debug, PartialEq, Eq)] pub enum Crime { Punch, Shooting, Wound, Kill }
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)] pub(crate) struct IncidentId(u32);
#[derive(Clone, Debug)]
pub struct Incident { pub(crate) id: IncidentId, pub crime: Crime, pub offender: Entity, pub victim: Option<Entity>,
                      pub(crate) attacks: Vec<(u32, f64)>, pub at: Vec3, pub last: f64, pub reported: bool }
#[derive(Resource, Default)] pub struct Crimes { next: u32, incidents: Vec<Incident> }
```
- `pub fn incidents(&self) -> &[Incident]` (read API for tests/T11); `pub(crate) fn clear(&mut self)`.
- `record(crime, offender, victim, attack, at, now, merge) -> IncidentId`: extends the matching incident, else pushes.
  Match = same `offender` and `crime`, and (`Shooting`: `now - last <= merge`) or (else: same `victim`). Extending
  pushes `(attack, now)` unless that attack id is already present, sets `at` and `last`. Matching ignores `reported`
  (a reported shooting episode extended by more shots stays reported: resolved Q2).
- `report(id, &HeatTable) -> Option<(u32, Vec3)>`, `resolve(Cause) -> Vec<IncidentId>` (3.2),
  `forget(now, memory)` (3.4).
- `enum Victim { Civilian, Gang, Other }`; `classify(victim, melee: bool, killed: bool) -> Option<Crime>`:
  Civilian → killed ? Kill : melee ? Punch : Wound; Gang → killed ? Kill : None; Other → None.
- `record_crimes` params: `Res<WantedConfig>`, `Res<LocomotionConfig>`, `Res<Time<Fixed>>`, `SpatialQuery`,
  `ResMut<Crimes>`, `ResMut<WantedLevel>`, `MessageReader<ShotFired>`, `MessageReader<MeleeHit>`,
  `MessageReader<DamageDealt>`, `Query<(), With<Player>>`, `Query<&Position>`,
  `Query<(Has<Civilian>, Has<GangMember>)>`, persons `Query<(Entity, &Position, &Health), Or<(With<Civilian>, With<GangMember>)>>`,
  cops `Query<(&Position, &Faction), Without<Dead>>`. Body, in order:
  1. Collect this tick's messages into locals: `melee: HashSet<u32>` (MeleeHit attacks), `dealt: Vec<DamageDealt>`,
     `killed: HashSet<(u32, Entity)>` from `dealt` with `killed`, player `shots: Vec<ShotFired>`.
  2. For each player `DamageDealt` (shooter is `Player`): skip a non-lethal one whose `(shot, target)` is in `killed`
     (same-attack Wound+Kill collapse, 3.1); `classify(victim_kind(target), melee.contains(&shot), killed)` →
     `record(.., at = shooter Position)`; remember the id in `touched`.
  3. For each player `ShotFired`: near = any person within `shooting_radius` of `muzzle` with
     `health.current > 0 || killed.contains(&(shot.attack, person))` (3.3) → `record(Shooting, .., at = muzzle)`,
     add to `touched`.
  4. For each id in `touched` still unreported: if any cop (`Faction::Police`) `witnesses` the player's eyes →
     `report` → `apply_report`.
  One-line comment on the at-shot rule and on the collapse.
- `take_calls`: `MessageReader<PoliceCall>` → 3.4.
- `forget_crimes`: `crimes.forget(time.elapsed_secs_f64(), cfg.incident_memory_seconds)`.
- `reset_wanted`, `drop_queued_calls`, `apply_report` live here or in `mod.rs` (implementer's choice, one place).
- `#[cfg(test)]` pure unit tests (literal `HeatTable {5, 10, 30, 40}`, hand-picked entities via
  `Entity::from_raw_u32`/`World::spawn_empty` — confirm the constructor in `bevy_ecs-0.19.1/src/entity/mod.rs`):
  1. `classify_table`: 9 rows (Civilian/Gang/Other × melee/gun/killed) → Civilian: (melee,¬k) Punch, (gun,¬k) Wound,
     killed Kill; Gang: killed Kill, else None; Other: None.
  2. `one_body_one_contribution`: `record(Kill, p, Some(v), 7, ..)`; `resolve(Body(v))` → `[id]`; `report` →
     `Some((40, at))`; second resolve+report → `None`; `resolve(Attack(7))` → same id → `None`.
  3. `body_resolves_only_the_kill`: `record(Punch, p, Some(v), 3, ..)` and `record(Wound, p, Some(v), 4, ..)`, no
     Kill → `resolve(Body(v))` == `[]`; add `record(Kill, p, Some(v), 5, ..)` → `resolve(Body(v))` == `[kill id]` only.
  4. `shooting_episode_merges`: attacks 1 @ t=0, 2 @ t=1.0 (merge 5) → one incident, attacks ids `[1, 2]`; attack 3 @
     t=7.0 (6.0 > 5 after 1.0) → second incident; report via `Attack(1)` → 10, `Attack(2)` → `None`, `Attack(3)` → 10.
  5. `forget_after_memory`: incident at t=0; `forget(60.0, 60.0)` keeps it; `forget(60.5, 60.0)` drops it;
     `resolve(Body/Attack)` → `[]`.
  6. `attack_list_is_bounded`: one Shooting episode with attacks id k at t = k for k = 0..=100 (merge 5) →
     `forget(100.0, 60.0)` → incident kept, `attacks.len() == 61` (t ∈ [40, 100]); `resolve(Attack(10))` → `[]`,
     `resolve(Attack(50))` → `[id]`. Flip: remove the `attacks.retain` → len 101 → RED.
  7. `victim_crimes_dedupe_per_victim`: two `Wound` on v (attacks 4, 5) → one incident; `Wound` on w → another;
     re-recording attack 4 on v does not duplicate the pair.

### Step 4 — `crates/gta_sim/src/wanted/search.rs` (new, ~220 lines incl. tests)

- `pub(crate) fn in_view(eye, forward, target, cone_deg, distance) -> bool` (3.5).
- `pub(crate) fn cop_sees(spatial, cop_eye, cop_forward, player_eye, cfg) -> bool` =
  `in_view(.., cfg.cop_view_cone_deg, cfg.cop_view_distance) && !sight_blocked(spatial, cop_eye, player_eye)`.
- `pub(crate) fn witnesses(spatial, cop_eye, offender_eye, cfg) -> bool` = distance ≤ `cop_witness_distance` && LOS.
- `pub(crate) fn search_step(..)` = 3.5 (uses `navigation::flat_distance`).
- `track_search`: player `Query<&Position, (With<Player>, Without<Dead>)>` (let-else return); cops
  `Query<(&Position, &Rotation, &Faction), Without<Dead>>` filtered to `Faction::Police`; the cop check runs only
  while `heat > 0`. Compute into a local copy, then `wanted.set_if_neq(new)` (`DetectChangesMut::set_if_neq`,
  `bevy_ecs-0.19.1/src/change_detection/traits.rs:215`) so `Changed` stays meaningful.
- `#[cfg(test)]`, shipped cone 110° / 35 m asserted (`GATE BROKEN` otherwise); forward = `Quat::from_rotation_y(yaw) * NEG_Z`,
  eye at origin:
  - yaw 0 (forward −Z): (0,0,−10) ✓; (−5,0,−10) (26.6°) ✓; (10,0,0) (90°) ✗; (0,0,10) ✗; off-grid edges
    54° (8.090,0,−5.878) ✓, 56° (8.290,0,−5.592) ✗; (0,0,−34.9) ✓; (0,0,−35.1) ✗; target (0,5,0) (zero flat vector,
    5 m) ✓.
  - yaw +90° (forward −X, verified: rotation about +Y maps (0,0,−1) → (−1,0,0)): (−10,0,0) ✓; (0,0,−10) ✗.
  - yaw 180° (forward +Z): (0,0,10) ✓; (0,0,−10) ✗.
  - `search_step` table, dt = 1/64, row 1 (40 m, 10 s), heat 40, `last_known` (0,0,0): inside (20,0,0) → hidden 0;
    outside (30,0,30) (42.43 m) 639 steps → `hidden == 639.0/64.0` exactly, stars 1; step 640 → default. `seen` at
    step 320 → `hidden == 0`, `last_known == player`. heat 0 → default. heat 40, `last_known None` → centre = player
    on the first step. heat 10 (sub-threshold) → row 1 rule applies (clears at step 640 outside), stars 0.

### Step 5 — `crates/gta_sim/src/perception/mod.rs`

- `/// What produced a threat: an attack id (`AttackSerial`) or a corpse.`
  `#[derive(Reflect, Clone, Copy, Debug, PartialEq, Eq)] pub enum Cause { Attack(u32), Body(Entity) }`.
- `Threat` gains `pub cause: Option<Cause>`.
- `StimulusLog(pub Vec<(u64, ThreatKind, Vec3, u32)>)`, doc `(tick, kind, point, attack)`; retain pattern
  `|&(then, ..)|`. `collect_stimuli`: Gunshot uses `shot.attack`, Fight `hit.attack`; the Hurt threat gets
  `cause: Some(Cause::Attack(hit.shot))`.
- `perceive`: `offer(kind, at, distance, cause)`; log entries → `Some(Attack(attack))`; corpse query
  `Query<(Entity, &Position), With<Corpse>>` → `Some(Body(e))`; Aimed → `None`.
- `PerceptionPlugin`: `register_type::<Cause>()`.

### Step 6 — `crates/gta_sim/src/combat/hitscan.rs`, `combat/melee.rs`

- `ShotFired` gains `/// Attack id (`AttackSerial`), equal to `DamageDealt.shot` of its pellets.` `pub attack: u32`,
  set from `shot` at `hitscan.rs:221`.
- `MeleeHit` gains `pub attack: u32` (same doc), set from `strike.attack` at `melee.rs:536`.

### Step 7 — `crates/gta_sim/src/civilian/mod.rs`

- `Report { progress: f32, about: Option<Cause> }`, doc: "`about` names the crime being phoned in".
- `react`: `Reaction::Report => CivilianState::Report { progress: 0.0, about: threat.cause }`.
- `next_state`: `(Report { progress, about }, None)` keeps `about` while progressing.
- `/// A completed witness call (GDD §6.2); `wanted` turns it into heat.`
  `#[derive(Message, Reflect, Clone, Copy, Debug)] #[reflect(Message)] pub struct PoliceCall { pub caller: Entity, pub about: Cause }`.
- `CivilianPlugin`: `add_message::<PoliceCall>()`, `register_type::<PoliceCall>()`.
- `civilian_fsm`: query gains `Entity`; new param `mut calls: MessageWriter<PoliceCall>`. Right after `next_state`
  and **before** `arrive`:
  `if let (CivilianState::Report { about: Some(about), .. }, CivilianState::Wander) = (civilian.state, state) { calls.write(PoliceCall { caller: entity, about }); }`
  (Report → Wander only by completion; interruption goes through `react(.., false)` which never returns Wander;
  `Dead` `continue`s earlier.)
- Blast radius check done: `civilian_fsm` is registered only by `CivilianPlugin`; no test harness registers it by
  hand. `PoliceCall` is new, so no other reader exists.

### Step 8 — `crates/gta_sim/src/lib.rs`

Load `WantedConfig` via `load_config::<WantedConfig>(&root, WANTED_CONFIG)?` + `validate()` mapped to
`ConfigError { path: root.path(WANTED_CONFIG), message }` like the others; `insert_resource(wanted)` with the rest.
`WantedPlugin` stays last.

### Step 9 — mechanical updates of existing code/tests

- `tests/civilians.rs`: add local `fn call_progress(state: CivilianState) -> Option<f32>` (`Report { progress, .. }`);
  exact asserts at `:344`, `:351`, `:401`, `:426` compare `call_progress(..)` with `Some(expected)`. `Threat` literal
  at `:381` gains `cause: None`.
- `civilian/reaction.rs:73` test literal: `cause: None`.
- `tests/civilian_bench.rs:107` and `src/visuals/character_gate.rs:488`: `ShotFired { .., attack: 0 }` (0 is never
  issued by `AttackSerial`).
- `tests/respawn.rs:97`: `stars = 3` would be overwritten by `stars_for(0)` next tick and prove nothing; set
  `heat = cfg.stars[2].heat` (550, read from `WantedConfig`) instead; `:107` asserts `heat == 0 && stars == 0` after
  death.
- `src/hud/witness.rs:41` → `Report { progress, .. }`; module doc line 2 likewise. `src/hud/witness_gate.rs:79,85,98`
  literals gain `about: None`.

### Step 10 — config gates in `crates/gta_sim/tests/config.rs`

- `shipped_wanted_config_loads_and_validates`.
- `unknown_wanted_field_names_file_and_field`: existing pattern, `shooting_radius: 15.0,` →
  `shooting_radius: 15.0, bogus: 1,`; error names `wanted.ron` and `bogus`.
- `star_thresholds_must_increase` via `sabotaged`: `(heat: 180,` → `(heat: 30,` (strictly below 40, off the
  boundary); error contains `stars[1].heat`.
- `five_star_rows_required`: remove the 5th row line → load error contains `length 5` (parse error, distinct from
  the validate error above).
- Each flipped per the gates domain: the sabotage input is the flip; restore = shipped file GREEN.

### Step 11 — headless gates, `crates/gta_sim/tests/wanted.rs` and `crates/gta_sim/tests/wanted_search.rs` (new)

Split in two files to stay < 750 lines each. Production composition (`headless_app()`), test floor,
`FixedTimesteps(1)`. Local helpers (modelled on `civilians.rs`, not moved there — surgical):
- `graph_app(h, extra_nodes/edges)` = `headless_app` + `test_graph` of the square of half-size `h` (nodes
  (−h,−h),(h,−h),(h,h),(−h,h), edges 0-1-2-3-0) plus the given extra segments; `settle`.
- `arm(app, weapon)` = full magazine via `set_loadout` (as `armed_app`).
- `SIDES` as in `civilians.rs`; `hold_idle(e)` = `Idle { left: 1e6 }`.
- `kill_with_one_shot(app, target)` = named test mutation `set_health_of(target, current = 1.0)`, `set_aim` from the
  player body at the target chest, `fire_requested`, `Shots::run` up to 4 ticks; assert a `killed` entry for
  `target` in `dealt_log` (`GATE BROKEN` otherwise); returns the attack id.
- `Calls` = `MessageCursor<PoliceCall>` reader like `Shots` (collects after every tick).
- `wanted(app) -> WantedLevel`, `incidents(app) -> Vec<Incident>` (via `Crimes::incidents`).
- `spawn_cop(app, chest, yaw)` = `(Name::new("Cop fixture"), Faction::Police, Position(chest), Rotation(Quat::from_rotation_y(yaw)))`
  (no collider, no body: blocks no ray; avian's `Position` has no required components — `avian3d-0.7.0/src/physics_transform/transform.rs:44-48`).
  `chest = feet + Y·float_height`.
- Every expected number comes from the shipped configs read at test start, with `GATE BROKEN` asserts on the values
  the worked example assumes: heats 5/10/30/40, `call_seconds × 64 = 256` integral, `clear_seconds × 64 = 640`
  integral, radii 15/40/50/35, cone 110, `hearing_radius` 40, `fight_hearing_radius` 15, `corpse_sight` 20,
  `report_min_distance` 25.

**Setup W** (report-prone corpse witness, from `civilians.rs:367-409`): `graph_app(10)`, pistol; witness on
`SIDES[0]` t=0.5 → feet (0,0,−10), temperament (0.6, 1.0, 1.4); victim on `SIDES[0]` t=0.8 → (6,0,−10);
both `hold_idle`; 16 ticks. Kill the victim with one player shot. Witness: corpse 6.0 m vs gunshot ≈ 9.5 m (< 25,
not reportable) → nearest = corpse: flee 0.6, cower 1.5·1.0·(1−6/15) = 0.9, report 0.8·1.4 = 1.12 → **Report**,
`about = Some(Body(victim))` within ≤ `slots` (4) ticks (assert, `GATE BROKEN`). Call completes after 256 ticks.
Incidents after the shot: `Kill(victim)` and `Shooting` (the witness at ≈ 9.5 m and the victim at ≈ 11.2 m are
within 15 m).

`tests/wanted.rs` (witness / incident gates):

- **A1 `unwitnessed_kill_is_zero_heat`**: `graph_app(10)` + far segment (−34,0,−30)–(−26,0,−30), pistol; victim
  (6,0,−10) `hold_idle`; report-prone civilian (0.7,1.0,1.5) on the far segment t=0.5 → (−30,0,−30): 42.4 m from the
  muzzle (> 40), 41.2 m from the body (> 20) — assert both. Kill the victim. Liveness: `incidents` holds exactly a
  `Kill{victim}` and a `Shooting`, offender = player, both `reported == false`. Run 2·256+16 ticks: heat 0, stars 0,
  zero `PoliceCall`, far civilian still calm. Flip: `record_crimes` reports every new incident as if witnessed → RED.
- **A2 `repeat_calls_about_one_corpse_count_once`** (binding note): setup W. First call → `PoliceCall` count 1,
  `heat == 40`, `stars == 1`, `last_known` within 0.1 m of the player position at the shot. `hold_idle` the witness
  again (named mutation, keeps it facing the body); it calls again → count 2, heat still 40, `last_known` unchanged.
  Liveness = count ≥ 2. Flip: `report` returns heat even when already reported → 80 → RED.
- **A3 `attack_call_then_body_call_counts_each_incident_once`** (V2 gate): `graph_app(10)` + segment
  (−5,0,30)–(5,0,30); setup W witness B and victim as above; caller A on the new segment t=0.5 → (0,0,30),
  temperament (0.7,1.0,1.5), `hold_idle`. Assert: A–muzzle ∈ [25, 40] (≈ 30.4 m; hearing has no LOS, the wall at z=14
  is irrelevant), A–body 40.4 m > 20. A reacts to the gunshot → Report `Attack(a)`; B to the body → Report `Body(v)`.
  After both calls (count 2, one `Attack(a)` and one `Body(victim)`): `heat == 40 + 10 = 50` in either completion
  order (A first: +50 then +0; B first: +40 then +10). Flip: `report` ignores `reported` → 90 → RED.
- **A4 `body_call_does_not_report_a_private_punch`** (V2 attribution counterexample): `graph_app(10)`, player
  **unarmed**; victim V on `SIDES[0]` t=0.5 → (0,0,−10), calm, `hold_idle`; witness W2 on `SIDES[1]` t=0.9 → (10,0,6),
  temperament (0.6,1.0,1.4), `hold_idle`. `place_player` feet (0,0,−9), aim at V's chest, click once; run ≤ 24 ticks
  until one player `MeleeHit` on V (liveness; fists do 10, V stays alive). Assert W2–hit point > 15 (≈ 18.4 m) and
  W2–V ∈ (15, 20] (18.87 m) — W2 neither hears the fight nor misses the body. Liveness: one `Punch{V}` incident,
  unreported. Named mutation: `set_health_of(V, current = 0.0)` — an unattributed death standing in for a gang kill.
  W2 sees the corpse (report 1.12 > flee 0.6 > cower 0) → Report `Body(V)` → completes (count 1) → heat 0, Punch still
  unreported. Flip: `resolve(Body(v))` returns every incident with `victim == v` → heat 5 → RED.
- **A5 `killing_the_caller_interrupts_the_call`**: setup W. Once the witness is in `Report`, run 24 ticks (still
  `Report`), kill the witness with one player shot. Run 256+16 ticks: zero `PoliceCall`, heat 0. (A2 is the positive
  twin.) Flip: write `PoliceCall` on entering Report instead of on completion → RED.
- **A6 `shot_near_people_is_reported_by_a_hearing_caller`**: `graph_app(30)` + segment (−5,0,8)–(5,0,8); bystander at
  (0,0,8) `hold_idle` (≈ 8.5 m from the upward muzzle, ≤ 15); witness `SIDES[0]` t=0.5 → (0,0,−30), (0.7,1.0,1.5)
  (row 4 of `reaction.rs`); `shoot_into_the_air` once. After the call: heat 10, stars 0, `last_known ≈ muzzle`.
  Continue: `place_player` feet (30,0,30) (≈ 42.4 m from the muzzle > 40) → after exactly 640 ticks heat 0 (resolved
  Q1: sub-threshold heat clears by the 1-star rule; tick 639 still heat 10). Control **`lone_shot_is_no_crime`**:
  same without the bystander → the call still completes (count 1), heat 0, no incident.
- **A7 `cop_fixture_witnesses_at_the_shot`** (cop path + at-shot rule, fresh app per case): `graph_app(10)`, pistol,
  civilian victim (6,0,−10) `hold_idle`, cop chest at (−20, fh, 0) (20 m, clear line).
  - lethal (health 1): **in the shot tick** heat == 40 + 10 = 50, stars 1; incidents Kill + Shooting reported.
  - non-lethal (full health; pistol 25 ± 10 % < 100): heat == 30 + 10 = 40 (Wound + Shooting).
  - already dead (named mutation `current = 0` 8 ticks before, corpse present), `shoot_into_the_air`: heat 0, no
    incident (the corpse is not "people").
  - blocked: cop at (0, fh, 20) (wall at z≈14 blocks the eyes line) → lethal shot → heat 0, Kill unreported.
  - far: cop at (−36, fh, −36) (50.9 m > 50) → heat 0.
  Flip: near-check back to `health.current > 0` only → lethal case 40 → RED.
- **A8 `one_shotgun_blast_is_one_kill`**: `graph_app(10)`, shotgun, cop (−20, fh, 0), civilian at (0,0,−3)
  `hold_idle`, named mutation health 10. Fire once at the chest. `GATE BROKEN` precondition: `dealt_log` for this
  attack on the target has ≥ 1 non-lethal entry followed by exactly one `killed` (pellet 8 ± 0.8 → 1st pellet
  alive, 2nd kills; if the seeded roll does not give that shape, change the health mutation, never the assertion).
  Expected: incidents Kill + Shooting only, heat 50. Flip: remove the `(shot, target)` collapse → Wound recorded,
  heat 80 → RED.
- **A9 `gang_victims_follow_the_table`**: `graph_app(10)`, pistol, cop (−20, fh, 0),
  `spawn_member(gang 0, (6,0,−10), Pistol)` (no territories → `GangSystems` off, member idle). One body shot at full
  health → heat 10 (Shooting; gang wound not a crime). Then health 1 + one shot → Kill → heat 10 + 40 = 50 (the
  second shot extends the reported shooting episode: +0).
- **A10 `gang_crimes_are_not_the_players`**: setup W geometry, but a gang member at (2,0,−2) kills the victim with its
  own pistol: set its `AimIntent` (origin chest, direction at the victim chest, `aiming = true`) and
  `ActionIntent.fire_requested`; victim health 1. Witness phones `Body(victim)` (count 1) → heat 0, `incidents`
  empty. Named fallback if driving the member's gun is flaky: a hand-written `DamageDealt { shooter: member, killed: true, .. }`
  plus `current = 0`; record it as weaker (bypasses `fire_weapons`).

`tests/wanted_search.rs` (search / lifecycle gates):

- **B8 `stars_follow_a_heat_mutation`** (liveness of the recompute, not BRP reachability — BRP is proven in t10.py):
  `WantedLevel.heat = 550` → after 1 tick `stars == 3`, `last_known == Some(player position)`.
- **B9 `hiding_outside_the_circle_clears`** (AC "вне круга и без видимости N с → 0"): heat 40 at the origin, 1 tick
  → `last_known ≈` origin. `place_player` feet (30,0,30) (42.43 m > 40). Each tick assert the measured `Position` is
  still > 40 m flat from `last_known` (physics drift guard), and after tick k `hidden == k/64` exactly. k = 639 →
  stars 1; k = 640 → heat 0, stars 0, `last_known == None`. Control: feet (20,0,0) for 1280 ticks → stars 1, hidden 0.
  Variant: 320 ticks out, 1 tick at (20,0,0), 320 ticks out → still stars 1 (hidden restarted). Flips: count
  `hidden` inside the circle → control RED; drop the re-entry reset → variant RED.
- **B10 `spotted_resets_timer_and_moves_circle`** (AC "замечен → таймер сброшен"): from B9 at k = 320 (hidden 5.0),
  `spawn_cop((30, fh, 10), yaw = π)` (forward +Z, player 20 m ahead, no fixture near x = 30). Next tick: `seen`,
  hidden 0, `last_known ≈` player. Despawn the cop; 1280 ticks at (30,30) → stars 1 (the old circle would have
  cleared after 320). `place_player` feet (−30,0,−30) (84.9 m from the new centre) → clears at exactly k = 640.
  Negative: cop at (30, fh, 10) yaw 0 (facing away) → hidden keeps growing. LOS through the production system (no
  `run_system_once`, keeps `cop_sees` crate-private; fresh app, `heat = 40` so the cop check runs): player at (0, fh, 8), cop at (0, fh, 20) yaw 0 (facing −Z,
  12 m, in cone) → `seen == false` (wall); cop at (−15, fh, 20) with `yaw = aim_yaw((15,0,−12))` → the line crosses
  z = 14 at x = −7.5, outside the wall → `seen == true`.
- **B11 `wasted_forgets_the_crimes`** (leak class, TASK-006/007 lesson): setup W; once the witness is in
  `Report { about: Some(Body(victim)) }` (liveness of the cause), `write_damage(1000)` → update until `Wasted`
  (the loop from `npcs_live_through_wasted`) → update until `Playing` (bounded, `GATE BROKEN`). The natural call may
  finish inside Wasted, where its message is dropped (Playing-only reader + `drop_queued_calls`), which would make
  the flip vacuous; so after respawn apply the named mutation `Report { progress: 0.0, about: Some(Body(victim)) }` on
  the witness (a pre-death call completing in Playing). Run 256+16 ticks: ≥ 1 `PoliceCall` about `Body(victim)`
  observed (liveness), heat 0, `incidents` empty. Flip: remove `crimes.clear()` from `reset_wanted` → heat 40 → RED.

Gate classes: correctness — A2, A3, A4, A7, A8, B9, B10, B11, config gates, unit tests of step 3/4; "no witness /
interrupted / not a crime" — A1, A5, A6 (+control); gang rules — A9, A10; liveness — B8. Record every flip
(perturbed input, RED, GREEN) in the implementer summary.

### Step 12 — client HUD

- `src/menu/config.rs`: `HudLayout` gains `pub stars: StarsConfig` (`Deserialize, Clone, Debug`,
  `deny_unknown_fields`): `glyph: String`, `size: f32`, `gap: f32`, `lit_color: Rgb`, `gray_color: Rgb`,
  `off_color: Rgba`, `blink_seconds: f32`, `///` docs. `UiConfig::validate`: glyph non-empty (`hud.stars.glyph`),
  `positive` for size and blink_seconds, `gap` finite ≥ 0, `unit` for the three colours.
- `assets/ui/strings.ron` `hud`: `stars: (glyph: "★", size: 26.0, gap: 2.0, lit_color: (1.0, 1.0, 1.0),
  gray_color: (0.55, 0.55, 0.55), off_color: (0.0, 0.0, 0.0, 0.45), blink_seconds: 0.25)` with a one-line comment
  "wanted stars: white seen, blinking grey unseen (GDD §6.4)".
- `src/hud/stars.rs` (new, ~110 lines): `#[derive(Component)] struct StarRow;`, `#[derive(Component)] struct StarSlot(u8);`
  `spawn_stars(commands, ui: Res<UiConfig>, fonts: Res<UiFonts>)` on the same `Loading → Playing` transition as
  `spawn_hud`: absolute row, `right: margin`, `top: margin + 2*(bar_height + bar_gap) + ammo_size + bar_gap`,
  `column_gap: gap`, `Visibility::Hidden`, 5 `Text(glyph)` children with `TextFont` from `fonts.regular` at `size`
  (same `TextFont` construction as `weapon.rs:43-47`). `pub(super) enum StarLook { Lit, Gray, Off }` and pure
  `pub(super) fn star_look(slot: u8, w: &WantedLevel, real_secs: f32, blink: f32) -> StarLook`: `slot >= stars` →
  Off; `seen` → Lit; else Gray when `floor(real_secs / blink)` is even, Off when odd. `update_stars` (`Update`):
  row `Visibility` Hidden when `stars == 0` else Inherited; per-slot `TextColor` updated only when it differs.
- `src/hud/mod.rs`: `mod stars;`, add `stars::spawn_stars` to the transition tuple and `stars::update_stars` to
  `Update`; plugin doc line mentions the stars.
- `#[cfg(test)]` in `stars.rs`: `star_look_table` (blink 0.25): stars 2 seen → slots 0,1 Lit, 2..4 Off; stars 2
  unseen at t = 0.1 → slots 0,1 Gray, at t = 0.3 → Off; stars 0 → all Off. Colour, size, position, blink feel are
  the owner's.

### Step 13 — runtime QA `tools/qa/scenarios/t10.py` (new, ~260 lines, t8/t9 style)

Via `brp.py`, `--seed 1`, release, features `dev`, `--out <dir>`:
1. Wait for `Playing`, golden `CityLayoutHash`, chunks (`t5.wait_chunks`), bubble ≥ 20 civilians. Pick up the pistol
   (t8 step 3 pattern).
2. Choose a victim civilian with ≥ 3 other civilians within 40 m. **Named QA mutations** (t9 armour precedent):
   victim `Civilian.state` → `{"Idle": {"left": 1e6}}`, `Health.current` → 1. One civilian at 25-38 m from the planned
   muzzle gets `Idle` + temperament `{flee 0.5, cower 1.0, report 1.5}` (guaranteed caller; the temperament roll is
   not the property under test). Teleport the player 5 m from the victim, `t6.aim_at` its chest, click once. Assert
   the victim is dead (the shot really fired).
3. Poll `WantedLevel` and the caller's `Civilian.state` up to `call_seconds + 3` s: the caller entered `Report` and
   left it, then `heat >= kill_person` and `stars >= 1`. Summary keeps heat, stars, `last_known`. Screenshot
   `hud_wanted.png`.
4. Teleport to whichever of `HospitalSpawn.point`, `CityLandmarks.plaza_center`, `park_center` is farthest from
   `last_known`; assert that distance > `search_radius[stars-1] + 10` (`GATE BROKEN` otherwise). After ~1 s screenshot
   `hud_blinking.png`, then a second one ≥ 0.15 s later (blink phase); assert `hidden > 0`, `seen == false`, and the
   measured player position still outside the radius.
5. Poll: `stars >= 1` at `clear_seconds − 1` s and `stars == 0`, `heat == 0` by `clear_seconds + 3` s (loose wall-clock
   bounds on purpose; BRP latency is not the gate). Screenshot `hud_clear.png`.
6. BRP reachability (separate assertion): `world.mutate_resources` (`bevy_remote-0.19.1` `BRP_MUTATE_RESOURCE_METHOD`,
   params `resource`, `path` ".heat", `value`) heat → 180 → poll `stars == 2`; then heat → 0 → poll `stars == 0`.
7. `log_errors` empty, `shutdown`. Every constant is read from `wanted.ron` / `civilian.ron` with the t8 `ron_number`
   regex helper.

Owner checklist for QA_REPORT.md (Russian, QA stage writes it): "стреляет при свидетелях → над свидетелем полоса
звонка → звёзды (мигают, копов ещё нет); убегает за круг → через 10 с при 1 звезде розыск пропадает; убийство
свидетеля до конца звонка → звёзд нет; стрельба в воздух без людей рядом → звёзд нет; одиночный выстрел рядом с
человеком (10 heat) звёзд не даёт и сгорает по правилу 1-й звезды".

### Step 14 — verification

`cargo build`, `cargo build -p gta_like --features dev`, `cargo clippy --all-targets -- -D warnings`,
`cargo clippy -p gta_sim --all-targets -- -D warnings`, `cargo test -p gta_sim`, `cargo test -p gta_like --bin gta_like`
(the touched `witness_gate` and the new `star_look_table`; run the client tests 3 times per the TASK-022 lesson),
`python tools/qa/scenarios/t10.py --out <dir>`. `citygen` not touched → its tests not required. Before trusting a red
in untouched code, `touch crates/*/src/lib.rs` and rebuild (TASK-009 phantom red).

---

## 5. Test plan (summary)

| What | How | Expected |
|---|---|---|
| Config strictness | `tests/config.rs` 4 gates (Step 10) | shipped loads; unknown field names file+field; `stars[1].heat`; `length 5` |
| Star thresholds | unit `stars_table` | 0/39→0 … 2399→4, 2400/`u32::MAX`→5 |
| Incident logic | unit tests step 3 (7) | one report per incident; Body → Kill only; merge 5 s; forget 60 s; attack list bounded (61) |
| View math | unit tests step 4 (3 yaw cases + edges + zero flat) | as listed |
| Search timer | unit `search_step` + B9/B10 | clear at exactly tick 640; re-entry and sight reset |
| AC "без свидетеля = 0" | A1 | heat 0, incidents unreported |
| AC "звонок прерван убийством" | A5 | 0 calls, heat 0 |
| AC "пороги" | `stars_table`, B8, A7/A9 | stars per table |
| AC "вне круга N с → 0" | B9, A6 continuation | tick 640 |
| AC "замечен → таймер сброшен" | B10 | hidden 0, circle moved, LOS respected |
| Binding note (one per corpse) | A2, A3 | 40; 50 |
| Attribution | A4, A9, A10 | 0; 50; 0 |
| At-shot proximity, pellet collapse | A7, A8 | 50/40/0; 50 |
| Wasted leak | B11 | heat 0, incidents empty |
| HUD look | `star_look_table` + owner run | table; feel by owner |
| Runtime | `t10.py` | call → heat ≥ 40, stars ≥ 1 → hide → 0; BRP mutation 180 → 2 stars |

---

## 6. Rollout notes

- New data file `assets/wanted/wanted.ron` (tracked, not under `third_party`); new `hud.stars` block in
  `assets/ui/strings.ron` — both strict (`deny_unknown_fields`), so a missing block fails at startup with a named
  error, by design.
- No migrations, no save data, no env vars, no feature flags, no new crate; `gta_sim` gains no render dependency
  (`cargo tree -p gta_sim -e normal -i bevy_render` stays empty).
- Message/API changes: `ShotFired.attack`, `MeleeHit.attack`, `Threat.cause`, `CivilianState::Report.about`,
  `StimulusLog` 4-tuple, new `PoliceCall`. BRP JSON of `CivilianState::Report` gains `about`; no existing QA script
  writes `Report`.
- `WantedLevel` changes from `{ stars }` to `{ heat, stars, last_known, seen, hidden }`; `stars` is derived, so BRP /
  T11 QA mutate `heat`, never `stars`.
- T11 hand-off: `WantedLevel.{stars, last_known, seen}` drives the dispatcher; cops are `Faction::Police` characters
  and work with witnessing/search without wanted-side changes; T11 adds cop crimes ("always") to `Crime`/`HeatTable`,
  "вырваться = +1 звезда" as `heat = max(heat, next row threshold)`, `Busted` repeats the `Wasted` reset + call clear,
  and widens `cop_sees` visibility if its FSM needs it.
- After the task closes (orchestrator): update `docs/narrative-graph.md`, README "Статус", AGENTS.md stage line.

---

## 7. Review notes (what changed from PLAN.md / PLAN_V2.md and why)

Disconfirmation tested first: *"V2's at-shot proximity rule is correct but its worked numbers were never
recomputed; some gate in the plan still expects the old value."* It **held**: `fire_weapons` writes `ShotFired`
before damage (`hitscan.rs:221` vs the pellet loop), so under V2's rule the victim killed by the shot counts as
near, and PLAN test 5 (cop fixture, lethal shot, victim ≈ 11.2 m from the muzzle, no one else near) becomes
Kill 40 + Shooting 10 = **50**, not 40; PLAN test 1's liveness ("exactly one Kill incident") becomes Kill +
Shooting. Both fixed (A7, A1).

Kept from V2 (verified): `Body(v)` → only player `Kill(v)`; at-shot proximity; bounded attack retention; clearing
`Messages<PoliceCall>` on `OnExit(Wasted)` (also demanded by the `NpcSystems` doc comment in `flow/mod.rs`); resolved
questions treated as decisions (no owner questions section); narrow public API; attack-then-body gate; drift guard in
teleport tests; B8 labelled liveness, BRP reachability moved to t10.py step 6.

Changed or added against both plans:
1. **Shotgun same-attack Wound + Kill** (new finding): `weapons.ron` shotgun has 10 pellets; pellets are applied in
   order and each non-lethal one writes its own `DamageDealt` before the lethal one. Both plans would record Wound 30 +
   Kill 40 for one trigger pull. Added the `(shot, target)` collapse and gate A8.
2. **HUD blink rule** aligned with GDD §6.4 ("когда ни один коп не видит … звёзды мигают"): PLAN/V2 had steady grey
   inside the circle (GTA IV reading), which contradicts the scope law; the look table changed accordingly.
3. **B11 Wasted gate made non-vacuous**: `advance_wasted` runs on `Time<Real>`, so how many fixed ticks fit in
   Wasted is wall-clock dependent; a call finishing inside Wasted is dropped and the `crimes.clear` flip would stay
   GREEN. The stale call is now forced to complete in Playing by a named mutation.
4. **A4 private-punch gate** designed concretely (V2 named it without geometry): unarmed punch, unattributed death as
   the stand-in for a gang kill (the Body path is what is under test), W2 at 18.87 m (strictly inside (15, 20]).
5. **A3 attack-then-body** geometry and numbers derived (50 in either completion order).
6. **B10 LOS** checked through the production system instead of `run_system_once(cop_sees)`, so `cop_sees` stays
   crate-private (V2's API narrowing).
7. Heat values validated > 0 (makes "report returned Some" ⇔ "heat added" and keeps `take_calls` simple).
8. Tests split into `wanted.rs` / `wanted_search.rs` (V2's additions push one file past the 750-line warning).
9. Line citations fixed: V2's `flow/wasted.rs:156-160` does not exist (file is 135 lines) — the precedent is
   `drop_queued_damage`; `reaction.rs:428-432` in PLAN is actually `reaction.rs:24-28`.

Verified API facts used: `Messages::clear`, `get_cursor_current` (`bevy_ecs-0.19.1/src/message/messages.rs:182,228`),
`Has<T>` (`query/fetch.rs:3263`), `set_if_neq` (`change_detection/traits.rs:215`), `Time<Fixed>` default 64 Hz,
`world.mutate_resources` params (`bevy_remote-0.19.1/src/builtin_methods.rs:87,308-321`), ron array length error.
Not verified here (implementer checks in the pinned source): the exact `Entity` constructor for pure unit tests.

Owner-run items (not gated): star colour/size/placement, blink feel, whether 40 m / 10 s and the 5 s merge feel right,
whether repeated witness bars over one corpse read as a bug (resolved Q3: heat counted once, behaviour stays).

children: 0 launched / 0 reported.
