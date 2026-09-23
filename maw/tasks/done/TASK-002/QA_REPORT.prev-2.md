# QA_REPORT — TASK-002 (GDD T1)

QA: claude opus, effort medium. Tree: `98149aa` (code in `7337cc3`), branch `feature/t01-skeleton-character-camera-qa`.
`QA_REPORT.prev-1.md` (the implementer's out-of-role checklist) was ignored; this report replaces it.

## 0. Disconfirmation

The counter-example I went after first: **the ledge-assist knobs the fixer added to `locomotion.ron` do not actually govern the mantle reach.** Reading Tnua 0.32 gave the reason to suspect it. A body that is airborne regrounds only when `proximity <= float_height` (`bevy-tnua-0.32.0/src/builtins/walk.rs:431-437`), and `cling_distance` plays no part in that check. It only lengthens the sensor cast (`walk.rs:516-520`). `spring_strength` and `spring_dampening` in the RON are Tnua's own defaults (`walk.rs:160-164`: 1.0 / 400 / 1.2), so the fix made the behaviour the owner saw nameable but did not change it.

To test it I ran a headless sweep through the production `compose_sim`. The ledge is 4×h×20 m, the player runs up in −Z and jumps at 0.4/0.8/1.2/1.8 m from the face, and I varied the RON values. Probe: `scratch/qa/qa_probe_ledge.rs`.

| RON variant | 0.8 m | 1.0 m | 1.2 m | 1.4 m | 1.6 m | 1.8–2.0 m |
|---|---|---|---|---|---|---|
| shipped (cling 1.0, spring 400/1.2) | TOP | TOP | TOP (run-up ≥ 1.2 m) | TOP (run-up ≥ 1.2 m) | fail | fail |
| cling 0.3 | TOP | TOP | TOP | TOP (1.8 m only) | fail | fail |
| cling 2.0 | TOP | TOP | TOP | TOP | fail (one case hangs on the edge at y 2.11) | fail |
| cling 0.0 | TOP | TOP | TOP | TOP | TOP (1.8 m only) | fail |
| jump_height 0.7 | TOP | TOP (≥ 1.2 m) | TOP (1.8 m only) | fail | fail | fail |

**The counter-example held.** `ledge_assist_cling_distance` from 0.3 to 2.0 gives the same reach (1.4 m). `jump_height` is what moves it. So the reach comes from `jump_height`, the capsule and sensor geometry, and the code const `SENSOR_INSET`. The three `ledge_assist_*` values do not set it, and no gate pins it. See bug B1.

## 1. Environment

There is no docker-compose and no dev server. I used cargo directly in the checkout, plus the real windowed build driven over BRP. Every cargo command ran with `--offline -j 4`, because TLS to crates.io fails here (same as for the implementer and fixer). The base commit `34f3bab` has no Rust code, so there was no earlier test suite to compare against.

```
cargo build -j 4 --offline
cargo clippy -j 4 --offline -- -D warnings
cargo clippy --workspace --all-targets -j 4 --offline -- -D warnings
cargo clippy --workspace --all-targets --features dev,debug -j 4 --offline -- -D warnings
cargo test -p gta_sim -j 4 --offline
cargo test -p citygen -j 4 --offline
python tools/qa/tree_check.py
cargo tree --offline -p gta_sim -e normal -i bevy_render          # and -e normal,dev
CARGO_NET_OFFLINE=true python tools/qa/scenarios/t1.py --out maw/tasks/in_progress/TASK-002/scratch/qa/runtime_t1
cargo build --release --features dev -j 4 --offline               # 14m37s cold
python maw/tasks/in_progress/TASK-002/scratch/qa/release_fps.py   # release exe + BRP diagnostics + screenshot + shutdown
```

QA probes are not gates. To rerun one, copy `scratch/qa/qa_probe_ledge.rs` into `crates/gta_sim/tests/`, run `cargo test -p gta_sim --offline --test qa_probe_ledge -- --nocapture --test-threads=1`, then delete it. I removed it from the tree after my runs.
Services started: none. The game processes I launched (dev and release) were shut down over BRP (exit 0). `tasklist` shows no `gta_like` left.

## 2. Test results

**Build/lint** (my own runs): `cargo build` PASS. Clippy PASS for all three shapes: root only, `--workspace --all-targets`, and `--workspace --all-targets --features dev,debug`.

**Existing suite** (`cargo test -p gta_sim`): 12/12 PASS.
- unit `camera_relative_axes`
- config ×2
- jump ×3: `jump_apex_matches_jump_height`, `tapped_jump_fires`, `late_tap_is_buffered_until_landing`
- movement ×4: yaw 0/90/180, sprint
- terrain ×2: box blocks walking, stairs climbed

`cargo test -p citygen`: 0 tests, PASS. No earlier suite existed, so there are no new failures.

**Tree gate:** `tree_check.py` exit 0. I also ran `cargo tree -p gta_sim -i bevy_render` myself with `-e normal` and with `-e normal,dev`: both print nothing ("nothing to print"). `cargo tree --features fast -i bevy_dylib` resolves to `vendor/bevy_dylib-0.19.1`. Its `lib.rs` is the stock `use bevy_internal;` shim.

**Flip-RED (mine, commit-first, restore checked by sha256 `6f85a8d7…6382`):**
- Removing `|| buffer.remaining > 0.0` in `drive_characters` turns `late_tap_is_buffered_until_landing` RED (`buffered jump rise=-0.00035`). Restored: GREEN.
- Removing the "stop the buffer once `ActionStarted`" clear (`buffer.remaining = 0.0`) leaves **all 12 tests GREEN**. Meanwhile the rise from a tap on the ground goes from 0.185 m to 0.728 m (probe `qa_tap_rise`). See B2.

**Runtime, dev build** (`tools/qa/scenarios/t1.py`): PASS.
- W for 1000 ms moved the player Δ = (0.0, 0.00003, −4.392) m.
- Mouse +200 moved yaw by −0.418879 rad (−24°, sign law OK).
- The PNG screenshot was written, FPS was 28.9, and shutdown passed. Summary: `scratch/qa/runtime_t1/summary.json`.
- Screenshot `scratch/qa/runtime_t1/t1.png`, which I looked at: the blue capsule stands on a grey floor with a shadow, the ramp is on the left, the stairs on the right, and the view is turned about 24° right after the mouse move. The frame is not black.

**Runtime, release build** (`scratch/qa/release_fps.py`, 6 s warm-up, 10 samples 1 s apart): frame time averaged 33.33 ms, with current samples from 31.39 to 35.32 ms. FPS read 30.0–30.1 on every sample (`scratch/qa/release_diagnostics.json`).
- Dev and release give the same 30 FPS. That points to a presentation cap, not CPU cost. The likely cause is that the harness launches the window unfocused and the background frame limiter kicks in. I did not verify the cause. Owner check below.
- Screenshot `scratch/qa/release.png`, which I looked at: spawn view at yaw 0, capsule centred slightly left (shoulder offset), ramp at x −10 on the left, stairs at x +10 on the right, a clean frame.

**Fast compiles, spot check:** after touching `src/main.rs`, `cargo build` (dev) took 7.8 s. That is consistent with the FIX_SUMMARY figure of 9.96 s. I did not re-run `--features fast` itself (a 12-minute dynamic build); that relies on the fixer's evidence.

**Ledge assist, feel measurement** (probe `qa_ledge_trace`, shipped RON, h = 1.4 m, jump at 1.2 m, jump released after 15 ticks; the trace is the same with the jump held):
- The capsule hits the edge at t = 70 with y = 1.89, i.e. its bottom is 0.26 m below the ledge top. It stays pressed against the edge for about 0.3 s, rising at about 1 m/s.
- It goes over at t ≈ 88. It then keeps rising slowly for about 1 s, from 0.26 m low (y 2.19) to 2.43 at t = 148, still short of the standing height 2.45. During that time the visual feet (centre − float_height) sit up to 0.26 m inside the ledge top.
- A 1.2 m ledge takes about 0.75 s to settle.
- A 1.0 m box is not ledge assist at all: the 1.0 m jump clears it (capsule bottom reaches about 1.36 m).

## 3. Acceptance criteria

| Criterion | Test performed | Result |
|---|---|---|
| yaw 0, 64 ticks → −Z within [0.8, 1.0]·v_run, \|x\| < 0.1 | `forward_yaw_0_moves_neg_z` via `compose_sim` | PASS |
| yaw 90° → −X | `forward_yaw_90_moves_neg_x` | PASS |
| yaw 180° → +Z | `forward_yaw_180_moves_pos_z` | PASS |
| Jump apex ±10% of `jump_height` | `jump_apex_matches_jump_height` | PASS |
| Unknown RON field → error with file and field names | `unknown_field_names_file_and_field` | PASS |
| `gta_sim` without `bevy_render` | my `cargo tree` (normal and normal,dev) + `tree_check.py` | PASS |
| Runtime QA: `t1.py` via `brp.py` (launch, W, yaw, screenshot, FPS, shutdown) | ran it, read the PNG | PASS |
| Owner checklist in QA_REPORT | section 6 | PASS (recorded) |
| Tuning values in GDD §12 data files, not in `const` | read the diff: locomotion/camera/render in RON with `deny_unknown_fields`. `SENSOR_INSET` is still a `const` and was declared a law, but it feeds mantle reach (B1). Placeholder mesh colours are literals, deferred to T3 | PASS with note |
| `cargo build`, clippy `-D warnings`, `cargo test -p gta_sim`/`-p citygen` green | my runs | PASS |
| Existing tests pass | no suite on base; 12/12 now | PASS |
| Owner: `[profile.dev]` opt 1 / `package."*"` opt 3 | `Cargo.toml` | PASS |
| Owner: `fast = ["bevy/dynamic_linking"]`, not default | `Cargo.toml`, `cargo tree --features fast` | PASS |
| Owner: `.cargo/config.toml` rust-lld | file present; every build used it | PASS |
| Owner: rebuild-time evidence + `fast`/`dev` start in FIX_SUMMARY | FIX_SUMMARY has 34.51 s → 9.96 s and both starts. I reproduced 7.8 s and the `dev` start, not the `fast` start | PASS (partly re-verified) |
| Ledge: mechanism named with file:line | walk.rs:516-520, 431-437, 410-421, 624-643 match the source. But the text implies cling_distance governs the pull-up, and it does not (line 433 uses `float_height`) | PASS (lines) / text misleading |
| Ledge: pull-up quick and clean, reach governed by named values in `locomotion.ron` | my sweep + trace (section 0 and section 2) | **FAIL**: the named `ledge_assist_*` values do not govern reach (B1). The pull-up includes about 1 s of slow float rise after hooking a 1.2–1.4 m edge (feel, owner to judge; B3) |
| Ledge: walking does not step onto arena boxes; stairs climbed | `walking_into_arena_box_stays_blocked`, `walking_climbs_stairs` | PASS (gate checks end position only; smoothness goes to the owner) |
| Ledge: jump and movement gates stay green | suite | PASS |

## 4. Bugs found

**B1 — major — ledge-assist reach is not governed by the named RON values.**
- Repro: `scratch/qa/qa_probe_ledge.rs::qa_ledge_reach_sweep`.
- Expected (TASK_FINAL, owner decision): the reach of the pull-up is set by named values in `locomotion.ron`, "not by an accident of geometry".
- Actual: `ledge_assist_cling_distance` 0.3 / 1.0 / 2.0 gives the same maximum mantle height, 1.4 m. The spring values are Tnua defaults and only change how the settle looks (spring 100 makes it sag and never settle). The reach comes from `jump_height`: 0.7 gives 1.2 m, 1.0 gives 1.4 m. It also comes from the capsule/sensor geometry, including the code const `SENSOR_INSET`, which I did not probe separately.
- No headless gate covers reach. A later change to `jump_height` or the capsule silently moves which ledges can be climbed.
- Suggested direction, to be recomputed: make the reach an explicit sim value, or state honestly that it derives from `jump_height` and rename or remove the knobs that don't act. Add a gate: a ledge at the reach height is mantled within N ticks, a ledge above it is not, with a flip-RED by changing the value.

**B2 — minor — the "tap = short hop" property of the new jump buffer has no gate.**
- Repro: in `crates/gta_sim/src/character/mod.rs`, replace `buffer.remaining = 0.0;` (the `ActionStarted` branch) with a no-op and run `cargo test -p gta_sim`.
- Expected: some gate goes RED, since FIX_SUMMARY claims "the buffer stops when the jump starts, preserving a short tap's jump height".
- Actual: all GREEN. A tap from the ground goes from 0.185 m to 0.728 m. `tapped_jump_fires` only asserts rise > 0.15. The `< 0.5` bound in `late_tap_is_buffered_until_landing` does not trip, because most of the buffer is already spent in the air.
- The owner would see this on the first jump, which is why it is rated minor. The fix is cheap: an upper bound in `tapped_jump_fires`.

**B3 — minor / owner feel — the mantle ends in a slow float rise.** After hooking a 1.2–1.4 m ledge, the body rides up to 0.26 m low and takes about 0.75–1.0 s to reach standing height. During that time the visual feet are inside the ledge. This may be the "slow crawl" the owner said they didn't want. The owner has to judge it. I measured it; I did not gate it.

**B4 — info — FPS locked at 30.0 in both dev and release BRP runs** (33.3 ms average, 31.4–35.3 ms). Same value regardless of optimisation level, so most likely a presentation or background-window cap in this environment. Not attributed to the game. Owner checks in a focused window.

**B5 — nit — `tools/qa/__pycache__/` is not in `.gitignore`.** Every `brp.py` run leaves an untracked directory. I deleted the one my run created.

## 5. Verdict

**NEEDS_FIXES.**
- The core T1 slice is solid and independently confirmed: build, clippy, 12 headless gates, the tree boundary, the BRP scenario, and screenshots I looked at. The review fixes I1, I3, I4, I5, I7 and I8 are in the code. The jump buffer does what it should for a late tap, and my own flip-RED proves its gate.
- One criterion from the owner's ledge finding fails as stated: the reach is not governed by the named `locomotion.ron` values (B1), and nothing gates it.
- B2 is a small gate hole in the fixer's own claim.
- B3 and B4 go to the owner.

## 6. Owner checklist (subjective, not gated)

1. `cargo run` (or `cargo run --features fast` for iteration). A "GTA-like" window opens and the capsule stands on the floor.
2. Controls: W/A/S/D run relative to the camera, Shift sprints (6.8 m/s), Alt walks (1.8 m/s), Space jumps about 1 m. A short tap on Space should give a small hop (about 0.2 m).
3. Ramp 30° at x = −10 and stairs at x = +10: walk up both smoothly with no stutter on the 0.2 m steps.
4. Boxes at (10..16, z = 10): walking into the 1 m box is blocked. Jumping onto the 1 m box is a plain jump. Ledges of 1.2–1.4 m are pulled up to; judge whether the pull-up is quick or crawls (B3). The 2 m box is out of reach.
5. Camera: mouse right turns the view right, mouse down lowers the view. Turn the camera with your back to the wall at z = 14: the camera must not go through the wall and should ease back out when released.
6. Esc frees the cursor, left click captures it again.
7. FPS in a focused window: check that it is not stuck at 30 (B4).
8. Tune feel in `assets/character/locomotion.ron`, `assets/camera/camera.ron` and `assets/world/render.ron`.

children: 0 launched / 0 reported.
