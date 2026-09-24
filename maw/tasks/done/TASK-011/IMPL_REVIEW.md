# Implementation review — TASK-011

## Verdict

**PASS** — Shipped wanted behavior follows the final plan; headless gates pass. One unsafe tuning combination remains.

## Disconfirmation tested

Counterexample: a cop behind the test floor wall witnesses a lethal player shot and adds heat despite blocked line of sight. `cop_fixture_witnesses_at_the_shot` asserts zero heat and unreported incidents for that geometry; it passed with `cargo test -p gta_sim --test wanted cop_fixture_witnesses_at_the_shot`. The production path calls `sight_blocked` at `crates/gta_sim/src/wanted/search.rs:63`; the cop fixture supplies matching `Transform` and `Position` at `crates/gta_sim/tests/wanted_support/mod.rs:260`. The counterexample did not hold.

## Confirmed correct

- `crates/gta_sim/src/wanted/crimes.rs:60` records player crimes by attack and victim, reports an incident once, and resolves a body only to a kill. Repeat corpse, private punch, shotgun pellet, gang attribution, and cop sight gates pass in `crates/gta_sim/tests/wanted.rs`.
- `crates/gta_sim/src/wanted/search.rs:66` applies the configured circle and timer in `FixedUpdate`. `crates/gta_sim/tests/wanted_search.rs` covers hiding, re-entry, sight reset, wall obstruction, and Wasted reset.
- `crates/gta_sim/src/civilian/mod.rs:414` sends `PoliceCall` on completed Report transitions. `crates/gta_sim/src/wanted/mod.rs:162` schedules wanted after NPC decisions and clears queued calls on exit from Wasted.
- Gameplay tuning is in `assets/wanted/wanted.ron`; HUD tuning is in `assets/ui/strings.ron`. `src/hud/stars.rs` only reads gameplay state. The star look test passed three consecutive runs.
- `cargo test -p gta_sim -j 4` and `cargo clippy -p gta_sim --all-targets -j 4 -- -D warnings` passed. Recorded runtime QA summaries for t10_run1 through t10_run3 show heat 50, one star, hiding progress, clear at about 10.1 seconds, and zero logged errors. An independent `cargo build` printed Finished but its PowerShell host exited with a console title error, so that invocation is not a clean command exit.

## Issues

- **Minor — `crates/gta_sim/src/wanted/mod.rs:115`:** validation only requires `incident_memory_seconds > shooting_merge_seconds`. Setting merge to 0 and memory to 1 passes validation, while the shipped civilian call takes 4 seconds. `forget_crimes` then removes the incident before a witness finishes calling, silently yielding zero heat. Validate memory against the maximum call delay, including perception slots, or retain incidents referenced by calls.

## Missing coverage

- Add a cross-config case with incident memory shorter than call completion. The shipped 60-second memory and 4-second call are safe.

## Review notes

The logged cop fixture dead end is resolved in code and matches avian3d 0.7.0's required `Transform`. The logged BRP despawn race is addressed by teleporting and checking entity presence before mutation in `tools/qa/scenarios/t10.py:174`; the pinned Bevy Remote mutation path calls `world.entity_mut(entity)`. No unsafe code was added.

children: 0 launched / 0 reported.

