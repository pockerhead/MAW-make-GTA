use super::RenderConfig;
use bevy::prelude::*;
use gta_sim::combat::{Pickup, PickupKind};

pub(super) fn visualize_pickup(
    event: On<Add, Pickup>,
    pickups: Query<&Pickup>,
    config: Res<RenderConfig>,
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    let Ok(pickup) = pickups.get(event.entity) else {
        return;
    };
    let visuals = &config.pickups;
    let (r, g, b) = match pickup.kind {
        PickupKind::Health => visuals.health_color,
        PickupKind::Armor => visuals.armor_color,
    };
    commands.entity(event.entity).insert((
        Visibility::default(),
        children![(
            Mesh3d(meshes.add(Cuboid::from_length(visuals.size))),
            MeshMaterial3d(materials.add(Color::srgb(r, g, b))),
            Transform::from_xyz(0.0, visuals.lift, 0.0),
        )],
    ));
}

// Every frame, not `Changed<Pickup>`: the cooldown changes every fixed tick.
pub(super) fn show_available_pickups(mut pickups: Query<(&Pickup, &mut Visibility)>) {
    for (pickup, mut visibility) in &mut pickups {
        let wanted = if pickup.available() {
            Visibility::Inherited
        } else {
            Visibility::Hidden
        };
        visibility.set_if_neq(wanted);
    }
}
