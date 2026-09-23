# QA_REPORT — TASK-002 (GDD T1), QA round 2

QA: claude opus, effort medium. Дерево `ea374d8` (код `ba35570`), ветка `feature/t01-skeleton-character-camera-qa`.
Round 1 (`QA_REPORT.prev-2.md`) дал NEEDS_FIXES с B1–B5. Этот отчёт проверяет фиксы B1–B3, всё, что фиксер не смог запустить (clippy, BRP-сценарий, release FPS), и причину B4.

## 0. Disconfirmation

Контрпример, который я искал первым: **новый `ledge_assist_max_height` управляет досягаемостью только в той геометрии, которую гоняет гейт** (прыжок строго в лоб, с одной дистанции разбега 1.2 м). Под подозрением было условие `wall_hit.normal.dot(*forward) > -0.5 → continue` в `crates/gta_sim/src/character/ledge.rs:87`. При подходе под углом больше 60° к нормали стены весь assist пропускается, включая барьер "слишком высоко", и решает опять натуральная пружина Tnua.

Проба: `scratch/qa/qa_r2_probe.rs` (временно лежала в `crates/gta_sim/tests/`, удалена). Композиция `compose_sim`, уступ 20×h×20 м с гранью на z = −2, разбег 5 м, прыжок держится 15 тиков.

| Случай | Результат |
|---|---|
| В лоб, shipped max 1.4: h 1.1/1.2/1.3/1.4, прыжок за 0.3/0.6/0.8/1.2/2.0 м | залез везде |
| В лоб, h 1.5 / 1.7 | не залез нигде |
| max 1.8: h 1.6 / 1.7, угол 0° и 45° | залез (ручка поднимает досягаемость) |
| max 1.2: h 1.3 / 1.35, угол 0° и 45° | не залез (ручка опускает досягаемость) |
| **max 1.1 или 1.2: h 1.25–1.3, угол 62°/70°** | **залез** (натуральный подъём Tnua, без snap) |
| shipped max 1.4: h 1.4, угол 62°/70° | не залез; h 1.2 при 62°/70° залез натурально |

**Контрпример подтвердился частично.** В лоб и до ~50° ручка работает в обе стороны. При скользящем подходе (≥ ~60° к нормали) досягаемость снова задаёт геометрия Tnua: с shipped-значениями это ~1.2–1.3 м вместо 1.4, а с уменьшенной ручкой уступ выше неё залезается. См. B6 (minor).

## 1. Environment

docker-compose и dev-сервера нет. Cargo прямо в checkout, плюс реальная оконная сборка через BRP. Все команды cargo по одной, с `--offline -j 4`, потому что TLS к crates.io здесь не работает. Сервисов не поднимал.

```
cargo build -j 4 --offline
cargo test -p gta_sim -j 4 --offline
cargo test -p citygen -j 4 --offline
cargo clippy -j 4 --offline -- -D warnings
cargo clippy --workspace --all-targets -j 4 --offline -- -D warnings
cargo clippy --workspace --all-targets --features dev,debug -j 4 --offline -- -D warnings
python tools/qa/tree_check.py
cargo tree --offline -p gta_sim -e normal -i bevy_render
CARGO_NET_OFFLINE=true python tools/qa/scenarios/t1.py --out maw/tasks/in_progress/TASK-002/scratch/qa/r2_runtime_t1
cargo build --release --features dev -j 4 --offline
python maw/tasks/in_progress/TASK-002/scratch/qa/r2_b4_probe.py debug   r2_b4_debug
python maw/tasks/in_progress/TASK-002/scratch/qa/r2_b4_probe.py release r2_b4_release
```

Чтобы повторить пробу: скопировать `scratch/qa/qa_r2_probe.rs` в `crates/gta_sim/tests/`, выполнить `cargo test -p gta_sim --offline --test qa_r2_probe -- --nocapture --test-threads=1`, потом удалить файл.
Все запущенные мной процессы игры (dev ×2, release ×1) завершены через `brp_extras/shutdown` с exit 0. `tasklist` не показывает `gta_like`. Два процесса `cargo` на хосте относятся к другому проекту (`the owner's previous project_*`), их я не трогал.

