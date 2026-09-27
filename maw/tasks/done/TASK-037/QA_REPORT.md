# QA_REPORT — TASK-037 (in-view junction box lock)

QA: claude opus, medium. Code under test: `feature/box-lock-in-view` at `3b24cca` (code commits `a66814a`
and `596399d`). `OPEN_DECISIONS.md` is binding: class E (G4 seed 7 liveness) and the class D residue (R1 seed 1)
belong to TASK-039.

**Verdict: SHIP** (by the orchestrator's rule: the R1 goal holds, the gates are green, and there is no new freeze
class). There are three findings for the orchestrator (section 4). None of them is a regression against main. The
owner run stays open (section 3, last row).

## 0. Disconfirmation

Counter-example written down before testing: **at runtime spot A the branch leaves 2 or more traffic cars standing
over 30 s, worse than main's 1.** The code path I looked for is the M2 fix. It makes plain cars stop for
`held_in_box` cars in both arms of `sense`, including the 25 m straight lane strip (`manoeuvre.rs`, `skip`). So a
lane car whose straight strip crosses the box could now brake for a held car that is not on its path.

**Result: the median does not hold, but single runs do.** Spot A over 5 runs gave 1, 3, 7, 0, 0 cars over 30 s, so
the median is 1 and equals main.
- The two bad runs are not the lane-strip mechanism.
- In both, the QA snap teleported the left car next to the east approach head while it waited at its stop line
  (x 11.4). A `Vehicle` switch fired on the snap tick (`switches_by_cause[1]` +1 at 63.5 s in A_2, +1 at 59.5 s in
  A_3). That head then stood `Dynamic` against the left car for the whole 150 s. This is class D, a body placed at
  a car's bumper, and TASK-039 owns it.
- In A_4 and A_5 the head was not touched by the snap. The east approach drained there: 0 cars over 30 s, worst
  stand 21 s.
- On main the east head stood 148 s in both runs.

## 1. Environment

- No docker-compose or dev server. The direct test runner on the working directory `D:/test-gta-like` (branch
  checkout, shared `target/`).
- Runtime: windowed release build with `--features dev` (BRP on 127.0.0.1:15702), started through
  `tools/qa/brp.py` / the TASK-031 `pt.Session` harness with `--settings-id com.github.pockerhead.maw-make-gta.qa`.
- One game at a time. Every run exited, and `tasklist` shows no `gta_like` left running. The owner's game was never
  running. No containers or services were started, so there is nothing to clean up.
- No second checkout or target directory was used. Main numbers are the ones from TASK-032 QA, as the orchestrator
  note gives them.
- Reproduce:
  - Suite: `cargo test -p gta_sim -p citygen --no-fail-fast -j 4`.
  - R1: `printf "A_1 1 8.2,-82.5\n..." | sh maw/tasks/in_progress/TASK-037/scratch/qa/r1_batch.sh`, or one run:
    `python maw/tasks/in_progress/TASK-037/scratch/qa/run_r1.py branch <out> 1 8.2,-82.5`. This is the TASK-032
    wrapper with only its `TASK` path moved to `done/`.
  - Tourist: `ISOLATE=2 BUMP_DV=3 python .../scratch/qa/tourist_box.py <out> 1`.
  - t15: `python tools/qa/scenarios/t15.py --out <dir> --seed 1`.
  - Walker probe: in `scratch/probe/ws/probe`, run
    `CARGO_TARGET_DIR=D:/test-gta-like/target cargo test --offline --test qa_walkers -- --nocapture --test-threads 1`.

## 2. Test results

