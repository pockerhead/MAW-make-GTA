use super::AttackSerial;
use super::melee::HitReaction;
use super::weapons::{
    FireMode, Loadout, Weapon, WeaponsConfig, falloff_factor, roll_damage, spread_deg,
};
use crate::character::{ActionIntent, AimIntent, Character, Dead, HeadHitbox, Health};
use crate::layers::GameLayer;
use avian3d::prelude::*;
use bevy::prelude::*;
use rand_chacha::{
    ChaCha8Rng,
    rand_core::{Rng, SeedableRng},
};
use serde::Deserialize;

/// Path of the aim config, relative to the assets root.
pub const AIM_CONFIG: &str = "combat/aim.ron";

/// Two-ray hitscan geometry (GDD §4.1).
#[derive(Resource, Deserialize, Clone, Debug)]
#[serde(deny_unknown_fields)]
pub struct AimConfig {
    /// Length of the camera ray that finds the aim point, m.
    pub max_aim_distance: f32,
    /// An aim point closer than this in front of the muzzle fires along the aim direction, m.
    pub min_aim_distance: f32,
    /// Muzzle relative to the body centre in the aim-yaw frame (x right, y up, z forward = −Z).
    pub muzzle_offset: (f32, f32, f32),
}

impl AimConfig {
    pub fn validate(&self) -> Result<(), String> {
        let (x, y, z) = self.muzzle_offset;
        if ![self.max_aim_distance, self.min_aim_distance, x, y, z]
            .iter()
            .all(|v| v.is_finite())
        {
            return Err("aim values must be finite".into());
        }
        if self.max_aim_distance <= 0.0 {
            return Err("max_aim_distance is out of range".into());
        }
        if self.min_aim_distance < 0.0 {
            return Err("min_aim_distance is out of range".into());
        }
        Ok(())
    }

    pub fn muzzle_offset(&self) -> Vec3 {
        let (x, y, z) = self.muzzle_offset;
        Vec3::new(x, y, z)
    }
}

/// One trigger pull that fired (flash, sound, recoil).
#[derive(Message, Reflect, Clone, Copy, Debug)]
#[reflect(Message)]
pub struct ShotFired {
    pub shooter: Entity,
    pub weapon: Weapon,
    pub muzzle: Vec3,
}

/// One pellet's path from the muzzle to where it stopped (tracer).
#[derive(Message, Reflect, Clone, Copy, Debug)]
#[reflect(Message)]
pub struct BulletTrace {
    pub shooter: Entity,
    pub from: Vec3,
    pub to: Vec3,
}

/// One hit that damaged a live target. `damage` is exactly the amount passed to `Health::take`
/// (before armour); hit markers and damage numbers show it, never recompute it.
#[derive(Message, Reflect, Clone, Copy, Debug)]
#[reflect(Message)]
pub struct DamageDealt {
    pub shooter: Entity,
    /// Attack (trigger pull or melee swing) that dealt it: every pellet of one shotgun blast
    /// carries the same value.
    pub shot: u32,
    pub target: Entity,
    pub point: Vec3,
    pub damage: u32,
    pub headshot: bool,
    pub killed: bool,
}

/// Sim-owned RNG of spread and damage rolls; seeded, so a run is reproducible.
#[derive(Resource)]
pub struct CombatRng(pub ChaCha8Rng);

impl CombatRng {
    pub fn seeded(seed: u64) -> Self {
        Self(ChaCha8Rng::seed_from_u64(seed))
    }
}

/// Uniform in `[0, 1)` from the top 24 bits.
fn unit_f32(rng: &mut impl Rng) -> f32 {
    (rng.next_u32() >> 8) as f32 * (1.0 / 16_777_216.0)
}

/// Yaw (about +Y) that turns −Z towards `direction`.
pub fn aim_yaw(direction: Vec3) -> f32 {
    f32::atan2(-direction.x, -direction.z)
}

/// Muzzle position for a body at `position` aiming along `direction`.
pub fn muzzle(position: Vec3, direction: Vec3, offset: Vec3) -> Vec3 {
    position + Quat::from_rotation_y(aim_yaw(direction)) * offset
}

/// A direction inside the cone of `half_angle` radians around `axis`; `u`, `v` ∈ [0, 1) give a
/// uniform distribution over the cone's solid angle.
pub fn cone_sample(axis: Dir3, half_angle: f32, u: f32, v: f32) -> Dir3 {
    let cos_theta = 1.0 - u * (1.0 - half_angle.cos());
    let sin_theta = (1.0 - cos_theta * cos_theta).max(0.0).sqrt();
    let phi = std::f32::consts::TAU * v;
    let a = axis.as_vec3();
    let (b1, b2) = a.any_orthonormal_pair();
    let d = a * cos_theta + (b1 * phi.cos() + b2 * phi.sin()) * sin_theta;
    Dir3::new(d).unwrap_or(axis)
}

