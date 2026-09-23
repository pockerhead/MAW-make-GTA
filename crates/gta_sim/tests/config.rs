mod common;

use common::assets_root;
use gta_sim::{
    character::{HEALTH_CONFIG, HealthConfig, LOCOMOTION_CONFIG, LocomotionConfig},
    config::{ConfigRoot, load_config},
    flow::{RESPAWN_CONFIG, RespawnConfig},
    world::{CITY_CONFIG, CityParams},
};
use std::{
    fs,
    time::{SystemTime, UNIX_EPOCH},
};

#[test]
fn shipped_locomotion_config_loads() {
    load_config::<LocomotionConfig>(&assets_root(), LOCOMOTION_CONFIG)
        .unwrap()
        .validate()
        .unwrap();
}

#[test]
fn locomotion_thresholds_are_validated() {
    let unique = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let root = ConfigRoot(std::env::temp_dir().join(format!(
        "gta_sim_anim_{}_{}",
        std::process::id(),
        unique
    )));
    let file = root.path(LOCOMOTION_CONFIG);
    fs::create_dir_all(file.parent().unwrap()).unwrap();
    let original = fs::read_to_string(assets_root().path(LOCOMOTION_CONFIG)).unwrap();
    assert!(
        original.contains("anim_idle_speed: 0.2,"),
        "GATE BROKEN: shipped locomotion.ron has no anim_idle_speed: 0.2"
    );
    fs::write(
        &file,
        original.replacen("anim_idle_speed: 0.2,", "anim_idle_speed: 2.0,", 1),
    )
    .unwrap();
    let loaded = load_config::<LocomotionConfig>(&root, LOCOMOTION_CONFIG);
    fs::remove_dir_all(&root.0).unwrap();
    let error = loaded.unwrap().validate().unwrap_err();
    assert!(error.contains("anim_idle_speed"), "{error}");
}

#[test]
fn shipped_city_config_loads_and_validates() {
    let params = load_config::<CityParams>(&assets_root(), CITY_CONFIG).unwrap();
    params.validate().unwrap();
}

#[test]
fn unknown_city_field_names_file_and_field() {
    let unique = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let root = ConfigRoot(std::env::temp_dir().join(format!(
        "gta_sim_city_{}_{}",
        std::process::id(),
        unique
    )));
    let file = root.path(CITY_CONFIG);
    fs::create_dir_all(file.parent().unwrap()).unwrap();
    let original = fs::read_to_string(assets_root().path(CITY_CONFIG)).unwrap();
    fs::write(&file, original.replacen('(', "(bogus_field: 1.0,", 1)).unwrap();
    let error = load_config::<CityParams>(&root, CITY_CONFIG)
        .unwrap_err()
        .to_string();
    fs::remove_dir_all(&root.0).unwrap();
    assert!(
        error.contains("city.ron") && error.contains("bogus_field"),
        "{error}"
    );
}

#[test]
fn unknown_field_names_file_and_field() {
    let unique = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let root = ConfigRoot(std::env::temp_dir().join(format!(
        "gta_sim_cfg_{}_{}",
        std::process::id(),
        unique
    )));
    let file = root.path(LOCOMOTION_CONFIG);
    fs::create_dir_all(file.parent().unwrap()).unwrap();
    let original = fs::read_to_string(assets_root().path(LOCOMOTION_CONFIG)).unwrap();
    fs::write(&file, original.replacen('(', "(bogus_field: 1.0,", 1)).unwrap();
    let error = load_config::<LocomotionConfig>(&root, LOCOMOTION_CONFIG)
        .unwrap_err()
        .to_string();
    fs::remove_dir_all(&root.0).unwrap();
    assert!(
        error.contains("locomotion.ron") && error.contains("bogus_field"),
        "{error}"
    );
}

#[test]
fn shipped_health_config_loads() {
    load_config::<HealthConfig>(&assets_root(), HEALTH_CONFIG)
        .unwrap()
        .validate()
        .unwrap();
}

#[test]
fn shipped_respawn_config_loads() {
    load_config::<RespawnConfig>(&assets_root(), RESPAWN_CONFIG)
        .unwrap()
        .validate()
        .unwrap();
}

/// Validation error of the shipped health.ron with `from` replaced by `to`.
fn health_error(tag: &str, from: &str, to: &str) -> String {
    let unique = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let root = ConfigRoot(std::env::temp_dir().join(format!(
        "gta_sim_{tag}_{}_{}",
        std::process::id(),
        unique
    )));
    let file = root.path(HEALTH_CONFIG);
    fs::create_dir_all(file.parent().unwrap()).unwrap();
    let original = fs::read_to_string(assets_root().path(HEALTH_CONFIG)).unwrap();
    assert!(
        original.contains(from),
        "GATE BROKEN: shipped health.ron has no {from:?}"
    );
    fs::write(&file, original.replacen(from, to, 1)).unwrap();
    let loaded = load_config::<HealthConfig>(&root, HEALTH_CONFIG);
    fs::remove_dir_all(&root.0).unwrap();
    loaded.unwrap().validate().unwrap_err()
}

#[test]
fn health_regen_cap_is_validated() {
    let error = health_error("regen_cap", "regen_cap: 0.5,", "regen_cap: 1.5,");
    assert!(error.contains("regen_cap"), "{error}");
}

#[test]
fn pickup_spacing_must_exceed_radius() {
    let error = health_error("spacing", "spacing: 6.0)", "spacing: 0.5)");
    assert!(error.contains("spacing"), "{error}");
}
