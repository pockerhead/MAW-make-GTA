mod pickups;

pub use pickups::{Pickup, PickupKind};

use crate::character::HealthSystems;
use crate::flow::{GameState, PlayingSystems};
use bevy::prelude::*;

pub struct CombatPlugin;

impl Plugin for CombatPlugin {
    fn build(&self, app: &mut App) {
        app.register_type::<Pickup>()
            .register_type::<PickupKind>()
            .add_systems(
                OnTransition {
                    exited: GameState::Loading,
                    entered: GameState::Playing,
                },
                pickups::spawn_pickups,
            )
            .add_systems(
                FixedUpdate,
                pickups::collect_pickups
                    .in_set(PlayingSystems)
                    .in_set(HealthSystems::Pickup),
            );
    }
}
