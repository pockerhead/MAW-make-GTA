# PLAN_FINAL — TASK-028: release CI for Linux and Windows

Reviewer: plan-reviewer-2. Base: `PLAN.md` (detail) + `PLAN_V2.md` (corrections, which win on every conflict).
Evidence added by this review: `scratch/crt_static_probe/` (`main.rs`, `result.txt`).

Cost of error: a broken release is mostly loud (red run, zip that does not start), but four failures are silent
and get the proof layer: (1) a release exe that dynamically links `bevy_dylib`/`std-*` (works on the builder, dies
on a user machine); (2) a Windows exe that needs the VC++ redistributable (same: works on the runner and on QA's
host, dies on a clean Windows); (3) a zip missing licence files or assets (boots, then errors in some scene);
(4) a boot smoke that reads the checkout's `assets/` instead of the zip's (green for the wrong reason). Everything
visual ("does the menu look right") is the owner's/QA's eye on the downloaded zip.

## 0. Disconfirmation (done first)

Counter-example tested: "the Windows `+crt-static` build (V2's main correction) either does not link with this
repo's `rust-lld.exe` linker config, or the new CRT byte-scan false-REDs on a statically linked exe (the static
UCRT still carries `api-ms-win-*` strings), so every Windows release run is RED".
Probe (`scratch/crt_static_probe/result.txt`, rustc 1.95.0 = the repo pin, `-C linker=rust-lld.exe`):
a std program using args, stdout and threads built with `-C target-feature=+crt-static` links, runs, and the CRT
regex finds nothing; the same program without the flag imports `VCRUNTIME140.dll` + five `api-ms-win-crt-*`. The
static exe DOES contain one ASCII `api-ms-win` string (a non-CRT api-set), so the regex must stay anchored on
`api-ms-win-crt-`; a broader `api-ms-win-.*` pattern would false-RED. Only C code in the Windows release graph is
`blake3` via `cc` 1.4.7 (`cargo tree -p gta_like --target x86_64-pc-windows-msvc -e normal,build -i cc`), and
`cc` switches to the static CRT when `CARGO_CFG_TARGET_FEATURE` contains `crt-static` (`cc-1.4.7/src/lib.rs:2231`).
**Did not hold.** The crt-static decision stands. Residual (not locally provable without a full crt-static Bevy
build, which is forbidden in the shared `target/`): a Bevy-sized link under crt-static — R3 keeps its fallback.

Second counter-example (cheap): "the smoke passes `--settings-id gta_like_smoke` and the game rejects it or the
argument order". `src/main.rs:39-51` `flag_value` scans all args for the flag in any position; `:179` reads
`--settings-id`. **Did not hold.**

V2's own disconfirmation (tag push vs `branches` + `paths`) carries over: GitHub docs "Path filters are not evaluated
for pushes of tags"; the five test workflows define only `branches-ignore`, so they do not run on tags.

## 1. Summary

Add one workflow `.github/workflows/release.yml` with three jobs: `release` (matrix ubuntu-24.04 / windows-2025:
fetch CC0 assets through the existing composite action, `cargo build --release --locked -p gta_like --bin gta_like`
with no features, static MSVC CRT on Windows, package a zip `gta-like-<version>-<platform>-x86_64.zip` with one top
folder holding the exe and the full `assets/` tree including third-party packs with their licence files and
`manifest.ron`, verify it, boot-smoke the UNPACKED zip on both OSes, upload the zip as an artifact),
`release-gates` (tag push only: headless `cargo test -p gta_sim -p citygen` on Linux) and `publish` (tag push
only: `gh release create` with both zips, `--prerelease` for tags containing `-`). One new Python tool
`tools/package_release.py` (subcommands `package`, `verify`, `smoke`) owns the packaging and all release checks
(no dynamic Rust/Bevy/CRT imports, zip entry set == allowlist, sha256 per entry, licence per pack, exec bit, boot
markers and zero ERROR/panic lines). One `info!("main menu ready")` marker in the menu makes the smoke distinguish
"menu reached" from "hung but alive". ADR-002 records the decisions; README "Запуск" and "Что проверяет CI" are
updated. Proof: local flip-RED of every check on this Windows host, a green branch run, then an rc tag
`v0.1.0-rc1` producing a real pre-release, and QA booting the downloaded Windows zip.

## 2. Verified facts the steps rely on

- Composite `.github/actions/setup/action.yml`: `dtolnay/rust-toolchain@1.95.0` (+clippy), apt
  `libasound2-dev libudev-dev libwayland-dev libxkbcommon-dev` only `if: inputs.linux-deps == 'true'` (the step runs
  `apt-get update` itself), `Swatinem/rust-cache@v2` (`cache-on-failure: true`, no key → key includes `GITHUB_JOB`,
  `runner.os` and the hash of `CARGO_*`/`RUST*` env vars present when the action runs), `actions/setup-python@v7`
  3.12, with `assets: "true"`: `actions/cache@v6` over `assets/third_party/*` minus `manifest.ron` keyed on the
  manifest hash, then `python tools/fetch_assets.py` and `python tools/fetch_assets.py --check`. All run steps
  `shell: bash` → works on the Windows runner (Git Bash).
- Five workflows (`clippy`, `sim`, `citygen`, `client`, `repo` job ids), `push: branches-ignore: [media]` +
  `pull_request` + `workflow_dispatch`, `permissions: contents: read`, `actions/checkout@v7`, `runs-on:
  ubuntu-latest`, env `CARGO_TERM_COLOR: always`, `CARGO_INCREMENTAL: 0`, `CARGO_PROFILE_DEV_DEBUG: 0`,
  `CARGO_PROFILE_TEST_DEBUG: 0`. None runs on a tag push.
