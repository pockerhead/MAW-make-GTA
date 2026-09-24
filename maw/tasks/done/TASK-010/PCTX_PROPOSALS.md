# PCTX proposals — TASK-010 (planner)

- 2026-09-24 (planner, TASK-010) — domain gates or bevy-ecs: a new NPC role draws from its own ChaCha stream
  (`seed_from_u64(seed)` + its own `set_stream(k)`), never from `NpcRng`. Why: `spawn_civilians` calls `rng.unit()` once per
  surviving candidate, so any extra draw or candidate filter shifts every later civilian roll, and the single-run
  `street_ahead_stays_populated` density gate (+2.6 margin, seed-1 route enters gang 0 turf) moves without a bug.
  Trigger: a new `ResMut<NpcRng>` parameter in a system that is not in `civilian/` or `population/mod.rs`.

- 2026-09-24 (plan-reviewer-2, TASK-010) — domain bevy-ecs (Pointers/Risk lessons): `pathfinding` 4.16.0 `astar`
  needs `C: Zero + Ord + Copy` (`src/directed/astar.rs:81`), so f32 edge costs do not compile; use integer
  centimetres. Measured on the seed-1 sidewalk graph (579 nodes): 11 us mean, <= 215 us worst per search, so a
  synchronous K-per-tick cap (`navigation.ron route_requests_per_tick`) fits the AI budget without async markers.
  Why: T11 police reuse the same router. Trigger: `pathfinding::` in a diff.

- 2026-09-24 (implementer, TASK-010) — domain gates (Risk lessons): the headless GLB harness (`MinimalPlugins` +
  `GltfPlugin`, no `PbrPlugin`) spawns glTF meshes WITHOUT `MeshMaterial3d<StandardMaterial>`: bevy_pbr 0.19.1
  makes them in its glTF extension handler (`bevy_pbr/src/gltf.rs`, not public). A material/tint gate there needs a
  stand-in `GltfExtensionHandler` plus `register_type::<MeshMaterial3d<StandardMaterial>>()` (the world asset
  spawner panics on the unregistered type). Precedent: `src/visuals/civilian_gate.rs`
  `StandardMaterialStandIn`. Trigger: `MeshMaterial3d` read in a `*_gate.rs` that builds the GLB harness.

- 2026-09-24 (implementer, TASK-010) — domain gates (Risk lessons): children under a glTF model instance (a held
  gun parented to a hand joint) can be replaced one update after `WorldInstanceReady` wired the model while the
  model root entity stays (seen 1 in ~6 full-suite runs). A gate over such a child waits for the state within a few
  updates instead of asserting "after exactly 1 update". Trigger: `ChildOf(hand)`-style attachments in a gate.

- 2026-09-24 (fixer, TASK-010) — domain gates (Risk lessons): in the headless GLB harness `AssetEvent<WorldAsset>`
  `LoadedWithDependencies`/`Modified` keep arriving for several updates while the scenes stream in; bevy_world_serialization
  0.19.1 re-instances on a `Modified` (`world_instance_spawner_system`) and debounces a `Modified` for 3 frames after any
  event for that id. A gate that forces a re-instance (`Assets::get_mut` + a real `DerefMut`; a bare `get_mut` emits
  nothing) first waits for quiet updates and retries. After a re-instance a joint-parented `HeldGun` is gone one update
  and `Hidden` one more (`attach_held_gun`/`show_held_gun` are unordered). Trigger: `WorldAssetRoot` or `ChildOf(hand)`
  in a `*_gate.rs`.
- 2026-09-24 (fixer, TASK-010) — agents/qa.md: the t8/t9 scenarios now record `frame_report()` (brp.py): monitors with
  refresh, present mode, FPS as shipped, then no-vsync frame cost. This host's only monitor is still the 30 Hz
  `\.\DISPLAY9`; T9 frame cost at the HQ firefight is ~2.5 ms. Why: the TASK-002 false alarm recurred in the T9 review.
- 2026-09-24 (fixer round 3, TASK-010) — domain game-design (Risk lessons), for T11 police reusing the gang fire
  rule: a hold-fire check that widens the line by the spread cone (0.5 m + d·tan 11° for an SMG) makes any
  non-hostile body standing within ~1.5 m of the target block that target from every angle beyond ~2-5 m. Moving to a
  clear spot cannot fix it: brawling groupmates or a pressed bystander starve the gunman by construction (measured:
  2 shots / 30 s). Decide the scrum/human-shield policy in the design, not in the movement rule. Trigger: `FireLine`
  / `fire_line.rs` or a new faction that shoots near allies.
- 2026-09-24 (qa round 3, TASK-010) — domain game-design (Risk lessons), for T11 police: the gang "close in" fallback (no usable candidate spot, blocker not pressed against the target) is a direct seek at the target; `avoid_offset` avoids only World geometry, so a groupmate standing in Hold exactly on that path stops the rear member for the whole fight (measured: corridor <= 3.0 m wide, SMG front / pistol behind, pistol 0 shots in 30 s; 4.0 m wide passes). Trigger: `unblock` / `Motion::Seek` toward the target in `fire_line.rs`.
