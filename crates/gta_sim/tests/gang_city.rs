//! Gang territories and group spawning in the generated seed-1 city (GDD §6.1, §6.3).

mod common;

use avian3d::prelude::*;
use bevy::{ecs::system::RunSystemOnce, prelude::*};
use common::*;
use gta_sim::{
    character::LocomotionConfig,
    combat::Weapon,
    gang::{GangMember, GangState, GangTerritories},
    navigation::{SidewalkGraph, flat_distance},
    perception::sight_blocked,
    population::{PopulationConfig, PopulationLoad, ViewCone, outside_cone},
    world::{BuildingKind, City},
};

fn loco(app: &App) -> LocomotionConfig {
    app.world().resource::<LocomotionConfig>().clone()
}

fn population_cfg(app: &App) -> PopulationConfig {
    app.world().resource::<PopulationConfig>().clone()
}

fn posts(app: &App, gang: usize) -> Vec<Vec3> {
    app.world()
        .get_resource::<GangTerritories>()
        .expect("GATE BROKEN: no GangTerritories after load")
        .gangs[gang]
        .posts
        .clone()
}

#[test]
fn territories_come_from_the_generator() {
    let app = city_app(1);
    let territories = app
        .world()
        .get_resource::<GangTerritories>()
        .expect("no GangTerritories after load");
    assert_eq!(territories.gangs.len(), 2);
    let layout = &app.world().resource::<City>().0;
    let params = city_params(&app);
    let margin = gang_cfg(&app).groups.hq_margin;
    let curb = params.roads.curb_height;
    for g in 0..2u8 {
        let turf = &territories.gangs[g as usize];
        let hq = layout
            .buildings
            .iter()
            .position(|b| b.kind == BuildingKind::GangHq(g))
            .expect("GATE BROKEN: no gang HQ in the layout");
        let (anchor, _) = citygen::sidewalk_anchor(layout, params, hq, margin)
            .expect("GATE BROKEN: HQ without a sidewalk anchor");
        let expected = Vec3::new(anchor.x, curb, anchor.y);
        assert!(
            turf.posts[0].distance(expected) < 1e-3,
            "gang {g}: HQ post {} vs anchor {expected}",
            turf.posts[0]
        );
        for &post in &turf.posts {
            assert_eq!(
                territories.territory_at(post),
                Some(g),
                "gang {g} post {post}"
            );
        }
        let district = layout.gang_districts[g as usize];
        let block = layout
            .blocks
            .iter()
            .find(|b| b.district == district && b.curb.len() >= 3)
            .expect("GATE BROKEN: gang district without blocks");
        let c = citygen::centroid(&block.curb);
        assert_eq!(
            territories.territory_at(Vec3::new(c.x, curb, c.y)),
            Some(g),
            "a block of gang {g}'s district"
        );
    }
    let spawn = layout.player_spawn;
    assert_eq!(
        territories.territory_at(Vec3::new(spawn.x, curb, spawn.y)),
        None,
        "the seed-1 spawn is neutral"
    );
}

/// The gang-0 post nearest to the HQ among posts whose HQ distance lies in the spawn ring.
fn ring_post(app: &App) -> Vec3 {
    let posts = posts(app, 0);
    let (inner, outer) = population_cfg(app).spawn_ring;
    posts[1..]
        .iter()
        .copied()
        .filter(|&p| (inner..=outer).contains(&flat_distance(p, posts[0])))
        .min_by(|a, b| flat_distance(*a, posts[0]).total_cmp(&flat_distance(*b, posts[0])))
        .expect("GATE BROKEN: no gang-0 post in the spawn ring of the HQ")
}

/// A point `distance` m from sidewalk node `node` along one of its edges at least twice as long.
fn along_sidewalk(app: &App, node: Vec3, distance: f32) -> Vec3 {
    let graph = app.world().resource::<SidewalkGraph>();
    let id = (0..graph.nodes().len() as u32)
        .find(|&n| graph.node(n).distance(node) < 1e-3)
        .expect("GATE BROKEN: the ring post is not a sidewalk node");
    let next = graph
        .neighbors(id)
        .iter()
        .map(|&n| graph.node(n))
        .find(|&n| flat_distance(n, node) >= 2.0 * distance)
        .expect("GATE BROKEN: no long sidewalk edge at the ring post");
    node + (next - node).with_y(0.0).normalize() * distance
}

