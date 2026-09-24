# FIX_SUMMARY — TASK-015 (GDD T14), fixer round

Input: `IMPL_REVIEW.md` (PASS, 5 minors + 3 missing gates) and the orchestrator's binding note: fix all of them. A
live owner report came in mid-round: the car catches a curb at speed. It was binding and had priority. Branch
`feature/t14-car`, nothing committed.

## 0. Preflight

- `scratch/` was read as a coverage map: `flips/`, `cr/golden/PROOF.txt`, `pr2/`, `probe_collision/`, `qa_t14/`.
- The claim most likely to break correct code if applied literally was item 4: "treat `GameLayer::Vehicle` as a
  blocker wherever World blocks". The seated driver's eye is at car centre + 0.75 m with a half height of 0.92 m, and
  the perception Car threat aims at the car's own centre. Both points are inside a car box, so a plain Vehicle mask
  makes cops blind to any driver and kills `ThreatKind::Car`. **Held, measured.** With that literal version,
  `fast_car_on_the_sidewalk_scares_people`, `witnessed_run_over_is_a_crime` and the new
  `a_cop_sees_the_driver_and_shoots_the_car` all go RED. My version skips any car that contains either end of the line.
- The review's damper prescription `(compression − previous)/dt` also breaks code as written. `previous` defaults to 0
  on a freshly spawned car, so every car gets kicked up at 0.83 m/s on its first tick. I measured this with the new
  spawn-kick row (below). My version uses no rate on the first grounded tick.

## 1. Fixed

### Owner report: the bumper catches the 0.15 m curb (binding, priority)
- **Reproduced headless first.** A car hits a city-style curb (a convex-hull `CityBlock` prism, top 0.15 m) at full
  throttle and while braking, at 10, 20 and 28 m/s, square on and at 30°, with run-ups of 3, 8 and 20 m (36 cases). On
  the owner's build (HEAD `chassis.rs`), 25 of 36 cases took damage: 20 m/s square 473..486 hp, 28 m/s square
  710..889 hp. The car was caught (lowest forward speed 0.2..2.3 m/s) or thrown back (−1.6 m/s). Raw rows are in
  `scratch/fixer/curb_hit_owner_build.txt`. A cuboid test slab did not reproduce it. The city's convex-hull blocks do.
- **Cause.** The chassis box bottom sits 0.24 m above the road at rest, only 0.09 m over the curb. Avian's
  speculative contact between the box's front-bottom edge and the curb's top edge has a diagonal normal. It stops the
  car before any wheel ray reaches the curb. The contact points are at y ≈ 0.195, halfway between 0.24 and 0.15. My
  compression-rate damper is not the cause: the owner's build shows the same numbers.
- **Fix (shape + clearance, all data).** The chassis collider is now a convex hull, defined in `sedan.ron` as
  `underbody: (lift: 0.2, chamfer_length: 0.5, chamfer_height: 0.3)`: the box bottom is raised 0.2 m, and the lower
  nose and tail are cut by a 0.5 × 0.3 m chamfer. Ground clearance at rest is now 0.44 m, which leaves 0.29 m over
  the curb.
  - `VehicleConfig::chassis_points()` and `chassis_volume()` (15.699 m³) keep the density exact (76.4 kg/m³, mass
    1200).
  - `validate` got two rules for the new values.
  - I isolated each part. Lift alone gives 0 damage in all 36 cases. The chamfer alone still gives 118..376 hp. Lift
    0.1 with the chamfer also gives 0. I kept lift 0.2 plus the chamfer as margin for nose dive and for taller steps.
- **After:** 0 damage in all 36 cases, and every run-up that reaches the curb ends on the sidewalk (centre y 1.31 =
  rest + 0.15). Raw rows are in `scratch/fixer/curb_hit_after.txt`.
- **Low steps never count as a damaging impact.** New data `damage.ron vehicle.scrape_normal: 0.5`. A contact whose
  normal (car → other) has at least 0.5 along the car's **down** axis is an underbody scrape: no damage and no
  `VehicleImpact`. Why this form:
  - It uses the car's own axis, not world vertical. A roof landing on a flipped car still counts as a crash.
  - Only the car whose underside is touched is spared. In a car-on-car pile-up, the lower car still takes the hit.
  - The chamfer normal has 0.86 along down, so it is a scrape. A wall hit has about 0.
  - With the old box geometry, this rule alone already made the curb cost 0 hp, but the car was still caught at
    0.92 m/s. So the geometry fixes the cause, and the rule is the second line of defence for landings and bottoming out.
