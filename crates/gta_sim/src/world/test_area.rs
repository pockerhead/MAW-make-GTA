use super::Block;
use avian3d::prelude::*;
use bevy::prelude::*;

// Fixture level for the headless character gates.
const TEST_AREA: &[(Vec3, Vec3, f32)] = &[
    (Vec3::new(80.0, 1.0, 80.0), Vec3::new(0.0, -0.5, 0.0), 0.0),
    (Vec3::new(1.0, 1.0, 1.0), Vec3::new(10.0, 0.5, 10.0), 0.0),
    (Vec3::new(2.0, 1.0, 2.0), Vec3::new(13.0, 0.5, 10.0), 0.0),
    (Vec3::new(1.5, 2.0, 1.5), Vec3::new(16.0, 1.0, 10.0), 0.0),
    (
        Vec3::new(4.0, 0.4, 10.0),
        Vec3::new(-10.0, 2.327, -16.430),
        30.0,
    ),
    (Vec3::new(4.0, 5.0, 4.0), Vec3::new(-10.0, 2.5, -22.66), 0.0),
    (Vec3::new(3.0, 1.6, 6.0), Vec3::new(10.0, 0.8, -17.4), 0.0),
    (Vec3::new(12.0, 4.0, 0.5), Vec3::new(0.0, 2.0, 14.0), 0.0),
];

pub fn spawn_test_area(mut commands: Commands) {
    for &(size, center, angle) in TEST_AREA {
        spawn_block(&mut commands, size, center, angle);
    }
    for step in 0..8 {
        let i = step as f32;
        let height = 0.2 * (i + 1.0);
        spawn_block(
            &mut commands,
            Vec3::new(3.0, height, 0.3),
            Vec3::new(10.0, height / 2.0, -12.15 - 0.3 * i),
            0.0,
        );
    }
}

fn spawn_block(commands: &mut Commands, size: Vec3, center: Vec3, angle: f32) {
    commands.spawn((
        Block { size },
        RigidBody::Static,
        Collider::cuboid(size.x, size.y, size.z),
        Transform::from_translation(center)
            .with_rotation(Quat::from_rotation_x(angle.to_radians())),
    ));
}
