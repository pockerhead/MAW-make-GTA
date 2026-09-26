//! What a car does to people and what people do to a car (GDD §5.2, §6.2, §6.4, T14), production
//! composition: run-over damage, sidewalk panic, car crimes, bullets against the body.

mod common;
mod police_support;
mod vehicle_support;
mod wanted_support;

use avian3d::prelude::*;
use bevy::{ecs::message::MessageCursor, prelude::*};
use common::*;
use gta_sim::{
    character::{Health, LocomotionConfig},
    civilian::CivilianState,
    combat::{
        AimConfig, BulletHitVehicle, HitReaction, Loadout, Weapon, WeaponsConfig, acquire, muzzle,
    },
    navigation::GraphWalker,
    police::{CopState, UnitKind},
    vehicle::VehicleImpact,
    world::CityBlock,
};
use police_support::*;
use vehicle_support::*;
use wanted_support::*;

/// `BulletHitVehicle` messages written while ticking, each read once.
struct CarHits(MessageCursor<BulletHitVehicle>, Vec<BulletHitVehicle>);

impl CarHits {
    fn new(app: &App) -> Self {
        Self(
            app.world()
                .resource::<Messages<BulletHitVehicle>>()
                .get_cursor_current(),
            Vec::new(),
        )
    }

    fn read(&mut self, app: &App) {
        let messages = app.world().resource::<Messages<BulletHitVehicle>>();
        self.1.extend(self.0.read(messages).copied());
    }
}

// ---------------------------------------------------------------- G4 pedestrian formula

struct RunOver {
    /// Damage of the first hit (the formula is per impact; a car that keeps rolling hits the
    /// braked, knocked-down body again).
    first: u32,
    loss: f32,
    knocked_down: bool,
    shooter_is_car: bool,
    car_loss: f32,
    /// `VehicleImpact` messages of the car (sound and shake of the hit).
    impacts: usize,
}

/// A driverless car kicked to `speed` coasts 1.0 m into an idle civilian.
fn run_over(speed: f32) -> RunOver {
    let mut app = headless_app();
    settle(&mut app);
    test_graph(
        &mut app,
        vec![Vec3::new(-25.0, 0.0, 0.0), Vec3::new(-25.0, 0.0, -20.0)],
        &[(0, 1)],
    );
    let victim = spawn_civilian(&mut app, GraphWalker { from: 0, to: 1 }, 0.0, calm());
    set_civilian_state(&mut app, victim, CivilianState::Idle { left: 100.0 });
    // Front face 1.0 m from the capsule (radius 0.3): 0.3 + 1.0 + 2.04.
    let car = spawn_car(&mut app, Vec2::new(-25.0, 3.34), 0.0);
    run_ticks(&mut app, 32);
    let at = position_of(&app, victim);
    assert!(
        flat(at - Vec3::new(-25.0, 0.0, 0.0)).length() < 0.05,
        "GATE BROKEN: victim wandered to {at}"
    );
    let before = health_of(&app, victim).current;
    let car_before = car_health(&app, car);
    let mut shots = Shots::new(&app);
    let mut impacts: MessageCursor<VehicleImpact> = app
        .world()
        .resource::<Messages<VehicleImpact>>()
        .get_cursor_current();
    kick(&mut app, car, speed);
    let mut knocked_down = false;
    let mut car_impacts = 0;
    for _ in 0..32 {
        shots.run(&mut app, 1);
        knocked_down |= app
            .world()
            .get::<HitReaction>(victim)
            .is_some_and(|r| r.is_knocked_down());
        car_impacts += impacts
            .read(app.world().resource::<Messages<VehicleImpact>>())
            .filter(|i| i.vehicle == car)
            .count();
    }
    RunOver {
        first: shots.dealt_log.first().map_or(0, |hit| hit.damage),
        loss: before - health_of(&app, victim).current,
        knocked_down,
        shooter_is_car: shots
            .dealt_log
            .iter()
            .all(|hit| hit.target == victim && hit.shooter == car),
        car_loss: car_before - car_health(&app, car),
        impacts: car_impacts,
    }
}

