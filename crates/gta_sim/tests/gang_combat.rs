//! Gang fights on the flat test floor (production composition): fire, distance band, routing
//! around a wall, retreat, melee, the dropped gun (GDD §6.3, §6.6).

mod common;

use avian3d::prelude::*;
use bevy::{ecs::system::RunSystemOnce, prelude::*};
use bevy_tnua::TnuaToggle;
use common::*;
use gta_sim::{
    character::{Dead, LocomotionConfig},
    combat::{Dropped, GunSlot, Loadout, Melee, MeleeConfig, Weapon, WeaponPickup, WeaponsConfig},
    gang::{Faction, GangState},
    navigation::{NavigationConfig, RouteLoad},
    perception::{PerceptionConfig, sight_blocked},
    population::Corpse,
};

fn loco(app: &App) -> LocomotionConfig {
    app.world().resource::<LocomotionConfig>().clone()
}

fn flat(a: Vec3, b: Vec3) -> f32 {
    (a - b).with_y(0.0).length()
}

/// Horizontal distance between the player and `entity`.
fn gap(app: &mut App, entity: Entity) -> f32 {
    let p = position(app);
    flat(p, position_of(app, entity))
}

fn route_load(app: &App) -> RouteLoad {
    *app.world().resource::<RouteLoad>()
}

fn held(app: &App, entity: Entity) -> Option<Weapon> {
    app.world().get::<Loadout>(entity).unwrap().held
}

fn speed(app: &App, entity: Entity) -> f32 {
    app.world()
        .get::<LinearVelocity>(entity)
        .unwrap()
        .0
        .with_y(0.0)
        .length()
}

/// Test floor, player at the origin with armour no fight can wear down, member `gun` at `feet`,
/// provoked; one tick has run before the provocation.
fn fight(feet: Vec3, gun: Weapon) -> (App, Entity) {
    fight_on(gang_floor_default(TurfLayout::WholeFloor), feet, gun)
}

fn fight_on(mut app: App, feet: Vec3, gun: Weapon) -> (App, Entity) {
    set_player_armor(&mut app, 1.0e6);
    let m = spawn_member(&mut app, 0, feet, gun);
    run_ticks(&mut app, 1);
    provoke(&mut app, m);
    (app, m)
}

/// Distance from `p` to the segment `a`-`b`.
fn to_segment(p: Vec3, a: Vec3, b: Vec3) -> f32 {
    let ab = b - a;
    let t = ((p - a).dot(ab) / ab.length_squared()).clamp(0.0, 1.0);
    p.distance(a + ab * t)
}

#[test]
fn attackers_open_fire_at_the_player() {
    let (mut app, m) = fight(Vec3::new(0.0, 0.0, -12.0), Weapon::Pistol);
    let cfg = gang_cfg(&app);
    let slots = u32::from(app.world().resource::<PerceptionConfig>().slots);
    let first_limit = (cfg.combat.trigger_seconds.1 * 64.0).ceil() as u32 + slots + 2;
    let mut shots = Shots::new(&app);
    let mut first = None;
    for k in 1..=256 {
        shots.run(&mut app, 1);
        let fired = shots.shots.iter().filter(|s| s.shooter == m).count();
        if fired > 0 && first.is_none() {
            first = Some(k);
            assert_eq!(
                held(&app, m),
                Some(Weapon::Pistol),
                "shot without the pistol"
            );
        }
        if fired > 0 {
            assert!(
                speed(&app, m) < 0.5,
                "tick {k}: M moves while holding the band"
            );
        }
    }
    let first = first.expect("M never fired");
    assert!(
        first <= first_limit,
        "first shot at tick {first} > {first_limit}"
    );
    let own = shots.shots.iter().filter(|s| s.shooter == m).count();
    assert!(own >= 2, "only {own} shots in 256 ticks");
    let pistol = app.world().resource::<WeaponsConfig>().pistol.spread;
    let cone =
        (cfg.combat.aim_error_deg + pistol.base_deg + pistol.max_bloom_deg + 0.3).to_radians();
    let chest = position(&mut app);
    let d = flat(chest, position_of(&app, m));
    let allowed = d * cone.tan() + 0.5;
    let traces: Vec<_> = shots.trace_log.iter().filter(|t| t.shooter == m).collect();
    assert!(!traces.is_empty());
    for t in traces {
        let miss = to_segment(chest, t.from, t.to);
        assert!(
            miss <= allowed,
            "trace {t:?} passes {miss} m from the chest > {allowed}"
        );
    }
}

