# PLAN_V2 — TASK-028: release CI for Linux and Windows

Reviewer: plan-reviewer-1. Base: `PLAN.md` (the structure holds up; fixes below). Evidence: `scratch/crt_probe.txt`
(new), `scratch/dylib_scan_probe.txt` (planner's, re-read).

## 0. Disconfirmation (done first)

Counter-example tested: "a `v*` tag push does NOT start `release.yml`, because the same `push` event also carries
`branches: ['**']` and a `paths` filter that a tag push does not satisfy". If it held, no release would ever be
created. Checked against GitHub workflow syntax docs (fetched 2026-09-27): "Path filters are not evaluated for pushes
of tags" and "If you define both branches/branches-ignore and paths/paths-ignore, the workflow will only run when both
filters are satisfied" (applies to branch pushes). Tags and branches are both defined, so a tag push runs, paths ignored.
**Did not hold.** The trigger design stands. The existing five workflows define only `branches-ignore`, so they do not
run on tags (also confirmed) — `release-gates` is needed.

## 1. Review notes (issues in PLAN.md, with evidence)

1. **MAJOR — the Windows zip depends on the Visual C++ runtime, and nothing in the plan can see it.** Byte scan of the
   local `target/release/gta_like.exe` (`scratch/crt_probe.txt`): imports `VCRUNTIME140.dll` and six
   `api-ms-win-crt-*.dll`. Rust `*-windows-msvc` links the CRT dynamically by default. A clean Windows install without
   the VC++ 2015-2022 redistributable fails before `main` with no console output. QA's host and the GitHub runner both
   have the redist, so every planned check is green while users fail. This is the same silent class as the
   `bevy_dylib` leak the plan already guards. Fix: build the Windows release with `+crt-static` (CI env only) and make
   `verify` reject CRT imports (§3 Step 2/3). Prior art: ripgrep ships msvc builds with `+crt-static`; msvc allows
   proc-macro dylibs under crt-static, so build scripts and derives keep working.
2. **Windows smoke rejected on a false premise.** Plan: "dx12 WARP in a service session is unproven". Bevy's own
   `example-run.yml` runs examples on `windows-latest` with `WGPU_BACKEND=dx12` every day. The menu smoke on the
   Windows runner is cheap (~40 s after the build) and checks the real Windows zip in CI, not only on QA's machine.
   Revised: attempt it as a gate in the first branch run; if it fails for an environment reason (no adapter, a
   `wgpu_hal` ERROR we cannot allow narrowly), remove it and log a `dead_end`. QA's local Windows run stays (spec AC).
3. **`linux-deps: ${{ matrix.platform == 'linux' }}` is a boolean passed to a composite that compares
   `inputs.linux-deps == 'true'`.** Composite inputs are documented as strings, so it probably works, but this is the
   one line where a silent miss means "Linux build without alsa/udev headers", which then fails loudly in the link.
   Also the smoke step's `apt-get install` has no `apt-get update` of its own and relies on the composite having run
   it. Put an explicit string in the matrix (`deps: "true"` / `"false"`) and run `sudo apt-get update` in the smoke
   step too. Cheap; removes both couplings.
4. **`publish` / `release-gates` condition is ref-only.** `workflow_dispatch` can be started on a tag ref; then
   `github.ref` starts with `refs/tags/v` and the plan would publish from a dispatch. Spec: "on workflow_dispatch,
   artifacts are uploaded". Condition becomes
   `github.event_name == 'push' && startsWith(github.ref, 'refs/tags/v')`.
5. **Linux-side checks were claimed covered without a flip.** Plan Step 6: "the Linux-specific parts (exec bit,
   `libbevy_dylib.so` spelling) are covered by the regex/`external_attr` checks in F1/F2's code paths". A code path
   that has never gone RED is not a gate (gates domain, flip-RED). Both are flippable locally on Windows with synthetic
   inputs (F1b, F2d below), no Linux run needed.
6. **`smoke-logs` artifact name is not per-OS.** With a Windows smoke, two failing legs would upload the same name;
   upload-artifact v7 `overwrite: false` fails the second. Name it `smoke-logs-<platform>`. Also `--log` must be
   resolved to an absolute path before the child starts, because the child's cwd is a temp dir.
