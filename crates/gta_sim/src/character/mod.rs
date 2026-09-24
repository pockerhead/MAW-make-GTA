mod anim;
mod health;
mod intent;
mod locomotion;

pub use anim::{AnimState, anim_state, is_airborne};
pub use health::{
    Dead, HEALTH_CONFIG, Health, HealthConfig, HealthSystems, PickupConfig, apply_damage,
    regenerate,
};
pub use intent::{ActionIntent, AimIntent, Gait, MoveIntent, WeaponRequest, move_direction};
pub use locomotion::{LOCOMOTION_CONFIG, LocomotionConfig};

use crate::combat::{HitReaction, Melee, MeleeConfig};
use crate::layers::GameLayer;
use crate::world::CityScoped;
use avian3d::prelude::*;
use bevy::prelude::*;
use bevy_tnua::TnuaSensorsSet;
use bevy_tnua::builtins::{TnuaBuiltinJump, TnuaBuiltinKnockback, TnuaBuiltinWalk};
use bevy_tnua::controller::TnuaActionFlowStatus;
use bevy_tnua::prelude::*;
use bevy_tnua_avian3d::prelude::*;

#[derive(TnuaScheme)]
#[scheme(basis = TnuaBuiltinWalk)]
pub enum CharacterScheme {
    Jump(TnuaBuiltinJump),
    Knockback(TnuaBuiltinKnockback),
}

#[derive(Component, Reflect, Default)]
#[reflect(Component)]
#[require(
    MoveIntent,
    AimIntent,
    ActionIntent,
    JumpBuffer,
    AnimState,
    HitReaction,
    Melee,
    CityScoped
)]
pub struct Character;

/// Held in place without control: the arrested player.
#[derive(Component, Reflect, Default)]
#[reflect(Component)]
pub struct Cuffed;

/// Head sphere sensor on the `Hitbox` layer; a hitscan ray that hits it deals headshot damage.
#[derive(Component, Reflect, Default)]
#[reflect(Component)]
pub struct HeadHitbox;

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
        let knockback = world.resource::<MeleeConfig>().knockback_tuning.tnua();
        let config = world.resource::<LocomotionConfig>().tnua_config(knockback);
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
            .register_type::<AimIntent>()
            .register_type::<ActionIntent>()
            .register_type::<HeadHitbox>()
            .register_type::<AnimState>()
            .register_type::<Health>()
            .register_type::<Dead>()
            .register_type::<Cuffed>()
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
            .add_observer(despawn_tnua_sensors)
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

/// bevy-tnua 0.32 relates its sensor entities without `linked_spawn`: without this every despawned
/// controller leaves them behind.
fn despawn_tnua_sensors(
    despawn: On<Despawn, TnuaSensorsSet>,
    sets: Query<&TnuaSensorsSet>,
    mut commands: Commands,
) {
    let Ok(set) = sets.get(despawn.entity) else {
        return;
    };
    for sensor in set.iter() {
        commands.entity(sensor).try_despawn();
    }
}

const SENSOR_INSET: f32 = 0.01;

/// Child of a character body: a sensor that neither collides nor is seen by Tnua's ground sensor.
pub fn head_hitbox(cfg: &LocomotionConfig) -> impl Bundle {
    (
        HeadHitbox,
        Name::new("Head hitbox"),
        Collider::sphere(cfg.head_radius),
        Sensor,
        CollisionLayers::new(GameLayer::Hitbox, LayerMask::NONE),
        Transform::from_xyz(0.0, cfg.head_height - cfg.float_height, 0.0),
    )
}

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
        CollisionLayers::new(GameLayer::Character, LayerMask::ALL),
        children![head_hitbox(cfg)],
    )
}

#[allow(clippy::type_complexity)]
fn drive_characters(
    cfg: Res<LocomotionConfig>,
    melee_cfg: Res<MeleeConfig>,
    time: Res<Time<Fixed>>,
    mut query: Query<
        (
            &mut MoveIntent,
            &AimIntent,
            &mut JumpBuffer,
            &mut TnuaController<CharacterScheme>,
            (Has<Dead>, Has<Cuffed>),
            (&HitReaction, &Melee),
        ),
        With<Character>,
    >,
) {
    for (mut intent, aim, mut buffer, mut controller, (dead, cuffed), (reaction, melee)) in
        &mut query
    {
        if dead || cuffed || reaction.is_active() {
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
        let (gait, forward) = if aim.aiming {
            let aim_flat = Vec3::new(aim.direction.x, 0.0, aim.direction.z);
            (intent.gait.min(cfg.aim_max_gait), aim_flat)
        } else {
            (intent.gait, direction)
        };
        // A swing faces the blow and keeps `swing_move_scale` of the gait; it cannot jump.
        let (speed, forward) = match melee.swing {
            Some(swing) => (
                cfg.speed(gait) * melee_cfg.swing_move_scale,
                swing.direction,
            ),
            None => (cfg.speed(gait), forward),
        };
        controller.basis = TnuaBuiltinWalk {
            desired_motion: direction * speed,
            desired_forward: Dir3::new(forward).ok(),
        };
        let swinging = melee.swing.is_some();
        if !swinging && (intent.jump_held || new_request || buffer.remaining > 0.0) {
            controller.action(CharacterScheme::Jump(Default::default()));
        }
        buffer.remaining = (buffer.remaining - time.delta_secs()).max(0.0);
        if new_request {
            intent.jump_requested = false;
        }
    }
}
