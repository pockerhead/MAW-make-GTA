use crate::camera::{OrbitCamera, apply_mouse_look};
use bevy::{
    prelude::*,
    window::{CursorGrabMode, CursorOptions, PrimaryWindow},
};
use bevy_enhanced_input::prelude::*;
use gta_sim::{
    character::{Gait, MoveIntent},
    player::Player,
};

#[derive(Component)]
struct OnFoot;

#[derive(InputAction)]
#[action_output(Vec2)]
struct Move;

#[derive(InputAction)]
#[action_output(Vec2)]
pub struct Look;

#[derive(InputAction)]
#[action_output(bool)]
struct Sprint;

#[derive(InputAction)]
#[action_output(bool)]
struct Walk;

#[derive(InputAction)]
#[action_output(bool)]
struct Jump;

#[derive(Resource)]
pub struct CursorCaptured(pub bool);

pub struct PlayerInputPlugin;

impl Plugin for PlayerInputPlugin {
    fn build(&self, app: &mut App) {
        app.add_input_context::<OnFoot>()
            .insert_resource(CursorCaptured(true))
            .add_systems(Startup, (spawn_input, capture_cursor))
            .add_systems(
                Update,
                (cursor_toggle, write_move_intent.after(apply_mouse_look)),
            );
    }
}

fn spawn_input(mut commands: Commands) {
    commands.spawn((
        Name::new("PlayerInput"),
        OnFoot,
        actions!(OnFoot[
            (Action::<Move>::new(), Bindings::spawn(Cardinal::wasd_keys())),
            (Action::<Look>::new(), bindings![Binding::mouse_motion()]),
            (Action::<Sprint>::new(), bindings![KeyCode::ShiftLeft]),
            (Action::<Walk>::new(), bindings![KeyCode::AltLeft]),
            (Action::<Jump>::new(), bindings![KeyCode::Space]),
        ]),
    ));
}

fn capture_cursor(mut cursor: Single<&mut CursorOptions, With<PrimaryWindow>>) {
    cursor.grab_mode = CursorGrabMode::Locked;
    cursor.visible = false;
}

fn cursor_toggle(
    keys: Res<ButtonInput<KeyCode>>,
    buttons: Res<ButtonInput<MouseButton>>,
    mut captured: ResMut<CursorCaptured>,
    mut cursor: Single<&mut CursorOptions, With<PrimaryWindow>>,
) {
    if keys.just_pressed(KeyCode::Escape) {
        captured.0 = false;
        cursor.grab_mode = CursorGrabMode::None;
        cursor.visible = true;
    } else if !captured.0 && buttons.just_pressed(MouseButton::Left) {
        captured.0 = true;
        cursor.grab_mode = CursorGrabMode::Locked;
        cursor.visible = false;
    }
}

fn write_move_intent(
    movement: Single<&Action<Move>>,
    sprint: Single<&Action<Sprint>>,
    walk: Single<&Action<Walk>>,
    jump: Single<(&Action<Jump>, &ActionEvents)>,
    camera: Single<&OrbitCamera>,
    mut intent: Single<&mut MoveIntent, With<Player>>,
) {
    intent.axis = ***movement;
    intent.yaw = camera.yaw;
    intent.gait = if ***walk {
        Gait::Walk
    } else if ***sprint {
        Gait::Sprint
    } else {
        Gait::Run
    };
    intent.jump_held = **jump.0;
    if jump.1.contains(ActionEvents::START) {
        intent.jump_requested = true;
    }
}
