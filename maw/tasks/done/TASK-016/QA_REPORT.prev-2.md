# QA_REPORT — TASK-016 round 2 (GDD T15: traffic and police cars)

QA: claude opus (medium). Commit under test: `b944403` on `feature/t15-traffic`. The code is the same as
`79fabe8`, the fixer's UNVERIFIED round-2 checkpoint. Cost of error: HIGH (police FSM, pull-out, traffic
kinematics). Full layer.

## 0. Preflight and disconfirmation

- I read `scratch/` first, as a coverage map only. It holds the reviewer's `cr_probe`, the fixer's
  probes (`fixer/`, `fixer2/`, where `test_sim_1.log` stops at "Compiling") and QA round 1 (`qa/`). All my
  evidence is new, in `scratch/qa2/`. From round 1 I reused one of my own role's probes,
  `qa_traffic_health.rs`, copied as `qa2_traffic_health.rs` with a seed-3 row added, so the numbers
  compare before and after.
- I read TASK_FINAL, PLAN_FINAL, IMPL_SUMMARY, IMPL_REVIEW, FIX_SUMMARY (round 2), QA_REPORT.prev-1 and
  OPEN_DECISIONS, the round-2 source diff `c5fe88b..79fabe8`, and every `dead_end` entry in the log.
- **Counter-example, written before testing:** "the new `driven_off` sets `stopped` to 0 whenever the
  player's car goes above 3 m/s. In t15's 1.5 s throttle / 1.5 s coast rhythm, or any stop-and-go, the
  player's car sits at ≤ 3 m/s for ≥ 1 s. Every responding car that is `blocked` or `route_done` then drops
  its crew far from the player. When he drives on, they re-board. The result is a slow
  dismount/re-board loop, and the cars never close in."
- **Result: it held.**
  - Headless stop-and-go at 2★, seed 3: 7 Respond→Dismounted at **132-145 m**, 4 re-boards, 14 crew
    spawns in 25 s.
  - Runtime `t15c_1`: Dismounted at 100 m → Respond → Dismounted again within 3 s.
  - It is not the main cause of the t15 chase failure; that is bug 3. Details in bugs 3 and 4.

## 1. Environment

- Direct run, no docker or services. Host: `\\.\DISPLAY1`, 144 Hz, Fifo, 9.8 GB free at start.
- Other projects' cargo processes were running and I left them alone. Every cargo command ran on its own,
  in the foreground, with `-j 2`.
- Runtime: release `--features dev`, prebuilt with `-j 2`, so `brp.py`'s own build was a no-op.
  `Game` always runs with `--settings-id com.github.pockerhead.maw-make-gta.qa`.
- Probe crate `scratch/qa2/probe/`: the round-1 manifest, a copy of the workspace `Cargo.lock`, and a
  path-patched copy of `tests/common`. It builds into the shared `target/` (`CARGO_TARGET_DIR`), not a
  second one.

Reproduce:
```
cargo test -p gta_sim -j 2 --no-fail-fast ; cargo test -p citygen -j 2
cargo clippy --workspace --all-targets -j 2 -- -D warnings
cargo test -p gta_like --bin gta_like -j 2          (x3)
cargo build -p gta_like --bin gta_like -j 2 --features dev --release
python tools/qa/scenarios/t15.py --out <dir>        (x5) ; python tools/qa/scenarios/t11.py --out <dir>
python maw/tasks/in_progress/TASK-016/scratch/qa2/t15_chase_diag.py --out <dir>   (instrumented t15 copy)
python maw/tasks/in_progress/TASK-016/scratch/qa2/hijack_diag.py <dir> 12
cd maw/tasks/in_progress/TASK-016/scratch/qa2/probe
CARGO_TARGET_DIR=D:/test-gta-like/target QA_SEEDS=1,2,3 cargo test -j 2 --test qa2_stopped -- --nocapture
CARGO_TARGET_DIR=D:/test-gta-like/target QA_CASE=2,0 cargo test -j 2 --test qa2_stopped trace_one_star -- --nocapture
CARGO_TARGET_DIR=D:/test-gta-like/target cargo test -j 2 --test qa2_traffic_health -- --nocapture --test-threads=2
```

