# IMPL_REVIEW — TASK-005

## Verdict

**NEEDS_WORK** — the visual configuration validator accepts finite values that later panic or produce a nonfinite model scale.

## Disconfirmation tested

A short Space tap could leave `AnimState` at `Idle` throughout a hop because Tnua's ground sensor still touches the floor. Production `is_airborne` also checks the active Jump action (`crates/gta_sim/src/character/anim.rs:46`), and `jump_goes_up_then_falls_then_lands` exercises a 10-tick tap (`crates/gta_sim/tests/anim_state.rs:65`). The counterexample did **not** hold; the headless test passed.

## Confirmed correct

- The manifest records 12 models, seven joints per skin, and 32 ordered clips. `fetch_assets.py` compares the record with installed GLB bytes; `python tools/fetch_assets.py --check` passed (`assets/third_party/manifest.ron:50`, `tools/fetch_assets.py:310`).
- `AnimState` is derived in `gta_sim` from velocity and airborne status after Avian's physics set, with no presentation dependency (`crates/gta_sim/src/character/anim.rs:19`, `crates/gta_sim/src/character/mod.rs:70`). The headless integration tests passed.
- The client builds clip handles from manifest indices, attaches a model to the character, and drives clip selection and rate from `AnimState` (`src/visuals/character.rs:31`, `src/visuals/character.rs:75`, `src/visuals/character.rs:185`). The seven client gates passed.
- The logged tint dead end is handled by cloning the body material before multiplication; the pinned GLB probe confirms body and head share the original material (`src/visuals/character.rs:157`, `scratch/reviewer2/probe_glb.txt`). The logged short-hop dead end is covered as above. The logged `WorldInstanceReady` construction limitation is consistent with the pinned crate source; live BRP evidence reports one model and one wired player (`scratch/implementer/t4/summary.json`).
- The two GDD edits correct the archive facts without changing the slice scope (`docs/design/GDD.md:407`, `docs/design/GDD.md:589`).

## Issues

1. **Major — `src/visuals/character_config.rs:48`: incomplete numeric validation.** `blend_seconds: 2e19` is finite and nonnegative, so `validate()` accepts it, but the first state transition calls `Duration::from_secs_f32` at `src/visuals/character.rs:203`, which panics on values that overflow `Duration` (confirmed in the pinned Rust standard library source). Similarly, a positive subnormal `model_height` can make `height / model_height` infinite at line 44, producing an invalid model scale. Reject values whose derived duration, scale, or clip speed denominator is invalid during preflight. Add cases for these inputs.
2. **Minor — `tools/qa/scenarios/t4.py:126`: screenshot cadence is not checked.** The loop aims for 200 ms, but `verify_png` only checks file signatures; the recorded run includes a 282 ms interval and still passes. A slower BRP run could pass with much larger gaps and miss a brief jump pose. Check an explicit interval tolerance, or mark the screenshot evidence incomplete when the cadence is missed.

## Missing coverage

- A visual-config test for finite values that overflow `Duration` or derived model scale, plus a normal config that still validates.
- A QA cadence check for delayed screenshot calls. The nonwhite tint and visible foot sliding remain deliberate owner-run checks; they should stay on the QA owner checklist.

## Verification

`cargo test -p gta_sim -p citygen`, `cargo test -p gta_like --bin gta_like`, `cargo clippy -- -D warnings`, and `python tools/fetch_assets.py --check` passed in this review. `cargo build` could not write an existing `target/debug/.fingerprint` entry (`os error 5`), so this review cannot independently confirm that command; the error is a filesystem permission failure, not a compiler diagnostic.

children: 0 launched / 0 reported.
