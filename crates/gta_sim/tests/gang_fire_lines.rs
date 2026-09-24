//! Gang fire discipline on the flat test floor (production composition): members never hurt a
//! groupmate or a bystander, by gun or by fist, and a member whose line of fire is blocked moves to a
//! clear line and keeps firing. Layouts are QA round 2's probe layouts (TASK-010).

mod common;

use bevy::prelude::*;
use common::*;
use gta_sim::{
    character::{ActionIntent, AimIntent, MoveIntent},
    combat::{DamageDealt, GunSlot, Loadout, Melee, Weapon},
    navigation::GraphWalker,
};

/// 30 s of fixed ticks.
const FIGHT_TICKS: u32 = 1920;

/// Shots every member with a gun fires in 30 s at least (derivation in `assert_keeps_firing`).
const MIN_SHOTS: usize = 6;

#[derive(Default)]
struct Layout {
    /// Provoked members: gang, feet, gun.
    members: Vec<(u8, Vec3, Weapon)>,
    /// Members left idle (bystanders of the other gang).
    idle: Vec<(u8, Vec3, Weapon)>,
    dummies: Vec<Vec3>,
    /// One calm civilian walks each sidewalk edge.
    civilian_edges: Vec<(Vec3, Vec3)>,
    /// Members (indices into `members`) stripped of their gun: they fight with fists.
    fists: Vec<usize>,
    /// The player walks a slow circle instead of standing.
    player_circles: bool,
    /// Static cuboids: centre, full size.
    walls: Vec<(Vec3, Vec3)>,
}

struct Outcome {
    shots: Vec<usize>,
    /// Hits (bullets or punches) on the player, per member.
    hits: Vec<usize>,
    /// Longest stretch without a shot, s, per member.
    longest_gap: Vec<f32>,
    /// Closest flat distance to the player over the fight, m, per member.
    closest: Vec<f32>,
    /// Near edge of the shipped `combat.keep_distance` band, m.
    keep_near: f32,
    friendly: Vec<DamageDealt>,
    bystander_hits: Vec<DamageDealt>,
}

fn run(layout: &Layout) -> Outcome {
    let mut app = if layout.civilian_edges.is_empty() {
        gang_floor_default(TurfLayout::WholeFloor)
    } else {
        let nodes: Vec<Vec3> = layout
            .civilian_edges
            .iter()
            .flat_map(|&(a, b)| [a, b])
            .collect();
        let edges: Vec<(u32, u32)> = (0..layout.civilian_edges.len() as u32)
            .map(|k| (2 * k, 2 * k + 1))
            .collect();
        gang_floor(TurfLayout::WholeFloor, nodes, &edges)
    };
    set_player_armor(&mut app, 1.0e6);
    for &(center, size) in &layout.walls {
        spawn_wall(&mut app, center, size);
    }
    let members: Vec<Entity> = layout
        .members
        .iter()
        .map(|&(gang, spot, gun)| spawn_member(&mut app, gang, spot, gun))
        .collect();
    for &k in &layout.fists {
        let gun = layout.members[k].2;
        let mut loadout = app.world_mut().get_mut::<Loadout>(members[k]).unwrap();
        loadout.guns[gun.index()] = GunSlot::default();
        loadout.held = None;
    }
    let mut bystanders: Vec<Entity> = layout
        .idle
        .iter()
        .map(|&(gang, spot, gun)| spawn_member(&mut app, gang, spot, gun))
        .collect();
    bystanders.extend(layout.dummies.iter().map(|&p| spawn_dummy(&mut app, p)));
    bystanders.extend((0..layout.civilian_edges.len() as u32).map(|k| {
        let walker = GraphWalker {
            from: 2 * k,
            to: 2 * k + 1,
        };
        spawn_civilian(&mut app, walker, 0.5, calm())
    }));
    run_ticks(&mut app, 1);
    for &m in &members {
        provoke(&mut app, m);
    }
    let mut shots = Shots::new(&app);
    let mut last_shot = vec![0u32; members.len()];
    let mut longest = vec![0u32; members.len()];
    let mut closest = vec![f32::INFINITY; members.len()];
    for tick in 1..=FIGHT_TICKS {
        if layout.player_circles {
            set_intent(&mut app, |i: &mut MoveIntent| {
                i.axis = Vec2::Y;
                i.yaw = tick as f32 * 0.01;
            });
        }
        let seen = shots.shots.len();
        shots.run(&mut app, 1);
        for shot in &shots.shots[seen..] {
            if let Some(k) = members.iter().position(|&m| m == shot.shooter) {
                last_shot[k] = tick;
            }
        }
        for (gap, &last) in longest.iter_mut().zip(&last_shot) {
            *gap = (*gap).max(tick - last);
        }
        let at = position(&mut app);
        for (near, &m) in closest.iter_mut().zip(&members) {
            *near = near.min((position_of(&app, m) - at).with_y(0.0).length());
        }
    }
    let target = player(&mut app);
    let hits = members
        .iter()
        .map(|&m| {
            let on_player = |d: &&DamageDealt| d.shooter == m && d.target == target;
            shots.dealt_log.iter().filter(on_player).count()
        })
        .collect();
    let friendly = shots
        .dealt_log
        .iter()
        .filter(|d| members.contains(&d.shooter) && members.contains(&d.target))
        .copied()
        .collect();
    let bystander_hits = shots
        .dealt_log
        .iter()
        .filter(|d| members.contains(&d.shooter) && bystanders.contains(&d.target))
        .copied()
        .collect();
    Outcome {
        shots: members
            .iter()
            .map(|&m| shots.shots.iter().filter(|s| s.shooter == m).count())
            .collect(),
        longest_gap: longest.iter().map(|&t| t as f32 / 64.0).collect(),
        hits,
        closest,
        keep_near: gang_cfg(&app).combat.keep_distance.0,
        friendly,
        bystander_hits,
    }
}

