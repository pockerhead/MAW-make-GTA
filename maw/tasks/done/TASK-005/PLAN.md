# PLAN — TASK-005 (GDD T4): гуманоид с анимациями

Источник объёма: `docs/design/GDD.md` §13 T4, §3.1, §9.2, §10.5, §11, §12 и `TASK_FINAL.md`
(Resolved questions: числа GDD "12 / 14 / 32" не гейт; в манифест пишется то, что реально лежит в
SHA-запиненном архиве, граф анимаций строится на реальном наборе клипов). Все API Bevy 0.19.1,
bevy-tnua 0.32.0, bevy_brp_extras 0.22.6, bevy_remote 0.19.1 ниже сверены по
`~/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/<crate>-<ver>/` и по примерам
`bevy-0.19.1/examples/animation/`. Архив скачан и разобран планировщиком, доказательства в
`scratch/planner/` (скрипты и их вывод).

**Цена ошибки.** Молча ломаются и получают гейты: (1) воспроизводимость ассета (URL, SHA-256 архива и
файлов, лицензия, записанный состав рига: если запись расходится с GLB, граф анимаций позже тихо
сыграет не тот клип); (2) вывод `AnimState` из скорости и опоры: это геймплейное состояние, его будут
читать T5/T7/T8 и QA, ошибка порога или знака вертикальной скорости не видна на глаз сразу. Вид
модели, её разворот, масштаб, тинт, плавность переходов и "ноги не скользят" владелец видит на первом
кадре: это его прогон и скриншоты QA, машинерию (headless-спавн glTF-сцены) под это не строим.

---

## 1. Understanding (как устроено сейчас)

**Ассеты и манифест (T3, TASK-004):**
- `assets/third_party/manifest.ron:1-50` — три пакета Kenney City Kit, у каждого `name, version, page,
  url, archive_sha256, license, license_file, files[(archive, path, sha256)]`. Ожидаемого состава
  "внутри файла" (риг, клипы) схема не знает.
- `crates/gta_sim/src/config/manifest.rs:11-41` — `ThirdPartyManifest / AssetPack / PackFile`, все с
  `#[serde(deny_unknown_fields)]`; `AssetPack::validate` (`:60-124`): имя `^[a-z0-9-]+$`, `page ==
  https://kenney.nl/assets/<name>`, `url` с префиксом `https://kenney.nl/media/pages/assets/<name>/` и
  `.zip`, sha256 (64 hex), безопасные пути, уникальность, `license_file` среди `files`.
  `missing_files` (`:142-152`), `contains_asset` (`:155-161`).
- `crates/gta_sim/tests/asset_manifest.rs` — `manifest_fixtures_are_judged` (`:27-63`, фикстуры в
  `tests/fixtures/manifest/`), `shipped_manifest_is_valid` (`:65-81`: жёстко 3 имени пакетов и по 4 файла
  в каждом), `local_assets_match_manifest` (`:105-170`: если хоть один пакет скачан, все должны быть
  скачаны, файлы совпадают по составу и SHA, лицензия содержит "Creative Commons Zero" и "CC0", в
  `assets/third_party/` нет лишнего).
- `tools/fetch_assets.py:1-380` — зеркало `validate()` на Python (свой парсер подмножества RON
  `:42-143`: struct, list, string, ident; **чисел и `Some(...)` не понимает**, ident+`(` падает на
  `:81-82`), `PACK_KEYS/FILE_KEYS` (`:29-30`), `check_schema` (`:162-205`), `pack_problems/check`
  (`:232-255`), `cached_archive/download/extract/fetch` (`:260-347`, кэш ищется по basename URL,
  по умолчанию `target/asset-cache`, `--cache DIR`).
- `.gitignore`: `/assets/third_party/*` + `!/assets/third_party/manifest.ron`, `*.glb`, `*.png`, `*.zip`
  игнорируются. Бинарники не коммитятся (правило владельца, TASK-004).

**Персонаж (T1):**
- `crates/gta_sim/src/character/mod.rs:14-133` — `CharacterScheme` (Tnua, basis `TnuaBuiltinWalk`, action
  `Jump`), `Character` (`#[require(MoveIntent, JumpBuffer)]`, `:20-23`), `CharacterBody {radius, height,
  float_height}` (`:30-36`), `CharacterPlugin` (`:52-66`: `drive_characters` в `FixedUpdate` в
  `TnuaUserControlsSystems`), `character_components` (`:70-92`: `RigidBody::Dynamic`, капсула,
  `LockedAxes::ROTATION_LOCKED.unlock_rotation_y()`).
- `crates/gta_sim/src/character/locomotion.rs:8-58` — `LocomotionConfig` (строгий, `locomotion.ron`),
  `speed(gait)`, `tnua_config()`.
- `assets/character/locomotion.ron` — walk 1.8, run 4.5, sprint 6.8 м/с, капсула r 0.3, h 1.5,
  float_height 1.05. Центр тела висит на 1.05 м над землёй, верх капсулы на 1.05 + 0.75 = 1.8 м.
- `crates/gta_sim/src/player/mod.rs:19-31` — спавн игрока на `OnEnter(Playing)`,
  `Transform = spawn + Y·float_height`.
- `crates/gta_sim/src/lib.rs:18-42` — `compose_sim` (единственная композиция sim).
- `crates/gta_sim/tests/common/mod.rs` — `headless_app`, `settle`, `run_ticks`, `set_intent`, `player`.

**Клиент:**
- `src/main.rs:44-80` — `preflight`: валидирует `render.ron`, грузит манифест, проверяет, что пути пропов
  есть в манифесте и скачаны. `:82-138` — сборка App: `DefaultPlugins` (включает `bevy_gltf`,
  `bevy_animation`, `gltf_animation`, `bevy_world_serialization`: фичи `default → 3d → 3d_bevy_render`,
  `bevy-0.19.1/Cargo.toml:2605-2635`), `compose_sim`, конфиги камеры/рендера, плагины.
- `src/visuals/mod.rs:85-116` — `visualize_character`: observer `On<Add, CharacterBody>` вешает на тело
  капсулу-меш и "нос"-кубоид. Это заменяемая заглушка T1.
- `src/visuals/props.rs:38-73` — прецедент загрузки Kenney GLB (`GltfAssetLabel::Primitive` + свой
  `StandardMaterial`); `src/visuals/city_gate.rs` — прецедент презентационного гейта в клиенте
  (`cargo test -p gta_like --bin gta_like`), в т.ч. `render_props_reference_manifest_files` (`:186-197`).
- `src/camera/mod.rs:70-121` — ноги = `translation − Y·float_height`; `:123-125` на игрока вешается
  `TransformInterpolation` (дочерняя модель унаследует сглаживание).
- `tools/qa/brp.py` — `Game`, `component_path`, `query`, `send_keys` (неблокирующий: extras ставит
  таймер отпускания, `bevy_brp_extras-0.22.6/src/keyboard/keys.rs:128-165`), `screenshot`,
  `mutate_component`, `log_tail`. `tools/qa/scenarios/t3.py` — телепорт в `CityLandmarks.park_center`,
  поворот камеры через `OrbitCamera.yaw`, фильтр ошибок лога по словам gltf/asset.

**Архив Kenney Mini Characters (проверено планировщиком, не по GDD):**
- Страница `https://kenney.nl/assets/mini-characters`: "Files 25×, License Creative Commons CC0, Updates
  1.0, Released in 2024" (`scratch/planner/page.html`). Ссылка на zip в HTML страницы:
  `https://kenney.nl/media/pages/assets/mini-characters/bfc7e272b4-1774770718/kenney_mini-characters.zip`.
- Повторная загрузка через Python urllib: 2 403 059 байт, SHA-256
  `9e1d48e6d7b8479ebbe84df71eb5bd8e1b3f0da546dea641890dccc8a02d0999` (совпало с копией premise-challenge
  `scratch/kenney_mini-characters.zip`).
