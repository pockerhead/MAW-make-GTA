# QA_REPORT — TASK-032 (QA, claude/opus, medium)

Branch `feature/oncoming-lane` @ d0b400b (implementer + continuation + fixer round 1). Pre-task baseline:
main 861bd90 (right before TASK-032, includes TASK-035), as the orchestrator note asked.

**Verdict: NEEDS_FIXES.** Spot B of R1 is worse on the branch than on main: 0 / 0 / 0 cars over 30 s on
main against 11 / 0 / 1 on the branch. The branch also turns bumped queue cars into `Abandoned` bodies (up to
8 in one run), and main never did that in any of the leave runs. t15 gave pressure in 4 of 6 runs, under the
5 of 6 bar. Every headless gate, clippy, the client tests, tree_check and the benches are green.

## 0. Preflight and disconfirmation

- Read: `scratch/` (as a coverage map only), TASK_FINAL, PLAN_FINAL, IMPL_SUMMARY, IMPL_REVIEW, FIX_SUMMARY,
  OPEN_DECISIONS, REDESIGN_NOTE, and the `dead_end` log entries. Every `dead_end` ref I relied on was checked
  against code or my own runs (R-A reverted: `stuck.rs` holds R-B only; the stop-rule rows are `#[ignore]`d in
  `traffic_causes.rs:424,430`, `traffic_junction_box.rs:296,302` and `police_close_in.rs:180,194`).
- **The counter-example I wrote first:** "at one of the R1 spots A/B/C, the branch leaves more traffic
  standing over 30 s than main 861bd90 does". The fixer's only pre-task number was one run at about spot B
  (0 cars, 9.2 s), so this could not be ruled out. **Result: it HOLDS at spot B** (section 3).

## 1. Environment

- Branch: the repo checkout (`cargo build --release -p gta_like --features dev`, default `target/`).
- Main: `git worktree add --detach scratch/qa/main_wt 861bd90`, with `assets/third_party/*` copied in (gitignored
  CC0 assets). `cargo build --release ... --target-dir scratch/qa/main_target` took 7 min 55 s. **The worktree and
  the 2.7 GB target dir were removed at the end** (`git worktree list` shows only the main checkout).
- Runtime: BRP through the `Game`/`Session` harness. Settings id `com.github.pockerhead.maw-make-gta.qa`. One game
  at a time, and each run's exit was checked. No game process is left, and the owner's game was never running.
  No docker or services were started.
- R1 wrapper: `scratch/qa/run_r1.py main|branch <out> <leave> [x,z]`. It runs the fixer's
  `scratch/tools/repro_abandoned_car.py` unchanged, with these additions:
  - Main builds: it points `brp.REPO` and `pt.REPO` at the worktree and sets `CARGO_TARGET_DIR`.
  - The car stops, and BRP snaps it onto the spot (heading kept). The snap is re-checked after 1.5 s.
  - Exit and teleport retry.

  The first main run showed why the snap is needed: the scripted drive left the car at (8.9, -74.9), 7 m off
  spot A and outside the box. Runs where the car was knocked off the spot after the snap are kept as
  `r1/invalid_*` and were rerun (log `decision` entries).
- Batch: `scratch/qa/r1_batch.sh` (sequential). Reproduce one run with
  `python maw/tasks/in_progress/TASK-032/scratch/qa/run_r1.py branch <out> 1 5.3,-84.5`.

## 2. Test results

| What | Result | Evidence |
|---|---|---|
| `cargo test -p gta_sim -p citygen --no-fail-fast` | 74 binaries, **586 passed, 0 failed, 11 ignored**. The ignored set: G6 ×2, R1 headless ×2, G4 liveness ×2, and pre-existing ones. | `scratch/qa/suite_gta_sim_citygen.txt` |
| Benches | `traffic_bench` mean 1.64 ms, `police_bench` mean 1.64 ms (limits 19 / 11 ms) | `scratch/qa/benches.txt` |
| `cargo clippy --workspace --all-targets -- -D warnings` | clean | `scratch/qa/clippy.txt` |
| `cargo test -p gta_like --bin gta_like` | 82 passed | `scratch/qa/client_tests.txt` |
| `python tools/qa/tree_check.py`; `cargo tree -p gta_sim -e normal -i bevy_render` | passed; empty | — |
| My flip of the hijack fix (see below) | RED, then GREEN after restore | sha256 below |

