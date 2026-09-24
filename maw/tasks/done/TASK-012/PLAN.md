# PLAN — TASK-012 (GDD T11): police on foot and arrest

Cost of error: **mixed**. The silent class gets gates with worked numbers: the unit count running over the table, an
arrest that never fires or fires early, a busted player who keeps the guns or the wanted level, cop crimes without
heat, cops chasing the player's real position instead of `LastKnownPosition`, a gang regression from the shared
fire-line refactor, the corridor starvation (TASK-010 QA Bug 1), and the frame budget with 12 cops. The owner class
gets the owner checklist, BRP screenshots and one presentation gate: how cops look, how the arrest reads, whether 1.5 s
feels fair, BUSTED screen colour.

Pinned (`Cargo.lock`, sources under `~/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/`): bevy / bevy_ecs /
bevy_state / bevy_time 0.19.1, avian3d 0.7.0, bevy-tnua 0.32.0, pathfinding 4.16.0, rand_chacha 0.10.0, ron 0.12.2.
**No new crate, no Cargo.toml change.** `citygen` is not changed; only its public `sidewalk_anchor` and
`BuildingKind::PoliceStation` are used.

Build and test with `CARGO_TARGET_DIR=D:/test-gta-like/target -j 4`, one cargo command at a time (host under memory
pressure), in place.

---

## 1. Understanding (verified in code)

### Wanted core (TASK-011 / TASK-024)
- `wanted/mod.rs`: `WantedLevel { heat, stars, last_known, seen, hidden }` (Reflect resource), `stars_for`,
  `WantedConfig` (`wanted.ron`), `HeatTable { punch_civilian, shooting_near_people, wound_civilian, kill_person }`
  (`:21-40`). `WantedSystems` = `after(AiSystems::Decide).in_set(PlayingSystems)`, chain
  `record_crimes → take_calls → track_search → forget_crimes` (`:175-194`). `OnEnter(Wasted)` → `reset_wanted`
  (wanted default + `crimes.clear()`), `OnExit(Wasted)` → `drop_queued_calls`.
- `wanted/crimes.rs`: `Crime { Punch, Shooting, Wound, Kill }`, `classify` (`:141-149`) — a hit on anyone who is not a
  `Civilian` or `GangMember` is `Victim::Other` → no crime. `record_crimes` (`:151-246`): the cop witness is any
  `Faction::Police` + `Position` without `Dead` (`:234-237`) within 50 m LOS. "Near people" = `Civilian` or
  `GangMember` (`:164`). `resolve(Body(v))` matches only `Crime::Kill` (`:118`).
- `wanted/search.rs`: `eye`, `in_view`, `cop_sees` (cone 110°, 35 m, LOS), `witnesses` (50 m, LOS) are `pub(crate)`
  but the module `search` is private (`mod search;`), so police cannot reach them yet. `track_search` (`:97-132`) sets
  `seen` from every live `Faction::Police` with `Position + Rotation`. `search_step` keeps `last_known` at the player
  while seen, freezes it when unseen. **Real cops become witnesses and spotters with no wanted-side change** (hand-off
  of TASK-011 PLAN_FINAL §6).
- Heat of cop crimes (GDD §6.4: punch 45, wound 80, kill 150, "всегда") is **not** implemented: T11 owns it.

### Gang combat stack (TASK-010) the police reuse
- `gang/behavior.rs`: `Ctx` (`:121-127`), `Seek`/`Motion` (`:129-142`), `head_for` (`:144-180`, route or direct seek;
  writes `member.avoid`), `walk` (`:182-190`), `pull` (`:192-207`), `gang_fsm` (`:209-608`), `select` (`:610-619`),
  `gang_death` (`:621-665`). The hold-fire block (`:443-495`) builds shields (bodies the gang spares), the `yielding`
  flags (within `melee_distance.1` of the target), the index tie-break against `shooters`, and calls `unblock`.
  The motion is applied at `:599-606`.
- `gang/behavior/fire_line.rs` (191 lines): `FireLine::of(c: &GangCombatConfig, …)` (uses `aim_error_deg`),
  `blocked`/`blockers`, `Shooter { gang: u8, … }`, `Blocked`, `usable`, `spots` (uses `reposition_offsets`,
  `reposition_step`), `clear_spot`, `pinned`, `unblock` (`:161-191`, uses `chase_gait`, `reposition_gait`).
- **Corridor bug** (`maw/tasks/done/TASK-010/QA_REPORT.md` Bug 1): `unblock` with no usable candidate spot and not
  `pinned` returns `Motion::Seek(target, direct)` (`:175-186`); `head_for` avoids only World geometry, so the rear
  member walks into the non-pinned front member standing in `Hold` and never fires (2.4 / 3.0 m corridor, 0 shots in
  30 s). Every candidate spot lies in a wall (offsets 1.5/3/4.5 m) or still on the blocked line (back/forward step).
- `gang::Faction { Player, Gang(u8), Police }`, `GangConfig::hostile/spares`. The matrix has gang–police pairs off
  (`gangs.ron:12-13`). `apply_strikes` spares by gang faction only (`combat/melee.rs:497-520`); cops will not punch.

### Flow, world, population, perception
- `flow/mod.rs`: `GameState { Loading, Playing, Wasted }`, `WastedPhase { SlowMo, Screen }`, `PlayingSystems`,
  `NpcSystems` = Playing or Wasted (`:53-57`; its doc demands a new pausing state joins it), `WastedSystems` (Update).
  `flow/wasted.rs`: `RespawnConfig { wasted_time_scale, wasted_slowmo, wasted_screen }`, `advance_wasted` on
  `Time<Real>`, `respawn_player` (`:97-123`) at `HospitalSpawn`, `drop_queued_damage`, `drop_queued_input`.
- Other `OnExit(Wasted)` hooks: `combat::melee::reset_player_melee` (`combat/mod.rs:71`), `drop_queued_calls`
  (wanted), client `camera::reset_pivot`, `hud::wasted::restore_saturation`; `OnEnter(Wasted)`: `hud::desaturate`,
  `input::release_held_actions`.
- `character/mod.rs::drive_characters` (`:154`) zeroes motion only for `Dead` or an active `HitReaction`; the client
  writes `MoveIntent` every frame in every state (`src/input/mod.rs:136-158`).
- `world/`: `HospitalSpawn` from `citygen::sidewalk_anchor(layout, params, idx, margin)` (`world/city.rs:121-142`);
  the city always has `pois.police_stations` (1 in `city.ron`, `BuildingKind::PoliceStation`), no spawn point for it.
- `population/`: `spawn_points` / `SpawnPoint` are private (`population/mod.rs:396-426`); `outside_cone`, `occluded`,
  `OCCLUSION_RAYS_PER_POINT`, `PopulationLoad`, `CameraView`, `Offscreen`, `corpse_components`, `Appearance` are
  public. Spawners return early while `CameraView.0` is `None` (headless tests without a view spawn nothing).
  RNG streams: `CombatRng` 0, `NpcRng` 1, `GangRng` 2 (lesson: a new role takes its own stream).
- `perception/`: `AiSystems::{Perceive, Decide}` then `PopulationSystems`, chained, in `NpcSystems`, after
  `HealthSystems::Death`; `Decide` before `TnuaUserControlsSystems`. `sight_blocked` casts World-only rays.
