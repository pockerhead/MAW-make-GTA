# IMPL_SUMMARY — TASK-003 (GDD T2: citygen v1 и прогулка), attempt 2

Pre-flight: all plan entities exist and match the assumed shapes (Bevy 0.19.1 / bevy_state / bevy_tasks /
bevy_remote / avian3d 0.7.0 APIs checked in the registry sources). The partial citygen draft from the
interrupted attempt 1 was reviewed against PLAN_FINAL: kept rng.rs, layout.rs, params.rs, roads.rs,
districts.rs, most of geom.rs; fixed a real bug in `clip_half_plane` (it dropped every frontage flag), grid
line ends made exact, districts now return `Result`. Everything else written in this attempt.

## 1. What was implemented

All plan steps 1-12. Line counts are file sizes after the change.

citygen (new crate content, Steps 1-5):
- `crates/citygen/Cargo.toml` (deps from attempt 1, as planned), `Cargo.lock` = `scratch/Cargo.lock.t2` (verified identical).
- `src/lib.rs` 51 (`generate`, re-exports), `params.rs` 283 (`CityParams`, `validate`, `district`,
  `half_carriageway`, `sidewalk`, unit test), `layout.rs` 111, `rng.rs` 50, `geom.rs` 229 (+4 unit tests),
  `grid.rs` 115 (`split_interval`, exact interval ends, unit test), `roads.rs` 201, `districts.rs` 115,
  `lots.rs` 191 (parks, OBB lot split with frontage, box fitting), `pois.rs` 75, `graphs.rs` 155
  (sidewalks, lanes, spawn), `hash.rs` 163 (FNV-1a canonical encoding, test vectors).
- `tests/common/mod.rs` 59, `tests/golden.rs` 53, `tests/properties.rs` 303, `tests/perf.rs` 27,
  `tests/golden_hashes.txt` 9 (blessed by the bless procedure: seed 1 `0xae2b9c82e875577e`,
  seed 2 `0x436944f502e8d30d`, seed 42 `0x37b9705bd7bc71a3`).
- `grep -nE "sin\(|cos\(|atan|powf|exp\(|ln\(" crates/citygen/src` is empty.

