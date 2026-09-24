//! Headless gates of the gang looks (real GLBs): every gang model animates from its own glTF clips,
//! gang tints come from `gang/gangs.ron`, and NPC held guns follow their `Loadout` and owner, also
//! across a model re-instance.

use super::{
    HeldGun, RENDER_CONFIG, RenderConfig,
    character::{CharacterAnimations, CharacterModel},
    civilian_gate::{
        animator_graphs, assets_root, glb_app, leg_turns, manifest, model_of, models,
        visual_config, wait_wired,
    },
    weapons::{WeaponVisualAssets, attach_held_gun, show_held_gun},
};
use bevy::{
    asset::LoadContext,
    gltf::{
        Gltf, GltfLoaderSettings, GltfMaterial, GltfMesh, GltfMeshName,
        extensions::{ErasedGltfExtensionHandler, GltfExtensionHandler, GltfExtensionHandlers},
        gltf,
    },
    prelude::*,
    world_serialization::WorldAsset,
};
use gta_sim::{
    character::{CharacterControlConfig, Gait, HealthConfig, LocomotionConfig, MoveIntent},
    combat::{Loadout, Weapon, WeaponsConfig},
    config::load_config,
    gang::{GangConfig, gang_member_bundle},
    population::Appearance,
};
use std::time::{Duration, Instant};

/// Gang members `k = 0..G` with `Appearance(k)` (one per gang model), gangs alternating 0, 1, 0, on
/// the test floor 3 m apart; no territories, so the gang AI is off.
fn spawn_gang_members(app: &mut App) -> Vec<(Entity, u8)> {
    let world = app.world();
    let loco = world.resource::<LocomotionConfig>().clone();
    let health = world.resource::<HealthConfig>().clone();
    let weapons = world.resource::<WeaponsConfig>().clone();
    let handle = world.resource::<CharacterControlConfig>().0.clone();
    (0..visual_config().gang_models.len())
        .map(|k| {
            let gang = (k % 2) as u8;
            let spot = Vec3::new(-6.0 + 3.0 * k as f32, 0.0, -20.0);
            let bundle = gang_member_bundle(
                &loco,
                handle.clone(),
                &health,
                &weapons,
                gang,
                0,
                spot,
                0.0,
                Weapon::Pistol,
                Appearance(k as u32),
            );
            (app.world_mut().spawn(bundle).id(), gang)
        })
        .collect()
}

/// Base colour of the untinted `tinted_mesh` material in the glTF file `model`.
fn source_base_color(app: &mut App, model: &str) -> LinearRgba {
    let handle = app
        .world()
        .resource::<AssetServer>()
        .load::<Gltf>(model.to_owned());
    let mesh_name = visual_config().tinted_mesh;
    let deadline = Instant::now() + Duration::from_secs(30);
    loop {
        let world = app.world();
        let material = world
            .resource::<Assets<Gltf>>()
            .get(&handle)
            .and_then(|gltf| gltf.named_meshes.get(mesh_name.as_str()))
            .and_then(|mesh| world.resource::<Assets<GltfMesh>>().get(mesh))
            .and_then(|mesh| mesh.primitives.first()?.material.clone())
            .and_then(|m| world.resource::<Assets<GltfMaterial>>().get(&m));
        if let Some(material) = material {
            return material.base_color.to_linear();
        }
        assert!(Instant::now() < deadline, "GATE BROKEN: {model} not loaded");
        app.update();
    }
}

/// Base colour of the tinted mesh under `character`'s model.
fn tinted_base_color(app: &mut App, character: Entity) -> LinearRgba {
    let name = visual_config().tinted_mesh;
    let mesh = app
        .world_mut()
        .query::<(Entity, &GltfMeshName, &MeshMaterial3d<StandardMaterial>)>()
        .iter(app.world())
        .filter(|(_, n, _)| n.0 == name)
        .map(|(e, _, m)| (e, m.0.clone()))
        .collect::<Vec<_>>();
    let (_, material) = mesh
        .into_iter()
        .find(|(e, _)| model_of(app, *e).is_some_and(|m| owner_of(app, m) == character))
        .expect("GATE BROKEN: no tinted mesh under the character");
    app.world()
        .resource::<Assets<StandardMaterial>>()
        .get(&material)
        .expect("tinted material missing")
        .base_color
        .to_linear()
}

fn owner_of(app: &App, model: Entity) -> Entity {
    app.world().get::<ChildOf>(model).unwrap().parent()
}

