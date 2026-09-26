# QA_REPORT — TASK-032, QA round 2 (claude/opus, medium)

Branch `feature/oncoming-lane` @ 3b087c1 (fixer rounds 2 and 3 included). Main numbers for comparison are the
round-1 QA ones (`QA_REPORT.prev-1.md`, main 861bd90). Evidence is in `scratch/qa2/` and `scratch/qa/r1/branch_qa2_*`.

**Verdict: SHIP.** All five checks of the orchestrator note pass. One known limit (below, finding 1) showed up
in the tourist minute. It is not a new freeze class: main freezes the same way, and IMPL_SUMMARY already lists it
("a pass that reaches the lane end is refused"). I recommend adding it to TASK-037.

## 0. Preflight and disconfirmation

- Read: `scratch/` (as a coverage map only: fixer2, fixer3, qa), TASK_FINAL, PLAN_FINAL (headings and the
  give-up part), IMPL_SUMMARY, IMPL_REVIEW, FIX_SUMMARY (round 3), FIX_SUMMARY.prev-2 (round 2),
  QA_REPORT.prev-1, OPEN_DECISIONS, and the `dead_end` log entries. The refs I used were checked against the diffs
  of 947d243 and da0ecfe.
- **Counter-example written first:** "with the round-2 rule, a bumped car that is on its lane, has nothing ahead
  and has a standing body pressed inside its recovery skin (0.4 m) can neither recover (`nobody_coming` and
  `corridor_clear` fail) nor give up (`off_lane` is false), so it stays `Dynamic` and standing forever in view."
  The code path is real: `recover.rs:137-165`. I tested it at runtime (section 5, pressed run).
  **Result: it did not hold in that run.** The bumped car did not stand. It drove on as a `Dynamic` car under
  its autopilot, reached lane 304 at 6.9 m/s, and left the bubble. Headless, the same state is reached only in the
  TASK-037 box lock (`spot_c…` prints "worst dynamic 89.4 s": led cars in the queue behind the box car wait by
  design). For a car standing `Dynamic`, the go-around still applies (`pass.rs:35`: `Traffic` that is dynamic
  and standing is passable), so the queue is not held behind it.

## 1. Environment

- Direct, no docker or services. Default `target/`, one tree (`git worktree list` shows only the checkout).
- `cargo build -p gta_like --bin gta_like --features dev --release` for runtime. BRP through `tools/qa/brp.py`
  and TASK-031's `pt.Session`, always with the QA settings id. One game at a time. No game process is left
  (`tasklist` is empty), and the owner's game was never running.
- Reproduce:
  - `cargo test -p gta_sim -p citygen --no-fail-fast`
  - `printf 'branch qa2_B_1 1 5.3,-84.5\n' | sh maw/tasks/in_progress/TASK-032/scratch/qa/r1_batch.sh`
  - `python tools/qa/scenarios/t15.py --seed N --out <dir>`
  - `python maw/tasks/in_progress/TASK-032/scratch/qa2/tourist_pressed.py <out> 1 [press]`

## 2. Test results

| What | Result | Evidence |
|---|---|---|
| `cargo test -p gta_sim -p citygen --no-fail-fast` (after `touch` of both lib.rs) | 74 binaries, **589 passed, 0 failed, 12 ignored**, 8 min 33 s | `scratch/qa2/suite.txt` |
| Ignored set | pre-existing 5 (bless ×2, budgets ×2, `street_kill_under_old_rules_misses`), G6 ×2, R1 headless ×2, G4 `_liveness` ×3. Every TASK-032 ignore reason points to TASK-037 or REDESIGN_NOTE | same |
| `cargo clippy --workspace --all-targets -- -D warnings` | clean | `scratch/qa2/clippy.txt` |
| `python tools/qa/tree_check.py` | passed | — |

### Diff of rounds 2 and 3: was anything correctness-related weakened?

- Round 2 (947d243), production code:
  - `recover.rs`: the give-up counter runs only when the car is `at_rest && off_lane && !led`.
  - `occupancy/mod.rs`: claims come only from `Kinematic`/`Dynamic` cars, so `Bailing` cars claim nothing.
  - Nothing else changed in `src/`.
