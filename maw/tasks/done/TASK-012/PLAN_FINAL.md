# PLAN_FINAL — TASK-012 (GDD T11): police on foot and arrest

Reviewer: plan-reviewer-2. Sources: `PLAN.md` (planner, full detail) + `PLAN_V2.md` (plan-reviewer-1 corrections) +
this review's corrections (§5). This file is self-contained: the implementer follows it and does not need PLAN.md or
PLAN_V2.md.

Cost of error: **mixed**. Silent class (full evidence layer, gates below): units over the table, an arrest that fires
early or never, a busted player keeping guns or wanted, cop crimes without heat, cops chasing the real position
instead of `LastKnownPosition`, a gang regression from the shared fire-line move, the corridor starvation (TASK-010
QA Bug 1), non-deterministic system order, re-engaged units after a reset, death vs arrest in one tick, the frame
budget at 12 cops. Owner class (mechanism gate + owner run): how cops look, how the arrest reads, whether 1.5 m /
1.5 s is fair, the BUSTED colour, SWAT readability.

Pinned (checked in `Cargo.lock` and `~/.cargo/registry/src/index.crates.io-*/`): bevy / bevy_ecs / bevy_state /
bevy_time / bevy_app 0.19.1, avian3d 0.7.0, bevy-tnua 0.32.0, pathfinding 4.16.0, rand_chacha 0.10.0, ron 0.12.2,
serde / serde_core 1.0.229. **No new crate, no Cargo.toml change.** `citygen` source is not changed; one test is
added to `crates/citygen/tests/properties.rs`.

Builds: `CARGO_TARGET_DIR=D:/test-gta-like/target`, `-j 4`, one cargo command at a time, in place (host under memory
pressure). Fixed tick = Bevy default 64 Hz (`1/64 s`, `wanted/search.rs` tests use it); under
`TimeUpdateStrategy::FixedTimesteps(1)` one `app.update()` = one fixed tick and `Time<Real>` advances 1/64 s per
update (`bevy_time-0.19.1/src/lib.rs:181-183`); the first `update()` runs 0 ticks, so count ticks via `Time<Fixed>`
(`common::run_ticks`).

---

## 1. Summary

T11 adds foot police on top of the TASK-011 wanted core. A shared NPC gunfight layer `crates/gta_sim/src/tactics/`
is extracted from the gang code (pure move, gang behaviour bit-for-bit unchanged), and its no-candidate fallback gains
a **queue slot** beside the nearest non-yielding blocker, which fixes the corridor starvation for gangs and police. A
new `police/` domain (`PolicePlugin`, data `assets/police/escalation.ron`) brings a `PoliceDispatcher` that keeps the
unit count at the star row (off-frame spawns on sidewalk points, SWAT share, reinforcement delay), a pure cop FSM
`Respond/Arrest/Attack/Search/Leave/Dead` (`Leave` is terminal), cop crimes with "always reported" heat, and the arrest:
a 1-star cop within 1.5 m for 1.5 s of player passivity (or a knocked-down player) → `GameState::Busted` (arrest phase
2 s + BUSTED screen 3 s on `Time<Real>`) → respawn at the police station with weapons confiscated and wanted reset on
exit; running farther than 3 m from the arresting cop gives +1 star. The client adds police/SWAT looks, the kneeling
pose while `Cuffed`, and the BUSTED screen from the death-screen widget. Navmesh is decided by evidence (city chase +
QA stuck log), not built. Q2=B stays off.

Buffered signals: no new `Message` type. Police systems read the existing `ShotFired`, `MeleeHit`, `DamageDealt` with
their own `MessageReader`s (independent cursors). No observers. The flow change uses `NextState` (buffered by design,
applied in `StateTransition`, which runs right after `PreUpdate`: `bevy_state-0.19.1/src/app.rs:335`).

### Verified facts the steps rely on (all in code)

- `WantedLevel { heat, stars, last_known, seen, hidden }`; `stars` is recomputed only in `search::search_step`
  (called by `track_search`, `WantedSystems`, `.after(AiSystems::Decide).in_set(PlayingSystems)`, chain
  `record_crimes → take_calls → track_search → forget_crimes`, `wanted/mod.rs:174-190`). `search_step` does
  `w.last_known.get_or_insert(player)` for any `heat > 0` (`search.rs:80`), so a BRP/test heat write gets a
  `last_known` on the next tick. Heat never decreases except to 0 (whole-resource reset).
- Star thresholds (shipped): 40 / 180 / 550 / 1200 / 2400 (`wanted.ron`; `stars_for` counts thresholds reached:
  39 → 0, 40 → 1, 179 → 1, 180 → 2, …).
- `HealthSystems::{Damage, Regen, Pickup, Death}` chained; `(Perceive, Decide, PopulationSystems)` chained, in
  `NpcSystems`, `.after(HealthSystems::Death)` (`perception/mod.rs:146-151`). Every damage writer runs before
  `Perceive`. `bevy_ecs-0.19.1` `ScheduleBuildSettings::auto_insert_apply_deferred` defaults to `true`
  (`schedule/schedule.rs:1629`), so a `Commands` insert in `HealthSystems::Death` (e.g. `detect_player_death`
  inserting `Dead`) is applied before any system ordered after `AiSystems::Decide`.
- `NpcSystems` runs only in `Playing | Wasted` **and** with `resource_exists::<SidewalkGraph>`
  (`flow/mod.rs:53-57`, `navigation/mod.rs:347-349`). `GangSystems` additionally needs `GangTerritories`; police do
  not join `GangSystems`.
- `gang_fsm` transitions state **every tick**; only senses (`sees`) refresh on the AI slot (`Perception.slot ==
  tick % slots`, `slots = 4`). Police copy this.
- `drive_characters` has no state gate (only `TnuaUserControlsSystems`); it zeroes the Tnua walk basis only for `Dead`
  or an active `HitReaction` (`character/mod.rs:154-178`).
- `GangConfig::spares` returns `false` for any non-gang attacker (`gang/mod.rs:349-354`); hitscan has no faction
  sparing, so cop→cop bullets hurt and the police hold-fire predicate is the only protection.
- `population::spawn_points` / `SpawnPoint` are private (`population/mod.rs:396-426`: every node with an edge + points
  every `spacing` inside each edge, `count = floor(len/spacing − 0.5)`); `outside_cone`, `occluded`,
  `OCCLUSION_RAYS_PER_POINT`, `PopulationLoad`, `CameraView`, `Offscreen`, `corpse_components`, `Appearance` are
  public. `spawn_civilians` resets `PopulationLoad` at its very start (`population/mod.rs:447`). Off-frame ageing
  (`despawn_far_gangs`) uses the view cone only: `Offscreen` grows every tick the body is outside the cone, resets to
  0 inside it.
- RNG streams: `CombatRng` 0, `NpcRng` 1, `GangRng` 2 → police take stream 3.
- `Health::full` sets `armor: 0` (`character/health.rs:94-100`): the shared respawn strips armour for Wasted and
  Busted alike (matches GTA IV; no new rule).
- `Loadout` derives `Default, Clone, Debug`, **not** `PartialEq`; `acquire(slot: &mut GunSlot, stats, gun: bool)` is a
  free function (`combat/weapons.rs:276`), used as in `gang_member_bundle` (`gang/mod.rs:460-461`).
- `Crimes::record` merges a non-shooting crime per `(offender, crime, victim)`: a second wound of the same cop joins
  the first `WoundCop` incident (one contribution per victim and crime kind, like civilians).
