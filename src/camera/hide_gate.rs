//! Headless gate of the near-camera player hide: the production `CameraPlugin` over the sim
//! composition (test area). The camera is placed by its orbit distance with time frozen, so
//! `follow_player` puts it exactly there.

use super::{CAMERA_CONFIG, CameraConfig, CameraPlugin, OrbitCamera};
use crate::{
    input::CursorCaptured,
    juice::{CameraRecoil, CameraShake},
    settings::GameSettings,
};
use bevy::{asset::AssetPlugin, prelude::*, state::app::StatesPlugin, time::TimeUpdateStrategy};
use gta_sim::{
    compose_sim,
    config::{ConfigRoot, load_config},
    player::Player,
    world::WorldSource,
};
use std::{path::Path, time::Duration};

fn assets_root() -> ConfigRoot {
    ConfigRoot(Path::new(env!("CARGO_MANIFEST_DIR")).join("assets"))
}

fn camera_config() -> CameraConfig {
    let cfg = load_config::<CameraConfig>(&assets_root(), CAMERA_CONFIG)
        .unwrap_or_else(|e| panic!("GATE BROKEN: {e}"));
    cfg.validate()
        .unwrap_or_else(|e| panic!("GATE BROKEN: {CAMERA_CONFIG}: {e}"));
    cfg
}

struct Rig {
    app: App,
    player: Entity,
    camera: Entity,
}

/// Sim composition plus the production `CameraPlugin`; the player settles, then time stops.
fn camera_app(config: &CameraConfig) -> Rig {
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
    app.insert_resource(config.clone())
        .insert_resource(CursorCaptured(false))
        .insert_resource(GameSettings::default())
        .init_resource::<CameraRecoil>()
        .init_resource::<CameraShake>()
        .add_plugins(CameraPlugin);
    app.finish();
    app.cleanup();
    let mut player = None;
    for _ in 0..20 {
        app.update();
        player = app
            .world_mut()
            .query_filtered::<Entity, With<Player>>()
            .single(app.world())
            .ok();
        if player.is_some() {
            break;
        }
    }
    let player = player.expect("GATE BROKEN: no Player after 20 updates");
    // Stand-in for the visuals plugin, which inserts it with the character model.
    app.world_mut()
        .entity_mut(player)
        .insert(Visibility::default());
    for _ in 0..60 {
        app.update();
    }
    app.insert_resource(TimeUpdateStrategy::ManualDuration(Duration::ZERO));
    let camera = app
        .world_mut()
        .query_filtered::<Entity, With<OrbitCamera>>()
        .single(app.world())
        .expect("GATE BROKEN: expected exactly one OrbitCamera");
    Rig {
        app,
        player,
        camera,
    }
}

impl Rig {
    fn camera_to_body(&self) -> f32 {
        let world = self.app.world();
        let at = |e: Entity| world.get::<Transform>(e).unwrap().translation;
        at(self.camera).distance(at(self.player))
    }

    fn visibility(&self) -> Visibility {
        *self.app.world().get::<Visibility>(self.player).unwrap()
    }

    /// Shoulder-to-body offset: the part of the camera-to-body distance the orbit distance
    /// cannot shrink (the orbit direction is perpendicular to it at pitch 0).
    fn shoulder_offset(&self) -> f32 {
        let world = self.app.world();
        let orbit = world.get::<OrbitCamera>(self.camera).unwrap();
        let camera = world.get::<Transform>(self.camera).unwrap();
        let back = Quat::from_euler(EulerRot::YXZ, orbit.yaw, orbit.pitch, 0.0) * Vec3::Z;
        let shoulder = camera.translation - back * orbit.distance;
        let body = world.get::<Transform>(self.player).unwrap().translation;
        shoulder.distance(body)
    }

    /// One update with the camera `distance` from the body; returns the measured distance.
    fn place(&mut self, distance: f32) -> f32 {
        let offset = self.shoulder_offset();
        assert!(
            distance > offset + 0.05,
            "GATE BROKEN: {distance} m is inside the shoulder offset {offset} m"
        );
        let orbit = (distance * distance - offset * offset).sqrt();
        self.app
            .world_mut()
            .get_mut::<OrbitCamera>(self.camera)
            .unwrap()
            .distance = orbit;
        self.app.update();
        let measured = self.camera_to_body();
        assert!(
            (measured - distance).abs() < 0.02,
            "GATE BROKEN: camera placed at {measured} m, wanted {distance} m (a wall behind it?)"
        );
        measured
    }
}

/// G-C1 (correctness): the camera closer than `hide_player_distance` to the body hides the player
/// (its model and held gun inherit it); a camera hovering across the threshold inside the band does
/// not toggle it; past `hide_player_distance + hide_player_band` it is visible again. All distances
/// come from `camera.ron`.
#[test]
fn player_hides_when_the_camera_is_pushed_into_it() {
    let cfg = camera_config();
    let (near, band) = (cfg.hide_player_distance, cfg.hide_player_band);
    let mut rig = camera_app(&cfg);
    let open = rig.camera_to_body();
    assert!(
        open > near + band,
        "GATE BROKEN: open camera at {open} m is not past the band"
    );
    assert_eq!(rig.visibility(), Visibility::Inherited, "open camera");

    let d = rig.place(near - 0.2);
    assert_eq!(rig.visibility(), Visibility::Hidden, "camera at {d} m");

    let mut shown = Vec::new();
    for frame in 0..12 {
        let wanted = if frame % 2 == 0 {
            near + band / 2.0
        } else {
            near - 0.1
        };
        let d = rig.place(wanted);
        if rig.visibility() != Visibility::Hidden {
            shown.push(format!("frame {frame} at {d} m"));
        }
    }
    assert!(
        shown.is_empty(),
        "camera hovering across {near} m inside the band showed the player: {shown:?}"
    );

    let d = rig.place(near + band + 0.1);
    assert_eq!(rig.visibility(), Visibility::Inherited, "camera at {d} m");
    let d = rig.place(near + band / 2.0);
    assert_eq!(
        rig.visibility(),
        Visibility::Inherited,
        "camera back inside the band at {d} m, coming from outside"
    );
}
