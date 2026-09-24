use crate::camera::{OrbitCamera, apply_mouse_look};
use bevy::{
    prelude::*,
    window::{CursorGrabMode, CursorOptions, PrimaryWindow},
};
use bevy_enhanced_input::prelude::*;
use gta_sim::{
    character::{ActionIntent, AimIntent, Gait, MoveIntent, WeaponRequest},
    combat::Weapon,
    flow::GameState,
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

#[derive(InputAction)]
#[action_output(bool)]
struct Fire;

#[derive(InputAction)]
#[action_output(bool)]
struct Aim;

#[derive(InputAction)]
#[action_output(bool)]
struct Reload;

#[derive(InputAction)]
#[action_output(bool)]
struct Slot1;

#[derive(InputAction)]
#[action_output(bool)]
struct Slot2;

#[derive(InputAction)]
#[action_output(bool)]
struct Slot3;

#[derive(InputAction)]
#[action_output(bool)]
struct Slot4;

#[derive(InputAction)]
#[action_output(f32)]
struct CycleWeapon;

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
                (
                    cursor_toggle,
                    write_move_intent.after(apply_mouse_look),
                    // Before `cursor_toggle`: the click that recaptures the cursor must not fire.
                    write_action_intent
                        .after(apply_mouse_look)
                        .before(cursor_toggle),
                ),
            )
            .add_systems(OnEnter(GameState::Wasted), release_held_actions)
            .add_systems(OnEnter(GameState::Busted), release_held_actions);
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
            (Action::<Fire>::new(), bindings![MouseButton::Left]),
            (Action::<Aim>::new(), bindings![MouseButton::Right]),
            (Action::<Reload>::new(), bindings![KeyCode::KeyR]),
            (Action::<Slot1>::new(), bindings![KeyCode::Digit1]),
            (Action::<Slot2>::new(), bindings![KeyCode::Digit2]),
            (Action::<Slot3>::new(), bindings![KeyCode::Digit3]),
            (Action::<Slot4>::new(), bindings![KeyCode::Digit4]),
            (
                Action::<CycleWeapon>::new(),
                bindings![(Binding::mouse_wheel(), SwizzleAxis::YXZ)],
            ),
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

/// Events of the one action `A` (reload, weapon slots).
type EventsOf<'w, 's, A> = Single<'w, 's, &'static ActionEvents, With<Action<A>>>;

#[allow(clippy::too_many_arguments)]
fn write_action_intent(
    captured: Res<CursorCaptured>,
    fire: Single<(&Action<Fire>, &ActionEvents)>,
    aim: Single<&Action<Aim>>,
    reload: EventsOf<Reload>,
    slots: (
        EventsOf<Slot1>,
        EventsOf<Slot2>,
        EventsOf<Slot3>,
        EventsOf<Slot4>,
    ),
    wheel: Single<&Action<CycleWeapon>>,
    mut wait_release: Local<bool>,
    player: Single<(&mut ActionIntent, &mut AimIntent), With<Player>>,
) {
    let (mut action, mut aim_intent) = player.into_inner();
    if !captured.0 {
        action.fire_held = false;
        aim_intent.aiming = false;
        // The click that recaptures the cursor stays held: it must be released before it fires.
        *wait_release = true;
        return;
    }
    let (fire_held, fire_events) = (**fire.0, *fire.1);
    if *wait_release && !fire_held {
        *wait_release = false;
    }
    let armed = !*wait_release;
    action.fire_held = armed && fire_held;
    if armed && fire_events.contains(ActionEvents::START) {
        action.fire_requested = true;
    }
    aim_intent.aiming = ***aim;
    if reload.contains(ActionEvents::START) {
        action.reload_requested = true;
    }
    // Number keys: 1 = unarmed, 2-4 = pistol, SMG, shotgun.
    let requests = [
        (&*slots.0, WeaponRequest::Unarmed),
        (&*slots.1, WeaponRequest::Gun(Weapon::Pistol)),
        (&*slots.2, WeaponRequest::Gun(Weapon::Smg)),
        (&*slots.3, WeaponRequest::Gun(Weapon::Shotgun)),
    ];
    for (events, request) in requests {
        if events.contains(ActionEvents::START) {
            action.select = Some(request);
        }
    }
    let wheel = ***wheel;
    if wheel != 0.0 {
        action.cycle += wheel.signum() as i32;
    }
}

fn release_held_actions(mut player: Single<(&mut ActionIntent, &mut AimIntent), With<Player>>) {
    player.0.fire_held = false;
    player.1.aiming = false;
}
