# PLAN_FINAL — TASK-008 (GDD T7): melee combat

Pinned versions (from `Cargo.lock`; sources under `~/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/`):
bevy / bevy_ecs / bevy_animation / bevy_time 0.19.1, avian3d 0.7.0, bevy-tnua 0.32.0 (+ bevy-tnua-macros 0.3.0),
vendored `vendor/bevy-tnua-avian3d-0.12.1`. **No new crate, no `Cargo.lock` change.** `TnuaBuiltinKnockback` ships in
bevy-tnua 0.32.0 (`src/builtins/knockback.rs:32-47`).

Probes (scratch workspace copy with its own target dir; nothing in the checkout touched):
`scratch/probe_ws/crates/gta_sim/tests/probe_knockback.rs`; logs `scratch/probe_knockback.log`,
`scratch/probe_second_shove.log` (planner), `scratch/probe_r2_overlap.log` (this review).

## 1. Summary

T7 adds melee to the headless sim and its presentation to the client. `gta_sim::combat::melee` (new file) owns a strict
`MeleeConfig` from `assets/combat/melee.ron`, an attacker component `Melee` driven by a pure state machine
`advance_swing` (fist combo of 3, one bat swing, attack windows from data, one buffered click, combo grace window), and a
victim component `HitReaction` (`Steady | Staggered | KnockedDown`). In `FixedUpdate`, `swing_melee` sweeps a 0.35 m
sphere from the chest on the explicit `[World, Character]` mask during the open window and writes a private `Strike`
message; `apply_strikes` applies `Health::take`, escalates the reaction, shoves the victim through Tnua's
`TnuaBuiltinKnockback` (with a memory reset so a second shove during a running knockback is not dropped), and writes the
existing `DamageDealt` plus a new public `MeleeHit`. Guns and fists share one `AttackSerial` for `DamageDealt.shot`.
Stagger/knockdown stop the victim's walk, swing and gun fire; knockdown turns off its head sensor for hitscan. The bat is
a `Loadout` flag (`has_bat`, `melee: Fists|Bat`) picked up from a `BatPickup` on the range; key 1 toggles fists/bat.
The client reads sim state: full-body swing/knockdown clips, a local 50 ms animation freeze of attacker and target on
`Time<Real>` (never `Time<Virtual>`), rotational trauma camera shake applied after the aim ray is written, and a
primitive bat mesh. Gates: `advance_swing` unit table, headless integration tests in `tests/melee.rs`, config fixtures,
a client presentation gate for hit-stop, and the BRP scenario `tools/qa/scenarios/t7.py`.

## 2. Implementation steps

Verified facts the steps rely on (all checked in pinned source during this review):
- `fire_weapons` (`combat/hitscan.rs:150-155`) takes `ActionIntent.fire_requested` for every living `Loadout` owner
  **before** it checks `loadout.held`; the input layer (`src/input/mod.rs:191-199`) raises it on every armed LMB press.
  Hence melee must run after `tick_loadouts` and before `fire_weapons` and take the flag only when `held == None`.
- `DamageDealt.damage` is the amount passed to `Health::take`, i.e. **before** armour (`hitscan.rs:71-72`,
  `character/health.rs` absorbs armour first). Melee keeps this contract.
- `TnuaController::action_interrupt` on an already-running action of the same variant only replaces `state.input`
  (`bevy-tnua-0.32.0/src/controller.rs:431-456`, macro `update_in_action_state`,
  `bevy-tnua-macros-0.3.0/src/scheme_derive/codegen.rs:88-105`); `TnuaBuiltinKnockback::apply` applies the shove only in
  memory `Shove` (`knockback.rs:113-124`). `TnuaController.current_action` is `pub` (`controller.rs:188`);
  `TnuaActionState { input, config, memory }` fields are `pub` (`action_state.rs:12-20`).
- Tnua sets: `Sensors → TnuaUserControlsSystems → Logic → Motors` chained in `FixedUpdate` (`controller.rs:53-62`).
- avian 0.7 `SpatialQuery::cast_shape(&shape, origin, rot, Dir3, &ShapeCastConfig, &SpatialQueryFilter) ->
  Option<ShapeHitData>`; `ShapeHitData { entity, distance, point1 (on hit shape), point2, normal1, normal2 }`;
  `ShapeCastConfig::from_max_distance` keeps `ignore_origin_penetration: false` (overlap at start = hit at distance 0).
- bevy_animation 0.19.1: `AnimationPlayer::pause_all/resume_all` (`lib.rs:922,931`), `ActiveAnimation::is_paused`
  (`:630`), `AnimationClip::duration` (`:254`); `AnimationPlayer::all_paused()` is `true` for a player with nothing
  playing (`:914-917`) — never use it as a gate assertion.
- bevy_time 0.19.1: `TimeUpdateStrategy::FixedTimesteps(n)` advances `Time<Real>` by `timestep × n` per update
  (`lib.rs:181-183`); the first update only initialises real time (0 delta, 0 fixed ticks). Default fixed step
  15 625 µs = exactly 1/64 s, so `delta_secs()` is binary-exact.
- Rig clip indices (`assets/third_party/manifest.ron:81-84`): 1 idle … 9 `die` … 19 `attack-melee-right`,
  20 `attack-melee-left`, 21 `attack-kick-right`. No hit/stagger clip exists.
- Geometry (`assets/character/locomotion.ron`): capsule r 0.3, total height 1.5, `float_height` 1.05 → capsule spans
  feet+0.3 … feet+1.8; head sensor r 0.35 centred at feet+1.6. Range pickups: `row(i, 6, 2.0)` → gun pickups at
  park_center + (±1, ±3, ±5, 0, 0); pickup radius 1.0.

### Sim — data and config

1. **`assets/combat/melee.ron` (new).** Every value is owner tuning; none is a `const`:
   ```ron
   (
       cast_radius: 0.35,        // m, sphere swept from the chest (GDD §4.2)
       cast_height: 1.3,         // m above the feet
       combo_window: 0.4,        // s after a swing ends in which a click continues the combo
       swing_move_scale: 0.0,    // share of gait speed kept while swinging (0 = feet planted)
       stagger: 0.35,            // s
       knockdown: 1.2,           // s
       fists: (range: 1.0, hits: [
           (damage: 10, duration: 0.35, active_from: 0.12, active_to: 0.22, knockback: 2.0, knockdown: false),
           (damage: 10, duration: 0.35, active_from: 0.12, active_to: 0.22, knockback: 2.0, knockdown: false),
           (damage: 20, duration: 0.35, active_from: 0.12, active_to: 0.22, knockback: 5.0, knockdown: true),
       ]),
       bat: (range: 1.3, hits: [
           (damage: 25, duration: 0.6, active_from: 0.2, active_to: 0.32, knockback: 6.0, knockdown: true),
       ]),
       knockback_tuning: (no_push_timeout: 0.2, barrier_strength_diminishing: 2.0,
                          acceleration_limit: 3.0, air_acceleration_limit: 1.0),
   )
   ```
   Jab knockback 2.0 m/s is the approved exception (TASK_FINAL Q1): probe slide 0.177 m per 2 m/s jab vs 0.363 m at
   3 m/s; two 3 m/s jabs push a dummy from 1.0 m to 1.73 m, past fist reach 1.65 m (range 1.0 + sphere 0.35 + capsule
   0.3). `knockback_tuning` = Tnua library defaults (`knockback.rs:76-85`), now owned by our file.

