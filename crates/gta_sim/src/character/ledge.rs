use super::{Character, CharacterScheme, LocomotionConfig, MoveIntent, move_direction};
use avian3d::prelude::*;
use bevy::prelude::*;
use bevy_tnua::controller::TnuaActionFlowStatus;
use bevy_tnua::prelude::TnuaController;

#[derive(Component, Default)]
pub(super) struct LedgeAssist {
    launch_feet_y: f32,
    remaining: f32,
    target: Option<LedgeTarget>,
}

struct LedgeTarget {
    position: Vec3,
    snapped: bool,
    wall_normal: Vec3,
}

type AssistQuery<'w, 's> = Query<
    'w,
    's,
    (
        Entity,
        &'static MoveIntent,
        &'static TnuaController<CharacterScheme>,
        &'static mut LedgeAssist,
        &'static Position,
    ),
    With<Character>,
>;

pub(super) fn assist_ledge(
    cfg: Res<LocomotionConfig>,
    time: Res<Time<Fixed>>,
    spatial: SpatialQuery,
    mut characters: AssistQuery,
) {
    for (entity, intent, controller, mut assist, position) in &mut characters {
        if matches!(
            controller.action_flow_status(),
            TnuaActionFlowStatus::ActionStarted(_)
        ) && (assist.remaining <= 0.0 || controller.basis_memory.standing_on_entity().is_some())
        {
            assist.launch_feet_y = position.y - cfg.float_height;
            assist.remaining = cfg.ledge_assist_window;
        }
        if assist.remaining <= 0.0 {
            continue;
        }
        assist.remaining = (assist.remaining - time.delta_secs()).max(0.0);
        let direction = move_direction(intent.axis, intent.yaw);
        let Ok(forward) = Dir3::new(direction) else {
            continue;
        };
        let origin = position.0
            + direction * cfg.ledge_assist_forward_probe
            + Vec3::Y
                * (assist.launch_feet_y + cfg.ledge_assist_max_height + cfg.float_height
                    - position.y);
        let filter = SpatialQueryFilter::from_excluded_entities([entity]);
        let Some(top_hit) = spatial.cast_ray(
            origin,
            Dir3::NEG_Y,
            cfg.ledge_assist_max_height + cfg.float_height * 2.0,
            false,
            &filter,
        ) else {
            continue;
        };
        if top_hit.normal.y < 0.9 {
            continue;
        }
        let top_y = origin.y - top_hit.distance;
        let rise = top_y - assist.launch_feet_y;
        if rise <= cfg.jump_height || position.y <= top_y {
            continue;
        }
        let wall_origin = Vec3::new(position.x, top_y - cfg.ledge_assist_clearance, position.z);
        let Some(wall_hit) = spatial.cast_ray(
            wall_origin,
            forward,
            cfg.ledge_assist_forward_probe,
            false,
            &filter,
        ) else {
            continue;
        };
        if wall_hit.entity != top_hit.entity {
            continue;
        }
        if rise > cfg.ledge_assist_max_height {
            assist.remaining = cfg.ledge_assist_window;
            let incidence = -wall_hit.normal.dot(*forward);
            let keep_out = (cfg.capsule_radius + cfg.ledge_assist_clearance
                - wall_hit.distance * incidence)
                .max(0.0);
            assist.target = Some(LedgeTarget {
                position: position.0 + wall_hit.normal * keep_out,
                snapped: false,
                wall_normal: wall_hit.normal,
            });
            continue;
        }
        if wall_hit.normal.dot(*forward) > -0.5 {
            continue;
        }
        // Tnua's float spring settles slowly at an edge; finish the jump onto the detected top.
        let target = position.0
            + direction * (wall_hit.distance + cfg.capsule_radius + cfg.ledge_assist_clearance);
        let target = Vec3::new(target.x, top_y + cfg.float_height, target.z);
        let capsule = Collider::capsule(
            cfg.capsule_radius,
            cfg.capsule_height - 2.0 * cfg.capsule_radius,
        );
        if !spatial
            .shape_intersections(&capsule, target, Quat::IDENTITY, &filter)
            .is_empty()
        {
            continue;
        }
        assist.target = Some(LedgeTarget {
            position: target,
            snapped: true,
            wall_normal: Vec3::ZERO,
        });
        assist.remaining = 0.0;
    }
}

pub(super) fn apply_ledge(
    mut characters: Query<
        (
            &mut LedgeAssist,
            &mut Position,
            &mut LinearVelocity,
            &mut Transform,
        ),
        With<Character>,
    >,
) {
    for (mut assist, mut position, mut velocity, mut transform) in &mut characters {
        let Some(target) = assist.target.take() else {
            continue;
        };
        position.0 = target.position;
        transform.translation = target.position;
        if target.snapped {
            velocity.0 = Vec3::ZERO;
        } else {
            let into_wall = velocity.0.dot(target.wall_normal).min(0.0);
            velocity.0 -= target.wall_normal * into_wall;
        }
    }
}
