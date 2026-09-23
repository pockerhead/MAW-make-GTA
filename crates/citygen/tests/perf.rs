mod common;

use citygen::{generate, layout_hash};
use common::{SEEDS, shipped_params};
use std::time::{Duration, Instant};

// Run in QA: cargo test -p citygen --release --test perf -- --ignored --nocapture
#[test]
#[ignore]
fn generation_under_budget() {
    let params = shipped_params();
    for seed in SEEDS {
        let start = Instant::now();
        let layout = generate(seed, &params).unwrap_or_else(|e| panic!("seed {seed}: {e}"));
        let hash = layout_hash(&layout);
        let elapsed = start.elapsed();
        println!(
            "seed {seed}: {elapsed:?}, {} buildings, {} lots, hash {hash:#018x}",
            layout.buildings.len(),
            layout.lots.len()
        );
        assert!(
            elapsed < Duration::from_secs(2),
            "seed {seed}: generation took {elapsed:?}"
        );
    }
}
