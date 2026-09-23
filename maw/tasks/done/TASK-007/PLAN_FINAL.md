# PLAN_FINAL — TASK-007 (GDD T6): shooting + floating damage numbers

Pinned versions (from `Cargo.lock`, sources under `~/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/`):
bevy / bevy_ecs / bevy_ui / bevy_text / bevy_camera / bevy_audio / bevy_light / bevy_time 0.19.1, avian3d 0.7.0
(avian_derive 0.2.3), bevy_enhanced_input 0.26.0, bevy_brp_extras 0.22.6, rodio 0.22.2, glam 0.32.1,
rand_chacha 0.10.0 / rand_core 0.10.1, vendored `vendor/bevy-tnua-avian3d-0.12.1`.
Executable probe from the planner: `maw/tasks/in_progress/TASK-007/scratch/probe/tests/probe.rs` (B1 reproduced: 70/100;
head sphere is the closest ray hit at 1.45-1.65 m; `ColliderOf` exists on body and child collider).

Cost of error. Silent defects get headless gates: damage from `Wasted` hitting the respawned player (B1); a headshot
that never fires; a wall that does not block; Tnua standing on a head sensor; wrong tick arithmetic for reload/spread;
damage shown != damage applied; damage-number entities leaking. Recoil, tracer, flash, sound, crosshair, aim camera,
strafe feel and the look of the damage-number animation are seen by the owner on the first frame: mechanism +
BRP screenshots + owner checklist, no extra machinery.

---

## 1. Summary

Add the T6 weapon slice to the headless sim (`gta_sim`) and its presentation to the client (`gta_like`). Sim:
collision layers `World/Character/Hitbox`, a child `Sensor` head sphere on every `Character`, `AimIntent` +
`ActionIntent` written by the client and consumed in `FixedUpdate`, a `Loadout` component with three guns from
`assets/combat/weapons.ron` (pistol/shotgun semi-auto, SMG automatic), two-ray hitscan through avian
`SpatialQuery::cast_ray_predicate`, cone spread with bloom and delayed recovery, magazines/reload, weapon switching,
weapon and ammo pickups, target dummies in the central park, a run-speed cap while aiming, and the B1 fix
(`Messages<DebugDamage>::clear()` on `OnExit(Wasted)`). Owner addition: every hit rolls its damage as
`round(base × falloff × head_mult × (1 + variance·(2u − 1)))` from the sim-owned seeded `CombatRng`; the rolled integer
is both applied to `Health` and carried in a new buffered message `DamageDealt { shooter, target, point, damage: u32,
headshot, killed }`. Client: aim camera, recoil, flash, tracer, procedural shot sound, ammo HUD, crosshair, hit/kill
marker, held/pickup primitives, and floating damage numbers implemented as absolutely positioned `bevy_ui` `Text`
nodes projected each frame with `Camera::world_to_viewport` (always camera-facing by construction), animated by
`UiTransform.scale` (pop), screen-space rise with ease-out, alpha fade and golden-angle sideways drift; headshots use
a larger red font and the `"{damage} CRIT"` template from `strings.ron`; all animation numbers live in
`assets/juice/juice.ron`; each label despawns when its lifetime ends. Headless gates in `gta_sim` for the gameplay
rules (bands derived from `weapons.ron`), a client-crate headless gate for label text/colour and no-leak, and a BRP
scenario `tools/qa/scenarios/t6.py`.

---

## 2. Implementation steps

Order: 0 baseline → A data → B sim foundation → C sim weapons → D sim gates → E client → F runtime QA → G completion.

### Step 0. Baseline and API check

- Run and record: `cargo test -p gta_sim`, `cargo test -p gta_like --bin gta_like`, `cargo clippy -- -D warnings`,
  `cargo tree -p gta_sim -e features -i bevy_render` (must be empty before and after).
- Before editing a system signature, grep its name across `src/`, `crates/*/src`, `crates/*/tests`:
  `drive_characters`, `follow_player`, `apply_mouse_look`, `spawn_player`, `respawn_player`, `apply_debug_damage`,
  `write_move_intent`, `LocomotionConfig`, `UiConfig`, `HudLayout`. Every harness must go through the plugin.
- Verify on city seed 1 (release build, BRP) that the park around `CityLandmarks.park_center` has a flat lane of
  ≥ 12 m along −Z from the centroid with no static collider (query `Collider` + `Position` within 15 m). If it does
  not, shift the range origin inside the same park block using the actual block geometry; do not modify `citygen`.
  `world/city.rs::landmarks` only computes a centroid; it does not guarantee a clear lane (V2 finding 10).

### A. Data files (every new tuning number lives here; no tuning `const`)

**A1. `assets/character/locomotion.ron`** — append:
```ron
    head_height: 1.6,
    head_radius: 0.35,
    aim_max_gait: Run,
```
(head sphere centre above the feet and radius, m; fastest gait while RMB is held — Q4 = B).

**A2. `assets/combat/weapons.ron`** (new, UTF-8 without BOM). Damage/rate/magazine/reload/base spread/range are the
GDD §4.1 table; the rest are starting values the owner tunes:
```ron
(
    headshot_multiplier: 2.0,
    pistol: (damage: 25.0, damage_variance: 0.1, pellets: 1, fire_mode: SemiAutomatic, fire_interval: 0.3,
             magazine: 12, reload: 1.2, range: 60.0, falloff: None,
             spread: (base_deg: 1.0, per_shot_deg: 1.0, max_bloom_deg: 3.0, recovery_delay: 0.35, recovery_deg_per_s: 6.0, moving_deg_per_mps: 0.3),
             max_reserve: 120, pickup_ammo: 24),
    smg:    (damage: 12.0, damage_variance: 0.1, pellets: 1, fire_mode: Automatic, fire_interval: 0.08,
             magazine: 30, reload: 1.8, range: 45.0, falloff: None,
             spread: (base_deg: 3.0, per_shot_deg: 0.4, max_bloom_deg: 4.0, recovery_delay: 0.15, recovery_deg_per_s: 8.0, moving_deg_per_mps: 0.4),
             max_reserve: 300, pickup_ammo: 60),
    shotgun: (damage: 8.0, damage_variance: 0.1, pellets: 10, fire_mode: SemiAutomatic, fire_interval: 0.9,
             magazine: 6, reload: 2.5, range: 25.0, falloff: Some((start: 10.0, min_factor: 0.3)),
             spread: (base_deg: 6.0, per_shot_deg: 2.0, max_bloom_deg: 3.0, recovery_delay: 1.0, recovery_deg_per_s: 4.0, moving_deg_per_mps: 0.2),
             max_reserve: 48, pickup_ammo: 12),
    pickups: (radius: 1.0, respawn: 30.0),
    range: (dummies: 3, dummy_spacing: 3.0, dummy_distance: 10.0, pickup_spacing: 2.0, dummy_reset: 3.0),
)
```
`damage_variance` is the half-width of the per-hit factor (0.1 → factor in [0.9, 1.1]); per weapon, as the owner
allowed "per weapon or global".

**A3. `assets/combat/aim.ron`** (new): `(max_aim_distance: 200.0, min_aim_distance: 0.5, muzzle_offset: (0.25, 0.35, -0.45))`
— ray-1 length; minimum depth of the aim point in front of the muzzle (closer → fire along the aim direction);
muzzle relative to the body centre in the aim-yaw frame (x right, y up, z forward = −Z).

**A4. `assets/camera/camera.ron`** — add (GDD §3.2): `aim_shoulder_offset: 0.55, aim_distance: 2.0, aim_fov_deg: 55.0,
aim_transition: 0.15, aim_sensitivity_scale: 0.7`.

**A5. `assets/juice/juice.ron`** (new):
```ron
(
    recoil_deg: (pistol: 1.0, smg: 0.5, shotgun: 2.0),
    recoil_half_life: 0.08,
    flash: (seconds: 0.05, size: 0.25, color: (1.0, 0.8, 0.4), light_intensity: 100000.0, light_range: 6.0),
    tracer: (seconds: 0.06, width: 0.02, color: (1.0, 0.9, 0.6)),
    damage_numbers: (
        lifetime: 0.8,          // s, real time
        pop_seconds: 0.08,      // s, scale eases from pop_start_scale to 1
        pop_start_scale: 1.8,   // <1 = grow-in, >1 = punch-in
        rise_px: 60.0,          // total upward travel, ease-out cubic
        drift_px: 24.0,         // max sideways travel, linear
        fade_start: 0.5,        // fraction of lifetime where alpha starts falling to 0
        size: 26.0,             // font px, body hit
        crit_size: 38.0,        // font px, headshot
        color: (1.0, 1.0, 1.0),
        crit_color: (0.95, 0.12, 0.12),
    ),
)
```
(Recoil 0.5-2°, flash 50 ms, tracer 60 ms — GDD §8; light intensity in lumens, `bevy_light-0.19.1/src/point_light.rs:53`.)
Remove the `//` comments if the strict RON loader rejects them (RON 0.12 accepts `//` comments; keep them only if
`shipped_juice_config_loads` passes).

