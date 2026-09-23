## Counter-example tested

Returning from `Wasted` to `Playing` spawns a second set of city visual meshes while city-chunk and building-collider counts stay unchanged, so the stated city gate passes despite duplicated visuals.

## Primary-source investigation

- `src/visuals/city.rs:24` schedules `start_city_mesh_build` on every entry to `Playing`; `src/visuals/city.rs:70` starts a new mesh task without checking whether the city was already rendered.
- `src/visuals/city.rs:100` and `src/visuals/city.rs:101` put `CityChunk` and `Mesh3d` on the same spawned entity. `src/visuals/city.rs:108` spawns props separately.
- `src/visuals/city_gate.rs:111` counts entities carrying `CityChunk` and `Mesh3d`; `src/visuals/city_gate.rs:70` and `src/visuals/city_gate.rs:71` show the existing gate waits for mesh work to finish before inspecting the city.
- `crates/gta_sim/src/flow/mod.rs:5` currently defines a state enum with `Loading` and `Playing`, and no `Wasted` state.

## Did it hold

No. A completed second city mesh spawn would increase the city-chunk count, because each visual chunk is spawned with `CityChunk` and `Mesh3d` together (`src/visuals/city.rs:100`, `src/visuals/city.rs:101`). The unguarded `OnEnter(Playing)` registration confirms the duplicate-spawn hazard named by the task (`src/visuals/city.rs:24`, `src/visuals/city.rs:70`). I found no primary-source evidence that the tested case can leave the specified chunk count unchanged.

## Verdict

PREMISE HOLDS — `src/visuals/city.rs:24`, `src/visuals/city.rs:100`, and `src/visuals/city.rs:101` show the re-entry hazard and show that a second completed city visual spawn would raise the stated chunk count.
