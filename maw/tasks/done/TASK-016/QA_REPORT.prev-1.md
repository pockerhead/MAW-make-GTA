# QA_REPORT — TASK-016 (GDD T15: traffic and police cars)

QA: claude opus (medium). Commit under test: `19ebb82` on `feature/t15-traffic` (the code is the same as `47072c0`;
later commits only touch `maw/`). Cost of error: HIGH (silent jams, police FSM, pass-through). Full layer.

## 0. Preflight and disconfirmation

- I read `scratch/` as a coverage map only: the reviewer probes (`cr_probe`), the fixer probes and logs, and the
  earlier `qa_t*` runs. I did not re-run any author script as evidence. All my probes are new files in `scratch/qa/`.
- I read the spec, PLAN_FINAL, IMPL_SUMMARY, IMPL_REVIEW, FIX_SUMMARY and OPEN_DECISIONS, then triaged the
  `dead_end` log entries. The go-around STOP and the fixer's "blocked from spawn" choice are consistent with the
  code (`car_route.rs:275-279`).
- **Counter-example written down first:** "a player who sits in a STOPPED car is never dealt with by the police
  cars: the fixer's new `driven_off` sense (`car_route.rs:372`: `driving && distance > reboard_distance`, with no
  check that the car moves) blocks every `Respond -> Dismounted` (`cars.rs:158`) while the stopped driver is more
  than 25 m away. The 1-star pull-out (O2) then never happens."
  **Result: it held.** See bugs 1 and 2. The headless gates miss it: `police_pull_out.rs` spawns one cop 1.2 m from
  the door, and `police_cars.rs` puts the player on foot or has him drive off.

## 1. Environment

- Direct run. No docker-compose and no dev server. The workspace has cargo tests and the BRP runtime driver
  `tools/qa/brp.py`.
- Host monitor during the runtime runs: `\\.\DISPLAY1` at 144 Hz, `Fifo`. That is not the 30 Hz virtual display
  of earlier tasks.
- Every game run used `--settings-id` QA (`brp.Game`), a release build and `--features dev`. The owner's
  `gta_like.exe` was not running, and I killed nothing. No game process was left afterwards: each run went through
  `Game.stop`.
- Probe crate: `scratch/qa/probe/` (copy of the reviewer's `cr_probe` manifest; `tests/common/mod.rs` is a patched
  copy of the gate helpers; the other helpers are included via `#[path]`). It builds with
  `CARGO_TARGET_DIR=D:/test-gta-like/target`, so there is no second target directory.

Reproduce:
```
cargo build -j 4; cargo clippy -j 4 -- -D warnings; cargo clippy --workspace --all-targets -j 4 -- -D warnings
cargo test -p gta_sim -j 4; cargo test -p citygen -j 4; cargo test -p gta_like --bin gta_like -j 4   (x3)
cd maw/tasks/in_progress/TASK-016/scratch/qa/probe
CARGO_TARGET_DIR=D:/test-gta-like/target QA_SEEDS=1,2 cargo test -j 4 --test qa_police_response -- --nocapture
CARGO_TARGET_DIR=D:/test-gta-like/target cargo test -j 4 --test qa_traffic_health -- --nocapture --test-threads=3
CARGO_TARGET_DIR=D:/test-gta-like/target cargo test -j 4 --test qa_pullout_city -- --nocapture
python maw/tasks/in_progress/TASK-016/scratch/qa/qa_hijack.py <out> 12
python maw/tasks/in_progress/TASK-016/scratch/qa/qa_runtime.py <out> [A,B,C,D,E,F]
python tools/qa/scenarios/t15.py --out <dir>      (x3), t8 .. t14 once each
```

## 2. Test results

**Existing suites (all at `19ebb82`)**
- `cargo build -j 4` ok.
- `cargo clippy -j 4 -- -D warnings` and `--workspace --all-targets` are clean.
- `cargo test -p gta_sim -j 4`: 56 targets, 450 passed, 0 failed (`scratch/qa/test_sim.log`).
- `cargo test -p citygen`: green.
- `cargo test -p gta_like --bin gta_like`, 3 runs: 79/79 each time.
- `tree_check.py` passes. `cargo tree -p gta_sim -e normal -i bevy_render` prints nothing. Largest file:
  `police/cars.rs` at 642 lines.
- Branch CI for `19ebb82`: all 5 jobs green, sim gates included (run 36106760388).

**Flip-RED done by me** (each file restored with `git checkout` and checked with sha256 `OK`)
- AC2: `junction.rs:131` conflict test disabled → `traffic_intersection::one_car_per_conflict_point` goes **RED**.
- AC3: `spawn.rs:112` offscreen condition set to `true` → 3 of the 4 `traffic_bubble` rows go **RED**.
- AC5: `car_dispatch.rs:141` `active < row.cars` → `active <= row.cars` → `cars_follow_row_1..5` all stay
  **GREEN**. The car cap never binds, because the units cap binds first in every row. See bug 5.

