# IMPL_SUMMARY — TASK-016 (GDD T15: traffic and police cars)

Branch `feature/t15-traffic` (commits `1708022` .. HEAD, pushed; Linux CI green, see Tests). Pre-flight: every file, type and API the plan
names exists with the assumed shape (`vehicle_bundle` holds `Name` + `RigidBody::Dynamic`, `Health::take(f32)`,
`BulletHitVehicle { shooter, attack, vehicle, point, damage }`, avian `cast_shape_predicate` /
`shape_intersections`, citygen `LaneGraph`, the `config_police.rs:98` and `new_city.rs:301` literals, the
cached car-kit zip). No PLAN_BLOCKED.

Cost of error: HIGH (silent class: infinite-mass pushes, two cars in one conflict point, despawn in view,
unit/car cap drift, leaked entities, frame time). Full evidence layer below; feel goes to the owner run.

## 1. What was implemented (78 files, +8696 / -171 lines outside `maw/`)

Data (`assets/`)
- `traffic/traffic.ron` (new, 26): IDM, desired speeds, turn speed, look-ahead, casts, connector samples,
  conflict margin, Vermeij bubble, TTC switch, lost rule. Strict loader + `validate(tick)` + cross rules.
- `police/escalation.ron` (+26/-): `cars:` per star row 1..5, `arrest.pull_out_seconds`, the `car:` block
  (PLAN fields + `blocked_seconds: 6.0`, see Deviations).
- `wanted/wanted.ron` `cop_car_view_distance: 50`; `vehicle/sedan.ron` `cabin`, `autopilot`;
  `vehicle/damage.ron` `cabin_driver_share: 0.5`; `audio/mix.ron` `siren.car_height`; `world/render.ron`
  `police_vehicle`, `taxi_vehicle`, `taxi_share`; `third_party/manifest.ron` police.glb + taxi.glb (hashes
  from the plan, `fetch_assets --check` passes).

Sim (`crates/gta_sim/src/`)
- `traffic/` (new domain, `TrafficPlugin`): `config.rs` 196, `graph.rs` 524 (slot-0 lanes, Bézier
  connectors with analytic tangents, conflict tables, spawn points, projection, `build_traffic_graph`),
  `idm.rs` 83 (`IDM_DELTA` law, ballistic step with stops), `drive.rs` 489 (`advance_traffic`, one pass in
  entity order), `junction.rs` 148 (FCFS reservations, don't-block-the-box), `contact.rs` 311 (swept TTC
  switch + `CollisionStart` backstop, pure SAT/circle fns), `spawn.rs` 237 (frame test, bubble, production
  spawn helper), `hijack.rs` 132 (driver thrown out, Taken/Abandoned), `bail.rs` 31 (`CabinHit` ->
  Bailing + `DriverScared`), `mod.rs` 285 (components, `TrafficRng` stream 4, intersections, stats,
  phase, sets, `abandon`).
