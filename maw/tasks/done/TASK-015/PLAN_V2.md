# PLAN_V2 — TASK-015 (GDD T14): drivable car

Reviewer: plan-reviewer-1. Base: `PLAN.md` (planner) + binding orchestrator decisions in `TASK_FINAL.md`
(Q1-Q5 resolved, **Q2 changed**: bullets stop on the car AND damage its health; **vendored Tnua motor bug is fixed
in this task with a gate**).

Cost of error, one line: physics (tunnelling, suspension equilibrium, rollover, impact speed), the enter/exit link,
Wasted/Busted/new-city while driving, bullet→car damage, the Tnua motor fix and the parked-car perf budget break
SILENTLY → full gates; handling, camera, engine sound, smoke and visuals are seen on the first frame → owner checklist.

---

## 0. Disconfirmation (done before the review)

**Counter-example written first:** the binding Tnua gate "with a corpse spawned first, a walking character still
moves the expected distance; flip-RED with `return`" stays GREEN under the bug, because iteration order is not spawn
order.

**Searched:** `bevy_ecs-0.19.1/src/query/state.rs:93-95,649-658` — `matched_storage_ids` is appended as tables /
archetypes are created and iterated in that order. `apply_motors_system` (`vendor/.../lib.rs:419-451`) iterates
`Query<(&TnuaMotor, Forces, Option<&TnuaToggle>, ..)>` with `iter_mut()`. A corpse is a live character that gets
`corpse_components()` inserted (`civilian/mod.rs:254`, `gang/behavior.rs:580`, `police/behavior.rs:438`) → it moves
into a NEW table created at death. Any walker whose table already existed (the player, spawned at Loading→Playing; or a
second civilian, which joins the old civilian table) is iterated BEFORE the corpse and never hit by the `return`.
I also checked the corpse still matches the query: `Forces` needs `VelocityIntegrationData` (required by `SolverBody`,
`integrator/mod.rs:48`), and removing `SolverBody` when the body turns static (`solver_body/plugin.rs:145,177`) does not
remove the required component, so a corpse stays in the motor query and the bug is real (also still present upstream
`idanarye/bevy-tnua` main, fetched 2026-09-24).

**Held:** yes. The gate as phrased in TASK_FINAL can be vacuous. Fix in Step 0 below (fresh table for the walker +
asserted iteration order).

---

## 1. Review notes (issues in PLAN.md, with evidence)

1. **Binding decisions not folded in.** PLAN keeps Q2 = (a) "car is cover, bullets do nothing" (§R7, Step 2
   "a car has no Health") and R9 "latent bug not fixed here". TASK_FINAL overrides both. → New Step 0 (vendored fix +
   gate), new Step 7b (bullet → car health), new gate G12, ADR-001 update.
2. **Tnua gate design is vacuous as written** (see §0). Fix: fresh table + order assertion.
3. **ADR-001 says "Единственное изменение … удаление строки debug-plugin"** (`docs/decisions/ADR-001-vendored-tnua-avian3d.md:9`)
   and the upgrade procedure (`:17`) says "удалить только указанную строку". Without an update the next re-vendor
   silently drops the motor fix. Must be updated.
4. **The reason to avoid `TnuaToggle` for the driver is gone** once the fix lands. And there is a reason to use it: the
   driver's ground sensor excludes only its owner (`lib.rs:321`) and `CollisionLayers::new(Character, ALL)` interacts
   with `Vehicle`, so the seated driver's sensor casts from inside the chassis every tick and Tnua keeps writing a
   motor boost on the disabled body. Decision: driver also gets `TnuaToggle::Disabled` (motors `continue`, sensors
   `return` per entity, trackers `continue`).
5. **Driver can shoot its own car.** `fire_weapons` shooters are `(With<Character>, Without<Dead>)`
   (`hitscan.rs:146-157`) and the pellet filter will now include `Vehicle`; `visible` excludes only the shooter's own
   colliders. A latched `fire_held` at the moment of entering would fire into the car body. PLAN never resets the
   action latches on enter. Fix: `enter` resets `ActionIntent` (like `drop_queued_input`), and G3 has a row for it.
6. **`asset_manifest.rs` would break:** `shipped_manifest_is_valid` asserts every non-audio, non-character, non-inter
   pack has exactly 4 files (`tests/asset_manifest.rs:101-106`). `car-kit` has 3 → the loop fails. PLAN only adds a
   `("car-kit", 3)` row. Fix: exclude `car-kit` from the 4-file loop and assert it separately.
7. **Plugin tuple at the arity limit.** `compose_sim` already adds 14 plugins in one tuple; bevy_app 0.19.1 implements
   `Plugins` for tuples up to 15 (`plugin.rs:186-193`). `VehiclePlugin` makes 15 — compiles, but T15 (`TrafficPlugin`)
   will not. Put `VehiclePlugin` in a second `add_plugins` call (no nesting games), one line.