#[test]
fn each_member_fires_no_faster_than_its_trigger_cadence() {
    let mut app = gang_floor_default(TurfLayout::WholeFloor);
    set_player_armor(&mut app, 1.0e6);
    let guns = [Weapon::Pistol, Weapon::Smg, Weapon::Shotgun];
    let spots = [
        Vec3::new(0.0, 0.0, -12.0),
        Vec3::new(-7.0, 0.0, -9.75),
        Vec3::new(7.0, 0.0, -9.75),
    ];
    let members: Vec<_> = guns
        .iter()
        .zip(spots)
        .map(|(&gun, spot)| spawn_member(&mut app, 0, spot, gun))
        .collect();
    run_ticks(&mut app, 1);
    for &m in &members {
        provoke(&mut app, m);
    }
    let cfg = gang_cfg(&app);
    let weapons = app.world().resource::<WeaponsConfig>().clone();
    let mut shots = Shots::new(&app);
    let mut ticks: Vec<Vec<u32>> = vec![Vec::new(); members.len()];
    for k in 1..=384 {
        shots.run(&mut app, 1);
        for (i, &m) in members.iter().enumerate() {
            let fired = shots.shots.iter().filter(|s| s.shooter == m).count();
            assert!(fired <= 1, "tick {k}: {:?} fired {fired} shots", guns[i]);
            if fired == 1 {
                ticks[i].push(k);
            }
        }
        shots.clear();
    }
    let target = player(&mut app);
    for (i, gun) in guns.iter().enumerate() {
        assert_eq!(
            gang_state(&app, members[i]),
            GangState::Attack { target },
            "GATE BROKEN: {gun:?} left the fight"
        );
        let own = &ticks[i];
        assert!(
            own.len() >= 3,
            "{gun:?}: only {} shots in 384 ticks",
            own.len()
        );
        // A pull re-rolls the trigger delay; a pull inside the weapon cooldown is dropped.
        let floor = cfg
            .combat
            .trigger_seconds
            .0
            .max(weapons.stats(*gun).fire_interval);
        for pair in own.windows(2) {
            let seconds = (pair[1] - pair[0]) as f32 / 64.0;
            assert!(
                seconds >= floor - 1e-6,
                "{gun:?}: shots at ticks {} and {} are {seconds} s apart < {floor} s",
                pair[0],
                pair[1]
            );
        }
    }
}

#[test]
fn members_hold_the_8_to_15m_band() {
    // Approach from 24 m in clear view: a straight seek, never a route search.
    let (mut app, m) = fight(Vec3::new(0.0, 0.0, -24.0), Weapon::Pistol);
    let direct = app
        .world()
        .resource::<NavigationConfig>()
        .direct_seek_distance;
    assert!(
        gap(&mut app, m) < direct,
        "GATE BROKEN: the start must be a direct seek"
    );
    for k in 1..=512 {
        run_ticks(&mut app, 1);
        assert_eq!(
            route_load(&app).searches,
            0,
            "tick {k}: route search in clear view"
        );
        if k == 128 {
            let d = gap(&mut app, m);
            assert!(d <= 20.0, "approach: {d} m after 128 ticks");
        }
    }
    let d = gap(&mut app, m);
    assert!((13.5..=15.5).contains(&d), "approach settled at {d} m");

    // Back off from 4 m (outside the melee reach).
    let (mut app, m) = fight(Vec3::new(0.0, 0.0, -4.0), Weapon::Pistol);
    run_ticks(&mut app, 128);
    let d = gap(&mut app, m);
    assert!(d >= 5.5, "back-off: {d} m after 128 ticks");
}