- `License.txt`: "Mini Characters (1.0) … License: (Creative Commons Zero, CC0)" — содержит обе строки,
  которые проверяет `local_assets_match_manifest`.
- `scratch/planner/audit_rig.txt` (скрипт `audit_rig.py`): **12** GLB персонажей
  `character-{female,male}-{a..f}.glb`; в каждом **2** skinned-меша (`body-mesh`, `head-mesh`) и **2** skin,
  оба skin связывают одни и те же **7** суставов по порядку `root, leg-left, leg-right, torso, arm-left,
  arm-right, head`; **32** клипа в одном и том же порядке (индекс = индекс анимации glTF):
  `static, idle, walk, sprint, jump, fall, crouch, sit, drive, die, pick-up, emote-yes, emote-no,
  holding-right, holding-left, holding-both, holding-right-shoot, holding-left-shoot, holding-both-shoot,
  attack-melee-right, attack-melee-left, attack-kick-right, attack-kick-left, interact-right,
  interact-left, wheelchair-sit, wheelchair-look-left, wheelchair-look-right, wheelchair-move-forward,
  wheelchair-move-back, wheelchair-move-left, wheelchair-move-right`. **Клипа `run` нет.** Все 12 моделей
  дают одинаковый набор фактов (`identical_rig_facts_across_models True`). Один материал `colormap`
  (текстура-палитра `Textures/colormap.png` 512×512, `KHR_texture_transform` только на base color, это Bevy
  поддерживает: `bevy_gltf-0.19.1/src/lib.rs:118,126`).
- Геометрия (`scratch/planner/inspect_male_a.txt`): ступни на y = 0, верх головы у `male-a/male-f/female-f`
  0.67132 ед. модели; у вариантов с причёской выше (до 0.79279). Тазобедренный сустав на y 0.17625 над
  `root`. Ноги жёсткие палки без колена.
- Направление "вперёд" модели = **+Z** (glTF-конвенция; подтверждено клипом `attack-kick-right`: `root`
  уходит в −Z на замахе и в +Z на ударе, `scratch/planner/clip_motion.txt`). Bevy-загрузчик по умолчанию
  не конвертирует оси (`GltfConvertCoordinates::default()` = всё false, `bevy_gltf-0.19.1/src/lib.rs:243-250`).
- Клипы (`clip_motion.txt`): `walk` 0.667 с, амплитуда ног ±60°, `root` подпрыгивает по Y до 0.05 ед.;
  `sprint` 0.5 с, ±90°, подскок `root` до 0.2 ед.; `idle` 1.333 с; `jump` 0.5 с и `fall` 0.333 с —
  зацикленные позы; `root`-translation только по Y (root motion вперёд нет, клипы на месте).