8. **DamageDealt consumers not audited.** Car impacts write `DamageDealt` with `shooter = driver | vehicle`. Readers:
   `perception` (Hurt stimulus — wanted), `police/behavior.rs:42` (cop hit by player → hostile — wanted),
   `gang/behavior.rs:57` (provocation via `Faction` of shooter: a driverless vehicle has no `Faction` → skipped, fine),
   `wanted/crimes.rs:171` (`players.get(shooter)` → driverless car = no crime, fine), client `audio/cues.rs:296`
   (death sound on kill), `hud/weapon.rs:172` (hit marker), `juice/damage_numbers.rs`, `juice/damage_arc.rs`,
   `juice/shake.rs`. All behave sensibly; the plan must state it so the implementer does not add special cases.
9. **Wrong line numbers / facts** (symbols exist, cites are off): `ActionIntent` is `character/intent.rs:38-45` (file
   has 77 lines, not :286-295); `AttackSerial::next_id` is `combat/mod.rs:33-38`; `respawn_at` is `flow/wasted.rs:119`;
   `HeatTable` is `wanted/mod.rs:25-34`; client `apply_mouse_look` `camera/mod.rs:78`, `follow_player` `:101`,
   `enable_player_interpolation` `:192` (file is 194 lines, not :320/:339-406); `MarkerKind` `minimap/markers.rs:20`,
   `sync_markers` `:96`; `add_trauma` `juice/shake.rs:22`. There is no `HEAT` const in `crates/gta_sim/tests/` (grep
   empty) — "update the HEAT const" is a no-op unless a unit test in `src/` has one.
10. **Ordering of the vehicle bullet system and the drive system** was undefined (new with Q2): a stall from bullets
    must stop throttle on the same tick → `apply_bullet_hits` after `HealthSystems::Damage`, before `VehicleSystems::Drive`.
11. Everything else I spot-checked holds: avian 0.7.0 `PhysicsSystems::{First..Last}` (`schedule/mod.rs:162-176`),
    physics in `FixedPostUpdate` (`lib.rs:753`), `CollisionStart { collider1, collider2, body1, body2 }` is both
    `Message` and `EntityEvent` (`collision_events.rs:169-187`, `add_message` `narrow_phase/mod.rs:120`),
    `Collisions::get` (`system_param.rs:71`), `Forces::non_waking` (`query_data.rs:153`), `velocity_at_point`,
    `apply_force_at_point`, `RigidBodyDisabled` keeps spatial queries (`rigid_body/mod.rs:330-380`), `ColliderDisabled`
    removes from spatial queries, own entity only (`collider/mod.rs:344-353`), `SleepThreshold` 0.15 scaled by length
    unit, user writes of `LinearVelocity` wake a sleeping island (`islands/sleeping.rs:566-608` — so `kick` helpers need
    no explicit `WakeBody`), BEI 0.26 `ContextActivity::{ACTIVE, INACTIVE}` + `Deref<bool>` (`context.rs:732-744`),
    `ActionSettings.require_reset` (`action.rs:186`), `App::register_required_components` (errors if `Character` was
    ever added to an entity → call it in `VehiclePlugin::build`, before any spawn), citygen stream tag 16 is free
    (`rng.rs:6-20`), `chance`/`stream` exist, `HASH_SCHEMA_VERSION = 2` and `player_spawn` is the last hashed field
    (`hash.rs:4,154`), `half_carriageway(Avenue) = 2·3.25 = 6.5` → spot 4.875 m from the centre line = outer-lane centre,
    cached zips match their sha256 (`fac7dac…`, `029d734…`), impact pack 26 files → 31 with 5 metal files.
12. Physics numbers re-derived and correct: k = 26 649 N/m, c = 2 262 N·s/m, x_eq = 0.1104 m, rest height 1.1596 m,
    stable at 64 Hz (ω·dt = 0.147). Test fixtures checked against `world/test_area.rs:9-23`: G1 lane x ∈ ±1.2 from
    z −12 to the wall is clear; G2 car at (17.4, −17.4) rotated spans x 15.1..19.7, clear of the box x 8.5..11.5; G3/G4/G5
    at x = −25 clear of the ramp (x −12..−8); G11 block x −35..−15, z −25..−15 clear. Capsule radius 0.3 → G4 gap 1.0 m ✓.

---

## 2. Updated understanding (existing code)

### Simulation (`crates/gta_sim`)
- `lib.rs:41-174` `compose_sim`: every RON via `load_config` + `validate`; one `add_plugins` tuple of 14 plugins
  (Flow, `PhysicsPlugins::default()`, `TnuaAvian3dPlugin::new(FixedUpdate)`, Character, World, Player, Combat,
  Navigation, Perception, Population, Civilian, Gang, Police, Wanted). No `vehicle/`, no `assets/vehicle/`.
- `layers.rs:5-9` `GameLayer { World (default), Character, Hitbox }`; GDD §12 law also lists `Vehicle`.
- `character/mod.rs:32-44` `Character` requires `MoveIntent, AimIntent, ActionIntent, JumpBuffer, AnimState,
  HitReaction, Melee, CityScoped`; `head_hitbox` (`:137-147`): `Sensor`, `CollisionLayers::new(Hitbox, NONE)`;
  `character_components` (`:152-176`): dynamic capsule r 0.3 h 1.5, `CollisionLayers::new(Character, ALL)`,
  `TnuaAvian3dSensorShape` cylinder; `drive_characters` (`:179`) in `TnuaUserControlsSystems`; `HealthSystems
  {Damage, Regen, Pickup, Death}` chained in `FixedUpdate` (`:100-109`); `despawn_tnua_sensors` observer.
