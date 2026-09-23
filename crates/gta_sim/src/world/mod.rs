mod test_area;

pub use test_area::spawn_test_area;

use bevy::prelude::*;

#[derive(Component, Reflect)]
#[reflect(Component)]
pub struct Block {
    pub size: Vec3,
}

#[derive(Resource)]
pub struct PlayerSpawn(pub Vec3);

pub struct WorldPlugin;

impl Plugin for WorldPlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(PlayerSpawn(Vec3::ZERO))
            .register_type::<Block>()
            .add_systems(Startup, spawn_test_area);
    }
}
