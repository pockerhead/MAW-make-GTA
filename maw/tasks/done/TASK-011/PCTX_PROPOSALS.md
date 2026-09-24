# PCTX proposals — TASK-011

- 2026-09-24 (implementer, domain bevy-ecs / gates) — avian3d 0.7 registers `Position -> Transform` as a
  required component (`physics_transform/mod.rs:81`) and `transform_to_position` copies `GlobalTransform` into
  `Position` every step. A test fixture spawned with only `Position` gets a default `Transform` and snaps to the
  origin (TASK-011 cop fixture: every cop gate passed vacuously until a debug print showed it). Rule: a fixture that
  needs a pose carries a matching `Transform`; assert the fixture's position after a tick. Trigger: `Position(` in
  `tests/**` without `Transform`.
- 2026-09-24 (implementer, domain gates) — bevy_remote 0.19.1 `world.mutate_components` on a despawned entity
  panics the game (`builtin_methods.rs:1194`, `world.entity_mut`), it does not return an error. Calm civilians past
  `recycle_distance` are recycled at any tick, so a BRP scenario must move the player next to an NPC (or re-query
  it) right before mutating it. Trigger: `mutate_component` on a civilian in `tools/qa/scenarios/*.py`.
