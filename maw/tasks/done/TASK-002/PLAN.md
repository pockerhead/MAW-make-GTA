# PLAN — TASK-002 (GDD T1): каркас, персонаж под камерой, agent QA

Цена ошибки: смешанная. Молчаливые дефекты здесь есть и получают гейты: утечка `bevy_render` в
`gta_sim` (граница headless ломается незаметно), знак yaw в формуле движения (тест "на модуль" его
не ловит), нестрогий RON (опечатка в поле молча даёт дефолт), дрейф версий (`image`, `bevy_egui`).
Feel бега, прыжка и камеры, проход камеры сквозь стену владелец видит на первом кадре: это его
прогон, гейтов под это не строим.

Все числа ниже, помеченные "проба", получены исполняемой пробой планировщика
(`scratch/probe/src/main.rs`, headless, `MinimalPlugins` + avian 0.7.0 + Tnua 0.32.0), а не из головы.

---

## 1. Understanding

### Что есть сейчас
Кода нет. В репо: `docs/design/GDD.md` (APPROVED), `AGENTS.md`, `README.md`, `.gitignore`
(`/target/`, `*.png` и др. игнорируются; `Cargo.lock` не игнорируется), `.gitattributes`
(`* text=auto eol=lf`), `.mcp.json` (ссылается на `bevy_brp_mcp`, ставится отдельно). Тестов нет,
"existing tests pass" выполняется тривиально.

### Что требует GDD для T1 (прочитано целиком)
- §13 T1: цель, 6 headless-гейтов, прогон владельца, BRP-сценарий (строки 570-574).
- §3.1 (стр. 117-130): ходьба 1.8, бег 4.5, спринт 6.8 м/с, разгон 0.15 с, поворот 720°/с, прыжок
  1.0 м, coyote 0.12 с, буфер 0.1 с. Tnua-конфиг это `Asset`; в RON лежит наша строгая
  `LocomotionConfig`, из неё строится конфиг Tnua.
- §3.2 (стр. 132-157): камера только из `camera.ron`, геометрия pivot 1.55 / плечо 0.45 /
  дистанция 3.8 / FOV 70 / pitch −70…+60 / 0.12°/count / сглаживание half-life 0.05 / коллизия
  сферой 0.25 м, притягивание мгновенно, отпускание half-life 0.25. Законы: Y вверх, вперёд −Z,
  +yaw против часовой сверху, мышь вправо уменьшает yaw, `world = R_y(yaw)·(x, 0, −y)`, три
  проверочных примера. Прицельные значения (2.0 м, 55°, 0.55, ×0.7, 0.15 с) это T6.
- §3.3: ввод только в клиенте (BEI), геймплей видит intent-компоненты; `Update` копит edge,
  `FixedUpdate` потребляет.
- §10.1/10.2/10.5/§12/Rollout: пины `=`, три крейта, `gta_sim` без рендера, `compose_sim`,
  `ConfigRoot`, фичи `dev`/`debug`/`profile`/`profile-tracy`, `bevy_settings` в клиенте,
  `BrpExtrasPlugin` только под `dev`, reflect для типов, которые читает QA, `tools/qa/brp.py`
  (stdlib), `tools/qa/scenarios/t1.py`, проверка дерева.

### Проверено по исходникам пиненных версий (не по памяти)
| Факт | Где |
|---|---|
| `TnuaControllerPlugin::<S>::new(schedule)` делает `init_asset::<S::Config>()` и цепочку `Sensors → TnuaUserControlsSystems → Logic → Motors` в том же расписании | `bevy-tnua-0.32.0/src/controller.rs:50-76` |
| "DO NOT mix Update with FixedUpdate" для Tnua | `controller.rs:39-40` |
| `desired_velocity = desired_motion * config.speed`, разгон линейный, от покоя ×1.5 на первом тике | `builtins/walk.rs:315-332` |
| `TnuaBuiltinJumpConfig::height` = подъём центра тела от float_height до апекса; дефолт `takeoff_extra_gravity = 30` | `builtins/jump.rs:49-59,137-153` |
| `#[derive(TnuaScheme)]` на `enum CharacterScheme { Jump(TnuaBuiltinJump) }` генерирует `CharacterSchemeConfig { basis, jump }` | пример `examples/example.rs`, проба компилируется |
| **`bevy-tnua-avian3d 0.12.1` включает `avian3d` фичу `debug-plugin` безусловно** → `bevy/bevy_gizmos`, `bevy/bevy_render` | `bevy-tnua-avian3d-0.12.1/Cargo.toml` (`features = ["3d","debug-plugin","parallel"]`), `avian3d-0.7.0/Cargo.toml [features] debug-plugin` |
| Код `bevy-tnua-avian3d` не использует ничего из debug-plugin (grep `debug|gizmo|render` пуст), без фичи компилируется | проба |
| avian: `PhysicsPlugins` по умолчанию в `FixedPostUpdate`; `ColliderTreeDiagnostics`/`SpatialQueryDiagnostics` регистрируются в `Plugin::finish` | `avian3d-0.7.0/src/lib.rs:757-788`, `collider_tree/mod.rs:96-98`, `spatial_query/mod.rs:221-223` |
| `SpatialQuery::cast_shape(&Collider, origin, rot, Dir3, &ShapeCastConfig, &SpatialQueryFilter) -> Option<ShapeHitData>`; `ShapeCastConfig::from_max_distance`; `SpatialQueryFilter::from_excluded_entities` | `spatial_query/system_param.rs:446`, `shape_caster.rs:457`, `query_filter.rs:71` |
| `TransformInterpolation` (реэкспорт `bevy_transform_interpolation`) в группе `PhysicsPlugins` | `interpolation.rs:9-14,183` |
| `PhysicsDebugPlugin` (unit struct), gizmo-группа `PhysicsGizmos` | `debug_render/mod.rs:90-97` |
| `MinimalPlugins` = TaskPool + FrameCount + Time + ScheduleRunner (нет Asset/Transform/States) | `bevy_internal-0.19.1/src/default_plugins.rs:163-170` |
| `TimeUpdateStrategy::FixedTimesteps(n)` | `bevy_time-0.19.1/src/lib.rs:120-122,181-183` |
| `FileAssetReader::get_base_path()`: `BEVY_ASSET_ROOT` → `CARGO_MANIFEST_DIR` → каталог exe | `bevy_asset-0.19.1/src/io/file/mod.rs:19-29,56` |
| `CursorOptions` это `Component` окна; при неудаче grab bevy_winit **откатывает** `grab_mode` к кэшу | `bevy_window-0.19.1/src/window.rs:740-752`, `bevy_winit-0.19.1/src/system.rs:611-621` |
| `StableInterpolate::smooth_nudge(target, decay_rate, dt)` = `1 − exp(−rate·dt)` | `bevy_math-0.19.1/src/common_traits.rs:467` |
| `TransformSystems::Propagate` | `bevy_transform-0.19.1/src/plugins.rs:13` |
| `AppExit::error()`, `impl Termination for AppExit` | `bevy_app-0.19.1/src/app.rs:1572,1608` |
| BEI 0.26: `EnhancedInputPlugin`, `add_input_context::<C>()`, `actions!`, `Bindings::spawn(Cardinal::wasd_keys())`, `Binding::mouse_motion()`, pull-API `Action<A>` + `ActionEvents::STARTED`; читает `ButtonInput<KeyCode>` и `AccumulatedMouseMotion`; контекст в `PreUpdate` | `bevy_enhanced_input-0.26.0/src/lib.rs`, `preset.rs:55-75`, `binding.rs:122`, `context.rs:126`, `context/input_reader.rs:8,26` |
| BEI `Cardinal`: north = +Y (W), совпадает с "y вперёд" GDD | `preset/cardinal.rs:38` |
| brp_extras 0.22.6: `send_keys {keys, duration_ms}` возвращается сразу, отпускание по таймеру; `move_mouse {delta}` пишет `MouseMotion` + `CursorMoved`; `screenshot {path}` это **watching**-метод (SSE); `get_diagnostics` без параметров, `FrameTimeDiagnosticsPlugin` добавляется плагином сам; `shutdown` через несколько кадров | `bevy_brp_extras-0.22.6/src/lib.rs`, `keyboard/keys.rs:89-137`, `mouse/support.rs:156-171`, `plugin.rs:357-360,444`, `shutdown.rs:27-43` |
| BRP HTTP: watching-ответ `text/event-stream`, строки `data: {...}`; порт 15702, 127.0.0.1; `world.query {data:{components,option,has}, filter:{with,without}, strict}`; `world.list_components` без параметров = все reflect-компоненты | `bevy_remote-0.19.1/src/http.rs:52,60,369-373,451`, `builtin_methods.rs:152-165,383-423,1378-1404` |
| `bevy-inspector-egui 0.37.0`: `bevy_egui ^0.40.0`, нужен `EguiPlugin::default()` (реэкспорт `bevy_inspector_egui::bevy_egui`), `WorldInspectorPlugin::new().run_if(..)` | `bevy-inspector-egui-0.37.0/Cargo.toml:168`, `src/quick.rs:41-86` |