## 2. Test results

**Build:** `cargo build` PASS.

**Clippy `-D warnings`**, мои прогоны, все exit 0:
- root;
- `--workspace --all-targets` (включает новый `tests/ledge.rs`);
- `--workspace --all-targets --features dev,debug`.

**Suite `cargo test -p gta_sim`: 14/14 PASS**, против 12 в round 1. Новых падений нет. Прибавились `ledge::configured_reach_climbs_and_above_reach_blocks` и `ledge::pull_up_settles_quickly_without_burying_feet`.
- unit `camera_relative_axes`
- config: `shipped_locomotion_config_loads`, `unknown_field_names_file_and_field`
- jump: `jump_apex_matches_jump_height`, `tapped_jump_fires`, `late_tap_is_buffered_until_landing`
- ledge ×2 (новые)
- movement: yaw 0/90/180, `sprint_uses_sprint_speed`
- terrain: `walking_into_arena_box_stays_blocked`, `walking_climbs_stairs`

`cargo test -p citygen`: PASS, 0 тестов.

**Граница headless:** `tree_check.py` exit 0. `cargo tree -p gta_sim -e normal -i bevy_render` печатает "nothing to print".

**Flip-RED, мои, на новых гейтах.** Код закоммичен в `ba35570`. Каждый раз восстанавливал через `git checkout` и сверял sha256.

| Саботаж (production-код) | Результат | Восстановление |
|---|---|---|
| `ledge.rs`: `if rise > cfg.ledge_assist_max_height` → `if rise > 2.0` (барьер перестаёт читать RON) | `configured_reach_climbs_and_above_reach_blocks` RED на `ledge.rs:82`: уступ 1.7 м залезается | sha256 `4a37f897…2db7fc` совпал |
| `ledge.rs`: snap-цель `top_y + float_height` → `… − 0.25` (ноги в уступе) | `pull_up_settles_quickly_without_burying_feet` RED: "pull-up took Some(41) ticks" | sha256 `4a37f897…2db7fc` совпал |
| `character/mod.rs`: `buffer.remaining = 0.0;` → `+= 0.0` (repro B2 из round 1) | `tapped_jump_fires` RED: "ground tap rise=0.7281376" | sha256 `10bb8369…412218` совпал |

После всех восстановлений `git status` чистый.

**Runtime, dev** (`tools/qa/scenarios/t1.py` через `brp.py`): PASS. Сводка в `scratch/qa/r2_runtime_t1/summary.json`.
- W 1000 мс дал Δ = (0.0, −0.0002, −4.532) м.
- Мышь +200 дала Δyaw = −0.418879 рад.
- PNG записан, FPS 30.2, `window_state_at_fps` = `{focused: true, present_mode: Fifo}`, shutdown прошёл.
- Скриншот `scratch/qa/r2_runtime_t1/t1.png` я посмотрел. Синяя капсула стоит на сером полу и отбрасывает тень. Слева рампа с платформой, справа лестница. Вид повёрнут вправо после движения мыши. Кадр не чёрный, артефактов нет.

**Runtime, release + B4** (`scratch/qa/r2_b4_probe.py`). Прогрев 6 с, потом 6 сэмплов в Fifo. Затем `Window.present_mode` переключается в `AutoNoVsync` прямо в рантайме через BRP `world.mutate_components`, продакшен-код не менялся. Потом ещё 6 сэмплов.

| Сборка | Fifo (как поставляется) | AutoNoVsync |
|---|---|---|
| dev (`target/debug`) | 29.8–30.7 FPS | 300–490 FPS (2.0–3.3 мс) |
| release | кадр 32.8–34.1 мс, средняя 33.4 мс (30 FPS) | кадр 2.23–2.61 мс, средняя 2.51 мс (~400 FPS) |

