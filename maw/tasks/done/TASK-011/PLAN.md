# PLAN — TASK-011 (GDD T10): wanted level core

Cost of error: **mixed, mostly silent.** Heat bookkeeping breaks silently and surfaces weeks later as "stars from
nowhere" or "stars never come": double counting of one corpse (the binding note), heat from an unwitnessed crime,
gang crimes charged to the player, stale calls or incidents leaking across `Wasted`, a search timer that never or
always clears. All of that gets headless gates with worked numbers. The owner class (how the stars look and blink,
whether 10 s / 40 m feels right, whether the witness loop is fun) gets the owner checklist plus BRP screenshots, no
machinery beyond one cheap pure-function table for the star looks.

Pinned (`Cargo.lock`, sources under `~/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/`): bevy / bevy_ecs /
bevy_time / bevy_state 0.19.1, avian3d 0.7.0, bevy-tnua 0.32.0, ron 0.12.2. **No new crate.** `citygen` is not
touched.

Evidence (planner, under `maw/tasks/in_progress/TASK-011/scratch/`):
- `ron_array_probe.txt` + `ron_probe/` (built with the shared `target/`): ron 0.12.2 reads `[StarRow; 5]` only from
  tuple syntax `((..), (..), ..)`. A list `[..]` fails with `ExpectedStructLike`. Four rows fail at parse time with
  `ExpectedDifferentLength { expected: "an array of length 5", found: 4 }`. So the type enforces the star count.
- `font_star_glyph.txt`: both shipped Inter fonts (`assets/third_party/inter/Inter-Regular.ttf`,
  `InterDisplay-Black.ttf`) contain U+2605 `★`, so the HUD can draw stars as text in the existing font.

Research (genre references are evidence, not law):
- GTA IV star colours: white = police see the player, grey = they lost sight, **blinking grey = they are about to
  abandon the search**. The wanted level dissolves when the player stays outside the search radius for some
  seconds. Callers phone "based on the last known location where the shooting occurred."
  https://gta.wiki/w/Wanted_Level_in_GTA_IV , https://gta.fandom.com/wiki/Wanted_Level_in_GTA_IV
- GTA V: a pedestrian must witness the crime and phone the police for stars; a crime with no NPC around gives no
  stars. https://gta.fandom.com/wiki/Wanted_Level_in_GTA_V , https://www.gta5-mods.com/scripts/crime-witness
- The GDD already cites gtamods thresholds (III/SA) and the Watch Dogs witness bar (§6.2, §6.4).

---

## 1. Understanding (what exists today)

**Wanted stub.** `crates/gta_sim/src/wanted/mod.rs:1-23`: `WantedLevel { stars: u8 }` (Reflect resource),
`WantedPlugin` resets it on `OnEnter(GameState::Wasted)`. `lib.rs:14,34,136` adds the plugin. The only other user
is `tests/respawn.rs:10,97,107` (sets `stars = 3`, expects 0 after death).

**Combat messages** (all written in `FixedUpdate`, `HealthSystems::Damage`, `PlayingSystems`, `combat/mod.rs:82-108`):
- `ShotFired { shooter, weapon, muzzle }`: `combat/hitscan.rs:56-62`, written once per trigger pull at `:221`.
  The attack id `shot = serial.next_id()` exists at `:190` but is **not** carried in `ShotFired`.
- `DamageDealt { shooter, shot, target, point, damage, headshot, killed }`: `hitscan.rs:75-87`, written by
  `fire_weapons` (`:263`) and `melee::apply_strikes` (`melee.rs:527`). `shot` is the `AttackSerial` id of the
  trigger pull or swing (`combat/mod.rs:29-38`, first id is 1, never 0).
- `MeleeHit { attacker, target, point, knockdown }`: `melee.rs:280-288`, written at `:536`. It does **not**
  carry the attack id; `Strike.attack` has it (`melee.rs:292-301`).

**Perception** (`perception/mod.rs`): `StimulusLog(Vec<(tick, ThreatKind, Vec3)>)` (`:89-91`), filled by
`collect_stimuli` (`:175-212`) from `ShotFired` (Gunshot), `MeleeHit` (Fight) and `DamageDealt` on a civilian
(Hurt, straight into `pending`). `perceive` (`:215-289`) picks the **nearest** threat per civilian on its slot:
sounds from the log, the nearest corpse in LOS ≤ `corpse_sight` (skipped while the civilian is in `Report`,
`:259-266`), a gun aimed at it. `Threat { kind, at, distance }` (`:62-68`) knows nothing about *which* shot or body.

**Civilian witness** (`civilian/mod.rs`): `CivilianState::Report { progress }` (`:125-128`). Entered from
`react` when the utility scorer picks Report (`:252-259`; reportable = Corpse always, Gunshot/Fight at
≥ `report_min_distance` 25 m, `reaction.rs:428-432`). A new threat during a call interrupts it
(`:283-285`, `allow_report = false` → Flee/Cower); death sets `Dead` (`:207-226`). Completion is
`progress >= 1.0 → Wander` (`:300-307`), and nothing is emitted. After completion the witness can see the same
corpse again and call again, up to ~7 times in the 30 s corpse life (TASK-009 FIX_SUMMARY.md:43-45; premise
challenge confirmed at `perception/mod.rs:259-270`, `civilian/mod.rs:279-305`).

**Corpses** (`population/mod.rs:180-185,232-241,313-334`): the victim entity itself becomes the corpse
(`corpse_components`), so the victim `Entity` is a stable key for "this body" until despawn at 30 s.

**Gangs** (`gang/mod.rs`): `Faction { Player, Gang(u8), Police }` component (`:31-37`), on the player
(`player/mod.rs:62`) and on members. `Police` exists already, and T11 cops will carry it. Gang members have
`Perception` but are not in `perceive` (it iterates `&Civilian`). They are no witnesses.

**Flow** (`flow/mod.rs`): `PlayingSystems` (fixed, only `Playing`), `NpcSystems` (fixed, `Playing` or `Wasted`).
`AiSystems::{Perceive, Decide}` then `PopulationSystems`, chained, in `NpcSystems`, after `HealthSystems::Death`
(`perception/mod.rs:136-146`). Civilians keep calling during `Wasted`.