/// No friendly or bystander damage, and every member with a gun fires at least `MIN_SHOTS` in 30 s.
/// `MIN_SHOTS`: a member starved by the old sidestep fired 0-5 shots in 30 s (QA round 2); with the
/// candidate spots the fewest over these layouts is 9 (a lone SMG whose line runs on into a pair),
/// guns running dry cap pistols at 24 and shotguns at 12.
fn assert_keeps_firing(name: &str, layout: &Layout) {
    let out = run(layout);
    println!(
        "{name}: shots {:?} hits on the player {:?} longest_no_shot_s {:?}",
        out.shots, out.hits, out.longest_gap
    );
    assert!(
        out.friendly.is_empty(),
        "{name}: friendly damage {:?}",
        out.friendly
    );
    assert!(
        out.bystander_hits.is_empty(),
        "{name}: bystander damage {:?}",
        out.bystander_hits
    );
    for (k, (&own, &(_, _, gun))) in out.shots.iter().zip(&layout.members).enumerate() {
        if layout.fists.contains(&k) {
            continue;
        }
        assert!(
            own >= MIN_SHOTS,
            "{name}: member {k} ({gun:?}) fired {own} < {MIN_SHOTS} shots in 30 s"
        );
    }
}

fn polar(r: f32, deg: f32) -> Vec3 {
    let a = deg.to_radians();
    Vec3::new(r * a.sin(), 0.0, -r * a.cos())
}

const JITTERS: [Vec3; 3] = [
    Vec3::ZERO,
    Vec3::new(0.37, 0.0, -0.21),
    Vec3::new(-0.53, 0.0, 0.44),
];

#[test]
fn inline_file_keeps_firing() {
    use Weapon::*;
    // QA round 2: the 22 m member sidestepped into the test-floor stairs and fired 0 shots.
    for (j, jit) in JITTERS.iter().enumerate() {
        let layout = Layout {
            members: vec![
                (0, Vec3::new(0.0, 0.0, -5.0) + *jit, Smg),
                (0, Vec3::new(0.4, 0.0, -9.0) + *jit, Pistol),
                (1, Vec3::new(-0.4, 0.0, -14.0) + *jit, Shotgun),
                (0, Vec3::new(0.2, 0.0, -22.0) + *jit, Smg),
            ],
            ..default()
        };
        assert_keeps_firing(&format!("mixed4 inline j{j}"), &layout);
    }
}

