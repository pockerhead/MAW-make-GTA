# QA_REPORT — TASK-039 (universal traffic progress rule)

QA: claude opus, medium. Code under test: `feature/box-uturn` at `0edd136` (HEAD `6790736` adds only task
artifacts). Evidence: `scratch/qa/final/` (my scripts, logs, flips, runtime sessions under `rt/`).

**Verdict: SHIP-PENDING-RUNTIME**, with two items for the orchestrator:
1. **Flip "symmetric exemption one-sided" stays GREEN on Windows.** Its only RED is the round-1 fixer's Linux
   run, made on code that still had the rules round 2 removed. It has not been re-flipped since. I did not
   use WSL, per the orchestrator note. Either accept the round-1 evidence or re-flip it through CI.
2. **MAJOR, not a regression (B1 below).** The rule does not cover two standing bodies in one lane closer
   together than about a car length plus the minimum gap. The passer squeezes into the first body, stops
   for the second and stays inside the first indefinitely. Without the rule the car also stands forever
   (control run), but behind the body rather than inside it.

Everything else the orchestrator asked for holds:
- The hook, trigger and wheel-ray flips go RED.
- No AI car stood more than 30 s in any runtime repro.
- t15 seed 1 passes.
- No relaxed car passed through a third body, headless or at runtime.

The look of the squeeze is an owner item (section 3, last row, and the checklist in section 5).

## 0. Disconfirmation

**Counter-example written down first:** "A passer whose relaxation has dropped to `physics_only` while it
is still inside its blocker, and whose next reason to stand is not that blocker, can never be relaxed
again. `candidates` needs `relaxed.is_none()` or a re-arm against the SAME blocker, and it needs a chain
with a sink. A 2-cycle (after the cycle trigger was dropped), or a second body right after the first, then
leaves the car standing inside the first body for good."

**Search.** In `progress.rs`, `candidates` defines `free = relaxed.is_none_or(|r| r.physics_only && r.blocker ==
edge.target)`, and it skips any edge without a sink (`roots` gives `None` on a cycle). Round 2 removed
`release_stalled`. So a car relaxed against B1 whose edge now points at B2 is never a candidate.

**Probe.** `scratch/qa/final/qa039_two_in_a_row.rs` was a temporary test in `crates/gta_sim/tests/`. It is
deleted from the tree and the working tree is clean. The setup: a kinematic car, parked car B1, then B2 (a
car or a dummy) `gap` metres further on. Output is in `probe_two_in_a_row_loop.txt` and
`probe_two_in_a_row_street.txt`.

| Floor | Gap / second body | Result |
|---|---|---|
| loop (no neighbour lane) | 1 m car, 3 m car, 1 m dummy, 3 m dummy | relaxed against B1 at 15.9 s, drives into it, stops 2 m before B2 **inside B1**, stands **69-70 s** until the run ends. The oracle's own relaxation check fires: "still relaxed 2561 ticks". |
| loop | 8 m car | passes both: B1 at 15.9 s, B2 at 28.5 s, past at 32.5 s |
| two-way street, oncoming lane free | 1 m car, 3 m dummy | clean lane pass around both, 0 relaxations |
| two-way street, oncoming feeder every 3 s | 1 m car | lap 1: lane pass at 16 s. Lap 2: relaxed against B1 at 65 s, stops inside it, stands **79.9 s** until the end (150 s run), same oracle violation. A second car stuck inside B1 in the 3 m case. |
| same, **rule off** (`wait_seconds` 1e6, control) | 1 m car, 3 m car/dummy | stands **100-120 s** behind B1 |

**Result: the counter-example HOLDS** for the "second body" form. It is not a regression in stand time,
because the control stands forever too, but the task's goal is not met for this geometry (B1 below). I did
not reproduce the 2-cycle form in any row or runtime run. `stalled_relaxed` peaked at 1 transiently in
M1/N1, and every runtime end-of-run snapshot shows 0.

## 1. Environment

- No docker-compose or dev server. The direct test runner ran on `D:/test-gta-like` (branch checkout,
  shared `target/`, `-j 4`, one cargo at a time). Other projects' cargo processes were running on the
  host. They were left untouched.