#[allow(clippy::type_complexity, clippy::too_many_arguments)]
pub(super) fn fire_weapons(
    cfg: Res<WeaponsConfig>,
    aim_cfg: Res<AimConfig>,
    spatial: SpatialQuery,
    mut rng: ResMut<CombatRng>,
    mut shooters: Query<
        (
            Entity,
            &Position,
            &AimIntent,
            &mut ActionIntent,
            &mut Loadout,
            &LinearVelocity,
            &HitReaction,
        ),
        (With<Character>, Without<Dead>),
    >,
    colliders: Query<(&ColliderOf, Has<HeadHitbox>)>,
    dead: Query<(), With<Dead>>,
    reactions: Query<&HitReaction>,
    mut targets: Query<&mut Health, Without<Dead>>,
    mut fired: MessageWriter<ShotFired>,
    mut traces: MessageWriter<BulletTrace>,
    mut dealt: MessageWriter<DamageDealt>,
    mut serial: ResMut<AttackSerial>,
) {
    let filter =
        SpatialQueryFilter::from_mask([GameLayer::World, GameLayer::Character, GameLayer::Hitbox]);
    for (shooter, position, aim, mut action, mut loadout, velocity, reaction) in &mut shooters {
        // A request during cooldown or reload is dropped, not buffered.
        let requested = std::mem::take(&mut action.fire_requested);
        if reaction.is_active() {
            continue;
        }
        let Some(weapon) = loadout.held else {
            continue;
        };
        let stats = cfg.stats(weapon);
        let wants = match stats.fire_mode {
            FireMode::SemiAutomatic => requested,
            FireMode::Automatic => action.fire_held || requested,
        };
        let loadout = &mut *loadout;
        let slot = &mut loadout.guns[weapon.index()];
        if !wants || loadout.reload_left > 0.0 || slot.cooldown > 0.0 {
            continue;
        }
        let Ok(dir) = Dir3::new(aim.direction) else {
            continue;
        };
        if slot.magazine == 0 {
            if slot.reserve > 0 {
                loadout.reload_left = stats.reload;
            }
            continue;
        }
        slot.magazine -= 1;
        slot.cooldown = stats.fire_interval;
        slot.recovery_wait = stats.spread.recovery_delay;
        slot.bloom_deg =
            (slot.bloom_deg + stats.spread.per_shot_deg).min(stats.spread.max_bloom_deg);
        let spread = loadout.spread_deg;
        loadout.spread_deg = spread_deg(stats, slot.bloom_deg, velocity.0);
        let shot = serial.next_id();

        // Skips the shooter's own body and head, and the head sensor of a dead or knocked-down character.
        let visible = |entity: Entity| match colliders.get(entity) {
            Ok((of, head)) => {
                of.body != shooter
                    && !(head
                        && (dead.contains(of.body)
                            || reactions.get(of.body).is_ok_and(|r| r.is_knocked_down())))
            }
            Err(_) => true,
        };
        let aim_point = spatial
            .cast_ray_predicate(
                aim.origin,
                dir,
                aim_cfg.max_aim_distance,
                true,
                &filter,
                &visible,
            )
            .map_or(aim.origin + dir * aim_cfg.max_aim_distance, |hit| {
                aim.origin + dir * hit.distance
            });
        let from = muzzle(position.0, dir.as_vec3(), aim_cfg.muzzle_offset());
        let to_aim = aim_point - from;
        let axis = if to_aim.dot(dir.as_vec3()) > aim_cfg.min_aim_distance {
            Dir3::new(to_aim).unwrap_or(dir)
        } else {
            dir
        };
        fired.write(ShotFired {
            shooter,
            weapon,
            muzzle: from,
        });
        for _ in 0..stats.pellets {
            let (u, v) = (unit_f32(&mut rng.0), unit_f32(&mut rng.0));
            let d = cone_sample(axis, spread.to_radians(), u, v);
            let Some(hit) =
                spatial.cast_ray_predicate(from, d, stats.range, true, &filter, &visible)
            else {
                traces.write(BulletTrace {
                    shooter,
                    from,
                    to: from + d * stats.range,
                });
                continue;
            };
            let point = from + d * hit.distance;
            traces.write(BulletTrace {
                shooter,
                from,
                to: point,
            });
            let (target, headshot) = colliders
                .get(hit.entity)
                .map_or((hit.entity, false), |(of, head)| (of.body, head));
            // `current > 0` covers a target killed earlier this tick whose `Dead` is still deferred.
            let Ok(mut health) = targets.get_mut(target) else {
                continue;
            };
            if health.current <= 0.0 {
                continue;
            }
            let multiplier = if headshot {
                cfg.headshot_multiplier
            } else {
                1.0
            };
            let base = stats.damage * falloff_factor(stats, hit.distance) * multiplier;
            let damage = roll_damage(base, stats.damage_variance, unit_f32(&mut rng.0));
            let killed = health.take(damage as f32);
            dealt.write(DamageDealt {
                shooter,
                shot,
                target,
                point,
                damage,
                headshot,
                killed,
            });
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn muzzle_follows_aim_yaw() {
        let offset = Vec3::new(0.25, 0.35, -0.45);
        for (direction, expected) in [
            (Vec3::NEG_Z, Vec3::new(0.25, 0.35, -0.45)),
            (Vec3::NEG_X, Vec3::new(-0.45, 0.35, -0.25)),
            (Vec3::Z, Vec3::new(-0.25, 0.35, 0.45)),
        ] {
            let got = muzzle(Vec3::ZERO, direction, offset);
            assert!((got - expected).length() < 1e-5, "{direction}: {got}");
        }
    }

    #[test]
    fn cone_samples_stay_inside_the_cone() {
        let mut rng = ChaCha8Rng::seed_from_u64(7);
        let axis = Dir3::new(Vec3::new(0.3, -0.2, -1.0)).unwrap();
        let alpha = 6.0_f32.to_radians();
        for _ in 0..1000 {
            let d = cone_sample(axis, alpha, unit_f32(&mut rng), unit_f32(&mut rng));
            assert!(d.angle_between(*axis) <= alpha + 1e-4);
        }
        let d = cone_sample(axis, 0.0, 0.7, 0.3);
        assert!(d.angle_between(*axis) < 1e-3);
    }
}
