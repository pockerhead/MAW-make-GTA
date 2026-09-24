# PLAN_FINAL — TASK-013 (GDD T12): minimap, menus, settings

Reviewer: plan-reviewer-2. Base: `PLAN_V2.md` (corrections win) + `PLAN.md` (detail restored where V2 compressed it
without correcting it). Re-checked against the tree on branch `feature/t12-minimap-menu` (HEAD `370d8ba`) and the
pinned sources in `~/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/` (bevy* 0.19.1, bevy-settings 0.19.1,
bevy_enhanced_input 0.26.0, bevy_brp_extras 0.22.6, rand_chacha 0.10.0, glam 0.32.1). No new crate and no
`Cargo.toml` change: the client already has `bevy = { default-features = true, features = ["bevy_settings"] }`
(`Cargo.toml:37`).

Cost of error: **mixed**.
- Silent class (full evidence layer): "Новый город" leaving the old city behind (colliders, a second player, stale NPCs,
  stale `SidewalkGraph`/`GangTerritories`, wanted/arrest state that holds despawned entities, a paused `Time<Virtual>`
  that freezes the new city), a pause that races a same-frame death or arrest, a mirrored raster, a projection sign
  error, RNG streams that do not follow the chosen seed.
- Owner class (mechanism + one honest gate + owner run): how the minimap reads (colours, size, radius, rotation),
  menu look and flow, settings feel, persistence between runs.

---

## 1. Summary

T12 adds two states to the sim flow (`MainMenu`, `Paused`), a pause that stops `Time<Virtual>`, and one schedule label
`flow::NEW_CITY = OnTransition { Paused → Loading }` where every domain drops its city-lifetime state. A marker
`CityScoped` (required by `Character`, pickups, city colliders, client chunks/props and put on client per-city roots)
lets one system despawn the whole city. RNGs are reseeded from `CitySeed` on every `OnEnter(Loading)`. The Esc rule
lives in the sim as a pure function `flow::pause_request` that refuses to pause while a transition is pending (a death
or arrest set in this frame's fixed tick must win). The minimap is a pure CPU raster in `citygen::minimap`
(deterministic, hash-gated), drawn by one `UiMaterial` fragment shader over an unrotated round node, with the
projection helpers (`project`, `map_px`, `heading_on_map`) in the same citygen module and gated there (GDD test plan:
"проекция мини-карты" is a `-p gta_sim`/`-p citygen` headless gate). Markers are ordinary UI nodes placed by `map_px`.
Menus (`src/menu/`): main menu only without `--seed`, pause menu with a seed field and a settings sub-screen.
Settings (`src/settings/`): `GameSettings` group via `bevy::settings::SettingsPlugin`, id overridable with
`--settings-id` so QA and showcase never touch the owner's file, `SaveSettingsDeferred` after each change,
`SaveSettings::IfChanged` when the settings screen closes, `SaveSettingsSync::IfChanged` on `AppExit`.

Signals: no new `Message`/`Event` type. Transitions go through `NextState` (buffered, applied in `StateTransition`).
Buttons are read as `Changed<Interaction>` in `Update`. Save-on-exit reads the existing `AppExit` message with its own
`MessageReader` in `Last`.

---

## 2. Implementation steps

Order: sim flow + teardown (1-6) → citygen raster and projection (7-8) → client (9-18) → QA (19-20).
Builds: one cargo command at a time, `-j 4`, in place (repo `target/`). The planner probe built into the shared target:
if an untouched module suddenly "does not exist", `touch crates/*/src/lib.rs` and rebuild (TASK-009 phantom red).
Run `cargo test -p gta_sim -p citygen` after step 8. After step 18 run `cargo test -p gta_like --bin gta_like` and
`cargo clippy -- -D warnings`.

### Step 1 — `crates/gta_sim/src/flow/mod.rs`: states, pause, `NEW_CITY`, `pause_request`
- `GameState`: add `MainMenu` (`///` "waiting for a seed; no city exists; the client starts here without `--seed`")
  and `Paused` (`///` "`Time<Virtual>` is paused; entered only from `Playing` through `pause_request`"). Keep
  `#[default] Loading` (headless tests unchanged). No exhaustive `match` on `GameState` exists (grep done).
- `pub const NEW_CITY: OnTransition<GameState> = OnTransition { exited: GameState::Paused, entered: GameState::Loading };`
  with `///` "every domain drops its city-lifetime state here".
- `pub fn pause_request(state: &GameState, next: &NextState<GameState>) -> Option<GameState>`:
  `let NextState::Unchanged = next else { return None };` then
  `match state { GameState::Playing => Some(GameState::Paused), GameState::Paused => Some(GameState::Playing), _ => None }`.
  One-line `///`: a death or arrest set in this frame's fixed tick is pending and must win (`NextState::set` overwrites,
  `bevy_state-0.19.1/src/state/resources.rs:198-200`; `NextState` has `Unchanged | Pending | PendingIfNeq`, `:180-190`).
- Two 3-line systems in `flow/mod.rs`: `pause_time(mut t: ResMut<Time<Virtual>>) { t.pause() }`,
  `resume_time(..) { t.unpause() }`. Register `add_systems(OnEnter(GameState::Paused), pause_time)` and
  `add_systems(OnExit(GameState::Paused), resume_time)`.
- `add_systems(NEW_CITY, wasted::drop_queued_damage)` (clears `Messages<DebugDamage>`). Do **not** add
  `drop_queued_input`: it resets `ActionIntent` of the player that `NEW_CITY` despawns in the same schedule (no effect).
- `NpcSystems` doc: append "`Paused` needs no entry: the fixed loop does not advance while paused (the frame that
  enters `Paused` may still run one tick with every gated set off), and `NEW_CITY` clears the buffers."
- Unit test `pause_request_table` in `flow/mod.rs`, one assert per row: (Playing, Unchanged) → Some(Paused);
  (Playing, Pending(Wasted)) → None; (Playing, Pending(Busted)) → None; (Paused, Unchanged) → Some(Playing);
  (Wasted, Unchanged) → None; (Busted, Unchanged) → None; (Loading, Unchanged) → None; (MainMenu, Unchanged) → None.
  Flip: drop the `Unchanged` guard → rows 2-3 RED. Record it.
- Why: GDD §7/§12. Transition order is Exit → Transition → Enter (`bevy_state-0.19.1/src/state/transitions.rs:225-228`),
  so `OnExit(Paused)` unpauses before `NEW_CITY`; a new city never starts frozen.

### Step 2 — `crates/gta_sim/src/world/`: `CityScoped`, teardown
- `world/mod.rs`: `#[derive(Component, Reflect, Default)] #[reflect(Component)] pub struct CityScoped;`
  (`///` "Lives as long as the current city: despawned on `flow::NEW_CITY`"), `register_type::<CityScoped>()` in
  `WorldPlugin::build`. Export it from `world`.
- `world/city.rs`: `#[require(CityScoped)]` on `CityBuilding`, `CityGround`, `CityEdgeWall`, `CityBlock`.
- `WorldPlugin` City branch: `.add_systems(NEW_CITY, (despawn_city, drop_city))`.
  `despawn_city(mut commands: Commands, scoped: Query<Entity, With<CityScoped>>)` → `commands.entity(e).try_despawn()`
  (a scoped child of a scoped parent is already gone: bevy-ecs rule). `drop_city` → `commands.remove_resource::<City>()`,
  `::<CityLayoutHash>()`, `::<CityLandmarks>()` (QA and menus never read the old hash during the new load).
  `CityGenTask` cannot exist here (pause only from `Playing`). `PlayerSpawn`/`HospitalSpawn`/`PoliceStationSpawn` are
  overwritten by the next generation before `Playing`.
