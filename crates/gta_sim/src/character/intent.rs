use bevy::prelude::*;

#[derive(Reflect, Default, Clone, Copy, Debug, PartialEq, Eq)]
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
