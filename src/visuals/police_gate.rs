//! Headless gates of the police looks (real GLBs): every police model animates from its own glTF
//! clips, and patrol and SWAT bodies carry their `character/visual.ron` tints.

use super::{
    character::CharacterAnimations,
    civilian_gate::{animator_graphs, glb_app, leg_turns, models, visual_config, wait_wired},
    gang_gate::{StandardMaterialStandIn, source_base_color, tinted_base_color},
};
use bevy::{gltf::extensions::GltfExtensionHandlers, prelude::*, world_serialization::WorldAsset};
use gta_sim::{
    character::{CharacterControlConfig, Gait, HealthConfig, LocomotionConfig, MoveIntent},
    combat::WeaponsConfig,
    police::{EscalationConfig, UnitKind, police_unit_bundle},
    population::Appearance,
};

/// One patrol cop and one SWAT unit per police model (`Appearance(k)` picks model `k`) on the test
/// floor 2 m apart; no sidewalk graph, so the police AI is off.
fn spawn_units(app: &mut App) -> Vec<(Entity, UnitKind, usize)> {
    let world = app.world();
    let loco = world.resource::<LocomotionConfig>().clone();
    let health = world.resource::<HealthConfig>().clone();
    let weapons = world.resource::<WeaponsConfig>().clone();
    let esc = world.resource::<EscalationConfig>().clone();
    let handle = world.resource::<CharacterControlConfig>().0.clone();
    let models = visual_config().police_models.len();
    (0..models)
        .flat_map(|k| [(k, UnitKind::Patrol), (k, UnitKind::Swat)])
        .enumerate()
        .map(|(i, (k, kind))| {
            let feet = Vec3::new(-6.0 + 2.0 * i as f32, 0.0, -20.0);
            let bundle = police_unit_bundle(
                &loco,
                handle.clone(),
                &health,
                &weapons,
                esc.spec(kind),
                kind,
                feet,
                0.0,
                Appearance(k as u32),
            );
            (app.world_mut().spawn(bundle).id(), kind, k)
        })
        .collect()
}

/// Updates until no world asset event arrives for 4 updates in a row: models re-instance while
/// scenes stream in, and a re-instance replaces the tinted meshes.
fn settle_world_assets(app: &mut App) {
    let mut events = app
        .world()
        .resource::<Messages<AssetEvent<WorldAsset>>>()
        .get_cursor();
    let mut quiet = 0;
    for _ in 0..600 {
        app.update();
        let messages = app.world().resource::<Messages<AssetEvent<WorldAsset>>>();
        quiet = if events.read(messages).count() == 0 {
            quiet + 1
        } else {
            0
        };
        if quiet == 4 {
            return;
        }
    }
    panic!("GATE BROKEN: world assets never settled");
}

#[test]
fn every_police_model_animates_and_carries_its_tint() {
    let mut app = glb_app();
    app.world_mut()
        .resource_mut::<GltfExtensionHandlers>()
        .0
        .write_blocking()
        .push(Box::new(StandardMaterialStandIn));
    app.register_type::<MeshMaterial3d<StandardMaterial>>();
    app.finish();
    app.cleanup();
    let units = spawn_units(&mut app);
    for &(unit, ..) in &units {
        let mut intent = app.world_mut().get_mut::<MoveIntent>(unit).unwrap();
        intent.axis = Vec2::Y;
        intent.gait = Gait::Walk;
    }
    let config = visual_config();
    let (c, g, p) = (
        config.civilian_models.len(),
        config.gang_models.len(),
        config.police_models.len(),
    );
    wait_wired(&mut app, units.len() + 1);
    let graphs = app.world().resource::<CharacterAnimations>().graphs.clone();
    let entities = units.iter().map(|&(e, ..)| e).collect::<Vec<_>>();
    for ((key, graph), &(_, kind, k)) in
        animator_graphs(&mut app, &entities).into_iter().zip(&units)
    {
        assert_eq!(
            key,
            1 + c + g + k,
            "{kind:?} with model {k} got the wrong model"
        );
        assert_eq!(
            graph, graphs[key],
            "{kind:?} {k} animates with another model's graph"
        );
    }
    let (before, turns) = leg_turns(&mut app, 1 + c + g + p);
    for (key, &turned) in turns.iter().enumerate().skip(1 + c + g) {
        assert!(
            before.iter().any(|(k, _)| *k == key),
            "model key {key} has no leg-left joint"
        );
        assert!(
            turned > 0.1,
            "model {} ({key}): leg-left turned {turned} rad while walking (T-pose)",
            models()[key]
        );
    }
    settle_world_assets(&mut app);
    for &(unit, kind, k) in &units {
        let source = source_base_color(&mut app, &config.police_models[k]);
        let tinted = tinted_base_color(&mut app, unit);
        let (r, gr, b) = match kind {
            UnitKind::Patrol => config.police_tint,
            UnitKind::Swat => config.swat_tint,
        };
        let expected = [source.red * r, source.green * gr, source.blue * b];
        let got = [tinted.red, tinted.green, tinted.blue];
        for (e, t) in expected.iter().zip(got) {
            assert!(
                (e - t).abs() < 1e-4,
                "{kind:?} {k}: {got:?} vs {expected:?}"
            );
        }
    }
}

#[test]
fn police_models_are_validated() {
    let mut config = visual_config();
    config.police_models.clear();
    let error = config.validate().unwrap_err();
    assert!(error.contains("police_models"), "{error}");
}
