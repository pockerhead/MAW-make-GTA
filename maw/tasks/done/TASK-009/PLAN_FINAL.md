# PLAN_FINAL — TASK-009 (GDD T8): civilians

Pinned versions (`Cargo.lock`, sources under `~/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/`): bevy / bevy_ecs /
bevy_app / bevy_state / bevy_gltf / bevy_animation / bevy_world_serialization / bevy_camera 0.19.1, avian3d 0.7.0,
bevy-tnua 0.32.0 (+ physics-integration-layer 0.13.0), vendored `vendor/bevy-tnua-avian3d-0.12.1`, rand_chacha 0.10.0.
**No new crate, no `Cargo.lock` change.** `pathfinding` (GDD §10.1) is not added: civilians never run A* (GDD §6.2), A*
arrives with police (T11).

Evidence (all under `maw/tasks/in_progress/TASK-009/scratch/`, nothing in the checkout touched):
- `probe_npc_dev.log`, `probe_npc_release.log`, `probe_corpse.log` (planner): Tnua cost, ray cost, corpse components.
- `probe_nodes.log` (reviewer 2, `probe_ws/crates/gta_sim/tests/probe_nodes.rs`): sidewalk node density.
- `probe_glb.log` (reviewer 2, `probe_ws/tests/probe_glb.rs`): real GLB loading + animation in a headless client test.

## 1. Summary

Four new `gta_sim` domain plugins, exactly the GDD §12 map. `navigation/` owns a validated `SidewalkGraph` built from
`City` plus pure graph steering. `perception/` owns time-sliced hearing/sight and a short stimulus log. `civilian/` owns the
enum FSM (`Wander/Idle/Flee/Cower/Report/Dead`) and a pure utility scorer. `population/` owns the capped bubble: node
spawning off-camera, a one-shot initial fill closer than the 60 m ring at load (resolved Q2), despawn beyond 150 m after
2 s off-frame, and corpses (30 s, limit 20). A civilian is an ordinary Tnua `Character` without `Loadout`; the AI only
writes `MoveIntent`, so damage, knockback and hit reactions work unchanged. All gameplay runs in `FixedUpdate` under the
single `compose_sim` composition and is gated headless. The sim knows the camera only through a derived resource
`CameraView(Option<ViewCone>)` that the client publishes after `follow_player`; `None` means "not known yet" and blocks
spawning. The client picks one of several Kenney models per civilian (resolved Q1) and builds one `AnimationGraph` per
model from that model's own clips (a male-a clip on a female-b instance moves nothing: probe 0.0000 rad vs 1.183 rad). It
also plays `die`/`crouch` and draws a filling witness bar during `Report`. Every tuning number lives in
`assets/npc/*.ron`, `assets/character/visual.ron` or `assets/ui/strings.ron`.

### Verified facts this plan relies on

- Main schedule order: `First, PreUpdate, StateTransition, RunFixedMainLoop, Update, SpawnScene, PostUpdate, Last`
  (`bevy_app-0.19.1/src/main_schedule.rs:30-36,225-229`). `spawn_player` runs on `OnTransition{Loading→Playing}`
  (`player/mod.rs:30-34`) inside `StateTransition`, so no `PreUpdate` system of that frame can see the player. The camera
  pose is written in `PostUpdate` by `follow_player` (`src/camera/mod.rs:43-46,89-156`). The first fixed ticks of
  `Playing` therefore run without a valid camera view.
- `HealthSystems::{Damage, Regen, Pickup, Death}` are chained (`character/mod.rs:91-100`). `drive_characters` is in
  `TnuaUserControlsSystems` (`:101-104`), **not** in `PlayingSystems`. No existing system is ordered after
  `TnuaUserControlsSystems` inside a Health set (grep of `.after(`/`.before(`), so ordering Health sets before it creates no
  cycle. Combat systems are `.in_set(PlayingSystems)` (`combat/mod.rs:80-105`).
- `ShotFired` (`hitscan.rs:55-62`) is written on every trigger pull that fired, including into the sky. `DamageDealt`
  (`:66-86`) is written for every hit on a live `Health`. `MeleeHit` is in `melee.rs`. All three are buffered `Message`s.
  `aim_yaw(d) = atan2(−d.x, −d.z)` (`hitscan.rs:105`), `move_direction(axis, yaw) = R_y(yaw)·(x, 0, −y)` (`intent.rs:54-57`),
  private `unit_f32` (`hitscan.rs:100`), `CombatRng` = `ChaCha8Rng::seed_from_u64(seed)` (`hitscan.rs:95`).
- `CityLayout.sidewalks: WalkGraph { nodes: Vec<Vec2>, edges: Vec<(u32,u32)> }` (`citygen/src/layout.rs:92-95`), layout
  (x, y) = world (x, z), sidewalk top = `roads.curb_height` (`world/city.rs:97`). City building colliders are spawned
  at once in `apply_city_generation` (`world/city.rs:97-117`), so World-mask occlusion rays are valid from the first
  `Playing` tick.
- Seed 1 graph (probes): 579 nodes, 1134 edges, 59 278 m, one component, max degree 6, **min edge and min node-pair
  distance 4.5 m**. Around the player spawn: **16 nodes in 60-120 m, 24 in 20-120 m** (18 outside a 120° wedge). Over
  all 579 node positions as player position, eligible 60-120 m nodes outside a 120° wedge: min 1, p10 6, median 10,
  max 20. Node-only spawning cannot reach 40 in one tick; the cap fills over seconds as walkers vacate nodes.
- Tnua cost: ≈ 3 µs per NPC per fixed tick in the seed-1 city (64 walkers 0.756 ms mean vs 0.557 ms empty, test
  profile); one 20 m ray ≈ 0.6 µs; per-tick max has 6-8 ms OS spikes, so only the mean is gateable. GDD §10.2's
  kinematic fallback is not needed.
- Corpse: inserting `(TnuaToggle::Disabled, RigidBody::Static, CollisionLayers::NONE)` freezes the body (0.00000 m in
  128 ticks); the chest ray stops hitting it; a walker passes through; `try_despawn` removes the head-hitbox child
  (`probe_corpse.log`; `bevy_ecs-0.19.1/src/system/commands/mod.rs:1921`, `Children` is `linked_spawn`,
  `hierarchy.rs:148`). `TnuaToggle` is re-exported as `bevy_tnua::TnuaToggle` (`bevy-tnua-0.32.0/src/lib.rs:202`).
- Kenney GLBs (parsed JSON of all 12): 32 animations in **identical order**, meshes `body-mesh`/`head-mesh`, identical
  joint names **and joint translations**, root node named after the file (`character-female-b`, ...). bevy_gltf puts the
  root name into `AnimationTargetId` paths (`bevy_gltf-0.19.1/src/loader/mod.rs:559-563,1545-1558`). The skeleton is
  shared, so one `scale()` (`height / model_height`) fits every model; only hair height differs.
- Headless client GLB load works (`probe_glb.log`): `MinimalPlugins + TransformPlugin + AssetPlugin{file_path} +
  ImagePlugin + MeshPlugin + AnimationPlugin + WorldSerializationPlugin + GltfPlugin` + `init_asset::<StandardMaterial>()`
  loads the scene, fires `WorldInstanceReady`, and the `walk` clip turns `leg-left` by 1.183 rad (own model) vs 0.0000 rad
  (male-a clip on female-b). GltfPlugin warns once about a missing `CompressedImageFormatSupport`; harmless.
- APIs used and confirmed in pinned source: `SystemCondition::or` (`bevy_ecs-0.19.1/src/schedule/condition.rs:537`),
  `any_with_component` (`condition.rs:1183`), `in_state` (`bevy_state-0.19.1/src/condition.rs:103`), `resource_exists`
  (`condition.rs:730`), `ChaCha8Rng::set_stream` (`rand_chacha-0.10.0/src/chacha.rs:179`), `GltfAssetLabel::{Scene,
  Animation}`, `AnimationGraph::{add_clip, add_clip_with_mask, add_target_to_mask_group}` (used in
  `src/visuals/character.rs` today), `WorldInstanceReady` at `bevy::world_serialization` (TASK-005 lesson).

### Cost of error → gate class