## 2. Test results

**Existing suites at `b944403`**

| command | result |
|---|---|
| `cargo test -p gta_sim -j 2 --no-fail-fast` | 57 targets, 455 passed, **1 failed: `vehicle_seat::no_arrest_in_a_car`** (`scratch/qa2/test_sim.log`) |
| same test rerun | 3/3 more runs fail: deterministic |
| branch CI `36112463048` (sim gates) | **failure**, same test and panic. The other 4 jobs are green |
| `cargo test -p citygen -j 2` | green |
| `cargo clippy --workspace --all-targets -j 2 -- -D warnings` | clean |
| `cargo test -p gta_like --bin gta_like -j 2` ×3 | 79/79 each time |

About `no_arrest_in_a_car`:
- The test (TASK-015) puts a cop in `Arrest` at the driver's door at 1★ and asserts that no
  `ArrestAttempt` is ever bound.
- Round 2's `pullers` exclusion now lets that cop pull the stopped driver out (O2), which binds the
  attempt.
- My flip `&pullers → &[]` makes it green again. So it only ever passed because the cop's own body
  blocked the left-door spot, which is QA bug 2 of round 1.
- This is a stale gate, not a game bug. It has to be re-anchored, for example to a moving car or to 2★,
  and until then the suite is red.

**Flip-RED done by me** (each file restored with `git checkout`, sha256 `OK`)

| flip | gate | result |
|---|---|---|
| `driven_off` → `driving && distance > reboard_distance` (the round-1 rule) | `police_stopped_driver` | both rows **RED** |
| `pull_out(.., &pullers, ..)` → `&[]` | `police_pull_out::a_second_arresting_cop_at_the_door_does_not_block_the_pull` | **RED**; the other 5 rows green |
| runtime only: police IDM target `v + a/(acceleration·speed_gain)` | t15 chase | failing geometry 0/3 → 2/4 pass (bug 3) |

**New probes (headless, production composition, `scratch/qa2/`)**

`qa2_stopped::stopped_driver_table`: the player sits in a stopped car on the lane nearest each spot, on
seeds 1-3 (`stopped_table.log`).

| seed | spot | 1★: pulled out / Busted | 5★: cars dismounted / worst Respond wait > 25 m |
|---|---|---|---|
| 1 | hospital | 7.3 s left door / 8.8 s | 5/5, 1.98 s |
| 1 | plaza | 8.4 s / 10.0 s | 5/5, 2.00 s |
| 1 | park | 11.5 s / 13.0 s | 5/5, 2.00 s |
| 2 | hospital | **never in 60 s** | 3/3, 2.00 s |
| 2 | plaza | **never in 60 s** | 5/5, 2.00 s |
| 2 | park | 9.0 s / 10.5 s | 5/5, 2.00 s |
| 3 | hospital | **never in 60 s** | 4/4, 2.00 s |
| 3 | plaza | **never in 60 s** | **4/5, one car 26.5 s at 41.8 m** |
| 3 | park | 10.5 s / 12.0 s | 5/5, 2.00 s |

- Bug-1 metric: at 1★ no car waits more than 2.0 s in Respond beyond 25 m, so bug 1 as filed is fixed.
- Every pull-out went through the left door, local x ≈ −1.70, with no attempt reset.
- The four 1★ failures are a different defect (bug 2 below).

`qa2_traffic_health`: 5 min roams on seeds 1, 2 and 3 at 0★, plus 3 min at 3★ (`traffic_health.log`).

| check | result |
|---|---|
| kinematic pass-through (penetration > 2 cm) | **0** pairs |
| dynamic-dynamic overlap | 0 |
| traffic → parked car contacts | 0 |
| longest AI stop on a connector | **7.6 s** (round 1: 13.3 s); 5 stops over 3 s, all behind crowds of civilians on a crosswalk |
| longest junction wait | 28.9 s; no car > 30 s |
| entity count | flat (1.7-2.05 k) |
| max cars | 24 traffic, 3 police cars at 3★ |

