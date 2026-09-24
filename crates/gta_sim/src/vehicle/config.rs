use bevy::prelude::*;
use serde::Deserialize;

/// Path of the car handling config, relative to the assets root.
pub const VEHICLE_CONFIG: &str = "vehicle/sedan.ron";
/// Path of the crash / bullet damage config, relative to the assets root.
pub const DAMAGE_CONFIG: &str = "vehicle/damage.ron";

/// Gravity the suspension is tuned against, m/s² (avian `Gravity::default()`).
pub const GRAVITY: f32 = 9.81;

/// Wheel mounts in the body frame (x right, y up, forward −Z).
#[derive(Deserialize, Clone, Debug)]
#[serde(deny_unknown_fields)]
pub struct WheelsConfig {
    pub half_track: f32,
    pub half_wheelbase: f32,
    /// Top of the suspension travel below the chassis centre, m (negative = below).
    pub mount_height: f32,
    pub radius: f32,
}

#[derive(Deserialize, Clone, Debug)]
#[serde(deny_unknown_fields)]
pub struct SuspensionConfig {
    pub travel: f32,
    pub frequency_hz: f32,
    pub damping_ratio: f32,
    /// The damper sees the compression rate clamped to ±this, m/s.
    pub max_damper_speed: f32,
}

#[derive(Deserialize, Clone, Debug)]
#[serde(deny_unknown_fields)]
pub struct SteerConfig {
    pub max_deg: f32,
    pub at_max_speed_deg: f32,
    pub rate_deg_per_s: f32,
}

#[derive(Deserialize, Clone, Debug)]
#[serde(deny_unknown_fields)]
pub struct GripConfig {
    pub mu: f32,
    pub front: f32,
    pub rear: f32,
    pub handbrake_rear: f32,
}

/// Chassis collider underside: the box bottom raised by `lift`, the lower nose and tail cut by a
/// chamfer `chamfer_length` long and `chamfer_height` high, m. Wheels take curbs, not the body.
#[derive(Deserialize, Clone, Debug)]
#[serde(deny_unknown_fields)]
pub struct UnderbodyConfig {
    pub lift: f32,
    pub chamfer_length: f32,
    pub chamfer_height: f32,
}

/// Raycast car handling (GDD §5.1).
#[derive(Resource, Deserialize, Clone, Debug)]
#[serde(deny_unknown_fields)]
pub struct VehicleConfig {
    pub mass: f32,
    pub chassis_half_extents: (f32, f32, f32),
    pub underbody: UnderbodyConfig,
    pub center_of_mass: (f32, f32, f32),
    pub wheels: WheelsConfig,
    pub suspension: SuspensionConfig,
    pub max_speed: f32,
    pub acceleration: f32,
    pub top_speed_band: f32,
    pub reverse_speed: f32,
    pub brake_deceleration: f32,
    pub coast_deceleration: f32,
    pub hold_speed: f32,
    pub steer: SteerConfig,
    pub grip: GripConfig,
    pub roll_influence: f32,
    pub seat: (f32, f32, f32),
    pub door: (f32, f32, f32),
    pub enter_radius: f32,
    pub exit_max_speed: f32,
}

fn vec3((x, y, z): (f32, f32, f32)) -> Vec3 {
    Vec3::new(x, y, z)
}

impl VehicleConfig {
    pub fn half_extents(&self) -> Vec3 {
        vec3(self.chassis_half_extents)
    }

    pub fn center_of_mass(&self) -> Vec3 {
        vec3(self.center_of_mass)
    }

    pub fn seat(&self) -> Vec3 {
        vec3(self.seat)
    }

    pub fn door(&self) -> Vec3 {
        vec3(self.door)
    }

    fn omega(&self) -> f32 {
        std::f32::consts::TAU * self.suspension.frequency_hz
    }

    /// Static compression of one spring under a quarter of the weight, m.
    pub fn static_compression(&self) -> f32 {
        GRAVITY / (self.omega() * self.omega())
    }

    /// Spring rate of one wheel, N/m.
    pub fn spring_rate(&self) -> f32 {
        self.mass / 4.0 * self.omega() * self.omega()
    }

    /// Damper rate of one wheel, N·s/m.
    pub fn damper_rate(&self) -> f32 {
        self.suspension.damping_ratio * 2.0 * (self.spring_rate() * self.mass / 4.0).sqrt()
    }