- `vehicle/`: `autopilot.rs` 161 (pure pursuit + speed control, stuck reverse), `chassis.rs` (kinematic
  bodies skipped; car's own `DriveIntent`), `impact.rs` (cabin zone, driver wound, `CabinHit`),
  `seat.rs` (`exit_spots`, `pull_out` left door only, forced eject never on a wall top), `mod.rs`
  (`CabinHit`, set edges `Bullets.before(Death)`, `Drive.after(PoliceSystems)`), `config.rs`.
- `police/`: `cars.rs` 572 (components, `PoliceCarRng` stream 5, `next_car_state`, dismount, hijacked
  police car, re-boarding, nearer door), `car_dispatch.rs` 198 (car dispatcher, car bubble),
  `car_route.rs` 422 (lane A*, route following, senses, motion), `dispatch.rs` (crews count, seat
  reservation), `arrest.rs` (`ArrestAttempt.pull`, `pull_out_driver`), `behavior.rs` (arrest at the
  car door, re-board seek, cars in the fire line), `mod.rs` (config types/validation, wiring).
- `wanted/`: cars see at 50 m and witness (`track_search`, `record_crimes`), `DriverScared` -> Shooting,
  `cop_sees(.., distance)`.
- `tactics/fire_line.rs` (`CarRect`, `car_blocks`, `nearby_cars`), `tactics/mod.rs` (`hold_fire(.., cars)`),
  `gang/behavior.rs` (car list), `civilian/mod.rs` (`roll_temperament(&mut ChaCha8Rng)`), `lib.rs`.

Client (`src/`): `visuals/vehicle.rs` (sedan/police/taxi by role and appearance roll, per-model wheel
lift, unit test), `visuals/config.rs`, `audio/loops.rs` + `config.rs` (sirens on active police cars at the
roof, foot cops otherwise), `audio/event_gate.rs` (`sirens_ride_police_cars`), `minimap/markers.rs`
(`MarkerKind::PoliceCar`, police tint).

QA/docs: `tools/qa/scenarios/t15.py` (new), `t13.py` (siren carrier may be a police car), `t14.py`
(parked cars only), `t9.py` (a health drop next to a car is a run-over, not friendly fire);
`docs/design/GDD.md` (the four one-line amendments of step 33); `docs/architecture/traffic.md` (new).
Post-merge docs (narrative-graph, README, AGENTS) are left for after the merge.

## 2. Deviations from plan (each logged in `log.jsonl`)
1. Intersection request distance `v²/2b + 2·s0 + half_length` (`traffic/junction.rs:12`): the plan's
   `+ s0` equals IDM's rest point, so queue heads crept to the line (loop gate: 197 m in 100 s).
2. Traffic spawn near a lane end starts at `min(v0, √(2b·d_stop))` (`traffic/spawn.rs`) instead of
   skipping points closer than `v0²/2b + s0 + half` to the lane end: that skip needs 80 m of avenue and
   gave zero off-frame spawns in seed 1.
3. Lane approach cap in `advance_traffic`: with a chosen turn, `v0 = min(lane v0, √(turn² + 2b·to_end))`
   (the plan's connector v0 alone braked at -8 m/s² inside the connector).
4. `TrafficMode::Bailing { attack, shooter }`: the shooter entity is kept for the flee origin (plan text
   needs it, plan type lacked it).
5. Police cars (`car_route.rs`, `cars.rs`): A* goal = first lane within `max(nearest, 0.75·dismount)` of
   the target (the nearest lane often runs the wrong way: cars looped a block); `route_done` =
   remaining ≤ `dismount_distance`; slow to `turn_speed` before a turn and brake at
   `idm.max_deceleration`; cast `turn_sense_distance` on connectors; re-board at the nearer door (left or
   right); Chase ends when the driver has stopped `stopped_seconds` (so Respond can dismount for the
   1-star pull-out); **new data `car.blocked_seconds: 6.0`**: a responding car held up that long lets its
   crew go on foot (cars stuck behind traffic queues forever otherwise). `PoliceCar.blocked` field added.
6. AC1 loop runs around the floor perimeter (x, z within ±37) instead of the z 22..38 band (10 m corner
   lanes cannot hold a car plus the 6.08 m box room); AC2 + intersection centred (-22, 0, 22) with 12 m
   arms (the plan's band gives 4.75 m arms) and 6 m/s lanes.
7. Gate 30b (a): "no BulletHitVehicle on the corner car" is unreachable under PLAN R7 (no spread cone for
   cars): the gate asserts the aggregate car hits stay under half of the rule-off count (police 8 vs 24,
   gang 6 vs 17 over six geometries). File `tests/car_fire_lines.rs` instead of additions to
   `police_fire_lines.rs`.
8. Gate 23 row (d) uses a civilian on a sidewalk run (walk 1.8 m/s) at 1.2 m from the bumper.
9. Liveness numbers derived by measurement (plan asked): AC2 ≥ 3 cars per approach in 100 s (measured
   4-8: one car per ~4 s through a fully conflicting box at a = 0.73); bench limit 19 ms (probe mean
   1.86 ms).
10. `police_city` car variant window 80 s (foot variant keeps 40 s); `street_spawn` (2 rows) and
    `civilian_bench` turnover set `max_cars = 0` (subject is civilians; with traffic: 0.067 m nudge on a
    crosswalk spawn (CI), 130 m of 270 run, 46 < 48 deficit ticks). Other city gates with a view
    (`civilian_city`, `gang_city`, `witness_city`, `civilian_bench` fps row) passed with traffic.
11. Flip "count only foot units in the car dispatcher -> row 2 overruns" stays GREEN (every shipped
    row has cars × crew ≤ units); seat-reservation flip is RED on rows 3-5. Flip "leader search own
    segment" stays GREEN on the loop (the destination-room rule spaces cars at every corner); replaced
    by "leaders ignored" (RED).

## 3. Test results
- `cargo build -j 4`: ok. `cargo clippy -j 4 -- -D warnings` and `cargo clippy --workspace --all-targets
  -j 4 -- -D warnings`: clean.
- `cargo test -p gta_sim -j 4` (run as two halves + `--lib`): every target ok (lib 82; 53 test targets).
- `cargo test -p citygen -j 4`: ok. `cargo test -p gta_like --bin gta_like -j 4` ×3: 79 passed each.
- `python tools/qa/tree_check.py`: passed; `cargo tree -p gta_sim -e normal -i bevy_render`: nothing.
- Runtime (release, `--settings-id .qa` via `brp.py`, scratch/qa_*): **t15 PASS** (34 traffic cars,
  mean 7.9 m/s, max 13.8; hijack + fleeing driver; chase 60.9 -> 9.95 m, ≤ 2 active cars; 2 Dismounted
  cars with 4 `CrewOf` cops; frame cost 2.87 ms no-vsync, 30 FPS = the 30 Hz `\\.\DISPLAY9` under Fifo).
  Named mutation in t15: heat re-raised if the level lapses (driving off unseen cleared 2 stars in run 2).
  t8, t10, t11, t12, t14 PASS; t9 PASS after the run-over attribution (first run failed: a member at
  14 HP, loss 86 = a 10 m/s car hit or 7 SMG pellets, not proven); t13 PASS on run 4 of 4 (runs 1-3 stopped
  on the known flaky harness precondition "six clicks did not fire six shots", TASK-015 FIX_SUMMARY).
- Bench `traffic_bench` (seed 1, 21 traffic cars, 5 police cars, 12 units, 40 civilians, player driving):
  mean 1.86 ms, p95 2.14, max 2.41 per tick (GDD §11: physics + AI ≤ 4 ms). `police_bench` (cars 0):
  1.60 ms.
- CI: push `6824f71`: repo checks, citygen, client, clippy green; sim gates red on
  `street_spawn::no_spawn_in_clear_view_over_60s_turning` (the crosswalk nudge) -> fixed; run 36096280552
  (after the fix): all jobs green incl. sim gates. The last pushes (turnover bench, summary) were still
  running at hand-off (`gh run list --repo pockerhead/MAW-make-GTA --branch feature/t15-traffic`).

Flip-RED log (each: sabotage -> RED observed -> restored -> GREEN)
- 21 `traffic_idm`: drop `v'<0` branch -> RED (tick 1, speed -0.0342); leaders ignored -> RED (overlap
  tick 270).
- 22 `traffic_intersection`: grant always -> RED; conflicts without the geometric test -> RED.
- 23 `traffic_contact` (`scratch/flip_contact.py`): backstop only -> rear-end/walk/sleeping-ahead RED;
  PLAN's closing-speed rule -> oncoming/parked/pedestrian RED; inside-reach -> same three RED; skip
  sleeping -> sleeping-ahead RED.
- 24 `traffic_bubble` (`scratch/flip_runner.py`): no offscreen condition -> 3 rows RED; bands swapped ->
  bands row RED; abandoned never despawn -> abandoned row RED (and `sensor_leak` traffic row RED).
- 25 `traffic_hijack`: keep Autopilot/DriveIntent -> dynamic row RED; driver on every entry -> re-entry
  row RED. 25b `traffic_bailout`: cabin test removed -> hood row RED; `DriverScared` unread -> crime/heat RED.
- 26 `traffic_graph`: straight chord connectors -> block clip RED; `config_traffic`: one sabotage per rule.
- 28 `police_cars`: seat reservation off -> rows 3-5 RED; "Dismounted -> Respond without crew" -> dead crew
  RED; car sight at foot distance -> `car_sees_at_50_m` RED; car witness off -> theft row RED.
- 27 `new_city`: `reset_traffic` off -> A25 RED; `CabinHit` not cleared -> A29 RED. `vehicle_hits` roof row:
  head sensor on while driving -> RED.
- 29 `police_pull_out`: speed check off -> moving row RED; pull through any exit -> blocked-door row RED.
- 30 cabin: share 0 -> window/roof rows RED; zone check off -> fender/low-door rows RED.
- 30b `car_fire_lines` (`scratch/flip_fire_lines.py`): empty car list (police / gang) -> RED; exclude
  nothing -> driven-car row RED; spread cone on cars -> beside row RED; cars up to reach -> behind row RED.
- 30d `traffic_parked`: traffic also on curb lanes -> RED (seed 2).
- 31 `vehicle_seat`: old forced fallback -> both rows RED. Client `sirens_ride_police_cars`: cars ignored
  -> RED.

## 4. How to verify manually
- `python tools/fetch_assets.py` (police.glb, taxi.glb), `cargo run --release -- --seed 1`.
- Owner checklist: streets have moving cars that queue and yield at crossings; jack a car from the flow
  (stand in front of it, then F at the door) and its driver runs; shoot into a traffic car's side window:
  the driver brakes, gets out and runs; taxis in the flow; get 2 stars in a car: police cars follow, ram,
  cops get out when you stop and get back in when you drive off; sirens ride the police cars; police and
  taxi model look and scale; at 1 star stop the car next to a cop: he pulls you out and arrests you.
- Open risks for the owner run: police cars wait behind traffic queues (then go on foot after 6 s);
  one car per ~4 s through a busy crossing (IDM a = 0.73 from rest); jams behind abandoned cars (no lane
  change); a cop reaching the far side of a car walks around it; siren height; first cops reach a park
  centre by car in ~63 s (1.3 s on foot before T15).

children: 0 launched / 0 reported.