So the fixer's traffic acceleration change did not cause pass-through, and the stand-offs got shorter.

`qa2_stopped::stop_and_go_transitions`: 2★, 1.5 s throttle / 1.5 s coast, 40 s (`stop_and_go.log`).
- Seed 2: 2 dismounts at 87-106 m.
- Seed 3: 7 dismounts at 132-145 m and 4 re-boards (bug 4).

**Runtime (release, BRP)**

`t15.py` ×5 (`scratch/qa2/t15_run*`): **2/5 PASS**.

| run | result |
|---|---|
| 1 | PASS: first 35.4 m → closest 11.8 m |
| 2 | **FAIL**: "the police cars never closed in", first = closest = 31.4 m |
| 3 | **FAIL**: hijack, "Driving the traffic car not reached in 1.0 s" |
| 4 | **FAIL**: hijack, same error |
| 5 | PASS: 38.3 → 4.4 m |

- On the passing runs: traffic 30 cars, mean 7.9-8.0 m/s, max 13.9, at most 2 active cars, both cars
  Dismounted after the stop.
- Frame cost with no vsync: **2.95-3.24 ms** (144 Hz, Fifo, `frame_report`).
- Instrumented copies (`t15_diag.py`, `t15_chase_diag.py`, runs `t15d_*`, `t15c_*`) were used to
  diagnose the failures (bug 3).
- `hijack_diag.py`, 12 attempts in one session: 10 entered within 0.14-0.25 s; 2 cars never stopped for
  the player.

`t11.py` ×1: PASS (exit 0, no log errors).

Screenshots:
- `t15_run2/chase_3.png`: the player's car on the avenue, the two police dots behind on the minimap.
- `t15d_1/*`: the hijack failure.

## 3. Acceptance criteria

| criterion | test performed | result |
|---|---|---|
| IDM: no negative speed, no overlap, 10 cars, 6400 ticks | `traffic_idm` green; roams: 0 kinematic pass-through on 3 seeds | PASS |
| One car per conflict point | `traffic_intersection` green (my round-1 flip RED) | PASS |
| Despawn only after ≥ 2 s off frame | `traffic_bubble` green (round-1 flip RED) | PASS |
| Kinematic → dynamic on contact | `traffic_contact` green; roams: 0 pass-through, 0 parked contacts | PASS |
| ≤ 2 police cars at 2★ | `police_cars` green including new `cars_cap_binds_with_spare_units`; t15: max 2 active in 5/5 | PASS |
| `t15.py` exists and passes via `brp.py` | 5 runs: **2/5** | **FAIL** (bug 3) |
| Owner checklist | section 6 | recorded |
| Tuning values in data | diff scan: `pull_give_up_seconds` in `escalation.ron`, validated; no new `const` | PASS |
| build, clippy, `cargo test -p gta_sim` green | clippy clean; **sim suite 1 failure**, CI sim gates red | **FAIL** (bug 1) |
| Existing tests pass | `no_arrest_in_a_car` red | **FAIL** (bug 1) |
| O2: 1★ pull-out of a stopped driver | fixer gates green and flip RED; city probe: seed 1 3/3, **seeds 2-3 2/6** | **FAIL** (bug 2) |
| Round-1 bug 1: stopped/stuck driver > 25 m gets approached | 1★ worst wait ≤ 2.0 s in 9/9; 5★ all cars dismount in 8/9 (one car stuck in a junction box) | PASS, minor residue (bug 5) |
| Round-1 bug 2: second cop at the door, no deadlock | gate RED under my flip; no pull reset or stall in any city case | PASS |
| Round-1 bug 3: t15 hijack flake | still 2/5 | **FAIL** (bug 3) |
| Round-1 bug 4: junction stand-offs 3-13 s | now max 7.6 s, 0 pass-through | PASS |
| Round-1 bug 5: car cap gate | `cars_cap_binds_with_spare_units` green; fixer's flip reported RED | PASS (not re-flipped by me) |

## 4. Bugs found

