//! The arrest (GDD §6.4): a cop in `Arrest` close to a passive player for `arrest.seconds` makes the
//! player `Busted`; running off breaks free for one more star.

use super::fsm::{ArrestStep, arrest_step, break_free_heat};
use super::{CopState, EscalationConfig, PoliceAlert, PoliceUnit};
use crate::character::{Dead, HeadHitbox, LocomotionConfig};
use crate::combat::{HitReaction, Melee, ShotFired};
use crate::flow::GameState;
use crate::navigation::flat_distance;
use crate::player::Player;
use crate::vehicle::{Driving, Vehicle, VehicleConfig, door_point, pull_out};
use crate::wanted::{WantedConfig, WantedLevel};
use avian3d::prelude::*;
use bevy::prelude::*;

/// The running arrest: the arresting cop, the seconds of passivity held so far and the seconds a cop
/// has been pulling the driver out of a stopped car; read by QA.
#[derive(Resource, Reflect, Default, Clone, Copy, Debug, PartialEq)]
#[reflect(Resource)]
pub struct ArrestAttempt {
    pub cop: Option<Entity>,
    pub hold: f32,
    pub pull: f32,
}

/// On an arrest row a cop in `Arrest` at the door of the player's stopped car pulls the driver out
/// through the left door after `pull_out_seconds` (T15); the arrest then runs on foot.
#[allow(clippy::too_many_arguments, clippy::type_complexity)]
pub(super) fn pull_out_driver(
    mut commands: Commands,
    spatial: SpatialQuery,
    configs: (
        Res<EscalationConfig>,
        Res<VehicleConfig>,
        Res<LocomotionConfig>,
    ),
    state: (Res<WantedLevel>, Res<PoliceAlert>, Res<Time<Fixed>>),
    mut attempt: ResMut<ArrestAttempt>,
    player: Query<(Entity, &Driving, Option<&Children>), (With<Player>, Without<Dead>)>,
    mut vehicles: Query<(&mut Vehicle, &Position, &Rotation, &LinearVelocity)>,
    cops: Query<(Entity, &Position, &PoliceUnit)>,
    heads: Query<(), With<HeadHitbox>>,
) {
    let (esc, vehicle_cfg, loco) = configs;
    let (wanted, alert, time) = state;
    let Ok((me, driving, children)) = player.single() else {
        return;
    };
    let arrests = wanted.stars >= 1
        && esc.stars[usize::from(wanted.stars) - 1].arrest
        && alert.hostile_left <= 0.0;
    let Ok((mut vehicle, car_pos, car_rot, car_vel)) = vehicles.get_mut(driving.vehicle) else {
        return;
    };
    let door = door_point(vehicle_cfg.door(), car_pos.0, car_rot.0);
    let arresting: Vec<(Entity, f32)> = cops
        .iter()
        .filter(|(.., unit)| unit.state == CopState::Arrest)
        .map(|(cop, p, _)| (cop, flat_distance(p.0, door)))
        .collect();
    let cop = arresting
        .iter()
        .copied()
        .filter(|&(_, distance)| distance <= esc.arrest.distance)
        .min_by(|a, b| a.1.total_cmp(&b.1));
    let (Some((cop, _)), true, true) =
        (cop, arrests, car_vel.length() <= vehicle_cfg.exit_max_speed)
    else {
        attempt.pull = 0.0;
        return;
    };
    attempt.pull += time.delta_secs();
    if attempt.pull < esc.arrest.pull_out_seconds {
        return;
    }
    // The arresting cops crowd the door they pull at: their bodies never block it.
    let pullers: Vec<Entity> = arresting.iter().map(|&(cop, _)| cop).collect();
    let give_up = attempt.pull >= esc.arrest.pull_out_seconds + esc.arrest.pull_give_up_seconds;
    let pulled = pull_out(
        &mut commands,
        &spatial,
        &vehicle_cfg,
        &loco,
        (me, children),
        &heads,
        (driving.vehicle, car_pos.0, car_rot.0),
        &mut vehicle,
        &pullers,
        give_up,
    );
    if pulled && !give_up {
        *attempt = ArrestAttempt {
            cop: Some(cop),
            hold: 0.0,
            pull: 0.0,
        };
    } else if give_up {
        // Out at another exit (or nowhere to go): the foot arrest starts over, never a break-free
        // from a cop at the far door; with no exit at all the pull starts over too.
        *attempt = ArrestAttempt::default();
    }
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
                pull: 0.0,
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
