# PLAN — TASK-028: release CI for Linux and Windows

Cost of error: a broken release is mostly loud (red run, zip that does not start), but three failures are
silent and get the proof layer: a release exe that dynamically links `bevy_dylib`/`std-*.dll` (works on the
builder, dies on a user machine), a zip that is missing licence files or assets (boots, then errors in some
scene), and a boot smoke that reads the checkout's `assets/` and not the zip's (green for the wrong reason).
Everything visual (does the Windows menu look right) goes to QA's local run of the downloaded zip.

## 1. Understanding

### Existing CI (TASK-029)
- `.github/actions/setup/action.yml:1-39` is a composite action: `dtolnay/rust-toolchain@1.95.0` (+clippy),
  optional apt `libasound2-dev libudev-dev libwayland-dev libxkbcommon-dev` (`linux-deps`, only `if: inputs.linux-deps == 'true'`),
  `Swatinem/rust-cache@v2` (`cache-on-failure: true`, no explicit key, so the key is `v0-rust-<GITHUB_JOB>-<os>-...`),
  `actions/setup-python@v7` 3.12, and with `assets: "true"`: `actions/cache@v6` over `assets/third_party/*` minus
  `manifest.ron`, keyed on the manifest hash, then `python tools/fetch_assets.py` and `python tools/fetch_assets.py --check`.
  All run steps use `shell: bash`, so the action works on a Windows runner too (Git Bash); the apt step is skipped there.
