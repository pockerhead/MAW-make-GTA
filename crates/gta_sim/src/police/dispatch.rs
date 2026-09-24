//! `PoliceDispatcher`: foot units up to the star row, spawned where the player cannot see them, and
//! sent home once off-frame (GDD §6.1, §6.4).

use super::fsm::{pick_spawn, spawn_kind};
use super::{
    CopState, EscalationConfig, PoliceDispatcher, PoliceRng, PoliceUnit, UnitKind,
    police_unit_bundle,
};
use crate::character::{Character, CharacterControlConfig, Dead, HealthConfig, LocomotionConfig};
use crate::combat::{WeaponsConfig, aim_yaw};
use crate::navigation::{SidewalkGraph, flat_distance};
use crate::player::Player;
use crate::population::{
    Appearance, CameraView, OCCLUSION_RAYS_PER_POINT, Offscreen, PopulationConfig, PopulationLoad,
    occluded, outside_cone, spawn_points,
};
use crate::wanted::WantedLevel;
use avian3d::prelude::*;
use bevy::prelude::*;

/// Off-frame ageing like the gangs (view cone only); a leaving unit goes once off-frame, any other
/// once off-frame and past `despawn_distance`. Corpses belong to `age_corpses`.
pub(super) fn despawn_police(
    mut commands: Commands,
    cfg: Res<PopulationConfig>,
    view: Res<CameraView>,
    loco: Res<LocomotionConfig>,
    time: Res<Time<Fixed>>,
    player: Query<&Position, With<Player>>,
    mut units: Query<(Entity, &Position, &mut Offscreen, &PoliceUnit)>,
) {
    let Some(view) = view.0 else {
        return;
    };
    let Ok(player) = player.single() else {
        return;
    };
    let dt = time.delta_secs();
    for (entity, position, mut offscreen, unit) in &mut units {
        if unit.state == CopState::Dead {
            continue;
        }
        let feet = position.0 - Vec3::Y * loco.float_height;
        if outside_cone(&view, feet, loco.head_height, 0.0) {
            offscreen.0 += dt;
        } else {
            offscreen.0 = 0.0;
        }
        let gone = unit.state == CopState::Leave
            || flat_distance(position.0, player.0) > cfg.despawn_distance;
        if offscreen.0 >= cfg.despawn_offscreen_seconds && gone {
            commands.entity(entity).try_despawn();
        }
    }
}

/// Keeps the active units (not dead, not leaving) at the current star row: SWAT first up to the row's
/// share, one reinforcement delay after a loss, at most `spawns_per_tick` per tick, on sidewalk points
/// of the spawn ring hidden from the camera (outside the cone or occluded), nearest to the last known
/// position first or, when the row surrounds, from the side farthest from the units already out.
#[allow(clippy::too_many_arguments, clippy::type_complexity)]
pub(super) fn dispatch_police(
    mut commands: Commands,
    configs: (
        Res<EscalationConfig>,
        Res<PopulationConfig>,
        Res<WeaponsConfig>,
    ),
    characters: (
        Res<LocomotionConfig>,
        Res<HealthConfig>,
        Res<CharacterControlConfig>,
    ),
    state: (Res<WantedLevel>, Res<CameraView>, Res<SidewalkGraph>),
    time: Res<Time<Fixed>>,
    mut dispatcher: ResMut<PoliceDispatcher>,
    mut load: ResMut<PopulationLoad>,
    mut rng: ResMut<PoliceRng>,
    spatial: SpatialQuery,
    player: Query<&Position, With<Player>>,
    units: Query<(&Position, &PoliceUnit)>,
    lost: Query<(), (With<PoliceUnit>, Added<Dead>)>,
    bodies: Query<&Position, With<Character>>,
) {
    let (esc, cfg, weapons) = configs;
    let (loco, health, handle) = characters;
    let (wanted, view, graph) = state;
    let row = (wanted.stars >= 1).then(|| &esc.stars[usize::from(wanted.stars) - 1]);
    dispatcher.reinforce_left = (dispatcher.reinforce_left - time.delta_secs()).max(0.0);
    if let (false, Some(row)) = (lost.is_empty(), row) {
        dispatcher.reinforce_left = row.reinforce_seconds;
    }
    let mut active: Vec<(Vec3, UnitKind)> = units
        .iter()
        .filter(|(_, u)| !matches!(u.state, CopState::Dead | CopState::Leave))
        .map(|(p, u)| (p.0, u.kind))
        .collect();
    let count = |active: &[(Vec3, UnitKind)]| {
        let swat = active.iter().filter(|a| a.1 == UnitKind::Swat).count() as u32;
        (active.len() as u32, swat)
    };
    (dispatcher.units, dispatcher.swat) = count(&active);
    let (Some(row), Some(view), Some(centre), Ok(player)) =
        (row, view.0, wanted.last_known, player.single())
    else {
        return;
    };
    if dispatcher.reinforce_left > 0.0
        || spawn_kind(row, dispatcher.units, dispatcher.swat).is_none()
    {
        return;
    }
    let (inner, outer) = esc.spawn_ring;
    let separation = cfg.spawn_min_separation;
    let taken: Vec<Vec3> = bodies.iter().map(|p| p.0).collect();
    let mut candidates: Vec<Vec3> = spawn_points(&graph, cfg.spawn_point_spacing)
        .map(|p| p.at)
        .filter(|&at| (inner..=outer).contains(&flat_distance(at, player.0)))
        .filter(|&at| taken.iter().all(|&p| flat_distance(p, at) >= separation))
        .collect();
    let margin = cfg.spawn_view_margin_deg.to_radians();
    let mut spawned = 0;
    while spawned < esc.spawns_per_tick {
        let (units, swat) = count(&active);
        let Some(kind) = spawn_kind(row, units, swat) else {
            break;
        };
        let out: Vec<Vec3> = active.iter().map(|a| a.0).collect();
        let Some(k) = pick_spawn(&candidates, centre, &out, row.surround) else {
            break;
        };
        let at = candidates.remove(k);
        if out.iter().any(|&p| flat_distance(p, at) < separation) {
            continue;
        }
        let hidden = outside_cone(&view, at, loco.head_height, margin)
            || (load.rays + OCCLUSION_RAYS_PER_POINT <= cfg.occlusion_rays_per_tick
                && occluded(
                    &spatial,
                    &view,
                    at,
                    cfg.occlusion_ray_height,
                    loco.capsule_radius,
                    &mut load.rays,
                ));
        if !hidden {
            continue;
        }
        commands.spawn(police_unit_bundle(
            &loco,
            handle.0.clone(),
            &health,
            &weapons,
            esc.spec(kind),
            kind,
            at,
            aim_yaw(centre - at),
            Appearance(rng.next_u32()),
        ));
        active.push((at + Vec3::Y * loco.float_height, kind));
        spawned += 1;
    }
    (dispatcher.units, dispatcher.swat) = count(&active);
}
