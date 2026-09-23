# QA_REPORT — TASK-004 (GDD T3: облик города)

Ветка `feature/t03-city-look`, HEAD `53be855`. QA работала in-place в `D:/test-gta-like`.

## 1. Environment

- docker-compose нет, dev-сервера нет. Использованы cargo test runner (headless) и реальная сборка игры
  с окном через BRP (`--features dev`, release). Моков нет.
- Хост: i9-11900K, RTX 4070 Ti (Vulkan), Windows 11. Пакеты Kenney уже лежали в `assets/third_party/`
  (не tracked), `python tools/fetch_assets.py --check` = 0.
- Воспроизведение:
  ```
  cargo build
  cargo clippy -- -D warnings
  cargo clippy --workspace --all-targets -- -D warnings
  cargo test --workspace
  python tools/qa/scenarios/t3.py --out maw/tasks/in_progress/TASK-004/scratch/qa/t3_qa
  python maw/tasks/in_progress/TASK-004/scratch/qa_novsync_probe.py
  python maw/tasks/in_progress/TASK-004/scratch/qa_publish_probe.py tools/fetch_assets.py
  ```
- Сервисы и контейнеры не запускались. Игра запускалась 3 раза (t3, novsync probe, preflight), каждый
  раз закрыта через `brp_extras/shutdown` или сама вышла с кодом 1. `tasklist` после прогонов:
  процесса `gta_like` нет.

## 2. Test results

### Существующий набор
- `cargo build`: green. `cargo clippy -- -D warnings`: green. `cargo clippy --workspace --all-targets -- -D warnings`: green.
- `cargo test --workspace`: 0 failed. citygen lib 8, golden 3 (+1 ignored bless), perf 0 (+1 ignored),
  properties 11; gta_like bin 7; gta_sim lib 1, asset_manifest 3, city 6 (+1 ignored budget), config 4,
  jump 3, movement 4, terrain 2. Все зелёные, новых падений нет. Ignored-тесты те же, что задуманы
  (bless, perf, budget).

### Моя проверка (независимые пробы и перепроверки)
| Проба | Результат |
|---|---|
| Flip-RED гейта слияния: `ChunkGrid::new` `ceil` -> `floor` (`src/visuals/city_mesh.rs:30`) | RED: `chunk entities with Mesh3d left 81 right 100`. Файл восстановлен `git checkout`, sha256 совпал |
| Деривация гейта: `render.ron` `chunk_size 128 -> 100` | гейт зелёный (ожидание 12² = 144 считается из данных, не захардкожено). Восстановлено |
| SHA-256 всех 12 файлов и 3 архивов Kenney, посчитано своим скриптом (не тестом автора) | все совпадают с `manifest.ron` |
| Бинарники в git: `git ls-files` + имена файлов во всех коммитах ветки | tracked только `assets/third_party/manifest.ron`, GLB/PNG/zip нет ни в одном коммите |
| Свежий клон: все `city-kit-*` убраны | `asset_manifest`: `SKIP file check ... run python tools/fetch_assets.py`, 3 passed; `city_gate` 2 passed. Восстановлено, `--check` = 0 |
| Preflight: убран `city-kit-roads`, запуск бинарника | exit 1, 4 строки `missing third-party asset ...; run python tools/fetch_assets.py`, окна нет |
| Фикс fixer'а (атомарная публикация пакета): своя проба `scratch/qa_publish_probe.py` с отказом второго `os.replace` | на HEAD: старый пакет на месте, нет `.old-`/`.tmp-`, повтор ставит новый. На версии до фикса (`7ba8415`): пакет пропал (FileNotFoundError). Фикс настоящий |
| Новые `const` с настроечными числами в диффе | нет. Новые const: `HASH_SCHEMA_VERSION`, пути (`THIRD_PARTY_*`, `RENDER_CONFIG`, `FACADE_SHADER`), `NO_UV` sentinel, `FIT_STEPS/FIT_EPS` (алгоритмические пределы). Числа вида, пропов, тумана, неба, разметки, фасада в `render.ron`; бордюр, уступы, ориентиры в `city.ron` |
| Dead_end из лога (rustfmt задел `roads.rs`) | `crates/citygen/src/roads.rs` не входит в дифф ветки. Подтверждено |

