# PLAN — TASK-015 (GDD T14): drivable car

Cost of error, one line: the physics (tunnelling, suspension equilibrium, rollover, impact speed), the enter/exit
link between two entities, Wasted/Busted/new-city while driving and the parked-car perf budget break SILENTLY and get
full gates; handling, camera, engine sound, smoke and visuals are seen by the owner on the first frame and get the
owner checklist only.

Binding inputs honoured: PREMISE_CHALLENGE (parked-car points belong to T14 → added to citygen with the golden-hash
impact stated; car self-damage + stall get their own named gate), Orchestrator note 1-5 (every row its own case,
posed fixtures carry `Transform`, leak gates count all entities minus `IsResource`, no client run, probes with `-j 4`).

---

## 1. Understanding (what exists today)

### Simulation (`crates/gta_sim`)
- `lib.rs:41-174` `compose_sim`: loads every RON with `load_config` + `validate`, inserts resources, adds the plugin
  tuple (Flow, `PhysicsPlugins::default()`, `TnuaAvian3dPlugin::new(FixedUpdate)`, Character, World, Player, Combat,
  Navigation, Perception, Population, Civilian, Gang, Police, Wanted). No vehicle domain, no `assets/vehicle/`.
- `layers.rs:5-9` `GameLayer { World (default), Character, Hitbox }` — GDD §12 law also lists `Vehicle`.
- `character/mod.rs:32-44` `Character` requires `MoveIntent, AimIntent, ActionIntent, …, CityScoped`;
  `character_components` (`:152-176`): dynamic capsule, `CollisionLayers::new(Character, ALL)`, child head sensor
  (`head_hitbox`, `Hitbox` layer, filter NONE). `intent.rs:286-295` `ActionIntent` = latches the client raises and the
  fixed tick clears. `drive_characters` (`:179-250`) feeds Tnua.
- `player/mod.rs:27-35` player spawns once on `OnTransition{Loading→Playing}`; because `Character` requires
  `CityScoped`, `world/mod.rs:115-119` `despawn_city` removes the player on `NEW_CITY` and a new one spawns.
- `flow/mod.rs`: `PlayingSystems` (Playing only), `NpcSystems` (Playing|Wasted|Busted), `NEW_CITY = Paused→Loading`;
  `wasted.rs:286-300` `respawn_at` teleports the SAME player entity (Position+Transform+velocity);
  `drop_queued_input` resets `ActionIntent` on leaving Wasted/Busted.
- `world/test_area.rs:9-23` fixture level: 80×1×80 floor (top y = 0), boxes at x ≈ 10..16, ramp/box at x ≈ −10,
  box (10, 0.8, −17.4), stairs at x = 10, **wall 12×4×0.5 at (0, 2, 14)** (near face z = 13.75). `PlayerSpawn = 0`.
- `world/city.rs:259-310` async generation → `City`, `PlayerSpawn`, colliders; ground slab top at y = 0, block
  prisms (`CityBlock`) up to the curb (0.15 m), edge walls 1 m thick just outside `ground_size/2 = 700`.
- `combat/hitscan.rs:167-168` bullet filter `[World, Character, Hitbox]`; `DamageDealt` (`:89-101`) is the one damage
  message; `AttackSerial::next_id` (`combat/mod.rs:236-244`) ids every attack. `melee.rs:251-277` `HitReaction`
  (`escalate(knockdown, cfg)`), `:549` `knock_back(controller, shove)`.
- `perception/mod.rs:53-60` `ThreatKind {Gunshot, Fight, Corpse, Aimed, Hurt}`; `collect_stimuli` turns a
  `DamageDealt` on a civilian into `Hurt`; `perceive` (`:231-310`) is time-sliced; GDD §6.2 also lists
  "машина, въехавшая на тротуар рядом" — not implemented yet. `civilian/reaction.rs:324-352` utility choice.
- `wanted/crimes.rs:16-25` `Crime` has no car rows; `classify` (`:149-159`) takes `melee: bool`;
  `record_crimes` (`:162-271`) builds the melee set from `MeleeHit`; witness = cop LOS. `wanted/mod.rs:463-486`
  `HeatTable`; `assets/wanted/wanted.ron:2-3` says "Car rows come with T14/T15". GDD §6.4: car theft 15, run over 30.
- `police/arrest.rs:32` arrests any live player — GDD §6.4 says a player in a car is never arrested.
- `combat/pickups.rs:57,132,197` player pickup queries.
- `population/mod.rs:233-241` `corpse_components` uses `TnuaToggle::Disabled` (see Risk R9).

### Client (`src/`)
- `input/mod.rs`: one entity with the BEI context `OnFoot` (Move, Look, Sprint, Walk, Jump, Fire, Aim, Reload,
  Slot1-4, CycleWeapon); `deactivate_input`/`activate_input` toggle only `OnFoot` on Paused. `Single<&Action<Look>>`
  in `camera/mod.rs:320`.
- `camera/mod.rs:339-406` `follow_player`: orbit from the player head, World-only collision cast, writes `AimIntent`;
  `camera.ron` has no car values; `enable_player_interpolation` inserts `TransformInterpolation` on the player.
- `minimap/markers.rs:311-318` `MarkerKind {Police, Gang, Pickup, Hospital, Station}`, `sync_markers` + hide-beyond-rim.
- `audio/`: `synth.rs` endless procedural decoders played `Once` (lesson TASK-014), `loops.rs` siren/ambience loops +
  `sync_loop_pause`, `cues.rs` `SoundClass` (COUNT 8), `spawn_sound`, `SoundStats`, pools; `mix.ron`.
- `hud/mod.rs` health/armour bars; `juice/shake.rs:21-62` trauma rows; `visuals/character.rs:300-330` scene-root
  child model + `WorldInstanceReady` observer pattern; `main.rs:65-166` `preflight` checks every third-party path.
- `assets/third_party/manifest.ron`: no Car Kit; `impact-sounds` lists 25 .ogg (no metal).

### Engine facts verified in pinned source / by probe
- avian3d 0.7.0 `NarrowPhaseConfig::default_speculative_margin = Scalar::MAX` (`collision/narrow_phase/mod.rs:252`);
  `SpeculativeMargin`, `SweptCcd` (`dynamics/ccd/mod.rs`). CollisionStart/End written in
  `PhysicsStepSystems::Finalize` "after the solver" (`collision_events.rs`, `narrow_phase/mod.rs:129-141`), only for
  colliders with `CollisionEventsEnabled`. `ContactManifold::normal` world-space, from collider1 to collider2
  (`contact_types/mod.rs:354`). `Collisions::get(e1, e2) -> Option<&ContactPair>` (`contact_types/system_param.rs:71`).
- **Probe** (`scratch/probe_collision/src/main.rs`, output `scratch/probe_collision/run.log`, built with
  `CARGO_TARGET_DIR=D:/test-gta-like/target -j 4 --offline`):
  - P1: a 1200 kg box at 10 / 28 m/s into a 70 kg rotation-locked capsule. `CollisionStart` lands on the SAME tick
    as the velocity jump (tick 17: ped 0 → −9.56, car −10 → −9.44, impulse 669 = 70·9.56); the velocity read BEFORE
    that step is the full 10.00 / 28.00, the post-step closing speed is ≈ 0.12. One `CollisionStart` per hit.
  - P2: the car-sized box (4.1 m) at 28 m/s (0.4375 m/tick) into the 12×4×0.5 wall: default margin stops it at the
    face (z 2.331 = 0.25 + 2.05) and it stays on the near side for 128 ticks; `SpeculativeMargin::ZERO` penetrates
    0.36 m in one tick and is pushed back — no tunnelling either. 45° into a 10 m cube corner: stops at the corner,
    normal (−0.707, 0, −0.707), both settings.
