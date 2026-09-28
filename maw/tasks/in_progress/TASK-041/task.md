# TASK-041: Final release v0.1.0

Type: chore
Mode: small-fix
Priority: high
Branch: chore/release-v0.1.0
Domains: gates

## Description
All GDD slices are done, both playtests are triaged and traffic is closed (TASK-039). Ship v0.1.0 from main. Carried items:
1. **Hide the Windows console window in release builds** (TASK-028 Q1): `#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]` or the equivalent for the release profile. Logs must then also go to a file next to the exe, or to a documented user dir, so the Windows CI smoke and QA can read them. Keep `tools/package_release.py smoke` working on Windows: it currently reads stdout, so point it at the log file.
2. **The CRT regex nit** (TASK-028 QA): `tools/package_release.py:35` misses `msvcp140_atomic_wait.dll`, `msvcp140_codecvt_ids.dll` and `vcruntime140_threads.dll`. Fix it to `msvcp140(?:_[0-9a-z_]+)?` / `vcruntime140(?:_1|_threads)?`, with a unit row.
3. **Release notes** for the GitHub release: what is in the game (the GDD §1 points), controls, how to run it (Windows: unpack first, SmartScreen; Linux deps), and known limitations (from README: traffic residue, no traffic lights, walkers do not avoid each other, the squeeze looks like driving through, no licence specified). Russian, short.
4. After merge and a green CI on main (5 test workflows plus release if triggered), tag `v0.1.0` on the main merge commit, push the tag, confirm the release run is green, confirm the GitHub Release is a full release (not a pre-release) with two zips, and download the Windows zip and boot it once from an unpacked folder (a double-click-equivalent start with no console window, reaching the main menu).

Cost of error: the owner and anyone downloading see it on first launch. Prove it with the real downloaded artifact.

## Acceptance criteria
- [ ] The release exe on Windows starts without a console window. The log file exists and the smoke reads it; the Windows CI smoke is green.
- [ ] The CRT regex covers the named DLLs, with a unit row that flips RED on the old regex.
- [ ] Release notes are in the release body.
- [ ] v0.1.0 is published from main, with two zips, is not a pre-release, and the release run is green; the downloaded Windows zip boots to the main menu.
- [ ] Existing tests pass; CI 5/5 on main.

## Dependencies
- blocked by TASK-039
