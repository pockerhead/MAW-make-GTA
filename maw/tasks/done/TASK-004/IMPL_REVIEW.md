# Implementation review — TASK-004

## Verdict

**NEEDS_WORK** — the city implementation passes its headless checks, but the roof QA evidence does not show a city overview, and asset replacement is not failure-atomic as planned.

## Disconfirmation tested

Counterexample: a chunk mesh could pass the entity-count gate while omitting visible block surfaces. I checked `build_city_meshes` and the gate: the current builder calls `add_blocks` and `add_markings` (`src/visuals/city_mesh.rs:186-188`), so the omission is **not present**. The gate checks chunk counts and a roof-corner vertex for each building (`src/visuals/city_gate.rs:101-161`); removing `add_blocks` would still pass it. This is a limit of the gate, with visual acceptance left to the owner run.

## Confirmed correct

- The render path creates one `Mesh3d` per chunk and no per-building mesh; the headless gate derives the expected count from city size and `render.ron` (`src/visuals/city.rs:89-111`, `src/visuals/city_gate.rs:92-133`).
- City generation adds a static curb collider per valid block, compound colliders for tiered buildings, and reflected landmark coordinates (`crates/gta_sim/src/world/city.rs:49-56,96-109,148-220`; `crates/gta_sim/src/world/mod.rs:59-64`).
- Manifest parsing, hash checks, expected file lists, and missing-asset startup messages are implemented (`crates/gta_sim/src/config/manifest.rs:46-160`, `crates/gta_sim/tests/asset_manifest.rs:74-170`, `src/main.rs:46-85`). Binary packs are ignored while the manifest is tracked (`.gitignore:53-55`).
- The implementation commit contains no change to `crates/citygen/src/roads.rs`, resolving the sole relevant `dead_end` log pointer. No `unsafe` code was added.
- Reran `cargo build`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo test -p citygen -p gta_sim`, `cargo test -p gta_like --bin gta_like`, `python tools/fetch_assets.py --check`, and `python tools/qa/tree_check.py`; all passed. The checked Bevy APIs (`VisibilityRange`, `DistanceFog`, `Mesh3d`, `On`) exist in the pinned 0.19.1 registry source. No Rust LSP tool was available in this session; reference searches used `rg`.

## Issues

1. **Major — `tools/qa/scenarios/t3.py:119-130`: roof FPS samples do not measure a city overview.** The script teleports to the tower roof centre, rotates and tilts the camera, then samples FPS at the same position. The supplied `scratch/qa/t3/roof_0.png` shows roof and sky; `roof_down.png` shows mostly roof with only a few buildings above its edge. Thus the reported 138.7–143.9 FPS samples do not support the planned “roof overview” evidence or demonstrate that the city can be seen from the tower. Move to a roof edge and face the city before capturing screenshots and FPS; verify the resulting screenshots with the owner. The city appearance itself remains an owner-run criterion, not an automatic FPS threshold.
2. **Major — `tools/fetch_assets.py:323-326`: a failed publish leaves an installed pack absent.** After moving `final` to `.old-<name>`, failure of `os.replace(tmp, final)` is outside the cleanup `try` block, so `final` is missing. The scratch-only probe `scratch/probe_fetch_publish.py` injected failure on the second replace and observed `final_exists False`, `old_exists True`, `tmp_exists True`. This contradicts the plan's atomic publish/recovery guarantee and makes the game fail startup until a later fetch succeeds. Restore `old` to `final` on a failed second rename, and clean up the temporary directory; verify with an injected publish failure.

## Missing coverage

- The mesh gate proves chunking and building roof vertices, but does not detect missing sidewalk/curb/marking geometry or unreferenced building vertices. Keep those visual checks in the owner run; the present roof screenshots do not cover the required skyline view.
- There is no failure-injection test for the pack publish transition. The scratch probe above is reproducible evidence for this path.

children: 0 launched / 0 reported.
