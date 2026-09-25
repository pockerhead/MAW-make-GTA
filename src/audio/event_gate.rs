//! Headless cue and loop gates over the production plugins: pellet impacts per attack, the
//! stingers and the death sting, sirens on live cops.

use super::{
    cues::{SoundBank, SoundClass, SoundStats, listener},
    gate::{SoundRow, audio_app, entity_set, mix, new_sounds, player_at, sounds, stand_in},
    loops::SirenEmitter,
    synth::Synth,
};
use crate::juice::{PlayerHurt, StarsRaised};
use bevy::{audio::Decodable, prelude::*};
use gta_sim::{
    combat::{BulletTrace, TraceHit},
    flow::GameState,
    player::DebugDamage,
    police::{CopState, PoliceCar, PoliceCarState, PoliceUnit, UnitKind},
    vehicle::VehicleImpact,
    wanted::WantedLevel,
};

/// Impacts (correctness): the pellets of one attack make one impact per kind; a second attack of
/// the same shooter in the same frame (two fixed ticks) makes its own.
#[test]
fn pellets_share_one_impact() {
    let mut app = audio_app();
    let (_, at) = player_at(&mut app);
    let hit = at + Vec3::new(3.0, 1.0, 4.0);
    let shooter = stand_in(&mut app, hit);
    let before = entity_set(&mut app);
    let pellets = [
        (50, TraceHit::Body, 8),
        (50, TraceHit::World, 2),
        (51, TraceHit::Body, 8),
    ];
    for (attack, kind, count) in pellets {
        for _ in 0..count {
            app.world_mut().write_message(BulletTrace {
                shooter,
                from: hit,
                to: hit,
                hit: kind,
                attack,
            });
        }
    }
    app.update();
    let impacts = new_sounds(&mut app, &before, SoundClass::Impact);
    assert_eq!(impacts.len(), 3, "one impact per (attack, kind)");
}

/// Crashes (correctness): a car-car crash comes as one `VehicleImpact` per car at the same point and
/// makes one impact sound; a crash at another point makes its own.
#[test]
fn car_crash_is_one_impact() {
    let mut app = audio_app();
    let (_, at) = player_at(&mut app);
    let hit = at + Vec3::new(3.0, 1.0, 4.0);
    let (a, b) = (stand_in(&mut app, hit), stand_in(&mut app, hit));
    let before = entity_set(&mut app);
    for (vehicle, point) in [(a, hit), (b, hit), (a, hit + Vec3::X)] {
        app.world_mut().write_message(VehicleImpact {
            vehicle,
            point,
            speed: 10.0,
        });
    }
    app.update();
    let impacts = new_sounds(&mut app, &before, SoundClass::Impact);
    assert_eq!(impacts.len(), 2, "one impact per crash point");
}

fn hurt_cues(app: &App) -> u64 {
    app.world().resource::<SoundStats>().spawned[SoundClass::Hurt.index()]
}

/// Hurt cue (correctness): under a hit every update the thud plays at most once per
/// `hurt.min_interval`; after a quiet gap the next hit plays at once.
#[test]
fn hurt_cue_keeps_its_min_interval() {
    let mut app = audio_app();
    let interval = mix().hurt.min_interval;
    let dt = app
        .world()
        .resource::<Time<Fixed>>()
        .timestep()
        .as_secs_f32();
    // 60 updates of 1/64 s under a 0.5 s interval: cues at update 0 and 32 (update 64 is past the end).
    assert_eq!(
        (interval / dt, dt),
        (32.0, 1.0 / 64.0),
        "GATE BROKEN: rows assume a 0.5 s interval at 64 Hz"
    );
    let before = hurt_cues(&app);
    for _ in 0..60 {
        app.world_mut().write_message(PlayerHurt);
        app.update();
    }
    assert_eq!(hurt_cues(&app) - before, 2, "hurt cues under 60 hits");
    for _ in 0..40 {
        app.update();
    }
    let before = hurt_cues(&app);
    app.world_mut().write_message(PlayerHurt);
    app.update();
    assert_eq!(hurt_cues(&app) - before, 1, "a hit after a quiet gap");
}

fn state(app: &App) -> GameState {
    app.world().resource::<State<GameState>>().get().clone()
}

fn alive_of(app: &mut App, class: SoundClass) -> Vec<SoundRow> {
    sounds(app)
        .into_iter()
        .filter(|row| row.1 == class)
        .collect()
}

