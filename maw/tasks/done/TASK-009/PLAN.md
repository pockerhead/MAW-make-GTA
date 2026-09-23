# PLAN — TASK-009 (GDD T8): мирные жители

Pinned versions (from `Cargo.lock`, sources under `~/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/`):
bevy / bevy_ecs / bevy_state / bevy_gltf / bevy_camera 0.19.1, avian3d 0.7.0, bevy-tnua 0.32.0
(+ bevy-tnua-physics-integration-layer 0.13.0), vendored `vendor/bevy-tnua-avian3d-0.12.1`, rand_chacha 0.10.0.
**No new crate and no `Cargo.lock` change.** `pathfinding` 4.16.0 (GDD §10.1) is NOT added: civilians never run
A* (GDD §6.2: wander "без A*", flee is greedy per node); A* arrives with police (T11).

Executable planner probes (scratch copy of the workspace `scratch/probe_ws/`, built with
`CARGO_TARGET_DIR=D:/test-gta-like/target`, nothing in the checkout touched):
- `scratch/probe_ws/crates/gta_sim/tests/probe_npc.rs` → `scratch/probe_npc_dev.log` (`cargo test` profile) and
  `scratch/probe_npc_release.log` (`--release`).
- `scratch/probe_ws/crates/gta_sim/tests/probe_corpse.rs` → `scratch/probe_corpse.log`.

Cost of error. Silent defects get headless gates: a civilian walking off the sidewalk graph or onto a non-adjacent
node, perception that never fires or fires only for one slot, a spawn inside the camera view or outside the ring, a
despawn before 2 s off-frame or inside 150 m, corpses piling up past the limit, a corpse still blocking bullets and
walkers, a per-tick cost that explodes with N. The crowd "feels alive", the crouch/death clips, the tint look and the
FPS with 40 skinned civilians are seen by the owner on the first run: owner checklist + BRP evidence, no machinery.

---

## 1. Understanding (what exists today)

**Sim (`crates/gta_sim`)**
- `lib.rs:28-93` `compose_sim`: loads + validates every RON config (`load_config` + `validate`), inserts them as
  resources before `add_plugins`; `:64-67` `combat_seed` = city seed (0 for `TestArea`); `:83-92` adds
  `FlowPlugin, PhysicsPlugins, TnuaAvian3dPlugin(FixedUpdate), CharacterPlugin, WorldPlugin, PlayerPlugin,
  CombatPlugin{seed}, WantedPlugin`. There is no `navigation/`, `perception/`, `population/`, `civilian/` yet and no
  `assets/npc/` directory.
- `world/city.rs:68-120` `apply_city_generation` (Update, `Loading`): polls the async `CityGenTask`
  (`AsyncComputeTaskPool`, `block_on(poll_once)`), then inserts `City(CityLayout)` (`:117`), `PlayerSpawn`,
  `HospitalSpawn`, `CityLandmarks`, and sets `GameState::Playing`. `WorldSource::TestArea` spawns the fixed 80×80
  floor + blocks (`world/test_area.rs`), no `City` resource.
- `citygen::CityLayout.sidewalks: WalkGraph { nodes: Vec<Vec2>, edges: Vec<(u32,u32)> }` (`citygen/src/layout.rs`,
  built in `citygen/src/graphs.rs:32-84`): per block a closed ring on the sidewalk centre line (`walk_offset` =
  half carriageway + sidewalk/2, `:28-30`; alley sides run along the curb), park blocks add a centroid node linked to
  every ring node, crossings link block corners across each road end. Layout (x, y) = world (x, z). Sidewalk top
  is `roads.curb_height` = 0.15 m (`assets/world/city.ron`), carriageway at y = 0.
  Probe, seed 1: **579 nodes, 1134 edges, 59 278 m, one connected component, no isolated node, max degree 6,
  shortest edge 4.5 m; 35 edges (1604 m) have their midpoint in the 60-120 m ring around the player spawn**.
- `character/mod.rs:28-38` `Character` requires `MoveIntent, AimIntent, ActionIntent, JumpBuffer, AnimState,
  HitReaction, Melee`; `:127-149` `character_components` (dynamic capsule r 0.3 h 1.5, `CollisionLayers(Character,
  ALL)`, head sensor child on `Hitbox`); `:154-222` `drive_characters` (FixedUpdate, `TnuaUserControlsSystems`,
  **not** gated by `PlayingSystems`) turns `MoveIntent { axis, yaw, gait }` into the Tnua walk basis
  (`move_direction`: `R_y(yaw)·(x, 0, −y)`), zeroes it for `Dead` or an active `HitReaction`.
- `character/health.rs`: `Health::take`, `Dead` marker, `HealthSystems::{Damage, Regen, Pickup, Death}` chained
  (`character/mod.rs:92-100`).
- `combat/hitscan.rs:55-62` `ShotFired { shooter, weapon, muzzle }` (Message) written by `fire_weapons`
  (`:221-225`) on every trigger pull that fired, including one into the sky; `:66-86` `DamageDealt { shooter, shot,
  target, point, damage, headshot, killed }` for every hit on a live `Health` (any character, not only dummies);
  `:100` private `unit_f32`; `:105` `pub fn aim_yaw(direction) = atan2(−x, −z)`. Hitscan skips the head sensor of a
  `Dead` body (`:193-201`).
- `combat/melee.rs`: `MeleeHit { attacker, target, point, knockdown }` (Message) per applied punch; `swing_melee`
  and `fire_weapons` only act on characters that have a `Loadout` (the player; dummies and future civilians have none).
- `combat/mod.rs:80-104`: all combat systems in `HealthSystems::Damage` / `Pickup` / `Death`, `.in_set(PlayingSystems)`.
- `combat/range.rs:78-100` `dummy_life`: the only NPC death handling today (insert `Dead`, revive after reset; the
  dead dummy keeps its Tnua controller and its standing capsule).
- `flow/mod.rs:29,44-47` `PlayingSystems` = FixedUpdate set `run_if(in_state(Playing))`; `GameState = Loading |
  Playing | Wasted`. During `Wasted` fixed ticks keep running (virtual time 0.3×, then 1×) and `drive_characters` keeps
  consuming whatever `MoveIntent` holds.
- `player/mod.rs`: `Player` marker, `spawn_player` on `OnTransition{Loading→Playing}` (keyed one-shot, TASK-006).
- Tests: `tests/common/mod.rs` (`composed_app`, `headless_app`, `city_app(seed)`, `run_ticks` counting
  `Time<Fixed>`, `spawn_dummy`, `set_aim`, `set_action`, `set_loadout`, `Shots` message cursors), `tests/config.rs`
  (shipped-config loads + `sabotaged` unknown-field / range fixtures), `tests/city.rs`, `tests/shooting.rs`.

**Client (`src/`)**
- `camera/mod.rs:89-156` `follow_player` (PostUpdate, before transform propagation) writes the player's `AimIntent`
  from the camera; FOV lives only in `camera.ron` (70°, aim 55°) and in the camera's `Projection::Perspective`
  (`fov`, `aspect_ratio`, `bevy_camera-0.19.1/src/projection.rs:283-309`). The sim knows nothing about the view.