**A6. `assets/audio/mix.ron`** (new): `(shot: (pistol: (seconds: 0.18, decay: 22.0, volume: 0.5), smg: (seconds: 0.10,
decay: 35.0, volume: 0.35), shotgun: (seconds: 0.35, decay: 12.0, volume: 0.7)))` — burst length, envelope decay (1/s),
linear volume.

**A7. `assets/ui/strings.ron`**:
- top level (next to `wasted`): `damage_crit: "{damage} CRIT",`
- `hud` section — add: `ammo_size: 22.0, ammo_color: (1.0, 1.0, 1.0), crosshair_dot: 4.0, crosshair_arm: 8.0,
  crosshair_thickness: 2.0, crosshair_color: (1.0, 1.0, 1.0, 0.9), hit_marker_size: 28.0, hit_marker_seconds: 0.1,
  hit_marker_color: (1.0, 1.0, 1.0), kill_marker_color: (0.9, 0.1, 0.1)` (hit marker 0.1 s — GDD §7).

**A8. `assets/world/render.ron`** — new section `weapons: (held_size: (0.08, 0.12, 0.35), pickup_size: (0.15, 0.2, 0.6),
ammo_size: 0.3, pistol_color: (0.2, 0.2, 0.22), smg_color: (0.35, 0.3, 0.2), shotgun_color: (0.45, 0.25, 0.12),
ammo_color: (0.85, 0.75, 0.2))`.

### B. Sim foundation: layers, head, intents, health, B1

**B1. `crates/gta_sim/src/layers.rs`** (new) + `pub mod layers;` in `lib.rs`:
`#[derive(PhysicsLayer, Default, Clone, Copy, Debug)] pub enum GameLayer { #[default] World, Character, Hitbox }`
with a `///` line: collision layers are GDD §12 law; `World` is the default layer of all static geometry.
Verified: avian_derive 0.2.3 reserves `1 << 0` for the `#[default]` variant, and `CollisionLayers::DEFAULT` has
`memberships: LayerMask::DEFAULT` (`avian3d-0.7.0/src/collision/collider/layers.rs:372-376`), so untouched statics are
`World`. `LayerMask: From<[L; N]>` exists (`layers.rs:100`).

