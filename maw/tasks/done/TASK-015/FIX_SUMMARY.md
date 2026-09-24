# FIX_SUMMARY — TASK-015, fixer round 3 (tiny)

Inputs: QA_REPORT.md B1/B2 and the binding last OPEN_DECISIONS.md entry. IMPL_REVIEW.md was acted on in rounds 1-2 (FIX_SUMMARY.prev-1/2.md).

Check before fixing (claim that could break correct code): QA's B1 recipe, "append `Engine` and a cap of 1 to the lists", would leave the same trap for the next class. The orchestrator note asked for the root cause instead. Checked: the Siren/Ambience/Engine caps are not in `mix.ron voices`, so a pure mix.ron derivation cannot cover them. Hence the split below.

## Fixed
- **B1, t13.py static class list.** `tools/qa/scenarios/t13.py`:
  - `CLASSES` is now parsed from the `SoundClass` enum in `src/audio/cues.rs`. That enum's order is the `SoundStats` index.
  - Caps come from these sources, by snake_case name:
    - the one-shot classes: the `mix.ron` `voices` tuple, parsed generically;
    - Siren: `siren.max_emitters`;
    - Ambience 2 (two beds) and Engine 1 (one car): a small `LOOP_CAPS` law table, because no data file holds them.
  - A class with no cap raises `GATE BROKEN` and names the class. `stats()` raises `GATE BROKEN` if the length of the reflected `SoundStats` differs from the parsed class list.
  - Result: after a future class is added, t13 fails with a message that names the class, not with an `IndexError`.
- **B2, exit through a thin fence.** `crates/gta_sim/src/vehicle/seat.rs` `exit_spot`:
  - A sphere of radius `capsule_radius` is cast from the seat to each candidate centre, against World|Vehicle, excluding the player's own car.
  - A hit invalidates that candidate.
  - `exit_spot` now takes `(Entity, Vec3, Quat)`. Both callers are updated (`enter_exit`, `eject_all`).
- **B2 gate.** `crates/gta_sim/tests/vehicle_seat.rs` `thin_fence_beside_the_door_is_not_walked_through`:
  - Built from QA's probe: fences 1.2 m high, 0.05 m thick at x −26.325 and 0.1 m thick at x −26.40, both beside the car at (−25, 0).
  - Expects: on foot, at the right door.
  - Flip-RED, with the path check disabled (`true || path_free`): fails with "fence at x -26.325: exited at [-1.70, -0.11, -0.30] from the car, not at the right door". Restored → GREEN.

## Skipped
- Nothing from B1/B2. O1/O2 and the forced eject onto a wall are carried into T15 by the orchestrator.

## Test results (all with -j 4)
- `cargo test -j 4 -p gta_sim -p citygen`: every binary `ok`, 0 failed (vehicle_seat 17/17).
- `cargo clippy -j 4 --all-targets -- -D warnings`: clean.
- `cargo test -j 4 -p gta_like --bin gta_like` ×3: 77 passed each time.
- `python tools/qa/scenarios/t14.py --out scratch/fixer3/t14`: exit 0, result PASS.
- `python tools/qa/scenarios/t13.py`: three runs.
  - Runs 1-2 (`scratch/fixer3/t13`, `t13b`): got past `check_peaks` (B1 gone). Both then stopped on the harness precondition "GATE BROKEN: six clicks did not fire six shots" (magazine 12→7, one OS click lost).
  - QA's own patched copy then passed (`scratch/fixer3/t13_qa_copy.txt`).
  - Run 3 of the fixed script (`scratch/fixer3/t13c.txt`): exit 0.
  - Reading: the lost click is flaky OS-input timing (a precondition, not a sound assertion). The class/cap logic cannot touch it. Worth watching if it recurs.
