# QA_REPORT — TASK-028: release CI for Linux and Windows

QA: claude opus, medium. Tree under test: `infra/release-ci` HEAD `a39f4d6` (main with the TASK-038 fix merged in).
Tag pushed by QA: `v0.1.0-rc1` -> `a39f4d6` (annotated tag object `939b1a5`).

## 0. Disconfirmation

Counter-example I went after: "the rc tag publishes a Windows zip that works on the runner but not on a user
machine: the exe still imports the dynamic MSVC CRT (or `bevy_dylib`/`std-*`) and the byte-regex scan misses it, or
the exe reads assets from somewhere other than its own `assets/`."
How I tested it, independently of the author's regex:
- Parsed the PE import table of the downloaded release exe with my own stdlib parser
  (`scratch/qa/pe_imports.py`). Imports: only OS DLLs (`kernel32`, `user32`, `ntdll`, `advapi32`, `combase`,
  `ole32`, `gdi32`, `setupapi`, `pdh`, `powrprof`, `uiautomationcore`, ..., plus `api-ms-win-core-synch-l1-2-0` and
  `api-ms-win-core-winrt-l1-1-0`). No `VCRUNTIME140`, no `api-ms-win-crt-*`, no delay-loads. Control: the same
  parser on the local dynamic-CRT `target/release/gta_like.exe` lists `VCRUNTIME140.dll` and six
  `api-ms-win-crt-*`, so the parser does see CRT imports when they exist (`scratch/qa/pe_imports.release.txt`).
- Parsed the Linux exe's ELF `DT_NEEDED` from inside the release zip (`scratch/qa/elf_needed.py`):
  `libwayland-client.so.0, libudev.so.1, libasound.so.2, libgcc_s.so.1, libm.so.6, libc.so.6, ld-linux`, max
  `GLIBC_2.39`, no `libbevy_dylib`/`libstd-*`. Entry mode `0o100755`.
- Booted the unpacked release exe from a temp cwd, and a copy with `assets/` deleted: the copy exits code 1 on
  `assets\world/render.ron` not found, so the exe reads its own folder.
**Did not hold.**

A weaker regex gap turned up on the way (Bug 1 below). It does not affect this release.

`log.jsonl` has no `dead_end` entries. I checked the `decision` refs that matter here against the code:
rustflags are set at job level (`release.yml:37-39`, and the job log shows the var in the rust-cache step env).
The pack sha comes from the manifest (`package_release.py:76-87`). There is no escape flag. The city marker is on
`OnTransition{Loading->Playing}` (`src/menu/mod.rs`). All four hold.

## 1. Environment

- Direct: GitHub Actions (the real release runs) plus this Windows 11 host (RTX 4070 Ti) for the downloaded zips.
  No docker, no mocks.
- Nothing is left running. The temp dir `%LOCALAPPDATA%\Temp\qa028_rel` (downloaded zips, unpacked copy) is removed
  at the end. No `gta_like` process was running before or after.
- Reproduce:
  ```
  gh run list --repo pockerhead/MAW-make-GTA --commit a39f4d698e1a338a4108f6dc1ae2e3ab45ce9b2a
  gh release view v0.1.0-rc1 --repo pockerhead/MAW-make-GTA --json url,isPrerelease,assets
  gh release download v0.1.0-rc1 --repo pockerhead/MAW-make-GTA -D <temp>
  python tools/package_release.py verify <temp>/gta-like-v0.1.0-rc1-{windows,linux}-x86_64.zip   # checkout at a39f4d6
  Expand-Archive <windows zip> <temp>\unpacked
  python tools/package_release.py smoke <unpacked>/gta-like-v0.1.0-rc1-windows-x86_64 --seconds 25 --log <log> --expect "AdapterInfo {" --expect "main menu ready"
  python maw/tasks/in_progress/TASK-028/scratch/qa/qa_verify_edges.py
  ```

## 2. Test results

### Branch CI on a39f4d6 (precondition from the orchestrator note)
All 5 test workflows are success: clippy 36291984044, sim gates 36291983992 (21m25s), citygen gates 36291984015,
client gates 36291984024, repo checks 36291983996. `release` did not run on a39f4d6. The merge touched no path in
the release `paths` filter (`git diff --name-only b93ac7b a39f4d6` has no hit), and that matches the design. The last
branch `release` run is 36280342827 on b93ac7b (success). The tag run below built a39f4d6 itself.

