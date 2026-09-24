# IMPL_SUMMARY — TASK-011 (GDD T10, wanted level core)

Цена ошибки: смешанная, в основном тихая (двойной учёт трупа, heat без свидетеля, вечный/мгновенный таймер,
утечка через `Wasted`) — на это стоят headless-гейты с flip-RED; вид звёзд и мигания — прогон владельца.

Pre-flight: план сверен с кодом (файлы, строки, сигнатуры, API Bevy 0.19.1 / avian 0.7 / ron 0.12.2). Блокеров нет.
`Entity::from_raw_u32` в 0.19.1 возвращает `Option<Entity>` (`bevy_ecs-0.19.1/src/entity/mod.rs:552`).

## 1. Что сделано (файлы, строки)

Новые:
- `assets/wanted/wanted.ron` (18) — heat-таблица, 5 строк звёзд (tuple-синтаксис), радиусы, память, конус копа.
- `crates/gta_sim/src/wanted/mod.rs` (231, переписан) — `WantedConfig` + `validate`, `HeatTable`, `StarRow`,
  `STARS = 5` (закон), `WantedLevel { heat, stars, last_known, seen, hidden }`, `stars_for`, `WantedSystems`,
  `WantedPlugin` (после `AiSystems::Decide`, в `PlayingSystems`), `reset_wanted` (OnEnter Wasted: сброс + `crimes.clear()`),
  `drop_queued_calls` (OnExit Wasted), unit `stars_table`.
- `crates/gta_sim/src/wanted/crimes.rs` (407) — `Crime`, `Incident`, `Crimes` (`record/report/resolve/forget`),
  `classify`, системы `record_crimes`, `take_calls`, `forget_crimes`; 7 unit-тестов плана.
- `crates/gta_sim/src/wanted/search.rs` (243) — `eye`, `in_view`, `cop_sees`, `witnesses`, `search_step`,
  `track_search` (`set_if_neq`); unit `view_cone_table`, `search_step_table`.
- `crates/gta_sim/tests/wanted.rs` (417) — A1..A10 (+ контроль `lone_shot_is_no_crime`).
- `crates/gta_sim/tests/wanted_search.rs` (215) — B8..B11 (+ LOS-гейт `walls_block_cop_sight`).
- `crates/gta_sim/tests/wanted_support/mod.rs` (316) — общие фикстуры двух файлов (graph_app, Probe, spawn_cop, setup_w, assert_shipped).
- `src/hud/stars.rs` (131) — ряд из 5 `★` под патронами, `star_look` + `update_stars`, unit `star_look_table`.
- `tools/qa/scenarios/t10.py` (295) — runtime BRP-сценарий.

Изменённые (git diff --stat): `perception/mod.rs` (+48/-: `Cause`, `Threat.cause`, `StimulusLog` 4-tuple),
`civilian/mod.rs` (+43: `Report.about`, `PoliceCall`, запись звонка только на Report→Wander), `civilian/reaction.rs` (+1),
`combat/hitscan.rs` (+3, `ShotFired.attack`), `combat/melee.rs` (+3, `MeleeHit.attack`), `lib.rs` (+8, загрузка `WantedConfig`),
`tests/config.rs` (+67, 4 гейта), `tests/civilians.rs` (28, `call_progress`), `tests/respawn.rs` (8, heat вместо stars),
`tests/civilian_bench.rs` (+1), `src/menu/config.rs` (+41, `StarsConfig` + валидация), `assets/ui/strings.ron` (+блок `stars`),
`src/hud/mod.rs` (7), `src/hud/witness.rs` (4), `src/hud/witness_gate.rs` (17), `src/visuals/character_gate.rs` (+1).

Весь тюнинг в `wanted.ron` / `strings.ron`; новый `const` только `STARS` (закон GDD) и `WANTED_CONFIG` (путь).
`cargo tree -p gta_sim -e normal -i bevy_render` пуст.

## 2. Отклонения от плана

- **Cop fixture несёт `Transform`** (`tests/wanted_support/mod.rs::spawn_cop`). План: "avian's `Position` has no required
  components". Неверно: avian3d 0.7 регистрирует `Position -> Transform` (`physics_transform/mod.rs:81`), и
  `transform_to_position` переписывал `Position` фикстуры в (0,0,0). Первый прогон A7 прошёл вакуумно (коп в точке игрока);
  найдено отладочной печатью, фикстура исправлена, все cop-гейты перепроверены с флипами. Предложение в `PCTX_PROPOSALS.md`.
- **A4 свидетель на `SIDES[1]` t = 0.8**, не 0.9: план пишет t=0.9 → (10,0,6), но t=0.9 даёт (10,0,8). Геометрия плана
  (W2–V 18.87 м) соответствует t=0.8; дистанции ассертятся в тесте (GATE BROKEN).
- **A4: жертву убивают сразу после удара.** Удар (Hurt) переводит спокойную жертву в Flee, `hold_idle` не держит. Мутация
  `current = 0` делается в следующий тик после `MeleeHit`, дистанция до трупа меряется после смерти и ассертится в (15, 20].
- **A10: мутация `Loadout.held = Some(Pistol)`** у бандита: idle-бандит держит оружие в кобуре (`held: None`), без этого
  выстрела нет. Названа в тесте.
- **Проверка свидетельства копом** в `record_crimes` делается один раз за тик по глазам игрока (все инциденты — игрока),
  затем репортятся все тронутые инциденты. Эквивалентно per-incident проверке плана, меньше лучей (log.jsonl decision).
