use crate::character::{CharacterControlConfig, LocomotionConfig, character_components};
use crate::world::{PlayerSpawn, spawn_test_area};
use bevy::prelude::*;

#[derive(Component, Reflect, Default)]
#[reflect(Component)]
pub struct Player;

pub struct PlayerPlugin;

impl Plugin for PlayerPlugin {
    fn build(&self, app: &mut App) {
        app.register_type::<Player>()
            .add_systems(Startup, spawn_player.after(spawn_test_area));
    }
}

fn spawn_player(
    mut commands: Commands,
    spawn: Res<PlayerSpawn>,
    config: Res<LocomotionConfig>,
    handle: Res<CharacterControlConfig>,
) {
    commands.spawn((
        Player,
        Name::new("Player"),
        Transform::from_translation(spawn.0 + Vec3::Y * config.float_height),
        character_components(&config, handle.0.clone()),
    ));
}