### Tag run (the real release proof)
https://github.com/pockerhead/MAW-make-GTA/actions/runs/36293015600 finished **success**. Job logs are saved in
`scratch/qa/job_*.log`.
| Job | Result | Evidence |
|---|---|---|
| release (ubuntu-24.04) | success | cold build 14m43s. `package: OK gta-like-v0.1.0-rc1-linux-x86_64.zip: 98 entries (exe + 97 assets), 54.5 MB ... no dynamic imports`. Menu smoke 30 s: adapter `llvmpipe (LLVM 20.1.2)`, `main menu ready`, 0 ERROR. Seed smoke 60 s: `city ready`, 0 ERROR |
| release (windows-2025, +crt-static) | success | build 29m56s. `package: OK ...windows-x86_64.zip: 98 entries, 37.5 MB ... no dynamic imports`. Smoke 30 s dx12: adapter `Microsoft Basic Render Driver`, `main menu ready`, 0 ERROR/panic, 3 WARN |
| release-gates | success | `cargo test --locked --no-fail-fast -p gta_sim -p citygen` really ran: 590 passed, 0 failed, 12 ignored across all test binaries, including `a_left_car_seed_1 ... ok` and `dummy_street_seed_1 ... ok` (the TASK-038 tests) |
| publish | success | download-artifact reported `Downloading raw file (non-zip)` for both (R6 layout OK). `test ... -eq 2` passed. `gh release create` printed the release URL |

Release: https://github.com/pockerhead/MAW-make-GTA/releases/tag/v0.1.0-rc1. `isPrerelease: true`,
`isDraft: false`, exactly two assets (`scratch/qa/release_view.json`):
- `gta-like-v0.1.0-rc1-linux-x86_64.zip`: 54474526 B, sha256 `8a3302b0...76ce4`
- `gta-like-v0.1.0-rc1-windows-x86_64.zip`: 37451173 B, sha256 `c13d621c...8182e`

The downloaded files hash to the same values as the digests the publish job logged.

### Downloaded release zips on this host (checkout clean at a39f4d6, `fetch_assets.py --check` green)
- `verify` on the Windows zip: rc 0. 98 entries, allowlist exact, sha256 match, licences for 9 packs, no dynamic
  imports.
- `verify` on the Linux zip: rc 0, same summary.
- Windows zip unpacked with PowerShell `Expand-Archive` outside the repo: 98 files, `gta_like.exe` (117.8 MB)
  next to `assets/`.
- `smoke` menu 25 s: GREEN. RTX 4070 Ti (Vulkan), `main menu ready`, 0 ERROR/panic, 1 WARN
  (`scratch/qa/rel_win_menu.log`).
- `smoke --seed 1` 60 s with `--expect "city ready"`: GREEN, 0 ERROR (`scratch/qa/rel_win_seed.log`). `city ready`
  comes 0.27 s after `AdapterInfo`. I checked this is real and not a premature marker: city generation is
  synchronous in `gta_sim/src/world/city.rs` (`next.set(GameState::Playing)` at the end of the generate system).
- Manual launch, the equivalent of a double-click: exe started from the unpacked folder (cwd = that folder) with
  `--settings-id .qa`, no BRP. The screenshot was taken with GDI `CopyFromScreen` of the game window
  (`scratch/qa/rel_win_manual_menu.png`). What I saw: the "GTA-like" window with the title, "Новая игра", the Seed
  field, the hint line "Enter — город с этим seed, пусто — случайный" and "Выход". The Cyrillic glyphs render. The
  console window with logs is behind it, as the README says. I passed `--settings-id .qa` on purpose: the project
  rule is that the owner's settings are never written. This is logged as a `decision`.

### My own flip-RED (independent of the author's scripts)
On the real release exe (a copy of the unpacked folder):
- F-Q1: `assets/third_party/mini-characters/character-male-a.glb` truncated to 100 bytes, `--seed 1`. Result: rc 1,
  `smoke: error line: ... ERROR bevy_asset::server: Failed to load asset 'third_party/mini-characters/character-male-a.glb'`
  (`scratch/qa/flip_glb_trunc.log`). The untouched original is GREEN (above).
- F-Q2: `assets/` deleted. Result: rc 1, `liveness: exited after 0.5 s with code 1` (`world/render.ron` not found).

On `verify` edge cases (`scratch/qa/qa_verify_edges.py` -> `.out.txt`, fixture zip rebuilt at a39f4d6):
| Case | Expected | Got |
|---|---|---|
| baseline fixture zip | GREEN | rc 0 |
| a directory entry `NAME/assets/` | RED | rc 1 `unexpected` |
| both `gta_like.exe` and `gta_like` | RED | rc 1 `exe entry` |
| exe containing `UCRTBASE.DLL` (upper case) | RED | rc 1, CRT line `UCRTBASE.DLL` |
| exe containing `msvcp140_atomic_wait.dll` | RED | **not reported** (Bug 1) |
| a stray `README.txt` at the zip root | RED | rc 1 `top folder` |
| `third_party/manifest.ron` altered in the zip | RED | rc 1 `sha256 third_party/manifest.ron` |
| exe containing only `api-ms-win-core-synch-l1-2-0.dll` | GREEN | rc 0 (the anchor on `api-ms-win-crt-` works) |

A side observation: the implementer's old fixture zip (built before the TASK-038 merge) now fails `verify` with
`sha256 traffic/traffic.ron`. This is the documented R11 behaviour: tracked files are compared with the current
checkout. A release zip must be verified from a checkout at its tag. It is correct, not a bug.

