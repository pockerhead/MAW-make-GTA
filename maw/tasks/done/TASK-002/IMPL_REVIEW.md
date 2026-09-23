# IMPL_REVIEW — TASK-002 (GDD T1)

Ревьюер: code-reviewer (claude opus, effort high). Проверено по дереву на `074a3ce` (код в `b60dbb7`), рабочее дерево чистое. TASK_FINAL.md перечитан после коммитов `0757427` и `35f55b1`: в нём появились два дополнения владельца для фиксера.

## 1. Verdict

**NEEDS_WORK.** Срез T1 сделан аккуратно, все гейты зелёные и честно проходят flip-RED. Но в TASK_FINAL остаются два открытых обязательных дополнения владельца (fast compiles; step-up на уступы). Кроме того, есть подтверждённый пробой major-дефект: короткий тап прыжка незадолго до приземления теряется, то есть буфер 0.1 с из GDD работает только пока клавиша зажата.

## 0. Disconfirmation (сделано до оценки)

Контрпример, которого я боялся больше всего: headless-гейты гоняют только `move_direction` в sim. Если у клиента соглашение yaw/pitch камеры или маппинг WASD в BEI не совпадает с `move_direction`, то в игре W при ненулевом yaw бежит не туда, куда смотрит камера, а все гейты остаются зелёными.

Что проверил:
- Камера `rotation = Quat::from_euler(YXZ, yaw, pitch, 0)`, при pitch 0 взгляд `R_y(yaw)·(−Z)` совпадает с `move_direction(W)`, а `right = R_y(yaw)·X` совпадает с `move_direction(D)` (`src/camera/mod.rs:80-82`, `crates/gta_sim/src/character/intent.rs:22-25`).
- `Cardinal::wasd_keys` даёт north = W (`bevy_enhanced_input-0.26.0/src/preset/cardinal.rs:38`).
- Tnua считает forward тела как `−Z` (`bevy-tnua-0.32.0/src/builtins/walk.rs:478`), поэтому визуальный "нос" на локальной −Z смотрит правильно (`src/visuals/mod.rs:71`).
- Рантайм `scratch/runtime_t1/summary.json`: W дал Δz = −4.39 м, мышь +200 counts дала Δyaw = −0.418879 рад, это ровно −24°.

**Итог: контрпример не подтвердился**, соглашения согласованы.

## 2. Confirmed correct

Всё ниже перепроверено мной командами или по исходникам, не по IMPL_SUMMARY.
- **Сборка и тесты (мои прогоны):** `cargo test -p gta_sim --offline` дал 8/8. `cargo clippy --workspace --all-targets -- -D warnings` зелёный с фичами по умолчанию и с `--features dev,debug`. `python tools/qa/tree_check.py` дал exit 0. Реализатор гонял clippy по пакетам, я закрыл буквальную команду шага 10 с `--workspace`.
- **Граница headless:** `tree_check.py:223-226` проверяет `-e normal` и `-e normal,dev`. Регэксп `packages()` ловит корневую строку инвертированного дерева. Flip-RED в `scratch/flip_red_tree_result.txt` показывает реальный путь утечки через `avian3d → bevy-tnua-avian3d`.
- **Vendored-патч:** `diff -r` с `~/.cargo/registry/.../bevy-tnua-avian3d-0.12.1` показывает одну удалённую строку `"debug-plugin",`, плюс отсутствует служебный `.cargo-ok`. ADR-001 на месте, условие снятия патча записано.
- **Манифесты:** `Cargo.toml:1-42` 1:1 с планом, пины через `=`, `Cargo.lock` в git, прямой зависимости на `bevy_egui` нет. `crates/gta_sim/Cargo.toml` соответствует плану.
- **Строгий конфиг:** `config/mod.rs:28-38` работает по golden path. `deny_unknown_fields` стоит на `LocomotionConfig` и `CameraConfig`. Ошибка содержит путь и имя поля (`tests/config.rs`).
- **Одна композиция:** `compose_sim` (`crates/gta_sim/src/lib.rs:16-26`) используется и клиентом (`src/main.rs:28`), и тестами (`tests/common/mod.rs:25`).
- **Расписания:** Tnua и `drive_characters` работают в `FixedUpdate` внутри `TnuaUserControlsSystems`, Avian работает в `FixedPostUpdate`. Ввод пишется в `Update`, `just_pressed` в `FixedUpdate` не читается. Защёлка `jump_requested` снимается только потребителем (`character/mod.rs:99-104`).
- **Tnua-маппинг:** `locomotion.rs:34-51`: `speed: 1.0`, `desired_motion = dir·v`, ускорение `run/time_to_run`. Тюнинг только в RON.
- **Гейты:** числа совпадают с выведенными в плане (4.2004 м, rise 1.0625, tap 0.2565). Скрипт `scratch/flip_red_headless.py` честный: портит production-код или данные и ищет конкретные `FAILED`. Все 7 саботажей дали RED с ожидаемым набором тестов: yaw_sign валит только 90°, latch даёт rise 7e−5, данные прыжка дают 1.165.
- **`GATE BROKEN` для обвязки:** корень ассетов, игрок, застрявший цикл, игрок не на полу (`tests/common/mod.rs`).
- **BEI API:** используется `ActionEvents::START`, потому что `STARTED` deprecated с 0.23 (`action/events.rs:46`). Отклонение от плана верное.
- **Камера:** spring-arm из двух cast-ов, мгновенное притягивание и плавное отпускание, снап pivot на первом кадре. Всё по плану, числа берутся из `camera.ron`.
- **Фича `debug` в рантайме (мой прогон):** `cargo build --features dev,debug` и 20 с работы exe прошли без паники (`scratch/review/debug_run.log`). `EguiPlugin` добавлен до `WorldInspectorPlugin` в одном кортеже, а `check_plugins` в `bevy-inspector-egui-0.37.0/src/quick.rs:462` этого требует.
- **Размеры файлов:** все < 250 строк. `unsafe` нет.