7. **`release-gates` cache is always cold.** rust-cache keys on the job id; `release-gates` runs only on tags, and tag
   runs restore only default-branch and own-ref caches, so no warm key ever exists. Cost ~10-15 min per release.
   Accepted (rare), but written into R3 instead of implied away.
8. Verified as correct (no change): composite action contents and `shell: bash` on Windows; `.cargo/config.toml` has
   only the Windows `rust-lld.exe` section; `fast = ["bevy/dynamic_linking"]` opt-in; bevy_asset 0.19.1 `get_base_path`
   order `BEVY_ASSET_ROOT` → `CARGO_MANIFEST_DIR` → `current_exe().parent()` (`src/io/file/mod.rs:19-29`);
   `ConfigRoot(... .join("assets"))` at `src/main.rs:194`; preflight `missing_files` at `:155` exits before `run`;
   `insert_state(GameState::MainMenu)` only without `--seed`/`--bench-scene`; `spawn_main_menu` at
   `src/menu/screens.rs:39`, registered at `src/menu/mod.rs:52`, `prelude::*` imported; bevy_render 0.19.1 panics on no
   adapter then `info!("{:?}", adapter_info)`; bevy_audio "No audio device found." is `warn!`; `fetch_assets.py`
   `MANIFEST` `:28`, `load_manifest` `:259`, `sha256_file` `:271`, `check` `:338`, license_file-in-files rule `:228`;
   26 tracked files under `assets/`; upload-artifact v7 `archive` input and download-artifact v8 `skip-decompress`
   input exist in their `action.yml`; `windows-2025` and `ubuntu-24.04` are current labels; `*.log` is gitignored,
   so smoke logs at the repo root are not tracked.
9. Minor: the Windows zip also needs no `dxcompiler.dll` — without `statically-linked-dxc` bevy_render 0.19.1 uses
   `dxcompiler.dll` only if it exists in cwd, else FXC (`src/settings.rs:113-125`). Nothing to ship; noted so nobody
   adds a DLL to the allowlist.

## 2. Updated understanding (deltas to PLAN.md §1 only; the rest of PLAN.md §1 is verified and carries over)

- Windows release exe today: static Rust std (no `std-*.dll`), dynamic MSVC CRT (`VCRUNTIME140.dll`,
  `api-ms-win-crt-*`). UCRT (`api-ms-win-crt-*`) ships with Windows 10+, `VCRUNTIME140.dll` does not.
- `CARGO_TARGET_X86_64_PC_WINDOWS_MSVC_RUSTFLAGS` applies to all crates when cargo is run without `--target`
  (host = target), including build scripts and proc-macros; msvc supports crt-static dylibs, so that is fine.
- The five test workflows do not run on tag pushes; `release.yml` does, and on branch pushes only through `paths`.
- Bevy's upstream CI: Linux examples under `xvfb-run` with `mesa-vulkan-drivers` (plus `ppa:kisak/turtle` for newer
  Mesa), Windows examples with `WGPU_BACKEND=dx12` on `windows-latest`.

## 3. Revised approach

Same shape as PLAN.md: one workflow `release.yml` (jobs `release` matrix, `release-gates`, `publish`), one tool
`tools/package_release.py` (package / verify / smoke), one `info!("main menu ready")` marker, ADR-002, README.
Composite setup action reused unchanged.

Decision table: PLAN.md §2 table carries over, with these rows replaced/added:

| Topic | Decision | Rejected alternative |
|---|---|---|
| Windows CRT | Windows matrix entry sets `CARGO_TARGET_X86_64_PC_WINDOWS_MSVC_RUSTFLAGS: "-C target-feature=+crt-static"`; `verify` fails on any `vcruntime140*.dll` / `msvcp140*.dll` / `api-ms-win-crt-*.dll` name in the Windows exe. ADR says the zip exe differs from `cargo run --release` locally only in CRT linkage. | dynamic CRT + README "install VC++ redist" (users hit a silent no-start); `rustflags` in `.cargo/config.toml` (changes local dev and `fast` builds). |
| Windows boot | menu smoke on the runner (`WGPU_BACKEND=dx12`, 30 s, expect `AdapterInfo {` + `main menu ready`), gate if the first run is green; environment failure → remove + `dead_end`. QA still runs the downloaded zip locally (spec AC). | no CI smoke (premise refuted by Bevy CI). |
| Publish condition | `release-gates` and `publish`: `if: github.event_name == 'push' && startsWith(github.ref, 'refs/tags/v')` | ref-only (dispatch on a tag would publish). |
| Linux deps input | matrix carries `deps: "true"`/`"false"` strings; smoke step does its own `apt-get update` | boolean expression into a string-compared input. |

