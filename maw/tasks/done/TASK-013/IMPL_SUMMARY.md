# IMPL_SUMMARY — TASK-013 (GDD T12): minimap, menus, settings

Two implementer runs. Run 1 (committed at `6142115`, then the session ended) did steps 1-18 and step 19, and
drafted `t12.py`. Run 2 (this one) re-verified everything, reviewed the diff against PLAN_FINAL, ran `t12.py`
at runtime (it passed with no changes) and added a main-menu probe. Run 2 changed no code.

## 1. What was implemented (diff vs `main`, code/assets/tools only)

Sim (`gta_sim`, `citygen`), steps 1-8:
- `crates/gta_sim/src/flow/mod.rs` +57 −1: `MainMenu`/`Paused`, `NEW_CITY`, `pause_request`, `pause_time`/`resume_time`, `drop_queued_damage` on `NEW_CITY`, `pause_request_table`.
- `crates/gta_sim/src/world/mod.rs` +25 −3, `world/city.rs` +5 −1: `CityScoped`, `despawn_city`, `drop_city`, minimap re-exports.
- Resets on `NEW_CITY`: `character/mod.rs` +3 −1 (`CityScoped` required), `combat/mod.rs` +25 −2 (`clear_combat_messages`, `CombatRng` reseed), `combat/pickups.rs` +4 −1, `gang/mod.rs` +21 −2, `navigation/mod.rs` +7 −2, `perception/mod.rs` +7 −2, `police/mod.rs` +16 −2, `population/mod.rs` +17 −0, `wanted/mod.rs` +61 −2 (`search_row`, `search_circle`, `search_circle_per_star`), `wanted/search.rs` +2 −2, `lib.rs` +2 −1.
- `crates/citygen/src/minimap.rs` +202 (new; `rasterize`, `Raster::hash`, `project`, `map_px`, `heading_on_map`), `citygen/src/lib.rs` +1.
- Gates: `crates/gta_sim/tests/new_city.rs` +550 (G1-G3), `crates/citygen/tests/minimap.rs` +325 (M1-M5), `crates/citygen/tests/golden_minimap.txt` +8.

Client, steps 9-18:
- Settings: `src/settings/mod.rs` +145 (new).
- Menus: `src/menu/menu_config.rs` +109, `screens.rs` +331, `widgets.rs` +173 (new); `menu/mod.rs` +34 −1, `menu/config.rs` +10 −5.
- Minimap: `src/minimap/{mod.rs +236, markers.rs +205, material.rs +56, config.rs +86}` (new), `assets/shaders/minimap.wgsl` +99 (new).
- Consumers and wiring: `src/main.rs` +30 −15, `src/input/mod.rs` +32 −20, `src/camera/mod.rs` +8 −3, `src/juice/shake.rs` +9 −1, `src/juice/config.rs` +10 −1, `src/vfx/mod.rs` +6 −1, `src/hud/{mod.rs +2, stars.rs +5 −1, weapon.rs +5, wasted.rs +12 −46}`, `src/visuals/{city.rs +11 −3, props.rs +2 −1, character_config.rs +1 −1}`.
- Presentation gates P1/P2: `src/visuals/city_gate.rs` +113 −11.
- Data: `assets/ui/strings.ron` +24 −1 (`menu`, `hud.minimap`), `assets/juice/juice.ron` +1 (`shake.reduced_scale`).

QA, steps 19-20:
- `tools/qa/brp.py` +7 −1 (`QA_SETTINGS_ID`, `--settings-id` on every launch, `type_text`).
- `tools/showcase/record.py` +2 −1 (`--settings-id ...qa`).
- `tools/qa/scenarios/t12.py` +243 (new).

## 2. Deviations from plan

- `MenuConfig.sensitivity` (the step triple) is named `sensitivity_range` (`src/menu/menu_config.rs`), because `sensitivity` is already the label string. Same data, different field name.
- `title_screen` takes the title as one `(text, size, color)` tuple (`src/menu/widgets.rs`), so Wasted/Busted pass `wasted_size` and menus pass `title_size`. The Wasted/Busted look is unchanged.
- Minimap dots and glyphs are centred with `UiTransform::from_translation(Val2::percent(-50, -50))` instead of `left = size/2 + px.x − dot/2` (`src/minimap/markers.rs`). Same result, and it also works for text glyphs of unknown width.
- The shipped city is 1200 m (`assets/world/city.ron`), not the 1400 m the plan used as an example. The raster is 1200² at 1 px/m, about 5.8 MB.
- t12 step 4 sets heat with a direct `world.mutate_resources` on `WantedLevel.heat` instead of a `set_heat` helper (no such helper is importable from t5/t6/t8). It gives the same effect.
- Esc during a pending death or arrest is refused by `pause_request`, as planned. The README/AGENTS/narrative-graph updates listed in rollout notes are post-close orchestrator work under AGENTS.md, so I did not touch them.

## 3. Test results

