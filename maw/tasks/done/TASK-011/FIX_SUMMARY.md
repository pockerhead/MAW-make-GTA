# FIX_SUMMARY — TASK-011

Preflight: самое рискованное предписание ревью — альтернатива "retain incidents referenced by calls" (держать инциденты, на которые ссылаются звонки). Звонок в пути ничего не знает об инциденте до завершения, так что это потребовало бы новой связи civilian->wanted. Отклонено. Сам диагноз подтверждён: `wanted/mod.rs` проверял только `memory > merge`, а merge 0 / memory 1 проходил.

## Fixed
- Minor, `wanted/mod.rs` (память инцидента короче звонка): добавил `WantedConfig::validate_call_delay(call_delay)`. `compose_sim` (`crates/gta_sim/src/lib.rs`) вызывает его после `validate()` с `call_delay = slots * tick + call_seconds`. Тик берётся из `Time::<Fixed>::default().timestep()` (64 Гц, в репо не переопределён), нового const нет. Ошибка указывает путь `wanted.ron`.
- Missing coverage: `crates/gta_sim/tests/config.rs::incident_memory_must_outlast_a_civilian_call`. Отгруженный конфиг проходит, задержка 4.0625 закреплена `GATE BROKEN`-проверкой. merge 0 / memory 4.0 проходит `validate()`, но `validate_call_delay` его отклоняет.
  Flip: условие заменил на `memory > 0.0`, тест упал (RED, config.rs:700). После восстановления GREEN.

## Skipped
- Вариант "retain incidents referenced by calls": не делал, причина в preflight выше. Хватает проверки на загрузке.
- Проводку в `compose_sim` отдельный тест не проверяет (для этого нужна копия всего дерева assets). Тест дублирует формулу задержки. Все headless-приложения идут через `compose_sim` с отгруженными значениями, поэтому эта проводка выполняется на каждом прогоне.

## Test results
- `CARGO_TARGET_DIR=D:/test-gta-like/target cargo clippy -p gta_sim --all-targets -j 4 -- -D warnings` -> Finished, 0 warnings.
- `cargo test -p gta_sim -j 4` -> все бинари `test result: ok`, 0 failed (config: 45 passed).