- `visuals/character.rs`: observer `spawn_character_model` (`:156-176`, `On<Add, CharacterBody>`) puts
  `character-male-a.glb` under every body; one shared `AnimationGraph` (`CharacterAnimations`, `:57-133`) built from
  that GLB's clips; `drive_character_animation` (`:308-393`) picks locomotion by `AnimState`, full-body actions
  (`ShownAction::{Swing, Knockdown}`) by `Melee`/`HitReaction`. **`Dead` is not handled**: a dead dummy keeps its
  locomotion clip. `tint_mesh` (`:281-305`) multiplies `tinted_mesh` by `visual.ron` `tint`.
- `visuals/character_gate.rs`: headless presentation harness (`character_visuals_app`).
- Rig clips (`assets/third_party/manifest.ron` rig): … 6 `crouch`, … 9 `die` … All 12 Kenney models are on disk.
- `tools/qa/brp.py` (`Game`, `query`, `send_mouse_button`, `move_mouse`, `screenshot`, `diagnostics`),
  `tools/qa/scenarios/t6.py` (`rows`, `player`, `teleport`, `aim_at`, `weapon_pickups`, pistol pickup flow),
  `t5.py` (`screenshot`, `wait_chunks`, `log_errors`).

**Verified engine facts this plan relies on**
- Tnua cost (probe, `probe_npc_*.log`, per fixed tick, mean over 640 ticks):
  | scene | N walkers | test profile | release |
  |---|---|---|---|
  | flat test floor | 0 / 16 / 32 / 64 / 128 | 462 / 738* / 675 / 807 / 944 µs | 452 / 563 / 639 / 783 / 924 µs |
  | seed-1 city | 0 / 64 | 557 / 756 µs | 509 / 698 µs |
  (*one 5.5 ms OS spike in that run.) ⇒ **≈ 3 µs per Tnua NPC per tick in the city, linear in N**; spawning 64 at
  once costs one 1.07 ms tick. One 20 m avian ray in the city ≈ 0.6 µs (1000 rays 0.57-0.60 ms). Per-tick **max**
  has OS spikes of 6-8 ms in both profiles → only the mean is gateable. The test and release profiles are within 10%
  because every dependency is `opt-level = 3` (`Cargo.toml [profile.dev.package."*"]`).
  GDD §10.2 fallback "kinematic far NPCs" is **not needed**.
- Corpse (probe, `probe_corpse.log`): inserting `(TnuaToggle::Disabled, RigidBody::Static, CollisionLayers::NONE)`
  on a standing character: moved 0.00000 m in 128 ticks, a chest ray `[World, Character, Hitbox]` that hit it before
  now misses, a walker walks straight through its spot, `try_despawn` removes its head-hitbox child.
  `TnuaToggle::Disabled` = "do not update the sensors, do not apply motor forces, the controller system does not run"
  (`bevy-tnua-physics-integration-layer-0.13.0/src/data_for_backends.rs:8-21`, re-exported as `bevy_tnua::TnuaToggle`,
  `bevy-tnua-0.32.0/src/lib.rs:202`); avian re-initialises a body on `On<Insert, RigidBody>`
  (`avian3d-0.7.0/src/dynamics/solver/solver_body/plugin.rs:133`, `collider_tree/update.rs:208`).
- `EntityCommands::try_despawn` (`bevy_ecs-0.19.1/src/system/commands/mod.rs:1921`); `Children` is
  `linked_spawn` (`bevy_ecs-0.19.1/src/hierarchy.rs:148`) so children go with the parent.
- `SystemCondition::or` (`bevy_ecs-0.19.1/src/schedule/condition.rs:537`), `in_state`
  (`bevy_state-0.19.1/src/condition.rs:103`), `resource_exists` (`condition.rs:730`).
- `ChaCha8Rng::set_stream` (`rand_chacha-0.10.0/src/chacha.rs:179`).
- **glTF animation targets include the root node name** (`bevy_gltf-0.19.1/src/loader/mod.rs:559-563` for clip
  curves, `:1545-1558` for scene nodes: the path starts with the animation-root node). The 12 Kenney GLBs have
  identical joints but root nodes named `character-male-a`, `character-female-b`, … (parsed from the GLB JSON).
  Clips loaded from `character-male-a.glb` do not drive any other model. → civilians use the same model with a tint
  (§2, Open question 1). PCTX proposal filed.

## 2. Approach

Four new sim domains, one plugin each, exactly the GDD §12 map: `navigation/` (graph + steering,
`navigation.ron`), `perception/` (stimuli, slices, `perception.ron`), `population/` (bubble, spawn/despawn, corpses,
`population.ron`), `civilian/` (FSM, utility reaction, `civilian.ron`). Client: a view-cone writer, death/cower clips,
civilian tint. QA: `t8.py`.

**Graph.** `SidewalkGraph` resource (nodes as `Vec3` with y = curb height, adjacency `Vec<Vec<u32>>`, edge list),
built once from `City` on `OnTransition{Loading→Playing}` (measured 20-30 µs, inline under the loading screen).
Tests on the test floor insert a hand-built graph through the same constructor.

**Movement.** A civilian is a normal Tnua `Character` (one archetype with the player, GDD §3.1) without `Loadout`.
The AI only writes `MoveIntent { axis: (0,1) or 0, yaw, gait }` — the same path as player input, so knockback,
stagger, headshots and damage work on civilians for free. `GraphWalker { from, to }` keeps the invariant "the
civilian is on edge (from, to) and `to` is adjacent to `from`"; the civilian seeks `graph.node(to)` and on arrival
(`arrive_radius`) picks the next node: Wander = random neighbour, not the one it came from unless it is a dead end;
Flee = the neighbour farthest from the threat (GDD §6.2). No A*, no separation steering (Tnua capsules already push
apart; decision logged), no stuck detector (Risk R4).

**FSM** (`CivilianState` enum field on `Civilian`, never an archetype move — GDD §6.5):
`Wander → Idle{left} → Wander` (idle on node arrival with `idle_chance`, 2-8 s), threat from a calm state
(`Wander`/`Idle`) → utility choice `Flee{from, left_m}` / `Cower{from, left}` / `Report{left}`;
`Flee` ends after 30-60 m → `Wander`; `Cower` ends after 3-6 s → `Flee` from the same point (get up and run);
`Report` (4 s call, T10 turns it into heat) is interrupted by any new threat → re-score **without** report (GDD §6.2
"звонок прерывается"); a new threat during `Flee`/`Cower` refreshes the threat point and restarts that state's
distance/timer, no re-scoring (commitment, below); `Health ≤ 0` from any state → `Dead`.

