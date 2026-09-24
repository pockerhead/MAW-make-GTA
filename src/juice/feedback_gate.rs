//! Headless juice gates: no system in `src/` writes `Time<Virtual>` (G-J1), every trauma row fires
//! with its `juice.ron` value while time runs untouched, the hurt vignette and the recoil scale
//! (G-J2), and the damage-arc angle table (G-J3).

use super::{
    CameraRecoil, CameraShake, JUICE_CONFIG, JuiceConfig, JuicePlugin, PlayerHurt, StarsRaised,
    damage_arc::{DamageArc, arc_angle},
};
use crate::{
    camera::OrbitCamera,
    menu::{UI_CONFIG, UiConfig, UiFonts},
    settings::GameSettings,
};
use bevy::{
    asset::AssetPlugin, ecs::message::MessageCursor, post_process::effect_stack::Vignette,
    prelude::*, state::app::StatesPlugin, time::TimeUpdateStrategy,
};
use gta_sim::{
    character::Health,
    combat::{DamageDealt, MeleeHit, ShotFired, Weapon},
    compose_sim,
    config::{ConfigRoot, load_config},
    flow::GameState,
    player::{DebugDamage, Player},
    wanted::WantedLevel,
    world::WorldSource,
};
use std::{
    f32::consts::{FRAC_PI_2, PI},
    path::{Path, PathBuf},
};

fn assets_root() -> ConfigRoot {
    ConfigRoot(Path::new(env!("CARGO_MANIFEST_DIR")).join("assets"))
}

fn juice() -> JuiceConfig {
    let cfg = load_config::<JuiceConfig>(&assets_root(), JUICE_CONFIG)
        .unwrap_or_else(|e| panic!("GATE BROKEN: {e}"));
    cfg.validate()
        .unwrap_or_else(|e| panic!("GATE BROKEN: {JUICE_CONFIG}: {e}"));
    cfg
}

// Every token is split with `concat!` so this file never matches its own scan.
const FORBIDDEN: [&str; 6] = [
    concat!("ResMut<", "Time<Virtual>>"),
    concat!("ResMut<", "Time<Fixed>>"),
    concat!("ResMut<", "Time>"),
    concat!("resource_mut::<", "Time<Virtual>>"),
    concat!("resource_mut::<", "Time<Fixed>>"),
    concat!("set_relative", "_speed("),
];

/// No whitespace, no `'w,` lifetimes, no `bevy::time::` / `bevy::prelude::` prefixes.
fn normalise(line: &str) -> String {
    let compact = line
        .chars()
        .filter(|c| !c.is_whitespace())
        .collect::<String>()
        .replace("bevy::time::", "")
        .replace("bevy::prelude::", "");
    let mut out = String::with_capacity(compact.len());
    let mut rest = compact.as_str();
    while let Some(quote) = rest.find('\'') {
        out.push_str(&rest[..quote]);
        let after = &rest[quote + 1..];
        let ident = after
            .find(|c: char| !(c.is_alphanumeric() || c == '_'))
            .unwrap_or(after.len());
        if ident > 0 && after[ident..].starts_with(',') {
            rest = &after[ident + 1..];
        } else {
            out.push('\'');
            rest = after;
        }
    }
    out.push_str(rest);
    out
}

fn forbidden(line: &str) -> bool {
    let norm = normalise(line);
    FORBIDDEN.iter().any(|token| norm.contains(token))
}

fn rust_files(dir: &Path, out: &mut Vec<PathBuf>) {
    let entries = std::fs::read_dir(dir).unwrap_or_else(|e| panic!("GATE BROKEN: {dir:?}: {e}"));
    for entry in entries {
        let path = entry.expect("GATE BROKEN: read_dir entry").path();
        if path.is_dir() {
            rust_files(&path, out);
        } else if path.extension().is_some_and(|ext| ext == "rs") {
            out.push(path);
        }
    }
}

