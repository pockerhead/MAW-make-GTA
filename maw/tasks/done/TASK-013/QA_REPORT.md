# QA_REPORT — TASK-013 (GDD T12): мини-карта, меню, настройки

QA: claude opus, medium. Tree: `feature/t12-minimap-menu` @ `a9f1d78` (checkout was clean). All artifacts are in
`scratch/qa/`.

## 0. Preflight and disconfirmation

- I read `scratch/` first and used it only as a coverage map. The implementer and fixer runs (t12 ×2, main-menu probe,
  fixer settings screenshot) were not re-used as evidence. Every runtime claim below comes from my own runs.
- I read TASK_FINAL, PLAN_FINAL (all 594 lines), IMPL_SUMMARY, IMPL_REVIEW, FIX_SUMMARY, OPEN_DECISIONS and log.jsonl.
  The log has no implementer `dead_end` entries. The planner ones (rotated ImageNode, `Strike` pub(super),
  `raise_heat` order) are consistent with the code: an unrotated `UiMaterial`, `Strike` is not asserted, and
  `raise_heat` runs before `spawn_unit`.
- **Counter-example chosen up front:** "Новый город" pressed while cops, gang members and civilians are alive leaves
  old-city entities in the client world, meaning entities with no `Transform` or no `CityScoped` tag that G3's
  `Transform`-based leak detector cannot see. **It held, in part.** Every `Character`, pickup, chunk, prop, HUD root
  and minimap marker was replaced, and no marker pointed at a dead target. But each despawned character leaves an
  orphaned bevy-tnua sensor entity (Bug 1). This leak is **pre-existing**: it grows about 1/s from civilian recycling
  before any new city. T12 is not its cause, but the new-city path is one more source.

## 1. Environment

No docker or dev server. I used the direct cargo test runners, and ran the real release game (`--features dev`)
over BRP through `tools/qa/brp.py`. Every launch used `--settings-id com.github.pockerhead.maw-make-gta.qa`, because
brp.py adds it.
- For clicks and keys while paused I used real OS input: Windows `SendInput` via ctypes (`scratch/qa/osinput.py`,
  window found by `--window-title`). BRP holds never release on the frozen virtual clock, so they cannot drive the
  paused menus.
- Host: `\\.\DISPLAY1` at 144 Hz, Fifo.
- The owner settings dir `%LOCALAPPDATA%\com.github.pockerhead.maw-make-gta` did not exist before QA and did not exist
  after it (checked in probe A). The QA dir `...maw-make-gta.qa\settings.toml` now holds probe values (sensitivity
  1.5, volume 0.9, all toggles on). It belongs to QA only. Delete it if a clean QA default is wanted.

Reproduce:
```
cargo build -j 4; cargo clippy -j 4 -- -D warnings
cargo clippy -j 4 -p gta_like --all-targets --features dev -- -D warnings
cargo clippy -j 4 -p gta_sim -p citygen --all-targets -- -D warnings
touch crates/*/src/lib.rs; cargo test -j 4 -p gta_sim -p citygen
cargo test -j 4 -p gta_like --bin gta_like        (x3)
python tools/qa/tree_check.py; python tools/qa/font_check.py
python tools/qa/scenarios/t12.py --out maw/tasks/in_progress/TASK-013/scratch/qa/t12
python tools/qa/scenarios/t{8,9,10,11}.py --out maw/tasks/in_progress/TASK-013/scratch/qa/t{N}
python maw/tasks/in_progress/TASK-013/scratch/qa/probe_a.py   # main menu, settings, persistence (real input)
python maw/tasks/in_progress/TASK-013/scratch/qa/probe_b.py   # no-flashes, new city under fire, leftover audit
python maw/tasks/in_progress/TASK-013/scratch/qa/probe_c.py   # sensor orphans, Esc in Busted/Wasted
```
Services started: none are persistent. Each game process was shut down by brp.py, and `tasklist` shows no `gta_like`
left.

## 2. Test results

