# PLAN FINAL — TASK-002 (GDD T1): каркас, персонаж под камерой, agent QA

Цена ошибки смешанная. Молча ломаются и поэтому получают исполняемые гейты: утечка `bevy_render` в
`gta_sim`, знак yaw в формуле движения, нестрогий RON, дрейф версий (`image`, `bevy_egui`), потеря
короткого нажатия прыжка. Feel бега/прыжка/камеры, проход камеры сквозь стену и читаемость картинки
владелец видит на первом кадре: это его прогон (owner checklist), гейтов под это не строим.

Все числа с пометкой "проба" получены исполняемой пробой (`scratch/probe/src/main.rs`, headless,
`MinimalPlugins` + avian3d 0.7.0 + bevy-tnua 0.32.0, та же раскладка расписаний, что в этом плане).

---

## 1. Summary

Создаётся workspace из трёх крейтов: корневой бинарник `gta_like` (презентация, ввод BEI, камера,
визуал, фичи `dev`/`debug`/`profile`/`profile-tracy`), headless `crates/gta_sim` (весь геймплей:
строгий RON-загрузчик с `ConfigRoot`, `character/` на Tnua+Avian, `player/`, `world/` с тестовой
площадкой, единственная функция композиции `compose_sim`) и пустой `crates/citygen`. Версии
пинятся через `=`, `Cargo.lock` коммитится. Утечку рендера через `bevy-tnua-avian3d 0.12.1`
(безусловная фича `avian3d/debug-plugin`) закрывает vendored-копия с одной удалённой строкой
манифеста через `[patch.crates-io]` и ADR-001 (решение Q1=A). Физика стоит в раскладке, которую
используют все примеры Tnua 0.32.0: `PhysicsPlugins::default()` (Avian в `FixedPostUpdate`), Tnua и
`drive_characters` в `FixedUpdate`; `FixedUpdate` исполняется перед `FixedPostUpdate` в каждом
фиксированном тике. Клиент в `Update` пишет `MoveIntent` (с защёлкой прыжка), `FixedUpdate`
потребляет. Headless-гейты в `crates/gta_sim/tests/` собирают `App` через ту же `compose_sim`,
крутят контролируемые фиксированные тики и проверяют направление/скорость (yaw 0/90/180), апекс
прыжка, защёлку, строгость конфига; каждый гейт проходит flip-RED. `tools/qa/tree_check.py`
проверяет дерево зависимостей, `tools/qa/brp.py` + `tools/qa/scenarios/t1.py` гоняют игру через
BRP (клавиши, мышь, скриншот обычным JSON-RPC, FPS, shutdown).

---

## 2. Implementation steps

Порядок: headless-скелет с гейтами (он же исполняемая проба в реальном дереве, требование GDD
"проба первым шагом"), затем клиент, затем QA. Работать в текущем checkout; отдельный `target/`,
`git worktree`, `git clone` не создавать. `cargo update` не запускать.

### Шаг 1. Workspace, пины, vendored-патч

**Файлы:** `/Cargo.toml`, `/Cargo.lock`, `/src/main.rs` (временная заглушка `fn main() {}` до
шага 7), `/crates/gta_sim/Cargo.toml`, `/crates/gta_sim/src/lib.rs`, `/crates/citygen/Cargo.toml`,
`/crates/citygen/src/lib.rs`, `/vendor/bevy-tnua-avian3d-0.12.1/` (целиком),
`/docs/decisions/ADR-001-vendored-tnua-avian3d.md`.

1. `/Cargo.toml` (эталон резолвится без предупреждений: `scratch/probe_ws/Cargo.toml`,
   перепроверено `cargo tree --offline --locked` на cargo 1.95.0):
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
   name = "gta_like"
   version = "0.1.0"
   edition.workspace = true
   rust-version.workspace = true
   publish = false

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
   ron = { workspace = true }
   bevy_brp_extras = { version = "=0.22.6", optional = true }
   bevy-inspector-egui = { version = "=0.37.0", optional = true }
   ```
   Все имена фич bevy (`bevy_remote`, `png`, `trace_chrome`, `trace_tracy`, `bevy_settings`,
   `std`, `multi_threaded`, `bevy_state`, `bevy_log`, `bevy_asset`, `serialize`,
   `reflect_auto_register`, `debug`) есть в `bevy-0.19.1/Cargo.toml` (проверено).
   `bevy_settings` включается по Rollout GDD ("в клиенте постоянно"), кода под него в T1 нет.
   Прямой зависимости на `bevy_egui` нет (ловушка GDD §10.1). Фичу `bevy/dev` не включать (GDD
   §10.1: наш `dev` это другое имя).
2. `crates/gta_sim/Cargo.toml`: `[package] name = "gta_sim"`, `version = "0.1.0"`,
   `edition.workspace`, `rust-version.workspace`, `publish = false`.
   `[dependencies]`: `bevy = { workspace = true, features = ["std", "multi_threaded", "bevy_state",
   "bevy_log", "bevy_asset", "serialize", "reflect_auto_register"] }` (к списку GDD §12 добавлен
   `bevy_asset`: sim сам пользуется `Assets`, сейчас он приходит только транзитивно через Tnua),
   `avian3d`, `bevy-tnua`, `bevy-tnua-avian3d`, `ron`, `serde` (все `workspace = true`).
   `[dev-dependencies] bevy = { workspace = true, features = ["debug"] }` — имена систем в паниках
   тестов. `bevy_internal/debug` = `bevy_utils/debug`, `bevy_ecs/debug`, `bevy_render?/debug`
   (слабая зависимость, рендер не включает; `bevy_internal-0.19.1/Cargo.toml:237-241`).
3. `crates/citygen/Cargo.toml` без зависимостей; `src/lib.rs` — одна строка `//!` о назначении
   (чистый генератор города, GDD §2.2). Генератор это T2.
4. Vendored-крейт: `cargo fetch`, затем скопировать
   `~/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/bevy-tnua-avian3d-0.12.1/` целиком в
   `vendor/bevy-tnua-avian3d-0.12.1/` (или распаковать
   `https://static.crates.io/crates/bevy-tnua-avian3d/bevy-tnua-avian3d-0.12.1.crate`). В его
   `Cargo.toml`, секция `[dependencies.avian3d] features`, удалить ровно строку `"debug-plugin",`.
   Больше ничего не менять: `diff -r` с архивом = одна строка.
5. ADR-001 (`docs/decisions/ADR-001-vendored-tnua-avian3d.md`, по-русски): контекст (строка
   `features = ["3d","debug-plugin","parallel"]` в `bevy-tnua-avian3d-0.12.1/Cargo.toml` →
   `avian3d/debug-plugin` → `bevy/bevy_gizmos`, `bevy/bevy_render`; код адаптера debug-API не
   использует), решение (Q1=A из TASK_FINAL), альтернативы (ослабить гейт; копия интеграции ~450
   строк в `gta_sim`; другой контроллер) и почему нет, единственный diff, условие снятия (апстрим
   сделает `debug-plugin` опциональным → удалить `vendor/` и `[patch.crates-io]`), процедура
   обновления (повторить п.4 для новой версии).
