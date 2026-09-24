# PCTX proposals — TASK-025

## 2026-09-24 (implementer) — gates: "all entities" in Bevy 0.19 includes resources

In Bevy 0.19.1 every resource is an entity carrying `bevy_ecs::resource::IsResource`
(`World::iter_entities` doc: "including resource entities"). A leak gate that diffs the set of all
entities against a baseline therefore flags resources inserted during play (`PreviousState<GameState>`
and four more after a pause and a new city) as leaks. Filter `IsResource` out of the diff; do not fall back
to a component query (that is what missed the Tnua sensor orphans). Precedent: `crates/gta_sim/tests/sensor_leak.rs`
`all_entities`. Proposed home: domain `gates`, risk lesson.

## 2026-09-24 (implementer) — gates: "Новый город" is `Paused -> Loading`

`NEW_CITY` is `OnTransition { exited: Paused, entered: Loading }`. A headless test that sets
`NextState(Loading)` straight from `Playing` gets a Loading state with the old city still alive, and a
leak gate over it fails for the wrong reason. Pause first (`new_city.rs::pause`, `sensor_leak.rs::set_state`).
Proposed home: domain `gates` or `bevy-ecs`, risk lesson.