/// First hit (closing − 3)·12, closing = kick minus ≤ 0.125 m/s of coasting over 1.0 m.
#[test]
fn run_over_at_10_mps_costs_the_formula() {
    let hit = run_over(10.0);
    assert!((82..=84).contains(&hit.first), "first hit {}", hit.first);
    assert!(hit.loss >= hit.first as f32, "loss {}", hit.loss);
    assert!(hit.knocked_down, "not knocked down at 10 m/s");
    assert!(hit.shooter_is_car, "a driverless car is its own attacker");
    assert_eq!(hit.car_loss, 0.0, "a person dented the car");
    assert!(
        hit.impacts >= 1,
        "a run-over made no impact (no sound, no shake)"
    );
}

#[test]
fn run_over_at_6_mps_costs_the_formula() {
    let hit = run_over(6.0);
    assert!((34..=36).contains(&hit.first), "first hit {}", hit.first);
    assert!(hit.loss >= hit.first as f32, "loss {}", hit.loss);
    assert!(hit.knocked_down, "not knocked down at 6 m/s");
}

/// NPC victims keep the full curve (the player's `player_share` does not apply): one hit at 15 m/s
/// kills a civilian at full health (TASK-035).
#[test]
fn run_over_at_15_mps_kills_a_civilian_in_one_hit() {
    let hit = run_over(15.0);
    assert!(hit.first >= 100, "first hit {} at 15 m/s", hit.first);
}

#[test]
fn push_at_2_5_mps_does_no_harm() {
    let hit = run_over(2.5);
    assert_eq!((hit.first, hit.loss), (0, 0.0));
    assert!(!hit.knocked_down, "knocked down at 2.5 m/s");
    assert_eq!(hit.impacts, 0, "a harmless push made an impact");
}

// ---------------------------------------------------------------- G11 sidewalk panic, car crimes

/// Sidewalk slab x 20..40, z −30..−10, top 0.15 (a test `CityBlock`); an idle civilian on it at
/// (22, −20).
fn sidewalk_app() -> (App, Entity) {
    let mut app = headless_app();
    settle(&mut app);
    app.world_mut().spawn((
        CityBlock,
        RigidBody::Static,
        Collider::cuboid(20.0, 0.15, 20.0),
        Transform::from_xyz(30.0, 0.075, -20.0),
    ));
    test_graph(
        &mut app,
        vec![Vec3::new(22.0, 0.15, -20.0), Vec3::new(22.0, 0.15, -28.0)],
        &[(0, 1)],
    );
    let civilian = spawn_civilian(&mut app, GraphWalker { from: 0, to: 1 }, 0.0, calm());
    hold_idle(&mut app, civilian);
    run_ticks(&mut app, 8);
    (app, civilian)
}

/// A car 8 m from the civilian, rolling at `speed` along −Z; the civilian's state over the next
/// `slots + 2` ticks.
fn car_near_civilian(at: Vec2, speed: f32) -> Vec<CivilianState> {
    let (mut app, civilian) = sidewalk_app();
    let car = spawn_car(&mut app, at, 0.0);
    let distance = position_of(&app, car).distance(position_of(&app, civilian));
    assert!(
        (distance - 8.0).abs() < 0.3,
        "GATE BROKEN: car {distance:.2} m from the civilian"
    );
    drive_in(&mut app, car);
    kick(&mut app, car, speed);
    let slots = app
        .world()
        .resource::<gta_sim::perception::PerceptionConfig>()
        .slots;
    let mut states = Vec::new();
    for _ in 0..u32::from(slots) + 2 {
        set_drive(&mut app, |d| d.throttle = 0.0);
        run_ticks(&mut app, 1);
        states.push(civilian_state(&app, civilian));
    }
    states
}

fn panicked(states: &[CivilianState]) -> bool {
    states
        .iter()
        .any(|s| matches!(s, CivilianState::Flee { .. } | CivilianState::Cower { .. }))
}

