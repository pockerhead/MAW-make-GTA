# PLAN_FINAL — TASK-005 (GDD T4): гуманоид с анимациями

Источники объёма: `docs/design/GDD.md` §3.1, §9.2, §12, §13 T4; `TASK_FINAL.md` (включая `### Resolved questions`:
правка двух фактических строк GDD принята, тинт = множитель на `body-mesh`, Run = клип `sprint` с производной скоростью,
модель игрока `male-a`). Pinned: bevy 0.19.1, avian3d 0.7.0, bevy-tnua 0.32.0, bevy-tnua-avian3d 0.12.1 (vendored),
bevy_brp_extras 0.22.6, ron 0.12.2 (`Cargo.lock`). Все API ниже проверены по
`~/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/<crate>-<ver>/`. Новых крейтов нет, `Cargo.toml`/`Cargo.lock` не меняются.

---

## 1. Summary

Добавляем пакет Kenney Mini Characters в манифест третьих ассетов вместе с записью рига, проверенной по байтам архива:
12 GLB, в каждом 2 skinned-меша (`body-mesh`, `head-mesh`), 2 skin с одними и теми же 7 суставами, 32 клипа, клипа `run`
нет. Rust-схема (`gta_sim`) проверяет согласованность записи, `tools/fetch_assets.py` сверяет её с байтами GLB. В
`gta_sim/character` появляется отражаемый компонент `AnimState { Idle, Walk, Run, Sprint, Jump, Fall }`. Его выводит
чистая функция из горизонтальной скорости и признака `is_airborne()` Tnua, система пересчёта стоит в `FixedPostUpdate`
после шага физики Avian. Клиент (`src/visuals/character.rs`) заменяет капсулу-заглушку на дочернюю glTF-модель
(`WorldAssetRoot`, масштаб до 1.8 м, разворот π), строит граф анимаций из индексов клипов манифеста, переключает клипы с
кроссфейдом по `AnimState`, скорость клипа ходьбы/бега берёт из скорости тела (защита от скольжения ног), красит
`body-mesh`. Все тюнинговые числа живут в `assets/character/locomotion.ron` и `assets/character/visual.ron`. Корректность
держат headless-гейты (`gta_sim`, `gta_like --bin`), фактическую загрузку модели держит BRP-сценарий `t4.py`, вид и
"ноги не скользят" принимает владелец.

---

## 2. Implementation steps

### Шаг 0. Подготовка

- Ветка `feature/t04-humanoid-animations` (уже есть). Работать в текущем checkout, без второго `target/`.
- Установить пакеты без сети (sandbox не ходит на kenney.nl), после шага 4:
  `python tools/fetch_assets.py --cache maw/tasks/in_progress/TASK-005/scratch`. В этой папке лежит
  `kenney_mini-characters.zip` с SHA-256 `9e1d48e6d7b8479ebbe84df71eb5bd8e1b3f0da546dea641890dccc8a02d0999`
  (перепроверено ревьюером), basename совпадает с URL. Бинарники не коммитятся (`.gitignore:64` `/assets/third_party/*`,
  проверено `git check-ignore -v`).
- Повторный аудит (только чтение): `python maw/tasks/in_progress/TASK-005/scratch/planner/audit_rig.py` должен дать
  те же строки, что `scratch/planner/audit_rig.txt` (`identical_rig_facts_across_models True`, 32 клипа, 7 суставов).
  Независимая проверка ревьюера: `scratch/reviewer2/probe_glb.txt` (male-a и female-c: 1 сцена, внешний
  `Textures/colormap.png`, один материал `colormap` без `baseColorFactor`, `body-mesh` и `head-mesh` делят материал 0,
  AnimationPlayer ляжет на корневой узел `character-<name>`).

### Шаг 1. `assets/third_party/manifest.ron`: пакет `mini-characters`

Добавить четвёртый пакет после `city-kit-industrial`, буквально (хэши из `scratch/planner/audit_rig.txt`, сверены с
архивом):

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
            // Checked against the GLB bytes by tools/fetch_assets.py (skinned meshes, joints per skin, clips in glTF order).
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

Файл UTF-8 без BOM. Все 12 моделей нужны T8/T9 (GDD §9.2); ~3 МБ в git не попадают.

### Шаг 2. Rust-схема рига (`crates/gta_sim/src/config/manifest.rs`)

- После `PackFile` (`:35-41`):
  ```rust
  /// Skinned models of a pack, recorded from the pinned archive and checked against the GLB bytes by fetch_assets.py.
  #[derive(Deserialize, Debug, Clone)]
  #[serde(deny_unknown_fields)]
  pub struct PackRig {
      pub models: Vec<String>,
      pub skinned_meshes: Vec<String>,
      pub joints: Vec<String>,
      pub clips: Vec<String>,
  }
  ```
- В `AssetPack` (`:19-28`) последнее поле: `#[serde(default)] pub rig: Option<PackRig>,`. Отсутствующее поле и явное
  `rig: None` оба дают `None` (ron 0.12 без `implicit_some`: значение пишется как `Some((...))`).
- `impl PackRig { fn validate(&self, pack: &str, paths: &HashSet<&str>) -> Result<(), String> }`:
  для каждого из четырёх списков (`models`, `skinned_meshes`, `joints`, `clips`) вызвать helper
  `fn check_names(pack: &str, field: &str, list: &[String]) -> Result<(), String>`: список не пуст, нет пустых строк,
  нет повторов. Затем для каждой модели: `paths.contains(model)` и `model.ends_with(".glb")`. Каждое сообщение
  начинается с `pack {pack}: rig ` (например `pack {pack}: rig clips has duplicate "idle"`,
  `pack {pack}: rig model "missing.glb" is not listed in files`). Golden Path: early return, без вложенности ≥ 2.
- В `AssetPack::validate` перед финальным `Ok(())` (`:122`):
  `let Some(rig) = &self.rig else { return Ok(()) }; rig.validate(name, &paths)`.
- `impl PackRig { pub fn clip_index(&self, name: &str) -> Option<usize> }` — позиция в `clips` = индекс glTF-анимации
  (`GltfAssetLabel::Animation(i)`, `bevy_gltf-0.19.1/src/label.rs:34-60`).
- `impl ThirdPartyManifest { pub fn rig_for(&self, asset_path: &str) -> Option<&PackRig> }` — пакет с `rig`, у которого
  `asset_path == format!("{THIRD_PARTY_DIR}/{}/{}", pack.name, model)` для одной из `rig.models`.

### Шаг 3. Фикстуры и тесты манифеста (`crates/gta_sim/tests/`)

Новые фикстуры в `tests/fixtures/manifest/` (копии `valid_minimal.ron` плюс поле `rig` последним в пакете, UTF-8 без BOM):

| Файл | Поле `rig` | Rust | Python `--validate-only` |
|---|---|---|---|
| `valid_rig.ron` | `Some((models: ["light-square.glb"], skinned_meshes: ["m"], joints: ["root"], clips: ["idle"]))` | parse + validate ok | `valid` |
| `valid_rig_none.ron` | `None` | parse + validate ok | `valid` |
| (`valid_minimal.ron`, поля нет) | — | ok (уже есть) | `valid` |
| `bad_rig_model.ron` | как `valid_rig`, но `models: ["missing.glb"]` | parse ok, validate err со словом `rig` | error со словом `rig` |
| `bad_rig_duplicate_clip.ron` | как `valid_rig`, но `clips: ["idle", "idle"]` | parse ok, validate err со словом `rig` | error со словом `rig` |

`tests/asset_manifest.rs`:
- `manifest_fixtures_are_judged` (`:27-63`): проверять цикл по `["valid_minimal.ron", "valid_rig.ron",
  "valid_rig_none.ron"]` (parse + validate ok); в список семантических добавить `("bad_rig_model.ron", "rig")`,
  `("bad_rig_duplicate_clip.ron", "rig")`.
