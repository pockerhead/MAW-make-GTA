//! Headless audio gates (G-A1..G-A7): the mix names manifest `.ogg` files that decode, the voice
//! budget holds, emitters are placed and pooled per event, loops are endless `Once` decoders, the
//! gain/pick tables, and the pan side through rodio's real `Spatial`. No `AudioPlugin`: there is no
//! device, sinks never form, so one-shots never finish on their own (the budget alone bounds them).

use super::{
    MIX_CONFIG, MixConfig,
    cues::{Sound, SoundBank, SoundClass, SoundCuesPlugin, SoundStats, listener, spatial_gain},
    loops::{SoundLoopsPlugin, ambience_gains, distance_to_polygon, park_weight, pick_sirens},
    synth::Synth,
};
use crate::{juice::PlayerHurt, settings::GameSettings};
use bevy::{
    asset::AssetPlugin,
    audio::{ChannelCount, Decodable, GlobalVolume, PlaybackMode, SampleRate, Source},
    prelude::*,
    state::app::StatesPlugin,
    time::TimeUpdateStrategy,
};
use gta_sim::{
    combat::{BulletTrace, MeleeHit, ShotFired, TraceHit, Weapon},
    compose_sim,
    config::{
        ConfigRoot, load_config,
        manifest::{THIRD_PARTY_MANIFEST, ThirdPartyManifest},
    },
    flow::GameState,
    player::{DebugDamage, Player},
    world::WorldSource,
};
use std::{
    collections::HashSet,
    panic::{AssertUnwindSafe, catch_unwind},
    path::Path,
};

fn assets_root() -> ConfigRoot {
    ConfigRoot(Path::new(env!("CARGO_MANIFEST_DIR")).join("assets"))
}

pub(super) fn mix() -> MixConfig {
    let mix = load_config::<MixConfig>(&assets_root(), MIX_CONFIG)
        .unwrap_or_else(|e| panic!("GATE BROKEN: {e}"));
    mix.validate()
        .unwrap_or_else(|e| panic!("GATE BROKEN: {MIX_CONFIG}: {e}"));
    mix
}

fn manifest() -> ThirdPartyManifest {
    load_config::<ThirdPartyManifest>(&assets_root(), THIRD_PARTY_MANIFEST)
        .unwrap_or_else(|e| panic!("GATE BROKEN: {e}"))
}

/// Sim composition (test area) plus the production cue and loop plugins, updated until the player exists.
pub(super) fn audio_app() -> App {
    let mut app = App::new();
    app.add_plugins((
        MinimalPlugins,
        TransformPlugin,
        AssetPlugin::default(),
        StatesPlugin,
    ))
    .insert_resource(TimeUpdateStrategy::FixedTimesteps(1))
    // Stand-ins for what `AudioPlugin` registers.
    .init_asset::<Synth>()
    .init_asset::<AudioSource>()
    .init_resource::<GlobalVolume>();
    compose_sim(&mut app, assets_root(), WorldSource::TestArea)
        .unwrap_or_else(|e| panic!("GATE BROKEN: compose_sim: {e}"));
    app.insert_resource(mix())
        .insert_resource(GameSettings::default())
        .add_plugins((SoundCuesPlugin, SoundLoopsPlugin));
    app.finish();
    app.cleanup();
    for _ in 0..20 {
        app.update();
        if find_player(&mut app).is_some() {
            return app;
        }
    }
    panic!("GATE BROKEN: no Player after 20 updates");
}

fn find_player(app: &mut App) -> Option<Entity> {
    app.world_mut()
        .query_filtered::<Entity, With<Player>>()
        .single(app.world())
        .ok()
}

pub(super) fn player_at(app: &mut App) -> (Entity, Vec3) {
    let player = find_player(app).expect("GATE BROKEN: expected exactly one Player");
    let at = app.world().get::<Transform>(player).unwrap().translation;
    (player, at)
}

pub(super) type SoundRow = (
    Entity,
    SoundClass,
    u64,
    PlaybackSettings,
    Option<Vec3>,
    AssetId<AudioSource>,
);

/// Every sound entity: class, serial, settings, position, and its `AudioSource` id (default for synths).
pub(super) fn sounds(app: &mut App) -> Vec<SoundRow> {
    app.world_mut()
        .query::<(
            Entity,
            &Sound,
            &PlaybackSettings,
            Option<&Transform>,
            Option<&AudioPlayer>,
        )>()
        .iter(app.world())
        .map(|(e, s, p, t, a)| {
            (
                e,
                s.class,
                s.serial,
                *p,
                t.map(|t| t.translation),
                a.map_or(AssetId::default(), |a| a.0.id()),
            )
        })
        .collect()
}

