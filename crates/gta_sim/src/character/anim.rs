use super::{Character, CharacterScheme, CharacterSchemeActionDiscriminant, LocomotionConfig};
use avian3d::prelude::LinearVelocity;
use bevy::prelude::*;
use bevy_tnua::prelude::TnuaController;

/// Locomotion animation state derived from body velocity and ground support.
/// The client indexes per-state arrays by `state as usize`: keep the declaration order.
#[derive(Component, Reflect, Default, Clone, Copy, Debug, PartialEq, Eq)]
#[reflect(Component, Default)]
pub enum AnimState {
    #[default]
    Idle,
    Walk,
    Run,
    Sprint,
    Jump,
    Fall,
}

/// Airborne: `Jump` while rising, else `Fall`. Grounded: by horizontal speed, split at `anim_idle_speed`
/// and at the midpoints between walk/run and run/sprint speeds.
pub fn anim_state(velocity: Vec3, airborne: bool, cfg: &LocomotionConfig) -> AnimState {
    if airborne {
        return if velocity.y > 0.0 {
            AnimState::Jump
        } else {
            AnimState::Fall
        };
    }
    let horizontal = Vec2::new(velocity.x, velocity.z).length();
    if horizontal < cfg.anim_idle_speed {
        return AnimState::Idle;
    }
    if horizontal < (cfg.walk_speed + cfg.run_speed) / 2.0 {
        return AnimState::Walk;
    }
    if horizontal < (cfg.run_speed + cfg.sprint_speed) / 2.0 {
        return AnimState::Run;
    }
    AnimState::Sprint
}

/// In the air: Tnua's walk basis says so, or a jump is running. The basis alone reports ground while its
/// sensor (`float_height + ground_sensor_cling_distance`) still reaches the floor, i.e. for any hop under ~1 m;
/// the jump action stays active until landing.
pub fn is_airborne(controller: &TnuaController<CharacterScheme>) -> bool {
    controller.is_airborne().unwrap_or(false)
        || controller.action_discriminant() == Some(CharacterSchemeActionDiscriminant::Jump)
}

pub(super) fn update_anim_state(
    cfg: Res<LocomotionConfig>,
    mut query: Query<
        (
            &LinearVelocity,
            &TnuaController<CharacterScheme>,
            &mut AnimState,
        ),
        With<Character>,
    >,
) {
    for (velocity, controller, mut state) in &mut query {
        state.set_if_neq(anim_state(velocity.0, is_airborne(controller), &cfg));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn anim_state_table() {
        let cfg = ron::from_str::<LocomotionConfig>(include_str!(
            "../../../../assets/character/locomotion.ron"
        ))
        .expect("GATE BROKEN: locomotion.ron");
        use AnimState::*;
        let cases = [
            ((0.0, 0.0, 0.0), false, Idle),
            ((0.1, 0.0, 0.0), false, Idle),
            ((0.0, 0.0, -1.8), false, Walk),
            ((0.15, 0.0, -0.15), false, Walk),
            ((1.08, 0.0, -1.44), false, Walk),
            ((0.0, 0.0, -4.5), false, Run),
            ((0.0, 0.0, -6.8), false, Sprint),
            ((0.0, 3.0, -2.5), false, Walk),
            ((0.0, 3.0, -4.5), true, Jump),
            ((0.0, -2.0, -4.5), true, Fall),
            ((0.0, 0.0, 0.0), true, Fall),
            ((0.0, 0.0, -3.145), false, Walk),
            ((0.0, 0.0, -3.16), false, Run),
            ((0.0, 0.0, -5.64), false, Run),
            ((0.0, 0.0, -5.66), false, Sprint),
            ((0.0, 0.0, -0.19), false, Idle),
            ((0.0, 0.0, -0.21), false, Walk),
        ];
        for ((x, y, z), airborne, expected) in cases {
            assert_eq!(
                anim_state(Vec3::new(x, y, z), airborne, &cfg),
                expected,
                "velocity ({x}, {y}, {z}), airborne {airborne}"
            );
        }
    }
}