- Forces: `Forces` QueryData (`dynamics/rigid_body/forces/query_data.rs:107-121`), `apply_force_at_point`,
  `velocity_at_point`; `non_waking()` drops forces on a `Sleeping` body (`:797-801`), the waking variant resets
  `SleepTimer` (`:687-693`). Sleeping: islands, `SleepThreshold` 0.15, `TimeToSleep` 0.5 s, `SleepingDisabled`
  wakes the island every step (`islands/sleeping.rs:163-181`), new contacts wake islands.
- `RigidBodyDisabled` disables velocity/forces/contacts but not spatial queries (`rigid_body/mod.rs:330-380`);
  `ColliderDisabled` removes a collider from collisions AND spatial queries, **only on its own entity, not children**
  (`collision/collider/mod.rs:380-394`, `collider_tree/update.rs:204`).
- Mass: `ColliderDensity`, `Mass`, `CenterOfMass`, `ComputedMass`, `ComputedAngularInertia`; the docs do not state
  that a `Mass` override rescales the collider's angular inertia → use density (Step 5).
- `Gravity::default() = (0, −9.81, 0)` (`dynamics/integrator/mod.rs:158-162`).
- `SpatialQuery::cast_ray(origin, Dir3, max, solid, &filter)` (`spatial_query/system_param.rs:111`),
  `shape_intersections(&Collider, pos, rot, &filter) -> Vec<Entity>` (`:1173`).
- bevy-tnua-avian3d 0.12.1 `apply_motors_system` (`vendor/.../src/lib.rs:419-431`, same upstream) **`return`s** from the
  whole loop at the first entity with `TnuaToggle::Disabled | SenseOnly` → do not use `TnuaToggle` for the driver.
- bevy_enhanced_input 0.26.0: several contexts on one entity (`context.rs:834` example uses `OnFoot` + `InCar`);
  `ContextActivity<C>::{ACTIVE, INACTIVE}` immutable, `Deref<bool>` (`context.rs:724-744`); inactive context zeroes its
  actions; `ActionSettings { require_reset, .. }` is a component required by `Action<A>` (`action.rs:73-77,172-204`):
  `require_reset` stops a held F from firing the other context's F.
- bevy_audio 0.19.1 `AudioSinkPlayback::set_speed` on `AudioSink` and `SpatialAudioSink` (`sinks.rs:38,184,288`).

### Content (downloaded by the planner, zips in scratch for the offline fetch cache)
- Kenney Car Kit 3.1, CC0: `https://kenney.nl/media/pages/assets/car-kit/1a312ec241-1775131960/kenney_car-kit.zip`,
  archive sha256 `fac7dacac5c7874348cf19729af3ef205f3d366493edaf0a827d93f4fdf3d0c4`, cached at
  `maw/tasks/in_progress/TASK-015/scratch/carkit/kenney_car-kit.zip`. Files:
  `Models/GLB format/sedan.glb` `b532ea7d2c59f7f6b22b138cf1955218a2c1898f1cea932af4d3fd563c3959b7`,
  `Models/GLB format/Textures/colormap.png` `f3622a03a20c6696065cae9cbe391351be873508af190c2ebd1d420c055787a5`,
  `License.txt` `c33b7f6453d134deae7b1b8493717d9ccfa754c25ab97f6de89b88f8fda19b00`.
- `sedan.glb` (parsed, `scratch/carkit/sedan_nodes.txt`): scene nodes `body`, `wheel-front-left/right`,
  `wheel-back-left/right`, no animations; model front = **+Z**; body bounds x ±0.75, y 0..1.15 (node at y 0.15),
  z ±1.275 (node z −0.025); wheel hubs (±0.3, 0.3, ±0.66), radius 0.3, the wheel mesh extends outward to |x| 0.6.
- Impact sounds (already in the manifest, archive sha matches `029d734a…`), cached at
  `scratch/impact/kenney_impact-sounds.zip`; metal files to add: `Audio/impactMetal_heavy_000.ogg`
  `e07045693e4a2b3d165c424e3dab4c781d9ff8880a386880ac89a51315d7f831`, `_001`
  `83554049f81f4db9209379e103c30bfa63f65c42189a03f300b045c2c82e23ae`, `_002`
  `b914c8f1eb7c0f34bb165d7c77f4be0351f6be0660c13c53e65424e262e2c093`, `_003`
  `b0f2ba4dabde9a87eb9c188a19d31e0c2300fd321adeba08d3b9b8aa011d7037`, `_004`
  `6d65b463c0555dd5be16b8db6d2cbe23a94e07a4637779b8ad17d0db3e500a87`.

No new crate: avian3d, bevy, BEI, rodio are already in `Cargo.lock`; the lock does not change.

