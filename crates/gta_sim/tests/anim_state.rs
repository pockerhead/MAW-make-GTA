mod common;

use avian3d::prelude::LinearVelocity;
use bevy::prelude::*;
use bevy_tnua::prelude::TnuaController;
use common::*;
use gta_sim::character::{
    AnimState, CharacterScheme, Gait, LocomotionConfig, anim_state, is_airborne,
};

fn state(app: &mut App) -> AnimState {
    let entity = player(app);
    *app.world()
        .get::<AnimState>(entity)
        .expect("GATE BROKEN: player missing AnimState")
}

/// Full jump: held past the 1.0 m apex. Short hop: a 10-tick (156 ms) tap rises ~0.85 m, inside the
/// ground sensor reach (float_height 1.05 + cling 1.0 m), where Tnua's walk basis alone still reports ground.
const FULL_JUMP_TICKS: u32 = 30;
const SHORT_HOP_TICKS: u32 = 10;

/// Holds jump for `hold` ticks, then records the state of each of 120 ticks; `check` runs after every tick.
fn jump_states(app: &mut App, hold: u32, mut check: impl FnMut(&mut App)) -> Vec<AnimState> {
    settle(app);
    set_intent(app, |intent| intent.jump_held = true);
    let mut states = Vec::new();
    for tick in 0..120 {
        if tick == hold {
            set_intent(app, |intent| intent.jump_held = false);
        }
        run_ticks(app, 1);
        check(app);
        states.push(state(app));
    }
    states
}

#[test]
fn idle_after_settle() {
    let mut app = headless_app();
    settle(&mut app);
    assert_eq!(state(&mut app), AnimState::Idle);
}

#[test]
fn gaits_map_to_states() {
    for (gait, expected) in [
        (Gait::Walk, AnimState::Walk),
        (Gait::Run, AnimState::Run),
        (Gait::Sprint, AnimState::Sprint),
    ] {
        let mut app = headless_app();
        settle(&mut app);
        set_intent(&mut app, |intent| {
            intent.axis = Vec2::Y;
            intent.gait = gait;
        });
        run_ticks(&mut app, 32);
        assert_eq!(state(&mut app), expected, "gait {gait:?}");
    }
}

#[test]
fn jump_goes_up_then_falls_then_lands() {
    for hold in [FULL_JUMP_TICKS, SHORT_HOP_TICKS] {
        let mut app = headless_app();
        assert_jump_order(hold, &jump_states(&mut app, hold, |_| {}));
    }
}

fn assert_jump_order(hold: u32, states: &[AnimState]) {
    let first = states
        .iter()
        .position(|s| *s != AnimState::Idle)
        .unwrap_or_else(|| panic!("never left Idle: hold {hold}: {states:?}"));
    assert_eq!(states[first], AnimState::Jump, "hold {hold}: {states:?}");
    let last_jump = states.iter().rposition(|s| *s == AnimState::Jump).unwrap();
    assert!(
        states[last_jump..].contains(&AnimState::Fall),
        "no Fall after the last Jump: hold {hold}: {states:?}"
    );
    assert_eq!(
        states.last(),
        Some(&AnimState::Idle),
        "hold {hold}: {states:?}"
    );
    assert!(
        !states
            .iter()
            .any(|s| matches!(s, AnimState::Walk | AnimState::Run | AnimState::Sprint)),
        "hold {hold}: {states:?}"
    );
}

#[test]
fn anim_state_matches_post_step_velocity() {
    let mut app = headless_app();
    let states = jump_states(&mut app, FULL_JUMP_TICKS, |app| {
        let entity = player(app);
        let world = app.world();
        let velocity = world.get::<LinearVelocity>(entity).unwrap().0;
        let airborne = is_airborne(
            world
                .get::<TnuaController<CharacterScheme>>(entity)
                .unwrap(),
        );
        let expected = anim_state(velocity, airborne, world.resource::<LocomotionConfig>());
        let actual = *world.get::<AnimState>(entity).unwrap();
        assert_eq!(actual, expected, "velocity {velocity}, airborne {airborne}");
    });
    assert!(
        states.contains(&AnimState::Jump) && states.contains(&AnimState::Fall),
        "GATE BROKEN: the jump never showed both Jump and Fall: {states:?}"
    );
}