- **Gates** (`tests/vehicle.rs`, G13):
  - `curb_at_20_mps_is_climbed_square` and `curb_at_20_mps_is_climbed_at_30_deg`: 0 damage, forward speed never below
    15 m/s (no catch, no rebound), ends on the sidewalk.
    - Flip (underbody lift 0, chamfer 0): RED, "caught or thrown back, slowest 0.92 m/s" (square) and 6.82 m/s (30°).
  - `flat_landing_is_not_a_crash`: dropped at 8 m/s, the underbody touches the floor (precondition: a
    `CollisionStart`), and health is unchanged.
    - Flip (skip the `scrape_normal` check): RED, "a flat landing cost 34.9".
- Tunnelling (G1/G2), rollover (G7), the seat gates and G4 stay green. Yaw inertia is re-derived for the hull: raised
  box minus two wedges = 2 181.9 kg·m². The tolerance is tightened from 5 % to 1 % and the gate is green.

### Review item 1: car-car impact → `VehicleImpact` for both cars
- `impact.rs`: the car-car branch loops over both cars (each with its own outward normal for the scrape rule) and
  writes one `VehicleImpact` per car at the same point and speed.
- Client `audio/cues.rs`: crashes are sorted with the driven car's first and deduplicated by point, so a car-car crash
  is one metal sound, not two. The shake needs no change: it already filters on the player's car.
- Gates:
  - `driven_car_rams_a_parked_one` and `parked_car_rolls_into_the_driven_one` (`tests/vehicle.rs`): both cars lose
    `(v − 5)·40 ± 3` from the pre-crash speed, and both get an impact at one point. The second test asserts (GATE
    BROKEN otherwise) that the driven car is the **second** body of avian's pair. I measured that the moving body comes
    first, so the parked car does the ramming.
    - Flip (impact for the first car only): both RED ("no crash impact for the driven car" / "the two impacts differ").
  - Client `car_crash_is_one_impact` (`audio/event_gate.rs`): two impacts at one point make 1 sound, a third at
    another point makes its own.
    - Flip (dedupe off): RED.

### Review item 2: damper on the compression rate
- `chassis.rs`: the rate is `(compression − previous)/dt` from the previous tick's `WheelState`, and 0 on the first
  grounded tick (a spawn or a landing has no previous compression). `spring_force(cfg, x, rate) = k·x + c·rate`,
  clamped at 0. Unit rows: k·0.1 ± c·0.5 = 2 664.8 ± 1 131 N, never pulls.
- **Numbers.** The equilibrium is unchanged because the damper is 0 at rest: x_eq 0.1104 m, rest height 1.15956 m
  (G5 measured), k 26 648, c 2 262. Settling on flat ground is the same damped system (ζ 0.4). `sedan.ron` notes that
  c acts on the compression rate.
- **G5 proof:** green. A new spawn-kick row requires |v_y| < 0.1 m/s after the first tick.
  - Flip (damper sign): RED ("tick 0: centre y 1.0925").
  - Flip (rate on the first grounded tick): RED ("spawn kick: 0.833 m/s").
- **Peak wheel force on a 0.15 m step** (probe `scratch/fixer/zz_curb_probe.rs`, coasting car, static wheel load
  2 943 N):

  | coasting speed | before (body-velocity damper) | after (compression-rate damper) |
  |---|---|---|
  | 5 m/s | 7.0 kN | 28.7 kN |
  | 10 m/s | 7.1 kN | 28.9 kN |

  **For the orchestrator and owner:** the standard model makes the step spike about 4× harsher (about 9.8× the static
  load). A vertical wheel ray sees the 0.15 m step as a compression jump inside one tick: 9.6 m/s × c. The car still
  climbs, the curb gates are green, and the peak hop is about 6 cm. How harsh this feels is an owner item. Two options
  if it is too harsh: cap the rate term (a new `sedan.ron` value), or use a shape cast per wheel so the jump is spread
  over the tyre radius. I made neither change unasked.