- `gh` on this host has pull-only rights; pushes go over SSH as pockerhead. The repo is public, has no tags and no
  releases. Main uses squash merges.
- `Cargo.toml`: `fast = ["bevy/dynamic_linking"]` opt-in; client `bevy` default features + `bevy_settings`; no
  `[profile.release]` (Cargo default release profile); `rust-version = "1.95.0"`. Features `dev`/`debug` gate BRP
  and debug tools (`src/main.rs:298-301`) and are not enabled by a no-features build. No tracing `max_level_*` /
  `release_max_level_*` feature in the graph, so `info!` survives in release.
- `.cargo/config.toml`: only `[target.x86_64-pc-windows-msvc] linker = "rust-lld.exe"`. No Linux section.
- `.gitattributes`: `* text=auto eol=lf` → tracked RON/WGSL bytes are LF on every checkout (including this host with
  `core.autocrlf=true`), so the Windows and Linux zips carry identical asset bytes and local sha comparisons match.
- Data root: `src/main.rs:194` `ConfigRoot(FileAssetReader::get_base_path().join("assets"))`; bevy_asset 0.19.1
  `src/io/file/mod.rs:19-29`: `BEVY_ASSET_ROOT` → `CARGO_MANIFEST_DIR` → `current_exe().parent()`. All compile-time
  paths (`include_str!`, `env!("CARGO_MANIFEST_DIR")`) are `#[cfg(test)]` (PREMISE_CHALLENGE).
- `preflight` (`src/main.rs:66-168`) checks configs and `manifest.missing_files(root)` (`:155`, existence only, not
  sha); on failure `eprintln!` + `AppExit::error()` before `app.run()` → exit code 1.
- Without `--seed`/`--bench-scene` the game enters `GameState::MainMenu` (`:217-219`); `MenuPlugin` runs
  `screens::spawn_main_menu` on `OnEnter(GameState::MainMenu)` (`src/menu/mod.rs:52`, fn at
  `src/menu/screens.rs:39`, `bevy::prelude::*` imported at `:10-14`). No log line marks it today.
- bevy_render 0.19.1 `src/renderer/mod.rs:287-288` `info!("{:?}", adapter_info)` → a line containing `AdapterInfo {`;
  no adapter → panic "Unable to find a GPU!" (`:142-144`). bevy_log 0.19.1 default `level: Level::INFO`.
  bevy_audio: "No audio device found." is `warn!`. bevy_gilrs 0.19.1 `src/lib.rs:113`
  `error!("Failed to start Gilrs. {}", err)` — a possible environment ERROR on a runner (R1).
  Asset load failures are `error!` in `bevy_asset-0.19.1/src/server/mod.rs:593,684,976`.
- `tools/fetch_assets.py`: `REPO` `:26`, `THIRD_PARTY` `:27`, `MANIFEST` `:28`, `load_manifest(path)` `:259`
  (parses + schema-checks, raises `ManifestError`), `sha256_file(path)` `:271`, `check(doc)` `:338` returns a list of
  problems (unexpected entries in `assets/third_party/`, missing/unexpected files, sha mismatch, rig mismatch);
  schema rule `:228`: every pack's `license_file` is listed in its `files`. Module-level code is imports and
  constants only; `main()` runs under `if __name__ == "__main__"` → importable.
- `git ls-files assets` = 26 files (RON configs, `shaders/*.wgsl`, `third_party/manifest.ron`).
- Dynamic-link probes: `scratch/dylib_scan_probe.txt` (release exe: no dylib names; `fast` debug exe:
  `bevy_dylib.dll`, `std-0cebe7c42cd80226.dll`), `scratch/crt_probe.txt` (local release exe imports
  `VCRUNTIME140.dll` + six `api-ms-win-crt-*`), `scratch/crt_static_probe/result.txt` (see §0).
- Actions (verified in `action.yml` at the pinned majors): upload-artifact v7 `archive: false` uploads one file
  as-is, artifact name = file name, `overwrite` default false (second upload with the same name fails);
  download-artifact v8 `pattern`, `merge-multiple`, `skip-decompress`. Bevy upstream CI: Linux examples under
  `xvfb-run` with `mesa-vulkan-drivers` (+`ppa:kisak/turtle`), Windows job `run-examples-on-windows-dx12` on
  `windows-latest` with `WGPU_BACKEND=dx12`.
- `*.log` and `/target/` are gitignored: smoke logs at the repo root and `target/release-dist` are not tracked.

## 3. Decisions (each is also in `log.jsonl`)