- winit видит **один монитор: `\\.\DISPLAY9`, 1920×1080, 30000 mHz (30 Гц), PrimaryMonitor**. Окно стоит в позиции `Automatic`, 1280×720, в фокусе.
- WMI показывает на хосте "Virtual Display Driver" 1920×1080 @ 30 Гц, "USB Mobile Monitor Virtual Display" и RTX 4070 Ti @ 144 Гц.
- Данные: `scratch/qa/r2_b4_release.json`, `scratch/qa/r2_b4_debug.json`.
- Скриншот `scratch/qa/r2_b4_release.png` (release, снят после переключения в no-vsync) я посмотрел: вид со спавна, капсула с тенью, рампа слева, лестница справа, кадр чистый.

**Гипотеза оркестратора подтверждена.** 30 FPS дают vsync (Fifo) и 30-герцовый виртуальный основной дисплей. Фокус окна ни при чём, он `true`. Без vsync release-кадр занимает ~2.5 мс. B4 не дефект игры.

**Характер snap'а** (та же проба, `max_tick_step`). Переход на уступ — это телепорт за один фиксированный тик: **1.2–1.5 м за 15.6 мс**, примерно через 6–16 тиков после отрыва. Snap срабатывает, как только центр капсулы поднялся выше верха уступа. Для уступа 1.4 м это центр 1.4 м, то есть +0.35 м от стойки, намного раньше апекса. Капсула рендерится с `TransformInterpolation`, pivot камеры сглажен (`follow_half_life 0.05`). Визуально это рывок примерно в один кадр, а не подтягивание. См. B7, это оценивает владелец.

**Препятствие у точки приземления.** Snap не проверяет свободное место. Два случая:
- тонкий уступ 0.4 м с 4-метровой стеной сразу за ним;
- уступ под потолочной плитой (просвет 1.4 м при капсуле 1.5 м).

В обоих Avian выталкивает капсулу в допустимую позицию: на уступ z −2.10, либо к кромке плиты z −2.70. Застревания и провала сквозь стену нет, пиковая скорость до 10.9 м/с на один тик. Сейчас это не дефект, но риск для T2, когда появится городская геометрия.

## 3. Проверка фиксов round 1

| Находка | Что заявил фиксер | Моя проверка | Итог |
|---|---|---|---|
| B1: досягаемость не управляется RON | свой ledge assist в sim, `ledge_assist_max_height` + probe/clearance/window в RON, барьер "слишком высоко", гейт | код `ledge.rs` прочитан. Гейт проверяет 1.4 залезает / 1.7 нет / max=1.1 не пускает на 1.4. Мой flip-RED красный. Свипы: в лоб и до 45–50° ручка работает вверх (1.8 → 1.6/1.7 залезает) и вниз (1.2 → 1.3 нет) | **FIXED** для подхода ≤ ~50°. Остаток при скользящем подходе: B6 |
| B2: у тапа нет гейта | `tapped_jump_fires` требует рост 0.15–0.35 м | repro из round 1 теперь RED (0.728 м) | **FIXED** |
| B3: медленный подъём, ноги в уступе | snap на верх уступа, гейт ≤ 19 тиков осадки и ≤ 6 тиков "закопан" | flip-RED красный (41 тик). Медленного подъёма больше нет, есть телепорт (B7) | **FIXED** по букве. Как это ощущается, решает владелец |
| B4: 30 FPS | в прогоне записываются focus и present mode | измерено выше: vsync плюс 30-герцовый основной дисплей | **Закрыт как не дефект игры** |
| B5: `__pycache__` | оркестратор добавил в `.gitignore` | `.gitignore` содержит `__pycache__/` и `*.pyc`. После моего прогона `brp.py` `git status` чистый | **FIXED** |

Текст "Fixed" в FIX_SUMMARY совпадает с кодом. Прежние `ledge_assist_cling_distance/spring_*` честно переименованы в `ground_sensor_cling_distance` / `ground_spring_*`. Все четыре новых поля `ledge_assist_*` используются в `ledge.rs`, лишних ручек нет.

## 4. Acceptance criteria

