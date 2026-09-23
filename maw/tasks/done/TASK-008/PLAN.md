# PLAN — TASK-008 (GDD T7): melee combat

Pinned versions (from `Cargo.lock`, sources under `~/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/`):
bevy / bevy_ecs / bevy_animation / bevy_time 0.19.1, avian3d 0.7.0, bevy-tnua 0.32.0 (+ bevy-tnua-macros 0.3.0),
vendored `vendor/bevy-tnua-avian3d-0.12.1`. **No new crate and no `Cargo.lock` change**: everything below uses crates
already in the lock (checked: `TnuaBuiltinKnockback` ships inside bevy-tnua 0.32.0, `src/builtins/knockback.rs`).

Executable planner probe (scratch copy of the workspace, own target dir, nothing in the checkout touched):
`scratch/probe_ws/crates/gta_sim/tests/probe_knockback.rs`, output `scratch/probe_knockback.log` and
`scratch/probe_second_shove.log`. It adds `Knockback(TnuaBuiltinKnockback)` to `CharacterScheme` and
`knockback: Default::default()` to `tnua_config()` — that change **compiles and runs** against the pinned crates.

Cost of error. Silent defects get headless gates: a hit landing outside its window, a combo that never knocks down,
knockback pushing the wrong way (sign error), a second shove silently dropped by Tnua, a hit-stop that freezes the
simulation clock, melee state leaking through `Wasted`, a knocked-down head still giving headshots, damage shown !=
damage applied. "Хлёсткость", shake comfort, clip choice, bat look: owner's run (checklist), BRP screenshots as evidence.

---

## 1. Understanding (what exists today)

**Sim (`crates/gta_sim`)**
- `lib.rs:26-85` `compose_sim`: loads and validates every RON config (`load_config` + `validate`), inserts them as
  resources **before** `add_plugins`, then adds `FlowPlugin, PhysicsPlugins, TnuaAvian3dPlugin(FixedUpdate),
  CharacterPlugin, WorldPlugin, PlayerPlugin, CombatPlugin{seed}, WantedPlugin`.
- `character/mod.rs:22-26` `CharacterScheme` = `TnuaBuiltinWalk` basis + `Jump(TnuaBuiltinJump)`;
  `:28-31` `Character` requires `MoveIntent, AimIntent, ActionIntent, JumpBuffer, AnimState`;
  `:51-63` `CharacterControlConfig::from_world` builds the Tnua config asset from `LocomotionConfig::tnua_config()`
  (`locomotion.rs:102-120`, the only caller); `:116-140` `character_components` (dynamic capsule r 0.3 h 1.5,
  `CollisionLayers(Character, ALL)`, child head sensor on `Hitbox`); `:142-201` `drive_characters` in
  `TnuaUserControlsSystems` — zeroes the walk basis for `Dead`, else walk + jump feeding, faces aim yaw when aiming.
- `character/health.rs`: `Health::take` (armour first, returns "killed on this hit"), `Dead` marker,
  `HealthSystems::{Damage, Regen, Pickup, Death}` chained in `FixedUpdate` (`character/mod.rs:80-89`).
- `combat/mod.rs:27-65` `CombatPlugin`: messages `ShotFired/BulletTrace/DamageDealt`; `FixedUpdate`
  `(tick_loadouts, fire_weapons).chain().in_set(HealthSystems::Damage)`, pickups in `Pickup`, `dummy_life` in
  `Death`, all `.in_set(PlayingSystems)` (runs only in `GameState::Playing`, `flow/mod.rs:362-365`).
- `combat/weapons.rs:289-299` `Loadout { held: Option<Weapon>, guns: [GunSlot; 3], reload_left, spread_deg }`;
  `:413-431` `select_weapon` (`WeaponRequest::Unarmed` → `held = None`); `:307-321` `cycle_weapon` ring `[None, guns]`.
- `combat/hitscan.rs:123-261` `fire_weapons`: **takes `ActionIntent.fire_requested` for every character with a
  `Loadout`, before it checks `held`** (`:152-155`); counts trigger pulls in a `Local<u32>` → `DamageDealt.shot`
  (`:146,182`); skips the head sensor of a `Dead` body (`:185-188`).
- `combat/range.rs`: `Dummy` + `dummy_bundle` (a full `Character` with `Health`, no `Loadout`), `spawn_range` on
  `OnTransition{Loading→Playing}` in the city only (dummies 10 m along −Z of the park centre, 6 gun/ammo pickups in a
  row through it); `dummy_life` kills at 0 HP and revives after `range.dummy_reset`.
- `combat/pickups.rs:137-161` `collect_weapon_pickups` (radius / respawn from `weapons.ron` `pickups`).
- `flow/wasted.rs:484-520` on `OnExit(Wasted)`: `respawn_player`, `drop_queued_damage`, `drop_queued_input`
  (the TASK-006/007 leak lessons).
- Tests: `tests/common/mod.rs` (`headless_app` = MinimalPlugins + TransformPlugin + AssetPlugin + StatesPlugin +
  `compose_sim(TestArea)`, `TimeUpdateStrategy::FixedTimesteps(1)`, `run_ticks`, `spawn_dummy`, `set_aim`,
  `set_action`, `Shots` message cursors); `tests/shooting.rs` (T6 gates), `tests/config.rs` (`sabotaged` helper).

**Client (`src/`)**
- `input/mod.rs:343,430-438`: LMB = `Fire`; every press sets `ActionIntent.fire_requested`, whatever is held.
  Keys 1-4 = `Unarmed / Pistol / Smg / Shotgun` (`:443-454`). No input change is needed for punches.
- `juice/mod.rs`: `CameraRecoil` (pitch, real-time decay) + `DamageNumbersPlugin`; `juice/damage_numbers.rs` spawns
  one label per `(shooter, shot, target)` of every **player** `DamageDealt` — melee hits appear there for free as
  long as they write `DamageDealt` with a unique `shot`. `hud/weapon.rs:164-188` hit marker also reads `DamageDealt`.
- `camera/mod.rs:277-343` `follow_player` (PostUpdate): writes `AimIntent` from the camera **before** applying the
  visual recoil rotation (`:339-342`) — the same slot takes the shake.
