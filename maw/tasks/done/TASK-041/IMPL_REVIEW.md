# TASK-041 IMPL_REVIEW (code-reviewer, claude opus, medium)

Reviewed commit `d266ca1` (chore/release-v0.1.0) against `task.md` (small-fix, no plan) and `IMPL_SUMMARY.md`.
All 8 changed files were read in full. `python -m unittest tools/qa/test_package_release.py` -> 6 OK (re-run by me).
No cargo builds were run: every finding below comes from the code and the pinned sources.

## Disconfirmation

Counter-example tested: "the owner double-clicks the release exe from inside the zip (or with a broken `assets/`),
and the game exits with no window, no console and no log line, while a stale `gta_like.log` from an earlier run
still says `main menu ready`."
**It held.** `src/main.rs:218-224` loads `RenderConfig` and on error calls `eprintln!` + `AppExit::error()` BEFORE
`DefaultPlugins`/`LogPlugin` is added (line 236), so `log_layer` never runs: no file is created or truncated, and
stderr goes nowhere under the GUI subsystem. Every later `eprintln!` path (lines 249, 258, 265, 272, 279, 286, 302)
also skips the file, because `eprintln!` is not a tracing event. Second case from the same class: panics. In
`bevy_app-0.19.1/src/panic_handler.rs` native builds install no hook (only wasm / `error_panic_hook`), and
`bevy_log-0.19.1/src/lib.rs:297-307` installs one only with `feature = "trace"`. So the most likely first-launch crash
on a stranger's PC, `bevy_render-0.19.1/src/renderer/mod.rs:286`
`selected_adapter.expect("Unable to find a GPU! ...")`, is invisible too and never reaches `gta_like.log`.

Log triage: one `decision` entry (tee layer + PE check). I checked its refs; they are accurate. No `dead_end` entries.

## 1. Verdict

**NEEDS_WORK.** Items 2-3 and the CI plumbing are solid, but item 1 made every start-up failure silent (fatal
pre-flight errors and panics reach neither a console nor the log), and the smoke lost its panic scan.

## 2. Confirmed correct

- `src/main.rs:2`: `#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]`. Dev/test builds keep the
  console. `cargo test -p gta_like --bin gta_like` is a debug build, so it is unaffected.
- `src/main.rs:183-191`, `log_layer`: `LogPlugin::fmt_layer: fn(&mut App) -> Option<BoxedFmtLayer>` and the default
  stderr fallback match `bevy_log-0.19.1/src/lib.rs:249,331-336`. `Arc<File>` goes through
  `MakeWriter for Arc<W> where &W: Write`. That writer is unbuffered, and fmt writes each event with one `write_all`,
  so a killed process (the smoke uses `TerminateProcess`) or a normal exit loses nothing and no flush is needed.
  The file layer has `with_ansi(false)`. The local log files in `scratch/local_smoke/` contain no ESC bytes (I checked
  with grep).
- stderr in the GUI subsystem: when handles are not redirected, std treats the invalid handle as a sink, so the
  console tee is harmless. When they are redirected (brp.py, smoke), it works; the `smoke-*.console.log` files show it.
- `tools/package_release.py:63-82`, `pe_subsystem`: offset `e_lfanew + 24 + 68` is right for both PE32 and PE32+.
  A non-PE file returns `None`, which fails the check (`None != 2`), so it cannot pass vacuously. The real-exe
  cross-check (old exe read 3, new exe read 2, and CI `package` on the real zip) confirms the offset independently
  of the synthetic `pe()` helper.
- `tools/package_release.py:221-255`, smoke: it deletes a stale `DIR/gta_like.log` first (and raises GateBroken if
  that fails, for example when an old process holds the file), then reads only the file this run wrote. If the file
  is missing, that is a product failure. An early exit prints both tails. The expects cannot match an old log.
  The flip (`scratch/flip_smoke_log.out`) is RED as claimed.
- CRT regex (`package_release.py:38-39`) covers the three names. The unit row flips RED on exactly those three with
  the TASK-028 regex (`scratch/flip_crt_and_subsystem.out`). `NOT_CRT` guards the `api-ms-win-core-*` false positive.
- `release.yml` publish: sparse checkout of `docs/releases` plus `--notes-file`. `repo-checks.yml` runs the new
  unittest.
