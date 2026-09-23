# PLAN_FINAL — TASK-004 (GDD T3): облик города

Ветка `feature/t03-city-look` (уже checked out). Работать in-place, без worktree/clone/второго `target/`.
Все API ниже сверены plan-reviewer-2 по `~/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/`
(версии из `Cargo.lock`: bevy 0.19.1, avian3d 0.7.0, parry3d 0.27.0) и по примерам тега v0.19.1 в
`maw/tasks/in_progress/TASK-004/scratch/bevy_examples/`. Хэши Kenney пересчитаны reviewer-2 по локальным
zip в `scratch/kenney/` (все 3 архива и 12 файлов совпали с таблицей шага 2).

## 1. Summary

Слайс T3 делает город похожим на город, не трогая границу sim/render. `citygen` получает ориентиры:
башню `BuildingKind::Tower` на площади у центра и центральный парк. Высотки получают ярусы-уступы
(`Building.upper_tiers`), это меняет схему хэша на v2 с намеренным re-bless golden. `gta_sim` поднимает
кварталы на физический бордюр (один выпуклый статический коллайдер на квартал), переводит здания с
ярусами на `Collider::compound`, оставляя один коллайдер на здание, поднимает `PlayerSpawn` на
`curb_height` и публикует `CityLandmarks` для BRP. Клиент удаляет `Mesh3d` у каждого здания. Геометрия
города (асфальт, бордюр, тротуары, лоты, здания с фасадным UV, разметка) собирается чистой функцией в
`AsyncComputeTaskPool` в один меш на render-чанк (`chunk_size` в `render.ron`) с общим
`ExtendedMaterial<StandardMaterial, FacadeExtension>` (окна рисует WGSL). Kenney-пропы (фонари,
светофоры, деревья, контейнеры) остаются только визуалом, у каждого свой `VisibilityRange`. Всё
применяется порциями с бюджетом из `render.ron`. Солнце настраивается каскадами, небо — градиентный
unlit-купол, `ClearColor` = цвет тумана, `DistanceFog::Linear`. Ассеты по owner rule в git не попадают:
tracked только `assets/third_party/manifest.ron`. `tools/fetch_assets.py` скачивает, проверяет и
распаковывает пакеты. Клиент при старте падает с понятным сообщением, если файлов нет. Гейты: строгий
манифест-гейт в `gta_sim` (схема всегда, файлы когда есть), headless гейт слияния в клиенте
(`cargo test -p gta_like`), property-гейты `citygen`, заново заякоренные T2-гейты коллайдеров и опоры.
Облик города принимает владелец по своему прогону и скриншотам QA t3.

## 2. Implementation steps

Порядок следует зависимостям. Каждый шаг заканчивается проверкой. Все новые настроечные числа лежат в
`assets/world/city.ron` или `assets/world/render.ron`. В коде остаются `const` только для законов:
пути файлов, толщина плиты, алгоритмические пределы.

### Шаг 0. Предусловия (без правок)

- TASK-003 (T2) лежит в `maw/tasks/done/` — подтверждено. `git status` чистый, ветка `feature/t03-city-look`.
- Перед написанием каждого Bevy/avian вызова сверить его по pinned source. Пути, уже сверенные
  ревьюером: `bevy_gltf-0.19.1/src/label.rs:41,112` (`GltfAssetLabel::Primitive{mesh,primitive}`,
  `.from_asset(impl Into<AssetPath<'static>>)`), `bevy_camera-0.19.1/src/visibility/range.rs:80`
  (`VisibilityRange{start_margin, end_margin, use_aabb}`), `bevy_pbr-0.19.1/src/fog.rs:55,102,131`
  (`DistanceFog{color, directional_light_color, directional_light_exponent, falloff}`,
  `FogFalloff::Linear{start,end}`), `bevy_light-0.19.1/src/cascade.rs:59` (`CascadeShadowConfigBuilder`,
  поля `num_cascades, minimum_distance, maximum_distance, first_cascade_far_bound, overlap_proportion`),
  `bevy_light-0.19.1/src/directional_light.rs:193` (`DirectionalLightShadowMap{size}`, степень двойки),
  `bevy_light` `NotShadowCaster`, `bevy_pbr-0.19.1/src/extended_material.rs:33,145`,
  `bevy_shader-0.19.1/src/shader.rs:406` (`ShaderRef`), `bevy_pbr-0.19.1/src/pbr_material.rs:664-673`
  (`cull_mode`, `unlit`, `fog_enabled`), `bevy_mesh-0.19.1/src/primitives/dim3/sphere.rs:172`
  (`Sphere::mesh().uv(sectors, stacks)`), `bevy_mesh-0.19.1/src/components.rs:101` (`Mesh3d` требует
  только `Transform`, то есть спавнится headless), `avian3d-0.7.0/src/collision/collider/parry/mod.rs:698`
  (`Collider::compound(Vec<(impl Into<Position>, impl Into<Rotation>, impl Into<Collider>)>)`,
  `Position: From<Vec3>` через derive `From`, `Rotation: From<Quat>`), `:985` (`convex_hull(Vec<Vector>)
  -> Option<Collider>`), `parry3d-0.27.0/src/shape/shape.rs:514` (`as_compound`).
- WGSL: `bevy_pbr-0.19.1/src/render/forward_io.wgsl:32-56` (`in.uv` под `VERTEX_UVS_A`, `in.color` под
  `VERTEX_COLORS`), `pbr_types.wgsl:6,11` (`base_color`, `perceptual_roughness`), `pbr_fragment.wgsl:54-55,101`
  (вершинный цвет подставляется в `base_color` и умножается на `material.base_color`, поэтому база
  материала WHITE даёт чистый вершинный цвет).

### Шаг 1. Git-правила (`.gitignore`; `.gitattributes` НЕ менять)

В конец `.gitignore` добавить:
```
# Third-party packs: fetched by tools/fetch_assets.py, never committed; only the manifest is tracked
/assets/third_party/*
!/assets/third_party/manifest.ron
```
Именно `/*`, а не `/assets/third_party/`. Проба reviewer-2 (`scratch/r2_gitprobe/`) показала: при
`/assets/third_party/` + `!…/manifest.ron` git молча игнорирует и манифест, потому что файл нельзя
вернуть из исключённого каталога. С `/*` индексируется только `manifest.ron`, а
`git check-ignore -v assets/third_party/city-kit-roads/License.txt` указывает на новое правило.
Проверка после шага 2: `git add -A --dry-run` показывает из `assets/third_party/` только `manifest.ron`,
`git check-ignore -v assets/third_party/city-kit-roads/License.txt` не пустой.

### Шаг 2. Манифест `assets/third_party/manifest.ron` (новый, tracked)

Содержимое дословно (все хэши перепроверены reviewer-2):
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
Факты об ассетах (планировщик, `scratch/kenney/INVENTORY.txt`). У GLB внешняя текстура
`Textures/colormap.png`, поэтому она лежит по относительному пути рядом. У каждой модели один меш и
один примитив. Узел контейнеров имеет scale 0.27: мы берём сырой примитив, так что это не важно.
Сырые габариты: `light-square` высота 0.600, плечо в −Z; `traffic-light` 0.515; `tree-large` 0.767;
`tree-small` 0.567; `shipping-container-a/c` 1.38 × 1.289 × 3.046, длинная ось Z. Отброшены: `dumpster`
(3 меша), `shipping-container-b` (2 примитива), `detail-tank` (зеркальный scale).

### Шаг 3. Единая схема манифеста: `crates/gta_sim/src/config/manifest.rs` (новый)

У схемы один источник. Её используют и гейт, и startup preflight клиента. Новых normal-зависимостей
нет, только serde.
- `crates/gta_sim/src/config/mod.rs`: добавить `pub mod manifest;`.
- `manifest.rs`:
  ```rust
  /// Path of the third-party asset manifest, relative to the assets root.
  pub const THIRD_PARTY_MANIFEST: &str = "third_party/manifest.ron";
  /// Directory of fetched packs, relative to the assets root.
  pub const THIRD_PARTY_DIR: &str = "third_party";

  #[derive(Deserialize, Debug, Clone)] #[serde(deny_unknown_fields)]
  pub struct ThirdPartyManifest { pub packs: Vec<AssetPack> }
  #[derive(Deserialize, Debug, Clone)] #[serde(deny_unknown_fields)]
  pub struct AssetPack { pub name: String, pub version: String, pub page: String, pub url: String,
      pub archive_sha256: String, pub license: AssetLicense, pub license_file: String, pub files: Vec<PackFile> }
  #[derive(Deserialize, Debug, Clone, Copy, PartialEq, Eq)]
  pub enum AssetLicense { CC0 }
  #[derive(Deserialize, Debug, Clone)] #[serde(deny_unknown_fields)]
  pub struct PackFile { pub archive: String, pub path: String, pub sha256: String }
  ```
  `impl ThirdPartyManifest { pub fn validate(&self) -> Result<(), String> }` проверяет правила ниже.
  Сообщение называет пакет, поле и значение.
  1. `packs` не пуст; `name` уникален и соответствует `^[a-z0-9-]+$`; `version` не пуст.
  2. `page == format!("https://kenney.nl/assets/{name}")`; `url` начинается с
     `https://kenney.nl/media/pages/assets/{name}/` и оканчивается на `.zip` (иначе ошибка со словом `url`).
  3. `archive_sha256` и каждый `sha256` содержат ровно 64 символа `[0-9a-f]` (ошибка со словом `sha256`).
  4. `files` не пуст. Каждый `path` и `archive` не пуст и не содержит `\`, `:`, ведущего `/`, сегментов
     `..`, `.` или пустого сегмента (ошибка со словом `path` и значением). Все `path` в пакете уникальны,
     все `archive` тоже (ошибка со словом `duplicate`).
  5. `license_file` есть среди `files[].path` (ошибка со словом `license_file`).
  Дубликаты ключей и неизвестные поля/варианты отвергает serde derive через `deny_unknown_fields`, а
  дубликат поля структуры даёт `duplicate_field`. Отдельный код для этого не нужен.
  `pub fn missing_files(&self, root: &ConfigRoot) -> Vec<PathBuf>` возвращает
  `root.path(THIRD_PARTY_DIR)/<name>/<path>` каждого отсутствующего файла.
  `pub fn contains_asset(&self, asset_path: &str) -> bool` отвечает, равен ли `asset_path`
  строке `third_party/<name>/<path>` какого-то файла.
- Проверка: `cargo build -p gta_sim`; `python tools/qa/tree_check.py` зелёный.

### Шаг 4. `tools/fetch_assets.py` (новый, только stdlib: `argparse, hashlib, json, os, pathlib, shutil, sys, time, urllib.request, zipfile`)

- **Разбор.** `parse_ron(text)` — рекурсивный спуск по фиксированной грамматике. `( key: value, ... )`
  даёт dict, повтор ключа — ошибка. `[ v, ... ]` даёт list. `"..."` даёт str, из escape-последовательностей
  разрешены только `\"` и `\\`. Голый идентификатор `[A-Za-z_][A-Za-z0-9_]*` в позиции значения даёт str.
  Поддерживаются `//`-комментарии и висячие запятые. Любой другой символ, включая имя структуры перед
  `(`, — `SystemExit` с номером строки.
  `check_schema(doc)` требует точные множества ключей на каждом уровне (лишний или недостающий ключ —
  ошибка), `license == "CC0"` и те же правила 1–5, что и `validate()` на шаге 3. Порядок и тексты
  проверок держать рядом с Rust-версией, у обеих сторон одинаковые ключевые слова ошибок.