**Bevy 0.19.1 API (сверено):**
- Сцены переименованы: `WorldAssetRoot(Handle<WorldAsset>)` (`bevy_world_serialization-0.19.1/src/components.rs:23`),
  событие `WorldInstanceReady { entity, instance_id }` (`world_asset_spawner.rs:33`), оба в prelude
  (`bevy_internal-0.19.1/src/prelude.rs:71-72`); миграционный гайд 0.18→0.19
  (https://bevy.org/learn/migration-guides/0-18-to-0-19/) называет ровно эти замены `SceneRoot` /
  `SceneInstanceReady`. Паттерн "spawn `WorldAssetRoot` + `.observe(On<WorldInstanceReady>)` → найти
  `AnimationPlayer` среди потомков → вставить `AnimationGraphHandle`" — `bevy-0.19.1/examples/animation/animated_mesh.rs`.
- `GltfAssetLabel::Scene(usize)`, `GltfAssetLabel::Animation(usize)` (`bevy_gltf-0.19.1/src/label.rs:34-60`);
  метка анимации = индекс анимации glTF (`loader/mod.rs:580-583`). `AnimationPlayer` вешается на корень
  анимации (`:1086-1094`), узлы получают `AnimationTargetId::from_names(path)` + `AnimatedBy(root)`
  (`:1545-1560`). Меш-сущности получают `Name = "<mesh>.<material>"` и `GltfMeshName(<mesh>)`
  (`:1722-1733`); материал вешает `bevy_pbr` как `MeshMaterial3d<StandardMaterial>`
  (`bevy_pbr-0.19.1/src/gltf.rs:128-150`), у `body-mesh` и `head-mesh` он общий (материал 0).
  Границы skinned-меша по умолчанию `Dynamic` (`bevy_gltf-0.19.1/src/lib.rs:214`).
- `AnimationGraph::new / add_clip(handle, weight, parent) -> AnimationNodeIndex`, `graph.root`
  (`bevy_animation-0.19.1/src/graph.rs:430-490`); `AnimationGraphHandle` (`graph.rs:138`);
  `AnimationTransitions::new / play(&mut player, node, Duration) -> &mut ActiveAnimation` (перезапускает
  клип через `player.start`, старый гасит за `Duration`; `transition.rs:68-103`);
  `ActiveAnimation::repeat / set_speed` (`lib.rs:641,666`); `AnimationPlayer::animation_mut` (`lib.rs:983`).
  Всё в `bevy::prelude` через `animation::prelude::*` (`bevy_animation-0.19.1/src/lib.rs:57-63`).
- Tnua: `TnuaController::is_airborne() -> Result<bool, _>` (`bevy-tnua-0.32.0/src/controller.rs:572`);
  "airborne" наступает только после истечения coyote-таймера (`builtins/walk.rs:586-592`), прыжок
  обнуляет coyote (`builtins/jump.rs:508`), приземление сбрасывает таймер сразу (`walk.rs:431-437`).
  Тело поворачивается так, что его −Z смотрит в `desired_forward` (`walk.rs:477-482`).
  `bevy_tnua::TnuaSystems` (реэкспорт `bevy-tnua-physics-integration-layer-0.13.0/src/lib.rs:100`).
- BRP: `world.get_components { entity, components: [full path], strict }`
  (`bevy_remote-0.19.1/src/builtin_methods.rs:118-136`); `brp_extras/send_keys { keys, duration_ms }`,
  коды `KeyW`, `ShiftLeft`, `Space`, `AltLeft` (`bevy_brp_extras-0.22.6/src/keyboard/key_code.rs`).

**Зависимости.** Новых крейтов нет: клиент уже тянет `bevy_gltf`/`bevy_animation`
(`scratch/planner/tree_gltf.txt`: `cargo tree --offline -p gta_like -i bevy_gltf` → есть; `-p gta_sim -i
bevy_gltf` → нет). `Cargo.toml`/`Cargo.lock` не меняются, передавать имплементеру lock не нужно.

---

## 2. Approach

### 2.1 Ассет: манифест с записанным ригом

Добавить пакет `mini-characters` в `assets/third_party/manifest.ron` (12 GLB + палитра + лицензия,
хэши посчитаны, шаг 1) и **необязательное** поле пакета `rig: Some((models, skinned_meshes, joints,
clips))` — списки имён. Число персонажей, суставов на skin и клипов = длины списков: одна запись, без
дублирования числа и списка. Семантика: каждая модель из `models` имеет ровно `skinned_meshes`
(по порядку узлов), каждый её skin связывает ровно `joints` (по порядку), её анимации ровно `clips`
(по порядку glTF-индекса).

Кто что проверяет:
- **Rust `validate()`** (`gta_sim`) — схема и согласованность: модели есть в `files`, оканчиваются на
  `.glb`, списки не пусты и без повторов. Ловит порчу манифеста в `cargo test -p gta_sim`.
- **`tools/fetch_assets.py`** — соответствие записи **байтам GLB**: при `extract` (до установки, по данным
  из zip) и в `--check` (по установленным файлам). Это единственное место, которое читает архив, и
  байты уже запинены SHA-256, поэтому одной проверки на тех же байтах достаточно. Rust-проверка GLB
  потребовала бы новой dev-зависимости `serde_json` ради дубля — отказ.

Python-парсер учится `Some(<value>)` (4 строки). Числа не нужны.

### 2.2 Геймплей: `AnimState` в `gta_sim/character`

`AnimState { Idle, Walk, Run, Sprint, Jump, Fall }` — компонент (enum-поле, без archetype move),
required у `Character`, `Reflect` + зарегистрирован (BRP). Шесть вариантов = шесть строк таблицы из
приёмки ("стоит, идёт, бежит, спринт, в воздухе вверх, падение"): с пятью вариантами строка "бежит"
неотличима от "спринт" в тесте. GDD перечисляет пять **клипов** (idle/walk/sprint/jump/fall), клипа
`run` в архиве нет, поэтому `Run` получает клип через данные (`visual.ron`), по умолчанию `sprint` на
пониженной скорости (обоснование в 2.4).

Чистая функция:
```rust
pub fn anim_state(velocity: Vec3, airborne: bool, cfg: &LocomotionConfig) -> AnimState
```
- `airborne` → `Jump`, если `velocity.y > 0`, иначе `Fall` (апекс `vy == 0` = `Fall`).
- на опоре — по **горизонтальной** скорости `h = |(vx, vz)|`: `h < anim_idle_speed` → `Idle`;
  `h < (walk_speed + run_speed)/2` → `Walk`; `h < (run_speed + sprint_speed)/2` → `Run`; иначе `Sprint`.
  Границы — середины между скоростями походок из того же `locomotion.ron` (один источник: владелец
  меняет `run_speed`, границы едут сами). Новое число одно: `anim_idle_speed` (м/с) в `locomotion.ron`.
  На шипованных данных: 0.2 / 3.15 / 5.65 м/с.
- "Опора" = `!controller.is_airborne()` Tnua (ошибка "конфиг не подтянут" → на опоре). Tnua сам даёт
  дебаунс: сход с бордюра/ступени становится `Fall` только после coyote 0.12 с, на лестнице `Fall` не
  мигает; прыжок обнуляет coyote, `Jump` появляется через ~1 тик после отрыва.

Система `update_anim_state` в `FixedUpdate` `.after(TnuaSystems)`: читает `LinearVelocity`,
`TnuaController`, пишет `AnimState` через `set_if_neq` (честный `Changed<AnimState>` для потребителей).
Презентация `AnimState` не пишет.

### 2.3 Презентация: модель, граф, тинт, масштаб (`src/visuals/character.rs`)

- Данные: новый `assets/character/visual.ron` (файл есть в GDD §12) → `CharacterVisualConfig`
  (строгий). Модель, высота 1.8 м и высота модели 0.67132 ед. (масштаб = 1.8 / 0.67132 = 2.6813), имя
  тонируемого меша и цвет тинта, длительность кроссфейда, по одному клипу на каждый `AnimState` и
  "родная" скорость клипов ходьбы.
- Индексы клипов берутся **из манифеста**: `preflight` находит риг модели
  (`ThirdPartyManifest::rig_for(model_path)`) и превращает имена клипов из `visual.ron` в индексы glTF
  (`PackRig::clip_index`). Порядок клипов в манифесте проверен `fetch_assets.py` по байтам GLB. Граф
  собирается синхронно в `Startup` из `GltfAssetLabel::Animation(i)` (паттерн примера `animated_mesh.rs`),
  без ожидания `Handle<Gltf>`. Отказ от `Gltf::named_animations`: лишний асинхронный шаг и гонка между
  готовностью сцены и готовностью графа.
- Граф: по узлу на каждый `AnimState` (6 узлов, `Run` и `Sprint` могут ссылаться на один клип; у каждого
  узла свой `ActiveAnimation`, кроссфейд между ними работает).
- Модель: observer `On<Add, CharacterBody>` (замена капсулы-заглушки) спавнит дочернюю сущность
  `CharacterModel` с `WorldAssetRoot(Scene0)`, `Transform { translation: (0, −float_height, 0), rotation:
  R_y(π), scale: 2.6813 }` и `.observe(on_model_ready)`. `R_y(π)` — закон (glTF +Z вперёд → Bevy −Z),
  `const`, не тюнинг.
- `on_model_ready (On<WorldInstanceReady>)`: среди потомков найти `AnimationPlayer` → вставить
  `AnimationGraphHandle`, `AnimationTransitions`, `CharacterAnimator { character, shown: AnimState::Idle }`
  и запустить `Idle` (`Duration::ZERO`, `repeat`); найти меш с `GltfMeshName == tinted_mesh` → клон его
  `StandardMaterial` с `base_color = tint` → новый `MeshMaterial3d`. `head-mesh` не трогаем.
- `drive_character_animation` (`Update`): если `AnimState` персонажа ≠ `shown` → `transitions.play(...,
  blend)`.repeat(); каждый кадр — скорость проигрывания главного клипа `playback_rate(...)`.
- Тинт: множитель на весь `body-mesh`. Честное ограничение (`scratch/planner/uv_colors.py`): UV
  `body-mesh` попадают и в тексели кожи (кисти, шея), тинт их тоже окрасит. Маски одежды в палитре нет.
  Дефолт тинта `(1,1,1)`; вид решает владелец (Open question 2).

### 2.4 Ноги не скользят: скорость клипа от скорости тела

Стандартный приём для анимаций "на месте": скорость проигрывания = фактическая скорость / скорость,
которую клип "подразумевает" при rate 1 (distance/speed matching; stride warping — вторая половина,
нам не нужна: ноги-палки без IK). Источники: https://nikoff.cc/resources/fix-foot-sliding-in-place-animations ,
https://mocaponline.com/blogs/mocap-news/locomotion-animations-game-dev ,
https://gamedev.net/forums/topic/646774-matching-walkrun-animation-with-character-movement/ .

`rate = h / (native_speed × scale)`, где `native_speed` — в единицах модели в секунду (не в м/с), чтобы
смена роста в `visual.ron` не требовала перенастройки. Для `Idle/Jump/Fall` rate = 1.

Выведенные значения (`scratch/planner/clip_motion.txt` + расчёт в `scratch/planner/`): нога — жёсткий
маятник длины L = 0.17625 ед. Ступня касается земли только около θ = 0 (на крайних фазах `root`
подпрыгивает и ступня в воздухе), поэтому условие "без скольжения" — скорость тела = L·ω в момент
θ = 0:
- `walk`: ключи −13.9° → 0° → +13.9° за 0.0667 с → ω = 417°/с = 7.28 рад/с → 0.17625·7.28 = **1.28 ед/с**
  (×2.6813 = 3.44 м/с);
- `sprint`: −14.4° → +14.4° за 0.0333 с → ω = 865°/с = 15.1 рад/с → **2.66 ед/с** (7.13 м/с).

Скорости проигрывания на шипованных данных: Walk 1.8 м/с → `walk` ×0.52; Run 4.5 м/с → `sprint` ×0.63
(каденс 152 шага/мин, близко к реальному бегу трусцой); Sprint 6.8 м/с → `sprint` ×0.95. Альтернатива
для Run — `walk` ×1.31 (236 шагов/мин, семенит) — хуже, поэтому по умолчанию `run.clip = "sprint"`.
Если на глаз ноги всё же едут, владелец крутит `native_speed` в `visual.ron` (альтернативная оценка
"средняя скорость опорной фазы" даёт 0.92 и 1.51 ед/с — это нижняя граница диапазона для подбора).

### 2.5 Что гейтится и чем

| Утверждение | Гейт | Класс |
|---|---|---|
| Таблица `AnimState` (6 случаев + границы + горизонтальность скорости) | `gta_sim` unit-тест чистой функции | корректность |
| Система реально пишет `AnimState` игроку в собранном App (бег, спринт, ходьба, прыжок → падение → стоит) | `crates/gta_sim/tests/anim_state.rs` на `headless_app()` | liveness + порядок |
| Строгость `locomotion.ron` с новым полем | существующий `shipped_locomotion_config_loads` + `unknown_field_names_file_and_field` | корректность |
| Манифест: схема рига | фикстуры `valid_rig.ron`, `bad_rig_model.ron`, `bad_rig_duplicate_clip.ron` + `shipped_manifest_is_valid` | корректность |
| Запись рига = байты GLB | `python tools/fetch_assets.py --check` (и `extract`) | корректность |
| Файлы/SHA/лицензия пакета на диске | существующий `local_assets_match_manifest` | корректность |
| Имена клипов/меша/модели из `visual.ron` есть в риге манифеста | клиентский тест `character_visuals_reference_manifest_rig` | корректность |
| `playback_rate`, разворот модели (3 направления) | клиентские unit-тесты | корректность |
| Модель видна, анимации правильные, ноги не едут, тинт, рост | прогон владельца + скриншоты `t4.py` | владелец |

Headless-спавн glTF-сцены в клиентском тесте не делаем: нужен `GltfPlugin` с рендер-ассетами, а дефект
виден на первом кадре. Это осознанный отказ от гейта.

---

## 3. Steps

### Шаг 1. Манифест: пакет `mini-characters` (`assets/third_party/manifest.ron`)

Добавить четвёртый пакет после `city-kit-industrial` (строки-хэши скопированы из
`scratch/planner/audit_rig.txt`, пересчитывать не нужно, `fetch_assets.py` всё равно сверит):

```ron
        (
            name: "mini-characters",
            version: "1.0",
            page: "https://kenney.nl/assets/mini-characters",
            url: "https://kenney.nl/media/pages/assets/mini-characters/bfc7e272b4-1774770718/kenney_mini-characters.zip",
            archive_sha256: "9e1d48e6d7b8479ebbe84df71eb5bd8e1b3f0da546dea641890dccc8a02d0999",
            license: CC0,
            license_file: "License.txt",
            files: [
                (archive: "Models/GLB format/character-female-a.glb", path: "character-female-a.glb", sha256: "8cfcff43460da8b421f2a7fdfb43ec177321cad2746db9497cbf128d5806e2a8"),
                (archive: "Models/GLB format/character-female-b.glb", path: "character-female-b.glb", sha256: "2288438e7baf9acc91a870c82dc00d66710bb486592cfc3474ef8ed93a03863a"),
                (archive: "Models/GLB format/character-female-c.glb", path: "character-female-c.glb", sha256: "3cd9e1b5d6409fce0ba1af617d9e008c3069493185a4c100f49fb99a2626a055"),
                (archive: "Models/GLB format/character-female-d.glb", path: "character-female-d.glb", sha256: "67f61708743bd34f91a4c2ba8160b61bf2e7d4ec2ca6647dfe99873422561410"),
                (archive: "Models/GLB format/character-female-e.glb", path: "character-female-e.glb", sha256: "3ee3939f80d718945fcb9199f909a92c39669e38aeec7f544536b5dce6e79f98"),
                (archive: "Models/GLB format/character-female-f.glb", path: "character-female-f.glb", sha256: "2ff4311897bfaf99be80d2fa13918db30f506607da0291640b2609d82b70e0ca"),
                (archive: "Models/GLB format/character-male-a.glb", path: "character-male-a.glb", sha256: "77572792bfe2773b715b8cd8e18644b52b3e1f155fe10450254b50f9c364382a"),
                (archive: "Models/GLB format/character-male-b.glb", path: "character-male-b.glb", sha256: "791fc0c203924c175c0a3d5b60d030daf36797b039506689cf7cd01ef5253b3c"),
                (archive: "Models/GLB format/character-male-c.glb", path: "character-male-c.glb", sha256: "672a6506f7475bbd655da1d7ff712c1729a430f134c62385a4e5d3c1378acb40"),
                (archive: "Models/GLB format/character-male-d.glb", path: "character-male-d.glb", sha256: "dd12b2e75ffb1cdb45aaec3916c3d6e732929ccb4a8b89b946f69baaabfa64da"),
                (archive: "Models/GLB format/character-male-e.glb", path: "character-male-e.glb", sha256: "cd76681090ce29861b055b0f8b2bdcf84bfccc4ca816f428df42a9bb76d6e1b6"),
                (archive: "Models/GLB format/character-male-f.glb", path: "character-male-f.glb", sha256: "ed151fc47c5cd6be9693c524e92f045170c965819f4f81b28ed66dcc3c1085bf"),
                (archive: "Models/GLB format/Textures/colormap.png", path: "Textures/colormap.png", sha256: "0d4947d34ff32acf4a359c7f22ca784e057e7e72f622170a9a77b6fc88fdb70e"),
                (archive: "License.txt", path: "License.txt", sha256: "28358ae5accc85b572eb42507956afc8beae05acb4648bb9026a5714d421b785"),
            ],
            // Verified against the GLB bytes by tools/fetch_assets.py: every model has exactly these
            // skinned meshes, each skin binds exactly these joints, animations are exactly these clips (glTF order).
            rig: Some((
                models: ["character-female-a.glb", "character-female-b.glb", "character-female-c.glb",
                         "character-female-d.glb", "character-female-e.glb", "character-female-f.glb",
                         "character-male-a.glb", "character-male-b.glb", "character-male-c.glb",
                         "character-male-d.glb", "character-male-e.glb", "character-male-f.glb"],
                skinned_meshes: ["body-mesh", "head-mesh"],
                joints: ["root", "leg-left", "leg-right", "torso", "arm-left", "arm-right", "head"],
                clips: ["static", "idle", "walk", "sprint", "jump", "fall", "crouch", "sit", "drive", "die",
                        "pick-up", "emote-yes", "emote-no", "holding-right", "holding-left", "holding-both",
                        "holding-right-shoot", "holding-left-shoot", "holding-both-shoot",
                        "attack-melee-right", "attack-melee-left", "attack-kick-right", "attack-kick-left",
                        "interact-right", "interact-left", "wheelchair-sit", "wheelchair-look-left",
                        "wheelchair-look-right", "wheelchair-move-forward", "wheelchair-move-back",
                        "wheelchair-move-left", "wheelchair-move-right"],
            )),
        ),
```
Зачем все 12 моделей: запись "12 персонажей" должна описывать то, что реально ставится; варианты a-f
нужны T8/T9 (GDD §9.2). Объём ~3 МБ, в git не попадает (`.gitignore` уже покрывает
`/assets/third_party/*`).

### Шаг 2. Схема рига в Rust (`crates/gta_sim/src/config/manifest.rs`)

- Новый тип после `PackFile` (`:35-41`):
  ```rust
  /// Skeleton and clips of the skinned models of a pack, recorded from the pinned archive.
  #[derive(Deserialize, Debug, Clone)]
  #[serde(deny_unknown_fields)]
  pub struct PackRig {
      pub models: Vec<String>,
      pub skinned_meshes: Vec<String>,
      pub joints: Vec<String>,
      pub clips: Vec<String>,
  }
  ```
- В `AssetPack` (`:19-28`) поле `#[serde(default)] pub rig: Option<PackRig>,` (старые пакеты и
  фикстуры без поля остаются валидными).
- В `AssetPack::validate` перед `Ok(())` (`:122`) вызвать `rig.validate(name, &paths)` если есть:
  `models` не пуст, каждая модель есть в `paths` и оканчивается на `.glb`; каждый из четырёх списков не
  пуст, без пустых строк и без повторов. Все сообщения начинаются с `pack {name}: rig ...` (фикстуры
  ищут слово `rig`). Golden Path: helper `fn unique_names(list: &[String], what: &str) -> Result<(), String>`.
- `impl PackRig { pub fn clip_index(&self, name: &str) -> Option<usize> }` (позиция в `clips` = индекс
  glTF-анимации).
- `impl ThirdPartyManifest { pub fn rig_for(&self, asset_path: &str) -> Option<&PackRig> }`: пакет, у
  которого `asset_path == "third_party/<pack>/<model>"` для одной из `rig.models`.

Проверка: `cargo test -p gta_sim --test asset_manifest`.

### Шаг 3. Фикстуры и тест манифеста (`crates/gta_sim/tests/`)

- Новые фикстуры в `tests/fixtures/manifest/` (копии `valid_minimal.ron` с правкой):
  - `valid_rig.ron` — `valid_minimal` + `rig: Some((models: ["light-square.glb"], skinned_meshes: ["m"],
    joints: ["root"], clips: ["idle"]))` (проверяет, что `Some((...))` парсится и валидируется);
  - `bad_rig_model.ron` — `models: ["missing.glb"]` (нет в `files`) → ошибка содержит `rig`;
  - `bad_rig_duplicate_clip.ron` — `clips: ["idle", "idle"]` → ошибка содержит `rig`.
- `tests/asset_manifest.rs`:
  - `manifest_fixtures_are_judged` (`:27-63`): `valid_rig.ron` рядом с `valid_minimal.ron` (parse +
    validate ok); две bad-фикстуры в список семантических с ключом `"rig"`.
  - `shipped_manifest_is_valid` (`:65-81`): множество имён + `"mini-characters"`; вместо `files.len()
    == 4` для всех — ожидание по пакету (`city-kit-*` → 4, `mini-characters` → 14); у `city-kit-*`
    `rig.is_none()`; у `mini-characters` риг: `models.len() == 12`, `skinned_meshes.len() == 2`,
    `joints.len() == 7`, `clips.len() == 32`, и `clip_index` для `idle, walk, sprint, jump, fall` =
    `Some(1), Some(2), Some(3), Some(4), Some(5)`. Числа — факты аудита архива
    (`scratch/planner/audit_rig.txt`), независимый от манифеста источник; тест ловит выпавший клип или
    сдвиг порядка при ручной правке манифеста. Это фиксация записи, а не проверка байтов (её делает
    шаг 4).
  - `local_assets_match_manifest` не менять: после установки пакета он сверит 14 файлов и лицензию
    (в `License.txt` есть "Creative Commons Zero" и "CC0", проверено).

### Шаг 4. `tools/fetch_assets.py`: `Some(...)` и проверка рига по GLB

- `Parser.value` (`:70-84`): если ident == `"Some"` и следующий символ `(` → `expect("(")`, `inner =
  self.value()`, `expect(")")`, вернуть `inner`. Остальные ident+`(` по-прежнему ошибка.
- Схема: `OPTIONAL_PACK_KEYS = {"rig"}`, `RIG_KEYS = {"models", "skinned_meshes", "joints", "clips"}`;
  `exact_keys(obj, keys, where, optional=frozenset())` — лишние = `set(obj) − keys − optional`, недостающие
  = `keys − set(obj)`. В `check_schema` для пакета с `rig` — те же правила, что в Rust (шаг 2), с тем же
  словом `rig` в сообщениях. Докстринг модуля: `--validate-only` теперь понимает `rig`.
- Разбор GLB (только stdlib): `glb_json(data)` — magic `b"glTF"`, version 2, длина = `len(data)`, первый
  чанк типа `0x4E4F534A` → `json.loads`. `rig_facts(gltf)` → `(skinned, joints_per_skin, clips)`:
  `skinned` = имена узлов с ключом `skin` в порядке индекса узла, `joints_per_skin` = для каждого skin
  список имён узлов `joints`, `clips` = имена `animations`.
- `rig_problems(pack, read)` (где `read(path) -> bytes`): для каждой модели из `rig.models` сравнить
  `skinned == rig.skinned_meshes`, каждый skin `== rig.joints`, число skin == число skinned-мешей,
  `clips == rig.clips`; сообщение `"{pack}: {model}: rig <поле> {фактическое}, manifest {записанное}"`.
- Вызовы: в `pack_problems` (`:232-244`) — после сверки SHA, если хэш-проблем нет и `rig` есть,
  `problems += rig_problems(pack, lambda p: (directory / p).read_bytes())`; в `extract` (`:300-321`) —
  после записи всех файлов во временную папку, до `os.replace`, `rig_problems` по `tmp` и при проблемах
  `raise ManifestError` (временная папка удаляется существующим `except`).
- Установка без сети (codex-песочница не ходит на kenney.nl):
  `python tools/fetch_assets.py --cache maw/tasks/in_progress/TASK-005/scratch` — там лежит
  `kenney_mini-characters.zip` с нужным SHA (basename совпадает с URL).

Проверка: `python tools/fetch_assets.py --validate-only crates/gta_sim/tests/fixtures/manifest/<f>.ron`
для `valid_rig`, `bad_rig_model`, `bad_rig_duplicate_clip` (valid / error со словом `rig` / error со
словом `rig`, как у Rust); `python tools/fetch_assets.py --check` → "third-party packs match the manifest".

### Шаг 5. `AnimState` в `gta_sim` (новый `crates/gta_sim/src/character/anim.rs`, ~60 строк + тесты)

```rust
/// Locomotion animation state derived from body velocity and ground support.
#[derive(Component, Reflect, Default, Clone, Copy, Debug, PartialEq, Eq)]
#[reflect(Component, Default)]
pub enum AnimState { #[default] Idle, Walk, Run, Sprint, Jump, Fall }

pub fn anim_state(velocity: Vec3, airborne: bool, cfg: &LocomotionConfig) -> AnimState
```
Тело по 2.2, let-else/early return, без вложенности. Система:
```rust
pub(super) fn update_anim_state(
    cfg: Res<LocomotionConfig>,
    mut query: Query<(&LinearVelocity, &TnuaController<CharacterScheme>, &mut AnimState), With<Character>>,
)
```
`let airborne = controller.is_airborne().unwrap_or(false);` → `state.set_if_neq(anim_state(velocity.0,
airborne, &cfg))`.

Unit-тест `anim_state_table` в том же файле (`#[cfg(test)]`), конфиг — шипованный `locomotion.ron`
через `load_config` c корнем `CARGO_MANIFEST_DIR/../../assets` (или литеральный `LocomotionConfig` в
тесте — на выбор имплементера; ожидания ниже выведены из шипованных чисел walk 1.8, run 4.5, sprint 6.8,
anim_idle_speed 0.2 → границы 0.2 / 3.15 / 5.65):

| velocity (x, y, z) | airborne | ожидание | что ловит |
|---|---|---|---|
| (0, 0, 0) | false | Idle | стоит |
| (0.1, 0, 0) | false | Idle | ниже порога покоя |
| (0, 0, −1.8) | false | Walk | идёт |
| (1.8·0.6, 0, −1.8·0.8) = (1.08, 0, −1.44) | false | Walk | модуль, а не одна ось |
| (0, 0, −4.5) | false | Run | бежит |
| (0, 0, −6.8) | false | Sprint | спринт |
| (0, 3.0, −2.5) | false | Walk | горизонтальная скорость (по 3D-длине 3.9 → Run) |
| (0, 3.0, −4.5) | true | Jump | в воздухе вверх |
| (0, −2.0, −4.5) | true | Fall | падение |
| (0, 0, 0) | true | Fall | апекс = Fall |
| (0, 3.15 граница) → (0, 0, −3.14) / (0, 0, −3.16) | false | Walk / Run | граница walk/run |

В `character/mod.rs`: `mod anim; pub use anim::{AnimState, anim_state};`; `#[require(MoveIntent,
JumpBuffer, AnimState)]` у `Character` (`:22`); в `CharacterPlugin::build` (`:54-65`)
`.register_type::<AnimState>()` и `update_anim_state.after(TnuaSystems)` в `FixedUpdate`
(`use bevy_tnua::TnuaSystems`). Новых `Res` у существующих систем нет — тест-харнессы не затронуты.

### Шаг 6. Данные локомоции (`locomotion.rs`, `assets/character/locomotion.ron`)

- `LocomotionConfig` (`locomotion.rs:10-26`): поле `pub anim_idle_speed: f32` (м/с, ниже — `Idle`).
- `locomotion.ron`: `anim_idle_speed: 0.2,` после `sprint_speed`. Обоснование: разгон до бега за
  0.15 с (30 м/с²) проходит 0.2 м/с за один тик 64 Гц, порог не задерживает старт анимации.
- Прецедент: `LocomotionConfig` не валидирует диапазоны; проверку `0 < anim_idle_speed < walk_speed`
  добавлять не нужно (YAGNI; неверное значение владелец увидит сразу).

### Шаг 7. Интеграционный гейт `crates/gta_sim/tests/anim_state.rs` (новый)

На `common::headless_app()` (та же `compose_sim`), время по тикам:
- `idle_after_settle`: `settle` → `AnimState::Idle`.
- `gaits_map_to_states`: для `(Gait::Walk, Walk)`, `(Gait::Run, Run)`, `(Gait::Sprint, Sprint)`:
  `settle`, `axis = Vec2::Y`, `run_ticks(32)` → ожидаемое состояние. Вывод: разгон 30 м/с² даёт
  установившуюся скорость за ≤ 15 тиков (6.8/30 = 0.23 с), `movement.rs` уже показывает, что за 64 тика
  проходится ≥ 0.8·v (путь по −Z от спавна (0,0,0) на площадке TestArea свободен: рампа стоит на
  x −12..−8, лестница на x 8.5..11.5, `world/test_area.rs:6-35`; пол до z = −40; за 0.5 с спринта 3.4 м).
- `jump_goes_up_then_falls_then_lands`: `settle`, `jump_held = true` на 30 тиков (как
  `late_tap_is_buffered_until_landing`), затем `false`; записать `AnimState` за 120 тиков. Проверить
  порядок: первое не-`Idle` состояние — `Jump`; после последнего `Jump` встречается `Fall`; последнее
  состояние — `Idle`; `Walk/Run/Sprint` не встречаются. Числа тиков не хардкодим (их даёт Tnua).
Класс: liveness + порядок, корректность таблицы держит unit-тест шага 5.

### Шаг 8. Клиент: конфиг визуала (`src/visuals/character_config.rs` новый, `assets/character/visual.ron` новый)

```rust
pub const CHARACTER_VISUAL_CONFIG: &str = "character/visual.ron";

#[derive(Resource, Deserialize, Clone)]
#[serde(deny_unknown_fields)]
pub struct CharacterVisualConfig {
    pub(super) model: String,          // asset path of the glTF model
    pub(super) model_height: f32,      // head top of the base head, model units (feet at 0)
    pub(super) height: f32,            // target body height, m
    pub(super) tinted_mesh: String,    // glTF mesh whose material gets `tint`
    pub(super) tint: (f32, f32, f32),  // linear multiplier of the base colour
    pub(super) blend_seconds: f32,     // cross-fade between states, s
    pub(super) idle: String, pub(super) jump: String, pub(super) fall: String,
    pub(super) walk: LocomotionClip, pub(super) run: LocomotionClip, pub(super) sprint: LocomotionClip,
}
#[derive(Deserialize, Clone)] #[serde(deny_unknown_fields)]
pub(super) struct LocomotionClip { clip: String, native_speed: f32 } // model units/s at rate 1
```
Методы: `scale() = height / model_height`; `validate()` (конечные положительные `model_height, height,
native_speed`, `blend_seconds >= 0`, тинт конечный и ≥ 0) в стиле `config.rs:160-178`;
`clip_names_by_state() -> [(AnimState, &str); 6]`; `pub fn resolve(&self, manifest:
&ThirdPartyManifest) -> Result<CharacterClips, Vec<String>>` — `rig_for(model)` (нет → ошибка "model …
has no rig in manifest"), `tinted_mesh ∈ rig.skinned_meshes`, каждое имя клипа → `clip_index` (нет →
ошибка с именем клипа и файла). `CharacterClips([usize; 6])` — `Resource`, индексы по порядку `AnimState`.

`assets/character/visual.ron`:
```ron
(
    model: "third_party/mini-characters/character-male-a.glb",
    model_height: 0.67132,
    height: 1.8,
    tinted_mesh: "body-mesh",
    tint: (1.0, 1.0, 1.0),
    blend_seconds: 0.15,
    idle: "idle",
    jump: "jump",
    fall: "fall",
    walk: (clip: "walk", native_speed: 1.28),
    run: (clip: "sprint", native_speed: 2.66),
    sprint: (clip: "sprint", native_speed: 2.66),
)
```
Откуда числа: 0.67132 — `max_y` `male-a` (`audit_rig.txt`, база без причёски: у `male-f/female-f` то же);
1.8 — GDD §9.2 и совпадает с верхом капсулы (1.05 + 0.75); `native_speed` — расчёт 2.4; 0.15 с — время
разгона до бега из `locomotion.ron` (переход не дольше смены скорости).

### Шаг 9. Клиент: плагин персонажа (`src/visuals/character.rs` новый, ~200 строк)

- `pub struct CharacterVisualsPlugin;` `build`: `register_type::<CharacterModel>()`,
  `Startup: build_character_graph`, `Update: drive_character_animation`, `add_observer(spawn_character_model)`.
- `const MODEL_YAW: f32 = std::f32::consts::PI; // glTF faces +Z, Bevy bodies face -Z` — закон.
- `#[derive(Resource)] struct CharacterAnimations { graph: Handle<AnimationGraph>, nodes: [AnimationNodeIndex; 6] }`
- `build_character_graph(commands, config, clips: Res<CharacterClips>, asset_server, mut graphs)`:
  `let mut graph = AnimationGraph::new();` для каждого индекса `graph.add_clip(asset_server.load(
  GltfAssetLabel::Animation(i).from_asset(config.model.clone())), 1.0, graph.root)`; `graphs.add(graph)`.
- `#[derive(Component, Reflect)] #[reflect(Component)] pub struct CharacterModel;` (QA находит модель по BRP).
- `spawn_character_model(event: On<Add, CharacterBody>, bodies, config, asset_server, commands)`: child
  `(CharacterModel, WorldAssetRoot(asset_server.load(GltfAssetLabel::Scene(0).from_asset(model))),
  model_transform(body.float_height, config.scale()))` + `.observe(on_model_ready)`; на тело
  `Visibility::default()` как сейчас.
- `pub(super) fn model_transform(float_height, scale) -> Transform` = `from_xyz(0, −float_height, 0)
  .with_rotation(Quat::from_rotation_y(MODEL_YAW)).with_scale(Vec3::splat(scale))`.
- `#[derive(Component)] struct CharacterAnimator { character: Entity, shown: AnimState }`.
- `on_model_ready(ready: On<WorldInstanceReady>, parents: Query<&ChildOf>, children: Query<&Children>,
  mut players: Query<&mut AnimationPlayer>, meshes: Query<(&GltfMeshName, &MeshMaterial3d<StandardMaterial>)>,
  animations: Res<CharacterAnimations>, config, mut materials, commands)`:
  персонаж = `ChildOf` модели (`ready.entity`); по `children.iter_descendants(ready.entity)`:
  у `AnimationPlayer` → `AnimationTransitions::new()`, `transitions.play(&mut player, nodes[Idle],
  Duration::ZERO).repeat()`, вставить `(AnimationGraphHandle(graph), transitions, CharacterAnimator {
  character, shown: Idle })`; у меша с `GltfMeshName == tinted_mesh` → `let Some(base) =
  materials.get(&handle).cloned() else { warn!(..); continue };` `base.base_color = Color::linear_rgb(tint)`
  (множитель на белый `base_color` glTF), вставить `MeshMaterial3d(materials.add(base))`. Если тинт
  `(1,1,1)` — материал не клонировать (ранний `continue`).
- `drive_character_animation(characters: Query<(&AnimState, &LinearVelocity)>, mut animators: Query<(&mut
  CharacterAnimator, &mut AnimationPlayer, &mut AnimationTransitions)>, animations, config)`:
  `let Ok((state, velocity)) = characters.get(animator.character) else { continue };`
  если `*state != animator.shown` → `transitions.play(&mut player, nodes[*state as usize],
  Duration::from_secs_f32(config.blend_seconds)).repeat(); animator.shown = *state;`
  затем `let Some(active) = player.animation_mut(nodes[*state as usize]) else { continue };
  active.set_speed(playback_rate(*state, horizontal(velocity), &config));`
- `pub(super) fn playback_rate(state: AnimState, horizontal_speed: f32, config: &CharacterVisualConfig) -> f32`:
  `Walk/Run/Sprint` → `h / (clip.native_speed * config.scale())`, прочие → 1.0.
- `src/visuals/mod.rs`: `mod character; mod character_config; pub use character_config::{...};`,
  добавить `character::CharacterVisualsPlugin` в `add_plugins`, **удалить** `visualize_character`
  (`:85-116`) и его `add_observer` (`:41`) — заглушка T1 заменена моделью; неиспользуемый импорт
  `CharacterBody` из `mod.rs` убрать.

Тесты в `character.rs` (`#[cfg(test)]`, `cargo test -p gta_like --bin gta_like`):
- `model_faces_body_forward` — три направленных примера (формула GDD §3.2, `R_y(θ)·(x,y,z) =
  (x·cosθ + z·sinθ, y, −x·sinθ + z·cosθ)`): лицо модели = `+Z` в её пространстве.
  1. Тело yaw 0: `R_y(0)·R_y(π)·(0,0,1)` = `R_y(0)·(0,0,−1)` = `(0,0,−1)` = `move_direction(W, 0)`.
  2. Тело yaw 90°: `R_y(90°)·(0,0,−1)` = `(−1·sin90, 0, −cos90)` = `(−1,0,0)` = `move_direction(W, 90°)`.
  3. Тело yaw 180°: `R_y(180°)·(0,0,−1)` = `(0,0,1)` = `move_direction(W, 180°)`.
  Тест: `(Quat::from_rotation_y(yaw) * model_transform(1.05, 2.68).rotation * Vec3::Z)` ≈
  `gta_sim::character::move_direction(Vec2::Y, yaw)` для yaw 0/90/180. Ловит забытый или лишний `π`
  (ошибка знака, которую тест "на модуль" не видит).
- `model_feet_on_ground` — `model_transform(1.05, s).translation.y == −1.05`.
- `playback_rate_worked_example` — конфиг из шипованного `visual.ron`: Walk при 1.8 м/с → `1.8 / (1.28 ·
  1.8/0.67132)` = 1.8 / 3.4321 = 0.5245 (±1e-3); Sprint при 6.8 → 6.8 / (2.66 · 2.6813) = 0.9534;
  Idle → 1.0.
- `character_visuals_reference_manifest_rig` (по образцу `city_gate.rs:186-197`): шипованные
  `visual.ron` + манифест → `validate()` ok и `resolve()` ok, индексы для
  `[Idle, Walk, Run, Sprint, Jump, Fall]` равны `[1, 2, 3, 3, 4, 5]` (клипы `idle, walk, sprint, sprint,
  jump, fall`, порядок из `audit_rig.txt`).
  Размещение: рядом с `city_gate.rs` в новом `src/visuals/character_gate.rs` (`#[cfg(test)] mod`), чтобы
  `character.rs` не рос.

### Шаг 10. Клиент: сборка (`src/main.rs`)

- Загрузить `CharacterVisualConfig` (`load_config`, как `render_config` `:111-117`).
- `preflight` (`:45-80`) получает `&CharacterVisualConfig` и возвращает `Result<CharacterClips,
  Vec<String>>`: `character.validate()` с путём `visual.ron` в сообщении; `model` должен пройти
  `manifest.contains_asset` (то же сообщение, что у пропов); `character.resolve(&manifest)?`; дальше как
  сейчас `missing_files`. Ноль новых проверок вне этих.
- `app.insert_resource(character_config).insert_resource(clips)` до `add_plugins((.., VisualsPlugin, ..))`.
- Файл вырастет на ~20 строк (сейчас 138).

### Шаг 11. QA-сценарий `tools/qa/scenarios/t4.py` (новый, по образцу `t3.py`)

1. `fetch_assets.py --check` должен пройти (иначе "run python tools/fetch_assets.py first").
2. `Game(features=("dev",), args=("--seed", "1"))`, `wait_resource("CityLayoutHash", 180)`, дождаться
   строки `Player`.
3. Проверить модель: `world.query` по `CharacterModel` → ровно 1; `world.query` по компонентам
   `AnimationPlayer` + `AnimationGraphHandle` → ≥ 1 (граф подключён). Пути через `component_path`.
4. Телепорт на `CityLandmarks.park_center` + 1.2 м (функции `resource_value`, `teleport` — скопировать
   из `t3.py`, общий модуль не заводим: два сценария, YAGNI), `OrbitCamera.yaw = 0`, `pitch = −0.2`,
   пауза 2 с, `AnimState` = `"Idle"`.
5. Чтение `AnimState`: `game.call("world.get_components", {"entity": player, "components": [path],
   "strict": True})` → значение enum (строка `"Run"`); принять и строгую форму `{path: value}`, и
   `{"components": {...}}`.
6. Фазы (между фазами ждать `Idle`, таймаут 3 с):
   - `W` 1600 мс; `W+ShiftLeft` 1600 мс; `AltLeft+W` 1600 мс (ходьба, дешёвая дополнительная фаза);
     `Space` 150 мс с наблюдением 1500 мс.
   - Внутри фазы: `AnimState` каждые 50 мс (метка времени от старта фазы), скриншот каждые 200 мс
     (`brp_extras/screenshot` без ожидания файла; файлы проверить на PNG-магию в конце).
7. Критерии (пороги выведены из данных: разгон 0.15 с, coyote 0.12 с, блендинг — чистая презентация):
   `W` — в окне 400..1500 мс все отсчёты `Run`; `W+Shift` — все `Sprint`; `Alt+W` — все `Walk`;
   `Space` — есть `Jump`, после него `Fall`, в конце `Idle`. Нарушение → `AssertionError` с выборкой.
8. `log_errors` как в `t3.py` (слова `gltf`, `asset`, `Failed to load`, `animation`) → провал при
   ошибках. Сырые `AnimationPlayer` (активные узлы, speed) записать в `summary.json` как доказательство
   для QA, без ассертов. Скриншоты и выборки — в `--out` (по умолчанию `target/qa/t4`).

### Шаг 12. GDD: фактическая правка чисел (`docs/design/GDD.md`) — только если оркестратор подтвердит (Open question 1)

- §9.2 (`:407`): "12 skinned-персонажей (male/female a-f), 14 костей, 32 клипа" → "12 skinned-персонажей
  (male/female a-f), 7 суставов (два skin на один скелет), 32 клипа (клипа run нет); факты записаны в
  `rig` манифеста".
- §13 T4 (`:589`): "перепроверить 12 персонажей / 14 костей / 32 клипа" → "записать в `rig` манифеста
  реальный состав архива".
Правка фактическая, скоуп не меняет.

### Шаг 13. Документация по окончании (оркестратор, после QA)

README/AGENTS "Статус", `docs/narrative-graph.md` — штатная синхронизация пайплайна, не шаг имплементера.

---

## 4. Test plan

### 4.1 Команды (все должны быть зелёными)
```
python tools/fetch_assets.py --cache maw/tasks/in_progress/TASK-005/scratch
python tools/fetch_assets.py --check
cargo build
cargo clippy -- -D warnings
cargo clippy -p gta_sim --tests -- -D warnings
cargo test -p gta_sim
cargo test -p gta_like --bin gta_like
python tools/qa/tree_check.py        # bevy_render не утёк в gta_sim (новых зависимостей нет, контроль)
python tools/qa/scenarios/t4.py --out target/qa/t4   # QA-стадия
```
`-p citygen` не затронут.

### 4.2 Flip-RED (испортить → RED → вернуть → GREEN; записать в IMPL_SUMMARY, что портили)
1. `anim_state`: заменить `velocity.y > 0.0` на `>= 0.0` → RED строка "апекс = Fall"; взять 3D-длину
   вместо горизонтальной → RED строка `(0, 3.0, −2.5)`.
2. `update_anim_state` не регистрировать в плагине → RED `tests/anim_state.rs` (все три теста).
3. В манифесте удалить клип `"crouch"` из `rig.clips` → RED `shipped_manifest_is_valid` (32) **и**
   `fetch_assets.py --check` (клипы расходятся с GLB). Отдельно поменять местами `"walk"` и `"sprint"`
   → `shipped_manifest_is_valid` RED по `clip_index`, `--check` RED по порядку.
4. `fetch_assets.py`: в `rig.joints` заменить `"head"` на `"neck"` → `--check` RED (сообщение называет
   модель и поле `joints`); Rust остаётся зелёным (схема цела) — это и есть граница ответственности.
5. `bad_rig_model.ron`: временно вернуть модель в `files` → тест фикстур RED ("must fail validate").
6. Клиент: убрать `MODEL_YAW` (0 вместо π) → RED `model_faces_body_forward` на всех трёх yaw.
7. `visual.ron`: `run: (clip: "run", …)` → RED `character_visuals_reference_manifest_rig` и отказ
   `preflight` с именем клипа.

### 4.3 Owner checklist (QA переносит в QA_REPORT.md)
- `python tools/fetch_assets.py` (или с `--cache`), `cargo run --features fast`.
- Вместо капсулы стоит человечек Kenney ростом примерно с верх бывшей капсулы (1.8 м), ступни на земле,
  не утоплены и не висят.
- W: бег, модель смотрит по ходу движения (не спиной вперёд), ноги заметно не скользят.
- Shift+W: спринт, ноги не скользят. Alt+W: ходьба, ноги не скользят.
- Space: поза прыжка на взлёте, поза падения на спуске, приземление в покой. Сход с бордюра — падение
  после короткой задержки (coyote 0.12 с) — нормально.
- Переходы между состояниями без рывков (0.15 с, `blend_seconds`).
- Тинт: поставить в `assets/character/visual.ron` `tint: (1.0, 0.35, 0.35)`, перезапустить: одежда
  краснеет (кисти тоже, это ограничение палитры); вернуть `(1.0, 1.0, 1.0)`.
- Крутилки, если ноги едут: `walk/run/sprint.native_speed` (меньше → ноги быстрее), `run.clip`
  (`"sprint"` или `"walk"`), `anim_idle_speed` в `locomotion.ron`.

---

## 5. Risk areas

1. **Совпадение `native_speed` с глазом.** Расчёт по ключам клипа (L·ω в момент касания) — модель жёсткого
   маятника; мультяшные клипы Kenney могут "читаться" иначе. Смягчение: значения в данных, диапазон для
   подбора указан (0.92..1.28 и 1.51..2.66 ед/с), решает владелец.
2. **Подскок `root` в `sprint` 0.2 ед. = 0.54 м при масштабе 2.68.** На Run (rate 0.63) это медленные
   высокие прыжки; может выглядеть "плавающе". Смягчение: `run.clip = "walk"` одной строкой данных.
3. **Тинт окрашивает кисти и шею** (`body-mesh` ходит в тексели кожи общей палитры). Для T4 приемлемо;
   для банд T9 может понадобиться палитра-маска — решение по факту прогона (Open question 2).
4. **Модель спавнится до готовности графа?** Нет: граф собирается в `Startup` синхронно из хэндлов меток,
   `WorldInstanceReady` приходит позже (после загрузки GLB). Если `CharacterAnimations` всё же нет —
   `on_model_ready` берёт его как `Res` и observer паникнет; имплементеру держать `build_character_graph`
   в `Startup`, а спавн игрока — `OnEnter(Playing)` (он и так после загрузки города).
5. **`AnimationTransitions::play` перезапускает клип** (`player.start`), переход Run↔Sprint на одном клипе
   начнёт цикл с нуля, но через кроссфейд 0.15 с. Узлы раздельные, поэтому смешение работает. Если
   владелец увидит "дёрг" — отдельная задача (синхронизация фазы), не блокер.
6. **`is_airborne` Tnua и coyote.** Сход с уступа даёт `Fall` через 0.12 с, на ступенях лестницы площадки
   `Fall` не мигает. Если на крутой рампе 30° спуск бегом дает кратковременный `Fall` — это видно
   владельцу, порога не добавляем без прогона.
7. **Материал ещё не загружен в `on_model_ready`.** Материал — метка того же GLB, загружается вместе со
   сценой; на всякий случай `let-else` + `warn!` без паники, тинт просто не применится (видно владельцу).
8. **`local_assets_match_manifest` покраснеет у всех, у кого стоят только city-пакеты**, пока не запущен
   `fetch_assets.py` ("partial install"). Это задуманное поведение T3; имплементер и QA ставят пакет из
   `--cache`, сеть не нужна.
9. **Python и Rust валидаторы разъедутся** (зеркало руками). Смягчение: одни и те же фикстуры прогоняются
   через `--validate-only` (шаг 4, проверка), слово `rig` в обоих.
10. **Имена BRP-путей.** `AnimState` должен находиться `component_path("AnimState")` однозначно: имя
    уникально в проекте (grep пуст), `bevy_tnua` своего `AnimState` не экспортирует (у него
    `TnuaAnimatingState`).
11. **Кодировка файлов.** `.gitattributes` `eol=lf`; фикстуры и RON читаются `ron`/Python без BOM —
    писать UTF-8 без BOM.

---

## 6. Open questions

Все с дефолтом, работу не блокируют.

1. **Править ли числа в GDD (шаг 12)?** Варианты: (а) да, две строки фактической правки (§9.2, §13 T4),
   скоуп не меняется; (б) нет, факты живут только в манифесте, GDD остаётся с устаревшими "14 костей".
   Рекомендация: (а) — иначе следующий планировщик (T7/T8) снова споткнётся о "14 костей" и "клип run".
2. **Как красить одежду банд/полиции (задел для T9)?** (а) множитель на `body-mesh` (этот план): дёшево,
   кисти тоже окрашиваются; (б) своя палитра-текстура на фракцию (перекрасить тексели одежды в
   `colormap.png` скриптом): чисто, но контент на каждый вариант a-f; (в) только выбор вариантов a-f без
   тинта. Рекомендация: (а) в T4, пересмотр по прогону владельца в T9.
3. **Какой клип у бега (`Run`, 4.5 м/с)?** (а) `sprint` ×0.63 — реалистичный каденс, но высокий подскок;
   (б) `walk` ×1.31 — ниже подскок, но семенящий шаг. Рекомендация: (а) по умолчанию, переключается
   одной строкой `visual.ron`, решает прогон владельца.
4. **Модель игрока.** Дефолт `character-male-a` (база без причёски, от неё посчитан масштаб). Другой вариант
   — одна строка `model` в `visual.ron`; у вариантов с причёской фигура та же, волосы выше 1.8 м.

children: 0 launched / 0 reported.
