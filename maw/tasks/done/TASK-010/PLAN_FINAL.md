# PLAN_FINAL — TASK-010 (GDD T9): gangs

Cost of error: **mixed.** Silent class (matrix leak, group radius or heat timer drift, hostility outside turf, groups
popping into view, dropped guns piling up, gang spawner starving/shifting the civilian spawner, A* blowing the tick
budget, a chase stuck behind a wall) gets headless gates. Owner class (tints, how the warning reads, firefight feel,
accuracy feel, retreat look, avoidance smoothness) gets the owner checklist plus BRP evidence, no machinery.

Pinned (`Cargo.lock`, sources under `~/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/`): bevy / bevy_ecs /
bevy_app / bevy_state / bevy_reflect 0.19.1, avian3d 0.7.0, bevy-tnua 0.32.0, vendored bevy-tnua-avian3d 0.12.1,
rand_chacha 0.10.0, ron 0.12.2. **New crate: `pathfinding` =4.16.0** (GDD §6.6/§10.1; in the local registry cache
together with its deps `deprecate-until` 1.0.0, `integer-sqrt` 0.1.5; `indexmap`, `num-traits`, `rustc-hash` 2,
`thiserror` 2 already in `Cargo.lock`). No Bevy dependency, `rust-version = "1.88.0"` ≤ workspace 1.95. `citygen`
is not touched.

Evidence (all under `maw/tasks/in_progress/TASK-010/scratch/`):
- `probe_territory/` (planner): seed 1/2/42 curb polygons all CCW; 579 sidewalk nodes (seed 1), 38 in no curb
  polygon, none in two; `nearest_block` 3.3 µs/query; seed 1 gang 0 = district 3, HQ anchor (115.8, −485.7);
  gang 1 = district 0, HQ anchor (−491.3, −307.6); spawn is in district 4.
- `probe_astar/` (this review, `output.txt`): seed-1 sidewalk graph 579 nodes / 1134 edges (edge length min 4.5,
  median 65.1, max 117.3 m). `pathfinding::prelude::astar` with integer-cm costs: 4632 random pairs **mean 11 µs,
  worst 95–215 µs** (OS spikes), no pair unreachable; pairs < 60 m mean 0.5 µs; nearest-node linear scan 0.8 µs.
  Gang posts (HQ + district nodes ≥ 25 m apart): gang 0 has 31 posts, non-HQ post distances to HQ sorted
  **44.1, 46.2, 104.6, 111.2, …**; gang 1 has 32 posts, **50.7, 64.4, 110.3, …**. HQ posts are 458 m / 566 m from
  the player spawn.

---

## 1. Summary

A new `gang/` sim domain (`GangPlugin`) owns territories (built once on `Loading → Playing` from the generator's
gang districts, HQ buildings and curb polygons), the `Faction` component and a complete symmetric faction matrix in
`assets/gang/gangs.ron` (gang↔gang and gang↔police off), per-gang `GangHeat` (120 s linear memory in fixed time),
provocation (hostile `ShotFired` near a member inside its turf, or any hostile `DamageDealt` on a member) that puts
every same-gang Idle/Warn member within 30 m into `Attack` in the same fixed tick, and a pure enum FSM
`Idle/Warn/Attack/Retreat/Dead` with a four-way utility tactic scorer inside `Attack`. The FSM acts only through the
existing `MoveIntent`/`AimIntent`/`ActionIntent`, so firing, melee, damage, knockback, death animation, tracers and
civilian panic are reused unchanged. `navigation/` gains A* over `SidewalkGraph` (`pathfinding` 4.16.0, integer
cm costs), route following, and ray avoidance for direct seek; members seek directly only at visible range
≤ `direct_seek_distance` (25 m, GDD §6.6), otherwise they route, with at most `route_requests_per_tick` searches per
fixed tick (GDD §11). A dead member's gun drops as a one-shot `WeaponPickup` with a lifetime. `population/gangs.rs`
spawns whole groups of 2–4 at HQ/corner posts under the shared bubble rules (ring, hidden, shared ray budget, cap 12)
from its own RNG stream. The client gets per-model graphs for three gang models, one tint per gang read from
`GangConfig`, and the held-gun mesh for every armed character. Headless gates cover the four acceptance claims plus
fight/drop/spawn/route behaviour; `tools/qa/scenarios/t9.py` drives the runtime scenario.

---

## 2. Implementation steps

Order = dependency order. Line numbers in the earlier PLAN.md were wrong (e.g. `perception/mod.rs` has 289 lines,
the plan cited :489); **reference symbols, not line numbers.**

### 2.0 Conventions (used by every step)

- Yaw (GDD §3.2): `forward(yaw) = (−sin yaw, 0, −cos yaw)`; `aim_yaw(d) = atan2(−d.x, −d.z)` (`combat::aim_yaw`,
  `navigation::steer`). Worked: d=(0,0,−1) → 0; d=(−1,0,0) → +π/2; d=(0,0,+1) → π; d=(+1,0,0) → −π/2 (these four
  are already asserted by `navigation::tests::steer_matches_move_direction`).
- Moving an NPC: `MoveIntent { axis: Vec2::Y, yaw: <move yaw>, gait }` (civilian convention); standing:
  `axis = Vec2::ZERO`. With `AimIntent.aiming = true`, `drive_characters` faces the aim and caps the gait at
  `aim_max_gait` (Run).
- Eyes = feet + `head_height`; chest = `Position` (feet + `float_height`); feet = `Position − Y·float_height`.
- `dt = Res<Time<Fixed>>::delta_secs()` (codebase convention; equals the 1/64 s timestep inside FixedUpdate).
- Every test number below is derived from the shipped data read at test time (tests read configs from resources and
  panic `"GATE BROKEN: …"` when a fixture precondition fails).

### Data (every tuning value introduced; no new tuning `const`)

1. **`assets/gang/gangs.ron`** (new):
   ```
   (
       // Exactly two gangs: citygen assigns two territories. Index = gang id.
       gangs: [
           (tint: (0.8, 0.3, 1.0), weapons: [Pistol, Smg]),      // purple
           (tint: (1.0, 0.25, 0.2), weapons: [Pistol, Shotgun]), // red; police will be blue (T11)
       ],
       // Faction matrix, symmetric, complete (GDD §6.3, Q2 = A): gangs are hostile only to the player in the MVP.
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
           leash_distance: 60.0,        // m from the member's post
       ),
       combat: (
           keep_distance: (8.0, 15.0),  // m (GDD §6.3)
           melee_distance: (1.5, 2.5),  // m: start / stop punching (fist sweep reaches 1.0 + 0.35 + 0.3 = 1.65 m)
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
   Types in `crates/gta_sim/src/gang/mod.rs`, all `#[derive(Deserialize, Clone, Debug)] #[serde(deny_unknown_fields)]`:
   `GangConfig { gangs: Vec<GangSpec>, factions: Vec<FactionPair>, groups: GroupConfig, hostility: HostilityConfig,
   combat: GangCombatConfig }` (+ `Resource`), `GangSpec { tint: (f32, f32, f32), weapons: Vec<Weapon> }`,
   `FactionPair { a: Faction, b: Faction, hostile: bool }`, `GroupConfig { size: (u32, u32), spread, post_spacing,
   hq_margin }`, `HostilityConfig { … }`, `GangCombatConfig { keep_distance: (f32, f32), melee_distance: (f32, f32),
   aim_error_deg, trigger_seconds: (f32, f32), retreat_health, retreat_distance, tactics: TacticWeights, warn_gait:
   Gait, chase_gait: Gait, back_off_gait: Gait, retreat_gait: Gait }`, `TacticWeights { retreat, melee, shoot, chase }`.
   `pub const GANG_CONFIG: &str = "gang/gangs.ron";` (a path, not tuning). `Weapon` and `Gait` already derive
   `Deserialize` (`combat/weapons.rs`, `character/intent.rs`).
   `GangConfig::validate() -> Result<(), String>`, every error names its field:
   - `gangs.len() == 2` ("gangs must list exactly 2 gangs: citygen assigns 2 territories"); each `weapons` non-empty
     ("gangs[i].weapons must not be empty"); each tint component finite and ≥ 0 ("gangs[i].tint …").
   - `factions`: `a != b`; each `Gang(i)` has `i < gangs.len()`; no unordered pair twice; **each** of the five pairs
     `{Gang(0),Gang(1)}`, `{Gang(i),Player}`, `{Gang(i),Police}` present (error names it, e.g.
     "factions: missing pair (Gang(0), Police)").
   - `groups.size`: `1 ≤ lo ≤ hi`; `spread`, `post_spacing`, `hq_margin` finite > 0; `post_spacing > 2·spread`.
   - `hostility`: all finite > 0 except `warn_seconds ≥ 0`; `warn_keep_distance ≤ warn_distance < warn_release_distance`.
   - `combat`: `melee_distance.0 < melee_distance.1 < keep_distance.0 < keep_distance.1`; `0 ≤ aim_error_deg < 90`;
     `0 < trigger_seconds.0 ≤ trigger_seconds.1`; `0 < retreat_health < 1`; `retreat_distance > 0`; weights finite ≥ 0.
   Method `pub fn hostile(&self, a: Faction, b: Faction) -> bool`: `a != b` and the unordered pair is listed with
   `hostile: true` (same faction is never hostile — law in code).
2. **`assets/npc/population.ron`**: add `max_gang_members: 12,  // GDD §6.1: bandits only in gang territories`.
   `PopulationConfig.max_gang_members: u32` with a doc line ("0 disables gangs; below `groups.size.0` nothing spawns").
   No extra validation.