- `shipped_manifest_is_valid` (`:65-81`): множество имён `{"city-kit-roads", "city-kit-suburban",
  "city-kit-industrial", "mini-characters"}`; число файлов по пакету: `city-kit-*` → 4, `mini-characters` → 14; у
  `city-kit-*` `rig.is_none()`. У `mini-characters`: `models.len() == 12`, `skinned_meshes.len() == 2`,
  `joints.len() == 7`, `clips.len() == 32`, `clip_index` для `idle, walk, sprint, jump, fall` =
  `Some(1), Some(2), Some(3), Some(4), Some(5)`, `clip_index("run") == None`. Эти числа взяты из байтового аудита
  архива (`scratch/planner/audit_rig.txt`, `scratch/reviewer2/probe_glb.txt`), а не из манифеста. Тест фиксирует
  запись (ловит выпавший клип или сдвиг порядка при ручной правке); сверку с байтами делает шаг 4.
- `local_assets_match_manifest` не менять. После установки он сверит 14 файлов и лицензию (`License.txt` содержит
  "Creative Commons Zero" и "CC0", проверено).

### Шаг 4. `tools/fetch_assets.py`: `Some`/`None`, схема рига, проверка по байтам GLB

- `Parser.value` (`:70-84`), ветка ident: если `match.group() == "Some"` и `self.peek() == "("` →
  `self.expect("("); inner = self.value(); self.expect(")"); return inner`. Если ident `== "None"` и следующий символ
  не `(` → `return None`. Для остальных ident + `(` оставить ошибку `named struct ... is not supported`.
- Схема: `OPTIONAL_PACK_KEYS = {"rig"}`, `RIG_KEYS = {"models", "skinned_meshes", "joints", "clips"}`.
  `exact_keys(obj, keys, where, optional=frozenset())`: `extra = set(obj) - keys - optional`, `missing = keys - set(obj)`.
  В `check_schema` пакет проверять через `exact_keys(pack, PACK_KEYS, "pack", OPTIONAL_PACK_KEYS)`. В конце цикла по
  пакету: `rig = pack.get("rig")`; если `None`, пропустить; иначе `exact_keys(rig, RIG_KEYS, f"pack {name} rig")` и те же
  правила, что в Rust (каждый список непуст, строки непусты, без повторов, модель в `paths`, оканчивается на `.glb`),
  с теми же сообщениями `pack {name}: rig ...`.
- Докстринг модуля: одна строка, что манифест может нести `rig`, который `--check` и установка сверяют с GLB.
- Разбор GLB (stdlib): `glb_json(data)`: `struct.unpack_from("<4sII", data, 0)` → magic `b"glTF"`, version 2, длина
  `== len(data)`; первый чанк `struct.unpack_from("<II", data, 12)` типа `0x4E4F534A` → `json.loads(data[20:20+len])`;
  иначе `ManifestError` с именем модели.
- `rig_facts(gltf)` → `(skinned, joints_per_skin, clips)`: `skinned` = имена узлов с ключом `skin` в порядке индекса узла,
  `joints_per_skin` = для каждого skin список имён узлов из `joints`, `clips` = имена `animations` по порядку.
- `rig_problems(pack, read)` (`read(path) -> bytes`): для каждой модели из `rig["models"]`: `skinned == rig.skinned_meshes`,
  `len(joints_per_skin) == len(skinned)`, каждый skin `== rig.joints`, `clips == rig.clips`. Сообщение
  `f"{pack}: {model}: rig {field} {actual}, manifest {recorded}"`.
- Вызовы:
  - `pack_problems` (`:232-244`): в конце, если `problems` пуст и `pack.get("rig")` задан,
    `problems += rig_problems(pack, lambda p: (directory / p).read_bytes())`.
  - `extract` (`:300-321`): внутри существующего `try`, после блока `with zipfile.ZipFile(...)`, если `rig` задан:
    `problems = rig_problems(pack, lambda p: (tmp / p).read_bytes())`, при непустом → `raise ManifestError("\n".join(problems))`.
    Существующий `except BaseException` удалит `tmp`, установленная папка не тронута.
  - `fetch` не меняется: неверная запись рига при верных SHA даёт проблему в `pack_problems` → переустановка →
    `extract` падает с сообщением про `rig`.

Проверка шага: `python tools/fetch_assets.py --validate-only crates/gta_sim/tests/fixtures/manifest/<f>.ron` для пяти
фикстур из таблицы шага 3 (результаты как в колонке Python); затем установка из `--cache` и
`python tools/fetch_assets.py --check` → `third-party packs match the manifest`.

### Шаг 5. Данные и валидация локомоции (`crates/gta_sim/src/character/locomotion.rs`, `assets/character/locomotion.ron`, `crates/gta_sim/src/lib.rs`)

- `LocomotionConfig`: новое поле `pub anim_idle_speed: f32` (м/с; ниже — `Idle`) после `sprint_speed`.
- `locomotion.ron`: `anim_idle_speed: 0.2,` после `sprint_speed: 6.8,`. Обоснование числа: разгон 30 м/с²
  (`run_speed / time_to_run_speed` = 4.5 / 0.15) проходит 0.2 м/с за один тик 64 Гц, порог не задерживает старт анимации.
- `impl LocomotionConfig { pub fn validate(&self) -> Result<(), String> }`: все четыре числа конечные и
  `0 < anim_idle_speed < walk_speed < run_speed < sprint_speed`; иначе сообщение с именами полей и значениями,
  например `"anim_idle_speed 2.0 must be finite and satisfy 0 < anim_idle_speed < walk_speed (1.8) < run_speed < sprint_speed"`.
  Проверка узкая: только поля, на которых стоит разбиение `AnimState`.
- `compose_sim` (`lib.rs:24`): сразу после `load_config::<LocomotionConfig>` —
  `cfg.validate().map_err(|message| ConfigError { path: root.path(LOCOMOTION_CONFIG), message })?;` (тот же приём, что у
  `CityParams`, `:27-30`).
- `crates/gta_sim/tests/config.rs`: `shipped_locomotion_config_loads` → загрузить и `validate().unwrap()`; новый тест
  `locomotion_thresholds_are_validated`: во временном корне (как `unknown_field_names_file_and_field`, `:50-73`)
  записать шипованный файл с `anim_idle_speed: 0.2` → `anim_idle_speed: 2.0`, `load_config` ok, `validate()` →
  `Err`, текст содержит `anim_idle_speed`.

### Шаг 6. `AnimState` в `gta_sim` (новый `crates/gta_sim/src/character/anim.rs`)

```rust
/// Locomotion animation state derived from body velocity and ground support.
#[derive(Component, Reflect, Default, Clone, Copy, Debug, PartialEq, Eq)]
#[reflect(Component, Default)]
pub enum AnimState { #[default] Idle, Walk, Run, Sprint, Jump, Fall }

/// Airborne: `Jump` while rising, else `Fall`. Grounded: by horizontal speed, split at `anim_idle_speed`
/// and at the midpoints between walk/run and run/sprint speeds.
pub fn anim_state(velocity: Vec3, airborne: bool, cfg: &LocomotionConfig) -> AnimState
```

Тело (let-else/early return):
```rust
if airborne {
    return if velocity.y > 0.0 { AnimState::Jump } else { AnimState::Fall };
}
let horizontal = Vec2::new(velocity.x, velocity.z).length();
if horizontal < cfg.anim_idle_speed { return AnimState::Idle; }
if horizontal < (cfg.walk_speed + cfg.run_speed) / 2.0 { return AnimState::Walk; }
if horizontal < (cfg.run_speed + cfg.sprint_speed) / 2.0 { return AnimState::Run; }
AnimState::Sprint
```
Границы на шипованных данных: 0.2 / 3.15 / 5.65 м/с. Порядок вариантов enum фиксирован: клиент индексирует
`[_; 6]` через `state as usize` (Idle=0 … Fall=5), это закреплено тестом шага 9.

