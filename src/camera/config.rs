use bevy::prelude::Resource;
use serde::Deserialize;

pub const CAMERA_CONFIG: &str = "camera/camera.ron";

#[derive(Resource, Deserialize, Clone, Debug)]
#[serde(deny_unknown_fields)]
pub struct CameraConfig {
    pub pivot_height: f32,
    pub shoulder_offset: f32,
    pub distance: f32,
    pub fov_deg: f32,
    pub pitch_min_deg: f32,
    pub pitch_max_deg: f32,
    pub mouse_sensitivity_deg: f32,
    pub follow_half_life: f32,
    pub collision_radius: f32,
    pub collision_release_half_life: f32,
    /// Aim mode (RMB, GDD §3.2): shoulder offset, distance and FOV while aiming.
    pub aim_shoulder_offset: f32,
    pub aim_distance: f32,
    pub aim_fov_deg: f32,
    /// Seconds to blend into and out of aim mode.
    pub aim_transition: f32,
    /// Mouse sensitivity multiplier while aiming.
    pub aim_sensitivity_scale: f32,
    /// Car camera (GDD §5.1): distance behind the pivot, pivot height above the car centre, m.
    pub car_distance: f32,
    pub car_pivot_height: f32,
    /// Pitch the car camera returns to, degrees.
    pub car_pitch_deg: f32,
    /// Half-life of the return behind the car, s.
    pub car_yaw_half_life: f32,
    /// Seconds without mouse look before the camera returns behind the car.
    pub car_look_return: f32,
    /// Camera closer than this to the player body (m) hides the player model; it shows again only
    /// past `hide_player_distance + hide_player_band`, so the threshold never flickers.
    pub hide_player_distance: f32,
    pub hide_player_band: f32,
}

impl CameraConfig {
    pub fn validate(&self) -> Result<(), String> {
        for (field, value) in [
            ("aim_shoulder_offset", self.aim_shoulder_offset),
            ("aim_distance", self.aim_distance),
            ("aim_fov_deg", self.aim_fov_deg),
            ("aim_transition", self.aim_transition),
            ("aim_sensitivity_scale", self.aim_sensitivity_scale),
            ("car_distance", self.car_distance),
            ("car_pivot_height", self.car_pivot_height),
            ("car_yaw_half_life", self.car_yaw_half_life),
            ("car_look_return", self.car_look_return),
            ("hide_player_distance", self.hide_player_distance),
            ("hide_player_band", self.hide_player_band),
        ] {
            if !(value.is_finite() && value > 0.0) {
                return Err(format!("{field} must be a finite number > 0, got {value}"));
            }
        }
        if !(self.pitch_min_deg..=self.pitch_max_deg).contains(&self.car_pitch_deg) {
            return Err(format!(
                "car_pitch_deg must be in [pitch_min_deg, pitch_max_deg], got {}",
                self.car_pitch_deg
            ));
        }
        if self.aim_sensitivity_scale > 1.0 {
            return Err(format!(
                "aim_sensitivity_scale must be in (0, 1], got {}",
                self.aim_sensitivity_scale
            ));
        }
        Ok(())
    }
}
