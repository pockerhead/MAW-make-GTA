# IMPL_SUMMARY — TASK-015 (GDD T14, drivable car)

Verdict: IMPLEMENTED. Pre-flight passed: every file, function and API the plan names exists with the shape the
plan assumes. Two plan claims turned out wrong once I ran them (bevy query order, the roll-influence sign). Both
were mechanical and had one fix each, so I adapted them and did not block. See Deviations.

Branch `feature/t14-car`, nothing committed (the orchestrator commits).

## 1. What was implemented (files, lines)

### Step 0: vendored Tnua motor fix (done first, flipped RED on its own before any car code)
- `vendor/bevy-tnua-avian3d-0.12.1/src/lib.rs` (+1 −1): `return` → `continue` in `apply_motors_system`.
- `docs/decisions/ADR-001-vendored-tnua-avian3d.md` (+5 −2): second change + update procedure (+ `--test tnua_motor`).
- New gate `crates/gta_sim/tests/tnua_motor.rs` (90): a walker whose table comes after a corpse moves ≥ 3.5 m in
  64 ticks; control test without a corpse.

### Sim (`crates/citygen`, `crates/gta_sim`)
- citygen: `params.rs` (+44, `ParkingParams` + validate rows), `layout.rs` (+8, `ParkingSpot`), `rng.rs` (+1,
  `PARKING = 16`), new `parking.rs` (35), `lib.rs` (+3), `hash.rs` (+7 −1, schema 2 → 3), `golden_hashes.txt`
  (reblessed), `tests/properties.rs` (+71, parking property); `assets/world/city.ron` (+3).
- New domain `crates/gta_sim/src/vehicle/`: `mod.rs` (250: components, messages, bundle, parked-car spawn, plugin,
  sets), `config.rs` (307: `VehicleConfig`/`DamageConfig` + validate + derived k, c, rest height, density),
  `chassis.rs` (276: 4 raycast wheels, spring/damper, tyre lateral cancel, drive/brake/reverse/hold/coast/handbrake,
  μN clamp, roll lever, non-waking forces for undriven cars; unit rows), `seat.rs` (269: enter/exit/eject/seat
  sync), `impact.rs` (192: pre-step velocity, crash → `DamageDealt`/`VehicleHit`/`VehicleImpact`, bullet damage;
  unit rows).
- New data `assets/vehicle/sedan.ron` (46), `assets/vehicle/damage.ron` (9).
- `lib.rs` (+37 −4): loads + validates both configs, compose-time `parking.curb_offset` check against the car width,
  `VehiclePlugin` added as a separate call after the 14-plugin tuple.
- `layers.rs` (+1 `Vehicle`); `combat/hitscan.rs` (+33 −2: filter + `BulletHitVehicle`, no RNG draw, 15 params);
  `combat/mod.rs` (+6 −2); `character/intent.rs` (+2 `vehicle_requested`); `police/arrest.rs` and
  `combat/pickups.rs` (`Without<Driving>`); `perception/mod.rs` (+21 `ThreatKind::Car`); `civilian/reaction.rs`
  (+3 −1, 2 table rows); `wanted/crimes.rs` (+69 −20: `HitSource`, `RunOver`, `CarTheft`, 7 new classify rows,
  `HEAT` const); `wanted/mod.rs` (+8); `assets/npc/perception.ron`, `assets/wanted/wanted.ron`.
- Gates: `tests/vehicle.rs` (240: G1 G2 G5 G6 G7), `tests/vehicle_seat.rs` (351: G3 G8), `tests/vehicle_hits.rs`
  (477: G4 G11 G12), `tests/vehicle_city.rs` (146: G9 G10), `tests/vehicle_support/mod.rs` (132),
  `tests/config_vehicle.rs` (244), `tests/asset_manifest.rs` (+10 −4).

### Client (`src/`)
- Input `input/mod.rs` (+129 −15): `InVehicle` context (Drive WASD, DriveLook, Handbrake Space, ExitVehicle F with
  require_reset), `EnterVehicle` F on `OnFoot`, `write_drive_intent`, `sync_contexts` (replaces deactivate/activate,
  table unit test), `release_held_actions` also zeroes `DriveIntent`.
- Camera `camera/mod.rs` (+99 −7), `camera/config.rs` (+19), `assets/camera/camera.ron` (+7): car pivot and distance,
  no shoulder, swing back behind the car after `car_look_return`, shortest-arc approach (unit rows),
  `TransformInterpolation` on cars.
- Visuals: new `visuals/vehicle.rs` (261: Kenney sedan under every `Vehicle`, wheel steer/spin/suspension, driver
  model hidden, stall smoke pool), `visuals/config.rs` (+32), `visuals/mod.rs` (+2), `assets/world/render.ron` (+5),
  `main.rs` preflight + `city_gate.rs` check the car model is in the manifest.
