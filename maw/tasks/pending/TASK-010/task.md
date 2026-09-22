# TASK-010: GDD T9 — Банды

Type: feature
Mode: full
Priority: high
Branch: feature/t09-gangs
Domains: bevy-ecs, gates, game-design

## Description
Implement slice **T9** of the APPROVED design document `docs/design/GDD.md` (§13), together with every
GDD section the slice touches (read them: world §2, player §3, combat §4, vehicles §5, NPC §6, UI §7,
audio/juice §8, content §9, stack §10, performance §11, workspace/plugin/data map §12). The GDD is the
scope law: its data files (§12), crate/plugin boundaries and verified crate versions (§10.1) are binding.

Goal (GDD §13): 2 банды, территории от генератора, тинты, группы у штабов, предупреждение, групповой аггро, бой (дистанция, разброс, ближний бой, отступление), выпадение оружия, матрица фракций с выключенными парами gang↔gang и gang↔police.

## Dependencies
- blocked by TASK-007 — GDD slice T6 must land first
- blocked by TASK-008 — GDD slice T7 must land first
- blocked by TASK-009 — GDD slice T8 must land first

## Acceptance criteria
- [ ] Headless: вне территории нейтральны
- [ ] Headless: атака на члена банды → вся группа в 30 м в `Attack`
- [ ] Headless: `GangHeat` затухает за 120 с
- [ ] Headless: при выключенной матрице бандиты не атакуют друг друга.
- [ ] Runtime QA (GDD D1): the scenario `tools/qa/scenarios/t9.py` exists and passes via `tools/qa/brp.py` — телепорт в штаб банды, выстрел рядом, чтение состояний бандитов (`Attack`), скриншот.
- [ ] Owner-run criterion recorded as an owner checklist in QA_REPORT.md: зашёл к бандитам, спровоцировал, получил перестрелку.
- [ ] Every new tuning value lives in its GDD §12 data file, not in a `const`
- [ ] `cargo build`, `cargo clippy -- -D warnings`, `cargo test -p gta_sim` (and `-p citygen` where touched) are green
- [ ] Existing tests pass
