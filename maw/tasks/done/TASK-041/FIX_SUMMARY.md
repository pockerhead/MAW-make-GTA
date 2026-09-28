# TASK-041 FIX_SUMMARY (fixer, claude opus, medium)

Fix commit: `b5a7f04` on `chore/release-v0.1.0` (on top of review commit `81edbc3`). Pushed.

## Preflight: the most dangerous prescription

Claim checked first: I2 "on the main thread, show the message box from the panic hook". Risk: `MessageBoxW` runs a
modal loop inside the hook, and a panic inside winit's callback could re-enter winit. Checked in the pinned source:
the realistic main-thread panic (`bevy_render-0.19.1/src/renderer/mod.rs:286`, "Unable to find a GPU!") fires in
`RenderCreation::create_render` -> `bevy_tasks::block_on` during plugin build (`settings.rs:259-296`, `lib.rs:517`),
before the event loop runs, so no winit re-entry there. Then I ran it for real (below): the dialog shows, the log has
the panic. A panic inside a running system happens on a task-pool thread (logged only; the executor re-raises with
`resume_unwind`, which does not call the hook again), so the main-thread-only dialog does not fire twice. Kept.

Diagnoses I1-I6 were all verified against the code and accepted (the orchestrator made them binding). Prescription
changes are listed per item.

## Fixed

