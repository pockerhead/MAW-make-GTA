# PLAN — TASK-013 (GDD T12): minimap, menus, settings

Cost of error: **mixed**.
- Silent class (full evidence layer): "Новый город" leaving the old city behind (duplicate colliders, a second
  player, stale NPCs, stale `SidewalkGraph`/`GangTerritories`, stale wanted/arrest state holding despawned entities,
  a paused `Time<Virtual>` that freezes the new city), a minimap raster that silently changes or is mirrored against
  the shader convention, a projection sign error, RNG streams that do not follow the chosen seed.
- Owner class (mechanism + one honest gate + owner run): how the minimap reads (colours, size, radius, rotation feel),
  menu look and flow, settings UI, persistence between runs (owner criterion of the AC), sensitivity/volume feel.

Pinned (checked in `Cargo.lock` + `~/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/`): bevy / bevy_ui /
bevy_ui_render / bevy_ui_widgets / bevy_text / bevy_input_focus / bevy_state / bevy_time / bevy-settings 0.19.1,
bevy_enhanced_input 0.26.0, bevy_brp_extras 0.22.6, rand_chacha 0.10.0, glam 0.32.1. **No new crate, no
`Cargo.toml` change**: the client already has `bevy = { default-features = true, features = ["bevy_settings"] }`
(`Cargo.toml:38`), `bevy-settings 0.19.1` and `toml 1.1.6` are already in `Cargo.lock` (lines 510, 6512).

Builds: `CARGO_TARGET_DIR=D:/test-gta-like/target`, `-j 4`, one cargo command at a time, in place (host memory
pressure). The planner probe (below) built `gta_sim`/`citygen` from this tree under a different profile into the
shared target: if an untouched module suddenly "does not exist", `touch crates/*/src/lib.rs` and rebuild (TASK-009
phantom-red lesson).

Planner probe (compiled and ran, `scratch/probe/`, log `scratch/probe/probe_test.log`, exit 0): the exact API shapes
below compile against the workspace's dependency graph — `#[derive(Resource, SettingsGroup, Reflect)]` +
`#[reflect(Resource, SettingsGroup, Default)]` + `#[settings_group(group = "game")]`; `SettingsPlugin::new(id)` in a
`MinimalPlugins` app inserts the registered group at `build` with its `Default` (printed
`Some(GameSettings { mouse_sensitivity: 1.0, volume: 1.0, invert_y: false })`); `#[derive(Asset, TypePath,
AsBindGroup)]` with `#[uniform(0)]` over a `#[derive(ShaderType)]` struct holding `[Vec4; 16]` + `#[texture(1)]
#[sampler(2)] Handle<Image>` and `impl UiMaterial`; `UiMaterialPlugin::<M>`, `MaterialNode::<M>`;
`EditableText { max_characters: Some(19), .. }` + `EditableTextFilter::new(|c| c.is_ascii_digit())` + `AutoFocus`;
`ContextActivity::<OnFoot>::INACTIVE`; `bevy::audio::GlobalVolume::new(Volume::Linear(..))`;
`const T: OnTransition<S> = OnTransition { exited: .., entered: .. }`.

---

## 1. Understanding (what exists today)

### Flow and world lifetime (gta_sim)
- `crates/gta_sim/src/flow/mod.rs:11-21` — `GameState { Loading (default), Playing, Wasted, Busted }`, sub-states
  `WastedPhase`, `BustedPhase`; sets `PlayingSystems` (`in_state(Playing)`), `NpcSystems` (Playing|Wasted|Busted),
  both in `FixedUpdate`. No `Paused`, no main menu. GDD §12 names `GameState (Loading, Playing, Paused, Wasted,
  Busted)`.
- `crates/gta_sim/src/lib.rs:41-173` — `compose_sim(app, root, WorldSource)`; RNG plugins take `combat_seed` =
  city seed at build: `CombatPlugin { seed }` (`CombatRng::seeded`, `combat/hitscan.rs:95-99`),
  `PopulationPlugin` (`NpcRng`, stream 1, `population/mod.rs:209-222`), `GangPlugin` (`GangRng`, stream 2,
  `gang/mod.rs:433-449`), `PolicePlugin` (`PoliceRng`, stream 3, `police/mod.rs:322-338`).
- `crates/gta_sim/src/world/mod.rs:60-108` — `WorldPlugin`: City mode inserts `CitySeed(seed)`, registers
  `CitySeed`/`CityLayoutHash`/`CityLandmarks`; `OnEnter(Loading)` → `start_city_generation` (async task,
  `world/city.rs:62-75`), `Update` `apply_city_generation` in `WorldSystems::Generation` (Loading only) spawns ground,
  4 edge walls, one convex collider per block, one collider per building, inserts `PlayerSpawn`, `HospitalSpawn`,
  `PoliceStationSpawn`, `CityLandmarks`, `CityLayoutHash`, `City`, then `next.set(Playing)` (`city.rs:77-128`).
  `start_city_generation` reads `Res<CitySeed>` at `OnEnter(Loading)` — so setting `CitySeed` before entering
  `Loading` already selects the city.
- Every per-city one-shot is `OnTransition { exited: Loading, entered: Playing }`: `player/mod.rs:30-36`
  (`spawn_player`), `combat/mod.rs:165-174` (health/armour pickups, range dummies + weapon/bat pickups),
  `navigation/mod.rs:350-357` (`SidewalkGraph`), `gang/mod.rs:531-538` (`GangTerritories`), client
  `hud/mod.rs:23-29` (HUD roots), `visuals/city.rs:25-31` (chunk-mesh task). Re-entering `Loading → Playing` fires all
  of them again — which is exactly what a new city needs, provided the old city is torn down first.
- Runtime spawns during play: civilians (`population/mod.rs:562`), gang members (`population/gangs.rs:146`), police
  (`police/dispatch.rs:149`), dropped guns (`gang/behavior.rs:582`, `police/behavior.rs:440`). All characters carry
  `Character` (`character/mod.rs:30-41`, `#[require(..)]` list) via `character_components`; pickups carry `Pickup`
  (`combat/pickups.rs:18-22`), `WeaponPickup`, `BatPickup`.
- World-lifetime resources (inventory by grep of `derive(Resource`): `City`, `CityLayoutHash`, `CityLandmarks`,
  `PlayerSpawn`/`HospitalSpawn`/`PoliceStationSpawn` (overwritten by generation), `SidewalkGraph`,
  `GangTerritories`, `GangHeat`, `PlayerTerritory`, `PoliceDispatcher`, `ArrestAttempt { cop: Option<Entity> }`
  (`police/arrest.rs:18-21`), `PoliceAlert`, `PopulationPhase` (`InitialFill`/`Steady`, `population/mod.rs:202-206`),
  `CameraView`, `StimulusLog`, `WantedLevel`, `Crimes` (holds offender entities, `wanted/crimes.rs:48-51`), the four
  RNGs. Messages: `ShotFired`, `BulletTrace`, `DamageDealt`, `MeleeHit`, `melee::Strike` (combat), `PoliceCall`
  (civilian), `DebugDamage` (player).
- Existing reset helpers to reuse: `wanted::reset_wanted` + `drop_queued_calls` (`wanted/mod.rs:215-223`),
  `flow::wasted::drop_queued_damage` (`flow/wasted.rs:158-160`).

### Time
- `bevy_time-0.19.1/src/fixed.rs:243-258` — `FixedMain` runs off `Time<Virtual>::delta`; `Time<Virtual>::pause()`
  (`virt.rs:215`) → 0 fixed ticks. `TimeUpdateStrategy::FixedTimesteps(n)` goes real → virtual → fixed
  (`lib.rs:181-186`), so pause is honoured in headless tests too. `Time` in `Update`/`PostUpdate` = virtual clock
  (`fixed.rs:257`), i.e. frozen while paused.
