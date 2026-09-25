//! Traffic, police car and T15 config rules (GDD §5.2, §5.3, §12): shipped files load, every
//! validate rule fires on its failing side with its own keyword, the cross-config rules hold.

mod common;

use bevy::prelude::*;
use common::{assets_root, sabotaged, sabotaged_load};
use gta_sim::{
    config::load_config,
    police::{EscalationConfig, POLICE_CONFIG},
    population::{POPULATION_CONFIG, PopulationConfig},
    traffic::{TRAFFIC_CONFIG, TrafficConfig},
    vehicle::{DAMAGE_CONFIG, DamageConfig, VEHICLE_CONFIG, VehicleConfig},
    wanted::{WANTED_CONFIG, WantedConfig},
};

fn tick() -> f32 {
    Time::<Fixed>::default().timestep().as_secs_f32()
}

fn traffic_error(tag: &str, from: &str, to: &str) -> String {
    sabotaged(TRAFFIC_CONFIG, tag, from, to, |c: &TrafficConfig| {
        c.validate(tick())
    })
}

#[test]
fn shipped_traffic_config_loads_and_validates() {
    let root = assets_root();
    let traffic = load_config::<TrafficConfig>(&root, TRAFFIC_CONFIG).unwrap();
    traffic.validate(tick()).unwrap();
    let vehicle = load_config::<VehicleConfig>(&root, VEHICLE_CONFIG).unwrap();
    let population = load_config::<PopulationConfig>(&root, POPULATION_CONFIG).unwrap();
    traffic
        .validate_cross(vehicle.max_speed, population.despawn_distance)
        .unwrap();
}

#[test]
fn unknown_traffic_field_names_file_and_field() {
    let error = sabotaged_load::<TrafficConfig>(
        TRAFFIC_CONFIG,
        "traffic_unknown",
        "    turn_speed: 6.0,",
        "    bogus_field: 1.0,\n    turn_speed: 6.0,",
    )
    .unwrap_err()
    .to_string();
    assert!(
        error.contains("traffic.ron") && error.contains("bogus_field"),
        "{error}"
    );
}

#[test]
fn traffic_rules_fire_with_their_keyword() {
    let rows = [
        (
            "time_headway: 1.5",
            "time_headway: -1.5",
            "idm.time_headway",
        ),
        ("min_gap: 2.0", "min_gap: 0.0", "idm.min_gap"),
        ("turn_speed: 6.0", "turn_speed: NaN", "turn_speed"),
        ("look_ahead: 60.0", "look_ahead: -60.0", "look_ahead"),
        (
            "offscreen_seconds: 2.0",
            "offscreen_seconds: 0.0",
            "bubble.offscreen_seconds",
        ),
        (
            "max_deceleration: 8.0",
            "max_deceleration: 1.0",
            "max_deceleration 1 must be >= idm.comfortable_deceleration",
        ),
        (
            "off_view: (spawn: 15.0, despawn: 25.0)",
            "off_view: (spawn: 15.0, despawn: 80.0)",
            "bubble bands",
        ),
        ("max_cars: 24", "max_cars: 0", "bubble.max_cars"),
        (
            "spawns_per_tick: 1, initial",
            "spawns_per_tick: 0, initial",
            "bubble.spawns_per_tick",
        ),
        (
            "initial_spawns_per_tick: 4",
            "initial_spawns_per_tick: 0",
            "bubble.initial_spawns_per_tick",
        ),
        (
            "connector_samples: 8",
            "connector_samples: 2",
            "connector_samples 2 must be >= 3",
        ),
        ("angle_deg: 60.0", "angle_deg: 190.0", "lost.angle_deg"),
        ("skin: 0.1", "skin: -0.1", "switch.skin"),
        (
            "horizon_seconds: 0.1",
            "horizon_seconds: 0.02",
            "two fixed ticks",
        ),
    ];
    for (i, (from, to, keyword)) in rows.into_iter().enumerate() {
        let error = traffic_error(&format!("traffic_{i}"), from, to);
        assert!(error.contains(keyword), "{from} -> {to}: {error}");
    }
}

#[test]
fn traffic_cross_rules() {
    let root = assets_root();
    let vehicle = load_config::<VehicleConfig>(&root, VEHICLE_CONFIG).unwrap();
    let population = load_config::<PopulationConfig>(&root, POPULATION_CONFIG).unwrap();
    let cross = |tag: &str, from: &str, to: &str| {
        sabotaged(TRAFFIC_CONFIG, tag, from, to, |c: &TrafficConfig| {
            c.validate(tick())?;
            c.validate_cross(vehicle.max_speed, population.despawn_distance)
        })
    };
    // (28 + 16) x 0.1 = 4.4 > 4.0.
    let error = cross("traffic_reach", "reach: 5.0", "reach: 4.0");
    assert!(error.contains("switch.reach 4 must be >="), "{error}");
    let error = cross(
        "traffic_despawn",
        "in_view: (spawn: 70.0, despawn: 90.0)",
        "in_view: (spawn: 70.0, despawn: 160.0)",
    );
    assert!(error.contains("population despawn_distance"), "{error}");
}

