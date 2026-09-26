# IMPL_REVIEW — TASK-028: release CI for Linux and Windows

Reviewer: code-reviewer (claude opus, medium). Reviewed tree: `652ed50` + `4d8a15e` on `infra/release-ci`.
Files read in full: `.github/workflows/release.yml`, `tools/package_release.py`, `src/menu/screens.rs` diff,
`docs/decisions/ADR-002-release-ci.md`, `README.md` diff, `.github/actions/setup/action.yml`, `src/main.rs:36-235`.
CI evidence: `scratch/ci_run1_linux.log`, `scratch/ci_run1_windows.log` (run 36277750356). New evidence from this
review: `scratch/cr_elf_needed.py`, `scratch/cr_elf_needed.txt` (ELF `DT_NEEDED` of the CI Linux exe).

## 1. Verdict

**PASS.** The workflow, the packaging tool and the permissions do what the plan says, and the branch run proves it on
both OSes. Nothing blocks the rc tag. Four minor issues: the seed smoke is weaker than its label, the README's
Linux deps are incomplete, two Windows first-launch pitfalls are undocumented, and publish is not idempotent. None of
them ships a broken release.

## 0. Disconfirmation

Counter-example tested: "the smoke goes green without the game reaching the menu from the zip's own `assets/`". I
checked four ways this could happen.
- ANSI-coloured `ERROR` escaping the scan: stripped by `ANSI` before `BAD_LINE` (`package_release.py:36-37,207`).
  F3e proved an `ERROR bevy_asset::server` line goes RED. **Did not hold.**
- An early exit with code 0 passing: any `poll()` result that is not `None` fails, whatever the code
  (`:199-211`). **Did not hold.**
- Reading the checkout's assets: the child gets a temp cwd and loses `CARGO_MANIFEST_DIR` / `BEVY_ASSET_ROOT`
  (`:190-192`). Bevy's base path then falls to `current_exe().parent()`. F3f and its control run prove the strip is
  what matters. **Did not hold.**
- A stale log from an earlier run: the log is opened `"wb"` (truncated) on every run. **Did not hold.**

It **partly holds** for the `--seed 1` run (Issue 1). That run expects only `AdapterInfo {`. The only seed-related
log line, `info!("city seed {seed}")` (`src/main.rs:212`), fires before the state is chosen, even on a menu launch. So
the run turns green whether or not the game ever leaves Loading.

No `dead_end` entries exist in `log.jsonl`. I checked the `decision` entries' refs against the code: the Windows smoke
was kept as a gate, rustflags are set at job level, pack sha comes from the manifest, and there is no escape flag.
All four hold.

## 2. Confirmed correct

- **Secrets and permissions** (`release.yml:17-18,95-113`)
  - The workflow has `contents: read`. Only `publish` gets `contents: write`.
  - `GH_TOKEN` is set only in the publish step.
  - There is no `pull_request` or `pull_request_target` trigger.
  - Refs reach shell only through `$GITHUB_REF_NAME`, `$GITHUB_REF` and `$GITHUB_SHA`, always double-quoted
    (`:48,113`). A tag name cannot contain a newline, so `echo "NAME=..." >> "$GITHUB_ENV"` cannot inject a second
    variable.
  - `${{ matrix.* }}` in `run:` holds only static values from the matrix. `${{ env.NAME }}` appears only in
    `with: path`, not in shell.
  - `publish` uses the preinstalled `gh`. No third-party action ever holds the write token.
- **Dispatch-on-tag guard**
  - `release-gates` and `publish` both require `github.event_name == 'push' && startsWith(github.ref,
    'refs/tags/v')` (`:86,96`).
  - The `name` step uses the same pair (`:48`). A dispatch on a tag ref therefore produces `sha-` artifacts and no
    release.
  - A branch named `v...` is `refs/heads/v...` and does not match.
  - A non-`v*` tag does not trigger the workflow at all. The `branches` filter never matches tags.
- **No half-published release.** `publish` needs both matrix legs and `release-gates`, and `fail-fast: false` still
  blocks it if either leg fails. `test ... -eq 2` catches a surprise from download-artifact's layout.