fn blocked(app: &mut App, from: Vec3, to: Vec3) -> bool {
    app.world_mut()
        .run_system_once(move |spatial: SpatialQuery| sight_blocked(&spatial, from, to))
        .expect("GATE BROKEN: ray system failed")
}

/// Head centre, both head sides (capsule half-width) and the feet are hidden from the camera.
fn hidden_behind_geometry(app: &mut App, origin: Vec3, feet: Vec3) -> bool {
    let head = feet + Vec3::Y * population_cfg(app).occlusion_ray_height;
    let side = (feet - origin).with_y(0.0).cross(Vec3::Y).normalize() * loco(app).capsule_radius;
    [head, head + side, head - side, feet + Vec3::Y * 0.1]
        .into_iter()
        .all(|to| blocked(app, origin, to))
}

fn gang_members(app: &mut App) -> Vec<(Entity, GangMember, Vec3)> {
    app.world_mut()
        .query::<(Entity, &GangMember, &Position)>()
        .iter(app.world())
        .map(|(e, m, p)| (e, m.clone(), p.0))
        .collect()
}

/// Seed 1, player on the ring post, the camera turning 360° every 8 s for 640 ticks; returns the
/// number of groups spawned. Asserts every spawn and the per-tick bounds.
fn spawn_under_a_turning_view(max_gang_members: Option<u32>) -> usize {
    let mut app = city_app(1);
    if let Some(max) = max_gang_members {
        set_population(&mut app, |p| p.max_gang_members = max);
    }
    let cfg = population_cfg(&app);
    let gangs = gang_cfg(&app);
    let l = loco(&app);
    let post = ring_post(&app);
    let at = along_sidewalk(&app, post, 6.0);
    place_player(&mut app, at + Vec3::Y * l.float_height);
    let (inner, outer) = cfg.spawn_ring;
    let all_posts = [posts(&app, 0), posts(&app, 1)];
    // The free ring post 6 m away is spawnable but for the ring (not crowded: 6 > 4 + 1 m).
    let near = flat_distance(post, at);
    assert!(
        near < inner && near > cfg.spawn_min_separation + gangs.groups.spread,
        "GATE BROKEN: the player must stand {near} m from a free post inside the ring"
    );
    let margin = cfg.spawn_view_margin_deg.to_radians();
    let mut known = Vec::new();
    let mut groups = 0;
    for k in 0..640u32 {
        let yaw = std::f32::consts::TAU * k as f32 / 512.0;
        let dir = Vec3::new(-yaw.sin(), 0.0, -yaw.cos());
        let feet = position(&mut app) - Vec3::Y * l.float_height;
        let view: ViewCone = chase_view(feet, dir);
        set_view(&mut app, Some(view));
        run_ticks(&mut app, 1);
        let members = gang_members(&mut app);
        let alive = members
            .iter()
            .filter(|(_, m, _)| m.state != GangState::Dead)
            .count() as u32;
        assert!(
            alive <= cfg.max_gang_members,
            "tick {k}: {alive} alive members"
        );
        let rays = app.world().resource::<PopulationLoad>().rays;
        assert!(rays <= cfg.occlusion_rays_per_tick, "tick {k}: {rays} rays");
        let fresh: Vec<_> = members
            .into_iter()
            .filter(|(e, ..)| !known.contains(e))
            .collect();
        let mut sizes: Vec<((u8, u16), u32)> = Vec::new();
        for (e, m, chest) in fresh {
            known.push(e);
            let post = all_posts[m.gang as usize][m.post as usize];
            let d = flat_distance(post, position(&mut app));
            assert!(
                (inner - 0.5..=outer + 0.5).contains(&d),
                "tick {k}: group at {d} m from the player"
            );
            let body = chest - Vec3::Y * l.float_height;
            let off = flat_distance(body, post);
            assert!(
                off <= gangs.groups.spread + 0.05,
                "tick {k}: {off} m off its post"
            );
            let hidden = outside_cone(&view, body, l.head_height, margin)
                || hidden_behind_geometry(&mut app, view.origin, body);
            assert!(hidden, "tick {k}: member spawned in clear view at {body}");
            match sizes.iter_mut().find(|(key, _)| *key == (m.gang, m.post)) {
                Some((_, n)) => *n += 1,
                None => sizes.push(((m.gang, m.post), 1)),
            }
        }
        for ((gang, post), n) in sizes {
            let (lo, hi) = gangs.groups.size;
            assert!(
                (lo..=hi).contains(&n),
                "tick {k}: group ({gang}, {post}) of {n}"
            );
            groups += 1;
        }
    }
    groups
}

