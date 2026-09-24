//! Pure police decisions: cop state transitions, the dispatcher's unit kind and spawn order, the
//! arrest hold and the break-free star.

use super::{ArrestConfig, CopState, EscalationRow, UnitKind};
use crate::wanted::{STARS, StarRow, stars_for};
use bevy::prelude::*;

/// What a cop knows this tick.
#[derive(Clone, Copy, Debug)]
pub struct CopSenses {
    pub sees: bool,
    /// A witnessed attack by the player is fresh (`PoliceAlert`).
    pub hostile: bool,
    /// The current star row arrests instead of shooting.
    pub arrest_row: bool,
    pub stars: u8,
    /// Within `search_arrive_distance` of its goal (the last known position, the search point).
    pub at_goal: bool,
}

/// Next cop state; `Leave` is terminal (only death moves it on).
pub fn next_state(state: CopState, s: &CopSenses) -> CopState {
    use CopState::*;
    let arrests = s.arrest_row && !s.hostile;
    match state {
        Dead => Dead,
        Leave => Leave,
        _ if s.stars == 0 => Leave,
        Respond | Search if s.sees => {
            if arrests {
                Arrest
            } else {
                Attack
            }
        }
        Respond if s.at_goal => Search,
        Respond | Search => state,
        Arrest if !arrests => Attack,
        Arrest if !s.sees => Respond,
        Arrest => Arrest,
        Attack if !s.sees => Respond,
        Attack if arrests => Arrest,
        Attack => Attack,
    }
}

/// Kind of the next unit to spawn with `units` active of them `swat` SWAT; `None` at the row's cap.
pub fn spawn_kind(row: &EscalationRow, units: u32, swat: u32) -> Option<UnitKind> {
    if units >= row.units {
        return None;
    }
    Some(if swat < row.swat {
        UnitKind::Swat
    } else {
        UnitKind::Patrol
    })
}

/// Heat after breaking free: at least the next star's threshold (1..4 stars), else unchanged.
pub fn break_free_heat(heat: u32, rows: &[StarRow; STARS]) -> u32 {
    let stars = stars_for(heat, rows) as usize;
    if (1..STARS).contains(&stars) {
        heat.max(rows[stars].heat)
    } else {
        heat
    }
}

fn bearing(p: Vec3, centre: Vec3) -> f32 {
    let d = p - centre;
    d.z.atan2(d.x)
}

/// Angle between two bearings, in `[0, PI]`.
fn bearing_gap(a: f32, b: f32) -> f32 {
    let d = (a - b).rem_euclid(std::f32::consts::TAU);
    d.min(std::f32::consts::TAU - d)
}