- `flow/wasted.rs:279-286` sets `Time<Virtual>` relative speed 0.3 during Wasted; pause must not stack with it (pause
  only from `Playing`).

### States API
- `bevy_state-0.19.1/src/app.rs:122-152` — `insert_state` after `init_state` overwrites the state and the initial
  transition event (so the client can start in `MainMenu` without touching `compose_sim`).
- `DespawnOnExit` works for sub-states (`add_sub_state` calls `enable_state_scoped_entities`, `app.rs:208`).
- The first `StateTransition` (initial `OnEnter`) runs before `Startup` (`src/menu/mod.rs:8-9` comment, relied on by
  `UiFonts`).

### Client (src/)
- `src/main.rs:49-61,165-274` — `--seed N` or clock seed, `compose_sim(.., City { seed })`, loads/validates client
  configs (`preflight`), adds presentation plugins. No `SettingsPlugin` yet.
- `src/menu/mod.rs` — only `UiFonts` + loading screen (`OnEnter(Loading)`, `DespawnOnExit(Loading)`).
  `src/menu/config.rs` — `UiConfig` (`assets/ui/strings.ron`, `deny_unknown_fields`, `validate()`), HUD numbers
  live here by precedent ("Q1: HUD numbers live here, no separate hud.ron", `config.rs:9`).
- `src/hud/wasted.rs:35-67` — `title_screen(..)` (full-screen node, backdrop `ui.wasted_backdrop`, one title text,
  `DespawnOnExit(exit)`), used by Wasted and Busted. The orchestrator asked to **reuse it, not duplicate**.
- `src/input/mod.rs` — BEI context `OnFoot` on one entity spawned at `Startup` (`spawn_input`, `:95-118`);
  `capture_cursor` at `Startup` (`:120-123`); `cursor_toggle` (`:125-140`): Esc releases the cursor, LMB recaptures;
  `write_action_intent` has a `wait_release` latch for the recapture click (`:181-196`); digits 1-4 are weapon slots.
- `src/camera/mod.rs:302-319` — `apply_mouse_look` (skips while `!CursorCaptured`), sensitivity from `camera.ron`;
  `reset_pivot` on respawn (`:408-411`); `OrbitCamera.yaw` is the camera yaw (convention GDD §3.2).
- `src/audio/mod.rs:165-179` — shots are one-shot `AudioPlayer`s; bevy_audio applies `GlobalVolume` when a sink is
  created (`bevy_audio-0.19.1/src/audio_output.rs:87`).
- `src/juice/shake.rs:83-93` — `shake_camera` writes `CameraShake.rotation`; `src/vfx/mod.rs:73-98` — muzzle flash
  (emissive sphere + point light) per `ShotFired`.
- `src/visuals/city.rs` — `CityMeshTask` → `PendingCitySpawn` (time-sliced spawn of `CityChunk`s and `CityProp`s).
  Neither is cancelled today (never needed). `src/visuals/city_gate.rs` — headless presentation harness
  (`compose_sim` + `CityVisualsPlugin`) with `city_is_built_once_across_respawn`.
- `src/remote/mod.rs` — `BrpExtrasPlugin` (feature `dev`).

