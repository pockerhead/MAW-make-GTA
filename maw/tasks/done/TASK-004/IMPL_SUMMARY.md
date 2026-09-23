# IMPL_SUMMARY — TASK-004 (GDD T3: облик города)

Status: DONE. Steps 1-20 implemented. All headless gates are green, and the runtime t3 scenario
passed on this machine (the implementer ran natively, not in the codex sandbox).

## Runtime self-check (t3, native, release + dev)
`python tools/qa/scenarios/t3.py --out maw/tasks/in_progress/TASK-004/scratch/qa/t3` — exit 0.
- CityLayoutHash 0xd2158922f1e5cd6f == golden. Spawned: 100 `CityChunk`, 4237 `CityProp`.
- Tower roof y 156.0; player after teleport stands at y 157.05 (plan prediction 157.05), so the
  compound collider holds. Park teleport: y 1.20 (curb 0.15 + 1.05).
- Roof FPS over 5 samples: min 138.7, avg 141.8, max frame time 7.2 ms, present mode Fifo, focused.
  This is evidence for the owner, not a gate.
- Log: no ERROR lines with wgsl/shader/gltf/asset/"Failed to load" (the facade WGSL compiled; GLB props loaded).
- Screenshots in `scratch/qa/t3/`. `park.png` shows windows per floor, lit wall faces (winding is right),
  Kenney trees, lamps, shadows and fog. **Note for QA/owner:** the roof shots are taken from the roof
  centre, and there they show mostly the roof and the sky. The roof edge hides everything within about
  550 m, and fog hides everything past 450 m. The `dy=150` tilt (−18°) does not change that. To see the
  city from the roof, walk to the edge in the owner run.
- No game process left running after the run.

Pre-flight: every file/type/API named by PLAN_FINAL was checked against the code and the pinned
registry sources (bevy 0.19.1, avian3d 0.7.0, parry3d 0.27.0, bevy_brp_extras 0.22.6). No
PLAN_BLOCKED mismatch.

## 1. What was implemented

Git / assets (owner rule: binaries never committed)
- `.gitignore` +4: `/assets/third_party/*` + `!/assets/third_party/manifest.ron`. `git add -A --dry-run`
  shows only `assets/third_party/manifest.ron` from that tree. `.gitattributes` unchanged.
- `assets/third_party/manifest.ron` (new, 53) — 3 Kenney CC0 packs, 12 files, SHA-256 verbatim from the plan.
- `tools/fetch_assets.py` (new, 372) — stdlib only; RON-subset parser, schema mirror of `validate()`,
  `--check`, `--validate-only FILE`, fetch with `--cache DIR` (repeatable) + `target/asset-cache/`,
  4 download attempts (2/4/8 s), atomic per-pack publish via `.tmp-<name>` / `.old-<name>`.

gta_sim
- `crates/gta_sim/src/config/manifest.rs` (new, 162) — `ThirdPartyManifest` schema (single source),
  `validate`, `missing_files`, `contains_asset`; `config/mod.rs` +2 (`pub mod manifest;`).
- `crates/gta_sim/Cargo.toml` +1: dev-dep `sha2 = "=0.10.9"`; `Cargo.lock` +7 packages (resolved
  offline, same set as `scratch/cargo_lock_diff.txt`).
- `crates/gta_sim/tests/asset_manifest.rs` (new, 170) + `tests/fixtures/manifest/*.ron` (10 fixtures).
- `crates/gta_sim/src/world/city.rs` (+~80): `CityBlock` (one convex-hull static collider per block,
  slab bottom to curb top; degenerate → `AppExit::error()`), `CityLandmarks` resource (Reflect,
  registered), `PlayerSpawn.y = curb_height`, `building_collider` (cuboid, or compound of base + tiers).
- `crates/gta_sim/src/world/mod.rs`: exports `CityBlock`, `CityLandmarks`, `Landmarks`, `Tier`,
  `centroid`, `contains_convex`; registers `CityLandmarks`.
- Tests: `tests/common/mod.rs::settle` re-anchored on `PlayerSpawn.y + float_height`;
  `tests/city.rs::one_static_collider_per_building` re-anchored (compound parts: half extents + local y);
  new `landmarks_resource_matches_layout`.

citygen
- `params.rs`: `roads.curb_height`, `MassingParams`, `LandmarkParams`, validation incl. "tower is the
  tallest"; test case `tower_floors = 30`. `assets/world/city.ron`: `curb_height: 0.15`, `massing`, `landmarks`.
