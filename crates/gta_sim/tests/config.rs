mod common;

use bevy::{asset::AssetPlugin, prelude::*, state::app::StatesPlugin};
use common::{assets_root, sabotaged};
use gta_sim::{
    character::{HEALTH_CONFIG, HealthConfig, LOCOMOTION_CONFIG, LocomotionConfig},
    civilian::{CIVILIAN_CONFIG, CivilianConfig},
    combat::{AIM_CONFIG, AimConfig, MELEE_CONFIG, MeleeConfig, WEAPONS_CONFIG, WeaponsConfig},
    compose_sim,
    config::{ConfigRoot, load_config},
    flow::{RESPAWN_CONFIG, RespawnConfig},
    gang::{GANG_CONFIG, GangConfig},
    navigation::{NAVIGATION_CONFIG, NavigationConfig},
    perception::{PERCEPTION_CONFIG, PerceptionConfig},
    population::{POPULATION_CONFIG, PopulationConfig},
    wanted::{WANTED_CONFIG, WantedConfig},
    world::{CITY_CONFIG, CityParams, WorldSource},
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
        "spawn_ring: (30.0, 120.0),",
        "spawn_ring: (130.0, 120.0),",
        PopulationConfig::validate,
    );
    assert!(error.contains("spawn_ring"), "{error}");
}

#[test]
fn occlusion_ray_budget_checks_a_whole_point() {
    let error = sabotaged::<PopulationConfig>(
        POPULATION_CONFIG,
        "occlusion_rays",
        "occlusion_rays_per_tick: 16,",
        "occlusion_rays_per_tick: 3,",
        PopulationConfig::validate,
    );
    assert!(error.contains("occlusion_rays_per_tick"), "{error}");
}

#[test]
fn spawn_points_no_denser_than_the_separation() {
    let error = sabotaged::<PopulationConfig>(
        POPULATION_CONFIG,
        "spawn_point_spacing",
        "spawn_point_spacing: 8.0,",
        "spawn_point_spacing: 2.0,",
        PopulationConfig::validate,
    );
    assert!(error.contains("spawn_point_spacing"), "{error}");
}

#[test]
fn recycling_stays_outside_the_spawn_ring_inner_edge() {
    let error = sabotaged::<PopulationConfig>(
        POPULATION_CONFIG,
        "recycle_distance",
        "recycle_distance: 50.0,",
        "recycle_distance: 20.0,",
        PopulationConfig::validate,
    );
    assert!(error.contains("recycle_distance"), "{error}");
}