Система:
```rust
pub(super) fn update_anim_state(
    cfg: Res<LocomotionConfig>,
    mut query: Query<(&LinearVelocity, &TnuaController<CharacterScheme>, &mut AnimState), With<Character>>,
) {
    for (velocity, controller, mut state) in &mut query {
        // Before Tnua pulls its config the controller cannot tell; treat the body as grounded.
        let airborne = controller.is_airborne().unwrap_or(false);
        state.set_if_neq(anim_state(velocity.0, airborne, &cfg));
    }
}
```
`is_airborne() -> Result<bool, TnuaControllerHasNotPulledConfiguration>` (`bevy-tnua-0.32.0/src/controller.rs:572`),
паники нет.

`crates/gta_sim/src/character/mod.rs`:
- `mod anim; pub use anim::{AnimState, anim_state};`
- `#[require(MoveIntent, JumpBuffer, AnimState)]` у `Character` (`:22` текущего файла, строка с `#[require]`).
- `CharacterPlugin::build`: `.register_type::<AnimState>()` и
  `.add_systems(FixedPostUpdate, anim::update_anim_state.after(PhysicsSystems::Last))`.
  `PhysicsSystems` уже в `avian3d::prelude::*` (`avian3d-0.7.0/src/lib.rs:554-556`), импорт есть.
  Почему `FixedPostUpdate`: `compose_sim` добавляет `PhysicsPlugins::default()`, а это `Self::new(FixedPostUpdate)`
  (`avian3d-0.7.0/src/lib.rs:751-754`); наборы `First..Last` сцеплены в этом расписании (`schedule/mod.rs:74-85`).
  Tnua (`TnuaAvian3dPlugin::new(FixedUpdate)`) пишет горизонтальную скорость boost-ом до физики
  (`vendor/bevy-tnua-avian3d-0.12.1/src/lib.rs` `apply_motors_system`: `linear_velocity_mut() += boost`), а гравитацию
  и ускорения интегрирует шаг Avian. Значит после `PhysicsSystems::Last` скорость соответствует концу текущего тика.
  `is_airborne()` отражает проход Tnua до физики этого тика; задержка опоры до одного тика допустима и не скрывается.
- Новых `Res` у существующих систем нет, харнессы тестов не трогаем.

Unit-тест `anim_state_table` в `anim.rs` (`#[cfg(test)]`): конфиг —
`ron::from_str::<LocomotionConfig>(include_str!("../../../../assets/character/locomotion.ron")).expect("GATE BROKEN: locomotion.ron")`
(ron парсит CRLF и LF одинаково). Ожидания выведены из шипованных чисел walk 1.8 / run 4.5 / sprint 6.8 /
anim_idle_speed 0.2:

| velocity (x, y, z) | airborne | ожидание | что ловит |
|---|---|---|---|
| (0, 0, 0) | false | Idle | стоит |
| (0.1, 0, 0) | false | Idle | ниже порога 0.2 |
| (0, 0, −1.8) | false | Walk | идёт |
| (0.15, 0, −0.15) | false | Walk | модуль 0.212 ≥ 0.2; по одной оси (0.15) вышло бы Idle |
| (1.08, 0, −1.44) | false | Walk | диагональ походки walk (модуль 1.8) |
| (0, 0, −4.5) | false | Run | бежит |
| (0, 0, −6.8) | false | Sprint | спринт |
| (0, 3.0, −2.5) | false | Walk | горизонталь 2.5; 3D-длина 3.905 дала бы Run |
| (0, 3.0, −4.5) | true | Jump | в воздухе вверх |
| (0, −2.0, −4.5) | true | Fall | падение |
| (0, 0, 0) | true | Fall | апекс: `vy > 0` строго |
| (0, 0, −3.14) / (0, 0, −3.16) | false | Walk / Run | граница 3.15 |
| (0, 0, −5.64) / (0, 0, −5.66) | false | Run / Sprint | граница 5.65 |
| (0, 0, −0.19) / (0, 0, −0.21) | false | Idle / Walk | граница 0.2 |

### Шаг 7. Интеграционный гейт `crates/gta_sim/tests/anim_state.rs` (новый)

`mod common; use common::*;`, приложение `headless_app()` (та же `compose_sim`, `app.finish(); app.cleanup();`,
`FixedTimesteps(1)`), время только через `run_ticks`/`settle` (считают `Time<Fixed>`). Спавн TestArea (0, 0, 0), пол
80×80 м, путь по −Z свободен до z = −12 (ступени лестницы на x 8.5..11.5, рампа на x −12..−8; `world/test_area.rs:6-35`).

1. `idle_after_settle` (liveness): `settle` → `AnimState::Idle` у игрока.
2. `gaits_map_to_states` (корректность в собранном App): для `(Gait::Walk, AnimState::Walk)`, `(Gait::Run, Run)`,
   `(Gait::Sprint, Sprint)` в отдельном `headless_app()`: `settle`, `set_intent(|i| { i.axis = Vec2::Y; i.gait = g; })`,
   `run_ticks(32)` → ожидаемое состояние. Вывод чисел: ускорение 30 м/с² → 6.8 м/с за 0.227 с ≈ 15 тиков из 32; за
   0.5 с спринта пройдено ≤ 3.4 м, препятствий нет. Walk 1.8 ∈ [0.2, 3.15), Run 4.5 ∈ [3.15, 5.65), Sprint 6.8 ≥ 5.65.
3. `jump_goes_up_then_falls_then_lands` (порядок): `settle`, `jump_held = true` на 30 тиков, затем `false` (приём
   `late_tap_is_buffered_until_landing`, `tests/jump.rs`); 120 раз `run_ticks(1)` с записью `AnimState`. Проверить:
   первое не-`Idle` = `Jump`; после последнего `Jump` есть `Fall`; последнее значение `Idle`; `Walk/Run/Sprint` не
   встречаются. Числа тиков не хардкодить. Воздушная фаза: v0 = √(2·9.81·1.0) ≈ 4.4 м/с, ≈ 0.9 с ≈ 58 тиков < 120.
4. `anim_state_matches_post_step_velocity` (корректность порядка): тот же прыжок, но после каждого `run_ticks(1)` читать
   `LinearVelocity`, `TnuaController::<CharacterScheme>::is_airborne().unwrap_or(false)` и `LocomotionConfig` и требовать
   `AnimState == anim_state(v, airborne, cfg)` на каждом тике; дополнительно требовать, что за прогон встречены и `Jump`,
   и `Fall` (гейт не пустой). Почему прыжок: горизонтальная скорость Tnua приходит boost-ом до физики, поэтому порядок
   систем по горизонтали почти не виден; вертикальная скорость на апексе меняет знак за счёт гравитации в шаге Avian,
   так что тик апекса различает "до" и "после" физики.
   Класс: 1 — liveness, 2 и 4 — корректность, 3 — порядок переходов; таблицу держит unit-тест шага 6.

### Шаг 8. Клиент: конфиг визуала (новый `src/visuals/character_config.rs`, новый `assets/character/visual.ron`)

