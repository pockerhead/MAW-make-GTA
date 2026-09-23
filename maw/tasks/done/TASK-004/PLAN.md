# PLAN — TASK-004 (GDD T3): облик города

Источник объёма: `docs/design/GDD.md` §13 T3 + §2.2 (шаги 5-6), §2.4, §2.6, §9.2, §10.5, §11, §12 и
`TASK_FINAL.md` (Resolved questions: гейт слияния считает `Mesh3d`, коллайдеры остаются по одному на
здание). Все API Bevy 0.19.1 / avian3d 0.7.0 / parry3d 0.27.0 / bevy_gltf / bevy_pbr / bevy_light /
bevy_camera ниже сверены по `~/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/` (версии из
`Cargo.lock`) и по примерам тега `v0.19.1` (скачаны в `scratch/bevy_examples/`). Пакеты Kenney скачаны,
разобраны и захэшированы планировщиком (`scratch/kenney/`, `scratch/third_party/`,
`scratch/kenney/INVENTORY.txt`).

**Цена ошибки.** Молча ломаются и получают исполняемые гейты: (1) слияние мешей (забытое слияние бьёт
по бюджету кадра, владелец увидит только падение FPS), (2) воспроизводимость ассетов (SHA-256, лицензия,
состав), (3) детерминизм генератора (golden-хэш меняется намеренно, ре-bless по процедуре), (4) коллайдеры
зданий с уступами и приподнятых кварталов (игрок, камера, позже пули и машины). `VisibilityRange` у
пропов — тоже бюджет кадра, дешёвая проверка в том же гейте. Вид города, разметка, окна, небо, туман,
расстановка и ориентация пропов, различимость районов владелец видит на первом кадре: это его прогон и
скриншоты QA, машинерию под это не строим.

---

## 1. Understanding (как устроено сейчас)

**citygen** (чистая генерация, без Bevy):
- `crates/citygen/src/lib.rs:23-51` — конвейер `grid → roads → districts → lots(+parks) → pois → graphs`.
- `crates/citygen/src/layout.rs:4-72` — `CityLayout`, `Block { curb, inner, is_park, sides, nodes }`,
  `Building { lot, center, axis, half_extents, height, kind }`, `BuildingKind { Generic, Hospital,
  PoliceStation, GangHq(u8) }`. Ориентиров (площадь, башня, центральный парк) и уступов нет.
- `crates/citygen/src/lots.rs:21-60` — `build`: `mark_parks` (`:63-81`, самый младший застраиваемый квартал
  района защищён от парка), рекурсивное OBB-деление, `fit_building` (`:142-191`: ось `u` вдоль самого
  длинного фронтажа, `v = u.perp()` смотрит внутрь лота, сжатие ×0.9 до 24 шагов, высота
  `floors × floor_height`).
- `crates/citygen/src/pois.rs:5-75` — больница/участок/штабы выбираются только среди `Generic`.
- `crates/citygen/src/geom.rs:30-60` — `inset` сохраняет число вершин и индексацию сторон (вершина `i` —
  начало стороны `i`), внутренняя нормаль стороны `a→b` = `(b-a).perp()` (CCW-полигоны в (x, y)).
- `crates/citygen/src/hash.rs:4,72-147` — `HASH_SCHEMA_VERSION = 1`, канонический FNV-1a.
- `crates/citygen/src/params.rs:4-246` — `CityParams` (`deny_unknown_fields`) + `validate()`.
- Гейты: `tests/golden.rs` + `tests/golden_hashes.txt` (seed 1, 2, 42; bless-процедура в шапке файла),
  `tests/properties.rs` (35 seed), `tests/perf.rs` (`#[ignore]`).