| Criterion | Test performed | Result |
|---|---|---|
| yaw 0, 64 тика → −Z в [0.8, 1.0]·v_run, \|x\| < 0.1 | `forward_yaw_0_moves_neg_z` через `compose_sim` | PASS |
| yaw 90° → −X | `forward_yaw_90_moves_neg_x` | PASS |
| yaw 180° → +Z | `forward_yaw_180_moves_pos_z` | PASS |
| Апекс прыжка ±10% от `jump_height` | `jump_apex_matches_jump_height` | PASS |
| Неизвестное поле RON → ошибка с файлом и полем | `unknown_field_names_file_and_field` | PASS |
| `gta_sim` без `bevy_render` | `cargo tree … -i bevy_render` пуст, `tree_check.py` exit 0 | PASS |
| Runtime QA: `t1.py` через `brp.py` (запуск, W, yaw, скриншот, FPS, shutdown) | запустил сам, PNG посмотрел | PASS |
| Owner checklist в QA_REPORT | раздел 7 | PASS (записан) |
| Тюнинг в RON, не в `const` | новые значения ledge assist лежат в `locomotion.ron` с `deny_unknown_fields`. `SENSOR_INSET` остаётся законом формы сенсора | PASS |
| build, clippy `-D warnings`, `test -p gta_sim`/`-p citygen` зелёные | мои прогоны (раздел 2) | PASS |
| Существующие тесты проходят | 14/14. 12 тестов round 1 все на месте и зелёные | PASS |
| Fast compiles: профили, фича `fast`, `rust-lld` | `Cargo.toml` и `.cargo/config.toml` на месте. Проверено в round 1, в этом раунде не менялось | PASS |
| Fast compiles: evidence в FIX_SUMMARY | `FIX_SUMMARY.prev-1.md`: 34.51 s → 9.96 s, старт `fast` и `dev`. Инкрементальная пересборка в этом раунде 22–39 s после правок sim | PASS (по round 1) |
| Ledge: механизм назван с file:line | `FIX_SUMMARY.prev-1.md` (Tnua `walk.rs:431-437, 516-520`) плюс свой механизм в `ledge.rs` | PASS |
| Ledge: подъём быстрый и чистый, досягаемость задают значения в RON | гейты `tests/ledge.rs` плюс мои flip-RED и свипы | PASS для подхода ≤ ~50°. Скользящий подход: B6. Как ощущается телепорт: B7, решает владелец |
| Ledge: ходьба не залезает на коробки, лестница проходится | `terrain.rs` ×2 | PASS |
| Ledge: гейты прыжка и движения зелёные | suite | PASS |

## 5. Bugs found

**B6 — minor — при скользящем подходе ledge assist пропускается, досягаемость снова задаёт геометрия Tnua.**
- Где: `crates/gta_sim/src/character/ledge.rs:87`, `if wall_hit.entity != top_hit.entity || wall_hit.normal.dot(*forward) > -0.5 { continue; }`. Условие пропускает и snap, и барьер "слишком высоко" (`ledge.rs:90`).
- Repro: `scratch/qa/qa_r2_probe.rs::qa_knob_range` / `qa_oblique`. `ledge_assist_max_height = 1.2`, уступ 1.3 м, подход 62°, прыжок за 1.2 м до грани: `on_top=true`, end y 2.33. При 0° и 45° тот же уступ не залезается.
- Ожидалось: "reach governed by named values in `locomotion.ron`, not by an accident of geometry" при любом подходе.
- Фактически: при ≥ ~60° к нормали работает натуральная пружина Tnua. С shipped 1.4 это даёт ~1.2–1.3 м (1.4 при 62°/70° не залезается, 1.2 залезается медленно, без snap). Если владелец уменьшит ручку, уступ выше неё при диагональном подходе всё равно залезается.
- Почему minor: с поставляемыми значениями утечки вверх нет. Скользящий прыжок под 60°+ — редкий случай. Гейт его не покрывает.
- Направление фикса, его надо пересчитать: барьер "слишком высоко" не должен зависеть от угла подхода. Проверять нормаль стены только для snap, а для барьера брать компоненту вдоль нормали. Добавить гейт на подход под 60–70° с уменьшенной ручкой.

