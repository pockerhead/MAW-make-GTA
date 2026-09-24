//! One-shot cues (GDD §8): shots, impacts, the player's hurt thud, wanted and death stingers, menu
//! clicks. Every sound goes through `spawn_sound`; a per-class budget steals the oldest voice.

use super::{MixConfig, synth::Synth};
use crate::{
    camera::OrbitCamera,
    juice::{PlayerHurt, StarsRaised},
    menu::MenuAction,
};
use bevy::{
    audio::{SpatialScale, Volume},
    prelude::*,
};
use gta_sim::{
    combat::{BulletTrace, DamageDealt, MeleeHit, ShotFired, TraceHit, Weapon},
    player::Player,
};
use std::collections::HashSet;

#[derive(Reflect, Clone, Copy, PartialEq, Eq, Debug)]
pub enum SoundClass {
    Shot,
    Impact,
    Hurt,
    Ui,
    Stinger,
    /// Own class: a wanted stinger in the same frame must not steal it.
    DeathSting,
    Siren,
    Ambience,
}

impl SoundClass {
    pub const COUNT: usize = 8;
    const ONE_SHOTS: [Self; 6] = [
        Self::Shot,
        Self::Impact,
        Self::Hurt,
        Self::Ui,
        Self::Stinger,
        Self::DeathSting,
    ];

    pub fn index(self) -> usize {
        self as usize
    }
}

/// A sound entity; `serial` orders voices by age.
#[derive(Component, Reflect)]
#[reflect(Component)]
pub struct Sound {
    pub class: SoundClass,
    pub serial: u64,
}

/// Per `SoundClass::index`: sounds ever spawned and the most alive after the budget; read by QA.
#[derive(Resource, Reflect, Default)]
#[reflect(Resource)]
pub struct SoundStats {
    pub spawned: [u64; SoundClass::COUNT],
    pub peak_alive: [u32; SoundClass::COUNT],
}

/// Files of one cue, played round-robin.
pub struct Pool {
    pub handles: Vec<Handle<AudioSource>>,
    next: usize,
}

impl Pool {
    fn load(assets: &AssetServer, paths: &[String]) -> Self {
        Self {
            handles: paths.iter().map(|path| assets.load(path.clone())).collect(),
            next: 0,
        }
    }

    fn pick(&mut self) -> Option<Handle<AudioSource>> {
        let handle = self
            .handles
            .get(self.next % self.handles.len().max(1))?
            .clone();
        self.next = self.next.wrapping_add(1);
        Some(handle)
    }
}

/// Every sound handle of the mix; loading the handles also preloads the files.
#[derive(Resource)]
pub struct SoundBank {
    /// By `Weapon::index`.
    pub shot: [Handle<Synth>; 3],
    /// One per siren emitter, each starting at its own point of the sweep.
    pub sirens: Vec<Handle<Synth>>,
    pub city: Handle<Synth>,
    pub birds: Handle<Synth>,
    pub bullet_body: Pool,
    pub bullet_world: Pool,
    pub punch: Pool,
    pub heavy: Pool,
    pub death: Pool,
    pub hurt: Pool,
    pub press: Handle<AudioSource>,
    pub pause: Handle<AudioSource>,
    pub wanted: Handle<AudioSource>,
    pub death_sting: Handle<AudioSource>,
    next_siren: usize,
    next_serial: u64,
}