- Замер планировщика (`scratch/probe_stats`, seed 1/2/42): 135-140 кварталов, ~1300 зданий, 50-68 зданий
  выше 45 м, макс. высота 117 м (30 × 3.9), 4 ближайших к центру квартала на 58-65 м с `inner` 3770-4700 м²,
  в радиусе 250 м изредка суперквартал ~11-12 тыс. м², пустых лотов в Industrial 0-2 (под "двор
  контейнеров" не годятся), длина не-переулочных улиц ~29 км.

**gta_sim** (геймплей, headless):
- `crates/gta_sim/src/world/city.rs:62-88` — `apply_city_generation`: `PlayerSpawn(x, 0, z)`, земля
  (плита 1 м, верх в y=0) и 4 стены края (`:90-113`), по сущности `CityBuilding` с одним
  `Collider::cuboid` на здание (`:115-138`), ресурсы `City`, `CityLayoutHash`.
- `crates/gta_sim/src/world/mod.rs:23-24` — `PlayerSpawn`; `:44-71` — `WorldPlugin` регистрирует Reflect-типы.
- Тесты: `crates/gta_sim/tests/city.rs:39-77` (`one_static_collider_per_building` требует cuboid с
  половинами = футпринт × высота/2), `tests/common/mod.rs:118-126` (`settle` ждёт `y ≈ float_height`
  абсолютно), `:46-61` (`city_app`), `:18-24` (`assets_root`).

**Клиент** (`src/`, бинарник `gta_like`):
- `src/visuals/city.rs:54-77` — наблюдатель `On<Add, CityBuilding>` вешает **по `Mesh3d` на каждое
  здание** (это и есть премиса задачи); `:79-110` — три плоских меша (земля, тротуары = весь `curb`,
  газоны). Лоты залиты цветом тротуара (находка QA TASK-003 для T3).
- `src/visuals/mod.rs:9-35` — `RenderConfig` (`deny_unknown_fields`, поля приватные), `:55-69` — солнце с
  тенями без настройки каскадов (дефолт `maximum_distance = 150`, `bevy_light-0.19.1/src/cascade.rs:135-155`).
- `src/camera/mod.rs:35-50` — камера `Camera3d` + `PerspectiveProjection { fov, ..default() }` (far = 1000,
  `bevy_camera-0.19.1/src/projection.rs:422`). Тумана и неба нет.
- `src/main.rs:63-85` — грузит `camera.ron`, `render.ron` строгим загрузчиком.

**QA/инфраструктура:** `tools/qa/brp.py` (сборка только debug, `:36-43`), `tools/qa/scenarios/t2.py`
(телепорт через `Position`, путь `""`, значение `[x, y, z]`). `tools/fetch_assets` и
`assets/third_party/` нет. **Конфликт с GDD §9.2:** корневой `.gitignore` игнорирует `*.glb`, `*.png`, а
`.gitattributes` (`* text=auto eol=lf`) нормализует `License.txt` Kenney (CRLF + CP1252) — хэш из манифеста
разойдётся на чужом клоне (проба `scratch/gitprobe/`).

**Kenney (проверено скачиванием, 2026-09-23):**

| Пакет | URL (со страницы `https://kenney.nl/assets/<pack>`) | SHA-256 zip |
|---|---|---|
| city-kit-roads 2.1 | `https://kenney.nl/media/pages/assets/city-kit-roads/74288c9459-1787042796/kenney_city-kit-roads.zip` | `22058af3d68173a7cf9bda9f0e243a8cef6bd68168c302ebc76327063849674e` |
| city-kit-suburban 2.0 | `https://kenney.nl/media/pages/assets/city-kit-suburban/2c871b7af2-1745479373/kenney_city-kit-suburban_20.zip` | `5869c35cf30b1c87bdb2d197b6d325eebadd2ef08ea27f04797e8e08d77a9a39` |
| city-kit-industrial 2.0 | `https://kenney.nl/media/pages/assets/city-kit-industrial/0ec35b139d-1788171848/kenney_city-kit-industrial_2.0.zip` | `5b381164e5760f3830a2dbee43b972deee38b2a695d091b56e238ab2910c96d2` |

`License.txt` каждого пакета: "License: (Creative Commons Zero, CC0)". GLB ссылаются на **внешнюю**
текстуру `Textures/colormap.png` (uri в JSON GLB), значит она коммитится рядом. Выбранные модели — один
меш, один примитив, материал `colormap`, без трансформа узла, кроме контейнеров (узел scale 0.27, мы берём
сырой примитив и учитываем это в своём `scale`). Сырые габариты (м, до масштаба): `light-square` высота
0.600, плечо лампы уходит в **−Z** (z от −0.213 до 0.025); `traffic-light` 0.515; `tree-large` 0.767,
`tree-small` 0.567; `shipping-container-a/c` 1.38 × 1.289 × 3.046 (длинная ось **Z**). Отброшены:
`dumpster` (3 меша, крышки отдельными узлами), `shipping-container-b` (2 примитива), `detail-tank`
(зеркальный scale узла). City Kit Commercial не нужен (готовые здания Kenney не входят в цель T3, см. Open
questions).

---

## 2. Approach

**2.1 Ориентиры и уступы — данные генератора (citygen), не декор.** Площадь = застраиваемый квартал,
ближайший к центру (с не-переулочной стороной длиной ≥ `min_frontage`); на ней один лот (весь `inner`) и
одно здание `BuildingKind::Tower` квадратного футпринта `landmarks.tower_footprint` высотой
`landmarks.tower_floors × downtown.floor_height`; `validate()` гарантирует, что это выше максимума любого
района, значит башня — самая высокая по построению. Центральный парк = квартал с наибольшей площадью
`inner` в радиусе `landmarks.park_radius` от центра, кроме площади и защищённых кварталов районов.
Уступы: здания с `floors ≥ massing.setback_min_floors` получают `upper_tiers` (каждые
`setback_tier_floors` этажей отступ `setback_inset` со всех сторон, пока полуразмер ≥ `setback_min_half`).
Почему в layout: коллайдер обязан совпадать с видом (камера кастует сферу, T6 стреляет лучами), а layout —
единственный источник геометрии. Коллайдер остаётся **один на здание**: `Collider::compound` из ярусов
(`avian3d-0.7.0/src/collision/collider/parry/mod.rs:698`). Отвергнуто: визуальные уступы поверх полного
кубоида (невидимые стены у башен).

**2.2 Бордюр — физический.** Каждый квартал с `curb` получает один статический
`Collider::convex_hull` (`parry/mod.rs:985`, возвращает `Option`) — призму `curb`-полигона от
y = −1 м (низ плиты земли) до `roads.curb_height` (0.15 м, типичная высота бордюра). Тротуары, лоты, парки
и площадь стоят на ней. Точка спавна игрока поднимается на `curb_height`. Tnua берёт ступень 0.2 м
(`tests/terrain.rs:19-28`), значит 0.15 м проходима. Отвергнуто: визуальный подъём без коллайдера
(ноги капсулы тонут на 15 см, будущие машины не чувствуют бордюр).

**2.3 Слияние по чанкам — только в клиенте, асинхронно.** Сетка чанков покрывает город:
`n = ceil(size / chunk_size)` по каждой оси, `chunk_size` из `render.ron`. **Каждый** чанк содержит свой
прямоугольник асфальта (крайние чанки дотянуты до края земли `±ground_size/2`), поэтому все `n²` чанков
непусты по построению и отдельной сущности земли нет. Квартальные поверхности (призма бордюра, кольцо
тротуара, заливка `inner`), здания (стены с фасадными UV, крыши ярусов) и разметка попадают в чанк по
якорю: центроид `curb`, центр здания, центр штриха. Один `Mesh` + один общий материал
`FacadeMaterial = ExtendedMaterial<StandardMaterial, FacadeExtension>` на чанк; цвета — вершинные
(`ATTRIBUTE_COLOR`, линейные), окна — UV0 в "пролётах/этажах", у не-фасадов UV0 = (−1, −1). Сборка мешей
идёт в `AsyncComputeTaskPool` (маркерный паттерн: задача → `block_on(poll_once)` → спавн), как требует
инвариант "procgen вне кадра"; вход — клон `CityLayout` + `CityParams` + `RenderConfig`. Пер-зданийный
`Mesh3d` удаляется. Сравнение: GPU-driven рендер Bevy 0.16+ удешевил draw calls
(https://bevy.org/news/bevy-0-16/), но GDD §2.4 требует чанки ради будущей активации/деактивации; цена
крупного чанка — худший frustum culling ("any part of it in view draws all of it",
https://github.com/jjgroenendijk/sunset-driver/issues/160), поэтому размер — данные.

**2.4 Гейт слияния — headless `App` в клиентском крейте** (`cargo test -p gta_like`), собранный из
production-плагина `CityVisualsPlugin` + `compose_sim` + `init_asset` вместо рендер-плагинов. В `gta_sim`
он невозможен: там нет и не должно быть `Mesh3d`. Ожидаемое число чанков тест считает сам из
`CityParams.size` и `RenderConfig.chunk_size` (не вызывая функцию реализации — иначе тавтология).

**2.5 Фасады — шейдер окон по UV "пролёт × этаж"** (world-space/procedural UV + `fract`, классический
приём: https://www.blog.radiator.debacle.us/2012/01/joys-of-using-world-space-procedural.html ,
https://techartaid.com/posts/procedural-building-random/). CPU пишет `u = t · bays`
(`bays = max(1, round(длина_стены / bay_width))`, окна ровно укладываются в стену), `v = y / floor_height`
района (башня — downtown), так что окна совпадают с реальными этажами и ярусами (низ яруса кратен этажу).
Шейдер: `pbr_input_from_standard_material` → внутри ячейки окна подмена `base_color` на стекло и
`perceptual_roughness` → `apply_pbr_lighting` → `main_pass_post_lighting_processing` (туман применяется).
Шаблон — `scratch/bevy_examples/extended_material.rs|.wgsl` (тег v0.19.1). Uniform из двух `Vec4`
(32 байта, выравнивание 16 без webgl-паддинга).

**2.6 Пропы Kenney — отдельные сущности с общими меш/материалом** (батчинг GPU-driven, GDD §2.4), каждая с
`VisibilityRange { start_margin: 0..0, end_margin: (range − fade)..range }`
(`bevy_camera-0.19.1/src/visibility/range.rs:80`; компонент не наследуется детьми, поэтому никаких
`WorldAssetRoot`-сцен — берём примитив напрямую `GltfAssetLabel::Primitive { mesh: 0, primitive: 0 }`,
`bevy_gltf-0.19.1/src/label.rs:41,112`). Материал — один `StandardMaterial` на текстуру пакета
(`base_color_texture = load("third_party/<pack>/Textures/colormap.png")`), всего 3. Расстановка — чистая
функция клиента от layout (пропы в T3 без физики). Отвергнуто: `SceneRoot`/`WorldAssetRoot` (в 0.19
сцены glTF спавнятся через `WorldAssetRoot`, `scratch/bevy_examples/atmospheric_fog.rs`) — иерархия детей
без `VisibilityRange`.

**2.7 Небо, солнце, туман.** `DistanceFog { falloff: FogFalloff::Linear { start, end } }`
(`bevy_pbr-0.19.1/src/fog.rs:55`) на камеру через наблюдатель `On<Add, Camera3d>`; `ClearColor` = цвет
тумана = цвет горизонта; небо — unlit-сфера с вершинным градиентом горизонт→зенит, `cull_mode: None`,
`fog_enabled: false`, `NotShadowCaster`, следует за камерой. Солнце — существующий `DirectionalLight` +
`CascadeShadowConfigBuilder` (`bevy_light-0.19.1/src/cascade.rs:59`) и ресурс
`DirectionalLightShadowMap { size }` (`directional_light.rs:193`). Отвергнуто: процедурная `Atmosphere`
(`bevy_light-0.19.1/src/atmosphere.rs:36`, `AtmosphereSettings` требует `Hdr`,
`bevy_pbr-0.19.1/src/atmosphere/mod.rs:296`; пример ставит `lux::RAW_SUNLIGHT` + `Exposure`) — перетюнинг
всего света и непроверенная связка с `DistanceFog`; вернуться можно вместе с рычагом "время суток".

**2.8 Ассеты: `tools/fetch_assets.py` + `assets/third_party/manifest.ron` + гейт в `gta_sim`.** Python
stdlib (как `tools/qa/*`): читает манифест маленьким парсером подмножества RON, качает zip с ретраями (TLS
kenney.nl нестабилен: `curl` один раз упал на рукопожатии), проверяет SHA-256 архива, извлекает только
перечисленные файлы, проверяет SHA-256 каждого. Строгий валидатор — Rust-тест
`crates/gta_sim/tests/asset_manifest.rs` (`deny_unknown_fields`, SHA-256 через `sha2 = "=0.10.9"`
dev-dependency, лицензия, точный состав каталога). Распакованные файлы коммитятся (GDD §9.2), zip — нет.
Отвергнуто: Rust-утилита загрузки (крейты zip + HTTP, офлайн-резолв для имплементера).

---

## 3. Steps

Порядок = порядок зависимостей. Каждый шаг заканчивается проверкой.

### Шаг 1. Git-правила для ассетов (`.gitignore`, `.gitattributes`)

- `.gitignore`: в конец (после блока бинарных паттернов) добавить
  ```
  # Committed CC0 third-party packs (GDD §9.2)
  !/assets/third_party/**
  ```
- `.gitattributes`: добавить строку `assets/third_party/*/** binary` (только каталоги пакетов; сам
  `manifest.ron` остаётся текстом).
- Проверка: `git check-ignore -v assets/third_party/city-kit-roads/light-square.glb` пусто;
  после шага 2 `git ls-files --eol assets/third_party/city-kit-roads/License.txt` показывает
  `i/crlf ... attr/-text` (проба планировщика: `scratch/gitprobe/`).

### Шаг 2. Манифест и загрузчик

**2a. `assets/third_party/manifest.ron`** (новый). Схема и значения (все хэши вычислены планировщиком,
`scratch/kenney/INVENTORY.txt`):
```ron
// Third-party assets: fetched by tools/fetch_assets.py, validated by crates/gta_sim/tests/asset_manifest.rs.
(
    packs: [
        (
            name: "city-kit-roads",
            version: "2.1",
            page: "https://kenney.nl/assets/city-kit-roads",
            url: "https://kenney.nl/media/pages/assets/city-kit-roads/74288c9459-1787042796/kenney_city-kit-roads.zip",
            archive_sha256: "22058af3d68173a7cf9bda9f0e243a8cef6bd68168c302ebc76327063849674e",
            license: CC0,
            license_file: "License.txt",
            files: [
                (archive: "Models/GLB format/light-square.glb", path: "light-square.glb", sha256: "6230d136c8883d7f9bc1b86f0309077e452e26e5e1e587ef1b77abef3c5f047c"),
                (archive: "Models/GLB format/traffic-light.glb", path: "traffic-light.glb", sha256: "6c9ee253b7370e1043168493b2e662b349484a0cdd02100cff04d9d8d1bd9abb"),
                (archive: "Models/GLB format/Textures/colormap.png", path: "Textures/colormap.png", sha256: "a48e2962661ae44368ffddf3054f80ed8491d082b757dcc3011184cd9a98b185"),
                (archive: "License.txt", path: "License.txt", sha256: "87b9404aef5dff6162ae1e53a27bc6427fe1cdd2e2dd5ec05fdd25f17da10498"),
            ],
        ),
        (
            name: "city-kit-suburban",
            version: "2.0",
            page: "https://kenney.nl/assets/city-kit-suburban",
            url: "https://kenney.nl/media/pages/assets/city-kit-suburban/2c871b7af2-1745479373/kenney_city-kit-suburban_20.zip",
            archive_sha256: "5869c35cf30b1c87bdb2d197b6d325eebadd2ef08ea27f04797e8e08d77a9a39",
            license: CC0,
            license_file: "License.txt",
            files: [
                (archive: "Models/GLB format/tree-large.glb", path: "tree-large.glb", sha256: "16d1f95c149bc727a953a473cf20bf21de9bf1a88747c4f9d2eeb4ac7d43e291"),
                (archive: "Models/GLB format/tree-small.glb", path: "tree-small.glb", sha256: "5f63359e5f392609d7617cc98070855ca1ffd4f4b4bc3978a5ff56ece88a58d6"),
                (archive: "Models/GLB format/Textures/colormap.png", path: "Textures/colormap.png", sha256: "9b5de86078c25ef02351a80d35ff3c978693a1044565b73eedd9ae9b5b80665d"),
                (archive: "License.txt", path: "License.txt", sha256: "d778fa769a89a7d9f167bc337fdf8fc2144e47d9abb0c35eb6ad897c0ee24d0a"),
            ],
        ),
        (
            name: "city-kit-industrial",
            version: "2.0",
            page: "https://kenney.nl/assets/city-kit-industrial",
            url: "https://kenney.nl/media/pages/assets/city-kit-industrial/0ec35b139d-1788171848/kenney_city-kit-industrial_2.0.zip",
            archive_sha256: "5b381164e5760f3830a2dbee43b972deee38b2a695d091b56e238ab2910c96d2",
            license: CC0,
            license_file: "License.txt",
            files: [
                (archive: "Models/GLB format/shipping-container-a.glb", path: "shipping-container-a.glb", sha256: "7b2d5ca874c8e659ee6cfd1614f0470b70410b2ddc7620ac83b8954dedeb861d"),
                (archive: "Models/GLB format/shipping-container-c.glb", path: "shipping-container-c.glb", sha256: "6a272420c37ccf4b354565d75c5d1fd889cd568297c215c681fcf11ef8c8ae13"),
                (archive: "Models/GLB format/Textures/colormap.png", path: "Textures/colormap.png", sha256: "950f4f891ebd05a2affac810e6eeb0fea1511bc39039b65a3cbf2e17d17bc6a2"),
                (archive: "License.txt", path: "License.txt", sha256: "60a8c5c31191256ec9779dc18745dea6d69c46f4b29703459ce064c1765a59ea"),
            ],
        ),
    ],
)
```

**2b. `tools/fetch_assets.py`** (новый, stdlib: `argparse`, `hashlib`, `urllib.request`, `zipfile`,
`pathlib`, `time`):
- `parse_ron(text)`: рекурсивный спуск по подмножеству RON этого файла — `(ident: value, ...)` → dict,
  `[...]` → list, `"..."` (с `\"`, `\\`) → str, голый идентификатор (`CC0`) → str, `//`-комментарии,
  висячие запятые. Ничего больше (любой другой символ → `SystemExit` с номером строки).
- Для каждого пакета: если все `files` уже лежат в `assets/third_party/<name>/<path>` с верным SHA-256 —
  пропуск. Иначе zip ищется в `--cache DIR` (повторяемый флаг) и в `target/asset-cache/`, иначе качается
  `urllib` с заголовком `User-Agent`, 4 попытки с паузой 2/4/8 с, в `target/asset-cache/<basename url>`.
  SHA-256 архива ≠ `archive_sha256` → ошибка с обоими значениями. Затем извлечь каждый `archive` →
  `path` байтами (`zf.read`), сверить `sha256`, записать.
- `--check`: без сети, только сверка файлов; код выхода 1 при любом расхождении.
- Имплементер без сети запускает `python tools/fetch_assets.py --cache maw/tasks/in_progress/TASK-004/scratch/kenney`
  (там три нужных zip; четвёртый, commercial, игнорируется). Проверка: `python tools/fetch_assets.py --check`
  → код 0; в `assets/third_party/` ровно 12 файлов.

**2c. `crates/gta_sim/Cargo.toml`**: в `[dev-dependencies]` добавить `sha2 = "=0.10.9"`. **`Cargo.lock`**:
заменить файлом `maw/tasks/in_progress/TASK-004/scratch/Cargo.lock.task004` (резолв онлайн в копии
workspace от текущего HEAD; добавляет ровно 7 пакетов: sha2 0.10.9, digest 0.10.7, block-buffer 0.10.4,
crypto-common 0.1.7, generic-array 0.14.7, typenum 1.20.1, cpufeatures 0.2.17; существующие версии не
тронуты, дифф `scratch/cargo_lock_diff.txt`; все исходники уже в `~/.cargo/registry`). Проверка:
`cargo tree -p gta_sim -e dev --offline | grep sha2`; `python tools/qa/tree_check.py` зелёный
(`sha2` не тянет `bevy_render`).

**2d. `crates/gta_sim/tests/asset_manifest.rs`** (новый гейт, класс: корректность воспроизводимости):
- Локальные типы `Manifest { packs: Vec<Pack> }`, `Pack { name, version, page, url, archive_sha256,
  license: License, license_file, files: Vec<PackFile> }`, `enum License { CC0 }`,
  `PackFile { archive, path, sha256 }`, все `#[serde(deny_unknown_fields)]`; чтение через
  `common::assets_root()` + `load_config::<Manifest>(&root, "third_party/manifest.ron")` (ошибка чтения →
  `GATE BROKEN: ...`).
- Тест `manifest_matches_committed_assets`, для каждого пакета: имя уникально и из `[a-z0-9-]`; `url`
  начинается с `https://kenney.nl/media/pages/assets/<name>/` и кончается `.zip`; `archive_sha256` и
  все `sha256` — 64 символа `[0-9a-f]`; каждый файл существует и `format!("{:x}", Sha256::digest(bytes))`
  равен манифесту (сообщение: пакет, путь, ожидаемый и фактический хэш); `license_file` есть в `files`,
  и для `License::CC0` его **байты** (не `read_to_string`: файл CP1252) содержат
  `b"Creative Commons Zero"` и `b"CC0"`; множество файлов на диске под
  `assets/third_party/<name>/` (рекурсивно, пути через `/`) **равно** множеству `files.path`; под
  `assets/third_party/` нет каталогов, которых нет в `packs`.
- Проверка: `cargo test -p gta_sim --test asset_manifest` зелёный; flip-RED в §4.2.

### Шаг 3. citygen: параметры (`crates/citygen/src/params.rs`, `assets/world/city.ron`)

- `RoadParams` + `pub curb_height: f32` (validate: `positive`).
- `CityParams` + `pub massing: MassingParams` и `pub landmarks: LandmarkParams`:
  ```rust
  pub struct MassingParams { pub setback_min_floors: u32, pub setback_tier_floors: u32,
                             pub setback_inset: f32, pub setback_min_half: f32 }
  pub struct LandmarkParams { pub tower_floors: u32, pub tower_footprint: f32, pub park_radius: f32 }
  ```
  (`Deserialize, Clone, Debug`, `deny_unknown_fields`). `validate()`: `setback_min_floors ≥ 1`,
  `setback_tier_floors ≥ 1`, `positive(setback_inset)`, `positive(setback_min_half)`,
  `positive(tower_footprint)`, `positive(park_radius)`, и
  `tower_floors × downtown.floor_height > max_k(floors_k.1 × floor_height_k)` по 4 районам
  (сообщение `"landmarks.tower_floors must make the tower the tallest building"`).
- `assets/world/city.ron`: `roads: (..., curb_height: 0.15)`, в конец
  `massing: (setback_min_floors: 12, setback_tier_floors: 8, setback_inset: 3.0, setback_min_half: 6.0),`
  `landmarks: (tower_floors: 40, tower_footprint: 32.0, park_radius: 300.0),`
  (40 × 3.9 = 156 м > 30 × 3.9 = 117 м).
- `params.rs` тест `validate_rejects_bad_params`: + случай `tower_floors = 30` → ошибка содержит
  `landmarks.tower_floors`.
- Проверка: `cargo test -p citygen --lib`, `cargo test -p gta_sim --test config`.

### Шаг 4. citygen: layout, ориентиры, уступы, хэш

**4a. `crates/citygen/src/layout.rs`:**
- `BuildingKind` + `Tower`.
- `Building` + `pub upper_tiers: Vec<Tier>`; `pub struct Tier { pub bottom: f32, pub half_extents: Vec2 }`
  (`Clone, Copy, Debug`). Смысл: `half_extents`/`height` остаются базовым футпринтом и полной высотой;
  ярус `k` занимает `[upper_tiers[k].bottom, upper_tiers[k+1].bottom или height)`, база —
  `[0, upper_tiers[0].bottom или height)`.
- `CityLayout` + `pub landmarks: Landmarks { pub plaza: u32 /*block*/, pub park: u32 /*block*/,
  pub tower: u32 /*building*/ }`.
- `GenError` + `NoLandmarkCandidate { landmark: &'static str }` (+ `Display`).

**4b. `crates/citygen/src/lots.rs`:**
- Вынести из `mark_parks` правило защиты в `pub(crate) fn protected_blocks(blocks, district_count) ->
  Vec<Option<usize>>` (поведение не меняется); `mark_parks` использует его.
- `build` меняет сигнатуру на `-> Result<(Vec<Lot>, Vec<Building>, Landmarks), GenError>`: после
  `mark_parks` вызвать `landmarks::choose(...)`, выставить `blocks[park].is_park = true`,
  `blocks[plaza].is_park = false`. В цикле по кварталам квартал `plaza` не делится: один лот
  `inner.clone()` (фронтаж как у корня) и `fit_tower`; индекс здания сохранить в `Landmarks.tower`.
- Вынести из `fit_building` цикл сжатия в `fn shrink_to_fit(shrunk: &[Vec2], center, u, half) ->
  Option<Vec2>` (те же `FIT_SHRINK`/`FIT_STEPS`/`FIT_EPS`). `fit_tower`: `u` от `longest_frontage`, полигон
  `inset` на downtown `setback`, центр = центроид, `half = Vec2::splat(tower_footprint / 2)` →
  `shrink_to_fit` (None → `NoLandmarkCandidate { landmark: "tower" }`), `height = tower_floors ×
  downtown.floor_height`, `kind: Tower`, `upper_tiers = tiers(...)` с `floors = tower_floors`.
- В `fit_building` после расчёта `floors`: `upper_tiers = tiers(&params.massing, floors,
  district.floor_height, half)`.
- `fn tiers(m: &MassingParams, floors: u32, floor_h: f32, base: Vec2) -> Vec<Tier>`:
  если `floors < m.setback_min_floors` → пусто; иначе для `k = 1..` пока `k·T < floors`:
  `next = prev − Vec2::splat(inset)`; если `next.x < min_half || next.y < min_half` → стоп; иначе push
  `Tier { bottom: (k·T) as f32 × floor_h, half_extents: next }`.
  **Рабочий пример 1** (downtown, 30 этажей, 3.9 м, база (15, 12), T=8, inset 3, min 6): k=1 → (12, 9),
  низ 31.2; k=2 → (9, 6), низ 62.4 (6 ≥ 6); k=3 → (6, 3) — 3 < 6, стоп. Итог 2 яруса, верх 117.
  **Пример 2** (башня 40 эт., база (16, 16)): (13,13)@31.2, (10,10)@62.4, (7,7)@93.6; k=4 → (4,4) стоп;
  крыша 14 × 14 м на 156 м. **Пример 3** (commercial 8 эт.): 8 < 12 → ярусов нет.
- Pipeline `lib.rs`: `let (lots, mut buildings, landmarks) = lots::build(...)?;`, `landmarks` в
  `CityLayout`.

**4c. `crates/citygen/src/landmarks.rs`** (новый, `mod landmarks;` в `lib.rs`):
`pub(crate) fn choose(params: &CityParams, districts: &[District], roads: &RoadGraph, blocks: &[Block],
protected: &[Option<usize>]) -> Result<(u32, u32), GenError>`:
- площадь: среди кварталов с непустым `inner`, у которых есть сторона `k` с классом ≠ Alley и длиной
  `inner[k]→inner[k+1]` ≥ `min_frontage` района квартала, — минимальное `|centroid(inner)|`, тай-брейк
  по меньшему id; нет → `NoLandmarkCandidate { landmark: "plaza" }`;
- парк: среди кварталов с непустым `inner`, `id ≠ plaza`, `protected[district] != Some(id)`,
  `|centroid(inner)| ≤ park_radius` — максимальная `signed_area(inner)`, тай-брейк меньший id; нет →
  `NoLandmarkCandidate { landmark: "park" }`.
Площадь в центральном районе (Downtown по построению `districts.rs:16-40`), POI выбираются только из
`Generic` (`pois.rs:56`), башня туда не попадает.

**4d. `crates/citygen/src/hash.rs`:** `HASH_SCHEMA_VERSION = 2`; `building_kind(Tower) = (4, 0)`; у
каждого здания после `height`: `len(upper_tiers)` + для каждого `f32(bottom)`, `vec2(half_extents)`;
после `gang_districts`: `u32(plaza)`, `u32(park)`, `u32(tower)`.

**4e. `crates/citygen/tests/properties.rs`** (новые гейты, класс: корректность):
- `landmarks_exist`: для всех 35 layout — `buildings[tower].kind == Tower` и это единственный `Tower`;
  `buildings[tower].height` строго больше любой другой высоты; `lots` квартала `plaza` — ровно один, и на
  нём ровно одно здание (`tower`); `blocks[park].is_park`, `park != plaza`,
  `|centroid(blocks[park].inner)| ≤ params.landmarks.park_radius`.
- `setback_tiers_nested`: для каждого здания — `bottom` строго растут, кратны `floor_height` района
  (для башни — downtown) с допуском 1e-3, `< height`; `half_extents[k] == half_prev − inset`
  (допуск 1e-4) и `≥ setback_min_half`; здания с `floors < min_floors` (height/floor_h округлённо) без
  ярусов; хотя бы у одного здания на seed 1 ярусы есть (по замеру 55 зданий выше 45 м).
- Существующий `buildings_inside_lots` не меняется (базовый футпринт).
- **Ре-bless golden** по процедуре из шапки `golden_hashes.txt`: сначала все properties зелёные, затем
  `cargo test -p citygen --test golden -- --ignored bless_print_golden --nocapture`, вставить 3 строки
  руками. Проверка: `cargo test -p citygen` зелёный целиком; `cargo test -p citygen --release --test perf
  -- --ignored --nocapture` < 2 с.

### Шаг 5. gta_sim: коллайдеры, спавн, ресурс ориентиров

`crates/gta_sim/src/world/city.rs`:
- Новый маркер `#[derive(Component)] pub struct CityBlock;` и ресурс
  ```rust
  /// Landmark points of the generated city; read by QA over BRP.
  #[derive(Resource, Reflect)] #[reflect(Resource)]
  pub struct CityLandmarks { pub tower_roof: Vec3, pub plaza_center: Vec3, pub park_center: Vec3 }
  ```
- `apply_city_generation`: `let curb = params.0.roads.curb_height;`
  `PlayerSpawn(Vec3::new(spawn.x, curb, spawn.y))`; вызвать новый `spawn_blocks(&mut commands, &layout,
  curb)`; вставить `CityLandmarks { tower_roof: (t.center.x, t.height, t.center.y), plaza_center:
  (centroid(plaza.inner), curb), park_center: (centroid(park.inner), curb) }` (центроид — среднее вершин,
  как `citygen::geom::centroid`; `geom` приватен, посчитать на месте одной строкой).
- `spawn_blocks`: для каждого квартала с `curb.len() >= 3`: точки `(p.x, -GROUND_SLAB_THICKNESS, p.y)` и
  `(p.x, curb, p.y)` → `let Some(collider) = Collider::convex_hull(points) else { continue };` →
  `commands.spawn((CityBlock, RigidBody::Static, collider, Transform::IDENTITY))`.
- `spawn_buildings`: если `upper_tiers` пуст — как сейчас; иначе `Collider::compound(parts)`, где части —
  `(Vec3::new(0, mid_k − height/2, 0), Quat::IDENTITY, Collider::cuboid(2·hx_k, top_k − bottom_k, 2·hy_k))`
  для базы и каждого яруса; `Transform` не меняется (центр `height/2`, поворот по `axis`).
  **Рабочий пример** (пример 1 выше, height 117): база 30 × 31.2 × 24, local y = 15.6 − 58.5 = −42.9;
  ярус 1: 24 × 31.2 × 18, y = 46.8 − 58.5 = −11.7; ярус 2: 18 × 54.6 × 12, y = 89.7 − 58.5 = 31.2.
- `world/mod.rs`: экспорт `CityBlock`, `CityLandmarks`, `Tier`, `Landmarks`; `.register_type::<CityLandmarks>()`
  в ветке `City`.
- Проверка: `cargo build -p gta_sim`, `cargo tree -p gta_sim -e normal -i bevy_render` пуст.

### Шаг 6. gta_sim: тесты (ре-анкор T2-гейтов + новые)

- `tests/common/mod.rs::settle`: `expected = PlayerSpawn.0.y + float_height` (для тестовой площадки
  `PlayerSpawn = ZERO`, поведение не меняется). Это и есть гейт бордюра: без коллайдера квартала игрок
  сядет на `float_height`, а не на `0.15 + float_height` → RED.
- `tests/city.rs::one_static_collider_per_building` (ре-анкор): для зданий без ярусов — прежняя проверка
  cuboid; с ярусами — `collider.shape().as_compound()` (`parry3d-0.27.0/src/shape/shape.rs:514`),
  `shapes().len() == 1 + upper_tiers.len()`, для каждой части полуразмеры cuboid и `translation.y`
  совпадают с ожидаемыми из layout (допуск 1e-3); число сущностей `CityBuilding` == числу зданий
  (один коллайдер на здание — T2-закон сохранён). Сопоставление по центру — как сейчас.
- Новый `landmarks_resource_matches_layout`: `CityLandmarks.tower_roof.y` == максимальной высоте зданий,
  xz == центр `buildings[landmarks.tower]`; `park_center.y == curb_height`; число `CityBlock` == числу
  кварталов с `curb.len() >= 3`.
- `runtime_hash_matches_golden` проходит после ре-bless без правок.
- Проверка: `cargo test -p gta_sim` зелёный целиком.

### Шаг 7. Клиент: конфиг рендера (`src/visuals/config.rs`, `assets/world/render.ron`)

Перенести `RenderConfig` из `visuals/mod.rs` в новый `src/visuals/config.rs` (как `camera/config.rs`),
`#[derive(Resource, Deserialize, Clone)]`, `deny_unknown_fields` на всех структурах, поля `pub(super)`.
Добавить поля (все новые числа — здесь, не в `const`):
```ron
(
    ambient_brightness: 300.0, sun_illuminance: 10000.0, sun_pitch_deg: -45.836624, sun_yaw_deg: -28.64789,
    shadows: (map_size: 4096, cascades: 4, first_cascade_far_bound: 20.0, maximum_distance: 350.0),
    chunk_size: 128.0,
    fog: (color: (0.74, 0.82, 0.92), start: 250.0, end: 450.0, sun_glow: (1.0, 0.95, 0.85, 0.5), sun_glow_exponent: 20.0),
    sky: (zenith: (0.32, 0.52, 0.86), radius: 900.0, gradient_exponent: 0.6),
    district_colors: (...без изменений...),
    tower_color: (0.55, 0.62, 0.72),
    hospital_color: ..., police_color: ..., gang_hq_color: ..., road_color: ..., sidewalk_color: ..., park_color: ...,  // как было
    curb_color: (0.70, 0.70, 0.68),
    plaza_color: (0.72, 0.68, 0.60),
    lot_colors: (downtown: (0.55, 0.55, 0.56), commercial: (0.60, 0.58, 0.55), residential: (0.38, 0.58, 0.30), industrial: (0.45, 0.43, 0.40)),
    surface_layer_step: 0.02,
    markings: (color: (0.92, 0.92, 0.88), center_color: (0.95, 0.78, 0.20), width: 0.15, dash: 3.0, gap: 6.0,
               crosswalk_depth: 3.0, crosswalk_stripe: 0.5, crosswalk_gap: 0.5),
    facade: (bay_width: 3.2, window_width: 0.55, window_height: 0.55, window_sill: 0.25,
             glass_color: (0.18, 0.24, 0.32), glass_roughness: 0.12, wall_roughness: 0.85),
    props: (
        visibility_range: 120.0, fade: 10.0,
        lamp: (model: "third_party/city-kit-roads/light-square.glb", texture: "third_party/city-kit-roads/Textures/colormap.png", scale: 10.0),
        traffic_light: (model: "third_party/city-kit-roads/traffic-light.glb", texture: "third_party/city-kit-roads/Textures/colormap.png", scale: 10.0),
        tree_large: (model: "third_party/city-kit-suburban/tree-large.glb", texture: "third_party/city-kit-suburban/Textures/colormap.png", scale: 10.0),
        tree_small: (model: "third_party/city-kit-suburban/tree-small.glb", texture: "third_party/city-kit-suburban/Textures/colormap.png", scale: 10.0),
        container_a: (model: "third_party/city-kit-industrial/shipping-container-a.glb", texture: "third_party/city-kit-industrial/Textures/colormap.png", scale: 2.0),
        container_c: (model: "third_party/city-kit-industrial/shipping-container-c.glb", texture: "third_party/city-kit-industrial/Textures/colormap.png", scale: 2.0),
        lamp_spacing: 24.0, lamp_corner_clearance: 4.0, lamp_curb_offset: 0.6, tree_curb_offset: 1.2,
        traffic_light_offset: 1.0, park_tree_spacing: 14.0, park_tree_jitter: 3.0, park_tree_margin: 3.0,
        container_gap: 1.0,
    ),
)
```
Масштабы выведены из сырых габаритов: фонарь 0.600 × 10 = 6.0 м, светофор 5.15 м, дерево 7.7 / 5.7 м,
контейнер 3.046 × 2 = 6.1 м длина, 2.76 м ширина, 2.58 м высота (20-футовый: 6.06 × 2.44 × 2.59).
`RenderConfig::validate(&self) -> Result<(), String>`: `chunk_size > 0`, `0 < fog.start < fog.end`,
`sky.radius > fog.end`, `0 ≤ props.fade < props.visibility_range`, все `scale`, `*_spacing`, `bay_width`,
`dash`, `width` > 0, `window_*` в (0, 1). `src/main.rs:70-76`: после загрузки вызвать `validate()`, ошибка →
`eprintln!("{}: {message}", root.path(RENDER_CONFIG).display())` + `AppExit::error()` (как city.ron).
Проверка: `cargo build` (клиент).

### Шаг 8. Клиент: фасадный материал (`src/visuals/facade.rs`, `assets/shaders/facade.wgsl`)

- `facade.rs`: `pub type FacadeMaterial = ExtendedMaterial<StandardMaterial, FacadeExtension>;`
  `#[derive(Asset, AsBindGroup, Reflect, Debug, Clone)] pub struct FacadeExtension { #[uniform(100)]
  pub params: FacadeParams }`, `#[derive(ShaderType, Reflect, Debug, Clone)] pub struct FacadeParams {
  glass: Vec4 /* linear rgb, 1 */, window: Vec4 /* width, height, sill, glass_roughness */ }`;
  `impl MaterialExtension for FacadeExtension { fn fragment_shader() -> ShaderRef {
  "shaders/facade.wgsl".into() } }` (путь — закон, `const FACADE_SHADER`); `fn facade_material(config) ->
  FacadeMaterial` (base: `StandardMaterial { base_color: WHITE, perceptual_roughness: wall_roughness,
  ..default() }`). Импорты: `bevy::pbr::{ExtendedMaterial, MaterialExtension}`,
  `bevy::render::render_resource::{AsBindGroup, ShaderType}`, `bevy::shader::ShaderRef` (как в
  `scratch/bevy_examples/extended_material.rs`).
- `facade.wgsl`: скелет примера (forward-ветка), между `pbr_input_from_standard_material` и
  `alpha_discard`:
  ```wgsl
  #ifdef VERTEX_UVS_A
  if (in.uv.x >= 0.0) {
      let cell = fract(in.uv);
      let inside = abs(cell.x - 0.5) < 0.5 * facade.window.x
          && cell.y > facade.window.z && cell.y < facade.window.z + facade.window.y;
      if (inside) {
          pbr_input.material.base_color = vec4<f32>(facade.glass.rgb, 1.0);
          pbr_input.material.perceptual_roughness = facade.window.w;
      }
  }
  #endif
  ```
  Имена полей `VertexOutput` сверены (`bevy_pbr-0.19.1/src/render/forward_io.wgsl:32-56`); поле
  `perceptual_roughness` есть в `StandardMaterial` WGSL (`bevy_pbr-0.19.1/src/render/pbr_types.wgsl:11`). Prepass-шейдер не
  переопределяется (окна не меняют глубину/альфу).
- Проверка: `cargo build`; визуально — шаг 13.

### Шаг 9. Клиент: геометрия чанков (`src/visuals/city_mesh.rs`, новый, чистые функции)

`pub(super) fn build_city_meshes(layout: &CityLayout, params: &CityParams, config: &RenderConfig) ->
Vec<ChunkMesh>`, `pub(super) struct ChunkMesh { pub coord: UVec2, pub buildings: u32, pub mesh: Mesh }`.
Внутри аккумулятор `struct MeshBuilder { positions, normals, uvs, colors, indices }` → `Mesh` с
`ATTRIBUTE_POSITION/NORMAL/UV_0/COLOR`, `Indices::U32`, `RenderAssetUsages::default()`. Цвет вершины =
`Color::srgb(r,g,b).to_linear().to_f32_array()` (вершинные цвета линейны; `render.ron` хранит sRGB). UV
не-фасадов = `[-1.0, -1.0]`.

- **Сетка:** `n = (params.size / config.chunk_size).ceil() as u32`;
  `chunk_of(p: Vec2) -> UVec2`: `i = ((p.x + size/2) / chunk).floor()` с clamp в `[0, n-1]`, так же по y.
  Прямоугольник асфальта чанка `i`: `lo = -size/2 + i·chunk`, `hi = lo + chunk`, при `i == 0`
  `lo = -ground/2`, при `i == n-1` `hi = ground/2`.
  **Примеры** (size 1200, chunk 128, ground 1400): `n = ceil(9.375) = 10` → 100 чанков; точка (0, 0) →
  `floor(600/128) = 4`; (−600, −600) → 0; (620, 0) → `floor(1220/128) = 9` (clamp не нужен), (700, 0) →
  `floor(10.16) = 10` → clamp 9; чанк 9 по x: [552, 700], чанк 0: [−700, −472]; объединение = [−700, 700].
- **Правило обхода (одно на весь файл, проверено на двух направлениях):** горизонтальный CCW-полигон в
  (x, y) (положительная площадь) выводится веером `(0, i+1, i)` с нормалью +Y — как в текущем
  `flat_mesh` (`src/visuals/city.rs:119-122`). Вертикальный квад по ребру `a→b` CCW-контура (внутренность
  слева, `(b-a).perp()`), низ `y0`, верх `y1`: вершины `A0, B0, B1, A1`, индексы `[A0, B1, B0, A0, A1, B1]`,
  нормаль `-(b-a).perp().normalize()` в мир как `(x, 0, y)`. Пример 1: `a=(0,0) → b=(10,0)`, наружу −Z;
  с камеры в z = −5, смотрящей на +Z, экранная ось вправо = −X; `A0 (0,0)`, `B1 (−10,h)`, `B0 (−10,0)`:
  `(−10,h)×(−10,0) = 10h > 0` → CCW → лицевая. Пример 2: `a=(0,0) → b=(0,10)`, наружу +X; камера в x = +5
  смотрит на −X, вправо = −Z, экранные координаты те же → лицевая. Пример 3 (обратное ребро
  `a=(10,0) → b=(0,0)`, внутренность теперь −Z, наружу +Z): камера z = +5 смотрит на −Z, вправо = +X;
  `A0 (10,0)`, `B1 (0,h)`, `B0 (0,0)`: `(−10,h)×(−10,0) = 10h > 0` → лицевая.
- **Асфальт:** на каждый чанк квад `[lo, hi]²` на y = 0, `road_color`.
- **Квартал** (`curb.len() >= 3`, чанк по центроиду `curb`): стенки бордюра по каждому ребру `curb`
  (y 0 → `curb_height`, `curb_color`); если `inner` пуст — верх = весь `curb` на `curb_height`
  (`sidewalk_color`); иначе кольцо тротуара трапециями `curb[k], curb[k+1], inner[k+1], inner[k]` (CCW,
  индексы `inset` совпадают, `geom.rs:39-49`) и `inner` на той же высоте цветом: `park_color` если
  `is_park`, `plaza_color` для `landmarks.plaza`, иначе `lot_colors[district]`. Одна высота, без
  перекрытия — нет z-fighting.
- **Здание** (чанк по центру): углы `p0 = c − u·hx − v·hy`, `p1 = c + u·hx − v·hy`, `p2 = c + u·hx + v·hy`,
  `p3 = c − u·hx + v·hy` (CCW, `v = u.perp()`), для базы и каждого яруса: 4 стены по правилу квада
  (y от низа до верха яруса), крыша яруса веером на его верхе; цвет: `Tower → tower_color`, `Hospital`,
  `PoliceStation`, `GangHq`, иначе `district_colors`. UV стены: `bays = max(1, round(len/bay_width))`,
  `u = t·bays` (t = 0 у `a`, 1 у `b`), `v = y / floor_h` (floor_h района; башня — downtown). Крыши UV −1.
- **Разметка** (рёбра не Alley): `trim[node] = max half_carriageway` инцидентных рёбер (как
  `citygen/src/graphs.rs:83-88`), рабочий отрезок ребра `a + d·(trim_a + crosswalk_depth)` →
  `b − d·(trim_b + crosswalk_depth)` (длина ≤ 0 → пропуск), квады на `surface_layer_step`, чанк по центру
  квада. Avenue: сплошная осевая `center_color`; street: пунктир `color` (`dash`/`gap`, центрирован);
  разделители полос пунктиром на смещениях `±k·lane_width`, `k = 1..lanes_per_direction−1`. Зебра на
  обоих концах: полосы вдоль `d` длиной `crosswalk_depth`, шириной `crosswalk_stripe`, шаг
  `stripe + gap`, поперёк проезжей части `[−half, half]`, сразу за `trim` узла.
- `ChunkMesh.buildings` = число зданий, попавших в чанк. Результат упорядочен по `coord`.
- Файл < 750 строк; если больше — вынести разметку в `city_markings.rs`.

### Шаг 10. Клиент: пропы (`src/visuals/props.rs`, новый)

- `#[derive(Clone, Copy)] pub(super) enum PropKind { Lamp, TrafficLight, TreeLarge, TreeSmall,
  ContainerA, ContainerC }`, `pub(super) struct PropPlacement { kind, position: Vec3, yaw: f32 }`.
- `pub(super) fn place_props(layout, params, config) -> Vec<PropPlacement>` (чистая, детерминированная;
  "случайность" — локальный `splitmix64(seed ^ block ^ i ^ j)`, без RNG-крейтов). Все пропы на
  `y = curb_height`.
  1. **Фонари:** каждое ребро `curb[k]→curb[k+1]` квартала со стороной `sides[k]` ≠ Alley: длина `L`,
     `usable = L − 2·clearance`; `usable < 0` → нет; `count = floor(usable/spacing) + 1`, позиции
     `t_j = clearance + (usable − (count−1)·spacing)/2 + j·spacing`, точка `a + d·t_j + inward·lamp_curb_offset`
     (`inward = d.perp()`). Плечо модели смотрит в −Z; нужно наружу (к дороге) `o = −inward` → в мир
     `(o.x, 0, o.y)`; `yaw = atan2(−o.x, −o.z)` при `R_y(θ)·(0,0,−1) = (−sinθ, 0, −cosθ)` (формула GDD §3.2).
     Примеры: `o = (0,0,−1)` → `θ = atan2(0, 1) = 0` → плечо −Z ✓; `o = (1,0,0)` → `θ = atan2(−1, 0) = −90°`
     → `(−sin(−90°), 0, −cos(−90°)) = (1,0,0)` ✓; `o = (0,0,1)` → `θ = atan2(0, −1) = 180°` → `(0,0,1)` ✓.
     Пример расстановки: `L = 80, clearance 4, spacing 24` → `usable 72`, `count 4`, `t = 4, 28, 52, 76`.
  2. **Деревья вдоль улиц:** те же рёбра у кварталов района Residential и у парков: по одному дереву в
     середине каждого промежутка между соседними фонарями (`t = 16, 40, 64` в примере),
     `inward·tree_curb_offset`, крупное/мелкое по чётности хэша.
  3. **Деревья в парках** (`is_park`, непустой `inner`): сетка по мировым осям с шагом
     `park_tree_spacing`, сдвиг хэшем в `±park_tree_jitter`, оставить точки, где расстояние до каждой
     стороны `inner` ≥ `park_tree_margin` (через `(p − a)·inward ≥ margin`).
  4. **Светофоры:** вершина `k` `curb`, у которой `sides[k−1]` и `sides[k]` — Avenue: точка
     `curb[k] + bisector·traffic_light_offset` (`bisector = normalize(inward_{k−1} + inward_k)`), модель −Z к
     перекрёстку (`o = −bisector`, та же формула yaw). Ориентацию головы светофора владелец проверяет на
     скриншоте (у модели нет плеча, направление "лицом" по габаритам не определить).
  5. **Контейнеры:** здания `Generic` в районе Industrial: тыльная сторона `+v` (фронтаж на `−v`, т.к. `v`
     смотрит внутрь лота, `lots.rs:150-151`). Ряд вдоль `u`: ширина 2.76 м и длина 6.1 м
     (сырые габариты × `scale`), центр ряда `c + v·(hy + container_gap + w/2)`, `count = floor(2·hx /
     (len + gap))`, шаг `len + gap`, каждый контейнер ставится, только если 4 угла внутри полигона лота
     (`citygen::contains_convex(lot, corner, 0.0)`, pub). Длинная ось модели Z вдоль `u`:
     `R_y(θ)·(0,0,1) = (sinθ, 0, cosθ)` → `θ = atan2(u.x, u.y)`. Примеры: `u = (1,0)` → 90° → (1,0,0) ✓;
     `u = (0,1)` → 0 → (0,0,1) ✓; `u = (−1,0)` → −90° → (−1,0,0) ✓. A/C по чётности хэша.
- `#[derive(Resource)] pub(super) struct PropAssets { meshes: [Handle<Mesh>; 6], materials: [Handle<StandardMaterial>; 6] }`,
  `Startup`-система `load_prop_assets`: `asset_server.load(GltfAssetLabel::Primitive { mesh: 0, primitive: 0 }.from_asset(path))`
  и по одному `StandardMaterial { base_color_texture: Some(asset_server.load(texture)), perceptual_roughness: 0.8, ..default() }`
  на уникальную текстуру (HashMap по пути). Текстура пакета — та же, на которую ссылается GLB, значит один
  `Image` в памяти.
- `#[derive(Component, Reflect)] #[reflect(Component)] pub struct CityProp;` Спавн (из шага 11):
  `(CityProp, Mesh3d(mesh), MeshMaterial3d(material), Transform::from_translation(pos).with_rotation(Quat::from_rotation_y(yaw)).with_scale(Vec3::splat(scale)),
  VisibilityRange { start_margin: 0.0..0.0, end_margin: (range − fade)..range, use_aabb: false })`
  (`bevy::camera::visibility::VisibilityRange`, путь как в `scratch/bevy_examples/visibility_range.rs:6`).
  Оценка числа: ~2400 фонарей, ~1200 деревьев улиц, ~400 в парках, ~300 контейнеров, ~100 светофоров.

### Шаг 11. Клиент: плагин города (`src/visuals/city.rs`, переписать)

- Удалить `visualize_city_building`, `spawn_city_surfaces`, `CityPalette`, `flat_mesh` (логика
  переезжает в `city_mesh.rs`).
- `#[derive(Component, Reflect)] #[reflect(Component)] pub struct CityChunk { pub coord: UVec2, pub buildings: u32 }`.
- `#[derive(Resource)] struct CityMeshTask(Task<(Vec<ChunkMesh>, Vec<PropPlacement>)>)`;
  `#[derive(Resource)] struct CityFacade(Handle<FacadeMaterial>)`.
- `CityVisualsPlugin::build`: `register_type::<CityChunk>().register_type::<CityProp>()`,
  `Startup`: `create_facade_material` (из `RenderConfig`), `load_prop_assets`;
  `OnEnter(GameState::Playing)`: `start_city_mesh_build` (клоны `City.0`, `CityParamsRes.0`,
  `RenderConfig` → `AsyncComputeTaskPool::get().spawn(async move { (build_city_meshes(..),
  place_props(..)) })`); `Update`: `apply_city_mesh_build.run_if(resource_exists::<CityMeshTask>)` —
  `let Some(result) = block_on(poll_once(&mut task.0)) else { return };`, удалить ресурс, заспавнить
  `(CityChunk { coord, buildings }, Mesh3d(meshes.add(mesh)), MeshMaterial3d(facade.clone()),
  Transform::IDENTITY)` на каждый чанк и пропы через `commands.spawn_batch`. `run_if(resource_exists)` здесь
  — жизненный цикл задачи, не гейтинг по состоянию (он уже на `OnEnter`). Тот же паттерн, что
  `gta_sim/src/world/city.rs:47-88`.
- Клон `CityLayout` на входе задачи — один раз за игру, порядка мегабайта; приемлемо, записать в Risk.
- Проверка: `cargo build`, `cargo clippy -- -D warnings`.

### Шаг 12. Клиент: свет, небо, туман, сборка (`src/visuals/mod.rs`, `src/visuals/sky.rs`)

- `mod.rs`: `mod config; mod facade; mod city_mesh; mod props; mod sky; #[cfg(test)] mod city_gate;`;
  `pub use config::{RENDER_CONFIG, RenderConfig};` (импорт в `main.rs` сохраняется).
  `VisualsPlugin::build`: как сейчас + `.insert_resource(DirectionalLightShadowMap { size })`,
  `.insert_resource(ClearColor(fog_color))`, `.add_plugins((MaterialPlugin::<FacadeMaterial>::default(),
  sky::SkyPlugin, city::CityVisualsPlugin))`. `spawn_light`: добавить `CascadeShadowConfigBuilder {
  num_cascades, first_cascade_far_bound, maximum_distance, ..default() }.build()` на сущность солнца
  (`bevy::light::CascadeShadowConfigBuilder`). `visualize_block`, `visualize_character` не трогать.
- `sky.rs` (новый): `SkyPlugin`: наблюдатель `On<Add, Camera3d>` вставляет `DistanceFog { color,
  directional_light_color: Color::srgba(sun_glow), directional_light_exponent, falloff:
  FogFalloff::Linear { start, end } }`; `Startup` спавнит `SkyDome` (`Sphere::new(radius).mesh().uv(32, 16)`,
  `bevy_mesh-0.19.1/src/primitives/dim3/sphere.rs:172`, + `ATTRIBUTE_COLOR` по вершине:
  `mix(horizon=fog.color, zenith, clamp(y/radius, 0, 1)^gradient_exponent)` в линейном пространстве,
  материал `StandardMaterial { base_color: WHITE, unlit: true, fog_enabled: false, cull_mode: None }`,
  `NotShadowCaster`); `Update`-система `follow_camera` копирует `translation` камеры в купол (лаг в кадр на
  радиусе 900 м невидим). `sky.radius 900 < far 1000` камеры.
- Проверка: `cargo build`, `cargo clippy -- -D warnings`.

### Шаг 13. Клиент: гейт слияния (`src/visuals/city_gate.rs`, `#[cfg(test)]`)

Класс: корректность (бюджет кадра). Харнесс `city_visuals_app(seed)`:
`MinimalPlugins + TransformPlugin + AssetPlugin::default() + StatesPlugin`,
`TimeUpdateStrategy::FixedTimesteps(1)`, `init_asset::<Mesh>()`, `init_asset::<StandardMaterial>()`,
`init_asset::<Image>()`, `init_asset::<FacadeMaterial>()` (заглушки рендер-плагинов; без них
`AssetServer::load` паникует "asset type has not been initialized", `bevy_asset-0.19.1/src/server/info.rs:807-823`), `insert_resource(RenderConfig)` из
`ConfigRoot(CARGO_MANIFEST_DIR/assets)` через `load_config` + `validate`, `compose_sim(..,
WorldSource::City { seed: 1 })`, `add_plugins(CityVisualsPlugin)` (production-плагин целиком),
`app.finish(); app.cleanup();`, затем `update()` до появления хотя бы одного `CityChunk` (дедлайн 120 с,
`GATE BROKEN` при `should_exit`). Загрузка GLB/PNG без загрузчиков даёт ошибки в логе, не панику —
имплементер подтверждает первым прогоном.
Тест `city_meshes_are_merged_per_chunk`:
1. `expected = ((params.size / render.chunk_size).ceil() as usize).pow(2)` — **посчитано в тесте**,
   `chunk_of`/`n` реализации не вызываются. Комментарий-пример: 1200/128 → 10 → 100.
2. `count(With<Mesh3d>, With<CityBuilding>) == 0` (нет пер-зданийных мешей).
3. `count(With<Mesh3d>, With<CityChunk>) == expected`; множество `coord` уникально и покрывает
   `[0, n)²`.
4. `Σ CityChunk.buildings == City.0.buildings.len()` (слияние ничего не потеряло).
5. `count(With<Mesh3d>, Without<CityChunk>, Without<CityProp>) == 0` (никаких посторонних мешей города:
   ни отдельной земли, ни пер-квартальных тротуаров).
6. Каждая сущность `CityProp` имеет `VisibilityRange` с `end_margin.end == props.visibility_range`;
   пропов > 0.
Запуск: `cargo test -p gta_like --bin gta_like city_meshes_are_merged_per_chunk`.

### Шаг 14. QA: `tools/qa/brp.py` + `tools/qa/scenarios/t3.py`

- `brp.py`: `Game.__init__(..., release=False)`; в `start` при `release` добавить `"--release"` в команду
  сборки и брать `.../release/gta_like(.exe)`. Остальное не трогать (t1/t2 работают как раньше).
- `t3.py` (по образцу `t2.py`): `Game(features=("dev",), args=("--seed", "1"), release=True)`;
  `wait_resource("CityLayoutHash", 180)` == `load_golden()[1]`; дождаться строк `CityChunk` (query,
  ≤ 30 с) и записать их число; прочитать `CityLandmarks` через
  `game.call("world.get_resources", {"resource": game.resource_path("CityLandmarks")})["value"]`
  (`vec3()` принимает и список, и dict); телепорт игрока (`Position`, путь `""`, `[x, roof_y + 1.2, z]`),
  2 с, проверить `roof_y + 0.8 < y < roof_y + 1.4` (стоит на крыше яруса — коллайдер compound работает);
  3 скриншота с крыши с поворотом `move_mouse(dx=1000, dy=0)` между ними и один с наклоном вниз
  (`dy=150`); прогрев 5 с, 5 чтений `get_diagnostics` раз в секунду → FPS min/avg и frame time, плюс
  `window_state()` (present mode); телепорт в `park_center + 1.2`, 2 с, скриншот; `summary.json` в `--out`
  (по умолчанию `target/qa/t3`). FPS — доказательство для владельца, не assert.
- Проверка: `python tools/qa/scenarios/t3.py --out <scratch>/qa/t3` (QA-стадия; имплементер на codex
  окно не запускает).

---

## 4. Test plan

### 4.1 Гейты и их класс

| Гейт | Где | Класс | Что держит |
|---|---|---|---|
| `manifest_matches_committed_assets` | `gta_sim/tests/asset_manifest.rs` | корректность | SHA-256 каждого файла, лицензия CC0 по байтам, точный состав пакета |
| `city_meshes_are_merged_per_chunk` | `src/visuals/city_gate.rs` (`-p gta_like`) | корректность (бюджет кадра) | 0 пер-зданийных `Mesh3d`, `n²` чанков из `render.ron`, все здания в чанках, пропы с `VisibilityRange` |
| `landmarks_exist`, `setback_tiers_nested` | `citygen/tests/properties.rs` | корректность | башня единственная и самая высокая, площадь/парк, вложенность ярусов |
| golden (ре-bless) | `citygen/tests/golden.rs`, `gta_sim/tests/city.rs` | детерминизм | новая схема хэша v2 |
| `one_static_collider_per_building` (ре-анкор) | `gta_sim/tests/city.rs` | корректность | один коллайдер на здание, compound = ярусы |
| `settle` (изменён) через все city-тесты | `gta_sim/tests/common` | корректность | игрок стоит на бордюре `curb + float` |
| `landmarks_resource_matches_layout` | `gta_sim/tests/city.rs` | liveness + корректность | BRP-точки QA совпадают с layout |
| `validate_rejects_bad_params` (+ башня) | `citygen/src/params.rs` | строгость данных | башня обязана быть выше всех |

Команды: `cargo build`, `cargo clippy -- -D warnings`, `cargo test -p citygen`, `cargo test -p gta_sim`,
`cargo test -p gta_like`, `python tools/qa/tree_check.py`, `python tools/fetch_assets.py --check`.

### 4.2 Flip-RED (испортить → RED → вернуть → GREEN; в IMPL_SUMMARY записать, что портили)

1. Слияние: вернуть наблюдатель `On<Add, CityBuilding>`, вешающий `Mesh3d(Cuboid)` (старый
   `src/visuals/city.rs:54-77`) → пункт 2 гейта RED (требование `TASK_FINAL.md`).
2. Размер чанка: в `build_city_meshes` взять `n = 1` (один меш на город) → пункт 3 RED (1 ≠ 100).
3. Потеря зданий: пропускать здания последнего чанка → пункт 4 RED.
4. Манифест: поменять одну hex-цифру `sha256` у `light-square.glb` → RED с именем файла; положить
   лишний `assets/third_party/city-kit-roads/extra.glb` → RED по составу.
5. Башня: в `city.ron` `tower_floors: 20` → `validate` падает (строгость) и, в обход validate на
   тестовом `CityParams`, `landmarks_exist` RED на "строго выше".
6. Бордюр: не спавнить `CityBlock` → `settle` RED (`y ≈ float` вместо `0.15 + float`).
7. Ярусы: в `spawn_buildings` всегда cuboid → `one_static_collider_per_building` RED на башнях.

### 4.3 Owner checklist (QA переносит в QA_REPORT.md)

`cargo run --release --features fast -- --seed 1`, затем `--seed 2`:
1. Город выглядит как город: тротуары приподняты с бордюром, на асфальте осевые, разделители полос и
   зебры, у зданий окна по этажам, у высоток видны уступы.
2. Районы различимы: Downtown (серо-синие высотки, бетонные лоты), Commercial (бежевый, средняя высота),
   Residential (низкие дома, зелёные лоты, деревья вдоль улиц), Industrial (низкие корпуса, контейнеры за
   зданиями).
3. Ориентиры: самая высокая башня на площади у центра видна издалека; центральный парк с деревьями.
4. Солнце даёт тени от зданий до ~350 м; небо — градиент; дальний план уходит в туман 250-450 м без
   шва с небом.
5. Фонари стоят на тротуаре плечом к дороге; светофоры на углах авеню смотрят разумно; пропы исчезают
   дальше ~120 м без заметного "попа" (fade 10 м).
6. Игрок заходит на тротуар с дороги без застревания (бордюр 0.15 м); с крыши башни виден город.
7. FPS с крыши (цифры из `summary.json` t3) — информация для владельца.
Всё крутится в `assets/world/render.ron` (вид) и `assets/world/city.ron` (ориентиры, уступы, бордюр).

---

## 5. Risk areas

- **Golden ре-bless маскирует случайное изменение генератора.** Порядок: сначала properties зелёные
  (включая новые), потом bless; в IMPL_SUMMARY перечислить, какие изменения меняют хэш (схема v2, ярусы,
  башня, площадь/парк).
- **Выбор площади/парка упадёт на каком-то seed** (`NoLandmarkCandidate`). 35-seed properties и прошлый
  прогон 2007 seed (TASK-003) — доказательство; при провале — расширить радиус, не убирать ошибку.
- **Имена в WGSL (`pbr_input.material.perceptual_roughness`, `VERTEX_UVS_A`)** могли измениться в 0.19 —
  сверка по `bevy_pbr-0.19.1/src/render/*.wgsl` до написания; ошибка шейдера видна только в окне (QA).
- **Харнесс клиентского гейта:** `Mesh3d`/`MeshMaterial3d` без рендер-плагинов, загрузка GLB без
  `GltfPlugin`. Если `AssetServer::load` паникует на незарегистрированном типе — добавить соответствующий
  `init_asset`, не выносить систему из production-плагина.
- **Обход граней:** неверный порядок индексов даст невидимые стены (back-face culling). Правило и три
  примера в шаге 9; QA-скриншоты с крыши и земли это покажут.
- **Кадр применения:** спавн ~100 мешей + ~4.5 тыс. пропов в одном кадре после задачи. Замерить
  (`profile` или `city_startup_budget`-подобный лог); если > 16 мс — разрезать спавн пропов курсором по
  N за кадр (данные в `render.ron`).
- **Клон `CityLayout`** в задачу (порядка МБ) — одноразовый, в пределах миллисекунды; если trace
  покажет иначе — перейти на `Arc` в `City` отдельной задачей.
- **Внешняя текстура GLB:** `Textures/colormap.png` должна лежать по относительному пути, иначе glTF-
  загрузчик логирует ошибку; манифест и состав каталога это держат.
- **Кенни поменяет хэш в URL** — манифест держит проверенный архив; `fetch_assets.py` падает с явным
  сообщением, новая ссылка берётся со страницы пакета (GDD §9.2).
- **Z-fighting разметки** (2 см над асфальтом на 400+ м) — при жалобе владельца увеличить
  `surface_layer_step`, туман с 250 м это частично скрывает.
- **Физика бордюра и машины (T14):** ступень 0.15 м для raycast-подвески — проверит T14.
- **Unifying features:** `cargo test -p gta_like` собирает клиент с рендер-фичами — это клиентский
  тест, граница sim/render проверяется отдельно `tree_check.py`.

## 6. Open questions

Блокирующих нет; ниже — решения с рекомендацией по умолчанию (оркестратор может подтвердить по
делегированию).

1. **Коллизия пропов.** В T3 фонари, деревья и контейнеры — только визуал, игрок проходит сквозь них.
   - A (рекомендуется): так и оставить до T14 (машина) — тогда вынести расстановку в `citygen` и дать
     статические коллайдеры слоя `Prop`.
   - B: коллайдеры сейчас — расстановка переезжает в `citygen` (golden, gta_sim), слайс растёт.
2. **Готовые здания Kenney на части лотов** (GDD §2.2 шаг 5, рычаг §2.6) не входят в список цели T3.
   - A (рекомендуется): не делать в T3; добавить отдельной задачей, если владелец захочет разнообразия.
   - B: сделать сейчас — пакет City Kit Commercial (`f8b09b08...`, уже скачан в `scratch/kenney/`),
     подбор по футпринту, коллайдер по габаритам модели.
3. **Небо.** Градиентный купол (рекомендуется, дёшево, тюнится в `render.ron`) против процедурной
   `Atmosphere` Bevy 0.19 (реалистичнее, но требует HDR, физического солнца и перетюнинга света) —
   вернуться вместе с "временем суток".

---

children: 0 launched / 0 reported.
