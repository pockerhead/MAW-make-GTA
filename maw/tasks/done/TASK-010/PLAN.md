# PLAN — TASK-010 (GDD T9): gangs

Cost of error, one line: **mixed.** Silent class (a matrix leak that starts gang wars, a group radius or heat timer that
drifts, gangs hostile outside their turf, a group popping into view, dropped guns piling up, the gang spawner starving or
shifting the civilian spawner) gets headless gates. Owner class (tints, how the warning reads, firefight feel, accuracy
feel, retreat look) gets the owner checklist plus BRP evidence, no machinery.

Pinned versions (`Cargo.lock`, sources under `~/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/`): bevy / bevy_ecs /
bevy_app / bevy_reflect / bevy_remote 0.19.1, avian3d 0.7.0, bevy-tnua 0.32.0, vendored bevy-tnua-avian3d 0.12.1,
rand_chacha 0.10.0, ron 0.12.2. **No new crate, no `Cargo.lock` change.** `pathfinding` stays out: gang members move by
direct seek toward a point they saw (GDD §6.6 "при прямой видимости ≤ 25 м прямой seek"); A* arrives with police (T11).
`citygen` is **not touched**: it already produces both gang districts and HQ buildings (PREMISE_CHALLENGE.md).

Evidence written by this planner (all under `maw/tasks/in_progress/TASK-010/scratch/`, nothing in the checkout touched):
- `probe_territory/` (read-only probe over `citygen::generate`, built with `CARGO_TARGET_DIR=D:/test-gta-like/target`,
  `cargo run --release --offline`). Output, seeds 1 / 2 / 42:
  - 140 / 135 / 140 blocks, **all curb polygons counter-clockwise** (so `citygen::contains_convex` is valid on them), 568
    curb vertices; `nearest_block` over all blocks costs **3.3 µs per query** (release).
  - Sidewalk nodes: 579 (seed 1); 38 of them lie in no curb polygon (alley walks run on the curb line), none in two.
  - Seed 1: gang 0 = district 3 (Residential, 16 blocks, HQ anchor (115.8, −485.7)), gang 1 = district 0 (Residential,
    19 blocks, HQ anchor (−491.3, −307.6)); `sidewalk_anchor(margin 3.0)` exists for every HQ of the three seeds; posts at
    25 m spacing: 31 / 32 per gang; **nearest gang post to the seed-1 player spawn: 135.1 m** (spawn is in district 4).

Research (web, 2026-09-24):
- Utility selection with a health input that makes "retreat" win below ~30% health, and ranged NPCs holding a band while
  melee NPCs close in, is the standard shape (https://recited.io/kb/ai-in-game-development/npc-behavior-and-intelligence/utility-based-ai-systems/ ,
  https://theneuralbase.com/ai-for-gaming/learn/beginner/utility-ai/ ; GDD §6.5 already cites Game AI Pro ch. 4:
  FSM + small scorer). Taken: enum FSM per member, a four-action scorer only inside `Attack`.