Silent defects get headless gates: a walker off its graph edge, perception firing for one slot only or never, a spawn
in view or outside its radius, a spawn before the view is known, despawn inside 150 m or before 2 s, corpse pile-up, a
corpse still blocking bullets/walkers, a civilian model left in T-pose, unbounded per-tick perception work. Owner-visible
defects (crowd feel, clip look, tints, witness bar look, FPS with 40 skinned civilians, first-seconds street life) go to
the owner checklist plus BRP evidence.

## 2. Implementation steps

Order = dependency order. Each step names its check.

### Data (every tuning value here, none as `const`)

1. **`assets/npc/population.ron`** (new):
   ```
   (
       max_civilians: 40,
       spawn_ring: (60.0, 120.0),        // m from the player, GDD §6.1 steady-state spawning
       spawns_per_tick: 1,
       initial_inner_radius: 20.0,       // m; one-shot fill at load uses [this, spawn_ring.1] (resolved Q2)
       initial_spawns_per_tick: 8,       // initial fill budget per fixed tick
       spawn_min_separation: 4.0,        // m to any living civilian or corpse; < 4.5 m min node spacing => one per node
       spawn_view_margin_deg: 5.0,       // spawn only outside ViewCone.half_angle + this
       despawn_distance: 150.0,
       despawn_offscreen_seconds: 2.0,
       corpse_seconds: 30.0,             // GDD §4.3
       corpse_limit: 20,
   )
   ```
   `PopulationConfig` in `crates/gta_sim/src/population/mod.rs`, `#[serde(deny_unknown_fields)]`, `validate()`: all
   floats finite; `0 < initial_inner_radius < spawn_ring.0 < spawn_ring.1 < despawn_distance`; `spawns_per_tick ≥ 1`,
   `initial_spawns_per_tick ≥ 1`, `spawn_min_separation > 0`, `0 ≤ spawn_view_margin_deg < 90`,
   `despawn_offscreen_seconds ≥ 0`, `corpse_seconds > 0`. `max_civilians`/`corpse_limit` are `u32`.
2. **`assets/npc/perception.ron`** (new): `(slots: 4, hearing_radius: 40.0, fight_hearing_radius: 15.0,
   corpse_sight: 20.0, aimed_distance: 15.0, aimed_cone_deg: 10.0)`. `PerceptionConfig` (`perception/mod.rs`):
   `slots: u8 ≥ 1`, radii finite > 0, `0 < aimed_cone_deg < 90`.
3. **`assets/npc/navigation.ron`** (new): `(arrive_radius: 0.5)`. `NavigationConfig` (`navigation/mod.rs`): finite > 0.
4. **`assets/npc/civilian.ron`** (new):
   ```
   (
       wander_gait: Walk,
       flee_gait: Run,
       idle_chance: 0.15,             // per node arrival while wandering
       idle_seconds: (2.0, 8.0),
       flee_distance: (30.0, 60.0),   // m of travelled path
       cower_seconds: (3.0, 6.0),
       call_seconds: 4.0,             // Report duration (GDD §6.2)
       reaction: (
           flee: 1.0,
           cower: 1.5,
           report: 0.8,
           temperament_spread: 0.5,   // each temperament factor in [1 - s, 1 + s]
           panic_distance: 15.0,      // cower weight 0 at and beyond this
           report_min_distance: 25.0, // an active threat closer than this is never phoned in (resolved Q3)
       ),
   )
   ```
   `CivilianConfig` + `ReactionConfig` (`civilian/mod.rs`): ranges ordered `(lo ≤ hi)`, finite, ≥ 0;
   `0 ≤ idle_chance ≤ 1`; weights ≥ 0; `0 ≤ temperament_spread < 1`; `panic_distance > 0`; `report_min_distance ≥ 0`;
   `call_seconds > 0`. `Gait` is the existing `character::Gait` (already derives `Deserialize`, `intent.rs:6`).
5. **`assets/character/visual.ron`** (edit): keep `model` (player, male-a) and `tint`. Add:
   ```
   civilian_models: [
       "third_party/mini-characters/character-female-a.glb",
       "third_party/mini-characters/character-female-b.glb",
       "third_party/mini-characters/character-female-d.glb",
       "third_party/mini-characters/character-male-b.glb",
       "third_party/mini-characters/character-male-c.glb",
       "third_party/mini-characters/character-male-e.glb",
   ],
   civilian_tints: [(1.0, 1.0, 1.0), (0.55, 0.75, 1.0), (1.0, 0.7, 0.45), (0.6, 1.0, 0.6)],  // multipliers of tinted_mesh
   death: "die",     // played once for every Dead character, holds the last pose
   cower: "crouch",  // looped while a civilian is in Cower
   ```
6. **`assets/ui/strings.ron`** (edit, `hud` section): `witness_bar: (width: 48.0, height: 6.0, head_offset: 0.35,
   fill_color: (1.0, 0.85, 0.2), back_color: (0.0, 0.0, 0.0, 0.55))` — px size, metres above the head top, colours.
   Extend the HUD config struct in `src/menu/config.rs` with a `WitnessBarConfig` (`deny_unknown_fields`) and its
   `validate` (sizes finite > 0, `head_offset` finite ≥ 0, colour components finite in [0, 1]).

### Sim — code (`crates/gta_sim`)

7. **`src/combat/hitscan.rs:100`**: `fn unit_f32` → `pub(crate) fn unit_f32` (one uniform-float helper for civilian
   rolls). No other change in `combat/`.
8. **`src/flow/mod.rs`**: add `pub struct NpcSystems;` (`#[derive(SystemSet, Debug, Clone, PartialEq, Eq, Hash)]`, doc:
   "NPC gameplay; keeps running while the player is wasted") and in `FlowPlugin::build`:
   `.configure_sets(FixedUpdate, NpcSystems.run_if(in_state(GameState::Playing).or(in_state(GameState::Wasted))))`.
   Reason: during `Wasted` fixed ticks keep running (slow virtual time) and `drive_characters` keeps consuming
   `MoveIntent`; gating the AI with `PlayingSystems` would freeze decisions while bodies keep walking.
   Risk note in code comment only if a future `Paused`/`Busted` state is added: its readers must be added to this
   condition or `Messages<T>::clear()`ed on exit (TASK-006/007 lesson) — put this as the set's doc line.
9. **`src/navigation/mod.rs`** (new, ~170 lines):
   - `pub const NAVIGATION_CONFIG: &str = "npc/navigation.ron"`, `NavigationConfig`.
   - `#[derive(Resource)] pub struct SidewalkGraph { nodes: Vec<Vec3>, edges: Vec<(u32, u32)>, adjacency: Vec<Vec<u32>> }`.
     `pub fn new(nodes: Vec<Vec3>, edges: &[(u32, u32)]) -> Result<Self, String>`: an out-of-range index or a self-loop
     is an `Err` naming the edge (no silent drop); node ids are preserved; duplicate edges are kept once.
     `pub fn from_walk_graph(graph: &WalkGraph, y: f32) -> Result<Self, String>` (node (x, y2) → `Vec3(x, y, y2)`),
     `node(u32) -> Vec3`, `nodes() -> &[Vec3]`, `neighbors(u32) -> &[u32]`, `edges() -> &[(u32, u32)]`,
     `is_edge(a, b) -> bool`.
   - `#[derive(Component, Reflect, Clone, Copy, Debug, PartialEq)] #[reflect(Component)] pub struct GraphWalker { pub from: u32, pub to: u32 }`
     — invariant: `(from, to)` is an edge; the walker seeks `node(to)`.
   - Pure fns (horizontal `xz` distances everywhere):
     `pub fn wander_next(graph, from, to, u: f32) -> u32` — uniform over `neighbors(to)` without `from`; `from` only if it
     is the sole neighbour. `pub fn flee_next(graph, from, to, threat: Vec3) -> u32` — neighbour of `to` maximising
     distance to `threat`, ties → lower index. `pub fn flee_start(graph, walker, threat) -> GraphWalker` — keep `to` if
     `node(to)` is farther from the threat than `node(from)`, else swap. `pub fn steer(position: Vec3, target: Vec3) -> Option<f32>`
     = `aim_yaw(flat(target − position))`, `None` for a zero vector.
   - `NavigationPlugin`: `configure_sets(FixedUpdate, NpcSystems.run_if(resource_exists::<SidewalkGraph>))` (set
     conditions AND together); `OnTransition{exited: Loading, entered: Playing}` →
     `build_sidewalk_graph.run_if(resource_exists::<City>)`: `from_walk_graph(&city.0.sidewalks, params.0.roads.curb_height)`;
     on `Err` → `error!("sidewalk graph invalid: {e}")` + `MessageWriter<AppExit>` `AppExit::error()` (same policy as
     `apply_city_generation`); on `Ok` → `insert_resource`. Registers `GraphWalker`.
   - `#[cfg(test)]` unit tests: `new` rejects `(0, 0)` and `(0, 9)` on a 3-node graph with an error naming the edge;
     `wander_next` never returns `from` on degree ≥ 2, returns it on degree 1; `flee_next` worked example: `to` at origin,
     neighbours (10,0,0), (−10,0,0), (0,0,10), threat (8,0,1) → distances 2.24 / 18.03 / 12.04 → (−10,0,0);
     `flee_start` swaps when `from` is farther; **`steer` four directions** (checked against `aim_yaw` and
     `move_direction(Vec2::Y, yaw)`): target −Z → yaw 0, direction (0,0,−1); −X → yaw +π/2, (−1,0,0); +Z → yaw π,
     (0,0,+1); +X → yaw −π/2, (1,0,0) (tolerance 1e-5).