**Hijack-during-pass fix, checked independently.**
- `clear_ai_state` is called from both `abandon()` and `on_hijack` (`traffic/mod.rs:256`, `hijack.rs:117`).
- `snapshot_road` derives claims only for `is_ai()` cars (`occupancy/mod.rs:171`).
- My flip removed both defences at once. `a_hijacked_passer_claims_nothing` went RED on all 4 assertions: the
  taken car kept `Pass`, still claimed road, the oncoming car stopped at x 21.0, and the fields-only claim came
  back.
- Restore check: sha256 `ab28b9cc…cd9b` (hijack.rs) and `892c8a76…e8b4` (occupancy/mod.rs) match the pre-flip
  hashes. The row is green again: 9/9 in `traffic_occupancy`.
- The first GREEN re-run was a stale build (the restored files had an old mtime). Fixed with `touch`; this is
  recorded in PCTX_PROPOSALS.
- Residue, minor: a `Bailing` car is `is_ai()`, so a car shot mid-pass that has no free door keeps its claim.
  Before this task it already stood forever (`drive.rs` step 8). With this task it also blocks the oncoming
  lane. Not observed at runtime.
- Note: the fixture sets `go: false`, but after one tick the car reports `go: true`, so the gate hijacks a moving
  passer. The fix does not depend on `go`.

## 3. R1: main vs branch (the decisive measurement)

Setup: seed 1, player at STAND (7.2, -50), 31 m from junction (7.9, -81.3), 150 s. Each cell is the number of
traffic cars within 45 m standing over 30 s, with the longest stand in brackets. Per-run JSON is in
`scratch/qa/r1/<build>_<spot>_<n>/r1.json`.

| Spot (car left at) | main runs | branch runs | median main → branch (count) | median longest stand main → branch |
|---|---|---|---|---|
| A (8.2,-82.5) | 1 (148.2 s) · 1 (148.9 s, Dynamic) | 3 (149.1 s) · 0 (23.4 s) | 1 → 1.5 | 148.6 → 86.3 s |
| **B (5.3,-84.5)** | **0 (13.6) · 0 (14.1) · 0 (11.1)** | **11 (148.8) · 0 (10.4) · 1 (149.1, Abandoned)** | **0 → 1** (first two runs: 0 → 5.5) | **13.6 → 148.8 s** |
| C (0.7,-86.7) | 3 (132.1) · 5 (132.3) | 9 (149.2, 8 Abandoned) · 0 (16.6) | 4 → 4.5 | 132.2 → 82.9 s |
| control (leave=0) | 0 (4.1) | 0 (9.1) | 0 → 0 | — |

`Abandoned` traffic cars in the run (maximum at any time, from `TrafficStats`):

| | A | B | C |
|---|---|---|---|
| main | 0, 0 | 0, 0, 0 | 0, 0 |
| branch | 0, 1 | 1, 1, 1 | 8, 0 |

Controls: 1 on both builds.

Reading:
- **B is a regression.** Main never locks at B (3/3 runs, worst 14.1 s). The branch locked it once for the whole
  150 s (11 cars, 132-149 s) and left an `Abandoned` car standing 149 s in the box in another run.
  - Every branch B run gives up one traffic car next to the left car: (7.2, -81.2), (8.4, -81.8). That is two
    bodies in the box, the stage-5 class.
  - On main the car that touches it stays `Dynamic` and pushes through.