- `layout.rs`: `BuildingKind::Tower`, `Building.upper_tiers`, `Tier`, `Landmarks`, `GenError::NoLandmarkCandidate`.
- `landmarks.rs` (new, 44): plaza/park choice. `lots.rs`: `protected_blocks`, plaza = single lot + `fit_tower`,
  `shrink_to_fit`, `tiers` (+ unit test on the plan's worked examples). `geom::centroid` public.
- `hash.rs`: schema v2 (tiers, Tower code 4, plaza/park/tower indices). Golden re-blessed.
- `tests/properties.rs`: `landmarks_exist`, `setback_tiers_nested`.

Client
- `src/visuals/config.rs` (new, 340): `RenderConfig` moved here, all new sections, `validate()`,
  `prop_asset_paths()`. `assets/world/render.ron` rewritten to the plan's content.
- `src/visuals/facade.rs` (new, 56) + `assets/shaders/facade.wgsl` (new, 59): `ExtendedMaterial` window grid.
- `src/visuals/city_mesh.rs` (new, 421): pure `build_city_meshes` → exactly n² chunk meshes (asphalt,
  curb walls, sidewalk ring, lot/park/plaza tops, buildings with tiers and facade UVs, markings:
  avenue centre line, street dashes, lane dividers, crosswalks). Markings stayed in this file (< 750).
- `src/visuals/props.rs` (new, 340): pure `place_props` (lamps, street trees, park trees, traffic lights,
  containers), `PropAssets` loading GLB primitive 0/0, `prop_bundle` with `VisibilityRange`.
- `src/visuals/city.rs` (rewritten, 113): `CityVisualsPlugin` — async build in `AsyncComputeTaskPool`,
  `poll_once` polling, budgeted spawn from `spawn_budget`; no per-building `Mesh3d`.
- `src/visuals/sky.rs` (new, 84): `DistanceFog` (linear) observer on `Camera3d`, gradient unlit sky dome
  following the camera. `src/visuals/mod.rs`: modules, `MaterialPlugin::<FacadeMaterial>`,
  `DirectionalLightShadowMap`, `ClearColor` = fog colour, cascade config on the sun.
- `src/visuals/city_gate.rs` (new, 197, `#[cfg(test)]`): headless merge gate + manifest-path gate.
- `src/main.rs` (+44): startup `preflight` (render validate, manifest validate, prop paths listed,
  files present → clear message naming `python tools/fetch_assets.py`, exit code 1, no window).
- `tools/qa/brp.py` (+4/−4): `release=False` option. `tools/qa/scenarios/t3.py` (new, 149).

## 2. Deviations from plan
- `gta_sim::world` also re-exports `citygen::contains_convex` (`crates/gta_sim/src/world/mod.rs:10`):
  the client has no direct `citygen` dependency and the plan's container fit calls it.
- `mark_parks` takes the `protected` vector computed once in `lots::build` instead of calling
  `protected_blocks` itself (`crates/citygen/src/lots.rs`); behaviour identical.
- Prop randomness: nested `splitmix64` over (seed, block, i, j) instead of a single XOR of the four,
  to avoid `block ^ i` collisions (`src/visuals/props.rs::hash`).
- Container row width/length uses `max(container_a.scale, container_c.scale)` so A and C share one pitch.
- t3 reads `CityLandmarks` via its own `resource_value()` helper: `Game.resource()` in brp.py casts to
  `int` and cannot return a struct; brp.py otherwise changed only by `release`.
- `fetch_assets.py`: a cached zip with the wrong archive hash is reported on stderr and ignored, then
  the tool downloads; with no network this ends in a download error (negative verified offline).

## 3. Test results
- `cargo build` — green. `cargo clippy -- -D warnings` — green.
  `cargo clippy --workspace --all-targets -- -D warnings` — green (no pre-existing warnings either).
- `cargo test -p citygen` — lib 8, golden 3 (+1 ignored bless), properties 11 — all ok.
  `cargo test -p citygen --release --test perf -- --ignored` — seeds 1/2/42: 1.96/1.59/1.80 ms.
- `cargo test -p gta_sim` — lib 1, asset_manifest 3, city 6 (+1 ignored budget), config 4, jump 3,
  movement 4, terrain 2 — all ok.
- `cargo test -p gta_like --bin gta_like` — 7 ok (city_gate 2, city_mesh 2, props 3).
- `python tools/qa/tree_check.py` — passed. `python tools/fetch_assets.py --check` — 0.
- Fresh-clone state (all `city-kit-*` moved out): asset_manifest prints SKIP and passes; city_gate green.
- Golden v2 (reasons: hash schema v2 encodes tiers + landmark indices; setback tiers on buildings of
  ≥ 12 floors; tower building on the plaza lot; plaza/park choice changes which blocks are parks/lots):
  `1 0xd2158922f1e5cd6f`, `2 0x0672ef45a54e0587`, `42 0xe62c12ebedcb8268`.
- fetch_assets checks: fetch with `--cache scratch/kenney` → 3 installed, 12 files; rerun → 3 "skipped";
  `--check` 0; empty tree → 1; flipped byte → 1 + refetch restores; `extra.glb` → 1; renamed commercial
  zip as roads in a cache → "archive sha256 f8b0…, manifest 2205…; ignored". Parity: `--validate-only`
  0 on shipped + valid_minimal, 1 on all 9 `bad_*` (`scratch/parity_validate_only.sh`).

## 4. Flip-RED record (sabotage → RED → restore → GREEN)
1. Per-building `Mesh3d` observer back: RED "buildings carry their own Mesh3d, left 1320 right 0".
2. Grid `n = 1`: RED "chunk entities with Mesh3d, left 1 right 100".
3. Tower skipped in the mesh: RED "building 677: roof corner … at 156 missing from chunk [5, 4]".
4. Extra ground `Mesh3d` spawned in the plugin: RED "a mesh outside the chunk grid … left 101 right 100".
5. `VisibilityRange` removed from the prop bundle: RED "CityProp without VisibilityRange".
   (1-5: `scratch/flip_red_city_gate.py`, output `scratch/flip_red_city_gate.out.txt`.)
6. Manifest with packs installed: one flipped byte in `light-square.glb` → RED naming file and both
   hashes; `extra.glb` → RED "files on disk differ"; `city-kit-industrial` moved out → RED "partial install".
7. `bad_hash.ron` := `valid_minimal.ron` → `manifest_fixtures_are_judged` RED. (6-7: `scratch/flip_red_manifest.sh`.)
8. Test params `tower_floors = 20` bypassing validate → `landmarks_exist` RED ("building 409 height 117
   not below tower 78"); `city.ron` `tower_floors: 30` → `shipped_city_config_loads_and_validates` RED.
9. No `CityBlock` spawned → `settle` RED at y = 1.0968 (expected 1.20).
10. Building collider always a cuboid → `one_static_collider_per_building` RED (tiered collider not a compound).
11. Preflight: `city-kit-roads` hidden → binary exits 1 before any window with 4 lines
    "missing third-party asset …; run `python tools/fetch_assets.py`".
(8b-10: `scratch/flip_red_gta_sim.sh`.)

## 5. How to verify manually
1. `python tools/fetch_assets.py` (or `--cache <dir with the Kenney zips>`), then `--check`.
2. `cargo run --release --features fast -- --seed 1`, then `--seed 2`.
3. `python tools/qa/scenarios/t3.py --out <dir>` — roof/park screenshots and FPS in `summary.json`.

## 6. Owner checklist (for QA to carry into QA_REPORT.md; not an automated gate)
1. Город выглядит как город: тротуары приподняты бордюром, на асфальте осевые, разделители полос и
   зебры, у зданий окна по этажам, у высоток видны уступы.
2. Районы различимы: Downtown (серо-синие высотки, бетонные лоты), Commercial (бежевый, средняя
   высота), Residential (низкие дома, зелёные лоты, деревья вдоль улиц), Industrial (низкие корпуса,
   контейнеры за зданиями).
3. Ориентиры: самая высокая башня на площади у центра видна издалека, центральный парк с деревьями.
4. Солнце даёт тени примерно до 350 м; небо — градиент; дальний план уходит в туман на 250–450 м без шва.
5. Фонари на тротуаре плечом к дороге; светофоры на углах авеню смотрят разумно; пропы исчезают
   дальше ~120 м без заметного «попа».
6. Игрок заходит с дороги на тротуар без застревания (бордюр 0.15 м); с крыши башни виден город.
7. FPS с крыши (t3 `summary.json`) — информация для владельца, не порог.
