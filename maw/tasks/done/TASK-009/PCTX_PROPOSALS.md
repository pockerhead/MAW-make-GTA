# PCTX proposals — TASK-009

## 2026-09-23 (planner) — bevy-ecs risk lesson: glTF clips drive only the model whose root node name matches
bevy_gltf 0.19.1 builds `AnimationTargetId::from_names(path)` where the path STARTS with the glTF animation-root node
name (`bevy_gltf-0.19.1/src/loader/mod.rs:559-563` for clip curves, `:1545-1558` for scene nodes). Every Kenney
Mini Characters GLB has the same joints but a different root name (`character-male-a`, `character-female-b`, ...,
checked by parsing the 12 GLBs). A clip loaded from `character-male-a.glb` therefore targets nothing in any other
model: the shared `CharacterAnimations` graph silently leaves another model in its bind pose, with no error.
Why: silent in headless gates (they see node indices, not poses); anyone adding character variants (civilians T8,
gangs T9, police T11) hits it. Using another model needs its own clips/graph, or a rename of the root on load.

## 2026-09-23 (planner) — measured Tnua NPC cost (planning fact for T9/T11/T15/T16)
Headless probe (TASK-009 `scratch/probe_npc_*.log`, i9-11900K, `cargo test` profile and `--release` nearly equal because
deps are opt-level 3): Tnua walker ≈ 3 us per character per fixed tick in the seed-1 city (64 walkers: 0.76 ms/tick
vs 0.56 ms empty city), ≈ 4-5 us on the flat test floor; one 20 m avian ray in the city ≈ 0.6 us; per-tick max has
OS spikes up to 6-8 ms, so only MEAN tick time is gateable. GDD §10.2 "kinematic far NPCs" fallback is not needed at
these counts.
Why: saves the next NPC slice from re-measuring and from gating on a noisy max.

## 2026-09-23 (plan-reviewer-2) — gates lesson: a headless client test CAN load real Kenney GLBs and prove a pose
`scratch/probe_ws/tests/probe_glb.rs`: `MinimalPlugins + TransformPlugin + AssetPlugin{file_path} + ImagePlugin + MeshPlugin +
AnimationPlugin + WorldSerializationPlugin + GltfPlugin` + `init_asset::<StandardMaterial>()` (no bevy_render) loads the GLB,
spawns the scene, fires `WorldInstanceReady` and animates joints. Walk clip from the model's own GLB turned `leg-left` by
1.183 rad in 200 updates; the male-a clip on a female-b instance by 0.0000 rad. A joint-rotation assertion is a falsifiable
"not a T-pose" gate; it corrects the TASK-005 lesson's implication that only BRP can gate such handlers.
Why: T9/T11 add more character models; structure-only graph tests cannot see the root-name bug.

## 2026-09-23 (plan-reviewer-2) — planning fact: sidewalk graph node density
Seed 1: 579 nodes over the city, min node spacing 4.5 m; 16 nodes in the 60-120 m ring at the player spawn, median 10
(p10 6, min 1) outside a 120 deg view wedge over all node positions (`scratch/probe_nodes.log`). Node-only spawning fills a
40 cap over seconds, never in one tick. Why: T9/T11 spawn rules and any "cap reached by t" gate must be derived from this.

## 2026-09-23 (implementer) — environment lesson: a shared target/ between worktrees links the other tree's crate
With `.worktrees/showcase` building into the same `D:/test-gta-like/target`, workspace crates (`gta_sim`, `citygen`) get
the same metadata hash in both trees (cargo hashes path sources relative to the workspace root). Whichever tree built
last owns `libgta_sim-<hash>.rlib`, and the other tree's fingerprint can still look fresh: test binaries then failed with
"could not find `civilian` in `gta_sim`" and a citygen unit test failed on code this task never touched. `touch
crates/<crate>/src/lib.rs` before each cargo command forces the rebuild. Why: an agent that trusts the red/green of a
shared-target run can chase a phantom regression; parallel worktrees should use their own target dir, or agents touch
the crate roots first.