- Audio: new `audio/engine.rs` (146: engine emitter on the driven car, `engine_voice` unit rows), `synth.rs`
  (+46 `Synth::Engine`), `cues.rs` (+25 −4: `SoundClass::Engine` COUNT 9, metal impact pool, own crash always heard),
  `loops.rs` (pause includes Engine), `config.rs` (+47), `gate.rs` (+1 endless row), `assets/audio/mix.ron` (+11).
- HUD/minimap/juice: `hud/mod.rs` (+31 car bar, `Display::None` on foot), `hud/weapon.rs` (no crosshair while
  driving), `minimap/markers.rs` (+15 `MarkerKind::Vehicle`), `minimap/config.rs`, `menu/config.rs`,
  `assets/ui/strings.ron`; `juice/shake.rs` (+36 crash trauma, unit rows), `juice/config.rs` (+44 smoke + shake rows),
  `assets/juice/juice.ron` (+6), `juice/feedback_gate.rs` (+1).
- Assets: `assets/third_party/manifest.ron` (+19: `car-kit` pack, 5 `impactMetal_heavy`); installed offline from the
  planner's cache (`fetch_assets.py --cache …carkit --cache …impact`, then `--check` OK). The GLB references
  `Textures/colormap.png` (checked in the GLB JSON).
- Runtime QA: new `tools/qa/scenarios/t14.py` (250).

## 2. Deviations from plan

1. **Tnua gate precondition** (`tests/tnua_motor.rs`). The plan says query iteration order equals archetype
   creation order, and it builds the precondition with a fresh `world.query`. That is wrong in bevy_ecs 0.19.1:
   with required components, `update_archetypes` matches through the component index HashMap
   (`query/state.rs:575-600`). After `Character` got two required components, the fresh query put the walker first
   (GATE BROKEN). The fix: a `QueryState` created before the corpse. It matches tables incrementally, the same way
   the motor system does. The flip was re-run after the change: `return` → RED, `continue` → GREEN.
2. **Roll lever sign** (`chassis.rs`). The plan text says `contact + up·(roll_influence − 1)·h`. I implemented
   `contact + up·(1 − roll_influence)·h`, which gives lever = roll_influence·h = 0.198 m, the plan's own worked
   number (§3). The text's sign would put the force 1.7·h below the CoM.
3. **Exit feet ray** uses `[World, Vehicle]` instead of World only. Otherwise the roof candidate lands under the car.
4. **Test fixtures changed because the plan's versions could not fail** (the flips stayed GREEN or hit fixtures):
   - G4 checks the first hit's damage against the formula, not the total health loss. A car that keeps rolling
     hits the knocked-down body again: 6 m/s gave 36 + 12.
   - G5 adds a 0.5 m/s drop and a 0.02 m/s creep. Otherwise the damper and hold flips could not go RED from an
     exact equilibrium.
   - G6 stall row reverses away from the wall, because pressing into the wall would not move the car anyway.
   - G8b puts the cop 1.25 m from the seat. The plan's "1 m from the car" is 3.5 m from the seat and out of arrest
     range, so the flip stayed GREEN.
   - G11 theft row re-enters again after `incident_memory_seconds`. The incident dedupe alone kept the `first` flip
     GREEN.
   - G11 fixtures: the block moved to x 20..40 and is 20×0.15×20. The witness is the bare `spawn_cop` sight fixture
     from the existing wanted gates; `spawn_unit` at 0 stars leaves.
   - G11 run-over also asserts the crime kind, because `wound_civilian` and `run_over` are both 30 heat.
5. The G7 flip "roll 1.0" alone stays GREEN. "roll 1.0 + CoM at centre" goes RED, and so does removing the μN clamp.
   Both are recorded.
6. Gate files split into `vehicle{,_seat,_hits,_city}.rs` + `vehicle_support/` to stay well under 750 lines.
7. `render.ron` `wheels` is a RON tuple `(…)`, not `[…]`, because it deserializes as `[String; 4]`.

Everything else follows PLAN_FINAL. Log entries: `log.jsonl` (6 decisions). PCTX proposals added (query order,
rolling re-hit).

## 3. Test results

- `cargo build -j 4`: OK. `cargo clippy -j 4 -- -D warnings` and `cargo clippy --workspace --all-targets -j 4 -- -D warnings`:
  clean.
- `cargo test -p gta_sim -j 4`: 357 passed, 0 failed (run twice after the last change, plus once more after the flips
  were restored). Right after Step 0, the full suite was green with no expected number changed.
