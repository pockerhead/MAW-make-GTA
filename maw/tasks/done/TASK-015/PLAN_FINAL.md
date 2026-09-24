# PLAN_FINAL — TASK-015 (GDD T14): drivable car

Reviewer: plan-reviewer-2. Sources: `PLAN.md` (planner, detail), `PLAN_V2.md` (plan-reviewer-1, corrections),
`TASK_FINAL.md` (binding: Q1 a, Q2 c+a, Q3 a, Q4 a, Q5 a; vendored Tnua motor fix with a gate). V2 wins over PLAN on
every conflict; PLAN detail is restored wherever V2 compressed it without correcting it; my own corrections are marked
**[PR2]** and listed in §5.

Branch: the checkout is `feature/t14-car` (TASK_FINAL says `feature/t14-drivable-car`; work on the existing branch).
Every cargo command in this task uses `-j 4`.

Cost of error, one line: physics (tunnelling, suspension equilibrium, rollover, impact speed), the enter/exit link,
Wasted/Busted/new-city while driving, bullet→car damage, the Tnua motor fix and the parked-car perf budget break
SILENTLY → full gates; handling, camera, engine sound, smoke and visuals are seen on the first frame → owner checklist.

---

## 0. Disconfirmation (plan-reviewer-2)

**Counter-example written first:** G12(c) ("driver protected") in PLAN_V2 is vacuous: with the fixture V2 gives (dummy
on a 3 m block at (−25, 0, 8), aiming at the driver's head), the bullet hits the car roof before it can reach the head
even when the head collider is left enabled, so the flip "skip `ColliderDisabled` on the head hitbox" stays GREEN.

**Searched / computed** (`scratch/pr2/g12c_geometry.txt`, numbers from `assets/character/locomotion.ron`
`float_height 1.05, head_height 1.6, head_radius 0.35`, `character/mod.rs:137-147` head child at
`head_height − float_height = 0.55` above the body centre, `sedan.ron` seat y 0.2, rest height 1.1596, half height 0.92):
- head centre y = 1.1596 + 0.2 + 0.55 = 1.9096, top 2.2596; roof 2.0796 → the head sphere pokes 0.18 m above the roof.
- a ray aimed at the head centre enters the sphere above the roof iff `1.9096 + 0.35·sin α > 2.0796` → elevation
  α > **29.06°**.
- V2 fixture: muzzle ≈ (−24.65, 4.40, 7.55), head (−24.9, 1.91, 0.45) → α = **19.3°** → the ray crosses the roof first
  → flip stays GREEN. **Held: yes, V2's fixture is vacuous.**
- avian3d 0.7.0 `update_child_collider_position` (`collision/collider/collider_transform/plugin.rs:67-96`) has no
  `RigidBodyDisabled` filter: the head sensor follows the seat-synced body Position, so with its `ColliderDisabled`
  removed it really sits above the roof at the seat.

**Settled (the open G12(c) question):** yes, a bullet CAN reach the driver's head over the roof when the head collider is
enabled, for any shot steeper than ~29° (a cop on a roof, a shooter on a ledge). Driver protection therefore rests on
the head `ColliderDisabled`, and G12(c) must shoot from above that angle. Replacement fixture with derived numbers in
Step 9 G12(c) (elevation 55°, entry y 2.16..2.20 including 1° spread).

Also found while checking (details §5): G3(f) as written (pistol + `fire_held`) is vacuous because the pistol is
`SemiAutomatic` and ignores `fire_held`; the Enter/fire order on the enter tick was undefined; the `HEAT` const that V2
called a no-op exists at `wanted/crimes.rs:309` and will not compile without the new fields.

---

## 1. Summary

Add GDD slice T14: a new sim domain `crates/gta_sim/src/vehicle/` (config, components/plugin, chassis, seat, impact)
with a single `Vehicle` archetype: one dynamic avian box body on the new law layer `GameLayer::Vehicle`, 4 raycast
wheels computed inside one system (no wheel entities), spring/damper suspension, tyre lateral cancel, drive / brake /
reverse / hold / coast / handbrake, speed-sensitive steering, roll influence; all tuning in `assets/vehicle/sedan.ron`
and `assets/vehicle/damage.ron`. The player stays the same entity: entering adds `Driving { vehicle }`,
`RigidBodyDisabled`, `ColliderDisabled` (body and head hitbox child) and `TnuaToggle::Disabled`, resets
`ActionIntent`, and the player is pinned to the seat after every physics step; exit/eject is the exact inverse. Crash
impacts come from avian `CollisionStart` + velocities recorded before the physics step + the manifold normal, and feed
the existing `DamageDealt` → crime → death pipeline; car self-damage stalls the car at 0 hp. Bullets now stop on car
bodies and damage `VehicleHealth` through a new combat message `BulletHitVehicle` (no RNG draw). citygen emits parked
car spots on avenue curb lanes (own RNG stream, golden hashes reblessed with a proven procedure). Perception gets
`ThreatKind::Car`, wanted gets `RunOver` 30 / `CarTheft` 15. The client gets the `InVehicle` input context, car camera,
car visuals (Kenney Car Kit sedan), procedural engine hum, metal impact sounds, HUD car bar, minimap car markers and
crash shake. The vendored `bevy-tnua-avian3d` motor loop bug (`return` → `continue`) is fixed first with an
order-asserted gate. Runtime QA `tools/qa/scenarios/t14.py`; handling/camera/sound/visuals go to the owner checklist.

---

## 2. Facts the steps rely on (verified in code / pinned source)

### Simulation (`crates/gta_sim`)
- `lib.rs:41-174` `compose_sim`: every RON via `load_config` + `validate`; one `add_plugins` tuple of **14** plugins
  (Flow, `PhysicsPlugins::default()`, `TnuaAvian3dPlugin::new(FixedUpdate)`, Character, World{source}, Player,
  Combat{seed}, Navigation, Perception, Population{seed}, Civilian, Gang{seed}, Police{seed}, Wanted). bevy_app 0.19.1
  implements `Plugins` for tuples up to 15 (`plugin.rs:186-193`, verified).
- `layers.rs:5-9` `GameLayer { World (default), Character, Hitbox }`; GDD §12 law also lists `Vehicle`.
- `character/mod.rs:32-44` `Character` requires `MoveIntent, AimIntent, ActionIntent, JumpBuffer, AnimState,
  HitReaction, Melee, CityScoped`; `head_hitbox` (`:137-147`): `Sensor`, `CollisionLayers::new(Hitbox, NONE)`, child at
  `(0, head_height − float_height, 0)`; `character_components` (`:152-176`): dynamic capsule r 0.3 h 1.5,
  `CollisionLayers::new(Character, ALL)`, `TnuaAvian3dSensorShape` cylinder; `drive_characters` (`:179`);
  `HealthSystems {Damage, Regen, Pickup, Death}` chained in FixedUpdate (`:100-109`); `despawn_tnua_sensors` observer.
- `character/intent.rs:38-45` `ActionIntent { fire_held, fire_requested, reload_requested, select, cycle }`.
- `player/mod.rs:27-35` player spawns once on `OnTransition{Loading→Playing}`; `Character` requires `CityScoped`, so
  `despawn_city` removes the player on `NEW_CITY`.
- `flow/mod.rs`: `PlayingSystems` (Playing), `NpcSystems` (Playing|Wasted|Busted), `NEW_CITY` (Paused→Loading);
  `flow/wasted.rs:119` `respawn_at` teleports the SAME player entity; `drop_queued_input` (`wasted.rs:163-167`) resets
  `ActionIntent` wholesale on OnExit Wasted/Busted (a new field is cleared automatically).
- `world/test_area.rs:9-23`: floor 80×1×80 (top y 0, x/z ∈ ±40), boxes at x 10|13|16 z 10, ramp (−10, 2.327, −16.43)
  30°, box 4×5×4 at (−10, 2.5, −22.66), box 3×1.6×6 at (10, 0.8, −17.4), wall 12×4×0.5 at (0, 2, 14) (near face
  z 13.75), stairs at x 10, z −12.15..−14.55. Player spawn = origin.
- `world/city.rs:52` `pub struct CityBlock` (static block prisms up to the curb, 0.15 m).
- `combat/mod.rs:33-38` `AttackSerial::next_id`; `combat/mod.rs:93-117`: `(tick_loadouts, melee chain, fire_weapons)
  .chain().in_set(HealthSystems::Damage)`, all in `PlayingSystems`; `clear_combat_messages` on `NEW_CITY`.
  Re-exported: `aim_yaw`, `muzzle`, `Loadout`, `GunSlot`, `Weapon`, `acquire`, `falloff_factor`, `dummy_bundle`.
- `combat/hitscan.rs:141-297` `fire_weapons` — 13 system params today; filter `[World, Character, Hitbox]`
  (`:167-168`); two-ray aim (aim ray from `AimIntent.origin`, pellet ray from `muzzle(position, dir, offset)`);
  pellet cone half-angle = `loadout.spread_deg` of the previous tick; on a hit with `Health` → `roll_damage` (one
  `CombatRng` draw) → `DamageDealt`; else `TraceHit::World`, **no** damage draw.
- `combat/weapons.rs:296-299` `spread_deg = base + bloom + 0.3·speed`; pistol is `SemiAutomatic` (fires only on
  `fire_requested`), SMG `Automatic` (fires on `fire_held`); pistol base 1.0°, bloom +1° per shot, recovery after
  0.35 s at 6°/s; `falloff: None` for pistol/SMG.
- `combat/melee.rs:270` `HitReaction::escalate(knockdown, cfg)`, `:549` `pub fn knock_back(controller, shove)`; melee
  targets are found by a shape cast `[World, Character]` (`:454-468`) → a driver with disabled colliders is not hit.
- `population/mod.rs:233-241` `corpse_components() = (Dead, Corpse, TnuaToggle::Disabled, RigidBody::Static,
  CollisionLayers::NONE)`; civilians get it in `civilian/mod.rs:254` when `health ≤ 0`.
- `perception/mod.rs:54-60` `ThreatKind {Gunshot, Fight, Corpse, Aimed, Hurt}`; GDD §6.2 "машина на тротуаре" not
  implemented.
- `wanted/crimes.rs`: `Crime` (no car rows), `classify(victim, melee: bool, killed)` `:149` (callers `:206` and the unit
  test `:342`), `record_crimes` `:162` (13 params), unit-test const `HEAT: HeatTable` at `:309` **[PR2]**;
  `wanted/mod.rs:25-34` `HeatTable`; `assets/wanted/wanted.ron:2-3` "Car rows come with T14/T15".
- `police/arrest.rs:32` player query `(With<Player>, Without<Dead>)`; GDD §6.4: no arrest in a car.
- `combat/pickups.rs:57,132,197` player queries `(With<Player>, Without<Dead>)`.
- `tests/asset_manifest.rs:78-124`: names set; every pack except mini-characters/inter/audio must have exactly 4 files
  (`:101-106`); impact-sounds 26.
- Vendored `vendor/bevy-tnua-avian3d-0.12.1/src/lib.rs:419-451` `apply_motors_system`: `return` on
  `Disabled | SenseOnly` at `:429-431` (the bug); sensors `:171-175` `return` inside a per-entity closure (correct);
  trackers `:117-121` `continue` (correct). ADR-001 (`docs/decisions/ADR-001-vendored-tnua-avian3d.md`) documents one
  change only.

### Client (`src/`)
- `input/mod.rs`: context `OnFoot` (`:15`, actions `:102-…`, `Look` = mouse motion `:104`); `deactivate_input` /
  `activate_input` (`:142-151`) toggle `OnFoot` on Paused; `release_held_actions` (`:235`).
- `camera/mod.rs` (194 lines): `apply_mouse_look` `:78` (`Single<&Action<Look>>`), `follow_player` `:101`,
  `enable_player_interpolation` `:192`.
- `minimap/markers.rs:20` `MarkerKind`, `:96` `sync_markers`; `audio/cues.rs:21` `SoundClass`, `COUNT = 8` `:34`,
  `play_impacts` `:291`; `juice/shake.rs:22` `add_trauma`; `hud/mod.rs:54` `HudBar`; `hud/weapon.rs:134`
  `update_crosshair`; `main.rs:65` `preflight`; `visuals/character.rs` scene-root child + `WorldInstanceReady` pattern.
- `assets/third_party/manifest.ron`: no Car Kit; `impact-sounds` 25 .ogg + License.

### Engine (pinned source)
- avian3d 0.7.0: physics in `FixedPostUpdate`; `PhysicsSystems::{First..Last}`; `CollisionStart { collider1,
  collider2, body1: Option, body2: Option }` is both `Message` and `EntityEvent` (`collision_events.rs:169-187`), written
  only for colliders with `CollisionEventsEnabled`; `Collisions::get(e1, e2) -> Option<&ContactPair>`
  (`contact_types/system_param.rs:71`); manifold normal world-space, collider1→collider2; `Forces` QueryData
  (`forces/query_data.rs:107-121`), `non_waking()` (`:153`), `velocity_at_point` (`:269`), `apply_force_at_point`
  (`:330`); `RigidBodyDisabled` keeps spatial queries (`rigid_body/mod.rs:330-380`); `ColliderDisabled` removes from
  collisions and spatial queries, own entity only (`collision/collider/mod.rs:380-394`); child collider positions follow
  the body even when it has `RigidBodyDisabled` (`collider_transform/plugin.rs:67-96`) **[PR2]**; user writes of
  `LinearVelocity` wake a sleeping island (`islands/sleeping.rs:566-608`); `SleepThreshold` 0.15, `TimeToSleep` 0.5 s,
  `SleepingDisabled`; `SpatialQuery::cast_ray`, `shape_intersections`; `Gravity::default() = (0, −9.81, 0)`.
- Planner probe (`scratch/probe_collision/run.log`): P1 a 1200 kg box at 10/28 m/s into a 70 kg capsule —
  `CollisionStart` on the same step as the velocity jump, pre-step velocity 10.00/28.00, post-step closing ≈ 0.12, one
  `CollisionStart` per hit, contact persists. P2 4.1 m box at 28 m/s (0.4375 m/tick) into the 0.5 m wall: stops at the
  face with default margin and with `SpeculativeMargin::ZERO` (no tunnelling either way); 45° into a cube corner: stops,
  normal (−0.707, 0, −0.707).
- bevy_ecs 0.19.1: query iteration order = archetype/table creation order (`query/state.rs:93-95,649-658`), not spawn
  order.
- bevy_enhanced_input 0.26.0: several contexts on one entity (`context.rs:834`); `ContextActivity<C>::{ACTIVE,
  INACTIVE}`, `Deref<bool>` (`context.rs:724-744`); inactive context zeroes its actions; `ActionSettings.require_reset`
  (`action.rs:186`).
- bevy_audio 0.19.1 `AudioSinkPlayback::set_speed` on `AudioSink`/`SpatialAudioSink` (`sinks.rs:38,184,288`).
- `App::register_required_components::<Character, X>()` must run before any `Character` entity exists → call it in
  `VehiclePlugin::build`.
- No new crate; `Cargo.lock` does not change.

### Content (zips cached for the offline fetch)
- Kenney Car Kit 3.1, CC0, `https://kenney.nl/media/pages/assets/car-kit/1a312ec241-1775131960/kenney_car-kit.zip`,
  sha256 `fac7dacac5c7874348cf19729af3ef205f3d366493edaf0a827d93f4fdf3d0c4`, cached at
  `maw/tasks/in_progress/TASK-015/scratch/carkit/kenney_car-kit.zip`. Files:
  `Models/GLB format/sedan.glb` `b532ea7d2c59f7f6b22b138cf1955218a2c1898f1cea932af4d3fd563c3959b7`;
  `Models/GLB format/Textures/colormap.png` `f3622a03a20c6696065cae9cbe391351be873508af190c2ebd1d420c055787a5`;
  `License.txt` `c33b7f6453d134deae7b1b8493717d9ccfa754c25ab97f6de89b88f8fda19b00`.
- `sedan.glb` (`scratch/carkit/sedan_nodes.txt`): nodes `body`, `wheel-front-left/right`, `wheel-back-left/right`, no
  animations; model front = **+Z**; body x ±0.75, y 0..1.15 (node y 0.15), z ±1.275 (node z −0.025); wheel hubs
  (±0.3, 0.3, ±0.66), radius 0.3, wheel mesh to |x| 0.6.
- Impact sounds (archive `029d734a…`, cached `scratch/impact/kenney_impact-sounds.zip`), add
  `Audio/impactMetal_heavy_000.ogg` `e07045693e4a2b3d165c424e3dab4c781d9ff8880a386880ac89a51315d7f831`,
  `_001` `83554049f81f4db9209379e103c30bfa63f65c42189a03f300b045c2c82e23ae`,
  `_002` `b914c8f1eb7c0f34bb165d7c77f4be0351f6be0660c13c53e65424e262e2c093`,
  `_003` `b0f2ba4dabde9a87eb9c188a19d31e0c2300fd321adeba08d3b9b8aa011d7037`,
  `_004` `6d65b463c0555dd5be16b8db6d2cbe23a94e07a4637779b8ad17d0db3e500a87`.

### Research
- Raycast car = one rigid body + per-wheel suspension ray, spring + damper along the wheel up axis, lateral force
  cancelling tyre slip, drive/brake along tyre forward (Toyful Games, "Making Custom Car Physics in Unity",
  https://www.youtube.com/watch?v=CdPYlj5uZeI).
- Rollover: tyre forces act below the CoM (https://en.wikipedia.org/wiki/Vehicle_rollover, SSF t/2h). Bullet
  `btRaycastVehicle` scales the side-impulse lever by `m_rollInfluence`; its known bug scales a WORLD offset — scale along
  the BODY up axis (https://pybullet.org/Bullet/phpBB3/viewtopic.php?f=9&t=5832,
  https://github.com/bulletphysics/bullet3/blob/master/src/BulletDynamics/Vehicle/btRaycastVehicle.cpp).
- CCD: avian recommends speculative contacts by default, `SweptCcd` only when needed; GDD §5.1 "SweptCcd only if the
  gate fails".
- Upstream bevy-tnua main still has the `return` (https://raw.githubusercontent.com/idanarye/bevy-tnua/main/avian3d/src/lib.rs,
  fetched 2026-09-24 by plan-reviewer-1): the fix is ours to carry.

---

## 3. Worked numbers and directional examples

### Physics (all derived from `sedan.ron`; Kenney sedan × render scale 1.6)
- Chassis box 2.4 × 1.84 × 4.08 m (half 1.2, 0.92, 2.04); volume 18.017 m³ → density 1200/18.017 = 66.6 kg/m³.
- Body bottom 0.15·1.6 = 0.24 m above the road → chassis centre at rest **1.16 m**.
- Wheels: radius 0.3·1.6 = 0.48; half track 0.45·1.6 = 0.72; half wheelbase 0.66·1.6 = 1.06; hub at rest −0.68 below
  the centre.
- Suspension f 1.5 Hz, ζ 0.4, travel 0.3: ω = 9.4248, ω² = 88.83; m/4 = 300 kg → k = 26 649 N/m,
  c = 0.4·2·√(26 649·300) = 2 262 N·s/m; x_eq = 9.81/88.83 = 0.1104 m; spring length at rest 0.1896 →
  `mount_height` = −0.68 + 0.1896 = **−0.49**. `rest_height()` = −mount_height + (travel − g/ω²) + radius =
  0.49 + 0.1896 + 0.48 = **1.1596 m**. Full compression: centre 0.97, chassis bottom 0.05 m above road.
  Stability: ω·dt = 0.147 rad/step, 2ζω·dt = 0.118 → explicit spring stable at 64 Hz.
- Drive: 4.6 m/s² → 0→27.8 m/s in 6.0 s; per rear wheel 1200·4.6/2 = 2 760 N < μ·N = 1.1·2 943 = 3 237 N. Brake
  9 m/s² → 2 700 N per wheel.
- Rollover: CoM 0.5 below centre → 0.66 m above road; SSF = 1.44/(2·0.66) = 1.09 ≈ μ → `roll_influence` 0.3
  (lever 0.198 m → SSF 3.6).
- Yaw inertia m/12·(w² + l²) = 100·(5.76 + 16.646) = **2 240.6 kg·m²**.
- Impact (`damage.ron`): pedestrian `(v − 3)·12` hp, knockdown ≥ 4 m/s; vehicle `(v − 5)·40` of 1000 hp.
  10 m/s → 84 (pedestrian), 200 (car); 28 m/s → 920 (car).
- Bullets → car: pistol 25·`bullet_scale` 1.0 = 25 per hit → 40 hits to stall 1000 hp; SMG 12 per hit at 12.5 shots/s
  → 150 hp/s (~6.7 s); shotgun 8 × 10 pellets ≤ 10 m = 80 per blast.
- Tnua gate: run 4.5 m/s, `time_to_run_speed` 0.15 → 64 ticks of run ≈ 4.5·(1 − 0.075) = 4.16 m; no motor → ≈ 0 m.

### Directional examples
Body frame x right, y up, forward −Z (GDD §3.2). `R_y(θ)(x,y,z) = (x cosθ + z sinθ, y, −x sinθ + z cosθ)`.
1. Drive: yaw 0 → forward (0,0,−1); yaw 90° → (−1,0,0); yaw 180° → (0,0,+1).
2. Steer: input +1 = D = right turn = yaw decreasing. Front-wheel forward in body frame = R_y(−δ)(0,0,−1) =
   (sin δ, 0, −cos δ); δ 30° → (0.5, 0, −0.866); right = forward × up = (0.866, 0, 0.5).
3. Door body (−1.7, 0, −0.3), car yaw −45° (forward (0.707,0,−0.707), right (0.707,0,0.707), back (−0.707,0,0.707)):
   world offset = −1.7·right − 0.3·back = (−0.990, 0, −1.414) (same via R_y(−45°)).
4. Camera yaw of a car `aim_yaw(rot·NEG_Z) = atan2(−f.x, −f.z)`: (0,0,−1) → 0; (−1,0,0) → 90°; (0,0,1) → 180°.
   Shortest arc: current 170°, target −170° → +20°.
5. Visual wheel spin: the model is rotated 180° about Y; in the model frame forward is +Z; R_x(θ)(0,1,0) =
   (0, cosθ, sinθ) → θ increases with forward speed, Δθ = v_long·dt/r.
6. **[PR2]** Car at yaw 90° (G12): forward (−1,0,0), right (0,0,−1), left side faces +Z at z = +1.2. Door (−1.7,0,−0.3)
   → offset (−0.3, 0, 1.7); seat (−0.45, 0.2, 0.1) → (0.1, 0.2, 0.45).

---

## 4. Implementation steps

Order = dependency order; each step names its check.

### Step 0 — vendored Tnua motor fix + gate (binding orchestrator addition)
- `vendor/bevy-tnua-avian3d-0.12.1/src/lib.rs:429-431`: `return;` → `continue;` in `apply_motors_system` (both
  `Disabled` and `SenseOnly` skip motors). Nothing else in the file changes.
- `docs/decisions/ADR-001-vendored-tnua-avian3d.md` (Russian, like the file): "Решение" lists the second change
  (`continue` вместо `return` в `apply_motors_system`: труп с `TnuaToggle::Disabled` молча отключал моторы всех
  персонажей, итерируемых после него; upstream main проверен 2026-09-24, баг там есть); "Обновление" becomes: apply both
  changes, diff against the published crate, run `cargo tree -p gta_sim -e normal -i bevy_render` and
  `cargo test -p gta_sim --test tnua_motor`. Do not edit the upstream `CHANGELOG.md`/`README.md` in `vendor/`.
- New gate `crates/gta_sim/tests/tnua_motor.rs` [correctness], `mod common;`, `headless_app()` (= `composed_app(TestArea)`):
  1. `settle(&mut app)`; `test_graph(&mut app, vec![(−25,0,10), (−25,0,20)], &[(0,1)])`;
     `spawn_civilian(&mut app, GraphWalker { from: 0, to: 1 }, 0.0, calm())`; run 1 tick; `set_health_of(.., |h| h.current = 0.0)`;
     tick until the civilian has `Corpse` and `TnuaToggle::Disabled` (production death path); `GATE BROKEN` if not
     within 8 ticks.
  2. test-only `#[derive(Component)] struct LateTable;` inserted on the **player** → the player moves to a table created
     after the corpse's.
  3. **[PR2]** precondition over the SAME query data as the system: `world.query::<(Entity, &TnuaMotor, Forces)>()` +
     `iter_mut`; the corpse must appear and its index must be < the player's, else `GATE BROKEN: corpse not in the motor
     loop before the walker`.
  4. `set_intent(|i| i.axis = Vec2::Y)` (yaw 0 → −Z; path from the origin to z −4 is clear), run 64 fixed ticks; flat
     displacement ≥ 3.5 m (derived 4.16 m).
  5. Control test (same file, separate `#[test]`, no corpse): same run ≥ 3.5 m.
  - Flip-RED: restore `return` → row 4 ≈ 0 m → RED; restore `continue` → GREEN. Record in the stage summary.
- Run the whole `cargo test -p gta_sim -j 4` right after this step: characters iterated after a corpse now get motors;
  any changed expected number is re-derived with a written reason, never relaxed.
- Check: `cargo test -p gta_sim --test tnua_motor -j 4`, full `-p gta_sim` green.

### Step 1 — `crates/citygen`: parking spots (+ golden impact)
- `src/params.rs`: `CityParams.parking: ParkingParams { spacing: f32, chance: f32, end_margin: f32, curb_offset: f32 }`
  (`#[serde(deny_unknown_fields)]`); `validate`: spacing ≥ 5, chance ∈ [0,1], end_margin ≥ 0,
  0 < curb_offset ≤ `roads.lane_width`. `validate_rejects_bad_params` gets one row per rule with values strictly on the
  failing side (spacing 4.0, chance 1.5, end_margin −1.0, curb_offset 0.0 and 3.5), each asserting a keyword that names
  the field.
- `assets/world/city.ron`: `parking: (spacing: 40.0, chance: 0.25, end_margin: 15.0, curb_offset: 1.625)`; comment: spot
  centre `curb_offset` m in from the curb = outer-lane centre on an avenue (3.25/2).
- `src/layout.rs`: `pub struct ParkingSpot { pub position: Vec2, pub heading: Vec2 }` (unit lane direction);
  `CityLayout.parking: Vec<ParkingSpot>`.
- `src/rng.rs`: `pub(crate) const PARKING: u64 = 16;` (15 is `GANGS`, 16 free).
- New `src/parking.rs` (~60 lines): for every `RoadClass::Avenue` edge `e` in index order, both directions
  `(from,to)` in `[(a,b),(b,a)]` exactly as `graphs::lanes` (d = (q−p)/|q−p|, right = `d.perp()`, `graphs.rs:100-102`);
  one `stream(seed, PARKING, e as u64)` per edge, both directions draw from it in order; stations
  `s = end_margin, end_margin + spacing, … ≤ len − end_margin`; each kept with `chance(rng, p)`; position =
  `p + d·s + right·(half_carriageway(Avenue) − curb_offset)` (= 6.5 − 1.625 = 4.875 m from the centre line), heading = d.
  Streets/alleys get none (Q1 = a). Called last in `generate` (after `graphs::build`, `lib.rs:40`) so no stream moves.
- `src/hash.rs`: after `w.vec2(layout.player_spawn)` (`:154`) append `w.len(layout.parking.len())` and each `position`,
  `heading`; `HASH_SCHEMA_VERSION` 2 → 3.
- **Golden procedure** (all three hashes change, geometry unchanged): (a) implement parking WITHOUT the hash lines →
  `cargo test -p citygen --test golden` GREEN (no existing field moved); (b) add hash lines + schema bump → RED on all 3
  seeds; (c) bless with the command in `golden_hashes.txt`, paste 3 lines; (d) `tests/minimap.rs` raster golden stays
  green untouched. Record (a)-(d) with old/new lines in the stage summary. `gta_sim tests/city.rs` (via
  `common::golden`) and `tools/qa/scenarios/t2.py` read the same file — no other edit.
- `tests/properties.rs` new property over `layouts()` (SEEDS + SWEEP): every seed ≥ 1 spot; each spot within 0.01 m of
  `half_carriageway(Avenue) − curb_offset` from the centre line of an avenue edge, ≥ `end_margin − 0.01` from both its
  nodes, `heading` parallel to that edge (|cross| < 1e-4); no spot inside any block `curb` polygon; two spots in the same
  direction of an edge ≥ `spacing − 0.01` apart. Report the per-seed count range (target 60..200 on seeds 1/2/42; if
  outside, tune `chance` in `city.ron`, record the final numbers).
- Check: `cargo test -p citygen -j 4`.

### Step 2 — layers + bullet filter + `BulletHitVehicle`
- `crates/gta_sim/src/layers.rs`: add `Vehicle` after `Hitbox` (law, GDD §12).
- `combat/hitscan.rs`: filter `[World, Character, Hitbox, Vehicle]` (both the aim ray and the pellet ray use it).
- New message in `combat/hitscan.rs` (re-export from `combat/mod.rs`, `add_message` + `register_type` in
  `CombatPlugin`, cleared in `clear_combat_messages`):
  ```rust
  /// A pellet that stopped on a vehicle body; `damage` is before the vehicle's `bullet_scale`.
  #[derive(Message, Reflect, Clone, Copy, Debug)]
  #[reflect(Message)]
  pub struct BulletHitVehicle { pub shooter: Entity, pub attack: u32, pub vehicle: Entity, pub point: Vec3, pub damage: f32 }
  ```
- `fire_weapons` gains `layers: Query<&CollisionLayers>` and `mut vehicle_hits: MessageWriter<BulletHitVehicle>`
  (13 → **15** params **[PR2]**, ≤ 16). In the pellet loop, after the trace is written and before
  `targets.get_mut(target)`: if `target` has no `Health` (`!targets.contains(target)`) and
  `layers.get(hit.entity).is_ok_and(|l| l.memberships.has_all(GameLayer::Vehicle))`, write
  `BulletHitVehicle { shooter, attack: shot, vehicle: target, point, damage: stats.damage * falloff_factor(stats, hit.distance) }`
  and `continue`. No RNG draw (existing `CombatRng` sequences unchanged). `BulletTrace.hit` stays `TraceHit::World` (no
  new enum variant). Combat knows only the law layer, not vehicle config.
- Check: existing `shooting.rs`, `gang_fire_lines.rs`, `police_fire_lines.rs` green.

### Step 3 — configs `assets/vehicle/{sedan,damage}.ron` + `vehicle/config.rs`
- `VEHICLE_CONFIG = "vehicle/sedan.ron"`, `DAMAGE_CONFIG = "vehicle/damage.ron"`; all structs `deny_unknown_fields`.
- `sedan.ron` (comments carry the §3 derivations, units named):
  ```
  mass: 1200.0, chassis_half_extents: (1.2, 0.92, 2.04), center_of_mass: (0.0, -0.5, 0.0),
  wheels: (half_track: 0.72, half_wheelbase: 1.06, mount_height: -0.49, radius: 0.48),
  suspension: (travel: 0.3, frequency_hz: 1.5, damping_ratio: 0.4),
  max_speed: 28.0, acceleration: 4.6, top_speed_band: 2.0, reverse_speed: 6.0,
  brake_deceleration: 9.0, coast_deceleration: 1.0, hold_speed: 0.5,
  steer: (max_deg: 32.0, at_max_speed_deg: 6.0, rate_deg_per_s: 180.0),
  grip: (mu: 1.1, front: 1.0, rear: 1.0, handbrake_rear: 0.25), roll_influence: 0.3,
  seat: (-0.45, 0.2, 0.1), door: (-1.7, 0.0, -0.3), enter_radius: 2.5, exit_max_speed: 3.0,
  ```
- `damage.ron`: `vehicle: (max_health: 1000.0, threshold_speed: 5.0, per_mps: 40.0, bullet_scale: 1.0)`,
  `pedestrian: (threshold_speed: 3.0, per_mps: 12.0, knockdown_speed: 4.0, shove_scale: 0.6)`. `bullet_scale` comment:
  "multiplier on a weapon's base damage per pellet that hits the body".
- `VehicleConfig::validate`: all finite; mass, extents, radius, travel, frequency, damping_ratio, max/reverse speed,
  acceleration, brake, coast, top_speed_band > 0; hold_speed, enter_radius, exit_max_speed > 0;
  `travel > g/(2πf)²` (message names `suspension.travel`); `steer.at_max_speed_deg ≤ max_deg`;
  `roll_influence ∈ [0,1]`; `handbrake_rear ∈ [0,1]`; `|door.x| > chassis_half_extents.x`. Derived pure helpers
  `spring_rate()`, `damper_rate()`, `rest_height()`, `chassis_density()`; `GRAVITY = 9.81` law const shared with tests.
  `DamageConfig::validate`: thresholds ≥ 0, per_mps > 0, max_health > 0, knockdown_speed ≥ pedestrian threshold,
  `bullet_scale > 0`.
- `lib.rs compose_sim`: load + validate both; for `WorldSource::City` also check `curb_offset ≥ chassis_half_extents.x`
  (1.625 ≥ 1.2) and `curb_offset + chassis_half_extents.x ≤ roads.lane_width` (2.825 ≤ 3.25); error path
  `world/city.ron`, message names `parking.curb_offset`. Insert both resources. **`app.add_plugins(VehiclePlugin);` as a
  separate call after the existing 14-plugin tuple** (tuple limit 15; T15 must not grow the tuple).
- New `crates/gta_sim/tests/config_vehicle.rs` (`tests/config.rs` is at 748 lines): shipped files load + validate;
  unknown field names file and field; one sabotage per validate rule via `common::sabotaged` with strictly failing values
  (e.g. `travel: 0.3` → `0.1`, keyword `suspension.travel`; `bullet_scale: 1.0` → `0.0`, keyword `bullet_scale`), each
  fixture's keyword different; compose-time curb check via a sabotaged `city.ron` copy (`curb_offset: 1.0`, keyword
  `parking.curb_offset`).
- Check: `cargo test -p gta_sim --test config_vehicle -j 4`.

### Step 4 — `vehicle/mod.rs`: components, messages, bundle, plugin
- Components (all `Reflect` + registered):
  - `Vehicle { driver: Option<Entity>, steer: f32 (rad, + = right), on_sidewalk: bool, taken: bool, wheels: [WheelState; 4] }`,
    `#[require(VehicleHealth, PreStepVelocity, CityScoped)]`; `WheelState { compression: f32, grounded: bool }`; law
    `const WHEELS: [(f32, f32); 4]` = front-left (−,−), front-right (+,−), back-left (−,+), back-right (+,+) (x sign,
    z sign; front = −Z).
  - `VehicleHealth { current: f32 }` (Default 0; the bundle sets `max_health`).
  - `Driving { vehicle: Entity }` on the player.
  - `DriveIntent { throttle: f32, steer: f32, handbrake: bool }` (`Reflect, Default`) and `PreStepVelocity(pub Vec3)`,
    both via `app.register_required_components::<Character, _>()` in `VehiclePlugin::build`.
  - `VehicleLoad { awake: u32, rays: u32 }` resource.
- `character/intent.rs:38-45`: `ActionIntent` gains `pub vehicle_requested: bool` (doc: "F: enter/exit, cleared by the
  fixed tick") — the only edit in `character/`.
- Messages (`add_message`, `Reflect`): `VehicleEntered { vehicle, driver, attack: u32, first: bool }`,
  `VehicleHit { vehicle, driver: Option<Entity>, target, attack: u32, speed: f32 }`,
  `VehicleImpact { vehicle, point: Vec3, speed: f32 }`.
- `pub fn vehicle_bundle(cfg: &VehicleConfig, dmg: &DamageConfig, transform: Transform) -> impl Bundle`:
  `Vehicle::default()`, `VehicleHealth { current: dmg.vehicle.max_health }`, `Name::new("Vehicle")`, transform,
  `RigidBody::Dynamic`, `Collider::cuboid(2·hx, 2·hy, 2·hz)`, `ColliderDensity(cfg.chassis_density())`,
  `CenterOfMass(cfg.center_of_mass)`, `CollisionLayers::new(Vehicle, [World, Character, Vehicle])`,
  `CollisionEventsEnabled`. No `Mass` (density keeps inertia consistent).
- `spawn_parked_cars` on `OnTransition{Loading→Playing}` `.run_if(resource_exists::<City>)`: one bundle per spot at
  `(x, rest_height, y)`, rotation `Quat::from_rotation_y(aim_yaw(Vec3::new(h.x, 0, h.y)))`.
- `VehicleSystems {Enter, Bullets, Impact, Drive, Record, Seat}`:
  - FixedUpdate: `Enter` in `PlayingSystems` and **`.before(HealthSystems::Damage)` [PR2]** (the `ActionIntent` reset on
    the enter tick must precede `tick_loadouts`/`fire_weapons`); `Impact` in `HealthSystems::Damage` (ungated);
    `Bullets` = `apply_bullet_hits` `.after(HealthSystems::Damage)` (ungated); `Drive` ungated,
    `.after(Enter).after(Bullets).after(Impact)`.
  - FixedPostUpdate: `Record` `.before(PhysicsSystems::First)`, `Seat` `.after(PhysicsSystems::Last)`.
  - `OnEnter(Wasted)`, `OnEnter(Busted)` → `eject_all`; `NEW_CITY` → clear `VehicleEntered`, `VehicleHit`,
    `VehicleImpact`.
- Check: `cargo check -p gta_sim -j 4`.

### Step 5 — `vehicle/chassis.rs`: suspension, tyres, drive (~250 lines)
- `drive_vehicles` (`VehicleSystems::Drive`): `SpatialQuery`, `Res<VehicleConfig>`, `Res<Time<Fixed>>`,
  `ResMut<VehicleLoad>`, `Query<(Entity, &mut Vehicle, &VehicleHealth, Forces, Has<Sleeping>)>`, `Query<&DriveIntent>`,
  `Query<(), With<CityBlock>>`.
- Per car: sleeping → `continue` (no rays). Intent = driver's `DriveIntent` or default; `health ≤ 0` → throttle 0.
- Steering: target δ = steer·lerp(max_deg, at_max_speed_deg, clamp(|v_f|/max_speed)); `vehicle.steer` moves toward it at
  `rate_deg_per_s`. Front wheel forward = R_y(−δ)(0,0,−1) (§3 ex. 2).
- Per wheel: mount = pos + rot·(sx·half_track, mount_height, sz·half_wheelbase); ray `rot·NEG_Y`, max `travel + radius`,
  filter `[World, Vehicle]` excluding the car. Miss → not grounded. Hit: x = (travel + radius − distance).clamp(0, travel);
  spring speed = `velocity_at_point(mount)·up`; F_s = (k·x − c·speed).max(0) along body up at the mount; N = F_s.
- Tyre frame: f_w = wheel forward projected on the hit plane, r_w = f_w × n. At the contact: v = `velocity_at_point`;
  F_lat = −r_w·(v·r_w)·grip·(m/4)/dt (front/rear grip; rear × handbrake_rear while handbrake). Longitudinal: throttle > 0
  on rear wheels F = f_w·throttle·m·acceleration/2·clamp((max_speed − v_f)/top_speed_band, 0, 1); throttle < 0 → brake
  (all wheels, m·brake/4, against v_long) while v_f > hold_speed, else reverse drive up to `reverse_speed`; no throttle
  and |v_f| < hold_speed (or handbrake on rear) → hold F = −f_w·(v·f_w)·(m/4)/dt; no throttle above hold_speed → coast
  m·coast/4. Brake/coast/hold forces capped at (m/4)·|v_long|/dt. Clamp |F_lat + F_long| ≤ μ·N.
  Application point = contact + up·(roll_influence − 1)·(contact→CoM height along BODY up).
- `forces.non_waking()` for undriven cars, waking `ForcesItem` for a driven one; record wheel states; `on_sidewalk` = any
  grounded wheel whose hit entity has `CityBlock`; `VehicleLoad` counts awake cars and rays.
- Pure fns + in-file unit tests: `wheel_forward(steer)` (3 rows §3), `steer_limit(v)`, `drive_force(...)` rows
  (below/at/above max_speed, reverse, brake), `spring_force(x, speed)` (k, c of §3), `lateral_force` sign row (sliding
  +X → force −X).
- Check: unit tests + G1, G2, G5, G7.

### Step 6 — `vehicle/seat.rs`: enter, exit, eject, seat sync (~220 lines)
- `enter_exit` (`VehicleSystems::Enter`): `std::mem::take(&mut action.vehicle_requested)` always. Player not `Dead`, not
  `Cuffed`, `HitReaction` not knocked down:
  - on foot: nearest car with `driver == None` whose door point (pos + rot·door, flat) is ≤ `enter_radius` from the
    player's flat position → `enter(...)`: player gets `Driving`, `RigidBodyDisabled`, `ColliderDisabled`,
    `TnuaToggle::Disabled`; head hitbox child (`Children` + `HeadHitbox`) gets `ColliderDisabled`; `LinearVelocity`
    zero; `*action = ActionIntent::default()` (direct mutation, after the take). Car: `driver = Some`,
    `SleepingDisabled`; `first = !taken`, `taken = true`; `VehicleEntered { attack: serial.next_id(), .. }`.
  - driving: car speed ≤ `exit_max_speed` → `exit(...)`: candidates door point, mirrored right door, roof
    (pos + up·(hy + float_height)); feet from a down ray (World) from candidate + 2 m; clear if
    `shape_intersections(capsule, centre, IDENTITY, [World, Vehicle, Character])` is empty; first clear wins; none → stay.
    Remove `Driving`, `RigidBodyDisabled`, `TnuaToggle`, both `ColliderDisabled` (`try_remove`); Position + Transform =
    candidate centre, velocity zero; car `driver = None`, remove `SleepingDisabled`.
- `eject_all` (OnEnter Wasted/Busted) and the seat-sync fallback use `exit` with a forced fallback to the door point.
- `sync_seats` (`VehicleSystems::Seat`): for `(player, Driving)`: car missing → eject in place; else Position +
  Transform.translation = car pos + rot·seat, Rotation + Transform.rotation = car rotation, `LinearVelocity` = car
  velocity. A car whose `driver` points at an entity without a matching `Driving` gets `driver = None`.
- `police/arrest.rs:32` query adds `Without<Driving>`; `combat/pickups.rs:57,132,197` player queries add
  `Without<Driving>`.
- Check: G3, G8.

### Step 7 — `vehicle/impact.rs`: crash impacts → damage (~200 lines)
- `record_pre_step` (`VehicleSystems::Record`): `PreStepVelocity = LinearVelocity` for all holders.
- `apply_impacts` (FixedUpdate, `HealthSystems::Damage`, ungated): `MessageReader<CollisionStart>`, `Collisions`,
  vehicles, `PreStepVelocity`, `Query<&mut Health, Without<Dead>>`,
  `Query<(&mut HitReaction, &mut TnuaController<CharacterScheme>)>`, `Res<MeleeConfig>`, `ResMut<AttackSerial>`,
  `Res<DamageConfig>`, writers `DamageDealt`, `VehicleHit`, `VehicleImpact`. Resolve `body1/body2`, skip pairs without a
  vehicle; first manifold normal of `collisions.get(c1, c2)` (none → skip), oriented vehicle→other (flip if the vehicle is
  `collider2`). Closing = `(v_veh_pre − v_other_pre)·n` (static/no body → 0).
  - other = character with `Health`: striker rule `v_veh_pre·n ≥ pedestrian.threshold_speed`, else no hit. Damage =
    round((closing − threshold)·per_mps), ≥ 1 → `Health::take`, `DamageDealt { shooter: driver.unwrap_or(vehicle),
    shot: attack, target, point: target position, damage, headshot: false, killed }`, `VehicleHit`;
    closing ≥ knockdown_speed → `escalate(true, melee_cfg)` + `knock_back(n_flat·closing·shove_scale)`. The vehicle takes
    no damage from characters.
  - other = world / vehicle: `VehicleHealth.current -= (closing − threshold)·per_mps` when positive (both cars for a car
    pair, clamp 0); write `VehicleImpact`.
  - One code line: `DamageDealt` from a car goes through every existing reader unchanged (perception Hurt,
    police hostility, gang provocation — skipped for a driverless car, no `Faction` — `record_crimes` — driverless car =
    no crime —, client audio/hit marker/damage numbers/arc/shake).
- Pure fns + unit rows: `pedestrian_damage(car_along_n, other_along_n, cfg)`: (10,0) → 84; (6,0) → 36; (2.5,0) → 0;
  (0,−6.8) → 0; (10,−2) → 108. `vehicle_damage(closing)`: 4 → 0, 10 → 200, 28 → 920.
- Check: G4, G6.

### Step 7b — bullets → car health (Q2 = c+a), `vehicle/impact.rs` (or `vehicle/bullets.rs` if impact.rs nears 400 lines)
- `apply_bullet_hits` (`VehicleSystems::Bullets`): `MessageReader<BulletHitVehicle>`, `Query<&mut VehicleHealth>`,
  `Res<DamageConfig>`: `current = (current − bullet_damage(hit.damage, bullet_scale)).max(0)`; unknown vehicle → skip.
  Stall at 0 = the crash rule (throttle 0). Shooting a car is not a crime (GDD §6.4); a shot near people still records
  `Shooting` via `ShotFired`.
- Unit row: `bullet_damage(25.0, 1.0) == 25.0`.
- Check: G12.

### Step 8 — perception, civilians, wanted
- `perception/mod.rs`: `ThreatKind::Car`; `PerceptionConfig` + `assets/npc/perception.ron`: `car_distance: 12.0`,
  `car_speed: 3.0` (validate > 0, sabotage rows in `config_vehicle.rs` or the existing perception config test). In
  `perceive`, each vehicle with `on_sidewalk && speed ≥ car_speed` within `car_distance` of the chest and not
  `sight_blocked` → `offer(Car, pos, distance, None)` (not a crime, like `Aimed`).
- `civilian/reaction.rs`: `Car` not reportable (same arm as `Aimed | Hurt`); `worked_reaction_table` rows
  `(Car, 5.0, (1,1,1), true, Flee)` (flee 1.0 ≥ cower 1.5·(1−5/15) = 1.0) and `(Car, 2.0, (1,1,1), true, Cower)`
  (cower 1.3 > 1.0).
- `wanted/crimes.rs`: `Crime::{RunOver, CarTheft}`; `enum HitSource { Gun, Melee, Vehicle }`;
  `classify(victim, source: HitSource, killed)` replaces `melee: bool` (update the call at `:206` and the unit test
  `:342`); Vehicle rows: civilian wound → RunOver, civilian kill → Kill, gang wound → None, gang kill → Kill, cop wound
  → WoundCop, cop kill → KillCop, other → None. `record_crimes` gains `MessageReader<VehicleHit>` (attack-id set, source
  Vehicle wins over Gun) and `MessageReader<VehicleEntered>` (15 params): `driver` is the player and `first` →
  `crimes.record(CarTheft, driver, Some(vehicle), attack, player pos, now, merge)` → `touched` (cop LOS witness only, Q4).
- `wanted/mod.rs` `HeatTable { .., run_over, car_theft }` + validate (> 0) + `of`; `assets/wanted/wanted.ron`:
  `run_over: 30, car_theft: 15`, comment "car rows: GDD §6.4". **[PR2]** Add `run_over: 30, car_theft: 15` to the
  unit-test `const HEAT: HeatTable` at `wanted/crimes.rs:309` (compile requirement).
- Unit tests: `classify_table` keeps its 12 rows (rewritten to `HitSource`) + one row per new Vehicle combination (7).
- Check: G11 + unit tests.

### Step 9 — headless gates `crates/gta_sim/tests/vehicle.rs` (+ `vehicle_city.rs`)
Helpers in the test file (`common/` is at 614 lines): `spawn_car(app, feet_xz, yaw_deg) -> Entity` via `vehicle_bundle`
at y = `rest_height()`; after one tick assert its Position equals the Transform (`GATE BROKEN` else); before spawning,
`shape_intersections(chassis box)` at the start pose must be empty (`GATE BROKEN: fixture overlaps the test area`);
`drive_in(app, car)` places the player at the door point, raises `vehicle_requested`, runs 1 tick, asserts `Driving`;
`kick(app, car, speed)` sets `LinearVelocity = forward·speed`; `set_drive(app, |d| ..)`; `forward_of(car)`.
All gates run in `composed_app`; expected values computed in the test from the loaded configs. Every flip is recorded in
the stage summary.

- **G1 wall** [correctness]: player settled, car at (0, h, −10) yaw 180° (forward +Z, door (1.7, 0, −9.7)), driven,
  settle 32 ticks, kick 28, throttle 1, 128 ticks sampled every tick: centre z ≤ 13.75 − 2.04 + 0.1 and y < 4.0 on every
  tick; liveness: speed ≥ 0.95·28 on some tick before contact, final speed ≤ 3. Flip-RED: car filter without `World` →
  passes z 14.25 → RED. (Margin flip does not go red — probe P2; say so in the test doc comment.)
- **G2 corner 45°** [correctness]: test static box 10×10×10 at (33, 5, −33) (x 28..38, z −38..−28), car at
  (17.4, h, −17.4) yaw −45°, door (16.41, −18.81) (§3 ex. 3), kick 28, throttle 1, 128 ticks: centre never inside the
  box (`!(x > 28 && z < −28)`); liveness as G1. Same flip.
- **G3 control ownership** [correctness], car at (−25, h, 0) yaw 0, door (−26.7, 0, −0.3):
  (a) on foot, DriveIntent throttle 1 for 64 ticks → car moves < 0.05 m;
  (b) enter → `Driving{car}`, `car.driver == player`, player has `RigidBodyDisabled`, `TnuaToggle::Disabled`, body and
  head `ColliderDisabled`; throttle 1 + MoveIntent axis Y 64 ticks → car moved ≥ 1.5 m along −Z (½·4.6·1² = 2.3 minus
  settle), |Δx| < 0.3, player Position within 0.01 of car pos + rot·seat;
  (c) throttle 0 + handbrake until speed < 3, exit → link cleared both sides, player offset·car right < −1.0, colliders
  back, no `TnuaToggle`; throttle 1 + MoveIntent Y 64 ticks → car < 0.05, player ≥ 1 m;
  (d) exit request at 10 m/s → still driving; (e) enter request 3.0 m from the door → nothing;
  (f) **[PR2]** latched fire: player holds an **SMG** (`Loadout` with `acquire` for `Weapon::Smg`, `held = Some(Smg)`),
  aimed at the car body, `fire_held = true` and `vehicle_requested = true` set in the same tick at the door; run 32 ticks
  → zero `ShotFired` from the player, zero `BulletHitVehicle`, `VehicleHealth` unchanged. Flip-RED: remove the
  `ActionIntent` reset in `enter` → the SMG keeps firing into the own car → RED. (A pistol here is vacuous:
  `SemiAutomatic` ignores `fire_held`.)
  Flip-RED of the table: chassis reads the player's intent regardless of `driver` → row (a) RED.
- **G4 pedestrian formula (AC)** [correctness]: civilian (`civilian_bundle` via `spawn_civilian`, state `Idle{left:100}`,
  test graph `[(−25,0,0), (−25,0,−20)]`) at (−25, 0, 0); car at (−25, h, 3.34) yaw 0 (front 1.0 m from the capsule), no
  driver (shooter = car), settle 32, kick v, throttle 0 (coast −0.125 m/s over ≤ 8 ticks). Rows, each its own case:
  v 10 → loss ∈ [82, 84] + knocked down; v 6 → [34, 36] + knocked down; v 2.5 → 0, not knocked down. Flip-RED: use
  post-step `LinearVelocity` instead of `PreStepVelocity` → row 10 ≈ 0 → RED.
- **G5 rest, no drift (AC)** [correctness], car at (−25, h, 0): (a) driven, no input: settle 64 then 640 ticks: drift
  < 0.02 m, |Δyaw| < 0.2°, centre y within 0.02 of `rest_height()` every sampled tick; (b) parked: same bounds and
  `Sleeping` by tick 640. Flip-RED: damper sign flipped (−c) → oscillation RED; or hold force removed + 0.02 m/s nudge
  → drift RED. Record which one was run.
- **G6 car self-damage + stall** [correctness], G1 geometry, throttle 0 after the kick: 4 m/s → health unchanged;
  10 m/s → loss ∈ [192, 200]; `VehicleHealth` 150 then 10 m/s → 0 → throttle 1 for 64 ticks: speed < 0.3,
  displacement < 0.1; car into the G4 pedestrian at 10 → car health unchanged. Flip-RED: ignore `health ≤ 0` in drive →
  stall row RED.
- **G7 no flip, steering sign** [correctness]: far floor `spawn_wall((500, −0.5, 500), (400, 1, 400))`; car at
  (500, h, 600) yaw 0, driven, kick 28, throttle 1, steer +1 for 192 ticks: every tick car up·Y ≥ 0.5, speed ≥ 8 at the
  end. Directional rows on the test floor from rest, throttle 1 for 64 ticks: yaw 0 → Δz < −1; yaw 90 → Δx < −1; yaw 180 →
  Δz > 1; yaw 0 + steer +1 → Δx > 0.2 and yaw < −5°. Flip-RED: `roll_influence` 1.0 and CoM at the box centre → roll RED
  (if it stays green, record it and flip by removing the μ·N clamp).
- **G8 flow while driving** [correctness]: (a) lethal `DebugDamage` while driving → in `Wasted` no `Driving`,
  `car.driver == None`, none of the four disabled components; after respawn (real-time advance as in `respawn.rs`) the
  player is at the hospital and MoveIntent moves it ≥ 1 m in 64 ticks; (b) 1 star, cop `Arrest` 1 m from the car, player
  passive for `arrest.seconds + 1` → still `Playing`; (c) forced `NextState(Busted)` while driving → ejected, respawn at
  station not driving; (d) car despawned while driving → next tick player on foot, colliders enabled, no `TnuaToggle`;
  (e) `ComputedMass` 1200 ± 1, yaw inertia 2240.6 ± 5 %.
- **G9 new city while driving + leak** (`vehicle_city.rs`): `composed_app(City{seed 1})`, baseline b0 = all entities
  minus `IsResource` after the first `app.update()` (before the city exists; same procedure as `new_city.rs`); run to
  `Playing`, enter the nearest parked car; pause; new city 2 → right after the transition frame the all-entity set
  equals b0 (car, player, Tnua sensors gone);
  `until_playing` → one player, no `Driving`, `count::<With<Vehicle>>() == City.parking.len()`, every `driver == None`.
- **G10 parked cars + perf** (`vehicle_city.rs`) [correctness + perf]: seed 1, player settled, 128 ticks: vehicle count
  = parking spots; each car within 0.05 m (xz) of its spot and y within 0.03 of `rest_height`; all `Sleeping`;
  `VehicleLoad.rays == 0` on the last tick. Flip-RED: waking forces for undriven cars → never sleep → RED.
- **G11 perception and crimes**: (a) test-spawned `CityBlock` prism 20×0.15×10 at (−25, 0.075, −20); civilian on it 8 m
  ahead; car driven onto the block at 6 m/s → civilian `Flee|Cower` within `slots + 2` ticks; car on the floor beside
  the block at the same distance → `Idle`; car on the block at 1 m/s → `Idle`. (b) cop (`police_support::spawn_unit`)
  with LOS 30 m: run over a civilian at 6 m/s → `WantedLevel.heat == 30`; same without a cop → 0; enter a parked car in
  view of the cop → 15; exit and re-enter → still 15.
- **G12 bullets vs car (Q2)** [correctness], car `spawn_car((−25, 0), 90°)`: centre (−25, 1.1596, 0), long axis X,
  +Z face at z = 1.2, door (−25.3, 0, 1.7), seat → player body centre (−24.9, 1.36, 0.45) (§3 ex. 6).
  - (a) player on foot at (−25, 0, 8) with a pistol, aim from its muzzle at the car centre, `fire_requested` once →
    exactly one `BulletHitVehicle` with `damage == 25.0`; `VehicleHealth` = 1000 − 25·`bullet_scale`; the
    `BulletTrace.to.z` within 0.05 of 1.2; no `DamageDealt`. Flip-RED: drop `Vehicle` from the bullet filter → no
    message → RED.
  - (b) stall via bullets: `VehicleHealth` 20, shoot once → 0 (clamped); `drive_in`; throttle 1 for 64 ticks →
    displacement < 0.1 m; exit request → on foot, no `Driving`. Flip-RED: ignore `health ≤ 0` in drive → car moves → RED.
  - (c) **[PR2] driver protected over the roof** (fixture replaces V2's; numbers in `scratch/pr2/g12c_geometry.txt`):
    1. `drive_in` (car stationary), settle 32 ticks. Read head centre = player `Position` + Y·(head_height −
       float_height) and roof y = car `Position.y` + `chassis_half_extents.y`; precondition `head.y + head_radius > roof`
       (expected 2.26 > 2.08) else `GATE BROKEN: head does not clear the roof, gate cannot fail`.
    2. Shooter platform `spawn_wall(center (−24.9, 1.5, 2.95), size (4.0, 3.0, 1.0))` → x −26.9..−22.9, y 0..3,
       z 2.45..3.45 (1.25 m from the car face, clear of the door point z 1.7 and of every test-area fixture).
    3. Shooter `spawn_dummy(feet (−24.9, 3.0, 2.65))` (centre 0.2 m inside the platform edge z 2.45) + insert a
       `Loadout` with `acquire(&mut guns[Weapon::Pistol.index()], stats, true)` and `held = Some(Weapon::Pistol)`. Run 64
       ticks; `GATE BROKEN` if the dummy moved > 0.05 m (fell off).
    4. Aim: iterate 4 times `dir = (head − from).normalize(); from = muzzle(dummy_pos, dir, aim_cfg.muzzle_offset())`
       (start `from = dummy_pos`); set the dummy's `AimIntent { origin: from, direction: dir }`. Expected muzzle
       ≈ (−24.72, 4.40, 2.17) — beyond the platform edge; elevation ≈ 55°, distance ≈ 3.04 m. `GATE BROKEN` if the
       elevation < 45° or `from.z > 2.40`.
    5. Fire 4 shots: raise `fire_requested` on the dummy, run 40 ticks between shots (pistol bloom fully recovered →
       1.0° half-cone every shot). `GATE BROKEN` if fewer than 4 `ShotFired` from the dummy.
    6. Assert: player `Health` unchanged; no `DamageDealt` with `target == player`; exactly 4 `BulletHitVehicle` for
       the car; `VehicleHealth` = start − 4·25·`bullet_scale` (= −100).
    Derivation: entry into the head sphere needs elevation > 29.06°; at 55° the zero-spread entry is at y 2.197 and the
    worst 1° pellet (offset 0.053 m) enters at y 2.163 — above the roof 2.080 → with the head enabled every pellet is a
    headshot. With the head disabled the ray crosses the roof 0.12 m short of the head centre, inside the roof → every
    pellet stops on the car.
    Flip-RED: skip `ColliderDisabled` on the head hitbox in `enter` → 4 headshots on the player (DamageDealt, health
    loss) → RED; restore → GREEN.

### Step 10 — client input `src/input/mod.rs`
- `add_input_context::<InVehicle>()`; the input entity gets `InVehicle` + `ContextActivity::<InVehicle>::INACTIVE` and
  `actions!(InVehicle[(Action::<Drive>::new(), Bindings::spawn(Cardinal::wasd_keys())), (Action::<Handbrake>, Space),
  (Action::<ExitVehicle>, ActionSettings { require_reset: true, .. }, KeyF), (Action::<DriveLook>, mouse motion)])`
  (exact binding syntax as the existing `OnFoot` actions); `OnFoot` gains
  `(Action::<EnterVehicle>, ActionSettings { require_reset: true, .. }, KeyF)`.
- `write_drive_intent` (Update, after `apply_mouse_look`): player `DriveIntent { throttle: drive.y, steer: drive.x,
  handbrake }`; `EnterVehicle`/`ExitVehicle` START → `ActionIntent.vehicle_requested = true`; cursor not captured → zero.
- Replace `deactivate_input`/`activate_input` with `sync_contexts` (Update): pure `context_activity(state, driving) ->
  (on_foot, in_vehicle)` = Paused → (false, false), else (!driving, driving); insert `ContextActivity` only when it
  differs from the current `Deref` value. Unit test: one row per `GameState` variant × driving.
- `camera::apply_mouse_look` reads `Action<Look>` and `Action<DriveLook>` (two `Single`s; the inactive one is zero).
- `release_held_actions` also zeroes `DriveIntent`.
- Check: `cargo test -p gta_like --bin gta_like -j 4` + owner run.

### Step 11 — client camera (`assets/camera/camera.ron`, `src/camera/{mod,config}.rs`)
- `camera.ron`: `car_distance: 6.5, car_pivot_height: 1.0, car_pitch_deg: -8.0, car_yaw_half_life: 0.2,
  car_look_return: 1.5`; validate > 0 except pitch within [pitch_min, pitch_max].
- `OrbitCamera.look_idle: f32` (reset by `apply_mouse_look` on a non-zero delta, grows by real dt).
- `follow_player`: with `Driving`, pivot = car interpolated Transform + up·car_pivot_height, no shoulder offset, distance
  `car_distance`; when `look_idle ≥ car_look_return`, yaw/pitch approach `aim_yaw(car forward)` / `car_pitch_deg` with
  half-life `car_yaw_half_life` via `approach_angle` (shortest arc; unit rows 170→−170 = +20, 10→−10 = −20, 0→180
  |Δ| ≤ 180). Collision cast unchanged (World only). AimIntent still written.
- Observer `On<Add, Vehicle>` → `TransformInterpolation`. Owner-judged.

### Step 12 — client visuals `src/visuals/vehicle.rs` + `assets/world/render.ron`
- `render.ron`: `vehicle: (model: "third_party/car-kit/sedan.glb", scale: 1.6, offset: (0.0, -1.16, -0.04),
  wheels: ["wheel-front-left", "wheel-front-right", "wheel-back-left", "wheel-back-right"])` (offset = −(0.24 + 0.92);
  z from the body node −0.025·1.6 flipped by the 180° model yaw); `RenderConfig::vehicle_asset_paths()`;
  `main.rs::preflight` checks the model is in the manifest.
- Observer `On<Add, Vehicle>`: `Visibility::default()` + child `WorldAssetRoot(scene)` with the offset, `R_y(π)`, scale;
  `WorldInstanceReady` observer (`bevy::world_serialization::WorldInstanceReady`) records the 4 wheel nodes by `Name`.
  Update: front wheels yaw = −steer (model frame), spin θ += v_long·dt/r (§3 ex. 5), hub y offset =
  (compression − x_eq)/scale.
- Player model `Visibility::Hidden` while `Driving`, back to `Inherited`.
- Stall smoke: `juice.ron smoke: (interval: 0.25, seconds: 1.2, size: 0.5, rise: 1.2, color: (0.25,0.25,0.25,0.6),
  hood: (0.0, 0.6, -1.6))`; pooled unlit sphere puff at the hood of every vehicle with `VehicleHealth ≤ 0` every
  `interval`, rising and fading over `seconds` (real time). Owner-judged, no gate.

### Step 13 — client audio (`assets/audio/mix.ron`, `src/audio/`)
- `synth.rs`: `Synth::Engine(EngineSynth { base_hz, harmonics, noise })`, endless decoder (sum of `harmonics` sines
  k·phase with 1/k amplitude + `noise` of the existing xorshift), `is_endless` true, played `Once` (lesson TASK-014).
- `cues.rs`: `SoundClass::Engine` (COUNT 9; excluded from the one-shot budget like `Siren`); `SoundBank.engine`;
  `impacts.vehicle` pool (5 `impactMetal_heavy` files) played spatially on `VehicleImpact` of the player's car or within
  audibility, class `Impact`.
- New `engine.rs` (Update, Playing|Wasted|Busted): one `EngineEmitter` child on the car the player drives
  (`PlaybackSettings::ONCE`, spatial, `spawn_sound`), despawned when not driving; each frame `sink.set_speed(pitch)` and
  `set_volume(gain·global)` from `engine_voice(speed_ratio, throttle, cfg)`: rpm = max(|v_f|/max_speed,
  |throttle|·rev_share).clamp(0,1); pitch = lerp(idle_pitch, max_pitch, rpm); gain = lerp(idle_volume, max_volume,
  max(rpm, |throttle|)). Unit rows: (0,0) → (idle_pitch, idle_volume); (0,1) → rpm = rev_share; (1,0) → (max_pitch,
  max_volume); reverse (−0.2, −1).
- `mix.ron`: `engine: (base_hz: 45.0, harmonics: 6, noise: 0.15, idle_pitch: 0.8, max_pitch: 2.4, rev_share: 0.35,
  idle_volume: 0.25, max_volume: 0.6, ref_distance: 8.0)`; `sync_loop_pause` includes `Engine`; `MixConfig::validate` +
  `check_sounds` cover the new pool; existing audio gates (`gate.rs`, `event_gate.rs`) updated where they enumerate
  classes / endless synths.

### Step 14 — HUD, minimap, juice
- `hud/mod.rs`: `HudBar::Vehicle` third bar (`strings.ron hud.vehicle_color`), `Display::None` unless driving; value =
  car health / `max_health`. `weapon::update_crosshair` hides the crosshair while driving.
- `minimap/markers.rs`: `MarkerKind::Vehicle`, a dot for every `Vehicle` except the driven one (Q3 = a)
  (`strings.ron minimap.vehicle_color`, validated); hidden beyond the rim by existing code.
- `juice/shake.rs::add_trauma`: `VehicleImpact` of the player's car → `speed·crash_trauma_per_mps` when
  `speed ≥ crash_min_speed` (`juice.ron shake: crash_trauma_per_mps: 0.02, crash_min_speed: 5.0`).

### Step 15 — assets manifest + fetch
- `assets/third_party/manifest.ron`: pack `car-kit` (version "3.1", page `https://kenney.nl/assets/car-kit`, url and
  hashes of §2, files `sedan.glb`, `Textures/colormap.png`, `License.txt`); `impact-sounds` gains the 5 metal files.
- `crates/gta_sim/tests/asset_manifest.rs`: names set + `"car-kit"`; exclude `car-kit` from the 4-file loop (`:101-106`)
  and assert separately `car-kit` has 3 files, CC0, no rig; impact-sounds 26 → 31; comment "30 impact .ogg + License".
- Offline install: `python tools/fetch_assets.py --cache maw/tasks/in_progress/TASK-015/scratch/carkit --cache
  maw/tasks/in_progress/TASK-015/scratch/impact`, then `--check`. Confirm the GLB texture reference resolves to the
  manifest path (`Textures/colormap.png`).

### Step 16 — runtime QA `tools/qa/scenarios/t14.py` (AC)
Style of `t12.py`, helpers from `t5`/`t6`/`t8`. `--seed 1`: wait `Playing` + chunks; read
`rows(game, ["Vehicle","Position","Rotation"])` (≥ 1 else FAIL); nearest car → door point from `sedan.ron` (regex) and
the car quaternion → mutate player `Position` + `Transform`; `send_keys(["KeyF"], 100)`; poll `Driving` ≤ 2 s (FAIL);
`send_keys(["KeyW"], 3000)` with screenshots every 0.5 s (≥ 0.15 s apart); car speed ≥ 5 m/s and displacement ≥ 5 m
(3 s at 4.6 m/s² ≤ 13.8 m/s); `SoundStats` Engine spawned ≥ 1; minimap markers of kind `Vehicle` > 0. Wall: mutate the
car to (0, 1.16, 670) facing +Z with zero velocity, `KeyW` 4000 ms (27.96 m at 4.6 m/s² → 3.49 s, ≈ 16 m/s → car loss
≈ 440 hp): `VehicleHealth` dropped, car z < 700 − 2.04 + 0.3, speed < 2; screenshot; `KeyF` → `Driving` gone ≤ 2 s,
player flat distance to car ≤ 3 m, `GameState` Playing, player `Health` readable; screenshot; shutdown. Hard pass/fail
on components only.
Owner checklist (QA_REPORT.md): сел (F у двери), поехал (W/S/A/D, Space ручник), handling устраивает (`sedan.ron`:
acceleration, max_speed, steer, grip, roll_influence), камера машины (`camera.ron` car_*), гул двигателя по скорости,
удар в стену (звук, тряска, полоса HUD), дым при нуле, сбить пешехода, выход, припаркованные машины на проспектах и
метки на мини-карте, полиция стреляет по машине → полоса HUD падает, на нуле машина глохнет.

### Step 17 — close-out
`cargo build -j 4`, `cargo clippy -j 4 -- -D warnings`, `cargo test -p gta_sim -j 4`, `cargo test -p citygen -j 4`,
`cargo test -p gta_like --bin gta_like -j 4` (touched presentation gates run 3 times), `python tools/qa/tree_check.py`,
`cargo tree -p gta_sim -e normal -i bevy_render` empty. Every file < 750 lines (vehicle split into
mod/config/chassis/seat/impact[/bullets]).

---

## 5. Test plan (summary)

| Gate | File | Class | Proves | Flip-RED |
|---|---|---|---|---|
| Tnua motor | `tests/tnua_motor.rs` | correctness | walker after a corpse still moves ≥ 3.5 m | `return` restored |
| Parking property + golden | `citygen/tests/{properties,golden}.rs` | correctness | spots on curb lanes; hashes change only by parking | procedure (a)-(d) |
| Config | `tests/config_vehicle.rs` | correctness | every validate rule fires with its keyword | per-row sabotage |
| G1/G2 | `tests/vehicle.rs` | correctness (AC) | no tunnelling at 28 m/s, wall + 45° corner | filter without World |
| G3 | same | correctness (AC) | enter/exit own control; no latched fire | intent ignores driver; no ActionIntent reset (SMG) |
| G4 | same | correctness (AC) | 10 m/s pedestrian = formula | post-step velocity |
| G5 | same | correctness (AC) | 640 ticks no drift, rest height | damper sign / hold removed |
| G6 | same | correctness | car self-damage + stall | ignore health ≤ 0 |
| G7 | same | correctness | no roll-over, steering/drive signs | roll 1.0 + CoM centre |
| G8 | same | correctness | Wasted/Busted/arrest/despawn while driving; mass | — (state rows) |
| G9/G10 | `tests/vehicle_city.rs` | correctness + perf | new city no leak; parked cars sleep, 0 rays | waking forces |
| G11 | `tests/vehicle.rs` | correctness | Car threat, RunOver 30, CarTheft 15 once | per row |
| G12 | same | correctness | bullets stop on and damage the car; stall; driver unhittable over the roof | filter w/o Vehicle; ignore stall; head not disabled |
| Unit | in-file | correctness | pure fns rows (§4 steps 5, 7, 7b, 8, 10, 11, 13) | — |
| t14.py | runtime | liveness | enter, drive, wall hit, exit in the real game | — |
| Owner | QA_REPORT | feel | handling, camera, sound, visuals | — |

Existing tests: all `-p gta_sim`, `-p citygen`, `-p gta_like --bin gta_like` stay green; after Step 0 any shifted
number is re-derived with a reason.

---

## 6. Rollout notes
- No migrations, env vars or feature flags. `Cargo.lock` unchanged (no new crate).
- Vendored crate change (Step 0) + ADR-001 update: the next re-vendor must re-apply both changes.
- citygen `HASH_SCHEMA_VERSION` 2 → 3 and golden rebless (Step 1 procedure); `t2.py` and `gta_sim` tests read the same
  file.
- New data files `assets/vehicle/sedan.ron`, `assets/vehicle/damage.ron`; new fields in `city.ron`, `perception.ron`,
  `wanted.ron`, `camera.ron`, `render.ron`, `juice.ron`, `mix.ron`, `strings.ron` (all `deny_unknown_fields` → every
  shipped file must be updated in the same commit).
- Third-party assets: Car Kit pack + 5 metal sounds via the manifest and the offline cache; `assets/third_party/*` stays
  ignored except `manifest.ron`.
- `GameLayer::Vehicle` changes what bullets stop on (cars block fire for player, cops, gangs); perception, witness and
  camera casts stay World-only (cars do not block sight).
- T15 notes: the plugin tuple must not grow past 15 (VehiclePlugin is a separate call); parked cars occupy the avenue
  outer lane; wounding a driver through windows (Q2 b) is deferred — it will need the head collider enabled with a
  window rule, and G12(c) will change then.

---

## 7. Review notes (what changed from PLAN_V2 and why)
1. **G12(c) fixture replaced [PR2].** V2's shooter (3 m block at (−25, 0, 8)) shoots at 19.3° elevation; the head is
   reachable over the roof only above 29.06° → the flip could never go RED. New fixture (platform 1.25 m from the car,
   55° elevation, derived entry heights with spread, `GATE BROKEN` preconditions on head clearance, elevation and muzzle
   position). Question settled: the head does poke 0.18 m above the roof and a steep shot reaches it; head
   `ColliderDisabled` is load-bearing.
2. **G3(f) used a pistol with `fire_held` [PR2]** — `SemiAutomatic` never fires on `fire_held`, so the flip was
   vacuous. Now SMG (`Automatic`).
3. **Enter vs fire order on the enter tick was undefined [PR2].** `VehicleSystems::Enter.before(HealthSystems::Damage)`
   so the `ActionIntent` reset precedes `tick_loadouts`/`fire_weapons`; without it G3(f) is order-dependent (flaky).
   No cycle: Enter → Damage (Impact inside) → Bullets → Drive.
4. **`HEAT` const exists [PR2]** at `wanted/crimes.rs:309` (unit test); V2 said updating it was a no-op. Adding
   `HeatTable` fields without it fails to compile.
5. **Tnua gate precondition [PR2]** now iterates `(Entity, &TnuaMotor, Forces)` — the motor system's own query data — so
   it proves the corpse is in the motor loop, not only that it has a `TnuaMotor`. Civilian fixture made exact (test graph,
   `GraphWalker`).
6. **`fire_weapons` param count [PR2]:** 13 today + layers query + `BulletHitVehicle` writer = 15 (V2 counted 14 → 15
   for the query alone). Exact insertion point of the vehicle branch specified.
7. **Detail restored from PLAN** where V2 wrote "as PLAN": every step's files, types, numbers, worked examples, rows and
   flips (Steps 1, 3-13, 16), research sources, content hashes.
8. Kept from V2 unchanged (re-checked): vendored fix + ADR update, `TnuaToggle::Disabled` for the driver, `ActionIntent`
   reset on enter, asset-manifest 4-file loop exclusion, `VehiclePlugin` outside the tuple, `BulletHitVehicle` without
   RNG draw, `DamageDealt` consumer audit, corrected line numbers, physics numbers and fixture clearances.

children: 0 launched / 0 reported.
