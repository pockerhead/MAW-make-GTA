//! Shared fixtures of the wanted gates (`wanted.rs`, `wanted_search.rs`).
#![allow(dead_code)]

use crate::common::*;
use avian3d::prelude::*;
use bevy::{ecs::message::MessageCursor, prelude::*};
use gta_sim::{
    character::LocomotionConfig,
    civilian::{CivilianConfig, CivilianState, PoliceCall, Temperament},
    combat::{AimConfig, GunSlot, Weapon, WeaponsConfig, muzzle},
    gang::Faction,
    navigation::GraphWalker,
    perception::{Cause, PerceptionConfig},
    wanted::{Incident, WantedConfig, WantedLevel},
};

/// Side midpoints of the square: (0,-h), (h,0), (0,h), (-h,0).
pub const SIDES: [GraphWalker; 4] = [
    GraphWalker { from: 0, to: 1 },
    GraphWalker { from: 1, to: 2 },
    GraphWalker { from: 2, to: 3 },
    GraphWalker { from: 3, to: 0 },
];

/// Walker on the `k`-th extra segment of `graph_app`.
pub fn segment(k: u32) -> GraphWalker {
    GraphWalker {
        from: 4 + 2 * k,
        to: 5 + 2 * k,
    }
}

/// Test floor with the player settled at the origin, the square of half-size `h` (nodes
/// (-h,-h), (h,-h), (h,h), (-h,h)) and the `extra` segments.
pub fn graph_app(h: f32, extra: &[(Vec3, Vec3)]) -> App {
    let mut app = headless_app();
    let mut nodes = vec![
        Vec3::new(-h, 0.0, -h),
        Vec3::new(h, 0.0, -h),
        Vec3::new(h, 0.0, h),
        Vec3::new(-h, 0.0, h),
    ];
    let mut edges = vec![(0, 1), (1, 2), (2, 3), (3, 0)];
    for &(a, b) in extra {
        let first = nodes.len() as u32;
        nodes.extend([a, b]);
        edges.push((first, first + 1));
    }
    test_graph(&mut app, nodes, &edges);
    settle(&mut app);
    app
}

/// The player holds `weapon` with a full magazine.
pub fn arm(app: &mut App, weapon: Weapon) {
    let size = app
        .world()
        .resource::<WeaponsConfig>()
        .stats(weapon)
        .magazine;
    set_loadout(app, |l| {
        l.held = Some(weapon);
        l.guns[weapon.index()] = GunSlot {
            owned: true,
            magazine: size,
            reserve: 0,
            ..default()
        };
    });
}

pub fn hold_idle(app: &mut App, entity: Entity) {
    set_civilian_state(app, entity, CivilianState::Idle { left: 1.0e6 });
}

pub fn float_height(app: &App) -> f32 {
    app.world().resource::<LocomotionConfig>().float_height
}

/// Body centre of a character standing with its feet at `feet`.
pub fn chest(app: &App, feet: Vec3) -> Vec3 {
    feet + Vec3::Y * float_height(app)
}

pub fn wanted_cfg(app: &App) -> WantedConfig {
    app.world().resource::<WantedConfig>().clone()
}

pub fn wanted(app: &App) -> WantedLevel {
    *app.world().resource::<WantedLevel>()
}

pub fn set_heat(app: &mut App, heat: u32) {
    app.world_mut().resource_mut::<WantedLevel>().heat = heat;
}

pub fn incidents(app: &App) -> Vec<Incident> {
    app.world()
        .resource::<gta_sim::wanted::Crimes>()
        .incidents()
        .to_vec()
}

/// Fixed ticks in `seconds`; the gates assume a whole number.
pub fn ticks_in(app: &App, seconds: f32) -> u32 {
    let step = app
        .world()
        .resource::<Time<Fixed>>()
        .timestep()
        .as_secs_f32();
    let ticks = (seconds / step).round() as u32;
    assert_eq!(
        ticks as f32 * step,
        seconds,
        "GATE BROKEN: {seconds} s is not a whole tick count"
    );
    ticks
}

/// The shipped numbers every worked example of the wanted gates assumes.
pub fn assert_shipped(app: &App) {
    let w = wanted_cfg(app);
    let h = &w.heat;
    assert_eq!(
        (
            h.punch_civilian,
            h.shooting_near_people,
            h.wound_civilian,
            h.kill_person
        ),
        (5, 10, 30, 40),
        "GATE BROKEN: heat table"
    );
    assert_eq!(
        (
            w.stars[0].heat,
            w.stars[0].search_radius,
            w.stars[0].clear_seconds
        ),
        (40, 40.0, 10.0),
        "GATE BROKEN: first star"
    );
    assert_eq!(
        (
            w.shooting_radius,
            w.cop_witness_distance,
            w.cop_view_distance,
            w.cop_view_cone_deg
        ),
        (15.0, 50.0, 35.0, 110.0),
        "GATE BROKEN: radii"
    );
    assert_eq!(ticks_in(app, w.stars[0].clear_seconds), 640, "GATE BROKEN");
    let civilian = app.world().resource::<CivilianConfig>();
    assert_eq!(ticks_in(app, civilian.call_seconds), 256, "GATE BROKEN");
    assert_eq!(
        civilian.reaction.report_min_distance, 25.0,
        "GATE BROKEN: report_min_distance"
    );
    let p = app.world().resource::<PerceptionConfig>();
    assert_eq!(
        (
            p.hearing_radius,
            p.fight_hearing_radius,
            p.corpse_sight,
            p.slots
        ),
        (40.0, 15.0, 20.0, 4),
        "GATE BROKEN: perception"
    );
}

