//! Police fire discipline (shared `tactics` layer) on the test floor, production composition: cops
//! never hurt each other or a bystander, and the rear cop of a file in a narrow corridor keeps
//! firing (TASK-010 QA round 3, Bug 1, for the police role).

mod common;
mod police_support;
mod wanted_support;

use bevy::prelude::*;
use common::*;
use gta_sim::{
    combat::{DamageDealt, Weapon},
    navigation::GraphWalker,
    police::{CopState, UnitKind},
};
use police_support::*;
use std::f32::consts::PI;
use wanted_support::*;

/// 30 s of fixed ticks.
const FIGHT_TICKS: u32 = 1920;

/// Shots every cop fires in 30 s at least (the gang corridor gate's floor; starved: 0).
const MIN_SHOTS: usize = 6;

#[derive(Default)]
struct Layout {
    /// Cops put into `Attack`: kind, feet.
    cops: Vec<(UnitKind, Vec3)>,
    /// Idle gang members of gang 1 (a rival outside its turf): feet.
    idle: Vec<Vec3>,
    dummies: Vec<Vec3>,
    /// One calm civilian walks each sidewalk edge.
    civilian_edges: Vec<(Vec3, Vec3)>,
    /// Static cuboids: centre, full size.
    walls: Vec<(Vec3, Vec3)>,
}

struct Outcome {
    shots: Vec<usize>,
    friendly: Vec<DamageDealt>,
    bystander_hits: Vec<DamageDealt>,
}

fn run(layout: &Layout) -> Outcome {
    let mut nodes = vec![Vec3::new(30.0, 0.0, 30.0), Vec3::new(35.0, 0.0, 30.0)];
    let mut edges = vec![(0, 1)];
    for &(a, b) in &layout.civilian_edges {
        let first = nodes.len() as u32;
        nodes.extend([a, b]);
        edges.push((first, first + 1));
    }
    let mut app = gang_floor(TurfLayout::WholeFloor, nodes, &edges);
    assert_shipped_police(&app);
    set_player_armor(&mut app, 1.0e6);
    raise_heat(&mut app, 180);
    for &(center, size) in &layout.walls {
        spawn_wall(&mut app, center, size);
    }
    let cops: Vec<Entity> = layout
        .cops
        .iter()
        .map(|&(kind, feet)| spawn_unit(&mut app, kind, feet, PI))
        .collect();
    let mut bystanders: Vec<Entity> = layout
        .idle
        .iter()
        .map(|&spot| spawn_member(&mut app, 1, spot, Weapon::Pistol))
        .collect();
    bystanders.extend(layout.dummies.iter().map(|&p| spawn_dummy(&mut app, p)));
    bystanders.extend((0..layout.civilian_edges.len() as u32).map(|k| {
        let walker = GraphWalker {
            from: 2 + 2 * k,
            to: 3 + 2 * k,
        };
        spawn_civilian(&mut app, walker, 0.5, calm())
    }));
    for &cop in &cops {
        set_cop_state(&mut app, cop, CopState::Attack);
    }
    let mut shots = Shots::new(&app);
    shots.run(&mut app, FIGHT_TICKS);
    assert_eq!(wanted(&app).stars, 2, "GATE BROKEN: the wanted level moved");
    let is_cop = |e: Entity| cops.contains(&e);
    Outcome {
        shots: cops
            .iter()
            .map(|&c| shots.shots.iter().filter(|s| s.shooter == c).count())
            .collect(),
        friendly: shots
            .dealt_log
            .iter()
            .filter(|d| is_cop(d.shooter) && is_cop(d.target))
            .copied()
            .collect(),
        bystander_hits: shots
            .dealt_log
            .iter()
            .filter(|d| is_cop(d.shooter) && bystanders.contains(&d.target))
            .copied()
            .collect(),
    }
}

