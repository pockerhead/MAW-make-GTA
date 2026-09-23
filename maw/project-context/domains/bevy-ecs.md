# Domain: bevy-ecs
# NORMATIVE when active — a constraint to satisfy, not a claim for you to audit.
# Covers any Bevy code: systems, queries, components, resources, messages, observers, plugins, schedules.

## Invariants

- **Every Bevy system IS the frame.** The schedule runs synchronously inside `app.update()`; the
  frame does not present until it finishes. Bevy's system parallelism shortens this, it does not make
  any system "background". Heavy or unbounded-N work (pathfinding for many agents, procgen, streaming
  decisions over many cells, large `.clone()`) goes to `AsyncComputeTaskPool` via the marker pattern
  `*Requested` → `*InFlight { task: Task<T> }` → poll with `block_on(poll_once(..))` → `*Ready` →
  apply, or is time-sliced (N items per frame via a cursor resource). A naked `block_on(task)` that
  waits to completion inside a system is a blocking defect. The task body is a pure function of its
  inputs; results are applied keyed by entity, not in completion order.
- **Fixed vs variable time.** State that gameplay depends on (movement integration, physics, damage,
  timers, AI decisions) advances in `FixedUpdate` (or the physics crate's own fixed schedule).
  Presentation (camera smoothing, visual interpolation, animation blending, UI) runs in
  `Update`/`PostUpdate` from that state. Input is read in `Update`/`PreUpdate` and accumulated into an
  intent component/resource that `FixedUpdate` consumes; never read `just_pressed` / `just_released`
  inside `FixedUpdate` (it is missed or repeated when the fixed step runs 0 or 2+ times per frame).
- Exception to the fixed-time rule: timers that by design run on REAL time (GDD §3.4 `Wasted`/`Busted`,
  so slow-mo does not stretch them) tick in `Update` on `Time<Real>`, never in `FixedUpdate`.
- One-shot spawns (player, city meshes/colliders) never hang on `OnEnter` of a state the game RETURNS to
  (`Playing` after `Wasted`/`Busted`): use `OnTransition { exited: Loading, entered: Playing }` or a
  keyed one-shot, and gate "state re-entry spawns nothing twice" (TASK-006).
- Buffered cross-system signals are `Message`s (`MessageWriter<T>` / `MessageReader<T>`); `Event` +
  observers (`On<T>`, `add_observer`) are for immediate reactions. Pick by semantics and name which one
  in the plan. Confirm both APIs against the pinned Bevy source — they were renamed recently.
- Prefer `With<T>` / `Without<T>` marker filters over `Option<&T>`. Avoid per-frame insert/remove of
  components on many entities (archetype moves); use a state field or an enum component for fast-changing state.
- Use `Changed<T>` / `Added<T>` for reactive work instead of recomputing every frame — but a `Changed<T>`
  consumer is only correct if every writer really goes through `Mut<T>` of that component.
- Operations on entities that may already be despawned use `try_insert` / `try_remove` /
  `try_despawn`, or check `get_entity` first.
- State-gated systems are gated at SET level: `configure_sets(.., MySet.run_if(in_state(..)))`, not a
  per-system `resource_exists` band-aid.
- One `Plugin` per domain; the game `App` is composed in exactly one place so the headless test app and
  the windowed app share the same composition function (the test app adds `MinimalPlugins` instead of
  `DefaultPlugins`, then the same game plugins). A second hand-rolled composition for tests drifts.
- A new `Res<T>` / `ResMut<T>` parameter on an existing system is an API change for every test harness
  that registers that system: grep the system name across `src/` and `tests/`, each site must go
  through the plugin or `init_resource` the type.
- Types that QA reads or mutates over the Bevy Remote Protocol (GDD D1) `#[derive(Reflect)]` with
  `#[reflect(Component)]` / `#[reflect(Resource)]` and are registered; BRP sees nothing else.
  `bevy_brp_extras` pulls `bevy_render`, so it is a dependency of the client binary under feature `dev`
  only — never of the headless sim crate (`cargo tree -p gta_sim -e normal -i bevy_render` stays empty).
- Before claiming "system X runs in state/schedule Y", check both its `run_if` and the clock of its
  schedule (`Time<Virtual>` pause freezes `FixedUpdate` entirely).

## Risk lessons

- 2026-09-23 (inherited from the owner's previous Bevy 0.18 project) — production frame spikes (chunk spawn
  11 ms, grass 80 ms) all came from heavy work inline in a system; all were fixed by the async marker
  pattern above.
- 2026-09-23 (inherited from the owner's previous Bevy project) — a disputed Bevy runtime behaviour (message buffer rotation
  while paused) was settled by a 40-line executable probe after two plan amendments built on the wrong
  claim. When a plan depends on engine behaviour, write the probe instead of arguing from a comment.

- 2026-09-23 (TASK-002) — a crate's own manifest can switch on a render feature of a THIRD crate:
  bevy-tnua-avian3d 0.12.1 enables `avian3d/debug-plugin` → `bevy/bevy_render` unconditionally, invisible
  from both READMEs. Before adding any ecosystem crate to `gta_sim`, run
  `cargo tree -p gta_sim -e features -i bevy_render`; the fix precedent is the vendored patch (ADR-001).

- 2026-09-23 (TASK-005) — bevy-tnua 0.32 `TnuaController::is_airborne()` stays `false` while the ground
  sensor still reaches the floor (`float_height + cling_distance`, 2.05 m here): any hop under ~1 m never
  reports airborne. "In the air" for gameplay/animation = `is_airborne() || action is Jump`
  (`gta_sim::character::is_airborne`). Do not gate on Tnua's airborne flag alone.
- 2026-09-23 (TASK-005) — in Bevy 0.19.1 `WorldInstanceReady` / `GltfMeshName` are not in the prelude
  (`bevy::world_serialization::WorldInstanceReady`), and `InstanceId::new` is private: a headless test cannot
  trigger `WorldInstanceReady` by hand. Gate such handlers via a real `WorldAsset` spawn or runtime BRP QA.

- 2026-09-23 (TASK-006/007) — a `MessageReader` in a state-gated set does not consume while the state is
  off: the messages stay buffered and are read in the FIRST fixed tick after re-entry, after `OnExit` has
  already respawned the player. A read-time state check cannot catch it. Pattern: `Messages<T>::clear()`
  and reset of intent latches on `OnExit` of the state that must not leak (`flow/wasted.rs`).
- 2026-09-23 (TASK-007) — `AnimationGraph` node masks live in the shared graph asset (toggling one changes
  every character); per-character layers need duplicate nodes chosen per `AnimationPlayer`, mask groups from
  the runtime `AnimationTargetId`s. Anything parented to a glTF joint inherits the model scale (Kenney x2.68).

## Pointers

- `~/.cargo/registry/src/*/bevy_ecs-<pinned>/` — the only authority on the ECS API for this project.
- `https://bevy.org/learn/migration-guides/` — what changed between versions (read when a remembered API does not compile).
- `.claude/local/donor.md` — local-only pointers to the owner's previous Bevy project (may be absent on a fresh clone).
