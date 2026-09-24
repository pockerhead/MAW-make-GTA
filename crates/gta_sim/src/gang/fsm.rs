//! Pure gang decisions: state transitions, the fight tactic scorer and the distance band.

use super::{GangCombatConfig, GangState, HostilityConfig};
use bevy::prelude::*;

/// What a member knows about one character this tick.
#[derive(Clone, Copy, Debug)]
pub struct Focus {
    pub distance: f32,
    /// Horizontal distance of the focus from the member's `spot`.
    pub from_post: f32,
    pub visible: bool,
}

/// Inputs of `next_state`.
#[derive(Clone, Copy, Debug)]
pub struct Senses {
    pub heat: f32,
    /// The live player; `None` if absent or dead.
    pub player: Option<Focus>,
    pub player_in_turf: bool,
    pub dwell: f32,
    pub aimed_at: bool,
    /// The live Attack/Retreat target; `None` if despawned or dead.
    pub target: Option<Focus>,
    pub target_is_player: bool,
    pub tactic: Tactic,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Tactic {
    Retreat,
    Melee,
    Shoot,
    Chase,
}

/// Utility scorer of the fight: the highest available weight wins, ties Retreat > Melee > Shoot > Chase.
pub fn choose_tactic(
    health_fraction: f32,
    distance: f32,
    visible: bool,
    has_ammo: bool,
    punching: bool,
    in_range: bool,
    cfg: &GangCombatConfig,
) -> Tactic {
    let w = &cfg.tactics;
    let score = |available: bool, weight: f32| if available { weight } else { 0.0 };
    let options = [
        (
            Tactic::Retreat,
            score(
                health_fraction < cfg.retreat_health && distance < cfg.retreat_distance,
                w.retreat,
            ),
        ),
        (
            Tactic::Melee,
            score(
                distance <= cfg.melee_distance.0 || (punching && distance <= cfg.melee_distance.1),
                w.melee,
            ),
        ),
        (
            Tactic::Shoot,
            score(visible && has_ammo && in_range, w.shoot),
        ),
        (Tactic::Chase, w.chase),
    ];
    let mut best = options[0];
    for option in &options[1..] {
        if option.1 > best.1 {
            best = *option;
        }
    }
    best.0
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Move {
    Approach,
    Hold,
    BackOff,
}

/// Keeps the distance within `(lo, hi)`.
pub fn band_move(distance: f32, (lo, hi): (f32, f32)) -> Move {
    if distance > hi {
        Move::Approach
    } else if distance < lo {
        Move::BackOff
    } else {
        Move::Hold
    }
}

/// Next state of a live member.
pub fn next_state(
    state: GangState,
    player: Option<Entity>,
    s: &Senses,
    h: &HostilityConfig,
) -> GangState {
    let give_up = |target: Option<Focus>| target.is_none() || (s.target_is_player && s.heat <= 0.0);
    match state {
        GangState::Idle | GangState::Warn => {
            if let (Some(player), Some(p)) = (player, s.player)
                && s.heat > 0.0
                && s.player_in_turf
                && p.visible
                && p.distance <= h.sight_distance
                && p.from_post <= h.leash_distance
            {
                return GangState::Attack { target: player };
            }
            if state == GangState::Idle {
                if s.player_in_turf && (s.dwell >= h.warn_seconds || s.aimed_at) {
                    return GangState::Warn;
                }
                return GangState::Idle;
            }
            let released = s
                .player
                .is_none_or(|p| p.distance > h.warn_release_distance);
            if released || !s.player_in_turf {
                return GangState::Idle;
            }
            GangState::Warn
        }
        GangState::Attack { target } => {
            if give_up(s.target) || s.target.is_some_and(|t| t.from_post > h.leash_distance) {
                return GangState::Idle;
            }
            if s.tactic == Tactic::Retreat {
                return GangState::Retreat { from: target };
            }
            state
        }
        GangState::Retreat { .. } => {
            if give_up(s.target) {
                return GangState::Idle;
            }
            state
        }
        GangState::Dead => state,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::gang::{Faction, GangConfig};

    fn cfg() -> GangConfig {
        let cfg: GangConfig = ron::from_str(include_str!("../../../../assets/gang/gangs.ron"))
            .expect("GATE BROKEN: gang/gangs.ron does not parse");
        let h = &cfg.hostility;
        let c = &cfg.combat;
        let w = &c.tactics;
        // The tables below are derived from these values.
        assert!(
            (h.warn_distance, h.warn_seconds, h.warn_release_distance) == (8.0, 3.0, 12.0)
                && (h.sight_distance, h.leash_distance) == (40.0, 60.0)
                && (w.retreat, w.melee, w.shoot, w.chase) == (3.0, 2.0, 1.0, 0.5)
                && (c.retreat_health, c.retreat_distance) == (0.3, 25.0)
                && c.melee_distance == (1.5, 2.5)
                && c.keep_distance == (8.0, 15.0),
            "GATE BROKEN: gangs.ron changed, re-derive the tables"
        );
        cfg
    }

    fn focus(distance: f32, from_post: f32, visible: bool) -> Option<Focus> {
        Some(Focus {
            distance,
            from_post,
            visible,
        })
    }

    fn senses() -> Senses {
        Senses {
            heat: 0.0,
            player: focus(5.0, 5.0, true),
            player_in_turf: true,
            dwell: 0.0,
            aimed_at: false,
            target: None,
            target_is_player: false,
            tactic: Tactic::Chase,
        }
    }

    #[test]
    fn next_state_table() {
        let cfg = cfg();
        let h = &cfg.hostility;
        let player = Entity::from_raw_u32(7).unwrap();
        let rival = Entity::from_raw_u32(9).unwrap();
        let attack = GangState::Attack { target: player };
        let retreat = GangState::Retreat { from: player };
        let on_player = |target: Option<Focus>| Senses {
            target,
            target_is_player: true,
            ..senses()
        };
        let rows: Vec<(u32, GangState, Senses, GangState)> = vec![
            (
                1,
                GangState::Idle,
                Senses {
                    dwell: 191.0 / 64.0,
                    ..senses()
                },
                GangState::Idle,
            ),
            (
                2,
                GangState::Idle,
                Senses {
                    dwell: 3.0,
                    ..senses()
                },
                GangState::Warn,
            ),
            (
                3,
                GangState::Idle,
                Senses {
                    player_in_turf: false,
                    aimed_at: true,
                    ..senses()
                },
                GangState::Idle,
            ),
            (
                4,
                GangState::Idle,
                Senses {
                    player: focus(10.0, 10.0, true),
                    aimed_at: true,
                    ..senses()
                },
                GangState::Warn,
            ),
            (
                5,
                GangState::Idle,
                Senses {
                    heat: 50.0,
                    player: focus(30.0, 30.0, true),
                    ..senses()
                },
                attack,
            ),
            (
                6,
                GangState::Idle,
                Senses {
                    heat: 50.0,
                    player: focus(45.0, 45.0, true),
                    ..senses()
                },
                GangState::Idle,
            ),
            (
                7,
                GangState::Idle,
                Senses {
                    heat: 50.0,
                    player: focus(10.0, 10.0, true),
                    player_in_turf: false,
                    ..senses()
                },
                GangState::Idle,
            ),
            (
                8,
                GangState::Warn,
                Senses {
                    player: focus(12.5, 12.5, true),
                    ..senses()
                },
                GangState::Idle,
            ),
            (
                9,
                GangState::Warn,
                Senses {
                    player: focus(11.0, 11.0, true),
                    ..senses()
                },
                GangState::Warn,
            ),
            (
                10,
                GangState::Warn,
                Senses {
                    player_in_turf: false,
                    ..senses()
                },
                GangState::Idle,
            ),
            (
                11,
                attack,
                Senses {
                    heat: 0.0,
                    ..on_player(focus(12.0, 12.0, true))
                },
                GangState::Idle,
            ),
            (
                12,
                attack,
                Senses {
                    heat: 10.0,
                    ..on_player(focus(12.0, 61.0, true))
                },
                GangState::Idle,
            ),
            (
                13,
                attack,
                Senses {
                    heat: 10.0,
                    tactic: Tactic::Retreat,
                    ..on_player(focus(12.0, 12.0, true))
                },
                retreat,
            ),
            (
                14,
                attack,
                Senses {
                    heat: 10.0,
                    ..on_player(None)
                },
                GangState::Idle,
            ),
            (
                15,
                retreat,
                Senses {
                    heat: 10.0,
                    ..on_player(focus(12.0, 12.0, true))
                },
                retreat,
            ),
            (
                16,
                retreat,
                Senses {
                    heat: 0.0,
                    ..on_player(focus(12.0, 12.0, true))
                },
                GangState::Idle,
            ),
            (
                17,
                GangState::Attack { target: rival },
                Senses {
                    heat: 0.0,
                    target: focus(12.0, 20.0, true),
                    ..senses()
                },
                GangState::Attack { target: rival },
            ),
        ];
        for (row, state, s, expected) in rows {
            assert_eq!(
                next_state(state, Some(player), &s, h),
                expected,
                "row {row}"
            );
        }
        assert_eq!(
            next_state(GangState::Dead, Some(player), &senses(), h),
            GangState::Dead
        );
    }

    #[test]
    fn choose_tactic_table() {
        let cfg = cfg();
        let c = &cfg.combat;
        for (row, (hp, d, visible, ammo, punching, in_range), expected) in [
            (1, (1.0, 12.0, true, true, false, true), Tactic::Shoot),
            (2, (1.0, 12.0, false, true, false, true), Tactic::Chase),
            (3, (1.0, 1.2, true, true, false, true), Tactic::Melee),
            (4, (1.0, 2.0, true, true, true, true), Tactic::Melee),
            (5, (1.0, 2.0, true, true, false, true), Tactic::Shoot),
            (6, (1.0, 3.0, true, true, true, true), Tactic::Shoot),
            (7, (0.25, 12.0, true, true, false, true), Tactic::Retreat),
            (8, (0.25, 30.0, true, true, false, true), Tactic::Shoot),
            (9, (0.31, 12.0, true, true, false, true), Tactic::Shoot),
            (10, (1.0, 12.0, true, false, false, true), Tactic::Chase),
            (11, (0.1, 1.0, true, true, false, true), Tactic::Retreat),
            (12, (1.0, 70.0, true, true, false, false), Tactic::Chase),
        ] {
            assert_eq!(
                choose_tactic(hp, d, visible, ammo, punching, in_range, c),
                expected,
                "row {row}"
            );
        }
    }

    #[test]
    fn band_move_table() {
        let band = cfg().combat.keep_distance;
        for (d, expected) in [
            (20.0, Move::Approach),
            (15.0, Move::Hold),
            (8.0, Move::Hold),
            (5.0, Move::BackOff),
            (15.5, Move::Approach),
        ] {
            assert_eq!(band_move(d, band), expected, "{d}");
        }
    }

    #[test]
    fn shipped_faction_matrix() {
        let cfg = cfg();
        assert!(cfg.hostile(Faction::Gang(0), Faction::Player));
        assert!(cfg.hostile(Faction::Player, Faction::Gang(1)));
        assert!(!cfg.hostile(Faction::Gang(0), Faction::Gang(1)));
        assert!(!cfg.hostile(Faction::Gang(1), Faction::Gang(0)));
        assert!(!cfg.hostile(Faction::Gang(0), Faction::Police));
        assert!(!cfg.hostile(Faction::Gang(0), Faction::Gang(0)));
        cfg.validate().expect("shipped gangs.ron validates");
    }
}
