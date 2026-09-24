//! Wanted level, witnesses and incidents (GDD §6.4, T10) on the test floor, production composition.

mod common;
mod wanted_support;

use bevy::prelude::*;
use common::*;
use gta_sim::{
    character::{ActionIntent, AimIntent},
    civilian::{CivilianConfig, CivilianState},
    combat::{Loadout, MeleeHit, Weapon},
    perception::{Cause, StimulusLog, ThreatKind},
    wanted::Crime,
};
use wanted_support::*;

fn calls_about(probe: &Probe, about: Cause) -> usize {
    probe.call_log.iter().filter(|c| c.about == about).count()
}

fn crimes_of(app: &App) -> Vec<(Crime, bool)> {
    let mut list: Vec<_> = incidents(app)
        .iter()
        .map(|i| (i.crime, i.reported))
        .collect();
    list.sort_by_key(|&(crime, _)| crime as u8);
    list
}

fn calm_state(state: CivilianState) -> bool {
    matches!(state, CivilianState::Wander | CivilianState::Idle { .. })
}

#[test]
fn unwitnessed_kill_is_zero_heat() {
    let far = (Vec3::new(-34.0, 0.0, -30.0), Vec3::new(-26.0, 0.0, -30.0));
    let mut app = graph_app(10.0, &[far]);
    assert_shipped(&app);
    arm(&mut app, Weapon::Pistol);
    let victim = spawn_civilian(&mut app, SIDES[0], 0.8, calm());
    let bystander = spawn_civilian(&mut app, segment(0), 0.5, SHOT_CALLER);
    hold_idle(&mut app, victim);
    hold_idle(&mut app, bystander);
    run_ticks(&mut app, 16);
    let to_victim = position_of(&app, victim) - position(&mut app);
    let muzzle = muzzle_of(&mut app, to_victim);
    let far_chest = position_of(&app, bystander);
    assert!(
        muzzle.distance(far_chest) > 40.0 && position_of(&app, victim).distance(far_chest) > 20.0,
        "GATE BROKEN: the far civilian is within hearing or corpse sight"
    );
    let mut probe = Probe::new(&app);
    kill_with_one_shot(&mut app, &mut probe, victim);
    let player = player(&mut app);
    let list = incidents(&app);
    assert_eq!(
        crimes_of(&app),
        vec![(Crime::Shooting, false), (Crime::Kill, false)],
        "liveness: the crimes were recorded"
    );
    assert!(list.iter().all(|i| i.offender == player));
    let call = ticks_in(&app, app.world().resource::<CivilianConfig>().call_seconds);
    probe.run(&mut app, 2 * call + 16);
    let w = wanted(&app);
    assert_eq!((w.heat, w.stars), (0, 0), "{w:?}");
    assert!(probe.call_log.is_empty(), "{:?}", probe.call_log);
    assert!(calm_state(civilian_state(&app, bystander)));
}

#[test]
fn repeat_calls_about_one_corpse_count_once() {
    let (mut app, witness, victim) = setup_w(&[]);
    let mut probe = Probe::new(&app);
    let at_shot = position(&mut app);
    kill_with_one_shot(&mut app, &mut probe, victim);
    await_report(&mut app, &mut probe, witness, Cause::Body(victim));
    probe.run_until_calls(&mut app, 1, 256 + 16);
    let first = wanted(&app);
    assert_eq!((first.heat, first.stars), (40, 1), "{first:?}");
    assert!(
        first.last_known.unwrap().distance(at_shot) < 0.1,
        "{first:?}"
    );
    hold_idle(&mut app, witness);
    probe.run_until_calls(&mut app, 2, 256 + 16);
    assert_eq!(calls_about(&probe, Cause::Body(victim)), 2);
    let second = wanted(&app);
    assert_eq!(second.heat, 40, "a repeat call added heat: {second:?}");
    assert_eq!(second.last_known, first.last_known);
}