#[test]
fn chase_routes_around_the_wall() {
    let app = gang_floor(
        TurfLayout::WholeFloor,
        vec![
            Vec3::new(-8.0, 0.0, 20.0),
            Vec3::new(-8.0, 0.0, 8.0),
            Vec3::new(0.0, 0.0, 8.0),
        ],
        &[(0, 1), (1, 2)],
    );
    let mut app = app;
    let float = loco(&app).float_height;
    place_player(&mut app, Vec3::new(0.0, float, 4.0));
    let (mut app, m) = fight_on(app, Vec3::new(0.0, 0.0, 20.0), Weapon::Pistol);
    let l = loco(&app);
    let eyes = position_of(&app, m) - Vec3::Y * l.float_height + Vec3::Y * l.head_height;
    let chest = position(&mut app);
    let blocked = app
        .world_mut()
        .run_system_once(move |spatial: SpatialQuery| sight_blocked(&spatial, eyes, chest))
        .unwrap();
    assert!(blocked, "GATE BROKEN: the wall must hide the player from M");
    let budget = app
        .world()
        .resource::<NavigationConfig>()
        .route_requests_per_tick;
    let mut shots = Shots::new(&app);
    let (mut searched, mut saw) = (false, false);
    for k in 1..=400 {
        shots.run(&mut app, 1);
        let load = route_load(&app);
        assert!(
            load.searches <= budget,
            "tick {k}: {} searches",
            load.searches
        );
        searched |= load.searches >= 1;
        saw |= member(&app, m).sees;
    }
    assert!(saw, "M never got a line of sight around the wall");
    let own = shots.shots.iter().filter(|s| s.shooter == m).count();
    assert!(own >= 1, "M never fired after routing around the wall");
    assert!(searched, "no route search while the player was hidden");
}

#[test]
fn wounded_member_retreats() {
    let mut app = gang_floor_default(TurfLayout::WholeFloor);
    set_player_armor(&mut app, 1.0e6);
    let m = spawn_member(&mut app, 0, Vec3::new(0.0, 0.0, -10.0), Weapon::Pistol);
    run_ticks(&mut app, 1);
    set_health_of(&mut app, m, |h| h.current = 25.0);
    provoke(&mut app, m);
    run_ticks(&mut app, 1);
    let target = player(&mut app);
    assert_eq!(gang_state(&app, m), GangState::Retreat { from: target });
    run_ticks(&mut app, 383);
    let d = gap(&mut app, m);
    assert!((24.5..=26.5).contains(&d), "retreat ended at {d} m");
    assert_eq!(gang_state(&app, m), GangState::Retreat { from: target });
}

#[test]
fn close_member_punches() {
    let (mut app, m) = fight(Vec3::new(0.0, 0.0, -1.2), Weapon::Pistol);
    let cfg = gang_cfg(&app);
    let melee = app.world().resource::<MeleeConfig>().clone();
    let limit = (cfg.combat.trigger_seconds.1 * 64.0).ceil() as u32 + 4;
    let mut shots = Shots::new(&app);
    let swung = (1..=limit).find(|_| {
        shots.run(&mut app, 1);
        held(&app, m).is_none() && app.world().get::<Melee>(m).unwrap().swing.is_some()
    });
    assert!(swung.is_some(), "no fist swing within {limit} ticks");
    let target = player(&mut app);
    let fist = |damage: u32| melee.fists.hits.iter().any(|h| h.damage == damage);
    let landed = (0..128).find(|_| {
        if shots
            .dealt_log
            .iter()
            .any(|d| d.shooter == m && d.target == target && fist(d.damage))
        {
            return true;
        }
        shots.run(&mut app, 1);
        false
    });
    assert!(
        landed.is_some(),
        "no fist hit on the player within 128 ticks"
    );
}

