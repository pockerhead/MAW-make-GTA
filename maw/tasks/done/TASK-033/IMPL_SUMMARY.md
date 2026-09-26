# TASK-033 — IMPL_SUMMARY (implementer, small-fix)

Cost of error: breaks silently (traffic state gridlocks with no signal), so a real headless gate plus flips.
Branch `fix/traffic-gridlock`, commit `6b11f66` (+ this summary commit), pushed.

## Diagnosis (headless, real seed-1 city, production population, no input, 120 s)

Reproduction first: `crates/gta_sim/tests/traffic_gridlock.rs` pose (player at `PlayerSpawn`, chase view
looking north = runtime yaw 0). On the unfixed code it reproduces the playtest exactly: 19 cars standing
for the rest of the run from t ~ 29 s, and the same car (Dynamic, `waiting: Some(638)`) at (7.9, -81.3) as
the runtime baseline. The unfixed code gridlocks on all four gate seeds (stash run of the original
`src/`): worst stand 107.8 / 117.0 / 108.1 / 87.4 s (seeds 1 / 2 / 7 / 42), cars standing > 20 s at the end
19 / 15 / 8 / 20, stale grants up to 113 s. That is AC1 (RED, K = 19 on seed 1).

The playtester's hypothesis (a Dynamic car keeps its reservation) is NOT what the data shows: the Dynamic
cars held no grants. Three causes, found in order (each fix exposed the next):

1. **Circular wait at node 83.** The grants were held by the SECOND cars of two lanes (2080 on lane 248,
   2081 on lane 299), standing behind ungranted lane heads whose connectors conflict with those grants. A
   follower asks early (request distance grows with speed) and got a grant while its head waited. Nobody
   could move, nothing ever released.
2. **Walker / car mutual wait on the crosswalk.** With only a lease, 4 cars still stood 53 s: a granted head
   at the stop line with three Wander civilians pressed into its nose corners. citygen puts the crossing
   centre line 0.01-2.95 m (median 1.68) before EVERY lane end (365 lane ends), so a car waiting at the old
   stop line (the lane end) covered the crosswalk; walkers (walls-only avoidance) push into it, its forward
   cast sees them at distance 0, it waits for them. This is also the playtest's "civilians wait at the
   crossing up to 139 s".
3. **No expiry.** Any holder that cannot move (the above, a Dynamic car stuck, a bailing car with no free
   door, an abandoned car ahead that the occupancy does not see) locks the node forever.

## What was implemented

- `crates/gta_sim/src/traffic/junction.rs` (+49/-11): only the first AI car of a lane joins a junction queue
  (heads from the occupancy; followers drop out of `waiters`); a grant is a lease: it lapses when its holder
  stands (< vehicle `hold_speed`) before its stop line for `reservation_timeout`; the holder re-queues behind
  earlier waiters. A holder whose nose is past the stop line keeps it (dropping it would hold the car on the
  crosswalk or let a crossing kinematic car pass through it, since kinematic casts skip kinematic cars).
  Request distance measured to the stop line.
- `crates/gta_sim/src/traffic/mod.rs` (+3): `Junction::moved` (lease renewal tick per occupant), cleared by
  `release`.
- `crates/gta_sim/src/traffic/graph.rs` (+34/-1): `TrafficLane::stop` (s of the stop line, default = lane end);
  `from_layout` moves it to `crossing - crossing_clearance` for the sidewalk crossing at the lane end.
- `crates/gta_sim/src/traffic/drive.rs` (+7/-4): stop-line obstacle and the never-run-the-line clamp use
  `stop`; passes the lease to `junction::update`.
- `crates/gta_sim/src/traffic/spawn.rs` (+4/-4): spawn speed so a car can stop at `stop`.
- `crates/gta_sim/src/traffic/config.rs` (+6), `assets/traffic/traffic.ron` (+7): data
  `reservation_timeout: 5.0` s and `crossing_clearance: 1.5` m (> walker reach 0.8 = keep_right 0.5 +
  capsule 0.3), both validated > 0.
- `crates/gta_sim/tests/config_traffic.rs` (+10): rule rows for the two new fields.
- `crates/gta_sim/tests/traffic_gridlock.rs` (new, 237): the gates below.

## Gates (`cargo test -p gta_sim --test traffic_gridlock`)

- `seed_{1,2,7,42}_keeps_flowing` — liveness: no Kinematic/Dynamic car stands (< 0.5 m/s) > 60 s in 120 s.
  The bound sits between the unfixed runs (87-117 s) and the fixed ones (<= 39 s). Correctness: no grant
  stays with a car standing before its stop line longer than the shipped `reservation_timeout` + 1 tick.
  Liveness of the fixture: >= 10 AI cars on average, >= 40 grants, casts > 0, player within 5 m of spawn.
- `stop_lines_leave_the_crosswalk_free` — geometry on the seed-1 graph: every stop line is more than a
  walker's reach (navigation `keep_right` + capsule radius, read from data) before its crossing; >= 100
  lane ends checked.

Fixed results (worst stand / stands > 20 s / worst stale grant):
seed 1: 31.7 s / 18 / 2.03 s; seed 2: 34.8 s / 19 / 0.84 s; seed 7: 18.7 s / 0 / 1.61 s;
seed 42: 38.8 s / 19 / 5.00 s (the lease fired).

