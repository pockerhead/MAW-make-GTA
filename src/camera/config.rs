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
}

impl CameraConfig {
    pub fn validate(&self) -> Result<(), String> {
        for (field, value) in [
            ("aim_shoulder_offset", self.aim_shoulder_offset),
            ("aim_distance", self.aim_distance),
            ("aim_fov_deg", self.aim_fov_deg),
            ("aim_transition", self.aim_transition),
            ("aim_sensitivity_scale", self.aim_sensitivity_scale),
        ] {
            if !(value.is_finite() && value > 0.0) {
                return Err(format!("{field} must be a finite number > 0, got {value}"));
            }
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
