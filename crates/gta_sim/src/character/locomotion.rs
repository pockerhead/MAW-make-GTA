use super::{CharacterSchemeConfig, Gait};
use bevy::prelude::Resource;
use bevy_tnua::builtins::{
    TnuaBuiltinJumpConfig, TnuaBuiltinKnockbackConfig, TnuaBuiltinWalkConfig,
};
use serde::Deserialize;

pub const LOCOMOTION_CONFIG: &str = "character/locomotion.ron";

#[derive(Resource, Deserialize, Clone, Debug)]
#[serde(deny_unknown_fields)]
pub struct LocomotionConfig {
    pub walk_speed: f32,
    pub run_speed: f32,
    pub sprint_speed: f32,
    /// Horizontal speed below which the body shows `AnimState::Idle`, m/s.
    pub anim_idle_speed: f32,
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
    /// Centre of the head hitbox sphere above the feet, m.
    pub head_height: f32,
    pub head_radius: f32,
    /// Fastest gait while aiming (GDD §3.2).
    pub aim_max_gait: Gait,
}

impl LocomotionConfig {
    /// Checks the speeds that split `AnimState` and the head hitbox against the capsule.
    pub fn validate(&self) -> Result<(), String> {
        self.validate_speeds()?;
        self.validate_head()
    }

    fn validate_speeds(&self) -> Result<(), String> {
        let speeds = [
            self.anim_idle_speed,
            self.walk_speed,
            self.run_speed,
            self.sprint_speed,
        ];
        let ordered = speeds.iter().all(|s| s.is_finite())
            && 0.0 < self.anim_idle_speed
            && speeds.windows(2).all(|pair| pair[0] < pair[1]);
        if ordered {
            return Ok(());
        }
        Err(format!(
            "anim_idle_speed {} / walk_speed {} / run_speed {} / sprint_speed {} must be finite and satisfy \
             0 < anim_idle_speed < walk_speed < run_speed < sprint_speed",
            self.anim_idle_speed, self.walk_speed, self.run_speed, self.sprint_speed
        ))
    }

    /// The head sphere must stick out of the capsule top, otherwise a ray always hits the capsule
    /// before the head.
    fn validate_head(&self) -> Result<(), String> {
        let values = [
            self.head_height,
            self.head_radius,
            self.capsule_radius,
            self.capsule_height,
            self.float_height,
        ];
        if !values.iter().all(|v| v.is_finite()) || self.head_radius <= 0.0 {
            return Err(format!(
                "head_height {} / head_radius {} must be finite, head_radius > 0",
                self.head_height, self.head_radius
            ));
        }
        let top = self.float_height + self.capsule_height / 2.0;
        let inside_top = self.head_height < top && self.head_height + self.head_radius > top;
        let sticks_out = self.head_height - self.head_radius < top - self.capsule_radius;
        if inside_top && sticks_out {
            return Ok(());
        }
        Err(format!(
            "head_height {} / head_radius {} must satisfy head_height < {top} < head_height + \
             head_radius and head_height - head_radius < {}, otherwise a ray always hits the \
             capsule before the head",
            self.head_height,
            self.head_radius,
            top - self.capsule_radius
        ))
    }

    pub fn speed(&self, gait: Gait) -> f32 {
        match gait {
            Gait::Walk => self.walk_speed,
            Gait::Run => self.run_speed,
            Gait::Sprint => self.sprint_speed,
        }
    }

    pub fn tnua_config(&self, knockback: TnuaBuiltinKnockbackConfig) -> CharacterSchemeConfig {
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
            knockback,
        }
    }
}
