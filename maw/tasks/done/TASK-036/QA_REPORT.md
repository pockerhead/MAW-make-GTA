# QA_REPORT — TASK-036 (final code c4edce8, tree at 353f3db)

QA: claude opus, medium. All my outputs are in `scratch/qa2/`. I read the fixer's and implementer's
scratch only to see what they had covered. I did not count any of their runs as verification. The one
exception is the `c_sweep_real` probe, which the orchestrator asked me to re-run under my own flip.

## Preflight and disconfirmation

- Read: scratch listing, TASK_FINAL, PLAN_FINAL, IMPL_SUMMARY, IMPL_REVIEW, FIX_SUMMARY, OPEN_DECISIONS,
  log.jsonl (one `dead_end`: planner #0, counter-flow refuted), PCTX_PROPOSALS, and the production diff
  `9e01c0c..c4edce8`.
- Counter-example I tested first: **the corner shift freezes walkers in a loop.** `off_corner` moves the
  target `keep_right` = 0.5 m, and `around_cars` treats a corner as reached within
  `corner - clearance` = 0.5 m of the *unshifted* corner. If those two numbers fought each other, a walker
  would circle the corner and never reach it.
  - **It did not hold.** A walker steering at `C + 0.5·perp(d̂)` still has a radial speed component of
    `|d|/sqrt(|d|²+0.25) > 0` towards C, so `|d|` keeps shrinking and passes 0.5 m in finite time.
  - Empirical check: A3 passes at 3.27 s, A2 shows no orbit stalls on either heading, and both runtime R1
    runs show 0 stalls.
- Log triage: the only `dead_end` (counter-flow refuted) is consistent with the evidence. The Linux head-on
  case the fix exposed is a separate class, and the corner shift covers it. See A3 and the Linux flip in
  FIX_SUMMARY; I did not re-run WSL (see Environment).

## 1. Environment

- Direct Windows host, working dir `D:/test-gta-like`, branch `refactor/occupancy-consumers`. No docker
  and no mocks.
- Headless: the `cargo test` suites. Runtime: `cargo build --release --features dev`, driven over BRP
  through `tools/qa/brp.py` (`--settings-id .qa`). One game at a time, screenshots taken one at a time.
  No game process was left running, and the owner's game was never touched.
- Linux/WSL: not re-run by me. CI on c4edce8 is 5/5 success (`gh run list`: clippy, sim gates, client
  gates, citygen gates, repo checks), and the sim gates job runs on Linux.
- Reproduce:
  - `cargo test -j 2 --no-fail-fast -p gta_sim -p citygen`
  - `cargo clippy --locked --workspace --all-targets -- -D warnings`
  - `cargo clippy --locked -p gta_sim -p citygen --all-targets -- -D warnings`
  - `cargo test -p gta_like --bin gta_like`
  - `python tools/qa/tree_check.py`
  - `python tools/qa/scenarios/t15.py --seed 1|2 --out <dir>`
  - `python scratch/qa2/run_r1_qa.py branch <out> 1 5.3,-84.5`, then `python scratch/qa2/stall8.py <out>`
  - `python scratch/qa2/siren_tourist.py <out> 60`

## 2. Test results

| Run | Result | File |
|---|---|---|
| Full suite, Windows (`-p gta_sim -p citygen`) | **621 passed, 0 failed, 10 ignored** (9m53s). The ignored rows include `r1_car_left_in_the_box_seed_7` with the TASK-039 class E reason, as OPEN_DECISIONS decides | `scratch/qa2/full_suite_win.txt` |
| Clippy, both CI invocations | clean (exit 0) | `scratch/qa2/clippy_ws.txt`, `clippy_sim.txt` |
| Client tests | 82 passed | console |
| `tree_check.py` | passed | console |
| `traffic_gridlock` seeds 1/2/7/42, benches under `MEAN_LIMIT` | green inside the full suite (they assert the 40 s bound and the limits) | `full_suite_win.txt` |
| Gates re-run after every restore (touched, sha256 verified: `scratch/qa2/sha_before.txt` OK) | walk_arrival 4/4, traffic_box_overhang 8/8, sirens unit row green. A2 heading 0 still shows 2 walkers at 16.25 s / 15.75 s (bound 20 s) | `scratch/qa2/restored_green.txt` |

### My flips (from the orchestrator note; each perturbs a different input than the fixer's flips did)

| Flip (perturbed input) | Expected | Observed | File |
|---|---|---|---|
| B: `BoxInputs.body` in `drive.rs` = `(half.x + margin/2, 0.15)`, i.e. the old centre-line band fed as the input (the implementer swapped the function instead) | B rows RED | **RED**: B1 G1 0.533 m, B3 G1 0.683 m, B2 Y granted at 0 s and neither car leaves. B4, B5 and C stay green | `scratch/qa2/flip_band.txt` |
| A: the reach of `reached`'s covered branch set to 0 (`car_blocks(position, position, car, 0.0)`) | A1/A2 RED | **RED**: A1 0 of 6 arrivals; A2 heading 90 stalls 95.25 s, heading 0 109.5 s; A3 green (not its input) | `scratch/qa2/flip_reach0.txt` |
| C: `sirens::yield_reach` × 0.5 | the C sweep finds contacts | **Partly.** The in-tree unit row `curb_yield_stands_within_its_reach` goes RED (v 3..16, e.g. v 6: travel 13.74 > reach 11.12). The App rows C1-C3 stay **GREEN**. The real-start C sweep (seeds 1/7 pair 0, 322 cells) stays **GREEN**: curb is chosen from room 12 m, with 0 contacts | `scratch/qa2/flip_half_reach_tree.txt`, `flip_half_reach_sweep.txt` |
| C, extra: `yield_reach` × 0.25 | calibrate the sweep | one contact cell (seed 1, room 10 m, delay 16, **0.403 m**), same cell as the fixer's guard-off flip | `scratch/qa2/flip_quarter_reach_sweep.txt` |

All sources were restored with `git checkout`, touched and hash-checked, and re-run green.

### Runtime (release + dev, seed-1/2 cities, `--settings-id .qa`)

| Scene | Result | Evidence |
|---|---|---|
| t15 seed 1 | **PASS.** 19 cars (mean 6.5 m/s, max 12.4 m/s); hijack ok; police pressure by car at 12.5 s (9.2 m), not escaped; dismount 2 cars + 4 crew; no log errors; frame cost with no vsync 3.17 ms worst average (30 Hz Fifo monitor DISPLAY9) | `scratch/qa2/t15_s1.log`, `t15_s1/*.png` (chase_12: a two-star chase on the avenue, police car behind the player) |
| t15 seed 2 | **PASS.** 17 cars; pressure by car at 6.3 s (14.8 m), foot units within 2-4 m at 7-9 s, not escaped; dismount ok; no log errors; 3.02 ms | `scratch/qa2/t15_s2.log`, `t15_s2/` |
| R1 spot B run 1 (car snapped 27 m onto (5.3, -84.2): the drive fell short) | 14 walkers passed within 8 m of the left car; **0 Wander/Flee stalls ≥ 10 s** (8 m metric, as in A2); 0 traffic stands > 20 s; no stuck cars | `scratch/qa2/r1_B1/`, `stall8.py` output |
| R1 spot B run 2 | The first attempt failed in the harness (`enter_car_failed`, no car left: `r1_B2_harness_fail/`, not a game finding). Retry: car at (5.4, -84.5), 9 walkers within 8 m, **0 stalls**, 0 stands > 20 s | `scratch/qa2/r1_B2/` |
| Tourist minute, sirens (spawn sidewalk facing the junction, heat held at 2★, 60 s) | 6 police cars, **12 cars yielded** (73 yield samples, curb offsets up to 3.25 m on lanes), **0 yields seen on a connector**, **0 kinematic body overlaps > 0.02 m** (OBB check over 62 close pairs, max depth -0.71 m; detector self-test ok), 0 stands > 30 s. The player was Wasted twice by the cops (expected at 2★) and respawned | `scratch/qa2/siren/siren_result.json`, `siren_rows.json`, `siren/shots/0003_0015s_auto.jpg` (queue pulled to the curb beside the player) |

## 3. Acceptance criteria

| Criterion (TASK_FINAL rescope) | Test performed | Result |
|---|---|---|
| A: walker pile-up at a car on a crosswalk reproduced, fixed, gated with a flip | walk_arrival A1/A2/A3 green; my reach-0 flip RED; runtime R1 spot B 2 runs with 0 stalls | PASS |
| B: `connector_rects` overhang G1 fixture, fixed with body shapes, side effects measured | B1-B5 green; my band-as-input flip RED (0.533/0.683 m G1, B2 mutual wait); R1 seed 7 moved to TASK-039 per OPEN_DECISIONS (ignored with reason) | PASS |
| C: residual lateral at connector entry: fix what reproduces, with a flip | C1-C3 green; guard unit row RED at half reach; tourist siren minute shows 0 yields in the box and 0 overlaps. The half-reach flip does not redden the App rows or the sweep (finding F1) | PASS (with F1) |
| No regressions: full suite Windows (+ Linux) | Windows 621/0/10 by me; Linux via CI sim gates 5/5 (and the fixer's WSL 621/0/10, not re-run) | PASS |
| `traffic_gridlock` seeds 1/2/7/42 within 40 s; benches under `MEAN_LIMIT` | green in the suite | PASS |
| Clippy, client tests, tree_check, CI 5/5 | all clean / green | PASS |
| traffic.md records the rescope | `docs/architecture/traffic.md:24, 56-59, 191, 221` | PASS |
| t15 pressure holds (orchestrator) | seeds 1 and 2: pressure at 12.5 s / 6.3 s, no escape | PASS |

## 4. Bugs and findings

No blocking bug. Findings:

- **F1 (low, gate strength): the guard's magnitude is gated only by the unit replay row.**
  - At half the reach, C1-C3 and the real-start sweep all stay green: C2 sits at 0.37 m and C3 at reach
    + 1 m, both far from the halved boundary, and the sweep found contacts only at a quarter of the reach,
    in one of 161 cells.
  - So the App sweep is a sparse detector, not a bound. At 6 m/s the guard is about 2x more conservative
    than any contact seen.
  - Cost: a curb yield is refused (the slack yield is used instead) within ~22-27 m of a stop line where
    it would probably be safe. That is a behaviour cost, not a safety one.
  - The replay row is derived through the real code path and does go RED, so the rule is gated. Logged as
    a `dead_end` in log.jsonl.
- **F2 (low, thin margin): A2 heading 0 on Windows.**
  - Two walkers are pressed for 16.25 s and 15.75 s, against a bound of 20 s. This is walker-walker
    counter-flow between the left car and a standing traffic car, the class the fixer named. It is
    deterministic on Windows (identical in my run and the fixer's).
  - A future trajectory change (TASK-038: one traffic rule moves every trajectory) could cross 20 s
    without a regression in this task's code. Watch it; do not widen the bound blindly.
- **F3 (info): runtime harness.** `run_r1` can fail to enter a car (one attempt) and can snap the car
  8-27 m. Both are the known TASK-037 harness caveats. The retried run placed the car exactly on spot B.

## 5. Verdict

**SHIP.** The build, clippy, full Windows suite and CI 5/5 are green. The orchestrator's flips A and B go
RED on my own perturbations. For flip C, the in-tree gate (the replay row) goes RED, but the sweep does
not at 0.5x (F1, which says the guard is conservative, not unsafe).

Runtime shows no new freeze and no pass-through class: 0 walker stalls at spot B in 2 runs, 12 siren
yields with 0 overlaps and none in the box, and t15 pressure held on seeds 1 and 2.

Owner eye (feel, not blocking): park a car across the spot B crosswalk (5.3, -84.5) and watch walkers
step round it. At a corner they should keep to the side away from the car. Stand near a junction at
2★ and watch traffic pull to the curb for sirens.

Services started: only the game processes of the runs above. All were shut down; no `gta_like` process
is left.

children: 0 launched / 0 reported