#[test]
fn attack_call_then_body_call_counts_each_incident_once() {
    let north = (Vec3::new(-5.0, 0.0, 30.0), Vec3::new(5.0, 0.0, 30.0));
    let (mut app, witness, victim) = setup_w(&[north]);
    let caller = spawn_civilian(&mut app, segment(0), 0.5, SHOT_CALLER);
    hold_idle(&mut app, caller);
    run_ticks(&mut app, 8);
    let to_victim = position_of(&app, victim) - position(&mut app);
    let muzzle = muzzle_of(&mut app, to_victim);
    let a = position_of(&app, caller);
    assert!(
        (25.0..=40.0).contains(&muzzle.distance(a)) && a.distance(position_of(&app, victim)) > 20.0,
        "GATE BROKEN: caller at {} m from the muzzle",
        muzzle.distance(a)
    );
    let mut probe = Probe::new(&app);
    let attack = kill_with_one_shot(&mut app, &mut probe, victim);
    await_report(&mut app, &mut probe, witness, Cause::Body(victim));
    await_report(&mut app, &mut probe, caller, Cause::Attack(attack));
    probe.run_until_calls(&mut app, 2, 256 + 16);
    assert_eq!(calls_about(&probe, Cause::Body(victim)), 1);
    assert_eq!(calls_about(&probe, Cause::Attack(attack)), 1);
    let w = wanted(&app);
    assert_eq!(w.heat, 40 + 10, "{w:?}");
}

/// `graph_app(10)` with an idle victim at (0,0,-10) and the player 1 m north of it.
fn punch_floor() -> (App, Entity) {
    let mut app = graph_app(10.0, &[]);
    assert_shipped(&app);
    let victim = spawn_civilian(&mut app, SIDES[0], 0.5, calm());
    hold_idle(&mut app, victim);
    let start = chest(&app, Vec3::new(0.0, 0.0, -9.0));
    place_player(&mut app, start);
    (app, victim)
}

/// One unarmed punch at `victim`; ticks until it lands.
fn land_punch(app: &mut App, probe: &mut Probe, victim: Entity) -> MeleeHit {
    let origin = position(app);
    let target = position_of(app, victim);
    set_aim(app, origin, target);
    let mut hits = app
        .world()
        .resource::<Messages<MeleeHit>>()
        .get_cursor_current();
    set_action(app, |a| a.fire_requested = true);
    for _ in 0..24 {
        probe.run(app, 1);
        let messages = app.world().resource::<Messages<MeleeHit>>();
        if let Some(h) = hits.read(messages).find(|h| h.target == victim) {
            return *h;
        }
    }
    panic!("GATE BROKEN: the punch never landed");
}

#[test]
fn punch_seen_by_a_civilian_is_reported() {
    let (mut app, victim) = punch_floor();
    let witness = spawn_civilian(&mut app, SIDES[1], 0.8, CORPSE_WITNESS);
    hold_idle(&mut app, witness);
    run_ticks(&mut app, 16);
    let mut probe = Probe::new(&app);
    let hit = land_punch(&mut app, &mut probe, victim);
    let d = position_of(&app, witness).distance(hit.point);
    assert!(
        (18.0..=20.0).contains(&d),
        "GATE BROKEN: the witness is {d} m from the punch"
    );
    await_report(&mut app, &mut probe, witness, Cause::Attack(hit.attack));
    probe.run_until_calls(&mut app, 1, 256 + 16);
    let w = wanted(&app);
    assert_eq!((w.heat, w.stars), (5, 0), "{w:?}");
    assert_eq!(crimes_of(&app), vec![(Crime::Punch, true)]);
}

