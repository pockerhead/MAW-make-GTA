mod stars;
mod wasted;
mod weapon;
mod witness;
#[cfg(test)]
mod witness_gate;

use crate::menu::UiConfig;
use bevy::prelude::*;
use gta_sim::{
    character::{Health, HealthConfig},
    flow::{GameState, WastedPhase},
    player::Player,
};

/// Health and armour bars (top right), ammo, wanted stars, crosshair, hit marker, witness bars and the
/// "ПОТРАЧЕНО" screen.
pub struct HudPlugin;

impl Plugin for HudPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(witness::WitnessBarPlugin)
            .add_systems(
                OnTransition {
                    exited: GameState::Loading,
                    entered: GameState::Playing,
                },
                (spawn_hud, weapon::spawn_weapon_hud, stars::spawn_stars),
            )
            .add_systems(
                Update,
                (
                    update_bars,
                    weapon::update_ammo,
                    weapon::update_crosshair,
                    weapon::update_hit_marker,
                    stars::update_stars,
                ),
            )
            .add_systems(OnEnter(WastedPhase::Screen), wasted::spawn_wasted_screen)
            .add_systems(OnEnter(GameState::Wasted), wasted::desaturate)
            .add_systems(OnExit(GameState::Wasted), wasted::restore_saturation);
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum HudBar {
    Health,
    Armor,
}

/// The coloured fill of one bar; its width is the percentage of the maximum.
#[derive(Component)]
struct HudFill(HudBar);

fn rgb((r, g, b): (f32, f32, f32)) -> Color {
    Color::srgb(r, g, b)
}

fn rgba((r, g, b, a): (f32, f32, f32, f32)) -> Color {
    Color::srgba(r, g, b, a)
}

fn spawn_hud(mut commands: Commands, ui: Res<UiConfig>) {
    let hud = &ui.hud;
    let bar = |kind: HudBar, color: Color| {
        (
            Node {
                width: px(hud.bar_width),
                height: px(hud.bar_height),
                ..default()
            },
            BackgroundColor(rgba(hud.back_color)),
            children![(
                HudFill(kind),
                Node {
                    width: percent(100),
                    height: percent(100),
                    ..default()
                },
                BackgroundColor(color),
            )],
        )
    };
    commands.spawn((
        Name::new("Hud"),
        Node {
            position_type: PositionType::Absolute,
            top: px(hud.margin),
            right: px(hud.margin),
            flex_direction: FlexDirection::Column,
            row_gap: px(hud.bar_gap),
            ..default()
        },
        children![
            bar(HudBar::Health, rgb(hud.health_color)),
            bar(HudBar::Armor, rgb(hud.armor_color)),
        ],
    ));
}

// Not `Changed<Health>`: `since_damage` changes every fixed tick, so the filter would always pass.
fn update_bars(
    cfg: Res<HealthConfig>,
    player: Query<&Health, With<Player>>,
    mut fills: Query<(&HudFill, &mut Node)>,
) {
    let Ok(health) = player.single() else {
        return;
    };
    for (fill, mut node) in &mut fills {
        let (value, max) = match fill.0 {
            HudBar::Health => (health.current, cfg.max_health),
            HudBar::Armor => (health.armor, cfg.max_armor),
        };
        let width = percent(100.0 * value / max);
        if node.width != width {
            node.width = width;
        }
    }
}