#[test]
fn fast_car_on_the_sidewalk_scares_people() {
    let states = car_near_civilian(Vec2::new(22.0, -12.0), 6.0);
    assert!(panicked(&states), "no reaction: {states:?}");
}

#[test]
fn car_on_the_road_scares_nobody() {
    let states = car_near_civilian(Vec2::new(14.0, -20.0), 6.0);
    assert!(
        !panicked(&states),
        "reacted to a car on the road: {states:?}"
    );
}

#[test]
fn creeping_car_on_the_sidewalk_scares_nobody() {
    let states = car_near_civilian(Vec2::new(22.0, -12.0), 1.0);
    assert!(!panicked(&states), "reacted to a car at 1 m/s: {states:?}");
}

/// A bare cop sight fixture 30 m behind the car, clear line.
fn cop_behind(app: &mut App) {
    let chest = Vec3::new(
        -25.0,
        app.world().resource::<LocomotionConfig>().float_height,
        30.0,
    );
    spawn_cop(app, chest, 0.0);
}

/// Runs a civilian over at 6 m/s (wound: 36 hp); the heat 32 ticks later.
fn run_over_heat(with_cop: bool) -> u32 {
    use gta_sim::wanted::Crime;
    let mut app = headless_app();
    settle(&mut app);
    test_graph(
        &mut app,
        vec![Vec3::new(-25.0, 0.0, -5.0), Vec3::new(-25.0, 0.0, -25.0)],
        &[(0, 1)],
    );
    let victim = spawn_civilian(&mut app, GraphWalker { from: 0, to: 1 }, 0.0, calm());
    hold_idle(&mut app, victim);
    let car = spawn_car(&mut app, Vec2::new(-25.0, 0.0), 0.0);
    drive_in(&mut app, car);
    // The cop arrives after the theft: only the run-over is witnessed.
    if with_cop {
        cop_behind(&mut app);
    }
    run_ticks(&mut app, 8);
    let before = health_of(&app, victim).current;
    kick(&mut app, car, 6.0);
    run_ticks(&mut app, 32);
    let loss = before - health_of(&app, victim).current;
    assert!(
        loss > 0.0 && health_of(&app, victim).current > 0.0,
        "GATE BROKEN: loss {loss}"
    );
    let crimes: Vec<Crime> = incidents(&app).iter().map(|i| i.crime).collect();
    // `wound_civilian` has the same heat: the crime kind tells a run-over from a shot.
    assert!(
        crimes.contains(&Crime::RunOver) && !crimes.contains(&Crime::Wound),
        "crimes {crimes:?}"
    );
    wanted(&app).heat
}

#[test]
fn witnessed_run_over_is_a_crime() {
    let heat = wanted_cfg_heat_run_over();
    assert_eq!(run_over_heat(true), heat);
}

#[test]
fn unwitnessed_run_over_adds_no_heat() {
    assert_eq!(run_over_heat(false), 0);
}

fn wanted_cfg_heat_run_over() -> u32 {
    let app = headless_app();
    wanted_cfg(&app).heat.run_over
}