## 4. Revised steps

### Step 1 — `src/menu/screens.rs`: menu marker
First line of `spawn_main_menu` body (`:39`): `info!("main menu ready");`. Nothing else.
→ check: `cargo clippy --workspace --all-targets -- -D warnings` clean; `cargo test -p gta_like --bin gta_like` green
(menu gates untouched); `cargo run --release` prints the line when the menu appears.

### Step 2 — `tools/package_release.py` (new, target < 300 lines)
As PLAN.md Step 2, with these changes:
- `DYLIB = re.compile(rb"(?:lib)?(?:bevy_dylib|std-[0-9a-f]{16})\.(?:dll|so)")` (unchanged) and new
  `CRT = re.compile(rb"(?i)(?:vcruntime140(?:_1)?|msvcp140(?:_[0-9])?|api-ms-win-crt-[a-z-]+-l1-1-0)\.dll")`.
  `dynamic_imports(exe_bytes, windows: bool)` returns the sorted DYLIB matches, plus CRT matches when `windows`
  (decided by the `.exe` suffix of the entry/file).
- `package` step 2 and `verify` step 5 fail on any `dynamic_imports` hit with a message naming the class:
  "exe links Rust/Bevy dynamically: … (built with `fast`?)" vs "exe links the MSVC CRT dynamically: … (built
  without +crt-static)". Different messages so each flip proves its own branch.
- `smoke`: resolve `--log` to an absolute path before spawning; child cwd `tempfile.mkdtemp()`; env minus
  `CARGO_MANIFEST_DIR`/`BEVY_ASSET_ROOT`; pass `--settings-id gta_like_smoke` by default ahead of user game args so no
  run (CI or QA) touches the owner's settings file; everything else as PLAN.md (poll 0.5 s, early exit = RED with
  last 40 lines, kill at deadline, strip ANSI, fail on `\bERROR\b` / `panicked` not matched by `--allow`, fail on a
  missing `--expect`).
- Every failure message starts with the subcommand and the check name; a missing input (no zip, no exe, no git)
  says "GATE BROKEN: …" so plumbing failures are distinguishable from product failures (gates domain).
Gate classes, stated in the tool's summary output: liveness = alive at N s; state = `AdapterInfo {` and
`main menu ready`; correctness = zero ERROR/panic lines, zip set == allowlist, sha256 match, no dynamic Rust/CRT
imports. Not claimed: "the menu looks right" (QA/owner).

