# TASK-007: GDD T6 — Стрельба

Type: feature
Mode: full
Priority: high
Branch: feature/t06-shooting
Domains: bevy-ecs, gates, game-design

## Description
Implement slice **T6** of the APPROVED design document `docs/design/GDD.md` (§13), together with every
GDD section the slice touches (read them: world §2, player §3, combat §4, vehicles §5, NPC §6, UI §7,
audio/juice §8, content §9, stack §10, performance §11, workspace/plugin/data map §12). The GDD is the
scope law: its data files (§12), crate/plugin boundaries and verified crate versions (§10.1) are binding.

Goal (GDD §13): оружие из `weapons.ron` (пистолет, SMG, дробовик), режим прицела (камера и strafe), `AimIntent` из клиента, hitscan двумя лучами, сенсор головы в слое `Hitbox` (сначала проверить, что сенсор земли Tnua его не видит), разброс, магазин, перезарядка, смена оружия, пикапы оружия и патронов, манекены-мишени, HUD патронов, прицел, хит-маркер, вспышка, трассер, отдача камеры.

## Dependencies
- blocked by TASK-006 — GDD slice T5 must land first

## Acceptance criteria
- [ ] Headless: луч по манекену на 10 м → урон по таблице
- [ ] Headless: стена между дулом и целью блокирует
- [ ] Headless: попадание в сенсор головы → ×2
- [ ] Headless: перезарядка по времени
- [ ] Headless: дробовик даёт 10 лучей
- [ ] Headless: разброс растёт при серии и сжимается со временем.
- [ ] Runtime QA (GDD D1): the scenario `tools/qa/scenarios/t6.py` exists and passes via `tools/qa/brp.py` — телепорт к манекену, наведение `move_mouse`, `send_mouse_button` ЛКМ, чтение `Health` манекена и патронов игрока, скриншот с трассером и хит-маркером.
- [ ] Owner-run criterion recorded as an owner checklist in QA_REPORT.md: стрельба ощущается (отдача, звук-заглушка, трассер), попадания читаются.
- [ ] Every new tuning value lives in its GDD §12 data file, not in a `const`
- [ ] `cargo build`, `cargo clippy -- -D warnings`, `cargo test -p gta_sim` (and `-p citygen` where touched) are green
- [ ] Existing tests pass