#[test]
fn body_call_does_not_report_a_private_punch() {
    let (mut app, victim) = punch_floor();
    run_ticks(&mut app, 16);
    let mut probe = Probe::new(&app);
    land_punch(&mut app, &mut probe, victim);
    assert_eq!(
        crimes_of(&app),
        vec![(Crime::Punch, false)],
        "liveness: the punch was recorded"
    );
    // The witness arrives after the fight is no longer audible.
    probe.run(&mut app, 8);
    assert!(
        app.world()
            .resource::<StimulusLog>()
            .0
            .iter()
            .all(|&(_, kind, ..)| kind != ThreatKind::Fight),
        "GATE BROKEN: the fight is still audible"
    );
    hold_idle(&mut app, victim);
    let witness = spawn_civilian(&mut app, SIDES[1], 0.8, CORPSE_WITNESS);
    hold_idle(&mut app, witness);
    probe.run(&mut app, 16);
    let w2 = position_of(&app, witness);
    // Named mutation: an unattributed death standing in for a gang kill.
    set_health_of(&mut app, victim, |h| h.current = 0.0);
    probe.run(&mut app, 1);
    let d = w2.distance(position_of(&app, victim));
    assert!(
        d > 15.0 && d <= 20.0,
        "GATE BROKEN: the body is {d} m from the witness"
    );
    await_report(&mut app, &mut probe, witness, Cause::Body(victim));
    probe.run_until_calls(&mut app, 1, 256 + 16);
    let w = wanted(&app);
    assert_eq!(w.heat, 0, "a corpse call reported a private punch: {w:?}");
    assert_eq!(crimes_of(&app), vec![(Crime::Punch, false)]);
}

#[test]
fn killing_the_caller_interrupts_the_call() {
    let (mut app, witness, victim) = setup_w(&[]);
    let mut probe = Probe::new(&app);
    kill_with_one_shot(&mut app, &mut probe, victim);
    await_report(&mut app, &mut probe, witness, Cause::Body(victim));
    probe.run(&mut app, 24);
    assert!(matches!(
        civilian_state(&app, witness),
        CivilianState::Report { .. }
    ));
    kill_with_one_shot(&mut app, &mut probe, witness);
    probe.run(&mut app, 256 + 16);
    assert!(probe.call_log.is_empty(), "{:?}", probe.call_log);
    assert_eq!(wanted(&app).heat, 0);
}

/// `graph_app(30)` with a gunshot caller at (0,0,-30) and, optionally, a bystander at (0,0,8).
fn hearing_app(bystander: bool) -> (App, Entity) {
    let south = (Vec3::new(-5.0, 0.0, 8.0), Vec3::new(5.0, 0.0, 8.0));
    let mut app = graph_app(30.0, &[south]);
    assert_shipped(&app);
    arm(&mut app, Weapon::Pistol);
    let caller = spawn_civilian(&mut app, SIDES[0], 0.5, SHOT_CALLER);
    if bystander {
        let near = spawn_civilian(&mut app, segment(0), 0.5, calm());
        hold_idle(&mut app, near);
    }
    run_ticks(&mut app, 16);
    let muzzle = muzzle_of(&mut app, Vec3::Y);
    let d = muzzle.distance(position_of(&app, caller));
    assert!((25.0..40.0).contains(&d), "GATE BROKEN: caller at {d} m");
    (app, caller)
}

#[test]
fn shot_near_people_is_reported_by_a_hearing_caller() {
    let (mut app, caller) = hearing_app(true);
    let mut probe = Probe::new(&app);
    let muzzle = muzzle_of(&mut app, Vec3::Y);
    let attack = shoot_into_the_air(&mut app, &mut probe);
    await_report(&mut app, &mut probe, caller, Cause::Attack(attack));
    probe.run_until_calls(&mut app, 1, 256 + 16);
    let w = wanted(&app);
    assert_eq!((w.heat, w.stars), (10, 0), "{w:?}");
    assert!(w.last_known.unwrap().distance(muzzle) < 0.1, "{w:?}");
    let away = chest(&app, Vec3::new(30.0, 0.0, 30.0));
    place_player(&mut app, away);
    assert!(
        (away - w.last_known.unwrap()).xz().length() > 40.0,
        "GATE BROKEN: not outside the circle"
    );
    probe.run(&mut app, 639);
    assert_eq!(wanted(&app).heat, 10, "cleared before 640 ticks");
    probe.run(&mut app, 1);
    assert_eq!(wanted(&app), gta_sim::wanted::WantedLevel::default());
}

