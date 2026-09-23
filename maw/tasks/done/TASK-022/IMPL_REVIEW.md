# TASK-022 implementation review

## Verdict

**NEEDS_WORK** — the occlusion rule works in the tested city, but the required 20-second street-life target is not demonstrated and the observed run contains failing 20-second windows.

## Disconfirmation checked

Counter-example: a steady-state spawn inside the camera cone with a clear camera ray after the per-tick ray budget is exhausted. In `population/mod.rs:443-456`, a budget-exhausted in-cone candidate is skipped; otherwise spawning requires `occluded` to pass. The seed-1 turning test observed 107 spawns, including 64 occluded in-cone spawns, and no clear in-cone spawn. This counter-example did **not** hold in the tested run.

## Confirmed correct

- `crates/gta_sim/src/population/mod.rs:228-242,443-456` uses the existing `sight_blocked` query, which filters to `GameLayer::World`, for head and feet rays; in-cone spawns consume at most the configured budget. The Avian 0.7.0 pinned `SpatialQuery::cast_ray` and `SpatialQueryFilter::from_mask` signatures match the call path.
- `crates/gta_sim/src/population/mod.rs:405-430,458-498` applies forward preference in the steady phase, includes edge points, and preserves cap and separation checks. `docs/design/GDD.md:260` was amended to authorize edge points; the summary's statement that the GDD was not edited is stale.
- `crates/gta_sim/tests/street_spawn.rs:128-206` exercises real seed-1 geometry, both hidden and clear ring nodes, turning views, spawn visibility, and the ray bound through the production headless composition. The recorded flip-RED runs perturb the occlusion result, and the current focused test passed.
- `assets/npc/population.ron:3-12` owns the new tuning values; `PopulationConfig::validate` checks finite values and the relevant bounds. No new tuning constant was added to production code.
- I reran `cargo test -p gta_sim --test street_spawn -- --nocapture`, `cargo clippy --workspace --all-targets -- -D warnings`, and `cargo tree -p gta_sim -e normal -i bevy_render`; all passed, and the render reverse tree was empty. The supplied test and runtime logs report the other requested gates passing.

## Issues

1. **Major — `crates/gta_sim/tests/street_spawn.rs:263-324`: the density gate measures a different interval and duration from the acceptance criterion.** It waits until every initial-fill civilian has despawned (47.5 seconds in the rerun), then averages 40 seconds. The task requires a 20-second forward walk with mean at least 5. The test itself reports 20-second windows of **3.26..8.60**, so a 20-second walk can fail while this gate passes at 7.63. The first 20 seconds of the printed run also contain mostly 0-2 civilians in view. `OPEN_DECISIONS.md` records the orchestrator's acceptance of the longer steady-state gate, but it does not establish that the original 20-second player experience meets the target. **Suggested fix:** either meet and gate the stated 20-second interval, or explicitly revise that acceptance target and report the early empty-street behavior for owner acceptance.

## Missing coverage

- `crates/gta_sim/src/population/mod.rs:412-430` rebuilds, scores, and sorts all eligible graph points whenever there is a population deficit. `civilian_bench` spends most measured ticks at its cap with an upward-looking camera, so its 673 µs mean does not isolate sustained deficit ticks with occlusion rays. A targeted fixed-tick mean under sustained turnover would establish the frame-cost claim; the 16-ray bound alone cannot bound candidate enumeration and sorting.
- The visible head/legs check remains an owner-run visual judgment, as the summary notes. The tests check two ray endpoints at configured heights, not the rendered character silhouette.

children: 0 launched / 0 reported.
