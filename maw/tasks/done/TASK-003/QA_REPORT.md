# QA_REPORT — TASK-003 (GDD T2: citygen v1 и прогулка)

## 1. Environment

- Direct, in-place on `D:/test-gta-like`, branch `feature/t02-citygen-v1`, HEAD `f8db2d4`. Base for comparison is `d6cfe06`, the last commit before TASK-003.
- No docker or dev server. Headless gates ran through `cargo test`. The runtime check used the windowed `gta_like` dev build (`--features dev`, BRP on 127.0.0.1:15702), driven by `tools/qa/brp.py`.
- No services or containers were left running. After each run I checked for `gta_like` processes and none were left.
- Temporary file: `crates/citygen/tests/qa_sweep.rs`, used for my own sweep and deleted afterwards. A copy is in `scratch/qa/qa_sweep.rs`. The final `git status --short` shows only this report.

Reproduce:
```
cargo build -j 4
cargo clippy -j 4 --workspace --all-targets -- -D warnings
cargo clippy -j 4 --features dev -- -D warnings
cargo test -j 4 --workspace
cargo test -j 4 -p citygen --release --test perf -- --ignored --nocapture
cargo test -j 4 -p gta_sim --release --test city -- --ignored --nocapture
python tools/qa/scenarios/t2.py --out maw/tasks/in_progress/TASK-003/scratch/qa/t2
python maw/tasks/in_progress/TASK-003/scratch/qa/qa_runtime.py
# sweep: copy scratch/qa/qa_sweep.rs to crates/citygen/tests/, then
QA_SWEEP_N=1000 cargo test -j 4 -p citygen --release --test qa_sweep
```

## 2. Test results

### Disconfirmation (preflight step 3)

The counterexample I went looking for: the client uses a clock-derived u64 seed when `--seed` is not given. The author's property gates cover only 35 seeds (1, 2, 42 and 0..32). A seed outside that set could make `generate()` return `Err`, which exits the game. It could also hit the `expect` in `graphs::player_spawn` and panic inside the async task. It could also break a property: overlap, frontage, connectivity, POIs or the spawn position.

To test it, I copied `properties.rs` and replaced the seed source with 2007 seeds: 1000 splitmix64 u64 values, 100..1099, 0, 3, `u64::MAX`, `u64::MAX-1`, 2^63, 2^32, and a real clock-like value. Generation was wrapped in `catch_unwind`. I also added a check that every city has parks and that the tallest building is more than twice the height of the lowest. All 10 tests passed in release (4.05 s): no `Err`, no panic, no property violation. **The counterexample did not hold.**

### Existing suite

- `cargo build`: green.
- `cargo clippy --workspace --all-targets -- -D warnings`: green. `cargo clippy --features dev -- -D warnings`: green.
- `cargo test --workspace`: all green. I compared tests by name against `d6cfe06`. All base tests are present and pass: `camera_relative_axes`, config ×2, jump ×3, movement ×4, terrain ×2. There are no new failures.
- New tests: citygen lib 7, golden 3 (+1 ignored bless), properties 9, perf (ignored). gta_sim city 5 (+1 ignored), config +2.

### Release budgets (ignored gates, run by QA)

- `citygen perf`: seed 1 took 2.20 ms (1327 buildings), seed 2 1.85 ms (1060), seed 42 1.75 ms (1224). The release hashes equal the debug golden values.
- `gta_sim city_startup_budget`: time to `Playing` 24.9 ms, slowest update 16.0 ms (the apply frame). Both are far below the 2 s limit.

### Flip-RED (mine, different from the author's 18 rows)

The tree was clean at `f8db2d4` beforehand, and I recorded sha256 values in `scratch/qa/sha_before.txt`.

1. `crates/gta_sim/src/world/city.rs`: `PlayerSpawn(Vec3::new(spawn.x * 0.5, ...))` → `player_spawns_on_sidewalk_and_stands` went **RED**: "player at (3.60, 1.07, -40.23) is 4.90 m from the nearest street axis, sidewalk is [6.5, 10.5]".
2. `crates/citygen/src/pois.rs`: I placed the hospital twice → `pois_exist` went **RED**: "seed 1: hospitals left 2 right 1".
3. After `git checkout` of both files, sha256 matched the recorded values and both test files were GREEN again.