**Utility reaction** — pure `choose_reaction(threat, temperament, cfg, allow_report) -> Reaction` (Graham, "An
Introduction to Utility Theory", Game AI Pro ch. 9: score each option with a response curve, pick the max;
https://www.gameaipro.com/GameAIPro/GameAIPro_Chapter09_An_Introduction_to_Utility_Theory.pdf). With proximity
`p = clamp(1 − d / panic_distance, 0, 1)` and a per-NPC temperament `t` rolled at spawn
(`t.x = 1 + spread·(2u − 1)`, "разные NPC реагируют по-разному"):
- `flee = w.flee · t.flee`
- `cower = w.cower · t.cower · p`
- `report = w.report · t.report` if allowed, else 0. Allowed: `Corpse` at any distance; `Gunshot`/`Fight` only when
  `d ≥ report_min_distance`; never for `Aimed`/`Hurt` (you do not phone with a gun on you) and never when re-scoring
  an interrupted call. Ties resolve Flee > Cower > Report.
Scoring happens only on entry from a calm state or an interrupted call — the "commitment" remedy against utility
oscillation (Mark & Lewis, GDC 2015 "Building a Better Centaur", https://www.gdcvault.com/play/1021848/Building-a-Better-Centaur-AI).
Shipped weights make the acceptance case deterministic: a gunshot at 20 m has `p = 0` (panic_distance 15) and report
blocked (report_min_distance 25) ⇒ always `Flee` (worked table in Step 12).

**Perception** (GDD §6.2, §6.5, §11). Stimulus kinds: `Gunshot` (`ShotFired.muzzle`, hearing 40 m), `Fight`
(`MeleeHit.point`, `fight_hearing_radius`), `Corpse` (a `Corpse` in line of sight ≤ 20 m), `Aimed` (a live
character with `Loadout.held.is_some()` and `AimIntent.aiming` whose aim ray passes within `aimed_cone_deg` of the
NPC chest, ≤ 15 m, line of sight), `Hurt` (a `DamageDealt` whose target is this NPC). "Машина, въехавшая на тротуар"
needs vehicles (T14): out of this slice, noted for T14.
- **Time-slicing.** Each NPC gets a slot `0..slots` round-robin when its `Perception` is added (observer). A tick
  counter `AiClock.tick` advances once per fixed tick; an NPC perceives when `tick % slots == slot` (slots = 4 →
  1/4 of the agents per tick, 16 Hz at 64 Hz, GDD §6.5 `AiTickSlot`).
- **No missed events without per-NPC bookkeeping.** Sound stimuli from messages go into a `StimulusLog` resource
  stamped with the tick and are kept for exactly `slots` ticks. Any `slots` consecutive ticks contain exactly one tick
  of every slot ⇒ every NPC sees each sound exactly once, within one perception cycle of it. (Same idea as the
  stimulus "Max Age" short-term memory of Unreal's AI Perception,
  https://dev.epicgames.com/documentation/en-us/unreal-engine/ai-perception-in-unreal-engine.) Sight checks
  (`Corpse`, `Aimed`) are evaluated at the NPC's slot tick against current world state. `Hurt` bypasses slicing: the
  victim reacts in the tick of the hit.
- Result: the nearest threat of the cycle goes into `Perception.pending: Option<Threat { kind, at, distance }>`
  (field write), consumed by the civilian FSM in the same tick. LOS rays: mask `World` only, from the NPC's eyes
  (`LocomotionConfig.head_height` above the feet) to the target chest.

**Population bubble** (GDD §6.1, §11). Cap 40 alive civilians (corpses do not count). Spawn: while under the cap, at
most `spawns_per_tick` per tick, a random point on an edge that crosses the 60-120 m ring around the player (edge
candidates rebuilt from the 1134 edges only when there is a deficit, O(E) ≈ tens of µs), accepted when the point is
in the ring, **outside the view cone** widened by `spawn_view_margin_deg`, and ≥ `spawn_min_separation` from every
civilian; up to `spawn_attempts` tries. GDD says "на узлах графа": nodes are too sparse (edges ≥ 4.5 m, only 35 edges
in the ring) and two spawns on one node overlap capsules — edge points keep the rule's intent (on the sidewalk graph,
off-frame, in the ring); decision logged. Despawn: every civilian (alive or corpse) accumulates `offscreen_for`
while outside the view cone (reset to 0 inside it); despawned when horizontal distance to the player
> `despawn_distance` (150) **and** `offscreen_for ≥ despawn_offscreen_seconds` (2 s) (GTA SA rule, GDD §6.1).
Corpses: `Corpse { age }`, despawned at `corpse_seconds` (30) and the oldest beyond `corpse_limit` (20) (GDD §4.3).

**"In frame" without a renderer.** New sim resource `ViewCone { origin, forward, half_angle }` in `population/`,
written by the client camera every frame (PostUpdate, after `follow_player`), exactly like `AimIntent`: derived
state, camera geometry stays owned by `camera.ron`. The cone is the bounding cone of the frustum through its corner:
`half_angle = atan(√(tan²(fov_y/2) + (aspect·tan(fov_y/2))²))` (gamedev.net "Constructing a cone for the view
frustum", https://gamedev.net/forums/topic/283251-constructing-a-cone-for-the-view-frustum/): 70°, 16:9 → 55.00°;
55° aim FOV → 46.72°; 90°, 1:1 → 54.74°. Occlusion is ignored (a civilian behind a building counts as in frame —
conservative for both spawn and despawn). Default `ViewCone` has `forward = ZERO` ⇒ nothing is in frame (headless).

**Death** (GDD §4.3 A). `Health ≤ 0` on a civilian → `state = Dead`, insert `(Dead, Corpse, TnuaToggle::Disabled,
RigidBody::Static, CollisionLayers::NONE)` — one archetype move per death (probe above): the body stays where it
fell, no longer blocks bullets or walkers, Tnua is off ("контроллер снимается"). The client plays `die` once for
every `Dead` character (player and dummies too — they had no death clip at all).

**Schedule** (FixedUpdate). New `NpcSystems` set declared in `flow/` next to `PlayingSystems`, configured
`run_if(in_state(Playing).or(in_state(Wasted)))` — NPCs keep living during the death slow-mo; gating them with
`PlayingSystems` would freeze the AI while `drive_characters` keeps walking every NPC straight along its last
`MoveIntent` for ~4.5 s (decision logged). `NavigationPlugin` adds `run_if(resource_exists::<SidewalkGraph>)` to the
same set (conditions on a set combine with AND). Inner order, all chained with explicit `.chain()` on flat tuples
(TASK-008 lesson):
`HealthSystems::Damage` (shots, hits, `DamageDealt` written) → `AiSystems::Perceive` (`advance_clock`,
`collect_stimuli`, `perceive`) → `AiSystems::Decide` (`civilian_fsm` = state + steering → `MoveIntent`) →
`TnuaUserControlsSystems` (existing `drive_characters`). `HealthSystems::Death`: `civilian_death`; after it
`PopulationSystems` (`age_corpses`, `despawn_far`, `spawn_civilians`). Messages used: the existing buffered
`ShotFired` / `MeleeHit` / `DamageDealt` (`MessageReader`); no new message, no observer event except the
`On<Add, Perception>` slot assignment (an immediate reaction to a component insert).

**Inline vs time-sliced vs async** (the bevy-ecs domain question, answered with the probe numbers):
| work | where | why / measured bound |
|---|---|---|
| Tnua + avian for N NPCs | inline (physics) | ≈ 3 µs/NPC/tick ⇒ 40 NPC ≈ 0.12 ms, 64 ≈ 0.2 ms over a 0.56 ms city baseline; GDD budget physics+AI 4 ms |
| FSM + steering → `MoveIntent` | inline, every NPC, every tick | O(N) arithmetic + ≤ 1 random/neighbour pick on arrival; est. < 0.05 ms at 64 |
| perception | **time-sliced**, 1/`slots` NPCs per tick | ≤ ⌈N/4⌉ NPCs × ≤ 2 LOS rays (nearest corpse, aimer) × 0.6 µs ⇒ ≤ 20 µs at 64; sounds are O(stimuli) distance checks |
| spawn | **throttled** (`spawns_per_tick`), candidate scan O(E) only while under the cap | one spawn = one Tnua body + one glTF scene instance in the client; 64 at once cost one 1.07 ms sim tick |
| despawn / corpse aging | inline | O(N + corpses) comparisons |
| graph adjacency | inline once on Loading→Playing | 20-30 µs |
| async (`AsyncComputeTaskPool`) | **nothing** | no unbounded work: N is capped by data, the heaviest per-tick item is 16 rays; a task would add a tick of latency to reactions |

## 3. Steps

Order = dependency order. Each step names its check.

### Data (all tuning here, none in `const`)

1. **`assets/npc/population.ron`** (new):
   ```
   (
       max_civilians: 40,
       spawn_ring: (60.0, 120.0),        // m from the player, GDD §6.1
       spawns_per_tick: 1,
       spawn_attempts: 8,
       spawn_min_separation: 4.0,        // m between a new civilian and any other
       spawn_view_margin_deg: 5.0,       // spawn only outside ViewCone.half_angle + this
       despawn_distance: 150.0,
       despawn_offscreen_seconds: 2.0,
       corpse_seconds: 30.0,             // GDD §4.3
       corpse_limit: 20,
   )
   ```
   `PopulationConfig` (`population/mod.rs`, `deny_unknown_fields`) + `validate()`: all finite; `max_civilians ≥ 0`;
   `0 < spawn_ring.0 < spawn_ring.1 < despawn_distance` (a spawn must not be despawnable at once);
   `spawns_per_tick ≥ 1`, `spawn_attempts ≥ 1`, separation > 0, `0 ≤ margin < 90`, `despawn_offscreen_seconds ≥ 0`,
   `corpse_seconds > 0`. Check: `tests/config.rs` shipped-load test + one range fixture (ring order) + unknown field.
2. **`assets/npc/perception.ron`** (new): `slots: 4, hearing_radius: 40.0, fight_hearing_radius: 15.0,
   corpse_sight: 20.0, aimed_distance: 15.0, aimed_cone_deg: 10.0`. `PerceptionConfig` + `validate()`:
   `slots ≥ 1` (u8), radii > 0, `0 < aimed_cone_deg < 90`.
3. **`assets/npc/navigation.ron`** (new): `arrive_radius: 0.5`. `NavigationConfig` + `validate()` (> 0).
   (A* queue values arrive with T11, not now.)
4. **`assets/npc/civilian.ron`** (new):
   ```
   (
       wander_gait: Walk,
       flee_gait: Run,
       idle_chance: 0.15,             // per node arrival
       idle_seconds: (2.0, 8.0),
       flee_distance: (30.0, 60.0),   // m
       cower_seconds: (3.0, 6.0),
       call_seconds: 4.0,             // Report (GDD §6.2)
       reaction: (
           flee: 1.0,
           cower: 1.5,
           report: 0.8,
           temperament_spread: 0.5,   // t in [0.5, 1.5]
           panic_distance: 15.0,      // cower weight 0 at and beyond this
           report_min_distance: 25.0, // an active threat closer than this is never phoned in
       ),
   )
   ```
   `CivilianConfig` + `validate()`: ranges ordered and ≥ 0, `0 ≤ idle_chance ≤ 1`, weights ≥ 0,
   `0 ≤ temperament_spread < 1`, `panic_distance > 0`, `report_min_distance ≥ 0`, `call_seconds > 0`.
5. **`assets/character/visual.ron`**: add `death: "die"` (played once, holds the last pose), `cower: "crouch"`
   (looped while `Cower`), `civilian_tints: [(0.55, 0.75, 1.0), (1.0, 0.7, 0.45), (0.6, 1.0, 0.6),
   (1.0, 0.55, 0.75), (0.85, 0.85, 0.85)]` (linear multipliers of `tinted_mesh`; the player keeps `tint`).
   `CharacterVisualConfig` gets the three fields; `resolve` resolves `death`/`cower` against the rig
   (`CharacterClips.death`, `.cower`); `validate` requires `civilian_tints` non-empty, components finite ≥ 0.

### Sim — code

6. **`crates/gta_sim/src/combat/hitscan.rs:100`**: `fn unit_f32` → `pub(crate) fn unit_f32` (civilian rolls reuse
   it; one uniform-float helper). No other change in `combat/`.
7. **`crates/gta_sim/src/flow/mod.rs`**: add `pub struct NpcSystems;` (FixedUpdate `SystemSet`) configured in
   `FlowPlugin::build` with `.run_if(in_state(GameState::Playing).or(in_state(GameState::Wasted)))`, doc comment:
   "NPCs keep living while the player is wasted". Check: compiles; the Wasted test below.
8. **`crates/gta_sim/src/navigation/mod.rs`** (new, ~150 lines):
   - `pub const NAVIGATION_CONFIG: &str = "npc/navigation.ron"`, `NavigationConfig`.
   - `#[derive(Resource)] pub struct SidewalkGraph { nodes: Vec<Vec3>, edges: Vec<(u32, u32)>, adjacency:
     Vec<Vec<u32>> }` with `pub fn new(nodes: Vec<Vec3>, edges: &[(u32, u32)]) -> Self` (skips self-loops and
     out-of-range indices with `warn!`), `from_walk_graph(&WalkGraph, y: f32)`, `node(u32) -> Vec3`,
     `neighbors(u32) -> &[u32]`, `edges() -> &[(u32, u32)]`, `is_edge(a, b) -> bool`.
   - `#[derive(Component, Reflect)] pub struct GraphWalker { pub from: u32, pub to: u32 }`.
   - Pure fns: `pub fn wander_next(graph, from, to, u: f32) -> u32` (uniform over `neighbors(to)` minus `from`;
     `from` only when it is the only neighbour); `pub fn flee_next(graph, from, to, threat: Vec3) -> u32`
     (neighbour of `to` maximising horizontal distance to `threat`, ties → lower index); `pub fn flee_start(graph,
     walker, threat) -> GraphWalker` (keep `to` if `node(to)` is farther from the threat than `node(from)`, else
     swap); `pub fn steer(position: Vec3, target: Vec3) -> Option<f32>` = `aim_yaw(flat(target − position))`,
     `None` for a zero vector. Horizontal distance everywhere (`xz`).
   - `NavigationPlugin`: inserts nothing global; `configure_sets(FixedUpdate,
     NpcSystems.run_if(resource_exists::<SidewalkGraph>))`; `OnTransition{exited: Loading, entered: Playing}` →
     `build_sidewalk_graph.run_if(resource_exists::<City>)` inserting `SidewalkGraph::from_walk_graph(&city.0.sidewalks,
     params.roads.curb_height)`; registers `GraphWalker`.
   - `#[cfg(test)]`: `wander_next` never returns `from` on a node of degree ≥ 2 and returns it on degree 1;
     `flee_next` worked example: `to` at origin with neighbours (10,0,0), (−10,0,0), (0,0,10), threat (8,0,1) →
     distances 2.24 / 18.03 / 12.04 → (−10,0,0); **`steer` three directional examples** (sign errors pass magnitude
     tests): target −Z → yaw 0 and `move_direction(Vec2::Y, yaw) = (0,0,−1)`; target −X → yaw +90°, direction
     (−1,0,0); target +Z → yaw 180°, direction (0,0,+1); plus +X → −90°, (1,0,0).
9. **`crates/gta_sim/src/population/mod.rs`** (new, ~300 lines):
   - `POPULATION_CONFIG`, `PopulationConfig` (Step 1).
   - `#[derive(Resource, Reflect, Default)] #[reflect(Resource)] pub struct ViewCone { pub origin: Vec3, pub
     forward: Vec3, pub half_angle: f32 }`; `pub fn from_perspective(origin, forward, fov_y, aspect) -> Self`
     (diagonal formula, §2); `pub fn contains(&self, point, margin_rad) -> bool` (`Dir3::new(forward)` failing ⇒
     `false`; else `angle_between(point − origin, forward) ≤ half_angle + margin`; `point == origin` ⇒ `true`).
     Unit tests: the three cone examples (55.00°, 46.72°, 54.74° ± 0.01°); forward −Z: (0,0,−10) in, (−5,0,−10)
     (26.6°) in, (10,0,0) (90°) out, (0,0,10) out; default cone contains nothing.
   - `#[derive(Resource)] pub struct NpcRng(pub ChaCha8Rng)` seeded `seed_from_u64(seed)` + `set_stream(1)` (a
     different stream than `CombatRng`); `PopulationPlugin { seed }` (same seed as `CombatPlugin`).
   - `#[derive(Component, Reflect, Default)] pub struct Offscreen(pub f32)` — seconds outside the view cone
     (required by `Civilian`). `#[derive(Component, Reflect)] pub struct Corpse { pub age: f32 }`.
   - `pub fn corpse_components() -> impl Bundle` = `(Dead, Corpse { age: 0.0 }, TnuaToggle::Disabled,
     RigidBody::Static, CollisionLayers::NONE)` (reused by T9/T11 for their NPCs).
   - Systems (`PopulationSystems` set, in `NpcSystems`, `.after(HealthSystems::Death)`, chained):
     `age_corpses` (age += dt; `try_despawn` at `corpse_seconds`; if more than `corpse_limit` corpses, `try_despawn`
     the oldest extras — sort ≤ 21 ages), `despawn_far` (for each `Civilian`: `Offscreen` = 0 if the chest point is in
     `ViewCone` (margin 0) else += dt; `try_despawn` when horizontal distance to the player > `despawn_distance`
     and `Offscreen ≥ despawn_offscreen_seconds`), `spawn_civilians` (deficit = `max_civilians` − alive civilians;
     ≤ `spawns_per_tick` spawns, candidate edges = those whose segment comes within `spawn_ring.1` of the player and
     has an endpoint beyond `spawn_ring.0`; for each attempt pick a candidate uniformly and `t ∈ [0,1)`, accept per §2
     using `ViewCone.contains(point + 1 m up, spawn_view_margin_deg)`; spawns `civilian_bundle`). Player position:
     `Query<&Position, With<Player>>` + let-else (no player ⇒ return). Registers `ViewCone`, `Offscreen`, `Corpse`.
10. **`crates/gta_sim/src/perception/mod.rs`** (new, ~280 lines):
    - `PERCEPTION_CONFIG`, `PerceptionConfig` (Step 2).
    - `#[derive(Component, Reflect, Default)] pub struct Perception { pub slot: u8, pub pending: Option<Threat> }`;
      `#[derive(Reflect, Clone, Copy, Debug, PartialEq)] pub struct Threat { pub kind: ThreatKind, pub at: Vec3,
      pub distance: f32 }`; `pub enum ThreatKind { Gunshot, Fight, Corpse, Aimed, Hurt }`.
    - `#[derive(Resource, Default)] pub struct AiClock { pub tick: u64 }`; `#[derive(Resource, Default)] struct
      SlotCursor(u8)`; observer `On<Add, Perception>` sets `slot = cursor; cursor = (cursor + 1) % slots`.
    - `#[derive(Resource, Default)] pub struct StimulusLog(Vec<(u64, ThreatKind, Vec3)>)`.
    - `#[derive(SystemSet)] pub enum AiSystems { Perceive, Decide }` configured
      `(Perceive, Decide).chain().in_set(NpcSystems).after(HealthSystems::Damage).before(TnuaUserControlsSystems)`.
    - Systems in `Perceive`, `.chain()`: `advance_clock` (tick += 1); `collect_stimuli` (drop entries with
      `tick_then + slots ≤ tick`; push every `ShotFired` as `Gunshot` at `muzzle`, every `MeleeHit` as `Fight` at
      `point`; for every `DamageDealt` whose target has `Perception` and is not `Dead`, write `pending = Hurt` at the
      shooter's `Position` (fallback `point`), distance 0 — the unsliced path); `perceive` (for `Perception` with
      `tick % slots == slot`, `Without<Dead>`: nearest of — log entries within their radius (`hearing_radius` for
      gunshot, `fight_hearing_radius` for fight); nearest `Corpse` within `corpse_sight` with LOS; every live
      `(AimIntent, Loadout)` holder (≠ self) with `held.is_some() && aiming`, within `aimed_distance`, angle between
      `aim.direction` and `chest − aim.origin` ≤ `aimed_cone_deg`, with LOS — and keep a `Hurt` already pending).
      LOS: `SpatialQuery::cast_ray` mask `[GameLayer::World]`, eyes = feet + `head_height`, blocked if a hit is
      shorter than the target distance.
    - `PerceptionPlugin`: `init_resource` for the three resources, `add_observer`, registers types.
11. **`crates/gta_sim/src/civilian/mod.rs`** (new, ~330 lines) + **`civilian/reaction.rs`** (new, ~150 lines incl.
    tests):
    - `CIVILIAN_CONFIG`, `CivilianConfig` (Step 4).
    - `#[derive(Component, Reflect)] #[reflect(Component)] #[require(Character, Perception, Offscreen)] pub struct
      Civilian { pub state: CivilianState, pub temperament: Temperament }`;
      `#[derive(Reflect, Clone, Copy, Debug, PartialEq)] pub enum CivilianState { Wander, Idle { left: f32 },
      Flee { from: Vec3, left: f32 }, Cower { from: Vec3, left: f32 }, Report { left: f32 }, Dead }`;
      `Temperament { flee, cower, report }`.
    - `pub fn civilian_bundle(loco, handle, health, graph, walker: GraphWalker, t: f32, temperament) -> impl Bundle`
      = `(Civilian{Wander, temperament}, walker, Name::new("Civilian"), Transform at lerp(node(from), node(to), t) +
      Y·float_height, character_components(loco, handle), Health::full(health))` — used by the spawner and tests.
      `pub fn roll_temperament(rng, spread) -> Temperament`.
    - `reaction.rs`: `pub enum Reaction { Flee, Cower, Report }`, `pub fn choose_reaction(threat: &Threat, t:
      &Temperament, cfg: &ReactionConfig, allow_report: bool) -> Reaction` (§2 formulas; no RNG inside).
    - `civilian_fsm` (`AiSystems::Decide`): per civilian `Without<Dead>`, take `perception.pending`; transitions per
      §2 (rolls via `NpcRng` + `unit_f32`); state timers tick by `Time<Fixed>` dt; `Flee.left` decreases by
      horizontal `LinearVelocity`·dt; arrival at `node(to)` within `arrive_radius` advances `GraphWalker`
      (`wander_next` / `flee_next`, idle roll on Wander arrivals); on entering `Flee`, `flee_start`. Output:
      moving states write `MoveIntent { axis: Vec2::Y, yaw: steer(..), gait }` (wander/flee gait from data);
      `Idle`/`Cower`/`Report` write `axis = Vec2::ZERO`. Use `set_if_neq`-free plain writes (MoveIntent has no
      `Changed` consumer). Golden Path: one helper per state returning the next state, the system only dispatches.
    - `civilian_death` (`HealthSystems::Death`, `NpcSystems`): `Query<(Entity, &Health, &mut Civilian),
      Without<Dead>>`, `current ≤ 0` → `state = Dead`, `insert(corpse_components())`, `pending = None`.
    - `CivilianPlugin`: registers `Civilian`, `CivilianState`, `Temperament`, `Perception`, `Threat`, `ThreatKind`
      for BRP.
12. **`civilian/reaction.rs` `#[cfg(test)]` worked table** with the shipped `civilian.ron` (`include_str!`,
    `lines()`-independent RON parse, "GATE BROKEN" on parse error). `p = clamp(1 − d/15, 0, 1)`:
    | # | kind, d | t (flee, cower, report) | scores flee / cower / report | expected |
    |---|---|---|---|---|
    | 1 | Gunshot 20 | 1, 1, 1 | 1.0 / 0 / 0 (20 < 25) | Flee |
    | 2 | Gunshot 2 | 1, 1, 1 | 1.0 / 1.3 / 0 | Cower |
    | 3 | Gunshot 10 | 0.5, 1.5, 1 | 0.5 / 0.75 / 0 | Cower |
    | 4 | Gunshot 30 | 0.7, 1, 1.5 | 0.7 / 0 / 1.2 | Report |
    | 5 | Gunshot 30 | 1, 1, 1 | 1.0 / 0 / 0.8 | Flee |
    | 6 | Corpse 10 | 0.6, 1, 1.4 | 0.6 / 0.5 / 1.12 | Report |
    | 7 | Hurt 0 | 1, 1, 1.5 | 1.0 / 1.5 / 0 (never) | Cower |
    | 8 | Aimed 6 | 1, 1, 1.5 | 1.0 / 0.9 / 0 (never) | Flee |
    | 9 | Gunshot 30, `allow_report = false` | 0.7, 1, 1.5 | 0.7 / 0 / 0 | Flee |
    No case sits on a tie (gates lesson: never on the boundary). Plus `roll_temperament` stays in `[1−s, 1+s]` over
    1000 rolls.
13. **`crates/gta_sim/src/lib.rs`**: `pub mod civilian; pub mod navigation; pub mod perception; pub mod population;`;
    in `compose_sim` load + validate the four configs exactly like the existing ones and insert them; add
    `NavigationPlugin, PerceptionPlugin, PopulationPlugin { seed: combat_seed }, CivilianPlugin` to `add_plugins`
    (after `CombatPlugin`; the tuple grows from 8 to 12 entries, within the 15-entry `Plugins` tuple impl,
    `bevy_app-0.19.1/src/plugin.rs:186-192`). Check: `cargo build`, `cargo test -p gta_sim` (all existing tests still green: the new sets only
    run when a `SidewalkGraph` exists, which the test floor does not have).

### Sim — gates (`cargo test -p gta_sim`)

Every integration test uses the production composition (`composed_app` / `headless_app` / `city_app`) and the
production `civilian_bundle`; configs are read from the resources, never retyped. Where a test mutates a config
resource (`max_civilians = 0` to keep the spawner idle), that is the only deviation and it is named in the test.

14. **`tests/config.rs`**: shipped-load tests for the four new files; unknown-field test for `population.ron`
    (error names file and field); range fixtures strictly on the failing side (gates lesson): `spawn_ring: (130.0,
    120.0)`, `slots: 0`, `temperament_spread: 1.5`.
15. **`tests/civilians.rs`** (new; helpers in `tests/common/mod.rs`: `spawn_civilian(app, walker, t, temperament)`,
    `civilian_state(app, e)`, `test_graph(app, nodes, edges)` = inserts `SidewalkGraph::new`):
    - **`gunshot_at_20m_flee_or_cower_within_one_cycle`** (acceptance). Test floor, `max_civilians = 0`, graph = square
      loop with corners (±20, 0, ±20) plus a far segment (−34,0,−30)→(−26,0,−30). Player settled at the origin,
      holding a pistol, aim straight up (`set_aim(origin, origin + Y)`). Four civilians at the side midpoints
      (±20, 0, 0), (0, 0, ±20) spawned in consecutive order ⇒ slots 0..3; one control at (−30, 0, −30).
      Preconditions from data (GATE BROKEN otherwise): muzzle-to-civilian distances < `hearing_radius` and
      < `report_min_distance`, control distance > `hearing_radius`. Run 16 ticks, all 5 `Wander`. `fire_requested =
      true`, record the tick T of the `ShotFired` (via `Shots`). Run `slots` ticks one by one recording each
      civilian's first non-calm tick. Assert: each of the four is `Flee` or `Cower` by tick T + slots − 1; their
      reaction ticks are **all distinct** (proves the slicing is real) and all in [T, T + slots − 1]; the control is
      still `Wander` after 2·slots more ticks.
      Flip-RED (record in IMPL_SUMMARY): (a) `perceive` ignores slots → distinct-ticks RED; (b) log retention 1 tick
      → 3 civilians never react → RED; (c) hearing radius check removed → control RED.
    - **`hurt_reacts_same_tick`**: shoot a civilian in the chest at 10 m (T6 geometry), assert in the tick of the
      `DamageDealt` its state is `Flee` or `Cower` (never `Report`). Flip: route `Hurt` through the log → RED.
    - **`report_is_interrupted_by_a_new_threat`**: civilian with temperament (0.7, 1, 1.5) at 30 m from a shot
      (between `report_min_distance` and `hearing_radius`) → `Report` (table row 4); a second shot at 30 m within
      `call_seconds` → `Flee` (row 9). Flip: `allow_report` always true → stays `Report` → RED.
    - **`death_makes_a_corpse`**: set a civilian's `Health.current = 0`; next tick: `state == Dead`, has `Dead`,
      `Corpse`, `TnuaToggle::Disabled`, `RigidBody::Static`, `CollisionLayers::NONE`; position unchanged after 64
      ticks; a chest ray with the hitscan mask no longer hits it (the probe's check). Flip: drop
      `CollisionLayers::NONE` → ray RED.
    - **`corpse_limit_and_lifetime`**: spawn `corpse_limit + 1` civilians on the square, kill one per tick; after the
      last kill + 1 tick exactly `corpse_limit` corpses remain and the first-killed entity is gone; run to
      `corpse_seconds` after the second kill → all gone at the tick the ages reach it (count ticks from the data:
      `corpse_seconds × 64` is exact for 30 s). Flip: sort youngest-first → wrong entity survives → RED.
16. **`tests/civilian_city.rs`** (new, city seed 1 — needs the real graph, curbs and crossings):
    - **`wander_stays_on_the_graph`** (acceptance). `max_civilians = 0`; 12 civilians via `civilian_bundle` on edges
      within 40 m of the player spawn, ≥ 6 m apart (worst case 40 + 30 s × walk 1.8 m/s = 94 m < 150 m ⇒ no
      despawn). Run 1920 ticks (30 s); every 8 ticks for every civilian: state ∈ {Wander, Idle};
      `graph.is_edge(from, to)`; horizontal distance from its feet to segment `node(from)`–`node(to)` ≤
      `street.sidewalk / 2` (= 1.5 m from `city.ron`; the narrowest sidewalk half-width, also > `arrive_radius`).
      Liveness (a frozen NPC trivially stays on the graph): every civilian changed `to` ≥ 2 times and travelled
      ≥ 20 m of path. Flip: steer toward `node(to) + (3, 0, 0)` → distance RED; `wander_next` returning any node →
      `is_edge` RED.
    - **`despawn_after_2s_offscreen_beyond_150m`** (acceptance). `max_civilians = 0`, `ViewCone::default()` (nothing
      in frame). Civilians at graph points: A at > 150 m (e.g. ~160 m), B at 130-145 m, C at > 150 m with the view
      cone pointed at it (`ViewCone::from_perspective(player eye, C − eye, 70°, 16/9)`). All `Idle` with a long timer
      so they do not walk across the 150 m line (state set by the test before the first tick). Tick counting: A is
      present after `2.0 × 64 − 1 = 127` ticks and gone after 128 (derive from `despawn_offscreen_seconds`; 1/64 is
      exact in binary); B and C present after 256 ticks; then turn the cone away from C → C gone 128 ticks later, not
      earlier. Flip: `||` instead of `&&` → B RED; no reset of `Offscreen` in view → C RED.
    - **`spawns_off_frame_in_the_ring`**: shipped `max_civilians`, view cone looking +X from the player eye. Run until
      the count reaches the cap (bound: `max_civilians / spawns_per_tick × 4` ticks, liveness); assert every
      civilian's spawn point (recorded on its first tick via `Added<Civilian>`) is in `[spawn_ring.0, spawn_ring.1]`,
      outside the cone + margin, on a graph edge (distance ≤ 1e-3 to `node(from)`–`node(to)`), ≥
      `spawn_min_separation` from the others. Flip: skip the view test → some spawn in view → RED (cone 55° + 5° margin covers ~120° of the
      360° ring, so about a third of unfiltered candidates fall inside).
    - **`npcs_live_through_wasted`**: one wandering civilian; `write_damage(1000)` → `Wasted`; during the slow-mo it
      keeps its graph invariant (distance check as above) and its `GraphWalker.to` still advances. Flip: put the AI
      in `PlayingSystems` → it walks straight off → RED.
17. **`tests/civilian_bench.rs`** (new, own test binary so it never shares the CPU with other tests of the same
    binary; not `#[ignore]`): city seed 1, `max_civilians = 64`; run until 64 alive (liveness bound as above); then
    640 measured ticks, each `app.update()` = one fixed tick; every 64 ticks the test writes a `ShotFired` at the
    position of a civilian (exercises perception, reaction and flee for its neighbours); print mean / p50 / p95 /
    max tick time and min/max civilian count (`--nocapture` shows it; it is also in the test output on failure).
    **Hard assertion: mean < 8 ms.** Derivation: the probe's seed-1 city mean with 64 Tnua walkers is 0.756 ms
    (test profile), ×10 ("на порядок") ≈ 7.6 → 8 ms; the AI adds an estimated < 0.1 ms. The max is not asserted
    (OS spikes 6-8 ms in the probe). What it catches: a hang, and any O(N²) whose per-pair work exceeds
    (8 − 0.76) ms / 4096 pairs ≈ 1.8 µs (e.g. a per-pair Tnua/physics op, allocation or search). What it does NOT
    catch: one 0.6 µs ray per pair (≈ 2.4 ms total) — the slicing claim is carried by the distinct-ticks assertion
    of Step 15, not by this bench. Flip-RED: (a) `std::thread::sleep(10 ms)` in `civilian_fsm` (hang) → RED;
    (b) in `perceive`, cast 4 rays from every civilian to every other every tick (4 × 4096 × 0.6 µs ≈ 10 ms) → RED.
    Record the real mean in IMPL_SUMMARY; a mean above 2 ms is a finding to investigate even though it passes.

### Client

18. **`src/camera/mod.rs`**: new system `publish_view_cone` in `PostUpdate`, `.after(follow_player)`:
    `Single<(&Transform, &Projection), With<OrbitCamera>>`, `ResMut<ViewCone>`; for `Projection::Perspective(p)`
    writes `ViewCone::from_perspective(t.translation, t.rotation * NEG_Z, p.fov, p.aspect_ratio)`. The rotation used
    is the camera's final rotation (recoil/shake included — visual, and the cone is conservative). Registered in
    `CameraPlugin::build`. Check: `cargo test -p gta_like --bin gta_like` unaffected; QA reads `ViewCone` over BRP.
19. **`src/visuals/character_config.rs`**: fields `death`, `cower`, `civilian_tints` (Step 5), `CharacterClips {
    death, cower }`, `resolve` + `validate` as Step 5.
20. **`src/visuals/character.rs`**:
    - `CharacterAnimations`: `death` and `cower` nodes (`graph.add_clip(clip(clips.death | clips.cower), 1.0, root)`).
    - `ShownAction` gains `Death` and `Cower`. In `drive_character_animation` add `Has<Dead>` and
      `Option<&Civilian>` to the character query; priority `Death` (Dead) > `Knockdown` > `Cower`
      (`Civilian.state` is `Cower`) > `Swing`. `Death` plays once (no `.repeat()`), `Cower` loops; both skip
      locomotion like the existing actions. When `Dead` is removed (dummy revive, player respawn) the existing
      `action_ended` path returns to locomotion; the `rest` layer resets unkeyed joints (TASK-008 lesson). This
      also gives the player and the dummies a death clip — intended (GDD §4.3 A applies to every character).
    - `on_model_ready` → `tint_mesh`: tint = `civilian_tints[character.index() as usize % len]` when the character
      has `Civilian`, else `config.tint` (pass the tint in instead of reading `config.tint` inside).
    - Same model for civilians (see §1 glTF fact; Open question 1).
21. **`src/visuals/character_gate.rs`**: one test in the existing harness: a character with `Dead` inserted gets the
    `death` node active and no locomotion node started after it; a `Civilian` in `Cower` gets the `cower` node,
    `Wander` returns to locomotion. Assert `animation(node).is_some_and(..)` on the node the test expects, not
    `all_paused()` (TASK-008 lesson). Flip: drop the `Has<Dead>` branch → RED.

### Runtime QA and owner run

22. **`tools/qa/scenarios/t8.py`** (new, stdlib, imports `brp`, `t5`, `t6` helpers like `t7.py`):
    1. `Game(features=("dev",), args=("--seed", "1"), release=True)`; `wait_resource("CityLayoutHash")` equals the
       golden; `wait_chunks`.
    2. Poll `rows(game, ["Civilian", "Position"])` until ≥ 40 alive (non-`Dead`), deadline 90 s; record the time to
       40; `diagnostics()` at that moment ("get_diagnostics при 40 мирных": FPS + frame time into the summary);
       screenshot `crowd.png`.
    3. Pistol: teleport onto the pistol `WeaponPickup` (t6 flow), wait until `Loadout.held == Pistol`.
    4. Pick the civilian with the most live civilians within `hearing_radius` (read from `perception.ron`);
       teleport the player 1.5 m from it toward that group's centroid (both on sidewalks); wait 0.5 s.
    5. Count states (tolerant enum reader like `t7.reaction_name`) among civilians within `hearing_radius` of the
       player → `before`. Aim into the sky: `move_mouse(0, −(60 / sensitivity_deg))` (pitch up to the clamp),
       `send_mouse_button("Left", 80)`; confirm the pistol magazine dropped by one (the shot really fired).
       Wait 0.3 s (one perception cycle is 62.5 ms plus BRP latency); count again → `after`; screenshot `scatter.png`.
    6. **Hard pass:** at least one civilian was in hearing range; share(Flee + Cower) after > share before; zero
       `ERROR` lines in the log (`log_errors`). Summary JSON: counts per state before/after, FPS at 40, time to 40,
       screenshots. Screenshots are evidence, not verdicts.
23. **Owner checklist** (QA writes it into QA_REPORT.md): "улицы живые" (civilians walk the sidewalks, stop, cross
    the roads, no crowd stuck on a corner), "выстрел в воздух разгоняет толпу" (nearby civilians run or crouch,
    none stands still), death clip and the body lying 30 s, crouch reads as cowering, tints tell civilians from the
    player, FPS with 40 civilians. Feel values live in `civilian.ron`/`population.ron`.

### Order and checks

24. Steps 1-4 + 14 → `cargo test -p gta_sim --test config`. Steps 6-13 → `cargo build`, `cargo clippy -- -D warnings`
    (and `cargo clippy -p gta_sim --tests -- -D warnings`). Steps 12, 8, 9 unit tests → `cargo test -p gta_sim --lib`.
    Steps 15-17 → `cargo test -p gta_sim` (full, existing tests green). Steps 5, 18-21 →
    `cargo test -p gta_like --bin gta_like`. `python tools/qa/tree_check.py` and
    `cargo tree -p gta_sim -e normal -i bevy_render` (must stay empty — no new dependency, but it is the law).
    Step 22 last. `cargo test -p citygen` unchanged (citygen is not touched).

## 4. Risk areas

- **R1. Stimulus timing across the schedule.** If `collect_stimuli` ever runs before `fire_weapons` in the same tick,
  the shot lands one tick later and a civilian of the shot tick's slot waits a whole extra cycle — the acceptance
  bound breaks by one tick, silently. Mitigation: `AiSystems` `.after(HealthSystems::Damage)` at set level, flat
  `.chain()` (TASK-008 nested-chain lesson), and the exact-tick assertion of Step 15.
- **R2. Stale buffered messages across state changes.** `NpcSystems` runs in `Playing` and `Wasted`, so its readers
  consume every tick in both; in `Loading` nobody writes these messages. If a future state (Paused, Busted) is added,
  it must be added to the `NpcSystems` condition or the readers will see a backlog on resume (TASK-006/007 lesson).
- **R3. Crowds on narrow sidewalks.** No separation steering: two civilians walking head-on rely on capsule
  contacts; a group can jam on a 3 m sidewalk or at a corner. Gates cover 12 sparse civilians only. Trigger for a
  fix: the owner run or a QA screenshot showing a jam → add neighbour separation (spatial grid) in `navigation/`.
- **R4. Stuck civilians.** A civilian pinned against the standing player keeps pushing (no stuck detector). Same
  trigger as R3; the fix is an edge timeout that turns the walker back.
- **R5. Wander test tolerance vs physics.** Knockback or a player shove can push a civilian > 1.5 m off its edge; the
  city gate has no player interaction, so it stays deterministic enough. If it flakes, investigate the steering, do
  not widen the tolerance blindly.
- **R6. View cone is frustum-only.** Occlusion is ignored (a civilian behind a building counts as visible), so fewer
  spawn/despawn opportunities, never a pop-in. The spawn ring (60 m inner) means the first ~30 s after loading the
  nearest streets are empty until civilians walk in — owner-visible, tunable in `population.ron` (Open question 2).
- **R7. Client cost of 40 skinned civilians.** Not measurable headless. GDD §11 "~64 animated humanoids"; QA records
  FPS at 40 via `get_diagnostics`; the owner decides. Each spawn instantiates one glTF scene (asset cached).
- **R8. Death clip on the player and dummies.** A new visible behaviour outside civilians (intended by GDD §4.3).
  The `rest` layer must bring a revived dummy up; if the owner sees a dummy lying after reset, check that
  `action_ended` fires on `Dead` removal.
- **R9. Corpse head sensor.** The corpse body gets `CollisionLayers::NONE`, its head child keeps `(Hitbox, NONE)`;
  hitscan already skips a dead head (`hitscan.rs:193-201`), perception rays use `World` only. A new ray user with a
  `Hitbox` mask must keep that predicate.
- **R10. Bench on a slower machine.** 8 ms mean is 10× this machine; CI on a weaker CPU still has ≥ 5× headroom.
  If it ever fails, read the printed breakdown before touching the threshold.

## 5. Open questions

Вопросы владельцу (по делегированию отвечает оркестратор). Ни один не блокирует реализацию: у каждого есть
рекомендуемый вариант по умолчанию.

1. **Внешность мирных.** Все 12 моделей Kenney на диске, но клипы анимаций одной модели не двигают другую (путь
   анимационной цели начинается с имени корневого узла GLB, а оно у каждой модели своё).
   - A (по умолчанию): одна модель, у мирных тинт одежды из `visual.ron` `civilian_tints`. Дёшево, игрок отличается.
   - B: разные модели в T8: свой набор клипов/граф на каждую модель (≈ +150 строк в `visuals/`, 12 графов). Толпа
     разнообразнее, риск анимационных гейтов выше.
   - C: тинт сейчас, модели позже отдельной задачей полировки (T16).
   Рекомендация: A (+ C как запись на будущее).
2. **Ближний край кольца спавна.** GDD: спавн вне кадра в кольце 60-120 м. Пешком 1.8 м/с → первые ~30 с после
   загрузки ближайшие улицы пустые.
   - A (по умолчанию): оставить 60 м (число в `population.ron`, владелец крутит после прогона).
   - B: 30 м вне кадра (в духе SA для машин вне кадра 15 м) — улицы оживают быстрее, выше шанс увидеть "появление"
     за углом камеры при повороте.
   Рекомендация: A, решение по прогону владельца.
3. **Свидетель рядом не звонит.** По скореру активная угроза ближе 25 м (`report_min_distance`) никогда не
   вызывает `Report`: рядом с выстрелом все бегут/приседают, звонят только дальние (25-40 м) и увидевшие труп.
   - A (по умолчанию): так и оставить, T10 проверит, хватает ли свидетелей для розыска.
   - B: после окончания `Flee` часть мирных звонит (второй вход в `Report`) — больше свидетелей, +1 переход FSM.
   Рекомендация: A; B — кандидат в T10, если розыск будет "не набираться".

children: 0 launched / 0 reported.
