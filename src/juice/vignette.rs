//! Red screen-edge vignette while the player is hurt (GDD §8); off with "no flashes".

use super::{JuiceConfig, PlayerHurt};
use crate::{camera::OrbitCamera, settings::GameSettings};
use bevy::{post_process::effect_stack::Vignette, prelude::*};

/// Current vignette intensity before the "no flashes" setting.
#[derive(Resource, Default)]
pub(super) struct VignetteLevel(f32);

pub(super) struct VignettePlugin;

impl Plugin for VignettePlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<VignetteLevel>()
            .register_type::<Vignette>()
            .add_observer(attach_vignette);
    }
}

fn attach_vignette(event: On<Add, OrbitCamera>, juice: Res<JuiceConfig>, mut commands: Commands) {
    let cfg = &juice.vignette;
    let (r, g, b) = cfg.color;
    commands.entity(event.entity).insert(Vignette {
        intensity: 0.0,
        color: Color::srgb(r, g, b),
        radius: cfg.radius,
        smoothness: cfg.smoothness,
        ..default()
    });
}

pub(super) fn update_vignette(
    mut hurts: MessageReader<PlayerHurt>,
    juice: Res<JuiceConfig>,
    settings: Res<GameSettings>,
    real: Res<Time<Real>>,
    mut level: ResMut<VignetteLevel>,
    mut vignettes: Query<&mut Vignette>,
) {
    let cfg = &juice.vignette;
    for _ in hurts.read() {
        level.0 = (level.0 + cfg.per_hurt).min(cfg.max);
    }
    level.0 = (level.0 - cfg.decay_per_s * real.delta_secs()).max(0.0);
    let intensity = if settings.no_flashes { 0.0 } else { level.0 };
    for mut vignette in &mut vignettes {
        if vignette.intensity != intensity {
            vignette.intensity = intensity;
        }
    }
}
