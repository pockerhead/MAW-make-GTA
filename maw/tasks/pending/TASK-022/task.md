# TASK-022: Живая улица — спавн мирных из-за угла и впереди

Type: feature
Mode: small-fix
Priority: high
Branch: feature/street-density
Domains: bevy-ecs, gates, game-design

## Description
QA TASK-009 (finding 1, `maw/tasks/done/TASK-009/QA_REPORT.md`, repro `maw/tasks/done/TASK-009/scratch/qa/qa_street_life.py`): with 40 civilians alive, the camera cone within 60 m holds 0-3 of them, and walking forward for 8 s drops the count in view from 5 to 0. Cause: the GDD §6.1 rule "spawn only outside the camera cone, 60-120 m". The orchestrator amended GDD §6.1 (2026-09-23), the new rule is binding:

1. A sidewalk node is a valid spawn point when the player cannot see the spawn happen: either it is outside the camera cone (existing check) at 30-120 m, or it is inside the cone and the ray from the camera to the civilian's head height at that node is blocked by static city geometry (a building), at 30-120 m.
2. Spawning prefers nodes ahead of the camera look direction, so the street the player walks into refills (e.g. score or sort candidate nodes by forward dot, or split the per-tick budget), without breaking `spawn_min_separation` or the cap.
3. Initial fill and despawn rules stay as they are. Tuning (ring, forward preference, head height for the occlusion ray) lives in `assets/npc/population.ron`; the occlusion ray uses the existing static-geometry collision layer. Occlusion rays are bounded per tick (the perception module already budgets rays — follow that pattern and state the budget).

Reference code: `crates/gta_sim/src/population/mod.rs` (spawn and visibility check), `crates/gta_sim/src/navigation/`, the camera view resource published from `src/camera/mod.rs`.

## Acceptance criteria
- [ ] Headless (production composition, seed-1 city): an occluded in-cone node is accepted and an unoccluded in-cone node is rejected — both nodes found from the real city, not hand-built; flip-RED by disabling the occlusion ray.
- [ ] Headless: no civilian ever spawns at an in-cone node with a clear camera ray, over a 60 s run with the camera turning (assert every spawn event).
- [ ] Headless street-life gate: player walks forward along a seed-1 street for 20 s with the camera looking ahead; the mean count of civilians inside the view cone within 60 m is >= 5 (baseline before the change measured and reported; must be clearly higher than baseline).
- [ ] Bench `civilian_bench` still green; occlusion rays per tick bounded and reported.
- [ ] Runtime QA: rerun `qa_street_life.py` style probe (release, seed 1): count in view within 60 m reported before/after, screenshots; `t8.py` still passes.
- [ ] `cargo clippy --workspace --all-targets -- -D warnings`, `cargo test -p gta_sim -p citygen`, `cargo test -p gta_like --bin gta_like` green; `cargo tree -p gta_sim -e normal -i bevy_render` empty.

## Dependencies
- blocked by TASK-009 — civilians must land first
