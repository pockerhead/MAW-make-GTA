# IMPL_REVIEW — TASK-015 (GDD T14, drivable car)

Reviewer: code-reviewer (claude opus, effort high). Reviewed commit `a070717` on `feature/t14-car` against
`TASK_FINAL.md`, `PLAN_FINAL.md` and `IMPL_SUMMARY.md`. All 74 changed files were checked. The sim domain
(`vehicle/*`, combat/wanted/perception/arrest/pickups diffs, citygen, the vendored fix, every new gate) was read
in full. The client files (input, camera, visuals, audio, HUD, minimap, shake) were read through their diffs.

## 1. Verdict

**PASS**, with minor follow-ups. No critical or major defect found. The driver state stays consistent across
Wasted, Busted, a new city and a vanished car, and headless gates prove it. The vendored fix is one line and has
an order-asserted gate. I reproduced the golden rebless independently: the geometry is unchanged. Parked cars
sleep and cast no rays. Everything claimed green is green when I run it.

### Disconfirmation (done first)
**Counter-example tested:** the player gets Wasted, Busted, or loses the car while driving, and one of the
seat components survives. That would be the head hitbox `ColliderDisabled`, `RigidBodyDisabled` or `TnuaToggle`.
Or the ungated `sync_seats` re-pins the player to the car after the respawn teleport.
- `seat.rs:76-100` `leave()` removes `Driving`, `RigidBodyDisabled`, `ColliderDisabled` and `TnuaToggle` from the
  body, and removes `ColliderDisabled` from every `HeadHitbox` child. `eject_all` runs on
  `OnEnter(Wasted|Busted)` (`mod.rs:239-240`), and `sync_seats` has a vanished-car fallback (`seat.rs:245-252`).
  All three paths call `leave`. The commands apply before the next `FixedPostUpdate`, so `sync_seats` no longer
  sees `Driving`.
- The player death path (`flow/wasted.rs detect_player_death`) inserts only `Dead`. It never uses `TnuaToggle`,
  so `leave` removing the toggle cannot revive a corpse-style toggle. Only civilians get `TnuaToggle`
  (`population/mod.rs:239`).
- `vehicle_seat.rs::assert_on_foot` checks all four, including the head. G8a/c/d run it in Wasted, in Busted,
  after the respawn and after the despawn. The recorded flips "no eject" and "no fallback" are RED
  (`scratch/flips/flip_results.txt`).
- **The counter-example did not hold.**

A second counter-example targeted the golden rebless: the geometry changed and the rebless hid it. I copied
`crates/citygen` from HEAD into `scratch/cr/golden/`, put back the pre-task `hash.rs` (schema 2, no parking lines)
and the pre-task `golden_hashes.txt`, and ran `cargo test --test golden`. **GREEN.** The new generator reproduces
all three old hashes, so every field hashed before T14 is unchanged. Only the appended parking list and the
schema bump moved the hashes (`scratch/cr/golden/PROOF.txt`). This did not hold either.

### Log triage
The log has no `dead_end` from the implementer. The only `dead_end` is plan-reviewer-2's G12(c) fixture, and the
implemented G12c uses the replacement fixture (platform, about 55°, preconditions). Its flip "head left enabled"
is recorded RED with 4 headshots. I checked the implementer's decision about query order against
`bevy_ecs-0.19.1/src/query/state.rs:575-600`, and it is correct. When a query has required components, new
archetypes are matched through the component-index `HashMap`, so matching within one batch runs in hash order and
not in creation order. The fix in `tnua_motor.rs:57-72` is sound: a `QueryState` created before the corpse, with
the corpse archetype and the `LateTable` archetype appearing in different ticks. The roll-lever sign deviation is
also correct: `point = contact + up·(1−ri)·h` leaves a lever of `ri·h = 0.198 m`, which is the plan's worked number.

### Commands run (foreground)
- `cargo test -p gta_sim -j 4`: all binaries ok (vehicle 9, vehicle_seat 10, vehicle_hits 12, vehicle_city 2,
  tnua_motor 2, config_vehicle 6, unit 66, the rest unchanged).
- `cargo test -p citygen -j 4`: all ok.
- `cargo test -p gta_like --bin gta_like -j 4`: 76 passed.
- `cargo clippy --workspace --all-targets -j 4 -- -D warnings`: clean.
- `cargo tree -p gta_sim -e normal -i bevy_render`: empty.
- `python tools/qa/tree_check.py`: passed.
- Largest touched files: `wanted/crimes.rs` 487, `tests/vehicle_hits.rs` 477. All are under 750.

## 2. Confirmed correct

