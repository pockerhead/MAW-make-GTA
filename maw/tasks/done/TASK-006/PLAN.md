# PLAN — TASK-006 (GDD T5): здоровье, урон, смерть и возрождение

Цена ошибки: смешанная. Молчаливые дефекты (двойной город/игрок после возврата в `Playing`, таймер `Wasted` по виртуальному времени, `Time<Virtual>` застрял на 0.3, броня/реген считают не то) получают headless-гейты. Вид полос, экрана "ПОТРАЧЕНО", читаемость slow-mo — прогон владельца + скриншоты BRP.

Новых крейтов нет: `Cargo.lock` не меняется (bevy_ui/bevy_text уже в клиенте, `SubStates`/`Message`/`Has` есть в пиненной bevy 0.19.1). Новый бинарный ассет: шрифт Inter 4.1 (OFL), zip уже скачан в `scratch/Inter-4.1.zip` для `--cache`.

---

## 1. Understanding (что есть сейчас)

**Sim (`crates/gta_sim`)**
- `src/flow/mod.rs:1-17` — `GameState { Loading, Playing }`, `FlowPlugin` только `init_state`. Нет `Reflect` у `GameState`, BRP его не видит.
- `src/player/mod.rs:15` — `spawn_player` на `OnEnter(GameState::Playing)`. **Второй хазард, не названный в задаче:** возврат `Wasted → Playing` заспавнит второго `Player` (камера `Single<Player>` и тесты `single()` упадут).
- `src/character/mod.rs:22-141` — `Character` (`#[require(MoveIntent, JumpBuffer, AnimState)]`), `character_components`, `drive_characters` в `FixedUpdate` (`TnuaUserControlsSystems`) пишет `controller.basis` каждый тик; `basis` — постоянное поле Tnua (`bevy-tnua-0.32.0/src/controller.rs:150`), без записи тело продолжит идти.
- `src/world/mod.rs:44-80`, `src/world/city.rs:76-109` — генерация в `Update` под `in_state(Loading)`; коллайдеры (`CityGround`, `CityEdgeWall`, `CityBlock`, `CityBuilding`) спавнятся один раз в `apply_city_generation` (не на `OnEnter(Playing)` — сим-сторона хазарда не имеет). `PlayerSpawn` ресурс, `CityLandmarks` (Reflect).
- `src/lib.rs:19-46` — `compose_sim`: грузит `locomotion.ron`, `city.ron`, добавляет плагины. Единственная точка композиции (тесты и клиент).
- `src/config/manifest.rs:72-139` — схема манифеста жёстко Kenney-only: `page == https://kenney.nl/assets/{name}`, `url` с префиксом `https://kenney.nl/media/pages/assets/{name}/`, `AssetLicense { CC0 }`. Зеркало в `tools/fetch_assets.py:185-230`.
- `tests/common/mod.rs` — `composed_app`, `city_app`, `run_ticks` (по `Time<Fixed>`), `place_player`, `settle`, `set_intent`. `tests/asset_manifest.rs:62-209` — точный список паков, проверка лицензии текстом "Creative Commons Zero"/"CC0" для **каждого** пака (строки 188-199).

**citygen**
- `src/pois.rs:66` — ровно одна больница (`BuildingKind::Hospital`); property `pois_exist` это держит.
- `src/graphs.rs:26-29,136-155` — приватные `walk_offset`, `road_polygon`; `player_spawn` = середина стороны квартала + `d.normalize().perp() * walk_offset(class)` (проверено тестом `player_spawn_on_sidewalk`).
- `src/hash.rs:73-155` — `layout_hash` хэширует и `player_spawn`; новые поля layout изменили бы golden.

**Client (`src/`)**
- `src/visuals/city.rs:24` — `start_city_mesh_build` на `OnEnter(Playing)` → второй полный набор 100 чанков при возврате из `Wasted` (подтверждено PREMISE_CHALLENGE.md).
- `src/visuals/city_gate.rs:42-82` — headless-гейт презентации (`compose_sim` + `CityVisualsPlugin` + `init_asset` заглушки).
- `src/menu/mod.rs:12-27` — экран загрузки латиницей, комментарий прямо откладывает шрифт и `strings.ron` до T5.
- `src/camera/mod.rs:70-121` — `follow_player` сглаживает `pivot` (half-life 0.05 с) → после телепорта на ~500 м камера "проедет" через город.
- `src/debug/mod.rs` — фича `debug`, F1/F2.
- `src/main.rs:49-106` — `preflight` сверяет пути ассетов из RON с манифестом.
- Камера: `Camera3d` требует `ColorGrading` (`bevy_render-0.19.1/src/camera.rs:67`), `ColorGradingGlobal::post_saturation` (0 = серый, `bevy_render-0.19.1/src/view/mod.rs:459-463`).

**Проверено исполняемой пробой** (`scratch/probe/`, вывод `scratch/probe_wasted_timing.txt`, bevy 0.19.1 с фичами `gta_sim`):
- `#[derive(Message, Reflect)] #[reflect(Message)]` авто-регистрируется (`reflect_auto_register`), `ReflectMessage::write_message` (путь BRP `world.write_message`, `bevy_remote-0.19.1/src/builtin_methods.rs:1519-1552`) доставляет сообщение в `FixedUpdate`-читатель; от записи до `Wasted` — 2 update.
- `register_type_state::<GameState>()` регистрирует `State<GameState>` (type path `bevy_state::state::resources::State<…::GameState>`).
- `FixedTimesteps(1)` + `set_relative_speed(0.3)`: таймер по `Time<Real>` в `Update` даёт `Playing` ровно через 288 update (= 4.5 / 0.015625), фаза `Screen` через 96 (= 1.5 / 0.015625); фиксированных тиков за это время 86 (≈ 0.3×288) — slow-mo реально замедляет симуляцию.
- `OnTransition { exited: Loading, entered: Playing }` срабатывает один раз и НЕ срабатывает на `Wasted → Playing`.
- `DespawnOnExit(WastedPhase::Screen)` для sub-state работает.

