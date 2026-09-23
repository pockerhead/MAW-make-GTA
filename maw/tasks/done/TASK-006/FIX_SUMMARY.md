# FIX_SUMMARY — TASK-006 (fixer, claude/opus)

## Preflight

- Прочитал scratch/ (flip_red.py/log, probe_wasted_timing.txt, python_parity.txt, t5/ с PNG и summary.json, stale_capture.png ревьюера) как карту покрытия.
- Прочитал IMPL_REVIEW.md (NEEDS_WORK, 2 пункта).
- Самое опасное предписание, если выполнить буквально: п.2 "make the polygon lookup fallible and propagate None", то есть поменять сигнатуру `road_polygon`. Проверил: `road_polygon` вызывают ещё `sidewalks()` (graphs.rs:42) и `player_spawn` (graphs.rs:141). Оба на пути генерации layout и golden-хэшей. Смена сигнатуры потянула бы генератор ради контракта одного helper'а. Диагноз верный, предписание переписал: fallible lookup только внутри `sidewalk_anchor`, `road_polygon` не трогал.

## Fixed

1. **Major, `tools/qa/scenarios/t5.py` screenshot() принимал старый PNG.** Подтвердил: `screenshot()` ждал только `path.is_file()` и сигнатуру PNG, `run()` пишет в тот же `--out`, так что PNG прошлого прогона проходил как новый снимок, а следующая проверка `Wasted` заверяла старый кадр.
   Сделано: перед запросом `path.unlink(missing_ok=True)`, потом ожидание, пока файл существует и начинается с сигнатуры PNG (хелпер `published()`). Это же закрывает окно, когда файл уже создан, а байты ещё не записаны. Имена файлов оставил стабильными (`wasted.png` и т.д.), решение в log.jsonl.
   Гейт: `scratch/probe_stale_screenshot.py`. Заглушка игры, которая ничего не пишет, и старый PNG на месте цели: хелпер должен упасть. Заглушка, которая пишет свежий PNG: хелпер должен принять именно свежие байты.
   Flip-RED: `git stash` правки t5.py, запуск пробы → `STALE ACCEPTED (defect)`, exit 1 (RED). Правку вернул → `stale rejected ...` / `fresh capture accepted`, exit 0 (GREEN).
   Отдельного Python-теста в репо нет (у tools/qa нет тестовой обвязки), поэтому гейт остаётся пробой в scratch. У `t3.py:26-32` тот же дефект, но это вне скоупа T5, я его не трогал. Стоит завести задачу.

2. **Minor, `crates/citygen/src/graphs.rs` sidewalk_anchor падал с паникой на битом индексе узла.** Подтвердил: док обещает `None` для плохого индекса, а `road_polygon` делает `roads.nodes[n as usize]`. Длину кольца проверил отдельно: `geom::inset` возвращает `None` при `poly.len() != offsets.len()` (geom.rs:32), поэтому `ring[k]` для k по `classes` безопасен.
   Сделано: в `sidewalk_anchor` полигон собирается через `layout.roads.nodes.get(n as usize).copied()` → `Option<Vec<_>>` → `?`. `road_polygon`, генерация и golden не изменились.
   Гейт: `crates/citygen/tests/properties.rs::sidewalk_anchor_rejects_bad_indices` (корректность контракта). Индекс здания за пределами даёт `None`. Клон layout seed 1, где у квартала больницы узел заменён на `u32::MAX`, тоже даёт `None`.
   Flip-RED: временно вернул прямую индексацию (`Some(layout.roads.nodes[n as usize])`) → RED, panic `graphs.rs:182:42` index out of bounds. Откатил → GREEN. Первая попытка флипа была неверной: подмена только вызова `inset` оставляла `?` на новом полигоне, и тест прошёл. Это не RED, в зачёт не пошло.

## Skipped

- Ничего из пунктов ревью не пропущено. Оценку визуала и ощущения slow-mo ревьюер оставил владельцу, я тоже (owner-чеклист G3 плана, его ведёт QA).
- Live BRP сценарий `t5.py` я не перезапускал. Логика прогона не менялась, поменялся только хелпер скриншота, и он покрыт пробой. Полный прогон остаётся за QA.

## Test results

- `cargo test -p citygen` → все наборы ok (properties 13 passed, включая новый тест).
- `cargo test -p gta_sim -p citygen` → все 16 наборов `test result: ok`, 0 failed.
- `cargo clippy --workspace --all-targets -- -D warnings` → `Finished`, без предупреждений.
- `rustfmt --edition 2024 --check` на обоих изменённых .rs → без диффа.
- `python maw/tasks/in_progress/TASK-006/scratch/probe_stale_screenshot.py` → `stale rejected: ... was not published as PNG` / `fresh capture accepted`, exit 0.

## Changed files

- `crates/citygen/src/graphs.rs`
- `crates/citygen/tests/properties.rs`
- `tools/qa/scenarios/t5.py`
- `maw/tasks/in_progress/TASK-006/log.jsonl` (2 записи decision)
- `maw/tasks/in_progress/TASK-006/scratch/probe_stale_screenshot.py` (проба)

children: 0 launched / 0 reported.
