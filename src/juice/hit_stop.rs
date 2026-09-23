//! Hit-stop: the animations of attacker and target freeze for `hit_stop_seconds` of real time on a
//! melee hit. Presentation only: `Time<Virtual>` and the fixed tick are never touched (GDD §8).

use super::JuiceConfig;
use bevy::prelude::*;
use gta_sim::{character::Character, combat::MeleeHit};

/// Real seconds left of this character's animation freeze.
#[derive(Component, Default)]
pub struct HitStop {
    pub left: f32,
}

#[derive(SystemSet, Debug, Clone, PartialEq, Eq, Hash)]
pub struct HitStopSystems;

pub struct HitStopPlugin;

impl Plugin for HitStopPlugin {
    fn build(&self, app: &mut App) {
        app.add_observer(attach_hit_stop).add_systems(
            Update,
            (tick_hit_stop, start_hit_stop)
                .chain()
                .in_set(HitStopSystems),
        );
    }
}

fn attach_hit_stop(event: On<Add, Character>, mut commands: Commands) {
    commands.entity(event.entity).insert(HitStop::default());
}

fn tick_hit_stop(real: Res<Time<Real>>, mut stops: Query<&mut HitStop>) {
    let dt = real.delta_secs();
    for mut stop in &mut stops {
        if stop.left > 0.0 {
            stop.left = (stop.left - dt).max(0.0);
        }
    }
}

fn start_hit_stop(
    mut hits: MessageReader<MeleeHit>,
    juice: Res<JuiceConfig>,
    mut stops: Query<&mut HitStop>,
) {
    for hit in hits.read() {
        for entity in [hit.attacker, hit.target] {
            if let Ok(mut stop) = stops.get_mut(entity) {
                stop.left = juice.hit_stop_seconds;
            }
        }
    }
}
