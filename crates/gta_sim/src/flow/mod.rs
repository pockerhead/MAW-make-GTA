mod wasted;

pub use wasted::{RESPAWN_CONFIG, RespawnConfig, WastedClock};

use crate::character::HealthSystems;
use bevy::prelude::*;

/// Top-level game flow. The app starts in `Loading` and enters `Playing` once the world is built.
#[derive(States, Default, Clone, PartialEq, Eq, Hash, Debug, Reflect)]
pub enum GameState {
    #[default]
    Loading,
    Playing,
    /// The player died: slow motion, then the "ПОТРАЧЕНО" screen, then respawn at the hospital.
    Wasted,
}

/// Phase of `GameState::Wasted`; the resource exists only while wasted.
#[derive(SubStates, Default, Clone, PartialEq, Eq, Hash, Debug, Reflect)]
#[source(GameState = GameState::Wasted)]
pub enum WastedPhase {
    #[default]
    SlowMo,
    Screen,
}

/// Fixed-tick gameplay that runs only in `GameState::Playing`.
#[derive(SystemSet, Debug, Clone, PartialEq, Eq, Hash)]
pub struct PlayingSystems;

/// NPC gameplay; keeps running while the player is wasted. A new state that pauses gameplay must
/// join this condition or clear the NPC message readers' backlog on exit.
#[derive(SystemSet, Debug, Clone, PartialEq, Eq, Hash)]
pub struct NpcSystems;

/// Frame systems that run only in `GameState::Wasted`.
#[derive(SystemSet, Debug, Clone, PartialEq, Eq, Hash)]
pub struct WastedSystems;

pub struct FlowPlugin;

impl Plugin for FlowPlugin {
    fn build(&self, app: &mut App) {
        app.init_state::<GameState>()
            .add_sub_state::<WastedPhase>()
            .register_type_state::<GameState>()
            .register_type_state::<WastedPhase>()
            .init_resource::<WastedClock>()
            .configure_sets(
                FixedUpdate,
                PlayingSystems.run_if(in_state(GameState::Playing)),
            )
            .configure_sets(
                FixedUpdate,
                NpcSystems
                    .run_if(in_state(GameState::Playing).or_else(in_state(GameState::Wasted))),
            )
            .configure_sets(Update, WastedSystems.run_if(in_state(GameState::Wasted)))
            .add_systems(
                FixedUpdate,
                wasted::detect_player_death
                    .in_set(PlayingSystems)
                    .in_set(HealthSystems::Death),
            )
            .add_systems(OnEnter(GameState::Wasted), wasted::enter_wasted)
            .add_systems(Update, wasted::advance_wasted.in_set(WastedSystems))
            .add_systems(OnExit(WastedPhase::SlowMo), wasted::restore_time_scale)
            .add_systems(
                OnExit(GameState::Wasted),
                (
                    wasted::respawn_player,
                    wasted::drop_queued_damage,
                    wasted::drop_queued_input,
                ),
            );
    }
}