- **Режимы** (коды выхода: 0 — успех, 1 — любая ошибка, сообщения в stderr):
  - `--validate-only FILE`: только разбор и схема указанного файла. Нужен для проверки паритета с Rust.
  - `--check`: без сети. Все пакеты манифеста должны существовать в `assets/third_party/<name>/`, состав
    каталога (рекурсивно, пути через `/`) должен совпадать с `files[].path`, SHA-256 каждого файла — с
    манифестом. Под `assets/third_party/` не должно быть ничего, кроме `manifest.ron` и каталогов пакетов.
    Выход печатает каждое расхождение.
  - По умолчанию (fetch): для каждого пакета, если он уже проходит `--check`-проверку, — пропуск (это даёт
    идемпотентность). Иначе zip берётся из кэша: каждый `--cache DIR` (флаг повторяемый), затем
    `target/asset-cache/`, файл `basename(url)` с совпадающим `archive_sha256`. Если в кэше нет, скачать
    `urllib.request` с заголовком `User-Agent: gta-like-fetch-assets` в `target/asset-cache/<basename>.part`:
    4 попытки, паузы 2/4/8 с (TLS kenney.nl нестабилен). Проверить SHA-256, при расхождении — ошибка с
    обоими значениями. Затем `os.replace` в `<basename>`.
  - **Извлечение**: только перечисленные `archive` через `zf.read(name)` (отсутствует — ошибка с именем).
    Имя берётся из манифеста, а не из zip, поэтому zip-slip невозможен. Сверить `sha256`, записать в
    `assets/third_party/.tmp-<name>/<path>`. После успеха по всем файлам: если есть старый
    `assets/third_party/<name>`, переименовать его в `.old-<name>`, затем `os.replace(tmp, final)` и
    `shutil.rmtree(.old-<name>)`. Ошибка на любом шаге удаляет `.tmp-<name>` и старый каталог не трогает.
    Так пакет публикуется атомарно.
- Кэш zip в `target/` уже игнорируется git. Скачанный zip не коммитится.
- Проверка (фиксируется в IMPL_SUMMARY):
  1. `python tools/fetch_assets.py --cache maw/tasks/in_progress/TASK-004/scratch/kenney` — код 0.
     Commercial zip в кэше игнорируется. В `assets/third_party/` появляются ровно 12 файлов и `manifest.ron`.
  2. Повторный запуск печатает три пропуска и возвращает код 0.
  3. `python tools/fetch_assets.py --check` — код 0.
  4. Негативы: пустой `assets/third_party/` без манифестных пакетов → `--check` код 1. Изменить байт в
     `light-square.glb` → `--check` код 1 с именем файла, после чего повторный fetch восстанавливает пакет.
     Положить `extra.glb` в пакет → `--check` код 1. Подложить в `--cache` переименованный commercial zip
     под именем roads → ошибка хэша архива.
  5. Паритет: `--validate-only` на shipped манифесте и на `valid_minimal.ron` даёт 0, на каждом `bad_*.ron`
     из шага 5 — 1.

### Шаг 5. Манифест-гейт в `gta_sim` (headless, класс: корректность воспроизводимости)

- `crates/gta_sim/Cargo.toml`, `[dev-dependencies]`: добавить `sha2 = "=0.10.9"`.
- `Cargo.lock`: сначала `cargo check -p gta_sim --tests --offline`. Если резолвер требует сеть, заменить
  файл на `maw/tasks/in_progress/TASK-004/scratch/Cargo.lock.task004`. Reviewer-2 проверил: его дифф с
  текущим HEAD `Cargo.lock` совпадает с `scratch/cargo_lock_diff.txt` и только добавляет 7 пакетов (sha2
  0.10.9, digest 0.10.7, block-buffer 0.10.4, crypto-common 0.1.7, generic-array 0.14.7, typenum 1.20.1,
  cpufeatures 0.2.17). Существующая `cpufeatures 0.3.1` получает в зависимостях явную версию. Исходники
  есть в registry.
- Фикстуры `crates/gta_sim/tests/fixtures/manifest/` (текстовые `.ron`, tracked). `valid_minimal.ron` —
  один пакет `city-kit-roads` с двумя файлами (`light-square.glb` и `License.txt`, хэши из шага 2). Каждый
  `bad_*.ron` — копия valid_minimal с одним дефектом:
  | файл | дефект | ожидание |
  |---|---|---|
  | `bad_unknown_field.ron` | лишнее поле `extra: "x"` в пакете | parse Err |
  | `bad_duplicate_field.ron` | `version` дважды | parse Err |
  | `bad_license.ron` | `license: MIT` | parse Err |
  | `bad_absolute_path.ron` | `path: "/light-square.glb"` | validate Err, содержит `path` |
  | `bad_parent_path.ron` | `path: "../light-square.glb"` | validate Err, содержит `path` |
  | `bad_duplicate_path.ron` | два файла с `path: "light-square.glb"` | validate Err, содержит `duplicate` |
  | `bad_hash.ron` | `sha256` из 63 символов | validate Err, содержит `sha256` |
  | `bad_license_file.ron` | `license_file: "LICENSE.md"` | validate Err, содержит `license_file` |
  | `bad_url.ron` | `url: "https://example.com/x.zip"` | validate Err, содержит `url` |
- `crates/gta_sim/tests/asset_manifest.rs` (новый; `mod common;` для `assets_root()`):
  - Хелпер `fixture(name) -> String`: `fs::read_to_string(CARGO_MANIFEST_DIR/tests/fixtures/manifest/<name>)`.
    Если файла нет — `panic!("GATE BROKEN: fixture {name} missing")`.
  - `manifest_fixtures_are_judged`: `valid_minimal.ron` → `ron::from_str::<ThirdPartyManifest>` Ok и
    `validate()` Ok. Это анти-тавтология: провал на bad-фикстурах не может идти от самой обвязки. Далее
    каждая строка таблицы должна дать ожидаемую ошибку, для validate-ошибок проверяется и подстрока.
  - `shipped_manifest_is_valid`: `load_config::<ThirdPartyManifest>(&assets_root(), THIRD_PARTY_MANIFEST)`
    (ошибка → `GATE BROKEN: ...`), `validate()` Ok, имена пакетов ровно
    `{city-kit-roads, city-kit-suburban, city-kit-industrial}`, в каждом пакете 4 файла.
  - `local_assets_match_manifest` проверяет файлы по трём состояниям:
    - ни одного каталога пакета нет → `eprintln!("SKIP file check: third-party packs not fetched; run python tools/fetch_assets.py")` и return. Это свежий клон.
    - есть часть пакетов → panic со списком отсутствующих (частичная установка — дефект).
    - есть все → для каждого пакета множество файлов на диске (рекурсивно, относительные пути через `/`)
      **равно** множеству `files[].path`. `format!("{:x}", Sha256::digest(bytes))` равен манифесту, иначе
      сообщение с пакетом, путём, ожидаемым и фактическим хэшем. **Байты** `license_file` (не
      `read_to_string`, файл в CP1252) содержат `b"Creative Commons Zero"` и `b"CC0"`. Под
      `assets/third_party/` нет записей, кроме `manifest.ron` и каталогов пакетов.
- Проверка: `cargo test -p gta_sim --test asset_manifest` зелёный в двух состояниях: без пакетов (SKIP
  виден в `--nocapture`) и после шага 4.

### Шаг 6. citygen: параметры (`crates/citygen/src/params.rs`, `assets/world/city.ron`)

- `RoadParams`: добавить `pub curb_height: f32`, в validate — `positive("roads.curb_height", ..)`.
- `CityParams`: добавить `pub massing: MassingParams` и `pub landmarks: LandmarkParams`
  (`#[derive(Deserialize, Clone, Debug)] #[serde(deny_unknown_fields)]`):
  ```rust
  pub struct MassingParams { pub setback_min_floors: u32, pub setback_tier_floors: u32,
                             pub setback_inset: f32, pub setback_min_half: f32 }
  pub struct LandmarkParams { pub tower_floors: u32, pub tower_footprint: f32, pub park_radius: f32 }
  ```
  В `validate()` добавить: `setback_min_floors ≥ 1`, `setback_tier_floors ≥ 1`, `positive(setback_inset)`,
  `positive(setback_min_half)`, `positive(tower_footprint)`, `positive(park_radius)`, а также
  `tower_floors as f32 × downtown.floor_height > max по 4 районам (floors.1 as f32 × floor_height)`.
  Сообщение: `"landmarks.tower_floors must make the tower the tallest building"`.
