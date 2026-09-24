//! Car configs (GDD §5, T14): shipped files load, every validate rule fires with its own keyword.

mod common;

use bevy::{asset::AssetPlugin, prelude::*, state::app::StatesPlugin};
use common::{assets_root, sabotaged, sabotaged_load};
use gta_sim::{
    compose_sim,
    config::{ConfigRoot, load_config},
    perception::{PERCEPTION_CONFIG, PerceptionConfig},
    vehicle::{DAMAGE_CONFIG, DamageConfig, VEHICLE_CONFIG, VehicleConfig},
    wanted::{WANTED_CONFIG, WantedConfig},
    world::{CITY_CONFIG, CityParams, WorldSource},
};
use std::{
    fs,
    path::Path,
    time::{SystemTime, UNIX_EPOCH},
};

#[test]
fn shipped_vehicle_configs_load_and_validate() {
    let root = assets_root();
    load_config::<VehicleConfig>(&root, VEHICLE_CONFIG)
        .unwrap()
        .validate()
        .unwrap();
    load_config::<DamageConfig>(&root, DAMAGE_CONFIG)
        .unwrap()
        .validate()
        .unwrap();
}

#[test]
fn unknown_vehicle_fields_name_file_and_field() {
    let error = sabotaged_load::<VehicleConfig>(
        VEHICLE_CONFIG,
        "veh_unknown",
        "    mass: 1200.0,",
        "    bogus_field: 1.0,\n    mass: 1200.0,",
    )
    .unwrap_err()
    .to_string();
    assert!(
        error.contains("sedan.ron") && error.contains("bogus_field"),
        "{error}"
    );
    let error = sabotaged_load::<DamageConfig>(
        DAMAGE_CONFIG,
        "dmg_unknown",
        "    vehicle: (",
        "    bogus_field: 1.0,\n    vehicle: (",
    )
    .unwrap_err()
    .to_string();
    assert!(
        error.contains("damage.ron") && error.contains("bogus_field"),
        "{error}"
    );
}

fn vehicle_error(tag: &str, from: &str, to: &str) -> String {
    sabotaged(VEHICLE_CONFIG, tag, from, to, |c: &VehicleConfig| {
        c.validate()
    })
}

fn damage_error(tag: &str, from: &str, to: &str) -> String {
    sabotaged(DAMAGE_CONFIG, tag, from, to, |c: &DamageConfig| {
        c.validate()
    })
}

#[test]
fn vehicle_rules_fire_with_their_keyword() {
    let rows = [
        ("mass: 1200.0", "mass: NaN", "vehicle values must be finite"),
        ("mass: 1200.0", "mass: -1.0", "mass must be positive"),
        (
            "hold_speed: 0.5",
            "hold_speed: 0.0",
            "hold_speed must be positive",
        ),
        (
            "exit_max_speed: 3.0",
            "exit_max_speed: -3.0",
            "exit_max_speed must be positive",
        ),
        ("travel: 0.3", "travel: 0.1", "suspension.travel"),
        (
            "max_damper_speed: 0.5",
            "max_damper_speed: 0.0",
            "suspension.max_damper_speed must be positive",
        ),
        (
            "at_max_speed_deg: 6.0",
            "at_max_speed_deg: 40.0",
            "steer.at_max_speed_deg",
        ),
        (
            "roll_influence: 0.3",
            "roll_influence: 1.5",
            "roll_influence",
        ),
        (
            "handbrake_rear: 0.25",
            "handbrake_rear: 1.5",
            "grip.handbrake_rear",
        ),
        ("door: (-1.7,", "door: (-1.0,", "door.x"),
        (
            "chamfer_height: 0.3",
            "chamfer_height: -0.1",
            "underbody lift and chamfer must be nonnegative",
        ),
        (
            "lift: 0.2",
            "lift: 1.6",
            "must stay below the chassis height",
        ),
    ];
    for (i, (from, to, keyword)) in rows.into_iter().enumerate() {
        let error = vehicle_error(&format!("veh_{i}"), from, to);
        assert!(error.contains(keyword), "{from} -> {to}: {error}");
    }
}

