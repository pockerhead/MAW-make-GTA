mod hitscan;
mod melee;
mod pickups;
mod range;
mod weapons;

pub(crate) use hitscan::unit_f32;
pub use hitscan::{
    AIM_CONFIG, AimConfig, BulletTrace, CombatRng, DamageDealt, ShotFired, aim_yaw, cone_sample,
    muzzle,
};
pub use melee::{
    HitReaction, KnockbackTuning, MELEE_CONFIG, Melee, MeleeConfig, MeleeHit, MeleeHitStats,
    MeleeWeapon, MeleeWeaponStats, Swing, advance_swing, knock_back,
};
pub use pickups::{BatPickup, Dropped, Pickup, PickupKind, WeaponPickup, dropped_gun};
pub use range::{Dummy, dummy_bundle};
pub use weapons::{
    FireMode, GunSlot, Loadout, WEAPONS_CONFIG, Weapon, WeaponsConfig, acquire, cycle_weapon,
    falloff_factor, roll_damage,
};

use crate::character::HealthSystems;
use crate::flow::{GameState, PlayingSystems};
use crate::world::CityLandmarks;
use bevy::prelude::*;
use bevy_tnua::prelude::*;

/// Id of one attack — a trigger pull or a melee swing — carried in `DamageDealt.shot`.
#[derive(Resource, Default)]
pub struct AttackSerial(u32);

impl AttackSerial {
    pub fn next_id(&mut self) -> u32 {
        self.0 = self.0.wrapping_add(1);
        self.0
    }
}

pub struct CombatPlugin {
    /// Seed of `CombatRng`.
    pub seed: u64,
}

impl Plugin for CombatPlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(CombatRng::seeded(self.seed))
            .init_resource::<AttackSerial>()
            .add_message::<ShotFired>()
            .add_message::<BulletTrace>()
            .add_message::<DamageDealt>()
            .add_message::<MeleeHit>()
            .add_message::<melee::Strike>()
            .register_type::<Pickup>()
            .register_type::<PickupKind>()
            .register_type::<Loadout>()
            .register_type::<GunSlot>()
            .register_type::<Weapon>()
            .register_type::<WeaponPickup>()
            .register_type::<Dropped>()
            .register_type::<Dummy>()
            .register_type::<ShotFired>()
            .register_type::<BulletTrace>()
            .register_type::<DamageDealt>()
            .register_type::<Melee>()
            .register_type::<Swing>()
            .register_type::<MeleeWeapon>()
            .register_type::<HitReaction>()
            .register_type::<MeleeHit>()
            .register_type::<BatPickup>()
            .add_systems(OnExit(GameState::Wasted), melee::reset_player_melee)
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
                    (
                        weapons::tick_loadouts,
                        (
                            melee::recover_from_hits,
                            melee::swing_melee,
                            melee::apply_strikes,
                        )
                            .chain()
                            .before(TnuaUserControlsSystems),
                        hitscan::fire_weapons,
                    )
                        .chain()
                        .in_set(HealthSystems::Damage),
                    (
                        pickups::collect_pickups,
                        pickups::collect_weapon_pickups,
                        pickups::expire_dropped.after(pickups::collect_weapon_pickups),
                        pickups::collect_bat_pickups,
                    )
                        .in_set(HealthSystems::Pickup),
                    range::dummy_life.in_set(HealthSystems::Death),
                )
                    .in_set(PlayingSystems),
            );
    }
}
