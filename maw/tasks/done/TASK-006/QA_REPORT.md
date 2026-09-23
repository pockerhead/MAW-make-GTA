# QA_REPORT — TASK-006 (GDD T5: здоровье, урон, смерть, возрождение)

QA: claude/opus, effort medium. HEAD `76152fe` (фикс `3b689b0`), рабочее дерево чистое. Base для диффа `c730212`.

## Disconfirmation (сначала)

Контрпример, который сломал бы фичу: "второй цикл смерти ведёт себя не так, как первый" (OnTransition, `Dead`, `WastedClock`, `Time<Virtual>` накапливают состояние: второй Wasted длиннее/короче, скорость не 1.0, второй игрок или второй город). Плюс побочный: "урон, записанный во время Wasted, бьёт возрождённого игрока".

Результат: первый НЕ подтвердился — две смерти подряд дают 288 и 288 апдейтов в Wasted, та же сущность, розыск 0, скорость 1.0, число зданий и пикапов не меняется (`qa_two_deaths_in_a_row`). Второй подтвердился частично — см. баг B1 (низкий, заявлен автором как known limitation).

## 1. Environment

- docker-compose нет; `cargo` напрямую в `D:/test-gta-like` (in-place, без worktree).
- Headless: `cargo build`, `cargo clippy`, `cargo test --workspace`.
- Runtime: `python tools/qa/scenarios/t5.py --out maw/tasks/in_progress/TASK-006/scratch/qa/t5` (release + `--features dev`, BRP на 127.0.0.1:15702, драйвер `tools/qa/brp.py`). Игра завершена через `brp_extras/shutdown`, `tasklist` — процесса `gta_like` не осталось.
- Сервисов/контейнеров не запускалось.
- Мои пробы: `scratch/qa/qa_probe.rs` (временно клался в `crates/gta_sim/tests/qa_probe.rs`, запускался `cargo test -p gta_sim --test qa_probe`, потом убран из дерева).

## 2. Test results

Существующие:
- `cargo build` — OK.
- `cargo clippy --workspace --all-targets -- -D warnings` — чисто; `cargo clippy --features dev,debug -- -D warnings` — чисто.
- `cargo test --workspace` — 0 failed во всех наборах: citygen unit 9, golden 3 (+1 ign), perf 0 (+1 ign), properties 13; gta_like bin 16; gta_sim lib 4, anim_state 4, asset_manifest 3, city 6 (+1 ign), config 9, health 6, jump 3, movement 4, respawn 3, terrain 2. Ноль падений на HEAD, значит новых падений относительно base нет.
- `cargo tree -p gta_sim -e normal -i bevy_render` — "nothing to print".
- `python tools/fetch_assets.py --check` — OK.

Мои пробы (`scratch/qa/qa_probe.rs`):
| Тест | Результат |
|---|---|
| `qa_two_deaths_in_a_row` — две смерти, между ними уход от больницы и розыск 5 | PASS (288/288 апдейтов, та же сущность, розыск 0, скорость 1.0, счётчики мира стабильны) |
| `qa_respawn_after_falling_death_kills_velocity` — смерть в полёте на высоте 40 м со скоростью (20,-30,0) | PASS (стоит у больницы, не дрейфует) |
| `qa_regen_above_cap_is_not_reduced...` — 55 HP не срезается до 50 за 20 с; броня 10 ровно гасит 10 урона | PASS |
| `qa_damage_during_wasted_does_not_hit_respawned_player` — `DebugDamage(30)` каждый апдейт весь Wasted | FAIL: после возрождения 70/100 (утёк один удар, записанный в последнем апдейте Wasted). Баг B1 |

Мои flip-RED (sha256 до/после совпал, восстановление через `git checkout`):
| Сломал | Гейт | Итог |
|---|---|---|
| `reset_wanted` не обнуляет `stars` (`wanted/mod.rs`) | `respawn::death_wasted_respawn_at_hospital` | RED (`left: 3, right: 0`, respawn.rs:101), восстановлено, sha `24ec2528...` |
| `advance_wasted` на `Time<Virtual>` вместо `Time<Real>` | тот же | RED ("Screen first at k=318"), восстановлено, sha `d41814b7...` |

Runtime t5.py — PASSED (`scratch/qa/t5/summary.json`): hash = golden seed 1, 100 чанков до и после, HospitalSpawn (-591.5, 0.15, 537.84), 40 урона → (60,0), пикап брони → (60,50), 30 урона → (60,20), летальный → Wasted, скриншот в фазе Screen при state=Wasted, Wasted длился 4.52 с реального времени (ожидание 4.5), та же сущность, 0.0 м от точки больницы, (100,0), ошибок шрифтов/ассетов в логе нет.

Скриншоты (смотрел сам):
- `scratch/qa/t5/hud_damaged.png` — красная полоса сверху справа ~60%, полоса брони пустая; город нормальный.
- `scratch/qa/t5/wasted.png` — кадр полностью обесцвечен, по центру красное "ПОТРАЧЕНО" крупной кириллицей (Inter Display Black), не обрезано; полосы HUD пустые.
- `scratch/qa/t5/respawned.png` — цвет вернулся, игрок на тротуаре у белого здания (больница), здоровье полное, броня 0.