Строки `Cargo.toml` о поддержке Bevy (требование planner-роли):
- `avian3d 0.7.0`: `[dependencies.bevy] version = "0.19.0"` (`avian3d-0.7.0/Cargo.toml:323-324`).
- `bevy-tnua 0.32.0`: `[dependencies.bevy] version = "^0.19"` (`bevy-tnua-0.32.0/Cargo.toml:88-89`).
- `bevy-tnua-avian3d 0.12.1`: `bevy = { version = "^0.19" }`, `avian3d = { version = "^0.7" }` (`Cargo.toml.orig`).
- `bevy_enhanced_input 0.26.0`: `[dependencies.bevy] version = "0.19.0"` (`:56-57`).
- `bevy_brp_extras 0.22.6`: `[dependencies.bevy] version = "0.19.1"`, `image = "=0.25.9"` (`:73-74,119-120`).
- `bevy-inspector-egui 0.37.0`: `bevy_app = "0.19.0"`, `bevy_egui = "0.40.0"` (`:148-149,168-169`).

### Результаты пробы (`scratch/probe_*.txt`, `scratch/tree_*.txt`)
1. **Утечка рендера.** Со стоковым `bevy-tnua-avian3d 0.12.1` дерево
   `cargo tree -e normal -i bevy_render` для sim-крейта **не пусто** (`tree_render_unpatched.txt`,
   путь `avian3d feature "debug-plugin" ← bevy-tnua-avian3d`). С vendored-копией, где из
   манифеста убрана одна строка `"debug-plugin",`, дерево пусто и всё компилируется
   (`tree_render_patched.txt`). Полный workspace-манифест (клиент + sim + citygen, фичи dev,debug)
   резолвится: `cargo tree -p gta_sim -e normal -i bevy_render` пусто; `image` одна версия
   0.25.9; `bevy_egui` одна версия 0.40.1; дубликатов `bevy*`/`avian*`/`egui`/`image`/`wgpu`/`winit`
   нет, есть неустранимые чужие (`glam 0.32/0.33`, `hashbrown`, `windows-sys` и т.п.)
   (`tree_ws.txt`).
2. **Минимальный набор плагинов** для avian + Tnua: `MinimalPlugins` + `AssetPlugin` (без него
   паника "AssetServer does not exist"); `TransformPlugin` для плоских тел не обязателен
   (`probe_variant3.txt`), но берём его: он есть в `DefaultPlugins`, понадобится дочерним
   коллайдерам (голова T6) и держит тест ближе к игре.
3. **`app.finish(); app.cleanup();` обязательны** перед первым `app.update()`: без них паника
   `ResMut<ColliderTreeDiagnostics> ... Resource does not exist` (`probe_run1.txt`).
4. **Первый `app.update()` при `FixedTimesteps(1)` даёт 0 фиксированных тиков**, дальше ровно
   по одному (64 update → 63 тика). Тесты считают тики по `Time<Fixed>`, а не по вызовам update.
5. **Движение** (капсула r 0.3, высота 1.5, float 1.05; `acceleration = 4.5/0.15 = 30`): за 64 тика
   из покоя 4.2004 м = **0.9334·v_run** (коридор 0.8…1.0 проходит с запасом). yaw 0 → (0, −4.20),
   yaw 90° → (−4.20, 0), yaw 180° → (+4.20 по Z, x = 3.7e−7), yaw −90° → (+4.20, 0). Боковой
   дрейф < 1e−6 м.
6. **Прыжок.** С дефолтным Tnua `takeoff_extra_gravity = 30` подъём центра **1.165 м при
   height 1.0 (+16.5%)**: гейт ±10% из T1 падает. Свип (`probe_jump_sweep.txt`): 0 → 0.996,
   5 → 1.043, 10 → 1.063, 15 → 1.088, 20 → 1.128, 30 → 1.165. Tap (1 тик подачи) → 0.257 м.
7. **Рельеф**: ступени 0.2 × 0.3 проходятся (подъём 1.58 м на лестнице 1.6 м), рампа 30°
   проходится (`probe_terrain.txt`). Размеры капсулы и float 1.05 (зазор 0.3 м под капсулой)
   рабочие.

---

## 2. Approach

### 2.1 Три крейта, граница рендера как ошибка сборки
- Корень: `[workspace]` + пакет `gta_like` (бинарник, презентация, фичи).
- `crates/gta_sim`: весь геймплей; `bevy` с `default-features = false` и явными фичами; avian без
  `debug-plugin`; Tnua.
- `crates/citygen`: пустой lib-крейт (генератор это T2). Зависимостей нет (YAGNI).
- **Vendored `bevy-tnua-avian3d 0.12.1`** в `vendor/bevy-tnua-avian3d-0.12.1/` через
  `[patch.crates-io]`, единственное отличие от опубликованного архива: из списка фич `avian3d`
  убрана строка `"debug-plugin",`. Версия и код те же, что требует GDD §10.1; решение
  фиксируется ADR-001. Альтернативы и почему нет: ослабить гейт (6) — противоречит задаче и
  домену bevy-ecs; скопировать интеграцию (~450 строк) внутрь `gta_sim` — больше дрейфа и
  нарушение "крейт как в GDD"; сменить контроллер — GDD фиксирует Tnua. Вариант остаётся
  открытым вопросом с дефолтом (раздел 5).

