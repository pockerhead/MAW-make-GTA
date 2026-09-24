//! Gang groups at their posts, spawned whole where the player cannot see them and despawned far
//! away off-frame (GDD §6.1, §6.3).

use super::{
    Appearance, CameraView, OCCLUSION_RAYS_PER_POINT, Offscreen, PopulationConfig, PopulationLoad,
    occluded, outside_cone,
};
use crate::character::{Character, CharacterControlConfig, HealthConfig, LocomotionConfig};
use crate::combat::{WeaponsConfig, aim_yaw};
use crate::gang::{
    GangConfig, GangMember, GangRng, GangState, GangTerritories, gang_member_bundle,
};
use crate::navigation::flat_distance;
use crate::player::Player;
use avian3d::prelude::*;
use bevy::prelude::*;

pub(super) fn despawn_far_gangs(
    mut commands: Commands,
    cfg: Res<PopulationConfig>,
    view: Res<CameraView>,
    loco: Res<LocomotionConfig>,
    time: Res<Time<Fixed>>,
    player: Query<&Position, With<Player>>,
    mut members: Query<(Entity, &Position, &mut Offscreen), With<GangMember>>,
) {
    let Some(view) = view.0 else {
        return;
    };
    let Ok(player) = player.single() else {
        return;
    };
    let dt = time.delta_secs();
    for (entity, position, mut offscreen) in &mut members {
        let feet = position.0 - Vec3::Y * loco.float_height;
        if outside_cone(&view, feet, loco.head_height, 0.0) {
            offscreen.0 += dt;
        } else {
            offscreen.0 = 0.0;
        }
        if offscreen.0 >= cfg.despawn_offscreen_seconds
            && flat_distance(position.0, player.0) > cfg.despawn_distance
        {
            commands.entity(entity).try_despawn();
        }
    }
}

/// One group per free post in the spawn ring, HQ posts first, then the nearest; a post is skipped
/// this tick when any of its spots is visible, crowded, or the occlusion ray budget is spent.
#[allow(clippy::too_many_arguments)]
pub(super) fn spawn_gangs(
    mut commands: Commands,
    configs: (Res<PopulationConfig>, Res<GangConfig>, Res<WeaponsConfig>),
    characters: (
        Res<LocomotionConfig>,
        Res<HealthConfig>,
        Res<CharacterControlConfig>,
    ),
    view: Res<CameraView>,
    territories: Res<GangTerritories>,
    mut load: ResMut<PopulationLoad>,
    mut rng: ResMut<GangRng>,
    spatial: SpatialQuery,
    player: Query<&Position, With<Player>>,
    members: Query<&GangMember>,
    bodies: Query<&Position, With<Character>>,
) {
    let (cfg, gang_cfg, weapons) = configs;
    let (loco, health, handle) = characters;
    let Some(view) = view.0 else {
        return;
    };
    let Ok(player) = player.single() else {
        return;
    };
    let mut alive = members
        .iter()
        .filter(|m| m.state != GangState::Dead)
        .count() as u32;
    if alive >= cfg.max_gang_members {
        return;
    }
    let occupied = members.iter().map(|m| (m.gang, m.post)).collect::<Vec<_>>();
    let (inner, outer) = cfg.spawn_ring;
    let mut candidates = Vec::new();
    for (gang, turf) in territories.gangs.iter().enumerate() {
        for (post, &at) in turf.posts.iter().enumerate() {
            let (gang, post) = (gang as u8, post as u16);
            let d = flat_distance(at, player.0);
            if (inner..=outer).contains(&d) && !occupied.contains(&(gang, post)) {
                candidates.push((gang, post, at, d));
            }
        }
    }
    candidates.sort_by(|a, b| {
        (a.1 != 0)
            .cmp(&(b.1 != 0))
            .then(a.3.total_cmp(&b.3))
            .then(a.0.cmp(&b.0))
    });
    let taken = bodies.iter().map(|p| p.0).collect::<Vec<_>>();
    let groups = &gang_cfg.groups;
    let margin = cfg.spawn_view_margin_deg.to_radians();
    for (gang, post, at, _) in candidates {
        let (lo, hi) = groups.size;
        let size = lo + rng.next_u32() % (hi - lo + 1);
        if alive + size > cfg.max_gang_members {
            continue;
        }
        let spots = (0..size)
            .map(|k| {
                let theta = std::f32::consts::TAU * k as f32 / size as f32;
                at + Vec3::new(theta.cos(), 0.0, -theta.sin()) * groups.spread
            })
            .collect::<Vec<_>>();
        let crowded = spots.iter().any(|&spot| {
            taken
                .iter()
                .any(|&p| flat_distance(p, spot) < cfg.spawn_min_separation)
        });
        if crowded {
            continue;
        }
        let hidden = spots.iter().all(|&spot| {
            if outside_cone(&view, spot, loco.head_height, margin) {
                return true;
            }
            load.rays + OCCLUSION_RAYS_PER_POINT <= cfg.occlusion_rays_per_tick
                && occluded(
                    &spatial,
                    &view,
                    spot,
                    cfg.occlusion_ray_height,
                    loco.capsule_radius,
                    &mut load.rays,
                )
        });
        if !hidden {
            continue;
        }
        let arsenal = &gang_cfg.gangs[gang as usize].weapons;
        for spot in spots {
            let gun = arsenal[rng.next_u32() as usize % arsenal.len()];
            let appearance = Appearance(rng.next_u32());
            commands.spawn(gang_member_bundle(
                &loco,
                handle.0.clone(),
                &health,
                &weapons,
                gang,
                post,
                spot,
                aim_yaw(at - spot),
                gun,
                appearance,
            ));
        }
        alive += size;
    }
}
