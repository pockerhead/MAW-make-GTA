//! Endless beds (GDD §8): city noise and park birds cross-faded by the distance to the nearest park,
//! sirens on the nearest live cops while the player is wanted. Loops pause with the game.

use super::{
    MixConfig,
    config::AmbienceConfig,
    cues::{Sound, SoundBank, SoundClass, SoundStats, spawn_sound},
};
use bevy::{
    audio::{GlobalVolume, SpatialScale, Volume},
    prelude::*,
};
use gta_sim::{
    flow::GameState,
    player::Player,
    police::{CopState, PoliceUnit},
    wanted::WantedLevel,
    world::{City, contains_convex, dist_point_segment},
};

/// Which ambience bed an entity plays.
#[derive(Component, Clone, Copy, PartialEq, Eq, Debug)]
pub enum AmbienceBed {
    City,
    Park,
}

/// Current bed gains; read by QA.
#[derive(Resource, Reflect, Default)]
#[reflect(Resource)]
pub struct AmbienceMix {
    pub city: f32,
    pub park: f32,
}

/// A siren sound parented to a cop.
#[derive(Component)]
pub struct SirenEmitter;

pub struct SoundLoopsPlugin;

impl Plugin for SoundLoopsPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<AmbienceMix>()
            .register_type::<AmbienceMix>()
            .add_systems(Startup, spawn_ambience)
            .add_systems(
                Update,
                (
                    update_ambience,
                    update_sirens.run_if(
                        in_state(GameState::Playing)
                            .or_else(in_state(GameState::Wasted))
                            .or_else(in_state(GameState::Busted)),
                    ),
                    revolume_sirens.run_if(resource_changed::<GlobalVolume>),
                    sync_loop_pause,
                ),
            );
    }
}

/// 0 inside the counter-clockwise convex `poly`, else the distance to its nearest edge.
pub fn distance_to_polygon(p: Vec2, poly: &[Vec2]) -> f32 {
    if contains_convex(poly, p, 0.0) {
        return 0.0;
    }
    (0..poly.len())
        .map(|i| dist_point_segment(p, poly[i], poly[(i + 1) % poly.len()]))
        .fold(f32::INFINITY, f32::min)
}

/// 1 inside a park, falling linearly to 0 at `fade` metres outside it.
pub fn park_weight(distance: f32, fade: f32) -> f32 {
    (1.0 - distance / fade).max(0.0)
}

/// `(city, park)` bed gains at park weight `park`.
pub fn ambience_gains(park: f32, cfg: &AmbienceConfig) -> (f32, f32) {
    (
        cfg.city_volume * (1.0 - cfg.city_duck_in_park * park),
        cfg.park_volume * park,
    )
}

/// Up to `max` cops within `audible` of the listener. A cop in `carried` keeps its siren while it
/// stays in the list and in range; free slots go to the nearest others (ties by entity bits); a
/// carrier yields only to a cop closer to the listener by more than `margin`.
pub fn pick_sirens(
    listener: Vec3,
    cops: &[(Entity, Vec3)],
    carried: &[Entity],
    max: usize,
    audible: f32,
    margin: f32,
) -> Vec<Entity> {
    let mut near = cops
        .iter()
        .map(|&(cop, at)| (listener.distance(at), cop))
        .filter(|&(d, _)| d <= audible)
        .collect::<Vec<_>>();
    near.sort_by(|a, b| a.0.total_cmp(&b.0).then(a.1.to_bits().cmp(&b.1.to_bits())));
    let (mut kept, others): (Vec<_>, Vec<_>) =
        near.into_iter().partition(|(_, cop)| carried.contains(cop));
    kept.truncate(max);
    for challenger in others {
        if kept.len() < max {
            kept.push(challenger);
            continue;
        }
        let Some((i, farthest)) = kept
            .iter()
            .copied()
            .enumerate()
            .max_by(|a, b| a.1.0.total_cmp(&b.1.0))
        else {
            break;
        };
        if challenger.0 + margin >= farthest.0 {
            break;
        }
        kept[i] = challenger;
    }
    kept.into_iter().map(|(_, cop)| cop).collect()
}

// Session-lived, not `CityScoped`: silent until `update_ambience` raises the gains.
fn spawn_ambience(
    mut commands: Commands,
    mut bank: ResMut<SoundBank>,
    mut stats: ResMut<SoundStats>,
) {
    for (bed, source) in [
        (AmbienceBed::City, bank.city.clone()),
        (AmbienceBed::Park, bank.birds.clone()),
    ] {
        spawn_sound(
            &mut commands,
            &mut bank,
            &mut stats,
            SoundClass::Ambience,
            (
                bed,
                AudioPlayer(source),
                PlaybackSettings::ONCE.with_volume(Volume::Linear(0.0)),
            ),
        );
    }
}

