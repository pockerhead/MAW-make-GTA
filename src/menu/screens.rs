//! Main menu, pause menu and settings screen (GDD §7) and the systems that drive them.

use super::widgets::{
    MenuAction, SeedField, SettingKey, button, clock_seed, label, rgba, seed_field,
    seed_from_field, title_screen,
};
use super::menu_config::MenuConfig;
use super::{PauseMenu, UiConfig, UiFonts};
use crate::settings::{GameSettings, step_value};
use bevy::{
    prelude::*,
    settings::{SaveSettings, SaveSettingsDeferred},
    text::EditableText,
};
use gta_sim::{
    flow::{GameState, pause_request},
    world::CitySeed,
};

/// The value text of one row of the settings screen.
#[derive(Component)]
pub(super) struct SettingValue(SettingKey);

/// Row of the seed label and the seed field, then the hint under it.
fn seed_rows(parent: &mut ChildSpawnerCommands, ui: &UiConfig, fonts: &UiFonts) {
    parent
        .spawn(Node {
            column_gap: px(ui.menu.item_gap),
            align_items: AlignItems::Center,
            ..default()
        })
        .with_children(|row| {
            row.spawn(label(ui.menu.seed_label.clone(), ui, fonts));
            row.spawn(seed_field(ui, fonts));
        });
    parent.spawn(label(ui.menu.seed_hint.clone(), ui, fonts));
}

pub(super) fn spawn_main_menu(mut commands: Commands, ui: Res<UiConfig>, fonts: Res<UiFonts>) {
    let menu = &ui.menu;
    let title = (menu.title.clone(), menu.title_size, menu.text_color);
    let root = title_screen(
        &mut commands,
        &ui,
        &fonts,
        "Main menu",
        title,
        rgba(menu.menu_backdrop),
        GameState::MainMenu,
    );
    let width = menu.button_width;
    commands.entity(root).with_children(|parent| {
        parent.spawn(button(
            menu.new_game.clone(),
            MenuAction::NewGame,
            width,
            &ui,
            &fonts,
        ));
        seed_rows(parent, &ui, &fonts);
        parent.spawn(button(
            menu.quit.clone(),
            MenuAction::Quit,
            width,
            &ui,
            &fonts,
        ));
    });
}

pub(super) fn spawn_pause_menu(
    mut commands: Commands,
    ui: Res<UiConfig>,
    fonts: Res<UiFonts>,
    seed: Res<CitySeed>,
) {
    let menu = &ui.menu;
    let title = (menu.paused.clone(), menu.title_size, menu.text_color);
    let root = title_screen(
        &mut commands,
        &ui,
        &fonts,
        "Pause menu",
        title,
        rgba(menu.backdrop),
        PauseMenu::Main,
    );
    let width = menu.button_width;
    let seed_line = menu.current_seed.replace("{seed}", &seed.0.to_string());
    commands.entity(root).with_children(|parent| {
        parent.spawn(label(seed_line, &ui, &fonts));
        for (text, action) in [
            (&menu.resume, MenuAction::Resume),
            (&menu.new_city, MenuAction::NewCity),
            (&menu.settings, MenuAction::OpenSettings),
            (&menu.quit, MenuAction::Quit),
        ] {
            parent.spawn(button(text.clone(), action, width, &ui, &fonts));
        }
        seed_rows(parent, &ui, &fonts);
    });
}

fn setting_text(key: SettingKey, settings: &GameSettings, ui: &UiConfig) -> String {
    let flag = |on: bool| if on { &ui.menu.on } else { &ui.menu.off }.clone();
    match key {
        SettingKey::Sensitivity => ui
            .menu
            .sensitivity_value
            .replace("{value}", &format!("{:.2}", settings.mouse_sensitivity)),
        SettingKey::Volume => ui
            .menu
            .volume_value
            .replace("{value}", &format!("{:.0}", settings.volume * 100.0)),
        SettingKey::InvertY => flag(settings.invert_y),
        SettingKey::ReduceShake => flag(settings.reduce_shake),
        SettingKey::NoFlashes => flag(settings.no_flashes),
    }
}

