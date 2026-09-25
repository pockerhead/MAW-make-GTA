# FIX_SUMMARY — TASK-017, fixer round 2 (continuation)

Round 1 is `FIX_SUMMARY.prev-1.md`. The stopped round-2 fixer already committed the round-2 code:
- 91dd3d2: bench car on rails, a kinematic ghost at `bench.speed` (`src/bench/mod.rs` `drive`,
  `assets/world/render.ron` `bench`); t16 moving precondition (car above `bench.moving_speed` for at least 90 % of the
  measured window, otherwise `GATE BROKEN` with no verdict); streaming report.
- 1c2d54a: t16 counts police crews aboard as units (the dispatcher's count).

This continuation changed no code. It verified the tree, ran t16 three times, pushed 1c2d54a and checked CI.

Preflight: I read the scratch dir and `IMPL_REVIEW.md`. The review is the round-1 review, and round 1 already acted on
it (see prev-1). The riskiest prescription in it was the fire queue: clearing `Loadout.fire_queued` when the player
enters a car. I checked the committed code. The clear and its gate are in place, and the sim suite below is green.
Nothing from the review is open for this round.

## 1. Fixed

Nothing new. The orchestrator's round-2 items (ghost bench car, moving precondition, cheat named in the summary,
3-run re-measure, scene-qualified verdict) are done in 91dd3d2 and 1c2d54a. I verified them with the runs below. The
summary's `bench_cheats` lists: 5 stars pinned; player health and armour held at max; driven car health held at max;
"car on rails: kinematic lane route at bench.speed, rightmost turn at every junction, collides with nothing (traffic
and police cannot box it in)".

## 2. Skipped

- No optimisation, as ordered. No budget is breached (see 3.3).

### Phantom red (recorded, not a code bug)
The first client-gate runs after the continuation started failed 3/3 on
`bench::tests::the_ride_takes_the_right_turn_and_the_lane_after_it` ("left: Connector(0), right: Connector(1)").
The fixture arithmetic says connector 1 should win (lane 0 north, right = +X, lane 1 dir +X gives dot 1, lane 2
gives dot 0). A probe printed the fixture graph: lane 0 out [0, 1], conn 1 goes to lane 1. After that rebuild the test
passed. I restored the file byte-identical (`git diff` empty), rebuilt, and the gates passed 3/3. Cause: a stale test
binary in the shared `target/`, the same class as the TASK-009 lesson. Clippy had finished in 0.56 s against the same
stale fingerprints, so I touched `src/main.rs` and `crates/*/src/lib.rs` and re-ran clippy and the sim suite from that
rebuild. The results below come from those runs.

## 3. Test results

### 3.1 Headless (sequential, `-j 2`, after the touch)
| Command | Result |
|---|---|
| `cargo test -p gta_sim -j 2` | 58 binaries, 478 passed, 0 failed (`scratch/fix/r2c_sim.txt`) |
| `cargo test -p citygen -j 2` | 32 passed, 0 failed |
| `cargo clippy --workspace --all-targets -j 2 -- -D warnings` | clean (after touch, 15.7 s rebuild) |
| `cargo clippy -p gta_sim -p citygen --all-targets -j 2 -- -D warnings` | clean |
| `cargo test -p gta_like --bin gta_like -j 2` ×3 | 81 passed ×3 (`scratch/fix/r2c_client_x3.txt`) |

### 3.2 Runtime: `python tools/qa/repeat.py t16 --runs 3 --out target/qa/fix2c`
`t16: 3/3 passed` (441 s, 246 s, 256 s). Summaries: `scratch/fix/t16_r2c/run{1,2,3}.summary.json`.
Scene: seed 1, 1920×1080, 5 stars, 5 police cars, 12 units (SWAT; 2 on foot, 10 aboard), 40 civilians, 0 gang members
near the centre at composition (2 after). Log errors: none in either session.

| Run | moving share | mean FPS | 1 % low | min FPS | p99 ms | max ms | FixedMain mean / p99 ms | AI ms/tick | physics ms/tick | metres driven | chunks resident / visited | traffic (compose → start/end of window) |
|---|---|---|---|---|---|---|---|---|---|---|---|---|
| 1 | 100 % | 347 | 186 | 78 | 4.66 | 12.75 | 2.63 / 4.08 | 0.71 | 0.72 | 285.6 | 100 / 2 | 24 → 22 / 14 (spawned 101, despawned 109) |
| 2 | 100 % | 329 | 190 | 122 | 4.97 | 8.20 | 2.49 / 3.27 | 0.72 | 0.67 | 286.3 | 100 / 2 | 24 → 21 / 15 (99 / 105) |
| 3 | 100 % | 289 | 143 | 66 | 5.70 | 15.07 | 2.80 / 4.21 | 0.67 | 0.82 | 286.6 | 100 / 2 | 24 → 24 / 20 (100 / 104) |

FPS numbers are no-vsync, from `BenchFrames`, over the 30 s window. FixedMain, AI and physics come from the session-B
chrome trace (10 s window, with tracing). Budgets: frame 16.7 ms, FixedMain 4 ms/tick, AI 1.5 ms/tick.

Verdict (the t16 text, scene-qualified; run 1): "at the measured scene (14 traffic cars, 2 gang members near the
centre, 5 stars, 5 police cars, 12 units, bench car on rails moving 100% of the measured window, 285.6 m driven):
nothing to fix by trace: every budget holds". `over_budget` is empty in all three runs.

### 3.3 Budget read and observations for the orchestrator
- No breach. The worst mean FixedMain is 2.80 ms/tick against 4 ms. The worst frame is 15.07 ms, still under 16.7 ms.
  AI plus physics is at most 1.49 ms/tick. The trace top-10 for the worst run (run 3, ms per frame, with tracing):
  prepare_windows 2.70 (present wait), submit_pending_command_buffers 0.39, propagate_parent_transforms 0.27,
  prepare_clusters_for_gpu_clustering 0.20, prepare_preprocess_bind_groups 0.17, `gta_sim::traffic::spawn::spawn_traffic`
  0.15, animate_targets 0.14, collect_meshes_for_gpu_building 0.11, mark_dirty_trees 0.11, render_system 0.11.
- FixedMain p99 is just over 4 ms in runs 1 and 3 (4.08, 4.21). The budget is judged on the mean, and these are
  single-tick spikes with tracing on. Reported, not acted on.
- **The streaming claim is weak.** The car drives about 286 m per window on a block loop (always the rightmost turn).
  It visits only 2 chunks of 128 m, and the resident set stays at 100. The window therefore measures driving plus
  traffic churn (about 100 spawns and 105 despawns per window), but not heavy chunk streaming. Covering streaming would
  need a longer or straighter route. That is the orchestrator's call; this round did not change it.
- Traffic around the ghost thins during the window (24 → 14/15/20 by its end), and spawn/despawn churn is high. That
  fits a car at 10 m/s leaving the traffic bubble behind. It is a property of the scene, not a failure.

### 3.4 Push and CI
Pushed `feature/t16-final` 91dd3d2..1c2d54a. CI on 1c2d54a: all 5 workflows `success` (sim gates 36185606982,
client gates 36185607096, clippy 36185607076, citygen gates 36185607059, repo checks 36185607078).

## 4. Tree
`git status --short`: `OPEN_DECISIONS.md` modified (the orchestrator's edit, not mine, left as found) and this file.
No code changes.