- `Plugins` is implemented for tuples up to 15 (`bevy_app-0.19.1/src/plugin.rs:186-193`); `SystemCondition::or_else`
  (`bevy_ecs-0.19.1/src/schedule/condition.rs:508`); `DespawnOnExit<S: States>` (`bevy_state-0.19.1/src/
  state_scoped.rs:149`); serde's array visitor error contains `"an array of length 5"` (`serde_core-1.0.229/src/de/
  impls.rs:1277`).
- Test floor (`world/test_area.rs`, `Collider::cuboid` takes full lengths): floor 80×80 around the origin; box 4×5×4 at
  (−10, 2.5, −22.66) (x ∈ [−12, −8], z ∈ [−24.66, −20.66]); ramp 4×0.4×10 at (−10, 2.327, −16.43) tilted 30°
  (x ∈ [−12, −8], z ≈ [−20.8, −12.1]); block 3×1.6×6 at (10, 0.8, −17.4); stairs x ∈ [8.5, 11.5], z ∈ [−14.4, −12.0];
  cubes near (10..16, *, 10); **wall 12×4×0.5 at (0, 2, 14)** (x ∈ [−6, 6], z ∈ [13.75, 14.25], 4 m high).
- Locomotion: `run_speed 4.5`, `sprint_speed 6.8`, `time_to_run_speed 0.15`, `capsule_radius 0.3`,
  `float_height 1.05`, `head_height 1.6`. Navigation: `direct_seek_distance 25`, `arrive_radius 0.5`,
  `route_requests_per_tick 2`. Population: `spawn_point_spacing 8`, `spawn_min_separation 4`,
  `spawn_view_margin_deg 5`, `occlusion_ray_height 1.8`, `despawn_distance 150`, `despawn_offscreen_seconds 2`.
  Camera view from `common::chase_view`: half-angle 55.0° (+5° margin).

---

## 2. Implementation steps

### Step 1 — data

**`assets/police/escalation.ron` (new, path from GDD §12):**
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
Tuple syntax for `stars` (ron 0.12.2 reads `[T; 5]` from a tuple, like `wanted.ron`). Every number is starting data
for the owner run; none becomes a `const`.

Also:
- `assets/wanted/wanted.ron` `heat`: add `punch_cop: 45, wound_cop: 80, kill_cop: 150` (GDD §6.4); comment becomes
  "Heat of a crime by the player (GDD §6.4): civilian/gang rows need a witness, cop rows are reported always (the cop is
  the witness). Car rows come with T14/T15."
- `assets/flow/respawn.ron`: add `busted_arrest: 2.0, busted_screen: 3.0` (GDD §3.4, §6.4).
- `assets/ui/strings.ron`: add `busted: "BUSTED"`, `busted_color: (0.20, 0.45, 1.0)` next to `wasted*` (the widget
  reuses `wasted_size`, `wasted_backdrop`, `wasted_saturation`).
- `assets/character/visual.ron`: `police_models: ["third_party/mini-characters/character-male-c.glb",
  "third_party/mini-characters/character-female-e.glb", "third_party/mini-characters/character-female-f.glb"]`,
  `police_tint: (0.25, 0.40, 1.0)`, `swat_tint: (0.22, 0.22, 0.30)`, one-line comment "Police looks: `Appearance`
  picks a model; blue patrol, dark SWAT". All three GLBs are in the manifest rig list. (`male-c` is also a civilian
  model and the civilian tint `(0.55, 0.75, 1.0)` is bluish: owner checklist line, list kept.)

### Step 2 — shared gunfight layer `crates/gta_sim/src/tactics/` (pure move, green checkpoint)

- `git mv crates/gta_sim/src/gang/behavior/fire_line.rs crates/gta_sim/src/tactics/fire_line.rs` (keeps history);
  `crates/gta_sim/src/lib.rs`: `pub(crate) mod tactics;`. The empty `gang/behavior/` directory goes away.
- `tactics/mod.rs` (new, ~200 lines), moved from `gang/behavior.rs` with visibility `pub(crate)`: `Ctx` (`:121-127`),
  `Seek`/`Motion` (`:129-142`), `head_for` (`:144-180`), `walk` (`:182-190`), `select` (`:610-619`). `head_for` takes
  `avoid: &mut f32` instead of `member: &mut GangMember` (only `member.avoid` is used).
- `pub(crate) struct Discipline<'a> { aim_error_deg: f32, fire_line_margin: f32, pressed_distance: f32,
  reposition_offsets: &'a [f32], reposition_step: f32, reposition_gait: Gait, chase_gait: Gait }`.
  `GangCombatConfig::discipline(&self) -> Discipline<'_>` fills it with `pressed_distance = melee_distance.1` (today's
  yielding radius). No RON change for gangs.
- `fire_line.rs`: `FireLine::of(aim_error_deg: f32, weapons, gun, loadout, clearance)`; `spots(b, d)`,
  `pinned(b, d)`, `clear_spot(.., d, ..)`, `unblock(ctx, b, plan, on_slot, d: &Discipline, radius, rays)` read the
  `Discipline` instead of `GangCombatConfig`; `Shooter.gang: u8` → `faction: Faction`; imports from `super` (tactics).
- `pub(crate) fn hold_fire(...)` = the block `gang/behavior.rs:453-495`, generalised. Inputs: `me: Entity`, chest,
  target entity + chest, `line: FireLine`, `living: &[(Entity, Vec3, Option<Faction>)]`, `shooters: &[Shooter]`,
  `spares: impl Fn(Option<Faction>) -> bool`, `plan: Option<Vec3>`, `on_slot`, `ctx`, `d: &Discipline`, `radius`,
  `rays: &mut u32`. Returns `(line_blocked: bool, kept_spot: Option<Vec3>, clearing: Option<Motion>)`. Mapping:
  shields filter `cfg.spares(Some(gang), f)` → `spares(f)`; tie-break `cfg.spares(Some(gang),
  Some(Faction::Gang(s.gang)))` → `spares(Some(s.faction))`; `yielding` uses `d.pressed_distance`. Preserve exactly:
  - `let plan = member.reposition.take();` stays in `gang_fsm` **before** the block, unconditionally (a clear line
    drops the kept spot);
  - in the `yields` branch `clearing = Some(Motion::Stand)` and `kept_spot = None`;
  - `kept_spot` is `unblock`'s returned spot only when `unblock` ran; the gang writes `member.reposition = kept_spot`
    (always `None` when the line was clear or it yielded — identical to today).
- `pub(crate) fn apply_motion(ctx, motion, chest, on_slot, avoid: &mut f32, route, load, intent)` = the match at
  `gang/behavior.rs:599-606`.
- `gang/behavior.rs`: delete the moved items, import from `crate::tactics`, `let d = c.discipline();` once per
  `gang_fsm` run, replace the hold-fire block and the motion match by the calls; `pull`, the FSM and `gang_death`
  stay. The file shrinks from 665 lines.
- **Checkpoint (before Step 3):** `cargo test -p gta_sim -j 4 --test gang_fire_lines --test gang_combat --test gangs
  --test gang_city` green, run once, recorded (incl. `of_two_members_blocking_each_other_the_lower_index_moves`). A red
  here is a move bug, never a gate to adjust.

### Step 3 — corridor fix (both roles), `tactics/fire_line.rs`

Add (one-line doc):
```rust
/// Beside the nearest blocker that is not pressed against the target, shoulder to shoulder with it: in a
/// passage too narrow for the candidate spots the rear shooter queues up level with the front one.
fn queue_slot(spatial: &SpatialQuery, b: &Blocked, radius: f32, rays: &mut u32) -> Option<Vec3>
```
- Blockers = `b.line.blockers(b.chest, b.to, b.shields)` filtered by `!b.yielding[k]`; take the one nearest to
  `b.chest` (flat distance; tie → lower index). None → `None`.
- With blocker `p`: `ahead = (b.to − p).with_y(0).normalize_or_zero()`, `side = Vec3::new(−ahead.z, 0, ahead.x)` (same
  convention as `spots`), offset `o = radius + b.line.clearance` (0.3 + 0.5 = 0.8 m: body radius + existing margin,
  no new number). Candidates `p + side·o`, `p − side·o`, the one nearer to `b.chest` first, tie → `+side`. The first
  that passes `usable(spatial, b, slot, radius, rays)` wins (same line, bump and wall-ray test as every spot).
- `unblock`, in the `let Some(spot) = plan else { .. }` branch (no plan / no usable candidate):
  `if pinned(b, d) { return (None, None); }` (unchanged), **then** `if let Some(slot) = queue_slot(ctx.spatial, b,
  radius, rays) { return (Some(slot), Some(Motion::Yaw(steer(b.chest, slot), d.reposition_gait))); }`, then the old
  close-in seek. The slot becomes the kept plan and is re-checked on the AI slot like any spot.
- Update the `unblock` doc comment by one sentence naming the queue slot.
- Worked geometry (both roles, widths 2.4 / 3.0 m, three weapon orders): `scratch/corridor_geometry.txt` — slot
  (−0.8, −10), rear-line perp 0.797 > limit ≤ 0.512, front line not blocked (along 0, perp 0.8 > 0.5), the walk passes
  the front body at 0.784 m ≥ 0.6 m, slot body edge 1.1 m < wall face 1.2 m in the 2.4 m corridor.

### Step 4 — wanted: cop crimes and re-exports

- `wanted/mod.rs`: `HeatTable` gains `punch_cop`, `wound_cop`, `kill_cop` (`u32`, one `///` "reported always: the cop
  is the witness"); `of()` maps `Crime::PunchCop/WoundCop/KillCop`; `validate` loops the three new fields (`== 0` →
  `heat.<field> must be > 0`). Re-export for police: `pub(crate) use search::{cop_sees, eye, witnesses};`.