## 3. Issues

### I1 — major — буфер прыжка не работает для тапа (`crates/gta_sim/src/character/mod.rs:99-104`)
Защёлка `jump_requested` кормит `controller.action(Jump)` ровно один фиксированный тик. В Tnua буфер ввода держится только пока действие кормят непрерывно:
- `initiation_decision` в воздухе возвращает `Delay`, пока `being_fed_for < input_buffer_time` (`bevy-tnua-0.32.0/src/builtins/jump.rs:224-231`);
- контендер сбрасывается, как только действие перестаёт считаться fed (`controller.rs:720-751`).

**Проба** (`scratch/review/buffer_probe/src/main.rs`, headless, та же `compose_sim`): первый прыжок, затем на 1..6 тиков (16..94 мс) до приземления:
- `jump_requested` на один тик при `jump_held = false`: второго прыжка нет (rise −0.0001) во всех шести случаях;
- `jump_held = true` с того же тика: rise 1.0575 во всех случаях.

Буфер `jump_buffer: 0.1` из GDD §3.1 на деле равен min(время удержания клавиши, 0.1 с). Короткий тап перед приземлением молча теряется. Это тот самый класс "прыжок иногда не срабатывает", ради которого план вводил защёлку, но гейт `tapped_jump_fires_once` проверяет тап только с земли. Корень в плане (3d), а не в отклонении реализатора.

**Направление фикса** (рецепт пересчитать, это не предписание): sim сам держит буфер, например оставшееся время `jump_buffer` в компоненте персонажа. Он кормит `Jump`, пока таймер > 0 и прыжок ещё не начался, и прекращает кормить, как только Tnua начал Jump. Иначе тап превратится в прыжок на полную высоту вместо короткого. Гейт: сценарий пробы как тест в `tests/jump.rs` с flip-RED через возврат одно-тиковой защёлки.

### I2 — major (открытая приёмка, назначена фиксеру) — дополнения владельца в TASK_FINAL не выполнены
- **Fast compiles:** `[profile.dev] opt-level = 1`, `[profile.dev.package."*"] opt-level = 3`, фича `fast = ["bevy/dynamic_linking"]`, `.cargo/config.toml` с `rust-lld.exe`. Ничего из этого в дереве нет: `Cargo.toml` без `[profile.*]`, каталога `.cargo/` нет. FPS 18.2 в QA-прогоне согласуется с `opt-level = 0` для всех зависимостей.
- **Step-up на уступы:** предел подъёма должен быть именованным значением в `locomotion.ron`, с headless-тестом "на пределе забирается, при ~2× блокируется" и flip-RED.

Подсказка фиксеру (выведено из геометрии, пробой **не** проверено): зазор между низом коллайдера и полом равен `float_height − capsule_height/2 = 1.05 − 0.75 = 0.30 м`. Всё ниже этого пружина Tnua поднимает через сенсор. Сверх того, у полусферы капсулы точка контакта с кромкой высотой h над низом коллайдера отстоит от оси на `sqrt(0.3² − (0.3 − h)²)`. При сенсоре радиуса `0.3 − SENSOR_INSET = 0.29` он достаёт до верха кромки при h < ~0.22 м. Оценка непреднамеренного step-up: ~0.5 м. Проверить пробой до выбора решения. Числа гейтов движения и прыжка пересчитать, если изменится геометрия float.

### I3 — minor — предупреждение B0004: визуальные дети игрока без `InheritedVisibility` у родителя (`src/visuals/mod.rs:59-73`)
`target/qa/game.log` и мой прогон `scratch/review/debug_run.log` содержат `warning[B0004]: Entity … has a parent (the Player entity) without InheritedVisibility`. Сейчас это только шум, но скрыть персонажа через `Visibility` (T4/T5: машина, смерть) не получится. Фикс в клиенте: в `visualize_character` добавить `Visibility::default()` на сущность игрока. Это презентационный компонент, sim не трогается.

