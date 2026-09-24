# FIX_SUMMARY — TASK-024 (fixer, раунд 1)

Вход: `IMPL_REVIEW.md` (NEEDS_WORK) + заметка оркестратора (Major + оба пункта Missing coverage).

Pre-flight: scratch/ прочитан как карта покрытия (`flips.py`, `star_backdrops.py`, `stars_after/`, `t10_runs.*`).
Самая конкретная претензия ревью, которая при дословном исполнении сломала бы код: "retain a distinct outline or
faint shadow for earned stars through the dark phase". Если просто вернуть тень на `Off`, тень появится и у пустых
слотов (у них та же ветка `StarLook::Off`), и различие исчезнет с другой стороны. Проверено в `src/hud/stars.rs:24-36`
и `:96-101` (до правки): `star_look` отдаёт `Off` и для незаработанного слота, и для тёмной фазы заработанной звезды,
`update_stars` красит оба одинаково. Диагноз ревью верен, рецепт пересчитан: нужен отдельный вид, а не тень на `Off`.
Ещё одно расхождение с ревью: "this is a visual acceptance check, not a headless color test". Оркестратор попросил
unit-тест, и он имеет смысл: он ловит механизм (тёмная фаза свалилась в `Off`), а вид остаётся за владельцем.

## 1. Fixed

| Пункт ревью | Что сделано |
|---|---|
| Major: в тёмной фазе мигания заработанная звезда = пустой слот (`src/hud/stars.rs:34`, `:100`) | Новый `StarLook::Dim` (заработанная, тёмная фаза). `star_look` отдаёт `Dim` вместо `Off`. Чистая функция `star_style(look, cfg) -> (fill, shadow)`, `update_stars` берёт цвета из неё. `Dim` = непрозрачная тёмно-серая заливка `hud.stars.dim_color` (0.40) + та же тень, что у яркой фазы. Пустой слот без изменений: белый 25 % без тени. Данные: `assets/ui/strings.ron` (`dim_color`), `src/menu/config.rs` (поле + валидация [0,1]), экспорт `StarsConfig` из `src/menu/mod.rs`. |
| Там же: unit-тест | `star_look_table`: тёмная фаза теперь `[Dim, Dim, Off, Off, Off]`. Новый `earned_star_styles_differ_from_empty`: на поставленном `strings.ron` стиль `Lit`/`Gray`/`Dim` != стиль `Off`, и `Gray` != `Dim` (мигание видно). |
| Missing coverage 1: скриншоты обеих фаз | `scratch/star_backdrops.py --out scratch/stars_fixer` (release+dev, seed 1, стена больницы / небо / асфальт). Обе фазы на всех трёх фонах попали в серию (кадры чередуются через 0.2 с при фазе 0.25 с). Шесть кадров для владельца: `scratch/stars_fixer/zoom_{wall,sky,street}_heat180_0.png` (яркая фаза) и `zoom_{wall,sky,street}_heat180_2.png` (тёмная фаза). Таблица ниже. |
| Missing coverage 2: кросс-проверка `fight_report_min_distance < fight_hearing_radius` | `CivilianConfig::validate_fight_hearing(fight_hearing_radius)` в `crates/gta_sim/src/civilian/mod.rs`, вызывается в общем пути `compose_sim` (`crates/gta_sim/src/lib.rs`) сразу после `civilian.validate()`, ошибка на `npc/civilian.ron`. Строгое `<`: при равенстве кольцо сообщения схлопывается в точку. Фикстура `fight_report_distance_below_fight_hearing` (`crates/gta_sim/tests/config.rs`): копия 12 конфигов sim во временный корень, `fight_report_min_distance: 15.0 -> 21.0` (строго за 20 м слышимости, не на границе), `CivilianConfig::validate()` проходит, `compose_sim` возвращает ошибку с путём `civilian.ron` и именем поля. Через `compose_sim`, а не только метод: так ловится и выпавший вызов. |

### Звёзды: luma по фазам (`scratch/fixer_phase_table.py` -> `scratch/fixer_phase_table.txt`)

Средняя sRGB luma пикселей глифа; заработанные = слоты 1-2, пустые = 3-5 (heat 180, 2 звезды). |Δ| к фону =
среднее |luma кадра - luma голого фона| по пикселям глифа (тень тоже считается).

| Фон | Фаза | Кадр | Заработанная | Пустая | Δ luma | |Δ| к фону: заработ. / пустая |
|---|---|---|---|---|---|---|
| стена | яркая | `zoom_wall_heat180_0.png` | 166.4 | 220.9 | -54.5 | 61.1 / 7.3 |
| стена | тёмная | `zoom_wall_heat180_2.png` | 117.6 | 220.9 | -103.3 | 108.8 / 7.3 |
| небо | яркая | `zoom_sky_heat180_0.png` | 149.9 | 163.4 | -13.5 | 53.3 / 29.0 |
| небо | тёмная | `zoom_sky_heat180_2.png` | 91.7 | 163.4 | -71.7 | 40.7 / 29.0 |
| асфальт | яркая | `zoom_street_heat180_0.png` | 143.6 | 140.4 | +3.2 | 79.3 / 44.7 |
| асфальт | тёмная | `zoom_street_heat180_2.png` | 97.5 | 140.4 | -42.9 | 42.2 / 44.7 |

До правки (`scratch/stars_after/analysis.json`, кадры имплементера) тёмная фаза рисовала заработанные слоты тем же
`off_color` без тени, что и пустые: стена 219.2 vs 221.0 (Δ -1.8), небо 162.2 vs 163.4 (Δ -1.2), то есть счёт
пропадал. Яркая фаза не менялась (те же числа, что у имплементера).