| Run | Result |
|---|---|
| `cargo build -j 4` | ok |
| clippy (3 shapes above) | clean |
| `cargo test -p gta_sim -p citygen` | 40 result lines, all ok, 0 failed (`scratch/qa/sim_tests.txt`). Includes `new_city` 3/3, `minimap` 5 + 1 ignored bless, `pause_request_table`, `search_circle_per_star`. |
| `cargo test -p gta_like --bin gta_like` ×3 | 51/51 each run (`scratch/qa/client_tests_3runs.txt`), including P1/P2 `city_gate` and `step_at_a_bound_leaves_settings_unmarked` |
| `tree_check.py` | passed |
| `font_check.py` | `153 literals, 2 fonts, 0 missing glyphs`, exit 0 |
| t12 (my run) | **PASS** (`scratch/qa/t12/summary.json`). See §3 for details. |
| t8, t9, t10, t11 (regressions, all now boot through the new flow with `--seed`) | all exit 0. Summaries and screenshots are in `scratch/qa/t8..t11/`. t11 covers Busted → Playing. |

No new failures, so no base-commit comparison of failure lists was needed: the failure list is empty.

**Own flip-RED** (commit `a9f1d78`, restore checked by sha256): I removed `#[require(CityScoped)]` from `Pickup`
(`crates/gta_sim/src/combat/pickups.rs`). This flip is not in the implementer's list. Result: G3 went RED with
`A1: 2 entities of the old city left` (`new_city.rs:406`). After restore, `sha256sum -c` gave OK.

### New runtime probes (mine, independent of t12)

- **probe A** (`scratch/qa/probe_a/summary.json`, `main_menu.png`, `settings_default.png`, `settings_changed.png`)
  - **Boot without `--seed`:** `MainMenu` after 3 s, 0 players, no `CityLayoutHash`. Esc in the main menu stays in
    `MainMenu`.
  - **"Новая игра" (real click):** `Playing`, random seed `1790256437505346300`, 100 chunks, 1 player.
  - **Esc (real key), then click "Настройки":** `PauseMenu::Settings`. I then clicked every control once with the real
    mouse. `GameSettings` after each click:
    - sens+ → 1.25; sens+ → 1.5
    - vol− → 0.9
    - invert → true; shake → true; flashes → true
  - **Labels:** `×1.50`, `90%`, `вкл` ×3. The `−`/`+`/`↔` glyphs render (screenshot).
  - **Save on screen close:** the file did not exist before closing. After Esc (settings → pause) it existed with all
    five values: `SaveSettings::IfChanged` on close works while paused.
  - **Effects:**
    - Mouse look with the same BRP delta (100, 50): dyaw −0.2094 → −0.3142. The ratio is exactly 1.5.
    - dpitch −0.1047 → +0.1571: the sign flips (invert Y) and the magnitude is ×1.5.
    - `GlobalVolume` = Linear 0.9.
  - **Restart with the same `.qa` id and `--seed 3`:** `GameSettings` came back identical. Persistence works.
- **probe B** (`scratch/qa/probe_b/summary.json`, `survivors.json`, `before_new_city.png`, `after_new_city.png`)
  - **No flashes, 8 s under police fire:**

    | Setting | Muzzle flash | Tracer |
    |---|---|---|
    | on | 0 | 32 |
    | off | 37 | 41 |

  - **New city under fire:** 55 characters alive (4 cops, 7 gang members, 40 civilians), 22 markers, 2 stars. Esc
    (real key), `type_text("7")`, Enter (real key).
    - Result: seed 7, 100 chunks, 1 player, 1 minimap, 0 cops and 0 gang members, `WantedLevel` default, no marker
      with a dead target.
    - Leftover audit: I compared entity ids from before and after and listed the components of every survivor.
      Survivors were window, camera, input, observers, one-shot systems and asset resources, plus **237 entities
      carrying only `TnuaProximitySensor`**. This is Bug 1.
- **probe C** (`scratch/qa/probe_c/summary.json`)
  - **Orphan sensor count, seed 1:**

    | When | Orphan sensors |
    |---|---|
    | idle, 0 s | 31 |
    | idle, 30 s | 68 |
    | idle, 60 s | 95 |
    | after a 20 s run | 162 |
    | before new city | 178 |
    | after new city | 272 |

    Owned sensors track the characters (about 1.3 per character).
  - **Esc during Busted:** 12 real presses, then 2 s watched with no presses. Sequence `Busted → Playing`, never
    `Paused`.
  - **Esc during Wasted:** 11 presses. Sequence `Wasted → Playing`, never `Paused`.
  - After the respawn, Esc gives `Paused` and a second Esc gives `Playing`: resume works with real input.
  - `log_errors` was empty.
  - Side note: in the first probe B run, `Paused` appeared after Busted. My press loop kept running after Busted had
    already ended, so that `Paused` is a script artifact and not a bug. Probe C was rewritten to press only while
    in `Busted`/`Wasted`.

