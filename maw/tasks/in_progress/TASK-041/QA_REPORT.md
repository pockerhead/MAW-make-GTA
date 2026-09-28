# TASK-041 QA_REPORT (qa, claude opus, medium)

Target: the PUBLISHED release https://github.com/pockerhead/MAW-make-GTA/releases/tag/v0.1.0 (tag object
`6bd6f72` -> commit `8fd8c28` on `origin/main`). I checked the downloaded artifacts, not a branch build.

## Disconfirmation

Counter-example: "the published Windows zip is not what the branch checks proved". That could mean the CI-built exe
is console-subsystem or links the CRT, the body is `--generate-notes` instead of the notes file, or the exe finds
`assets/` only relative to the cwd.
**It did not hold.** Release body == `docs/releases/v0.1.0.md` byte for byte (3143 chars). My own PE parser, which
reads the real import directory rather than a byte regex, gives subsystem 2 and no CRT imports. The exe boots from
ShellExecute and from the smoke's temp cwd (not the exe folder).

## 1. Environment

- Direct, no docker/services. Windows 11, RTX 4070 Ti (Vulkan). Nothing to clean up except the unpacked release in
  the session scratchpad (outside the repo).
- Download: `gh release download v0.1.0 --repo pockerhead/MAW-make-GTA` into
  `C:/Users/user/AppData/Local/Temp/claude/D--test-gta-like/37f0577b-.../scratchpad/rel/`; the Windows zip was
  extracted to `rel/win/`.
- The scripts are in `maw/tasks/in_progress/TASK-041/scratch/qa/` (gitignored like all of scratch):
  `pe_imports.py`, `boot_probe.py`, `fatal_probe.py` / `fatal_probe2.py` / `fatal_probe_x.py`, and the `*.out`
  output of each.
- No cargo build was run: the code is merged and CI tested it. This QA covers the release artifact.

## 2. Test results

