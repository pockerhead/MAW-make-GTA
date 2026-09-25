//! A driver who stops is dealt with (GDD §5.3, §6.4 T15, TASK-016 QA rounds 1-2): police cars close in
//! on a player sitting in a stopped car (or one stuck at a wall) farther than `reboard_distance` and let
//! their crews out; only a driver who moves away keeps the crews aboard. At 1 star the crew then pulls
//! the driver out and arrests him, on seeds 1-3, around the traffic queued behind his car. A
//! stop-and-go driver does not make the crews hop out and back in.

mod common;
mod police_support;
mod vehicle_support;
mod wanted_support;

use avian3d::prelude::*;
use bevy::prelude::*;
use common::*;
use gta_sim::{
    character::{HealthConfig, LocomotionConfig},
    flow::GameState,
    police::{PoliceAlert, PoliceCar, PoliceCarState},
    traffic::{Segment, TrafficGraph},
    world::{City, CityLandmarks, HospitalSpawn},
};
use police_support::*;
use vehicle_support::*;
use wanted_support::*;

/// A city of `seed`, nobody else about but traffic; the player sits in a car on the city lane nearest
/// `spot`, 8 m or more from both lane ends, the view along the lane, at `stars`. With `wall`, a wall
/// stands 0.5 m in front of the bumper. Returns (app, car).
fn stopped_in_city(seed: u64, stars: u8, wall: bool, spot: fn(&App) -> Vec3) -> (App, Entity) {
    let mut app = city_app(seed);
    settle(&mut app);
    set_population(&mut app, |p| {
        p.max_civilians = 0;
        p.max_gang_members = 0;
    });
    let at = spot(&app);
    let graph = app.world().resource::<TrafficGraph>().clone();
    let Some((Segment::Lane(lane), s, _)) = graph.nearest(at) else {
        panic!("GATE BROKEN: the nearest segment to the hospital is not a lane");
    };
    let s = s.clamp(8.0, graph.lane(lane).length - 8.0);
    let (p, t) = graph.pose(Segment::Lane(lane), s);
    let yaw = (-t.x).atan2(-t.z).to_degrees();
    let car = spawn_car(&mut app, Vec2::new(p.x, p.z), yaw);
    drive_in(&mut app, car);
    set_player_armor(&mut app, 1.0e6);
    if wall {
        let half = vehicle_cfg(&app).half_extents();
        let front = p + t * (half.z + 0.5 + 0.25);
        let side = Vec3::new(-t.z, 0.0, t.x);
        let size = side.abs() * 8.0 + t.abs() * 0.5 + Vec3::Y * 3.0;
        spawn_wall(&mut app, front + Vec3::Y * 1.5, size);
    }
    set_view(&mut app, Some(chase_view(p, t)));
    let heat = wanted_cfg(&app).stars[usize::from(stars) - 1].heat;
    raise_heat(&mut app, heat);
    (app, car)
}

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

fn police_cars(app: &mut App) -> Vec<(Entity, PoliceCar, f32)> {
    let me = position(app);
    let mut cars: Vec<_> = app
        .world_mut()
        .query::<(Entity, &PoliceCar, &Position)>()
        .iter(app.world())
        .map(|(e, c, p)| (e, c.clone(), (p.0 - me).with_y(0.0).length()))
        .collect();
    cars.sort_by_key(|c| c.0.to_bits());
    cars
}

/// 1 star, the player sits in a stopped stolen car: a police car lets its crew out and the driver is
/// pulled out and BUSTED.
fn hospital(app: &App) -> Vec3 {
    app.world().resource::<HospitalSpawn>().point
}

fn plaza(app: &App) -> Vec3 {
    app.world().resource::<CityLandmarks>().plaza_center
}

fn park(app: &App) -> Vec3 {
    app.world().resource::<CityLandmarks>().park_center
}

/// The sidewalk at the plaza tower (QA round 2's plaza spot).
fn tower(app: &App) -> Vec3 {
    let layout = &app.world().resource::<City>().0;
    let margin = app.world().resource::<HealthConfig>().pickups.spacing;
    let params = city_params(app);
    let (anchor, _) =
        citygen::sidewalk_anchor(layout, params, layout.landmarks.tower as usize, margin)
            .expect("GATE BROKEN: no sidewalk anchor at the tower");
    Vec3::new(anchor.x, params.roads.curb_height, anchor.y)
}

