## Counter-example tested

The proposed chunk-entity count can match the city dimensions and `render.ron` chunk size while every building still has its own mesh, so that acceptance check could pass without merging building meshes.

## Primary-source investigation

Opened `crates/gta_sim/src/world/city.rs:116` and `crates/gta_sim/src/world/city.rs:127`: `spawn_buildings` loops over every layout building and spawns a separate `CityBuilding` entity for each. Opened `src/visuals/city.rs:54` and `src/visuals/city.rs:73-75`: the `CityBuilding` add observer inserts a newly created cuboid `Mesh3d` on that same entity.

## Did it hold

Yes. The current per-building `Mesh3d` path exists independently of any future chunk entities (`crates/gta_sim/src/world/city.rs:127`; `src/visuals/city.rs:73-75`). Counting chunk entities alone would leave that path unconstrained: the count could be correct while the building meshes remain separate.

## Verdict

PREMISE SUSPECT — `crates/gta_sim/src/world/city.rs:116`, `crates/gta_sim/src/world/city.rs:127`, and `src/visuals/city.rs:73-75` show one mesh-bearing entity per building, independent of the proposed chunk count; smallest implied reframing: Treat the chunk-entity count as insufficient evidence that building meshes were merged.
