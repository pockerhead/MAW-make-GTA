# PLAN_FINAL — TASK-003 (GDD T2): генератор города v1 и прогулка

Источник истины по объёму: `docs/design/GDD.md` §2, §7, §10.5, §11, §12, §13 T2 и `TASK_FINAL.md`
(Resolved questions Q1=B невидимые стены, Q2 упрощения v1 приняты, Q3 латинская строка загрузки).
Все API Bevy 0.19.1 / avian3d 0.7.0 / bevy_state / bevy_tasks / bevy_remote / rand_core 0.10.1 /
rand_chacha 0.10.0 / glam 0.32.1 ниже сверены по исходникам в
`~/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/` (версии из `Cargo.lock`), ссылки `файл:строка`.

Цена ошибки смешанная. Молча ломаются и получают исполняемые гейты: детерминизм генерации (golden-хэш),
связность графа дорог и сильная связность графа полос (на них строятся T8/T15), непересечение лотов и
выход к улице, наличие больницы/участка/двух штабов (от них зависят T5, T9, T11), проводка
seed → layout → `CityLayoutHash` в рантайме, стены края, бюджет генерации. Облик города, "кварталы разной
высоты", парки, вид с земли, возможный однокадровый "поп" мешей при снятии экрана загрузки владелец видит
на первом кадре: это его прогон (owner checklist в QA_REPORT), гейтов под это не строим.

---

## 1. Summary

Строим крейт `citygen` — чистую детерминированную функцию
`generate(seed: u64, &CityParams) -> Result<CityLayout, GenError>` без Bevy и глобального RNG (сетка
ядро/окраина без узких остаточных кварталов, jitter узлов, суперкварталы/переулки по паросочетанию граней,
кварталы с отступами проезжей части и тротуара, районы Voronoi с гарантией двух территорий банд, парки,
лоты рекурсивным OBB-делением с frontage к живой не-переулочной улице, здания-коробки, POI, граф тротуаров,
граф направленных полос, точка спавна на тротуаре) и канонический FNV-1a-хэш `layout_hash`. Все числа
генератора и высота стен края лежат в `assets/world/city.ron` со строгим загрузчиком и `validate()`.
В `gta_sim` появляется домен `flow/` (`GameState { Loading, Playing }`), `world/` получает источник мира
`WorldSource::{TestArea, City { seed }}`, асинхронную генерацию по маркерному паттерну
(`AsyncComputeTaskPool` → `block_on(poll_once)`), ресурсы `City`, `CitySeed`, `CityLayoutHash` (Reflect,
для BRP), статические коллайдеры земли, зданий и 4 невидимые стены края; `spawn_player` переезжает на
`OnEnter(Playing)`. T1-гейты остаются на тестовой площадке (`WorldSource::TestArea`) через ту же
`compose_sim`. Клиент парсит `--seed N`, показывает экран загрузки (`menu/`), рисует простые меши (земля,
тротуары, газоны, коробки цвета района/POI) по палитре из `assets/world/render.ron`. Гейты: golden для
seed 1/2/42, property-тесты на 35 seed, рантайм-гейты в headless `App`, `#[ignore]` бюджеты, BRP-сценарий
`tools/qa/scenarios/t2.py`.

---

## 2. Текущее состояние (проверено по коду)

- `/Cargo.toml:1-51`: члены `crates/gta_sim`, `crates/citygen`; пины `bevy =0.19.1`, `avian3d =0.7.0`,
  `bevy-tnua =0.32.0`, `ron =0.12.2`, `serde 1`; патчи `vendor/` (ADR-001); `profile.dev` opt-level 1
  для членов, 3 для зависимостей.
- `crates/citygen`: пустой (`Cargo.toml:1-6` без зависимостей, `src/lib.rs:1` одна строка `//!`).
- `crates/gta_sim/Cargo.toml:9`: bevy-фичи `std, multi_threaded, bevy_state, bevy_log, bevy_asset,
  serialize, reflect_auto_register` — `bevy::state` доступен (`bevy_internal-0.19.1/src/lib.rs:93-94`).
- `crates/gta_sim/src/lib.rs:16-26` `compose_sim(app, root)`: `LocomotionConfig`, `ConfigRoot`,
  `PhysicsPlugins`, `TnuaAvian3dPlugin::new(FixedUpdate)`, `CharacterPlugin`, `WorldPlugin`, `PlayerPlugin`.
  Вызывающие: `src/main.rs:28` и `crates/gta_sim/tests/common/mod.rs:25` (grep, других нет).
- `crates/gta_sim/src/config/mod.rs:5-38`: `ConfigRoot`, `ConfigError { path, message }`,
  `load_config<T: DeserializeOwned>`.
- `crates/gta_sim/src/world/mod.rs:1-24`: `Block { size }` (Reflect), `PlayerSpawn(Vec3)` = ZERO,
  `Startup: spawn_test_area`; `pub use test_area::spawn_test_area` (единственный внешний пользователь —
  `player/mod.rs:2,14`). `world/test_area.rs:5` комментарий "citygen replaces this table in T2".
- `crates/gta_sim/src/player/mod.rs:14`: `Startup: spawn_player.after(spawn_test_area)`.
- Тесты `crates/gta_sim/tests/`: `common/mod.rs:21-29` `headless_app()` (`MinimalPlugins`,
  `TransformPlugin`, `AssetPlugin`, `TimeUpdateStrategy::FixedTimesteps(1)`, `compose_sim`,
  `finish/cleanup`), `run_ticks` считает тики через `Time<Fixed>`, `settle()` проверяет y ≈ float_height;
  `terrain.rs` завязан на коробку (10,0.5,10) и лестницу у z≈−12 тестовой площадки, `place_player`
  (terrain.rs:7-14) меняет и `Position`, и `Transform`.
- Клиент: `src/main.rs:18-58` (`DefaultPlugins` + `compose_sim` + ввод/камера/визуал, ошибки конфигов →
  `eprintln!` + `AppExit::error()`), `--seed` не парсится. `src/visuals/mod.rs:49-63` observer
  `On<Add, Block>` создаёт меш и **новый материал на каждую сущность**. `src/camera/mod.rs:25` камера
  спавнится в `Startup` без зависимости от игрока; `follow_player` (`:74`) и `write_move_intent`
  (`src/input/mod.rs:94`) берут `Single<.., With<Player>>` и при отсутствии игрока пропускаются
  (`bevy_ecs-0.19.1/src/system/system_param.rs:371-381`).
- `assets/world/render.ron`: только свет (4 поля), `city.ron` нет.
- QA: `tools/qa/brp.py` (`Game`, `call`, `component_path`, `query`, `screenshot`, `diagnostics`,
  `shutdown`, `vec3` принимает и dict, и list); `tools/qa/scenarios/t1.py` ждёт голые 2 с, потом W 1000 мс,
  ожидает смещение по −Z > 2 м и |x| < 0.5.
- Lock: `scratch/Cargo.lock.t2` отличается от текущего `/Cargo.lock` ровно диффом
  `scratch/cargo_lock_diff.txt` (проверено `diff` в этом ревью: зависимости `citygen`, `citygen` в
  `gta_sim`, пакеты `ppv-lite86 0.2.21`, `rand_chacha 0.10.0`) — актуален.

Проверенные факты движка, на которые опирается план:
- `init_state` паникует без расписания `StateTransition` (`bevy_state-0.19.1/src/app.rs:102-104`);
  `StatesPlugin` есть в `DefaultPlugins` (`bevy_internal-0.19.1/src/default_plugins.rs:90-91`), в
  `MinimalPlugins` нет → тестовый харнесс добавляет `bevy::state::app::StatesPlugin` **до** `compose_sim`.
- `StatesPlugin` ставит `StateTransition` перед `PreStartup` и после `PreUpdate`
  (`bevy_state-0.19.1/src/app.rs:333-336`); стартовые расписания исполняются внутри первого
  `app.update()` (`bevy_app-0.19.1/src/main_schedule.rs:290-305`). Значит `OnEnter(Loading)` идёт в
  первом `update()` до `Startup`, а `NextState(Playing)`, выставленный там, применяется в том же первом
  `update()` в пост-`PreUpdate` `StateTransition`.
- `DespawnOnExit<S>` (`state_scoped.rs:149`) включается `init_state` автоматически и исполняется в
  `StateTransition` в наборе `ExitSchedules` (`app.rs:244-265`) — в том же прогоне `StateTransition`,
  где идут `OnEnter` нового состояния.
- `AsyncComputeTaskPool` (`bevy_tasks-0.19.1/src/lib.rs:82`, `usages.rs`), `spawn`
  (`task_pool.rs:559`), `block_on` (`lib.rs:107`), `poll_once` (`lib.rs:85`).
- `NextState::set` (`bevy_state-0.19.1/src/state/resources.rs:198`), `in_state` (`condition.rs:103`),
  `AppExit` — `Message` (`bevy_app-0.19.1/src/app.rs:1554`), `App::should_exit` (`app.rs:1421`).
- avian `transform_to_position` не перетирает `Position`, изменённый после прошлого физ-тика
  (`avian3d-0.7.0/src/physics_transform/mod.rs:187-230`), `position_to_transform` затем пишет `Transform`
  (`:256-271`). `Position` — `Reflect` + `reflect(Component)` (`physics_transform/transform.rs:44-48`).
- BRP: `world.mutate_components` params `{entity, component, path, value}`
  (`bevy_remote-0.19.1/src/builtin_methods.rs:285-300`); `world.get_resources` params `{resource}`
  (`:140-145`), ответ `{"value": ...}` (`:482-487`); `world.list_resources` (`:90`).
