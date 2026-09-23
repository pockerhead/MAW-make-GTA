# TASK-002 — отчёт реализации

## Реализовано

Три крейта, закреплённый `Cargo.lock`, vendored-патч Tnua/Avian с ADR, единый `compose_sim`, строгий RON-загрузчик, тестовая площадка, капсульный персонаж с ходьбой/бегом/спринтом/прыжком, камера с коллизией, BEI-ввод и Esc, визуал, dev/debug/profile-фичи, headless-гейты и BRP QA-сценарий.

Строки в новых файлах (по `Measure-Object -Line`; полный перечень в `scratch/line_counts.txt`):

| Файл | Строк |
|---|---:|
| `Cargo.toml`; `Cargo.lock` | 36; 7134 |
| `crates/citygen/Cargo.toml`; `src/lib.rs` | 6; 1 |
| `crates/gta_sim/Cargo.toml`; `src/lib.rs` | 15; 23 |
| `crates/gta_sim/src/config/mod.rs` | 32 |
| `crates/gta_sim/src/character/{intent,locomotion,mod}.rs` | 40; 48; 94 |
| `crates/gta_sim/src/player/mod.rs` | 26 |
| `crates/gta_sim/src/world/{mod,test_area}.rs` | 18; 42 |
| `crates/gta_sim/tests/{common/mod,config,jump,movement}.rs` | 68; 38; 42; 40 |
| `assets/{character/locomotion,camera/camera}.ron` | 14; 12 |
| `src/main.rs`; `src/input/mod.rs` | 48; 96 |
| `src/camera/{mod,config}.rs` | 117; 17 |
| `src/visuals/mod.rs`; `src/remote/mod.rs`; `src/debug/mod.rs` | 69; 8; 33 |
| `tools/qa/{brp,tree_check}.py`; `tools/qa/scenarios/t1.py` | 115; 34; 55 |
| `docs/decisions/ADR-001-vendored-tnua-avian3d.md` | 9 |
| `vendor/bevy-tnua-avian3d-0.12.1/` | 8 файлов, 4333 строки; единственное изменение к архиву: удалена строка `"debug-plugin",` в `Cargo.toml` |

`citygen` остаётся пустым крейтом по плану: генератор города — T2. Владельческий feel и проверка камеры у стены остаются пунктами `QA_REPORT.md`, как предписывает GDD.

## Отклонения от плана

- Проверенные API Bevy 0.19.1 потребовали локальных поправок: `DirectionalLight.shadow_maps_enabled` в `src/visuals/mod.rs`, `ActionEvents::START` и дополнительное разыменование `Action<Move>` в `src/input/mod.rs`.
- Flip-RED дерева выполнен возвратом `"debug-plugin",` в vendored-манифест, затем файл восстановлен. Предложенное в плане отключение `[patch.crates-io]` не удалось проверить: offline resolver конфликтует на `nix 0.31.2/0.31.3`, а TLS crates.io в среде недоступен. Прямой саботаж проверяет тот же причинный путь утечки `bevy_render`.
- Команды Cargo запускались с `--offline` из-за ошибки TLS `SEC_E_NO_CREDENTIALS`. Версии и lockfile остались закреплёнными.

## Проверки

| Команда | Результат |
|---|---|
| `cargo build -p gta_like --bin gta_like -j 4 --offline` | PASS |
| `cargo build -p gta_like --bin gta_like -j 4 --features dev,debug --offline` | PASS |
| `cargo build -p citygen --lib -j 4 --offline` | PASS |
| `cargo clippy -p gta_sim --all-targets -j 4 --offline -- -D warnings` | PASS |
| `cargo clippy -p gta_like --bin gta_like -j 4 --offline -- -D warnings` | PASS |
| `cargo clippy -p gta_like --bin gta_like -j 4 --features dev,debug --offline -- -D warnings` | PASS |
| `cargo clippy -p citygen --lib -j 4 --offline -- -D warnings` | PASS |
| `cargo test -p gta_sim -j 4 --offline` | PASS: 8/8 тестов |
| `cargo test -p citygen --lib -j 4 --offline` | PASS: 0 тестов, T2-крейт |
| `python tools/qa/tree_check.py` | PASS: headless без `bevy_render`, `image 0.25.9`, `bevy_egui 0.40.1`, без критических дублей |
| `python tools/qa/scenarios/t1.py --out maw/tasks/in_progress/TASK-002/scratch/runtime_t1` с `CARGO_NET_OFFLINE=true` | PASS: W −4.3916 м по Z, дрейф X ≈ 0; yaw −0.418879 рад; PNG, FPS 18.19, shutdown |

Flip-RED: `scratch/flip_red_headless.py` дал RED при неверном знаке оси (yaw 0/90/180), знаке yaw (90), скорости (все направления), высоте прыжка в коде и данных, потере защёлки прыжка и отключении строгого RON. После восстановления полный `cargo test -p gta_sim` дал GREEN. `scratch/flip_red_tree.py` дал RED при включении `avian3d/debug-plugin`; восстановленный `tree_check.py` дал GREEN. Результаты в `scratch/flip_red_headless_results.json` и `scratch/flip_red_tree_result.txt`.

Скриншот `scratch/runtime_t1/t1.png` осмотрен: персонаж, пол, рампа и лестница видны; кадр не чёрный. Оставшегося процесса `gta_like` нет.
`scratch/verify_vendor_copy.py` сравнил все 8 файлов с архивом крейта: отличается только указанная строка манифеста.

## Ручная проверка

Выполнить checklist в `QA_REPORT.md`: `cargo run`, движение и прыжок, рампа и лестница, камера у стены, Esc/ЛКМ, оценка feel. Настройки — `assets/character/locomotion.ron` и `assets/camera/camera.ron`.

children: 0 launched / 0 reported.