6. `cargo generate-lockfile` (или первый `cargo check`); `Cargo.lock` в git. `bevy-tnua-avian3d` в
   lock будет path-источником без checksum — это ожидаемо.

Проверка шага: `cargo check --workspace` зелёный; `cargo tree -p gta_sim -e normal -i bevy_render`
в stdout ничего не печатает (в stderr `warning: nothing to print.`); `cargo tree -i
bevy-tnua-avian3d` показывает путь `vendor/`.

### Шаг 2. `config/`: строгий загрузчик

**Файл:** `crates/gta_sim/src/config/mod.rs` (~70 строк).
- `#[derive(Resource, Clone, Debug)] pub struct ConfigRoot(pub PathBuf);` + `pub fn path(&self,
  rel: &str) -> PathBuf`.
- `#[derive(Debug)] pub struct ConfigError { pub path: PathBuf, pub message: String }` +
  `impl Display` (`"{}: {}"`, `path.display()` и сообщение) + `impl std::error::Error`.
- `pub fn load_config<T: DeserializeOwned>(root: &ConfigRoot, rel: &str) -> Result<T,
  ConfigError>`: `fs::read_to_string(root.path(rel))` (IO-ошибка → `ConfigError` с путём),
  `ron::from_str::<T>` (ошибка → `ConfigError { path, message: err.to_string() }`). Golden path:
  два `map_err` + `?`.
- ron 0.12.2 печатает `"line:col: Unexpected field named `bogus_field` in `LocomotionConfig`..."`
  (`ron-0.12.2/src/error.rs:118-121,238-246`), поэтому `Display` ошибки содержит и путь файла, и
  имя поля.
- Каждая наша структура конфигов: `#[serde(deny_unknown_fields)]` (Tnua-конфиги его не объявляют,
  GDD §3.1, поэтому в RON лежит своя структура).

### Шаг 3. `character/`, `player/`, `world/`, `compose_sim`

**3a. `assets/character/locomotion.ron`** — единственный источник чисел движения:
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
    float_height: 1.05,               // m, body centre above ground; head top = 1.05 + 0.75 = 1.8
)
```
Числа GDD §3.1 плюс капсула, float и takeoff (Q3=A, 10.0). Под них `const` нет.

**3b. `crates/gta_sim/src/character/locomotion.rs`** (~80 строк): `LocomotionConfig`
(`#[derive(Resource, Deserialize, Clone, Debug)]`, `#[serde(deny_unknown_fields)]`), поля 1:1 с
RON; `pub const LOCOMOTION_CONFIG: &str = "character/locomotion.ron";` (путь файла, не тюнинг);
`pub fn speed(&self, gait: Gait) -> f32`; `pub fn tnua_config(&self) -> CharacterSchemeConfig`:
- `basis: TnuaBuiltinWalkConfig { speed: 1.0 /* desired_motion is in m/s */, float_height,
  acceleration: run_speed / time_to_run_speed, coyote_time, turning_angvel:
  turn_rate_deg.to_radians(), ..Default::default() }`;
- `jump: TnuaBuiltinJumpConfig { height: jump_height, input_buffer_time: jump_buffer,
  takeoff_extra_gravity: jump_takeoff_extra_gravity, ..Default::default() }`.
Эквивалентность `speed: 1.0` + `desired_motion = dir·v` пробе (`speed: 4.5`, единичный вектор):
`desired_velocity = desired_motion * config.speed` (`bevy-tnua-0.32.0/src/builtins/walk.rs:315`),
а `acceleration` абсолютная и от `speed` не зависит (`walk.rs:323-328`). Прочие поля Tnua остаются
его дефолтами, новых чисел в коде нет.

**3c. `crates/gta_sim/src/character/intent.rs`** (~80 строк):
- `#[derive(Component, Reflect, Default, Clone, Debug)] #[reflect(Component, Default)] pub struct
  MoveIntent { pub axis: Vec2, pub yaw: f32, pub gait: Gait, pub jump_held: bool, pub
  jump_requested: bool }` — `axis` x вправо / y вперёд, `yaw` в радианах (yaw камеры),
  `jump_held` уровень, `jump_requested` защёлка edge: ставит клиент в `Update`, снимает
  потребитель в `FixedUpdate` (тап короче интервала тиков при высоком FPS иначе теряется,
  bevyengine/bevy#6183).
- `#[derive(Reflect, Default, Clone, Copy, Debug, PartialEq, Eq)] #[reflect(Default)] pub enum
  Gait { Walk, #[default] Run, Sprint }`.
- `pub fn move_direction(axis: Vec2, yaw: f32) -> Vec3 { let a = axis.clamp_length_max(1.0);
  Quat::from_rotation_y(yaw) * Vec3::new(a.x, 0.0, -a.y) }` — формула GDD §3.2.
- `#[cfg(test)]` юнит-тест: шесть векторов, допуск 1e−5 (проверка формулы; проводку доказывают
  интеграционные тесты шага 4). Выведено через `R_y(θ)(x,y,z) = (x cosθ + z sinθ, y, −x sinθ +
  z cosθ)`:
  - yaw 0: W `(0,1)` → `(0,0,−1)`; D `(1,0)` → `(1,0,0)`;
  - yaw +90°: W → `(0·0 + (−1)·1, 0, −0·1 + (−1)·0)` = `(−1,0,0)`; D → `(1·0 + 0, 0, −1·1)` =
    `(0,0,−1)`;
  - yaw 180°: W → `(0,0,+1)`; D → `(−1,0,0)`.

**3d. `crates/gta_sim/src/character/mod.rs`** (~150 строк):
- `#[derive(TnuaScheme)] #[scheme(basis = TnuaBuiltinWalk)] pub enum CharacterScheme {
  Jump(TnuaBuiltinJump) }` (генерирует `CharacterSchemeConfig { basis, jump }`, компилируется в
  пробе).
- `#[derive(Component, Reflect, Default)] #[reflect(Component)] #[require(MoveIntent)] pub struct
  Character;`
- `#[derive(Component, Reflect, Clone, Copy, Debug)] #[reflect(Component)] pub struct
  CharacterBody { pub radius: f32, pub height: f32, pub float_height: f32 }` — производное
  состояние для презентации; клиент не открывает `locomotion.ron` (GDD §12).
- `#[derive(Resource)] pub struct CharacterControlConfig(pub Handle<CharacterSchemeConfig>);` +
  `impl FromWorld`: берёт `LocomotionConfig`, `world.resource_mut::<Assets<CharacterSchemeConfig>>()
  .add(cfg.tnua_config())`. Ресурс инициализируется в `CharacterPlugin::build` после
  `add_plugins(TnuaControllerPlugin)`, который делает `init_asset::<S::Config>()`
  (`bevy-tnua-0.32.0/src/controller.rs:52`).
- `pub fn character_components(cfg: &LocomotionConfig, handle: Handle<CharacterSchemeConfig>) ->
  impl Bundle`: `Character`, `CharacterBody { .. из cfg }`, `RigidBody::Dynamic`,
  `Collider::capsule(cfg.capsule_radius, cfg.capsule_height - 2.0 * cfg.capsule_radius)`,
  `LockedAxes::ROTATION_LOCKED.unlock_rotation_y()`, `TnuaController::<CharacterScheme>::default()`,
  `TnuaConfig::<CharacterScheme>(handle)`,
  `TnuaAvian3dSensorShape(Collider::cylinder(cfg.capsule_radius - SENSOR_INSET, 0.0))`.
- `const SENSOR_INSET: f32 = 0.01;` с однострочным комментарием "Tnua: sensor slightly narrower
  than the capsule so it does not hit walls" — инженерный закон интеграции, не feel (см. Review
  notes, пункт 6).
