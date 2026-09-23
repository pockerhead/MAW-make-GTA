# IMPL_SUMMARY — TASK-007 (GDD T6 shooting + floating damage numbers)

Status: DONE. Pre-flight passed (no PLAN_BLOCKED). All gates green, every new gate flip-RED'd, runtime
scenario `t6.py` and regression `t5.py` pass on the release `dev` build.

## Pre-flight
Checked against code and pinned sources: avian3d 0.7.0 `SpatialQuery::cast_ray_predicate`
(`spatial_query/system_param.rs:176`), `ColliderOf { body }`, `SpatialQueryFilter::from_mask` /
`with_excluded_entities`, avian_derive 0.2.3 default layer = bit 0, `CollisionLayers::DEFAULT`; vendored Tnua
sensor guards (`vendor/bevy-tnua-avian3d-0.12.1/src/lib.rs:248-307`); bevy_ecs `Messages::clear`,
`get_cursor_current`; bevy_camera `world_to_viewport` (camera.rs:579), `CameraUpdateSystems`; bevy_ui
`UiTransform`, `Val2::percent`, `UiSystems` chain (lib.rs:147); BEI 0.26 `Binding::mouse_wheel` +
`SwizzleAxis::YXZ`; bevy_audio `Decodable` / `add_audio_source` / `PlaybackSettings::DESPAWN`;
`bevy_light::NotShadowCaster`; bevy_brp_extras 0.22.6 `send_mouse_button { button, duration_ms }`;
rand_chacha 0.10 imports as in `citygen/src/rng.rs`. All existed with the shapes the plan assumed.

## 1. What was implemented

Data (A1-A8): `assets/combat/weapons.ron` (new, 17), `assets/combat/aim.ron` (new, 1),
`assets/juice/juice.ron` (new, 19), `assets/audio/mix.ron` (new, 7); extended `character/locomotion.ron` (+3),
`camera/camera.ron` (+5), `ui/strings.ron` (+5/-1), `world/render.ron` (+2).

Sim `crates/gta_sim` (numstat +/-):
- `src/layers.rs` (new, 10): `GameLayer { World (default), Character, Hitbox }`.
- `src/combat/weapons.rs` (new, 471; 97 of them unit tests): `Weapon`, `FireMode`, `WeaponsConfig` + strict
  validation, `GunSlot`, `Loadout`, `cycle_weapon`, `falloff_factor`, `acquire`, `roll_damage`, `tick_loadouts`.
- `src/combat/hitscan.rs` (new, 287): `AimConfig`, messages `ShotFired` / `BulletTrace` / `DamageDealt`,
  `CombatRng` (ChaCha8, seed 0), `aim_yaw`, `muzzle`, `cone_sample`, two-ray `fire_weapons`.
- `src/combat/range.rs` (new, 96): `Dummy`, `dummy_bundle`, one-shot `spawn_range` (city only), `dummy_life`.
- `src/combat/pickups.rs` (+48), `src/combat/mod.rs` (+40/-6): `WeaponPickup`, `collect_weapon_pickups`,
  plugin wiring (`HealthSystems::Damage/Pickup/Death`, `PlayingSystems`).
- `src/character/{mod,intent,locomotion,health}.rs` (+35/-5, +32/-1, +43/-1, +17): `HeadHitbox` + `head_hitbox`
  child sensor, `CollisionLayers(Character, ALL)`, `AimIntent`, `ActionIntent`, `WeaponRequest`,
  `Gait: Deserialize + Ord`, head/aim-cap config + validation, aiming facing + gait cap in `drive_characters`,
  `Health::take`.
- `src/flow/{mod,wasted}.rs` (+4/-1, +6/-1): B1 fix `drop_queued_damage` on `OnExit(Wasted)`.
- `src/player/mod.rs` (+4/-3): `Loadout::default()` on spawn, debug damage through `Health::take`.
- `src/lib.rs` (+14/-1): load/validate `weapons.ron`, `aim.ron`. `Cargo.toml` (+1): `rand_chacha =0.10.0`;
  `Cargo.lock` = `scratch/Cargo.lock.with_rand_chacha` (+1 line).

Sim gates: `tests/shooting.rs` (new, 612), `tests/common/mod.rs` (+144/-3 helpers incl. `Shots` message
collector), `tests/respawn.rs` (+68/-9), `tests/config.rs` (+84).