- Release notes: the content matches GDD §1 items 1-10 and the "вне прототипа" list. Controls match
  `src/input/mod.rs:130-165` (Shift/Alt/Space/LMB/RMB/R/1-4/F, Space = handbrake). "Guns in the park shooting range"
  matches README:269 and `combat/pickups.rs` (the bat also lies on the range). The limitations cover everything the
  task lists (README:193-194, TASK-039 squeeze, no licence).
- README "Запуск" and the ADR-002 bullet agree with the code.

## 3. Issues

### I1 (major): fatal start-up errors are silent and leave a stale log. `src/main.rs:193-306`
With no console, every `eprintln!` + `AppExit::error()` path is invisible. The `RenderConfig` failure (the realistic
"run from inside the zip" / missing `assets/` case) happens before `LogPlugin`, so no `gta_like.log` is written, and an
old one from a good run stays next to the exe and misleads. The release notes (line 28) point the user at
`gta_like.log`, and that is exactly where this error is missing.
**Minimal fix (no new crate):**
1. At the very top of `main` in release builds (`cfg!(not(debug_assertions))`), create the log file once:
   `static LOG_FILE: OnceLock<Arc<File>>` set from `File::create(current_exe().with_file_name("gta_like.log"))`.
   This truncates the stale log before anything can fail. `log_layer` (a `fn` pointer, so it cannot capture) reads
   `LOG_FILE.get()` instead of opening the file itself. There is one handle, so no two writers race on one file.
