//! AC5 and police car behaviour (GDD §5.3, §6.4): the car dispatcher keeps the row's cars and units
//! (crews count as units, foot cops leave the seats of missing cars free), cars close in over the lanes
//! and drop their crew, chase a driver, pick the crew up again, and free the slot when the crew dies.

mod common;
mod police_support;
mod traffic_support;
mod vehicle_support;
mod wanted_support;

use avian3d::prelude::*;
use bevy::prelude::*;
use common::*;
use gta_sim::{
    police::{CopState, CrewOf, PoliceCar, PoliceCarState, PoliceDispatcher, PoliceUnit},
    vehicle::Autopilot,
    world::HospitalSpawn,
};
use police_support::*;
use vehicle_support::*;
use wanted_support::*;

const TICKS: u32 = 1920;

/// Seed-1 city, nobody else about, the player on the hospital sidewalk with the view along the
/// sidewalk, armour so nobody dies, `heat` for the row and one tick (TASK-012).
fn city_at_row(stars: u8) -> App {
    let mut app = city_app(1);
    settle(&mut app);
    set_population(&mut app, |p| {
        p.max_civilians = 0;
        p.max_gang_members = 0;
    });
    let spawn = *app.world().resource::<HospitalSpawn>();
    let float = float_height_of(&app);
    place_player(&mut app, spawn.point + Vec3::Y * float);
    run_ticks(&mut app, 16);
    set_player_armor(&mut app, 1.0e6);
    set_view(&mut app, Some(chase_view(spawn.point, spawn.along)));
    let heat = wanted_cfg(&app).stars[usize::from(stars) - 1].heat;
    raise_heat(&mut app, heat);
    stay_hostile(&mut app);
    app
}

/// Arrest-row cops shoot instead of arresting (a passive player would be Busted at 1 star).
fn stay_hostile(app: &mut App) {
    app.world_mut()
        .resource_mut::<gta_sim::police::PoliceAlert>()
        .hostile_left = 1.0e6;
}

pub fn police_cars(app: &mut App) -> Vec<(Entity, PoliceCar)> {
    let mut cars: Vec<(Entity, PoliceCar)> = app
        .world_mut()
        .query::<(Entity, &PoliceCar)>()
        .iter(app.world())
        .map(|(e, c)| (e, c.clone()))
        .collect();
    cars.sort_by_key(|c| c.0.to_bits());
    cars
}

fn active_cars(app: &mut App) -> Vec<(Entity, PoliceCar)> {
    police_cars(app)
        .into_iter()
        .filter(|(_, c)| c.active())
        .collect()
}

/// Foot units (not dead, not leaving) and the crews aboard active cars.
fn foot_and_aboard(app: &mut App) -> (u32, u32) {
    let foot = units(app)
        .iter()
        .filter(|(_, u)| !matches!(u.state, CopState::Dead | CopState::Leave))
        .count() as u32;
    let aboard = active_cars(app)
        .iter()
        .map(|(_, c)| c.crew.len() as u32)
        .sum();
    (foot, aboard)
}

/// `GATE BROKEN` unless the wanted level is still on row `stars`; then the named mutation "the search
/// never clears it" (`hidden` back to 0) so a player driving off stays wanted.
fn hold_row(app: &mut App, stars: u8) {
    let w = wanted(app);
    assert_eq!(
        w.stars, stars,
        "GATE BROKEN: the wanted level left row {stars}: {w:?}"
    );
    app.world_mut()
        .resource_mut::<gta_sim::wanted::WantedLevel>()
        .hidden = 0.0;
}

fn cars_follow_row(stars: u8) {
    cars_follow_row_with(stars, |_| {});
}