Client `src/` (numstat or new-file lines):
- `camera/{config,mod}.rs` (+31, +45/-12): aim blend (shoulder/distance/FOV/sensitivity), world-only camera
  casts, writes `AimIntent` from the camera, visual recoil.
- `input/mod.rs` (+119/-3): Fire/Aim/Reload/Slot1-4/wheel actions, `write_action_intent` (release-before-fire
  after cursor recapture), clear held actions on `OnEnter(Wasted)`.
- `juice/{mod,config,damage_numbers,damage_numbers_gate}.rs` (new, 43/148/169/178): `CameraRecoil`,
  `JuiceConfig`, floating damage numbers (UI label per `DamageDealt`), headless gate E10.
- `vfx/mod.rs` (new, 117): muzzle flash (sphere + point light) and tracers, `NotShadowCaster`, real-time lifetime.
- `audio/mod.rs` (new, 179): `MixConfig`, noise-burst `Decodable` gunshot per weapon.
- `hud/weapon.rs` (new, 188), `hud/mod.rs` (+12/-3): ammo counter, crosshair (dot + spread arms while aiming),
  hit/kill marker.
- `visuals/weapons.rs` (new, 136), `visuals/{config,mod}.rs` (+36, +13/-2): gun/ammo pickup boxes, held gun box.
- `menu/config.rs` (+35/-1): `damage_crit` + HUD fields + validation. `main.rs` (+41/-1): load/validate
  juice/mix/camera configs in `preflight`, add `JuicePlugin`, `VfxPlugin`, `ShotAudioPlugin`.

Runtime QA: `tools/qa/brp.py` (+4, `send_mouse_button`), `tools/qa/scenarios/t6.py` (new).

## 2. Deviations from plan
- D14 `head_must_stick_out_of_capsule` sabotages `head_radius: 0.15`, not 0.2: with `head_height 1.6` a 0.2 sphere
  exactly touches the capsule top (1.6 + 0.2 = 1.8); f32 gives 1.8000001 > 1.8 and validation passes. The rule is
  as planned; the fixture moved strictly inside (`tests/config.rs`). Logged.
- D12 waits until the bloom is back to 0 between shots (`recovery_delay + per_shot / recovery_rate`, from data)
  instead of `run_ticks(20)`: with 20 ticks the pistol bloom stays 1° and shot 2 missed (trace hit the floor).
- D2/D10 message reading uses a `Shots` collector with `get_cursor_current()` cursors read after every tick
  (the plan's "fresh `get_cursor()`" would re-read the previous tick's buffer).
- D7: the double-removal flip-RED lives inside the gate as a positive control (a plain sphere in the same place
  must lift the body > 0.2 m); the one-factor probes ran as perturbations (see flip table).
- D10 dead-head check uses a dedicated geometry (muzzle and head at the same x, 1.5 m apart, shot line 0.24 m from
  the head centre and 0.33 m from the capsule top): the plan's "fire at head height" from 10 m hits the corpse
  capsule top first, so removing `dead.contains` would not flip it.
- Added gate `city_range_is_clear_on_seed_1` (Step 0 lane check done headless instead of BRP): dummies stand at
  park level and the line from the park centre to each dummy is unobstructed. No range origin shift was needed.
