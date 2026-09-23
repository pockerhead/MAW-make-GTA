mod config;

pub use config::{UI_CONFIG, UiConfig};

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

pub struct MenuPlugin;

impl Plugin for MenuPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<UiFonts>()
            .add_systems(OnEnter(GameState::Loading), spawn_loading_screen);
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
