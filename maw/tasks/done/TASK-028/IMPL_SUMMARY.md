# IMPL_SUMMARY — TASK-028: release CI for Linux and Windows

Pre-flight: PASS. Every plan presupposition was checked against the code: the composite `.github/actions/setup`,
`.cargo/config.toml` (Windows-only linker section), `tools/fetch_assets.py` (`REPO`/`THIRD_PARTY`/`MANIFEST`
:26-28, `load_manifest` :259, `sha256_file` :271, `check` :338, the licence-in-files schema rule, importable),
`src/main.rs` (`flag_value` :39, `--settings-id`, preflight `missing_files`, MainMenu when there is no seed/bench),
`spawn_main_menu` at `src/menu/screens.rs:39` with `bevy::prelude::*`, bevy_render 0.19.1
`info!("{:?}", adapter_info)` at `renderer/mod.rs:287`, and the README anchors.

Branch `infra/release-ci`, commits `652ed50` (code) and `4d8a15e` (ADR + README), pushed to origin. Main was not
touched.

## 1. What was implemented

| File | Lines | What |
|---|---|---|
| `src/menu/screens.rs` | +1 | `info!("main menu ready");` as the first line of `spawn_main_menu` |
| `tools/package_release.py` | +258 (new) | `package` / `verify` / `smoke`, following plan Step 2. Stdlib only; imports `MANIFEST, ManifestError, check, load_manifest, sha256_bytes, sha256_file` from `fetch_assets`. Product failure exits 1, `GATE BROKEN` exits 2, `verify` collects every problem |
| `.github/workflows/release.yml` | +113 (new) | the plan's YAML verbatim, plus one comment saying the paths filter does not apply to tags |
| `docs/decisions/ADR-002-release-ci.md` | +96 (new) | Контекст / Решение / Альтернативы / Обновление. It records every decision in plan §3, the release procedure and the first run's result |
| `README.md` | +12 / -6 | "Запуск": where to download, how to run, Windows and Linux requirements. The stale "assets from the latest release" paragraph now says fetch_assets + licences + manifest in the zip + "Лицензия на код игры пока не выбрана". "Что проверяет CI": intro sentence and a new `release` row |

## 2. Deviations from the plan

- None in the product files. The Windows CI smoke went green on its first run, so it stays as a gate. No fallback
  was needed (no `link.exe`, no crt-static drop, no `--allow`).
- F3b deletes `car-kit/police.glb` (the first `.glb` of `car-kit`). The plan says only "one listed file".
- Extra evidence not in the plan: an F3f control run and a local boot of the CI Windows artifact (both in §3).
- Step 7 (rc tag and release) is **pending on TASK-038**, per the orchestrator note. Main `sim gates` is red on
  `a_left_car_seed_1` and `dummy_street_seed_1` (main run 36276095952). The branch run fails on the same two
  tests. The tag `v0.1.0-rc1` was not pushed. `decision` is logged.
- Finding for QA Step 7.4: `gh run download` auto-extracts an `archive: false` zip artifact into a folder, so
  `verify` on a CI zip needs `gh release download` (a release asset), as Step 7.4 already says.

## 3. Test results

### GitHub runners (branch push, commit 652ed50)
- **release**: https://github.com/pockerhead/MAW-make-GTA/actions/runs/36277750356. Status: success.
  `release-gates` and `publish` were skipped, as expected on a branch.
  - Linux (ubuntu-24.04): cold build 15m06s. `package: OK gta-like-sha-652ed502-linux-x86_64.zip: 98 entries
    (exe + 97 assets), 54.4 MB ... licences for 9 packs, no dynamic imports`.
    - Menu smoke 30 s: adapter `llvmpipe (LLVM 20.1.2, 256 bits)`, `main menu ready`, 0 ERROR/panic lines,
      5 WARN.
    - `--seed 1` smoke 60 s: 0 ERROR/panic lines, 5 WARN.
  - Windows (windows-2025, `+crt-static`, `rust-lld.exe`): cold build 31m02s. `package: OK
    gta-like-sha-652ed502-windows-x86_64.zip: 98 entries, 37.4 MB ... no dynamic imports`.
    - Smoke 30 s (dx12): adapter `Microsoft Basic Render Driver`, `main menu ready`, 0 ERROR/panic lines.
  - Artifacts: `gta-like-sha-652ed502-linux-x86_64.zip` (54404723 B) and
    `gta-like-sha-652ed502-windows-x86_64.zip` (37379677 B).
  - rust-cache keys: `v0-rust-release-Linux-x64-a972f308-670c1f85` and
    `v0-rust-release-Windows_NT-x64-d05cb9d4-670c1f85`. The job-level RUSTFLAGS env is visible in the setup step.
  - Logs: `scratch/ci_run1_linux.log`, `scratch/ci_run1_windows.log`.