fn dropped(app: &mut App) -> Vec<(Entity, Weapon, bool, Vec3, bool)> {
    app.world_mut()
        .query::<(Entity, &WeaponPickup, &Transform, Has<Dropped>)>()
        .iter(app.world())
        .map(|(e, p, t, d)| (e, p.weapon, p.ammo_only, t.translation, d))
        .collect()
}

#[test]
fn dead_member_drops_its_gun() {
    let mut app = gang_floor_default(TurfLayout::WholeFloor);
    let m = spawn_member(&mut app, 0, Vec3::new(0.0, 0.0, -10.0), Weapon::Pistol);
    run_ticks(&mut app, 1);
    assert!(
        dropped(&mut app).is_empty(),
        "GATE BROKEN: pickups on the test floor"
    );
    set_health_of(&mut app, m, |h| h.current = 0.0);
    run_ticks(&mut app, 1);
    assert_eq!(gang_state(&app, m), GangState::Dead);
    let world = app.world();
    assert!(world.get::<Dead>(m).is_some() && world.get::<Corpse>(m).is_some());
    assert!(matches!(
        world.get::<TnuaToggle>(m),
        Some(TnuaToggle::Disabled)
    ));
    let body_loadout = world.get::<Loadout>(m).unwrap();
    assert_eq!(body_loadout.held, None);
    assert!(!body_loadout.guns[Weapon::Pistol.index()].owned);
    let drops = dropped(&mut app);
    assert_eq!(drops.len(), 1, "{drops:?}");
    let (pickup, weapon, ammo_only, at, is_dropped) = drops[0];
    assert!(
        weapon == Weapon::Pistol && !ammo_only && is_dropped,
        "{drops:?}"
    );
    let body = position_of(&app, m);
    let feet = body.y - loco(&app).float_height;
    assert!(flat(at, body) <= 0.01, "drop at {at}, body at {body}");
    assert!((at.y - feet).abs() < 1e-4, "drop y {} vs feet {feet}", at.y);

    // The player takes it: once, and the pickup is gone.
    set_loadout(&mut app, |l| {
        l.held = None;
        l.guns[Weapon::Pistol.index()] = GunSlot::default();
    });
    let float = loco(&app).float_height;
    place_player(&mut app, at + Vec3::Y * float);
    run_ticks(&mut app, 1);
    assert!(loadout(&mut app).guns[Weapon::Pistol.index()].owned);
    assert!(
        app.world().get_entity(pickup).is_err(),
        "taken drop still exists"
    );

    // A drop left alone lies `drop_seconds`.
    let second = spawn_member(&mut app, 0, Vec3::new(-20.0, 0.0, -20.0), Weapon::Pistol);
    run_ticks(&mut app, 1);
    set_health_of(&mut app, second, |h| h.current = 0.0);
    run_ticks(&mut app, 1);
    let drops = dropped(&mut app);
    assert_eq!(drops.len(), 1, "{drops:?}");
    let lying = drops[0].0;
    let seconds = app.world().resource::<WeaponsConfig>().pickups.drop_seconds;
    let ticks = (seconds * 64.0) as u32;
    run_ticks(&mut app, ticks - 1);
    assert!(
        app.world().get_entity(lying).is_ok(),
        "gone before {ticks} ticks"
    );
    run_ticks(&mut app, 1);
    assert!(
        app.world().get_entity(lying).is_err(),
        "still there at {ticks} ticks"
    );
}

/// Members of a group `guns` in a 1 m ring around `post` (the spawner's layout).
fn ring(post: Vec3, guns: &[Weapon]) -> Vec<(Vec3, Weapon)> {
    let n = guns.len() as f32;
    guns.iter()
        .enumerate()
        .map(|(k, &gun)| {
            let t = std::f32::consts::TAU * k as f32 / n;
            (post + Vec3::new(t.cos(), 0.0, -t.sin()), gun)
        })
        .collect()
}