10. **`src/population/mod.rs`** (new, ~380 lines):
    - `pub const POPULATION_CONFIG`, `PopulationConfig` (Step 1).
    - View types:
      ```rust
      #[derive(Reflect, Clone, Copy, Debug, PartialEq)]
      pub struct ViewCone { pub origin: Vec3, pub forward: Vec3, pub half_angle: f32 }   // forward is unit length
      #[derive(Resource, Reflect, Default, Clone, Copy, Debug)] #[reflect(Resource)]
      pub struct CameraView(pub Option<ViewCone>);   // None: not published yet -> no spawning, no off-frame ageing
      ```
      `ViewCone::from_perspective(origin, forward: Dir3, fov_y, aspect)` with
      `half_angle = atan(√(tan²(fov_y/2) + (aspect·tan(fov_y/2))²))` (bounding cone of the frustum through its corner);
      `contains(&self, point, margin_rad) -> bool` = `point == origin || angle_between(point − origin, forward) ≤
      half_angle + margin_rad`.
      Unit tests: 70°, 16:9 → 55.00°; 55°, 16:9 → 46.72°; 90°, 1:1 → 54.74° (± 0.01°). Forward −Z from origin: (0,0,−10)
      in; (−5,0,−10) (26.6°) in; (10,0,0) (90°) out; (0,0,10) out; forward +Y: every point with y ≤ origin.y out.
    - `#[derive(Resource)] pub struct NpcRng(pub ChaCha8Rng)` = `seed_from_u64(seed)` then `set_stream(1)` (a different
      stream than `CombatRng`). `PopulationPlugin { pub seed: u64 }` (same seed as `CombatPlugin`).
    - Components: `#[derive(Component, Reflect, Default)] pub struct Offscreen(pub f32)` (seconds outside the view);
      `#[derive(Component, Reflect)] pub struct Corpse { pub age: f32 }`;
      `#[derive(Component, Reflect, Clone, Copy)] pub struct Appearance(pub u32)` (random u32 rolled at spawn; the client
      maps it to a model and a tint; the sim never names an asset).
    - `#[derive(Resource, Reflect, Default, PartialEq, Debug)] pub enum PopulationPhase { #[default] InitialFill, Steady }`.
    - `pub fn corpse_components() -> impl Bundle` = `(Dead, Corpse { age: 0.0 }, TnuaToggle::Disabled,
      RigidBody::Static, CollisionLayers::NONE)` (reused by T9/T11).
    - Two plain visibility functions: `pub fn outside_cone(view: &ViewCone, feet: Vec3, head_height: f32, margin_rad: f32) -> bool` = both
      `feet + 0.1·Y` and `feet + head_height·Y` are outside `contains(.., margin)`; `occluded(spatial, view, feet, head)` =
      World-mask rays (`SpatialQueryFilter::from_mask(GameLayer::World)`) from `view.origin` to both points are blocked
      (a hit shorter than the point distance).
    - Systems, set `PopulationSystems` (Step 13 ordering), chained `(age_corpses, despawn_far, spawn_civilians)`:
      - `age_corpses`: `age += dt`; `try_despawn` at `age ≥ corpse_seconds`; if more than `corpse_limit` corpses remain,
        `try_despawn` the oldest extras (sort ≤ 21 ages, oldest first).
      - `despawn_far`: `let Some(view) = view.0 else { return }` (no ageing without a known view). Per `Civilian`
        (alive or corpse): `Offscreen = 0` if `outside_cone(view, feet, head, 0)` is false, else `+= dt`; then
        `try_despawn` when horizontal distance to the player `> despawn_distance` **and** `Offscreen ≥
        despawn_offscreen_seconds`. Occlusion is ignored here (a civilian behind a building counts as in frame:
        conservative, never a pop-out).
      - `spawn_civilians`: `let Some(view) = view.0 else { return }`; `let Ok(player) = ...` (let-else, no player → return).
        Mode from `PopulationPhase`: `InitialFill` → radii `[initial_inner_radius, spawn_ring.1]`, budget
        `initial_spawns_per_tick`, acceptance `outside_cone(.., margin) || occluded(..)` (resolved Q2: outside the view
        cone **or** out of line of sight); `Steady` → radii `spawn_ring`, budget `spawns_per_tick`, acceptance
        `outside_cone(.., margin)` only (GDD §6.1). Deficit = `max_civilians − alive civilians` (corpses excluded); if 0,
        `InitialFill` → `Steady`, return. Candidates = graph nodes with horizontal distance to the player in the mode's
        radii whose node position is ≥ `spawn_min_separation` from every `Civilian` (alive or corpse) — O(nodes +
        civilians·nearby) per tick only while under the cap (579 nodes: microseconds). Shuffle candidates with `NpcRng`
        (Fisher-Yates via `unit_f32`), test acceptance in order (occlusion rays only for candidates inside the cone),
        spawn up to `min(budget, deficit)` accepted ones, re-checking separation against the civilians spawned this tick.
        Each spawn: `civilian_bundle(.., GraphWalker { from: node, to: wander_next-chosen neighbour }, t = 0, ..)` with a
        rolled `Temperament` and `Appearance(rng.next_u32())`. After an `InitialFill` tick, switch to `Steady` if fewer
        than the budget were spawned (no eligible node left) or the cap is reached.
    - `PopulationPlugin::build`: `init_resource::<CameraView>()`, `init_resource::<PopulationPhase>()`, `insert_resource(NpcRng)`,
      registers `CameraView`, `ViewCone`, `Offscreen`, `Corpse`, `Appearance`, `PopulationPhase`.