#[test]
fn damage_rules_fire_with_their_keyword() {
    let rows = [
        (
            "max_health: 1000.0",
            "max_health: NaN",
            "damage values must be finite",
        ),
        (
            "max_health: 1000.0",
            "max_health: 0.0",
            "vehicle.max_health",
        ),
        (
            "threshold_speed: 5.0",
            "threshold_speed: -1.0",
            "vehicle.threshold_speed",
        ),
        ("per_mps: 40.0", "per_mps: 0.0", "vehicle.per_mps"),
        ("bullet_scale: 1.0", "bullet_scale: 0.0", "bullet_scale"),
        (
            "scrape_normal: 0.5",
            "scrape_normal: 1.5",
            "vehicle.scrape_normal",
        ),
        (
            "threshold_speed: 3.0",
            "threshold_speed: -1.0",
            "pedestrian.threshold_speed",
        ),
        ("per_mps: 12.0", "per_mps: 0.0", "pedestrian.per_mps"),
        (
            "knockdown_speed: 4.0",
            "knockdown_speed: 2.0",
            "pedestrian.knockdown_speed",
        ),
        (
            "shove_scale: 0.6",
            "shove_scale: -1.0",
            "pedestrian.shove_scale",
        ),
    ];
    for (i, (from, to, keyword)) in rows.into_iter().enumerate() {
        let error = damage_error(&format!("dmg_{i}"), from, to);
        assert!(error.contains(keyword), "{from} -> {to}: {error}");
    }
}

#[test]
fn car_threat_and_crime_rows_are_validated() {
    let perception = |tag: &str, from: &str, to: &str| {
        sabotaged(PERCEPTION_CONFIG, tag, from, to, |c: &PerceptionConfig| {
            c.validate()
        })
    };
    let error = perception("car_distance", "car_distance: 12.0", "car_distance: 0.0");
    assert!(error.contains("car_distance"), "{error}");
    let error = perception("car_speed", "car_speed: 3.0", "car_speed: -1.0");
    assert!(error.contains("car_speed"), "{error}");
    let wanted = |tag: &str, from: &str, to: &str| {
        sabotaged(WANTED_CONFIG, tag, from, to, |c: &WantedConfig| {
            c.validate()
        })
    };
    let error = wanted("run_over", "run_over: 30", "run_over: 0");
    assert!(error.contains("heat.run_over"), "{error}");
    let error = wanted("car_theft", "car_theft: 15", "car_theft: 0");
    assert!(error.contains("heat.car_theft"), "{error}");
}

/// Copies every shipped `.ron` config under `from` into `to`.
fn copy_configs(from: &Path, to: &Path) {
    for entry in fs::read_dir(from).unwrap() {
        let path = entry.unwrap().path();
        let target = to.join(path.file_name().unwrap());
        if path.is_dir() {
            if path.ends_with("third_party") {
                continue;
            }
            copy_configs(&path, &target);
        } else if path.extension().is_some_and(|e| e == "ron") {
            fs::create_dir_all(to).unwrap();
            fs::copy(&path, &target).unwrap();
        }
    }
}

/// `compose_sim` of a city whose `city.ron` has `from` replaced by `to`.
fn compose_city_with(tag: &str, from: &str, to: &str) -> Result<(), gta_sim::config::ConfigError> {
    let unique = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let root = ConfigRoot(std::env::temp_dir().join(format!(
        "gta_sim_{tag}_{}_{}",
        std::process::id(),
        unique
    )));
    copy_configs(&assets_root().0, &root.0);
    let city = root.path(CITY_CONFIG);
    let original = fs::read_to_string(&city).unwrap();
    assert!(
        original.contains(from),
        "GATE BROKEN: shipped {CITY_CONFIG} has no {from:?}"
    );
    fs::write(&city, original.replacen(from, to, 1)).unwrap();
    let own = load_config::<CityParams>(&root, CITY_CONFIG).map(|c| c.validate());
    let mut app = App::new();
    app.add_plugins((
        MinimalPlugins,
        TransformPlugin,
        AssetPlugin::default(),
        StatesPlugin,
    ));
    let composed = compose_sim(&mut app, root.clone(), WorldSource::City { seed: 1 });
    fs::remove_dir_all(&root.0).unwrap();
    assert!(
        matches!(own, Ok(Ok(()))),
        "GATE BROKEN: sabotaged city.ron fails its own checks: {own:?}"
    );
    composed
}

#[test]
fn parked_car_must_fit_its_curb_lane() {
    compose_city_with("curb_ok", "curb_offset: 1.625", "curb_offset: 1.625")
        .expect("GATE BROKEN: shipped city.ron rejected");
    for (tag, to) in [
        ("curb_near", "curb_offset: 1.0"),
        ("curb_far", "curb_offset: 2.5"),
    ] {
        let error = compose_city_with(tag, "curb_offset: 1.625", to)
            .expect_err("compose_sim accepted a parked car outside its lane");
        assert!(error.path.ends_with(CITY_CONFIG), "{error}");
        assert!(error.message.contains("parking.curb_offset"), "{error}");
    }
}
