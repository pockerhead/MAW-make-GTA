mod common;

use common::assets_root;
use gta_sim::{
    character::{HEALTH_CONFIG, HealthConfig, LOCOMOTION_CONFIG, LocomotionConfig},
    civilian::{CIVILIAN_CONFIG, CivilianConfig},
    combat::{AIM_CONFIG, AimConfig, MELEE_CONFIG, MeleeConfig, WEAPONS_CONFIG, WeaponsConfig},
    config::{ConfigRoot, load_config},
    flow::{RESPAWN_CONFIG, RespawnConfig},
    navigation::{NAVIGATION_CONFIG, NavigationConfig},
    perception::{PERCEPTION_CONFIG, PerceptionConfig},
    population::{POPULATION_CONFIG, PopulationConfig},
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

#[test]
fn shipped_weapons_config_loads() {
    load_config::<WeaponsConfig>(&assets_root(), WEAPONS_CONFIG)
        .unwrap()
        .validate()
        .unwrap();
}

#[test]
fn shipped_aim_config_loads() {
    load_config::<AimConfig>(&assets_root(), AIM_CONFIG)
        .unwrap()
        .validate()
        .unwrap();
}

/// Validation error of the shipped `rel` config with `from` replaced by `to`.
fn sabotaged<T: serde::de::DeserializeOwned>(
    rel: &str,
    tag: &str,
    from: &str,
    to: &str,
    validate: impl FnOnce(&T) -> Result<(), String>,
) -> String {
    let unique = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let root = ConfigRoot(std::env::temp_dir().join(format!(
        "gta_sim_{tag}_{}_{}",
        std::process::id(),
        unique
    )));
    let file = root.path(rel);
    fs::create_dir_all(file.parent().unwrap()).unwrap();
    let original = fs::read_to_string(assets_root().path(rel)).unwrap();
    assert!(
        original.contains(from),
        "GATE BROKEN: shipped {rel} has no {from:?}"
    );
    fs::write(&file, original.replacen(from, to, 1)).unwrap();
    let loaded = load_config::<T>(&root, rel);
    fs::remove_dir_all(&root.0).unwrap();
    validate(&loaded.unwrap()).unwrap_err()
}

#[test]
fn head_must_stick_out_of_capsule() {
    let error = sabotaged::<LocomotionConfig>(
        LOCOMOTION_CONFIG,
        "head",
        "head_radius: 0.35,",
        // 0.2 would only touch the capsule top (1.6 + 0.2 = 1.8 = top): f32 rounding decides it.
        "head_radius: 0.15,",
        LocomotionConfig::validate,
    );
    assert!(error.contains("head_radius"), "{error}");
}

#[test]
fn damage_variance_below_one() {
    let error = sabotaged::<WeaponsConfig>(
        WEAPONS_CONFIG,
        "variance",
        "damage: 25.0, damage_variance: 0.1,",
        "damage: 25.0, damage_variance: 1.0,",
        WeaponsConfig::validate,
    );
    assert!(error.contains("pistol.damage_variance"), "{error}");
}

#[test]
fn reload_must_be_positive() {
    let error = sabotaged::<WeaponsConfig>(
        WEAPONS_CONFIG,
        "reload",
        "reload: 1.2,",
        "reload: 0.0,",
        WeaponsConfig::validate,
    );
    assert!(error.contains("pistol.reload"), "{error}");
}

#[test]
fn shipped_melee_config_loads() {
    load_config::<MeleeConfig>(&assets_root(), MELEE_CONFIG)
        .unwrap()
        .validate()
        .unwrap();
}

#[test]
fn melee_window_must_end_before_the_swing() {
    // The finisher's window closes strictly after its 0.35 s swing.
    let error = sabotaged::<MeleeConfig>(
        MELEE_CONFIG,
        "melee_window",
        "active_to: 0.22, knockback: 5.0",
        "active_to: 0.40, knockback: 5.0",
        MeleeConfig::validate,
    );
    assert!(error.contains("fists.hits[2].active_to"), "{error}");
}

#[test]
fn melee_weapon_needs_a_hit() {
    let mut cfg = load_config::<MeleeConfig>(&assets_root(), MELEE_CONFIG).unwrap();
    cfg.bat.hits.clear();
    let error = cfg.validate().unwrap_err();
    assert!(error.contains("bat.hits"), "{error}");
}

#[test]
fn melee_cast_radius_must_be_positive() {
    let error = sabotaged::<MeleeConfig>(
        MELEE_CONFIG,
        "melee_radius",
        "cast_radius: 0.35",
        "cast_radius: -0.35",
        MeleeConfig::validate,
    );
    assert!(error.contains("cast_radius"), "{error}");
}

#[test]
fn unknown_melee_field_names_file_and_field() {
    let root = assets_root();
    let original = fs::read_to_string(root.path(MELEE_CONFIG)).unwrap();
    assert!(
        original.contains("stagger: 0.35"),
        "GATE BROKEN: shipped melee.ron has no stagger: 0.35"
    );
    let unique = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let tmp = ConfigRoot(std::env::temp_dir().join(format!(
        "gta_sim_melee_unknown_{}_{unique}",
        std::process::id()
    )));
    let file = tmp.path(MELEE_CONFIG);
    fs::create_dir_all(file.parent().unwrap()).unwrap();
    fs::write(
        &file,
        original.replacen("stagger: 0.35", "stagger: 0.35, bogus_field: 1.0", 1),
    )
    .unwrap();
    let error = load_config::<MeleeConfig>(&tmp, MELEE_CONFIG)
        .unwrap_err()
        .to_string();
    fs::remove_dir_all(&tmp.0).unwrap();
    assert!(
        error.contains("melee.ron") && error.contains("bogus_field"),
        "{error}"
    );
}

#[test]
fn shipped_npc_configs_load() {
    let root = assets_root();
    load_config::<PopulationConfig>(&root, POPULATION_CONFIG)
        .unwrap()
        .validate()
        .unwrap();
    load_config::<PerceptionConfig>(&root, PERCEPTION_CONFIG)
        .unwrap()
        .validate()
        .unwrap();
    load_config::<NavigationConfig>(&root, NAVIGATION_CONFIG)
        .unwrap()
        .validate()
        .unwrap();
    load_config::<CivilianConfig>(&root, CIVILIAN_CONFIG)
        .unwrap()
        .validate()
        .unwrap();
}

#[test]
fn unknown_population_field_names_file_and_field() {
    let original = fs::read_to_string(assets_root().path(POPULATION_CONFIG)).unwrap();
    assert!(
        original.contains("max_civilians: 40,"),
        "GATE BROKEN: shipped population.ron has no max_civilians: 40"
    );
    let unique = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let tmp = ConfigRoot(std::env::temp_dir().join(format!(
        "gta_sim_population_unknown_{}_{unique}",
        std::process::id()
    )));
    let file = tmp.path(POPULATION_CONFIG);
    fs::create_dir_all(file.parent().unwrap()).unwrap();
    fs::write(
        &file,
        original.replacen(
            "max_civilians: 40,",
            "max_civilians: 40, bogus_field: 1.0,",
            1,
        ),
    )
    .unwrap();
    let error = load_config::<PopulationConfig>(&tmp, POPULATION_CONFIG)
        .unwrap_err()
        .to_string();
    fs::remove_dir_all(&tmp.0).unwrap();
    assert!(
        error.contains("population.ron") && error.contains("bogus_field"),
        "{error}"
    );
}

#[test]
fn spawn_ring_must_be_ordered() {
    let error = sabotaged::<PopulationConfig>(
        POPULATION_CONFIG,
        "spawn_ring",
        "spawn_ring: (60.0, 120.0),",
        "spawn_ring: (130.0, 120.0),",
        PopulationConfig::validate,
    );
    assert!(error.contains("spawn_ring"), "{error}");
}

#[test]
fn initial_fill_starts_inside_the_ring() {
    let error = sabotaged::<PopulationConfig>(
        POPULATION_CONFIG,
        "initial_inner",
        "initial_inner_radius: 20.0,",
        "initial_inner_radius: 70.0,",
        PopulationConfig::validate,
    );
    assert!(error.contains("initial_inner_radius"), "{error}");
}

#[test]
fn perception_needs_a_slot() {
    let error = sabotaged::<PerceptionConfig>(
        PERCEPTION_CONFIG,
        "slots",
        "slots: 4,",
        "slots: 0,",
        PerceptionConfig::validate,
    );
    assert!(error.contains("slots"), "{error}");
}

#[test]
fn temperament_spread_below_one() {
    let error = sabotaged::<CivilianConfig>(
        CIVILIAN_CONFIG,
        "spread",
        "temperament_spread: 0.5,",
        "temperament_spread: 1.5,",
        CivilianConfig::validate,
    );
    assert!(error.contains("temperament_spread"), "{error}");
}