11. **`src/perception/mod.rs`** (new, ~300 lines):
    - `PERCEPTION_CONFIG`, `PerceptionConfig` (Step 2).
    - `#[derive(Component, Reflect, Default)] #[reflect(Component)] pub struct Perception { pub slot: u8, pub pending: Option<Threat> }`;
      `#[derive(Reflect, Clone, Copy, Debug, PartialEq)] pub struct Threat { pub kind: ThreatKind, pub at: Vec3, pub distance: f32 }`;
      `#[derive(Reflect, Clone, Copy, Debug, PartialEq, Eq)] pub enum ThreatKind { Gunshot, Fight, Corpse, Aimed, Hurt }`.
    - `#[derive(Resource, Reflect, Default)] pub struct AiClock { pub tick: u64 }`; private `SlotCursor(u8)`; observer
      `On<Add, Perception>`: `slot = cursor; cursor = (cursor + 1) % slots` (immediate reaction to an insert: an observer).
    - `#[derive(Resource, Default)] pub struct StimulusLog(Vec<(u64, ThreatKind, Vec3)>)`.
    - `#[derive(Resource, Reflect, Default, Clone, Copy, Debug)] #[reflect(Resource)] pub struct PerceptionLoad { pub agents: u32, pub rays: u32 }`
      — work done by `perceive` in the current tick (reset each tick); read by the bench's bounded-work assertion and by
      QA over BRP.
    - `#[derive(SystemSet)] pub enum AiSystems { Perceive, Decide }`.
    - Systems in `AiSystems::Perceive`, one flat `.chain()`: `advance_clock` (`tick += 1`); `collect_stimuli` (drop entries
      with `tick_then + slots ≤ tick`; push every `ShotFired` as `Gunshot` at `muzzle` and every `MeleeHit` as `Fight` at
      `point`; for every `DamageDealt` whose target has `Perception` and a `Civilian` state other than `Dead`, write
      `pending = Hurt` at the shooter's `Position` (fallback `point`), distance 0 — the unsliced path); `perceive` (for
      `Perception` with `tick % slots == slot` and state ≠ `Dead`: the nearest of: log entries within `hearing_radius`
      (gunshot) / `fight_hearing_radius` (fight); the **nearest** `Corpse` within `corpse_sight`, one LOS ray; each live
      `(AimIntent, Loadout)` holder other than self with `held.is_some() && aiming`, within `aimed_distance`, angle between
      `aim.direction` and `chest − aim.origin` ≤ `aimed_cone_deg`, one LOS ray; a pending `Hurt` is kept (highest
      priority). LOS = `SpatialQuery::cast_ray` from eyes (feet + `LocomotionConfig.head_height`) to the target chest,
      `SpatialQueryFilter::from_mask(GameLayer::World)`, blocked if a hit is shorter than the target distance. Writes
      `PerceptionLoad { agents, rays }`.
    - `PerceptionPlugin`: `init_resource` for `AiClock`, `SlotCursor`, `StimulusLog`, `PerceptionLoad`; the observer;
      registers `Perception`, `Threat`, `ThreatKind`, `AiClock`, `PerceptionLoad`.
12. **`src/civilian/mod.rs`** (new, ~340 lines) + **`src/civilian/reaction.rs`** (new, ~160 lines incl. tests):
    - `CIVILIAN_CONFIG`, `CivilianConfig`, `ReactionConfig` (Step 4).
    - `#[derive(Component, Reflect)] #[reflect(Component)] #[require(Character, Perception, Offscreen)]
      pub struct Civilian { pub state: CivilianState, pub temperament: Temperament }`.
      `#[derive(Reflect, Clone, Copy, Debug, PartialEq)] pub enum CivilianState { Wander, Idle { left: f32 },
      Flee { from: Vec3, left: f32 }, Cower { from: Vec3, left: f32 }, Report { progress: f32 }, Dead }` —
      `Report.progress` goes 0 → 1 over `call_seconds`, so the client draws the bar without reading sim config.
      `#[derive(Reflect, Clone, Copy, Debug, PartialEq)] pub struct Temperament { pub flee: f32, pub cower: f32, pub report: f32 }`.
    - `pub fn civilian_bundle(loco: &LocomotionConfig, handle, health: &HealthConfig, graph: &SidewalkGraph,
      walker: GraphWalker, t: f32, temperament: Temperament, appearance: Appearance) -> impl Bundle` = `(Civilian { state:
      Wander, temperament }, walker, appearance, Name::new("Civilian"), Transform at lerp(node(from), node(to), t) +
      Y·float_height, character_components(loco, handle), Health::full(health))` — used by the spawner and every test.
      `pub fn roll_temperament(rng, spread) -> Temperament` (each factor `1 + spread·(2u − 1)`).
    - `reaction.rs`: `pub enum Reaction { Flee, Cower, Report }`; `pub fn choose_reaction(threat: &Threat, t: &Temperament,
      cfg: &ReactionConfig, allow_report: bool) -> Reaction`: `p = clamp(1 − d / panic_distance, 0, 1)`;
      `flee = w.flee·t.flee`; `cower = w.cower·t.cower·p`; `report = w.report·t.report` only if `allow_report` and
      (`Corpse` at any distance, or `Gunshot`/`Fight` with `d ≥ report_min_distance`), never for `Aimed`/`Hurt`; else 0.
      Max wins, ties Flee > Cower > Report. No RNG inside.
    - `civilian_fsm` (`AiSystems::Decide`): per `Civilian` with state ≠ `Dead` (check the **state field**, not only a
      `Without<Dead>` filter), take `perception.pending`. Transitions:
      calm (`Wander`/`Idle`) + threat → `choose_reaction(.., allow_report = true)` → `Flee { from: at, left: roll(flee_distance) }`
      (and `flee_start`) / `Cower { from: at, left: roll(cower_seconds) }` / `Report { progress: 0 }`;
      `Report` + threat → `choose_reaction(.., allow_report = false)` (call interrupted, GDD §6.2);
      `Flee`/`Cower` + threat → refresh `from` and restart that state's distance/timer, no re-scoring (commitment);
      `Flee.left -= horizontal |LinearVelocity|·dt`, ≤ 0 → `Wander`; `Cower.left -= dt`, ≤ 0 → `Flee` from the same point;
      `Idle.left -= dt`, ≤ 0 → `Wander`; `Report.progress += dt / call_seconds`, ≥ 1 → `Wander` (T10 turns a completed
      call into heat). Arrival at `node(to)` within `arrive_radius` advances `GraphWalker` (`wander_next` for Wander with an
      `idle_chance` roll → `Idle { left: roll(idle_seconds) }`; `flee_next` for Flee). Output: `Wander` →
      `MoveIntent { axis: Vec2::Y, yaw: steer(..), gait: wander_gait }`, `Flee` → same with `flee_gait`, others →
      `axis = Vec2::ZERO`. `dt` = `Time<Fixed>` timestep. One helper fn per state returning the next state; the system only
      dispatches (Golden Path). No vehicle stimulus (needs T14; noted for T14).
    - `civilian_death` (`HealthSystems::Death`, `NpcSystems`): `Query<(Entity, &Health, &mut Civilian, &mut Perception, &mut MoveIntent)>`
      with state ≠ `Dead` and `current ≤ 0` → `state = Dead`, `pending = None`, `axis = ZERO`,
      `commands.entity(e).insert(corpse_components())`.
    - `CivilianPlugin`: registers `Civilian`, `CivilianState`, `Temperament`.
13. **Scheduling (FixedUpdate), written in the plugins above:**
    - `PerceptionPlugin`: `configure_sets(FixedUpdate, (AiSystems::Perceive, AiSystems::Decide, PopulationSystems).chain()
      .in_set(NpcSystems).after(HealthSystems::Death))` and `configure_sets(FixedUpdate, AiSystems::Decide.before(TnuaUserControlsSystems))`.
      Resulting order in one tick: combat writers (`HealthSystems::Damage`: shots, hits, `DamageDealt`) → Regen → Pickup →
      `HealthSystems::Death` (`civilian_death`, `dummy_life`, `detect_player_death`) → Perceive → Decide → population →
      and Decide before `drive_characters`. `PopulationSystems` is declared in `population/`.
    - Consequence: all Health sets now run before `TnuaUserControlsSystems` (previously unordered with it). This only
      removes an ambiguity; the full existing suite must stay green (Step 20).
    - A fatal hit and its `DamageDealt` in the same tick: `civilian_death` sets `state = Dead` first, so the `Hurt` write
      is skipped and the FSM ignores the body — death wins.
14. **`src/lib.rs`**: `pub mod civilian; pub mod navigation; pub mod perception; pub mod population;`. In `compose_sim`
    load + `validate` the four configs exactly like the existing ones (`load_config` + `ConfigError { path, message }`)
    and insert them; append `NavigationPlugin, PerceptionPlugin, PopulationPlugin { seed: combat_seed },
    CivilianPlugin` to the `add_plugins` tuple after `CombatPlugin` (tuple grows 8 → 12, within the 15-entry tuple impl,
    `bevy_app-0.19.1/src/plugin.rs:186-192`). Check: `cargo build`; `cargo test -p gta_sim` (existing tests stay green:
    the test area has no `SidewalkGraph`, and in city tests `CameraView` stays `None`, so nothing spawns).

### Sim — gates (`cargo test -p gta_sim`)

All integration tests use the production composition (`composed_app` / `headless_app` / `city_app`), production
`civilian_bundle`, configs read from the resources (never retyped) and `run_ticks` / `Shots` (count ticks via
`Time<Fixed>`; the first `update()` can run 0 ticks). Allowed test-side mutations, each named in its test: set
`CameraView` (synthetic view), set `PopulationPhase`, set `max_civilians = 0` to idle the spawner, set a civilian's
state/`Health` for a focused case. Expected tick numbers below are derived from the shipped data; each test derives
them from the resources and panics "GATE BROKEN" if a precondition (distance vs radius, etc.) does not hold. Record the
flip-RED of each new gate (perturbed input, RED seen, GREEN restored) in IMPL_SUMMARY.

15. **`tests/config.rs`**: shipped-load tests for the four new files; `population.ron` unknown field → error names
    file and field; range fixtures strictly on the failing side: `spawn_ring: (130.0, 120.0)`, `initial_inner_radius:
    70.0` (> ring inner 60), `slots: 0`, `temperament_spread: 1.5`. Each fixture's error must mention its own field
    name (unique keywords: `spawn_ring`, `initial_inner_radius`, `slots`, `temperament_spread`).
16. **`src/civilian/reaction.rs` `#[cfg(test)]` worked table** with the shipped `civilian.ron` (`include_str!`, parsed
    with `ron`, "GATE BROKEN" on parse error; `p = clamp(1 − d/15, 0, 1)`):
    | # | kind, d | t (flee, cower, report) | flee / cower / report | expected |
    |---|---|---|---|---|
    | 1 | Gunshot 20 | 1, 1, 1 | 1.0 / 0 / 0 (20 < 25) | Flee |
    | 2 | Gunshot 2 | 1, 1, 1 | 1.0 / 1.3 / 0 | Cower |
    | 3 | Gunshot 10 | 0.5, 1.5, 1 | 0.5 / 0.75 / 0 | Cower |
    | 4 | Gunshot 30 | 0.7, 1, 1.5 | 0.7 / 0 / 1.2 | Report |
    | 5 | Gunshot 30 | 1, 1, 1 | 1.0 / 0 / 0.8 | Flee |
    | 6 | Corpse 10 | 0.6, 1, 1.4 | 0.6 / 0.5 / 1.12 | Report |
    | 7 | Hurt 0 | 1, 1, 1.5 | 1.0 / 1.5 / 0 | Cower |
    | 8 | Aimed 6 | 1, 1, 1.5 | 1.0 / 0.9 / 0 | Flee |
    | 9 | Gunshot 30, `allow_report = false` | 0.7, 1, 1.5 | 0.7 / 0 / 0 | Flee |
    No row sits on a tie. Plus `roll_temperament` stays in `[1 − s, 1 + s]` over 1000 rolls.
17. **`tests/civilians.rs`** (new, test floor; helpers in `tests/common/mod.rs`: `test_graph(app, nodes, edges)` =
    `insert_resource(SidewalkGraph::new(..).unwrap())`, `spawn_civilian(app, walker, t, temperament) -> Entity` via
    `civilian_bundle`, `civilian_state(app, e)`, `set_view(app, Option<ViewCone>)`):
    - **`gunshot_at_20m_flee_or_cower_within_one_cycle`** (acceptance). `max_civilians = 0`; graph = square loop with
      corners (±20, 0, ±20) plus a far segment (−34,0,−30)→(−26,0,−30). Player settled at the origin with a pistol
      (`set_loadout`), aim straight up (`set_aim(eye, eye + Y)`). Four civilians at the side midpoints (±20,0,0),
      (0,0,±20) spawned consecutively ⇒ slots 0..3; control at (−30,0,−30). Preconditions: muzzle-to-civilian < `hearing_radius`
      and < `report_min_distance`; control > `hearing_radius`. Run 16 ticks: all five `Wander`. `fire_requested`, find the
      tick T of the `ShotFired` via `Shots`, then step one tick at a time recording each civilian's first non-calm tick.
      Assert: each of the four is `Flee` or `Cower` by T + slots − 1; their reaction ticks are all distinct and in
      [T, T + slots − 1]; the control is still `Wander` after 2·slots more ticks. Flip-RED: (a) `perceive` ignores the
      slot → distinct-ticks RED; (b) log retention 1 tick → three civilians never react → RED; (c) hearing-radius check
      removed → control RED.
    - **`hurt_reacts_same_tick`**: real pistol shot into a civilian's chest at 10 m; in the tick of the first non-lethal
      `DamageDealt` its state is `Flee` or `Cower`, never `Report`. Flip: route `Hurt` through the sliced log → RED for
      three of four slot phases; the test picks the civilian whose slot ≠ T % slots (derive from `AiClock`).
    - **`fatal_hit_kills_in_the_same_tick`** (damage path): fire at the civilian's head at 5 m until a `DamageDealt`
      with `killed == true` (bounded by the magazine; GATE BROKEN if none); in that tick: `state == Dead`, `Corpse` and
      `TnuaToggle::Disabled` present, `pending == None`. Flip: order `civilian_death` after `AiSystems::Decide` →
      state is `Flee`/`Cower` in the kill tick → RED.
    - **`report_is_interrupted_by_a_new_threat`**: temperament (0.7, 1, 1.5), shot at 30 m (between
      `report_min_distance` and `hearing_radius`) → `Report` (row 4); a second shot at 30 m before `call_seconds` → `Flee`
      (row 9). Flip: `allow_report` always true → stays `Report` → RED.
    - **`report_completes_after_call_seconds`**: same setup, one shot; `progress` rises by `1/(64·call_seconds)` per
      tick (1/256, exact in binary) and the state returns to `Wander` exactly `64·call_seconds` = 256 ticks after the
      reaction tick, not one earlier. Flip: complete at `progress > 1` instead of `≥ 1` → one tick late → RED.
    - **`death_makes_a_corpse`** (focused conversion): set `Health.current = 0`; next tick: `state == Dead`; `Dead`,
      `Corpse`, `TnuaToggle::Disabled`, `RigidBody::Static`, `CollisionLayers::NONE` present; position unchanged after 64
      ticks; a chest ray with the hitscan mask `[World, Character, Hitbox]` no longer hits it. Flip: drop
      `CollisionLayers::NONE` → ray RED.
    - **`corpse_limit_and_lifetime`**: `corpse_limit + 1` civilians on the square, kill one per tick; one tick after
      the last kill exactly `corpse_limit` corpses remain and the first-killed entity is gone; the second-killed corpse is
      gone exactly `corpse_seconds × 64` = 1920 ticks after its death tick (age += 1/64 is exact), present one tick before.
      Flip: sort youngest-first → the wrong entity survives → RED.
18. **`tests/civilian_city.rs`** (new, city seed 1 — real graph, curbs, crossings, buildings):
    - **`graph_matches_city`**: `SidewalkGraph.edges().len()` equals the city's edge count (1134 for seed 1, read from
      `City`, not hard-coded) and every city edge is `is_edge` both ways.
    - **`wander_stays_on_the_graph`** (acceptance). `max_civilians = 0`; view `None`. 12 civilians via `civilian_bundle` on
      edges within 40 m of the player spawn, ≥ 6 m apart (worst case 40 + 30 s × 1.8 m/s = 94 m < 150 m: no despawn,
      and despawn is off anyway with no view). Run 1920 ticks (30 s); every 8 ticks for every civilian: state ∈ {Wander,
      Idle}; `graph.is_edge(from, to)`; horizontal distance from its feet to segment `node(from)`–`node(to)` ≤
      `street.sidewalk / 2` (= 1.5 m, read from `city.ron`, the narrowest sidewalk half-width, > `arrive_radius`).
      Liveness: every civilian changed `to` ≥ 2 times and travelled ≥ 20 m. Flip: steer toward `node(to) + (3,0,0)` →
      distance RED; `wander_next` returning any node → `is_edge` RED.
    - **`no_spawn_before_the_view_is_known`**: shipped config, `CameraView(None)`; run 128 ticks → 0 civilians. Then set a
      synthetic view → ≥ 1 civilian after 2 ticks. Flip: treat `None` as "nothing visible" → civilians in the first phase → RED.
    - **`initial_fill_closer_than_the_ring`** (resolved Q2). Synthetic view: origin = player eye + 4 m back (+Z),
      forward −Z, `from_perspective(70°, 16/9)`. Run 8 ticks. Assert: count > 0; every civilian stands on a graph node
      (distance to its node ≤ 1e-3 on the first tick, recorded via `Added<Civilian>`); distance to the player in
      `[initial_inner_radius, spawn_ring.1]`; for each: outside the cone + margin, **or** both World rays from the view
      origin to its feet+0.1 and head blocked (the test casts its own rays with `SpatialQuery`); pairwise ≥
      `spawn_min_separation`; **at least one civilian closer than `spawn_ring.0`** (precondition GATE BROKEN if the
      seed-1 graph has no node in `[initial_inner_radius, spawn_ring.0)` outside the cone — probe: 4). Flip: initial
      radius = `spawn_ring.0` in the spawner → the "closer than the ring" assertion RED; skip the hidden check → some
      spawn visible → RED (precondition: at least one node in range lies inside the cone and unoccluded).
    - **`steady_spawns_in_the_ring_off_frame`**: set `PopulationPhase::Steady` before the view; same synthetic view;
      run 64 ticks. Every spawn on a node, in `spawn_ring`, outside cone + margin (no occlusion allowance), separated.
      Precondition: at least one ring node is inside the cone (probe at spawn: 16 in ring, 14 outside a 120° wedge).
      Flip: skip the cone test → the in-cone ring nodes spawn (deficit 40 > 16 candidates, every candidate spawns within
      16 ticks) → RED.
    - **`despawn_after_2s_offscreen_beyond_150m`** (acceptance). `max_civilians = 0`. View: forward `+Y` (nothing on the
      ground in frame). Civilians at graph points: A at ~160 m, B at 130-145 m, C at > 150 m; all `Idle { left: 1e6 }`
      set before the first tick. Then point the view at C (`from_perspective(eye, C − eye, 70°, 16/9)`) from the start.
      A present after `2.0 × 64 − 1 = 127` ticks, gone after 128 (1/64 exact in binary); B and C present after 256; then
      turn the view to `+Y` → C gone exactly 128 ticks later, not earlier. Flip: `||` instead of `&&` → B RED; no
      `Offscreen` reset in view → C RED.
    - **`npcs_live_through_wasted`**: one wandering civilian; `write_damage(1000)` → `Wasted`; during the slow-mo the graph
      invariant holds and `GraphWalker.to` still advances. Flip: put the AI sets in `PlayingSystems` → it walks straight
      off → RED.