### 2.2 Одна композиция
`gta_sim::compose_sim(app: &mut App, root: ConfigRoot) -> Result<(), ConfigError>`:
вставляет `ConfigRoot`, грузит `LocomotionConfig` строгим загрузчиком, вставляет ресурс, добавляет
`PhysicsPlugins::default()`, `TnuaAvian3dPlugin::new(FixedUpdate)` и доменные плагины
(`CharacterPlugin`, `PlayerPlugin`, `WorldPlugin`). Клиент: `DefaultPlugins` + `compose_sim` +
плагины презентации. Тест: `MinimalPlugins` + `TransformPlugin` + `AssetPlugin::default()` +
`compose_sim` + `TimeUpdateStrategy::FixedTimesteps(1)` + `finish()`/`cleanup()`. `compose_sim`
не добавляет `AssetPlugin`/`TransformPlugin` (их даёт `DefaultPlugins`, дубль плагина = паника).

Почему `Result`, а не паника в `Plugin::build`: ошибка конфига должна печататься с именем файла и
поля и завершать процесс кодом ошибки (`AppExit::error()`), а тест проверяет ту же функцию.

### 2.3 Строгий конфиг
`load_config::<T: DeserializeOwned>(root: &ConfigRoot, rel: &str) -> Result<T, ConfigError>`:
синхронно читает `root.join(rel)`, `ron::from_str`, ошибка `ConfigError { path, message }` с
`Display` вида `"{path}: {ron SpannedError}"` (ron 0.12.2 печатает
`"line:col: Unexpected field named `bogus_field` in `LocomotionConfig`, expected ..."`,
`ron-0.12.2/src/error.rs:118-121,238-243`). Все наши структуры конфигов с
`#[serde(deny_unknown_fields)]`. `ConfigRoot` указывает на каталог `assets/`. Клиент:
`ConfigRoot(FileAssetReader::get_base_path().join("assets"))` — тот же корень, что у `AssetPlugin`;
тесты: `Path::new(env!("CARGO_MANIFEST_DIR")).join("../../assets")`.