- `world/mod.rs` re-export (the client has no direct `citygen` dependency; it reaches citygen through
  `gta_sim::world`, `world/mod.rs:8`): add `pub use citygen::minimap::{Raster, RasterStyle, heading_on_map, map_px, project, rasterize};`.

### Step 3 — per-domain resets in `NEW_CITY` (each plugin registers its own, no cross-domain resets)
| Domain / file | State | Reset in `NEW_CITY` |
|---|---|---|
| `character/mod.rs:32-41` | `Character` | add `CityScoped` to its `#[require(..)]` list (player, dummies, civilians, gang, police, corpses) |
| `combat/pickups.rs` | `Pickup`, `WeaponPickup`, `BatPickup` | `#[require(CityScoped)]` on each (covers range pickups and `dropped_gun`) |
| `combat/mod.rs` | `Messages<ShotFired/BulletTrace/DamageDealt/MeleeHit/melee::Strike>` | one `clear_combat_messages` system (`.clear()` each) |
| `navigation/mod.rs` | `SidewalkGraph` | `remove_resource` (also stops `NpcSystems` until rebuilt) |
| `gang/mod.rs` | `GangTerritories` | `remove_resource` |
| `gang/mod.rs` | `GangHeat.left` | every entry `0.0` (length kept) |
| `gang/mod.rs` | `PlayerTerritory` | `default()` |
| `police/mod.rs` | `PoliceDispatcher`, `ArrestAttempt`, `PoliceAlert` | `default()` each |
| `population/mod.rs` | `PopulationPhase` | `InitialFill` (the new city gets its load-time fill) |
| `population/mod.rs` | `CameraView` | `CameraView(None)` (`spawn_civilians` returns early on `None`; a stale cone would place the fill against the old camera) |
| `perception/mod.rs` | `StimulusLog` | `.0.clear()` |
| `wanted/mod.rs` | `WantedLevel`, `Crimes`, `Messages<PoliceCall>` | existing `reset_wanted` + `drop_queued_calls` (`wanted/mod.rs:215-223`) |
| `flow/` (step 1) | `Messages<DebugDamage>` | `drop_queued_damage` |

Not reset (monotonic, per-tick or overwritten): `AiClock`, `SlotCursor`, `AttackSerial`, `RouteLoad`, `PopulationLoad`,
`PerceptionLoad`, `PlayerSpawn`/`HospitalSpawn`/`PoliceStationSpawn`, `WastedClock`/`BustedClock`. Consequence: city N
after "Новый город" shares the four RNG streams with `--seed N`, not tick-exact NPC behaviour (see step 4).
Inventory check: this list equals every `derive(Resource)` and `add_message` in `gta_sim/src` (grep done); no system in
`gta_sim` or `src/` holds city data in a `Local` (only `wait_release: Local<bool>` and `spawned: Local<u32>`).
File sizes after the change stay far below 750 (population 628 → ~640).

### Step 4 — RNG reseed from `CitySeed` on every `OnEnter(Loading)`
- `CombatPlugin`, `PopulationPlugin`, `GangPlugin`, `PolicePlugin`:
  `add_systems(OnEnter(GameState::Loading), reseed_x.run_if(resource_exists::<CitySeed>))`, body
  `*rng = XRng::seeded(seed.0)` (`CombatRng`, `NpcRng` stream 1, `GangRng` stream 2, `PoliceRng` stream 3).
  TestArea has no `CitySeed` → keeps its build seed 0 (existing gates unchanged). First boot with `--seed N`: build
  seed = N → the reseed is a no-op.
- Update the `compose_sim` comment at `lib.rs:129` ("rolls from `CitySeed` at every load").
- Why: menu/new-city seed N must equal `--seed N` for the RNG streams (TASK-010 lesson: streams drive density gates).

### Step 5 — `wanted/`: one derivation of the search circle
- `wanted/mod.rs`: `pub fn search_row(rows: &[StarRow; STARS], stars: u8) -> &StarRow { &rows[usize::from(stars.max(1)) - 1] }`.
- `wanted/search.rs:79`: `let row = search_row(rows, w.stars);` (pure refactor; `wanted_search.rs` stays green).
- `impl WantedLevel { pub fn search_circle(&self, rows: &[StarRow; STARS]) -> Option<(Vec3, f32)> }`: `None` when
  `stars == 0` or `last_known` is `None`; else `Some((last_known, search_row(rows, stars).search_radius))`.