2. Add one helper, `fn fatal(lines: &[String]) -> AppExit`: `eprintln!` each line, `write_all` them to `LOG_FILE`
   when it is set, and on `cfg(all(windows, not(debug_assertions)))` show
   `MessageBoxW(null, text, "GTA-like", MB_OK | MB_ICONERROR)` with a hint ("распакуйте zip целиком; подробности в
   gta_like.log"). Replace the 9 duplicated `eprintln!`/`return AppExit::error()` blocks with `return fatal(..)`.
   That also shortens `main`, which is about 145 lines now.
3. MessageBoxW source, checked against the pinned deps: `windows-sys 0.52.0` is already in `Cargo.lock` through
   `winit 0.30.13`, with `Win32_Foundation` and `Win32_UI_WindowsAndMessaging` enabled (`winit-0.30.13/Cargo.toml:525-551`;
   `MessageBoxW` at `windows-sys-0.52.0/src/Windows/Win32/UI/WindowsAndMessaging/mod.rs:486`, `MB_ICONERROR = 16`,
   `MB_OK = 0`). Declaring `[target.'cfg(windows)'.dependencies] windows-sys = { version = "=0.52.0", features =
   ["Win32_Foundation", "Win32_UI_WindowsAndMessaging"] }` adds no crate and no new compile unit. The zero-dependency
   alternative is a 3-line `#[link(name = "user32")] unsafe extern "system" { fn MessageBoxW(..) }`; user32 is
   already linked by winit. Both need one `unsafe` call with a `// SAFETY:` line (null-terminated UTF-16 buffers
   that outlive the call). Prefer windows-sys, because its signature is generated rather than hand-typed.
   `tree_check.py` pins must accept the `=` pin; check that when adding it.

### I2 (major): panics reach neither the log nor the user. No panic hook is set anywhere (`grep set_hook src/` -> none)
See the disconfirmation: the "Unable to find a GPU!" `expect`, system panics (`bevy_ecs` executor prints
"Encountered a panic in system" with `eprintln!`), and worker-thread panics all go to stderr only.
**Fix:** in release, `std::panic::set_hook` chained to `take_hook()` that writes `info` (and a
`std::backtrace::Backtrace::capture()` if wanted) to `LOG_FILE`. On the main thread
(`std::thread::current().name() == Some("main")`), also call the I1 message box so the owner sees why the window
vanished. Worker panics caught by `bevy_tasks` (`task_pool.rs:197`) should only be logged, not shown as a box.

### I3 (major): regression, the smoke no longer catches panics. `tools/package_release.py:245-256`
Before this commit, `BAD_LINE` (`\bERROR\b|panicked`) scanned stdout and stderr combined. Now it scans only
`gta_like.log`, which never contains `panicked` (a panic message is not a tracing event; see I2). A non-fatal panic
used to turn the smoke RED and now passes GREEN: for example a panic in an `AsyncComputeTaskPool` task (city
generation), which `bevy_tasks` catches while the process stays alive, or a panic on an audio thread. It is only
written to `*.console.log`, and nothing scans that file.
**Fix:** apply the `BAD_LINE`/`--allow` scan to the console file lines too (the expects can stay file-only). After
I2, the file also carries panics; keep the console scan anyway, because it is the only channel for pre-subscriber
output. Flip it: inject `panicked at` into the console stream only and expect RED.

### I4 (minor): ANSI can leak into the file through span fields. `src/main.rs:186-190`
tracing-subscriber 0.3.23 `fmt_layer.rs:878` formats span fields once per `FormattedFields<N>` type, and the first
layer to see the span wins. `console.and_then(file)` runs the console layer first, and its ANSI is on by default
(`fmt_layer.rs:743`; `ansi` is a default feature and bevy_log keeps the defaults). Both layers use `DefaultFields`,
so the file layer reuses the colored span fields despite `with_ansi(false)`. The current release has almost no
spans with fields (Bevy system spans need `trace`), so the local logs are clean, but the `profile` feature or any
future `info_span!(.., field)` would put ESC bytes in the file. The smoke strips ANSI, so it would not notice.
**Fix:** swap the order (`file.and_then(console)`) so the plain formatting is stored first, or give the file layer
its own field-formatter type.

### I5 (minor): a read-only or locked folder silently loses the log. `src/main.rs:184-185`
`current_exe()?` and `File::create(..).ok()?` fall back to stderr-only without a trace. On Windows that means no
output anywhere, for example when the exe is unpacked under `Program Files` or into a read-only share. A second
concurrent instance truncates the first instance's log (std opens with `FILE_SHARE_WRITE`). This is rare in the
"unpack and double-click" flow. **Fix (cheap):** fall back to `std::env::temp_dir().join("gta_like.log")`, and emit
one `warn!` once the subscriber exists that names the path actually used. If the owner prefers, just document it in
the notes.

### I6 (minor): publish silently falls back to `--generate-notes`. `.github/workflows/release.yml:115`
If `docs/releases/v0.1.0.md` is missing on the tagged commit (the sparse path is wrong, or the file lives only on an
unmerged branch), the job stays green with auto notes, and acceptance item 3 fails silently. The `publish` job runs
for the first time on this tag. **Fix:** for non-prerelease tags, require the file (`[[ -f "$f" ]] || { echo
"missing $f"; exit 1; }`), or QA must check the release body after publish. At minimum, QA should run
`gh release view v0.1.0 --json body,isPrerelease,assets`.

## 4. Missing coverage

- Smoke: a console-only `panicked` line must turn RED (I3). No such case exists today.
- A fatal pre-flight path in a release exe: boot the unpacked exe with `assets/` renamed and assert that
  `gta_like.log` exists, is fresh (not the stale one), and names the missing file. Local only or CI Windows. The
  message box blocks, so the check needs an env or flag guard (for example, skip the box when stderr is redirected,
  or `GTA_LIKE_NO_DIALOG`), or the smoke must kill the process after N seconds and read the log.
- `pe()` in `test_package_release.py` writes the subsystem at the same `+24+68` offset the parser reads, so the unit
  row alone is partly tautological. Its independent proof is the real-exe read (3 -> 2) and CI `package`. Say so in
  the test docstring, or add a row that reads a checked-in 1 KB PE header from a real console exe.

## 5. Nits

- `package_release.py:204`: verify prints "GUI subsystem on Windows" for the Linux zip too. It is harmless but
  reads as a claim.
- Release notes line 28, "сразу закрывается": with no console, the user sees nothing at all. After I1 this becomes
  "shows an error window", so update the sentence together with the fix.
- Release notes: you could add "if the game does not start, send gta_like.log" once I1/I2 land, and the GPU
  requirement (DX12/Vulkan GPU), because the no-adapter case is the most likely stranger failure.

For the owner (not machine-gated): the absence of a console window on a real double-click, and the look of the
message box after I1, are first-launch visuals that only the owner's run can judge.