**HUD** (`src/hud/`): top-right bars (`mod.rs:61-97`), ammo text below them (`weapon.rs:296-309`, top =
`margin + 2*(bar_height+bar_gap)`), witness bar over callers reading `Report { progress }` (`witness.rs:157-162`)
with a headless client gate (`witness_gate.rs`). HUD numbers live in `assets/ui/strings.ron` → `UiConfig.hud`
(`src/menu/config.rs:9-67`, "no separate hud.ron").

**Tests harness** (`crates/gta_sim/tests/common/mod.rs`): `headless_app()` (production `compose_sim`, 80×80 m test
floor), `run_ticks`, `place_player`, `spawn_civilian`, `set_civilian_state`, `test_graph`, `Shots`,
`spawn_member`, `set_health_of`, `spawn_wall`. Test floor fixtures (`world/test_area.rs:6-19`): a 12×4×0.5 m wall
centred (0, 2, 14), x ∈ [−6, 6], z ∈ [13.75, 14.25].

**Data law.** GDD §12 names `assets/wanted/wanted.ron` for heat, stars, witnessing and the search circle. It does
not exist yet.

---

## 2. Approach

### 2.1 Model: incidents, not calls

A **crime** by the player creates or extends an **incident** in a `Crimes` resource. An incident contributes its heat
**once**, when it is first *reported*. A report comes from a cop fixture that sees the crime, or from a completed
civilian call that names it. A call or sighting that resolves only to already-reported incidents does nothing: no
heat, no circle move, no timer reset. That is the binding note ("one contribution per corpse/incident") as a type
invariant (`Incident.reported: bool`), not a per-call counter.

Incident kinds (`Crime`), heat from `wanted.ron` (GDD §6.4 table, T10 rows only):

| Crime | Recorded when (offender = the `Player` entity) | Keyed by | Heat |
|---|---|---|---|
| `Punch` | melee `DamageDealt`, non-lethal, target is a `Civilian` | victim | `punch_civilian` 5 |
| `Wound` | gun `DamageDealt`, non-lethal, target is a `Civilian` | victim | `wound_civilian` 30 |
| `Kill` | `DamageDealt.killed`, target is a `Civilian` **or** `GangMember` | victim | `kill_person` 40 |
| `Shooting` | `ShotFired` with a live (`Health.current > 0`) civilian or gang member within `shooting_radius` 15 m of the muzzle | offender episode: joins the offender's last `Shooting` incident if its last shot is ≤ `shooting_merge_seconds` old | `shooting_near_people` 10 |

A second hit of the same kind on the same victim extends the existing incident (appends the attack id), so it does
not create a new one. Crimes by gang members (or anyone who is not the `Player`) are **not** recorded, because heat
is the player's. Punching or wounding a gang member is not in the GDD table, so it gives 0. Killing one is 40 (GDD:
"Убийство мирного / бандита"). Shooting near gang members counts as "near people". Cop rows (45/80/150, "always"),
car theft and run-over belong to T11/T14/T15, which add them to `HeatTable` and `Crime`.

### 2.2 Witness → incident identity (the missing link)

Perception gains a **cause** for every threat: `Cause::Attack(u32)` for Gunshot/Fight/Hurt (the `AttackSerial`
id), `Cause::Body(Entity)` for a Corpse, `None` for Aimed. `Report` stores it: `Report { progress, about:
Option<Cause> }`. On completion the civilian writes a `PoliceCall { caller, about }` message. `wanted` resolves it
exactly:
- `Attack(a)` → every incident whose `attacks` contains `a` (the shooting episode and any wound or kill done by that
  very trigger pull or swing);
- `Body(v)` → every incident with `victim == v` (kill plus earlier wounds or punches on that body).

Rejected (log `decision`):
- Resolving by offender entity. A call would also report the offender's *older, unwitnessed* crimes elsewhere,
  which breaks "crime without a witness = 0 heat".
- A `wanted`-owned geometric witness set. It duplicates perception radii and LOS, and drift is guaranteed.

To carry the ids, `ShotFired` gains `attack: u32` and `MeleeHit` gains `attack: u32`. Both values are already
computed at the write sites.

### 2.3 Search: `LastKnownPosition`, circle, timer (GDD §6.4, GTA IV reading)

All state lives in one reflected resource, so QA and T11 read one thing and a reset is `*w = default()`:

```rust
pub struct WantedLevel {
    pub heat: u32,
    pub stars: u8,                // stars_for(heat), recomputed every tick (a BRP heat mutation shows up next tick)
    pub last_known: Option<Vec3>, // GDD `LastKnownPosition`: centre of the search circle; None while heat == 0
    pub seen: bool,               // a cop sees the player this tick
    pub hidden: f32,              // s the player has been continuously unseen AND outside the circle
}
```

`search_step` runs once per fixed tick while the player is alive:
1. `heat == 0` → reset to default, return.
2. `row = cfg.stars[max(stars, 1) - 1]`. Sub-threshold heat (e.g. one shooting, 10) clears by the star-1 rule and does
   not linger until death. This is a decision, see §5 Q1.
3. `last_known.get_or_insert(player)`. A heat raised by a BRP mutation (T11 QA) gets a centre.
4. If a cop sees the player: `last_known = player`, `hidden = 0`, `seen = true`.
5. Else if `flat_distance(player, last_known) > row.search_radius`: `hidden += dt`, and when `hidden >= row.clear_seconds`
   → reset to default (0 stars).
6. Else (unseen, inside the circle): `hidden = 0`.

"Вне круга **и** не замечен N с" is read as both conditions holding *continuously* for N s. Coming back into the
circle resets the timer. A **new** report (heat added) moves the circle: `last_known = incident.at` (offender
position at crime time, IV: "last known location where the shooting occurred"), `hidden = 0`.

**Cop sight** (fixture now, T11 cops later): any live entity with `Faction::Police` + `Position` + `Rotation`.
Eyes = `position − Y·float_height + Y·head_height` (the perception convention, `perception/mod.rs:241`).
- *Sees the player* (search): flat angle between the cop's forward (`rotation * NEG_Z`) and the flat line to the
  player ≤ `cop_view_cone_deg / 2`, 3D distance ≤ `cop_view_distance` (35 m on foot), and `!sight_blocked` eyes → eyes.