- `rand_core 0.10.1`: трейт `Rng` (`lib.rs:49`), `SeedableRng::seed_from_u64` объявлен value-stable
  (`seedable_rng.rs:105-109`); `rand_chacha 0.10.0` реэкспортирует `rand_core` (`lib.rs:93`).
- glam 0.32.1: при `std` и `nostd-libm` одновременно используется `std`-математика
  (`src/f32/math.rs:35,164` cfg). Утверждение PLAN.md "в двух сборках трансцендентные функции разные"
  неверно для одной машины; запрет `sin/cos/atan2/powf/exp` в citygen остаётся как дешёвая гигиена
  межплатформенной переносимости.
- Меш: `Mesh::new(PrimitiveTopology, RenderAssetUsages)` (`bevy_mesh-0.19.1/src/mesh.rs:341`),
  `PrimitiveTopology` (`mesh.rs:2`, путь `bevy::mesh::PrimitiveTopology`), `Indices`,
  `bevy::asset::RenderAssetUsages` (`bevy_asset-0.19.1/src/lib.rs:200`), `Cuboid::from_size`
  (`bevy_math-0.19.1/src/primitives/dim3.rs:716`); UI `Node`, `BackgroundColor`
  (`bevy_ui-0.19.1/src/ui_node.rs:492,2229`), `Text` (`widget/text.rs:111`).

---

## 3. Implementation steps

Работать в текущем checkout. `git worktree`, отдельный `target/`, `cargo update` запрещены. После
стрелки "→" — проверка шага.

### Шаг 1. Зависимости и lock

Файлы: `crates/citygen/Cargo.toml`, `crates/gta_sim/Cargo.toml`, `/Cargo.lock`.

`crates/citygen/Cargo.toml` — добавить ровно:
```toml
[dependencies]
glam = { version = "=0.32.1", default-features = false, features = ["std"] }
rand_chacha = { version = "=0.10.0", default-features = false }
serde = { workspace = true }

[dev-dependencies]
ron = { workspace = true }
```
`crates/gta_sim/Cargo.toml` `[dependencies]`: `citygen = { path = "../citygen" }`. Клиенту новая
зависимость не нужна (типы citygen реэкспортирует `gta_sim::world`).

Затем скопировать `maw/tasks/in_progress/TASK-003/scratch/Cargo.lock.t2` поверх `/Cargo.lock`
(offline-резолв в этой песочнице падает на `nix`, см. PCTX_PROPOSALS; lock разрешён онлайн заранее).
glam 0.32.1 — та же версия, что у `bevy_math` 0.19.1 (`Cargo.lock:1455`), поэтому `glam::Vec2` ==
`bevy::math::Vec2`.

→ `git diff Cargo.lock` совпадает с `scratch/cargo_lock_diff.txt`; `cargo tree --offline --locked -p citygen`
резолвится; `cargo tree --offline --locked -p gta_sim -e features -i bevy_render` пуст. Если набор
зависимостей пришлось изменить — остановиться и сообщить (новый lock требует сети).

### Шаг 2. `citygen`: параметры, типы, RNG, геометрия

**`crates/citygen/src/lib.rs`** — модули `params, layout, rng, geom, grid, roads, districts, lots, pois,
graphs, hash`; публичный API: `CityParams` (+ вложенные), `CityLayout` и типы вывода, `GenError`,
`generate`, `layout_hash`; `pub use glam::Vec2`.

**`src/params.rs`** — все структуры `#[derive(Deserialize, Clone, Debug)] #[serde(deny_unknown_fields)]`:
```rust
pub struct CityParams { size: f32, ground_margin: f32, edge_wall: EdgeWallParams, grid: GridParams,
    roads: RoadParams, superblock_chance: f32, alley_chance: f32, districts: DistrictsParams,
    split_jitter: f32, min_building_area: f32, pois: PoiParams }            // поля pub
pub struct EdgeWallParams { height: f32, thickness: f32 }
pub struct GridParams { core_radius: f32, core_block: (f32, f32), outer_block: (f32, f32),
    node_jitter: f32, avenue_every: u32 }
pub struct RoadParams { lane_width: f32, avenue: RoadClassParams, street: RoadClassParams, alley_width: f32 }
pub struct RoadClassParams { lanes_per_direction: u32, sidewalk: f32 }
pub struct DistrictsParams { grid: u32, seed_jitter: f32, weights: DistrictWeights,
    downtown: DistrictParams, commercial: DistrictParams, residential: DistrictParams, industrial: DistrictParams }
pub struct DistrictWeights { commercial: f32, residential: f32, industrial: f32 }
pub struct DistrictParams { floors: (u32, u32), floor_height: f32, park_share: f32,
    lot_area: (f32, f32), min_frontage: f32, setback: f32 }
pub struct PoiParams { hospital_districts: Vec<DistrictKind>, police_districts: Vec<DistrictKind>, police_stations: u32 }
#[derive(Deserialize, Clone, Copy, Debug, PartialEq, Eq)] pub enum DistrictKind { Downtown, Commercial, Residential, Industrial }
```
Районы — структура с четырьмя именованными полями (не map), пропуск района = ошибка разбора.
`impl CityParams`:
- `pub fn validate(&self) -> Result<(), String>` — сообщение называет поле. Проверки: все f32 конечны;
  `size, ground_margin, edge_wall.height, edge_wall.thickness, lane_width, alley_width, sidewalk-и,
  floor_height, min_building_area, min_frontage > 0`; `setback ≥ 0`; `min ≤ max` во всех диапазонах;
  `floors.0 ≥ 1`; `0 < core_radius < size/2`; оба интервала сетки разбиваемы:
  `split_feasible(core_radius, core_block)` и `split_feasible(size/2 − core_radius, outer_block)`
  (см. Шаг 3, `n = ceil(L/hi)`, `n·lo ≤ L`); `4·node_jitter < min(core_block.0, outer_block.0)`;
  `min(core_block.0, outer_block.0) > 2·(half_carriageway(Avenue) + sidewalk(Avenue))` (числа из `roads`,
  не литералы); вероятности `superblock_chance, alley_chance, park_share, split_jitter ∈ [0,1]`,
  `superblock_chance + alley_chance ≤ 1`, `split_jitter < 0.5`; `districts.grid` нечётное ≥ 3;
  `seed_jitter ∈ [0, 0.5)`; веса ≥ 0 и сумма > 0; `avenue_every ≥ 1`; `police_stations ∈ 1..=2`;
  `hospital_districts`, `police_districts` непусты; `lanes_per_direction ≥ 1`.
- `pub fn district(&self, kind: DistrictKind) -> &DistrictParams`;
  `pub fn half_carriageway(&self, class: RoadClass) -> f32` (Avenue/Street:
  `lanes_per_direction · lane_width`, Alley: `alley_width / 2`);
  `pub fn sidewalk(&self, class: RoadClass) -> f32` (Alley: 0). Последние два — единственный источник ширин
  для генератора и тестов (Avenue: 2·3.25 = 6.5 м + 4 м, Street: 3.25 м + 3 м, Alley: 2.25 м + 0).

**`src/layout.rs`** — выходные типы (все id `u32`, индексы в соответствующих `Vec`):
```rust
pub struct CityLayout { seed: u64, size: f32, ground_size: f32, roads: RoadGraph,
    districts: Vec<District>, blocks: Vec<Block>, lots: Vec<Lot>, buildings: Vec<Building>,
    gang_districts: [u32; 2], sidewalks: WalkGraph, lanes: LaneGraph, player_spawn: Vec2 }
pub struct RoadGraph { nodes: Vec<Vec2>, edges: Vec<RoadEdge>, center: u32 }   // только живые рёбра
pub struct RoadEdge { a: u32, b: u32, class: RoadClass }
pub enum RoadClass { Avenue, Street, Alley }
pub struct District { kind: DistrictKind, seed_point: Vec2 }
pub struct Block { district: u32, nodes: Vec<u32>, sides: Vec<u32>, curb: Vec<Vec2>, inner: Vec<Vec2>, is_park: bool }
pub struct Lot { block: u32, polygon: Vec<Vec2> }
pub struct Building { lot: u32, center: Vec2, axis: Vec2, half_extents: Vec2, height: f32, kind: BuildingKind }
pub enum BuildingKind { Generic, Hospital, PoliceStation, GangHq(u8) }
pub struct WalkGraph { nodes: Vec<Vec2>, edges: Vec<(u32, u32)> }
pub struct LaneGraph { lanes: Vec<Lane>, connectors: Vec<Connector> }
pub struct Lane { edge: u32, from: Vec2, to: Vec2 }
pub struct Connector { from: u32, to: u32, intersection: u32 }
#[derive(Debug)] pub enum GenError { NotEnoughGangDistricts { found: u32 }, NoPoiCandidate { poi: &'static str } }
```
(`Display` + `std::error::Error` для `GenError`.) Инварианты типа: `sides[k]` — id живого ребра между
`nodes[k]` и `nodes[k+1]`; `curb.len() == nodes.len()`; `inner` пуст или `inner.len() == sides.len()`
(у слитого квартала почти-180° вершины сохраняются, сторона `inner[k]→inner[k+1]` соответствует
`sides[k]`). Координаты: `Vec2(x, y)` citygen = мир `(x, z)`, земля y = 0, центр в начале координат.

