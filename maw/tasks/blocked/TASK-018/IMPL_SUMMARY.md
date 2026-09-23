# Implementation summary

## Verdict

Implemented and verified.

## Changes

- `crates/gta_sim/src/character/ledge.rs` (28 added, 9 removed lines): moved the over-limit barrier ahead of the angle filter, corrected the barrier push for oblique approaches, kept the original launch height while the same high wall remains in reach, and checked capsule clearance at the pull-up destination.
- `crates/gta_sim/tests/ledge.rs` (58 added, 3 removed lines): added an angle table and low-ceiling headless correctness gates.
- `scratch/flip_red_ledge.py`: retained the gate sabotage probe and its output.

## Deviations from spec

None. B7's one-tick pull-up remains intact.

## Tests

- `cargo test --workspace --lib -j 4`: 1 passed, 0 failed.
- `cargo test -p gta_sim --test <config|jump|movement|terrain|ledge> -j 4`: 15 passed, 0 failed. This includes all 14 pre-existing `gta_sim` tests and the two new gates across the library and integration targets.
- `cargo clippy -p gta_sim --lib -j 4 -- -D warnings`: passed.
- `cargo clippy -p gta_sim --test ledge -j 4 -- -D warnings`: passed.
- `python tools/qa/scenarios/t1.py --out maw/tasks/in_progress/TASK-018/scratch/t1`: passed; movement, camera, screenshot, FPS, and shutdown checks passed.
- Flip-RED: restoring the angle gate fails the 62° case; removing the capsule intersection query fails the ceiling case. See `scratch/flip_red_ledge.txt`. Both protections were restored and the ledge suite passed afterward.

## Manual verification

In the game, jump toward a ledge above the configured limit from frontal and oblique angles; the capsule should stay outside the wall. Jump toward a reachable ledge beneath a low ceiling; the player should remain outside the ceiling instead of snapping into it. Judge the existing one-tick pull-up feel in the owner run.
