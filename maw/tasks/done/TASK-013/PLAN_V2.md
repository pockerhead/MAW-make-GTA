# PLAN_V2 — TASK-013 (GDD T12): minimap, menus, settings

Reviewer: plan-reviewer-1. Base: `PLAN.md` (planner). Everything below was re-checked against the tree on
branch `feature/t12-minimap-menu` (HEAD `f3466a8`) and the pinned sources in
`~/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/` (bevy* 0.19.1, bevy-settings 0.19.1, bevy_brp_extras 0.22.6,
rand_chacha 0.10.0).

Cost of error: **mixed** (unchanged from PLAN.md). Silent class: "Новый город" leaving state behind, a pause racing a
death/arrest, a mirrored raster or a projection sign error. These get the full evidence layer. Owner class: how the
minimap reads, menu look, settings feel, persistence between runs. These get the mechanism, one honest gate and the owner run.

---

## 0. Disconfirmation (done before the review)

**Counter-example tested:** the whole projection AC (and the shader inverse) assumes `OrbitCamera.yaw` means "camera
forward = `R_y(yaw)·(−Z)` = `(−sin yaw, −cos yaw)` in (x, z)". If the camera used any other sign, M4 would be green
while the on-screen map is mirrored.
**Where I looked:** `src/camera/mod.rs:117-161`. `follow_player` builds `rotation = Quat::from_euler(YXZ, yaw, pitch, 0)`
and sets `aim.direction = rotation * Vec3::NEG_Z`, `right = rotation * Vec3::X`. The xz forward is `(−sin yaw, −cos yaw)`
and right is `(cos yaw, −sin yaw)`. **It held.** The plan's `project` and the six worked rows are correct (I recomputed all six).
The same check turned up a real sign error elsewhere: the arrow in the shader (R2 below).

Second counter-example: "pause is entered only from `Playing`" is enforced by the client `escape` system reading
`State<GameState>`. That **did not hold** under a same-frame race (R1).

---

## 1. Review notes (issues in PLAN.md, with evidence)

**R1 (silent, blocking): Esc can overwrite a pending `Wasted`/`Busted` transition and soft-lock the player.**
`detect_player_death` (`flow/wasted.rs:62-74`) inserts `Dead` and calls `next.set(GameState::Wasted)` inside
`FixedUpdate`. The main schedule order is `StateTransition → RunFixedMainLoop → Update`, so the planned `escape` system
in `Update` still sees `State == Playing` in that same frame. `NextState::set` is a plain overwrite
(`bevy_state-0.19.1/src/state/resources.rs:198-200`, `*self = Self::Pending(state)`), so `Pending(Wasted)` becomes
`Pending(Paused)`. After resume the player stays `Dead` in `Playing`, and `detect_player_death` (filter `Without<Dead>`)
never fires again. Busted (arrest in fixed) has the same race. Fix: request a pause only when
`NextState<GameState>` is `Unchanged`. Put that rule in `gta_sim::flow` as a pure function with a table test (step 1, G0).

**R2 (owner-visible, wrong spec): the arrow direction in the shader is given in the wrong space.** The plan says
"apex along `(−sin h, cos h)` in `p` space". But `p = (uv − 0.5)·2` has y **down** (UI uv origin is top-left:
`bevy_ui_render-0.19.1/src/ui_material_pipeline.rs:500-517`, `uvs = rect.min.. / rect.max`). `(−sin h, cos h)` is the
direction in `m` space (y = ahead/up). This is exactly what M5 proves for `project`. In `p` space the apex is
`(−sin h, −cos h)`. As written, the arrow points backwards when the player faces the camera direction.

**R3 (compile/runtime): shader binding facts the plan leaves out.** UiMaterial bindings live in `@group(1)`
(`ui_material_pipeline.rs:215-216`, `SetUiMaterialBindGroup<M, 1>`). `UiMaterial: AsBindGroup + Asset + Clone`
(`ui_material.rs:102`), so `MinimapMaterial` must derive `Clone`. The Bevy example uses
`#[derive(AsBindGroup, Asset, TypePath, Debug, Clone)]` (`bevy-0.19.1/examples/ui/ui_material.rs:63`).

**R4 (gate honesty): M3 cases (a) and (b) can be wrong for reasons that have nothing to do with the code.**
(b) The hospital's block may be in a gang district. The territory layer blends over the whole `curb` polygon, so the
texel is `blend(building, territory)`, not `building`. (a) Superblocks remove road edges (GDD §2.2 step 1), so
`roads.nodes[roads.center]` is not guaranteed to lie outside every curb polygon. Each probe point needs a precondition
computed in the test (`contains_convex` against all curbs / gang blocks), with `GATE BROKEN` on failure, and must pick
a point that passes it.

**R5 (gate honesty): G3 as written is slow, flaky and can test the wrong transition.** It runs natural spawning for
400 + up to 1500 ticks with the player standing on a gang post at 2 stars. Gang fire or an arrest can move the app to
`Wasted`/`Busted`. The test then sets `NextState(Paused)` by hand, which skips the "only from Playing" rule
and runs a `Busted → Paused` path the game never takes. Fix: build the population with the **production bundles**
through existing helpers (`spawn_civilian`, `common::spawn_member`, `police_support::spawn_unit`, plus one
`combat::dropped_gun` spawn). They all attach `Character`/`WeaponPickup` exactly as the spawners do. Keep cops out of
arrest range and assert `State == Playing` right before the pause (`GATE BROKEN` otherwise). Liveness of the natural
spawners stays in case B (`set_view` + 200 ticks → ≥ 1 civilian).

**R6 (rationale wrong, mechanism right): `ContextActivity::<OnFoot>::INACTIVE` during pause.** The plan justifies it
with "typed digits 1-4 must not select weapons". `write_action_intent` already returns early while
`!CursorCaptured` (`src/input/mod.rs:185-191`), so slots cannot fire. The real leak is `write_move_intent`, which has
no capture check (`:142-163`): Space during pause latches `jump_requested = true`, which fires on resume, and WASD
writes `MoveIntent.axis`. Keep the mechanism and fix the reason, so nobody later drops it as redundant.