2. **`crates/gta_sim/src/combat/melee.rs` (new) — config.**
   - `pub const MELEE_CONFIG: &str = "combat/melee.ron";` (path, not tuning).
   - `#[derive(Resource, Deserialize, Clone, Debug)] #[serde(deny_unknown_fields)] pub struct MeleeConfig { pub
     cast_radius: f32, pub cast_height: f32, pub combo_window: f32, pub swing_move_scale: f32, pub stagger: f32, pub
     knockdown: f32, pub fists: MeleeWeaponStats, pub bat: MeleeWeaponStats, pub knockback_tuning: KnockbackTuning }`.
   - `MeleeWeaponStats { pub range: f32, pub hits: Vec<MeleeHitStats> }` and `MeleeHitStats { pub damage: u32, pub
     duration: f32, pub active_from: f32, pub active_to: f32, pub knockback: f32, pub knockdown: bool }`, both
     `deny_unknown_fields`. `damage` is an integer so `DamageDealt.damage == damage` passed to `Health::take` (as f32).
   - `KnockbackTuning { no_push_timeout, barrier_strength_diminishing, acceleration_limit, air_acceleration_limit: f32 }`
     (`deny_unknown_fields`) with `pub fn tnua(&self) -> TnuaBuiltinKnockbackConfig` (struct literal; Tnua's own type
     derives `Deserialize` without `deny_unknown_fields`, so we do not deserialize it directly).
   - `pub fn stats(&self, w: MeleeWeapon) -> &MeleeWeaponStats`; `pub fn hit(&self, w: MeleeWeapon, step: u8) ->
     &MeleeHitStats` = `&stats.hits[step as usize % stats.hits.len()]`.
   - `pub fn validate(&self) -> Result<(), String>`; the error names the field (e.g. `fists.hits[1].active_to is out of
     range`): every float finite; `cast_radius, cast_height, stagger, knockdown > 0`; `combo_window >= 0`;
     `0 <= swing_move_scale <= 1`; per weapon `range > 0`, `hits` non-empty (`fists.hits is empty`); per hit
     `damage >= 1`, `0 <= active_from < active_to < duration`, `knockback >= 0`; all four knockback tuning values `> 0`.

3. **`melee.rs` — types.**
   - `#[derive(Reflect, Default, Clone, Copy, Debug, PartialEq, Eq, Deserialize)] pub enum MeleeWeapon { #[default]
     Fists, Bat }` with `pub fn other(self) -> Self`.
   - `#[derive(Component, Reflect, Default, Clone, Debug)] #[reflect(Component, Default)] pub struct Melee { pub swing:
     Option<Swing>, pub next_step: u8, pub combo_left: f32, pub queued: bool }` (`///` doc per field).
     `pub fn cancel(&mut self)` sets `swing = None, queued = false, next_step = 0, combo_left = 0.0`.
   - `#[derive(Reflect, Clone, Copy, Debug, PartialEq)] pub struct Swing { pub weapon: MeleeWeapon, pub step: u8, pub
     elapsed: f32, pub duration: f32, pub direction: Vec3, pub landed: bool, pub attack: u32 }`. `duration` is copied
     at start so presentation reads sim state, not `melee.ron` (GDD §12 "one owner").
   - `#[derive(Component, Reflect, Default, Clone, Copy, Debug, PartialEq)] #[reflect(Component, Default)] pub enum
     HitReaction { #[default] Steady, Staggered { left: f32 }, KnockedDown { left: f32 } }` with `is_active()`,
     `is_knocked_down()`, `escalate(&mut self, knockdown: bool, cfg: &MeleeConfig)`:
     `knockdown → KnockedDown { left: cfg.knockdown }` (always, also re-arms a running knockdown);
     else if already `KnockedDown` → unchanged (stagger never shortens a knockdown); else `Staggered { left: cfg.stagger }`.
   - `#[derive(Message, Reflect, Clone, Copy, Debug)] #[reflect(Message)] pub struct MeleeHit { pub attacker: Entity,
     pub target: Entity, pub point: Vec3, pub knockdown: bool }` — one applied hit (presentation input). Buffered
     `Message`, not an observer `Event`: two client consumers (hit-stop, shake) read it later in the same frame.
   - `#[derive(Message, Clone, Copy, Debug)] pub(super) struct Strike { attacker: Entity, target: Entity, point: Vec3,
     direction: Vec3, damage: u32, knockback: f32, knockdown: bool, attack: u32 }` — written and read in the same
     fixed tick by two chained systems, so it cannot leak across state changes.
   - `BatPickup` lives in `pickups.rs` (step 9).

4. **`melee.rs` — `advance_swing` (pure; the tick order below is normative, every test number is derived from it).**
   Signature: `pub fn advance_swing(melee: &mut Melee, clicked: bool, weapon: MeleeWeapon, direction: Vec3,
   next_attack: impl FnOnce() -> u32, dt: f32, cfg: &MeleeConfig) -> bool` — returns "window open this tick and the
   swing has not landed".
   ```text
   A. if melee.swing is Some(s):
        melee.queued |= clicked
        s.elapsed += dt
        if s.elapsed < s.duration: return window(s)
        // swing ends this tick
        melee.next_step = (s.step + 1) % hits(s.weapon).len()
        melee.combo_left = cfg.combo_window          // full value; NOT decremented on this tick
        melee.swing = None
        if !take(&mut melee.queued): return false
        goto C                                       // buffered click starts the next swing on the end tick
   B. else (idle):
        melee.combo_left = max(melee.combo_left - dt, 0)
        if !clicked: return false
   C. start:
        step = if melee.combo_left > 0 { melee.next_step % hits(weapon).len() } else { 0 }
        melee.swing = Some(Swing { weapon, step, elapsed: 0, duration: hit(weapon, step).duration, direction,
                                   landed: false, attack: next_attack() })
        return window(swing)
   window(s) = !s.landed && hit.active_from <= s.elapsed && s.elapsed <= hit.active_to   (hit = cfg.hit(s.weapon, s.step))
   ```
   Worked table (`dt = 1/64`, tick `T0 + k` of a swing started at `T0` has `elapsed = k/64`, exact):
   `0.12·64 = 7.68` and `0.22·64 = 14.08` ⇒ window k = 8..14; `0.35·64 = 22.4` ⇒ swing ends at k = 23.
   Bat: `0.2·64 = 12.8`, `0.32·64 = 20.48` ⇒ window k = 13..20; `0.6·64 = 38.4` ⇒ ends at k = 39.
   Combo grace: after an end at k = 23, the idle tick 23 + n leaves `combo_left = 0.4 − n/64`; n = 25 (k = 48) ⇒
   0.0094 > 0 ⇒ step 1; n = 26 (k = 49) ⇒ ≤ 0 ⇒ step 0.

