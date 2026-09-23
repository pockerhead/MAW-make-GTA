# FIX_SUMMARY — TASK-005

## Preflight

- Прочитал `scratch/` (planner/reviewer2/implementer), это только карта покрытия. Проверку делал своими тестами и пробами.
- Самое опасное предписание ревью, если выполнить его дословно: пункт 2, "Check an explicit interval tolerance" в
  `tools/qa/scenarios/t4.py`. Жёсткий ассерт на интервал скриншотов валит сценарий из-за задержки BRP, а не из-за игры.
  Проверил на записанном прогоне `scratch/implementer/t4/summary.json`: интервалы 125..282 мс, пропуска отметки 200 мс нет,
  но допуск "200 ± немного" его бы завалил. Скриншоты в сценарии нужны владельцу как доказательство. `AnimState` (в том числе
  поза прыжка) гейтится отсчётами каждые 50 мс, не картинками. Поэтому я взял второй вариант ревьюера: отмечать
  неполные доказательства, не ронять сценарий.

## Fixed

1. **Major, `src/visuals/character_config.rs`: неполная проверка чисел.** Подтвердил: `Duration::from_secs_f32` паникует
   при переполнении (`core/src/time.rs`, `try_from_secs_f32` возвращает ошибку для отрицательных, NaN, inf и
   переполнения), `height / model_height` при субнормальном `model_height` даёт inf. Что сделано в `validate()`:
   - `blend_seconds` проверяется через `Duration::try_from_secs_f32(..).is_err()`. Это та же функция, на которой держится
     `from_secs_f32`, так что граница точная.
   - производный `scale()` должен быть конечным и > 0 (ловит и inf, и потерю точности до 0);
   - `native_speed * scale` для walk/run/sprint должно быть конечным и > 0. Это знаменатель в `playback_rate`.
   - Новый гейт `visual_config_rejects_values_that_break_derived_numbers` (`src/visuals/character_gate.rs`): шипованный
     `visual.ron` проходит; `blend_seconds 2e19`, `model_height 1e-40` (scale inf), `height 1e-45 / model_height 1e30`
     (scale 0), `walk.native_speed 3e38` (inf в метрах) отклоняются, и сообщение называет поле. Класс: корректность.
   - Flip-RED 1: вернул `character_config.rs` из HEAD → RED `Duration overflow: ()` (validate принял 2e19). Вернул фикс → GREEN.
   - Flip-RED 2: оставил фикс Duration и убрал только проверки scale/скорости клипа → RED `infinite scale: ()`. Вернул → GREEN.
2. **Minor, `tools/qa/scenarios/t4.py`: темп скриншотов не проверяется.** Диагноз верный, предписание пересчитал (см.
   preflight). Добавлен `cadence_misses(shots)`: интервал > 2 × 200 мс значит, что отметка 200 мс пропущена. Список
   пишется в `summary.json` у каждой фазы (`cadence_misses`) и печатается в итоговом JSON как
   `screenshot_cadence_misses`. Сценарий от этого не падает. Решение записано в `log.jsonl` (decision).
   Проверка офлайн: на записанном прогоне пропусков нет (`{'run': [], 'sprint': [], 'walk': [], 'jump': []}`), на
   синтетическом интервале 401 мс пропуск найден. Живой прогон `t4.py` не делал, это стадия QA.

## Skipped

- Ничего не пропущено целиком. Из пункта 2 не взят жёсткий ассерт интервала, причина выше.
- Замечание ревью о `cargo build` (`os error 5`) относится к среде ревьюера. У меня `cargo build` прошёл.

## Test results

```
cargo test -p gta_like --bin gta_like      -> test result: ok. 15 passed; 0 failed
cargo clippy -- -D warnings                -> Finished, без предупреждений
cargo clippy -p gta_like --tests -- -D warnings -> без warning/error
cargo build                                -> Finished `dev` profile
cargo test -p gta_sim -p citygen           -> все наборы ok, 0 failed (ignored как и раньше)
python tools/fetch_assets.py --check       -> third-party packs match the manifest
```

`git status --short`: изменены только `src/visuals/character_config.rs`, `src/visuals/character_gate.rs`,
`tools/qa/scenarios/t4.py` (плюс артефакты в task dir). `rustfmt --edition 2024` только по двум правленым Rust-файлам.

## Owner checklist (без изменений)

Тинт не белого цвета и "ноги заметно не скользят" остаются проверкой владельца (PLAN_FINAL §3.4).

children: 0 launched / 0 reported.