| Topic | Decision | Rejected alternative |
|---|---|---|
| Triggers | `push: tags: ['v*']` → build + gates + publish; `push: branches: ['**']` + `paths` filter on release files → build + package + smoke + artifacts; `workflow_dispatch` → same as branch push (artifacts only). | every push to main (30+ min Windows build on docs commits); tag-only (no pre-merge run). |
| Runners | `ubuntu-24.04`, `windows-2025`, pinned. Linux zip needs glibc >= 2.39. | `*-latest` (silently moves the glibc floor). |
| Toolchain | composite's `dtolnay/rust-toolchain@1.95.0` = `rust-version`. | a second pin. |
| Build | `cargo build --release --locked -p gta_like --bin gta_like`, no `--features`, default release profile. | LTO/strip/custom profile (not asked; changes what the owner measures). |
| Windows CRT | `+crt-static` via `CARGO_TARGET_X86_64_PC_WINDOWS_MSVC_RUSTFLAGS` set at JOB level from the matrix (so rust-cache hashes it into the key); `package`/`verify` fail on any CRT DLL import in a `.exe`. | dynamic CRT + README "install VC++ redist"; `rustflags` in `.cargo/config.toml` (changes local dev/`fast`); step-level env (cache key blind to it); `RUSTFLAGS` (would also hit Linux). |
| Linker | keep `rust-lld.exe` (probe: links with crt-static). Link failure on windows-2025 → `CARGO_TARGET_X86_64_PC_WINDOWS_MSVC_LINKER: link.exe` in the Windows matrix env, `dead_end` logged. | `link.exe` up front. |
| Caching | rust-cache via the composite, job ids `release` (key differs per OS) and `release-gates`; assets via `actions/cache`. No sccache. | sccache (more moving parts for a rare job). |
| No-dylib / no-CRT proof | byte scan of the exe for import names (two regexes, two messages) in `package` and `verify`; zip allowlist (only exe + `assets/**`, so no `.dll`/`.so`/`.pdb` rides along); boot smoke from the unpacked zip. | `dumpbin`/`readelf` (not portable), `cargo tree` (graph, not binary). |
| Zip layout | `gta-like-<version>-<platform>-x86_64.zip`, one top folder of the same name with `gta_like[.exe]` + `assets/`. `<version>` = tag name on tag push, `sha-<8 hex>` otherwise; `<platform>` = `linux`/`windows`. Exe entry unix mode 0755. | tarball; flat zip. |
| Linux smoke | `xvfb-run` + Mesa lavapipe (`WGPU_BACKEND=vulkan`): (a) menu 30 s, expect `AdapterInfo {` + `main menu ready`; (b) `--seed 1` 60 s, expect `AdapterInfo {`. | bare liveness; a `--smoke` CLI mode in the game. |
| Windows smoke | on the runner: `WGPU_BACKEND=dx12`, menu 30 s, same expects. Gate if the first run is green; environment failure → remove the step (never `continue-on-error`), `dead_end`. QA still boots the downloaded zip locally (spec AC). | no CI smoke (premise "WARP in a service session is unproven" refuted by Bevy CI). |
| Headless tests | `release-gates` (ubuntu-24.04, assets) `cargo test --locked --no-fail-fast -p gta_sim -p citygen`, tag push only; `publish` needs it. | on every trigger (duplicates sim/citygen gates on branch pushes). |
| Publish | `publish` job, `if: github.event_name == 'push' && startsWith(github.ref, 'refs/tags/v')`, `needs: [release, release-gates]`, `permissions: contents: write` on this job only, preinstalled `gh release create --verify-tag --generate-notes [--prerelease]`, `GH_TOKEN: ${{ github.token }}`. | ref-only condition (dispatch on a tag would publish); `softprops/action-gh-release` (third-party code with a write token); per-OS upload (half-published release). |
| Secrets | workflow `permissions: contents: read`; only `GITHUB_TOKEN`; no `pull_request_target`; refs reach shell only via env (`$GITHUB_REF_NAME`, `$GITHUB_SHA`), never `${{ github.* }}` inside `run:`. | — |
| Local GREEN zip for flips | F2 packages a synthetic fixture exe; F3 swaps the local release exe into the unpacked GREEN zip. The tool has NO escape flag. | `--allow-dynamic-crt` (a bypass shipped in the release tool); `gh run download` of the CI artifact (untested with `archive: false`; forces flips after CI). |
| Console window (Q1) | keep the console subsystem in this pipeline; README says a console window with logs opens. | `windows_subsystem = "windows"` (separate polish before v0.1.0 final). |
| Licence (Q4) | no LICENSE for game code; README "лицензия на код игры пока не выбрана" next to the third-party attribution. | adding one (owner's legal choice). |

## 4. Implementation steps

### Step 1 — `src/menu/screens.rs`: menu marker
Insert `info!("main menu ready");` as the first line of the body of `spawn_main_menu` (`:39`, before
`let menu = &ui.menu;`). `info!` comes from `bevy::prelude::*` (already imported). Nothing else changes.
Reason: the smoke must tell "MainMenu `OnEnter` ran" from "alive but hung before the first update".
→ check: `cargo clippy --workspace --all-targets -- -D warnings` clean; `cargo test -p gta_like --bin gta_like`
green (menu gates untouched); `cargo run --release` prints the line when the menu appears.

### Step 2 — `tools/package_release.py` (new; target < 300 lines; Python 3.10+ compatible; stdlib only)
Header docstring with usage of the three subcommands. `REPO = Path(__file__).resolve().parents[1]`;
`sys.path.insert(0, str(Path(__file__).parent))`; `from fetch_assets import MANIFEST, check, load_manifest,
sha256_file`. `argparse` with subparsers. Every failure prints to stderr a line starting with
`<subcommand>: <check>: ...` and exits 1. A missing INPUT (zip/exe/dir not found, `git` fails, manifest unreadable)
prints `<subcommand>: GATE BROKEN: ...` and exits 2, so plumbing failures are distinguishable from product failures.
`verify` collects ALL problems before exiting (each flip must be able to go RED alone and show its own message).

Helpers:
- `DYLIB = re.compile(rb"(?:lib)?(?:bevy_dylib|std-[0-9a-f]{16})\.(?:dll|so)")`
- `CRT = re.compile(rb"(?i)(?:vcruntime140(?:_1)?|msvcp140(?:_[0-9a-z]+)?|ucrtbased?|api-ms-win-crt-[a-z0-9-]+)\.dll")`
  (must stay anchored on `api-ms-win-crt-`: a static exe contains other `api-ms-win-*` strings, §0).
- `dynamic_imports(data: bytes, windows: bool) -> list[str]`: returns problem strings, one per class present:
  `"exe links Rust/Bevy dynamically: <sorted names> (built with `fast`?)"` for DYLIB matches, and, only when
  `windows`, `"exe links the MSVC CRT dynamically: <sorted names> (built without +crt-static)"` for CRT matches.
  `windows` = the exe file/entry name ends with `.exe`.
- `expected_assets() -> list[str]`: `git -C REPO ls-files -z assets` (posix paths relative to `assets/`) ∪
  `third_party/<pack name>/<file path>` for every file of every pack of `load_manifest(MANIFEST)`, sorted, deduped.
- `expected_sha(rel, doc) -> str`: for `third_party/<pack>/<path>` → that file's `sha256` from the manifest (fixed
  quantity, independent of the local checkout); otherwise `sha256_file(REPO / "assets" / rel)` (tracked file).