// Values are spawned current: `resource_changed` does not fire when the screen opens.
pub(super) fn spawn_settings_screen(
    mut commands: Commands,
    ui: Res<UiConfig>,
    fonts: Res<UiFonts>,
    settings: Res<GameSettings>,
) {
    let menu = &ui.menu;
    let title = (menu.settings.clone(), menu.title_size, menu.text_color);
    let root = title_screen(
        &mut commands,
        &ui,
        &fonts,
        "Settings",
        title,
        rgba(menu.backdrop),
        PauseMenu::Settings,
    );
    let small = menu.button_height;
    commands.entity(root).with_children(|parent| {
        for (key, name, numeric) in [
            (SettingKey::Sensitivity, &menu.sensitivity, true),
            (SettingKey::Volume, &menu.volume, true),
            (SettingKey::InvertY, &menu.invert_y, false),
            (SettingKey::ReduceShake, &menu.reduce_shake, false),
            (SettingKey::NoFlashes, &menu.no_flashes, false),
        ] {
            parent
                .spawn(Node {
                    width: px(2.0 * menu.button_width),
                    column_gap: px(menu.item_gap),
                    align_items: AlignItems::Center,
                    ..default()
                })
                .with_children(|row| {
                    row.spawn((
                        label(name.clone(), &ui, &fonts),
                        Node {
                            flex_grow: 1.0,
                            ..default()
                        },
                    ));
                    row.spawn((
                        SettingValue(key),
                        label(setting_text(key, &settings, &ui), &ui, &fonts),
                    ));
                    if numeric {
                        row.spawn(button(
                            menu.step_down.clone(),
                            MenuAction::Step(key, -1),
                            small,
                            &ui,
                            &fonts,
                        ));
                        row.spawn(button(
                            menu.step_up.clone(),
                            MenuAction::Step(key, 1),
                            small,
                            &ui,
                            &fonts,
                        ));
                    } else {
                        row.spawn(button(
                            menu.toggle.clone(),
                            MenuAction::Toggle(key),
                            small,
                            &ui,
                            &fonts,
                        ));
                    }
                });
        }
        parent.spawn(button(
            menu.back.clone(),
            MenuAction::CloseSettings,
            menu.button_width,
            &ui,
            &fonts,
        ));
    });
}

pub(super) fn update_setting_labels(
    ui: Res<UiConfig>,
    settings: Res<GameSettings>,
    mut values: Query<(&SettingValue, &mut Text)>,
) {
    for (value, mut text) in &mut values {
        text.0 = setting_text(value.0, &settings, &ui);
    }
}

/// Esc: back from the settings screen, otherwise pause or resume by the sim rule (which refuses
/// while a death or arrest is pending, and in `Loading`, `MainMenu`, `Wasted`, `Busted`).
pub(super) fn escape(
    keys: Res<ButtonInput<KeyCode>>,
    state: Res<State<GameState>>,
    pause_menu: Option<Res<State<PauseMenu>>>,
    mut next: ResMut<NextState<GameState>>,
    mut next_menu: ResMut<NextState<PauseMenu>>,
) {
    if !keys.just_pressed(KeyCode::Escape) {
        return;
    }
    if pause_menu.is_some_and(|menu| *menu.get() == PauseMenu::Settings) {
        next_menu.set(PauseMenu::Main);
        return;
    }
    if let Some(target) = pause_request(state.get(), &next) {
        next.set(target);
    }
}

/// Only from `MainMenu` or `Paused`: no fixed-tick transition can be pending there.
fn start_city(seed: u64, city_seed: &mut CitySeed, next: &mut NextState<GameState>) {
    city_seed.0 = seed;
    next.set(GameState::Loading);
}

fn typed_seed(field: &Query<&EditableText, With<SeedField>>) -> u64 {
    let text = field.iter().next().map(|f| f.value().to_string());
    seed_from_field(text.as_deref().unwrap_or(""), clock_seed)
}

pub(super) fn submit_seed(
    keys: Res<ButtonInput<KeyCode>>,
    field: Query<&EditableText, With<SeedField>>,
    mut city_seed: ResMut<CitySeed>,
    mut next: ResMut<NextState<GameState>>,
) {
    if !keys.any_just_pressed([KeyCode::Enter, KeyCode::NumpadEnter]) {
        return;
    }
    start_city(typed_seed(&field), &mut city_seed, &mut next);
}