3. **`assets/combat/weapons.ron`**: `pickups: (radius: 1.0, respawn: 30.0, drop_seconds: 60.0),` — seconds a gun dropped
   by a dead NPC lies before it vanishes. `WeaponPickupConfig.drop_seconds: f32` (doc line); in
   `WeaponsConfig::validate` add `("pickups.drop_seconds", p.drop_seconds)` to the finite list and
   `check(p.drop_seconds > 0.0, "pickups.drop_seconds")?`.
4. **`assets/npc/navigation.ron`** (edit; GDD §6.6 / §11):
   ```
   (
       arrive_radius: 0.5,
       keep_right: 0.5,
       direct_seek_distance: 25.0, // m: gangs/police seek a visible target straight within this, else route (GDD §6.6)
       route_requests_per_tick: 2, // A* searches per fixed tick, at most (GDD §11 queue K)
       route_refresh_seconds: 1.0, // s: a route to a moving goal is re-planned at most this often
       avoid_distance: 2.0,        // m: probe ray ahead of a direct seek
       avoid_step_deg: 30.0,       // deg: detour headings tried at ±1, ±2, ±3 steps
   )
   ```
   (existing comments on the first two fields kept.) `NavigationConfig` fields `direct_seek_distance: f32,
   route_requests_per_tick: u32, route_refresh_seconds: f32, avoid_distance: f32, avoid_step_deg: f32`; `validate`:
   finite > 0 for the four floats, `route_requests_per_tick ≥ 1`, `avoid_step_deg < 60` (3 steps stay within ±180°).
5. **`assets/character/visual.ron`** (client): add
   ```
   // Gang looks: `Appearance` picks one of these models; the tint comes from gang/gangs.ron.
   gang_models: [
       "third_party/mini-characters/character-male-d.glb",
       "third_party/mini-characters/character-male-f.glb",
       "third_party/mini-characters/character-female-c.glb",
   ],
   ```
   All three are in `assets/third_party/manifest.ron` (rig model list) and unused so far. **No tint list here**
   (one source: `gangs.ron`).

### Sim — dependency

6. **`crates/gta_sim/Cargo.toml`**: `pathfinding = "=4.16.0"` under `[dependencies]` (pinned like `rand_chacha`).
   `cargo build --offline` must resolve from the local cache; `Cargo.lock` gains `pathfinding`, `deprecate-until`,
   `integer-sqrt`. Check `cargo tree -p gta_sim -e normal -i bevy_render` stays empty.

### Sim — navigation (`crates/gta_sim/src/navigation/mod.rs`, target ≤ 480 lines)

7. Add (all `pub`, documented):
   - `pub fn nearest_node(graph: &SidewalkGraph, p: Vec3) -> Option<u32>`: node with ≥ 1 neighbour minimising
     `flat_distance`; ties → lower id; `None` on an edgeless graph.
   - `pub fn find_route(graph: &SidewalkGraph, from: u32, to: u32) -> Option<Vec<u32>>`: `pathfinding::prelude::astar`
     (signature verified in `pathfinding-4.16.0/src/directed/astar.rs:81`: `astar(&start, successors, heuristic,
     success) -> Option<(Vec<N>, C)>` with `C: Zero + Ord + Copy`) — **cost must be an integer** (f32 is not `Ord`):
     edge cost and heuristic = `(flat_distance(a, b) * 100.0) as u32` (cm). Returns the node list including `from`
     and `to`.
   - `#[derive(Component, Reflect, Default, Clone, Debug)] #[reflect(Component)] pub struct Route { pub goal:
     Option<u32>, pub nodes: Vec<u32>, pub next: usize, pub age: f32 }` — the current plan of one NPC; mutated in
     place (no per-tick insert/remove).
   - `pub fn plan_route(graph, route: &mut Route, from: Vec3, goal: u32) -> bool`: `start = nearest_node(from)`;
     `find_route(start, goal)`; on `Some(path)`: if `path.len() ≥ 2` and `flat_distance(from, node(path[1])) <
     flat_distance(node(path[0]), node(path[1]))`, drop `path[0]` (already past the first corner); set `goal`,
     `nodes`, `next = 0`, `age = 0`; return true. On `None`: clear `nodes`, `goal = Some(goal)`, `age = 0`, return false.
   - `pub fn route_point(graph, route: &mut Route, from: Vec3, destination: Vec3, arrive_radius: f32) -> Vec3`: while
     `next < nodes.len()` and `flat_distance(from, node(nodes[next])) ≤ arrive_radius` → `next += 1`; returns
     `node(nodes[next])` or `destination` once the nodes are used up.
   - `pub fn avoid_offset(spatial: &SpatialQuery, chest: Vec3, yaw: f32, cfg: &NavigationConfig, rays: &mut u32) -> f32`:
     tries offsets `0, +s, −s, +2s, −2s, +3s, −3s` (`s = avoid_step_deg` in rad); for each, `rays += 1` and returns the
     first offset `o` with `!sight_blocked(spatial, chest, chest + forward(yaw + o) · avoid_distance)`; `0.0` if all
     blocked. (`sight_blocked` uses the World mask only — characters are not obstacles, Tnua capsules push apart.)
   - `#[derive(Resource, Reflect, Default, Clone, Copy, Debug)] #[reflect(Resource)] pub struct RouteLoad { pub
     searches: u32, pub rays: u32 }` — work of the current fixed tick.
   - `NavigationPlugin::build`: `init_resource::<RouteLoad>()`, `register_type::<Route>()`,
     `register_type::<RouteLoad>()`, and `reset_route_load.in_set(AiSystems::Perceive)` in `FixedUpdate`
     (`*load = RouteLoad::default()`; `AiSystems::Perceive` runs before `Decide`, so every route user sees a fresh budget).
   - Unit tests (`#[cfg(test)]`, worked directional examples; square graph n0 (0,0,0), n1 (0,0,−20), n2 (−20,0,−20),
     n3 (−20,0,0), edges (0,1), (1,2), (2,3), **no (3,0)**):
     - `route_goes_around_the_missing_edge`: `find_route(0, 3) == Some([0, 1, 2, 3])` (the straight 0→3 is not an edge).
     - `route_turns_north_west_south`: walker at (0,0,0.3), destination (−20,0,5). `plan_route` → start n0 (0.3 m),
       path [0,1,2,3], n0 kept (`flat((0,0,0.3), n1) = 20.3 ≥ 20`). `route_point` + `steer`, arrive_radius 0.5:
       at (0,0,0.3) n0 is within 0.3 → target n1, d = (0,0,−20.3) → yaw 0 (north, −Z); at (0,0,−20) → target n2,
       d = (−20,0,0) → yaw +π/2 (west, −X); at (−20,0,−20) → target n3, d = (0,0,20) → yaw π (south, +Z); at (−20,0,0)
       → nodes used up → returns the destination (−20,0,5), yaw π. Compare yaws modulo 2π with 1e-5 (positions sit
       exactly on nodes so the yaws are exact).
     - `plan_route_skips_a_passed_corner`: walker at (0,0,−5) routing to 3: path [0,1,2,3]; `flat((0,0,−5), n1) = 15 <
       20` → nodes = [1,2,3].
     - `no_route_on_a_disconnected_graph`: two separate edges → `find_route` `None`, `plan_route` false, `nodes` empty.
     - `nearest_node_prefers_the_lower_id_on_a_tie` and skips edgeless nodes.

### Sim — `gang/` domain (`crates/gta_sim/src/gang/`)

