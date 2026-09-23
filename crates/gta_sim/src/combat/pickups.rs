use crate::character::{CharacterBody, Dead, Health, HealthConfig};
use crate::player::Player;
use crate::world::HospitalSpawn;
use avian3d::prelude::*;
use bevy::prelude::*;

#[derive(Reflect, Clone, Copy, Debug, PartialEq, Eq)]
pub enum PickupKind {
    Health,
    Armor,
}

/// A medkit or armour pickup; taken when the player's feet come within `pickups.radius`.
#[derive(Component, Reflect, Debug)]
#[reflect(Component)]
pub struct Pickup {
    pub kind: PickupKind,
    /// Seconds until the pickup is available again.
    pub cooldown: f32,
}

impl Pickup {
    pub fn available(&self) -> bool {
        self.cooldown <= 0.0
    }
}

pub(super) fn spawn_pickups(
    mut commands: Commands,
    spawn: Res<HospitalSpawn>,
    cfg: Res<HealthConfig>,
) {
    let offset = spawn.along * cfg.pickups.spacing;
    for (kind, at, name) in [
        (PickupKind::Health, spawn.point + offset, "Pickup Health"),
        (PickupKind::Armor, spawn.point - offset, "Pickup Armor"),
    ] {
        commands.spawn((
            Pickup {
                kind,
                cooldown: 0.0,
            },
            Name::new(name),
            Transform::from_translation(at),
        ));
    }
}

#[allow(clippy::type_complexity)]
pub(super) fn collect_pickups(
    cfg: Res<HealthConfig>,
    time: Res<Time<Fixed>>,
    mut pickups: Query<(&mut Pickup, &Transform)>,
    mut players: Query<(&Position, &CharacterBody, &mut Health), (With<Player>, Without<Dead>)>,
) {
    let dt = time.delta_secs();
    let p = &cfg.pickups;
    for (mut pickup, transform) in &mut pickups {
        pickup.cooldown = (pickup.cooldown - dt).max(0.0);
        if !pickup.available() {
            continue;
        }
        for (position, body, mut health) in &mut players {
            let feet = position.0 - Vec3::Y * body.float_height;
            if feet.distance(transform.translation) > p.radius {
                continue;
            }
            match pickup.kind {
                PickupKind::Health if health.current < cfg.max_health => {
                    health.current = (health.current + p.health).min(cfg.max_health);
                    pickup.cooldown = p.respawn;
                }
                PickupKind::Armor if health.armor < cfg.max_armor => {
                    health.armor = (health.armor + p.armor).min(cfg.max_armor);
                    pickup.cooldown = p.respawn;
                }
                _ => {}
            }
        }
    }
}