- `search_step` принимает `&[StarRow; STARS]`, а не весь конфиг (чистая функция, тесту не нужен весь `WantedConfig`).
- B8 `last_known` сравнивается с допуском 1e-3 м: физика шагает после wanted-систем, тело "плавает" на микрометры.
- Добавлен отдельный LOS-гейт `walls_block_cop_sight` (в плане это часть B10).
- `t10.py`: сначала телепорт к жертве, потом мутации. Второй прогон упал: жертва была recycled популяцией между
  запросом и BRP-мутацией, а `world.mutate_components` на удалённой сущности **паникует игру** (`bevy_remote-0.19.1/src/builtin_methods.rs:1194`).
  После телепорта жертва и звонящий в пределах `recycle_distance` 50 м, скрипт перепроверяет их наличие перед мутацией.

## 3. Тесты

| Команда | Результат |
|---|---|
| `cargo test -p gta_sim -j 4` | все зелёные: lib 51, wanted 11, wanted_search 5, config 44, civilians 10, respawn 5, gang_* / shooting / melee и остальные — 0 failed |
| `cargo clippy -p gta_sim --all-targets -j 4 -- -D warnings` | чисто |
| `cargo clippy --all-targets -j 4 -- -D warnings` | чисто (после замены `% 2 == 0` на `is_multiple_of`) |
| `cargo build -j 4`, `cargo build -p gta_like --features dev -j 4` | ок |
| `cargo test -p gta_like --bin gta_like -j 4` ×3 | 42 passed ×3 (`star_look_table`, `witness_gate` ×2) |
| `python tools/qa/tree_check.py`, `python tools/qa/test_brp.py` | ок |
| `python tools/qa/scenarios/t10.py --out scratch/t10_run{1,2,3}` | run1 PASS; run2 упал (recycle race, см. §2), исправлено; run2 PASS, run3 PASS |

`citygen` не тронут.

### Flip-RED (скрипт `scratch/flips.py`, лог `scratch/flips_result.txt`; файл восстанавливается побайтово)

| Флип (что сломано) | Гейт | RED |
|---|---|---|
| `witness`: инциденты репортятся без копа | A1 `unwitnessed_kill_is_zero_heat` | Shooting/Kill reported=true |
| `report_once`: `report` игнорирует `reported` | A2 (80 вместо 40), A3 (90 вместо 50) | да |
| `body_all`: `Body(v)` резолвит все инциденты жертвы | A4 | heat 5 |
| `call_on_entry`: `PoliceCall` при входе в Report | A5 `killing_the_caller_interrupts_the_call` | звонок записан |
| `near_alive_only`: "рядом" только живые | A7 | 40 вместо 50 |
| `no_collapse`: без схлопывания дробовика | A8 | Wound записан |
| `gang_wound`: ранение бандита = Wound | A9 | да |
| `any_shooter`: DamageDealt бандита как преступление игрока | A10 | heat > 0 |
| `no_clear`: без `crimes.clear()` | B11 | heat 40 |
| `hidden_inside`: таймер идёт внутри круга | B9 (контроль) | да |
| `no_reentry_reset`: без сброса при возврате в круг | B9 (вариант) | да |
| `seen_ignored`: замеченный игрок не сбрасывает таймер | B10 | да |
| `cop_xray`: `cop_sees` без LOS | `walls_block_cop_sight` | да |
| `no_prune`: без `attacks.retain` | unit `attack_list_is_bounded` | 101 вместо 61 |
| `thresholds_unordered`: без проверки возрастания | config `star_thresholds_must_increase` | да |
| (client) `star_look` без ветки `seen → Lit` | `star_look_table` | да |

Config-гейты флипаются своим входом (саботаж — это и есть флип): unknown field → ошибка с `wanted.ron` и `bogus`;
`(heat: 30,` → `stars[1].heat`; удалённая 5-я строка → parse-ошибка `length 5` (разные ошибки у двух саботажей).

Классы гейтов: корректность — A2, A3, A4, A7, A8, B9, B10, B11, LOS, config, unit; "нет свидетеля / прерван / не
преступление" — A1, A5, A6 + контроль; правила банд — A9, A10; liveness — B8.

## 4. Как проверить вручную

1. `cargo run --features fast` (или release), подобрать пистолет, выстрелить рядом с мирными.
2. Над звонящим мирным появляется полоса звонка, через ~4 с под счётчиком патронов загорается звезда; копов ещё нет
   (T11), поэтому звезда мигает серым.
3. Убежать > 40 м от места преступления: через ~10 с при 1 звезде розыск пропадает. Убийство звонящего до конца полосы —
   звёзд нет. Выстрел в воздух без людей рядом — звёзд нет. Одиночный выстрел рядом с человеком (10 heat) звёзд не даёт и
   сгорает по правилу 1-й звезды.
4. BRP: `world.mutate_resources` `WantedLevel` `.heat = 180` → 2 звезды.

### Чеклист владельца (для QA_REPORT.md)

- стреляет при свидетелях → над свидетелем полоса звонка → звёзды (мигают, копов ещё нет);
- убегает за круг → через 10 с при 1 звезде розыск пропадает;
- убийство свидетеля до конца звонка → звёзд нет;
- стрельба в воздух без людей рядом → звёзд нет;
- одиночный выстрел рядом с человеком (10 heat) звёзд не даёт и сгорает по правилу 1-й звезды.

Замечание для владельца (вид, не гейт): на светлом небе серая "горящая" звезда и полупрозрачные чёрные "пустые" слабо
контрастируют (`scratch/t10_run1/hud_wanted.png`, `hud_blinking_2.png`); цвета в `assets/ui/strings.ron` `hud.stars`.
Повторные звонки об одном трупе видны полосой (heat считается один раз, решение Q3).

children: 0 launched / 0 reported.