Честные оговорки:
- На асфальте в яркой фазе Δ между слотами +3.2: фон под слотами неоднородный (93..191, полоса света), поэтому
  межслотовая дельта тут мало что значит. Различие держится на тени и на |Δ| к фону (79 против 45). На тёмном
  асфальте тёмная фаза даёт тёмную звезду на тёмном фоне (слот 1: |Δ| к фону 26): её видно по контуру тени и по
  отличию от светлого призрака, но это самый слабый случай. Решает владелец по `zoom_street_heat180_2.png`.
- Я посмотрел три зума тёмной фазы: на стене, небе и асфальте две тёмные звезды отличаются от трёх светлых призраков.

## 2. Skipped

- Ничего из списка оркестратора не пропущено.
- Рецепт ревью "retain a faint shadow" дословно не применён (см. pre-flight): тень на `Off` задела бы пустые слоты.
  Вместо этого отдельный `Dim`.
- Поле `off_color` в ревью и в доке называлось и "пустой слот", и "тёмная фаза": док-комментарий поправлен, теперь
  `off_color` только для незаработанной звезды.

## 3. Flip-RED (`scratch/fixer_flips.py` -> `scratch/fixer_flips_result.txt`, файлы восстанавливаются побайтно, sha256 сверен)

| Саботаж | Гейт | RED | После восстановления |
|---|---|---|---|
| `star_look`: тёмная фаза снова `StarLook::Off` | `hud::stars::tests::star_look_table` | RED (stars.rs:147) | GREEN |
| `star_style`: `Dim` => стиль `Off` (off_color, без тени) | `earned_star_styles_differ_from_empty` | RED (stars.rs:161, "looks like an empty slot") | GREEN |
| `star_style`: `Dim` => стиль `Gray` (нет мигания) | `earned_star_styles_differ_from_empty` | RED (stars.rs:167) | GREEN |
| `validate_fight_hearing`: `min < radius` -> `min.is_finite()` | `fight_report_distance_below_fight_hearing` | RED (config.rs:540, `expect_err`) | GREEN |
| `compose_sim` передаёт `f32::INFINITY` вместо `fight_hearing_radius` (проверка по сути выключена) | `fight_report_distance_below_fight_hearing` | RED (config.rs:540) | GREEN |

Первый прогон флипов 4-5 падал паникой в `bevy_state` (у `App` не было `StatesPlugin`, `compose_sim` шёл дальше
конфигов): RED от обвязки, а не от кода. Исправил: фикстура строит `App` как `composed_app` (`MinimalPlugins`,
`TransformPlugin`, `AssetPlugin`, `StatesPlugin`), теперь RED на самом `expect_err`.

Класс гейтов: `star_look_table` и `earned_star_styles_differ_from_empty` проверяют корректность механизма (у тёмной
фазы свой вид), а не видимость на экране, её оценивает владелец по скриншотам. `fight_report_distance_below_fight_hearing` проверяет корректность
отказа (ошибка на правильном файле и поле).

## 4. Test results

| Команда | Итог |
|---|---|
| `cargo clippy --workspace --all-targets -j 4 -- -D warnings` | 0 предупреждений |
| `cargo test -p gta_sim -p citygen -j 4` | все бинари `0 failed`, config 46 (было 45, +1 фикстура) (`scratch/fixer_test_sim_citygen.txt`) |
| `cargo test -p gta_like --bin gta_like -j 4` x3 | 43 passed x3 (было 42, +1 тест стилей) (`scratch/fixer_test_client_x3.txt`) |
| `rustfmt --edition 2024 --check` на `src/hud/stars.rs`, `crates/gta_sim/src/lib.rs` (с детьми) | чисто; в `tests/config.rs` один дифф в чужой строке `assert_eq!(call_delay, 4.0625, ...)` (было до меня, не трогал) |
| `python scratch/star_backdrops.py --out scratch/stars_fixer` | exit 0, `summary.json`, 39 кадров + зумы |

Игровой процесс после скриншотов завершён (`Get-Process gta_like` пусто). `t10.py` не перезапускал: правки его не
затрагивают (HUD и валидация конфигов, числа в `.ron` sim не менялись).

## 5. Чеклист владельца

1. Открыть `scratch/stars_fixer/zoom_{wall,sky,street}_heat180_0.png` и `..._2.png`: можно ли сосчитать две
   заработанные звезды в обеих фазах на всех трёх фонах. Самый слабый кадр `zoom_street_heat180_2.png`.
2. В игре: без копов звёзды мигают серый <-> тёмно-серый, обе фазы с тенью; пустые слоты бледные без тени. Цвет
   тёмной фазы `assets/ui/strings.ron` -> `hud.stars.dim_color`.

## Изменённые файлы

`src/hud/stars.rs`, `src/menu/config.rs`, `src/menu/mod.rs`, `assets/ui/strings.ron`,
`crates/gta_sim/src/civilian/mod.rs`, `crates/gta_sim/src/lib.rs`, `crates/gta_sim/tests/config.rs`,
`maw/tasks/in_progress/TASK-024/log.jsonl` (2 decision), scratch: `fixer_flips.py`, `fixer_flips_result.txt`,
`fixer_phase_table.py`, `fixer_phase_table.txt`, `fixer_test_*.txt`, `stars_fixer/`, `stars_fixer.log`.

children: 0 launched / 0 reported.