- `assets/world/city.ron`: в `roads: (...)` добавить `curb_height: 0.15`, в конец файла
  `massing: (setback_min_floors: 12, setback_tier_floors: 8, setback_inset: 3.0, setback_min_half: 6.0),`
  `landmarks: (tower_floors: 40, tower_footprint: 32.0, park_radius: 300.0),`.
  Проверка неравенства: 40 × 3.9 = 156 м > 30 × 3.9 = 117 м.
- `params.rs::validate_rejects_bad_params`: добавить случай `p.landmarks.tower_floors = 30` → ошибка
  содержит `landmarks.tower_floors` (117 не больше 117).
- Проверка: `cargo test -p citygen --lib`, `cargo test -p gta_sim --test config`.

### Шаг 7. citygen: layout, ориентиры, уступы, хэш

**7a. `crates/citygen/src/layout.rs`:**
- `BuildingKind`: добавить `Tower`.
- `Building`: добавить `pub upper_tiers: Vec<Tier>`;
  `#[derive(Clone, Copy, Debug)] pub struct Tier { pub bottom: f32, pub half_extents: Vec2 }`.
  `half_extents`/`height` здания остаются базовым футпринтом и полной высотой. Ярус `k` занимает высоты
  `[upper_tiers[k].bottom, upper_tiers[k+1].bottom или height)`, база — `[0, upper_tiers[0].bottom или height)`.
- `CityLayout`: добавить `pub landmarks: Landmarks`;
  `#[derive(Clone, Copy, Debug)] pub struct Landmarks { pub plaza: u32 /* block */, pub park: u32 /* block */, pub tower: u32 /* building */ }`.
- `GenError`: вариант `NoLandmarkCandidate { landmark: &'static str }` и ветка `Display`.
- `crates/citygen/tests/golden.rs::geometry_hash` использует `..layout.clone()`, правка не нужна.

**7b. `crates/citygen/src/geom.rs` / `lib.rs`:** сделать `centroid` `pub` и добавить в
`pub use geom::{centroid, contains_convex, convex_overlap, dist_point_segment};`. Он нужен `gta_sim` и
клиенту, дублировать формулу не надо.

**7c. `crates/citygen/src/lots.rs`:**
- Вынести из `mark_parks` правило защиты в `pub(crate) fn protected_blocks(blocks: &[Block], district_count: usize) -> Vec<Option<usize>>`,
  поведение не меняется. `mark_parks` вызывает его.
- `build` возвращает `Result<(Vec<Lot>, Vec<Building>, Landmarks), GenError>`. После `mark_parks`:
  `let (plaza, park) = landmarks::choose(params, districts, roads, blocks, &protected)?;`,
  `blocks[park].is_park = true; blocks[plaza].is_park = false;`. RNG-потоки per-block
  (`rng::stream(seed, PARKS, id)`), поэтому остальные кварталы не сдвигаются. В цикле квартал `plaza`
  не делится: лот один — весь `inner.clone()` с фронтажем как у корня — и в нём `fit_tower`. Индекс
  здания пишется в `Landmarks.tower`. Если `fit_tower` вернул None → `Err(NoLandmarkCandidate { landmark: "tower" })`.
- Вынести цикл сжатия из `fit_building` в
  `fn shrink_to_fit(shrunk: &[Vec2], center: Vec2, u: Vec2, half: Vec2) -> Option<Vec2>`. Константы те
  же: `FIT_SHRINK`, `FIT_STEPS`, `FIT_EPS`.
- `fit_tower(params, lot_id, shape) -> Option<Building>`: `u` от `longest_frontage`, полигон `inset` на
  `downtown.setback`, центр = `geom::centroid(&shrunk)`, `half = Vec2::splat(tower_footprint / 2.0)`,
  затем `shrink_to_fit`. `height = tower_floors × downtown.floor_height`, `kind: Tower`,
  `upper_tiers = tiers(&params.massing, tower_floors, downtown.floor_height, half)`.
- В `fit_building`, после расчёта `floors`:
  `upper_tiers: tiers(&params.massing, floors, district.floor_height, half)`.
- `fn tiers(m: &MassingParams, floors: u32, floor_h: f32, base: Vec2) -> Vec<Tier>`: при
  `floors < m.setback_min_floors` — пусто. Иначе для `k = 1..` пока `k·T < floors` (`T = setback_tier_floors`):
  `next = prev − Vec2::splat(inset)`. Если `next.x < min_half || next.y < min_half` — стоп, иначе push
  `Tier { bottom: (k·T) as f32 × floor_h, half_extents: next }`, `prev = next`.
  - **Пример 1** (downtown, 30 этажей, 3.9 м, база (15, 12)): k=1 → (12, 9) с низом 31.2; k=2 → (9, 6) с
    низом 62.4 (6 ≥ 6 проходит); k=3 → (6, 3), 3 < 6, стоп. Итого 2 яруса, верх 117.
  - **Пример 2** (башня 40 этажей, (16, 16)): (13,13)@31.2, (10,10)@62.4, (7,7)@93.6; k=4 → (4,4), стоп.
    Крыша 14 × 14 м на высоте 156.
  - **Пример 3** (commercial, 8 этажей): 8 < 12, ярусов нет.
- `lib.rs`: `mod landmarks;`, `let (lots, mut buildings, landmarks) = lots::build(...)?;`, поле
  `landmarks` в `CityLayout`.

**7d. `crates/citygen/src/landmarks.rs`** (новый):
`pub(crate) fn choose(params: &CityParams, districts: &[District], roads: &RoadGraph, blocks: &[Block], protected: &[Option<usize>]) -> Result<(usize, usize), GenError>`
- Площадь. Кандидаты: кварталы с непустым `inner`, у которых есть сторона `k` с классом
  `roads.edges[sides[k]].class != Alley` и длиной `inner[k]→inner[k+1]` ≥ `min_frontage` района квартала.
  Выбирается минимальный `|centroid(inner)|`, при равенстве — меньший id. Нет кандидатов →
  `NoLandmarkCandidate { landmark: "plaza" }`. Это правило держит `lots_face_a_street` зелёным для
  единственного лота площади.
- Парк. Кандидаты: непустой `inner`, `id ≠ plaza`, `protected[district] != Some(id)`,
  `|centroid(inner)| ≤ park_radius`. Выбирается максимальная `signed_area(inner)`, при равенстве —
  меньший id. Нет кандидатов → `NoLandmarkCandidate { landmark: "park" }`.
- POI (`pois.rs:56`) выбираются только среди `Generic`, так что башня не станет больницей/участком.

**7e. `crates/citygen/src/hash.rs`:** `HASH_SCHEMA_VERSION = 2`; `building_kind(Tower) = (4, 0)`. У
каждого здания после `height` пишется `len(upper_tiers)`, затем для каждого яруса `f32(bottom)` и
`vec2(half_extents)`. После `gang_districts` пишутся `u32(plaza)`, `u32(park)`, `u32(tower)`.

- Проверка: `cargo build -p citygen`.

### Шаг 8. citygen: гейты и re-bless (класс: корректность, детерминизм)

`crates/citygen/tests/properties.rs` гоняет все `layouts()`: SEEDS {1,2,42} ∪ SWEEP 0..32, всего 33 layout.
- `landmarks_exist`: `buildings[tower].kind == Tower`, и это единственный `Tower`. Его `height` строго
  больше высоты любого другого здания. У квартала `plaza` ровно один лот, и на нём ровно одно здание
  (`tower`). `blocks[park].is_park`, `park != plaza`,
  `|centroid(blocks[park].inner)| ≤ params.landmarks.park_radius`.
- `setback_tiers_nested`: у каждого здания `bottom` строго растут, кратны `floor_height` района (у башни —
  downtown) с допуском 1e-3 и меньше `height`. `half_extents[k] == prev − inset` (допуск 1e-4) и
  `≥ setback_min_half`. У зданий с `round(height / floor_h) < setback_min_floors` ярусов нет. На seed 1
  хотя бы у одного здания кроме башни ярусы есть.
- Существующие `buildings_inside_lots`, `lots_face_a_street`, `pois_exist`, `player_spawn_on_sidewalk`,
  `gang_guarantee_recolors` не меняются и должны остаться зелёными.
- **Re-bless golden** по процедуре из шапки `tests/golden_hashes.txt`. Сначала все properties зелёные.
  Затем `cargo test -p citygen --test golden -- --ignored bless_print_golden --nocapture` и три строки
  вставляются руками. В IMPL_SUMMARY перечислить причины смены хэша: схема v2, ярусы, башня, выбор
  площади/парка.
- Проверка: `cargo test -p citygen` целиком зелёный;
  `cargo test -p citygen --release --test perf -- --ignored --nocapture` < 2 с.

### Шаг 9. gta_sim: бордюр, ярусы, спавн, ориентиры (`crates/gta_sim/src/world/city.rs`, `world/mod.rs`)

- Новые типы в `city.rs`:
  ```rust
  #[derive(Component)] pub struct CityBlock;
  /// Landmark points of the generated city; read by QA over BRP.
  #[derive(Resource, Reflect)] #[reflect(Resource)]
  pub struct CityLandmarks { pub tower_roof: Vec3, pub plaza_center: Vec3, pub park_center: Vec3 }
  ```