#[test]
fn t15_police_rules_fire_with_their_keyword() {
    let police_error = |tag: &str, from: &str, to: &str| {
        sabotaged(POLICE_CONFIG, tag, from, to, |c: &EscalationConfig| {
            c.validate()?;
            c.validate_ring(150.0)
        })
    };
    let rows = [
        (
            "surround: false, cars: 1)",
            "surround: false, cars: 0)",
            "stars[0].cars must be >= 1",
        ),
        (
            "surround: true, cars: 4)",
            "surround: true, cars: 2)",
            "stars[3].cars 2 must be >= stars[2].cars 3",
        ),
        ("crew: 2", "crew: 0", "car.crew"),
        (
            "spawn_ring: (30.0, 60.0)",
            "spawn_ring: (30.0, 20.0)",
            "car.spawn_ring",
        ),
        (
            "spawn_ring: (30.0, 60.0)",
            "spawn_ring: (30.0, 160.0)",
            "car.spawn_ring (30.0, 160.0) must end below",
        ),
        ("goal_margin: 10.0", "goal_margin: 0.0", "car.goal_margin"),
        ("pull_over: 3.25", "pull_over: -1.0", "car.pull_over"),
        (
            "dismount_distance: 20.0",
            "dismount_distance: 45.0",
            "car.dismount_distance 45 must be < car.direct_chase_distance",
        ),
        (
            "pursuit_speed: 20.0",
            "pursuit_speed: 0.0",
            "car.pursuit_speed",
        ),
        (
            "routes_per_tick: 1",
            "routes_per_tick: 0",
            "car.routes_per_tick",
        ),
        (
            "spawns_per_tick: 1, pursuit",
            "spawns_per_tick: 0, pursuit",
            "car.spawns_per_tick",
        ),
        (
            "pull_out_seconds: 1.0",
            "pull_out_seconds: -1.0",
            "arrest.pull_out_seconds",
        ),
        (
            "pull_give_up_seconds: 3.0",
            "pull_give_up_seconds: 0.0",
            "arrest.pull_give_up_seconds",
        ),
        (
            "approach_distance: 20.0",
            "approach_distance: 0.0",
            "arrest.approach_distance",
        ),
        (
            "moving_seconds: 2.0",
            "moving_seconds: -1.0",
            "car.moving_seconds",
        ),
        (
            "junction_factor: 3.0",
            "junction_factor: 0.5",
            "car.junction_factor 0.5 must be finite and >= 1",
        ),
    ];
    for (i, (from, to, keyword)) in rows.into_iter().enumerate() {
        let error = police_error(&format!("police_t15_{i}"), from, to);
        assert!(error.contains(keyword), "{from} -> {to}: {error}");
    }
}

#[test]
fn t15_wanted_vehicle_damage_rules() {
    let error = sabotaged(
        WANTED_CONFIG,
        "wanted_car_view",
        "cop_car_view_distance: 50.0",
        "cop_car_view_distance: 30.0",
        |c: &WantedConfig| c.validate(),
    );
    assert!(
        error.contains("cop_car_view_distance 30 must be >="),
        "{error}"
    );
    let error = sabotaged(
        WANTED_CONFIG,
        "wanted_car_view_neg",
        "cop_car_view_distance: 50.0",
        "cop_car_view_distance: -50.0",
        |c: &WantedConfig| c.validate(),
    );
    assert!(error.contains("cop_car_view_distance -50"), "{error}");
    let vehicle = |tag: &str, from: &str, to: &str| {
        sabotaged(VEHICLE_CONFIG, tag, from, to, |c: &VehicleConfig| {
            c.validate()
        })
    };
    let error = vehicle(
        "veh_cabin",
        "half_extents: (1.25, 0.5, 1.0)",
        "half_extents: (1.25, 0.0, 1.0)",
    );
    assert!(error.contains("cabin.half_extents.y"), "{error}");
    let error = vehicle("veh_ap", "lookahead_min: 4.0", "lookahead_min: -4.0");
    assert!(error.contains("autopilot.lookahead_min"), "{error}");
    let error = vehicle("veh_ap_stuck", "stuck_seconds: 2.0", "stuck_seconds: 0.0");
    assert!(error.contains("autopilot.stuck_seconds"), "{error}");
    let error = sabotaged(
        DAMAGE_CONFIG,
        "dmg_share",
        "cabin_driver_share: 0.5",
        "cabin_driver_share: 1.5",
        |c: &DamageConfig| c.validate(),
    );
    assert!(error.contains("cabin_driver_share"), "{error}");
}