| What | Result | Evidence |
|---|---|---|
| `cargo test -p gta_sim -p citygen --no-fail-fast` (Windows) | 74 binaries, **607 passed, 0 failed, 9 ignored**. The ignored rows are 7 pre-existing plus `r1_car_left_in_the_box_seed_1` and `seed_7_box_keeps_moving_liveness`, both for TASK-039. The fixer's 605+11 had the two un-ignored rows still ignored, so the set is the same. `traffic_gridlock` 5/5, `traffic_bench` / `police_bench` / `civilian_bench` green. | `scratch/qa/full_suite.txt` (8 min) |
| `cargo clippy -j 4 --workspace --all-targets -- -D warnings` | clean | `scratch/qa/clippy.txt` |
| `cargo test -p gta_like --bin gta_like` | 82 passed | `scratch/qa/client_tests.txt` |
| `python tools/qa/tree_check.py`; `cargo tree -p gta_sim -e normal -i bevy_render` | passed; empty | — |
| My flip H1: heading cap doubled (`clamp(-2cap, 2cap)`; the fixer removed it) | `a_pass_keeps_the_body_in_its_band` **RED** (away side 1.759 m > 1.625 m). `avenue_seed_2` stays green at 2x. | `scratch/qa/flip_H1_cap_x2.txt` |
| My flip H2: cap removed, run on `avenue_seed_2` only (the fixer flipped only the floor row) | **RED**: `passing cars touched vehicles: [(8315, 1576v1, 1828v0)]`. The Windows regression is held by the cap. | `scratch/qa/flip_H2_no_cap_avenue2.txt` |
| My flip M1: `around_cars` with the straight-line corner metric (`if sees(p) \|\| true`; the fixer edited `via`) | `around_cars_near_diagonal_target_arrives` **RED**. `r1_car_left_in_the_box_seed_7` **RED**: 3 cars stood 100-106 s. | `scratch/qa/flip_M1_*.txt` |
| Restore | `git checkout` plus `touch`, then sha256 check against `scratch/qa/sha_before.txt` (drive.rs `027deaf3…71bb`, fire_line.rs `7442d44a…71b`): OK. All flipped rows are **GREEN** again (R1 seed 7 worst 23.6 s, `Dynamic` 6.3 s, G1 0.000). `git status` shows no change under `crates/`. | `scratch/qa/flips_restored_green.txt` |

### Runtime R1 (seed 1, player at (7.2, −50), 150 s; cars within 45 m standing > 30 s, worst stand in brackets)

| Spot | pre-TASK-032 main | TASK-032 final | **TASK-037 branch (this QA)** | median |
|---|---|---|---|---|
| A (8.2, −82.5) | 1 (148) · 1 (149) | 3 (149) | 1 (148.8, `Abandoned`) · 3 (148.8, 1 `Dynamic`) · 7 (148.8, 2 `Dynamic`) · 0 (21.1) · 0 (21.6) | **1** (= main) |
| B (5.3, −84.5) | 0 (13.6) · 0 | 0 · 0 | 0 (14.1) · 0 (11.6) | **0** |
| C (0.7, −86.7) | 4 (132) | 0 · 0 | 0 (22.8) · 0 (11.5) | **0** |
| control (car driven on 150 m) | 0 | 0 | 0 (10.7) · 0 (6.8) | **0** |

Placement checks:
- Every left car was snapped once and was within 0.3 m of the spot at leave time: A (8.2,−82.4), (8.0,−82.2),
  (8.0,−82.6), (7.9,−82.4), (8.0,−82.5); B (5.5,−84.7), (5.4,−84.5); C (0.7,−86.7), (0.7,−86.6).
- A_1's single car is not the lock. It is a traffic car the scripted drive shoved on the way in; it went `lost` and
  was given up (`Abandoned` at 24 s, before the car was left). The approach behind it passed it.
- Control run 1 is invalid and was rerun (log `decision`, `scratch/qa/r1/invalid_control_1_car_near_box`). Its
  "drive on" failed: the player rammed the east approach queue for about 18 s and left the car at (12.5, −81.0), at
  the box entry. It is described as finding 2 below.
- Screenshots: `r1/A_2/shots/0027_0146s_repro_watch.jpg` shows the three-car east queue at the box and the avenue
  moving. `r1/A_1/shots/0018_0089s_repro_watch.jpg` shows the avenue moving and no column.

### Tourist minute (bump a traffic car in a box, watch 90 s)

