//! Witness bar: a progress bar over each civilian phoning the police (GDD §6.2). The sim owns the
//! call (`CivilianState::Report { progress, .. }`); this only draws it.

use super::{rgb, rgba};
use crate::camera::{OrbitCamera, follow_player};
use crate::menu::UiConfig;
use bevy::{camera::CameraUpdateSystems, prelude::*, ui::UiSystems};
use gta_sim::{
    character::CharacterBody,
    civilian::{Civilian, CivilianState},
};

/// Back node of the bar over `civilian`; `fill` is its coloured child.
#[derive(Component)]
pub(super) struct WitnessBar {
    pub(super) civilian: Entity,
    pub(super) fill: Entity,
}

pub struct WitnessBarPlugin;

impl Plugin for WitnessBarPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Update, sync_witness_bars).add_systems(
            PostUpdate,
            place_witness_bars
                .after(follow_player)
                .after(CameraUpdateSystems)
                .before(UiSystems::Layout),
        );
    }
}

/// Fill width in px of a call at `progress` (0..1) on a bar `width` px wide.
pub(super) fn bar_fill_width(progress: f32, width: f32) -> f32 {
    width * progress.clamp(0.0, 1.0)
}

fn call_progress(state: CivilianState) -> Option<f32> {
    match state {
        CivilianState::Report { progress, .. } => Some(progress),
        _ => None,
    }
}

/// One bar per calling civilian: spawned when the call starts, removed when it ends in any way.
fn sync_witness_bars(
    mut commands: Commands,
    ui: Res<UiConfig>,
    civilians: Query<(Entity, &Civilian)>,
    bars: Query<(Entity, &WitnessBar)>,
    mut nodes: Query<&mut Node>,
) {
    let cfg = &ui.hud.witness_bar;
    let mut shown = Vec::new();
    for (bar, witness) in &bars {
        let progress = civilians
            .get(witness.civilian)
            .ok()
            .and_then(|(_, c)| call_progress(c.state));
        let Some(progress) = progress else {
            commands.entity(bar).despawn();
            continue;
        };
        shown.push(witness.civilian);
        if let Ok(mut fill) = nodes.get_mut(witness.fill) {
            fill.width = px(bar_fill_width(progress, cfg.width));
        }
    }
    for (civilian, state) in &civilians {
        let Some(progress) = call_progress(state.state) else {
            continue;
        };
        if shown.contains(&civilian) {
            continue;
        }
        let fill = commands
            .spawn((
                Node {
                    width: px(bar_fill_width(progress, cfg.width)),
                    height: percent(100),
                    ..default()
                },
                BackgroundColor(rgb(cfg.fill_color)),
            ))
            .id();
        commands
            .spawn((
                WitnessBar { civilian, fill },
                Name::new("Witness bar"),
                Node {
                    position_type: PositionType::Absolute,
                    width: px(cfg.width),
                    height: px(cfg.height),
                    ..default()
                },
                BackgroundColor(rgba(cfg.back_color)),
                Visibility::Hidden,
            ))
            .add_child(fill);
    }
}

fn place_witness_bars(
    ui: Res<UiConfig>,
    camera: Query<(&Camera, &Transform), With<OrbitCamera>>,
    bodies: Query<(&Transform, &CharacterBody)>,
    mut bars: Query<(&WitnessBar, &mut Node, &mut Visibility)>,
) {
    let Ok((camera, transform)) = camera.single() else {
        return;
    };
    let cfg = &ui.hud.witness_bar;
    // The camera is a root entity: its Transform is this frame's pose, GlobalTransform lags a frame.
    let global = GlobalTransform::from(*transform);
    for (bar, mut node, mut visibility) in &mut bars {
        let anchor = bodies.get(bar.civilian).ok().and_then(|(body, shape)| {
            // Capsule top = head top of the model (feet at centre - float_height).
            let top = body.translation + Vec3::Y * (shape.height / 2.0 + cfg.head_offset);
            camera.world_to_viewport(&global, top).ok()
        });
        let Some(anchor) = anchor else {
            visibility.set_if_neq(Visibility::Hidden);
            continue;
        };
        node.left = px(anchor.x - cfg.width / 2.0);
        node.top = px(anchor.y - cfg.height);
        visibility.set_if_neq(Visibility::Inherited);
    }
}