- *Witnesses a crime*: distance ≤ `cop_witness_distance` (50 m) and LOS, no cone (GDD: "коп видит лично (LOS ≤ 50 м)").
  Checked at record time for each new or extended unreported incident against the offender's eyes.

Cost: ≤ 12 cops × 1 ray per tick while heat > 0, plus a few rays per crime. Measured avian ray ~0.6 µs (TASK-009
lesson), so this runs synchronously with no slicing and no async task.

### 2.4 Schedule

New `WantedSystems` set in `FixedUpdate`: `.after(AiSystems::Decide).in_set(PlayingSystems)`, systems chained:
`record_crimes → take_calls → track_search → forget_crimes`. After `Decide` means this tick's combat messages and this
tick's completed calls are both visible (`MessageReader` of messages written earlier in the same tick is the
existing pattern, e.g. `collect_stimuli`). `PlayingSystems` means no wanted logic while the player is dead.
`OnEnter(Wasted)`: `WantedLevel` default **and** `Crimes` cleared. A call that completes during or after `Wasted`
about a pre-death crime then resolves to nothing. Ids are monotonic, so an old cause never matches a new incident.
`Messages<PoliceCall>` is deliberately *not* cleared on `OnExit(Wasted)`: with the log cleared it would be an
ungateable no-op (log `dead_end`).

Which clock: `Time<Fixed>` (`delta_secs` for `hidden`, `elapsed_secs_f64` for incident timestamps). The HUD blink
uses `Time<Real>` in `Update` (presentation).

### 2.5 HUD

`src/hud/stars.rs`: a row of 5 `★` text nodes top-right under the ammo line. It is hidden while `stars == 0`. Looks
follow GTA IV: earned star + `seen` → lit; earned + unseen inside the circle (`hidden == 0`) → grey; earned +
`hidden > 0` → blinking grey/off every `blink_seconds`; unearned slot → off. Before T11 there are no cops, so the
owner sees grey after a call, blinking once outside the circle, then nothing. Numbers go in `strings.ron` `hud.stars`.
The witness bar over callers already exists (T8) and stays as it is.

### 2.6 T11 hand-off (what TASK-012 consumes)

`WantedLevel.{stars, last_known, seen}` drives the dispatcher and "go to `LastKnownPosition`". Cops are characters
with `Faction::Police`. Witnessing and search sight then work with no wanted-side change. `wanted::cop_sees` /
`wanted::in_view` are `pub` for the cop FSM. T11 adds cop crimes ("always" = report at record time) to `Crime` /
`HeatTable`, and "вырваться = +1 звезда" as `heat = max(heat, next row threshold)`. `Busted` repeats the `Wasted`
reset.

---

## 3. Steps

Build and test with `CARGO_TARGET_DIR=D:/test-gta-like/target` and `-j 4` (the host is under memory pressure).

### 3.1 Data: `assets/wanted/wanted.ron` (new)

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
    shooting_radius: 15.0,          // m: a live civilian or gang member this close to the muzzle makes a shot a crime
    shooting_merge_seconds: 5.0,    // a shot this soon after the offender's last one joins that shooting incident
    incident_memory_seconds: 60.0,  // an incident untouched this long is forgotten (a later call about it adds nothing)
    cop_witness_distance: 50.0,     // m, LOS, any direction: a cop witnesses a crime (GDD §6.4)
    cop_view_distance: 35.0,        // m on foot: a cop sees the player (search)
    cop_view_cone_deg: 110.0,       // full cone of that view
)
```
Tuple syntax for `stars` is required (probe). `shooting_merge_seconds` and `incident_memory_seconds` are new tuning
knobs the GDD does not number. They are data, and the owner can turn them (§5 Q2).

### 3.2 `crates/gta_sim/src/wanted/mod.rs` (rewrite of the 23-line stub, ~220 lines)

- `pub const WANTED_CONFIG: &str = "wanted/wanted.ron";`
- `pub const STARS: usize = 5;`: law (GDD §6.4 "у нас 5 звёзд"), with a one-line comment.
- `WantedConfig` (`Resource, Deserialize, Clone, Debug`, `deny_unknown_fields`): `heat: HeatTable`,
  `stars: [StarRow; STARS]`, `shooting_radius`, `shooting_merge_seconds`, `incident_memory_seconds`,
  `cop_witness_distance`, `cop_view_distance`, `cop_view_cone_deg` (all `f32`). `HeatTable { punch_civilian,
  shooting_near_people, wound_civilian, kill_person: u32 }`, `StarRow { heat: u32, search_radius: f32,
  clear_seconds: f32 }`, both `deny_unknown_fields`.
- `WantedConfig::validate()` in the style of `PerceptionConfig::validate`: row heats > 0 and strictly increasing
  (error names `stars[i].heat`); `search_radius`, `clear_seconds` finite > 0; `shooting_radius`,
  `cop_witness_distance`, `cop_view_distance` finite > 0; `shooting_merge_seconds` finite ≥ 0;
  `incident_memory_seconds` finite and > `shooting_merge_seconds`; `cop_view_cone_deg` in (0, 360).
- `WantedLevel` as in §2.3 (`Resource, Reflect, Default, Debug, Clone, Copy, PartialEq`, `#[reflect(Resource)]`), with
  `///` docs per field.
- `pub fn stars_for(heat: u32, rows: &[StarRow; STARS]) -> u8`: the count of rows with `heat >= row.heat`.
- `#[derive(SystemSet)] pub struct WantedSystems;`
- `WantedPlugin::build`: `init_resource::<WantedLevel>()`, `init_resource::<Crimes>()`, `register_type` for
  `WantedLevel`; `configure_sets(FixedUpdate, WantedSystems.after(AiSystems::Decide).in_set(PlayingSystems))`;
  `add_systems(FixedUpdate, (crimes::record_crimes, crimes::take_calls, search::track_search,
  crimes::forget_crimes).chain().in_set(WantedSystems))`; `add_systems(OnEnter(GameState::Wasted), reset_wanted)`.