- Runtime: release build with `--features dev` (BRP on 127.0.0.1:15702), launched through `tools/qa/brp.py`
  Game / the TASK-040 and TASK-031 `pt.Session` harness with the QA settings id. One game at a time and one
  screenshot at a time. `tasklist` showed no `gta_like` after each session.
- Independent runtime oracle: `scratch/qa/final/qa_watch.py <out.json> <x> <z> <harness.py> [args]`. It runs
  any harness unchanged via `runpy`. A thread polls raw JSON-RPC at about 5 Hz and records:
  - the SAT depth of every traffic car rect (1.2 x 2.04) against every vehicle rect and every on-foot
    character circle (r 0.3), exempt only while `TrafficCar.relaxed` names the pair (tolerance 0.15 m for
    vehicles, 0.10 m for characters);
  - the relaxed-pair depth;
  - stands within 45 m of the scene;
  - the player's height and health;
  - `TrafficStats`.
- WSL was not used (orchestrator note). Linux verification comes from CI: all 5 workflows are `success` on
  `0edd136` (`gh run list`: sim gates 36392719784 in 23m25s, client gates 36392719614, clippy 36392719604,
  citygen 36392719648, repo checks 36392719833).
- No containers or services were started. There is nothing to clean up beyond the game processes, which
  are closed.
- Reproduce:
  - Suite: `cargo test -j 4 -p gta_sim --no-fail-fast` (split in three calls, see `win_*.txt`) and
    `cargo test -p citygen`.
  - Flips: `python scratch/qa/final/qa_flip.py apply|restore <hook_true|trigger_off|symmetric_off|rays_off|relax_all>`.
    Restore runs `git checkout` + touch + a sha256 check against `sha_before.txt`.
  - Runtime, for example:
    - `python scratch/qa/final/qa_watch.py rt/m1_2.watch.json 34.5 -78.5 scratch/qa/final/qa_m1.py rt/m1_2`;
    - R1 through `maw/tasks/done/TASK-037/scratch/qa/run_r1.py branch <out> 1 <x,z>`;
    - `tools/qa/scenarios/t15.py --out <dir> --seed 1`.

## 2. Test results

### Existing suite (Windows)

| Group | Result | File |
|---|---|---|
| Heavy traffic targets (`traffic_causes`, `_progress`, `_progress_plumbing`, `_junction_box`, `_gridlock`, `_go_around`), run after all flips were restored | 48 passed, 0 failed | `scratch/qa/final/win_heavy.txt` |
| All other `gta_sim` targets + `--lib` | 553 passed, 0 failed, 4 ignored | `win_light.txt` |
| Benches (one run each) | traffic mean **2.32 ms** (limit 19), civilians **2.07 ms** (limit 8), SWAT **2.46 ms** (limit 11), street turnover 1.17 ms | `win_bench.txt` |
| `citygen` | 32 passed, 3 ignored | `win_citygen.txt` |
| clippy, client gates, tree check | not run locally; CI `clippy`, `client gates` and `repo checks` are `success` on `0edd136` | CI |

Total for `gta_sim`: 601 + 4 bench = 605 passed, 0 failed, 4 ignored. The un-ignored rows
(`seed_7_box_keeps_moving_liveness`, `r1_car_left_in_the_box_seed_1` / `_seed_7`) all ran green.

Key row numbers:
- M1: worst 18.2 s, relaxations S→D and N→D at 13.97 s, D→P at 17.8 s, 2079→P at 20.75 s, depth 1.80 m.
- N1: worst 19.8 s, 6 relaxations against the player, lift 0.018 m.
- R1 seed 1: worst 27.8 s.
- R1 seed 7: 16.9 s.
- G4 seed 7: 24.8 s.

These match FIX_SUMMARY.

Bench caveat: the traffic bench scene makes 0 relaxations, so no bench measures the progress path while a
relaxation is active.

### My flips (independent of the fixer's `flip.py`; each restored and sha256-checked, then GREEN in `win_heavy.txt`)