- **Cache key collision with TASK-029: none.**
  - Job ids in the other workflows are `clippy`, `sim`, `citygen`, `client` and `repo`.
  - Real keys from the run: `v0-rust-release-Linux-x64-a972f308[-670c1f85]` and
    `v0-rust-release-Windows_NT-x64-d05cb9d4[-670c1f85]`.
  - The restore prefix `v0-rust-release-<OS>-` cannot prefix-match `v0-rust-release-gates-...`.
  - The assets cache key `third-party-<manifest hash>` is shared by design (same content). On Windows it missed, as
    it should: `actions/cache` versions entries per OS unless `enableCrossOsArchive` is set. The job then fetched
    the packs and saved a Windows entry (`ci_run1_windows.log:469,1465`). Correct, just cold once.
- **The verify gate cannot go green vacuously**
  - An empty zip gives `tops=[]` and fails.
  - Two exes, or none, fail (`:148-149`).
  - The allowlist is exact in both directions (`:150-155`).
  - Pack sha comes from the manifest, a quantity the local checkout cannot move (`:76-87`).
  - Every pack's licence is checked (`:164-166`).
  - The CRT scan runs only for `.exe` names and the exec bit only for extension-less names (`:167-171`).
  - Plumbing failures exit 2 with `GATE BROKEN`. I ran these myself: a missing zip, a non-zip, `--` given to
    `verify`, and a dir without an exe all give rc=2 (below).
- **Zip layout for a Windows double-click.**
  - One top folder `NAME/`, with `gta_like.exe` and `assets/` inside.
  - The exe resolves assets from `current_exe().parent()` when `CARGO_MANIFEST_DIR` is unset, which is the normal
    case on a user machine.
  - The Windows exe has no CRT imports (CI package summary, plus the local check of the CI exe in IMPL_SUMMARY).
    It needs no redistributable.
  - Explorer's "Extract All" keeps the structure. The longest entry is 98 characters with a tag name, well under
    MAX_PATH even after Explorer doubles the folder name.
- **Linux exe.** The entry is 0755. `unzip` keeps the mode, and the smoke started the exe from the unpacked zip.
  The glibc floor in the binary is `GLIBC_2.39` (`cr_elf_needed.txt`), so the README's "glibc >= 2.39" is exact.
- **Menu marker.** `src/menu/screens.rs:40` adds `info!("main menu ready")` on `OnEnter(MainMenu)`, one line. Both CI
  smokes matched it.
- **Plan conformance.** The YAML matches plan Step 3 verbatim, plus one comment. The tool follows Step 2, including
  the `CRT` anchor on `api-ms-win-crt-`, which the CI exe itself confirmed (`api-ms-win-core-*` present, no CRT
  imports). The ADR and README cover Steps 4 and 5.
- **CLI checks I ran:** `--help` and each subcommand's `--help` exit 0. `verify nope.zip`, `verify README.md`,
  `verify x.zip -- --seed 1` and `smoke <dir without exe>` each exit 2 with a message naming the gate or the usage
  error.

## 3. Issues

### Issue 1 — minor — `release.yml:62`: the seed smoke passes without proving the seed path ran
The `--seed 1` run expects only `AdapterInfo {`. That line appears before any game state, so the 60 s ERROR scan could
cover a game stuck in Loading or one that silently ignored `--seed`. PLAN §5 labels it "correctness (bounded window)
… in a seeded city". R7 and the ADR are more honest ("liveness + error scan"). Locally, F3e showed the player GLB
loads within 60 s on an RTX GPU, but nothing shows the same on lavapipe. The seed log is not uploaded on success, so
the green run 36277750356 cannot be checked after the fact.
**Fix:** add one `info!` marker when the city finishes loading (the `Loading -> Playing` transition) and
`--expect` it in the seed run. The cheaper fallback is to upload `smoke-*.log` with `if: always()` so a green run's
log can be read.