- `character/intent.rs:38-45` `ActionIntent { fire_held, fire_requested, reload_requested, select, cycle }`.
- `flow/mod.rs`: `PlayingSystems` (Playing), `NpcSystems` (Playing|Wasted|Busted), `NEW_CITY` (`:65`, Paused→Loading);
  OnExit Wasted/Busted run `respawn_*`, `drop_queued_damage` (clears `DebugDamage` only), `drop_queued_input`
  (`wasted.rs:163`, resets `ActionIntent` wholesale — a new field is cleared automatically).
- `world/test_area.rs:9-23`: 80×1×80 floor (top y 0, x/z ∈ ±40), boxes at (10|13|16, *, 10), ramp (−10, 2.327,
  −16.43) rotated 30°, box 4×5×4 at (−10, 2.5, −22.66), box 3×1.6×6 at (10, 0.8, −17.4), wall 12×4×0.5 at (0, 2, 14)
  (near face z 13.75), stairs at x = 10, z −12.15..−14.55. `STATION_SPAWN (−20, 0, 20)`.
- `world/city.rs:52,234` `CityBlock` static prisms (block up to the curb, 0.15 m).
- `combat/hitscan.rs:141-297` `fire_weapons` (in `HealthSystems::Damage` ∩ `PlayingSystems`, `combat/mod.rs:91-120`):
  filter `[World, Character, Hitbox]` (`:167-168`); a pellet hit resolves `ColliderOf.body`; `Health` target → RNG damage
  roll (`roll_damage`, one `CombatRng` draw) → `DamageDealt`; otherwise `TraceHit::World`, no draw.
- `combat/melee.rs:270` `HitReaction::escalate(knockdown, cfg)`, `:549` `knock_back(controller, shove)`.
- `population/mod.rs:235-243` `corpse_components() = (Dead, Corpse, TnuaToggle::Disabled, RigidBody::Static,
  CollisionLayers::NONE)`.
- `perception/mod.rs:54-60` `ThreatKind {Gunshot, Fight, Corpse, Aimed, Hurt}`; GDD §6.2 "машина на тротуаре" not
  implemented.
- `wanted/crimes.rs:17-25` `Crime` (no car rows), `classify(victim, melee, killed)` `:149`, `record_crimes` `:162`
  (13 params; +2 readers = 15 ≤ 16), witness = cop LOS `witnesses` (`search.rs:56`, World-only sight);
  `wanted/mod.rs:25-34` `HeatTable`; `assets/wanted/wanted.ron:2-3` "Car rows come with T14/T15".
- `police/arrest.rs:32` player query `(With<Player>, Without<Dead>)`; GDD §6.4: no arrest in a car.
- `combat/pickups.rs:57,132,197` player queries `(With<Player>, Without<Dead>)`.
- Vendored `vendor/bevy-tnua-avian3d-0.12.1/src/lib.rs:427-431`: `return` on `Disabled | SenseOnly` (the bug);
  sensors `:171-175` `return` inside a per-entity closure (correct); trackers `:117-121` `continue` (correct).
  `Cargo.toml [patch.crates-io]` points at it; ADR-001 documents one change only.

### Client (`src/`)
- `input/mod.rs`: context `OnFoot` (`:15`, actions `:102-…`, `Look` = mouse motion `:104`); `deactivate_input` /
  `activate_input` (`:142-151`) toggle `OnFoot` on Paused; `release_held_actions` (`:235`).
- `camera/mod.rs`: `apply_mouse_look` (`:78`, `Single<&Action<Look>>`), `follow_player` (`:101`),
  `enable_player_interpolation` (`:192`).
- `minimap/markers.rs:20` `MarkerKind`, `:96` `sync_markers`; `audio/cues.rs:21` `SoundClass`, `COUNT = 8` (`:34`),
  `play_impacts` (`:291`); `juice/shake.rs:22` `add_trauma`; `hud/mod.rs:54` `HudBar`; `hud/weapon.rs:134`
  `update_crosshair`; `main.rs:65` `preflight`.
- `assets/third_party/manifest.ron`: no Car Kit; `impact-sounds` has 25 .ogg + License (26 files).

### Engine facts (pinned source, verified by me unless marked "planner probe")
- avian3d 0.7.0: see §1 item 11. Planner probe (`scratch/probe_collision/run.log`, re-read): a 1200 kg box at 10 and
  28 m/s into a 70 kg capsule — `CollisionStart` on the same step as the velocity jump, pre-step velocity 10.00/28.00,
  post-step closing ≈ 0.12; car and pedestrian then move together with the contact persisting ("touching true" for the
  following ticks), one `CollisionStart` per hit. 28 m/s into the 0.5 m wall: stops at the face with default margin and
  with `SpeculativeMargin::ZERO`.
- bevy_ecs 0.19.1: query iteration order = table/archetype creation order (§0).
- bevy_app 0.19.1: `Plugins` tuples up to 15.
- bevy-tnua-avian3d: motor applies only to its own body (`lib.rs:435-449`), no reaction on the ground body.

