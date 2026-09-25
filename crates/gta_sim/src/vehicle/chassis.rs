use super::{DriveIntent, Vehicle, VehicleConfig, VehicleHealth, VehicleLoad, WHEELS};
use crate::layers::GameLayer;
use crate::world::CityBlock;
use avian3d::prelude::*;
use bevy::prelude::*;

/// Wheel forward in the body frame for a steer angle `steer` (rad, + = right): R_y(−steer)·(−Z).
pub fn wheel_forward(steer: f32) -> Vec3 {
    Vec3::new(steer.sin(), 0.0, -steer.cos())
}

/// Largest front wheel angle at forward speed `speed`, rad.
pub fn steer_limit(cfg: &VehicleConfig, speed: f32) -> f32 {
    let t = (speed.abs() / cfg.max_speed).clamp(0.0, 1.0);
    cfg.steer
        .max_deg
        .lerp(cfg.steer.at_max_speed_deg, t)
        .to_radians()
}

/// Suspension force of one wheel along the body up axis, N; `rate` is the compression rate, m/s
/// (+ = compressing). A step seen by the ray is a compression jump within one tick; the clamp keeps
/// it from becoming a damper spike.
pub fn spring_force(cfg: &VehicleConfig, compression: f32, rate: f32) -> f32 {
    let cap = cfg.suspension.max_damper_speed;
    (cfg.spring_rate() * compression + cfg.damper_rate() * rate.clamp(-cap, cap)).max(0.0)
}

/// Tyre force along the wheel right axis that cancels the sideways slip `slip` (m/s along the
/// wheel right) of a quarter of the car within one tick, N.
pub fn lateral_force(cfg: &VehicleConfig, slip: f32, grip: f32, dt: f32) -> f32 {
    -slip * grip * cfg.mass / 4.0 / dt
}

/// Tyre force along the wheel forward axis, N. `forward_speed` is the car's speed along its
/// forward axis, `wheel_speed` the contact velocity along the wheel forward.
pub fn drive_force(
    cfg: &VehicleConfig,
    intent: &DriveIntent,
    forward_speed: f32,
    wheel_speed: f32,
    rear: bool,
    dt: f32,
) -> f32 {
    let quarter = cfg.mass / 4.0;
    let hold = -wheel_speed * quarter / dt;
    // Brake and coast stop the wheel at most, never push it backwards.
    let resist = |deceleration: f32| {
        -wheel_speed.signum() * (quarter * deceleration).min(quarter * wheel_speed.abs() / dt)
    };
    let drive = cfg.mass * cfg.acceleration / 2.0;
    if rear && intent.handbrake {
        return hold;
    }
    if intent.throttle > 0.0 {
        let fade = ((cfg.max_speed - forward_speed) / cfg.top_speed_band).clamp(0.0, 1.0);
        return if rear {
            intent.throttle * drive * fade
        } else {
            0.0
        };
    }
    if intent.throttle < 0.0 {
        if forward_speed > cfg.hold_speed {
            return resist(cfg.brake_deceleration);
        }
        let fade = ((cfg.reverse_speed + forward_speed) / cfg.top_speed_band).clamp(0.0, 1.0);
        return if rear {
            intent.throttle * drive * fade
        } else {
            0.0
        };
    }
    if forward_speed.abs() < cfg.hold_speed {
        return hold;
    }
    resist(cfg.coast_deceleration)
}

