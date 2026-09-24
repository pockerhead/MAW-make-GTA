//! Shared full-screen shell, buttons and the seed field of the menus.

use super::{UiConfig, UiFonts};
use bevy::{
    input_focus::AutoFocus,
    prelude::*,
    text::{EditableText, EditableTextFilter, TextCursorStyle},
};
use std::time::{SystemTime, UNIX_EPOCH};

/// A setting the settings screen changes.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum SettingKey {
    Sensitivity,
    Volume,
    InvertY,
    ReduceShake,
    NoFlashes,
    ReduceCameraMotion,
}

/// What a menu button does when pressed.
#[derive(Component, Clone, Copy, PartialEq, Debug)]
pub enum MenuAction {
    NewGame,
    Resume,
    NewCity,
    OpenSettings,
    CloseSettings,
    Quit,
    Step(SettingKey, i32),
    Toggle(SettingKey),
}

/// The seed text field of the main and pause menus.
#[derive(Component)]
pub struct SeedField;

fn rgb((r, g, b): (f32, f32, f32)) -> Color {
    Color::srgb(r, g, b)
}

pub(crate) fn rgba((r, g, b, a): (f32, f32, f32, f32)) -> Color {
    Color::srgba(r, g, b, a)
}

/// A full-screen column with a title over `backdrop`, gone when `exit` is left; menus add children.
pub(crate) fn title_screen(
    commands: &mut Commands,
    ui: &UiConfig,
    fonts: &UiFonts,
    name: &'static str,
    (text, size, color): (String, f32, (f32, f32, f32)),
    backdrop: Color,
    exit: impl States,
) -> Entity {
    commands
        .spawn((
            Name::new(name),
            Node {
                width: percent(100),
                height: percent(100),
                position_type: PositionType::Absolute,
                flex_direction: FlexDirection::Column,
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                row_gap: px(ui.menu.item_gap),
                ..default()
            },
            BackgroundColor(backdrop),
            DespawnOnExit(exit),
            children![(
                Text::new(text),
                TextFont {
                    font: FontSource::Handle(fonts.title.clone()),
                    font_size: FontSize::from(size),
                    ..default()
                },
                TextColor(rgb(color)),
            )],
        ))
        .id()
}

/// A line of menu text in the body font.
pub(crate) fn label(text: String, ui: &UiConfig, fonts: &UiFonts) -> impl Bundle {
    (
        Text::new(text),
        TextFont {
            font: FontSource::Handle(fonts.regular.clone()),
            font_size: FontSize::from(ui.menu.item_size),
            ..default()
        },
        TextColor(rgb(ui.menu.text_color)),
    )
}

pub(crate) fn button(
    text: String,
    action: MenuAction,
    width: f32,
    ui: &UiConfig,
    fonts: &UiFonts,
) -> impl Bundle {
    let menu = &ui.menu;
    (
        Button,
        action,
        Node {
            width: px(width),
            height: px(menu.button_height),
            justify_content: JustifyContent::Center,
            align_items: AlignItems::Center,
            ..default()
        },
        BackgroundColor(rgba(menu.button_color)),
        children![label(text, ui, fonts)],
    )
}

/// Digits only; 19 digits always fit a `u64` (law).
pub(crate) fn seed_field(ui: &UiConfig, fonts: &UiFonts) -> impl Bundle {
    let menu = &ui.menu;
    (
        SeedField,
        Node {
            width: px(menu.button_width),
            padding: px(6.0).all(),
            ..default()
        },
        EditableText {
            max_characters: Some(19),
            ..default()
        },
        EditableTextFilter::new(|c| c.is_ascii_digit()),
        TextCursorStyle::default(),
        AutoFocus,
        TextFont {
            font: FontSource::Handle(fonts.regular.clone()),
            font_size: FontSize::from(menu.item_size),
            ..default()
        },
        TextColor(rgb(menu.text_color)),
        BackgroundColor(rgba(menu.field_color)),
    )
}

/// The typed seed, or `fallback()` for an empty or unparsable field.
pub fn seed_from_field(text: &str, fallback: impl FnOnce() -> u64) -> u64 {
    text.trim().parse().unwrap_or_else(|_| fallback())
}

/// A seed from the clock (nanoseconds since the epoch).
pub fn clock_seed() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.as_nanos() as u64)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn seed_from_field_table() {
        assert_eq!(seed_from_field("", || 77), 77, "empty");
        assert_eq!(seed_from_field("0", || 77), 0);
        assert_eq!(seed_from_field("42", || 77), 42);
        assert_eq!(
            seed_from_field("9999999999999999999", || 77),
            9_999_999_999_999_999_999
        );
    }
}