- Round 3 (da0ecfe) is tests only.
  - `traffic_junction_box::check`: the `Dynamic` 30 s bound is asserted only with `liveness`, as option A says.
  - **Deviation from "only the Dynamic bound moved":** the extra-car bound (`extra ≤ MAX_STOP`) also moved behind
    `liveness`, into the new ignored `seed_1_car_behind_the_body_leaves_liveness`. The fixer logged this as a
    `decision`. The bound is a stand (liveness) bound of the same lock, not a correctness bound.
  - The correctness rows keep the lease bound, G1 and `GATE BROKEN`. The fixer's D2 flip turns
    `seed_1_car_behind_the_body_leaves` RED on the lease ("stale holder 100 s"), so the row still has a job.
  - `traffic_causes::c`: the `Dynamic` bound was narrowed to the scene lane and its approach (−15 m..stop,
    lateral −1.7..3.25 m). Before, it covered the whole city. The "stood > 30 s behind the character" bound is
    unchanged.
  - `StandClock` now records where each car's longest `Dynamic` stand happened. It is test support only.
- Conclusion: no correctness assertion was dropped. What moved is liveness: the `Dynamic` bound plus the
  extra-car stand bound (G4), and the scope of the `Dynamic` bound in (c).

### Flip (item 2): give-up back to the old trigger

- Sabotage in `recover.rs:156`: `let may_give_up = at_rest;`. `off_lane`/`led` were still evaluated, so the
  build is warning-free.
- Result: `spot_c_queue_behind_a_box_car_is_never_given_up` **RED**: "cars of the queue behind the box car were
  given up: [8 cars Abandoned at 35.03 s, z −92 … −135]". That is QA round 1's column.
  Log: `scratch/qa2/flip_old_giveup.txt`.
- Restore: file copied back, sha256 `381ca3f7…d349` matches the pre-flip hash, both files `touch`ed, and
  `git status` is clean for `crates/`.
- Re-run: **GREEN** ("given up: []; worst dynamic 89.4 s; G1 max depth 0.000"). Log: `scratch/qa2/flip_restored.txt`.

## 3. R1 runtime, spots B and C, 2 runs each (branch)

Seed 1, player at STAND (7.2, −50), 150 s. Each cell is the number of traffic cars within 45 m standing over
30 s, with the worst stand in brackets. Main is QA round 1.

| Spot | main (round 1) | branch, this QA | median count main → branch | max Abandoned / Dynamic alive |
|---|---|---|---|---|
| B (5.3,−84.5) | 0 (13.6) · 0 (14.1) · 0 (11.1) | **0 (17.8) · 0 (17.8)** | 0 → 0 | 0/4 · 0/2 |
| C (0.7,−86.7) | 3 (132.1) · 5 (132.3) | **0 (9.2) · 0 (15.3)** | 4 → 0 | 1/3 · 0/2 |

- Every left car was snapped onto its spot once, and the 1.5 s re-check needed no second snap. At leave time
  each car was within 0.3 m of the target (its end position was not logged).
- B worst stand is 17.8 s against main's 13.6 s median. Both are under the 20 s control line and the 30 s bound,
  and the count is equal, so B is not worse by the note's rule.
- C is clearly better than main.
- Screenshot `scratch/qa/r1/branch_qa2_B_1/shots/0037_0177s_auto.jpg` (177 s): traffic moving on the avenue, and
  no column.
- Runs: `scratch/qa/r1/branch_qa2_{B,C}_{1,2}/r1.json`, summary `scratch/qa2/r1_batch.out`.

## 4. t15, seeds 1-3, one run each

| Seed | Pressure | Hijack / ≤ 2 active cars | Result |
|---|---|---|---|
| 1 | car at 12.5 s (9.1 m) | PASS / 2 | PASS |
| 2 | on foot at 24.08 s (17.0 m), just inside `CHASE_S` 25 s | PASS / 2 | PASS |
| 3 | car at 10.52 s (11.9 m) | PASS / 2 | PASS |

- Pressure in 3/3 runs. No freeze and no stuck police car was seen.
- Files: `scratch/qa2/t15_s{1,2,3}/summary.json`.

## 5. Tourist minute (seed 1)

