//! Headless gate: the production damage-number plugin shows exactly the sim's damage (one sum per
//! shot and target), marks headshots with the CRIT label, colour and size, and despawns every
//! label after its lifetime.
//! Placement on screen needs a camera and is left to the runtime scenario (t6.py).

use super::{
    JUICE_CONFIG, JuiceConfig,
    damage_numbers::{DamageNumber, DamageNumbersPlugin, pose},
};
use crate::menu::{UI_CONFIG, UiConfig, UiFonts};
use bevy::{prelude::*, time::TimeUpdateStrategy};
use gta_sim::{
    combat::DamageDealt,
    config::{ConfigRoot, load_config},
    player::Player,
};
use std::{path::Path, time::Duration};

const FRAME: f32 = 1.0 / 60.0;

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

fn ui() -> UiConfig {
    load_config::<UiConfig>(&assets_root(), UI_CONFIG)
        .unwrap_or_else(|e| panic!("GATE BROKEN: {e}"))
}

/// Production damage-number plugin with a stand-in `DamageDealt` source and one player shooter.
fn numbers_app() -> (App, Entity) {
    let mut app = App::new();
    app.add_plugins(MinimalPlugins)
        .insert_resource(TimeUpdateStrategy::ManualDuration(Duration::from_secs_f32(
            FRAME,
        )))
        .add_message::<DamageDealt>()
        .insert_resource(juice())
        .insert_resource(ui())
        .insert_resource(UiFonts {
            regular: Handle::default(),
            title: Handle::default(),
        })
        .add_plugins(DamageNumbersPlugin);
    app.finish();
    app.cleanup();
    app.update();
    let player = app.world_mut().spawn(Player).id();
    (app, player)
}

fn hit(shooter: Entity, shot: u32, damage: u32, headshot: bool) -> DamageDealt {
    DamageDealt {
        shooter,
        shot,
        target: Entity::PLACEHOLDER,
        point: Vec3::new(1.0, 1.5, -3.0),
        damage,
        headshot,
        killed: false,
    }
}

fn count(app: &mut App) -> usize {
    app.world_mut()
        .query::<&DamageNumber>()
        .iter(app.world())
        .count()
}

fn px_size(font: &TextFont) -> f32 {
    match font.font_size {
        FontSize::Px(size) => size,
        other => panic!("font size is not in px: {other:?}"),
    }
}

#[test]
fn label_matches_message() {
    let (mut app, player) = numbers_app();
    let cfg = juice().damage_numbers;
    app.world_mut().write_message(hit(player, 1, 27, false));
    app.world_mut().write_message(hit(player, 2, 54, true));
    app.update();
    let mut labels = app
        .world_mut()
        .query::<(&DamageNumber, &Text, &TextColor, &TextFont)>()
        .iter(app.world())
        .map(|(n, t, c, f)| (n.value, n.headshot, t.0.clone(), c.0, px_size(f)))
        .collect::<Vec<_>>();
    labels.sort_by_key(|l| l.0);
    let (r, g, b) = cfg.color;
    let (cr, cg, cb) = cfg.crit_color;
    assert_eq!(
        labels,
        vec![
            (27, false, "27".to_string(), Color::srgb(r, g, b), cfg.size),
            (
                54,
                true,
                "54 CRIT".to_string(),
                Color::srgb(cr, cg, cb),
                cfg.crit_size
            ),
        ]
    );
}

#[test]
fn numbers_despawn_after_lifetime() {
    let (mut app, player) = numbers_app();
    let lifetime = juice().damage_numbers.lifetime;
    for frame in 0..20 {
        for k in 0..10 {
            app.world_mut().write_message(hit(
                player,
                frame * 10 + k,
                10 + k,
                (frame + k) % 3 == 0,
            ));
        }
        app.update();
    }
    let alive = count(&mut app);
    assert!(alive > 0, "no damage numbers spawned (liveness)");
    let frames = ((lifetime + 0.1) / FRAME).ceil() as u32;
    for _ in 0..frames {
        app.update();
    }
    assert_eq!(
        count(&mut app),
        0,
        "damage numbers leaked ({alive} were alive)"
    );
}

#[test]
fn non_player_hits_spawn_nothing() {
    let (mut app, _player) = numbers_app();
    let npc = app.world_mut().spawn_empty().id();
    app.world_mut().write_message(hit(npc, 1, 30, true));
    app.update();
    assert_eq!(count(&mut app), 0);
}

/// Shotgun: the pellets of one blast on one target make one label with their sum (CRIT if any
/// pellet hit the head); another target or another blast gets its own label.
#[test]
fn pellets_show_one_sum_per_shot_and_target() {
    let (mut app, player) = numbers_app();
    let other = app.world_mut().spawn_empty().id();
    let on = |target: Entity, mut h: DamageDealt| {
        h.target = target;
        h
    };
    for h in [
        hit(player, 7, 8, false),
        hit(player, 7, 9, true),
        on(other, hit(player, 7, 9, false)),
        hit(player, 7, 8, false),
        hit(player, 8, 7, false),
    ] {
        app.world_mut().write_message(h);
    }
    app.update();
    let mut labels = app
        .world_mut()
        .query::<(&DamageNumber, &Text)>()
        .iter(app.world())
        .map(|(n, t)| (n.value, n.headshot, t.0.clone()))
        .collect::<Vec<_>>();
    labels.sort();
    assert_eq!(
        labels,
        vec![
            (7, false, "7".to_string()),
            (9, false, "9".to_string()),
            (25, true, "25 CRIT".to_string()),
        ]
    );
}

#[test]
fn pose_worked_examples() {
    let cfg = juice().damage_numbers;
    assert_eq!(
        (
            cfg.lifetime,
            cfg.pop_seconds,
            cfg.pop_start_scale,
            cfg.rise_px,
            cfg.drift_px,
            cfg.fade_start
        ),
        (0.8, 0.08, 1.8, 60.0, 24.0, 0.5),
        "GATE BROKEN: the worked examples assume the shipped juice.ron"
    );
    for (age, offset, scale, alpha) in [
        (0.0, Vec2::ZERO, 1.8, 1.0),
        (0.04, Vec2::new(1.2, -8.5575), 1.2, 1.0),
        (0.4, Vec2::new(12.0, -52.5), 1.0, 1.0),
        (0.6, Vec2::new(18.0, -59.0625), 1.0, 0.5),
        (0.8, Vec2::new(24.0, -60.0), 1.0, 0.0),
    ] {
        let (got_offset, got_scale, got_alpha) = pose(age, 1.0, &cfg);
        assert!(
            (got_offset - offset).length() < 1e-3
                && (got_scale - scale).abs() < 1e-3
                && (got_alpha - alpha).abs() < 1e-3,
            "age {age}: {got_offset} {got_scale} {got_alpha}"
        );
    }
}
