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

### Resolved questions

Answered by the orchestrator under the owner's delegation (2026-09-23):
- Q1 (bevy_render leaks via bevy-tnua-avian3d → avian3d/debug-plugin): **A** — vendored copy of bevy-tnua-avian3d 0.12.1 in `vendor/` with the single `"debug-plugin",` line removed, wired via `[patch.crates-io]`, decision recorded as `docs/decisions/ADR-001-*.md`. The headless boundary stays a build-time gate; the upgrade cost is accepted.
- Q2 (test arena geometry): **A** — cuboid table in code (`world/test_area.rs`), it is level content replaced by the city in T2, not tuning.
- Q3 (`jump_takeoff_extra_gravity`): **A** — 10.0 in `assets/character/locomotion.ron`; final value is the owner's run.

### Owner addition (2026-09-23, during implementation) — fast compiles, mandatory in this task

Source: Bevy Quick Start "Enable fast compiles" (https://bevy.org/learn/quick-start/getting-started/setup/). Stay on the stable pinned toolchain (no nightly-only options: no cranelift, no `-Zshare-generics`).
- [ ] Root `Cargo.toml`: `[profile.dev] opt-level = 1` and `[profile.dev.package."*"] opt-level = 3` (Bevy recommendation; also mandatory on Windows for `dynamic_linking`, otherwise "too many exported symbols").
- [ ] Client feature `fast = ["bevy/dynamic_linking"]` for local iteration (`cargo run --features fast`), NOT default and never used for release/QA-shipping builds (it needs `bevy_dylib` next to the exe). `cargo test -p gta_sim` stays unaffected.
- [ ] `.cargo/config.toml`: `[target.x86_64-pc-windows-msvc] linker = "rust-lld.exe"` (ships with the stable toolchain at `lib/rustlib/x86_64-pc-windows-msvc/bin/rust-lld.exe`).
- [ ] Evidence in FIX_SUMMARY: incremental rebuild time of the client after touching one client source file, before vs after (same command, same features), plus `cargo run --features fast` and `--features dev` both start; headless tests and the BRP scenario still pass.

### Owner run finding (2026-09-23, corrected by the owner) — jump "mantles" onto ledges

Owner: ledges correctly BLOCK walking. But when the owner JUMPS at a ledge, the capsule "дозабирается" onto it — it hauls itself up onto the top. The owner finds it funny, not annoying.
Likely mechanism (verify in Tnua 0.32 source, do not assume): near the jump apex the ledge top enters the float/ground-sensor range and the float spring pulls the body up onto it.
Decision (orchestrator, owner delegation): keep it as a deliberate "ledge assist" (GTA-like auto-climb of low obstacles), do not remove it.
- [ ] Name the mechanism in FIX_SUMMARY with file:line in the Tnua source.
- [ ] The pull-up is quick and clean (no slow crawl up the edge); its reach is governed by named values in `assets/character/locomotion.ron`, not by an accident of geometry.
- [ ] Walking still does not step onto the arena boxes; stairs (0.2 m) are still walked up smoothly.
- [ ] Jump height and movement gates stay green.