### Review item 3: entry only at speed ≤ `exit_max_speed`
- `seat.rs`: entry candidates are filtered by car speed ≤ `exit_max_speed`. The `sedan.ron` comment now says "Entry
  and exit".
- Gate `no_entry_into_a_rolling_car` (`tests/vehicle_seat.rs`): a driverless car at 6 m/s with its door at the
  player, then F: not entered. The precondition checks the car is still above the limit after the tick.
  - Flip (filter removed): RED.

### Review item 4: NPC sight and fire lines see cars
- `perception::sight_blocked` now casts against `[World, Vehicle]` and skips any car that contains either end of the
  line (`point_intersections` + `cast_ray_predicate`). Callers: perception (corpse, aimed, car threat), gang `sees`,
  `wanted::cop_sees` / `witnesses` (police sight and crime witnesses), and the `tactics::fire_line` spot check.
- **Dead end, logged.** I first changed all callers. t11 (seed 1) then left a Patrol stuck in Respond at 43.7 m for
  11.3 s; on HEAD that same cop arrived in 6.6 s. The cause is police `dest_clear` (direct seek vs route): it turned
  false behind a parked car. Movement tests now use the new `wall_blocked` (World only, the old rule): navigation
  `avoid_offset`, police `dest_clear`, gang `home_clear`, and population spawn cover. The rerun of t11 is below.