**B2. `character/locomotion.rs`**: fields `head_height: f32`, `head_radius: f32`, `aim_max_gait: Gait` in
`LocomotionConfig`. `validate()` (after the speed checks): all finite; `head_radius > 0`;
with `top = float_height + capsule_height / 2` (1.80 m shipped) — `head_height < top` and
`head_height + head_radius > top` and `head_height - head_radius < top - capsule_radius` (the sphere must stick out of
the capsule's upper hemisphere where the head is aimed; with shipped data: 1.25 < 1.50). Error names the field and the
reason ("otherwise a ray always hits the capsule before the head"). The earlier `head_radius > capsule_radius` rule is
dropped (V2 finding 12: arbitrary); the real property is proved by gate D3/D7 geometry.
**`character/intent.rs`**: `Gait` gains `serde::Deserialize, PartialOrd, Ord` (variant order Walk < Run < Sprint is
the cap order).

**B3. `character/mod.rs`**:
- `#[derive(Component, Reflect, Default)] #[reflect(Component)] pub struct HeadHitbox;` + `register_type`.
- `pub fn head_hitbox(cfg: &LocomotionConfig) -> impl Bundle` = `(HeadHitbox, Name::new("Head hitbox"),
  Collider::sphere(cfg.head_radius), Sensor, CollisionLayers::new(GameLayer::Hitbox, LayerMask::NONE),
  Transform::from_xyz(0.0, cfg.head_height - cfg.float_height, 0.0))` (local y 0.55 with shipped data).
- `character_components` (`:95-117`): add `CollisionLayers::new(GameLayer::Character, LayerMask::ALL)` and
  `children![head_hitbox(cfg)]`.
- `Character` `#[require(...)]` (`:29`): add `AimIntent, ActionIntent`; `register_type` both.
- `drive_characters` (`:119-171`): add `&AimIntent` to the query. Speed:
  `let gait = if aim.aiming { intent.gait.min(cfg.aim_max_gait) } else { intent.gait };` then `cfg.speed(gait)`.
  Facing: if `aim.aiming`, `desired_forward = Dir3::new(Vec3::new(aim.direction.x, 0.0, aim.direction.z)).ok()`
  (body faces aim yaw, translation stays camera-relative → strafe, GDD §3.2); else unchanged. Do not touch the `dead`
  branch. Clamping in the sim, not the client, so headless tests and future AI get the same cap (V2 finding 1).

**B4. `character/intent.rs`**:
```rust
#[derive(Component, Reflect, Default, Clone, Debug)] #[reflect(Component, Default)]
pub struct AimIntent { pub origin: Vec3, pub direction: Vec3, pub aiming: bool }
#[derive(Component, Reflect, Default, Clone, Debug)] #[reflect(Component, Default)]
pub struct ActionIntent { pub fire_held: bool, pub fire_requested: bool, pub reload_requested: bool,
                          pub select: Option<WeaponRequest>, pub cycle: i32 }
#[derive(Reflect, Clone, Copy, Debug, PartialEq, Eq)]
pub enum WeaponRequest { Unarmed, Gun(Weapon) }
```
Key 1 → `Unarmed`, 2-4 → `Gun(Pistol/Smg/Shotgun)`. `Weapon` is defined once in `combat/weapons.rs` and imported
(`crate::combat::Weapon`). Re-export in `character/mod.rs:11`.

**B5. `character/health.rs`**: `impl Health { pub fn take(&mut self, amount: f32) -> bool }` — `let was_alive =
self.current > 0.0; (self.current, self.armor) = apply_damage(..); self.since_damage = 0.0; was_alive && self.current <= 0.0`
(true only on the positive-to-zero transition). `player/mod.rs:72-75` `apply_debug_damage` switches to
`health.take(amount)` (one armour rule for debug and weapons).

**B6. B1 — `flow/wasted.rs`**: `pub(super) fn drop_queued_damage(mut queued: ResMut<Messages<DebugDamage>>) { queued.clear(); }`
with one comment line ("the damage reader is gated to PlayingSystems: without this a message from Wasted hits the
respawned player"). `flow/mod.rs:58`: `OnExit(GameState::Wasted)` → `(wasted::respawn_player, wasted::drop_queued_damage)`.
`Messages::clear` verified at `bevy_ecs-0.19.1/src/message/messages.rs:228`. Do not clear `ShotFired`/`BulletTrace`/
`DamageDealt` (presentation; nothing sim-side reads them).

**B7. `lib.rs` `compose_sim`**: load + validate `WeaponsConfig` (`combat/weapons.ron`) and `AimConfig`
(`combat/aim.ron`) exactly like `:31-44`, `insert_resource` both before `add_plugins`.

**B8. `crates/gta_sim/Cargo.toml`**: `rand_chacha = { version = "=0.10.0", default-features = false }` (same line as
`crates/citygen/Cargo.toml:10`). Already in `Cargo.lock` (0.10.0 / rand_core 0.10.1); the lock gains one dependency
line (`scratch/Cargo.lock.with_rand_chacha`). After adding: `cargo tree -p gta_sim -e features -i bevy_render` still empty.

### C. Sim weapons (`crates/gta_sim/src/combat/`)

**C1. `combat/weapons.rs`** (new):
- `pub const WEAPONS_CONFIG: &str = "combat/weapons.ron";`
- `#[derive(Reflect, Clone, Copy, Debug, PartialEq, Eq, Deserialize)] pub enum Weapon { Pistol, Smg, Shotgun }` +
  `pub const ALL: [Weapon; 3]` + `fn index(self) -> usize`; `///` "order = index into `Loadout.guns`".
- `#[derive(Deserialize, Clone, Copy, Debug, PartialEq, Eq)] pub enum FireMode { SemiAutomatic, Automatic }`.
- `WeaponsConfig {headshot_multiplier, pistol, smg, shotgun: WeaponStats, pickups: WeaponPickupConfig, range: RangeConfig}`,
  `WeaponStats {damage, damage_variance, pellets: u32, fire_mode: FireMode, fire_interval, magazine: u32, reload, range,
  falloff: Option<Falloff>, spread: SpreadConfig, max_reserve: u32, pickup_ammo: u32}`, `Falloff {start, min_factor}`,
  `SpreadConfig {base_deg, per_shot_deg, max_bloom_deg, recovery_delay, recovery_deg_per_s, moving_deg_per_mps}`,
  `WeaponPickupConfig {radius, respawn}`, `RangeConfig {dummies: u32, dummy_spacing, dummy_distance, pickup_spacing,
  dummy_reset}` — all `#[derive(Resource?/Deserialize, Clone, Debug)] #[serde(deny_unknown_fields)]`
  (`Resource` only on `WeaponsConfig`). `fn stats(&self, Weapon) -> &WeaponStats`.
- `validate()`: every float finite; `damage > 0`; `0 <= damage_variance < 1`; `pellets >= 1`; `fire_interval > 0`;
  `magazine >= 1`; **`reload > 0`** (V2 finding 5: reload-in-progress is `reload_left > 0`, zero would never transfer);
  `range > 0`; all spread fields `>= 0`; `0 < falloff.start < range`; `min_factor ∈ [0, 1]`;
  `headshot_multiplier >= 1`; `pickup_ammo >= 1`; `max_reserve >= pickup_ammo`; `pickups.radius > 0`;
  `pickups.respawn >= 0`; `range.dummies >= 1`; distances > 0; `pickup_spacing > pickups.radius`. **No**
  `recovery_delay > fire_interval` rule (V2 finding 6: sufficient, not necessary; D6 proves growth on shipped SMG
  data). Errors carry the weapon prefix (`smg.damage_variance is out of range`), reuse the `check()` pattern of
  `health.rs:37-43`.
- `#[derive(Reflect, Default, Clone, Copy, Debug, PartialEq)] pub struct GunSlot { pub owned: bool, pub magazine: u32, pub reserve: u32 }`.
- `#[derive(Component, Reflect, Default, Clone, Debug)] #[reflect(Component, Default)] pub struct Loadout { pub held:
  Option<Weapon>, pub guns: [GunSlot; 3], pub cooldown: f32, pub reload_left: f32, pub since_shot: f32, pub bloom_deg: f32,
  pub spread_deg: f32 }` (`spread_deg` is derived state for HUD and gates; written only by `tick_loadouts`/`fire_weapons`).
- Pure functions with `///`:
  - `cycle_weapon(held: Option<Weapon>, owned: [bool; 3], step: i32) -> Option<Weapon>` — ring `[None, Pistol, Smg,
    Shotgun]` over owned guns, `None` always in the ring.
  - `falloff_factor(stats, distance) -> f32` — 1 up to `start`, linear to `min_factor` at `range`.
  - `acquire(slot: &mut GunSlot, stats, gun: bool) -> bool` — gun: `owned = true`; first time
    `magazine = min(magazine_size, pickup_ammo)`, rest to `reserve`; ammo: `reserve = min(reserve + pickup_ammo, max_reserve)`;
    `false` if nothing changed.
  - **`roll_damage(base: f32, variance: f32, u: f32) -> u32`** = `(base * (1.0 + variance * (2.0 * u - 1.0))).round().max(1.0) as u32`,
    `u ∈ [0, 1)`. `base` already includes falloff and head multiplier. Integer by type, so the client cannot show a
    fraction and the applied value equals the shown value.
- System `tick_loadouts(cfg, time: Res<Time<Fixed>>, Query<(&mut Loadout, &mut ActionIntent, &LinearVelocity), (With<Character>, Without<Dead>)>)`.
  Order inside one tick (gate numbers depend on it):
  1. `select`/`cycle` → new `held` (owned guns only); on change `reload_left = 0`; clear both latches.
  2. `since_shot += dt`; `cooldown = (cooldown - dt).max(0)`.
  3. if `reload_left > 0`: `reload_left -= dt`; at `<= 0` transfer `min(magazine_size - magazine, reserve)` and set 0.
  4. if `since_shot >= recovery_delay`: `bloom = (bloom - recovery_deg_per_s * dt).max(0)`.
  5. `reload_requested` (clear it): if a gun is held, not reloading, magazine not full, reserve > 0 → `reload_left = reload`.
  6. `spread_deg = base_deg + bloom_deg + moving_deg_per_mps * |v_xz|` (0 without a gun).

**C2. `combat/hitscan.rs`** (new):
- `pub const AIM_CONFIG: &str = "combat/aim.ron";` `AimConfig {max_aim_distance, min_aim_distance, muzzle_offset: (f32, f32, f32)}`
  + `validate` (finite; `max_aim_distance > 0`; `min_aim_distance >= 0`).
- Buffered messages (`add_message`, written in `FixedUpdate`, read in `Update` — `Message` semantics, not observers,
  because presentation consumes them in another schedule):
  - `ShotFired { shooter: Entity, weapon: Weapon, muzzle: Vec3 }` — one per trigger pull (flash, sound, recoil).
  - `BulletTrace { shooter: Entity, from: Vec3, to: Vec3 }` — one per pellet (tracer).
  - **`DamageDealt { shooter: Entity, target: Entity, point: Vec3, damage: u32, headshot: bool, killed: bool }`** —
    one per pellet that damaged a live target; `#[derive(Message, Reflect, Clone, Copy, Debug)] #[reflect(Message)]`
    + `register_type`. `damage` is the exact amount passed to `Health::take` (before armour; on a lethal hit the health
    drop can be smaller — overkill is not clamped in the number). Hit marker and damage numbers read this message;
    a future melee hit (T7) writes the same message. This replaces PLAN.md's `BulletHit` enum (one source of `killed`).
- `#[derive(Resource)] pub struct CombatRng(ChaCha8Rng)` with `Default` = `ChaCha8Rng::seed_from_u64(0)` (imports as in
  `crates/citygen/src/rng.rs:1-4`: `rand_chacha::{ChaCha8Rng, rand_core::{Rng, SeedableRng}}`). Plus a local
  `fn unit_f32(rng: &mut impl Rng) -> f32 { (rng.next_u32() >> 8) as f32 * (1.0 / 16_777_216.0) }` (citygen's is
  `pub(crate)`, not reachable). The seed is not tuning (reproducibility law). Draw order per pellet: cone `u`, cone `v`,
  then damage `u` only when the pellet damages a live target.
- `pub fn aim_yaw(direction: Vec3) -> f32 { f32::atan2(-direction.x, -direction.z) }`,
  `pub fn muzzle(position: Vec3, direction: Vec3, offset: Vec3) -> Vec3 = position + Quat::from_rotation_y(aim_yaw(direction)) * offset`.
  Three directional worked examples (`R_y(θ)·(x,y,z) = (x cosθ + z sinθ, y, −x sinθ + z cosθ)`, offset (0.25, 0.35, −0.45)):
  1. dir (0,0,−1): yaw 0 → (0.25, 0.35, −0.45): forward −Z, right +X.
  2. dir (−1,0,0): yaw +90° → (−0.45, 0.35, −0.25): forward −X, right −Z (same as D at yaw 90°, GDD §3.2).
  3. dir (0,0,1): yaw 180° → (−0.25, 0.35, 0.45): forward +Z, right −X.
  Unit test on these three.
- `pub fn cone_sample(axis: Dir3, half_angle: f32, u: f32, v: f32) -> Dir3`: `cosθ = 1 − u(1 − cos α)`, `φ = 2πv`,
  `(b1, b2) = axis.any_orthonormal_pair()` (glam 0.32.1 `Vec3::any_orthonormal_pair`), `d = a·cosθ + (b1 cosφ + b2 sinφ)·sinθ`.
  `half_angle` is **radians**; the caller passes `spread_deg.to_radians()`.
- System `fire_weapons(cfg, aim_cfg, spatial: SpatialQuery, mut rng: ResMut<CombatRng>, shooters: Query<(Entity,
  &Position, &AimIntent, &mut ActionIntent, &mut Loadout, &LinearVelocity), (With<Character>, Without<Dead>)>,
  colliders: Query<(&ColliderOf, Has<HeadHitbox>)>, dead: Query<(), With<Dead>>, mut targets: Query<&mut Health, Without<Dead>>,
  fired: MessageWriter<ShotFired>, traces: MessageWriter<BulletTrace>, dealt: MessageWriter<DamageDealt>)`.
  Resolve query conflicts (`Health` is not in `shooters`; if clippy/ECS reports a conflict use a `ParamSet`).
  Per shooter, Golden Path:
  1. `let requested = take(&mut action.fire_requested);`
     `let wants = match stats.fire_mode { SemiAutomatic => requested, Automatic => action.fire_held || requested };`
     A semi-auto gun never fires from `fire_held`. A request that arrives during cooldown or reload is **discarded**
     (consumed and dropped; no buffering). No `held`, `!wants`, `reload_left > 0`, `cooldown > 0` → continue.
     `let Ok(dir) = Dir3::new(aim.direction) else continue` (an empty intent spends no ammo).
  2. Magazine 0 → if reserve > 0, `reload_left = reload` (auto-reload); continue.
  3. `magazine -= 1; cooldown = fire_interval; since_shot = 0`; the shot uses the current `spread_deg`; then
     `bloom = min(bloom + per_shot, max_bloom)` and recompute `spread_deg`.
  4. Filter `SpatialQueryFilter::from_mask([GameLayer::World, GameLayer::Character, GameLayer::Hitbox])`; predicate
     `|e| match colliders.get(e) { Ok((of, head)) => of.body != shooter && !(head && dead.contains(of.body)), Err(_) => true }`
     — excludes the shooter's own body and head, and makes the head sensor of a `Dead` character invisible (GDD §4.1
     "head sensor off in death"; the corpse capsule still blocks). No layer mutation on death/reset is needed: the
     predicate follows `Dead` automatically (replaces V2's "remove/restore ray visibility" step).
     Ray 1: `cast_ray_predicate(aim.origin, dir, max_aim_distance, true, &filter, &pred)` → point `P` (or
     `origin + dir·max_aim_distance`). Muzzle `m = muzzle(position, dir, offset)`. Pellet axis: `(P − m)` if
     `(P − m)·dir > min_aim_distance`, else `dir`.
  5. Write `ShotFired`. For each of `pellets`: `d = cone_sample(axis, spread.to_radians(), unit_f32(rng), unit_f32(rng))`;
     ray 2 `cast_ray_predicate(m, d, range, true, ..)`. No hit → `BulletTrace { to: m + d·range }`. Hit → `to = m + d·hit.distance`;
     `(body, head) = colliders.get(hit.entity).map_or((hit.entity, false), |(of, h)| (of.body, h))`;
     `targets.get_mut(body)`: `Ok(health)` and `health.current > 0` → `base = damage × falloff_factor(stats, hit.distance)
     × (if head { headshot_multiplier } else { 1 })`, `amount = roll_damage(base, damage_variance, unit_f32(rng))`,
     `killed = health.take(amount as f32)`, write `DamageDealt { shooter, target: body, point: to, damage: amount, headshot: head, killed }`.
     Otherwise (wall, corpse, zero-health target whose `Dead` is still deferred) → no damage. Always write the
     `BulletTrace`. The `current > 0` guard covers the deferred-`Dead` window (V2 finding 3).
  Signatures verified: `cast_ray_predicate(origin, Dir, max, solid, &SpatialQueryFilter, &dyn Fn(Entity)->bool) -> Option<RayHitData>`
  (`avian3d-0.7.0/src/spatial_query/system_param.rs:176-184`); `SpatialQueryFilter::test` checks only memberships
  vs mask (`query_filter.rs:97-100`).

**C3. `combat/pickups.rs`** — next to the existing `Pickup` (do not touch it: T5 gates and `t5.py` count exactly
2 `Pickup` and key by `kind`):
- `#[derive(Component, Reflect, Debug)] #[reflect(Component)] pub struct WeaponPickup { pub weapon: Weapon, pub ammo_only: bool, pub cooldown: f32 }` + `available()`.
- `collect_weapon_pickups(cfg, time, pickups, players: Query<(&Position, &CharacterBody, &mut Loadout), (With<Player>, Without<Dead>)>)`,
  same shape as `collect_pickups` (`combat/pickups.rs:49-81`): feet within `pickups.radius` → `acquire(..)`; if `true`,
  `cooldown = respawn`; if a gun was taken and `held == None` → `held = Some(weapon)`.

**C4. `combat/range.rs`** (new):
- `#[derive(Component, Reflect, Default)] #[reflect(Component)] pub struct Dummy { pub reset_left: f32 }`.
- `pub fn dummy_bundle(loco: &LocomotionConfig, handle: Handle<CharacterSchemeConfig>, health: &HealthConfig, feet: Vec3) -> impl Bundle`
  = `(Dummy::default(), Name::new("Dummy"), Transform::from_translation(feet + Vec3::Y * loco.float_height),
  character_components(loco, handle), Health::full(health))`. Used by the range and by gates.
- `spawn_range` on `OnTransition { exited: Loading, entered: Playing }` with `.run_if(resource_exists::<CityLandmarks>)`
  (one-shot, not a state gate): origin `c = park_center` (or the Step-0 adjusted point); dummies at
  `c + X·(i − (n−1)/2)·dummy_spacing − Z·dummy_distance`; a row of 6 pickups (gun + ammo per weapon) along X, spaced
  `pickup_spacing`, centred on `c`.
- `dummy_life(cfg, health_cfg, time, alive: Query<(Entity, &Health, &mut Dummy), Without<Dead>>, dead: Query<(Entity, &mut Health, &mut Dummy), With<Dead>>, commands)`
  in `HealthSystems::Death`: `current <= 0` → `insert(Dead)`, `reset_left = dummy_reset`; with `Dead`: `reset_left -= dt`,
  at `<= 0` → `Health::full`, `try_remove::<Dead>()`. Dummies do not regenerate (`regenerate_player` is `With<Player>`),
  so the health drop per hit equals the damage exactly (armour 0 from `Health::full`).

**C5. `combat/mod.rs`**: modules, re-exports (`Weapon, FireMode, WeaponsConfig, WEAPONS_CONFIG, Loadout, GunSlot,
AimConfig, AIM_CONFIG, ShotFired, BulletTrace, DamageDealt, WeaponPickup, Dummy, dummy_bundle, cone_sample,
falloff_factor, cycle_weapon, roll_damage, muzzle, aim_yaw, CombatRng`), `init_resource::<CombatRng>()`,
`add_message` ×3, `register_type` (`Loadout, GunSlot, Weapon, WeaponPickup, Dummy, DamageDealt`), systems:
```rust
.add_systems(FixedUpdate, (
    (weapons::tick_loadouts, hitscan::fire_weapons).chain().in_set(HealthSystems::Damage),
    pickups::collect_weapon_pickups.in_set(HealthSystems::Pickup),
    range::dummy_life.in_set(HealthSystems::Death),
).in_set(PlayingSystems))
.add_systems(OnTransition { exited: GameState::Loading, entered: GameState::Playing },
    range::spawn_range.run_if(resource_exists::<CityLandmarks>))
```
Each file < 400 lines.

**C6. `player/mod.rs:46-60`** `spawn_player`: add `Loadout::default()` (starts unarmed). `respawn_player` does not touch
`Loadout` → guns and ammo survive death (GDD §3.4).

### D. Sim gates (`cargo test -p gta_sim`; each is flip-RED, perturbation named)

Helpers in `tests/common/mod.rs` (existing: `composed_app`, `headless_app`, `run_ticks`, `place_player`, `set_intent`,
`set_health`, `write_damage`, `count`): add `set_aim(app, origin, target)`, `set_action(app, |a| ..)`, `loadout(app)`,
`set_loadout(app, |l| ..)`, `spawn_dummy(app, feet) -> Entity` (via `combat::dummy_bundle` and the app's resources),
`health_of(app, e)`, `set_health_of(app, e, |h| ..)`, `traces(app) -> Vec<BulletTrace>` and `dealt(app) -> Vec<DamageDealt>`
(fresh cursor from `Messages::get_cursor()`, read right after the firing tick), `spawn_wall(app, center, size)` (static
cuboid fixture). Positions below are in the free zone of `TestArea` (x = −20; floor is 80×80 around 0; nearest
obstacle is the wall at (0, 2, 14) spanning x −6..6). After spawning targets, `run_ticks(8)`.
Expected values are read from the shipped RON through the loaded resources, never hard-coded.

Geometry (A1-A3): shooter feet (−20, 0, 30), body centre y 1.05, muzzle at yaw 0 = (−19.75, 1.40, 29.55). Dummy feet
(−20, 0, 20): capsule r 0.3 from y 0.3 to 1.8, upper hemisphere centre 1.5; head sphere r 0.35 centred at 1.6
(sticks out of the capsule above ≈1.42 m).

Damage bands (derived through `roll_damage`, `u ∈ [0,1)`, `round` = half away from zero):
| case | base | band `[roll(u=0), roll(u=1)]` |
|---|---|---|
| pistol body | 25 | 22.5→**23** .. 27.5→**28** |
| pistol head | 25×2 = 50 | **45** .. **55** |
| SMG body | 12 | 10.8→**11** .. 13.2→**13** |
| shotgun pellet < 10 m | 8 | 7.2→**7** .. 8.8→**9** |
| shotgun pellet 17.5 m | 8×0.65 = 5.2 | 4.68→**5** .. 5.72→**6** |
Pistol body and head bands are disjoint, and pistol and SMG bands are disjoint, so a wrong multiplier or a wrong table
row lands outside the band.

**D1. `tests/shooting.rs::pistol_hits_dummy_at_10m_for_table_damage`** (AC1 + owner "applied == shown").
Pistol held (12/0), `AimIntent` from (−20, 1.05, 30) at the chest (−20, 1.0, 20), `fire_requested`, 1 tick.
Derivation: P on the capsule front (z ≈ 20.3); pellet deviation ≤ 9.3 m·tan 1° = 0.16 m < 0.3 laterally and height
1.0 ± 0.16 < 1.42 → body only. Expect: exactly one `DamageDealt` with `headshot == false`, `killed == false`,
`target == dummy`, `damage ∈ [roll_damage(25, v, 0.0), roll_damage(25, v, 1.0)]` = [23, 28];
`100 − Health.current == damage as f32` (exact); magazine 11; one `BulletTrace`.
RED: take damage from the SMG row (band 11-13); apply the head multiplier to body hits (45-55); apply
`damage × factor` to health but write the unrolled base into the message (drop ≠ message).

**D2. `wall_between_muzzle_and_target_blocks`** (AC2). Wall centre (−20, 0.8, 29.0), size (2.0, 1.6, 0.2) (z 28.9..29.1,
top 1.6). `AimIntent.origin` (−20, 2.5, 33) at (−20, 1.0, 20). Ray 1 passes over the wall: at z 29.1 height
2.5 − 1.5·(3.9/13) = 2.05 > 1.6 → P on the dummy. Ray 2 from the muzzle (y 1.40, z 29.55): 0.45 m later height ≈1.38 < 1.6
→ wall. Expect: health 100, zero `DamageDealt`, one trace with `to.z ≈ 29.1` (±0.05). Positive control in the same test:
despawn the wall, `run_ticks(20)` (cooldown 0.3 s = 19.2 ticks), new `fire_requested` → one `DamageDealt` in [23, 28].
RED: cast ray 2 to P from ray 1 (skip the muzzle ray).

**D3. `head_sensor_doubles_damage`** (AC3). As D1, target (−20, 1.65, 20). P on the head sphere; deviation ≤ 0.16 m
around height ~1.65 keeps every point within 0.23 m of the sphere centre (< 0.35) → `HeadHitbox`. Expect one
`DamageDealt { headshot: true }`, damage ∈ [45, 55], health drop == damage. RED: drop `Has<HeadHitbox>` from resolution
(damage falls to 23-28); or move the head inside the capsule (`head_radius: 0.2` in a temp root → caught by B2
validation too).

**D4. `reload_takes_configured_time`** (AC4). Pistol, magazine 3, reserve 20; `reload_requested`; `run_ticks(1)` (tick R:
step 5 sets 1.2). By C1 order, on tick R+k `1.2 − k/64 <= 0` ⇔ k ≥ 76.8. Compute `n = ceil(reload * 64)` (= 77) in the
test from `weapons.ron`. `run_ticks(n − 1)` → still 3/20, and a `fire_requested` during that window does not fire (no
`ShotFired`); `run_ticks(1)` → 12/11. RED: finish the reload immediately; remove the `reload_left > 0` check in
`fire_weapons`.

**D5. `shotgun_fires_ten_pellets`** (AC5). Shotgun, shooter feet (−20, 0, 22.0), aim at the chest (−20, 1.0, 20).
Muzzle z 21.55, ≈1.55 m to the dummy axis; deviation ≤ 1.55·tan 6° = 0.163 m plus the muzzle's lateral offset toward
the axis ≈0.06 → ≤ 0.22 < 0.3; height within 0.3..1.42 → all pellets hit the body, distance < `falloff.start`.
Expect: `traces().len() == pellets` (10), `dealt().len() == 10`, every damage ∈ [7, 9], health drop == sum of the
ten damages (sum ∈ [70, 90], dummy stays alive). RED: loop one pellet.

**D6. `spread_grows_in_series_and_recovers`** (AC6). SMG, shooter standing, aim into empty space (−Z), `fire_held = true`.
dt = 1/64: cooldown 0.08 − k/64 ≤ 0 ⇔ k ≥ 5.12 → a shot every 6 ticks; recovery needs `since_shot ≥ 0.15` ⇔ k ≥ 9.6 →
none between shots. After shot n `bloom = min(0.4n, 4.0)`: strictly rising to shot 10, then 4.0 (tolerance 1e-4);
`spread_deg == base + bloom` (tolerance 0.01 for residual `|v_xz|`). Then `fire_held = false`: after the last shot,
bloom is 4.0 on k = 9, 3.875 on k = 10, 0 on k = 41 (4.0 − 32·0.125). The test derives these ticks from `weapons.ron`.
Unit `cone_sample`: 1000 samples with α = 6° → angle ≤ α + 1e-4; α = 0 → the axis. RED: remove `recovery_delay`;
remove `bloom +=`.

**D7. `head_hitbox_is_not_ground`** (GDD "check first that Tnua's ground sensor does not see it"). Fixture: static
cuboid 2 × 1.0 × 2 centred at (−20, 1.3, 25) (top 1.8) with a child `character::head_hitbox(cfg)` (centre 1.85, top
2.2). Place the player above the cuboid, `run_ticks(128)`. Expect `y ≈ 1.8 + float_height = 2.85` (±0.05); if Tnua sees
the head: 2.2 + 1.05 = 3.25. The vendored Tnua sensor skips the head by two independent guards (`Sensor`,
`vendor/bevy-tnua-avian3d-0.12.1/src/lib.rs:248,307`; non-interacting layers, `:295-307`). One-factor probes, recorded
in the stage summary: remove only `Sensor` → still GREEN (layers guard); default layers only → still GREEN (sensor
guard). Flip-RED: a fixture copy with **both** removed (plain collider, default layers) → body rests ≈3.25 → RED. This
is the answer to V2 finding 8: one-at-a-time does not flip a gate with two redundant guards; the probes document each
guard, the double removal proves the gate can fail.

**D8. `tests/respawn.rs::damage_queued_during_wasted_is_dropped`** (B1). `headless_app()`, `settle`, `kill`, run
`Wasted` until the update before exit; write **one** `write_damage(30)` only before the final `Wasted` update (probe
setup). After entering `Playing`: `health == (max, 0 armour)` immediately and after `run_ticks(4)`. RED: remove
`drop_queued_damage` → 70. In `death_wasted_respawn_at_hospital` replace the stand-in `struct Loadout(u32)`
(`tests/respawn.rs:16`, inserted `:90`, asserted `:126`) with `gta_sim::combat::Loadout` (pistol held, 7/20) and compare
before/after; extend `world_counts` (`:188`) with `Dummy` and `WeaponPickup` (one-shot range on city seed 1).

**D9. `aiming_turns_body_and_caps_speed`** (strafe + Q4). (a) `MoveIntent.axis = (0,1)`, yaw 0 (moves −Z),
`AimIntent { direction: (−1,0,0), aiming: true }`, 64 ticks: `Rotation * −Z · (−1,0,0) > 0.99` and displacement along −Z.
(b) `gait = Sprint`, aiming, 128 ticks: horizontal speed ≤ `run_speed + 0.05`; same without aiming → > `run_speed + 0.5`
(positive control). RED: remove the aiming facing branch; remove the gait clamp.

**D10. `weapon_and_ammo_pickups`** and **`dummy_dies_and_resets`**: `WeaponPickup { Pistol, ammo_only: false }` at
(−20, 0, 35), stand on it → `owned`, magazine `min(12, 24) = 12`, reserve 12, `held == Some(Pistol)`, cooldown > 0; ammo
pickup → reserve +24; `run_ticks(respawn·64 + 2)` → available again. Dummy: set `Health.current = 0` → one tick → `Dead`;
fire at its head height: no `DamageDealt`, trace passes through where the head was (hits the wall/floor or nothing, not
`HeadHitbox`); after `ceil(dummy_reset·64)` ticks → full health, no `Dead`, and a head shot again yields `headshot: true`.
RED: remove the `dead.contains` term from the predicate (trace stops at the dead head).

**D11. `semi_auto_vs_automatic`** (Q3). Pistol: `fire_held = true` + one `fire_requested` → exactly 1 shot in 64 ticks;
second `fire_requested` 5 ticks after the first (inside cooldown) → discarded, still 1; new `fire_requested` after 20
ticks → 2. SMG: `fire_held` for 64 ticks → shots at ticks 0, 6, 12, … = `1 + floor(63/6)` = 11 (derive from data).
RED: make pistol fire from `fire_held`.

**D12. `damage_variance_stays_in_band_and_varies`** (owner addition). 24 pistol body shots at the D1 setup (reset the
dummy to full health with `set_health_of` before each, `run_ticks(20)` between shots): every damage ∈ [23, 28],
every health drop == its message, at least 2 distinct values. Shotgun: 3 blasts at the D5 setup (30 pellets): every
damage ∈ [7, 9], at least 2 distinct values. Deterministic (`CombatRng` seed 0). RED: pass `u = 0.5` instead of a draw
(all equal → "distinct" fails); multiply `variance` by 3 (values leave the band).

**D13. Unit tests in modules**: `cycle_weapon` (owned {Pistol, Shotgun}: None +1 → Pistol; Shotgun +1 → None; None −1 →
Shotgun); `falloff_factor` shotgun (5 m → 1.0; 17.5 m → 0.65; 25 m → 0.3); `muzzle` (three C2 examples); `acquire`;
`roll_damage` table: (25, 0.1, 0.0) → 23, (25, 0.1, 0.5) → 25, (25, 0.1, 0.999999) → 27 or 28 (≤ 28), (50, 0.1, 0.0) → 45,
(8, 0.1, 0.0) → 7, (8, 0.1, 0.999999) → 9, (25, 0.0, any u) → 25 (the table value is the band centre), (0.4, 0.1, 0.0) → 1
(floor at 1); `Health::take` (lethal returns true once, a second hit on 0 HP returns false).

**D14. `tests/config.rs`**: `shipped_weapons_config_loads`, `shipped_aim_config_loads`, `head_must_stick_out_of_capsule`
(temp root with `head_radius: 0.2` → error mentions `head_radius`), `damage_variance_below_one` (temp root with the
pistol's `damage_variance: 1.0` → error mentions `pistol.damage_variance`), `reload_must_be_positive` (`reload: 0.0` →
`pistol.reload`). Template: `health_error` in `tests/config.rs`. Flip each with a sabotage yielding a DIFFERENT error.

Gate classes: D1-D5, D8, D12 correctness (numbers derived); D6 correctness of spread arithmetic + cone bound; D7
correctness of Tnua filtering; D9-D11 behavioural correctness; D13/D14 unit/config.

### E. Client (`src/`)

**E1. `camera/config.rs` + `camera/mod.rs`**:
- 5 fields from A4 in `CameraConfig` + `validate` (positive; `aim_sensitivity_scale ∈ (0, 1]`).
- `OrbitCamera`: add `aim_blend: f32` (0..1).
- `apply_mouse_look`: sensitivity × `lerp(1, aim_sensitivity_scale, ease(aim_blend))`.
- `follow_player`: move `aim_blend` toward the player's `AimIntent.aiming` at `1/aim_transition` per real second,
  ease-out `1 − (1 − t)²`; interpolate shoulder, distance and FOV (`Projection::Perspective`).
  Both `cast_shape` filters → `SpatialQueryFilter::from_mask(GameLayer::World).with_excluded_entities([entity])`
  (**mandatory in the same change as B3**: the pivot at 1.55 m sits inside the head sphere centred at 1.6 m with r 0.35,
  so an all-layer cast hits at distance 0 and the camera collapses; characters no longer push the camera — TPS convention).
- After positioning, write `AimIntent.origin = camera translation`, `direction = rotation * −Z` **before** applying
  recoil; then `camera_transform.rotation = rotation * Quat::from_rotation_x(recoil.pitch)` (visual only, GDD §4.1).

**E2. `input/mod.rs`**: actions in `OnFoot`: `Fire` (`MouseButton::Left`, bool), `Aim` (`MouseButton::Right`, bool),
`Reload` (`KeyCode::KeyR`), `Slot1..Slot4` (`Digit1..Digit4`), `CycleWeapon` (`f32`, `(Binding::mouse_wheel(),
SwizzleAxis::YXZ)`, `bevy_enhanced_input-0.26.0/src/binding.rs:61-94`). System `write_action_intent` in `Update`,
ordered `.after(apply_mouse_look).before(cursor_toggle)` so the click that recaptures the cursor is seen while
`CursorCaptured` is still false. When `!CursorCaptured`: set `fire_held = false`, `AimIntent.aiming = false`, raise no
latches, and remember (a `Local<bool>`) that Fire must be released once before it counts again (otherwise the held
capture click would start SMG fire). When captured: `fire_held = ***fire`; `ActionEvents::START` → `fire_requested` /
`reload_requested = true` (same pattern as `write_move_intent`'s jump); `Slot1` → `select = Some(Unarmed)`, `Slot2..4` →
`Gun(..)`; `cycle += sign(wheel)`; `aiming = ***aim`. Latches are only raised here; `FixedUpdate` clears them.
Also clear `fire_held`/`aiming` on `OnEnter(GameState::Wasted)` (V2 finding 11).

**E3. `src/juice/mod.rs`** (new) + `juice/config.rs` `JuiceConfig` (`juice/juice.ron`, `deny_unknown_fields`, `validate`):
resource `CameraRecoil { pitch }`; `Update` system reads `ShotFired` of the player → `pitch += recoil_deg(weapon).to_radians()`,
then `pitch.smooth_nudge(&0.0, LN_2 / recoil_half_life, real_dt)`. The camera (E1) reads the resource.

**E4. `src/juice/damage_numbers.rs`** (new; owner addition). Choice: **screen-space UI label anchored to the projected
world point**, not a world-space billboard. Reason: Bevy 0.19.1 has no 3D text (`Text2d` renders only through a 2D
camera, `bevy_sprite-0.19.1/src/text2d.rs:102`; grep for a 3D text/billboard type in bevy_* 0.19.1 finds none), so a
billboard would need a new crate or a text-to-texture pipeline; a UI node always faces the camera, keeps a readable
pixel size at any distance and reuses the HUD font.
- Component `#[derive(Component, Reflect)] #[reflect(Component)] pub struct DamageNumber { pub point: Vec3, pub value: u32,
  pub headshot: bool, pub age: f32, pub drift: f32 }` (`drift ∈ [−1, 1]`), `register_type` (BRP reads it in t6).
- `pub fn label_text(value: u32, headshot: bool, crit_template: &str) -> String` — body `"{value}"`; head
  `crit_template.replace("{damage}", &value.to_string())` → `"50 CRIT"`.
- `pub fn pose(age: f32, cfg: &DamageNumbersConfig) -> (Vec2, f32, f32)` (offset px, scale, alpha), `n = (age/lifetime).clamp(0,1)`:
  `rise = rise_px·(1 − (1 − n)³)`; offset = `(drift·drift_px·n, −rise)` (viewport y grows downward);
  `scale = if age < pop_seconds { 1 + (pop_start_scale − 1)·(1 − age/pop_seconds)² } else { 1 }`;
  `alpha = if n < fade_start { 1 } else { 1 − (n − fade_start)/(1 − fade_start) }`.
  Worked examples (shipped data, drift = 1): age 0 → offset (0, 0), scale 1.8, alpha 1; age 0.04 → scale 1 + 0.8·0.25 = 1.2;
  age 0.4 (n 0.5) → offset (12, −52.5), scale 1, alpha 1; age 0.6 (n 0.75) → offset (18, −59.06), alpha 0.5;
  age 0.8 → offset (24, −60), alpha 0.
- `spawn_damage_numbers` (`Update`): `MessageReader<DamageDealt>`, keep messages whose `shooter` has `Player`; for each
  spawn `(DamageNumber { point, value: damage, headshot, age: 0, drift: (k as f32 * GOLDEN_ANGLE).sin() }, Text(label_text(..)),
  TextFont { font: FontSource::Handle(fonts.regular.clone()), font_size: FontSize::from(size or crit_size), ..default() },
  TextColor(color or crit_color), Node { position_type: PositionType::Absolute, ..default() },
  UiTransform { translation: Val2::percent(-50, -50), scale: Vec2::splat(pop_start_scale), ..default() },
  Visibility::Hidden)`. `k` is a `Local<u32>` counter; `GOLDEN_ANGLE = 2.399_963` rad is a math constant (not
  tuning) so consecutive numbers drift to well-separated sides without a client RNG crate. `Val2::percent` in
  `UiTransform.translation` resolves against the node's own layout size (`bevy_ui-0.19.1/src/layout/mod.rs:294` passes
  `layout_size`), which centres the label on its anchor. Scale is animated through `UiTransform.scale`, never
  `font_size` (a new font size builds a new glyph atlas, `bevy_text-0.19.1/src/text.rs:385-392`).
- `age_damage_numbers` (`Update`): `age += Time<Real>.delta_secs()`; `age >= lifetime` → `commands.entity(e).despawn()`.
  Independent of the camera, so the leak gate runs headless.
- `place_damage_numbers` (`PostUpdate`, `.after(follow_player).after(CameraUpdateSystems).before(UiSystems::Layout)`):
  get the camera with `Query<(&Camera, &Transform), With<OrbitCamera>>` + `let Ok(..) = .single() else { return }`;
  the camera is a root entity, so `GlobalTransform::from(*transform)` is this frame's pose (its `GlobalTransform` is
  propagated later). `camera.world_to_viewport(&global, point)` (`bevy_camera-0.19.1/src/camera.rs:579`, logical px,
  top-left origin — the same space as `Node.left/top`): `Err(_)` (behind the camera, `NoViewportSize`) →
  `Visibility::Hidden`; `Ok(p)` → `left = px(p.x + off.x)`, `top = px(p.y + off.y)`, `UiTransform.scale = splat(scale)`,
  `TextColor` alpha, `Visibility::Inherited`. Verify `UiSystems` and `CameraUpdateSystems` paths (`bevy::ui::UiSystems`,
  `bevy::camera::CameraUpdateSystems`) at compile time.
- `DamageNumbersConfig` is the `damage_numbers` section of `JuiceConfig`; `validate`: `lifetime > 0`,
  `0 < pop_seconds < lifetime`, `pop_start_scale > 0`, `rise_px >= 0`, `drift_px >= 0`, `0 <= fade_start < 1`,
  `size > 0`, `crit_size >= size`, colours in [0, 1]. `UiConfig.validate` checks `damage_crit` contains `{damage}`.
- Shotgun: one number per pellet (up to 10 small numbers per blast), literal to the owner's request; aggregation per
  target is an owner-checklist question, not implemented.

**E5. `src/vfx/mod.rs`** (new): shared handles (unit cuboid, unlit flash and tracer materials) in a `FromWorld`
resource; `ShotFired` → flash quad at the muzzle + `PointLight { intensity, range, shadow_maps_enabled: false, .. }` for
`flash.seconds`; `BulletTrace` → cuboid stretched `from → to` (`Transform::looking_to` + scale (width, width, len)) for
`tracer.seconds`; `Lifetime(f32)` component ticked on `Time<Real>`, despawn at 0. No pool: ≤ ~20 short-lived entities/s
with shared handles (GDD §11 "measure, don't guess").

**E6. `src/audio/mod.rs`** (new) + `MixConfig` (`audio/mix.ron`): `#[derive(Asset, TypePath)] struct ShotSound {seconds, decay, volume}`
with `Decodable` → `NoiseBurstDecoder` (xorshift32 noise × `exp(−decay·t)`, mono, 44 100 Hz — technical `const`, ends
with `None` after `seconds·rate` samples, `total_duration = Some`). API model: `bevy-0.19.1/examples/audio/decodable.rs`
(`ChannelCount::new(1)`, `SampleRate::new(44_100)`, `Source::{current_span_len, channels, sample_rate, total_duration}`;
rodio 0.22.2 `SampleRate = NonZero<u32>`, `ChannelCount = NonZero<u16>`, `rodio-0.22.2/src/common.rs:5,8`).
`app.add_audio_source::<ShotSound>()`; three handles in a resource; `ShotFired` → `commands.spawn((AudioPlayer(handle),
PlaybackSettings::DESPAWN.with_volume(Volume::Linear(volume))))` (`bevy_audio-0.19.1/src/audio.rs:106,130`).

**E7. `src/hud/weapon.rs`** (new) + `hud/mod.rs` + `menu/config.rs` (`HudLayout` fields from A7 + `validate`, pattern
`menu/config.rs:60-97`): on `OnTransition { Loading → Playing }` — ammo text under the bars (`UiFonts.regular`,
`"{magazine} / {reserve}"`, hidden without a gun, `set_if_neq`); centre crosshair (dot `crosshair_dot`; when
`aim_blend > 0` four arms at gap `px = tan(spread_deg.to_radians()) / tan(fov/2) · (window_height / 2)` — radians fix,
V2 finding 4); hit marker (`Text "×"`, `hit_marker_size`, white or `kill_marker_color`, hidden after `hit_marker_seconds`
on `Time<Real>`) driven by the player's `DamageDealt` (`killed` → red).

**E8. `src/visuals/weapons.rs`** (new) + `visuals/config.rs` (section `weapons`, A8, `validate`): observer
`On<Add, WeaponPickup>` → cuboid `pickup_size` (gun) or cube `ammo_size` (ammo) coloured per weapon/ammo, visibility by
`available()` (as `visuals/pickups.rs:31-41`); held gun = child cuboid `held_size` of the player at
`AimConfig.muzzle_offset + (0, 0, held_size.z/2)`, colour by `Loadout.held`, hidden without a gun. Dummies get the
Kenney humanoid for free (`visuals/character.rs` observer on any `CharacterBody`); `character_gate.rs` still expects one
`CharacterModel` in `TestArea` — the range spawns only in `City`, so it stays green.

**E9. `main.rs`**: load `JuiceConfig`, `MixConfig` (pattern `:142-170`), validate them in `preflight`, insert
resources, add `juice::JuicePlugin` (recoil + damage numbers), `vfx::VfxPlugin`, `audio::ShotAudioPlugin`, `hud` weapon
parts to the plugin tuple at `:186-193`; `mod juice; mod vfx; mod audio;`.

**E10. Client presentation gate** `src/juice/damage_numbers_gate.rs` (`#[cfg(test)]`, runs in
`cargo test -p gta_like --bin gta_like`): headless `App` with `MinimalPlugins`, `TimeUpdateStrategy::ManualDuration(1/60 s)`
(advances `Time<Real>`, `bevy_time-0.19.1/src/lib.rs:180`), the production `JuicePlugin` (damage-number part),
`add_message::<DamageDealt>()` as the sim stand-in, shipped `JuiceConfig`/`UiConfig` from `assets/`, `UiFonts` with
default handles, one `Player` entity as shooter; no camera (placement is skipped, aging still runs).
- `label_matches_message`: write `DamageDealt { damage: 27, headshot: false }` and `{ damage: 54, headshot: true }`,
  update → `Text` values `"27"` and `"54 CRIT"`, `TextColor` = `color` / `crit_color`, `TextFont.font_size` = `size` /
  `crit_size`, `DamageNumber.value` == message damage. RED: render `value + 1` or ignore `headshot`.
- `numbers_despawn_after_lifetime` (owner "no leak"): write 200 messages over 20 updates, then update until
  `lifetime + 0.1 s` of real time past the last write → `count::<With<DamageNumber>>() == 0`; mid-way count > 0 (liveness).
  RED: remove the despawn.
- `pose_worked_examples`: the five E4 examples (tolerance 1e-3).
- A message from a non-player shooter spawns nothing.

### F. Runtime QA (GDD D1)

**F1. `tools/qa/brp.py`**: `send_mouse_button(self, button, ms)` → `brp_extras/send_mouse_button` with
`{"button": "Left"|"Right", "duration_ms": ms}` (`bevy_brp_extras-0.22.6/src/mouse/button.rs:26-37`, default 100 ms).

**F2. `tools/qa/scenarios/t6.py`** (pattern `t5.py`: release, `features=("dev",)`, `--seed 1`, `--out`):
1. `CityLayoutHash` == golden; chunks settled. Read `CityLandmarks.park_center`, dummies (`Dummy` + `Position` +
   `Health`, expect `range.dummies` = 3), `WeaponPickup` count 6.
2. Teleport onto the pistol pickup → 0.5 s → `Loadout.guns[0].owned`, `held` = Pistol (tolerant `Option` parser: `"None"`,
   `{"Some": x}` or `null`).
3. Teleport 5 m in front (+Z) of the middle dummy, wait 1 s. Aim with `move_mouse`: read the player's `AimIntent`,
   compute Δyaw/Δpitch to the chest (1.0 m above feet), `dx = −Δyaw° / 0.12`, `dy = −Δpitch° / 0.12` (`camera.ron`
   sensitivity; ×1/0.7 while RMB is held), 2-3 iterations until the `AimIntent` ray passes < 0.1 m from the dummy axis at
   chest height. Aim is corrected **before** firing, never after reading the result.
4. Body shot: `send_mouse_button("Left", 100)`; within ~0.2 s screenshot `body_hit.png`; query `DamageNumber`:
   exactly one with `headshot == false`. Hard pass/fail: magazine 11; dummy health drop ∈ [23, 28] (band from
   `weapons.ron` via the same formula) **and** == `DamageNumber.value` (end-to-end "applied == shown").
5. Head shot: re-aim at 1.7 m above the feet, wait `fire_interval`, `send_mouse_button("Left", 100)`; screenshot
   `crit_hit.png`; a `DamageNumber` with `headshot == true` exists; health drop ∈ [45, 55] and == its value. If the
   first try is a body hit (aim error), re-aim once more; fail after 3 tries.
6. Teleport onto the SMG pickup, `send_keys(["Digit3"], 100)`, re-aim at the chest; `send_mouse_button("Right", 4000)`,
   after 0.5 s `send_mouse_button("Left", 800)`, screenshots at ~0.25 s and ~0.45 s into the burst (`aim_burst_1.png`,
   `aim_burst_2.png`, tracers 60 ms every 80 ms, hit markers 0.1 s); `AimIntent.aiming == true` while held; magazine
   dropped by > 1; dummy health dropped (or dummy is `Dead`).
7. `send_keys(["KeyR"], 100)`; after `reload + 0.5` s the SMG magazine is full.
8. After 2 s: `DamageNumber` count == 0 (runtime no-leak). Log has no asset/font errors; `shutdown`; `summary.json`
   with all numbers.
Screenshots are owner/multimodal evidence; the hard pass/fail rests on the numbers. Run `t5.py` again (pickups/B1
regression).

**F3. Owner checklist (QA writes it into `QA_REPORT.md`)**: `cargo run --features fast`, park at the city centre: pick up
pistol/SMG/shotgun, fire with and without RMB; recoil noticeable, not nauseating; placeholder sound tolerable; tracer
and flash readable; hit marker and red kill marker visible; strafe while aiming comfortable; aim camera (shoulder, 2 m,
FOV 55°) acceptable; dummies drop to 0 and come back; damage numbers: readable size, pop/rise/fade timing, drift
spacing, CRIT readable and red; shotgun's 10 numbers per blast acceptable or should be summed per target. Knobs:
`weapons.ron`, `aim.ron`, `camera.ron`, `juice.ron`, `mix.ron`, `strings.ron`, `locomotion.ron`.

### G. Completion

`cargo build`; `cargo clippy -- -D warnings`; `cargo test -p gta_sim` (all old + D1-D14); `cargo test -p citygen` (not
touched, run anyway); `cargo test -p gta_like --bin gta_like` (existing gates + E10; `character_model_spawns_under_player`
stays green); `cargo tree -p gta_sim -e normal -i bevy_render` empty; `python tools/qa/tree_check.py`;
`python tools/qa/scenarios/t6.py`, `python tools/qa/scenarios/t5.py`. Grep for new `const`: allowed only `GameLayer`,
`WEAPONS_CONFIG`/`AIM_CONFIG`/config paths, sample rate 44 100, the `CombatRng` seed, `GOLDEN_ANGLE`, `unit_f32`'s 2^24.
No `cargo test --workspace` (feature unification) — `-p` only.

---

## 3. Test plan

| Claim | Gate | Class | Perturbation for RED |
|---|---|---|---|
| AC1 pistol at 10 m → table damage (band 23-28), drop == message | D1 | correctness | SMG row; head multiplier on body; unrolled value in message |
| AC2 wall blocks muzzle ray | D2 (+ positive control) | correctness | ray 2 replaced by P |
| AC3 head sensor ×2 (45-55) | D3 | correctness | drop `Has<HeadHitbox>`; head inside capsule |
| AC4 reload by time (77 ticks) | D4 | correctness | instant reload; no `reload_left` check |
| AC5 shotgun 10 rays | D5 | correctness | 1 pellet |
| AC6 spread grows / recovers | D6 + cone unit | correctness | no delay; no bloom |
| GDD: Tnua ignores head | D7 (+ 2 one-factor probes) | correctness | remove Sensor and layers |
| B1 queued damage dropped | D8 | correctness | remove `drop_queued_damage` |
| Q4 aim cap + strafe | D9 | behaviour | no clamp; no facing branch |
| pickups; dead head invisible; reset | D10 | behaviour | remove `dead.contains` |
| Q3 semi vs auto | D11 | behaviour | pistol fires from held |
| Owner: variance in band, varies, per pellet | D12, D13 `roll_damage` | correctness | fixed u; ×3 variance |
| Config strictness | D14 | correctness | different-error sabotage |
| Owner: shown text == message, CRIT/red, no leak | E10 | correctness + liveness | value+1; ignore headshot; no despawn |
| Runtime: AC runtime + numbers visible | F2 | end-to-end | — (numbers are hard pass/fail) |
| Feel, animation, readability | F3 | owner run | — |

Expected outcomes: all gates GREEN on shipped data; each RED under its named perturbation; the implementer records the
perturbation and RED/GREEN in the stage summary. If any expected number differs from the derivation above after
implementation, recompute it through the actual code path; do not tune the test to the observed value.

## 4. Rollout notes

- No migrations, save data or feature flags. New data files: `assets/combat/weapons.ron`, `assets/combat/aim.ron`,
  `assets/juice/juice.ron`, `assets/audio/mix.ron`; extended: `locomotion.ron`, `camera.ron`, `strings.ron`,
  `render.ron`. Strict loaders (`deny_unknown_fields`) mean the data and code must land in the same commit.
- New dependency: `rand_chacha =0.10.0` in `gta_sim` (already locked for `citygen`); render-free check stays empty.
- Behaviour changes to existing systems: characters get `CollisionLayers(Character, ALL)` and a child head sensor —
  character↔world physics unchanged (`Character/ALL` vs default `World/ALL` interact); camera collision now ignores
  characters (world only); `drive_characters` caps sprint while aiming; `apply_debug_damage` goes through `Health::take`
  (same arithmetic). Existing `settle()`, `jump.rs`, `movement.rs`, `respawn.rs` must stay green.
- BRP: `AimIntent`, `ActionIntent`, `Loadout`, `GunSlot`, `Weapon`, `WeaponPickup`, `Dummy`, `DamageDealt`,
  `DamageNumber` are reflected and registered.
- Risks to watch: fixed ticks per frame 0 or 2+ (latches consumed once); reading `BulletTrace`/`DamageDealt` right after
  the firing tick in tests (buffers rotate); first `update()` under `FixedTimesteps` runs 0 ticks (use `run_ticks`);
  Kenney Mini Characters have no strafe clips (legs run forward while strafing — owner-visible, separate task);
  transient 60 ms tracers may miss a screenshot (retake, not a failure).

## 5. Review notes (changes against PLAN_V2 / PLAN.md)

Disconfirmation tested first: "the plan's exact-damage gates survive the owner's damage variance." Counter-example:
with `damage_variance` the D1 assertion `Health == 75`, D3 `== 50`, D5 `== 20` (PLAN.md) and V2 step 11 "checks
expected target damage" at runtime all become false on most hits. Searched: `TASK_FINAL.md` owner addition requires
per-hit variance; neither PLAN.md nor PLAN_V2.md mentions variance, `DamageDealt`, or damage numbers. **It held.**
Fix: bands derived from `roll_damage(base, variance, u ∈ {0, 1})` through the real formula, plus the exact identity
"health drop == message damage", which is falsifiable because the drop comes from `Health` and the message from the
roll (a defect that applies one value and reports another moves only one side). Second check: "applied == shown" fails
for armour or overkill — defined `damage` as the pre-armour amount passed to `Health::take` and gated only non-lethal
hits on armour-free dummies (dummies do not regenerate: `regenerate_player` is `With<Player>`, verified).

Folded in (owner addition, absent from both plans): `damage_variance` per weapon in `weapons.ron`; `roll_damage` →
`u32`; `CombatRng` (renamed from `SpreadRng`, now also drives damage); `DamageDealt` message (replaces `BulletHit`,
single source of `killed`; hit marker reads it); client damage numbers (E4) with UI-label choice verified in Bevy 0.19.1
source (`Camera::world_to_viewport` at `bevy_camera-0.19.1/src/camera.rs:579`, `UiTransform` at
`bevy_ui-0.19.1/src/ui_transform.rs:130`, percent translation against node size, `UiSystems` chain in
`bevy_ui-0.19.1/src/lib.rs:147-157`, `TextFont.font: FontSource` as used in `src/hud/wasted.rs:24-27`); `juice.ron`
`damage_numbers` section; `strings.ron` `damage_crit`; gates D12, D13 `roll_damage`, E10; runtime steps F2.4/5/8.

Kept from V2 (verified against code): Q1-Q4 resolved answers bind (open questions removed); `aim_max_gait` cap in the
sim (`drive_characters` uses `cfg.speed(intent.gait)`, `character/mod.rs:160`); `reload > 0`; no
`recovery_delay > fire_interval` rule; radians in the crosshair formula; B1 gate with a single last-frame message; use
`tests/common` production composition; clear held intents on cursor release/Wasted; runtime shot at close range with
aim corrected before firing; park lane verified before placement.

Changed from V2:
- V2 step 3 "remove/restore head query visibility with life state" → replaced by a ray predicate that ignores a head
  whose body has `Dead`; nothing to restore on reset/respawn, no per-death component mutation.
- V2 finding 8 (D7 "one change at a time") → with two redundant Tnua guards a single removal cannot flip the gate; kept
  one-factor runs as documentation probes and the double removal as the flip-RED.
- V2 finding 12 → replaced `head_radius > capsule_radius` with the property-based rule `head_height − head_radius <
  top − capsule_radius` plus the existing bounds.
- Cooldown/reload policy for an early press made explicit: discarded (V2 left it open).
- Capture-click ordering: `write_action_intent.before(cursor_toggle)` + release-before-fire latch (V2 only said "clear
  held states").

Restored from PLAN.md (V2 compressed without correcting): full RON contents A1-A8, all type/field lists C1-C5, tick order,
three muzzle examples, cone formula, gate geometry and tick derivations D1-D11, camera/input/juice/vfx/audio/HUD/visuals
details E1-E9, BRP method shape, scenario steps, owner checklist, completion commands.

Corrected in PLAN.md line references: `tests/respawn.rs` stand-in `Loadout` is at `:16/:90/:126` (not `:12-14/:481/:517`).

Unverified, flagged for the implementer: exact module path of `CameraUpdateSystems`/`UiSystems` re-exports under
`bevy::` (compile-time check); whether `Query<&mut Health, Without<Dead>>` conflicts with the `shooters` query in
`fire_weapons` (it does not include `Health`, so it should not; use `ParamSet` if Bevy reports B0001).

children: 0 launched / 0 reported.