- Client: `hud/wasted.rs` (the death screen widget), `visuals/character.rs::body_look` (model key + tint by civilian
  / gang), `CharacterAnimations` builds one graph per model from its own GLB (TASK-009 lesson), held guns follow any
  `Loadout`. `visual.ron` has `civilian_models`, `gang_models`; `female-e` and `female-f` are unused.
- Tests: `tests/common/mod.rs` (production composition, `FixedTimesteps(1)`, helpers), `gang_fire_lines.rs` (layout
  runner with walls), `wanted_support/mod.rs` (`spawn_cop` fixture: `Faction::Police` + `Transform` + `Position` +
  `Rotation`, no body). `tests/config.rs` is already 775 lines (over the 750 warning).
- Under `FixedTimesteps(1)` `Time<Real>` also advances one timestep per update (`bevy_time-0.19.1/src/lib.rs:181-183`),
  so Busted timers are deterministic in tests (the Wasted gate relies on it: 288 updates for 4.5 s).

---

## 2. Approach

1. **Shared NPC gunfight layer** `crates/gta_sim/src/tactics/` (no plugin, helpers only, like `layers.rs`):
   `git mv gang/behavior/fire_line.rs tactics/fire_line.rs` and move `Ctx`, `Seek`, `Motion`, `head_for`, `walk`,
   `select` plus the hold-fire block and the motion application out of `gang/behavior.rs`. Role differences go through
   one borrowed view `Discipline` built by each role from its own config (no RON schema change for gangs). Gang
   behaviour must stay bit-for-bit the same: the existing gang gates are the regression net.