- Gates (`tests/vehicle_hits.rs`, G14):
  - `a_parked_car_blocks_sight_and_fire`: a cop in Attack 20 m from the player, 2 stars, 128 ticks.
    - Without a car it sees and fires (GATE BROKEN otherwise).
    - With a parked car broadside in front of the player: 0 ticks seen, 0 shots, 0 car dents. A per-tick GATE BROKEN
      checks that the cop → player line still crosses the car.
    - Flip (sight mask World only): RED.
  - `a_cop_sees_the_driver_and_shoots_the_car`: a cop 20 m ahead of a driving player sees them, and its bullets hit
    the car (Q2).
    - Flip (the car holding the driver's eye blocks): RED.
- Existing gang and police gates are green. The full `-p gta_sim` suite is below.

### Review item 5: a run-over writes `VehicleImpact` (sound + shake)
- `impact.rs`: the pedestrian branch writes `VehicleImpact { vehicle, point, speed: closing }` when damage > 0. A
  harmless push writes nothing.
- G4 extended: 10 m/s row has ≥ 1 impact of the car; 2.5 m/s row has 0.
  - Flip (write removed): RED on the 10 m/s row.

### Missing coverage
- (a) `a_sleeping_car_drives_off` (`tests/vehicle_seat.rs`): the car sleeps (GATE BROKEN otherwise), then drive_in,
  throttle 1 for 64 ticks, moved ≥ 1.5 m.
  - Flip (no `SleepingDisabled` insert): RED.
- (b) The car-car gates of item 1.
- (c) `blocked_left_door_exits_on_the_right`: a 4 m wall 0.1 m off the left side; F lands the player at the right
  door (offset·right > 1).
  - Flip (try only the left door): RED.
  - Extra row `boxed_in_driver_stays_seated`: both doors and the roof are blocked, so the player stays in the seat.
- Config: new sabotage rows, each with its own keyword: underbody negative, underbody too tall, `scrape_normal` 1.5.
  - Flip (checks removed): RED.

## 2. Skipped

- **Review item 2's prescription as written** (previous compression defaults to 0): replaced by "no rate on the first
  grounded tick". Reason in §0, gated by the spawn-kick row.
- **Review item 4's "cars block everywhere World blocks"**: applied to sight and shooting only. Movement stays on
  walls for the t11 regression above. Population spawn cover stays World-only too, because it is not NPC sight and a
  change there would move the density gates.
- **Nits** (unnamed exit-ray constants, hoisting the filter hash set, splitting `drive_vehicles`, reordering the
  perception car loop, asserting `GRAVITY` against avian, the `run_if(resource_exists::<City>)`): not in the
  orchestrator's list. No change.
- **Parking-spot crosswalk margin assertion** (the review called it acceptable): not in the list.
- Found while testing, not fixed: an exit next to a wall lower than about 3.1 m can place the player on top of the
  wall. The feet ray starts 2 m above the door point at car-centre height and finds the wall's top. The blocked-door
  gates use 4 m walls for this reason. This is a candidate for T15.

## 3. Test results

- `cargo test -p gta_sim -j 4`: **368 passed, 0 failed** (357 before plus 11 new gates).
  - `vehicle` 14 (incl. `curb_at_20_mps_is_climbed_square`, `curb_at_20_mps_is_climbed_at_30_deg`,
    `flat_landing_is_not_a_crash`, `driven_car_rams_a_parked_one`, `parked_car_rolls_into_the_driven_one`).
  - `vehicle_seat` 14 (incl. `no_entry_into_a_rolling_car`, `a_sleeping_car_drives_off`,
    `blocked_left_door_exits_on_the_right`, `boxed_in_driver_stays_seated`).
  - `vehicle_hits` 14 (incl. `a_parked_car_blocks_sight_and_fire`, `a_cop_sees_the_driver_and_shoots_the_car`).
  - `config_vehicle` 6.
  - Every gang, police and wanted test binary green.
- `cargo test -p citygen -j 4`: 32 passed.
- `cargo test -p gta_like --bin gta_like -j 4`: 77 passed, run 3 times after the client change (3/3 green), plus
  once more after the last sim change.
- `cargo clippy --workspace --all-targets -j 4 -- -D warnings`: clean. `python tools/qa/tree_check.py`: passed.
  `cargo tree -p gta_sim -e normal -i bevy_render`: empty.
- Every flip above was observed RED, then restored and GREEN. Flips were done by a temporary edit plus a copy-back;
  `git diff` was checked after each one.
- **Runtime** (release, `dev` features; no game process was running before or after):
  - `python tools/qa/scenarios/t14.py --out …/scratch/qa_t14_fixer`: **PASS**. 143 parked cars; entered; 3 s W →
    13.3 m/s, 24.1 m; engine 1; 142 minimap car dots; wall crash 1000 → 568.6 hp, stopped at z 697.96; exit 1.73 m
    from the car, `Playing`; no log errors. Frame cost with no vsync ≤ 3.8 ms.
  - `python tools/qa/scenarios/t11.py --out …/scratch/qa_t11_fixer` (final sight split): exit 0.
    - Arrest: reach 6.5 s / 9.9 s, busted 11.5 s, no stuck cop (TASK-014 baseline 9.9 / 11.6).
    - SWAT run: 7 reached; the "stuck in Attack at range" rows are the same as the HEAD-sight run
      (`scratch/qa_t11_fixer_headsight/`). No log errors.
- Files: `vehicle_hits.rs` 589, `vehicle_seat.rs` 441, `vehicle.rs` 438, `config.rs` 365 lines, all < 750.

## 4. Owner checklist additions
- [ ] Бордюр на скорости: машина заезжает на тротуар, без урона и без отскока. Теперь днище поднято на 0,2 м, нос и
      хвост скошены (`sedan.ron` `underbody`). Модель бампера висит ниже коллайдера и может визуально пройти через
      край бордюра.
- [ ] Жёсткость удара о бордюр: демпфер теперь считается от скорости сжатия, пик силы на ступеньке ~29 кН против 7 кН
      раньше. Если трясёт слишком сильно, скажи — есть два способа смягчить (см. §1, пункт 2).
- [ ] Столкновение с машиной: трясёт и один металлический звук. Удар по пешеходу: тоже звук и тряска.
- [ ] Припаркованная машина между тобой и копом — укрытие: коп не видит и не стреляет. Из своей машины тебя видно.

## 5. Artifacts
- Probes (not gates): `scratch/fixer/zz_curb_hit_probe.rs` and `scratch/fixer/zz_curb_probe.rs`; rows in
  `curb_hit_owner_build.txt`, `curb_hit_after.txt`, `curb_peak_before.txt` and `curb_peak_after.txt`.
- QA: `scratch/qa_t14_fixer/`, `scratch/qa_t11_fixer/`, `scratch/qa_t11_fixer_headsight/`.
- Log: 5 entries (4 decisions, 1 dead end). PCTX: 2 proposals (curb vs speculative margin,
  sight through cars).

children: 0 launched / 0 reported.
