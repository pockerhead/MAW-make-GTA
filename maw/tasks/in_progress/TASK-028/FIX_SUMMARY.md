# FIX_SUMMARY — TASK-028 (fixer, claude opus medium)

Commit `b93ac7b` on `infra/release-ci`, pushed. Review verdict was PASS; scope = minors 1-4 + the `--allow` nit
(orchestrator note).

## Preflight: the riskiest prescription

Claim checked first: "add the marker on the `Loading -> Playing` transition". Implemented naively as
`OnEnter(GameState::Playing)` it would also fire on Paused/Wasted/Busted -> Playing (`gta_sim/src/flow/busted.rs:40`,
`wasted.rs:99`), so the marker would mean nothing. The crate already uses `OnTransition { exited: Loading, entered:
Playing }` in `hud/mod.rs:28`, `minimap/mod.rs:50`, `visuals/city.rs:26`; used the same. The diagnosis was right,
the prescription needed the exact schedule.

## Fixed

1. **Issue 1 (seed smoke does not prove the city loaded).** `src/menu/mod.rs`: `log_city_ready` (`info!("city ready")`)
   on `OnTransition { Loading -> Playing }`. `release.yml`: the `--seed 1` smoke now also `--expect "city ready"`;
   the `smoke-logs-<platform>` upload is `if: always()` (a green run's logs are readable). ADR-002 smoke paragraph
   updated. Class: state (the game left Loading on the seed path), not "city meshes fully built".
2. **Issue 2 (Linux deps).** README "Запуск": one `apt install` line from `scratch/cr_elf_needed.txt`:
   `libasound2t64 libudev1 libwayland-client0 libxkbcommon0 libvulkan1 libxkbcommon-x11-0 libxcursor1 libxi6
   libx11-xcb1`, plus a Vulkan driver; notes `libwayland-client0` is needed under X11 too, the last four only for X11.
3. **Issue 3 (Windows first run).** README: extract the whole zip first (run from inside the zip -> no `assets/`,
   game exits, console closes); SmartScreen "More info -> Run anyway" for the unsigned exe.
4. **Issue 4 (publish not idempotent).** ADR-002 "Как выпустить релиз": recovery = `gh release delete <tag> --yes`
   (tag stays) + "Re-run failed jobs"; `upload --clobber` only if the zips are at hand. Not made idempotent in the
   workflow: the publish path cannot run before the rc tag (TASK-038), and an unexercised branch in the write-token
   job is worse than a documented manual step. Also: `gh run download` unpacks `archive: false` zip artifacts
   (implementer's note in `scratch/flips.md`), so the review's "download artifacts + upload --clobber" is not a
   working recipe and is not in the ADR. Logged as `decision`.
5. **Nit (invalid `--allow`).** `tools/package_release.py` `cmd_smoke`: `re.error` -> `GateBroken` (exit 2).

## Skipped

- Nit `--seconds 0` lower bound: not in the orchestrator's list; the expects already make it RED, not vacuous.
- Nit SHA-pinning third-party actions: pre-existing (TASK-029 composite), out of scope; review agrees.
- Nit 211 MB unstripped Linux exe: ADR keeps the default profile deliberately; no action.

## Flips (local, this Windows host; no release build, per orchestrator)

`scratch/fixer_flips.py` -> `scratch/fixer_flips.out.txt` (GREEN fixture zip from the implementer, unpacked to %TEMP%):
| Flip | Perturbed input | Exit | First message |
|---|---|---|---|
| invalid `--allow` | `--allow "("` | 2 | `smoke: GATE BROKEN: invalid --allow regex: missing ), unterminated subpattern at position 0` |
| `city ready` never printed | local release exe built before the fix, `--seed 1`, `--expect "city ready"` | 1 | `smoke: expect: 'city ready' on no line of ...fx_city.log` |
| control | same exe/run without the `city ready` expect | 0 | `expect matched: ... AdapterInfo { name: "NVIDIA GeForce RTX 4070 Ti" ...` |

`scratch/fixer_marker_probe.py` -> `scratch/fixer_marker_probe.out.txt` (new `fast` debug exe, checkout assets):
`seed: alive=True 'city ready' x1 'main menu ready' x0`; `menu: alive=True 'city ready' x0 'main menu ready' x1`.
The marker prints once on the seed path and never on a menu launch. No game process left running.

## Test results

- `cargo clippy -p gta_like --all-targets -- -D warnings` -> clean.
- `cargo test -p gta_like --bin gta_like` -> `test result: ok. 82 passed; 0 failed`.
- `python tools/qa/tree_check.py` -> `tree checks passed`; `python tools/qa/font_check.py` -> `0 missing glyphs`.
- `rustfmt --edition 2024 --check src/menu/mod.rs` -> no diff in `mod.rs` (pre-existing diffs in `screens.rs`, not
  touched).
- CI run on `b93ac7b`: see below.

## CI (commit `b93ac7b`)

- **release: success** — https://github.com/pockerhead/MAW-make-GTA/actions/runs/36280342827. Both legs green;
  `release-gates` and `publish` skipped (branch push). Smoke logs (now uploaded on success too) saved to
  `scratch/ci_run2_logs/`:
  - Linux menu: `AdapterInfo { name: "llvmpipe (LLVM 20.1.2, 256 bits)"`, `main menu ready`, 0 ERROR/panic lines.
  - Linux `--seed 1`: `city ready` at +0.16 s after `AdapterInfo` on lavapipe, 0 ERROR/panic lines over 60 s.
  - Windows menu: `AdapterInfo { name: "Microsoft Basic Render Driver"`, `main menu ready`, 0 ERROR/panic lines.
- clippy, client gates, citygen gates, repo checks: success.
- **sim gates: failure, pre-existing, not from this task.** Failing: `traffic_causes::a_left_car_seed_1`
  (`crates/gta_sim/tests/traffic_causes.rs:127`) and `traffic_go_around::dummy_street_seed_1`
  (`traffic_go_around.rs:195`). The same two tests fail on `main` at `7779605` (TASK-032 closure, run 36276095952)
  and on this branch's `652ed50`. This fix touches no `gta_sim` code. Needs its own task (TASK-032 follow-up) before
  the merge commit can show 5/5 green.
- Tag not pushed (TASK-038 pending), per orchestrator.

children: 0 launched / 0 reported.
