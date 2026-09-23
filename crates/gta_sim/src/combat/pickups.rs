use super::melee::MeleeWeapon;
use super::weapons::{Loadout, Weapon, WeaponsConfig, acquire};
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

/// A gun (or only its ammo) lying on the shooting range.
#[derive(Component, Reflect, Debug)]
#[reflect(Component)]
pub struct WeaponPickup {
    pub weapon: Weapon,
    pub ammo_only: bool,
    /// Seconds until the pickup is available again.
    pub cooldown: f32,
}

impl WeaponPickup {
    pub fn available(&self) -> bool {
        self.cooldown <= 0.0
    }
}

#[allow(clippy::type_complexity)]
pub(super) fn collect_weapon_pickups(
    cfg: Res<WeaponsConfig>,
    time: Res<Time<Fixed>>,
    mut pickups: Query<(&mut WeaponPickup, &Transform)>,
    mut players: Query<(&Position, &CharacterBody, &mut Loadout), (With<Player>, Without<Dead>)>,
) {
    let dt = time.delta_secs();
    for (mut pickup, transform) in &mut pickups {
        pickup.cooldown = (pickup.cooldown - dt).max(0.0);
        if !pickup.available() {
            continue;
        }
        for (position, body, mut loadout) in &mut players {
            let feet = position.0 - Vec3::Y * body.float_height;
            if feet.distance(transform.translation) > cfg.pickups.radius {
                continue;
            }
            let weapon = pickup.weapon;
            let gun = !pickup.ammo_only;
            if !acquire(&mut loadout.guns[weapon.index()], cfg.stats(weapon), gun) {
                continue;
            }
            pickup.cooldown = cfg.pickups.respawn;
            if gun && loadout.held.is_none() {
                loadout.held = Some(weapon);
            }
        }
    }
}

/// The bat lying on the shooting range; shares the gun pickups' radius and respawn.
#[derive(Component, Reflect, Debug, Default)]
#[reflect(Component)]
pub struct BatPickup {
    /// Seconds until the pickup is available again.
    pub cooldown: f32,
}

impl BatPickup {
    pub fn available(&self) -> bool {
        self.cooldown <= 0.0
    }
}

#[allow(clippy::type_complexity)]
pub(super) fn collect_bat_pickups(
    cfg: Res<WeaponsConfig>,
    time: Res<Time<Fixed>>,
    mut pickups: Query<(&mut BatPickup, &Transform)>,
    mut players: Query<(&Position, &CharacterBody, &mut Loadout), (With<Player>, Without<Dead>)>,
) {
    let dt = time.delta_secs();
    for (mut pickup, transform) in &mut pickups {
        pickup.cooldown = (pickup.cooldown - dt).max(0.0);
        if !pickup.available() {
            continue;
        }
        for (position, body, mut loadout) in &mut players {
            let feet = position.0 - Vec3::Y * body.float_height;
            if loadout.has_bat || feet.distance(transform.translation) > cfg.pickups.radius {
                continue;
            }
            loadout.has_bat = true;
            if loadout.held.is_none() {
                loadout.melee = MeleeWeapon::Bat;
            }
            pickup.cooldown = cfg.pickups.respawn;
        }
    }
}
