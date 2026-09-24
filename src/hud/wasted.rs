use super::{rgb, rgba};
use crate::menu::{UiConfig, UiFonts};
use bevy::{
    prelude::*,
    render::view::{ColorGrading, ColorGradingGlobal},
};
use gta_sim::flow::{BustedPhase, WastedPhase};

pub(super) fn spawn_wasted_screen(commands: Commands, ui: Res<UiConfig>, fonts: Res<UiFonts>) {
    let (text, color) = (ui.wasted.clone(), ui.wasted_color);
    title_screen(
        commands,
        &ui,
        &fonts,
        "Wasted screen",
        text,
        color,
        WastedPhase::Screen,
    );
}

pub(super) fn spawn_busted_screen(commands: Commands, ui: Res<UiConfig>, fonts: Res<UiFonts>) {
    let (text, color) = (ui.busted.clone(), ui.busted_color);
    title_screen(
        commands,
        &ui,
        &fonts,
        "Busted screen",
        text,
        color,
        BustedPhase::Screen,
    );
}

/// A full-screen title over the wasted backdrop, gone when `exit` is left.
fn title_screen(
    mut commands: Commands,
    ui: &UiConfig,
    fonts: &UiFonts,
    name: &'static str,
    text: String,
    color: (f32, f32, f32),
    exit: impl States,
) {
    commands.spawn((
        Name::new(name),
        Node {
            width: percent(100),
            height: percent(100),
            position_type: PositionType::Absolute,
            justify_content: JustifyContent::Center,
            align_items: AlignItems::Center,
            ..default()
        },
        BackgroundColor(rgba(ui.wasted_backdrop)),
        DespawnOnExit(exit),
        children![(
            Text::new(text),
            TextFont {
                font: FontSource::Handle(fonts.title.clone()),
                font_size: FontSize::from(ui.wasted_size),
                ..default()
            },
            TextColor(rgb(color)),
        )],
    ));
}

pub(super) fn desaturate(ui: Res<UiConfig>, mut cameras: Query<&mut ColorGrading, With<Camera3d>>) {
    for mut grading in &mut cameras {
        grading.global.post_saturation = ui.wasted_saturation;
    }
}

// The camera is created with the default grading and nothing else changes it.
pub(super) fn restore_saturation(mut cameras: Query<&mut ColorGrading, With<Camera3d>>) {
    for mut grading in &mut cameras {
        grading.global.post_saturation = ColorGradingGlobal::default().post_saturation;
    }
}
