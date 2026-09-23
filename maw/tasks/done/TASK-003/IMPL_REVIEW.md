# IMPL_REVIEW — TASK-003

## Verdict

**NEEDS_WORK** — The city runs and its headless gates pass, but the sidewalk mesh is geometrically wrong and two acceptance gates can pass without proving their stated properties.

## Disconfirmation tested

Counterexample: clipping a block could leave a lot marked as street-facing after its street edge was removed. I traced `clip_half_plane` and lot subdivision in `crates/citygen/src/geom.rs:64` and `crates/citygen/src/lots.rs:100`, then ran `cargo test -p citygen --test properties lots_face_a_street -- --exact`. The test passed across the 35 configured seeds; this counterexample did **not** hold. The property test checks each lot edge against a live, non-alley road edge (`crates/citygen/tests/properties.rs:139-204`).

## Confirmed correct

- `crates/citygen/src/lib.rs:26-51` composes deterministic grid, road, district, lot, POI and graph stages; `crates/citygen/tests/golden.rs:10-27` anchors three hashes to a separate fixture. Connectivity, frontage, non-overlap and POI property tests pass.
- `crates/gta_sim/src/world/city.rs:51-89` generates on `AsyncComputeTaskPool`, polls without waiting for completion, applies the layout, then enters `Playing`. The headless client and test use the same `compose_sim` (`crates/gta_sim/src/lib.rs:22-47`).
- `crates/gta_sim/src/world/city.rs:92-137` creates the ground, four edge walls and building colliders; the wall position and player stopping tests pass. `CitySeed` and `CityLayoutHash` are reflected and registered in `crates/gta_sim/src/world/mod.rs:51-65` for BRP.
- The changed dependency graph keeps rendering out of `gta_sim`; `python tools/qa/tree_check.py` passes. I also ran `cargo build`, `cargo clippy --workspace --all-targets -- -D warnings`, and `cargo test -p citygen -p gta_sim`: all passed. Release budget gates passed at 1.57–1.94 ms per generated city and 16.63 ms to `Playing` in the headless app. The supplied BRP summary records two golden hashes, successful teleports and four PNG screenshots; I inspected a screenshot, but owner visual acceptance remains pending.

## Issues

1. **Major — `src/visuals/city.rs:95-105`: the sidewalk mesh covers the entire block.** Every `curb` polygon is triangulated as a filled surface with the sidewalk material. The actual sidewalk is the strip between `curb` and `inner`; all non-park lot interiors are therefore rendered as sidewalk. This conflicts with GDD §2.2's carriageway/sidewalk inset geometry and is visible as broad grey paving under the buildings. Generate a ring mesh between the two contours, with a separate interior ground treatment. Keep the park interior green. The final plan specified the same filled-polygon approach, so this also needs correction relative to the GDD scope law.
2. **Major — `crates/gta_sim/tests/city.rs:47-49`: the “static collider” gate accepts dynamic bodies.** The query counts entities with *any* `RigidBody` component, while the stated property and test name require `RigidBody::Static`. Replacing every building body with `Dynamic` would leave this gate green; the existing flip-RED only removes some buildings. Query `&RigidBody` and assert every city building uses `Static`, then flip that variant to show RED. The production spawn currently sets `Static` (`crates/gta_sim/src/world/city.rs:126-134`).
3. **Minor — `crates/citygen/tests/golden.rs:37-43`: hash inequality does not prove two seeds make different cities.** `layout_hash` writes `layout.seed` before geometry (`crates/citygen/src/hash.rs:75`), so the inequality test and the analogous check in `tools/qa/scenarios/t2.py:99-100` can pass with identical roads, lots and buildings. Compare at least one seed-independent geometry digest or layout field between seeds, and flip-RED by making the generator use one RNG seed for both. The current generator does use the requested seed; this is a gap in the regression gate.

## Missing coverage

- No headless gate checks the **values** of building collider size and rotation against the generated building, only the component count. A collider with wrong extents could silently block streets or let the player pass through buildings.
- The release startup budget gate times `gta_sim` only (`crates/gta_sim/tests/city.rs:141-157`); client mesh creation in `src/visuals/city.rs:59-76,80-109` is outside that number. The current visual result and any loading-screen pop need the owner's run, as required by the project context. No automated visual-feel gate is warranted.

children: 0 launched / 0 reported.