#[test]
fn stealing_a_car_in_view_of_a_cop_is_a_crime_once() {
    let mut app = headless_app();
    settle(&mut app);
    let theft = wanted_cfg(&app).heat.car_theft;
    let car = spawn_car(&mut app, Vec2::new(-25.0, 0.0), 0.0);
    cop_behind(&mut app);
    run_ticks(&mut app, 1);
    drive_in(&mut app, car);
    run_ticks(&mut app, 2);
    assert_eq!(wanted(&app).heat, theft, "first entry");
    request_vehicle(&mut app);
    run_ticks(&mut app, 2);
    assert_eq!(driving(&mut app), None, "GATE BROKEN: did not get out");
    request_vehicle(&mut app);
    run_ticks(&mut app, 2);
    assert_eq!(
        driving(&mut app),
        Some(car),
        "GATE BROKEN: did not get back in"
    );
    assert_eq!(wanted(&app).heat, theft, "re-entering the same car");
    // Once the theft incident is forgotten, only the car's `taken` flag remembers it.
    request_vehicle(&mut app);
    run_ticks(&mut app, 2);
    assert_eq!(
        driving(&mut app),
        None,
        "GATE BROKEN: did not get out again"
    );
    let memory = wanted_cfg(&app).incident_memory_seconds;
    run_ticks(&mut app, ((memory + 1.0) * 64.0) as u32);
    assert!(
        incidents(&app).is_empty(),
        "GATE BROKEN: incidents not forgotten"
    );
    request_vehicle(&mut app);
    run_ticks(&mut app, 2);
    assert_eq!(
        driving(&mut app),
        Some(car),
        "GATE BROKEN: did not get back in"
    );
    assert_eq!(
        wanted(&app).heat,
        theft,
        "re-entering own car after a minute"
    );
}

// ---------------------------------------------------------------- G12 bullets against the car

fn pistol_damage(app: &App) -> f32 {
    app.world().resource::<WeaponsConfig>().pistol.damage
}

/// Car at (−25, 0) yaw 90: long axis X, its +Z side at z 1.2 faces the shooter.
fn broadside_app() -> (App, Entity) {
    let mut app = headless_app();
    settle(&mut app);
    let car = spawn_car(&mut app, Vec2::new(-25.0, 0.0), 90.0);
    (app, car)
}

/// The player on foot at (−25, 0, 8) fires one pistol shot at the car centre.
fn shoot_car_once(app: &mut App, car: Entity) -> (Shots, CarHits) {
    let float = float_height_of(app);
    let at = Vec3::new(-25.0, float, 8.0);
    place_player(app, at);
    arm(app, Weapon::Pistol);
    run_ticks(app, 8);
    let from = position(app);
    let target = position_of(app, car);
    set_aim(app, from, target);
    let mut shots = Shots::new(app);
    let mut hits = CarHits::new(app);
    set_action(app, |a| a.fire_requested = true);
    for _ in 0..4 {
        shots.run(app, 1);
        hits.read(app);
    }
    (shots, hits)
}

#[test]
fn bullets_stop_on_the_car_and_dent_it() {
    let (mut app, car) = broadside_app();
    let max = car_health(&app, car);
    let damage = pistol_damage(&app);
    let scale = damage_cfg(&app).vehicle.bullet_scale;
    let (shots, hits) = shoot_car_once(&mut app, car);
    assert_eq!(shots.shots.len(), 1, "GATE BROKEN: shots {:?}", shots.shots);
    assert_eq!(hits.1.len(), 1, "car hits {:?}", hits.1);
    assert_eq!(hits.1[0].vehicle, car);
    assert_eq!(hits.1[0].damage, damage);
    assert_eq!(car_health(&app, car), max - damage * scale);
    let trace = shots.trace_log.first().expect("no tracer");
    assert!(
        (trace.to.z - 1.2).abs() < 0.05,
        "tracer stopped at {}",
        trace.to
    );
    assert!(
        shots.dealt_log.is_empty(),
        "damage dealt: {:?}",
        shots.dealt_log
    );
}

#[test]
fn shot_to_zero_the_car_stalls() {
    let (mut app, car) = broadside_app();
    set_car_health(&mut app, car, 20.0);
    shoot_car_once(&mut app, car);
    assert_eq!(car_health(&app, car), 0.0, "20 - 25 must clamp to 0");
    drive_in(&mut app, car);
    let from = position_of(&app, car);
    set_drive(&mut app, |d| d.throttle = 1.0);
    run_ticks(&mut app, 64);
    let moved = flat(position_of(&app, car) - from).length();
    assert!(moved < 0.1, "a wrecked car drove {moved:.3} m");
    request_vehicle(&mut app);
    run_ticks(&mut app, 1);
    assert_eq!(driving(&mut app), None, "cannot leave a wrecked car");
}

