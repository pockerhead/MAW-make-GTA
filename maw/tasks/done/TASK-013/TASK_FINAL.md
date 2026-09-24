# TASK-013: GDD T12 — Мини-карта, меню и настройки

Type: feature
Mode: full
Priority: high
Branch: feature/t12-minimap-menu-settings
Domains: bevy-ecs, gates, game-design

## Description
Implement slice **T12** of the APPROVED design document `docs/design/GDD.md` (§13), together with every
GDD section the slice touches (read them: world §2, player §3, combat §4, vehicles §5, NPC §6, UI §7,
audio/juice §8, content §9, stack §10, performance §11, workspace/plugin/data map §12). The GDD is the
scope law: its data files (§12), crate/plugin boundaries and verified crate versions (§10.1) are binding.

Goal (GDD §13): растр карты из layout, вращающаяся мини-карта с метками, территориями, кругом поиска и конусами копов; главное меню с seed; пауза и "Новый город"; настройки через `SettingsPlugin` (фича `bevy_settings`, reverse-domain id, `SaveSettingsDeferred` + сохранение при выходе).

## Dependencies
- blocked by TASK-003 — GDD slice T2 must land first
- blocked by TASK-011 — GDD slice T10 must land first
- blocked by TASK-012 — GDD slice T11 must land first

## Acceptance criteria
- [ ] Headless: растеризация детерминирована (хэш картинки по seed)
- [ ] Headless: проекция мира в мини-карту на 3 направленных примерах: точка в 10 м впереди камеры при yaw 0°, 90°, 180° попадает строго вверх от центра карты.
- [ ] Runtime QA (GDD D1): the scenario `tools/qa/scenarios/t12.py` exists and passes via `tools/qa/brp.py` — Esc, скриншот паузы, ввод seed через `type_text`, подтверждение, чтение нового `CityLayoutHash`; скриншот мини-карты при активном розыске.
- [ ] Owner-run criterion recorded as an owner checklist in QA_REPORT.md: ориентируется по мини-карте, меняет seed из меню, настройки сохраняются между запусками.
- [ ] Every new tuning value lives in its GDD §12 data file, not in a `const`
- [ ] `cargo build`, `cargo clippy -- -D warnings`, `cargo test -p gta_sim` (and `-p citygen` where touched) are green
- [ ] Existing tests pass

### Resolved questions

Planner's open questions, answered by the orchestrator (owner delegation, 2026-09-24):
- Seed input in the pause menu: a text field, empty = random seed.
- All five accessibility settings land in T12 (not T13).
- Minimap shape: round.
- Esc now opens the pause menu (was: release the cursor); ignored during Wasted/Busted. Noted for the owner checklist.