impl FromWorld for SoundBank {
    fn from_world(world: &mut World) -> Self {
        let mix = world.resource::<MixConfig>().clone();
        let mut synths = world.resource_mut::<Assets<Synth>>();
        let shot = Weapon::ALL.map(|weapon| synths.add(Synth::shot(&mix.shot.sound(weapon))));
        // Starts spread over the sweep, so two cops never wail in unison.
        let emitters = mix.siren.max_emitters;
        let sirens = (0..emitters)
            .map(|i| {
                let start = mix.siren.period * i as f32 / emitters as f32;
                synths.add(Synth::siren(&mix.siren, start))
            })
            .collect();
        let city = synths.add(Synth::city(&mix.ambience));
        let birds = synths.add(Synth::birds(&mix.ambience.birds));
        let assets = world.resource::<AssetServer>();
        let i = &mix.impacts;
        Self {
            shot,
            sirens,
            city,
            birds,
            bullet_body: Pool::load(assets, &i.bullet_body),
            bullet_world: Pool::load(assets, &i.bullet_world),
            punch: Pool::load(assets, &i.punch),
            heavy: Pool::load(assets, &i.heavy),
            death: Pool::load(assets, &i.death),
            hurt: Pool::load(assets, &mix.hurt.pool),
            press: assets.load(mix.interface.press.clone()),
            pause: assets.load(mix.interface.pause.clone()),
            wanted: assets.load(mix.stinger.wanted.clone()),
            death_sting: assets.load(mix.stinger.death.clone()),
            next_siren: 0,
            next_serial: 0,
        }
    }
}

impl SoundBank {
    /// The next siren start, round-robin.
    pub fn next_siren(&mut self) -> Handle<Synth> {
        let siren = self.sirens[self.next_siren % self.sirens.len()].clone();
        self.next_siren = self.next_siren.wrapping_add(1);
        siren
    }
}

/// The only way a sound entity is spawned: stamps its age and counts it.
pub fn spawn_sound(
    commands: &mut Commands,
    bank: &mut SoundBank,
    stats: &mut SoundStats,
    class: SoundClass,
    bundle: impl Bundle,
) -> Entity {
    let serial = bank.next_serial;
    bank.next_serial += 1;
    stats.spawned[class.index()] += 1;
    commands.spawn((Sound { class, serial }, bundle)).id()
}

/// Gain at `distance` under rodio's `(1/dist²).min(1)` with `spatial_scale = 1/ref_distance`.
pub fn spatial_gain(distance: f32, ref_distance: f32) -> f32 {
    if distance <= ref_distance {
        1.0
    } else {
        (ref_distance / distance).powi(2)
    }
}

fn spatial(ref_distance: f32) -> PlaybackSettings {
    PlaybackSettings::DESPAWN
        .with_spatial(true)
        .with_spatial_scale(SpatialScale::new(1.0 / ref_distance))
}

/// Camera listener with `gap` metres between the ears.
pub fn listener(gap: f32) -> SpatialListener {
    // rodio 0.22.2 Spatial swaps the ear gains (spatial.rs:57-60); mirrored ears restore left/right.
    SpatialListener {
        left_ear_offset: Vec3::X * gap / 2.0,
        right_ear_offset: Vec3::X * gap / -2.0,
    }
}

/// Loud enough at the listener; everything is audible without one.
fn audible(ear: Option<Vec3>, at: Vec3, ref_distance: f32, min_gain: f32) -> bool {
    ear.is_none_or(|ear| spatial_gain(ear.distance(at), ref_distance) >= min_gain)
}

fn ear(listeners: &Query<&GlobalTransform, With<SpatialListener>>) -> Option<Vec3> {
    listeners.iter().next().map(GlobalTransform::translation)
}

#[derive(SystemSet, Debug, Clone, PartialEq, Eq, Hash)]
pub struct SoundCues;

pub struct SoundCuesPlugin;

impl Plugin for SoundCuesPlugin {
    fn build(&self, app: &mut App) {
        app.add_message::<PlayerHurt>()
            .add_message::<StarsRaised>()
            .init_resource::<SoundStats>()
            .init_resource::<SoundBank>()
            .register_type::<SoundClass>()
            .register_type::<Sound>()
            .register_type::<SoundStats>()
            .register_type::<SpatialListener>()
            .add_observer(attach_listener)
            .add_systems(
                Update,
                (
                    (
                        play_shots,
                        play_impacts,
                        play_hurt,
                        play_stinger,
                        play_ui_press,
                    )
                        .in_set(SoundCues),
                    enforce_voice_budget.after(SoundCues),
                ),
            )
            .add_systems(OnEnter(gta_sim::flow::GameState::Paused), play_pause_sound)
            .add_systems(OnEnter(gta_sim::flow::GameState::Wasted), play_death_sting);
    }
}

