//! Floating damage numbers: one UI label per shot and target (the sum of its `DamageDealt`
//! pellets), anchored to the projected hit point.
//!
//! A UI node instead of a world-space billboard: Bevy 0.19 has no 3D text (`Text2d` renders only
//! through a 2D camera), and a UI label faces the camera by construction at a readable pixel size.

use super::config::{DamageNumbersConfig, JuiceConfig};
use crate::camera::{OrbitCamera, follow_player};
use crate::menu::{UiConfig, UiFonts};
use bevy::{camera::CameraUpdateSystems, prelude::*, ui::UiSystems};
use gta_sim::{combat::DamageDealt, player::Player};

/// Golden angle in radians: consecutive labels drift to well-separated sides.
const GOLDEN_ANGLE: f32 = 2.399_963;

/// One floating number; `value` is exactly the damage the sim applied to one target by one shot.
#[derive(Component, Reflect)]
#[reflect(Component)]
pub struct DamageNumber {
    pub point: Vec3,
    pub value: u32,
    pub headshot: bool,
    /// Real seconds since spawn.
    pub age: f32,
    /// Sideways direction in [-1, 1].
    pub drift: f32,
}

pub struct DamageNumbersPlugin;

impl Plugin for DamageNumbersPlugin {
    fn build(&self, app: &mut App) {
        app.register_type::<DamageNumber>()
            .add_systems(Update, (spawn_damage_numbers, age_damage_numbers).chain())
            .add_systems(
                PostUpdate,
                place_damage_numbers
                    .after(follow_player)
                    .after(CameraUpdateSystems)
                    .before(UiSystems::Layout),
            );
    }
}

/// Text of a label: the number, or the `{damage}` template of a headshot ("54 CRIT").
pub fn label_text(value: u32, headshot: bool, crit_template: &str) -> String {
    if headshot {
        crit_template.replace("{damage}", &value.to_string())
    } else {
        value.to_string()
    }
}

/// Screen offset in px (y down), scale and alpha of a label `age` seconds old.
pub fn pose(age: f32, drift: f32, cfg: &DamageNumbersConfig) -> (Vec2, f32, f32) {
    let n = (age / cfg.lifetime).clamp(0.0, 1.0);
    let rise = cfg.rise_px * (1.0 - (1.0 - n).powi(3));
    let offset = Vec2::new(drift * cfg.drift_px * n, -rise);
    let scale = if age < cfg.pop_seconds {
        1.0 + (cfg.pop_start_scale - 1.0) * (1.0 - age / cfg.pop_seconds).powi(2)
    } else {
        1.0
    };
    let alpha = if n < cfg.fade_start {
        1.0
    } else {
        1.0 - (n - cfg.fade_start) / (1.0 - cfg.fade_start)
    };
    (offset, scale, alpha)
}

fn rgb((r, g, b): (f32, f32, f32)) -> Color {
    Color::srgb(r, g, b)
}

/// Merges the pellets of one shot on one target, in first-hit order: damage summed, headshot if
/// any pellet was one, point at the pellets' mean.
fn sum_per_shot_and_target<'a>(hits: impl Iterator<Item = &'a DamageDealt>) -> Vec<DamageDealt> {
    let mut merged: Vec<(DamageDealt, f32)> = Vec::new();
    for hit in hits {
        let key = (hit.shooter, hit.shot, hit.target);
        let Some((sum, pellets)) = merged
            .iter_mut()
            .find(|(m, _)| (m.shooter, m.shot, m.target) == key)
        else {
            merged.push((*hit, 1.0));
            continue;
        };
        *pellets += 1.0;
        sum.damage += hit.damage;
        sum.headshot |= hit.headshot;
        sum.killed |= hit.killed;
        sum.point += (hit.point - sum.point) / *pellets;
    }
    merged.into_iter().map(|(hit, _)| hit).collect()
}

fn spawn_damage_numbers(
    mut commands: Commands,
    mut hits: MessageReader<DamageDealt>,
    players: Query<(), With<Player>>,
    juice: Res<JuiceConfig>,
    ui: Res<UiConfig>,
    fonts: Res<UiFonts>,
    mut spawned: Local<u32>,
) {
    let cfg = &juice.damage_numbers;
    let player_hits = hits.read().filter(|hit| players.contains(hit.shooter));
    for hit in sum_per_shot_and_target(player_hits) {
        *spawned = spawned.wrapping_add(1);
        let (size, color) = if hit.headshot {
            (cfg.crit_size, cfg.crit_color)
        } else {
            (cfg.size, cfg.color)
        };
        commands.spawn((
            DamageNumber {
                point: hit.point,
                value: hit.damage,
                headshot: hit.headshot,
                age: 0.0,
                drift: (*spawned as f32 * GOLDEN_ANGLE).sin(),
            },
            Name::new("Damage number"),
            Text::new(label_text(hit.damage, hit.headshot, &ui.damage_crit)),
            TextFont {
                font: FontSource::Handle(fonts.regular.clone()),
                font_size: FontSize::from(size),
                ..default()
            },
            TextColor(rgb(color)),
            Node {
                position_type: PositionType::Absolute,
                ..default()
            },
            // Percent translation resolves against the node's own size: centres the label on its anchor.
            UiTransform {
                translation: Val2::percent(-50, -50),
                scale: Vec2::splat(cfg.pop_start_scale),
                ..default()
            },
            Visibility::Hidden,
        ));
    }
}

fn age_damage_numbers(
    mut commands: Commands,
    real: Res<Time<Real>>,
    juice: Res<JuiceConfig>,
    mut numbers: Query<(Entity, &mut DamageNumber)>,
) {
    let dt = real.delta_secs();
    for (entity, mut number) in &mut numbers {
        number.age += dt;
        if number.age >= juice.damage_numbers.lifetime {
            commands.entity(entity).despawn();
        }
    }
}

fn place_damage_numbers(
    juice: Res<JuiceConfig>,
    camera: Query<(&Camera, &Transform), With<OrbitCamera>>,
    mut numbers: Query<(
        &DamageNumber,
        &mut Node,
        &mut UiTransform,
        &mut TextColor,
        &mut Visibility,
    )>,
) {
    let Ok((camera, transform)) = camera.single() else {
        return;
    };
    // The camera is a root entity: its Transform is this frame's pose, GlobalTransform lags a frame.
    let global = GlobalTransform::from(*transform);
    for (number, mut node, mut ui_transform, mut color, mut visibility) in &mut numbers {
        let Ok(anchor) = camera.world_to_viewport(&global, number.point) else {
            visibility.set_if_neq(Visibility::Hidden);
            continue;
        };
        let (offset, scale, alpha) = pose(number.age, number.drift, &juice.damage_numbers);
        node.left = px(anchor.x + offset.x);
        node.top = px(anchor.y + offset.y);
        ui_transform.scale = Vec2::splat(scale);
        color.0.set_alpha(alpha);
        visibility.set_if_neq(Visibility::Inherited);
    }
}
