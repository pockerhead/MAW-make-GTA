# IMPL_SUMMARY — TASK-008 (GDD T7, melee)

Status: DONE. Every step of PLAN_FINAL (1-21) is in. There are three deviations, listed below.
Pre-flight passed: every file, type and API the plan names was checked against the checkout and the
pinned sources (bevy-tnua 0.32.0 knockback/controller/action_state, avian3d 0.7.0 `cast_shape`/`ShapeHitData`,
bevy_animation 0.19.1 `pause_all`/`resume_all`/`is_paused`/`duration`/`AnimationTransitions::play`,
bevy_time 0.19.1 `FixedTimesteps`).

## 1. What was implemented (files, lines added/removed per `git diff --stat`; new files: total lines)

Sim (`gta_sim`, headless):
- `assets/combat/melee.ron` (new, 19): combo windows, damage, knockback, stagger/knockdown, Tnua knockback tuning.
- `crates/gta_sim/src/combat/melee.rs` (new, 695, of which about 200 are unit tests): `MeleeConfig` (strict loader and
  validation with the field in each error), `MeleeWeapon`, `Melee`, `Swing`, `HitReaction`, `MeleeHit`, private `Strike`,
  pure `advance_swing`, systems `recover_from_hits`, `swing_melee`, `apply_strikes`, `knock_back` (resets the knockback
  memory before `action_interrupt`), `reset_player_melee`, and the `advance_swing` unit table (3.1).
- `combat/mod.rs` (+47/-): `AttackSerial` (shared attack id), messages, reflection registration,
  `OnExit(Wasted)` reset, and the FixedUpdate order `tick_loadouts -> (recover, swing, apply).chain().before(TnuaUserControlsSystems) -> fire_weapons`.
  `collect_bat_pickups` is in `HealthSystems::Pickup`.
- `combat/hitscan.rs` (+27/-): `AttackSerial` replaces `Local<u32>`. Stagger and knockdown stop gun fire.
  A knocked-down head is not a hitscan target. The `DamageDealt.shot` doc is updated.
- `combat/weapons.rs` (+13/-): `Loadout.melee` and `Loadout.has_bat`. Key 1 while unarmed toggles fists/bat when a bat is owned.
- `combat/pickups.rs` (+42): `BatPickup` and `collect_bat_pickups`. `combat/range.rs` (+7/-): the bat pickup lies at park centre + 2 m Z.
- `character/mod.rs` (+36/-): `CharacterScheme::Knockback`. `Character` requires `HitReaction` and `Melee`.
  In `drive_characters`, stagger/knockdown sets a zero basis; a swing faces the blow, uses `swing_move_scale` and blocks jumping.
- `character/locomotion.rs` (+7/-): `tnua_config(knockback)`. `character/intent.rs` (+1): `WeaponRequest::Unarmed` doc.
- `lib.rs` (+10/-): loads and validates `MeleeConfig` in `compose_sim`.
- Tests: `crates/gta_sim/tests/melee.rs` (new, 527, 14 gates), `tests/config.rs` (+76, 5 melee config gates).

Client (`gta_like`, presentation):
- `assets/juice/juice.ron` + `src/juice/config.rs` (+63): `hit_stop_seconds` and `shake` (validated), 2 unit tests.
- `src/juice/hit_stop.rs` (new, 55): `HitStop`, `HitStopSystems`, `HitStopPlugin` (real time only).
- `src/juice/shake.rs` (new, 131): `CameraShake`, trauma from `MeleeHit`, `shake_rotation`, `smooth_noise`, 4 unit tests.
- `src/juice/mod.rs` (+17/-). `src/camera/mod.rs` (+9/-): shake is multiplied into the camera rotation after the aim ray is written.
- `assets/character/visual.ron` + `src/visuals/character_config.rs` (+42): `fists`, `bat`, `knockdown`, `rest` clips,
  resolved through the manifest rig.
- `src/visuals/character.rs` (+148/-, 474 total): full-body swing and knockdown clips, with swing speed = clip duration / sim
  swing duration. The arm layer is off while knocked down. `apply_hit_stop` pauses/resumes the animator. The rest-pose layer is new (see deviations).
- `assets/world/render.ron`, `src/visuals/config.rs`, `weapons.rs`, `mod.rs`: a primitive bat in the hand (reuses `HeldGun`)
  and on the range.
- `src/visuals/character_gate.rs` (+225, 697 total): harness split `character_visuals_app_with`, updated literals,
  graph-node check for the melee and rest nodes, and 2 new gates (hit-stop, melee clips).
- `tools/qa/scenarios/t7.py` (new, 212): runtime BRP scenario.

## 2. Deviations from plan
1. **Inner melee tuple is `.chain()`ed** (`combat/mod.rs`). The plan's `(recover, swing, apply).before(..)` inside the outer
   `.chain()` leaves the three systems unordered. `apply_strikes` sometimes ran before `swing_melee`, and hits landed one tick
   late, nondeterministically (T0+9 instead of T0+8). `hit_lands_only_in_window` caught it. There is a flip case for it
   (apply chained before swing -> RED). Logged as `dead_end`, and a PCTX proposal was added.