## 2. Approach

1. **Здоровье — `character/`** (GDD §12): компонент `Health { current, armor, since_damage }` (Reflect), маркер `Dead`, `HealthConfig` из `assets/character/health.ron`, чистые ядра `apply_damage` и `regenerate`. Игрок-специфичное (регенерация в стиле V, `DebugDamage`) — в `player/` (зависимость `player → character` уже есть, обратной не вводим).
2. **`DebugDamage` — buffered `Message`** (не observer `Event`): его пишут BRP и debug-клавиша в произвольный момент кадра, читает `FixedUpdate`; буфер не теряется, пока не отработал `FixedMain` (`bevy_time-0.19.1/src/lib.rs:92-98`, `signal_message_update_system`).
3. **Flow владеет смертью и временем** (GDD §3.4): `GameState::Wasted` + sub-state `WastedPhase { SlowMo, Screen }`. `OnEnter(Wasted)` → `Time<Virtual>::set_relative_speed(scale)`; таймер по `Time<Real>` в **`Update`** (осознанное исключение из "таймеры в `FixedUpdate`": под slow-mo `FixedUpdate` крутится 0.3 раза за кадр, и `Time<Real>`-таймер там сломался бы); `OnExit(Wasted)` → скорость 1.0 и возрождение. Slow-mo держится весь `Wasted`, экран появляется через 1.5 с — иначе критерий "восстанавливает 1.0 при выходе" нефальсифицируем (решение в log.jsonl).
4. **Хазард повторного входа**: одноразовые спавны переезжают с `OnEnter(Playing)` на `OnTransition { exited: Loading, entered: Playing }` (игрок и мешевый билд города). Возрождение переиспользует ту же сущность игрока (оружие T6 будет компонентами на ней — сохраняется бесплатно).
5. **Точка у больницы**: чистая функция citygen `sidewalk_anchor(layout, params, building) -> Option<(Vec2, Vec2)>` (точка + единичная касательная тротуара) по той же формуле, что `player_spawn`. Layout и `layout_hash` не меняются → golden не трогаем. Sim кладёт ресурс `HospitalSpawn { point, along }`.
6. **Пикапы — `combat/`** (GDD §12: "пикапы"): аптечка и броня на тротуаре у больницы (`point ± along * spacing`), подбор проверкой расстояния в `FixedUpdate` (2 пикапа × 1 игрок, без сенсоров и слоёв коллизий), кулдаун возрождения пикапа — поле-таймер, без insert/remove.
7. **Розыск 0**: минимальный `wanted::WantedLevel { stars: u8 }` (Reflect, GDD §10.5 называет его для BRP) со сбросом на `OnEnter(Wasted)`. T10 расширяет. Без него пункт критерия "розыск 0" пустой.
8. **Клиент**: `hud/` (полосы здоровья/брони, экран "ПОТРАЧЕНО" поверх обесцвеченного кадра через `ColorGrading.global.post_saturation`), `menu/` грузит `assets/ui/strings.ron` и шрифты, `visuals/pickups.rs` (примитивы), `debug/` F5 → `DebugDamage`, камера сбрасывает `pivot` на выходе из `Wasted`.
9. **Шрифт**: Kenney Fonts отпали (0 кириллических глифов во всех 12 TTF, скан fontTools — log.jsonl). Inter 4.1, SIL OFL 1.1, 248 кириллических глифов, релиз-zip GitHub. OFL прямо разрешает поставку в играх ([SIL Font FAQ](https://software.sil.org/fonts/faq/), [openfontlicense.org](https://openfontlicense.org/)); бинарник не коммитится (только манифест). Схема манифеста получает `OFL` и правило источника "GitHub release".

Подтверждения поведения движка: `Time<Fixed>` следует за `Time<Virtual>` ([docs.rs Fixed](https://docs.rs/bevy/latest/bevy/time/struct.Fixed.html), [Virtual](https://docs.rs/bevy/latest/bevy/time/struct.Virtual.html)) — плюс собственная проба выше. Пикапы у больниц — жанровый референс ([gta.fandom Health](https://gta.fandom.com/wiki/Health), [Hospitals](https://www.grandtheftwiki.com/Hospitals); вторичные вики, R13).

## 3. Steps

Порядок: данные → citygen → sim → тесты sim → манифест/шрифт → клиент → гейт презентации → QA-сценарий.

### A. Данные

**A1. `assets/character/health.ron`** (новый):
```ron
(
    max_health: 100.0,
    max_armor: 100.0,
    regen_delay: 5.0,      // с без урона
    regen_rate: 5.0,       // HP/с
    regen_cap: 0.5,        // доля max_health: реген только ниже и только до неё
    debug_damage: 25.0,    // урон клавиши F5 (фича debug)
    pickups: (health: 50.0, armor: 50.0, radius: 1.0, respawn: 30.0, spacing: 6.0),
)
```
Значения из GDD §3.4; `respawn` (30 с) и `spacing` (6 м) — стартовые данные (в VC аптечка у больницы появляется через 5 с — референс, не закон; 30 с выбрано, чтобы нельзя было "стоять и лечиться").

**A2. `assets/flow/respawn.ron`** (новый): `(wasted_time_scale: 0.3, wasted_slowmo: 1.5, wasted_screen: 3.0)`.

**A3. `assets/ui/strings.ron`** (новый; единственный UI-файл в GDD §12, см. Open questions Q1):
```ron
(
    font: "third_party/inter/Inter-Regular.ttf",
    title_font: "third_party/inter/InterDisplay-Black.ttf",
    loading: "Генерация города (seed {seed})",
    loading_size: 32.0,
    wasted: "ПОТРАЧЕНО",
    wasted_size: 96.0,
    wasted_color: (0.78, 0.08, 0.08),
    wasted_backdrop: (0.0, 0.0, 0.0, 0.35),
    wasted_saturation: 0.0,
    hud: (margin: 24.0, bar_width: 200.0, bar_height: 12.0, bar_gap: 6.0,
          health_color: (0.80, 0.15, 0.15), armor_color: (0.25, 0.45, 0.90), back_color: (0.0, 0.0, 0.0, 0.55)),
)
```
Файл сохранить UTF-8 без BOM.

**A4. `assets/world/render.ron`**: добавить секцию `pickups: (size: 0.5, lift: 0.5, health_color: (0.90, 0.95, 0.90), armor_color: (0.25, 0.45, 0.90))`.

**A5. `assets/third_party/manifest.ron`**: добавить пак (хэши посчитаны по скачанному архиву, `scratch/Inter-4.1.zip`):
```ron
(
    name: "inter",
    version: "4.1",
    page: "https://github.com/rsms/inter",
    url: "https://github.com/rsms/inter/releases/download/v4.1/Inter-4.1.zip",
    archive_sha256: "9883fdd4a49d4fb66bd8177ba6625ef9a64aa45899767dde3d36aa425756b11e",
    license: OFL,
    license_file: "LICENSE.txt",
    files: [
        (archive: "extras/ttf/Inter-Regular.ttf", path: "Inter-Regular.ttf", sha256: "40d692fce188e4471e2b3cba937be967878f631ad3ebbbdcd587687c7ebe0c82"),
        (archive: "extras/ttf/InterDisplay-Black.ttf", path: "InterDisplay-Black.ttf", sha256: "25460b0d5b3afd9764d63cf838f94145af624f8d4c9d20e0f74116be42878b32"),
        (archive: "LICENSE.txt", path: "LICENSE.txt", sha256: "262481e844521b326f5ecd053e59b98c8b2da78c8ee1bdbb6e8174305e54935a"),
    ],
),
```
Установка: `python tools/fetch_assets.py --cache maw/tasks/in_progress/TASK-006/scratch` (архив уже там, сеть не нужна). `.gitignore` уже игнорирует `*.ttf` и `/assets/third_party/*`; проверить `git add -A --dry-run`, что шрифты не попали.

### B. citygen

**B1. `crates/citygen/src/graphs.rs`**: добавить
```rust
/// Sidewalk centre point facing `building` and the unit sidewalk direction there: the midpoint of the
/// non-alley side of the building's block closest to the building centre.
pub fn sidewalk_anchor(layout: &CityLayout, params: &CityParams, building: usize) -> Option<(Vec2, Vec2)>
```
Алгоритм: `block = layout.blocks[layout.lots[b.lot].block]`; `poly = road_polygon(&layout.roads, block)`; по `k` со стороной не `Alley`: `(a, b) = (poly[k], poly[(k+1)%n])`, кандидат `mid = (a+b)/2 + (b-a).normalize().perp() * walk_offset(params, class)`, ключ `geom::dist_point_segment(building.center, a, b)`, ничья → меньший `k`; вернуть `(mid, (b-a).normalize())`. Формула точки идентична `player_spawn` (строка 148) — знак `perp` уже доказан тестом. `None` только если у квартала нет не-alley стороны (лоты требуют фронтажа — `lots.rs:42-46`, так что для зданий не бывает).
**B2. `crates/citygen/src/lib.rs`**: `pub use graphs::sidewalk_anchor;`.
**B3. `crates/citygen/tests/properties.rs`**: тест `hospital_anchor_on_sidewalk` для всех `layouts()`: якорь больницы есть; расстояние до оси ближайшей не-alley дороги в `[half_carriageway, half_carriageway + sidewalk]` (та же проверка, что `player_spawn_on_sidewalk`, строки 268-293); не внутри лотов; `|along| ≈ 1`; расстояние от якоря до центра больницы ≤ длины стороны квартала. Flip-RED: убрать `* walk_offset` (точка на оси дороги) → RED.

### C. Sim (`crates/gta_sim`)

**C1. `src/character/health.rs`** (новый, ~120 строк):
- `pub const HEALTH_CONFIG: &str = "character/health.ron";`
- `HealthConfig` (`Resource, Deserialize, Clone, Debug`, `deny_unknown_fields`) + `PickupConfig`; `validate()`: всё конечно; `max_health > 0`, `max_armor > 0`, `0 < regen_cap <= 1`, `regen_delay >= 0`, `regen_rate >= 0`, `debug_damage > 0`, pickups `health, armor, radius > 0`, `respawn, spacing >= 0`; ошибка называет поле.
- `#[derive(Component, Reflect, Clone, Copy, Debug)] #[reflect(Component)] pub struct Health { pub current: f32, pub armor: f32, pub since_damage: f32 }` + `Health::full(cfg)` (`current = max_health`, `armor = 0`).
- `#[derive(Component, Reflect, Default)] #[reflect(Component)] pub struct Dead;` — редкое событие, archetype move допустим.
- `pub fn apply_damage(current: f32, armor: f32, amount: f32) -> (f32, f32)`: `absorbed = armor.min(amount)`; `((current - (amount - absorbed)).max(0.0), armor - absorbed)`.
- `pub fn regenerate(current: f32, since_damage: f32, dt: f32, cfg: &HealthConfig) -> f32`: cap = `max_health * regen_cap`; если `current <= 0 || current >= cap || since_damage < regen_delay` → `current`; иначе `(current + regen_rate * dt).min(cap)`.
- `#[cfg(test)]` таблицы: броня `(100, 50, 30) → (100, 20)`, `(100, 20, 30) → (90, 0)`, `(100, 0, 30) → (70, 0)`, `(10, 0, 30) → (0, 0)`, `(100, 50, 150) → (0, 0)`; реген `(30, 4.99) → 30`, `(30, 5.0, dt=1/64) → 30.078125`, `(49.99, 5, 1/64) → 50`, `(60, 10) → 60`, `(0, 10) → 0`.
**C2. `src/character/mod.rs`**: `mod health; pub use health::{Dead, Health, HealthConfig, HEALTH_CONFIG, PickupConfig, apply_damage, regenerate};`; `register_type::<Health>().register_type::<Dead>()`; в `drive_characters` добавить `Has<Dead>` в query и ранний выход в начале цикла:
```rust
if dead {
    controller.initiate_action_feeding();
    controller.basis = TnuaBuiltinWalk { desired_motion: Vec3::ZERO, desired_forward: None };
    continue;
}
```
(ввод клиента продолжает писать `MoveIntent`, тело мёртвого стоит).
**C3. `src/flow/mod.rs`**:
- `GameState` + `Reflect`, вариант `Wasted`. Новый `#[derive(SubStates, Default, Clone, PartialEq, Eq, Hash, Debug, Reflect)] #[source(GameState = GameState::Wasted)] pub enum WastedPhase { #[default] SlowMo, Screen }`.
- `pub const RESPAWN_CONFIG: &str = "flow/respawn.ron";` `RespawnConfig { wasted_time_scale, wasted_slowmo, wasted_screen }` + `validate()` (`0 < scale <= 1`, длительности конечны и `>= 0`).
- `#[derive(SystemSet, …)] pub struct PlayingSystems;` — все системы геймплея, которые живут только в `Playing` (здоровье, пикапы).
- `FlowPlugin::build`: `init_state::<GameState>()`, `add_sub_state::<WastedPhase>()`, `register_type_state::<GameState>()`, `register_type_state::<WastedPhase>()`, `configure_sets(FixedUpdate, PlayingSystems.run_if(in_state(GameState::Playing)))`, `configure_sets(Update, WastedSystems.run_if(in_state(GameState::Wasted)))`, плюс системы из C4. Файл ≤ 60 строк, логика в `wasted.rs`.
**C4. `src/flow/wasted.rs`** (новый, ~110 строк):
- `WastedClock(f32)` (Resource).
- `detect_player_death` (`FixedUpdate`, `in_set(PlayingSystems)`, `after` систем урона из C6): `Query<(Entity, &Health), (With<Player>, Without<Dead>)>`; при `current <= 0` → `commands.entity(e).insert(Dead)`, `next.set(GameState::Wasted)`.
- `enter_wasted` (`OnEnter(Wasted)`): `Time<Virtual>::set_relative_speed(cfg.wasted_time_scale)`, `WastedClock(0.0)`.
- `advance_wasted` (`Update`, `in_set(WastedSystems)`): `clock += Res<Time<Real>>::delta_secs()`; `SlowMo && clock >= wasted_slowmo` → `NextState<WastedPhase>::set(Screen)`; `clock >= wasted_slowmo + wasted_screen` → `NextState<GameState>::set(Playing)`. Комментарий одной строкой: почему `Update` и `Time<Real>`.
- `OnExit(Wasted)`: `restore_time` (`set_relative_speed(1.0)`) и `respawn_player`: для `(Entity, &mut Position, &mut Transform, &mut LinearVelocity, &mut Health, &CharacterBody)` с `With<Player>`: `Position = HospitalSpawn.point + Y*float_height`, `Transform.translation` то же, `LinearVelocity = ZERO`, `Health::full(cfg)` (броня на момент смерти всегда 0: броня поглощает первой), `commands.entity(e).remove::<Dead>()`. Две отдельные системы — без `too_many_arguments`.
**C5. `src/world/mod.rs` + `city.rs`**:
- `#[derive(Resource, Reflect)] #[reflect(Resource)] pub struct HospitalSpawn { pub point: Vec3, pub along: Vec3 }`; `WorldPlugin::build` вставляет `HospitalSpawn { point: ZERO, along: X }` рядом с `PlayerSpawn(ZERO)` и `register_type::<HospitalSpawn>()` (TestArea: больница = точка спавна; пикапы на `(±6, 0, 0)`, свободно от блоков `test_area.rs:6-19`).
- `apply_city_generation`: после `spawn_buildings` — хелпер `hospital_spawn(&layout, &params.0, curb) -> Result<HospitalSpawn, String>` (ищет `BuildingKind::Hospital`, зовёт `citygen::sidewalk_anchor`); ошибка идёт в существующий путь `error! + AppExit::error()`. Точка `Vec3::new(p.x, curb, p.y)`, `along = Vec3::new(t.x, 0, t.y)`.
**C6. `src/player/mod.rs`**:
- `spawn_player` → `add_systems(OnTransition { exited: GameState::Loading, entered: GameState::Playing }, spawn_player)`; бандл + `Health::full(&health_cfg)` (новый `Res<HealthConfig>`; все харнессы идут через `compose_sim`, который его вставляет — grep `spawn_player` вне плагина пуст).
- `#[derive(Message, Reflect, Clone, Copy, Debug)] #[reflect(Message)] pub struct DebugDamage { pub amount: f32 }` — урон игроку, "для отладки и BRP `world.write_message`". Type path для QA: `gta_sim::player::DebugDamage`. `add_message::<DebugDamage>()`, `register_type::<DebugDamage>()`.
- `apply_debug_damage` (`FixedUpdate`, `PlayingSystems`): читает все сообщения; `amount` не конечный или `<= 0` → `warn!` и пропуск; иначе на `Query<&mut Health, (With<Player>, Without<Dead>)>`: `apply_damage`, `since_damage = 0`.
- `regenerate_player` (`FixedUpdate`, `PlayingSystems`, `.after(apply_debug_damage)`): `since_damage += Time<Fixed>::delta_secs()`, затем `current = regenerate(current, since_damage, dt, cfg)`. Порядок "сначала `+=`, потом проверка" фиксирован — от него выведены числа тестов.
- Цепочка: `(apply_debug_damage, regenerate_player).chain()`; `flow::detect_player_death.after(regenerate_player)` и `after(combat::collect_pickups)`.
**C7. `src/combat/mod.rs` + `combat/pickups.rs`** (новые, ~90 строк):
- `#[derive(Reflect, Clone, Copy, Debug, PartialEq, Eq)] pub enum PickupKind { Health, Armor }`; `#[derive(Component, Reflect)] #[reflect(Component)] pub struct Pickup { pub kind: PickupKind, pub cooldown: f32 }` + `fn available(&self) -> bool { self.cooldown <= 0.0 }`.
- `spawn_pickups` на `OnTransition{Loading→Playing}`: `Health` на `point + along*spacing`, `Armor` на `point - along*spacing`, `Transform::from_translation`, `Name`.
- `collect_pickups` (`FixedUpdate`, `PlayingSystems`, после `regenerate_player`): тикает `cooldown -= dt`; для доступных пикапов: ноги игрока `Position - Y*float_height`, расстояние `<= radius` → `Health`: если `current < max_health` → `current = (current+health).min(max)`, `cooldown = respawn`; `Armor`: если `armor < max_armor` → аналогично. Полный запас → пикап не тратится.
- `CombatPlugin` регистрирует типы и системы.
**C8. `src/wanted/mod.rs`** (новый, ~25 строк): `#[derive(Resource, Reflect, Default)] #[reflect(Resource)] pub struct WantedLevel { pub stars: u8 }`; `WantedPlugin`: `init_resource`, `register_type`, `OnEnter(GameState::Wasted)` → `stars = 0`. Doc-комментарий: T10 владеет heat/звёздами.
**C9. `src/lib.rs`**: `pub mod combat; pub mod wanted;`; в `compose_sim` загрузить и провалидировать `HealthConfig` и `RespawnConfig` по образцу `LocomotionConfig` (ошибка с путём файла), вставить ресурсы; добавить `CombatPlugin`, `WantedPlugin` в кортеж плагинов.

### D. Headless-гейты sim (`cargo test -p gta_sim`)

**D1. `tests/common/mod.rs`**: хелперы `health(app) -> Health`, `write_damage(app, amount)` (`app.world_mut().write_message(DebugDamage { amount })`), `game_state(app)`.
**D2. `tests/health.rs`** (TestArea, `headless_app()` + `settle`):
1. `armor_absorbs_then_health` (корректность): `Health.armor = 50` напрямую; `write_damage(30)`, `run_ticks(1)` → `(100, 20)`; ещё 30 → `(90, 0)`; ещё 30 → `(60, 0)`. Flip-RED: поменять порядок в `apply_damage` (сначала здоровье) → RED.
2. `regen_waits_then_stops_at_cap` (корректность, числа выведены из C6): `write_damage(70)`, `run_ticks(1)` (тик урона: `since = 1/64`) → 30; `run_ticks(318)` → 30 (since = 319/64 < 5); `run_ticks(1)` → `30.078125 ± 1e-4` (since = 320/64 = 5, +5/64); `run_ticks(64*10)` → ровно 50.0 и за весь прогон `max ≤ 50.0`. Второй случай: урон 40 → 60, `run_ticks(64*10)` → 60. Flip-RED: убрать `.min(cap)` → RED; поставить проверку `since` до `+=` → RED на границе 318/319.
3. `pickups_heal_armor_and_respawn`: урон 70 → 30; `place_player` на аптечку (`HospitalSpawn.point + along*spacing + Y*float_height`), `run_ticks(2)` → 80, `cooldown > 0`; отойти и вернуться → остаётся 80 (пикап неактивен); `place_player` на броню → armor 50; при `current == max` аптечка не тратится (отдельная проверка после реген-сброса через прямую запись `current = max`). Через `respawn*64 + 2` тиков пикап снова `available()`. Flip-RED: убрать проверку `cooldown` → RED.
4. `debug_damage_through_reflection` (liveness пути BRP): как в пробе — `AppTypeRegistry` → `ReflectMessage` для `DebugDamage`, `DynamicStruct { amount: 20 }` → `write_message`; `run_ticks(1)` → 80. Flip-RED: убрать `#[reflect(Message)]` → RED (нет type data).
5. `dead_player_cannot_walk`: урон 1000, update до `Wasted`; `set_intent(axis = Y)`; 64 update → горизонтальный сдвиг < 0.1 м. Flip-RED: убрать ветку `if dead` в `drive_characters` → RED.
**D3. `tests/respawn.rs`** (город, `city_app(1)` + `settle`):
1. `death_wasted_respawn_at_hospital` (корректность): запомнить сущность игрока, вставить тестовый `#[derive(Component)] struct Loadout(u32)` (прокси "оружие сохранено": T6 кладёт оружие компонентами на игрока), `WantedLevel.stars = 3`. `write_damage(1000)`; update до `Wasted` (≤ 3). Проверки в `Wasted`: `Time<Virtual>::relative_speed() == wasted_time_scale`, есть `Dead`, `WantedLevel == 0`. Счёт update `k` до наблюдения `WastedPhase::Screen` == `ceil(wasted_slowmo / dt)` (= 96), до `GameState::Playing` == `ceil((wasted_slowmo + wasted_screen) / dt)` (= 288), где `dt = Time<Fixed>::timestep()` (выведено: update входа в `Wasted` уже прибавляет первый `dt`, `next.set` при `(k+1)·dt ≥ T`, переход наблюдается на следующем update; проба дала 96/288). После: `relative_speed == 1.0`, та же `Entity`, `Loadout(7)` на месте, `Health == full`, `Dead` снят; горизонтальное расстояние до `HospitalSpawn.point` < 0.05 м; `settle`-подобная проверка через 64 тика: стоит на тротуаре (y = point.y + float_height ± 0.05); `HospitalSpawn` на тротуаре квартала больницы (та же проверка расстояния до оси, что `player_spawns_on_sidewalk`).
   Flip-RED: (a) таймер на `Res<Time>` (виртуальное) → k≈960 → RED; (b) убрать `set_relative_speed(1.0)` из `OnExit` → RED; (c) деспавн/респавн игрока вместо телепорта → RED (`Loadout` пропал).
2. `respawn_keeps_world_one_shot` (корректность хазарда, sim-сторона): до смерти посчитать `With<Player>`, `CityBuilding`, `CityBlock`, `CityGround`, `CityEdgeWall`, `Pickup`; полный цикл смерть→`Playing` + 64 тика; числа равны (`Player == 1`). Flip-RED: вернуть `spawn_player`/`spawn_pickups` на `OnEnter(Playing)` → `Player == 2` → RED.
**D4. `tests/config.rs`**: `shipped_health_config_loads` и `shipped_respawn_config_loads` (load + validate); `health_regen_cap_is_validated` по образцу `locomotion_thresholds_are_validated` (заменить `regen_cap: 0.5,` на `regen_cap: 1.5,` во временной копии, ошибка содержит `regen_cap`; `GATE BROKEN`, если в файле нет исходной строки).

### E. Манифест и загрузчик ассетов

**E1. `crates/gta_sim/src/config/manifest.rs`**: `AssetLicense { CC0, OFL }`; в `AssetPack::validate` заменить блок page/url (строки 85-98) на `check_source(name)`: если `page` начинается с `https://kenney.nl/` — текущие правила Kenney без изменений; иначе правило GitHub-релиза: `page == https://github.com/{owner}/{repo}` (ровно два сегмента, символы `[A-Za-z0-9._-]`), `url` начинается с `{page}/releases/download/` и кончается `.zip`; иначе `Err("pack {name}: url … / page …")` (слово `url` в тексте — `bad_url.ron` продолжает проходить). Добавить `pub fn license_marker(self) -> &'static str` (CC0 → `"CC0"`, OFL → `"SIL Open Font License"`).
**E2. `tools/fetch_assets.py`**: зеркало E1 в `check_schema` (строки 200-206): ветки Kenney/GitHub, `license in ("CC0", "OFL")`. Докстринг упомянуть GitHub-релизы.
**E3. `crates/gta_sim/tests/asset_manifest.rs`**: `shipped_manifest_is_valid` — множество имён + `"inter"`; фильтр 4-файловых паков исключает и `inter`; `inter`: 3 файла, `license == OFL`, `rig.is_none()`. `local_assets_match_manifest` (строки 188-199): иглы по `pack.license.license_marker()` вместо жёстких CC0-игл для всех (для CC0 оставить обе прежние иглы). Новые фикстуры: `valid_github_ofl.ron` (в список valid), `bad_github_url.ron` (github page, url чужого репо → keyword `url`). Flip-RED: временно вернуть Kenney-only проверку → `valid_github_ofl` RED.

### F. Клиент (`src/`)

**F1. `src/visuals/city.rs:24`**: `OnEnter(GameState::Playing)` → `OnTransition { exited: GameState::Loading, entered: GameState::Playing }`. Одна строка + импорт.
**F2. `src/visuals/city_gate.rs`**: тест `city_is_built_once_across_respawn` (гейт презентации, корректность): `city_visuals_app(1)`; посчитать `(With<CityChunk>, With<Mesh3d>)` (100) и `With<CityProp>`; `write_message(DebugDamage { amount: 1000 })`; update до `Wasted`, затем до `Playing`, затем ещё update, пока `CityMeshTask`/`PendingCitySpawn` существуют, плюс 10 update; ассерты: `CityMeshTask` ни разу не появился после первого билда (флаг в цикле), счёт чанков и пропов не изменился. Flip-RED: откатить F1 → 200 чанков → RED (цикл ждёт завершения второго билда — контрпример premise-challenge закрыт).
**F3. `src/menu/config.rs`** (новый): `pub const UI_CONFIG: &str = "ui/strings.ron"`; `UiConfig` + `HudLayout` (`deny_unknown_fields`, `Resource`); `validate()`: строки непусты, `loading` содержит `{seed}`, размеры > 0, цвета в `[0,1]`, `wasted_saturation >= 0`; `font_paths() -> [&str; 2]`.
**F4. `src/menu/mod.rs`**: `pub use config::…`; `#[derive(Resource)] pub struct UiFonts { pub regular: Handle<Font>, pub title: Handle<Font> }` через `FromWorld` (`AssetServer::load`), `init_resource` в `build` (экран загрузки стартует на первом `StateTransition` раньше `Startup`, поэтому не `Startup`-система — тот же приём, что `CharacterAnimations`). `spawn_loading_screen`: `Text::new(ui.loading.replace("{seed}", &seed.0.to_string()))` + `TextFont { font: FontSource::Handle(fonts.regular.clone()), font_size: FontSize::from(ui.loading_size), ..default() }`. Удалить устаревший комментарий строки 12.
**F5. `src/hud/mod.rs`** (новый, ~90 строк): `HudPlugin`; `spawn_hud` на `OnTransition{Loading→Playing}`: абсолютный `Node` справа сверху (`top/right: px(margin)`, `flex_direction: Column`, `row_gap: px(bar_gap)`), две подложки `px(bar_width)×px(bar_height)` с `BackgroundColor(back_color)`, внутри заливка с маркером `HudFill(HudBar::Health|Armor)`; `update_bars` (`Update`): `Query<&Health, (With<Player>, Changed<Health>)>` → `node.width = percent(100 * current / max)` (max из `Res<HealthConfig>`).
**F6. `src/hud/wasted.rs`** (новый, ~60 строк): `OnEnter(WastedPhase::Screen)` → полноэкранный `Node` с `BackgroundColor(wasted_backdrop)`, центрированный `Text(wasted)` с `title`-шрифтом, `TextColor(wasted_color)`, `DespawnOnExit(WastedPhase::Screen)`; `OnEnter(GameState::Wasted)` → `ColorGrading.global.post_saturation = wasted_saturation` на `Camera3d` (`bevy::render::view::ColorGrading`); `OnExit(GameState::Wasted)` → `ColorGradingGlobal::default().post_saturation`.
**F7. `src/visuals/pickups.rs`** (новый, ~50 строк) + `visuals/config.rs`: `PickupVisuals { size, lift, health_color, armor_color }` в `RenderConfig` (валидация `size, lift > 0`); наблюдатель `On<Add, Pickup>` → дочерний куб `Cuboid::from_length(size)` на высоте `lift` с цветом по виду; `Update`-система: `Visibility::Hidden`, пока `!pickup.available()`, через `Changed<Pickup>` не делать (кулдаун меняется каждый тик) — просто 2 сущности каждый кадр. Добавить в `VisualsPlugin`.
**F8. `src/camera/mod.rs`**: система на `OnExit(GameState::Wasted)`: `orbit.pivot = None` (после телепорта камера встаёт сразу, без пролёта через город).
**F9. `src/debug/mod.rs`**: в `toggle_debug` (или отдельной системе `Update`) `F5` → `MessageWriter<DebugDamage>::write(DebugDamage { amount: health.debug_damage })`. Читать ввод в `Update` — правило "не читать `just_pressed` в `FixedUpdate`" соблюдено.
**F10. `src/main.rs`**: `mod hud;`; загрузить `UiConfig` (тот же `match`-паттерн); `preflight` получает `&UiConfig`: `validate()` и проверка `manifest.contains_asset` для обоих шрифтов (ошибка `"{UI_CONFIG}: font {path} is not listed in {THIRD_PARTY_MANIFEST}"`); `insert_resource(ui_config)` до `add_plugins`; добавить `hud::HudPlugin`.

### G. QA

**G1. `tools/qa/scenarios/t5.py`** (по образцу `t3.py`, `--out`, `release=True`, `--seed 1`):
1. `fetch_assets.py --check`; `wait_resource("CityLayoutHash")` == golden; дождаться 100 стабильных `CityChunk`.
2. Прочитать `HospitalSpawn`, строки `Pickup` (+`Transform`), `Health` игрока == `(100, 0)`.
3. `world.write_message {"message": "gta_sim::player::DebugDamage", "value": {"amount": 40.0}}` → через 0.5 с `current == 60`; скриншот `hud_damaged.png`.
4. Телепорт на броню (`Position` = пикап + Y·1.2) → armor 50; урон 30 → `(60, 20)` (поглощение в рантайме).
5. Урон 1000 → опрос `State<gta_sim::flow::GameState>` до `Wasted` (≤ 2 с); `sleep(2.0)` (> 1.5 с) → скриншот `wasted.png` ("ПОТРАЧЕНО" на сером кадре), `State<gta_sim::flow::WastedPhase>` == `Screen`. Форму значения `State<…>` в JSON разбирать защитно (строка или обёртка) и печатать сырое значение в summary.
6. Опрос до `Playing` (таймаут 10 с), записать реальное время (ожидание ≈ 4.5 с, мягкий ассерт 3.5–8 с). Позиция игрока горизонтально < 1.0 м от `HospitalSpawn.point`, `Health == (100, 0)`; скриншот `respawned.png`.
7. Через 5 с: `CityChunk == 100`, строк `Player == 1` (рантайм-проверка хазарда).
8. Лог без `ERROR` со словами `font`/`asset`/`Failed to load`; `shutdown`; `summary.json`.
**G2. Owner-чеклист для QA_REPORT.md** (QA-агент вписывает): `python tools/fetch_assets.py`; `cargo run --features fast,debug`; F5 несколько раз — красная полоса убывает; подобрать броню у больницы — синяя полоса, F5 сначала съедает броню; дождаться регена ниже 50% — останавливается на половине; умереть — slow-mo читается, кадр серый, "ПОТРАЧЕНО" видно и не режется, кириллица рендерится; возрождение у больницы, камера на месте, город не мигает и не двоится; экран загрузки по-русски.

### H. Проверка готовности

`cargo build`, `cargo clippy -- -D warnings` (и `--features dev,debug`), `cargo test -p gta_sim -p citygen`, `cargo test -p gta_like --bin gta_like`, `python tools/qa/tree_check.py`, `cargo tree -p gta_sim -e normal -i bevy_render` пуст, `python tools/fetch_assets.py --check`, `python tools/qa/scenarios/t5.py --out target/qa/t5`. Каждый новый гейт — flip-RED с записью, какой вход ломали (списки выше).

## 4. Risk areas

- **Порядок `FixedUpdate`-систем**: урон → реген → пикапы → смерть. Сломанный порядок даёт "воскрешение" регеном в тике смерти (реген пропускает `current <= 0`, но проверить тестом D2.2/D3.1).
- **Несколько фиксированных тиков в кадре**: `Dead` вставляется командой; до применения команд в том же `FixedUpdate`-проходе следующий тик может снова ударить по трупу — безвредно (`max(0)`), `next.set` идемпотентен.
- **Телепорт avian/Tnua**: запись `Position` + `Transform` + обнуление `LinearVelocity` — тот же приём, что `place_player` и QA `t3.py`; `TransformInterpolation` может дать один тик "шлейфа" — под экраном смерти не виден. Если Tnua держит память опоры и тело подпрыгивает — тест D3.1 (стоит через 64 тика) поймает.
- **Пикапы в TestArea на `(±6, 0, 0)`**: движение в `movement.rs` (≤ 4.5 м по −X за 64 тика) не доходит до радиуса 1 м вокруг `x = −6`; подбор не влияет на движение в любом случае (нет коллайдера).
- **Пропы клиента на тротуаре** (фонари `lamp_curb_offset`) могут визуально стоять на пикапе — коллайдеров у пропов нет, только вид; владелец/QA увидит.
- **BRP-формы**: путь `gta_sim::player::DebugDamage` зависит от модуля объявления (проба: type path = путь модуля); сериализация `State<GameState>` не проверена вживую — t5.py печатает сырое значение.
- **`Health` в `world.list_components`**: `component_path("Health")` требует уникального суффикса `::Health`; при коллизии QA упадёт громко.
- **Шрифт не загрузился** → текст невидим без паники. Ловится preflight (файл в манифесте и на диске) и скриншотом `wasted.png`.
- **Скорость загрузки GitHub-релиза** (33.7 МБ): `fetch_assets` уже ретраит; zip лежит в `scratch/` для `--cache`.
- **Регресс клиентских гейтов**: `character_gate`/`city_gate` строятся через `compose_sim` — новые ресурсы (`HealthConfig`, `RespawnConfig`) приходят автоматически; `city_gate` ждёт `Playing` — игрок теперь спавнится на `OnTransition` того же кадра.
- **Размер файлов**: все новые < 150 строк; `visuals/config.rs` 340 → ~360.

## 5. Open questions

Решения по умолчанию указаны; реализация не блокируется.

- **Q1. Где лежат числа HUD и экрана смерти.** Варианты: (A) в `assets/ui/strings.ron` рядом со строками и шрифтами — единственный UI-файл из GDD §12; минус: файл называется "strings", а хранит и раскладку. (B) новый `assets/ui/hud.ron` + правка списка файлов GDD §12 в этой задаче. **По умолчанию A.**
- **Q2. Анимация смерти игрока.** Сейчас тело стоит (idle) под slow-mo. Варианты: (A) без анимации в T5, `die` добавит T8 для всех персонажей; (B) `AnimState::Dead` + клип `die` уже в T5 (правит клиентскую таблицу из 6 клипов, `visual.ron`, гейты T4). **По умолчанию A**, пункт в чеклисте владельца.
- **Q3. Где пикапы.** (A) аптечка и броня обе у больницы (владелец видит их сразу после первого возрождения); (B) броня у участка, как в III-эпохе. **По умолчанию A.**
- **Q4. Клавиша отладочного урона.** F1-F4 заняты по GDD §3.3. **По умолчанию F5** под фичей `debug`, урон `debug_damage` из `health.ron`.

## Приложение: артефакты

- `scratch/probe/` + `scratch/probe_wasted_timing.txt` — проба тайминга `Wasted`, `ReflectMessage`, `OnTransition`, `DespawnOnExit` для sub-state.
- `scratch/Inter-4.1.zip` (sha256 `9883fdd4…b11e`) — архив для `fetch_assets.py --cache`.
- `scratch/kenney_kenney-fonts.zip` — отвергнутый кандидат (0 кириллических глифов).
- Источники: [SIL Font FAQ](https://software.sil.org/fonts/faq/), [OFL](https://openfontlicense.org/), [Inter v4.1](https://github.com/rsms/inter/releases/tag/v4.1), [docs.rs bevy Fixed](https://docs.rs/bevy/latest/bevy/time/struct.Fixed.html), [docs.rs bevy Virtual](https://docs.rs/bevy/latest/bevy/time/struct.Virtual.html), [gta.fandom Health](https://gta.fandom.com/wiki/Health), [grandtheftwiki Hospitals](https://www.grandtheftwiki.com/Hospitals).

children: 0 launched / 0 reported.