**`src/rng.rs`** — `pub(crate) fn stream(seed: u64, tag: u64, cell: u64) -> ChaCha8Rng` =
`ChaCha8Rng::seed_from_u64(fnv1a64(seed.to_le_bytes() ++ tag.to_le_bytes() ++ cell.to_le_bytes()))`.
Импорт: `use rand_chacha::{ChaCha8Rng, rand_core::{Rng, SeedableRng}};` (API проверен пробой
`scratch/probe_rng`). Теги — `u64`-константы (закон): `GRID_CORE_POS_X, GRID_CORE_NEG_X, GRID_OUTER_POS_X,
GRID_OUTER_NEG_X` и те же для Z, `JITTER, DISTRICTS, EDGES, PARKS, LOTS, BUILDINGS, GANGS`. Ячейка =
id квартала/лота или 0. Выборки только через хелперы: `unit_f32(rng) = (rng.next_u32() >> 8) as f32 *
(1.0 / 16_777_216.0)`, `range_f32(rng, lo, hi) = lo + (hi − lo)·unit_f32`, `range_u32(rng, lo, hi_incl) =
lo + (rng.next_u64() % (hi_incl − lo + 1) as u64) as u32`, `chance(rng, p) = unit_f32 < p`. `usize` не
сэмплируется (GDD §2.5). Перемешивание — Fisher–Yates через `range_u32`.

**`src/geom.rs`** — выпуклая геометрия на `Vec2`, только `+ − * /`, `sqrt`, `abs`, `min/max`, `round`,
сравнения: `signed_area` (`Σ(x_i·y_{i+1} − x_{i+1}·y_i)/2`), `centroid`, `is_convex(poly, eps)` (все
`cross ≥ −eps`, eps = 1e-4 м²), `inset(poly, offsets: &[f32]) -> Option<Vec<Vec2>>` (сдвиг каждой стороны
внутрь по `d.perp()` = `(−d.y, d.x)`; вершина = пересечение соседних сдвинутых прямых; при
`|d1.perp_dot(d2)| < 1e-6` — параллельные стороны: вершина + нормаль·отступ; результат `None`, если не
выпуклый, площадь ≤ 0 или сменил знак ориентации), `clip_half_plane(poly, flags, point, normal) ->
(Vec<Vec2>, Vec<bool>)` (Sutherland–Hodgman для одной прямой, сторона разреза получает флаг `false`),
`min_area_obb` (перебор направлений рёбер), `contains_convex(poly, p, eps)`,
`convex_overlap(a, b) -> f32` (SAT, глубина проникновения, касание = 0), `dist_point_segment`.
Unit-тесты с числами, посчитанными вручную: квадрат 10×10 `inset` 1 → площадь 64; квадрат 10×10 клип
прямой x = 4 → площади 40 и 60, у новой стороны флаг false; inward-нормали квадрата
(0,0),(1,0),(1,1),(0,1): ребро (0,0)→(1,0) → (0,1), (1,0)→(1,1) → (−1,0), (0,1)→(0,0) → (1,0);
два квадрата с общей стороной → overlap 0; сдвинутые на 0.5 → overlap 0.5; вырожденный inset
(квадрат 2×2, отступ 1.5) → `None`.

→ `cargo test -p citygen --lib` зелёный. Каждый файл < 750 строк.

### Шаг 3. `citygen`: конвейер

Порядок этапов фиксирован; каждый этап берёт свой подпоток RNG, поэтому порядок этапов на поток других
не влияет.

**`src/grid.rs` — линии сетки (исправлено относительно PLAN.md).**
`fn split_interval(len, lo, hi, rng) -> Option<Vec<f32>>`: `n = ceil(len/hi)`; если `n·lo > len` →
`None`; `base = len/n`; `δ = min(base − lo, hi − base)`; `o_i = range_f32(−δ/2, δ/2)` для i∈0..n;
`m = mean(o)`; шаги `s_i = base + o_i − m` ∈ `[base−δ, base+δ] ⊆ [lo, hi]`, сумма = `len`.
`split_feasible(len, lo, hi) = ceil(len/hi)·lo ≤ len` (использует `validate`).
Каждая полуось (+X, −X, +Z, −Z) делится на два интервала: ядро `[0, core_radius]` шагами `core_block`,
окраина `[core_radius, size/2]` шагами `outer_block`, у каждого интервала свой тег/подпоток. Линии:
0, накопленные суммы, `±core_radius`, последняя ровно `±size/2` (присваивается точно, не суммой).
Это принятое упрощение Q2 ("шаг ядро/окраина вместо районной таблицы").
Разобранные примеры (проба `scratch/pr2/split_probe.py`): `(250, 70, 90)` → n=3, base=83.333,
δ=6.667, шаги ∈ [76.667, 90.0]; `(350, 100, 160)` → n=3, base=116.667, δ=16.667, шаги ∈ [100, 133.333];
`(160, 70, 90)` → n=2, base=80, δ=10, шаги ∈ [70, 90]; `(90, 70, 90)` → n=1, шаг 90; `(91, 70, 90)` и
`(130, 70, 90)` → `None` (n=2, 140 > len). Старое правило PLAN.md ("остаток r > max делится пополам")
на `(250, 70, 90)` дало квартал < 70 м в 10008 из 20000 прогонов.

**`src/roads.rs` — узлы, рёбра, кварталы, суперкварталы, отступы.**
1. Узел (i, j) = (xs[i], zs[j]) + jitter `range_f32(±node_jitter)` по обеим осям (поток `JITTER`, ячейка
   = i·1024 + j). Центральный узел (0,0) без jitter; узлы периметра двигаются только вдоль своей линии;
   углы без jitter. Итог: центр — ровно перекрёсток двух авеню, город квадратный.
2. Класс ребра = класс линии: линия с `|i − center| % avenue_every == 0` — Avenue, иначе Street;
   центральные линии — авеню.
3. Кварталы-четырёхугольники по ячейкам сетки, узлы по кругу с положительной ориентированной площадью.
4. Суперкварталы/переулки: внутренние (не периметр) рёбра в порядке id; `r = unit_f32` (поток `EDGES`,
   ячейка = id ребра). `r < superblock_chance` → удалить ребро и слить два квартала; иначе
   `r < superblock_chance + alley_chance` → ребро становится Alley. Условия для обоих: ни один из двух
   соседних кварталов ещё не помечен (после операции оба помечены); для слияния ещё — объединённый
   многоугольник проходит `is_convex(eps = 1e-4)` (иначе ребро остаётся как было). Эффективная доля
   слияний ниже `superblock_chance` — это ожидаемо, её крутит владелец.
   Почему связность держится: помеченные рёбра образуют паросочетание граней, периметр не трогается,
   поэтому в двойственном графе помеченные рёбра цикла не образуют ⇒ удаление не режет граф. Степень
   каждого узла в графе "без удалённых и без переулков" ≥ 2 (внутренний узел окружён 4 гранями, каждое
   помеченное ребро съедает 2; узел периметра теряет ≤ 1 внутреннее ребро). Тест проверяет прямо.
5. Компактировать рёбра: удалённые убрать, живые перенумеровать; `Block.sides` пишется по новым id.
   `RoadGraph.center` = id узла (0,0).
6. Отступы: `curb = inset(poly, [half_carriageway(class(sides[k]))])`, `inner = inset(poly,
   [half_carriageway + sidewalk])`. Если `inner` — `None`, квартал становится парком с пустым `inner`
   (газон рисуется по `curb`, лотов нет). Если `curb` — `None` (при валидных параметрах невозможно) —
   тоже парк с пустыми `curb` и `inner`.

**`src/districts.rs` — районы (после кварталов, исправлено).**
Семена на сетке `grid × grid` (по умолчанию 3): центр ячейки + jitter `±seed_jitter·cell` (поток
`DISTRICTS`); центральное семя ровно (0,0), вид Downtown. Остальные виды взвешенным выбором
(`unit_f32 · sum`) по `weights`. Район квартала = ближайшее семя к центроиду **итогового** (после
слияний) многоугольника квартала (квадраты расстояний, ничья → меньший id). "Шум плотности" в v1 нет (Q2).
Гарантия территорий банд: `E` = районы вида Residential/Industrial, владеющие ≥ 1 кварталом с непустым
`inner`. Пока `|E| < 2`: среди не-Downtown районов вне `E` с ≥ 1 таким кварталом взять район с наибольшим
числом таких кварталов (ничья → меньший id) и перекрасить в Residential. Нет кандидата →
`Err(GenError::NotEnoughGangDistricts { found })`.

**`src/lots.rs` — парки, лоты, здания.**
1. Парки: квартал целиком парк с вероятностью `park_share` своего района (поток `PARKS`, ячейка = id
   квартала), **кроме** квартала с наименьшим id среди кварталов района с непустым `inner` — он никогда не
   парк (так у каждого района с застраиваемым кварталом есть лоты).
2. Лоты — рекурсивное OBB-деление `inner` (Vanegas 2012; Martin Evans "Lots"): у каждой стороны флаг
   "фасад на улицу"; стороны `inner` получают `true`, если `sides[k]` — не Alley, и `false` для переулка
   (переулок "без тротуаров, без трафика", GDD §2.3, не улица для правила Access); стороны от разреза —
   `false`. Узел — лист, если площадь ≤ `lot_area.1` района. Иначе `min_area_obb`, разрез
   перпендикулярно длинной оси через центр со сдвигом `range_f32(±split_jitter)·длина` (поток `LOTS`,
   ячейка = id квартала), `clip_half_plane` на обе стороны. Разрез принят, если у обоих детей площадь
   ≥ `lot_area.0` **и** самая длинная сторона с флагом `true` ≥ `min_frontage`; иначе пробуем короткую
   ось; иначе лист. Глубина ≤ 16 (закон алгоритма). Корень всегда даёт ≥ 1 лот.
3. Здания (поток `BUILDINGS`, ячейка = id лота): ось `u` = нормированное направление самой длинной
   стороны лота с флагом `true` (фасад к улице), `v = u.perp()`; лот `inset` на `setback` района
   (`None` → без здания); прямоугольник в кадре (u, v) = проекции сжатого многоугольника, центр — его
   центроид; пока хоть один угол вне сжатого многоугольника — полуоси ×0.9 (не более 24 шагов; константы
   подгонки — закон алгоритма, не тюнинг). Не влез или `4·hx·hz < min_building_area` — лот без здания.
   Высота = `range_u32(floors) × floor_height` района.