- The other workflows on the branch: clippy (36277750277), client gates (36277750274), citygen gates
  (36277750310) and repo checks (36277750352) are all success. sim gates (36277750347) failed on the two
  TASK-038 traffic tests, the same as main.
- **CI Windows exe on this host** (downloaded artifact):
  - `dynamic_imports(exe, windows=True) == []`.
  - The exe contains non-CRT `api-ms-win-core-synch-l1-2-0` and `api-ms-win-core-winrt-l1-1-0`. This confirms
    the regex must stay anchored on `api-ms-win-crt-`.
  - `smoke` 20 s: GREEN, `main menu ready`, 0 ERROR.

### Local flip-RED (Windows host)
Commands and outputs are in `scratch/flips.md`. The scripts `scratch/flips_f1_f2.py` and `scratch/flips_f3.py`
can be re-run; both exit 0. Every flip gave the predicted exit code and message class.
- F1a: fast debug exe → 1, `bevy_dylib.dll, std-0cebe7c42cd80226.dll`.
- F1b: fake ELF → 1, `libbevy_dylib.so, libstd-….so`, no CRT line.
- F1c: local release exe → 1, `VCRUNTIME140.dll` + 6 `api-ms-win-crt-*`, no dylib line.
- GREEN: fixture zip → 0.
- F2a: extra.dll → `unexpected:`.
- F2b: inter licence dropped → `missing:` and `licence missing: inter`.
- F2c: `sha256 world/city.ron`.
- F2c2: `sha256 third_party/car-kit/police.glb`. This is the manifest branch.
- F2d: `exec bit: mode 0o600`.
- F2e: verify's own CRT scan → RED.
- F2f: missing zip → exit 2, `GATE BROKEN`.
- The GREEN zip was still GREEN after all F2 flips.
- F3a: menu smoke 3/3 GREEN (RTX 4070 Ti, `main menu ready`).
- F3b: pack file deleted → exit code 1 after 1.0 s (preflight "missing third-party asset").
- F3c: exe-only dir → exit code 1 (`assets\world/render.ron` not found).
- F3d: unknown marker → `expect:`.
- F3e: player GLB truncated, `--seed 1` → `error line: ... ERROR bevy_asset::server: Failed to load asset
  'third_party/mini-characters/character-male-a.glb'`. Restored → 60 s GREEN.
- F3f: `CARGO_MANIFEST_DIR=repo` in the caller with the exe-only dir → still RED.
  - Control: the same dir launched directly with that variable in the child env stayed alive and logged
    `main menu ready`. So the env strip is what the flip proves.
- No `gta_like.exe` process was left running.

### Other local checks
- `python tools/qa/tree_check.py`: passed.
- `python tools/qa/font_check.py`: 0 missing glyphs.
- `python tools/package_release.py --help` and each subcommand's `--help` work.
- Local `cargo build --release --locked -p gta_like --bin gta_like -j 2`: OK (19m10s).
- Clippy and `cargo test -p gta_like --bin gta_like` were proven on the runner (the clippy and client gates
  workflows are green on 652ed50) rather than locally, per the orchestrator note: the parallel TASK-038 work
  needs local cargo kept to a minimum.
- `cargo test -p gta_sim -p citygen`: citygen is green on the runner. gta_sim is red only on the TASK-038 tests.
  This task does not touch gta_sim.

## 4. How to verify manually

1. Open run 36277750356, download both artifacts, unpack each and run `gta_like[.exe]`. The main menu appears,
   and a console window with logs opens next to it on Windows.
2. After TASK-038 merges and main is green, run Step 7:
   - `git tag -a v0.1.0-rc1 -m "TASK-028 release CI proof" <infra/release-ci HEAD>`
   - `git push origin v0.1.0-rc1`
   - `gh run watch` the release run. Expect the gates job and a pre-release with two zips.
   - `gh release download v0.1.0-rc1 -p "*windows*"`, then `python tools/package_release.py verify <zip>` from a
     clean checkout at the tag. Unpack it outside the repo, run `smoke` with the menu expects, and start it once
     by hand.
3. `workflow_dispatch` shows up only once `release.yml` is on main.

## Remaining acceptance items
- AC4 (a real release and its link) and the tag half of AC1/AC3 (release-gates green, GitHub Release with two
  zips): **tag pending on TASK-038**.
- The Windows zip booted locally from the downloaded artifact is a partial AC2. QA does it again on the release
  asset.

children: 0 launched / 0 reported.