**B7 — minor / решает владелец — подтягивание на уступ — это телепорт за один тик на 1.2–1.5 м.**
- Repro: `qa_runup_sweep`, колонка `max_tick_step`. Пример: h 1.4, прыжок за 1.2 м дал 1.501 м за тик 64, через ~6 тиков после отрыва, когда центр капсулы всего на 0.35 м выше стойки.
- Гейт `pull_up_settles_quickly_without_burying_feet` считает тики после пересечения грани. Телепорт он пропускает, как и должен, но плавное подтягивание от рывка не отличает.
- Требование владельца: "quick and clean (no slow crawl)". Быстро — да. Чисто ли выглядит, владелец увидит в первом же прыжке. Если рывок мешает, нужно ограничить перемещение за тик (довести до цели за N тиков) и гейтнуть `max_tick_step`.

**B4 — info, закрыт — 30 FPS дают vsync и 30-герцовый основной дисплей хоста**, это не дефект игры. Доказательства в разделе 2. На 144-герцовом мониторе владельца Fifo даст до 144 FPS. Release без vsync рисует кадр за ~2.5 мс.

**Наблюдение, не баг:** snap ставит капсулу, не проверяя свободное место. В тестовых случаях Avian корректно выталкивает капсулу, но для T2 (город, навесы, узкие карнизы) стоит добавить shape-cast по точке цели.

## 6. Verdict

**SHIP-PENDING-RUNTIME.**
- Сборка, clippy во всех трёх формах, 14 headless-гейтов, граница headless, BRP-сценарий и release-замер я прогнал сам. Скриншоты посмотрел.
- B1–B3 исправлены в коде. Гейты B1/B3/B2 я сам перевёл в RED саботажем production-кода и вернул в GREEN, sha256 совпал.
- B4 измерен: vsync на 30-герцовом виртуальном основном дисплее, не игра.
- B6 — остаток B1 в узком случае (скользящий подход, пониженная ручка). С поставляемыми значениями он не проявляется, поэтому ставлю minor и не блокирую.
- B7 и feel в целом решает владелец.
- Если оркестратор считает формулировку "not by an accident of geometry" буквальной для всех углов подхода, B6 переводит вердикт в NEEDS_FIXES. Фикс маленький (раздел 5).

## 7. Owner checklist (субъективное, не гейтится)

1. `cargo run` (для итераций `cargo run --features fast`). Открывается окно "GTA-like", капсула стоит на полу.
2. W/A/S/D двигают относительно камеры, Shift — спринт (6.8 м/с), Alt — шаг (1.8 м/с). Space даёт прыжок ~1 м, короткий тап — маленький подскок ~0.2 м.
3. Рампа 30° на x = −10 и лестница на x = +10 проходятся плавно, на ступенях 0.2 м нет рывков.
4. Коробки у (10..16, z = 10): в 1-метровую коробку ходьба упирается, на неё можно запрыгнуть. Уступы 1.1–1.4 м берутся прыжком с разбега. **Посмотрите, как выглядит заход на уступ: сейчас это рывок ~1.5 м за один кадр (B7).** Устраивает или нужно плавнее? Коробка 2 м и блок 1.6 м (рядом с лестницей, с пола) не берутся.
5. Прыгните на уступ 1.2–1.4 м по диагонали, под большим углом к стене. Сейчас такой прыжок либо не залезает, либо лезет медленно (B6).
6. Камера: мышь вправо поворачивает вид вправо, мышь вниз опускает взгляд. У стены z = 14 камера не проходит сквозь стену и плавно отъезжает обратно.
7. Esc освобождает курсор, ЛКМ снова захватывает.
8. FPS на своём мониторе: с vsync он должен совпасть с частотой монитора (144 Гц). 30 FPS здесь давал 30-герцовый виртуальный дисплей хоста.
9. Тюнинг: `assets/character/locomotion.ron` (включая `ledge_assist_*`), `assets/camera/camera.ron`, `assets/world/render.ron`.

children: 0 launched / 0 reported.