/// Returns whether a tick had the row's cars out while the unit cap left room for one more crew
/// (the car cap, not the unit cap, held the dispatcher back).
fn cars_follow_row_with(stars: u8, mutate: impl FnOnce(&mut App)) -> bool {
    let mut app = city_at_row(stars);
    mutate(&mut app);
    let row = esc(&app).stars[usize::from(stars) - 1].clone();
    let crew = esc(&app).car.crew;
    let mut reached = false;
    let mut car_cap_bound = false;
    let mut seen = std::collections::HashSet::new();
    for tick in 0..TICKS {
        run_ticks(&mut app, 1);
        hold_row(&mut app, stars);
        // A new car faces its target (no block to loop before it can head there).
        let target = wanted(&app).last_known;
        for (car, _) in police_cars(&mut app) {
            if !seen.insert(car) {
                continue;
            }
            let to = target.expect("GATE BROKEN: a police car without a last known position")
                - position_of(&app, car);
            let heading = forward_of(&app, car).dot(to.with_y(0.0));
            assert!(
                heading > 0.0,
                "row {stars} tick {tick}: new police car {car} faces away from its target ({heading})"
            );
        }
        let cars = active_cars(&mut app).len() as u32;
        let dispatcher = *app.world().resource::<PoliceDispatcher>();
        let (foot, aboard) = foot_and_aboard(&mut app);
        assert!(
            cars <= row.cars,
            "row {stars} tick {tick}: {cars} cars > {}",
            row.cars
        );
        assert!(
            dispatcher.units <= row.units,
            "row {stars} tick {tick}: {} units > {}",
            dispatcher.units,
            row.units
        );
        assert_eq!(
            foot + aboard,
            dispatcher.units,
            "row {stars} tick {tick}: foot {foot} + aboard {aboard} != dispatcher {}",
            dispatcher.units
        );
        reached |= cars == row.cars;
        car_cap_bound |= cars == row.cars && dispatcher.units + crew <= row.units;
    }
    let (foot, aboard) = foot_and_aboard(&mut app);
    eprintln!(
        "row {stars}: cars {:?}, foot {foot}, aboard {aboard}",
        police_cars(&mut app)
            .iter()
            .map(|(_, c)| (c.state, c.crew.len()))
            .collect::<Vec<_>>()
    );
    assert!(
        reached,
        "GATE BROKEN: row {stars} never reached {} cars",
        row.cars
    );
    car_cap_bound
}

#[test]
fn cars_follow_row_1() {
    cars_follow_row(1);
}

#[test]
fn cars_follow_row_2() {
    cars_follow_row(2);
}

#[test]
fn cars_follow_row_3() {
    cars_follow_row(3);
}

#[test]
fn cars_follow_row_4() {
    cars_follow_row(4);
}

#[test]
fn cars_follow_row_5() {
    cars_follow_row(5);
}

/// In the shipped rows the unit cap binds first (cars × crew fill the units), so the rows above never
/// test the `cars` column. Named mutation: row 2 gets 12 units, room for 6 crews; the dispatcher must
/// still stop at the row's 2 cars (QA bug 5).
#[test]
fn cars_cap_binds_with_spare_units() {
    let bound = cars_follow_row_with(2, |app| {
        app.world_mut()
            .resource_mut::<gta_sim::police::EscalationConfig>()
            .stars[1]
            .units = 12;
    });
    assert!(
        bound,
        "GATE BROKEN: the unit cap still bound first (no tick with 2 cars and room for a third crew)"
    );
}

/// A car closes in over the lanes and drops its crew near the player. Other cars may get out farther
/// away first (held up in traffic for `car.blocked_seconds`, or a route end on the far side of the
/// street), so the gate follows the first car that gets out near the player.
#[test]
fn car_closes_in_and_dismounts() {
    let mut app = city_at_row(2);
    let dismount = esc(&app).car.dismount_distance;
    let mut spawned_at = std::collections::HashMap::new();
    let mut near = None;
    for tick in 0..TICKS {
        run_ticks(&mut app, 1);
        hold_row(&mut app, 2);
        let player = position(&mut app);
        for (car, c) in police_cars(&mut app) {
            let d = (position_of(&app, car) - player).with_y(0.0).length();
            spawned_at.entry(car).or_insert(d);
            if c.state == PoliceCarState::Dismounted && d <= dismount + 6.0 {
                near.get_or_insert((car, d, tick));
            }
        }
        if near.is_some() {
            break;
        }
    }
    let (car, d, tick) = near.expect("no police car dismounted near the player in 30 s");
    let from = spawned_at[&car];
    eprintln!("spawned {from:.1} m away, dismounted at {d:.1} m after {tick} ticks");
    assert!(d < from, "the car did not close in: {from} -> {d}");
    run_ticks(&mut app, 2);
    let crew: Vec<Entity> = app
        .world_mut()
        .query::<(Entity, &CrewOf, &PoliceUnit)>()
        .iter(app.world())
        .filter(|(_, c, _)| c.car == car)
        .map(|(e, ..)| e)
        .collect();
    assert!(!crew.is_empty(), "no crew of the dismounted car");
}