### Runtime t3 (BRP, release + dev, seed 1)
`python tools/qa/scenarios/t3.py --out .../scratch/qa/t3_qa` — exit 0.
- `CityLayoutHash 0xd2158922f1e5cd6f` = golden. Заспавнено 100 `CityChunk`, 4237 `CityProp`.
- Крыша башни y 156.0; игрок на 4 точках крыши стоит на y 157.05 (compound-коллайдер держит). Парк: y 1.20.
- `log_errors: []` (нет ошибок wgsl/shader/gltf/asset).
- FPS в сценарии (Fifo, окно без фокуса): min 57.9, avg 59.2, max frame time 17.3 ms. Это частота
  дисплея под vsync, не стоимость кадра (у имплементера на этой машине было ~141 при Fifo, значит
  монитор и/или его частота сейчас другие).
- Стоимость кадра: `scratch/qa_novsync_probe.py` ставит `PresentMode::AutoNoVsync` через BRP
  `world.mutate_components` (без правки кода), обзор с края крыши на север, pitch −45°, 8 замеров после
  6 с прогрева: FPS min 316, avg 414; frame time 1.9–3.2 ms, среднее ~2.4 ms. Файл `scratch/qa/t3_qa/novsync.json`.
  Доказательство для владельца, не гейт.

Скриншоты (`maw/tasks/in_progress/TASK-004/scratch/qa/t3_qa/`), смотрел сам:
- `roof_north.png`: с крыши виден город. Слева серо-синие высотки даунтауна с сеткой окон, дальше
  лососевые жилые дома, два зелёных парка, дороги с перекрёстками, тени от зданий, дальний план уходит
  в туман без шва.
- `roof_west.png`: плотный даунтаун, у высоток видны уступы-ярусы, окна по этажам, справа жилой район в тумане.
- `park.png`: зелёная трава парка, деревья Kenney с тенями, игрок стоит. Стволы деревьев выглядят
  розово-оранжевыми: это цвет палитры Kenney или сэмплинг текстуры, по одному кадру не определить. Отмечено для владельца.
- `roof_east.png`, `roof_south.png` тоже сняты (PNG валидные).

## 3. Acceptance criteria

| Criterion | Test performed | Result |
|---|---|---|
| Headless: манифест валиден (SHA-256, лицензия, состав на пакет) | `asset_manifest` 3 теста (фикстуры, shipped, локальные файлы); своя пересчётка SHA-256 12 файлов + 3 zip; SKIP на свежем клоне | PASS |
| Headless: число чанков = f(размер города, `render.ron`), ловит забытое слияние | `city_meshes_are_merged_per_chunk` через production `CityVisualsPlugin` + `compose_sim`; мой flip-RED (81 vs 100); деривация при chunk 100 | PASS |
| Mesh-merge gate из Resolved questions: нет per-building `Mesh3d`, мешей = чанков | тот же гейт: `With<Mesh3d>, With<CityBuilding>` = 0, `Mesh3d Without<CityProp>` = n². Ограничение ниже (Bug 2) | PASS |
| Runtime QA: `t3.py` существует и проходит через `brp.py`; крыша, парк, скриншоты, FPS | прогон t3, exit 0, скриншоты осмотрены, FPS записан + отдельный замер без vsync | PASS |
| Owner checklist в QA_REPORT | раздел 6 | PASS (записан; сам критерий принимает владелец) |
| Новые настроечные числа в data-файлах §12, не `const` | grep добавленных `const` и литералов в диффе | PASS |
| `cargo build`, `clippy -D warnings`, `cargo test -p gta_sim`, `-p citygen` зелёные | прогнаны | PASS |
| Existing tests pass | `cargo test --workspace` 0 failed | PASS |
| Owner rule: бинарники не в git, fetch + понятная ошибка | `git ls-files`, история ветки, preflight exit 1 с командой | PASS |

