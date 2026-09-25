# FIX_SUMMARY — TASK-016 fixer round 4 (option B: spawn filter + police pressure)

**Status: items 1-3 done. t15 is 1/5, so per the note I report the failing trace and do not patch further
(section 4).** All sim/client gates, clippy and t11 are green.

Code commit `d96757d` (filter, gates, t15). This summary and the log entries are in the next commit.

Inputs: orchestrator note round 4 (binding: last `OPEN_DECISIONS.md` entry, option B); `FIX_SUMMARY.prev-3.md`;
`IMPL_REVIEW.md` (round-1 review, handled in rounds 1-3, nothing new to act on).

## 0. Preflight
- `scratch/` read as a coverage map only (round-3 `fixer3/` traces and flips). My evidence is in `scratch/fixer4/`.
- **The claim that breaks correct code if applied verbatim:** the literal option B, "police cars spawn only on
  lane stretches with no AI traffic between the spawn point and the player", applied to every dispatch.
  I implemented it that way first and ran the full sim suite (`scratch/fixer4/test_sim_literal_b.log`):
  8 gates red, among them O2 (binding):
  - `police_stopped_driver::a_stopped_driver_is_approached_and_busted`: seed 1 park and seed 3 hospital "no police
    car let its crew out", seed 2 park not busted in 24.8 s;
  - `police_stopped_driver::stop_and_go_does_not_make_crews_hop`: seed 3 "no police car came";
  - `police_cars::cars_follow_row_2..5`, `cars_cap_binds_with_spare_units`, `dead_crew_frees_the_slot`: the row
    never gets its cars.
  - Probe (`scratch/fixer4/probe_literal_b_row2.log`, hospital, 2 stars): 7 ring points face the player, 6 of
    them are behind traffic; the one clear point is taken by the first car. With the F6 seat reservation a
    missing car also keeps its crew's foot slots empty, so fewer cops come at all.
  - Why: every route ends on the lanes by the player. A stopped driver sits in a queue, so every approach to
    him is "behind traffic"; at the hospital the only ring points facing the player are in queues too.
  - For a stopped driver or a player on foot, spawning behind a queue is fine: the blocked rule lets the crew
    out and they walk (O2 12/12 in round 3). The round-3 failure was the MOVING driver (pursuit).
- **Recomputed prescription:** the filter applies while the player drives a moving car (speed above
  `exit_max_speed`, the same "moving" test `drive_police_cars` uses). Strict there, no fallback. Foot and
  stopped-driver dispatch is unchanged, so "everything else stays as it is" holds. Logged as a `decision`.

## 1. Fixed

**Item 1: police cars never spawn behind a traffic queue in a pursuit**
- `police/car_route.rs`:
  - `lane_costs_to(graph, goal, radius)`: one reverse Dijkstra over the lane graph with the costs and goal test
    of `find_lane_route` (goal lanes within `nearest + goal_margin` of the player);
  - `approach_clear(graph, costs, traffic, (lane, s), goal, radius)`: walks the cheapest route from the spawn
    point (the route A* takes, up to ties) and fails on any AI traffic car (`TrafficCar::is_ai`, segment and s:
    the existing traffic occupancy data) ahead on the start lane, on every connector and lane of the route, and
    on the goal lane up to the player's projection.
- `police/car_dispatch.rs dispatch_police_cars`: `pursuit` = player `Driving` and his car faster than
  `exit_max_speed`. In a pursuit a candidate must pass `approach_clear`.
  - Bound per tick: one reverse search (computed lazily, only when a car is missing and a pursuit is on) and
    one route walk per candidate (at most the lane count of steps).