### Content
Unchanged from PLAN §1 "Content" (URLs, sha256, sedan node layout, 5 `impactMetal_heavy_00{0..4}.ogg` hashes). Zips
cached in `scratch/carkit/` and `scratch/impact/` (hashes re-verified).

### Research
Raycast-car structure (per-wheel spring/damper ray, lateral slip cancel, drive along tyre forward) and the Bullet
`btRaycastVehicle` roll-influence note are standard (PLAN §1 Research, sources kept). Upstream bevy-tnua main still has
the `return` (https://raw.githubusercontent.com/idanarye/bevy-tnua/main/avian3d/src/lib.rs, fetched 2026-09-24) — the fix
is ours to carry; an upstream issue/PR is optional and outside this task.

---

## 3. Revised approach

Same architecture as PLAN §2 (one sim domain `vehicle/`, single archetype, 4 raycast wheels inside one system, no wheel
entities, all numbers in `sedan.ron`/`damage.ron`, impacts from `CollisionStart` + pre-step velocities + manifold
normal, parked cars from citygen, player stays the same entity). Changes:

1. **Vendored Tnua fix first (Step 0)**, gated with an order-asserted fixture; ADR-001 records the second change.
2. **Driver = `Driving { vehicle }` + `RigidBodyDisabled` + `ColliderDisabled` (body and head hitbox child) +
   `TnuaToggle::Disabled`**; exit/eject removes all four. `enter` also resets `ActionIntent` (no latched fire/reload/
   melee from inside the car). Seat sync unchanged.
3. **Bullets vs car (Q2 = (c)+(a))**: pellet filter gains `Vehicle`, so bullets stop on the body (driver unhittable:
   body and head colliders disabled). When the hit collider's `CollisionLayers.memberships` contains
   `GameLayer::Vehicle`, `fire_weapons` writes a new combat message `BulletHitVehicle { shooter, attack, vehicle,
   point, damage: f32 }` with `damage = stats.damage · falloff_factor(distance)` — **no RNG draw** (keeps every
   existing `CombatRng` sequence unchanged). Combat knows only the law layer, not vehicle config. The vehicle domain
   system `apply_bullet_hits` subtracts `damage · damage.ron vehicle.bullet_scale` from `VehicleHealth` (clamp 0).
   Stall at 0 is the same rule as crash damage (throttle forced to 0). Shooting a car is not a crime (GDD §6.4 has no
   row); a shot near people still records `Shooting` via the existing `ShotFired` path.
4. **Impact speed** as PLAN: `(v_a_pre − v_b_pre)·n`, velocities recorded in `FixedPostUpdate` before
   `PhysicsSystems::First`, normal from `Collisions::get(c1, c2)`, processed next tick in `HealthSystems::Damage`,
   **ungated** (the damage of a driverless rolling car during Wasted is fine: `shooter = vehicle` → no crime, no hostility,
   Wasted respawn clears nothing relevant). `DamageDealt` consumers need no special cases (§1 item 8).
5. **Tunnelling:** keep avian defaults, no `SweptCcd` (probe P2 + GDD §5.1 rule); gate flipped by breaking the chassis
   collision filter.
6. `VehiclePlugin` goes into its own `app.add_plugins(VehiclePlugin)` call after the tuple (arity limit 15).

Worked physics numbers and directional examples: unchanged from PLAN §2 (re-derived, §1 item 12). Added numbers:
- Bullet damage to a car: pistol 25 · 1.0 (`bullet_scale`) = 25 per hit → 1000 hp = 40 hits; SMG 12 per hit at
  1/0.08 = 12.5 shots/s → 150 hp/s if all hit (~6.7 s to stall). Shotgun 8 · 10 pellets at ≤ 10 m = 80 per blast.
- Tnua gate: run speed 4.5 m/s, `time_to_run_speed` 0.15 s → 64 ticks (1.0 s) of run ≈ 4.5·(1 − 0.075) = 4.16 m.
  Without the motor the body gets no horizontal force → ≈ 0 m.

---

## 4. Revised steps (complete)

Order = dependency order; each step names its check. `-j 4` for every cargo command during the run.

### Step 0 — vendored Tnua motor fix + gate (binding orchestrator addition)
- `vendor/bevy-tnua-avian3d-0.12.1/src/lib.rs:429-431`: `return;` → `continue;` (both `Disabled` and `SenseOnly` skip
  motors, per their meaning). Nothing else in the file changes (sensors `:172` `return` is inside a per-entity closure
  and is correct).
- `docs/decisions/ADR-001-vendored-tnua-avian3d.md`: second change listed (why: a corpse silently skips every motor
  iterated after it; upstream main still has it, checked 2026-09-24); the upgrade procedure now says "apply both
  changes, diff against the published crate, run the Tnua motor gate". If `vendor/` has its own notes (it has
  `CHANGELOG.md` from upstream only) do not edit upstream files.
