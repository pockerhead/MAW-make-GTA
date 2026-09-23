use crate::{CityParams, rng};
use rand_chacha::ChaCha8Rng;

pub(crate) struct GridLines {
    pub x: Vec<f32>,
    pub z: Vec<f32>,
    pub cx: usize,
    pub cz: usize,
}

pub(crate) fn split_feasible(len: f32, lo: f32, hi: f32) -> bool {
    len.is_finite()
        && lo.is_finite()
        && hi.is_finite()
        && len > 0.0
        && lo > 0.0
        && hi >= lo
        && (len / hi).ceil() * lo <= len
}

fn split_interval(len: f32, lo: f32, hi: f32, rng: &mut ChaCha8Rng) -> Option<Vec<f32>> {
    if !split_feasible(len, lo, hi) {
        return None;
    }
    let n = (len / hi).ceil() as usize;
    let base = len / n as f32;
    let delta = (base - lo).min(hi - base);
    let mut offsets = (0..n)
        .map(|_| rng::range_f32(rng, -delta / 2.0, delta / 2.0))
        .collect::<Vec<_>>();
    let mean = offsets.iter().sum::<f32>() / n as f32;
    offsets.iter_mut().for_each(|o| *o = base + *o - mean);
    Some(offsets)
}

// Lines along one axis, ascending, with the centre line 0 at the returned index.
fn axis(seed: u64, params: &CityParams, tags: [u64; 4]) -> (Vec<f32>, usize) {
    let core = params.grid.core_radius;
    let half = params.size / 2.0;
    let half_axis = |sign: f32, core_tag: u64, outer_tag: u64| {
        let mut lines = Vec::new();
        let mut at = 0.0;
        for (start, end, pair, tag) in [
            (0.0, core, params.grid.core_block, core_tag),
            (core, half, params.grid.outer_block, outer_tag),
        ] {
            let mut r = rng::stream(seed, tag, 0);
            let steps =
                split_interval(end - start, pair.0, pair.1, &mut r).expect("validated grid");
            for step in steps {
                at += step;
                lines.push(sign * at);
            }
            // Interval ends are exact, not accumulated sums.
            at = end;
            *lines.last_mut().expect("n >= 1") = sign * end;
        }
        lines
    };
    let mut neg = half_axis(-1.0, tags[1], tags[3]);
    let pos = half_axis(1.0, tags[0], tags[2]);
    let center = neg.len();
    neg.reverse();
    neg.push(0.0);
    neg.extend(pos);
    (neg, center)
}

pub(crate) fn build(seed: u64, params: &CityParams) -> GridLines {
    let (x, cx) = axis(
        seed,
        params,
        [
            rng::GRID_CORE_POS_X,
            rng::GRID_CORE_NEG_X,
            rng::GRID_OUTER_POS_X,
            rng::GRID_OUTER_NEG_X,
        ],
    );
    let (z, cz) = axis(
        seed,
        params,
        [
            rng::GRID_CORE_POS_Z,
            rng::GRID_CORE_NEG_Z,
            rng::GRID_OUTER_POS_Z,
            rng::GRID_OUTER_NEG_Z,
        ],
    );
    GridLines { x, z, cx, cz }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn split_steps_stay_in_range() {
        for (len, lo, hi) in [
            (250.0, 70.0, 90.0),
            (350.0, 100.0, 160.0),
            (160.0, 70.0, 90.0),
            (90.0, 70.0, 90.0),
        ] {
            for seed in 0..1000 {
                let mut r = rng::stream(seed, 99, 0);
                let steps = split_interval(len, lo, hi, &mut r).unwrap();
                assert!(steps.iter().all(|s| *s >= lo - 1e-3 && *s <= hi + 1e-3));
                assert!((steps.iter().sum::<f32>() - len).abs() < 1e-3);
            }
        }
        for len in [91.0, 130.0] {
            assert!(split_interval(len, 70.0, 90.0, &mut rng::stream(0, 99, 0)).is_none());
        }
    }
}