pub(super) fn new_sounds(
    app: &mut App,
    before: &HashSet<Entity>,
    class: SoundClass,
) -> Vec<SoundRow> {
    sounds(app)
        .into_iter()
        .filter(|row| row.1 == class && !before.contains(&row.0))
        .collect()
}

pub(super) fn entity_set(app: &mut App) -> HashSet<Entity> {
    sounds(app).into_iter().map(|row| row.0).collect()
}

fn stats(app: &App) -> &SoundStats {
    app.world().resource::<SoundStats>()
}

pub(super) fn stand_in(app: &mut App, at: Vec3) -> Entity {
    app.world_mut().spawn(Transform::from_translation(at)).id()
}

/// G-A1 (correctness): every mix sound is a listed `.ogg`, pools are non-empty; each defect has its
/// own message.
#[test]
fn mix_sounds_are_manifest_oggs() {
    let manifest = manifest();
    mix()
        .check_sounds(&manifest)
        .unwrap_or_else(|e| panic!("shipped mix: {e:?}"));
    let expect = |mutate: fn(&mut MixConfig), keyword: &str| {
        let mut cfg = mix();
        mutate(&mut cfg);
        let errors = cfg.check_sounds(&manifest).expect_err(keyword);
        assert!(
            errors.iter().any(|e| e.contains(keyword)),
            "{keyword}: {errors:?}"
        );
    };
    expect(
        |c| c.impacts.punch[0] = "third_party/impact-sounds/notInTheManifest.ogg".into(),
        "is not listed",
    );
    expect(
        |c| c.stinger.death = c.stinger.death.replace(".ogg", ".wav"),
        "must be an .ogg",
    );
    expect(|c| c.hurt.pool.clear(), "pool is empty");
}

/// Decodes `bytes` fully on the production decoder; `Err` on a panic or a bad stream.
fn decodes(bytes: &[u8]) -> Result<(u16, usize), String> {
    catch_unwind(AssertUnwindSafe(|| {
        let decoder = AudioSource {
            bytes: bytes.into(),
        }
        .decoder();
        (decoder.channels().get(), decoder.count())
    }))
    .map_err(|_| "decoder panicked".to_string())
}

/// G-A2 (correctness): every `.ogg` the mix plays decodes on the production decoder (bevy's
/// `AudioSource::decoder` unwraps: a bad file panics at play time).
#[test]
fn mix_oggs_decode() {
    let mix = mix();
    let root = assets_root();
    let packs = ["impact-sounds", "interface-sounds", "music-jingles"];
    if packs
        .iter()
        .any(|pack| !root.path(&format!("third_party/{pack}")).is_dir())
    {
        eprintln!("SKIP decode check: packs not fetched; run python tools/fetch_assets.py");
        return;
    }
    let mut first = None;
    for path in mix.sound_paths() {
        let bytes =
            std::fs::read(root.path(path)).unwrap_or_else(|e| panic!("GATE BROKEN: {path}: {e}"));
        let (channels, samples) = decodes(&bytes).unwrap_or_else(|e| panic!("{path}: {e}"));
        assert!(matches!(channels, 1 | 2), "{path}: {channels} channels");
        assert!(samples > 0, "{path}: no samples");
        first.get_or_insert(bytes);
    }
    let bytes = first.expect("GATE BROKEN: the mix names no files");
    assert!(
        decodes(&bytes[..100]).is_err(),
        "a truncated file must not decode"
    );
}