`package --exe PATH --name NAME --out DIR`
1. `PATH` missing → GATE BROKEN. `problems = check(load_manifest(MANIFEST))`; non-empty → fail
   "third-party packs do not match the manifest: <problems>; run tools/fetch_assets.py".
2. `dynamic_imports(PATH bytes, PATH.name.endswith(".exe"))` non-empty → fail with those messages. No zip written.
3. `DIR.mkdir(parents=True, exist_ok=True)`; write `DIR/NAME.zip` with `ZIP_DEFLATED`: exe entry
   `NAME/<PATH.name>` via `ZipInfo` with `create_system = 3`, `external_attr = (0o100755 << 16)`,
   `compress_type = ZIP_DEFLATED`, date_time from the file; then `NAME/assets/<rel>` for each `expected_assets()`
   path, read from `REPO/assets/<rel>` (use `ZipInfo` too so every entry has a fixed `create_system = 3` and mode
   `0o100644 << 16`).
4. Run `verify` on the written zip; print its summary (entry count, zip size, "no dynamic imports").

`verify ZIP`
1. `ZIP` missing → GATE BROKEN. All entries start with one common top folder `NAME/`; exactly one of
   `NAME/gta_like` / `NAME/gta_like.exe` exists (else "exe entry").
2. Entry set == `{exe} ∪ {NAME/assets/<rel> for rel in expected_assets()}`: report "missing: ..." and
   "unexpected: ..." separately (the allowlist).
3. For each `assets/` entry present and expected: sha256 of the entry bytes == `expected_sha(rel)`; else
   "sha256 <rel>: zip <a>, expected <b>".
4. For every pack: `NAME/assets/third_party/<pack>/<license_file>` present, else "licence missing: <pack>"
   (in addition to the "missing" line of 2).
5. Exe entry: `dynamic_imports(entry bytes, name.endswith(".exe"))` empty; for the extension-less exe,
   `(external_attr >> 16) & 0o111 != 0`, else "exec bit: mode <oct>".
6. Print the summary with the gate classes: correctness = allowlist, sha256, licences, no dynamic imports.

