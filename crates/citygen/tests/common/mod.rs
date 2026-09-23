#![allow(dead_code)]

use citygen::{CityLayout, CityParams, generate};
use std::sync::OnceLock;

pub const SEEDS: [u64; 3] = [1, 2, 42];
pub const SWEEP: std::ops::Range<u64> = 0..32;

pub fn shipped_params() -> CityParams {
    let path = concat!(env!("CARGO_MANIFEST_DIR"), "/../../assets/world/city.ron");
    let text = std::fs::read_to_string(path)
        .unwrap_or_else(|e| panic!("GATE BROKEN: cannot read {path}: {e}"));
    let params: CityParams =
        ron::from_str(&text).unwrap_or_else(|e| panic!("GATE BROKEN: cannot parse {path}: {e}"));
    params
        .validate()
        .unwrap_or_else(|e| panic!("GATE BROKEN: {path} fails validate: {e}"));
    params
}

/// Layouts for `SEEDS` and `SWEEP`, generated once per test binary.
pub fn layouts() -> &'static [(u64, CityLayout)] {
    static LAYOUTS: OnceLock<Vec<(u64, CityLayout)>> = OnceLock::new();
    LAYOUTS.get_or_init(|| {
        let params = shipped_params();
        let mut seeds = SEEDS.to_vec();
        seeds.extend(SWEEP.filter(|s| !SEEDS.contains(s)));
        seeds
            .into_iter()
            .map(|s| {
                (
                    s,
                    generate(s, &params).unwrap_or_else(|e| panic!("seed {s}: {e}")),
                )
            })
            .collect()
    })
}

/// `(seed, hash)` pairs from `golden_hashes.txt`; line-ending independent.
pub fn golden() -> Vec<(u64, u64)> {
    include_str!("../golden_hashes.txt")
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty() && !line.starts_with('#'))
        .map(|line| {
            let (seed, hash) = line
                .split_once(' ')
                .unwrap_or_else(|| panic!("GATE BROKEN: bad golden line {line:?}"));
            let hash = hash.trim().trim_start_matches("0x");
            (
                seed.parse()
                    .unwrap_or_else(|_| panic!("GATE BROKEN: bad seed in {line:?}")),
                u64::from_str_radix(hash, 16)
                    .unwrap_or_else(|_| panic!("GATE BROKEN: bad hash in {line:?}")),
            )
        })
        .collect()
}