**`src/pois.rs` — точки интереса (исправлено: банды первыми, без fallback-нарушения).**
Кандидаты района = здания его не-парковых лотов, сортировка по площади следа (`4·hx·hz`) убыв., id возр.
1. Банды: `E2` = районы Residential/Industrial с ≥ 1 кандидатом, перемешать (поток `GANGS`, Fisher–Yates);
   `|E2| < 2` → `Err(NotEnoughGangDistricts { found })`. Первые два → `gang_districts`; штаб i = первый
   кандидат района → `BuildingKind::GangHq(i)`.
2. Больница: первый свободный кандидат (глобальный порядок сортировки) в районах из
   `pois.hospital_districts`, иначе первый свободный не-Downtown, иначе первый свободный; нет →
   `Err(NoPoiCandidate { poi: "hospital" })`.
3. Участки: `police_stations` раз то же по `pois.police_districts` минус занятые; нет →
   `Err(NoPoiCandidate { poi: "police" })`.
POI — это `BuildingKind` у здания, отдельного списка нет; одно здание — одна роль.

**`src/graphs.rs` — графы и спавн.**
1. Тротуары: узел на каждом углу каждого квартала (точка на осевой тротуара: отступ
   `half_carriageway + sidewalk/2` у обеих сторон угла; у переулочной стороны — по линии бордюра).
   Рёбра: кольцо вдоль сторон квартала; "зебры" — для каждого живого ребра дороги с кварталами по обе
   стороны на каждом его конце соединить угловые узлы этих двух кварталов у этого узла дороги; дорожки
   парка — узел в центроиде парка, рёбра к его углам.
2. Полосы: для каждого живого не-Alley ребра `n = lanes_per_direction` полос в каждую сторону,
   правостороннее движение. Для направления `d` (единичный, (x,z)) правая сторона мира = `d.perp()` =
   `(−d.y, d.x)`: `d=(0,−1)` (−Z, "вперёд") → (1,0) = +X; `d=(1,0)` → (0,1) = +Z; `d=(0,1)` → (−1,0) = −X
   (все три — правая рука при Y вверх). Полоса i смещена на `(i+0.5)·lane_width` вправо; концы подрезаны
   на полуразмер перекрёстка (максимум `half_carriageway` рёбер узла). Коннекторы: в каждом узле с каждой
   входящей полосы ребра `e` на каждую исходящую полосу каждого ребра `f ≠ e` (без разворотов),
   `intersection` = id узла. Зона конфликта v1 = весь перекрёсток (Q2).
3. Спавн: среди сторон кварталов с непустым `inner`, чьё ребро не Alley и идёт "север–юг"
   (`|dz| > |dx|`), взять сторону, у которой середина тротуара ближе всего к (0,0) (ничья → меньший id
   квартала, потом k); спавн = середина стороны `curb` + inward-нормаль × `sidewalk/2`
   (= середина ребра + нормаль × (`half_carriageway + sidewalk/2`)). Игрок по умолчанию смотрит в −Z
   (yaw 0), W идёт вдоль тротуара (для `t1.py`).
4. `ground_size = size + 2·ground_margin`.

**`src/hash.rs` — `layout_hash` (исправлено: явная схема).**
FNV-1a 64 (offset `0xcbf29ce484222325`, prime `0x100000001b3`) по канонической записи. Примитивы:
`u8`; `u32`/`u64` LE; float → `quantize_mm(v) = (v * 1000.0).round() as i64` → LE (`f32::round` —
половина от нуля); `Vec2` = x, y; коллекция = `u32` длина + элементы. Коды enum через явный `match`
(не `as u8`): RoadClass Avenue=0, Street=1, Alley=2; DistrictKind Downtown=0, Commercial=1,
Residential=2, Industrial=3; BuildingKind Generic=0, Hospital=1, PoliceStation=2, GangHq=3, после кода
всегда `u8` индекс банды (0 у не-банд). Порядок полей:
1. `HASH_SCHEMA_VERSION: u32 = 1` (закон; поднимается при любом изменении схемы);
2. `seed`, `size`, `ground_size`;
3. `roads.center`, `roads.nodes`, `roads.edges` (a, b, class);
4. `districts` (kind, seed_point);
5. `blocks` (district, is_park как u8, nodes, sides, curb, inner);
6. `lots` (block, polygon);
7. `buildings` (lot, center, axis, half_extents, height, kind);
8. `gang_districts[0]`, `gang_districts[1]`;
9. `sidewalks.nodes`, `sidewalks.edges` (u32, u32);
10. `lanes.lanes` (edge, from, to), `lanes.connectors` (from, to, intersection);
11. `player_spawn`.
Unit-тесты test vectors (посчитаны `scratch/pr2/fnv_probe.py`): `fnv1a64(b"") == 0xcbf29ce484222325`,
`fnv1a64(b"a") == 0xaf63dc4c8601ec8c`, `fnv1a64(b"foobar") == 0x85944171f73967e8`,
`quantize_mm(1.2345) == 1235`, `quantize_mm(0.0005) == 1`, `quantize_mm(-0.0005) == -1`,
`fnv1a64(&1235i64.to_le_bytes()) == 0x08ce64bd5cc50a22`. Свой хэш вместо `DefaultHasher`: алгоритм std не
специфицирован.

**`generate()` в `lib.rs`**: вызывает `grid → roads → districts → lots → pois → graphs` и собирает
`CityLayout`; возвращает `Result<CityLayout, GenError>`. Не валидирует параметры (это делает загрузчик);
документировать в `///`, что вход должен пройти `validate()`.

Запрет трансцендентных функций в citygen: `grep -nE "sin\(|cos\(|atan|powf|exp\(|ln\(" crates/citygen/src`
пуст (ревьюер проверяет).

→ `cargo build -p citygen`, `cargo clippy -p citygen -- -D warnings`.

### Шаг 4. Данные: `assets/world/city.ron` и `assets/world/render.ron`

`assets/world/city.ron` (владелец `world/`, тип `citygen::CityParams`):
```ron
(
    size: 1200.0,
    ground_margin: 100.0,
    edge_wall: (height: 10.0, thickness: 1.0),
    grid: (core_radius: 250.0, core_block: (70.0, 90.0), outer_block: (100.0, 160.0),
           node_jitter: 6.0, avenue_every: 3),
    roads: (lane_width: 3.25,
            avenue: (lanes_per_direction: 2, sidewalk: 4.0),
            street: (lanes_per_direction: 1, sidewalk: 3.0),
            alley_width: 4.5),
    superblock_chance: 0.08,
    alley_chance: 0.08,
    districts: (
        grid: 3,
        seed_jitter: 0.25,
        weights: (commercial: 0.3, residential: 0.45, industrial: 0.25),
        downtown:    (floors: (8, 30), floor_height: 3.9, park_share: 0.04, lot_area: (800.0, 2500.0), min_frontage: 20.0, setback: 1.0),
        commercial:  (floors: (3, 8),  floor_height: 3.9, park_share: 0.06, lot_area: (400.0, 1200.0), min_frontage: 14.0, setback: 2.0),
        residential: (floors: (2, 5),  floor_height: 3.1, park_share: 0.12, lot_area: (250.0, 700.0),  min_frontage: 10.0, setback: 3.0),
        industrial:  (floors: (1, 2),  floor_height: 3.9, park_share: 0.02, lot_area: (1000.0, 3000.0), min_frontage: 20.0, setback: 4.0),
    ),
    split_jitter: 0.15,
    min_building_area: 60.0,
    pois: (hospital_districts: [Commercial, Residential], police_districts: [Residential, Commercial],
           police_stations: 1),
)
```
Числа из GDD §2.3 (размер 1200, кварталы 70-90 / 100-160, полоса 3.25, авеню 4 полосы + 4 м, улица 2
полосы + 3 м, переулок 4-5 м, этажи 3.1/3.9, этажность по районам); `ground_margin = 100`, стены — Q1=B.
Остальное (доли, лоты, отступы, высота стены) — стартовые данные, их крутит владелец. Проверка
осуществимости на этих числах: `split_feasible(250, 70, 90)` (3·70 = 210 ≤ 250) и
`split_feasible(350, 100, 160)` (3·100 = 300 ≤ 350) — true; `4·6 = 24 < 70`; `70 > 2·(6.5 + 4) = 21`.

`assets/world/render.ron` (владелец `visuals/`) — дописать поля (sRGB `(r, g, b)` в 0..1):
```ron
    district_colors: (downtown: (0.62, 0.64, 0.70), commercial: (0.78, 0.70, 0.55),
                      residential: (0.80, 0.62, 0.52), industrial: (0.55, 0.55, 0.50)),
    hospital_color: (0.95, 0.95, 0.95),
    police_color: (0.20, 0.30, 0.75),
    gang_hq_color: (0.75, 0.20, 0.55),
    road_color: (0.18, 0.18, 0.20),
    sidewalk_color: (0.60, 0.60, 0.58),
    park_color: (0.30, 0.55, 0.25),
    surface_layer_step: 0.02,
```
`surface_layer_step` — подъём плоских слоёв над землёй против z-fighting (презентация).

**Законы (const, не тюнинг)** — каждый с однострочным комментарием "почему закон": 1 unit = 1 м, Y вверх,
`Vec2(x,y)` citygen = мир `(x,z)`; теги подпотоков RNG; `HASH_SCHEMA_VERSION`; квант хэша 1 мм; параметры
FNV; шаг подгонки прямоугольника 0.9 × 24; глубина деления 16; eps геометрии; толщина плиты земли 1 м
(`GROUND_SLAB_THICKNESS`, коллизионная геометрия, как пол `test_area.rs:7`). Высота и толщина стен —
данные (`edge_wall`), не const.