#[test]
fn blocked_member_does_not_walk_into_a_wall() {
    // A dummy 6 m in front blocks the line; the nearest clear spot (4.5 m to -X) is behind a wall.
    let layout = Layout {
        members: vec![(0, Vec3::new(0.0, 0.0, -12.0), Weapon::Smg)],
        dummies: vec![Vec3::new(0.0, 0.0, -6.0)],
        walls: vec![(Vec3::new(-2.2, 1.5, -12.0), Vec3::new(0.3, 3.0, 4.0))],
        ..default()
    };
    assert_keeps_firing("wall beside the member", &layout);
}

#[test]
fn surrounding_members_keep_firing() {
    use Weapon::*;
    for (j, jit) in JITTERS.iter().enumerate() {
        let mixed = Layout {
            members: vec![
                (0, polar(6.0, 0.0) + *jit, Smg),
                (0, polar(10.0, 180.0) + *jit, Pistol),
                (0, polar(4.0, 90.0) + *jit, Shotgun),
                (0, polar(16.0, 200.0) + *jit, Smg),
            ],
            ..default()
        };
        assert_keeps_firing(&format!("surround4 mixed j{j}"), &mixed);
        let cross = Layout {
            members: vec![
                (0, polar(9.0, 10.0) + *jit, Smg),
                (0, polar(11.0, 100.0) + *jit, Pistol),
                (0, polar(13.0, 190.0) + *jit, Smg),
                (0, polar(7.0, 280.0) + *jit, Pistol),
            ],
            ..default()
        };
        assert_keeps_firing(&format!("surround4 cross j{j}"), &cross);
        let pair = Layout {
            members: vec![
                (0, polar(10.0, 0.0) + *jit, Smg),
                (0, polar(10.0, 180.0) + *jit, Smg),
            ],
            ..default()
        };
        assert_keeps_firing(&format!("crossfire pair j{j}"), &pair);
    }
}

#[test]
fn side_by_side_pairs_do_not_lock_each_other() {
    use Weapon::*;
    // A bystander far behind the player blocks both lines; the two used to step into each other.
    let layouts = [
        (
            "west pair 1 m apart + dummy east",
            vec![
                (0, Vec3::new(-12.0, 0.0, 0.5), Smg),
                (0, Vec3::new(-12.0, 0.0, -0.5), Pistol),
            ],
            vec![Vec3::new(25.0, 0.0, 1.0)],
        ),
        (
            "west pair 0.7 m apart + dummy east",
            vec![
                (0, Vec3::new(-12.0, 0.0, 0.35), Smg),
                (0, Vec3::new(-12.0, 0.0, -0.35), Pistol),
            ],
            vec![Vec3::new(25.0, 0.0, 0.0)],
        ),
        (
            "pair + dummy behind the player",
            vec![
                (0, Vec3::new(0.35, 0.0, -12.0), Smg),
                (0, Vec3::new(-0.35, 0.0, -12.0), Pistol),
            ],
            vec![Vec3::new(0.0, 0.0, 25.0)],
        ),
        (
            "pair 0.6 m + dummy behind the player",
            vec![
                (0, Vec3::new(0.5, 0.0, -13.0), Pistol),
                (0, Vec3::new(-0.1, 0.0, -13.0), Smg),
            ],
            vec![Vec3::new(0.0, 0.0, 25.0)],
        ),
        (
            "west pair + groupmate east",
            vec![
                (0, Vec3::new(-12.0, 0.0, 0.35), Smg),
                (0, Vec3::new(-12.0, 0.0, -0.35), Pistol),
                (0, Vec3::new(15.0, 0.0, 0.0), Smg),
            ],
            vec![],
        ),
    ];
    for (name, members, dummies) in layouts {
        let layout = Layout {
            members,
            dummies,
            ..default()
        };
        assert_keeps_firing(name, &layout);
    }
}