8. **`gang/mod.rs`** (new, ~330 lines): config (Step 1), components, resources, bundle, plugin.
   - `#[derive(Component, Reflect, Deserialize, Clone, Copy, Debug, PartialEq, Eq, Hash)] #[reflect(Component)]
     pub enum Faction { Player, Gang(u8), Police }` (`Police` exists only so the matrix states the off pairs; T11
     attaches it to cops).
   - `#[derive(Reflect, Clone, Copy, Debug, PartialEq)] pub enum GangState { Idle, Warn, Attack { target: Entity },
     Retreat { from: Entity }, Dead }` (`Entity` derives `Reflect` in bevy_ecs 0.19.1, so BRP reads it).
   - ```rust
     #[derive(Component, Reflect, Clone, Debug)]
     #[reflect(Component)]
     #[require(Character, Perception, Offscreen, Route)]
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
         /// Line of sight to the current focus (target, else the player), refreshed on the member's AI slot.
         pub sees: bool,
         pub last_seen: Vec3,
         /// Line of sight from the chest to `spot`, refreshed on the AI slot while away from it.
         pub home_clear: bool,
         /// Yaw offset of the current direct seek chosen by `avoid_offset`, refreshed on the AI slot.
         pub avoid: f32,
         /// Seconds until the next trigger pull or punch may happen (clamped at 0).
         pub trigger_left: f32,
     }
     ```
     `Perception` is required only for its `slot` (civilian `collect_stimuli`/`perceive` query `&Civilian`, so they
     skip members; `pending` stays `None`).
   - `#[derive(Resource, Reflect, Debug)] #[reflect(Resource)] pub struct GangHeat { pub left: Vec<f32> }` (seconds,
     one per gang, zeros from `GangConfig.gangs.len()`).
   - `#[derive(Resource, Reflect, Default, Debug, PartialEq)] #[reflect(Resource)] pub struct PlayerTerritory(pub Option<u8>)`.
   - `#[derive(Resource)] pub struct GangRng(pub ChaCha8Rng)` with `seeded(seed)` = `seed_from_u64(seed)` +
     `set_stream(2)` (`NpcRng` is stream 1, `CombatRng` the default stream), `unit()` via `combat::unit_f32`,
     `next_u32()` — same shape as `NpcRng`.
   - `#[derive(SystemSet, Debug, Clone, PartialEq, Eq, Hash)] pub struct GangSystems;`
   - `pub fn roll(rng: &mut GangRng, (lo, hi): (f32, f32)) -> f32 = lo + (hi − lo)·rng.unit()`.
   - `#[allow(clippy::too_many_arguments)] pub fn gang_member_bundle(loco: &LocomotionConfig, handle:
     Handle<CharacterSchemeConfig>, health: &HealthConfig, weapons: &WeaponsConfig, gang: u8, post: u16, spot: Vec3,
     facing_yaw: f32, gun: Weapon, appearance: Appearance) -> impl Bundle` = `(GangMember { gang, post, spot, gun,
     state: Idle, dwell: 0, sees: false, last_seen: spot, home_clear: true, avoid: 0, trigger_left: 0 },
     Faction::Gang(gang), appearance, Name::new("Gang member"), Transform::from_translation(spot + Y·float_height)
     .with_rotation(Quat::from_rotation_y(facing_yaw)), character_components(loco, handle), Health::full(health),
     loadout)` where `loadout = Loadout::default()` then `acquire(&mut loadout.guns[gun.index()], weapons.stats(gun),
     true)`, `held: None` (holstered). The handle is `CharacterControlConfig.0.clone()` (what `civilian_bundle` gets).
   - `pub struct GangPlugin { pub seed: u64 }`, `build`:
     `insert_resource(GangHeat { left: vec![0.0; cfg.gangs.len()] })` (reads the inserted `GangConfig`),
     `init_resource::<PlayerTerritory>()`, `insert_resource(GangRng::seeded(seed))`, `register_type` for `Faction,
     GangState, GangMember, GangHeat, PlayerTerritory, GangTerritories, Turf, TurfBlock`;
     `configure_sets(FixedUpdate, GangSystems.in_set(NpcSystems).run_if(resource_exists::<GangTerritories>))`;
     `add_systems(OnTransition { exited: Loading, entered: Playing },
     territory::build_gang_territories.run_if(resource_exists::<City>))` (one-shot on the transition, never on a
     `Playing` re-entry after `Wasted`);
     `FixedUpdate`:
     - `behavior::gang_death.in_set(HealthSystems::Death).in_set(NpcSystems)` — **exactly like `civilian_death`; NOT in
       `GangSystems`** (a death must be handled even without territories, e.g. presentation harnesses);
     - `(behavior::decay_gang_heat, behavior::locate_player, behavior::provoke_gangs).chain()
       .in_set(AiSystems::Perceive).in_set(GangSystems)`;
     - `behavior::gang_fsm.in_set(AiSystems::Decide).in_set(GangSystems)`.
     The spawner systems are registered by `PopulationPlugin` (Step 13).