#[allow(clippy::type_complexity)]
pub(super) fn drive_vehicles(
    spatial: SpatialQuery,
    cfg: Res<VehicleConfig>,
    time: Res<Time<Fixed>>,
    mut load: ResMut<VehicleLoad>,
    mut vehicles: Query<(
        Entity,
        &mut Vehicle,
        &VehicleHealth,
        &RigidBody,
        Forces,
        Has<Sleeping>,
    )>,
    intents: Query<&DriveIntent>,
    blocks: Query<(), With<CityBlock>>,
) {
    let dt = time.delta_secs();
    *load = VehicleLoad::default();
    if dt <= 0.0 {
        return;
    }
    let reach = cfg.suspension.travel + cfg.wheels.radius;
    for (entity, mut vehicle, health, body, mut forces, sleeping) in &mut vehicles {
        // A kinematic traffic car gets its wheel state from the traffic system.
        if sleeping || !body.is_dynamic() {
            continue;
        }
        load.awake += 1;
        // The seated driver's input, else the car's own AI driver.
        let mut intent = vehicle
            .driver
            .and_then(|driver| intents.get(driver).ok())
            .or_else(|| intents.get(entity).ok())
            .copied()
            .unwrap_or_default();
        if health.current <= 0.0 {
            intent.throttle = 0.0;
        }
        let position = forces.position().0;
        let rotation = forces.rotation().0;
        let up = rotation * Vec3::Y;
        let forward_speed = forces.linear_velocity().dot(rotation * Vec3::NEG_Z);
        let target = intent.steer.clamp(-1.0, 1.0) * steer_limit(&cfg, forward_speed);
        let step = cfg.steer.rate_deg_per_s.to_radians() * dt;
        vehicle.steer += (target - vehicle.steer).clamp(-step, step);
        let com = position + rotation * cfg.center_of_mass();
        let filter = SpatialQueryFilter::from_mask([GameLayer::World, GameLayer::Vehicle])
            .with_excluded_entities([entity]);
        let Ok(down) = Dir3::new(-up) else {
            continue;
        };
        let mut applied: [(Vec3, Vec3); 8] = [(Vec3::ZERO, Vec3::ZERO); 8];
        let mut on_sidewalk = false;
        for (i, &(sx, sz)) in WHEELS.iter().enumerate() {
            let rear = sz > 0.0;
            let local = Vec3::new(
                sx * cfg.wheels.half_track,
                cfg.wheels.mount_height,
                sz * cfg.wheels.half_wheelbase,
            );
            let mount = position + rotation * local;
            load.rays += 1;
            let Some(hit) = spatial.cast_ray(mount, down, reach, true, &filter) else {
                vehicle.wheels[i] = default();
                continue;
            };
            let compression = (reach - hit.distance).clamp(0.0, cfg.suspension.travel);
            let previous = vehicle.wheels[i];
            // No rate on the first grounded tick: a spawn or a landing has no previous compression.
            let rate = if previous.grounded {
                (compression - previous.compression) / dt
            } else {
                0.0
            };
            vehicle.wheels[i].compression = compression;
            vehicle.wheels[i].grounded = true;
            on_sidewalk |= blocks.contains(hit.entity);
            let normal_force = spring_force(&cfg, compression, rate);
            vehicle.wheels[i].force = normal_force;
            applied[2 * i] = (up * normal_force, mount);

            let contact = mount - up * hit.distance;
            let steer = if rear { 0.0 } else { vehicle.steer };
            let heading = rotation * wheel_forward(steer);
            let n = hit.normal;
            let Ok(tyre_forward) = Dir3::new(heading - n * heading.dot(n)) else {
                continue;
            };
            let tyre_right = tyre_forward.cross(n);
            let velocity = forces.velocity_at_point(contact);
            let grip = match (rear, intent.handbrake) {
                (false, _) => cfg.grip.front,
                (true, false) => cfg.grip.rear,
                (true, true) => cfg.grip.rear * cfg.grip.handbrake_rear,
            };
            let side = tyre_right * lateral_force(&cfg, velocity.dot(tyre_right), grip, dt);
            let along = tyre_forward.as_vec3()
                * drive_force(
                    &cfg,
                    &intent,
                    forward_speed,
                    velocity.dot(*tyre_forward),
                    rear,
                    dt,
                );
            let tyre = (side + along).clamp_length_max(cfg.grip.mu * normal_force);
            // Tyre forces act `roll_influence` of the way from the centre-of-mass height to the contact.
            let lever = (com - contact).dot(up);
            let point = contact + up * (1.0 - cfg.roll_influence) * lever;
            applied[2 * i + 1] = (tyre, point);
        }
        vehicle.on_sidewalk = on_sidewalk;
        if vehicle.driver.is_some() {
            for (force, point) in applied {
                forces.apply_force_at_point(force, point);
            }
        } else {
            let mut forces = forces.non_waking();
            for (force, point) in applied {
                forces.apply_force_at_point(force, point);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{ConfigRoot, load_config};
    use crate::vehicle::VEHICLE_CONFIG;

    const DT: f32 = 1.0 / 64.0;

    fn shipped() -> VehicleConfig {
        let root = ConfigRoot(concat!(env!("CARGO_MANIFEST_DIR"), "/../../assets").into());
        load_config(&root, VEHICLE_CONFIG).expect("GATE BROKEN: sedan.ron")
    }

    fn close(a: Vec3, b: Vec3) -> bool {
        a.abs_diff_eq(b, 1e-4)
    }

    #[test]
    fn wheel_forward_rows() {
        assert!(close(wheel_forward(0.0), Vec3::NEG_Z));
        let d = wheel_forward(30f32.to_radians());
        assert!(close(d, Vec3::new(0.5, 0.0, -0.866_025_4)));
        // Wheel right = forward × up.
        assert!(close(d.cross(Vec3::Y), Vec3::new(0.866_025_4, 0.0, 0.5)));
        assert!(close(
            wheel_forward(-30f32.to_radians()),
            Vec3::new(-0.5, 0.0, -0.866_025_4)
        ));
    }

    #[test]
    fn steer_limit_rows() {
        let cfg = shipped();
        assert!((steer_limit(&cfg, 0.0) - 32f32.to_radians()).abs() < 1e-5);
        assert!((steer_limit(&cfg, 14.0) - 19f32.to_radians()).abs() < 1e-5);
        assert!((steer_limit(&cfg, 28.0) - 6f32.to_radians()).abs() < 1e-5);
        assert!((steer_limit(&cfg, -40.0) - 6f32.to_radians()).abs() < 1e-5);
    }

    #[test]
    fn spring_force_rows() {
        let cfg = shipped();
        // k = 300·(3π)² = 26 648 N/m, c = 0.4·2·√(k·300) = 2 262 N·s/m.
        assert!((cfg.spring_rate() - 26_648.0).abs() < 1.0);
        assert!((cfg.damper_rate() - 2_262.0).abs() < 1.0);
        assert!((cfg.rest_height() - 1.1596).abs() < 1e-3);
        assert!((spring_force(&cfg, 0.1104, 0.0) - 300.0 * 9.81).abs() < 5.0);
        // Compressing adds c·rate, extending subtracts it, never pulls.
        assert!((spring_force(&cfg, 0.1, 0.5) - (2_664.8 + 1_131.0)).abs() < 2.0);
        assert!((spring_force(&cfg, 0.1, -0.5) - (2_664.8 - 1_131.0)).abs() < 2.0);
        assert_eq!(spring_force(&cfg, 0.01, -2.0), 0.0);
        // A 0.15 m step in one tick (9.6 m/s) is damped at the 0.5 m/s cap: k·0.26 + c·0.5.
        assert!((spring_force(&cfg, 0.26, 9.6) - (6_928.5 + 1_131.0)).abs() < 2.0);
    }

    #[test]
    fn lateral_force_opposes_slip() {
        let cfg = shipped();
        // Sliding +X at 1 m/s → force −X that stops a quarter car in one tick.
        assert!((lateral_force(&cfg, 1.0, 1.0, DT) + 300.0 * 64.0).abs() < 1e-2);
        assert!(lateral_force(&cfg, -1.0, 1.0, DT) > 0.0);
    }

    #[test]
    fn drive_force_rows() {
        let cfg = shipped();
        let full = DriveIntent {
            throttle: 1.0,
            ..default()
        };
        let back = DriveIntent {
            throttle: -1.0,
            ..default()
        };
        let idle = DriveIntent::default();
        // 1200·4.6/2 = 2 760 N per rear wheel below the top-speed band, faded inside it, 0 above.
        assert!((drive_force(&cfg, &full, 10.0, 10.0, true, DT) - 2760.0).abs() < 1e-2);
        assert!((drive_force(&cfg, &full, 27.0, 27.0, true, DT) - 1380.0).abs() < 1e-2);
        assert_eq!(drive_force(&cfg, &full, 28.0, 28.0, true, DT), 0.0);
        assert_eq!(drive_force(&cfg, &full, 10.0, 10.0, false, DT), 0.0);
        // Brake: 300·9 = 2 700 N against the motion on every wheel.
        assert!((drive_force(&cfg, &back, 10.0, 10.0, true, DT) + 2700.0).abs() < 1e-2);
        assert!((drive_force(&cfg, &back, 10.0, 10.0, false, DT) + 2700.0).abs() < 1e-2);
        // Reverse below hold speed, faded at reverse_speed.
        assert!((drive_force(&cfg, &back, 0.0, 0.0, true, DT) + 2760.0).abs() < 1e-2);
        assert_eq!(drive_force(&cfg, &back, -6.0, -6.0, true, DT), 0.0);
        // Coast 300 N, capped to stop the wheel; hold cancels the rolling speed.
        assert!((drive_force(&cfg, &idle, 10.0, 10.0, true, DT) + 300.0).abs() < 1e-2);
        assert!((drive_force(&cfg, &idle, 0.6, 0.01, true, DT) + 300.0 * 0.01 * 64.0).abs() < 1e-2);
        assert!((drive_force(&cfg, &idle, 0.2, 0.2, false, DT) + 0.2 * 300.0 * 64.0).abs() < 1e-2);
    }
}