- `visuals/character.rs`: shared `AnimationGraph` with full-body locomotion `nodes`, arm-masked `legs`, arm layer
  `hold/shoot`; `CharacterAnimator { character, shown, armed, arms }` on each model's `AnimationPlayer`;
  `drive_character_animation` (Update). `visuals/character_config.rs` resolves clip names against the manifest rig.
  `visuals/character_gate.rs` = headless presentation harness (`character_visuals_app`: MinimalPlugins + asset
  stand-ins + `compose_sim` + `CharacterVisualsPlugin`, animators spawned by hand).
- `visuals/weapons.rs`: `HeldGun` box on the hand joint, material per held gun, hidden when `held == None`.
- Rig clips (`assets/third_party/manifest.ron:81-87`, glTF order): 1 idle … 9 **die** … 19 **attack-melee-right**,
  20 **attack-melee-left**, 21 **attack-kick-right**, 22 **attack-kick-left**. No hit/stagger clip exists.
- `tools/qa/scenarios/t6.py`: teleport, `aim_at` via `move_mouse`, `send_mouse_button`, reads `Loadout.held`
  (reflect format of `Option<Weapon>`) and asserts **exactly 6** `WeaponPickup` rows.

**Verified engine facts this plan relies on**
- `TnuaBuiltinKnockback { shove: Vec3, force_forward: Option<Dir3> }` (`bevy-tnua-0.32.0/src/builtins/knockback.rs:32-47`),
  config `TnuaBuiltinKnockbackConfig { no_push_timeout, barrier_strength_diminishing, acceleration_limit,
  air_acceleration_limit }` (`:50-74`, derives `Deserialize` but not `deny_unknown_fields`). The shove is applied once
  in memory state `Shove`, then `Pushback` limits braking (`:113-143`).
- `TnuaScheme` derive: variant `Knockback(..)` ⇒ config field `knockback` (snake case, `bevy-tnua-macros-0.3.0/src/scheme_derive/parsed.rs:147`),
  action-state enum `CharacterSchemeActionState::Knockback(TnuaActionState { input, config, memory })`
  (`gen_action_state.rs`, `action_state.rs:12-20`, all fields `pub`; `TnuaController.current_action` is `pub`,
  `controller.rs:189`).
- **Pitfall (probed):** `action_interrupt` on the action that is already running only swaps `state.input`
  (`update_in_action_state`, `codegen.rs:88-104`); memory stays `Pushback`, the new shove is never applied. Probe:
  3 m/s then 5 m/s 4 ticks later → end displacement 0.363 m (= first shove alone) without a reset, 1.108 m with
  `state.memory = TnuaBuiltinKnockbackMemory::Shove` first (`scratch/probe_second_shove.log`). A PCTX proposal is filed.
- Slide per shove on the test floor, standing dummy, default knockback config (`scratch/probe_knockback.log`,
  identical for −Z / +X / +Z): 1.0 m/s → 0.057 m; 1.5 → 0.109; **2.0 → 0.177 m** (action finished tick 11);
  **3.0 → 0.363 m** (tick 14-15); 4.5 → 0.763 m (tick 18-21); 6.0 → 1.309 m (tick 23). No vertical motion.
- `action_interrupt` needs no `initiate_action_feeding` (only `action()` asserts it, `controller.rs:296-301`);
  `drive_characters` calls `initiate_action_feeding()` every tick for every character, so an interrupt raised before
  `TnuaUserControlsSystems` starts in the same tick.
- avian 0.7 `SpatialQuery::cast_shape(shape, origin, rot, dir, &ShapeCastConfig, &SpatialQueryFilter)` returns the
  closest `ShapeHitData { entity, distance, point1, point2, .. }` (`spatial_query/system_param.rs:446-600`); with the
  default `ignore_origin_penetration: false` a shape starting inside a collider hits it at distance 0 — the attacker's
  own capsule must be excluded (`with_excluded_entities`).
- bevy_animation 0.19.1: `advance_animations` skips paused animations (`lib.rs:1031-1062`, `!active_animation.paused`),
  runs on `Time` (virtual) in PostUpdate; `AnimationPlayer::pause_all/resume_all` (`:922,931`),
  `ActiveAnimation::is_paused` (`:630`), `AnimationClip::duration` (`:254`).
- bevy_time 0.19.1 `TimeUpdateStrategy::FixedTimesteps(n)` advances `Time<Real>` by `timestep × n` per update
  (`lib.rs:181-183`) — the client gate gets a deterministic real clock of 15.625 ms per update.

## 2. Approach

**Sim owns every rule, windows are data, not animation events** (GDD §4.2). One new file
`crates/gta_sim/src/combat/melee.rs` holds the config, the attacker state, the victim state and the systems.

- **Selection.** `Loadout.held == None` means "melee"; `Loadout` gains `melee: MeleeWeapon` (`Fists | Bat`) and
  `has_bat: bool`. Key 1 selects melee; pressing it again while in melee toggles fists ↔ bat when a bat is owned.
  The wheel ring stays `[None, guns]` (None = the current melee weapon). Rejected `Weapon::Bat`: `guns[3]` indexing,
  `WeaponsConfig::stats`, hitscan, juice/audio `PerWeapon`, HUD and t6.py's reflected `held` all assume a gun.
- **Attacker state** `Melee { swing: Option<Swing>, next_step, combo_left, queued }`, `Swing { weapon, step,
  elapsed, duration, direction, landed, attack }`. A pure function `advance_swing` runs the state machine (start /
  buffer a click / advance / window open? / end → queued next step or open the combo window). Clicks during a swing
  are buffered (one), clicks during stagger/knockdown are dropped (same as the gun cooldown rule).
- **Hit test.** In the open window, every tick until the first hit: sphere `cast_radius` from
  `feet + Y·cast_height`, along the swing direction, `max_distance = weapon range`, mask `[World, Character]`
  (named explicitly — TASK-007 lesson; head sensors on `Hitbox` stay out), attacker excluded. First hit on a live
  `Character` = strike; a wall hit = miss.
- **Victim state** `HitReaction` enum component (`Steady | Staggered { left } | KnockedDown { left }`), a field
  change per hit, never an archetype move. Knockdown overrides stagger, stagger never shortens a knockdown.
  Stagger/knockdown cancel the victim's own swing on its next step and zero its walk basis; knockdown also switches
  off its head sensor for hitscan (GDD §4.1).