**Plain run** (`scratch/qa2/tourist_s1`, the round-1 script with a 4 s W hold):
- Bumped car 12884895542 (lane 298, 10.8 m/s) was shoved about 50 m and was `Abandoned` on connector 731 by
  25.8 s. That is the G3(b) path (beyond `lost`).
- It stood 88.9 s as an `Abandoned` body. It is the only stand over 30 s.
- All live traffic within 45 m stood at most 14.9 s. No freeze.

**Pressed run, the counter-example** (`scratch/qa2/tourist_s1_pressed`):
- Setup: a 1.5 s W hold. After the bump, the player's car was placed by BRP 0.2 m behind the bumped car, then the
  player got out and watched from 15 m ahead for 90 s. Log `decision` entry.
- The bumped car (Dynamic from 16.5 s) did not stand: it drove on in `Dynamic` and left the bubble at about 51 s
  (last sample on lane 304 at 6.9 m/s). So the counter-example did not hold.
- **But the player's car then stood at the head of lane 302, at the stop line, in view:** 7 kinematic cars queued
  behind it for 64-89 s, until the run ended. Screenshot `shots/0016_0079s_tourist_watch.jpg` shows the column
  behind the left car at the crosswalk, with the player on the corner.
- Cause: `pass.rs:216-218` refuses a pass whose merge would reach the lane end ("would enter the junction off
  its line"). The stuck cheat acts only out of view.

## 6. Acceptance / note items

| Item | Test performed | Result |
|---|---|---|
| (1) suite + clippy; rounds 2-3 diff | full suite 589/0/12, clippy clean; diff read | PASS. Only liveness moved; the extra-car bound moved too (documented deviation) |
| (2) old give-up trigger flip | `may_give_up = at_rest` → spot_c RED (8 Abandoned), restore → GREEN, sha256 match | PASS |
| (3) R1 B and C ×2 vs main | B 0/0 (main 0), C 0/0 (main 4) | PASS |
| (4) t15 seeds 1-3 | 3/3 pressure, hijack and cap asserts PASS | PASS |
| (5) tourist minute | bump, leave, watch 90 s; plus the pressed variant | PASS, with no new freeze class. Known limit: finding 1 |
| G4 liveness / spot A in-view box lock | not re-run (owned by TASK-037, option A) | not a blocker |

## 7. Findings

1. **MINOR / known limit (recommend adding it to TASK-037): a car left at the lane end, in view, is never passed.**
   - Repro: `tourist_pressed.py <out> 1 press` (seed 1). The player's car ends at (−0.9, 76.4), the head of
     lane 302 at its stop line.
   - Expected: GTA fantasy (spec C): the queue goes around it.
   - Actual: 7 cars stand 64-89 s (to the end of the watch). The pass is refused because the merge would run into
     the junction (`pass.rs:216`), and the stuck cheat waits for out-of-view.
   - Not a regression: on main any left car freezes its lane (M1). IMPL_SUMMARY §1 stage 4 already lists it
     ("step 6 not built").
   - TASK-037 today names only a car inside the box. It should also name "a car left within a pass length of the
     stop line", because the same re-route/U-turn/box-pass remedy applies.
2. INFO: in `spot_c_queue_behind_a_box_car_is_never_given_up`, led queue cars stand `Dynamic` 89.4 s, which is
   printed, not asserted. This is the TASK-037 lock class behind the box car, consistent with option A.
3. INFO (process): round 3 also moved the G4 extra-car stand bound into `_liveness`. That was beyond the literal
   "only the Dynamic bound moved", but it is the same lock and was logged as a decision.

## 8. Owner checklist (not gated)

- `cargo run --release --features fast`, seed 1.
- Bump a traffic car at speed. It either drives on or, if shoved far, is left as a wreck that traffic goes around.
- Leave a car mid-block. The queue shifts out and passes it.
- Leave a car right at a junction's stop line and watch it. The lane behind stays jammed while you look
  (finding 1). Look away for more than 45 s and it clears.
- At 2★, watch cars pull to the curb for sirens.

## 9. Housekeeping

- Log: 1 `decision` (pressed placement). No PCTX proposal.
- `git status`: `metrics.md` (not mine), `log.jsonl` (my append), and this report. `scratch/` is gitignored.
- Services: none started. The game processes were closed by the harness each run, and `tasklist` shows no
  `gta_like`.

children: 0 launched / 0 reported.