/// A pistol dummy on a 3 m platform 1.25 m from the car shoots down at the driver's head at ~55°:
/// with the head sensor enabled every pellet would enter it above the roof (needs > 29.06°).
#[test]
fn driver_is_not_hit_over_the_roof() {
    let (mut app, car) = broadside_app();
    drive_in(&mut app, car);
    run_ticks(&mut app, 32);
    let loco = app.world().resource::<LocomotionConfig>().clone();
    let me = player(&mut app);
    let head = position(&mut app) + Vec3::Y * (loco.head_height - loco.float_height);
    let roof = position_of(&app, car).y + vehicle_cfg(&app).half_extents().y;
    assert!(
        head.y + loco.head_radius > roof,
        "GATE BROKEN: head does not clear the roof ({} <= {roof}), gate cannot fail",
        head.y + loco.head_radius
    );

    spawn_wall(
        &mut app,
        Vec3::new(-24.9, 1.5, 2.95),
        Vec3::new(4.0, 3.0, 1.0),
    );
    let shooter = spawn_dummy(&mut app, Vec3::new(-24.9, 3.0, 2.65));
    let weapons = app.world().resource::<WeaponsConfig>().clone();
    let mut loadout = Loadout::default();
    acquire(
        &mut loadout.guns[Weapon::Pistol.index()],
        &weapons.pistol,
        true,
    );
    loadout.held = Some(Weapon::Pistol);
    app.world_mut().entity_mut(shooter).insert(loadout);
    let stand = position_of(&app, shooter);
    run_ticks(&mut app, 64);
    let dummy = position_of(&app, shooter);
    assert!(
        dummy.distance(stand) < 0.05,
        "GATE BROKEN: shooter moved from {stand} to {dummy}"
    );

    let offset = app.world().resource::<AimConfig>().muzzle_offset();
    let mut from = dummy;
    let mut dir = Vec3::NEG_Y;
    for _ in 0..4 {
        dir = (head - from).normalize();
        from = muzzle(dummy, dir, offset);
    }
    let elevation = (-dir.y).asin().to_degrees();
    assert!(
        elevation >= 45.0 && from.z <= 2.40,
        "GATE BROKEN: shot from {from} at {elevation:.1} deg"
    );

    let player_health = app.world().get::<Health>(me).copied().unwrap();
    let car_start = car_health(&app, car);
    let mut shots = Shots::new(&app);
    let mut hits = CarHits::new(&app);
    for _ in 0..4 {
        {
            let mut aim = app
                .world_mut()
                .get_mut::<gta_sim::character::AimIntent>(shooter)
                .unwrap();
            aim.origin = from;
            aim.direction = dir;
        }
        app.world_mut()
            .get_mut::<gta_sim::character::ActionIntent>(shooter)
            .unwrap()
            .fire_requested = true;
        for _ in 0..40 {
            shots.run(&mut app, 1);
            hits.read(&app);
        }
    }
    // T15: the roof over the seat is cabin, so every pellet wounds the driver by the cabin share;
    // the head sensor stays off while driving: never a headshot.
    let at_player: Vec<_> = shots.dealt_log.iter().filter(|h| h.target == me).collect();
    assert!(
        at_player.iter().all(|h| !h.headshot),
        "a headshot on the driver: {at_player:?}"
    );
    assert_eq!(
        at_player.len(),
        4,
        "every roof pellet over the seat is a cabin hit"
    );
    let share = damage_cfg(&app).vehicle.cabin_driver_share;
    let wound = (weapons.pistol.damage * share).round() as u32;
    assert!(
        at_player.iter().all(|h| h.damage == wound),
        "cabin wounds {at_player:?}, expected {wound} each"
    );
    let fired = shots.shots.iter().filter(|s| s.shooter == shooter).count();
    assert_eq!(fired, 4, "GATE BROKEN: the dummy fired {fired} shots");
    let taken: u32 = at_player.iter().map(|h| h.damage).sum();
    assert_eq!(
        app.world().get::<Health>(me).copied().unwrap().current,
        player_health.current - taken as f32
    );
    let on_car = hits.1.iter().filter(|h| h.vehicle == car).count();
    assert_eq!(on_car, 4, "car hits: {:?}", hits.1);
    let scale = damage_cfg(&app).vehicle.bullet_scale;
    assert_eq!(
        car_health(&app, car),
        car_start - 4.0 * weapons.pistol.damage * scale
    );
}