fn attach_listener(event: On<Add, OrbitCamera>, mix: Res<MixConfig>, mut commands: Commands) {
    commands
        .entity(event.entity)
        .insert(listener(mix.listener_ear_gap));
}

#[allow(clippy::too_many_arguments)]
fn play_shots(
    mut commands: Commands,
    mut shots: MessageReader<ShotFired>,
    mix: Res<MixConfig>,
    players: Query<(), With<Player>>,
    listeners: Query<&GlobalTransform, With<SpatialListener>>,
    mut bank: ResMut<SoundBank>,
    mut stats: ResMut<SoundStats>,
) {
    let ear = ear(&listeners);
    let variants = &mix.shot.pitch_variants;
    for shot in shots.read() {
        let volume = Volume::Linear(mix.shot.sound(shot.weapon).volume);
        let speed = variants[shot.attack as usize % variants.len()];
        let source = AudioPlayer(bank.shot[shot.weapon.index()].clone());
        if players.contains(shot.shooter) {
            let settings = PlaybackSettings::DESPAWN
                .with_speed(speed)
                .with_volume(volume);
            spawn_sound(
                &mut commands,
                &mut bank,
                &mut stats,
                SoundClass::Shot,
                (source, settings),
            );
            continue;
        }
        let reference = mix.shot.npc_ref_distance;
        if !audible(ear, shot.muzzle, reference, mix.min_gain) {
            continue;
        }
        let settings = spatial(reference).with_speed(speed).with_volume(volume);
        spawn_sound(
            &mut commands,
            &mut bank,
            &mut stats,
            SoundClass::Shot,
            (source, settings, Transform::from_translation(shot.muzzle)),
        );
    }
}

#[allow(clippy::too_many_arguments)]
fn play_impacts(
    mut commands: Commands,
    mut traces: MessageReader<BulletTrace>,
    mut melee: MessageReader<MeleeHit>,
    mut dealt: MessageReader<DamageDealt>,
    mix: Res<MixConfig>,
    listeners: Query<&GlobalTransform, With<SpatialListener>>,
    mut bank: ResMut<SoundBank>,
    mut stats: ResMut<SoundStats>,
    // One impact per (attack, hit a body): a shotgun blast is one thud, not ten.
    mut heard: Local<HashSet<(u32, bool)>>,
) {
    heard.clear();
    let ear = ear(&listeners);
    let cfg = &mix.impacts;
    let mut hits = Vec::new();
    for trace in traces.read() {
        let body = match trace.hit {
            TraceHit::Nothing => continue,
            TraceHit::World => false,
            TraceHit::Body => true,
        };
        if heard.insert((trace.attack, body)) {
            let pool = if body {
                &mut bank.bullet_body
            } else {
                &mut bank.bullet_world
            };
            hits.push((pool.pick(), trace.to));
        }
    }
    for hit in melee.read() {
        let pool = if hit.knockdown {
            &mut bank.heavy
        } else {
            &mut bank.punch
        };
        hits.push((pool.pick(), hit.point));
    }
    for hit in dealt.read().filter(|hit| hit.killed) {
        hits.push((bank.death.pick(), hit.point));
    }
    for (handle, at) in hits {
        let Some(handle) = handle else {
            continue;
        };
        if !audible(ear, at, cfg.ref_distance, mix.min_gain) {
            continue;
        }
        let settings = spatial(cfg.ref_distance).with_volume(Volume::Linear(cfg.volume));
        spawn_sound(
            &mut commands,
            &mut bank,
            &mut stats,
            SoundClass::Impact,
            (
                AudioPlayer(handle),
                settings,
                Transform::from_translation(at),
            ),
        );
    }
}

/// A non-spatial one-shot at `volume`.
fn flat(handle: Handle<AudioSource>, volume: f32) -> impl Bundle {
    (
        AudioPlayer(handle),
        PlaybackSettings::DESPAWN.with_volume(Volume::Linear(volume)),
    )
}

