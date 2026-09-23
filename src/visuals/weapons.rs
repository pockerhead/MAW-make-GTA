//! Primitive gun and ammo boxes: pickups on the range and the gun in the player's hand.

use super::{CharacterVisualConfig, RenderConfig};
use bevy::prelude::*;
use gta_sim::{
    combat::{Loadout, Weapon, WeaponPickup},
    player::Player,
};

#[derive(Resource)]
pub(super) struct WeaponVisualAssets {
    guns: [Handle<StandardMaterial>; 3],
    ammo: Handle<StandardMaterial>,
    held: Handle<Mesh>,
    pickup: Handle<Mesh>,
    ammo_box: Handle<Mesh>,
}

fn size((x, y, z): (f32, f32, f32)) -> Vec3 {
    Vec3::new(x, y, z)
}

impl FromWorld for WeaponVisualAssets {
    fn from_world(world: &mut World) -> Self {
        let w = world.resource::<RenderConfig>().weapons.clone();
        let mut meshes = world.resource_mut::<Assets<Mesh>>();
        let held = meshes.add(Cuboid::from_size(size(w.held_size)));
        let pickup = meshes.add(Cuboid::from_size(size(w.pickup_size)));
        let ammo_box = meshes.add(Cuboid::from_length(w.ammo_size));
        let mut materials = world.resource_mut::<Assets<StandardMaterial>>();
        let mut material = |(r, g, b): (f32, f32, f32)| materials.add(Color::srgb(r, g, b));
        Self {
            guns: [
                material(w.pistol_color),
                material(w.smg_color),
                material(w.shotgun_color),
            ],
            ammo: material(w.ammo_color),
            held,
            pickup,
            ammo_box,
        }
    }
}

/// The gun box in a player's hand; shows the held weapon.
#[derive(Component)]
pub struct HeldGun {
    pub owner: Entity,
    /// Muzzle end of the barrel in the gun's own frame (presentation origin of flash and tracer).
    pub barrel: Vec3,
}

pub(super) fn visualize_weapon_pickup(
    event: On<Add, WeaponPickup>,
    pickups: Query<&WeaponPickup>,
    config: Res<RenderConfig>,
    assets: Res<WeaponVisualAssets>,
    mut commands: Commands,
) {
    let Ok(pickup) = pickups.get(event.entity) else {
        return;
    };
    let (mesh, material) = if pickup.ammo_only {
        (assets.ammo_box.clone(), assets.ammo.clone())
    } else {
        (
            assets.pickup.clone(),
            assets.guns[pickup.weapon.index()].clone(),
        )
    };
    commands.entity(event.entity).insert((
        Visibility::default(),
        children![(
            Mesh3d(mesh),
            MeshMaterial3d(material),
            Transform::from_xyz(0.0, config.pickups.lift, 0.0),
        )],
    ));
}

// Every frame, not `Changed<WeaponPickup>`: the cooldown changes every fixed tick.
pub(super) fn show_available_weapon_pickups(mut pickups: Query<(&WeaponPickup, &mut Visibility)>) {
    for (pickup, mut visibility) in &mut pickups {
        let wanted = if pickup.available() {
            Visibility::Inherited
        } else {
            Visibility::Hidden
        };
        visibility.set_if_neq(wanted);
    }
}

/// Parents the gun to the hand joint of each player's model once the glTF instance exists.
/// The joint carries the model scale, so the gun undoes it to keep its size in metres.
#[allow(clippy::too_many_arguments)]
pub(super) fn attach_held_gun(
    players: Query<Entity, With<Player>>,
    guns: Query<&HeldGun>,
    children: Query<&Children>,
    names: Query<&Name>,
    config: Res<RenderConfig>,
    visual: Res<CharacterVisualConfig>,
    assets: Res<WeaponVisualAssets>,
    mut commands: Commands,
) {
    for player in &players {
        if guns.iter().any(|gun| gun.owner == player) {
            continue;
        }
        let Some(hand) = children
            .iter_descendants(player)
            .find(|e| names.get(*e).is_ok_and(|n| n.as_str() == visual.hand_joint))
        else {
            continue;
        };
        let w = &config.weapons;
        let scale = visual.scale();
        let rotation = Quat::from_euler(
            EulerRot::YXZ,
            w.hand_yaw_deg.to_radians(),
            w.hand_pitch_deg.to_radians(),
            0.0,
        );
        commands.spawn((
            Name::new("Held gun"),
            HeldGun {
                owner: player,
                barrel: Vec3::Z * w.held_size.2 / 2.0,
            },
            ChildOf(hand),
            Mesh3d(assets.held.clone()),
            MeshMaterial3d(assets.guns[Weapon::Pistol.index()].clone()),
            Transform::from_translation(size(w.hand_offset) / scale)
                .with_rotation(rotation)
                .with_scale(Vec3::splat(1.0 / scale)),
            Visibility::Hidden,
        ));
    }
}

pub(super) fn show_held_gun(
    assets: Res<WeaponVisualAssets>,
    players: Query<&Loadout, With<Player>>,
    mut guns: Query<(
        &HeldGun,
        &mut MeshMaterial3d<StandardMaterial>,
        &mut Visibility,
    )>,
) {
    for (gun, mut material, mut visibility) in &mut guns {
        let Ok(loadout) = players.get(gun.owner) else {
            continue;
        };
        let Some(weapon) = loadout.held else {
            visibility.set_if_neq(Visibility::Hidden);
            continue;
        };
        let wanted = &assets.guns[weapon.index()];
        if material.0 != *wanted {
            material.0 = wanted.clone();
        }
        visibility.set_if_neq(Visibility::Inherited);
    }
}