- `apply_city_generation`: `let curb = params.0.roads.curb_height;`;
  `PlayerSpawn(Vec3::new(spawn.x, curb, spawn.y))`. Это верно: `player_spawn` — середина стороны плюс
  `walk_offset = half_carriageway + sidewalk/2`, а `curb` — inset на `half_carriageway`
  (`roads.rs:166-177`, `graphs.rs:137-155`), значит спавн внутри `curb`-полигона.
  Далее вызывается `spawn_blocks(&mut commands, &layout, curb)`. Он возвращает `Result<(), String>`;
  при `Err` → `error!("city generation failed: {err}")`, `exit.write(AppExit::error())`, return, как у
  `GenError`. Затем вставляется
  `CityLandmarks { tower_roof: (t.center.x, t.height, t.center.y), plaza_center: (centroid(plaza.inner).x, curb, .y), park_center: (centroid(park.inner), curb) }`
  через `citygen::centroid`.
- `spawn_blocks`: для каждого квартала с `curb.len() >= 3` точки `(p.x, -GROUND_SLAB_THICKNESS, p.y)` и
  `(p.x, curb, p.y)` передаются в `Collider::convex_hull(points)`. `None` → `Err(format!("block {id}: degenerate curb polygon"))`,
  молчаливого пропуска нет. `curb` выпуклый по построению (`geom::inset` отвергает невыпуклые вход и
  выход, `geom.rs:31,50`; супер-кварталы сливаются только при `is_convex`, `roads.rs:127`), так что
  оболочка дорогу не заливает. Спавн: `(CityBlock, RigidBody::Static, collider, Transform::IDENTITY)`.
  Static-static перекрытия с плитой земли и зданиями контактов не дают.
- `spawn_buildings`: при пустом `upper_tiers` — как сейчас (`Collider::cuboid`). Иначе
  `Collider::compound(parts)`. Части — база и каждый ярус:
  `(Vec3::new(0.0, mid − height/2, 0.0), Quat::IDENTITY, Collider::cuboid(2·hx, top − bottom, 2·hy))`,
  где `mid = (bottom + top)/2`. `Transform` не меняется: центр на `height/2`, поворот
  `from_rotation_y(atan2(-u.y, u.x))` переводит локальную X в `u`, локальную Z в `v = u.perp()`.
  `CityBuilding.size` остаётся базовым. **Пример** (пример 1, height 117, центр 58.5): база 30 × 31.2 × 24,
  local y = 15.6 − 58.5 = −42.9; ярус 1 24 × 31.2 × 18, y = 46.8 − 58.5 = −11.7; ярус 2 18 × 54.6 × 12,
  y = 89.7 − 58.5 = 31.2.
- `world/mod.rs`: экспортировать `CityBlock`, `CityLandmarks` и из citygen `Tier`, `Landmarks`,
  `centroid`. В ветке `City` добавить `.register_type::<CityLandmarks>()`.
- Проверка: `cargo build -p gta_sim`; `python tools/qa/tree_check.py` зелёный.

### Шаг 10. gta_sim: тесты (ре-анкор T2 и новые)

- `tests/common/mod.rs::settle`: `expected = app.world().resource::<PlayerSpawn>().0.y + float_height`.
  Для TestArea `PlayerSpawn = ZERO`, получается 1.05, как раньше. Для города 0.15 + 1.05 = 1.20. Это гейт
  бордюра: без `CityBlock` игрок падает на 1.05, |1.05 − 1.20| = 0.15 > 0.05, RED. Импорт `PlayerSpawn`
  из `gta_sim::world`.
- `tests/city.rs::one_static_collider_per_building` (ре-анкор). Сопоставление по центру — как сейчас. Для
  здания без ярусов — прежняя cuboid-проверка. Для здания с ярусами —
  `collider.shape().as_compound().expect(..)`, `shapes().len() == 1 + upper_tiers.len()`; у каждой части
  `as_cuboid()` half extents и `translation.y` равны ожидаемым из layout (допуск 1e-3). Проверка
  поворота прежняя. Число сущностей `CityBuilding` == `buildings.len()` (один коллайдер на здание —
  T2-закон сохранён).
- Новый `landmarks_resource_matches_layout` (seed 1): `tower_roof.y` == max высоты зданий, xz == центр
  `buildings[landmarks.tower]`; `park_center.y == curb_height` и xz == `centroid(blocks[park].inner)`;
  число `CityBlock` == числу кварталов с `curb.len() >= 3`.
- `runtime_hash_matches_golden`, `edge_wall_stops_player` (y 1.05 в поле за городом, без бордюра),
  `player_spawns_on_sidewalk_and_stands` проходят без правок.
- Проверка: `cargo test -p gta_sim` целиком зелёный.

### Шаг 11. Клиент: конфиг рендера (`src/visuals/config.rs` новый, `assets/world/render.ron`)

Перенести `RenderConfig` и `DistrictColors` из `visuals/mod.rs` в `config.rs` по образцу
`camera/config.rs`: `#[derive(Resource, Deserialize, Clone)]`, `deny_unknown_fields` на всех структурах,
поля `pub(super)`, `pub const RENDER_CONFIG`. Итоговый `render.ron`, все новые числа живут здесь:
```ron
(
    ambient_brightness: 300.0,
    sun_illuminance: 10000.0,
    sun_pitch_deg: -45.836624,
    sun_yaw_deg: -28.64789,
    shadows: (map_size: 4096, cascades: 4, first_cascade_far_bound: 20.0, maximum_distance: 350.0),
    chunk_size: 128.0,
    spawn_budget: (chunks_per_frame: 10, props_per_frame: 500),
    fog: (color: (0.74, 0.82, 0.92), start: 250.0, end: 450.0, sun_glow: (1.0, 0.95, 0.85, 0.5), sun_glow_exponent: 20.0),
    sky: (zenith: (0.32, 0.52, 0.86), radius: 900.0, gradient_exponent: 0.6, sectors: 32, stacks: 16),
    district_colors: (downtown: (0.62, 0.64, 0.70), commercial: (0.78, 0.70, 0.55),
                      residential: (0.80, 0.62, 0.52), industrial: (0.55, 0.55, 0.50)),
    tower_color: (0.55, 0.62, 0.72),
    hospital_color: (0.95, 0.95, 0.95),
    police_color: (0.20, 0.30, 0.75),
    gang_hq_color: (0.75, 0.20, 0.55),
    road_color: (0.18, 0.18, 0.20),
    sidewalk_color: (0.60, 0.60, 0.58),
    park_color: (0.30, 0.55, 0.25),
    curb_color: (0.70, 0.70, 0.68),
    plaza_color: (0.72, 0.68, 0.60),
    lot_colors: (downtown: (0.55, 0.55, 0.56), commercial: (0.60, 0.58, 0.55), residential: (0.38, 0.58, 0.30), industrial: (0.45, 0.43, 0.40)),
    surface_layer_step: 0.02,
    markings: (color: (0.92, 0.92, 0.88), center_color: (0.95, 0.78, 0.20), width: 0.15, dash: 3.0, gap: 6.0,
               crosswalk_depth: 3.0, crosswalk_stripe: 0.5, crosswalk_gap: 0.5),
    facade: (bay_width: 3.2, window_width: 0.55, window_height: 0.55, window_sill: 0.25,
             glass_color: (0.18, 0.24, 0.32), glass_roughness: 0.12, wall_roughness: 0.85),
    props: (
        visibility_range: 120.0, fade: 10.0, roughness: 0.8,
        lamp: (model: "third_party/city-kit-roads/light-square.glb", texture: "third_party/city-kit-roads/Textures/colormap.png", scale: 10.0),
        traffic_light: (model: "third_party/city-kit-roads/traffic-light.glb", texture: "third_party/city-kit-roads/Textures/colormap.png", scale: 10.0),
        tree_large: (model: "third_party/city-kit-suburban/tree-large.glb", texture: "third_party/city-kit-suburban/Textures/colormap.png", scale: 10.0),
        tree_small: (model: "third_party/city-kit-suburban/tree-small.glb", texture: "third_party/city-kit-suburban/Textures/colormap.png", scale: 10.0),
        container_a: (model: "third_party/city-kit-industrial/shipping-container-a.glb", texture: "third_party/city-kit-industrial/Textures/colormap.png", scale: 2.0),
        container_c: (model: "third_party/city-kit-industrial/shipping-container-c.glb", texture: "third_party/city-kit-industrial/Textures/colormap.png", scale: 2.0),
        container_raw_size: (1.38, 3.046),
        lamp_spacing: 24.0, lamp_corner_clearance: 4.0, lamp_curb_offset: 0.6, tree_curb_offset: 1.2,
        traffic_light_offset: 1.0, park_tree_spacing: 14.0, park_tree_jitter: 3.0, park_tree_margin: 3.0,
        container_gap: 1.0,
    ),
)
```
Масштабы получены из сырых габаритов: фонарь 0.600 × 10 = 6.0 м, светофор 5.15 м, деревья 7.7 и 5.7 м.
Контейнер: (1.38, 3.046) × 2 = 2.76 × 6.09 м в плане, 2.58 м в высоту (20-футовый 6.06 × 2.44 × 2.59).
`container_raw_size` — (ширина по X, длина по Z) сырой модели в её единицах.

`impl RenderConfig { pub fn validate(&self) -> Result<(), String> }`. Все f32 конечны. `chunk_size > 0`.
`spawn_budget.*_per_frame ≥ 1`. `shadows.map_size` — степень двойки, `cascades ≥ 1`,
`0 < first_cascade_far_bound < maximum_distance`. `0 < fog.start < fog.end < sky.radius <
PerspectiveProjection::default().far` (камера использует `..default()`, `src/camera/mod.rs:38-41`;
far = 1000). `sky.sectors ≥ 3`, `sky.stacks ≥ 2`. `0 ≤ props.fade < props.visibility_range`. Все `scale`,
`*_spacing`, `bay_width`, `dash`, `gap`, `width`, `crosswalk_*`, `container_raw_size.*`, `container_gap`
больше 0. `window_width`, `window_height` в (0, 1); `window_sill ≥ 0`, `window_sill + window_height < 1`;
roughness в [0, 1]. Сообщение называет поле.
- Проверка: `cargo build`.

### Шаг 12. Клиент: startup preflight (`src/main.rs`)

