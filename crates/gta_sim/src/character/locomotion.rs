use super::{CharacterSchemeConfig, Gait};
use bevy::prelude::Resource;
use bevy_tnua::builtins::{TnuaBuiltinJumpConfig, TnuaBuiltinWalkConfig};
use serde::Deserialize;

pub const LOCOMOTION_CONFIG: &str = "character/locomotion.ron";

#[derive(Resource, Deserialize, Clone, Debug)]
#[serde(deny_unknown_fields)]
pub struct LocomotionConfig {
    pub walk_speed: f32,
    pub run_speed: f32,
    pub sprint_speed: f32,
    pub time_to_run_speed: f32,
    pub turn_rate_deg: f32,
    pub jump_height: f32,
    pub jump_takeoff_extra_gravity: f32,
    pub coyote_time: f32,
    pub jump_buffer: f32,
    pub capsule_radius: f32,
    pub capsule_height: f32,
    pub float_height: f32,
    pub ground_sensor_cling_distance: f32,
    pub ground_spring_strength: f32,
    pub ground_spring_dampening: f32,
    pub ledge_assist_max_height: f32,
    pub ledge_assist_forward_probe: f32,
    pub ledge_assist_clearance: f32,
    pub ledge_assist_window: f32,
}

impl LocomotionConfig {
    pub fn speed(&self, gait: Gait) -> f32 {
        match gait {
            Gait::Walk => self.walk_speed,
            Gait::Run => self.run_speed,
            Gait::Sprint => self.sprint_speed,
        }
    }

    pub fn tnua_config(&self) -> CharacterSchemeConfig {
        CharacterSchemeConfig {
            basis: TnuaBuiltinWalkConfig {
                speed: 1.0,
                float_height: self.float_height,
                cling_distance: self.ground_sensor_cling_distance,
                spring_strength: self.ground_spring_strength,
                spring_dampening: self.ground_spring_dampening,
                acceleration: self.run_speed / self.time_to_run_speed,
                coyote_time: self.coyote_time,
                turning_angvel: self.turn_rate_deg.to_radians(),
                ..Default::default()
            },
            jump: TnuaBuiltinJumpConfig {
                height: self.jump_height,
                input_buffer_time: self.jump_buffer,
                takeoff_extra_gravity: self.jump_takeoff_extra_gravity,
                ..Default::default()
            },
        }
    }
}