- `reset_wanted(mut wanted, mut crimes)`: `*wanted = WantedLevel::default(); crimes.clear();`.
- `mod crimes; mod search;` with re-exports `pub use crimes::{Crime, Crimes, Incident, IncidentId, Victim,
  classify}` and `pub use search::{cop_sees, in_view, search_step, witnesses}`. `Cause` lives in perception (3.5).
- `#[cfg(test)]` unit tests: `stars_table` over the shipped file (`include_str!("../../../../assets/wanted/wanted.ron")`,
  `GATE BROKEN` if the rows are not 40/180/550/1200/2400): heat 0→0, 39→0, 40→1, 179→1, 180→2, 549→2, 550→3,
  1199→3, 1200→4, 2399→4, 2400→5, `u32::MAX`→5.

### 3.3 `crates/gta_sim/src/wanted/crimes.rs` (new, ~330 lines incl. tests)

Plain data plus thin systems, so the dedupe logic is unit-testable without an `App`:

```rust
#[derive(Clone, Copy, Debug, PartialEq, Eq)] pub enum Crime { Punch, Shooting, Wound, Kill }
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)] pub struct IncidentId(u32);
pub struct Incident { pub id, pub crime: Crime, pub offender: Entity, pub victim: Option<Entity>,
                      pub attacks: Vec<u32>, pub at: Vec3, pub last: f64, pub reported: bool }
#[derive(Resource, Default)] pub struct Crimes { next: u32, incidents: Vec<Incident> }
```
- `Crimes::record(&mut self, crime, offender, victim, attack, at, now, merge) -> IncidentId`. It extends the matching
  open incident, else pushes a new one. Match: same `offender` and `crime`, and either same `victim` (victim crimes)
  or (`Shooting`) `now - last <= merge`. Extending appends `attack`, sets `at` and `last`.
- `Crimes::report(&mut self, id) -> Option<(u32 heat, Vec3 at)>`: `Some` only on the first report (sets
  `reported`), `None` after. Heat is looked up by the caller via `HeatTable::of(crime)`. Simplest: `report` takes
  `&HeatTable` and returns `Option<(u32, Vec3)>`.
- `Crimes::resolve(&self, cause: Cause) -> Vec<IncidentId>`: the rules of §2.2.
- `Crimes::forget(&mut self, now, memory)`: drops incidents with `now - last > memory`. `Crimes::clear()`.
- `pub enum Victim { Civilian, Gang, Other }` and `pub fn classify(victim, melee, killed) -> Option<Crime>`:
  Civilian → Kill / Punch / Wound; Gang → Kill if killed, else None; Other → None.
- `record_crimes` system: params `Res<WantedConfig>`, `Res<LocomotionConfig>`, `Res<Time<Fixed>>`,
  `SpatialQuery`, `ResMut<Crimes>`, `ResMut<WantedLevel>`, `MessageReader<ShotFired>`,
  `MessageReader<MeleeHit>`, `MessageReader<DamageDealt>`, `Query<(), With<Player>>`,
  `Query<&Position>`, `Query<(Has<Civilian>, Has<GangMember>)>`,
  `Query<(&Position, &Health), Or<(With<Civilian>, With<GangMember>)>>`, cops `Query<(&Position, &Faction),
  Without<Dead>>`. Order inside: collect melee attack ids from `MeleeHit` (this tick), then each player
  `DamageDealt` → `classify(victim, melee = attack in melee set, killed)` → `record` (at = shooter `Position`);
  then each player `ShotFired` → if any person has `health.current > 0` within `shooting_radius` of `muzzle` →
  `record(Shooting, .., at = muzzle)`. `current > 0` rather than `Without<Dead>` makes a victim killed by this very
  shot deterministically not "near": `Dead` is a deferred insert. For each id recorded this tick that is still
  unreported, if any cop `witnesses` (search.rs) the offender → `report` → `apply_report`.
- `apply_report(wanted, heat, at)` (shared by cops and calls): `wanted.heat = wanted.heat.saturating_add(heat);
  wanted.last_known = Some(at); wanted.hidden = 0.0;`
- `take_calls` system: `MessageReader<PoliceCall>` → `resolve(call.about)` → `report` each. It calls `apply_report`
  once with the summed new heat and the `at` of the newest reported incident, only if the sum is > 0 or some report
  returned `Some`.
- `forget_crimes` system: `crimes.forget(time.elapsed_secs_f64(), cfg.incident_memory_seconds)`.
- `#[cfg(test)]` unit tests (pure, worked numbers, heat from a literal `HeatTable {5, 10, 30, 40}`):
  1. `classify_table`: 9 rows (Civilian/Gang/Other × melee/gun/killed) → expected `Option<Crime>`.
  2. `one_body_one_contribution`: `record(Kill, p, Some(v), 7, ..)`; `resolve(Body(v))` → `[id]`, `report` → `Some(40)`;
     second `resolve`+`report` → `None`; `resolve(Attack(7))` → same id → `None`.
  3. `shooting_episode_merges`: attacks 1 @ t=0, 2 @ t=1.0 (merge 5) → one incident `attacks == [1, 2]`; attack 3 @
     t=7.0 (6.0 > 5 after the last shot at 1.0) → a second incident; `report` via `Attack(1)` → 10, via `Attack(2)` →
     `None`, via `Attack(3)` → 10.
  4. `victim_crimes_dedupe_per_victim`: two `Wound` on v (attacks 4, 5) → one incident; `Wound` on w → another.
  5. `forget_after_memory`: incident at t=0, `forget(60.0, 60.0)` keeps it, `forget(60.5, 60.0)` drops it;
     `resolve` then returns `[]`.
  6. `other_offenders_record_nothing` is covered by the system rule (player filter). Keep it as an integration test
     (3.9), not here.

### 3.4 `crates/gta_sim/src/wanted/search.rs` (new, ~220 lines incl. tests)

- `pub fn in_view(eye: Vec3, forward: Vec3, target: Vec3, cone_deg: f32, distance: f32) -> bool`: 3D distance ≤
  `distance` and flat angle (XZ) between `forward` and `target − eye` ≤ `cone_deg / 2`. A zero flat vector counts as
  in view.