    /// Chassis centre above flat ground at rest, m.
    pub fn rest_height(&self) -> f32 {
        -self.wheels.mount_height
            + (self.suspension.travel - self.static_compression())
            + self.wheels.radius
    }

    /// Corners of the chassis collider in the body frame (convex hull).
    pub fn chassis_points(&self) -> Vec<Vec3> {
        let h = self.half_extents();
        let u = &self.underbody;
        let bottom = -h.y + u.lift;
        let nose = bottom + u.chamfer_height;
        [-h.x, h.x]
            .into_iter()
            .flat_map(|x| {
                [
                    Vec3::new(x, h.y, -h.z),
                    Vec3::new(x, h.y, h.z),
                    Vec3::new(x, nose, -h.z),
                    Vec3::new(x, nose, h.z),
                    Vec3::new(x, bottom, -h.z + u.chamfer_length),
                    Vec3::new(x, bottom, h.z - u.chamfer_length),
                ]
            })
            .collect()
    }

    /// Volume of the chassis collider, m³: the raised box minus the two chamfer wedges.
    pub fn chassis_volume(&self) -> f32 {
        let h = self.half_extents();
        let u = &self.underbody;
        2.0 * h.x * ((2.0 * h.y - u.lift) * 2.0 * h.z - u.chamfer_length * u.chamfer_height)
    }

    /// Density that gives the chassis collider its `mass`, kg/m³.
    pub fn chassis_density(&self) -> f32 {
        self.mass / self.chassis_volume()
    }

    pub fn validate(&self) -> Result<(), String> {
        let (hx, hy, hz) = self.chassis_half_extents;
        let (cx, cy, cz) = self.center_of_mass;
        let (sx, sy, sz) = self.seat;
        let (dx, dy, dz) = self.door;
        let w = &self.wheels;
        let s = &self.suspension;
        let u = &self.underbody;
        let values = [
            self.mass,
            hx,
            hy,
            hz,
            u.lift,
            u.chamfer_length,
            u.chamfer_height,
            cx,
            cy,
            cz,
            w.half_track,
            w.half_wheelbase,
            w.mount_height,
            w.radius,
            s.travel,
            s.frequency_hz,
            s.damping_ratio,
            s.max_damper_speed,
            self.max_speed,
            self.acceleration,
            self.top_speed_band,
            self.reverse_speed,
            self.brake_deceleration,
            self.coast_deceleration,
            self.hold_speed,
            self.steer.max_deg,
            self.steer.at_max_speed_deg,
            self.steer.rate_deg_per_s,
            self.grip.mu,
            self.grip.front,
            self.grip.rear,
            self.grip.handbrake_rear,
            self.roll_influence,
            sx,
            sy,
            sz,
            dx,
            dy,
            dz,
            self.enter_radius,
            self.exit_max_speed,
        ];
        if !values.iter().all(|v| v.is_finite()) {
            return Err("vehicle values must be finite".into());
        }
        for (name, value) in [
            ("mass", self.mass),
            ("chassis_half_extents.x", hx),
            ("chassis_half_extents.y", hy),
            ("chassis_half_extents.z", hz),
            ("wheels.radius", w.radius),
            ("wheels.half_track", w.half_track),
            ("wheels.half_wheelbase", w.half_wheelbase),
            ("suspension.frequency_hz", s.frequency_hz),
            ("suspension.damping_ratio", s.damping_ratio),
            ("suspension.max_damper_speed", s.max_damper_speed),
            ("max_speed", self.max_speed),
            ("reverse_speed", self.reverse_speed),
            ("acceleration", self.acceleration),
            ("brake_deceleration", self.brake_deceleration),
            ("coast_deceleration", self.coast_deceleration),
            ("top_speed_band", self.top_speed_band),
            ("hold_speed", self.hold_speed),
            ("steer.max_deg", self.steer.max_deg),
            ("steer.rate_deg_per_s", self.steer.rate_deg_per_s),
            ("grip.mu", self.grip.mu),
            ("grip.front", self.grip.front),
            ("grip.rear", self.grip.rear),
            ("enter_radius", self.enter_radius),
            ("exit_max_speed", self.exit_max_speed),
        ] {
            if value <= 0.0 {
                return Err(format!("{name} must be positive"));
            }
        }
        if s.travel <= self.static_compression() {
            return Err(format!(
                "suspension.travel {} must exceed the static compression g/(2πf)² = {:.4}",
                s.travel,
                self.static_compression()
            ));
        }
        if !(0.0..=self.steer.max_deg).contains(&self.steer.at_max_speed_deg) {
            return Err("steer.at_max_speed_deg must be in [0, steer.max_deg]".into());
        }
        if !(0.0..=1.0).contains(&self.roll_influence) {
            return Err("roll_influence must be in [0, 1]".into());
        }
        if !(0.0..=1.0).contains(&self.grip.handbrake_rear) {
            return Err("grip.handbrake_rear must be in [0, 1]".into());
        }
        if u.lift < 0.0 || u.chamfer_length < 0.0 || u.chamfer_height < 0.0 {
            return Err("underbody lift and chamfer must be nonnegative".into());
        }
        if u.lift + u.chamfer_height >= 2.0 * hy || u.chamfer_length >= hz {
            return Err(
                "underbody lift + chamfer_height must stay below the chassis height and \
                 chamfer_length below chassis_half_extents.z"
                    .into(),
            );
        }
        if dx.abs() <= hx {
            return Err(
                "door.x must lie outside the chassis (|door.x| > chassis_half_extents.x)".into(),
            );
        }
        Ok(())
    }
}

