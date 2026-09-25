# IMPL_REVIEW — TASK-029 (test CI + README badges)

Reviewer: code-reviewer (claude opus, medium). Reviewed HEAD c49a9dc on `infra/test-ci`.

## 1. Verdict

**PASS.** Все 5 workflow зелёные на HEAD c49a9dc. Все 4 flip-RED подтверждены в логах CI. Путей, где провал проходит молча, я не нашёл. Три minor-замечания, мерж они не блокируют.

Мерж блокирует только то, что по плану и так делается после него: Step 8. Первый прогон на `main` будет холодным, все 5 должны стать `success`, и нужно открыть бейджи на GitHub. На этой стадии это проверить нельзя.

## 0. Disconfirmation

Контрпример, который я проверял: тестовый шаг `cargo test ... | tee test.log` уходит в green, хотя тест упал, потому что статус пайпа равен статусу `tee`.

- Код: у каждого тестового шага стоит `shell: bash`. GitHub Actions запускает его как `bash -eo pipefail`, а сверху шаг ещё явно делает `set -o pipefail` (`sim-gates.yml:28`, `citygen-gates.yml:26`, `client-gates.yml:29`).
- Первоисточник: во флипе (b) `scratch/flip_b_ci_36078556547.log` есть `runtime_hash_matches_golden ... FAILED`, затем `test result: FAILED`, затем `##[error]Process completed with exit code 101.`
- **Контрпример не подтвердился.** Второй кандидат, SKIP-путь ассетов, тоже не подтвердился (см. ниже).

Dead_end в log.jsonl ровно один: у планировщика в WSL стоял rustc 1.89 и не смог собрать проект, поэтому поведение gta_sim на Linux оставалось непроверенным. Настоящий CI это закрыл: sim зелёный на Linux в прогонах 1-3 и на HEAD.

## 2. Confirmed correct

- **Нет тихого проглатывания ошибок.** `continue-on-error` нет нигде. Единственный `|| true` стоит на `grep` в информационном шаге "Test counts" (`*-gates.yml:31/33/34`), а сам тестовый шаг к этому моменту уже определил цвет job. В clippy и repo checks пайпов нет.
- **SKIP-ветки ассетов в CI недостижимы.** `crates/gta_sim/tests/asset_manifest.rs:186` (SKIP, если нет ни одного пака) и `src/audio/gate.rs:199-205` (SKIP, если нет хотя бы одного из трёх звуковых паков) срабатывают только при отсутствии каталогов. `fetch_assets.py --check` в `.github/actions/setup/action.yml:37-39` выполняется в той же job до `cargo test` и завершается с 1, если не хватает хотя бы одного пака. В каждом прогоне sim, client и repo логи содержат `third-party packs match the manifest`. Флип (c) показал, что `mix_oggs_decode` действительно работает в CI: `GATE BROKEN: ..._999.ogg`, 75 passed, 2 failed. Оговорка: отсутствие строк "SKIP" в логе ничего не доказывает, потому что cargo захватывает stdout и stderr прошедших тестов. Реальная гарантия здесь даёт `--check`. Все совпадения по "skip" в логах, которые я просмотрел, безобидные: это `up to date, skipped` из fetch и выключенные условные шаги composite.
- **Ключи кэша не пересекаются.** Job id у всех разные (`clippy/sim/citygen/client/repo`). `gh cache list` показывает пять разных ключей `v0-rust-<job>-Linux-x64-a972f308-670c1f85`. Общий ключ `third-party-<hash manifest>` делится между job намеренно, потому что содержимое у них одинаковое. Сохранение из вложенного composite работает.
- **Права.** Везде задано `permissions: contents: read` на уровне workflow. `secrets.` нигде не используется (AC5).
- **Триггеры.** `push` с `branches-ignore: [media]` покрывает `main` и ветки задач, дальше идут `pull_request` и `workflow_dispatch`. Теги не запускают workflow, так и задумано.
- **Бейджи.** `README.md:9`, одна строка сразу после `<!-- SHOWCASE-HERO:END -->` (стр. 7), вне маркеров. `set_hero` (`tools/showcase/publish.py:85-89`) переписывает только блок между маркерами, так что строку с бейджами он не снесёт. Пять ссылок идут на `actions/workflows/<file>/badge.svg?branch=main` и `?query=branch%3Amain`. Имена файлов совпадают с `.github/workflows/*`, ветка правильная, потому что дефолтная у репо `main`.
- **Размещение "Что проверяет CI".** `README.md:154`, после `## Статус` (стр. 82) и перед `<!-- COST:START -->`. Раздел ниже статуса, как просил владелец, и бейджей в нём нет.
- **Счётчики README.** Я пересчитал их тем же sed/awk по логам: sim 378 passed / 2 ignored (run3), citygen 32 / 3, client 77. С README совпадает.
- **Flip-RED 4/4 по job, предсказания сделаны до пуша.** Шаги в CI (a) clippy, (b) citygen+sim, (c) client, (d) repo подтверждены логами в `scratch/flip_*`, а для (d) ещё и `gh run view 36079346675`: `font_check failure`, `test_brp skipped`.
- **Кэш работает.** Wall time: 16m22s холодный, 7m13s и 6m47s тёплые. В тёплых прогонах `full match: true`. AC4 выполнен.
- **`--no-fail-fast`** стоит обоснованно: Q2 требует полный список упавших тестов.
- **HEAD c49a9dc:** все 5 workflow `success` (runs 36080516475/491/498/509/526), это я проверил сам через `gh run list --commit`.
- AGENTS.md: +1 упоминание в "Сборка" и абзац "CI после мержа". Правка хирургическая.