| Flip (input perturbed) | Rows | Result |
|---|---|---|
| `hook_true`: `modify_contacts` keeps every contact | plumbing + city | **RED (D3).** Row a): player hit, 100 → 88 hp, `KnockedDown`. Row b): dummy knocked down. Rows c/d: `CollisionStart`, parked car moved/tilted. Pinned rows and re-plan row RED. N1 RED: stands of 90.8 / 85 / 71 s, and the relaxation outlives its bound (`still relaxed 2561 ticks`). **M1 stays GREEN** (worst 26.3 s; the contact pushed D and it was abandoned). The third-body oracle is not what goes RED here: with contacts on, nothing passes through anything. |
| `relax_all` (my own variant of the spec's sabotage): `exempt_pair` true for any body while relaxed, and sensing skips every body while relaxed | city | **Third-body oracle RED**: G4 seed 7 (1.22 m), G4 seed 1 (0.87 m), seed-1 extra (1.38 m), M1 G1 (2079 through P, 1.80 m). N1, R1 and `c_character` stay GREEN (liveness rows). |
| `trigger_off`: `candidates` never pushes | city | **RED.** M1: 120 s in `Dynamic`, 4 cars > 30 s, 0 past. N1: 94.5 s, "no relaxation against the player". |
| `rays_off`: `cast_ray_predicate` accepts the partner | plumbing | **RED.** Awake parked car drifts 33.0 m and tilts 167.8°. Pinned rows: the body behind moved 0.052 / 0.054 m. |
| `symmetric_off`: recover.rs passer skip and contact.rs reverse exemption both off | `traffic_causes` 13, `traffic_progress` 2, plumbing 10 | **GREEN on Windows** (b3 16.4 s, `c_character` 21.9 s). See verdict item 1. |

Files: `scratch/qa/final/flip_*.txt`.

### Runtime (release dev build, seed 1, `--settings-id .qa`)

| Session | Placement | Stands (harness / my watcher) | Relaxations, depth | Third bodies (watcher) | Other |
|---|---|---|---|---|---|
| M1, fixer's `m1_snap.py` unmodified (`rt/m1_1`) | **Q landed 3.9 m off** (30.11, -78.86): not the M1 geometry, see log `dead_end` | worst 13.7 s, 0 > 30 | 1 | – | not counted as M1 evidence |
| M1, `qa_m1.py` (re-snap, pitched camera, bursts) (`rt/m1_2`) | P 0.06 m, Q 0.14 m off | 14.0 / **16.1 s**, 0 > 30 | 1: westbound car through Q at 20.3 s, **1.16 m** | none | stats: unexplained 0, stalled 0 |
| N1, fixer's `repro_player_in_junction.py` (`rt/n1_1`) | player 0.6 m from (7.9, -81.3) | in-box max stop 17.2 s, aside 10.3 s / watcher 23.0 s, 0 > 30; 18 samples with a car through the box | 4 against the player, 0.94 m | none | health 100, y 1.08-1.20 |
| N1 look, `qa_n1_look.py` (teleport to the TASK-040 point, facing the approach) (`rt/n1_look`) | (7.9, 1.07, -81.3) | watcher **18.8 s**, 0 > 30 | **6 against the player in 100 s**, **1.45 m** | none | health 100, y 1.058-1.20 |
| R1 spot A (8.2, -82.5), TASK-037 `run_r1.py` (`rt/r1_A`) | left at (7.8, -82.4) after a 42.9 m snap | harness over_30 [] / watcher 18.2 s | 6: a follower through the Dynamic head (2.03 m), the head through the left car (1.39 m), 4 more through the left car | 0.173 m vehicle pair at the snap tick (the snap put the left car against the east head, the TASK-037 known class); 0.111 m walker vs car 96 m away, no relaxation involved | 1 progress recovery |
| R1 spot B (5.3, -84.5) (`rt/r1_B`) | (5.3, -84.5) | [] / 29.0 s (no relaxation in the run) | 0 | 0.207 m character near the spawn at 6.7 s, no relaxation involved | |
| R1 spot B rerun (`rt/r1_B2`) | (5.3, -84.6) | [] / 34.5 s, which is an **`Abandoned`** car the scripted drive shoved, standing 36.3-70.8 s, before the car was left (81 s) | 1: through that Abandoned car, 2.37 m | 0.395 m walker vs car at (-77.1, -1.3), 85 m from the scene; that car was never relaxed | not an AI stand |
| R1 spot C (0.7, -86.7) (`rt/r1_C`) | (0.7, -86.7) | [] / 20.8 s (an Abandoned car) | 1: through an Abandoned car, **2.40 m** | none | |
| t15 seed 1 (`rt/t15_s1`) | – | watcher 6.8 s | 0 | none | **PASS**: 33 cars at 7.4 m/s mean, hijack OK, pressure by car at 18.09 s at 16.7 m (bound 19.33 s), not escaped, ≤ 2 police cars, dismount 2 crew reached (`dismount.png`: cop on foot by the cars). Frame cost 3.05 ms without vsync (monitor `\\.\DISPLAY9` 30 Hz, Fifo, so the shipped FPS is 30). No log errors. |

Runtime third bodies: none of the overlaps above involves a relaxed car or its exempt partner. They are
ordinary contacts with collisions on: the snap tick, walkers against car sides. I have no HEAD runtime
baseline for those. The headless third-body oracle, 0 on HEAD over 27 city oracles and 0 now, carries the
rule's guarantee.

What the screenshots show:
- `rt/m1_2/shots/0010_0021s_m1_relaxed.jpg`: the westbound car's roof inside Q. Two red bodies merged.
- `rt/m1_2/shots/0011_0022s_m1_relaxed.jpg`: it comes out the other side.
- `rt/n1_look/shots/0008_0019s_n1_squeeze.jpg`: the car drives straight through the player. His head
  shows through the bonnet and windscreen, and his body is wholly inside the car.
- Bursts at 17-19 s, 29-31 s, 49-51 s, 60-64 s, 74-77 s and 86-88 s repeat that N1 image.
- R1 A-C: the depth reaches 2.0-2.4 m, a whole car width.

At 6 m/s this reads as ghosting, not as squeezing past, especially through the player. D1 accepted "brushing
past or slightly into the blocker", so this goes to the owner.

## 3. Acceptance criteria

| Criterion | Test performed | Result |
|---|---|---|
| New headless rows (M1, N1; class E/D un-ignored) with stand ≤ 30 s, 0 third-body pass-through, no `Dynamic` > 30 s, N1 player unhurt, depth reported, fixtures `GATE BROKEN`-checked | Ran `traffic_progress` (M1, N1) and `traffic_progress_plumbing` (10), all green. The rows print depth, `stalled_relaxed` and relaxation order. | PASS |
| Un-ignore G4 seed 7 liveness and R1 seeds 1 and 7; green on Windows and Linux | Windows: green in `win_heavy.txt` (27.8 / 16.9 / 24.8 s). Linux: CI sim gates `success` on `0edd136`. | PASS |
| Flip-RED: trigger, relaxation, relax-all → third-body oracle | trigger_off RED; hook_true RED on D3 rows; rays_off RED; relax_all RED on the third-body oracle (G4 seeds 1/7, M1). The cycle break was dropped by the binding amendment. The symmetric exemption (added by the fixer, flip asked by the orchestrator) stays GREEN on Windows. | PASS for the spec's list; **orchestrator's symmetric flip not confirmed** (verdict item 1) |
| No regressions: suite, gridlock 1/2/7/42, G1 gates, benches under `MEAN_LIMIT`, clippy, client gates, tree check, 5 CI workflows | Windows suite 605/0/4 + citygen 32/0/3. Benches 2.32 / 2.07 / 2.46 ms. CI 5/5 `success` on `0edd136`. | PASS |
| Runtime QA (TASK-040 repros, as re-scoped by the orchestrator to `m1_snap` / N1 repro / R1 A-B-C / t15) | 8 valid sessions plus 1 invalid placement: 0 AI cars stood > 30 s; no relaxed car through a third body; player health 100 in N1; t15 PASS | PASS |
| Docs: `traffic.md` Progress section, box "Open (TASK-039)" resolved, GDD §5.2 with D1 | Read `git diff a10f030 0edd136 -- docs`: section present, GDD bullet present. The round-2 edits removed the dropped rules' sentences. | PASS (content not re-audited line by line) |
| Plan names D10 with a cost estimate | `PLAN_FINAL.md` "D10, one body owner" (12-15 files, about 25 gate files, 3-5 tasks) | PASS |
| Owner run (not gated) | Screenshots above. The squeeze reads as a car passing through the body. | OWNER |

## 4. Bugs found

**B1 (MAJOR, not a regression): two standing bodies in one lane strand the passer inside the first.**
- Repro: `qa039_two_in_a_row.rs` (section 0; copy it into `crates/gta_sim/tests/` and run
  `cargo test -p gta_sim --test qa039_two_in_a_row -- --nocapture`).
  - Loop floor: B1 parked at lane-0 s 30, B2 (car or dummy) 1-3 m after it. Car at s 10.
  - Or: the two-way street with an oncoming feeder every 3 s, B1 at s 22.
- Expected (spec Goal 1-2: "a car has waited on a stationary body longer than T → it passes"): the car gets
  past both, or at least is not left standing > 30 s.
- Actual:
  - the car is relaxed against B1 at about 16 s, drives into it at `pass.speed`, stops `idm.min_gap` before
    B2, and stands 69-80 s until the run ends, inside B1 with contacts off;
  - `TrafficStats.stalled_relaxed` = 1;
  - the M4 oracle reports "still relaxed 2561 ticks";
  - a city row with this shape would be RED.
- Cause (code): `progress.rs` `candidates`:
  - `free` requires `relaxed.is_none()` or `physics_only` with the SAME blocker;
  - the car's edge now points at B2 (sensing skips B1 while planning, and B1 is behind the nose once
    inside it);
  - `upkeep` keeps the exemption while the pair touches;
  - so the car can never be relaxed against B2.