/// The longest a 1-star stopped driver may wait for BUSTED, from the data: a car from the far edge of
/// its spawn ring, held up for `blocked_seconds`, the driver stopped `stopped_seconds`; the crew runs
/// the ring back on foot, pulls (with the give-up) and holds the arrest.
fn busted_within(app: &App) -> f32 {
    let esc = esc(app);
    let run = app.world().resource::<LocomotionConfig>().run_speed;
    let (car, arrest) = (&esc.car, &esc.arrest);
    car.spawn_ring.1 / car.pursuit_speed
        + car.blocked_seconds
        + car.stopped_seconds
        + car.spawn_ring.1 / run
        + arrest.pull_out_seconds
        + arrest.pull_give_up_seconds
        + arrest.seconds
}

/// Longest time (ticks) any police car stood (at most exit speed) in `Respond` farther than
/// `reboard_distance` from the player, and that distance; outside and inside intersections apart.
#[derive(Default)]
struct RespondWait {
    per_car: std::collections::HashMap<Entity, (u32, bool)>,
    worst: (u32, f32),
    worst_in_box: (u32, f32),
    dismounted: std::collections::HashSet<Entity>,
}

impl RespondWait {
    fn watch(&mut self, app: &mut App) {
        let esc = esc(app);
        let exit = vehicle_cfg(app).exit_max_speed;
        let half = vehicle_cfg(app).half_extents();
        let graph = app.world().resource::<TrafficGraph>().clone();
        for (police, c, d) in police_cars(app) {
            if c.state == PoliceCarState::Dismounted {
                self.dismounted.insert(police);
            }
            let at = position_of(app, police);
            let held = c.state == PoliceCarState::Respond
                && d > esc.car.reboard_distance
                && velocity_of(app, police).length() <= exit;
            let (w, in_box) = self.per_car.entry(police).or_default();
            *w = if held { *w + 1 } else { 0 };
            // A wait that touched a box counts as a wait in the box.
            *in_box = held && (*in_box || graph.in_junction(at, half.z));
            let worst = if *in_box {
                &mut self.worst_in_box
            } else {
                &mut self.worst
            };
            if *w > worst.0 {
                *worst = (*w, d);
            }
        }
    }

    /// A car held up gets out after `blocked_seconds` (inside an intersection after `blocked_seconds ×
    /// junction_factor`); one more second covers a door that is not free at once and the stop before it.
    fn check(&self, app: &App, what: &str) -> Result<(), String> {
        let car = esc(app).car;
        let limit = car.blocked_seconds + 1.0;
        let limit_in_box = car.blocked_seconds * car.junction_factor + 1.0;
        let worst = self.worst.0 as f32 / 64.0;
        let worst_in_box = self.worst_in_box.0 as f32 / 64.0;
        eprintln!(
            "{what}: {} cars let their crews out; longest Respond wait beyond reboard {worst:.2} s at {:.1} m, in a box {worst_in_box:.2} s at {:.1} m",
            self.dismounted.len(),
            self.worst.1,
            self.worst_in_box.1
        );
        if self.dismounted.is_empty() {
            return Err(format!("{what}: no police car let its crew out"));
        }
        if worst >= limit || worst_in_box >= limit_in_box {
            return Err(format!(
                "{what}: a police car stood {worst:.2} s ({worst_in_box:.2} s in a box) in Respond from a stopped driver (limits {limit} s, {limit_in_box} s)"
            ));
        }
        Ok(())
    }
}

/// 1 star, the player sits in a stopped stolen car on seeds 1-3 (the traffic queues up behind him):
/// the police cars let their crews out wherever they are held up, and the driver is pulled out and
/// BUSTED within `busted_within` seconds.
#[test]
fn a_stopped_driver_is_approached_and_busted() {
    let mut failures = Vec::new();
    for seed in 1..=3 {
        for (spot_name, spot) in [
            ("hospital", hospital as fn(&App) -> Vec3),
            ("plaza", plaza),
            ("tower", tower),
            ("park", park),
        ] {
            let name = format!("seed {seed} {spot_name}");
            if let Err(e) = stopped_and_busted(seed, &name, spot) {
                failures.push(e);
            }
        }
    }
    assert!(failures.is_empty(), "{failures:#?}");
}