- `CharacterPlugin::build`: `add_plugins(TnuaControllerPlugin::<CharacterScheme>::new(FixedUpdate))`,
  `init_resource::<CharacterControlConfig>()`, `add_systems(FixedUpdate,
  drive_characters.in_set(TnuaUserControlsSystems))`. Tnua сам цепляет `Sensors →
  TnuaUserControlsSystems → Logic → Motors` в этом расписании (`controller.rs:53-63`).
- `drive_characters(cfg: Res<LocomotionConfig>, mut q: Query<(&mut MoveIntent, &mut
  TnuaController<CharacterScheme>), With<Character>>)`: для каждого `controller.
  initiate_action_feeding()`; `dir = move_direction(intent.axis, intent.yaw)`; `controller.basis =
  TnuaBuiltinWalk { desired_motion: dir * cfg.speed(intent.gait), desired_forward:
  Dir3::new(dir).ok() }`; если `intent.jump_held || intent.jump_requested` →
  `controller.action(CharacterScheme::Jump(Default::default()))`; затем `if intent.jump_requested
  { intent.jump_requested = false; }` (условие, чтобы не метить компонент изменённым каждый тик).
  Сигнал — состояние компонента, не `Message` и не observer.

**3e. `crates/gta_sim/src/world/mod.rs` + `world/test_area.rs`** (~120 строк):
- `#[derive(Component, Reflect)] #[reflect(Component)] pub struct Block { pub size: Vec3 }` —
  описание статического бокса для презентации.
- `#[derive(Resource)] pub struct PlayerSpawn(pub Vec3);` — точка ног игрока.
- `WorldPlugin`: `insert_resource(PlayerSpawn(Vec3::ZERO))`, `Startup` система `spawn_test_area`:
  каждый элемент = `(Block { size }, RigidBody::Static, Collider::cuboid(size.x, size.y, size.z),
  Transform::from_translation(pos).with_rotation(Quat::from_rotation_x(rot_x_deg.to_radians())))`.
- Таблица `const TEST_AREA: [(Vec3, Vec3, f32); N]` (size, center, rot_x_deg) с комментарием
  "level geometry, replaced by citygen in T2" (Q2=A). Верх пола y = 0; круг 8 м вокруг спавна
  свободен (гейты бегут 4.2 м в −Z, −X, +Z):
  - пол: size (80, 1, 80), center (0, −0.5, 0), rot 0;
  - коробки: (1,1,1) в (10, 0.5, 10); (2,1,2) в (13, 0.5, 10); (1.5,2,1.5) в (16, 1, 10);
  - рампа 30°: size (4, 0.4, 10), rot_x +30° (поднимает −Z конец: `R_x(30°)·(0,0,−1) =
    (0, 0.5, −0.866)`). Центр выводится так, чтобы нижний край верхней грани лёг на пол у z = −12:
    нижний край верхней грани в локальных координатах (0, +0.2, +5), `R_x(30°)·(0, 0.2, 5) =
    (0, 0.2·cos30 − 5·sin30, 0.2·sin30 + 5·cos30) = (0, −2.327, 4.430)`, отсюда
    `center_y = 2.327`, `center_z = −12 − 4.430 = −16.430`, x = −10. Верхний край: `R_x(30°)·
    (0, 0.2, −5) = (0, 2.673, −4.230)` → (y 5.000, z −20.660). Верхняя грань от (z −12, y 0) до
    (z −20.660, y 5.0);
  - площадка рампы: size (4, 5, 4), center (−10, 2.5, −22.66) (верх y = 5, ближний край z =
    −20.66 совпадает с верхом рампы);
  - лестница: 8 ступеней, ступень i = 0..7: size (3, 0.2·(i+1), 0.3), center (10, 0.1·(i+1),
    −12 − 0.15 − 0.3·i) (сплошные столбики от пола, как в пробе `probe_terrain.txt`);
  - площадка лестницы: size (3, 1.6, 6), center (10, 0.8, −14.4 − 3) = (10, 0.8, −17.4) (ближний
    край z = −14.4 совпадает с дальним краем 8-й ступени);
  - стена: size (12, 4, 0.5), center (0, 2, 14) — для проверки владельцем "камера не проходит
    сквозь стену".
  Реализатор пересчитывает эти центры в тесте-однострочнике или вручную при изменении таблицы; это
  контент уровня, не тюнинг.

**3f. `crates/gta_sim/src/player/mod.rs`** (~40 строк): `#[derive(Component, Reflect, Default)]
#[reflect(Component)] pub struct Player;`, `PlayerPlugin` со `Startup` системой `spawn_player
.after(spawn_test_area)`: `commands.spawn((Player, Name::new("Player"),
Transform::from_translation(spawn.0 + Vec3::Y * cfg.float_height), character_components(&cfg,
handle.0.clone())))`.

**3g. `crates/gta_sim/src/lib.rs`** (~40 строк): `pub mod config; pub mod character; pub mod
player; pub mod world;` и
```rust
pub fn compose_sim(app: &mut App, root: ConfigRoot) -> Result<(), ConfigError>
```
Порядок: `let cfg = load_config::<LocomotionConfig>(&root, LOCOMOTION_CONFIG)?;` →
`app.insert_resource(root).insert_resource(cfg)` → `app.add_plugins((PhysicsPlugins::default(),
TnuaAvian3dPlugin::new(FixedUpdate), CharacterPlugin, WorldPlugin, PlayerPlugin))` → `Ok(())`.
`compose_sim` не добавляет `AssetPlugin`/`TransformPlugin` (их даёт `DefaultPlugins`; дубль
плагина = паника). `Result`, а не паника в `Plugin::build`: клиент печатает ошибку с файлом и полем
и выходит с `AppExit::error()` (`bevy_app-0.19.1/src/app.rs:1572`), тест проверяет ту же функцию.

