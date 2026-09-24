//! Cop sight and the search circle (GDD §6.4: hybrid of IV's circle and V's view cones).

use super::{STARS, StarRow, WantedConfig, WantedLevel, stars_for};
use crate::character::{Dead, LocomotionConfig};
use crate::gang::Faction;
use crate::navigation::flat_distance;
use crate::perception::sight_blocked;
use crate::player::Player;
use avian3d::prelude::*;
use bevy::prelude::*;

/// Eye point of a character whose body centre is `chest` (perception convention).
pub(crate) fn eye(chest: Vec3, loco: &LocomotionConfig) -> Vec3 {
    chest - Vec3::Y * loco.float_height + Vec3::Y * loco.head_height
}

/// `target` within `distance` of `eye` and inside the flat cone of `cone_deg` around `forward`;
/// a target straight above or below counts as in view.
pub(crate) fn in_view(
    eye: Vec3,
    forward: Vec3,
    target: Vec3,
    cone_deg: f32,
    distance: f32,
) -> bool {
    if eye.distance(target) > distance {
        return false;
    }
    let Some(to) = (target - eye).xz().try_normalize() else {
        return true;
    };
    let Some(ahead) = forward.xz().try_normalize() else {
        return false;
    };
    ahead.dot(to) >= (cone_deg / 2.0).to_radians().cos()
}

/// A cop sees the player: view cone, distance and a clear line.
pub(crate) fn cop_sees(
    spatial: &SpatialQuery,
    cop_eye: Vec3,
    cop_forward: Vec3,
    player_eye: Vec3,
    cfg: &WantedConfig,
) -> bool {
    in_view(
        cop_eye,
        cop_forward,
        player_eye,
        cfg.cop_view_cone_deg,
        cfg.cop_view_distance,
    ) && !sight_blocked(spatial, cop_eye, player_eye)
}

/// A cop witnesses a crime in person: any direction, distance and a clear line.
pub(crate) fn witnesses(
    spatial: &SpatialQuery,
    cop_eye: Vec3,
    offender_eye: Vec3,
    cfg: &WantedConfig,
) -> bool {
    cop_eye.distance(offender_eye) <= cfg.cop_witness_distance
        && !sight_blocked(spatial, cop_eye, offender_eye)
}

/// One fixed tick of the search; heat below the first star clears by the first star's rule.
pub(crate) fn search_step(
    w: &mut WantedLevel,
    player: Vec3,
    seen: bool,
    dt: f32,
    rows: &[StarRow; STARS],
) {
    if w.heat == 0 {
        *w = WantedLevel::default();
        return;
    }
    w.stars = stars_for(w.heat, rows);
    let row = &rows[usize::from(w.stars.max(1)) - 1];
    let centre = *w.last_known.get_or_insert(player);
    w.seen = seen;
    if seen {
        w.last_known = Some(player);
        w.hidden = 0.0;
        return;
    }
    if flat_distance(player, centre) <= row.search_radius {
        w.hidden = 0.0;
        return;
    }
    w.hidden += dt;
    if w.hidden >= row.clear_seconds {
        *w = WantedLevel::default();
    }
}

