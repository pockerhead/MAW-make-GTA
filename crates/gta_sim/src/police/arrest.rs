//! The arrest (GDD §6.4): a cop in `Arrest` close to a passive player for `arrest.seconds` makes the
//! player `Busted`; running off breaks free for one more star.

use super::fsm::{ArrestStep, arrest_step, break_free_heat};
use super::{CopState, EscalationConfig, PoliceAlert, PoliceUnit};
use crate::character::Dead;
use crate::combat::{HitReaction, Melee, ShotFired};
use crate::flow::GameState;
use crate::navigation::flat_distance;
use crate::player::Player;
use crate::vehicle::Driving;
use crate::wanted::{WantedConfig, WantedLevel};
use avian3d::prelude::*;
use bevy::prelude::*;

/// The running arrest: the arresting cop and the seconds of passivity held so far; read by QA.
#[derive(Resource, Reflect, Default, Clone, Copy, Debug, PartialEq)]
#[reflect(Resource)]
pub struct ArrestAttempt {
    pub cop: Option<Entity>,
    pub hold: f32,
}

#[allow(clippy::too_many_arguments, clippy::type_complexity)]
pub(super) fn arrest_player(
    configs: (Res<EscalationConfig>, Res<WantedConfig>),
    time: Res<Time<Fixed>>,
    mut attempt: ResMut<ArrestAttempt>,
    mut wanted: ResMut<WantedLevel>,
    mut next: ResMut<NextState<GameState>>,
    mut shots: MessageReader<ShotFired>,
    // `Without<Dead>`: a death this tick is already applied here and beats the arrest.
    player: Query<
        (Entity, &Position, &HitReaction, &Melee),
        (With<Player>, Without<Dead>, Without<Driving>),
    >,
    cops: Query<(Entity, &Position, &PoliceUnit)>,
) {
    let (esc, wanted_cfg) = configs;
    let shooters: Vec<Entity> = shots.read().map(|s| s.shooter).collect();
    let Ok((me, at, reaction, melee)) = player.single() else {
        return;
    };
    let arresting = |cop: Entity| {
        cops.get(cop)
            .ok()
            .filter(|(.., unit)| unit.state == CopState::Arrest)
            .map(|(_, p, _)| flat_distance(p.0, at.0))
    };
    let current = attempt
        .cop
        .and_then(|cop| arresting(cop).map(|distance| (cop, distance)));
    let (cop, distance) = match current {
        Some(found) => found,
        None => {
            *attempt = ArrestAttempt::default();
            let nearest = cops
                .iter()
                .filter(|(.., unit)| unit.state == CopState::Arrest)
                .map(|(cop, p, _)| (cop, flat_distance(p.0, at.0)))
                .filter(|&(_, distance)| distance <= esc.arrest.distance)
                .min_by(|a, b| a.1.total_cmp(&b.1));
            let Some(found) = nearest else {
                return;
            };
            found
        }
    };
    let attacking = shooters.contains(&me) || melee.swing.is_some();
    let knocked_down = matches!(reaction, HitReaction::KnockedDown { .. });
    match arrest_step(
        attempt.hold,
        distance,
        attacking,
        knocked_down,
        time.delta_secs(),
        &esc.arrest,
    ) {
        ArrestStep::Hold(hold) => {
            *attempt = ArrestAttempt {
                cop: Some(cop),
                hold,
            }
        }
        ArrestStep::Busted => {
            attempt.cop = Some(cop);
            next.set(GameState::Busted);
        }
        ArrestStep::BrokeFree => {
            wanted.heat = break_free_heat(wanted.heat, &wanted_cfg.stars);
            *attempt = ArrestAttempt::default();
        }
    }
}

pub(super) fn reset_arrest(mut attempt: ResMut<ArrestAttempt>, mut alert: ResMut<PoliceAlert>) {
    *attempt = ArrestAttempt::default();
    *alert = PoliceAlert::default();
}