**New probes (headless, production composition)**

| probe | what | result |
|---|---|---|
| `qa_police_response` | seeds 1 and 2, park / hospital / plaza, 1 / 2 / 4 stars, player on foot | first cop within 18 m: seed 1 at 3.0-12.6 s (park 2★ 12.6 s is the worst), seed 2 at 3.4-9.8 s. Every cell ≤ 15 s. At 1★ a cop arrests in 6.7-15.1 s (`scratch/qa/police_response.log`) |
| `qa_traffic_health` | 5 min of roaming on seeds 1 and 2 (0★) and 3 min on seed 1 at 3★. The player walks the sidewalk graph at 6 m/s with the view along the walk, 40 civilians. Checks run every tick | kinematic pass-through (penetration > 2 cm) **0** pairs. Dynamic-dynamic overlap 0. First contacts traffic → parked car **0**. Longest junction wait 23.6 s, no car waits > 30 s. Entity count flat: 1.7-2.05 k over 5 min. Max 24 traffic cars and 3 police cars at 3★. AI car standing on a connector: up to **13.3 s** (bug 4) (`traffic_health.log`, `traffic_health_detail.log`) |
| `qa_pullout_city` | 1★, player sitting in a stopped car on a city lane (seed 1), hospital and park | hospital: pulled out at 7.3 s, Busted at 8.8 s. **Park: never pulled out in 45 s** (bug 2) |

**Runtime (release, BRP)**
- `t15.py` ×3: **3/3 PASS**.
  - Traffic: 18-19 cars, mean 8.2 m/s, max 12.3.
  - Hijack: 1 fleeing driver.
  - Chase: at most 2 active cars, first 38 m, closest 4.2-4.6 m.
  - Dismount: 2-4 `CrewOf` cops.
  - Frame cost with no vsync: 3.2-3.5 ms (144 Hz monitor, Fifo).
  - Logs: `scratch/qa/t15_run{1,2,3}/`.
- Hijack flake diagnosis (`qa_hijack.py`, 12 attempts in one session): 11/12 in the car within 0.03 s. In the one
  failure the player was `KnockedDown` (0.29 s left) at the moment F was sent, and the target car had turned
  `Dynamic`: the player had been teleported 12 m in front of a moving car and was hit.
  - `enter_exit` consumes `vehicle_requested` and ignores it while the player is knocked down (`seat.rs:229-236`).
  - Verdict: a **harness defect**, not a game bug (bug 3). The data is in `scratch/qa/hijack/attempts.json`.
- `qa_runtime.py` (`scratch/qa/runtime*/summary.json`):
  - A. The pistol pickup works.
  - B. The cabin shot could not be made over BRP. A stopped car drives off as soon as the player steps out of its
    lane to get a side-window line (dead_end logged). The mechanism stays covered by the headless
    `traffic_bailout` gate, which is green. It is also on the owner checklist.
  - C. 1★ with the player in a stopped hijacked car: the police car crept 32.9 → 25.6 m in 24 s in `Respond`. It
    dismounted only at **exactly 25.0 m**, 27 s in. Its 2 cops then stood in `Respond` 8.2 m away until the 45 s
    timeout: no pull-out, no Busted. See bugs 1 and 2.
  - D. 5★ chase: 5 active cars, SWAT on foot, frame cost with no vsync **3.0-3.7 ms** at 5★ with 21-24 traffic cars.
    - Without armour the driver is **Wasted in about 17 s, twice**: SWAT fire plus cabin wounds. This is balance
      and goes to the owner.
    - With armour: the player's car got stuck against a wall. Four police cars then waited 33-39 m away in
      `Respond` for more than 25 s and never dismounted. Only the car that stopped at 24.9 m let its crew out
      (bug 1, screenshot `runtime2/chase5_3.png`).
    - Re-boarding was seen: the crew of a car at 38.8 m got back in once the player was in a car again (crew out
      4 → 2).
  - E. Wasted while driving: `Wasted` → `Playing`, the player is out of the car at ground height (y 1.20 = float
    height), and traffic keeps running (21 cars).
  - F. New city while driving at 2★: seed 7 loads, then 12 traffic cars, 0 police cars, 0★, 1 player, not driving.
- Regressions `t8`, `t9`, `t10`, `t11`, `t12`, `t13`, `t14`: all exited 0 with no errors in the log
  (`scratch/qa/reg_t*.log`).

## 3. Acceptance criteria