#[test]
fn groups_spawn_hidden_in_the_ring() {
    let groups = spawn_under_a_turning_view(None);
    assert!(groups >= 1, "no group spawned in 640 ticks");
    // A tight cap is honoured too.
    spawn_under_a_turning_view(Some(4));
}

#[test]
fn hq_group_spawns_behind_a_static_view() {
    let mut app = city_app(1);
    set_population(&mut app, |p| p.max_civilians = 0);
    let cfg = population_cfg(&app);
    let gangs = gang_cfg(&app);
    let l = loco(&app);
    let at = ring_post(&app);
    place_player(&mut app, at + Vec3::Y * l.float_height);
    let hq = posts(&app, 0)[0];
    let dir = -(hq - at).with_y(0.0).normalize();
    let view = chase_view(at, dir);
    let (inner, outer) = cfg.spawn_ring;
    assert!(
        (inner..=outer).contains(&flat_distance(hq, at)),
        "GATE BROKEN: HQ post outside the spawn ring"
    );
    let margin = cfg.spawn_view_margin_deg.to_radians();
    let (lo, hi) = gangs.groups.size;
    for size in lo..=hi {
        for k in 0..size {
            let theta = std::f32::consts::TAU * k as f32 / size as f32;
            let spot = hq + Vec3::new(theta.cos(), 0.0, -theta.sin()) * gangs.groups.spread;
            assert!(
                outside_cone(&view, spot, l.head_height, margin),
                "GATE BROKEN: HQ spot {spot} inside the static view"
            );
        }
    }
    set_view(&mut app, Some(view));
    run_ticks(&mut app, 2);
    let hq_group = gang_members(&mut app)
        .iter()
        .filter(|(_, m, _)| m.gang == 0 && m.post == 0)
        .count();
    assert!(
        hq_group >= lo as usize,
        "no HQ group after 2 ticks ({hq_group} members)"
    );
}

#[test]
fn far_gang_member_despawns_after_2s_offscreen() {
    let mut app = city_app(1);
    let l = loco(&app);
    let player_at = position(&mut app);
    let node_at = |app: &App, lo: f32, hi: f32| {
        app.world()
            .resource::<SidewalkGraph>()
            .nodes()
            .iter()
            .copied()
            .find(|&n| (lo..=hi).contains(&flat_distance(n, player_at)))
            .expect("GATE BROKEN: no sidewalk node at that distance")
    };
    let far = node_at(&app, 155.0, 170.0);
    let near = node_at(&app, 135.0, 148.0);
    let cfg = population_cfg(&app);
    assert!(
        flat_distance(far, player_at) > cfg.despawn_distance
            && flat_distance(near, player_at) < cfg.despawn_distance,
        "GATE BROKEN: the fixture must straddle despawn_distance"
    );
    let far_member = spawn_member(&mut app, 0, far, Weapon::Pistol);
    let near_member = spawn_member(&mut app, 0, near, Weapon::Pistol);
    let eyes = player_at - Vec3::Y * l.float_height + Vec3::Y * l.head_height;
    let up = ViewCone::from_perspective(eyes, Dir3::Y, 70f32.to_radians(), 16.0 / 9.0);
    set_view(&mut app, Some(up));
    let ticks = (cfg.despawn_offscreen_seconds * 64.0) as u32;
    run_ticks(&mut app, ticks - 1);
    assert!(app.world().get_entity(far_member).is_ok(), "gone early");
    run_ticks(&mut app, 1);
    assert!(
        app.world().get_entity(far_member).is_err(),
        "still there at {ticks} ticks"
    );
    assert!(
        app.world().get_entity(near_member).is_ok(),
        "the near member was despawned"
    );
}
