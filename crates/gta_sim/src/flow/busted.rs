use super::wasted::respawn_at;
use super::{BustedPhase, GameState, RespawnConfig};
use crate::character::{ActionIntent, CharacterBody, Cuffed, Health, HealthConfig};
use crate::combat::Loadout;
use crate::player::Player;
use crate::world::PoliceStationSpawn;
use avian3d::prelude::*;
use bevy::prelude::*;

/// Real seconds spent in the current `GameState::Busted`.
#[derive(Resource, Default)]
pub struct BustedClock(pub f32);

pub(super) fn enter_busted(
    mut commands: Commands,
    mut clock: ResMut<BustedClock>,
    mut players: Query<(Entity, &mut ActionIntent), With<Player>>,
) {
    clock.0 = 0.0;
    for (entity, mut action) in &mut players {
        *action = ActionIntent::default();
        commands.entity(entity).insert(Cuffed);
    }
}

// Update + Time<Real>, like advance_wasted.
pub(super) fn advance_busted(
    cfg: Res<RespawnConfig>,
    real: Res<Time<Real>>,
    phase: Res<State<BustedPhase>>,
    mut clock: ResMut<BustedClock>,
    mut next_phase: ResMut<NextState<BustedPhase>>,
    mut next_state: ResMut<NextState<GameState>>,
) {
    clock.0 += real.delta_secs();
    if *phase.get() == BustedPhase::Arrest && clock.0 >= cfg.busted_arrest {
        next_phase.set(BustedPhase::Screen);
    }
    if clock.0 >= cfg.busted_arrest + cfg.busted_screen {
        next_state.set(GameState::Playing);
    }
}

/// Respawn at the police station; guns, ammo and the bat are confiscated (GDD §3.4).
#[allow(clippy::type_complexity)]
pub(super) fn respawn_at_station(
    mut commands: Commands,
    spawn: Res<PoliceStationSpawn>,
    cfg: Res<HealthConfig>,
    mut players: Query<
        (
            (
                Entity,
                &mut Position,
                &mut Transform,
                &mut LinearVelocity,
                &mut Health,
                &CharacterBody,
            ),
            &mut Loadout,
        ),
        With<Player>,
    >,
) {
    for (body, mut loadout) in &mut players {
        let entity = body.0;
        respawn_at(spawn.point, &cfg, &mut commands, body);
        *loadout = Loadout::default();
        commands.entity(entity).try_remove::<Cuffed>();
    }
}