#[test]
fn lone_shot_is_no_crime() {
    let (mut app, caller) = hearing_app(false);
    let mut probe = Probe::new(&app);
    let attack = shoot_into_the_air(&mut app, &mut probe);
    await_report(&mut app, &mut probe, caller, Cause::Attack(attack));
    probe.run_until_calls(&mut app, 1, 256 + 16);
    assert_eq!(wanted(&app).heat, 0);
    assert!(incidents(&app).is_empty());
}

enum CopCase {
    Lethal,
    Wound,
    AlreadyDead,
}

/// Cop fixture at `cop_feet`, facing -Z; a civilian at (6,0,-10). Returns heat after the shot tick.
fn cop_case(cop_feet: Vec3, case: CopCase) -> (u32, Vec<(Crime, bool)>) {
    let mut app = graph_app(10.0, &[]);
    assert_shipped(&app);
    arm(&mut app, Weapon::Pistol);
    let victim = spawn_civilian(&mut app, SIDES[0], 0.8, calm());
    hold_idle(&mut app, victim);
    let cop_chest = chest(&app, cop_feet);
    spawn_cop(&mut app, cop_chest, 0.0);
    run_ticks(&mut app, 16);
    let mut probe = Probe::new(&app);
    match case {
        CopCase::Lethal => {
            kill_with_one_shot(&mut app, &mut probe, victim);
        }
        CopCase::Wound => {
            let target = position_of(&app, victim);
            let attack = fire_at(&mut app, &mut probe, target);
            assert!(
                probe
                    .shots
                    .dealt_log
                    .iter()
                    .any(|h| h.shot == attack && h.target == victim && !h.killed),
                "GATE BROKEN: no wounding hit"
            );
        }
        CopCase::AlreadyDead => {
            set_health_of(&mut app, victim, |h| h.current = 0.0);
            probe.run(&mut app, 8);
            shoot_into_the_air(&mut app, &mut probe);
        }
    }
    (wanted(&app).heat, crimes_of(&app))
}

#[test]
fn cop_fixture_witnesses_at_the_shot() {
    let clear = Vec3::new(-20.0, 0.0, 0.0);
    assert_eq!(
        cop_case(clear, CopCase::Lethal),
        (50, vec![(Crime::Shooting, true), (Crime::Kill, true)])
    );
    assert_eq!(
        cop_case(clear, CopCase::Wound),
        (40, vec![(Crime::Shooting, true), (Crime::Wound, true)])
    );
    assert_eq!(cop_case(clear, CopCase::AlreadyDead), (0, vec![]));
    let behind_wall = Vec3::new(0.0, 0.0, 20.0);
    assert_eq!(
        cop_case(behind_wall, CopCase::Lethal),
        (0, vec![(Crime::Shooting, false), (Crime::Kill, false)])
    );
    let too_far = Vec3::new(-36.0, 0.0, -36.0);
    assert_eq!(cop_case(too_far, CopCase::Lethal).0, 0);
}