- **Valid run** (`scratch/qa/tourist_box_s1_despawn/`): +3 m/s bump on `Connector(717)` at s 8.4 in the spawn
  junction. The player's car was then exited in place and despawned by BRP (a named QA isolation), and the player
  watched from 12 m.
  - The bumped car rolled out onto its exit lane at 3.9 m/s, then recovered `Kinematic` on `Lane(296)` 17 s after
    the bump and drove on.
  - Approaches: 0 cars over 30 s, worst 17.4 s.
  - No walker stood (< 0.2 m/s) within 2.8 m of a car.
  - Connector recovery itself was not exercised at runtime: the car left the box before it came to rest. It is
    gated headless (f-rows).
- **First run** (reverse out instead of despawn, +4 m/s, `scratch/qa/tourist_box_s1/`): 15 cars over 30 s. It
  measured two stacked obstacles, not the bump:
  - The bumped car went `lost` (> 60 deg or 4 m off its path) and was abandoned 1.2 m into its exit lane. That is
    the pre-existing `lost` rule.
  - The player's reversing car pressed the east approach head, which stood `Dynamic` 89.6 s against it (class D).
- Two isolation attempts are kept as `invalid_*`: the bump missed the box, or the player's car moved away and took
  the view, so the bubble despawned the bumped car. Log `dead_end`.

### t15 seed 1 (police arrest/approach after the `around_cars` change)

**PASS** (`scratch/qa/t15_s1/summary.json`):
- 33 traffic cars, mean 7.1 m/s.
- Hijack OK.
- Chase: pressure by car at 14.7 s (3.7 m), no escape, at most 2 active police cars.
- Dismount: 2 cars `Dismounted`, 4 crew on foot closing to about 3 m (`dismount.png`: cops next to the player
  between cars).
- No log errors.
- Frame cost without vsync 3.6 ms (monitor `\\.\DISPLAY9` 30 Hz, Fifo, so the shipped FPS is 30).

## 3. Acceptance criteria

| Criterion | Test performed | Result |
|---|---|---|
| Un-ignore the 3 G4 `_liveness` rows and R1 seeds 1/7: green, with the `Dynamic` 30 s bound | Full suite. G4 seed 1 liveness, the extra row and R1 seed 7 are un-ignored and green. G4 seed 7 liveness (class E) and R1 seed 1 (class D) stay ignored and point to TASK-039, per binding `OPEN_DECISIONS.md`. | **PASS as rescoped** (2 of 5 rows deferred by decision) |
| (c) asserts the `Dynamic` bound city-wide | `traffic_causes.rs:290` uses `clock.dynamic_violation()`, and `dynamic_bound_on` is deleted. (c) and rb far are green in the suite. | PASS |
| Every new mechanism flips RED; G1 clean in touched city gates | My own H1/H2/M1 flips went RED and then GREEN after restore (above). G1 max depth 0.000 is printed in every city row I ran. The other mechanisms (f, g, M2, m1, m2) were flipped by the implementer and fixer (`scratch/flips/`). I did not repeat those. | PASS |
| Runtime R1: A median ≤ main (1), B and C 0, control 0 | 5 A runs, 2 B, 2 C, 2 valid controls (above) | **PASS** (A median 1; B 0/0; C 0/0; control 0/0). A has high variance: 2 of 5 runs are class D after the snap. |
| No regressions: suite, gridlock, benches, clippy, client, tree_check, 5 CI workflows | All green on Windows. Linux/WSL was not re-run by me; the fixer has logs in `scratch/fixer/wsl/`. CI runs after the merge. | PASS locally; CI pending the merge |
| `traffic.md` box section; GDD line | `traffic.md` "Junction box" states what TASK-037 resolved and the open TASK-039 / TASK-036 item 4. GDD §6.2 gets "Пешеход обходит машину, стоящую у него на пути (поправка 2026-09-27, TASK-037)". | PASS |
| Owner run (not gated) | Not gated. Checklist below. | OWNER |

Owner checklist, `cargo run --release`, seed 1:
1. Drive into the junction in front of the spawn, bump a turning car, get out, and watch 1-2 min. The bumped car
   should settle and drive on. A car that returns to its line in the box slides sideways without turning its nose.
2. On an avenue, watch a car go around a parked car. The nose now turns at most about 13 deg while merging back
   (it was 31-39 deg), so at low speed the car crabs more.
