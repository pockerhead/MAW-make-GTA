# IMPL_SUMMARY — TASK-024 (розыск: полировка после QA T10)

Режим small-fix, спека = task.md. Pre-flight: все названные сущности на месте (`hud.stars` в `strings.ron`,
`qa_second_star_row_governs_search` в `done/TASK-011/scratch/qa/qa_wanted_probe.rs`, `search_step` с
`rows[usize::from(w.stars.max(1)) - 1]`, `fight_hearing_radius` 15 / `report_min_distance` 25, 14 пробелов в
`validate_call_delay`). `TextShadow { offset: Vec2, color: Color }` сверен с `bevy_ui-0.19.1/src/widget/text.rs:146`,
есть в `bevy::prelude`.

Цена ошибки: пункты 2 и 4 ломаются молча (строки 3..5 розыска, heat за удар) → гейты с flip-RED. Пункт 1 владелец
видит на первом кадре → скриншоты + числа, решает владелец. Пункт 3 это харнесс, 5 прогонов подряд.

## 1. Что сделано

| Файл | Строк (+/-) | Что |
|---|---|---|
| `assets/ui/strings.ron` | +4/-1 | `hud.stars`: `gray_color` 0.55 → 0.72, `off_color` чёрный 45 % → белый 25 % (пустой слот — светлый призрак), новые `shadow_offset: 2.0`, `shadow_color: (0,0,0,0.9)` |
| `src/menu/config.rs` | +11 | `StarsConfig.shadow_offset`, `shadow_color` + валидация (>= 0, компоненты в [0,1]) |
| `src/hud/stars.rs` | +14/-6 | у каждого слота `TextShadow`; тень только у заработанной звезды (Lit/Gray), у Off `Color::NONE` |
| `crates/gta_sim/tests/wanted_search.rs` | +103 | перенесён `second_star_row_governs_search` (QA, строка 2, реальная геометрия); новый табличный `higher_star_rows_govern_search` (строки 3..5) |
| `assets/npc/perception.ron` | +1/-1 | `fight_hearing_radius` 15 → 20 |
| `assets/npc/civilian.ron` | +2/-1 | новое `reaction.fight_report_min_distance: 15.0` |
| `crates/gta_sim/src/civilian/mod.rs` | +7/-1 | поле `fight_report_min_distance` + валидация |
| `crates/gta_sim/src/civilian/reaction.rs` | +5/-1 | Fight сообщаем с `fight_report_min_distance`, Gunshot по-прежнему с `report_min_distance`; 3 строки в `worked_reaction_table` |
| `crates/gta_sim/tests/wanted.rs` | +56/-19 | новый гейт `punch_seen_by_a_civilian_is_reported`; хелперы `punch_floor`/`land_punch`; фикстура `body_call_does_not_report_a_private_punch` переделана (см. раздел 2) |
| `crates/gta_sim/tests/wanted_support/mod.rs` | +6/-2 | `assert_shipped`: `fight_hearing_radius` 20, `fight_report_min_distance` 15 |
| `crates/gta_sim/src/wanted/mod.rs` | +2/-1 | сообщение `validate_call_delay` без 14 пробелов (`\`-перенос строки) |
| `crates/gta_sim/tests/config.rs` | +1 | `incident_memory_must_outlast_a_civilian_call`: в ошибке нет двойных пробелов |
| `tools/qa/scenarios/t10.py` | +77/-36 | план выстрела без мирных на линии огня (1 м), до 4 попыток с другой точки, `CALLER_RANGE_M` 25..38 → 27..36 |

### Пункт 1: звёзды
Проблема была в том, что никакой один цвет заливки не контрастирует и с белой стеной (~214), и с асфальтом (~90).
Поэтому заработанной звезде дана тёмная тень 2 px (контур читается на светлом), пустому слоту светлый полупрозрачный
призрак без тени (на светлом почти исчезает, на тёмном бледный). Серый 0.72, а не ярче: при 0.82 тело звезды на белой
стене сливалось со стеной, читалась одна тень (dead_end в логе, `scratch/stars_try_gray082/`).

Снимки: `scratch/star_backdrops.py` (release+dev, seed 1, точка t10 у больницы): стена (как у QA), небо (поворот
~78° влево и вверх до упора), асфальт (вниз до упора). На каждом фоне 1 голый кадр, 6 кадров с 5 звёздами (маска
глифов), 6 кадров с 2 звёздами. Зумы `zoom_*.png`.

- До: `scratch/stars_before/` (зумы `zoom_wall_heat180_0.png`, `zoom_sky_heat180_2.png`, `zoom_street_heat180_1.png`)
- После: `scratch/stars_after/` (те же имена, серая фаза в `*_heat180_0.png`)

Средний |Δ luma| глифа против голого фона (`scratch/star_table.py`, кадр с максимальным контрастом заработанных):

| Фон | До: заработанная / пустая | После: заработанная / пустая |
|---|---|---|
| белая стена | 67 / 46 (слот 2; слот 1 задет краем окна) | 54 / 7 |
| небо | 32 / 32 (в серой фазе заработанная +8 к небу, пустая −32) | 53 / 29 |
| асфальт | 49 / 43 | 79 / 45 |

Метрика грубая (маска включает тень, кадр выбирается по максимуму), окончательные числа за QA, вид за владельцем.

### Пункт 4: удар (решение, `log.jsonl`)
Выбран вариант "ниже порог для драк", а не проверка по взгляду: только данные + одна строка в `choose_reaction`.
`fight_hearing_radius` 20 (= `corpse_sight`), `fight_report_min_distance` 15 (= `panic_distance`, ближе мирный
бежит/прячется). Сообщаемое кольцо для драки 15..20 м. Выстрелы не тронуты (25..40 м). Вариант со зрением (луч на
каждый стимул драки до `corpse_sight`) даёт то же кольцо ценой лучей и кода в `perceive`.

## 2. Отклонения и что не сделано

- `body_call_does_not_report_a_private_punch`: старый свидетель стоял в ~18.9 м от удара и с новым радиусом слышит
  драку (удар стал бы не "приватным"). Теперь свидетель спавнится после того, как стимул драки ушёл из `StimulusLog`
  (проверка `GATE BROKEN: the fight is still audible`), жертва перед этим снова удержана в Idle. Суть гейта (звонок
  про труп не сообщает старый удар) та же, проверки расстояния 15..20 м до трупа остались.
- `t10.py`: кроме промаха нашёлся второй источник флейка. `CALLER_RANGE_M` начинался ровно с `report_min_distance`
  25 м, после телепорта (допуск 1 м) звонящий оказался в 24.99 м → `GATE BROKEN` (прогон с форсированным промахом).
  Диапазон сужен до 27..36 м (в 5 прогонах звонящий 26.6..30.7 м, в форс-прогоне до этого 39.8 при плане ≤ 38).
- Именованная мутация здоровья до 0 отвергнута: смерть не от игрока не создаёт инцидент Kill, heat < 40, гейт падает.
- Остальные QA-гейты из `qa_wanted_probe.rs` (`qa_two_callers_one_corpse` и т.д.) не переносились: задача просит
  только строку 2.
- Не добавлена кросс-проверка конфигов `fight_report_min_distance < fight_hearing_radius` (иначе удары снова никто не
  сообщит). Сейчас это держит только `assert_shipped` на поставленных числах. Кандидат на follow-up, в скоуп не лез.

## 3. Тесты

| Команда | Итог |
|---|---|
| `cargo clippy --workspace --all-targets -j 4 -- -D warnings` | 0 предупреждений |
| `cargo test -p gta_sim -p citygen -j 4` | все бинари 0 failed: lib 51, config 45, wanted 12, wanted_search 7, civilians 10, melee 19, shooting 16, gang_* ... (`scratch/test_sim_citygen.txt`) |
| `cargo test -p gta_like --bin gta_like -j 4` x3 | 42 passed x3 (`scratch/test_client_x3.txt`) |
| `cargo tree -p gta_sim -e normal -i bevy_render` | пусто |
| `python -m unittest tools/qa/test_brp.py`, `python tools/qa/tree_check.py` | ok |
| `python scratch/t10_runs.py 1 5` (t10.py x5 подряд) | 5/5 PASS, 0 промахов, звонок 4.06..4.17 с, heat 50, сброс 10.06..10.14 с, лог чистый (`scratch/t10_runs.txt`, `t10_run{1..5}/`) |
| `python scratch/t10_forced_miss.py` (первый прицел на 2.5 м выше) | PASS: 1 промах записан в `kill.misses`, вторая точка убила, звонок 4.2 с, heat 60, сброс 10.09 с (`scratch/t10_forced_miss/`) |

**Flip-RED** (`scratch/flips.py` → `scratch/flips_result.txt`, файл восстанавливается побайтно, sha256 сверен):

| Саботаж | Гейт | RED | После восстановления |
|---|---|---|---|
| `search_step`: `row = &rows[0]` (флип QA) | `second_star_row_governs_search`, `higher_star_rows_govern_search` | оба RED (строка 2: 168, строки 3..5: 219 "row 2") | GREEN |
| `search_step`: `clear_seconds` из `rows[0]`, радиус по строке | оба | RED (178, 228: "cleared before N ticks") | GREEN |
| `reaction.rs`: Fight снова с `report_min_distance` | `punch_seen_by_a_civilian_is_reported` | RED (свидетель не начал звонок) | GREEN |
| `perception`: драка слышна только в 15 м | `punch_seen_by_a_civilian_is_reported` | RED | GREEN |
| сообщение с 14 пробелами | `incident_memory_must_outlast_a_civilian_call` | RED (config.rs:702) | GREEN |

Классы: `*_row_governs_search` и `punch_seen_*` это корректность (точные тики, heat 5 и один reported Punch), не
liveness. Гейт звёзд headless не делался: вид проверяет владелец.

## 4. Как проверить руками (чеклист владельца)

`cargo run --release -- --seed 1`, подобрать пистолет.
1. Застрелить мирного в толпе, дождаться звезды. Встать так, чтобы под HUD было небо, потом белый дом, потом асфальт
   (камеру вниз). Сосчитать звёзды с одного взгляда: заработанная серая с тёмной тенью, пустые бледные без тени.
   Цвета в `assets/ui/strings.ron` → `hud.stars` (`gray_color`, `off_color`, `shadow_offset`, `shadow_color`).
2. Мигание: в тёмной фазе заработанная звезда выглядит как пустая. Не путается ли?
3. Ударить мирного кулаком, отойти так, чтобы рядом (15-20 м) стоял другой мирный: над ним может пойти полоса звонка,
   через ~4 с heat 5 (звезды нет, первая с 40). Мирные ближе 15 м от драки убегают, не звонят.
4. `python tools/qa/scenarios/t10.py --out <dir>` проходит; в `summary.json` поле `kill.misses` показывает промахи.

Игровой процесс после всех прогонов завершён (`Get-Process gta_like` пусто).

children: 0 launched / 0 reported.
