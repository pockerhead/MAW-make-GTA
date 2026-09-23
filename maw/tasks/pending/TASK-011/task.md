# TASK-011: GDD T10 — Розыск (ядро)

Type: feature
Mode: full
Priority: high
Branch: feature/t10-wanted-level
Domains: bevy-ecs, gates, game-design

## Description
Implement slice **T10** of the APPROVED design document `docs/design/GDD.md` (§13), together with every
GDD section the slice touches (read them: world §2, player §3, combat §4, vehicles §5, NPC §6, UI §7,
audio/juice §8, content §9, stack §10, performance §11, workspace/plugin/data map §12). The GDD is the
scope law: its data files (§12), crate/plugin boundaries and verified crate versions (§10.1) are binding.

Goal (GDD §13): heat-таблица, пороги звёзд, свидетельство (коп видит / звонок мирного с индикатором и прерыванием), `LastKnownPosition`, круг поиска и таймер сброса, HUD звёзд с миганием. До T11 копов нет, свидетельство копом тестируется фикстурой.

## Dependencies
- blocked by TASK-007 — GDD slice T6 must land first
- blocked by TASK-009 — GDD slice T8 must land first

## Acceptance criteria
- [ ] Headless: преступление без свидетеля = 0 heat
- [ ] Headless: звонок прерван убийством свидетеля
- [ ] Headless: пороги дают нужные звёзды
- [ ] Headless: вне круга и без видимости N с → 0 звёзд
- [ ] Headless: замечен → таймер сброшен.
- [ ] Runtime QA (GDD D1): the scenario `tools/qa/scenarios/t10.py` exists and passes via `tools/qa/brp.py` — выстрелы рядом с мирными, ожидание звонка, чтение `WantedLevel`, скриншот HUD; телепорт за круг поиска, ожидание таймера, `WantedLevel` = 0.
- [ ] Owner-run criterion recorded as an owner checklist in QA_REPORT.md: стреляет при свидетелях, видит звёзды; убегает и прячется, розыск падает.
- [ ] Every new tuning value lives in its GDD §12 data file, not in a `const`
- [ ] `cargo build`, `cargo clippy -- -D warnings`, `cargo test -p gta_sim` (and `-p citygen` where touched) are green
- [ ] Existing tests pass

## Orchestrator note (from TASK-009)

- A witness who keeps seeing the same corpse can complete several calls in a row (up to ~7 during a 30 s corpse lifetime, see TASK-009 FIX_SUMMARY.md). Wanted-level accounting must count one report per corpse/incident, not per completed call; gate it.