**1. MAJOR (red suite, CI red): `vehicle_seat::no_arrest_in_a_car` fails at `b944403`.**
- **Reproduce:** `cargo test -p gta_sim --test vehicle_seat no_arrest`.
- **Actual:** `assertion left == right failed: an arrest attempt on a driver, left: Some(234v0)` at
  `vehicle_seat.rs:471`. It fails 4/4 locally and in CI run 36112463048.
- **Cause:** a stale TASK-015 gate (see §2). The cop at 1★ now pulls the stopped driver out (O2). The old
  green depended on the cop's own body blocking the left-door spot, and the flip `pullers → &[]` makes it
  green again.
- **Expected:** the gate is re-anchored to what it guards. For example: no arrest while the car moves
  above `exit_max_speed`, or at 2★.

**2. MAJOR: the O2 pull-out at 1★ still fails in the city on seeds 2 and 3 (4 of 6 spots). The car is
still a safe haven.**
- **Reproduce:**
  `QA_CASE=2,0 cargo test --test qa2_stopped trace_one_star -- --nocapture` (`trace_s2_hospital.log`).
- **What happens:**
  - The police car dismounts behind the player's stopped car at 34 m.
  - Both cops walk up the lane and stop **8.2 m** from the driver, for 38 s and more, in `Respond`,
    `sees = false`.
  - A traffic car that queued behind the player's car stands between them. It is Dynamic, 2.4 m from the
    cops and 5.9 m from the player.
  - The cops push against it: foot movement checks are walls-only, and cops have no car avoidance. The
    same car blocks their sight, so they never switch to `Arrest`.
  - `pull` stays 0 and nobody is Busted.
- **Why the gates miss it:** a stopped car in a lane always builds a queue behind it, so this is the
  common case. The fixer's `police_stopped_driver` gate runs only seed 1, where the approach comes from
  the side.
- **Expected:** a cop reaches the door (walks around the queued car) and pulls out, as on seed 1.

**3. MAJOR: `t15.py` passes 2/5 (AC6). The round-1 bug 3 fix did not work, and the chase step has a
game-side cause.**
- **(a) Hijack step, 2/5 failures.**
  - The "Steady" waits do not help. The stand point, 1.5 m out of the LEFT door, is in the oncoming inner
    lane, so oncoming traffic knocks the player down right after the Steady poll: `t15d_1`, first sample
    after F `KnockedDown {left: 1.17}`, and the player was shoved 1.56 m.
  - Or the target car, no longer blocked, drives off while the script waits: `t15d_4`, at F the car is at
    1.53 m/s and 3.9 m from the door.
  - This is a harness defect. A fix could stand at the right door side, re-put the player in the lane
    until F, or retry.
- **(b) Chase step, 1/5 official, and 7 of 9 instrumented runs whose first hijack worked.**
  - Both police cars spawn on the player's lane (299), in the traffic queue behind the hijacked car,
    about 31 m back.
  - **Game bug:** a police car accelerates from rest behind any body within the cast range at a crawl.
    - `car_route.rs:510`: `speed = min(v + a·dt)` makes the autopilot throttle `a·dt·gain` ≈ 0.006.
    - Raw trace `t15c_7`: `Autopilot.speed` 0.018 → 0.11 over 4 s, and the car moves 0.3 m.
    - This is the same defect the fixer fixed for traffic (bug 4b, `drive.rs:403-413`), but not for
      police cars.
  - While they crawl, the player leaves sight (the queue blocks LOS). The cars then route to a stale
    `last_known`, stop there and dismount about 100 m away when he stops.
  - The runtime flip of the police target to the traffic formula raised the pass rate from 0/3 to 2/4 on
    this geometry. The rest is search behaviour on a stale `last_known`. It is legitimate, but it means
    t15's "cars close in" assumption does not hold for this spawn.
- **Expected:** the police IDM follows the traffic formula. Then t15 either keeps the police in sight
  (re-raise heat each sample so `last_known` tracks the player) or accepts a spawn that is not behind a
  queue.