- Why it is not a regression: with the rule off, the car stands 100-120 s behind B1 in the same scenes.
- Why it matters: the task's promise is exactly "no stand > 30 s behind a standing body in frame". The
  player leaving a car and standing 1-3 m in front of it, or two left cars in a row, is ordinary play.
  The new form is also worse to look at: a traffic car parked inside the player's car.
- Direction, not prescribed:
  - let a relaxation name the chain of bodies on the passer's way out, so the car relaxes against B1 and
    B2 together when B2 lies within a car length plus `min_gap` past B1;
  - or allow a re-target to B2 while keeping B1's exemption until separation (for example a small set of
    exempt partners);
  - or accept it and gate it as a known residue.

**B2 (minor, gate coverage): the symmetric exemption has no Windows-visible gate.**
- Repro: `qa_flip.py apply symmetric_off`, then run `traffic_causes`, `traffic_progress` and the plumbing
  rows. Everything stays green.
- Expected: at least one row RED. The PCTX proposal of round 1 calls the symmetric exemption necessary.
- Actual: it goes RED only on the Linux `c_character` trajectory, and that was shown on round-1 code.
- Fix: a floor row where a passer stops inside a `Dynamic` blocker that then tries to recover (or re-flip
  it through CI).

**Observations (not bugs of this task):**
- Linux `c_character_in_the_lane` still prints 87.4 s and 85.5 s stands 60 m from the scene, the same as
  HEAD. In the fixer's trace (`scratch/fixer/wsl/c_dbg3.log`) it is car 2080, head of `Lane(506)`, waiting
  87 s on `Grant` edges to a changing stream of moving holders (1983, 1979, ...). That is grant starvation,
  which the rule by design never relaxes (`Grant` edges are not candidates).
- The traffic bench has 0 relaxations, so the frame cost of an active squeeze is not benched. Runtime frame
  cost in t15 is 3.05 ms, also with 0 relaxations.
- The TASK-040 bot drive and the unmodified `m1_snap.py` placement are unreliable harness pieces (Q
  displaced 3.9 m in 1 of 2 runs). `qa_m1.py` re-snaps.

## 5. Owner checklist (not gated)

- `cargo run --release -- --seed 1`.
- Stand on the crosswalk at (7.9, -81.3), facing east at the approach. Every 12-20 s a car drives through
  you at 6 m/s. You take no damage, but the car visibly swallows the character (1.45 m deep).
- Leave a car across the street south of the park, next to a second car. The westbound car passes through
  the second car (about 1.2 m).
- Leave a car in box 83. Cars pass through it by up to a whole car width (2.0-2.4 m).
- Decide whether this is the "brush" D1 accepted, or whether the squeeze needs a slower speed or visual
  cover.

## Children

children: 0 launched / 0 reported