#[allow(clippy::too_many_arguments)]
pub(super) fn press_buttons(
    mut commands: Commands,
    ui: Res<UiConfig>,
    buttons: Query<(&Interaction, &MenuAction), Changed<Interaction>>,
    field: Query<&EditableText, With<SeedField>>,
    state: Res<State<GameState>>,
    mut city_seed: ResMut<CitySeed>,
    mut next: ResMut<NextState<GameState>>,
    mut next_menu: ResMut<NextState<PauseMenu>>,
    mut settings: ResMut<GameSettings>,
    mut exit: MessageWriter<AppExit>,
) {
    let menu = &ui.menu;
    for (interaction, action) in &buttons {
        if *interaction != Interaction::Pressed {
            continue;
        }
        match *action {
            MenuAction::NewGame => start_city(clock_seed(), &mut city_seed, &mut next),
            MenuAction::NewCity => start_city(typed_seed(&field), &mut city_seed, &mut next),
            MenuAction::Resume => {
                if let Some(target) = pause_request(state.get(), &next) {
                    next.set(target);
                }
            }
            MenuAction::OpenSettings => next_menu.set(PauseMenu::Settings),
            MenuAction::CloseSettings => next_menu.set(PauseMenu::Main),
            MenuAction::Quit => {
                exit.write(AppExit::Success);
            }
            MenuAction::Step(key, steps) => {
                if apply_step(&mut settings, key, steps, menu) {
                    commands.queue(SaveSettingsDeferred::default());
                }
            }
            MenuAction::Toggle(key) => {
                let s = &mut *settings;
                let flag = match key {
                    SettingKey::InvertY => &mut s.invert_y,
                    SettingKey::ReduceShake => &mut s.reduce_shake,
                    SettingKey::NoFlashes => &mut s.no_flashes,
                    _ => continue,
                };
                *flag = !*flag;
                commands.queue(SaveSettingsDeferred::default());
            }
        }
    }
}

#[allow(clippy::type_complexity)]
pub(super) fn hover_buttons(
    ui: Res<UiConfig>,
    mut buttons: Query<
        (&Interaction, &mut BackgroundColor),
        (Changed<Interaction>, With<MenuAction>),
    >,
) {
    for (interaction, mut color) in &mut buttons {
        let c = match interaction {
            Interaction::None => ui.menu.button_color,
            _ => ui.menu.button_hover_color,
        };
        color.0 = rgba(c);
    }
}

// The deferred save timer ticks on virtual time, frozen while paused.
pub(super) fn save_on_close(mut commands: Commands) {
    commands.queue(SaveSettings::IfChanged);
}

/// A step clamped at a range bound leaves `GameSettings` unmarked, so nothing is rewritten.
pub(super) fn apply_step(
    settings: &mut impl DetectChangesMut<Inner = GameSettings>,
    key: SettingKey,
    steps: i32,
    menu: &MenuConfig,
) -> bool {
    let mut updated = settings.bypass_change_detection().clone();
    match key {
        SettingKey::Sensitivity => {
            let (min, max, step) = menu.sensitivity_range;
            updated.mouse_sensitivity = step_value(updated.mouse_sensitivity, steps, min, max, step);
        }
        SettingKey::Volume => {
            updated.volume = step_value(updated.volume, steps, 0.0, 1.0, menu.volume_step);
        }
        _ => return false,
    }
    settings.set_if_neq(updated)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::menu::UI_CONFIG;
    use bevy::ecs::system::RunSystemOnce;
    use gta_sim::config::{ConfigRoot, load_config};
    use std::path::Path;

    fn menu() -> MenuConfig {
        let root = ConfigRoot(Path::new(env!("CARGO_MANIFEST_DIR")).join("assets"));
        load_config::<UiConfig>(&root, UI_CONFIG)
            .unwrap_or_else(|e| panic!("GATE BROKEN: {e}"))
            .menu
    }

    /// Presses one step on settings `start`; returns (reported change, change mark, value after).
    fn press(start: GameSettings, key: SettingKey, steps: i32) -> (bool, bool, GameSettings) {
        let menu = menu();
        let mut world = World::new();
        world.insert_resource(start);
        world.clear_trackers();
        let changed = world
            .run_system_once(move |mut s: ResMut<GameSettings>| apply_step(&mut s, key, steps, &menu))
            .unwrap_or_else(|e| panic!("GATE BROKEN: {e}"));
        let marked = world.is_resource_changed::<GameSettings>();
        (changed, marked, world.resource::<GameSettings>().clone())
    }

    #[test]
    fn step_at_a_bound_leaves_settings_unmarked() {
        let (min, max, _) = menu().sensitivity_range;
        let at = |sensitivity: f32, volume: f32| GameSettings {
            mouse_sensitivity: sensitivity,
            volume,
            ..default()
        };
        let rows = [
            ("sensitivity + at max", at(max, 0.5), SettingKey::Sensitivity, 1),
            ("sensitivity - at min", at(min, 0.5), SettingKey::Sensitivity, -1),
            ("volume + at 1", at(1.0, 1.0), SettingKey::Volume, 1),
            ("volume - at 0", at(1.0, 0.0), SettingKey::Volume, -1),
        ];
        for (row, start, key, steps) in rows {
            let (changed, marked, after) = press(start.clone(), key, steps);
            assert_eq!((changed, marked, after), (false, false, start), "{row}");
        }
        let (changed, marked, after) = press(at(1.0, 0.5), SettingKey::Volume, 1);
        assert!(changed && marked && after.volume > 0.5, "in-range step must change and mark");
    }
}