#[derive(Deserialize, Clone, Debug)]
#[serde(deny_unknown_fields)]
pub struct VehicleDamage {
    pub max_health: f32,
    /// Closing speed below which a crash does no damage to the car, m/s.
    pub threshold_speed: f32,
    /// Car health lost per m/s of closing speed above the threshold.
    pub per_mps: f32,
    /// Multiplier on a weapon's base damage per pellet that hits the body.
    pub bullet_scale: f32,
    /// A contact whose normal (car → other) has at least this component along the car's down axis
    /// is an underbody scrape (a curb edge, a flat landing): no crash damage, no impact.
    pub scrape_normal: f32,
}

#[derive(Deserialize, Clone, Debug)]
#[serde(deny_unknown_fields)]
pub struct PedestrianDamage {
    /// Car speed along the contact normal below which a pedestrian is not hit, m/s.
    pub threshold_speed: f32,
    /// Health lost per m/s of closing speed above the threshold.
    pub per_mps: f32,
    /// Closing speed that knocks the pedestrian down, m/s.
    pub knockdown_speed: f32,
    /// Knockback velocity per m/s of closing speed.
    pub shove_scale: f32,
}

/// Crash and bullet damage of cars (GDD §5.2).
#[derive(Resource, Deserialize, Clone, Debug)]
#[serde(deny_unknown_fields)]
pub struct DamageConfig {
    pub vehicle: VehicleDamage,
    pub pedestrian: PedestrianDamage,
}

impl DamageConfig {
    pub fn validate(&self) -> Result<(), String> {
        let v = &self.vehicle;
        let p = &self.pedestrian;
        let values = [
            v.max_health,
            v.threshold_speed,
            v.per_mps,
            v.bullet_scale,
            v.scrape_normal,
            p.threshold_speed,
            p.per_mps,
            p.knockdown_speed,
            p.shove_scale,
        ];
        if !values.iter().all(|v| v.is_finite()) {
            return Err("damage values must be finite".into());
        }
        if v.max_health <= 0.0 {
            return Err("vehicle.max_health must be positive".into());
        }
        if v.threshold_speed < 0.0 {
            return Err("vehicle.threshold_speed must be nonnegative".into());
        }
        if v.per_mps <= 0.0 {
            return Err("vehicle.per_mps must be positive".into());
        }
        if v.bullet_scale <= 0.0 {
            return Err("vehicle.bullet_scale must be positive".into());
        }
        if !(v.scrape_normal > 0.0 && v.scrape_normal <= 1.0) {
            return Err("vehicle.scrape_normal must be in (0, 1]".into());
        }
        if p.threshold_speed < 0.0 {
            return Err("pedestrian.threshold_speed must be nonnegative".into());
        }
        if p.per_mps <= 0.0 {
            return Err("pedestrian.per_mps must be positive".into());
        }
        if p.knockdown_speed < p.threshold_speed {
            return Err(
                "pedestrian.knockdown_speed must be at least pedestrian.threshold_speed".into(),
            );
        }
        if p.shove_scale < 0.0 {
            return Err("pedestrian.shove_scale must be nonnegative".into());
        }
        Ok(())
    }
}