- `cargo test -p citygen -j 4`: 32 passed. Golden procedure: (a) parking without the hash lines → golden GREEN;
  (b) hash lines + schema 3 → RED (seed 1 `0xd2158922f1e5cd6f` → `0x31849ea45f8ec142`); (c) blessed seed 1
  `0x31849ea45f8ec142`, seed 2 `0x0672ef45a54e0587` → `0x67a43b6eb7a7bd76`, seed 42 `0xe62c12ebedcb8268` →
  `0xaa9407c7d1b9df77`; (d) the minimap raster golden stays green untouched. Parking spots per seed: 105..161 over
  seeds 0..31; seeds 1/2/42 have 143/128/138 (target 60..200, `chance` 0.25 unchanged). Property flip (spots on the
  left side) → RED.
- `cargo test -p gta_like --bin gta_like -j 4`: 76 passed, run 3 times, 3/3 green.
- `python tools/qa/tree_check.py`: passed. `cargo tree -p gta_sim -e normal -i bevy_render`: empty.
- **Flip-RED** (`scratch/flips/flip_red.py`, results in `scratch/flips/flip_results.txt`, re-run after rustfmt):
  G1/G2 filter without World → RED (passes the obstacle at tick 34); G3a intent ignores driver → RED (2.26 m);
  G3f no ActionIntent reset → RED (6 SMG shots); G4 post-step velocity → RED (first hit 0); G5 damper sign → RED;
  G5 hold removed → RED (0.2 m drift); G6 / G12b stall ignored → RED; G7 roll 1.0 + CoM centre → RED, μN clamp
  removed → RED (roll 1.0 alone GREEN, recorded); G8a/G8c no eject → RED; G8b arrest in a car → RED (busted);
  G8d vanished-car fallback removed → RED; G9 car not CityScoped → RED; G10 waking forces → RED (143/143 awake);
  G11 no car threat / sidewalk ignored / speed ignored / run-over as a shot / every entry a theft → all RED;
  G12a bullets ignore cars → RED; G12c head hitbox left enabled → RED (headshots on the driver). Config gates: one
  strictly failing sabotage per rule, each with its own keyword. Tnua gate: `return` → RED, `continue` → GREEN.
- Measured: rest height 1.15956 m (derived 1.1596); G1 car stops at z 11.71 (face 13.75 − 2.04); mass 1200; yaw
  inertia matches 2240.6 within 5 %; first pedestrian hit at 10 m/s is 83, at 6 m/s is 34 (probe output).
- **Runtime QA** `python tools/qa/scenarios/t14.py --out maw/tasks/in_progress/TASK-015/scratch/qa_t14` (release, dev
  features, QA settings id): **PASS**. 143 parked cars; entered by F; W 3 s → top speed 13.2 m/s, 24.1 m travelled;
  engine sound spawned 1; 142 car dots on the minimap; wall crash from 30 m: car health 1000 → 574, stopped at z
  697.88 (limit 698.26), speed 0.02; F → on foot 1.73 m from the car, `Playing`. frame_report: 144 Hz Fifo,
  no-vsync frame cost ≤ 3.2 ms. No errors in the log. Screenshots are in `scratch/qa_t14/`. The game was shut down.

## 4. How to verify manually (owner checklist)

`cargo run --features fast -- --seed 1`, then walk to a car parked on an avenue curb:
- [ ] сел: F у левой двери (≤ 2.5 м) — камера переходит за машину, модель игрока скрыта, прицел пропал, полоса машины
      (оранжевая) появилась в HUD.
- [ ] поехал: W/S/A/D, S на ходу тормозит, стоя — задний ход; Space — ручник (занос задней оси).
- [ ] handling устраивает — значения в `assets/vehicle/sedan.ron` (`acceleration`, `max_speed`, `steer.*`, `grip.*`,
      `roll_influence`, `suspension.*`).
- [ ] камера машины: `assets/camera/camera.ron` `car_*` (дистанция 6.5, возврат за машину через 1.5 с без мыши).
- [ ] гул двигателя растёт со скоростью и газом (`assets/audio/mix.ron` `engine`).
- [ ] удар в стену: металлический звук, тряска, полоса HUD падает; на нуле — дым из-под капота, машина не едет.
- [ ] сбить пешехода: урон, пешеход падает; при продолжении движения машина бьёт лежащего повторно (решить, нравится ли).
- [ ] выход F только на скорости ≤ 3 м/с; игрок появляется у левой двери.
- [ ] припаркованные машины на крайних полосах проспектов, точки машин на мини-карте.
- [ ] полиция стреляет по машине → полоса HUD падает, на нуле машина глохнет; водителя сверху не задеть.

Notes for the reviewer / owner:
- Re-hit behaviour: a rolling car keeps damaging a knocked-down pedestrian, about 12 hp per re-hit at 4 m/s closing.
  This is feel/design, not gated. A per-pair cooldown would be a new tuning value.
- The impact of the city curb on handling (0.15 m step, wheels ride up) is owner-judged.