`smoke DIR --seconds N --log FILE [--expect TEXT]... [--allow REGEX]... [-- GAME_ARGS...]`
1. `DIR` is the unpacked top folder; exe = `DIR/gta_like.exe` or `DIR/gta_like` (neither → GATE BROKEN).
2. `log = Path(FILE).resolve()` BEFORE spawning (the child's cwd differs). `env = dict(os.environ)` minus
   `CARGO_MANIFEST_DIR` and `BEVY_ASSET_ROOT`; `cwd = tempfile.mkdtemp()` (not the exe dir, not the checkout);
   argv = `[exe, "--settings-id", "gta_like_smoke", *GAME_ARGS]` (the game's `flag_value` takes the first
   occurrence, so nothing a caller passes can touch the owner's settings file); stdout+stderr → `log`.
3. Poll every 0.5 s up to `N` s: early exit (any code) → fail "exited after X s with code C" + last 40 log lines.
   At the deadline `kill()` + `wait()`; the kill exit code is not checked.
4. Read the log (utf-8, `errors="replace"`), strip ANSI `\x1b\[[0-9;]*m`. Fail on any line matching the
   case-sensitive `\bERROR\b` or `panicked` that no `--allow` regex matches (print each such line); fail on any
   `--expect` text found on no line. Print the matched expect lines, the `AdapterInfo` line, and the WARN count.
5. Summary names the classes: liveness = alive at N s; state = `AdapterInfo {` (renderer up) and
   `main menu ready` (MainMenu `OnEnter` ran); correctness = zero ERROR/panic lines over the window. Not claimed:
   "the menu looks right" (QA/owner), "the city fully loaded" (seed run is liveness + error scan).

→ check: `python tools/package_release.py --help` and each `<sub> --help` work; flips in Step 6.

### Step 3 — `.github/workflows/release.yml` (new)
```yaml
name: release
on:
  push:
    tags: ['v*']
    branches: ['**']
    paths:
      - .github/workflows/release.yml
      - .github/actions/**
      - tools/package_release.py
      - tools/fetch_assets.py
      - assets/third_party/manifest.ron
      - Cargo.toml
      - Cargo.lock
      - .cargo/**
  workflow_dispatch:
permissions:
  contents: read
concurrency:
  group: ${{ github.workflow }}-${{ github.ref }}
  cancel-in-progress: ${{ !startsWith(github.ref, 'refs/tags/') }}
env:
  CARGO_TERM_COLOR: always
  CARGO_INCREMENTAL: 0
  CARGO_PROFILE_DEV_DEBUG: 0
  CARGO_PROFILE_TEST_DEBUG: 0
jobs:
  release:            # unique job id: rust-cache key (TASK-029 N1); matrix differs by runner.os
    strategy:
      fail-fast: false
      matrix:
        include:
          - { os: ubuntu-24.04, platform: linux, exe: gta_like, deps: "true", rustflags: "" }
          - { os: windows-2025, platform: windows, exe: gta_like.exe, deps: "false", rustflags: "-C target-feature=+crt-static" }
    runs-on: ${{ matrix.os }}
    timeout-minutes: 90
    env:
      # Job level so rust-cache (inside the setup action) hashes it; only the msvc target reads it.
      CARGO_TARGET_X86_64_PC_WINDOWS_MSVC_RUSTFLAGS: ${{ matrix.rustflags }}
    steps:
      - uses: actions/checkout@v7
      - uses: ./.github/actions/setup
        with:
          linux-deps: ${{ matrix.deps }}
          assets: "true"
      - name: name
        shell: bash
        run: |
          if [[ "$GITHUB_EVENT_NAME" == push && "$GITHUB_REF" == refs/tags/v* ]]; then v="$GITHUB_REF_NAME"; else v="sha-${GITHUB_SHA::8}"; fi
          echo "NAME=gta-like-$v-${{ matrix.platform }}-x86_64" >> "$GITHUB_ENV"
      - name: build (release, no features)
        shell: bash
        run: cargo build --release --locked -p gta_like --bin gta_like
      - name: package + verify
        shell: bash
        run: python tools/package_release.py package --exe "target/release/${{ matrix.exe }}" --name "$NAME" --out dist
      - name: boot smoke (Linux, unpacked zip, xvfb + lavapipe)
        if: matrix.platform == 'linux'
        shell: bash
        env:
          WGPU_BACKEND: vulkan
        run: |
          sudo apt-get update
          sudo apt-get install --no-install-recommends -y xvfb mesa-vulkan-drivers libvulkan1 libxkbcommon-x11-0 libxcursor1 libxrandr2 libxi6 libx11-xcb1
          mkdir -p "$RUNNER_TEMP/unpacked" && unzip -q "dist/$NAME.zip" -d "$RUNNER_TEMP/unpacked"
          xvfb-run -a -s "-screen 0 1280x720x24" python tools/package_release.py smoke "$RUNNER_TEMP/unpacked/$NAME" --seconds 30 --log smoke-menu.log --expect "AdapterInfo {" --expect "main menu ready"
          xvfb-run -a -s "-screen 0 1280x720x24" python tools/package_release.py smoke "$RUNNER_TEMP/unpacked/$NAME" --seconds 60 --log smoke-seed.log --expect "AdapterInfo {" -- --seed 1
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
      - uses: actions/upload-artifact@v7
        with:
          path: dist/${{ env.NAME }}.zip
          archive: false
          if-no-files-found: error
  release-gates:
    if: github.event_name == 'push' && startsWith(github.ref, 'refs/tags/v')
    runs-on: ubuntu-24.04
    timeout-minutes: 60
    steps:
      - uses: actions/checkout@v7
      - uses: ./.github/actions/setup
        with: { assets: "true" }
      - shell: bash
        run: cargo test --locked --no-fail-fast -p gta_sim -p citygen
  publish:
    if: github.event_name == 'push' && startsWith(github.ref, 'refs/tags/v')
    needs: [release, release-gates]
    runs-on: ubuntu-24.04
    timeout-minutes: 15
    permissions:
      contents: write
    steps:
      - uses: actions/download-artifact@v8
        with: { pattern: "gta-like-*", path: dist, merge-multiple: true, skip-decompress: true }
      - shell: bash
        env:
          GH_TOKEN: ${{ github.token }}
          GH_REPO: ${{ github.repository }}
        run: |
          ls -lR dist
          test "$(ls dist/*.zip | wc -l)" -eq 2
          pre=(); [[ "$GITHUB_REF_NAME" == *-* ]] && pre=(--prerelease)
          gh release create "$GITHUB_REF_NAME" dist/*.zip --verify-tag --generate-notes "${pre[@]}"
```
Notes for the implementer:
- `${{ matrix.* }}` / `${{ env.NAME }}` in `run:`/`with:` are static values we control, not user input; refs only via
  env vars. Do not add `pull_request` (branch pushes already cover it; fork PRs would get no useful run).
- `smoke-logs-*` does not match the publish `pattern: "gta-like-*"`.
- If `download-artifact` with `skip-decompress` produces a nested layout, adjust the `ls`/glob, not the checks.
- An empty `CARGO_TARGET_X86_64_PC_WINDOWS_MSVC_RUSTFLAGS` on the Linux leg is never read (msvc target only). If
  cargo on Windows rejects the value shape, keep it job-level but move it into a Windows-only `if:`-guarded
  `echo ... >> "$GITHUB_ENV"` step placed BEFORE the setup action; never use `RUSTFLAGS`.
- The Windows smoke is part of the first branch run. If it is RED for an environment reason (no dx12 adapter,
  a WARP-specific `wgpu_hal` ERROR that cannot be allowed narrowly), delete the step, log `dead_end` with the log
  excerpt, and state it in ADR-002 and the QA summary. An `--allow` is permitted only for an exact
  target + message with a one-line YAML comment why; never drop the ERROR scan.
→ check (first branch push): `release` starts for both OSes (paths filter hits `release.yml`); `name`, `build`,
`package + verify` (Windows summary prints "no dynamic imports"), both smokes green with their marker lines; two
artifacts `gta-like-sha-<8>-linux-x86_64.zip` and `gta-like-sha-<8>-windows-x86_64.zip`; rust-cache log keys start
`v0-rust-release-Linux` / `v0-rust-release-Windows`; `release-gates` and `publish` are skipped.

### Step 4 — `docs/decisions/ADR-002-release-ci.md` (new, Russian like ADR-001; sections Контекст / Решение / Альтернативы / Обновление)
Records §3's table: triggers and paths filter, runners + glibc floor, toolchain pin, build command/profile, CRT
decision (+crt-static in CI only, why: a clean Windows has UCRT but not `VCRUNTIME140.dll`; how verified: the
`package`/`verify` CRT scan; the zip exe differs from a local `cargo run --release` only in CRT linkage), linker,
caches (incl. `release-gates` and the rc tag run being cold on every tag, ~10-15 min for gates), no-`fast` proof,
zip naming/layout, Linux smoke, Windows CI smoke status (kept, or removed with the `dead_end` reason), gates on tags,
publish on tag push only + permissions, console window kept (Q1), no game-code licence (Q4), and the procedure to
cut a release: `git tag -a vX.Y.Z[-rcN] -m "..." <main commit>`, `git push origin <tag>`; a failed tag is never
moved or deleted, the next rc number is used.

### Step 5 — `README.md`
- "Запуск" (`:212`): the first paragraph gains where to download: releases page
  `https://github.com/pockerhead/MAW-make-GTA/releases`, one zip per OS, unpack and run `gta_like.exe` / `./gta_like`;
  the `assets/` folder must stay next to the exe. Windows: nothing extra to install (static CRT); a console window
  with logs opens next to the game. Linux: glibc >= 2.39 (Ubuntu 24.04+), `libasound2`, `libudev1`, a Vulkan
  driver, X11 or Wayland.
- Replace the stale paragraph at `:236-238` ("...Для сборки из исходников папку `assets/` берём из последнего
  релиза.") with the truth: from source, the binary assets come from `python tools/fetch_assets.py` (already said at
  `:214-219`); release zips carry them together with the licence files and `manifest.ron` (attribution); the game
  code licence is not chosen yet ("лицензия на код игры пока не выбрана").
- "Что проверяет CI" (`:175-189`): add a table row `release` (tag `v*` → Linux + Windows zips, boot smoke on both,
  `cargo test -p gta_sim -p citygen`, GitHub Release; release-file changes / manual run → artifacts only). Change
  "Пять workflow ... на каждый push и PR" to say five on every push and PR plus `release` on tags and release-file
  changes. No new badge (a tag-only workflow badge is mostly "no status"). If the Windows smoke was removed, the row
  says "boot smoke on Linux".
→ check: the table renders (same column count); `python tools/qa/tree_check.py` and `python tools/qa/font_check.py`
green.

### Step 6 — local flip-RED on this Windows host (implementer; commands + outputs in `scratch/flips.md`)
Setup: `cargo build --release --locked -p gta_like --bin gta_like` (local, dynamic CRT); `target/debug/gta_like.exe`
from `cargo build --features fast` (rebuild if stale). Output under `target/release-dist/` (git-ignored). Do NOT
build a crt-static Bevy exe locally (it rebuilds the whole release tree in the shared `target/`); GREEN of the CRT
check on a real exe is proven by the CI run.
- F1a dylib (package): `package --exe target/debug/gta_like.exe --name T --out target/release-dist` → RED, message
  "links Rust/Bevy dynamically" naming `bevy_dylib.dll` and `std-*.dll` (a CRT line may also appear; the dylib line
  is what this flip asserts).
- F1b Linux spelling, dylib branch alone: synthetic `target/release-dist/fake_linux/gta_like` with bytes
  `b"\x7fELF\0libbevy_dylib.so\0libstd-0123456789abcdef.so\0"` → `package` RED naming both, and NO CRT line
  (no `.exe` suffix).
- F1c CRT branch alone: `package --exe target/release/gta_like.exe ...` → RED "links the MSVC CRT dynamically"
  naming `VCRUNTIME140.dll`, and NO dylib line.
- GREEN fixture zip: synthetic `target/release-dist/fixture/gta_like.exe` = `b"MZ fixture exe, no imports"` →
  `package --exe target/release-dist/fixture/gta_like.exe --name gta-like-local-windows-x86_64 --out
  target/release-dist` → GREEN (this fixture stands in for the exe only; F2 tests the zip checks).
- F2 verify (copies of the GREEN zip, rewritten with `zipfile`; original stays GREEN):
  (a) extra entry `NAME/extra.dll` → RED "unexpected";
  (b) without `NAME/assets/third_party/inter/<its license_file>` → RED "licence missing: inter" and "missing";
  (c) one RON entry's bytes changed → RED "sha256 <rel>";
  (c2) one third-party GLB entry's bytes changed → RED "sha256" (proves the manifest-sha branch);
  (d) exe entry renamed to `NAME/gta_like` with `external_attr = 0` → RED "exec bit";
  (e) exe entry bytes replaced by `target/release/gta_like.exe` → RED "links the MSVC CRT dynamically" (proves
  `verify`'s own scan, which QA relies on for the downloaded zip);
  (f) a nonexistent zip path → exit 2 "GATE BROKEN".
- F3 smoke (real GPU): unpack the GREEN fixture zip with `python -m zipfile -e` to a temp dir OUTSIDE the repo, then
  overwrite `<dir>/gta_like.exe` with `target/release/gta_like.exe` (runs here: the host has the redist).
  (a) `smoke <dir> --seconds 20 --log <abs>/f3a.log --expect "AdapterInfo {" --expect "main menu ready"` → GREEN,
  run 3 times (gates domain: a touched presentation-adjacent gate runs >= 3 times);
  (b) delete one listed file under `<dir>/assets/third_party/` → RED "exited ... code 1" (preflight); restore;
  (c) copy only the exe into another empty dir and smoke it → RED (exe-relative resolution: same exe, no `assets/`);
  (d) `--expect "no such marker"` → RED "expect";
  (e) truncate `<dir>/assets/third_party/mini-characters/character-male-a.glb` (the player model from
  `assets/character/visual.ron`; the copy, never the checkout) to 100 bytes, `--seconds 60 -- --seed 1` → RED on an
  `ERROR` line (preflight checks existence only, so the game starts and the loader errors); restore → GREEN;
  (f) run (a) with `CARGO_MANIFEST_DIR=D:/test-gta-like` exported in the calling shell and case (c)'s exe-only dir
  → still RED (proves the tool strips the variable; without stripping it would read the checkout and go GREEN).
Record each flip: command, perturbed input, first failing message, exit code.

### Step 7 — real run proof (QA; in scope per orchestrator)
1. Branch push run: record its URL, both artifact names, the smoke marker lines, adapter names (Linux expect
   `llvmpipe`, Windows expect `Microsoft Basic Render Driver`), the Windows "no dynamic imports" summary, or the
   Windows smoke `dead_end`.
2. Tag: `git tag -a v0.1.0-rc1 -m "TASK-028 release CI proof" <branch HEAD>`; `git push origin v0.1.0-rc1` (SSH as
   pockerhead; gh here is read-only). Watch: `gh run list --repo pockerhead/MAW-make-GTA --workflow release.yml
   --limit 3`, `gh run watch <id> --repo pockerhead/MAW-make-GTA`. Expect the tag run to be cold (tag refs cannot
   restore branch caches).
3. `gh release view v0.1.0-rc1 --repo pockerhead/MAW-make-GTA --json url,isPrerelease,assets` → two zips,
   `isPrerelease: true`. Record run URL and release URL.
4. Windows: `gh release download v0.1.0-rc1 --repo pockerhead/MAW-make-GTA -p "*windows*" -D <temp outside repo>`;
   from a clean checkout at the tag commit (fetched packs, `python tools/fetch_assets.py --check` green) run
   `python tools/package_release.py verify <zip>` (GREEN: no CRT/dylib imports, allowlist, sha); unpack with
   PowerShell `Expand-Archive` outside the repo; run `smoke` with the menu expects (GREEN); start `gta_like.exe` by
   hand once and confirm the main menu shows (screenshot into scratch).
5. Tag run fails → fix on the branch, push, use `v0.1.0-rc2`; never move or delete a pushed tag.
6. `workflow_dispatch`: the "Run workflow" button exists only once `release.yml` is on main; proven by the owner's
   click after merge, or accepted as identical to the branch-push path (same non-tag steps, `publish` skipped by
   its `event_name` condition).

## 5. Test plan

| Claim | Gate | Class | How / expected |
|---|---|---|---|
| no dynamic Rust/Bevy in the exe | `package`/`verify` DYLIB scan | correctness | F1a, F1b RED; CI GREEN on both OSes |
| no dynamic MSVC CRT in the Windows exe | `package`/`verify` CRT scan | correctness | F1c, F2e RED; CI Windows GREEN (crt-static); QA `verify` on the release zip GREEN |
| zip holds exactly exe + assets (+licences, manifest) | `verify` allowlist + licence | correctness | F2a, F2b RED; CI GREEN |
| zip bytes are the right bytes | `verify` sha256 (manifest for packs, checkout for tracked) | correctness | F2c, F2c2 RED |
| Linux exe is executable after unzip | `verify` exec bit | correctness | F2d RED; CI Linux smoke starts the exe |
| exe reads the zip's `assets/`, not the checkout | `smoke` env strip + temp cwd | correctness | F3c, F3f RED |
| game reaches the menu with a renderer | `smoke` expects | state | F3a GREEN x3, F3d RED; CI both OSes |
| no asset/load errors in menu and in a seeded city for 60 s | `smoke` ERROR/panic scan | correctness (bounded window) | F3e RED; CI Linux seed run GREEN |
| all third-party files present at start | game preflight via `smoke` early exit | liveness | F3b RED |
| headless gameplay gates on release tags | `release-gates` | correctness | green in the rc tag run |
| a real release exists | `publish` | liveness | rc tag → release with 2 zips, prerelease |
| the Windows menu looks right | owner/QA eye | — | Step 7.4 screenshot; not gated |
Also: `cargo clippy --workspace --all-targets -- -D warnings`, `cargo test -p gta_like --bin gta_like`,
`cargo test -p gta_sim -p citygen`, `python tools/qa/tree_check.py`, `python tools/qa/font_check.py` green locally;
the five existing workflows green on the branch push.

## 6. Rollout notes

- No migrations, no new crates, no `Cargo.lock` change, no game tuning values (smoke durations and CI flags are CI
  arguments, not game tuning). No new secrets: only `GITHUB_TOKEN`, `contents: write` only in `publish`.
- New env vars in CI only: `CARGO_TARGET_X86_64_PC_WINDOWS_MSVC_RUSTFLAGS` (Windows leg), `WGPU_BACKEND` (smoke
  steps). Local builds are unchanged: the local Windows release exe keeps the dynamic CRT.
- Two new rust caches (~1-2.5 GB each) under the 10 GB repo cap evict older generations (LRU); watch sim/client cache
  misses after the first release. First Windows release build cold: 30-45 min (`timeout-minutes: 90`).
- Post-merge: `release` also runs on the merge commit (paths filter hits); the orchestrator updates the AGENTS.md CI
  rule at closure ("the 5 test workflows success on the merge commit; release.yml success on its own triggers", Q3).
- The rc tag points at a branch commit that will not be on main after the squash merge; fine for the pre-release
  proof. The owner cuts `v0.1.0` from main.

### Risk areas
- **R1 lavapipe/xvfb under Bevy 0.19**: environment ERROR lines (winit/wgpu on Xvfb; `Failed to start Gilrs` if
  udev is unusable) → a narrow `--allow` with exact target + message and a one-line YAML comment; never drop the
  scan. If Ubuntu 24.04's lavapipe cannot run the game: first `ppa:kisak/turtle` (Bevy CI), then `WGPU_BACKEND=gl`
  (+ `libgl1-mesa-dri libegl1`).
- **R2 `rust-lld.exe` on windows-2025 (VS 2026)**: link failure → `CARGO_TARGET_X86_64_PC_WINDOWS_MSVC_LINKER: link.exe`
  in the Windows matrix entry's job env, `dead_end` logged.
- **R3 +crt-static at Bevy scale**: probe covers std + rust-lld; if the full link fails with CRT conflicts (LNK2038,
  duplicate `__imp_` symbols), log `dead_end`, drop the flag, ship the dynamic CRT with a README requirement
  ("Microsoft Visual C++ 2015-2022 Redistributable x64") and remove the CRT branch from `package` only by an ADR
  update. Never ship a Windows zip that silently needs the redist.
- **R4 Windows CI smoke**: one environment failure → remove the step (not `continue-on-error`); QA-local stays.
- **R5 cold caches**: `release-gates` and the whole tag run are cold on every tag (tag refs restore only
  default-branch and own-ref caches): ~10-15 min for gates, 30-45 min for the Windows build. Accepted (rare).
- **R6 download-artifact v8**: `skip-decompress: true` keeps the zips; `test ... -eq 2` catches a layout surprise.
- **R7 smoke timing**: menu marker from `OnEnter`, well inside 30 s; the seed run is liveness + error scan over
  60 s, not "city fully loaded". Do not raise the claim.
- **R8 Linux runtime deps on user machines**: glibc >= 2.39, `libasound.so.2`, `libudev.so.1`, dlopen'd
  X11/Wayland/Vulkan. README, not gated.
- **R9 script injection / token scope**: refs only via env in `run:`; `contents: write` only in `publish`; no
  third-party action receives the write token; publish only on tag push.
- **R10 paths filter**: a `src/**`-only change does not build Windows until the next tag or release-file change
  (accepted, Q2).
- **R11 local verify on a dirty checkout**: `verify` compares tracked RON/WGSL with the local checkout; QA runs it on
  a clean checkout at the tag commit, otherwise a local edit reads as a sha RED.

## 7. Review notes (what changed from PLAN_V2 and why)

1. **Local GREEN zip for flips is decided, not "pick one".** V2 left two options (`--allow-dynamic-crt` flag or
   `gh run download`). Chosen: a synthetic fixture exe for F2 and the local release exe swapped into the unpacked
   zip for F3. The tool keeps no escape flag; `gh run download` of an `archive: false` artifact is unverified and
   would order flips after CI. Logged as a decision.
2. **Rustflags env moved from the build step to job level.** rust-cache runs inside the setup composite before the
   build step; with step-level env the cache key never saw the flag (V2 R5 claimed it changes the key — it would
   not). Job level makes the key and the build agree.
3. **`verify` checks third-party entries against the manifest sha256**, tracked files against the checkout. V1/V2
   compared everything with checkout bytes, which moves together with a corrupted local pack (a tautology on QA's
   host, where `fetch_assets --check` is not part of `verify`). New flip F2c2.
4. **CRT regex adjusted and constrained by evidence.** Added `ucrtbase[d]` and alnum `msvcp140_*`; kept the anchor on
   `api-ms-win-crt-` because the crt-static probe exe contains another `api-ms-win` string (§0).
5. **New flips**: F2e (verify's own exe scan, which QA's check of the downloaded zip relies on), F2f (GATE BROKEN
   exit 2), F3f (env stripping actually matters). F1a/F1b/F1c now assert which message class appears and which
   does not.
6. **`verify` collects all problems before exiting; plumbing failures exit 2 with GATE BROKEN** (gates domain: each
   assertion must go RED alone; plumbing vs product).
7. **`name` step uses `GITHUB_EVENT_NAME == push` too**, matching the publish condition, so a dispatch on a tag ref
   produces `sha-` artifacts instead of tag-named ones.
8. **Workflow env matches the test workflows** (`CARGO_PROFILE_DEV_DEBUG: 0`, `CARGO_PROFILE_TEST_DEBUG: 0`) so
   `release-gates` builds like `sim-gates`; `publish` gets `timeout-minutes: 15`; `ls -lR` for layout debugging.
9. **F3e names the file**: the player model `mini-characters/character-male-a.glb` (certain to load in a seeded
   run), truncated in the unpacked copy only. V1's "e.g. a car-kit model" was not certain to load within 60 s.
10. **R1 names `Failed to start Gilrs`** (bevy_gilrs 0.19.1 `error!`) as a candidate environment ERROR.
11. **Restored from PLAN.md** where V2 compressed without correcting: full helper/subcommand specs, full YAML,
    verified-facts section, README anchors, ADR sections, Step 7 commands, the risk list.
Verified unchanged from V2: publish/gates condition `event_name == 'push' && startsWith(ref, 'refs/tags/v')`;
string `deps` matrix values; `apt-get update` in the smoke step; per-OS `smoke-logs-<platform>`; absolute `--log`;
default `--settings-id gta_like_smoke`; Windows CI smoke as a gate with a removal fallback; no `dxcompiler.dll`.

children: 0 launched / 0 reported.