/// Stingers (correctness): a rise plays the wanted stinger; a wanted stinger in the frame after
/// death does not steal the death sting.
#[test]
fn stingers_keep_the_death_sting() {
    let mut app = audio_app();
    let wanted = app.world().resource::<SoundBank>().wanted.id();
    let death = app.world().resource::<SoundBank>().death_sting.id();
    let before = entity_set(&mut app);
    app.world_mut().write_message(StarsRaised);
    app.update();
    let stings = new_sounds(&mut app, &before, SoundClass::Stinger);
    assert_eq!(stings.len(), 1, "one stinger per StarsRaised");
    assert_eq!(stings[0].5, wanted);

    app.world_mut()
        .write_message(DebugDamage { amount: 10_000.0 });
    for _ in 0..10 {
        app.update();
        if state(&app) == GameState::Wasted {
            break;
        }
    }
    assert_eq!(state(&app), GameState::Wasted, "GATE BROKEN: no death");
    app.world_mut().write_message(StarsRaised);
    app.update();
    let stingers = alive_of(&mut app, SoundClass::Stinger);
    let wanted_newest = stingers.iter().max_by_key(|row| row.2).map(|row| row.5);
    assert_eq!(
        wanted_newest,
        Some(wanted),
        "GATE BROKEN: no second stinger"
    );
    let deaths = alive_of(&mut app, SoundClass::DeathSting);
    let ids = deaths.iter().map(|row| row.5).collect::<Vec<_>>();
    assert_eq!(
        ids,
        vec![death],
        "the death sting survives a wanted stinger"
    );
}

fn cop(app: &mut App, at: Vec3, state: CopState) -> Entity {
    let unit = PoliceUnit {
        kind: UnitKind::Patrol,
        state,
        sees: false,
        dest: None,
        dest_clear: false,
        avoid: 0.0,
        trigger_left: 0.0,
        reposition: None,
        search_point: None,
    };
    app.world_mut()
        .spawn((unit, Transform::from_translation(at)))
        .id()
}

/// Sirens (correctness): while wanted the `max_emitters` nearest live cops carry a siren child, a
/// dead cop never does, each siren starts at its own point of the sweep; 0 stars removes them.
#[test]
fn sirens_ride_live_cops() {
    let mut app = audio_app();
    let mix = mix();
    let (_, at) = player_at(&mut app);
    app.world_mut()
        .spawn((listener(0.3), Transform::from_translation(at)));
    app.world_mut().resource_mut::<WantedLevel>().heat = 180;
    app.update();
    assert_eq!(
        app.world().resource::<WantedLevel>().stars,
        2,
        "GATE BROKEN: heat 180 is not 2 stars"
    );
    let dead = cop(&mut app, at + Vec3::X * 2.0, CopState::Dead);
    let near = cop(&mut app, at + Vec3::X * 4.0, CopState::Respond);
    let next = cop(&mut app, at + Vec3::Z * 6.0, CopState::Respond);
    let far = cop(&mut app, at + Vec3::Z * 8.0, CopState::Respond);
    let mut sirens = Vec::new();
    for _ in 0..80 {
        app.update();
        sirens = app
            .world_mut()
            .query_filtered::<(&ChildOf, &AudioPlayer<Synth>, &Transform), With<SirenEmitter>>()
            .iter(app.world())
            .map(|(parent, player, transform)| {
                (parent.parent(), player.0.clone(), transform.translation)
            })
            .collect::<Vec<_>>();
        if !sirens.is_empty() {
            break;
        }
    }
    let unit = app
        .world()
        .get::<PoliceUnit>(dead)
        .expect("GATE BROKEN: dead cop gone");
    assert_eq!(
        unit.state,
        CopState::Dead,
        "GATE BROKEN: the dead cop revived"
    );
    assert_eq!(
        mix.siren.max_emitters, 2,
        "GATE BROKEN: rows assume 2 emitters"
    );
    let mut parents = sirens.iter().map(|s| s.0).collect::<Vec<_>>();
    parents.sort();
    let mut expected = vec![near, next];
    expected.sort();
    assert_eq!(parents, expected, "dead {dead}, far {far}");
    for (_, _, translation) in &sirens {
        assert_eq!(*translation, Vec3::Y * mix.siren.height);
    }
    let synths = app.world().resource::<Assets<Synth>>();
    let start = |handle: &Handle<Synth>| {
        synths
            .get(handle)
            .expect("GATE BROKEN: siren synth missing")
            .decoder()
            .take(4410)
            .collect::<Vec<_>>()
    };
    assert_ne!(
        start(&sirens[0].1),
        start(&sirens[1].1),
        "two sirens wail in unison"
    );

    app.world_mut().resource_mut::<WantedLevel>().heat = 0;
    app.update();
    app.update();
    assert!(
        alive_of(&mut app, SoundClass::Siren).is_empty(),
        "sirens outlive the wanted level"
    );
}

