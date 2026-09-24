# FIX_SUMMARY — TASK-015 (GDD T14), fixer round 2 (small)

Input: the orchestrator's binding note for round 2 (the last `OPEN_DECISIONS.md` entry, 2026-09-25) with two items:
the damper rate cap and exit on top of a low wall. `IMPL_REVIEW.md` was re-read: all of its items were closed in round 1
(`FIX_SUMMARY.prev-1.md`), and nothing new is taken from it. Branch `feature/t14-car`, nothing committed.

## 0. Preflight

- I read `scratch/` as a coverage map only: `fixer/` (round-1 curb probes and peak rows), `flips/`, `cr/golden/`,
  `qa_t14_fixer/`.
- **The claim most likely to break correct code if applied literally.** Round 1 recorded the peak as "old ~7 kN" and
  the orchestrator asks to come back near it. But `curb_peak_before.txt` shows the spring alone (`k·x`) already peaks at
  7.0 kN on the step. A cap cannot reach 7 kN unless it goes to about 0, and a cap near 0 removes the damping and breaks
  G5. **Checked:** I measured the caps (table below). The reachable value is spring + c·cap, and the gate bound is
  derived from data rather than pinned at 7 kN.
- A second trap: a gate that recomputes the force in the test with its own formula (like the round-1 probe) cannot go
  RED when the production clamp is removed. The measured force therefore comes from production: new field
  `WheelState.force`, the suspension force applied that tick.

## 1. Fixed

### Item 1: damper rate cap
- `sedan.ron` `suspension.max_damper_speed: 0.5` (m/s). The damper sees the compression rate clamped to ±cap
  (`chassis.rs spring_force`). `VehicleConfig`: new field, included in the finite check and the positive check.
- `WheelState.force` (N, `Reflect`) records the applied suspension force, so the gate and BRP can read it.
- **Measured** (probe `scratch/fixer/zz_damper_probe.rs`, city-style convex curb 0.15 m, full throttle, peak wheel
  force / largest upward body velocity):

  | cap, m/s | 20 m/s square | 20 m/s 30° | 10 m/s | 28 m/s | hop v_y at 20 m/s |
  |---|---|---|---|---|---|
  | none (100) | 28 398 N | 28 860 | 29 165 | 28 655 | 1.23 m/s |
  | 1.0 | 9 346 | 9 388 | 9 184 | 9 202 | 0.84 |
  | **0.5** | **8 230** | 8 277 | 8 041 | 8 071 | 0.99 |
  | 0.3 | 7 770 | 7 828 | 7 563 | 7 619 | 1.11 |

  All rows: 0 damage, never slower than the kick speed, end on the sidewalk.
- **Why 0.5:** it is the closest value to the ~7 kN target that does not change the settle. 8.2 kN = spring 6.9 kN +
  c·0.5. G5's drop starts at exactly 0.5 m/s, so the settle dynamics are identical to before. Lower caps under-damp the
  rebound: at 0.3 the hop is 1.11 m/s. 1.0 gives the lowest hop (0.84) at 9.3 kN, which is an owner option (see §4).
- **Gate row** in `climb_curb` (`curb_at_20_mps_is_climbed_square` and `_at_30_deg`): the peak `WheelState.force` over
  the run is ≤ `k·travel + c·max_damper_speed` = 9 125 N. That is a fully compressed spring plus the damper at its cap,
  all from `sedan.ron`. Precondition (GATE BROKEN): peak > 2× the static wheel load, so the wheels really took the
  step.
  - **Flip** (no clamp in `spring_force`): **RED** on both rows, "a wheel pushed 28398 N at the curb, over 9125 N" and
    28860 N. Restored: GREEN.
- Unit row in `spring_force_rows`: a 0.15 m step in one tick (9.6 m/s) gives k·0.26 + c·0.5.
- Sabotage row in `config_vehicle.rs`: `max_damper_speed: 0.0` → "suspension.max_damper_speed must be positive".
- Untouched and green: G5 (spawn kick, drop, creep, 640 ticks), `flat_landing_is_not_a_crash`, G1/G2 tunnelling,
  G7 rollover, the car-car rows.

### Item 2: exit onto the top of a low wall
- **Reproduced first.** New gate `low_wall_at_the_door_is_not_an_exit` (`tests/vehicle_seat.rs`): walls 1, 2 and 3 m
  high, 0.1 m off the driver door. On the old code it went RED: "1 m wall: exited at [-26.7, 2.05, -0.3]", with the
  player standing on the wall top. Cause: the feet ray starts at the door point + 2 m (car centre + 2 m = 3.16 m) and
  lands on any top lower than that.