#[test]
fn members_fire_past_bystanders_and_walking_civilians() {
    use Weapon::*;
    for (j, jit) in JITTERS.iter().enumerate() {
        let layout = Layout {
            members: vec![
                (0, Vec3::new(0.0, 0.0, -12.0) + *jit, Smg),
                (0, Vec3::new(2.0, 0.0, -11.0) + *jit, Pistol),
            ],
            idle: vec![(1, Vec3::new(-1.0, 0.0, -8.0), Pistol)],
            dummies: vec![
                Vec3::new(0.0, 0.0, -6.0),
                Vec3::new(0.0, 0.0, 8.0),
                Vec3::new(0.9, 0.0, 3.0),
            ],
            ..default()
        };
        assert_keeps_firing(&format!("dummies + idle rival j{j}"), &layout);
    }
    let group = || {
        vec![
            (0, Vec3::new(0.0, 0.0, -12.0), Smg),
            (0, Vec3::new(1.0, 0.0, -12.8), Pistol),
            (0, Vec3::new(-1.0, 0.0, -12.8), Smg),
        ]
    };
    let streets = [
        (
            "sidewalk behind the player",
            vec![(Vec3::new(-15.0, 0.0, 6.0), Vec3::new(15.0, 0.0, 6.0))],
        ),
        (
            "sidewalk between",
            vec![(Vec3::new(-15.0, 0.0, -6.0), Vec3::new(15.0, 0.0, -6.0))],
        ),
        (
            "street along the line",
            vec![
                (Vec3::new(6.0, 0.0, -40.0), Vec3::new(6.0, 0.0, 40.0)),
                (Vec3::new(-6.0, 0.0, -40.0), Vec3::new(-6.0, 0.0, 40.0)),
            ],
        ),
        (
            "sidewalk far behind",
            vec![(Vec3::new(6.0, 0.0, 20.0), Vec3::new(6.0, 0.0, 40.0))],
        ),
        (
            "cross street behind the player",
            vec![
                (Vec3::new(-20.0, 0.0, 20.0), Vec3::new(20.0, 0.0, 20.0)),
                (Vec3::new(-20.0, 0.0, 26.0), Vec3::new(20.0, 0.0, 26.0)),
            ],
        ),
    ];
    for (name, civilian_edges) in streets {
        let layout = Layout {
            members: group(),
            civilian_edges,
            ..default()
        };
        assert_keeps_firing(name, &layout);
    }
}

fn fist_scrum(a: Vec3, b: Vec3, player_circles: bool) -> Layout {
    use Weapon::*;
    Layout {
        members: vec![
            (0, a, Pistol),
            (0, b, Pistol),
            (0, Vec3::new(0.0, 0.0, -12.0), Smg),
        ],
        fists: vec![0, 1],
        player_circles,
        ..default()
    }
}

const FLANK: (Vec3, Vec3) = (Vec3::new(1.2, 0.0, -1.0), Vec3::new(-1.2, 0.0, -1.0));
const INLINE: (Vec3, Vec3) = (Vec3::new(0.0, 0.0, -1.6), Vec3::new(0.3, 0.0, -3.0));

#[test]
fn gunman_behind_a_fist_scrum_keeps_firing() {
    // Two out-of-ammo members punch next to a circling player; the gunman at 12 m used to circle unfired.
    // The brawlers lag the moving player, lines open, and the gunman fires from its band.
    for (name, (a, b)) in [("flank", FLANK), ("inline", INLINE)] {
        assert_keeps_firing(
            &format!("fist scrum {name}, player circles"),
            &fist_scrum(a, b, true),
        );
    }
}

/// Group hits on the player in 30 s behind a pinning scrum, at least: measured 15 (flank) and 10
/// (inline), deterministic; 8 leaves room for a punch or two of timing drift.
const MIN_GROUP_HITS: usize = 8;

/// Metres a pinned gunman may drift inside the near edge of its `keep_distance` band.
const BAND_TOLERANCE: f32 = 0.5;

