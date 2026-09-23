## Counter-example tested

The runtime may already generate a city from `--seed` and spawn its streets, parks, and player into that layout; if so, the task's claimed implementation gap is false.

## Primary-source investigation

- `crates/citygen/src/lib.rs:1` contains only a module comment; there is no generator implementation in the crate's source file.
- `crates/gta_sim/src/world/mod.rs:18-23` initializes `PlayerSpawn(Vec3::ZERO)` and registers `spawn_test_area` at startup.
- `crates/gta_sim/src/world/test_area.rs:6-19,21-35` defines fixed block geometry and spawns it without a seed input.
- `crates/gta_sim/src/player/mod.rs:18-29` spawns the player from that `PlayerSpawn` resource.
- `src/main.rs:18-58` constructs and runs the app without parsing a `--seed` argument.

## Did it hold

No. The actual runtime composition uses the fixed test area and origin spawn. The city generator crate has no implementation, so the proposed existing seeded city was not found in the code that runs.

## Verdict

PREMISE HOLDS — `crates/citygen/src/lib.rs:1`; `crates/gta_sim/src/world/mod.rs:18-23`; `crates/gta_sim/src/world/test_area.rs:6-35`; `src/main.rs:18-58`.