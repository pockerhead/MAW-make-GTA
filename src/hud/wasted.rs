use super::{rgb, rgba};
use crate::menu::{UiConfig, UiFonts};
use bevy::{
    prelude::*,
    render::view::{ColorGrading, ColorGradingGlobal},
};
use gta_sim::flow::WastedPhase;

pub(super) fn spawn_wasted_screen(mut commands: Commands, ui: Res<UiConfig>, fonts: Res<UiFonts>) {
    commands.spawn((
        Name::new("Wasted screen"),
        Node {
            width: percent(100),
            height: percent(100),
            position_type: PositionType::Absolute,
            justify_content: JustifyContent::Center,
            align_items: AlignItems::Center,
            ..default()
        },
        BackgroundColor(rgba(ui.wasted_backdrop)),
        DespawnOnExit(WastedPhase::Screen),
        children![(
            Text::new(ui.wasted.clone()),
            TextFont {
                font: FontSource::Handle(fonts.title.clone()),
                font_size: FontSize::from(ui.wasted_size),
                ..default()
            },
            TextColor(rgb(ui.wasted_color)),
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