После загрузки `render_config`:
1. `render_config.validate()`. При `Err(message)`:
   `eprintln!("{}: {message}", root.path(RENDER_CONFIG).display())` и `return AppExit::error()`.
2. `let manifest = load_config::<ThirdPartyManifest>(&root, THIRD_PARTY_MANIFEST)` + `validate()`; ошибка
   уходит в `eprintln!` и `AppExit::error()`.
3. Каждый `model`/`texture` из `render_config.props` обязан проходить `manifest.contains_asset(path)`.
   Иначе: `"{RENDER_CONFIG}: prop asset {path} is not listed in {THIRD_PARTY_MANIFEST}"` и выход с ошибкой.
4. `manifest.missing_files(&root)` не пуст → для каждого
   `eprintln!("missing third-party asset {}; run `python tools/fetch_assets.py`", path.display())` и
   `AppExit::error()`.
   Хэши при старте не считаются: их держит гейт шага 5 и `fetch_assets.py --check`. Для этого пришлось
   бы тащить `sha2` в клиент.
- Всё это выполняется до `app.run()`. Если нужен геттер для списка путей пропов, это
  `RenderConfig::prop_asset_paths(&self) -> impl Iterator<Item = &str>` в `config.rs`.
- Проверка: временно переименовать `assets/third_party/city-kit-roads` → `cargo run` завершается с кодом
  ≠ 0 и сообщением про `tools/fetch_assets.py`, окно не держит. Вернуть каталог.

### Шаг 13. Клиент: фасадный материал (`src/visuals/facade.rs`, `assets/shaders/facade.wgsl`)

- `facade.rs`. Импорты как в примере v0.19.1: `bevy::pbr::{ExtendedMaterial, MaterialExtension}`,
  `bevy::render::render_resource::{AsBindGroup, ShaderType}`, `bevy::shader::ShaderRef`.
  ```rust
  const FACADE_SHADER: &str = "shaders/facade.wgsl";
  pub type FacadeMaterial = ExtendedMaterial<StandardMaterial, FacadeExtension>;
  #[derive(Asset, AsBindGroup, Reflect, Debug, Clone)]
  pub struct FacadeExtension { #[uniform(100)] pub params: FacadeParams }
  #[derive(ShaderType, Reflect, Debug, Clone)]
  pub struct FacadeParams { pub glass: Vec4 /* linear rgb, 1 */, pub window: Vec4 /* width, height, sill, glass_roughness */ }
  impl MaterialExtension for FacadeExtension { fn fragment_shader() -> ShaderRef { FACADE_SHADER.into() } }
  pub fn facade_material(config: &RenderConfig) -> FacadeMaterial
  ```
  База: `StandardMaterial { base_color: Color::WHITE, perceptual_roughness: facade.wall_roughness, ..default() }`.
  `glass` = `Color::srgb(glass_color).to_linear()` в `Vec4` с w = 1. Uniform — два `Vec4`, 32 байта,
  выравнивание 16, webgl-паддинг не нужен.
- `facade.wgsl`: скелет `extended_material.wgsl` из v0.19.1 — те же импорты и ветки
  `PREPASS_PIPELINE`/forward. Структура `struct FacadeParams { glass: vec4<f32>, window: vec4<f32> }`,
  `@group(#{MATERIAL_BIND_GROUP}) @binding(100) var<uniform> facade: FacadeParams;`. Между
  `pbr_input_from_standard_material` и `alpha_discard` вставить:
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
  Forward-ветка: `apply_pbr_lighting`, затем `main_pass_post_lighting_processing` — туман применяется.
  Модификаций после освещения, как в примере, нет. Prepass-шейдер не переопределяется: окна не меняют
  глубину и альфу.
- Проверка: `cargo build`. Корректность шейдера видна только в окне: шаг 19 и владелец.

### Шаг 14. Клиент: геометрия чанков (`src/visuals/city_mesh.rs`, новый, чистые функции)

`pub(super) struct ChunkMesh { pub coord: UVec2, pub mesh: Mesh }`;
`pub(super) fn build_city_meshes(layout: &CityLayout, params: &CityParams, config: &RenderConfig) -> Vec<ChunkMesh>`.
Результат отсортирован по `coord` и содержит ровно `n²` элементов. Аккумулятор
`struct MeshBuilder { positions, normals, uvs, colors, indices }` превращается в `Mesh` с
`ATTRIBUTE_POSITION/NORMAL/UV_0/COLOR`, `Indices::U32`, `RenderAssetUsages::default()` (MAIN_WORLD |
RENDER_WORLD — меш остаётся в main world, гейт его читает). Цвет вершины =
`Color::srgb(r,g,b).to_linear().to_f32_array()`: вершинные цвета линейны, `render.ron` хранит sRGB. UV у
всего, что не фасад, — `[-1.0, -1.0]`. Позиции задаются в мировых координатах, `Transform::IDENTITY`.

- **Сетка.** `n = (params.size / config.chunk_size).ceil() as u32`;
  `chunk_of(p: Vec2) -> UVec2`: `i = ((p.x + size/2) / chunk).floor()` с clamp в `[0, n-1]`, по y так же.
  Асфальт чанка `i`: `lo = -size/2 + i·chunk`, `hi = lo + chunk`; при `i == 0` `lo = -ground/2`, при
  `i == n-1` `hi = ground/2`. **Примеры** (size 1200, chunk 128, ground 1400): `n = ceil(9.375) = 10`,
  всего 100 чанков. (0,0) → floor(600/128) = 4; (−600,−600) → 0; (620,0) → floor(1220/128) = 9;
  (700,0) → floor(10.16) = 10, clamp даёт 9. Чанк 9 по x — [552, 700], чанк 0 — [−700, −472], объединение —
  [−700, 700], без дыр.
- **Обход граней** (одно правило на файл). Горизонтальный CCW-полигон в (x, y) (положительная площадь)
  выводится веером `(0, i+1, i)` с нормалью +Y, как `flat_mesh` в текущем `src/visuals/city.rs:115-126`.
  Вертикальный квад по ребру `a→b` CCW-контура (внутренность слева, `(b-a).perp()`), низ `y0`, верх `y1`:
  вершины `A0, B0, B1, A1`, индексы `[A0, B1, B0, A0, A1, B1]`, нормаль `-(b-a).perp().normalize()` в
  мире `(x, 0, y)`. Проверено на трёх направлениях (правая ось камеры = forward × up):
  1. `a=(0,0) → b=(10,0)`, наружу −Z. Камера в z = −5 смотрит на +Z, экранная ось вправо −X.
     A0 (0,0), B1 (−10,h), B0 (−10,0): (−10,h)×(−10,0) = 10h > 0, CCW, лицевая.
  2. `a=(0,0) → b=(0,10)`, наружу +X. Камера в x = +5 смотрит на −X, вправо −Z. Экранные координаты те
     же, лицевая.
  3. `a=(10,0) → b=(0,0)`, наружу +Z. Камера в z = +5 смотрит на −Z, вправо +X. A0 (10,0), B1 (0,h),
     B0 (0,0): (−10,h)×(−10,0) = 10h > 0, лицевая.
- **Асфальт**: на каждый чанк квад `[lo, hi]²` на y = 0 цветом `road_color`.
- **Квартал** (`curb.len() >= 3`, чанк по `citygen::centroid(curb)`): стенки бордюра по каждому ребру
  `curb` (y от 0 до `curb_height`, `curb_color`). Если `inner` пуст, верх — весь `curb` на `curb_height`
  цветом `sidewalk_color`. Иначе кольцо тротуара трапециями `curb[k], curb[k+1], inner[k+1], inner[k]`
  (CCW; индексы `inset` совпадают, оба полигона построены из одного полигона с одними `sides`,
  `roads.rs:166-181`) и `inner` на той же высоте. Цвет `inner`: `park_color`, если `is_park`;
  `plaza_color` для `landmarks.plaza`; иначе `lot_colors[district]`. Всё на одной высоте без перекрытия,
  поэтому z-fighting нет.
- **Здание** (чанк по `center`). Углы `p0 = c − u·hx − v·hy`, `p1 = c + u·hx − v·hy`,
  `p2 = c + u·hx + v·hy`, `p3 = c − u·hx + v·hy` (CCW, `v = u.perp()`). Для базы и каждого яруса: 4 стены
  по правилу квада (y от низа до верха яруса) и крыша веером на верхе. Цвет: `Tower → tower_color`,
  `Hospital`, `PoliceStation`, `GangHq`, иначе `district_colors[district]`. UV стены:
  `bays = max(1, round(len / bay_width))`, `u = t·bays` (t = 0 у `a`, 1 у `b`), `v = y / floor_h`
  (floor_h района, у башни downtown), так что окна совпадают с этажами, а низ яруса кратен этажу. UV крыш
  −1. Стены начинаются с y = 0, часть ниже `curb_height` скрыта призмой квартала.
- **Разметка** (рёбра не Alley). `trim[node] = max half_carriageway` инцидентных рёбер (как
  `citygen/src/graphs.rs:83-88`). Рабочий отрезок ребра — от `a + d·(trim_a + crosswalk_depth)` до
  `b − d·(trim_b + crosswalk_depth)`, при длине ≤ 0 ребро пропускается. Квады на высоте
  `surface_layer_step`, чанк по центру квада. Avenue — сплошная осевая `center_color`. Street — пунктир
  `color` (`dash`/`gap`, центрирован). Разделители полос пунктиром на смещениях `±k·lane_width`,
  `k = 1..lanes_per_direction−1`. Зебра на обоих концах: полосы вдоль `d` длиной `crosswalk_depth`,
  шириной `crosswalk_stripe`, шаг `stripe + gap`, поперёк проезжей части `[−half, half]`, сразу за
  `trim` узла.
- Файл < 750 строк; если выходит больше, разметка уходит в `src/visuals/city_markings.rs`.
- Проверка: `cargo build`; unit-тест в файле `grid_covers_ground`: при size 1200, chunk 128,
  ground 1400 объединение асфальта = [−700, 700]², `n = 10`.