- New gate `crates/gta_sim/tests/tnua_motor.rs` [correctness] in `composed_app(TestArea)`:
  1. settle; spawn a civilian (common `spawn_civilian`, idle) at (−25, 0, 10); kill it via `set_health_of(.., 0)` and
     tick until it has `Corpse` + `TnuaToggle::Disabled` (production death path, `civilian/mod.rs:254`); `GATE BROKEN`
     if not within 8 ticks.
  2. insert a test-only `#[derive(Component)] struct LateTable;` on the **player** → the player moves to a table created
     after the corpse's table.
  3. precondition: iterate `world.query::<(Entity, &TnuaMotor)>()`; the corpse index < the player index, else
     `GATE BROKEN: corpse not iterated before the walker`.
  4. `set_intent` run forward for 64 fixed ticks; flat displacement ≥ 3.5 m (derived 4.16 m, §3).
  5. Control row (same file, no corpse): same run ≥ 3.5 m (proves the bound is reachable without the fix's help).
  - Flip-RED: restore `return` → row 4 gives ≈ 0 m → RED; restore `continue` → GREEN. Record in the stage summary.
- Run the whole `cargo test -p gta_sim` right after this step: the fix can change behaviour of gates that ran with
  corpses present (civilians/gangs/police near bodies now get motors). Any changed number is re-derived, never relaxed.
- Check: `cargo test -p gta_sim --test tnua_motor`, full `-p gta_sim` green.

### Step 1 — `crates/citygen`: parking spots (+ golden impact)
As PLAN Step 1, unchanged: `CityParams.parking: ParkingParams { spacing, chance, end_margin, curb_offset }`
(`deny_unknown_fields`, validate rows: spacing ≥ 5, chance ∈ [0,1], end_margin ≥ 0, 0 < curb_offset ≤ lane_width,
strictly failing sabotage values); `assets/world/city.ron parking: (spacing: 40.0, chance: 0.25, end_margin: 15.0,
curb_offset: 1.625)`; `layout.rs ParkingSpot { position: Vec2, heading: Vec2 }`, `CityLayout.parking`;
`rng.rs PARKING = 16`; new `parking.rs` (avenue edges in index order, both directions exactly as `graphs::lanes`,
one `stream(seed, PARKING, e)` per edge, stations from `end_margin` by `spacing`, `chance`, position =
`p + d·s + right·(half_carriageway(Avenue) − curb_offset)`), called last in `generate`; `hash.rs` appends
`len + positions + headings` after `player_spawn`, `HASH_SCHEMA_VERSION` 2 → 3.
- Golden procedure (a) GREEN before the hash lines, (b) RED on 3 seeds after, (c) bless via the command in
  `golden_hashes.txt`, (d) `tests/minimap.rs` untouched and green. `gta_sim tests/city.rs` and `t2.py` read the same
  file.
- `tests/properties.rs` parking property as PLAN (per-seed count range reported; target 60..200 on seeds 1/2/42, tune
  `chance` in data if outside).
- Check: `cargo test -p citygen`.

### Step 2 — layers + bullet filter + `BulletHitVehicle`
- `layers.rs`: `Vehicle` after `Hitbox` (law).
- `combat/hitscan.rs`: filter `[World, Character, Hitbox, Vehicle]`. New message (combat module, `Reflect`,
  `add_message` in `CombatPlugin`, cleared in `clear_combat_messages`):
  `BulletHitVehicle { shooter: Entity, attack: u32, vehicle: Entity, point: Vec3, damage: f32 }`.
  In the pellet loop, when the target has no `Health`: read the hit collider's `CollisionLayers` (new read-only query
  param `Query<&CollisionLayers>`, 14th → 15th param) and if `memberships` has `GameLayer::Vehicle` write the message
  with `stats.damage * falloff_factor(stats, hit.distance)`. `BulletTrace.hit` stays `TraceHit::World` (no new enum
  variant: presentation keeps its world-hit sound).
- Check: existing `shooting.rs`, `gang_fire_lines.rs`, `police_fire_lines.rs` green (no car in them, no RNG change).