- `pose(age, drift, cfg)` takes `drift` as a parameter (the plan's signature had nowhere to read it from).
- Muzzle flash is a small unlit sphere, not a quad (a quad needs billboarding; a sphere reads from any side).
- Flash and tracer carry `NotShadowCaster` (first capture showed the tracer only as a shadow line).
- `combat/weapons.rs` is 471 lines (plan: < 400); 97 are unit tests, under the project's 750 warning.
- `CameraConfig` had no `validate`; added one for the five new aim fields, called from `preflight`.

## 3. Test results
- `cargo test -p gta_sim`: all green — lib 11, anim_state 4, asset_manifest 3, city 6 (+1 ignored), config 14,
  health 6, jump 3, movement 4, respawn 4, shooting 14, terrain 2.
- `cargo test -p gta_like --bin gta_like`: 20 passed (incl. E10: `label_matches_message`,
  `numbers_despawn_after_lifetime`, `non_player_hits_spawn_nothing`, `pose_worked_examples`;
  `character_model_spawns_under_player` still green).
- `cargo test -p citygen`: green (not touched).
- `cargo build`: ok. `cargo clippy -- -D warnings`: clean; also clean `-p gta_sim --tests`,
  `-p gta_like --tests --features dev`.
- `cargo tree -p gta_sim -e features -i bevy_render` and `-e normal`: empty. `python tools/qa/tree_check.py`: passed.

Flip-RED (scripts `scratch/flip_red_sim.py`, `scratch/flip_red_client.py`; logs `scratch/flip_red_*.log`):

| Gate | Perturbation | Result |
|---|---|---|
| D1 | damage from SMG row / head multiplier on body / unrolled base in message | RED ×3 |
| D2 | ray 2 cast from the aim origin along the aim dir | RED |
| D3 | `Has<HeadHitbox>` dropped | RED |
| D4 | instant reload / fire while reloading | RED ×2 |
| D5 | one pellet | RED |
| D6 | no recovery delay / no bloom growth | RED ×2 |
| D7 | probe: no `Sensor` only → GREEN (layers guard); probe: default layers only → GREEN (sensor guard); both removed → RED |
| D8 | `drop_queued_damage` removed | RED |
| D9 | no facing branch / no gait clamp | RED ×2 |
| D10 | `dead.contains` term disabled | RED |
| D11 | semi-auto fires from `fire_held` | RED |
| D12 | fixed `u = 0.5` / variance ×3 | RED ×2 |
| D14 | variance/reload errors name another field; head validation replaced by a different error | RED ×3 |
| E10 | label `value + 1` / headshot ignored / no despawn / non-player filter removed | RED ×4 |

(The first D14 head perturbation only renamed one occurrence of the field and stayed GREEN; replaced by a
different-error sabotage, RED.)

Runtime QA (release `dev` build, seed 1; output `scratch/qa_t6/summary.json` + PNGs):
`python tools/qa/scenarios/t6.py --out maw/tasks/in_progress/TASK-007/scratch/qa_t6` → PASS: layout hash golden,
3 dummies, 6 weapon pickups; pistol picked up (held Pistol); mouse-aimed body shot: drop 26 == number 26 in
[23, 28], magazine 11; head shot: drop 52 == red "52 CRIT" in [45, 55]; key 3 selects SMG; RMB-held burst:
`aiming == true`, magazine 30 → 21, fresh dummy 100 → 15; reload → 30; 0 damage numbers after 2 s; no font/asset
errors. Screenshots: `body_hit.png` (white "26" + hit marker), `crit_hit.png` (red larger "52 CRIT" + marker),
`aim_burst_1/2.png` (aim camera, crosshair arms, stacked drifting numbers, hit marker).
`python tools/qa/scenarios/t5.py --out .../scratch/qa_t5` → PASS (pickups, B1-adjacent flow, respawn).
Tracers were not caught in any capture: 60 ms, 2 cm wide and seen nearly end-on from behind the shooter.
No game process left running.

## 4. How to verify manually (owner checklist, for QA_REPORT.md)
`cargo run --features fast`, walk to the central park (dummies 10 m along −Z from its centre, 6 pickups in a
row through the centre):
- [ ] pick up pistol / SMG / shotgun and their ammo; keys 1-4 and the wheel switch; R reloads; ammo counter right.
- [ ] fire with and without RMB: recoil noticeable, not nauseating; placeholder noise-burst sound tolerable.
- [ ] tracer and muzzle flash readable (in the runs above the tracer was hard to see from behind the player:
      knobs `juice.ron` `tracer.width` / `tracer.seconds`).
- [ ] hit marker visible; red when the hit kills.
- [ ] aim camera (shoulder 0.55 m, 2 m, FOV 55°): in the captures the Kenney character's big head covers the left
      third of the view while aiming — `camera.ron` `aim_shoulder_offset` / `aim_distance` (GDD §3.2 values kept).
- [ ] strafe while aiming comfortable; no sprint while RMB is held (legs run forward while strafing: Kenney
      Mini Characters have no strafe clips — separate task).
- [ ] dummies drop to 0, lie 3 s and come back.
- [ ] damage numbers: size, pop/rise/fade timing, drift spacing; CRIT readable and red; shotgun's 10 numbers
      per blast acceptable or should be summed per target.
Knobs: `weapons.ron`, `aim.ron`, `camera.ron`, `juice.ron`, `mix.ron`, `strings.ron`, `locomotion.ron`.

children: 0 launched / 0 reported.