#[test]
fn gunman_holds_its_band_behind_a_fist_scrum() {
    // Brawlers pressed against a standing player block every line of the gunman beyond ~5.4 m (the
    // SMG line widens 0.5 m + d tan 11 deg past a body 1.56 m off the target), inside the band's near
    // edge: the gunman holds its band instead of walking into the scrum, the group keeps hitting.
    for (name, (a, b)) in [("flank", FLANK), ("inline", INLINE)] {
        let out = run(&fist_scrum(a, b, false));
        println!(
            "fist scrum {name}, player stands: shots {:?} hits {:?} closest {:?}",
            out.shots, out.hits, out.closest
        );
        assert!(
            out.friendly.is_empty(),
            "{name}: friendly damage {:?}",
            out.friendly
        );
        let group: usize = out.hits.iter().sum();
        assert!(
            group >= MIN_GROUP_HITS,
            "{name}: the group hit the player {group} < {MIN_GROUP_HITS} times in 30 s"
        );
        assert!(
            out.closest[2] >= out.keep_near - BAND_TOLERANCE,
            "{name}: the gunman came within {:.2} m of the player (band edge {} m)",
            out.closest[2],
            out.keep_near
        );
    }

    // A bystander pressed against the player pins the gunman the same way.
    let shield = Layout {
        members: vec![(0, Vec3::new(0.0, 0.0, -12.0), Weapon::Smg)],
        dummies: vec![Vec3::new(0.0, 0.0, -0.8)],
        ..default()
    };
    let out = run(&shield);
    println!(
        "human shield: shots {:?} hits {:?} closest {:?}",
        out.shots, out.hits, out.closest
    );
    assert!(
        out.bystander_hits.is_empty(),
        "bystander damage {:?}",
        out.bystander_hits
    );
    assert!(
        out.closest[0] >= out.keep_near - BAND_TOLERANCE,
        "the gunman came within {:.2} m of the shielded player (band edge {} m)",
        out.closest[0],
        out.keep_near
    );
}

#[test]
fn of_two_members_blocking_each_other_the_lower_index_moves() {
    // Opposite sides of the player: each line runs on past the player into the other member.
    let mut app = gang_floor_default(TurfLayout::WholeFloor);
    set_player_armor(&mut app, 1.0e6);
    let a = spawn_member(&mut app, 0, Vec3::new(0.0, 0.0, -10.0), Weapon::Smg);
    let b = spawn_member(&mut app, 0, Vec3::new(0.0, 0.0, 10.0), Weapon::Smg);
    run_ticks(&mut app, 1);
    provoke(&mut app, a);
    provoke(&mut app, b);
    let (low, high) = if a.index_u32() < b.index_u32() {
        (a, b)
    } else {
        (b, a)
    };
    let drawn = |app: &App, e: Entity| app.world().get::<Loadout>(e).unwrap().held.is_some();
    let mut ticks = 0;
    while !(drawn(&app, low) && drawn(&app, high)) {
        ticks += 1;
        assert!(ticks <= 32, "GATE BROKEN: guns not drawn in 32 ticks");
        run_ticks(&mut app, 1);
    }
    let (low_from, high_from) = (position_of(&app, low), position_of(&app, high));
    let mut low_planned = false;
    for tick in 0..48 {
        run_ticks(&mut app, 1);
        low_planned |= member(&app, low).reposition.is_some();
        let high_member = member(&app, high);
        assert!(
            high_member.reposition.is_none()
                && app.world().get::<MoveIntent>(high).unwrap().axis == Vec2::ZERO,
            "tick {tick}: the higher index {high} moves instead of holding"
        );
    }
    assert!(low_planned, "the lower index {low} never picked a spot");
    let moved = |from: Vec3, e: Entity| (position_of(&app, e) - from).with_y(0.0).length();
    assert!(moved(low_from, low) > 0.5, "the lower index stayed put");
    assert!(
        moved(high_from, high) < 0.1,
        "the higher index moved {} m",
        moved(high_from, high)
    );
}

