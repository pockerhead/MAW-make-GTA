# FIX_SUMMARY — TASK-032, fixer round 2 (narrow)

Inputs: the orchestrator note (items 1-5), `QA_REPORT.prev-1.md` (Bugs 1-2 and the minor Bug 4), the last
`OPEN_DECISIONS.md` entry, and `IMPL_REVIEW.md` (read; its items were handled in round 1). Evidence from this
round is in `scratch/fixer2/` (suite, clippy, client tests, flip script, t15) and `scratch/qa/r1/branch_fix2_*`.

**Headline for the orchestrator**
- The give-up rule is changed as ordered. At runtime R1 the branch is now at or below main at every spot on
  both measures (section 3). No `Abandoned` columns: the worst run had 1 abandoned car, against 8 in QA's C_1.
- t15 pressure 5/6 this round. Hijack and "at most 2 cars" passed in all six runs.
- **4 non-ignored city gates are RED** on the round-1 G3 clause "no AI car stands > 30 s in `Dynamic`".
  The failing rows are `traffic_causes::c_character_in_the_lane` and the three `traffic_junction_box`
  correctness rows. Give-up used to break these locks, and the new rule forbids exactly that. One attempt to
  reconcile them failed and was reverted (section 4). Under the stop rule I did not try a second patch.
  **Your decision is needed** (section 4 lists options).
- Headless R1 rows stay `#[ignore]`d (item 5): RED on seeds 1 and 7.

**Preflight: the claim that would break code if done verbatim.** The note says "Bailing cars drop their pass
claim (the same `clear_ai_state` helper)". `clear_ai_state` sets `lateral = 0`. A kinematic bailing car is
still moved by the kinematic law, so the next tick puts it back on its path line: a 3.25 m sideways jump
for a passer that is out beside its obstacle. I checked this with a probe: the new G8 row, with the fields
reset the way `clear_ai_state` resets them, reports "the bailing car moved 3.25 m sideways". I dropped the
claim where it is read instead (section 1). The rule's second clause also conflicts with G3 row (e) and
cause row b3 as they were written (section 1).

## 1. Fixed

