use bevy::prelude::*;
use serde::Deserialize;

/// Path of the traffic config, relative to the assets root.
pub const TRAFFIC_CONFIG: &str = "traffic/traffic.ron";

/// Intelligent Driver Model parameters (GDD §5.2).
#[derive(Deserialize, Clone, Debug)]
#[serde(deny_unknown_fields)]
pub struct IdmConfig {
    /// T, s.
    pub time_headway: f32,
    /// a, m/s².
    pub acceleration: f32,
    /// b, m/s².
    pub comfortable_deceleration: f32,
    /// s0, m.
    pub min_gap: f32,
    /// The IDM result is clamped at −this, m/s².
    pub max_deceleration: f32,
}

/// v0 by road class, m/s.
#[derive(Deserialize, Clone, Debug)]
#[serde(deny_unknown_fields)]
pub struct DesiredSpeed {
    pub avenue: f32,
    pub street: f32,
}

/// Spawn and despawn distance of one band, m.
#[derive(Deserialize, Clone, Copy, Debug)]
#[serde(deny_unknown_fields)]
pub struct Band {
    pub spawn: f32,
    pub despawn: f32,
}

/// The traffic bubble (Vermeij rules).
#[derive(Deserialize, Clone, Debug)]
#[serde(deny_unknown_fields)]
pub struct BubbleConfig {
    pub max_cars: u32,
    pub in_view: Band,
    pub off_view: Band,
    pub offscreen_seconds: f32,
    pub spawns_per_tick: u32,
    pub initial_spawns_per_tick: u32,
    pub spawn_spacing: f32,
    /// A car standing this long out of frame (a traffic car anywhere, a driverless car in a junction
    /// box) despawns, s.
    pub stuck_despawn_seconds: f32,
    /// A driverless car in a junction box farther than this from the player counts as out of frame
    /// for the stuck cheat (and so does one with only a corner in frame), m.
    pub stuck_in_view_distance: f32,
}

/// Time-to-contact switch of a kinematic car to a dynamic body.
#[derive(Deserialize, Clone, Debug)]
#[serde(deny_unknown_fields)]
pub struct SwitchConfig {
    /// Broadphase growth of the chassis box, m.
    pub reach: f32,
    pub horizon_seconds: f32,
    /// Growth of the car's footprint in the sweep test, m.
    pub skin: f32,
}

/// A dynamic car off its path is given up.
#[derive(Deserialize, Clone, Debug)]
#[serde(deny_unknown_fields)]
pub struct LostConfig {
    pub distance: f32,
    pub angle_deg: f32,
}

/// Sideways motion of a kinematic car off its path (rejoin, pass, yield): rate `rate_at_rest + slope
/// x speed`, m/s; heading turned towards the move at most `yaw_rate_deg` per second.
#[derive(Deserialize, Clone, Debug)]
#[serde(deny_unknown_fields)]
pub struct LateralConfig {
    pub rate_at_rest: f32,
    pub slope: f32,
    pub yaw_rate_deg: f32,
}

impl LateralConfig {
    /// Lateral rate at speed `v`, m/s.
    pub fn rate(&self, v: f32) -> f32 {
        self.rate_at_rest + self.slope * v.max(0.0)
    }
}

/// A bumped (`Dynamic`) traffic car goes back to kinematic driving: upright, not lost, at rest, no
/// dynamic body whose relative sweep over `horizon_seconds` reaches its footprint grown by `skin`, and
/// a clear rejoin corridor, all for `seconds`; one that stood `give_up_seconds` without that is
/// given up.
#[derive(Deserialize, Clone, Debug)]
#[serde(deny_unknown_fields)]
pub struct RecoverConfig {
    pub seconds: f32,
    pub skin: f32,
    pub horizon_seconds: f32,
    pub max_tilt_deg: f32,
    pub give_up_seconds: f32,
}

/// Going around a standing body: a car whose nearest obstacle ahead within `trigger_gap` has stood
/// `vehicle_seconds` (a vehicle) or `character_seconds` (a character) passes it with `clearance` on
/// each side, at most at `speed`.
#[derive(Deserialize, Clone, Debug)]
#[serde(deny_unknown_fields)]
pub struct PassConfig {
    pub vehicle_seconds: f32,
    pub character_seconds: f32,
    pub trigger_gap: f32,
    pub clearance: f32,
    pub speed: f32,
}

/// Yielding to a siren car: a car it comes up behind (or that it drives at) within `yield_distance`
/// pulls to its curb side and stops; it drives on once the car has passed or after `timeout_seconds`
/// (then it ignores sirens that long).
#[derive(Deserialize, Clone, Debug)]
#[serde(deny_unknown_fields)]
pub struct SirensConfig {
    pub yield_distance: f32,
    pub timeout_seconds: f32,
}

