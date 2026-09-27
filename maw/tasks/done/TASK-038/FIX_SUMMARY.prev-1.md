# FIX_SUMMARY — TASK-038 (fixer, small-fix)

Cost of error: silent (pass-through / stuck car on some machines only) — full evidence layer.

## Preflight

- Scratch read as a coverage map (implementer probes, WSL scripts, reviewer probes `zz_cr038_*`).
- The review claim that would break correct code if applied verbatim: **I5, "a loader check
  `conflict_margin >= some minimum tied to BODY_SAMPLE_STEP`"**. Checked in `graph.rs:108-160` with a probe
  (`scratch/zz_fix038_step.rs`, `probe_step`): at the 0.2 m centre step the corners of a car on a right-turn
  pivot move **0.83-0.98 m** between samples on seeds 1..8 (notch-depth bound up to 0.48 m per body, against
  0.15 m of half margin). A rigorous floor derived from the step would reject the shipped `0.3` and the game
  would exit at load. On the other side, `probe_margin_calibration` builds the table with `conflict_margin`
  0.01 / 0.1 / 0.2 and finds **0 unmargined touches under a 0.03 m oracle on all 8 seeds**
  (`scratch/fix_margin_calibration.log`), so any floor above 0 would be an invented number. The premise
  "`conflict_margin: 0.0` silently uncovers the gap" is also false: `config.rs:183` already runs it through
  `positive()`, so 0.0 and negative are rejected today.

## 1. Fixed

- **I0 (major), decision A** — `crates/gta_sim/tests/traffic_causes.rs`.
  - One helper `dynamic_bound_on(row, approaches, clock)` replaces (c)'s `dynamic_stands_on_the_approach`:
    it asserts the TASK-032 `Dynamic` 30 s bound on the scene approach lanes (same band as before: 15 m
    before the lane start to its stop line, -1.7..3.25 m lateral; reuses `dynamic_longer_than`) and prints
    the rest with a TASK-036/TASK-037 pointer.
  - (a): approach = the scene lane. (c): same as before. rb: every lane ending at the box node
    (`LeftInBox.approaches`, `end_node == node`).
  - Module header and rb doc say which rows scope the bound and that TASK-037 restores it city-wide
    (the TASK-037 acceptance item was already added by the orchestrator, `maw/tasks/pending/TASK-037/task.md`
    "Added by TASK-038").
  - G1 oracle, the new graph gate, the lease, "left car cleared" asserts: untouched.
  - rb now prints `rb: Dynamic stands > 30 s off the scene approaches ...: [(98.77, (-9.43, 1.16, -81.73))]`
    (the grant holder on exit lane 249 with walkers at its nose) and passes.
  - **Flip-RED** (re-anchored gate): partition sabotaged to `true || on_approach(p)` (every stand counts as
    on the approaches): rb RED with `"AI cars on the scene approaches stood > 30 s in Dynamic:
    [(98.765625, Vec3(-9.426896, ...))]"` (`scratch/fix_flip_rb_scope.log`). Restored: GREEN.
