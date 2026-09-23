use bevy::prelude::*;

/// Top-level game flow. The app starts in `Loading` and enters `Playing` once the world is built.
#[derive(States, Default, Clone, PartialEq, Eq, Hash, Debug)]
pub enum GameState {
    #[default]
    Loading,
    Playing,
}

pub struct FlowPlugin;

impl Plugin for FlowPlugin {
    fn build(&self, app: &mut App) {
        app.init_state::<GameState>();
    }
}