// ---------------------------------------------------------------- G14 a car is cover

/// A cop in `Attack` 20 m from the player (2 stars) for 128 ticks, with or without a parked car
/// broadside just in front of the player (x −2.04..2.04, z −3.0..−0.6, roof 2.08 over the 1.6 m
/// eye line).
/// Returns (ticks the cop saw the player, the cop's shots, bullets that hit the car).
fn cop_across(with_car: bool) -> (u32, usize, usize) {
    let mut app = graph_app(10.0, &[]);
    set_player_armor(&mut app, 1.0e6);
    raise_heat(&mut app, 180);
    if with_car {
        spawn_car(&mut app, Vec2::new(0.0, -1.8), 90.0);
    }
    let unit = spawn_unit(
        &mut app,
        UnitKind::Patrol,
        Vec3::new(0.0, 0.0, -20.0),
        std::f32::consts::PI,
    );
    set_cop_state(&mut app, unit, CopState::Attack);
    let mut shots = Shots::new(&app);
    let mut car_hits = CarHits::new(&app);
    let mut seen = 0;
    for tick in 0..128 {
        shots.run(&mut app, 1);
        car_hits.read(&app);
        seen += u32::from(cop(&app, unit).sees);
        if !with_car {
            continue;
        }
        // The flat line cop → player must still cross both long faces of the car.
        let (at, me) = (position_of(&app, unit), position(&mut app));
        let x_at = |z: f32| at.x + (me.x - at.x) * (z - at.z) / (me.z - at.z);
        assert!(
            at.z < -3.3 && me.z > -0.3 && x_at(-3.0).abs() < 1.7 && x_at(-0.6).abs() < 1.7,
            "GATE BROKEN: tick {tick}: the cop at {at} sees past the car to {me}"
        );
    }
    let fired = shots.shots.iter().filter(|s| s.shooter == unit).count();
    (seen, fired, car_hits.1.len())
}

/// Parked car between a cop and the player: no sight, no shot, no dent. Control without the car:
/// the cop sees and fires. Flip: `sight_blocked` with the World mask only.
#[test]
fn a_parked_car_blocks_sight_and_fire() {
    let (seen, fired, dents) = cop_across(false);
    assert!(
        seen > 0 && fired > 0,
        "GATE BROKEN: no car, yet the cop saw {seen} ticks and fired {fired}"
    );
    let (seen, fired, dents_behind) = cop_across(true);
    assert_eq!(dents, 0);
    assert_eq!(seen, 0, "the cop saw through the car for {seen} ticks");
    assert_eq!(fired, 0, "the cop fired {fired} shots through the car");
    assert_eq!(dents_behind, 0, "{dents_behind} bullets hit the cover car");
}

/// The driver's own car hides nothing: a cop 20 m ahead sees the driver and its bullets land on the
/// car (Q2). Flip: let the car that holds the driver's eye block the line.
#[test]
fn a_cop_sees_the_driver_and_shoots_the_car() {
    let mut app = graph_app(10.0, &[]);
    raise_heat(&mut app, 180);
    let car = spawn_car(&mut app, Vec2::new(-5.0, 0.0), 0.0);
    drive_in(&mut app, car);
    let unit = spawn_unit(
        &mut app,
        UnitKind::Patrol,
        Vec3::new(-5.0, 0.0, -20.0),
        std::f32::consts::PI,
    );
    set_cop_state(&mut app, unit, CopState::Attack);
    let mut car_hits = CarHits::new(&app);
    let mut seen = 0;
    for _ in 0..128 {
        run_ticks(&mut app, 1);
        car_hits.read(&app);
        seen += u32::from(cop(&app, unit).sees);
    }
    assert_eq!(driving(&mut app), Some(car), "GATE BROKEN: not driving");
    assert!(seen > 0, "the cop never saw the driver");
    assert!(
        car_hits
            .1
            .iter()
            .any(|h| h.vehicle == car && h.shooter == unit),
        "no cop bullet reached the car"
    );
}