9. **`gang/territory.rs`** (new, ~230 lines incl. tests):
   - ```rust
     #[derive(Reflect, Clone, Debug)] pub struct Turf { pub posts: Vec<Vec3> }  // posts[0] = HQ, y = sidewalk top
     #[derive(Resource, Reflect, Debug)] #[reflect(Resource)]
     pub struct GangTerritories { pub gangs: Vec<Turf>, blocks: Vec<TurfBlock> }
     #[derive(Reflect, Clone, Debug)] pub struct TurfBlock { curb: Vec<Vec2>, gang: Option<u8> } // layout (x, z), CCW
     ```
   - `pub fn new(gangs: Vec<Turf>, blocks: Vec<(Vec<Vec2>, Option<u8>)>) -> Result<Self, String>`: every block ≥ 3
     points with positive signed area (CCW; error names the block index); every gang ≥ 1 post.
   - `pub fn from_layout(layout: &CityLayout, params: &CityParams, groups: &GroupConfig) -> Result<Self, String>`:
     blocks = every `layout.blocks` entry with `curb.len() ≥ 3`, gang = position of `block.district` in
     `layout.gang_districts`; for g in 0..2: HQ = building with `kind == BuildingKind::GangHq(g)` ("no HQ for gang
     {g}"); anchor = `citygen::sidewalk_anchor(layout, params, hq, groups.hq_margin)` ("gang {g} HQ {hq}: no sidewalk
     side of length ≥ {2·margin}"); post 0 = anchor as world (x, `params.roads.curb_height`, y); then sidewalk nodes
     (`layout.sidewalks`, with ≥ 1 edge) in node-id order whose `territory_at == Some(g)`, kept when ≥ `post_spacing`
     from every kept post of that gang (probe: 31 / 32 posts on seed 1).
   - `pub fn territory_at(&self, p: Vec3) -> Option<u8>`: `q = Vec2(p.x, p.z)`; the first block with
     `contains_convex(curb, q, 0.0)` gives its gang; else the block with the smallest `dist_point_segment` to its curb
     edges (ties → lower index). O(blocks) ≈ 3.3 µs.
   - `pub(crate) fn build_gang_territories(mut commands, city: Res<City>, params: Res<CityParamsRes>, cfg:
     Res<GangConfig>, mut exit: MessageWriter<AppExit>)`: `Ok` → `insert_resource`; `Err` → `error!("gang territories
     invalid: {e}")` + `exit.write(AppExit::error())` (policy of `build_sidewalk_graph` / the hospital).
   - Unit tests: (a) worked example on two synthetic squares — gang 0 `[(−40,−40),(0,−40),(0,40),(−40,40)]`, none
     `[(2,−40),(40,−40),(40,40),(2,40)]` (CCW in (x, z)): (−10,·,5) → Some(0); (30,·,0) → None; (0.5,·,0) (0.5 m from
     gang 0, 1.5 m from none) → Some(0); (1.5,·,0) → None; a clockwise square → `new` error naming block 0.
     (b) **`every_sweep_seed_has_two_territories`**: shipped `world/city.ron` + `gang/gangs.ron` via `include_str!`
     ("GATE BROKEN" on parse), seeds 0..32 plus 42 through `citygen::generate` + `from_layout`: `Ok`; per gang ≥ 2
     posts; `territory_at(post) == Some(g)` for every post; posts of a gang pairwise ≥ `post_spacing`; posts[0] within
     0.01 m of the recomputed `sidewalk_anchor`. Flip-RED: keep nodes of every district → `territory_at` RED; skip the
     spacing filter → spacing RED.
10. **`gang/fsm.rs`** (new, ~320 lines incl. tests) — pure decisions, no ECS:
    - `pub struct Focus { pub distance: f32, pub from_post: f32, pub visible: bool }` (`from_post` = the focus'
      horizontal distance to the member's `spot`).
    - `pub struct Senses { pub heat: f32, pub player: Option<Focus>, pub player_in_turf: bool, pub dwell: f32, pub
      aimed_at: bool, pub target: Option<Focus>, pub target_is_player: bool, pub tactic: Tactic }` (`player` = the live
      player, `None` if absent or `Dead`; `target` = the live Attack/Retreat target, `None` if despawned or `Dead`).
    - `pub enum Tactic { Retreat, Melee, Shoot, Chase }`; `pub fn choose_tactic(health_fraction: f32, distance: f32,
      visible: bool, has_ammo: bool, punching: bool, in_range: bool, cfg: &GangCombatConfig) -> Tactic`: scores
      `retreat = w.retreat if health_fraction < retreat_health && distance < retreat_distance`, `melee = w.melee if
      distance ≤ melee_distance.0 || (punching && distance ≤ melee_distance.1)`, `shoot = w.shoot if visible &&
      has_ammo && in_range`, `chase = w.chase`, else 0; highest wins, ties Retreat > Melee > Shoot > Chase.
    - `pub enum Move { Approach, Hold, BackOff }`; `pub fn band_move(distance, (lo, hi)) -> Move`: `> hi` Approach,
      `< lo` BackOff, else Hold.
    - `pub fn next_state(state: GangState, player: Option<Entity>, s: &Senses, h: &HostilityConfig) -> GangState`, rules
      in order:
      - `Idle`/`Warn`: if `s.heat > 0 && s.player_in_turf` and the player is `visible`, `distance ≤ sight_distance`,
        `from_post ≤ leash_distance` → `Attack { target: player }`. Else `Idle`: `player_in_turf && (dwell ≥
        warn_seconds || aimed_at)` → `Warn`; `Warn`: no player, `!player_in_turf` or `distance > warn_release_distance`
        → `Idle`; else unchanged.
      - `Attack { t }`: `target == None` → `Idle`; `target_is_player && heat == 0` → `Idle`; `from_post >
        leash_distance` → `Idle`; `tactic == Retreat` → `Retreat { from: t }`; else unchanged.
      - `Retreat { f }`: `target == None` → `Idle`; `target_is_player && heat == 0` → `Idle`; else unchanged.
      - `Dead` → `Dead`.
    - Unit tests read the shipped `gangs.ron` via `include_str!` (numbers below derive from it; "GATE BROKEN" if they
      change):

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
      | 17 | Attack{rival} | heat 0, target not player, from_post 20 | Attack{rival} |

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
      `(Gang(1), Gang(0))`, `(Gang(0), Police)` false; `(Gang(0), Gang(0))` false.
11. **`gang/behavior.rs`** (new, ~450 lines; split the movement helper into `gang/movement.rs` if it passes 600):
    - `decay_gang_heat(time, heat)`: `left[g] = (left[g] − dt).max(0.0)`. Runs in `NpcSystems` (Playing **and**
      Wasted), so heat keeps decaying during Wasted (resolved Q3). Its seconds are fixed-time seconds; under the
      Wasted slow-motion (`Time<Virtual>` relative speed) they pass slower in wall-clock time — state this in QA.
    - `locate_player`: `PlayerTerritory(territories.territory_at(player.position))`, `None` without a live player.
    - `provoke_gangs`: `MessageReader<ShotFired>`, `MessageReader<DamageDealt>` (buffered messages: the writers
      `fire_weapons`/`apply_strikes` run earlier in the same fixed tick in `HealthSystems::Damage`); `factions:
      Query<&Faction>`; `positions: Query<&Position>`; `members: Query<(Entity, &Position, &mut GangMember)>`;
      `cfg: Res<GangConfig>`, `territories: Res<GangTerritories>`, `ResMut<GangHeat>`, `ResMut<GangRng>`.
      Builds `provocations: Vec<(gang: u8, at: Vec3, attacker: Entity)>`:
      - per `DamageDealt` whose `target` is a `GangMember` (alive or killed this tick — its `Position` is still valid)
        and `cfg.hostile(Faction::Gang(member.gang), attacker faction)` (attacker without `Faction` → ignored) →
        `(g, victim position, shooter)` — **regardless of where the attacker stands** (resolved Q1: a hit always provokes);
      - per `ShotFired` with shooter faction `f`: every non-Dead member within `shot_radius` (3D, muzzle → chest) with
        `territory_at(muzzle) == Some(member.gang)` and `cfg.hostile(Gang(member.gang), f)` → `(g, member position, shooter)`.
      Then per provocation: if the attacker's faction is `Player` → `heat.left[g] = heat_seconds`; every member of gang
      `g` with state `Idle`/`Warn` and horizontal distance to `at` ≤ `group_radius` → `state = Attack { target:
      attacker }`, `last_seen = attacker Position` (or `at` if it has none), `sees = false`, `trigger_left =
      roll(trigger_seconds)` (GangRng). Members in `Attack`/`Retreat` keep their target; `Dead` untouched. One hop
      only (distance to `at`, never member-to-member chains).
    - `gang_fsm` (Decide): params `Res<GangConfig>`, `Res<PerceptionConfig>` (`slots`, `aimed_distance`,
      `aimed_cone_deg` — the numbers civilians use), `Res<LocomotionConfig>`, `Res<NavigationConfig>`,
      `Res<WeaponsConfig>`, `Res<AiClock>`, `Res<GangHeat>`, `Res<PlayerTerritory>`, `Res<SidewalkGraph>`,
      `ResMut<GangRng>`, `ResMut<RouteLoad>`, `Res<Time<Fixed>>`, `SpatialQuery`; `members: Query<(Entity, &mut
      GangMember, &mut Route, &Perception, &Position, &Health, &Loadout, &mut MoveIntent, &mut AimIntent, &mut
      ActionIntent)>`; `player: Query<(Entity, &Position, &AimIntent, &Loadout, Has<Dead>), (With<Player>,
      Without<GangMember>)>` (disjoint from `members`, no B0001); `bodies: Query<(&Position, Has<Dead>),
      Without<GangMember>>` plus a read of member positions collected before the mutable loop (a member target is
      another member only when the matrix is on). Group the resource params into tuples if clippy's argument limit
      bites (as `spawn_civilians` does). Per member with state ≠ `Dead`:
      1. `dwell += dt` if the player is alive, `PlayerTerritory == Some(gang)` and horizontal distance ≤
         `warn_distance`; else `dwell = 0`.
      2. On the member's slot (`clock.tick % slots == perception.slot`): focus = target (Attack/Retreat) or player;
         `sees = focus within sight_distance && !sight_blocked(eyes, focus chest)`; if `sees`, `last_seen = focus
         position`; `aimed_at` = player alive, `aim.aiming`, `held.is_some()`, distance ≤ `aimed_distance`,
         `aim.direction.angle_between(member chest − aim.origin) ≤ aimed_cone_deg`, and `sees`; `home_clear =
         !sight_blocked(chest, spot + Y·float_height)` when farther than `arrive_radius` from `spot`; if the member is
         doing a direct seek this tick, `avoid = avoid_offset(spatial, chest, seek yaw, nav, &mut load.rays)`.
         Off-slot: `aimed_at = false`, others keep last values.
      3. `tactic = choose_tactic(health.current / max_health, d, sees, magazine + reserve > 0, held.is_none(), d ≤
         stats(gun).range, combat)` (meaningful only in Attack).
      4. `state = next_state(state, player entity, &senses, hostility)`; on entering `Attack` here (heat on sight) roll
         `trigger_left`.
      5. In `Attack`/`Retreat`: `trigger_left = (trigger_left − dt).max(0.0)`.
      6. Intents by state. `fire_requested` is only ever **set**, never cleared (the weapon systems take it):
         - `Idle`: `aim.aiming = false`; want `held = None`; if farther than `arrive_radius` from `spot`: head for `spot`
           (`warn_gait`, direct iff `home_clear && d ≤ direct_seek_distance`), else `axis = ZERO`.
         - `Warn`: want `held = Some(gun)`; `aim = { origin: eyes, direction: player chest − eyes, aiming: true }`; head
           for the player (`warn_gait`, direct iff `sees && d ≤ direct_seek_distance`) while `d > warn_keep_distance`,
           else stand.
         - `Attack` by tactic:
           **Shoot** — want `held = Some(gun)`; aim at the target chest; `band_move` → Approach (head for the target,
           `chase_gait`) / BackOff (move yaw = `steer(target, self)`, i.e. away, `back_off_gait`, still aiming, no
           route) / Hold (stand); when `trigger_left == 0 && sees && in_range && loadout.held == Some(gun)`:
           `fire_requested = true`, `aim.direction = cone_sample(dir, aim_error_deg, u, v)` (GangRng) for this tick
           only, `trigger_left = roll(trigger_seconds)`. **The `held == Some(gun)` condition is mandatory**: a pull
           while the select is still pending would reach `swing_melee` as a punch.
           **Melee** — want `held = None`; flat aim toward the target (`aiming = true`); head for the target (direct)
           while `d > melee_distance.0`, else stand; when `trigger_left == 0 && loadout.held.is_none()`:
           `fire_requested = true`, re-roll.
           **Chase** — want `held = Some(gun)` if it has ammo else `None`; aim toward `last_seen`; head for `last_seen`
           (`chase_gait`, direct iff `sees && d ≤ direct_seek_distance`), stand within `arrive_radius`.
         - `Retreat`: want `held = Some(gun)`; if `d < retreat_distance`: `aiming = false`, move yaw away from the target
           (`retreat_gait`, direct with avoidance, no route); else stand and act like Shoot's trigger (aiming, pulls
           when `sees`).
         - Weapon choice goes through `action.select`: `Some(WeaponRequest::Gun(gun))` / `Some(WeaponRequest::Unarmed)`
           only when `loadout.held` differs from the wanted one (never `Unarmed` while unarmed: no bat toggle).
      7. **"Head for `dest`"** (one helper): `route.age += dt`. Direct → `route.nodes.clear()`, `yaw = steer(pos, dest)
         + avoid`. Else → `goal = nearest_node(graph, dest)`; if `route.nodes` empty or `route.goal != goal` or
         `route.age ≥ route_refresh_seconds`, and `load.searches < route_requests_per_tick` → `load.searches += 1`,
         `plan_route(graph, route, pos, goal)`; `yaw = steer(pos, route_point(graph, route, pos, dest,
         arrive_radius))`; with no usable route (none yet and budget spent, or `plan_route` false) fall back to the
         direct yaw. `MoveIntent { axis: Vec2::Y, yaw, gait }`.
    - `gang_death` (`HealthSystems::Death` + `NpcSystems`): `Query<(Entity, &Health, &Position, &CharacterBody,
      &mut GangMember, &mut MoveIntent, &mut AimIntent, &mut ActionIntent, &mut Loadout)>`, `Res<WeaponsConfig>`;
      state ≠ `Dead` and `current ≤ 0` → `state = Dead` (the state field, since the `Dead` insert is deferred),
      `axis = ZERO`, `aiming = false`, `fire_requested = false`, the gun leaves the body (`held = None`,
      `guns[gun.index()] = GunSlot::default()`, `reload_left = 0`), `insert(corpse_components())`, and
      `commands.spawn(dropped_gun(gun, feet, &weapons))`. A fatal hit and its `DamageDealt` in one tick: death runs
      first (Death < Perceive), `provoke_gangs` still aggroes the group from the victim's position.
12. **`crates/gta_sim/src/lib.rs`**: `pub mod gang;`; load + validate `GangConfig` (`GANG_CONFIG`) like the others;
    `insert_resource(gangs)`; append `GangPlugin { seed: combat_seed }` after `CivilianPlugin` in the plugin tuple
    (12 → 13 entries; the tuple impl goes to 15).
    **`combat/pickups.rs`**: `#[derive(Component, Reflect, Debug)] #[reflect(Component)] pub struct Dropped { pub left:
    f32 }`; `pub fn dropped_gun(weapon: Weapon, feet: Vec3, cfg: &WeaponsConfig) -> impl Bundle` = `(WeaponPickup {
    weapon, ammo_only: false, cooldown: 0.0 }, Dropped { left: cfg.pickups.drop_seconds }, Name::new(format!("Dropped
    {weapon:?}")), Transform::from_translation(feet))`. `collect_weapon_pickups` gets `Entity`, `Has<Dropped>` and
    `Commands`: on a successful take a dropped pickup is `try_despawn`ed instead of `cooldown = respawn`, and the loop
    over players breaks for it. New `pub(super) fn expire_dropped(commands, time, Query<(Entity, &mut Dropped)>)`:
    `left −= dt`, `try_despawn` at `left ≤ 0.0`; registered next to the other pickup systems in `combat/mod.rs`
    (`HealthSystems::Pickup`, inside the `PlayingSystems` tuple), `.after(pickups::collect_weapon_pickups)` so a gun
    taken in its last tick is taken, not expired. Register `Dropped`; export `Dropped`, `dropped_gun`.
    **`player/mod.rs` `spawn_player`**: add `Faction::Player` to the bundle (the player entity persists through
    Wasted/respawn, so the component stays).

### Sim — gang spawner (`population/`)

13. **`population/gangs.rs`** (new, ~230 lines) + **`population/mod.rs`**: `mod gangs;` and, in `PopulationPlugin::build`,
    `(gangs::despawn_far_gangs, gangs::spawn_gangs).chain().after(spawn_civilians).in_set(PopulationSystems)
    .in_set(GangSystems)`. `spawn_civilians`, `despawn_far` and `NpcRng` are not changed.
    - `despawn_far_gangs`: no view or no player → return; per `GangMember` (alive or corpse): `Offscreen` as in
      `despawn_far` (`outside_cone(view, feet, head_height, 0.0)`), `try_despawn` when `Offscreen ≥
      despawn_offscreen_seconds` **and** horizontal distance to the player `> despawn_distance`. No recycling.
    - `spawn_gangs`: no view or no player → return. `alive` = members with state ≠ `Dead`; `alive ≥ max_gang_members`
      → return. Occupied posts = every `(gang, post)` of any `GangMember` (alive or corpse). Bodies = `Position` of
      every `Character`. Candidates = free posts with horizontal distance to the player in `spawn_ring`, sorted HQ
      (post 0) first, then distance ascending (stable, then gang id). For each candidate: `size = lo +
      (rng.next_u32() % (hi − lo + 1))`; skip if `alive + size > max_gang_members`; spots `post + (cos θ, 0, −sin θ)·
      spread`, θ = 2πk/size; skip if any spot is closer than `spawn_min_separation` to any body; each spot must satisfy
      `outside_cone(view, spot, head_height, margin)` or — if `load.rays + OCCLUSION_RAYS_PER_POINT ≤
      occlusion_rays_per_tick` — `occluded(spatial, view, spot, occlusion_ray_height, capsule_radius, &mut load.rays)`;
      one visible or unaffordable spot skips the whole post this tick. Then spawn: gun = `weapons[rng.next_u32() %
      len]`, `Appearance(rng.next_u32())`, facing `aim_yaw(post − spot)`; `alive += size`. `load.rays` is **not**
      reset here (`spawn_civilians` resets it at its start on every path), so the tick total stays within one budget.
      Worked facing: pair k=0 at post + (1,0,0) faces −X: `aim_yaw((−1,0,0)) = +π/2` ✓.
    - Known and accepted: `spawn_civilians` checks separation only against civilians, so a civilian may appear on a
      post node 1 m from a group (capsules 0.3 m do not intersect). Not changed (surgical; civilian gates stay put).
14. **Scheduling in one fixed tick** (for the implementer): Damage (shots, strikes; `ShotFired`/`DamageDealt`) → Regen →
    Pickup (`collect_weapon_pickups` → `expire_dropped`) → Death (`civilian_death`, `gang_death`, `dummy_life`, player
    death) → Perceive (`advance_clock → collect_stimuli → perceive` ‖ `reset_route_load` ‖ `decay_gang_heat →
    locate_player → provoke_gangs`) → Decide (`civilian_fsm` ‖ `gang_fsm`, both before `TnuaUserControlsSystems`) →
    PopulationSystems (`age_corpses → despawn_far → spawn_civilians → despawn_far_gangs → spawn_gangs`). NPC intents
    written in Decide are consumed in the next tick's Damage set (one tick, 15.6 ms). `gang_death` sits in `Death` and
    `NpcSystems` (as `civilian_death`), never in `GangSystems`; no set is ordered against a set it contains.

### Sim — gates (`cargo test -p gta_sim`)

All integration tests use the production composition (`headless_app` / `city_app`), the production
`gang_member_bundle`, configs read from resources, ticks counted by `Time<Fixed>` (`run_ticks`, `Shots::run`).
Allowed, named test-side mutations: insert a synthetic `GangTerritories`; insert a test `SidewalkGraph` (the floor has
none and `NpcSystems` needs one); set `CameraView`; set `max_gang_members` / `max_civilians`; flip one matrix pair in
the `GangConfig` resource (positive controls); set a member's `Health`; set the player's `armor = 1.0e6` (so a fight
cannot kill it — `apply_damage` does not clamp armour); **`provoke(app, member)`** = `state = Attack { target: player }`,
`last_seen = player position`, `GangHeat[gang] = heat_seconds` (fight/route tests only — provocation itself is gated in
`gangs.rs`); despawn a member; place the player. Test floor (`world/test_area.rs`): 80 × 80 m, x, z ∈ [−40, 40];
the wall at (0, 2, 14) spans x ∈ [−6, 6], z ∈ [13.75, 14.25], 4 m tall; ramp/blocks occupy x ∈ [−12, −8],
z ∈ [−24.7, −11.6] and x ∈ [8.5, 11.5], z ∈ [−20.4, −12.1]; boxes at (10|13|16, ·, 10). All positions below avoid them.
Record each flip-RED (perturbed input, RED seen, GREEN restored) in IMPL_SUMMARY.

15. **`tests/common/mod.rs`** helpers (file is 442 lines; stays < 750): `gang_floor(turf: TurfLayout, nodes:
    Vec<Vec3>, edges: &[(u32, u32)]) -> App` (headless app + `test_graph` + synthetic `GangTerritories` with gang posts
    (−30,0,−30) / (30,0,30) + `settle` + player pistol as `civilians.rs::armed_app`: `held = Pistol`, full magazine);
    `TurfLayout::{WestHalf, WholeFloor}` (WestHalf: gang 0 = square x ∈ [−40, 0], none = square x ∈ [2, 40];
    WholeFloor: gang 0 = the 80 × 80 floor); `spawn_member(app, gang, spot, gun) -> Entity` via `gang_member_bundle`
    (post 0, facing −Z, `Appearance(0)`); `gang_state(app, e)`, `heat(app, g)`, `provoke(app, e)`,
    `set_player_armor(app, a)`, `set_matrix(app, a, b, hostile)`. Default floor graph for tests that do not route:
    nodes (30,0,30), (35,0,30), edge (0,1).
16. **`tests/gangs.rs`** (new, acceptance for hostility):
    - **`outside_the_territory_members_stay_neutral`** (acceptance "вне территории нейтральны"). `WestHalf`. A
      (−4,0,−10), B (−2,0,−12), gang 0, pistols. (a) Player at (3,0,−10) (outside; 7 m from A < `warn_distance`),
      `aiming = true` (set on `AimIntent` directly; `set_aim` does not set it) at A's chest: run `warn_seconds·64 +
      slots + 8` = 204 ticks → A, B `Idle`, `PlayerTerritory == None`, heat `[0, 0]`. Then a shot into the air (aim
      +Y, `fire_requested`; muzzle x ≈ 3 > 0, within `shot_radius` of both chests): 4 more ticks → still `Idle`, heat
      0, and one `ShotFired` seen (precondition). (b) Positive control: player to (−6,0,−4) (inside; 6.32 m to A,
      8.94 m to B), `aiming = false`, aim straight up: A is `Idle` after 191 ticks and `Warn` at tick 192
      (`warn_seconds·64`; dwell increments are k/64, exact); B stays `Idle` (8.94 > 8). Then a shot into the air → in
      the shot tick A and B are `Attack { target: player }`, `heat[0] == heat_seconds`. Flip-RED: drop the turf check
      from `dwell` → (a) RED; drop `territory_at(muzzle)` from the shot rule → (a) RED; `dwell > warn_seconds` → Warn at
      193 → (b) RED.
    - **`aiming_at_a_member_warns_within_one_cycle`**: `WholeFloor` (inside), member A at (0,0,−10) (10 m >
      `warn_distance`, < `aimed_distance` 15), player aims at A's chest with `aiming = true`, pistol held → `Warn`
      within `slots` ticks; control: same fixture with `aiming = false` → `Idle` after 64 ticks. Flip: ignore
      `aimed_at` → RED.
    - **`attack_on_a_member_aggroes_the_group_within_30m`** (acceptance). **Shooter outside the turf** so heat-on-sight
      cannot put anyone into `Attack` (it needs `player_in_turf`): `WestHalf`, player at (3,0,−10) (none block),
      armour 1e6. Gang 0: V (−2,0,−10) (5 m from the player), M1 (−4,0,−6) (4.47 m from V), M2 (−2,0,19) (29 m from V),
      C (−2,0,21) (31 m from V); gang 1: R (−4,0,−12) (2.83 m from V). Preconditions from config (GATE BROKEN otherwise):
      `|M2−V| < group_radius < |C−V|`; `PlayerTerritory == None` at the hit tick; `territory_at(player muzzle) ==
      None` (so the `ShotFired` rule cannot fire). Aim at V's chest, `fire_requested`, run `Shots::run` tick by tick
      until the `DamageDealt { shooter: player, target: V }` (tick T, ≤ 4 ticks; GATE BROKEN otherwise). In tick T: V,
      M1, M2 `Attack { target: player }`; C `Idle`; R `Idle`; `heat[0] == heat_seconds`, `heat[1] == 0.0`. Then over
      `slots` more ticks C and R stay `Idle`. Flip-RED (deterministic, no slot luck): aggro only the victim → M1/M2
      RED; ignore `group_radius` → C RED; drop the gang filter → R RED.
    - **`hit_outside_the_territory_still_provokes`** (resolved Q1): `WestHalf`, player at (3,0,−10) shoots A at
      (−4,0,−10) → A `Attack` in the hit tick, `heat[0] == heat_seconds`. Flip: require `player_in_turf` for hits → RED.
    - **`gang_heat_decays_in_120s`** (acceptance). `WholeFloor`, single member V (0,0,−10); player at origin shoots V;
      tick T has `heat[0] == 120.0`; despawn V (named mutation, ends the fight). Exact values: `T+1` → 120 − 1/64,
      `T+3840` → 60.0, `T+7679` → 1/64, `T+7680` → 0.0, `T+7681` → 0.0 (multiples of 1/64 below 128 are exact in f32).
      Memory: at `T+7000` (heat 10.625) spawn F at (0,0,−20) (in turf, visible, 20 m > `warn_distance`, <
      `sight_distance`, `from_post` 20) → `Attack` within `slots` ticks, then despawn F; at `T+7681` spawn G at the
      same spot → not `Attack` over 64 ticks. Flip-RED: decay `2·dt` → the `T+3840` value RED; drop `heat > 0` from
      attack-on-sight → G RED.
    - **`gang_heat_keeps_decaying_while_wasted`** (resolved Q3). `WholeFloor`, player shoots V (0,0,−10); after the hit
      tick despawn V; kill the player with `write_damage(app, 1000.0)` and update until `GameState::Wasted` (≤ 3
      updates, GATE BROKEN otherwise; do not use `run_ticks` here — slow motion stalls it, see `respawn.rs`). Record
      `heat0` and `Time<Fixed>::elapsed` at Wasted entry, update while still `Wasted` (≥ 20 updates, stop before it
      ends), then assert: fixed ticks elapsed `n > 0` (GATE BROKEN otherwise) and `heat[0] == heat0 − n/64` exactly.
      Flip: move `decay_gang_heat` into `PlayingSystems` → heat constant → RED.
    - **`disabled_matrix_gangs_do_not_attack_each_other`** (acceptance). `WholeFloor` (gang 0 turf), player at
      (20,0,20) (far, not aiming). Four halves, **each in a fresh `gang_floor` app** (no carried-over swing cooldown,
      health or positions):
      1. Punch, matrix off: A (gang 0) at (0,0,−10), B (gang 1) at (0,0,−11.2). Set A's `AimIntent.direction` toward B
         and `fire_requested` (A is `Idle`, holstered → a real fist swing through `swing_melee` → `apply_strikes`).
         Precondition: `DamageDealt { shooter: A, target: B }` within 30 ticks. Then 64 ticks: A, B never `Attack`,
         heat `[0, 0]`.
      2. Punch, matrix on (positive control): `set_matrix(Gang(0), Gang(1), true)` before spawning; same punch →
         `DamageDealt` seen and B `Attack { target: A }` in that tick.
      3. Shot, matrix off: shooter A = **Gang(1)** at (0,0,−10), listener B = **Gang(0)** at (0,0,−12) (so
         `territory_at(muzzle) == B's gang` — the rule is live, only the matrix blocks it). Set A `held = Pistol`
         directly + aim +Y + `fire_requested` (fired in the next Damage set before the FSM holsters it). Precondition:
         one `ShotFired { shooter: A }`. Then 64 ticks: B `Idle`.
      4. Shot, matrix on: same → B `Attack { target: A }` in the shot tick.
      Flip-RED: `provoke_gangs` ignores the matrix → halves 1 and 3 RED.
17. **`tests/gang_combat.rs`** (new; player armour 1e6; members provoked with `provoke` unless stated):
    - **`attackers_open_fire_at_the_player`**: `WholeFloor`, player at origin; member M (pistol) at (0,0,−12). Within
      `ceil(trigger_seconds.1·64) + slots + 2` = 77 ticks M holds its pistol and ≥ 1 `ShotFired { shooter: M }`;
      over 256 ticks ≥ 2 shots; every `BulletTrace` of M passes within `d·tan(aim_error + base + max_bloom + 0.3°) +
      0.5 m` of the player's chest (d = 12, pistol 4 + 1 + 3 + 0.3 = 8.3° → 2.25 m; M stands, `|v| < 0.5 m/s`
      checked). Flip: aim at chest + (5,0,0) → RED.
    - **`members_hold_the_8_to_15m_band`** (also the "clear line of sight uses direct seek" gate): approach half — M at
      (0,0,−24) (visible, 24 m < `direct_seek_distance`): `RouteLoad.searches == 0` on every tick, distance ≤ 20 after
      128 ticks and in [13.5, 15.5] after 512 ticks. Back-off half (fresh app) — M at (0,0,−4) (outside melee 2.5):
      distance ≥ 5.5 after 128 ticks (walk 1.8 m/s). Flip: `band_move` always Hold → RED; force routing when direct →
      searches > 0 → RED.
    - **`chase_routes_around_the_wall`**: `WholeFloor`, graph n0 (−8,0,20), n1 (−8,0,8), n2 (0,0,8), edges (0,1), (1,2).
      Player at (0,0,4); M at (0,0,20) behind the wall (LOS eyes→player chest crosses x ∈ [−6, 6] at z = 14:
      precondition via `sight_blocked`, GATE BROKEN otherwise). Derivation: nearest node to M = n0 (8 m; n2 12 m, n1
      14.4 m), to the player = n2 (4 m) → route [n0, n1, n2] (first node kept: 14.4 > 12); the path passes x = −8, 1.7 m
      clear of the wall end. Over 400 ticks: some tick has `RouteLoad.searches ≥ 1`, every tick ≤
      `route_requests_per_tick`, M's `sees` becomes true (clear LOS appears around (−8, 17)), and ≥ 1 `ShotFired {
      shooter: M }`. Flip-RED: always direct seek → M presses against the wall, `sees` stays false, no shot → RED.
    - **`wounded_member_retreats`**: M at (0,0,−10), `Health.current = 25` (0.25 < 0.3), provoked → `Retreat` on its
      first Decide; after 384 ticks distance ∈ [24.5, 26.5] and still `Retreat` (regen may lift health after 5 s; the
      Retreat state does not re-score). Flip: `tactics.retreat = 0` → stays `Attack` → RED.
    - **`close_member_punches`**: M 1.2 m from the player (0,0,−1.2), holstered → `held == None` and `Melee.swing` within
      `ceil(trigger_seconds.1·64) + 4` = 75 ticks; a `DamageDealt { shooter: M, target: player }` with the fist damage
      within 128 ticks. Flip: `melee` weight 0 → RED.
    - **`dead_member_drops_its_gun`**: M (pistol) at (0,0,−10), `Health.current = 0` → next tick: `state == Dead`,
      `Dead`, `Corpse`, `TnuaToggle::Disabled`; `Loadout.held == None`, pistol not owned; exactly one new
      `WeaponPickup { Pistol, ammo_only: false }` + `Dropped`, horizontally ≤ 0.01 m from the body, y = feet. Player
      placed on it → next tick the player owns a pistol and the pickup entity is gone. Second member killed at
      (−20,0,−20), its drop left alone: present `drop_seconds·64 − 1` = 3839 ticks after the tick it appeared, gone at
      3840. Flip: ignore `Dropped` in collect → pickup stays with cooldown → RED; `left < 0.0` → one tick late → RED.
18. **`tests/gang_city.rs`** (new, seed 1):
    - **`territories_come_from_the_generator`**: `GangTerritories` exists after load, 2 gangs; posts[0] of each equals the
      recomputed `sidewalk_anchor(GangHq(g), hq_margin)` (world x, curb_height, y); every post `territory_at ==
      Some(g)`; the seed-1 player spawn → `None` (district 4); a point inside a block of the other gang's district →
      that gang.
    - **`groups_spawn_hidden_in_the_ring`** (correctness under a moving view): player placed on the gang-0 post
      **nearest to the HQ post among posts whose HQ distance lies in `spawn_ring`** (seed 1: 44.1 m; GATE BROKEN if
      none); `chase_view` turning 360° every 8 s for 640 ticks. Every tick with new `GangMember`s: distance to the
      player in `spawn_ring`; spot within `spread + 0.05` of a post of its own gang; outside cone + margin **or** the 4
      independent camera rays (copy the `street_spawn.rs` occlusion helper) blocked; group size per (gang, post) in
      `groups.size`. Every tick: alive members ≤ `max_gang_members` and `PopulationLoad.rays ≤ occlusion_rays_per_tick`.
      Non-vacuity: ≥ 1 group spawned during the run. Precondition: some post lies closer than `spawn_ring.0` (so a ring
      flip is visible). Flip-RED: skip the visibility check → "spawned in clear view"; drop the ring filter → a spawn
      inside 30 m; ignore the cap (second run with `max_gang_members = 4`) → cap RED.
    - **`hq_group_spawns_behind_a_static_view`** (liveness, controlled): same player post; `max_civilians = 0` (named
      mutation: no civilian can sit on the HQ spots); static `CameraView` looking from the player **away** from the HQ
      post (`dir = −(hq − player)` flattened). Precondition computed before publishing the view: HQ post in `spawn_ring`
      and every possible HQ spot (sizes lo..=hi) `outside_cone` with margin (GATE BROKEN otherwise). Assert a group with
      `post == 0`, `gang == 0` exists after 2 fixed ticks. Flip: leave post 0 out of the candidate list (the HQ
      post is built but never offered) → RED.
    - **`far_gang_member_despawns_after_2s_offscreen`**: member spawned via the bundle at a post; player 160 m away;
      view up (+Y): present at 127 ticks, gone at 128; a second member 140 m away stays. Flip: `||` for `&&` → RED.
19. **`tests/config.rs`**: `shipped_gang_config_loads` (+ `population.ron` validates with `max_gang_members`,
    `weapons.ron` with `drop_seconds`, `navigation.ron` with the new fields); `unknown_gang_field_names_file_and_field`;
    `sabotaged` fixtures, each strictly on the failing side and asserting its own keyword: a third gang entry
    (`gangs`), the `(a: Gang(0), b: Police, hostile: false),` line removed (`Police`), `keep_distance: (15.0, 8.0)`
    (`keep_distance`), `retreat_health: 1.5` (`retreat_health`), `drop_seconds: 0.0` (`drop_seconds`),
    `route_requests_per_tick: 0` (`route_requests_per_tick`), `direct_seek_distance: -1.0` (`direct_seek_distance`).

### Client (`gta_like`)

20. **`src/visuals/character_config.rs`**: field `gang_models: Vec<String>` (`pub(crate)`); `validate`: non-empty
    ("gang_models must name at least one model"); `resolve` checks every gang model like a civilian model (message
    "gang model {m} resolves clips differently from {model}").
21. **`src/main.rs` `preflight`**: every gang model `manifest.contains_asset` (same loop/message format as civilians).
22. **`src/visuals/character.rs`** (601 lines → ≤ 650):
    - `CharacterAnimations::from_world` chains `config.model`, `civilian_models`, then `gang_models`; doc of `graphs`:
      0 = player, 1..=C civilians, C+1.. gangs.
    - Replace `model_key` / `body_tint` with `body_look(appearance: Option<Appearance>, civilian: bool, gang:
      Option<u8>, config: &CharacterVisualConfig, gangs: &GangConfig) -> (usize, (f32, f32, f32))`: civilian → as today
      (`1 + a % C`, civilian tint); gang → key `1 + C + a % G`, tint `gangs.gangs[gang].tint`; else `(0, config.tint)`.
      `spawn_character_model` and `on_model_ready` query `(Option<&Appearance>, Has<Civilian>, Option<&GangMember>)`
      and take `Res<GangConfig>` (present in every app built by `compose_sim`, including the GLB harness).
23. **`src/visuals/weapons.rs`**: `attach_held_gun` iterates `Query<Entity, With<Loadout>>` instead of `With<Player>`;
    `show_held_gun` reads `Query<&Loadout>`; doc lines say "each armed character". Civilians have no `Loadout`, so
    nothing changes for them. A dead member's `Loadout` is cleared by the sim, so its gun hides; the `HeldGun` is a
    `ChildOf` descendant of the character (`Children` is `linked_spawn` in bevy_ecs 0.19.1 `hierarchy.rs:148`), so a
    despawned member takes its gun with it.

### Client — gates (`cargo test -p gta_like --bin gta_like`; run each touched presentation gate ≥ 3 times)

24. **`src/visuals/civilian_gate.rs`** (424 lines; if it would pass 750, put the new tests in `gang_gate.rs` next to it,
    `#[cfg(test)] mod gang_gate;` in `visuals/mod.rs`): `models()` also chains `gang_models` (so
    `graph_clips_come_from_their_own_model` covers gang graphs) and `require_glbs` covers them.
    - **`every_gang_model_animates_from_its_own_clips`** in the GLB harness (`MinimalPlugins + TransformPlugin +
      AssetPlugin{file_path} + StatesPlugin + ImagePlugin + MeshPlugin + AnimationPlugin + WorldSerializationPlugin +
      GltfPlugin` + `init_asset::<StandardMaterial>()` + `compose_sim` + `CharacterVisualsPlugin`): one member per gang
      model key (`Appearance(k)` so `k % G` hits each key; gangs alternate 0, 1, 0), no `GangTerritories` so the gang
      AI is off (`gang_death` stays on, harmless) and the test drives each member's `MoveIntent` (walk forward). Assert
      per member key `1 + C + k` and graph handle `graphs[1 + C + k]`, `leg-left` turn range > 0.1 rad over 32 updates
      (range, not two snapshots), and the tinted mesh's `base_color` equals the untinted source material ×
      `GangConfig.gangs[gang].tint` (linear, 1e-4). Flip: wire every gang animator with `graphs[0]` → leg turn 0 → RED;
      use `config.tint` for gangs → tint RED.
    - **`gang_held_gun_follows_loadout_and_owner`**: same harness plus `RenderConfig` (`load_config`) and
      `init_resource::<WeaponVisualAssets>()`, systems `attach_held_gun` + `show_held_gun` in `Update`. After models are
      wired: each member has exactly one `HeldGun` with `owner == member`, `Visibility::Hidden` (holstered); set one
      member's `Loadout.held = Some(Pistol)` → after 1 update its gun is `Inherited`; despawn that member → after 1
      update no `HeldGun` with that owner exists. Flip: revert `attach_held_gun` to `With<Player>` → RED.
    - **`gang_models_are_validated`**: a non-rig gang model is rejected by `resolve`; empty `gang_models` rejected by
      `validate`, naming the field.