5. **`melee.rs` — systems** (all `FixedUpdate`, `Res<Time<Fixed>>`, golden path: let-else + `continue`).
   - `recover_from_hits(time, mut q: Query<&mut HitReaction>)`: for `Staggered{left}`/`KnockedDown{left}`:
     `left -= dt`; `Steady` when `left <= 0`. All characters including `Dead` (a stale reaction self-heals).
   - `swing_melee`: attackers `Query<(Entity, &Position, &Rotation, &CharacterBody, &AimIntent, &mut ActionIntent,
     &Loadout, &mut Melee, &HitReaction), (With<Character>, Without<Dead>)>`; `SpatialQuery`,
     `ResMut<AttackSerial>`, `MessageWriter<Strike>`, `Res<MeleeConfig>`, `Res<Time<Fixed>>`, `Query<&ColliderOf>`,
     `Query<(), (With<Character>, With<Health>, Without<Dead>)>` (live targets). Per attacker, in this order:
     1. `if loadout.held.is_some() { melee.cancel(); continue }` — the click belongs to `fire_weapons`, do not take it.
     2. `if reaction.is_active() { melee.cancel(); action.fire_requested = false; continue }` — a click while staggered is dropped.
     3. `if melee.swing.is_some_and(|s| s.weapon != loadout.melee) { melee.cancel() }` — weapon toggled mid-swing.
     4. `let clicked = std::mem::take(&mut action.fire_requested);`
        `direction` = `Vec3::new(aim.direction.x, 0, aim.direction.z).normalize_or_zero()`, if zero then
        `(rotation.0 * Vec3::NEG_Z)` flattened+normalized (used only when a swing starts).
     5. `if !advance_swing(&mut melee, clicked, loadout.melee, direction, || serial.next(), dt, &cfg) { continue }`
     6. Cast `Collider::sphere(cfg.cast_radius)` from `position.0 − Y·body.float_height + Y·cfg.cast_height`, rotation
        `Quat::IDENTITY`, along `Dir3::new(swing.direction)`, `ShapeCastConfig::from_max_distance(cfg.stats(w).range)`,
        filter `SpatialQueryFilter::from_mask([GameLayer::World, GameLayer::Character]).with_excluded_entities([attacker])`
        (mask named explicitly: head sensors on `Hitbox` stay out; TASK-007 lesson). No hit ⇒ `continue` (window stays open next tick).
     7. Resolve `body = colliders.get(hit.entity).map_or(hit.entity, |of| of.body)`. Set `swing.landed = true` for any
        hit. If `body` is not a live target (wall, prop) ⇒ no strike (the swing is spent on the wall).
        Else write `Strike { attacker, target: body, point: hit.point1, direction: swing.direction, damage,
        knockback, knockdown, attack: swing.attack }` from `cfg.hit(swing.weapon, swing.step)`.
   - `apply_strikes`: `MessageReader<Strike>`, `Query<(&mut Health, &mut HitReaction, &mut
     TnuaController<CharacterScheme>), Without<Dead>>`, `Res<MeleeConfig>`, `MessageWriter<DamageDealt>`,
     `MessageWriter<MeleeHit>`. Per strike in message order: `let Ok(..) = q.get_mut(strike.target) else { continue }`;
     `if health.current <= 0.0 { continue }` (killed earlier this tick, `Dead` still deferred — same rule as
     `hitscan.rs:235-241`); `let killed = health.take(strike.damage as f32)`; `reaction.escalate(strike.knockdown,
     &cfg)`; `knock_back(&mut controller, strike.direction * strike.knockback)`; then write `DamageDealt { shooter:
     attacker, shot: attack, target, point, damage, headshot: false, killed }` and `MeleeHit { attacker, target, point,
     knockdown }`. Messages are written only after `Health::take` (an applied hit). Two strikes on one victim in the
     same tick: both damage, the later shove replaces the earlier one (Tnua keeps one contender); message order is the
     attacker query order — acceptable for T7 (one attacker), noted for T9.
   - `pub fn knock_back(controller: &mut TnuaController<CharacterScheme>, shove: Vec3)`: `let Ok(forward) =
     Dir3::new(-shove) else { return }` (zero shove = no knockback);
     `if let Some(CharacterSchemeActionState::Knockback(state)) = controller.current_action.as_mut() { state.memory =
     TnuaBuiltinKnockbackMemory::Shove; }` then `controller.action_interrupt(CharacterScheme::Knockback(
     TnuaBuiltinKnockback { shove, force_forward: Some(forward) }))`. One comment line: "an interrupt on a running
     knockback only swaps its input; without the reset the new shove is dropped". `force_forward` turns the victim to
     face the attacker (angular motor only).
   - `reset_player_melee(Query<(&mut Melee, &mut HitReaction), With<Player>>)` on `OnExit(GameState::Wasted)`:
     `Melee::default()`, `HitReaction::Steady` (a knockdown or buffered swing from before death must not continue at
     the hospital; TASK-006/007 leak class). `Strike` needs no clearing (same-tick); `MeleeHit` is presentation-only.