/// G-A3 (correctness): 30 NPC shots and 30 wall impacts per update keep exactly `voices.shot` /
/// `voices.impact` alive, the newest ones, while `SoundStats` counts every spawn.
#[test]
fn voice_budget_holds() {
    let mut app = audio_app();
    let mix = mix();
    let (_, player) = player_at(&mut app);
    let positions = (0..30)
        .map(|i| {
            let a = i as f32 / 30.0 * std::f32::consts::TAU;
            player + Vec3::new(20.0 * a.cos(), 1.0, 20.0 * a.sin())
        })
        .collect::<Vec<_>>();
    let shooters = positions
        .iter()
        .map(|&at| stand_in(&mut app, at))
        .collect::<Vec<_>>();
    let before_shot = stats(&app).spawned[SoundClass::Shot.index()];
    let before_impact = stats(&app).spawned[SoundClass::Impact.index()];
    for k in 1..=4u64 {
        for (i, (&shooter, &at)) in shooters.iter().zip(&positions).enumerate() {
            let world = app.world_mut();
            world.write_message(ShotFired {
                shooter,
                weapon: Weapon::Smg,
                muzzle: at,
                attack: (k * 100) as u32 + i as u32,
            });
            world.write_message(BulletTrace {
                shooter,
                from: at,
                to: at,
                hit: TraceHit::World,
                attack: (k * 100) as u32 + i as u32,
            });
        }
        app.update();
        let all = sounds(&mut app);
        for (class, cap) in [
            (SoundClass::Shot, mix.voices.shot),
            (SoundClass::Impact, mix.voices.impact),
        ] {
            let mut alive = all
                .iter()
                .filter(|row| row.1 == class)
                .map(|row| (row.2, row.4.expect("spatial one-shot has a Transform")))
                .collect::<Vec<_>>();
            alive.sort_by_key(|row| row.0);
            assert_eq!(alive.len(), cap, "update {k}: alive {class:?}");
            // Written in shooter order, so the newest `cap` sit at the last `cap` positions.
            let newest = &positions[30 - cap..];
            let at = alive.iter().map(|row| row.1).collect::<Vec<_>>();
            assert_eq!(
                at, newest,
                "update {k}: {class:?} survivors are not the newest"
            );
        }
        let s = stats(&app);
        assert_eq!(s.spawned[SoundClass::Shot.index()] - before_shot, 30 * k);
        assert_eq!(
            s.spawned[SoundClass::Impact.index()] - before_impact,
            30 * k
        );
        assert_eq!(
            s.peak_alive[SoundClass::Shot.index()],
            mix.voices.shot as u32
        );
        assert_eq!(
            s.peak_alive[SoundClass::Impact.index()],
            mix.voices.impact as u32
        );
    }
}

fn scale_of(settings: &PlaybackSettings) -> Vec3 {
    settings
        .spatial_scale
        .map(|s| s.0)
        .expect("spatial one-shot has a scale")
}

/// G-A4 (correctness): each event spawns its sound at the right place, spatial or not, from the
/// right pool; the death sting plays once on `Wasted`.
#[test]
fn emitters_are_placed() {
    let mut app = audio_app();
    let mix = mix();
    let (player, at) = player_at(&mut app);
    let muzzle = at + Vec3::new(5.0, 1.2, -3.0);
    let npc = stand_in(&mut app, muzzle);

    let before = entity_set(&mut app);
    app.world_mut().write_message(ShotFired {
        shooter: npc,
        weapon: Weapon::Pistol,
        muzzle,
        attack: 7,
    });
    app.update();
    let shots = new_sounds(&mut app, &before, SoundClass::Shot);
    assert_eq!(shots.len(), 1, "NPC shot");
    let (_, _, _, settings, translation, _) = shots[0];
    assert!(settings.spatial, "an NPC shot is spatial");
    assert_eq!(translation, Some(muzzle));
    let scale = scale_of(&settings);
    let expected = Vec3::splat(1.0 / mix.shot.npc_ref_distance);
    assert!((scale - expected).abs().max_element() < 1e-6, "{scale}");

    let before = entity_set(&mut app);
    app.world_mut().write_message(ShotFired {
        shooter: player,
        weapon: Weapon::Pistol,
        muzzle: at,
        attack: 8,
    });
    app.update();
    let shots = new_sounds(&mut app, &before, SoundClass::Shot);
    assert_eq!(shots.len(), 1, "player shot");
    assert!(!shots[0].3.spatial, "the player's own shot is not spatial");

    let wall = at + Vec3::new(-4.0, 1.0, 2.0);
    let before = entity_set(&mut app);
    app.world_mut().write_message(BulletTrace {
        shooter: npc,
        from: muzzle,
        to: wall,
        hit: TraceHit::World,
        attack: 7,
    });
    app.update();
    let impacts = new_sounds(&mut app, &before, SoundClass::Impact);
    assert_eq!(impacts.len(), 1, "wall impact");
    assert_eq!(impacts[0].4, Some(wall));
    let bank_ids = |pick: fn(&SoundBank) -> &super::cues::Pool, app: &App| {
        pick(app.world().resource::<SoundBank>())
            .handles
            .iter()
            .map(Handle::id)
            .collect::<Vec<_>>()
    };
    assert!(bank_ids(|b| &b.bullet_world, &app).contains(&impacts[0].5));

    let before = entity_set(&mut app);
    app.world_mut().write_message(MeleeHit {
        attacker: npc,
        target: player,
        point: at,
        knockdown: true,
        attack: 9,
    });
    app.update();
    let impacts = new_sounds(&mut app, &before, SoundClass::Impact);
    assert_eq!(impacts.len(), 1, "knockdown punch");
    assert!(bank_ids(|b| &b.heavy, &app).contains(&impacts[0].5));

    let before = entity_set(&mut app);
    app.world_mut().write_message(PlayerHurt);
    app.update();
    let hurts = new_sounds(&mut app, &before, SoundClass::Hurt);
    assert_eq!(hurts.len(), 1, "hurt thud");
    assert!(!hurts[0].3.spatial, "the player's own body is not spatial");
    assert!(bank_ids(|b| &b.hurt, &app).contains(&hurts[0].5));

    let before = entity_set(&mut app);
    app.world_mut()
        .write_message(DebugDamage { amount: 10_000.0 });
    for _ in 0..10 {
        app.update();
        if *app.world().resource::<State<GameState>>().get() == GameState::Wasted {
            break;
        }
    }
    assert_eq!(
        *app.world().resource::<State<GameState>>().get(),
        GameState::Wasted,
        "GATE BROKEN: the player did not die"
    );
    app.update();
    let stings = new_sounds(&mut app, &before, SoundClass::DeathSting);
    assert_eq!(stings.len(), 1, "death sting");
    let death = app.world().resource::<SoundBank>().death_sting.id();
    assert_eq!(stings[0].5, death, "the death sting plays the death jingle");
}