→ разбирается тестом Шага 5 (`shipped_params()`), `validate()` проходит.

### Шаг 5. Гейты `citygen` (`crates/citygen/tests/`)

- `tests/common/mod.rs`: `shipped_params()` — читает
  `CARGO_MANIFEST_DIR/../../assets/world/city.ron`, `ron::from_str::<CityParams>`, `validate()`; при
  отсутствии файла/ошибке — `panic!("GATE BROKEN: ...")` с путём. `SEEDS: [u64; 3] = [1, 2, 42]`,
  `SWEEP = 0..32`. `layouts() -> &'static [(u64, CityLayout)]` — `OnceLock`, генерирует один раз на
  тестовый бинарь для `SEEDS ∪ SWEEP` (35 seed; `generate(..).unwrap_or_else(|e| panic!("seed {s}: {e}"))`).
  `golden()` — `include_str!("../golden_hashes.txt")`, разбор построчно (`lines()` + `trim()`, пустые и
  `#` пропускаются, `seed` десятичный + пробел + `hash` в hex `0x…`): устойчив к `\r\n`
  (`core.autocrlf=true`).
- `tests/golden_hashes.txt` — шапка-комментарий: "меняется только вместе с намеренным изменением генератора
  или city.ron; читается citygen, gta_sim и tools/qa/scenarios/t2.py; обновлять процедурой bless", затем три
  строки `seed hash`.
- `tests/golden.rs`:
  - `golden_hashes_match`: для каждой строки `layout_hash(&generate(seed, &params)?) == hash`; при
    несовпадении — сообщение с ожидаемым/фактическим и командой bless, **без** автоматической перезаписи.
  - `same_seed_same_hash` (два независимых вызова `generate`), `different_seeds_differ` (1 ≠ 2 ≠ 42).
  - `#[ignore] bless_print_golden`: печатает готовые строки файла. **Процедура bless:** сначала зелёные
    `properties.rs`; затем `cargo test -p citygen --test golden -- --ignored bless_print_golden
    --nocapture`; значения вручную вписать в `golden_hashes.txt`; повторный прогон `golden` зелёный.
    Первое заполнение файла — этой же процедурой.
- `tests/properties.rs` — одна функция на свойство, цикл по `layouts()`:
  1. `road_graph_connected`: BFS из `center` по всем живым рёбрам достигает всех узлов;
     `nodes[center] == Vec2::ZERO`.
  2. `lane_graph_strongly_connected`: полос > 0; BFS по коннекторам и по обратным коннекторам из полосы 0
     достигает всех полос (граф фактический, с реальными переулками и суперкварталами).
  3. `sidewalk_graph_connected`: BFS по тротуарам достигает всех узлов.
  4. `lots_do_not_overlap`: все пары лотов (AABB-префильтр, затем `convex_overlap`) — проникновение
     ≤ 1e-3 м; каждая вершина лота внутри `inner` своего квартала (eps 1e-3).
  5. `lots_face_a_street` (независимо от флагов генератора, по id рёбер): (а) для каждого непаркового
     квартала с непустым `inner` и каждого k: `sides[k] < roads.edges.len()` (живое ребро), сторона
     `inner[k]→inner[k+1]` параллельна ребру `roads.edges[sides[k]]` (|perp_dot нормированных| ≤ 1e-3) и
     лежит на расстоянии `half_carriageway(class) + sidewalk(class)` ± 1e-2 от его осевой; (б) у каждого
     лота есть сторона, оба конца которой в пределах 1e-3 от стороны `inner[k]` квартала, у которой
     `roads.edges[sides[k]].class != Alley`, и длина которой ≥ `min_frontage` района − 1e-3.
  6. `buildings_inside_lots`: 4 угла каждого здания (`center ± u·hx ± v·hz`) внутри его лота (eps 1e-3);
     высота > 0.
  7. `pois_exist`: ровно 1 `Hospital`; `PoliceStation` ровно `pois.police_stations`; `GangHq(0)` и
     `GangHq(1)` по одному, каждое лежит в `gang_districts[i]` (здание → лот → квартал → район), районы
     различны и вида Residential/Industrial.
  8. `player_spawn_on_sidewalk`: есть живое не-Alley ребро, расстояние от спавна до его осевой в
     `[half_carriageway, half_carriageway + sidewalk]`; спавн вне всех лотов.
  9. `grid_steps_in_range` — unit-тесты в `grid.rs` (`#[cfg(test)]`): примеры из Шага 3 (250/350/160/90 →
     все шаги в `[lo−1e-3, hi+1e-3]`, сумма = len ± 1e-3; 91 и 130 → `None`), по 1000 разных rng.
  10. `validate_rejects_bad_params` (unit в `params.rs`): `core_radius = 91` при `core_block (70, 90)` →
      ошибка, называющая `grid.core_radius`; `superblock_chance + alley_chance = 1.2` → ошибка; чётный
      `districts.grid` → ошибка.
  11. `gang_guarantee_recolors` (в `properties.rs`): `shipped_params()` с весами
      `(commercial: 1.0, residential: 0.0, industrial: 0.0)`; `generate(1)` → `Ok`, и `pois_exist`-проверки
      выполняются (гарантия территорий работает перекраской).
- `tests/perf.rs` — `#[ignore] generation_under_budget`: для `SEEDS` меряет `Instant` вокруг `generate` +
  `layout_hash`, печатает, `assert!(elapsed < Duration::from_secs(2))` на каждый seed. Запуск в QA:
  `cargo test -p citygen --release --test perf -- --ignored --nocapture`.

→ `cargo test -p citygen` зелёный; flip-RED по таблице 4.2.

### Шаг 6. `gta_sim/flow`

Новый `crates/gta_sim/src/flow/mod.rs`:
```rust
#[derive(States, Default, Clone, PartialEq, Eq, Hash, Debug)]
pub enum GameState { #[default] Loading, Playing }
pub struct FlowPlugin;  // build: app.init_state::<GameState>();
```
Только два нужных сейчас варианта (`Paused/Wasted/Busted` добавят их слайсы). `lib.rs`: `pub mod flow;`,
`FlowPlugin` первым в кортеже плагинов `compose_sim`. `StatesPlugin` не добавляется внутри `compose_sim`
(клиент получает его из `DefaultPlugins`; добавить второй раз нельзя) — его добавляет тестовый харнесс
(Шаг 8).

### Шаг 7. `gta_sim/world` + `player` + `compose_sim`

- `crates/gta_sim/src/world/mod.rs`:
  - `pub enum WorldSource { TestArea, City { seed: u64 } }` (`Clone, Copy, Debug`);
    `pub struct WorldPlugin { pub source: WorldSource }`;
    `pub use citygen::{BuildingKind, CityLayout, CityParams, DistrictKind, RoadClass};`
    `pub const CITY_CONFIG: &str = "world/city.ron";` (путь — закон, как `LOCOMOTION_CONFIG`).
  - `#[derive(SystemSet, Debug, Clone, PartialEq, Eq, Hash)] enum WorldSystems { Generation }`.
  - `build`: `insert_resource(PlayerSpawn(Vec3::ZERO))`, `register_type::<Block>()`, затем по источнику:
    `TestArea` → `add_systems(OnEnter(GameState::Loading), (spawn_test_area, finish_loading))`, где
    `finish_loading(mut next: ResMut<NextState<GameState>>) { next.set(GameState::Playing) }`;
    `City { seed }` → `insert_resource(CitySeed(seed))`, `register_type::<CitySeed>()`,
    `register_type::<CityLayoutHash>()`,
    `configure_sets(Update, WorldSystems::Generation.run_if(in_state(GameState::Loading)))`,
    `add_systems(OnEnter(GameState::Loading), start_city_generation)`,
    `add_systems(Update, apply_city_generation.in_set(WorldSystems::Generation))`.
  - `pub use test_area::spawn_test_area` → приватный `use` (внешний пользователь `player` уходит).
- `world/test_area.rs:5`: комментарий → `// Fixture level for the headless character gates.` (старый
  после T2 ложен). Таблица и функции без изменений.