| criterion | test performed | result |
|---|---|---|
| IDM: no negative speed, no overlap, 10 cars, 6400 ticks | `traffic_idm` green; my 5-min roams: 0 kinematic pass-through | PASS |
| One car per intersection conflict point | `traffic_intersection` green; my flip (conflict test off) → RED | PASS |
| Despawn only after ≥ 2 s off frame | `traffic_bubble` green; my flip (offscreen check off) → 3 rows RED | PASS |
| Kinematic → dynamic on contact | `traffic_contact` green; roams: 0 pass-through, 0 contacts with parked cars, switches by Character 28-43 per 5 min | PASS |
| ≤ 2 police cars at 2★ | `cars_follow_row_2` green; t15 ×3 at most 2 active | PASS as a behaviour. The gate does not bind the car column (bug 5) |
| `t15.py` exists and passes via `brp.py` | 3/3 PASS; the 2/5 flake is a harness issue (bug 3) | PASS |
| Owner checklist | section 6 | recorded (SHIP-PENDING-RUNTIME item) |
| Tuning values in data | diff scan: the only new `const`s are the laws `IDM_DELTA`, `ROOF_EXIT` and the config path | PASS |
| build, clippy, tests green | section 2 | PASS |
| Existing tests pass | 450 sim + 79 client ×3 + citygen; t8-t14 runtime | PASS |
| O2: pull a stopped driver out at 1★ | gate `police_pull_out` green (one cop, set up at the door); **city probe park: never pulled out; runtime: never pulled out** | **FAIL** (bugs 1, 2) |
| Orchestrator: police response ≤ ~15 s (park, hospital, plaza; 1/2/4★) | on foot: ≤ 12.6 s on seeds 1 and 2. **A stopped or stuck driver farther than 25 m: never** | PASS on foot / **FAIL** in a car (bug 1) |
| Traffic health over 5 min (box gridlock, connector stops, pass-through, parked, entity count) | `qa_traffic_health` | PASS, with minor bug 4 |

## 4. Bugs found

**1. MAJOR — a stopped driver farther than 25 m is treated as "driven off": police cars never dismount.**
- **Where.** `police/car_route.rs:372` sets `driven_off: driving.is_some() && distance > c.reboard_distance` and
  never checks the player's car speed. `cars.rs:158` then refuses `Respond -> Dismounted` for any reason
  (route_done, blocked, no_route) while `driven_off` is true.
- **Why it happens.** The fixer added this to stop the dismount/re-board flip-flop. But a player who has stopped
  (`car.stopped >= stopped_seconds`) or who is stuck against a wall counts as driven off too.
- **Reproduction:**
  - (a) Runtime `qa_runtime.py … C`: 1★, the player sits in a stopped hijacked car in a lane. Traffic queues
    behind it. The police car crawls 32.9 → 25.6 m over 24 s, then dismounts at 25.0 m at 27 s
    (`runtime2/summary.json` C trace).
  - (b) `qa_runtime.py … D` with armour: the player's car is stuck on a sidewalk against a wall at 5★. Four cars
    wait in `Respond` at 33-39 m for more than 25 s. Only the car at 24.9 m dismounts (`runtime2/chase5_3.png`).
- **Expected:** a stopped or stuck driver gets approached. Cars that are held up let their crews out
  (`blocked_seconds`).
- **Actual:** the police wait at 25-40 m for as long as the player stays in the car.
- **Suggested direction** (the fixer decides): `driven_off` only for a moving driver, for example
  `driving && !stopped_driver && distance > reboard_distance`. Re-run the fixer's flip-flop probe
  `fix_chase.rs` after the change.
- **Missing gate:** a city row with the player in a stopped car 30-40 m from the police route end → a dismount
  within N s.

**2. MAJOR — the pull-out stalls forever when the second arresting cop stands on the left-door exit spot.**
- **Where.** `vehicle/seat.rs:193` calls `exit_spots(.., &[])`, which excludes no bodies. `police/arrest.rs:72`
  retries every tick with `pull` growing.
- **Why it happens.** Both crew cops go to `Arrest` and walk to the same door point (`behavior.rs:374-379`,
  `stand_distance`). One ends up 0.44 m from the door point. The left-door capsule is then never clear, so
  `pull_out` returns false on every tick.
- **Reproduction:** `scratch/qa/probe/tests/qa_pullout_city.rs`, park case. The player sits in a car on the
  nearest lane to `park_center`, 1★.
  - `pull` grows from 0.6 to 30.6 s and nobody is pulled out.
  - Teleporting both cops 5 m away made the pull-out and Busted happen within 2 s. That shows the cop on the spot
    is the cause.
  - The hospital case passed only because the cops happened to stand elsewhere.
- **Expected:** after `pull_out_seconds` the driver is pulled out and arrested (O2).
- **Actual:** the player sits in the car at 1★ forever. That is the "car is a safe haven" defect O2 was meant to
  remove.