## 3. Acceptance criteria

| Criterion | Test performed | Result |
|---|---|---|
| Headless: raster is deterministic (image hash by seed) | M1 golden hashes for seeds 1/2/42 + M2 pairwise different + M3 texel orientation. All green on my run. Implementer flips (mirrored row) recorded. | PASS |
| Headless: 10 m ahead at yaw 0/90/180 lands straight up | M4 (independent `Quat` forward, 6 rows, `map_px` → (0, −20)), green. At runtime, the police dots and glyphs agree with the 3D view in `t12/minimap_wanted.png`. | PASS |
| Runtime t12 via brp.py: Esc, pause screenshot, typed seed, new `CityLayoutHash`, minimap under wanted | My t12 run PASS. Esc → `Paused`/`Main`, `AiClock` frozen over 1 s. `type_text 42` + Enter → seed 42, golden hash, 100 chunks, 1/1/1, clock advancing. Police markers = live cops in 5/5 samples. Frame cost without vsync 2.6 ms. `pause.png` shows "Seed города: 1", 4 buttons and the field. `minimap_wanted.png` shows the search circle, red cones, blue dots, `+`/`П` glyphs and the arrow. | PASS |
| Owner checklist in QA_REPORT | §6 below | PASS (recorded) |
| New tuning values in GDD §12 data files, not `const` | Diff scan. New consts: `NEW_CITY` (schedule label), `MAX_CONES` (shader array law), `SETTINGS_APP_ID` (identity), and the test-only `BLESS`/`RANGE`. Tuning lives in `strings.ron` (`hud.minimap`, `menu`) and `juice.ron` (`reduced_scale`). The seed-field padding `px(6.0)` and the WGSL arrow ratios are inline literals: review minor 3, accepted in OPEN_DECISIONS. | PASS |
| `cargo build`, `clippy -D warnings`, `cargo test -p gta_sim`/`-p citygen` green | §2 | PASS |
| Existing tests pass | §2. Client 51/51 ×3, t8–t11 runtime regressions pass. | PASS |
| Orchestrator: main menu boot without `--seed` | probe A | PASS |
| Orchestrator: every setting changes something visible or audible and persists across a restart | probe A/B. Sensitivity, invert Y, volume (`GlobalVolume`) and no flashes were measured. Reduce shake is checked by code only: `slerp(.., 0.3)` in `juice/shake.rs`. `CameraShake` is not reflected, so its feel is owner-run. Persistence: PASS. | PASS (shake feel: owner) |
| Orchestrator: new city while cops/gangs are active leaves no leftovers (BRP count) | probe B/C. Every gameplay and visual entity is gone. Orphaned Tnua sensor entities remain; this leak is pre-existing (Bug 1). | PASS for T12 scope, with pre-existing Bug 1 |
| Orchestrator: Esc during Wasted/Busted ignored | probe C, real keyboard | PASS |
| Orchestrator: `font_check.py` passes | exit 0 | PASS |

## 4. Bugs found

1. **Medium, pre-existing (not a T12 regression): orphaned bevy-tnua sensor entities.**
   - Cause: bevy-tnua 0.32 spawns ground and headroom sensors through `with_related_entities::<TnuaSensorOf>`
     (`bevy-tnua-0.32.0/src/sensor_sets.rs:98-107`). `TnuaSensorsSet` has no `linked_spawn`
     (`bevy-tnua-physics-integration-layer-0.13.0/src/data_for_backends.rs:62-68`). When a character is despawned, each
     of its sensors stays alive with only `TnuaProximitySensor`.
   - Why nothing catches it: the component is not reflected and has no `Transform`, so neither BRP queries nor G3 A1
     see it.
   - Repro: `python scratch/qa/probe_c.py`. Orphans grow about 1/s with the player idle (civilian recycling), which
     is about 4k per hour. "Новый город" adds about one orphan per character alive.
   - Expected: no orphans. Actual: 272 orphans after 2.5 min of play plus one new city.
   - Cost: a slow, silent growth of small entities (each one is a few hundred bytes plus a query scan). No
     gameplay effect was seen.
   - Suggested follow-up task, not a T12 blocker: an observer on character despawn that despawns its
     `TnuaSensorsSet` targets, and a leak gate that counts entities per component through `list_components`.
   - PCTX proposal added.