/// A 2-star patrol cop shooting at the driver dents the car by the pistol's full damage: the police
/// `DamageScale` is for the player's body, not for the car (TASK-035).
#[test]
fn a_patrol_bullet_dents_the_car_by_the_full_pistol_damage() {
    let mut app = graph_app(10.0, &[]);
    raise_heat(&mut app, 180);
    let car = spawn_car(&mut app, Vec2::new(-5.0, 0.0), 0.0);
    drive_in(&mut app, car);
    let unit = spawn_unit(
        &mut app,
        UnitKind::Patrol,
        Vec3::new(-5.0, 0.0, -20.0),
        std::f32::consts::PI,
    );
    set_cop_state(&mut app, unit, CopState::Attack);
    let start = car_health(&app, car);
    let mut car_hits = CarHits::new(&app);
    for _ in 0..128 {
        run_ticks(&mut app, 1);
        car_hits.read(&app);
    }
    let from_cop: Vec<f32> = car_hits
        .1
        .iter()
        .filter(|h| h.vehicle == car)
        .map(|h| {
            assert_eq!(h.shooter, unit, "GATE BROKEN: another shooter hit the car");
            h.damage
        })
        .collect();
    assert!(
        !from_cop.is_empty(),
        "GATE BROKEN: no cop bullet reached the car"
    );
    let pistol = pistol_damage(&app);
    let scale = damage_cfg(&app).vehicle.bullet_scale;
    let lost = start - car_health(&app, car);
    println!("{} cop hits, car lost {lost}", from_cop.len());
    assert!(
        from_cop.iter().all(|&d| d == pistol),
        "cop bullets on the car: {from_cop:?}, pistol {pistol}"
    );
    assert_eq!(lost, from_cop.len() as f32 * pistol * scale);
}

// ---------------------------------------------------------------- T15 cabin wounds (O2)

/// The player drives the broadside car (facing −X, its left side towards +Z); a dummy with `gun`
/// stands 8 m off the left side. Returns (app, car, player, shooter).
fn cabin_range(gun: Weapon) -> (App, Entity, Entity, Entity) {
    let (mut app, car) = broadside_app();
    drive_in(&mut app, car);
    let me = player(&mut app);
    let shooter = spawn_dummy(&mut app, Vec3::new(-25.0, 0.0, 8.0));
    let weapons = app.world().resource::<WeaponsConfig>().clone();
    let mut loadout = Loadout::default();
    acquire(&mut loadout.guns[gun.index()], weapons.stats(gun), true);
    loadout.held = Some(gun);
    app.world_mut().entity_mut(shooter).insert(loadout);
    run_ticks(&mut app, 32);
    (app, car, me, shooter)
}

/// The dummy fires once (semi-auto) or for `ticks` (automatic) at the car's body-frame point `local`.
fn fire_at_car(
    app: &mut App,
    shooter: Entity,
    car: Entity,
    local: Vec3,
    ticks: u32,
) -> (Shots, CarHits) {
    let target = position_of(app, car) + rotation_of(app, car) * local;
    let offset = app.world().resource::<AimConfig>().muzzle_offset();
    let body = position_of(app, shooter);
    let mut from = body;
    let mut dir = Vec3::NEG_Z;
    for _ in 0..4 {
        dir = (target - from).normalize();
        from = muzzle(body, dir, offset);
    }
    {
        let mut aim = app
            .world_mut()
            .get_mut::<gta_sim::character::AimIntent>(shooter)
            .unwrap();
        aim.origin = from;
        aim.direction = dir;
        aim.aiming = true;
    }
    let mut shots = Shots::new(app);
    let mut hits = CarHits::new(app);
    for _ in 0..ticks {
        app.world_mut()
            .get_mut::<gta_sim::character::ActionIntent>(shooter)
            .unwrap()
            .fire_requested = true;
        shots.run(app, 1);
        hits.read(app);
    }
    for _ in 0..8 {
        shots.run(app, 1);
        hits.read(app);
    }
    (shots, hits)
}

