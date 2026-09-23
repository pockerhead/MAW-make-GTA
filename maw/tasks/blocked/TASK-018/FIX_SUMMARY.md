# TASK-018 fix summary — round 2

## Preflight disconfirmation

The review suggests removing the barrier's window refresh. Applied verbatim, this breaks the height limit: `scratch/qa/probe_norefresh.txt` records above-limit climbs after the window expires, including 0°, 62°, and 75° approaches. I checked `crates/gta_sim/src/character/ledge.rs`: the refresh is in the barrier branch. I preserved it and corrected the height anchor instead. I did not run the author's probe as independent verification.

## Fixed

- **Review issue 1 / QA B1 — stale launch anchor:** On each fixed tick, `assist_ledge` now measures the surface directly under the capsule when Tnua reports support. It accepts the height only when the downward hit is that support entity and is within `ledge_assist_clearance` of the configured feet distance. This updates the anchor while standing on the 0.8 m crate, including before a buffered jump starts, and freezes it in the air. Using `position.y - float_height` on every Tnua support report was tried and rejected: it made the existing high-wall gates fail because Tnua can report wall support while the body is still rising. The barrier window continues refreshing.
- **Review issues 3–4 / QA B2 — angle table:** The angle case now starts 4 m from the wall, jumps within 1.0 m of its face, runs 400 fixed ticks, and records any successful arrival on top. The other ledge fixtures keep their original start, jump timing, and duration. Restoring the old angle gate now shows climbs at **both 62° and 75°**.
- **Missing crate coverage:** Added separate headless gates for walking forward from the crate and for a jump tapped before landing. Both were RED on the pre-fix source and GREEN after the support-height fix. The existing after-landing re-jump gate remains green.

## Skipped

- **Review issue 2, tangential velocity:** Already fixed in the current source before this round: `apply_ledge` removes only the velocity component into the wall. No change needed. Its feel still needs the owner's runtime judgment.
- **Review issue 5, sensors in the free-space query:** Verified the query does not exclude sensors, but no current mission or pickup sensor fixture uses this path. Adding collision layers now would expand TASK-018 beyond its specified geometry fix; carry this concern into work that introduces those sensors.
- **Review nits, capsule helper / callback allocation / extraction:** No demonstrated current failure. The capsule dimensions match the body, and the intersection query runs only on snap attempts. Left these unchanged.
- **Review suggestion to remove window refresh:** Rejected because the QA `norefresh` probe shows it permits above-limit climbs. The support-height correction solves B1 while retaining the barrier.
- **T1 runtime scenario in this round:** The orchestrator explicitly prohibited building or running the windowed client in the Codex sandbox. QA's previous `scratch/qa/t1/summary.json` records a passing runtime run; this round did not repeat it.

## Test results

- `cargo test -p gta_sim -j 4` — **PASS**, 19 tests: lib 1, config 2, jump 3, ledge 7, movement 4, terrain 2; doc tests 0. All 14 pre-existing tests remain green.
- `cargo clippy -p gta_sim --all-targets -j 4 -- -D warnings` — **PASS**, no warnings.
- `python maw/tasks/in_progress/TASK-018/scratch/fixer_round2_flip.py` — independent temporary source perturbations, restored byte-for-byte after each run: old angle gate **RED** (`above-limit climbs at [62.0, 75.0]`); remove support re-anchor **RED** for walking and buffered jump; remove capsule intersection check **RED** (`capsule snapped into low ceiling`). The restored full suite is GREEN as above.
- `git -c safe.directory=D:/test-gta-like diff --check` — **PASS**. `git status --short` shows only `crates/gta_sim/src/character/ledge.rs`, `crates/gta_sim/tests/ledge.rs`, and this report outside ignored scratch. No windowed client was launched. Children: 0 launched / 0 reported.