19. **`tests/civilian_bench.rs`** (new, own test binary; not `#[ignore]`): city seed 1, `max_civilians = 64`, synthetic
    view (forward +Y). Place 64 civilians via `civilian_bundle` on graph edges within 100 m of the player (≥ 3 m apart),
    then kill 4 (`Health = 0`) so corpses exist and the spawner works (60 alive + refill). Run 64 warm-up ticks, then
    **exactly 640 observed fixed ticks** (`run_ticks(app, 1)` per sample, `Time<Fixed>`-counted); every 64 ticks write a
    `ShotFired` at a civilian's position. Print mean / p50 / p95 / max tick time, min/max alive, and max
    `PerceptionLoad`. Also run the same with 32 civilians and print both means (comparison only, no assertion).
    Hard assertions: (1) **mean < 8 ms** — derived: probe seed-1 mean with 64 Tnua walkers 0.756 ms × 10 ("на порядок")
    ≈ 7.6 → 8 ms; catches a hang and O(N²) work costing more than ≈ 1.8 µs per pair; it does **not** prove linearity
    (4096 rays of 0.6 µs ≈ 2.4 ms passes). (2) **bounded work**, every tick: `PerceptionLoad.agents ≤ ceil(alive / slots)
    + 1` and `rays ≤ agents × (1 + aimers)` with aimers = live `(AimIntent.aiming, Loadout.held)` holders (0 here) —
    this, with the distinct-ticks assertion of Step 17, carries the algorithmic claim. Max is not asserted (OS spikes).
    Flip-RED: (a) `std::thread::sleep(10 ms)` in `civilian_fsm` → mean RED; (b) in `perceive` cast a ray from every
    agent to every other civilian → `rays` bound RED; (c) ignore the slot → `agents` bound RED. Record the real mean in
    IMPL_SUMMARY; a mean above 2 ms is a finding to investigate even though it passes.

