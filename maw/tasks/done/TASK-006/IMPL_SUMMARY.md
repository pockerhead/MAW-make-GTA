# IMPL_SUMMARY — TASK-006 (GDD T5: здоровье, урон, смерть, возрождение)

Verdict: IMPLEMENTED. Pre-flight of PLAN_FINAL passed (every named file, type and API matched the
plan's assumed shape in the pinned 0.19.1 sources). All of A–H is implemented; the gates are GREEN and
every new gate was shown RED by a flip (24 RED flips in `scratch/flip_red.log`, harness `scratch/flip_red.py`).

Note for the orchestrator: the owner's commit `e92372b` ("docs: README — how it started") ran while I
was working and swept my in-progress A–D files into the feature branch. I did not commit anything.
The rest of my work is uncommitted in the working tree.

## 1. What was implemented (lines: + added / − removed vs `c730212`)

Data (GDD §12):
- `assets/character/health.ron` (new, 9), `assets/flow/respawn.ron` (new, 1), `assets/ui/strings.ron` (new, 13, UTF-8 no BOM),
  `assets/world/render.ron` (+1 `pickups`), `assets/third_party/manifest.ron` (+14, pack `inter` 4.1 OFL).

citygen:
- `crates/citygen/src/graphs.rs` (+108): `pub fn sidewalk_anchor` (closest point on the `geom::inset` sidewalk ring side,
  clamped by `margin`), `fn anchor_on_segment`, unit test `anchor_on_segment_examples`. `src/lib.rs` (+1 re-export).
- `crates/citygen/tests/properties.rs` (+83): `hospital_anchor_on_sidewalk` (34 seeds; margin read from health.ron).
- Layout, `layout_hash` and golden hashes are unchanged (golden test GREEN).

gta_sim:
- `character/health.rs` (new, 189): `HealthConfig`/`PickupConfig` + `validate`, `Health`, `Dead`, `apply_damage`, `regenerate`, `HealthSystems`, unit tables.
- `character/mod.rs` (+31): registration, chained `HealthSystems`, `Has<Dead>` branch in `drive_characters` (zero walk basis, drop jump request/buffer).
- `flow/mod.rs` (+45): `GameState::Wasted`, sub-state `WastedPhase {SlowMo, Screen}`, `PlayingSystems`/`WastedSystems` sets, reflect registration.
- `flow/wasted.rs` (new, 123): `RespawnConfig`, `WastedClock`, `detect_player_death`, `enter_wasted`, `advance_wasted` (Update + `Time<Real>`),
  `restore_time_scale` on `OnExit(WastedPhase::SlowMo)`, `respawn_player` on `OnExit(Wasted)` (teleports the same entity).
- `player/mod.rs` (+62): `DebugDamage` message (`#[reflect(Message)]`), `spawn_player` on `OnTransition{Loading→Playing}` with `Health`, `apply_debug_damage`, `regenerate_player`.
- `combat/mod.rs` (new, 29) + `combat/pickups.rs` (new, 81): `Pickup`/`PickupKind`, one-shot spawn at `point ± along·spacing`, `collect_pickups`.
- `wanted/mod.rs` (new, 23): minimal `WantedLevel`, reset on `OnEnter(Wasted)`.
- `world/mod.rs` (+14) `HospitalSpawn` resource (TestArea: origin, +X); `world/city.rs` (+35) `hospital_spawn()`; failure = existing error+exit path.
- `lib.rs` (+~30): loads and validates health.ron / respawn.ron in `compose_sim`, adds `CombatPlugin`, `WantedPlugin`.
- `config/manifest.rs` (+63/−14): `AssetLicense::OFL`, `markers()`, `check_source()` (CC0 = Kenney, OFL = GitHub release pinned to `v{version}`).

Sim gates: `tests/common/mod.rs` (+47 helpers), `tests/health.rs` (new, 213), `tests/respawn.rs` (new, 213),
`tests/config.rs` (+53), `tests/asset_manifest.rs` (+22/−4), 4 new fixtures in `tests/fixtures/manifest/`.

Tools: `tools/fetch_assets.py` (+18/−4, OFL/GitHub mirror of `check_source`), `tools/qa/scenarios/t5.py` (new, 234).

Client (`src/`):
- `visuals/city.rs` (+8/−1): `start_city_mesh_build` on `OnTransition{Loading→Playing}`.
- `menu/config.rs` (new, 103) `UiConfig`/`HudLayout` + `validate`, `font_paths`; `menu/mod.rs` (+40) `UiFonts` (FromWorld), Russian loading screen with Inter.
- `hud/mod.rs` (new, 105) bars (write `Node.width` only on change); `hud/wasted.rs` (new, 45) "ПОТРАЧЕНО" screen (`DespawnOnExit(WastedPhase::Screen)`), `post_saturation` on enter/exit Wasted.
- `visuals/config.rs` (+24) `PickupVisuals` + validation; `visuals/pickups.rs` (new, 41) cube observer + show/hide by cooldown; `visuals/mod.rs` (+4).
- `camera/mod.rs` (+7) `reset_pivot` on `OnExit(Wasted)`; `debug/mod.rs` (+14) F5 → `DebugDamage` (under `debug`); `main.rs` (+27) loads/validates `UiConfig`, font manifest check in `preflight`, `HudPlugin`.
- `visuals/city_gate.rs` (+67): gate `city_is_built_once_across_respawn`.

Inter install: `python tools/fetch_assets.py --cache maw/tasks/in_progress/TASK-006/scratch` → "inter: installed";
`--check` OK. `git check-ignore -v` shows `.gitignore:64:/assets/third_party/*` for the TTFs and LICENSE.txt; `git add -A --dry-run`
stages only `assets/third_party/manifest.ron`. No binary asset is tracked. `Cargo.lock` unchanged, no new crates or features.

## 2. Deviations from plan

1. **D2.2 regen gate: added an off-grid case** (`crates/gta_sim/tests/health.rs`, end of `regen_waits_then_stops_at_cap`):
   damage 50.5 → 49.5, 400 ticks, `max <= 50`, end `== 50`. With the plan's numbers only (70 → 30, steps of 5/64) the value hits
   50.0 exactly after 256 steps, and the `current >= cap` guard stops it, so flip (a) "remove `.min(cap)`" stayed **GREEN**.
   With the off-grid case the flip is RED. The unit table (`49.99 → 50.0`) was already RED. Log: decision entry.
2. **E5 `bad_license_source` keyword `"page"` → `"OFL requires page"`**: an "OFL accepts a Kenney page" flip stayed **GREEN**
   because the resulting url error quotes `https://kenney.nl/media/pages/...`, which contains "page". Now RED. Log: dead_end entry.
3. `#[allow(clippy::type_complexity)]` on 4 systems (`drive_characters`, `collect_pickups`, `detect_player_death`,
   `respawn_player`). `cargo clippy -D warnings` flags these queries, and the repo had no precedent. The allow is the standard
   Bevy idiom; aliases would only move the tuples.
4. `regenerate_player` and `apply_debug_damage` filter `Without<Dead>` (the plan named it only for damage). Behaviour is the
   same, since `regenerate` returns 0 for a dead body.
5. `respawn_keeps_world_one_shot` does not call `player()` after the cycle, because it panics on two players before the
   count assertion can report. Counts carry the gate.
6. D3.1 flip (d) (`enter_wasted` without `set_relative_speed`) goes RED on the k=0 speed assertion before the SlowMo tick
   count is reached. Still RED; noted for honesty.

Nothing from the plan was skipped.

## 3. Test results

- `cargo build` — OK. `cargo build -p gta_like --features dev --release` — OK.
- `cargo clippy --workspace --all-targets -- -D warnings` — clean; `cargo clippy --features dev,debug -- -D warnings` — clean.
- `cargo test -p gta_sim -p citygen` — all green: citygen unit 9, golden 3 (+1 ignored bless), properties 12;
  gta_sim lib 4, anim_state 4, asset_manifest 3, city 6 (+1 ignored budget), config 9, health 6, jump 3, movement 4, respawn 3, terrain 2.
- `cargo test -p gta_like --bin gta_like` — 16 passed (incl. `city_is_built_once_across_respawn`).
- `python tools/qa/tree_check.py` — passed. `cargo tree -p gta_sim -e normal|features -i bevy_render` — "nothing to print".
- `python tools/fetch_assets.py --check` — "third-party packs match the manifest".
- **Runtime QA** `python tools/qa/scenarios/t5.py --out maw/tasks/in_progress/TASK-006/scratch/t5` — PASSED:
  hash = golden, 100 chunks before, HospitalSpawn (-591.5, 0.15, 537.84) along (0,0,-1), pickups at z 531.84 / 543.84;
  40 dmg → (60,0); on the armour pickup → (60,50); 30 dmg → (60,20); lethal → Wasted; `wasted.png` taken in Screen
  (state still Wasted after publish); Wasted lasted **4.52 s real time** (1.5 + 3.0 expected); same player entity,
  0.0 m from the hospital point, (100,0); 100 chunks after; no font/asset errors in the log. Screenshots:
  `scratch/t5/hud_damaged.png`, `scratch/t5/wasted.png`, `scratch/t5/respawned.png`, plus `scratch/t5/summary.json`.
  I viewed them: the Cyrillic "ПОТРАЧЕНО" renders in Inter Display Black over a grey frame, the HUD bars show top right,
  and the respawn frame shows the sidewalk beside the white hospital. No game process is left running.

Measured D3.1 (plan ranges in brackets): Screen first at k=96 [95..97], Playing at k=288 [287..289],
fixed ticks SlowMo 29 [27..31], Screen 191 [189..193].

Python parity (`scratch/python_parity.txt`): `--validate-only` gives valid for `valid_github_ofl`/`valid_minimal`;
url errors for `bad_github_url` and `bad_ofl_version`; "license OFL requires page" for `bad_license_source`;
"license 'MIT' is not CC0 or OFL" for `bad_license`. This matches the Rust verdicts.

### Flip-RED record (break → RED → restore → GREEN; full output in `scratch/flip_red.log`)
| Gate | Sabotage | Result |
|---|---|---|
| `anchor_on_segment_examples` | `t = len / 2` | RED |
| `hospital_anchor_on_sidewalk` | `t = len / 2` | RED (facing-hospital check, properties.rs:371) |
| `hospital_anchor_on_sidewalk` | `road_polygon` instead of the inset ring | RED (on-sidewalk check, :350) |
| `damage_table` + D2.1 `armor_absorbs_then_health` | health takes damage before armour | RED / RED |
| `regen_table` + D2.2 | remove `.min(cap)` | RED / RED (after the off-grid case; GREEN before, see deviation 1) |
| D2.2 | check `since` before `+=` | RED |
| D2.3 `pickups_heal_armor_and_respawn` | drop the `available()` check | RED |
| D2.5 `debug_damage_through_reflection` | remove `#[reflect(Message)]` | RED |
| D2.6 `dead_player_cannot_walk` | disable the `if dead` branch | RED |
| D3.1 | `advance_wasted` on `Res<Time>` (virtual) | RED (Screen timing) |
| D3.1 + D3.2 + D3.3 | remove `restore_time_scale` | RED (all three) |
| D3.1 | despawn + spawn a new player on respawn | RED ("respawn keeps the player entity") |
| D3.1 | `enter_wasted` without slow-mo | RED |
| D3.3 `respawn_keeps_world_one_shot` | `spawn_player` on `OnEnter(Playing)` | RED |
| D3.3 | `spawn_pickups` on `OnEnter(Playing)` | RED |
| D4 `health_regen_cap_is_validated` | drop the `regen_cap` check | RED |
| D4 `pickup_spacing_must_exceed_radius` | drop the spacing check | RED |
| E5 `manifest_fixtures_are_judged` | Kenney-only source for OFL | RED (valid_github_ofl) |
| E5 | OFL accepted with a Kenney page | RED (bad_license_source; GREEN with the old keyword) |
| E5 | CC0 rules applied to OFL | RED |
| E5 `local_assets_match_manifest` | OFL marker → CC0 text | RED |
| G1 `city_is_built_once_across_respawn` | revert F1 to `OnEnter(Playing)` | RED |

Gate classes: all of B3, C1, D2.1–D2.3, D2.6, D3, D4, E5 and G1 check correctness. D2.4 checks data liveness; its mechanism is
the D4 spacing gate. D2.5 checks liveness of the BRP path. t5.py covers runtime liveness and provides screenshots.

## 4. How to verify manually (owner checklist, G3; for QA_REPORT)

1. `python tools/fetch_assets.py`. **Every existing checkout must re-run this**: without the new `inter` pack,
   `local_assets_match_manifest` fails with "partial install: packs ["inter"]" and the client's preflight refuses to start.
2. `cargo run --features fast,debug`. The loading screen reads "Генерация города (seed N)" and the Cyrillic renders.
3. Press F5 a few times: the red bar (top right) shrinks by 25 each press.
4. Walk along the sidewalk to the hospital (white building). The pickups are cubes 6 m either side of the respawn point:
   a white-green medkit and a blue armour cube. The blue bar fills; F5 then eats armour first.
5. Drop below 50% and wait 5 s: health regenerates and stops at half.
6. Die (F5 until zero): slow motion for 1.5 s and the frame turns grey, then "ПОТРАЧЕНО" for 3 s, fully visible and not clipped.
7. You respawn at the hospital with full health. The camera snaps to the player (no fly-through) and the city does
   not flicker or double. There is no death animation (Q2, expected).

Owner-feel items (not gated by tests): bar look, slow-mo readability, title size/colour, cube size/colour. All
of these are data in `assets/ui/strings.ron`, `assets/flow/respawn.ron` and `assets/world/render.ron`.

Known limitation (from the plan): a `DebugDamage` written during `Wasted` stays buffered and may apply on the first
tick after respawn. It affects debug/BRP only.

children: 0 launched / 0 reported.