/// G-J1 (correctness): no line in the client's `src/` takes `Time<Virtual>` / `Time<Fixed>` /
/// `Time` mutably or changes the relative speed; hit-stop, shake and every other juice run on real
/// time.
#[test]
fn juice_never_writes_virtual_time() {
    for line in [
        concat!("mut t: ResMut<", "Time<Virtual>>"),
        concat!("pub t: ResMut<'w, ", "Time<Virtual>>"),
        concat!("t: ResMut<bevy::time::", "Time<Virtual>>"),
        concat!("w.resource_mut::<", "Time<Fixed>>()"),
        concat!("t.set_relative", "_speed(0.5)"),
    ] {
        assert!(forbidden(line), "scanner misses {line:?}");
    }
    for line in [
        "world.resource::<Time<Virtual>>()",
        "mut s: ResMut<TimeUpdateStrategy>",
    ] {
        assert!(!forbidden(line), "scanner flags {line:?}");
    }
    let mut files = Vec::new();
    rust_files(
        &Path::new(env!("CARGO_MANIFEST_DIR")).join("src"),
        &mut files,
    );
    assert!(
        files.len() > 10,
        "GATE BROKEN: only {} files found",
        files.len()
    );
    let mut hits = Vec::new();
    for file in files {
        let text =
            std::fs::read_to_string(&file).unwrap_or_else(|e| panic!("GATE BROKEN: {file:?}: {e}"));
        for (i, line) in text.lines().enumerate() {
            if forbidden(line) {
                hits.push(format!("{}:{}", file.display(), i + 1));
            }
        }
    }
    assert!(hits.is_empty(), "time writers in src/: {hits:#?}");
}