A side note, logged as a `dead_end`: swapping the spawn axes to (z, x) left the gate GREEN. That is not a gate hole. The swapped point happens to be on the sidewalk of the E-W street through the origin, so the property really does hold there.

### Runtime (BRP)

- The author's scenario, `tools/qa/scenarios/t2.py --out scratch/qa/t2`: **PASS**.
  - Seed 1 gives `CityLayoutHash` `0xae2b9c82e875577e` and seed 2 gives `0x436944f502e8d30d`. Both equal the golden values, they differ from each other, and `CitySeed` equals `--seed` in both runs.
  - Teleport through `world.mutate_components` on avian `Position` (path `""`, value `[0, 1.2, 0]`). After 1.5 s the player was standing at (0, 1.053, 0) for both seeds.
  - Screenshots:
    - `scratch/qa/t2/spawn_1.png`, `spawn_2.png`: the player capsule on a grey sidewalk, the dark asphalt street to the left, downtown towers on both sides.
    - `center_2.png`: the player in the middle of the centre intersection, sidewalk strips on both sides, towers ahead.
- My probes, `scratch/qa/qa_runtime.py`, summary in `scratch/qa/rt/summary.json`:
  - `--seed abc` exits with code 1 and prints `--seed expects an unsigned integer, got "abc"`. `--seed` with no value exits with code 1 and prints `--seed needs a value`.
  - Without `--seed`, two launches got clock seeds `1790152448484451200` and `1790152453407391300`. Each equals the `city seed` line in the log, and the hashes differ (`0x751011d7cb0bcdeb` vs `0x60edf24ccd3a4151`). Screenshot `rt/noseed_spawn.png` shows a different downtown street layout from seed 1.
  - I teleported into the nearest seed-1 park, at (43.98, -309.63), and the player stood at y 1.148. Screenshot `rt/park_seed1.png` shows green park ground, tan low-rise buildings around it, and a grey sidewalk line at the park edge.
  - I teleported to (0, 1.2, 690) and held S for 6 s. The player stopped at z = 699.7, which is the ground edge (700) minus the capsule radius (0.3). The invisible edge wall works at runtime. Screenshot `rt/edge_after_walk_seed1.png` looks back from the edge: the grey 100 m margin, the city skyline with tall grey-blue downtown towers in the centre, low tan outskirts, and green park strips. Block heights clearly vary.
  - Diagnostics: 139–156 FPS with `present_mode: Fifo` on the debug dev build. This is not a frame-cost measurement, and no FPS criterion applies to T2.

## 3. Acceptance criteria

