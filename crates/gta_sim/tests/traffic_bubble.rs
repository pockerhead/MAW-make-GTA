//! AC3 and the Vermeij bands (correctness, GDD §5.2): traffic cars despawn only after 2 s out of
//! frame (the test's own frame test from the published view: a chassis-top corner or the centre line
//! in the cone, within 90 m) and beyond 25 m; steady spawns land in frame at 70-90 m or out of frame
//! at 15-25 m; a car in frame is kept, a far one in the cone goes; abandoned cars go too.

mod common;
mod traffic_support;
mod vehicle_support;

use avian3d::prelude::*;
use bevy::prelude::*;
use common::*;
use gta_sim::{
    population::ViewCone,
    traffic::{Segment, TrafficCar, TrafficConfig, TrafficMode, TrafficPhase},
    vehicle::VehicleConfig,
    world::HospitalSpawn,
};
use std::collections::HashMap;
use traffic_support::*;
use vehicle_support::*;

/// The test's frame test: a top corner of the chassis or its centre line inside the cone, within
/// `far` m (flat) of the player.
fn framed(view: &ViewCone, p: Vec3, r: Quat, half: Vec3, player: Vec3, far: f32) -> bool {
    if (p - player).with_y(0.0).length() > far {
        return false;
    }
    let mut points: Vec<Vec3> = [(-1.0, -1.0), (1.0, -1.0), (-1.0, 1.0), (1.0, 1.0)]
        .iter()
        .map(|&(x, z)| p + r * Vec3::new(x * half.x, half.y, z * half.z))
        .collect();
    points.push(Vec3::new(p.x, 0.1, p.z));
    points.push(p + Vec3::Y * half.y);
    points.iter().any(|&q| view.contains(q, 0.0))
}

/// Seed-1 city, nobody else on the streets, the player on the hospital sidewalk.
fn city() -> App {
    let mut app = city_app(1);
    set_population(&mut app, |p| {
        p.max_civilians = 0;
        p.max_gang_members = 0;
    });
    let spawn = *app.world().resource::<HospitalSpawn>();
    let float = float_height_of(&app);
    place_player(&mut app, spawn.point + Vec3::Y * float);
    run_ticks(&mut app, 1);
    app
}

fn flat_dir(deg: f32) -> Vec3 {
    let a = deg.to_radians();
    Vec3::new(a.sin(), 0.0, -a.cos())
}

struct Track {
    last: (Vec3, Quat),
    /// Consecutive ticks out of frame by the test's frame test.
    out: u32,
}

#[test]
fn despawn_needs_two_seconds_off_frame_and_spawns_keep_their_bands() {
    let mut app = city();
    let cfg = app.world().resource::<TrafficConfig>().clone();
    let half = app.world().resource::<VehicleConfig>().half_extents();
    let feet = position(&mut app) - Vec3::Y * float_height_of(&app);
    let player = position(&mut app);
    let b = &cfg.bubble;
    let mut tracks: HashMap<Entity, Track> = HashMap::new();
    let (mut despawns, mut spawns_in, mut spawns_out) = (0, 0, 0);
    for tick in 0..(40 * 64) {
        let view = chase_view(feet, flat_dir(90.0 * (tick / (3 * 64)) as f32));
        set_view(&mut app, Some(view));
        let steady = *app.world().resource::<TrafficPhase>() == TrafficPhase::Steady;
        run_ticks(&mut app, 1);
        let mut seen = Vec::new();
        let cars: Vec<(Entity, Vec3, Quat)> = app
            .world_mut()
            .query::<(Entity, &Position, &Rotation, &TrafficCar)>()
            .iter(app.world())
            .map(|(e, p, r, _)| (e, p.0, r.0))
            .collect();
        for (car, p, r) in cars {
            seen.push(car);
            let inside = framed(&view, p, r, half, player, b.in_view.despawn);
            match tracks.get_mut(&car) {
                Some(track) => {
                    track.last = (p, r);
                    track.out = if inside { 0 } else { track.out + 1 };
                }
                None => {
                    let d = (p - player).with_y(0.0).length();
                    if steady {
                        let (lo, hi, kind) = if framed(&view, p, r, half, player, f32::INFINITY) {
                            spawns_in += 1;
                            (b.in_view.spawn, b.in_view.despawn, "in frame")
                        } else {
                            spawns_out += 1;
                            (b.off_view.spawn, b.off_view.despawn, "out of frame")
                        };
                        assert!(
                            (lo..hi).contains(&d),
                            "tick {tick}: {kind} spawn at {d:.1} m, band [{lo}, {hi})"
                        );
                    }
                    tracks.insert(
                        car,
                        Track {
                            last: (p, r),
                            out: if inside { 0 } else { 1 },
                        },
                    );
                }
            }
        }
        let gone: Vec<Entity> = tracks
            .keys()
            .filter(|e| !seen.contains(e))
            .copied()
            .collect();
        for car in gone {
            let track = tracks.remove(&car).unwrap();
            despawns += 1;
            // 2 s = 128 ticks out of frame; the despawn tick itself is not observed here.
            assert!(
                track.out >= 127,
                "tick {tick}: despawned after {} ticks out of frame",
                track.out
            );
            let d = (track.last.0 - player).with_y(0.0).length();
            assert!(d > b.off_view.despawn, "tick {tick}: despawned at {d:.1} m");
        }
    }
    eprintln!("despawns {despawns}, steady spawns in frame {spawns_in}, out of frame {spawns_out}");
    assert!(
        despawns >= 5 && spawns_in >= 5 && spawns_out >= 5,
        "GATE BROKEN: too little churn: {despawns} despawns, {spawns_in} / {spawns_out} spawns"
    );
}