- **Missing gate:** `police_pull_out` with two crew cops in `Arrest`. Today it has one cop placed by hand.

**3. MINOR (harness) — the t15 hijack flake.**
- **Cause.** `t15.py` step 2 teleports the player 12 m in front of a car that may be doing 12-16 m/s. That is
  inside the car's braking distance at `max_deceleration` 8. The car switches to dynamic and knocks the player
  down, and the F press is ignored while he is knocked down.
- **Evidence:** `scratch/qa/hijack/attempts.json` attempt 0: `hit: KnockedDown`, target `Dynamic`, no entry in
  4 s. The other 11 attempts entered within 0.03 s.
- **Fix in the scenario.** Wait for `HitReaction == Steady` before F, or retry F. Choosing a car that is already
  slow, or standing farther ahead, also works.

**4. MINOR — pedestrian/car stand-offs inside intersections, up to 13 s.**
- In 5-min roams, AI cars (kinematic and dynamic) stood on a connector for 3-13.3 s. Each time, civilians stood
  against the car's nose or flank at 0-0.2 m/s ("ahead 0.0-0.9 m, side ±1.5 m"), inside the forward cast.
- The car waits for the pedestrian and the pedestrian is pressed against the car. Both resolve on their own, and
  no junction wait passed 23.6 s. The box is locked for that time.
- Evidence: `scratch/qa/traffic_health_detail.log`. For the owner run, and possibly a follow-up (the cast ignores
  a body that is already beside the car, not ahead of it).

**5. MINOR (gate) — AC5's car cap is never exercised.**
- Flipping `active < row.cars` to `<=` keeps `cars_follow_row_1..5` green. The units cap (`cars × crew ≤ units`,
  plus the foot seats that fill) always binds first.
- The criterion "≤ 2 cars at 2★" is true by arithmetic of the shipped data. No gate protects the `cars` column
  if `units` or `crew` change.
- The implementer logged a related dead_end.

**Observation (owner):** at 5★ an unarmoured player in a car was Wasted in about 17 s, twice (SWAT SMG plus cabin
wounds at share 0.5). This is balance, not a defect.

## 5. Verdict

**NEEDS_FIXES.** The traffic core is solid under independent checks: 0 pass-through, 0 contacts with parked cars,
flat entity count, AC1-AC4 gates falsifiable. The foot response time and t15 ×3 also pass. But the police-car
side fails the binding O2 note and the orchestrator's response focus whenever the player stays in a car:
- bug 1: a stopped or stuck driver farther than 25 m is ignored by every car;
- bug 2: the 1★ pull-out can stall forever.

Both are silent in the headless gates, cheap to fix, and need one new gate row each. Bug 3 is a scenario fix.

## 6. Owner checklist (for the owner run after the fixes)

Запуск: `python tools/fetch_assets.py`, затем `cargo run --release -- --seed 1`.
- Трафик: на улицах едут машины, встают в очередь и уступают на перекрёстках. Пропускная способность около одной
  машины за 4 с через загруженный перекрёсток (OPEN_DECISIONS). Посмотреть, не бесит ли.
- Пешеходы и машины: иногда машина стоит посреди перекрёстка до ~13 с, упершись в пешехода (баг 4).
- Угон: встаньте перед машиной, дождитесь остановки, подойдите к двери, F. Водитель убегает, через время звонит
  (звезда).
- Выстрел в боковое окно машины трафика: водитель тормозит, выходит и убегает, появляется инцидент стрельбы.
  По BRP проверить не удалось, есть только headless-гейт.
- Такси в потоке, модели полиции и такси: масштаб и посадка колёс.
- Погоня на 2-5 звёздах: машины преследуют, таранят, копы выходят, когда вы стоите рядом, и садятся обратно,
  когда уезжаете. Строй SWAT на 4-5 звёздах. Без брони на 5 звёздах в машине смерть примерно за 17 с.
- Ожидание за брошенной машиной без объезда (go-around STOP): полоса стоит, пока машину не уберёт пузырь.
- Высота сирены на крыше полицейской машины.
- 1 звезда, стоите в машине: коп вытаскивает и арестовывает. Проверять после фикса багов 1 и 2.

## 7. Cleanup and git status

No containers or services were started. Every game session ended through `Game.stop`, and no `gta_like` process
remains. `git status --short` outside the task dir shows only `maw/tasks/pending/TASK-028/task.md` (M) and
`maw/tasks/pending/TASK-031/` (??). I did not create or touch either; they come from the orchestrator, so I left
them alone. My files are all under `maw/tasks/in_progress/TASK-016/scratch/qa/`, plus this report and 2 log
entries.

children: 0 launched / 0 reported.