/// No cop-to-cop or bystander damage; with `firing`, every cop fires at least `MIN_SHOTS` in 30 s.
fn assert_discipline(name: &str, layout: &Layout, firing: bool) {
    let out = run(layout);
    println!("{name}: shots {:?}", out.shots);
    assert!(
        out.friendly.is_empty(),
        "{name}: cop-to-cop damage {:?}",
        out.friendly
    );
    assert!(
        out.bystander_hits.is_empty(),
        "{name}: bystander damage {:?}",
        out.bystander_hits
    );
    if !firing {
        return;
    }
    for (k, &own) in out.shots.iter().enumerate() {
        assert!(
            own >= MIN_SHOTS,
            "{name}: cop {k} ({:?}) fired {own} < {MIN_SHOTS} shots in 30 s",
            layout.cops[k].0
        );
    }
}

/// Two 14 m walls (z -20..-6, 0.3 m thick) leaving a passage of `width` m around x = 0.
fn corridor(width: f32) -> Vec<(Vec3, Vec3)> {
    let x = width / 2.0 + 0.15;
    [x, -x]
        .map(|x| (Vec3::new(x, 1.5, -13.0), Vec3::new(0.3, 3.0, 14.0)))
        .to_vec()
}

fn corridor_case(width: f32, front: UnitKind, rear: UnitKind) {
    let layout = Layout {
        cops: vec![
            (front, Vec3::new(0.0, 0.0, -10.0)),
            (rear, Vec3::new(0.0, 0.0, -14.0)),
        ],
        walls: corridor(width),
        ..Layout::default()
    };
    let name = format!("corridor {width} m, {front:?} front, {rear:?} rear");
    assert_discipline(&name, &layout, true);
}

#[test]
fn corridor_2_4_patrol_pair() {
    corridor_case(2.4, UnitKind::Patrol, UnitKind::Patrol);
}

#[test]
fn corridor_3_0_patrol_pair() {
    corridor_case(3.0, UnitKind::Patrol, UnitKind::Patrol);
}

#[test]
fn corridor_2_4_patrol_front_swat_rear() {
    corridor_case(2.4, UnitKind::Patrol, UnitKind::Swat);
}

#[test]
fn corridor_3_0_patrol_front_swat_rear() {
    corridor_case(3.0, UnitKind::Patrol, UnitKind::Swat);
}

// Not starved even without the queue slot (flip: the rear still fires 24): a safety case only.
#[test]
fn corridor_2_4_swat_front_patrol_rear() {
    corridor_case(2.4, UnitKind::Swat, UnitKind::Patrol);
}

#[test]
fn corridor_3_0_swat_front_patrol_rear() {
    corridor_case(3.0, UnitKind::Swat, UnitKind::Patrol);
}

#[test]
fn cops_never_hit_bystanders() {
    let rival = Layout {
        cops: vec![
            (UnitKind::Swat, Vec3::new(0.0, 0.0, -12.0)),
            (UnitKind::Patrol, Vec3::new(2.0, 0.0, -11.0)),
        ],
        idle: vec![Vec3::new(-1.0, 0.0, -8.0)],
        dummies: vec![
            Vec3::new(0.0, 0.0, -6.0),
            Vec3::new(0.0, 0.0, 8.0),
            Vec3::new(0.9, 0.0, 3.0),
        ],
        ..Layout::default()
    };
    assert_discipline("dummies + idle rival", &rival, false);
    let between = Layout {
        cops: vec![
            (UnitKind::Swat, Vec3::new(0.0, 0.0, -12.0)),
            (UnitKind::Patrol, Vec3::new(1.0, 0.0, -12.8)),
            (UnitKind::Swat, Vec3::new(-1.0, 0.0, -12.8)),
        ],
        civilian_edges: vec![(Vec3::new(-15.0, 0.0, -6.0), Vec3::new(15.0, 0.0, -6.0))],
        ..Layout::default()
    };
    assert_discipline("sidewalk between", &between, false);
}