- **The give-up rule makes Abandoned columns.** In branch C_1, 8 cars in the south approach queue (x ≈ -4.6,
  z -90 … -120, up to 40 m from the box) became `Abandoned` one after another. The abandoned count rose
  0 → 8 over 131 s while switch events accumulated (`samples.json` `tstats`).
  - At most the first one (z -90.4, about 9 m from the junction centre) is within a car length of the box. The
    other 7 (z -96 … -120) are not. So the orchestrator's pre-decided narrow fix ("a bumped car is never given
    up within a car length of a box entry") would not cover this run.
  - Main in the same spot had 3-5 `Kinematic` cars queued about 132 s, and 0 abandoned.
- A and C have similar medians; each build has a stuck run and the branch is bimodal. Main's A lock is one
  `Dynamic`/`Kinematic` car on the east approach. Main's C lock is the queue at x ≈ -0.5 north of the box.
- Civilians within 8 m of the left car (reported, not asserted) are the same class on both builds: people pinned
  for about 140-149 s in most leave runs on BOTH main and branch. So this is pre-existing, TASK-036.
- By the orchestrator's rule (not worse at any spot, median of the runs) **the branch fails at B.**

## 4. t15 runtime, seeds 1-3, two runs each (branch)

| Seed | run a | run b |
|---|---|---|
| 1 | pressure at 20.97 s by car (9.6 m) | **escaped** |
| 2 | **no pressure, no escape** (the car got to 22.9 m at the end) | pressure at 7.31 s by car |
| 3 | pressure at 12.58 s on foot | pressure at 9.38 s on foot |

- Pressure in **4/6** (criterion ≥ 5/6). The implementer's runs gave 5/6 and the fixer's 6/6, so the metric has
  a large run-to-run spread. The pooled 18 runs give 15/18. A single 6-run sample does not hold 5/6 reliably.
- Hijack PASS in 6/6. At most 2 active police cars in 6/6.
- Files: `scratch/qa/t15_s{1,2,3}{,_b}/summary.json`.

## 5. Tourist minute (branch, seed 1): drive, bump, leave mid-lane, watch 90 s

Script: `scratch/qa/tourist_minute.py`.
- The first attempt chased traffic for 90 s by driving and never made contact (`invalid_tourist_s1_no_bump`).
- The final run puts the player's car 6.5 m behind a moving traffic car, at its heading and speed, then holds W
  (a named QA placement).
- Result:
  - Bumped car 12884895416 at 10 m/s on connector 733 → `Dynamic`. Still `Dynamic` and moving at 3.3 m/s at
    +10 s, then despawned by the bubble, so its recovery was not observed at runtime.
  - The player's car was left mid-lane at (-1.3, 28.4) (lane 303, s 57.8) and watched from the sidewalk 15 m
    ahead.
  - **No traffic car within 45 m stood over 30 s** (worst 18.8 s). Up to 7 stood briefly. Screenshots are in
    `scratch/qa/tourist_s1/shots/*tourist_watch*`: traffic flows past on the avenue.
  - The left car moved 2.9 m during the watch (pushed).
- Mid-lane go-around: no freeze seen. Headless G3(a) covers bumped-car recovery.

## 6. Acceptance criteria