### Client (`gta_like`)

20. **`src/camera/mod.rs`**: new `publish_camera_view` in `PostUpdate`, `.after(follow_player)`,
    `.run_if(any_with_component::<Player>)` (no player → the camera pose is meaningless → keep the last value; `None`
    until the first publish). Params: `Single<(&Transform, &Projection), With<OrbitCamera>>`, `ResMut<CameraView>`.
    For `Projection::Perspective(p)`: `CameraView(Some(ViewCone::from_perspective(t.translation, Dir3 of t.rotation *
    NEG_Z, p.fov, p.aspect_ratio)))` (final rotation incl. recoil/shake; the cone is conservative). Register in
    `CameraPlugin::build`. Do **not** try to publish before the first fixed tick of `Playing`: the player is spawned in
    `StateTransition`, after `PreUpdate`, and the camera pose exists only after `follow_player`. The initial fill runs on
    the first fixed tick after this publish (one frame after the transition frame; invisible).
21. **`src/visuals/character_config.rs`**: fields `civilian_models: Vec<String>`, `civilian_tints: Vec<(f32, f32, f32)>`,
    `death: String`, `cower: String`. `CharacterClips` gains `death: usize`, `cower: usize`. `resolve`: resolves `death`
    and `cower` against the rig; for **every** civilian model: it has a rig in the manifest, its `tinted_mesh` is a
    skinned mesh of that rig, the arm/hand joints exist, and resolving all clip names against its rig yields exactly the
    player's `CharacterClips` (else error "civilian model {m} resolves clips differently from {model}"). `validate`:
    `civilian_models` and `civilian_tints` non-empty, tint components finite ≥ 0. **`src/main.rs` `preflight`**: every
    civilian model must be `manifest.contains_asset` (same message format as the player model).
22. **`src/visuals/character.rs`** (per-model graphs; target ≤ 650 lines):
    - `CharacterAnimations`: replace `graph: Handle<AnimationGraph>` with `graphs: Vec<Handle<AnimationGraph>>` and add
      `scenes: Vec<Handle<WorldAsset>>`, both indexed by `ModelKey`: key 0 = `config.model` (player, dummies), keys
      1..=n = `civilian_models`. `from_world` builds each graph with **the same function in the same order** from that
      model's own `GltfAssetLabel::Animation(i).from_asset(model)` clips, so node indices (`nodes`, `legs`, `hold`,
      `shoot`, `fists`, `bat`, `knockdown`, `rest`, new `death`, `cower`) are identical across graphs and stay single
      fields; `debug_assert_eq!` that each build returns the same indices. `fist_clips`/`bat_clip` stay from key 0 (only
      the player swings). `scenes[k]` = `GltfAssetLabel::Scene(0).from_asset(model_k)` loaded here (strong handles:
      civilian GLBs load under the loading screen, so the initial fill is not delayed by asset IO).
    - `#[derive(Component, Clone, Copy, Debug, PartialEq, Eq)] pub(super) struct ModelKey(pub(super) usize)` on the
      `CharacterModel` entity and on its `CharacterAnimator`.
    - `spawn_character_model`: add `civilians: Query<&Appearance, With<Civilian>>` (the body is spawned with
      `Civilian`/`Appearance` in one bundle, so both are present when `On<Add, CharacterBody>` fires). Key = `1 +
      appearance % n` for a civilian, else 0. Spawn `(CharacterModel, ModelKey(key), WorldAssetRoot(scenes[key].clone()),
      transform)`. One `scale()` for every model (identical skeleton, measured).
    - `on_model_ready`: read `ModelKey` of `ready.entity`; `wire_player` inserts `AnimationGraphHandle(graphs[key])` and
      the key; `assign_mask_groups(&animations.graphs[key], ..)` (mask groups live in the per-model graph asset: the
      TASK-007 lesson). Tint: civilians use `civilian_tints[(appearance / n) % len]`, others `config.tint`; pass the tint
      into `tint_mesh` instead of reading `config.tint` inside.
    - `ShownAction` gains `Death` and `Cower`. `drive_character_animation` adds `Has<Dead>` and `Option<&Civilian>` to the
      character query; priority `Death` (Dead) > `Knockdown` > `Cower` (`Civilian.state` is `Cower`) > `Swing`.
      `Death` plays once (no `.repeat()`), except when switching from `Knockdown` while `clips.death == clips.knockdown`:
      then keep the playing node and only set `animator.action = Death` (no visible re-fall). `Cower` loops. Both skip
      locomotion like the existing actions; on `Dead` removal (dummy revive, player respawn) the existing `action_ended`
      path returns to locomotion and the `rest` layer resets unkeyed joints. This also gives the player and dummies a
      death clip (intended, GDD §4.3 A).