#[test]
fn cabin_shots_wound_the_driver() {
    let share_wound = |app: &App| {
        let w = app.world().resource::<WeaponsConfig>().pistol.damage;
        (w * damage_cfg(app).vehicle.cabin_driver_share).round()
    };
    // (name, body-frame point, wounds the driver)
    for (name, local, wounds) in [
        ("side window", Vec3::new(-1.2, 0.5, 0.0), true),
        ("front fender", Vec3::new(-1.2, 0.3, -1.8), false),
        ("low door", Vec3::new(-1.2, -0.3, 0.0), false),
        ("roof over the seat", Vec3::new(0.0, 0.92, 0.3), true),
    ] {
        let (mut app, car, me, shooter) = cabin_range(Weapon::Pistol);
        let before = app.world().get::<Health>(me).unwrap().current;
        let (_, hits) = fire_at_car(&mut app, shooter, car, local, 1);
        assert_eq!(
            hits.1.iter().filter(|h| h.vehicle == car).count(),
            1,
            "GATE BROKEN: {name}: the pellet missed the car"
        );
        let lost = before - app.world().get::<Health>(me).unwrap().current;
        let expected = if wounds { share_wound(&app) } else { 0.0 };
        assert_eq!(lost, expected, "{name}: the driver lost {lost}");
    }
}

/// A pellet from a shooter with the 2-star police `DamageScale` through the side window: the car
/// loses the full pistol damage, the player driver's wound is scaled (TASK-035).
#[test]
fn a_police_cabin_pellet_wounds_the_player_driver_by_the_scale() {
    let (mut app, car, me, shooter) = cabin_range(Weapon::Pistol);
    let scale = app
        .world()
        .resource::<gta_sim::police::EscalationConfig>()
        .stars[1]
        .damage_scale;
    app.world_mut()
        .entity_mut(shooter)
        .insert(gta_sim::combat::DamageScale(scale));
    let before = app.world().get::<Health>(me).unwrap().current;
    let car_before = car_health(&app, car);
    let (_, hits) = fire_at_car(&mut app, shooter, car, Vec3::new(-1.2, 0.5, 0.0), 1);
    assert_eq!(
        hits.1.iter().filter(|h| h.vehicle == car).count(),
        1,
        "GATE BROKEN: the pellet missed the car"
    );
    let pistol = pistol_damage(&app);
    let dmg = damage_cfg(&app);
    assert_eq!(
        car_before - car_health(&app, car),
        pistol * dmg.vehicle.bullet_scale,
        "the car took a scaled pellet"
    );
    let lost = before - app.world().get::<Health>(me).unwrap().current;
    assert_eq!(
        lost,
        (pistol * scale * dmg.vehicle.cabin_driver_share).round(),
        "the player driver's wound"
    );
}

#[test]
fn an_smg_burst_into_the_cabin_kills_the_driver_who_lands_on_the_ground() {
    let (mut app, car, me, shooter) = cabin_range(Weapon::Smg);
    fire_at_car(&mut app, shooter, car, Vec3::new(-1.2, 0.5, 0.0), 180);
    assert!(
        app.world().get::<Health>(me).unwrap().current <= 0.0,
        "the driver survived: {:?}",
        app.world().get::<Health>(me)
    );
    run_ticks(&mut app, 2);
    assert_eq!(game_state(&app), gta_sim::flow::GameState::Wasted);
    assert!(driving(&mut app).is_none(), "still seated after Wasted");
    let feet = position(&mut app).y - float_height_of(&app);
    assert!(feet.abs() < 0.05, "ejected with feet at {feet}");
}