| Item | What was done |
|---|---|
| **1. Give-up trigger** (QA Bugs 1-2) | `traffic/recover.rs`: `stood` counts only while the car is at rest **and** `off_lane` **and** not `led`. `off_lane`: the footprint reaches past half the lane pitch (`left_gap`, else the car's width) from the path line. For an aligned car that means lateral offset > the 0.425 m lane slack. `led`: a body or claim on the lane line within **2 × `idm.min_gap`** of the nose. A queued car rests at exactly the jam gap, and a strip of that exact length can miss its leader (TASK-016 rest-point lesson; strip C uses the same margin). `recover.give_up_seconds` is kept for the qualifying case. |
| **2. Bailing drops its claim** (QA Bug 4) | `occupancy/mod.rs`: claims are derived only for `Kinematic`/`Dynamic` cars, not for every `is_ai()` car. The bailing car keeps its `lateral`/`manoeuvre`, so it brakes where it is. Rejected: `clear_ai_state` on the Bailing transition (see the preflight). |
| Docs/data | `traffic.md` (the give-up paragraph and the bailing claim), plus the comment on `traffic.ron` `recover:`. No new tuning value. |

**Gates** (each flip-RED shown in section 5):
- New `traffic_causes::spot_c_queue_behind_a_box_car_is_never_given_up`.
  - Setup: seed 1, the car left in the box, the player 31 m away, as in R1. The queue on the south approach
    (lane 294, through (-4.5, -92)) is filled to 8 standing cars at the jam gap behind its tail at 25 s. A
    feeder at the lane start is out of the bubble and gets despawned. From then on, every queued car gets a
    dummy pressed at its right side for 20 s.
  - Why dummies: QA's C_1 column was switched by characters (13 of 17 switches had cause `Character`).
  - Result: longest queue 8, 8 pressed, 8 switched to `Dynamic`, **0 given up**, G1 clean.
  - This row reproduces QA's column under the old rule: 8 cars `Abandoned` at 35 s.
- New `traffic_recovery::e_off_path_car_with_nothing_ahead_gives_up`.
  - Setup: a car shoved 1.5 m and yawed 30 deg on the street's U connector, with no grant (so it holds) and
    nothing ahead.
  - Result: given up at 9.98 s (`give_up_seconds` 10), moved 0.00 m.
- `traffic_recovery::e_blocked_…` re-anchored and renamed `e_blocked_corridor_waits_in_the_queue_and_clear_corridor_recovers`.
  - The blocked car stands behind a held car at the jam gap, so under the rule it must wait: it stays
    `Dynamic`, with no recovery and no re-switch. The clear case still recovers in 1.47 s.
  - Before, this row asserted a give-up. I also tried "lead removed": the car then drives 38 m and is lost
    at the U-turn, which is a different path, so the give-up case moved to the new connector row above.
- `traffic_causes::b3_dummy_pressed_at_the_bumper` re-anchored.
  - The dummy stands at the bumper for `PRESSED_S` = 20 s and is then removed.
  - The row asserts the car is not given up while the dummy is there, and that the car is not `Dynamic` and
    standing after 60 s. The Dynamic bound still holds: worst 20.1 s. The car drives off after the dummy leaves.
  - Before, it went RED under the rule: an on-path car behind a standing body can neither recover (the
    dummy is within the switch skin) nor give up.
- New G8 `traffic_occupancy::a_bailing_passer_claims_nothing`.
  - Setup: a passer at lateral -3.25 is set to `Bailing` while moving.
  - Result: its claim is gone the next tick while it is still bailing, the sideways move is under 0.1 m,
    and G1 is clean.

## 2. Skipped / not met

- **4 city gates RED on the `Dynamic` ≤ 30 s clause** (section 4). Not weakened and not ignored: that is a
  scope decision.
- **Item 5, headless R1 rows**: `r1_car_left_in_the_box_seed_{1,7}` are RED and stay ignored.
  - Stands of 126-149 s within 45 m on both seeds.
  - They now also fail the Dynamic clause (149.4 s and 122.4 s): the car pressed against the left car in
    the box waits instead of being given up.
- Nothing else from the review was open for this round.

## 3. Runtime

The runtime runs used the release `--features dev` build, QA's `scratch/qa/run_r1.py` and `r1_batch.sh`
with BRP snap-to-spot, one game at a time, and no game left running.

**R1**: seed 1, the player at STAND (7.2, -50), 150 s. Each cell is the number of traffic cars within
45 m standing over 30 s, with the longest stand in brackets. Every left car was within 0.1 m of its target.
The main column is QA's numbers from `QA_REPORT.prev-1.md`.

| Spot | main (QA) | branch, this round | median cars main → branch | median longest stand main → branch |
|---|---|---|---|---|
| A (8.2,-82.5) | 1 (148.2) · 1 (148.9) | 3 (149.1) · 12 (149.0) · 2 (144.9) | 1 → 3 | 148.6 → 149.0 s |
| B (5.3,-84.5) | 0 (13.6) · 0 (14.1) · 0 (11.1) | 0 (15.5) · 0 (12.3) · 0 (12.9) | **0 → 0** | 13.6 → 12.9 s |
| C (0.7,-86.7) | 3 (132.1) · 5 (132.3) | 0 (7.4) · 0 (16.6) · 0 (9.9) | **4 → 0** | 132.2 → 9.9 s |
| control (leave=0) | 0 | 0 (14.2) | 0 → 0 | — |

Maximum `Abandoned` alive per run on the branch: A 0 / 1 / 0, B 0 / 0 / 0, C 1 / 1 / 0. In QA's round 1 the
branch had up to 8.

Reading:
- **B**: fixed. 0/0/0, the same as main; QA's branch runs were 11/0/1.
- **C**: better than main (0/0/0 against 3 and 5).
- **A: worse than main on the car count** (median 3 against 1), and about equal on the longest stand
  (149.0 against 148.6 s).
  - Main's A lock is 1 car on the east approach. The branch's A lock is the east approach queue (x 11-24,
    z -81), 2-3 cars, with 1 `Dynamic` car at the box entry.
  - A_2 had 12 cars: the east queue plus the north queue (x ≈ -0.5) and the south queue. It also had 1
    `Abandoned` car at (17.4, -91.4), off the lane, which the rule allows.
  - Both builds lock at A in every run. The branch lock is longer.
- The orchestrator's goal (branch median ≤ main median per spot, on cars over 30 s and on the worst stand)
  is **met at B and C, missed at A on the car count**.
- The first control run was invalid: the scripted drive left the car at (-13.7, -92.8), 14 m from the box.
  It is kept as `r1/invalid_branch_fix2_control_car_near_box` and was rerun (log `decision`).

**t15** (`scratch/fixer2/t15_s{1,2,3}_{a,b}`): pressure in **5/6** runs.

| Seed | run a | run b |
|---|---|---|
| 1 | car at 14.67 s | car at 14.67 s |
| 2 | escaped | on foot at 16.75 s |
| 3 | on foot at 9.42 s | on foot at 10.5 s |

All six runs passed: hijack, and at most 2 active cars. Pooled with the earlier 18 runs: 20/24.

## 4. Open: the G3 `Dynamic` ≤ 30 s clause against the binding rule

Failing rows (full suite, one run, `scratch/fixer2/suite.txt`):

| Row | Dynamic stand | What stands |
|---|---|---|
| `traffic_junction_box::seed_1_box_keeps_moving` | 95.5 s | Two `Dynamic` cars at the box entries, 6-7 m from the watched body: `Connector(719)` s 0 at (-4, -86), the spot C head, and lane 299's end at (0, -72). `stood` stays 0 because both are led. |
| `…::seed_7_box_keeps_moving` | 77.0 s | The same class (not probed car by car). |
| `…::seed_1_car_behind_the_body_leaves` | 95.5 s, extra car 100 s | The extra car right behind the body on its connector is switched by the body and waits behind it. Before, it "left" only by being given up, which made the second body in the box. |
| `traffic_causes::c_character_in_the_lane` | 53.5 s | A `Dynamic` grant holder in a box 60 m from the scene (node 141, `Connector(1296)`, s 2.4). It is the sole occupant with 4 waiters. Pinned walkers stand around it, 1.9-2.3 m away for 5-15 s: they block its recovery sweep and flicker in its `led` strip, so `stood` keeps resetting. |

All four were green in round 1 only because give-up turned these cars into `Abandoned` bodies. That is the
same mechanism as QA Bug 1, the second body in the box at B. The G4 rows' lock is the in-view R1 class; their
liveness twins are already ignored. The c-row lock is the pinned-walker class, owned by TASK-036.

- **Attempt (1st, reverted, log `dead_end`)**: let `Dynamic` cars recover on a connector. The lateral was
  scaled for the decay along the connector, and recovery was refused near its end. It changed none of the
  numbers: the walkers and the body keep `nobody_coming`/the corridor blocked.
- **Options:**
  - **A (recommended)**: move the `Dynamic` clause of the three G4 correctness rows into their ignored
    `_liveness` twins (same class: the in-view box lock, R1 open). In c, assert the clause for the scene lane
    only; the lock at node 141 goes to TASK-036 (pinned walkers).
  - **B**: characters do not count as leaders. This probably fixes c (not verified) but not G4, and it goes
    against "a car or body".
  - **C**: allow give-up again for cars on a connector. This brings back spot B's second box body, which is
    what QA measured as the regression.

## 5. Flips

All flips were run with `scratch/fixer2/flip.py`. It restores the file, checks the sha256 match and touches
the file and `lib.rs`.

| Sabotage (`recover.rs` / `occupancy/mod.rs`) | Result |
|---|---|
| `may_give_up = at_rest` (the old rule) | `spot_c…` RED (8 queue cars `Abandoned` at 35 s: QA's column); `b3` RED ("left Dynamic at 10.22 s (Abandoned) while a body stood in its way"); `e_blocked_corridor_waits…` RED ("a car standing behind a car in its lane was given up") |
| `may_give_up = false` | `e_off_path_car_with_nothing_ahead_gives_up` RED ("not given up after 10 s") |
| `!led` removed | `e_blocked_corridor_waits…` RED |
| `off_lane` removed | `c_character_in_the_lane` RED. c is RED with the real code too (section 4), so this flip proves nothing about `off_lane`. `off_lane` has no gate of its own that goes RED alone. |
| claims for every `is_ai()` car (Bailing included) | `a_bailing_passer_claims_nothing` RED ("tick 1: the bailing car still claims road") |
| probe: `clear_ai_state` fields reset on Bailing | `a_bailing_passer_claims_nothing` RED ("moved 3.25 m sideways"), which is why that prescription was rejected |

## 6. Test results

- `cargo test -j 2 -p gta_sim -p citygen --no-fail-fast` (once, `scratch/fixer2/suite.txt`): 74 binaries,
  **585 passed, 4 failed, 11 ignored**. The 4 failures are the rows in section 4.
  - The suite ran before the connector-recovery attempt. That attempt was reverted, so the tested code is
    the current code.
  - The only later edits are the `traffic.ron`/`traffic.md` comments (`config_traffic` re-run: 6 passed)
    and the removal of the temporary probe prints.
- Rows run separately:
  - `traffic_recovery`: 6/6 passed.
  - `traffic_occupancy`: 10/10 passed.
  - `traffic_causes`: 11 passed, 1 failed (c), 2 ignored.
- `cargo clippy --workspace --all-targets -j 2 -- -D warnings`: clean (`scratch/fixer2/clippy.txt`).
- `cargo test -j 2 -p gta_like --bin gta_like`: 82 passed.
- `python tools/qa/tree_check.py`: passed.
- File sizes: `recover.rs` 167 lines; the test files are 468-636 lines.
- `git status`: the source and test edits above, plus `traffic.ron`, `traffic.md`, `log.jsonl` (4 decision
  entries and 1 dead_end), `PCTX_PROPOSALS.md` (2 entries) and this file. `metrics.md` was already modified
  before this round (not mine). `scratch/` is gitignored.

children: 0 launched / 0 reported.
