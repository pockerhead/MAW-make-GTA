# FIX_SUMMARY — TASK-002

## Fixed

- I1: verified in Tnua 0.32.0 that a delayed jump contender is discarded when feeding stops (`controller.rs:720-751`). Added a fixed-tick buffer in the character plugin and a late-tap regression test. The buffer stops when the jump starts, preserving a short tap's jump height.
- I2, fast compiles: added the requested dev optimization profiles, opt-in `fast` feature, and Windows `rust-lld.exe` linker setting. The local Cargo index lacks `bevy_dylib 0.19.1`, so the exact published crate is vendored to make the pinned feature resolvable. Its archive SHA-256 matches the crates.io checksum (`e3c03a12…8d3d13c0e`), and all copied files match the archive.
- I2, ledge assist: retained jump-assisted pull-up and exposed Tnua's cling distance and spring values in `locomotion.ron`; walking collision remains in place. Tnua casts to `float_height + cling_distance` (`bevy-tnua-0.32.0/src/builtins/walk.rs:516-520`); on reaching a ledge top it leaves the airborne state (`walk.rs:431-437`) and applies the float spring (`walk.rs:410-421,624-643`). The new terrain gates confirm walking is blocked by the box and climbs the stairs.
- I3: gave the player a presentation `Visibility` component before adding visual children.
- I4: disabled physics gizmos initially after registering their group.
- I5: moved ambient brightness and sun intensity/angles into strict `assets/world/render.ron` loaded by the client.
- I7: ran the direction unit test under an axis-sign sabotage and observed RED, then GREEN after restoration.
- I8: run tree checks from the repository root.
- Missing coverage: added a Sprint speed gate from the RON value, and renamed the short-tap test to match its actual assertion.

## Skipped

- I2's original walking step-up prescription: contradicted by the owner's later correction; arena boxes must block walking. The cited character code has no walking step-up implementation.
- I6: `SENSOR_INSET` is a fixed wall-clearance rule for the sensor. Ledge assist reach is now named separately in RON.
- I5's placeholder mesh colors: defer to the visual/content work in T3; they are not lighting values or `const`s and changing them now would add a second character visual config before it has an owner.
- Camera `unwrap`/shape allocation/yaw wrapping nits: inspected `src/camera/mod.rs`; the cast directions are unit basis vectors transformed by a unit quaternion, the sphere is a small per-frame object, and this T1 run has no demonstrated yaw precision loss. Left them unchanged.
- Config-test temporary directory nit: only leaks when the test deliberately panics on a broken strict-loader gate; changing cleanup would not affect the gate's verdict. Fixed-tick loop nit: the existing bounded loop already fails with `GATE BROKEN` if it stalls.
- BRP `-j 4` and `Popen` nit: the concurrency cap is required by this Windows task environment; process launch failure is outside the reviewed game behavior. The game is stopped by the context manager when launched.
- API-doc nit: `MoveIntent` conventions are specified by the approved plan and exercised by movement tests; no behavior defect was identified.

## Test results

- Before the fast profile change, an incremental `cargo build --bin gta_like -j 4 --offline` after touching `src/main.rs` passed in 34.51 s.
- After the profile change, the same one-file incremental command passed in 9.96 s; initial optimized client build passed in 20m 34s.
- `cargo test -p gta_sim -j 4 --offline`: 12 tests passed (1 unit, 2 config, 3 jump, 4 movement, 2 terrain); doc tests passed.
- `cargo test -p citygen -j 4 --offline`: passed.
- `python D:/test-gta-like/tools/qa/tree_check.py` from outside the repo: passed. Removing the new `cwd=REPO` caused RED (no manifest), then restoration passed.
- Flip-RED for new gates: removing the continued jump feed gave buffered rise -0.00035 m and failed; returning it passed. Mapping Sprint to run speed gave 4.20044 m against 6.8 m and failed; returning it passed. Removing the arena box made the walking gate fail at z=5.97; removing stairs made the stair gate fail at z=-14.10; both passed after restoration.
- `cargo run --bin gta_like --features fast -j 4 --offline`: built in 12m 25s and opened the GTA-like window on the NVIDIA Vulkan adapter. The process was stopped and no `gta_like` process remained.
- `cargo run --bin gta_like --features dev -j 4 --offline`: opened the GTA-like window and BRP port; stopped afterwards. No game process remains.
- `python tools/qa/scenarios/t1.py --out maw/tasks/in_progress/TASK-002/scratch/fixer_runtime_t1`: passed; movement Δz = -4.39160 m, yaw Δ = -0.418879 rad, valid screenshot of player and arena, FPS = 28.8083, shutdown passed. `target/qa/game.log` had no B0004 visibility warning.
- `cargo build -p gta_like --bin gta_like --features dev,debug -j 4 --offline`: passed in 18m 26s. Direct debug executable stayed alive for 10 s; no panic or B0004 warning in its log. It was stopped afterward.
- `cargo clippy -p gta_like --bin gta_like -j 4 --offline -- -D warnings`: passed. The same command with `--features dev,debug`: passed.
- `cargo clippy -p gta_sim --lib`, and separately `--test config`, `--test jump`, `--test movement`, `--test terrain`, each with `-j 4 --offline -- -D warnings`: all passed. `cargo clippy -p citygen --lib -j 4 --offline -- -D warnings`: passed.
- `python tools/qa/tree_check.py`: passed after the final feature builds. `git diff --check`: passed. Branch is `feature/t01-skeleton-character-camera-qa`; no game process remains.

## Owner checklist

Run `cargo run`; check the window, run/Shift sprint/Alt walk/Space jump, ramp and stairs, camera against the wall at z=14, Esc/left-click cursor capture, and the feel of movement and camera. Tune `assets/character/locomotion.ron` and `assets/camera/camera.ron` as needed. Confirm that walking is blocked by boxes and jumping onto a low ledge pulls up quickly and cleanly.

children: 0 launched / 0 reported.
