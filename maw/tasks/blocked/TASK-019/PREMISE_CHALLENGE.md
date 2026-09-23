## Counter-example tested

The pre-task baseline `2ade7af` fails at least one of the exact crate walk-off or buffered-landing climbs that the task says it could perform; if so, requiring every such path to remain climbable misstates the baseline behavior.

## Primary-source investigation

- Read the probe's actual geometry and success check: `maw/tasks/blocked/TASK-018/scratch/qa/qa_probe.rs:226-283`. It creates a crate flush against a wall, drives walking and buffered jump taps, and returns `Some(tick)` only after the character crosses the wall at sufficient height. The probe runner maps `old` to `git show 2ade7af:crates/gta_sim/src/character/ledge.rs` at `maw/tasks/blocked/TASK-018/scratch/qa/run_probe.py:30-44`.
- Read the raw recorded baseline probe artifact verbatim. For the narrowest 0.25 m crates, `maw/tasks/blocked/TASK-018/scratch/qa/probe_old.txt:368,373,383` reports `walk=Some(...)`, `buf28=Some(...)`, and `buf32=Some(...)` for 0.8/1.5 m, 0.9/1.6 m, and 0.5/1.45 m crate/wall heights. The 0.35 m and 2 m rows also report success at `probe_old.txt:367,369,372,379,382`.
- Read the recorded TASK-018 probe artifact for the same narrow cases. `maw/tasks/blocked/TASK-018/scratch/qa/probe_head_full.txt:352,357,367` reports `buf32=None` for all three and `walk=None`, `buf28=None` for the 0.9 m crate and 1.6 m wall. Current implementation reads `standing_on_entity()` but re-anchors height using a downward centre `cast_ray` at `crates/gta_sim/src/character/ledge.rs:41-52`.
- Ran `cargo test -p gta_sim --test ledge -j 4` in this checkout: seven tests passed, including `walking_from_crate_clears_short_step` and `buffered_jump_from_crate_clears_short_step`. Those two tests use only the 2 m deep crate (`crates/gta_sim/tests/ledge.rs:212-267`), so their green result does not decide the narrow-crate case.

## Did it hold

No. The tested baseline crate walk-off and buffered-landing rows all show a wall crossing. The `held=None` result at `probe_old.txt:367` concerns a continuously held jump, outside the counter-example's walk-off and buffered-tap paths. The raw TASK-018 artifact shows regression on the same narrow geometry, while the current code confirms a centre-ray support re-anchor.

## Verdict

PREMISE HOLDS — `maw/tasks/blocked/TASK-018/scratch/qa/probe_old.txt:367-383` records successful baseline walk-off and buffered-tap climbs for the tested crate rows; `maw/tasks/blocked/TASK-018/scratch/qa/probe_head_full.txt:352,357,367` records narrow-crate regressions, and `crates/gta_sim/src/character/ledge.rs:41-52` contains the centre-ray support re-anchor.