#[allow(clippy::type_complexity)]
pub(super) fn track_search(
    cfg: Res<WantedConfig>,
    loco: Res<LocomotionConfig>,
    time: Res<Time<Fixed>>,
    spatial: SpatialQuery,
    mut wanted: ResMut<WantedLevel>,
    player: Query<&Position, (With<Player>, Without<Dead>)>,
    cops: Query<(&Position, &Rotation, &Faction), Without<Dead>>,
) {
    let Ok(player) = player.single() else {
        return;
    };
    let mut next = *wanted;
    let player_eye = eye(player.0, &loco);
    let seen = next.heat > 0
        && cops
            .iter()
            .filter(|(.., faction)| **faction == Faction::Police)
            .any(|(cop, rotation, _)| {
                cop_sees(
                    &spatial,
                    eye(cop.0, &loco),
                    rotation.0 * Vec3::NEG_Z,
                    player_eye,
                    &cfg,
                )
            });
    search_step(
        &mut next,
        player.0,
        seen,
        time.timestep().as_secs_f32(),
        &cfg.stars,
    );
    wanted.set_if_neq(next);
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::f32::consts::{FRAC_PI_2, PI};

    fn shipped() -> WantedConfig {
        let cfg: WantedConfig = ron::from_str(include_str!("../../../../assets/wanted/wanted.ron"))
            .unwrap_or_else(|e| panic!("GATE BROKEN: wanted.ron: {e}"));
        assert_eq!(
            (cfg.cop_view_cone_deg, cfg.cop_view_distance),
            (110.0, 35.0),
            "GATE BROKEN: shipped cop view changed"
        );
        assert_eq!(
            (
                cfg.stars[0].heat,
                cfg.stars[0].search_radius,
                cfg.stars[0].clear_seconds
            ),
            (40, 40.0, 10.0),
            "GATE BROKEN: shipped first star changed"
        );
        cfg
    }

    #[test]
    fn view_cone_table() {
        let cfg = shipped();
        let (cone, far) = (cfg.cop_view_cone_deg, cfg.cop_view_distance);
        let rows = [
            (0.0, Vec3::new(0.0, 0.0, -10.0), true),
            (0.0, Vec3::new(-5.0, 0.0, -10.0), true),
            (0.0, Vec3::new(10.0, 0.0, 0.0), false),
            (0.0, Vec3::new(0.0, 0.0, 10.0), false),
            (0.0, Vec3::new(8.090, 0.0, -5.878), true),
            (0.0, Vec3::new(8.290, 0.0, -5.592), false),
            (0.0, Vec3::new(0.0, 0.0, -34.9), true),
            (0.0, Vec3::new(0.0, 0.0, -35.1), false),
            (0.0, Vec3::new(0.0, 5.0, 0.0), true),
            (FRAC_PI_2, Vec3::new(-10.0, 0.0, 0.0), true),
            (FRAC_PI_2, Vec3::new(0.0, 0.0, -10.0), false),
            (PI, Vec3::new(0.0, 0.0, 10.0), true),
            (PI, Vec3::new(0.0, 0.0, -10.0), false),
        ];
        for (yaw, target, expected) in rows {
            let forward = Quat::from_rotation_y(yaw) * Vec3::NEG_Z;
            assert_eq!(
                in_view(Vec3::ZERO, forward, target, cone, far),
                expected,
                "yaw {yaw}, target {target}"
            );
        }
    }

    fn at_heat(heat: u32, last_known: Option<Vec3>) -> WantedLevel {
        WantedLevel {
            heat,
            last_known,
            ..default()
        }
    }

    #[test]
    fn search_step_table() {
        let rows = shipped().stars;
        let dt = 1.0 / 64.0;
        let origin = Some(Vec3::ZERO);
        let outside = Vec3::new(30.0, 0.0, 30.0);

        let mut w = at_heat(40, origin);
        search_step(&mut w, Vec3::new(20.0, 0.0, 0.0), false, dt, &rows);
        assert_eq!((w.hidden, w.stars), (0.0, 1));

        let mut w = at_heat(40, origin);
        for _ in 0..639 {
            search_step(&mut w, outside, false, dt, &rows);
        }
        assert_eq!((w.hidden, w.stars), (639.0 / 64.0, 1));
        search_step(&mut w, outside, false, dt, &rows);
        assert_eq!(w, WantedLevel::default());

        let mut w = at_heat(40, origin);
        for step in 1..=320 {
            search_step(&mut w, outside, step == 320, dt, &rows);
        }
        assert_eq!((w.hidden, w.last_known, w.seen), (0.0, Some(outside), true));

        let mut w = WantedLevel {
            stars: 3,
            last_known: origin,
            hidden: 2.0,
            ..default()
        };
        search_step(&mut w, outside, false, dt, &rows);
        assert_eq!(w, WantedLevel::default());

        let mut w = at_heat(40, None);
        search_step(&mut w, outside, false, dt, &rows);
        assert_eq!((w.last_known, w.hidden), (Some(outside), 0.0));

        let mut w = at_heat(10, origin);
        for _ in 0..639 {
            search_step(&mut w, outside, false, dt, &rows);
        }
        assert_eq!((w.heat, w.stars, w.hidden), (10, 0, 639.0 / 64.0));
        search_step(&mut w, outside, false, dt, &rows);
        assert_eq!(w, WantedLevel::default());
    }
}