- **Vendored Tnua fix.** `vendor/bevy-tnua-avian3d-0.12.1/src/lib.rs:430` changes `return` to `continue`, and
  nothing else in the vendor tree changed. ADR-001 records both changes and the update procedure, including
  `--test tnua_motor`. The gate `tests/tnua_motor.rs` asserts the corpse comes before the walker in a QueryState
  that matches incrementally, like the motor system's own. It also has a control test without a corpse. The
  recorded flip goes RED with `return`.
- **Enter/exit state machine** (`seat.rs`).
  - `mem::take` of `vehicle_requested` runs before the Dead / Cuffed / knocked-down checks.
  - On enter, `*action = ActionIntent::default()` clears held fire, and `Enter.before(HealthSystems::Damage)`
    (`mod.rs:205-207`) makes that reset precede `fire_weapons`. G3f flips RED with 6 SMG shots.
  - The exit speed gate is `exit_max_speed`.
  - The exit candidates are the left door, the right door and the roof, with a capsule overlap test against
    `[World, Vehicle, Character]`.
  - The `driver` link is cleaned from both sides, including the orphan sweep in `sync_seats:260-268`.
- **Sleeping parked cars.**
  - `drive_vehicles` skips `Sleeping` bodies before casting any ray (`chassis.rs:95`).
  - Undriven cars use `forces.non_waking()` (`chassis.rs:178-183`).
  - Entering inserts `SleepingDisabled`. I verified in avian 0.7.0 `islands/sleeping.rs:164-181` and `:260-266`
    that the physics step wakes an island containing a `SleepingDisabled` body, so a parked car starts driving
    one tick after entry.
  - G10 asserts 143 of 143 parked cars asleep with `VehicleLoad.rays == 0`. Its flip "waking forces" goes RED.
- **Per-tick work is bounded.** Each awake car casts 4 rays, and the car count is capped by the parking-spot
  count. `record_pre_step` is one copy per body. The perception car loop is O(civilians in slot × cars), cheap
  checks first, with a ray only for a car on the sidewalk, fast and near.
- **Crash impacts** (`impact.rs`).
  - Velocities come from before the step: `record_pre_step` runs before `PhysicsSystems::First`, and
    `CollisionStart` of step N is read in FixedUpdate N+1, before the next record overwrites them. This is
    consistent.
  - The normal is oriented by `own_collider`.
  - Pedestrian damage uses the striker rule. `DamageDealt` goes through the existing pipeline with
    `shooter = driver.unwrap_or(car)`.
  - Every `DamageDealt.shooter` consumer uses `get` / `contains` and never panics on a car entity. I grepped all
    of them in sim and client.
- **Bullets vs car (Q2 c+a).**
  - `hitscan.rs` stops pellets on `GameLayer::Vehicle` and writes `BulletHitVehicle` without a `CombatRng` draw.
  - `apply_bullet_hits` clamps health at 0.
  - The driver is protected by the head `ColliderDisabled`, and G12c proves it from 55°.
- **Wanted.** `HitSource` replaces `melee: bool`. `classify_table` has 19 rows (12 old + 7 Vehicle). `RunOver` and
  `CarTheft` go through `touched` (cop line-of-sight witness only, Q4). The `HEAT` test const was updated.
- **Arrest and pickups** are excluded while driving (`Without<Driving>`), and G8b places the cop 1.25 m from the
  seat, which is within arrest distance.
- **Data first.** All tuning lives in `sedan.ron`, `damage.ron`, `city.ron`, `perception.ron`, `wanted.ron`,
  `camera.ron`, `render.ron`, `juice.ron`, `mix.ron` and `strings.ron`, each with `deny_unknown_fields` and
  validation. The new `const`s are all law: `GRAVITY`, `WHEELS`, `MODEL_YAW`, `SoundClass::COUNT` and the paths.
  A compose-time check `parking_fits` ties `curb_offset` to the car width.
- **citygen.** Parking uses its own stream (`PARKING = 16`, one stream per edge) and is called last in `generate`.
  The property test checks the lane offset, the end margins, parallel headings and spacing, and that no spot lies
  inside a block. G10 is the physical overlap check: every car is still within 0.05 m of its spot after settling.
- **Composition.** `VehiclePlugin` is a separate `add_plugins` call after the 15-tuple. Required components are
  registered in `build` before any `Character` exists. Messages are cleared on `NEW_CITY`.
- **Client input.** `sync_contexts` inserts `ContextActivity` only on a change, and its table test covers every
  state. `ExitVehicle` and `EnterVehicle` both use `require_reset` (semantics checked in
  `bevy_enhanced_input-0.26.0/src/action.rs:178-186`). `release_held_actions` zeroes `DriveIntent`.

## 3. Issues