Run 2, on this tree:
- `touch crates/*/src/lib.rs; cargo test -p gta_sim -p citygen -j 4`: every `test result` line ok, 0 failed. That covers 40 result lines, including `new_city` 3/3 and `minimap` 5 passed + 1 ignored (bless). Run 1 log is in `scratch/sim_tests_run1.txt`.
- `cargo test -p gta_like --bin gta_like -j 4`: 50 passed, 0 failed. Run 1 ran it 3× with 50/50 each time (`scratch/client_tests_3runs.txt`, includes P1/P2 `city_gate`).
- `cargo clippy -j 4 -- -D warnings`: clean. `cargo clippy -p gta_like --all-targets --features dev -- -D warnings`: clean. `cargo clippy -p gta_sim -p citygen --all-targets -- -D warnings`: clean.
- `cargo build -j 4`: ok. `cargo build -p gta_like --release --features dev`: ok.
- `python tools/qa/tree_check.py`: passed (no `bevy_render` in `gta_sim`).
- **Runtime `python tools/qa/scenarios/t12.py`**: PASS twice (`scratch/t12_run1/`, `scratch/t12_run2/`, each with `summary.json`, `pause.png` and `minimap_wanted.png`):
  - seed 1: golden hash, 100 chunks, player/minimap/Hud = 1/1/1, Hospital and Station markers present.
  - Esc → `Paused` + `PauseMenu::Main`. `AiClock` was frozen over 1 s: 136→136 in run 1, 111→111 in run 2.
  - `type_text("42")` + Enter → `CitySeed 42`, hash `0xe62c12ebedcb8268` = golden[42], 100 chunks (no leftovers), 1/1/1, `AiClock` advancing (219→285).
  - heat 200 → the police marker set equals the live cops in 5/5 samples, with none hidden inside `view_radius − 5 m`.
  - `log_errors` was empty (including "shader"/"wgsl"). `frame_report`: 144 Hz Fifo 144 FPS, no-vsync frame cost 2.6-2.7 ms.
  - Both runs ended in `Wasted`, because the cops shot the passive player after the markers were sampled. This does not affect the gate.
- **Probe `scratch/probe_main_menu.py`** (not a plan gate; it covers the no-`--seed` boot): PASS (`scratch/main_menu_probe/`). The game stays in `MainMenu` after 3 s with 0 players and no `CityLayoutHash`. Typing `2` + Enter gives `CitySeed 2`, the golden hash, 100 chunks and 1 player. Screenshot: `main_menu.png`.
- No settings directory was created under `%LOCALAPPDATA%`/`%APPDATA%` by QA runs: nothing changed, and `IfChanged` wrote nothing.

### Flip-RED record (all in `scratch/flips.jsonl`, run 1, flip runner `scratch/flip_runner.py`)
| Flip (perturbed input) | RED |
|---|---|
| raster `row` → `height−1−row` | M3 (b) building texel, M1 hash |
| `project`: `s = −s` | M4 yaw-90 row, M5 |
| `heading_on_map` swapped | M5 (camera 0 / facing 90) |
| `pause_request` without `Unchanged` guard | `pause_request_table` |
| `search_row` → `rows[0]` | `search_circle_per_star` |
| no `pause_time` | G1 (clock advanced while paused) |
| no reseed Combat / Npc / Gang / Police (4 flips) | G2, each its own row |
| `Character` without `CityScoped` | G3 A1 (98 entities left) |
| no `despawn_city` | G3 A1 (1573 left) |
| no `drop_city` | G3 A2 |
| no SidewalkGraph / GangTerritories / GangHeat / PlayerTerritory / Dispatcher / ArrestAttempt / PoliceAlert / PopulationPhase / CameraView / StimulusLog reset | G3 A5 / A6 / A7 / A8 / A9 / A10 / A11 / A12 / A13 / A14 |
| no `reset_wanted` | G3 A15 |
| no `clear_combat_messages` / `drop_queued_calls` / `drop_queued_damage` | G3 A17 / A21 / A22 |
| no `resume_time` | G3 A23 |
| `CityChunk` without `CityScoped` | P1 + P2 |
| `CityProp` without `CityScoped` | P1 (props survived) |
| keep `PendingCitySpawn` on `NEW_CITY` | P2 |

Not flipped, as the plan says: A16 `Crimes` (its recorder is `pub(crate)`, and it shares `reset_wanted` with A15), the `CityMeshTask` removal (timing-dependent), and M2 (liveness).

## 4. How to verify manually / owner checklist (AC 4)

- `cargo run --release` without `--seed` should open the main menu. "Новая игра" gives a random city. A typed seed + Enter gives that city.
- In game: Esc opens the pause menu, and Esc is ignored during Wasted/Busted. The pause menu shows "Seed города: N", "Продолжить", "Новый город" (typed seed, empty = random), "Настройки" and "Выход".
- Behaviour change: Esc no longer just releases the cursor. After an alt-tab that drops the grab, Esc twice recaptures it.
- The minimap is bottom-left and rotates with the camera. Check the arrow = facing, the gang territories tint, the search circle when wanted, the red cones of cops, blue police dots, gang/pickup dots, and the hospital/station glyphs clamped to the rim. The owner judges readability, colours, size and radius (`assets/ui/strings.ron` `hud.minimap`).
- Settings (sensitivity, volume, invert Y, reduce shake, no flashes) should change the game and survive a restart. The file is at `preferences_dir()/com.github.pockerhead.maw-make-gta/`. QA and showcase use the `.qa` id.
- While paused, sounds already playing finish (no audio pause in T12). A volume change applies to new sounds only.
- The HUD stars blink (dark phase) while no cop sees the player. In `minimap_wanted.png` they look empty while 2 stars are active, which is the existing T10 blink and not a T12 regression.

children: 0 launched / 0 reported.