- Faction systems: a baseline relation table between factions, with player actions changing hostility over time
  (https://www.thegamebeyond.com/blog/2026/8/26-2 , https://github.com/matiaszanolli/ByroRedux/issues/4414 — "arming combat
  AI from faction relations to determine enemies"). Taken: a static symmetric table in `gangs.ron` (the baseline) plus
  `GangHeat` (the player-driven part), exactly GDD §6.3.
- GTA SA reference (community wiki, secondary): a war starts after killing gang members on their turf
  (https://www.grandtheftwiki.com/Gang_Warfare_in_GTA_San_Andreas , https://gta.fandom.com/wiki/Gang_Warfare_in_GTA_San_Andreas).
  Reference only; territory capture is out of scope (GDD §6.3).

---

## 1. Understanding (what exists today)

**Composition.** `crates/gta_sim/src/lib.rs:36-129` `compose_sim` loads every config with `load_config` + `validate`,
inserts them, then adds 12 plugins in one tuple (the tuple impl goes to 15, `bevy_app-0.19.1/src/plugin.rs:186-192`).
`combat_seed` = city seed or 0 (`lib.rs:88-91`).

**Flow and sets.** `flow/mod.rs:31-40`: `PlayingSystems` (Playing only) and `NpcSystems` (Playing or Wasted). Navigation adds
`NpcSystems.run_if(resource_exists::<SidewalkGraph>)` (`navigation/mod.rs:184-187`). Perception chains
`(AiSystems::Perceive, AiSystems::Decide, PopulationSystems).in_set(NpcSystems).after(HealthSystems::Death)` and
`AiSystems::Decide.before(TnuaUserControlsSystems)` (`perception/mod.rs:442-452`). Health sets are chained
Damage → Regen → Pickup → Death (`character/mod.rs:226-235`). All combat systems (`tick_loadouts`, melee, `fire_weapons`,
pickups, `dummy_life`) are `PlayingSystems` (`combat/mod.rs:312-337`). So in one fixed tick: weapons fire and write
`ShotFired`/`DamageDealt`/`MeleeHit` → deaths → perceive → decide (writes intents consumed next tick) → population.

**Characters.** One archetype `Character` (`character/mod.rs:165-176`), `character_components` (`:262-286`).
`drive_characters` (`:288-358`): `aim.aiming` makes the body face the aim yaw and caps the gait at `aim_max_gait`; a zero
`MoveIntent.axis` gives `desired_forward: None` (Tnua keeps the facing). `Health::take` returns true on the lethal hit
(`health.rs:102-108`).

**Weapons are NPC-ready.** `fire_weapons` (`combat/hitscan.rs:126-274`) serves any `Character` with `Loadout`, `AimIntent`,
`ActionIntent`: ray 1 from `aim.origin` along `aim.direction` (own colliders skipped by the `visible` predicate, `:193-201`),
ray 2 from the muzzle; spread from `Loadout.spread_deg`; auto-reload on an empty pull (`:177-182`). `swing_melee`
(`melee.rs:385-491`) needs `&Loadout` and swings when `held` is `None`; it takes the blow direction from the flat
`aim.direction`. `select_weapon` (`weapons.rs:352-376`) applies `ActionIntent.select`; `Unarmed` while already unarmed
toggles fists/bat only if `has_bat`. `acquire` (`weapons.rs:272-283`) gives a picked-up gun `min(magazine, pickup_ammo)`
plus the rest as reserve. `cone_sample` and `aim_yaw` are public (`combat/mod.rs:239-242`).

**Pickups.** `WeaponPickup { weapon, ammo_only, cooldown }` (`pickups.rs:85-99`), `collect_weapon_pickups` (`:101-130`) gives
the player the gun and puts the pickup on `cooldown = pickups.respawn` — built for the fixed range, a dropped gun would
respawn forever. The client already draws any new `WeaponPickup` through the observer `visualize_weapon_pickup`
(`src/visuals/mod.rs:105`, `weapons.rs:59`).

**NPC stack from T8 / TASK-022.**
- `perception/mod.rs`: `Perception { slot, pending }` with a slot assigned by the observer `assign_slot` (`:462-473`),
  `AiClock` advanced first in `Perceive` (`:475-477`), `sight_blocked` (`:414-422`, World-mask ray). `perceive` and
  `collect_stimuli` query `&Civilian` (`:489`, `:528`), so any non-civilian `Perception` is ignored by them. The aimers
  query (`:530`) is every live `(AimIntent, Loadout)` holder: an aiming gangster scares civilians with no change.
- `population/mod.rs` (612 lines): `CameraView(Option<ViewCone>)` (`:166-168`), `outside_cone` (`:239-242`), `occluded`
  with 4 rays per point (`:244-271`), `corpse_components` (`:228-236`), `Offscreen`, `Corpse`, `Appearance`, `NpcRng`
  (stream 1, `:203-222`). `PopulationPlugin` runs `(age_corpses, despawn_far, spawn_civilians).chain()` in
  `PopulationSystems` (`:291-296`). `spawn_civilians` resets `PopulationLoad` at its start (`:434`), spends at most
  `occlusion_rays_per_tick` (`:500-503`) and calls `rng.unit()` once per candidate that survives its filters (`:479-486`).
  `age_corpses` handles every `Corpse` (`:300-321`); `despawn_far` handles only `Civilian` (`:323-380`).
- `civilian/mod.rs`: the pattern to mirror — `#[require(Character, Perception, Offscreen)]` (`:140-146`), a pure
  `next_state` per state (`:270-326`), `civilian_death` in `HealthSystems::Death` + `NpcSystems` (`:195-226`) that
  checks the **state field** (the `Dead` insert is deferred).
- `navigation/mod.rs`: `steer`, `flat_distance` (`:111-118`, `:170-177`), `NavigationConfig.arrive_radius`.

**World.** `world/city.rs:171-213` builds the city; the hospital anchor uses `citygen::sidewalk_anchor`
(`city.rs:215-236`) and exits the app on failure. `City(CityLayout)` and `CityParamsRes` are resources. Layout (x, y) is
world (x, z). `CityLayout.gang_districts: [u32; 2]`, `BuildingKind::GangHq(u8)` (`citygen/src/layout.rs:69,143`), blocks
carry `district` and a convex CCW `curb` (probe). No territory, faction or gang code exists in `gta_sim` yet (only
`world/mod.rs:9` re-exports `BuildingKind`, which has `GangHq`).

**Client.** `src/visuals/character.rs` (601 lines): one animation graph and scene per model, keyed by `ModelKey`
(0 = player model, 1.. = `civilian_models`), each built from its own GLB (`:150-190`; the TASK-009 root-name lesson).
`spawn_character_model` picks the key from `Appearance` of a `Civilian` (`:231-264`), `on_model_ready` tints the
`tinted_mesh` (`:266-332`). Death/knockdown/arm layers are generic over `Loadout`/`Dead` (`:412-518`). Held gun:
`attach_held_gun` / `show_held_gun` (`src/visuals/weapons.rs:102-177`) exist only for `With<Player>`. VFX tracers and
flashes already work for any shooter that has a `HeldGun` (`src/vfx/mod.rs:18-23`). `src/main.rs:64-146` `preflight`
checks every model against the manifest. The manifest rig lists all 12 Kenney models (`assets/third_party/manifest.ron:74-87`);
unused so far: female-c, female-e, female-f, male-d, male-f.

**QA.** `tools/qa/brp.py` (Game, `query`, `mutate_component`, `send_mouse_button`, `move_mouse`), helpers in
`scenarios/t5.py` (`resource_value`, `screenshot`, `log_errors`, `wait_chunks`, `game_state`), `t6.py` (`rows`, `player`,
`teleport`, `weapon_pickups`, `camera_config`), `t8.py` (`variant`, `scalar`, `horizontal`). `OrbitCamera { yaw, pitch, .. }`
is a reflected component (`src/camera/mod.rs:18-27`); a reflect path's leading dot is optional
(`bevy_reflect-0.19.1/src/path/mod.rs:128`), so `"yaw"` addresses the field.

---

## 2. Approach

A new `gang/` domain plugin (GDD §12: territories, hostility, faction matrix, gang FSM) and a gang spawner in
`population/` (GDD §12: bubble, spawn/despawn, caps). Everything reuses T8 machinery instead of duplicating it:

1. **Territory from the generator.** `GangTerritories` is built once on `Loading → Playing` from `City`: per gang the HQ
   post (`sidewalk_anchor` of the `GangHq(g)` building) and block-corner posts (sidewalk nodes of the gang's district,
   ≥ `post_spacing` apart). `territory_at(p)` = gang of the **nearest block** (0 inside its curb polygon). This matches how
   `citygen` assigned blocks and how T12 will paint territory; Voronoi on district seeds disagrees at borders.
   Tests insert a synthetic `GangTerritories` on the test floor.
2. **Factions are data.** `Faction { Player, Gang(u8), Police }` component (player gets `Player`, members get `Gang(g)`;
   `Police` exists only so the matrix can state the off pairs; T11 attaches it to cops). The matrix is an explicit,
   complete, symmetric list of pairs in `gangs.ron` with gang↔gang and gang↔police **off** (Q2 = A). Same faction is never
   hostile (law in code).
3. **Hostility = provocation + memory.** A provocation is (a) a `ShotFired` of a hostile faction whose muzzle is within
   `shot_radius` of a live member **and inside that gang's territory**, or (b) a `DamageDealt` on a member by a hostile
   faction (any position: self-defence; see Open question 1). It turns every same-gang member within `group_radius` (30 m)
   of the provoked member into `Attack { target: attacker }` **in the same tick**, and, when the attacker is the player,
   sets `GangHeat[g] = heat_seconds` (120 s, linear seconds-left timer, decays in fixed time). Warnings (player in the
   turf closer than 8 m for 3 s, or aiming at a member) and heat-driven attack on sight need the **player** inside the
   gang's territory: "вне территории нейтральны".
4. **FSM + small scorer.** `GangState { Idle, Warn, Attack { target }, Retreat { from }, Dead }` (GDD §6.5 roles) with a pure
   `next_state` table; inside `Attack` a four-way utility pick `Retreat / Melee / Shoot / Chase` with data weights. Shoot
   keeps the 8-15 m band, aims at the chest with an extra `aim_error_deg` cone (worse than police, T11 sets theirs),
   pulls the trigger every `trigger_seconds`. Melee holsters and punches inside 1.5 m. Retreat below 30 % health runs to
   25 m and holds there, shooting. Movement is direct seek to a visible target or to where it was last seen; LOS is
   refreshed on the member's AI slot (`Perception.slot`, the existing `AiTickSlot` mechanism).
5. **All combat through intents.** The FSM writes only `MoveIntent`, `AimIntent`, `ActionIntent` (`select`,
   `fire_requested`), consumed next tick by the unchanged `fire_weapons`/`swing_melee`/`drive_characters`. Hits, knockback,
   death animations, tracers, civilian panic come for free.
6. **Weapon drop.** On death a member's gun leaves the body (`Loadout` cleared) and becomes a one-shot `WeaponPickup` with
   a `Dropped { left }` lifetime; `collect_weapon_pickups` despawns a dropped pickup when taken instead of cooling it down.
7. **Spawner shares the bubble rule.** `population/gangs.rs` spawns whole groups (2-4) at free posts in `spawn_ring`,
   only if every member spot is outside the camera cone (+margin) or occluded (`outside_cone` / `occluded`, the same
   functions civilians use), within the ray budget civilians left this tick, under `max_gang_members` (12). Far + 2 s
   off-frame despawn, same rule and numbers as civilians. A post is occupied while any member of its group (alive or
   corpse) exists, so a wiped group comes back only after its corpses are gone (30 s) and the player is 30+ m away and
   not looking.
8. **Isolation from the fragile civilian gates.** Gangs roll from their own `GangRng` (ChaCha stream 2) and never touch
   `NpcRng`, and the civilian spawner is left untouched: any extra draw or filter would shift the seed-1
   `street_ahead_stays_populated` density gate, whose route enters gang 0 territory (district 3 lies straight north of the
   spawn). Group spots sit 1.0 m around a corner node, a civilian spawn point is the node itself or ≥ 4 m inside an edge,
   so they never overlap.
9. **Client.** Gang models get their own per-model graphs (the root-name lesson, same code path as civilians), one tint
   per gang, and every armed character (not only the player) gets the held-gun mesh so a gangster visibly draws.

Rejected: gangs inside `perceive` (the stimulus log has no shooter, so it cannot tell a friendly shot from the player's);
Voronoi territory (border mismatch with block colours); a separate ray-budget knob for gangs (second source for one
limit); per-post respawn cooldown knob (corpse lifetime already gives it); a gang bench (≤ 12 agents, bounded rays; the
existing civilian bench and the ray-budget assertion carry the perf claim — declining is the finding).

### Verified API facts used

- `Entity` derives `Reflect` (opaque, Serialize with `serialize`) (`bevy_ecs-0.19.1/src/entity/mod.rs:413-420`), so
  `GangState::Attack { target: Entity }` is BRP-readable; `ReflectComponent` needs only `FromReflect`
  (`bevy_ecs-0.19.1/src/reflect/component.rs:146,309`).
- `resource_exists` (`bevy_ecs-0.19.1/src/schedule/condition.rs:730`); set-level run conditions AND together.
- Plugin tuple ≤ 15 (`bevy_app-0.19.1/src/plugin.rs:186-192`); the tuple grows 12 → 13.
- `ChaCha8Rng::set_stream`, `unit_f32`, `cone_sample`, `aim_yaw`, `sight_blocked`, `outside_cone`, `occluded`,
  `corpse_components`, `sidewalk_anchor`, `contains_convex`, `dist_point_segment` — all in the pinned code (lines above).

### Conventions and worked directional examples (sign errors pass magnitude tests)

Yaw convention (GDD §3.2): `forward(yaw) = R_y(yaw)·(0,0,−1) = (−sin yaw, 0, −cos yaw)`; `aim_yaw(d) = atan2(−d.x, −d.z)`.
Used for (a) a member facing its post centre at spawn, (b) `MoveIntent.yaw` via `steer`, (c) QA pointing `OrbitCamera.yaw`.
1. d = (0,0,−1): yaw = atan2(0, 1) = 0 → forward (0,0,−1). ✓
2. d = (−1,0,0): yaw = atan2(1, 0) = +90° → (−1,0,0). ✓
3. d = (0,0,+1): yaw = atan2(0, −1) = 180° → (0,0,+1). ✓ (d = (+1,0,0) → −90° → (1,0,0).)
Group spot k of n: `post + (cos θ, 0, −sin θ)·spread`, θ = 2πk/n; facing = `aim_yaw(post − spot)` (member k=0 of a pair at
post + (1,0,0) faces −X: aim_yaw((−1,0,0)) = +90° ✓).
Back-off: move yaw = `steer(target, self)` (away), with `aiming = true` the body keeps facing the target (strafe).

---

## 3. Steps

Order = dependency order; each step names its check.

### Data (every tuning value introduced here; no new tuning `const`)

1. **`assets/gang/gangs.ron`** (new):
   ```
   (
       // Exactly two gangs: citygen assigns two territories. Tints live in character/visual.ron.
       gangs: [
           (weapons: [Pistol, Smg]),
           (weapons: [Pistol, Shotgun]),
       ],
       // Faction matrix, symmetric (GDD §6.3, Q2 = A): gangs are hostile only to the player in the MVP.
       factions: [
           (a: Gang(0), b: Player, hostile: true),
           (a: Gang(1), b: Player, hostile: true),
           (a: Gang(0), b: Gang(1), hostile: false),
           (a: Gang(0), b: Police, hostile: false),
           (a: Gang(1), b: Police, hostile: false),
       ],
       groups: (
           size: (2, 4),           // members per group (GDD §6.3)
           spread: 1.0,            // m from the post to each member
           post_spacing: 25.0,     // m between posts (HQ first, then block corners of the territory)
           hq_margin: 3.0,         // m: the HQ post sits on a sidewalk side at least twice this long
       ),
       hostility: (
           warn_distance: 8.0,          // m (GDD §6.3)
           warn_seconds: 3.0,           // s closer than warn_distance before a warning (GDD §6.3)
           warn_release_distance: 12.0, // m: a warning ends beyond this
           warn_keep_distance: 3.0,     // m: warning members walk up to this
           shot_radius: 15.0,           // m from the muzzle: "shoots nearby" (GDD §6.4 uses 15 m for the same act)
           group_radius: 30.0,          // m from the provoked member (GDD §6.3)
           heat_seconds: 120.0,         // GangHeat memory (GDD §6.3)
           sight_distance: 40.0,        // m: while GangHeat > 0 members attack the player on sight in their territory
           leash_distance: 60.0,        // m from the member's post (≥ pistol range: a sniper in the turf is engaged)
       ),
       combat: (
           keep_distance: (8.0, 15.0),  // m (GDD §6.3)
           melee_distance: (1.5, 2.5),  // m: start / stop punching
           aim_error_deg: 4.0,          // cone added on top of the weapon spread; police aim better (T11)
           trigger_seconds: (0.5, 1.1), // s between trigger pulls or punches, also the first one after the provocation
           retreat_health: 0.3,         // share of max health (GDD §6.3)
           retreat_distance: 25.0,      // m: a retreating member stops and holds this far away
           tactics: (retreat: 3.0, melee: 2.0, shoot: 1.0, chase: 0.5), // utility weights, highest wins
           warn_gait: Walk,
           chase_gait: Run,
           back_off_gait: Walk,
           retreat_gait: Run,
       ),
   )
   ```
   `GangConfig` + `GroupConfig` + `HostilityConfig` + `GangCombatConfig` + `TacticWeights` + `FactionPair` + `GangSpec`
   in `crates/gta_sim/src/gang/mod.rs`, all `#[serde(deny_unknown_fields)]`, `pub const GANG_CONFIG: &str = "gang/gangs.ron"`.
   `validate()` (every error names its field):
   - `gangs.len() == 2` ("gangs must list exactly 2 gangs: citygen assigns 2 territories"); every `weapons` non-empty.
   - `factions`: `a != b`; every `Gang(i)` has `i < gangs.len()`; no unordered pair twice; **each** of the five pairs
     `{Gang(0),Gang(1)}, {Gang(i),Player}, {Gang(i),Police}` present (error names the missing pair, e.g.
     "factions: missing pair (Gang(0), Police)").
   - `groups.size`: `1 ≤ lo ≤ hi`; `spread`, `post_spacing`, `hq_margin` finite > 0; `post_spacing > 2·spread`.
   - `hostility`: all finite > 0 except `warn_seconds ≥ 0`; `warn_keep_distance ≤ warn_distance < warn_release_distance`.
   - `combat`: `melee_distance.0 < melee_distance.1 < keep_distance.0 < keep_distance.1`; `0 ≤ aim_error_deg < 90`;
     `0 < trigger_seconds.0 ≤ trigger_seconds.1`; `0 < retreat_health < 1`; `retreat_distance > 0`; weights finite ≥ 0.
   Methods: `hostile(a: Faction, b: Faction) -> bool` (`a != b` and the pair is listed `hostile: true`, order-insensitive).
2. **`assets/npc/population.ron`** (edit): add `max_gang_members: 12,  // GDD §6.1: only in gang territories`.
   `PopulationConfig.max_gang_members: u32` (`population/mod.rs:27`); no extra validation (0 disables gangs; a cap below
   `groups.size.0` spawns nothing — stated in the field's doc line).
3. **`assets/combat/weapons.ron`** (edit): `pickups: (radius: 1.0, respawn: 30.0, drop_seconds: 60.0),` — seconds a gun
   dropped by a dead NPC lies before it vanishes. `WeaponPickupConfig.drop_seconds` (`weapons.rs:89-95`), validated finite
   > 0 in `WeaponsConfig::validate` (`:168-190`, message "pickups.drop_seconds is out of range").
4. **`assets/character/visual.ron`** (edit, client):
   ```
   // Gang looks: `Appearance` picks one of these models, the gang picks its tint (index = gang id).
   gang_models: [
       "third_party/mini-characters/character-male-d.glb",
       "third_party/mini-characters/character-male-f.glb",
       "third_party/mini-characters/character-female-c.glb",
   ],
   gang_tints: [(0.8, 0.3, 1.0), (1.0, 0.25, 0.2)],   // gang 0 purple, gang 1 red; police will be blue (T11)
   ```
   Tints chosen away from the civilian tints (`visual.ron:15`: white, light blue, orange, light green). Owner judges.

### Sim — `gang/` domain (`crates/gta_sim/src/gang/`)

5. **`gang/mod.rs`** (new, ~330 lines): config types (Step 1), plugin, components, resources, bundle.
   - `#[derive(Component, Reflect, Deserialize, Clone, Copy, Debug, PartialEq, Eq, Hash)] #[reflect(Component)]
     pub enum Faction { Player, Gang(u8), Police }`.
   - `#[derive(Reflect, Clone, Copy, Debug, PartialEq)] pub enum GangState { Idle, Warn, Attack { target: Entity },
     Retreat { from: Entity }, Dead }`.
   - ```rust
     #[derive(Component, Reflect, Clone, Debug)]
     #[reflect(Component)]
     #[require(Character, Perception, Offscreen)]
     pub struct GangMember {
         pub gang: u8,
         /// Index into `GangTerritories.gangs[gang].posts` (0 = HQ).
         pub post: u16,
         /// Own standing point (feet) at the post.
         pub spot: Vec3,
         pub gun: Weapon,
         pub state: GangState,
         /// Seconds the player has stood in this gang's territory within `warn_distance`.
         pub dwell: f32,
         /// Line of sight to the current focus (player, or the target), refreshed on the member's AI slot.
         pub sees: bool,
         pub last_seen: Vec3,
         /// Line of sight from the chest to `spot`, refreshed on the AI slot while away from it.
         pub home_clear: bool,
         /// Seconds until the next trigger pull or punch.
         pub trigger_left: f32,
     }
     ```
     `Perception` is required only for its `slot` (civilian `perceive`/`collect_stimuli` filter on `Civilian`,
     `perception/mod.rs:489,528`, so they skip members; `pending` stays `None`).
   - `#[derive(Resource, Reflect, Debug)] #[reflect(Resource)] pub struct GangHeat { pub left: Vec<f32> }` (seconds, one
     per gang, initialised to zeros from `GangConfig.gangs.len()`).
   - `#[derive(Resource, Reflect, Default, Debug, PartialEq)] #[reflect(Resource)] pub struct PlayerTerritory(pub Option<u8>)`
     — gang whose territory the player stands in; written every tick; read by the FSM and by QA.
   - `#[derive(Resource)] pub struct GangRng(pub ChaCha8Rng)` = `seed_from_u64(seed)` + `set_stream(2)` (NpcRng is 1),
     with `unit()` / `next_u32()` like `NpcRng` (`population/mod.rs:207-222`).
   - `#[derive(SystemSet, Debug, Clone, PartialEq, Eq, Hash)] pub struct GangSystems;`
   - `pub fn gang_member_bundle(loco: &LocomotionConfig, handle: Handle<CharacterSchemeConfig>, health: &HealthConfig,
     weapons: &WeaponsConfig, gang: u8, post: u16, spot: Vec3, facing_yaw: f32, gun: Weapon, appearance: Appearance)
     -> impl Bundle` = `(GangMember { state: Idle, dwell: 0, sees: false, last_seen: spot, home_clear: true,
     trigger_left: 0, .. }, Faction::Gang(gang), appearance, Name::new("Gang member"),
     Transform::from_translation(spot + Y·float_height).with_rotation(Quat::from_rotation_y(facing_yaw)),
     character_components(loco, handle), Health::full(health), loadout)` where `loadout = Loadout::default()` with
     `acquire(&mut loadout.guns[gun.index()], weapons.stats(gun), true)` and `held: None` (holstered).
   - `GangPlugin { pub seed: u64 }::build`: `insert_resource(GangHeat{ left: vec![0.0; n] })`, `init_resource::<PlayerTerritory>()`,
     `insert_resource(GangRng::seeded(seed))`, registers `Faction, GangState, GangMember, GangHeat, PlayerTerritory,
     GangTerritories, Turf`;
     `configure_sets(FixedUpdate, GangSystems.in_set(NpcSystems).run_if(resource_exists::<GangTerritories>))` (set-level
     gating, the same shape as `NpcSystems.run_if(resource_exists::<SidewalkGraph>)`);
     `OnTransition { exited: Loading, entered: Playing }` → `territory::build_gang_territories.run_if(resource_exists::<City>)`;
     FixedUpdate:
     - `behavior::gang_death.in_set(HealthSystems::Death).in_set(GangSystems)`;
     - `(behavior::decay_gang_heat, behavior::locate_player, behavior::provoke_gangs).chain().in_set(AiSystems::Perceive).in_set(GangSystems)`;
     - `behavior::gang_fsm.in_set(AiSystems::Decide).in_set(GangSystems)`.
     The spawner systems are registered by `PopulationPlugin` (Step 11).
6. **`gang/territory.rs`** (new, ~230 lines incl. tests):
   - ```rust
     #[derive(Reflect, Clone, Debug)] pub struct Turf { pub posts: Vec<Vec3> }   // posts[0] = HQ, y = sidewalk top
     #[derive(Resource, Reflect, Debug)] #[reflect(Resource)]
     pub struct GangTerritories { pub gangs: Vec<Turf>, blocks: Vec<TurfBlock> }
     #[derive(Reflect, Clone, Debug)] struct TurfBlock { curb: Vec<Vec2>, gang: Option<u8> } // layout (x, z), CCW
     ```
   - `pub fn new(gangs: Vec<Turf>, blocks: Vec<(Vec<Vec2>, Option<u8>)>) -> Result<Self, String>`: every block has ≥ 3
     points and positive signed area (CCW; error names the block index), every gang has ≥ 1 post. Used by tests.
   - `pub fn from_layout(layout: &CityLayout, params: &CityParams, groups: &GroupConfig) -> Result<Self, String>`:
     blocks = every block with `curb.len() ≥ 3`, `gang = position of block.district in layout.gang_districts`;
     for g in 0..2: HQ = building with `kind == GangHq(g)` ("no HQ for gang {g}"), anchor =
     `sidewalk_anchor(layout, params, hq, groups.hq_margin)` ("gang {g} HQ {hq}: no sidewalk side of length ≥ {2·margin}"),
     post 0 = anchor at y = `params.roads.curb_height`; then sidewalk nodes in node-id order with `territory_at(node) ==
     Some(g)`, kept when ≥ `post_spacing` from every kept post of that gang.
   - `pub fn territory_at(&self, p: Vec3) -> Option<u8>`: `q = Vec2(p.x, p.z)`; first block with
     `contains_convex(curb, q, 0.0)` returns its gang; otherwise the gang of the block with the smallest
     `dist_point_segment` to its curb edges (ties → lower index). O(blocks) ≈ 3.3 µs (probe).
   - `pub(crate) fn build_gang_territories(commands, city: Res<City>, params: Res<CityParamsRes>, cfg: Res<GangConfig>,
     exit: MessageWriter<AppExit>)`: `Ok` → `insert_resource`; `Err` → `error!("gang territories invalid: {e}")` +
     `AppExit::error()` (same policy as the hospital, `world/city.rs:201-208`).
   - `#[cfg(test)]`: (a) worked example on two synthetic squares — gang 0 `[(−40,−40),(0,−40),(0,40),(−40,40)]`, none
     `[(2,−40),(40,−40),(40,40),(2,40)]` (CCW in (x, z) as (x, y)): (−10,·,5) → Some(0); (30,·,0) → None; (0.5,·,0)
     (0.5 from gang 0, 1.5 from none) → Some(0); (1.5,·,0) → None; a clockwise square → `new` error naming block 0.
     (b) **`every_sweep_seed_has_two_territories`**: shipped `world/city.ron` + `gang/gangs.ron` (`include_str!`, "GATE
     BROKEN" on parse), seeds 0..32 plus 42 through `citygen::generate` + `from_layout`: `Ok`; per gang ≥ 2 posts;
     `territory_at(post) == Some(g)` for every post; posts pairwise ≥ `post_spacing`; posts[0] is within 0.01 m of the
     `sidewalk_anchor` point. Flip: keep nodes of every district → `territory_at` RED; skip the spacing filter → RED.
7. **`gang/fsm.rs`** (new, ~320 lines incl. tests) — pure decisions, no ECS:
   - `pub struct Focus { pub distance: f32, pub from_post: f32, pub visible: bool }` (target seen from the member;
     `from_post` = target's horizontal distance to the member's `spot`).
   - `pub struct Senses { pub heat: f32, pub player: Option<Focus>, pub player_in_turf: bool, pub dwell: f32,
     pub aimed_at: bool, pub target: Option<Focus>, pub target_is_player: bool, pub tactic: Tactic }`
     (`player` = the live player, `None` if absent or `Dead`; `target` = the live Attack/Retreat target).
   - `pub enum Tactic { Retreat, Melee, Shoot, Chase }`;
     `pub fn choose_tactic(health_fraction, distance, visible, has_ammo, punching: bool, in_range: bool, cfg: &GangCombatConfig) -> Tactic`:
     scores `retreat = w.retreat if health_fraction < retreat_health && distance < retreat_distance`,
     `melee = w.melee if distance ≤ melee_distance.0 || (punching && distance ≤ melee_distance.1)`,
     `shoot = w.shoot if visible && has_ammo && in_range`, `chase = w.chase`; else 0; highest wins, ties in the order
     Retreat > Melee > Shoot > Chase. (`punching` = currently unarmed in Attack; `in_range` = distance ≤ gun range.)
   - `pub enum Move { Approach, Hold, BackOff }`; `pub fn band_move(distance, (lo, hi)) -> Move`: `> hi` Approach,
     `< lo` BackOff, else Hold.
   - `pub fn next_state(state: GangState, s: &Senses, h: &HostilityConfig) -> GangState`, rules in order:
     - `Idle`/`Warn`: if `s.heat > 0 && s.player_in_turf` and the player is `visible`, `distance ≤ sight_distance`,
       `from_post ≤ leash_distance` → `Attack { target: player }` (the caller passes the player entity); else `Idle`:
       `player_in_turf && (dwell ≥ warn_seconds || aimed_at)` → `Warn`; `Warn`: no player, `!player_in_turf` or
       `distance > warn_release_distance` → `Idle`; else unchanged.
     - `Attack { t }`: `target == None` (despawned or dead) → `Idle`; `target_is_player && heat == 0` → `Idle`;
       `from_post > leash_distance` → `Idle`; `tactic == Retreat` → `Retreat { from: t }`; else unchanged.
     - `Retreat { f }`: `target == None` → `Idle`; `target_is_player && heat == 0` → `Idle`; else unchanged.
     - `Dead` → `Dead`.
     The signature carries the player entity for the `Attack` result (`next_state(state, player: Option<Entity>, &Senses, h)`).
   - `#[cfg(test)]` worked tables, shipped `gangs.ron` via `include_str!` (numbers below derive from it; the test reads them
     from the parsed config, "GATE BROKEN" if they change):

     **`next_state`** (warn 8 m / 3 s / release 12 m, sight 40, leash 60):
     | # | state | senses | expected |
     |---|---|---|---|
     | 1 | Idle | in turf, d 5, dwell 2.984375 (191/64), heat 0 | Idle |
     | 2 | Idle | in turf, d 5, dwell 3.0 | Warn |
     | 3 | Idle | out of turf, dwell 0, aimed | Idle |
     | 4 | Idle | in turf, d 10, dwell 0, aimed | Warn |
     | 5 | Idle | heat 50, in turf, visible, d 30, from_post 30 | Attack{player} |
     | 6 | Idle | heat 50, in turf, visible, d 45 | Idle |
     | 7 | Idle | heat 50, out of turf, visible, d 10 | Idle |
     | 8 | Warn | in turf, d 12.5 | Idle |
     | 9 | Warn | in turf, d 11 | Warn |
     | 10 | Warn | out of turf, d 5 | Idle |
     | 11 | Attack{player} | heat 0, visible, d 12 | Idle |
     | 12 | Attack{player} | heat 10, from_post 61 | Idle |
     | 13 | Attack{player} | heat 10, tactic Retreat | Retreat{player} |
     | 14 | Attack{player} | target None | Idle |
     | 15 | Retreat{player} | heat 10 | Retreat{player} |
     | 16 | Retreat{player} | heat 0 | Idle |
     | 17 | Attack{rival} | heat 0 (not player), from_post 20 | Attack{rival} |
     **`choose_tactic`** (weights 3/2/1/0.5, retreat 0.3 & 25 m, melee (1.5, 2.5), pistol range 60):
     | # | hp | d | visible | ammo | punching | in range | expected |
     |---|---|---|---|---|---|---|---|
     | 1 | 1.0 | 12 | yes | yes | no | yes | Shoot |
     | 2 | 1.0 | 12 | no | yes | no | yes | Chase |
     | 3 | 1.0 | 1.2 | yes | yes | no | yes | Melee |
     | 4 | 1.0 | 2.0 | yes | yes | yes | yes | Melee |
     | 5 | 1.0 | 2.0 | yes | yes | no | yes | Shoot |
     | 6 | 1.0 | 3.0 | yes | yes | yes | yes | Shoot |
     | 7 | 0.25 | 12 | yes | yes | no | yes | Retreat |
     | 8 | 0.25 | 30 | yes | yes | no | yes | Shoot |
     | 9 | 0.31 | 12 | yes | yes | no | yes | Shoot |
     | 10 | 1.0 | 12 | yes | no | no | yes | Chase |
     | 11 | 0.1 | 1.0 | yes | yes | no | yes | Retreat |
     | 12 | 1.0 | 70 | yes | yes | no | no | Chase |
     No row sits on a threshold (0.31 vs 0.3; 191/64 vs 3.0 is the exact-tick neighbour, exact in binary).
     **`band_move`**: 20 → Approach; 15 → Hold; 8 → Hold; 5 → BackOff; 15.5 → Approach.
     **Faction matrix**: shipped `hostile(Gang(0), Player)` and `(Player, Gang(1))` true; `(Gang(0), Gang(1))`,
     `(Gang(1), Gang(0))`, `(Gang(0), Police)` false; `(Gang(0), Gang(0))` false even if listed (validate rejects it anyway).
8. **`gang/behavior.rs`** (new, ~340 lines) — ECS systems (all `GangSystems`; `dt = Time<Fixed>::timestep()`):
   - `decay_gang_heat`: `left[g] = (left[g] − dt).max(0.0)`.
   - `locate_player`: `PlayerTerritory(territories.territory_at(player.position))`, `None` without a live player.
   - `provoke_gangs`: readers `MessageReader<ShotFired>`, `MessageReader<DamageDealt>`; `factions: Query<&Faction>`;
     `members: Query<(Entity, &Position, &mut GangMember)>`; `cfg: Res<GangConfig>`, `territories`, `ResMut<GangHeat>`,
     `ResMut<GangRng>`. Builds `provocations: Vec<(gang, at: Vec3, attacker: Entity)>`:
     - per `DamageDealt` whose `target` is a `GangMember` (alive or just killed — the victim's position is still valid) and
       `cfg.hostile(member faction, attacker faction)` (no `Faction` on the attacker → ignored) → `(g, victim pos, shooter)`;
     - per `ShotFired` with a hostile shooter faction: every alive member within `shot_radius` (3D distance muzzle→chest)
       whose gang == `territory_at(muzzle)` → `(g, member pos, shooter)`.
     Then per provocation: if the attacker's faction is `Player` → `heat.left[g] = heat_seconds`; every member of gang `g`
     with state `Idle`/`Warn` and horizontal distance to `at` ≤ `group_radius` → `Attack { target: attacker }`,
     `last_seen = attacker's Position` (or `at`), `sees = false`, `trigger_left = roll(trigger_seconds)` (GangRng).
     Members already in `Attack`/`Retreat` keep their target; `Dead` untouched. No transitive chain beyond one hop.
   - `gang_fsm` (Decide): params `cfg`, `GangConfig`, `PerceptionConfig` (slots, `aimed_distance`, `aimed_cone_deg` — the
     same NPC perception numbers civilians use), `LocomotionConfig`, `NavigationConfig.arrive_radius`, `WeaponsConfig`
     (gun range), `Res<AiClock>`, `Res<GangHeat>`, `Res<PlayerTerritory>`, `ResMut<GangRng>`, `SpatialQuery`;
     `members: Query<(Entity, &mut GangMember, &Perception, &Position, &Health, &Loadout, &mut MoveIntent, &mut AimIntent,
     &mut ActionIntent)>`; `player: Query<(Entity, &Position, &AimIntent, &Loadout, Has<Dead>), (With<Player>, Without<GangMember>)>`
     (disjoint from `members` by `Without<GangMember>`: B0001-safe); `bodies: Query<(&Position, Has<Dead>)>` (read only)
     for targets. Per member with state ≠ `Dead`:
     1. `dwell += dt` if the player is alive, `PlayerTerritory == Some(gang)` and horizontal distance ≤ `warn_distance`;
        else `dwell = 0`.
     2. On the member's slot (`clock.tick % slots == perception.slot`): focus = target (Attack/Retreat) or player (else);
        `sees = !sight_blocked(eyes, focus chest)` if focus within `sight_distance`, else false; if `sees`,
        `last_seen = focus position`; `aimed_at` = player alive, aiming, `held.is_some()`, distance ≤ `aimed_distance`,
        `angle(aim.direction, member chest − aim.origin) ≤ aimed_cone_deg`, and `sees`; `home_clear` =
        `!sight_blocked(chest, spot + Y·float_height)` when farther than `arrive_radius` from `spot`. Off-slot:
        `aimed_at = false`, others keep last values. Eyes = feet + `head_height`; chest = `Position`.
     3. `tactic = choose_tactic(health.current / max_health, d, sees, magazine + reserve > 0, held.is_none(),
        d ≤ stats(gun).range, combat)` (only meaningful in Attack).
     4. `state = next_state(...)`; on entering `Attack` here (heat on sight) roll `trigger_left`.
     5. Intents by state (`fire_requested` is only ever **set**, never cleared, so a test or the weapon systems own the latch):
        - `Idle`: `aim.aiming = false`; want `held = None`; walk (`idle` uses `warn_gait`) to `spot` if farther than
          `arrive_radius` and `home_clear`, else `axis = ZERO`.
        - `Warn`: want `held = Some(gun)`; `aim = { origin: eyes, direction: player chest − eyes, aiming: true }`;
          walk toward the player (`warn_gait`) while distance > `warn_keep_distance`, else stand.
        - `Attack` by tactic: **Shoot** — want `held = Some(gun)`; aim at the target chest; `band_move` → Approach
          (`chase_gait`, toward) / BackOff (`back_off_gait`, away, still aiming) / Hold; `trigger_left −= dt`, when ≤ 0 and
          `sees` and in range: `fire_requested = true`, `aim.direction = cone_sample(dir, aim_error_deg, u, v)` for this
          tick only, re-roll `trigger_left`. **Melee** — want `held = None`; flat aim toward the target (`aiming = true`);
          approach while `d > melee_distance.0`, else stand; same trigger timer → `fire_requested = true` (a punch).
          **Chase** — want `held = Some(gun)` if it has ammo else `None`; aim toward `last_seen`; run (`chase_gait`) to
          `last_seen`, stand within `arrive_radius`.
        - `Retreat`: want `held = Some(gun)`; if `d < retreat_distance`: `aiming = false`, run away (`retreat_gait`);
          else stand and act like Shoot's trigger (aiming, pulls when `sees`).
        - Weapon choice goes through `action.select`: `Some(WeaponRequest::Gun(gun))` / `Some(WeaponRequest::Unarmed)` only
          when `loadout.held` differs from the wanted one (never `Unarmed` while unarmed: no bat toggle).
   - `gang_death` (`HealthSystems::Death`): `Query<(Entity, &Health, &Position, &CharacterBody, &mut GangMember,
     &mut MoveIntent, &mut AimIntent, &mut ActionIntent, &mut Loadout)>`; state ≠ `Dead` and `current ≤ 0` → `state = Dead`,
     `axis = ZERO`, `aiming = false`, `fire_requested = false`, the gun leaves the body (`held = None`,
     `guns[gun] = GunSlot::default()`, `reload_left = 0`), `insert(corpse_components())`, and
     `commands.spawn(dropped_gun(gun, feet, &weapons))` (Step 10). A fatal hit and its `DamageDealt` in one tick: death wins,
     `provoke_gangs` (later in the tick) still aggroes the group from the victim's position.
9. **`crates/gta_sim/src/lib.rs`**: `pub mod gang;`; load + validate `GangConfig` like the others; `insert_resource(gangs)`;
   `GangPlugin { seed: combat_seed }` appended after `CivilianPlugin` (tuple 12 → 13).
10. **`combat/pickups.rs`**: `#[derive(Component, Reflect, Debug)] #[reflect(Component)] pub struct Dropped { pub left: f32 }`;
    `pub fn dropped_gun(weapon: Weapon, feet: Vec3, cfg: &WeaponsConfig) -> impl Bundle` = `(WeaponPickup { weapon,
    ammo_only: false, cooldown: 0.0 }, Dropped { left: cfg.pickups.drop_seconds }, Name::new(format!("Dropped {weapon:?}")),
    Transform::from_translation(feet))`; `collect_weapon_pickups` (`:101-130`) gets `Entity`, `Has<Dropped>` and
    `Commands`: on a successful take a dropped pickup is `try_despawn`ed instead of `cooldown = respawn`;
    new `expire_dropped` (`left −= dt`, `try_despawn` at `left ≤ 0`) in `combat/mod.rs` next to the other pickup systems
    (`HealthSystems::Pickup`, `PlayingSystems`); register `Dropped`; export `Dropped`, `dropped_gun`. (Lifetime is combat's
    number: combat owns pickups; T11 police reuse `dropped_gun`.)
    **`player/mod.rs:561-568`**: add `Faction::Player` to the spawn bundle (the player is a faction of the matrix).

### Sim — gang spawner (`population/`)

11. **`population/gangs.rs`** (new, ~220 lines) + **`population/mod.rs`** edits: `mod gangs;` and, in
    `PopulationPlugin::build`, `(gangs::despawn_far_gangs, gangs::spawn_gangs).chain().after(spawn_civilians)
    .in_set(PopulationSystems).in_set(GangSystems)`. `spawn_civilians` and `despawn_far` are not changed.
    - `despawn_far_gangs`: no view → return; per `GangMember` (alive or corpse): `Offscreen` as in `despawn_far`
      (`outside_cone(view, feet, head_height, 0)`), `try_despawn` when `Offscreen ≥ despawn_offscreen_seconds` **and**
      horizontal distance to the player `> despawn_distance`. No recycling (gangs are not a street budget).
    - `spawn_gangs`: no view or no player → return. `alive` = members with state ≠ `Dead`; `alive ≥ max_gang_members` →
      return. Occupied posts = every `(gang, post)` of any `GangMember` (alive or corpse). Bodies = `Position` of every
      `Character`. Candidates = free posts with horizontal distance to the player in `spawn_ring`, sorted HQ (post 0) first,
      then by distance ascending (stable, then gang id). For each candidate: roll `size` in `groups.size` (GangRng);
      skip if `alive + size > max_gang_members`; spots per the formula in §2; skip if any spot is closer than
      `spawn_min_separation` to any body; each spot must satisfy `outside_cone(view, spot, head_height, margin)` or — if
      `load.rays + OCCLUSION_RAYS_PER_POINT ≤ occlusion_rays_per_tick` — `occluded(spatial, view, spot,
      occlusion_ray_height, capsule_radius, &mut load.rays)`; one visible or unaffordable spot skips the whole post this
      tick. Then spawn the group: gun = uniform from `gangs[g].weapons`, `Appearance(rng.next_u32())`, facing the post.
      `load.rays` is **not** reset here: the tick total stays within the one budget (`spawn_civilians` reset it first).
12. **Scheduling result in one fixed tick** (for the reviewer): Damage (player/NPC shots, strikes) → Regen → Pickup
    (`expire_dropped`) → Death (`civilian_death`, `gang_death`, `dummy_life`, `detect_player_death`) → Perceive
    (civilians' chain ‖ `decay_gang_heat → locate_player → provoke_gangs`) → Decide (`civilian_fsm` ‖ `gang_fsm`, both
    before `TnuaUserControlsSystems`) → PopulationSystems (`age_corpses → despawn_far → spawn_civilians →
    despawn_far_gangs → spawn_gangs`). `civilian_fsm` and `gang_fsm` both write `MoveIntent` on disjoint entities through
    two systems; the executor serialises them (no error). NPC intents written in Decide are consumed in the next tick's
    Damage set (one tick, 15.6 ms latency — same as civilians).

### Sim — gates (`cargo test -p gta_sim`)

All integration tests use the production composition (`headless_app` / `city_app`), the production `gang_member_bundle`,
configs read from resources, `run_ticks` counted by `Time<Fixed>`. Allowed, named test-side mutations: insert a synthetic
`GangTerritories`, insert a test `SidewalkGraph` (the floor has none and `NpcSystems` needs one), set `CameraView`,
`max_gang_members`, the faction matrix (positive controls), a member's `Health`, despawn a member, place the player.
Test-floor geometry avoids the fixture blocks (`world/test_area.rs:6-19`): lanes along x = 0 for z ∈ [−39, 0] are clear;
the wall at (0, 2, 14), 12 × 4 × 0.5 m, is used as an LOS blocker. Each test derives its numbers from the resources and
panics "GATE BROKEN" when a fixture precondition fails. Record each flip-RED (perturbed input, RED seen, GREEN restored)
in IMPL_SUMMARY.

13. **`tests/common/mod.rs`** helpers: `gang_floor(turf: TurfLayout) -> App` (headless app + a 2-node test graph at
    (30,0,30)-(35,0,30) + synthetic `GangTerritories` + settle + pistol as in `civilians.rs::armed_app`);
    `TurfLayout::{WestHalf, WholeFloor}` (WestHalf: gang 0 = x ≤ 0 square, none = x ≥ 2 square; WholeFloor: gang 0 =
    the 80 × 80 floor); `spawn_member(app, gang, spot, gun) -> Entity` via `gang_member_bundle` (post 0, facing −Z);
    `gang_state(app, e)`, `heat(app, g)`, `set_armor(app, 100)` for the player.
14. **`tests/gangs.rs`** (new, acceptance for hostility):
    - **`outside_the_territory_members_stay_neutral`** (acceptance "вне территории нейтральны"). `WestHalf`. Members A
      (−4,0,−10), B (−2,0,−12), gang 0. (a) Player at (3,0,−10) (outside; 7 m from A < `warn_distance`), aiming
      (`aiming = true`, pistol) at A's chest: run `warn_seconds·64 + slots + 8` = 204 ticks → A, B `Idle`,
      `PlayerTerritory == None`, heat 0. Then a shot into the air (muzzle at x > 0, within `shot_radius` of both): 4 more
      ticks → still `Idle`, heat 0. (b) Positive control: player to (−6,0,−4) (inside; 6.3 m to A, 8.9 m to B), not
      aiming, aim straight up: A is `Idle` after 191 ticks and `Warn` at tick 192 (`warn_seconds·64`, exact: dwell
      increments are k/64); B stays `Idle` (8.9 > 8). Then a shot into the air → in the shot tick A and B are
      `Attack { target: player }`, `heat[0] == heat_seconds`. Flip-RED: drop the turf check from `dwell` → (a) RED; drop
      `territory_at(muzzle)` from the shot rule → (a) RED; `dwell > warn_seconds` → Warn at 193 → (b) RED.
    - **`aiming_at_a_member_warns_within_one_cycle`**: inside, A at 10 m (> `warn_distance`), player aims at A's chest
      with `aiming = true` → `Warn` within `slots` ticks; control: same with `aiming = false` → `Idle` after 64 ticks.
      Flip: ignore `aimed_at` → RED.
    - **`attack_on_a_member_aggroes_the_group_within_30m`** (acceptance). `WholeFloor`, player at origin (pistol, armour
      100). Gang 0: V (0,0,−10), M1 (4,0,−12), M2 (0,0,−39) (29 m from V), C (0,0,21) (31 m from V, 33.2 from M1, behind
      the fixture wall from the player); gang 1: R (−4,0,−8) (4.5 m from V). Preconditions from config: M2−V < `group_radius`
      < C−V, C−M1; C farther than `shot_radius` from the muzzle; LOS player↔C blocked (test ray); nobody within
      `warn_distance` of the player. Aim at V's chest, `fire_requested`, run tick by tick with `Shots` until the
      `DamageDealt` on V (tick T, bounded by 4 ticks). In tick T: V, M1, M2 `Attack { target: player }`; C not `Attack`;
      R `Idle`; `heat[0] == 120.0`, `heat[1] == 0.0`. C still not `Attack` after `slots` more ticks (cannot see).
      Flip-RED: aggro only the victim → M1/M2 RED; group radius ignored → C RED; gang filter removed → R RED.
    - **`hit_outside_the_territory_still_provokes`** (pins Open question 1's default): `WestHalf`, player at (3,0,−10)
      shoots A → A `Attack` in the hit tick, `heat[0] == 120`. If the owner picks option B, this test inverts.
    - **`gang_heat_decays_in_120s`** (acceptance). `WholeFloor`, single member V (0,0,−10); the player shoots V; tick T
      has `heat[0] == 120.0`; despawn V (named mutation, ends the fight). Assert exact values: `T+1` → 120 − 1/64,
      `T+3840` → 60.0, `T+7679` → 1/64, `T+7680` → 0.0, `T+7681` → 0.0 (all exact in f32: multiples of 1/64 below 128).
      Memory: at `T+7000` spawn F at (0,0,−20) (in turf, visible, 20 m > `warn_distance`, < `sight_distance`) → `Attack`
      within `slots` ticks, then despawn F; at `T+7681` spawn G at the same spot → not `Attack` over 64 ticks.
      Flip-RED: decay `2·dt` → the `T+3840` value RED; drop the `heat > 0` test from attack-on-sight → G RED.
    - **`disabled_matrix_gangs_do_not_attack_each_other`** (acceptance). `WholeFloor`, player at (20,0,20) (far, no
      warn). A (gang 0) at (0,0,−10), B (gang 1) at (0,0,−11.2). The test sets A's `AimIntent.direction` toward B and
      `fire_requested` (A is `Idle`, holstered, so it is a real punch through `swing_melee` → `apply_strikes`); wait for the
      `DamageDealt { shooter: A, target: B }` (precondition, ≤ 30 ticks: the fist window opens at tick 8). Then over 64
      ticks: B and A never `Attack`, `heat == [0, 0]`. Positive control: set `(Gang(0), Gang(1)).hostile = true` in the
      `GangConfig` resource, punch again → B `Attack { target: A }` in the hit tick. Flip-RED: `provoke_gangs` ignores the
      matrix → first half RED. Also a shot variant: A fires a pistol into the air next to B (A given a pistol via
      `select`) → B stays `Idle` with the matrix off.
15. **`tests/gang_combat.rs`** (new, fight and drops; player given 100 armour so it survives):
    - **`attackers_open_fire_at_the_player`**: member at (0,0,−12), provoked by a shot into the air. Within
      `ceil(trigger_seconds.1·64) + slots + 2` ticks the member holds its gun and has ≥ 1 `ShotFired`; over 256 ticks
      ≥ 2 shots; every `BulletTrace` of the member passes within `d·tan(aim_error + base + max_bloom + 0.3°) + 0.5 m` of the
      player's chest (d = 12; ≈ 2.2 m; the member stands, `|v| < 0.5 m/s` checked). Flip: aim at chest + (5,0,0) → RED.
    - **`members_hold_the_8_to_15m_band`**: provoked member at 4 m (outside melee 2.5) backs off: distance +≥ 1.5 m after
      128 ticks; a member at 25 m approaches: distance ≤ 20 after 128 ticks and in [13.5, 15.5] after 512 ticks.
      Flip: `band_move` always Hold → RED.
    - **`wounded_member_retreats`**: member at (0,0,−10) with `Health.current = 25` (0.25 < 0.3), provoked → `Retreat` in
      the provocation tick; after 384 ticks distance ∈ [24.5, 26.5] and still `Retreat`. Flip: `tactics.retreat = 0`
      → stays `Attack` → RED.
    - **`close_member_punches`**: provoked member 1.2 m from the player → `held == None` and `Melee.swing` within
      `ceil(trigger_seconds.1·64) + 4` ticks; a `DamageDealt` from the member on the player with the fist damage within
      128 ticks. Flip: `melee` weight 0 → RED.
    - **`dead_member_drops_its_gun`**: member with a pistol, `Health.current = 0` → next tick: `state == Dead`, `Dead`,
      `Corpse`, `TnuaToggle::Disabled`; its `Loadout` has `held None` and the pistol not owned; exactly one new
      `WeaponPickup { Pistol, ammo_only: false }` + `Dropped`, horizontally ≤ 0.01 m from the body, y = feet. Player placed
      on it → next tick the player owns a pistol and the pickup entity is gone. Second drop left alone: present at
      `drop_seconds·64 − 1` = 3839 ticks after it appeared, gone at 3840. Flip: ignore `Dropped` in collect → still
      present with cooldown → RED; `left < 0` → one tick late → RED.
16. **`tests/gang_city.rs`** (new, seed 1):
    - **`territories_come_from_the_generator`**: `GangTerritories` exists after load, 2 gangs; for each gang posts[0]
      equals `sidewalk_anchor(GangHq(g), hq_margin)` (recomputed from `City`); every post `territory_at == Some(g)`; the
      seed-1 player spawn → `None` (district 4); a point inside a block of the other gang's district → that gang.
    - **`groups_spawn_hidden_in_the_ring`**: player placed on the sidewalk node of gang 0's turf whose distance to the HQ
      post is in [50, 70] m (GATE BROKEN if none); `chase_view` turning 360° every 8 s for 640 ticks. Every `Added<GangMember>`
      tick: distance to the player in `spawn_ring`; spot within `spread + 0.05` of a post of its own gang; outside
      cone + margin **or** the 4 independent camera rays (head centre, both sides, feet — copy the `street_spawn.rs:49-70`
      helper) blocked; group size per (gang, post) in `groups.size`; each tick alive members ≤ `max_gang_members` and
      `PopulationLoad.rays ≤ occlusion_rays_per_tick`. Liveness: ≥ 1 group and the HQ post has a group by the end.
      Preconditions: a post closer than `spawn_ring.0` exists (so a ring flip is visible). Flip-RED: skip the visibility
      check → "spawned in clear view"; drop the ring filter → a spawn inside 30 m; ignore the cap (with
      `max_gang_members = 4` set in a second run) → cap RED.
    - **`far_gang_member_despawns_after_2s_offscreen`**: member spawned via bundle at a post; player 160 m away; view up
      (+Y): present at 127 ticks, gone at 128; a second member 140 m away stays. Flip: `||` for `&&` → RED.
17. **`tests/config.rs`**: `shipped_gang_config_loads` (+ `population.ron` still validates with the new field,
    `weapons.ron` with `drop_seconds`); `unknown_gang_field_names_file_and_field`; `sabotaged` fixtures, each strictly on
    the failing side and asserting its own keyword: gang list of three (`gangs`), the `(Gang(0), Police)` line removed
    (`Police`), `keep_distance: (15.0, 8.0)` (`keep_distance`), `retreat_health: 1.5` (`retreat_health`),
    `drop_seconds: 0.0` (`drop_seconds`).

### Client (`gta_like`)

18. **`src/visuals/character_config.rs`**: fields `gang_models: Vec<String>`, `gang_tints: Vec<(f32, f32, f32)>`;
    `validate`: both non-empty, tint components finite ≥ 0 (same loop as civilians, `:139-154`); `resolve` checks every
    gang model like a civilian model (`:171-188`, message "gang model {m} resolves clips differently from {model}").
19. **`src/main.rs` `preflight`**: every gang model `manifest.contains_asset` (same message format as civilians,
    `:113-119`), and `gang_tints.len() == GangConfig.gangs.len()` (pass the gang count in; error
    "character/visual.ron: gang_tints must have one tint per gang (2)").
20. **`src/visuals/character.rs`** (target ≤ 650 lines):
    - `CharacterAnimations::from_world` chains `config.model`, `civilian_models`, then `gang_models` (`:155-157`); key
      doc updated: 0 = player, 1..=C civilians, C+1.. gangs.
    - Replace `model_key` / `body_tint` with one `body_look(appearance, civilian: bool, gang: Option<u8>, config) ->
      (usize, (f32, f32, f32))`: civilian → as today; gang → key `1 + C + a % G`, tint `gang_tints[gang]`; else (0,
      `tint`). `spawn_character_model` and `on_model_ready` query `Option<&GangMember>` alongside `Civilian`/`Appearance`
      (the member bundle carries both, so they exist when `On<Add, CharacterBody>` fires).
21. **`src/visuals/weapons.rs`**: `attach_held_gun` iterates `Query<Entity, With<Loadout>>` instead of `With<Player>`
    (`:102-104`), `show_held_gun` reads `Query<&Loadout>` (`:147-150`); doc lines say "each armed character". A dead
    member's `Loadout` is cleared by the sim, so its gun hides.

### Client — gates (`cargo test -p gta_like --bin gta_like`; run each touched presentation gate 3 times)

22. **`src/visuals/civilian_gate.rs`**: `models()` also chains `gang_models` (so `graph_clips_come_from_their_own_model`
    covers gang graphs); `require_glbs` covers them. New **`every_gang_model_animates_from_its_own_clips`** in the same
    GLB harness: one member per gang model key (`Appearance` chosen so `a % G` hits each key, gangs alternating), no
    `GangTerritories` so the gang AI is off and the test drives each member's `MoveIntent` (walk forward). Assert per member
    key and graph handle (`graphs[1 + C + k]`), leg-left turn range > 0.1 rad over 32 updates (range, not two snapshots),
    and the tinted mesh's `base_color` equals the untinted player material × `gang_tints[gang]` (linear, 1e-4).
    Flip: wire every gang animator with `graphs[0]` → leg turn 0.0 → RED. **`gang_models_are_validated`**: a non-rig gang
    model is rejected by `resolve`; empty `gang_models` / `gang_tints` rejected by `validate`, each naming its field.

### Runtime QA and owner run

23. **`tools/qa/scenarios/t9.py`** (new, stdlib, imports `brp`, `t5`, `t6`, `t8` helpers):
    1. `Game(features=("dev",), args=("--seed", "1"), release=True)`; wait `Playing`, `CityLayoutHash` == golden,
       `wait_chunks`.
    2. `territories = resource_value(game, "GangTerritories")`; gang 0 posts; HQ = posts[0].
    3. Pistol: t6 pickup flow until `held == Pistol`.
    4. Approach unseen: teleport to the gang-0 post whose distance to the HQ is in [50, 80] m; point the camera away from
       the HQ: `mutate_component(camera, OrbitCamera, "yaw", atan2(−d.x, −d.z))` with `d = post − hq` (§2 examples).
       Poll `GangMember` rows until a group with `post == 0`, `gang == 0` exists (deadline 15 s). Record spawn time.
    5. Teleport 12 m from the HQ group's centroid along the line from the centroid to the approach post (on the sidewalk
       side, still in turf: check `PlayerTerritory == 0`, else move 2 m closer, up to 3 tries); yaw toward the group; wait
       0.5 s; read states → `before` (expected all `Idle`/`Warn`). Screenshot `hq.png`.
    6. Aim into the sky (`move_mouse` as t8) and one LMB click; confirm the magazine dropped; wait 0.3 s; read every gang
       row within `group_radius` (from `gangs.ron`) of the group centroid → `after`; read `GangHeat`. Screenshot
       `aggro.png` (≥ 0.15 s after the previous capture).
    7. Firefight evidence: wait 3 s, read player `Health`, count gang `Loadout.held` drawn, screenshot `firefight.png`.
    8. Hard pass: the HQ group spawned; `before` has no `Attack`; every member in `after` is `Attack`; `GangHeat.left[0] > 110`;
       `PlayerTerritory` was 0 at the shot; zero `ERROR` lines. Soft evidence: player health drop, `get_diagnostics`.
    Also re-run `t8.py` once as a regression (its pistol pickup filter must skip `Dropped` pickups if any exist).
24. **Owner checklist** (QA writes it into QA_REPORT.md): "зашёл к бандитам, спровоцировал, получил перестрелку";
    both gangs readable by tint and distinct from civilians and (later) police; a warning reads as a threat (gun drawn,
    aimed, walking up); groups never pop into view; the firefight: they keep distance, miss more than the player, punch
    when rushed, wounded ones back off; a killed gangster drops its gun and the gun can be picked up; FPS near a fight.

### Order and checks

25. Steps 1-4, 17 → `cargo test -p gta_sim --test config`. Steps 5-10 → `cargo build`, `cargo clippy -- -D warnings`,
    `cargo clippy -p gta_sim --tests -- -D warnings`, `cargo test -p gta_sim --lib` (territory + fsm tables). Steps 11,
    13-16 → full `cargo test -p gta_sim` (every existing test green; report `street_ahead_stays_populated`'s mean
    and window spread next to its TASK-022 value 7.63 / 3.26..8.60 — a change is a finding, not an expected effect).
    `cargo test -p citygen` once (untouched). Steps 18-22 → `cargo test -p gta_like --bin gta_like` ×3.
    `python tools/qa/tree_check.py`, `cargo tree -p gta_sim -e normal -i bevy_render` (empty). Step 23 last, release build.

---

## 4. Test plan (claim → gate)

| Claim | Gate | Class |
|---|---|---|
| Neutral outside the territory (acceptance) | `outside_the_territory_members_stay_neutral` | correctness (+ positive control) |
| Attack on a member → whole group within 30 m in `Attack` (acceptance) | `attack_on_a_member_aggroes_the_group_within_30m` | correctness, same-tick |
| `GangHeat` decays in 120 s (acceptance) | `gang_heat_decays_in_120s` | correctness, exact ticks + memory effect |
| Disabled matrix: no gang-on-gang attacks (acceptance) | `disabled_matrix_gangs_do_not_attack_each_other` | correctness (+ positive control) |
| Warning: 3 s within 8 m, or aimed at | exact tick 192 in the neutrality test, `aiming_at_a_member_warns_within_one_cycle` | correctness |
| Territories from the generator | `every_sweep_seed_has_two_territories`, `territories_come_from_the_generator` | correctness over 33 seeds |
| FSM and scorer rules | `fsm.rs` tables | correctness |
| Fight: distance, spread, melee, retreat | `gang_combat.rs` four fight tests | liveness + correctness |
| Weapon drop, one-shot, lifetime | `dead_member_drops_its_gun` | correctness |
| Groups never spawn in view, cap, ring, ray budget | `groups_spawn_hidden_in_the_ring`, `far_gang_member_despawns_after_2s_offscreen` | correctness + bound |
| Gang models animate from their own clips, gang tint applied | `every_gang_model_animates_from_its_own_clips` | correctness (real GLB) |
| Config strictness | `tests/config.rs` fixtures | correctness |
| Runtime: HQ, shot, `Attack`, screenshot (acceptance) | `t9.py` | runtime |
| Tints, warning read, firefight feel | owner checklist | owner |

Declined on purpose: a gang perf bench (≤ 12 agents, O(members) per tick, LOS on slots, rays under the shared budget —
asserted in `groups_spawn_hidden_in_the_ring`); a held-gun-on-NPC gate (needs a real hand joint; owner sees it on the
first frame).

---

## 5. Risk areas

1. **Existing seed-1 street gates.** `street_ahead_stays_populated` runs 270 m along the longest clear street from the
   spawn (7.2, −40.2); the direction is chosen at run time and not recorded here. Gang 0's district 3 is the north-centre
   cell (z ≈ −600..−200), so a northward run enters it and gang groups spawn during the gate. Mitigated by construction: own RNG stream,
   civilian spawner untouched, gang rays only from the leftover budget. Remaining coupling: physical members at corners
   and `Perception` slot interleaving (no stimuli in that test). Implementer reports the number; if it drops below its
   bound the finding goes to the orchestrator, the gate is not re-anchored silently.
2. **Existing QA scenarios** (`t6`/`t7`/`t8`) may now meet gang groups if their park or route is inside a territory; a
   pistol shot there provokes them. t8 is re-run in Step 23.
3. **Direct seek without A\***: members chasing around a building corner can rub along walls. `home_clear` prevents the
   worst case (walking home into a wall); the rest is owner-visible. Navmesh/A\* decision stays with T11 (GDD §6.6).
4. **NPC trigger cadence** makes the SMG fire single rounds (one pull = one round via `fire_requested`); owner may want
   bursts — data change later (a `burst` knob), not in this slice.
5. **Friendly fire**: a member's bullet can hit and kill a fellow member standing in the line of fire; it never provokes
   (same faction). Owner-visible, acceptable.
6. **Wasted**: gangs keep deciding (NpcSystems) while `fire_weapons` is off; the dead player is no target → members go
   `Idle`; `GangHeat` keeps decaying (GDD is silent on resetting it at death — kept, see Open question 3).
7. **`territory_at` on roads** uses the nearest block: on a street between two districts the player flips territory at the
   street's midline. Minimap (T12) paints blocks, so what the owner sees matches block ownership.
8. **One-tick-late intents**: the first NPC shot comes one tick after the pull decision; gates count ticks from messages,
   not from decisions.

## 6. Open questions (для владельца; план идёт по рекомендованным вариантам, решения можно поменять данными/одним тестом)

1. **Ответ на удар за пределами территории.** GDD §6.3: "вне территории нейтральны". Что делает бандит, если игрок,
   стоя вне территории, попал в него?
   - A (рекомендую): попадание по бандиту провоцирует всегда (самооборона). Вне территории не работают предупреждение,
     "выстрел рядом" и атака по памяти `GangHeat`. Нейтральность = "сами не начинают".
   - B: буквально — вне территории не провоцирует ничто, даже попадание. Бандита можно расстрелять с соседней улицы
     без ответа.
   - C: провоцирует всё, территория влияет только на предупреждение.
2. **Как читается предупреждение.** A (рекомендую): достают ствол, целятся в игрока, подходят до 3 м — без нового
   контента. B: только поворачиваются и подходят. C: A плюс жест `emote-no` из клипов Kenney (ещё одно действие аниматора).
3. **`GangHeat` после смерти игрока.** A (рекомендую): продолжает затухать, как указано в GDD (сброс при смерти касается
   только розыска). B: обнуляется на "ПОТРАЧЕНО".
4. **Оружие банд и цвета.** По умолчанию банда 0: пистолет/SMG, фиолетовый тинт; банда 1: пистолет/дробовик, красный.
   Меняется в `gangs.ron` и `visual.ron`.

children: 0 launched / 0 reported.