23. **`src/hud/witness.rs`** (new, ~120 lines) + `WitnessBarPlugin` added by `HudPlugin`: one UI node (back + fill child)
    per civilian in `Report`, keyed by a `WitnessBar { civilian: Entity }` component. `Update`: spawn a bar when a civilian
    is in `Report` and has none; set fill width = `width × progress.clamp(0, 1)`; despawn the bar when its civilian is
    gone or not in `Report` (interruption, death, completion, despawn). `PostUpdate` placement
    `.after(follow_player).after(CameraUpdateSystems).before(UiSystems::Layout)` (same slots as `place_damage_numbers`):
    project `feet + (model head top + head_offset)·Y` with `Camera::world_to_viewport`; let-else → hide. Pure fn
    `bar_fill_width(progress, width) -> f32`. Sizes/colours from `witness_bar` (Step 6).

### Client — gates (`cargo test -p gta_like --bin gta_like`)

`src/visuals/character_gate.rs` is 697 lines; new gates go to **`src/visuals/civilian_gate.rs`** (`#[cfg(test)] mod`
in `visuals/mod.rs`) and **`src/hud/witness_gate.rs`**.

24. **`civilian_gate.rs`**:
    - **`every_civilian_model_animates_from_its_own_clips`** (resolved Q1 gate, real GLBs). App: `MinimalPlugins,
      TransformPlugin, AssetPlugin { file_path: <CARGO_MANIFEST_DIR>/assets }, StatesPlugin, ImagePlugin::default(),
      MeshPlugin, AnimationPlugin, WorldSerializationPlugin, GltfPlugin::default()`, `init_asset::<StandardMaterial>()`,
      `TimeUpdateStrategy::FixedTimesteps(1)`, `compose_sim(TestArea)`, shipped `visual.ron` + clips,
      `CharacterVisualsPlugin` (do **not** also `init_asset` the types the real plugins register). A helper checks every
      configured GLB exists and panics "GATE BROKEN: missing third-party asset …; run python tools/fetch_assets.py".
      Insert a test `SidewalkGraph` (straight 40 m loop), spawn one civilian per configured model with `Appearance`
      chosen so `1 + appearance % n` hits each key, all wandering. Update until every `CharacterModel` has a wired
      `CharacterAnimator` (wall-clock deadline 30 s, 1 ms sleep, asset IO is async); then sample `leg-left` rotation of
      each model (walk up `ChildOf` to the `CharacterModel`), run 32 more updates (0.5 s virtual), sample again. Assert
      per model: rotation angle change > 0.1 rad (probe: 1.18 rad own clips) and the animator's graph handle is
      `graphs[key]`. Flip-RED: wire every animator with `graphs[0]` → non-male-a models change 0.0 rad → RED.
    - **`graph_clips_come_from_their_own_model`** (cheap structure): for each key, every clip node of `graphs[key]` has
      a handle path `<model_k>#Animation{i}` and `scenes[key]` path is `<model_k>#Scene0`; node indices equal across keys.
    - **`dead_and_cower_select_their_nodes`**: in the existing stand-in harness pattern (copy the harness helper into
      this file, not a new composition): a character with `Dead` inserted gets the `death` node active and no locomotion
      node started after it; a civilian in `Cower` gets `cower`; back to `Wander` returns to locomotion. Assert
      `animation(node).is_some_and(..)` on the expected node (not `all_paused()`, TASK-008 lesson). Flip: drop the
      `Has<Dead>` branch → RED.
    - **`civilian_models_are_validated`**: `resolve` rejects a civilian model outside the manifest rig, and
      `validate` rejects an empty `civilian_models` (fixtures strictly on the failing side, each error names its field).
25. **`src/hud/witness_gate.rs`**: `bar_fill_width` worked values (0 → 0, 0.5 → 24, 1.2 → 48 with width 48); lifecycle
    in a headless app (`compose_sim` + `WitnessBarPlugin` + shipped `UiConfig`): a civilian set to `Report { progress:
    0.5 }` gets exactly one bar with fill 24 px after one update; switched to `Flee` → the bar is gone next update;
    despawned civilian → bar gone. Placement is not asserted (no render target headless: owner run). Flip: skip the
    despawn branch → RED.

### Runtime QA and owner run

26. **`tools/qa/scenarios/t8.py`** (new, stdlib, imports `brp`, `t5`, `t6` helpers like `t7.py`):
    1. `Game(features=("dev",), args=("--seed", "1"), release=True)`; `wait_resource("CityLayoutHash")` equals the
       golden; `wait_chunks`.
    2. First fill: ≤ 1 s after `GameState` becomes `Playing`, read `Civilian` + `Position` rows and the player; record
       the count and how many are closer than `spawn_ring.0` (read from `population.ron`); screenshot `first_fill.png`.
       Hard pass: at least one civilian closer than `spawn_ring.0` within 1 s.
    3. Poll until ≥ 40 alive civilians (state ≠ Dead), deadline 120 s; record time to 40; `diagnostics()` at that moment
       ("get_diagnostics при 40 мирных": FPS + frame time into the summary); counts per state and per model
       (`Appearance` → `1 + a % n` with `n` from `visual.ron`), must cover ≥ 2 models; screenshot `crowd.png`.
    4. Pistol: t6 pickup flow until `Loadout.held == Pistol`.
    5. Pick the civilian with the most live civilians within `hearing_radius` (read from `perception.ron`); teleport the
       player 1.5 m from it toward that group's centroid; wait 0.5 s.
    6. Count states among civilians within `hearing_radius` → `before`. Aim into the sky: `move_mouse(0, −(60 /
       sensitivity_deg))`, `send_mouse_button("Left", 80)`; confirm the magazine dropped by one. Wait 0.3 s (a
       perception cycle is 62.5 ms plus BRP latency); count again → `after`; screenshot `scatter.png` (≥ 0.15 s after the
       previous capture, TASK-007 lesson). Also read `PerceptionLoad` once.
    7. Hard pass: ≥ 1 civilian in hearing range; share(Flee + Cower) after > before; step 2 passed; zero `ERROR` lines
       (`log_errors`). Summary JSON: counts per state before/after, per-model counts, FPS at 40, time to 40, first-fill
       count, screenshots (evidence, not verdicts).
27. **Owner checklist** (QA writes it into QA_REPORT.md): "улицы живые" (people visible within the first seconds after
    loading, they walk the sidewalks, stop, cross roads, no crowd stuck on a corner), "выстрел в воздух разгоняет толпу"
    (nearby civilians run or crouch, none stands still), several distinct models and no T-pose, death clip and a body
    lying 30 s, crouch reads as cowering, witness bar fills over ~4 s and vanishes when the caller is scared or killed,
    FPS with 40 civilians. Feel values live in `civilian.ron` / `population.ron` / `visual.ron` / `strings.ron`.

### Order and checks

28. Steps 1-4 + 15 → `cargo test -p gta_sim --test config`. Steps 7-14 → `cargo build`, `cargo clippy -- -D warnings`,
    `cargo clippy -p gta_sim --tests -- -D warnings`. Steps 9, 10, 16 unit tests → `cargo test -p gta_sim --lib`.
    Steps 17-19 → `cargo test -p gta_sim` (full; every existing test green). Steps 5-6, 20-25 →
    `cargo test -p gta_like --bin gta_like`. Then `python tools/qa/tree_check.py` and
    `cargo tree -p gta_sim -e normal -i bevy_render` (must stay empty). `cargo test -p citygen` is unchanged (citygen not
    touched; run it once anyway). Step 26 last, on the release build.

## 3. Test plan

