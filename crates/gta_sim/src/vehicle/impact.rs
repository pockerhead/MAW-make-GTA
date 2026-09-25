use super::{
    CabinHit, DamageConfig, PreStepVelocity, Vehicle, VehicleConfig, VehicleDamage, VehicleHealth,
    VehicleHit, VehicleImpact,
};
use crate::character::{CharacterScheme, Dead, Health};
use crate::combat::{
    AttackSerial, BulletHitVehicle, DamageDealt, HitReaction, MeleeConfig, knock_back,
};
use crate::vehicle::PedestrianDamage;
use avian3d::prelude::*;
use bevy::prelude::*;
use bevy_tnua::prelude::*;

/// Health a pedestrian loses when a car moving `car_along_n` m/s along the contact normal
/// (car → pedestrian) meets a pedestrian moving `other_along_n`; the car must be the striker.
pub fn pedestrian_damage(car_along_n: f32, other_along_n: f32, cfg: &PedestrianDamage) -> u32 {
    if car_along_n < cfg.threshold_speed {
        return 0;
    }
    let closing = car_along_n - other_along_n;
    ((closing - cfg.threshold_speed) * cfg.per_mps)
        .round()
        .max(0.0) as u32
}

/// Car health lost in a crash at `closing` m/s.
pub fn vehicle_damage(closing: f32, cfg: &VehicleDamage) -> f32 {
    ((closing - cfg.threshold_speed) * cfg.per_mps).max(0.0)
}

/// Car health lost to one pellet of `damage` base damage.
pub fn bullet_damage(damage: f32, bullet_scale: f32) -> f32 {
    damage * bullet_scale
}

pub(super) fn record_pre_step(mut bodies: Query<(&LinearVelocity, &mut PreStepVelocity)>) {
    for (velocity, mut pre) in &mut bodies {
        pre.0 = velocity.0;
    }
}

#[allow(clippy::type_complexity, clippy::too_many_arguments)]
pub(super) fn apply_impacts(
    mut starts: MessageReader<CollisionStart>,
    collisions: Collisions,
    dmg: Res<DamageConfig>,
    melee_cfg: Res<MeleeConfig>,
    mut serial: ResMut<AttackSerial>,
    mut vehicles: Query<(&Vehicle, &mut VehicleHealth, &Rotation)>,
    pre_step: Query<&PreStepVelocity>,
    positions: Query<&Position>,
    mut healths: Query<&mut Health, Without<Dead>>,
    mut reactions: Query<(&mut HitReaction, &mut TnuaController<CharacterScheme>)>,
    mut dealt: MessageWriter<DamageDealt>,
    mut hits: MessageWriter<VehicleHit>,
    mut impacts: MessageWriter<VehicleImpact>,
) {
    for start in starts.read() {
        let e1 = start.body1.unwrap_or(start.collider1);
        let e2 = start.body2.unwrap_or(start.collider2);
        let (vehicle, other, own_collider) = if vehicles.contains(e1) {
            (e1, e2, start.collider1)
        } else if vehicles.contains(e2) {
            (e2, e1, start.collider2)
        } else {
            continue;
        };
        let Some(pair) = collisions.get(start.collider1, start.collider2) else {
            continue;
        };
        let Some(manifold) = pair.manifolds.first() else {
            continue;
        };
        // The manifold normal points from `pair.collider1` to `pair.collider2`.
        let normal = if pair.collider1 == own_collider {
            manifold.normal
        } else {
            -manifold.normal
        };
        let velocity = |e: Entity| pre_step.get(e).map_or(Vec3::ZERO, |v| v.0);
        let car_along = velocity(vehicle).dot(normal);
        let other_along = velocity(other).dot(normal);
        let closing = car_along - other_along;
        let driver = vehicles.get(vehicle).ok().and_then(|(v, ..)| v.driver);
        let point = manifold.points.first().map_or(Vec3::ZERO, |p| p.point);

        // A character is never a crash for the car; only a live one is hurt.
        if reactions.contains(other) {
            let Ok(mut health) = healths.get_mut(other) else {
                continue;
            };
            let damage = pedestrian_damage(car_along, other_along, &dmg.pedestrian);
            if damage == 0 {
                continue;
            }
            let attack = serial.next_id();
            let killed = health.take(damage as f32);
            dealt.write(DamageDealt {
                shooter: driver.unwrap_or(vehicle),
                shot: attack,
                target: other,
                point: positions.get(other).map_or(Vec3::ZERO, |p| p.0),
                damage,
                headshot: false,
                killed,
            });
            hits.write(VehicleHit {
                vehicle,
                driver,
                target: other,
                attack,
                speed: closing,
            });
            impacts.write(VehicleImpact {
                vehicle,
                point,
                speed: closing,
            });
            if closing < dmg.pedestrian.knockdown_speed {
                continue;
            }
            if let Ok((mut reaction, mut controller)) = reactions.get_mut(other) {
                reaction.escalate(true, &melee_cfg);
                let flat = Vec3::new(normal.x, 0.0, normal.z).normalize_or_zero();
                knock_back(&mut controller, flat * closing * dmg.pedestrian.shove_scale);
            }
            continue;
        }
        let loss = vehicle_damage(closing, &dmg.vehicle);
        // A car-car crash is an impact of both cars (the client shakes and plays the driven one).
        for (car, outward) in [(vehicle, normal), (other, -normal)] {
            let Ok((_, mut health, rotation)) = vehicles.get_mut(car) else {
                continue;
            };
            // The underbody meets curbs and landings; the wheels own those, not a crash.
            if outward.dot(rotation.0 * Vec3::NEG_Y) >= dmg.vehicle.scrape_normal {
                continue;
            }
            health.current = (health.current - loss).max(0.0);
            impacts.write(VehicleImpact {
                vehicle: car,
                point,
                speed: closing,
            });
        }
    }
}