### Research (web)
- Raycast car = one rigid body + per-wheel suspension ray, spring + damper along the wheel up axis, lateral force that
  cancels the sliding velocity at the tyre, drive/brake along the tyre forward (Toyful Games, "Making Custom Car Physics
  in Unity (for Very Very Valet)", https://www.youtube.com/watch?v=CdPYlj5uZeI).
- Rollover: tyre forces act at ground level below the centre of mass (Wikipedia, Vehicle rollover,
  https://en.wikipedia.org/wiki/Vehicle_rollover; static stability factor t/2h). Bullet's `btRaycastVehicle` scales the
  side-impulse lever arm by `m_rollInfluence` (0 = no roll); its known bug scales a WORLD-space offset, making turning
  heading-dependent — the fix scales along the body up axis
  (https://pybullet.org/Bullet/phpBB3/viewtopic.php?f=9&t=5832, source
  https://github.com/bulletphysics/bullet3/blob/master/src/BulletDynamics/Vehicle/btRaycastVehicle.cpp). Our
  `roll_influence` scales the lift along the chassis up axis.
- CCD: avian docs (in source) recommend speculative contacts by default and swept CCD only when a body must never
  tunnel; the GDD §5.1 already says "SweptCcd only if the gate fails".

---

## 2. Approach

**One sim domain `vehicle/`** (GDD §12 map) with a single archetype `Vehicle`: dynamic avian body, chassis box on the
new `GameLayer::Vehicle`, 4 raycast wheels computed inside one system (no wheel entities → no child leaks), drive /
brake / hold / handbrake / speed-sensitive steering, all numbers in `assets/vehicle/sedan.ron`; impacts from avian
`CollisionStart` + pre-step velocities + contact normals, formulas in `assets/vehicle/damage.ron`.

**Control ownership**: the player stays the same entity (respawn/HUD/police/minimap all keep working). Enter puts
`Driving { vehicle }` on the player, `RigidBodyDisabled` + `ColliderDisabled` on the body and its head hitbox child,
`Vehicle.driver = Some(player)` + `SleepingDisabled` on the car; the car reads the driver's `DriveIntent`; the player
is pinned to the seat after every physics step. Exit/eject is the exact inverse. Not `TnuaToggle` (R9).

**Why raycast suspension and not joints**: GDD §5.1 fixes it; no controller exists in avian 0.7; the probe shows the
chassis box handles walls itself. **Tunnelling decision (explicit)**: keep avian's default unbounded speculative
margin, no `SweptCcd` — the probe proves the 4 m chassis cannot tunnel the 0.5 m wall at 0.4375 m/tick even with
`SpeculativeMargin::ZERO`; the gate stays (cheap, catches a layer/collider regression) and is flipped RED by breaking
the car's collision layers, not the margin.

**Impact speed** = `(v_a_pre − v_b_pre)·n` with velocities recorded in `FixedPostUpdate` before
`PhysicsSystems::First` (after Tnua motors, which write `LinearVelocity` inside `FixedUpdate`) and `n` from the contact
manifold; processed the next tick in `HealthSystems::Damage`, so car damage joins the existing damage→crime→death
pipeline. The probe shows post-step velocities would give ≈ 0.

**Parked cars**: citygen emits `parking: Vec<ParkingSpot>` on avenue curb lanes from its own RNG stream; the sim
spawns them on `Loading→Playing`; they fall asleep (undriven cars use non-waking forces and cast no rays while
`Sleeping`) so 100+ parked cars cost ~0 per tick.

**Reuse**: perception gets `ThreatKind::Car` (GDD §6.2 "машина на тротуаре"); wanted gets `RunOver` 30 and
`CarTheft` 15 (GDD §6.4) via the existing incident/witness path; audio gets `Synth::Engine` + `SoundClass::Engine`
driven by `set_speed`; minimap gets `MarkerKind::Vehicle`; input gets the `InVehicle` context; HUD gets the car bar
(GDD §7); juice gets crash trauma; `SoundClass::Impact` gets a metal pool.

### Worked physics numbers (all derive from `sedan.ron`; Kenney sedan × `render.ron` scale 1.6)
- Chassis box 2.4 × 1.84 × 4.08 m (half 1.2, 0.92, 2.04); volume 18.017 m³ → density 1200 / 18.017 = 66.6 kg/m³.
- Body bottom 0.15·1.6 = 0.24 m above the road → chassis centre at rest **1.16 m** above the road.
- Wheels: radius 0.3·1.6 = 0.48; contact centre |x| = 0.45·1.6 = 0.72 (half track); |z| = 0.66·1.6 = 1.06
  (half wheelbase); hub at rest 0.48 above road = −0.68 below the centre.
- Suspension f = 1.5 Hz, ζ = 0.4, travel 0.3 (GDD): ω = 2π·1.5 = 9.4248, ω² = 88.83; per-wheel mass m/4 = 300 kg →
  k = 300·88.83 = 26 649 N/m, c = 0.4·2·√(26 649·300) = 2 262 N·s/m; static compression x_eq = g/ω² = 9.81/88.83 =
  0.1104 m; spring length at rest 0.3 − 0.1104 = 0.1896 → `mount_height` = −0.68 + 0.1896 = **−0.49**.
  Derived rest height h = −mount_height + (travel − g/ω²) + radius = 0.49 + 0.1896 + 0.48 = **1.1596 m**. Full
  compression: centre at 0.97 m, chassis bottom 0.05 m above the road (no scraping before the bump stop).
  Stability: ω·dt = 9.42/64 = 0.147 rad/step, 2ζω·dt = 0.118 — explicit spring is stable at 64 Hz.
- Drive: `acceleration` 4.6 m/s² → 0→27.8 m/s in 6.0 s (GDD "0-100 ≈ 6 s"); per rear wheel 1200·4.6/2 = 2 760 N vs
  grip limit μ·load = 1.1·2 943 = 3 237 N (not clamped). Brake 9 m/s² → 2 700 N per wheel (not clamped).
- Rollover: CoM 0.5 m below the box centre → 0.66 m above the road; SSF = 1.44/(2·0.66) = 1.09 ≈ μ 1.1, i.e. tyre
  forces at the contact point would roll a sliding car — hence `roll_influence` 0.3 (effective lever 0.198 m →
  SSF 3.6).
- Yaw inertia check: m/12·(w² + l²) = 100·(5.76 + 16.646) = **2 240.6 kg·m²**.
- Impact (damage.ron): pedestrian `(v − 3)·12` hp, knockdown ≥ 4 m/s, striker rule below; vehicle `(v − 5)·40` of
  1000 hp. 10 m/s → 84 (pedestrian), 200 (car); 28 m/s head-on → 920 (car).

### Directional examples (sign errors pass magnitude tests)
Body frame: x right, y up, forward = −Z (GDD §3.2 law). `R_y(θ)(x,y,z) = (x cosθ + z sinθ, y, −x sinθ + z cosθ)`.
1. Drive: yaw 0 → forward (0,0,−1), throttle +1 moves −Z; yaw 90° → R_y(90)(0,0,−1) = (−1,0,0), moves −X;
   yaw 180° → (0,0,+1), moves +Z.
2. Steer: input +1 = D = right turn = clockwise from above = yaw decreasing. Front-wheel forward in body frame =
   R_y(−δ)(0,0,−1) = (sin δ, 0, −cos δ); δ = 30° → (0.5, 0, −0.866), right-forward ✓; right = forward × up =
   (0.5,0,−0.866)×(0,1,0) = (0.866, 0, 0.5).
3. Door (left side, body (−1.7, 0, −0.3)) of a car at yaw −45° (forward (0.707,0,−0.707), right (0.707,0,0.707),
   back (−0.707,0,0.707)): world = −1.7·right − 0.3·back = (−1.202+0.212, 0, −1.202−0.212) = (−0.990, 0, −1.414);
   the same by R_y(−45°): x' = −1.7·0.707 + (−0.3)(−0.707) = −0.990, z' = −(−1.7)(−0.707) + (−0.3)(0.707) = −1.414 ✓.
4. Camera yaw of a car: `aim_yaw(rot·NEG_Z) = atan2(−f.x, −f.z)`: f (0,0,−1) → 0; (−1,0,0) → 90°; (0,0,1) → 180°.
   Yaw return uses the shortest arc: current 170°, target −170° → +20° (to 190° ≡ −170°), never −340°.
5. Visual wheel spin: the model is rotated 180° about Y, so in the model frame forward is +Z; a wheel rolling forward
   rotates its top toward +Z: R_x(θ)(0,1,0) = (0, cosθ, sinθ) → θ increases with forward speed (Δθ = v_long·dt/r).

---

## 3. Steps

Order = dependency order. Each step names its check.

### Step 1 — `crates/citygen`: parking spots (+ golden impact)
- `src/params.rs`: `CityParams.parking: ParkingParams { spacing: f32, chance: f32, end_margin: f32, curb_offset: f32 }`
  (`deny_unknown_fields`); `validate`: spacing ≥ 5 (a car length), chance ∈ [0,1], end_margin ≥ 0, 0 < curb_offset ≤
  `roads.lane_width`. Test `validate_rejects_bad_params` gets one row per rule (strictly failing values).
- `assets/world/city.ron`: `parking: (spacing: 40.0, chance: 0.25, end_margin: 15.0, curb_offset: 1.625)` —
  comment: spot centre `curb_offset` m in from the curb = outer-lane centre on an avenue (3.25/2).
- `src/layout.rs`: `pub struct ParkingSpot { pub position: Vec2, pub heading: Vec2 }` (unit lane direction) and
  `CityLayout.parking: Vec<ParkingSpot>`.
- `src/rng.rs`: `pub(crate) const PARKING: u64 = 16;`.
- New `src/parking.rs` (≈ 60 lines): for every `RoadClass::Avenue` edge `e` (index order), both directions
  `(from,to)` in `[(a,b),(b,a)]` exactly as `graphs::lanes` (d = (q−p)/|q−p|, right = d.perp() — the driver's right,
  `graphs.rs:100-102`), one `stream(seed, PARKING, e as u64)` per edge (both directions draw from it in order);
  stations `s = end_margin, end_margin + spacing, … ≤ len − end_margin`; each kept with `chance(rng, p)`; position =
  `p + d·s + right·(half_carriageway(Avenue) − curb_offset)`, heading = d. Streets/alleys get none (Open question Q1).
  Called last in `generate` (`lib.rs:40`) so no existing stream moves.
- `src/hash.rs`: after `w.vec2(layout.player_spawn)` append `w.len(parking.len())` and each `position`, `heading`;
  bump `HASH_SCHEMA_VERSION` 2 → 3.
- **Golden procedure (stated impact: all three hashes change, geometry unchanged)**: (a) implement parking without the
  hash change → `cargo test -p citygen --test golden` stays GREEN (proves no existing field moved); (b) add the hash
  lines + schema bump → golden RED on all 3 seeds; (c) bless with the command in `golden_hashes.txt` and paste 3 lines;
  (d) `tests/minimap.rs` raster golden must stay green untouched (parking is not rasterised). Record (a)-(d) in the
  stage summary. `gta_sim` tests and `tools/qa/scenarios/t2.py` read the same file — no other edit.
- `tests/properties.rs` new property over `layouts()` (SEEDS + SWEEP): every seed has ≥ 1 spot; each spot is within
  0.01 m of `half_carriageway(Avenue) − curb_offset` from the centre line of an avenue edge, ≥ `end_margin` − 0.01 from
  both its nodes, `heading` parallel (|cross| < 1e-4) to that edge; no spot lies inside any block `curb` polygon;
  two spots in the same direction are ≥ `spacing` − 0.01 apart. Report the per-seed count range (target 60..200 on
  seeds 1/2/42; tune `chance` if outside, record the final numbers).
- Check: `cargo test -p citygen` green; the stage summary lists old/new golden lines.

### Step 2 — `crates/gta_sim/src/layers.rs`
- Add `Vehicle` to `GameLayer` (after `Hitbox`, law, GDD §12). Existing `LayerMask::ALL` masks include it.
- `combat/hitscan.rs:168`: bullet filter `[World, Character, Hitbox, Vehicle]` — a bullet stops on a car body
  (`TraceHit::World`; a car has no `Health`). Perception/camera/witness stay World-only (cars do not block sight).
- Check: existing `shooting.rs` green.

### Step 3 — configs `assets/vehicle/sedan.ron`, `assets/vehicle/damage.ron` + `vehicle/config.rs`
- `VEHICLE_CONFIG = "vehicle/sedan.ron"`, `DAMAGE_CONFIG = "vehicle/damage.ron"`; all structs `deny_unknown_fields`.
- `sedan.ron` (every value named in units, comments carry the derivations of §2):
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
- `damage.ron`: `vehicle: (max_health: 1000.0, threshold_speed: 5.0, per_mps: 40.0)`,
  `pedestrian: (threshold_speed: 3.0, per_mps: 12.0, knockdown_speed: 4.0, shove_scale: 0.6)`.
- `VehicleConfig::validate`: all finite; mass, extents, radius, travel, frequency, damping_ratio, max/reverse speed,
  acceleration, brake, coast ≥ 0, hold_speed, enter_radius, exit_max_speed > 0; `travel > g/(2πf)²` (else the car rests
  on the bump stop — message names `suspension.travel`); `steer.at_max_speed_deg ≤ max_deg`; `roll_influence ∈ [0,1]`;
  `handbrake_rear ∈ [0,1]`; `|door.x| > chassis_half_extents.x` (door outside the body). Derived helpers (pure):
  `spring_rate()`, `damper_rate()`, `rest_height()` (= −mount_height + travel − g/ω² + radius, g = 9.81 law const
  `GRAVITY` shared with the test), `chassis_density()`. `DamageConfig::validate`: thresholds ≥ 0, per_mps > 0,
  max_health > 0, knockdown_speed ≥ threshold_speed.
- `lib.rs compose_sim`: load + validate both; for `WorldSource::City` also check that a parked car stays inside its
  lane: `curb_offset ≥ chassis_half_extents.x` (does not cross the curb; shipped 1.625 ≥ 1.2) and
  `curb_offset + chassis_half_extents.x ≤ roads.lane_width` (does not cross the lane line; 2.825 ≤ 3.25); error path =
  `world/city.ron`, message names `parking.curb_offset`. Insert resources; add `VehiclePlugin` after `CombatPlugin` (uses `AttackSerial`).
- New test file `crates/gta_sim/tests/config_vehicle.rs` (config.rs is at 748 lines): shipped files load+validate;
  unknown field names file and field; one sabotage per validate rule via `common::sabotaged` with values strictly on
  the failing side (e.g. `travel: 0.3` → `0.1` fails, message contains `suspension.travel`; each fixture's error is a
  different keyword); the compose-time curb check via a sabotaged `city.ron` copy.
- Check: `cargo test -p gta_sim --test config_vehicle`.

### Step 4 — `vehicle/mod.rs`: components, messages, bundle, plugin
- Components (all `Reflect` + registered, BRP reads them):
  - `Vehicle { driver: Option<Entity>, steer: f32 (rad, + = right), on_sidewalk: bool, taken: bool, wheels: [WheelState; 4] }`,
    `#[require(VehicleHealth, PreStepVelocity, CityScoped)]`; `WheelState { compression: f32, grounded: bool }`;
    wheel index law `const WHEELS: [(f32 /*x sign*/, f32 /*z sign*/); 4]` = front-left (−,−), front-right (+,−),
    back-left (−,+), back-right (+,+) (front = −Z).
  - `VehicleHealth { current: f32 }` (Default = 0; the bundle sets `max_health`).
  - `Driving { vehicle: Entity }` on the player.
  - `DriveIntent { throttle: f32, steer: f32, handbrake: bool }` (`Reflect, Default`), registered as required by
    `Character` via `app.register_required_components::<Character, DriveIntent>()` in `VehiclePlugin::build` (keeps
    `character/` untouched; verify in `bevy_ecs-0.19.1` that registering before any spawn is allowed).
  - `PreStepVelocity(pub Vec3)`; `register_required_components::<Character, PreStepVelocity>()`.
  - `VehicleLoad { awake: u32, rays: u32 }` resource (like `PerceptionLoad`), read by the perf gate.
- `ActionIntent` gains `pub vehicle_requested: bool` (doc: "F: enter/exit, cleared by the fixed tick") in
  `character/intent.rs:288` — the one edit in `character/`; `drop_queued_input` already clears it.
- Messages (`add_message`, `Reflect`): `VehicleEntered { vehicle, driver, attack: u32, first: bool }`,
  `VehicleHit { vehicle, driver: Option<Entity>, target, attack: u32, speed: f32 }`,
  `VehicleImpact { vehicle, point: Vec3, speed: f32 }` (vehicle vs non-character, presentation + QA).
- `pub fn vehicle_bundle(cfg: &VehicleConfig, dmg: &DamageConfig, transform: Transform) -> impl Bundle`:
  `Vehicle::default()`, `VehicleHealth { current: dmg.vehicle.max_health }`, `Name::new("Vehicle")`, transform,
  `RigidBody::Dynamic`, `Collider::cuboid(2·hx, 2·hy, 2·hz)`, `ColliderDensity(cfg.chassis_density())`,
  `CenterOfMass(cfg.center_of_mass)`, `CollisionLayers::new(Vehicle, [World, Character, Vehicle])`,
  `CollisionEventsEnabled`. No `Mass` (decision logged).
- `spawn_parked_cars` on `OnTransition{Loading→Playing}` `.run_if(resource_exists::<City>)`: one bundle per
  `ParkingSpot` at `(x, rest_height, y)` with `Quat::from_rotation_y(aim_yaw(heading as world (x,0,y)))`.
- `VehiclePlugin`: registers types/messages/required components; `VehicleSystems {Enter, Drive, Impact, Record, Seat}`:
  FixedUpdate — `Impact` in `HealthSystems::Damage` (ungated), `Enter` in `PlayingSystems` `.before(Drive)`,
  `Drive` ungated (suspension must run whenever physics steps; cars exist only in Playing/Wasted/Busted since fixed
  time stops in Paused); FixedPostUpdate — `Record` `.before(PhysicsSystems::First)`, `Seat`
  `.after(PhysicsSystems::Last)`; `OnEnter(Wasted)`, `OnEnter(Busted)` → `eject_all`; `NEW_CITY` → clear the three
  vehicle messages (like `clear_combat_messages`).
- Check: `cargo check -p gta_sim`.

### Step 5 — `vehicle/chassis.rs`: suspension, tyres, drive (≈ 250 lines, pure helpers unit-tested)
- System `drive_vehicles` (FixedUpdate, `VehicleSystems::Drive`): `SpatialQuery`, `Res<VehicleConfig>`,
  `Res<Time<Fixed>>`, `ResMut<VehicleLoad>`, `Query<(Entity, &mut Vehicle, &VehicleHealth, Forces, Has<Sleeping>)>`,
  `Query<&DriveIntent>` (driver), `Query<(), With<CityBlock>>`.
- Per car: `if sleeping { continue }` (no rays: perf gate). Intent = driver's `DriveIntent` or default; stalled
  (`health ≤ 0`) → throttle 0.
- Steering: target δ = steer·lerp(max_deg, at_max_speed_deg, |v_f|/max_speed clamped) → move `vehicle.steer`
  toward it at `rate_deg_per_s`. Front wheel forward = `R_y(−δ)·(0,0,−1)` in body frame (§2 example 2).
- Per wheel: mount = pos + rot·(sx·half_track, mount_height, sz·half_wheelbase); ray down `rot·NEG_Y`, max
  `travel + radius`, filter `[World, Vehicle]` excluding the car (wheels do not ride on people). Miss → not grounded.
  Hit: compression x = (travel + radius − distance).clamp(0, travel); spring speed = `velocity_at_point(mount)·up`;
  F_s = (k·x − c·speed).max(0) along body up, applied at the mount point. Normal load N = F_s.
- Tyre frame: `f_w` = wheel forward projected on the hit plane, `r_w` = f_w × n. At the contact point:
  v = `velocity_at_point(contact)`; lateral F_lat = −r_w·(v·r_w)·grip·(m/4)/dt (grip = front/rear, rear ×
  handbrake_rear while handbrake); longitudinal: throttle > 0 on rear wheels F = f_w·throttle·m·acceleration/2·
  ((max_speed − v_f)/top_speed_band).clamp(0,1); throttle < 0: brake (all wheels, m·brake/4, sign against v_long) while
  v_f > hold_speed, else reverse drive up to `reverse_speed`; no throttle and |v_f| < hold_speed (or handbrake on rear)
  → hold F = −f_w·(v·f_w)·(m/4)/dt; no throttle above hold_speed → coast m·coast/4. Every brake/coast/hold force is
  capped at (m/4)·|v_long|/dt (never reverses the wheel). Clamp |F_lat + F_long| ≤ μ·N (friction circle).
  Application point = contact + up·(roll_influence − 1)·(contact→CoM height along the BODY up axis) — the lever is
  scaled along body up, not world Y (Bullet bug, §1 Research).
- Forces via `forces.non_waking()` for undriven cars, waking `ForcesItem` for a driven one; record wheel states and
  `on_sidewalk` = any grounded wheel whose hit entity has `CityBlock`; count awake cars and rays into `VehicleLoad`.
- Pure fns with unit tests (in-file `#[cfg(test)]`): `wheel_forward(steer)` (§2 examples, 3 rows), `steer_limit(v)`,
  `drive_force(...)` rows (below/at/above max_speed, reverse, brake), `spring_force(x, speed)` (worked k, c),
  `lateral_force` sign row (car sliding +X → force −X).
- Check: unit tests + Step 11 gates G1-G7.

### Step 6 — `vehicle/seat.rs`: enter, exit, eject, seat sync (≈ 220 lines)
- `enter_exit` (FixedUpdate, PlayingSystems, `VehicleSystems::Enter`): `std::mem::take(&mut action.vehicle_requested)`
  always (latch never leaks). Player not `Dead`, not `Cuffed`, `HitReaction` not knocked down:
  - on foot: nearest car with `driver == None` whose door point (pos + rot·door, flat) is ≤ `enter_radius` from the
    player's flat position → `enter(...)`: player gets `Driving`, `RigidBodyDisabled`, `ColliderDisabled`, head hitbox
    child (`Children` + `HeadHitbox`) gets `ColliderDisabled`, `LinearVelocity` zero; car `driver = Some`, insert
    `SleepingDisabled`; `first = !taken`, `taken = true`; write `VehicleEntered { attack: serial.next_id(), .. }`.
  - driving: if car speed ≤ `exit_max_speed` → `exit(...)`: candidates door point, mirrored right door, roof
    (pos + up·(hy + float_height)); feet from a down ray (World) from candidate + 2 m; clear if
    `shape_intersections(capsule, centre, IDENTITY, [World, Vehicle, Character])` is empty; first clear candidate
    wins; none → stay in the car. Inverse of enter: remove `Driving`, `RigidBodyDisabled`, both `ColliderDisabled`;
    Position+Transform = candidate centre, velocity zero; car `driver = None`, remove `SleepingDisabled`.
- `eject_all` (OnEnter Wasted/Busted) and the seat-sync fallback use `exit` with a forced fallback to the door point
  (no clearance requirement).
- `sync_seats` (FixedPostUpdate after `PhysicsSystems::Last`): for `(player, Driving)`: car missing
  (`get` fails) → eject in place (dangling link fix); else Position = Transform.translation = car pos + rot·seat,
  Rotation/Transform.rotation = car rotation, `LinearVelocity` = car velocity. Also on the car side: a car whose
  `driver` points at an entity without a matching `Driving` gets `driver = None` (both link directions converge).
- `police/arrest.rs:32` query adds `Without<Driving>` (GDD §6.4); `combat/pickups.rs:57,132,197` player queries add
  `Without<Driving>` (disabled body in a car collects nothing).
- Check: G3, G8.

### Step 7 — `vehicle/impact.rs`: impacts → damage (≈ 200 lines)
- `record_pre_step` (FixedPostUpdate before `PhysicsSystems::First`): `PreStepVelocity = LinearVelocity` for all
  holders (vehicles + characters). Runs after Tnua's motor writes (FixedUpdate).
- `apply_impacts` (FixedUpdate, `HealthSystems::Damage`, ungated): `MessageReader<CollisionStart>`, `Collisions`,
  vehicles, `PreStepVelocity`, `Query<&mut Health, Without<Dead>>`, `Query<(&mut HitReaction, &mut TnuaController<CharacterScheme>)>`,
  `Res<MeleeConfig>`, `ResMut<AttackSerial>`, `Res<DamageConfig>`, writers `DamageDealt`, `VehicleHit`,
  `VehicleImpact`. For each event resolve bodies (`body1/2`), skip pairs without a vehicle; take the first manifold
  normal from `collisions.get(c1, c2)` (none → skip), orient n from the vehicle to the other body (flip if the vehicle
  is `collider2`). Closing = `(v_veh_pre − v_other_pre)·n` (static/no body → 0 velocity).
  - other = character with `Health`: striker rule — the car must strike: `v_veh_pre·n ≥ pedestrian.threshold_speed`,
    else no hit (a sprinting pedestrian into a parked car is harmless). Damage = round((closing − threshold)·per_mps)
    ≥ 1 → `Health::take(damage)`, `DamageDealt { shooter: driver.unwrap_or(vehicle), shot: attack, target, point:
    target position, damage, headshot: false, killed }`, `VehicleHit`; closing ≥ knockdown_speed →
    `HitReaction::escalate(true, melee_cfg)` + `knock_back(controller, n_flat·closing·shove_scale)`. The vehicle takes
    no damage from characters.
  - other = world / vehicle: `VehicleHealth.current -= (closing − vehicle.threshold)·per_mps` when positive (both
    vehicles for a car pair, clamp at 0); write `VehicleImpact`.
- Pure fns + unit rows: `pedestrian_damage(car_along_n, other_along_n, cfg)` rows: (10, 0) → 84; (6, 0) → 36;
  (2.5, 0) → 0; (0, −6.8) (pedestrian runs into parked car) → 0; (10, −2) (head-on walker) → (12−3)·12 = 108;
  `vehicle_damage(closing)` rows 4 → 0, 10 → 200, 28 → 920.
- Check: G4, G6.

### Step 8 — perception, civilians, wanted
- `perception/mod.rs`: `ThreatKind::Car`; `PerceptionConfig` + `npc/perception.ron`: `car_distance: 12.0`,
  `car_speed: 3.0` (validate > 0). In `perceive`, for each vehicle with `on_sidewalk && speed ≥ car_speed` within
  `car_distance` of the chest and `!sight_blocked` → `offer(Car, pos, distance, None)` (not a crime, like `Aimed`).
- `civilian/reaction.rs`: `Car` is not reportable (same arm as `Aimed | Hurt`); `worked_reaction_table` rows:
  `(Car, 5.0, (1,1,1), true, Flee)` (flee 1.0 ≥ cower 1.5·(1−5/15) = 1.0) and `(Car, 2.0, (1,1,1), true, Cower)`
  (cower 1.5·0.867 = 1.3 > 1.0).
- `wanted/crimes.rs`: `Crime::{RunOver, CarTheft}`; `classify(victim, source: HitSource {Gun, Melee, Vehicle}, killed)`
  (replaces `melee: bool`): Vehicle rows — civilian wound → RunOver, civilian kill → Kill, gang wound → None, gang kill
  → Kill, cop wound → WoundCop, cop kill → KillCop, other → None. `record_crimes` gains `MessageReader<VehicleHit>` (set
  of attack ids, like `melee`) and `MessageReader<VehicleEntered>`: `driver` is the player and `first` →
  `crimes.record(CarTheft, driver, Some(vehicle), attack, player pos, now, merge)` → `touched` (cop LOS witness; no
  civilian stimulus for theft in T14 — Open question Q4).
- `wanted/mod.rs` `HeatTable { run_over, car_theft }` + validate (> 0) + `of`; `assets/wanted/wanted.ron`:
  `run_over: 30, car_theft: 15`, comment updated ("car rows: GDD §6.4"). Update the `HEAT` const in crates tests.
- Unit tests: `classify_table` one row per (source × victim × killed) combination that matters (≥ 7 new rows).
- Check: G10, G11.

### Step 9 — headless gates `crates/gta_sim/tests/vehicle.rs` (+ `vehicle_city.rs`), helpers
- Helpers in the test file (not `common/`, it is at 614 lines): `spawn_car(app, feet_xz, yaw_deg) -> Entity` via
  `vehicle_bundle` at y = `rest_height`; assert its Position equals the Transform after one tick (`GATE BROKEN` else);
  `drive_in(app, car)` places the player at the door point and raises `vehicle_requested`, runs 1 tick, asserts
  `Driving`; `kick(app, car, speed)` sets `LinearVelocity` = forward·speed; `set_drive(app, |d| ..)`;
  `forward_of(car)`; fixture-clear check: before spawning, `shape_intersections(chassis box)` at the start pose must be
  empty (`GATE BROKEN: fixture overlaps the test area`).
- Gate table (each row its own `#[test]` or its own case with a row label; class in brackets):
  - **G1 wall** [correctness]: player settled, car at (0, h, −10) yaw 180° (forward +Z), driven, settle 32 ticks,
    kick 28 m/s, throttle 1, run 128 ticks sampling every tick: centre z ≤ 13.75 − 2.04 + 0.1 and y < 4.0 on every
    tick; liveness: speed ≥ 0.95·28 on some tick before contact, final speed ≤ 3. Flip-RED: car
    `CollisionLayers` filter without `World` → car passes z 14.25 → RED; restore GREEN. (Margin flip does not go red —
    probe P2, write it in the test doc comment.)
  - **G2 corner 45°** [correctness]: test-spawned static box 10×10×10 at (33, 5, −33) (x 28..38, z −38..−28, clear of
    all fixtures), car at (17.4, h, −17.4) yaw −45° (forward (0.707,0,−0.707)), door (16.41, −18.81) (§2 example 3),
    kick 28, throttle 1, 128 ticks: the centre is never inside the box AABB (`!(x > 28 && z < −28)`); liveness as G1.
    Same flip.
  - **G3 control ownership** [correctness], car at (−25, h, 0) yaw 0, door (−26.7, 0, −0.3), rows:
    (a) on foot, DriveIntent throttle 1 for 64 ticks → car moves < 0.05 m; (b) enter → `Driving{car}`,
    `car.driver == player`, player has `RigidBodyDisabled` and body + head hitbox `ColliderDisabled`; throttle 1 and
    MoveIntent axis Y for 64 ticks → car moved ≥ 1.5 m along −Z (derived: ½·4.6·1² = 2.3 m minus settle), |Δx| < 0.3,
    player Position within 0.01 of car pos + rot·seat; (c) throttle 0 + handbrake until speed < 3, request exit →
    link cleared both sides, player flat offset from the car dotted with car right < −1.0 (left side), colliders back;
    throttle 1 + MoveIntent Y 64 ticks → car moves < 0.05, player moves ≥ 1 m; (d) exit request at 10 m/s → still
    driving; (e) enter request at 3.0 m from the door → nothing. Flip-RED: make the chassis read the player's intent
    regardless of `driver` → row (a) RED.
  - **G4 pedestrian formula (AC)** [correctness], civilian (production `civilian_bundle`, `Idle{left: 100}`, test graph
    `[(−25,0,0), (−25,0,−20)]`) at (−25, 0, 0); car at (−25, h, 3.34) yaw 0 (front 1.0 m from the capsule), no driver
    (shooter = car), settle 32 ticks, kick v, throttle 0 (coast 1 m/s² over ≤ 8 ticks: −0.125 m/s). Rows each own case:
    v 10 → health loss ∈ [82, 84] (derived (9.875..10 − 3)·12 = 82.5..84, u32 rounding) + knocked down;
    v 6 → [34, 36] + knocked down; v 2.5 → 0 and not knocked down. Expected values computed in the test from the
    loaded `DamageConfig`. Flip-RED: use post-step `LinearVelocity` instead of `PreStepVelocity` → row 10 gives ≈ 0 → RED.
  - **G5 rest, no drift (AC)** [correctness], car at (−25, h, 0): rows (a) driven, no input: settle 64, then 640 ticks:
    horizontal drift < 0.02 m, |Δyaw| < 0.2°, centre y within 0.02 of `rest_height()` (1.1596) on every sampled tick
    (suspension equilibrium); (b) parked (no driver): same bounds and the car is `Sleeping` by tick 640 (liveness of
    sleep). Flip-RED candidates: damper sign flipped (−c) → oscillation → RED; hold force removed + a 0.02 m/s nudge →
    drift RED. Record which one was run.
  - **G6 car self-damage + stall (Orchestrator note 1)** [correctness], G1 geometry at chosen speeds, throttle 0 after
    the kick: rows 4 m/s → health unchanged; 10 m/s → loss ∈ [192, 200] (closing 9.8..10 after coast); `VehicleHealth`
    set to 150 then 10 m/s → 0 → throttle 1 for 64 ticks: speed stays < 0.3 and displacement < 0.1 (stalled);
    car hitting the G4 pedestrian at 10 → car health unchanged. Flip-RED: ignore `health ≤ 0` in drive → stall row RED.
  - **G7 no flip, steering sign** [correctness]: separate far floor `spawn_wall(center (500, −0.5, 500), 400×1×400)`;
    car at (500, h, 600) yaw 0, driven, kick 28, throttle 1, steer +1 for 192 ticks: every tick car up·Y ≥ 0.5
    (< 60° roll/pitch), speed ≥ 8 at the end (still driving, liveness). Directional rows on the test floor from rest,
    throttle 1 for 64 ticks: yaw 0 → Δz < −1, yaw 90 → Δx < −1, yaw 180 → Δz > 1; yaw 0 + steer +1 → Δx > 0.2 and yaw
    < −5°. Flip-RED: `roll_influence` 1.0 and CoM at the box centre → roll RED (if it stays green, record it and flip
    by removing the lateral μ·N clamp instead).
  - **G8 flow while driving (Orchestrator note 3)** [correctness]: (a) Wasted: driving, lethal `DebugDamage` → in
    `Wasted` the player has no `Driving`, car.driver None, no disabled components; after respawn (reuse the real-time
    advance of `respawn.rs`) the player is at the hospital and MoveIntent moves it ≥ 1 m in 64 ticks; (b) arrest:
    1 star, cop `Arrest` 1 m from the car, player passive for `arrest.seconds + 1` → still `Playing`; (c) forced
    `NextState(Busted)` while driving → ejected, respawn at station not driving; (d) the car is despawned while driving
    → next tick player on foot with colliders enabled; (e) mass: `ComputedMass` 1200 ± 1, yaw inertia 2240.6 ± 5 %.
  - **G9 new city while driving + leak (Orchestrator notes 3, 5)** in `vehicle_city.rs`: city seed 1, baseline b0 = all
    entities minus `IsResource` after the first update; enter the nearest parked car; pause; new city 2 → right after the
    transition frame the all-entity set equals b0 (car, player, Tnua sensors gone); `until_playing` → one player, no
    `Driving`, `count(Vehicle) == City.parking.len()`, every `Vehicle.driver == None`.
  - **G10 parked cars + perf** in `vehicle_city.rs` [correctness + perf]: seed 1, player settled, 128 ticks: vehicle
    count = parking spots; each car within 0.05 m (xz) of its spot and y within 0.03 of `rest_height`; all `Sleeping`;
    `VehicleLoad.rays == 0` on the last tick. Flip-RED: use waking forces for undriven cars → never sleep → RED.
  - **G11 perception and crimes**: (a) test-spawned `CityBlock` prism 20×0.15×10 at (−25, 0.075, −20); civilian on it
    8 m ahead; car driven onto the block at 6 m/s → civilian `Flee|Cower` within `slots` + 2 ticks; row car on the floor
    beside the block at the same distance → stays `Idle`; row car on the block at 1 m/s → `Idle`.
    (b) cop (police_support `spawn_unit`) with LOS 30 m: run over a civilian at 6 m/s → `WantedLevel.heat == 30`; same
    without a cop → 0; enter a parked car in view of the cop → 15, exit and re-enter → still 15.
- All gates run in `composed_app` (production composition). Stage summary lists each flip performed.

### Step 10 — client input `src/input/mod.rs`
- `add_input_context::<InVehicle>()`; the input entity gets `InVehicle` + `ContextActivity::<InVehicle>::INACTIVE` and
  `actions!(InVehicle[(Action::<Drive>::new(), Bindings::spawn(Cardinal::wasd_keys())), (Action::<Handbrake>, Space),
  (Action::<ExitVehicle>, ActionSettings{require_reset: true, ..}, KeyF), (Action::<DriveLook>, mouse_motion)])`;
  `OnFoot` gains `(Action::<EnterVehicle>, ActionSettings{require_reset: true, ..}, KeyF)`.
- `write_drive_intent` (Update, after `apply_mouse_look`): player `DriveIntent { throttle: drive.y, steer: drive.x,
  handbrake }`; `EnterVehicle`/`ExitVehicle` `START` → `ActionIntent.vehicle_requested = true`; not captured → zero.
- Replace `deactivate_input`/`activate_input` with `sync_contexts` (Update): `context_activity(state, driving) ->
  (on_foot, in_vehicle)` = Paused → (false, false), else (!driving, driving); insert a `ContextActivity` only when it
  differs from the current `Deref` value. Unit test: one row per `GameState` variant × driving.
- `camera::apply_mouse_look` reads `Action<Look> + Action<DriveLook>` (two `Single`s; the inactive one is zero).
- `release_held_actions` also zeroes `DriveIntent`.
- Check: `cargo test -p gta_like --bin gta_like` + owner run.

### Step 11 — client camera (`assets/camera/camera.ron`, `src/camera/{mod,config}.rs`)
- `camera.ron`: `car_distance: 6.5, car_pivot_height: 1.0 (m above the car body centre ≈ 2.2 m above the road),
  car_pitch_deg: -8.0, car_yaw_half_life: 0.2, car_look_return: 1.5 (s after the last mouse move)`; validate > 0
  except pitch within [pitch_min, pitch_max].
- `OrbitCamera.look_idle: f32` (reset by `apply_mouse_look` when the delta ≠ 0, grows by real dt).
- `follow_player`: if the player has `Driving`, the pivot is the car's interpolated Transform + up·car_pivot_height,
  no shoulder offset, distance `car_distance`; when `look_idle ≥ car_look_return`, yaw and pitch approach
  `aim_yaw(car forward)` / `car_pitch_deg` with half-life `car_yaw_half_life` via `approach_angle` (shortest arc, unit
  rows: 170 → −170 = +20; 10 → −10 = −20; 0 → 180 = either sign but |Δ| ≤ 180). Collision cast unchanged (World only:
  the car never pushes its own camera). AimIntent still written.
- Observer `On<Add, Vehicle>` → `TransformInterpolation` (like the player).

### Step 12 — client visuals `src/visuals/vehicle.rs` + `assets/world/render.ron`
- `render.ron`: `vehicle: (model: "third_party/car-kit/sedan.glb", scale: 1.6, offset: (0.0, -1.16, -0.04),
  wheels: ["wheel-front-left", "wheel-front-right", "wheel-back-left", "wheel-back-right"])` (offset = −(0.24 + 0.92),
  z from the body node −0.025·1.6 flipped by the 180° model yaw); `RenderConfig::vehicle_asset_paths()`;
  `main.rs::preflight` checks the model is in the manifest (like the character model).
- Observer `On<Add, Vehicle>`: `Visibility::default()` + child `WorldAssetRoot(scene)` with the offset, `R_y(π)`,
  scale; `WorldInstanceReady` observer (`bevy::world_serialization::WorldInstanceReady`, lesson TASK-005) records the
  4 wheel node entities by `Name`. Update: front wheels yaw = −steer (model frame), spin θ += v_long·dt/r (§2 example
  5), hub y offset = (compression − x_eq)/scale.
- Player model hidden while `Driving` (`Visibility::Hidden` on the player, back to `Inherited`).
- Stall smoke (GDD §5.1 "дым"): `juice.ron smoke: (interval: 0.25, seconds: 1.2, size: 0.5, rise: 1.2, color:
  (0.25,0.25,0.25,0.6), hood: (0.0, 0.6, -1.6))`; a vfx system spawns a pooled unlit sphere puff at the hood of every
  vehicle with `VehicleHealth ≤ 0` every `interval`, rising and fading over `seconds` (real time). Owner-judged.
- Check: owner run only (presentation, lesson TASK-009: no gate over look).

### Step 13 — client audio (`assets/audio/mix.ron`, `src/audio/`)
- `synth.rs`: `Synth::Engine(EngineSynth { base_hz, harmonics, noise })`, endless decoder (sum of `harmonics` sines
  k·phase with 1/k amplitude + `noise` of the existing xorshift), `is_endless` true.
- `cues.rs`: `SoundClass::Engine` (COUNT 9; excluded from the one-shot budget like `Siren`); `SoundBank.engine`;
  `impacts.vehicle` pool (the 5 `impactMetal_heavy` files) played spatially on `VehicleImpact` from the player's car or
  within audibility, class `Impact`.
- New `engine.rs` (Update, Playing|Wasted|Busted): exactly one `EngineEmitter` child on the car the player drives
  (`PlaybackSettings::ONCE`, spatial, `spawn_sound`), despawned when not driving; every frame
  `sink.set_speed(pitch)` and `set_volume(gain · global)` from `engine_voice(speed_ratio, throttle, cfg)`:
  rpm = max(|v_f|/max_speed, |throttle|·rev_share).clamp(0,1); pitch = lerp(idle_pitch, max_pitch, rpm);
  gain = lerp(idle_volume, max_volume, max(rpm, |throttle|)). Unit rows: idle (0,0) → (idle_pitch, idle_volume);
  (0, 1) → rpm = rev_share; (1, 0) → (max_pitch, max_volume); reverse (−0.2, −1).
- `mix.ron`: `engine: (base_hz: 45.0, harmonics: 6, noise: 0.15, idle_pitch: 0.8, max_pitch: 2.4, rev_share: 0.35,
  idle_volume: 0.25, max_volume: 0.6, ref_distance: 8.0)`; `sync_loop_pause` includes `Engine`; `MixConfig::validate`
  + `check_sounds` cover the new pool.
- Existing audio gates (`gate.rs`, `event_gate.rs`) updated where they enumerate classes/endless synths.

### Step 14 — HUD, minimap, juice
- `hud/mod.rs`: `HudBar::Vehicle` third bar (`strings.ron hud.vehicle_color`), `Display::None` unless the player
  drives; value = car health / `max_health`. `weapon::update_crosshair` hides the crosshair while driving.
- `minimap/markers.rs`: `MarkerKind::Vehicle`, a dot for every `Vehicle` except the one the player drives
  (`strings.ron minimap.vehicle_color`, validate unit); hidden beyond the rim by the existing code.
- `juice/shake.rs::add_trauma`: row `VehicleImpact` of the player's car → `speed·crash_trauma_per_mps` when
  `speed ≥ crash_min_speed` (`juice.ron shake: crash_trauma_per_mps: 0.02, crash_min_speed: 5.0`).

### Step 15 — assets manifest + fetch
- `assets/third_party/manifest.ron`: pack `car-kit` (version "3.1", page `https://kenney.nl/assets/car-kit`, url and
  hashes of §1, files `sedan.glb`, `Textures/colormap.png`, `License.txt`); `impact-sounds` gains the 5 metal files.
- `crates/gta_sim/tests/asset_manifest.rs`: names set + `("car-kit", 3)`, `("impact-sounds", 31)` rows.
- Install offline: `python tools/fetch_assets.py --cache maw/tasks/in_progress/TASK-015/scratch/carkit --cache
  maw/tasks/in_progress/TASK-015/scratch/impact` then `--check`.

### Step 16 — runtime QA `tools/qa/scenarios/t14.py` (AC)
Style of `t12.py`, helpers from `t5`/`t6`/`t8`. `--seed 1`: wait `Playing` + chunks; read vehicles
`rows(game, ["Vehicle","Position","Rotation"])` (≥ 1 else FAIL); nearest car → door point from `sedan.ron` (regex)
and the car quaternion → mutate player `Position` + `Transform`; `send_keys(["KeyF"], 100)`; poll `Driving` on the
player ≤ 2 s (FAIL); `send_keys(["KeyW"], 3000)` with screenshots every 0.5 s (≥ 0.15 s spacing, lesson TASK-007);
car speed ≥ 5 m/s and displacement ≥ 5 m (derived: 3 s at 4.6 m/s² ≤ 13.8 m/s); `SoundStats` Engine spawned ≥ 1;
minimap markers of kind `Vehicle` > 0. Wall: mutate the car to (0, 1.16, 670) facing +Z (edge wall inner face z ≈
700, empty margin beyond the 600 m city) with zero velocity, `KeyW` 4000 ms: `VehicleHealth` dropped, car z <
700 − 2.04 + 0.3, speed < 2 after; screenshot; `KeyF` → `Driving` gone ≤ 2 s, player flat distance to car ≤ 3 m,
`GameState` Playing, player `Health` readable; screenshot; shutdown. Hard pass/fail on components only.
Owner checklist (for QA_REPORT.md): сел (F у двери), поехал (W/S/A/D, Space ручник), handling устраивает (значения в
`sedan.ron`: acceleration, max_speed, steer, grip, roll_influence), камера машины (`camera.ron` car_*), гул двигателя по
скорости, удар в стену (звук, тряска, полоса HUD), дым при нуле, сбить пешехода, выход, припаркованные машины на
проспектах и метки на мини-карте.

### Step 17 — close-out checks
`cargo build`, `cargo clippy -- -D warnings`, `cargo test -p gta_sim`, `cargo test -p citygen`,
`cargo test -p gta_like --bin gta_like`, `python tools/qa/tree_check.py`, `cargo tree -p gta_sim -e normal -i
bevy_render` empty. Files stay < 750 lines (vehicle split into mod/config/chassis/seat/impact).

---

## 4. Risk areas
- **R1 Handling instability** (explosive springs, jitter on curbs, spinning on the lateral cancel): the per-tick lateral
  cancel with grip 1 is stiff; if it oscillates, lower `grip` (data) before touching code; G5/G7 catch divergence.
- **R2 Inertia**: if `ColliderDensity` on a box does not give ComputedMass 1200 exactly (child colliders, `CenterOfMass`
  override interplay), G8e fails loudly — fix mass setup, do not relax the gate.
- **R3 Sleep never happens** (suspension micro-jitter above 0.15 m/s) → G10 RED and 100+ cars cast rays; remedy: the
  hold force, not a higher threshold.
- **R4 Seat sync vs avian transform sync**: writing Position+Transform after `PhysicsSystems::Last` on a
  `RigidBodyDisabled` body; if avian overwrites it next step, G3b fails — then also write in FixedUpdate before physics.
- **R5 Impact timing** relies on `CollisionStart` of step N being read at FixedUpdate N+1 with `PreStepVelocity` of N
  still intact; a second fixed tick in the same frame keeps order (records happen after impacts); the probe settles the
  within-step timing.
- **R6 Parked cars in the avenue curb lane** block the outer lane for T15 traffic (Q1).
- **R7 Player invulnerable to bullets in a car** (colliders disabled, bullets stop on the car) — Q2.
- **R8 Golden rebless** could hide an unintended citygen change: mitigated by step 1(a).
- **R9 Existing latent bug (not fixed here, surgical)**: `bevy-tnua-avian3d` `apply_motors_system` returns on the first
  `TnuaToggle::Disabled` entity (`vendor/.../lib.rs:429-431`); corpses already use it (`population/mod.rs:239`), so
  characters iterated after a corpse archetype can lose motor forces for that tick. Reported in PCTX_PROPOSALS; the plan
  avoids `TnuaToggle` for the driver.
- **R10 Input**: two `Look`-like actions and the F key in two contexts; `require_reset` prevents instant re-entry.
  `Single<&Action<Look>>` must stay unique (hence `DriveLook`).
- **R11 Headless GLB**: car visuals are not gated (presentation); `WorldInstanceReady` cannot be triggered by hand.
- **R12 Frame budget**: one driven car = 4 rays + a handful of forces; perception adds a loop over moving sidewalk cars
  (0-1 typical). Engine voice is one emitter.

## 5. Open questions (для владельца; дефолт указан, работа не блокируется)
- **Q1. Где стоят припаркованные машины?** (а) у бордюра во внешней полосе проспектов — дефолт: улицы с одной полосой
  не перекрыты, трафик T15 объезжает по внутренней полосе; (б) также на улицах — больше машин, но T15 придётся учить
  объезду или машины трафика будут стоять за припаркованными; (в) наполовину на тротуаре — пешеходы упираются в машины,
  на бордюре машина стоит криво. Рекомендация (а).
- **Q2. Пули и игрок в машине.** (а) машина — укрытие: пули останавливаются на кузове, игрок урона не получает —
  дефолт T14; (б) пуля, попавшая в машину с игроком, ранит игрока (как стрельба в окно) — полиция с T11 сможет убить
  водителя; (в) пули бьют по здоровью машины. Рекомендация (б) в T15 вместе с погоней; в T14 — (а).
- **Q3. Метки машин на мини-карте.** (а) все машины в радиусе мини-карты — дефолт; (б) только последняя машина
  игрока; (в) никаких, кроме полицейских (T15). Рекомендация (а), при шуме — (б).
- **Q4. Кто свидетель угона.** (а) только коп в прямой видимости — дефолт T14; (б) ещё и прохожие (новый стимул и
  звонок) — больше работы восприятия, логичнее вместе с угоном из трафика в T15.
- **Q5. Выход на ходу.** (а) только при скорости ≤ 3 м/с (`exit_max_speed`) — дефолт; (б) выпрыгнуть на любой
  скорости с отбросом (как в GTA) — нужен knockdown игрока при выходе.

children: 0 launched / 0 reported.