### Шаг 15. Клиент: пропы (`src/visuals/props.rs`, новый)

- `#[derive(Clone, Copy)] pub(super) enum PropKind { Lamp, TrafficLight, TreeLarge, TreeSmall, ContainerA, ContainerC }`,
  `#[derive(Clone, Copy)] pub(super) struct PropPlacement { pub kind: PropKind, pub position: Vec3, pub yaw: f32 }`.
- `pub(super) fn place_props(layout, params, config) -> Vec<PropPlacement>` — чистая и детерминированная.
  Случайность — локальный `splitmix64(seed ^ block ^ i ^ j)`, без RNG-крейтов. Все пропы стоят на
  `y = curb_height`.
  1. **Фонари.** Каждое ребро `curb[k]→curb[k+1]` квартала, у которого `sides[k]` ≠ Alley. Длина `L`,
     `usable = L − 2·clearance`; при `usable < 0` фонарей нет. `count = floor(usable/spacing) + 1`,
     `t_j = clearance + (usable − (count−1)·spacing)/2 + j·spacing`, точка
     `a + d·t_j + inward·lamp_curb_offset` (`inward = d.perp()`). Плечо модели смотрит в −Z, а нужно
     наружу, к дороге: `o = −inward`, в мире `(o.x, 0, o.y)`. `yaw = atan2(−o.x, −o.z)`, потому что
     `R_y(θ)·(0,0,−1) = (−sinθ, 0, −cosθ)`. Примеры: `o=(0,0,−1)` → θ = 0 → −Z ✓; `o=(1,0,0)` → θ = −90°
     → (1,0,0) ✓; `o=(0,0,1)` → θ = 180° → (0,0,1) ✓. Расстановка: `L=80, clearance 4, spacing 24` →
     `usable 72`, `count 4`, `t = 4, 28, 52, 76`.
  2. **Деревья вдоль улиц.** Те же рёбра у кварталов района Residential и у парков. По одному дереву в
     середине каждого промежутка между соседними фонарями (в примере `t = 16, 40, 64`) со смещением
     `inward·tree_curb_offset`. Крупное или мелкое — по чётности хэша.
  3. **Деревья в парках** (`is_park`, непустой `inner`). Сетка по мировым осям с шагом
     `park_tree_spacing` и сдвигом хэшем в `±park_tree_jitter`. Остаются точки, у которых для каждой
     стороны `inner` выполнено `(p − a)·inward ≥ park_tree_margin`.
  4. **Светофоры.** Вершина `k` `curb`, у которой `sides[k−1]` и `sides[k]` — Avenue. Точка
     `curb[k] + bisector·traffic_light_offset`, где `bisector = normalize(inward_{k−1} + inward_k)`. Модель
     повёрнута −Z к перекрёстку: `o = −bisector`, формула yaw та же. Ориентацию головы проверяет владелец на
     скриншоте.
  5. **Контейнеры.** Здания `Generic` района Industrial, тыльная сторона `+v` (фронтаж на `−v`, потому что
     `v` смотрит внутрь лота, `lots.rs:150-151`). `w = container_raw_size.0 × scale`,
     `len = container_raw_size.1 × scale`. Ряд вдоль `u` с центром `c + v·(hy + container_gap + w/2)`,
     `count = floor(2·hx / (len + gap))`, шаг `len + gap`. Контейнер ставится, только если все 4 угла
     внутри полигона лота (`citygen::contains_convex(lot, corner, 0.0)`). Длинная ось модели (Z) идёт
     вдоль `u`: `R_y(θ)·(0,0,1) = (sinθ, 0, cosθ)` → `θ = atan2(u.x, u.y)`. Примеры: u=(1,0) → 90° → (1,0,0) ✓;
     u=(0,1) → 0 → (0,0,1) ✓; u=(−1,0) → −90° → (−1,0,0) ✓. A или C — по чётности хэша.
- `#[derive(Resource)] pub(super) struct PropAssets { meshes: [Handle<Mesh>; 6], materials: [Handle<StandardMaterial>; 6], scales: [f32; 6] }`.
  `Startup`-система `load_prop_assets`:
  `asset_server.load::<Mesh>(GltfAssetLabel::Primitive { mesh: 0, primitive: 0 }.from_asset(model.clone()))`.
  На каждую уникальную текстуру (HashMap по пути) один
  `StandardMaterial { base_color_texture: Some(asset_server.load(texture.clone())), perceptual_roughness: props.roughness, ..default() }`,
  всего 3. Это та же текстура, на которую ссылается GLB.
- `#[derive(Component, Reflect)] #[reflect(Component)] pub struct CityProp;`. Бандл спавна (шаг 16):
  `(CityProp, Mesh3d, MeshMaterial3d<StandardMaterial>, Transform::from_translation(pos).with_rotation(Quat::from_rotation_y(yaw)).with_scale(Vec3::splat(scale)), VisibilityRange { start_margin: 0.0..0.0, end_margin: (range − fade)..range, use_aabb: false })`
  (`bevy::camera::visibility::VisibilityRange`). `VisibilityRange` висит на самой mesh-сущности: он не
  наследуется, поэтому `SceneRoot`/`WorldAssetRoot` не используются. Коллайдеров нет (визуал до T14).
  Оценка: около 2400 фонарей, 1200 деревьев вдоль улиц, 400 в парках, 300 контейнеров, 100 светофоров.
- Проверка: `cargo build`; unit-тесты в файле на три примера yaw фонаря и контейнера (числа выше) и на
  расстановку `L=80 → t = 4, 28, 52, 76`.

### Шаг 16. Клиент: плагин города (`src/visuals/city.rs`, переписать)

- Удалить `visualize_city_building`, `spawn_city_surfaces`, `CityPalette`, `setup_city_palette`,
  `flat_mesh`. Их логика переезжает в `city_mesh.rs`. После этого никакой код не вешает `Mesh3d` на
  `CityBuilding`.
- Типы:
  ```rust
  #[derive(Component, Reflect)] #[reflect(Component)] pub struct CityChunk { pub coord: UVec2 }
  #[derive(Resource)] struct CityMeshTask(Task<(Vec<ChunkMesh>, Vec<PropPlacement>)>);
  #[derive(Resource)] struct PendingCitySpawn { chunks: Vec<ChunkMesh>, props: Vec<PropPlacement> }
  #[derive(Resource)] struct CityFacade(Handle<FacadeMaterial>);
  ```
- `CityVisualsPlugin::build`: `register_type::<CityChunk>().register_type::<CityProp>()`.
  `Startup`: `create_facade_material` (`Assets<FacadeMaterial>::add(facade_material(&config))` → `CityFacade`)
  и `load_prop_assets`. `OnEnter(GameState::Playing)`: `start_city_mesh_build` клонирует `City.0`,
  `CityParamsRes.0` и `RenderConfig`, затем
  `AsyncComputeTaskPool::get().spawn(async move { (build_city_meshes(..), place_props(..)) })`.
  `Update` (две системы, `.chain()`):
  - `poll_city_mesh_build.run_if(resource_exists::<CityMeshTask>)`:
    `let Some((chunks, props)) = block_on(poll_once(&mut task.0)) else { return };`, удалить
    `CityMeshTask`, вставить `PendingCitySpawn { chunks, props }`.
  - `apply_city_spawn.run_if(resource_exists::<PendingCitySpawn>)`: за кадр снимать до
    `spawn_budget.chunks_per_frame` чанков → `(CityChunk { coord }, Mesh3d(meshes.add(mesh)), MeshMaterial3d(facade.0.clone()), Transform::IDENTITY)`
    и до `spawn_budget.props_per_frame` пропов → бандл шага 15. Когда оба списка пусты — удалить ресурс.
  `run_if(resource_exists)` здесь описывает жизненный цикл задачи, это не гейтинг по состоянию: он уже
  на `OnEnter`. Паттерн тот же, что `gta_sim/src/world/city.rs:47-88`.
- Клон `CityLayout` в задачу делается один раз за игру, порядка мегабайта. Записать в Risk.
- Проверка: `cargo build`, `cargo clippy -- -D warnings`.

### Шаг 17. Клиент: свет, небо, туман, сборка (`src/visuals/mod.rs`, `src/visuals/sky.rs`)

- `mod.rs`: `mod config; mod facade; mod city_mesh; mod props; mod sky; mod city; #[cfg(test)] mod city_gate;`,
  `pub use config::{RENDER_CONFIG, RenderConfig};` (импорт в `main.rs` не меняется). `VisualsPlugin::build`:
  как сейчас, плюс `.insert_resource(DirectionalLightShadowMap { size: shadows.map_size })`,
  `.insert_resource(ClearColor(fog_color))` и
  `.add_plugins((MaterialPlugin::<FacadeMaterial>::default(), sky::SkyPlugin, city::CityVisualsPlugin))`.
  `MaterialPlugin` добавляется только здесь, не в `CityVisualsPlugin`, иначе headless-гейт получит
  рендер-зависимость. `spawn_light`: на сущность солнца добавить
  `CascadeShadowConfigBuilder { num_cascades, first_cascade_far_bound, maximum_distance, ..default() }.build()`
  (`bevy::light::CascadeShadowConfigBuilder`). `visualize_block` и `visualize_character` не трогать.
  Вся геометрия города живёт только в `CityVisualsPlugin` — на этом правиле держится гейт шага 18.
- `sky.rs` (новый), `SkyPlugin`:
  - наблюдатель `On<Add, Camera3d>` вставляет `DistanceFog { color: fog.color, directional_light_color: Color::srgba(sun_glow), directional_light_exponent: sun_glow_exponent, falloff: FogFalloff::Linear { start, end } }`;
  - `Startup` спавнит `SkyDome`: `Sphere::new(radius).mesh().uv(sectors, stacks)` с `ATTRIBUTE_COLOR` на
    каждую вершину, `mix(horizon = fog.color, zenith, clamp(y/radius, 0, 1)^gradient_exponent)` в
    линейном пространстве. Материал
    `StandardMaterial { base_color: WHITE, unlit: true, fog_enabled: false, cull_mode: None, ..default() }`,
    плюс `NotShadowCaster`;
  - `Update`-система `follow_camera` копирует `translation` камеры в купол. Лаг в один кадр на радиусе
    900 м не виден.