6. **`crates/gta_sim/src/combat/mod.rs`.** `mod melee;` + `pub use melee::{HitReaction, KnockbackTuning, MELEE_CONFIG,
   Melee, MeleeConfig, MeleeHit, MeleeHitStats, MeleeWeapon, MeleeWeaponStats, Swing, advance_swing, knock_back};`
   `pub use pickups::BatPickup;`. New `#[derive(Resource, Default)] pub struct AttackSerial(u32)` with `pub fn
   next(&mut self) -> u32 { self.0 = self.0.wrapping_add(1); self.0 }` (`///` "id of one attack — a trigger pull or a
   melee swing — carried in `DamageDealt.shot`"). Plugin additions: `init_resource::<AttackSerial>()`,
   `add_message::<MeleeHit>()`, `add_message::<melee::Strike>()`, `register_type` for `Melee, Swing, MeleeWeapon,
   HitReaction, MeleeHit, BatPickup`; `add_systems(OnExit(GameState::Wasted), melee::reset_player_melee)`. Replace the
   `FixedUpdate` damage tuple with:
   ```rust
   (
       weapons::tick_loadouts,
       (melee::recover_from_hits, melee::swing_melee, melee::apply_strikes)
           .before(TnuaUserControlsSystems),
       hitscan::fire_weapons,
   )
       .chain()
       .in_set(HealthSystems::Damage),
   ```
   and add `pickups::collect_bat_pickups` next to `collect_weapon_pickups` in `HealthSystems::Pickup`. All stay inside
   the existing outer `.in_set(PlayingSystems)`. Reasons: melee before `fire_weapons` (step 2 facts); before
   `TnuaUserControlsSystems` so the victim's zeroed basis (`drive_characters`) and the knockback interrupt act in the hit
   tick, exactly as the probe applied them.

7. **`crates/gta_sim/src/combat/hitscan.rs` (`fire_weapons`).**
   - Replace `mut pulls: Local<u32>` with `mut serial: ResMut<AttackSerial>`; `let shot = serial.next();` at the point
     where `*pulls = pulls.wrapping_add(1)` is today; `DamageDealt { shot, .. }`. Only `CombatPlugin` registers
     `fire_weapons` (grep: no harness adds it).
   - Add `&HitReaction` to the shooter query. Right after `let requested = std::mem::take(&mut action.fire_requested);`
     insert `if reaction.is_active() { continue; }` — stagger/knockdown interrupt gun fire too (GDD §4.2 "stagger
     прерывает атаку цели"), including held automatic fire.
   - Head sensor off in knockdown (GDD §4.1): add `reactions: Query<&HitReaction>`; predicate becomes
     `of.body != shooter && !(head && (dead.contains(of.body) || reactions.get(of.body).is_ok_and(|r| r.is_knocked_down())))`.
     Update the comment above it ("…the head sensor of a dead or knocked-down character").
   - `DamageDealt.shot` doc: "Attack (trigger pull or melee swing) that dealt it: every pellet of one shotgun blast
     carries the same value."

8. **`crates/gta_sim/src/combat/weapons.rs`.** `Loadout` gains `/// What empty hands swing. pub melee: MeleeWeapon` and
   `/// A bat was picked up (survives death like the guns, GDD §3.4). pub has_bat: bool` (defaults Fists/false).
   `select_weapon`: capture `let was_unarmed = loadout.held.is_none();` before the request; the arm becomes
   `WeaponRequest::Unarmed => { if was_unarmed && loadout.has_bat { loadout.melee = loadout.melee.other(); } None }`.
   `cycle_weapon` unchanged (ring `[None, guns]`; `None` = current melee weapon). Update the `WeaponRequest::Unarmed`
   doc in `character/intent.rs`: "Melee slot; pressed again while unarmed, toggles fists/bat when a bat is owned."
   `tests/respawn.rs` builds `Loadout { held, ..default() }` — compiles unchanged.

9. **`crates/gta_sim/src/combat/pickups.rs`.** `#[derive(Component, Reflect, Debug, Default)] #[reflect(Component)] pub
   struct BatPickup { pub cooldown: f32 }` + `available()`. `collect_bat_pickups` mirrors `collect_weapon_pickups`
   with `WeaponsConfig.pickups.{radius, respawn}` (shared pickup rule, no new number): players `(With<Player>,
   Without<Dead>)`; on contact, if `loadout.has_bat` → nothing (no cooldown); else `has_bat = true`, if `held.is_none()`
   → `melee = Bat`, `cooldown = respawn`. Separate component, not a `WeaponPickup` variant: `t6.py` asserts exactly 6
   `WeaponPickup` rows.

10. **`crates/gta_sim/src/combat/range.rs`.** In `spawn_range`, spawn `(BatPickup::default(), Name::new("Bat pickup"),
    Transform::from_translation(c + Vec3::Z * r.pickup_spacing))`. Worked: gun pickups at c + (±1, ±3, ±5, 0, 0), bat at
    c + (0, 0, 2) ⇒ nearest gun pickup √(1² + 2²) = 2.236 m > radius 1.0, so t6 teleports onto gun pickups never
    collect the bat; dummies are 10 m along −Z.

11. **`crates/gta_sim/src/character/mod.rs`.**
    - `CharacterScheme`: add `Knockback(TnuaBuiltinKnockback)` (probe compiled and ran with it; import from
      `bevy_tnua::builtins`).
    - `Character`: `#[require(MoveIntent, AimIntent, ActionIntent, JumpBuffer, AnimState, HitReaction, Melee)]`.
    - `CharacterControlConfig::from_world`: `let knockback = world.resource::<MeleeConfig>().knockback_tuning.tnua();
      let config = world.resource::<LocomotionConfig>().tnua_config(knockback);`.
    - `drive_characters`: add `melee_cfg: Res<MeleeConfig>` and `(&HitReaction, &Melee)` to the query. Branch
      `if dead || reaction.is_active()` → the existing zero-basis block (jump request cleared, buffer 0, no jump fed).
      While `melee.swing` is `Some(s)`: `desired_motion = direction * speed * melee_cfg.swing_move_scale`,
      `desired_forward = Dir3::new(s.direction).ok()`, and skip the jump feeding (buffer keeps counting down).
      Only `CharacterPlugin` registers `drive_characters`; every harness builds through `compose_sim`, which inserts
      `MeleeConfig` (grep confirmed: no other `CharacterPlugin`/`CombatPlugin` site).

12. **`crates/gta_sim/src/character/locomotion.rs`.** `pub fn tnua_config(&self, knockback:
    TnuaBuiltinKnockbackConfig) -> CharacterSchemeConfig` fills the new `knockback` field (the derive names the config
    field after the variant in snake case). The only caller is step 11.

13. **`crates/gta_sim/src/lib.rs`.** Load and validate `MeleeConfig` exactly like the other configs (`load_config` +
    `validate` mapped to `ConfigError { path: root.path(MELEE_CONFIG), message }`) and `insert_resource(melee)` before
    `add_plugins` (`CharacterControlConfig::from_world` reads it inside `CharacterPlugin::build`).

### Client

14. **`assets/juice/juice.ron` + `src/juice/config.rs`.** New fields `hit_stop_seconds: 0.05` (real seconds, GDD §8) and
    `shake: (melee_trauma: 0.25, decay_per_s: 1.2, max_yaw_deg: 3.0, max_pitch_deg: 3.0, max_roll_deg: 5.0,
    noise_hz: 15.0)` as `pub shake: ShakeConfig` (`deny_unknown_fields`). Validation (field-named errors via existing
    `positive`/`non_negative` helpers): `hit_stop_seconds > 0`; `0 < melee_trauma <= 1`; `decay_per_s > 0`;
    `max_*_deg >= 0`; `noise_hz > 0`. `damage_numbers_gate.rs` loads the shipped file — unaffected.

15. **`src/juice/hit_stop.rs` (new).** `#[derive(Component, Default)] pub struct HitStop { pub left: f32 }` (real
    seconds); `#[derive(SystemSet, ..)] pub struct HitStopSystems`; `pub struct HitStopPlugin`: observer `On<Add,
    Character>` → insert `HitStop::default()` (attached once, never inserted/removed per hit); `Update`:
    `(tick_hit_stop, start_hit_stop).chain().in_set(HitStopSystems)`. `tick_hit_stop`: `left = (left −
    real.delta_secs()).max(0)` with `Res<Time<Real>>`. `start_hit_stop`: per `MeleeHit`, attacker and target `left =
    juice.hit_stop_seconds` via `get_mut` (missing entity ignored). No `Time<Virtual>` anywhere in juice (GDD §8).

16. **`src/juice/shake.rs` (new) + `src/juice/mod.rs`.** `#[derive(Resource, Default)] pub struct CameraShake { pub
    trauma: f32, pub rotation: Quat }`. `add_melee_trauma` (reads `MeleeHit`; if the `Player` is attacker or target,
    `trauma = (trauma + melee_trauma).min(1.0)`), then `shake_camera` (`Time<Real>`): `trauma = (trauma − decay_per_s ·
    dt).max(0)`, `rotation = shake_rotation(trauma, real.elapsed_secs() * noise_hz, &cfg.shake)`.
    `pub fn shake_rotation(trauma, t, cfg) -> Quat` = `Quat::from_euler(YXZ, yaw·s·n(0,t), pitch·s·n(1,t),
    roll·s·n(2,t))` with `s = trauma²` and angles in radians. `pub fn smooth_noise(channel: u32, t: f32) -> f32` in
    [−1, 1]: integer-hash lattice value at `floor(t)` and `floor(t)+1` for the channel, smoothstep blend. Hash
    constants are algorithm, not tuning. `JuicePlugin`: `add_plugins(HitStopPlugin)`, `init_resource::<CameraShake>()`,
    `add_systems(Update, (add_melee_trauma, shake_camera).chain())`. Update the plugin doc line.

17. **`src/camera/mod.rs` `follow_player`.** New param `shake: Res<CameraShake>` (only `CameraPlugin` registers the
    system; `JuicePlugin` inits the resource like `CameraRecoil`). Last line becomes
    `camera_transform.rotation = rotation * Quat::from_rotation_x(recoil.pitch) * shake.rotation;` — after
    `aim.origin/direction` are written, so the aim ray is unaffected (rotational shake: TASK_FINAL Q3).

18. **`assets/character/visual.ron` + `src/visuals/character_config.rs`.** New fields `fists: ["attack-melee-right",
    "attack-melee-left", "attack-kick-right"]` (combo step order), `bat: "attack-melee-right"`, `knockdown: "die"`.
    `fists: Vec<String>`; `resolve` pushes `"fists must name exactly 3 clips, got N"` when `len != 3`, resolves names
    through the rig like the others, and fills `CharacterClips { .., melee: [usize; 3], bat: usize, knockdown: usize }`
    (stays `Copy`). Shipped indices: melee `[19, 20, 21]`, bat 19, knockdown 9.

19. **`src/visuals/character.rs`.**
    - `CharacterAnimations`: add full-body nodes `fists: [AnimationNodeIndex; 3]`, `bat`, `knockdown` (`add_clip`, no
      mask) and their `Handle<AnimationClip>`s (`fist_clips: [Handle<AnimationClip>; 3]`, `bat_clip`) for durations.
    - `CharacterAnimator` gains `pub(super) action: Option<ShownAction>` with `enum ShownAction { Swing(u32 /* attack
      id */), Knockdown }`. Update `wire_player` and the two hand-built literals in `character_gate.rs` (`action: None`).
    - `drive_character_animation`: characters query adds `&HitReaction, &Melee`; add `Res<Assets<AnimationClip>>`.
      Per animator, wanted action: `KnockedDown` → `Knockdown`; else `melee.swing = Some(s)` → `Swing(s.attack)`; else
      `None`. On a changed wanted action: `Knockdown` → `transitions.play(knockdown, blend)` without `repeat()` (holds the
      last pose); `Swing` → play `fists[s.step]` or `bat` once, speed `clip.duration() / s.duration` when the clip asset
      is loaded, else 1.0 (bounded fallback; corrected on the next frame it loads); `None` after an action → force the
      locomotion replay (treat as `shown` mismatch). While an action is shown, skip the locomotion `play` and the
      locomotion speed write; while `KnockedDown`, pass `pose = None` to `drive_arms` so an armed victim's arm layer
      stops. Stagger has no clip in the rig: it reads through knockback + hit-stop (owner checklist).
    - `apply_hit_stop` (`Update`, `.after(drive_character_animation).after(HitStopSystems)`): per animator, if the
      character's `HitStop.left > 0` → `player.pause_all()`, else `player.resume_all()`. Query `Option<&HitStop>`
      (deliberate exception to With/Without: the plain visuals harness has no juice plugin). Register it in
      `CharacterVisualsPlugin`. Keep the file < 750 lines (356 now).

20. **Bat visuals — `assets/world/render.ron`, `src/visuals/config.rs`, `src/visuals/weapons.rs`, `src/visuals/mod.rs`.**
    `WeaponVisuals` gains `bat_size: (0.06, 0.06, 0.85)` and `bat_color: (0.55, 0.38, 0.2)` (validated like the other
    sizes/colours). `WeaponVisualAssets` gains `bat: Handle<Mesh>` (`Cuboid::from_size(bat_size)`) and `bat_material`.
    `show_held_gun` query adds `&mut Mesh3d`: gun held → gun mesh + gun material; `held == None && melee == Bat` → bat
    mesh + bat material, visible; otherwise hidden. The bat reuses the `HeldGun` entity, which already undoes the joint
    scale (`with_scale(1/scale)`), so `bat_size` is in metres; `HeldGun.barrel` stays gun-only metadata and VFX react
    only to `ShotFired` (never written while unarmed). Observer `visualize_bat_pickup` (`On<Add, BatPickup>`, child with
    bat mesh/material lifted by `pickups.lift`) and `show_available_bat_pickups` (mirror of the weapon one); register
    both in `visuals/mod.rs`. Bat grip placement (`hand_offset` is tuned for guns) is owner-run.

### Runtime QA

21. **`tools/qa/scenarios/t7.py` (new)**, reusing t6/t5 helpers (`Game`, `rows`, `player`, `dummies`, `teleport`,
    `aim_at`, `damage_numbers`, `game_state`, `log_errors`, `wait_chunks`, `ron_number`): fetch-check assets; seed 1,
    release, `--features dev`; read fist damages and bat damage from `melee.ron`.
    1. Player starts unarmed (`held` None, `melee` "Fists"). Middle dummy; teleport the player to its feet +
       (0, 0, 1.0); settle 1 s; `aim_at` its chest (feet + 1.0).
    2. Three `send_mouse_button("Left", 80)` presses 0.3 s apart (each lands in a running swing and is buffered; if
       latency pushes one past the 0.359 s swing end it lands in the 0.4 s combo window — same result). Screenshots at
       +0.45, +0.9, +1.2 s (≥ 0.15 s apart, TASK-007 lesson).
    3. Poll the dummy's reflected `HitReaction` (tolerant reader of `{"KnockedDown": {"left": ..}}`) until
       `KnockedDown`, deadline 2.0 s after the first click; assert health drop == 10 + 10 + 20 and at least one live
       `DamageNumber` whose value is in {10, 20} (liveness of melee numbers). Then poll until `Steady` within
       `knockdown + 1.0` s.
    4. Bat: teleport onto the `Bat pickup` row; assert `has_bat` and `melee == "Bat"`; `send_keys(["Digit1"])` →
       `"Fists"`; again → `"Bat"`. Teleport to another dummy (feet + (0,0,1.0)), aim, one click → poll `KnockedDown`,
       drop 25; screenshot.
    5. Assert `log_errors` empty; `shutdown`; write `summary.json` into `--out`.

## 3. Test plan

Classes: **C** = correctness, **L** = liveness. Every new gate is flipped RED by perturbing the named mechanism, then
restored GREEN; the implementer records the perturbation and both outcomes in the stage summary.

### 3.1 `advance_swing` unit table — `crates/gta_sim/src/combat/melee.rs` `#[cfg(test)]` (C)
Config parsed from the shipped `assets/combat/melee.ron` via `include_str!` + `ron` (line-ending independent), `dt = 1/64`,
"click at k" = `clicked = true` on tick k:
- (a) click at k=0: `false` for k = 0..7, `true` for k = 8..14, `false` for 15..22; at k=23 returns `false`,
  `swing == None`, `combo_left == 0.4` (exact: set, not decremented), `next_step == 1`.
- (b) clicks at k=0 and k=10 → at k=23 a swing with `step 1`, `elapsed 0`; its window k = 31..37; ends k = 46.
- (c) plus a click at k=30 → at k=46 `step 2`, `cfg.hit(Fists, 2).knockdown == true`, window 54..60, ends 69,
  `next_step == 0`.
- (d) single swing, no click until k=49 → new swing `step 0`; (d') click at k=48 → `step 1` (boundary pair 48/49).
- (e) click at k=24 → `step 1`.
- (f) `landed = true` at k=8 → k=9 returns `false`.
- (g) clicks at k=5 and k=6 (two in one swing) → exactly one queued swing: swing 2 at k=23, `swing == None` at k=46.
- (h) bat click at k=0: window k = 13..20, end k = 39.
- (i) `HitReaction::escalate`: stagger on `KnockedDown{0.5}` stays `KnockedDown{0.5}`; knockdown on `Staggered` →
  `KnockedDown{1.2}`.
Flip-RED: decrement `combo_left` on the end tick too (the ambiguous PLAN.md order) → (a) and (d') fail.

### 3.2 Headless integration — `crates/gta_sim/tests/melee.rs` (new, production `headless_app()`)
Setup per test: `settle`, `place_player(A + Y·float_height)` with `A = (-20, 0, 21)`, dummy by `spawn_dummy` at
`A + d·1.0`, `run_ticks(8)`; aim `set_aim(position, dummy_feet + Y·1.0)` (origin y 1.05, target y 1.0 ⇒ flat direction
= d); click = `set_action(|a| a.fire_requested = true)` before a tick; `T0` = the first tick after the click. A local
`Hits` helper (cursors over `DamageDealt` and `MeleeHit`, read after every single tick, like `common::Shots`).
Geometry: cast origin at chest height, target axis 1.0 m away, contact at sphere-centre distance 0.35 + 0.3 = 0.65 ⇒
travel 0.35 ≤ range 1.0; a target 2.0 m away needs travel 1.35 > 1.0 ⇒ out of reach.
- `hit_lands_only_in_window` (C): no `DamageDealt` in T0..T0+7, exactly one at T0+8 (damage 10, shooter = player,
  `headshot == false`, `shot` = the swing's attack id), health drop == 10 (unarmoured), and none after through T0+23.
  Second part: a fresh dummy 2.0 m away, swing started, dummy teleported to 1.0 m (Position + Transform) after tick
  T0+15 (elapsed 0.234 > 0.22) → no hit through the swing end. Positive control: teleport after T0+9 → one hit by
  T0+14 (one tick of slack for the post-teleport BVH refresh; the sharp tick lives in the standing case).
  Flip-RED: `active_from` 0.12 → 0.10 in `melee.ron` (hit moves to T0+7) — record.
- `third_combo_hit_knocks_down` (C): clicks before T0, T0+10, T0+30. Derived: hits at T0+8, T0+31, T0+54; reaction
  read right after the hit tick: `Staggered`, `Staggered`, `KnockedDown`; health drop 40; three `DamageDealt` with three
  distinct `shot` ids; `KnockedDown` at T0+130, `Steady` at T0+131 (1.2 − 76/64 = 0.0125 > 0; 1.2 − 77/64 < 0).
  Reach: two 2 m/s jabs → 1.0 + 2·0.177 = 1.354 m, travel 0.704 ≤ 1.0. Control `combo_resets_after_window`: third click
  at T0+46+27 (after swing 2 ends at T0+46 with no queue, n = 27 > 25.6) → `Staggered`, never `KnockedDown`.
  Flip-RED: `knockdown: true` → `false` on the third fist hit.
- `knockback_pushes_along_the_blow` (C), d = −Z, +X, +Z (fresh app each): one jab; 32 ticks after the hit,
  `Δ = dummy_after − dummy_before`: `Δ·d >= 0.1` and `|Δ − (Δ·d)d| < 0.02` (probe with `force_forward: None`: 0.177 m
  along d, 0.000 lateral; record the measured value under `Some(-d)`). +X is the sign/axis sentinel (an x/z swap or
  sign error fails at least one case). Flip-RED: negate `direction` in `apply_strikes`.
- `second_shove_during_knockback_is_applied` (C, wrapper): `knock_back` directly on a spawned dummy: 3 m/s along −Z,
  4 ticks, 5 m/s along −Z, 128 ticks → displacement along −Z > 0.8 m (probe 1.108 with reset, 0.363 without).
  Flip-RED: delete the memory reset.
- `real_strike_during_knockback_shoves_again` (C, integration of `apply_strikes` → `knock_back`): standing d = −Z case;
  after tick T0+3 call `knock_back(dummy, −Z · 2.0)`; record `v_before = LinearVelocity.z` after T0+7; the jab lands at
  T0+8; `v_after` after T0+8. Assert `(-v_after) − (-v_before) > 0.6` m/s. Probe (`scratch/probe_r2_overlap.log`,
  2 m/s then 2 m/s 4 ticks apart): speed 1.750 → 3.047 (+1.30) with the reset, 1.750 → 1.549 (−0.20) without.
  Flip-RED: remove the reset → RED; also RED if `apply_strikes` calls `action_interrupt` directly.
- `stagger_interrupts_the_victims_swing` (C): a second attacker = `spawn_dummy` at `A − Z` + `Loadout::default()`
  inserted, its `AimIntent` toward the player, its `fire_requested` raised before T0+1 (player's before T0). Derived:
  player hits it at T0+8 → `Staggered`; its own window would open at T0+9, but `swing_melee` cancels it → no
  `DamageDealt` with target = player through T0+40. Control: without the player's click the dummy-attacker hits the
  player at T0+9, damage 10.
- `stagger_blocks_gun_fire` (C): player holds a pistol (as `give` in shooting.rs), `HitReaction::Staggered{0.3}` set on
  the player, `fire_requested = true` → no `ShotFired` that tick and `fire_requested == false` after it; control after
  `Steady` → one `ShotFired`. Flip-RED: remove the `is_active()` continue in `fire_weapons`.
- `bat_hit_knocks_down_at_once` (C): spawn `BatPickup` at the player's feet → after one tick `has_bat`, `melee == Bat`;
  one click → hit at T0+13, damage 25, `KnockedDown`. Then `select = Some(WeaponRequest::Unarmed)` while unarmed →
  `melee == Fists`; again → `Bat`; with a pistol held, `Unarmed` → `held == None`, `melee` unchanged.
- `wall_blocks_the_punch` (C): `spawn_wall` 0.2 m thick midway (0.5 m) between attacker and dummy → no `DamageDealt`
  through the swing end, `swing.landed == true` after the window opens. Control: same without the wall → hit.
- `knocked_down_head_is_no_headshot` (C): finisher sequence, wait 32 ticks (still `KnockedDown`), give the pistol, shoot
  from 5 m at the dummy's current feet + 1.65 → `headshot == false` and one body hit; control on a `Steady` dummy →
  `headshot == true`. Flip-RED: drop the `is_knocked_down` term.
- `armour_absorbs_punch_first` (C): dummy `armor = 50` → one jab: `DamageDealt.damage == 10`, armour 40, health 100
  (existing message contract: damage before armour).
- `gun_shot_and_punch_have_distinct_shot_ids` (C): pistol shot then (unarmed) punch → two `DamageDealt` with different
  `shot`. Flip-RED: melee uses a private counter starting at the same value.
- `melee_state_is_reset_by_respawn` (C): player `HitReaction::KnockedDown{1.0}` and a running swing (click, 2 ticks),
  kill with `write_damage`, run through `Wasted` (loop as `input_raised_during_wasted_is_dropped`) → back in
  `Playing`: `Steady`, `swing == None`, `queued == false`; `has_bat` preserved. Flip-RED: unregister `reset_player_melee`.

### 3.3 Config gates — `crates/gta_sim/tests/config.rs` (C)
`shipped_melee_config_loads` (load + validate). Via `sabotaged::<MeleeConfig>` (first occurrence replaced):
- `"active_to: 0.22, knockback: 5.0"` → `"active_to: 0.40, knockback: 5.0"` (strictly past duration 0.35) → error
  contains `fists.hits[2].active_to`;
- empty hits, tested in code (a text sabotage of a RON list cannot empty it without a parse error): load the shipped
  config, `bat.hits.clear()`, `validate()` error contains `bat.hits`;
- `"cast_radius: 0.35"` → `"cast_radius: -0.35"` → error contains `cast_radius`;
- `"stagger: 0.35"` → `"stagger: 0.35, bogus_field: 1.0"` → error names file and `bogus_field`.
Each fixture yields a different error text (TASK-006 lesson).

### 3.4 Client — `cargo test -p gta_like --bin gta_like`
- `src/juice/shake.rs` unit tests (C): `smooth_noise` in [−1, 1] over 10 000 samples; continuous
  (`|n(t+1e-3) − n(t)| < 0.01`); channels 0/1/2 differ at the same t; `shake_rotation(0.0, ..) == Quat::IDENTITY`;
  at trauma 1 every Euler angle ≤ its `max_*_deg`.
- `src/juice/config.rs`: shipped `juice.ron` validates; `hit_stop_seconds = 0` → error names it.
- `src/visuals/character_gate.rs`: update the `CharacterClips` literal (`melee: [19, 20, 21], bat: 19, knockdown: 9`)
  and the `CharacterAnimator` literals (`action: None`). Split `character_visuals_app()` into
  `character_visuals_app_with(extra: impl FnOnce(&mut App))` (extra runs before `finish/cleanup`; the old name calls it
  with a no-op).
- New gate `hit_stop_freezes_only_the_pair_and_not_time` (C; presentation gate, client crate by law): app with
  `extra` = insert the shipped, validated `JuiceConfig` + `add_plugins(HitStopPlugin)` (the production plugin that
  `JuicePlugin` adds). Place the player at A, one dummy at A − Z (target), one bystander dummy 4 m away. Spawn three
  hand-built animators (`AnimationPlayer`, `AnimationTransitions`, `CharacterAnimator { character, .. }`) and start
  `nodes[Idle]` on each by hand (so each has an active node — never assert `all_paused()`). Aim and click; update one
  frame at a time reading `MeleeHit` with a cursor; `U` = the update in which it appears. Derived (`FixedTimesteps(1)`,
  real dt 15.625 ms after warm-up, order tick → start → apply): `left` = 0.05, 0.034375, 0.01875, 0.003125, 0 ⇒ every
  active animation of attacker and target `is_paused()` in U..U+3 and none paused in U+4; the bystander's active node is
  never paused. In every update U−1..U+4: `Time<Virtual>::relative_speed() == 1.0`, `!Time<Virtual>::is_paused()`, and
  `Time<Fixed>::elapsed()` grew by exactly one timestep (an expected clock fixed by the update count, independent of
  hit-stop). Flip-RED (record both): (1) `start_hit_stop` also calls `Time<Virtual>::set_relative_speed(0.0)` → time
  assertions RED; (2) unregister `apply_hit_stop` → pause assertions RED.
- `melee_actions_drive_full_body_clips` (L): with the same app, after the click the attacker's
  `transitions.get_main_animation() == fists[0]`; set the dummy `HitReaction::KnockedDown{1.0}` → its main animation
  `knockdown`; set `Steady` → main animation back to `nodes[Idle]`.

### 3.5 Runtime QA and owner run
- `python tools/qa/scenarios/t7.py --out <task scratch>/qa/t7` (step 21) passes; re-run `t6.py` (6 `WeaponPickup` rows,
  shooting unchanged). Screenshots are evidence, not the correctness gate.
- Owner checklist for `QA_REPORT.md` (Russian): `cargo run --release -- --seed 1`, тир в парке: три ЛКМ по манекену —
  удары читаются как комбо (правый, левый, пинок), заморозка на ударе ощутима, но короткая, добивающий сбивает с ног,
  манекен встаёт; тряска заметна на ударах и не тошнит; бита подбирается, клавиша 1 переключает кулаки/бита, удар битой
  тяжелее; бита в руке выглядит сносно. Рычаги: `melee.ron` (окна, отброс, stagger/knockdown), `juice.ron`
  (`hit_stop_seconds`, `shake`), `visual.ron` (клипы), `render.ron` (размер/цвет биты). Feel, clip choice, shake comfort
  and bat look are owner-gated only.

### 3.6 Final verification
`cargo build`, `cargo clippy -- -D warnings`, `cargo test -p gta_sim`, `cargo test -p gta_like --bin gta_like`,
`t6.py`, `t7.py`, `python tools/qa/tree_check.py` and `cargo tree -p gta_sim -e features -i bevy_render` (must stay
empty — no crate added to `gta_sim`). File sizes: `melee.rs`, `character_gate.rs`, `tests/melee.rs` each < 750 lines
(split `tests/melee.rs` helpers into `tests/common` only if a second test file needs them).

Order: steps 1-4 + 3.1 → `cargo test -p gta_sim --lib`; steps 5-13 → `cargo build`, 3.2-3.3 green with flip-RED
recorded; steps 14-20 → 3.4 green with flip-RED recorded, clippy; step 21 → 3.5.

## 4. Rollout notes

- No migrations, env vars or feature flags. New data files/fields: `assets/combat/melee.ron` (new, required at
  startup: `compose_sim` fails with a file+field error if missing or invalid), `juice.ron` (`hit_stop_seconds`,
  `shake`), `visual.ron` (`fists`, `bat`, `knockdown`), `render.ron` (`weapons.bat_size`, `weapons.bat_color`). All
  loaders are `deny_unknown_fields`: the files and the code land in one commit.
- Behaviour change for existing play: an unarmed LMB now punches (before it did nothing). `DamageDealt.shot` ids now come
  from the shared `AttackSerial` (still unique and wrapping; damage numbers and hit markers key on it unchanged).
  Characters in stagger/knockdown cannot fire guns.
- BRP: `Melee`, `Swing`, `MeleeWeapon`, `HitReaction`, `MeleeHit`, `BatPickup` registered for reflection; `Loadout`
  gains `melee` and `has_bat` (t6.py reads `held`/`guns` only — unaffected).
- `gta_sim` gains no dependency; `bevy_render` isolation unchanged.
- Known limits recorded for T9: same-tick strikes on one victim do not sum shoves; frame advantage (stagger 0.35 s vs
  the next jab window) lets an NPC victim escape between jabs — data tweak later.

## 5. Review notes

Disconfirmation tested first: "if `fire_weapons` consumed LMB only with a gun held, melee input ownership and the
system order are wrong" — did not hold (`hitscan.rs:150-155` takes the flag before `held`). Second counterexample: the
tick numbers T0+8/31/54/131 depend on an ambiguous end-of-swing branch in PLAN.md step 4 — recomputed under the explicit
order of step 4 above; they hold, and the boundary pair k=48/49 is now in the unit table.

Changes versus PLAN_V2 (V2 corrections kept; detail restored from PLAN.md where V2 dropped it without correcting it —
exact RON, types, signatures, system order, worked numbers, per-gate flip-RED):
1. `advance_swing` order written as normative pseudo-code (V2 diagnosis, recomputed prescription): the end tick sets
   `combo_left = combo_window` and returns without a decrement; derived tables added for bat and the grace boundary.
2. V2's "real two-strike overlap gate" with two attackers replaced by `real_strike_during_knockback_shoves_again`: one
   wrapper shove plus one real jab, asserting a velocity rise measured by a new probe (+1.30 vs −0.20 m/s). A final
   displacement threshold was rejected (0.270 vs 0.177 m, thin margin). Evidence: `scratch/probe_r2_overlap.log`.
3. V2 step 8 "on dummy revival clear swing/reaction" dropped: dummies carry no `Loadout` and never swing, and
   `recover_from_hits` ticks `Dead` characters, so a reaction cannot outlive a revival for long; YAGNI.
4. V2 "gate bat mesh visibility" in the client harness dropped: it needs the whole `VisualsPlugin` asset stack, and a
   wrong bat mesh is visible on the first frame — owner run + t7 screenshot, per project evidence rule.
5. V2 gun suppression during stagger kept and made concrete (`is_active()` continue after taking the flag) with its own
   gate `stagger_blocks_gun_fire`.
6. V2 armour note kept as `armour_absorbs_punch_first`; the existing "damage before armour" contract is not changed.
7. Swing cancellation made explicit (held gun without taking the click; reaction with dropping it; weapon toggle).
8. Hit-stop gate hardened: animators must have a started node, assertions use `is_paused()` on active nodes; V2's
   warm-up/`Time<Fixed>` counting kept. A PCTX proposal records the `all_paused()`-on-empty trap.
9. Empty-hits fixture moved from a text sabotage to an in-code mutation (a text sabotage of a RON list either fails to
   parse differently or needs an unknown field that masks the rule).
10. PLAN.md line references to `weapons.rs`, `input/mod.rs`, `camera/mod.rs`, `flow/wasted.rs` were stale; the final
    plan cites symbols (and re-verified line numbers where given).
11. Open questions Q1-Q3 are resolved by TASK_FINAL and not reopened.

children: 0 launched / 0 reported.