- `pub fn cop_sees(spatial, cop_eye, cop_forward, player_eye, cfg) -> bool` = `in_view(..cop_view..) &&
  !sight_blocked(spatial, cop_eye, player_eye)`.
- `pub fn witnesses(spatial, cop_eye, offender_eye, cfg) -> bool` = distance ≤ `cop_witness_distance` && LOS.
- `pub fn search_step(w: &mut WantedLevel, player: Vec3, seen: bool, dt: f32, cfg: &WantedConfig)`: §2.3 verbatim
  (uses `navigation::flat_distance`, sets `stars = stars_for(heat)` first).
- `track_search` system: player `Query<&Position, (With<Player>, Without<Dead>)>` (let-else return), cops
  `Query<(&Position, &Rotation, &Faction), Without<Dead>>` filtered to `Faction::Police`. The cop check runs only
  while `heat > 0`. Then `search_step`. Write through `ResMut` only when the value changes (`set_if_neq`-style) so
  `Changed` stays meaningful for the HUD.
- `#[cfg(test)]` unit tests, all with the shipped cone 110° / 35 m (`GATE BROKEN` otherwise). There are 3 directional
  cases, per the math rule. Forward is `Quat::from_rotation_y(yaw) * NEG_Z`:
  - yaw 0 (forward −Z), eye origin: (0,0,−10) ✓; (−5,0,−10) (26.6°) ✓; (10,0,0) (90°) ✗; (0,0,10) ✗;
    off-grid edges 54°: (8.090,0,−5.878) ✓, 56°: (8.290,0,−5.592) ✗; (0,0,−34.9) ✓, (0,0,−35.1) ✗.
  - yaw +90° (forward −X): (−10,0,0) ✓; (0,0,−10) ✗.
  - yaw 180° (forward +Z): (0,0,10) ✓; (0,0,−10) ✗.
  - `search_step` table with dt = 1/64, row 1 (40 m, 10 s), `last_known` (0,0,0): inside (20,0,0) → hidden stays 0;
    outside (30,0,30) (42.43 m) 639 steps → `hidden == 639/64`, still 1 star; step 640 → default. `seen` at step 320
    → `hidden == 0`, `last_known == player`. `heat == 0` → default. `heat = 40, last_known = None` → centre set to the
    player on the first step.

### 3.5 `crates/gta_sim/src/perception/mod.rs`

- Add `#[derive(Reflect, Clone, Copy, Debug, PartialEq, Eq)] pub enum Cause { Attack(u32), Body(Entity) }` with a `///`
  line ("what produced a threat: an attack id (`AttackSerial`) or a corpse").
- `Threat` gets `pub cause: Option<Cause>`.
- `StimulusLog` becomes `(tick, kind, point, attack)`, and the doc comment says so. `collect_stimuli`: Gunshot uses
  `shot.attack`, Fight uses `hit.attack`, Hurt threat `cause: Some(Cause::Attack(hit.shot))`. Retain logic unchanged.
- `perceive`: `offer` takes a `cause`. Log entries → `Some(Cause::Attack(attack))`. The corpse query becomes
  `Query<(Entity, &Position), With<Corpse>>` → `Some(Cause::Body(e))`. Aimed → `None`.
- `PerceptionPlugin`: `register_type::<Cause>()`.

### 3.6 `crates/gta_sim/src/combat/hitscan.rs`, `combat/melee.rs`

- `ShotFired` gains `/// Attack id (`AttackSerial`), equal to `DamageDealt.shot` of its pellets. pub attack: u32`.
  Set it at `hitscan.rs:221` from `shot`.
- `MeleeHit` gains `pub attack: u32` (same doc), set from `strike.attack` at `melee.rs:536`.

### 3.7 `crates/gta_sim/src/civilian/mod.rs`

- `CivilianState::Report { progress: f32, about: Option<Cause> }`, with the doc "`about` names the crime being
  phoned in".
- `react`: `Reaction::Report => CivilianState::Report { progress: 0.0, about: threat.cause }`.
- `next_state` `(Report { progress, about }, None)` keeps `about` while progressing.
- `#[derive(Message, Reflect, Clone, Copy, Debug)] #[reflect(Message)] pub struct PoliceCall { pub caller: Entity,
  pub about: Cause }`, doc: "a completed witness call (GDD §6.2); `wanted` turns it into heat".
- `CivilianPlugin`: `add_message::<PoliceCall>()`, `register_type::<PoliceCall>()`.
- `civilian_fsm`: the query gains `Entity`, and a param `MessageWriter<PoliceCall>` is added. Right after
  `next_state`, and **before** `arrive` (which may turn Wander into Idle):
  `if let (CivilianState::Report { about: Some(about), .. }, CivilianState::Wander) = (civilian.state, state) {
  calls.write(PoliceCall { caller: entity, about }); }`. Report → Wander happens only by completion (a threat during
  a call goes through `react(.., false)`, which never returns Wander). Death never reaches this point
  (`Dead` → `continue`).
- Grep check (domain rule on new params): `civilian_fsm` is registered only by `CivilianPlugin`, and
  `PoliceCall` is registered there too.

### 3.8 `crates/gta_sim/src/lib.rs`

Load and validate `WantedConfig` like the others (`WANTED_CONFIG`) and `insert_resource(wanted)` before
`add_plugins`. `WantedPlugin` stays last in the tuple.

### 3.9 Headless integration gates: `crates/gta_sim/tests/wanted.rs` (new, ~550 lines)

