mod config;
mod menu_config;
mod screens;
mod widgets;

pub(crate) use config::{Rgb, Rgba, positive, unit};
pub use config::{StarsConfig, UI_CONFIG, UiConfig};
pub use widgets::clock_seed;
pub(crate) use widgets::title_screen;

use bevy::prelude::*;
use gta_sim::{flow::GameState, world::CitySeed};

/// Fonts of `UiConfig`; loaded while the plugin builds because the loading screen spawns on the
/// first `StateTransition`, before `Startup`.
#[derive(Resource)]
pub struct UiFonts {
    pub regular: Handle<Font>,
    pub title: Handle<Font>,
}

impl FromWorld for UiFonts {
    fn from_world(world: &mut World) -> Self {
        let ui = world.resource::<UiConfig>();
        let (regular, title) = (ui.font.clone(), ui.title_font.clone());
        let assets = world.resource::<AssetServer>();
        Self {
            regular: assets.load(regular),
            title: assets.load(title),
        }
    }
}

/// Screen of `GameState::Paused`; read by QA over BRP.
#[derive(SubStates, Default, Clone, PartialEq, Eq, Hash, Debug, Reflect)]
#[source(GameState = GameState::Paused)]
pub enum PauseMenu {
    #[default]
    Main,
    Settings,
}

pub struct MenuPlugin;

impl Plugin for MenuPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<UiFonts>()
            .add_sub_state::<PauseMenu>()
            .register_type_state::<PauseMenu>()
            .add_systems(OnEnter(GameState::Loading), spawn_loading_screen)
            .add_systems(OnEnter(GameState::MainMenu), screens::spawn_main_menu)
            .add_systems(OnEnter(PauseMenu::Main), screens::spawn_pause_menu)
            .add_systems(OnEnter(PauseMenu::Settings), screens::spawn_settings_screen)
            .add_systems(OnExit(PauseMenu::Settings), screens::save_on_close)
            .add_systems(
                Update,
                (
                    screens::escape,
                    screens::submit_seed
                        .run_if(in_state(GameState::MainMenu).or_else(in_state(PauseMenu::Main))),
                    screens::press_buttons,
                    screens::hover_buttons,
                    screens::update_setting_labels
                        .run_if(resource_changed::<crate::settings::GameSettings>),
                ),
            );
    }
}

fn spawn_loading_screen(
    mut commands: Commands,
    seed: Res<CitySeed>,
    ui: Res<UiConfig>,
    fonts: Res<UiFonts>,
) {
    commands
        .spawn((
            Node {
                width: percent(100),
                height: percent(100),
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                ..default()
            },
            BackgroundColor(Color::BLACK),
            DespawnOnExit(GameState::Loading),
        ))
        .with_child((
            Text::new(ui.loading.replace("{seed}", &seed.0.to_string())),
            TextFont {
                font: FontSource::Handle(fonts.regular.clone()),
                font_size: FontSize::from(ui.loading_size),
                ..default()
            },
        ));
}