/// Sirens on police cars (T15, correctness): a responding police car in earshot carries the siren
/// (at the roof, `siren.car_height`) instead of the cops on foot; once the car is gone the sirens move
/// to the live cops.
#[test]
fn sirens_ride_police_cars() {
    let mut app = audio_app();
    let mix = mix();
    let (_, at) = player_at(&mut app);
    app.world_mut()
        .spawn((listener(0.3), Transform::from_translation(at)));
    app.world_mut().resource_mut::<WantedLevel>().heat = 180;
    app.update();
    assert_eq!(
        app.world().resource::<WantedLevel>().stars,
        2,
        "GATE BROKEN: heat 180 is not 2 stars"
    );
    let near = cop(&mut app, at + Vec3::X * 4.0, CopState::Respond);
    let next = cop(&mut app, at + Vec3::Z * 6.0, CopState::Respond);
    let car = app
        .world_mut()
        .spawn((
            PoliceCar {
                state: PoliceCarState::Respond,
                crew: vec![UnitKind::Patrol, UnitKind::Patrol],
                stopped: 0.0,
                moving: 0.0,
                blocked: 0.0,
                reboard_left: 10.0,
            },
            Transform::from_translation(at + Vec3::Z * 30.0),
        ))
        .id();
    let carriers = |app: &mut App| {
        app.world_mut()
            .query_filtered::<(&ChildOf, &Transform), With<SirenEmitter>>()
            .iter(app.world())
            .map(|(parent, transform)| (parent.parent(), transform.translation))
            .collect::<Vec<_>>()
    };
    let mut sirens = Vec::new();
    for _ in 0..80 {
        app.update();
        sirens = carriers(&mut app);
        if !sirens.is_empty() {
            break;
        }
    }
    assert_eq!(
        sirens,
        vec![(car, Vec3::Y * mix.siren.car_height)],
        "cops {near} {next}"
    );
    app.world_mut().entity_mut(car).despawn();
    let mut parents = Vec::new();
    for _ in 0..200 {
        app.update();
        parents = carriers(&mut app).iter().map(|s| s.0).collect::<Vec<_>>();
        if parents.len() == 2 {
            break;
        }
    }
    parents.sort();
    let mut expected = vec![near, next];
    expected.sort();
    assert_eq!(parents, expected, "the sirens did not move to the cops");
}

/// Siren of each cop: `(cop, siren entity)`, sorted by cop.
fn siren_owners(app: &mut App) -> Vec<(Entity, Entity)> {
    let mut owners = app
        .world_mut()
        .query_filtered::<(Entity, &ChildOf), With<SirenEmitter>>()
        .iter(app.world())
        .map(|(siren, parent)| (parent.parent(), siren))
        .collect::<Vec<_>>();
    owners.sort();
    owners
}

/// Siren hysteresis (correctness): a carrier keeps the very same siren entity (its sweep is not
/// restarted) against a cop closer by less than `siren.switch_margin`, and yields to one closer by
/// more.
#[test]
fn sirens_hold_their_cop() {
    let mut app = audio_app();
    let mix = mix();
    let (_, at) = player_at(&mut app);
    app.world_mut()
        .spawn((listener(0.3), Transform::from_translation(at)));
    app.world_mut().resource_mut::<WantedLevel>().heat = 180;
    app.update();
    assert_eq!(
        (mix.siren.max_emitters, mix.siren.switch_margin),
        (2, 10.0),
        "GATE BROKEN: rows assume 2 emitters and a 10 m margin"
    );
    let a = cop(&mut app, at + Vec3::X * 2.0, CopState::Respond);
    let b = cop(&mut app, at + Vec3::Z * 14.0, CopState::Respond);
    let c = cop(&mut app, at - Vec3::X * 30.0, CopState::Respond);
    // One repick is 1 s = 64 updates; 70 covers at least one.
    let repick = |app: &mut App| {
        for _ in 0..70 {
            app.update();
        }
        siren_owners(app)
    };
    let start = repick(&mut app);
    let owners = start.iter().map(|o| o.0).collect::<Vec<_>>();
    let mut expected = vec![a, b];
    expected.sort();
    assert_eq!(owners, expected, "GATE BROKEN: first pick, c {c}");

    // c at 8 m: closer than b (14 m) by 6 < 10.
    app.world_mut().get_mut::<Transform>(c).unwrap().translation = at - Vec3::X * 8.0;
    assert_eq!(
        repick(&mut app),
        start,
        "a siren left its cop for one 6 m closer"
    );

    // c at 3 m: closer than b by 11 > 10; a keeps its siren entity.
    app.world_mut().get_mut::<Transform>(c).unwrap().translation = at - Vec3::X * 3.0;
    let after = repick(&mut app);
    let kept_a = start.iter().find(|o| o.0 == a).copied();
    assert!(
        after.contains(&kept_a.unwrap()),
        "a lost its siren: {after:?}"
    );
    let owners = after.iter().map(|o| o.0).collect::<Vec<_>>();
    let mut expected = vec![a, c];
    expected.sort();
    assert_eq!(
        owners, expected,
        "the siren did not move to a cop 11 m closer"
    );
}
