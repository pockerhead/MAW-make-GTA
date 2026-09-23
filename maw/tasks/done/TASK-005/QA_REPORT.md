# QA_REPORT — TASK-005 (GDD T4): гуманоид с анимациями

## Environment

- Direct, no services, no docker. Checkout `D:/test-gta-like`, branch `feature/t04-humanoid-animations`,
  HEAD `9646497`; base commit for comparison `664e33e` (last commit before `task: start TASK-005`).
- Assets: `python tools/fetch_assets.py --check` → `third-party packs match the manifest` (mini-characters already
  installed; `assets/third_party/mini-characters/*.glb` ignored by `.gitignore:64 /assets/third_party/*`).
- Runtime: `python tools/qa/scenarios/t4.py --out maw/tasks/in_progress/TASK-005/scratch/qa/t4`
  (the driver builds `gta_like --release --features dev`, launches with `--seed 1`, drives BRP on 127.0.0.1:15702,
  shuts the game down itself). After the run: no `gta_like` process left (`Get-Process gta_like` empty).
- Reproduce: `cargo build`; `cargo clippy -- -D warnings`; `cargo clippy -p gta_sim -p gta_like --tests -- -D warnings`;
  `cargo test -p gta_sim -p citygen`; `cargo test -p gta_like --bin gta_like`; the t4 command above.

## Test results

Existing + new suites (my runs, one cargo command at a time):

| Command | Result |
|---|---|
| `cargo build` | ok |
| `cargo clippy -- -D warnings` | ok, no warnings |
| `cargo clippy -p gta_sim -p gta_like --tests -- -D warnings` | ok |
| `cargo test -p gta_sim -p citygen` | all ok: citygen lib 8, golden 3 (+1 ignored), perf 0 (+1 ignored), properties 11; gta_sim lib 2, anim_state 4, asset_manifest 3, city 6 (+1 ignored), config 5, jump 3, movement 4, terrain 2 |
| `cargo test -p gta_like --bin gta_like` | 15/15 ok (8 `character_gate`, 7 pre-existing) |
| `python tools/fetch_assets.py --check` | ok |
| `python tools/qa/scenarios/t4.py` | PASS, exit 0 |

Failure list vs base: no failures on HEAD; the diff `664e33e..HEAD` removes no `#[test]` function
(only `preflight` signature changed and the placeholder `visualize_character` removed), so zero new failures and no swapped test.

### Independent probes (mine, not the author's scripts)

Probe source kept at `scratch/qa/qa_probe.rs.txt` (it was run as `crates/gta_sim/tests/qa_probe.rs` through the production
`headless_app()` = `compose_sim` + `MinimalPlugins`, then removed from the crate).

1. Jump sequences for gait {none, Walk, Run, Sprint} x hold {1, 3, 5, 10, 30} ticks: for hold >= 3 every case is
   `Jump -> Fall -> <ground gait>` with no ground gait mid-air (e.g. Run hold 5: `Jump 9, Fall 14, Run 127`).
   Holding Space for 400 ticks while running: `Jump 22, Fall 18, Run 360` (no endless re-jump, no stuck state).
2. 1-tick jump press (see Bug 1): body rises 0.185 m, `AnimState` shows `Jump` for 1 tick, then `Idle`/`Run` for the
   rest of the bob. Same with `jump_requested = true` (the real press path through the 0.1 s jump buffer).

### Flip-RED (mine; sha256 before/after verified `OK` for both files, working tree restored with `git checkout`)

| Sabotage | Observed |
|---|---|
| `anim.rs`: horizontal speed from `(velocity.x, velocity.y)` instead of `(x, z)` | RED `anim_state_table`, `gaits_map_to_states`, `jump_goes_up_then_falls_then_lands`; `idle_after_settle` and `anim_state_matches_post_step_velocity` stay GREEN (expected: the latter compares against the same pure fn, it gates ordering, not the rule) |
| `character.rs` `playback_rate`: drop `* config.scale()` | RED `playback_rate_worked_example`, `animator_follows_anim_state` |

Both sabotages differ from the implementer's flip list (they sabotaged thresholds/ordering, not the axis or scale).

### Runtime (BRP, release build, seed 1) — `scratch/qa/t4_run.log`, `scratch/qa/t4/summary.json`, 27 PNGs

- Model liveness: `models 1, wired_players 1`; `log_errors: []`.
- AnimState samples (every ~50-150 ms): run phase 141..1157 ms all `Run`; sprint 234..1187 ms all `Sprint`
  (`Run` at 156 ms while accelerating); walk 156..1187 ms all `Walk`; jump `Idle -> Jump (125 ms) -> Fall (203..422 ms) -> Idle (562 ms on)`.
- Screenshots I looked at:
  - `scratch/qa/t4/run_03.png` (Run): Kenney humanoid seen from behind, mid-stride, one arm swung forward, feet at the shadow.
  - `scratch/qa/t4/walk_00.png` (Idle->Walk at 156 ms): standing pose, both feet on the ground on its shadow, arms out.
  - `scratch/qa/t4/jump_00.png` (Jump, 125 ms): knees tucked, arms out, shadow offset from the feet.
  - `scratch/qa/t4/jump_01.png` (Fall, 344 ms): arms raised, legs together, shadow clearly detached below/right — body in the air.
  - The capsule placeholder is gone; the model faces the direction of travel (back to camera while moving away).
- Screenshot cadence: intervals 188..375 ms; `summary.json` reports no `cadence_misses`, although four gaps
  (e.g. run 1016 -> 1391 ms, walk 406 -> 781 ms) skipped a 200 ms mark — see Bug 2.
