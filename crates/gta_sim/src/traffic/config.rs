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
    pub bubble: BubbleConfig,
    pub switch: SwitchConfig,
    pub lost: LostConfig,
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
            ("bubble.in_view.spawn", b.in_view.spawn),
            ("bubble.in_view.despawn", b.in_view.despawn),
            ("bubble.off_view.spawn", b.off_view.spawn),
            ("bubble.off_view.despawn", b.off_view.despawn),
            ("bubble.offscreen_seconds", b.offscreen_seconds),
            ("bubble.spawn_spacing", b.spawn_spacing),
            ("switch.reach", s.reach),
            ("switch.horizon_seconds", s.horizon_seconds),
            ("lost.distance", self.lost.distance),
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
