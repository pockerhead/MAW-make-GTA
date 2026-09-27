# FIX_SUMMARY — TASK-036 (fixer round 1)

**Verdict: FIXED.** The suite is green on Windows and Linux (WSL), all three orchestrator items are
built, and the new or re-anchored rows go RED under their flips. Commit `c4edce8` on
`refactor/occupancy-consumers` is pushed, and CI is 5/5 success on it (section 5).

## Preflight

- `scratch/` read as a coverage map only (stage0-4, cr, wsl, probe crate `probe036`). IMPL_REVIEW.md read
  from disk. Binding inputs: the last OPEN_DECISIONS.md entry, the orchestrator note (items 1-5), and the
  implementer's traces.
- The claim I checked before any fix, picked as the one most likely to break correct code if applied
  verbatim, is I-1: "a shift to the right of travel moves the corner towards the car, 0.8 - 0.5 = 0.3 m".
  - **It is real.** In the Linux stall (`scratch/wsl/probe_spot_b_heading_90.log`), walker 1995 goes east
    along the car's south side to corner (8.14, -86.5). Right of +x travel is +z, and +z points at the car
    (its edge is at z -85.7).
  - **A narrow reading would not fix it.** If "outward only" meant "shift only when the right side points
    outward", 1995 would get no shift, and the head-on with 2022 would stay (2022 heads for a lane target,
    which is never shifted).
  - **What was built.** The reviewer's wording ("the sign of the corner's perpendicular") means: shift by
    `keep_right` across the travel, and pick the sign that points away from the car. That is the rule in
    the code. The WSL flip below shows it is the cause.

## 1. Fixed

| Review / orchestrator item | What was done |
|---|---|
| Orch. 1 (R1 seed 7 red, class E) | `traffic_causes::r1_car_left_in_the_box_seed_7` gets `#[ignore = "TASK-039 class E: ..."]`, and the header says seeds 1 and 7 are ignored. `maw/tasks/pending/TASK-039/task.md` gets an "Added by TASK-036 fixer" note: un-ignore the row there, with the evidence paths. |
| Orch. 2 + I-1 (Linux A2 heading 90, corner keep-right) | `civilian/mod.rs`: new `off_corner` + `car_gap`, used only at the civilian caller when `around_cars` returns a corner (`target != to`). The target moves `keep_right` across the walker's travel, to whichever side is farther from every standing car. `around_cars` and police are untouched. Unit rows `off_corner_rows` cover the car on the walker's right (shift left) and on its left (shift right). New floor row `walk_arrival::a3_head_on_walkers_pass_at_a_car_corner`, described below. |
| Orch. 3 + I-2 (C-G1, lane-end guard) | `sirens.rs`: `yield_reach(offset, v, cfg) = offset·top/lateral.rate(top) + yield_gap(pass.speed)` with `top = max(v, pass.speed)`, the reviewer's tight form. The curb branch needs `room = lane.stop - (car.s + half.z) >= yield_reach(pitch, v)`, otherwise the existing slack branch runs. The guard applies at the start only; there is no "keep" rule. |
| Guard unit row | `sirens::tests::curb_yield_stands_within_its_reach` replays `idm_acceleration` / `ballistic_step` / `step_lateral` in `drive.rs` tick order, for v 0..16 m/s in 0.5 m/s steps at offset 3.25. It asserts replay <= reach. Tightest margin: 8.51 m at 6 m/s. |
| I-3 (C rows and sweep bypassed the start) | `traffic_box_overhang.rs` has a new helper, `siren_start`: a real responding police car (production component set, empty crew) at rest 5.08 m behind the car. The yield starts through `sirens::update`, then the siren car is removed, and the helper returns the chosen offset. C1 (+ floor, slack) and C2 (seed 1 lane 437) now need, as a precondition, that the car was on connector c in `Yield` for at least one tick. C2 now asserts that the curb is refused inside the reach (slack chosen, 0.37 m before the stop line), which is the "refused inside the guard" row the reviewer asked for. New C3: 1 m outside the reach the curb is taken, and the granted car stands at its offset before the stop line (9.59 m before it). |
| I-3 (C sweep) | New probe `scratch/probe/ws/probe/tests/c_sweep_real.rs`, which goes through the real siren start. It covers seeds 1 and 7 with the worst 3 curb pairs each. Room is the old HEAD red region 6-12 m plus need-5..need+10 (17.25-32.25 m), in 1 m steps, with B delays of 0-96 ticks. That is 966 cells, each recording the offset chosen and the 20 s G1 oracle. **Result: 0.000 in every cell.** Slack is chosen below 22.25 m and curb from 22.25 m up; no cell is n/a (`scratch/fix/c_sweep_real.txt`). |
| I-4 (B4 precondition) | `GATE BROKEN` unless W is a waiter for c5 at node 0 on every tick while H holds its grant. The check stops once H has lost the grant, so the flip still goes RED rather than tripping the precondition. |
| I-5 (`from_s` behind the car) | `junction.rs`: the holder path check skips holders missing from the index and holders already on `Lane(to_lane)`. There was no behaviour change: `stuck` was only read on the source lane or the connector, and an index miss is dropped by `retain`. |
| N-5, N-6 (traffic.md) | The three "Open (TASK-036, for the orchestrator)" paragraphs are rewritten to the decided state: the corner shift and the remaining 2-car squeeze, R1 seed 7 ignored under TASK-039, and the guard with its numbers. The box check now says "guaranteed by the oracle gates, not the step". |
| PCTX | Three entries appended to `PCTX_PROPOSALS.md`: manoeuvre gates must go through the real start rule; parked cars sit on avenue curb lanes; corner keep-right points away from the car. |