- Gates:
  - `police_car_floor::police_car_never_spawns_behind_a_traffic_queue`: the player drives lane 0 of the loop at
    8 m/s, a traffic car follows 10 m behind; every ring point facing him lies behind it. No police car in 64
    ticks. Then the traffic car is removed (named mutation `max_cars = 0`, no replacement) and a car spawns
    within 16 ticks (GATE BROKEN otherwise; also GATE BROKEN if the player's car slows to exit speed).
  - Unit rows `police::car_route::tests::approach_clear_rows` (square loop): clear, car behind the start, car
    ahead on the start lane, car on the connector, car on the middle lane, car before the goal projection, car
    past the goal projection.

| flip | result |
|---|---|
| filter off (`if false && pursuit`) | RED: tick 0, police car spawned at x = -29 behind the traffic car at x = 0 (`scratch/fixer4/flip_no_approach_check.log`) |
| connector check off | RED: row `(Connector(0), 1.0)` clear (`scratch/fixer4/flip_no_connector_check.log`) |
| scoping off (filter on every dispatch) | RED: the 8 gates in section 0 (`scratch/fixer4/test_sim_literal_b.log`) |

**Item 2: t15 chase checks police pressure**
- `tools/qa/scenarios/t15.py` step 3: every 0.5 s the nearest active police car and the nearest live
  `PoliceUnit` (crews included). Pass when either is within `PRESSURE_M` = 18 m of the player within N s; the
  kind that got there first, the time and the distance go to `summary.chase.pressure`.
- N from data (`pressure_seconds`): `car.spawn_ring.1 / car.pursuit_speed + car.blocked_seconds ×
  car.junction_factor + car.stopped_seconds + (car.spawn_ring.1 − 18) / run_speed` = 3 + 6 + 1 + 9.33 =
  **19.33 s**; GATE BROKEN if it exceeds the 25 s chase window.
- The "never more than 2 active police cars" check stays. "A car closes in" is gone.

## 2. Skipped
- Nothing else from the orchestrator note. `IMPL_REVIEW.md`: all items were handled in rounds 1-3.
- `maw/tasks/pending/TASK-031/task.md` has an uncommitted change that is not mine; left untouched and not
  committed.

## 3. Test results

| command | result |
|---|---|
| `cargo test -p gta_sim -j 2 --no-fail-fast` | **58 targets, 464 passed, 0 failed, 2 ignored** (`scratch/fixer4/test_sim_full.log`) |
| `cargo clippy --workspace --all-targets -j 2 -- -D warnings` | clean (`scratch/fixer4/clippy.log`) |
| `cargo test -p gta_like --bin gta_like -j 2` ×3 | 79/79 each (`scratch/fixer4/test_client_{1,2,3}.log`) |
| `cargo build -p gta_like --bin gta_like -j 2 --features dev --release` | ok (`scratch/fixer4/build_release.log`) |
| `python tools/qa/scenarios/t15.py` ×5 | **1/5 PASS** (section 4; `scratch/fixer4/t15_run{1..5}/summary.json`) |
| `python tools/qa/scenarios/t11.py` | PASS (exit 0), no log errors, no-vsync frame cost 3.06 ms (`scratch/fixer4/t11/summary.json`) |
| `rustfmt --edition 2024` on the edited files | only my hunks |

- Hijack and "≤ 2 active cars" passed in all 5 t15 runs; 30 traffic cars, mean speed about 8 m/s.
- No game process left running.
- Push: `6f06d32` on `feature/t15-traffic`. Branch CI at that commit: all 5 jobs green (sim gates
  36129280861, client gates, clippy, citygen gates, repo checks).

## 4. t15 failing trace (not patched further)

| run | result | pressure |
|---|---|---|
| 1 | FAIL | none |
| 2 | PASS | car, 16.2 m at 2.6 s |
| 3 | FAIL | none |
| 4 | FAIL | none |
| 5 | FAIL | none |

The four failures look the same. Run 1 (`t15_run1/summary.json`), nearest active police car / nearest live cop, m:

| t, s | 0 | 2.7 | 5.2 | 7.8 | 10.5 | 13.1 | 15.8 | 19.9 | 24.1 |
|---|---|---|---|---|---|---|---|---|---|
| car | 30.2 | 40.2 | 49.8 | 56.6 | 74.9 | 86.4 | 93.3 | 96.9 | 97.2 |
| foot | - | - | - | - | - | - | 91.3 | 87.7 | 80.9 |

- Both police cars are already out at the first sample, 30-31 m away. The chase loop sends W and samples in the
  same iteration, so they spawned during the 2-star raise, before the first burst.
- At that moment the hijacked car stands, below `exit_max_speed`. By the scoping in section 0 the filter is off,
  and the traffic that queued behind the stopped hijacked car is exactly the queue the cars spawn behind.
- Then the player drives off, the cars recede to ~100 m, and their crews get out ~90 m back at 15.8 s. This is
  the round-3 failure. It is moved to the moment of the 2-star raise and not fixed.
- This is a direct conflict:
  - the filter that would catch the spawn (on while the player sits in any car) is the one that took O2 away in
    jams (section 0: stopped-driver busted seeds 1-3, stop-and-go seed 3);
  - a stopped driver in traffic is surrounded by queues.
- Options for the orchestrator, none implemented:
  - (a) Filter while `Driving` at any speed. Accept that a driver stuck in a jam gets fewer or later cops, and
    re-anchor the O2 stopped-driver gate to lanes without traffic behind the player.
  - (b) Keep this scoping and change t15 so the chase starts once the player's car moves (heat raised after the
    first burst). That tests pursuit spawns, not spawns at a standstill.
  - (c) Proposal A from round 3 (cars as crew transport, no pursuit criterion).

## 5. Owner checklist (after this round)
- 2 звезды, едете в потоке: новые полицейские машины не появляются позади вас в той же пробке; если все подъезды к
  вам забиты трафиком, машин нет, пока вы не остановитесь.
- Стоите в угнанной машине в пробке на 1 звезде: копы по-прежнему подъезжают сзади, выходят и вытаскивают вас
  (как в раунде 3).
- Известное ограничение: если звёзды получены, пока машина стоит, полиция может появиться позади вашей пробки и
  отстать, когда вы тронетесь (раздел 4).

children: 0 launched / 0 reported.
