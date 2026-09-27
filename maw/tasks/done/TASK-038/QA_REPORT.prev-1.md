# QA_REPORT — TASK-038 (hotfix: Linux CI traffic gates)

QA: claude opus, medium. The code under test is 663ba66 on branch `bugfix/linux-traffic-gates`. Local HEAD
0b7d508 adds only task-dir artifacts (`git diff --stat 663ba66 HEAD`: 5 files, all under `maw/tasks/in_progress/TASK-038/`).
origin/bugfix/linux-traffic-gates = 663ba66.

## Disconfirmation

The counter-example I went looking for: decision A's scoping hides a real regression. With the mechanism fix
reverted, the original Linux CI failure of `a_left_car_seed_1` ("stood 71.0 s in Dynamic") no longer goes RED,
because the stand now counts as off the scene approach.

**It held.** See check 1 and finding F1.

## 1. Environment

- Linux: WSL Ubuntu-22.04, rustc 1.95.0 (the CI toolchain), CI env (`CARGO_INCREMENTAL=0`, debug=0), `-j 2`.
  The worktree is mirrored into `~/gta038` by the implementer's `scratch/wsl_sync.sh` / `scratch/linux_test.sh`.
  I used the scripts only as transport. I chose every command and every sabotage myself.
  Command: `wsl -d Ubuntu-22.04 -- bash <scratch>/linux_test.sh [cargo test args]` (run from PowerShell).
  Git Bash rewrites `/mnt/...` paths, so don't launch it from there.
- Windows: `python tools/qa/scenarios/t15.py --out <scratch>/qa/t15_seed1 --seed 1`. The driver builds
  `--features dev` debug and launches the game with `--settings-id .qa`. No owner game was running before or after
  (`Get-Process *gta*` was empty).
- GitHub CI: `gh run list/view --repo pockerhead/MAW-make-GTA`.
- Nothing is left running. There are no containers (Docker was not used). I launched and shut down one game process.

## 2. Test results

| Run | Result | Evidence |
|---|---|---|
| GitHub, branch head 663ba66: sim, citygen, client gates, clippy, repo checks | **5/5 success** (sim run 36286459995 on headSha 663ba66, 15m23s) | `gh run list` / `gh run view` |
| Linux, sweep clause OFF (`\|\| (false && sweeps_touch(..))`), `traffic_graph` + `traffic_go_around` + `traffic_causes` | graph gate **RED** (seed 1: 198 of 2340 pairs touch, e.g. (74, 75, 14, 0.361)); `dummy_street_seed_1` **RED** at traffic_go_around.rs:195 with G1 `((1943v2, 1919v1), 0.18426514, 9334, (true, true), ...)`, byte-identical to the original CI failure; `a_left_car_seed_1` **GREEN** (F1) | `scratch/qa/qa_linux_flip_nosweep.log`, `scratch/qa/qa_linux_flip_a_nocapture.log` |
| Linux, restored (sha256 `f17725d2…` equals the pre-flip value, file touched), same three binaries `--nocapture` | graph 2/2, go_around 10/10, causes 11 passed / 2 ignored (r1, TASK-037) | `scratch/qa/qa_linux_restored.log` |
| Linux, probe: (a) put back on the city-wide `clock.dynamic_violation()`, sweep ON | `a_left_car_seed_1`, `a_left_car_seed_7` **GREEN** | `scratch/qa/qa_linux_a_citywide_sweep_on.log` |
| Linux, same probe, sweep OFF | `a_left_car_seed_1` **RED**: `["an AI car stood 71.0 s in Dynamic (> 30 s)"]`, the original CI message | `scratch/qa/qa_linux_a_citywide_sweep_off.log` |
| Probe reverted | `git checkout` on both files plus touch; sha256 graph.rs `f17725d2…`, traffic_causes.rs `eb28bcd8…` (both equal the pre-probe values); `git status` shows no code changes | — |
| Runtime t15 seed 1 (BRP) | **PASS** | `scratch/qa/t15_seed1/summary.json` |

I did not rerun the full `cargo test -p gta_sim -p citygen` locally. The independent GitHub Linux run on the exact
commit is green, and the fixer's local logs (Windows and WSL, 590 passed) agree with it.

## 3. Orchestrator checks

**(1) Flip the swept-body clause off.**
- The graph gate goes RED and the G1 CI failure comes back exactly. Restored, all GREEN.
- The second CI failure (`a_left_car_seed_1`, 71.0 s `Dynamic`) does **not** come back. On Linux with the sweep off,
  the (a) run prints `(a): Dynamic stands > 30 s off the scene approaches ...: [(71.046875, (371.63, 1.16, 487.42)),
  (34.80, ...)]` and passes. Why: the fixer's option-A change made (a) use the scoped bound.
- **Partial: F1.**

**(2) Diff of traffic_causes.rs.**
- Only the `Dynamic` liveness scope changed, for (a), (c) and rb, plus the `TrafficLane` import, the docs and
  `LeftInBox.approaches`.
- (c): the new helper has the same band as the old `dynamic_stands_on_the_approach`: -15 m..stop along the lane,
  -1.7..3.25 m lateral, the same `dynamic_longer_than(30)`.
