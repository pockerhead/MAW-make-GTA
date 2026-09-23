use crate::flow::GameState;
use bevy::prelude::*;

/// Police wanted level. T10 owns heat and how stars are earned; T5 only clears it on death.
#[derive(Resource, Reflect, Default, Debug)]
#[reflect(Resource)]
pub struct WantedLevel {
    pub stars: u8,
}

pub struct WantedPlugin;

impl Plugin for WantedPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<WantedLevel>()
            .register_type::<WantedLevel>()
            .add_systems(OnEnter(GameState::Wasted), reset_wanted);
    }
}

fn reset_wanted(mut wanted: ResMut<WantedLevel>) {
    wanted.stars = 0;
}
