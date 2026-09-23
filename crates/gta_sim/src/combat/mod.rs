mod hitscan;
mod pickups;
mod range;
mod weapons;

pub use hitscan::{
    AIM_CONFIG, AimConfig, BulletTrace, CombatRng, DamageDealt, ShotFired, aim_yaw, cone_sample,
    muzzle,
};
pub use pickups::{Pickup, PickupKind, WeaponPickup};
pub use range::{Dummy, dummy_bundle};
pub use weapons::{
    FireMode, GunSlot, Loadout, WEAPONS_CONFIG, Weapon, WeaponsConfig, acquire, cycle_weapon,
    falloff_factor, roll_damage,
};

use crate::character::HealthSystems;
use crate::flow::{GameState, PlayingSystems};
use crate::world::CityLandmarks;
use bevy::prelude::*;

pub struct CombatPlugin {
    /// Seed of `CombatRng`.
    pub seed: u64,
}

impl Plugin for CombatPlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(CombatRng::seeded(self.seed))
            .add_message::<ShotFired>()
            .add_message::<BulletTrace>()
            .add_message::<DamageDealt>()
            .register_type::<Pickup>()
            .register_type::<PickupKind>()
            .register_type::<Loadout>()
            .register_type::<GunSlot>()
            .register_type::<Weapon>()
            .register_type::<WeaponPickup>()
            .register_type::<Dummy>()
            .register_type::<ShotFired>()
            .register_type::<BulletTrace>()
            .register_type::<DamageDealt>()
            .add_systems(
                OnTransition {
                    exited: GameState::Loading,
                    entered: GameState::Playing,
                },
                (
                    pickups::spawn_pickups,
                    range::spawn_range.run_if(resource_exists::<CityLandmarks>),
                ),
            )
            .add_systems(
                FixedUpdate,
                (
                    (weapons::tick_loadouts, hitscan::fire_weapons)
                        .chain()
                        .in_set(HealthSystems::Damage),
                    (pickups::collect_pickups, pickups::collect_weapon_pickups)
                        .in_set(HealthSystems::Pickup),
                    range::dummy_life.in_set(HealthSystems::Death),
                )
                    .in_set(PlayingSystems),
            );
    }
}