Production composition (`headless_app()`), test floor, `TimeUpdateStrategy::FixedTimesteps(1)`. Local helpers
modelled on `civilians.rs` (not moved: surgical): `armed(h)` = square graph of half-size `h` + far segment
(−34,0,−30)–(−26,0,−30), player settled at the origin with a full pistol; `muzzle_of`; `kill_with_one_shot(app,
target)` = named test mutation `set_health_of(target, current = 1.0)` then aim at its chest and `Shots::run(1)`,
assert `killed` in `dealt_log` (GATE BROKEN otherwise); `calls(app)` = a `MessageCursor<PoliceCall>` reader like
`Shots`; `wanted(app) -> WantedLevel`; `incidents(app)` via a `pub fn Crimes::incidents(&self) -> &[Incident]`
accessor; `spawn_cop(app, chest, yaw)` = the bare fixture `(Name::new("Cop fixture"), Faction::Police,
Position(chest), Rotation(Quat::from_rotation_y(yaw)))`. It has no collider, so it blocks no ray, and avian does not
touch an entity with no body and no `Transform`. Chest = feet + `float_height`. Every expected number below comes from
the shipped configs read at test start, with `GATE BROKEN` asserts on the values the worked example assumes (heat
40/10, 256-tick call, 640-tick clear, radii).

Worked setup W (report-prone corpse witness, from `civilians.rs:367-409`): `armed(10)`, witness on `SIDES[0]` t=0.5
→ feet (0,0,−10), temperament (flee 0.6, cower 1.0, report 1.4); victim on `SIDES[0]` t=0.8 → (6,0,−10); both
`hold_idle`. Kill the victim. Nearest threat for the witness = the corpse at 6.0 m (the gunshot muzzle is ≈ 9.5 m,
and it is < 25 m, so not reportable): flee 0.6, cower 1.5·1.0·(1−6/15) = 0.9, report 0.8·1.4 = 1.12 → **Report**,
`about = Some(Body(victim))` within ≤ `slots` (4) ticks. The call completes after 256 ticks (4 s × 64).

1. `unwitnessed_kill_is_zero_heat`: `armed(10)`, victim only, plus a report-prone civilian on the far segment at
   (−30,0,−30): 42.4 m from the muzzle (> hearing 40) and 41.2 m from the body (> corpse sight 20). Kill the victim.
   Liveness: `incidents` holds exactly one `Kill` with `victim`, offender = player, `reported == false`. Run 2×256+16
   ticks: `heat == 0`, `stars == 0`, zero `PoliceCall`, the far civilian still calm. *Flip*: `record_crimes` reports
   every new incident as if witnessed → RED.
2. `repeat_calls_about_one_corpse_count_once` (binding note): setup W. First completed call → `PoliceCall` count 1,
   `heat == kill_person (40)`, `stars == 1`, `last_known ≈` player position at the shot (±0.1 m). `hold_idle` the
   witness again (named mutation: keeps it in sight of the body). It calls again → count 2, `heat` still 40.
   Liveness is count ≥ 2, otherwise the gate is vacuous. *Flip*: `Crimes::report` returns heat even when already
   reported → heat 80 → RED.
3. `killing_the_caller_interrupts_the_call`: setup W. Once the witness is in `Report`, run 24 ticks (still
   `Report`), then kill the witness with one shot. Run 256+16 ticks: zero `PoliceCall`, `heat == 0`. Test 2 is the
   positive twin. *Flip*: write `PoliceCall` on *entering* Report instead of on completion → RED.
4. `shot_near_people_is_reported_by_a_hearing_caller`: `armed(30)` + a segment (−5,0,8)–(5,0,8) for a bystander at
   (0,0,8) (≈ 8.5 m from the muzzle, ≤ 15), `hold_idle`; witness on `SIDES[0]` t=0.5 → (0,0,−30), temperament
   (0.7, 1.0, 1.5) → Report on a gunshot at 30 m (row 4 of `reaction.rs` table). Shoot into the air once
   (`shoot_into_the_air` pattern). After the call: `heat == shooting_near_people (10)`, `stars == 0`, and
   `last_known == Some(muzzle)` (sub-threshold search runs). **Control** in the same test file:
   `lone_shot_is_no_crime`: same without the bystander → the call still completes (count 1), `heat == 0`, no
   incident.
5. `cop_fixture_witnesses_a_kill`: `armed(10)`, victim at (6,0,−10), cop fixture chest at (−20, fh, 0) (20 m, clear
   line; fixtures at x=−10 lie at z ≤ −11.4). Kill → **in the shot tick** `heat == 40`, `stars == 1`. Two more cases
   in the same test with fresh apps: cop at (0, fh, 20) (the test wall at z≈14 blocks the line to the origin) → heat
   0, the Kill incident unreported; cop at (−36, fh, −36) (50.9 m > 50) → heat 0.
6. `gang_victims_follow_the_table`: `armed(10)`, cop at (−20, fh, 0), `spawn_member(gang 0, (6,0,−10), Pistol)`
   (no territories on the floor → `GangSystems` off, the member stays idle). One body shot at full health (a wound,
   not lethal) → `heat == shooting_near_people (10)` (the member is alive within 15 m; wounding a gang member is not
   a crime). Then kill it with one shot (health 1) → `heat == 10 + 40`.
7. `gang_crimes_are_not_the_players`: setup W, but a gang member kills the victim through the real gun.
   `spawn_member` at (2,0,−2) (no territories → its FSM does not run and cannot overwrite intents). Set its
   `AimIntent` at the victim's chest and `ActionIntent.fire_requested`; its `Loadout` is armed by
   `gang_member_bundle`. The matrix spares bystanders only in melee (`gang/mod.rs:349-354`), so the bullet hits. The
   witness still phones in `Body(victim)` (count 1) → `heat == 0` and `incidents` empty. If driving a member's gun
   proves flaky, the named fallback is a hand-written `DamageDealt { shooter: member, killed: true, .. }`. That one is
   weaker (it bypasses `fire_weapons`), so record it as such.
8. `stars_follow_a_heat_mutation` (T11 QA path): `WantedLevel.heat = 550` → after 1 tick `stars == 3`,
   `last_known == Some(player position)`.
9. `hiding_outside_the_circle_clears` (AC "вне круга и без видимости N с → 0"): heat 40 at the origin, 1 tick →
   `last_known ≈ origin`. `place_player((30, y, 30))` (42.43 m > 40). After tick k: `hidden == k/64` (exact in
   f32). k = 639 → `stars == 1`; k = 640 → `heat == 0`, `stars == 0`, `last_known == None`. The number of ticks is
   `clear_seconds × 64`, asserted integral. **Control**: `place_player((20, y, 0))` for 1280 ticks → `stars == 1`,
   `hidden == 0`. *Flip*: count `hidden` even inside the circle → control RED; drop the `hidden = 0` reset on
   re-entry → a variant (320 ticks out, 1 tick in, 320 out → still 1 star) RED.