/// Ticks one fixed step at a time, keeping every weapon message and police call.
pub struct Probe {
    pub shots: Shots,
    calls: MessageCursor<PoliceCall>,
    pub call_log: Vec<PoliceCall>,
}

impl Probe {
    pub fn new(app: &App) -> Self {
        Self {
            shots: Shots::new(app),
            calls: app
                .world()
                .resource::<Messages<PoliceCall>>()
                .get_cursor_current(),
            call_log: Vec::new(),
        }
    }

    pub fn run(&mut self, app: &mut App, ticks: u32) {
        for _ in 0..ticks {
            self.shots.run(app, 1);
            let messages = app.world().resource::<Messages<PoliceCall>>();
            self.call_log.extend(self.calls.read(messages).copied());
        }
    }

    /// Runs until `count` calls were seen, at most `limit` ticks.
    pub fn run_until_calls(&mut self, app: &mut App, count: usize, limit: u32) {
        for _ in 0..limit {
            if self.call_log.len() >= count {
                return;
            }
            self.run(app, 1);
        }
        assert!(
            self.call_log.len() >= count,
            "{} calls after {limit} ticks, expected {count}",
            self.call_log.len()
        );
    }
}

/// Muzzle of a shot fired from the player's body along `direction`.
pub fn muzzle_of(app: &mut App, direction: Vec3) -> Vec3 {
    let offset = app.world().resource::<AimConfig>().muzzle_offset();
    muzzle(position(app), direction, offset)
}

/// Pulls the trigger aiming at `target`; returns the attack id of the shot.
pub fn fire_at(app: &mut App, probe: &mut Probe, target: Vec3) -> u32 {
    let origin = position(app);
    set_aim(app, origin, target);
    set_action(app, |a| a.fire_requested = true);
    let before = probe.shots.shots.len();
    for _ in 0..4 {
        probe.run(app, 1);
        if let Some(shot) = probe.shots.shots[before..].first() {
            return shot.attack;
        }
    }
    panic!("GATE BROKEN: the gun did not fire");
}

pub fn shoot_into_the_air(app: &mut App, probe: &mut Probe) -> u32 {
    let up = position(app) + Vec3::Y;
    fire_at(app, probe, up)
}

/// Named test mutation `current = 1`, then one player shot at the chest that must kill.
pub fn kill_with_one_shot(app: &mut App, probe: &mut Probe, target: Entity) -> u32 {
    set_health_of(app, target, |h| h.current = 1.0);
    let attack = fire_at(app, probe, position_of(app, target));
    assert!(
        probe
            .shots
            .dealt_log
            .iter()
            .any(|hit| hit.shot == attack && hit.target == target && hit.killed),
        "GATE BROKEN: the shot did not kill: {:?}",
        probe.shots.trace_log
    );
    attack
}

/// A bare cop sight fixture: no body, no collider, blocks no ray. Avian requires a `Transform` for
/// `Position` and syncs it into `Position`, so both carry the pose.
pub fn spawn_cop(app: &mut App, chest: Vec3, yaw: f32) -> Entity {
    let rotation = Quat::from_rotation_y(yaw);
    app.world_mut()
        .spawn((
            Name::new("Cop fixture"),
            Faction::Police,
            Transform::from_translation(chest).with_rotation(rotation),
            Position(chest),
            Rotation(rotation),
        ))
        .id()
}

/// Report-prone temperament of the corpse witness (reaction row 6).
pub const CORPSE_WITNESS: Temperament = Temperament {
    flee: 0.6,
    cower: 1.0,
    report: 1.4,
};

/// Report-prone temperament of the gunshot caller (reaction row 4).
pub const SHOT_CALLER: Temperament = Temperament {
    flee: 0.7,
    cower: 1.0,
    report: 1.5,
};

/// Setup W: `graph_app(10, extra)` with a pistol, the witness at (0,0,-10) and the victim at
/// (6,0,-10), both held idle and settled.
pub fn setup_w(extra: &[(Vec3, Vec3)]) -> (App, Entity, Entity) {
    let mut app = graph_app(10.0, extra);
    assert_shipped(&app);
    arm(&mut app, Weapon::Pistol);
    let witness = spawn_civilian(&mut app, SIDES[0], 0.5, CORPSE_WITNESS);
    let victim = spawn_civilian(&mut app, SIDES[0], 0.8, calm());
    hold_idle(&mut app, witness);
    hold_idle(&mut app, victim);
    run_ticks(&mut app, 16);
    (app, witness, victim)
}

/// Runs until `witness` calls about `about`, at most one perception cycle plus a tick.
pub fn await_report(app: &mut App, probe: &mut Probe, witness: Entity, about: Cause) {
    let slots = app.world().resource::<PerceptionConfig>().slots;
    for _ in 0..=slots {
        if let CivilianState::Report { about: Some(a), .. } = civilian_state(app, witness)
            && a == about
        {
            return;
        }
        probe.run(app, 1);
    }
    panic!(
        "GATE BROKEN: the witness did not start a call about {about:?}: {:?}",
        civilian_state(app, witness)
    );
}
