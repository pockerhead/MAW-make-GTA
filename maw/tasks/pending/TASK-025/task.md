# TASK-025: Утечка сенсоров Tnua при деспавне персонажей

Type: bug
Mode: small-fix
Priority: high
Branch: fix/tnua-sensor-leak
Domains: bevy-ecs, gates

## Description
QA TASK-013 (bug 1, `maw/tasks/done/TASK-013/QA_REPORT.md`): bevy-tnua 0.32 spawns proximity-sensor entities via `with_related_entities::<TnuaSensorOf>`; `TnuaSensorsSet` has no `linked_spawn` (`bevy-tnua-0.32.0/src/sensor_sets.rs:98-107`). Every character despawn (civilian recycling, deaths/corpse expiry, gang/police leave, new city, respawn) orphans an entity with only `TnuaProximitySensor`. Seed 1 idle: ~1 orphan/s; unbounded growth over a session. Fix at the root for ALL character despawn paths (e.g. an `On<Remove, Character>`/despawn hook that despawns the character's sensor targets, or a relationship-level fix if the vendored integration allows it) — surgical, no per-call-site patches scattered across domains.

## Acceptance criteria
- [ ] Headless gate: spawn and despawn N characters through every production despawn path (recycle, corpse expiry, police Leave, new city); total entity count returns to baseline (count ALL entities, not reflected queries); flip-RED by disabling the fix.
- [ ] Long-run gate: seed-1 city idle 120 s at the production cap — entity count bounded (report min/max), no monotonic growth.
- [ ] Runtime: `t8.py` + a 3-minute idle BRP probe reporting total entity count over time.
- [ ] `cargo clippy --workspace --all-targets -j 4 -- -D warnings`, `cargo test -p gta_sim -p citygen -j 4`, `cargo test -p gta_like --bin gta_like` green.

## Dependencies
- blocked by TASK-013