- Five workflows (`clippy.yml`, `sim-gates.yml`, `citygen-gates.yml`, `client-gates.yml`, `repo-checks.yml`), each with a
  unique job id (`clippy`, `sim`, `citygen`, `client`, `repo`) because rust-cache keys on `GITHUB_JOB` (TASK-029 N1),
  `permissions: contents: read`, triggers `push: branches-ignore: [media]` + `pull_request` + `workflow_dispatch`,
  `actions/checkout@v7`. Because they define only `branches-ignore`, **none of them runs on a tag push**
  (GitHub: "If you define only tags/tags-ignore or only branches/branches-ignore, the workflow won't run for events
  affecting the undefined Git ref").
- `client-gates.yml` already links the whole client (`cargo test -p gta_like --bin gta_like`) on ubuntu with the four apt
  packages, so the Linux link-time deps are known good. Live cache list (`gh cache list`): ~1.7 GB per generation
  (client 686 MB, clippy 400 MB, sim 350 MB, citygen 165 MB, repo 136 MB), 10 GB repo cap.
- `gh` on this host is `SG-all` with **pull-only** rights (`gh api repos/pockerhead/MAW-make-GTA -q .permissions` →
  `push:false`). Agents can read runs/releases and push over SSH as pockerhead, but cannot `gh workflow run` or create
  releases. The repo is PUBLIC (free Actions minutes); it has no tags and no releases yet. Main uses squash merges.

### Build facts
- `Cargo.toml`: `fast = ["bevy/dynamic_linking"]` is opt-in; bevy 0.19.1 `default` has no `dynamic_linking`
  (PREMISE_CHALLENGE §2). No `[profile.release]` section: release is Cargo's default release profile, the one the owner
  uses for `cargo run --release`. `[patch.crates-io] bevy_dylib = { path = "vendor/bevy_dylib-0.19.1" }` only matters
  when `fast` is on.
- `.cargo/config.toml:1-2`: only `[target.x86_64-pc-windows-msvc] linker = "rust-lld.exe"`. No Linux section, so Linux
  links with the default toolchain linker (as in client-gates today).
- `rust-version = "1.95.0"` (workspace) = the composite action's pin. No `rust-toolchain.toml`.
- `.gitattributes`: `* text=auto eol=lf`, binaries marked binary, so checkout bytes are the same on both runners.

### Runtime data root (why "exe next to assets/" is the right layout)
- `src/main.rs:194` `ConfigRoot(FileAssetReader::get_base_path().join("assets"))`; bevy_asset 0.19.1
  `src/io/file/mod.rs:19-29`: `BEVY_ASSET_ROOT` env, else `CARGO_MANIFEST_DIR` env, else `current_exe().parent()`.
  A binary started outside cargo resolves `<exe dir>/assets`. **Carry-over from PREMISE_CHALLENGE:** the boot smoke runs
  the binary from the UNPACKED zip with `CARGO_MANIFEST_DIR` and `BEVY_ASSET_ROOT` removed from its env.
- `src/main.rs:170-303`: `DefaultPlugins` are added, then every RON config is loaded and `preflight` (`:64-168`) checks
  configs, manifest listing and `manifest.missing_files(root)` (`:155`) — any missing third-party file means
  `eprintln!` + `AppExit::error()` before `app.run()`. So "process still alive" already implies "every config parsed and
  every manifest file exists next to the exe".
- Without `--seed` the game starts in `GameState::MainMenu` (`main.rs:216-218`); `MenuPlugin` runs
  `screens::spawn_main_menu` on `OnEnter(GameState::MainMenu)` (`src/menu/mod.rs:52`, `src/menu/screens.rs:39`).
  No log line marks it today. With `--seed N` it goes `Loading → Playing` (`crates/gta_sim/src/world/city.rs:131`),
  where GLBs, sounds and the facade shader load.
- bevy_render 0.19.1 `src/renderer/mod.rs:286-288`: panics with "Unable to find a GPU!" if no adapter, else
  `info!("{:?}", adapter_info)` (a line containing `AdapterInfo {`); a CPU adapter adds a `warn!`. `WGPU_BACKEND` is read
  via `Backends::from_env()` (`src/settings.rs:84`). bevy_audio 0.19.1 logs "No audio device found." at `warn!`
  (`src/audio_output.rs:24`), so a runner with no sound card does not emit ERROR.
- Asset load failures are `error!` in `bevy_asset::server` (`src/server/mod.rs:593,684,976,992,...`).

### Third-party assets and licences
- `tools/fetch_assets.py`: `MANIFEST` (`:28`), `load_manifest(path)` (`:259`), `check(doc)` (`:338`) returns problems,
  including "unexpected entry" for anything in `assets/third_party/` that is not a listed pack. Every pack's
  `license_file` must be listed in its `files` (schema, `:178-235`), so installed packs carry their `License.txt` /
  OFL file. The manifest itself (URLs, versions, licences) is the attribution record.
- Tracked assets: `git ls-files assets` = 26 files (RON configs, `shaders/*.wgsl`, `third_party/manifest.ron`).

### Dynamic-link probe (evidence for the "no fast in release" proof)
`scratch/dylib_scan_probe.py` + `scratch/dylib_scan_probe.txt`: a byte scan for
`(?:lib)?(?:bevy_dylib|std-[0-9a-f]{16})\.(?:dll|so)` finds **nothing** in the local static `target/release/gta_like.exe`
and finds `bevy_dylib.dll` + `std-0cebe7c42cd80226.dll` in the local `fast` build `target/debug/gta_like.exe`
(its import table names). The same names are what an ELF `DT_NEEDED` would carry (`libbevy_dylib.so`, `libstd-<hash>.so`).

## 2. Approach

One new workflow `.github/workflows/release.yml` with four jobs, one small Python tool
`tools/package_release.py` (package / verify / smoke), one `info!` marker in the menu, an ADR and README updates.
The composite setup action is reused unchanged.

Decisions (each also in `log.jsonl`):

| Topic | Decision | Rejected alternative |
|---|---|---|
| Triggers | `push: tags: ['v*']` → build + gates + publish; `push: branches: ['**']` with a `paths` filter on release-relevant files → build + package + smoke + artifacts (no publish); `workflow_dispatch` → same as branch push. GitHub: "Path filters are not evaluated for pushes of tags", so tags always run. | Every main push (30+ min Windows build on docs commits); tag-only (no pre-merge run, and dispatch only works after merge). |
| Runners | `ubuntu-24.04` and `windows-2025`, pinned. Linux release needs glibc >= 2.39 (documented in README). | `*-latest` (silently moves; ubuntu-latest → 26.04 raises the glibc floor); ubuntu-22.04 (lower glibc, but next in line for retirement). |
| Toolchain | composite action's `dtolnay/rust-toolchain@1.95.0` = `rust-version`. | a second pin in release.yml. |
| Build | `cargo build --release --locked -p gta_like --bin gta_like`, no `--features`. Default release profile, same as the owner's `cargo run --release`. | LTO / `strip` / a custom profile (not asked, changes what the owner measures locally). |
| Linker | keep `.cargo/config.toml` `rust-lld.exe` for Windows (same link as local release builds; rustc sets up the MSVC lib paths itself, which is why it works locally outside a VS prompt). If the first Windows run fails to link: `CARGO_TARGET_X86_64_PC_WINDOWS_MSVC_LINKER: link.exe` in the Windows matrix env only, logged as a dead_end. Linux: config has no Linux section, nothing to do. | switching to `link.exe` up front. |
| Caching | `Swatinem/rust-cache@v2` via the composite, unique job id `release` (key differs per OS via `runner.os`) and `release-gates`; assets via the composite's `actions/cache`. No sccache. Tag runs can restore caches of the default branch and their own ref only, so the first tag run may be cold; accepted. | sccache + GHA backend (more moving parts, no gain for a rare release). |
| No-`fast` proof | byte scan of the exe for dynamic Rust/Bevy library names (probe above) inside `package` and `verify`, plus a zip allowlist (only the exe and `assets/**`, so no `.dll`/`.so` can ride along), plus on Linux the boot smoke from the unpacked zip (a dylib build would not even start without `LD_LIBRARY_PATH`). | `dumpbin` (not on PATH, needs vswhere), `readelf` (Linux only), `cargo tree -i bevy_dylib` (checks the graph, not the binary). |
| Zip layout | `gta-like-<version>-<platform>-x86_64.zip` containing one top folder of the same name with `gta_like[.exe]` and `assets/` (tracked assets + installed packs incl. each pack's licence file + `manifest.ron`). `<version>` = tag name on tags, `sha-<8 hex>` otherwise; `<platform>` = `linux` / `windows`. Exe entry gets unix mode 0755. | tarball for Linux (task asks for zips); flat zip (tarbomb). |
| Linux boot smoke | `xvfb-run` + mesa lavapipe (`WGPU_BACKEND=vulkan`), two runs from the `unzip`-ed folder, cwd a fresh temp dir outside the checkout, `CARGO_MANIFEST_DIR`/`BEVY_ASSET_ROOT` removed: (a) no args, 30 s, must log `AdapterInfo {` and `main menu ready`; (b) `--seed 1`, 60 s, must log `AdapterInfo {`. Both: process alive at the deadline, zero lines with a tracing `ERROR` level or `panicked`. A new one-line `info!("main menu ready")` in `spawn_main_menu` makes (a) distinguish "menu state entered" from "hung before the first update", which is still alive. | bare liveness (hang = green); a `--smoke`/`--version` CLI mode in the game (product code only CI needs). |
| Windows boot | no smoke on the runner (dx12 WARP in a service session is unproven; a flaky gate would block releases). QA runs the same `smoke` subcommand locally on the zip downloaded from the release. | `continue-on-error` WARP smoke (not a gate). |
| Headless tests | job `release-gates` (ubuntu-24.04, `assets: "true"`) runs `cargo test --locked --no-fail-fast -p gta_sim -p citygen`, only on tags (branch pushes already run sim/citygen gates; tag pushes do not). `publish` needs it. | running it on every release.yml trigger (duplicates sim/citygen gates on branch pushes). |
| Publish | job `publish` (tags only, `needs: [build, release-gates]`, `permissions: contents: write` on this job only) downloads both zips and runs the preinstalled `gh release create` with `GH_TOKEN: ${{ github.token }}`, `--generate-notes`, `--prerelease` when the tag contains `-`. | `softprops/action-gh-release` (third-party code holding a write token); uploading from each build job (half-published release when one OS fails). |
| Secrets | workflow-level `permissions: contents: read`; only `GITHUB_TOKEN`; no `pull_request_target`; ref/tag names reach shell only through env vars (`$GITHUB_REF_NAME`), never `${{ }}` inside `run:` (GitHub security hardening guide, script injection section). | — |

Sources: GitHub workflow syntax (path filters vs tags, branch/tag filter rule, `contents: write` creates a release),
[docs.github.com/.../workflow-syntax](https://docs.github.com/en/actions/reference/workflows-and-actions/workflow-syntax);
events (`workflow_dispatch` "will only trigger a workflow run if the workflow file exists on the default branch"),
[docs.github.com/.../events-that-trigger-workflows](https://docs.github.com/en/actions/reference/workflows-and-actions/events-that-trigger-workflows);
[security hardening for GitHub Actions](https://docs.github.com/en/actions/security-for-github-actions/security-guides/security-hardening-for-github-actions);
`actions/upload-artifact@v7.0.1` `action.yml` (`archive: false` uploads one file as-is, name = file name) and
`actions/download-artifact@v8.0.1` (`skip-decompress`, content-type based decompression; README "What's new");
runner labels from `actions/runner-images` README (ubuntu-24.04, windows-2025 = windows-latest with VS 2026);
Bevy's own CI installs `xvfb ... mesa-vulkan-drivers` to run examples under `xvfb-run`.
Action versions checked today via `gh release view`: checkout v7.0.1, cache v6.1.0, upload-artifact v7.0.1,
download-artifact v8.0.1, rust-cache v2.9.2.

## 3. Steps

### Step 1 — `src/menu/screens.rs`: menu marker
In `spawn_main_menu` (`:39`), first line of the body: `info!("main menu ready");` (`info!` is in `bevy::prelude`,
already imported at `:10-14`). Nothing else changes.
→ check: `cargo clippy --workspace --all-targets -- -D warnings` clean; `cargo run --release` shows the line in the
console when the menu appears.

### Step 2 — `tools/package_release.py` (new, target < 250 lines, Python 3.10-compatible)
Imports `load_manifest`, `check`, `MANIFEST`, `sha256_file` from `fetch_assets` (same directory; insert
`Path(__file__).parent` into `sys.path`). `REPO = Path(__file__).resolve().parents[1]`. Three subcommands; each prints
what it checked and exits 1 with a message naming the failing check.

- `DYLIB = re.compile(rb"(?:lib)?(?:bevy_dylib|std-[0-9a-f]{16})\.(?:dll|so)")`;
  `dylib_imports(exe_bytes) -> sorted set of matches`.
- `expected_assets() -> list[str]`: `git ls-files -z assets` (run in `REPO`) ∪ `third_party/<pack>/<file.path>` for every
  file of every manifest pack, as posix paths relative to `assets/`, sorted. Fail if `git` fails.

`package --exe PATH --name NAME --out DIR`
1. `problems = check(load_manifest(MANIFEST))`; non-empty → fail "third-party packs do not match the manifest: ...;
   run tools/fetch_assets.py".
2. `dylib_imports(exe)` non-empty → fail "exe links dynamically: <names> (built with `fast`?)".
3. Write `DIR/NAME.zip` (`ZIP_DEFLATED`): entry `NAME/<exe basename>` via a `ZipInfo` with
   `create_system = 3`, `external_attr = (0o100755 << 16)`; then `NAME/assets/<rel>` for each `expected_assets()` path,
   read from `REPO/assets/<rel>`.
4. Call `verify` on the written zip and print its summary (entry count, size).

`verify ZIP`
1. All entries start with one top folder `NAME/`; the exe entry is `NAME/gta_like` or `NAME/gta_like.exe`
   (exactly one of them).
2. Set of entries == `{exe} ∪ {NAME/assets/<rel> for rel in expected_assets()}` — report missing and unexpected
   separately (this is the allowlist: no `.dll`/`.so`/`.pdb` can ship).
3. Every assets entry's sha256 == sha256 of `REPO/assets/<rel>` (checkout packs were already checked against the manifest
   sha256 by `fetch_assets.py --check` in the setup action and by step 1 of `package`).
4. Every pack's `license_file` is present (explicit message "licence missing: <pack>").
5. The exe entry: `dylib_imports` empty; for the extension-less (Linux) exe, `(external_attr >> 16) & 0o111 != 0`.

`smoke DIR --seconds N --log FILE [--expect TEXT]... [--allow REGEX]... [-- GAME_ARGS...]`
1. `DIR` is the unpacked top folder; the exe is `DIR/gta_like` or `DIR/gta_like.exe` (fail if neither).
2. `env = os.environ` minus `CARGO_MANIFEST_DIR` and `BEVY_ASSET_ROOT`; `cwd = tempfile.mkdtemp()` (not the exe dir,
   not the checkout); stdout+stderr → `FILE`.
3. Poll every 0.5 s until `N` seconds: if the process exits early → fail "exited after X s with code C" + last 40 log
   lines. At the deadline `kill()` it and `wait()`.
4. Strip ANSI (`\x1b\[[0-9;]*m`) from the log. Fail on any line matching `\bERROR\b` (tracing level column) or
   `panicked` that no `--allow` regex matches; fail on any `--expect` text that appears on no line. Print the matched
   expect lines and the count of WARN lines.
Liveness class: "alive at N s". State class: `main menu ready` (MainMenu `OnEnter` ran) and `AdapterInfo {` (renderer
initialised). Correctness class: zero ERROR/panic lines over the window (asset load failures are `error!` in
bevy_asset). The claim is NOT "the menu looks right": that is QA/owner.

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
jobs:
  release:            # unique job id: rust-cache key (TASK-029 N1); matrix differs by runner.os
    strategy:
      fail-fast: false
      matrix:
        include:
          - { os: ubuntu-24.04, platform: linux, exe: gta_like }
          - { os: windows-2025, platform: windows, exe: gta_like.exe }
    runs-on: ${{ matrix.os }}
    timeout-minutes: 90
    steps:
      - uses: actions/checkout@v7
      - uses: ./.github/actions/setup
        with:
          linux-deps: ${{ matrix.platform == 'linux' }}
          assets: "true"
      - name: name
        shell: bash
        run: |
          if [[ "$GITHUB_REF" == refs/tags/v* ]]; then v="$GITHUB_REF_NAME"; else v="sha-${GITHUB_SHA::8}"; fi
          echo "NAME=gta-like-$v-${{ matrix.platform }}-x86_64" >> "$GITHUB_ENV"
      - name: build (release, no features)
        shell: bash
        run: cargo build --release --locked -p gta_like --bin gta_like
      - name: package + verify
        shell: bash
        run: python tools/package_release.py package --exe "target/release/${{ matrix.exe }}" --name "$NAME" --out dist
      - name: boot smoke (unpacked zip, xvfb + lavapipe)
        if: matrix.platform == 'linux'
        shell: bash
        env:
          WGPU_BACKEND: vulkan
        run: |
          sudo apt-get install --no-install-recommends -y xvfb mesa-vulkan-drivers libvulkan1 libxkbcommon-x11-0 libxcursor1 libxrandr2 libxi6 libx11-xcb1
          mkdir -p "$RUNNER_TEMP/unpacked" && unzip -q "dist/$NAME.zip" -d "$RUNNER_TEMP/unpacked"
          xvfb-run -a -s "-screen 0 1280x720x24" python tools/package_release.py smoke "$RUNNER_TEMP/unpacked/$NAME" --seconds 30 --log smoke-menu.log --expect "AdapterInfo {" --expect "main menu ready"
          xvfb-run -a -s "-screen 0 1280x720x24" python tools/package_release.py smoke "$RUNNER_TEMP/unpacked/$NAME" --seconds 60 --log smoke-seed.log --expect "AdapterInfo {" -- --seed 1
      - if: failure() && matrix.platform == 'linux'
        uses: actions/upload-artifact@v7
        with: { name: smoke-logs, path: "smoke-*.log", if-no-files-found: ignore }
      - uses: actions/upload-artifact@v7
        with:
          path: dist/${{ env.NAME }}.zip
          archive: false
          if-no-files-found: error
  release-gates:
    if: startsWith(github.ref, 'refs/tags/v')
    runs-on: ubuntu-24.04
    timeout-minutes: 60
    steps:
      - uses: actions/checkout@v7
      - uses: ./.github/actions/setup
        with: { assets: "true" }
      - shell: bash
        run: cargo test --locked --no-fail-fast -p gta_sim -p citygen
  publish:
    if: startsWith(github.ref, 'refs/tags/v')
    needs: [release, release-gates]
    runs-on: ubuntu-24.04
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
          ls -l dist
          test "$(ls dist/*.zip | wc -l)" -eq 2
          pre=(); [[ "$GITHUB_REF_NAME" == *-* ]] && pre=(--prerelease)
          gh release create "$GITHUB_REF_NAME" dist/*.zip --verify-tag --generate-notes "${pre[@]}"
```
Notes for the implementer: the YAML above is the design, verify each key against the action's `action.yml` of the
pinned major (done for upload v7 / download v8). If `download-artifact` with `skip-decompress` yields a nested file
layout, adjust the `ls`/glob, not the checks. `linux-deps` receives `true`/`false` from the expression; the composite
compares with `'true'`. `smoke-logs` upload only on failure. Do not add `pull_request` (fork PRs cannot pass these
paths usefully, and branch pushes already cover it).
→ check: first push of the branch starts `release` for both OSes (paths filter hits `release.yml`); the `name`, `build`,
`package + verify`, Linux smoke steps are green; the run page lists two artifacts named `gta-like-sha-<8>-linux-x86_64.zip`
and `...-windows-x86_64.zip`; each rust-cache log line shows a key starting `v0-rust-release-Linux` / `v0-rust-release-Windows`.

### Step 4 — `docs/decisions/ADR-002-release-ci.md` (new, Russian like ADR-001; sections Контекст / Решение / Альтернативы / Обновление)
Records the table in §2: triggers, runners + glibc floor, toolchain pin, build command and profile, linker, cache,
no-`fast` proof, zip naming/layout, Linux smoke and why Windows is QA-local, gates-on-tags, publish via gh + permissions,
and the "cut a release" procedure (`git tag -a vX.Y.Z[-rcN]` on a main commit, `git push origin <tag>`; a failed tag
is never moved or deleted, the next rc number is used).

### Step 5 — `README.md`
- "Запуск" (`:212-238`): first paragraph gains where to download: releases page
  `https://github.com/pockerhead/MAW-make-GTA/releases`, one zip per OS, unpack and run `gta_like.exe` / `./gta_like`
  (the `assets/` folder must stay next to it). Linux needs glibc >= 2.39 (Ubuntu 24.04+), `libasound2`, `libudev1`,
  a Vulkan driver, X11 or Wayland. Replace the stale paragraph `:236-238` ("папку `assets/` берём из последнего релиза")
  with the truth: from source, assets come from `python tools/fetch_assets.py` (already said at `:214-219`), release zips
  carry them.
- "Что проверяет CI" (`:175-189`): one table row for `release` (tag `v*` → Linux + Windows zips, boot smoke on Linux,
  `cargo test -p gta_sim -p citygen`, GitHub Release; on release-file changes → artifacts only). Fix "Пять workflow" to
  say five on every push plus release. No new badge (a tag-only workflow badge is mostly "no status").
→ check: `python tools/qa/font_check.py`-style repo checks are untouched; README renders (no broken table).

### Step 6 — local flip-RED on this Windows host (implementer; record commands and outputs in `scratch/flips.md`)
Build once: `cargo build --release --locked -p gta_like --bin gta_like`, and a `fast` build `cargo build --features fast`
(it already exists as `target/debug/gta_like.exe`, rebuild if stale). Output dir `target/release-dist` (git-ignored).
- F1 dylib: `package --exe target/debug/gta_like.exe ...` → RED naming `bevy_dylib.dll` and `std-*.dll`;
  `package --exe target/release/gta_like.exe ...` → GREEN.
- F2 zip allowlist / licence: copy the GREEN zip, add a stray `NAME/extra.dll` entry with `zipfile` (append mode) →
  `verify` RED "unexpected"; a copy without `NAME/assets/third_party/inter/<licence file>` (rewrite without that entry)
  → RED "licence missing" + "missing"; a copy with one RON's bytes changed → RED sha256. Original → GREEN.
- F3 smoke (Windows, real GPU): unpack the GREEN zip (`python -m zipfile -e`) to a temp dir outside the repo.
  (a) `smoke <dir> --seconds 20 --expect "AdapterInfo {" --expect "main menu ready" --log ...` → GREEN;
  (b) delete one listed file under `<dir>/assets/third_party/` → RED "exited ... code 1" (preflight);
  (c) copy only the exe into another empty dir and smoke it → RED (proves exe-relative resolution: same exe, no
  `assets/` next to it);
  (d) `--expect "no such marker"` → RED (expect is enforced);
  (e) truncate one GLB listed for the city (e.g. a `car-kit` model) and run with `-- --seed 1 --seconds 60` → RED on an
  `ERROR bevy_asset` line (the error scan is live); restore → GREEN.
  Use `--settings-id gta_like_smoke` in the game args of local runs so the owner's settings file is not read or written.
- Linux-side flips are not repeated in CI (each costs a full run); the same script code is exercised by F1-F3, and the
  Linux-specific parts (exec bit, `libbevy_dylib.so` spelling) are covered by the regex/`external_attr` checks in F1/F2's
  code paths. Say so in the summary.

### Step 7 — real run proof (QA, after the branch run of Step 3 is green; this is in scope per the orchestrator)
1. Branch push run: record its URL, both artifact names, the smoke step's printed marker lines (menu + seed) and the
   adapter name (expect `llvmpipe`).
2. Tag: `git tag -a v0.1.0-rc1 -m "TASK-028 release CI proof" <branch HEAD>` and `git push origin v0.1.0-rc1` (SSH as
   pockerhead; gh here is read-only). Watch with `gh run list --repo pockerhead/MAW-make-GTA --workflow release.yml --limit 3`
   and `gh run watch <id> --repo pockerhead/MAW-make-GTA`.
3. `gh release view v0.1.0-rc1 --repo pockerhead/MAW-make-GTA --json url,isPrerelease,assets` → two zips, prerelease true.
   Record the run URL and the release URL in the QA report and IMPL/QA summary.
4. Windows: `gh release download v0.1.0-rc1 --repo pockerhead/MAW-make-GTA -p "*windows*" -D <temp>` (public repo, read
   access suffices), unpack with PowerShell `Expand-Archive` to a folder outside the repo, run the Step 2 `smoke` on it
   (menu marker GREEN), then start `gta_like.exe` by hand once and confirm the main menu shows (screenshot into scratch).
5. If the tag run fails: fix on the branch, push, and use `v0.1.0-rc2`; never move or delete a pushed tag.
6. Post-merge (orchestrator/owner): `release` also runs on the merge commit (paths filter hits); the "Run workflow" button
   exists only once `release.yml` is on main, so the `workflow_dispatch` path is proven by the owner's click (or is
   accepted as identical to the branch-push path, which runs the same non-tag steps).

## 4. Risk areas
- **R1 lavapipe/xvfb under Bevy 0.19** (unknown until the first run): no adapter → panic (caught, RED); environment
  ERROR lines unrelated to our code (winit/wgpu on Xvfb) → a false RED. Fix by an `--allow` regex naming the exact
  target+message with a one-line workflow comment why, never by dropping the ERROR scan. If lavapipe cannot run the game
  at all, fall back to `WGPU_BACKEND=gl` (llvmpipe, needs `libgl1-mesa-dri libegl1`) before touching the gate.
- **R2 `rust-lld.exe` on windows-2025 (VS 2026)**: if rustc fails to find the MSVC libs, link errors in the Windows build.
  Fallback in §2 (env override in the Windows matrix entry only); log a dead_end.
- **R3 cold builds**: the first Windows release build is cold (maybe 30-45 min); `timeout-minutes: 90`. Tag runs restore
  only default-branch and own-ref caches. Cache cap 10 GB: two more rust caches (~1-2.5 GB each) push older generations
  out (LRU); acceptable, watch for sim/client cache misses after the first release.
- **R4 download-artifact v8 decompression**: it decides by content type; `skip-decompress: true` keeps our zips intact.
  The publish step's `test ... -eq 2` catches a surprise.
- **R5 smoke timing**: menu marker is logged by the `OnEnter` system, well before 30 s even on lavapipe; the seed run is
  liveness + error scan over 60 s, not "city fully loaded". Do not raise the claim in summaries.
- **R6 tag on a squash-merged branch**: the rc tag points at a branch commit that will not be on main after the squash
  merge. Fine for a pre-release proof; the owner cuts `v0.1.0` from main.
- **R7 Linux runtime deps on user machines**: dynamic `libasound.so.2`, `libudev.so.1`, dlopen'd X11/Wayland/Vulkan.
  Documented in README; not gated.
- **R8 script injection / token scope**: only `$GITHUB_REF_NAME`/`$GITHUB_SHA` via env in `run:`; `contents: write` only in
  `publish`; no third-party action receives the write token.
- **R9 `branches: ['**']` with `paths`**: a later task that changes only `src/**` does not build Windows until the next tag;
  a Windows-only compile break then shows at release time. Accepted (see Q2).

## 5. Open questions
- **Q1 (owner, non-blocking)**: the Windows exe is a console-subsystem app, so double-clicking opens a console window
  next to the game. `#![windows_subsystem = "windows"]` hides it but also hides preflight error messages. Plan keeps the
  console (errors visible); change later if the owner wants.
- **Q2 (orchestrator)**: should `release` build on every push to main (catches Windows-only compile breaks early, warms
  caches for tags) instead of the paths filter? Plan says no (cost vs rare releases); easy to flip later.
- **Q3 (orchestrator)**: the AGENTS.md post-merge rule says "all 5 workflows success"; with this task the merge commit
  also runs `release` (paths hit). Update the rule/CI line in AGENTS.md "Проект" during the post-task docs sync.
- **Q4 (owner, non-blocking)**: the repo has no licence for the game code itself; the zips carry only third-party
  licences + `manifest.ron`. Adding a LICENSE is the owner's call, out of scope here.

No new crates. No `Cargo.lock` change. No new tuning values (smoke durations are CI arguments, not game tuning).
children: 0 launched / 0 reported.