/// Spawn candidate to use: the nearest to `centre` (flat; ties to the lower index), or with
/// `surround` and units already out the one whose bearing around `centre` is farthest from every
/// taken unit's bearing (ties to the nearer).
pub fn pick_spawn(
    candidates: &[Vec3],
    centre: Vec3,
    taken: &[Vec3],
    surround: bool,
) -> Option<usize> {
    let flat = |p: Vec3| (p - centre).with_y(0.0).length();
    let nearest = |best: usize, k: usize| {
        if flat(candidates[k]) < flat(candidates[best]) {
            k
        } else {
            best
        }
    };
    if !surround || taken.is_empty() {
        return (0..candidates.len()).reduce(nearest);
    }
    let spread = |p: Vec3| {
        taken
            .iter()
            .map(|&t| bearing_gap(bearing(p, centre), bearing(t, centre)))
            .fold(f32::INFINITY, f32::min)
    };
    (0..candidates.len()).reduce(|best, k| {
        let (a, b) = (spread(candidates[k]), spread(candidates[best]));
        if a > b || (a == b && flat(candidates[k]) < flat(candidates[best])) {
            k
        } else {
            best
        }
    })
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum ArrestStep {
    /// Seconds of passivity held so far.
    Hold(f32),
    BrokeFree,
    Busted,
}

/// One tick of an arrest: `distance` from the arresting cop (flat), `attacking` the player shot or
/// swings this tick, `knocked_down` the player lies on the ground.
pub fn arrest_step(
    hold: f32,
    distance: f32,
    attacking: bool,
    knocked_down: bool,
    dt: f32,
    cfg: &ArrestConfig,
) -> ArrestStep {
    if knocked_down && distance <= cfg.distance {
        return ArrestStep::Busted;
    }
    if attacking {
        return ArrestStep::Hold(0.0);
    }
    if distance <= cfg.distance {
        let held = hold + dt;
        return if held >= cfg.seconds {
            ArrestStep::Busted
        } else {
            ArrestStep::Hold(held)
        };
    }
    if distance > cfg.break_free_distance {
        return ArrestStep::BrokeFree;
    }
    ArrestStep::Hold(hold)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::police::EscalationConfig;
    use crate::wanted::WantedConfig;
    use CopState::*;

    fn shipped() -> EscalationConfig {
        let cfg: EscalationConfig =
            ron::from_str(include_str!("../../../../assets/police/escalation.ron"))
                .unwrap_or_else(|e| panic!("GATE BROKEN: police/escalation.ron: {e}"));
        let rows: Vec<(u32, u32, bool)> = cfg
            .stars
            .iter()
            .map(|r| (r.units, r.swat, r.arrest))
            .collect();
        assert_eq!(
            rows,
            [
                (2, 0, true),
                (4, 0, false),
                (6, 0, false),
                (8, 4, false),
                (12, 12, false)
            ],
            "GATE BROKEN: shipped escalation rows changed"
        );
        let a = &cfg.arrest;
        assert_eq!(
            (a.distance, a.seconds, a.break_free_distance),
            (1.5, 1.5, 3.0),
            "GATE BROKEN: shipped arrest block changed"
        );
        cfg
    }

    fn shipped_stars() -> [StarRow; STARS] {
        let cfg: WantedConfig = ron::from_str(include_str!("../../../../assets/wanted/wanted.ron"))
            .unwrap_or_else(|e| panic!("GATE BROKEN: wanted.ron: {e}"));
        assert_eq!(
            cfg.stars.each_ref().map(|row| row.heat),
            [40, 180, 550, 1200, 2400],
            "GATE BROKEN: shipped thresholds changed"
        );
        cfg.stars
    }

    fn senses(sees: bool, hostile: bool, arrest_row: bool, stars: u8, at_goal: bool) -> CopSenses {
        CopSenses {
            sees,
            hostile,
            arrest_row,
            stars,
            at_goal,
        }
    }

    #[test]
    fn next_state_table() {
        let cfg = shipped();
        for (k, row) in cfg.stars.iter().enumerate() {
            let stars = k as u8 + 1;
            for sees in [false, true] {
                for hostile in [false, true] {
                    for at_goal in [false, true] {
                        let s = senses(sees, hostile, row.arrest, stars, at_goal);
                        let arrests = row.arrest && !hostile;
                        let engaged = if arrests { Arrest } else { Attack };
                        let expected = [
                            (
                                Respond,
                                if sees {
                                    engaged
                                } else if at_goal {
                                    Search
                                } else {
                                    Respond
                                },
                            ),
                            (Search, if sees { engaged } else { Search }),
                            (
                                Arrest,
                                if !arrests {
                                    Attack
                                } else if sees {
                                    Arrest
                                } else {
                                    Respond
                                },
                            ),
                            (Attack, if !sees { Respond } else { engaged }),
                            (Leave, Leave),
                            (Dead, Dead),
                        ];
                        for (from, to) in expected {
                            assert_eq!(next_state(from, &s), to, "row {k} {from:?} {s:?}");
                        }
                    }
                }
            }
        }
        for from in [Respond, Search, Arrest, Attack, Leave] {
            assert_eq!(
                next_state(from, &senses(true, false, true, 0, true)),
                Leave,
                "{from:?} at 0 stars"
            );
        }
        assert_eq!(next_state(Dead, &senses(true, false, true, 0, true)), Dead);
    }

    #[test]
    fn spawn_kind_table() {
        let cfg = shipped();
        let expect = |row: &EscalationRow, units: u32, swat: u32| {
            if units >= row.units {
                None
            } else if swat < row.swat {
                Some(UnitKind::Swat)
            } else {
                Some(UnitKind::Patrol)
            }
        };
        for (k, row) in cfg.stars.iter().enumerate() {
            for units in 0..=13 {
                for swat in 0..=units.min(12) {
                    assert_eq!(
                        spawn_kind(row, units, swat),
                        expect(row, units, swat),
                        "row {k}: {units} units, {swat} swat"
                    );
                }
            }
            assert_eq!(spawn_kind(row, row.units, 0), None, "row {k} full");
            assert_eq!(
                spawn_kind(row, row.units - 1, row.swat),
                Some(UnitKind::Patrol),
                "row {k} last unit"
            );
        }
        let five = &cfg.stars[4];
        assert_eq!(spawn_kind(five, 0, 0), Some(UnitKind::Swat));
        assert_eq!(spawn_kind(&cfg.stars[0], 1, 0), Some(UnitKind::Patrol));
    }

    #[test]
    fn break_free_heat_table() {
        let rows = shipped_stars();
        for (heat, expected) in [
            (40, 180),
            (179, 180),
            (39, 39),
            (180, 550),
            (300, 550),
            (550, 1200),
            (1200, 2400),
            (2400, 2400),
            (5000, 5000),
        ] {
            assert_eq!(break_free_heat(heat, &rows), expected, "heat {heat}");
        }
    }

    #[test]
    fn pick_spawn_table() {
        let centre = Vec3::ZERO;
        let at = |deg: f32, r: f32| {
            let a = deg.to_radians();
            Vec3::new(r * a.cos(), 0.0, r * a.sin())
        };
        let near_10 = at(10.0, 50.0);
        let far_180 = at(180.0, 80.0);
        let candidates = [far_180, near_10, at(90.0, 60.0)];
        assert_eq!(pick_spawn(&candidates, centre, &[], false), Some(1));
        assert_eq!(
            pick_spawn(&candidates, centre, &[at(0.0, 40.0)], false),
            Some(1)
        );
        assert_eq!(pick_spawn(&candidates, centre, &[], true), Some(1));
        assert_eq!(
            pick_spawn(&candidates, centre, &[at(0.0, 40.0)], true),
            Some(0)
        );
        assert_eq!(pick_spawn(&[], centre, &[], false), None);
        // Equal distance: the lower index.
        let (east, north) = (Vec3::new(50.0, 0.0, 0.0), Vec3::new(0.0, 0.0, 50.0));
        assert_eq!(pick_spawn(&[east, north], centre, &[], false), Some(0));
        assert_eq!(pick_spawn(&[north, east], centre, &[], false), Some(0));
    }

    #[test]
    fn arrest_step_table() {
        let cfg = shipped().arrest;
        let dt = 1.0 / 64.0;
        let mut hold = 0.0;
        for step in 1..=96 {
            match arrest_step(hold, 1.2, false, false, dt, &cfg) {
                ArrestStep::Hold(h) => {
                    assert!(step < 96, "Hold at step {step}");
                    assert_eq!(h, step as f32 / 64.0, "step {step}");
                    hold = h;
                }
                ArrestStep::Busted => assert_eq!(step, 96, "Busted at step {step}"),
                ArrestStep::BrokeFree => panic!("broke free at step {step}"),
            }
        }
        for distance in [1.51, 2.0, 3.0] {
            assert_eq!(
                arrest_step(0.5, distance, false, false, dt, &cfg),
                ArrestStep::Hold(0.5),
                "paused at {distance}"
            );
        }
        assert_eq!(
            arrest_step(0.5, 3.01, false, false, dt, &cfg),
            ArrestStep::BrokeFree
        );
        assert_eq!(
            arrest_step(0.0, 1.5, false, true, dt, &cfg),
            ArrestStep::Busted
        );
        assert_eq!(
            arrest_step(0.0, 1.6, false, true, dt, &cfg),
            ArrestStep::Hold(0.0)
        );
        assert_eq!(
            arrest_step(1.0, 1.0, true, false, dt, &cfg),
            ArrestStep::Hold(0.0)
        );
    }
}