```rust
pub const CHARACTER_VISUAL_CONFIG: &str = "character/visual.ron";

#[derive(Resource, Deserialize, Clone, Debug)]
#[serde(deny_unknown_fields)]
pub struct CharacterVisualConfig {
    pub(super) model: String,          // asset path of the glTF model
    pub(super) model_height: f32,      // head top of the model in model units (feet at 0)
    pub(super) height: f32,            // target body height, m
    pub(super) tinted_mesh: String,    // glTF mesh whose material is multiplied by `tint`
    pub(super) tint: (f32, f32, f32),  // linear multiplier of the base colour
    pub(super) blend_seconds: f32,     // cross-fade between states, s
    pub(super) idle: String,
    pub(super) jump: String,
    pub(super) fall: String,
    pub(super) walk: LocomotionClip,
    pub(super) run: LocomotionClip,
    pub(super) sprint: LocomotionClip,
}

#[derive(Deserialize, Clone, Debug)]
#[serde(deny_unknown_fields)]
pub(super) struct LocomotionClip {
    pub(super) clip: String,
    pub(super) native_speed: f32, // body speed at playback rate 1, model units/s
}

/// glTF animation indices of the clips shown for each `AnimState`, in `AnimState` declaration order.
#[derive(Resource, Clone, Copy, Debug, PartialEq, Eq)]
pub struct CharacterClips(pub [usize; 6]);
```
Методы `CharacterVisualConfig`:
- `pub(super) fn scale(&self) -> f32 { self.height / self.model_height }`.
- `pub fn validate(&self) -> Result<(), String>` в стиле `RenderConfig::validate` (`src/visuals/config.rs:189`):
  `model_height`, `height`, три `native_speed` конечные и > 0; `blend_seconds` конечный и ≥ 0; компоненты `tint` конечные
  и ≥ 0. Сообщение называет поле.
- `fn clip_names(&self) -> [&str; 6]` в порядке `AnimState`: `[idle, walk.clip, run.clip, sprint.clip, jump, fall]`.
- `pub fn resolve(&self, manifest: &ThirdPartyManifest) -> Result<CharacterClips, Vec<String>>`: `manifest.rig_for(&self.model)`
  (нет → `"model {model} has no rig in {THIRD_PARTY_MANIFEST}"`); `tinted_mesh ∈ rig.skinned_meshes` (иначе ошибка с
  именем меша); каждое имя клипа → `rig.clip_index` (нет → `"clip {name:?} is not in the rig of {model}"`). Собрать все
  ошибки, не только первую.

`assets/character/visual.ron` (UTF-8 без BOM):
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
Происхождение чисел:
- 0.67132 — `max_y` у `male-a` (`audit_rig.txt`), масштаб 1.8 / 0.67132 = 2.68128.
- 1.8 м — GDD §9.2; совпадает с верхом капсулы 1.05 + 1.5/2.
- 0.15 с — `time_to_run_speed` из `locomotion.ron`: переход не дольше смены скорости.
- `native_speed`: нога — жёсткий маятник длиной L = 0.17625 ед. (бедро над `root`), ступня касается земли около θ = 0.
  `walk`: −13.9° → +13.9° за 0.0667 с → ω = 7.28 рад/с → L·ω = 1.28 ед/с (3.43 м/с после масштаба).
  `sprint`: −14.4° → +14.4° за 0.0333 с → ω = 15.1 рад/с → 2.66 ед/с (7.13 м/с). Источник: `scratch/planner/clip_motion.txt`.
  Приём "скорость клипа = скорость тела / скорость клипа при rate 1" — стандартное distance matching для анимаций на месте.
- Run использует клип `sprint` с пониженной скоростью (решение `TASK_FINAL.md`).

### Шаг 9. Клиент: плагин персонажа (новый `src/visuals/character.rs`, ~200 строк)

Импорты сверх `bevy::prelude::*`: `bevy::gltf::GltfMeshName` (не в prelude: `bevy_gltf-0.19.1/src/lib.rs:156-159`),
`bevy::world_serialization::WorldInstanceReady` (не в prelude: `bevy_world_serialization-0.19.1/src/lib.rs:38-44`; так
импортирует и `examples/animation/animated_mesh.rs`). `WorldAssetRoot`, `WorldAsset`, `GltfAssetLabel`, `AnimationGraph`,
`AnimationGraphHandle`, `AnimationTransitions`, `AnimationPlayer`, `AnimationNodeIndex` — в prelude.
`avian3d::prelude::LinearVelocity`, `gta_sim::character::{AnimState, CharacterBody}`, `std::time::Duration`.

- `pub struct CharacterVisualsPlugin;` `build`:
  `app.register_type::<CharacterModel>().init_resource::<CharacterAnimations>().add_observer(spawn_character_model)
  .add_systems(Update, drive_character_animation);`
- `/// glTF faces +Z, gameplay bodies face -Z.` `const MODEL_YAW: f32 = std::f32::consts::PI;` — закон системы координат,
  не тюнинг.
- `#[derive(Resource)] pub(super) struct CharacterAnimations { pub(super) graph: Handle<AnimationGraph>, pub(super) nodes: [AnimationNodeIndex; 6] }`
  с `impl FromWorld` (прецедент `CharacterControlConfig`, `crates/gta_sim/src/character/mod.rs:83-92`):
  ```rust
  let model = world.resource::<CharacterVisualConfig>().model.clone();
  let clips = world.resource::<CharacterClips>().0;
  let asset_server = world.resource::<AssetServer>().clone();
  let mut graph = AnimationGraph::new();
  let root = graph.root;
  let nodes = clips.map(|index| {
      graph.add_clip(asset_server.load(GltfAssetLabel::Animation(index).from_asset(model.clone())), 1.0, root)
  });
  let graph = world.resource_mut::<Assets<AnimationGraph>>().add(graph);
  Self { graph, nodes }
  ```
  Граф готов в момент сборки плагина, до любого спавна. Инвариант композиции: `CharacterVisualConfig` и
  `CharacterClips` вставлены до `add_plugins(VisualsPlugin)` (шаг 10), иначе `resource::<..>()` паникует при старте с
  именем типа. Это та же точка отказа, что у `CharacterControlConfig`. Шесть узлов, `Run` и `Sprint` ссылаются на один
  клип, но у каждого узла свой `ActiveAnimation`, поэтому кроссфейд между ними работает.
- `#[derive(Component, Reflect, Default)] #[reflect(Component)] pub struct CharacterModel;` (QA находит модель по BRP).
- `pub(super) fn model_transform(float_height: f32, scale: f32) -> Transform`:
  `Transform::from_xyz(0.0, -float_height, 0.0).with_rotation(Quat::from_rotation_y(MODEL_YAW)).with_scale(Vec3::splat(scale))`.
  Ступни модели (y = 0) попадают на землю: центр тела висит на `float_height` (`player/mod.rs`).
- `spawn_character_model(event: On<Add, CharacterBody>, bodies: Query<&CharacterBody>, config: Res<CharacterVisualConfig>, asset_server: Res<AssetServer>, mut commands: Commands)`:
  `let Ok(body) = bodies.get(event.entity) else { return };` →
  `commands.entity(event.entity).insert(Visibility::default()).with_children(|parent| { parent.spawn((CharacterModel,
  WorldAssetRoot(asset_server.load(GltfAssetLabel::Scene(0).from_asset(config.model.clone()))),
  model_transform(body.float_height, config.scale()))).observe(on_model_ready); });`