/// Seed-1 city at `stars` with the player on the sidewalk beside the first parked car, looking along
/// its avenue; returns the app, that car and the avenue direction.
fn city_by_parked_car(stars: u8) -> (App, Entity, Vec3) {
    let mut app = city_app(1);
    settle(&mut app);
    set_population(&mut app, |p| {
        p.max_civilians = 0;
        p.max_gang_members = 0;
    });
    let (spot, curb, offset) = {
        let world = app.world();
        let params = &world.resource::<gta_sim::world::CityParamsRes>().0;
        // The parked car nearest the centre that faces the centre: a long drive ahead.
        let spot = *world
            .resource::<gta_sim::world::City>()
            .0
            .parking
            .iter()
            .filter(|s| s.heading.dot(-s.position) > 0.0)
            .min_by(|a, b| a.position.length().total_cmp(&b.position.length()))
            .expect("GATE BROKEN: no parked car facing the centre");
        (
            spot,
            params.roads.curb_height,
            params.parking.curb_offset + params.roads.avenue.sidewalk / 2.0,
        )
    };
    let heading = Vec3::new(spot.heading.x, 0.0, spot.heading.y);
    let right = Vec3::new(-heading.z, 0.0, heading.x);
    let feet = Vec3::new(spot.position.x, curb, spot.position.y) + right * offset;
    let float = float_height_of(&app);
    place_player(&mut app, feet + Vec3::Y * float);
    run_ticks(&mut app, 16);
    set_player_armor(&mut app, 1.0e6);
    set_view(&mut app, Some(chase_view(feet, heading)));
    let at = Vec3::new(spot.position.x, 0.0, spot.position.y);
    let parked = app
        .world_mut()
        .query_filtered::<(Entity, &Position), (With<gta_sim::vehicle::Vehicle>, Without<PoliceCar>)>()
        .iter(app.world())
        .min_by(|a, b| {
            (a.1.0 - at).with_y(0.0).length().total_cmp(&(b.1.0 - at).with_y(0.0).length())
        })
        .map(|(e, _)| e)
        .expect("GATE BROKEN: no parked car");
    let heat = wanted_cfg(&app).stars[usize::from(stars) - 1].heat;
    raise_heat(&mut app, heat);
    stay_hostile(&mut app);
    (app, parked, heading)
}

/// Keeps the player's car near `speed` m/s on the traffic lanes: steers (pure pursuit) at the point
/// 8 m ahead on the nearest lane heading its way, straight on across intersections.
fn cruise(app: &mut App, car: Entity, speed: f32) {
    let cfg = app
        .world()
        .resource::<gta_sim::vehicle::VehicleConfig>()
        .clone();
    let graph = app
        .world()
        .resource::<gta_sim::traffic::TrafficGraph>()
        .clone();
    let (p, f) = (position_of(app, car), forward_of(app, car));
    let v = velocity_of(app, car).dot(f);
    let (lane, _) = graph
        .lanes()
        .iter()
        .enumerate()
        .filter(|(_, l)| l.dir.dot(f) > 0.7)
        .map(|(k, l)| {
            let s = (p - l.from).dot(l.dir).clamp(0.0, l.length);
            (k, (l.from + l.dir * s - p).with_y(0.0).length())
        })
        .min_by(|a, b| a.1.total_cmp(&b.1))
        .expect("GATE BROKEN: no lane ahead of the player's car");
    let l = graph.lane(lane as u32);
    let s = (p - l.from).dot(l.dir) + 8.0;
    let target = if s <= l.length {
        l.from + l.dir * s
    } else {
        let straight = l
            .out
            .iter()
            .map(|&c| graph.lane(graph.connector(c).to_lane))
            .max_by(|a, b| a.dir.dot(l.dir).total_cmp(&b.dir.dot(l.dir)))
            .expect("GATE BROKEN: dead-end lane");
        straight.from + straight.dir * (s - l.length).min(straight.length)
    };
    let steer = gta_sim::vehicle::pursuit_steer(&cfg, p, f, v, target);
    set_drive(app, |d| {
        d.throttle = ((speed - v) * 0.5).clamp(-1.0, 1.0);
        d.steer = steer;
    });
}

