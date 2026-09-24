//! Player crimes as incidents: each adds its heat once, on its first report (GDD §6.4).

use super::search::{eye, witnesses};
use super::{HeatTable, WantedConfig, WantedLevel, apply_report};
use crate::character::{Dead, Health, LocomotionConfig};
use crate::civilian::{Civilian, PoliceCall};
use crate::combat::{DamageDealt, MeleeHit, ShotFired};
use crate::gang::{Faction, GangMember};
use crate::perception::Cause;
use crate::player::Player;
use crate::police::PoliceUnit;
use avian3d::prelude::*;
use bevy::prelude::*;
use std::collections::HashSet;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Crime {
    Punch,
    Shooting,
    Wound,
    Kill,
    PunchCop,
    WoundCop,
    KillCop,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub(crate) struct IncidentId(u32);

/// One crime: a shooting episode of an offender, or what the offender did to one victim.
#[derive(Clone, Debug)]
pub struct Incident {
    pub(crate) id: IncidentId,
    pub crime: Crime,
    pub offender: Entity,
    pub victim: Option<Entity>,
    /// `(attack id, time)` of every attack that made or extended it.
    pub(crate) attacks: Vec<(u32, f64)>,
    /// Where the offender was: its body for a victim crime, the muzzle for a shooting.
    pub at: Vec3,
    /// Fixed-clock time of the last attack, s.
    pub last: f64,
    pub reported: bool,
}

/// Recent player crimes, witnessed or not.
#[derive(Resource, Default)]
pub struct Crimes {
    next: u32,
    incidents: Vec<Incident>,
}

impl Crimes {
    pub fn incidents(&self) -> &[Incident] {
        &self.incidents
    }

    pub(crate) fn clear(&mut self) {
        self.incidents.clear();
    }

    /// Extends the matching incident or starts a new one.
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn record(
        &mut self,
        crime: Crime,
        offender: Entity,
        victim: Option<Entity>,
        attack: u32,
        at: Vec3,
        now: f64,
        merge: f32,
    ) -> IncidentId {
        let matching = self.incidents.iter_mut().rev().find(|i| {
            i.offender == offender
                && i.crime == crime
                && match crime {
                    Crime::Shooting => now - i.last <= f64::from(merge),
                    _ => i.victim == victim,
                }
        });
        if let Some(incident) = matching {
            if !incident.attacks.iter().any(|&(a, _)| a == attack) {
                incident.attacks.push((attack, now));
            }
            incident.at = at;
            incident.last = now;
            return incident.id;
        }
        self.next += 1;
        let id = IncidentId(self.next);
        self.incidents.push(Incident {
            id,
            crime,
            offender,
            victim,
            attacks: vec![(attack, now)],
            at,
            last: now,
            reported: false,
        });
        id
    }

    /// Heat and place of an incident on its first report; `None` after that.
    pub(crate) fn report(&mut self, id: IncidentId, heat: &HeatTable) -> Option<(u32, Vec3)> {
        let incident = self.incidents.iter_mut().find(|i| i.id == id)?;
        if incident.reported {
            return None;
        }
        incident.reported = true;
        Some((heat.of(incident.crime), incident.at))
    }

    /// Incidents a witness can be talking about, oldest first. A body proves a death, not older
    /// punches or wounds of the same victim.
    pub(crate) fn resolve(&self, cause: Cause) -> Vec<IncidentId> {
        self.incidents
            .iter()
            .filter(|i| match cause {
                Cause::Attack(a) => i.attacks.iter().any(|&(id, _)| id == a),
                Cause::Body(v) => {
                    matches!(i.crime, Crime::Kill | Crime::KillCop) && i.victim == Some(v)
                }
            })
            .map(|i| i.id)
            .collect()
    }

    /// Drops incidents and attacks older than `memory`; bounds the attack lists.
    pub(crate) fn forget(&mut self, now: f64, memory: f32) {
        let memory = f64::from(memory);
        self.incidents.retain(|i| now - i.last <= memory);
        for incident in &mut self.incidents {
            incident.attacks.retain(|&(_, t)| now - t <= memory);
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Victim {
    Civilian,
    Gang,
    Cop,
    Other,
}

/// Crime of one player hit; gang wounds and hits on anyone else are none.
pub(crate) fn classify(victim: Victim, melee: bool, killed: bool) -> Option<Crime> {
    match (victim, killed) {
        (Victim::Civilian | Victim::Gang, true) => Some(Crime::Kill),
        (Victim::Civilian, false) if melee => Some(Crime::Punch),
        (Victim::Civilian, false) => Some(Crime::Wound),
        (Victim::Cop, true) => Some(Crime::KillCop),
        (Victim::Cop, false) if melee => Some(Crime::PunchCop),
        (Victim::Cop, false) => Some(Crime::WoundCop),
        _ => None,
    }
}

#[allow(clippy::type_complexity, clippy::too_many_arguments)]
pub(super) fn record_crimes(
    cfg: Res<WantedConfig>,
    loco: Res<LocomotionConfig>,
    time: Res<Time<Fixed>>,
    spatial: SpatialQuery,
    mut crimes: ResMut<Crimes>,
    mut wanted: ResMut<WantedLevel>,
    mut shots: MessageReader<ShotFired>,
    mut hits: MessageReader<MeleeHit>,
    mut dealt: MessageReader<DamageDealt>,
    players: Query<&Position, With<Player>>,
    kinds: Query<(Has<Civilian>, Has<GangMember>, Has<PoliceUnit>)>,
    persons: Query<
        (Entity, &Position, &Health),
        Or<(With<Civilian>, With<GangMember>, With<PoliceUnit>)>,
    >,
    cops: Query<(&Position, &Faction), Without<Dead>>,
) {
    let now = time.elapsed_secs_f64();
    let merge = cfg.shooting_merge_seconds;
    let melee: HashSet<u32> = hits.read().map(|hit| hit.attack).collect();
    let dealt: Vec<DamageDealt> = dealt.read().copied().collect();
    let killed: HashSet<(u32, Entity)> = dealt
        .iter()
        .filter(|hit| hit.killed)
        .map(|hit| (hit.shot, hit.target))
        .collect();
    let mut touched = Vec::new();
    // Crimes on a cop: the victim is the witness.
    let mut always = Vec::new();
    for hit in &dealt {
        let Ok(at) = players.get(hit.shooter) else {
            continue;
        };
        // Pellets that only wounded a victim the same blast killed are part of the kill.
        if !hit.killed && killed.contains(&(hit.shot, hit.target)) {
            continue;
        }
        let victim = match kinds.get(hit.target) {
            Ok((true, _, _)) => Victim::Civilian,
            Ok((false, true, _)) => Victim::Gang,
            Ok((false, false, true)) => Victim::Cop,
            _ => Victim::Other,
        };
        let Some(crime) = classify(victim, melee.contains(&hit.shot), hit.killed) else {
            continue;
        };
        let id = crimes.record(
            crime,
            hit.shooter,
            Some(hit.target),
            hit.shot,
            at.0,
            now,
            merge,
        );
        if victim == Victim::Cop {
            always.push(id);
        } else {
            touched.push(id);
        }
    }
    for id in always {
        if let Some((heat, at)) = crimes.report(id, &cfg.heat) {
            apply_report(&mut wanted, heat, at);
        }
    }
    for shot in shots.read() {
        if !players.contains(shot.shooter) {
            continue;
        }
        // "Near" is judged at the shot: a person this shot killed still counts, an older body does not.
        // A person killed earlier in the same tick by another attack counts as dead (sub-tick edge).
        let near = persons.iter().any(|(person, position, health)| {
            position.0.distance(shot.muzzle) <= cfg.shooting_radius
                && (health.current > 0.0 || killed.contains(&(shot.attack, person)))
        });
        if near {
            let id = crimes.record(
                Crime::Shooting,
                shot.shooter,
                None,
                shot.attack,
                shot.muzzle,
                now,
                merge,
            );
            touched.push(id);
        }
    }
    if touched.is_empty() {
        return;
    }
    let Ok(player) = players.single() else {
        return;
    };
    let offender_eye = eye(player.0, &loco);
    let witnessed = cops
        .iter()
        .filter(|(_, faction)| **faction == Faction::Police)
        .any(|(cop, _)| witnesses(&spatial, eye(cop.0, &loco), offender_eye, &cfg));
    if !witnessed {
        return;
    }
    for id in touched {
        if let Some((heat, at)) = crimes.report(id, &cfg.heat) {
            apply_report(&mut wanted, heat, at);
        }
    }
}

/// Turns completed civilian calls into heat; a call about already reported crimes adds nothing.
pub(super) fn take_calls(
    cfg: Res<WantedConfig>,
    mut calls: MessageReader<PoliceCall>,
    mut crimes: ResMut<Crimes>,
    mut wanted: ResMut<WantedLevel>,
) {
    for call in calls.read() {
        let mut heat = 0u32;
        let mut last_at = None;
        for id in crimes.resolve(call.about) {
            let Some((h, at)) = crimes.report(id, &cfg.heat) else {
                continue;
            };
            heat = heat.saturating_add(h);
            last_at = Some(at);
        }
        let Some(at) = last_at else {
            continue;
        };
        apply_report(&mut wanted, heat, at);
    }
}

pub(super) fn forget_crimes(
    cfg: Res<WantedConfig>,
    time: Res<Time<Fixed>>,
    mut crimes: ResMut<Crimes>,
) {
    crimes.forget(time.elapsed_secs_f64(), cfg.incident_memory_seconds);
}

#[cfg(test)]
mod tests {
    use super::*;

    const HEAT: HeatTable = HeatTable {
        punch_civilian: 5,
        shooting_near_people: 10,
        wound_civilian: 30,
        kill_person: 40,
        punch_cop: 45,
        wound_cop: 80,
        kill_cop: 150,
    };

    fn entity(index: u32) -> Entity {
        Entity::from_raw_u32(index).expect("GATE BROKEN: entity index")
    }

    #[test]
    fn classify_table() {
        use Victim::*;
        let rows = [
            (Civilian, true, false, Some(Crime::Punch)),
            (Civilian, false, false, Some(Crime::Wound)),
            (Civilian, true, true, Some(Crime::Kill)),
            (Civilian, false, true, Some(Crime::Kill)),
            (Gang, true, false, None),
            (Gang, false, false, None),
            (Gang, false, true, Some(Crime::Kill)),
            (Cop, true, false, Some(Crime::PunchCop)),
            (Cop, false, false, Some(Crime::WoundCop)),
            (Cop, false, true, Some(Crime::KillCop)),
            (Other, true, false, None),
            (Other, false, true, None),
        ];
        for (victim, melee, killed, expected) in rows {
            assert_eq!(
                classify(victim, melee, killed),
                expected,
                "{victim:?} melee {melee} killed {killed}"
            );
        }
    }

    #[test]
    fn one_body_one_contribution() {
        let (p, v) = (entity(1), entity(2));
        let at = Vec3::new(1.0, 2.0, 3.0);
        let mut crimes = Crimes::default();
        let id = crimes.record(Crime::Kill, p, Some(v), 7, at, 0.0, 5.0);
        assert_eq!(crimes.resolve(Cause::Body(v)), vec![id]);
        assert_eq!(crimes.report(id, &HEAT), Some((40, at)));
        assert_eq!(crimes.resolve(Cause::Body(v)), vec![id]);
        assert_eq!(crimes.report(id, &HEAT), None);
        assert_eq!(crimes.resolve(Cause::Attack(7)), vec![id]);
        assert_eq!(crimes.report(id, &HEAT), None);
    }

    #[test]
    fn body_resolves_only_the_kill() {
        let (p, v) = (entity(1), entity(2));
        let mut crimes = Crimes::default();
        crimes.record(Crime::Punch, p, Some(v), 3, Vec3::ZERO, 0.0, 5.0);
        crimes.record(Crime::Wound, p, Some(v), 4, Vec3::ZERO, 0.1, 5.0);
        assert_eq!(crimes.resolve(Cause::Body(v)), vec![]);
        let kill = crimes.record(Crime::Kill, p, Some(v), 5, Vec3::ZERO, 0.2, 5.0);
        assert_eq!(crimes.resolve(Cause::Body(v)), vec![kill]);
    }

    #[test]
    fn shooting_episode_merges() {
        let p = entity(1);
        let mut crimes = Crimes::default();
        let first = crimes.record(Crime::Shooting, p, None, 1, Vec3::ZERO, 0.0, 5.0);
        let again = crimes.record(Crime::Shooting, p, None, 2, Vec3::ZERO, 1.0, 5.0);
        assert_eq!(first, again);
        assert_eq!(crimes.incidents().len(), 1);
        let ids: Vec<u32> = crimes.incidents()[0].attacks.iter().map(|a| a.0).collect();
        assert_eq!(ids, vec![1, 2]);
        let second = crimes.record(Crime::Shooting, p, None, 3, Vec3::ZERO, 7.0, 5.0);
        assert_ne!(second, first);
        let report = |crimes: &mut Crimes, attack: u32| {
            let ids = crimes.resolve(Cause::Attack(attack));
            assert_eq!(ids.len(), 1, "attack {attack}");
            crimes.report(ids[0], &HEAT).map(|(heat, _)| heat)
        };
        assert_eq!(report(&mut crimes, 1), Some(10));
        assert_eq!(report(&mut crimes, 2), None);
        assert_eq!(report(&mut crimes, 3), Some(10));
    }

    #[test]
    fn forget_after_memory() {
        let (p, v) = (entity(1), entity(2));
        let mut crimes = Crimes::default();
        let id = crimes.record(Crime::Kill, p, Some(v), 9, Vec3::ZERO, 0.0, 5.0);
        crimes.forget(60.0, 60.0);
        assert_eq!(crimes.resolve(Cause::Body(v)), vec![id]);
        crimes.forget(60.5, 60.0);
        assert!(crimes.incidents().is_empty());
        assert_eq!(crimes.resolve(Cause::Body(v)), vec![]);
        assert_eq!(crimes.resolve(Cause::Attack(9)), vec![]);
    }

    #[test]
    fn attack_list_is_bounded() {
        let p = entity(1);
        let mut crimes = Crimes::default();
        let mut id = None;
        for k in 0..=100u32 {
            id = Some(crimes.record(Crime::Shooting, p, None, k, Vec3::ZERO, f64::from(k), 5.0));
        }
        crimes.forget(100.0, 60.0);
        assert_eq!(crimes.incidents().len(), 1);
        assert_eq!(crimes.incidents()[0].attacks.len(), 61);
        assert_eq!(crimes.resolve(Cause::Attack(10)), vec![]);
        assert_eq!(crimes.resolve(Cause::Attack(50)), vec![id.unwrap()]);
    }

    #[test]
    fn victim_crimes_dedupe_per_victim() {
        let (p, v, w) = (entity(1), entity(2), entity(3));
        let mut crimes = Crimes::default();
        let a = crimes.record(Crime::Wound, p, Some(v), 4, Vec3::ZERO, 0.0, 5.0);
        let b = crimes.record(Crime::Wound, p, Some(v), 5, Vec3::ZERO, 0.5, 5.0);
        assert_eq!(a, b);
        let c = crimes.record(Crime::Wound, p, Some(w), 6, Vec3::ZERO, 0.6, 5.0);
        assert_ne!(a, c);
        crimes.record(Crime::Wound, p, Some(v), 4, Vec3::ZERO, 0.7, 5.0);
        assert_eq!(crimes.incidents().len(), 2);
        let ids: Vec<u32> = crimes.incidents()[0].attacks.iter().map(|x| x.0).collect();
        assert_eq!(ids, vec![4, 5]);
    }
}