/// G-A5 (correctness): loops never end and are played `Once` (rodio `Loop` would buffer them).
#[test]
fn loops_are_endless_once() {
    let mix = mix();
    for synth in [
        Synth::siren(&mix.siren, 0.0),
        Synth::city(&mix.ambience),
        Synth::birds(&mix.ambience.birds),
        Synth::engine(&mix.engine),
    ] {
        assert!(synth.is_endless());
        assert_eq!(synth.decoder().total_duration(), None);
        assert_eq!(synth.decoder().take(441_000).count(), 441_000);
    }
    let shot = Synth::shot(&mix.shot.pistol);
    assert!(!shot.is_endless());
    assert!(shot.decoder().total_duration().is_some());

    let mut app = audio_app();
    let ambience = sounds(&mut app)
        .into_iter()
        .filter(|row| row.1 == SoundClass::Ambience)
        .collect::<Vec<_>>();
    assert_eq!(ambience.len(), 2, "city and park beds");
    for row in ambience {
        assert!(
            matches!(row.3.mode, PlaybackMode::Once),
            "a bed must be played Once, got {:?}",
            row.3.mode
        );
    }
}

fn close(a: f32, b: f32) -> bool {
    (a - b).abs() < 1e-5
}

/// G-A6 (pure math): gains, polygon distance, park weight and siren pick.
#[test]
fn pure_tables() {
    for (d, r, g) in [
        (4.0, 8.0, 1.0),
        (8.0, 8.0, 1.0),
        (16.0, 8.0, 0.25),
        (24.0, 8.0, 1.0 / 9.0),
    ] {
        assert!(close(spatial_gain(d, r), g), "gain({d}, {r})");
    }
    let square = [
        Vec2::new(0.0, 0.0),
        Vec2::new(10.0, 0.0),
        Vec2::new(10.0, 10.0),
        Vec2::new(0.0, 10.0),
    ];
    for (p, d) in [((5.0, 5.0), 0.0), ((15.0, 5.0), 5.0), ((13.0, 14.0), 5.0)] {
        let got = distance_to_polygon(Vec2::new(p.0, p.1), &square);
        assert!(close(got, d), "{p:?}: {got}");
    }
    for (d, w) in [(0.0, 1.0), (12.5, 0.5), (30.0, 0.0)] {
        assert!(close(park_weight(d, 25.0), w), "park_weight({d})");
    }
    let cfg = mix().ambience;
    let (city, park) = ambience_gains(1.0, &cfg);
    assert!(
        close(city, 0.25 * 0.5) && close(park, 0.35),
        "{city} {park}"
    );
    let (city, park) = ambience_gains(0.0, &cfg);
    assert!(close(city, 0.25) && close(park, 0.0), "{city} {park}");

    let mut world = World::new();
    let [a, b, c] = [(); 3].map(|_| world.spawn_empty().id());
    let cops = [
        (c, Vec3::new(200.0, 0.0, 0.0)),
        (a, Vec3::new(10.0, 0.0, 0.0)),
        (b, Vec3::new(0.0, 0.0, 30.0)),
    ];
    assert_eq!(
        pick_sirens(Vec3::ZERO, &cops, &[], 2, 150.0, 10.0),
        vec![a, b]
    );
    assert_eq!(pick_sirens(Vec3::ZERO, &cops, &[], 1, 150.0, 10.0), vec![a]);
    let far = cops.map(|(cop, at)| (cop, at + Vec3::X * 200.0));
    assert_eq!(
        pick_sirens(Vec3::ZERO, &far, &[], 2, 150.0, 10.0),
        Vec::<Entity>::new()
    );
    let (low, high) = if a.to_bits() < b.to_bits() {
        (a, b)
    } else {
        (b, a)
    };
    let tie = [(high, Vec3::X * 20.0), (low, Vec3::Z * 20.0)];
    assert_eq!(
        pick_sirens(Vec3::ZERO, &tie, &[], 1, 150.0, 10.0),
        vec![low]
    );
    let tie = [tie[1], tie[0]];
    assert_eq!(
        pick_sirens(Vec3::ZERO, &tie, &[], 1, 150.0, 10.0),
        vec![low]
    );
    // Hysteresis, margin 10 m: the carrier b at 30 m keeps its siren against a at 25 m, yields to
    // a at 10 m, is dropped beyond `audible`, and a free slot goes to the nearest other.
    let hold = [(a, Vec3::X * 25.0), (b, Vec3::Z * 30.0)];
    assert_eq!(
        pick_sirens(Vec3::ZERO, &hold, &[b], 1, 150.0, 10.0),
        vec![b]
    );
    assert_eq!(
        pick_sirens(Vec3::ZERO, &cops, &[b], 1, 150.0, 10.0),
        vec![a]
    );
    assert_eq!(
        pick_sirens(Vec3::ZERO, &cops, &[c], 1, 150.0, 10.0),
        vec![a]
    );
    assert_eq!(
        pick_sirens(Vec3::ZERO, &cops, &[b], 2, 150.0, 10.0),
        vec![b, a]
    );
}