/// Stand-in for the glTF handler of `PbrPlugin` (absent headless): gives each glTF mesh a
/// `StandardMaterial` with its file's base colour, so `on_model_ready` has a material to tint.
#[derive(Clone)]
struct StandardMaterialStandIn;

impl GltfExtensionHandler for StandardMaterialStandIn {
    fn dyn_clone(&self) -> Box<dyn ErasedGltfExtensionHandler> {
        Box::new(self.clone())
    }

    fn on_root(&mut self, load: &mut LoadContext<'_>, _: &gltf::Gltf, _: &GltfLoaderSettings) {
        let label = format!("{}/std", bevy::gltf::GltfAssetLabel::DefaultMaterial);
        load.add_labeled_asset(label, StandardMaterial::default());
    }

    fn on_material(
        &mut self,
        load: &mut LoadContext<'_>,
        _: &gltf::Material,
        _: Handle<GltfMaterial>,
        material: &GltfMaterial,
        label: &str,
    ) {
        let standard = StandardMaterial {
            base_color: material.base_color,
            ..default()
        };
        load.add_labeled_asset(format!("{label}/std"), standard);
    }

    fn on_spawn_mesh_and_material(
        &mut self,
        load: &mut LoadContext<'_>,
        _: &gltf::Primitive,
        _: &gltf::Mesh,
        _: &gltf::Material,
        entity: &mut EntityWorldMut,
        label: &str,
    ) {
        let handle = load.get_label_handle::<StandardMaterial>(format!("{label}/std"));
        entity.insert(MeshMaterial3d(handle));
    }
}