### I4 — minor — F2-гизмо физики стартуют включёнными (`src/debug/mod.rs:10-19`)
План (шаг 8) требует, чтобы гизмо стартовали выключенными. `GizmoConfig::default()` имеет `enabled: true` (`bevy_gizmos-0.19.1/src/config.rs:237`), а `PhysicsDebugPlugin::build` его не меняет (`avian3d-0.7.0/src/debug_render/mod.rs:93-105`). Фикс: в `DebugToolsPlugin::build` после плагинов выставить `config_mut::<PhysicsGizmos>().0.enabled = false`.

### I5 — minor — числа освещения литералами в коде (`src/visuals/mod.rs:8-27`)
`brightness: 300.0`, `illuminance: 10000.0`, углы солнца, цвета. По GDD §12 рендер-тюнинг живёт в `assets/world/render.ron` и `assets/character/visual.ron`, а солнце с тенями относится к T3. Для заглушки T1 это не блок. Записать в T3, чтобы эти числа переехали в `render.ron` и не прижились как `const`.

### I6 — minor — `SENSOR_INSET` (`crates/gta_sim/src/character/mod.rs:62`)
План оставил решение ревью. Как чистая форма интеграции это закон, но константа напрямую участвует в эффекте step-up (I2). Фиксеру по I2: либо вывести радиус сенсора из нового `max_step_height` в RON, либо явно оставить закон. Решать вместе с I2, отдельно не трогать.

### I7 — minor — flip-RED юнит-теста формулы не записан
Таблица 4e ждёт RED и у `camera_relative_axes`, но скрипт гоняет только `--test movement` (`scratch/flip_red_headless.py:11`). Юнит-тест тривиально упадёт, но заявка "RED" для него не подтверждена выводом. Достаточно добавить `--lib` в прогон axis_sign.

### I8 — minor — `tools/qa/tree_check.py:212` зависит от текущего каталога
`cargo tree` запускается без `cwd=`. Запуск не из корня репо даёт ложный RED с ошибкой манифеста. `brp.py` уже задаёт `REPO`, стоит передать тот же `cwd`.

## 4. Missing coverage

- **Тап прыжка в воздухе в пределах `jump_buffer` до приземления** → прыжок после касания (I1). Сейчас непокрыто, дефект подтверждён.
- **Step-up:** на пределе забирается, при ~2× блокируется (I2, требование владельца).
- **Скорость Sprint/Walk:** гейты проверяют только `Gait::Run`. Саботаж `speed()` в ветке Sprint (например, вернуть `run_speed`) не поймает ни один тест. Нужен дешёвый гейт: 64 тика со Sprint, смещение в `[0.8·v_sprint, v_sprint]`. Приоритет Walk над Sprint живёт в клиенте, это прогон владельца.
- `tapped_jump_fires_once` проверяет "срабатывает", но не "один раз". Либо переименовать, либо добавить проверку, что за 32 тика нет второго взлёта.
- Runtime QA проверяет направление бега только при yaw 0. Этого достаточно для liveness, согласованность соглашений я закрыл в разделе 0 по коду. Как гейт не требую.

## 5. Nits

- `src/camera/mod.rs:95,105`: `Dir3::new(..).unwrap()` можно заменить на `rotation * Dir3::X` / `rotation * Dir3::Z`, тогда `unwrap` в продакшене уходит.
- `src/camera/mod.rs:88`: `Collider::sphere` создаётся каждый кадр. Дёшево, но можно держать в `Local` или ресурсе.
- `OrbitCamera.yaw` накапливается без `rem_euclid(TAU)`. За долгую сессию теряется точность.
- `tests/config.rs:33-36`: при `Ok` (`unwrap_err` паникует) временный каталог остаётся в `%TEMP%`.
- `tests/common/mod.rs:34-40`: после последнего `update()` цикла нет финальной проверки перед `panic!`. С `FixedTimesteps(1)` это не срабатывает.
- `tools/qa/brp.py:37`: захардкожен `-j 4`. Если `Popen` упадёт, `log_file` не закрывается.
- В публичных API sim нет `///` там, где тип не выражает инвариант: `MoveIntent.axis` (x вправо, y вперёд), `yaw` (радианы, yaw камеры), `jump_requested` (edge-защёлка, снимает `FixedUpdate`), `compose_sim`.

## Для владельца (не гейтится)

Feel бега, прыжка и камеры, камера у стены z = 14, pitch мышью, Esc/ЛКМ. Это чек-лист в `QA_REPORT.md`, он полный. FPS 18 в debug-сборке ожидаем до внедрения `opt-level` (I2).

children: 0 launched / 0 reported.