/// The progress rule (TASK-039, `progress`): a car stuck behind a body that has stood
/// `wait_seconds`, itself standing `grace_seconds`, squeezes past it with collision relaxed against
/// that body only, at most at `pass.speed`; the planning part ends after `max_seconds` at the latest.
#[derive(Deserialize, Clone, Debug)]
#[serde(deny_unknown_fields)]
pub struct ProgressConfig {
    pub wait_seconds: f32,
    pub grace_seconds: f32,
    pub max_seconds: f32,
}

/// Traffic tuning (GDD §5.2).
#[derive(Resource, Deserialize, Clone, Debug)]
#[serde(deny_unknown_fields)]
pub struct TrafficConfig {
    pub idm: IdmConfig,
    pub desired_speed: DesiredSpeed,
    pub turn_speed: f32,
    pub look_ahead: f32,
    pub sense_distance: f32,
    pub turn_sense_distance: f32,
    pub connector_samples: u32,
    pub conflict_margin: f32,
    /// A junction grant lapses after its holder stood this long before the stop line while a car
    /// waits for a conflicting connector, s.
    pub reservation_timeout: f32,
    /// A waiting car's nose stays this far before the centre line of a pedestrian crossing, m.
    pub crossing_clearance: f32,
    pub bubble: BubbleConfig,
    pub switch: SwitchConfig,
    pub lost: LostConfig,
    pub lateral: LateralConfig,
    pub recover: RecoverConfig,
    pub pass: PassConfig,
    pub sirens: SirensConfig,
    pub progress: ProgressConfig,
}

fn positive(field: &str, value: f32) -> Result<(), String> {
    if value.is_finite() && value > 0.0 {
        Ok(())
    } else {
        Err(format!("{field} {value} must be finite and > 0"))
    }
}

