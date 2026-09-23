mod common;

use common::assets_root;
use gta_sim::{
    character::{LOCOMOTION_CONFIG, LocomotionConfig},
    config::{ConfigRoot, load_config},
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