Расписание (обоснование): Avian по умолчанию в `FixedPostUpdate` (`avian3d-0.7.0/src/lib.rs:
751-754`); Tnua-плагины и `drive_characters` в `FixedUpdate`. `FixedMain` исполняет `FixedFirst,
FixedPreUpdate, FixedUpdate, FixedPostUpdate, FixedLast` по порядку в каждом тике
(`bevy_app-0.19.1/src/main_schedule.rs:359-363`), значит Sensors → controls → Logic → Motors
всегда перед шагом физики того же тика, ровно один шаг физики на тик. Это раскладка всех девяти
примеров `bevy-tnua-0.32.0/examples/*.rs` (строки 15-19) и пробы. `.before(PhysicsStepSystems::
First)` внутри `TnuaAvian3dPlugin` в `FixedUpdate` ничего не упорядочивает (набор живёт во
вложенном `PhysicsSchedule`) и не мешает.

### Шаг 4. Headless-гейты (`cargo test -p gta_sim`)

**4a. `crates/gta_sim/tests/common/mod.rs`**:
- `pub fn assets_root() -> ConfigRoot`: `Path::new(env!("CARGO_MANIFEST_DIR")).join("../../assets")
  .canonicalize()`; при ошибке `panic!("GATE BROKEN: assets root not found at {path}")`.
- `pub fn headless_app() -> App`: `App::new()`, `add_plugins((MinimalPlugins, TransformPlugin,
  AssetPlugin::default()))`, `insert_resource(TimeUpdateStrategy::FixedTimesteps(1))`
  (`bevy_time-0.19.1/src/lib.rs:120-122`), `compose_sim(&mut app, assets_root()).expect("GATE
  BROKEN: compose_sim failed")`, `app.finish(); app.cleanup();` (без них паника
  `ColliderTreeDiagnostics ... does not exist`, `probe_run1.txt`).
- `pub fn run_ticks(app: &mut App, n: u32)`: крутит `app.update()`, пока `Time<Fixed>::elapsed()`
  не вырастет на `n · timestep`; не более `n + 4` вызовов, иначе `panic!("GATE BROKEN: fixed loop
  stalled")`. Причина: первый `update()` при `FixedTimesteps(1)` даёт 0 тиков (проба: 64 update →
  63 тика).
- `pub fn player(app: &mut App) -> Entity`: `Query<Entity, With<Player>>` single; иначе
  `panic!("GATE BROKEN: expected exactly one Player")`.
- `pub fn settle(app: &mut App)`: `run_ticks(app, 64)` (как в пробе), затем предусловие
  `|Position.y − float_height| < 0.05`, иначе `panic!("GATE BROKEN: player not resting on the
  floor at spawn, y = ...")`. Проба: `y = 1.04992` при float 1.05.
- `pub fn set_intent(app, f: impl FnOnce(&mut MoveIntent))` — запись в компонент игрока.

**4b. `crates/gta_sim/tests/movement.rs`** (корректность направления и скорости): хелпер
`run_forward(yaw_deg: f32) -> (Vec3, f32)`: `headless_app`, `settle`, `p0 = Position`,
`set_intent(axis = Vec2::Y, yaw = yaw_deg.to_radians(), gait = Run)`, `run_ticks(64)`, вернуть
`(Position − p0, v_run)`, где `v_run = world.resource::<LocomotionConfig>().run_speed` (из RON).
- `forward_yaw_0_moves_neg_z`: `−d.z ∈ [0.8·v, 1.0·v]`, `|d.x| < 0.1`.
- `forward_yaw_90_moves_neg_x`: `−d.x ∈ [0.8·v, 1.0·v]`, `|d.z| < 0.1`.
- `forward_yaw_180_moves_pos_z`: `d.z ∈ [0.8·v, 1.0·v]`, `|d.x| < 0.1`.

Рабочий пример через реальный код Tnua (`walk.rs:315-332`), 64 Гц, `acceleration = 4.5/0.15 = 30`:
тик 1 из покоя `direction_change_factor = 1.5 − 0.5·0 = 1.5`, Δv = 1.5·30/64 = 0.703 м/с; далее
коэффициент 1.0, Δv = 0.469 м/с за тик, 4.5 м/с достигается на ~9-м тике; путь ≈ 4.5 − потеря на
разгоне (~0.3 м) ≈ 4.2 м. Проба: 4.2004 м = 0.9334·v_run, боковой дрейф < 1e−6 м, `yaw 180 →
x = 3.7e−7`. Коридор 3.6…4.5 м проходит с запасом с обеих сторон.

**4c. `crates/gta_sim/tests/jump.rs`**:
- `jump_apex_matches_jump_height`: `settle`, `rest = Position.y`, `jump_held = true`, 96 тиков с
  отслеживанием `max(y)`, `rise = max − rest`; assert `|rise − jump_height| ≤ 0.1·jump_height`.
  Проба при takeoff 10: 1.0625 (апекс на 22-м тике, 96 тиков с запасом).
- `tapped_jump_fires_once`: `settle`, `jump_requested = true`, `jump_held = false`, `run_ticks(1)`,
  assert `jump_requested == false`; ещё 32 тика, assert `max rise > 0.15`. Проба (подача 1 тик):
  0.2565, апекс на 2-м тике. Это гейт защёлки: молчаливый дефект "прыжок иногда не срабатывает".

**4d. `crates/gta_sim/tests/config.rs`**:
- `shipped_locomotion_config_loads`: `load_config::<LocomotionConfig>(&assets_root(),
  LOCOMOTION_CONFIG)` → `Ok`.
- `unknown_field_names_file_and_field`: уникальный каталог `std::env::temp_dir()/
  gta_sim_cfg_<pid>_<nanos>/character/`; файл `locomotion.ron` = настоящий текст из
  `assets_root()`, где сразу после первой `(` вставлено `bogus_field: 1.0,`; `load_config` → `Err`;
  `err.to_string()` содержит `"locomotion.ron"` и `"bogus_field"`. Каталог удалить в конце.
  Вставка после первой `(` не зависит от `\r\n` (`core.autocrlf=true`).

**4e. Flip-RED (обязательно; в IMPL_SUMMARY записать, какой вход ломался и что стало RED/GREEN):**