**4. MINOR (owner-visible, the fixer's flagged side effect): stop-and-go makes crews hop in and out far
away.**
- **Reproduce:** `qa2_stopped::stop_and_go_transitions`, seed 3.
  - 7 Respond→Dismounted at 132-145 m, 4 Dismounted→Respond, 14 crew spawns in 25 s.
  - Runtime `t15c_1` at 100 m: Dismounted (17.5 s) → Respond (19.0 s) → Dismounted (20.6 s).
- **Cause:** `blocked`/`route_done` combined with a 1 s "stopped" window counted at any distance.
- **Expected:** crews get out at a far goal only when the driver has been stopped for longer than a short
  pause, or never beyond a range. Owner or orchestrator call.

**5. MINOR: a police car stuck inside a junction box never lets its crew out.**
- Seed 3 plaza, 5★ (`trace_s3_plaza_5.log`): the car stands at 0.0 m/s in `Respond`, 41.8 m from the
  stopped player, with `blocked` rising to 34.5 s.
- `in_junction = true`, so `next_car_state` refuses Dismounted (round-1 rule "never stop in a box"). It is
  held there by the cars ahead.
- The crew stays aboard for good and the car locks that box.

**Observation (code only, not seen in runs):** after `pull_give_up_seconds`, `pull_out(.., any_exit = true)`
takes the first clear spot of `exit_spots`, which includes the roof (index 2). Crew dismount excludes the
roof; the pull-out give-up does not.

## 5. Verdict

**NEEDS_FIXES.**
- Round 2 fixed what it targeted:
  - a stopped driver is approached at every tested spot (bug 1);
  - the second-cop deadlock is gone (bug 2);
  - junction stand-offs dropped from 13.3 to 7.6 s with 0 pass-through;
  - the car cap has a gate.
- My flips confirm the new gates are falsifiable.
- The commit still fails the bar:
  - the sim suite is red, locally and in CI (bug 1, a stale gate);
  - AC6 `t15.py` passes 2/5 (bug 3: harness hijack stand point, plus the police crawl-from-rest defect);
  - O2 fails in the city on seeds 2 and 3, where cops stall behind the traffic car queued at the
    player's bumper (bug 2).
- Bugs 1 and 3(b) are small, bounded code or test changes. Bug 2 needs a decision:
  - cops walk around cars on their last metres, or
  - the pull-out/arrest counts a cop at the far side of a queued car.

## 6. Owner checklist (after the fixes)

Запуск: `python tools/fetch_assets.py`, затем `cargo run --release -- --seed 1` (и `--seed 2`).
- Трафик едет, встаёт в очередь и уступает на перекрёстках. Около одной машины за 4 с через загруженный
  перекрёсток. Иногда машина стоит до ~8 с перед толпой пешеходов на переходе.
- Угон: встаньте перед машиной, подойдите к двери, F. Водитель убегает.
- Выстрел в боковое окно машины трафика: водитель тормозит, выходит и убегает (есть только headless-гейт).
- Такси в потоке, модели и масштаб полиции и такси, высота сирены.
- Погоня на 2-5 звёздах: машины догоняют, таранят, экипаж выходит у стоящего игрока. Проверить, не
  прыгают ли копы из машин за 100+ м при коротких остановках (баг 4).
- 1 звезда, стоите в машине посреди полосы: коп подходит к двери, вытаскивает, арест. На seed 2 у
  больницы проверить, что копы не застревают за машиной в очереди (баг 2).
- Без брони на 5 звёздах в машине смерть примерно за 17 с (баланс, раунд 1).

## 7. Cleanup and git status

- No containers or services were started. Every game session ended through `Game.stop`, and
  `tasklist` shows no `gta_like`.
- The three temporary flips were restored, sha256 `OK` for `car_route.rs`, `cars.rs` and `arrest.rs`.
  After the runtime flip the release build was rebuilt from clean sources before `t11`.
- `git status --short`: only `log.jsonl` (2 appended entries) and this report. All my files are in
  `scratch/qa2/`, which is ignored.

children: 0 launched / 0 reported.