fn crew_of(app: &mut App, car: Entity) -> Vec<Entity> {
    app.world_mut()
        .query::<(Entity, &CrewOf, &PoliceUnit)>()
        .iter(app.world())
        .filter(|(_, c, u)| c.car == car && u.state != CopState::Dead)
        .map(|(e, ..)| e)
        .collect()
}

/// Runs until a police car is Dismounted (at most `limit` ticks).
fn until_dismounted(app: &mut App, stars: u8, limit: u32) -> Entity {
    for _ in 0..limit {
        run_ticks(app, 1);
        hold_row(app, stars);
        if let Some((car, _)) = police_cars(app)
            .into_iter()
            .find(|(_, c)| c.state == PoliceCarState::Dismounted)
        {
            return car;
        }
    }
    panic!("GATE BROKEN: no police car dismounted in {limit} ticks");
}

#[test]
fn car_chases_a_driver() {
    let (mut app, car, _) = city_by_parked_car(2);
    // The subject is the police car: no traffic between it and the driver (a traffic car blocks
    // the sight line and the lane).
    traffic_support::set_traffic(&mut app, |t| t.bubble.max_cars = 0);
    drive_in(&mut app, car);
    let chase = esc(&app).car.direct_chase_distance;
    let mut first = None;
    let mut closest = f32::INFINITY;
    let mut chased = false;
    for _ in 0..TICKS {
        cruise(&mut app, car, 10.0);
        // Named mutation: the player is reported all the time, so the cars route to the driver.
        let me = position(&mut app);
        app.world_mut()
            .resource_mut::<gta_sim::wanted::WantedLevel>()
            .last_known = Some(me);
        run_ticks(&mut app, 1);
        hold_row(&mut app, 2);
        let me = position(&mut app);
        for (police, c) in police_cars(&mut app) {
            let d = (position_of(&app, police) - me).with_y(0.0).length();
            first.get_or_insert(d);
            closest = closest.min(d);
            if c.state == PoliceCarState::Chase {
                assert!(d <= chase + 1.0, "chasing from {d} m");
                let target = app.world().get::<Autopilot>(police).unwrap().target;
                assert!(
                    (target - me).with_y(0.0).length() < 0.5,
                    "chase target {target}, player {me}"
                );
                chased = true;
            }
        }
    }
    let first = first.expect("GATE BROKEN: no police car dispatched");
    eprintln!("first {first:.1} m, closest {closest:.1} m, chased {chased}");
    assert!(closest < first, "the police cars never closed in");
    assert!(chased, "no police car chased the driver");
}

