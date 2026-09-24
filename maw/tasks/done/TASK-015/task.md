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
