mod stars;
mod wasted;
mod weapon;
mod witness;
#[cfg(test)]
mod witness_gate;

use crate::{juice::StarsRaised, menu::UiConfig};
use bevy::prelude::*;
use gta_sim::{
    character::{Health, HealthConfig},
    flow::{BustedPhase, GameState, WastedPhase},
    player::Player,
    vehicle::{DamageConfig, Driving, VehicleHealth},
    world::CityScoped,
};

/// Health and armour bars (top right), ammo, wanted stars, crosshair, hit marker, witness bars and the
/// "ПОТРАЧЕНО" and "BUSTED" screens.
pub struct HudPlugin;

impl Plugin for HudPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(witness::WitnessBarPlugin)
            .add_message::<StarsRaised>()
            .register_type::<stars::StarPulse>()
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
                    stars::pulse_stars,
                ),
            )
            .add_systems(OnEnter(WastedPhase::Screen), wasted::spawn_wasted_screen)
            .add_systems(OnEnter(GameState::Wasted), wasted::desaturate)
            .add_systems(OnExit(GameState::Wasted), wasted::restore_saturation)
            .add_systems(OnEnter(BustedPhase::Screen), wasted::spawn_busted_screen)
            .add_systems(OnEnter(GameState::Busted), wasted::desaturate)
            .add_systems(OnExit(GameState::Busted), wasted::restore_saturation);
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum HudBar {
    Health,
    Armor,
    /// The car the player drives; hidden on foot.
    Vehicle,
}

/// The coloured fill of one bar; its width is the percentage of the maximum.
#[derive(Component)]
struct HudFill(HudBar);

/// The background of one bar (the whole bar row).
#[derive(Component)]
struct HudBarRoot(HudBar);

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
            HudBarRoot(kind),
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
        CityScoped,
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
            bar(HudBar::Vehicle, rgb(hud.vehicle_color)),
        ],
    ));
}

// Not `Changed<Health>`: `since_damage` changes every fixed tick, so the filter would always pass.
#[allow(clippy::type_complexity)]
fn update_bars(
    cfg: Res<HealthConfig>,
    damage: Res<DamageConfig>,
    player: Query<(&Health, Option<&Driving>), With<Player>>,
    cars: Query<&VehicleHealth>,
    mut fills: Query<(&HudFill, &mut Node), Without<HudBarRoot>>,
    mut roots: Query<(&HudBarRoot, &mut Node), Without<HudFill>>,
) {
    let Ok((health, driving)) = player.single() else {
        return;
    };
    let car = driving.and_then(|d| cars.get(d.vehicle).ok());
    for (root, mut node) in &mut roots {
        if root.0 != HudBar::Vehicle {
            continue;
        }
        let display = if car.is_some() {
            Display::Flex
        } else {
            Display::None
        };
        if node.display != display {
            node.display = display;
        }
    }
    for (fill, mut node) in &mut fills {
        let (value, max) = match fill.0 {
            HudBar::Health => (health.current, cfg.max_health),
            HudBar::Armor => (health.armor, cfg.max_armor),
            HudBar::Vehicle => (car.map_or(0.0, |c| c.current), damage.vehicle.max_health),
        };
        let width = percent(100.0 * value / max);
        if node.width != width {
            node.width = width;
        }
    }
}
