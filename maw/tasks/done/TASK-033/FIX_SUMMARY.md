# TASK-033 — FIX_SUMMARY (fixer round 1, claude opus, medium)

Inputs: `IMPL_REVIEW.md` (NEEDS_WORK), the last `OPEN_DECISIONS.md` entries, and the orchestrator note (binding).
Commits on `fix/traffic-gridlock`: `e19c5b1` (code + gates) and the follow-up summary commit.

## Preflight: the review claim that would break correct code if applied verbatim

The review's option (b) for issue 1 says "the code does not re-grant a lapsed holder in the tick it lapsed".
If a waiter for a conflicting connector is only room-blocked (its destination lane is full), it does not
block later connectors (`junction.rs` grant loop: `continue` without `blocking.push`). With (b), the node
would sit idle for a tick while the holder that could still go is kept out. That is harmless but useless
churn, and it would still leave uncontested holders flapping every `reservation_timeout`. I checked the
diagnosis: it is real. A lapsed holder re-requests with a fresh stamp (`waiting` was `None` while it held
the grant), and it is re-granted in the same call when nothing earlier conflicts. The gate's continuous
`(c, e)` counter cannot see that gap. I went with the orchestrator's rule (review option (a), enforced in
code as well as in the gate), not (b).

## Fixed

1. **Orchestrator 1: `idm.acceleration: 1.5`, stand bound 40 s.** `assets/traffic/traffic.ron`: `acceleration: 1.5`,
   and the comment says it was retuned in TASK-033 and why (GDD §5.2 calls 0.73 a start value).
   `traffic_gridlock.rs`: `MAX_STOP = 40.0`, and each seed prints its margin. Worst stand per seed:
   - seed 1: 18.8 s (margin 21.2 s)
   - seed 2: **34.3 s (margin 5.7 s, the tightest)**
   - seed 7: 18.0 s (margin 22.0 s)
   - seed 42: 27.7 s (margin 12.3 s)
2. **Review issue 1 (lease re-grant vs the gate), orchestrator 2.** The rule is now "a lease lapses in favour of a
   competitor".
   - `junction.rs`: waiters are pruned before occupants, so contention comes from the current heads. A
     holder on its source lane with its nose before the stop line loses the grant only when
     `tick - last >= lease_ticks` AND some waiter at that node wants a connector in `conn.conflicts`. An
     uncontested holder keeps the grant, with no lapse/re-grant churn.
   - The module doc, the `TrafficConfig::reservation_timeout` doc and the `traffic.ron` comment now state
     the rule.
   - `traffic_gridlock.rs`: a stale grant is counted only while a waiter for a conflicting connector sits
     in that junction's `waiters`.
   - New deterministic gate `traffic_intersection.rs::contested_lease_lapses_to_the_waiter` on `plus()`. The
     north head is granted N→S. A production dummy stands 1.8 m before its bumper, short of the stop line.
     An east head waits for E→W, which conflicts. Asserted: the grant lapses on loop tick `lease` or
     `lease + 1` (lease = ceil(timeout/dt) = 320; observed tick 320), the east car holds the grant, and the
     north car is back in `waiters`. `GATE BROKEN` preconditions: the connectors conflict, the holder is
     granted on its first tick, the holder stands before its stop line every tick, the walker does not
     move, traffic ran, and the east car waits.
3. **Review issue 2 (asserts masked each other), orchestrator 3.** `nobody_playing` now collects both
   violations (stand bound, contested stale grant) and panics once with all of them. Flips re-run below.
   With the lease off, seed 42 now reports both: `a car stood 116.5 s ... ; a car standing before its stop
   line held a contested grant 112.20 s`.
4. **Review issue 3 (re-run flips at a = 1.5 / 40 s), orchestrator 4.** All flips are RED (table below). The
   heads-off flip stays RED (seed 1 at 69.0 s), so per the orchestrator's condition no separate lane-head
   fixture was added.
5. **Review issue 4 (runtime baseline), orchestrator 5.** I re-ran the playtester's `baseline.py` on seed 1
   for 2 min at a = 1.5 (release `--features dev`, `--settings-id .qa` via `tools/qa/brp.py`). Output is in
   `scratch/baseline_s1_a15/`. See Test results. **No gridlock. The saturation queues are still there**:
   the jam oracle fires 13 times (27 before), and `stopped_cars` averages 9.9 with a peak of 19. I am
   reporting this for the owner as junction saturation, not as a clean pass.
6. **Review issue 5 (lease scope residue), orchestrator 6.** One line added to `OPEN_DECISIONS.md` and to
   `maw/tasks/pending/TASK-032/task.md`. A holder standing on its connector, or with its nose past the stop
   line, keeps its grant indefinitely (for example behind a player-parked car in the box). This was the
   behaviour before this task, and TASK-032's shared occupancy rule inherits it.

### Flip-RED record (a = 1.5, MAX_STOP = 40 s)

