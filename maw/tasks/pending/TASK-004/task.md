# TASK-004: GDD T3 — Облик города

Type: feature
Mode: full
Priority: high
Branch: feature/t03-city-look
Domains: bevy-ecs, gates, game-design

## Description
Implement slice **T3** of the APPROVED design document `docs/design/GDD.md` (§13), together with every
GDD section the slice touches (read them: world §2, player §3, combat §4, vehicles §5, NPC §6, UI §7,
audio/juice §8, content §9, stack §10, performance §11, workspace/plugin/data map §12). The GDD is the
scope law: its data files (§12), crate/plugin boundaries and verified crate versions (§10.1) are binding.

Goal (GDD §13): тротуары с бордюром, разметка, фасады (шейдер окон), уступы высоток, Kenney-пропы, ориентиры (башня, площадь, центральный парк), солнце с тенями, небо, `DistanceFog`, слияние мешей по чанкам (размер в `render.ron`), `VisibilityRange` для пропов, `tools/fetch_assets` + манифест.

## Dependencies
- blocked by TASK-003 — GDD slice T2 must land first

## Acceptance criteria
- [ ] Headless: манифест валиден (SHA-256, лицензия, ожидаемый состав на каждый пакет)
- [ ] Headless: число сущностей-чанков соответствует размеру города и размеру чанка из `render.ron` (ловит забытое слияние).
- [ ] Runtime QA (GDD D1): the scenario `tools/qa/scenarios/t3.py` exists and passes via `tools/qa/brp.py` — телепорт на крышу самой высокой башни и в парк, скриншоты; `get_diagnostics` на обзоре с крыши записывает FPS (доказательство для владельца, не автоматический гейт).
- [ ] Owner-run criterion recorded as an owner checklist in QA_REPORT.md: город выглядит как город, районы различимы.
- [ ] Every new tuning value lives in its GDD §12 data file, not in a `const`
- [ ] `cargo build`, `cargo clippy -- -D warnings`, `cargo test -p gta_sim` (and `-p citygen` where touched) are green
- [ ] Existing tests pass