- Проверка: `cargo build`, `cargo clippy -- -D warnings`.

### Шаг 18. Клиент: headless гейт слияния (`src/visuals/city_gate.rs`, `#[cfg(test)]`)

Класс: корректность, бюджет кадра. Харнесс `city_visuals_app(seed) -> App`:
`MinimalPlugins + TransformPlugin + AssetPlugin::default() + StatesPlugin`,
`TimeUpdateStrategy::FixedTimesteps(1)`, `init_asset::<Mesh>()`, `init_asset::<StandardMaterial>()`,
`init_asset::<Image>()`, `init_asset::<FacadeMaterial>()`. Это заглушки рендер-плагинов: без них
`AssetServer::load` паникует "asset type has not been initialized"
(`bevy_asset-0.19.1/src/server/info.rs:817-823`). Далее `RenderConfig` из
`ConfigRoot(CARGO_MANIFEST_DIR/assets)` через `load_config` + `validate` (ошибка → `GATE BROKEN`),
`compose_sim(.., WorldSource::City { seed })` — production-путь sim — и
`add_plugins(CityVisualsPlugin)` — production-плагин города целиком. Затем
`app.finish(); app.cleanup();` и `update()` в цикле с `sleep(1 ms)`, пока не выполнены все три условия:
`GameState::Playing`, нет `CityMeshTask`, нет `PendingCitySpawn`. Дедлайн 120 с, при `should_exit` —
`GATE BROKEN`. Файлы Kenney для теста не нужны: без GLB-загрузчика и без файлов `AssetServer` пишет
ошибку в лог и не паникует. Это подтверждается первым прогоном на клоне без `assets/third_party/*`.

Тест `city_meshes_are_merged_per_chunk` (seed 1):
1. `n = (params.size / render.chunk_size).ceil() as u32`, `expected = n²`. Считается **в тесте**,
   `chunk_of` и `build_city_meshes` не вызываются. В комментарии пример: 1200/128 → 10 → 100.
2. `count(With<Mesh3d>, With<CityBuilding>) == 0` — пер-зданийных мешей нет (требование TASK_FINAL).
3. `count(With<Mesh3d>, With<CityChunk>) == expected`; множество `coord` уникально и равно `[0, n)²`.
4. `count(With<Mesh3d>, Without<CityProp>) == expected`. В этом харнессе нет `VisualsPlugin`, поэтому
   персонаж и небо не появляются, и любой другой меш — посторонняя геометрия города (отдельная земля,
   пер-квартальные тротуары).
5. Покрытие, проверка независимая от счётчиков реализации. Для каждого здания `b` тест сам считает
   чанк `c = clamp(floor((b.center + size/2) / chunk), 0, n−1)` и верхнюю часть (последний ярус или
   база) с полуразмерами `h`. Угол крыши `p2 = center + u·h.x + v·h.y` на `y = b.height` должен быть
   вершиной меша чанка `c`: `Assets<Mesh>` → `ATTRIBUTE_POSITION`, множество позиций, квантованных до
   1 мм. Сообщение называет индекс здания и чанк.
6. Каждый меш чанка непуст: `indices.len() > 0` и кратно 3.
7. `count(With<CityProp>) > 0`, и у каждого `CityProp` есть `VisibilityRange` с
   `end_margin.end == props.visibility_range` и `end_margin.start == visibility_range − fade`.

Тест `render_props_reference_manifest_files`: каждый `model`/`texture` из `render.ron` проходит
`manifest.contains_asset` для shipped манифеста. Так CI ловит расхождение путей без скачанных ассетов.

Запуск: `cargo test -p gta_like --bin gta_like city_gate`.

### Шаг 19. QA: `tools/qa/brp.py` + `tools/qa/scenarios/t3.py`

- `brp.py`: `Game.__init__(self, features=("dev",), args=(), port=15702, release=False)`. В `start` при
  `release` добавить `"--release"` в команду `cargo build` и брать бинарник из `target/release/`. Больше
  ничего не менять: t1/t2 работают как раньше.
- `t3.py` по образцу `t2.py`:
  1. `subprocess.run([sys.executable, "tools/fetch_assets.py", "--check"])`; код ≠ 0 →
     `AssertionError("run python tools/fetch_assets.py first")`.
  2. `Game(features=("dev",), args=("--seed", "1"), release=True)`.
     `wait_resource("CityLayoutHash", 180) == load_golden()[1]`.
  3. Дождаться, пока число строк `CityChunk` станет 100, а `CityProp` перестанет расти (≤ 60 с). Записать оба числа.
  4. `CityLandmarks` через `game.resource("CityLandmarks")`; `vec3()` принимает список и dict.
  5. Телепорт игрока (`Position`, путь `""`, значение `[x, roof_y + 1.2, z]`), 2 с ожидания, проверить
     `roof_y + 0.8 < y < roof_y + 1.4`: игрок стоит на крыше верхнего яруса, compound-коллайдер работает.
     Ожидаемое y = 156 + 1.05 = 157.05.
  6. Три скриншота с крыши с `move_mouse(dx=1000, dy=0)` между ними и один с наклоном вниз (`dy=150`).
  7. Прогрев 5 с, затем 5 чтений `diagnostics()` раз в секунду: FPS min/avg и frame time, плюс
     `window_state()` (present mode). Это доказательство для владельца, не assert.
  8. Телепорт в `park_center + (0, 1.2, 0)`, 2 с, скриншот.
  9. `log_tail()` не содержит строк уровня ERROR с `wgsl`, `shader`, `gltf`, `asset` или `Failed to load`.
     Если содержит — AssertionError с этими строками.
  10. `summary.json` в `--out` (по умолчанию `target/qa/t3`).
- Проверка: `python tools/qa/scenarios/t3.py --out <scratch>/qa/t3` выполняет QA-стадия на машине с окном
  и ассетами. Имплементер в codex-sandbox окно не запускает.

### Шаг 20. Финальная проверка

`cargo build`; `cargo clippy -- -D warnings` (команда acceptance); дополнительно
`cargo clippy --workspace --all-targets -- -D warnings` — новые предупреждения исправить, старые
перечислить в IMPL_SUMMARY без правок. Далее `cargo test -p citygen`, `cargo test -p gta_sim`,
`cargo test -p gta_like`, `python tools/qa/tree_check.py`, `python tools/fetch_assets.py --check`.
Гейты `gta_sim` и `gta_like` прогнать ещё раз с временно перемещённым `assets/third_party/city-kit-*`
(состояние свежего клона): манифест-гейт показывает SKIP, гейт слияния зелёный. После вернуть.
`git status` / `git ls-files assets/third_party`: tracked только `manifest.ron`; в диффе нет
GLB/PNG/License/zip. `.gitattributes` не изменён. Каждый flip-RED из §3.2 записан в IMPL_SUMMARY.

## 3. Test plan

### 3.1 Гейты и их класс

| Гейт | Где | Класс | Что держит |
|---|---|---|---|
| `manifest_fixtures_are_judged` | `gta_sim/tests/asset_manifest.rs` | строгость данных | схема отвергает неизвестные и дублирующие поля, небезопасные пути, плохие хэши, лицензию, URL; valid_minimal проходит |
| `shipped_manifest_is_valid` | там же | корректность | манифест в git валиден, 3 пакета по 4 файла |
| `local_assets_match_manifest` | там же | корректность воспроизводимости | при наличии пакетов: SHA-256 каждого файла, CC0 по байтам, точный состав; частичная установка — RED; свежий клон — SKIP |
| `city_meshes_are_merged_per_chunk` | `src/visuals/city_gate.rs` (`-p gta_like`) | корректность (бюджет кадра) | 0 `Mesh3d` на `CityBuilding`, `n²` чанков из `render.ron`, нет посторонних мешей, каждое здание в своём чанке по вершине крыши, пропы с `VisibilityRange` |
| `render_props_reference_manifest_files` | там же | целостность данных | пути пропов в `render.ron` ⊂ манифест |
| `landmarks_exist`, `setback_tiers_nested` | `citygen/tests/properties.rs` | корректность | башня единственная и самая высокая, площадь — один лот, парк в радиусе, вложенность ярусов |
| golden (re-bless) | `citygen/tests/golden.rs`, `gta_sim/tests/city.rs::runtime_hash_matches_golden` | детерминизм | схема хэша v2 |
| `one_static_collider_per_building` (ре-анкор) | `gta_sim/tests/city.rs` | корректность | один коллайдер на здание, compound = ярусы с точными размерами |
| `settle` (ре-анкор) во всех city-тестах | `gta_sim/tests/common` | корректность | игрок стоит на бордюре `curb + float` = 1.20 |
| `landmarks_resource_matches_layout` | `gta_sim/tests/city.rs` | liveness + корректность | BRP-точки совпадают с layout, число `CityBlock` |
| `validate_rejects_bad_params` (+ башня) | `citygen/src/params.rs` | строгость данных | башня обязана быть выше всех |
| `grid_covers_ground`, yaw/расстановка | unit в `city_mesh.rs`, `props.rs` | корректность | формулы шагов 14–15 на рабочих примерах |
| t3 runtime | `tools/qa/scenarios/t3.py` | liveness + доказательство | крыша держит игрока, скриншоты, FPS, лог без ошибок shader/asset |

### 3.2 Flip-RED (сломать → RED → вернуть → GREEN; в IMPL_SUMMARY записать, что портили)