| Perturbation (code, restored after) | Gate | Result |
|---|---|---|
| Lease off: `tick - *last < lease_ticks` → `true` in `junction.rs` | `contested_lease_lapses_to_the_waiter` | RED: the north car kept its grant for 640 ticks |
| same | `traffic_gridlock` | RED seed 42: stand 116.5 s AND contested stale grant 112.20 s (both asserts fire); seeds 1/2/7 green (lease never contested there) — `scratch/fix_flip_lease_off.txt` |
| Lane-head rule off (no `heads` filter in request + waiter retain) | `traffic_gridlock` | RED seed 1: 69.0 s; seeds 2/7/42 green (28.4 / 25.0 / 36.7 s) — `scratch/fix_flip_heads_off.txt` |
| Stop line at the lane end (`if false && t > 0.5` in `graph.rs`) | `traffic_gridlock` | RED seed 1 104.5 s, seed 2 54.9 s, and `stop_lines_leave_the_crosswalk_free` (lane 4: 104.48 vs 102.20) — `scratch/fix_flip_stopline_off.txt` |
| none (fixed code) | both | GREEN: fixture lapse on tick 320 of lease 320; city seeds as in item 1, contested stale ≤ 5.00 s (seed 42) |

## Skipped

- **Review option (b) for issue 1**: not applied (see Preflight). The orchestrator chose contention.
- **Review "missing coverage": uncontested re-grant case.** Not gated. The old behaviour (lapse and
  re-grant in the same tick) cannot be observed from outside `junction::update`, because the occupant
  entry persists across samples. So an "uncontested holder keeps its grant" test would stay GREEN against
  the old code. It could only be flipped by reading `Junction::moved`, which is the code's own timer and
  close to a tautology.
- **Review "missing coverage": lower bound in `stop_lines_leave_the_crosswalk_free`.** Not in the orchestrator's
  list, and scope is small-fix. Noted as a possible follow-up. The test shares the production `t > 0.5` filter.
- **Nits** (the `lease` tuple, a `///` on `Junction::moved`, the duplicated numbers in the `traffic.ron` comment):
  cosmetic and not required. Left as they are to keep the diff surgical.

## Test results

- `cargo test -j 2 -p gta_sim --test traffic_intersection -- contested --nocapture`: ok, `lease 320 ticks, the north car lost its grant on tick 320`.
- `cargo test -j 2 -p gta_sim --test traffic_gridlock -- --nocapture`: 5 passed. Per-seed lines:
  `seed 1: 21.8 cars, 141 grants, worst stand 18.8 s, stands > 20 s: [], worst stale grant 0.91 s`;
  `seed 2: 16.6 cars, 151 grants, worst stand 34.3 s at (13.9, -2.0), 13 stands > 20 s, stale 1.80 s`;
  `seed 7: 12.2 cars, 115 grants, 18.0 s, [], stale 0.44 s`;
  `seed 42: 20.7 cars, 140 grants, 27.7 s at (-2.4, -14.4), 11 stands > 20 s, stale 5.00 s`.
- `cargo test -j 2 -p gta_sim -p citygen`: exit 0, **516 passed, 0 failed, 5 ignored** (the 515 before, plus the new lease gate).
- `cargo test -j 2 -p gta_sim --test traffic_bench -- --nocapture`: mean 1.68 ms, p95 2.00 ms, max 2.28 ms (limit 19 ms, GDD 4 ms).
- `cargo clippy -j 2 -p gta_sim --all-targets -- -D warnings` and `cargo clippy -j 2 --workspace --all-targets -- -D warnings`: clean.
- `cargo test -j 2 -p gta_like --bin gta_like`, three runs: 81 passed each time.
- `python tools/qa/tree_check.py`: passed.
- Runtime baseline `python maw/tasks/in_progress/TASK-031/scratch/tools/baseline.py scratch/baseline_s1_a15 1` (128 s):
  - `stopped_cars` series: 0, 0, 1, 5, 3, 5, 6, 7, 6, 8, 7, 11, 13, 12, 14, 13, 14, 12, 10, 12, 12, 14, 11, 8, 7, 7, 9, 11, 10, 10,
    9, 8, 8, 10, 10, 9, 8, 12, 13, 14, 17, 18, 19, 18, 15, 12, 11, 9, 8, 10, 9, 12 (mean 9.9, max 19, about 22 cars alive).
  - Compared with a = 0.73 (`scratch/baseline_s1`): the jam oracle fires 13 times (was 27). Cars stuck > 20 s: 7
    (was 17), worst 25.1 s at (−4.5, −90.4) (was 28.1 s). Stuck civilians: 0. No car stays frozen.
  - The queues at (−0.6, −69.4) and (−4.5, −90.4) still reach 20-25 s. That is saturation at node 83 in front
    of the spawn, and the owner may still read it as a jam. The game process was shut down (`tasklist`
    shows no `gta_like`).
- CI on `d85e128` (branch push): all 5 workflows `success`: clippy, sim gates, citygen gates, client gates, repo checks.

## Owner checklist

- `cargo run --release -- --seed 1`, stand at the spawn looking north for 2 min. Expected: the junction keeps
  discharging and nothing stays frozen, but queues of several cars wait 20-25 s. Decide whether that level
  of saturation is acceptable, or whether a junction model with shorter exclusive holds is wanted (TASK-032 / later).
- Is the snappier start with `idm.acceleration 1.5` acceptable to the eye?