| Claim | Gate | Class | Expected |
|---|---|---|---|
| Wander stays on the graph (acceptance) | `wander_stays_on_the_graph` | correctness + liveness | every sample on an edge, ≤ 1.5 m off; ≥ 2 node changes, ≥ 20 m each |
| Shot at 20 m → Flee/Cower within one cycle (acceptance) | `gunshot_at_20m_flee_or_cower_within_one_cycle` | correctness | 4 civilians react in 4 distinct ticks within [T, T+3]; control calm |
| Despawn beyond 150 m after 2 s off-frame (acceptance) | `despawn_after_2s_offscreen_beyond_150m` | correctness | A gone at tick 128 not 127; B stays; C stays in view, gone 128 ticks after the view turns |
| 64 NPC × 640 ticks bench (acceptance) | `civilian_bench` | liveness + coarse perf + bounded work | mean < 8 ms printed with p50/p95/max; agents ≤ ceil(N/4)+1, rays ≤ agents per tick |
| Initial fill closer than 60 m, hidden from the camera (Q2) | `initial_fill_closer_than_the_ring`, `no_spawn_before_the_view_is_known`, QA step 2 | correctness | ≥ 1 civilian < 60 m, all hidden, none before a view |
| Steady spawns on nodes in the ring off-frame | `steady_spawns_in_the_ring_off_frame` | correctness | every spawn at a node, 60-120 m, outside cone+margin |
| Each model animates from its own clips (Q1) | `every_civilian_model_animates_from_its_own_clips` | correctness (real GLB) | leg rotation change > 0.1 rad per model |
| Close witnesses flee, not call (Q3) | reaction table row 1, gunshot test | correctness | Flee at 20 m |
| Report interrupted / completes | `report_is_interrupted_by_a_new_threat`, `report_completes_after_call_seconds`, witness gate | correctness | Flee after 2nd shot; Wander at +256 ticks; bar removed |
| Death via damage wins same tick; corpse inert | `fatal_hit_kills_in_the_same_tick`, `death_makes_a_corpse`, `corpse_limit_and_lifetime` | correctness | Dead in kill tick; ray misses; 20 corpses max; gone at +1920 ticks |
| NPCs live during Wasted | `npcs_live_through_wasted` | correctness | walker advances on graph |
| Config strictness | `tests/config.rs` fixtures | correctness | each fixture errors naming its field |
| Tuning in data | review of the diff: no new `const` holding a tuning value | review | only laws (`MODEL_YAW`, layers) as `const` |
| Street life, crowd scatter, looks, FPS at 40 | QA `t8.py` + owner checklist | owner | recorded in QA_REPORT.md |

Every new gate is flipped RED by the perturbation named in its step and restored GREEN; IMPL_SUMMARY records the
perturbed input for each.

## 4. Rollout notes

- No migration, no save data, no env var, no feature flag, no new dependency. Four new data files under `assets/npc/`;
  new fields in `assets/character/visual.ron` and `assets/ui/strings.ron` (strict loaders: a stale local copy fails
  loudly naming the field).
- Behaviour changes outside civilians: (1) every Health set now runs before `TnuaUserControlsSystems` (was unordered);
  (2) the player and range dummies play the `die` clip while `Dead` (GDD §4.3 A); (3) the client loads six more GLBs at
  startup. All three are covered by the full test suite plus the owner run.
- Headless sim without the client never spawns civilians (no camera view) — intended; tests set a synthetic view.
- `NpcSystems` runs in `Playing` and `Wasted`. A future `Paused`/`Busted` state must be added to its condition or clear
  `ShotFired`/`MeleeHit`/`DamageDealt` readers' backlog on exit (TASK-006/007 lesson).
- After the Wasted respawn the hospital area fills only through the 60-120 m ring (the initial fill runs once per load,
  per resolved Q2's wording). Owner-visible; if unwanted, re-arm `PopulationPhase::InitialFill` on `OnExit(Wasted)` in a
  follow-up.
- T10 consumes completed `Report`s (heat); T14 adds the vehicle-on-sidewalk stimulus; T9/T11 reuse `corpse_components`,
  `Appearance`, `SidewalkGraph`, per-model graphs.
- Client gates now need the third-party GLBs on disk (`python tools/fetch_assets.py`); the gate helper says so.

## 5. Review notes (changes against PLAN_V2 / PLAN.md)

Disconfirmation tested first: "node-only spawning (V2) cannot populate the city" and "V2's early view publish is
impossible". Results: the second held (schedule order, see below); the first held partly — node spawning works but
cannot fill 40 at load (16-24 nodes near the spawn), so the plan now states the real fill dynamics instead of
promising 40 immediately.

1. **V2 step 8 corrected.** "Publish an initial view in `PreUpdate` after Loading→Playing spawned the player, before the
   first fixed tick" is impossible: `PreUpdate` runs before `StateTransition` (`main_schedule.rs:30-36`), where the player
   is spawned. The view is `None` until the first `PostUpdate` publish; the initial fill runs on the first fixed tick
   after it. V2's gate "the first Playing fixed tick gets the initial fill" is replaced by "no spawn before a view; fill on
   the first tick with a view". The PreUpdate publish and its shared-geometry refactor are dropped (YAGNI; one frame).
2. **Node spawning made concrete with measured numbers** (V2 asked to measure). Seed 1 at the player spawn: 16 nodes
   in 60-120 m, 24 in 20-120 m; min node spacing 4.5 m > separation 4.0 ⇒ one civilian per node. Candidate selection is
   a shuffled eligible-node list (deterministic count, derivable tests) instead of PLAN.md's random `spawn_attempts`
   (field removed). QA deadline to 40 raised to 120 s and time-to-40 recorded.
3. **Q2 interpretation.** Initial fill accepts a node outside the cone **or** occluded by World geometry (both rays
   blocked); steady state keeps the GDD cone-only rule. V2's wording ("outside the widened view and LOS") read as AND,
   which leaves the visible street empty on the first frame, against Q2's intent.
4. **Q1 gate made real.** V2 required real scenes but did not show that a headless client test can load GLBs; the
   probe shows it can and that the root-name bug gives exactly 0.0 rad. Gate asserts joint motion per model. Design
   simplified: identical graph build order ⇒ identical node indices, so only the graph handle is per model; scenes are
   preloaded. Measured identical skeletons ⇒ one scale for all models (no per-model height field).
5. **Report progress** is `Report { progress }` in the sim, so the client witness bar reads derived state instead of the
   sim's `civilian.ron` (GDD §12 one-owner rule). Bar data in `ui/strings.ron` `hud.witness_bar`; the UI-node projection
   pattern of `damage_numbers.rs` is reused (Bevy 0.19 has no 3D text/billboard UI).
6. **Schedule**: V2's "death before perception" is expressed as set edges `(Perceive, Decide, PopulationSystems).chain()
   .after(HealthSystems::Death)` + `Decide.before(TnuaUserControlsSystems)`; checked for cycles; the resulting new edge
   (Health sets before Tnua user controls) is listed as a behaviour change. FSM checks the state field because
   `Dead` is inserted by deferred commands.
7. **Bench**: V2's bounded-work assertion made concrete (`PerceptionLoad` resource, bounds, flips); 32 vs 64 comparison
   printed only (noise); the bench places 64 directly (node spawning cannot reach 64 in bounded time) and keeps the
   spawner busy with 4 corpses.
8. **Invalid view semantics**: `None` also pauses off-frame ageing (the old "default cone = nothing visible" would
   despawn civilians in a sim with no camera and made the first fixed ticks spawn in view).
9. **Restored from PLAN.md** where V2 compressed without correcting: data files with values, types and signatures,
   worked examples (cone angles, steering directions, flee example, reaction table), tick derivations (127/128, 256,
   1920), flip-RED per gate, QA steps, bench derivation.
10. **Dropped from PLAN.md**: the three open questions (resolved), the single-tinted-model approach, edge-point
    spawning, `ViewCone::default()` = nothing in frame, `spawn_attempts`, PLAN.md risk R6 (empty streets for 30 s).
11. **File-size law**: new client gates go into new files (`character_gate.rs` is already 697 lines).

Owner-run items (not gated by machinery): crowd feel, clip look (die/crouch), tints and model mix, witness bar look and
placement, first-seconds street life, FPS with 40 skinned civilians.

children: 0 launched / 0 reported.
