use super::pickups::WeaponPickup;
use super::weapons::{Weapon, WeaponsConfig};
use crate::character::{
    CharacterControlConfig, CharacterSchemeConfig, Dead, Health, HealthConfig, LocomotionConfig,
    character_components,
};
use crate::world::CityLandmarks;
use bevy::prelude::*;

/// A target dummy: a character that lies dead for `dummy_reset` s, then stands at full health.
#[derive(Component, Reflect, Default)]
#[reflect(Component)]
pub struct Dummy {
    pub reset_left: f32,
}

/// A dummy standing with its feet at `feet`.
pub fn dummy_bundle(
    loco: &LocomotionConfig,
    handle: Handle<CharacterSchemeConfig>,
    health: &HealthConfig,
    feet: Vec3,
) -> impl Bundle {
    (
        Dummy::default(),
        Name::new("Dummy"),
        Transform::from_translation(feet + Vec3::Y * loco.float_height),
        character_components(loco, handle),
        Health::full(health),
    )
}

/// Offset of item `i` of `n` in a row along X centred on 0.
fn row(i: usize, n: usize, spacing: f32) -> Vec3 {
    Vec3::X * (i as f32 - (n as f32 - 1.0) / 2.0) * spacing
}

/// Dummies in a row `dummy_distance` m along −Z from the park centre, pickups in a row through it.
pub(super) fn spawn_range(
    mut commands: Commands,
    landmarks: Res<CityLandmarks>,
    cfg: Res<WeaponsConfig>,
    loco: Res<LocomotionConfig>,
    health: Res<HealthConfig>,
    handle: Res<CharacterControlConfig>,
) {
    let c = landmarks.park_center;
    let r = &cfg.range;
    let dummies = r.dummies as usize;
    for i in 0..dummies {
        let feet = c + row(i, dummies, r.dummy_spacing) - Vec3::Z * r.dummy_distance;
        commands.spawn(dummy_bundle(&loco, handle.0.clone(), &health, feet));
    }
    let kinds: Vec<(Weapon, bool)> = Weapon::ALL
        .into_iter()
        .flat_map(|weapon| [(weapon, false), (weapon, true)])
        .collect();
    for (i, &(weapon, ammo_only)) in kinds.iter().enumerate() {
        let suffix = if ammo_only { " ammo" } else { "" };
        commands.spawn((
            WeaponPickup {
                weapon,
                ammo_only,
                cooldown: 0.0,
            },
            Name::new(format!("Weapon pickup {weapon:?}{suffix}")),
            Transform::from_translation(c + row(i, kinds.len(), r.pickup_spacing)),
        ));
    }
}

#[allow(clippy::type_complexity)]
pub(super) fn dummy_life(
    mut commands: Commands,
    cfg: Res<WeaponsConfig>,
    health_cfg: Res<HealthConfig>,
    time: Res<Time<Fixed>>,
    mut alive: Query<(Entity, &Health, &mut Dummy), Without<Dead>>,
    mut dead: Query<(Entity, &mut Health, &mut Dummy), With<Dead>>,
) {
    for (entity, health, mut dummy) in &mut alive {
        if health.current > 0.0 {
            continue;
        }
        commands.entity(entity).insert(Dead);
        dummy.reset_left = cfg.range.dummy_reset;
    }
    for (entity, mut health, mut dummy) in &mut dead {
        dummy.reset_left -= time.delta_secs();
        if dummy.reset_left > 0.0 {
            continue;
        }
        *health = Health::full(&health_cfg);
        commands.entity(entity).try_remove::<Dead>();
    }
}