| Гейт | Саботаж (production-механизм или вход) | Ожидание (выведено) |
|---|---|---|
| знак оси | в `move_direction` `Vec3::new(a.x, 0.0, a.y)` | все три RED: yaw 0 → +Z, yaw 90 → `R_y(90)(0,0,1)` = +X, yaw 180 → −Z; юнит-тест RED |
| знак yaw | `Quat::from_rotation_y(-yaw)` | RED только yaw 90 (`R_y(−90)(0,0,−1)` = +X); yaw 0 и 180 GREEN, потому что `sin(±π) = 0` — поэтому тест 90° и нужен |
| скорость | в `drive_characters` `cfg.speed(Gait::Walk)` вместо `intent.gait` | все три RED: ~1.68 м < 3.6 м |
| прыжок (код) | в `tnua_config` `height: self.jump_height * 1.3` | `jump_apex...` RED (> 1.1) |
| прыжок (данные) | в RON `jump_takeoff_extra_gravity: 30.0` | RED (проба 1.165 > 1.1) |
| защёлка | убрать `\|\| intent.jump_requested` | `tapped_jump_fires_once` RED (rise ≈ 0) |
| строгость | убрать `#[serde(deny_unknown_fields)]` у `LocomotionConfig` | `unknown_field...` RED (`Ok` вместо `Err`) |

Каждый раз вернуть и увидеть GREEN.

### Шаг 5. Проверка дерева как гейт

**Файл:** `tools/qa/tree_check.py` (stdlib, ~90 строк). Запуск из корня репо; каждая команда
`subprocess.run(..., capture_output=True, text=True)`; разбор строк stdout вида
`<name> v<version>`; ненулевой код cargo = RED с выводом stderr (например, `cargo tree -i image`
падает с "multiple `image` packages" при двух версиях — это тоже нарушение).
1. `cargo tree -p gta_sim -e normal -i bevy_render` → stdout без строк пакетов (в stderr
   `warning: nothing to print.`; stderr не разбирать).
2. `cargo tree -p gta_sim -e normal,dev -i bevy_render` → то же (закрывает буквальный критерий
   "`cargo test -p gta_sim` собирается без `bevy_render`": dev-зависимость `bevy/debug` не должна
   тянуть рендер).
3. `cargo tree -i image --features dev,debug -e normal --depth 0` → ровно одна строка `image
   v0.25.9`.
4. `cargo tree -i bevy_egui --features dev,debug -e normal --depth 0` → ровно одна версия
   `0.40.x` (проба: `0.40.1`).
5. `cargo tree -d --features dev,debug -e normal --depth 0` → нет дубликатов с именами по regex
   `^(bevy|bevy_.*|bevy-.*|avian3d|parry3d|egui|bevy_egui|image|wgpu.*|naga|winit)$`; прочие
   дубликаты печатаются как информация (базовая линия `scratch/tree_ws.txt`: `glam 0.32/0.33`,
   `hashbrown`, `foldhash`, `indexmap`, `itertools`, `miniz_oxide`, `windows-sys` и др. — чужие).
Код выхода ≠ 0 при любом нарушении, сообщение называет проверку. Flip-RED: закомментировать
`[patch.crates-io]` → проверки 1 и 2 RED (путь `avian3d feature "debug-plugin" ←
bevy-tnua-avian3d`, `scratch/tree_render_unpatched.txt`); вернуть → GREEN. Внимание: после отката
patch cargo перепишет `Cargo.lock` — вернуть его из git (`git checkout -- Cargo.lock`).

### Шаг 6. `assets/camera/camera.ron`

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
`CameraConfig` в `src/camera/config.rs`: `#[derive(Resource, Deserialize, Clone, Debug)]`,
`#[serde(deny_unknown_fields)]`, `pub const CAMERA_CONFIG: &str = "camera/camera.ron"`. Прицельные
значения (2.0 м, 55°, 0.55, ×0.7, 0.15 с) и `AimIntent` — это T6, поля не добавлять.

### Шаг 7. Клиент: ввод, камера, визуал

**7a. `src/main.rs`** (~50 строк): `fn main() -> AppExit`:
`let root = ConfigRoot(FileAssetReader::get_base_path().join("assets"));`
(`bevy::asset::io::file::FileAssetReader::get_base_path`: `BEVY_ASSET_ROOT` → `CARGO_MANIFEST_DIR`
→ каталог exe, `bevy_asset-0.19.1/src/io/file/mod.rs:19-29,56` — тот же корень, что у
`AssetPlugin`). `let mut app = App::new(); app.add_plugins(DefaultPlugins.set(WindowPlugin {
primary_window: Some(Window { title: "GTA-like".into(), ..default() }), ..default() }));`
`if let Err(err) = compose_sim(&mut app, root.clone()) { eprintln!("{err}"); return
AppExit::error(); }`; то же для `load_config::<CameraConfig>(&root, CAMERA_CONFIG)` →
`insert_resource`. Плагины: `EnhancedInputPlugin`, `PlayerInputPlugin`, `CameraPlugin`,
`VisualsPlugin`, `#[cfg(feature = "dev")] QaRemotePlugin`, `#[cfg(feature = "debug")]
DebugToolsPlugin`; `app.run()`.

**7b. `src/input/mod.rs`** (~130 строк), BEI 0.26.0:
- Контекст `#[derive(Component)] struct OnFoot;` + `add_input_context::<OnFoot>()`. Действия (BEI
  `#[derive(InputAction)] #[action_output(..)]`): `Move` (Vec2, `Bindings::spawn(Cardinal::
  wasd_keys())`, север = +Y = W, `preset/cardinal.rs:38`), `Look` (Vec2,
  `bindings![Binding::mouse_motion()]`, `binding.rs:122`), `Sprint` (bool, `KeyCode::ShiftLeft`),
  `Walk` (bool, `KeyCode::AltLeft`), `Jump` (bool, `KeyCode::Space`). Сущность-контроллер
  `(Name::new("PlayerInput"), OnFoot, actions!(OnFoot[...]))` спавнится в `Startup` (не зависит от
  спавна игрока). Точный синтаксис `actions!`/`bindings!`/`#[action_output]` сверить с
  `bevy_enhanced_input-0.26.0/src/lib.rs` (пример в doc-комментарии, строки ~240-290) и
  проверить компиляцией.
- `#[derive(Resource)] pub struct CursorCaptured(pub bool)` (старт `true`) + `Startup` захват
  курсора + `Update` система: Esc → `CursorCaptured(false)` и `CursorOptions { grab_mode:
  CursorGrabMode::None, visible: true }`; ЛКМ при `false` → `true` + `CursorGrabMode::Locked`,
  `visible: false`. Доступ: `Single<&mut CursorOptions, With<PrimaryWindow>>` (`CursorOptions` —
  компонент окна, `bevy_window-0.19.1/src/window.rs:752`). Гейтить взгляд по
  `CursorOptions.grab_mode` нельзя: при неудачном grab (окно без фокуса — режим BRP-QA)
  bevy_winit откатывает поле (`bevy_winit-0.19.1/src/system.rs:611-621`). Esc/ЛКМ читаются через
  `ButtonInput` в `Update` (управление окном, не геймплей).