- rb: the band covers all lanes with `end_node == node`.
- Untouched: G1 oracle asserts (rb, rb-near, spot C), "left car still there", "stood behind the car", and the other
  rows' city-wide `dynamic_violation()` (b*, d, rb-near at lines 217, 399, 495).
- No correctness assert was weakened. **Holds.**

**(3) Runtime t15 seed 1.**
- Traffic: 19 cars, mean speed 6.46 m/s, max 12.4. Hijack OK. The chase reached pressure at 13.7 s (car, 8.8 m).
  Dismount: 2 cars and 4 crew. `log_errors []`, result PASS.
- `street.png`: cars parked and moving along the avenue, nothing piled up.
- `chase_12.png`: a moving car ahead of the player. A queue stands at the crosswalk of the next junction, with police
  and a pedestrian around. It is an ordinary stop, not a frozen box. The next samples' distances keep changing.
- Frame cost without vsync: 3.8 ms. The display is DISPLAY9 at 30 Hz with Fifo, so 30 FPS is the refresh, not the
  game cost.
- The stricter junction table did not visibly freeze traffic. **Holds**, with one caveat: t15 is one 25 s drive, not a
  junction-throughput measurement. The capacity cost (2340 → 1323 free pairs on seed 1) is documented for TASK-036.

## 4. Acceptance criteria

| Criterion | Test performed | Result |
|---|---|---|
| Both gates reproduced RED on Linux, command recorded | Implementer's record plus my flip: G1 byte-identical. (a) reproduces only with the city-wide bound (probe) | PASS (G1); (a) only with the probe |
| Root cause named file:line; fix in the mechanism, not a relaxed bound | G1: `graph.rs` conflict table (sweep), confirmed by the flip. (a) passes with the fix under the **unrelaxed** city-wide bound (probe), yet the shipped code relaxes it | **FAIL (F1)**: an unneeded relaxed bound on (a) |
| Both gates green on Linux and Windows; full sim/citygen suite green on both | Linux: my run of the 3 binaries plus CI 5/5. Windows: the fixer's log (590/0) | PASS |
| Determinism note (libm ULP, not iteration) as a PCTX proposal | `PCTX_PROPOSALS.md` exists (implementer). Not re-probed | PASS (not re-verified) |
| After merge, main CI 5/5 | Not merged yet. The branch is 5/5 on 663ba66 | PENDING (post-merge) |
| Existing tests pass | CI plus my Linux runs | PASS |

## 5. Bugs found

**F1 — medium (gate honesty, the "silent for a month" class) — `crates/gta_sim/tests/traffic_causes.rs:124-127` (`abandoned_car`).**

(a) was switched from the city-wide `clock.dynamic_violation()` to `dynamic_bound_on("(a)", &[scene lane], ..)`.
With the fix, that scoping does nothing: on Windows (fixer log) and Linux (mine), the (a) off-scene lists for seeds
1 and 7 are `[]`. What it does is hide the original Linux CI failure.

Repro:
1. Sabotage the sweep clause at graph.rs:314.
2. In WSL run `linux_test.sh -p gta_sim --test traffic_causes a_left_car_seed_1 -- --exact --nocapture`.

Result: the test is GREEN, and the 71.05 s `Dynamic` stand appears only as a printed line.

Expected: RED with "an AI car stood 71.0 s in Dynamic" (that is what the unscoped bound gives, see the probe).

The spec says "the fix is in the mechanism, not a relaxed bound". OPEN_DECISIONS named "(and (a))" before anyone
had measured that (a) no longer needs the scope.

Fix, verified on Linux both ways:
- in `abandoned_car`, replace the `approach` + `dynamic_bound_on("(a)", ..)` lines with
  `let mut failures: Vec<String> = clock.dynamic_violation().into_iter().collect();`
- drop "(a)" from the module header and from the `dynamic_bound_on` mention;
- drop (a) from the TASK-037 "restore city-wide" item.

rb and (c) keep their scoping: rb really needs it (98.8 s holder), and (c) is unchanged.

No other defects found. Not re-litigated: I2 (`connector_rects` overhang) and the build cost, which are recorded for
TASK-036.

## 6. Verdict

**NEEDS_FIXES**, for one test-only change (F1). The mechanism fix is sound:
- the graph gate and G1 flip RED/GREEN on Linux;
- CI is 5/5 on the branch;
- the runtime t15 shows no frozen traffic;
- traffic_causes weakens no correctness assert.

But check (1) does not fully hold. One of the two original CI failures is now invisible because of a bound
relaxation the fix does not need. Once (a) is back on the city-wide bound (GREEN with the fix, RED without it, both
shown on Linux), I would SHIP. The orchestrator may choose to accept F1 under decision A as is. If so, record that
the (a) row no longer guards the 71 s Linux case.

## Paths and cleanup

- Evidence: `maw/tasks/in_progress/TASK-038/scratch/qa/` (`qa_linux_*.log`, `t15_seed1.log`, `t15_seed1/*.png`, `summary.json`).
- Log: +1 `decision` entry (stage qa).
- `git status`: only `metrics.md` (orchestrator, it was there before) and this report. No code changes remain.
- No game process, no containers.

children: 0 launched / 0 reported.
