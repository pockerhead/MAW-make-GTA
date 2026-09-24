//! Ammo counter, crosshair and hit marker (GDD §7).

use super::{rgb, rgba};
use crate::camera::OrbitCamera;
use crate::menu::{UiConfig, UiFonts};
use bevy::{prelude::*, window::PrimaryWindow};
use gta_sim::{
    combat::{DamageDealt, Loadout},
    player::Player,
    world::CityScoped,
};

#[derive(Component)]
pub(super) struct AmmoText;

/// One crosshair arm; `0` points away from the centre in screen space (y down).
#[derive(Component)]
pub(super) struct CrosshairArm(Vec2);

#[derive(Component)]
pub(super) struct CrosshairDot;

/// Real seconds left on screen.
#[derive(Component, Default)]
pub(super) struct HitMarker(f32);

/// A node whose top-left corner sits at the screen centre; `UiTransform` moves it from there.
fn centred(width: f32, height: f32) -> (Node, UiTransform) {
    (
        Node {
            position_type: PositionType::Absolute,
            left: percent(50),
            top: percent(50),
            width: px(width),
            height: px(height),
            ..default()
        },
        UiTransform::from_translation(Val2::px(-width / 2.0, -height / 2.0)),
    )
}

pub(super) fn spawn_weapon_hud(mut commands: Commands, ui: Res<UiConfig>, fonts: Res<UiFonts>) {
    let hud = &ui.hud;
    let font = |size: f32| TextFont {
        font: FontSource::Handle(fonts.regular.clone()),
        font_size: FontSize::from(size),
        ..default()
    };
    commands.spawn((
        Name::new("Ammo"),
        CityScoped,
        AmmoText,
        Text::new(""),
        font(hud.ammo_size),
        TextColor(rgb(hud.ammo_color)),
        Node {
            position_type: PositionType::Absolute,
            top: px(hud.margin + 2.0 * (hud.bar_height + hud.bar_gap)),
            right: px(hud.margin),
            ..default()
        },
        Visibility::Hidden,
    ));
    let color = BackgroundColor(rgba(hud.crosshair_color));
    commands.spawn((
        Name::new("Crosshair dot"),
        CityScoped,
        CrosshairDot,
        centred(hud.crosshair_dot, hud.crosshair_dot),
        color,
        Visibility::Hidden,
    ));
    for dir in [Vec2::X, Vec2::NEG_X, Vec2::Y, Vec2::NEG_Y] {
        let (w, h) = if dir.x != 0.0 {
            (hud.crosshair_arm, hud.crosshair_thickness)
        } else {
            (hud.crosshair_thickness, hud.crosshair_arm)
        };
        commands.spawn((
            Name::new("Crosshair arm"),
            CityScoped,
            CrosshairArm(dir),
            centred(w, h),
            color,
            Visibility::Hidden,
        ));
    }
    commands.spawn((
        Name::new("Hit marker"),
        CityScoped,
        HitMarker::default(),
        Text::new("×"),
        font(hud.hit_marker_size),
        TextColor(rgb(hud.hit_marker_color)),
        Node {
            position_type: PositionType::Absolute,
            left: percent(50),
            top: percent(50),
            ..default()
        },
        UiTransform::from_translation(Val2::percent(-50, -50)),
        Visibility::Hidden,
    ));
}

fn shown(on: bool) -> Visibility {
    if on {
        Visibility::Inherited
    } else {
        Visibility::Hidden
    }
}

pub(super) fn update_ammo(
    player: Query<&Loadout, With<Player>>,
    mut text: Query<(&mut Text, &mut Visibility), With<AmmoText>>,
) {
    let (Ok(loadout), Ok((mut text, mut visibility))) = (player.single(), text.single_mut()) else {
        return;
    };
    let Some(weapon) = loadout.held else {
        visibility.set_if_neq(Visibility::Hidden);
        return;
    };
    let slot = loadout.guns[weapon.index()];
    let label = format!("{} / {}", slot.magazine, slot.reserve);
    if text.0 != label {
        text.0 = label;
    }
    visibility.set_if_neq(Visibility::Inherited);
}

#[allow(clippy::type_complexity)]
pub(super) fn update_crosshair(
    ui: Res<UiConfig>,
    player: Query<&Loadout, With<Player>>,
    camera: Query<(&OrbitCamera, &Projection)>,
    window: Query<&Window, With<PrimaryWindow>>,
    mut dot: Query<&mut Visibility, (With<CrosshairDot>, Without<CrosshairArm>)>,
    mut arms: Query<(&CrosshairArm, &mut UiTransform, &mut Visibility), Without<CrosshairDot>>,
) {
    let (Ok(loadout), Ok((orbit, projection)), Ok(window)) =
        (player.single(), camera.single(), window.single())
    else {
        return;
    };
    let armed = loadout.held.is_some();
    for mut visibility in &mut dot {
        visibility.set_if_neq(shown(armed));
    }
    let Projection::Perspective(perspective) = projection else {
        return;
    };
    let hud = &ui.hud;
    let gap = loadout.spread_deg.to_radians().tan() / (perspective.fov / 2.0).tan()
        * (window.height() / 2.0);
    for (arm, mut transform, mut visibility) in &mut arms {
        visibility.set_if_neq(shown(armed && orbit.aim_blend > 0.0));
        let size = if arm.0.x != 0.0 {
            Vec2::new(hud.crosshair_arm, hud.crosshair_thickness)
        } else {
            Vec2::new(hud.crosshair_thickness, hud.crosshair_arm)
        };
        let centre = arm.0 * (gap + hud.crosshair_arm / 2.0) - size / 2.0;
        transform.translation = Val2::px(centre.x, centre.y);
    }
}

pub(super) fn update_hit_marker(
    ui: Res<UiConfig>,
    real: Res<Time<Real>>,
    mut hits: MessageReader<DamageDealt>,
    players: Query<(), With<Player>>,
    mut marker: Query<(&mut HitMarker, &mut TextColor, &mut Visibility)>,
) {
    let Ok((mut marker, mut color, mut visibility)) = marker.single_mut() else {
        return;
    };
    let mut player_hits = hits.read().filter(|hit| players.contains(hit.shooter));
    if let Some(first) = player_hits.next() {
        // One marker per frame; red if any pellet of it killed.
        let killed = first.killed || player_hits.any(|hit| hit.killed);
        marker.0 = ui.hud.hit_marker_seconds;
        color.0 = rgb(if killed {
            ui.hud.kill_marker_color
        } else {
            ui.hud.hit_marker_color
        });
    } else {
        marker.0 = (marker.0 - real.delta_secs()).max(0.0);
    }
    visibility.set_if_neq(shown(marker.0 > 0.0));
}