- `Update` система `write_move_intent.after(camera::apply_mouse_look)`: `Single<&Action<Move>>`,
  `Single<&Action<Sprint>>`, `Single<&Action<Walk>>`, `Single<(&Action<Jump>, &ActionEvents)>`,
  `Single<&OrbitCamera>`, `Single<&mut MoveIntent, With<Player>>` (`Single` пропускает систему,
  пока игрока нет). Пишет `axis`, `yaw = camera.yaw`, `gait` (Walk приоритетнее Sprint, иначе
  Run), `jump_held`, и `jump_requested = true` при `events.contains(ActionEvents::STARTED)`
  (`action/events.rs:47`); сама никогда не сбрасывает. `just_pressed` в `FixedUpdate` не читается.
- Геймпад в T1 не биндится (не в цели T1).

**7c. `src/camera/mod.rs` + `src/camera/config.rs`** (~200 строк):
- `#[derive(Component, Reflect)] #[reflect(Component)] pub struct OrbitCamera { pub yaw: f32, pub
  pitch: f32, pub distance: f32, pub pivot: Option<Vec3> }` (BRP читает `yaw`).
- `Startup`: спавн `(Camera3d::default(), Projection::Perspective(PerspectiveProjection { fov:
  cfg.fov_deg.to_radians(), ..default() }), OrbitCamera { yaw: 0.0, pitch: 0.0, distance:
  cfg.distance, pivot: None }, Transform::default())`.
- `Update` `apply_mouse_look` (только при `CursorCaptured(true)`; `Single<&Action<Look>>`):
  `sens = cfg.mouse_sensitivity_deg.to_radians()`; `yaw −= dx·sens`; `pitch −= dy·sens`; clamp
  pitch в `[pitch_min_deg, pitch_max_deg]` (в радианах). Мышь вправо (dx > 0) уменьшает yaw —
  закон GDD §3.2. Знак pitch проверяет владелец (мышь вниз опускает взгляд).
- `PostUpdate` `follow_player.before(TransformSystems::Propagate)`:
  - `rot = Quat::from_euler(EulerRot::YXZ, yaw, pitch, 0.0)`; `back = rot * Vec3::Z`;
    `right = rot * Vec3::X`. Примеры: yaw 0, pitch 0 → back (0,0,1), камера позади бегущего в −Z;
    yaw 90° → back (1,0,0), взгляд −X (совпадает с W при yaw 90°); yaw 180° → back (0,0,−1),
    взгляд +Z; pitch +30° → `R_x(30°)·(0,0,1) = (0, −0.5, 0.866)`, камера ниже pivot, смотрит
    вверх (положительный pitch = взгляд вверх).
  - `feet = player.translation − Y·body.float_height`; `head = feet + Y·cfg.pivot_height`;
    сглаживание `pivot.smooth_nudge(&head, LN_2 / cfg.follow_half_life, dt)`
    (`bevy_math-0.19.1/src/common_traits.rs:467`, `1 − exp(−rate·dt)`); первый кадр (`pivot ==
    None`) — снап.
  - Коллизия spring-arm в два отрезка, `SpatialQuery::cast_shape(&Collider::sphere(r), origin,
    Quat::IDENTITY, dir, &ShapeCastConfig::from_max_distance(len),
    &SpatialQueryFilter::from_excluded_entities([player]))`: (1) от pivot по `right` на
    `shoulder_offset` → `shoulder = pivot + right·min(hit.distance, offset)`; (2) от shoulder по
    `back` на `cfg.distance` → `target = min(hit.distance, cfg.distance)`. Если `target <
    current` — мгновенно, иначе `current.smooth_nudge(&target, LN_2 /
    cfg.collision_release_half_life, dt)`. Камера: `translation = shoulder + back·current`,
    `rotation = rot`. Слои коллизий не заводятся (первый потребитель T6/T8).
  - Сверить сигнатуру `cast_shape` с `avian3d-0.7.0/src/spatial_query/system_param.rs:446`.
- Observer `On<Add, Player>` → `commands.entity(ev.entity).insert(TransformInterpolation)` (плавность
  при 64 Гц физики и 144 Гц экрана; `TransformInterpolation` из `PhysicsInterpolationPlugin`
  группы `PhysicsPlugins`). Headless-тесты читают `Position`, на них это не влияет.

**7d. `src/visuals/mod.rs`** (~90 строк): `DirectionalLight` с тенями + ресурс
`GlobalAmbientLight` (в 0.19.1 ресурс называется так, `AmbientLight` — компонент камеры;
`bevy_light-0.19.1/src/ambient_light.rs:12,62`); observer `On<Add, Block>` →
`Mesh3d(meshes.add(Cuboid::from_size(size)))` + `MeshMaterial3d` (серый); observer `On<Add,
CharacterBody>` → дочерняя визуальная капсула от ног до макушки: `Capsule3d { radius,
half_length: (float_height + height/2)/2 − radius }`, смещение вниз на `(float_height −
height/2)/2` от центра тела (ноги y = feet, макушка 1.8), и маленький кубик-"нос" на локальной
−Z, чтобы владелец видел поворот к движению. Цвета — визуал, держать в одном месте модуля.

**7e. Проверка шага:** `cargo run` открывает окно, персонаж стоит на полу, WASD/мышь/Space
работают (подтверждает владелец/QA).

### Шаг 8. Фичи `dev`, `debug`, `profile*`

- `src/remote/mod.rs`, подключение `#[cfg(feature = "dev")] mod remote;`: `QaRemotePlugin` →
  `app.add_plugins(BrpExtrasPlugin::default())` (сам добавляет `RemotePlugin`,
  `RemoteHttpPlugin`, `FrameTimeDiagnosticsPlugin`; порт 15702 или env `BRP_EXTRAS_PORT`).
- `src/debug/mod.rs`, `#[cfg(feature = "debug")]`: `EguiPlugin::default()` (через
  `bevy_inspector_egui::bevy_egui`), `WorldInspectorPlugin::new().run_if(inspector_visible)`,
  `PhysicsDebugPlugin`; F1 переключает ресурс `InspectorVisible`, F2 —
  `GizmoConfigStore::config_mut::<PhysicsGizmos>().0.enabled` (старт выключено). F3/F4 не делаются
  (AI нет, свободная камера не в цели T1).
- `profile`/`profile-tracy`: только фичи в манифесте.
- Reflect: `Player`, `Character`, `MoveIntent`, `CharacterBody`, `Block`, `OrbitCamera` видны через
  `reflect_auto_register`. Если сценарий шага 9 не найдёт их в `world.list_components`, добавить
  явные `app.register_type::<T>()` в плагин соответствующего домена (не менять сценарий).

### Шаг 9. QA-контур