fn stopped_and_busted(seed: u64, name: &str, spot: fn(&App) -> Vec3) -> Result<(), String> {
    let (mut app, _) = stopped_in_city(seed, 1, false, spot);
    let limit = busted_within(&app);
    let mut wait = RespondWait::default();
    let mut ticks = 0u32;
    while ticks < (limit * 64.0) as u32 && game_state(&app) == GameState::Playing {
        set_drive(&mut app, |d| {
            d.throttle = 0.0;
            d.steer = 0.0;
        });
        run_ticks(&mut app, 1);
        ticks += 1;
        if game_state(&app) != GameState::Playing {
            break;
        }
        hold_row(&mut app, 1);
        wait.watch(&mut app);
    }
    let busted = game_state(&app) == GameState::Busted;
    eprintln!(
        "{name}: busted {busted} after {:.2} s (limit {limit:.1} s)",
        ticks as f32 / 64.0
    );
    wait.check(&app, name)?;
    if !busted {
        return Err(format!(
            "{name}: the stopped driver was not busted within {limit:.1} s"
        ));
    }
    Ok(())
}

/// 5 stars, the player's car is stuck at a wall with the throttle held (QA runtime: four cars waited
/// 33-39 m away for more than 25 s).
#[test]
fn a_driver_stuck_at_a_wall_is_approached() {
    let (mut app, car) = stopped_in_city(1, 5, true, hospital);
    app.world_mut().resource_mut::<PoliceAlert>().hostile_left = 1.0e6;
    let exit = vehicle_cfg(&app).exit_max_speed;
    let mut wait = RespondWait::default();
    let mut top_speed = 0.0f32;
    for tick in 0..64 * 60 {
        set_drive(&mut app, |d| {
            d.throttle = 1.0;
            d.steer = 0.0;
        });
        run_ticks(&mut app, 1);
        hold_row(&mut app, 5);
        if tick > 64 {
            top_speed = top_speed.max(velocity_of(&app, car).length());
        }
        wait.watch(&mut app);
    }
    assert!(
        top_speed <= exit,
        "GATE BROKEN: the player's car is not stuck ({top_speed} m/s)"
    );
    wait.check(&app, "stuck at a wall").unwrap();
    let row = esc(&app).stars[4].cars as usize;
    assert!(
        wait.dismounted.len() >= row,
        "only {} of {row} police cars let their crews out for the stuck driver",
        wait.dismounted.len()
    );
}

/// Stop and go at 2 stars (t15's rhythm: throttle 1.5 s, coast 1.5 s, straight on for 40 s; QA
/// round 2, seed 3: 7 dismounts 132-145 m away and 4 re-boards in 25 s): a crew gets out only for a
/// driver stopped `stopped_seconds` and gets back in only once he has moved `moving_seconds`, so each
/// car gets out and back in at most once.
#[test]
fn stop_and_go_does_not_make_crews_hop() {
    let mut failures = Vec::new();
    for seed in [2, 3] {
        let (mut app, car) = stopped_in_city(seed, 2, false, hospital);
        let mut states: std::collections::HashMap<Entity, PoliceCarState> = Default::default();
        let mut hops: std::collections::HashMap<Entity, u32> = Default::default();
        let (mut stopped_ticks, mut top) = (0u32, 0.0f32);
        let exit = vehicle_cfg(&app).exit_max_speed;
        for tick in 0..64 * 40u32 {
            let throttle = if (tick / 96) % 2 == 0 { 1.0 } else { 0.0 };
            set_drive(&mut app, |d| {
                d.throttle = throttle;
                d.steer = 0.0;
            });
            run_ticks(&mut app, 1);
            hold_row(&mut app, 2);
            let v = velocity_of(&app, car).length();
            top = top.max(v);
            stopped_ticks += u32::from(v <= exit);
            for (police, c, _) in police_cars(&mut app) {
                let before = states.insert(police, c.state);
                if matches!(
                    (before, c.state),
                    (Some(PoliceCarState::Respond), PoliceCarState::Dismounted)
                        | (Some(PoliceCarState::Dismounted), PoliceCarState::Respond)
                ) {
                    *hops.entry(police).or_default() += 1;
                }
            }
        }
        eprintln!(
            "seed {seed}: player car top {top:.1} m/s, {:.1} s at most exit speed; hops per car {:?}",
            stopped_ticks as f32 / 64.0,
            hops.values().collect::<Vec<_>>()
        );
        if top <= exit {
            failures.push(format!(
                "GATE BROKEN: seed {seed}: the player's car never moved"
            ));
        }
        if states.is_empty() {
            failures.push(format!("GATE BROKEN: seed {seed}: no police car came"));
        }
        if let Some((police, n)) = hops.iter().find(|(_, n)| **n > 2) {
            failures.push(format!(
                "seed {seed}: police car {police} went Respond <-> Dismounted {n} times"
            ));
        }
    }
    assert!(failures.is_empty(), "{failures:#?}");
}
