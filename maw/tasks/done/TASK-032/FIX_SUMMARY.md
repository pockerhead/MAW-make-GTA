# FIX_SUMMARY — TASK-032, fixer round 3 (tests and follow-up task only)

Inputs: the orchestrator note (items 1-4) and the last `OPEN_DECISIONS.md` entry (2026-09-27, option A,
binding). `IMPL_REVIEW.md` was read; its items were handled in rounds 1-2 (`FIX_SUMMARY.prev-1.md`,
`FIX_SUMMARY.prev-2.md`). No production code changed in this round. Evidence is in `scratch/fixer3/`.

Archive: `FIX_SUMMARY.prev-2.md` (round 2) was already in place, committed in bf024c5, and no
`FIX_SUMMARY.md` existed. Nothing was moved.

Preflight, the review claim that would break things if applied verbatim: issue 2, "add
`worst_dynamic() <= 30 s` to every city gate". I checked it: on the current code that bound is RED in the
three G4 rows (95.5 / 77.0 / 95.5 s) and in cause (c) (53.5 s, node 141). The cause is the in-view box
lock, which predates this task. Option A moves the bound out of those rows, which this round does.

## 1. Fixed

| Note item | What was done |
|---|---|
| 1. G4 `Dynamic` bound → `_liveness` | `traffic_junction_box.rs`: `check()` asserts the `Dynamic` bound only when `liveness` is set. Otherwise it prints `seed N (reported, TASK-037): …`. The three correctness rows (`seed_1_box_keeps_moving`, `seed_7_box_keeps_moving`, `seed_1_car_behind_the_body_leaves`) keep the lease assertion, G1 and `GATE BROKEN` checks. There is no contact assertion in this file. All `_liveness` ignore reasons now point to TASK-037. The module doc is rewritten to match. |
| 1. (extra row) | The extra row had no `_liveness` twin, so I added `seed_1_car_behind_the_body_leaves_liveness` (ignored, TASK-037). Its extra-car bound (`extra ≤ MAX_STOP`) also moved there, because the extra car stands 100 s from the same lock. If it had stayed, the correctness row would stay RED, which contradicts option A. This is a log `decision`. |
| 2. Cause (c), scene lane only | `traffic_support::StandClock` now records where each car's longest `Dynamic` stand happened. New method: `dynamic_longer_than(s)`. In `traffic_causes.rs`, `dynamic_stands_on_the_approach` splits `Dynamic` stands over `DYNAMIC_STAND_S` into two groups. The first is on the scene lane or its approach: from 15 m before the lane start (the feeding connector) to its stop line, lateral −1.7..3.25 m, so the lane plus a curb shove. That group is asserted. The rest is printed. The module doc was updated. The node-141 holder is at along −6.1 m, lateral −2.5 m, on `Connector(1296)`, lane 462 → 510, which does not feed scene lane 507, so it is printed and not asserted. |
| (same class) headless R1 ignore reasons | `traffic_causes::r1_car_left_in_the_box_seed_{1,7}` reasons pointed at "FIX_SUMMARY", which is round-specific. They now point at TASK-037, the same lock. |
| 3. TASK-037 | `maw/tasks/pending/TASK-037/task.md` (Mode full, Priority high, Branch `feature/box-lock-in-view`, Domains bevy-ecs, gates, game-design, blocked by TASK-032). It contains the evidence: runtime R1 spot A on main (1, 1 cars; 148.2/148.9 s) and on the branch (3/12/2; 149.1/149.0/144.9 s), the headless R1 rows (re-run this round: seed 1 149.4 s, 149.4 s `Dynamic`; seed 7 147.8 s, 122.4 s `Dynamic`), the G4 liveness rows (re-run: 97.7/94.8 s, extra 100 s), and the node-141 case. It also has the redesign direction (re-route or U-turn of the blocked queue, or a box pass), the push-through physics (≈16 kN grip against 6.9 kN drive, pinned walkers, bulldozer rejected), and the acceptance rows. |
| 3. TASK-036 link | One paragraph in `pending/TASK-036/task.md`: TASK-037 owns the box side of node 141, and the pinned walkers are item 1 here. |

## 2. Skipped

- Review issues 1-7 and the nits: handled in rounds 1-2. This round is tests only, by the orchestrator note.
- Review issue 2 (a city-wide `Dynamic` bound in every gate): superseded by option A for G4 and cause (c).
  Every other city gate keeps it.
- `seed_1_car_behind_the_body_leaves` keeps its name, although the "leaves" bound now lives in its
  `_liveness` twin. I kept the rename out of this round to stay surgical. The doc says which row asserts what.

## 3. Flips (re-anchored gates)

| Sabotage | Result |
|---|---|
| `junction.rs` D2: a stuck holder on its connector is never demoted (`Segment::Connector(k) if k == c => true`) | `seed_1_car_behind_the_body_leaves` RED: "worst stale holder 100.00 s" against the lease of 5 + 1 s. The seed 1/7 rows stay green, as their holders are not stale on the connector. Restored with `git checkout`, and `lib.rs` was touched. |
| Cause (c) classifier widened to the whole city (test-side) | `c_character_in_the_lane` RED: "AI cars on the scene lane or its approach stood > 30 s in Dynamic: [(53.5, (363.0, 492.2))]". This proves the assertion is wired. The file was restored from a copy, with 0 `FLIP` markers left. |
| `--ignored` on the 3 G4 `_liveness` rows | All RED (approach stands 94.8-100 s, the `Dynamic` bound, the extra car 100 s). These rows hold the open lock for TASK-037. |
| `--ignored` on the headless R1 rows | Both RED (numbers above). |

There is no production-side flip for the scoped (c) bound. No mechanism puts a `Dynamic` car on the scene
lane in this scene today. The test-side flip shows detection only. The b-rows cover the bumped-car recovery.

## 4. Test results

- `cargo test -j 2 -p gta_sim -p citygen --no-fail-fast` (once, `scratch/fixer3/suite.txt`): 74 binaries,
  **589 passed, 0 failed, 12 ignored**. Round 2 had 585 passed, 4 failed and 11 ignored. The 4 failures
  are now green, and the +1 ignored is the new extra `_liveness` row.
- After the suite, `traffic_causes.rs` only got a `type Stands` alias for clippy `type_complexity`, with no
  change in behaviour. Re-run: `cargo test -j 2 -p gta_sim --test traffic_causes` gave 11 passed, 2 ignored
  (`scratch/fixer3/traffic_causes.txt`).
- `cargo test -j 2 -p gta_sim --test traffic_junction_box`: 3 passed, 3 ignored.
- `cargo clippy -j 2 --workspace --all-targets -- -D warnings`: clean on the second run
  (`scratch/fixer3/clippy.txt`). The first run caught `type_complexity`, which is now fixed.
- `python tools/qa/tree_check.py`: passed.
- Not run: `cargo test -p gta_like --bin gta_like`. No client code changed.
- `rustfmt --edition 2024` was applied to `traffic_causes.rs` only. `git diff --stat` shows no other file
  touched by it.
- `git status`: the 3 test files, `pending/TASK-036/task.md`, the new `pending/TASK-037/`, `log.jsonl`
  (2 `decision` entries) and this file. `metrics.md` was already modified before this round (not mine).

children: 0 launched / 0 reported.
