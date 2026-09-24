use super::{Driving, Vehicle, VehicleConfig, VehicleEntered, door_point};
use crate::character::{ActionIntent, Cuffed, Dead, HeadHitbox, LocomotionConfig};
use crate::combat::{AttackSerial, HitReaction, aim_yaw};
use crate::layers::GameLayer;
use crate::player::Player;
use avian3d::prelude::*;
use bevy::prelude::*;
use bevy_tnua::TnuaToggle;

type Body<'a> = (
    &'a mut Position,
    &'a mut Rotation,
    &'a mut Transform,
    &'a mut LinearVelocity,
);

/// Where the driver stands after leaving a car: body centre and yaw.
struct Spot {
    centre: Vec3,
    rotation: Quat,
}

/// Exit points around a car: the left door, the right door, the roof. A door counts only at the
/// car's ground level and the roof only on the car's top, so the top of a low wall or an overhang
/// is never an exit. The way from the seat to a spot must be free too, else a thin fence beside
/// the door is walked through. `forced` falls back to the left door when none is clear.
fn exit_spot(
    spatial: &SpatialQuery,
    cfg: &VehicleConfig,
    loco: &LocomotionConfig,
    car: (Entity, Vec3, Quat),
    forced: bool,
) -> Option<Spot> {
    let (vehicle, position, rotation) = car;
    let door = cfg.door();
    let mirrored = Vec3::new(-door.x, door.y, door.z);
    let up = rotation * Vec3::Y;
    let roof = position + up * (cfg.half_extents().y + loco.float_height);
    let ground = (position - up * cfg.rest_height()).y;
    let top = (position + up * cfg.half_extents().y).y;
    // (candidate, expected feet height)
    let candidates = [
        (door_point(door, position, rotation), ground),
        (door_point(mirrored, position, rotation), ground),
        (roof, top),
    ];
    // A step the float spring walks over: the capsule's clearance above the ground.
    let step = loco.float_height - loco.capsule_height / 2.0;
    let floors = SpatialQueryFilter::from_mask([GameLayer::World, GameLayer::Vehicle]);
    let blockers =
        SpatialQueryFilter::from_mask([GameLayer::World, GameLayer::Vehicle, GameLayer::Character]);
    let capsule = Collider::capsule(
        loco.capsule_radius,
        loco.capsule_height - 2.0 * loco.capsule_radius,
    );
    let feet = |at: Vec3| {
        spatial
            .cast_ray(at + Vec3::Y * 2.0, Dir3::NEG_Y, 6.0, true, &floors)
            .map(|hit| at + Vec3::Y * (2.0 - hit.distance))
    };
    let seat = position + rotation * cfg.seat();
    let walls = floors.clone().with_excluded_entities([vehicle]);
    let ball = Collider::sphere(loco.capsule_radius);
    let path_free = |to: Vec3| {
        let Ok((direction, distance)) = Dir3::new_and_length(to - seat) else {
            return true;
        };
        let config = ShapeCastConfig::from_max_distance(distance);
        spatial
            .cast_shape(&ball, seat, Quat::IDENTITY, direction, &config, &walls)
            .is_none()
    };
    let yaw = Quat::from_rotation_y(aim_yaw(rotation * Vec3::NEG_Z));
    let clear = candidates.into_iter().find_map(|(at, level)| {
        let feet = feet(at).filter(|feet| (feet.y - level).abs() <= step)?;
        let centre = feet + Vec3::Y * loco.float_height;
        (path_free(centre)
            && spatial
                .shape_intersections(&capsule, centre, Quat::IDENTITY, &blockers)
                .is_empty())
        .then_some(centre)
    });
    let centre = match clear {
        Some(centre) => centre,
        None if forced => {
            let (at, _) = candidates[0];
            feet(at).unwrap_or(at - Vec3::Y * cfg.rest_height()) + Vec3::Y * loco.float_height
        }
        None => return None,
    };
    Some(Spot {
        centre,
        rotation: yaw,
    })
}

/// Undoes `enter` on the player and puts it at `spot`.
fn leave(
    commands: &mut Commands,
    player: Entity,
    children: Option<&Children>,
    heads: &Query<(), With<HeadHitbox>>,
    spot: Spot,
) {
    // Inserted, not mutated: the callers hold a `SpatialQuery`, which reads `Position`.
    commands.entity(player).try_insert((
        Position(spot.centre),
        Rotation(spot.rotation),
        Transform::from_translation(spot.centre).with_rotation(spot.rotation),
        LinearVelocity::ZERO,
    ));
    commands
        .entity(player)
        .try_remove::<(Driving, RigidBodyDisabled, ColliderDisabled, TnuaToggle)>();
    for head in children
        .into_iter()
        .flatten()
        .filter(|c| heads.contains(**c))
    {
        commands.entity(*head).try_remove::<ColliderDisabled>();
    }
}

fn free_car(commands: &mut Commands, car: Entity, vehicle: &mut Vehicle) {
    vehicle.driver = None;
    commands.entity(car).try_remove::<SleepingDisabled>();
}

