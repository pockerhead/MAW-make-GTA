# FIX_SUMMARY — TASK-016 fixer round 3 (QA round 2 bugs, orchestrator items 1-7)

**Status: items 1-7 done. The STOP rule is triggered by the chase.**
- The 1★ pull-out now passes on seeds 1-3 at hospital, plaza centre, the tower sidewalk (QA's plaza) and the
  park: 12 of 12 cases BUSTED in 8.8-17.3 s. The data-derived limit is 24.8 s.
- The chase step of `t15.py` still fails 3 of 5 runs. The cause is the car model: police cars cannot pass
  traffic. Per the STOP rule I did not patch it further. Section 4 gives the failing case and proposes a
  simpler model.

Code commits: `c075807` (fixes and gates) and `e422ac8` (rustfmt). Docs and this summary are in the next commit.

Inputs:
- `QA_REPORT.prev-2.md` and orchestrator items 1-7 (binding);
- `FIX_SUMMARY.prev-2.md`;
- `IMPL_REVIEW.md` (round-1 review, already handled in rounds 1-2: nothing new in it to act on).

## 0. Preflight
- I read `scratch/` as a coverage map only: QA's `qa2_stopped.rs` probe and the traces `trace_s2_hospital.log`
  and `trace_s3_plaza_5.log`. All my evidence is new and lives in `scratch/fixer3/`.
- **The claim that would fail if applied verbatim:** item 3, bullet 1: "once a cop in Arrest is within an
  approach radius, it seeks the door directly without LOS".
  - `scratch/qa2/trace_s2_hospital.log` shows the stalled cops in `Respond` with `sees=false`. They never
    reach `Arrest`.
  - They already seek directly: the destination is clear of walls and within 25 m.
  - What stops them is a car body. `avoid_offset` probes the World layer only (`navigation/mod.rs`), so it
    never sees the car.
  - Applied as written, the rule would change nothing on seeds 2-3.
  - So the fix is to the transition (near the door, no LOS needed) and to the walk (around cars). See item 3.
- **QA 3(b), second half,** suggests re-raising heat every sample in t15 so that `last_known` tracks the player.
  I rejected it: it would mask a game-side failure.

## 1. Fixed

**Item 1: red gate `vehicle_seat::no_arrest_in_a_car` re-anchored**
- It now guards what must stay impossible. A cop is held in `Arrest` at the driver's door (named mutation) in
  two rows:
  - 1★ with the car kept at 5 m/s;
  - 2★ with the car at rest.
- Over `pull_out + give_up + arrest + 1` s it asserts, every tick: no BUSTED, still `Driving`, no bound attempt.
- Flips:

| flip | result |
|---|---|
| drop `Without<Driving>` from `arrest_player` | RED: attempt bound at tick 0 |
| drop the speed check in `pull_out_driver` | RED: pulled out at tick 62 |

- The 2★ row is protected twice: by `next_state` (Arrest becomes Attack off an arrest row) and by the
  `arrests` check in `pull_out_driver`. A single flip of either stays green. Say so if a single-flip row is
  wanted; `police_pull_out::no_pull_at_two_stars` also covers 2★.

**Item 2: police cars crawled from rest**
- New `vehicle::follow_speed(cfg, v, a, dt)` holds the traffic bug-4b law, which was inline in
  `traffic/drive.rs`:
  - accelerating: `v + a/(acceleration·speed_gain)`;
  - braking: `(v + a·dt).max(0)`.
- `drive.rs` and `police/car_route.rs:510` both use it.
- Gate `police_car_floor::police_car_pulls_away_behind_a_leader`:
  - setup: a responding car on a 54 m lane, a traffic car pulling away 15 m ahead;
  - rule: top speed within 4 s must be at least `0.5·idm.acceleration·4 = 1.46` m/s;
  - named mutation: `blocked_seconds = 1e6`, because the blocked rule would stop the drive under test after 2 s;
  - GATE BROKEN if the leader leaves the cast.
- Result: 2.50 m/s. Flip to the old `v + a·dt`: 0.10 m/s, RED.

**Item 3: O2 pull-out in the city**
- `CopSenses.near_driver`: the player drives and his door is within new data `arrest.approach_distance`
  (20 m).
  - On an arrest row, Respond/Search go to `Arrest` without sight.
  - `Arrest` stays while `sees || near_driver`.
  - `next_state_table` gains this dimension on every row.
- Walking in `Arrest`:
  - direct while `sees || dest_clear` (no wall in the way);
  - via `tactics::around_cars` (new, next to `car_blocks`). If no car lies within the capsule radius of the
    straight line, the cop walks straight. Otherwise it heads for the corner, `radius + arrive_radius` out, of
    the first car in the way that is in plain view and shortest to go via.
  - This applies to arrests on foot too.
  - Unit rows in `fire_line.rs::around_cars_rows`.
- The arresting cop now aims at (faces) the door, not the seated driver.
  - Trace `scratch/fixer3/trace_s3_hospital.log`: the puller aimed into the car, so the pulled-out player 1 m
    away was outside its 110° cone.
  - It went Arrest → Respond → Search and the arrest dropped.
- Gate `police_stopped_driver::a_stopped_driver_is_approached_and_busted`:
  - now seeds 1-3 × hospital / plaza centre / tower sidewalk (QA's plaza) / park;
  - BUSTED within `busted_within` from data: ring 60/20 + blocked 2 + stopped 1 + 60/run 4.5 + pull 1 + give-up 3
    + arrest 1.5 = 24.8 s;
  - every case is reported, not just the first failure.

| seed | hospital | plaza | tower | park |
|---|---|---|---|---|
| 1 | 8.83 | 9.95 | 9.92 | 12.70 |
| 2 | 11.67 | 17.19 | 17.31 | 10.61 |
| 3 | 11.17 | 12.44 | 12.44 | 11.38 |

(seconds to BUSTED)

- Flips:

| flip | result |
|---|---|
| `near_driver: false` | RED, 9 of 12 cases never busted (seeds 1-3) |
| `around_cars` → straight | RED, seed 2 at hospital, plaza, tower |
| aim at the seat | GREEN, seed 3 hospital 14.4 s |
| detour only while driving | GREEN, seed 3 hospital 11.2 s |

  - My first build had both of the last two changes off, and seed 3 hospital was never busted
    (`scratch/fixer3/stopped1.log`). Each change alone rescues that case, so they are two independent defenses.
- QA's own probe spot definitions are covered: the tower row is QA's plaza.

**Item 4: stop-and-go thrash**
- New data `car.moving_seconds` (2.0).
- `PoliceCar.moving` accumulates while the player's car is above exit speed.
- `driven_off` = `driving && moving >= moving_seconds && distance > reboard_distance`. It is used at all five
  sites unchanged.
- `next_car_state`: for a driver, every reason to get out needs `stopped >= stopped_seconds`.
- Table rows added, plus unit test `driven_off_needs_moving_seconds`.
- Gate `police_stopped_driver::stop_and_go_does_not_make_crews_hop`:
  - script: QA's (hospital lane, 2★, throttle 1.5 s / coast 1.5 s, 40 s) on seeds 2 and 3;
  - rule: at most 2 Respond↔Dismounted transitions per car (K = one out and one back);
  - GATE BROKEN if the player's car never moved or no police car came.
- Result: 1 transition per car on both seeds. Flip to the old `driven_off` and old dismount rule: seed 3 has 5
  per car, RED.

**Item 5: the give-up never pulls onto the roof**
- `pull_out(.., any_exit)` now takes the left door, or with `any_exit` either door, never `ROOF_EXIT`.
- Gate `police_pull_out::a_give_up_never_pulls_onto_the_roof`:
  - setup: 1 m walls off both doors (the cop sees over them);
  - rule: never pulled out and heat unchanged for give-up + 10 s;
  - GATE BROKEN if the pull never reached the give-up.
- Flip to the old find: pulled out at tick 254 with feet at 2.08 m (the roof), RED.

**Item 6: a police car stuck in a junction box**
- New data `car.junction_factor` (3.0, validated finite and at least 1).
- `CarSenses.stuck_in_junction = blocked >= blocked_seconds × junction_factor` allows a dismount inside the box.
- Gate `police_car_floor::police_car_stuck_in_an_intersection_lets_its_crew_out`:
  - setup: the car is held mid-box by a car stopped on its exit lane;
  - result: the crew gets out after 383 ticks (rule: 384), not before, and is outside.
- Flip `stuck_in_junction: false`: it never gets out, RED.
- `a_driver_stuck_at_a_wall_is_approached`: one 5★ car now waits 6.00 s inside a box, exactly the new rule.
  - `RespondWait` now keeps box waits separate, with limit `blocked × factor + 1`. The limit outside boxes is
    unchanged (`blocked + 1`).
  - This follows the new data rule and does not loosen the old one.

**Item 7: t15 hijack**
- The stand point is now in front of the bumper on the driver's side, inside the car's own lane: local
  `(door.x + 0.5, 0, -(chassis_half_z + capsule_radius + 0.25))`.
  - It is 2.34 m from the door point, below `enter_radius` 2.5; GATE BROKEN if that ever changes.
  - The car's forward cast keeps it standing for the player.
  - Oncoming traffic passes 0.55 m clear.
- The script asserts the target car stands (≤ 0.5 m/s) right before F.
- The literal curb side is out of reach: the door is on the left and is 3.4 m from the right side.
- Result: hijack passed 7 of 7 runs (5 official + 2 instrumented), speed before F 0.0 every time.

**Config sabotage rows** in `config_traffic.rs` for `approach_distance`, `moving_seconds` and `junction_factor`,
each with a distinct error text.

**Docs**
- `docs/architecture/traffic.md`: hysteresis, junction factor, follow speed, the no-passing limit, the unseen
  approach and walk around cars, no roof.
- GDD §6.4: one line on the give-up doors and the approach.
- Comments in `escalation.ron`.

**Log:** 2 `decision` entries and 1 `dead_end` (the STOP). **PCTX:** 2 proposals: walls-only avoidance vs
cars, and aim turns the body.

## 2. Skipped / STOPPED

- **The t15 chase, per the STOP rule.** Official runs: **2/5 PASS**.

| run | result | traffic cars | mean speed | chase first → closest | frame cost no-vsync |
|---|---|---|---|---|---|
| 1 | PASS | 30 | 8.0 | 32.4 → 5.9 m | 2.96 ms |
| 2 | FAIL | 30 | 8.0 | 31.7 → 31.7 m | |
| 3 | PASS | 19 | 8.3 | 38.3 → 4.8 m | 3.10 ms |
| 4 | FAIL | 30 | 7.9 | 30.9 → 30.9 m | |
| 5 | FAIL | 29 | 7.8 | 30.8 → 30.8 m | |

  - Every failure is "the police cars never closed in". Hijack and the ≤ 2 active cars check passed in all 5.
  - Failing case (`scratch/fixer3/t15d_1/summary.json`, the chase trace; `t15_run4/chase_2.png` shows both
    police dots behind the player in his lane):
    - Both cars spawn 31 and 41 m behind the player, in the traffic queue on his lane.
    - Since item 2 they no longer crawl. They reach 7-8 m/s, but only at the pace of the traffic ahead: no
      lane change, and go-around was STOPPED in round 1.
    - The queue hides the player: `last_known` falls 5 → 100 m behind.
    - The cars drive to the stale `last_known`, stop there, and let their crews out ~100 m away when the player
      stops.
  - This is the model limit QA named in 3(b). No further patch.
- **QA 3(b), "re-raise heat each sample" in t15:** rejected. It would hide the game-side failure.

## 3. Test results

| command | result |
|---|---|
| `cargo test -p gta_sim -j 2 --no-fail-fast` | **58 targets, 462 passed, 0 failed**, 2 ignored (the existing `city_startup_budget` and a hand-run flip). Log: `scratch/fixer3/test_sim_full.log` |
| `cargo test -p gta_sim --lib` | 84 passed |
| `cargo test -p citygen -j 2` | green |
| `cargo clippy --workspace --all-targets -j 2 -- -D warnings` | clean (`scratch/fixer3/clippy.log`) |
| `cargo test -p gta_like --bin gta_like -j 2` ×3 | 79/79 each run |
| `cargo build -p gta_like --bin gta_like -j 2 --features dev --release` | ok |
| `python tools/qa/scenarios/t15.py` ×5 | **2/5 PASS** (section 2) |
| `python tools/qa/scenarios/t11.py` | PASS, no log errors, no-vsync frame cost 3.2 ms |
| `rustfmt --edition 2024` on edited files | only my hunks changed |

- All flips run through `scratch/fixer3/flip.py`: apply the sabotage, run the gate, `git checkout` the file.
  The logs are `flip_*.log`, and each restore was verified clean.
- The existing flips from rounds 1-2 were not re-run.
- File sizes: `police/cars.rs` is 716 lines, under 750.
- No game process was left running.
- Push: `306594b` on `feature/t15-traffic`. Branch CI at that commit: all 5 jobs green (sim gates run
  36123027280, client gates, clippy, citygen gates, repo checks).

## 4. STOP: proposed simpler police-car model (for the orchestrator)

**The failing case:** a police car spawned behind the traffic queue on the player's lane cannot pass it. It
loses sight, routes to a stale `last_known`, and never closes in. This is seed 1 (t15), 3 of 5 runs.

**Proposal A, recommended: cars as crew transport.**
- Drop `Chase` and the pursuit of a moving driver.
- A police car drives (Respond, A*) to `last_known`, lets its crew out there or where it is held up, and waits.
  Crews re-board and re-route when `last_known` moves by more than `reboard_distance` (the round-3 hysteresis
  stays).
- The t15 chase check becomes: while the player drives, a car lets its crew out within `dismount_distance` of a
  `last_known`, and the foot units stay ≤ row.
- Keep the 1★ pull-out: it passes 12/12 now and O2 is binding.
- The orchestrator's example, "all arrests on foot", would drop O2, so I do not recommend that half.
- Cost: less code (Chase, the direct-chase distance and the ram branch go away).
- Loss: no rams, which is owner-visible.

**Proposal B, cheaper, keeps the chase: spawn filter.**
- The car dispatcher skips spawn points whose lane stretch ahead to the player's position holds AI traffic, so
  a car never starts inside the queue it cannot pass.
- It is one predicate over the traffic occupancy that already exists.
- It does not help once traffic fills in ahead of the car later.

**Not recommended: passing or "traffic yields to sirens".**
- It needs an oncoming-lane reservation that kinematic oncoming cars honour (round-1 STOP).
- The risk is silent pass-through.

## 5. Owner checklist (after this round)

- 1 звезда, стоите в угнанной машине в пробке: копы подходят к левой двери, обходя машины сзади, вытаскивают и
  арестовывают (seed 1, 2, 3; у больницы, на площади, в парке).
- Стоп-энд-гоу на 2 звёздах: экипажи не прыгают из машин туда-обратно; выходят, только когда вы постояли ≥ 1 с,
  садятся обратно, когда вы едете ≥ 2 с.
- Полицейская машина, застрявшая посреди перекрёстка: через ~6 с экипаж выходит прямо там.
- Погоня: если полиция появилась сзади в том же потоке, она не может обогнать трафик и отстаёт (известное
  ограничение, см. раздел 4). Ждём решения оркестратора.
- Коп при аресте в машине смотрит на дверь, а не в салон.

children: 0 launched / 0 reported.