- `#[derive(Component)] pub(super) struct CharacterAnimator { pub(super) character: Entity, pub(super) shown: AnimState }`.
- `on_model_ready(ready: On<WorldInstanceReady>, models: Query<&ChildOf, With<CharacterModel>>, children: Query<&Children>, mut players: Query<&mut AnimationPlayer>, meshes: Query<(&GltfMeshName, &MeshMaterial3d<StandardMaterial>)>, animations: Res<CharacterAnimations>, config: Res<CharacterVisualConfig>, mut materials: ResMut<Assets<StandardMaterial>>, mut commands: Commands)`:
  - `let Ok(child_of) = models.get(ready.entity) else { return }; let character = child_of.parent();`
  - По `children.iter_descendants(ready.entity)`; тело цикла разнести на два helper-а, чтобы вложенность осталась < 2:
    - `wire_player`: если у потомка есть `AnimationPlayer` (glTF-загрузчик кладёт его на корень анимации,
      `bevy_gltf-0.19.1/src/loader/mod.rs:1086-1094`; у Kenney это узел `character-<name>`):
      `let mut transitions = AnimationTransitions::new(); transitions.play(&mut player, animations.nodes[AnimState::Idle as usize], Duration::ZERO).repeat();`
      вставить `(AnimationGraphHandle(animations.graph.clone()), transitions, CharacterAnimator { character, shown: AnimState::Idle })`.
      Посчитать подключённых.
    - `tint_mesh`: если `config.tint == (1.0, 1.0, 1.0)`, ничего не делать. Если `GltfMeshName.0 == config.tinted_mesh`
      (`GltfMeshName` висит на сущности примитива, `loader/mod.rs:1722-1724`; `MeshMaterial3d` туда же ставит
      `bevy_pbr-0.19.1/src/gltf.rs:141-150`): `let Some(mut material) = materials.get(&handle.0).cloned() else { error!("gltf character model: material of {mesh} not loaded, tint skipped"); return };`
      `let base = material.base_color.to_linear();`
      `material.base_color = Color::linear_rgba(base.red * r, base.green * g, base.blue * b, base.alpha);`
      вставить на этот примитив новый `MeshMaterial3d(materials.add(material))`. Материал `head-mesh` общий с body
      (материал 0), поэтому клон обязателен: голова остаётся на исходном.
  - Если подключено 0 плееров: `error!("gltf character model {}: no AnimationPlayer in the scene", ready.entity)`. Слова
    `ERROR` и `gltf` ловит фильтр лога QA.
- `drive_character_animation(characters: Query<(&AnimState, &LinearVelocity)>, mut animators: Query<(&mut CharacterAnimator, &mut AnimationPlayer, &mut AnimationTransitions)>, animations: Res<CharacterAnimations>, config: Res<CharacterVisualConfig>)`:
  ```rust
  for (mut animator, mut player, mut transitions) in &mut animators {
      let Ok((state, velocity)) = characters.get(animator.character) else { continue };
      let node = animations.nodes[*state as usize];
      if *state != animator.shown {
          transitions.play(&mut player, node, Duration::from_secs_f32(config.blend_seconds)).repeat();
          animator.shown = *state;
      }
      let Some(active) = player.animation_mut(node) else { continue };
      let horizontal = Vec2::new(velocity.x, velocity.z).length();
      active.set_speed(playback_rate(*state, horizontal, &config));
  }
  ```
  Персонаж пропал (despawn) → `get` вернул `Err` → `continue`, без паники. `AnimationTransitions::play` перезапускает
  клип через `player.start` и гасит старый за `Duration` (`bevy_animation-0.19.1/src/transition.rs:78-100`).
  `advance_transitions` регистрирует `AnimationPlugin` в `PostUpdate` (`lib.rs:1288-1305`).
- `pub(super) fn playback_rate(state: AnimState, horizontal_speed: f32, config: &CharacterVisualConfig) -> f32`:
  `Walk` → `config.walk`, `Run` → `config.run`, `Sprint` → `config.sprint`: `horizontal_speed / (clip.native_speed * config.scale())`;
  `Idle/Jump/Fall` → `1.0`.
- `src/visuals/mod.rs`: `mod character; mod character_config; #[cfg(test)] mod character_gate;`,
  `pub use character_config::{CHARACTER_VISUAL_CONFIG, CharacterClips, CharacterVisualConfig};`, добавить
  `character::CharacterVisualsPlugin` в `add_plugins` (`:35-39`); удалить `visualize_character` (`:85-116`) и
  `.add_observer(visualize_character)` (`:41`); из импорта `:17` убрать `character::CharacterBody` (останется
  `world::Block`). Gameplay-композицию не менять.

### Шаг 10. Клиент: сборка (`src/main.rs`)

- Импорт `visuals::{CHARACTER_VISUAL_CONFIG, CharacterClips, CharacterVisualConfig, ...}`.
- После загрузки `render_config` (`:111-117`) загрузить `character_config` через `load_config::<CharacterVisualConfig>(&root, CHARACTER_VISUAL_CONFIG)` тем же `match`.
- `preflight(root, render_config, character_config) -> Result<CharacterClips, Vec<String>>`:
  1. как сейчас `render_config.validate()`;
  2. `character_config.validate()` → `vec![format!("{}: {message}", root.path(CHARACTER_VISUAL_CONFIG).display())]`;
  3. как сейчас загрузка и `validate` манифеста;
  4. к списку `unlisted` добавить модель: если `!manifest.contains_asset(&character_config.model)` →
     `"{CHARACTER_VISUAL_CONFIG}: model {path} is not listed in {THIRD_PARTY_MANIFEST}"` (поле `model` читать через
     `pub(super)`-геттер или сделать `pub(crate)` — на выбор имплементера, минимально);
  5. `let clips = character_config.resolve(&manifest).map_err(|errors| errors.into_iter().map(|e| format!("{CHARACTER_VISUAL_CONFIG}: {e}")).collect::<Vec<_>>())?;`
  6. как сейчас `missing_files`; вернуть `Ok(clips)`.
- `app.insert_resource(camera_config).insert_resource(render_config).insert_resource(character_config).insert_resource(clips)`
  строго до `.add_plugins((.., VisualsPlugin, ..))`: `CharacterAnimations::from_world` читает оба ресурса при сборке плагина.

### Шаг 11. Клиентские гейты (`src/visuals/character_gate.rs`, `#[cfg(test)]`, `cargo test -p gta_like --bin gta_like`)

Хелперы по образцу `city_gate.rs`: `assets_root()`, загрузка шипованных `visual.ron` и манифеста с паникой
`GATE BROKEN: ...`. Headless-приложение `character_visuals_app()`:
`App::new()` + `(MinimalPlugins, TransformPlugin, AssetPlugin::default(), StatesPlugin)` +
`TimeUpdateStrategy::FixedTimesteps(1)` + заглушки ассетов `init_asset::<Mesh>()`, `StandardMaterial`, `Image`,
`WorldAsset`, `AnimationClip`, `AnimationGraph` (без них `asset_server.load` паникует: `bevy_asset-0.19.1/src/server/info.rs:815-823`);
`compose_sim(&mut app, sim_assets_root, WorldSource::TestArea)`; `insert_resource(config)`, `insert_resource(clips)`
(из `resolve` шипованного манифеста); `add_plugins(CharacterVisualsPlugin)` (production-плагин); `finish(); cleanup();`
`update()` до появления `Player` (не больше 10 раз, иначе `GATE BROKEN`). Загрузка `.glb` в этом приложении
заканчивается ошибкой "нет загрузчика" в логе, это ожидаемо: гейт проверяет ECS-проводку, а не glTF.
`WorldInstanceReady` вручную не вызвать (`InstanceId::new` приватный, `world_asset_spawner.rs:52-58`), поэтому
`on_model_ready` здесь не проверяется. Его держит QA (шаг 12).

Тесты:
1. `character_visuals_reference_manifest_rig` (корректность данных): шипованный `visual.ron` → `validate()` ok;
   `resolve(manifest)` → `CharacterClips([1, 2, 3, 3, 4, 5])` (клипы `idle, walk, sprint, sprint, jump, fall`;
   порядок из аудита). Тот же тест фиксирует `AnimState::Idle as usize == 0 … Fall as usize == 5`.