| # | Severity | Where | Description | Suggested fix |
|---|---|---|---|---|
| 1 | minor | `crates/gta_sim/src/vehicle/impact.rs:122-132`; consumers `src/juice/shake.rs` (crash rows), `src/audio/cues.rs` (own-crash branch) | A car-car pair writes one `VehicleImpact` for `vehicle` = whichever of `body1`/`body2` is checked first. When the player's car is the `other` side (about half the rams into a parked car, since both are ordinary `Vehicle`s), the client sees no impact "of the player's car". The crash shake is skipped, and the metal sound falls back to the audibility check. Both cars' health is still reduced correctly. | Write a `VehicleImpact` for each car in a car-car pair (same point and speed), or add `other: Option<Entity>` and match either side in the client. |
| 2 | minor (owner/feel) | `crates/gta_sim/src/vehicle/chassis.rs:139-140` | The damper term uses the mount's body velocity along up, not the rate of change of compression. On a step (the 0.15 m curb) or on a moving surface (another car's roof), the damper does not resist the compression jump, so the spring kicks at `k·0.15 ≈ 4 kN` per wheel. G5 is green because the floor is flat and static. | Leave for the owner run ("curb impact", already on the checklist). If it feels harsh, damp `(compression − previous)/dt` stored in `WheelState`. |
| 3 | minor (design) | `crates/gta_sim/src/vehicle/seat.rs:157-172` | Entry has no car-speed check. A driverless car rolling past at speed, with its door within 2.5 m, teleports the player in. The plan did not ask for a check. | Owner decision. If unwanted, filter candidates by `car_vel.length() <= exit_max_speed`. |
| 4 | minor (design, T15) | `combat/hitscan.rs` filter vs NPC hold-fire and sight checks (World only) | Cars now stop bullets, but NPC line-of-fire and sight checks do not see cars. Cops and gangs shoot into a parked car between them and the player and dent it. This works as emergent cover. It is noted because the plan's rollout note mentions only the player. | Record for T15; no change now. |
| 5 | minor | `crates/gta_sim/src/vehicle/impact.rs:120` | A pedestrian hit writes `VehicleHit` + `DamageDealt` but no `VehicleImpact`. A run-over that does not kill makes no impact sound and no shake. | Owner checklist item ("сбить пешехода"). Add a cue only if the owner asks. |

No critical or major issues.

## 4. Missing coverage

- **Entering a sleeping parked car and driving it (headless).** Every G3/G5/G7 car is entered one tick after
  spawn, while still awake. G9 enters a parked city car but never applies throttle. The wake path (insert
  `SleepingDisabled`, then avian wakes the island next step, then `drive_vehicles` stops skipping) is proven only
  by runtime `t14.py`. This path breaks silently: if avian's wake rule changes, parked cars ignore the throttle.
  Suggested gate: spawn a car, run until `Sleeping`, `drive_in`, throttle 1 for 64 ticks, displacement ≥ 1.5 m.
  Flip: remove the `SleepingDisabled` insert.
- **Car-to-car crash.** No gate proves both cars lose `(closing − 5)·40`, or that the normal orientation holds
  when the driven car is `collider2`. Issue 1 lives in the same branch.
- **Exit with the left door blocked.** Nothing checks the fallback to the right door or the roof, or that the
  player stays seated when all three are blocked.
- **Parking spot clearance from crosswalk edges.** The property test checks only that the spot centre is outside
  a block. The crosswalk margin (car end ≥ 12.96 m from the node vs about 8.5 m for a crossing) is covered only
  indirectly by G10's "cars stay on their spots". That is acceptable, but the margin is not asserted.

## 5. Nits

- `seat.rs:50` exit feet ray: `2.0` m lift and `6.0` m length are unnamed geometry. They are not tuning, but one
  line of naming would help.
- `chassis.rs:115-116`: `with_excluded_entities([entity])` builds a hash set per awake car per tick. This is
  cheap at the current car counts. It could be hoisted if T15 adds traffic.
- `chassis.rs:79-185` `drive_vehicles` is about 105 lines. Splitting out a per-wheel `wheel_forces(...) ->
  (normal, tyre)` would make the one pure part unit-testable.
- `perception/mod.rs` car loop: it computes `distance` before the cheaper `on_sidewalk` check. Reorder.
- `config.rs:10` `GRAVITY` duplicates avian's `Gravity::default()`. If a `Gravity` resource is ever set, the
  rest height and the visual wheel rest diverge silently. A one-line comment exists; asserting equality at
  compose time would make it a law check.
- `mod.rs:247` `spawn_parked_cars.run_if(resource_exists::<City>)` is a per-system resource gate. It is fine here
  because it selects the world source, not a state.

children: 0 launched / 0 reported.