**R7 (silent): a duplicated HUD or minimap after "Новый город" is invisible.** Two identical overlays stacked
pixel-for-pixel look like one. Step 16 tags the roots with `CityScoped`, but nothing gates it. Add a BRP count in the
t12 QA (exactly one `Minimap`, one `Name == "Hud"` root after the new city). Cheap, and it catches a missing tag.

**R8 (engine fact to record): messages do not rotate while `Time<Virtual>` is paused.** `TimePlugin` sets
`MessageRegistry.should_update = Waiting` and signals only from `FixedPostUpdate`
(`bevy_time-0.19.1/src/lib.rs:95-98`). Open upstream issue: [bevy #14152](https://github.com/bevyengine/bevy/issues/14152).
Readers keep cursors, so nothing is read twice. But every `Messages<T>` (mouse motion, keyboard) grows for the whole
pause, and a gameplay message written from `Update`/BRP during pause is read on the first tick after resume.
`NEW_CITY` clears the gameplay ones. Do not "fix" this with `ShouldUpdateMessages::Always`: it would drop sim
messages written in the last tick before the pause. Document it only.

**R9 (owner data leak): `tools/showcase/record.py` launches the game without `--settings-id`**
(`record.py:99`). Owner settings (invert Y, sensitivity) would change the mouse-driven showcase clips. Pass the QA id
there too (one line). Surgical and traceable to this task.

**R10 (robustness): hand-edited settings are used as is.** A `NaN` or negative sensitivity or volume in
`settings.toml` breaks the camera or audio with no error. Add a `sanitize` step after load that clamps into the menu
ranges from `strings.ron` (one fn, unit-tested). Cost is ~10 lines.

**R11 (architecture, minor): `project`/`map_px`/`heading_on_map` do not belong in `citygen`.** GDD §12 defines
`citygen` as "чистая генерация города" and §2.2 step 8 puts only the **raster** there. The projection is the camera-yaw
convention (GDD §3.2, owned by the client `camera/`) and has one consumer, `src/minimap/`. Move the three functions
and M4/M5 to `src/minimap/projection.rs`. They stay headless and pure in `cargo test -p gta_like --bin gta_like`. The
raster, M1-M3 and the golden file stay in `citygen`.

**R12 (claim too strong): "menu seed N rolls the same as `--seed N`".** This is true for the four RNG streams only.
`AiClock`, `SlotCursor`, `AttackSerial` keep counting (deliberately not reset), so the NPC trajectories of city N
after a "Новый город" are not identical to a fresh `--seed N`. Nothing in the AC needs full equivalence. State the
narrower claim and do not build a gate on the wider one.

**R13 (P2 precondition): pausing does not stop `apply_city_spawn`** (`Update`, no state gate,
`src/visuals/city.rs:32-38`; budget 10 chunks/frame over 100 chunks, `render.ron:8`). The two updates needed to reach
`Paused → Loading` spawn up to 20 more chunks. P2 must assert `PendingCitySpawn` still exists in the frame before
`NEW_CITY`. Otherwise `GATE BROKEN`, because the gate would pass without testing the removal.

**Minor / citation fixes:**
- `src/camera/mod.rs` has 189 lines. `apply_mouse_look` is at `:76-93`, not `:302-319`. `reset_pivot` is at `:183-185`,
  not `:408-411`. `shake_camera` is at `src/juice/shake.rs:29-39`, not `:83-93`.
- `WantedLevel.last_known` is `Option<Vec3>`. `search_circle` returns `None` when it is `None`.
- `Vec3::xz()` needs `Vec3Swizzles` in scope (plain glam in tests).
- `rng::fnv1a64` is `pub(crate)` in `citygen/src/rng.rs:23`. Usable from `citygen::minimap`, fine.

**Verified as correct in PLAN.md (no change):** `insert_state` overwrites the initial state and its transition event
(`bevy_state-0.19.1/src/app.rs:122-152`). Transition order Exit → Transition → Enter
(`state/transitions.rs:225-228`), so `OnExit(Paused)` unpauses before `NEW_CITY`. `SaveSettingsDeferred` ticks on
`Res<Time>` in `PostUpdate` and freezes during pause (`bevy-settings-0.19.1/src/lib.rs:123,585-594`).
`SaveSettingsSync`/`SaveSettings` are commands (`:205-235`). `ExitSystems` in `Last` (`bevy_window-0.19.1/src/lib.rs:139-151`).
BRP shutdown writes `AppExit::Success` (`bevy_brp_extras-0.22.6/src/shutdown.rs:46-56`). `ChaCha8Rng: PartialEq`
(`rand_chacha-0.10.0/src/chacha.rs:210`). The four `XRng::seeded` exist with streams 0/1/2/3. `EditableText`,
`EditableTextFilter::new`, `EditableText::value()` (`bevy_text-0.19.1/src/editing.rs:96,284-291`). Classic `Button`
with required `Interaction` (`bevy_ui-0.19.1/src/widget/button.rs:8`). `UiSystems::Layout` (`bevy_ui-0.19.1/src/lib.rs:95-113`).
`Image::new(Extent3d, TextureDimension, Vec<u8>, TextureFormat, RenderAssetUsages)` (`bevy_image-0.19.1/src/image.rs:1102`).
`ImageSampler::linear()` (`:687`). The resource inventory of PLAN §1 matches every `derive(Resource)` in `gta_sim`.
Every consumer of `SidewalkGraph`/`GangTerritories`/`City`/`CityLandmarks` is already gated
(`NpcSystems.run_if(resource_exists::<SidewalkGraph>)`, `GangSystems.run_if(resource_exists::<GangTerritories>)`,
one-shots `run_if(resource_exists::<City>)`). The post-teardown `Loading` looks like the first `Loading`, so removing
these resources cannot panic a `Res<T>`. No existing QA scenario sends `Escape`. Every scenario passes `--seed`.
No `*_gate.rs` registers `CameraPlugin`/`JuicePlugin`/`VfxPlugin`/`ShotAudioPlugin`/`HudPlugin`/`MenuPlugin`. The
planner probe (`scratch/probe/probe_test.log`) compiled the settings/UiMaterial/EditableText shapes and passed.

Research: the pause via `Time<Virtual>::pause()` (FixedUpdate stops, `delta` = 0) is the documented Bevy pattern
([docs.rs `Virtual`](https://docs.rs/bevy/latest/bevy/time/struct.Virtual.html),
[virtual_time example](https://github.com/bevyengine/bevy/blob/main/examples/time/virtual_time.rs)). A static top-down
raster sampled with rotated UVs is the common minimap technique. GDD §7 already rejected render-to-texture. The Bevy
UI clipping limitation for rotated nodes is confirmed in source (`bevy_ui_render-0.19.1/src/lib.rs:1726-1745`) and
by [bevy #9381](https://github.com/bevyengine/bevy/issues/9381). So the shader approach stands.

---

## 2. Updated understanding (corrected where PLAN.md §1 was wrong; everything else in PLAN.md §1 is verified)

- `src/camera/mod.rs` (189 lines): `apply_mouse_look` `:76-93` (`camera.yaw -= delta.x * sens`, pitch clamp),
  `follow_player` `:95-163` (PostUpdate, `before(TransformSystems::Propagate)`, reads `Time` = virtual, so it freezes
  while paused), `publish_camera_view` `:166-180` (`run_if(any_with_component::<Player>)`), `reset_pivot` `:183-185`
  (registered on `OnExit(Wasted)`/`OnExit(Busted)`).
- `src/input/mod.rs`: `write_move_intent` has **no** capture check. `write_action_intent` blanks fire/aim and arms
  `wait_release` while `!captured`.
- `src/juice/shake.rs:29-39` `shake_camera` (real clock). `hit_stop.rs` runs on `Time<Real>` and never writes
  `Time<Virtual>`.
- `src/visuals/city.rs`: `start_city_mesh_build` on `OnTransition{Loading→Playing}`. `poll_city_mesh_build` and
  `apply_city_spawn` in `Update`, gated only by `resource_exists` (they run while paused). The budget of 10
  chunks/frame means 100 chunks take ≥ 10 frames.
- Bevy main schedule: `First → PreUpdate → StateTransition → RunFixedMainLoop → Update → PostUpdate → Last`. A
  `NextState` written in `FixedUpdate` is applied at the **next** frame's `StateTransition`, so `Update` of the same
  frame can overwrite it (R1).
- While `Time<Virtual>` is paused, no `Messages<T>` rotate (R8).

---

## 3. Revised approach

1. **Flow (sim `flow/`)**: add `GameState::MainMenu` and `GameState::Paused`. `OnEnter(Paused)` → `Time<Virtual>::pause()`,
   `OnExit(Paused)` → `unpause()`. `pub const NEW_CITY: OnTransition<GameState> = { exited: Paused, entered: Loading }`.
   **New:** a pure `pub fn pause_request(state: &GameState, next: &NextState<GameState>) -> Option<GameState>` that owns the
   Esc rule (Playing + `Unchanged` → Paused; Paused → Playing; everything else, or any pending transition → `None`). The
   client only calls it. The rule lives in the sim and is table-tested there.
2. **Teardown (sim `world/`)**: marker `CityScoped` (`#[require]`d by `Character`, `Pickup`, `WeaponPickup`,
   `BatPickup`, `CityGround`, `CityEdgeWall`, `CityBlock`, `CityBuilding`; client `CityChunk`, `CityProp`; client roots
   tagged explicitly). One `NEW_CITY` system `try_despawn`s them. Each domain resets its own resources in `NEW_CITY`. RNGs
   reseed from `CitySeed` on every `OnEnter(Loading)` (City mode). Only the RNG streams become equal to `--seed N`, not
   whole trajectories (R12).
3. **Minimap**: `citygen::minimap::rasterize` (pure, gated in `cargo test -p citygen`). Projection helpers in
   `src/minimap/projection.rs` (pure, gated in `cargo test -p gta_like --bin gta_like`) (R11). One `UiMaterial` shader over
   an unrotated round node. Dots and landmarks are ordinary UI nodes placed by `map_px`.
4. **Menus (`src/menu/`)**: main menu only without `--seed`. Pause menu with seed field and settings sub-state. Shared
   `title_screen` shell moved to `menu/widgets.rs`.
5. **Settings (`src/settings/`)**: `GameSettings` group, `SettingsPlugin::new(id)`, id overridable by `--settings-id`
   (QA and showcase use a `.qa` id). `SaveSettingsDeferred` after each change, `SaveSettings::IfChanged` when the settings
   screen closes, `SaveSettingsSync::IfChanged` on `AppExit`. **New:** `sanitize` clamps loaded values (R10).

Signals: no new `Message`/`Event` type. Transitions go through `NextState`. Buttons use `Changed<Interaction>` in `Update`.
Save-on-exit reads `AppExit` with its own `MessageReader` in `Last`.

---

## 4. Revised steps (complete list)

Order: sim flow + teardown (1-6) → citygen raster (7-8) → client (9-18) → QA (19-20).
Builds: one cargo command at a time, `-j 4`, in place (`CARGO_TARGET_DIR` = repo `target/`). The planner probe
built into the shared target. If an untouched module suddenly "does not exist", run `touch crates/*/src/lib.rs` and
rebuild (TASK-009 phantom red). Run `cargo test -p gta_sim -p citygen` after step 8. After step 18 run
`cargo test -p gta_like --bin gta_like` and `cargo clippy -- -D warnings`.

### Step 1 — `crates/gta_sim/src/flow/mod.rs`: states, pause, `NEW_CITY`, `pause_request`
- `GameState`: add `MainMenu` (doc: "waiting for a seed; no city exists") and `Paused` (doc: "`Time<Virtual>` is paused;
  entered only from `Playing` through `pause_request`"). Keep `#[default] Loading`.
- `pub const NEW_CITY: OnTransition<GameState> = OnTransition { exited: GameState::Paused, entered: GameState::Loading };`
  (`///` "every domain drops its city-lifetime state here").
- `pub fn pause_request(state: &GameState, next: &NextState<GameState>) -> Option<GameState>`:
  `let NextState::Unchanged = next else { return None };` then `match state { Playing => Some(Paused), Paused => Some(Playing), _ => None }`.
  `///` comment: why the pending check exists (a death or arrest set in this frame's fixed tick must win).
- `add_systems(OnEnter(Paused), pause_time)`, `add_systems(OnExit(Paused), resume_time)`.
- `add_systems(NEW_CITY, (wasted::drop_queued_damage, wasted::drop_queued_input))`. `drop_queued_input` is harmless
  with no player and clears nothing else.
- `NpcSystems` doc: add "`Paused` needs no entry: the fixed loop does not run while paused".
- Unit test `pause_request_table` in `flow/mod.rs`, **one assert per row**: (Playing, Unchanged) → Paused;
  (Playing, Pending(Wasted)) → None; (Playing, Pending(Busted)) → None; (Paused, Unchanged) → Playing; (Wasted, Unchanged) →
  None; (Busted, Unchanged) → None; (Loading, Unchanged) → None; (MainMenu, Unchanged) → None. Flip: drop the
  `Unchanged` guard → rows 2-3 RED. Record it.

### Step 2 — `crates/gta_sim/src/world/`: `CityScoped`, teardown
- `world/mod.rs`: `#[derive(Component, Reflect, Default)] #[reflect(Component)] pub struct CityScoped;`
  (doc "despawned on `flow::NEW_CITY`"), `register_type`, re-export.
- `world/city.rs`: `#[require(CityScoped)]` on `CityBuilding`, `CityGround`, `CityEdgeWall`, `CityBlock`.
- `WorldPlugin` City branch: `.add_systems(NEW_CITY, (despawn_city, drop_city))`. `despawn_city` →
  `commands.entity(e).try_despawn()` for every `With<CityScoped>`. `drop_city` →
  `remove_resource::<City/CityLayoutHash/CityLandmarks>()`. `CityGenTask` cannot exist here (pause only from Playing).

### Step 3 — per-domain resets in `NEW_CITY` (each plugin registers its own)
| Domain / file | State | Reset |
|---|---|---|
| `character/mod.rs:30-41` | `Character` | add `CityScoped` to `#[require(..)]` (player, dummies, civilians, gang, police, corpses) |
| `combat/pickups.rs` | `Pickup`, `WeaponPickup`, `BatPickup` | `#[require(CityScoped)]` each (covers `dropped_gun`) |
| `combat/mod.rs` | `Messages<ShotFired/BulletTrace/DamageDealt/MeleeHit/melee::Strike>` | `clear_combat_messages` |
| `navigation/mod.rs` | `SidewalkGraph` | `remove_resource` |
| `gang/mod.rs` | `GangTerritories` | `remove_resource` |
| `gang/mod.rs` | `GangHeat.left` | all 0.0 (length kept) |
| `gang/mod.rs` | `PlayerTerritory` | `default()` |
| `police/mod.rs` | `PoliceDispatcher`, `ArrestAttempt`, `PoliceAlert` | `default()` each |
| `population/mod.rs` | `PopulationPhase` | `InitialFill` |
| `population/mod.rs` | `CameraView` | `CameraView(None)` (`spawn_civilians` returns early on `None`, `population/mod.rs:452-454`) |
| `perception/mod.rs` | `StimulusLog` | `.0.clear()` |
| `wanted/mod.rs` | `WantedLevel`, `Crimes`, `Messages<PoliceCall>` | existing `reset_wanted` + `drop_queued_calls` |
Not reset (monotonic, per-tick or overwritten): `AiClock`, `SlotCursor`, `AttackSerial`, `RouteLoad`, `PopulationLoad`,
`PerceptionLoad`, `PlayerSpawn`/`HospitalSpawn`/`PoliceStationSpawn`, `WastedClock`/`BustedClock`. Consequence (R12):
city N after "Новый город" shares RNG streams with `--seed N`, not tick-exact behaviour.

### Step 4 — RNG reseed on every `OnEnter(Loading)`
`CombatPlugin`, `PopulationPlugin`, `GangPlugin`, `PolicePlugin`:
`add_systems(OnEnter(GameState::Loading), reseed_x.run_if(resource_exists::<CitySeed>))`, body `*rng = XRng::seeded(seed.0)`.
TestArea has no `CitySeed`, so it keeps seed 0. Update the comment at `lib.rs:129`.

### Step 5 — `wanted/`: one derivation of the search circle
- `pub fn search_row(rows: &[StarRow; STARS], stars: u8) -> &StarRow` (`rows[usize::from(stars.max(1)) - 1]`).
  `search_step` (`search.rs:79`) uses it (pure refactor, `wanted_search.rs` stays green).
- `impl WantedLevel { pub fn search_circle(&self, rows) -> Option<(Vec3, f32)> }`: `None` when `stars == 0` or
  `last_known` is `None`.
- Unit test in `wanted/mod.rs`: one row per star 1..5 with the shipped `wanted.ron` radius (TASK-011 lesson: one case
  per table row), plus `stars 0 → None`. Flip: `rows[0]` → rows 2..5 RED.

### Step 6 — headless gates `crates/gta_sim/tests/new_city.rs` (new)
Helpers in the file: `pause(app)` (writes the `pause_request` result, asserts it is `Some(Paused)`, updates until
`State == Paused`, bounded), `new_city(app, seed)` (`CitySeed(seed)`, `NextState(Loading)`, one `update()`),
`until_playing(app)` (bounded like `city_app`).
- **G1 `pause_freezes_the_fixed_clock`** (correctness): `headless_app()` settled, `set_intent` forward, pause. Record
  `Time<Fixed>::elapsed` and position. 10 updates: both unchanged, `Time<Virtual>::is_paused()`. `NextState(Playing)`,
  10 updates: elapsed grew by ≥ 8 ticks and the player moved ≥ 0.5 m. Flip: drop `pause_time` → RED.
- **G2 `main_menu_waits_for_a_seed`**: build the city app by hand (like `composed_app`, `City{seed: 7}`) with
  `app.insert_state(GameState::MainMenu)` after `compose_sim`, **before** `finish()`, mirroring `main.rs`. 30 updates:
  `MainMenu`, no `City`/`CityLayoutHash`, zero `Player`. Then `CitySeed(2)` + `NextState(Loading)`, one update, and
  one assert per RNG: `CombatRng.0 == CombatRng::seeded(2).0`, the same for `NpcRng`, `GangRng`, `PoliceRng`. Then
  `until_playing` → `CityLayoutHash == golden(2)`. Flip each reseed separately → its row RED (4 flips).
- **G3 `new_city_replaces_the_city`** (silent-defect gate, R5 rework):
  - Build `city_app(1)`. Baseline `B0` = set of entities with `Transform` recorded after the **first** `update()` of a
    hand-built app in `Loading`. Expected empty. Record it anyway, do not assume.
  - Fixtures with production bundles: 2 civilians (`spawn_civilian`), 1 gang member of gang 0 (`common::spawn_member`),
    1 police unit (`police_support::spawn_unit`) **≥ 40 m** from the player (outside arrest reach), 1 `dropped_gun` spawn.
    Every point checked against the city buildings (`GATE BROKEN` if inside, TASK-012 lesson). `raise_heat(app, 200)`.
    `set_view(chase_view(..))`, run 60 ticks.
  - Preconditions (`GATE BROKEN` otherwise): `State == Playing`, ≥ 1 of each fixture kind alive, `WantedLevel.heat == 200`.
    Snapshot `A` = all `Character`, pickup and city-collider entities.
  - Pause (through `pause_request`) → `new_city(app, 2)` → **case A (state `Loading`, right after the transition
    frame)**: A1 no `Transform` entity outside `B0`. A2..A14 one assert per step-3 table row (`City`/`CityLayoutHash`/
    `CityLandmarks`/`SidewalkGraph`/`GangTerritories` absent; `GangHeat.left` all 0; `PlayerTerritory(None)`; dispatcher/
    arrest/alert fields default; `PopulationPhase::InitialFill`; `CameraView(None)`; `StimulusLog` empty;
    `WantedLevel == default()`; `Crimes.incidents()` empty; each of the 7 `Messages<..>` `len() == 0`). A15
    `!Time<Virtual>::is_paused()`.
  - **case B (after `until_playing`)**: `CityLayoutHash == golden(2)`; exactly one `Player` at
    `PlayerSpawn + float_height` (±0.05 m after `settle`); `CityBuilding` count == `City.buildings.len()`; `CityGround == 1`;
    `CityEdgeWall == 4`; `CityBlock` == blocks with `curb.len() >= 3`; `Pickup == 2`; `Dummy == WeaponsConfig.range.dummies`;
    no entity of `A` alive; `set_view` + 200 ticks → `Time<Fixed>` advanced and ≥ 1 civilian spawned (natural spawner liveness).
  - Flips (record each): drop `CityScoped` from `Character` → A1 RED; delete `despawn_city` → A1 RED; delete each reset
    row → its assert RED; delete `resume_time` → A15 + B liveness RED; delete `drop_city` → A2 RED.
- Numbers are derived: counts from loaded configs, hashes from `golden_hashes.txt`, 200 heat = 2 stars by the shipped
  table `[40, 180, ...]` (`wanted/mod.rs` `stars_table`).

### Step 7 — `crates/citygen/src/minimap.rs` (new, `pub mod minimap`)
```rust
pub struct RasterStyle { pub px_per_m: f32, pub road: [u8; 4], pub sidewalk: [u8; 4], pub block: [u8; 4],
    pub park: [u8; 4], pub building: [u8; 4], pub territory: [[u8; 4]; 2] }   // sRGB RGBA8; territory alpha = [3]
pub struct Raster { pub width: u32, pub height: u32, pub origin: Vec2, pub px_per_m: f32, pub rgba: Vec<u8> }
pub fn rasterize(layout: &CityLayout, style: &RasterStyle) -> Raster
impl Raster { pub fn texel(&self, col: u32, row: u32) -> [u8; 4]; pub fn hash(&self) -> u64 }
```
- Contract (doc on `Raster`, shared with the shader): covers `[-g/2, g/2]²`, `g = layout.ground_size`;
  `origin = (-g/2, -g/2)`; texel `(col, row)` covers x ∈ `origin.x + [col, col+1)/ppm`, z ∈ `origin.y + [row, row+1)/ppm`;
  **row 0 = smallest z (north)**; `width = height = ceil(g·ppm)`; texel centres are sampled.
- Layers: fill `road`; each block with `curb.len() >= 3`: `curb` → `sidewalk`, then `inner` (if `len >= 3`) → `park`/`block`;
  each building's base oriented rectangle → `building`; each block with `district == gang_districts[g]`: blend
  `territory[g]` over `curb` (`out = (dst·(255−a) + src·a + 127) / 255` per RGB, alpha 255). Edge normals precomputed per polygon.
- `hash`: `rng::fnv1a64` over width, height, origin (mm), `px_per_m.to_bits()`, `rgba`.

### Step 8 — citygen gates `crates/citygen/tests/minimap.rs` + `tests/golden_minimap.txt`
Style literal in the test (px_per_m 1.0, six distinct colours, territory alphas 96/160), independent of `strings.ron`.
- **M1 `raster_hashes_match_golden`** (seeds 1, 2, 42; `#[ignore] bless_print_minimap_golden`; bless command in the
  file header like `golden_hashes.txt`). Drift guard.
- **M2 `raster_differs_by_seed`**: three hashes pairwise different.
- **M3 `raster_marks_known_places`** (correctness). Texel index computed **in the test** from the contract, never via a
  raster helper. Seed 1, one assert per case, each probe point with a test-side precondition (R4):
  (a) road: midpoint of the first `roads.edges` entry whose midpoint is outside every block's `curb`
  (`contains_convex`), `GATE BROKEN` if none; (b) building: the first building whose centre texel is not in a gang
  block and not inside another building's footprint (hospital preferred); (c) park: centroid of
  `blocks[landmarks.park].inner`, precondition not gang; (d) block: 1 m inside an `inner` vertex of a non-park,
  non-gang block that no building contains; (e) sidewalk: midpoint of `curb[0]`/`inner[0]` of a non-gang block;
  (f) territory: construction (e) in a `gang_districts[0]` block → the blend formula; (g) asymmetry: the row-mirrored
  texel of (b) is **not** `building` (choose a (b) whose mirror point is outside every building, `GATE BROKEN` otherwise).
  Flip: `row` ↔ `height−1−row` in `rasterize` → M3 and M1 RED.

### Step 9 — client configs
- `UiConfig.menu: MenuConfig` in `strings.ron` (as PLAN.md step 9: all texts, `{seed}` in `current_seed`, layout sizes,
  colours, `sensitivity: (min, max, step)` = (0.25, 3.0, 0.25), `volume_step` 0.1). Put the struct in `src/menu/config.rs`,
  or `src/menu/menu_config.rs` if `config.rs` would pass ~350 lines.
- `UiConfig.hud.minimap: MinimapConfig` (`src/minimap/config.rs`): `size` 220, `view_radius` 120, `raster_px_per_m` 1.0
  (validate `(0, 2]`: 1400 m × 2 = 2800 px, 31 MB RGBA; the PLAN's `(0, 4]` allowed a 125 MB texture for no benefit),
  colours (`road`, `sidewalk`, `block`, `park`, `building`, `outside`), `territory_alpha`, `rim_*`, `search_*`, `cone_color`,
  `arrow_*`, `dot_px`, `pickup_color`, `hospital`/`station` glyph+colour, `glyph_size`. Validate each field (positive,
  unit colours, alpha in [0, 1]).
- `juice.ron` `shake.reduced_scale: 0.3` + `ShakeConfig.reduced_scale` + validate `[0, 1]`. Update the `cfg()` literal in the shake tests.

### Step 10 — `src/settings/mod.rs` (new domain)
- `GameSettings { mouse_sensitivity: f32 = 1.0, volume: f32 = 1.0, invert_y, reduce_shake, no_flashes: bool = false }`,
  derives as the probe (`Resource, SettingsGroup, Reflect, Clone, Debug, PartialEq`, `#[reflect(Resource, SettingsGroup, Default)]`,
  `#[settings_group(group = "game")]`), manual `Default`.
- `pub const SETTINGS_APP_ID: &str = "com.github.pockerhead.maw-make-gta";` (identity, not tuning).
- `GameSettingsPlugin { app_id: String }`: `register_type::<GameSettings>()` **before** `add_plugins(SettingsPlugin::new(&app_id))`.
  Then `Startup` `sanitize_settings`: clamp sensitivity into `ui.menu.sensitivity` range and volume into `[0, 1]`, and
  replace non-finite values with the default (R10). Pure `fn sanitize(s, range) -> GameSettings` with a unit table:
  NaN, −1, 99, in-range (one assert each).
  `apply_volume.run_if(resource_changed::<GameSettings>)` (`GlobalVolume::new(Volume::Linear(v))`) in `Update`.
  `save_on_exit` in `Last.after(bevy::window::ExitSystems)`: any `AppExit` → `commands.queue(SaveSettingsSync::IfChanged)`.
- `pub fn step_value(value, steps, min, max, step) -> f32` with its unit table (up, clamp min, clamp max, off-grid
  `0.33 + 1 → 0.5`), as PLAN.md.
- `main.rs`: `--settings-id` via `flag_value`, default `SETTINGS_APP_ID`. Add the plugin before the presentation plugins.

### Step 11 — settings consumers (surgical)
- `camera/mod.rs:76-93` `apply_mouse_look`: `+ settings: Res<GameSettings>`; sensitivity × `mouse_sensitivity`; pitch
  delta sign by `invert_y`.
- `juice/shake.rs:29-39` `shake_camera`: `Quat::IDENTITY.slerp(shake_rotation(..), scale)`, where scale is
  `reduced_scale` if `reduce_shake`, else 1.
- `vfx/mod.rs` `spawn_flashes`: when `no_flashes` drain the reader and return.
- No gate harness registers these plugins (checked). No harness change.

### Step 12 — `src/menu/widgets.rs` (new): shell and widgets
As PLAN.md step 12: move `title_screen` from `hud/wasted.rs:37-67` (Wasted/Busted look unchanged). Add
`button(label, MenuAction)`, `MenuAction` enum, `seed_field()` (`SeedField`, `EditableText { max_characters: Some(19), .. }`,
digit filter, `AutoFocus`), `seed_from_field(text, fallback)` (table: `""`, `"0"`, `"42"`, `"9999999999999999999"`),
`clock_seed()`. `main.rs::parse_seed` → `cli_seed() -> Result<Option<u64>, String>`.

### Step 13 — `src/menu/mod.rs` + `src/menu/screens.rs`: main menu, pause, settings screen
As PLAN.md step 13, with these changes:
- `escape` (`Update`): on `just_pressed(Escape)`: in `PauseMenu::Settings` → `NextState(PauseMenu::Main)`. Otherwise
  `if let Some(target) = flow::pause_request(&state, &next) { next.set(target) }` (R1). Loading/MainMenu/Wasted/Busted
  are ignored by the rule, not by the client.
- `start_city(seed)` is called only from `MainMenu`/`Paused`, where no fixed-tick transition can be pending.
- Register `PauseMenu` with `register_type_state` so QA can read it over BRP.
- `OnExit(PauseMenu::Settings)` → `commands.queue(SaveSettings::IfChanged)`.

### Step 14 — `src/input/mod.rs`: pause-aware input and cursor
- Remove `cursor_toggle` and the `Startup` `capture_cursor`. Add capture on `OnEnter(GameState::Playing)` and release
  on `OnEnter(Paused)` and `OnEnter(MainMenu)`. Drop `.before(cursor_toggle)`. Keep the `wait_release` latch (a click on
  "Продолжить" must not fire).
- `OnEnter(Paused)` → `ContextActivity::<OnFoot>::INACTIVE`, `OnExit(Paused)` → `ACTIVE`. Why (R6): `write_move_intent`
  has no capture check, so Space during pause would latch `jump_requested` and jump on resume. Put this reason in a one-line comment.
- Owner note: Esc now pauses (T1 "Esc отпускает курсор" is gone).

### Step 15 — `main.rs`: main menu vs `--seed`
`let cli = cli_seed()?; compose_sim(.., City { seed: cli.unwrap_or_else(clock_seed) })`. If `cli.is_none()`, call
`app.insert_state(GameState::MainMenu)` right after `compose_sim` (overwrite semantics verified). Add
`GameSettingsPlugin { app_id }`, `minimap::MinimapPlugin`.

### Step 16 — `CityScoped` on client per-city spawns, cancel in-flight city work
- `CityScoped` on the roots from `hud/mod.rs::spawn_hud`, `hud/weapon.rs::spawn_weapon_hud` (all roots), `hud/stars.rs::spawn_stars`.
- `#[require(CityScoped)]` on `CityChunk` (`visuals/city.rs`) and `CityProp` (`visuals/props.rs`). `NEW_CITY` system
  removes `CityMeshTask` and `PendingCitySpawn` (dropping the `Task` cancels it).
- `camera/mod.rs`: `add_systems(NEW_CITY, reset_pivot)`.
- Witness bars, damage numbers, tracers and flashes expire by themselves or are keyed to live entities. No tag.

### Step 17 — `src/minimap/`: `mod.rs`, `projection.rs`, `material.rs`, `markers.rs`, `config.rs`; `assets/shaders/minimap.wgsl`
- **`projection.rs` (moved from citygen, R11)**: `project(point, center, yaw) -> Vec2` (x right, y ahead, metres):
  `d = point − center`, `(s, c) = yaw.sin_cos()`, `(d.x·c − d.y·s, −d.x·s − d.y·c)`. `map_px(point, center, yaw, px_per_m)`
  = `Vec2::new(m.x, −m.y)·px_per_m` (UI y down). `heading_on_map(facing_yaw, camera_yaw) = facing_yaw − camera_yaw`
  (CCW from map-up). Tests in the same file:
  - **M4 `camera_ahead_is_straight_up`** (the AC gate): yaw 0°, 90°, 180°. Forward computed independently as
    `(Quat::from_rotation_y(yaw) * Vec3::NEG_Z).xz()`, centre `(123, −45)`, point = centre + 10·forward. Assert
    `|x| < 1e-4`, `|y − 10| < 1e-4`, `map_px(.., 2.0) ≈ (0, −20)`. Plus right rows (`Quat·X`) → `(10, 0)`. 6 rows, one assert
    each. Worked values (recomputed): yaw 90° ahead point `c+(−10,0)` → `(0, 10)`; yaw 180° ahead `c+(0,10)` → `(0, 10)`;
    right rows `(10,0)`, `(0,−10)`, `(−10,0)` → `(10, 0)`. Flip: negate `s` → yaw-90° rows RED (0°/180° stay green, which is why
    all three yaws are there).
  - **M5 `arrow_points_along_facing`**: (camera 0, facing 90°) → `(−1, 0)` left; (90°, 90°) → `(0, 1)` up; (90°, 0) →
    `(1, 0)` right. Expected `(−sin h, cos h)` in **m space** equals `project(center + facing_dir, center, camera).normalize()`.
- `MinimapPlugin`: `UiMaterialPlugin::<MinimapMaterial>::default()`, `register_type::<MinimapMarker>()`, `register_type::<Minimap>()`
  (`Minimap` root marker is `Reflect` so QA can count it, R7).
- `OnTransition{Loading→Playing}`: `spawn_minimap` (root `Minimap` + `CityScoped`, absolute bottom-left at `hud.margin`,
  `Visibility::Hidden` until ready, two landmark children). `start_raster` starts an `AsyncComputeTaskPool` task over
  `City.0.clone()` + `RasterStyle` (from `MinimapConfig` + `GangConfig` tints) → `MinimapRasterTask`.
- `Update` `poll_raster.run_if(resource_exists::<MinimapRasterTask>)` with `block_on(poll_once)`. On ready:
  `Image::new(Extent3d{w,h,1}, D2, rgba, Rgba8UnormSrgb, RenderAssetUsages::RENDER_WORLD)`, sampler `ImageSampler::linear()`,
  insert `MaterialNode(materials.add(..))`, show, remove task. `NEW_CITY` → remove `MinimapRasterTask`.
- `material.rs` (R3): `#[derive(Asset, TypePath, AsBindGroup, Debug, Clone)] MinimapMaterial { #[uniform(0)] data: MinimapUniform,
  #[texture(1)] #[sampler(2)] map: Handle<Image> }`. `impl UiMaterial` with `fragment_shader() → "shaders/minimap.wgsl"`.
  `MinimapUniform` (`ShaderType`) as PLAN.md (`center`, `heading = (sin, cos)` of camera yaw, raster origin/size, view radius,
  arrow heading, px sizes, `search: Vec4`, `cone: Vec4`, linear colours, `cops: [Vec4; MAX_CONES]`).
  `const MAX_CONES: usize = 16` (shader array length, a law ≥ escalation max 12).
- `minimap.wgsl`: bindings at **`@group(1) @binding(0/1/2)`**. `p = (in.uv − 0.5)·2` (y down), outside `length(p) > 1`
  → alpha 0. `m = vec2(p.x, −p.y)·view_radius` (y ahead). `w = center + vec2(c, −s)·m.x + vec2(−s, −c)·m.y`.
  `uv = (w − raster_origin)/raster_size`. `textureSampleLevel(.., 0.0)`, or `outside` when uv ∉ [0,1]². Then search fill and
  ring (ring width px → m via `view_radius·2/in.size.x`), cones, arrow, rim. **Arrow in m space**: apex direction
  `(−sin h, cos h)` in `m` (y up). That equals `(−sin h, −cos h)` in `p` (R2). The header comment names the space.
- `PostUpdate` `update_minimap.after(camera::follow_player).before(UiSystems::Layout)`: centre = player `Transform` xz,
  yaw = `OrbitCamera.yaw`, arrow = `heading_on_map(player yaw from rotation.to_euler(YXZ).0, yaw)`,
  circle = `WantedLevel::search_circle(&WantedConfig.stars)`, cones = nearest ≤ 16 live `PoliceUnit`s within
  `view_radius + cop_view_distance`. Write through `materials.get_mut`.
- `markers.rs`: `MinimapMarker { target: Option<Entity>, kind: MarkerKind }` (Reflect), `sync_markers` (Update),
  `place_markers` (PostUpdate next to `update_minimap`): `px = map_px(.., size/(2·view_radius))`,
  `left = size/2 + px.x − dot/2`, `top = size/2 + px.y − dot/2`. NPC/pickup markers hide beyond `view_radius`,
  landmark glyphs clamp to the rim.
- Config ownership note: the minimap reads `GangConfig` tints, `CharacterVisualConfig.police_tint` and
  `WantedConfig` (cone and star rows). This follows the `hud/mod.rs` precedent (reads `HealthConfig`). GDD §12 "one
  owner per value" is respected because nothing is copied into a second file.

### Step 18 — client presentation gate `src/visuals/city_gate.rs` (extend)
- **P1 `new_city_replaces_city_visuals`**: `city_visuals_app(1)`, record chunk and prop sets. Pause (via `pause_request`),
  `CitySeed(2)`, `NextState(Loading)`. Update until `done`. Assert chunks == `n²` (same worked example: 1200/128 → 10 → 100),
  sets disjoint from city 1 (entity generations differ), `Player == 1`.
- **P2 `new_city_cancels_pending_city_spawn`**: hand-built loop that stops at the first update where
  `PendingCitySpawn` exists. Pause, then new city 2. **Precondition (R13): assert `PendingCitySpawn` still exists in the
  update before the `Paused → Loading` transition** (`GATE BROKEN` otherwise). At the end chunk count == exactly `n²`.
- Flips: drop `#[require(CityScoped)]` from `CityChunk` → P1 RED. Drop the task/pending removal → P2 RED.
  Run the touched `city_gate` tests **3×** and report all three (TASK-022).

### Step 19 — QA/showcase launchers
- `tools/qa/brp.py`: `QA_SETTINGS_ID = "com.github.pockerhead.maw-make-gta.qa"`; `start()` launches
  `[exe, "--settings-id", QA_SETTINGS_ID, *self.args]`; `def type_text(self, text)` → `brp_extras/type_text`.
- `tools/showcase/record.py:99`: add `"--settings-id", "com.github.pockerhead.maw-make-gta.qa"` to its launch list (R9).
- `test_brp.py` untouched.

### Step 20 — `tools/qa/scenarios/t12.py` (new)
As PLAN.md step 20, plus:
- Step 1: exactly one `Minimap` entity and one `Name == "Hud"` root.
- Step 3 (after the new city): again exactly one `Minimap`, one `Hud` root, one `Player`; `wait_chunks == 100`
  (R7: a missing `CityScoped` on a client root doubles an invisible overlay).
- `pause.png` step: also read `PauseMenu == Main` over BRP.
- Owner checklist for QA_REPORT.md: orients by the minimap; changes seed from the pause menu and from the main menu (start
  without `--seed`); settings change the game and survive a restart; Esc now means pause; while paused, audio already
  playing finishes (no audio pause in T12).

---

## 5. Risk areas (updated)

- **Pause vs same-frame death/arrest (silent, fixed by R1)**: guarded by `pause_request` + its table test. Any
  future path that sets `Paused` must go through it.
- **Teardown completeness (silent)**: G3 A1 is a generic leak detector over `Transform` entities, fed by
  production-bundle fixtures. A new spawn site with a new component set is caught only if its bundle carries neither
  `Character` nor a pickup marker. The review found none today.
- **Message buffers during pause (R8, bevy #14152)**: input `Messages` grow for the pause length. Gameplay messages
  from `Update`/BRP during pause are read on resume. `NEW_CITY` clears gameplay buffers. Accepted, documented.
- **Mass despawn frame**: ~2.5k building + ~200 block colliders, 100 chunks, props, NPCs in one frame, under the loading
  screen. Measure around the transition in t12 `game.log` if it hitches.
- **`insert_state(MainMenu)`** relies on bevy_state overwrite semantics (verified `app.rs:139-151`). G2 mirrors it.
- **Settings save while paused**: the deferred timer freezes (virtual clock). Mitigated by save-on-settings-close and
  save-on-exit. A hard kill with the settings screen still open loses the last change.
- **Settings file**: `%LOCALAPPDATA%\com.github.pockerhead.maw-make-gta\settings.toml`. QA and showcase use `.qa`. Bad
  values are clamped on load (R10).
- **Shader correctness (owner-visible)**: WGSL inverse vs `project`. The markers use the gated `map_px`, so a mismatch
  shows as dots off the streets. Arrow space fixed (R2). QA screenshot + owner run.
- **Determinism claim (R12)**: the menu seed equals `--seed` for the RNG streams only.
- **Raster memory**: 1400² RGBA8 ≈ 7.8 MB at 1 px/m; validation caps at 2 px/m.
- **Phantom red from the shared target** (probe build): see the build note.
- **`reflect_auto_register`**: the explicit `register_type` keeps the order guarantee for `SettingsPlugin::build`.

## 6. Open questions

None. Resolved questions in TASK_FINAL.md stand. The R11 move of the projection helpers to the client is a reviewer
decision within the plan's latitude (no AC names the crate).

children: 0 launched / 0 reported.
