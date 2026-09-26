# ADR-002: CI релизов под Linux и Windows

## Контекст

Владелец хочет, чтобы релизы собирались сами на GitHub Actions под Linux и Windows (TASK-028). Бинарных
CC0-ассетов в git нет, они скачиваются `tools/fetch_assets.py` по `assets/third_party/manifest.ron`. Тестовые
workflow (TASK-029) уже есть: composite `.github/actions/setup` ставит Rust 1.95.0, rust-cache, Python и ассеты
с кэшем. Самые дорогие ошибки релиза тихие: exe работает на сборщике и падает у игрока (динамический `bevy_dylib`
или MSVC CRT), в zip нет лицензии или ассета, smoke читает `assets/` из checkout вместо zip.

## Решение

Один workflow `.github/workflows/release.yml` и инструмент `tools/package_release.py` (`package`, `verify`, `smoke`).

- **Триггеры.** Push тега `v*` собирает оба zip, гоняет headless-гейты и публикует GitHub Release. Push в ветку,
  который трогает файлы релиза (`release.yml`, `.github/actions/**`, `tools/package_release.py`,
  `tools/fetch_assets.py`, манифест, `Cargo.toml`, `Cargo.lock`, `.cargo/**`), и ручной `workflow_dispatch`
  собирают zip, проверяют и выкладывают их артефактами без релиза. На каждый push не гоняем: сборка под Windows
  холодной идёт 30-45 минут. Фильтр `paths` для тегов не применяется (документация GitHub), так что тег собирается
  всегда. Изменение только в `src/**` не собирается под Windows до следующего тега.
- **Раннеры.** `ubuntu-24.04` и `windows-2025`, закреплены. Linux-бинарь требует glibc >= 2.39 (Ubuntu 24.04+);
  `*-latest` сдвинул бы этот порог молча.
- **Тулчейн.** Тот же `dtolnay/rust-toolchain@1.95.0` из composite, он совпадает с `rust-version`. Второго пина нет.
- **Сборка.** `cargo build --release --locked -p gta_like --bin gta_like` без `--features` и со стандартным
  release-профилем. Фича `fast` (`bevy/dynamic_linking`) в релиз не попадает; `dev`/`debug` (BRP, отладка) тоже.
- **MSVC CRT.** На Windows `-C target-feature=+crt-static` через `CARGO_TARGET_X86_64_PC_WINDOWS_MSVC_RUSTFLAGS`
  на уровне job: так его видит ключ rust-cache, а Linux эту переменную не читает. Причина: на чистой Windows есть
  UCRT, но нет `VCRUNTIME140.dll`, и exe с динамическим CRT там не стартует. Локальные сборки не меняются, zip-exe
  отличается от локального `cargo run --release` только линковкой CRT. Проверка: `package` и `verify` ищут в exe
  имена CRT-библиотек (`vcruntime140`, `msvcp140*`, `ucrtbase[d]`, `api-ms-win-crt-*`); regex привязан к
  `api-ms-win-crt-`, потому что статический exe содержит другие `api-ms-win-*` строки. Единственный C-код в
  графе Windows (`blake3` через `cc`) переключается на статический CRT сам.
- **Линкер.** `.cargo/config.toml` задаёт `rust-lld.exe` только для `x86_64-pc-windows-msvc`, Linux он не трогает.
  Оставляем; если на windows-2025 линковка сломается, запасной вариант — `CARGO_TARGET_X86_64_PC_WINDOWS_MSVC_LINKER:
  link.exe` в env Windows-ноги.
- **Кэши.** rust-cache через composite, job id `release` (ключ различается по ОС) и `release-gates`; ассеты через
  `actions/cache`. sccache не нужен. Тег не видит кэши веток, поэтому прогон тега всегда холодный: гейты
  ~10-15 минут, Windows-сборка 30-45.
- **Без динамики.** `package` отказывается паковать exe, где есть `bevy_dylib`/`std-<hash>` (`.dll`/`.so`) или
  CRT-импорты; `verify` проверяет то же на готовом zip.
- **Zip.** `gta-like-<version>-<platform>-x86_64.zip`, где `<version>` — имя тега на push тега и `sha-<8 hex>`
  иначе, `<platform>` — `linux`/`windows`. Внутри одна папка с тем же именем: `gta_like[.exe]` и `assets/`
  (отслеживаемые RON/WGSL, манифест и все файлы паков вместе с лицензиями). Linux-exe в zip с режимом 0755.
  `verify` сверяет набор записей со списком (лишний `.dll`/`.pdb` не проедет), sha256 паков с манифестом,
  sha256 отслеживаемых файлов с checkout, наличие лицензии каждого пака.