1. Слияние: вернуть наблюдатель `On<Add, CityBuilding>` с `Mesh3d(Cuboid)` (старый `src/visuals/city.rs:54-77`) → RED пункты 2 и 4 гейта.
2. Сетка: в `build_city_meshes` взять `n = 1` → RED пункт 3 (1 ≠ 100).
3. Потеря здания: в `build_city_meshes` пропустить здание `landmarks.tower` → RED пункт 5 с индексом башни.
4. Посторонний меш: временно заспавнить в `CityVisualsPlugin` отдельный меш земли → RED пункт 4.
5. `VisibilityRange`: убрать его из бандла пропа → RED пункт 7.
6. Манифест (с установленными пакетами): одна hex-цифра `sha256` у `light-square.glb` → RED с именем
   файла; `extra.glb` в `city-kit-roads` → RED по составу; перенести `city-kit-industrial` наружу → RED
   «частичная установка».
7. Фикстуры: заменить содержимое `bad_hash.ron` на `valid_minimal.ron` → `manifest_fixtures_are_judged`
   RED: негативная проверка не пустая.
8. Башня: в тестовом `CityParams` (в обход validate) `tower_floors = 20` → 78 м < 117 м, `landmarks_exist`
   RED; в `city.ron` `tower_floors: 30` → `validate` падает, падает `cargo test -p gta_sim --test config`.
9. Бордюр: не спавнить `CityBlock` → `settle` RED (y ≈ 1.05 вместо 1.20).
10. Ярусы: в `spawn_buildings` всегда cuboid → `one_static_collider_per_building` RED на зданиях с ярусами.
11. Preflight (ручной): переименовать `assets/third_party/city-kit-roads` → `cargo run` завершается с
    ошибкой, названа команда `python tools/fetch_assets.py`.

### 3.3 Owner checklist (QA переносит в QA_REPORT.md)

`python tools/fetch_assets.py`, затем `cargo run --release --features fast -- --seed 1`, потом `--seed 2`:
1. Город выглядит как город: тротуары приподняты бордюром, на асфальте осевые, разделители полос и
   зебры, у зданий окна по этажам, у высоток видны уступы.
2. Районы различимы: Downtown (серо-синие высотки, бетонные лоты), Commercial (бежевый, средняя высота),
   Residential (низкие дома, зелёные лоты, деревья вдоль улиц), Industrial (низкие корпуса, контейнеры за
   зданиями).
3. Ориентиры: самая высокая башня на площади у центра видна издалека, центральный парк с деревьями.
4. Солнце даёт тени от зданий примерно до 350 м; небо — градиент; дальний план уходит в туман на
   250–450 м без шва с небом.
5. Фонари стоят на тротуаре плечом к дороге; светофоры на углах авеню смотрят разумно; пропы исчезают
   дальше ~120 м без заметного «попа» (fade 10 м).
6. Игрок заходит с дороги на тротуар без застревания (бордюр 0.15 м); с крыши башни виден город.
7. FPS с крыши (цифры из `summary.json` t3) — информация для владельца, не порог.

Всё настраивается в `assets/world/render.ron` (вид, бюджеты спавна) и `assets/world/city.ron`
(ориентиры, уступы, бордюр).

## 4. Rollout notes

- **Ассеты не в git (owner rule).** Свежий клон: `cargo build` и все `cargo test` зелёные (манифест-гейт
  показывает SKIP для файлов). `cargo run` завершается ошибкой с командой `python tools/fetch_assets.py`.
  Разработчику нужен один запуск fetch (сеть или `--cache DIR` с zip). В релизный zip владелец кладёт
  `assets/third_party/<pack>/` рядом с бинарником: preflight ищет файлы по тем же путям.
- **Миграция данных:** `city.ron` получает `roads.curb_height`, `massing`, `landmarks`; `render.ron` —
  новые секции. Оба загрузчика `deny_unknown_fields`, старые файлы не загрузятся. Правятся в том же
  коммите.
- **Детерминизм:** хэш схемы v2, golden 1/2/42 заново bless-нуты. Это читают `citygen`, `gta_sim` и t2.py
  (`load_golden`), у t2 правок нет.
- **Зависимости:** `sha2 =0.10.9` только в dev-dependencies `gta_sim`; `tree_check.py` держит границу
  `bevy_render`. В клиенте новых крейтов нет.
- **Фич-флагов нет.** `.gitattributes` не меняется.
- **Risk areas:**
  - Кадр применения: 100 мешей и около 4.4 тыс. пропов режутся бюджетом `spawn_budget`. GPU-upload
    крупных чанков всё равно ложится на первые кадры, это меряет t3 и владелец. `VisibilityRange` не
    заменяет frustum culling и не доказывает FPS.
  - Клон `CityLayout` в задачу — одноразовый, порядка миллисекунды. Если trace покажет иначе, перейти на
    `Arc` в `City` отдельной задачей.
  - WGSL-имена (`in.uv`, `VERTEX_UVS_A`, `pbr_input.material.perceptual_roughness`) сверены по 0.19.1, но
    ошибка шейдера видна только в окне: t3 проверяет лог, владелец — картинку.
  - Выбор площади/парка может упасть на каком-то seed (`NoLandmarkCandidate`). Держится 33 layout
    property-гейтов. При провале расширить радиус данными, ошибку не убирать.
  - Z-fighting разметки на 2 см над асфальтом вдали: при жалобе владельца увеличить `surface_layer_step`.
  - Бордюр 0.15 м для raycast-подвески машин проверит T14.
  - Kenney сменит хэш в URL: манифест держит проверенный архив, fetch падает с явным сообщением, новая
    ссылка берётся со страницы пакета и перепроверяется руками.
  - Python-парсер и Rust/serde — две реализации одной схемы. Паритет держится общими фикстурами
    (`--validate-only`), это ручная проверка, а не CI. RON-синтаксис манифеста сужен до грамматики шага 4
    (без имён структур).

## 5. Review notes (plan-reviewer-2)

**Опровержение (обязательное).** Контрпример 1: `PlayerSpawn` в городе стоит на дороге, тогда подъём до
`curb_height` и новая `settle` дают ложный RED. Проверено по коду: `player_spawn` = середина стороны +
`walk_offset` (`half_carriageway + sidewalk/2`, `graphs.rs:137-155`), а `curb` = inset на
`half_carriageway` (`roads.rs:166-177`). Спавн всегда внутри `curb`. **Не подтвердился.** Контрпример 2:
вогнутый квартал, и `convex_hull` бордюра заливает дорогу. `geom::inset` отвергает невыпуклые вход и выход
(`geom.rs:31,50`), супер-кварталы сливаются только при `is_convex` (`roads.rs:127`). **Не подтвердился.**
Контрпример 3: буквальное `/assets/third_party/` из TASK_FINAL. Проба `scratch/r2_gitprobe/` показала, что
тогда игнорируется и `manifest.ron`. **Подтвердился**, правило исправлено на `/*` (шаг 1).

**Что изменено относительно PLAN_V2 и PLAN.md:**
1. Детали PLAN.md, которые V2 сжал без исправления, восстановлены: точный манифест, типы, сигнатуры,
   рабочие примеры (ярусы, compound, сетка, обход граней, yaw), содержимое `render.ron`, WGSL, харнесс,
   flip-RED.
2. Исправления V2 приняты: ассеты не коммитятся, `.gitattributes` не меняется; гейт отделён от персонажа;
   покрытие проверяется независимо от счётчика; startup preflight; `--check` отделён от гейта; порционное
   применение; строгость и паритет парсеров; бордюр без silent skip; `settle` заякорен на `PlayerSpawn.y`;
   runtime-проверка лога; решённые вопросы закрыты.
3. Уточнения reviewer-2:
   - `.gitignore` — `/assets/third_party/*`, доказано пробой.
   - Схема манифеста одна (`gta_sim::config::manifest`), её используют гейт и `main.rs`. Правила
     валидации перечислены явно и продублированы в Python; общие фикстуры с таблицей ожиданий и
     анти-тавтологический `valid_minimal.ron`.
   - Три состояния файлового гейта: нет пакетов → SKIP, часть → RED, все → полная проверка.
   - Покрытие зданий проверяется вершиной крыши в чанке, который считает сам тест. Поле
     `CityChunk.buildings` удалено: самоотчётный счётчик движется вместе с дефектом.
   - Пункт «нет посторонних мешей» стал точным: в харнессе только `CityVisualsPlugin`, всю геометрию
     города держит этот плагин (правило шага 17), `MaterialPlugin` — в `VisualsPlugin`.
   - Сломанный бордюр-коллайдер (`convex_hull → None`) ведёт к `AppExit::error()`, а не к `continue`.
   - `citygen::centroid` стал `pub`, вместо копий формулы в `gta_sim` и клиенте.
   - Захардкоженные настроечные числа PLAN.md перенесены в `render.ron`: `perceptual_roughness: 0.8`
     пропов → `props.roughness`, сегменты неба → `sky.sectors/stacks`, габариты контейнера →
     `props.container_raw_size`. Бюджет применения → `spawn_budget`.
   - `RenderConfig::validate` проверяет `sky.radius < PerspectiveProjection::default().far` (камера
     берёт default far = 1000), степень двойки shadow map, отношения окна.
   - Число property-layout исправлено: 33, а не 35 (SEEDS ∪ SWEEP 0..32).
   - Числа flip-RED пересчитаны: бордюр |1.05 − 1.20| = 0.15 > 0.05; башня 20 этажей = 78 м < 117 м;
     крыша t3 = 157.05.
   - `Cargo.lock` из scratch проверен против текущего HEAD: дифф совпадает, только 7 добавлений.
   - Хэши Kenney перепроверены по локальным zip.
4. Сознательно не сделано: SHA-256 при старте клиента (гейт и `--check` уже это держат, пришлось бы
   добавить `sha2` в клиент); CI-проверка паритета Python/Rust (фикстуры и ручной прогон
   `--validate-only`); автоматический FPS-порог (решение GDD §11).

children: 0 launched / 0 reported.