2. **Low, cosmetic: `step_value` does not produce the value its doc claims.**
   - `step_value(1.0, −1, 0, 1, 0.1)` gives 0.90000004 (9 × 0.1f32), and the settings file stores
     `volume = 0.9000000357627869` (`probe_a/summary.json` `file_text`).
   - The doc comment on `src/settings/mod.rs` `step_value` says snapping "keeps 0.7 from becoming 0.70000005 in the
     settings file". It does not do that for volume steps.
   - The display shows `90%`, so the owner sees nothing wrong unless they hand-edit the toml.
   - Fix: divide instead of multiply (`(n / (1/step))`), or round to 1e-4 before storing. Or drop the claim from the
     doc.
3. **Low, owner-visible: the hospital `+` and station `П` glyphs can sit on top of each other on the minimap rim.**
   - Both are clamped to the same rim direction when both landmarks are far away in the same direction.
   - Seen in `probe_a/settings_changed.png`, bottom-left of the minimap.
   - Readability is the owner's call. The markers are not wrong.

No bug found in the pause race, the teardown of gameplay and visual entities, the reseed, the menus, the settings
persistence, or the Esc rule.

## 5. Verdict

**SHIP-PENDING-RUNTIME.**
- Every headless and runtime criterion I could test passes on my own runs: build, clippy, all tests, t12, the t8–t11
  regressions, main menu, settings effects and persistence, Esc during Wasted/Busted, font gate.
- Bug 1 is real and silent, but it predates T12 (it grows before any new city). I recommend a follow-up task rather
  than holding this slice.
- Bugs 2 and 3 are cosmetic.
- AC 4 and the look and feel below are owner-run.

## 6. Чек-лист владельца (AC 4 и feel)

Запуск: `cargo run --release`, без `--seed`.

1. **Главное меню.** Открывается экран "GTA-like" с кнопками "Новая игра" и "Выход" и полем Seed.
   - "Новая игра" даёт случайный город.
   - Seed в поле + Enter даёт этот город. Проверьте на двух запусках: одинаковый seed даёт одинаковый город.
2. **Esc теперь пауза** (раньше Esc отпускал курсор).
   - В игре Esc открывает "ПАУЗА": "Seed города: N", "Продолжить", "Новый город", "Настройки", "Выход", поле seed.
   - Повторный Esc или "Продолжить" возвращает в игру, курсор снова захвачен, персонаж не прыгает и не стреляет сам.
   - Звуки, которые уже играли, доигрывают.
   - Во время ПОТРАЧЕНО и BUSTED Esc ничего не делает.
   - После alt-tab курсор возвращается двумя нажатиями Esc.
3. **"Новый город" из паузы.** Введите seed, нажмите Enter или кнопку. Пустое поле даёт случайный seed.
   - Появляется экран загрузки, потом новый город.
   - Ни копов, ни розыска, ни звёзд, ни меток от старого города не остаётся.
4. **Мини-карта** (внизу слева, круглая, вращается с камерой).
   - Стрелка показывает, куда смотрит персонаж.
   - Видны территории банд (подкраска), синий круг поиска при розыске, красные конусы копов, синие точки полиции,
     цветные точки банд и пикапов, значки больницы "+" и участка "П" на ободе.
   - Оцените читаемость, размер (220 px), радиус (120 м), цвета. Всё это в `assets/ui/strings.ron`, `hud.minimap`.
   - Замечание QA: "+" и "П" могут наложиться друг на друга на ободе.
5. **Настройки** (Пауза → Настройки).
   - Чувствительность мыши, шаг ×0.25.
   - Громкость, шаг 10%. Действует на новые звуки.
   - Инверсия Y.
   - Уменьшить тряску. Проверьте в ближнем бою: тряска камеры должна стать заметно слабее.
   - Без вспышек. Вспышек у стволов нет, трассеры остаются.
   - Esc или "Назад" возвращает в паузу.
   - Выйдите из игры, запустите снова: значения сохранились. Файл лежит в
     `%LOCALAPPDATA%\com.github.pockerhead.maw-make-gta\settings.toml`.
6. **Внешний вид меню и паузы.** Шрифты, фон, наведение на кнопки. Решает владелец.

children: 0 launched / 0 reported.
