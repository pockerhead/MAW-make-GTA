# IMPL_SUMMARY — TASK-025: Tnua sensor entities orphaned on character despawn

Cost of error: silent and unbounded (about 1 orphan per second, invisible to BRP component queries), so it gets a real gate.

## Pre-flight (small-fix, spec = task.md)

- `TnuaSensorsSet` has no `linked_spawn`: confirmed at `bevy-tnua-physics-integration-layer-0.13.0/src/data_for_backends.rs:62-68`. That crate is **not** vendored (only `bevy-tnua-avian3d` is, `vendor/`), so a relationship-level fix would mean vendoring a second crate.
- Sensors are spawned through `with_related_entities::<TnuaSensorOf>`: `bevy-tnua-0.32.0/src/sensor_sets.rs:98-107`, confirmed.
- `TnuaSensorsSet` is re-exported at `bevy_tnua::TnuaSensorsSet` (`bevy-tnua-0.32.0/src/lib.rs:202`).
- Bevy 0.19.1 despawn order (`bevy_ecs-0.19.1/src/world/entity_access/world_mut.rs:1629-1710`): the `Despawn` observers run first, then the `on_discard` hooks. The target's `on_discard` hook (`relationship/mod.rs:306-326`) only strips `TnuaSensorOf` from the sources. That is exactly the orphan QA found, with only `TnuaProximitySensor` left. When the `Despawn` observer runs, the component and its source list still exist.

## 1. What was implemented

| File | Change |
|---|---|
| `crates/gta_sim/src/character/mod.rs` | +17 lines. Global observer `despawn_tnua_sensors(On<Despawn, TnuaSensorsSet>)`, registered in `CharacterPlugin`. It calls `try_despawn` on every sensor in the set. This is what `linked_spawn` would do. |
| `crates/gta_sim/tests/sensor_leak.rs` | New file, 344 lines. Five gates, described below. |

The fix is keyed on the Tnua relation, not on our `Character` marker or on call sites. Every despawn path (civilian recycle/far, corpse expiry and the corpse limit, gang far, police Leave/far, new city via `CityScoped`, recursive despawns) is covered by one observer. `On<Remove, Character>` was rejected: it also fires on a component removal without a despawn, and it is tied to our marker, not to the entity that owns the sensors. Decision logged in `log.jsonl`.

### Gates (`cargo test -p gta_sim --test sensor_leak`)

`all_entities` = `World::iter_entities()` minus `IsResource` entities. In Bevy 0.19 resources are entities, and a resource inserted during play is state, not a leak. It is not a component query, so a bare `TnuaProximitySensor` orphan counts. Every path asserts that (a) the despawn path really ran (`GATE BROKEN` otherwise), (b) no sensor recorded before the despawn is alive, and (c) the entity set equals the baseline taken before the characters were spawned.

| Test | Production path | Fixture |
|---|---|---|
| `recycled_civilians_leave_no_entity` | `population::despawn_far` recycle branch | 8 civilians on a 20 m square. `max_civilians = 0` (at the cap), `recycle_distance = 1`, `recycles_per_tick = 8`, view straight up |
| `expired_corpses_leave_no_entity` | `population::age_corpses` | 8 civilians killed (Health 0), `corpse_seconds = 0.5` |
| `leaving_police_leave_no_entity` | `police::dispatch::despawn_police` (Leave) | 4 patrol units at 1 star, then heat 0, so the FSM sends every unit to Leave. The graph lies inside the spawn ring, so the dispatcher adds none. View up |
| `new_city_leaves_no_entity` | `world::despawn_city` on `NEW_CITY` (Paused -> Loading) | seed-1 city, 120 ticks with a chase view (44 characters, 44 sensors), pause, then Loading with seed 2. Baseline = all entities after the first update |
| `idle_city_entity_count_stays_bounded` (long-run) | everything that runs while idle | seed-1 city, production caps, fixed chase view, 120 s (7200 ticks). Total entity count sampled every second. Liveness: >= 30 characters despawned. Correctness: max over 90-120 s <= max over 30-60 s + `SLACK` (10) |

Long-run result (3 runs, deterministic): 63 characters despawned, 44 alive. Entities min/max **1675/1676** over the whole 120 s, 1675/1676 in both windows.

