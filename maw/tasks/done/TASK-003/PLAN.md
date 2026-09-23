# PLAN — TASK-003 (GDD T2): генератор города v1 и прогулка

Цена ошибки смешанная. Молча ломаются и получают исполняемые гейты: детерминизм генерации (golden-хэш),
связность графа дорог и сильная связность графа полос (T8/T15 строятся на них), непересечение лотов и
выход к улице, наличие больницы/участка/штабов (T5, T9, T11 от них зависят), проводка seed → layout →
`CityLayoutHash` в рантайме, бюджет генерации. Облик города, "кварталы разной высоты", парки, вид с земли
владелец видит на первом кадре: это его прогон (owner checklist в QA_REPORT), под это гейтов не строим.

Все API Bevy/avian/glam/rand ниже сверены с исходниками в
`~/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/` (версии из `Cargo.lock`), ссылки `файл:строка`.

---

## 1. Understanding — что есть сейчас

**Workspace** (`/Cargo.toml:1-51`): члены `crates/gta_sim`, `crates/citygen`; пины `bevy =0.19.1`,
`avian3d =0.7.0`, `bevy-tnua =0.32.0`, `ron =0.12.2`, `serde 1`. Патчи `vendor/` (ADR-001).
`profile.dev` opt-level 1 для членов workspace, 3 для зависимостей.

**`crates/citygen`**: пустой. `Cargo.toml:1-6` без зависимостей, `src/lib.rs:1` одна строка `//!`.

**`crates/gta_sim`**:
- `src/lib.rs:16-26` — `compose_sim(app, root) -> Result<(), ConfigError>`: грузит
  `LocomotionConfig`, вставляет `ConfigRoot`, добавляет `PhysicsPlugins`, `TnuaAvian3dPlugin`,
  `CharacterPlugin`, `WorldPlugin`, `PlayerPlugin`.
- `src/config/mod.rs:5-38` — `ConfigRoot`, `ConfigError { path, message }`, `load_config<T>` (ron, строгость
  даёт `#[serde(deny_unknown_fields)]` на структуре).
- `src/world/mod.rs:1-24` — `Block { size }` (Reflect), `PlayerSpawn(Vec3)` = `Vec3::ZERO`,
  `Startup: spawn_test_area`. `src/world/test_area.rs:5-45` — таблица коробок тестовой площадки
  (пол 80×80, коробки, рампа 30°, лестница, стена), комментарий "citygen replaces this table in T2".
- `src/player/mod.rs:11-30` — `Startup: spawn_player.after(spawn_test_area)`, спавн на
  `PlayerSpawn + Y*float_height`.
- `src/character/*` — Tnua-архетип, `drive_characters` в `FixedUpdate`. Не меняется.
- Состояний (`States`) в проекте нет. `flow/` нет.

**Тесты `crates/gta_sim/tests/`**: `common/mod.rs:21-29` `headless_app()` = `MinimalPlugins` +
`TransformPlugin` + `AssetPlugin` + `TimeUpdateStrategy::FixedTimesteps(1)` + `compose_sim` +
`finish/cleanup`. `terrain.rs` завязан на геометрию тестовой площадки (коробка в (10,0.5,10), лестница у
z=−12), `movement.rs`/`jump.rs` на ровный пол у начала координат, `config.rs` на строгий загрузчик.

**Клиент `src/`**: `main.rs:18-58` — `DefaultPlugins` + `compose_sim` + камера/ввод/визуал, `--seed` не
парсится. `visuals/mod.rs:49-63` — observer `On<Add, Block>` вешает `Cuboid` меш и **новый материал на
каждую сущность**. `camera/mod.rs:70-121` `follow_player` берёт `Single<.., With<Player>>`;
`input/mod.rs:88-109` `write_move_intent` тоже `Single` — при отсутствии игрока системы пропускаются
(`bevy_ecs-0.19.1/src/system/system_param.rs:371-381`: `QuerySingleError::NoEntities` →
`SystemParamValidationError::skipped`).

**QA**: `tools/qa/brp.py:15-134` (`Game(features, args)`, `call`, `component_path`, `query`, `screenshot`,
`diagnostics`, `shutdown`), `tools/qa/scenarios/t1.py` (ждёт 2 с после BRP и сразу читает `Player`;
W 1000 мс, ожидает смещение по −Z > 2 м, |x| < 0.5).

**Premise** (`PREMISE_CHALLENGE.md`): PREMISE HOLDS — генератора нет, мир фиксированный.

**GDD, что касается T2**: §2.2 конвейер (сетка с jitter узлов, районы Voronoi, кварталы, лоты OBB с
правилами Area/Access/Frontage, коробки, POI, графы тротуаров и полос), §2.3 масштаб, §2.4 async под
экраном загрузки ≤ 2 с, §2.5 детерминизм (`rand_chacha` ChaCha8Rng, подпоток на этап
`seed_from_u64(hash(world_seed, stage_tag, cell_id))`, не сэмплировать `usize`, гейты), §2.6 MVP мира,
§6.3 (территории банд из Residential/Industrial по seed, штаб в районе банды), §7 (экран загрузки,
`--seed N`), §10.5 (`CityLayoutHash` с Reflect для BRP, `--seed`), §12 (`world/` владеет `city.ron`,
CityLayout как Resource, async генерация; `flow/` владеет `GameState`; `visuals/` читает `render.ron`),
§13 T2.

---

## 2. Approach

### 2.1 Крейт `citygen`: чистая функция

`pub fn generate(seed: u64, params: &CityParams) -> CityLayout` и `pub fn layout_hash(&CityLayout) -> u64`.
Без Bevy, без глобального RNG. Зависимости (все уже в локальном реестре, проверено пробой, см. 2.6):
`glam =0.32.1` (та же версия, что у `bevy_math` 0.19.1 в `Cargo.lock`, поэтому `glam::Vec2` это тот же
тип, что `bevy::math::Vec2`), `rand_chacha =0.10.0` (GDD §10.1; `rand_core ^0.10` → 0.10.1, уже в
lock), `serde` (derive для параметров). Dev: `ron`.

Координаты: `Vec2(x, y)` в citygen = мировые `(x, z)`, земля y = 0. Центр города в начале координат.

**Конвейер** (порядок фиксирован, каждый этап берёт свой подпоток RNG, см. 2.3):

1. **Линии сетки.** X-линии строятся от 0 наружу отдельно в + и в − (разные подпотоки, город не
   симметричен). Шаг берётся из `grid.core_block` (70-90 м), если |x| < `grid.core_radius`, иначе из
   `grid.outer_block` (100-160 м) — это "шаг из районной таблицы" GDD §2.2 в виде ядро/окраина (Downtown в
   центре). Цикл: пока `half - x > max + min`, `x += range(min, max)`; остаток `r` (он в `(min, max+min]`):
   если `r ≤ max` — один квартал, иначе два по `r/2`. Последняя линия ровно на `±size/2` (периметр).
   Z-линии так же. Центральные линии (x = 0, z = 0) существуют всегда.
