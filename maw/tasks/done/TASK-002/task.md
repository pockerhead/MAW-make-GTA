# TASK-002: GDD T1 — Каркас: персонаж под камерой + agent QA

Type: feature
Mode: full
Priority: high
Branch: feature/t01-skeleton-character-camera-qa
Effort: planner=high, code-reviewer=high
Domains: bevy-ecs, gates, game-design

## Description
Implement slice **T1** of the APPROVED design document `docs/design/GDD.md` (§13), together with every
GDD section the slice touches (read them: world §2, player §3, combat §4, vehicles §5, NPC §6, UI §7,
audio/juice §8, content §9, stack §10, performance §11, workspace/plugin/data map §12). The GDD is the
scope law: its data files (§12), crate/plugin boundaries and verified crate versions (§10.1) are binding.

Goal (GDD §13): workspace из трёх крейтов, `bevy = "=0.19.1"`, avian3d 0.7.0, bevy-tnua 0.32.0 + bevy-tnua-avian3d 0.12.1, BEI 0.26.0; `Cargo.lock` в git; `compose_sim`; строгий RON-загрузчик с `ConfigRoot`; исполняемая проба "avian + Tnua под `MinimalPlugins`" первым шагом; тестовая площадка (пол, коробки, рампа 30°, лестница, стена); игрок-капсула с бегом, спринтом, прыжком; камера из-за плеча с коллизией; Esc отпускает курсор. Фичи `dev` (`bevy_remote`, `png`, `bevy_brp_extras =0.22.6`), `debug`, `profile`, `profile-tracy`. `tools/qa/brp.py` и `tools/qa/scenarios/t1.py`. Проверка дерева: `cargo tree -d`, `cargo tree -i image`, `cargo tree -i bevy_egui` с `--features dev,debug`.

## Acceptance criteria
- [ ] Headless: `MoveIntent` вперёд при yaw = 0, 64 тика → смещение по −Z между 0.8·v_run и 1.0·v_run метров (v_run из RON), |x| < 0.1 м
- [ ] Headless: то же при yaw = 90° → смещение по −X (ловит ошибку знака)
- [ ] Headless: yaw = 180° → по +Z
- [ ] Headless: прыжок → максимум высоты в пределах ±10% от `jump_height`
- [ ] Headless: неизвестное поле в RON → ошибка с именем файла и поля
- [ ] Headless: `cargo test -p gta_sim` собирается без `bevy_render` в дереве (`cargo tree -p gta_sim -e normal -i bevy_render` пуст).
- [ ] Runtime QA (GDD D1): the scenario `tools/qa/scenarios/t1.py` exists and passes via `tools/qa/brp.py` — скрипт запускает игру, ждёт порт, держит W 1000 мс через `send_keys`, сравнивает `Transform` игрока до и после (`world.query`), двигает мышь и проверяет изменение yaw камеры, снимает скриншот, читает FPS через `get_diagnostics`, вызывает `shutdown`. Отдельный пункт приёмки D1: "QA-скрипт запускает игру, делает скриншот и получает FPS".
- [ ] Owner-run criterion recorded as an owner checklist in QA_REPORT.md: `cargo run`, окно, бегает, прыгает, камера не проходит сквозь стену, feel бега и камеры устраивает (крутится в `locomotion.ron`/`camera.ron`).
- [ ] Every new tuning value lives in its GDD §12 data file, not in a `const`
- [ ] `cargo build`, `cargo clippy -- -D warnings`, `cargo test -p gta_sim` (and `-p citygen` where touched) are green
- [ ] Existing tests pass
