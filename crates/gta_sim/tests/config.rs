mod common;

use common::assets_root;
use gta_sim::{
    character::{LOCOMOTION_CONFIG, LocomotionConfig},
    config::{ConfigRoot, load_config},
};
use std::{
    fs,
    time::{SystemTime, UNIX_EPOCH},
};

#[test]
fn shipped_locomotion_config_loads() {
    load_config::<LocomotionConfig>(&assets_root(), LOCOMOTION_CONFIG).unwrap();
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