## 3. Issues

1. **minor — `cancel-in-progress: true` применяется и к `main`** (все 5 файлов, стр. 9-11). Если два мержа в `main` придут подряд, прогон первого мерж-коммита отменится (`cancelled`). Тогда правило AGENTS.md "все 5 `success` на мерж-коммите" выполнить нельзя, а красный коммит между двумя мержами останется непроверенным. Предлагаемый фикс: `cancel-in-progress: ${{ github.ref != 'refs/heads/main' }}`. Сейчас мержи идут по одному, так что это не блокер.
2. **minor — в repo checks упавший шаг скрывает следующие** (`repo-checks.yml:29-37`). Во флипе (d) `test_brp` оказался `skipped`. Job от этого всё равно красная, так что тихого провала нет, но результат независимых проверок пропадает. Фикс: добавить `if: success() || failure()` на шаги tree_check, font_check и test_brp. Тогда они будут пропускаться при сбое setup, но не при падении соседа.
3. **minor — push и pull_request дублируют прогоны** для PR из ветки этого же репо. Группы concurrency у них разные (`refs/heads/x` и `refs/pull/N/merge`), поэтому идут оба прогона и кэшей получается вдвое больше. Сейчас пайплайн PR не открывает, так что это только цена. Если PR появятся, стоит ограничить `push` до `branches: [main]` плюс ветки задач или принять дублирование осознанно.

## 4. Missing coverage

- **Красный путь `tree_check.py` в CI не показан.** Флип (d) задел только font_check. Код простой (`RuntimeError` -> `sys.exit(1)`, `tools/qa/tree_check.py:44-49`), а гейт существовал до этой задачи, поэтому я это не блокирую. Оценка по коду, флип не делал.
- **"Test counts" отражают только liveness и ничего не утверждают.** Если job соберёт 0 тестов, она будет зелёной с `0 binaries`. Флипы (b) и (c) показывают, что тесты сейчас реально запускаются. Защиты от будущего "0 tests" нет. Можно добавить `[ "$p" -gt 0 ]`, если владелец захочет.
- Step summary никто не видел глазами, имплементер это честно признаёт. Сам пайплайн я проверил локально на логах, но то, как блок рендерится на странице run, остаётся за QA или оркестратором (Step 8).

## 5. Nits

- `tree_check.py` вызывает `cargo tree` без `--locked`. Рассинхрон lock-файла всё равно ловят остальные job с `--locked`, так что это просто к сведению.
- Кэш `repo` (136 MiB) ради `cargo tree` почти ничего не даёт, это лишний объём в лимите 10 GB (R3).

children: 0 launched / 0 reported