#[allow(clippy::type_complexity, clippy::too_many_arguments)]
pub(super) fn enter_exit(
    mut commands: Commands,
    spatial: SpatialQuery,
    cfg: Res<VehicleConfig>,
    loco: Res<LocomotionConfig>,
    mut serial: ResMut<AttackSerial>,
    mut entered: MessageWriter<VehicleEntered>,
    mut players: Query<
        (
            Entity,
            &mut ActionIntent,
            &Position,
            Option<&Driving>,
            (&HitReaction, Has<Dead>, Has<Cuffed>),
            Option<&Children>,
        ),
        With<Player>,
    >,
    mut vehicles: Query<
        (Entity, &mut Vehicle, &Position, &Rotation, &LinearVelocity),
        Without<Player>,
    >,
    heads: Query<(), With<HeadHitbox>>,
) {
    for (player, mut action, position, driving, (reaction, dead, cuffed), children) in &mut players
    {
        if !std::mem::take(&mut action.vehicle_requested)
            || dead
            || cuffed
            || reaction.is_knocked_down()
        {
            continue;
        }
        if let Some(driving) = driving {
            let Ok((car, mut vehicle, car_pos, car_rot, car_vel)) =
                vehicles.get_mut(driving.vehicle)
            else {
                continue;
            };
            if car_vel.length() > cfg.exit_max_speed {
                continue;
            }
            let Some(spot) = exit_spot(&spatial, &cfg, &loco, (car, car_pos.0, car_rot.0), false) else {
                continue;
            };
            leave(&mut commands, player, children, &heads, spot);
            free_car(&mut commands, car, &mut vehicle);
            continue;
        }
        let flat = |v: Vec3| Vec2::new(v.x, v.z);
        // Getting in has the same speed limit as getting out.
        let nearest = vehicles
            .iter()
            .filter(|(.., velocity)| velocity.length() <= cfg.exit_max_speed)
            .filter(|(_, vehicle, ..)| vehicle.driver.is_none())
            .map(|(car, _, p, r, _)| {
                let door = door_point(cfg.door(), p.0, r.0);
                (car, flat(door).distance(flat(position.0)))
            })
            .filter(|&(_, distance)| distance <= cfg.enter_radius)
            .min_by(|a, b| a.1.total_cmp(&b.1));
        let Some((car, _)) = nearest else {
            continue;
        };
        let Ok((_, mut vehicle, ..)) = vehicles.get_mut(car) else {
            continue;
        };
        commands.entity(player).insert((
            Driving { vehicle: car },
            RigidBodyDisabled,
            ColliderDisabled,
            TnuaToggle::Disabled,
            LinearVelocity::ZERO,
        ));
        for head in children
            .into_iter()
            .flatten()
            .filter(|c| heads.contains(**c))
        {
            commands.entity(*head).insert(ColliderDisabled);
        }
        // Held fire or a queued request must not fire from the seat on the enter tick.
        *action = ActionIntent::default();
        vehicle.driver = Some(player);
        let first = !vehicle.taken;
        vehicle.taken = true;
        commands.entity(car).insert(SleepingDisabled);
        entered.write(VehicleEntered {
            vehicle: car,
            driver: player,
            attack: serial.next_id(),
            first,
        });
    }
}

/// Wasted / Busted while driving: out of the car before the respawn teleport.
#[allow(clippy::type_complexity)]
pub(super) fn eject_all(
    mut commands: Commands,
    spatial: SpatialQuery,
    cfg: Res<VehicleConfig>,
    loco: Res<LocomotionConfig>,
    players: Query<(Entity, &Driving, &Position, Option<&Children>), With<Player>>,
    mut vehicles: Query<(&mut Vehicle, &Position, &Rotation), Without<Player>>,
    heads: Query<(), With<HeadHitbox>>,
) {
    for (player, driving, position, children) in &players {
        let spot = match vehicles.get_mut(driving.vehicle) {
            Ok((mut vehicle, car_pos, car_rot)) => {
                free_car(&mut commands, driving.vehicle, &mut vehicle);
                exit_spot(
                    &spatial,
                    &cfg,
                    &loco,
                    (driving.vehicle, car_pos.0, car_rot.0),
                    true,
                )
            }
            Err(_) => None,
        };
        let spot = spot.unwrap_or(Spot {
            centre: position.0,
            rotation: Quat::IDENTITY,
        });
        leave(&mut commands, player, children, &heads, spot);
    }
}

/// Pins every driver to its seat after the physics step; a driver whose car is gone is put back
/// on foot where it sat.
#[allow(clippy::type_complexity)]
pub(super) fn sync_seats(
    mut commands: Commands,
    cfg: Res<VehicleConfig>,
    mut players: Query<(Entity, &Driving, Body, Option<&Children>), With<Player>>,
    mut vehicles: Query<
        (Entity, &mut Vehicle, &Position, &Rotation, &LinearVelocity),
        Without<Player>,
    >,
    heads: Query<(), With<HeadHitbox>>,
    drivers: Query<&Driving>,
) {
    for (player, driving, body, children) in &mut players {
        let (mut position, mut rotation, mut transform, mut velocity) = body;
        let Ok((_, _, car_pos, car_rot, car_vel)) = vehicles.get(driving.vehicle) else {
            let spot = Spot {
                centre: position.0,
                rotation: Quat::from_rotation_y(aim_yaw(rotation.0 * Vec3::NEG_Z)),
            };
            leave(&mut commands, player, children, &heads, spot);
            continue;
        };
        let seat = car_pos.0 + car_rot.0 * cfg.seat();
        position.0 = seat;
        rotation.0 = car_rot.0;
        transform.translation = seat;
        transform.rotation = car_rot.0;
        velocity.0 = car_vel.0;
    }
    for (car, mut vehicle, ..) in &mut vehicles {
        let Some(driver) = vehicle.driver else {
            continue;
        };
        if drivers.get(driver).is_ok_and(|d| d.vehicle == car) {
            continue;
        }
        free_car(&mut commands, car, &mut vehicle);
    }
}
