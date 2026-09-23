mod common;

use citygen::{CityLayout, generate, layout_hash};
use common::{SEEDS, golden, shipped_params};

const BLESS: &str =
    "cargo test -p citygen --test golden -- --ignored bless_print_golden --nocapture";

#[test]
fn golden_hashes_match() {
    let params = shipped_params();
    let golden = golden();
    assert_eq!(
        golden.len(),
        SEEDS.len(),
        "GATE BROKEN: golden_hashes.txt must list seeds {SEEDS:?}"
    );
    for (seed, expected) in golden {
        let layout = generate(seed, &params).unwrap_or_else(|e| panic!("seed {seed}: {e}"));
        let actual = layout_hash(&layout);
        assert_eq!(
            actual, expected,
            "seed {seed}: layout hash {actual:#018x}, golden {expected:#018x}; if the change is intended, bless: {BLESS}"
        );
    }
}

#[test]
fn same_seed_same_hash() {
    let params = shipped_params();
    let first = layout_hash(&generate(7, &params).unwrap());
    let second = layout_hash(&generate(7, &params).unwrap());
    assert_eq!(first, second);
}

/// Hash of everything except the `seed` field, so equal geometry from different seeds collides.
fn geometry_hash(layout: &CityLayout) -> u64 {
    layout_hash(&CityLayout {
        seed: 0,
        ..layout.clone()
    })
}

#[test]
fn different_seeds_differ() {
    let params = shipped_params();
    let hashes = SEEDS.map(|s| geometry_hash(&generate(s, &params).unwrap()));
    assert_ne!(hashes[0], hashes[1]);
    assert_ne!(hashes[1], hashes[2]);
    assert_ne!(hashes[0], hashes[2]);
}

#[test]
#[ignore]
fn bless_print_golden() {
    let params = shipped_params();
    for seed in SEEDS {
        let layout = generate(seed, &params).unwrap_or_else(|e| panic!("seed {seed}: {e}"));
        println!("{seed} {:#018x}", layout_hash(&layout));
    }
}