- Новый `world/city.rs`:
  - `#[derive(Resource, Reflect)] #[reflect(Resource)] pub struct CitySeed(pub u64);` и
    `pub struct CityLayoutHash(pub u64);` (одно-полевой tuple struct сериализуется reflect'ом как число,
    `bevy_reflect-0.19.1/src/serde/ser/tuple_structs.rs:47-49`).
  - `#[derive(Resource)] pub struct City(pub CityLayout);`, `#[derive(Resource)] pub struct
    CityParamsRes(pub CityParams);` (citygen без Bevy не может derive `Resource`).
  - `#[derive(Component)] pub struct CityBuilding { pub size: Vec3, pub district: DistrictKind, pub kind:
    BuildingKind }`, `#[derive(Component)] pub struct CityGround;`, `#[derive(Component)] pub struct
    CityEdgeWall;` (без Reflect: QA их не читает).
  - `#[derive(Resource)] struct CityGenTask(Task<Result<(CityLayout, u64), GenError>>);`
  - `start_city_generation(mut commands, seed: Res<CitySeed>, params: Res<CityParamsRes>)`: клонировать
    seed/params, `AsyncComputeTaskPool::get().spawn(async move { let layout = generate(seed, &params)?;
    let hash = layout_hash(&layout); Ok((layout, hash)) })`, `commands.insert_resource(CityGenTask(task))`.
    Тело задачи — чистая функция входа.
  - `apply_city_generation(mut commands, mut task: ResMut<CityGenTask>, params: Res<CityParamsRes>,
    mut next: ResMut<NextState<GameState>>, mut exit: MessageWriter<AppExit>)`:
    `let Some(result) = block_on(poll_once(&mut task.0)) else { return };` (голого `block_on(task)` нет)
    → `commands.remove_resource::<CityGenTask>()`; `let Ok((layout, hash)) = result else {
    error!("city generation failed: {err}"); exit.write(AppExit::error()); return; }` (let-else с
    `match`/`map_err`, без вложенности) → `PlayerSpawn(Vec3::new(sp.x, 0.0, sp.y))`; спавн земли, стен,
    зданий (обычный цикл `commands.spawn`, ~1-2 тыс. статиков); `insert_resource(CityLayoutHash(hash))`,
    `insert_resource(City(layout))`, `next.set(GameState::Playing)`.
  - Земля: `CityGround` + `RigidBody::Static` + `Collider::cuboid(g, GROUND_SLAB_THICKNESS, g)` (полные
    длины, `avian3d-0.7.0/src/collision/collider/parry/mod.rs:745-747`, как `test_area.rs:41`) +
    `Transform::from_xyz(0.0, −GROUND_SLAB_THICKNESS/2, 0.0)`; `g = layout.ground_size`.
  - Стены (Q1=B): 4 × (`CityEdgeWall` + `RigidBody::Static` + `Collider::cuboid` + `Transform`), `h =
    edge_wall.height`, `t = edge_wall.thickness`, `L = g + 2t`: ±X — `cuboid(t, h, L)` в
    `(±(g/2 + t/2), h/2, 0)`; ±Z — `cuboid(L, h, t)` в `(0, h/2, ±(g/2 + t/2))`. Внутренняя грань ровно на
    `±g/2` (край плиты земли). На shipped-данных: g = 1400, центры (±700.5, 5, 0) и (0, 5, ±700.5).
    Без меша (невидимые).
  - Здание: `CityBuilding { size: Vec3::new(2hx, h, 2hz), .. }` + `RigidBody::Static` +
    `Collider::cuboid(2hx, h, 2hz)` + `Transform::from_xyz(c.x, h/2, c.y).with_rotation(
    Quat::from_rotation_y(f32::atan2(−u.y, u.x)))`. Рантайм не участвует в хэше — `atan2` здесь допустим.
    Проверка поворота (`R_y(θ)·X = (cosθ, 0, −sinθ)`, `R_y(θ)·Z = (sinθ, 0, cosθ)`; локальная X должна
    лечь на (u.x, 0, u.y), локальная Z — на `v = u.perp()`):
    1. u = (1,0): θ = 0 → X → (1,0,0) ✓; Z → (0,0,1) = v (0,1) ✓.
    2. u = (0,1) (мир +Z): θ = atan2(−1,0) = −90° → X → (0,0,1) ✓; Z → (−1,0,0) = v (−1,0) ✓.
    3. u = (0,−1) (мир −Z): θ = +90° → X → (0,0,−1) ✓; Z → (1,0,0) = v (1,0) ✓.
  - `const GROUND_SLAB_THICKNESS: f32 = 1.0;` с комментарием (закон коллизионной геометрии).
- `crates/gta_sim/src/lib.rs`: `pub fn compose_sim(app: &mut App, root: ConfigRoot, source: WorldSource)
  -> Result<(), ConfigError>`; для `City`: `let params = load_config::<CityParams>(&root, CITY_CONFIG)?;
  params.validate().map_err(|message| ConfigError { path: root.path(CITY_CONFIG), message })?;
  app.insert_resource(CityParamsRes(params));` (до `insert_resource(root)`, которое забирает `root`);
  плагины `(FlowPlugin, PhysicsPlugins::default(), TnuaAvian3dPlugin::new(FixedUpdate), CharacterPlugin,
  WorldPlugin { source }, PlayerPlugin)`. Для `TestArea` `city.ron` не читается.
- `crates/gta_sim/src/player/mod.rs:2,14`: `.add_systems(OnEnter(GameState::Playing), spawn_player)`;
  импорт `spawn_test_area` убрать. Остальное без изменений.

→ `cargo build -p gta_sim`; `python tools/qa/tree_check.py` (`cargo tree -p gta_sim -e normal -i
bevy_render` пуст).

### Шаг 8. Тестовый харнесс `gta_sim`

`crates/gta_sim/tests/common/mod.rs`:
- `headless_app()`: в кортеж плагинов добавить `bevy::state::app::StatesPlugin` (до `compose_sim`);
  `compose_sim(&mut app, assets_root(), WorldSource::TestArea)`. Остальное без изменений.
- Вынести `place_player(app, at)` из `terrain.rs:7-14` в `common` без изменений (меняет `Position` и
  `Transform`); `terrain.rs` использует его из `common`.
- Новый `city_app(seed: u64) -> App`: как `headless_app`, но `WorldSource::City { seed }`; затем цикл
  `app.update()` + `std::thread::sleep(Duration::from_millis(1))` до `*app.world().resource::<State<
  GameState>>().get() == GameState::Playing`; на каждом круге `if let Some(exit) = app.should_exit() {
  panic!("city generation failed: {exit:?}") }`; дедлайн 120 с по стене →
  `panic!("city generation did not finish in 120 s")` (это отказ кода, не харнесса, поэтому без
  "GATE BROKEN").
- `golden(seed) -> u64`: разбор `include_str!("../../../citygen/tests/golden_hashes.txt")` тем же
  построчным правилом (путь относительно `crates/gta_sim/tests/common/mod.rs`); нет строки →
  `panic!("GATE BROKEN: no golden for seed {seed}")`.
- `city_params(app) -> &CityParams` из `CityParamsRes`.

`tests/config.rs`: + `shipped_city_config_loads_and_validates` (`load_config::<CityParams>(&assets_root(),
CITY_CONFIG)` + `validate()`) и `unknown_city_field_names_file_and_field` (тот же приём, что
`unknown_field_names_file_and_field`, для `city.ron`: ошибка содержит `city.ron` и `bogus_field`).

→ существующие `movement`, `jump`, `terrain`, `config` зелёные без изменения своих чисел (проверено по
расписанию: игрок в TestArea появляется в первом `update()`, как и раньше при `Startup`; первый `update()`
под `FixedTimesteps(1)` фиксированных тиков не делает — `settle()` считает тики через `Time<Fixed>`).

### Шаг 9. Рантайм-гейты города `crates/gta_sim/tests/city.rs`

1. `runtime_hash_matches_golden`: для seed 1 и 2 — `city_app(seed)`; `CityLayoutHash.0 == golden(seed)`,
   `CitySeed.0 == seed`. Фиксированная величина — golden-файл (не результат той же сборки): ловит
   неверный seed, неверные параметры и расхождение float между сборками `-p citygen` и `-p gta_sim`.
2. `one_static_collider_per_building`: число сущностей `(With<CityBuilding>, With<Collider>,
   With<RigidBody>)` == `City.0.buildings.len()` > 0; ровно 1 `CityGround`.
3. `edge_walls_at_ground_edge`: ровно 4 `CityEdgeWall`; множество их `Transform.translation` ==
   `{(±(g/2+t/2), h/2, 0), (0, h/2, ±(g/2+t/2))}` ± 1e-3, где g, h, t из `City.0.ground_size` и
   `CityParamsRes` (не литералы).
4. `edge_wall_stops_player`: `city_app(1)`, `settle()`; `place_player(Vec3::new(g/2 − 5.0, 1.05, 0.0))`
   (в полосе `ground_margin`, зданий там нет: город кончается на `size/2 = 600`); `set_intent(axis =
   Vec2::X, yaw = 0)` (→ мир +X, `intent.rs:36`); `run_ticks(200)`. Разобранный пример: 200 тиков / 64 Гц
   = 3.125 с × 4.5 м/с ≈ 14 м → без стены x ≈ 695 + 13.7 ≈ 708.7 (за краем плиты, игрок падает).
   Assert: `x < g/2 − capsule_radius + 0.05` (699.75) и `y > 0.5` (стоит на земле).
5. `player_spawns_on_sidewalk_and_stands`: `city_app(1)`, `settle()` (64 тика, y ≈ `float_height`
   ± 0.05 — на земле, не на крыше/в коробке); xz игрока: расстояние до осевой ближайшего живого не-Alley
   ребра в `[half_carriageway, half_carriageway + sidewalk]` (ширины через `CityParams`), точка вне
   следов всех зданий.
6. `#[ignore] city_startup_budget` (V2: бюджет не только чистой `generate`): `city_app` с замером — время
   от первого `update()` до `Playing` и максимальная длительность одного `update()` (кадр применения);
   печатать оба; `assert!(total < 2 s)` (GDD §2.4, §11: "генерация ≤ 2 с в release под экраном загрузки").
   Длительность кадра применения — замер, не assert (под экраном загрузки; решение о time-slice
   принимается по числу, см. Шаг 12). Запуск: `cargo test -p gta_sim --release --test city --
   --ignored --nocapture`.

→ `cargo test -p gta_sim` зелёный.

### Шаг 10. Клиент

- `src/main.rs`: `fn parse_seed() -> Result<u64, String>` (линейный разбор `std::env::args()`:
  `--seed N`, u64 десятичный; без флага — `SystemTime::now().duration_since(UNIX_EPOCH)` nanos как u64);
  ошибку печатать `eprintln!` и `return AppExit::error()`, как ошибки конфигов (`main.rs:28-45`);
  `info!("city seed {seed}")`; `compose_sim(&mut app, root.clone(), WorldSource::City { seed })`;
  `.add_plugins(menu::MenuPlugin)`; `mod menu;`.