#[test]
fn crew_reboards() {
    let (mut app, car, _) = city_by_parked_car(2);
    // The subject is the police car: no traffic queue between the player and the open road.
    traffic_support::set_traffic(&mut app, |t| t.bubble.max_cars = 0);
    let police = until_dismounted(&mut app, 2, TICKS);
    run_ticks(&mut app, 2);
    assert!(
        !crew_of(&mut app, police).is_empty(),
        "GATE BROKEN: no crew out"
    );
    drive_in(&mut app, car);
    let start = position(&mut app);
    let mut away = false;
    let mut boarded = None;
    for tick in 0..(3 * TICKS) {
        if !away {
            cruise(&mut app, car, 8.0);
        } else {
            set_drive(&mut app, |d| {
                d.throttle = 0.0;
                d.steer = 0.0;
            });
        }
        run_ticks(&mut app, 1);
        hold_row(&mut app, 2);
        away |= (position(&mut app) - start).with_y(0.0).length() >= 60.0;
        let c = app
            .world()
            .get::<PoliceCar>(police)
            .cloned()
            .expect("GATE BROKEN: the police car was despawned");
        if away && c.state == PoliceCarState::Respond && crew_of(&mut app, police).is_empty() {
            boarded = Some((tick, c.crew.len()));
            break;
        }
    }
    let (tick, aboard) = boarded.expect("the crew never re-boarded");
    eprintln!("re-boarded after {tick} ticks, {aboard} aboard");
    assert!(aboard >= 1);
}

#[test]
fn dead_crew_frees_the_slot() {
    let mut app = city_at_row(2);
    let police = until_dismounted(&mut app, 2, TICKS);
    run_ticks(&mut app, 2);
    let crew = crew_of(&mut app, police);
    assert!(!crew.is_empty(), "GATE BROKEN: no crew out");
    for cop in crew {
        set_health_of(&mut app, cop, |h| h.current = 0.0);
    }
    let mut abandoned = false;
    let mut replaced = false;
    for _ in 0..TICKS {
        run_ticks(&mut app, 1);
        hold_row(&mut app, 2);
        let state = app.world().get::<PoliceCar>(police).map(|c| c.state);
        abandoned |= state == Some(PoliceCarState::Abandoned);
        let active = active_cars(&mut app);
        assert!(active.len() <= 2);
        replaced |= abandoned && active.len() == 2 && active.iter().all(|(e, _)| *e != police);
        if replaced {
            break;
        }
    }
    assert!(abandoned, "the car of a dead crew was not abandoned");
    assert!(replaced, "no new car took the free slot");
}

/// A crewed police car (no lane graph on the test floor: it stands still) at `centre` facing `yaw`.
fn parked_police_car(
    app: &mut App,
    centre: Vec2,
    yaw_deg: f32,
    crew: Vec<gta_sim::police::UnitKind>,
) -> Entity {
    let car = spawn_car(app, centre, yaw_deg);
    app.world_mut().entity_mut(car).insert(PoliceCar {
        state: PoliceCarState::Respond,
        crew,
        stopped: 0.0,
        moving: 0.0,
        blocked: 0.0,
        reboard_left: 10.0,
    });
    car
}

#[test]
fn car_sees_at_50_m() {
    use gta_sim::police::UnitKind::Patrol;
    // The player at the origin; cars 45 m and 55 m along +Z facing it (yaw PI faces +Z: 0 faces −Z).
    for (distance, crew, seen) in [
        (45.0, vec![Patrol], true),
        (45.0, vec![], false),
        (55.0, vec![Patrol], false),
    ] {
        let mut app = headless_app();
        settle(&mut app);
        parked_police_car(
            &mut app,
            Vec2::new(-30.0, distance - 30.0),
            0.0,
            crew.clone(),
        );
        let float = float_height_of(&app);
        place_player(&mut app, Vec3::new(-30.0, float, -30.0));
        set_heat(&mut app, 180);
        run_ticks(&mut app, 2);
        assert_eq!(
            wanted(&app).seen,
            seen,
            "car {distance} m away with crew {crew:?}: {:?}",
            wanted(&app)
        );
    }
}

#[test]
fn crewed_car_witnesses_theft() {
    use gta_sim::police::UnitKind::Patrol;
    for (crew, heat) in [(vec![Patrol], 15), (vec![], 0)] {
        let mut app = headless_app();
        settle(&mut app);
        parked_police_car(&mut app, Vec2::new(-30.0, 0.0), 0.0, crew.clone());
        let car = spawn_car(&mut app, Vec2::new(-30.0, -25.0), 0.0);
        drive_in(&mut app, car);
        run_ticks(&mut app, 1);
        assert_eq!(
            wanted(&app).heat,
            heat,
            "theft witnessed by a car with crew {crew:?}"
        );
    }
}
