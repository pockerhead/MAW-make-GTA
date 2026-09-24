//! Minimap dots (police, gang members, pickups, cars) and landmark glyphs (hospital, police station):
//! ordinary UI children of the minimap root, placed by `map_px`.

use super::{Minimap, MinimapConfig};
use crate::camera::OrbitCamera;
use crate::menu::{UiConfig, UiFonts};
use crate::visuals::CharacterVisualConfig;
use bevy::prelude::*;
use gta_sim::{
    character::Dead,
    combat::{BatPickup, Pickup, WeaponPickup},
    gang::{GangConfig, GangMember},
    player::Player,
    police::PoliceUnit,
    vehicle::{Driving, Vehicle},
    world::{HospitalSpawn, PoliceStationSpawn, map_px, project},
};
use std::collections::HashSet;

#[derive(Reflect, Clone, Copy, PartialEq, Eq, Debug)]
pub enum MarkerKind {
    Police,
    Gang(u8),
    Pickup,
    Vehicle,
    Hospital,
    Station,
}

/// A mark on the minimap; `target` is the tracked entity (`None` for landmarks). Read by QA.
#[derive(Component, Reflect, Clone, Copy, Debug)]
#[reflect(Component)]
pub struct MinimapMarker {
    pub target: Option<Entity>,
    pub kind: MarkerKind,
}

fn color((r, g, b): (f32, f32, f32)) -> Color {
    Color::srgb(r.clamp(0.0, 1.0), g.clamp(0.0, 1.0), b.clamp(0.0, 1.0))
}

fn dot(cfg: &MinimapConfig, target: Entity, kind: MarkerKind, tint: Color) -> impl Bundle {
    (
        Name::new("Minimap dot"),
        MinimapMarker {
            target: Some(target),
            kind,
        },
        Node {
            position_type: PositionType::Absolute,
            width: px(cfg.dot_px),
            height: px(cfg.dot_px),
            border_radius: BorderRadius::MAX,
            ..default()
        },
        UiTransform::from_translation(Val2::percent(-50, -50)),
        BackgroundColor(tint),
        Visibility::Hidden,
    )
}

/// Hospital and police station glyphs, children of the minimap `root`.
pub(super) fn spawn_landmarks(
    commands: &mut Commands,
    root: Entity,
    cfg: &MinimapConfig,
    fonts: &UiFonts,
) {
    for (kind, (glyph, tint)) in [
        (MarkerKind::Hospital, &cfg.hospital),
        (MarkerKind::Station, &cfg.station),
    ] {
        commands.spawn((
            Name::new("Minimap landmark"),
            MinimapMarker { target: None, kind },
            Text::new(glyph.clone()),
            TextFont {
                font: FontSource::Handle(fonts.regular.clone()),
                font_size: FontSize::from(cfg.glyph_size),
                ..default()
            },
            TextColor(color(*tint)),
            Node {
                position_type: PositionType::Absolute,
                ..default()
            },
            UiTransform::from_translation(Val2::percent(-50, -50)),
            ChildOf(root),
        ));
    }
}

