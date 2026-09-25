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
pub(crate) struct Spot {
    pub(crate) centre: Vec3,
    pub(crate) rotation: Quat,
}

/// Index of the roof among the exit candidates (0 = left door, 1 = right door).
pub(crate) const ROOF_EXIT: usize = 2;

/// Exit points of a car: the left door, the right door, the roof, each with the feet level it
/// counts at (the car's ground level at a door, the car's top on the roof).
fn candidates(
    cfg: &VehicleConfig,
    loco: &LocomotionConfig,
    position: Vec3,
    rotation: Quat,
) -> [(Vec3, f32); 3] {
    let door = cfg.door();
    let mirrored = Vec3::new(-door.x, door.y, door.z);
    let up = rotation * Vec3::Y;
    let roof = position + up * (cfg.half_extents().y + loco.float_height);
    let ground = (position - up * cfg.rest_height()).y;
    let top = (position + up * cfg.half_extents().y).y;
    [
        (door_point(door, position, rotation), ground),
        (door_point(mirrored, position, rotation), ground),
        (roof, top),
    ]
}

/// Feet under `at`: the first floor (World or Vehicle) below `at` + 2 m.
fn feet_below(spatial: &SpatialQuery, at: Vec3) -> Option<Vec3> {
    let floors = SpatialQueryFilter::from_mask([GameLayer::World, GameLayer::Vehicle]);
    spatial
        .cast_ray(at + Vec3::Y * 2.0, Dir3::NEG_Y, 6.0, true, &floors)
        .map(|hit| at + Vec3::Y * (2.0 - hit.distance))
}

/// A step the float spring walks over: the capsule's clearance above the ground.
fn step(loco: &LocomotionConfig) -> f32 {
    loco.float_height - loco.capsule_height / 2.0
}

/// Every clear exit point, in candidate order (index 0 = left door). A door counts only at the car's
/// ground level and the roof only on the car's top, so the top of a low wall or an overhang is never
/// an exit; the capsule must fit there (bodies of `exclude` aside) and the way from the seat must be
/// free, else a thin fence beside the door is walked through.
pub(crate) fn exit_spots(
    spatial: &SpatialQuery,
    cfg: &VehicleConfig,
    loco: &LocomotionConfig,
    car: (Entity, Vec3, Quat),
    exclude: &[Entity],
) -> Vec<(usize, Spot)> {
    let (vehicle, position, rotation) = car;
    let step = step(loco);
    let floors = SpatialQueryFilter::from_mask([GameLayer::World, GameLayer::Vehicle]);
    let blockers =
        SpatialQueryFilter::from_mask([GameLayer::World, GameLayer::Vehicle, GameLayer::Character])
            .with_excluded_entities(exclude.iter().copied());
    let capsule = Collider::capsule(
        loco.capsule_radius,
        loco.capsule_height - 2.0 * loco.capsule_radius,
    );
    let seat = position + rotation * cfg.seat();
    let walls = floors.with_excluded_entities([vehicle]);
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
    candidates(cfg, loco, position, rotation)
        .into_iter()
        .enumerate()
        .filter_map(|(k, (at, level))| {
            let feet = feet_below(spatial, at).filter(|feet| (feet.y - level).abs() <= step)?;
            let centre = feet + Vec3::Y * loco.float_height;
            (path_free(centre)
                && spatial
                    .shape_intersections(&capsule, centre, Quat::IDENTITY, &blockers)
                    .is_empty())
            .then_some((
                k,
                Spot {
                    centre,
                    rotation: yaw,
                },
            ))
        })
        .collect()
}

/// The first clear exit point. `forced` (Wasted, Busted) never fails: the first candidate standing at
/// its own level (a body or a fence may be in the way), else the car's roof with no ray at all (an
/// overhang above the car); never the top of a wall beside a door.
fn exit_spot(
    spatial: &SpatialQuery,
    cfg: &VehicleConfig,
    loco: &LocomotionConfig,
    car: (Entity, Vec3, Quat),
    forced: bool,
) -> Option<Spot> {
    if let Some((_, spot)) = exit_spots(spatial, cfg, loco, car, &[]).into_iter().next() {
        return Some(spot);
    }
    if !forced {
        return None;
    }
    let (_, position, rotation) = car;
    let rotation_yaw = Quat::from_rotation_y(aim_yaw(rotation * Vec3::NEG_Z));
    let all = candidates(cfg, loco, position, rotation);
    let step = step(loco);
    let level = all.iter().find_map(|&(at, level)| {
        feet_below(spatial, at).filter(|feet| (feet.y - level).abs() <= step)
    });
    let centre = match level {
        Some(feet) => feet + Vec3::Y * loco.float_height,
        None => all[2].0,
    };
    Some(Spot {
        centre,
        rotation: rotation_yaw,
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

/// A cop pulls the driver out through the left door: only a clear left-door spot counts (the right
/// door or the roof would put the player out of the cop's reach), unless `any_exit` (either door,
/// never the roof). The bodies of `pullers` (the arresting cops) do not block a spot. `false`: nothing
/// happened.
#[allow(clippy::too_many_arguments)]
pub(crate) fn pull_out(
    commands: &mut Commands,
    spatial: &SpatialQuery,
    cfg: &VehicleConfig,
    loco: &LocomotionConfig,
    player: (Entity, Option<&Children>),
    heads: &Query<(), With<HeadHitbox>>,
    car: (Entity, Vec3, Quat),
    vehicle: &mut Vehicle,
    pullers: &[Entity],
    any_exit: bool,
) -> bool {
    let spot = exit_spots(spatial, cfg, loco, car, pullers)
        .into_iter()
        .find(|(k, _)| *k == 0 || (any_exit && *k != ROOF_EXIT));
    let Some((_, spot)) = spot else {
        return false;
    };
    leave(commands, player.0, player.1, heads, spot);
    free_car(commands, car.0, vehicle);
    true
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
            let Some(spot) = exit_spot(&spatial, &cfg, &loco, (car, car_pos.0, car_rot.0), false)
            else {
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