/// The lane spawn point closest to `distance` m (flat) from the player.
fn lane_point_at(app: &App, player: Vec3, distance: f32) -> (Segment, f32) {
    let graph = graph(app);
    let (lane, s) = graph
        .spawn_points()
        .iter()
        .copied()
        .min_by(|a, b| {
            let da = (graph.pose(Segment::Lane(a.0), a.1).0 - player)
                .with_y(0.0)
                .length();
            let db = (graph.pose(Segment::Lane(b.0), b.1).0 - player)
                .with_y(0.0)
                .length();
            (da - distance).abs().total_cmp(&(db - distance).abs())
        })
        .expect("GATE BROKEN: no spawn points");
    (Segment::Lane(lane), s)
}

fn look_at(app: &mut App, feet: Vec3, at: Vec3) {
    let dir = (at - feet).with_y(0.0).normalize();
    set_view(app, Some(chase_view(feet, dir)));
}

#[test]
fn in_frame_car_is_kept() {
    let mut app = city();
    set_traffic(&mut app, |t| t.bubble.max_cars = 1);
    let feet = position(&mut app) - Vec3::Y * float_height_of(&app);
    let player = position(&mut app);
    let (seg, s) = lane_point_at(&app, player, 60.0);
    let car = spawn_traffic_car(&mut app, seg, s, 0.0);
    for tick in 0..640 {
        let at = position_of(&app, car);
        look_at(&mut app, feet, at);
        run_ticks(&mut app, 1);
        assert!(
            app.world().get_entity(car).is_ok(),
            "tick {tick}: the car in frame at {:.1} m was despawned",
            (at - player).with_y(0.0).length()
        );
        if (position_of(&app, car) - player).with_y(0.0).length() > 85.0 {
            break;
        }
    }
}

#[test]
fn far_in_cone_car_goes() {
    let mut app = city();
    set_traffic(&mut app, |t| t.bubble.max_cars = 1);
    let feet = position(&mut app) - Vec3::Y * float_height_of(&app);
    let player = position(&mut app);
    let (seg, s) = lane_point_at(&app, player, 100.0);
    let car = spawn_traffic_car(&mut app, seg, s, 0.0);
    let mut ticks = 0;
    loop {
        if app.world().get_entity(car).is_err() {
            break;
        }
        let at = position_of(&app, car);
        assert!(
            (at - player).with_y(0.0).length() > 90.0,
            "GATE BROKEN: the car came within 90 m"
        );
        look_at(&mut app, feet, at);
        run_ticks(&mut app, 1);
        ticks += 1;
        assert!(ticks <= 640, "the far car in the cone was never despawned");
    }
    assert!(ticks >= 128, "despawned after {ticks} ticks");
}

#[test]
fn abandoned_cars_are_despawned() {
    let at = |x: f32| Vec3::new(x, 0.0, 30.0);
    let mut app = traffic_floor(
        vec![
            (at(-30.0), at(10.0), 12.0, 0),
            (at(20.0), at(25.0), 12.0, 1),
        ],
        &[(0, 1, 0), (1, 0, 1)],
        &[],
    );
    set_traffic(&mut app, |t| t.bubble.max_cars = 0);
    let feet = Vec3::new(-20.0, 0.0, 24.0);
    set_view(&mut app, Some(chase_view(feet, Vec3::X)));
    let car = spawn_traffic_car(&mut app, Segment::Lane(0), 10.0, 0.0);
    run_ticks(&mut app, 640);
    drive_in(&mut app, car);
    set_drive(&mut app, |d| d.throttle = 0.6);
    run_ticks(&mut app, 64);
    set_drive(&mut app, |d| {
        d.throttle = -1.0;
    });
    for _ in 0..320 {
        run_ticks(&mut app, 1);
        if velocity_of(&app, car).length() < 0.5 {
            break;
        }
    }
    set_drive(&mut app, |d| d.throttle = 0.0);
    request_vehicle(&mut app);
    run_ticks(&mut app, 2);
    assert_eq!(driving(&mut app), None, "GATE BROKEN: could not get out");
    assert_eq!(traffic_car(&app, car).mode, TrafficMode::Abandoned);
    // The player walks off 40 m and looks away from the car.
    let float = float_height_of(&app);
    let away = Vec3::new(position_of(&app, car).x - 40.0, float, 24.0);
    place_player(&mut app, away);
    set_view(
        &mut app,
        Some(chase_view(away - Vec3::Y * float, Vec3::NEG_X)),
    );
    let mut ticks = 0;
    while app.world().get_entity(car).is_ok() {
        assert!(ticks <= 640, "the abandoned car was never despawned");
        run_ticks(&mut app, 1);
        ticks += 1;
    }
    assert!(ticks >= 128, "despawned after {ticks} ticks");
}