- **Fix** (`seat.rs exit_spot`): each candidate carries its expected feet height:
  - doors: the car's ground, `(centre − up·rest_height).y`;
  - roof: the car's top, `(centre + up·hy).y`.

  A candidate is valid only if the feet are within `float_height − capsule_height/2` = 0.3 m of that height. This is
  the capsule's clearance, the step the float spring walks over, so no new const is needed. The existing capsule
  overlap test still gives the headroom. If no candidate is valid, the player stays seated
  (`boxed_in_driver_stays_seated`, green).
- After the fix all three walls → the right door at ground level (centre y 1.05 ± 0.05).
  - **Flip** (skip the height check for the doors): **RED**, the 1 m row again.
- **Roof, a decision** (logged): the orchestrator's rule reads "ground level next to the car". I kept the planned roof
  candidate but checked it against the car's top, because the roof is the car, not an obstacle. The same rule rejects
  an overhang above the roof. New row `walled_doors_exit_onto_the_roof`: both doors walled with 4 m walls, roof open →
  the player stands on the roof (centre = car y + hy + float_height).
  - **Flip** (roof checked against the ground level): **RED**.
- Unchanged on purpose: the `forced` fallback (Wasted/Busted eject) still uses the left door's feet ray, so an eject
  next to a low wall can still land on the wall top. Wasted respawns the player right away. Busted leaves the player
  there, and a cop arrest next to a low wall is rare. This is outside the note's scope and noted for T15.

### Own leftover from round 1
- `config.rs` underbody error message had 18 spaces inside the string: a lost `\` line continuation from my round-1
  edit. Restored the continuation. The sabotage keyword still matches.

## 2. Skipped

- `IMPL_REVIEW.md` items 1-5, the missing coverage and the nits: all were handled or explicitly skipped with reasons in
  round 1 (`FIX_SUMMARY.prev-1.md`). This round has no new review input.
- Pinning the step peak at 7 kN: not reachable with a cap, see §0. The bound is derived from data instead.

## 3. Test results

- `cargo test -p gta_sim -j 4`: **370 passed, 0 failed**, 41 test binaries ok (368 before + `low_wall_at_the_door_is_not_an_exit`
  + `walled_doors_exit_onto_the_roof`; the curb rows grew in place).
- `cargo clippy --workspace --all-targets -j 4 -- -D warnings`: clean.
- `cargo test -p gta_like --bin gta_like -j 4`, 3 runs: 77 passed / 77 / 77.
- `rustfmt --edition 2024 --check` on every edited `.rs`: clean.
- citygen was not touched, so it was not rerun.
- Flips: clamp removed → curb rows RED (28.4 / 28.9 kN); door height check off → low-wall RED; roof level = ground →
  roof row RED. Each was restored and the diff checked (`cmp` against the backup).
- **Runtime:** `python tools/qa/scenarios/t14.py --out maw/tasks/in_progress/TASK-015/scratch/qa_t14_fixer2`:
  **PASS**.
  - 143 parked cars, entered, 3 s W → 13.2 m/s and 19.3 m, engine 1, 142 minimap car dots.
  - Wall crash 1000 → 571.5 hp; exit 1.73 m from the car, state `Playing`.
  - No log errors; frame cost without vsync ≤ 3.04 ms.
  - No game process was running before or after.

## 4. Owner checklist additions
- [ ] Удар о бордюр теперь мягче: пик силы на колесо ~8 кН вместо ~28 кН (`sedan.ron suspension.max_damper_speed: 0.5`).
      Если машина после бордюра подпрыгивает дольше, чем хочется, попробуй 1.0: пик 9,3 кН, подскок меньше.
- [ ] Выход у низкой стены / забора: выходишь с другой стороны или через крышу, а не на верх стены. Если зажат со всех
      сторон, остаёшься в машине.

## 5. Artifacts
- Probe (not a gate, removed from `tests/`): `scratch/fixer/zz_damper_probe.rs`.
- QA: `scratch/qa_t14_fixer2/` (`summary.json`, screenshots).
- Log: 2 decisions (cap value, exit-height rule with the roof kept).
- Files touched: `assets/vehicle/sedan.ron`, `vehicle/{chassis,config,mod,seat}.rs`,
  `tests/{vehicle,vehicle_seat,config_vehicle}.rs`. `git status` also shows `OPEN_DECISIONS.md` and new
  `maw/tasks/{blocked,pending}/TASK-026..029`. Those are not mine; the orchestrator writes them.

children: 0 launched / 0 reported.