2. `model_faces_body_forward` (корректность, три направленных примера): лицо модели в её пространстве `+Z`.
   `R_y(θ)·(x, y, z) = (x·cosθ + z·sinθ, y, −x·sinθ + z·cosθ)`.
   - yaw 0: `R_y(0)·R_y(π)·(0,0,1) = (0,0,−1)` = `move_direction(Vec2::Y, 0)`;
   - yaw 90°: `R_y(90°)·(0,0,−1) = (−1·sin90°, 0, −cos90°) = (−1,0,0)` = `move_direction(Vec2::Y, 90°)`;
   - yaw 180°: `R_y(180°)·(0,0,−1) = (0,0,1)` = `move_direction(Vec2::Y, 180°)`.
   Проверка: `(Quat::from_rotation_y(yaw) * model_transform(1.05, 2.68).rotation * Vec3::Z)` ≈
   `gta_sim::character::move_direction(Vec2::Y, yaw)` (допуск 1e-5). Tnua поворачивает −Z тела к `desired_forward`
   (`bevy-tnua-0.32.0/src/builtins/walk.rs:477-482`), `drive_characters` передаёт туда направление движения.
3. `model_feet_on_ground`: `model_transform(1.05, s).translation == (0, −1.05, 0)`, `scale == splat(s)`.
4. `playback_rate_worked_example` (шипованный `visual.ron`, scale 2.68128, допуск 1e-3):
   Walk 1.8 → 1.8 / (1.28·2.68128 = 3.4320) = 0.5245; Run 4.5 → 4.5 / (2.66·2.68128 = 7.1322) = 0.6309;
   Sprint 6.8 → 6.8 / 7.1322 = 0.9534; Idle/Jump/Fall → 1.0.
5. `character_model_spawns_under_player` (liveness production-плагина): в `character_visuals_app()` ровно одна сущность
   `CharacterModel`; её `ChildOf.parent()` = игрок; `WorldAssetRoot.0.path()` в строке =
   `"third_party/mini-characters/character-male-a.glb#Scene0"`; `Transform` = `model_transform(1.05, 1.8/0.67132)`;
   у игрока нет дочерних `Mesh3d` (капсула-заглушка удалена).
6. `graph_nodes_follow_manifest_clips` (корректность графа): `Assets<AnimationGraph>` по `CharacterAnimations.graph`;
   для каждого i из 0..6 `graph.get(nodes[i])` → `AnimationNodeType::Clip(handle)`, `handle.path()` в строке =
   `"third_party/mini-characters/character-male-a.glb#Animation{clips[i]}"` (т.е. `#Animation1, #Animation2,
   #Animation3, #Animation3, #Animation4, #Animation5`).
7. `animator_follows_anim_state` (корректность `drive_character_animation`): в `character_visuals_app()` 64 раза
   `update()` (игрок стоит); заспавнить заглушку аниматора `(AnimationPlayer::default(), AnimationTransitions::new(),
   CharacterAnimator { character: player, shown: AnimState::Idle })`; у игрока `MoveIntent { axis: Vec2::Y, gait: Gait::Run, .. }`;
   делать `update()` (не больше 64 раз) до `AnimState::Run`, затем ещё один `update()`. Прочитать `LinearVelocity`
   игрока, `h = |(vx, vz)|` (≈ 4.5). Ожидание: `transitions.get_main_animation() == Some(nodes[Run as usize])`,
   `|player.animation(nodes[Run]).speed() − playback_rate(Run, h, cfg)| < 1e-4` (≈ 0.631 при h = 4.5). Затем
   `gait = Sprint`, то же до `AnimState::Sprint`: main = `nodes[Sprint]`, speed ≈ `h / 7.1322` (≈ 0.953 при 6.8).
   Скорость после `update()` равна той, что видел `drive` в `Update`: `FixedUpdate`/`FixedPostUpdate` идут раньше
   `Update`, после `Update` скорость никто не пишет.

### Шаг 12. QA-сценарий `tools/qa/scenarios/t4.py` (новый, по образцу `t3.py`)

1. `fetch_assets.py --check` проходит, иначе `AssertionError("run python tools/fetch_assets.py first")`.
2. `Game(features=("dev",), args=("--seed", "1"), release=True)` (как `t3.py`); `wait_resource("CityLayoutHash", 180)`;
   дождаться одной строки `Player`.
3. Готовность модели (таймаут 60 с): `game.query([CharacterModel], with_=[CharacterModel])` → ровно 1; запрос с
   `with_=[AnimationPlayer, AnimationGraphHandle, AnimationTransitions]` (пути через `component_path`) → ≥ 1. Это
   liveness `on_model_ready` на реальном GLB.
4. Скопировать из `t3.py` `resource_value`, `player_position`, `teleport`, `face`, `log_errors`, `screenshot` (общий
   модуль не заводить: два сценария). `ERROR_WORDS = ("gltf", "asset", "Failed to load", "animation")`.
   `park = CityLandmarks.park_center`; `face(game, (0.0, -1.0), -15.0)` (камера смотрит по −Z, W ведёт по −Z).
5. `AnimState` читать через `game.call("world.get_components", {"entity": player, "components": [path], "strict": True})`,
   принять и `{"components": {path: value}}`, и `{path: value}`; значение — строка варианта (`"Run"`).
6. Фазы; перед каждой телепорт на `park + (0, 1.2, 0)` и ожидание `"Idle"` (таймаут 3 с):
   | фаза | `send_keys` | удержание, мс | наблюдение, мс | ожидаемо |
   |---|---|---|---|---|
   | run | `["KeyW"]` | 1200 | 1500 | `Run` |
   | sprint | `["KeyW", "ShiftLeft"]` | 1200 | 1500 | `Sprint` |
   | walk | `["KeyW", "AltLeft"]` | 1200 | 1500 | `Walk` |
   | jump | `["Space"]` | 150 | 2000 | `Jump` → `Fall` → `Idle` |
   В фазе: цикл опроса с целевым шагом 50 мс; каждый отсчёт = `(t_ms от возврата send_keys, AnimState, Position)`;
   скриншот, когда `t` пересекает очередную отметку кратную 200 мс, с записью фактического `t` и интервала до
   предыдущего снимка (`brp_extras/screenshot` без ожидания файла; PNG проверить в конце, как `screenshot()` в `t3.py`).
   Длина пути: спринт 6.8 × 1.2 = 8.2 м от центра парка.
7. Критерии:
   - gait-фазы: все отсчёты с `400 ≤ t ≤ 1100` равны ожидаемому. Если нет, сначала проверить маршрут: горизонтальное
     смещение `Position` между первым и последним отсчётом окна ≥ 0.8·v·Δt (v = 1.8 / 4.5 / 6.8). Не выполнено →
     `AssertionError("route blocked in phase …")` (правка маршрута, не дефект игры). Выполнено → `AssertionError` с выборкой.
   - jump: в выборке есть `Jump`, после него `Fall`, все отсчёты последних 300 мс = `Idle`.
   - `log_errors` пуст.
   Окна выведены из данных: разгон ≤ 0.23 с, coyote 0.12 с, отпускание на 1200 мс. Если реальный прогон выявит другое,
   окна и маршрут подбираются по записанным временам и позициям. Headless-гейты шагов 6-7 при этом не ослабляются.
8. `summary.json` в `--out` (по умолчанию `target/qa/t4`): отсчёты по фазам, снимки с фактическими `t` и интервалами,
   смещения маршрута, найденные строки аниматора, ошибки лога. Сырые данные `AnimationPlayer` — только как доказательство,
   без ассертов.

### Шаг 13. Правка GDD (`docs/design/GDD.md`), принята в `TASK_FINAL.md`

- §9.2, строка `:407`: фрагмент "12 skinned-персонажей (male/female a-f), 14 костей, 32 клипа внутри каждого GLB" →
  "12 skinned-персонажей (male/female a-f), 7 суставов (два skin на один скелет), 32 клипа внутри каждого GLB (клипа run нет)";
  фразу "T4 перепроверяет их на скачанном файле" → "T4 записал фактический состав в `rig` манифеста".
