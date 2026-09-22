# TASK-003: GDD T2 — Генератор города v1 и прогулка

Type: feature
Mode: full
Priority: high
Branch: feature/t02-citygen-v1
Effort: planner=high, code-reviewer=high
Domains: bevy-ecs, gates, game-design

## Description
Implement slice **T2** of the APPROVED design document `docs/design/GDD.md` (§13), together with every
GDD section the slice touches (read them: world §2, player §3, combat §4, vehicles §5, NPC §6, UI §7,
audio/juice §8, content §9, stack §10, performance §11, workspace/plugin/data map §12). The GDD is the
scope law: its data files (§12), crate/plugin boundaries and verified crate versions (§10.1) are binding.

Goal (GDD §13): `citygen` (сетка, районы, кварталы, лоты, коробки, парки, больница, участок, штабы банд, графы тротуаров и полос), асинхронная генерация под экраном загрузки, спавн коллайдеров и простых мешей (цвет района), `--seed`, игрок появляется на тротуаре, ресурс `CityLayoutHash`.

## Dependencies
- blocked by TASK-002 — GDD slice T1 must land first

## Acceptance criteria
- [ ] Headless: golden-хэш layout для 3 seed
- [ ] Headless: связность графа дорог и сильная связность графа полос
- [ ] Headless: лоты не пересекаются и выходят к улице
- [ ] Headless: больница, участок и 2 штаба есть
- [ ] Headless: время генерации в release < 2 с (`#[ignore]`, запуск в QA).
- [ ] Runtime QA (GDD D1): the scenario `tools/qa/scenarios/t2.py` exists and passes via `tools/qa/brp.py` — запуск с `--seed 1` и `--seed 2`, чтение `CityLayoutHash` (хэши различны и равны golden), телепорт игрока в центр через `world.mutate_components`, скриншоты с земли для обоих seed.
- [ ] Owner-run criterion recorded as an owner checklist in QA_REPORT.md: бегает по городу, видит улицы, кварталы разной высоты и парки; другой seed даёт другой город.
- [ ] Every new tuning value lives in its GDD §12 data file, not in a `const`
- [ ] `cargo build`, `cargo clippy -- -D warnings`, `cargo test -p gta_sim` (and `-p citygen` where touched) are green
- [ ] Existing tests pass