- Unit test `search_circle_per_star` in `wanted/mod.rs`: one case per star 1..5 with the shipped `wanted.ron` radii
  (loaded via the crate's config loader, not literals), plus `stars 0 → None` and `last_known None → None`. Flip:
  `rows[0]` in `search_row` → rows 2..5 RED (TASK-011 lesson: one case per table row).

### Step 6 — headless gates `crates/gta_sim/tests/new_city.rs` (new)
Uses `common::{composed_app, headless_app, run_ticks, set_view, chase_view, golden, player, position, settle, set_intent,
spawn_civilian, spawn_member, calm}` and `police_support::{raise_heat, spawn_unit}`. Helpers in the file:
- `pause(app)`: read `State<GameState>` and `NextState<GameState>`, call `flow::pause_request`, assert it is
  `Some(Paused)`, set it, update until `State == Paused` (bound 3 updates). Note: the update that applies the
  transition may still run one fixed tick (virtual delta was taken in `First`); record baselines only after `pause`
  returns.
- `new_city(app, seed)`: `CitySeed(seed)`, `NextState(Loading)`, one `update()`.
- `until_playing(app)`: bounded loop like `city_app` (120 s, panics on `should_exit`).
- `city_app_with_baseline(seed) -> (App, HashSet<Entity>)`: `composed_app(City { seed })`, one `update()`, record `B0` =
  every entity with `Transform` (expected empty; record it anyway), then `until_playing`.

**G1 `pause_freezes_the_fixed_clock`** (correctness): `headless_app()`, `settle`, `set_intent` forward (run gait),
`pause`. Record `Time<Fixed>::elapsed` and `position`. 10 updates: both unchanged, `Time<Virtual>::is_paused()`.
Resume through `pause_request` (→ `Some(Playing)`), 10 updates: elapsed grew by ≥ 8 ticks and the player moved ≥ 0.5 m.
Flip: delete `pause_time` registration → RED on elapsed.

**G2 `main_menu_waits_for_a_seed`**: build the app by hand as `composed_app` does but call
`app.insert_state(GameState::MainMenu)` right after `compose_sim` and before `finish()` (mirrors `main.rs`; overwrite
semantics `bevy_state-0.19.1/src/app.rs:139-151`), `WorldSource::City { seed: 7 }`. 30 updates: state `MainMenu`, no
`City`, no `CityLayoutHash`, zero `Player`. Then `CitySeed(2)` + `NextState(Loading)`, one update, one assert per RNG:
`CombatRng.0 == CombatRng::seeded(2).0`, same for `NpcRng`, `GangRng`, `PoliceRng` (`ChaCha8Rng: PartialEq`,
`rand_chacha-0.10.0/src/chacha.rs:210`; nothing draws in `Loading` because NPC/Playing sets are state-gated). Then
`until_playing` → `CityLayoutHash == golden(2)`. Flip each reseed separately → its row RED (4 flips).

**G3 `new_city_replaces_the_city`** (silent-defect gate):
1. `let (mut app, b0) = city_app_with_baseline(1); settle(&mut app);`
2. `raise_heat(&mut app, 200)` **first** (2 stars by the shipped table `[40, 180, ...]`; `spawn_unit` asserts
   `stars >= 1` when a `SidewalkGraph` exists, `police_support/mod.rs:38-44`).
3. Fixtures with production bundles, all on `SidewalkGraph` nodes 40..120 m (flat) from the player (sidewalk nodes are
   never inside a building; `GATE BROKEN` if fewer than 5 such nodes): 2 civilians
   (`spawn_civilian(app, GraphWalker { from: n, to: graph.neighbors(n)[0] }, 0.0, calm())`), 1 gang-0 member
   (`spawn_member(app, 0, node, Weapon::Pistol)`), 1 police unit (`spawn_unit(app, UnitKind::Patrol, node, 0.0)`),
   1 `combat::dropped_gun(Weapon::Pistol, node, &WeaponsConfig)` spawn. `set_view(Some(chase_view(feet, forward)))`,
   `run_ticks(60)`.
4. Preconditions (`GATE BROKEN` otherwise): `State == Playing`; ≥ 1 alive of each fixture kind; `WantedLevel.heat == 200`;
   `City`, `CityLayoutHash`, `CityLandmarks`, `SidewalkGraph`, `GangTerritories` present. Snapshot `A` = all entities with
   `Character`, `Pickup`, `WeaponPickup`, `BatPickup`, `CityBuilding`, `CityBlock`, `CityGround`, `CityEdgeWall`.
5. `pause(app)`. Then **dirty every reset row by hand** (no fixed tick runs while paused, so the values survive to
   `NEW_CITY`; a row whose value is already default would make its flip vacuous): `GangHeat.left[0] = 5.0`;
   `PlayerTerritory(Some(0))`; `PoliceDispatcher { units: 3, swat: 1, reinforce_left: 2.0 }`;
   `ArrestAttempt { cop: Some(unit), hold: 1.0 }`; `PoliceAlert { hostile_left: 5.0 }`; `PopulationPhase::Steady`;
   `StimulusLog.0.push((1, <any ThreatKind>, Vec3::ZERO, 1))`; `CameraView` is already `Some`; write one message of each
   public type with `world.write_message(..)` (`Entity::PLACEHOLDER` for entities): `ShotFired`, `BulletTrace`,
   `DamageDealt`, `MeleeHit`, `PoliceCall { caller, about: Cause::Attack(1) }`, `DebugDamage { amount: 1.0 }`. Assert
   each dirty value / `len() >= 1` (`GATE BROKEN` otherwise).
6. `new_city(&mut app, 2)` → **case A** (state `Loading`, right after the transition frame), one assert each:
   A1 no entity with `Transform` outside `b0` (generic leak detector); A2 `City` absent; A3 `CityLayoutHash` absent;
   A4 `CityLandmarks` absent; A5 `SidewalkGraph` absent; A6 `GangTerritories` absent; A7 `GangHeat.left` all 0 (length 2);
   A8 `PlayerTerritory(None)`; A9 `PoliceDispatcher` fields default; A10 `ArrestAttempt == default()`; A11
   `PoliceAlert.hostile_left == 0`; A12 `PopulationPhase::InitialFill`; A13 `CameraView(None)`; A14 `StimulusLog` empty;
   A15 `WantedLevel == default()`; A16 `Crimes.incidents()` empty; A17..A22 each of the 6 public `Messages<..>` has
   `len() == 0`; A23 `!Time<Virtual>::is_paused()`.
   `Messages<melee::Strike>` is `pub(super)` (`combat/melee.rs:295`), not nameable from `tests/`: it is cleared in
   `clear_combat_messages` but not asserted (its writer and reader run in the same fixed tick).
7. **case B** (after `until_playing`, then `settle`): `CitySeed == 2`; `CityLayoutHash == golden(2)`; exactly one
   `Player` (settle asserts `PlayerSpawn + float_height` ± 0.05 m); `CityBuilding` count == `City.buildings.len()`;
   `CityGround == 1`; `CityEdgeWall == 4`; `CityBlock` == blocks with `curb.len() >= 3` (`spawn_blocks`,
   `world/city.rs:214-217`); `Pickup == 2` (health + armour, `combat/pickups.rs:30-47`); `Dummy == WeaponsConfig.range.dummies`;
   no entity of `A` alive (`get_entity` is generation-checked); `set_view` + 200 ticks → `Time<Fixed>` advanced and ≥ 1
   `Civilian` spawned (natural spawner liveness in the new city).
8. Flips (record each): drop `CityScoped` from `Character`'s require → A1 RED; delete `despawn_city` → A1 RED; delete
   `drop_city` → A2 RED; delete each reset row → its assert RED (Crimes shares `reset_wanted` with WantedLevel: deleting
   `reset_wanted` → A15 RED; A16 has no independent flip, its recorder is `pub(crate)`); delete `resume_time` → A23 RED;
   delete `clear_combat_messages` → A17..A20 RED; delete `drop_queued_calls` → A21 RED; delete `drop_queued_damage` → A22 RED.

Numbers are derived: counts from loaded configs and the city layout, hashes from `golden_hashes.txt` via `common::golden`,
200 heat = 2 stars from `stars_for` (raise_heat asserts it).

### Step 7 — `crates/citygen/src/minimap.rs` (new, `pub mod minimap;` in `lib.rs`)
```rust
/// sRGB RGBA8 colours; territory alpha is `territory[g][3]`.
pub struct RasterStyle { pub px_per_m: f32, pub road: [u8; 4], pub sidewalk: [u8; 4], pub block: [u8; 4],
    pub park: [u8; 4], pub building: [u8; 4], pub territory: [[u8; 4]; 2] }
pub struct Raster { pub width: u32, pub height: u32, pub origin: Vec2, pub px_per_m: f32, pub rgba: Vec<u8> }
pub fn rasterize(layout: &CityLayout, style: &RasterStyle) -> Raster
impl Raster { pub fn hash(&self) -> u64 }
pub fn project(point: Vec2, center: Vec2, yaw: f32) -> Vec2              // metres: x right of the view, y ahead
pub fn map_px(point: Vec2, center: Vec2, yaw: f32, px_per_m: f32) -> Vec2 // UI px offset from the map centre, y down
pub fn heading_on_map(facing_yaw: f32, camera_yaw: f32) -> f32          // radians, counter-clockwise from map-up
```
- Contract (doc on `Raster`, shared with the shader): covers `[-g/2, g/2]²`, `g = layout.ground_size` (1400 m shipped);
  `origin = (-g/2, -g/2)` in layout (x, z = layout y); texel `(col, row)` covers x ∈ `origin.x + [col, col+1)/ppm`,
  z ∈ `origin.y + [row, row+1)/ppm`; **row 0 = smallest z (north, forward is −Z)**; `width = height = ceil(g·ppm)`;
  texel centres are sampled; `rgba` is row-major, 4 bytes per texel.
- Layers in order, each a scan of the polygon's texel bounding box testing the texel centre: fill with `road`; each block
  with `curb.len() >= 3`: `curb` → `sidewalk`, then `inner` (if `len >= 3`) → `park` if `is_park` else `block`; each
  building's base oriented rectangle (`|d·axis| <= half_extents.x && |d·axis.perp()| <= half_extents.y`, base only, not
  `upper_tiers`) → `building`; each block with `district == gang_districts[g]`: blend `territory[g]` over its `curb`
  polygon, `out = (dst·(255−a) + src·a + 127) / 255` per RGB channel in u32, alpha stays 255. Precompute each polygon's
  edge normals once (not `contains_convex`'s per-call work); ~2 MP × a few edges, runs async anyway.
- `hash`: `rng::fnv1a64` (`pub(crate)`, `rng.rs:23`) over `width`, `height`, `origin` quantised to mm (i64 LE),
  `px_per_m.to_bits()`, `rgba`.
- `project`: `d = point − center`, `(s, c) = yaw.sin_cos()`, return `(d.x·c − d.y·s, −d.x·s − d.y·c)` (camera right =
  `(cos, −sin)`, forward = `(−sin, −cos)` in (x, z): `src/camera/mod.rs` `follow_player` uses
  `Quat::from_euler(YXZ, yaw, pitch, 0) * NEG_Z`). `map_px` = `Vec2::new(m.x, −m.y) · px_per_m`.
  `heading_on_map` = `facing_yaw − camera_yaw`.
- Worked examples (they are the test rows; recomputed): yaw 0, point `c + (0, −10)` → `(0, 10)`; yaw 90° (`s = 1, c = 0`),
  point `c + (−10, 0)` → `x = −10·0 − 0·1 = 0`, `y = 10·1 − 0 = 10`; yaw 180° (`s = 0, c = −1`), point `c + (0, 10)` →
  `x = 0`, `y = −10·(−1) = 10`. Right rows: yaw 0 `(10, 0)` → `(10, 0)`; yaw 90° `(0, −10)` → `x = 0 − (−10)(1) = 10`,
  `y = 0`; yaw 180° `(−10, 0)` → `x = −10·(−1) = 10`, `y = 0`. `map_px` of an ahead row at 2 px/m → `(0, −20)`.
  Arrow: `project(c + (−sin f, −cos f), c, cam) = (−sin(f − cam), cos(f − cam)) = (−sin h, cos h)` with
  `h = heading_on_map(f, cam)`; rows (cam 0, f 90°) → `(−1, 0)` left; (90°, 90°) → `(0, 1)` up; (90°, 0) → `(1, 0)` right.

### Step 8 — citygen gates `crates/citygen/tests/minimap.rs` + `crates/citygen/tests/golden_minimap.txt`
Style literal in the test (independent of `strings.ron`, so colour tuning never re-blesses): `px_per_m 1.0`, six
distinct colours, territory alphas 96 and 160. Layouts from `common::layouts()`.
- **M1 `raster_hashes_match_golden`** (seeds 1, 2, 42). File format like `golden_hashes.txt` (`seed 0xhash`, `#`
  comments, parsed per line with `trim()`: `core.autocrlf=true`); `#[ignore] bless_print_minimap_golden`; bless
  command in the file header and in the test's failure message. Drift guard.
- **M2 `raster_differs_by_seed`**: the three hashes pairwise different (a blank raster is deterministic too).
- **M3 `raster_marks_known_places`** (correctness). Texel read **in the test** from the contract:
  `col = floor((x − origin.x)·ppm)`, `row = floor((z − origin.y)·ppm)`, bytes `rgba[(row·width + col)·4 ..][..4]` —
  never via a raster helper, so a mirrored/rotated raster is RED. Seed 1, one assert per case, each probe point with a
  precondition computed in the test (`contains_convex` against every block's `curb` / building rectangle / gang
  blocks), `GATE BROKEN` if no point satisfies it:
  (a) road: midpoint of the first `roads.edges` entry whose midpoint is outside every block's `curb` → `road`;
  (b) building: centre of the first building (hospital first) whose centre is not in a gang block, and whose
  row-mirrored point (`z' = −z`, i.e. row `height−1−row`) is outside every building and every gang block → `building`;
  (c) park: centroid of `blocks[landmarks.park].inner`, precondition: park block not a gang block → `park`;
  (d) block: 1 m inside an `inner` vertex (towards the centroid) of a non-park, non-gang block that no building
  contains → `block`; (e) sidewalk: midpoint of `curb[0]` and `inner[0]` of a non-gang block → `sidewalk`;
  (f) territory: construction (e) in a `gang_districts[0]` block → the blend formula applied to `sidewalk`;
  (g) asymmetry: the row-mirrored texel of (b) is **not** `building`.
  Flip: `row` ↔ `height−1−row` in `rasterize` → M3 (b)(g) and M1 RED.
- **M4 `camera_ahead_is_straight_up`** (the AC gate): yaw 0°, 90°, 180°; forward computed independently as
  `(glam::Quat::from_rotation_y(yaw) * glam::Vec3::NEG_Z).xz()` (`use glam::Vec3Swizzles`), centre `(123.0, −45.0)`,
  point = centre + 10·forward. Assert `|x| < 1e-4`, `|y − 10| < 1e-4`, `map_px(.., 2.0)` ≈ `(0, −20)`. Plus the three right
  rows (right = `Quat·X`) → `(10, 0)`. 6 rows, one assert each. Flip: negate `s` in `project` → yaw-90° rows RED (0°/180°
  stay green, which is why all three yaws exist).
- **M5 `arrow_points_along_facing`**: the three (camera, facing) rows of step 7; expected `(−sin h, cos h)` with
  `h = heading_on_map(..)` equals `project(center + facing_dir, center, camera).normalize()` (facing_dir via `Quat`).
  Flip: `camera_yaw − facing_yaw` → rows 1 and 3 RED.

### Step 9 — client configs (all new tuning values are data)
- `assets/ui/strings.ron` + `UiConfig.menu: MenuConfig`. Put `MenuConfig` in `src/menu/menu_config.rs` (config.rs is
  225 lines and would pass ~350 inline). Fields: texts `title`, `new_game`, `seed_label`, `seed_hint`, `quit`, `paused`,
  `current_seed` (must contain `{seed}`), `resume`, `new_city`, `settings`, `back`, `sensitivity`, `volume`, `invert_y`,
  `reduce_shake`, `no_flashes`, `on`, `off`; layout `title_size`, `item_size`, `item_gap`, `button_width`,
  `button_height`; colours `backdrop: Rgba` (pause), `menu_backdrop: Rgba` (main, opaque), `button_color: Rgba`,
  `button_hover_color: Rgba`, `text_color: Rgb`, `field_color: Rgba`; settings steps `sensitivity: (f32, f32, f32)` =
  `(0.25, 3.0, 0.25)` (min, max, step), `volume_step: 0.1`. `validate()`: non-empty strings, `{seed}` present, positive
  sizes, unit colours, `0 < min < max`, `step > 0`, `0 < volume_step <= 1`.
- `UiConfig.hud.minimap: MinimapConfig` (struct in `src/minimap/config.rs`, HUD-numbers precedent `config.rs:9`):
  `size` 220 px, `view_radius` 120 m, `raster_px_per_m` 1.0 (validate `(0, 2]`: 1400 m × 2 = 2800 px, 31 MB RGBA; 4 would
  allow a 125 MB texture for no benefit), colours `road`, `sidewalk`, `block`, `park`, `building` (Rgb), `outside` (Rgba),
  `territory_alpha` (0..1), `rim_color: Rgba`, `rim_px`, `search_fill: Rgba`, `search_ring: Rgba`, `search_ring_px`,
  `cone_color: Rgba`, `arrow_color: Rgb`, `arrow_px`, `dot_px`, `pickup_color: Rgb`, `hospital: (String, Rgb)`,
  `station: (String, Rgb)` (glyph + colour), `glyph_size`. Validate each field (positive, unit colours, alpha in [0, 1],
  non-empty glyphs). Start values: dark asphalt road, grey blocks, green parks, 0.35 territory, translucent red cones
  (α 0.25), blue-white search ring; owner tunes.
- `assets/juice/juice.ron` `shake.reduced_scale: 0.3` + `ShakeConfig.reduced_scale` + validate `[0, 1]`; add the field
  to the `cfg()` literal in `src/juice/shake.rs` tests (`:75-84`). Existing gates load the real `strings.ron`
  (`hud/witness_gate.rs:14`, `juice/damage_numbers_gate.rs:33`), so no other literal needs a change.

### Step 10 — `src/settings/mod.rs` (new domain)
- Imports as in the probe: `bevy::settings::{ReflectSettingsGroup, SettingsGroup, SettingsPlugin, SaveSettings,
  SaveSettingsDeferred, SaveSettingsSync}`.
- `#[derive(Resource, SettingsGroup, Reflect, Clone, Debug, PartialEq)] #[reflect(Resource, SettingsGroup, Default)]
  #[settings_group(group = "game")] pub struct GameSettings { pub mouse_sensitivity: f32, pub volume: f32,
  pub invert_y: bool, pub reduce_shake: bool, pub no_flashes: bool }`, manual `Default` = 1.0, 1.0, false, false, false
  (GDD §7 neutral multipliers; base numbers stay in `camera.ron`/`mix.ron`/`juice.ron`).
- `pub const SETTINGS_APP_ID: &str = "com.github.pockerhead.maw-make-gta";` (identity, not tuning).
- `pub struct GameSettingsPlugin { pub app_id: String }` → `app.register_type::<GameSettings>()` **then**
  `app.add_plugins(SettingsPlugin::new(&self.app_id))` (`SettingsPlugin::build` scans the registry,
  `bevy-settings-0.19.1/src/lib.rs:96-125`). Systems:
  - `Startup` `sanitize_settings(ui: Res<UiConfig>, mut s: ResMut<GameSettings>)`: `let clean = sanitize(&s, range);
    if clean != *s { *s = clean }` (write only when different, so an untouched file is not rewritten on exit).
    Pure `pub fn sanitize(s: &GameSettings, sensitivity: (f32, f32, f32)) -> GameSettings`: non-finite → default value,
    sensitivity clamped to `[min, max]`, volume to `[0, 1]`. Unit table: NaN, −1, 99, in-range (one assert each, for
    both fields).
  - `apply_volume.run_if(resource_changed::<GameSettings>)` in `Update`: `global.volume = Volume::Linear(s.volume)`
    (`GlobalVolume` applies when a sink is created, `bevy_audio-0.19.1/src/audio_output.rs:87`: new sounds only; shots
    are one-shots).
  - `save_on_exit` in `Last.after(bevy::window::ExitSystems)`: `MessageReader<AppExit>`, any message →
    `commands.queue(SaveSettingsSync::IfChanged)`.
- `pub fn step_value(value: f32, steps: i32, min: f32, max: f32, step: f32) -> f32` =
  `((value / step).round() + steps as f32) * step` clamped to `[min, max]` (keeps 0.7 from becoming 0.70000005 in the
  toml). Unit table: up one, down to min clamp, up to max clamp, off-grid `0.33 + 1 step (0.25) → 0.5`. One assert per row.
- `src/main.rs`: `--settings-id <id>` via the existing `flag_value`, default `SETTINGS_APP_ID`. Add
  `settings::GameSettingsPlugin { app_id }` as the **first** element of the presentation plugin tuple (after the
  `insert_resource(ui_config)` chain, so `UiConfig` exists for `sanitize_settings`).

### Step 11 — settings consumers (surgical)
- `src/camera/mod.rs:76-93` `apply_mouse_look`: `+ settings: Res<GameSettings>`; `sensitivity *= settings.mouse_sensitivity`;
  pitch delta sign `if settings.invert_y { -1.0 } else { 1.0 }`.
- `src/juice/shake.rs:29-39` `shake_camera`: `+ settings: Res<GameSettings>`; `shake.rotation =
  Quat::IDENTITY.slerp(shake_rotation(..), scale)` with `scale = if settings.reduce_shake { cfg.reduced_scale } else { 1.0 }`.
- `src/vfx/mod.rs:71` `spawn_flashes`: `+ settings`; when `no_flashes`, `shots.clear()` (drain the reader) and return
  (tracers stay).
- No gate harness registers `CameraPlugin`/`JuicePlugin`/`VfxPlugin`/`ShotAudioPlugin` (grep of `src/**/*_gate.rs`): no
  harness change.

### Step 12 — `src/menu/widgets.rs` (new): shared shell and widgets
- Move `title_screen` from `src/hud/wasted.rs:35-67` here as `pub(crate) fn title_screen(commands: &mut Commands,
  ui: &UiConfig, fonts: &UiFonts, name: &'static str, text: String, color: (f32, f32, f32), backdrop: Color,
  exit: impl States) -> Entity`; the node gains `flex_direction: Column` and `row_gap: px(ui.menu.item_gap)` (one child
  → the Wasted/Busted look is unchanged). `hud/wasted.rs` calls it with `rgba(ui.wasted_backdrop)`.
- `button(label: String, action: MenuAction, ui, fonts) -> impl Bundle` (classic `Button` + `Node` + `BackgroundColor` +
  child `Text`); `#[derive(Component, Clone, Copy)] enum MenuAction { NewGame, Resume, NewCity, OpenSettings,
  CloseSettings, Quit, Step(SettingKey, i32), Toggle(SettingKey) }`; `SettingKey { Sensitivity, Volume, InvertY,
  ReduceShake, NoFlashes }`.
- `seed_field(ui, fonts) -> impl Bundle`: `SeedField` marker, `EditableText { max_characters: Some(19), ..default() }`
  (19 digits always fit `u64`; law), `EditableTextFilter::new(|c| c.is_ascii_digit())`, `AutoFocus`,
  `TextFont` from `UiFonts.regular`, background `field_color`.
- `pub fn seed_from_field(text: &str, fallback: impl FnOnce() -> u64) -> u64` (empty or unparsable → fallback). Unit rows:
  `""` → fallback, `"0"` → 0, `"42"` → 42, `"9999999999999999999"` → that value.
- `pub fn clock_seed() -> u64` (nanos since epoch, `map_or(0, ..)`). `main.rs::parse_seed` becomes
  `cli_seed() -> Result<Option<u64>, String>`.

### Step 13 — `src/menu/mod.rs` + `src/menu/screens.rs` (new): main menu, pause, settings screen
- `#[derive(SubStates, Default, Clone, PartialEq, Eq, Hash, Debug, Reflect)] #[source(GameState = GameState::Paused)]
  pub enum PauseMenu { #[default] Main, Settings }`; `add_sub_state::<PauseMenu>()` + `register_type_state::<PauseMenu>()`
  (QA reads it over BRP).
- `OnEnter(GameState::MainMenu)` → `title_screen(.., ui.menu.title, .., menu_backdrop, GameState::MainMenu)` + children:
  `button(new_game, NewGame)`, row `seed_label` + `seed_field`, `seed_hint`, `button(quit, Quit)`.
- `OnEnter(PauseMenu::Main)` → shell `paused` over `backdrop`, `current_seed` with `CitySeed`, buttons `resume`,
  `new_city`, `settings`, `quit`, seed row + hint (the field is auto-focused → QA `type_text` lands there).
- `OnEnter(PauseMenu::Settings)` → shell `settings`: 5 rows (label, value text `SettingValue(SettingKey)` **spawned with
  the current value**, `−`/`+` buttons or a toggle), `back`. `update_setting_labels.run_if(resource_changed::<GameSettings>)`
  rewrites values (`"×{:.2}"`, `"{:.0}%"`, `on`/`off`).
- `Update` systems:
  - `escape`: on `ButtonInput<KeyCode>::just_pressed(Escape)`: if `Option<Res<State<PauseMenu>>>` is `Settings` →
    `NextState(PauseMenu::Main)`; otherwise `if let Some(target) = flow::pause_request(state.get(), &next) { next.set(target) }`.
    Loading/MainMenu/Wasted/Busted are refused by the rule, not by the client.
  - `submit_seed` (`run_if` MainMenu or `PauseMenu::Main`): `just_pressed(Enter | NumpadEnter)` →
    `start_city(seed_from_field(&field.value().to_string(), clock_seed))` (reads the `SeedField` entity directly, so
    focus does not matter).
  - `press_buttons`: `Query<(&Interaction, &MenuAction), Changed<Interaction>>`, on `Pressed`: `NewGame` →
    `start_city(clock_seed())` (GDD §7 "Новая игра (случайный seed)"); `NewCity` → `start_city(seed_from_field(..))`;
    `Resume` → through `pause_request`; `OpenSettings`/`CloseSettings` → `NextState<PauseMenu>`; `Quit` →
    `AppExit::Success`; `Step`/`Toggle` → mutate `GameSettings` via `step_value` then
    `commands.queue(SaveSettingsDeferred::default())`. Hover colour via `Interaction::Hovered`.
  - `start_city(seed)`: `city_seed.0 = seed; next.set(GameState::Loading)`. Called only from MainMenu/Paused (no
    fixed-tick transition can be pending there).
- `OnExit(PauseMenu::Settings)` → `commands.queue(SaveSettings::IfChanged)` (the deferred timer ticks on virtual time,
  `bevy-settings-0.19.1/src/lib.rs:585-594`, frozen while paused).
- File sizes: `menu/mod.rs` < 150, `screens.rs` < 350, `widgets.rs` < 200.

### Step 14 — `src/input/mod.rs`: pause-aware input and cursor
- Remove `cursor_toggle` (`:125-140`) and the `Startup` `capture_cursor`. Add `OnEnter(GameState::Playing)` → capture
  (`CursorCaptured(true)`, `Locked`, hidden), `OnEnter(GameState::Paused)` and `OnEnter(GameState::MainMenu)` → release.
  (The primary window exists from `WindowPlugin::build`, `bevy_window-0.19.1/src/lib.rs:128-136`, so the release on the
  first `StateTransition` before `Startup` finds it.) Drop `.before(cursor_toggle)` and its comment from
  `write_action_intent`; keep the `wait_release` latch (a click on "Продолжить" must not fire on resume).
- `OnEnter(Paused)` → insert `ContextActivity::<OnFoot>::INACTIVE` on the input entity, `OnExit(Paused)` → `ACTIVE`
  (`bevy_enhanced_input-0.26.0/src/context.rs:732-744`). One-line comment with the real reason: `write_move_intent` has no
  capture check (`:142-163`), so Space during the pause would latch `jump_requested` and jump on resume.
- Owner note: Esc now pauses (T1 "Esc отпускает курсор" is gone); the LMB recapture path is gone too, so after an
  alt-tab that drops the grab the cursor comes back with Esc twice.

### Step 15 — `src/main.rs`: main menu vs `--seed`
`let cli = cli_seed()?; let seed = cli.unwrap_or_else(clock_seed); compose_sim(.., City { seed })`;
`if cli.is_none() { app.insert_state(GameState::MainMenu); }` right after `compose_sim`. Every existing QA/showcase
script passes `--seed` → unchanged boot. Add `settings::GameSettingsPlugin { app_id }` (step 10) and
`minimap::MinimapPlugin` to the plugin tuple; `mod settings; mod minimap;`.

### Step 16 — `CityScoped` on client per-city spawns, cancel in-flight city work
- `CityScoped` on the roots of `hud/mod.rs::spawn_hud` (`Name "Hud"`), `hud/weapon.rs::spawn_weapon_hud` (Ammo,
  Crosshair dot, 4 arms, Hit marker) and `hud/stars.rs::spawn_stars` (they respawn on every `Loading → Playing`).
- `#[require(CityScoped)]` on `CityChunk` (`visuals/city.rs:45`) and `CityProp` (`visuals/props.rs`). New
  `NEW_CITY` system in `CityVisualsPlugin` removes `CityMeshTask` and `PendingCitySpawn` (dropping a `Task` cancels
  it). Update the comment at `visuals/city.rs:24` ("Once per session" → "Once per city: returning from `Wasted`
  must not build it again").
- `camera/mod.rs`: `add_systems(NEW_CITY, reset_pivot)`.
- Witness bars, damage numbers, tracers and flashes expire by themselves or are keyed to live entities; sky and sun are
  `Startup` one-shots that outlive cities by design. No tag.

### Step 17 — `src/minimap/`: `mod.rs`, `material.rs`, `markers.rs`, `config.rs`; `assets/shaders/minimap.wgsl`
- `MinimapPlugin`: `UiMaterialPlugin::<MinimapMaterial>::default()`, `register_type::<MinimapMarker>()`,
  `register_type::<Minimap>()` (`#[derive(Component, Reflect)] #[reflect(Component)] struct Minimap;` so QA can count it).
- `OnTransition { Loading → Playing }`: `spawn_minimap` — root `(Name::new("Minimap"), Minimap, CityScoped,
  Node { position_type: Absolute, left: px(hud.margin), bottom: px(hud.margin), width/height: px(size) },
  Visibility::Hidden)` until ready; two landmark children (`MinimapMarker { target: None, kind: Hospital/Station }`
  with glyph text) anchored at `HospitalSpawn.point` / `PoliceStationSpawn.point`. `start_raster` — an
  `AsyncComputeTaskPool` task over `City.0.clone()` and a `RasterStyle` built from `MinimapConfig` + `GangConfig`
  tints (alpha = `territory_alpha`), stored as `MinimapRasterTask(Task<Raster>)`.
- `Update` `poll_raster.run_if(resource_exists::<MinimapRasterTask>)` with `block_on(poll_once(..))` (no blocking wait):
  on ready `Image::new(Extent3d { width, height, depth_or_array_layers: 1 }, TextureDimension::D2, rgba,
  TextureFormat::Rgba8UnormSrgb, RenderAssetUsages::RENDER_WORLD)` (`bevy_image-0.19.1/src/image.rs:1102`), sampler
  `ImageSampler::linear()`, insert `MaterialNode(materials.add(MinimapMaterial { .. }))` on the root, set it visible,
  remove the task. `NEW_CITY` → remove `MinimapRasterTask` (the root is `CityScoped`).
- `material.rs`: `#[derive(Asset, TypePath, AsBindGroup, Debug, Clone)] pub struct MinimapMaterial { #[uniform(0)]
  data: MinimapUniform, #[texture(1)] #[sampler(2)] map: Handle<Image> }` (`UiMaterial: AsBindGroup + Asset + Clone`,
  `bevy_ui_render-0.19.1/src/ui_material.rs:102`); `impl UiMaterial { fn fragment_shader() -> ShaderRef {
  "shaders/minimap.wgsl".into() } }`. `#[derive(ShaderType, Clone, Debug, Default)] MinimapUniform`: `center: Vec2`
  (player x, z), `heading: Vec2` (sin, cos of camera yaw), `raster_origin: Vec2`, `raster_size: Vec2` (m = width/ppm),
  `view_radius: f32`, `arrow_heading: f32`, `rim_px: f32`, `arrow_px: f32`, `search: Vec4` (x, z, radius, on 0/1),
  `cone: Vec4` (range, cos(half angle), count, search_ring_px), colours `outside, rim, search_fill, search_ring,
  cone_color, arrow_color: Vec4` (linear, converted from config sRGB), `cops: [Vec4; MAX_CONES]` (x, z, fwd x, fwd z).
  `const MAX_CONES: usize = 16;` (shader array length, a law ≥ escalation max 12; nearest 16 if ever more). The WGSL
  array length literal must equal it (comment on both sides).
- `minimap.wgsl`: `#import bevy_ui::ui_vertex_output::UiVertexOutput`; bindings `@group(1) @binding(0/1/2)`
  (`SetUiMaterialBindGroup<M, 1>`, `ui_material_pipeline.rs:215-216`). `p = (in.uv − 0.5)·2` (UI uv origin top-left,
  y down); `length(p) > 1` → alpha 0. `m = vec2(p.x, −p.y)·view_radius` (y ahead). `w = center + vec2(c, −s)·m.x +
  vec2(−s, −c)·m.y` with `(s, c) = heading`. `uv = (w − raster_origin)/raster_size`. Colour = `textureSampleLevel(map,
  samp, uv, 0.0)` (not `textureSample`: uniformity analysis rejects implicit derivatives in non-uniform flow), or
  `outside` when `uv ∉ [0,1]²`. Then search fill and ring (distance of `w` to `search.xy`; ring width px → m via
  `view_radius·2/in.size.x`), cones (`d < range && dot(v/d, fwd) >= cos_half`), arrow, rim. **Arrow in m space**: apex
  direction `(−sin h, cos h)` in `m` (y up), which is `(−sin h, −cos h)` in `p`. The header comment names both spaces.
- `PostUpdate` `update_minimap.after(crate::camera::follow_player).before(UiSystems::Layout)`: centre = player
  `Transform` xz, yaw = `OrbitCamera.yaw`, arrow = `heading_on_map(player yaw from rotation.to_euler(EulerRot::YXZ).0,
  yaw)`, circle = `WantedLevel::search_circle(&WantedConfig.stars)`, cones = nearest ≤ 16 live `PoliceUnit`s
  (`Without<Dead>`) within `view_radius + cop_view_distance`, `cone = (cop_view_distance, cos(cop_view_cone_deg/2))`,
  forward = `rotation * NEG_Z`. Write through `materials.get_mut`. No `Player` → return early.
- `markers.rs`: `#[derive(Component, Reflect)] #[reflect(Component)] pub struct MinimapMarker { pub target:
  Option<Entity>, pub kind: MarkerKind }`, `MarkerKind { Police, Gang(u8), Pickup, Hospital, Station }` (Reflect).
  `sync_markers` (`Update`, like `sync_witness_bars`): despawn markers whose target is gone, `Dead`, or an unavailable
  pickup; spawn a round dot child (`Node { border_radius: BorderRadius::MAX, .. }`) for new `PoliceUnit` (colour
  `police_tint`), `GangMember` (`GangConfig.gangs[g].tint`), available `Pickup`/`WeaponPickup`/`BatPickup`
  (`pickup_color`). `place_markers` (`PostUpdate`, same slot as `update_minimap`): `px = map_px(target.xz, centre, yaw,
  size/(2·view_radius))`; `left = size/2 + px.x − dot/2`, `top = size/2 + px.y − dot/2`. NPC/pickup markers hidden beyond
  `view_radius`; landmark glyphs clamp to the rim.
- Config ownership: the minimap reads `GangConfig` tints, `CharacterVisualConfig.police_tint` and `WantedConfig`
  (search rows, cone). Precedent: `hud/mod.rs` reads `HealthConfig`. Nothing is copied into a second file.

### Step 18 — client presentation gate `src/visuals/city_gate.rs` (extend)
- **P1 `new_city_replaces_city_visuals`**: `city_visuals_app(1)` → record chunk and prop entity sets. Pause via
  `flow::pause_request`, then `CitySeed(2)`, `NextState(Loading)`; update until the existing `done` predicate. Assert
  chunks == `n²` (worked example of `city_meshes_are_merged_per_chunk`: size 1200 / chunk 128 → 10 → 100), chunk and prop
  sets disjoint from city 1's, `Player == 1`.
- **P2 `new_city_cancels_pending_city_spawn`**: a hand-built loop (same setup as `city_visuals_app`) that stops at the
  first update where `PendingCitySpawn` exists (`GATE BROKEN` if the spawn finished in that frame). Pause, then new
  city 2. Precondition: `PendingCitySpawn` still exists in the update right before `new_city` (`apply_city_spawn` is not
  state-gated and keeps spawning 10 chunks/frame while paused, `render.ron:8`; `GATE BROKEN` otherwise). At the end chunk
  count == exactly `n²` (without the removal, city-1 chunks keep spawning during the new `Loading` → > `n²`).
- Flips: drop `#[require(CityScoped)]` from `CityChunk` → P1 RED; drop the `PendingCitySpawn` removal → P2 RED. The
  `CityMeshTask` removal has no flip (whether an orphaned task leaks depends on which task finishes first); it is covered
  by review. Run the touched `city_gate` tests **3×** and report all three (TASK-022 lesson).

### Step 19 — QA/showcase launchers
- `tools/qa/brp.py`: `QA_SETTINGS_ID = "com.github.pockerhead.maw-make-gta.qa"`; `start()` launches
  `[str(executable), "--settings-id", QA_SETTINGS_ID, *self.args]`; `def type_text(self, text): return
  self.call("brp_extras/type_text", {"text": text})`.
- `tools/showcase/record.py:99`: add `"--settings-id", "com.github.pockerhead.maw-make-gta.qa"` to its launch list
  (owner settings such as invert Y must not change the showcase clips).
- `tools/qa/test_brp.py` untouched (it mocks `call`).

### Step 20 — `tools/qa/scenarios/t12.py` (new, runtime QA per AC)
Imports helpers like earlier scenarios (`game_state`, `resource_value`, `screenshot`, `wait_chunks`, `set_heat`, `poll`).
`Game(features=("dev",), args=("--seed", "1"), release=True)`.
**BRP constraint (verified):** `bevy_brp_extras` releases `send_keys` and mouse holds on `Res<Time>` in `Update`
(`keyboard/keys.rs:149-155`, `mouse/button.rs:114`, `mouse/click.rs:166`) = the virtual clock, frozen while paused. A key
sent during or just before the pause stays held until the game unpauses; a second press of the same key while paused
gives no `just_pressed`. So: each key at most once per pause, the seed goes in with `type_text` (per-frame, no timer),
no BRP mouse clicks on menu buttons while paused, and no resume check over BRP (owner run).
1. Playing, `CityLayoutHash == golden[1]`, `wait_chunks` settles at 100; exactly one `Minimap` entity, one
   `Name == "Hud"` root; `MinimapMarker` entities with kind `Hospital` and `Station` exist.
2. `send_keys(["Escape"], 100)` → poll `GameState == Paused` and `PauseMenu == Main`; `AiClock.tick` read twice 1 s apart
   → equal (fixed loop frozen); screenshot `pause.png` (seed "1" visible).
3. `type_text("42")`, wait ≥ 0.2 s, `send_keys(["Enter"], 100)` → poll `Loading` then `Playing` (≤ 180 s),
   `CitySeed == 42`, `CityLayoutHash == golden[42]`, exactly one `Player`, one `Minimap`, one `Hud` root,
   `wait_chunks == 100` (no leftovers), `AiClock.tick` advancing.
4. Minimap under wanted: `set_heat(game, 200)` (2 stars); poll until ≥ 1 `PoliceUnit` within `view_radius` (read from
   `strings.ron`) of the player; the set of `MinimapMarker { kind: Police }` targets equals the live cops within
   `view_radius − 5 m` (rim tolerance; 3 samples); screenshot `minimap_wanted.png` (circle, cones, blue dots).
5. `frame_report()` in the summary (FPS only via `frame_report`, TASK-010 lesson); `log_errors` empty with "shader" and
   "wgsl" added to the error words for this scenario; `summary.json`.
Evidence classes: hard pass/fail = states, hashes, counts, marker sets, tick freeze; screenshots = owner evidence.

---

## 3. Test plan

| Gate | Where / command | Class | Carries |
|---|---|---|---|
| `pause_request_table` | `flow/mod.rs`, `cargo test -p gta_sim` | correctness | R1 race: pending death/arrest wins over Esc |
| `search_circle_per_star` | `wanted/mod.rs` | correctness | one row per star |
| G1 `pause_freezes_the_fixed_clock` | `gta_sim/tests/new_city.rs` | correctness | pause stops fixed time and movement; resume restarts |
| G2 `main_menu_waits_for_a_seed` | same | correctness | MainMenu builds nothing; each RNG reseeds from `CitySeed` |
| G3 `new_city_replaces_the_city` | same | correctness (silent class) | teardown leak detector + each reset row + new city liveness |
| M1/M2 | `citygen/tests/minimap.rs`, `cargo test -p citygen` | drift / liveness | raster determinism by seed (AC 1) |
| M3 | same | correctness | raster orientation and layers |
| M4 | same | correctness | AC 2: 10 m ahead at yaw 0/90/180 is straight up |
| M5 | same | correctness | arrow direction |
| `sanitize`, `step_value`, `seed_from_field` tables | client unit tests | correctness | settings/seed input edge cases |
| P1/P2 | `src/visuals/city_gate.rs`, `cargo test -p gta_like --bin gta_like` (3 runs) | correctness | client visuals replaced, pending spawn cancelled |
| t12 | `python tools/qa/scenarios/t12.py --out <dir>` | runtime QA | AC 3: Esc, pause screenshot, typed seed, new hash, minimap screenshot |

Every new gate is flip-RED'd as listed in its step, with the perturbed input recorded in the stage summary.
Also run: `cargo build`, `cargo clippy -- -D warnings`, `cargo test -p gta_sim -p citygen`, `cargo test -p gta_like --bin
gta_like`, `python tools/qa/tree_check.py` (no `bevy_render` in `gta_sim`: this plan adds no dependency). Existing
tests stay green (TestArea keeps seed 0; boot with `--seed` is unchanged).

Owner checklist for QA_REPORT.md (AC 4): orients by the minimap (rotation, arrow, circle, cones, territories readable);
changes the seed from the pause menu and from the main menu (start without `--seed`); settings (sensitivity, invert Y,
volume, reduce shake, no flashes) change the game and survive a restart; Esc now means pause (and is ignored during
Wasted/Busted); while paused, sounds already playing finish (no audio pause in T12); a volume change applies to new
sounds.

---

## 4. Rollout notes

- No migrations, no new crates, no feature flags. New CLI flag `--settings-id <id>` (default
  `com.github.pockerhead.maw-make-gta`). New data fields in `assets/ui/strings.ron` (`menu`, `hud.minimap`) and
  `assets/juice/juice.ron` (`shake.reduced_scale`); strict loaders reject a file without them, so asset and code land
  in one commit.
- Settings file: `%LOCALAPPDATA%\com.github.pockerhead.maw-make-gta\settings.toml` (owner), `...maw-make-gta.qa\` (QA,
  showcase). Bad hand-edited values are clamped on load.
- Behaviour changes: Esc pauses instead of releasing the cursor; boot without `--seed` stops at the main menu (all
  scripts pass `--seed`).
- Messages do not rotate while `Time<Virtual>` is paused (`bevy_time-0.19.1/src/lib.rs:95-98`, bevy #14152): input
  buffers grow for the pause length, gameplay messages written from `Update`/BRP during the pause are read after
  resume, `NEW_CITY` clears the gameplay ones. Do not "fix" with `ShouldUpdateMessages::Always` (it would drop sim
  messages of the last tick before the pause).
- Settings save while paused: the deferred timer is frozen; mitigated by save on settings close and on exit. A hard kill
  with the settings screen open loses the last change.
- Mass despawn frame: ~2.5k building + ~200 block colliders, 100 chunks, props and NPCs in one frame under the loading
  screen. Check `game.log` frame time around the transition in t12 if it hitches.
- Determinism claim: a menu seed equals `--seed` for the four RNG streams only, not tick-exact NPC behaviour.
- Raster memory: 1400² RGBA8 ≈ 7.8 MB at 1 px/m, freed with the root; validation caps at 2 px/m.
- `reflect_auto_register` already registers `GameSettings`; the explicit `register_type` keeps the order guarantee for
  `SettingsPlugin::build`.
- After the task: update `docs/narrative-graph.md`, `README.md` ("Статус", "Запуск": main menu, Esc, `--settings-id`)
  and the "Проект" section of `AGENTS.md` (AGENTS rule).

---

## 5. Review notes (what changed from PLAN_V2 and why)

**Disconfirmation.** Counter-example chosen before evaluating: "G3, the central silent-class gate, as written in V2,
either does not compile or fails on its own plumbing". Searched `tests/police_support/mod.rs`, `tests/common/mod.rs`,
`combat/melee.rs`. **It held (the plan was wrong):** `spawn_unit` asserts `stars >= 1` when a `SidewalkGraph` exists
(`police_support/mod.rs:38-44`), so V2's order (fixtures, then `raise_heat`) panics `GATE BROKEN`; and
`Messages<melee::Strike>` is `pub(super)` (`combat/melee.rs:295`), so "each of the 7 `Messages<..>`" cannot compile in an
integration test. I also re-tested V2's own counter-example (camera yaw convention, `src/camera/mod.rs` `follow_player`):
it held; all six projection rows and the arrow formula recompute correctly (step 7).

Changes:
1. **Projection back in `citygen::minimap` (reverses V2 R11).** GDD test plan (`GDD.md:675`) lists "проекция
   мини-карты" among the headless gates of `cargo test -p gta_sim`, `-p citygen`, and the AC names those commands. The
   client has no `citygen` dependency (`Cargo.toml`), so step 2 adds a re-export through `gta_sim::world` (the existing
   path for `CityLayout`, `world/mod.rs:8`). V2 did not mention this re-export for the raster either.
2. **G3 fixed** (disconfirmation): `raise_heat` before `spawn_unit`; fixtures on sidewalk nodes 40..120 m out (never
   inside a building, outside arrest reach); the 6 public messages asserted, `Strike` cleared but not asserted.
3. **G3 flips made non-vacuous.** V2 claimed "delete each reset row → its assert RED", but with idle fixtures most rows
   (`PlayerTerritory`, `ArrestAttempt`, `PoliceAlert`, `StimulusLog`, `GangHeat`, messages) may already be default, so the
   flip would stay GREEN. The gate now dirties each row by hand after `pause` (no fixed tick runs while paused) and
   asserts the dirty value as a precondition. `Crimes` has no independent flip (recorder is `pub(crate)`).
4. **BRP key/mouse holds never release while paused** (`bevy_brp_extras` timers on virtual `Time`). Added the
   constraint to t12 and a PCTX proposal. The AC flow still works because each key is sent once.
5. **Pause frame runs one fixed tick** with state `Paused` (virtual delta is taken in `First`, before
   `StateTransition`). G1 records baselines after the transition update; the `NpcSystems` doc text says so.
6. **`drop_queued_input` removed from `NEW_CITY`**: it resets the intent of the player being despawned (no effect).
7. **`Raster::texel` removed**: M3 must not use a raster helper, and nothing else calls it (YAGNI).
8. **Settings**: `sanitize` writes only when a value changes (an untouched file is not rewritten on exit); settings
   labels are spawned with the current values (`resource_changed` does not fire on screen open); `GameSettingsPlugin`
   goes first in the plugin tuple, after `UiConfig` is inserted.
9. **Restored from PLAN.md** where V2 compressed without correcting: full resource/message table, `MenuConfig` and
   `MinimapConfig` field lists, `title_screen` signature, `MenuAction`/`SettingKey`, `press_buttons`/`submit_seed`/
   `start_city` behaviour, uniform layout, shader steps, markers, t12 steps, owner checklist.
10. Minor: comment at `visuals/city.rs:24` updated ("once per city"); `CityMeshTask` removal has no flip (timing), said
    so; WGSL array length must equal `MAX_CONES`; alt-tab cursor note for the owner; `GlobalVolume` affects new sounds only.

Kept from V2 as verified: R1 (`pause_request`), R2 (arrow space), R3 (`@group(1)`, `Clone`), R4 (M3 preconditions), R5
(production bundles), R6 (`ContextActivity` reason), R7 (overlay counts in QA), R8 (message rotation), R9
(`record.py`), R10 (sanitize), R12 (narrow determinism claim), R13 (P2 precondition).

Design note, not changed: GDD §12 says consumers should not open another domain's config; the minimap reads
`GangConfig`, `police_tint` and `WantedConfig` like `hud/mod.rs` already reads `HealthConfig`. No value is duplicated.

children: 0 launched / 0 reported.