impl TrafficConfig {
    /// `tick`: the fixed timestep, s.
    pub fn validate(&self, tick: f32) -> Result<(), String> {
        let i = &self.idm;
        let b = &self.bubble;
        let s = &self.switch;
        for (field, value) in [
            ("idm.time_headway", i.time_headway),
            ("idm.acceleration", i.acceleration),
            ("idm.comfortable_deceleration", i.comfortable_deceleration),
            ("idm.min_gap", i.min_gap),
            ("idm.max_deceleration", i.max_deceleration),
            ("desired_speed.avenue", self.desired_speed.avenue),
            ("desired_speed.street", self.desired_speed.street),
            ("turn_speed", self.turn_speed),
            ("look_ahead", self.look_ahead),
            ("sense_distance", self.sense_distance),
            ("turn_sense_distance", self.turn_sense_distance),
            ("conflict_margin", self.conflict_margin),
            ("reservation_timeout", self.reservation_timeout),
            ("crossing_clearance", self.crossing_clearance),
            ("bubble.in_view.spawn", b.in_view.spawn),
            ("bubble.in_view.despawn", b.in_view.despawn),
            ("bubble.off_view.spawn", b.off_view.spawn),
            ("bubble.off_view.despawn", b.off_view.despawn),
            ("bubble.offscreen_seconds", b.offscreen_seconds),
            ("bubble.spawn_spacing", b.spawn_spacing),
            ("bubble.stuck_despawn_seconds", b.stuck_despawn_seconds),
            ("bubble.stuck_in_view_distance", b.stuck_in_view_distance),
            ("switch.reach", s.reach),
            ("switch.horizon_seconds", s.horizon_seconds),
            ("lost.distance", self.lost.distance),
            ("lateral.rate_at_rest", self.lateral.rate_at_rest),
            ("lateral.slope", self.lateral.slope),
            ("lateral.yaw_rate_deg", self.lateral.yaw_rate_deg),
            ("recover.seconds", self.recover.seconds),
            ("recover.skin", self.recover.skin),
            ("recover.horizon_seconds", self.recover.horizon_seconds),
            ("recover.give_up_seconds", self.recover.give_up_seconds),
            ("pass.vehicle_seconds", self.pass.vehicle_seconds),
            ("pass.character_seconds", self.pass.character_seconds),
            ("pass.trigger_gap", self.pass.trigger_gap),
            ("pass.clearance", self.pass.clearance),
            ("pass.speed", self.pass.speed),
            ("sirens.timeout_seconds", self.sirens.timeout_seconds),
            ("progress.wait_seconds", self.progress.wait_seconds),
            ("progress.grace_seconds", self.progress.grace_seconds),
            ("progress.max_seconds", self.progress.max_seconds),
        ] {
            positive(field, value)?;
        }
        if i.max_deceleration < i.comfortable_deceleration {
            return Err(format!(
                "idm.max_deceleration {} must be >= idm.comfortable_deceleration {}",
                i.max_deceleration, i.comfortable_deceleration
            ));
        }
        if !(b.off_view.spawn < b.off_view.despawn
            && b.off_view.despawn <= b.in_view.spawn
            && b.in_view.spawn < b.in_view.despawn)
        {
            return Err(format!(
                "bubble bands must satisfy off_view.spawn < off_view.despawn <= in_view.spawn < \
                 in_view.despawn, got off_view {:?}, in_view {:?}",
                b.off_view, b.in_view
            ));
        }
        if b.max_cars < 1 {
            return Err("bubble.max_cars must be >= 1".into());
        }
        if b.spawns_per_tick < 1 {
            return Err("bubble.spawns_per_tick must be >= 1".into());
        }
        if b.initial_spawns_per_tick < 1 {
            return Err("bubble.initial_spawns_per_tick must be >= 1".into());
        }
        if self.connector_samples < 3 {
            return Err(format!(
                "connector_samples {} must be >= 3",
                self.connector_samples
            ));
        }
        let angle = self.lost.angle_deg;
        if !(angle.is_finite() && 0.0 < angle && angle < 180.0) {
            return Err(format!("lost.angle_deg {angle} must be in (0, 180)"));
        }
        if !(s.skin.is_finite() && s.skin >= 0.0) {
            return Err(format!("switch.skin {} must be finite and >= 0", s.skin));
        }
        let r = &self.recover;
        if !(r.max_tilt_deg.is_finite() && 0.0 < r.max_tilt_deg && r.max_tilt_deg < 90.0) {
            return Err(format!(
                "recover.max_tilt_deg {} must be in (0, 90)",
                r.max_tilt_deg
            ));
        }
        // Hysteresis: a body that would switch the car again within a tick never lets it recover.
        if r.skin <= s.skin {
            return Err(format!(
                "recover.skin {} must be > switch.skin {}",
                r.skin, s.skin
            ));
        }
        if r.horizon_seconds < s.horizon_seconds {
            return Err(format!(
                "recover.horizon_seconds {} must be >= switch.horizon_seconds {}",
                r.horizon_seconds, s.horizon_seconds
            ));
        }
        // Bodies stand in the road snapshot only within in_view.despawn + look_ahead of the player.
        let snapshot = b.in_view.despawn + self.look_ahead;
        if b.stuck_in_view_distance >= snapshot {
            return Err(format!(
                "bubble.stuck_in_view_distance {} must be < in_view.despawn + look_ahead {snapshot}",
                b.stuck_in_view_distance
            ));
        }
        let y = self.sirens.yield_distance;
        // 0 = nobody yields.
        if !(y.is_finite() && y >= 0.0) {
            return Err(format!("sirens.yield_distance {y} must be finite and >= 0"));
        }
        let p = &self.pass;
        // A threshold at IDM's rest gap never fires (the queue head stands exactly there).
        if p.trigger_gap <= i.min_gap {
            return Err(format!(
                "pass.trigger_gap {} must be > idm.min_gap {}",
                p.trigger_gap, i.min_gap
            ));
        }
        if p.character_seconds < p.vehicle_seconds {
            return Err(format!(
                "pass.character_seconds {} must be >= pass.vehicle_seconds {}",
                p.character_seconds, p.vehicle_seconds
            ));
        }
        let g = &self.progress;
        // A new waiter behind an old blocker still gets its grace before it squeezes.
        if g.wait_seconds <= g.grace_seconds {
            return Err(format!(
                "progress.wait_seconds {} must be > progress.grace_seconds {}",
                g.wait_seconds, g.grace_seconds
            ));
        }
        // A new waiter gets the clean pass around a person first.
        if g.grace_seconds < p.character_seconds {
            return Err(format!(
                "progress.grace_seconds {} must be >= pass.character_seconds {}",
                g.grace_seconds, p.character_seconds
            ));
        }
        // Law: the sweep covers at least two fixed steps.
        if s.horizon_seconds < 2.0 * tick {
            return Err(format!(
                "switch.horizon_seconds {} must be >= two fixed ticks ({})",
                s.horizon_seconds,
                2.0 * tick
            ));
        }
        Ok(())
    }

    /// Cross-config rules: the broadphase holds every body the sweep can reach, and traffic
    /// despawns inside the population bubble.
    pub fn validate_cross(&self, car_max_speed: f32, despawn_distance: f32) -> Result<(), String> {
        let v0 = self.desired_speed.avenue.max(self.desired_speed.street);
        let swept = (car_max_speed + v0) * self.switch.horizon_seconds;
        if self.switch.reach < swept {
            return Err(format!(
                "switch.reach {} must be >= (vehicle max_speed {car_max_speed} + desired speed {v0}) \
                 x horizon_seconds {} = {swept}",
                self.switch.reach, self.switch.horizon_seconds
            ));
        }
        if self.bubble.in_view.despawn >= despawn_distance {
            return Err(format!(
                "bubble.in_view.despawn {} must be < the population despawn_distance {despawn_distance}",
                self.bubble.in_view.despawn
            ));
        }
        Ok(())
    }
}