### A3 (new floor row)
- **Setup.** The car stands at (24.5, 0, 0) facing +X. Walker A (edge (18,0)->(32,0)) starts at (21.9, -2.0),
  past the south-west corner, and goes round the car to the SE corner (27.34, -2.0). Walker B (edge
  (32,-1.5)->(18,-1.5), lane target (18,-2.0)) starts at (26.3, -2.0), heading west.
- **Preconditions (`GATE BROKEN`):**
  - The corner, B's target and both starts lie on one line.
  - A's way to its target crosses the car.
  - After one tick A heads within 10° of +x and B within 1° of -x.
  - The car never moves.
- **Assertion.** The two walkers pass each other (each gets beyond the other's start) within 2 × gap /
  `walk_speed` = 4.89 s.

## 2. Flips (each perturbed input named; all restored, then GREEN)

| Row | Flip | Result | File |
|---|---|---|---|
| `sirens::tests::curb_yield_stands_within_its_reach` | `yield_reach` without the `yield_gap(pass.speed)` term | RED: v 3.5..16 over, e.g. v 6 travel 13.74 > reach 11.47 (margin -2.27) | `scratch/fix/guard_unit_row.txt` |
| A3 (Windows) | `off_corner(.., 0.0 * nav.keep_right)` (shift 0) | RED: never passed, A x 24.69 and B x 25.29 face to face (0.6 m) | this session |
| A3 + A2 heading 90 (Linux, WSL) | same | RED: A3 not passed; **A2 heading 90 reproduces the original Linux stall exactly (1995v0 and 2022v0, 89.25 s)**; restored, A2 has 0 stalls on both headings | `scratch/wsl/walk_arrival_flip_no_shift.log`, `scratch/wsl/walk_arrival.log` |
| C1, C2 | `sirens::update` plain `return None` off a lane | RED: C1 stands on `Connector(1)` s 0.65, C2 on `Connector(1091)` s 1.40 | `scratch/fix/overhang_flip_c_return_none.txt` |
| C2 | no reach check (`\|\| true`) | RED: curb 3.25 taken 0.37 m before the stop line | `scratch/fix/overhang_flip_no_guard.txt` |
| C3 | reach check always refuses (`room >= INF`) | RED: slack 0.425 taken 23.25 m out | `scratch/fix/overhang_flip_guard_always.txt` |
| B4 (with the new precondition) | holders' `from = 0.0 * from_s(..)` | RED: H demoted at 5.0 s (not GATE BROKEN) | `scratch/fix/overhang_flip_b4.txt` |
| C sweep probe | guard off (`\|\| true`), seed 1 pair 0 + seed 7 pair 0 | RED: G1 0.403 m (seed 1, 10 m, delay 16) and 0.249 m (seed 7, 6 m, delay 16) through the real start | `scratch/fix/c_sweep_real_flip.txt` |

## 3. Traces and observations

- **C3 first ran green for the wrong reason.** A seed-1 parking-spot car stands in the curb lane 11 m ahead,
  and the car stopped for it after 4.8 m (sensing). The curb-free check only looks beside the car. The fixed
  C3 removes parked cars on its run (a named mutation), puts the player past the box, and asserts `GATE
  BROKEN` if the car stands before its shift is done. The replayed travel now matches: 13.66 m against 13.74 m
  in the replay.
- **A2 heading 0 on Windows after the shift: 2 walkers pressed for 16.25 s and 15.75 s** (bound 20 s; 0 before
  the shift, 0 on Linux).
  - Trace: probe `scratch/probe/ws/probe/tests/walkers_fix.rs`, which copies the shift into the diagnostic.
  - What happens: the left car and a traffic car standing at the junction ((10.51, -81.18), standing 61-67 s)
    leave a ~2 m corridor. Walker 2034 is pinned at that car's corner (gap 0) heading east. Walker 2022 is
    right below it, heading north through it.
  - This is walker-walker counter-flow squeezed between two standing cars, and it resolves by itself at
    ~74 s while the second car still stands. Walkers have no walker-walker avoidance; that is a separate
    class, recorded in traffic.md and PCTX, not patched. The row stays under its bound (one printed Windows
    run; the suite run is green too, so it did not cross 20 s there either).
- `traffic_gridlock`: seed 1 is now 31.4 s (it was 31.0 s; civilian trajectories moved), seed 2 25.9 s,
  seed 7 17.6 s, seed 42 23.5 s. All are within 40 s, with G1 0.000 on every seed.

## 4. Skipped

- **I-6 (Flee ping-pong across a two-node covered edge).** Optional per the orchestrator. It was not
  reproduced; it needs a car on the street sidewalk lined up with a 4.5 m alley crossing. Left recorded in
  IMPL_REVIEW.
- **N-1 (parameter structs for `arrive`/`repick`).** Style only, and it would touch code outside the fix.
- **N-2 (skip `standing_cars` for non-moving states).** No measured cost: the civilian bench is 1.656 ms
  against a limit of 8.
- **N-3 (`connector_body` double `pose` call).** No measured cost: the traffic bench is 1.835 ms against a
  limit of 19.
- **N-4 (A1 stand window nearly vacuous).** The arrival assertion carries A1. I left the row as it is.
- **Missing coverage "ungranted body standing > 3 s".** Not requested by the orchestrator. That long-stand
  path is now covered only by the ignored R1 seed 7 (TASK-039).
- **The one tick of yield braking on connector entry.** Named by the plan; not patched.

## 5. Test results

- **Windows, `cargo test -j 2 --no-fail-fast -p gta_sim -p citygen`:** 621 passed, 0 failed, 10 ignored
  (`scratch/fix/full_suite_win.txt`).
- **Linux (WSL Ubuntu 22.04, toolchain 1.95.0, `~/gta036` via `scratch/wsl/wsl_sync.sh` + `linux_suite.sh`),
  same command:** 621 passed, 0 failed, 10 ignored (`scratch/wsl/full_suite.log`).
  - `walk_arrival` on Linux: A1 arrivals 1.8-5.2 s; A2 heading 0 and heading 90 have 0 stalls; A3 passed at
    3.27 s.
- **`walk_arrival` on Windows:** A1 arrivals 1.8-5.2 s (bound 11.71 s); A2 heading 90 has 0 stalls, heading 0
  has 16.25 s and 15.75 s (bound 20 s); A3 passed at 3.27 s (bound 4.89 s).
- **`traffic_box_overhang` (Windows):** 8/8 (`scratch/fix/overhang_green1.txt`).
  - B1-B3: G1 0.000; X leaves at 3.0-3.2 s, Y at 7.9-8.8 s.
  - B4: H keeps its grant; G1 0.000.
  - C1: leaves at 2.58 s. C2: leaves at 3.22 s. C3: stands 9.59 m before the stop line.
- **Clippy, exactly as in `clippy.yml`:** `--workspace --all-targets -- -D warnings` clean, and
  `-p gta_sim -p citygen --all-targets -- -D warnings` clean (`scratch/fix/clippy_ws.txt`, `clippy_sim.txt`).
- **Client tests,** `cargo test -p gta_like --bin gta_like`: 82 passed.
- **`python tools/qa/tree_check.py`:** passed.
- **Bench means (Windows, `scratch/fix/benches.txt`):** traffic 1.835 ms (limit 19), police 1.747 ms
  (limit 11), civilian 1.656 ms (limit 8).
- **`traffic_gridlock` seeds 1/2/7/42:** 31.4 / 25.9 / 17.6 / 23.5 s against the 40 s bound
  (`scratch/fix/gridlock.txt`).
- **CI on `c4edce8` (push to `refactor/occupancy-consumers`): 5/5 success.**
  - clippy https://github.com/pockerhead/MAW-make-GTA/actions/runs/36335560213
  - sim gates https://github.com/pockerhead/MAW-make-GTA/actions/runs/36335560132 (16m44s)
  - client gates https://github.com/pockerhead/MAW-make-GTA/actions/runs/36335560126
  - citygen gates https://github.com/pockerhead/MAW-make-GTA/actions/runs/36335560184
  - repo checks https://github.com/pockerhead/MAW-make-GTA/actions/runs/36335560124

## 6. Paths touched (`git status` accounted)

- **Production:** `crates/gta_sim/src/civilian/mod.rs`, `crates/gta_sim/src/traffic/sirens.rs`,
  `crates/gta_sim/src/traffic/junction.rs`.
- **Gates:** `crates/gta_sim/tests/walk_arrival.rs`, `traffic_box_overhang.rs`, `traffic_causes.rs`.
- **Docs:** `docs/architecture/traffic.md`, `maw/tasks/pending/TASK-039/task.md`.
- **Task dir:** `FIX_SUMMARY.md`, `PCTX_PROPOSALS.md` and `log.jsonl` (both appended).
  - `metrics.md` was already modified by the orchestrator before this round; I did not touch it.
- **Scratch (gitignored):** `scratch/fix/*`, `scratch/wsl/walk_arrival*.log`,
  `scratch/probe/ws/probe/tests/{c_sweep_real,walkers_fix}.rs`.
- No game process was launched.

children: 0 launched / 0 reported
