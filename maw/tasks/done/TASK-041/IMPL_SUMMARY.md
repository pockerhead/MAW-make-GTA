# TASK-041 IMPL_SUMMARY (small-fix, items 1-3)

Status: items 1-3 done and pushed (`d266ca1` on chore/release-v0.1.0); branch CI 6/6 green including the
Windows release smoke; item 4 (tag, publish, boot of the downloaded zip) left to the orchestrator/QA by design.

For the owner (visible on first launch, not machine-gated): a real double-click of `gta_like.exe` shows no console
window. CI proves the PE subsystem and the boot, not what the desktop shows.

## 1. What was implemented

- `src/main.rs` (+40/-5)
  - `#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]`: release builds on Windows open no
    console; dev/debug builds (debug_assertions on) keep it.
  - `log_layer`: `LogPlugin::fmt_layer` for release builds (`cfg!(debug_assertions)` picks `|_| None` otherwise).
    It returns the default stderr fmt layer `.and_then` a second fmt layer (`with_ansi(false)`) writing to
    `gta_like.log` next to the exe (`current_exe().with_file_name`, `File::create` truncates each launch,
    `Arc<File>` = unbuffered, so a killed process keeps its lines). If the file cannot be created it returns
    `None` = Bevy's default stderr layer.
  - Bevy API verified in `bevy_log-0.19.1/src/lib.rs:218-267,349-364` (`fmt_layer: fn(&mut App) ->
    Option<BoxedFmtLayer>`, replaces the default `fmt::Layer::default().with_writer(std::io::stderr)`),
    `pub use tracing_subscriber` (lib.rs:51); tracing-subscriber 0.3.23 `Layer::and_then` (layer/mod.rs:1040),
    `MakeWriter for Arc<W> where &W: Write` (fmt/writer.rs:694).
  - stderr stays in the tee on purpose: QA scenarios (`tools/qa/brp.py`, release=True) redirect stdout/stderr
    and keep working under the GUI subsystem (Popen passes handles via STARTF_USESTDHANDLES).
- `tools/package_release.py` (+~50)
  - CRT regex: `vcruntime140(?:_1|_threads)?`, `msvcp140(?:_[0-9a-z_]+)?` (task item 2).
  - New `pe_subsystem` + `exe_problems`: `package` and `verify` refuse a Windows exe whose PE optional-header
    Subsystem is not 2 (GUI). This is the CI-side static gate for "no console window".
  - `smoke`: deletes a stale `DIR/gta_like.log`, sends the process console to `<log stem>.console.log`
    (matches the `smoke-*.log` upload glob), after the run copies `DIR/gta_like.log` to `--log` and scans only
    that file. No log file -> product failure "log file: the game wrote no ...". Early exit prints tails of both.
- `tools/qa/test_package_release.py` (new, 66 lines): every CRT name incl. the three new ones refused, static-exe
  names (`api-ms-win-core-*`, kernel32, ntdll) pass; synthetic PE with subsystem 2 passes, 3 refused, non-PE
  refused, Linux exe exempt.
- `.github/workflows/repo-checks.yml` (+4): runs the new unittest.
- `.github/workflows/release.yml` (+4/-1): `publish` checks out `docs/releases` (sparse) and uses
  `--notes-file docs/releases/$GITHUB_REF_NAME.md` when it exists, else `--generate-notes`. Smoke steps unchanged
  (the tool now reads the log file on both OSes).
- `docs/releases/v0.1.0.md` (new): Russian release notes: GDD §1 content, controls (from `src/input/mod.rs`),
  how to run (Windows unpack + SmartScreen + log file; Linux glibc/Vulkan/apt line), known limitations.
- `README.md` (2 lines, "Запуск"): the console sentence became stale with this change -> "окна консоли нет, лог
  пишется в gta_like.log рядом с exe".
- `docs/decisions/ADR-002-release-ci.md` ("Окно консоли" bullet): now describes the decision taken.

## 2. Deviations / not implemented

- Added the PE subsystem check to `package`/`verify` (not literally named in the task): it is the only CI-visible
  proof of acceptance item 1 before QA boots the downloaded zip. Cheap, flip-tested.
- Tag / release publication (item 4) not done by design (orchestrator note): the orchestrator tags after merge.
- Known gap, not fixed (scope): errors printed with `eprintln!` before/around app build (bad CLI args, missing
  `assets/`, config load failures in `main`) do not reach `gta_like.log`; with no console on Windows such an
  exit is silent. The release notes and README tell the user to unpack first (the realistic cause).

## 3. Test results

- `python -m unittest tools/qa/test_package_release.py -v` -> 6 tests OK.
- Flip-RED `scratch/flip_crt_and_subsystem.py` (output `scratch/flip_crt_and_subsystem.out`): baseline GREEN;
  TASK-028 CRT regex -> RED on exactly vcruntime140_threads, msvcp140_atomic_wait, msvcp140_codecvt_ids;
  rule expecting subsystem 3 -> RED; parser always-GUI -> RED; restored GREEN.
- Real exe parse: the pre-change `target/release/gta_like.exe` read subsystem 3 (console).
- `cargo check -p gta_like --bin gta_like -j 2` OK; `cargo clippy -p gta_like --bin gta_like -j 2 -- -D warnings` OK.
- `cargo build --release --locked -p gta_like --bin gta_like -j 2` OK (3m10s).
- `cargo test -p gta_like --bin gta_like -j 2` -> 82 passed. gta_sim/citygen not touched; run on Linux CI.
- Local smoke of the new exe, `scratch/local_smoke.py` (out: `scratch/local_smoke.out`, logs in
  `scratch/local_smoke/`): new exe PE subsystem = 2 (GUI; the pre-change exe was 3); `smoke` 30 s OK, both
  expects (`AdapterInfo {`, `main menu ready`) matched in `gta_like.log`, 0 ERROR lines; the console file has the
  same 8 lines (stderr tee works under the GUI subsystem when handles are redirected). The local exe links the
  CRT dynamically (no +crt-static locally), so `package` was not run locally; CI covers it. The script makes an
  `assets` junction to the repo; I removed it after the run (a recursive delete of scratch through a live junction
  would delete `assets/`). Rerunning the script recreates it.
- Flip-RED of the smoke log rule, `scratch/flip_smoke_log.py` (out: `scratch/flip_smoke_log.out`): GAME_LOG pointed at
  a name the game does not write -> exit 1 "smoke: log file: the game wrote no ...".
- Commit `d266ca1` pushed to `origin/chore/release-v0.1.0`. Branch CI, all 6 green on d266ca1:
  release 36413072831 (Windows: `package: ... no dynamic imports, GUI subsystem on Windows`, smoke OK on dx12 WARP
  reading `gta_like.log`, both expects; Linux: package OK, menu 30 s + seed 60 s smokes OK from `gta_like.log`),
  repo checks 36413072751 (`test_package_release`: Ran 6 tests OK), clippy 36413072674, client gates 36413072616,
  citygen gates 36413072642, sim gates 36413072731.
- No gta_like process left running.

## 4. How to verify manually

- Double-click `gta_like.exe` in an unpacked release folder: no console window, main menu appears,
  `gta_like.log` next to the exe contains `main menu ready`.
- `cargo run --features fast` still shows a console with logs.