### Issue 2 — minor — `README.md` "Запуск" (Linux line): the runtime deps are incomplete
The CI Linux exe's `DT_NEEDED` list is `libwayland-client.so.0`, `libudev.so.1`, `libasound.so.2`, `libgcc_s`,
`libm`, `libc` (`scratch/cr_elf_needed.txt`). `libwayland-client0` is a hard load-time dependency even on a pure X11
session, so "X11 или Wayland" undersells it. The binary also dlopens `libxkbcommon.so.0` (both backends), plus
`libxkbcommon-x11.so.0`, `libX11-xcb.so.1`, `libXcursor.so.1` and `libXi.so.6` (X11) and `libvulkan.so.1`. A
desktop Ubuntu 24.04 has all of them, but a minimal or other distro may not, and the error it gives is unhelpful.
**Fix:** list the packages: `libasound2t64 libudev1 libwayland-client0 libxkbcommon0 libvulkan1` + a Vulkan driver,
and for X11 `libxkbcommon-x11-0 libxcursor1 libxi6 libx11-xcb1`. The smoke step's apt line
(`release.yml:59`) already names most of the X11 ones.

### Issue 3 — minor — `README.md` "Запуск" (Windows line): first-launch pitfalls are not stated
- If the owner double-clicks `gta_like.exe` inside the zip in Explorer without extracting, Explorer extracts only the
  exe to a temp folder. Preflight then prints "missing third-party asset" to the console and returns
  `AppExit::error()` (`src/main.rs:155-166`). The console window closes at once, so the user sees a flash and
  nothing else. The README says "unpack", but not why this matters.
- The exe is unsigned and downloaded from the internet, so SmartScreen shows "Windows protected your PC". The user
  must click "More info → Run anyway".

This is a first-launch failure the owner would see, not a silent one. **Fix:** two README sentences: "extract first,
don't run it from inside the zip" and the SmartScreen step. A visible error on missing assets belongs with the Q1
console/log-file polish, not here.

### Issue 4 — minor — `release.yml:113`: publish is not idempotent on re-run
`gh release create` creates the release and then uploads the assets. If an upload fails midway, a release without
all its assets remains, and "Re-run failed jobs" then fails with "release already exists". The ADR procedure ("a
failed tag is never moved, use the next rc") covers recovery but does not mention this case or the manual
`gh release delete` / `gh release upload --clobber` step.
**Fix:** one line in the ADR's "Как выпустить релиз". Alternatively, `gh release upload --clobber` if
`gh release view "$GITHUB_REF_NAME"` succeeds.

## 4. Missing coverage

- A state marker for the seeded run (Issue 1). Without it, the "no load errors in the city" claim is only liveness.
- A green-path log artifact for the smokes. Evidence is lost unless the run fails.
- Step 7 (the rc tag, `release-gates`, `publish`, download-artifact `skip-decompress` on an `archive: false` artifact,
  QA's `verify` of the release asset) is not exercised yet. It is pending TASK-038 per the orchestrator, so it is
  not flagged. Of all this, only the download-artifact layout (R6) is unproven. `test -eq 2` makes it fail loud.

## 5. Nits

- `package_release.py:188`: an invalid `--allow` regex raises an uncaught `re.error` traceback (exit 1, which reads as
  a product failure) instead of exit 2 with `GATE BROKEN`.
- `package_release.py:242`: `--seconds 0` kills the game at once. The expects would then fail, so it is not vacuous,
  but a lower bound would give a clearer message.
- The release job runs tag-pinned third-party actions (`Swatinem/rust-cache@v2`, `dtolnay/rust-toolchain@1.95.0`,
  inherited from the TASK-029 composite) in the job that produces the shipped binary. They hold a read-only token,
  but they could still tamper with the build. SHA-pinning is worth a note for the pre-v0.1.0 polish. This is
  pre-existing and out of this task's scope.
- The Linux exe is 211 MB unstripped (54 MB zipped). ADR-002 deliberately keeps the default profile, so this needs no
  action. It is recorded in case the owner asks about download size.

children: 0 launched / 0 reported.
