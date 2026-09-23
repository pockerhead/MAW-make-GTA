# IMPL_REVIEW — TASK-006

## Verdict

**NEEDS_WORK** — gameplay and presentation gates pass, but the runtime screenshot gate can accept a stale image as new evidence.

## Disconfirmation tested first

Counterexample: respawning at the hospital consumes a pickup on the first fixed tick or starts a second city build. It did **not** hold for the shipped config: `spacing = 6.0` exceeds `radius = 1.0` (`assets/character/health.ron:8`), `HealthConfig::validate` enforces the separation (`crates/gta_sim/src/character/health.rs:78`), and player, pickup, and city spawns use `OnTransition { Loading, Playing }` (`crates/gta_sim/src/player/mod.rs:29`, `crates/gta_sim/src/combat/mod.rs:16`, `src/visuals/city.rs:26`). The headless respawn and client city tests passed.

## Confirmed correct

- Armour absorbs damage before health, regeneration waits for the configured delay and stops at 50%, and health/pickup tuning is loaded from strict RON config (`crates/gta_sim/src/character/health.rs:48`, `crates/gta_sim/src/player/mod.rs:61`, `crates/gta_sim/src/combat/pickups.rs:50`).
- Death enters `Wasted`; the phase clock uses `Time<Real>`, the slow-motion speed is restored at `OnExit(WastedPhase::SlowMo)`, and the same player entity respawns at the hospital with full health (`crates/gta_sim/src/flow/mod.rs:40`, `crates/gta_sim/src/flow/wasted.rs:73`). The city and headless tests passed.
- The hospital anchor uses the sidewalk inset ring and keeps both pickups on its side for the tested seeds (`crates/citygen/src/graphs.rs:153`, `crates/citygen/tests/properties.rs:320`).
- The manifest validator rejects an OFL pack with a Kenney source; `bad_license_source.ron` produced the specific `OFL requires page` error in the current Python validator (`crates/gta_sim/src/config/manifest.rs:156`, `crates/gta_sim/tests/asset_manifest.rs:63`, `tools/fetch_assets.py:191`). This verifies the relevant `dead_end` log pointer against current code and command output.
- The saved `scratch/t5/wasted.png` visibly shows “ПОТРАЧЕНО” over a desaturated frame. `cargo test -p gta_sim -p citygen`, `cargo test -p gta_like --bin gta_like city_is_built_once_across_respawn`, `cargo clippy --workspace --all-targets -- -D warnings`, and `python tools/fetch_assets.py --check` passed in this review.

## Issues

1. **Major — `tools/qa/scenarios/t5.py:69-75,148`: stale screenshots can satisfy the runtime QA gate.** `run()` reuses the output directory, and `screenshot()` only waits for `path.is_file()` and a PNG signature. If a PNG from a previous run is already there, it returns immediately even when the new capture has not been published; the subsequent `Wasted` state check can therefore certify an old frame. I reproduced this with `scratch/stale_capture.png` and a capture stub that wrote nothing: `screenshot()` returned success and the old bytes remained. **Suggested fix:** remove or uniquely name each capture target before requesting it, then wait for a newly published PNG; add a repeat-run/stale-file test for the helper.
2. **Minor — `crates/citygen/src/graphs.rs:180` (via `:19-23`): malformed layout indices can panic.** `sidewalk_anchor` documents `None` for a bad index and checks building, lot, block, and road-edge indices, but `road_polygon` indexes `roads.nodes[n as usize]` directly. A block containing an out-of-range node index panics instead of returning `None`. Generated layouts are valid, so this is a public helper contract and robustness issue rather than a demonstrated shipped-seed failure. **Suggested fix:** validate node indices before calling `road_polygon`, or make the polygon lookup fallible and propagate `None`.

## Missing coverage

- A stale `wasted.png` in an existing QA output directory should not let `screenshot()` pass without a fresh capture.
- A malformed `CityLayout` with an out-of-range block node should return `None` from `sidewalk_anchor` without panicking, matching its documented contract.

The current visual appearance and slow-motion feel still require the owner-run check specified in the task. I did not rerun the live BRP scenario; its saved screenshot was inspected as an artifact, while the runtime gate defect above was reproduced independently.

children: 0 launched / 0 reported.
