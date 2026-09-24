//! Utility choice of a civilian's reaction to a threat (GDD §6.2).

use super::{ReactionConfig, Temperament};
use crate::perception::{Threat, ThreatKind};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Reaction {
    Flee,
    Cower,
    Report,
}

/// Highest score wins; ties go Flee > Cower > Report. `allow_report` is false during a call.
pub fn choose_reaction(
    threat: &Threat,
    t: &Temperament,
    cfg: &ReactionConfig,
    allow_report: bool,
) -> Reaction {
    let d = threat.distance;
    let panic = (1.0 - d / cfg.panic_distance).clamp(0.0, 1.0);
    let flee = cfg.flee * t.flee;
    let cower = cfg.cower * t.cower * panic;
    let reportable = match threat.kind {
        ThreatKind::Corpse => true,
        ThreatKind::Gunshot => d >= cfg.report_min_distance,
        ThreatKind::Fight => d >= cfg.fight_report_min_distance,
        ThreatKind::Aimed | ThreatKind::Hurt | ThreatKind::Car => false,
    };
    let report = if allow_report && reportable {
        cfg.report * t.report
    } else {
        0.0
    };
    if flee >= cower && flee >= report {
        Reaction::Flee
    } else if cower >= report {
        Reaction::Cower
    } else {
        Reaction::Report
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::civilian::CivilianConfig;
    use bevy::math::Vec3;

    fn shipped() -> CivilianConfig {
        ron::from_str(include_str!("../../../../assets/npc/civilian.ron"))
            .unwrap_or_else(|e| panic!("GATE BROKEN: civilian.ron: {e}"))
    }

    #[test]
    fn worked_reaction_table() {
        use Reaction::*;
        use ThreatKind::*;
        let cfg = shipped().reaction;
        let rows = [
            (Gunshot, 20.0, (1.0, 1.0, 1.0), true, Flee),
            (Gunshot, 2.0, (1.0, 1.0, 1.0), true, Cower),
            (Gunshot, 10.0, (0.5, 1.5, 1.0), true, Cower),
            (Gunshot, 30.0, (0.7, 1.0, 1.5), true, Report),
            (Gunshot, 30.0, (1.0, 1.0, 1.0), true, Flee),
            (Corpse, 10.0, (0.6, 1.0, 1.4), true, Report),
            (Hurt, 0.0, (1.0, 1.0, 1.5), true, Cower),
            (Aimed, 6.0, (1.0, 1.0, 1.5), true, Flee),
            (Gunshot, 30.0, (0.7, 1.0, 1.5), false, Flee),
            (Fight, 18.0, (0.6, 1.0, 1.4), true, Report),
            (Gunshot, 18.0, (0.6, 1.0, 1.4), true, Flee),
            (Fight, 12.0, (0.6, 1.0, 1.4), true, Flee),
            (Car, 5.0, (1.0, 1.0, 1.0), true, Flee),
            (Car, 2.0, (1.0, 1.0, 1.0), true, Cower),
        ];
        for (row, (kind, distance, (flee, cower, report), allow, expected)) in
            rows.into_iter().enumerate()
        {
            let threat = Threat {
                kind,
                at: Vec3::ZERO,
                distance,
                cause: None,
            };
            let t = Temperament {
                flee,
                cower,
                report,
            };
            assert_eq!(
                choose_reaction(&threat, &t, &cfg, allow),
                expected,
                "row {}",
                row + 1
            );
        }
    }

    #[test]
    fn temperament_stays_in_its_spread() {
        let spread = shipped().reaction.temperament_spread;
        let mut rng = crate::population::NpcRng::seeded(3);
        for _ in 0..1000 {
            let t = super::super::roll_temperament(&mut rng, spread);
            for v in [t.flee, t.cower, t.report] {
                assert!((1.0 - spread..=1.0 + spread).contains(&v), "{v}");
            }
        }
    }
}