2. **Corridor fix in the shared fallback**: before closing in, try a *queue slot* beside the nearest non-yielding
   blocker, shoulder to shoulder, `radius + clearance` (0.3 + 0.5 = 0.8 m) to its side. Worked geometry for both
   roles, both corridor widths and three weapon orders in `scratch/corridor_geometry.txt`: the rear line clears
   (perp 0.797 > limit 0.51), the front line is not blocked (along 0), the walk passes the blocker at 0.784 m > 0.6 m,
   and the body fits a 2.4 m corridor. Only the fallback changes, so every layout that finds a candidate spot today
   behaves as before. Precedent: tactical position selection = generate candidates around a reference, filter by line
   of fire and claims (Game AI Pro ch. 26, Jack; Killzone's position picking drops spots claimed by others, Straatman).
3. **Police domain** `crates/gta_sim/src/police/` with `PolicePlugin`, data `assets/police/escalation.ron` (GDD §12):
   - `PoliceDispatcher` (resource + systems): target count per star row, SWAT share, reinforcement delay after a loss,
     off-frame spawn on the sidewalk points of a police ring (reusing the population machinery), spawn order by
     distance to `last_known`, or by bearing spread on "surround" rows; stand-down and despawn at 0 stars.
   - Cop FSM as a pure function (GDD §6.5 style) over `CopState { Respond, Arrest, Attack, Search, Leave, Dead }`.
     `Leave` is the stand-down the bubble needs (0 stars: holster, walk off, despawn off-frame); no new mechanic.
     Respond/Search always head for `WantedLevel.last_known` (shared radio knowledge: any cop seeing the player keeps
     it on the player), so "copы идут к LastKnownPosition" holds by construction and is gated.
   - Arrest (GDD §6.4): a 1-star row cop in `Arrest` walks up to 1.0 m; while it is within 1.5 m and the player does
     not attack, a hold runs; 1.5 s → `GameState::Busted`; a knocked-down player is taken at once. Breaking free =
     getting farther than 3 m from the arresting cop while the attempt is active → heat raised to the next star
     threshold (+1 star), which turns the cops to `Attack` (row 2 does not arrest). This matches GTA IV: resisting
     arrest by running or fighting "immediately sparks a two-star wanted level" (gta.wiki, Wanted Level in GTA IV).
   - Heat rows for cop crimes, reported always (the victim is the witness).
4. **Busted flow** in `flow/` mirroring Wasted: `GameState::Busted` + `BustedPhase { Arrest 2 s, Screen 3 s }` on
   `Time<Real>` (bevy-ecs invariant: Busted timers are real time), the player held by a `Cuffed` marker, respawn at
   the police station, weapons confiscated (`Loadout::default()`), wanted and crimes reset **on exit** (so the
   arresting cop stays on the kneeling player during the arrest phase; decision logged). "Анимация ареста 2 с" is the
   `Arrest` phase; the AC "пассивен 1.5 с → Busted" holds literally.
5. **Client**: police/SWAT looks (models + tints in `visual.ron`), the kneel pose (`crouch` clip) while `Cuffed`, the
   BUSTED screen as the death-screen widget with its own text/colour, desaturation, input release and camera pivot
   reset on Busted.
6. **Navmesh decision by evidence**: keep graph + A* + direct seek (GDD §6.6); a seed-1 city chase gate and the t11
   progress log decide. Navmesh only after a reproduced stuck chase, as its own task.
7. **Q2=B stays off** (recommended default, Open questions): cops target only the player.

Buffered signals: no new `Message` type. Police systems read the existing `ShotFired`, `MeleeHit`, `DamageDealt`
buffers with their own `MessageReader`s (independent cursors). No observers.

---

## 3. Steps

### Step 1 — data: `assets/police/escalation.ron` (new)

```ron
(
    // Wanted stars 1..5 (GDD §6.4): foot units alive at most, of them SWAT; seconds before a lost unit is
    // replaced; whether cops try to arrest instead of shooting; whether new units come in from other sides.
    stars: (
        (units: 2,  swat: 0,  reinforce_seconds: 20.0, arrest: true,  surround: false),
        (units: 4,  swat: 0,  reinforce_seconds: 15.0, arrest: false, surround: false),
        (units: 6,  swat: 0,  reinforce_seconds: 10.0, arrest: false, surround: true),
        (units: 8,  swat: 4,  reinforce_seconds: 8.0,  arrest: false, surround: true),
        (units: 12, swat: 12, reinforce_seconds: 3.0,  arrest: false, surround: true),
    ),
    // Gear and distance band per unit kind; SWAT keeps a closer band ("штурм").
    patrol: (gun: Pistol, armor: 0.0,  reserve: 120, keep_distance: (8.0, 18.0)),
    swat:   (gun: Smg,    armor: 50.0, reserve: 300, keep_distance: (5.0, 12.0)),
    spawn_ring: (40.0, 90.0),   // m from the player, sidewalk points off-frame or behind buildings
    spawns_per_tick: 1,
    arrest: (
        distance: 1.5,            // m: a cop this close holds the player (GDD §6.4)
        stand_distance: 1.0,      // m: an arresting cop stops here
        seconds: 1.5,             // s the player stays passive within `distance` before BUSTED
        break_free_distance: 3.0, // m from the arresting cop: the player broke free, +1 star
        hostile_seconds: 5.0,     // s a witnessed attack by the player makes 1-star cops shoot
    ),
    search_arrive_distance: 3.0,  // m: a cop this close to its goal (last known position, search point) arrives
    combat: (
        aim_error_deg: 2.0,           // police aim better than gangs (4.0)
        trigger_seconds: (0.4, 0.9),
        fire_line_margin: 0.2,
        pressed_distance: 2.5,        // a spared body this close to the target yields it (gang: melee_distance.1)
        reposition_offsets: [1.5, 3.0, 4.5],
        reposition_step: 2.0,
        chase_gait: Run,
        reposition_gait: Walk,
        search_gait: Walk,
        leave_gait: Walk,
    ),
)
```
Tuple syntax for `stars` (ron 0.12.2 reads `[T; 5]` only from a tuple: TASK-011 probe). All numbers are starting data
for the owner run.

Also:
- `assets/wanted/wanted.ron` `heat`: add `punch_cop: 45, wound_cop: 80, kill_cop: 150` (GDD §6.4), update the comment
  ("Cop rows come with T11" → cop rows are reported always).
- `assets/flow/respawn.ron`: add `busted_arrest: 2.0, busted_screen: 3.0` (GDD §3.4, §6.4).
- `assets/ui/strings.ron`: add `busted: "BUSTED"`, `busted_color: (0.20, 0.45, 1.0)` next to `wasted*` (the widget
  reuses `wasted_size`, `wasted_backdrop`, `wasted_saturation`).
- `assets/character/visual.ron`: `police_models: ["third_party/mini-characters/character-male-c.glb",
  "third_party/mini-characters/character-female-e.glb", "third_party/mini-characters/character-female-f.glb"]`,
  `police_tint: (0.25, 0.40, 1.0)`, `swat_tint: (0.22, 0.22, 0.30)` with a one-line comment (blue patrol, dark SWAT).
  All three GLBs are in the manifest rig list (`manifest.ron:75-78`).

### Step 2 — shared gunfight layer `crates/gta_sim/src/tactics/`

- `git mv crates/gta_sim/src/gang/behavior/fire_line.rs crates/gta_sim/src/tactics/fire_line.rs` (keeps history),
  `lib.rs`: `pub(crate) mod tactics;`.
- `tactics/mod.rs` (new, ~200 lines), moved from `gang/behavior.rs` with visibility `pub(crate)`:
  `Ctx`, `Seek`, `Motion`, `head_for`, `walk`, `select`. `head_for` takes `avoid: &mut f32` instead of
  `&mut GangMember` (only `member.avoid` was used).
- `pub(crate) struct Discipline<'a> { aim_error_deg, fire_line_margin, pressed_distance, reposition_offsets: &'a [f32],
  reposition_step, reposition_gait: Gait, chase_gait: Gait }`. `GangCombatConfig::discipline()` fills it with
  `pressed_distance = melee_distance.1` (exactly today's yielding radius).
- `fire_line.rs`: `FireLine::of(aim_error_deg, weapons, gun, loadout, clearance)`; `spots(b, d)`, `pinned(b, d)`,
  `unblock(ctx, b, plan, on_slot, d, radius, rays)` read the `Discipline`; `Shooter.gang: u8` → `faction: Faction`.
- `pub(crate) fn hold_fire(...)` = the block `gang/behavior.rs:453-495` verbatim, generalised: inputs `me`, own
  `Faction`, chest, target entity and chest, `FireLine`, `living: &[(Entity, Vec3, Option<Faction>)]`,
  `shooters: &[Shooter]`, `spares: impl Fn(Option<Faction>) -> bool`, `plan`, `on_slot`, `&Discipline`, `radius`,
  `&mut rays`; returns `(line_blocked, kept_spot, clearing: Option<Motion>)`. The tie-break predicate
  `cfg.spares(Some(gang), Some(Faction::Gang(s.gang)))` becomes `spares(Some(s.faction))`; `yielding` uses
  `d.pressed_distance`.
- `pub(crate) fn apply_motion(ctx, motion, chest, on_slot, avoid, route, load, intent)` = `gang/behavior.rs:599-606`.
- `gang/behavior.rs`: delete the moved items, import from `crate::tactics`, build `let d = c.discipline();` once,
  replace the hold-fire block and the motion match by the calls; `pull`, the FSM and `gang_death` stay. The file
  shrinks from 665 lines.
- Verification of zero gang change: all of `gang_fire_lines` (incl. the exact tie-break gate), `gang_combat`, `gangs`,
  `gang_city` stay green **before** Step 3 lands (run them after this step alone).

### Step 3 — corridor fix (both roles), `tactics/fire_line.rs`

Add, one-line doc each:
```rust
/// Beside the nearest blocker that is not pressed against the target, shoulder to shoulder with it: in a
/// passage too narrow for the candidate spots the rear shooter queues up level with the front one.
fn queue_slot(spatial: &SpatialQuery, b: &Blocked, radius: f32, rays: &mut u32) -> Option<Vec3>
```
- Blockers = `b.line.blockers(b.chest, b.to, b.shields)` with `!b.yielding[k]`; take the one nearest to `b.chest`
  (flat distance; tie → lower index).
- `ahead = (b.to − p).with_y(0).normalize_or_zero()`, `side = Vec3::new(−ahead.z, 0, ahead.x)` (same convention as
  `spots`), offset `radius + b.line.clearance` (0.3 + 0.5 = 0.8 m; derived from the body radius and the existing
  margin, no new number). Candidates `p + side·o`, `p − side·o`, the one nearer to `b.chest` first, tie → `+side`.
  First that passes `usable(spatial, b, slot, radius, rays)` wins (same line, bump and wall-ray test as every spot).
- `unblock`: in the `plan == None` branch, after `pinned` returns `(None, None)`, try
  `queue_slot` → `Some(slot)` returns `(Some(slot), Some(Motion::Yaw(steer(b.chest, slot), d.reposition_gait)))`
  (kept as the plan and re-checked on the slot like any spot); only when it is `None` the old close-in seek remains.
- Update the `unblock` doc comment (one sentence) to name the queue slot.

### Step 4 — wanted: cop crimes and re-exports

- `wanted/mod.rs`: `HeatTable` gains `punch_cop`, `wound_cop`, `kill_cop` (`///` "reported always: the cop is the
  witness"); `of()` maps the new variants; `validate` loops the three new fields (> 0). Re-export for police:
  `pub(crate) use search::{cop_sees, eye, witnesses};`.
- `wanted/crimes.rs`: `Crime` gains `PunchCop, WoundCop, KillCop`; `Victim::Cop`; `classify`: `(Cop, true) → KillCop`,
  `(Cop, false) if melee → PunchCop`, `(Cop, false) → WoundCop`. `kinds` query becomes
  `(Has<Civilian>, Has<GangMember>, Has<PoliceUnit>)` (civilian first, then gang, then cop); `persons` filter adds
  `With<PoliceUnit>` (a cop is a person for "shooting near people"; fixtures without `Health` stay out).
  In step 4 of `record_crimes`, ids of cop crimes are reported **regardless** of `witnessed` (keep a separate
  `always: Vec<IncidentId>`, report them before the witness early-return). `resolve(Body(v))` accepts
  `Kill | KillCop`. Test constant `HEAT` gains the three fields; `classify_table` gains 3 cop rows (each own case).
- `WantedPlugin`: add `.add_systems(OnExit(GameState::Busted), (reset_wanted, drop_queued_calls))`.

### Step 5 — flow: Busted

- `flow/mod.rs`: `GameState::Busted` (doc: arrested; the arrest scene, the "BUSTED" screen, respawn at the police
  station without weapons, GDD §3.4). `#[derive(SubStates …)] #[source(GameState = GameState::Busted)] pub enum
  BustedPhase { #[default] Arrest, Screen }`, `BustedSystems` (Update, `in_state(Busted)`), register types, init
  `BustedClock`. `NpcSystems` condition: `in_state(Playing).or_else(in_state(Wasted)).or_else(in_state(Busted))`;
  update its doc. Registrations: `OnEnter(Busted)` → `busted::enter_busted`; `Update` → `busted::advance_busted`
  in `BustedSystems`; `OnExit(Busted)` → `(busted::respawn_at_station, wasted::drop_queued_damage,
  wasted::drop_queued_input)`.
- `flow/busted.rs` (new, ~110 lines): `BustedClock(f32)`; `enter_busted` (clock 0, insert `Cuffed` on the player,
  `ActionIntent::default()`); `advance_busted` (`Time<Real>`, `Arrest` → `Screen` at `busted_arrest`, `Playing` at
  `busted_arrest + busted_screen`, same shape as `advance_wasted`); `respawn_at_station`: teleport to
  `PoliceStationSpawn.point + Y·float_height`, full health, `*loadout = Loadout::default()` (all guns, ammo and the bat
  confiscated, GDD §3.4), `try_remove::<Cuffed>()`. Extract the teleport/heal body of `wasted::respawn_player` into
  one shared `fn respawn_at(...)` used by both (no copy).
- `RespawnConfig` gains `busted_arrest`, `busted_screen` (finite, ≥ 0, like `wasted_screen`).
- `character/mod.rs`: `#[derive(Component, Reflect, Default)] pub struct Cuffed;` ("held in place without control:
  the arrested player") registered; `drive_characters` adds `Has<Cuffed>` and treats it like `dead`.
- `combat/mod.rs`: `melee::reset_player_melee` also on `OnExit(GameState::Busted)`.

### Step 6 — world: police station spawn

- `world/mod.rs`: `#[derive(Resource, Reflect, Clone, Copy, Debug)] #[reflect(Resource)] pub struct
  PoliceStationSpawn { pub point: Vec3, pub along: Vec3 }` ("read by QA over BRP"), inserted in `WorldPlugin::build`
  with the test-area fixture `test_area::STATION_SPAWN = Vec3::new(-20.0, 0.0, 20.0)` (clear floor, far from the
  hospital default at the origin, so "respawned at the station" is falsifiable on the test floor).
- `world/city.rs`: `station_spawn(layout, params, curb, margin)` = `hospital_spawn` for the first
  `BuildingKind::PoliceStation` (with `police_stations: 2` the first one; noted in Risks), same margin
  (`health.pickups.spacing`, one-line comment "same clearance from the block corners as the hospital spawn"); insert
  like the hospital, error exits like the hospital.

### Step 7 — population: share the spawn points

`population/mod.rs`: `SpawnPoint` and `spawn_points` become `pub(crate)` (fields too). Nothing else.

### Step 8 — police domain `crates/gta_sim/src/police/`

**`mod.rs`** (~320 lines): `pub const POLICE_CONFIG = "police/escalation.ron"`. Config structs, all
`#[serde(deny_unknown_fields)]`: `EscalationConfig { stars: [EscalationRow; STARS], patrol: UnitSpec, swat: UnitSpec,
spawn_ring: (f32, f32), spawns_per_tick: u32, arrest: ArrestConfig, search_arrive_distance: f32, combat:
PoliceCombatConfig }`, `EscalationRow { units, swat: u32, reinforce_seconds: f32, arrest, surround: bool }`,
`UnitSpec { gun: Weapon, armor: f32, reserve: u32, keep_distance: (f32, f32) }`, `ArrestConfig { distance,
stand_distance, seconds, break_free_distance, hostile_seconds }`, `PoliceCombatConfig { aim_error_deg,
trigger_seconds, fire_line_margin, pressed_distance, reposition_offsets, reposition_step, chase_gait, reposition_gait,
search_gait, leave_gait }` + `discipline()`.
`validate()` (errors name the field, `stars[i].…`): `swat <= units`; `units` and `swat` non-decreasing over rows;
`units[0] >= 1`; `reinforce_seconds` finite ≥ 0; `armor` finite in [0, ∞); keep bands `0 < lo < hi`;
`spawn_ring` `0 < inner < outer`; `spawns_per_tick >= 1`; arrest `0 < stand_distance < distance <
break_free_distance`, `seconds > 0`, `hostile_seconds >= 0`; `search_arrive_distance > 0`; combat like
`GangCombatConfig::validate_combat` (aim error in [0, 90), trigger `0 < lo <= hi`, margin ≥ 0, offsets non-empty
finite > 0, step > 0, `pressed_distance > 0`). `validate_ring(despawn_distance)`: `spawn_ring.1 < despawn_distance`
(cross-config, called from `compose_sim` like `validate_fight_hearing`).
Components/resources:
- `enum UnitKind { Patrol, Swat }` (Reflect), `enum CopState { Respond, Arrest, Attack, Search, Leave, Dead }`.
- `#[require(Character, Perception, Offscreen, Route)] pub struct PoliceUnit { kind, state, sees: bool,
  dest: Option<Vec3>, dest_clear: bool, avoid: f32, trigger_left: f32, reposition: Option<Vec3> }` (Reflect, `///`
  per field; `dest` = where it is heading this tick, for gates and QA).
- `PoliceRng` = `ChaCha8Rng::seed_from_u64(seed)` + `set_stream(3)` (own stream, TASK-010 lesson), `unit()`.
- `PoliceDispatcher { units: u32, swat: u32, reinforce_left: f32 }` (Reflect resource: active counts for QA).
- `ArrestAttempt { cop: Option<Entity>, hold: f32 }`, `PoliceAlert { hostile_left: f32 }` (Reflect resources).
- `pub fn police_unit_bundle(loco, handle, health, weapons, spec: &UnitSpec, kind, feet, facing_yaw, appearance)`:
  `PoliceUnit { state: Respond, … }`, `Faction::Police`, `Name`, `Transform` (feet + float_height, yaw),
  `character_components`, `Health { armor: spec.armor, ..full }`, `Loadout` with `acquire(gun)` then
  `reserve = spec.reserve.min(stats.max_reserve)`, `appearance`.
- `PoliceSystems` set. `PolicePlugin { seed }` registers types, resources, and:
  - `FixedUpdate`: `behavior::police_death.in_set(HealthSystems::Death).in_set(NpcSystems)`;
    `behavior::police_alert.in_set(AiSystems::Perceive)`; `behavior::police_fsm.in_set(AiSystems::Decide)`;
    `(dispatch::despawn_police, dispatch::dispatch_police.in_set(PlayingSystems)).chain().in_set(PoliceSystems)`
    with `configure_sets(FixedUpdate, PoliceSystems.after(PopulationSystems).in_set(NpcSystems))`: `spawn_civilians`
    resets the shared `PopulationLoad.rays` budget at its start (`population/mod.rs:447`), so the dispatcher must run
    after it to share one budget per tick; `arrest::arrest_player.after(AiSystems::Decide).before(WantedSystems)
    .in_set(PlayingSystems)`. All flat tuples, no nested `.chain()` (TASK-008 lesson).
  - `OnEnter(Wasted)` and `OnExit(Busted)` → `arrest::reset_arrest` (`ArrestAttempt` and `PoliceAlert` default).
- `compose_sim`: load + validate + `validate_ring(population.despawn_distance)`, insert, add
  `PolicePlugin { seed: combat_seed }` **before** `WantedPlugin` (keep `WantedPlugin` last).

**`fsm.rs`** (~260 lines incl. tests), pure:
- `CopSenses { sees, hostile, arrest_row: bool, stars: u8, at_goal: bool }`;
  `next_state(state, &s) -> CopState`:
  `Dead → Dead`; `stars == 0 → Leave`; `Leave → Respond`;
  `Respond | Search`: `sees` → `if arrest_row && !hostile { Arrest } else { Attack }`; else `Respond && at_goal →
  Search`; else unchanged;
  `Arrest`: `!(arrest_row && !hostile) → Attack`; `!sees → Respond`; else `Arrest`;
  `Attack`: `!sees → Respond`; `arrest_row && !hostile → Arrest`; else `Attack`.
- `spawn_kind(row, units, swat) -> Option<UnitKind>`: `None` if `units >= row.units`; `Swat` while `swat < row.swat`;
  else `Patrol`.
- `break_free_heat(heat, rows) -> u32`: `stars = stars_for(heat)`; `stars in 1..5` → `max(heat, rows[stars].heat)`;
  else `heat`.
- `pick_spawn(candidates: &[Vec3], centre, taken: &[Vec3], surround) -> Option<usize>`: no surround or no taken →
  nearest to `centre` (flat; tie → lower index); surround → maximise the smallest flat bearing difference (around
  `centre`) to `taken`, tie → nearer.
- `arrest_step(attempt_hold, distance, attacking, knocked_down, dt, cfg) -> ArrestStep { Hold(f32), BrokeFree,
  Busted }`: knocked down within `distance` → `Busted`; attacking → `Hold(0.0)`; `distance <= cfg.distance` →
  `hold + dt`, `>= seconds` → `Busted`; `distance > break_free_distance` → `BrokeFree`; else `Hold(hold)` (paused).

**`behavior.rs`** (~480 lines):
- `police_alert`: `dt` decay of `hostile_left`; set to `arrest.hostile_seconds` when a player `ShotFired` or player
  `MeleeHit` is witnessed by a live `PoliceUnit` (`witnesses(spatial, eye(cop), eye(player), wanted_cfg)`) or a player
  `DamageDealt` targets a `PoliceUnit`.
- `police_fsm` (params grouped in tuples like `gang_fsm`): per unit, skip `Dead`:
  1. On its AI slot (`Perception.slot == tick % slots`, like `gang_fsm`; a new unit waits ≤ `slots` ticks):
     `sees = player alive && cop_sees(spatial, eye(chest),
     rotation·(−Z), eye(player), wanted_cfg)`; `dest_clear = dest.is_some_and(|d| !sight_blocked(eyes, d + Y·fh))`.
  2. `row = stars ≥ 1 → esc.stars[stars−1]`; `goal` = Respond: `wanted.last_known`; Search: kept search point.
     `at_goal = goal.is_some_and(|g| flat_distance(feet, g) <= search_arrive_distance)`.
  3. `state = next_state(..)`; entering `Attack` rolls `trigger_left` (`PoliceRng`, `combat.trigger_seconds`).
  4. Search point: when entering `Search` or arriving at the current point, pick uniformly (`PoliceRng`) among
     `spawn_points(graph, population.spawn_point_spacing)` within `WantedConfig.stars[stars−1].search_radius` of
     `last_known`; none → `last_known + polar(radius·√u, 2πv)`.
  5. Intents per state (aim from the eyes like `gang_fsm`): **Respond** gun drawn, not aiming, `Seek(last_known,
     chase_gait, direct = dest_clear && d <= nav.direct_seek_distance)`; **Arrest** aim at the player chest, gun drawn,
     `Seek(player, chase_gait, direct = sees && d <= direct_seek_distance)` while `d > stand_distance`, else `Stand`,
     never pulls; **Attack** aim, `hold_fire` with `spares = |f| f != Some(Faction::Player)` (Q2=A) and the police
     `shooters` list, pull when `trigger_left == 0 && sees && in_range && held == gun && !line_blocked`, motion =
     clearing or `band_move(d, spec.keep_distance)` (`gang::fsm::band_move`, make it `pub(crate)` if needed);
     **Search** gun drawn, not aiming, `Seek(point, search_gait, dest_clear)`; **Leave** holster, not aiming,
     `Yaw(steer(player, chest), leave_gait)` (walks away), no player → `Stand`.
  6. `dest` written every tick (Respond: `last_known`; Arrest/Attack: player chest; Search: point; Leave: `None`).
  7. `apply_motion` (shared). A 6-line `pull` of its own (roll from `PoliceRng`, error cone `aim_error_deg`).
- `police_death` = `gang_death` for `PoliceUnit` (state `Dead`, zero intents, drop the gun with `dropped_gun`,
  `corpse_components`).

**`dispatch.rs`** (~220 lines):
- `despawn_police` (NpcSystems): off-frame ageing like `despawn_far_gangs`; despawn when `offscreen >=
  despawn_offscreen_seconds` and (`state == Leave` or flat distance > `despawn_distance`). Corpses stay to `age_corpses`.
- `dispatch_police` (PlayingSystems): `reinforce_left -= dt` (≥ 0); a unit with `Added<Dead>` → `reinforce_left =
  row.reinforce_seconds`; return when `stars == 0`, view `None`, `last_known` `None` or `reinforce_left > 0`.
  Up to `spawns_per_tick`: `kind = spawn_kind(row, active, swat)` (active = not `Dead`, not `Leave`); candidates =
  `spawn_points` within `spawn_ring` of the player, ≥ `population.spawn_min_separation` from every `Character` and
  from this tick's spawns, and hidden: `outside_cone(view, at, head_height, margin)` or `occluded(..)` within the
  shared `PopulationLoad.rays` budget (same rules as `spawn_gangs`); order by `pick_spawn(.., last_known, active
  positions, row.surround)`; spawn `police_unit_bundle` facing `last_known`, `Appearance(rng.next_u32())`.
  Write `PoliceDispatcher { units, swat, reinforce_left }`.

**`arrest.rs`** (~150 lines):
- `arrest_player` (PlayingSystems): player `(Entity, &Position, &HitReaction, &Melee)` without `Dead`; attempt cop
  valid only while it is a live `PoliceUnit` in `Arrest`, else cancel (hold 0, cop `None`). No attempt → the nearest
  `Arrest` cop within `arrest.distance` starts one. `attacking` = a player `ShotFired` this tick or `melee.swing`.
  `arrest_step` → `Busted`: `next.set(GameState::Busted)`; `BrokeFree`: `wanted.heat = break_free_heat(..)`, cancel;
  `Hold(h)`: store.
- `reset_arrest`.

### Step 9 — client

- `src/visuals/character_config.rs`: `police_models: Vec<String>` (non-empty, listed and present like
  `gang_models`), `police_tint`, `swat_tint`; `src/main.rs` preflight lists police models like gang models.
- `src/visuals/character.rs`: `CharacterAnimations` chains `police_models` after `gang_models` (update the `ModelKey`
  doc); `body_look` gains `police: Option<UnitKind>` → `(1 + C + G + a % P, police_tint | swat_tint)`; `LookQuery`
  adds `Option<&PoliceUnit>`. `drive_character_animation`: `Has<Cuffed>` shows the `Cower` action (the `crouch` clip,
  kneeling) like a cowering civilian.
- `src/visuals/civilian_gate.rs:66` model list (every model animates) chains `police_models`.
- `src/hud/wasted.rs`: `spawn_wasted_screen` becomes a generic `title_screen(text, color, exit: impl States)` builder
  used by `spawn_wasted_screen` and a new `spawn_busted_screen` (`DespawnOnExit(BustedPhase::Screen)`).
  `src/menu/config.rs`: `busted`, `busted_color` (+ validation like `wasted`, `wasted_color`).
- `src/hud/mod.rs`: `OnEnter(BustedPhase::Screen)` → busted screen, `OnEnter(Busted)` → `desaturate`,
  `OnExit(Busted)` → `restore_saturation`; plugin doc mentions BUSTED. `src/input/mod.rs`: `OnEnter(Busted)` →
  `release_held_actions`. `src/camera/mod.rs`: `OnExit(Busted)` → `reset_pivot`.
- `src/visuals/police_gate.rs` (new, `#[cfg(test)]`, modelled on `gang_gate.rs::every_gang_model_animates_from_its_own
  _clips` + the tint check): one patrol and one SWAT per police model through `police_unit_bundle`; each model's
  `leg-left` rotation range over every update ≥ the threshold the civilian gate uses (TASK-022 lesson: range over
  all updates, not two snapshots); body mesh base colour = source × `police_tint` / `swat_tint` (via the
  `StandardMaterialStandIn` handler, TASK-010 lesson). Run the client tests 3 times.

### Step 10 — runtime QA `tools/qa/scenarios/t11.py` (new, t9/t10 style, `--seed 1`, release, `dev`, `--out`)

1. Wait `Playing`, golden hash, chunks; stand the player on the hospital sidewalk; pick up nothing (the player gets
   a pistol by a named mutation of `Loadout` so confiscation is observable; t9 armour precedent).
2. **Busted run**: `world.mutate_resources` `WantedLevel.heat` → `stars[0].heat` (from `wanted.ron`). Poll
   `PoliceUnit` entities (≤ 2, assert ≤ `units` of row 1 on every poll), then a cop in `Arrest` within
   `arrest.distance`; the player stays passive (no keys). Poll `GameState` → `Busted`; during `BustedPhase::Screen`
   (after `busted_arrest` s) screenshot `busted.png` (spacing ≥ 0.15 s from any other capture). Poll `Playing`; read
   the player `Loadout` (no gun owned, `held` null, `has_bat` false), `Position` within 1 m (flat) of
   `PoliceStationSpawn.point`, `WantedLevel.heat == 0`.
   Record per cop the flat distance to the player every poll (stuck evidence: a cop with no progress for 5 s while
   not in `Arrest`/`Attack` is written to `summary.json` `stuck`, for the navmesh decision).
3. **SWAT run**: player armour 1e6 (named mutation), heat → `stars[3].heat`; poll until a `PoliceUnit` with kind
   `Swat` exists and `PoliceDispatcher.units <= 8`, `swat <= 4` on every poll; `t6.aim_at` the nearest SWAT chest
   when within 30 m, screenshot `swat.png`. Record `frame_report()` (FPS only through it: 30 Hz monitor lesson).
4. `log_errors` empty, `shutdown`. Every constant read from the RON files (t10 `ron_number` helper).

### Step 11 — config gates `crates/gta_sim/tests/config_police.rs` (new; `config.rs` is already 775 lines)

Move `sabotaged` from `tests/config.rs` into `tests/common/mod.rs` (`pub fn`, used by both). Gates, each flipped by its
sabotage (off-boundary values, TASK-007 lesson):
- shipped `escalation.ron` loads and validates; unknown field → error names `escalation.ron` and the field.
- `stars[3].swat` 4 → 9 (> units 8) → error names `stars[3].swat`.
- `stars[2].units` 6 → 3 (< row 2's 4) → error names `stars[2].units`.
- `arrest.break_free_distance` 3.0 → 1.2 (< distance 1.5) → error names `break_free_distance`.
- `spawn_ring: (40.0, 90.0)` → `(40.0, 160.0)` (> despawn 150) → cross-config error names `spawn_ring`.
- 4 rows instead of 5 → parse error contains `length 5`.
- `wanted.ron` `kill_cop: 150` → `0` → `heat.kill_cop must be > 0`; `respawn.ron` `busted_screen: 3.0` → `-1.0` →
  error names `busted_screen`.

### Step 12 — headless gates (production composition, test floor unless stated)

Shared fixtures `crates/gta_sim/tests/police_support/mod.rs`: `spawn_unit(app, kind, feet, yaw) -> Entity` through
`police_unit_bundle` (a posed fixture carries its `Transform`: the bundle does; assert the position after one tick —
TASK-011 lesson), `set_cop_state`, `cop(app, e) -> PoliceUnit`, `esc(app)`, `assert_shipped(app)` with `GATE BROKEN` on
every number the worked examples below assume (rows, arrest block, gaits, `run_speed 4.5`, `sprint_speed 6.8`, slots 4,
`despawn_offscreen_seconds 2`, heats 40/180/550/1200/2400).

**`tests/police_arrest.rs`**
- **P1 `passive_player_is_busted_and_disarmed`** (AC 1, correctness): `graph_app(10)`-style floor, player at origin
  holding a pistol (magazine 12, reserve 20) with `has_bat = true`; heat 40; cop (patrol) feet (0,0,−10) facing +Z.
  Liveness: cop state `Arrest` within 4 ticks (one slot). Record tick `s` when `ArrestAttempt.cop` becomes `Some`;
  the hold reaches 1.5 s after exactly 96 fixed ticks (1.5 × 64, dyadic, exact); `GameState::Busted` becomes visible
  on the update after the tick that set it (derive the exact offset from the schedule: `NextState` set in
  `FixedUpdate`, applied by `StateTransition` of the next update; assert it). Then `BustedPhase::Arrest` for 128
  updates and `Screen` for 192 (`Time<Real]` = 1/64 per update), `Playing` after (bounds like `run_wasted`, derive
  exact). After: `Loadout == Loadout::default()`, flat distance to `PoliceStationSpawn.point` < 0.05, `WantedLevel`
  default, `Crimes` empty, no `Cuffed`. Flips: remove the hold (Busted on the first in-range tick) → the 96-tick
  assert RED; remove the `Loadout` reset → RED.
- **P2 `cuffed_player_cannot_move`**: from P1 at `BustedPhase::Arrest`, set `MoveIntent.axis = Y`, 64 updates → flat
  displacement < 0.05 m. Flip: drop `Has<Cuffed>` from `drive_characters` → RED.
- **P3 `breaking_free_adds_a_star`** (AC 4, correctness): P1 setup until the hold is > 0; the player sprints away
  from the cop (cop at −Z, so `MoveIntent { axis: Y, yaw: π, gait: Sprint }` moves along +Z, GDD §3.2 example 3).
  Worked: gap grows at ≈ 6.8 − 4.5 = 2.3 m/s from ≤ 1.5 m to > 3.0 m in < 1 s (Tnua reaches speed in 0.15 s); assert
  within 128 ticks `heat == 180` exactly (from 40), next tick `stars == 2`, `ArrestAttempt.cop == None`, cop state
  `Attack` within 4 more ticks, and `GameState` never `Busted`. Flip: remove the `BrokeFree` branch → heat 40 → RED.
- **P4 `attacking_player_is_shot_not_arrested`** (1-star rule): P1 setup, armour 1e6, when the cop is in `Arrest` at
  ≈ 5 m, `shoot_into_the_air` (the cop within 15 m of the muzzle and 50 m LOS: heat 40 + 10 = 50, still 1 star).
  Assert cop `Attack` within 4 ticks and ≥ 1 cop `ShotFired` within 2 s, no `Busted`; after `hostile_seconds` (320
  ticks) without attacks the cop is back in `Arrest`. Flip: ignore `hostile` in `next_state` → RED.
- **P5 `knocked_down_player_is_taken_at_once`**: cop in `Arrest` within 1.5 m, named mutation player
  `HitReaction::KnockedDown { left: 1.2 }` → `Busted` on the next tick (no 96-tick hold). Flip: drop the knockdown
  branch → RED (hold still running after 10 ticks).
- **P6 `lost_player_is_searched_at_last_known`** (AC 3): heat 180, armour 1e6, cop feet (−12,0,0) facing +X, player at
  origin. Liveness: cop `Attack`, `wanted.seen`. Named mutation: `place_player` feet (0,0,22) (behind the test wall
  z≈14, x∈[−6,6]; the cop–player line crosses z = 14 at x ≈ −4.4, inside the wall). `L` = the player position before
  the teleport. Assert within 8 ticks: `seen == false`, `last_known` within 0.05 m of `L`, cop `Respond`; every tick
  after: `cop.dest == last_known`; the cop's flat distance to `L` reaches ≤ 3.0 within 5 s (12 m at 4.5 m/s) and the
  cop enters `Search`; every search point afterwards lies within 70 m (`stars[1].search_radius`) of `L`, and ≥ 2
  distinct points in 20 s (liveness). Flip: Respond heads for the player → `dest` assert RED (and the cop's path
  passes ≥ 10 m from `L`: line (−12,0)→(0,22) perp 10.5 m).
- **P7 `busted_drops_queued_damage`** and **P8 `busted_keeps_world_one_shot`** (city_app(1), `NextState::set(Busted)`
  named mutation, `world_counts` pattern of `respawn.rs`): damage queued during Busted does not hit the respawned player;
  city/player counts unchanged after Busted → Playing.

**`tests/police_dispatch.rs`** — fixture: player feet (−30,0,−30), graph edges A (30,0,−35)–(30,0,35) and
B (−35,0,35)–(25,0,35) (19 spawn points at 8 m spacing, all 60–88.5 m from the player, inside the 40–90 m ring, ≥ 4 m
apart), `chase_view(feet, (−1,0,−1).normalize())` (every point behind), `max_civilians = 0` (named mutation via
`set_population`), armour 1e6.
- **D1..D5 `units_follow_row_k`** (AC 2; one case per row, fresh app each): heat = `stars[k].heat`; every tick for
  320 ticks assert active ≤ `units_k` and SWAT ≤ `swat_k`; by tick 64 active == `units_k` and SWAT == `swat_k`
  (liveness: the table is reached, else "≤" is vacuous; 12 spawns at 1 per tick). Flip: `spawn_kind` compares
  `units > row.units` → one extra unit → RED.
- **D6 `spawns_stay_off_frame`**: view turned to face edge A (open floor, no occluder) → every spawned unit lies on B.
  Flip: skip the hidden check → a unit on A → RED.
- **D7 `lost_unit_is_replaced_after_reinforce_seconds`** (row 3, 10 s = 640 ticks): after 6 active, named mutation
  `Health.current = 0` on one → active 5 for exactly the derived tick count, then 6. Flip: no cooldown → RED.
- **D8 `cleared_wanted_sends_units_away`**: row 1 reached, heat → 0 → all units `Leave` within 2 ticks, all despawned
  within 130 ticks (off-frame since spawn). Flip: skip `Leave` despawn → RED.
- Unit tests in `police/fsm.rs`: `next_state_table` (every state × sees/hostile × one case per shipped row's `arrest`
  flag, read from `escalation.ron` with `GATE BROKEN` on the row flags), `spawn_kind_table` (per row), 
  `break_free_heat_table` (40→180, 179 (0★)→179, 180→550, 550→1200, 1200→2400, 2400→2400, 300→550),
  `pick_spawn_table` (nearest; surround with taken bearings 0° → picks the candidate at 180° over a nearer one at 10°),
  `arrest_step_table` (hold accumulates to exactly 1.5 at 96 steps; paused between 1.5 and 3.0; 3.01 → BrokeFree;
  knockdown within 1.5 → Busted; attacking → Hold(0)).

**`tests/police_crimes.rs`** (cop rows, each its own case, falsifiable because the witness distance is cut by a named
config mutation `cop_witness_distance = 0.5` below the actual eye distance, so only the "always" rule can report):
- **C1 punch** (unarmed, cop 1.2 m in front, one click): heat 45, one reported `PunchCop`.
- **C2 wound** (pistol at 10 m, full health): heat 80 (the `Shooting` incident stays unreported), `WoundCop` reported.
- **C3 kill** (health 1): heat 150, `KillCop`. Flip for all three: drop the always-branch → heat 0 → RED.
- **C4 `cop_witnesses_shooting_near_itself`**: shipped witness distance, cop 8 m away, `shoot_into_the_air` → heat 10.

**`tests/police_fire_lines.rs`** (the corridor note, both roles):
- Gang, in `tests/gang_fire_lines.rs` (the runner and `Layout.walls` exist): **G-C1..G-C4** corridors 2.4 and 3.0 m
  (walls x = ±1.35 / ±1.65, 0.3 thick, 14 m long, z −20..−6, QA3 layout), orders SMG-front/pistol-rear and
  pistol-front/SMG-rear, each its own `assert_keeps_firing` (≥ `MIN_SHOTS` 6 in 30 s, 0 friendly/bystander). Measured
  before the fix: rear member 0 shots (QA3) → the flip is "remove `queue_slot`".
- Police: **P-C1..P-C4** same corridors with two cops through `spawn_unit` (patrol+patrol, SWAT front + patrol rear),
  state `Attack`, heat 180, armour 1e6, 1920 ticks: each cop ≥ 6 shots, 0 cop→cop damage.
- **P-B1 `cops_never_hit_bystanders`**: the gang "dummies + idle rival" and "sidewalk between" layouts with cops: 0
  damage to dummies, civilians, gang members. Flip: `spares` returns false for everyone → RED.

**`tests/police_city.rs`** (seed 1): **N1 `cops_reach_the_player_in_the_city`** — the navmesh evidence gate: player on
3 sidewalk spots (hospital spawn, plaza, park centre), heat 180, view fixed, `max_civilians = 0`; each cop must reach
`sees` or ≤ `keep_distance.1` within 40 s; print per-cop times. A RED here is not loosened: it is the reproduced stuck
that opens the navmesh task (report it). **N2 `station_spawn_exists`**: `PoliceStationSpawn` lies within 30 m of the
station building centre; plus a pure sweep over seeds 1..=50 (`citygen::generate` + `sidewalk_anchor` of the first
`PoliceStation`) → all `Some` (a failure would exit the game on load).

**`tests/police_bench.rs`**: seed 1, `max_civilians 40`, heat 2400 (12 SWAT), armour 1e6, 640 observed ticks after the
units exist; print mean/p50/p95/max; assert only the mean (TASK-009 lesson) against 10× the probe mean the implementer
measures once (record the probe number in the const's comment, `civilian_bench.rs` style).

Existing tests to keep green and touch only where needed: `crimes.rs` unit tests (`HEAT` literal), `respawn.rs`,
`wanted*.rs` (fixtures lack `PoliceUnit` and `Health`, so nothing changes), all gang tests (Step 2 regression net).

### Step 13 — verification

`cargo build -j 4`, `cargo build -p gta_like --features dev -j 4`, `cargo clippy --workspace --all-targets -j 4 --
-D warnings`, `cargo test -p gta_sim -j 4`, `cargo test -p citygen -j 4` (only if anything in `citygen` is touched —
the plan touches none), `cargo test -p gta_like --bin gta_like -j 4` ×3, `cargo tree -p gta_sim -e normal -i
bevy_render` empty, `python tools/qa/tree_check.py`, `python tools/qa/scenarios/t11.py --out <dir>`. Before trusting
a red in untouched code: `touch crates/*/src/lib.rs` and rebuild (phantom red lesson). Record every flip (perturbed
input, RED, GREEN) in the implementer summary; run every new/touched presentation gate ≥ 3 times.

### Owner checklist (QA writes it into QA_REPORT.md, Russian)

"Полная петля": `cargo run --release -- --seed 1`, взять пистолет.
- [ ] Набедокурить при свидетеле → 1 звезда → через ~10-20 с приходят 2 синих копа со стволами.
- [ ] Стоять спокойно: коп подходит вплотную, через 1.5 с — сцена ареста (игрок на коленях), экран BUSTED 3 с,
      появление у участка без оружия и без звёзд.
- [ ] На 1 звезде, когда коп вплотную, рвануть спринтом прочь → 2 звезды, копы открывают огонь. Понятно ли, что
      это "вырвался"?
- [ ] Выстрел рядом с копом на 1 звезде → копы стреляют; через 5 с спокойствия снова пытаются арестовать.
- [ ] Спрятаться за домом: звёзды мигают, копы идут к последней точке и бродят по кругу; уйти за круг → розыск спадает.
- [ ] Поднять до 4 звёзд (убийство копа = 150): тёмные SWAT с SMG, держатся ближе. Смерть → ПОТРАЧЕНО, как раньше.
- [ ] Копы не стреляют сквозь прохожих и друг друга, не толпятся гуськом в узком проходе.
- [ ] Ручки: `assets/police/escalation.ron` (число копов, дистанции, 1.5 с, 3 м), `wanted.ron` (heat за копов),
      `respawn.ron` (2 с + 3 с), `visual.ron` (цвета), `strings.ron` (BUSTED).

---

## 4. Risk areas

- **Shared-layer refactor changes gang behaviour silently.** Mitigation: Step 2 lands and all gang gates pass before
  Step 3; `hold_fire` is a verbatim move with only the predicate/radius parameterised (`pressed_distance =
  melee_distance.1`). The exact tie-break gate (`of_two_members_blocking_each_other_the_lower_index_moves`) catches
  order changes.
- **Queue slot changes transient fallback cases** in existing gang layouts (open ground rarely reaches the fallback).
  If a gang gate reds after Step 3, the diagnosis is which layout reached the fallback; do not widen the gate.
  A corridor narrower than 2·(0.8 + 0.3) = 2.2 m cannot hold two bodies with a clear line: the slot fails `usable`
  and the old seek remains (alleys are 4-5 m, GDD §2.3). Three shooters in a 2.4 m file: the third may still starve
  (only pairs are gated); owner checklist line.
- **Busted state leaks** (TASK-006/007 class): every Wasted hook now has a Busted twin (damage, input, melee, calls,
  camera pivot, saturation, input release). P7/P8 gate the two silent ones; the rest are owner-visible.
- **`NpcSystems` now runs in Busted**: cops keep acting during the 5 s; `fire_weapons`/`swing_melee` are
  `PlayingSystems`, so nobody shoots or punches; the dispatcher is `PlayingSystems`, so no reinforcements.
- **State transition lag**: `NextState` set in `FixedUpdate` applies in the next update's `StateTransition`, which
  runs right after `PreUpdate` (`bevy_state-0.19.1/src/app.rs:335`), before `RunFixedMainLoop`: the next fixed tick
  already runs in `Busted`, no extra `Playing` tick. P1's exact-tick assert confirms it on the real schedule.
- **Dispatcher spawns in wanted tests**: guarded by `CameraView None` (all existing wanted/respawn tests have no view).
- **City station anchor**: a seed whose station has no long enough sidewalk side would exit the game on load; N2's
  50-seed sweep guards it. With `police_stations: 2` the first station is used (city.ron ships 1).
- **Frame budget** at 5 stars (12 cops × fire-line checks over ~60 bodies + candidate rays on the slot): bench gate
  on the mean; per-tick max has OS spikes (TASK-009), not gated.
- **Arrest reads unfair or too fast** (1.5 m / 1.5 s), SWAT tint readability, BUSTED colour, a cop walking away while
  others search: owner run, knobs in data.
- **`or_else` chain of three `in_state`** compiles in 0.19.1 (`bevy_ecs-0.19.1/src/schedule/condition.rs:508`,
  `SystemCondition::or_else`); verify by `cargo check`.

## 5. Open questions (orchestrator, by the owner's delegation)

1. **Q2=B (банды ↔ полиция враждебны)** — варианты: (A) выключено, копы стреляют только в игрока (рекомендую: нужен
   выбор целей-NPC в FSM копа и фракционный `spares` — отдельная задача); (B) включить сейчас (+ цели-бандиты,
   бой банд с копами, риск дружественного огня по новой матрице). По умолчанию A.
2. **Что значит "вырваться"** — (A) убежать дальше 3 м от арестующего копа во время удержания (рекомендую: ближе
   всего к IV, где сопротивление аресту бегом или дракой сразу даёт 2 звезды); (B) отдельная кнопка во время
   2-секундной анимации ареста (нужны блокировки управления в 4 системах и подсказка в HUD). По умолчанию A.
3. **Сброс розыска при аресте** — на выходе из BUSTED (рекомендую: коп стоит над игроком всю сцену ареста) или на
   входе, как у смерти (копы сразу уходят посреди ареста). По умолчанию на выходе.
4. **"На 5 звёздах мирных меньше" (GDD §6.1)** не входит в цель T11 — оставить на T15/T16? По умолчанию да.

## 6. Research notes (sources)

- GTA IV arrest: arrest is likeliest at one star; resisting "will immediately spark a two-star Wanted Level and cause
  the police to start firing"; the search circle stays at the last seen location while the cops cannot see the
  player — https://gta.wiki/w/Wanted_Level_in_GTA_IV (community wiki, secondary). GDD §6.4 already cites the same
  family of sources.
- Tactical position selection as candidates + filters (line of fire, claimed spots): Matthew Jack, "Tactical Position
  Selection", Game AI Pro ch. 26 — https://www.gameaipro.com/GameAIPro/GameAIPro_Chapter26_Tactical_Position_Selection.pdf;
  Killzone picks positions for a better line of fire and drops those claimed by others (Straatman et al.) —
  http://cse.unl.edu/~choueiry/Documents/straatman_remco_killzone_ai.pdf. The queue slot is one more candidate of
  this kind, used only when the regular candidates fail.
- Bevy APIs relied on, checked in the pinned sources: `SubStates` + `#[source]` (existing `WastedPhase`),
  `DespawnOnExit` (`bevy_state-0.19.1/src/state_scoped.rs:149`), `SystemCondition::or_else`
  (`bevy_ecs-0.19.1/src/schedule/condition.rs:508`), `TimeUpdateStrategy::FixedTimesteps` advancing `Time<Real>`
  (`bevy_time-0.19.1/src/lib.rs:181-183`).

Evidence on disk: `scratch/corridor_geometry.py` → `scratch/corridor_geometry.txt`. Decisions:
`log.jsonl` (planner entries).

children: 0 launched / 0 reported.