Flip-RED (each mechanism broken in code, run, restored; outputs in `scratch/flip_*.txt`):
- lease disabled (`true ||` in the lapse condition): seed 42 RED, a car stood 116.6 s (gridlock).
- head-of-lane rule disabled: seed 42 RED 62.1 s (marginal; seed 1 59.1 s).
- stop line at the lane end (crossing rule skipped): stop-line gate RED (lane 4); liveness RED on seeds
  1 / 2 / 42 (98.9 / 73.6 / 65.7 s).
- `crossing_clearance: 0.7` (data, below the 0.8 m reach): stop-line gate RED.
- all three off (original code): RED on all four seeds (above).

## Deviations from the spec / not met

1. Scope: besides the lease, the diagnosis required two more rules (head-of-lane queueing, stop line before
   the crossing). The lease alone left 53-62 s stands (walkers) and follower head-of-line blocking.
2. The lease lapses only before the stop line (task text: "no progress along its connector"): a holder
   already on the crosswalk / in the box keeps it, for the reason above. Logged as a decision.
3. **AC2 as written ("no car stopped > 20 s except in a queue behind a moving leader") is NOT met and not
   asserted.** After the fix there is no gridlock, but the junction in front of a standing player saturates:
   18-19 cars per 120 s stand 20-39 s in moving queues (seed 7: none). Measured cause: the in-view junction
   passes ~11 cars/min, 18 of 22 grants start from rest with a mean box hold of 8.1 s (IDM a = 0.73 m/s²
   from a standstill), versus 2-3.5 s holds for cars that arrive moving; the Vermeij bubble keeps ~15 of 22
   cars in frame, so the queue refills. A "queue behind a moving leader" excuse also cannot tell a lease
   livelock (grant, stand, lapse, re-queue) from such a queue, so the gate asserts the gridlock bound and
   prints the > 20 s stands. Levers probed (seeds 1/42 unless noted): `max_cars` 16 → 38 s; `turn_speed` 8 →
   no change; no civilians → 33 s; `idm.acceleration` 1.5 → seeds 1 and 7 clean, seed 2 34 s, seed 42 28 s.
   None applied: the IDM start values and the bubble are GDD §5.2 data, an owner/orchestrator decision.
   Options: raise `idm.acceleration` (city start-up), pace steady spawns in the bubble, or a junction model
   with a shorter exclusive hold (e.g. signal phases).
4. **Runtime AC ("baseline on seed 1 shows no jam") partially met.** `scratch/baseline_s1/` (the
   playtester's `baseline.py`, release `--features dev`, `--settings-id .qa` via `tools/qa/brp.py`): no
   gridlock: worst stuck car 28.1 s (was 109.5 s, 19 cars frozen from t = 29 s), stuck civilians 0 (was 8,
   up to 94 s at the crossing). But `stopped_cars` stays 6-18 (series in `samples.json`: 0, 0, 1, 3, 7, 8,
   9, 10, 11, 11, 10, 10, 8, 7, 9, 10, 11, 13, 14, 14, 15, 14, 13, 11, 12, 14, 15, 18, 15, 14, 13, 14, 14,
   12, 10, 8, 6, 6, 8, 9, 10, 11, 12, 10, 10, 10, 10, 8, 6, 6, 8, 9) and the harness's jam oracle (>= 3 cars
   stopped >= 10 s within 25 m) fires 27 times: slow-moving queues at node 83, the same saturation as item 3.
   The owner will likely still read this as a queue in front of the spawn.
5. No permanent "RED before the fix" test: the head rule and the lease cannot be switched off from data
   without also moving the gate's own bound; the reproduction numbers are recorded above.

## Test results

- `cargo test -j 2 -p gta_sim -p citygen`: 515 passed, 0 failed, 5 ignored (8 min 20 s; per-binary lines in
  `scratch/test_sim_citygen.txt`). Existing traffic/police gates green (traffic_intersection, traffic_*,
  police_cars 12/12, ...).
- `traffic_bench`: mean 1.63 ms/tick (limit 19 ms; TASK-016 probe 1.86 ms).
- `cargo clippy -j 2 -p gta_sim -p citygen --all-targets -- -D warnings`: clean.
- `cargo clippy -j 2 --workspace --all-targets -- -D warnings`: clean.
- `cargo test -j 2 -p gta_like --bin gta_like`: 81 passed.
- `python tools/qa/tree_check.py`: passed.
- CI: not yet observed (branch pushed; the orchestrator watches the merge run).

## How to verify manually

- `cargo test -p gta_sim --test traffic_gridlock -- --nocapture`: per seed it prints average cars, grants,
  worst stand, every stand > 20 s and the worst stale grant.
- Runtime: `python maw/tasks/in_progress/TASK-031/scratch/tools/baseline.py <out> 1`, then read
  `summary.json` (`stuck_car`, `stuck_npc`, `jams`) and the `stopped_cars` series in `samples.json`.
- Owner eye: `cargo run --release -- --seed 1`, stand at the spawn looking north for 2 minutes. Expected: the
  junction at (−2, −80) keeps discharging (no frozen row of cars, pedestrians cross in front of waiting
  cars), but queues of several cars form and move slowly — decide on item 3.

Probes and logs (all in `scratch/`): `probe_gridlock_diag.rs` (timelines, junction dumps, crossing
geometry), `probe_gridlock_measure.rs` (per-seed series, grant hold times, lever probes), `probe_*.log`,
`flip_*.txt`, `baseline_s1/`, `test_sim_citygen.txt`. Nothing launched is left running.