- Новый `src/menu/mod.rs` (домен клиента по GDD §12): `MenuPlugin` → `OnEnter(GameState::Loading)`
  `spawn_loading_screen(mut commands, seed: Res<CitySeed>)`: полноэкранный `Node` (100%×100%, по центру)
  + `BackgroundColor(Color::BLACK)` + `DespawnOnExit(GameState::Loading)`, дочерний
  `Text::new(format!("Generating city (seed {})", seed.0))`. Латиница в коде (Q3): дефолтный шрифт
  `bevy_text-0.19.1/src/FiraMono-subset.ttf` без кириллицы; `assets/ui/strings.ron` вводит T5.
- `src/visuals/mod.rs` (при росте > ~400 строк вынести город в `src/visuals/city.rs`):
  - `RenderConfig` + поля из Шага 4 (`district_colors: DistrictColors { downtown, commercial,
    residential, industrial: (f32, f32, f32) }`, `hospital_color`, `police_color`, `gang_hq_color`,
    `road_color`, `sidewalk_color`, `park_color`, `surface_layer_step`), `deny_unknown_fields` остаётся.
  - `Startup: setup_city_palette` → ресурс `CityPalette` (один `Handle<StandardMaterial>` на цвет, не на
    сущность).
  - Observer `visualize_city_building(On<Add, CityBuilding>, ...)` → `Mesh3d(Cuboid::from_size(size))` +
    материал: POI-цвет для `Hospital/PoliceStation/GangHq`, иначе цвет района.
  - `spawn_city_surfaces` в `OnEnter(GameState::Playing)` (исправлено: не `Update` +
    `resource_added`): читает `Res<City>`, спавнит три плоских меша: земля/асфальт по `ground_size`
    (y = 0), `curb`-многоугольники всех кварталов = тротуар (y = step), `inner` парков (или `curb`, если
    `inner` пуст) = газон (y = 2·step). Хелпер `fn flat_mesh(polys: impl Iterator<Item = &[Vec2]>, y: f32)
    -> Mesh`: `Mesh::new(PrimitiveTopology::TriangleList, RenderAssetUsages::default())`, позиции
    `(x, y, z)`, нормали `(0,1,0)`, `Indices::U32`, веер `(0, i+1, i)`.
    **Порядок обхода** (посчитано): треугольник a=(0,0,0), b=(0,0,1), c=(1,0,0) имеет `(b−a)×(c−a) =
    (0,1,0)`, а его ориентированная площадь в (x,z) = −0.5. Значит многоугольник с положительной площадью
    (наша конвенция) выдаётся веером `(0, i+1, i)`. Пример: квадрат (0,0),(1,0),(1,1),(0,1) (площадь +1)
    → треугольник (0,2,1) = (0,0),(1,1),(1,0): `(1,0,1)×(1,0,0) = (0,1,0)` → нормаль +Y ✓.
  - Почему `OnEnter(Playing)` без отдельного "флага готовности" (V2 предлагал): `apply_city_generation`
    в кадре N спавнит здания (observer вешает меши при применении команд в кадре N) и ставит
    `NextState`; в кадре N+1 один прогон `StateTransition` сначала деспавнит `DespawnOnExit(Loading)`
    (ExitSchedules), затем исполняет `OnEnter(Playing)` (surfaces, игрок) (`bevy_state-0.19.1/src/app.rs:
    244-265`); рендер извлекает мир после всего `Main`. Кадра "экран снят, геометрии нет" на уровне ECS
    нет. Остаточный риск — асинхронная компиляция пайплайнов (материалы появляются через кадр-два) —
    виден владельцу на первом кадре, машинерию не строим.
  - Существующий `visualize_block` не трогать. Деревья, фасады, слияние мешей, ориентиры — T3 (GDD §13).

→ `cargo build`, `cargo clippy -- -D warnings`; `cargo run --features fast -- --seed 1` показывает экран
загрузки и город (самопроверка, не гейт).

### Шаг 11. QA-инфраструктура

- `tools/qa/brp.py` (класс `Game`), только stdlib:
  - `resource_path(suffix)` — через `world.list_resources`, уникальный суффикс `::Name` (как
    `component_path`);
  - `resource(suffix)` — `world.get_resources` с `{"resource": path}` → `["value"]`; нормализовать число
    или `[n]` в `int`;
  - `wait_resource(suffix, timeout)` — опрос раз в 0.5 с, пока ресурс не появится (ошибка RPC = "ещё нет");
    таймаут → `TimeoutError` с хвостом `game.log`;
  - `mutate_component(entity, component, path, value)` — `world.mutate_components` с `{entity, component,
    path, value}`;
  - `load_golden()` — разбор `crates/citygen/tests/golden_hashes.txt` тем же правилом (строки, `trim`,
    `#`, hex).
- `tools/qa/scenarios/t1.py`: `Game(features=("dev",), args=("--seed", "1"))`; вместо `time.sleep(2)` —
  `game.wait_resource("CityLayoutHash", 180)`, затем ожидание строки `Player` (опрос `query` до 10 с) и
  пауза 1.0 с на посадку; остальные проверки T1 без изменений. (На seed 1 спавн на N-S тротуаре, W идёт
  вдоль него: jitter ±6 м на ребре ≥ 76 м даёт уход ≤ 4.5·12/76 ≈ 0.7 м при полуширине тротуара ≥ 1.5 м;
  здания в ≥ 1.5 м + setback от осевой тротуара. Если упор — проверить правило спавна, а не ослаблять t1.)
- Новый `tools/qa/scenarios/t2.py` (`--out <dir>`, по умолчанию `target/qa/t2`): для seed 1 и 2 отдельным
  запуском игры `Game(features=("dev",), args=("--seed", str(seed)))`:
  1. `wait_resource("CityLayoutHash", 180)`; `CitySeed == seed`; хэш == golden(seed);
  2. дождаться `Player`, пауза 1.5 с; скриншот со спавна `spawn_<seed>.png` (проверка PNG-сигнатуры);
  3. сущность игрока из `world.query` с `Player`; `mutate_component(player, <путь Position avian>, "",
     [0.0, 1.2, 0.0])`; если RPC вернул ошибку формы — попробовать `path=".0"` с тем же массивом, затем
     `{"x":0.0,"y":1.2,"z":0.0}`; рабочую форму зафиксировать в `brp.py` и в summary (форма Vec3 не
     считается проверенной до этого прогона);
  4. пауза 1.5 с; прочитать и `Position`, и `Transform` игрока: у обоих |x|,|z| < 1.0 и 0.9 < y < 1.3
     (стоит на центральном перекрёстке, не провалился; avian не перетирает свежий `Position`,
     `physics_transform/mod.rs:187-230`); скриншот `center_<seed>.png`;
  5. `diagnostics()` FPS в summary; `shutdown`.
  После обоих: хэши различны. `summary.json` с хэшами, seed, координатами, рабочей формой мутации,
  путями скриншотов.

→ QA: `python tools/qa/scenarios/t1.py --out ...` и `python tools/qa/scenarios/t2.py --out ...` проходят;
мультимодальный осмотр скриншотов.

### Шаг 12. Бюджеты и финальная проверка

`cargo build`, `cargo clippy -- -D warnings`, `cargo test -p gta_sim`, `cargo test -p citygen`,
`python tools/qa/tree_check.py`; файлы < 750 строк. Затем release-замеры:
`cargo test -p citygen --release --test perf -- --ignored --nocapture` (< 2 с на seed) и
`cargo test -p gta_sim --release --test city -- --ignored --nocapture` (время до `Playing` < 2 с,
печать длительности кадра применения). Если кадр применения > ~100 мс — записать число в IMPL_SUMMARY и
отметить для владельца (кадр под экраном загрузки); time-slice спавна (курсор-ресурс, N зданий за кадр,
`Playing` после последнего) вводить только если время до `Playing` превышает 2 с. Чанковую инфраструктуру
T3 не вводить. Затем BRP QA t1/t2.

---

## 4. Test plan

### 4.1 Гейты и их класс

| Гейт | Класс | Что несёт |
|---|---|---|
| `citygen` unit: `geom`, `grid`, `hash` vectors, `validate` | корректность | геометрия, шаги сетки в диапазоне, стабильность хэш-примитивов, отказ плохих параметров |
| `golden.rs::golden_hashes_match` | корректность (регрессия) | тихое изменение генерации |
| `properties.rs` 1-3 | корректность | связность дорог, сильная связность полос, связность тротуаров |
| `properties.rs` 4-6 | корректность | непересечение лотов, frontage к живой не-переулочной улице по id, здания в лотах |
| `properties.rs` 7 + 11 | корректность | больница, участок(и), 2 штаба в разных районах Residential/Industrial; гарантия перекраской |
| `properties.rs` 8 + `city.rs` 5 | корректность | спавн на тротуаре, игрок стоит на земле |
| `city.rs` 1 | корректность проводки | seed/params/хэш в рантайме == golden; float одинаков в двух сборках |
| `city.rs` 2 | liveness + счёт | коллайдер на каждое здание, одна земля |
| `city.rs` 3-4 | корректность | 4 стены на краю земли, игрок не проходит край |
| `config.rs` новые | корректность | `city.ron` грузится и валиден; неизвестное поле называет файл и поле |
| `perf.rs`, `city.rs` 6 (`#[ignore]`) | бюджет | генерация < 2 с; время до `Playing` < 2 с; кадр применения — замер |
| `t1.py`, `t2.py` | runtime QA | T1 не сломан на городе; BRP видит хэш и seed, телепорт работает, скриншоты |
| owner checklist | feel/визуал | 4.3 |

Liveness готовности города (`city_app` дошёл до `Playing`) отделена от корректности layout (golden +
properties).