- **I1 (major, docs)** — measured myself: the `traffic_graph` gate now prints free pairs per seed
  (review §4 "no gate pins the concurrency cost"): seed 1 **1323** (old 2340), seeds 2..8 1244, 1291, 1276,
  1314, 1164, 1261, 1376 (`scratch/fix_win_causes_graph.log`). Written into `docs/architecture/traffic.md`
  (right turns conflict with almost everything; ~2-2.6 m connectors, radius ~1.7 m; 2340 → 1323; opposite
  straights unaffected; TASK-036 item 4 owns the capacity) and into `maw/tasks/pending/TASK-036/task.md` item 4.
  The shortest right-turn connectors I saw are 1.82-2.0 m (the probe's worst-corner connectors), so I wrote
  "~2-2.6 m" rather than the review's single 2.6 m.
- **I2 (major, deferred as instructed)** — not fixed in code. Added to TASK-036 item 4: `connector_rects`
  centre ± half-width band, corners 0.87-0.91 m outside it, the demoted stuck holder path
  (`junction.rs:149-153`, kinematic), a request for a G1 fixture, and why `body_sweep` shapes are not a
  drop-in (reach 4 m onto the exit lane).
- **I4 (minor)** — `tests/traffic_graph.rs` header names what the oracle shares with the table (pose,
  extent) and what it does not cover (a holder with its nose past the stop line, residual lateral offset at
  connector entry via `effective_lateral`, touches thinner than its own sampling: corners move ~0.5 m between
  its 0.1 m samples). The residual-lateral case is also a TASK-036 item 4 note.
- **I5 (minor), recomputed** — the `BODY_SAMPLE_STEP` doc (`graph.rs:108-110`) claimed "a touch between two
  samples is thinner than the half conflict margin". That is false (corner step up to ~1 m, above). The doc
  now says the guarantee is the unmargined seeds 1..8 oracle, not the step. No config floor added (see
  Skipped). PCTX proposal appended (gates: measure corner displacement, not centre step).

## 2. Skipped

- **I5 config floor** ("reject `conflict_margin` below what `BODY_SAMPLE_STEP` needs", with a flip-RED config
  row). The orchestrator asked for it; I did not add it because no number is derivable: the geometric bound
  would reject the shipped 0.3, and the measurement shows the table touch-free down to 0.01 m margin. A
  floor like `>= 0.2` would be an undeclared tuning law that guards nothing (gates domain: "test numbers are
  derived, not intended"). Zero/negative margins are already rejected by `positive()`. What does guard the
  property is `cars_granted_together_never_touch` over seeds 1..8. If the orchestrator wants a finer oracle
  (0.03 m step), note it costs ~250 s per 8 seeds in the dev profile (`probe_margin_calibration` timing), too
  slow for CI as is.
- **I3 (optional)** — build cost 16 → ~54 ms, one-shot on `Loading → Playing`. Computing each pair once
  would halve it, but it touches production code with no functional gain in a hotfix. Left as is.
- **Nits** (`clear` comment, "+-" vs "±", IMPL_SUMMARY "+23 %"): cosmetic or another stage's artifact.
  The concurrency figure is now in traffic.md and TASK-036.

## 3. Test results

| Command | Result |
|---|---|
| `cargo test --locked -j 2 -p gta_sim --test traffic_causes --test traffic_graph -- --nocapture` (Windows) | causes 11 passed / 2 ignored (r1, TASK-037); graph 2 passed (`scratch/fix_win_causes_graph.log`) |
| `cargo test --locked -j 2 --no-fail-fast -p gta_sim -p citygen` (Windows) | **590 passed, 0 failed, 12 ignored**, 74 binaries (`scratch/fix_win_full_suite.log`) |
| `cargo clippy --locked -j 2 --workspace --all-targets -- -D warnings` (Windows) | clean (`scratch/fix_win_clippy.log`) |
| WSL Ubuntu 22.04, 1.95.0, CI env: `wsl -d Ubuntu-22.04 -- bash scratch/linux_test.sh --no-fail-fast -p gta_sim -p citygen` | **590 passed, 0 failed, 12 ignored**, 74 binaries (`scratch/fix_linux_full_suite.log`) |
| GitHub on the pushed branch (commit 663ba66) | all 5 workflows success; sim gates https://github.com/pockerhead/MAW-make-GTA/actions/runs/36286459995 (15m23s); clippy 36286460013, client 36286459996, citygen 36286459979, repo checks 36286459987 |

Probe (removed from `crates/`, copy in `scratch/zz_fix038_step.rs`): `probe_step`, `probe_fine_grown`,
`probe_margin_calibration`.

## Files changed (commit 663ba66, pushed to origin/bugfix/linux-traffic-gates)

- `crates/gta_sim/tests/traffic_causes.rs` — I0.
- `crates/gta_sim/tests/traffic_graph.rs` — I4 header, free-pair print (I1).
- `crates/gta_sim/src/traffic/graph.rs` — `BODY_SAMPLE_STEP` doc only (I5).
- `docs/architecture/traffic.md` — I1.
- `maw/tasks/pending/TASK-036/task.md` — I1, I2, I4(c).
- Task dir (not committed, left for the orchestrator's artifact commit; `scratch/` is gitignored): `FIX_SUMMARY.md`, `PCTX_PROPOSALS.md` (+1), `log.jsonl` (+2), `scratch/fix_*`, `scratch/zz_fix038_step.rs`.
- `maw/tasks/in_progress/TASK-038/metrics.md` was already modified before this stage (orchestrator); not mine.

children: 0 launched / 0 reported.
