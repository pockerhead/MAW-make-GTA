use crate::combat::Weapon;
use bevy::prelude::*;
use serde::Deserialize;

/// Variant order is the speed order: `Walk < Run < Sprint` (the aiming cap relies on it).
#[derive(Reflect, Default, Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Deserialize)]
#[reflect(Default)]
pub enum Gait {
    Walk,
    #[default]
    Run,
    Sprint,
}

#[derive(Component, Reflect, Default, Clone, Debug)]
#[reflect(Component, Default)]
pub struct MoveIntent {
    pub axis: Vec2,
    pub yaw: f32,
    pub gait: Gait,
    pub jump_held: bool,
    pub jump_requested: bool,
}

/// Where the character aims; written by the client (camera ray), read by the fixed-tick weapons.
#[derive(Component, Reflect, Default, Clone, Debug)]
#[reflect(Component, Default)]
pub struct AimIntent {
    pub origin: Vec3,
    pub direction: Vec3,
    /// Aim mode (RMB): the body faces the aim yaw and speed is capped at `aim_max_gait`.
    pub aiming: bool,
}

/// Weapon actions; the client raises the `*_requested` latches, the fixed tick clears them.
#[derive(Component, Reflect, Default, Clone, Debug)]
#[reflect(Component, Default)]
pub struct ActionIntent {
    pub fire_held: bool,
    pub fire_requested: bool,
    pub reload_requested: bool,
    pub select: Option<WeaponRequest>,
    /// Weapon wheel steps since the last tick (+ next, − previous).
    pub cycle: i32,
    /// F: enter/exit a car, cleared by the fixed tick.
    pub vehicle_requested: bool,
}

#[derive(Reflect, Clone, Copy, Debug, PartialEq, Eq)]
pub enum WeaponRequest {
    /// Melee slot; pressed again while unarmed, toggles fists/bat when a bat is owned.
    Unarmed,
    Gun(Weapon),
}

pub fn move_direction(axis: Vec2, yaw: f32) -> Vec3 {
    let axis = axis.clamp_length_max(1.0);
    Quat::from_rotation_y(yaw) * Vec3::new(axis.x, 0.0, -axis.y)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn camera_relative_axes() {
        let cases = [
            (0.0_f32, Vec2::Y, -Vec3::Z),
            (0.0, Vec2::X, Vec3::X),
            (90.0, Vec2::Y, -Vec3::X),
            (90.0, Vec2::X, -Vec3::Z),
            (180.0, Vec2::Y, Vec3::Z),
            (180.0, Vec2::X, -Vec3::X),
        ];
        for (yaw, axis, expected) in cases {
            assert!((move_direction(axis, yaw.to_radians()) - expected).length() < 1e-5);
        }
    }
}
