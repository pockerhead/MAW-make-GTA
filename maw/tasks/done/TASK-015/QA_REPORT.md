# QA_REPORT — TASK-015 (GDD T14, drivable car)

QA: claude opus, effort medium. Commit under test: `1a7b866` on `feature/t14-car` (clean tree). Verdict at the end:
**NEEDS_FIXES** (one regression in the T13 runtime scenario, a one-line tooling fix), otherwise SHIP-PENDING-RUNTIME.

## 0. Preflight and disconfirmation

- Read `scratch/` first as a coverage map: `fixer/` (curb probes and peak rows), `flips/`, `cr/golden/`, `qa_t14*`,
  `qa_t11_fixer*`, `pr2/`, `probe_collision/`. I did not rerun any of their probes as evidence.
- Read TASK_FINAL, PLAN_FINAL (all 731 lines), IMPL_SUMMARY, IMPL_REVIEW, FIX_SUMMARY and FIX_SUMMARY.prev-1,
  OPEN_DECISIONS, PCTX_PROPOSALS and the log. The only `dead_end` entries are the plan-reviewer-2 G12(c) fixture (the
  implemented `driver_is_not_hit_over_the_roof` uses the replacement fixture) and the fixer's t11 stall with cars in
  `dest_clear`. I checked the second in code: `police/behavior.rs:235` uses `wall_blocked` (World only) for
  `dest_clear`, and `cop_sees` uses `sight_blocked`.
- **Counter-example written first:** "In the real seed-1 city, a car driven at 28 m/s square into a 0.15 m convex-hull
  curb still gets caught: it loses health or bounces back." The headless gate covers only 20 m/s on a hand-built slab.
  I tested it on real city curbs at runtime (§2.2). **It did not hold.** Curbs cost 0 hp at 10, 20 and 28 m/s, square
  and at 30°, 60°, 80° and 85°, under throttle and under braking, and also when driving off the curb. The car never
  lost speed at the curb and always ended up on the sidewalk.
- The second counter-example did hold (a latent bug, §4 B2). The exit checks only a capsule at the door point and never
  the path from the seat to it. A thin fence standing between the car's side and the door point lets the player exit
  through it.

## 1. Environment

- Direct, no docker or mocks. Windows 11 host. The release build with `dev` features is
  `target/release/gta_like.exe`; `cargo build --release --features dev` was already fresh at HEAD. BRP on
  `127.0.0.1:15702`. Every run used `--settings-id com.github.pockerhead.maw-make-gta.qa`. The owner's game was never
  running, and the target was not locked, so no scratch `CARGO_TARGET_DIR` was needed.
- My runtime driver is `scratch/qa/qalib.py` + `phases.py`. It launches the exe once and attaches over BRP. The phase
  scripts are `curb.py`, `curb2.py`, `runover.py`, `theft.py`, `cover.py`, `exits2.py`, `flows.py`, `newcity.py`,
  `carcar.py`, `perf.py` and `perf2.py`, all in `scratch/qa/`. Their JSON results sit next to them, and the PNGs are
  local (git-ignored). The menu was driven with real OS input (`scratch/qa/osinput.py`, copied from TASK-013).
- To reproduce: `python -c "import sys; sys.path.insert(0,'maw/tasks/in_progress/TASK-015/scratch/qa'); import qalib; qalib.launch()"`,
  then run `python <phase>.py` from `scratch/qa/`, starting with `phases.py` helpers (run the p1 block in §2.1 first:
  it writes `p1_enter_drive.json`, which later phases read).
- Services left: none. The game was shut down after each session, and `tasklist` showed no `gta_like.exe` at the end.

## 2. Test results

### 2.1 Build, lint, existing suites (all foreground, `-j 4`)
| Command | Result |
|---|---|
| `cargo build -j 4` | OK |
| `cargo clippy -j 4 -- -D warnings`; `cargo clippy --workspace --all-targets -j 4 -- -D warnings` | clean |
| `cargo test -p gta_sim -j 4` | 41 binaries, **370 passed, 0 failed** |
| `cargo test -p citygen -j 4` | 32 passed, 0 failed |
| `cargo test -p gta_like --bin gta_like -j 4`, 3 runs | 77 / 77 / 77 passed |
| `python tools/qa/tree_check.py` / `cargo tree -p gta_sim -e normal -i bevy_render` | passed / empty |