| Criterion | Test performed | Result |
|---|---|---|
| Headless golden hash for 3 seeds | `golden_hashes_match` (seeds 1/2/42 against a separate fixture). The hash covers every `CityLayout` field (checked `hash.rs`). The release run gives the same hashes. The author flip-RED'd it with `node_jitter` 6.0→6.5. `different_seeds_differ` now hashes the geometry without the seed field (the fixer's claim holds, checked in the code) | PASS |
| Road graph connected, lane graph strongly connected | `road_graph_connected`, `lane_graph_strongly_connected` (forward and backward reachability from lane 0). My sweep covered 2007 seeds | PASS |
| Lots do not overlap and face a street | `lots_do_not_overlap` (SAT, every lot inside the block's `inner`), `lots_face_a_street` (an edge ≥ min_frontage on a non-alley side at the exact carriageway+sidewalk offset). 2007-seed sweep | PASS |
| Hospital, police station and 2 gang HQs exist | `pois_exist`, `gang_guarantee_recolors`. My flip-RED (duplicate hospital) went RED. 2007-seed sweep | PASS |
| Release generation < 2 s (`#[ignore]`) | `perf` 1.75–2.20 ms. `city_startup_budget` 24.9 ms to Playing | PASS |
| Runtime QA t2.py: `--seed 1/2`, hash == golden and different, teleport, ground screenshots | `t2.py` run by QA plus independent probes (clock seed, bad seed, park, edge wall) | PASS |
| Owner checklist recorded | Section 6 below | PASS (recorded; the owner's run is still pending) |
| New tuning values in GDD §12 data files | New `const`s checked: `GROUND_SLAB_THICKNESS` (collision geometry law), RNG stream ids, hash schema/quantum, and algorithm internals `MAX_SPLIT_DEPTH` / `FIT_SHRINK` / `FIT_STEPS` / eps values. No inline tuning literals in the citygen stages or the client visuals. Sizes, blocks, roads, districts, POIs and walls are in `assets/world/city.ron`. Colours and the layer step are in `render.ron` | PASS (see the note in Bugs about `FIT_*`) |
| `cargo build`, `clippy -D warnings`, `cargo test -p gta_sim -p citygen` green | Run by QA | PASS |
| Existing tests pass | Name-by-name comparison against `d6cfe06` | PASS |

Gameplay rules with a headless path through the production composition: `city_app` → `composed_app` → `compose_sim` (the same function `main.rs` calls). Gates by class:
- Correctness: golden, properties, collider values, wall positions, spawn position.
- Liveness plus correctness: `edge_wall_stops_player`, `player_spawns_on_sidewalk_and_stands`.

## 4. Bugs found

No functional defects found.

Notes, all non-blocking:
1. **Low, presentation, for the owner.** Lot interiors use the sidewalk colour. `src/visuals/city.rs` fills the whole `curb` polygon with sidewalk grey, so the ground under and between buildings reads as paving (visible in `spawn_1.png`). The reviewer flagged this and the fixer consciously skipped it as a presentation choice that T3 reworks. I agree it is owner-visible on the first frame, not a silent defect. It goes on the owner checklist.
2. **Low, presentation.** Downtown generic buildings (0.62, 0.64, 0.70) look almost the same as each other at street level. Commercial and residential tans are close. The values are in `render.ron`, so this is for the owner's taste.
3. **Info.** `FIT_SHRINK = 0.9` and `FIT_STEPS = 24` in `lots.rs` are fitting-loop internals. They are not values the owner would tune, so I don't count them as a data-first violation.
4. **Info, pre-existing, not T2.** `python tools/qa/brp.py --help` does not print help. It builds and launches the game and prints diagnostics, because `__main__` ignores argv. This also happens on `d6cfe06`.

## 5. Verdict

**SHIP-PENDING-RUNTIME.** Every covered criterion passes headless and at runtime:
- the golden hashes;
- graph connectivity;
- lot overlap and frontage;
- the POIs;
- the release budgets;
- the BRP scenario.

My independent 2007-seed sweep and two flip-REDs did not break anything. The owner criterion is still open: the feel of walking the city and the visual read of streets, blocks and parks. Checklist below.

## 6. Owner checklist (бегает по городу, улицы, кварталы разной высоты, парки; другой seed — другой город)

1. `cargo run --release -- --seed 1`. You should briefly see the black "Generating city (seed 1)" screen, then the player on a sidewalk next to a downtown street.
2. Run along the street and across intersections. You should see dark asphalt streets, grey sidewalks, and tall grey-blue towers in the centre. Further out the buildings are lower and tan.
3. Find parks: green lawns without buildings. In seed 1 the nearest park is at about (44, -310), south of the centre.
4. Look for coloured POIs: the hospital is white, the police station blue, gang HQs magenta. In seed 1 the hospital is near (-562, 538), police near (513, 295), and HQs near (-518, -309) and (114, -508).
5. Walk to the city edge. After about 100 m of plain ground past the last street there is an invisible wall. You should not be able to pass it or fall off.
6. Run `--seed 2`, then run with no `--seed` twice. Each should give a different city, and the log prints `city seed N`.
7. Judge by eye: lot interiors are sidewalk-grey (note 1), whether downtown grey is readable (note 2), and whether there is a visible "pop" when the loading screen disappears.

children: 0 launched / 0 reported.