Data (Step 4): `assets/world/city.ron` 26 (new, exactly the plan's values), `assets/world/render.ron` +9 fields.

gta_sim (Steps 6-9):
- `src/flow/mod.rs` 17 (`GameState { Loading, Playing }`, `FlowPlugin`).
- `src/world/mod.rs` 75 (`WorldSource`, `WorldPlugin { source }`, `CITY_CONFIG`, `WorldSystems::Generation`
  gated by `in_state(Loading)`, re-exports), `src/world/city.rs` 138 (`CitySeed`, `CityLayoutHash`
  (Reflect), `City`, `CityParamsRes`, `CityBuilding`, `CityGround`, `CityEdgeWall`, async generation with
  `block_on(poll_once(..))`, ground slab, 4 edge walls, building colliders), `world/test_area.rs` comment.
- `src/lib.rs` `compose_sim(app, root, source)`; city.ron loaded + validated only for `City`.
- `src/player/mod.rs` `spawn_player` on `OnEnter(GameState::Playing)`.
- Tests: `tests/common/mod.rs` (StatesPlugin before compose_sim, `composed_app`, `city_app`, `golden`,
  `city_params`, `place_player` moved from terrain.rs), `tests/config.rs` (+2 tests), `tests/terrain.rs`
  (uses common `place_player`), new `tests/city.rs` 157 (5 gates + `#[ignore] city_startup_budget`).

Client (Step 10): `src/main.rs` (`parse_seed`, `--seed N` or clock seed, logged), `src/menu/mod.rs` 27
(loading screen, `DespawnOnExit(Loading)`, Latin text per Q3), `src/visuals/mod.rs` (RenderConfig fields),
`src/visuals/city.rs` 132 (`CityPalette` shared materials, building observer, `spawn_city_surfaces` on
`OnEnter(Playing)`, `flat_mesh` with fan `(0, i+1, i)`).

QA (Step 11): `tools/qa/brp.py` (`resource_path`, `resource`, `wait_resource`, `log_tail`,
`mutate_component`, `load_golden`), `tools/qa/scenarios/t1.py` (`--seed 1`, waits for `CityLayoutHash` and
Player), new `tools/qa/scenarios/t2.py` 105.

## 2. Deviations from plan

- `crates/citygen/src/geom.rs` `contains_convex`: eps is in metres (edge direction normalised) instead of
  cross-product units. With cross-product units the planned `1e-3` became ~1e-5 m on 80 m edges and
  `lots_do_not_overlap` failed on exact boundary vertices at |x|~500 m (f32 ulp ~3e-5). Metres is what the
  plan's tolerances mean. (log.jsonl decision)
- citygen additionally exports `contains_convex`, `convex_overlap`, `dist_point_segment` (plan's public API
  list omitted them, but its integration tests use them).
- Park/city visuals live in `src/visuals/city.rs` from the start (plan allowed the split).
- `city_startup_budget` uses `common::composed_app` (made `pub`) to time the loop itself.
- Flip-RED `lots overlap`: sabotage = second child's cut line shifted 1 m into the first child, not "second
  child = original polygon" (that recursion explodes to 2^16 leaves per block: a hang, not a RED). (log.jsonl)
- Flip-RED `road connected`: sabotage rewrites every edge touching the last z line to a self-loop. Only
  dropping the horizontal edges of that line (plan wording) leaves the nodes connected by the vertical
  edges, and removing edges before block construction panics in the lookup (plumbing, not the gate).
- Flip-RED `runtime golden`: `seed ^ 1` applied in `start_city_generation` (the generation input), so
  `CitySeed` stays correct and the hash assertion is the one that fires.

## 3. Test results

- `cargo build` (default features) — green; `cargo build -p gta_like --features dev` — green.
- `cargo clippy -- -D warnings`, `cargo clippy --workspace --all-targets -- -D warnings`,
  `cargo clippy --features dev -- -D warnings` — green.
- `cargo test -p citygen` — lib 7, golden 3 (+1 ignored bless), properties 9 (35 seeds: SEEDS ∪ 0..32), perf ignored — all pass.
- `cargo test -p gta_sim` — lib 1, city 5 (+1 ignored), config 4, jump 3, movement 4, terrain 2 — all pass
  (T1 gates unchanged numbers).
- `python tools/qa/tree_check.py` — passed (no bevy_render in gta_sim).
- Release budgets: `cargo test -p citygen --release --test perf -- --ignored --nocapture` → seed 1 1.95 ms,
  seed 2 1.51 ms, seed 42 1.66 ms (1060-1327 buildings); hashes equal the debug golden.
  `cargo test -p gta_sim --release --test city -- --ignored --nocapture` → time to Playing 16.2 ms, slowest
  update (apply frame, spawns ~1300 static colliders) 9.3 ms. No time-slicing needed.
- BRP runtime (self-check, native, windowed dev build; artifacts in `scratch/qa_t1`, `scratch/qa_t2`):
  - `t1.py` on `--seed 1`: W moved (0.0, -0.004, -4.46), yaw delta -0.419, FPS 136, shutdown passed.
  - `t2.py`: seed 1 hash `0xae2b9c82e875577e`, seed 2 `0x436944f502e8d30d` (== golden, differ, `CitySeed`
    == seed); teleport form that works: `world.mutate_components` with `path=""`, `value=[0.0, 1.2, 0.0]`
    on avian `Position` (recorded in `brp.py` docstring). After 1.5 s Position and Transform ≈ (0, 1.053, 0)
    for both seeds. Screenshots `spawn_1/2.png`, `center_1/2.png` show streets, sidewalks and downtown towers.
    FPS 146 / 150.
- Flip-RED: all 18 rows of plan table 4.2 shown RED then GREEN with `scratch/flip_red.py`
  (results + panic messages in `scratch/flip_red_results.json`). Highlights: golden (node_jitter 6.5) →
  hash 0xc89aac50970a24ea; grid old rule → step out of range; roads → 13 nodes unreachable; lanes one-way →
  52 lanes unreachable; no crossings → 574 sidewalk nodes unreachable; overlap 1 m; frontage (b) → "lot 0 has
  no frontage"; frontage (a) → side at 6.5 m, expected 10.5; no fitting → corner outside lot; no police → 0;
  no recolour → "found 0"; quantize without round → 1234; runtime seed^1 → hash mismatch; every 10th
  building skipped → 1194 != 1327; wall at g/2 → "no wall at 700.5"; no +X wall → player at x 708.76,
  y -117 (the plan's derived 708.7); spawn on centreline → 0 m (both citygen and gta_sim gates); sleep 2.1 s
  → perf 2.10 s and time to Playing 2.11 s.
- Scratch probes: `scratch/layout_dump` (+`layout_png.py`, `layout_seed1.png`) top-down render of seed 1.

## 4. How to verify manually / owner checklist (QA copies into QA_REPORT.md)

- `cargo run --release -- --seed 1`: black loading screen "Generating city (seed 1)", then the city; run
  along sidewalks and streets.
- Visible: asphalt streets, grey sidewalks, blocks of different heights (tall Downtown in the centre, low
  outskirts), green parks; hospital (white), police (blue), gang HQs (magenta) coloured.
- `--seed 2` (and no `--seed`) gives a different city.
- Edge: 100 m of ground beyond the last street, then an invisible wall — cannot fall off or pass.
- Watch for a visible "pop" when the loading screen goes away (feel, not a gate).
- Owner-visible notes from my screenshots: the clear colour is dark grey and downtown towers read as
  similar greys; tuning is in `render.ron`.

children: 0 launched / 0 reported.