2. **Узлы и jitter.** Узел (i, j) = (xs[i], zs[j]) + равномерный jitter ±`grid.node_jitter` по обеим
   осям. Исключения: центральный узел (0, 0) без jitter; узлы периметра двигаются только вдоль своей
   линии периметра; углы города без jitter. Итог: центр города всегда ровно перекрёсток двух авеню (для
   телепорта QA в центр), город квадратный.
3. **Классы рёбер.** Ребро наследует класс своей линии: линия с `|i - center| % grid.avenue_every == 0`
   — Avenue, иначе Street. Центральные линии — авеню.
4. **Районы.** Семена на джиттерной сетке `districts.grid`×`districts.grid` (нечётное, по умолчанию 3):
   семя = центр ячейки + jitter ±`districts.seed_jitter`×ячейки; центральное семя ровно в (0,0), вид
   Downtown. Остальные виды по весам `districts.weights` (Commercial/Residential/Industrial); если
   Residential+Industrial < 2, последние по id Commercial перекрашиваются в Residential до двух
   (гарантия места под 2 территории банд). Район квартала = ближайшее семя к центроиду исходной ячейки
   (сравнение квадратов расстояний, ничья → меньший id). "Шум плотности" GDD §2.2 в v1 не делаем: его
   нечем проверить, и разнообразие даёт таблица этажности. Это упрощение, не новая механика.
5. **Суперкварталы и переулки.** Внутренние (не периметр) рёбра в порядке id; `r = unit()`.
   `r < superblock_chance` → удалить ребро и слить два соседних квартала; иначе
   `r < superblock_chance + alley_chance` → ребро становится Alley. Условие для обоих: ни один из двух
   соседних кварталов ещё не помечен (после операции оба помечены), для слияния ещё — объединённый
   многоугольник выпуклый (допуск на ~180° в вершинах удалённого ребра). Слитый квартал берёт район
   квартала с меньшим id.
   **Почему связность держится по построению:** множество помеченных рёбер двойственно паросочетанию
   граней (каждая грань касается не более одного помеченного ребра), периметр не трогаем, значит в
   двойственном графе помеченные рёбра не образуют цикла, а разрез в плоском графе ⇔ цикл в двойственном.
   Граф дорог без удалённых рёбер и граф "без удалённых и без переулков" остаются связными. Степень
   любого узла в графе без переулков ≥ 2 (вокруг внутреннего узла 4 грани, три помеченных ребра
   потребовали бы 6 разных граней; узел периметра теряет максимум 1 внутреннее ребро). Тест всё равно
   проверяет.
6. **Кварталы.** Многоугольник квартала — узлы дороги по кругу, положительная ориентированная площадь в
   (x, z) (формула `Σ(x_i·z_{i+1} − x_{i+1}·z_i)/2 > 0`), плюс `sides[k]` = id ребра между `nodes[k]` и
   `nodes[k+1]`. Два отступа (выпуклый inset с отступом на каждую сторону): `curb` = отступ на половину
   проезжей части стороны (Avenue 2×2×3.25/2 = 6.5 м, Street 3.25 м, Alley `alley_width/2`), `inner` =
   ещё плюс тротуар стороны (Avenue 4 м, Street 3 м, Alley 0). Если `inner` вырожден (не выпуклый,
   площадь ≤ 0, сменил ориентацию) — квартал становится парком с пустым `inner` (газон тогда рисуется по
   `curb`, лотов нет; свойство 5 проверяет только непустые `inner`).
7. **Парки.** Квартал целиком парк с вероятностью `park_share` его района (подпоток на квартал).
8. **Лоты.** Рекурсивное OBB-деление `inner` (Vanegas 2012, Martin Evans "Lots": срез поперёк длинной оси
   OBB; правила Area / Access / Frontage). Каждая сторона многоугольника несёт флаг "на улице"; стороны
   `inner` все на улице, стороны от разреза — нет. Узел: если площадь ≤ `lot_area.1` района — лист.
   Иначе OBB минимальной площади (перебор направлений рёбер, только `+ − * / sqrt`), разрез
   перпендикулярно длинной оси через центр со сдвигом `±split_jitter` доли длины; клип выпуклого
   многоугольника полуплоскостью (Sutherland–Hodgman для одной прямой). Разрез принимается, если у обоих
   детей площадь ≥ `lot_area.0` и самая длинная уличная сторона ≥ `min_frontage`; иначе пробуем короткую
   ось; иначе лист. Глубина ≤ 16. Так каждый лист выходит к улице по построению.
9. **Здания (коробки).** В каждом лоте (подпоток на лот): ось `u` = направление самой длинной уличной
   стороны лота (фасад к улице), `v = u.perp()`; лот сжимается на `setback` района; прямоугольник в кадре
   (u, v) = проекции сжатого многоугольника, центр = его центроид; пока хоть один угол вне многоугольника
   — полуоси ×0.9 (не более 24 шагов, константы алгоритма подгонки, а не тюнинг). Не влез или площадь <
   `min_building_area` — лот без здания. Высота = `range_u32(floors)` × `floor_height` района.
10. **Точки интереса.** Кандидаты — здания не-парковых лотов, сортировка по площади следа убыв., id возр.
    Больница: первый кандидат в районах из `pois.hospital_districts`, иначе первый не-Downtown, иначе
    первый. Участки (`pois.police_stations`, по умолчанию 1): то же по `pois.police_districts`, минус
    занятые. Территории банд: районы вида Residential/Industrial, у которых есть кандидат, в порядке
    перемешивания подпотоком `GANGS`; первые два → `gang_districts: [u32; 2]`; штаб = крупнейшее здание
    района. Запасной путь: если таких районов < 2, добираем из любых не-Downtown с кандидатом. POI это
    `BuildingKind::{Hospital, PoliceStation, GangHq(0|1)}` у здания; отдельного списка нет.
11. **Граф тротуаров.** Узел на каждом углу каждого квартала (точка на осевой тротуара: отступ
    `half_carriageway + sidewalk/2` у сторон угла; для переулочной стороны — по линии бордюра). Рёбра:
    кольцо вдоль сторон квартала; "зебры" — для каждого неудалённого ребра дороги с кварталами по обе
    стороны на каждом его конце соединяем угловые узлы этих двух кварталов; дорожки парка — узел в
    центроиде парка, рёбра к его углам (GDD §2.2 "дорожки через парки как рёбра").