| Criterion | Test performed | Result |
|---|---|---|
| G1 no kinematic pass-through | oracle in the suite rows (gridlock ×4, G2-G5, causes), all green | PASS (headless) |
| G2 go-around | `traffic_go_around` 10 rows green; tourist minute: no stand > 30 s mid-lane | PASS |
| G3 recovery (+ Dynamic ≤ 30 s in every city gate) | `traffic_recovery`, `dynamic_violation()` in the task's city gates, green | PASS (headless). At runtime a bumped car was not seen recovering (despawned). |
| G4 junction box | correctness rows green; the liveness rows are `#[ignore]`d (89-100 s in view) | **FAIL (liveness), accepted as open by the orchestrator** |
| G5 siren yield | `police_sirens` green | PASS |
| G6 police close in | rows ignored; rescoped to t15 (orchestrator option A) | see t15 |
| G7 spawn sectors | `police_spawn_sectors` green | PASS |
| G8 occupancy migration | `traffic_occupancy` 9 rows green; my flip of the hijack row went RED | PASS |
| G9 no regressions | suite 586/0/11, benches 1.64/1.64 ms, clippy, client 82, tree_check | PASS. The 5 CI workflows run after the merge (not run here). |
| R1 runtime | main vs branch, 3 spots, 2-3 runs each | **FAIL**: 3-5 cars over 30 s at A/C on main as well, and the branch is worse than main at B |
| t15 runtime ≥ 5/6 | 6 runs | **FAIL: 4/6** (hijack 6/6, ≤ 2 cars 6/6) |
| Data / GDD / traffic.md / TASK-036 | keys with strict loaders, GDD §5.2/§5.3 lines, `traffic.md`, `maw/tasks/pending/TASK-036/task.md` exist | PASS (presence checked) |
| Owner run | not gated | owner checklist below |

## 7. Bugs found

1. **MAJOR: R1 spot B regressed against main** (the orchestrator's decisive check).
   - Repro: `run_r1.py branch <out> 1 5.3,-84.5`, 3 runs.
   - Expected: no worse than main (0/0/0 cars over 30 s, worst 14.1 s).
   - Actual: 11/0/1 cars, worst 148.8 / 10.4 / 149.1 s.
   - Every branch run gives up a traffic car in the box next to the left car (`Abandoned` at about (7-8, -81.5)).
2. **MAJOR: the give-up rule builds columns of Abandoned cars in approach queues far from any box.**
   - Seen in branch C_1: 8 cars at x ≈ -4.6, z -90 … -120, standing 63-145 s, in view, so the stuck cheat does
     not fire.
   - `Dynamic` cars in a jammed queue (bumped by their neighbours) that do not recover within
     `recover.give_up_seconds` 10 s are abandoned. Each one is a new standing obstacle, and the car behind it is
     the next to be bumped.
   - Main in the same scene: 0 abandoned.
   - The narrow fix in OPEN_DECISIONS ("within a car length of a box entry") would not cover it. The fixer
     should address the give-up trigger itself, for example "no give-up while the car has a leader within the
     jam gap", or "only give up a car that is off its path".
3. **MAJOR (criterion): t15 pressure 4/6 < 5/6.** The spread between agents is 4/6 to 6/6, so the criterion is
   noisy. Seed 2 run a: no pressure within 25 s. Seed 1 run b: escape.
4. MINOR: a `Bailing` car keeps a pass claim (`is_ai()` includes Bailing). With no free door it stands forever,
   so a car shot mid-pass would block the oncoming lane. This is from code reading only.
5. Known open (the orchestrator accepted it; not counted against the verdict): G4 liveness and the in-view box
   lock (headless R1 rows ignored).

## 8. Owner checklist (not gated)

- Launch `cargo run --release --features fast`.
- Drive into traffic and bump a car: it should go back onto its lane smoothly (the in-place rotation).
- Leave a car mid-street: the queue shifts sideways at rest, then glides past it.
- Get 2★ and watch cars pull to the curb in front of the police car, and a police car in the opposite lane.
- Leave a car in a box and walk more than 40 m away: after 45 s it despawns while possibly still visible (R-B
  pop-out risk).

## 9. Housekeeping

- Log: two `decision` entries (snap-to-spot method; the third B run).
- PCTX_PROPOSALS: the mtime-after-restore lesson; the worktree/target-dir exception.
- `git status` shows only `log.jsonl` (M), `PCTX_PROPOSALS.md` (M) and this report. `scratch/` is gitignored. The worktree is removed,
  `main_target` is deleted, and no game process is running.

children: 0 launched / 0 reported.