**9a. `tools/qa/brp.py`** (stdlib: `subprocess`, `urllib.request`, `json`, `time`, `pathlib`,
`argparse`; ~200 строк):
- `class Game` (контекст-менеджер): `__init__(features=("dev",), args=(), port=15702)`.
- `start()`: `cargo build --features <f>` (таймаут 1800 с, отдельно от ожидания порта); путь exe
  = `target_directory` из `cargo metadata --format-version 1 --no-deps` + `debug/gta_like(.exe)`;
  запуск exe напрямую (не `cargo run`: на Windows убийство `cargo.exe` не убивает игру — отклонение
  от буквы GDD §10.5 ради гарантированного убийства), `cwd=repo`, env `BEVY_ASSET_ROOT=<repo>`,
  `BRP_EXTRAS_PORT=<port>`; stdout/stderr в `target/qa/game.log`.
- `wait_ready(timeout=120)`: опрос `rpc.discover` раз в 0.5 с; если процесс умер — ошибка с хвостом
  лога.
- `call(method, params=None, timeout=10)`: POST JSON-RPC 2.0 на `http://127.0.0.1:<port>/`,
  разбор JSON, исключение при поле `error`. Используется для **всех** методов, включая
  `brp_extras/screenshot` (с `timeout=60`): без `+watch` в имени HTTP ждёт первый результат и
  отдаёт обычный JSON (`bevy_remote-0.19.1/src/http.rs:407-428`), а обработчик скриншота
  возвращает `Ok(None)` до завершения захвата (`bevy_brp_extras-0.22.6/src/screenshot/mod.rs:
  176-192`), то есть первый результат = завершённый PNG. SSE-парсер не писать. Если реальный
  ответ окажется другим — сохранить Content-Type и тело в `target/qa/` и адаптировать по
  наблюдению.
- Хелперы: `component_path(suffix)` (через `world.list_components`, ровно одно совпадение по
  `endswith("::" + suffix)`, иначе ошибка "not reflected/registered"), `query(components, with_)`
  (`world.query {data:{components}, filter:{with}}`), `send_keys(keys, ms)`, `move_mouse(dx, dy)`,
  `screenshot(path)` (абсолютный путь), `diagnostics()`, `shutdown()`, `vec3(value)` (принимает
  `[x,y,z]` и `{x,y,z}`).
- `__exit__`/`stop()`: `shutdown` если жив, ждать выход 15 с, иначе `kill()`; всегда, в `finally`.

**9b. `tools/qa/scenarios/t1.py`** (~100 строк), код выхода 0 = pass, печатает и пишет JSON-сводку
в `<out>/summary.json` (по умолчанию `target/qa/t1/`, аргумент `--out`):
1. `Game(features=("dev",))`, `wait_ready`, пауза 2 с (оседание капсулы).
2. `player = component_path("Player")`, `tf = component_path("Transform")`, `p0 = query([tf],
   with_=[player])`.
3. `send_keys(["KeyW"], 1000)` (возвращается сразу, отпускание по таймеру), сон 1.6 с, `p1`.
   Assert `p1.z − p0.z < −2.0` и `|Δx| < 0.5` (yaw камеры на старте 0 → бег в −Z; ожидаемо ~4 м,
   порог половина — liveness доставки синтетического ввода до BEI, не точность).
