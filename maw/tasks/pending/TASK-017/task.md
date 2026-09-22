# TASK-017: GDD T16 — Производительность и финальная приёмка

Type: feature
Mode: full
Priority: high
Branch: feature/t16-performance-final-acceptance
Domains: bevy-ecs, gates, game-design

## Description
Implement slice **T16** of the APPROVED design document `docs/design/GDD.md` (§13), together with every
GDD section the slice touches (read them: world §2, player §3, combat §4, vehicles §5, NPC §6, UI §7,
audio/juice §8, content §9, stack §10, performance §11, workspace/plugin/data map §12). The GDD is the
scope law: its data files (§12), crate/plugin boundaries and verified crate versions (§10.1) are binding.

Goal (GDD §13): `--bench-scene` (худшая сцена раздела 11), замер, исправление узких мест только по трейсу (разрезка, слияние мешей, occlusion culling по замеру, кинематические дальние NPC при нужде), чек-лист раздела 1, баг-баш.

## Dependencies
- blocked by TASK-002 — GDD slice T1 must land first
- blocked by TASK-003 — GDD slice T2 must land first
- blocked by TASK-004 — GDD slice T3 must land first
- blocked by TASK-005 — GDD slice T4 must land first
- blocked by TASK-006 — GDD slice T5 must land first
- blocked by TASK-007 — GDD slice T6 must land first
- blocked by TASK-008 — GDD slice T7 must land first
- blocked by TASK-009 — GDD slice T8 must land first
- blocked by TASK-010 — GDD slice T9 must land first
- blocked by TASK-011 — GDD slice T10 must land first
- blocked by TASK-012 — GDD slice T11 must land first
- blocked by TASK-013 — GDD slice T12 must land first
- blocked by TASK-014 — GDD slice T13 must land first
- blocked by TASK-015 — GDD slice T14 must land first
- blocked by TASK-016 — GDD slice T15 must land first

## Acceptance criteria
- [ ] Headless: все бенч-тесты T2/T8/T15 зелёные
- [ ] Headless: `cargo test -p gta_sim`, `-p citygen` зелёные
- [ ] Headless: `cargo clippy` без предупреждений.
- [ ] Runtime QA (GDD D1): the scenario `tools/qa/scenarios/t16.py` exists and passes via `tools/qa/brp.py` — `--bench-scene --features dev,profile`, `get_diagnostics` 30 с (средний и минимальный FPS, frame time), трейс `trace_chrome` с топом систем, скриншоты. Результат это доказательство для владельца, не автоматический гейт.
- [ ] Owner-run criterion recorded as an owner checklist in QA_REPORT.md: проходит все 11 пунктов раздела 1 на своей машине, включая угон, трафик и автомобильную погоню.
- [ ] Every new tuning value lives in its GDD §12 data file, not in a `const`
- [ ] `cargo build`, `cargo clippy -- -D warnings`, `cargo test -p gta_sim` (and `-p citygen` where touched) are green
- [ ] Existing tests pass