### Runtime QA and owner run

25. **`tools/qa/scenarios/t9.py`** (new, stdlib, imports `brp` and helpers from `t5`/`t6`/`t8`: `resource_value`,
    `screenshot`, `log_errors`, `wait_chunks`, `game_state`, `rows`, `player`, `teleport`, `weapon_pickups`, `variant`,
    `scalar`, `horizontal`):
    1. `Game(features=("dev",), args=("--seed", "1"), release=True)`; wait `Playing`, `CityLayoutHash` == golden,
       `wait_chunks`.
    2. `territories = resource_value(game, "GangTerritories")`; gang 0 posts; HQ = posts[0]; ring from
       `population.ron`.
    3. Pistol: t6 pickup flow until `held == Pistol` (filter out any pickup carrying `Dropped`).
    4. Approach unseen: teleport to the gang-0 post nearest to the HQ among posts with HQ distance in `spawn_ring`
       (seed 1: 44.1 m; fail with a message if none); point the camera away from the HQ:
       `mutate_component(camera, OrbitCamera, "yaw", atan2(−d.x, −d.z))` with `d = post − hq` (§2.0 convention).
       Poll `GangMember` rows until a group with `post == 0`, `gang == 0` exists (deadline 15 s).
    5. Teleport 12 m from the HQ group's centroid along the line centroid → approach post; require `PlayerTerritory ==
       0` there, else move 2 m closer (≤ 3 tries); yaw toward the group; wait 0.5 s; read states → `before` (all
       `Idle`/`Warn`). Screenshot `hq.png`.
    6. Aim into the sky (`move_mouse` as t8) and one LMB click; confirm the magazine dropped; wait 0.3 s; read every gang
       row within `group_radius` (from `gangs.ron`) of the centroid → `after`; read `GangHeat`. Screenshot `aggro.png`
       (≥ 0.15 s after the previous capture).
    7. Firefight evidence: wait 3 s, read player `Health`, count gang `Loadout.held` drawn, screenshot `firefight.png`.
    8. Hard pass: HQ group spawned; `before` has no `Attack`; every member in `after` is `Attack`; `GangHeat.left[0] >
       110`; `PlayerTerritory` was 0 at the shot; zero `ERROR` log lines. Soft evidence: player health drop,
       `get_diagnostics` FPS.
    Re-run `t8.py` once as a regression. Note in QA_REPORT that `GangHeat` seconds are fixed-time seconds (slower in
    wall-clock time during the Wasted slow motion).
26. **Owner checklist** (QA writes it into QA_REPORT.md, Russian): "зашёл к бандитам, спровоцировал, получил
    перестрелку"; обе банды читаются по тинту и отличаются от мирных; предупреждение читается как угроза (достали
    ствол, целятся, подходят); группы не появляются в кадре; в перестрелке держат дистанцию, мажут чаще игрока, бьют в
    упор, раненые отходят; погоня обходит угол дома, не трётся о стены; убитый бандит роняет ствол, его можно поднять;
    FPS рядом с перестрелкой.

### Order and checks

27. Steps 1–6, 19 → `cargo test -p gta_sim --test config`. Steps 7–12 → `cargo build`, `cargo clippy -- -D warnings`,
    `cargo clippy -p gta_sim --tests -- -D warnings`, `cargo test -p gta_sim --lib` (navigation, territory, fsm tables).
    Steps 13–18 → full `cargo test -p gta_sim` (every existing test green; report `street_ahead_stays_populated`'s mean
    and window spread next to its TASK-022 value 7.63 / 3.26..8.60, and the civilian bench tick mean before/after — a
    change is a finding, not an expected effect; do not re-anchor silently). `cargo test -p citygen` once (untouched).
    Steps 20–24 → `cargo test -p gta_like --bin gta_like` ×3. `python tools/qa/tree_check.py`,
    `cargo tree -p gta_sim -e normal -i bevy_render` (empty). Step 25 last, release build. Before trusting a red from the
    shared `target/`, `touch crates/*/src/lib.rs` and rebuild (phantom-red lesson).

---

## 3. Test plan (claim → gate → class)

| Claim | Gate | Class |
|---|---|---|
| Neutral outside the territory (acceptance) | `outside_the_territory_members_stay_neutral` | correctness + positive control |
| Attack on a member → group within 30 m in `Attack` (acceptance) | `attack_on_a_member_aggroes_the_group_within_30m` (shooter outside turf: no heat-on-sight path) | correctness, exact tick |
| `GangHeat` decays in 120 s (acceptance) | `gang_heat_decays_in_120s` | correctness, exact ticks + memory effect |
| Heat keeps decaying during Wasted (Q3) | `gang_heat_keeps_decaying_while_wasted` | correctness, fixed-tick count |
| Disabled matrix: no gang-on-gang attacks (acceptance) | `disabled_matrix_gangs_do_not_attack_each_other` (punch + shot, each with a fresh positive control) | correctness |
| A hit outside the turf provokes (Q1) | `hit_outside_the_territory_still_provokes` | correctness |
| Warning: 3 s within 8 m, or aimed at | exact tick 192 in the neutrality test; `aiming_at_a_member_warns_within_one_cycle` | correctness |
| Territories from the generator | `every_sweep_seed_has_two_territories` (33 seeds), `territories_come_from_the_generator` | correctness |
| FSM, scorer, band, matrix rules | `fsm.rs` tables | correctness |
| A* route, waypoint turns N/W/S, corner skip | `navigation` unit tests | correctness |
| Chase routes around a wall; no search at clear short range; search budget | `chase_routes_around_the_wall`, `members_hold_the_8_to_15m_band` | correctness + bound |
| Fight: distance, spread, melee, retreat | `gang_combat.rs` | liveness + correctness |
| Weapon drop, one-shot, lifetime | `dead_member_drops_its_gun` | correctness |
| Groups never spawn in view; ring; cap; ray budget; HQ gets a group | `groups_spawn_hidden_in_the_ring`, `hq_group_spawns_behind_a_static_view`, `far_gang_member_despawns_after_2s_offscreen` | correctness + bound + liveness |
| Config strictness | `tests/config.rs` fixtures | correctness |
| Gang models animate from their own clips; gang tint from `gangs.ron` | `every_gang_model_animates_from_its_own_clips` (real GLBs) | correctness |
| Held gun on NPCs follows `Loadout`, leaves with its owner | `gang_held_gun_follows_loadout_and_owner` | correctness |
| Runtime: HQ, shot, `Attack`, screenshots (acceptance) | `t9.py` | runtime |
| Tints, warning read, firefight feel, avoidance smoothness | owner checklist | owner |

Declined on purpose: a dedicated gang perf bench (≤ 12 agents; A* measured 11 µs mean, ≤ 2 searches/tick → ≤ 0.4 ms
worst; LOS and avoidance rays on AI slots ≤ 3 members × 8 rays per tick; spawn rays under the shared budget asserted
in `groups_spawn_hidden_in_the_ring`). The existing civilian bench number is reported before/after instead.
Avoidance rays have no dedicated gate: their effect is owner-visible; the stall case they cannot solve is gated by
routing.

---

## 4. Rollout notes

- **New dependency:** `pathfinding = "=4.16.0"` in `gta_sim` (pure Rust, no Bevy, MIT/Apache). `Cargo.lock` changes;
  builds offline from the cache. `cargo tree -p gta_sim -e normal -i bevy_render` stays empty.
- **Data files:** new `assets/gang/gangs.ron`; new required fields `max_gang_members` (`npc/population.ron`),
  `drop_seconds` (`combat/weapons.ron`), five navigation fields (`npc/navigation.ron`), `gang_models`
  (`character/visual.ron`). Strict loaders: a stale file fails at startup naming the file and field. No saves exist.
- **No feature flag.** Gangs exist only when `GangTerritories` exists (city runs); the test area has none, so every
  existing test-floor gate is unaffected. `max_gang_members: 0` disables gang spawning in data.
- **Determinism:** gangs roll from `GangRng` (ChaCha stream 2); `NpcRng` and `spawn_civilians` are untouched, so
  civilian roll sequences keep their order. The seed-1 street density gate can still move through physical coupling
  (members on corners, shared ray budget); report it, do not re-anchor.
- **Existing QA scenarios** (`t6`/`t7`/`t8`) may meet gang groups if their route is inside a territory (seed-1 HQs
  are 458 m / 566 m from the spawn; the central range is not in a gang district). t8 is re-run.
- **Backward compat:** `WeaponPickup` keeps its fields; regular range pickups keep the cooldown behaviour.
  `attach_held_gun`/`show_held_gun` widen from `Player` to any `Loadout` holder (only player and gang members have one).
- BRP-visible: `GangMember`, `GangState`, `Faction`, `Dropped`, `Route` components; `GangHeat`, `PlayerTerritory`,
  `GangTerritories`, `RouteLoad` resources (all `Reflect` + registered).

---

## 5. Review notes (changes against PLAN_V2.md and PLAN.md)

**Disconfirmation tested first.** Most concrete counter-example to PLAN_V2: its blocking finding #1 ("`gang_death` in
`HealthSystems::Death` and in `GangSystems ⊂ NpcSystems` is a schedule cycle"). Checked in code: `civilian_death` is
already registered `.in_set(HealthSystems::Death).in_set(NpcSystems)` (`civilian/mod.rs`, `CivilianPlugin::build`)
while `perception/mod.rs` orders `(Perceive, Decide, PopulationSystems).in_set(NpcSystems).after(HealthSystems::Death)`
— and the game runs. The bevy_ecs 0.19.1 errors (`CrossDependency`, `SetsHaveOrderButIntersect`, `schedule/error.rs`)
need a set ordered against its own member or two ordered sets sharing a system; neither `NpcSystems` nor `GangSystems`
is ordered against anything. **The counter-example held: V2's cycle diagnosis is wrong.** Its constraint ("not in
`GangSystems`") is harmless and kept; its prescription (`PlayingSystems`) is replaced by the precedent (`NpcSystems`).

Changes, each verified:
1. **Schedule:** `gang_death` in `HealthSystems::Death` + `NpcSystems`, not `GangSystems` (above).
2. **A\* (V2 #2, kept and made concrete):** V2 left "bounded synchronous vs async" open pending a measurement. Measured
   now (`scratch/probe_astar/output.txt`): 11 µs mean, ≤ 215 µs worst per search on seed 1 → **synchronous, at most
   `route_requests_per_tick` = 2 per tick** (GDD §11 queue-K rule), no `AsyncComputeTaskPool`. Found that
   `pathfinding::astar` requires `C: Ord` → integer cm costs (f32 would not compile). Defined `Route`, `plan_route`,
   `route_point`, `nearest_node`, `avoid_offset`, `RouteLoad`, the "head for" helper and the worked N/W/S example.
   V2's separate "stuck threshold" knob dropped: the `route_refresh_seconds` re-plan covers blocked progress; one knob
   fewer (YAGNI). V2's "use edge sample points if corners fail" dropped: GDD §6.3 places groups "у штаба и на углах
   кварталов" — corners are exactly the nodes.
3. **Colour ownership (V2 #3, kept):** tint per gang in `gangs.ron` (`GangSpec.tint`); no `gang_tints` in `visual.ron`,
   no cross-file count check in `preflight`; client reads `GangConfig`.
4. **Group-aggro gate (V2 #4, kept, recomputed):** PLAN's layout left M1 (12.6 m) and M2 (39 m) inside `sight_distance`
   40, so with the propagation broken one of them could still enter `Attack` via heat-on-sight on its slot — the flip
   was slot-luck dependent. Final gate puts the shooter outside the turf (heat-on-sight needs `player_in_turf`), new
   coordinates checked against the floor fixtures.
5. **Matrix gate (V2 #5, kept, plus a new defect):** each half runs in a fresh app. New: PLAN's shot variant was a
   tautology — B was gang 1 on a gang-0 `WholeFloor`, so `territory_at(muzzle) != B's gang` blocked the shot rule
   whatever the matrix said. Final variant: shooter Gang(1), listener Gang(0) on gang-0 turf, with its own positive
   control.
6. **Spawner/QA liveness (V2 #6, kept, recomputed):** PLAN placed the player "on the post 50–70 m (QA: 50–80 m) from the
   HQ" — the probe shows gang 0 has **no** post in that range (44.1, 46.2, then 104.6 m), so both would be GATE BROKEN.
   Replaced by "nearest non-HQ post with HQ distance in `spawn_ring`" (44.1 m). Liveness split into a static-view gate
   with `max_civilians = 0` and an eligibility precondition; the rotating-view gate keeps correctness with a
   non-vacuity check.
7. **Held-gun cleanup (V2 #7):** verified no leak by construction (`HeldGun` is a `ChildOf` descendant; `Children` is
   `linked_spawn`). PLAN had declined an NPC held-gun gate as "needs a real hand joint" — the GLB harness has real joints,
   so `gang_held_gun_follows_loadout_and_owner` is added.
8. **Resolved questions (V2 #8, kept):** Q1 hits always provoke, Q2 Warn draws/aims/walks, Q3 heat decays during Wasted
   (new gate `gang_heat_keeps_decaying_while_wasted`), Q4 defaults in `gangs.ron`. PLAN §6 open questions removed.
9. **New: NPC pull while the select is pending.** In Shoot, `fire_requested` is set only when `loadout.held ==
   Some(gun)`; otherwise `swing_melee` (which runs before `fire_weapons` and consumes the click when `held` is `None`)
   would turn the shot into a punch. `trigger_left` counts down in every Attack/Retreat tick (clamped at 0) so its
   bound in the fire gate is derivable.
10. **New: fight tests could kill the player.** PLAN gave the player 100 armour; a pistol member over 512 ticks can deal
    ~16 × 25 = 400 → Wasted → target dead → `Idle`, breaking the band/retreat gates. Armour set to 1e6 (named mutation;
    `apply_damage` does not clamp armour). Fight/route gates provoke via a named `provoke` mutation.
11. **Boundary fixes:** the band approach starts at 24 m, not 25 (25 m is the direct-seek boundary). `expire_dropped` is
    ordered after `collect_weapon_pickups` so the last-tick outcome is defined.
12. **Detail restored from PLAN.md** where V2 compressed it: RON contents, type definitions, validation rules, FSM rules
    and tables, worked yaw examples, exact tick numbers, test coordinates, flip-RED perturbations, QA steps.
13. PLAN.md line references were wrong (e.g. `perception/mod.rs:442-452` for code at 136–152 of a 289-line file); this
    plan names symbols instead.

Residual risks (for the implementer and QA): seed-1 street density coupling (report); members on post nodes can meet
civilians spawned on the same node (accepted, no overlap); avoidance is a 7-ray heuristic whose look is owner-judged;
A* on a coarse corner graph can take a longer way around a block than a navmesh would (T11 decides navmesh by fact,
GDD §6.6); SMG fires single rounds per pull (owner may want bursts later — data knob, not this slice).

children: 0 launched / 0 reported.
