use bevy::prelude::*;
use gta_sim::{flow::GameState, world::CitySeed};

pub struct MenuPlugin;

impl Plugin for MenuPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(OnEnter(GameState::Loading), spawn_loading_screen);
    }
}

// Latin text until the Cyrillic font and assets/ui/strings.ron arrive (GDD T5).
fn spawn_loading_screen(mut commands: Commands, seed: Res<CitySeed>) {
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
        .with_child(Text::new(format!("Generating city (seed {})", seed.0)));
}