- `wanted/crimes.rs`: `Crime` gains `PunchCop, WoundCop, KillCop`; `Victim::Cop`; `classify`: `(Cop, true) →
  KillCop`, `(Cop, false) if melee → PunchCop`, `(Cop, false) → WoundCop` (doc: "gang wounds and hits on anyone else
  are none"). `kinds` query becomes `(Has<Civilian>, Has<GangMember>, Has<PoliceUnit>)` matched civilian first, then
  gang, then cop; `persons` filter becomes `Or<(With<Civilian>, With<GangMember>, With<PoliceUnit>)>` (a cop is a
  person for "shooting near people"; bare `spawn_cop` fixtures have no `Health` and stay out). In `record_crimes`,
  keep a separate `always: Vec<IncidentId>` for cop crimes and report them (`crimes.report` + `apply_report`) **before**
  the `touched.is_empty()` / witness early-returns; `touched` keeps only the others. `resolve(Body(v))` accepts
  `Crime::Kill | Crime::KillCop`. Unit tests: `HEAT` literal gains the three fields; `classify_table` gains 3 cop rows
  (each its own case).
- `WantedPlugin`: `.add_systems(OnExit(GameState::Busted), (reset_wanted, drop_queued_calls))`.

### Step 5 — flow: Busted

- `flow/mod.rs`: `GameState::Busted` (doc: "Arrested: the arrest scene, the BUSTED screen, then respawn at the police
  station without weapons (GDD §3.4)"). `#[derive(SubStates, Default, Clone, PartialEq, Eq, Hash, Debug, Reflect)]
  #[source(GameState = GameState::Busted)] pub enum BustedPhase { #[default] Arrest, Screen }`; `BustedSystems` set
  (Update, `run_if(in_state(GameState::Busted))`); `add_sub_state::<BustedPhase>()`, `register_type_state`,
  `init_resource::<BustedClock>()`. `NpcSystems` condition: `in_state(Playing).or_else(in_state(Wasted)).or_else(
  in_state(Busted))`, doc "keeps running while the player is wasted or busted". Registrations: `OnEnter(Busted)` →
  `busted::enter_busted`; `Update` → `busted::advance_busted.in_set(BustedSystems)`; `OnExit(Busted)` →
  `(busted::respawn_at_station, wasted::drop_queued_damage, wasted::drop_queued_input)`. Re-export `BustedPhase`,
  `BustedClock`.
- `flow/busted.rs` (new, ~110 lines): `#[derive(Resource, Default)] pub struct BustedClock(pub f32)`;
  `enter_busted` (clock 0, `commands.entity(player).insert(Cuffed)`, `ActionIntent::default()` on the player);
  `advance_busted` (`Time<Real>`, same shape as `advance_wasted`: `Arrest → Screen` when `clock >= busted_arrest`,
  `Playing` when `clock >= busted_arrest + busted_screen`; comment "Update + Time<Real>, like advance_wasted");
  `respawn_at_station`: `respawn_at(PoliceStationSpawn.point, ..)`, then `*loadout = Loadout::default()` (guns, ammo
  and bat confiscated, GDD §3.4) and `commands.entity(player).try_remove::<Cuffed>()`.
- `flow/wasted.rs`: extract the teleport/heal body of `respawn_player` into one shared `pub(super) fn respawn_at(point:
  Vec3, cfg: &HealthConfig, commands, (entity, position, transform, velocity, health, body))` used by
  `respawn_player` (hospital) and `respawn_at_station` — no copy. `RespawnConfig` gains `busted_arrest`,
  `busted_screen` (finite, `>= 0`, error text `busted_screen must be >= 0` / `... is not finite` in the existing loop).
- `character/mod.rs`: `#[derive(Component, Reflect, Default)] #[reflect(Component)] pub struct Cuffed;` ("held in
  place without control: the arrested player"), registered; `drive_characters` adds `Has<Cuffed>` and treats it like
  `dead` (zero the walk basis, no jump).
- `combat/mod.rs`: `melee::reset_player_melee` also on `OnExit(GameState::Busted)`.

### Step 6 — world: police station spawn

- `world/mod.rs`: `#[derive(Resource, Reflect, Clone, Copy, Debug)] #[reflect(Resource)] pub struct PoliceStationSpawn
  { pub point: Vec3, pub along: Vec3 }` ("read by QA over BRP"), registered, inserted in `WorldPlugin::build` with the
  test-area fixture `test_area::STATION_SPAWN = Vec3::new(-20.0, 0.0, 20.0)` (clear floor, far from the hospital
  default at the origin, so "respawned at the station" is falsifiable).
- `world/city.rs`: `station_spawn(layout, params, curb, margin)` = `hospital_spawn` for the **first**
  `BuildingKind::PoliceStation` (with `police_stations: 2` the first one; `city.ron` ships 1), same margin
  (`health.pickups.spacing`, one-line comment "same clearance from the block corners as the hospital spawn"), inserted
  and error-exited like the hospital.
- Property gate in `crates/citygen/tests/properties.rs`: `police_station_anchor_on_sidewalk` over `layouts()`
  (SEEDS + SWEEP, cached). Extract the body of `hospital_anchor_on_sidewalk` (`:320-352`, anchor + unit direction +
  three points on a sidewalk) into a small helper `assert_anchor_on_sidewalk(seed, layout, params, idx, margin,
  what)` used by both tests (no copy). For stations: **every** `BuildingKind::PoliceStation` index must anchor
  (at least one per layout, else panic naming the seed). Flip: call the helper with margin `10_000.0` for stations →
  `None` → RED; restore → GREEN (perturbs the anchor search itself, not the test's input index).

### Step 7 — population

`population/mod.rs`: `SpawnPoint` (and its fields) and `spawn_points` become `pub(crate)`. Nothing else.

### Step 8 — police domain `crates/gta_sim/src/police/` (`lib.rs`: `pub mod police;`)

**`mod.rs`** (~320 lines): `pub const POLICE_CONFIG: &str = "police/escalation.ron";`. Config structs, all
`#[derive(Deserialize, Clone, Debug)] #[serde(deny_unknown_fields)]`, `EscalationConfig` also `Resource`:
`EscalationConfig { stars: [EscalationRow; STARS], patrol: UnitSpec, swat: UnitSpec, spawn_ring: (f32, f32),
spawns_per_tick: u32, arrest: ArrestConfig, search_arrive_distance: f32, combat: PoliceCombatConfig }`,
`EscalationRow { units: u32, swat: u32, reinforce_seconds: f32, arrest: bool, surround: bool }`, `UnitSpec { gun:
Weapon, armor: f32, reserve: u32, keep_distance: (f32, f32) }`, `ArrestConfig { distance, stand_distance, seconds,
break_free_distance, hostile_seconds }`, `PoliceCombatConfig { aim_error_deg, trigger_seconds: (f32, f32),
fire_line_margin, pressed_distance, reposition_offsets: Vec<f32>, reposition_step, chase_gait, reposition_gait,
search_gait, leave_gait }` + `discipline()`.
`validate()` (errors name the field, rows as `stars[i].<field>`): `swat <= units`; `units` and `swat`
non-decreasing over rows (`stars[i].units must be >= stars[i-1].units`); `stars[0].units >= 1`;
`reinforce_seconds` finite `>= 0`; `armor` finite `>= 0`; keep bands `0 < lo < hi`; `spawn_ring` `0 < inner <
outer`; `spawns_per_tick >= 1`; arrest `0 < stand_distance < distance < break_free_distance` (error names the
violated field, e.g. `arrest.break_free_distance`), `seconds > 0`, `hostile_seconds >= 0`;
`search_arrive_distance > 0`; combat like `GangCombatConfig::validate_combat` (aim error in [0, 90), trigger
`0 < lo <= hi`, margin `>= 0`, offsets non-empty finite `> 0`, step `> 0`, `pressed_distance > 0`).
`validate_ring(despawn_distance)`: `spawn_ring.1 < despawn_distance` (error names `spawn_ring`), cross-config, called
from `compose_sim` like `validate_fight_hearing`.

Components / resources (all `Reflect`, registered):
- `enum UnitKind { Patrol, Swat }`; `enum CopState { Respond, Arrest, Attack, Search, Leave, Dead }`.
- `#[derive(Component, Reflect, Clone, Debug)] #[reflect(Component)] #[require(Character, Perception, Offscreen,
  Route)] pub struct PoliceUnit { kind, state, sees: bool, dest: Option<Vec3>, dest_clear: bool, avoid: f32,
  trigger_left: f32, reposition: Option<Vec3>, search_point: Option<Vec3> }` (`///` per field; `dest` = where it is
  heading this tick, for gates and QA).
- `PoliceRng(ChaCha8Rng)`: `seeded(seed)` = `seed_from_u64(seed)` + `set_stream(3)`; `unit()`, `next_u32()`.
- `PoliceDispatcher { units: u32, swat: u32, reinforce_left: f32 }` (Resource, Reflect: active counts for QA).
- `ArrestAttempt { cop: Option<Entity>, hold: f32 }`, `PoliceAlert { hostile_left: f32 }` (Resources, Reflect).
- `pub fn police_unit_bundle(loco, handle, health_cfg, weapons, spec: &UnitSpec, kind, feet: Vec3, facing_yaw: f32,
  appearance: Appearance) -> impl Bundle`: `PoliceUnit { state: Respond, kind, .. }`, `Faction::Police`, `Name`,
  `Transform::from_translation(feet + Y·float_height).with_rotation(Quat::from_rotation_y(facing_yaw))` (forward =
  rotation·(−Z)), `character_components(loco, handle)`, `Health { armor: spec.armor, ..Health::full(health_cfg) }`,
  `Loadout`: `Loadout::default()`, `acquire(&mut loadout.guns[gun.index()], weapons.stats(gun), true)`, then
  `loadout.guns[gun.index()].reserve = spec.reserve.min(stats.max_reserve)`; `appearance`.
- `PoliceSystems` set. `PolicePlugin { seed }` registers types and resources, inserts `PoliceRng::seeded(seed)`, and:
  - `configure_sets(FixedUpdate, PoliceSystems.after(PopulationSystems).after(WantedSystems).in_set(NpcSystems))`
    (after `PopulationSystems`: `spawn_civilians` resets the shared `PopulationLoad.rays` budget first; after
    `WantedSystems`: the dispatcher reads `stars` / `last_known` that `track_search` writes — without this edge the
    Res/ResMut conflict runs in unspecified order).
  - `FixedUpdate`: `behavior::police_death.in_set(HealthSystems::Death).in_set(NpcSystems)`;
    `behavior::police_alert.in_set(AiSystems::Perceive)`; `behavior::police_fsm.in_set(AiSystems::Decide)`;
    `(dispatch::despawn_police, dispatch::dispatch_police.in_set(PlayingSystems)).chain().in_set(PoliceSystems)`;
    `arrest::arrest_player.after(AiSystems::Decide).before(WantedSystems).in_set(PlayingSystems)`. Flat tuples only,
    no nested `.chain()` (TASK-008 lesson).
  - `OnEnter(GameState::Wasted)` and `OnExit(GameState::Busted)` → `arrest::reset_arrest` (`ArrestAttempt` and
    `PoliceAlert` to default).
  - Resulting order of every `WantedLevel` reader/writer per tick (record it in the implementer summary):
    `police_fsm` (Decide, reads last tick's `stars`) → `arrest_player` (may write `heat`) → `WantedSystems`
    (`record_crimes`, `take_calls` write heat; `track_search` recomputes `stars`, `last_known`) → `PoliceSystems`
    (`dispatch_police` reads fresh `stars`).
- `compose_sim` (`lib.rs`): load `EscalationConfig` from `POLICE_CONFIG`, `validate()` and
  `validate_ring(population.despawn_distance)` (both mapped to `ConfigError { path: root.path(POLICE_CONFIG), .. }`),
  insert, and add `police::PolicePlugin { seed: combat_seed }` to the plugin tuple before `WantedPlugin` (keep
  `WantedPlugin` last; the tuple grows 13 → 14 ≤ 15).

**`fsm.rs`** (~260 lines incl. tests), pure:
- `pub(crate) struct CopSenses { sees: bool, hostile: bool, arrest_row: bool, stars: u8, at_goal: bool }`;
  `next_state(state, &s) -> CopState`, in this order:
  `Dead → Dead`; `Leave → Leave` (**terminal**: only `police_death` moves it on); `stars == 0 → Leave`;
  `Respond | Search`: `sees` → `if arrest_row && !hostile { Arrest } else { Attack }`; else `Respond && at_goal →
  Search`; else unchanged;
  `Arrest`: `!(arrest_row && !hostile) → Attack`; `!sees → Respond`; else `Arrest`;
  `Attack`: `!sees → Respond`; `arrest_row && !hostile → Arrest`; else `Attack`.
- `spawn_kind(row, units, swat) -> Option<UnitKind>`: `None` if `units >= row.units`; `Swat` while `swat < row.swat`;
  else `Patrol`.
- `break_free_heat(heat, rows: &[StarRow; STARS]) -> u32`: `s = stars_for(heat, rows)`; `s in 1..5` →
  `max(heat, rows[s as usize].heat)` (threshold of the next star); else `heat`.
- `pick_spawn(candidates: &[Vec3], centre, taken: &[Vec3], surround) -> Option<usize>`: no surround or no taken →
  nearest to `centre` (flat; tie → lower index); surround → maximise the smallest flat bearing difference (around
  `centre`) to `taken`, tie → nearer.
- `arrest_step(hold, distance, attacking, knocked_down, dt, cfg: &ArrestConfig) -> ArrestStep { Hold(f32), BrokeFree,
  Busted }`: `knocked_down && distance <= cfg.distance → Busted`; `attacking → Hold(0.0)`; `distance <=
  cfg.distance` → `h = hold + dt`, `h >= seconds → Busted` else `Hold(h)`; `distance > break_free_distance →
  BrokeFree`; else `Hold(hold)` (paused).

**`behavior.rs`** (~480 lines):
- `police_alert` (Perceive): `hostile_left = (hostile_left − dt).max(0)`; set to `arrest.hostile_seconds` when a player
  `ShotFired` (muzzle) or player `MeleeHit` is witnessed by a live `PoliceUnit` (`witnesses(spatial, eye(cop_chest,
  loco), eye(player_chest, loco), wanted_cfg)`) or a player `DamageDealt` targets a `PoliceUnit`.
- `police_fsm` (Decide; params grouped in tuples like `gang_fsm`; per unit, skip `Dead` and `Leave`-with-no-player
  details below):
  1. On its AI slot (`perception.slot == clock.tick % slots`; a new unit waits ≤ `slots` ticks): `sees = player alive
     && cop_sees(spatial, eye(chest), rotation·(−Z), eye(player), wanted_cfg)`; `dest_clear = dest.is_some_and(|d|
     !sight_blocked(spatial, eyes, d + Y·float_height))`.
  2. `stars = wanted.stars`; `row = (stars >= 1).then(|| &esc.stars[stars − 1])`; `hostile = alert.hostile_left > 0`;
     `goal` = Respond: `wanted.last_known`; Search: `search_point`. `at_goal = goal.is_some_and(|g| flat_distance(feet,
     g) <= search_arrive_distance)`.
  3. `state = next_state(..)` every tick; entering `Attack` rolls `trigger_left` (`PoliceRng`,
     `combat.trigger_seconds`); `trigger_left` decays by `dt` in `Attack`.
  4. Search point: when entering `Search` or on arriving at the current point, pick uniformly (`PoliceRng`) among
     `spawn_points(graph, population.spawn_point_spacing)` within `wanted_cfg.stars[stars−1].search_radius` of
     `last_known`; none → `last_known + polar(radius·√u, 2πv)`.
  5. Intents per state (aim from the eyes like `gang_fsm`): **Respond** gun drawn, not aiming, `Seek(last_known,
     chase_gait, direct = dest_clear && d <= nav.direct_seek_distance)`; **Arrest** aim at the player chest, gun
     drawn, `Seek(player, chase_gait, direct = sees && d <= direct_seek_distance)` while `d > stand_distance`, else
     `Stand`, never pulls; **Attack** aim, `hold_fire` with `spares = |f| f != Some(Faction::Player)` (Q2=A) and the
     police `shooters` list (units in `Attack` that see and hold their gun), pull when `trigger_left == 0 && sees &&
     in_range && held == gun && !line_blocked`, motion = clearing or `gang::fsm::band_move(d, spec.keep_distance)`
     (already `pub`) mapped like the gang (`Approach` → seek, `BackOff` → yaw away, `Hold` → stand); **Search** gun
     drawn, not aiming, `Seek(search_point, search_gait, dest_clear)`; **Leave** holster (`select(None)`), not aiming,
     `Yaw(steer(player_chest, chest), leave_gait)` (walks away), no player → `Stand`.
  6. `dest` written every tick (Respond: `last_known`; Arrest/Attack: player chest; Search: `search_point`; Leave:
     `None`).
  7. `apply_motion` (shared). A 6-line `pull` of its own (roll `trigger_left` from `PoliceRng`, `fire_requested`,
     error cone `aim_error_deg` via `cone_sample`).
- `police_death` (HealthSystems::Death + NpcSystems) = `gang_death` for `PoliceUnit` (state `Dead`, zero intents, drop
  the gun with `dropped_gun`, `corpse_components()`).

**`dispatch.rs`** (~220 lines):
- `despawn_police` (NpcSystems, in `PoliceSystems`): skip `Dead` (corpses belong to `age_corpses`); off-frame ageing
  like `despawn_far_gangs` (cone only, margin 0); despawn (`try_despawn`) when `offscreen >= despawn_offscreen_seconds`
  and (`state == Leave` or flat distance to the player > `despawn_distance`).
- `dispatch_police` (PlayingSystems, in `PoliceSystems`): `reinforce_left = (reinforce_left − dt).max(0)`; any
  `(With<PoliceUnit>, Added<Dead>)` → `reinforce_left = row.reinforce_seconds` (current row); return when `stars == 0`,
  `CameraView` `None`, `last_known` `None`, no player, or `reinforce_left > 0`. `active` = units not `Dead` and not
  `Leave` (`Leave` never comes back, so the table bound holds after a reset). Up to `spawns_per_tick`: `kind =
  spawn_kind(row, active, active_swat)`; candidates = `spawn_points` within `spawn_ring` (flat) of the player, `>=
  population.spawn_min_separation` from every `Character` and this tick's spawns, and hidden: `outside_cone(view, at,
  head_height, margin)` or `occluded(..)` within the shared `PopulationLoad.rays` budget (same rules as
  `spawn_gangs`); order by `pick_spawn(.., last_known, active positions, row.surround)`; spawn `police_unit_bundle`
  facing `last_known` (`aim_yaw`), `Appearance(rng.next_u32())`. Write `PoliceDispatcher { units, swat,
  reinforce_left }` every run.

**`arrest.rs`** (~150 lines):
- `arrest_player` (PlayingSystems, after Decide, before WantedSystems): player `(Entity, &Position, &HitReaction,
  &Melee)` with `(With<Player>, Without<Dead>)`. The `Without<Dead>` filter is the death guard (the `Dead` insert of a
  same-tick death is already applied, §1); no extra health check (P9 gates it). An attempt's cop is valid only while
  it is a live `PoliceUnit` in `Arrest`, else cancel (hold 0, cop `None`). No attempt → the nearest `Arrest` cop within
  `arrest.distance` (flat) starts one **and the same tick runs `arrest_step` with `hold = 0`** (so the start tick ends
  at `Hold(dt)`). `attacking` = a player `ShotFired` read this tick or `melee.swing.is_some()`. `Busted` →
  `next.set(GameState::Busted)`; `BrokeFree` → `wanted.heat = break_free_heat(wanted.heat, &wanted_cfg.stars)`,
  cancel; `Hold(h)` → store.
- `reset_arrest`.

### Step 9 — client

- `src/visuals/character_config.rs`: `police_models: Vec<String>` (non-empty, listed in the manifest and present like
  `gang_models`), `police_tint`, `swat_tint`; `src/main.rs` preflight lists police models like gang models.
- `src/visuals/character.rs`: `CharacterAnimations` chains `police_models` after `gang_models` (update the `ModelKey`
  doc); `body_look` gains `police: Option<UnitKind>` → model `1 + C + G + a % P`, tint `police_tint` / `swat_tint`;
  `LookQuery` adds `Option<&PoliceUnit>`. `drive_character_animation`: `Has<Cuffed>` shows `ShownAction::Cower` (the
  `crouch` clip, kneeling) like a cowering civilian.
- `src/visuals/civilian_gate.rs` `models()` (`:62-68`) chains `police_models` (every model animates).
- `src/hud/wasted.rs`: `spawn_wasted_screen` body becomes a generic `title_screen(commands, ui, fonts, text, color,
  exit: impl States)` builder (uses `DespawnOnExit(exit)`), used by `spawn_wasted_screen` (`WastedPhase::Screen`) and a
  new `spawn_busted_screen` (`BustedPhase::Screen`). `src/menu/config.rs`: `busted`, `busted_color` (+ validation like
  `wasted`, `wasted_color`).
- `src/hud/mod.rs`: `OnEnter(BustedPhase::Screen)` → busted screen, `OnEnter(GameState::Busted)` → `desaturate`,
  `OnExit(GameState::Busted)` → `restore_saturation`; plugin doc mentions BUSTED. `src/input/mod.rs`:
  `OnEnter(GameState::Busted)` → `release_held_actions`. `src/camera/mod.rs`: `OnExit(GameState::Busted)` →
  `reset_pivot`.
- `src/visuals/police_gate.rs` (new, `#[cfg(test)]`, modelled on `gang_gate.rs::every_gang_model_animates_from_its_
  own_clips` + the tint check; harness per the TASK-009/010 lessons: `MinimalPlugins + TransformPlugin +
  AssetPlugin{file_path} + ImagePlugin + MeshPlugin + AnimationPlugin + WorldSerializationPlugin + GltfPlugin`,
  `init_asset::<StandardMaterial>()`, `StandardMaterialStandIn` handler from `civilian_gate.rs`): one patrol and one
  SWAT per police model through `police_unit_bundle`; each model's `leg-left` rotation **range over every update** ≥
  the civilian gate's threshold (TASK-022: never two snapshots); body mesh base colour = source × `police_tint` /
  `swat_tint`. Wait for quiet updates before tint asserts (TASK-010 re-instance lesson). Run the client tests 3 times.

### Step 10 — runtime QA `tools/qa/scenarios/t11.py` (new, t9/t10 style: `--seed 1`, release, feature `dev`, `--out`)

1. Wait `Playing`, golden hash, chunks; stand the player on the hospital sidewalk; give the player a pistol by a named
   mutation of `Loadout` (confiscation observable; t9 armour precedent). Every constant read from the RON files
   (t10 `ron_number` helper).
2. **Busted run**: `world.mutate_resources` `WantedLevel.heat` → `stars[0].heat` (from `wanted.ron`). Poll
   `PoliceUnit` entities; on every poll assert active (state not `Leave`/`Dead`) ≤ `escalation.ron stars[0].units` and
   `PoliceDispatcher.units` ≤ it. Wait for a cop in `Arrest` within `arrest.distance`; the player stays passive (no
   keys). Poll `GameState` → `Busted`; during `BustedPhase::Screen` (after `busted_arrest` s) screenshot
   `busted.png` (≥ 0.15 s from any other capture). Poll `Playing`; read the player `Loadout` (no gun owned, `held`
   null, `has_bat` false), `Position` within 1 m (flat) of `PoliceStationSpawn.point`, `WantedLevel.heat == 0`.
   Record per cop the flat distance to the player every poll; a cop with no progress for 5 s while not in
   `Arrest`/`Attack` goes to `summary.json` `stuck` (with reach times), for the navmesh decision.
3. **SWAT run**: player armour 1e6 (named mutation), heat → `stars[3].heat`; poll until a `PoliceUnit` with kind
   `Swat` exists; on every poll `PoliceDispatcher.units <= stars[3].units` and `swat <= stars[3].swat`; `t6.aim_at` the
   nearest SWAT chest when within 30 m, screenshot `swat.png`. Record `frame_report()` (FPS only through it: 30 Hz
   monitor lesson).
4. `log_errors` empty, `shutdown`. The `stuck` list and reach times feed the navmesh decision written into
   QA_REPORT.md.

### Step 11 — config gates `crates/gta_sim/tests/config_police.rs` (new; `config.rs` is already 775 lines)

- Move `sabotaged` from `tests/config.rs` into `tests/common/mod.rs` (it already has `#![allow(dead_code)]`), split
  in two: `pub fn sabotaged_load<T: DeserializeOwned>(rel, tag, from, to) -> Result<T, ConfigError>` (the temp-dir
  copy + `replacen` + `load_config`, with the `GATE BROKEN: shipped {rel} has no {from:?}` assert) and `pub fn
  sabotaged<T>(rel, tag, from, to, validate) -> String` = `validate(&sabotaged_load(..).unwrap()).unwrap_err()`.
  `config.rs` imports it from `common`; its behaviour is unchanged.
- Gates, each flipped by its sabotage (values strictly on the failing side, TASK-007 lesson; each yields a different
  error than its neighbours):
  - shipped `escalation.ron` loads and validates, and `validate_ring(shipped despawn_distance)` passes;
  - unknown field (`replacen('(', "(bogus_field: 1.0,", 1)`) → `sabotaged_load` error names `escalation.ron` and
    `bogus_field`;
  - `stars[3]` `swat: 4` → `swat: 9` (> units 8) → error names `stars[3].swat`;
  - `stars[2]` `units: 6` → `units: 3` (< row 1's 4) → error names `stars[2].units`;
  - `break_free_distance: 3.0` → `1.2` (< distance 1.5) → error names `break_free_distance`;
  - `spawn_ring: (40.0, 90.0)` → `(40.0, 160.0)` with `|c| c.validate_ring(shipped despawn 150)` → names `spawn_ring`;
  - the fifth row line removed → `sabotaged_load` error contains `length 5`;
  - `wanted.ron` `kill_cop: 150` → `kill_cop: 0` → `heat.kill_cop must be > 0`; `respawn.ron` `busted_screen: 3.0` →
    `busted_screen: -1.0` → error names `busted_screen`.

### Step 12 — headless gates (production composition via `common::headless_app` / `city_app`)

**Fixtures `crates/gta_sim/tests/police_support/mod.rs`** (`#![allow(dead_code)]`):
- `raise_heat(app, heat)`: `set_heat` + `run_ticks(app, 1)` + assert `wanted(app).stars == stars_for(heat, ..)`
  (`GATE BROKEN` otherwise). **Rule:** in every test with a sidewalk graph (cop FSM on), call it before
  `spawn_unit`: `stars` is recomputed only in `track_search`, after `Decide`, and a unit whose first `police_fsm` run
  sees `stars == 0` goes to terminal `Leave`.
- `spawn_unit(app, kind, feet, yaw) -> Entity` through `police_unit_bundle` (shipped spec for `kind`, `Appearance(0)`);
  if a `SidewalkGraph` exists, assert `wanted.stars >= 1` first (`GATE BROKEN: raise_heat before spawn_unit`); run
  one tick and assert the flat position within 0.05 m of `feet` (TASK-011 lesson). Yaw convention: forward =
  `Quat::from_rotation_y(yaw)·(−Z)`, so facing +Z is `yaw = PI`, facing +X is `yaw = -FRAC_PI_2`.
- `set_cop_state`, `cop(app, e) -> PoliceUnit`, `esc(app) -> EscalationConfig`, `active(app) -> (u32, u32)` (units not
  `Dead`/`Leave`, of them SWAT), `assert_shipped(app)` with `GATE BROKEN` on every shipped number the worked examples
  use (the five rows, arrest block, gaits, `run_speed 4.5`, `sprint_speed 6.8`, `slots 4`,
  `despawn_offscreen_seconds 2`, `spawn_point_spacing 8`, star heats 40/180/550/1200/2400, search radii 40/70).
- Loadout comparison helper `assert_confiscated(loadout)`: every `guns[i].owned == false`, `held == None`,
  `has_bat == false` (`Loadout` has no `PartialEq`).

**`tests/police_arrest.rs`** — base app `graph_app(10.0, &[])` (player settled at the origin; the graph only switches
`NpcSystems` on), patrol cop via `spawn_unit` at feet (0, 0, −10), `yaw = PI` (facing +Z), after `raise_heat(40)`.
- **P1 `passive_player_is_busted_and_disarmed`** (AC 1, correctness): player holds a pistol (named `set_loadout`:
  magazine + reserve from `weapons.ron`) and `has_bat = true`. Liveness: cop `Arrest` within 4 ticks. Record tick `s`
  = the tick after which `ArrestAttempt.cop` is first `Some`. Worked: hold after tick `s` = 1/64; after tick `s + k` =
  (k+1)/64 (exact in f32: multiples of 1/64); `>= 1.5` first at k = 95 → `NextState::set(Busted)` in tick `s + 95`.
  Assert: after the update running tick `s + 94` hold == 95/64 exactly and state `Playing`; after the update running
  `s + 95` state still `Playing` (applied next `StateTransition`); after one more update `GameState::Busted` and
  `BustedPhase::Arrest`. Then `Arrest` for ⌈2.0·64⌉ = 128 real steps and `Screen` for 192, `Playing` after — derive
  the exact update offsets like `run_wasted` in `respawn.rs` (clock advances in the entering update) and assert them.
  After: `assert_confiscated`, flat distance to `PoliceStationSpawn.point` < 0.05 on the first `Playing` update,
  `WantedLevel` default, `Crimes` empty, no `Cuffed`, `ArrestAttempt` default. Flips: make `arrest_step` return
  `Busted` on the first in-range tick → the tick assert RED; remove the `Loadout` reset → RED.
- **P2 `cuffed_player_cannot_move`**: from P1 at `BustedPhase::Arrest`, `set_intent(axis = Vec2::Y, gait Run)`, 64
  updates → flat displacement < 0.05 m. Flip: drop `Has<Cuffed>` from `drive_characters` → RED.
- **P3 `breaking_free_adds_a_star`** (AC 4, correctness): P1 setup until hold > 0; player `MoveIntent { axis: Y, yaw:
  PI, gait: Sprint }` (moves +Z away from the cop at −Z; assert the player's z increases over the first 16 ticks,
  `GATE BROKEN` otherwise). Worked: gap grows at ≈ 6.8 − 4.5 = 2.3 m/s from ≤ 1.5 m to > 3.0 m in ≈ 1 s (≈ 67
  ticks incl. 0.15 s acceleration). Assert within 128 ticks: `heat == 180` exactly (from 40; `break_free_heat`),
  `stars == 2` at the end of that same update (`track_search` runs after `arrest_player`), `ArrestAttempt.cop ==
  None`, cop `Attack` within 4 more ticks, `GameState` never `Busted`. Flip: remove the `BrokeFree` branch → heat 40 →
  RED.
- **P4 `attacking_player_is_shot_not_arrested`** (1-star rule): P1 setup, `set_player_armor(1e6)`; when the cop is in
  `Arrest` at ≈ 5 m, `shoot_into_the_air` (cop within 15 m of the muzzle, 50 m LOS: heat 40 + 10 = 50, still 1 star).
  Assert cop `Attack` within 4 ticks, ≥ 1 cop `ShotFired` within 128 ticks, no `Busted` while hostile; after
  `hostile_seconds` = 320 ticks without attacks the cop is back in `Arrest`. Flip: ignore `hostile` in `next_state` →
  RED.
- **P5 `knocked_down_player_is_taken_at_once`**: cop in `Arrest` within 1.5 m, named mutation player `HitReaction::
  KnockedDown { left: 1.2 }` → `NextState` Busted set in the next tick, state `Busted` one update later (no 96-tick
  hold). Flip: drop the knockdown branch of `arrest_step` → RED (hold still running after 10 ticks).
- **P6 `lost_player_is_searched_at_last_known`** (AC 3). Fixture (revised, probe `scratch/pr2_p6_fixture.txt`):
  `headless_app()` + `test_graph(nodes [(-6,0,-10), (6,0,-10), (6,0,0), (-6,0,0)], edges (0,1),(1,2),(2,3),(3,0))` +
  `settle`; player at the origin, `set_player_armor(1e6)`, `raise_heat(180)`, patrol cop feet (−12, 0, 0),
  `yaw = -FRAC_PI_2` (facing +X). Spawn points: 4 nodes + 1 mid-point on each 12 m edge = 6. Liveness: cop
  `Attack`, `wanted.seen`. Named mutation: `place_player` feet (0, 0, 22). `L` = player position before the teleport.
  Worked: every point (x0, z0) with |x0| ≤ 6, z0 ≤ 0 sees (0, 22) through z = 14 at |x| ≤ 6·8/22 = 2.18 < 6 — behind
  the 4 m wall; the cop start (−12, 0, 0) crosses at x = −4.36 (inside); no edge touches a test-area block. Assert
  within 8 ticks: `seen == false`, `last_known` within 0.05 m of `L`, cop `Respond`; every tick until `Search`:
  `cop.dest == last_known`; the cop's flat distance to `L` ≤ 3.0 within 5 s (12 m at 4.5 m/s) and the cop enters
  `Search`; every search point lies within 70 m (`stars[1].search_radius`) of `L`; ≥ 2 distinct search points within
  20 s (farthest point 11.7 m from L at walk 1.8 m/s ≈ 6.5 s per leg); `GATE BROKEN` if `WantedLevel.seen` becomes true
  during the search phase (a re-sight is a fixture failure). Flip: Respond heads for the player → `dest` assert RED.
- **P7 `busted_drops_queued_damage`**, **P8 `busted_keeps_world_one_shot`** (`city_app(1)`, named mutation
  `NextState::set(Busted)`, `world_counts` pattern of `respawn.rs`): damage queued during Busted does not hit the
  respawned player; city/player counts unchanged after Busted → Playing.
- **P9 `death_beats_arrest_in_the_same_tick`**: P1 setup until the update running tick `s + 94` (hold = 95/64);
  named mutation `set_health(|h| h.current = 0.0)`; next update (tick `s + 95`: `detect_player_death` sets Wasted and
  inserts `Dead`, applied before `arrest_player`) → after one more update `GameState::Wasted`, never `Busted`. Flip:
  remove `Without<Dead>` from `arrest_player`'s player query → the hold completes in the same tick, `Busted` overwrites
  `Wasted` (last `NextState` writer) → RED. If P9 is RED **with** the filter (sync point absent), add `health.current
  <= 0 → return` to `arrest_player`, re-run, and record it.

**`tests/police_dispatch.rs`** — fixture: `headless_app()`, player feet (−30, 0, −30), `test_graph` edges A (30,0,−35)
–(30,0,35) and B (−35,0,35)–(25,0,35) (19 spawn points: A 2 nodes + 8 inner at z = −27 + 8k, B 2 nodes + 7 inner; all
60.1–88.5 m from the player, inside the 40–90 m ring), `set_population(max_civilians = 0)`, `set_player_armor(1e6)`.
Default view `set_view(Some(chase_view(feet, (−1,0,−1).normalize())))`: every point is behind the camera
(outside the 60° widened half-cone).
- **D1..D5 `units_follow_row_k`** (AC 2; one case per row, fresh app each): `set_heat(stars[k].heat)`; every tick for
  320 ticks assert active ≤ `units_k` and SWAT ≤ `swat_k`; by tick 64 active == `units_k` and SWAT == `swat_k`
  (liveness: 12 spawns at 1 per tick; without it "≤" is vacuous). Flip: `spawn_kind` compares `units > row.units` →
  one extra unit → RED.
- **D6 `spawns_stay_off_frame`** (revised): row 1 (`set_heat(40)`), view `chase_view(feet, (60,0,30).normalize())`
  (toward A's middle). Probe `scratch/pr2_d6_occlusion.txt`: A points z = −11, −3, 13, 21 and B (−35,35), (13,35),
  (21,35) are hidden (box / ramp / wall occlusion, or outside the cone) and legal; the three A points (30,0,−35),
  (30,0,−27), (30,0,−19) are in view with clear lines. Assert: 2 units spawn within 64 ticks (liveness) and no unit's
  first position lies within 0.5 m (flat) of those three visible points. Flip: skip the hidden check → the first spawn
  is the candidate nearest to `last_known` = the player, A (30,0,−27) at 60.07 m → RED.
- **D7 `lost_unit_is_replaced_after_reinforce_seconds`** (row 3, 10 s = 640 ticks): after 6 active, named mutation
  `set_health_of(unit, current = 0)` → active 5 for exactly the tick count derived from the dispatcher code (tick of
  `Added<Dead>` visibility + decrement-before-check order), then 6. Flip: no cooldown → RED.
- **D8 `cleared_wanted_sends_units_away`**: row 1 reached, `set_heat(0)` → all `Leave` within 2 ticks (tick 1:
  `track_search` resets `stars`; tick 2: FSM), all despawned within 130 ticks (off-frame since spawn). Flip: skip the
  `Leave` despawn → RED.
- **D9 `reset_units_do_not_rejoin`** (AC 2 after a reset): row 5 reached (12 active); then
  `set_view(chase_view(feet, (1,0,1).normalize()))` so every unit is in view (`Offscreen` stays 0, nobody despawns);
  `set_heat(0)` → all `Leave`; `set_heat(40)` (row 1). For 320 ticks: active ≤ 2 and no SWAT among units not in
  `Leave`; `GATE BROKEN` if any of the 12 old units despawns (they must stay alive for the gate to mean anything).
  Flip: `Leave → Respond` in `next_state` → 12 active → RED.
- Unit tests in `police/fsm.rs` (shipped RON via `include_str!`, `GATE BROKEN` on the numbers used):
  `next_state_table` (every state × sees/hostile × each shipped row's `arrest` flag; `Leave` × every sense → `Leave`;
  `Dead` → `Dead`; stars 0 → `Leave`), `spawn_kind_table` (per row), `break_free_heat_table` (rows = shipped
  `wanted.ron` stars): 40→180, 179→180 (1★), 39→39 (0★), 180→550, 300→550, 550→1200, 1200→2400, 2400→2400 (5★),
  5000→5000; `pick_spawn_table` (nearest; surround with taken bearing 0° → picks the candidate at 180° over a nearer
  one at 10°); `arrest_step_table` (hold accumulates to exactly 1.5 at the 96th step; paused between 1.5 and 3.0; 3.01
  → `BrokeFree`; knockdown within 1.5 → `Busted`; attacking → `Hold(0.0)`).

**`tests/police_crimes.rs`** — test floor **without** a sidewalk graph: `NpcSystems` is off by design (`navigation/
mod.rs:347-349`), so the cop FSM and `police_death` do not run and the cop stays put; `record_crimes` is
`PlayingSystems` and runs (comment says so; nobody adds a graph). Cops through `spawn_unit` (full body, `PoliceUnit`,
`Health`). Falsifiable because a named config mutation `cop_witness_distance = 0.5` is below the actual eye distance,
so only the "always" rule can report:
- **C1 punch** (unarmed, cop 1.2 m in front, one click): heat 45, one reported `PunchCop`.
- **C2 wound** (pistol at 10 m, full health): heat 80 (the `Shooting` incident stays unreported), `WoundCop` reported.
- **C3 kill** (cop health 1): heat 150, `KillCop`. Flip for C1..C3: drop the always-branch → heat 0 → RED.
- **C4 `cop_witnesses_shooting_near_itself`**: shipped witness distance, cop 8 m away, `shoot_into_the_air` → heat 10.

**Corridor gates** (TASK-010 QA Bug 1, both roles):
- Gang, in `tests/gang_fire_lines.rs` (the layout runner and `Layout.walls` exist): **G-C1..G-C4** corridors 2.4 and
  3.0 m (walls x = ±1.35 / ±1.65, 0.3 thick, 14 m long, z −20..−6, QA3 layout), orders SMG-front/pistol-rear and
  pistol-front/SMG-rear, each its own `assert_keeps_firing` (≥ `MIN_SHOTS` 6 in 30 s, 0 friendly/bystander damage);
  plus one 18 m rear variant (QA3 V6 layout). Measured before the fix: rear member 0 shots.
- Police, `tests/police_fire_lines.rs` (graph present, `raise_heat(180)` first): **P-C1..P-C4** the same corridors with
  two cops through `spawn_unit` (patrol+patrol, SWAT front + patrol rear), `set_cop_state(Attack)`, armour 1e6, 1920
  ticks: each cop ≥ 6 shots, 0 cop→cop damage. **P-B1 `cops_never_hit_bystanders`**: the gang "dummies + idle rival"
  and "sidewalk between" layouts with cops: 0 damage to dummies, civilians, gang members. Flip: `spares` returns false
  for everyone → RED.
- Flip for every corridor case: remove the `queue_slot` call → rear 0 shots → RED.

**`tests/police_city.rs`** (seed 1):
- **N1 `cops_reach_the_player_in_the_city`** (navmesh evidence, liveness only): three sidewalk spots (hospital spawn,
  plaza, park centre), heat 180, view fixed, `max_civilians = 0`, 40 s each. Assert only that at least one cop reaches
  `sees` or ≤ `keep_distance.1` at each spot. Print per-cop reach time or "stuck at d m". The implementer copies the
  printout into its summary; a stuck cop is a finding for the navmesh decision (new task), not a red build.
- **N2 `station_spawn_exists`**: `PoliceStationSpawn` lies within 30 m of the first station building's centre. (The
  seed sweep lives in citygen, Step 6.)

**`tests/police_bench.rs`**: seed 1, `max_civilians 40`, heat 2400 (12 SWAT), armour 1e6, 640 observed ticks after the
units exist; print mean/p50/p95/max; assert only the mean (TASK-009 lesson) against 10× the probe mean the implementer
measures once (record the probe number in the const's comment, `civilian_bench.rs` style).

Existing tests kept green, touched only where needed: `crimes.rs` units (`HEAT` literal), `respawn.rs`, `wanted*.rs`
(fixtures lack `PoliceUnit` and `Health`, so nothing changes), all gang tests (Step 2 regression net),
`civilian_bench.rs` (watch item: it fires fake `ShotFired` near civilians under a view; if cops now change its outcome,
pin heat to 0 by a named mutation — never loosen its assertions).

### Step 13 — verification

`cargo build -j 4`; `cargo build -p gta_like --features dev -j 4`; `cargo clippy --workspace --all-targets -j 4 --
-D warnings`; `cargo test -p gta_sim -j 4`; `cargo test -p citygen -j 4`; `cargo test -p gta_like --bin gta_like -j 4`
×3; `cargo tree -p gta_sim -e normal -i bevy_render` empty; `python tools/qa/tree_check.py`;
`python tools/qa/scenarios/t11.py --out <dir>`; **re-run `t10.py` and `t9.py`** (cops now exist in those runs; adapt
only by named mutation, e.g. player armour or heat reset). Run each exact-tick gate (P1, P3, P9, D7) 3 times to show
determinism. Phantom-red rule: `touch crates/*/src/lib.rs` and rebuild before trusting red in untouched code. Record
every flip (perturbed input, RED, GREEN) in the implementer summary. File sizes: every touched/new `.rs` < 750 lines
(hard limit 950).

---

## 3. Test plan (summary of what carries which claim)

| Claim | Gate | Class | Expected |
|---|---|---|---|
| AC1: 1★ + passive 1.5 s → Busted, disarmed | P1 (+P2, P5, P9) | correctness, exact tick | Busted set at tick s+95, visible one update later; loadout confiscated, station, wanted reset |
| AC2: units ≤ table | D1..D5, D9, t11 polls | correctness + liveness | ≤ row every tick, row reached by tick 64; no re-engagement after reset |
| AC3: lost → LastKnownPosition | P6 | correctness | `dest == last_known`, ≤ 3 m in 5 s, Search ≥ 2 points |
| AC4: break free → +1★ | P3, `break_free_heat_table` | correctness | heat 40 → 180, stars 2, cops Attack |
| Off-frame spawns | D6 | correctness | no spawn at visible points |
| Reinforcement / stand-down | D7, D8 | correctness | exact cooldown; Leave + despawn |
| Cop crimes always reported | C1..C4, `classify_table` | correctness | 45 / 80 / 150 / 10 |
| Corridor starvation fixed | G-C*, P-C*, P-B1 | liveness + safety | ≥ 6 shots each, 0 friendly damage |
| Gang unchanged by the move | existing gang suites after Step 2 | regression | green before Step 3 |
| Config strictness | `config_police.rs`, `stars_table` etc. | correctness | each sabotage its own error |
| Station anchor | citygen `police_station_anchor_on_sidewalk`, N2 | property | every station anchors, all seeds |
| Frame budget | `police_bench.rs` | perf (mean only) | < 10× probe mean |
| Navmesh decision | N1 + t11 `stuck` | evidence | printed, not gated |
| Looks, BUSTED, pose | `police_gate.rs` ×3, t11 screenshots, owner run | presentation / owner | leg range, tint; owner checklist |

### Owner checklist (QA writes it into QA_REPORT.md, in Russian)

"Полная петля": `cargo run --release -- --seed 1`, взять пистолет.
- [ ] Набедокурить при свидетеле → 1 звезда → через ~10-20 с приходят 2 синих копа со стволами.
- [ ] Стоять спокойно: коп подходит вплотную, через 1.5 с — сцена ареста (игрок на коленях), экран BUSTED 3 с,
      появление у участка без оружия, без брони и без звёзд.
- [ ] На 1 звезде, когда коп вплотную, рвануть спринтом прочь → 2 звезды, копы открывают огонь. Понятно ли, что
      это "вырвался"?
- [ ] Выстрел рядом с копом на 1 звезде → копы стреляют; через 5 с спокойствия снова пытаются арестовать.
- [ ] Спрятаться за домом: звёзды мигают, копы идут к последней точке и бродят по кругу; уйти за круг → розыск спадает.
- [ ] Поднять до 4 звёзд (убийство копа = 150): тёмные SWAT с SMG, держатся ближе. Смерть → ПОТРАЧЕНО, как раньше.
- [ ] Копы не стреляют сквозь прохожих и друг друга, не толпятся гуськом в узком проходе.
- [ ] Отличаются ли копы (синий тинт) от мирных, особенно от голубоватого мирного на модели male-c?
- [ ] После ареста или смерти старые копы уходят и не возвращаются; новая звезда приводит новых копов по таблице.
- [ ] На 5 звёздах после 4: 4 патрульных остаются, добавляются SWAT (8 SWAT + 4 патруля, а не 12 SWAT) — нормально?
- [ ] Ручки: `assets/police/escalation.ron` (число копов, дистанции, 1.5 с, 3 м), `wanted.ron` (heat за копов),
      `respawn.ron` (2 с + 3 с), `visual.ron` (цвета), `strings.ron` (BUSTED).

---

## 4. Rollout notes

- No migrations, no env vars, no feature flags, no Cargo changes. New data file `assets/police/escalation.ron`; new
  fields in `wanted.ron`, `respawn.ron`, `strings.ron`, `visual.ron` (strict loaders: all four must ship together
  with the code, an old RON fails to load with a named field).
- `GameState` gains `Busted`: every `match` over `GameState` in the client must handle it (compile error otherwise).
  `NpcSystems` now also runs in `Busted` (cops act during the 5 s; `fire_weapons`/`swing_melee`/dispatcher are
  `PlayingSystems`, so nobody shoots or punches and no reinforcements come).
- Behaviour changes visible in existing runtime QA: t9/t10 now spawn real cops when heat rises (re-run, §Step 13).
- Risks kept (owner-visible, not gated): NPC `fire_requested` set during Busted fires once on the first Playing tick
  at the old spot (player already moved); three shooters in a 2.4 m file may still starve (only pairs gated;
  corridors < 2.2 m cannot hold two bodies with a clear line); route search budget (`route_requests_per_tick 2`) is
  shared with gangs at 12 cops (N1 / t11 evidence); feel of 1.5 m / 1.5 s, SWAT tint, BUSTED colour, cops walking
  off.
- Q2=B off; break free = farther than 3 m from the arresting cop during the hold; wanted and crimes reset on exit from
  Busted; "fewer civilians at 5 stars" deferred (TASK_FINAL "Resolved questions"). No open questions.

---

## 5. Review notes (plan-reviewer-2)

**Disconfirmation test (done first).** Counter-example chosen: "the plan's worked numbers contradict the real
thresholds / tick rate". **It held for one row:** `wanted.ron` puts 1★ at heat 40, so heat 179 is 1★ (`stars_table`
in `wanted/mod.rs` asserts `(179, 1)`); PLAN's `break_free_heat_table` row "179 (0★) → 179" is wrong (correct: 179 →
180), and V2 did not fix it. The 64 Hz tick (96 ticks = 1.5 s) and 40 = 1★ held. The search surfaced the defects
below.

Changed from PLAN_V2 (each verified in code or by a probe under `scratch/`):

1. **P9 flip was vacuous.** V2 added a `health.current <= 0` check and flips P9 by removing it. `bevy_ecs 0.19.1`
   auto-inserts `apply_deferred` (`schedule.rs:1629`, default `true`) and there is an ordering path
   `HealthSystems::Death → Perceive → Decide → arrest_player`, so `detect_player_death`'s `Dead` insert is applied
   before `arrest_player` and its `Without<Dead>` filter already skips the dying player: removing the health check stays
   GREEN. Final: one guard (`Without<Dead>`), P9 flips by removing it; fallback recorded if the sync point turns out
   absent.
2. **D6 fixture was wrong on correct code.** "View toward A, open floor, no occluder" is false: the 5 m box, the ramp
   and the wall occlude A points z = −11, −3, 13, 21 from that camera (probe `scratch/pr2_d6_occlusion.py/.txt`), so
   they are legal off-frame spawns, and A (30, −11) at 62.9 m is nearer to `last_known` than any hidden B point
   (65.2 m). "Every unit on B" would be RED by construction. Final: assert no spawn at the three clearly visible A
   points; flip lands the first spawn at A (30, −27).
3. **P6 search square went through test-area geometry.** V2's nodes (±12, 0, −24) put the z = −24 edge through the
   4×5×4 box (x ∈ [−12, −8], z ∈ [−24.66, −20.66]) and the x = −12 edge along the box and ramp (probe
   `scratch/pr2_p6_fixture.txt`): a search point inside the box is unreachable, so "≥ 2 points in 20 s" fails. Final
   square x ∈ [−6, 6], z ∈ [−10, 0]: no block touched, wall still hides the player from every point.
4. **Terminal `Leave` + fixtures that set heat and spawn in the same update.** `set_heat` writes only `heat`; `stars`
   is recomputed in `track_search` after `Decide`, so a freshly spawned test cop sees `stars == 0` on its first FSM run
   and leaves for good (P1..P6, P-C*, P-B1 would all fail on correct code). Final: `raise_heat` helper + `spawn_unit`
   precondition. Rejected alternative: computing stars inside `police_fsm` (a second source of `stars`).
5. **D9 raced the despawn.** Units spawned behind the camera have been off-frame since spawn; once `Leave` they
   despawn almost at once (`Offscreen` ≥ 2 s), so "heat → 40 while the old units are still alive" depended on timing
   and the flip could go vacuous. Final: turn the view onto the units before the reset + `GATE BROKEN` if any
   despawns.
6. **`Loadout == Loadout::default()` does not compile** (`Loadout` has no `PartialEq`): field-wise
   `assert_confiscated`.
7. **Parse-level config gates cannot use `sabotaged`** (it unwraps the load): split into `sabotaged_load` +
   `sabotaged` in `tests/common`.
8. **Station property flip** used an out-of-range index (perturbs the test's input, not the anchor search); final
   flip = huge margin through the shared helper.
9. `break_free_heat_table` corrected (179 → 180 at 1★, 39 → 39 at 0★, 5000 → 5000 added); P3 "next tick stars == 2"
   corrected to "same update" (`track_search` runs after `arrest_player`).
10. Precision added where V2 said "as PLAN": exact `police_unit_bundle` loadout via `acquire`, yaw convention
    (`PI` = +Z, `-FRAC_PI_2` = +X), P1 tick arithmetic (NextState at tick s+95, `Busted` visible one update later),
    Busted strips armour via `Health::full` (existing behaviour, owner line), t11 counts only active units.

Kept from PLAN_V2 (verified): `PoliceSystems.after(WantedSystems)` edge; `Leave` terminal; N1 as evidence, not a hard
gate; station property in the citygen sweep; t9/t10 re-runs; `hold_fire` preservation details; queue slot only on the
non-pinned fallback; P6 wall-crossing argument (|x| ≤ 12·8/22 holds, only the square moved); crime gates without a
graph; plugin tuple 14 ≤ 15; `band_move` already `pub`. Line references in `gang/behavior.rs` and `fire_line.rs`
re-checked: correct.

Research: engine behaviour settled in the pinned sources (cited above), which outrank web docs for this project;
genre references unchanged from PLAN_V2 §7 (GTA IV arrest/break-free, strip of weapons and armour — community wikis,
secondary). No open uncertainty remains that a web source would settle.

Evidence on disk: `scratch/corridor_geometry.{py,txt}` (planner), `scratch/pr2_d6_occlusion.{py,txt}`,
`scratch/pr2_p6_fixture.{py,txt}` (this review). Decisions: `log.jsonl` (plan-reviewer-2 entries). Context
proposal: `PCTX_PROPOSALS.md`.

children: 0 launched / 0 reported.
