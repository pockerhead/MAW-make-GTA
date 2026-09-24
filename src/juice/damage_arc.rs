//! Damage-direction arc (GDD §8): a red arc on a ring around the screen centre points at whoever
//! hurt the player, follows the attacker while it lives and fades out on real time.

use super::JuiceConfig;
use crate::camera::OrbitCamera;
use bevy::prelude::*;
use gta_sim::{combat::DamageDealt, player::Player, world::CityScoped};
use std::collections::HashSet;

#[derive(Component, Reflect)]
#[reflect(Component)]
pub struct DamageArc {
    pub shooter: Entity,
    /// Clockwise from screen-up, radians.
    pub angle: f32,
    /// Real seconds left.
    pub left: f32,
}

pub(super) struct DamageArcPlugin;

impl Plugin for DamageArcPlugin {
    fn build(&self, app: &mut App) {
        app.register_type::<DamageArc>()
            .add_systems(Update, (spawn_or_refresh_arcs, update_arcs).chain());
    }
}

/// Screen angle of `attacker` seen from `player` under camera yaw `yaw`: 0 = straight ahead (up),
/// positive = clockwise (to the right). `None` when they coincide on the ground plane.
pub fn arc_angle(yaw: f32, player: Vec3, attacker: Vec3) -> Option<f32> {
    let d = (attacker - player).with_y(0.0);
    if d.length() < 1e-3 {
        return None;
    }
    let forward = Vec3::new(-yaw.sin(), 0.0, -yaw.cos());
    let right = Vec3::new(yaw.cos(), 0.0, -yaw.sin());
    Some(d.dot(right).atan2(d.dot(forward)))
}

fn spawn_or_refresh_arcs(
    mut commands: Commands,
    mut dealt: MessageReader<DamageDealt>,
    juice: Res<JuiceConfig>,
    players: Query<(Entity, &Transform), With<Player>>,
    cameras: Query<&OrbitCamera>,
    bodies: Query<&Transform>,
    mut arcs: Query<&mut DamageArc>,
) {
    let cfg = &juice.damage_arc;
    let (Ok((player, at)), Ok(camera)) = (players.single(), cameras.single()) else {
        dealt.clear();
        return;
    };
    // Arcs spawned in this call are not in `arcs` yet: one blast's pellets must not stack arcs.
    let mut spawned = HashSet::new();
    for hit in dealt.read() {
        if hit.target != player || hit.shooter == player {
            continue;
        }
        if let Some(mut arc) = arcs.iter_mut().find(|arc| arc.shooter == hit.shooter) {
            arc.left = cfg.seconds;
            continue;
        }
        if !spawned.insert(hit.shooter) {
            continue;
        }
        let Ok(shooter) = bodies.get(hit.shooter) else {
            continue;
        };
        let Some(angle) = arc_angle(camera.yaw, at.translation, shooter.translation) else {
            continue;
        };
        let (r, g, b) = cfg.color;
        let radius = cfg.radius_px;
        commands.spawn((
            Name::new("Damage arc"),
            DamageArc {
                shooter: hit.shooter,
                angle,
                left: cfg.seconds,
            },
            CityScoped,
            Node {
                position_type: PositionType::Absolute,
                width: px(2.0 * radius),
                height: px(2.0 * radius),
                left: percent(50),
                top: percent(50),
                margin: UiRect {
                    left: px(-radius),
                    top: px(-radius),
                    ..default()
                },
                border: UiRect::top(px(cfg.thickness_px)),
                border_radius: BorderRadius::MAX,
                ..default()
            },
            BorderColor {
                top: Color::srgb(r, g, b),
                ..BorderColor::DEFAULT
            },
            UiTransform::from_rotation(Rot2::radians(angle)),
        ));
    }
}

fn update_arcs(
    mut commands: Commands,
    juice: Res<JuiceConfig>,
    real: Res<Time<Real>>,
    players: Query<&Transform, With<Player>>,
    cameras: Query<&OrbitCamera>,
    bodies: Query<&Transform>,
    mut arcs: Query<(Entity, &mut DamageArc, &mut UiTransform, &mut BorderColor)>,
) {
    let seconds = juice.damage_arc.seconds;
    let view = players.single().ok().zip(cameras.single().ok());
    for (entity, mut arc, mut transform, mut color) in &mut arcs {
        arc.left -= real.delta_secs();
        if arc.left <= 0.0 {
            commands.entity(entity).try_despawn();
            continue;
        }
        // A despawned attacker leaves the arc frozen where it was.
        let angle = view.and_then(|(player, camera)| {
            let shooter = bodies.get(arc.shooter).ok()?;
            arc_angle(camera.yaw, player.translation, shooter.translation)
        });
        if let Some(angle) = angle {
            arc.angle = angle;
        }
        transform.rotation = Rot2::radians(arc.angle);
        color.top.set_alpha(arc.left / seconds);
    }
}
