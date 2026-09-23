mod anim;
mod health;
mod intent;
mod locomotion;

pub use anim::{AnimState, anim_state, is_airborne};
pub use health::{
    Dead, HEALTH_CONFIG, Health, HealthConfig, HealthSystems, PickupConfig, apply_damage,
    regenerate,
};
pub use intent::{Gait, MoveIntent, move_direction};
pub use locomotion::{LOCOMOTION_CONFIG, LocomotionConfig};

use avian3d::prelude::*;
use bevy::prelude::*;
use bevy_tnua::builtins::{TnuaBuiltinJump, TnuaBuiltinWalk};
use bevy_tnua::controller::TnuaActionFlowStatus;
use bevy_tnua::prelude::*;
use bevy_tnua_avian3d::prelude::*;

#[derive(TnuaScheme)]
#[scheme(basis = TnuaBuiltinWalk)]
pub enum CharacterScheme {
    Jump(TnuaBuiltinJump),
}

#[derive(Component, Reflect, Default)]
#[reflect(Component)]
#[require(MoveIntent, JumpBuffer, AnimState)]
pub struct Character;

#[derive(Component, Default)]
struct JumpBuffer {
    remaining: f32,
}

#[derive(Component, Reflect, Clone, Copy, Debug)]
#[reflect(Component)]
pub struct CharacterBody {
    pub radius: f32,
    pub height: f32,
    pub float_height: f32,
}

#[derive(Resource)]
pub struct CharacterControlConfig(pub Handle<CharacterSchemeConfig>);

impl FromWorld for CharacterControlConfig {
    fn from_world(world: &mut World) -> Self {
        let config = world.resource::<LocomotionConfig>().tnua_config();
        Self(
            world
                .resource_mut::<Assets<CharacterSchemeConfig>>()
                .add(config),
        )
    }
}

pub struct CharacterPlugin;

impl Plugin for CharacterPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(TnuaControllerPlugin::<CharacterScheme>::new(FixedUpdate))
            .init_resource::<CharacterControlConfig>()
            .register_type::<Character>()
            .register_type::<CharacterBody>()
            .register_type::<MoveIntent>()
            .register_type::<AnimState>()
            .register_type::<Health>()
            .register_type::<Dead>()
            .configure_sets(
                FixedUpdate,
                (
                    HealthSystems::Damage,
                    HealthSystems::Regen,
                    HealthSystems::Pickup,
                    HealthSystems::Death,
                )
                    .chain(),
            )
            .add_systems(
                FixedUpdate,
                drive_characters.in_set(TnuaUserControlsSystems),
            )
            // Avian steps in FixedPostUpdate (`PhysicsPlugins::default()`): read the post-step velocity.
            .add_systems(
                FixedPostUpdate,
                anim::update_anim_state.after(PhysicsSystems::Last),
            );
    }
}

const SENSOR_INSET: f32 = 0.01;

pub fn character_components(
    cfg: &LocomotionConfig,
    handle: Handle<CharacterSchemeConfig>,
) -> impl Bundle {
    (
        Character,
        CharacterBody {
            radius: cfg.capsule_radius,
            height: cfg.capsule_height,
            float_height: cfg.float_height,
        },
        RigidBody::Dynamic,
        Collider::capsule(
            cfg.capsule_radius,
            cfg.capsule_height - 2.0 * cfg.capsule_radius,
        ),
        LockedAxes::ROTATION_LOCKED.unlock_rotation_y(),
        TnuaController::<CharacterScheme>::default(),
        TnuaConfig::<CharacterScheme>(handle),
        // A narrower sensor avoids snagging on walls beside the capsule.
        TnuaAvian3dSensorShape(Collider::cylinder(cfg.capsule_radius - SENSOR_INSET, 0.0)),
    )
}

#[allow(clippy::type_complexity)]
fn drive_characters(
    cfg: Res<LocomotionConfig>,
    time: Res<Time<Fixed>>,
    mut query: Query<
        (
            &mut MoveIntent,
            &mut JumpBuffer,
            &mut TnuaController<CharacterScheme>,
            Has<Dead>,
        ),
        With<Character>,
    >,
) {
    for (mut intent, mut buffer, mut controller, dead) in &mut query {
        if dead {
            // The walk basis persists in Tnua: without an explicit zero the body keeps walking.
            intent.jump_requested = false;
            buffer.remaining = 0.0;
            controller.initiate_action_feeding();
            controller.basis = TnuaBuiltinWalk {
                desired_motion: Vec3::ZERO,
                desired_forward: None,
            };
            continue;
        }
        let new_request = intent.jump_requested;
        if !new_request
            && matches!(
                controller.action_flow_status(),
                TnuaActionFlowStatus::ActionStarted(_)
            )
        {
            buffer.remaining = 0.0;
        }
        if new_request {
            buffer.remaining = cfg.jump_buffer;
        }
        controller.initiate_action_feeding();
        let direction = move_direction(intent.axis, intent.yaw);
        controller.basis = TnuaBuiltinWalk {
            desired_motion: direction * cfg.speed(intent.gait),
            desired_forward: Dir3::new(direction).ok(),
        };
        if intent.jump_held || new_request || buffer.remaining > 0.0 {
            controller.action(CharacterScheme::Jump(Default::default()));
        }
        buffer.remaining = (buffer.remaining - time.delta_secs()).max(0.0);
        if new_request {
            intent.jump_requested = false;
        }
    }
}
