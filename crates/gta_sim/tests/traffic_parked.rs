//! Traffic never touches parked cars (correctness, GDD §5.2): traffic runs on the inner lanes, parked
//! cars stand in the avenue curb lanes. Seeds 1..=3, the player on an avenue sidewalk by a parked car,
//! a full bubble for 4096 ticks: no contact, no switch for a parked car, no footprint overlap.

mod common;
mod traffic_support;

use avian3d::prelude::*;
use bevy::{ecs::message::MessageCursor, prelude::*};
use common::*;
use gta_sim::{
    traffic::{TrafficCar, TrafficStats},
    vehicle::Vehicle,
    world::{City, CityParamsRes},
};
use std::collections::HashSet;
use traffic_support::*;

fn run_seed(seed: u64) {
    let mut app = city_app(seed);
    set_population(&mut app, |p| {
        p.max_civilians = 0;
        p.max_gang_members = 0;
    });
    let (spot, curb, offset) = {
        let world = app.world();
        let params = &world.resource::<CityParamsRes>().0;
        let spot = world.resource::<City>().0.parking[0];
        (
            spot,
            params.roads.curb_height,
            params.parking.curb_offset + params.roads.avenue.sidewalk / 2.0,
        )
    };
    let heading = Vec3::new(spot.heading.x, 0.0, spot.heading.y);
    let right = Vec3::new(-heading.z, 0.0, heading.x);
    let feet = Vec3::new(spot.position.x, curb, spot.position.y) + right * offset;
    let float = app
        .world()
        .resource::<gta_sim::character::LocomotionConfig>()
        .float_height;
    place_player(&mut app, feet + Vec3::Y * float);
    set_view(&mut app, Some(chase_view(feet, heading)));
    let parked: HashSet<Entity> = app
        .world_mut()
        .query_filtered::<Entity, (With<Vehicle>, Without<TrafficCar>)>()
        .iter(app.world())
        .collect();
    assert!(
        !parked.is_empty(),
        "GATE BROKEN: seed {seed} has no parked cars"
    );
    for _ in 0..1280 {
        run_ticks(&mut app, 1);
        if app.world().resource::<TrafficStats>().cars >= 12 {
            break;
        }
    }
    let full = app.world().resource::<TrafficStats>().cars;
    assert!(
        full >= 12,
        "GATE BROKEN: seed {seed}: only {full} traffic cars"
    );
    let mut contacts: MessageCursor<CollisionStart> = app
        .world()
        .resource::<Messages<CollisionStart>>()
        .get_cursor_current();
    let mut dynamic: HashSet<Entity> = HashSet::new();
    let mut max_cars = 0;
    for tick in 0..4096 {
        run_ticks(&mut app, 1);
        let traffic: Vec<(Entity, bool)> = app
            .world_mut()
            .query::<(Entity, &RigidBody, &TrafficCar)>()
            .iter(app.world())
            .map(|(e, b, _)| (e, b.is_dynamic()))
            .collect();
        max_cars = max_cars.max(traffic.len());
        let is_traffic = |e: Entity| traffic.iter().any(|t| t.0 == e);
        for m in contacts.read(app.world().resource::<Messages<CollisionStart>>()) {
            let (a, b) = (
                m.body1.unwrap_or(m.collider1),
                m.body2.unwrap_or(m.collider2),
            );
            assert!(
                !((is_traffic(a) && parked.contains(&b)) || (is_traffic(b) && parked.contains(&a))),
                "seed {seed} tick {tick}: a traffic car touched a parked car"
            );
        }
        let parked_rects: Vec<_> = parked
            .iter()
            .filter(|&&p| app.world().get_entity(p).is_ok())
            .map(|&p| footprint(&app, p))
            .collect();
        for &(car, is_dynamic) in &traffic {
            let rect = footprint(&app, car);
            // The test's own attribution: a car that just turned dynamic next to a parked car.
            if is_dynamic && dynamic.insert(car) {
                let near = parked_rects
                    .iter()
                    .any(|p| p.centre.distance(rect.centre) < 6.0);
                assert!(
                    !near,
                    "seed {seed} tick {tick}: a car turned dynamic beside a parked car"
                );
            }
            for p in parked_rects
                .iter()
                .filter(|p| p.centre.distance(rect.centre) < 6.0)
            {
                assert!(
                    !obb_overlap(&rect, p),
                    "seed {seed} tick {tick}: a traffic car overlaps a parked car"
                );
            }
        }
    }
    let causes = app.world().resource::<TrafficStats>().switches_by_cause;
    eprintln!("seed {seed}: {max_cars} cars at most, switches {causes:?}");
    assert_eq!(causes[1], 0, "seed {seed}: vehicle switches {causes:?}");
}

#[test]
fn traffic_never_touches_parked_cars() {
    for seed in 1..=3 {
        run_seed(seed);
    }
}