### Existing suites
I did not re-run cargo locally. The exact commit a39f4d6 is green on CI for clippy (`-D warnings`), client gates,
sim gates and citygen gates, and the tag run's `release-gates` re-ran sim + citygen (590 passed, 0 failed). The
release binary itself was built by CI on both OSes and booted on three machines (two runners and this host).

## 3. Acceptance criteria

| Criterion | Test performed | Result |
|---|---|---|
| `release.yml`: on a `v*` tag both OS builds succeed and a GitHub Release is created with two zips | pushed `v0.1.0-rc1`, run 36293015600: both release legs, gates and publish success; release has exactly 2 zips, prerelease | PASS |
| on `workflow_dispatch`, artifacts are uploaded | Not exercisable: the "Run workflow" button needs `release.yml` on main. The same non-tag path (the `sha-` name, no gates, no publish) is proven by branch runs 36277750356 and 36280342827. The owner can click once after the merge | PASS by equivalent path; dispatch itself unexercised |
| Zips verified: unpack -> exe + `assets/` (incl. third-party + licences) -> boots to the main menu. Linux: xvfb boot smoke. Windows: verified locally by QA from the downloaded artifact | `verify` GREEN on both downloaded release zips (allowlist, sha256 vs manifest, 9 licences). Linux: tag-run xvfb smoke reached `main menu ready` and `city ready`, 0 ERROR. Windows: downloaded asset unpacked with Expand-Archive, smoke GREEN (menu and seed), manual launch screenshot shows the main menu. PE imports are OS-only | PASS |
| Headless test job green on Linux in CI | `release-gates` on the tag: 590 passed, 0 failed; the 5 test workflows on a39f4d6 are success | PASS |
| A real run on GitHub proven: link to the successful run and the release | run https://github.com/pockerhead/MAW-make-GTA/actions/runs/36293015600, release https://github.com/pockerhead/MAW-make-GTA/releases/tag/v0.1.0-rc1 | PASS |
| README "Запуск" updated: where to download releases | README on the branch: releases link, unpack + run, Windows (static CRT, extract first, SmartScreen), Linux apt line. That line matches the DT_NEEDED I parsed myself | PASS |

Spec "decide and document (ADR)": ADR-002 covers the trigger, toolchain pin, Linux deps, profile/no-`fast`, the
linker, caches, naming, layout and smoke. The one claim to correct later: the ADR's "Как выпустить релиз" says tag a
main commit, while this rc was tagged on the branch commit a39f4d6 on purpose (plan §6). This is not a defect.

## 4. Bugs found

1. **Minor (nit) — `tools/package_release.py:35`: the CRT regex misses multi-underscore `msvcp140_*` names.**
   `msvcp140(?:_[0-9a-z]+)?\.dll` matches `msvcp140_1.dll` / `msvcp140_2.dll` but not `msvcp140_atomic_wait.dll` or
   `msvcp140_codecvt_ids.dll`. It also has no `vcruntime140_threads.dll`.
   Reproduce: `scratch/qa/qa_verify_edges.py` case Q3. The exe carries both `UCRTBASE.DLL` and
   `msvcp140_atomic_wait.dll`, and the report names only `UCRTBASE.DLL`.
   Expected: both named. Actual: the second is missed.
   Impact on this release: none. The Windows graph has no C++ code (only `blake3` C, which uses the static CRT with
   `+crt-static`), and my PE parse shows no CRT import at all. A future C++ dependency with the dynamic C++ runtime
   could pass the gate.
   Fix: `msvcp140(?:_[0-9a-z_]+)?` and `vcruntime140(?:_1|_threads)?`, or match `(?:msvcp|vcruntime)140[0-9a-z_]*\.dll`.
   This does not block the release.

No other defects. The review's four minors are fixed as FIX_SUMMARY claims. I checked each in the code, not in the
summary:
- The seed smoke has `--expect "city ready"` (`release.yml:67`), and the marker is real (`src/menu/mod.rs`, the
  OnTransition). The tag-run log shows it.
- `smoke-logs` is uploaded `if: always()` (`:77`).
- The README Linux apt line and the Windows extract/SmartScreen sentences are present.
- The ADR has the publish recovery paragraph.
- An invalid `--allow` gives GATE BROKEN (`package_release.py:188-191`).

## 5. Verdict

**SHIP.** Every acceptance criterion is covered by the real tag run and the downloaded artifacts. The one criterion
that can only run from main (`workflow_dispatch`) is proven by the identical branch path. Bug 1 is a nit in a gate
regex with no effect on the shipped binaries.

Owner items (not blocking, first-frame class):
- After the merge, click "Run workflow" on `release` once. Expect two `sha-` zip artifacts and no release.
- Cut `v0.1.0` from the squash-merged main commit (the rc1 tag points at the branch commit `a39f4d6`, which will not
  be on main).
- The post-merge push to main also runs `release` (the paths filter hits `release.yml`): about 30 min of Windows
  build.

children: 0 launched / 0 reported.