### Engine facts for the UI (verified in source)
- **Rotated UI nodes are not clipped correctly**: `bevy_ui_render-0.19.1/src/lib.rs:1726-1745` clips by moving quad
  vertices against an axis-aligned rect, comment "this won't work with rotation/scaling"; same in
  `ui_texture_slice_pipeline.rs:496`, `gradient.rs:740`, `box_shadow.rs:399` (upstream: bevy issue #9381). A rotated
  big map image inside an `Overflow::clip()` frame would spill/distort. → the map is drawn by a shader.
- `UiMaterial` (`bevy_ui_render-0.19.1/src/ui_material.rs`), in `bevy::prelude` via `ui_render::prelude`
  (`bevy_internal-0.19.1/src/prelude.rs:91-92`); fragment input `UiVertexOutput { uv, size, .. }`
  (`ui_vertex_output.wgsl`).
- `UiTransform.rotation` rotates **clockwise** in UI space (`bevy_ui-0.19.1/src/ui_transform.rs:130-137`).
- `EditableText` (`bevy_text-0.19.1/src/editing.rs:106-164`), input via `EditableTextInputPlugin`
  (`bevy_ui_widgets-0.19.1/src/text_input.rs`, in `UiWidgetsPlugins`, in `DefaultPlugins` under default feature
  `ui`): on `FocusedInput<KeyboardInput>` it inserts `input.text` for `Key::Character`; Enter is **not** consumed when
  `allow_newlines == false` (propagates). `EditableText` gets `Node` as a required component
  (`text_input.rs:528`). `AutoFocus` (`bevy_input_focus-0.19.1/src/autofocus.rs`) sets `InputFocus` on spawn.
  `dispatch_focused_input` ignores the event's `window` field (`bevy_input_focus-0.19.1/src/lib.rs:341-383`).
- `bevy_brp_extras-0.22.6/src/keyboard/typing.rs` — `type_text` presses one char per frame with `text: Some(c)`,
  `logical_key: Key::Character(c)`; `'\n'` → Enter. So QA typing reaches a focused `EditableText`.
- `bevy::prelude::Button` is the classic `bevy_ui::widget::Button` with `Interaction` (`bevy_ui-0.19.1/src/lib.rs:70`).
- Settings (`bevy-settings-0.19.1/src/lib.rs`): registry scan at `SettingsPlugin::build` (`:96-125`, needs the type
  registered **before**, with `ReflectDefault` + `ReflectSettingsGroup`); save is manual — `SaveSettingsDeferred`
  (debounced, `:243-263`) ticks on `Res<Time>` in `PostUpdate` (`:585-594`) = the **virtual** clock → it does not
  fire while paused; `SaveSettingsSync::IfChanged` (`:205-219`); file `%LOCALAPPDATA%\<id>\settings.toml` on Windows
  (`store_fs.rs`, `bevy_platform-0.19.1/src/dirs/windows.rs:32-34`, no env override).
- Exit paths: window close → `close_when_requested` + `exit_on_all_closed` in `Last`, `ExitSystems`
  (`bevy_window-0.19.1/src/lib.rs:144-151`, `system.rs:9`); BRP `shutdown` writes `AppExit` in `Update`
  (`bevy_brp_extras-0.22.6/src/shutdown.rs:46-56`, `plugin.rs:368`).

### Minimap inputs
- `citygen::CityLayout` (`crates/citygen/src/layout.rs`): `blocks[].{curb, inner, district, is_park}` (convex, CCW —
  `gang/territory.rs:43-49` validates CCW), `buildings[].{center, axis, half_extents, kind}` (oriented rectangles),
  `gang_districts: [u32; 2]`, `ground_size = size + 2·ground_margin` (1400 m shipped). Park blocks have no lots or
  buildings (`lots.rs:37-40`). `contains_convex` (`geom.rs:127`).
- Territory = blocks whose `district == gang_districts[g]` (`gang/territory.rs:75-88`), a pure function of the layout.
- Gang colour `GangConfig.gangs[g].tint` (`assets/gang/gangs.ron`); police colour `CharacterVisualConfig.police_tint`
  (`assets/character/visual.ron`). The client already reads sim configs directly (`hud/mod.rs:106-123` reads
  `HealthConfig`), so reading `GangConfig` is precedent.
- Search circle: `WantedLevel.last_known` + star row `search_radius` (`wanted/mod.rs:50-59,155-168`); the row rule
  `rows[stars.max(1) - 1]` lives inline in `search_step` (`wanted/search.rs:79`). Cop view: `cop_view_cone_deg`
  (110), `cop_view_distance` (35 m); cop forward = `rotation * NEG_Z` (`search.rs:113-121`).

### Research (web)
- Minimaps are usually a static top-down picture sampled with rotated/translated UVs in a shader, or a top-down
  render-to-texture camera ([gameidea](https://gameidea.org/2024/12/13/how-to-make-mini-map-or-radar-for-3d-game/),
  [HIVE-RD Unity tutorial](https://www.hive-rd.com/blog/unity3d-basic-minimap-tutorial/),
  [Godot recipe](https://kidscancode.org/godot_recipes/4.x/ui/minimap/index.html)); GDD §7 already rejected the
  second camera. We take the static picture + shader UV rotation.
- Bevy UI clipping only works for axis-aligned, untransformed rects ([bevy #9381](https://github.com/bevyengine/bevy/issues/9381)) —
  matches the source above.
- Settings groups + reverse-domain id per [Bevy 0.19 release notes](https://bevy.org/news/bevy-0-19/).
- Accessibility: offer an option to turn off / reduce camera shake and flashing
  ([Xbox Accessibility Guideline 117](https://learn.microsoft.com/en-us/gaming/accessibility/xbox-accessibility-guidelines/117));
  GDD §7 settings group already lists "уменьшить тряску" and "без вспышек".

---

## 2. Approach

1. **Flow (sim, `flow/`)**: add `GameState::MainMenu` and `GameState::Paused`. `OnEnter(Paused)` →
   `Time<Virtual>::pause()`, `OnExit(Paused)` → `unpause()` (GDD §7 "Пауза (Esc, `Time<Virtual>::pause` через
   `flow`)"). Pause is entered only from `Playing` (the client never requests it elsewhere). A public
   `const NEW_CITY: OnTransition<GameState> = OnTransition { exited: Paused, entered: Loading }` is the one schedule
   where every domain drops its city-lifetime state. Starting a city = write `CitySeed`, `NextState(Loading)`; the
   existing generation reads the seed at `OnEnter(Loading)`.
2. **Teardown (sim, `world/`)**: a public marker `CityScoped` ("lives as long as the current city"), attached via
   `#[require(CityScoped)]` to `Character`, `Pickup`, `WeaponPickup`, `BatPickup`, `CityGround`, `CityEdgeWall`,
   `CityBlock`, `CityBuilding`, and explicitly by the client on everything its `Loading → Playing` one-shots spawn
   (HUD roots, city chunks/props, minimap root). One `NEW_CITY` system `try_despawn`s every `CityScoped` entity
   (children go with it). Each domain resets its own resources in `NEW_CITY` (table §3 step 3). RNGs are reseeded
   from `CitySeed` on **every** `OnEnter(Loading)` (City mode only), so "seed N from the menu" rolls the same streams
   as `--seed N`. Rejected: process restart with `--seed` (window closes/reopens, `brp.py` loses the PID, orphans;
   AGENTS "правильная система важнее костылей").
3. **Minimap**: pure `citygen::minimap` (GDD §2.2 step 8: "CPU-растеризация layout в картинку (чистая функция,
   тестируется)") — `rasterize(layout, style) -> Raster` (RGBA8 sRGB, 1 texel = 1/ppm m, row 0 = smallest z = north)
   and `project` / `map_px` / `heading_on_map` (the map turns with the camera: ahead = up). Both gated in
   `cargo test -p citygen` (the GDD's "headless" definition). The client (`src/minimap/`) rasterizes once per city
   on `AsyncComputeTaskPool` (marker pattern), draws the map, territories (baked in the raster), search circle, cop
   cones and the player arrow with **one `UiMaterial` fragment shader** over an unrotated node (rotated nodes are
   mis-clipped, §1), and draws dots (police, gang, pickups) and landmark glyphs (hospital, station) as ordinary
   unrotated UI nodes positioned by `map_px`.
4. **Menus (`src/menu/`)**: main menu (only when no `--seed`: every existing QA/showcase script passes `--seed`, so
   they keep booting straight into the city), pause menu (Esc) with the current seed visible and an auto-focused
   seed field, settings screen as a client sub-state `PauseMenu { Main, Settings }`. Enter in the seed field (empty =
   random) or the "Новый город" / "Новая игра" button starts a city. `title_screen` moves from `hud/wasted.rs` to
   `menu/widgets.rs` and becomes the shared screen shell (Wasted/Busted/pause/main menu/settings).
5. **Settings (`src/settings/`)**: `GameSettings` group (5 neutral multipliers/flags from GDD §7),
   `SettingsPlugin::new(id)` with `id` = reverse-domain const `com.github.pockerhead.maw-make-gta` (repo URL, as
   bevy-settings recommends), overridable by `--settings-id` so `tools/qa/brp.py` uses a `.qa` id and QA never reads
   or writes the owner's file. `SaveSettingsDeferred` after each change (GDD), `SaveSettings::IfChanged` when the
   settings screen closes (the deferred timer is frozen while paused, §1), `SaveSettingsSync::IfChanged` on `AppExit`
   in `Last` after `ExitSystems`. Consumers: camera (sensitivity, invert Y), `GlobalVolume`, shake (`juice.ron`
   reduced scale), muzzle flash (skipped).

Buffered vs immediate signals: no new `Message`/`Event` type. Transitions use `NextState` (buffered, applied in
`StateTransition`). Buttons are read as `Changed<Interaction>` in `Update`. Save-on-exit reads the existing
`AppExit` message with its own `MessageReader`.

---

## 3. Steps

Order: sim flow + teardown (1-6) → citygen minimap (7-8) → client (9-18) → QA (19-20). Run
`cargo test -p gta_sim -p citygen` after step 8; `cargo test -p gta_like --bin gta_like` + `cargo clippy -- -D
warnings` after step 18.

### Step 1 — `crates/gta_sim/src/flow/mod.rs`: new states, pause, `NEW_CITY`
- `GameState`: add `MainMenu` (doc: "waiting for a seed; no city exists; the client starts here without `--seed`")
  and `Paused` (doc: "`Time<Virtual>` is paused; entered only from `Playing`"). Keep `#[default] Loading` (headless
  tests unchanged).
- `pub const NEW_CITY: OnTransition<GameState> = OnTransition { exited: GameState::Paused, entered:
  GameState::Loading };` with a one-line `///` ("every domain drops its city-lifetime state here").
- `add_systems(OnEnter(GameState::Paused), pause_time)` / `OnExit(GameState::Paused), resume_time)` — two 3-line
  fns in `flow/mod.rs` (`ResMut<Time<Virtual>>` → `pause()` / `unpause()`).
- `add_systems(NEW_CITY, wasted::drop_queued_damage)` (reuse; `Messages<DebugDamage>` clear).
- Update the `NpcSystems` doc comment: `Paused` needs no entry because the fixed loop does not run while paused.
- Why: GDD §7/§12; `OnExit(Paused)` runs before `OnTransition`, so a new city never starts frozen.

### Step 2 — `crates/gta_sim/src/world/`: `CityScoped`, teardown, RNG-reseed hook
- `world/mod.rs`: `#[derive(Component, Reflect, Default)] #[reflect(Component)] pub struct CityScoped;` (doc: "Lives
  as long as the current city: despawned on `flow::NEW_CITY`"), `register_type`.
- `world/city.rs`: add `#[require(CityScoped)]` to `CityBuilding`, `CityGround`, `CityEdgeWall`, `CityBlock`
  (derive `Default` not needed for markers used via require of the *target*; `CityScoped` itself derives `Default`).
- `WorldPlugin` City branch: `.add_systems(NEW_CITY, (despawn_city, drop_city))` where
  `despawn_city(mut commands, scoped: Query<Entity, With<CityScoped>>)` → `commands.entity(e).try_despawn()`
  (`try_`: a scoped child of a scoped parent is gone already — bevy-ecs domain rule) and `drop_city` removes
  `City`, `CityLayoutHash`, `CityLandmarks` (`commands.remove_resource`), so QA/menu never read the old hash during
  the new load.
- Why: one generic teardown; the leak gate (step 6, case A) catches any future spawn site without the marker.

### Step 3 — per-domain resets in `NEW_CITY` (each domain owns its rows)
| Domain / file | State | Reset in `NEW_CITY` |
|---|---|---|
| `character/mod.rs:30-41` | `Character` | add `CityScoped` to its `#[require(..)]` list (player, dummies, civilians, gang, police) |
| `combat/pickups.rs` | `Pickup`, `WeaponPickup`, `BatPickup` | `#[require(CityScoped)]` on each (covers dropped guns) |
| `combat/mod.rs` | `Messages<ShotFired/BulletTrace/DamageDealt/MeleeHit/melee::Strike>` | one `clear_combat_messages` system |
| `navigation/mod.rs` | `SidewalkGraph` | `remove_resource` (also stops `NpcSystems` until rebuilt) |
| `gang/mod.rs` | `GangTerritories` | `remove_resource` |
| `gang/mod.rs` | `GangHeat` | `left` = zeros (same length as today's build) |
| `gang/mod.rs` | `PlayerTerritory` | `default()` |
| `police/mod.rs` | `PoliceDispatcher`, `ArrestAttempt`, `PoliceAlert` | `default()` each |
| `population/mod.rs` | `PopulationPhase` | `InitialFill` (the new city gets its load-time fill) |
| `population/mod.rs` | `CameraView` | `CameraView(None)`: no spawning until the camera publishes the new view (stale cone would place the initial fill against the old camera) |
| `perception/mod.rs` | `StimulusLog` | `clear()` (entries point at old places/ticks) |
| `wanted/mod.rs` | `WantedLevel`, `Crimes`, `Messages<PoliceCall>` | reuse `reset_wanted` + `drop_queued_calls` |
| `player/` via flow | `Messages<DebugDamage>` | step 1 |
Deliberately **not** reset (monotonic or overwritten): `AiClock`, `SlotCursor`, `AttackSerial`, `RouteLoad`,
`PopulationLoad`, `PerceptionLoad` (per-tick), `PlayerSpawn`/`HospitalSpawn`/`PoliceStationSpawn` (overwritten
before `Playing`), `WastedClock`/`BustedClock`.
- Each domain gets one small `reset_for_new_city` fn (or a tuple of existing fns) registered in its plugin with
  `add_systems(NEW_CITY, ..)`; no cross-domain resets. Files stay far below 750 lines (population 628 → ~640).

### Step 4 — RNG reseed from `CitySeed` on every `OnEnter(Loading)`
- In `CombatPlugin`, `PopulationPlugin`, `GangPlugin`, `PolicePlugin`: `add_systems(OnEnter(GameState::Loading),
  reseed_x.run_if(resource_exists::<CitySeed>))`, body `*rng = XRng::seeded(seed.0)`. TestArea has no `CitySeed` →
  keeps its build seed 0 (existing gates unchanged). First boot with `--seed N`: build seed = N → reseed is a no-op.
- Update the `compose_sim` comment at `lib.rs:129` ("A city run rolls from its own seed" — now "from `CitySeed` at
  every load").
- Why: menu seed / new-city seed N must equal `--seed N` for the population streams (TASK-010 lesson: streams drive
  density gates).

### Step 5 — `wanted/`: one derivation of the search circle
- `wanted/mod.rs`: `pub fn search_row(rows: &[StarRow; STARS], stars: u8) -> &StarRow { &rows[usize::from(stars.max(1)) - 1] }`
  and `impl WantedLevel { pub fn search_circle(&self, rows: &[StarRow; STARS]) -> Option<(Vec3, f32)> }` →
  `Some((last_known, search_row(rows, stars).search_radius))` only when `stars > 0` (no stars on the HUD = no circle).
- `wanted/search.rs:79`: `search_step` uses `search_row` (pure refactor; `wanted_search.rs` gates stay green).
- Why: the minimap must draw the circle the sim actually uses; one rule, one place.

### Step 6 — headless gates `crates/gta_sim/tests/new_city.rs` (new)
Uses `common::{composed_app, city_app, run_ticks, set_view, chase_view, golden, player, position}` and
`police_support::raise_heat`. Helpers in the file: `pause(app)` (NextState Paused + updates until state == Paused),
`new_city(app, seed)` (set `CitySeed(seed)`, NextState Loading, one `update()`), `until_playing(app)` (like
`city_app`'s loop).
- **G1 `pause_freezes_the_fixed_clock`** (correctness): `headless_app()` (TestArea) settled; set `MoveIntent` forward
  (`common::set_intent`); pause; record `Time<Fixed>::elapsed` and player position; 10 updates → both unchanged and
  `Time<Virtual>::is_paused()`; NextState Playing; 10 updates → elapsed grew by ≥ 8 ticks and the player moved
  ≥ 0.5 m. Flip: comment out `pause_time` → RED on elapsed.
- **G2 `main_menu_waits_for_a_seed`**: `composed_app(City { seed: 7 })` built by hand with
  `app.insert_state(GameState::MainMenu)` **before** `finish()` (mirror what `main.rs` will do); 30 updates → state
  `MainMenu`, no `City`, no `CityLayoutHash`, zero `Player`. Then `new_city`-like start from the menu
  (`CitySeed(2)`, NextState Loading) → at the first update in `Loading`: four RNG rows, **one assert each**:
  `CombatRng.0 == CombatRng::seeded(2).0`, `NpcRng.0 == NpcRng::seeded(2).0`, `GangRng.0 == GangRng::seeded(2).0`,
  `PoliceRng.0 == PoliceRng::seeded(2).0` (`ChaCha8Rng: PartialEq`, `rand_chacha-0.10.0/src/chacha.rs:210`;
  nothing draws in `Loading` because NPC/Playing sets are state-gated); then until Playing → `CityLayoutHash ==
  golden(2)`. Flip: remove the reseed from one plugin → that row RED (do each of the 4).
- **G3 `new_city_replaces_the_city`** (silent-defect gate). Seed 1 → Playing. Baseline `B0` = entities with
  `Transform` observed while the app was still in `Loading` before the first city (build the app by hand like
  `city_app`, record after the first `update()`). `set_view(chase_view(..))`, run 400 ticks; `raise_heat(app, 200)`
  (2 stars) and run until ≥ 1 `PoliceUnit` exists (bounded 1500 ticks, `GATE BROKEN` otherwise). To exercise the
  gang spawner, `place_player` near `GangTerritories.gangs[0].posts[0]` (on its sidewalk, checked against buildings)
  before the 400 ticks. Preconditions (`GATE BROKEN` if not met): ≥ 1 civilian, ≥ 1 `GangMember`, ≥ 1 `PoliceUnit`,
  `WantedLevel.heat == 200`, `StimulusLog` or `Crimes` may be empty (not required). Snapshot `A` = all `Character`, pickup, city-collider entities.
  Pause → `new_city(app, 2)` → **case A (state == Loading, right after the transition frame)**:
  - A1 no entity with `Transform` outside `B0` (generic leak detector: any spawn site without `CityScoped` is RED);
  - A2..A14, one assert per row of the step-3 table: `City`, `CityLayoutHash`, `CityLandmarks`, `SidewalkGraph`,
    `GangTerritories` absent; `GangHeat.left` all 0; `PlayerTerritory(None)`; `PoliceDispatcher`/`ArrestAttempt`/
    `PoliceAlert` == default (compare fields); `PopulationPhase::InitialFill`; `CameraView(None)`; `StimulusLog` empty;
    `WantedLevel == default()`; `Crimes.incidents()` empty; each of the 7 `Messages<..>` has `len() == 0`;
  - A15 `!Time<Virtual>::is_paused()`.
  **case B (after `until_playing`)**: `CitySeed == 2`; `CityLayoutHash == golden(2)`; exactly one `Player`, standing
  at `PlayerSpawn + float_height` (±0.05 m after `settle`); `CityBuilding` count == `City.buildings.len()`;
  `CityGround == 1`; `CityEdgeWall == 4`; `CityBlock` count == blocks with `curb.len() >= 3`; `Pickup == 2`;
  `Dummy == WeaponsConfig.range.dummies`; no entity of `A` alive; `set_view` + 200 ticks → `Time<Fixed>` advanced
  and ≥ 1 civilian spawned (liveness of the new city's NPC loop).
  Flips (record each): drop `CityScoped` from `Character`'s require → A1 RED; delete `despawn_city` → A1 RED;
  delete each reset row → its assert RED; delete `resume_time` → A15 + B liveness RED; delete `drop_city` → A2 RED.
- Numbers derived, not intended: dummy count and pickup count read from the loaded configs; golden hashes from
  `golden_hashes.txt`; 200 heat = 2 stars by the shipped table (40/180/…, `wanted/mod.rs:229-254`).

### Step 7 — `crates/citygen/src/minimap.rs` (new, `pub mod minimap` in `lib.rs`)
```rust
pub struct RasterStyle { pub px_per_m: f32, pub road: [u8; 4], pub sidewalk: [u8; 4], pub block: [u8; 4],
    pub park: [u8; 4], pub building: [u8; 4], pub territory: [[u8; 4]; 2] }   // sRGB RGBA8; territory alpha = [3]
pub struct Raster { pub width: u32, pub height: u32, pub origin: Vec2, pub px_per_m: f32, pub rgba: Vec<u8> }
pub fn rasterize(layout: &CityLayout, style: &RasterStyle) -> Raster
pub fn project(point: Vec2, center: Vec2, yaw: f32) -> Vec2       // metres: x right of the view, y ahead
pub fn map_px(point: Vec2, center: Vec2, yaw: f32, px_per_m: f32) -> Vec2   // UI px offset from the map centre, y down
pub fn heading_on_map(facing_yaw: f32, camera_yaw: f32) -> f32    // radians, counter-clockwise from map-up
impl Raster { pub fn texel(&self, col: u32, row: u32) -> [u8; 4]; pub fn hash(&self) -> u64 }
```
- Convention (doc comment on `Raster`, the contract with the shader): the raster covers the ground square
  `[-g/2, g/2]²` (`g = layout.ground_size`), `origin = (-g/2, -g/2)` in layout (x, z); texel `(col, row)` covers x ∈
  `origin.x + [col, col+1)/ppm`, z ∈ `origin.y + [row, row+1)/ppm`; **row 0 = smallest z (north, forward is −Z)**;
  `width = height = ceil(g · ppm)`; sampling at texel centres.
- Layers in order, each a scan of the polygon's texel bounding box testing the texel centre: fill all with `road`;
  every block with `curb.len() >= 3`: `curb` → `sidewalk`, `inner` → `park` if `is_park` else `block`; every
  building's oriented rectangle (`|d·axis| <= hx && |d·axis.perp()| <= hy`, base footprint only) → `building`; every
  block whose `district == gang_districts[g]`: blend `territory[g]` over the `curb` polygon with
  `out = (dst·(255−a) + src·a + 127) / 255` per RGB channel, alpha stays 255. Precompute each polygon's edge normals
  once (not `contains_convex`'s per-call `normalize`) — ~2 MP × a few edges, async anyway.
- `project`: `d = point − center`, `(s, c) = yaw.sin_cos()`, return `(d.x·c − d.y·s, −d.x·s − d.y·c)` (right =
  `(cos, −sin)`, forward = `(−sin, −cos)` in layout (x, z) — GDD §3.2 `R_y`). `map_px`: `Vec2::new(m.x, −m.y) ·
  px_per_m`. `heading_on_map`: `facing_yaw − camera_yaw`.
- `hash`: FNV-1a (`rng::fnv1a64`) over `width`, `height`, `origin` quantised to mm, `px_per_m.to_bits()`, `rgba`.
- Worked examples (they are the test rows): yaw 0, point `center + (0, −10)` (10 m ahead: forward `(0,−1)`) → `(0,
  10)`; yaw 90° (`s = 1, c = 0`), point `center + (−10, 0)` → `x = −10·0 − 0·1 = 0`, `y = 10·1 − 0 = 10`; yaw 180°
  (`s = 0, c = −1`), point `center + (0, 10)` → `x = 0`, `y = −10·(−1) = 10`. Right side: yaw 0 `(10, 0)` → `(10, 0)`;
  yaw 90° `(0, −10)` → `x = 0 − (−10)(1) = 10`, `y = 0`; yaw 180° `(−10, 0)` → `x = −10·(−1) = 10`, `y = 0`.
  `map_px` of the "ahead" rows at 2 px/m → `(0, −20)` (negative = up in UI). Arrow: camera 0, facing 90° →
  `heading 90°` = left; camera 90°, facing 90° → 0 = up; camera 90°, facing 0 → −90° = right (each equals the
  direction of `project(center + facing_dir, center, camera_yaw)`).

### Step 8 — citygen gates `crates/citygen/tests/minimap.rs` + `crates/citygen/tests/golden_minimap.txt`
Test style literal (fixed in the test, independent of `strings.ron` so colour tuning never re-blesses):
`px_per_m 1.0`, six distinct colours, territory alphas 96 and 160. Layouts from `common::layouts()`.
- **M1 `raster_hashes_match_golden`** (seeds 1, 2, 42; golden file format like `golden_hashes.txt`, bless test
  `#[ignore] bless_print_minimap_golden`, bless command in the header comment). Liveness of determinism + drift guard.
- **M2 `raster_differs_by_seed`**: three hashes pairwise different (a blank raster is deterministic too).
- **M3 `raster_marks_known_places`** — correctness; texel index computed **in the test** from the documented
  convention (`col = floor((x − origin.x)·ppm)`, `row = floor((z − origin.y)·ppm)`), never via a raster helper, so a
  mirrored/rotated raster is RED. Seed 1, one case per layer (each its own assert): (a) road — `roads.nodes[roads.center]`
  → `road`; (b) building — hospital `center` → `building`; (c) park — centroid of `blocks[landmarks.park].inner` →
  `park`; (d) block — a point 1 m inside an `inner` vertex of a non-park, non-gang block that no building contains
  (search; `GATE BROKEN` if none) → `block`; (e) sidewalk — midpoint of `curb[0]` and `inner[0]` of a non-gang block →
  `sidewalk`; (f) territory — same construction in a block of `gang_districts[0]` → blended colour by the formula
  above; (g) asymmetry — for (b) also assert the texel mirrored in rows (`height−1−row`) is **not** the building
  colour unless the layout has a building there (choose a hospital whose mirror texel is road; `GATE BROKEN`
  otherwise).
- **M4 `camera_ahead_is_straight_up`** (the AC gate): for yaw 0°, 90°, 180°: forward computed independently as
  `(glam::Quat::from_rotation_y(yaw) * Vec3::NEG_Z).xz()`, point = centre + 10·forward with a non-zero centre
  `(123.0, −45.0)`; assert `|project.x| < 1e-4`, `|project.y − 10| < 1e-4`, and `map_px(.., 2.0)` = `(≈0, ≈−20)`.
  Plus the three right-side rows (right = `Quat·X`) → `(10, 0)` — one assert per row, 6 rows. Flip: swap the sign of
  `s` in `project` → yaw 90° rows RED (yaw 0/180 rows stay green — that is why all three yaws exist).
- **M5 `arrow_points_along_facing`**: the three (camera, facing) rows of step 7; expected direction
  `(−sin h, cos h)` equals `project(center + facing_dir, center, camera).normalize()`.
Flip-RED for M1/M3: swap `row` for `height−1−row` in `rasterize` → M3(a..g) and M1 RED.

### Step 9 — client config: `assets/ui/strings.ron` + `src/menu/config.rs`, `src/minimap/config.rs`, `assets/juice/juice.ron`
All new tuning values are data (AC "every new tuning value in its §12 file"); `validate()` extended field by field
(non-empty strings, `{seed}` placeholder, `positive`, `unit` colours, ranges ordered).
- `UiConfig.menu: MenuConfig` (strings.ron top level): texts `title`, `new_game`, `seed_label`, `seed_hint`,
  `quit`, `paused`, `current_seed` (contains `{seed}`), `resume`, `new_city`, `settings`, `back`, `sensitivity`,
  `volume`, `invert_y`, `reduce_shake`, `no_flashes`, `on`, `off`; layout `item_size`, `item_gap`, `button_width`,
  `button_height`, `backdrop: Rgba` (pause), `menu_backdrop: Rgba` (main, opaque), `button_color: Rgba`,
  `button_hover_color: Rgba`, `text_color: Rgb`, `field_color: Rgba`; settings steps `sensitivity: (min, max, step)`
  (0.25, 3.0, 0.25), `volume_step: 0.1`.
- `UiConfig.hud.minimap: MinimapConfig` (struct in `src/minimap/config.rs`, HUD numbers precedent): `size` 220 px,
  `view_radius` 120 m, `raster_px_per_m` 1.0 (validate `(0, 4]`: 1400 m × 4 = 5600 ≤ 8192 wgpu default max texture
  side), colours `road`, `sidewalk`, `block`, `park`, `building`, `outside` (Rgb/Rgba), `territory_alpha` (0..1),
  `rim_color: Rgba`, `rim_px`, `search_fill: Rgba`, `search_ring: Rgba`, `search_ring_px`, `cone_color: Rgba`,
  `arrow_color: Rgb`, `arrow_px`, `dot_px`, `pickup_color: Rgb`, `hospital: (glyph, color)`, `station: (glyph,
  color)`, `glyph_size`. Start values: owner tunes; pick readable GTA-ish defaults (dark asphalt, grey blocks, green
  parks, 35 % territory, translucent red cones 0.25 α, blue-white search ring). Minimap bottom-left at `hud.margin`.
- `juice.ron` `shake.reduced_scale: 0.3` + `ShakeConfig.reduced_scale` + validate `[0, 1]`; update the `cfg()` literal
  in `juice/shake.rs` tests.
- Why: `config.rs` would pass 350 lines with everything inline → `MinimapConfig` in its own domain file.

### Step 10 — `src/settings/mod.rs` (new domain): group, plugin, persistence, glue
- `GameSettings { mouse_sensitivity: f32 (1.0), volume: f32 (1.0), invert_y: bool, reduce_shake: bool, no_flashes: bool }`,
  derives exactly as the probe (`Resource, SettingsGroup, Reflect, Clone, Debug, PartialEq`,
  `#[reflect(Resource, SettingsGroup, Default)]`, `#[settings_group(group = "game")]`), `impl Default` with the
  neutral values (GDD §7: neutral defaults, the base numbers stay in `camera.ron`/`mix.ron`/`juice.ron`).
- `pub const SETTINGS_APP_ID: &str = "com.github.pockerhead.maw-make-gta";` (identity, not tuning).
- `pub struct GameSettingsPlugin { pub app_id: String }` → `register_type::<GameSettings>()` **then**
  `add_plugins(SettingsPlugin::new(&self.app_id))` (the scan needs the registration, §1); systems:
  `apply_volume.run_if(resource_changed::<GameSettings>)` (`GlobalVolume.volume = Volume::Linear(v)`) in `Update`;
  `save_on_exit` in `Last` `.after(bevy::window::ExitSystems)`: any `AppExit` read → `commands.queue(SaveSettingsSync::IfChanged)`.
- `pub fn step_value(value, steps: i32, min, max, step) -> f32` = clamp(round(value/step + steps)·step) (keeps
  `0.7` from becoming `0.70000005` in the toml); unit test table: up, down to min clamp, up to max clamp, off-grid
  start `0.33 + 1 step → 0.5` (one assert per row).
- `src/main.rs`: `--settings-id <id>` via the existing `flag_value`, default `SETTINGS_APP_ID`; add
  `GameSettingsPlugin` before the presentation plugins.

### Step 11 — settings consumers (surgical)
- `src/camera/mod.rs` `apply_mouse_look`: `+ settings: Res<GameSettings>`; `sensitivity *= settings.mouse_sensitivity`;
  pitch delta sign `if settings.invert_y { -1.0 } else { 1.0 }`. (No gate registers `CameraPlugin` — grep done.)
- `src/juice/shake.rs` `shake_camera`: `+ settings`; `shake.rotation = Quat::IDENTITY.slerp(shake_rotation(..),
  scale)` with `scale = if settings.reduce_shake { cfg.reduced_scale } else { 1.0 }`.
- `src/vfx/mod.rs` `spawn_flashes`: `+ settings`; when `no_flashes` drain the reader and return (tracers stay).
- No gate harness registers `JuicePlugin`/`VfxPlugin`/`ShotAudioPlugin` (grep of `src/**/*_gate.rs`): no harness change.

### Step 12 — `src/menu/widgets.rs` (new): shared shell and widgets; `hud/wasted.rs` reuses it
- Move `title_screen` from `hud/wasted.rs:35-67` here as `pub(crate) fn title_screen(commands: &mut Commands, ui,
  fonts, name, text, color, backdrop: Color, exit: impl States) -> Entity`; node gains `flex_direction: Column` and
  `row_gap: px(ui.menu.item_gap)` (one child → Wasted/Busted look unchanged). `hud/wasted.rs` calls it with
  `rgba(ui.wasted_backdrop)`.
- `button(label, MenuAction) -> impl Bundle` (classic `Button` + `Node` + `BackgroundColor` + child `Text`),
  `#[derive(Component, Clone, Copy)] enum MenuAction { NewGame, Resume, NewCity, OpenSettings, CloseSettings, Quit,
  Step(SettingKey, i32), Toggle(SettingKey) }`, `seed_field() -> impl Bundle` (`SeedField` marker, `EditableText {
  max_characters: Some(19), .. }` — 19 digits always fit `u64` —, `EditableTextFilter::new(|c| c.is_ascii_digit())`,
  `AutoFocus`, `TextFont` from `UiFonts.regular`, background `field_color`).
- `pub fn seed_from_field(text: &str, fallback: impl FnOnce() -> u64) -> u64` (empty → fallback); unit test rows:
  `""`, `"0"`, `"42"`, `"9999999999999999999"`.
- `pub fn clock_seed() -> u64` (nanos since epoch, `map_or(0, ..)`); `main.rs::parse_seed` becomes `cli_seed() ->
  Result<Option<u64>, String>` and uses `clock_seed` for the fallback.

### Step 13 — `src/menu/mod.rs` + `src/menu/screens.rs` (new): main menu, pause, settings screen
- `#[derive(SubStates, Default, ..)] #[source(GameState = GameState::Paused)] pub enum PauseMenu { #[default] Main, Settings }`,
  `add_sub_state::<PauseMenu>()`.
- `OnEnter(GameState::MainMenu)` → `title_screen(.., ui.menu.title, .., menu_backdrop, GameState::MainMenu)` +
  children: `button(new_game, NewGame)`, row `seed_label` + `seed_field`, `seed_hint`, `button(quit, Quit)`.
- `OnEnter(PauseMenu::Main)` → shell `paused` over `backdrop`, `current_seed` with `CitySeed`, buttons `resume`,
  `new_city`, `settings`, `quit`, seed row + hint (field focused → QA `type_text` lands there).
- `OnEnter(PauseMenu::Settings)` → shell `settings`: 5 rows (label, value text `SettingValue(SettingKey)`, `−`/`+`
  buttons or a toggle), `back`. `update_setting_labels.run_if(resource_changed::<GameSettings>)` rewrites values
  (`"×{:.2}"`, `"{}%"`, `on`/`off`).
- `Update` systems (all `run_if` state, set-level where possible):
  - `escape`: `ButtonInput<KeyCode>::just_pressed(Escape)` → Playing → `NextState(Paused)`; `PauseMenu::Main` →
    `NextState(GameState::Playing)`; `PauseMenu::Settings` → `NextState(PauseMenu::Main)`. Ignored in
    Loading/MainMenu/Wasted/Busted (no pause stacking with slow-mo).
  - `submit_seed` (MainMenu or PauseMenu::Main): `just_pressed(Enter | NumpadEnter)` → `start_city(seed_from_field(field.value().to_string(), clock_seed))`.
  - `press_buttons`: `Query<(&Interaction, &MenuAction), Changed<Interaction>>`, on `Pressed`: NewGame →
    `start_city(clock_seed())`; NewCity → `start_city(seed_from_field(..))`; Resume → Playing; Open/CloseSettings →
    `PauseMenu`; Quit → `AppExit::Success`; Step/Toggle → mutate `GameSettings` via `step_value` then
    `commands.queue(SaveSettingsDeferred::default())`. Hover colour via `Interaction::Hovered`.
  - `start_city(seed)`: `city_seed.0 = seed; next.set(GameState::Loading)` (from MainMenu or Paused only).
- `OnExit(PauseMenu::Settings)` → `commands.queue(SaveSettings::IfChanged)` (async; the deferred timer is frozen while
  paused).
- File sizes: `menu/mod.rs` < 150, `screens.rs` < 350, `widgets.rs` < 200.

### Step 14 — `src/input/mod.rs`: pause-aware input and cursor
- Remove `cursor_toggle` and the `Startup` `capture_cursor`; add `OnEnter(GameState::Playing)` → capture (sets
  `CursorCaptured(true)`, `Locked`, hidden), `OnEnter(Paused)` and `OnEnter(MainMenu)` → release. (The Startup capture
  would re-grab the cursor after `OnEnter(MainMenu)`, which runs before `Startup`.) Drop `.before(cursor_toggle)`
  from `write_action_intent`; its `wait_release` latch stays (a click on "Продолжить" must not fire on resume).
- `OnEnter(Paused)` → insert `ContextActivity::<OnFoot>::INACTIVE` on the input entity, `OnExit(Paused)` →
  `ACTIVE` (typed seed digits 1-4 must not select weapons; `ContextActivity` doc: inactive = all actions to None).
- Behaviour change to record for the owner: Esc now pauses (T1 "Esc отпускает курсор" becomes "Esc — пауза с
  курсором").

### Step 15 — `main.rs`: main menu vs `--seed`
- `let cli = cli_seed()?; let seed = cli.unwrap_or_else(clock_seed); compose_sim(.., City { seed })`;
  `if cli.is_none() { app.insert_state(GameState::MainMenu); }` right after `compose_sim` (overwrites the initial
  state and its transition event, §1). Every existing QA/showcase script passes `--seed` → unchanged boot.
- Add `GameSettingsPlugin { app_id }` and `minimap::MinimapPlugin` to the plugin tuple.

### Step 16 — `CityScoped` on client per-city spawns + cancel in-flight city work
- `src/hud/mod.rs` `spawn_hud`, `hud/weapon.rs` `spawn_weapon_hud` (Ammo, Crosshair dot, 4 arms, Hit marker),
  `hud/stars.rs` `spawn_stars`: add `CityScoped` to each root bundle (they re-spawn on every `Loading → Playing`).
- `src/visuals/city.rs`: `#[require(CityScoped)]` on `CityChunk`; `src/visuals/props.rs`: on `CityProp`; add
  `NEW_CITY` system removing `CityMeshTask` and `PendingCitySpawn` (a pause right after load must not spawn city A
  chunks into city B).
- `src/camera/mod.rs`: `add_systems(NEW_CITY, reset_pivot)` (the pivot snaps to the new spawn).
- Witness bars, damage numbers, VFX are self-expiring and keyed to live entities (checked `hud/witness.rs:46-66`,
  `vfx/mod.rs:124-138`) — no tag.

### Step 17 — `src/minimap/` (new domain): `mod.rs`, `material.rs`, `markers.rs`, `config.rs`; `assets/shaders/minimap.wgsl`
- `MinimapPlugin`: `UiMaterialPlugin::<MinimapMaterial>::default()`, `register_type::<MinimapMarker>()`.
- `OnTransition { Loading → Playing }`: `spawn_minimap` — root `Minimap` + `CityScoped`, `Node { position_type:
  Absolute, left: hud.margin, bottom: hud.margin, width/height: size }`, `Visibility::Hidden` until the raster is
  ready; two landmark children (`MinimapMarker { target: None, kind: Hospital/Station }` with glyph text) anchored at
  `HospitalSpawn.point` / `PoliceStationSpawn.point`. `start_raster` — `AsyncComputeTaskPool` task over a clone of
  `City.0` and a `RasterStyle` built from `MinimapConfig` + `GangConfig` tints (alpha = `territory_alpha`), stored as
  `MinimapRasterTask(Task<Raster>)`.
- `Update` `poll_raster.run_if(resource_exists::<MinimapRasterTask>)`: `block_on(poll_once)` (no blocking wait —
  domain rule); on ready: `Image::new(Extent3d, D2, rgba, Rgba8UnormSrgb, RenderAssetUsages::RENDER_WORLD)` with
  `ImageSampler::linear()`, insert `MaterialNode(materials.add(MinimapMaterial { .. }))` on the root, show it, remove
  the task. `NEW_CITY` → remove `MinimapRasterTask` (the root is `CityScoped`).
- `material.rs`: `MinimapMaterial { #[uniform(0)] data: MinimapUniform, #[texture(1)] #[sampler(2)] map }`;
  `MinimapUniform` (ShaderType): `center: Vec2` (player x, z), `heading: Vec2` (sin, cos of camera yaw),
  `raster_origin: Vec2`, `raster_size: Vec2` (m), `view_radius`, `arrow_heading`, `rim_px`, `arrow_px`,
  `search: Vec4` (x, z, radius, on 0/1), `cone: Vec4` (range, cos(half angle), count, search_ring_px), colours
  `outside, rim, search_fill, search_ring, cone_color, arrow_color: Vec4` (linear, converted from config sRGB),
  `cops: [Vec4; MAX_CONES]` (x, z, fwd x, fwd z). `const MAX_CONES: usize = 16;` — a law (shader array length,
  ≥ escalation max 12 units); nearest 16 if ever more.
- `PostUpdate` `update_minimap.after(follow_player).before(UiSystems::Layout)`: centre = player `Transform`
  (interpolated) xz, yaw = `OrbitCamera.yaw`, arrow = `heading_on_map(player facing yaw from
  `rotation.to_euler(YXZ).0`, yaw)`, circle = `WantedLevel::search_circle(&WantedConfig.stars)`, cones = live
  `PoliceUnit`s (`Without<Dead>`) within `view_radius + cop_view_distance`, `cone = (cop_view_distance,
  cos(cop_view_cone_deg/2))`; write through `materials.get_mut`.
- `assets/shaders/minimap.wgsl` (inverse of `project`, documented in its header): `p = (in.uv − 0.5) · 2`; outside
  `length(p) > 1` → alpha 0; `m = vec2(p.x, −p.y) · view_radius`; `w = center + vec2(c, −s)·m.x + vec2(−s, −c)·m.y`
  with `(s, c) = heading`; `uv = (w − raster_origin) / raster_size`; colour = `textureSampleLevel(map, samp, uv, 0.0)`
  (**`SampleLevel`, not `Sample`**: WGSL uniformity analysis rejects implicit-derivative sampling in non-uniform
  control flow) or `outside` when `uv ∉ [0,1]²`; blend search fill/ring (distance of `w` to `search.xy`, ring width
  `search_ring_px` converted with `view_radius · 2 / in.size.x`), then cones (`d < range && dot(v/d, fwd) >=
  cos_half`), then the arrow triangle (apex along `(−sin h, cos h)` in `p` space, `arrow_px`), then the rim ring.
- `markers.rs`: `#[derive(Component, Reflect)] #[reflect(Component)] pub struct MinimapMarker { pub target:
  Option<Entity>, pub kind: MarkerKind }`, `MarkerKind { Police, Gang(u8), Pickup, Hospital, Station }` (Reflect, so
  QA reads markers over BRP). `sync_markers` (Update, like `sync_witness_bars`): despawn markers whose target is gone,
  `Dead`, or an unavailable pickup; spawn a round dot child (`Node { border_radius: BorderRadius::MAX, .. }`) for new
  `PoliceUnit` (colour `police_tint`), `GangMember` (`GangConfig.gangs[g].tint`), available `Pickup`/`WeaponPickup`/
  `BatPickup` (`pickup_color`). `place_markers` (same `PostUpdate` slot as `update_minimap`): `px =
  map_px(target.xz, centre, yaw, size / (2·view_radius))`; `left = size/2 + px.x − dot/2`, `top = size/2 + px.y −
  dot/2`; NPC/pickup markers hidden beyond `view_radius`, landmark glyphs clamped to the rim.

### Step 18 — client presentation gate `src/visuals/city_gate.rs` (extend)
- **P1 `new_city_replaces_city_visuals`**: `city_visuals_app(1)` → record chunk and prop entity sets; pause;
  `CitySeed(2)`, NextState Loading; update until Playing and the city-visual resources are gone (reuse the `done`
  predicate); assert chunk count == `n²` (same worked example as `city_meshes_are_merged_per_chunk`), chunk and prop
  sets disjoint from city 1's, `Player == 1`.
- **P2 `new_city_cancels_pending_city_spawn`**: build seed 1 but stop updating as soon as `PendingCitySpawn` exists
  (the time-sliced spawn is mid-way; `GATE BROKEN` if it finished in one frame), then pause + new city 2 → at the end
  chunk count == exactly `n²` (without the `NEW_CITY` removal the leftover city-1 chunks make it larger).
- Flips: remove `#[require(CityScoped)]` from `CityChunk` → P1 RED; remove the task removal → P2 RED. Presentation
  gate: run the touched `city_gate` tests **3×** and report all three (gates domain, TASK-022).

### Step 19 — `tools/qa/brp.py`
- `QA_SETTINGS_ID = "com.github.pockerhead.maw-make-gta.qa"`; `start()` launches `[exe, "--settings-id",
  QA_SETTINGS_ID, *self.args]` (QA never touches the owner's settings; `tools/showcase/record.py` has its own
  launcher and is left alone).
- `def type_text(self, text): return self.call("brp_extras/type_text", {"text": text})`.
- `test_brp.py` untouched (it mocks `call`).

### Step 20 — `tools/qa/scenarios/t12.py` (new, runtime QA per AC)
Imports helpers from t5/t10/t11 like earlier scenarios (`game_state`, `resource_value`, `screenshot`,
`wait_chunks`, `set_heat`, `poll`). `Game(features=("dev",), args=("--seed", "1"), release=True)`.
1. Playing, `CityLayoutHash == golden[1]`, chunks settle, minimap marker entities for Hospital + Station exist.
2. `send_keys(["Escape"], 100)` → poll `GameState == Paused`; `AiClock.tick` read twice 1 s apart → equal
   (fixed loop frozen); screenshot `pause.png` (seed "1" visible).
3. `type_text("42")`, wait ≥ 0.2 s, `send_keys(["Enter"], 100)` → poll Loading then Playing (≤ 180 s), `CitySeed ==
   42`, `CityLayoutHash == golden[42]`, exactly one `Player`, `wait_chunks` == city-1 count (100, no leftovers),
   `AiClock.tick` advancing.
4. Minimap under wanted: `set_heat(game, 200)` (2 stars); poll until ≥ 1 `PoliceUnit` within `view_radius` (read from
   `strings.ron`) of the player; assert the set of `MinimapMarker{kind: Police}` targets equals the live cops within
   `view_radius − 5 m` (rim tolerance; retry 3 samples); screenshot `minimap_wanted.png` (circle, cones, blue dots).
5. `frame_report()` into the summary (FPS only via `frame_report`, TASK-010 lesson); `log_errors` empty (add
   "shader"/"wgsl" to the error words for this scenario); `summary.json`.
Evidence classes: hard pass/fail = states, hashes, counts, marker sets, tick freeze; screenshots = owner evidence.
Owner checklist for QA_REPORT.md (AC): orients by the minimap (rotation, arrow, circle, cones readable); changes seed
from the pause menu and from the main menu (start without `--seed`); settings (sensitivity, invert, volume, shake,
flashes) change the game and survive a restart.

---

## 4. Risk areas

- **Teardown completeness** (silent). A future spawn site without `CityScoped` leaks into the next city. G3 A1 is a
  generic detector (any surviving `Transform` entity outside the pre-city baseline), so it catches new sites as long
  as the gate's play phase exercises them (dropped guns are not exercised — covered by `WeaponPickup`'s require).
- **Mass despawn frame**: ~2.5k building + ~200 block colliders, 100 chunks, props and NPCs despawn in one frame;
  under the loading screen, one-off. Measure in QA (`game.log` frame time around the transition) if it hitches.
- **Messages during pause**: fixed loop frozen ⇒ message buffers do not rotate; NEW_CITY clears the gameplay ones.
  A BRP `write_message` during pause is read after resume — intended.
- **`insert_state(MainMenu)` after `compose_sim`**: relies on `bevy_state` overwrite semantics (`app.rs:139-151`); G2
  mirrors it.
- **Settings save while paused**: `SaveSettingsDeferred` ticks on virtual time (frozen in pause) — mitigated by
  save-on-settings-close and save-on-exit. A hard kill while paused after a change loses it only if the settings
  screen was not closed.
- **Settings file location**: `%LOCALAPPDATA%\com.github.pockerhead.maw-make-gta\settings.toml`; QA uses `.qa`. A
  hand-edited out-of-range value (e.g. sensitivity −1) is used as is (no clamp on load) — owner-only risk.
- **Shader correctness** (owner-visible, not headless): the WGSL inverse projection must match `project`; the markers
  use `map_px` (gated) and sit over the shader map, so a mismatch shows as dots off the streets on the first frame —
  QA screenshot + owner run. Uniformity: use `textureSampleLevel`.
- **Esc semantics change** (T1 behaviour) and Esc ignored during Wasted/Busted — record in QA_REPORT owner notes.
- **Main-menu Enter vs buttons**: classic `Button` activates on press; the `wait_release` latch prevents a shot on
  resume. `EditableText` keeps focus after a click elsewhere only if clicked back (Enter submits the field's value
  regardless of focus since `submit_seed` reads the field entity directly).
- **Raster memory**: 1400² RGBA8 ≈ 7.8 MB CPU + GPU once per city; freed with the root. Raster time ~0.1-0.5 s in dev
  on the async pool; the map is hidden until ready.
- **Probe artefacts in the shared target**: see header (phantom-red).
- **`reflect_auto_register`** already registers `GameSettings`; the explicit `register_type` keeps the order
  guarantee for `SettingsPlugin::build` regardless of that feature.

## 5. Open questions

None blocking; defaults chosen (orchestrator may override):
1. **"Новый город" UX.** Chosen: the pause menu has the seed field (empty = random) and "Новый город" starts it
   directly; alternatives: (a) "Новый город" always random, seed only in the main menu (QA `type_text` then needs a
   trip to the main menu); (b) go back to the main menu (two clicks).
2. **Accessibility toggles in T12 vs T13.** Chosen: all five GDD §7 settings in T12, wired to the existing shake and
   muzzle flash (T13's "переключатели доступности" then only extends to its new effects). Alternative: T12 ships
   sensitivity/volume/invert only.
3. **Round vs square minimap.** Chosen: round (shader mask), landmarks clamp to the rim, NPC dots hide outside.

children: 0 launched / 0 reported.
