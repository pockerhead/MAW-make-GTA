//! Intelligent Driver Model (Treiber) with the ballistic update that stops at zero speed
//! (Treiber & Kanagaraj 2015; traffic-simulation.de/info/info_IDM.html).

use super::IdmConfig;

/// Free-road exponent δ of the IDM formula (law: part of the GDD formula).
pub const IDM_DELTA: i32 = 4;

/// IDM acceleration at speed `v` towards `v0`; `gap` = (bumper gap s, m; v − v_leader, m/s), `None`
/// on a free road. Clamped at −`max_deceleration`.
pub fn idm_acceleration(v: f32, v0: f32, gap: Option<(f32, f32)>, c: &IdmConfig) -> f32 {
    let free = 1.0 - (v / v0).powi(IDM_DELTA);
    let interaction = gap.map_or(0.0, |(s, dv)| {
        let s_star = c.min_gap
            + (v * c.time_headway
                + v * dv / (2.0 * (c.acceleration * c.comfortable_deceleration).sqrt()))
            .max(0.0);
        (s_star / s.max(1e-3)).powi(2)
    });
    (c.acceleration * (free - interaction)).max(-c.max_deceleration)
}

/// One step of `dt` at acceleration `a`: (distance travelled from `s`, new speed); a car that would
/// reverse stops where its speed reaches zero.
pub fn ballistic_step(s: f32, v: f32, a: f32, dt: f32) -> (f32, f32) {
    let next = v + a * dt;
    if next < 0.0 {
        return (s - v * v / (2.0 * a), 0.0);
    }
    (s + v * dt + a * dt * dt / 2.0, next)
}

#[cfg(test)]
mod tests {
    use super::*;

    const DT: f32 = 1.0 / 64.0;

    fn shipped() -> IdmConfig {
        IdmConfig {
            time_headway: 1.5,
            acceleration: 0.73,
            comfortable_deceleration: 1.67,
            min_gap: 2.0,
            max_deceleration: 8.0,
        }
    }

    fn close(a: f32, b: f32) -> bool {
        (a - b).abs() < 1e-4
    }

    #[test]
    fn idm_rows() {
        let c = shipped();
        assert!(close(idm_acceleration(0.0, 12.0, None, &c), 0.73));
        assert!(close(idm_acceleration(12.0, 12.0, None, &c), 0.0));
        assert!(close(
            idm_acceleration(0.0, 12.0, Some((2.0, 0.0)), &c),
            0.0
        ));
        assert!(close(
            idm_acceleration(0.0, 12.0, Some((1.0, 0.0)), &c),
            -2.19
        ));
        // s* = 2 + 18 = 20, a = 0.73·(1 − 1 − 0.4444).
        assert!(close(
            idm_acceleration(12.0, 12.0, Some((30.0, 0.0)), &c),
            -0.324_444
        ));
        // Touching bumpers clamp at the maximum deceleration.
        assert_eq!(idm_acceleration(10.0, 12.0, Some((0.0, 10.0)), &c), -8.0);
    }

    #[test]
    fn ballistic_rows() {
        let (s, v) = ballistic_step(0.0, 1.0, -8.0, DT);
        assert!(close(s, 0.014_648_4) && close(v, 0.875), "{s} {v}");
        let (s, v) = ballistic_step(0.0, 0.1, -8.0, DT);
        assert!(close(s, 0.000_625) && v == 0.0, "{s} {v}");
        assert_eq!(ballistic_step(5.0, 0.0, -2.19, DT), (5.0, 0.0));
    }
}
