//! Parked cars in the generated city (GDD §5, §12, T14), production composition: they stand still
//! and asleep on their spots, cost no rays, and a new city while driving leaves nothing behind.

mod common;
mod vehicle_support;

use bevy::{ecs::resource::IsResource, prelude::*};
use common::*;
use gta_sim::{
    flow::GameState,
    vehicle::{Driving, Vehicle, VehicleLoad},
    world::{City, CitySeed, WorldSource},
};
use std::{
    collections::HashSet,
    time::{Duration, Instant},
};
use vehicle_support::*;

fn all_entities(app: &App) -> HashSet<Entity> {
    app.world()
        .iter_entities()
        .filter(|e| !e.contains::<IsResource>())
        .map(|e| e.id())
        .collect()
}

fn state(app: &App) -> GameState {
    app.world().resource::<State<GameState>>().get().clone()
}

fn set_state(app: &mut App, to: GameState) {
    app.world_mut()
        .resource_mut::<NextState<GameState>>()
        .set(to.clone());
    for _ in 0..3 {
        app.update();
        if state(app) == to {
            return;
        }
    }
    panic!("GATE BROKEN: not {to:?} after 3 updates");
}

fn until_playing(app: &mut App) {
    let deadline = Instant::now() + Duration::from_secs(120);
    while state(app) != GameState::Playing {
        app.update();
        assert!(Instant::now() < deadline, "GATE BROKEN: city not ready");
        std::thread::sleep(Duration::from_millis(1));
    }
}

fn cars(app: &mut App) -> Vec<Entity> {
    app.world_mut()
        .query_filtered::<Entity, With<Vehicle>>()
        .iter(app.world())
        .collect()
}

fn spots(app: &App) -> Vec<Vec2> {
    app.world()
        .resource::<City>()
        .0
        .parking
        .iter()
        .map(|s| Vec2::new(s.position.x, s.position.y))
        .collect()
}

/// [correctness + perf] Every spot has its car, standing on it at rest height, asleep; parked cars
/// cast no suspension rays.
#[test]
fn parked_cars_rest_on_their_spots_asleep() {
    let mut app = city_app(1);
    settle(&mut app);
    run_ticks(&mut app, 128);
    let spots = spots(&app);
    let cars = cars(&mut app);
    assert_eq!(cars.len(), spots.len(), "one car per parking spot");
    assert!(!cars.is_empty(), "GATE BROKEN: no parking spots in seed 1");
    let rest = vehicle_cfg(&app).rest_height();
    let mut awake = 0;
    for &car in &cars {
        let at = position_of(&app, car);
        let spot = spots
            .iter()
            .copied()
            .min_by(|a, b| a.distance(flat(at)).total_cmp(&b.distance(flat(at))))
            .unwrap();
        assert!(
            spot.distance(flat(at)) < 0.05,
            "car at {at} is {:.3} m from its spot",
            spot.distance(flat(at))
        );
        assert!(
            (at.y - rest).abs() < 0.03,
            "car at {at}, rest height {rest}"
        );
        if app.world().get::<avian3d::prelude::Sleeping>(car).is_none() {
            awake += 1;
        }
    }
    assert_eq!(awake, 0, "{awake} of {} parked cars awake", cars.len());
    let load = *app.world().resource::<VehicleLoad>();
    assert_eq!(load.rays, 0, "parked cars cast rays: {load:?}");
}

#[test]
fn new_city_while_driving_leaves_nothing() {
    let mut app = composed_app(WorldSource::City { seed: 1 });
    app.update();
    let baseline = all_entities(&app);
    until_playing(&mut app);
    settle(&mut app);
    let me = position(&mut app);
    let car = cars(&mut app)
        .into_iter()
        .min_by(|a, b| {
            let da = position_of(&app, *a).distance(me);
            let db = position_of(&app, *b).distance(me);
            da.total_cmp(&db)
        })
        .expect("GATE BROKEN: no parked car");
    drive_in(&mut app, car);
    run_ticks(&mut app, 8);

    set_state(&mut app, GameState::Paused);
    app.world_mut().resource_mut::<CitySeed>().0 = 2;
    set_state(&mut app, GameState::Loading);
    let extra: Vec<Entity> = all_entities(&app).difference(&baseline).copied().collect();
    assert!(extra.is_empty(), "left behind by the new city: {extra:?}");

    until_playing(&mut app);
    let me = player(&mut app);
    assert!(
        app.world().get::<Driving>(me).is_none(),
        "driving in the new city"
    );
    let spots = spots(&app).len();
    let cars = cars(&mut app);
    assert_eq!(cars.len(), spots, "cars of the new city");
    for car in cars {
        assert_eq!(app.world().get::<Vehicle>(car).unwrap().driver, None);
    }
}
