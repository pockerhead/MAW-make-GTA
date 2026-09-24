use super::rgba;
use crate::menu::{UiConfig, UiFonts, title_screen};
use bevy::{
    prelude::*,
    render::view::{ColorGrading, ColorGradingGlobal},
};
use gta_sim::flow::{BustedPhase, WastedPhase};

pub(super) fn spawn_wasted_screen(mut commands: Commands, ui: Res<UiConfig>, fonts: Res<UiFonts>) {
    let title = (ui.wasted.clone(), ui.wasted_size, ui.wasted_color);
    title_screen(
        &mut commands,
        &ui,
        &fonts,
        "Wasted screen",
        title,
        rgba(ui.wasted_backdrop),
        WastedPhase::Screen,
    );
}

pub(super) fn spawn_busted_screen(mut commands: Commands, ui: Res<UiConfig>, fonts: Res<UiFonts>) {
    let title = (ui.busted.clone(), ui.wasted_size, ui.busted_color);
    title_screen(
        &mut commands,
        &ui,
        &fonts,
        "Busted screen",
        title,
        rgba(ui.wasted_backdrop),
        BustedPhase::Screen,
    );
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