- §13 T4, строка `:589`: "перепроверить 12 персонажей / 14 костей / 32 клипа и записать в манифест" →
  "записать в `rig` манифеста фактический состав архива (12 персонажей / 7 суставов / 32 клипа)".
Скоуп не меняется, других строк GDD не трогать.

### Шаг 14. Документация по окончании

README/AGENTS "Статус" и `docs/narrative-graph.md` синхронизирует оркестратор после QA. Имплементер их не трогает.

---

## 3. Test plan

### 3.1 Команды (все зелёные)

```
python tools/fetch_assets.py --cache maw/tasks/in_progress/TASK-005/scratch
python tools/fetch_assets.py --check
python tools/fetch_assets.py --validate-only crates/gta_sim/tests/fixtures/manifest/<каждая из 5 фикстур шага 3>
cargo build
cargo clippy -- -D warnings
cargo clippy -p gta_sim --tests -- -D warnings
cargo clippy -p gta_like --tests -- -D warnings      # новый клиентский тест-код; чужие старые предупреждения — отметить, не чинить
cargo test -p gta_sim -p citygen                      # не --workspace (AGENTS.md: унификация фич)
cargo test -p gta_like --bin gta_like
python tools/qa/tree_check.py
cargo tree -p gta_sim -e features -i bevy_render      # пусто
python tools/qa/scenarios/t4.py --out target/qa/t4    # QA-стадия
```
`citygen` не затрагивается, его тесты прогоняются как существующие.

### 3.2 Что каким гейтом доказано

| Утверждение | Гейт | Класс |
|---|---|---|
| Таблица `AnimState`, границы, горизонтальность | `anim_state_table` (unit, `gta_sim`) | корректность |
| Система пишет `AnimState` в собранном App | `idle_after_settle` | liveness |
| Походки → состояния в собранном App | `gaits_map_to_states` | корректность |
| Прыжок: Jump → Fall → Idle | `jump_goes_up_then_falls_then_lands` | порядок |
| Состояние считается по скорости после шага физики | `anim_state_matches_post_step_velocity` | корректность порядка систем |
| Пороги анимации валидны | `locomotion_thresholds_are_validated`, `shipped_locomotion_config_loads` | корректность |
| Схема рига, `None`/отсутствие | фикстуры + `manifest_fixtures_are_judged` + `--validate-only` | корректность, паритет Rust/Python |
| Запись рига шипованного пакета | `shipped_manifest_is_valid` | корректность записи |
| Запись рига = байты GLB | `fetch_assets.py --check` / `extract` | корректность |
| Файлы/SHA/лицензия на диске | `local_assets_match_manifest` | корректность |
| `visual.ron` ↔ риг манифеста | `character_visuals_reference_manifest_rig` | корректность |
| Разворот модели, ступни, скорость клипа | `model_faces_body_forward`, `model_feet_on_ground`, `playback_rate_worked_example` | корректность |
| Плагин спавнит модель под игроком, капсулы нет | `character_model_spawns_under_player` | liveness |
| Узлы графа = клипы манифеста | `graph_nodes_follow_manifest_clips` | корректность |
| Аниматор следует `AnimState`, rate = формула | `animator_follows_anim_state` | корректность |
| Реальный GLB: плеер, граф, переходы подключены, ошибок загрузки нет | `t4.py` шаги 3, 7 | liveness (runtime) |
| `AnimState` в живой игре по W / Shift+W / Alt+W / Space | `t4.py` фазы | корректность (runtime) |
| Вид, тинт, рост, направление, ноги не скользят | owner-run | владелец |

### 3.3 Flip-RED (испортить → RED → вернуть → GREEN; в IMPL_SUMMARY записать, что портили и какой тест покраснел)

1. `anim_state`: `velocity.y > 0.0` → `>= 0.0` → RED строка апекса; горизонталь → `velocity.length()` → RED строка `(0, 3.0, −2.5)`;
   `(walk + run) / 2` → `run_speed` → RED строки 3.16 и `(0,0,−4.5)`.