12. **Граф полос.** Для каждого ребра не-Alley: `n` полос в каждую сторону (Avenue 2, Street 1),
    правостороннее движение. Для направления `d` (единичный, (x,z)) правая сторона мира = `d.perp()` =
    `(−d.y, d.x)`; полоса `i` смещена на `(i+0.5)·lane_width` вправо; концы подрезаны на полуразмер
    перекрёстка (максимум `half_carriageway` рёбер узла). Коннекторы: в каждом узле с каждой входящей
    полосы ребра `e` на каждую исходящую полосу каждого ребра `f ≠ e` (без разворотов), с полем
    `intersection` = id узла. Зона конфликта v1 = весь перекрёсток (T15 резервирует её, GDD §5.2).
    **Почему сильная связность держится:** граф полос без разворотов — это non-backtracking граф дорожного
    графа (с размножением по полосам). Для связного графа с минимальной степенью ≥ 2, не являющегося
    циклом, non-backtracking матрица неприводима (Glover & Kempton, Prop. 2.3,
    https://arxiv.org/pdf/2011.09385 ; Lemma 3.2 в https://arxiv.org/pdf/math/0403414). Минимальная
    степень ≥ 2 гарантирует п.5. Тест проверяет прямо.
13. **Точка спавна игрока.** Среди сторон кварталов, чьё ребро не Alley и идёт "север–юг"
    (|dz| > |dx|), берём ту, у которой середина тротуара ближе всего к (0,0) (ничья → меньший id); спавн =
    середина ребра + `inward_normal × (half_carriageway + sidewalk/2)`. Игрок по умолчанию смотрит в −Z
    (yaw 0), то есть вдоль тротуара: W в `t1.py` не упирается в здание.
14. **Земля.** `layout.ground_size = size + 2·ground_margin` (квадрат с центром в 0).

**Хэш** (`layout_hash`): FNV-1a 64 (offset `0xcbf29ce484222325`, prime `0x100000001b3`,
http://www.isthe.com/chongo/tech/comp/fnv/) по канонической байтовой записи всего `CityLayout` в
фиксированном порядке: seed, size, узлы, рёбра, районы, кварталы (узлы, стороны, curb, inner, use),
лоты, здания, `gang_districts`, граф тротуаров, граф полос, спавн, земля. Длины коллекций как `u32 LE`,
enum как `u8`, float квантуется в миллиметры `(v * 1000.0).round() as i64` → `LE`. Свой хэш вместо
`std::hash::DefaultHasher`: алгоритм std не специфицирован и может смениться между релизами Rust.

**Детерминизм float.** В citygen разрешены только `+ − * /`, `sqrt`, `abs`, `min/max`, `round`, сравнения
(IEEE, корректно округляются, одинаковы на всех x86_64/aarch64). Никаких `sin/cos/atan2/powf/exp`:
при сборке через `gta_sim` у glam включается `nostd-libm` (из `bevy_math`), при `cargo test -p citygen`
нет (`cargo tree -e features -i glam@0.32.1`: у `gta_sim` фичи `std, nostd-libm, rand, serde, approx,
bytemuck`), и трансцендентные функции могли бы дать разные биты. Равенство хэша в двух сборках ловит
рантайм-гейт (Шаг 9) по тому же golden-файлу.

### 2.2 Данные: `assets/world/city.ron` (владелец `world/`, тип `citygen::CityParams`)

Все числа генератора — здесь (GDD §2.3, §12). Структура целиком `#[serde(deny_unknown_fields)]`,
районы — структура с четырьмя именованными полями (не map), чтобы пропуск района был ошибкой. Плюс
семантическая проверка `CityParams::validate() -> Result<(), String>` (сообщение называет поле), которую
`compose_sim` превращает в `ConfigError { path, message }`.

```ron
(
    size: 1200.0,
    ground_margin: 100.0,
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

Числа из GDD §2.3 (размер, кварталы, полоса 3.25, авеню 4 полосы + 4 м, улица 2 полосы + 3 м, переулок
4-5 м, этажи 3.1/3.9, этажность по районам). Остальное (доли, лоты, отступы) — стартовые данные, их
крутит владелец. `validate()` проверяет: размеры > 0; `min ≤ max` во всех диапазонах; `floors.0 ≥ 1`;
`4·node_jitter < min(core_block.0, outer_block.0)`; вероятности в [0,1] и
`superblock_chance + alley_chance ≤ 1`; `districts.grid` нечётное ≥ 3; сумма весов > 0;
`police_stations ≥ 1`; `min(core_block.0, outer_block.0) > 2·(6.5 + 4.0)` (иначе `inner` авеню-квартала
вырожден; числа считаются из `roads`, а не литералами).

**Законы (const, не тюнинг):** 1 unit = 1 м, Y вверх, `Vec2(x,y)` citygen = мир `(x,z)`; теги подпотоков
RNG; квант хэша 1 мм; параметры FNV; шаг подгонки прямоугольника 0.9 × 24; толщина плиты земли-коллайдера
1 м (коллизионная геометрия, как пол тестовой площадки `test_area.rs:7`). Каждая такая константа с
однострочным комментарием "почему закон".

**Цвета** — презентация, `assets/world/render.ron` (владелец `visuals/`): `district_colors` (4 поля),
`hospital_color`, `police_color`, `gang_hq_color`, `road_color`, `sidewalk_color`, `park_color`,
`surface_layer_step` (подъём плоских слоёв над землёй против z-fighting, 0.02 м).

### 2.3 RNG

`fn stream(seed: u64, tag: u64, cell: u64) -> ChaCha8Rng` = `ChaCha8Rng::seed_from_u64(mix)`,
`mix` = FNV-1a от `seed`, `tag`, `cell` (LE). Теги: `GRID_POS_X, GRID_NEG_X, GRID_POS_Z, GRID_NEG_Z,
JITTER, DISTRICTS, EDGES, PARKS, LOTS, BUILDINGS, GANGS` (u64-константы). Ячейка = id квартала/лота
или 0. Так изменение числа лотов в одном квартале не сдвигает поток другого.
Выборки только через свои хелперы (`rng.rs`): `unit_f32 = (next_u32() >> 8) as f32 * 2^-24`,
`range_f32(lo, hi)`, `range_u32(lo, hi_inclusive) = lo + (next_u64() % span) as u32`, `chance(p)`.
`usize` не сэмплируется (GDD §2.5, Rand Book https://rust-random.github.io/book/crate-reprod.html).
API проверен пробой (`scratch/probe_rng/src/main.rs`, запуск прошёл):
`use rand_chacha::{ChaCha8Rng, rand_core::{Rng, SeedableRng}}; ChaCha8Rng::seed_from_u64(1).next_u32()`.
`seed_from_u64` в rand_core 0.10.1 объявлен value-stable ("Changing the implementation ... value-breaking
change", `rand_core-0.10.1/src/seedable_rng.rs:107-109`); версии пинятся `=` + lock.

### 2.4 `gta_sim`: `flow/` + `world/` + `player/`

- **`flow/`** (новый домен по GDD §12): `GameState { #[default] Loading, Playing }`
  (`#[derive(States, Default, Clone, PartialEq, Eq, Hash, Debug)]`), `FlowPlugin` → `init_state`.
  Только два нужных сейчас варианта; `Paused/Wasted/Busted` добавят их слайсы.
  `init_state` требует `StatesPlugin` (`bevy_state-0.19.1/src/app.rs:102-104`); он есть в `DefaultPlugins`
  (`bevy_internal-0.19.1/src/default_plugins.rs:91`), в `MinimalPlugins` нет (`:163-170`) → тестовый
  харнесс добавляет `StatesPlugin`. `StatesPlugin` ставит `StateTransition` перед `PreStartup` и после
  `PreUpdate` (`app.rs:333-336`): `OnEnter(Loading)` исполняется до `Startup`, `NextState` из него
  применяется в том же первом `update()`. `DespawnOnExit` включается сам в `init_state` (`app.rs:113`).
- **`WorldSource`** — новый параметр композиции: `enum WorldSource { TestArea, City { seed: u64 } }`.
  `compose_sim(app, root, source)`. `TestArea`: `OnEnter(Loading)` спавнит площадку и сразу
  `NextState(Playing)` — T1-гейты остаются на своей фикстуре. `City`: `compose_sim` грузит и валидирует
  `city.ron`, `OnEnter(Loading)` запускает задачу, `Update`-система в наборе, загейченном
  `in_state(Loading)` на уровне набора (`configure_sets`), опрашивает её. Отвергнуто: перевести T1-гейты
  на город (в городе нет лестницы и коробки из `terrain.rs`).
- **Async (маркерный паттерн, domain bevy-ecs):** Requested = `OnEnter(Loading)`;
  InFlight = ресурс `CityGenTask(Task<(CityLayout, u64)>)`, тело задачи чистое:
  `generate(seed, &params)` + `layout_hash`; Ready = `block_on(poll_once(&mut task.0))` вернул `Some`
  (`bevy_tasks-0.19.1/src/lib.rs:85,107`; `AsyncComputeTaskPool::get().spawn` `task_pool.rs:559`,
  `usages.rs:29`). Голого `block_on(task)` нет. Apply в том же кадре: ресурсы `City(CityLayout)`,
  `CityLayoutHash(u64)`, `PlayerSpawn`, спавн коллайдеров, `NextState(Playing)`, удаление `CityGenTask`.
  Одноразовый спавн ~1-2 тыс. статиков происходит под экраном загрузки, в кадре перехода; это не
  геймплейный кадр.
- **Сущности (только физика, геймплей):** земля — `RigidBody::Static` + `Collider::cuboid(g, 1.0, g)`
  (полные длины: `avian3d-0.7.0/src/collision/collider/parry/mod.rs:745-747`, как в `test_area.rs:41`) с
  верхом в y = 0; здание — `CityBuilding { size: Vec3, district: DistrictKind, kind: BuildingKind }` +
  `RigidBody::Static` + `Collider::cuboid(2hx, h, 2hz)` + `Transform` (центр (c.x, h/2, c.y),
  `Quat::from_rotation_y(f32::atan2(-u.y, u.x))`). Рантайм не участвует в хэше, трансцендентная функция
  здесь допустима.
  Проверка поворота (локальная X коробки должна лечь на (u.x, 0, u.y); `R_y(θ)·X = (cosθ, 0, −sinθ)`):
  1. u = (1,0) → θ = atan2(0,1) = 0 → X → (1,0,0) ✓.
  2. u = (0,1) (мир +Z) → θ = atan2(−1,0) = −90° → (cos(−90°), 0, −sin(−90°)) = (0,0,1) ✓.
  3. u = (0,−1) (мир −Z) → θ = +90° → (0,0,−1) ✓.
- **BRP:** `CityLayoutHash(pub u64)` и `CitySeed(pub u64)` — `#[derive(Resource, Reflect)]
  #[reflect(Resource)]`, `register_type` как у `Block`. Новый тип с одним полем сериализуется reflect'ом
  как голое число (`bevy_reflect-0.19.1/src/serde/ser/tuple_structs.rs:47-49`), ответ
  `world.get_resources` — `{"value": ...}` (`bevy_remote-0.19.1/src/builtin_methods.rs:484-487`).
- **Игрок:** `spawn_player` переезжает с `Startup` на `OnEnter(GameState::Playing)`. Остальное как было.

### 2.5 Клиент

- `main.rs`: разбор `--seed N` (u64, десятичный; без `--seed` — seed из `SystemTime` nanos; ошибка
  разбора → `eprintln!` + `AppExit::error()`), `info!` с seed, `compose_sim(.., WorldSource::City { seed })`.
- `menu/` (новый домен клиента, GDD §7 "экран загрузки"): `OnEnter(Loading)` — полноэкранный `Node` с
  `BackgroundColor` и `Text::new(format!("Generating city (seed {seed})"))`, `DespawnOnExit(Loading)`.
  Текст латиницей в коде: дефолтный шрифт `bevy_text-0.19.1/src/FiraMono-subset.ttf` — 95 глифов, без
  кириллицы (проверено fontTools); `assets/ui/strings.ron` вводит T5 (GDD §7 требует его для
  "ПОТРАЧЕНО"/"BUSTED").
- `visuals/`: палитра материалов (один `Handle<StandardMaterial>` на цвет, а не на сущность), observer
  `On<Add, CityBuilding>` → `Cuboid::from_size(size)` + материал района/POI; при появлении `City`
  (`run_if(resource_added::<City>)`, `bevy_ecs-0.19.1/src/schedule/condition.rs:853`) — три плоских меша:
  земля/асфальт (y = 0), бордюрные многоугольники всех кварталов = тротуар (y = step), `inner` парков =
  газон (y = 2·step). Меш — веер треугольников, `Mesh::new(PrimitiveTopology::TriangleList,
  RenderAssetUsages::default())` (`bevy_mesh-0.19.1/src/mesh.rs:341`, пути `bevy::mesh::{Indices,
  PrimitiveTopology}`, `bevy::asset::RenderAssetUsages`), нормали (0,1,0).
  **Порядок обхода** (проверено вычислением): треугольник a=(0,0,0), b=(0,0,1), c=(1,0,0) имеет
  `(b−a)×(c−a) = (0,1,0)` (смотрит вверх), а его ориентированная площадь в (x,z) = −1. Значит многоугольник
  с положительной площадью (наша конвенция) выдаётся веером `(0, i+1, i)`. Пример: квадрат
  (0,0),(1,0),(1,1),(0,1) → треугольник (0,2,1) = (0,0),(1,1),(1,0), площадь −1 → нормаль +Y ✓.
  Деревья в парках и прочие пропы — T3 (Kenney-пропы); здесь парк = газон.

### 2.6 Зависимости и lock (проверено пробой)

`crates/citygen/Cargo.toml`:
```toml
[dependencies]
glam = { version = "=0.32.1", default-features = false, features = ["std"] }
rand_chacha = { version = "=0.10.0", default-features = false }
serde = { workspace = true }

[dev-dependencies]
ron = { workspace = true }
```
`crates/gta_sim/Cargo.toml`: `citygen = { path = "../citygen" }` в `[dependencies]`.
Клиенту новая зависимость не нужна (типы citygen реэкспортирует `gta_sim::world`).

`rand_chacha 0.10.0` не было в реестре — скачан пробой (`scratch/probe_rng`, `cargo fetch`), теперь в
`~/.cargo/registry/src/.../rand_chacha-0.10.0`. Его зависимости: `ppv-lite86 ^0.2.14` (0.2.21 в реестре),
`rand_core ^0.10.0` (0.10.1 уже в lock) — crates.io API. **Offline-резолв не работает**: `cargo tree
--offline` с новыми зависимостями падает на `nix` (офлайн-резолвер берёт скачанный 0.31.2, `gilrs-core`
просит ^0.31.3; см. `log.jsonl` dead_end). Поэтому lock разрешён онлайн в копии workspace
(`scratch/ws_probe/`): дифф — 4 изменения (зависимости `citygen`, `citygen` в `gta_sim`, новые пакеты `ppv-lite86 0.2.21`,
`rand_chacha 0.10.0`), `scratch/cargo_lock_diff.txt`, готовый файл `scratch/Cargo.lock.t2`. С ним
`cargo check --offline --locked -p citygen` проходит; `cargo tree --offline --locked -p gta_sim -e normal
-i bevy_render` пуст.

---

## 3. Steps

Работать в текущем checkout, отдельный `target/`, `git worktree`, `cargo update` запрещены.
Проверки шага — после стрелки.

### Шаг 1. Зависимости и lock
Файлы: `crates/citygen/Cargo.toml`, `crates/gta_sim/Cargo.toml`, `Cargo.lock`.
Добавить ровно строки из 2.6 (никаких других изменений зависимостей), затем скопировать
`maw/tasks/in_progress/TASK-003/scratch/Cargo.lock.t2` поверх `/Cargo.lock`.
→ `cargo tree --offline --locked -p citygen` резолвится; `git diff Cargo.lock` совпадает с
`scratch/cargo_lock_diff.txt`. Если набор зависимостей пришлось изменить, lock надо резолвить заново с
сетью (у codex-песочницы её нет) — остановиться и сообщить.

### Шаг 2. `citygen`: типы, параметры, RNG, геометрия
- `crates/citygen/src/lib.rs` — модули и публичный API: `CityParams` (+ вложенные), `CityLayout`,
  `generate`, `layout_hash`; `pub use glam::Vec2`.
- `src/params.rs` — `CityParams` и вложенные структуры, все `#[derive(Deserialize, Clone, Debug)]
  #[serde(deny_unknown_fields)]`; `DistrictKind` (`Deserialize` для `pois.*_districts`);
  `impl CityParams { pub fn validate(&self) -> Result<(), String>; pub fn district(&self, kind) ->
  &DistrictParams; pub fn half_carriageway(&self, class: RoadClass) -> f32; pub fn sidewalk(&self, class)
  -> f32 }` (последние два — единственный источник ширин для генератора и тестов).
- `src/layout.rs` — выходные типы: `CityLayout { seed, size, ground_size, roads: RoadGraph, districts:
  Vec<District>, blocks: Vec<Block>, lots: Vec<Lot>, buildings: Vec<Building>, gang_districts: [u32; 2],
  sidewalks: WalkGraph, lanes: LaneGraph, player_spawn: Vec2 }`; `RoadGraph { nodes: Vec<Vec2>, edges:
  Vec<RoadEdge { a, b: u32, class: RoadClass }>, center: u32 }` (удалённые рёбра в граф не попадают);
  `RoadClass { Avenue, Street, Alley }`; `District { kind, seed_point }`; `Block { district: u32,
  nodes: Vec<u32>, sides: Vec<u32>, curb: Vec<Vec2>, inner: Vec<Vec2>, is_park: bool }`;
  `Lot { block: u32, polygon: Vec<Vec2> }`; `Building { lot: u32, center: Vec2, axis: Vec2,
  half_extents: Vec2, height: f32, kind: BuildingKind }`; `BuildingKind { Generic, Hospital,
  PoliceStation, GangHq(u8) }`; `WalkGraph { nodes: Vec<Vec2>, edges: Vec<(u32, u32)> }`;
  `LaneGraph { lanes: Vec<Lane { edge: u32, from: Vec2, to: Vec2 }>, connectors: Vec<Connector {
  from: u32, to: u32, intersection: u32 }> }`. Все id — `u32`.
- `src/rng.rs` — `stream(seed, tag, cell)`, теги, `unit_f32/range_f32/range_u32/chance` (2.3).
- `src/geom.rs` — выпуклая геометрия на `Vec2`: `signed_area`, `centroid`, `is_convex(poly, eps)`,
  `inset(poly, &[f32]) -> Option<Vec<Vec2>>` (сдвиг каждой стороны внутрь по `d.perp()`, пересечение
  соседних прямых; при `|d1.perp_dot(d2)| < 1e-6` — параллельные стороны: вершина + нормаль·отступ;
  результат проверяется на выпуклость, площадь > 0 и знак ориентации), `clip_half_plane(poly, flags,
  point, normal) -> (poly, flags)`, `min_area_obb`, `contains_convex(poly, p, eps)`,
  `convex_overlap(a, b) -> f32` (SAT, глубина проникновения; касание = 0).
  Unit-тесты в `geom.rs` с числами, посчитанными вручную: квадрат 10×10 inset 1 → площадь 64; квадрат
  10×10 клип прямой x = 4 → площади 40 и 60, у новой стороны флаг false; inward-нормали квадрата
  (0,0),(1,0),(1,1),(0,1): ребро (0,0)→(1,0) → (0,1), (1,0)→(1,1) → (−1,0), (0,1)→(0,0) → (1,0);
  два квадрата с общей стороной → overlap 0; сдвинутые на 0.5 → overlap 0.5.
→ `cargo test -p citygen --lib` зелёный. Файлы < 750 строк каждый.

### Шаг 3. `citygen`: конвейер
- `src/roads.rs` — п.1-3, 5, 6 из 2.1: линии, узлы, классы, кварталы-четырёхугольники, слияния/переулки
  (паросочетание + выпуклость), `curb`/`inner`.
- `src/districts.rs` — п.4: семена, виды с гарантией ≥ 2 Residential/Industrial, район квартала.
- `src/lots.rs` — п.7-9: парки, OBB-деление с флагами уличных сторон, здания.
- `src/pois.rs` — п.10.
- `src/graphs.rs` — п.11-13: граф тротуаров, граф полос, точка спавна.
- `src/hash.rs` — `layout_hash` (FNV-1a, квант 1 мм, порядок из 2.1).
- `generate()` в `lib.rs` вызывает этапы в порядке 2.1 и собирает `CityLayout`.
- Запрет трансцендентных функций в citygen: соблюдать при написании (grep `sin(|cos(|atan|powf|exp(` в
  `crates/citygen/src` должен быть пуст — ревьюер проверяет).
→ `cargo build -p citygen`, `cargo clippy -p citygen -- -D warnings`.

### Шаг 4. `assets/world/city.ron`
Файл из 2.2 целиком. → парсится тестом Шага 5.

### Шаг 5. Гейты `citygen` (`crates/citygen/tests/`)
- `tests/common/mod.rs`: `shipped_params()` — читает `CARGO_MANIFEST_DIR/../../assets/world/city.ron`,
  `ron::from_str::<CityParams>`, `validate()`; на отсутствие файла — `panic!("GATE BROKEN: ...")`.
  `golden()` — `include_str!("../golden_hashes.txt")`, разбор построчно (`lines()` + `trim()`, пустые и
  `#` пропускаются, `seed` десятичный + `hash` hex): формат устойчив к `\r\n` (`core.autocrlf=true`).
  `SEEDS: [u64; 3] = [1, 2, 42]`.
- `tests/golden_hashes.txt` — три строки `seed hash`. Значения **выводятся прогоном**, не придумываются:
  тест при несовпадении печатает полный новый файл; первый прогон даёт значения, их вставить, повторный
  прогон зелёный. Шапка-комментарий: "меняется только вместе с намеренным изменением генератора или
  `city.ron`; читается `citygen`, `gta_sim` и `tools/qa/scenarios/t2.py`".
- `tests/golden.rs` — `golden_hashes_match`: для каждой строки `layout_hash(&generate(seed, &params))
  == hash`; `same_seed_same_hash` (два вызова); `different_seeds_differ` (1 ≠ 2 ≠ 42).
- `tests/properties.rs` — для `SEEDS` и развёртки `0..32` (одна функция на свойство, внутри цикл):
  1. `road_graph_connected`: BFS из `center` по всем рёбрам достигает всех узлов; `nodes[center] ==
     Vec2::ZERO`.
  2. `lane_graph_strongly_connected`: BFS по коннекторам и по обратным коннекторам из полосы 0
     достигает всех полос; полос > 0.
  3. `sidewalk_graph_connected`: BFS по тротуарам достигает всех узлов.
  4. `lots_do_not_overlap`: все пары лотов (AABB-префильтр, затем `convex_overlap`) — проникновение
     ≤ 1e-3 м; каждая вершина лота внутри `inner` своего квартала (eps 1e-3).
  5. `lots_face_a_street`: независимо от флагов генератора — у каждого лота есть сторона, оба конца
     которой лежат на одной стороне `inner` его квартала (расстояние точка–отрезок ≤ 1e-3), длиной
     ≥ `min_frontage` района − 1e-3; и каждая сторона `inner` параллельна своему ребру дороги на
     расстоянии `half_carriageway + sidewalk` ± 1e-2 (это связывает "сторону `inner`" с улицей).
  6. `buildings_inside_lots`: 4 угла каждого здания внутри его лота; высота > 0.
  7. `pois_exist`: ровно 1 `Hospital`; `PoliceStation` ровно `pois.police_stations`; `GangHq(0)` и
     `GangHq(1)` по одному, лежат в `gang_districts[0]` и `[1]`, районы различны и вида
     Residential/Industrial.
  8. `player_spawn_on_sidewalk`: есть не-Alley ребро, до осевой которого расстояние от спавна в
     `[half_carriageway, half_carriageway + sidewalk]`; спавн вне всех лотов.
- `tests/perf.rs` — `#[ignore] generation_under_budget`: для `SEEDS` меряет `Instant` вокруг `generate`,
  печатает время, `assert!(elapsed < Duration::from_secs(2))` на каждый seed. Запуск QA:
  `cargo test -p citygen --release --test perf -- --ignored --nocapture`.
→ `cargo test -p citygen` зелёный; flip-RED по таблице раздела 4.2.

### Шаг 6. `gta_sim/flow`
Новый `crates/gta_sim/src/flow/mod.rs`: `GameState`, `FlowPlugin { fn build: app.init_state::<GameState>() }`.
`lib.rs`: `pub mod flow;`, `FlowPlugin` первым в кортеже плагинов `compose_sim`.

### Шаг 7. `gta_sim/world` + `player`
- `world/mod.rs`: `pub enum WorldSource { TestArea, City { seed: u64 } }` (`Clone, Copy, Debug`);
  `WorldPlugin { pub source: WorldSource }`; `pub use citygen::{BuildingKind, CityLayout, CityParams,
  DistrictKind}`; `CITY_CONFIG = "world/city.ron"`. `build`: `insert_resource(PlayerSpawn(Vec3::ZERO))`,
  `register_type::<Block>()`, затем по источнику: `TestArea` → `add_systems(OnEnter(GameState::Loading),
  (spawn_test_area, finish_loading))`; `City { seed }` → `insert_resource(CitySeed(seed))`,
  `register_type::<CitySeed>()`, `register_type::<CityLayoutHash>()`,
  `configure_sets(Update, WorldSystems::Generation.run_if(in_state(GameState::Loading)))`,
  `add_systems(OnEnter(GameState::Loading), start_city_generation)`,
  `add_systems(Update, apply_city_generation.in_set(WorldSystems::Generation))`.
  `pub use test_area::spawn_test_area` становится приватным `use` (единственный внешний пользователь
  — `player`, его зависимость уходит).
- `world/test_area.rs`: только комментарий строки 5 → "Fixture level for the headless character gates."
  (старый комментарий после T2 ложен).
- Новый `world/city.rs`: `CitySeed`, `CityLayoutHash`, `City(pub CityLayout)` (`Resource`, без Reflect),
  `CityBuilding` (`Component`, без Reflect: поля — типы citygen), `CityGenTask(Task<(CityLayout, u64)>)`,
  `start_city_generation(commands, seed: Res<CitySeed>, params: Res<CityParamsRes>)` →
  `AsyncComputeTaskPool::get().spawn(async move { let layout = generate(seed, &params); let hash =
  layout_hash(&layout); (layout, hash) })`; `apply_city_generation(commands, task: ResMut<CityGenTask>,
  next: ResMut<NextState<GameState>>)` — `let Some((layout, hash)) = block_on(poll_once(&mut task.0))
  else { return };` → `PlayerSpawn(Vec3::new(sp.x, 0.0, sp.y))`, спавн земли и зданий (обычный цикл
  `commands.spawn`), `insert_resource(CityLayoutHash(hash))`, `insert_resource(City(layout))`,
  `remove_resource::<CityGenTask>()`, `next.set(GameState::Playing)`. `CityParams` хранится в ресурсе
  `CityParamsRes(pub CityParams)` (citygen без Bevy не может derive `Resource`).
  Константа `GROUND_SLAB_THICKNESS: f32 = 1.0` с комментарием (закон коллизии, не тюнинг).
- `lib.rs`: сигнатура `pub fn compose_sim(app: &mut App, root: ConfigRoot, source: WorldSource) ->
  Result<(), ConfigError>`; для `City` — `load_config::<CityParams>(&root, CITY_CONFIG)?`, затем
  `validate().map_err(|message| ConfigError { path: root.path(CITY_CONFIG), message })?`,
  `insert_resource(CityParamsRes(..))`; `WorldPlugin { source }`.
- `player/mod.rs:14`: `.add_systems(OnEnter(GameState::Playing), spawn_player)`; импорт
  `spawn_test_area` убрать.
→ `cargo build -p gta_sim`; `cargo tree -p gta_sim -e normal -i bevy_render` пуст
(`python tools/qa/tree_check.py`).

### Шаг 8. Тестовый харнесс `gta_sim`
`tests/common/mod.rs`:
- `headless_app()`: в кортеж плагинов добавить `bevy::state::app::StatesPlugin`; `compose_sim(&mut app,
  assets_root(), WorldSource::TestArea)`. Остальное без изменений.
- Новый `city_app(seed: u64) -> App`: то же, но `WorldSource::City { seed }`, затем цикл `app.update()` +
  `std::thread::sleep(1 ms)` до `*State<GameState> == Playing`, дедлайн 120 с по стене →
  `panic!("city generation did not finish in 120 s")` (это отказ кода, не харнесса, поэтому без
  "GATE BROKEN").
- `golden(seed) -> u64`: разбор `include_str!("../../../citygen/tests/golden_hashes.txt")` тем же
  построчным способом (путь относительно `crates/gta_sim/tests/common/mod.rs`).
`tests/config.rs`: + `shipped_city_config_loads_and_validates` (`load_config::<CityParams>` +
`validate()`).
→ существующие `movement`, `jump`, `terrain`, `config` зелёные без изменения своих чисел.

### Шаг 9. Рантайм-гейты города `crates/gta_sim/tests/city.rs`
1. `runtime_hash_matches_golden`: для seed 1 и 2 — `city_app(seed)`; `CityLayoutHash.0 == golden(seed)`,
   `CitySeed.0 == seed`. Фиксированная величина — golden-файл, поэтому тест ловит неверный seed,
   неверные параметры и расхождение float между сборками `-p citygen` и `-p gta_sim`.
2. `one_static_collider_per_building`: число сущностей `(With<CityBuilding>, With<Collider>,
   With<RigidBody>)` == `City.0.buildings.len()` > 0.
3. `player_spawns_on_sidewalk_and_stands`: `city_app(1)`, `settle()` (64 тика, y ≈ `float_height` ±0.05
   — стоит на земле, а не на крыше/внутри коробки); xz игрока: расстояние до осевой ближайшего не-Alley
   ребра в `[half_carriageway, half_carriageway + sidewalk]` (ширины через `CityParams`), точка вне
   следов всех зданий.
→ `cargo test -p gta_sim` зелёный.

### Шаг 10. Клиент
- `src/main.rs`: `fn parse_seed() -> Result<u64, String>` (линейный разбор `std::env::args()`:
  `--seed N`; без флага — `SystemTime::now().duration_since(UNIX_EPOCH)` nanos как u64); ошибку печатать и
  `return AppExit::error()`, как ошибки конфигов (`main.rs:28-45`); `info!("city seed {seed}")`;
  `compose_sim(&mut app, root.clone(), WorldSource::City { seed })`; `.add_plugins(menu::MenuPlugin)`.
- Новый `src/menu/mod.rs`: `MenuPlugin` → `OnEnter(GameState::Loading)` `spawn_loading_screen(commands,
  seed: Res<CitySeed>)` (2.5).
- `src/visuals/mod.rs`: `RenderConfig` + поля цветов и `surface_layer_step` (2.2), значения в
  `assets/world/render.ron`; `Startup: setup_city_palette` → ресурс `CityPalette`; observer
  `visualize_city_building(On<Add, CityBuilding>, ...)`; `Update: spawn_city_surfaces.run_if(
  resource_added::<City>)` с хелпером `fn flat_mesh(polys: impl Iterator<Item = &[Vec2]>, y: f32) ->
  Mesh` (веер `(0, i+1, i)`, 2.5). Существующий `visualize_block` не трогать. Если файл перерастает
  ~400 строк — вынести город в `src/visuals/city.rs`.
→ `cargo build`, `cargo clippy -- -D warnings`, `cargo run --features fast -- --seed 1` показывает
экран загрузки и город (самопроверка, не гейт).

### Шаг 11. QA-инфраструктура
- `tools/qa/brp.py` (класс `Game`): `resource_path(suffix)` (через `world.list_resources`, уникальный
  суффикс `::Name`, как `component_path`), `resource(suffix)` (`world.get_resources` → `["value"]`),
  `wait_resource(suffix, timeout)` (опрос раз в 0.5 с, пока ресурс не появится), `mutate_component(
  entity, component, path, value)` (`world.mutate_components`, параметры
  `bevy_remote-0.19.1/src/builtin_methods.rs:286-300`), `load_golden()` (разбор
  `crates/citygen/tests/golden_hashes.txt` тем же правилом). Хэш из BRP может прийти числом или `[n]` —
  нормализовать.
- `tools/qa/scenarios/t1.py`: `Game(features=("dev",), args=("--seed", "1"))` и
  `game.wait_resource("CityLayoutHash", 180)` вместо голого `time.sleep(2)` перед чтением `Player`
  (иначе на случайном seed и медленной генерации сценарий падает не по делу). Остальные проверки T1 не
  меняются.
- Новый `tools/qa/scenarios/t2.py` (`--out <dir>`): для seed 1 и 2 отдельным запуском игры:
  `wait_resource("CityLayoutHash", 180)`; `CitySeed == seed`; хэш == golden; скриншот со спавна
  `spawn_<seed>.png`; сущность игрока из `world.query` c `Player`; `mutate_component(player,
  <путь Position avian>, "", [0.0, 1.2, 0.0])` (`Position` — `Reflect` + `reflect(Component)`,
  `avian3d-0.7.0/src/physics_transform/transform.rs:44-48`; если BRP ждёт другую форму Vec3 — объект
  `{x,y,z}`); пауза 1.5 с; `Transform` игрока: |x|,|z| < 1.0 и 0.9 < y < 1.3 (стоит на перекрёстке
  центра, не провалился); скриншот `center_<seed>.png`; `get_diagnostics` FPS в summary; `shutdown`.
  После обоих: хэши различны. `summary.json` с хэшами, координатами, путями скриншотов.
→ QA: `python tools/qa/scenarios/t1.py --out ...` и `t2.py --out ...` проходят; мультимодальный осмотр
скриншотов; `cargo test -p citygen --release --test perf -- --ignored --nocapture` < 2 с на seed.

### Шаг 12. Финальная проверка
`cargo build`, `cargo clippy -- -D warnings`, `cargo test -p gta_sim`, `cargo test -p citygen`,
`python tools/qa/tree_check.py`. Файлы < 750 строк.

---

## 4. Проверка

### 4.1 Гейты и их класс

| Гейт | Класс | Что несёт |
|---|---|---|
| `golden.rs::golden_hashes_match` | корректность (регрессия) | тихое изменение генерации |
| `properties.rs` 1-3 | корректность | связность дорог, сильная связность полос, связность тротуаров |
| `properties.rs` 4-6 | корректность | непересечение и выход лотов к улице, здания в лотах |
| `properties.rs` 7 | корректность | больница, участок(и), 2 штаба в районах банд |
| `properties.rs` 8 + `city.rs` 3 | корректность | спавн на тротуаре, игрок стоит на земле |
| `city.rs` 1 | корректность проводки | seed/params/хэш в рантайме == golden; float одинаков в двух сборках |
| `city.rs` 2 | liveness + счёт | коллайдер на каждое здание |
| `perf.rs` (`#[ignore]`) | бюджет | генерация < 2 с в release |
| `t2.py` | runtime QA | BRP видит хэш, seed, телепорт работает, скриншоты |
| owner checklist | feel/визуал | см. 4.3 |

### 4.2 Flip-RED (каждый гейт показать красным, записать в IMPL_SUMMARY, что именно испорчено)

| Гейт | Порча механизма | Ожидаемо |
|---|---|---|
| golden | `grid.node_jitter` 6.0 → 6.5 в `city.ron` | RED, печатает новые хэши; вернуть |
| road connected | off-by-one: не строить рёбра последней z-линии | RED (узлы периметра изолированы) |
| lanes strongly connected | строить полосы только в направлении a→b | RED |
| sidewalks connected | не строить "зебры" | RED |
| lots overlap | в `clip` второму ребёнку отдавать исходный многоугольник | RED |
| lots face street | убрать проверку frontage при принятии разреза | RED (внутренние лоты глубоких кварталов) |
| buildings inside | не сжимать прямоугольник (0 шагов подгонки) | RED на непрямоугольных лотах |
| pois | пропустить размещение участка | RED |
| runtime golden | в `WorldPlugin` передать `seed ^ 1` | RED |
| collider count | пропускать каждое 10-е здание при спавне | RED |
| player on sidewalk | спавн = середина ребра без сдвига (на проезжей части) | RED |
| perf | временный `sleep(2.1 s)` в `generate` | RED |

### 4.3 Owner checklist (QA переносит в QA_REPORT)
- `cargo run --release -- --seed 1`: экран загрузки, затем город; бегает по тротуарам и улицам.
- Видны улицы (асфальт), тротуары, кварталы разной высоты (Downtown высокий в центре, окраины низкие),
  парки-газоны, больница/участок/штабы выделены цветом.
- `--seed 2` (и без `--seed`) даёт другой город.
- Край города: за периметром полоса земли `ground_margin` м, дальше обрыв — владелец решает, нужны ли
  стены (см. Open questions).

---

## 5. Risk areas

1. **Геометрия выпуклых многоугольников** (inset у почти-180° вершин слитых кварталов, тонкие клипы).
   Мера: параллельный случай в `inset`, проверка выпуклости/ориентации результата с откатом к парку,
   `min_area`, свойства 4-6 на 35 seed'ах. Слитые кварталы не бывают переулочными (паросочетание), поэтому
   у 180°-вершины обе стороны одного класса и одного отступа.
2. **Float-детерминизм между сборками** (glam `nostd-libm` в `gta_sim`, нет в `-p citygen`). Мера: запрет
   трансцендентных функций в citygen, квант 1 мм в хэше, рантайм-гейт сравнивает с тем же golden.
   Остаточный риск: другая платформа (не x86_64 Windows) — golden только для этой машины, это приемлемо.
3. **Golden-хэш ломается от тюнинга `city.ron`.** Это задумано (генерация изменилась). Процедура re-bless
   описана в файле и в сообщении теста; QA/owner должны знать, что правка `city.ron` = новый golden.
4. **Lock и офлайн-песочница.** Любое отклонение от набора зависимостей 2.6 → офлайн-резолв упадёт на
   `nix`. Мера: готовый `Cargo.lock.t2`, Шаг 1 требует остановиться, а не импровизировать.
5. **Старт в состоянии `Loading`.** Системы клиента на `Single<With<Player>>` пропускаются
   (проверено), но любая будущая система с `Res<City>` без гейта упадёт в `Loading`. Мера: ресурсы
   города появляются до `Playing`; новые системы гейтить набором.
6. **Кадр перехода Loading → Playing** (спавн ~1-2 тыс. коллайдеров + вставка в BVH avian на следующем
   шаге физики). Под экраном загрузки не критично; если T3/T16 увидят спайк — тайм-слайс спавна.
7. **Меш на каждое здание** (уникальный `Cuboid` на сущность, материалы общие). Для T2 приемлемо;
   слияние по чанкам — T3 (GDD §2.4, `render.ron`).
8. **`t1.py` теперь идёт по городу.** На seed 1 спавн на тротуаре N-S улицы, W ведёт вдоль тротуара;
   jitter узлов ±6 м даёт уклон улицы ≤ ~7°, за 4.5 м бега уход ≤ 0.6 м при ширине тротуара ≥ 3 м.
   Если QA увидит упор — проверить правило спавна, а не ослаблять t1.
9. **Форма Vec3 в BRP `mutate_components`** (массив или объект) не проверена исполнением — QA пробует
   обе и фиксирует рабочую в `brp.py`.
10. **Падение с края города.** Земля шире города на `ground_margin`; стен нет.

---

## 6. Open questions

Блокирующих нет. Решения, принятые планом (оркестратор может перевернуть до реализации):

1. **Край города** (владельцу, не блокирует T2):
   - A (по умолчанию). Полоса земли `ground_margin` = 100 м за периметром, дальше обрыв. Ноль кода сверх
     плиты; игрок может упасть, если долго бежит наружу.
   - B. Невидимые стены по краю земли: 4 статических коллайдера, +10 строк, высота стены в `city.ron`.
   - C. Зациклить мир / телепорт назад — вне GDD, не рекомендую.
2. **Упрощения v1 относительно текста GDD §2.2** (не механики, внутренности генератора): шаг сетки
   "ядро/окраина" вместо районной таблицы (районы Voronoi считаются после сетки); без "шума плотности";
   переулки — рёбра сетки, а не проезды внутри квартала; одна зона конфликта на перекрёсток; деревья в
   парках — T3. Каждое можно добавить позже без смены интерфейса `CityLayout`.
3. **Строка экрана загрузки** латиницей в коде до T5 (нет кириллического шрифта, `strings.ron` вводит T5).

children: 0 launched / 0 reported.