- FPS not measured: no criterion of T4 is about performance.

## Acceptance criteria

| Criterion | Test performed | Result |
|---|---|---|
| Headless: `AnimState` from speed and support by a pure function, table (стоит, идёт, бежит, спринт, вверх, падение) | `anim::tests::anim_state_table` (17 cases incl. all six + boundaries 0.2 / 3.15 / 5.65) green; my axis flip turns it RED; integration `anim_state` tests run through `compose_sim`; my probe of 20 jump/gait combinations | PASS |
| Runtime QA: `tools/qa/scenarios/t4.py` exists and passes via `tools/qa/brp.py` (send_keys W, W+Shift, Space; screenshots every 200 ms; AnimState via `world.get_components`) | ran it myself: exit 0, states as above, 27 PNGs, read 4 of them | PASS (cadence is ~200-375 ms, not strictly 200; Bug 2) |
| Owner-run: runs with correct animations, feet do not visibly slide | cannot be gated by a test; checklist below | OWNER |
| Every new tuning value in its GDD §12 data file, not a `const` | new Rust `const`s: `MODEL_YAW = PI` (coordinate law), `CHARACTER_VISUAL_CONFIG` (path), test-only `FULL_JUMP_TICKS`/`SHORT_HOP_TICKS`; tuning in `assets/character/locomotion.ron` (`anim_idle_speed`) and `assets/character/visual.ron` | PASS |
| `cargo build`, `cargo clippy -- -D warnings`, `cargo test -p gta_sim` (and `-p citygen`) green | run above | PASS |
| Existing tests pass | run above, no removed tests | PASS |
| (Goal) manifest records the real archive: 12 models / 7 joints x 2 skins / 32 clips | `fetch_assets.py --check` compares the rig record with the GLB bytes: ok; `shipped_manifest_is_valid` green | PASS |

## Bugs found

1. **Low — 1-tick jump press shows no airborne state while the body bobs 0.185 m.**
   Repro: probe `qa_probe_y` (`scratch/qa/qa_probe.rs.txt`): settle, set `jump_held = true` (or `jump_requested = true`)
   for exactly one fixed tick. Expected: either no hop, or `Jump`/`Fall` while the body is up. Actual: `Jump` for one tick,
   then `Idle` (or `Run` when running) while the body is +0.18..+0.04 m above rest for 40+ ticks (spring bob, Tnua reports
   ground). A human tap is several ticks long (the 3-tick case is correct), so this is visible only on a sub-16 ms tap
   (or a BRP key pulse); it is a sim-side jump quirk from T1 plus a one-tick animation flash, not a T4 rule violation.
   Not blocking.
2. **Low — `t4.py::cadence_misses` under-reports skipped 200 ms marks.** It flags only intervals > 400 ms, but a
   375 ms gap already skips a mark (run 1016 -> 1391 ms skipped 1200; walk 406 -> 781 skipped 600). The docstring
   says "gaps that skipped a 200 ms mark". Correct test: `floor(t_i/200) - floor(t_{i-1}/200) >= 2`. Evidence-only
   field (does not affect the verdict of the scenario); AnimState is still gated by the dense samples.

No Medium/High bugs found. The review's Major item (visual config overflow) is fixed in code: `validate()` uses
`Duration::try_from_secs_f32` and checks derived `scale()` and `native_speed * scale` (`src/visuals/character_config.rs:48-90`),
and gate `visual_config_rejects_values_that_break_derived_numbers` is green.

## Owner checklist (open until the owner runs it)

1. `python tools/fetch_assets.py`, then `cargo run --features fast`.
2. A Kenney humanoid ~1.8 m tall instead of the capsule; feet on the ground, not sunk and not floating (check from the side).
3. W (run), Shift+W (sprint), Alt+W (walk): the model faces the travel direction, the legs cycle at a rate that matches
   ground speed — feet do not visibly slide. Knobs if they do: `walk/run/sprint.native_speed` in `assets/character/visual.ron`.
4. Space: jump pose going up, fall pose coming down, idle after landing. Transitions cross-fade (0.15 s), no snaps;
   a light hitch Run<->Sprint is expected (same clip restarted).
5. Tint: set `tint: (1.0, 0.35, 0.35)` in `assets/character/visual.ron`, restart — clothes (and hands) turn red, head
   does not; restore `(1.0, 1.0, 1.0)`. No automated gate covers the tint.

## Disconfirmation

Counter-example tested: "a short Space tap while running makes `AnimState` show a ground gait (Run) mid-air, because
Tnua's ground sensor (2.05 m reach) still sees the floor during a hop < 1 m". Searched in `anim.rs::is_airborne` and
Tnua `jump.rs:452-490` (jump action stays active until `displacement.dot(up) <= 0`), then probed 20 gait x hold
combinations headless. Did not hold for any tap of 3+ ticks; held only in the degenerate 1-tick case (Bug 1).

## Verdict

**SHIP-PENDING-RUNTIME.** Build, clippy and all tests are green; both gameplay criteria are gated headless through the
production composition, and my own flip-RED sabotages turned the gates RED; the live BRP scenario passes and the
screenshots show the humanoid in run/idle/jump/fall poses. Two Low findings, neither breaks a criterion. The
"animations look right, feet do not slide" and tint criteria are owner-run by design (checklist above).

Cleanup: no services started; the game process exited via `brp_extras/shutdown` (none left). Probe file removed from
the crate; only `QA_REPORT.md` is new outside `scratch/` (scratch is gitignored, no binary is tracked).