2. **`AttackSerial::next` renamed `next_id`**: clippy `should_implement_trait` (`-D warnings`) rejects `next`.
3. **Rest-pose animation layer (new, not in the plan).** The Kenney `die` clip keys the root rotation, and the kick keys the legs and
   the root translation. `idle` keys neither. In the windowed game a knocked-down dummy therefore stayed lying after
   `HitReaction` was already `Steady` (`scratch/probe_glb_channels.log`, screenshot before the fix). Fix: the `static` clip
   (keys every joint) plays under all clips at node weight `rest.weight = 0.01` from `visual.ron`. Bevy blends each property
   as a weighted average (`animation_curves.rs` `combine`), so animated joints are about 99 % unchanged and unkeyed joints return to rest.
   Screenshot after the fix: `scratch/qa/probe_visual/standing_again.png` (both dummies stand, facing the attacker).
   Side effect: legs no longer freeze in the last walk frame when going idle, which was already happening before this task. Logged as `decision`.
- t7.py aims at a point 20 m beyond the dummy on the same line, not at its chest. From 1 m the shoulder camera never
  converged within `aim_at`'s 0.1 m (miss 0.275 m), and only the aim yaw steers a swing (logged as `decision`).
- The plan's "owner checklist in QA_REPORT.md" is given in section 4 below. QA_REPORT belongs to the QA stage.

## 3. Test results
- `cargo build`: green. `cargo clippy -- -D warnings`: green. `cargo clippy --workspace --all-targets -- -D warnings`: green.
- `cargo test -p gta_sim`: 101 passed, 0 failed (lib 19, melee 14, config 19, rest unchanged). Log: `scratch/test_gta_sim.log`.
- `cargo test -p gta_like --bin gta_like`: 32 passed.
- Knockback measurements (`knockback_pushes_along_the_blow`, with `force_forward: Some(-d)`): 0.177 m along d for each of
  −Z, +X and +Z; lateral 0.0006-0.0007 m. Two shoves: 1.108 m. Real jab during a knockback: 1.750 -> 3.047 m/s.
- Flip-RED of the sim gates: `scratch/flip_red_sim.py`, log `scratch/flip_red_sim.log`. 16/16 went RED through a test panic, not a compile error,
  and GREEN after restore. Perturbations: `active_from` 0.10; finisher `knockdown:false`; negated shove; memory reset deleted
  (both shove gates); direct `action_interrupt`; no cancel while staggered; `fire_weapons` ignores `HitReaction`;
  knocked-down head sensor on; private attack id `|| 1`; `reset_player_melee` unregistered; sweep mask without `World`;
  `collect_bat_pickups` unregistered; apply chained before swing; the three config rules removed.
  The 3.1 unit table flip ("decrement `combo_left` on the end tick") turned `single_jab_window_and_end` and `combo_grace_boundary` RED.
- Flip-RED of the client gates: `scratch/flip_red_client.py`, log `scratch/flip_red_client.log`. 6/6: `set_relative_speed(0)` in
  `start_hit_stop` (time assertion RED); `apply_hit_stop` unregistered (pause RED); freeze of every animator (bystander RED);
  knockdown clip never played (liveness RED); shake ignores trauma; `hit_stop_seconds` not validated.
- `python tools/qa/scenarios/t7.py --out scratch/qa/t7`: PASS. Combo drop 100 -> 60, KnockedDown then Steady again,
  live numbers {10, 20}, bat picked up, key 1 toggles `Fists`/`Bat`, bat drop 25 with knockdown, no log errors.
  `python tools/qa/scenarios/t6.py --out scratch/qa/t6`: PASS (6 `WeaponPickup` rows, shooting unchanged).
- `python tools/qa/tree_check.py`: passed. `cargo tree -p gta_sim -e features -i bevy_render`: empty. No `Cargo.lock` change.
- Screenshots I checked by eye: `scratch/qa/t7/combo_1.png` (jab with a "10"), `combo_2.png` (kick finisher, "10"/"20"),
  `bat_knockdown.png`, and `scratch/qa/probe_visual/*` (dummy lying from the side, bat in hand, bat swing with "25", dummies standing again).
  No game process is left running.

## 4. How to verify manually (owner checklist, Russian)
`cargo run --release -- --seed 1`, тир в парке (манекены в 10 м от центра парка по −Z):
- [ ] три ЛКМ по манекену без оружия (клавиша 1): удары читаются как комбо (правый, левый, пинок), третий сбивает с ног, манекен встаёт через ~1.2 с;
- [ ] заморозка на ударе (50 мс) ощутима, но короткая; драка "хлёсткая";
- [ ] тряска камеры заметна на ударах и не тошнит;
- [ ] бита лежит в 2 м от центра парка по +Z, подбирается; клавиша 1 без оружия переключает кулаки/бита; удар битой тяжелее и сразу сбивает с ног;
- [ ] бита в руке выглядит сносно (хват настроен под пистолет, `render.ron` `hand_offset`, `bat_size`, `bat_color`).
Tuning files: `melee.ron` (windows, knockback, stagger/knockdown), `juice.ron` (`hit_stop_seconds`, `shake`),
`visual.ron` (clips, `rest.weight`), `render.ron` (bat size/colour).

## Known limits
- The victim does not stagger visually, because the rig has no hit clip. Stagger shows only through knockback and hit-stop (owner run).
- Same-tick strikes on one victim do not sum their shoves (the later one wins). Frame advantage: stagger is 0.35 s (22.4 ticks), and a buffered
  next jab lands 23 ticks after the previous one, so an NPC victim has a 1-tick gap in which it could act between jabs. Both are recorded for T9.
- `AnimationTransitions::play` skips the fade-out of a paused animation. A clip change during the 50 ms freeze
  (not seen in T7 flows) would cut instead of blend.

children: 0 launched / 0 reported.