## 3. Acceptance criteria

| Критерий | Проверка | Результат |
|---|---|---|
| Броня поглощает до нуля, потом здоровье | `health::armor_absorbs_then_health` через `compose_sim`; unit `damage_table`; моя проба точного истощения брони; runtime (60,50)→(60,20) | PASS |
| Регенерация не превышает 50% | `health::regen_waits_then_stops_at_cap` (вкл. off-grid 49.5); моя проба 55 не срезается | PASS |
| Смерть → Wasted → по `Time<Real>` → Playing у больницы, оружие сохранено, розыск 0 | `respawn::death_wasted_respawn_at_hospital`; мои flip-RED (wanted, Time<Real>) дают RED; моя проба двух смертей | PASS. "Оружие" — пока заглушка `Loadout` на той же сущности (оружия нет до T6), сохранение сущности проверено |
| Slow-mo восстанавливает `Time<Virtual>` 1.0 при выходе из Wasted | `death_wasted_respawn_at_hospital` (скорость 0.3 в SlowMo, 1.0 в Screen и после), `wasted_abort_from_slowmo_restores_time` | PASS |
| Runtime t5.py через brp.py | прогнан мной, PASSED, скриншоты осмотрены | PASS |
| Owner checklist в QA_REPORT | ниже | записан, ждёт владельца |
| Тюнинг в data-файлах GDD §12 | `health.ron`, `respawn.ron`, `strings.ron`, `render.ron`; новых `const` для тюнинга в диффе нет (только пути к конфигам) | PASS |
| build / clippy / test green | выше | PASS |
| Existing tests pass | выше | PASS |
| Хазард TASK-004: город не спавнится повторно | `respawn_keeps_world_one_shot` (sim), `city_is_built_once_across_respawn` (client), runtime 100→100 чанков, моя проба двух смертей | PASS |

Фиксы ревью проверены по коду: `t5.py::screenshot` удаляет цель до запроса и ждёт PNG-сигнатуру (строки 65-76); `sidewalk_anchor` собирает полигон через `.get()` (graphs.rs ~180), тест `sidewalk_anchor_rejects_bad_indices` есть и зелёный. Бинарных ассетов в git нет (`git ls-files` без ttf/otf/zip).

## 4. Bugs found

**B1 — Low.** `DebugDamage`, записанный в последнем апдейте `Wasted` (кадр, где происходит переход в Playing), применяется к уже возрождённому игроку.
- Репро: `scratch/qa/qa_probe.rs::qa_damage_during_wasted_does_not_hit_respawned_player` — писать `DebugDamage(30)` каждый апдейт во время Wasted, после возврата в Playing и 8 тиков.
- Ожидалось: 100/0. Фактически: 70/0 (утёк ровно один удар; более ранние сообщения теряются при ротации буфера).
- Влияние: только F5/BRP (debug). Автор заявил это как known limitation в IMPL_SUMMARY. Когда в T6 урон пойдёт другим путём, стоит проверить, что источник урона сбрасывается/игнорируется на границе респавна (например, `MessageReader::clear` в `respawn_player` или отбрасывание сообщений в первом тике Playing). Не блокирует T5.

Наблюдения (не баги):
- Броня обнуляется при смерти (`Health::full`), GDD §3.4 это не запрещает (сохраняются "оружие и патроны").
- `tools/qa/scenarios/t3.py:26-32` имеет тот же дефект stale-screenshot (заметил фиксер, вне скоупа T5) — стоит отдельной задачей.
- FPS не мерил: в критериях T5 нет требований к производительности.

## 5. Owner checklist (субъективное, тестом не закрывается)

1. `python tools/fetch_assets.py` (в старом checkout обязательно — без пака `inter` preflight не стартует).
2. `cargo run --features fast,debug`. Экран загрузки: "Генерация города (seed N)" кириллицей.
3. F5 несколько раз: красная полоса справа сверху уменьшается на 25 за нажатие.
4. Дойти по тротуару к больнице (белое здание): кубы аптечки и брони по 6 м в обе стороны от точки возрождения. Броня заполняет синюю полосу, следующий F5 ест сначала броню.
5. Опуститься ниже 50% и подождать 5 с: здоровье растёт до половины и останавливается.
6. Умереть: 1.5 с замедления и серый кадр, затем "ПОТРАЧЕНО" 3 с. Читается ли замедление, не резко ли, размер/цвет надписи.
7. Возрождение у больницы с полным здоровьем, камера без пролёта, город не мигает и не удваивается. Анимации смерти нет (Q2, ожидаемо).

## 6. Verdict

**SHIP-PENDING-RUNTIME.** Все headless-критерии и runtime-сценарий t5.py PASS, сборка и clippy чистые, ноль падений тестов, два моих flip-RED дали RED. Осталась только owner-проверка ощущений (slow-mo, читаемость экрана смерти, вид полос и кубов) — чеклист выше. B1 низкий, касается только debug-пути, в T5 заявлен автором.

children: 0 launched / 0 reported.
