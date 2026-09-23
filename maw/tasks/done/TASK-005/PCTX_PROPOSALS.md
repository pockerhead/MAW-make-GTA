# PCTX proposals — TASK-005

## 2026-09-23 (TASK-005, plan-reviewer-2) — domain bevy-ecs, Risk lessons

Proposed lesson: in Bevy 0.19.1 `WorldInstanceReady` is NOT in `bevy::prelude` (import `bevy::world_serialization::WorldInstanceReady`), and `InstanceId::new` is private, so a headless test cannot trigger `WorldInstanceReady` by hand. A handler of that event is gated either by spawning a real `WorldAsset` or by runtime QA over BRP. Plan for this up front instead of promising a headless gate on it. Evidence: `bevy_world_serialization-0.19.1/src/lib.rs:38-44`, `world_asset_spawner.rs:49-58`.

## 2026-09-23 (TASK-005, implementer) — domain bevy-ecs, Risk lessons

Proposed lesson: bevy-tnua 0.32 `TnuaController::is_airborne()` (walk basis) stays `false` while the ground sensor still reaches the floor, i.e. within `float_height + cling_distance` (2.05 m here). With `ground_sensor_cling_distance: 1.0` any hop under ~1 m never reports airborne; the full 1.0 m jump flips it only near the apex. "In the air" for gameplay/animation = `is_airborne() || jump action running` (the jump action stays active until landing). A headless jump gate that only uses the full-height jump hides this; test a short tap too. Evidence: `bevy-tnua-0.32.0/src/builtins/walk.rs:404-440`, `jump.rs:452-490`, TASK-005 `scratch/implementer/probe_short_hop.rs`, `t4_run1.log`.

> RESOLVED: both folded into maw/project-context/domains/bevy-ecs.md (Risk lessons) on 2026-09-23