/// Mean |sample| of (left, right) of a mono 0.5 tone through rodio's `Spatial`, ears and emitter
/// placed and scaled as `bevy_audio` does (`audio_output.rs`).
fn channel_means(
    listener: &SpatialListener,
    cam: Transform,
    emitter: Vec3,
    scale: f32,
) -> (f32, f32) {
    let camera = GlobalTransform::from(cam);
    let left = camera.transform_point(listener.left_ear_offset) * scale;
    let right = camera.transform_point(listener.right_ear_offset) * scale;
    let source = rodio::buffer::SamplesBuffer::new(
        ChannelCount::new(1).unwrap(),
        SampleRate::new(44_100).unwrap(),
        vec![0.5 as rodio::Sample; 64],
    );
    let samples =
        rodio::source::Spatial::new(source, (emitter * scale).into(), left.into(), right.into())
            .take(128)
            .collect::<Vec<_>>();
    let mean = |parity: usize| {
        let channel = samples
            .iter()
            .skip(parity)
            .step_by(2)
            .map(|s| s.abs())
            .collect::<Vec<_>>();
        channel.iter().sum::<f32>() / channel.len() as f32
    };
    (mean(0), mean(1))
}

/// G-A7 (correctness, upgrade tripwire): with the production listener a source on the camera's
/// right is louder in the right channel. When a Bevy upgrade brings a rodio with fixed ear gains this
/// goes RED: remove the mirror from `listener()`, not this gate.
#[test]
fn listener_pans_right_to_right() {
    let ears = listener(0.3);
    let scale = 1.0 / 8.0;
    let yaw90 = Transform::from_rotation(Quat::from_rotation_y(std::f32::consts::FRAC_PI_2));
    for (name, cam, emitter, right_louder) in [
        (
            "yaw 0, source right",
            Transform::IDENTITY,
            Vec3::new(10.0, 0.0, 0.0),
            true,
        ),
        (
            "yaw 0, source left",
            Transform::IDENTITY,
            Vec3::new(-10.0, 0.0, 0.0),
            false,
        ),
        (
            "yaw 90, source right",
            yaw90,
            Vec3::new(0.0, 0.0, -10.0),
            true,
        ),
    ] {
        let (left, right) = channel_means(&ears, cam, emitter, scale);
        assert!(
            (right > left) == right_louder,
            "{name}: left {left}, right {right}"
        );
    }
}
