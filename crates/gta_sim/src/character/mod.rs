mod intent;
mod ledge;
mod locomotion;

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
#[require(MoveIntent, JumpBuffer, ledge::LedgeAssist)]
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
            .add_systems(
                FixedUpdate,
                drive_characters.in_set(TnuaUserControlsSystems),
            )
            .add_systems(
                FixedUpdate,
                (ledge::assist_ledge, ledge::apply_ledge)
                    .chain()
                    .after(TnuaPipelineSystems::Motors),
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

fn drive_characters(
    cfg: Res<LocomotionConfig>,
    time: Res<Time<Fixed>>,
    mut query: Query<
        (
            &mut MoveIntent,
            &mut JumpBuffer,
            &mut TnuaController<CharacterScheme>,
        ),
        With<Character>,
    >,
) {
    for (mut intent, mut buffer, mut controller) in &mut query {
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
