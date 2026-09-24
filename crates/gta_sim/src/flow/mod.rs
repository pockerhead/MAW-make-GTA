mod busted;
mod wasted;

pub use busted::BustedClock;
pub use wasted::{RESPAWN_CONFIG, RespawnConfig, WastedClock};

use crate::character::HealthSystems;
use bevy::prelude::*;

/// Top-level game flow. The app starts in `Loading` and enters `Playing` once the world is built.
#[derive(States, Default, Clone, PartialEq, Eq, Hash, Debug, Reflect)]
pub enum GameState {
    /// Waiting for a seed; no city exists. The client starts here without `--seed`.
    MainMenu,
    #[default]
    Loading,
    Playing,
    /// `Time<Virtual>` is paused; entered only from `Playing` through `pause_request`.
    Paused,
    /// The player died: slow motion, then the "ПОТРАЧЕНО" screen, then respawn at the hospital.
    Wasted,
    /// Arrested: the arrest scene, the BUSTED screen, then respawn at the police station without
    /// weapons (GDD §3.4).
    Busted,
}

/// Phase of `GameState::Wasted`; the resource exists only while wasted.
#[derive(SubStates, Default, Clone, PartialEq, Eq, Hash, Debug, Reflect)]
#[source(GameState = GameState::Wasted)]
pub enum WastedPhase {
    #[default]
    SlowMo,
    Screen,
}

/// Phase of `GameState::Busted`; the resource exists only while busted.
#[derive(SubStates, Default, Clone, PartialEq, Eq, Hash, Debug, Reflect)]
#[source(GameState = GameState::Busted)]
pub enum BustedPhase {
    #[default]
    Arrest,
    Screen,
}

/// Fixed-tick gameplay that runs only in `GameState::Playing`.
#[derive(SystemSet, Debug, Clone, PartialEq, Eq, Hash)]
pub struct PlayingSystems;

/// NPC gameplay; keeps running while the player is wasted or busted. A new state that pauses gameplay must
/// join this condition or clear the NPC message readers' backlog on exit. `Paused` needs no entry: the
/// fixed loop does not advance while paused (the frame that enters `Paused` may still run one tick with
/// every gated set off), and `NEW_CITY` clears the buffers.
#[derive(SystemSet, Debug, Clone, PartialEq, Eq, Hash)]
pub struct NpcSystems;

/// Frame systems that run only in `GameState::Wasted`.
#[derive(SystemSet, Debug, Clone, PartialEq, Eq, Hash)]
pub struct WastedSystems;

/// Frame systems that run only in `GameState::Busted`.
#[derive(SystemSet, Debug, Clone, PartialEq, Eq, Hash)]
pub struct BustedSystems;

/// `Paused -> Loading` ("Новый город"): every domain drops its city-lifetime state here.
pub const NEW_CITY: OnTransition<GameState> = OnTransition {
    exited: GameState::Paused,
    entered: GameState::Loading,
};

/// The state an Esc press asks for; `None` while a transition is pending, because a death or arrest
/// set in this frame's fixed tick must win (`NextState::set` overwrites).
pub fn pause_request(state: &GameState, next: &NextState<GameState>) -> Option<GameState> {
    let NextState::Unchanged = next else {
        return None;
    };
    match state {
        GameState::Playing => Some(GameState::Paused),
        GameState::Paused => Some(GameState::Playing),
        _ => None,
    }
}

fn pause_time(mut time: ResMut<Time<Virtual>>) {
    time.pause();
}

fn resume_time(mut time: ResMut<Time<Virtual>>) {
    time.unpause();
}

pub struct FlowPlugin;

impl Plugin for FlowPlugin {
    fn build(&self, app: &mut App) {
        app.init_state::<GameState>()
            .add_sub_state::<WastedPhase>()
            .add_sub_state::<BustedPhase>()
            .register_type_state::<GameState>()
            .register_type_state::<WastedPhase>()
            .register_type_state::<BustedPhase>()
            .init_resource::<WastedClock>()
            .init_resource::<BustedClock>()
            .configure_sets(
                FixedUpdate,
                PlayingSystems.run_if(in_state(GameState::Playing)),
            )
            .configure_sets(
                FixedUpdate,
                NpcSystems.run_if(
                    in_state(GameState::Playing)
                        .or_else(in_state(GameState::Wasted))
                        .or_else(in_state(GameState::Busted)),
                ),
            )
            .configure_sets(Update, WastedSystems.run_if(in_state(GameState::Wasted)))
            .configure_sets(Update, BustedSystems.run_if(in_state(GameState::Busted)))
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
            )
            .add_systems(OnEnter(GameState::Paused), pause_time)
            .add_systems(OnExit(GameState::Paused), resume_time)
            .add_systems(NEW_CITY, wasted::drop_queued_damage)
            .add_systems(OnEnter(GameState::Busted), busted::enter_busted)
            .add_systems(Update, busted::advance_busted.in_set(BustedSystems))
            .add_systems(
                OnExit(GameState::Busted),
                (
                    busted::respawn_at_station,
                    wasted::drop_queued_damage,
                    wasted::drop_queued_input,
                ),
            );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pause_request_table() {
        let unchanged = NextState::<GameState>::Unchanged;
        let pending = |s: GameState| NextState::Pending(s);
        use GameState::*;
        assert_eq!(pause_request(&Playing, &unchanged), Some(Paused));
        assert_eq!(pause_request(&Playing, &pending(Wasted)), None);
        assert_eq!(pause_request(&Playing, &pending(Busted)), None);
        assert_eq!(pause_request(&Paused, &unchanged), Some(Playing));
        assert_eq!(pause_request(&Wasted, &unchanged), None);
        assert_eq!(pause_request(&Busted, &unchanged), None);
        assert_eq!(pause_request(&Loading, &unchanged), None);
        assert_eq!(pause_request(&MainMenu, &unchanged), None);
    }
}