### Flip-RED (input perturbed: the `.add_observer(despawn_tnua_sensors)` line commented out)

- `scratch/flip_red_no_observer.txt`: all 4 path gates RED. Recycle 8/8 sensors alive, +8 entities. Corpse RED. Police 4/4, +4. New city 44/44, +44. Each extra entity has only `TnuaProximitySensor`.
- `scratch/flip_red_idle.txt`: long-run RED. 30-60 s (1707, 1716) against 90-120 s (1728, 1738), +22 > SLACK 10, with 63 despawned.
- Restored: all GREEN (`scratch/green_sensor_leak.txt`, `scratch/sensor_leak_3runs.txt`).

SLACK = 10 comes from the measurement: the spread with the fix is 1, and the leak adds about 22 over the same interval. The margin is roughly 2x. If recycling slows, the liveness assert (>= 30 despawned) trips first.

## 2. Deviations / not implemented

- No plan (small-fix). Gang far-despawn and the corpse-limit path have no dedicated gate. They go through the same observer (it is keyed on the relation, not the path), and the long-run city gate plus the new-city gate include gang members that exist at that moment. The acceptance criteria name four paths, and all four are gated.
- Respawn (Wasted/Busted) does not despawn the player in this codebase (`grep despawn crates/gta_sim/src/flow` is empty), so there is nothing to gate there.
- Test-side knobs (`recycle_distance`, `corpse_seconds`, `recycles_per_tick`) are changed on the live `PopulationConfig` resource so the path fits on the 80 m test floor. The systems under test are the production ones.

## 3. Test results

- `cargo clippy --workspace --all-targets -j 4 -- -D warnings`: exit 0 (`scratch/clippy.txt`).
- `touch crates/*/src/lib.rs; cargo test -p gta_sim -p citygen -j 4`: exit 0, 339 passed, 0 failed (`scratch/sim_tests.txt`). Includes the 5 new gates.
- `cargo test -p gta_like --bin gta_like -j 4`: 51 passed (`scratch/client_tests.txt`).
- `cargo test -p gta_sim --test sensor_leak` x3: 5/5 each run, identical numbers (`scratch/sensor_leak_3runs.txt`).
- Runtime `python tools/qa/scenarios/t8.py --out scratch/t8`: exit 0 (first fill 40 in 0.157 s, cap in 1.4 s, scatter 0.917, no log errors), `scratch/t8.log`.
- Runtime `python scratch/idle_entity_probe.py` (release `--features dev`, seed 1, `--settings-id ...qa` via brp.py, 3 min idle): total entities from BRP `world.query` with no data and no filter = **7382 at all 19 samples** (every 10 s, 0..180 s). Civilians stayed at 40 while **81 of 121 civilians seen were despawned**, so recycling was live. Final census: 38 sensors, 38 owned, **0 orphans**. No log errors. Before the fix, QA TASK-013 probe C measured 31 -> 95 orphans over 60 s idle. `scratch/idle_entity_probe/summary.json`, `scratch/idle_entity_probe.log`. The first probe version (census every 90 s, about 100 s per census over BRP) is kept as `summary_run1_slow_census.json`: 0 orphans at 0/106/214 s, total 7382 throughout.
- No `gta_like` process left running (`tasklist` is empty).

## 4. Manual verification

1. `cargo test -p gta_sim --test sensor_leak -- --nocapture`: 5 pass, and the idle line prints the min/max.
2. Comment out `.add_observer(despawn_tnua_sensors)` in `crates/gta_sim/src/character/mod.rs` and rerun: all 5 go RED, listing the bare `TnuaProximitySensor` entities. Restore it.
3. `python maw/tasks/in_progress/TASK-025/scratch/idle_entity_probe.py`: the total entity count stays flat for 3 minutes, and the census reports `orphans: 0`.

## Proposals

`PCTX_PROPOSALS.md`: (1) in Bevy 0.19, "all entities" includes resource entities (`IsResource`), so leak diffs must filter them; (2) `NEW_CITY` is `Paused -> Loading`, so a headless test has to pause before it sets Loading.

children: 0 launched / 0 reported
