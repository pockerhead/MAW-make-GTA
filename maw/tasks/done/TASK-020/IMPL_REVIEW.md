# IMPL_REVIEW — TASK-020 (remove custom ledge assist)

## Verdict

**PASS.** Commit 4c3a561 deletes exactly what the spec lists and nothing else. The build, clippy and tests are green on my own runs.

## Disconfirmation

Counter-example tested: something outside the named files still references `LedgeAssist`, `ledge::*` or a `ledge_assist_*` config field. Candidates were the windowed client in `src/`, BRP QA scenarios in `tools/qa`, docs/README/AGENTS, and the shared test helpers. If any of these existed, the client build, `t1.py` or the strict RON loader would break.
Result: it did not hold. `rg -n -i ledge` over the repo (excluding `maw/`) returns no matches. `cargo check --workspace --all-targets` is green, and it includes the `gta_like` client.

## Log triage

`log.jsonl` is empty. There are no dead_end entries to triage.

## Confirmed correct

- The diff scope matches the spec list one to one (`git show --stat 4c3a561`, 5 files, +1/-258):
  - `crates/gta_sim/src/character/ledge.rs` deleted.
  - `crates/gta_sim/tests/ledge.rs` deleted.
  - `crates/gta_sim/src/character/mod.rs`: `mod ledge;` removed, `#[require(MoveIntent, JumpBuffer)]` no longer carries `LedgeAssist`, and the `(assist_ledge, apply_ledge).chain().after(TnuaPipelineSystems::Motors)` registration is gone. `drive_characters` and `JumpBuffer` are unchanged.
  - `crates/gta_sim/src/character/locomotion.rs`: the 4 fields are removed. The struct keeps `#[serde(deny_unknown_fields)]`.
  - `assets/character/locomotion.ron`: the same 4 keys are removed. The RON and the struct match, and `shipped_locomotion_config_loads` passes.
- No debris is left behind. `TnuaPipelineSystems` was only used by the removed registration, and it came in through the `bevy_tnua::prelude::*` glob, so no import goes unused. Every helper in `tests/common/mod.rs` is still used by at least one remaining test file. `#![allow(dead_code)]` was there before this change (it exists in 10deafc).
- Everything else from TASK-002 is kept: jump buffer, tap gate, sprint, terrain tests, and the Tnua values in RON.
- My own runs:
  - `cargo clippy -p gta_sim --all-targets -- -D warnings`: green.
  - `cargo test -p gta_sim`: 12 passed. That is lib 1, config 2, jump 3 (apex, tap, buffered late tap), movement 4 (yaw 0/90/180, sprint), terrain 2 (box blocks, stairs).
  - `cargo check --workspace --all-targets`: green.
- The summary's claims agree with the tree and with the cargo output.

## Issues

None.

## Missing coverage

None required. The spec explicitly says the natural Tnua pull-up is not gated. The owner judges control feel with `tools/qa/scenarios/t1.py`, and I did not run that windowed run. That is correct for a first-frame visible behaviour (per the project context).

## Nits

- The summary says "Deviations from plan: None", but this is small-fix mode and has no plan. The wording is harmless.
- The summary's clippy was run target by target instead of with `--all-targets`. My single `--all-targets` run confirms the result is the same.
