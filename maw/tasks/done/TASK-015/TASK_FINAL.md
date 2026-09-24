# TASK-015: GDD T14 — Управляемая машина

Type: feature
Mode: full
Priority: high
Branch: feature/t14-drivable-car
Effort: planner=high, code-reviewer=high
Domains: bevy-ecs, gates, game-design

## Description
Implement slice **T14** of the APPROVED design document `docs/design/GDD.md` (§13), together with every
GDD section the slice touches (read them: world §2, player §3, combat §4, vehicles §5, NPC §6, UI §7,
audio/juice §8, content §9, stack §10, performance §11, workspace/plugin/data map §12). The GDD is the
scope law: its data files (§12), crate/plugin boundaries and verified crate versions (§10.1) are binding.

Goal (GDD §13): Kenney Car Kit, raycast-подвеска на avian, вход/выход (F), контекст ввода `InVehicle`, камера машины, урон машины и пешеходов от относительной скорости, припаркованные машины из генератора, процедурный гул двигателя, метка машины на мини-карте.

## Dependencies
- blocked by TASK-003 — GDD slice T2 must land first
- blocked by TASK-006 — GDD slice T5 must land first
- blocked by TASK-009 — GDD slice T8 must land first
- blocked by TASK-013 — GDD slice T12 must land first

## Acceptance criteria
- [ ] Headless: машина на макс. скорости не проходит сквозь стену за 128 тиков
- [ ] Headless: то же в угол здания под 45°
- [ ] Headless: вход и выход меняют владельца управления
- [ ] Headless: удар в пешехода на 10 м/с даёт урон по формуле
- [ ] Headless: машина в покое на ровной дороге не дрейфует за 640 тиков.
- [ ] Runtime QA (GDD D1): the scenario `tools/qa/scenarios/t14.py` exists and passes via `tools/qa/brp.py` — телепорт к припаркованной машине, F, W 3000 мс, чтение скорости машины, серия скриншотов, заезд в стену, F для выхода, чтение состояния игрока.
- [ ] Owner-run criterion recorded as an owner checklist in QA_REPORT.md: сел, поехал, handling устраивает (значения в `sedan.ron`).
- [ ] Every new tuning value lives in its GDD §12 data file, not in a `const`
- [ ] `cargo build`, `cargo clippy -- -D warnings`, `cargo test -p gta_sim` (and `-p citygen` where touched) are green
- [ ] Existing tests pass

### Resolved questions

Planner's open questions, answered by the orchestrator (owner delegation, 2026-09-24):
- Q1 parking: (a) curb lane of avenues.
- Q2 bullets vs a player in a car: (c)+(a) now — bullets stop on the body AND damage the car's own health (so police fire can stall the car; the stall/eject path is gated). Wounding the driver through windows (b) is deferred to T15 with the chase. A fully invulnerable car would make T11 police pointless against a driving player.
- Q3 minimap: (a) all cars within the minimap radius.
- Q4 theft witness: (a) cops in line of sight only in T14; civilian theft witnesses with T15 traffic theft.
- Q5 exit while moving: (a) only at <= exit_max_speed.

### Orchestrator addition (binding): vendored Tnua motor bug
`vendor/bevy-tnua-avian3d-0.12.1/src/lib.rs` `apply_motors_system` does `return` on the first `TnuaToggle::Disabled|SenseOnly` entity (lines ~428-431), so while any corpse exists (corpses use `TnuaToggle::Disabled`, `population/mod.rs corpse_components`) every motor iterated after it is skipped that tick — random NPCs/player lose motor forces. Fix in the vendored crate (`continue`, per ADR-001 vendoring), record in the ADR/vendor notes, and gate it: with a corpse spawned first, a walking character still moves the expected distance; flip-RED with `return`.