### 2.4 Intent → Tnua в `FixedUpdate`
- `MoveIntent` (компонент, reflect): `axis: Vec2` (x вправо, y вперёд), `yaw: f32` (рад, yaw
  камеры), `gait: Gait {Walk, Run(default), Sprint}`, `jump_held: bool` (уровень),
  `jump_requested: bool` (защёлка edge, ставит клиент в `Update`, снимает потребитель в
  `FixedUpdate`). Защёлка нужна, потому что тап короче интервала между тиками при высоком FPS
  иначе теряется (Bevy issue #6183, раздел "Источники").
- `move_direction(axis, yaw) = Quat::from_rotation_y(yaw) * Vec3::new(a.x, 0, −a.y)`,
  `a = axis.clamp_length_max(1.0)` — формула GDD §3.2 как чистая функция.
- Система `drive_characters` в `FixedUpdate`, `.in_set(TnuaUserControlsSystems)` (Tnua ставит
  этот сет между Sensors и Logic в том же расписании): `initiate_action_feeding()`,
  `basis = TnuaBuiltinWalk { desired_motion: dir * speed(gait), desired_forward: Dir3::new(dir).ok() }`,
  прыжок кормится при `jump_held || jump_requested`, затем `jump_requested = false`.
- Tnua `speed = 1.0` (соглашение: `desired_motion` в м/с), `acceleration = run_speed /
  time_to_run_speed`, `turning_angvel = turn_rate_deg.to_radians()`, `coyote_time`, прыжок:
  `height`, `input_buffer_time`, `takeoff_extra_gravity`. Прочие поля Tnua остаются его
  дефолтами (`..Default::default()`); новые числа в коде не появляются.
- Tnua-конфиг кладётся в `Assets<CharacterSchemeConfig>` один раз: ресурс
  `CharacterControlConfig(Handle<CharacterSchemeConfig>)` с `impl FromWorld` (берёт
  `LocomotionConfig`, добавляет asset), `app.init_resource` в `CharacterPlugin::build` после
  `TnuaControllerPlugin` (у которого `init_asset` создаёт `Assets<_>` сразу).
- Капсула: `RigidBody::Dynamic`, `Collider::capsule(radius, capsule_height − 2·radius)`,
  `LockedAxes::ROTATION_LOCKED.unlock_rotation_y()` (поворот 720°/с к направлению движения),
  `TnuaAvian3dSensorShape(Collider::cylinder(radius − SENSOR_INSET, 0.0))`.
- Производное состояние для презентации: `CharacterBody { radius, height, float_height }`
  (reflect) на каждом персонаже. Клиент строит по нему меш и вычисляет "ноги" для pivot камеры и
  не открывает `locomotion.ron` (GDD §12 "потребители получают производное состояние").

Проверка знаков (3 примера, `R_y(θ)(x,y,z) = (x cosθ + z sinθ, y, −x sinθ + z cosθ)`):
yaw 0: W → (0,0,−1), D → (1,0,0); yaw 90°: W → (−1,0,0), D → (0,0,−1); yaw 180°: W → (0,0,1),
D → (−1,0,0). Проба подтвердила для W на всех трёх yaw.

### 2.5 Камера (клиент, `Update`/`PostUpdate`)
Компонент `OrbitCamera { yaw, pitch, distance, pivot: Option<Vec3> }` (reflect: BRP читает yaw).
- Взгляд: `yaw −= dx·sens`, `pitch −= dy·sens`, clamp pitch в [−70°, +60°]. Мышь вправо (dx > 0)
  уменьшает yaw — закон GDD. Мышь вниз (dy > 0) опускает взгляд.
- Поворот камеры `rot = Quat::from_euler(EulerRot::YXZ, yaw, pitch, 0)`; `back = rot·(0,0,1)`;
  камера в `pivot + back·d` с `rotation = rot` смотрит ровно на pivot.
  Примеры: yaw 0, pitch 0: back (0,0,1), камера позади игрока, бегущего в −Z; yaw 90°: back
  (1,0,0), взгляд −X (совпадает с W при yaw 90° из 2.4); yaw 180°: back (0,0,−1), взгляд +Z;
  pitch +30°: `R_x(30)·(0,0,1) = (0, −0.5, 0.866)`, камера ниже pivot, смотрит вверх.
- Pivot: `feet = player.translation − Y·float_height`; `head = feet + Y·pivot_height`;
  сглаживание `smooth_nudge(head, ln2/follow_half_life, dt)` (half-life, независимо от FPS;
  Rory Driscoll, см. Источники); первый кадр — снап.
- Коллизия в два отрезка (стандартный spring-arm: сфера, исключить своего игрока, см. Источники):
  (1) сфера r 0.25 от сглаженной головы вправо на `shoulder_offset` → плечо укорачивается у
  стены; (2) сфера от плеча по `back` на `distance` → целевая дистанция. Если цель меньше
  текущей — мгновенно, иначе `smooth_nudge` с half-life 0.25. Фильтр
  `SpatialQueryFilter::from_excluded_entities([player])`. Слои коллизий (закон GDD §12) в T1 не
  нужны никому, заводятся, когда появится первый потребитель (T6 `Hitbox`/T8 NPC).
- Плавность при 64 Гц физики и 144 Гц экрана: клиент вешает `TransformInterpolation` на игрока
  (observer `On<Add, Player>`), камера читает интерполированный `Transform`; камера работает в
  `PostUpdate.before(TransformSystems::Propagate)`. Headless-тесты читают `Position`, на них это
  не влияет.

### 2.6 Курсор
Свой ресурс `CursorCaptured(bool)` (старт `true`), взгляд работает только при `true`. Esc →
`false` + `CursorOptions { grab_mode: None, visible: true }`; ЛКМ при `false` → `true` + `Locked`,
`visible: false`. Гейтить по `CursorOptions.grab_mode` нельзя: при неудачном grab (окно без
фокуса, т.е. ровно режим BRP-QA) bevy_winit откатывает поле к старому значению. Esc/ЛКМ читаются
через `ButtonInput` в `Update` (управление окном, не геймплей; ЛКМ в T6 станет огнём в BEI).

### 2.7 BRP-контур
- `src/remote/` под `#[cfg(feature = "dev")]`: `BrpExtrasPlugin::default()` (сам добавит
  `RemotePlugin`, `RemoteHttpPlugin`, `FrameTimeDiagnosticsPlugin`).
- `tools/qa/brp.py` (stdlib): сборка, запуск exe напрямую (не `cargo run`: на Windows убийство
  `cargo.exe` не убивает игру), JSON-RPC, SSE для скриншота, гарантированное убийство по таймауту.
- `tools/qa/scenarios/t1.py`: сценарий из §13.
- `tools/qa/tree_check.py`: проверка дерева как исполняемый гейт (дрейф зависимостей ломает
  молча).

---

## 3. Steps

Порядок: сначала headless-скелет с гейтами (он же исполняемая проба в реальном дереве, GDD
"проба первым шагом"; планировщик уже прогнал её в `scratch/probe`), потом клиент, потом QA.
Отдельный `target/` вне корня не создавать.

### Шаг 1. Workspace, пины, vendored-патч
**Файлы:** `/Cargo.toml`, `/Cargo.lock`, `/src/main.rs` (заглушка до шага 6),
`/crates/gta_sim/{Cargo.toml,src/lib.rs}`, `/crates/citygen/{Cargo.toml,src/lib.rs}`,
`/vendor/bevy-tnua-avian3d-0.12.1/`, `/docs/decisions/ADR-001-vendored-tnua-avian3d.md`.

1. `/Cargo.toml` (эталон проверен резолвом в `scratch/probe_ws/Cargo.toml`):
   ```toml
   [workspace]
   members = ["crates/gta_sim", "crates/citygen"]
   exclude = ["vendor"]
   resolver = "3"

   [workspace.package]
   edition = "2024"
   rust-version = "1.95.0"

   [workspace.dependencies]
   bevy = { version = "=0.19.1", default-features = false }
   avian3d = { version = "=0.7.0", default-features = false, features = ["3d", "f32", "parry-f32", "xpbd_joints", "parallel"] }
   bevy-tnua = "=0.32.0"
   bevy-tnua-avian3d = "=0.12.1"
   ron = "=0.12.2"
   serde = { version = "1", features = ["derive"] }

   [patch.crates-io]
   bevy-tnua-avian3d = { path = "vendor/bevy-tnua-avian3d-0.12.1" }

   [package]
   name = "gta_like"  # version 0.1.0, edition/rust-version из workspace, publish = false

   [features]
   dev = ["bevy/bevy_remote", "bevy/png", "dep:bevy_brp_extras"]
   debug = ["dep:bevy-inspector-egui", "avian3d/debug-plugin"]
   profile = ["bevy/trace_chrome"]
   profile-tracy = ["bevy/trace_tracy"]

   [dependencies]
   gta_sim = { path = "crates/gta_sim" }
   bevy = { workspace = true, default-features = true, features = ["bevy_settings"] }
   avian3d = { workspace = true }
   bevy_enhanced_input = "=0.26.0"
   serde = { workspace = true }
   bevy_brp_extras = { version = "=0.22.6", optional = true }
   bevy-inspector-egui = { version = "=0.37.0", optional = true }
   ```
   `bevy_settings` включается по Rollout GDD ("в клиенте постоянно"), кода под него в T1 нет.
   Прямой зависимости на `bevy_egui` нет (ловушка GDD §10.1).
2. `crates/gta_sim/Cargo.toml`: `bevy = { workspace = true, features = ["std", "multi_threaded",
   "bevy_state", "bevy_log", "bevy_asset", "serialize", "reflect_auto_register"] }` (к списку GDD
   добавлен `bevy_asset`: sim пользуется `Assets`, сейчас он приходит только транзитивно из
   Tnua), `avian3d`, `bevy-tnua`, `bevy-tnua-avian3d`, `ron`, `serde` (все `workspace = true`).
   `[dev-dependencies] bevy = { workspace = true, features = ["debug"] }` — имена систем в паниках
   тестов (без этого "<Enable the debug feature to see the name>", проба); `-e normal` дерева это
   не затрагивает, рендер фича не тянет (проверено пробой).
3. `crates/citygen`: `Cargo.toml` без зависимостей, `src/lib.rs` с одной строкой `//!` о назначении
   крейта (GDD §2.2).
4. Vendored-крейт: скачать `https://static.crates.io/crates/bevy-tnua-avian3d/bevy-tnua-avian3d-0.12.1.crate`
   (или скопировать из `~/.cargo/registry/src/*/bevy-tnua-avian3d-0.12.1/` после `cargo fetch`),
   распаковать в `vendor/bevy-tnua-avian3d-0.12.1/` целиком, в `Cargo.toml` в
   `[dependencies.avian3d] features` удалить строку `"debug-plugin",`. Больше ничего не менять,
   чтобы `diff` с архивом был в одну строку.
5. ADR-001 (`docs/decisions/`): контекст (строки манифестов выше), решение, альтернативы,
   условие отмены (апстрим вынесет `debug-plugin` в фичу → удалить `vendor/` и `[patch]`), как
   обновлять (повторить п.4 для новой версии).
6. `cargo generate-lockfile` (или первый `cargo check`), `Cargo.lock` в git. `cargo update` не
   запускать.

Проверка шага: `cargo check --workspace` зелёный; `cargo tree -p gta_sim -e normal -i bevy_render`
печатает "nothing to print"; `cargo tree -i bevy-tnua-avian3d` показывает путь `vendor/`.

### Шаг 2. `config/`: строгий загрузчик
**Файл:** `crates/gta_sim/src/config/mod.rs` (~70 строк).
- `#[derive(Resource, Clone, Debug)] pub struct ConfigRoot(pub PathBuf)` + `fn path(&self, rel) -> PathBuf`.
- `#[derive(Debug)] pub struct ConfigError { pub path: PathBuf, pub message: String }` +
  `impl Display` (`"{}: {}"`, путь и сообщение), `impl std::error::Error`.
- `pub fn load_config<T: DeserializeOwned>(root: &ConfigRoot, rel: &str) -> Result<T, ConfigError>`:
  `fs::read_to_string` (ошибка IO → `ConfigError` с путём), `ron::from_str::<T>` (ошибка →
  `ConfigError { path, message: err.to_string() }`). Golden path: два `map_err` + `?`.

### Шаг 3. `character/`, `player/`, `world/`, `compose_sim`
**3a. `assets/character/locomotion.ron`** (единственный источник; комментарии с единицами):
```ron
(
    walk_speed: 1.8,                  // m/s, Alt
    run_speed: 4.5,                   // m/s, default gait
    sprint_speed: 6.8,                // m/s, Shift
    time_to_run_speed: 0.15,          // s from rest; acceleration = run_speed / this
    turn_rate_deg: 720.0,             // deg/s, body turns toward movement
    jump_height: 1.0,                 // m, body-centre rise at apex
    jump_takeoff_extra_gravity: 10.0, // m/s^2; Tnua default 30 overshoots jump_height by ~16% at 64 Hz
    coyote_time: 0.12,                // s
    jump_buffer: 0.1,                 // s
    capsule_radius: 0.3,              // m
    capsule_height: 1.5,              // m, total incl. hemispheres
    float_height: 1.05,               // m, body centre above ground; head top = 1.05 + 1.5/2 = 1.8
)
```
Числа GDD §3.1 плюс три новых значения (капсула, float, takeoff), обоснованных пробой. Все в
этом файле, `const` под них нет.

**3b. `crates/gta_sim/src/character/locomotion.rs`** (~80 строк): `LocomotionConfig`
(`Resource, Deserialize, Clone, Debug`, `#[serde(deny_unknown_fields)]`), поле в поле с RON;
`pub const LOCOMOTION_CONFIG: &str = "character/locomotion.ron"`; `fn speed(&self, Gait) -> f32`;
`fn tnua_config(&self) -> CharacterSchemeConfig` (маппинг из 2.4, `speed: 1.0` с однострочным
комментарием "desired_motion is in m/s").

**3c. `crates/gta_sim/src/character/intent.rs`** (~80 строк): `MoveIntent`, `Gait`
(`Reflect`, `#[reflect(Component, Default)]` / `#[reflect(Default)]`), `pub fn move_direction`.
Юнит-тесты `#[cfg(test)]`: шесть векторов из 2.4 (W и D на yaw 0/90/180), допуск 1e−5 — это
корректность формулы; интеграционные тесты шага 4 доказывают проводку.

**3d. `crates/gta_sim/src/character/mod.rs`** (~150 строк):
- `#[derive(TnuaScheme)] #[scheme(basis = TnuaBuiltinWalk)] pub enum CharacterScheme { Jump(TnuaBuiltinJump) }`.
- `#[derive(Component, Reflect)] #[reflect(Component)] #[require(MoveIntent)] pub struct Character;`
- `CharacterBody { radius, height, float_height }` (reflect, component).
- `CharacterControlConfig(Handle<CharacterSchemeConfig>)` + `impl FromWorld`.
- `pub fn character_components(cfg: &LocomotionConfig, handle: Handle<CharacterSchemeConfig>) -> impl Bundle`
  (всё из 2.4: `Character`, `CharacterBody`, `RigidBody::Dynamic`, collider, `LockedAxes`,
  `TnuaController::<CharacterScheme>::default()`, `TnuaConfig::<CharacterScheme>(handle)`, сенсор).
- `const SENSOR_INSET: f32 = 0.01;` с комментарием "Tnua: sensor slightly narrower than the
  capsule" — инженерный закон интеграции, не feel.
- `CharacterPlugin::build`: `add_plugins(TnuaControllerPlugin::<CharacterScheme>::new(FixedUpdate))`,
  `init_resource::<CharacterControlConfig>()`, `add_systems(FixedUpdate,
  drive_characters.in_set(TnuaUserControlsSystems))`.
- `drive_characters`: `Res<LocomotionConfig>`, `Query<(&mut MoveIntent, &mut TnuaController<CharacterScheme>), With<Character>>`.
  Снятие защёлки `if intent.jump_requested { intent.jump_requested = false; }`, чтобы не метить
  компонент изменённым каждый тик.

**3e. `crates/gta_sim/src/world/mod.rs` + `world/test_area.rs`** (~120 строк):
- `#[derive(Component, Reflect)] #[reflect(Component)] pub struct Block { pub size: Vec3 }` — описание
  статического бокса для презентации.
- `#[derive(Resource)] pub struct PlayerSpawn(pub Vec3)` — точка ног игрока.
- `WorldPlugin`: `insert_resource(PlayerSpawn(Vec3::ZERO))`, `Startup` система `spawn_test_area`:
  каждый элемент = `(Block { size }, RigidBody::Static, Collider::cuboid(size.x, size.y, size.z),
  Transform)`. Таблица уровня (все кубоиды, верх пола y = 0; радиус 8 м вокруг спавна свободен,
  чтобы тесты шага 4 бежали 4.2 м в −Z, −X, +Z без препятствий):
  - пол 80 × 1 × 80, центр (0, −0.5, 0);
  - коробки: 1×1×1 в (10, 0.5, 10); 2×1×2 в (13, 0.5, 10); 1.5×2×1.5 в (16, 1, 10);
  - рампа 30°: плита 4 × 0.4 × 10, `Quat::from_rotation_x(30°)` (поднимает −Z конец:
    `R_x(30)·(0,0,−1) = (0, 0.5, −0.866)`), x = −10, верхняя поверхность начинается вровень с полом
    у z = −12 и поднимается к z ≈ −20.66, y ≈ 5; площадка 4 × 5 × 4 в конце, верх y = 5;
  - лестница: 8 ступеней подъём 0.2, проступь 0.3, ширина 3, x = +10, от z = −12 в −Z; площадка
    3 × 1.6 × 6 за последней ступенью;
  - стена 12 × 4 × 0.5, центр (0, 2, 14) — для проверки владельцем "камера не проходит сквозь
    стену".
  Это геометрия тестового уровня, а не тюнинг: T2 заменит её городом. Реализатор выносит
  таблицу в один массив `const TEST_AREA: [(Vec3 size, Vec3 pos, f32 rot_x_deg); N]` с
  однострочным комментарием "level geometry, replaced by citygen in T2" (см. открытый вопрос Q2).

**3f. `crates/gta_sim/src/player/mod.rs`** (~40 строк): `#[derive(Component, Reflect)]
#[reflect(Component)] pub struct Player;`, `PlayerPlugin` со `Startup` системой `spawn_player`
`.after(spawn_test_area)`: `(Player, Name::new("Player"), Transform::from_translation(spawn.0 +
Vec3::Y * cfg.float_height), character_components(..))`.

**3g. `crates/gta_sim/src/lib.rs`** (~40 строк): `pub mod config; pub mod character; pub mod
player; pub mod world;` и `compose_sim` из 2.2. Порядок внутри: `insert_resource(root)` →
`load_config::<LocomotionConfig>` → `insert_resource` → `add_plugins((PhysicsPlugins::default(),
TnuaAvian3dPlugin::new(FixedUpdate), CharacterPlugin, WorldPlugin, PlayerPlugin))` → `Ok(())`.

### Шаг 4. Headless-гейты (`cargo test -p gta_sim`)
**4a. `crates/gta_sim/tests/common/mod.rs`**:
- `pub fn assets_root() -> ConfigRoot` — `CARGO_MANIFEST_DIR/../../assets`, `canonicalize`;
  если каталога нет → `panic!("GATE BROKEN: assets root not found at ...")` (падает на обвязке с
  явным сообщением, не на коде).
- `pub fn headless_app() -> App`: `MinimalPlugins`, `TransformPlugin`, `AssetPlugin::default()`,
  `insert_resource(TimeUpdateStrategy::FixedTimesteps(1))`, `compose_sim(&mut app,
  assets_root()).expect(..)`, `app.finish(); app.cleanup();`.
- `pub fn run_ticks(app, n)`: крутит `app.update()` до прироста `Time<Fixed>::elapsed()` на
  `n · timestep` (страховка: не более `n + 4` update, иначе panic "GATE BROKEN: fixed loop stalled").
- `pub fn player(app) -> Entity` (`Query<Entity, With<Player>>` single) и
  `pub fn settle(app)`: `run_ticks(32)`, затем проверка предусловия `|Position.y − float_height| < 0.05`,
  иначе `panic!("GATE BROKEN: player not resting on the floor at spawn ...")`.

**4b. `tests/movement.rs`** (liveness+correctness направления и скорости): общий хелпер
`run_forward(yaw_deg) -> (Vec3 displacement, f32 v_run)`: `headless_app`, `settle`, записать
`MoveIntent { axis: Vec2::Y, yaw, gait: Run, .. }`, `run_ticks(64)`, разница `Position`.
`v_run` берётся из `app.world().resource::<LocomotionConfig>().run_speed` (из RON).
- `forward_yaw_0_moves_neg_z`: `−d.z ∈ [0.8·v, 1.0·v]`, `|d.x| < 0.1`.
- `forward_yaw_90_moves_neg_x`: `−d.x ∈ [0.8·v, 1.0·v]`, `|d.z| < 0.1`.
- `forward_yaw_180_moves_pos_z`: `d.z ∈ [0.8·v, 1.0·v]`, `|d.x| < 0.1`.
Ожидание из пробы: 4.2004 м = 0.933·v по нужной оси, боковой дрейф < 1e−6.

**4c. `tests/jump.rs`**:
- `jump_apex_matches_jump_height`: `settle`, `rest = Position.y`, `jump_held = true`, 96 тиков,
  `rise = max(y) − rest`, assert `|rise − jump_height| ≤ 0.1·jump_height`. Проба: 1.0625 при
  takeoff 10.
- `tapped_jump_fires_once`: `jump_requested = true`, `jump_held = false`, 1 тик → assert
  `jump_requested == false`; ещё 32 тика, assert `max rise > 0.15` (проба: 0.257). Это гейт
  защёлки из 2.4 (молчаливый дефект "прыжок иногда не срабатывает" при высоком FPS).

**4d. `tests/config.rs`**:
- `shipped_locomotion_config_loads`: `load_config::<LocomotionConfig>(&assets_root(), LOCOMOTION_CONFIG)` → `Ok`.
- `unknown_field_names_file_and_field`: создать уникальный каталог в `std::env::temp_dir()`
  (`gta_sim_cfg_<pid>_<nanos>`), `character/locomotion.ron` = настоящий текст, в котором после
  первой `(` вставлено `bogus_field: 1.0,`; `load_config` → `Err`, `to_string()` содержит
  `"locomotion.ron"` и `"bogus_field"`. Каталог удалить в конце. Вставка после первой `(`
  не зависит от `\r\n` (checkout с `autocrlf`, домен gates).

**4e. Flip-RED (обязательно, записать в IMPL_SUMMARY, что именно ломалось):**
| Гейт | Саботаж | Ожидание |
|---|---|---|
| yaw 0 | в `move_direction` `Vec3::new(a.x, 0, a.y)` | yaw 0 и 180 RED |
| yaw 90 | `Quat::from_rotation_y(-yaw)` | только yaw 90 RED (0 и 180 зелёные — поэтому тест 90° и нужен) |
| скорость | в `drive_characters` `speed(Gait::Walk)` вместо `intent.gait` | все три RED по модулю |
| прыжок | в `tnua_config` `height: self.jump_height * 1.3` | RED |
| прыжок (данные) | в RON `jump_takeoff_extra_gravity: 30.0` | RED (1.165 > 1.1) |
| защёлка | убрать `|| intent.jump_requested` | `tapped_jump_fires_once` RED |
| строгость | убрать `#[serde(deny_unknown_fields)]` | `unknown_field...` RED |
Каждый раз вернуть и увидеть GREEN.

### Шаг 5. Проверка дерева как гейт
**Файл:** `tools/qa/tree_check.py` (stdlib, ~80 строк). Запускает из корня и проверяет:
1. `cargo tree -p gta_sim -e normal -i bevy_render` → вывод без строк пакетов ("nothing to print").
2. `cargo tree -i image --features dev,debug -e normal --depth 0` → ровно одна версия, `0.25.9`.
3. `cargo tree -i bevy_egui --features dev,debug -e normal --depth 0` → ровно одна версия `0.40.x`.
4. `cargo tree -d --features dev,debug -e normal --depth 0` → нет дубликатов с именами по regex
   `^(bevy|bevy_.*|bevy-.*|avian3d|parry3d|egui|bevy_egui|image|wgpu.*|naga|winit)$`; остальные
   дубликаты печатаются как информация (базовая линия в `scratch/tree_ws.txt`: glam 0.32/0.33,
   hashbrown, windows-sys и др. — чужие, неустранимы).
Код выхода ≠ 0 при нарушении. Flip-RED: временно закомментировать `[patch.crates-io]` →
проверка 1 RED; вернуть.

### Шаг 6. Клиент: ввод, камера, визуал
**6a. `assets/camera/camera.ron`** (+ `CameraConfig`, `deny_unknown_fields`, в `src/camera/config.rs`):
```ron
(
    pivot_height: 1.55,                // m above feet
    shoulder_offset: 0.45,             // m to the right
    distance: 3.8,                     // m
    fov_deg: 70.0,                     // vertical
    pitch_min_deg: -70.0,
    pitch_max_deg: 60.0,
    mouse_sensitivity_deg: 0.12,       // deg per mouse count
    follow_half_life: 0.05,            // s
    collision_radius: 0.25,            // m
    collision_release_half_life: 0.25, // s; pull-in is instant
)
```
Прицельный режим добавит поля в T6.

**6b. `src/main.rs`** (~50 строк): `fn main() -> AppExit`. `root = ConfigRoot(FileAssetReader::get_base_path().join("assets"))`;
`App::new().add_plugins(DefaultPlugins.set(WindowPlugin { primary_window: Some(Window { title:
"GTA-like".into(), ..default() }), ..default() }))`; `compose_sim(&mut app, root.clone())` — при
`Err` `eprintln!("{err}")` и `return AppExit::error()`; то же для `load_config::<CameraConfig>`;
плагины `EnhancedInputPlugin`, `PlayerInputPlugin`, `CameraPlugin`, `VisualsPlugin`,
`#[cfg(feature = "dev")] QaRemotePlugin`, `#[cfg(feature = "debug")] DebugToolsPlugin`; `app.run()`.

**6c. `src/input/mod.rs`** (~130 строк), домен ввода:
- Контекст `#[derive(Component)] struct OnFoot;` `add_input_context::<OnFoot>()`. Действия:
  `Move` (Vec2, `Bindings::spawn(Cardinal::wasd_keys())`), `Look` (Vec2,
  `bindings![Binding::mouse_motion()]`), `Sprint` (bool, ShiftLeft), `Walk` (bool, AltLeft),
  `Jump` (bool, Space). Отдельная сущность-контроллер `(Name::new("PlayerInput"), OnFoot,
  actions!(OnFoot[..]))` спавнится в `Startup` (не зависит от момента спавна игрока).
- `CursorCaptured(bool)` + `Startup` захват курсора + `Update` система Esc/ЛКМ из 2.6
  (`Single<&mut CursorOptions, With<PrimaryWindow>>`).
- `Update` система `write_move_intent` `.after(camera::apply_mouse_look)`: `Single<&Action<Move>>`,
  `Single<&Action<Sprint>>`, `Single<&Action<Walk>>`, `Single<(&Action<Jump>, &ActionEvents)>`,
  `Single<&OrbitCamera>`, `Single<&mut MoveIntent, With<Player>>` (Single пропускает систему,
  пока игрока нет: `bevy_ecs-0.19.1/src/system/system_param.rs:377`). Пишет `axis`, `yaw =
  camera.yaw`, `gait` (Walk приоритетнее Sprint), `jump_held`, и `jump_requested = true` при
  `ActionEvents::STARTED` (никогда не сбрасывает сама).
- Геймпад в T1 не биндим (GDD §3.3 MVP без отдельной приёмки; не в цели T1).

**6d. `src/camera/mod.rs` + `src/camera/config.rs`** (~200 строк):
- `Startup`: спавн `(Camera3d::default(), Projection::Perspective(PerspectiveProjection { fov:
  cfg.fov_deg.to_radians(), ..default() }), OrbitCamera { yaw: 0, pitch: 0, distance:
  cfg.distance, pivot: None }, Transform)`.
- `Update` `apply_mouse_look` (только при `CursorCaptured(true)`; `Single<&Action<Look>>`).
- `PostUpdate` `follow_player.before(TransformSystems::Propagate)`: алгоритм 2.5 целиком,
  `SpatialQuery` avian, `Collider::sphere(cfg.collision_radius)`.
- Observer `On<Add, Player>` → `insert(TransformInterpolation)`.
- `OrbitCamera`: `#[derive(Component, Reflect)] #[reflect(Component)]`.

**6e. `src/visuals/mod.rs`** (~90 строк): `DirectionalLight` (тени) + `AmbientLight`/свет
окружения по API 0.19 (сверить имя ресурса/компонента в `bevy_light-0.19.1` перед
использованием); observer `On<Add, Block>` → `Mesh3d(Cuboid::from_size(size))` +
`MeshMaterial3d` (серый); observer `On<Add, CharacterBody>` → визуальная капсула от ног до макушки
(`Capsule3d { radius, half_length: (float_height + height/2)/2 − radius }`, смещение дочерней
сущностью вниз на `(float_height − height/2)/2` относительно центра тела — от ног y=feet до 1.8)
и маленький кубик-"нос" спереди (локальная −Z), чтобы владелец видел поворот к движению. Цвета
это визуал, не тюнинг геймплея; держать их в одном месте модуля.

**6f. Проверка шага:** `cargo run` открывает окно, персонаж стоит на полу, WASD/мышь работают.

### Шаг 7. Фичи `dev`, `debug`, `profile*`
- `src/remote/mod.rs` (`#![cfg(feature = "dev")]` на модуле, `mod remote` с `#[cfg]`):
  `QaRemotePlugin` → `app.add_plugins(BrpExtrasPlugin::default())`. Порт 15702 или
  `BRP_EXTRAS_PORT`.
- `src/debug/mod.rs` (`#[cfg(feature = "debug")]`): `EguiPlugin::default()` (через
  `bevy_inspector_egui::bevy_egui`), `WorldInspectorPlugin::new().run_if(inspector_visible)`,
  `PhysicsDebugPlugin`; F1 переключает ресурс `InspectorVisible`, F2 — `GizmoConfigStore::
  config_mut::<PhysicsGizmos>().0.enabled` (старт выключено). F3/F4 из GDD §10.5 не делаем: AI
  нет, свободная камера не в цели T1.
- `profile`/`profile-tracy`: только фичи в манифесте.
- Reflect: `Player`, `Character`, `MoveIntent`, `CharacterBody`, `Block`, `OrbitCamera` видны
  через `reflect_auto_register` (bevy default_app). Если сценарий шага 8 не найдёт их в
  `world.list_components`, добавить явные `app.register_type::<T>()` в плагины домена.

### Шаг 8. QA-контур
**8a. `tools/qa/brp.py`** (stdlib: `subprocess`, `urllib.request`, `json`, `time`, `pathlib`,
`argparse`; ~220 строк):
- `class Game` (контекст-менеджер): `__init__(features=("dev",), args=(), port=15702)`;
  `start()`: `cargo build --features <f>` (отдельный таймаут 1800 с, чтобы холодная сборка не
  съедала ожидание порта), путь exe из `cargo metadata --format-version 1 --no-deps`
  (`target_directory`) + `debug/gta_like(.exe)`; запуск exe напрямую с `cwd=repo`,
  env `BEVY_ASSET_ROOT=<repo>` и `BRP_EXTRAS_PORT=<port>`; stdout/stderr в `target/qa/game.log`.
- `wait_ready(timeout=120)`: опрос `rpc.discover` раз в 0.5 с; если процесс умер — ошибка с
  хвостом лога.
- `call(method, params=None)`: POST JSON-RPC 2.0 на `http://127.0.0.1:<port>/`, ошибка при поле
  `error`.
- `call_watch_first(method, params, timeout)`: тот же POST, читать поток построчно до первой
  `data: `, распарсить, закрыть соединение (для `brp_extras/screenshot`).
- Хелперы: `component_path(suffix)` (через `world.list_components`, ровно одно совпадение по
  `endswith("::" + suffix)`, иначе ошибка "not reflected/registered"), `query(components, with_)`,
  `send_keys(keys, ms)`, `move_mouse(dx, dy)`, `screenshot(path)`, `diagnostics()`, `shutdown()`.
- `__exit__`/`stop()`: `shutdown` (если жив), ждать выход 15 с, иначе `kill()`; всегда.
- `vec3(value)`: принимает и список `[x,y,z]`, и словарь `{x,y,z}`.

**8b. `tools/qa/scenarios/t1.py`** (~100 строк), выход 0 = pass, печатает JSON-сводку:
1. `Game(features=("dev",))`, `wait_ready`, пауза 2 с (оседание капсулы).
2. `player = component_path("Player")`, `tf = component_path("Transform")` →
   `p0 = query([tf], with_=[player])`.
3. `send_keys(["KeyW"], 1000)`, сон 1.6 с, `p1`. Assert `p1.z − p0.z < −2.0` и `|Δx| < 0.5`
   (yaw камеры на старте 0 → бег в −Z; ожидаемо ~4 м, порог половина — это liveness доставки
   синтетического ввода до BEI, а не точность).
4. `cam = component_path("OrbitCamera")` → `yaw0`; `move_mouse(200, 0)`; сон 0.3 с; `yaw1`.
   Assert `−0.6 < yaw1 − yaw0 < −0.2` (ожидание: 200 · 0.12° = 24° = 0.419 рад, знак минус —
   закон "мышь вправо уменьшает yaw").
5. `screenshot(<out>/t1.png)` (по умолчанию `target/qa/t1/`, аргумент `--out`); assert файл
   существует и начинается с сигнатуры PNG `\x89PNG`.
6. `diagnostics()` → assert FPS-значение присутствует и > 0, записать в сводку (порога нет,
   GDD §11).
7. `shutdown()`, assert процесс завершился ≤ 15 с.
Скриншот осматривает QA-агент (сцена не чёрная, виден персонаж и площадка). Черный кадр при
свернутом окне — известное ограничение extras (док `lib.rs`), не провал кода.

**8c. `QA_REPORT.md` (стадия QA) — чек-лист владельца** (текстом, не гейт): `cargo run`; окно;
бег/спринт (Shift)/ходьба (Alt)/прыжок (Space); подъём на рампу 30° и лестницу; камера у стены
(z = 14) не проходит сквозь неё при вращении; Esc отпускает курсор, ЛКМ возвращает; feel бега и
камеры, крутилки в `assets/character/locomotion.ron` и `assets/camera/camera.ron`.

### Шаг 9. Финальная проверка (все команды зелёные, вывод в IMPL_SUMMARY)
```
cargo build --workspace
cargo build --features dev,debug
cargo clippy --workspace --all-targets -- -D warnings
cargo clippy --workspace --all-targets --features dev,debug -- -D warnings
cargo test -p gta_sim
cargo test -p citygen
python tools/qa/tree_check.py
python tools/qa/scenarios/t1.py
```
`cargo clippy -- -D warnings` без `--workspace` линтит только корневой пакет (default member при
root-пакете), поэтому команды с `--workspace`. Vendored-крейт не член workspace (проверено
`cargo metadata`), clippy его не линтит.

Размеры файлов: все < 250 строк, лимит 750/950 не под угрозой.

---

## 4. Risk areas

1. **Vendored-патч.** Дрейф от апстрима при обновлении Tnua; кто-то добавит зависимость, снова
   включающую `avian3d/debug-plugin` в sim. Мера: `tree_check.py` (шаг 5), ADR-001 с условием
   отмены. `Cargo.lock` покажет `bevy-tnua-avian3d` как path-источник без checksum — ожидаемо.
2. **Синтетический ввод и фокус окна.** Если окно игры без фокуса, bevy_winit шлёт
   `KeyboardFocusLost` и `ButtonInput` сбрасывается; зажатая W может отпуститься раньше. Порог
   сценария (−2.0 м из ~4 м) даёт запас; при провале QA фиксирует, окно фокусируется вручную.
   Мышь не гейтится по `CursorOptions` (2.6), поэтому неудачный grab без фокуса не глушит взгляд.
3. **Reflect-регистрация** через `reflect_auto_register` не доказана для типов из `gta_sim`
   (статическая регистрация через крейты). Мера: `component_path` падает с понятным сообщением,
   fallback — `register_type` (шаг 7).
4. **Таймер отпускания `send_keys`** считает время Bevy, сценарий спит по стене часов; при
   просадке FPS W держится дольше. Порог только снизу, поэтому это не ложный провал.
5. **Высокая скорость первого тика** (Tnua ×1.5 от покоя) и `LockedAxes` с `unlock_rotation_y`:
   поворот тела мог бы влиять на движение. Проба шла с этой же конфигурацией, влияния нет
   (дрейф < 1e−6 м).
6. **Порядок `Startup`**: `spawn_player.after(spawn_test_area)`; тест `settle` ловит случай, когда
   игрок появился не на полу (сообщение "GATE BROKEN").
7. **`AssetPlugin` в тестах** ищет `crates/gta_sim/assets` (нет каталога); sim ничего не грузит
   через `AssetServer`, проба с отсутствующим каталогом прошла. Если T4+ начнёт грузить ассеты в
   sim, тестам понадобится `AssetPlugin { file_path: <корень assets> }`.
8. **Alt на Windows** может активировать системное меню окна у части раскладок/оконных менеджеров;
   если владелец заметит, ходьбу перевесить (данные BEI-биндинга в коде, не тюнинг). Прогон
   владельца.
9. **Визуальная капсула ≠ физическая** (физическая висит на 0.3 м над полом, визуальная от ног).
   Это намеренно; T4 заменит визуал моделью по тем же `CharacterBody`.
10. **Холодная сборка `--features dev,debug`** долгая (render + egui); `brp.py` разводит таймаут
    сборки и таймаут порта.
11. **`cargo test --workspace`** унифицирует фичи и может собрать sim с рендером — честная граница
    только `-p gta_sim` (GDD R15); в плане так и гоняется.

---

## 5. Open questions

Вопросы владельцу (оркестратору по делегированию). У каждого есть дефолт, реализация по дефолту
не блокируется.

**Q1. Как выполнить критерий "в дереве `gta_sim` нет `bevy_render`", если `bevy-tnua-avian3d 0.12.1`
сам включает рендерную фичу avian?**
- A (дефолт): vendored-копия 0.12.1 с одной удалённой строкой манифеста + `[patch.crates-io]` +
  ADR-001. Критерий выполняется буквально, код Tnua тот же. Цена: ручное обновление при апгрейде.
- B: ослабить критерий до "`gta_sim` не использует рендер-типы" и принять `bevy_render` в дереве.
  Цена: граница headless перестаёт быть ошибкой сборки, тесты компилируют wgpu (дольше), закон
  домена bevy-ecs нарушается.
- C: своя интеграция Tnua↔avian внутри `gta_sim` (~450 строк копии). Цена: больше кода и дрейфа
  без выигрыша против A.
Рекомендация: A.

**Q2. Геометрия тестовой площадки: код или данные?** GDD §12 перечисляет data-файлы полностью, и
файла площадки там нет.
- A (дефолт): таблица кубоидов в `world/test_area.rs` с пометкой "геометрия уровня, заменяется
  городом в T2". Это контент уровня, а не feel-тюнинг.
- B: новый `assets/world/test_area.ron`. Цена: правка списка файлов GDD ради площадки, которая
  живёт один слайс.
Рекомендация: A.

**Q3. Стартовый `jump_takeoff_extra_gravity`** (нет в GDD, нужен, чтобы прыжок был честным).
- A (дефолт): 10 м/с² — прыжок бодрее, апекс 1.06 м при заданном 1.0 (гейт ±10% проходит).
- B: 0 — апекс ровно 1.0 м, прыжок "медленнее" на взлёте.
- C: 30 (дефолт Tnua) — апекс 1.165 м, гейт падает; тогда либо гейт шире, либо `jump_height` 0.86.
Рекомендация: A, окончательно решает прогон владельца (крутится в `locomotion.ron`).

---

## Источники
- Bevy: пропуск/дубль `just_pressed` в фиксированном шаге и накопление ввода —
  [bevyengine/bevy#6183](https://github.com/bevyengine/bevy/issues/6183),
  [пример physics_in_fixed_timestep](https://bevy.org/examples/movement/physics-in-fixed-timestep/),
  [Discussion #9164](https://github.com/bevyengine/bevy/discussions/9164).
- Коллизия камеры сферой, исключение своего коллайдера (spring arm):
  [UE5 Spring Arm guide](https://uhiyama-lab.com/en/notes/ue/camera-spring-arm-guide/),
  [Godot: third-person camera with spring arm](https://docs.godotengine.org/en/stable/tutorials/3d/spring_arm.html).
- Сглаживание через half-life, независимое от FPS:
  [Rory Driscoll, Frame Rate Independent Damping using Lerp](https://www.rorydriscoll.com/2016/03/07/frame-rate-independent-damping-using-lerp/).
- Исходники пиненных крейтов: `maw/tasks/done/TASK-001/scratch/crates/*`,
  `maw/tasks/in_progress/TASK-002/scratch/crates/*`, `~/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/*-0.19.1/`.
- Доказательства пробы: `scratch/probe/src/main.rs`, `scratch/probe_run1.txt`,
  `probe_run2.txt`, `probe_variant3.txt`, `probe_jump_sweep.txt`, `probe_terrain.txt`,
  `tree_render_unpatched.txt`, `tree_render_patched.txt`, `tree_ws.txt`,
  эталон манифестов `scratch/probe_ws/`.

children: 0 launched / 0 reported.