/// A holstered member at (0, 0, -10) throws one punch at whoever stands 1.2 m in front of it;
/// returns whether the sweep met a body and the damage it dealt.
fn punch(victim: impl FnOnce(&mut App, Vec3) -> Entity) -> (bool, Vec<DamageDealt>, Entity) {
    let mut app = gang_floor_default(TurfLayout::WholeFloor);
    let a = spawn_member(&mut app, 0, Vec3::new(0.0, 0.0, -10.0), Weapon::Pistol);
    let target = victim(&mut app, Vec3::new(0.0, 0.0, -11.2));
    run_ticks(&mut app, 1);
    let (from, to) = (position_of(&app, a), position_of(&app, target));
    *app.world_mut().get_mut::<AimIntent>(a).unwrap() = AimIntent {
        origin: from,
        direction: to - from,
        aiming: false,
    };
    app.world_mut()
        .get_mut::<ActionIntent>(a)
        .unwrap()
        .fire_requested = true;
    let mut shots = Shots::new(&app);
    let mut landed = false;
    for _ in 0..30 {
        shots.run(&mut app, 1);
        let melee = app.world().get::<Melee>(a).unwrap();
        landed |= melee.swing.is_some_and(|s| s.landed);
    }
    let dealt = shots
        .dealt_log
        .iter()
        .filter(|d| d.shooter == a)
        .copied()
        .collect();
    (landed, dealt, target)
}

/// Spawns a character with feet at the given spot.
type Spawn = fn(&mut App, Vec3) -> Entity;

#[test]
fn gang_punches_spare_non_hostile_characters_at_impact() {
    let victims: [(&str, Spawn); 3] = [
        ("groupmate", |app, feet| {
            spawn_member(app, 0, feet, Weapon::Pistol)
        }),
        ("the other gang at peace", |app, feet| {
            spawn_member(app, 1, feet, Weapon::Pistol)
        }),
        ("a bystander without a faction", spawn_dummy),
    ];
    for (name, victim) in victims {
        let (landed, dealt, _) = punch(victim);
        assert!(landed, "GATE BROKEN: the punch at {name} met no body");
        assert!(dealt.is_empty(), "{name} took damage: {dealt:?}");
    }
    // Positive control: the player is hostile and takes the punch.
    let (landed, dealt, target) = punch(|app, feet| {
        let float = app
            .world()
            .resource::<gta_sim::character::LocomotionConfig>()
            .float_height;
        place_player(app, feet + Vec3::Y * float);
        player(app)
    });
    assert!(landed, "GATE BROKEN: the punch at the player met no body");
    assert!(
        dealt.iter().any(|d| d.target == target),
        "the player was spared: {dealt:?}"
    );
}

/// Two 14 m walls (z -20..-6, 0.3 m thick) leaving a passage of `width` m around x = 0 (TASK-010 QA
/// round 3, Bug 1): every side spot of the rear member is behind a wall.
fn corridor(width: f32) -> Vec<(Vec3, Vec3)> {
    let x = width / 2.0 + 0.15;
    [x, -x]
        .map(|x| (Vec3::new(x, 1.5, -13.0), Vec3::new(0.3, 3.0, 14.0)))
        .to_vec()
}

/// Before the queue slot the rear member fired 0 shots in every case (QA round 3).
fn corridor_case(width: f32, front: Weapon, rear: Weapon, rear_z: f32) {
    let layout = Layout {
        members: vec![
            (0, Vec3::new(0.0, 0.0, -10.0), front),
            (0, Vec3::new(0.0, 0.0, rear_z), rear),
        ],
        walls: corridor(width),
        ..default()
    };
    assert_keeps_firing(
        &format!("corridor {width} m, {front:?} front, {rear:?} rear at {rear_z}"),
        &layout,
    );
}

#[test]
fn corridor_2_4_smg_front_pistol_rear() {
    corridor_case(2.4, Weapon::Smg, Weapon::Pistol, -14.0);
}

#[test]
fn corridor_3_0_smg_front_pistol_rear() {
    corridor_case(3.0, Weapon::Smg, Weapon::Pistol, -14.0);
}

#[test]
fn corridor_2_4_pistol_front_smg_rear() {
    corridor_case(2.4, Weapon::Pistol, Weapon::Smg, -14.0);
}

#[test]
fn corridor_3_0_pistol_front_smg_rear() {
    corridor_case(3.0, Weapon::Pistol, Weapon::Smg, -14.0);
}

#[test]
fn corridor_2_4_rear_member_18m_back() {
    corridor_case(2.4, Weapon::Smg, Weapon::Pistol, -18.0);
}