2. Не регистрировать `update_anim_state` → RED все четыре теста `tests/anim_state.rs`.
3. Перенести `update_anim_state` в `FixedUpdate.after(TnuaSystems)` → ожидается RED `anim_state_matches_post_step_velocity`
   на тике апекса. Если тест остался зелёным, записать это как факт ("порядок относительно шага физики этим тестом не
   наблюдается") и сообщить ревью. Не строить под это дополнительную машинерию и не выдавать тест за гейт порядка.
4. `locomotion.ron`: `anim_idle_speed: 2.0` → RED `compose_sim` во всех headless-тестах с сообщением про поле; вернуть.
5. Манифест: удалить `"crouch"` из `rig.clips` → RED `shipped_manifest_is_valid` (32) и `--check` (клипы расходятся с GLB).
   Поменять местами `"walk"` и `"sprint"` → RED `shipped_manifest_is_valid` (`clip_index`) и `--check` (порядок).
6. Манифест: в `rig.joints` `"head"` → `"neck"` → `--check` RED (сообщение называет модель и поле `joints`); Rust зелёный
   (схема цела) — это граница ответственности.
7. `bad_rig_model.ron`: временно добавить `missing.glb` в `files` → RED `manifest_fixtures_are_judged` ("must fail validate");
   `bad_rig_duplicate_clip.ron`: убрать дубль → RED. Python: убрать ветку `None` → `--validate-only valid_rig_none.ron` падает.
8. Клиент: `MODEL_YAW = 0.0` → RED `model_faces_body_forward` на всех трёх yaw.
9. `visual.ron`: `run: (clip: "run", …)` → RED `character_visuals_reference_manifest_rig` и отказ `preflight` с именем клипа.
10. Убрать `add_observer(spawn_character_model)` → RED `character_model_spawns_under_player`.
11. В `CharacterAnimations::from_world` взять `index + 1` → RED `graph_nodes_follow_manifest_clips`.
12. В `drive_character_animation` не вызывать `set_speed` → RED `animator_follows_anim_state` (1.0 против 0.631);
    отдельно `node = nodes[0]` → RED по main animation.

### 3.4 Owner checklist (QA переносит в QA_REPORT.md; до прогона владельца пункт открыт, автоматического PASS по картинке нет)

- `python tools/fetch_assets.py` (или с `--cache`), `cargo run --features fast`.
- Вместо капсулы стоит человечек Kenney ростом примерно с верх бывшей капсулы (1.8 м), ступни на земле, не утоплены и не висят.
- W: бег, модель смотрит по ходу (не спиной вперёд), ноги заметно не скользят.
- Shift+W: спринт; Alt+W: ходьба; ноги не скользят.
- Space: поза прыжка на взлёте, падения на спуске, приземление в покой. Сход с бордюра даёт падение после короткой задержки (coyote 0.12 с), это нормально.
- Переходы без рывков (`blend_seconds` 0.15 с). Run↔Sprint на одном клипе перезапускают цикл через кроссфейд: возможен лёгкий "дёрг", это не блокер.
- Тинт: в `assets/character/visual.ron` поставить `tint: (1.0, 0.35, 0.35)`, перезапустить: одежда краснеет (кисти тоже, ограничение палитры), голова нет; вернуть `(1.0, 1.0, 1.0)`.
- Крутилки, если ноги едут: `walk/run/sprint.native_speed` (меньше → ноги быстрее; диапазон подбора 0.92..1.28 и 1.51..2.66 ед/с), `run.clip` (`"sprint"` или `"walk"`), `anim_idle_speed` в `locomotion.ron`.

---

## 4. Rollout notes

- Миграций данных, env-переменных и фич-флагов нет. Зависимости и `Cargo.lock` не меняются (клиент уже тянет
  `bevy_gltf`/`bevy_animation`/`bevy_world_serialization` через `DefaultPlugins`; `gta_sim` их не получает).
- Новые файлы данных: `assets/character/visual.ron`; изменены `assets/character/locomotion.ron` (+`anim_idle_speed`) и
  `assets/third_party/manifest.ron` (+пакет). `LocomotionConfig` строгий: любой старый `locomotion.ron` без нового поля
  больше не грузится. Других копий в рабочем дереве нет (literal-конструкторов `LocomotionConfig { .. }` в `src/` и
  `crates/` нет).
- После правки манифеста у всех, у кого стоят только city-пакеты, `local_assets_match_manifest` краснеет ("partial install"),
  а клиент не стартует (`preflight`: missing file), пока не запущен `python tools/fetch_assets.py`. Это задуманное
  поведение T3. Имплементер и QA ставят пакет из `--cache maw/tasks/in_progress/TASK-005/scratch`.
- Бинарники (`*.glb`, `*.png`, `*.zip`, `assets/third_party/<pack>/`) в git не попадают. Перед коммитом проверить
  `git add -A --dry-run`: из третьих ассетов в список может попасть только `manifest.ron`.
- Совместимость манифеста: `rig` опционален, старые фикстуры и пакеты без поля валидны в Rust и Python.
- Совместимость BRP/QA: `AnimState` и `CharacterModel` — новые отражаемые компоненты; `component_path("AnimState")`
  уникален (у tnua свой `TnuaAnimatingState`).
- Кодировка: все новые RON/Python/Rust-файлы UTF-8 без BOM, LF (`.gitattributes`).

---

## 5. Review notes

**Проверенный контрпример (disconfirmation).** Главное исправление V2 — перенос `update_anim_state` в
`FixedPostUpdate.after(PhysicsSystems::Last)`. Оно было бы ошибкой, если бы Avian 0.7.0 шагал в `FixedUpdate` или если бы
`PhysicsSystems::Last` не был упорядочен в этом расписании. Проверил: `impl Default for PhysicsPlugins` →
`Self::new(FixedPostUpdate)` (`avian3d-0.7.0/src/lib.rs:751-754`), наборы `First..Last` сцеплены `.chain()` в этом
расписании (`schedule/mod.rs:74-85`), `compose_sim` использует `PhysicsPlugins::default()` (`lib.rs:35`). Контрпример не
подтвердился, исправление V2 верно. Попутно выяснилось: горизонтальную скорость Tnua пишет boost-ом до физики
(`vendor/.../lib.rs` `apply_motors_system`), поэтому порядок систем виден только по вертикали. Отсюда новый тест на
прыжке (шаг 7.4) вместо проверки на разгоне.

Что изменено относительно PLAN_V2 (V2 побеждает PLAN.md во всех конфликтах; детали PLAN.md восстановлены там, где V2 их
сжал без исправления: полный блок манифеста, сигнатуры, таблица случаев, числа `native_speed`, flip-RED, owner checklist):

1. **`WorldInstanceReady` и `GltfMeshName` не в `bevy::prelude`.** PLAN.md утверждал обратное для `WorldInstanceReady`.
   Prelude `bevy_world_serialization` экспортирует только `WorldAssetRoot`/`WorldAsset`/…
   (`bevy_world_serialization-0.19.1/src/lib.rs:38-44`), prelude `bevy_gltf` — только `Gltf`, `GltfExtras`,
   `GltfAssetLabel` (`bevy_gltf-0.19.1/src/lib.rs:156-159`). Шаг 9 даёт явные пути импорта.
2. **Headless-гейт `on_model_ready` невозможен в предложенном V2 виде.** `InstanceId::new` приватный
   (`world_asset_spawner.rs:52-58`), `WorldInstanceReady` руками не вызвать. Клиентский гейт V2 переопределён: production
   `CharacterVisualsPlugin` в headless-`App` проверяет спавн модели под игроком, содержимое графа и работу
   `drive_character_animation` (шаг 11, тесты 5-7). Подключение плеера/графа и тинт на реальном GLB держит `t4.py` по BRP
   плюс `error!` в `on_model_ready` для фильтра лога. Урок предложен в `PCTX_PROPOSALS.md`.
3. **Граф собирается через `FromWorld` при сборке плагина, не в `Startup`.** V2 требовал не паниковать из-за
   отсутствующего ресурса в observer-е. `init_resource::<CharacterAnimations>()` (прецедент `CharacterControlConfig`)
   убирает зависимость от порядка `Startup` / `OnEnter(Playing)`: ресурс есть до любого спавна, а отказ сдвигается на
   старт приложения с именем типа.
4. **Гейт порядка систем пересчитан с числами.** V2 предлагал проверить, что состояние соответствует скорости после
   шага физики, но не сказал как. Горизонталь Tnua приходит boost-ом до физики и порядок не различает. Проверка идёт
   по каждому тику прыжка (знак `vy` у апекса меняет гравитация в шаге Avian). Flip — перенос в `FixedUpdate`. Если
   flip не краснеет, это фиксируется честно, машинерию не строить.
5. **Где валидировать пороги.** V2 сказал "при загрузке/композиции". Конкретно: `LocomotionConfig::validate` вызывается
   в `compose_sim` сразу после загрузки (как `CityParams`), плюс тест с испорченным файлом.
6. **Python `None`.** V2 верно заметил пробел. Шаг 4 даёт точную правку `Parser.value` и фикстуру `valid_rig_none.ron`.
7. **QA-сценарий.** По замечаниям V2 фазы сокращены до 1200 мс (путь ≤ 8.2 м), перед каждой фазой телепорт в центр
   парка, ассерт идёт по окну 400..1100 мс. Провал маршрута отделён от провала состояния проверкой смещения. Записываются
   фактические времена снимков. Добавлен `release=True` как в `t3.py`. Коллайдеров у деревьев парка нет (коллайдеры
   только в `world/city.rs`, `world/test_area.rs`).
8. **Тинт.** Умножение исходного линейного `base_color` на tint (V2); клон материала обязателен, потому что `body-mesh` и
   `head-mesh` делят материал 0 (подтверждено пробой `scratch/reviewer2/probe_glb.txt`).
9. **Открытые вопросы PLAN.md сняты.** Правка GDD, тинт, клип Run и модель решены в `TASK_FINAL.md`. Шаг 13 выполняется
   безусловно, с точными строками.
10. **Команды проверки.** `cargo test -p gta_sim -p citygen` (не `--workspace`, AGENTS.md), добавлены
    `cargo clippy -p gta_like --tests` для нового клиентского тест-кода и прогон `--validate-only` по всем пяти фикстурам.
11. **Unit-таблица.** Добавлены границы 0.2 и 5.65. Конфиг теста — `include_str!` шипованного `locomotion.ron`, без
    файловой плюмбинги.

Остаточные риски (помечены для владельца, не гейтятся):
- `native_speed` посчитан по модели жёсткого маятника. Мультяшный клип может читаться иначе. Числа лежат в данных,
  диапазон подбора указан.
- Подскок `root` в `sprint` 0.2 ед. = 0.54 м при масштабе 2.68. На Run (rate 0.63) может смотреться "плавающе".
  Лечится одной строкой `run.clip = "walk"`.
- Тинт красит кисти и шею (UV `body-mesh` попадают в тексели кожи). Принято в `TASK_FINAL.md`.
- `is_airborne` Tnua отстаёт от скорости на тик и включает coyote 0.12 с: сход с уступа даёт `Fall` с задержкой. Это
  видно владельцу, порог без прогона не добавляем.
- Маршрут QA в парке seed 1 не измерен заранее. Сценарий различает "маршрут заблокирован" и "неверное состояние".

children: 0 launched / 0 reported.