#[test]
fn members_never_shoot_their_own_group() {
    use Weapon::*;
    // Every layout put groupmates into each other's line of fire before the hold-fire rule (QA probe).
    let layouts: [(&str, Vec<(Vec3, Weapon)>); 6] = [
        (
            "ring3 smg 12 m",
            ring(Vec3::new(0.0, 0.0, -12.0), &[Smg, Smg, Smg]),
        ),
        (
            "ring3 mixed 12 m",
            ring(Vec3::new(0.0, 0.0, -12.0), &[Pistol, Smg, Smg]),
        ),
        (
            "ring4 12 m",
            ring(
                Vec3::new(0.0, 0.0, -12.0),
                &[Shotgun, Pistol, Pistol, Shotgun],
            ),
        ),
        (
            "ring3 smg 20 m",
            ring(Vec3::new(3.0, 0.0, -20.0), &[Smg, Smg, Smg]),
        ),
        (
            "file of 3",
            vec![
                (Vec3::new(0.0, 0.0, -10.0), Smg),
                (Vec3::new(0.3, 0.0, -12.0), Smg),
                (Vec3::new(-0.3, 0.0, -14.0), Smg),
            ],
        ),
        (
            "spread 3",
            vec![
                (Vec3::new(0.0, 0.0, -12.0), Pistol),
                (Vec3::new(-7.0, 0.0, -9.75), Smg),
                (Vec3::new(7.0, 0.0, -9.75), Shotgun),
            ],
        ),
    ];
    // Liveness: a member that only dodged its groupmates would fire rarely (lowest seen: 11, shotgun).
    let min_shots = 8;
    for (name, layout) in layouts {
        let mut app = gang_floor_default(TurfLayout::WholeFloor);
        set_player_armor(&mut app, 1.0e6);
        let members: Vec<Entity> = layout
            .iter()
            .map(|&(spot, gun)| spawn_member(&mut app, 0, spot, gun))
            .collect();
        run_ticks(&mut app, 1);
        for &m in &members {
            provoke(&mut app, m);
        }
        let mut shots = Shots::new(&app);
        shots.run(&mut app, 1920);
        let friendly: Vec<_> = shots
            .dealt_log
            .iter()
            .filter(|d| members.contains(&d.shooter) && members.contains(&d.target))
            .collect();
        assert!(friendly.is_empty(), "{name}: friendly hits {friendly:?}");
        for (&m, &(_, gun)) in members.iter().zip(&layout) {
            let own = shots.shots.iter().filter(|s| s.shooter == m).count();
            assert!(
                own >= min_shots,
                "{name}: {gun:?} member fired {own} < {min_shots} shots in 30 s"
            );
        }
        let target = player(&mut app);
        let on_player = shots
            .dealt_log
            .iter()
            .filter(|d| d.target == target)
            .count();
        println!(
            "{name}: shots {:?}, player hits {on_player}",
            members
                .iter()
                .map(|&m| shots.shots.iter().filter(|s| s.shooter == m).count())
                .collect::<Vec<_>>()
        );
    }
}

#[test]
fn members_do_not_shoot_through_a_bystander() {
    // A rival (matrix off: not hostile) stands idle between the member and the player.
    let mut app = gang_floor_default(TurfLayout::WholeFloor);
    set_player_armor(&mut app, 1.0e6);
    let rival = spawn_member(&mut app, 1, Vec3::new(0.0, 0.0, -6.0), Weapon::Pistol);
    let (mut app, m) = fight_on(app, Vec3::new(0.0, 0.0, -12.0), Weapon::Smg);
    assert!(
        !gang_cfg(&app).hostile(Faction::Gang(0), Faction::Gang(1)),
        "GATE BROKEN: the shipped matrix must keep gang 0 and gang 1 at peace"
    );
    let mut shots = Shots::new(&app);
    shots.run(&mut app, 960);
    let hits = shots.dealt_log.iter().filter(|d| d.target == rival).count();
    assert_eq!(hits, 0, "the rival bystander was hit");
    assert_eq!(
        gang_state(&app, rival),
        GangState::Idle,
        "the rival was provoked"
    );
    let own = shots.shots.iter().filter(|s| s.shooter == m).count();
    assert!(
        own >= 3,
        "M fired only {own} shots: it froze behind the bystander"
    );
}