- **I1 (fatal start-up errors silent, stale log)** `src/main.rs`: `static LOG_FILE: OnceLock<(PathBuf, Arc<File>)>`
  opened by `open_log_file()` as the first thing in `main` (release only), which truncates a stale log. `log_layer`
  reads it (one handle). `fatal(error)` = `eprintln!` + `ERROR <line>` lines into the log + `error_dialog`
  (`MessageBoxW`, `MB_OK | MB_ICONERROR`, first 12 lines plus the log path; Russian text with the "распакуйте архив
  целиком" hint). All 9 `eprintln!`/`AppExit::error()` blocks plus `compose_sim` and the preflight list now go through
  `fatal`. `error_dialog` is a no-op outside `cfg(all(windows, not(debug_assertions)))`. `windows-sys = "=0.52.0"`
  (`Win32_Foundation`, `Win32_UI_WindowsAndMessaging`) as a `cfg(windows)` dependency. `Cargo.lock` diff: one line,
  the `"windows-sys 0.52.0"` edge under `gta_like`; no new package (diffed against a copy taken before the build),
  and the release build compiled only `gta_like`. `tree_check.py` passes. The `if cfg!(debug_assertions) { |_| None }`
  selector is gone: `LOG_FILE` is unset in debug, so `log_layer` returns `None` and Bevy keeps its default layer.
- **I2 (panics invisible)** `install_panic_hook()` (release only, installed right after the log opens): chains the
  default hook (stderr), writes `thread '<name>' <PanicHookInfo>` (contains `panicked at`) to the log, and shows the
  dialog when the thread is `main`.
- **I3 (smoke lost the panic scan)** `tools/package_release.py`: the scan moved into a pure
  `log_problems(lines, console_lines, allows, expects, log)`. The game log keeps `BAD_LINE` (`ERROR|panicked`); the
  ANSI-stripped `*.console.log` is scanned with `PANIC_LINE` (`panicked`), `--allow` applies to both. Prescription
  changed: not the full `BAD_LINE` on the console, because the console is a tee of the same tracing events and would
  only duplicate every file finding (log.jsonl decision). 4 unit rows in `SmokeScan`.
- **I4 (ANSI via span fields)** `file.and_then(console)`: the plain file layer formats span fields first
  (`tracing-subscriber-0.3.23/src/fmt/fmt_layer.rs:878` stores the first `FormattedFields<N>`). Local log: 0 ESC bytes.
- **I5 (no log in a read-only folder)** `open_log_file` falls back to `temp_dir()/gta_like.log`; after `LogPlugin` is
  up, `warn!("the exe folder is not writable; log file: <path>")` when the path is not beside the exe.
- **I6 (silent --generate-notes)** `release.yml` publish: `f=docs/releases/$GITHUB_REF_NAME.md`, missing file ->
  `::error::` + exit 1, else `--notes-file "$f"`. `--generate-notes` removed: `publish` only runs on `v*` tag pushes,
  so there is no non-tag path left to keep it for. ADR-002 text updated (publish and console bullets).
- **Nits**: verify prints ", GUI subsystem" only when the zip has an `.exe`. Release notes: "сразу закрывается" ->
  "показывает окно с ошибкой", plus the DX12/Vulkan GPU requirement and "пришлите gta_like.log". `pe()` docstring
  says the synthetic rows prove the rule, not the offset (the offset is proven by real exes and CI `package`).

## Evidence (all local, Windows, release exe from this commit)

- Smoke GREEN on the unpacked folder (exe + assets junction), `--expect "AdapterInfo {" --expect "main menu ready"`:
  `scratch/smoke_fixed.out` ("0 ERROR/panic lines, 0 console panic lines", adapter RTX 4070 Ti, Vulkan).
- Fatal path, `assets/` renamed, exe started detached with no stdio (double-click equivalent), log pre-seeded with a
  `STALE main menu ready` line: process alive after 8 s (dialog blocking), stale line gone, log =
  `ERROR ...\assets\world/render.ron: Системе не удается найти указанный путь. (os error 3)` (strict UTF-8 decode
  re-checked; the `probe_fatal_dialog.out` mojibake is only the cp1251 console of that probe). Dialog screenshot:
  `scratch/dialog_assets_missing_crop.png` (title GTA-like, "Игра не запустилась:", the error, the unpack hint, the
  log path). Probe: `scratch/probe_fatal_dialog.py`.
- Main-thread panic, `WGPU_BACKEND=metal` (no adapter on Windows): log has `thread 'main' panicked at
  ...bevy_render-0.19.1\src\renderer\mod.rs:286:36: Unable to find a GPU! ...`; dialog screenshot
  `scratch/dialog_no_gpu_panic_crop.png`. Screenshots cropped to the dialog; the full-desktop captures were deleted.
- I5: `icacls /deny <user>:(WD)` on the exe folder, 12 s boot: no log beside the exe, `%TEMP%\gta_like.log` has
  `WARN gta_like: the exe folder is not writable; log file: C:\Users\user\AppData\Local\Temp\gta_like.log` and
  `main menu ready`; ACL restored (`scratch/probe_log_fallback.py/.out`).
- Every probe killed only its own PID; `tasklist` shows no `gta_like` afterwards.

## Flip-RED

- Console panic scan: `PANIC_LINE` replaced by a never-matching regex -> RED on exactly
  `test_console_only_panic_is_refused`; restored -> GREEN (`scratch/flip_console_panic.py/.out`). Perturbed input:
  the console scan rule.
- The fatal/panic/fallback paths are runtime behaviour gated by the probes above (log content + dialog), not by a
  unit gate: the dialog blocks, and the owner judges the first-launch look.

## Skipped

- Review "missing coverage" option of a `GTA_LIKE_NO_DIALOG` env guard: not added (not asked by the orchestrator, and
  the CI smoke still fails correctly on a fatal start: the log gets the `ERROR` line and no expects match, it just
  waits the full `--seconds`). Note for QA: a release `--features dev` build that fails at start now blocks on the
  dialog until killed.

## Test results

- `cargo clippy --locked --workspace --all-targets -j 2 -- -D warnings`: clean.
- `cargo clippy --locked --release -p gta_like --bin gta_like -j 2 -- -D warnings`: clean (lints the Windows
  release-only `error_dialog`, which CI's debug clippy does not compile).
- `cargo test --locked -j 2 -p gta_like --bin gta_like`: `test result: ok. 82 passed; 0 failed`.
- `python -m unittest tools/qa/test_package_release.py`: `Ran 10 tests ... OK`.
- `python tools/qa/tree_check.py`: `tree checks passed` (windows-sys 0.52.0 is an existing, non-critical duplicate).
- CI on `b5a7f04`, all 6 workflows `success`:
  - clippy https://github.com/pockerhead/MAW-make-GTA/actions/runs/36422779611
  - client gates https://github.com/pockerhead/MAW-make-GTA/actions/runs/36422779415
  - citygen gates https://github.com/pockerhead/MAW-make-GTA/actions/runs/36422779555
  - repo checks (runs the new `SmokeScan` rows) https://github.com/pockerhead/MAW-make-GTA/actions/runs/36422779465
  - sim gates (Linux) https://github.com/pockerhead/MAW-make-GTA/actions/runs/36422779862
  - release https://github.com/pockerhead/MAW-make-GTA/actions/runs/36422779610 : Windows `package` "... no dynamic
    imports, GUI subsystem"; Windows smoke (dx12) and both Linux smokes "0 ERROR/panic lines, 0 console panic lines".
    The `publish` job does not run on a branch push, so the I6 notes-file check first runs on the `v0.1.0` tag.

## For the owner / QA

- The first-launch look (no console on a real double-click, the error dialog's wording) is the owner's call; the
  cropped screenshots are in `scratch/`.
- After tagging: `gh release view v0.1.0 --json body,isPrerelease,assets` to confirm the body is the notes file.
