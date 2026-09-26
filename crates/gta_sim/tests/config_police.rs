//! Strict loading and validation of the police config and the T11 fields of the wanted and respawn
//! configs; each sabotage sits strictly on the failing side and yields its own error.

mod common;

use common::{assets_root, sabotaged, sabotaged_load};
use gta_sim::{
    combat::{WEAPONS_CONFIG, WeaponsConfig},
    config::load_config,
    flow::{RESPAWN_CONFIG, RespawnConfig},
    police::{EscalationConfig, POLICE_CONFIG},
    population::{POPULATION_CONFIG, PopulationConfig},
    wanted::{WANTED_CONFIG, WantedConfig},
};

fn shipped_despawn_distance() -> f32 {
    let population = load_config::<PopulationConfig>(&assets_root(), POPULATION_CONFIG)
        .unwrap_or_else(|e| panic!("GATE BROKEN: {e}"));
    assert_eq!(
        population.despawn_distance, 150.0,
        "GATE BROKEN: shipped despawn_distance changed"
    );
    population.despawn_distance
}

fn shipped_weapons() -> WeaponsConfig {
    load_config::<WeaponsConfig>(&assets_root(), WEAPONS_CONFIG)
        .unwrap_or_else(|e| panic!("GATE BROKEN: {e}"))
}

fn police_error(tag: &str, from: &str, to: &str) -> String {
    sabotaged::<EscalationConfig>(POLICE_CONFIG, tag, from, to, EscalationConfig::validate)
}

#[test]
fn shipped_police_config_loads_and_validates() {
    let cfg = load_config::<EscalationConfig>(&assets_root(), POLICE_CONFIG).unwrap();
    cfg.validate().unwrap();
    cfg.validate_ring(shipped_despawn_distance()).unwrap();
    cfg.validate_overshoot(&shipped_weapons()).unwrap();
}

#[test]
fn unknown_police_field_names_file_and_field() {
    let error = sabotaged_load::<EscalationConfig>(
        POLICE_CONFIG,
        "police_unknown",
        "(",
        "(bogus_field: 1.0,",
    )
    .unwrap_err()
    .to_string();
    assert!(error.contains("escalation.ron"), "{error}");
    assert!(error.contains("bogus_field"), "{error}");
}

#[test]
fn swat_never_outnumber_the_row() {
    let error = police_error(
        "police_swat",
        "(units: 8,  swat: 4,",
        "(units: 8,  swat: 9,",
    );
    assert!(error.contains("stars[3].swat"), "{error}");
}

#[test]
fn units_never_drop_with_a_star() {
    let error = police_error(
        "police_units",
        "(units: 6,  swat: 0,",
        "(units: 3,  swat: 0,",
    );
    assert!(error.contains("stars[2].units"), "{error}");
}

#[test]
fn break_free_lies_beyond_the_hold() {
    let error = police_error(
        "police_break_free",
        "break_free_distance: 3.0",
        "break_free_distance: 1.2",
    );
    assert!(error.contains("break_free_distance"), "{error}");
}

#[test]
fn cop_damage_scale_is_positive() {
    let error = police_error(
        "police_damage_scale",
        "damage_scale: 0.15, cars: 2",
        "damage_scale: 0.0, cars: 2",
    );
    assert!(error.contains("stars[1].damage_scale"), "{error}");
}

#[test]
fn near_miss_distance_is_positive() {
    let error = police_error(
        "police_near_miss",
        "near_miss_distance: 1.5",
        "near_miss_distance: 0.0",
    );
    assert!(error.contains("arrest.near_miss_distance"), "{error}");
}

#[test]
fn spawn_ring_stays_inside_the_despawn_distance() {
    let despawn = shipped_despawn_distance();
    let error = sabotaged::<EscalationConfig>(
        POLICE_CONFIG,
        "police_ring",
        "spawn_ring: (40.0, 90.0)",
        "spawn_ring: (40.0, 160.0)",
        |c| c.validate_ring(despawn),
    );
    assert!(error.contains("spawn_ring"), "{error}");
}

#[test]
fn every_star_has_a_row() {
    let error = sabotaged_load::<EscalationConfig>(
        POLICE_CONFIG,
        "police_rows",
        "(units: 12, swat: 12, reinforce_seconds: 3.0,  arrest: false, surround: true, damage_scale: 0.3,  cars: 5),",
        "",
    )
    .unwrap_err()
    .to_string();
    assert!(error.contains("length 5"), "{error}");
}

#[test]
fn cop_heat_is_positive() {
    let error = sabotaged::<WantedConfig>(
        WANTED_CONFIG,
        "wanted_kill_cop",
        "kill_cop: 150",
        "kill_cop: 0",
        WantedConfig::validate,
    );
    assert!(error.contains("heat.kill_cop must be > 0"), "{error}");
}

#[test]
fn busted_screen_is_not_negative() {
    let error = sabotaged::<RespawnConfig>(
        RESPAWN_CONFIG,
        "respawn_busted",
        "busted_screen: 3.0",
        "busted_screen: -1.0",
        RespawnConfig::validate,
    );
    assert!(error.contains("busted_screen"), "{error}");
}

#[test]
fn police_overshoot_margin_is_positive() {
    for bad in ["overshoot_margin: -1.0,", "overshoot_margin: 0.0,"] {
        let error = police_error("overshoot", "overshoot_margin: 60.0,", bad);
        assert!(error.contains("overshoot_margin"), "{bad}: {error}");
    }
}

/// Cops keep the old full-reach rule only while no police gun outranges `overshoot_margin`.
#[test]
fn police_overshoot_covers_the_longest_police_gun() {
    let weapons = shipped_weapons();
    let longest = weapons.pistol.range.max(weapons.smg.range);
    assert!(
        longest > 30.0,
        "GATE BROKEN: shipped police guns reach {longest} m"
    );
    let error = sabotaged::<EscalationConfig>(
        POLICE_CONFIG,
        "police_overshoot_reach",
        "overshoot_margin: 60.0,",
        "overshoot_margin: 30.0,",
        |c| c.validate_overshoot(&weapons),
    );
    assert!(error.contains("longest police gun range"), "{error}");
}