#[test]
fn recycling_needs_a_budget() {
    let error = sabotaged::<PopulationConfig>(
        POPULATION_CONFIG,
        "recycles_per_tick",
        "recycles_per_tick: 1,",
        "recycles_per_tick: 0,",
        PopulationConfig::validate,
    );
    assert!(error.contains("recycles_per_tick"), "{error}");
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

#[test]
fn call_after_flee_is_a_chance() {
    let error = sabotaged::<CivilianConfig>(
        CIVILIAN_CONFIG,
        "call_after_flee",
        "call_after_flee: 0.8,",
        "call_after_flee: 1.5,",
        CivilianConfig::validate,
    );
    assert!(error.contains("call_after_flee"), "{error}");
}

#[test]
fn fight_report_distance_below_fight_hearing() {
    let shipped = assets_root();
    let hearing = load_config::<PerceptionConfig>(&shipped, PERCEPTION_CONFIG)
        .unwrap()
        .fight_hearing_radius;
    assert!(
        hearing < 21.0,
        "GATE BROKEN: shipped fight_hearing_radius {hearing} >= 21"
    );
    let unique = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let root = ConfigRoot(std::env::temp_dir().join(format!(
        "gta_sim_fight_report_{}_{}",
        std::process::id(),
        unique
    )));
    for rel in [
        LOCOMOTION_CONFIG,
        HEALTH_CONFIG,
        RESPAWN_CONFIG,
        WEAPONS_CONFIG,
        AIM_CONFIG,
        MELEE_CONFIG,
        POPULATION_CONFIG,
        PERCEPTION_CONFIG,
        NAVIGATION_CONFIG,
        CIVILIAN_CONFIG,
        GANG_CONFIG,
        WANTED_CONFIG,
    ] {
        let file = root.path(rel);
        fs::create_dir_all(file.parent().unwrap()).unwrap();
        fs::copy(shipped.path(rel), &file).unwrap();
    }
    // Passes civilian.ron's own checks, but sits past the distance a fight is heard at.
    let civilian = root.path(CIVILIAN_CONFIG);
    let original = fs::read_to_string(&civilian).unwrap();
    let from = "fight_report_min_distance: 15.0,";
    assert!(
        original.contains(from),
        "GATE BROKEN: shipped {CIVILIAN_CONFIG} has no {from:?}"
    );
    fs::write(
        &civilian,
        original.replacen(from, "fight_report_min_distance: 21.0,", 1),
    )
    .unwrap();
    let own = load_config::<CivilianConfig>(&root, CIVILIAN_CONFIG).map(|c| c.validate());
    let mut app = App::new();
    app.add_plugins((
        MinimalPlugins,
        TransformPlugin,
        AssetPlugin::default(),
        StatesPlugin,
    ));
    let composed = compose_sim(&mut app, root.clone(), WorldSource::TestArea);
    fs::remove_dir_all(&root.0).unwrap();
    assert!(matches!(own, Ok(Ok(()))), "GATE BROKEN: {own:?}");
    let error = composed.expect_err("compose_sim accepted an unreportable fight");
    assert!(error.path.ends_with(CIVILIAN_CONFIG), "{error}");
    assert!(
        error.message.contains("fight_report_min_distance"),
        "{error}"
    );
    assert!(!error.message.contains("  "), "{error}");
}

#[test]
fn shipped_gang_config_loads() {
    load_config::<GangConfig>(&assets_root(), GANG_CONFIG)
        .unwrap()
        .validate()
        .unwrap();
}

#[test]
fn unknown_gang_field_names_file_and_field() {
    let unique = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let root = ConfigRoot(std::env::temp_dir().join(format!(
        "gta_sim_gang_unknown_{}_{unique}",
        std::process::id()
    )));
    let file = root.path(GANG_CONFIG);
    fs::create_dir_all(file.parent().unwrap()).unwrap();
    let original = fs::read_to_string(assets_root().path(GANG_CONFIG)).unwrap();
    assert!(
        original.contains("spread: 1.0,"),
        "GATE BROKEN: shipped gangs.ron has no spread: 1.0"
    );
    fs::write(
        &file,
        original.replacen("spread: 1.0,", "spread: 1.0, bogus_field: 1.0,", 1),
    )
    .unwrap();
    let error = load_config::<GangConfig>(&root, GANG_CONFIG)
        .unwrap_err()
        .to_string();
    fs::remove_dir_all(&root.0).unwrap();
    assert!(
        error.contains("gangs.ron") && error.contains("bogus_field"),
        "{error}"
    );
}

fn gang_error(tag: &str, from: &str, to: &str) -> String {
    sabotaged::<GangConfig>(GANG_CONFIG, tag, from, to, GangConfig::validate)
}

#[test]
fn exactly_two_gangs() {
    let red = "(tint: (1.0, 0.25, 0.2), weapons: [Pistol, Shotgun]),";
    let error = gang_error("gang_count", red, &format!("{red} {red}"));
    assert!(error.contains("exactly 2 gangs"), "{error}");
}

#[test]
fn faction_matrix_must_be_complete() {
    let error = gang_error(
        "gang_police",
        "(a: Gang(0), b: Police, hostile: false),",
        "",
    );
    assert!(error.contains("missing pair (Gang(0), Police)"), "{error}");
}

#[test]
fn keep_distance_must_be_ordered() {
    let error = gang_error(
        "keep_distance",
        "keep_distance: (8.0, 15.0),",
        "keep_distance: (15.0, 8.0),",
    );
    assert!(error.contains("keep_distance"), "{error}");
}

#[test]
fn retreat_health_is_a_share() {
    let error = gang_error(
        "retreat_health",
        "retreat_health: 0.3,",
        "retreat_health: 1.5,",
    );
    assert!(error.contains("retreat_health"), "{error}");
}

#[test]
fn fire_line_margin_is_not_negative() {
    let error = gang_error(
        "fire_line_margin",
        "fire_line_margin: 0.2,",
        "fire_line_margin: -0.1,",
    );
    assert!(error.contains("fire_line_margin"), "{error}");
}

#[test]
fn reposition_spots_lie_away_from_the_member() {
    let error = gang_error(
        "reposition_offsets",
        "reposition_offsets: [1.5, 3.0, 4.5],",
        "reposition_offsets: [1.5, -3.0, 4.5],",
    );
    assert!(error.contains("reposition_offsets"), "{error}");
    let error = gang_error(
        "reposition_step",
        "reposition_step: 2.0,",
        "reposition_step: -2.0,",
    );
    assert!(error.contains("reposition_step"), "{error}");
}

#[test]
fn dropped_guns_need_a_lifetime() {
    let error = sabotaged::<WeaponsConfig>(
        WEAPONS_CONFIG,
        "drop_seconds",
        "drop_seconds: 60.0",
        "drop_seconds: 0.0",
        WeaponsConfig::validate,
    );
    assert!(error.contains("pickups.drop_seconds"), "{error}");
}

#[test]
fn route_search_budget_is_at_least_one() {
    let error = sabotaged::<NavigationConfig>(
        NAVIGATION_CONFIG,
        "route_requests",
        "route_requests_per_tick: 2,",
        "route_requests_per_tick: 0,",
        NavigationConfig::validate,
    );
    assert!(error.contains("route_requests_per_tick"), "{error}");
}

#[test]
fn direct_seek_distance_is_positive() {
    let error = sabotaged::<NavigationConfig>(
        NAVIGATION_CONFIG,
        "direct_seek",
        "direct_seek_distance: 25.0,",
        "direct_seek_distance: -1.0,",
        NavigationConfig::validate,
    );
    assert!(error.contains("direct_seek_distance"), "{error}");
}

#[test]
fn shipped_wanted_config_loads_and_validates() {
    load_config::<WantedConfig>(&assets_root(), WANTED_CONFIG)
        .unwrap()
        .validate()
        .unwrap();
}

/// Load error of the shipped wanted.ron with `from` replaced by `to`.
fn wanted_load_error(tag: &str, from: &str, to: &str) -> String {
    let unique = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let root = ConfigRoot(std::env::temp_dir().join(format!(
        "gta_sim_wanted_{tag}_{}_{unique}",
        std::process::id()
    )));
    let file = root.path(WANTED_CONFIG);
    fs::create_dir_all(file.parent().unwrap()).unwrap();
    let original = fs::read_to_string(assets_root().path(WANTED_CONFIG)).unwrap();
    assert!(
        original.contains(from),
        "GATE BROKEN: shipped wanted.ron has no {from:?}"
    );
    fs::write(&file, original.replacen(from, to, 1)).unwrap();
    let loaded = load_config::<WantedConfig>(&root, WANTED_CONFIG);
    fs::remove_dir_all(&root.0).unwrap();
    loaded.unwrap_err().to_string()
}

#[test]
fn unknown_wanted_field_names_file_and_field() {
    let error = wanted_load_error(
        "unknown",
        "shooting_radius: 15.0,",
        "shooting_radius: 15.0, bogus: 1,",
    );
    assert!(
        error.contains("wanted.ron") && error.contains("bogus"),
        "{error}"
    );
}

#[test]
fn star_thresholds_must_increase() {
    let error = sabotaged::<WantedConfig>(
        WANTED_CONFIG,
        "wanted_order",
        "(heat: 180,",
        "(heat: 30,",
        WantedConfig::validate,
    );
    assert!(error.contains("stars[1].heat"), "{error}");
}

#[test]
fn five_star_rows_required() {
    let error = wanted_load_error(
        "four_rows",
        "(heat: 2400, search_radius: 180.0, clear_seconds: 30.0),",
        "",
    );
    assert!(error.contains("length 5"), "{error}");
}

#[test]
fn incident_memory_must_outlast_a_civilian_call() {
    let root = assets_root();
    let civilian = load_config::<CivilianConfig>(&root, CIVILIAN_CONFIG).unwrap();
    let perception = load_config::<PerceptionConfig>(&root, PERCEPTION_CONFIG).unwrap();
    let loco = load_config::<LocomotionConfig>(&root, LOCOMOTION_CONFIG).unwrap();
    let corpse_seconds = load_config::<PopulationConfig>(&root, POPULATION_CONFIG)
        .unwrap()
        .corpse_seconds;
    let slots = f32::from(perception.slots) / 64.0;
    let flee_speed = loco.speed(civilian.flee_gait);
    // Same bound as compose_sim: a body in sight until it despawns (30 s), perception slots at 64 Hz,
    // a full crouch (6 s), a full flight (60 m at 4.5 m/s), then the call (4 s).
    let call_delay = civilian.longest_call_delay(corpse_seconds, slots, flee_speed);
    assert!(
        (call_delay - (30.0 + 0.0625 + 6.0 + 60.0 / 4.5 + 4.0)).abs() < 1e-4,
        "GATE BROKEN: shipped call delay changed: {call_delay}"
    );
    let mut wanted = load_config::<WantedConfig>(&root, WANTED_CONFIG).unwrap();
    wanted.validate_call_delay(call_delay).unwrap();
    // Passes the wanted.ron-only checks and outlasts a call counted from the last stimulus, but
    // forgets a kill before a witness who first sees the body late finishes its call.
    wanted.shooting_merge_seconds = 0.0;
    wanted.incident_memory_seconds = 40.0;
    wanted.validate().unwrap();
    let from_stimulus = civilian.longest_call_delay(0.0, slots, flee_speed);
    assert!(
        wanted.validate_call_delay(from_stimulus).is_ok(),
        "GATE BROKEN: 40 s does not outlast the per-stimulus bound {from_stimulus}"
    );
    let error = wanted.validate_call_delay(call_delay).unwrap_err();
    assert!(error.contains("incident_memory_seconds"), "{error}");
    assert!(!error.contains("  "), "a run of spaces in: {error}");
}

/// Copies every config under the shipped assets root (no third-party assets) into a fresh temp root.
fn config_copy(tag: &str) -> ConfigRoot {
    fn copy_ron(from: &std::path::Path, to: &std::path::Path) {
        for entry in fs::read_dir(from).unwrap() {
            let path = entry.unwrap().path();
            let name = path.file_name().unwrap();
            if path.is_dir() && name != "third_party" {
                copy_ron(&path, &to.join(name));
            } else if path.extension().is_some_and(|e| e == "ron") {
                fs::create_dir_all(to).unwrap();
                fs::copy(&path, to.join(name)).unwrap();
            }
        }
    }
    let unique = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let root = ConfigRoot(std::env::temp_dir().join(format!(
        "gta_sim_{tag}_{}_{}",
        std::process::id(),
        unique
    )));
    copy_ron(&assets_root().0, &root.0);
    root
}

#[test]
fn compose_counts_the_call_delay_from_the_crime() {
    let root = config_copy("call_delay");
    let wanted = root.path(WANTED_CONFIG);
    let original = fs::read_to_string(&wanted).unwrap();
    let from = "incident_memory_seconds: 60.0,";
    assert!(
        original.contains(from),
        "GATE BROKEN: shipped {WANTED_CONFIG} has no {from:?}"
    );
    // Longer than a call from the last stimulus (23.4 s), shorter than one from the kill (53.4 s).
    fs::write(
        &wanted,
        original.replacen(from, "incident_memory_seconds: 40.0,", 1),
    )
    .unwrap();
    let mut app = App::new();
    app.add_plugins((
        MinimalPlugins,
        TransformPlugin,
        AssetPlugin::default(),
        StatesPlugin,
    ));
    let composed = compose_sim(&mut app, root.clone(), WorldSource::TestArea);
    fs::remove_dir_all(&root.0).unwrap();
    let error = composed.expect_err("compose_sim accepted a memory shorter than a late body call");
    assert!(error.path.ends_with(WANTED_CONFIG), "{error}");
    assert!(error.message.contains("incident_memory_seconds"), "{error}");
}
