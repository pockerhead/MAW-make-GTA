# TASK-014: GDD T13 — Звук и juice

Type: feature
Mode: full
Priority: high
Branch: feature/t13-audio-juice
Domains: bevy-ecs, gates, game-design

## Description
Implement slice **T13** of the APPROVED design document `docs/design/GDD.md` (§13), together with every
GDD section the slice touches (read them: world §2, player §3, combat §4, vehicles §5, NPC §6, UI §7,
audio/juice §8, content §9, stack §10, performance §11, workspace/plugin/data map §12). The GDD is the
scope law: its data files (§12), crate/plugin boundaries and verified crate versions (§10.1) are binding.

Goal (GDD §13): процедурные выстрелы, Kenney CC0 импакты и интерфейс, эмбиент города и парков, пространственные сирены, stinger розыска, индикатор направления урона, trauma-тряска, виньетка, пульс звёзд, переключатели доступности.

## Dependencies
- blocked by TASK-007 — GDD slice T6 must land first
- blocked by TASK-008 — GDD slice T7 must land first
- blocked by TASK-012 — GDD slice T11 must land first

## Acceptance criteria
- [ ] Headless: манифест звуков валиден
- [ ] Headless: ни одна система juice не пишет `Time<Virtual>` (тест или grep-гейт).
- [ ] Runtime QA (GDD D1): the scenario `tools/qa/scenarios/t13.py` exists and passes via `tools/qa/brp.py` — серия выстрелов и ударов, скриншоты с эффектами, чтение числа активных `AudioPlayer`-сущностей (звук проверяется только наличием источников).
- [ ] Owner-run criterion recorded as an owner checklist in QA_REPORT.md: игра "звучит" и "бьёт"; звук и feel принимает только владелец.
- [ ] Every new tuning value lives in its GDD §12 data file, not in a `const`
- [ ] `cargo build`, `cargo clippy -- -D warnings`, `cargo test -p gta_sim` (and `-p citygen` where touched) are green
- [ ] Existing tests pass