### Step 3 — `.github/workflows/release.yml` (new)
PLAN.md Step 3 YAML with these edits:
```yaml
      matrix:
        include:
          - { os: ubuntu-24.04, platform: linux, exe: gta_like, deps: "true", rustflags: "" }
          - { os: windows-2025, platform: windows, exe: gta_like.exe, deps: "false", rustflags: "-C target-feature=+crt-static" }
    ...
      - uses: ./.github/actions/setup
        with:
          linux-deps: ${{ matrix.deps }}
          assets: "true"
      ...
      - name: build (release, no features)
        shell: bash
        env:
          CARGO_TARGET_X86_64_PC_WINDOWS_MSVC_RUSTFLAGS: ${{ matrix.rustflags }}
        run: cargo build --release --locked -p gta_like --bin gta_like
      ...
      - name: boot smoke (Linux, unpacked zip, xvfb + lavapipe)
        if: matrix.platform == 'linux'
        # as PLAN.md, but first line: sudo apt-get update
      - name: boot smoke (Windows, unpacked zip, dx12)
        if: matrix.platform == 'windows'
        shell: bash
        env:
          WGPU_BACKEND: dx12
        run: |
          mkdir -p "$RUNNER_TEMP/unpacked"
          python -m zipfile -e "dist/$NAME.zip" "$RUNNER_TEMP/unpacked"
          python tools/package_release.py smoke "$RUNNER_TEMP/unpacked/$NAME" --seconds 30 --log smoke-menu.log --expect "AdapterInfo {" --expect "main menu ready"
      - if: failure()
        uses: actions/upload-artifact@v7
        with: { name: "smoke-logs-${{ matrix.platform }}", path: "smoke-*.log", if-no-files-found: ignore }
  release-gates:
    if: github.event_name == 'push' && startsWith(github.ref, 'refs/tags/v')
    ...
  publish:
    if: github.event_name == 'push' && startsWith(github.ref, 'refs/tags/v')
    ...
```
An empty `CARGO_TARGET_..._RUSTFLAGS` on Linux is harmless (that variable only affects the msvc target). If cargo
treats an empty value oddly, move the env to a Windows-only step instead; do not put it in `RUSTFLAGS` (that would
also change the Linux build). Everything else (paths filter list, concurrency, permissions, publish via
`gh release create --verify-tag --generate-notes`, `test ... -eq 2`, env-only ref names in `run:`) is as PLAN.md.
→ check: first branch push starts `release` for both OSes; both smokes print their markers; the Windows `package`
summary prints "no dynamic imports"; two artifacts `gta-like-sha-<8>-{linux,windows}-x86_64.zip`; rust-cache keys
`v0-rust-release-Linux…` / `v0-rust-release-Windows…`. If the Windows smoke is RED for an environment reason, remove
the step, log `dead_end` with the log excerpt, keep QA-local only.

### Step 4 — `docs/decisions/ADR-002-release-ci.md` (new, Russian like ADR-001)
As PLAN.md Step 4, plus: CRT decision (+crt-static in CI only, why, how verified), the Windows CI smoke status
(kept or dead_end with reason), publish only on tag push, and the `release-gates` cold-cache cost.

### Step 5 — `README.md`
As PLAN.md Step 5 ("Запуск": releases link, zip per OS, keep `assets/` next to the exe, Linux needs glibc >= 2.39,
`libasound2`, `libudev1`, Vulkan driver, X11/Wayland; replace the stale paragraph at the end of "Запуск" about taking
`assets/` from the last release; "Что проверяет CI": `release` row, "five on every push plus release"). Windows: no
extra runtime needed (static CRT). Per resolved Q4: "лицензия на код игры пока не выбрана" next to the third-party
attribution line. Q1: note that the Windows build opens a console window with logs.
→ check: table renders; `python tools/qa/tree_check.py` and `font_check.py` untouched and green.

### Step 6 — local flip-RED on this Windows host (implementer; commands + outputs in `scratch/flips.md`)
Build once `cargo build --release --locked -p gta_like --bin gta_like` (local, dynamic CRT) and use the existing
`fast` build `target/debug/gta_like.exe` (rebuild if stale). Output under `target/release-dist`. Do NOT build a
crt-static exe locally (it would rebuild the whole release tree in the shared `target/`); GREEN for the CRT check is
proven by the CI run.
- F1a dylib: `package --exe target/debug/gta_like.exe` → RED "links Rust/Bevy dynamically", names `bevy_dylib.dll`,
  `std-*.dll`.
- F1b Linux spelling: a synthetic file `target/release-dist/fake_linux/gta_like` containing
  `b"\0libbevy_dylib.so\0libstd-0123456789abcdef.so\0"` → RED naming both (this is the ELF `DT_NEEDED` spelling).
- F1c CRT: `package --exe target/release/gta_like.exe` (local, dynamic CRT) → RED "links the MSVC CRT dynamically",
  names `VCRUNTIME140.dll`. To get a GREEN zip for F2/F3 locally, add a `--allow-dynamic-crt` flag used ONLY by local
  QA/flip runs, or build the GREEN zip from the CI artifact (preferred: download the branch-run artifact with
  `gh run download`). Pick one and record it; the CI command line never passes the flag.
- F2 zip (on a GREEN zip): (a) add `NAME/extra.dll` → RED "unexpected"; (b) drop the `inter` licence entry → RED
  "licence missing" + "missing"; (c) one RON's bytes changed → RED sha256; (d) rewrite the zip with the exe entry
  renamed to `NAME/gta_like` and `external_attr = 0` → RED exec bit. Original → GREEN.