/// Sim composition (test area) plus the production `JuicePlugin` and a stand-in orbit camera,
/// updated until the player exists.
fn juice_app() -> App {
    let mut app = App::new();
    app.add_plugins((
        MinimalPlugins,
        TransformPlugin,
        AssetPlugin::default(),
        StatesPlugin,
    ))
    .insert_resource(TimeUpdateStrategy::FixedTimesteps(1));
    compose_sim(&mut app, assets_root(), WorldSource::TestArea)
        .unwrap_or_else(|e| panic!("GATE BROKEN: compose_sim: {e}"));
    let ui = load_config::<UiConfig>(&assets_root(), UI_CONFIG)
        .unwrap_or_else(|e| panic!("GATE BROKEN: {e}"));
    app.insert_resource(juice())
        .insert_resource(ui)
        .insert_resource(UiFonts {
            regular: Handle::default(),
            title: Handle::default(),
        })
        .insert_resource(GameSettings::default())
        .add_plugins(JuicePlugin);
    app.world_mut().spawn((
        OrbitCamera {
            yaw: 0.0,
            pitch: 0.0,
            distance: 3.8,
            pivot: None,
            aim_blend: 0.0,
            look_idle: 0.0,
        },
        Transform::default(),
    ));
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

/// One update; `Time<Virtual>` keeps speed 1 and runs, the fixed clock advances by one step.
fn step(app: &mut App) {
    let timestep = app.world().resource::<Time<Fixed>>().timestep();
    let fixed = app.world().resource::<Time<Fixed>>().elapsed();
    app.update();
    let world = app.world();
    let virt = world.resource::<Time<Virtual>>();
    assert_eq!(virt.relative_speed(), 1.0, "juice scaled Time<Virtual>");
    assert!(!virt.is_paused(), "juice paused Time<Virtual>");
    assert_eq!(
        world.resource::<Time<Fixed>>().elapsed() - fixed,
        timestep,
        "the fixed tick did not advance by exactly one step"
    );
}

fn trauma(app: &App) -> f32 {
    app.world().resource::<CameraShake>().trauma
}

/// Trauma before and after one update with `send` applied, starting from a settled 0.
fn case(app: &mut App, send: impl FnOnce(&mut World)) -> (f32, f32) {
    app.world_mut().resource_mut::<CameraShake>().trauma = 0.0;
    step(app);
    let before = trauma(app);
    send(app.world_mut());
    step(app);
    (before, trauma(app))
}

fn vignette(app: &mut App) -> f32 {
    app.world_mut()
        .query::<&Vignette>()
        .single(app.world())
        .expect("GATE BROKEN: the stand-in camera has no Vignette")
        .intensity
}

fn recoil_after_shot(app: &mut App, player: Entity, reduce: bool) -> f32 {
    app.world_mut()
        .resource_mut::<GameSettings>()
        .reduce_camera_motion = reduce;
    app.world_mut().resource_mut::<CameraRecoil>().pitch = 0.0;
    app.world_mut().write_message(ShotFired {
        shooter: player,
        weapon: Weapon::Pistol,
        muzzle: Vec3::ZERO,
        attack: 900,
    });
    step(app);
    app.world().resource::<CameraRecoil>().pitch
}

/// G-J2 (correctness + time liveness): each trauma row adds exactly its `juice.ron` value, other
/// shooters and far kills add nothing; a hit taken lights the vignette unless "no flashes";
/// "reduce camera motion" scales the recoil kick; time is never scaled or paused.
#[test]
fn trauma_rows_and_time_untouched() {
    let mut app = juice_app();
    let cfg = juice();
    let shake = cfg.shake;
    let player = find_player(&mut app).unwrap();
    let at = app.world().get::<Transform>(player).unwrap().translation;
    let npc = app
        .world_mut()
        .spawn(Transform::from_translation(at + Vec3::new(6.0, 0.0, 0.0)))
        .id();
    let dt = app
        .world()
        .resource::<Time<Fixed>>()
        .timestep()
        .as_secs_f32();
    let rises = |(before, after): (f32, f32), row: f32, name: &str| {
        let low = before + row - shake.decay_per_s * dt - 1e-4;
        assert!(
            (low..=before + row + 1e-4).contains(&after),
            "{name}: {before} -> {after}, row {row}"
        );
    };
    let flat = |(before, after): (f32, f32), name: &str| {
        assert!(after <= before + 1e-4, "{name}: {before} -> {after}");
    };
    let shot = |shooter: Entity| {
        move |w: &mut World| {
            w.write_message(ShotFired {
                shooter,
                weapon: Weapon::Pistol,
                muzzle: Vec3::ZERO,
                attack: 1,
            });
        }
    };
    rises(
        case(&mut app, shot(player)),
        shake.shot_trauma,
        "player shot",
    );
    flat(case(&mut app, shot(npc)), "NPC shot");
    let punch = case(&mut app, |w| {
        w.write_message(MeleeHit {
            attacker: player,
            target: npc,
            point: at,
            knockdown: false,
            attack: 2,
        });
    });
    rises(punch, shake.melee_trauma, "punch landed");
    let hurt = case(&mut app, |w| {
        w.write_message(DebugDamage { amount: 10.0 });
    });
    rises(hurt, shake.hurt_trauma, "hit taken");
    assert!(vignette(&mut app) > 0.0, "a hit taken lights the vignette");
    let kill = |distance: f32| {
        move |w: &mut World| {
            w.write_message(DamageDealt {
                shooter: npc,
                shot: 3,
                target: npc,
                point: at + Vec3::X * distance,
                damage: 100,
                headshot: false,
                killed: true,
            });
        }
    };
    rises(
        case(&mut app, kill(shake.death_radius - 1.0)),
        shake.death_trauma,
        "kill nearby",
    );
    flat(
        case(&mut app, kill(shake.death_radius + 1.0)),
        "kill far away",
    );

    app.world_mut().resource_mut::<GameSettings>().no_flashes = true;
    app.world_mut().write_message(DebugDamage { amount: 10.0 });
    step(&mut app);
    assert_eq!(vignette(&mut app), 0.0, "\"no flashes\" hides the vignette");
    app.world_mut().resource_mut::<GameSettings>().no_flashes = false;

    let full = recoil_after_shot(&mut app, player, false);
    let reduced = recoil_after_shot(&mut app, player, true);
    assert!(full > 0.0, "GATE BROKEN: no recoil");
    let ratio = reduced / full;
    assert!(
        (ratio - cfg.camera_motion_reduced_scale).abs() < 1e-4,
        "reduced / full recoil = {ratio}"
    );
}

fn game_state(app: &App) -> GameState {
    app.world().resource::<State<GameState>>().get().clone()
}

fn enter(app: &mut App, state: GameState) {
    app.world_mut()
        .resource_mut::<NextState<GameState>>()
        .set(state.clone());
    step(app);
    assert_eq!(game_state(app), state, "GATE BROKEN: not in {state:?}");
}

/// Messages of type `M` written since `cursor` was taken or last read.
fn count<M: Message>(app: &App, cursor: &mut MessageCursor<M>) -> usize {
    cursor.read(app.world().resource::<Messages<M>>()).count()
}

/// G-J2 respawn row (correctness): the Busted respawn heals the SAME player entity and drops its
/// armour, so the pool falls (100 + 50 -> 100); that is not a hit taken.
#[test]
fn busted_respawn_is_not_a_hurt() {
    let mut app = juice_app();
    let player = find_player(&mut app).unwrap();
    app.world_mut().get_mut::<Health>(player).unwrap().armor = 50.0;
    step(&mut app);
    let mut hurts = app
        .world()
        .resource::<Messages<PlayerHurt>>()
        .get_cursor_current();
    enter(&mut app, GameState::Busted);
    app.world_mut().resource_mut::<CameraShake>().trauma = 0.0;
    enter(&mut app, GameState::Playing);
    step(&mut app);
    let health = app.world().get::<Health>(player).unwrap();
    assert_eq!(
        health.armor, 0.0,
        "GATE BROKEN: the respawn kept the armour"
    );
    assert_eq!(count(&app, &mut hurts), 0, "the respawn fired PlayerHurt");
    assert!(trauma(&app) <= 1e-4, "respawn trauma {}", trauma(&app));
    assert_eq!(vignette(&mut app), 0.0, "the respawn lit the vignette");

    app.world_mut().write_message(DebugDamage { amount: 10.0 });
    step(&mut app);
    assert_eq!(
        count(&app, &mut hurts),
        1,
        "GATE BROKEN: a hit after the respawn"
    );
}

/// `StarsRaised` fires once per rise of `WantedLevel.stars`, never on a hold or a fall.
#[test]
fn stars_raised_once_per_rise() {
    let mut app = juice_app();
    let mut raised = app
        .world()
        .resource::<Messages<StarsRaised>>()
        .get_cursor_current();
    // Heat of 2 stars, 1 star, 2 stars again (`wanted.ron`: 40, 180).
    for (heat, stars, rises) in [(180, 2, 1), (180, 2, 0), (40, 1, 0), (180, 2, 1), (0, 0, 0)] {
        app.world_mut().resource_mut::<WantedLevel>().heat = heat;
        step(&mut app);
        let got = app.world().resource::<WantedLevel>().stars;
        assert_eq!(got, stars, "GATE BROKEN: heat {heat} gave {got} stars");
        assert_eq!(count(&app, &mut raised), rises, "heat {heat}");
    }
}

/// Damage arc (correctness): the pellets of one blast draw one arc per shooter, not one per pellet.
#[test]
fn one_arc_per_shooter_per_blast() {
    let mut app = juice_app();
    let player = find_player(&mut app).expect("GATE BROKEN: no player");
    let at = app.world().get::<Transform>(player).unwrap().translation;
    let shotgun = app
        .world_mut()
        .spawn(Transform::from_translation(at + Vec3::X * 6.0))
        .id();
    let pistol = app
        .world_mut()
        .spawn(Transform::from_translation(at - Vec3::X * 6.0))
        .id();
    for (shooter, pellets) in [(shotgun, 8), (pistol, 1)] {
        for _ in 0..pellets {
            app.world_mut().write_message(DamageDealt {
                shooter,
                shot: 77,
                target: player,
                point: at,
                damage: 0,
                headshot: false,
                killed: false,
            });
        }
    }
    step(&mut app);
    step(&mut app);
    let mut shooters = app
        .world_mut()
        .query::<&DamageArc>()
        .iter(app.world())
        .map(|arc| arc.shooter)
        .collect::<Vec<_>>();
    shooters.sort();
    let mut expected = vec![shotgun, pistol];
    expected.sort();
    assert_eq!(shooters, expected, "one arc per shooter");
}

/// G-J3 (pure math): the arc angle under three camera yaws; 0 = ahead, + = clockwise (right).
#[test]
fn arc_angle_table() {
    let o = Vec3::ZERO;
    for (yaw, attacker, angle) in [
        (0.0, Vec3::new(0.0, 0.0, -10.0), 0.0),
        (0.0, Vec3::new(10.0, 0.0, 0.0), FRAC_PI_2),
        (FRAC_PI_2, Vec3::new(-10.0, 0.0, 0.0), 0.0),
        (FRAC_PI_2, Vec3::new(0.0, 0.0, -10.0), FRAC_PI_2),
        (PI, Vec3::new(0.0, 0.0, 10.0), 0.0),
        (PI, Vec3::new(-10.0, 0.0, 0.0), FRAC_PI_2),
        (PI, Vec3::new(10.0, 0.0, 0.0), -FRAC_PI_2),
    ] {
        let got = arc_angle(yaw, o, attacker).unwrap();
        assert!((got - angle).abs() < 1e-5, "yaw {yaw}, {attacker}: {got}");
    }
    let behind = arc_angle(FRAC_PI_2, o, Vec3::new(10.0, 0.0, 0.0)).unwrap();
    assert!((behind.abs() - PI).abs() < 1e-5, "behind: {behind}");
    assert_eq!(arc_angle(0.0, o, Vec3::new(0.0, 5.0, 0.0)), None);
}