/// `point` lies inside the cabin zone of a car at `position` / `rotation`.
pub fn in_cabin(cfg: &VehicleConfig, position: Vec3, rotation: Quat, point: Vec3) -> bool {
    let local = rotation.inverse() * (point - position) - cfg.cabin_centre();
    let half = cfg.cabin_half_extents();
    local.x.abs() <= half.x && local.y.abs() <= half.y && local.z.abs() <= half.z
}

/// Driver wound of a cabin pellet of `damage` base damage.
pub fn cabin_wound(damage: f32, share: f32) -> f32 {
    (damage * share).round()
}

/// Car health loss per pellet; a pellet in the cabin also wounds the seated driver.
#[allow(clippy::too_many_arguments)]
pub(super) fn apply_bullet_hits(
    mut bullets: MessageReader<BulletHitVehicle>,
    dmg: Res<DamageConfig>,
    cfg: Res<VehicleConfig>,
    mut vehicles: Query<(&mut VehicleHealth, &Vehicle, &Position, &Rotation)>,
    mut drivers: Query<&mut Health, Without<Dead>>,
    mut cabin: MessageWriter<CabinHit>,
    mut dealt: MessageWriter<DamageDealt>,
) {
    for hit in bullets.read() {
        let Ok((mut health, vehicle, position, rotation)) = vehicles.get_mut(hit.vehicle) else {
            continue;
        };
        let loss = bullet_damage(hit.damage, dmg.vehicle.bullet_scale);
        health.current = (health.current - loss).max(0.0);
        if !in_cabin(&cfg, position.0, rotation.0, hit.point) {
            continue;
        }
        cabin.write(CabinHit {
            shooter: hit.shooter,
            attack: hit.attack,
            vehicle: hit.vehicle,
            point: hit.point,
            damage: hit.damage,
        });
        let Some(driver) = vehicle.driver else {
            continue;
        };
        let Ok(mut life) = drivers.get_mut(driver) else {
            continue;
        };
        let wound = cabin_wound(hit.damage, dmg.vehicle.cabin_driver_share);
        let killed = life.take(wound);
        dealt.write(DamageDealt {
            shooter: hit.shooter,
            shot: hit.attack,
            target: driver,
            point: hit.point,
            damage: wound as u32,
            headshot: false,
            killed,
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pedestrian() -> PedestrianDamage {
        PedestrianDamage {
            threshold_speed: 3.0,
            per_mps: 12.0,
            knockdown_speed: 4.0,
            shove_scale: 0.6,
        }
    }

    #[test]
    fn pedestrian_damage_rows() {
        let cfg = pedestrian();
        assert_eq!(pedestrian_damage(10.0, 0.0, &cfg), 84);
        assert_eq!(pedestrian_damage(6.0, 0.0, &cfg), 36);
        assert_eq!(pedestrian_damage(2.5, 0.0, &cfg), 0);
        // A runner into a parked car: the car is not the striker.
        assert_eq!(pedestrian_damage(0.0, -6.8, &cfg), 0);
        assert_eq!(pedestrian_damage(10.0, -2.0, &cfg), 108);
    }

    #[test]
    fn vehicle_damage_rows() {
        let cfg = VehicleDamage {
            max_health: 1000.0,
            threshold_speed: 5.0,
            per_mps: 40.0,
            bullet_scale: 1.0,
            scrape_normal: 0.5,
            cabin_driver_share: 0.5,
        };
        assert_eq!(vehicle_damage(4.0, &cfg), 0.0);
        assert_eq!(vehicle_damage(10.0, &cfg), 200.0);
        assert_eq!(vehicle_damage(28.0, &cfg), 920.0);
    }

    #[test]
    fn cabin_rows() {
        let root =
            crate::config::ConfigRoot(concat!(env!("CARGO_MANIFEST_DIR"), "/../../assets").into());
        let cfg: VehicleConfig = crate::config::load_config(&root, crate::vehicle::VEHICLE_CONFIG)
            .expect("GATE BROKEN: sedan.ron");
        let (p, r) = (Vec3::new(3.0, 1.16, -2.0), Quat::from_rotation_y(0.7));
        let at = |local: Vec3| p + r * local;
        // Side window, roof over the seat: inside. Hood, low door: outside.
        assert!(in_cabin(&cfg, p, r, at(Vec3::new(1.2, 0.5, 0.0))));
        assert!(in_cabin(&cfg, p, r, at(Vec3::new(0.0, 0.92, 0.3))));
        assert!(!in_cabin(&cfg, p, r, at(Vec3::new(0.0, 0.3, -2.04))));
        assert!(!in_cabin(&cfg, p, r, at(Vec3::new(1.2, -0.3, 0.0))));
        // Pistol 25 x 0.5 = 12.5 rounds half away from zero.
        assert_eq!(cabin_wound(25.0, 0.5), 13.0);
    }

    #[test]
    fn bullet_damage_rows() {
        assert_eq!(bullet_damage(25.0, 1.0), 25.0);
        assert_eq!(bullet_damage(12.0, 0.5), 6.0);
    }
}