#[test]
fn one_shotgun_blast_is_one_kill() {
    let close = (Vec3::new(-2.0, 0.0, -3.0), Vec3::new(2.0, 0.0, -3.0));
    let mut app = graph_app(10.0, &[close]);
    assert_shipped(&app);
    arm(&mut app, Weapon::Shotgun);
    let victim = spawn_civilian(&mut app, segment(0), 0.5, calm());
    hold_idle(&mut app, victim);
    let cop = chest(&app, Vec3::new(-20.0, 0.0, 0.0));
    spawn_cop(&mut app, cop, 0.0);
    run_ticks(&mut app, 16);
    // Named mutation: one pellet wounds, the second kills.
    set_health_of(&mut app, victim, |h| h.current = 10.0);
    let mut probe = Probe::new(&app);
    let target = position_of(&app, victim);
    let attack = fire_at(&mut app, &mut probe, target);
    let on_victim: Vec<bool> = probe
        .shots
        .dealt_log
        .iter()
        .filter(|h| h.shot == attack && h.target == victim)
        .map(|h| h.killed)
        .collect();
    assert!(
        on_victim.len() >= 2
            && on_victim.last() == Some(&true)
            && on_victim.iter().filter(|&&k| k).count() == 1,
        "GATE BROKEN: the blast did not wound then kill: {on_victim:?}"
    );
    assert_eq!(
        crimes_of(&app),
        vec![(Crime::Shooting, true), (Crime::Kill, true)]
    );
    assert_eq!(wanted(&app).heat, 50);
}

#[test]
fn gang_victims_follow_the_table() {
    let mut app = graph_app(10.0, &[]);
    assert_shipped(&app);
    arm(&mut app, Weapon::Pistol);
    let cop = chest(&app, Vec3::new(-20.0, 0.0, 0.0));
    spawn_cop(&mut app, cop, 0.0);
    let member = spawn_member(&mut app, 0, Vec3::new(6.0, 0.0, -10.0), Weapon::Pistol);
    run_ticks(&mut app, 16);
    let mut probe = Probe::new(&app);
    let target = position_of(&app, member);
    let attack = fire_at(&mut app, &mut probe, target);
    assert!(
        probe
            .shots
            .dealt_log
            .iter()
            .any(|h| h.shot == attack && h.target == member && !h.killed),
        "GATE BROKEN: the member was not wounded"
    );
    assert_eq!(wanted(&app).heat, 10, "a gang wound is no crime");
    assert_eq!(crimes_of(&app), vec![(Crime::Shooting, true)]);
    probe.run(&mut app, 24);
    kill_with_one_shot(&mut app, &mut probe, member);
    assert_eq!(wanted(&app).heat, 10 + 40);
    assert_eq!(
        crimes_of(&app),
        vec![(Crime::Shooting, true), (Crime::Kill, true)]
    );
}

#[test]
fn gang_crimes_are_not_the_players() {
    let (mut app, witness, victim) = setup_w(&[]);
    let member = spawn_member(&mut app, 0, Vec3::new(2.0, 0.0, -2.0), Weapon::Pistol);
    run_ticks(&mut app, 8);
    set_health_of(&mut app, victim, |h| h.current = 1.0);
    let from = position_of(&app, member);
    let to = position_of(&app, victim);
    {
        let mut aim = app.world_mut().get_mut::<AimIntent>(member).unwrap();
        aim.origin = from;
        aim.direction = (to - from).normalize();
        aim.aiming = true;
    }
    // Named mutation: the idle member draws its pistol and fires once.
    app.world_mut().get_mut::<Loadout>(member).unwrap().held = Some(Weapon::Pistol);
    app.world_mut()
        .get_mut::<ActionIntent>(member)
        .unwrap()
        .fire_requested = true;
    let mut probe = Probe::new(&app);
    for _ in 0..4 {
        probe.run(&mut app, 1);
        if probe
            .shots
            .dealt_log
            .iter()
            .any(|h| h.target == victim && h.killed)
        {
            break;
        }
    }
    let kill = probe
        .shots
        .dealt_log
        .iter()
        .find(|h| h.target == victim && h.killed)
        .expect("GATE BROKEN: the member's shot did not kill");
    assert_eq!(kill.shooter, member, "GATE BROKEN: someone else fired");
    app.world_mut().get_mut::<AimIntent>(member).unwrap().aiming = false;
    await_report(&mut app, &mut probe, witness, Cause::Body(victim));
    probe.run_until_calls(&mut app, 1, 256 + 16);
    assert_eq!(calls_about(&probe, Cause::Body(victim)), 1);
    assert_eq!(wanted(&app).heat, 0);
    assert!(incidents(&app).is_empty());
}