- F3 smoke (real GPU, unpacked GREEN zip outside the repo): (a) menu run 20 s → GREEN; (b) delete one listed
  third-party file → RED early exit code 1 (preflight); (c) only the exe in an empty dir → RED (exe-relative
  resolution); (d) `--expect "no such marker"` → RED; (e) truncate one GLB the city loads at startup, `-- --seed 1`,
  60 s → RED on an `ERROR` line; restore → GREEN.
Run the F3 GREEN case 3 times (gates domain: touched presentation-adjacent gate runs at least 3 times).

### Step 7 — real run proof (QA; in scope per orchestrator)
As PLAN.md Step 7, plus:
- Record the Windows CI smoke result (markers + adapter name, expect `Microsoft Basic Render Driver`) or the
  `dead_end` if it was removed.
- On the Windows zip downloaded from the release, also run `package_release.py verify` (no CRT/dylib imports) before
  booting it.
- Workflow_dispatch: proven after merge by the owner's click, or accepted as identical to the branch-push path.
Tag procedure unchanged: `v0.1.0-rc1` on the branch HEAD, pushed over SSH; failure → `v0.1.0-rc2`, never move a tag.

## 5. Risk areas

- **R1 lavapipe/xvfb under Bevy 0.19**: environment ERROR lines → narrow `--allow` with a comment, never drop the
  scan. If Ubuntu 24.04's Mesa lavapipe cannot run it, first try `ppa:kisak/turtle` (what Bevy CI uses), then
  `WGPU_BACKEND=gl`.
- **R2 `rust-lld.exe` on windows-2025 (VS 2026)**: link failure → `CARGO_TARGET_X86_64_PC_WINDOWS_MSVC_LINKER: link.exe`
  in the Windows entry, `dead_end` logged.
- **R3 +crt-static**: a C dependency built by `cc` follows `CARGO_CFG_TARGET_FEATURE`, so it should match; if the
  link fails with CRT symbol conflicts (LNK2038 / duplicate `__imp_` symbols), log `dead_end`, drop the flag and ship
  the dynamic CRT with a README requirement ("Microsoft Visual C++ 2015-2022 Redistributable x64"). Do not ship a
  Windows zip that silently needs it.
- **R4 Windows CI smoke**: no dx12 adapter or WARP-specific ERROR → remove the step (not `continue-on-error`), QA-local
  stays. A flaky Windows smoke blocks releases, so one environment failure is enough to remove it.
- **R5 cold caches**: first Windows release build cold (30-45 min, `timeout-minutes: 90`); `release-gates` is cold on
  every tag (~10-15 min); two new rust caches (~1-2.5 GB each) under the 10 GB cap evict older generations (watch
  sim/client misses after the first release). crt-static changes the Windows cache key only for the `release` job.
- **R6 download-artifact v8**: `skip-decompress: true` keeps the zips; `test ... -eq 2` catches a layout surprise.
- **R7 smoke timing**: menu marker comes from `OnEnter`, well inside 30 s; the seed run is liveness + error scan over
  60 s, not "city fully loaded". Do not raise the claim.
- **R8 tag on a squash-merged branch**: rc tag points at a branch commit; fine for the pre-release proof; the owner
  cuts `v0.1.0` from main.
- **R9 Linux runtime deps on user machines**: glibc >= 2.39, `libasound.so.2`, `libudev.so.1`, dlopen'd
  X11/Wayland/Vulkan. README, not gated.
- **R10 script injection / token scope**: refs only through env in `run:`; `contents: write` only in `publish`; no
  third-party action gets the write token; publish only on tag push.
- **R11 paths filter**: a `src/**`-only change does not build Windows until the next tag or a release-file change
  (accepted, resolved Q2).

## 6. Open questions
None blocking. Q1-Q4 resolved in `TASK_FINAL.md`. New decision (CRT) is logged in `log.jsonl`; if the owner prefers
shipping the dynamic CRT, the fallback in R3 is the whole change.

No new crates, no `Cargo.lock` change, no new tuning values (smoke durations and CI flags are not game tuning).
children: 0 launched / 0 reported.
