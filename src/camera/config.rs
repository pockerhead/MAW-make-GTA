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
}