3. Park a car across a crosswalk and watch the walkers. See finding 1: some still jam there.

## 4. Bugs and findings

**F1 (medium, pre-existing, not closed by the Q1 fold-in): walkers jam at a car standing across a crosswalk.**
- Runtime spot B: 9 and 6 Wander civilians stood still (< 0.7 m moved) within 6 m of the left car, for up to
  102.9 s and 149.9 s. Traffic there is fine (0 cars over 30 s).
- For comparison, main B had 10 and 17 walkers up to 153 and 137 s, and TASK-032 final had 9 and 14 up to 150 and
  147 s. So the counts dropped somewhat, but the class is still there.
- Headless repro (my probe `scratch/probe/ws/probe/tests/qa_walkers.rs`, output
  `scratch/qa/probe_walkers_spot_b.txt`): seed 1, player at the R1 stand point, a car parked at (5.3, −84.5).
  - With the car heading 0 deg: 13 walkers stalled for 10 s or more, the worst 94 s.
  - With the car heading 90 deg: 11 walkers, the worst 118.5 s.
  - They stand in clusters with their capsules touching (nearest walker 0.60 m = 2 × 0.3), 0.1-1.3 m from the car.
    Their intents point both towards and away from the car.
- Reading: counter-flowing walkers meet in the corridor the car leaves around its corners, and push against each
  other.
- The claim "walkers pinned at a car: 0 s in every traced row" holds for the traced G4/R1 rows only.
- Expected: walkers pass a car on a crosswalk within seconds. Actual: groups stand 70-150 s.
- Route: TASK-036 (walk avoidance; walker-walker counterflow at a narrowed crossing). It is not a traffic lock and
  not new.

**F2 (medium, pre-existing, untracked): a player's car left at a box entry, against the approach head, freezes that
approach.**
- The "second shape" in `TASK_FINAL.md` ("Added at TASK-032 closure": a car within a pass length of a stop line)
  has no owner after this task. It is not in the ACs, not in `OPEN_DECISIONS.md`, and not in
  `pending/TASK-039/task.md`.
- Reproduced twice here:
  - Invalid control 1: the player rammed the east queue and left the car at (12.5, −81.0). Result: 15 cars over
    30 s, including 4 `Dynamic` cars pressed together in the queue for 149 s.
  - Tourist first run: 15 cars over 30 s.
- The `Dynamic` part is class D: bodies at rest inside each other's skin.
- Recommend adding it to TASK-039 explicitly.

**F3 (low, QA harness): the R1 snap can teleport the left car against a waiting queue head.**
- It turns a spot-A run into a class-D run (A_2, A_3), which is why A has a spread of 0-7.
- Proposed rule in `PCTX_PROPOSALS.md`: report runs with a `Vehicle` switch on the snap tick separately, or snap
  only onto a clear spot.

No regression was found against main. Code notes, not bugs:
- The M2 exception also acts on the 25 m straight lane strip. A plain lane car can brake for a held car in the box
  that is not on its path. That was already the case when such cars stood `Dynamic` (before TASK-037). Runtime B, C
  and control show no new stand.
- `around_cars` returns `to` when every corner fails both view checks (`unwrap_or(to)`), so the walker presses
  straight on. This is an edge case that may feed F1.

## 5. Verdict

**SHIP.**
- R1 goal: A median 1 (= main), B 0, C 0, control 0.
- The suite is green: 607 / 0 failed, and clippy and the client tests are clean.
- My flips of the heading cap and the `around_cars` metric went RED, and GREEN after restore.
- t15 seed 1 passes.
- The tourist minute is clean once the player's car is isolated.
- The residues seen at runtime are:
  - class D (A_2, A_3, invalid control 1, the first tourist run): TASK-039;
  - the untracked stop-line shape (F2): to add to TASK-039;
  - the walker counterflow jam (F1): TASK-036.

  All of them are pre-existing on main, so none is a new freeze class.

Log: 2 `decision` entries (control rerun and more A runs; F1 classification) and 1 `dead_end` (tourist isolation
attempts). 1 PCTX proposal (F3). children: 0 launched / 0 reported.