#[allow(clippy::too_many_arguments)]
fn update_ambience(
    mix: Res<MixConfig>,
    state: Res<State<GameState>>,
    city: Option<Res<City>>,
    players: Query<&Transform, With<Player>>,
    global: Res<GlobalVolume>,
    mut ambience: ResMut<AmbienceMix>,
    mut beds: Query<(&AmbienceBed, &mut AudioSink)>,
) {
    let cfg = &mix.ambience;
    let park = match (city, players.single()) {
        (Some(city), Ok(player)) => {
            let p = player.translation.xz();
            city.0
                .blocks
                .iter()
                .filter(|block| block.is_park)
                .map(|block| park_weight(distance_to_polygon(p, &block.curb), cfg.park_fade))
                .fold(0.0, f32::max)
        }
        _ => 0.0,
    };
    let (city_gain, park_gain) = match state.get() {
        GameState::Playing | GameState::Wasted | GameState::Busted => ambience_gains(park, cfg),
        _ => (0.0, 0.0),
    };
    if ambience.city != city_gain || ambience.park != park_gain {
        *ambience = AmbienceMix {
            city: city_gain,
            park: park_gain,
        };
    }
    for (bed, mut sink) in &mut beds {
        let gain = match bed {
            AmbienceBed::City => city_gain,
            AmbienceBed::Park => park_gain,
        };
        // `set_volume` replaces the sink volume, so the global factor is applied here again.
        sink.set_volume(Volume::Linear(gain) * global.volume);
    }
}

#[allow(clippy::too_many_arguments, clippy::type_complexity)]
fn update_sirens(
    mut commands: Commands,
    mix: Res<MixConfig>,
    real: Res<Time<Real>>,
    wanted: Res<WantedLevel>,
    listeners: Query<&GlobalTransform, With<SpatialListener>>,
    cops: Query<(Entity, &GlobalTransform, &PoliceUnit)>,
    sirens: Query<(Entity, &ChildOf), With<SirenEmitter>>,
    mut bank: ResMut<SoundBank>,
    mut stats: ResMut<SoundStats>,
    mut repick_in: Local<f32>,
) {
    if wanted.stars == 0 {
        for (siren, _) in &sirens {
            commands.entity(siren).try_despawn();
        }
        *repick_in = 0.0;
        return;
    }
    *repick_in -= real.delta_secs();
    if *repick_in > 0.0 {
        return;
    }
    let cfg = &mix.siren;
    *repick_in = cfg.repick_seconds;
    let Some(ear) = listeners.iter().next().map(GlobalTransform::translation) else {
        return;
    };
    let live = cops
        .iter()
        .filter(|(_, _, unit)| !matches!(unit.state, CopState::Dead | CopState::Leave))
        .map(|(cop, at, _)| (cop, at.translation()))
        .collect::<Vec<_>>();
    let current = sirens
        .iter()
        .map(|(_, parent)| parent.parent())
        .collect::<Vec<_>>();
    let picked = pick_sirens(
        ear,
        &live,
        &current,
        cfg.max_emitters,
        cfg.audible,
        cfg.switch_margin,
    );
    let mut carried = Vec::new();
    for (siren, parent) in &sirens {
        if picked.contains(&parent.parent()) {
            carried.push(parent.parent());
        } else {
            commands.entity(siren).try_despawn();
        }
    }
    for cop in picked.into_iter().filter(|cop| !carried.contains(cop)) {
        let settings = PlaybackSettings::ONCE
            .with_spatial(true)
            .with_spatial_scale(SpatialScale::new(1.0 / cfg.ref_distance))
            .with_volume(Volume::Linear(cfg.volume));
        let source = AudioPlayer(bank.next_siren());
        spawn_sound(
            &mut commands,
            &mut bank,
            &mut stats,
            SoundClass::Siren,
            (
                SirenEmitter,
                ChildOf(cop),
                Transform::from_xyz(0.0, cfg.height, 0.0),
                source,
                settings,
            ),
        );
    }
}

fn revolume_sirens(
    mix: Res<MixConfig>,
    global: Res<GlobalVolume>,
    mut sinks: Query<&mut SpatialAudioSink, With<SirenEmitter>>,
) {
    for mut sink in &mut sinks {
        sink.set_volume(Volume::Linear(mix.siren.volume) * global.volume);
    }
}

fn sync_loop_pause(
    state: Res<State<GameState>>,
    flat: Query<(&Sound, &AudioSink)>,
    spatial: Query<(&Sound, &SpatialAudioSink)>,
) {
    let paused = *state.get() == GameState::Paused;
    let loops = |sound: &Sound| {
        matches!(
            sound.class,
            SoundClass::Ambience | SoundClass::Siren | SoundClass::Engine
        )
    };
    let sinks = flat
        .iter()
        .filter(|(sound, _)| loops(sound))
        .map(|(_, sink)| sink as &dyn AudioSinkPlayback)
        .chain(
            spatial
                .iter()
                .filter(|(sound, _)| loops(sound))
                .map(|(_, sink)| sink as &dyn AudioSinkPlayback),
        );
    for sink in sinks {
        if paused && !sink.is_paused() {
            sink.pause();
        } else if !paused && sink.is_paused() {
            sink.play();
        }
    }
}