There are no failures, so there is no failure list to compare against base.

### 2.2 My own flip-RED (gates domain; restored by `git checkout`, sha256 verified OK for both files)
- Flip A, `seat.rs exit_spot`: door candidates accept any feet height. Result: `low_wall_at_the_door_is_not_an_exit`
  goes **RED** ("1 m wall: exited at [-26.7, 2.05, -0.3]"); the other 15 seat gates stay green.
- Flip B, `impact.rs`: the car-car loop writes only the first car. Result: `driven_car_rams_a_parked_one` and
  `parked_car_rolls_into_the_driven_one` go **RED**.
- After the restore, all six vehicle/tnua/config binaries were rerun and are green.

### 2.3 Runtime (release + dev, seed 1 and then seed 2 via the pause menu)
All numbers below come from BRP reads.

**Enter a parked (sleeping) car and drive off** (`p1_enter_drive.json`). The target car had `Sleeping` before F and
lost it after entry. W for 3 s gave 23.1 m of travel and a top speed of 13.5 m/s. `VehicleLoad` read awake 1, rays 4.
At start, 143 of 143 cars slept with rays 0.

**Curb (the owner's bug).** These runs used a real avenue curb, 12 m past a parked spot. The curb position was checked
at runtime: +3.5 m to the right gives y 1.31 and `on_sidewalk`. Each run started with the nose 8 m short of the curb,
the speed set by BRP and W held. Damage timeline from about 55 Hz polling (`curb_runtime_12.json`, `curb2.py` rows):

| speed | approach (0° = square) | curb damage | slowest speed before the centre is 3 m past | ends on sidewalk | peak wheel force |
|---|---|---|---|---|---|
| 10 | 0 / 30 / 60 | 0 / 0 / 0 | 10.0 / 9.95 / 9.95 | yes | 7.8-8.2 kN |
| 20 | 0 / 30 / 60 | 0 / 0 / 0 | 20.0 / 19.95 / 19.95 | yes | 6.2-8.3 kN |
| 28 | 0 / 30 / 60 | 0 / 0 / 0 | 27.95 / 27.95 / 27.94 | yes | 6.6-8.1 kN |
| 10 / 20 / 28 | 80, 85 (grazing) | 0 | no loss | yes (10 m/s at 85° stays on the road) | 7.8-8.8 kN |
| 20 / 28 braking (S) | 0, 30 | 0 | 13.8-24.2 (brake only) | yes | 8.7 kN |
| from rest, 1.5 s W | 0, 45 | 0 | climbs at 4-5 m/s | yes | 7.8-8.6 kN |
| off the curb 10 / 20 / 28 | 0, 45 | 0 from the curb | no loss | n/a | — |

- Every health drop in these runs came with the car centre 5.5-9.7 m past the curb. That means building faces after
  the sidewalk, or pistol hits from cops once I had built up heat (25 hp steps). None came at the curb.
- There is no rebound: the velocity along the approach never dropped at the curb. The wheel peak is ~8 kN, which
  matches the fixer's cap.
- A second location (spot −15 m) turned out to be an intersection with no curb. It was discarded; see the log
  dead_end.

**Car vs car** (`carcar.json`). At 15 m/s the closing speed at contact was 14.3 m/s (the car coasts over 10 m).
- The driven car rams a sleeping parked car: both lose 373.6. The formula gives (14.34 − 5)·40 = 373.6.
- A sleeping parked car is launched into the standing driven car: both lose 373.6.
- In both cases `SoundStats.spawned[Impact]` +1, so one metal sound, deduplicated.

**Run over a pedestrian** (`runover_10.json`, `runover_14.json`).
- 10 m/s, and the civilian fled from the car (Car threat): 19 + 3 hp (first hit plus a re-hit) and heat 0 → 30.
  That is the RunOver row. Heat came from a report, since no cop was in view.
- 14 m/s into a civilian running toward the car (closing about 18 m/s): 78 hp, killed, heat 0 → 80, one star (Kill).
- The formula itself at 10 m/s is gated headless (`run_over_at_10_mps_costs_the_formula`).

**Steal a car in a cop's view** (`theft.json`). A cop in Arrest 18.6 m from the car. Heat 90 → 105 at the entry tick,
which is CarTheft +15. It stayed 105 after 1 s.

**Exits** (`exits2.json`, `exits.json`, headless probe `scratch/qa/zz_qa_probe.rs`):
- Left door 0.3 m from a parked car's side: exit at the right door at ground level. The parked car's roof (2.08 m) is
  rejected as a floor, so it acts like a low wall.
- Gap 0.8 m: exit at the left door, between the cars.
- Car half on the curb, and car on the sidewalk: normal door exit, feet on the sidewalk.
- Headless walls of 1.0 m (0.3 thick), 1.2 m and 2.5 m fences at the door: right door.
- **Fence 0.05 m thick 0.1 m off the car side** (between the side and the door point): the player exits on the FAR side
  of the fence. See §4 B2.

**Police and cars** (`p_police_driving.json`, `cover_events_1.2_0.0.json`):
- Player sits in a car on the road at 2 stars, two cops in Attack 8-20 m away: car 1000 → 0 in ~20 s (25 per pistol
  hit), the player's health stays 100 (the driver is protected), no parked car is dented.
- Player on foot on the sidewalk behind a parked car, armour 1e6, 2 stars, 48 s: about 73 hits on the player and
  8 on the cover car. For every event I computed the cop → player line in the car's frame. **It never crossed the car
  box** (clearance 1.62 m and 0.37 m). The 8 car hits came from the cop whose line passed 0.37 m from the car's front
  corner: spread plus aim error clips the corner. No shot was fired through a line the car blocks, so the rule holds;
  see §4 O1.

**Driving through events** (`flow_wasted.json`, `flow_newcity.json`, `pause_resume_driving.json`):
- Wasted (DebugDamage 1000) while driving at 15 m/s. In Wasted: no `Driving`, `RigidBodyDisabled` or
  `ColliderDisabled` on the player, no disabled head, no car with a driver, no `SleepingDisabled` anywhere. The car
  coasted on (13.6 m/s → 1.1 m/s later); it was not stuck. After the respawn: player at the hospital, W 1 s = 4.39 m.
- New city (Esc, seed 2, Enter) while driving at 10 m/s: seed 2, one player, player components clean, 128 cars (seed 2
  spot count), 0 drivers, 128/128 asleep, rays 0, walk 4.39 m, a new car driven 11.8 m.
- Pause and resume (resume with OS Escape) while driving: still `Driving`, W 1.5 s = 7.2 m, F exits, walk 4.39 m.
- Busted while driving: not forced at runtime. `NextState` is not reflected, and no production path leads to Busted in
  a car. It is covered headless by `busted_while_driving_ejects_and_respawns_at_the_station` (logged decision).

**Frame cost while driving** (`perf_driving.json`, `perf_driving_heat*.json`). A 492 m straight avenue at 28 m/s, 40
civilians, seed 2:
- `frame_report` no-vsync averages were 3.07 / 2.70 / 2.84 ms.
- Per-frame samples (174 samples over 11 s): min 2.3, p50 2.78, p95 3.61, max 4.33 ms.
- Monitor: `\\.\DISPLAY1` 144 Hz primary, present mode Fifo, but the as-shipped rate reads 30.0 FPS. This is the known
  host display issue (TASK-010 lesson), not the car.
- The 2-star variant lost its heat before the drive (no cop saw the player), so it adds nothing beyond a second
  0-star sample (p95 3.24 ms).

**Scenario gates:**
- `t14.py` (AC): **PASS** (`scratch/qa/t14/summary.json`). 143 cars, 13.0 m/s top speed and 27.4 m in 3 s, engine
  sound 1, 142 minimap dots, wall crash 1000 → 579.9, stopped at z 697.79, exit 1.73 m from the car, `Playing`, no log
  errors.
- Regressions: `t8`, `t9`, `t10`, `t11`, `t12` exit 0 with no log errors. The t11 arrest reach took 6.2 / 7.6 s,
  Busted 11.7 s, no stuck cop.
- **`t13` FAILS** with an `IndexError` in `check_peaks` (§4 B1). A scratch copy with only the Engine class and a cap
  added passes (`scratch/qa/reg_t13_patched/`, frame cost 3.83 ms).

Screenshots I looked at: `t14/drive_3.png` (chase camera behind the car, HUD with the orange car bar, minimap dots),
`t14/crash.png` (car stopped at the city edge; the edge wall has no mesh, which predates T14), `t14/exit.png` (player
at the left door), `police_shoot_driven_car.png` (stalled car with hood smoke, empty car bar), `police_cover_car.png`,
`wasted_while_driving.png` (desaturated, body on the road, the car rolling on), `exit_beside_parked_car.png`.

## 3. Acceptance criteria

| # | Criterion | Test performed | Result |
|---|---|---|---|
| 1 | Headless: max speed does not pass a wall in 128 ticks | `car_at_max_speed_does_not_tunnel_the_wall` green (128-tick loop checked in code) | PASS |
| 2 | Headless: same into a building corner at 45° | `car_at_max_speed_does_not_tunnel_a_corner` green | PASS |
| 3 | Headless: enter and exit change who controls | `on_foot_throttle_does_not_move_the_car`, `enter_drive_exit_hands_control_over` green; runtime entry and exit of a sleeping car, pause/resume | PASS |
| 4 | Headless: 10 m/s pedestrian hit costs the formula | `run_over_at_10_mps_costs_the_formula` (+6 m/s, 2.5 m/s rows) green; runtime damage and RunOver heat observed | PASS |
| 5 | Headless: a car at rest does not drift in 640 ticks | `driven_car_at_rest_does_not_drift`, `parked_car_at_rest_does_not_drift_and_sleeps` green; runtime 143/143 asleep | PASS |
| 6 | Runtime `t14.py` exists and passes | run by me, PASS | PASS |
| 7 | Owner checklist in QA_REPORT | §6 | PASS (owner to execute) |
| 8 | Tuning in data files, not `const` | diff scan: the new consts are laws or paths (`GRAVITY`, `WHEELS`, `MODEL_YAW`, `COUNT`, config paths, schema, stream tag); tuning is in `sedan.ron`/`damage.ron`/... | PASS |
| 9 | build, clippy, tests green | §2.1 | PASS |
| 10 | Existing tests pass | all cargo suites green; **runtime scenario `t13.py` crashes** because of T14's new `SoundClass::Engine` | **FAIL** |
| — | Orchestrator focus: curb at 10/20/28, square and angled | §2.3 table | PASS |
| — | Car-car with the player's car on either side | §2.3 | PASS |
| — | Run over, RunOver heat; theft in cop view, CarTheft heat | §2.3 | PASS |
| — | Exits beside a low wall (parked car) and a fence | runtime + headless probe | PASS, one latent edge case (B2) |
| — | Cops do not fire through a blocking parked car; cops still hit the driven car | §2.3 | PASS (O1 note) |
| — | Wasted, new city, pause while driving; no leftovers, no stuck car | §2.3 | PASS (Busted headless only) |
| — | Frame cost at speed with population | p95 3.6 ms, max 4.3 ms | PASS |

## 4. Bugs found

**B1. Regression: `tools/qa/scenarios/t13.py` crashes (severity: minor, but it breaks "existing tests pass").**
- Repro: `python tools/qa/scenarios/t13.py --out <dir>` → `IndexError: list index out of range` at `t13.py:72`
  (`check_peaks`, first call after the melee step).
- Cause: T14 added `SoundClass::Engine` (`src/audio/cues.rs:33`, `COUNT = 9`), so `SoundStats.peak_alive` has 9
  entries. `t13.py:31` `CLASSES` and `mix_caps()` (`t13.py:55`) still list 8.
- Expected: t13 passes as before T14. Actual: it crashes before any assertion.
- Fix, verified in `scratch/qa/t13_patched_qa.py` (exit 0): append `"Engine"` to `CLASSES` and one cap (e.g. `1`, one
  emitter for the player's car) to the `caps` list.

**B2. Exit through a thin fence (severity: minor, latent; no such geometry exists in the city today).**
- Repro (headless, `scratch/qa/zz_qa_probe.rs`, which ran from `crates/gta_sim/tests/` and was then removed): car at
  (−25, 0) yaw 0, a wall 0.05 m thick and 1.2 m high centred at x −26.325, i.e. 0.1 m off the car's left side. Press F
  → the player stands at (−26.7, 1.05, −0.3), on the far side of the fence.
- Expected: the fence blocks the left door, so the exit is at the right door. That is what happens for fences
  ≥ 0.1 m thick that reach the door capsule.
- Cause: `seat.rs exit_spot` tests only a capsule overlap at the candidate. Nothing checks the line from the seat to
  the door point, so an obstacle entirely within 0–0.2 m of the car side is skipped.
- Today's city has no thin walls (lamp poles are not barriers), so this is a note for T15/T16 geometry. A capsule or
  ray cast from the seat to the candidate would close it.

**Observations (not bugs, for the orchestrator):**
- **O1. Partial cover.** A cop whose line to the player passes 0.37 m from a parked car's corner puts some of its
  pistol shots into the car: 8 of about 81 hits in 48 s. This is spread, not firing through a blocked line. The
  bystander hold-fire rule widens the line by the spread cone, but cars are not in that rule.
- **O2. In a car the player cannot be arrested or wounded.** At 1 star, cops stay in Arrest near a seated player and
  never fire. At 2+ stars, the car soaks all fire, and a car at 0 hp still shields the driver. Q2 and GDD §6.4 ("no
  arrest in a car, pull-out out of MVP") intend this, but together it means the player cannot lose while sitting in a
  car. For T15 (the forced eject onto a low wall is already noted there).
- **O3.** The Fifo frame rate reads 30 FPS on a 144 Hz primary monitor. This is the host issue known since TASK-010,
  not a T14 cost.

## 5. Verdict

**NEEDS_FIXES.** Everything the task gates is green, headless and at runtime:
- the owner's curb bug is gone on real city curbs at 10, 20 and 28 m/s;
- car-car, run-over, theft, police and all flow paths behave;
- frame cost is about 3 ms.

The one failing item is the T13 runtime scenario, which T14 broke by adding a sound class (B1). The fix is two list
entries in `t13.py`, and I proved it with a patched scratch copy. B2 is latent and can go to T15. After B1, this is
**SHIP-PENDING-RUNTIME** with the owner checklist below: handling, camera, sound and crash/curb feel are owner-only.

## 6. Owner checklist (что посмотреть)

`cargo run --release --features dev -- --seed 1` (или `--features fast`), подойти к машине у бордюра проспекта.

- [ ] **Управляемость.** F у левой двери, W/S/A/D, Space — ручник. Разгон до ~100 км/ч примерно за 6 с. На скорости
      руль становится туже. Заносит ли на ручнике так, как хочется? Значения в `assets/vehicle/sedan.ron`
      (`acceleration`, `max_speed`, `steer.*`, `grip.*`, `roll_influence`, `suspension.*`).
- [ ] **Камера.** Держится ли за машиной без рывков, возвращается ли за корму через ~1,5 с без мыши, не проваливается
      ли в стены. Настройки: `assets/camera/camera.ron` `car_*`.
- [ ] **Звук двигателя.** Гул на холостых, рост тона с газом и скоростью, пауза глушит его. Громкость и тон:
      `assets/audio/mix.ron` `engine`.
- [ ] **Ощущение удара.** Стена на 15–28 м/с: металлический звук, тряска, полоса машины в HUD падает. Машина в машину:
      один звук, бьются обе. На нуле машина дымит и не едет. Сбить пешехода: урон и падение; катящаяся машина бьёт
      лежащего ещё раз (оставить так?).
- [ ] **Бордюр.** На 10/20/28 м/с прямо и под углом машина заезжает на тротуар без урона и без отскока (в QA 0 урона
      во всех случаях). Посмотри, не слишком ли сильно машина подпрыгивает или клюёт носом, и не проходит ли бампер
      визуально сквозь край бордюра. Если подскок раздражает, есть `sedan.ron suspension.max_damper_speed` (0.5;
      1.0 — меньше подскок, жёстче удар).
- [ ] **Выход.** F только на скорости ≤ 3 м/с. Рядом с другой машиной вплотную выходишь с другой стороны; зажатый со
      всех сторон остаёшься внутри.
- [ ] **Полиция.** На 2 звёздах копы стреляют по машине, полоса падает, водителя не задевает. Припаркованная машина
      между тобой и копом работает как укрытие, но выстрелы у самого угла могут попадать в машину. Нравится ли, что в
      машине тебя нельзя арестовать и ранить (O2)?

children: 0 launched / 0 reported.