#[test]
fn every_gang_model_animates_from_its_own_clips() {
    let mut app = glb_app();
    app.world_mut()
        .resource_mut::<GltfExtensionHandlers>()
        .0
        .write_blocking()
        .push(Box::new(StandardMaterialStandIn));
    // `PbrPlugin` registers it for the world asset spawner in the game.
    app.register_type::<MeshMaterial3d<StandardMaterial>>();
    app.finish();
    app.cleanup();
    let members = spawn_gang_members(&mut app);
    for &(member, _) in &members {
        let mut intent = app.world_mut().get_mut::<MoveIntent>(member).unwrap();
        intent.axis = Vec2::Y;
        intent.gait = Gait::Walk;
    }
    let config = visual_config();
    let (c, g) = (config.civilian_models.len(), config.gang_models.len());
    wait_wired(&mut app, g + 1);
    let graphs = app.world().resource::<CharacterAnimations>().graphs.clone();
    let entities = members.iter().map(|&(e, _)| e).collect::<Vec<_>>();
    for (k, (key, graph)) in animator_graphs(&mut app, &entities).into_iter().enumerate() {
        assert_eq!(key, 1 + c + k % g, "gang member {k} got the wrong model");
        assert_eq!(
            graph, graphs[key],
            "gang member {k} animates with another model's graph"
        );
    }
    let gangs = app.world().resource::<GangConfig>().clone();
    for (k, &(member, gang)) in members.iter().enumerate() {
        let source = source_base_color(&mut app, &config.gang_models[k]);
        let tinted = tinted_base_color(&mut app, member);
        let (r, gr, b) = gangs.gangs[gang as usize].tint;
        let expected = [source.red * r, source.green * gr, source.blue * b];
        let got = [tinted.red, tinted.green, tinted.blue];
        for (e, t) in expected.iter().zip(got) {
            assert!(
                (e - t).abs() < 1e-4,
                "member {k} (gang {gang}): {got:?} vs {expected:?}"
            );
        }
    }
    let (before, turns) = leg_turns(&mut app, 1 + c + g);
    for (key, &turned) in turns.iter().enumerate().skip(1 + c) {
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
}

fn held_guns(app: &mut App) -> Vec<(Entity, Entity, Visibility)> {
    app.world_mut()
        .query::<(Entity, &HeldGun, &Visibility)>()
        .iter(app.world())
        .map(|(e, gun, v)| (e, gun.owner, *v))
        .collect()
}

/// Gun of `owner`, if one exists; panics on a second one.
fn gun_of(app: &mut App, owner: Entity) -> Option<Visibility> {
    let own = held_guns(app)
        .into_iter()
        .filter(|(_, o, _)| *o == owner)
        .map(|(_, _, v)| v)
        .collect::<Vec<_>>();
    assert!(own.len() <= 1, "{owner} has {} held guns", own.len());
    own.first().copied()
}

// A model can be re-instanced by the world asset spawner right after it is wired (its children, the
// held gun among them, are replaced): the gate waits for the state instead of counting updates.
#[test]
fn gang_held_gun_follows_loadout_and_owner() {
    let mut app = glb_app();
    let render = load_config::<RenderConfig>(&assets_root(), RENDER_CONFIG)
        .unwrap_or_else(|e| panic!("GATE BROKEN: {e}"));
    app.insert_resource(render)
        .init_resource::<WeaponVisualAssets>()
        .add_systems(Update, (attach_held_gun, show_held_gun).chain());
    app.finish();
    app.cleanup();
    let members = spawn_gang_members(&mut app);
    wait_wired(&mut app, members.len() + 1);
    let until = |app: &mut App, limit: u32, done: &dyn Fn(&mut App) -> bool| {
        (0..limit).any(|_| {
            app.update();
            done(app)
        })
    };
    let all_armed = |app: &mut App| members.iter().all(|&(m, _)| gun_of(app, m).is_some());
    assert!(
        until(&mut app, 60, &all_armed),
        "a gang member got no held gun"
    );
    for &(member, _) in &members {
        assert_eq!(
            gun_of(&mut app, member),
            Some(Visibility::Hidden),
            "holstered gun shown"
        );
    }
    let (armed, _) = members[0];
    app.world_mut().get_mut::<Loadout>(armed).unwrap().held = Some(Weapon::Pistol);
    let drawn = |app: &mut App| gun_of(app, armed) == Some(Visibility::Inherited);
    assert!(until(&mut app, 8, &drawn), "drawn gun hidden");
    // Modifying the model's world asset re-instances it: the joints and the gun under the hand are
    // despawned and the gun must come back on the new hand. Scenes still streaming in re-instance
    // models on their own and debounce a `Modified`, so settle first and retry the modification.
    let hand = |app: &mut App| {
        app.world_mut()
            .query::<(&HeldGun, &ChildOf)>()
            .iter(app.world())
            .find(|(gun, _)| gun.owner == armed)
            .map(|(_, parent)| parent.parent())
    };
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
            break;
        }
    }
    assert_eq!(quiet, 4, "GATE BROKEN: world assets never settled");
    let scene = app
        .world_mut()
        .query_filtered::<(&ChildOf, &WorldAssetRoot), With<CharacterModel>>()
        .iter(app.world())
        .find(|(owner, _)| owner.parent() == armed)
        .map(|(_, root)| root.0.id())
        .expect("GATE BROKEN: the armed member has no model");
    let reinstanced = (0..5).any(|_| {
        assert!(until(&mut app, 8, &drawn), "drawn gun lost");
        let old_hand = hand(&mut app).expect("drawn gun has no hand");
        let mut scenes = app.world_mut().resource_mut::<Assets<WorldAsset>>();
        let mut asset = scenes
            .get_mut(scene)
            .expect("GATE BROKEN: model asset gone");
        let _: &mut WorldAsset = &mut asset;
        drop(asset);
        until(&mut app, 8, &|app: &mut App| {
            app.world().get_entity(old_hand).is_err()
        })
    });
    assert!(reinstanced, "GATE BROKEN: the model was never re-instanced");
    assert!(
        until(&mut app, 1, &drawn),
        "the gun was not back and drawn one update after the model re-instance"
    );
    // Self-repair, not a loop: a re-instance takes the gun for one update; the new one is attached and
    // shown in the next (`attach_held_gun` is chained before `show_held_gun`, as in `VisualsPlugin`).
    let mut unseen = 0;
    for k in 0..32 {
        app.update();
        unseen = match gun_of(&mut app, armed) {
            Some(Visibility::Inherited) => 0,
            _ => unseen + 1,
        };
        assert!(
            unseen <= 1,
            "update {k}: drawn gun not shown for {unseen} updates"
        );
    }
    app.world_mut().despawn(armed);
    app.update();
    assert_eq!(gun_of(&mut app, armed), None, "held gun outlived its owner");
}

#[test]
fn gang_models_are_validated() {
    let manifest = manifest();
    let mut config = visual_config();
    config
        .gang_models
        .push("third_party/inter/Inter-Regular.ttf".into());
    let errors = config.resolve(&manifest).unwrap_err();
    assert!(
        errors.iter().any(|e| e.contains("Inter-Regular.ttf")),
        "{errors:?}"
    );
    let mut config = visual_config();
    config.gang_models.clear();
    let error = config.validate().unwrap_err();
    assert!(error.contains("gang_models"), "{error}");
}