## 4. Bugs found

Блокирующих дефектов нет. Замечания:

1. **Minor (QA-инструмент, не игра).** `tools/qa/brp.py --help` не печатает справку: `__main__`
   собирает и запускает окно игры. Проектный контекст советует именно `--help`. Не дефект этой задачи;
   предложение добавлено в `PCTX_PROPOSALS.md`.
2. **Minor (граница гейта).** Гейт слияния собирает только `CityVisualsPlugin`, без `VisualsPlugin`
   (тот тянет `MaterialPlugin`, в headless его нет). Если кто-то вернёт per-building `Mesh3d` через
   наблюдатель в `VisualsPlugin` (как сейчас `visualize_block` для `Block`), гейт останется зелёным. Сейчас
   такого кода нет (`src/visuals/mod.rs`), правило "вся геометрия города только в `CityVisualsPlugin`"
   записано в плане. Воспроизведение: добавить в `VisualsPlugin` `On<Add, CityBuilding>` с `Mesh3d`,
   `cargo test -p gta_like --bin gta_like city_gate` будет green. Ожидаемо RED, фактически GREEN.
   Фикс по желанию, не для этого слайса.
3. **Info.** `start_city_mesh_build` висит на `OnEnter(GameState::Playing)`. Сейчас в `Playing` входят
   один раз за запуск. Если позже появится выход из `Playing` и возврат (пауза-состояние, рестарт),
   город заспавнится повторно. Это заметка для будущих слайсов, сейчас не баг.

## 5. Verdict

**SHIP-PENDING-RUNTIME.** Сборка, clippy и все тесты зелёные. Headless-критерии покрыты гейтами через
production-композицию. Мой flip-RED гейта слияния дал RED. Runtime t3 прошёл, скриншоты показывают
город, стоимость кадра ~2.5 ms. Оба фикса fixer'а проверены своими пробами. Остаётся субъективная
часть "город выглядит как город, районы различимы": её по чеклисту ниже принимает владелец.

## 6. Owner checklist

Запуск: `python tools/fetch_assets.py` (или `--cache <папка с zip Kenney>`), затем
`cargo run --release --features fast -- --seed 1`, потом `--seed 2`.

1. Город выглядит как город: тротуары приподняты бордюром, на асфальте осевые, разделители полос и
   зебры, у зданий окна по этажам, у высоток видны уступы.
2. Районы различимы: Downtown (серо-синие высотки, бетонные лоты), Commercial (бежевый, средняя высота),
   Residential (низкие дома, зелёные лоты, деревья вдоль улиц), Industrial (низкие корпуса, контейнеры за зданиями).
3. Ориентиры: самая высокая башня на площади у центра видна издалека, центральный парк с деревьями.
4. Солнце даёт тени примерно до 350 м; небо градиентом; дальний план уходит в туман на 250–450 м без шва.
5. Фонари на тротуаре плечом к дороге; светофоры на углах авеню смотрят разумно; пропы пропадают
   дальше ~120 м без заметного "попа".
6. Игрок заходит с дороги на тротуар без застревания (бордюр 0.15 м); с края крыши башни виден город.
7. Цвет стволов деревьев Kenney: на `park.png` они розово-оранжевые. Если в оригинале пакета они
   коричневые, значит текстура сэмплится не той UV/палитрой.
8. FPS: в t3 59 FPS (Fifo = частота дисплея), без vsync ~2.5 ms на кадр с крыши. Информация, не порог.

Что крутить: `assets/world/render.ron` (вид, туман, небо, пропы, бюджеты спавна), `assets/world/city.ron`
(бордюр, уступы, ориентиры).

children: 0 launched / 0 reported.