10. `spotted_resets_timer_and_moves_circle` (AC "замечен → таймер сброшен"): from test 9 at k = 320 (hidden 5.0),
    `spawn_cop((30, fh, 10), yaw = π)` (forward +Z, the player 20 m straight ahead, no fixture at x=30). Next tick:
    `seen`, `hidden == 0`, `last_known ≈ player`. Despawn the cop. 1280 ticks at (30,30) → still `stars == 1` (the
    old circle would have cleared after 320). `place_player((−30, y, −30))` (84.9 m from the new centre) → clears at
    exactly k = 640. **Negative** in the same test: a cop at (30, fh, 10) with yaw 0 (facing away) → `hidden` keeps
    growing. LOS case (via `run_system_once` + `SpatialQuery` + `cop_sees`): cop eye (0, eye, 20) facing −Z, player
    eye (0, eye, 8) → blocked by the wall (false); cop at (−15, eye, 20) facing the player (`aim_yaw((15, 0, −12))`),
    the line crosses z=14 at x = −7.5, outside the wall → true.
11. `wasted_forgets_the_crimes` (leak class, TASK-006/007 lesson): setup W; once the witness is in `Report`,
    `write_damage(1000)` → `Wasted` (the loop from `npcs_live_through_wasted`) → update until `Playing` → run 300
    ticks. Liveness: a `PoliceCall` about `Body(victim)` was written after death (civilians call through `Wasted`).
    Result: `heat == 0`, `incidents` empty. *Flip*: remove `crimes.clear()` from `reset_wanted` → heat 40 → RED.

Tests 2, 5, 9, 10 and 11 are the correctness gates. 1, 3 and 4 carry "no witness / interrupted / not a crime". 6 and 7
carry the gang rules. 8 is liveness of the BRP path. Record every flip in the implementer summary.

### 3.10 Updates of existing tests (mechanical, forced by the new fields)

- `crates/gta_sim/tests/civilians.rs`: the exact-equality asserts on `Report { progress: .. }` (`:344`, `:351`,
  `:401`, `:426`) become asserts on a local `fn call_progress(state) -> Option<f32>` (`Report { progress, .. }`).
  The `Threat` literal at `:381` gains `cause: None` (the scorer does not read it).
- `crates/gta_sim/src/civilian/reaction.rs:73` test literal: `cause: None`.
- `crates/gta_sim/tests/civilian_bench.rs:107` and `src/visuals/character_gate.rs:488`: `ShotFired { .., attack: 0 }`
  (0 is never a real `AttackSerial` id).
- `crates/gta_sim/tests/respawn.rs:97`: `stars = 3` would now be overwritten by `stars_for(0)` on the next tick
  and prove nothing. Set `heat` to `stars[2].heat` (550) instead. After death assert `heat == 0 && stars == 0`.
- `src/hud/witness.rs:41` → `Report { progress, .. }`; module doc line 2 likewise. `src/hud/witness_gate.rs:79,85,98`
  literals gain `about: None`.

### 3.11 Config gates: `crates/gta_sim/tests/config.rs`

- `shipped_wanted_config_loads_and_validates`.
- `unknown_wanted_field_names_file_and_field`: the existing pattern, `shooting_radius: 15.0,` →
  `shooting_radius: 15.0, bogus: 1,`, error names `wanted.ron` and `bogus`.
- `star_thresholds_must_increase` via `sabotaged`: `(heat: 180,` → `(heat: 30,` (strictly below 40, not on the
  boundary), error mentions `stars[1].heat`.
- `five_star_rows_required`: drop the 5th row → the load error contains `length 5` (the parse error, a different
  error from the validate one).

### 3.12 Client HUD

- `src/menu/config.rs`: `HudLayout` gains `pub stars: StarsConfig` (`deny_unknown_fields`): `glyph: String`,
  `size: f32`, `gap: f32`, `lit_color: Rgb`, `gray_color: Rgb`, `off_color: Rgba`, `blink_seconds: f32`. Validate:
  glyph non-empty, size/gap/blink finite > 0 (gap ≥ 0), colours in [0, 1].
- `assets/ui/strings.ron` `hud`: `stars: (glyph: "★", size: 26.0, gap: 2.0, lit_color: (1.0, 1.0, 1.0),
  gray_color: (0.55, 0.55, 0.55), off_color: (0.0, 0.0, 0.0, 0.45), blink_seconds: 0.25)` with a one-line comment
  "GTA IV: white seen, grey lost, blinking grey = about to give up".
- `src/hud/stars.rs` (new, ~110 lines): `#[derive(Component)] struct StarSlot(u8)`; `spawn_stars` (on the same
  `Loading → Playing` transition as `spawn_hud`): an absolute row, `right: margin`, `top: margin + 2*(bar_height +
  bar_gap) + ammo_size + bar_gap`, `column_gap: gap`, 5 `Text(glyph)` children with `TextFont` from `UiFonts.regular`
  and `size`. `pub(super) enum StarLook { Lit, Gray, Off }`, and the pure `pub(super) fn star_look(slot: u8, w:
  &WantedLevel, real_secs: f32, blink: f32) -> StarLook`: slot ≥ stars → Off; `seen` → Lit; `hidden > 0` → Gray on
  even `floor(real_secs / blink)`, Off on odd; else Gray. `update_stars` (Update): row `Visibility` hidden when `stars
  == 0`, per-slot `TextColor` via `set_if_neq`.
- `src/hud/mod.rs`: `mod stars;`, add `stars::spawn_stars` to the transition tuple and `stars::update_stars` to
  `Update`; the plugin doc line mentions the stars.
- `#[cfg(test)]` in `stars.rs`: `star_look_table` with `WantedLevel` rows: stars 2 seen → slots 0,1 Lit, 2..4 Off;
  stars 2 unseen `hidden 0` → Gray; `hidden 0.1` at t = 0.1 (Gray) and t = 0.3 (Off) with blink 0.25; stars 0 →
  all Off. The look itself (colour, size, position) is the owner's.

