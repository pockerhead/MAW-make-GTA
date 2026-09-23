//! Headless gate: the production witness-bar plugin keeps exactly one bar per calling civilian, fills
//! it by the sim's call progress and removes it when the call ends. Placement on screen needs a
//! camera and is left to the owner run.

use super::witness::{WitnessBar, WitnessBarPlugin, bar_fill_width};
use crate::menu::{UI_CONFIG, UiConfig};
use bevy::prelude::*;
use gta_sim::{
    civilian::{Civilian, CivilianState, Temperament},
    config::{ConfigRoot, load_config},
};
use std::path::Path;

fn ui() -> UiConfig {
    let root = ConfigRoot(Path::new(env!("CARGO_MANIFEST_DIR")).join("assets"));
    let cfg =
        load_config::<UiConfig>(&root, UI_CONFIG).unwrap_or_else(|e| panic!("GATE BROKEN: {e}"));
    cfg.validate()
        .unwrap_or_else(|e| panic!("GATE BROKEN: {UI_CONFIG}: {e}"));
    cfg
}

fn bars_app() -> App {
    let mut app = App::new();
    app.add_plugins(MinimalPlugins)
        .insert_resource(ui())
        .add_plugins(WitnessBarPlugin);
    app.finish();
    app.cleanup();
    app.update();
    app
}

fn civilian(state: CivilianState) -> Civilian {
    Civilian {
        state,
        temperament: Temperament {
            flee: 1.0,
            cower: 1.0,
            report: 1.0,
        },
    }
}

/// `(civilian, fill width px)` of every bar.
fn bars(app: &mut App) -> Vec<(Entity, Val)> {
    let bars = app
        .world_mut()
        .query::<&WitnessBar>()
        .iter(app.world())
        .map(|b| (b.civilian, b.fill))
        .collect::<Vec<_>>();
    bars.into_iter()
        .map(|(civilian, fill)| {
            let node = app
                .world()
                .get::<Node>(fill)
                .expect("witness bar lost its fill");
            (civilian, node.width)
        })
        .collect()
}

#[test]
fn fill_width_follows_progress() {
    let width = ui().hud.witness_bar.width;
    assert_eq!(width, 48.0, "GATE BROKEN: worked values assume 48 px");
    assert_eq!(bar_fill_width(0.0, width), 0.0);
    assert_eq!(bar_fill_width(0.5, width), 24.0);
    assert_eq!(bar_fill_width(1.2, width), 48.0);
}

#[test]
fn bar_lives_exactly_as_long_as_the_call() {
    let mut app = bars_app();
    let width = ui().hud.witness_bar.width;
    let caller = app
        .world_mut()
        .spawn(civilian(CivilianState::Report { progress: 0.5 }))
        .id();
    let walker = app.world_mut().spawn(civilian(CivilianState::Wander)).id();
    app.update();
    assert_eq!(bars(&mut app), vec![(caller, px(width * 0.5))]);
    app.world_mut().get_mut::<Civilian>(caller).unwrap().state =
        CivilianState::Report { progress: 0.75 };
    app.update();
    assert_eq!(bars(&mut app), vec![(caller, px(width * 0.75))]);
    app.world_mut().get_mut::<Civilian>(caller).unwrap().state = CivilianState::Flee {
        from: Vec3::ZERO,
        left: 10.0,
    };
    app.update();
    assert!(
        bars(&mut app).is_empty(),
        "bar kept after the call was interrupted"
    );
    app.world_mut().get_mut::<Civilian>(walker).unwrap().state =
        CivilianState::Report { progress: 0.0 };
    app.update();
    assert_eq!(bars(&mut app), vec![(walker, px(0.0))]);
    app.world_mut().entity_mut(walker).despawn();
    app.update();
    assert!(
        bars(&mut app).is_empty(),
        "bar kept after its civilian despawned"
    );
    assert_eq!(
        app.world_mut().query::<&Node>().iter(app.world()).count(),
        0,
        "bar nodes left behind"
    );
}