- **Smoke на Linux.** Распакованный zip под `xvfb-run` с Mesa lavapipe (`WGPU_BACKEND=vulkan`): меню 30 с (ждём
  `AdapterInfo {` и `main menu ready`) и `--seed 1` 60 с (ждём `AdapterInfo {`). Ноль строк `ERROR`/`panicked`.
  `smoke` запускает exe из временной cwd без `CARGO_MANIFEST_DIR`/`BEVY_ASSET_ROOT` и с
  `--settings-id gta_like_smoke`, так что ассеты читаются только из zip, а настройки владельца не трогаются.
- **Smoke на Windows.** На раннере, `WGPU_BACKEND=dx12`, меню 30 с с теми же ожиданиями. Статус по первому
  прогону — в разделе "Обновление". Скачанный zip QA дополнительно запускает руками.
- **Гейты на теге.** Job `release-gates` (ubuntu-24.04) гоняет `cargo test --locked --no-fail-fast -p gta_sim -p
  citygen` только на push тега; на ветках их и так гоняют sim/citygen gates.
- **Публикация.** Job `publish` только при `push` тега `v*`, после `release` и `release-gates`. Права
  `contents: write` только у него; весь workflow — `contents: read`. `gh release create --verify-tag
  --generate-notes` с обоими zip, `--prerelease` для тегов с `-`. Только `GITHUB_TOKEN`; ref попадают в shell
  только через env (`$GITHUB_REF_NAME`, `$GITHUB_SHA`), не через `${{ github.* }}` внутри `run:`.
- **Окно консоли.** Пока остаётся: логи видны, и локальный smoke их читает. Скрыть его
  (`windows_subsystem = "windows"` плюс лог-файл) — отдельная полировка до v0.1.0.
- **Лицензия.** Лицензию на код игры не выбираем: это юридический выбор владельца. В zip есть лицензии паков и
  манифест с атрибуцией.

### Как выпустить релиз

```
git tag -a vX.Y.Z[-rcN] -m "..." <коммит main>
git push origin vX.Y.Z[-rcN]
```

Упавший тег не двигаем и не удаляем, берём следующий номер rc.

## Альтернативы

- Сборка на каждый push в main: 30+ минут Windows на коммитах с документацией. Только тег: нет прогона до мержа.
- Динамический CRT и "установите VC++ Redistributable" в README: exe молча не стартует на чистой Windows.
  `rustflags` в `.cargo/config.toml`: поменял бы локальные сборки и `fast`. `RUSTFLAGS`: задел бы и Linux.
- LTO/strip/свой профиль: не просили, и владелец мерил FPS на стандартном release.
- `dumpbin`/`readelf` вместо поиска байтов: не переносимы между ОС. `cargo tree`: показывает граф, а не бинарь.
- `softprops/action-gh-release`: сторонний код с токеном на запись. Загрузка по одной ОС: полупубликованный релиз.
- Отдельный режим `--smoke` в игре: код в продукте ради CI, а маркер в логе и таймаут дают то же.

## Обновление

Первый прогон на ветке (2026-09-27, https://github.com/pockerhead/MAW-make-GTA/actions/runs/36277750356) зелёный
на обеих ОС без запасных вариантов: `rust-lld.exe` линкует `+crt-static` на windows-2025, exe без CRT-импортов;
smoke на Linux (адаптер `llvmpipe`) и на Windows (адаптер `Microsoft Basic Render Driver`, dx12) дошёл до
`main menu ready` без строк `ERROR`. Windows-smoke в CI оставлен гейтом. Холодные сборки: Linux 15 мин,
Windows 31 мин.

Если Windows-smoke начнёт падать по причине окружения (нет адаптера dx12, ошибка WARP), шаг удаляем целиком, а не
ставим `continue-on-error`, и записываем причину сюда. `--allow` в smoke допустим только на точную пару target +
сообщение с однострочным комментарием в YAML; сканирование `ERROR` не отключаем.

При смене Rust-пина менять его в composite `.github/actions/setup` (он общий с тестовыми workflow). При смене
раннеров пересмотреть порог glibc в README. Если придётся отказаться от `+crt-static`, убрать CRT-проверку из
`package_release.py` только вместе с правкой этого ADR и требованием VC++ Redistributable в README.