fn play_hurt(
    mut commands: Commands,
    mut hurts: MessageReader<PlayerHurt>,
    mix: Res<MixConfig>,
    real: Res<Time<Real>>,
    mut bank: ResMut<SoundBank>,
    mut stats: ResMut<SoundStats>,
    mut last: Local<Option<f32>>,
) {
    let now = real.elapsed_secs();
    for _ in hurts.read() {
        if last.is_some_and(|at| now - at < mix.hurt.min_interval) {
            continue;
        }
        *last = Some(now);
        let Some(handle) = bank.hurt.pick() else {
            continue;
        };
        let bundle = flat(handle, mix.hurt.volume);
        spawn_sound(
            &mut commands,
            &mut bank,
            &mut stats,
            SoundClass::Hurt,
            bundle,
        );
    }
}

fn play_stinger(
    mut commands: Commands,
    mut raised: MessageReader<StarsRaised>,
    mix: Res<MixConfig>,
    mut bank: ResMut<SoundBank>,
    mut stats: ResMut<SoundStats>,
) {
    for _ in raised.read() {
        let bundle = flat(bank.wanted.clone(), mix.stinger.volume);
        spawn_sound(
            &mut commands,
            &mut bank,
            &mut stats,
            SoundClass::Stinger,
            bundle,
        );
    }
}

fn play_ui_press(
    mut commands: Commands,
    buttons: Query<&Interaction, (Changed<Interaction>, With<MenuAction>)>,
    mix: Res<MixConfig>,
    mut bank: ResMut<SoundBank>,
    mut stats: ResMut<SoundStats>,
) {
    for interaction in &buttons {
        if *interaction != Interaction::Pressed {
            continue;
        }
        let bundle = flat(bank.press.clone(), mix.interface.volume);
        spawn_sound(&mut commands, &mut bank, &mut stats, SoundClass::Ui, bundle);
    }
}

fn play_pause_sound(
    mut commands: Commands,
    mix: Res<MixConfig>,
    mut bank: ResMut<SoundBank>,
    mut stats: ResMut<SoundStats>,
) {
    let bundle = flat(bank.pause.clone(), mix.interface.volume);
    spawn_sound(&mut commands, &mut bank, &mut stats, SoundClass::Ui, bundle);
}

fn play_death_sting(
    mut commands: Commands,
    mix: Res<MixConfig>,
    mut bank: ResMut<SoundBank>,
    mut stats: ResMut<SoundStats>,
) {
    let bundle = flat(bank.death_sting.clone(), mix.stinger.volume);
    spawn_sound(
        &mut commands,
        &mut bank,
        &mut stats,
        SoundClass::DeathSting,
        bundle,
    );
}

/// Keeps at most `voices.<class>` one-shots of each class alive by despawning the oldest; records
/// the peak alive count of every class.
fn enforce_voice_budget(
    mut commands: Commands,
    sounds: Query<(Entity, &Sound)>,
    mix: Res<MixConfig>,
    mut stats: ResMut<SoundStats>,
) {
    let mut alive: [Vec<(u64, Entity)>; SoundClass::COUNT] = Default::default();
    for (entity, sound) in &sounds {
        alive[sound.class.index()].push((sound.serial, entity));
    }
    let v = &mix.voices;
    for class in SoundClass::ONE_SHOTS {
        let cap = match class {
            SoundClass::Shot => v.shot,
            SoundClass::Impact => v.impact,
            SoundClass::Hurt => v.hurt,
            SoundClass::Ui => v.ui,
            SoundClass::Stinger => v.stinger,
            SoundClass::DeathSting => v.death_sting,
            SoundClass::Siren | SoundClass::Ambience => continue,
        };
        let voices = &mut alive[class.index()];
        if voices.len() <= cap {
            continue;
        }
        voices.sort_unstable();
        for (_, entity) in voices.drain(..voices.len() - cap) {
            commands.entity(entity).try_despawn();
        }
    }
    for (peak, voices) in stats.peak_alive.iter_mut().zip(&alive) {
        *peak = (*peak).max(voices.len() as u32);
    }
}