### 3.13 Runtime QA: `tools/qa/scenarios/t10.py` (new, ~260 lines, the t8/t9 style)

Via `brp.py`, `--seed 1`, release, features `dev`:
1. Wait for `Playing`, golden `CityLayoutHash`, chunks (`t5.wait_chunks`), bubble ≥ 20 civilians. Pick up the pistol
   (t8 step 3).
2. Choose a victim civilian with ≥ 3 other civilians within 40 m. **Named QA mutations** (as t9's armour): the
   victim's `Civilian.state` → `{"Idle": {"left": 1e6}}` and `Health.current` → 1 (one shot kills). One more
   civilian at 25-38 m from the planned muzzle gets `Idle` + temperament `{flee 0.5, cower 1.0, report 1.5}`: a
   guaranteed caller, since the random temperament roll is not the property under test. Teleport the player 5 m from
   the victim, `t6.aim_at` the chest, click once.
3. Poll `WantedLevel` up to `call_seconds + 3` s: `heat >= kill_person` and `stars >= 1`. Summary keeps heat, stars,
   `last_known`. Screenshot `hud_wanted.png` (stars grey).
4. Teleport to whichever of `HospitalSpawn.point`, `CityLandmarks.plaza_center`, `park_center` is farthest from
   `last_known`. Assert that distance > `search_radius[stars-1] + 10` (GATE BROKEN otherwise). After ~1 s, screenshot
   `hud_blinking.png` (space captures ≥ 0.15 s). Assert `hidden > 0` and `seen == false`.
5. Poll: `stars` must still be ≥ 1 at `clear_seconds − 1` s and must be 0 (and `heat == 0`) by `clear_seconds + 3`
   s. Screenshot `hud_clear.png`. The wall-clock bounds are loose on purpose, since BRP latency is not the gate.
6. `log_errors` empty, `shutdown`. Each constant is read from `wanted.ron` / `civilian.ron` with the t8 `ron_number`
   regex helper.

Owner checklist for QA_REPORT.md (the QA stage writes it): "стреляет при свидетелях → над свидетелем полоса звонка
→ звёзды (серые); убегает за круг → звёзды мигают → через 10 с при 1 звезде розыск пропадает; убийство свидетеля до
конца звонка → звёзд нет; стрельба в воздух без людей рядом → звёзд нет".

---

## 4. Risk areas

- **Nearest-threat masking.** A witness reports only the threat it reacted to (nearest). A caller who heard the
  gunshot reports that attack (kill and shooting of that trigger pull). One who saw the body reports the body. So the
  heat per call can differ (40 vs 50) for one scene. This is by design, and the owner tunes it. Tests pin exact
  geometry to avoid ambiguity.
- **Repeat calls still happen visually.** The witness bar may appear several times over one body (T8 behaviour).
  Heat is correct. Per-civilian memory is a behaviour change outside this slice (§5 Q3).
- **`Dead` deferral.** "Near people" uses `Health.current > 0`, not `Without<Dead>`, so a victim of the same shot is
  excluded deterministically. Anyone porting this to cops in T11 must keep that.
- **Cop fixture fidelity.** The bare fixture has no body. T11 cops are Tnua characters whose `Rotation` follows
  movement; `cop_sees` reads `Rotation` either way. Tnua keeps the yaw when `desired_forward` is `None`, so a standing
  cop's facing is stable.
- **Test fragility around civilians.** The witness tests depend on the utility scorer's worked rows and on slot
  timing (≤ 4 ticks). Reuse the `civilians.rs` worked setups exactly and assert them as `GATE BROKEN` preconditions.
  Gang test 7 (aiming a gang member by intent) is the likeliest to be fiddly, and its fallback is named.
- **Literal churn.** Adding fields to `ShotFired`, `MeleeHit`, `Threat` and `Report` touches 2 crates and ~12 sites
  (§3.10). All are compiler-driven. `cargo build` for the client (`--features dev,debug` too) must stay green, because
  `character_gate.rs` is test-only in the bin crate: run `cargo test -p gta_like --bin gta_like --no-run`.
- **Phantom red from a shared target** (TASK-009 lesson): `touch crates/*/src/lib.rs` before trusting a red in
  untouched code.
- **Memory pressure**: `-j 4`, one build at a time. The city-based QA is release, so build it once.

---

## 5. Open questions (для владельца; у каждого есть дефолт, который план уже реализует)

**Q1. Heat ниже первой звезды (например, одна перестрелка = 10).**
- A (дефолт): сгорает по правилу 1-й звезды. Вне круга 40 м и не замечен 10 с → 0. Мелкие нарушения не копятся
  минутами.
- B: копится до смерти или ареста. Через несколько мелких эпизодов внезапно появляется звезда.

**Q2. Новые ручки без числа в GDD.** `shooting_merge_seconds` = 5 с. Выстрелы с паузами меньше 5 с считаются одной
"стрельбой" (10 heat), сколько бы звонков о ней ни было. `incident_memory_seconds` = 60 с: труп, найденный позже, уже
не даёт heat. Оба числа в `wanted.ron`. Дефолт: оставить, крутить после прогона.

**Q3. Повторные звонки об одном трупе (поведение, не heat).**
- A (дефолт, этот слайс): heat считается один раз, свидетель может "звонить" снова, полоса над ним видна.
- B: отдельная задача. Мирный помнит, о чём уже звонил, и не звонит повторно (память на мирного).

---

Sources: [GTA IV wanted level (gta.wiki)](https://gta.wiki/w/Wanted_Level_in_GTA_IV),
[GTA IV wanted level (Fandom)](https://gta.fandom.com/wiki/Wanted_Level_in_GTA_IV),
[GTA V wanted level (Fandom)](https://gta.fandom.com/wiki/Wanted_Level_in_GTA_V),
[Crime Witness mod (gta5-mods)](https://www.gta5-mods.com/scripts/crime-witness),
[Grand Theft Wiki: GTA IV era](https://www.grandtheftwiki.com/Wanted_Level_in_GTA_IV_Era).

children: 0 launched / 0 reported.