type Tracked<'w, 's, T> = Query<'w, 's, (Entity, &'static T), Without<Dead>>;

/// Adds a dot for every new live cop, gang member, available pickup and car the player does not
/// drive; drops dots whose target is gone, dead, taken or driven.
#[allow(clippy::too_many_arguments, clippy::type_complexity)]
pub(super) fn sync_markers(
    mut commands: Commands,
    root: Single<Entity, With<Minimap>>,
    ui: Res<UiConfig>,
    visual: Res<CharacterVisualConfig>,
    gangs: Res<GangConfig>,
    markers: Query<(Entity, &MinimapMarker)>,
    cops: Tracked<PoliceUnit>,
    members: Tracked<GangMember>,
    pickups: (
        Query<(Entity, &Pickup)>,
        Query<(Entity, &WeaponPickup)>,
        Query<(Entity, &BatPickup)>,
    ),
    cars: (Query<Entity, With<Vehicle>>, Query<&Driving, With<Player>>),
) {
    let cfg = &ui.hud.minimap;
    let (health, guns, bats) = pickups;
    let mut live: Vec<(Entity, MarkerKind, Color)> = Vec::new();
    let police = color(visual.police_tint);
    live.extend(cops.iter().map(|(e, _)| (e, MarkerKind::Police, police)));
    for (e, member) in &members {
        let tint = gangs
            .gangs
            .get(usize::from(member.gang))
            .map_or(Color::WHITE, |g| color(g.tint));
        live.push((e, MarkerKind::Gang(member.gang), tint));
    }
    let pickup = color(cfg.pickup_color);
    live.extend(
        health
            .iter()
            .filter(|(_, p)| p.available())
            .map(|(e, _)| (e, MarkerKind::Pickup, pickup)),
    );
    live.extend(
        guns.iter()
            .filter(|(_, p)| p.available())
            .map(|(e, _)| (e, MarkerKind::Pickup, pickup)),
    );
    live.extend(
        bats.iter()
            .filter(|(_, p)| p.available())
            .map(|(e, _)| (e, MarkerKind::Pickup, pickup)),
    );
    let (vehicles, driving) = cars;
    let driven = driving.single().ok().map(|d| d.vehicle);
    let car = color(cfg.vehicle_color);
    live.extend(
        vehicles
            .iter()
            .filter(|&e| Some(e) != driven)
            .map(|e| (e, MarkerKind::Vehicle, car)),
    );
    let wanted: HashSet<Entity> = live.iter().map(|l| l.0).collect();
    let mut shown = HashSet::new();
    for (marker, mark) in &markers {
        let Some(target) = mark.target else {
            continue;
        };
        if wanted.contains(&target) {
            shown.insert(target);
        } else {
            commands.entity(marker).try_despawn();
        }
    }
    for (target, kind, tint) in live {
        if shown.contains(&target) {
            continue;
        }
        commands.spawn((dot(cfg, target, kind, tint), ChildOf(*root)));
    }
}

/// Moves every mark to its place on the rotated map; NPC and pickup dots beyond the rim are hidden,
/// landmark glyphs stick to the rim.
#[allow(clippy::type_complexity)]
pub(super) fn place_markers(
    ui: Res<UiConfig>,
    hospital: Res<HospitalSpawn>,
    station: Res<PoliceStationSpawn>,
    player: Single<&Transform, With<Player>>,
    camera: Single<&OrbitCamera>,
    targets: Query<&Transform, Without<MinimapMarker>>,
    mut markers: Query<(&MinimapMarker, &mut Node, &mut Visibility)>,
) {
    let cfg = &ui.hud.minimap;
    let centre = player.translation.xz();
    let yaw = camera.yaw;
    let half = cfg.size / 2.0;
    let px_per_m = half / cfg.view_radius;
    for (marker, mut node, mut visibility) in &mut markers {
        let point = match (marker.kind, marker.target) {
            (MarkerKind::Hospital, _) => hospital.point,
            (MarkerKind::Station, _) => station.point,
            (_, Some(target)) => {
                let Ok(transform) = targets.get(target) else {
                    continue;
                };
                transform.translation
            }
            (_, None) => continue,
        };
        let mut at = map_px(point.xz(), centre, yaw, px_per_m);
        let landmark = marker.target.is_none();
        if landmark {
            at = at.clamp_length_max(half - cfg.glyph_size / 2.0);
        } else {
            let inside = project(point.xz(), centre, yaw).length() <= cfg.view_radius;
            let shown = if inside {
                Visibility::Inherited
            } else {
                Visibility::Hidden
            };
            visibility.set_if_neq(shown);
        }
        node.left = px(half + at.x);
        node.top = px(half + at.y);
    }
}