4. `cam = component_path("OrbitCamera")` → `yaw0`; `move_mouse(200, 0)`; подождать ≥ 0.3 с (ввод
   из `RemoteLast` попадает в `AccumulatedMouseMotion`/BEI следующего кадра); `yaw1`. Assert
   `−0.6 < yaw1 − yaw0 < −0.2` (ожидание 200·0.12° = 24° = 0.419 рад, знак минус — закон "мышь
   вправо уменьшает yaw").
5. `screenshot(<out>/t1.png)`; assert ответ без `error`, файл существует, начинается с
   `\x89PNG\r\n\x1a\n`. Это liveness публикации; содержимое (сцена не чёрная, виден персонаж и
   площадка) осматривает QA-агент. Чёрный кадр при свёрнутом окне — известное ограничение extras;
   выяснить фокус/состояние окна, не записывать PASS по одному заголовку.
6. `diagnostics()` → FPS присутствует и > 0, записать в сводку (порога нет, GDD §11).
7. `shutdown()`, assert процесс завершился ≤ 15 с.

**9c. `QA_REPORT.md` (стадия QA) — owner checklist** (текст, не гейт): `cargo run`; окно;
бег/спринт (Shift)/ходьба (Alt)/прыжок (Space); подъём на рампу 30° (x = −10) и лестницу
(x = +10); камера у стены (z = 14) не проходит сквозь неё при вращении; Esc отпускает курсор, ЛКМ
возвращает; feel бега и камеры, крутилки в `assets/character/locomotion.ron` и
`assets/camera/camera.ron`.

### Шаг 10. Финальная проверка (вывод в IMPL_SUMMARY)

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
`cargo clippy -- -D warnings` без `--workspace` линтит только корневой пакет, поэтому
`--workspace`. Vendored-крейт исключён (`exclude = ["vendor"]`), clippy его не линтит. `cargo test
--workspace` не использовать как гейт границы: унификация фич может собрать sim с рендером (GDD
R15); граница проверяется `-p gta_sim` и `tree_check.py`. Все файлы < 250 строк.

---

## 3. Test plan

| Что | Как | Ожидание | Класс |
|---|---|---|---|
| Формула направления | юнит-тест в `intent.rs` | 6 векторов из 3c, допуск 1e−5 | корректность формулы |
| Движение yaw 0 / 90 / 180 | `tests/movement.rs`, `compose_sim` + 64 тика | смещение по −Z / −X / +Z в [3.6, 4.5] м (проба 4.2004), боковое < 0.1 м | корректность (проводка + знак + скорость) |
| Апекс прыжка | `tests/jump.rs` | rise ∈ [0.9, 1.1] (проба 1.0625) | корректность |
| Защёлка тапа | `tests/jump.rs` | `jump_requested` снят за 1 тик, rise > 0.15 (проба 0.2565) | корректность |
| Строгий RON | `tests/config.rs` | `Err` с `locomotion.ron` и `bogus_field`; поставляемый файл грузится | корректность |
| Граница headless | `tree_check.py` пп.1-2 | stdout пуст для `-e normal` и `-e normal,dev` | build-time гейт |
| Дрейф версий | `tree_check.py` пп.3-5 | `image` 0.25.9 одна, `bevy_egui` 0.40.x одна, нет дублей из regex | build-time гейт |
| Сборка/линт | шаг 10 | всё зелёное | — |
| BRP-контур | `scenarios/t1.py` | W двигает ≥ 2 м в −Z; yaw −0.2…−0.6; PNG; FPS > 0; shutdown ≤ 15 с | liveness ввода/скриншота/диагностики |
| Feel, камера у стены, картинка | owner checklist в QA_REPORT.md | решение владельца | прогон владельца |

Flip-RED каждого нового гейта — таблица 4e и шаг 5; в IMPL_SUMMARY фиксируется, какой вход
ломался. Все гейты падают на коде: сбои обвязки (нет `assets/`, нет игрока, застрял фиксированный
цикл, игрок не на полу) дают `GATE BROKEN: ...`.

Если runtime QA нельзя запустить в среде агента (нет окна/GPU), записать точную причину и оставить
критерий непроверенным, не PASS.

---

## 4. Rollout notes

- Миграций, env-флагов и feature-флагов в рантайме нет. Новые cargo-фичи корня: `dev`, `debug`,
  `profile`, `profile-tracy`; по умолчанию все выключены.
- `Cargo.lock` коммитится; `cargo update` запрещён (есть bevy 0.20.0-rc.1). `bevy-tnua-avian3d` в
  lock — path-источник из `vendor/`.
- `vendor/bevy-tnua-avian3d-0.12.1/` + `[patch.crates-io]` + ADR-001: при апгрейде Tnua повторить
  процедуру из ADR или снять патч, если апстрим сделает `debug-plugin` опциональным.
- Корень ассетов: `cargo run` берёт `CARGO_MANIFEST_DIR`; прямой запуск exe требует
  `BEVY_ASSET_ROOT=<repo>` (так делает `brp.py`), иначе каталог exe.
- `.gitignore` уже игнорирует `/target/` и `*.png`: скриншоты QA в `target/qa/` не попадут в git.
- Холодная сборка `--features dev,debug` долгая (render + egui); `brp.py` разводит таймауты сборки
  и ожидания порта.
- Статусы README/AGENTS и проектный вектор обновляет пайплайн по завершении задачи; это не часть
  изменения T1.

---

## 5. Review notes

Проверенный контрпример (disconfirmation): главная поправка PLAN_V2 (п.3) утверждала, что раскладка
PLAN.md (Tnua в `FixedUpdate`, Avian в дефолтном `FixedPostUpdate`) не доказывает порядок, и
переводила физику в `PhysicsPlugins::new(FixedUpdate)` с `TnuaSystems.before(PhysicsSystems::
StepSimulation)`. Контрпример: если `FixedMain` исполняет `FixedUpdate` перед `FixedPostUpdate` в
каждом тике и апстрим Tnua использует именно исходную раскладку, поправка V2 неверна. **Контрпример
подтвердился**: `bevy_app-0.19.1/src/main_schedule.rs:359-363` задаёт порядок `FixedFirst,
FixedPreUpdate, FixedUpdate, FixedPostUpdate, FixedLast`; все девять примеров
`bevy-tnua-0.32.0/examples/*.rs` (строки 15-19) используют `PhysicsPlugins::default()` +
`TnuaControllerPlugin::new(FixedUpdate)` + `TnuaAvian3dPlugin::new(FixedUpdate)`; проба
(`scratch/probe/src/main.rs:57-64`) измерена на этой раскладке.

Изменения относительно PLAN_V2 и PLAN.md:
1. **Расписание физики: откат поправки V2.** Оставлена раскладка PLAN.md/апстрима (шаг 3g).
   Переход на `PhysicsPlugins::new(FixedUpdate)` отклонён: он уходит от протестированной апстримом
   конфигурации, обесценивает числа пробы и лечит несуществующую проблему. V2 прав в одном:
   `.before(PhysicsStepSystems::First)` адаптера в `FixedUpdate` ничего не упорядочивает — это
   записано в 3g. Числа пробы (4.2004 м, 1.0625 м, 0.2565 м) остаются диагностическим
   ожиданием; гейты — коридоры GDD.
2. **Скриншот BRP: принята поправка V2.** Проверено: `http.rs:407-428` включает поток только при
   `+watch` в имени; обработчик `screenshot/mod.rs:176-192` возвращает `Ok(None)` до завершения
   захвата. `call_watch_first`/SSE из PLAN.md удалены; обычный `call` с таймаутом 60 с.
3. **Flip-RED знака оси исправлен.** PLAN.md ожидал RED только для yaw 0 и 180; расчёт
   `R_y(90°)·(0,0,1) = (+1,0,0)` даёт RED и для yaw 90. Таблица 4e пересчитана.
4. **Гейт границы расширен** на `-e normal,dev` (шаг 5, п.2): буквальный критерий говорит о сборке
   `cargo test -p gta_sim`, а тесты тянут dev-зависимость `bevy/debug`. `bevy_internal/debug`
   включает `bevy_render?/debug` только слабо — проверка это закрепляет. Также явно: stdout, а не
   stderr, и ненулевой код `cargo tree -i` при нескольких версиях = RED; flip-RED требует вернуть
   `Cargo.lock`.
5. **Восстановлены детали, которые V2 сжал без исправления:** полный манифест, фичи bevy/avian,
   RON-файлы целиком, типы и сигнатуры, маппинг `LocomotionConfig → CharacterSchemeConfig`,
   таблица площадки, хелперы тестов, flip-RED таблица, алгоритм камеры с примерами, драйвер и
   сценарий QA с порогами, owner checklist. `settle` поднят с 32 до 64 тиков, чтобы совпасть с
   пробой (settle 63 тика, y = 1.04992).
6. **Геометрия площадки пересчитана с числами.** PLAN.md давал рампу словами ("начинается вровень
   у z = −12, y ≈ 5"); здесь центры рампы, площадок и ступеней выведены так, чтобы поверхности
   стыковались (3e), с учётом сдвига на половину толщины плиты по z (0.1 м), который наивный
   `−12 − 5·cos30` теряет. `SENSOR_INSET = 0.01` оставлен `const` как закон интеграции Tnua (форма
   сенсора, не feel); если code-review сочтёт это тюнингом — перенести в `locomotion.ron` как
   `sensor_inset`, это не меняет гейтов.
7. **API сверены с пиненными исходниками:** `On`/`Add` (`bevy_ecs-0.19.1`), `CursorOptions`,
   `Camera3d`, `Projection`, `TransformSystems::Propagate`, `smooth_nudge`, `AppExit::error`,
   `FileAssetReader::get_base_path` (pub), `TimeUpdateStrategy::FixedTimesteps`, BEI
   `Cardinal::wasd_keys`/`Binding::mouse_motion`/`ActionEvents::STARTED`/`actions!`. Уточнено:
   ресурс окружающего света в 0.19.1 — `GlobalAmbientLight` (7d), а не `AmbientLight`.
8. **Открытые вопросы PLAN.md закрыты** в TASK_FINAL (Q1=A vendored-патч, Q2=A таблица в коде,
   Q3=A takeoff 10.0); реализатору не передаются.
9. **Отклонения от буквы GDD, оставленные сознательно:** сигнатура `compose_sim(&mut App,
   ConfigRoot) -> Result<(), ConfigError>` вместо `compose_sim(&mut App)` (GDD §12 сам требует
   `ConfigRoot`, `Result` нужен для ошибки с файлом и полем); `brp.py` запускает exe, а не
   `cargo run` (надёжное убийство на Windows); `AimIntent` не пишется в T1 (потребитель — combat в
   T6, YAGNI).

children: 0 launched / 0 reported.