### Step 3 — configs `assets/vehicle/{sedan,damage}.ron` + `vehicle/config.rs`
As PLAN Step 3 with one addition: `damage.ron vehicle.bullet_scale: 1.0` (validate > 0; comment: "multiplier on a
weapon's base damage per pellet that hits the body"). Full `damage.ron`:
`vehicle: (max_health: 1000.0, threshold_speed: 5.0, per_mps: 40.0, bullet_scale: 1.0)`,
`pedestrian: (threshold_speed: 3.0, per_mps: 12.0, knockdown_speed: 4.0, shove_scale: 0.6)`.
`sedan.ron` exactly as PLAN (mass 1200, chassis (1.2, 0.92, 2.04), CoM (0, −0.5, 0), wheels (0.72, 1.06, −0.49,
0.48), suspension (0.3, 1.5, 0.4), max 28, accel 4.6, band 2.0, reverse 6.0, brake 9.0, coast 1.0, hold 0.5, steer
(32, 6, 180), grip (1.1, 1.0, 1.0, 0.25), roll 0.3, seat (−0.45, 0.2, 0.1), door (−1.7, 0, −0.3), enter_radius 2.5,
exit_max_speed 3.0). Validate rules, derived helpers (`spring_rate`, `damper_rate`, `rest_height`, `chassis_density`),
the compose-time `curb_offset` checks (city source only) and the new test file `tests/config_vehicle.rs`
(`tests/config.rs` is at 748 lines) as PLAN, plus one sabotage row for `bullet_scale: 0.0` (keyword `bullet_scale`).
- `lib.rs`: load + validate both; insert resources; `app.add_plugins(VehiclePlugin)` as a separate call after the
  existing tuple (§1 item 7).
- Check: `cargo test -p gta_sim --test config_vehicle`.

### Step 4 — `vehicle/mod.rs`: components, messages, bundle, plugin
As PLAN Step 4 (components `Vehicle { driver, steer, on_sidewalk, taken, wheels }` requiring `VehicleHealth,
PreStepVelocity, CityScoped`; `WheelState`; `WHEELS` law const; `Driving { vehicle }`; `DriveIntent` and
`PreStepVelocity` registered as required by `Character` in `VehiclePlugin::build`; `VehicleLoad`; `ActionIntent.
vehicle_requested` in `character/intent.rs:38-45`; messages `VehicleEntered`, `VehicleHit`, `VehicleImpact`;
`vehicle_bundle`; `spawn_parked_cars` on `OnTransition{Loading→Playing}` `.run_if(resource_exists::<City>)`).
Changes:
- `VehicleSystems {Enter, Bullets, Impact, Drive, Record, Seat}`; FixedUpdate: `Impact` in `HealthSystems::Damage`
  (ungated); `Bullets` = `apply_bullet_hits` `.after(HealthSystems::Damage)` (ungated: it only reads combat's message;
  fire itself is Playing-gated); `Enter` in `PlayingSystems`; `Drive` ungated and `.after(Enter).after(Bullets)
  .after(Impact)` so a stall or enter applies on the same tick. FixedPostUpdate: `Record` `.before(PhysicsSystems::First)`,
  `Seat` `.after(PhysicsSystems::Last)`. `OnEnter(Wasted|Busted)` → `eject_all`. `NEW_CITY` → clear `VehicleEntered`,
  `VehicleHit`, `VehicleImpact`.
- Check: `cargo check -p gta_sim -j 4`.

### Step 5 — `vehicle/chassis.rs`: suspension, tyres, drive
Unchanged from PLAN Step 5 (sleeping cars skipped, stalled `health ≤ 0` → throttle 0, speed-sensitive steer, per-wheel
ray `[World, Vehicle]` excluding the car, spring/damper along body up at the mount, tyre frame on the hit plane,
lateral cancel, drive/brake/reverse/hold/coast, per-wheel caps, friction circle μ·N, roll-influence lever along BODY up,
`non_waking()` for undriven cars, `on_sidewalk` via `CityBlock`, `VehicleLoad` counters, pure fns with unit rows).
- Check: unit tests + gates G1, G2, G5, G7.

### Step 6 — `vehicle/seat.rs`: enter, exit, eject, seat sync
As PLAN Step 6 with these changes:
- `enter(...)`: player gets `Driving`, `RigidBodyDisabled`, `ColliderDisabled`, **`TnuaToggle::Disabled`**; head
  hitbox child gets `ColliderDisabled`; `LinearVelocity` zero; **`*action = ActionIntent::default()`** (after the
  `vehicle_requested` take). Car: `driver = Some`, `SleepingDisabled`; `first = !taken`, `taken = true`;
  `VehicleEntered { attack: serial.next_id(), .. }`.
- `exit(...)` / `eject_all` / seat-sync fallback remove all four player-side components (`try_remove`, the car or player
  may be despawning). Clearance and candidates as PLAN.
- `sync_seats` as PLAN (both link directions converge; missing car → eject in place).
- `police/arrest.rs:32` query adds `Without<Driving>`; `combat/pickups.rs:57,132,197` player queries add
  `Without<Driving>`.
- Check: G3, G8.

### Step 7 — `vehicle/impact.rs`: crash impacts → damage
As PLAN Step 7 (record pre-step velocities for all holders; `apply_impacts` from `CollisionStart` + `Collisions::get`
normal oriented vehicle→other; striker rule `v_veh_pre·n ≥ threshold`; pedestrian damage `round((closing −
threshold)·per_mps)` → `Health::take`, `DamageDealt { shooter: driver.unwrap_or(vehicle), .. }`, `VehicleHit`,
knockdown + `knock_back(n_flat·closing·shove_scale)` at ≥ `knockdown_speed`; vehicle/world pairs reduce
`VehicleHealth` by `(closing − threshold)·per_mps`, clamp 0, write `VehicleImpact`; pure fns with the PLAN rows:
(10,0)→84, (6,0)→36, (2.5,0)→0, (0,−6.8)→0, (10,−2)→108; vehicle 4→0, 10→200, 28→920).
- Note in code (one line): `DamageDealt` from a car goes through every existing reader unchanged (§1 item 8).
- Check: G4, G6.

### Step 7b — `vehicle/impact.rs` (or `bullets.rs` if impact.rs nears 400 lines): bullets → car health (Q2)
- `apply_bullet_hits`: `MessageReader<BulletHitVehicle>`, `Query<&mut VehicleHealth>`, `Res<DamageConfig>`:
  `current = (current − damage·bullet_scale).max(0)`. Unknown/despawned vehicle → skip.
- Unit row: `bullet_damage(25.0, 1.0) = 25.0`.
- Check: G12.

### Step 8 — perception, civilians, wanted
As PLAN Step 8 (`ThreatKind::Car`, `perception.ron car_distance: 12.0, car_speed: 3.0`, non-reportable like `Aimed`,
two reaction rows; `Crime::{RunOver, CarTheft}`, `classify(victim, HitSource {Gun, Melee, Vehicle}, killed)` with one
unit row per combination; `record_crimes` + `MessageReader<VehicleHit>` and `MessageReader<VehicleEntered>` (15
params); `HeatTable { run_over, car_theft }`, `wanted.ron run_over: 30, car_theft: 15`, comment updated).
No crime for shooting a car (GDD §6.4 has no row).
- Check: G11, unit tests.

### Step 9 — headless gates `crates/gta_sim/tests/vehicle.rs` (+ `vehicle_city.rs`)
Helpers and G1-G11 as PLAN Step 9 (all in `composed_app`; `spawn_car` asserts Position == Transform after one tick;
fixture-clear shape check; `drive_in`, `kick`, `set_drive`, `forward_of`). Changes and additions:
- **G3** gains row (f): before entering set `ActionIntent.fire_held = true` with a pistol held; enter; run 32 ticks →
  zero `ShotFired` from the player and car `VehicleHealth` unchanged. Flip-RED: remove the `ActionIntent` reset in
  `enter` → shots into the own car → RED. Row (b) also asserts `TnuaToggle::Disabled` on the player, row (c) asserts it
  is gone.
- **G6** stall row keeps the crash path; the bullet path is G12.
- **G12 bullets vs car (Q2)** [correctness], car parked at (−25, h, 0) yaw 90° (broadside to +Z):
  (a) player on foot at (−25, 0, 8) with a pistol, aim at the car centre, `fire_requested` once → exactly one
  `BulletHitVehicle` with `damage == 25.0`; `VehicleHealth` = 1000 − 25·`bullet_scale` (expected computed from the
  loaded configs); the `BulletTrace.to` lies within 0.05 m of the chassis side face (z = 0 + 1.2 … with yaw 90° the
  half-extent facing +Z is `chassis_half_extents.x` = 1.2 → face z = 1.2) and no `DamageDealt` was written.
  Flip-RED: drop `Vehicle` from the bullet filter → no message, trace passes → RED.
  (b) stall/eject path: set `VehicleHealth` to 20 (< one pistol hit), shoot once → 0 (clamped); `drive_in`; throttle 1
  for 64 ticks → displacement < 0.1 m; exit request → player on foot, `Driving` gone (speed 0 ≤ `exit_max_speed`).
  Flip-RED: ignore `health ≤ 0` in drive → car moves → RED.
  (c) driver protected: player drives the car (stationary); a range dummy (`spawn_dummy`) at (−25, 0, 8) gets a
  `Loadout` with a held pistol (implementer: build it with the same constructor the shooting tests use) and each tick
  the test sets its `AimIntent` at the player's head and raises `fire_requested` for 4 shots (spaced ≥ `fire_interval`);
  `GATE BROKEN` if fewer than 4 `ShotFired` from the dummy; assert player `Health` unchanged, no `DamageDealt` with
  target = player, 4 `BulletHitVehicle`, car health −100. Flip-RED: skip `ColliderDisabled` on the head hitbox in
  `enter` → a pellet reaches the head (headshot) or the ray stops on the car first — **implementer: verify in the
  fixture that the head sphere pokes out of the chassis** (head centre = seat y 1.36 + (1.6 − 1.05) = 1.91 m, r 0.35 →
  top 2.26 > chassis top 2.08) and aim from above (dummy placed on a 3 m test block, `spawn_wall`) so the ray reaches
  the head before the body; if the geometry cannot reach the head, say so and flip with `RigidBodyDisabled` removed
  instead.
- All flips recorded in the stage summary.

### Step 10 — client input `src/input/mod.rs`
As PLAN Step 10 (`InVehicle` context, `Drive`/`Handbrake`/`ExitVehicle`/`DriveLook`, `EnterVehicle` on `OnFoot`, both
F actions with `require_reset`, `write_drive_intent`, `sync_contexts` replaces `deactivate_input`/`activate_input`
with a pure `context_activity(state, driving)` + unit rows, `apply_mouse_look` reads `Look` + `DriveLook`,
`release_held_actions` zeroes `DriveIntent`).
- Check: `cargo test -p gta_like --bin gta_like` + owner run.

### Step 11 — client camera
As PLAN Step 11 (`camera.ron car_distance 6.5, car_pivot_height 1.0, car_pitch_deg −8, car_yaw_half_life 0.2,
car_look_return 1.5`; `OrbitCamera.look_idle`; car pivot; shortest-arc `approach_angle` unit rows 170→−170 = +20,
10→−10 = −20; observer `On<Add, Vehicle>` → `TransformInterpolation`). Owner-judged.

### Step 12 — client visuals `src/visuals/vehicle.rs` + `render.ron`
As PLAN Step 12 (model, offset, wheel nodes via `WorldInstanceReady`, steer/spin/hub offset, player hidden while
driving, stall smoke from `juice.ron smoke`). Owner-judged; no gate over look.

### Step 13 — client audio
As PLAN Step 13 (`Synth::Engine`, `SoundClass::Engine` COUNT 9, one `EngineEmitter` on the driven car, `set_speed` /
`set_volume` from `engine_voice` with unit rows, `mix.ron engine`, metal impact pool on `VehicleImpact`, existing audio
gates updated where they enumerate classes).

### Step 14 — HUD, minimap, juice
As PLAN Step 14 (`HudBar::Vehicle`, crosshair hidden while driving, `MarkerKind::Vehicle` for every car except the
driven one (Q3 = a), crash trauma rows in `juice.ron`).

### Step 15 — assets manifest + fetch
- `manifest.ron`: pack `car-kit` (3 files) and 5 metal files in `impact-sounds` (hashes from PLAN §1 Content).
- `tests/asset_manifest.rs`: names set + `"car-kit"`; **exclude `car-kit` from the 4-file loop** (`:101-106`) and
  assert `("car-kit", 3)` + CC0 + no rig separately; `("impact-sounds", 31)`; update the comment ("30 impact .ogg +
  License").
- Offline install: `python tools/fetch_assets.py --cache maw/tasks/in_progress/TASK-015/scratch/carkit --cache
  maw/tasks/in_progress/TASK-015/scratch/impact`, then `--check`. Confirm the GLB's texture reference resolves to the
  manifest `path` (`Textures/colormap.png`).

### Step 16 — runtime QA `tools/qa/scenarios/t14.py` (AC)
As PLAN Step 16 (teleport to the nearest parked car door, F, poll `Driving`, W 3000 ms with screenshots ≥ 0.15 s apart,
speed ≥ 5 m/s and displacement ≥ 5 m, engine sound spawned, `Vehicle` minimap markers; wall run at (0, 1.16, 670)
facing +Z, W 4000 ms (derived: 27.96 m at 4.6 m/s² → 3.49 s, ≈ 16 m/s → car loss ≈ 440 hp), `VehicleHealth` dropped,
car z < 700 − 2.04 + 0.3, speed < 2; F → on foot ≤ 2 s, flat distance ≤ 3 m, `Playing`, player `Health` readable;
screenshots; shutdown; hard pass/fail on components only). Owner checklist as PLAN, plus "полиция стреляет по машине →
полоса HUD падает, на нуле машина глохнет".

### Step 17 — close-out
`cargo build`, `cargo clippy -- -D warnings`, `cargo test -p gta_sim`, `cargo test -p citygen`,
`cargo test -p gta_like --bin gta_like` (touched presentation gates run 3 times), `python tools/qa/tree_check.py`,
`cargo tree -p gta_sim -e normal -i bevy_render` empty. Files < 750 lines.

---

## 5. Risk areas

- **R0 Tnua fix shifts existing gates.** Characters iterated after a corpse now get motors; civilian/gang/police gates
  that ran with corpses present may move. Treat any change as re-derivation with a written reason; never relax.
- **R1 Handling instability** (stiff lateral cancel): lower `grip` in data first; G5/G7 catch divergence.
- **R2 Mass/inertia** (`ColliderDensity` + `CenterOfMass`): G8e fails loudly; fix setup, not the gate.
- **R3 Parked cars never sleep** → G10 RED; remedy is the hold force, not the threshold.
- **R4 Seat sync vs avian writeback** on a `RigidBodyDisabled` body: if G3b fails, also write in FixedUpdate before
  physics. `TransformInterpolation` on the seated player is presentation (owner run).
- **R5 Impact timing** (`CollisionStart` of step N read at FixedUpdate N+1, `PreStepVelocity` of N intact): settled by
  the probe; exact-tick gate G4.
- **R6 Parked cars occupy the avenue outer lane** (Q1 = a) — T15 traffic drives the inner lane.
- **R7 Bullet → car path (Q2)**: cops now stall a driving player's car (intended). Head sphere may poke above the
  chassis roof (1.91 + 0.35 = 2.26 m vs 2.08 m): the driver is protected only because the head collider is disabled —
  G12c pins it. Police at 1 star still cannot arrest a driver and will stand by the car (GDD §6.4, accepted for T14).
- **R8 Golden rebless** mitigated by Step 1(a).
- **R9 Gate vacuity by iteration order** (§0): any future flip that depends on "A before B" asserts the order.
- **R10 Input**: F in two contexts with `require_reset`; `Single<&Action<Look>>` stays unique (`DriveLook`).
- **R11 Headless GLB**: car visuals ungated (presentation).
- **R12 Frame budget**: one driven car = 4 rays; sleeping parked cars cast none (G10); perception adds one small loop.
- **R13 Plugin tuple arity**: `VehiclePlugin` in its own `add_plugins` call; T15 must not re-grow the tuple past 15.

## 6. Open questions

None new. Q1-Q5 are resolved in TASK_FINAL (Q1 a, Q2 c+a, Q3 a, Q4 a, Q5 a). Note for the orchestrator: TASK_FINAL
names branch `feature/t14-drivable-car`, the checkout is on `feature/t14-car`.

children: 0 launched / 0 reported.
