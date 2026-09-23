# TASK-023: Игрок застревает на перекрёстке seed 1

Type: bug
Mode: small-fix
Priority: high
Branch: fix/crossing-stall
Domains: bevy-ecs, gates

## Description
Found by QA TASK-022 (finding 4.5, `maw/tasks/done/TASK-022/QA_REPORT.md`, logs `maw/tasks/done/TASK-022/scratch/qa/runtime_nociv/log.json`): in the release client, seed 1, holding W from the spawn with camera yaw pi-0.035, the player stops near (5.6, 1.1, 5) after about 42 m. It happens with 0 civilians too. A headless run along the same street covers 270 m; the runtime path drifts (x 7.2 -> 5.6) into some obstacle at the crossing — curb, prop collider, lamp post, or a chunk seam. A player who cannot run through a crossing is a first-frame defect.

Find the obstacle (BRP: player transform over time, colliders near the point; spatial query of what the Tnua sensor/capsule touches) and fix the root cause in the city collision or the prop placement, not in the controller. If the cause is a legitimate obstacle placed on the walking line (e.g. a prop in a crosswalk), fix placement in citygen/visuals rules so no prop collider sits on a crosswalk or curb ramp.

## Acceptance criteria
- [ ] Root cause named with evidence (entity, collider, position) in IMPL_SUMMARY.md.
- [ ] Headless gate on the real seed-1 city reproducing the runtime path (same start, same direction including the drift) that fails before the fix and passes after; flip-RED.
- [ ] If the cause is a placement rule: a headless gate over the whole seed-1 city (and 2 more seeds) that no prop/static collider overlaps a crosswalk or curb-ramp walking corridor.
- [ ] Runtime: the QA probe from TASK-022 (hold W from spawn, yaw pi-0.035) runs past the crossing; `t1.py` and `t8.py` still pass.
- [ ] `cargo clippy --workspace --all-targets -- -D warnings`, `cargo test -p gta_sim -p citygen`, `cargo test -p gta_like --bin gta_like` green; `citygen` golden hashes changed only if placement changed (explain).

## Dependencies
- blocked by TASK-022
- QA TASK-022 round 2 also saw the player blocked by a lamp post at (6.8, 1.2, -19.6), seed 1 — likely the same class (street furniture on the walking line); check both points.