### 4.2 Flip-RED (каждый гейт показать красным, восстановить, показать зелёным; в IMPL_SUMMARY записать, что испорчено)

| Гейт | Порча механизма/входа | Ожидаемо |
|---|---|---|
| golden | `grid.node_jitter` 6.0 → 6.5 в `city.ron` | RED с новыми хэшами; вернуть |
| grid steps | в `split_interval` вернуть старое правило "остаток > max пополам" | RED (шаг < lo) |
| road connected | не строить рёбра последней z-линии | RED (узлы периметра изолированы) |
| lanes strongly connected | строить полосы только в направлении a→b | RED |
| sidewalks connected | не строить "зебры" | RED |
| lots overlap | в `clip_half_plane` второму ребёнку отдавать исходный многоугольник | RED |
| lots face street (б) | убрать проверку frontage при принятии разреза | RED (внутренние лоты глубоких кварталов) |
| lots face street (а) | `inner` строить с отступом только `half_carriageway` (без тротуара) | RED (расстояние до ребра) |
| buildings inside | не сжимать прямоугольник (0 шагов подгонки) | RED на непрямоугольных лотах |
| pois | пропустить размещение участка | RED |
| gang guarantee | отключить перекраску в `districts.rs` | RED (`generate` → `Err` на весах R=I=0) |
| hash vectors | `quantize_mm` через `as i64` без `round` | RED (`1.2345 → 1234`) |
| runtime golden | в `WorldPlugin` передать `seed ^ 1` | RED |
| collider count | пропускать каждое 10-е здание при спавне | RED |
| edge walls position | стену +X ставить в `g/2` вместо `g/2 + t/2` | RED |
| edge wall stops player | не спавнить стену +X | RED (x ≈ 708 > 699.75) |
| player on sidewalk | спавн = середина ребра без сдвига (на проезжей части) | RED |
| perf / startup budget | временный `sleep(2.1 s)` в `generate` | RED |

### 4.3 Owner checklist (QA переносит в QA_REPORT.md)

- `cargo run --release -- --seed 1`: экран загрузки, затем город; бегает по тротуарам и улицам.
- Видны улицы (асфальт), тротуары, кварталы разной высоты (Downtown высокий в центре, окраины низкие),
  парки-газоны, больница/участок/штабы выделены цветом.
- `--seed 2` (и без `--seed`) даёт другой город.
- Край: за периметром полоса земли 100 м, дальше невидимая стена — не выпасть, не пройти.
- Снятие экрана загрузки без заметного "попа" пустого мира (если есть — сообщить; это feel, не гейт).

---

## 5. Rollout notes

- **Зависимости/lock:** `glam =0.32.1`, `rand_chacha =0.10.0` (+ транзитивный `ppv-lite86 0.2.21`),
  `serde`, dev `ron` в citygen; `citygen` в gta_sim. Lock — готовый `scratch/Cargo.lock.t2` (проверен в
  этом ревью как текущий lock + ровно ожидаемый дифф). `bevy_render` в нормальном дереве `gta_sim`
  отсутствует (`tree_check.py`).
- **Изменение API:** `compose_sim` получает третий параметр `WorldSource`; оба вызывающих
  (`src/main.rs`, `tests/common/mod.rs`) обновляются в этой задаче. `world::spawn_test_area` больше не
  публичный. `spawn_player` переезжает с `Startup` на `OnEnter(GameState::Playing)`.
- **Новое состояние:** приложение стартует в `GameState::Loading`. Любая будущая система с `Res<City>`
  должна гейтиться набором `in_state(Playing)` (ресурсы города появляются до `Playing`, но не в `Loading`).
- **Новые данные:** `assets/world/city.ron` (новый), поля в `assets/world/render.ron`. Любая правка
  `city.ron`, влияющая на генерацию, меняет golden — обновлять процедурой bless (Шаг 5), не автоматически.
- **Env/feature flags:** нет новых. CLI: `--seed N` (без него — случайный seed из времени, пишется в лог).
- **Переносимость golden:** golden — регрессионный контракт для поддерживаемой сборки (x86_64 Windows,
  зафиксированные версии). Побитовое равенство на других платформах не доказано (Rand Book,
  https://rust-random.github.io/book/crate-reprod.html, ограниченные гарантии); запрет трансцендентных
  функций и квант 1 мм это снижают. Отличие на другой платформе — это риск, а не баг гейта.
- **Ошибка генерации в рантайме** (невозможна на shipped-данных для 35 проверенных seed, но возможна на
  случайном seed при экзотических данных): лог `error!` + `AppExit::error()`.
- **Миграций сохранений нет** (сохранений нет).

---

## 6. Review notes (что изменено относительно PLAN_V2.md и PLAN.md)

**Проверенный контрпример (disconfirmation).** Самый конкретный вход, делающий план неверным: правило
"остаток r > max делится пополам" на shipped-данных. Проба `scratch/pr2/split_probe.py`: на интервале
250 м с шагами 70-90 старое правило даёт квартал < 70 м в 10008 из 20000 прогонов — V2 п.2 подтверждён.
Второй проверенный контрпример — утверждение V2 п.6 "экран загрузки снимется до геометрии": по
исходнику `bevy_state-0.19.1/src/app.rs:244-265` деспавн `DespawnOnExit` и `OnEnter(Playing)` идут в одном
прогоне `StateTransition`, меши зданий висят с кадра раньше — на уровне ECS **не подтвердился**.

Исправления PLAN_V2 (V2 побеждает PLAN.md везде, кроме пунктов ниже, где проверка показала иное):
1. **Сетка (V2 п.2 → конкретный алгоритм).** V2 требовал "перераспределение" без алгоритма. Дан
   `split_interval` (n = ceil(L/hi), равная база + нулевое среднее смещений в ±δ), условие осуществимости в
   `validate`, разобранные примеры 250/350/160/90/91/130 и unit-тест.
2. **POI (V2 п.3 → конкретная гарантия).** Районы назначаются после итоговых кварталов; перекраска до двух
   Residential/Industrial с кварталами; первый застраиваемый квартал района не парк; банды назначаются
   раньше больницы/участка; `generate` возвращает `Result` с `GenError` вместо fallback в любой район (он
   нарушал инвариант) и вместо паники в async-задаче. Отклонение от сигнатуры GDD §2.2 (`-> CityLayout`)
   осознанное: интерфейс layout не меняется. Тест `gang_guarantee_recolors` + flip-RED.
3. **Frontage (V2 п.4 → проверка по id).** `Block.sides` — id живых рёбер после компактизации,
   `inner.len() == sides.len()`; гейт (а) связывает каждую сторону `inner` с живым ребром по id и
   расстоянию, (б) требует frontage к не-Alley стороне. Решение: переулок не считается улицей для
   правила Access (GDD §2.3 "без тротуаров, без трафика"); PLAN.md считал все стороны `inner` улицей.
4. **Экран загрузки (V2 п.6 отклонён по исходнику).** Вместо клиентского "признака готовности" —
   `DespawnOnExit(Loading)` + `spawn_city_surfaces` в `OnEnter(Playing)` (PLAN.md ставил его в `Update`
   по `resource_added`, что позволяло кадровый разрыв). Остаток — компиляция пайплайнов — owner-run.
5. **Бюджет (V2 п.7 → конкретный гейт).** Добавлен `#[ignore] city_startup_budget` в gta_sim (время до
   `Playing` < 2 с, печать кадра применения) и правило, когда вводить time-slice.
6. **Golden (V2 п.8 → явная схема).** Полный порядок полей, коды enum, `HASH_SCHEMA_VERSION`, test vectors
   FNV/кванта (посчитаны пробой), процедура bless через `#[ignore] bless_print_golden` вместо "тест
   печатает новый файл, вставить". PLAN.md-утверждение о побитовом равенстве float на всех x86_64/aarch64
   снято.
7. **Край (V2 п.1 → конкретные стены).** `edge_wall: (height, thickness)` в `city.ron`, точные позиции,
   гейты `edge_walls_at_ground_edge` и `edge_wall_stops_player` с разобранным числом (x ≈ 708.7 без стены).
   Толщина стены — данные, чтобы не спорить о const.
8. **Телепорт (V2 п.5).** По исходнику avian свежий `Position` не перетирается `transform_to_position`,
   поэтому мутация только `Position` корректна; сценарий всё равно проверяет и `Position`, и `Transform`
   и перебирает формы значения, фиксируя рабочую.
9. **glam `nostd-libm`.** Утверждение PLAN.md, что сборки `-p citygen` и `-p gta_sim` дают разные
   трансцендентные функции, неверно: при включённом `std` glam берёт std-математику
   (`glam-0.32.1/src/f32/math.rs:35,164`). Запрет остаётся как гигиена переносимости; рантайм-гейт по
   golden остаётся.
10. **Детали, восстановленные из PLAN.md** (V2 их сжал, не исправляя): точные типы `CityParams`/`CityLayout`,
    полный `city.ron`, сигнатуры систем, расписания, BRP-пути, поворот коробки с тремя примерами, порядок
    обхода меша, таблица flip-RED, owner checklist, lock-процедура.
11. **Мелочи:** `layouts()` с `OnceLock` (35 генераций на бинарь, а не на каждую функцию); `place_player`
    вынесен в `common`; `StatesPlugin` в харнессе обязан стоять до `compose_sim` (`init_state` паникует без
    `StateTransition`, `app.rs:102-104`); ориентированная площадь треугольника-примера = −0.5 (PLAN.md писал
    −1, знак тот же); спавн исключает кварталы с пустым `inner`.

Открытых вопросов к владельцу нет (Q1-Q3 закрыты в TASK_FINAL.md).

children: 0 launched / 0 reported.