| # | Check | Result |
|---|---|---|
| 1 | `gh release view v0.1.0`: `isPrerelease=false`, `isDraft=false`, 2 assets (`gta-like-v0.1.0-linux-x86_64.zip` 54.8 MB sha256 `0d2be82e...`, `gta-like-v0.1.0-windows-x86_64.zip` 37.7 MB sha256 `59d34537...`, both equal to the local downloads). Body == `docs/releases/v0.1.0.md` exactly (`body==file True`). Tag run 36430053506: `success`, headSha `8fd8c28`, jobs release-gates / linux / windows / publish all success. The publish log shows `gh release create ... --notes-file "$f"`. | PASS |
| 2a | `python tools/package_release.py verify` on both zips: both `OK`, 98 entries (exe + 97 assets), allowlist, sha256, licences for 9 packs, no dynamic imports, and `GUI subsystem` on the Windows zip. Exit 0 both. | PASS |
| 2b | Independent PE parser (`scratch/qa/pe_imports.py`): machine 0x8664, PE32+ magic 0x20b, **Subsystem 2**, 20 import DLLs (advapi32, kernel32, user32, ... `api-ms-win-core-*` only), 0 delay imports, **CRT imports: none**. Positive control: the same parser on the local dynamic-CRT `target/release/gta_like.exe` finds `VCRUNTIME140.dll` and `api-ms-win-crt-*`, so the "none" is a real result. | PASS |
| 3 | Double-click-equivalent boot (`scratch/qa/boot_probe.py`: `ShellExecuteExW` "open", no args, no stdio, cwd = exe folder): `main menu ready` in the log after 1.83 s. The PID's top-level windows are `Window Class 'GTA-like'` (visible) plus winit/IME/d3d helper windows. **No `ConsoleWindowClass`/`PseudoConsoleWindow`, no child processes (no conhost/OpenConsole), `AttachConsole(pid)` fails with error 6 (the process has no console).** `gta_like.log` next to the exe: 8 lines, 0 ERROR, 0 panicked, 1 WARN (`bevy_settings: settings.toml not found`, expected on a first run). Screenshot `scratch/qa/boot_menu_crop.png` (cropped to the game window; the full-desktop capture was deleted because it showed the owner's desktop): the title "GTA-like", the "Новая игра" button, the Seed field with the Enter hint, and the "Выход" button. Only that PID was killed. No `settings.toml` was written under APPDATA/LOCALAPPDATA. | PASS |
| 4 | Fatal path (`scratch/qa/fatal_probe2.py`): `assets/` renamed, the log pre-seeded with `STALE main menu ready`, the exe started the same way. A `#32770` dialog titled "GTA-like" appears after 0.5 s. Its text: "Игра не запустилась:", then the path `...\assets\world/render.ron: Системе не удается найти указанный путь. (os error 3)`, then "Если exe запущен прямо из zip, распакуйте архив целиком.", then "Лог: ...\gta_like.log". While the dialog is open, the log holds exactly 1 line, `ERROR ...render.ron ... (os error 3)`, and the stale line is gone. OK (BM_CLICK) closes the dialog: no windows are left and the process exits with **code 1**. The title-bar X (SC_CLOSE, `fatal_probe_x.out`) gives the same result. `assets/` was restored (97 files) after each run. Screenshot `scratch/qa/fatal_dialog.png`. | PASS |
| 5 | `python tools/package_release.py smoke <unpacked> --seconds 30 --log scratch/qa/smoke-menu.log --expect "AdapterInfo {" --expect "main menu ready"` (CI's arguments): `smoke: OK gta_like.exe alive 30.0 s; ... 2 expect(s) matched; 0 ERROR/panic lines, 0 console panic lines, 1 WARN`, exit 0 (`scratch/qa/smoke.out`). | PASS |
| 6 | CRT unit row flip (my own, in memory, no file edit): with the exact TASK-028 regex from `7da8588:tools/package_release.py:35` patched into the module, `CrtImports` goes RED on exactly `vcruntime140_threads.dll`, `msvcp140_atomic_wait.dll`, `msvcp140_codecvt_ids.dll`. On the shipped regex it is GREEN. `python -m unittest tools/qa/test_package_release.py`: 10 OK. | PASS |
| 7 | CI on main `8fd8c28`: clippy, client gates, citygen gates, sim gates, repo checks all `success`, and the branch `release` run is `success` too. (`1bd409c`, a docs-only commit after it, had sim gates still running; that commit is not part of the release.) | PASS |

## 3. Acceptance criteria

| Criterion | Test performed | Result |
|---|---|---|
| The release exe starts on Windows without a console window; the log file exists and the smoke reads it; the Windows CI smoke is green | #2b (subsystem 2), #3 (ShellExecute boot: no console-class window, no conhost child, AttachConsole error 6; log beside the exe), #5 (local smoke reads `gta_like.log`), tag run Windows `boot smoke ... smoke: OK` | PASS |
| The CRT regex covers the named DLLs, with a unit row that flips RED on the old regex | #6 (my flip against the real old regex: RED on exactly the 3 names) | PASS |
| Release notes are in the release body | #1 (body == `docs/releases/v0.1.0.md`, publish used `--notes-file`) | PASS |
| v0.1.0 is published from main with two zips, is not a pre-release, the release run is green, and the downloaded Windows zip boots to the main menu | #1, #2a, #3 (menu screenshot) | PASS |
| Existing tests pass; CI 5/5 on main | #7 (5/5 + release on `8fd8c28`), unittest 10 OK | PASS |

## 4. Bugs found

None that block the release.

- Nit (cosmetic): the GitHub release `name` is `null` (the publish step passes no `--title`), so GitHub shows the tag
  `v0.1.0` as the title. That is acceptable. Pass `--title` next time if a nicer title is wanted.
- Harness note (not a product bug): `PostMessage(WM_COMMAND, IDOK, 0)` does not close the MessageBox, so the process
  looked hung (exit code 259). A real click or the X closes it, and the game exits 1. Logged as `dead_end`.
- Not covered locally: the Linux zip boot. It passes `verify` here, and the tag run's Linux smokes (menu 30 s,
  seed 60 s) are OK. Per the project rule, Linux goes through CI, not local WSL.

## 5. Verdict

**SHIP.** Checks (1)-(5) of the orchestrator note pass on the downloaded v0.1.0 artifacts.

For the owner (visible on first launch, not machine-gated): a real mouse double-click from Explorer. The
ShellExecute probe is the same API call, but the owner's own look at the desktop (no console window, the menu) and
at the dialog wording (`scratch/qa/fatal_dialog.png`) is the final word on the look.

Cleanup: no `gta_like.exe` process left (tasklist count 0). No containers or services were started. `git status` is
clean apart from gitignored scratch. The unpacked release stays in the session scratchpad, outside the repo.
