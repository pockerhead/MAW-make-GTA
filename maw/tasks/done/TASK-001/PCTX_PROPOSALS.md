# PCTX proposals — TASK-001

- 2026-09-23 (plan-reviewer-2) — bevy-ecs domain: components/resources that QA reads or mutates over BRP (D1) must `#[derive(Reflect)]` + `#[reflect(Component)]`/`#[reflect(Resource)]`; `bevy_brp_extras` pulls `bevy_render`, so it may only be a dependency of the client binary under feature `dev`, never of the headless sim crate. Why: BRP sees only reflected, registered types, and a render dependency in the sim crate silently breaks the headless law.

> RESOLVED: folded into maw/project-context/domains/bevy-ecs.md (Invariants) on 2026-09-23