- **Knockback** through `TnuaBuiltinKnockback` via `action_interrupt`, resetting the memory of an already running
  knockback to `Shove` (probe above). `force_forward = -direction` so the victim faces the attacker (the knockdown
  clip falls away from the hit).
- **Two systems + one private message** instead of one ParamSet system: `swing_melee` (attackers, writes
  `Strike`) → `apply_strikes` (victims: `Health::take`, reaction, knockback, writes `DamageDealt` and the public
  `MeleeHit`). No aliasing between `&HitReaction` (attacker, read) and `&mut HitReaction` (victim).
- **Damage numbers / hit marker for melee** come from writing the existing `DamageDealt`. Its `shot` id is shared
  with guns through a new `AttackSerial` resource (replaces `fire_weapons`' `Local<u32>`), so a gun shot and a punch
  can never merge into one number.
- **Presentation (client).** `MeleeHit` (buffered `Message`, read by two consumers) drives: (a) local hit-stop — a
  `HitStop { left }` component on attacker and target, ticked on `Time<Real>`, applied by pausing that character's
  `AnimationPlayer` (never `Time<Virtual>`, GDD §8); (b) trauma — `CameraShake` resource, `trauma += 0.25` when the
  player is attacker or target, `shake = trauma²`, decay 1.2/s, smooth value noise, **rotational** offsets applied
  after `AimIntent` is written (visual only; rotational shake is Eiserloh's 3D recommendation and cannot push the
  camera through a wall the collision cast just cleared). Swing clips and the knockdown `die` clip are chosen from
  sim state (`Melee.swing`, `HitReaction`), bat is a primitive box like the guns.

Research used: attack phases startup/active/recovery and hit-stun vs recovery as "frame advantage"
([CAPCOM Shadaloo C.R.I. "Hour 9"](https://game.capcom.com/cfn/sfv/column/131432?lang=en),
[critpoints: Frame data patterns](https://critpoints.net/2023/02/20/frame-data-patterns-that-game-designers-should-know/));
hit-stop freezes attacker and victim only, the world keeps moving ([SmashWiki: Hitlag](https://www.ssbwiki.com/Hitlag));
trauma² shake with Perlin-like noise, rotational in 3D ([Eiserloh, GDC 2016 slides](http://www.mathforgameprogrammers.com/gdc2016/GDC2016_Eiserloh_Squirrel_JuicingYourCameras.pdf));
`game-feel` skill: shake the camera not the body, trigger hit-stop once per impact, let input buffer through it.

## 3. Steps

### Sim — data and types

1. **`assets/combat/melee.ron` (new).** Every value is tuning (owner-facing), so none of them is a `const`:
   ```ron
   (
       cast_radius: 0.35,        // m, sphere swept from the chest (GDD §4.2)
       cast_height: 1.3,         // m above the feet
       combo_window: 0.4,        // s after a swing ends in which a click continues the combo
       swing_move_scale: 0.0,    // share of the gait speed kept while swinging (0 = feet planted)
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
   `knockback: 2.0` for jabs is below the GDD "3-6 м/с" band on purpose (probe: 3 m/s slides 0.363 m; two jabs from
   1.0 m push the target to 1.73 m, past the 1.65 m fist reach = range 1.0 + sphere 0.35 + capsule 0.3, so the
   finisher misses; 2 m/s gives 1.35 m). Finisher 5.0 and bat 6.0 stay in the band. See Open question Q1.
   `knockback_tuning` = the Tnua library defaults, now owned by our file (they shape the slide distance).

2. **`crates/gta_sim/src/combat/melee.rs` (new) — config.** `pub const MELEE_CONFIG: &str = "combat/melee.ron"`.
   `#[derive(Resource, Deserialize, Clone, Debug)] #[serde(deny_unknown_fields)] pub struct MeleeConfig { cast_radius,
   cast_height, combo_window, swing_move_scale, stagger, knockdown: f32, fists: MeleeWeaponStats, bat:
   MeleeWeaponStats, knockback_tuning: KnockbackTuning }`; `MeleeWeaponStats { range: f32, hits: Vec<MeleeHitStats> }`;
   `MeleeHitStats { damage: u32, duration, active_from, active_to, knockback: f32, knockdown: bool }` (damage is an
   integer: `DamageDealt.damage` is exactly what `Health::take` gets, no rounding); `KnockbackTuning` (4 fields,
   `deny_unknown_fields`) with `fn tnua(&self) -> TnuaBuiltinKnockbackConfig` (strict wrapper, same pattern as
   `LocomotionConfig`, GDD §3.1). `fn stats(&self, MeleeWeapon) -> &MeleeWeaponStats`, `fn hit(&self, weapon, step)
   -> &MeleeHitStats` (`hits[step % hits.len()]`).
   `validate()` (error names the field, e.g. `fists.hits[1].active_to`): all floats finite; `cast_radius, cast_height,
   stagger, knockdown, range > 0`; `combo_window >= 0`; `0 <= swing_move_scale <= 1`; `hits` non-empty; per hit
   `damage >= 1`, `0 <= active_from < active_to < duration`, `knockback >= 0`; knockback tuning values `> 0`.

3. **`melee.rs` — types.**
   - `#[derive(Reflect, Default, Clone, Copy, Debug, PartialEq, Eq, Deserialize)] pub enum MeleeWeapon { #[default] Fists, Bat }`.
   - `#[derive(Component, Reflect, Default, Clone, Debug)] #[reflect(Component, Default)] pub struct Melee { pub swing:
     Option<Swing>, pub next_step: u8, pub combo_left: f32, pub queued: bool }` (`///` doc per field).
   - `#[derive(Reflect, Clone, Copy, Debug, PartialEq)] pub struct Swing { pub weapon: MeleeWeapon, pub step: u8, pub
     elapsed: f32, pub duration: f32, pub direction: Vec3, pub landed: bool, pub attack: u32 }` — `duration` is copied
     from the config at start so presentation reads derived state, not `melee.ron` (GDD §12 "one owner").
   - `#[derive(Component, Reflect, Default, Clone, Copy, Debug, PartialEq)] #[reflect(Component, Default)] pub enum
     HitReaction { #[default] Steady, Staggered { left: f32 }, KnockedDown { left: f32 } }` with `fn is_active()`,
     `fn is_knocked_down()`, `fn escalate(&mut self, knockdown: bool, cfg: &MeleeConfig)` (KnockedDown always sets
     `left = cfg.knockdown`; Staggered sets `left = cfg.stagger` unless currently KnockedDown).
   - `#[derive(Message, Reflect, Clone, Copy, Debug)] #[reflect(Message)] pub struct MeleeHit { pub attacker: Entity,
     pub target: Entity, pub point: Vec3, pub knockdown: bool }` — a landed, applied hit (juice input).
   - `#[derive(Message, Clone, Copy, Debug)] pub(super) struct Strike { attacker, target, point, direction: Vec3,
     damage: u32, knockback: f32, knockdown: bool, attack: u32 }`.
   - (`BatPickup` lives in `pickups.rs`, step 9.)

4. **`melee.rs` — `advance_swing` (pure, unit-tested).** Signature
   `pub fn advance_swing(melee: &mut Melee, clicked: bool, weapon: MeleeWeapon, direction: Vec3, attack: impl FnOnce() -> u32, dt: f32, cfg: &MeleeConfig) -> bool`
   (returns "window open this tick and not landed yet"). Exact order (test numbers below are derived from it):
   1. If a swing runs: `queued |= clicked`; `elapsed += dt`; if `elapsed >= duration` → end it: `next_step =
      (step + 1) % hits.len()`, `combo_left = cfg.combo_window`, `swing = None`, and `start = take(queued)`;
      else return `window(swing)`.
   2. If no swing ran this tick: `combo_left = (combo_left - dt).max(0.0)`; `start = clicked`.
   3. If `start`: `step = if combo_left > 0 { next_step % hits.len() } else { 0 }`, `swing = Some(Swing { elapsed: 0,
      duration: hit.duration, direction, landed: false, attack: attack(), .. })`; return `window(swing)`.
   4. `window(s) = !s.landed && hit.active_from <= s.elapsed && s.elapsed <= hit.active_to`.
   `fn cancel(&mut self)` on `Melee`: `swing = None, queued = false, next_step = 0, combo_left = 0`.
   `elapsed` accumulates `dt = 1/64` exactly (binary-exact), so tick `T0 + k` has `elapsed = k/64`:
   window 0.12..0.22 ⇒ k = 8..14; swing end at k = 23 (23/64 = 0.359 ≥ 0.35).

5. **`melee.rs` — systems** (all `FixedUpdate`, `Res<Time<Fixed>>`):
   - `recover_from_hits(Query<&mut HitReaction>)`: subtract `dt` from `left`, `Steady` at `<= 0`. All characters,
     dead ones included (a revived dummy must not stay down forever).
   - `swing_melee`: query `(Entity, &Position, &Rotation, &CharacterBody, &AimIntent, &mut ActionIntent, &Loadout,
     &mut Melee, &HitReaction)`, `(With<Character>, Without<Dead>)`; `SpatialQuery`, `ResMut<AttackSerial>`,
     `MessageWriter<Strike>`, `Res<MeleeConfig>`, `Query<&ColliderOf>`, `Query<(), (With<Character>, With<Health>)>`.
     Per attacker (let-else, early `continue`):
     `if loadout.held.is_some() || reaction.is_active() { melee.cancel(); continue }` — the gun path keeps its own
     `fire_requested` (must not be taken here); a staggered click is dropped: `action.fire_requested = false`.
     `clicked = take(&mut action.fire_requested)`; `direction` = horizontal `AimIntent.direction` normalized, else
     body forward `Rotation * NEG_Z` flattened (only used on start).
     `if !advance_swing(..) { continue }`; cast `Collider::sphere(cast_radius)` from
     `position - Y·float_height + Y·cast_height` along `swing.direction`, `ShapeCastConfig::from_max_distance(range)`,
     `SpatialQueryFilter::from_mask([GameLayer::World, GameLayer::Character]).with_excluded_entities([attacker])`.
     Resolve `ColliderOf.body` (fallback: the hit entity); if it is not a live `Character` with `Health` (a wall, a prop) →
     `landed = true` and no strike (the swing is spent on the wall, like a bullet; gate `wall_blocks_the_punch`).
     On a character: `landed = true`, write `Strike { point: hit.point1, direction, damage,
     knockback, knockdown, attack: swing.attack }` from `cfg.hit(swing.weapon, swing.step)`.
   - `apply_strikes`: `MessageReader<Strike>`, query `(&mut Health, &mut HitReaction, &mut TnuaController<CharacterScheme>)`
     `Without<Dead>`; skip `health.current <= 0` (killed earlier this tick, `Dead` still deferred — same rule as
     `hitscan.rs:235`). `killed = health.take(damage as f32)`; `reaction.escalate(..)`; `knock_back(&mut controller,
     direction * knockback)`; write `DamageDealt { shooter: attacker, shot: attack, target, point, damage, headshot:
     false, killed }` and `MeleeHit { attacker, target, point, knockdown }`.
   - `pub fn knock_back(controller: &mut TnuaController<CharacterScheme>, shove: Vec3)`: `if let Some(CharacterSchemeActionState::Knockback(state)) =
     controller.current_action.as_mut() { state.memory = TnuaBuiltinKnockbackMemory::Shove; }` then
     `controller.action_interrupt(CharacterScheme::Knockback(TnuaBuiltinKnockback { shove, force_forward:
     Dir3::new(-shove).ok() }))`. Comment (non-obvious invariant): "an interrupt on a running knockback only swaps
     its input; without the reset the new shove is dropped". Skip when `shove == 0`.
   - `reset_player_melee` on `OnExit(GameState::Wasted)`: player's `Melee::default()` + `HitReaction::Steady` (a
     knockdown or buffered swing from before death must not continue at the hospital; TASK-006/007 leak class).

6. **`crates/gta_sim/src/combat/mod.rs`.** `mod melee;` + `pub use melee::{HitReaction, MELEE_CONFIG, Melee,
   MeleeConfig, MeleeHit, MeleeWeapon, Swing, advance_swing, knock_back}`; `pub use pickups::BatPickup`
   (define `BatPickup` in `pickups.rs`, not in `melee.rs`).
   `#[derive(Resource, Default)] pub struct AttackSerial(pub u32)` with `fn next(&mut self) -> u32` (wrapping) —
   "id of one attack (trigger pull or swing); `DamageDealt.shot`". Plugin: `init_resource::<AttackSerial>()`,
   `add_message::<MeleeHit>()`, `add_message::<melee::Strike>()`, `register_type` for `Melee, Swing, MeleeWeapon,
   HitReaction, MeleeHit, BatPickup`; `OnExit(GameState::Wasted)` → `melee::reset_player_melee`. `FixedUpdate`:
   ```rust
   (
       weapons::tick_loadouts,
       (melee::recover_from_hits, melee::swing_melee, melee::apply_strikes).before(TnuaUserControlsSystems),
       hitscan::fire_weapons,
   ).chain().in_set(HealthSystems::Damage)
   ```
   (melee before `fire_weapons`: `fire_weapons` takes `fire_requested` for every `Loadout` before checking `held`;
   before `TnuaUserControlsSystems`: the victim's zeroed basis and the knockback interrupt act in the hit tick),
   `pickups::collect_bat_pickups` in `HealthSystems::Pickup`.

7. **`crates/gta_sim/src/combat/hitscan.rs`.**
   - `fire_weapons`: replace `mut pulls: Local<u32>` with `mut serial: ResMut<AttackSerial>`, `let shot = serial.next()`
     per trigger pull. Only `CombatPlugin` registers this system (grep: no test harness adds it directly).
   - GDD §4.1 "в knockdown сенсор головы отключается": replace `dead: Query<(), With<Dead>>` with
     `down: Query<(Has<Dead>, &HitReaction)>` and `visible = of.body != shooter && !(head && body_is_down(of.body))`
     where down = `Dead` or `HitReaction::is_knocked_down()`.
   - `DamageDealt.shot` doc: "Attack (trigger pull or melee swing) that dealt it".

8. **`crates/gta_sim/src/combat/weapons.rs`.** `Loadout` gains `pub melee: MeleeWeapon` ("what empty hands swing") and
   `pub has_bat: bool` (Default: Fists / false; survives death like the guns, GDD §3.4). `select_weapon`:
   `WeaponRequest::Unarmed => { if held.is_none() && loadout.has_bat { loadout.melee = other(loadout.melee) } None }`
   (compute `held` from the loadout's pre-request value; keep `reload_left = 0` rule). Update `WeaponRequest::Unarmed`
   doc in `character/intent.rs` ("melee slot; again toggles fists/bat"). `cycle_weapon` unchanged.

9. **`crates/gta_sim/src/combat/pickups.rs`.** `BatPickup { cooldown }` (+`available`), `collect_bat_pickups`
   mirroring `collect_weapon_pickups` with `WeaponsConfig.pickups.{radius,respawn}` (shared pickup rule, no new value):
   on contact, if `!has_bat` → `has_bat = true`, and if `held.is_none()` → `melee = Bat`; cooldown only when taken.
   Separate component (not a `WeaponPickup` variant): t6.py asserts exactly 6 `WeaponPickup` rows.

10. **`crates/gta_sim/src/combat/range.rs`.** In `spawn_range`, one `BatPickup` + `Name::new("Bat pickup")` at
    `park_center + Vec3::Z * range.pickup_spacing` (behind the gun row, away from the dummies; ≥ 2.2 m from every gun
    pickup, so t6 teleports do not collect it).

11. **`crates/gta_sim/src/character/mod.rs`.**
    - `CharacterScheme`: add `Knockback(TnuaBuiltinKnockback)` (compiles — probe).
    - `Character` `#[require(.., HitReaction, Melee)]` (every character can be hit; the attacker side only acts with a
      `Loadout`).
    - `CharacterControlConfig::from_world`: `world.resource::<LocomotionConfig>().tnua_config(world.resource::<MeleeConfig>().knockback_tuning.tnua())`.
    - `drive_characters`: add `Res<MeleeConfig>` and `(&HitReaction, &Melee)` to the query. `dead || reaction.is_active()`
      → the existing zero-basis branch (no jump, buffer cleared). While `melee.swing` is `Some(s)`: `desired_motion *=
      cfg.swing_move_scale`, `desired_forward = Dir3::new(s.direction)` (faces the swing), no jump feeding.
      Only `CharacterPlugin` registers it; every harness goes through `compose_sim`, which inserts `MeleeConfig`.

12. **`crates/gta_sim/src/character/locomotion.rs`.** `tnua_config(&self, knockback: TnuaBuiltinKnockbackConfig)`
    fills `knockback` of `CharacterSchemeConfig` (the only caller is step 11).

13. **`crates/gta_sim/src/lib.rs`.** Load + validate `MeleeConfig` like the others, `insert_resource(melee)` before
    `add_plugins` (`CharacterControlConfig::from_world` reads it during `CharacterPlugin::build`).

### Sim — gates (`cargo test -p gta_sim`)

14. **`crates/gta_sim/src/combat/melee.rs` `#[cfg(test)]`** (config parsed from the shipped `melee.ron` via
    `include_str!`, like `anim_state_table`): `advance_swing` tables with `dt = 1/64`:
    (a) click at k=0: window false for k = 0..7, true for k = 8..14, false 15..22, swing gone at k=23, `combo_left =
    0.4`; (b) clicks at k=0 and k=10 → second swing starts at k=23 with `step 1`; (c) third click during swing 2 →
    swing 3 has `step 2` and its stats have `knockdown: true`; (d) no click until the combo window expired
    (k = 23 + 26 = 49: 0.4 − 26/64 < 0) → next swing `step 0`; (e) a click at k=24 (inside the window) → `step 1`;
    (f) `landed` closes the window; (g) `HitReaction::escalate`: stagger does not shorten knockdown, knockdown
    replaces stagger. Class: correctness of the state machine.

15. **`crates/gta_sim/tests/melee.rs` (new)**, production composition (`headless_app`), attacker = player unarmed at
    feet `A = (-20, 0, 21)`, dummy (`spawn_dummy`) at `A + d·1.0`, `settle` + 8 ticks, aim with
    `set_aim(body_centre, target_feet + Y·1.0)`; the click is `set_action(|a| a.fire_requested = true)`; `T0` = the
    first single tick after it. A local `Hits` helper (cursors over `DamageDealt` and `MeleeHit`, like `common::Shots`).
    Geometry (worked): cast origin `(-20, 1.3, 21)`, target axis 1.0 m away, contact at sphere-centre distance
    0.35 + 0.3 = 0.65 ⇒ travel 0.35 m ≤ range 1.0; at 2.0 m the travel would be 1.35 > 1.0 ⇒ out of reach.
    - `hit_lands_only_in_window`: in-reach dummy; no `DamageDealt` in ticks T0..T0+7, exactly one in T0+8 (damage 10,
      shooter = player, `headshot == false`, health drop == damage). Then a second dummy placed out of reach (2.0 m),
      swing started, dummy teleported into reach (Position + Transform) after tick T0+15 (elapsed 0.234 > 0.22): no hit
      through the swing end. Positive control: same with the teleport after T0+9 → one hit by T0+14 (the BVH refresh
      after a teleport may take one physics step, so the control asserts "≤ T0+14", the sharp tick assertion lives in
      the standing case). Flip-RED: `active_from` 0.12 → 0.10 in the fixture copy moves the hit to T0+7 (use a
      shipped-config copy via the `sabotaged` pattern or temporarily in the code; record which input was perturbed).
    - `third_combo_hit_knocks_down`: clicks before T0, T0+10, T0+30 (buffered during swings 1 and 2). Derived: hits at
      T0+8, T0+31, T0+54; reaction after hit 1 and hit 2 = `Staggered`, after hit 3 = `KnockedDown`; health drop 40;
      three `DamageDealt` with three distinct `shot` ids; `KnockedDown` still at T0+130, `Steady` at T0+131
      (1.2 − 76/64 = 0.0125 > 0, 1.2 − 77/64 < 0). Reach check from the probe: two 2 m/s jabs move the dummy 2 × 0.177 m
      → 1.354 m, travel 0.704 ≤ 1.0. Control `combo_resets_after_window`: third click after the window expired →
      `Staggered`, never `KnockedDown`.
    - `knockback_pushes_along_the_blow` — three directed cases d = −Z, +X, +Z (aim direction flattened = d, derived
      from `set_aim`: origin y 1.05, target y 1.0 ⇒ dir ≈ d − 0.05·Y ⇒ flat = d). One jab each; 32 ticks after the hit:
      `Δ·d >= 0.1` (probe: 0.177 m at 2 m/s) and `|Δ − (Δ·d)d| < 0.02`. A sign error or an x/z swap fails at least one
      case (+X is the sign/axis sentinel). Flip-RED: negate `direction` in `knock_back`.
    - `second_shove_during_knockback_is_applied`: `knock_back` is `pub` (re-exported from `combat`) and tested
      directly on a spawned dummy, exactly the probe sequence: 3 m/s along −Z, 4 ticks, 5 m/s along −Z, 128 ticks →
      displacement along −Z > 0.8 m (probe: 1.108 m with the reset, 0.363 m without). Flip-RED: delete the memory
      reset → RED. (A real two-hit overlap needs two attackers inside one knockback — this is the T9 case.)
    - `stagger_interrupts_the_victims_swing`: a second attacker (spawned `dummy_bundle` + `Loadout::default()`,
      aiming at the player, `fire_requested` raised one tick after the player's) is hit first (player's window opens
      a tick earlier) → its swing is cancelled, the player takes no damage. Control: without the player's punch the
      dummy-attacker hits the player.
    - `bat_hit_knocks_down_at_once`: spawn a `BatPickup` in the test area (like `spawn_pickup` in shooting.rs), walk
      the player onto it → `has_bat`, `melee == Bat`; one click → damage 25, `KnockedDown`; key 1 (`select =
      Unarmed`) while unarmed → `melee == Fists`; again → `Bat`.
    - `wall_blocks_the_punch`: `spawn_wall` 0.2 m thick between attacker and dummy (at 0.5 m) → no hit.
    - `knocked_down_head_is_no_headshot`: finisher, wait until the knockback settles (32 ticks, still `KnockedDown`),
      give the pistol, shoot from 5 m at the dummy's current head point (feet + 1.65 m, as
      `head_sensor_doubles_damage`) → a body hit with `headshot == false`; control: the same shot at a `Steady`
      dummy → `headshot == true`. Flip-RED: drop the `is_knocked_down` term in `fire_weapons`' predicate.
    - `gun_shot_and_punch_have_distinct_shot_ids`: shot then punch → different `DamageDealt.shot`.
    - `melee_state_is_reset_by_respawn`: set player `HitReaction::KnockedDown{1.0}` and a running swing, kill with
      `write_damage`, run through `Wasted` (loop like `input_raised_during_wasted_is_dropped`) → `Steady`, `swing None`.

16. **`crates/gta_sim/tests/config.rs`.** `shipped_melee_config_loads` (load + validate); `melee_window_inside_swing`
    via `sabotaged::<MeleeConfig>`: `"active_to: 0.22, knockback: 5.0"` → `"active_to: 0.40, knockback: 5.0"`
    (strictly past `duration` 0.35, not on the boundary — TASK-007 lesson; a different error than the next fixture) →
    error contains `fists.hits[2].active_to`; `unknown_melee_field` → `bogus_field` names file and field.

### Client

17. **`assets/juice/juice.ron` + `src/juice/config.rs`.** New fields: `hit_stop_seconds: 0.05` (real s, GDD §8),
    `shake: (melee_trauma: 0.25, decay_per_s: 1.2, max_yaw_deg: 3.0, max_pitch_deg: 3.0, max_roll_deg: 5.0,
    noise_hz: 15.0)` (`ShakeConfig`, `deny_unknown_fields`); validation: `hit_stop_seconds > 0`, trauma in (0, 1],
    others finite and `>= 0`, `noise_hz > 0`.

18. **`src/juice/hit_stop.rs` (new).** `#[derive(Component, Default)] pub struct HitStop { pub left: f32 }` (real
    seconds); `pub struct HitStopSystems` set; `HitStopPlugin`: observer `On<Add, Character>` → insert
    `HitStop::default()` (no per-hit insert/archetype move), `Update` `(tick_hit_stop, start_hit_stop).chain()
    .in_set(HitStopSystems)`: tick = `left = (left − real.delta_secs()).max(0)`; start = for each `MeleeHit`, attacker
    and target `left = juice.hit_stop_seconds` (once per hit, `get_mut`, missing entity ignored). No `Time<Virtual>`
    anywhere in juice.

19. **`src/juice/shake.rs` (new) + `src/juice/mod.rs`.** `#[derive(Resource, Default)] pub struct CameraShake {
    pub trauma: f32, pub rotation: Quat }`; `add_melee_trauma` (reads `MeleeHit`; `trauma = (trauma +
    melee_trauma).min(1)` when the player is attacker or target), `shake_camera` (real time: decay, `s = trauma²`,
    `rotation = Quat::from_euler(YXZ, max_yaw·s·n(0,t), max_pitch·s·n(1,t), max_roll·s·n(2,t))`).
    `pub fn smooth_noise(channel: u32, t: f32) -> f32` in [−1, 1]: integer-hash lattice values, smoothstep between
    `floor(t)` and `floor(t)+1`, `t = elapsed_real · noise_hz`. Unit tests: bounded; continuous (|n(t+1e-3) − n(t)| <
    0.01); channels differ; `trauma = 0` ⇒ identity rotation. `JuicePlugin` adds `HitStopPlugin`, inits
    `CameraShake`, adds both systems in `Update`.

20. **`src/camera/mod.rs` `follow_player`.** New `shake: Res<CameraShake>` (only `CameraPlugin` registers the system);
    last line becomes `camera_transform.rotation = rotation * Quat::from_rotation_x(recoil.pitch) * shake.rotation;`
    — after `aim.origin/direction` are written (aim unaffected, GDD §4.1/§8).

21. **`assets/character/visual.ron` + `src/visuals/character_config.rs`.** New fields `fists: ["attack-melee-right",
    "attack-melee-left", "attack-kick-right"]` (combo step order), `bat: "attack-melee-right"`, `knockdown: "die"`.
    `resolve` requires exactly 3 fist clips (error names the field) and fills `CharacterClips.melee: [usize; 3]`,
    `bat: usize`, `knockdown: usize` (stays `Copy`). Shipped indices: fists `[19, 20, 21]`, bat 19, knockdown 9.

22. **`src/visuals/character.rs`.**
    - `CharacterAnimations`: full-body nodes `fists: [AnimationNodeIndex; 3]`, `bat`, `knockdown` (`add_clip`, no mask)
      and their `Handle<AnimationClip>`s (for durations).
    - `CharacterAnimator` gains `action: Option<ShownAction>` (`Swing(attack id)` / `Knockdown`).
    - `drive_character_animation` reads `(&HitReaction, &Melee)` too. Priority: KnockedDown → play `knockdown` once
      (no repeat, holds the last pose); else swing → on a new `attack` id play the step's clip once at speed
      `clip.duration() / swing.duration` (1.0 while the clip asset is not loaded); else the existing locomotion path —
      when an action ends set `animator.shown` so the existing `!=` check replays locomotion. Stagger has no clip in
      the rig: it reads through knockback + hit-stop (owner checklist).
    - `apply_hit_stop` (Update, `.after(drive_character_animation).after(HitStopSystems)`): for each animator,
      `HitStop` of its character `left > 0` → `player.pause_all()`, else `resume_all()`; query `Option<&HitStop>`
      (the plain `character_gate` harness has no juice observer).
    - Keep file < 750 lines (now 356).

23. **Bat visuals — `assets/world/render.ron`, `src/visuals/config.rs`, `src/visuals/weapons.rs`, `src/visuals/mod.rs`.**
    `weapons` gains `bat_size: (0.06, 0.06, 0.85)`, `bat_color: (0.55, 0.38, 0.2)` (+ validation). `WeaponVisualAssets`
    gets `bat: Handle<Mesh>`, `bat_material`; `show_held_gun` shows the bat mesh/material when `held == None &&
    melee == Bat` (swap `Mesh3d` and material), hides as today otherwise; observer `visualize_bat_pickup` (On<Add,
    BatPickup>) + visibility by `available()` in the existing show system.

24. **`src/visuals/character_gate.rs`.** Update the `CharacterClips` literal (`melee: [19, 20, 21], bat: 19,
    knockdown: 9`). Split `character_visuals_app` into `character_visuals_app_with(extra: impl FnOnce(&mut App))`
    so a test can add `HitStopPlugin` + the shipped `JuiceConfig` before the player spawns. New gate
    `hit_stop_freezes_only_the_pair_and_not_time` (presentation gate, client crate by law — it is still a headless
    `App` with no window/GPU; a `gta_sim`-only "time untouched" test would be a tautology, the sim has no hit-stop): player (unarmed) punches a
    dummy through the real sim; animators spawned by hand for player, dummy and a bystander dummy (each started on a
    locomotion node). Update one frame at a time; `U` = the update whose `MeleeHit` appears. Derived with
    `FixedTimesteps(1)` (real dt 15.625 ms, order tick → start → apply): `left` = 0.05, 0.034375, 0.01875, 0.003125,
    0 ⇒ attacker and target animations `is_paused()` in U..U+3, resumed in U+4; bystander never paused. In every
    update U−1..U+4: `Time<Virtual>::relative_speed() == 1.0`, `!is_paused()`, and `Time<Fixed>::elapsed()` grew by
    exactly one timestep (fixed tick count unchanged by hit-stop). Flip-RED (record both): (1) make `start_hit_stop`
    call `Time<Virtual>::set_relative_speed(0.0)` → time assertions RED; (2) remove `apply_hit_stop` → pause
    assertions RED. Also extend `armed_animator_layers_arm_clips`-style check: a running swing plays `fists[step]`,
    knockdown plays `knockdown` (one assertion each, liveness of the wiring).

### Runtime QA

25. **`tools/qa/scenarios/t7.py` (new)**, reusing t6/t5 helpers (`Game`, `rows`, `player`, `dummies`, `teleport`,
    `aim_at`, `damage_numbers`, `screenshot`, `game_state`, `log_errors`, `wait_chunks`): seed 1, release;
    read `melee.ron` fists damages and bat damage with `ron_number`.
    1. Player starts unarmed (`held None`, `melee Fists`). Middle dummy; teleport to its feet + (0, 0, 1.0); settle
       1 s; `aim_at` the chest (feet + 1.0).
    2. Three LMB clicks (`send_mouse_button("Left", 80)`) 0.3 s apart (each click lands in a running swing and is
       buffered, 0.3 < 0.359 s swing), screenshots at +0.45, +0.9, +1.2 s (≥ 0.15 s apart, TASK-007 lesson).
    3. At ~1.0 s after the first click read the dummy's `HitReaction` (tolerant reader of `{"KnockedDown": {...}}`)
       → `KnockedDown`; health drop == 10 + 10 + 20; at least one `DamageNumber` alive with a value from the table
       (liveness of melee numbers). Wait `knockdown` + 0.3 s → `Steady`.
    4. Bat: teleport onto `Bat pickup` (by `Name`/`BatPickup` row), check `has_bat` and `melee == "Bat"`; teleport to
       another dummy, aim, one click → `KnockedDown`, drop 25; screenshot.
    5. `log_errors` empty, `shutdown`; `summary.json` in `--out`.
    Owner checklist (for QA_REPORT.md): run `cargo run --release -- --seed 1`, park range: three LMB on a dummy —
    punches read as a combo (right, left, kick), freeze on impact is felt but short, finisher knocks it down and it
    gets up; camera shake is noticeable on hits and not nauseating; bat pickup, key 1 toggles fists/bat, bat swing
    feels heavier. Tuning knobs: `melee.ron` (windows, knockback, stagger/knockdown), `juice.ron` (`hit_stop_seconds`,
    `shake`), `visual.ron` (clips).

### Order and checks
1. Steps 1-4, 14 → `cargo test -p gta_sim --lib` (state machine table green).
2. Steps 5-13 → `cargo build`, then step 15-16 → `cargo test -p gta_sim` green, flip-RED recorded.
3. Steps 17-24 → `cargo test -p gta_like --bin gta_like` green, flip-RED recorded; `cargo clippy -- -D warnings`.
4. Step 25 → `python tools/qa/scenarios/t7.py --out <task scratch>/qa/t7` and re-run `t6.py` (pickup count and
   shooting unchanged). `python tools/qa/tree_check.py` (no `bevy_render` in `gta_sim`: nothing added there).

## 4. Risk areas

- **Combo reach vs knockback.** Numbers are probe-derived on flat ground with a standing dummy. A moving target,
  kerbs or `acceleration_limit` changes alter the slide; if the owner raises jab knockback to the GDD 3 m/s, the
  finisher misses from 1 m (the `third_combo_hit_knocks_down` gate catches it). Q1.
- **Frame advantage for T9.** Stagger 0.35 s from a hit at 0.12-0.22 s ends at 0.47-0.57 s while the next jab's
  window is 0.47-0.57 s after the first click: NPC victims (T9) may escape between jabs. Irrelevant for dummies and
  the player now; data tweak later.
- **Sphere cast from inside capsules.** A victim closer than 0.65 m is already overlapping at distance 0 — still a hit
  (default `ignore_origin_penetration: false`); the attacker is excluded by entity. Tnua ground sensor is a cast, not a
  collider, so it cannot be hit. Head sensors are excluded by the mask.
- **Teleport + BVH.** avian updates the collider tree in the physics step; tests that teleport assert with one tick
  of slack (step 15), sharp tick numbers only for bodies that stood still.
- **Knockback interplay.** Jump feeding is suppressed while a reaction is active; Tnua would keep a running knockback
  anyway (it never cancels; `apply` returns `StillActive` until the boundary clears), probe finished ≤ 23 ticks.
  `force_forward` rotates the victim toward the attacker (angular only, the probe ran with `None`; linear slide is
  unaffected by construction of `apply`).
- **Animation.** Kenney clip lengths vs 0.35 s swings are unknown until runtime (rate from `clip.duration()`); the
  `die` clip as knockdown and "stand up" as a blend back to idle may look abrupt. Owner-run only.
- **Shake comfort.** trauma² with 0.25 per hit is subtle alone and accumulates over a combo (≤ ~0.6); rotational only.
  Owner-run only ("тряска не тошнит"); accessibility multiplier arrives with settings (T12).
- **`Option<&HitStop>` in `apply_hit_stop`** is a deliberate exception to the With/Without rule (the plain visuals
  harness has no juice).
- **Surgical scope.** `fire_weapons` changes only its counter and the knocked-down head predicate; `WeaponPickup`,
  `cycle_weapon`, input bindings untouched. t6.py must stay green (6 `WeaponPickup` rows, `held` format).

## 5. Open questions

Вопросы владельцу (по делегированию — оркестратору). У каждого есть дефолт, план написан под него.

- **Q1. Отброс от джебов.** GDD §4.2 даёт "отброс 3-6 м/с". Замер (проба): 3 м/с отодвигают манекен на 0.36 м, после
  двух джебов цель на 1.73 м и добивающий удар (досягаемость 1.65 м) промахивается.
  - A (дефолт): джебы 2 м/с (0.18 м), добивающий 5, бита 6 — всё в `melee.ron`. Комбо гарантированно проходит.
  - B: джебы 3 м/с и шаг атакующего вперёд во время удара (новая механика "выпад", новое число в данных).
  - C: джебы без отброса (0), только stagger; отброс только у добивающего и биты. Самый "липкий" бой.
- **Q2. Выбор биты.** Клавиш 1-4 четыре, а состояний пять (кулаки, бита, 3 ствола).
  - A (дефолт): 1 = ближний бой; повторное нажатие 1 переключает кулаки ↔ бита; колесо попадает в "ближний бой" с
    текущим выбором. Код T6 не меняется.
  - B: бита как отдельный пункт колеса и клавиша 5 (правка GDD §3.3).
  - C: бита заменяет кулаки навсегда (как слот melee в GTA III); кулаки возвращаются только после ареста (T11).
- **Q3. Тряска.** GDD §8: "двигает только визуальный pivot камеры". План: поворот камеры (yaw/pitch/roll) после записи
  `AimIntent` — прицел не трогается, камера не лезет в стену.
  - A (дефолт): только поворот.
  - B: смещение позиции камеры (буквально "pivot"); риск залезть в стену сразу после коллизионного каста.

children: 0 launched / 0 reported.
