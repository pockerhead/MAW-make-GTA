# TASK-021: Gameplay showcase clips for the GitHub README

Type: chore
Mode: small-fix
Priority: high
Branch: feature/showcase-clips
Domains: gates

## Description
Owner request (2026-09-23): "надо еще на странице в гитхабе чтобы были гифки или видосики с геймплеем ... и чтобы они обновлялись отображая каждый раз новые грани геймплея и игры ... клипы маленькие нужны по 15 сек даже меньше но зато сразу будет видно что есть игра".
Build a repeatable pipeline that records short gameplay clips of the REAL windowed game and publishes them to the README.

This runs IN PARALLEL with another MAW task in a separate git worktree (`D:/test-gta-like/.worktrees/showcase`). Environment rules:
- Always build with `CARGO_TARGET_DIR=D:/test-gta-like/target` (the parent's target dir, shared). Builds from the other worktree may hold the cargo lock — waiting is fine; never start a second cargo command yourself while one runs.
- Never run the game straight from the shared target: copy the built exe to a temp dir and run the copy (a running exe would block the other task's builds on Windows). Assets must be found from the worktree (`assets/` path / asset root); run `python tools/fetch_assets.py --cache D:/test-gta-like/assets/third_party/../..` is NOT needed if you copy/point the asset root at the worktree's own `assets/` after fetching — simplest: `python tools/fetch_assets.py` in the worktree (it can reuse the parent's zip cache if the script supports a cache dir; the parent tree already has the packs installed under `D:/test-gta-like/assets/third_party/`).
- Another game window titled "GTA-like" may be open from the other task's QA: give the recorded game a unique window title.

Deliverables:
1. Game: a CLI flag `--window-title <text>` (client only, default unchanged). Optionally a `--showcase` flag if a scenario needs HUD tweaks — prefer none.
2. `tools/showcase/record.py`: builds `--release --features dev` (shared target), copies the exe, launches it with a unique title and `--seed`, waits for BRP readiness, starts ffmpeg `gdigrab` on that window title (30 fps), drives a scenario over BRP (reuse `tools/qa/brp.py`), stops ffmpeg, shuts the game down, then converts to an optimized GIF (ffmpeg palettegen/paletteuse, ~640-720 px wide, ~15 fps, ≤ 12 s, target ≤ 5 MB each). ffmpeg 8 is installed (`C:/ProgramData/chocolatey/bin/ffmpeg`).
3. Scenarios in `tools/showcase/scenarios/` for the features that exist NOW: (a) city walk / look around incl. the view from the tower roof, (b) shooting at the range with damage numbers and a CRIT, (c) melee combo + bat knockdown, (d) death → "ПОТРАЧЕНО" → hospital respawn. Each scenario is a small script so new features just add a file. Cinematic enough to show the game (camera moves, not static frames).
4. `tools/showcase/publish.py`: puts the GIFs onto an orphan branch `media` (single commit, force-pushed each time so repo history does not grow), prints the raw URLs; and rewrites a README section between markers `<!-- SHOWCASE:START -->` / `<!-- SHOWCASE:END -->` (a small gallery with a one-line caption per clip, placed right after the intro). Binary files never go into the main branches (the `.gitignore` already ignores gif/mp4). Do NOT run the push yourself — the orchestrator runs publish; you may test it with a `--dry-run` that stages into a local temp branch/dir.
5. `tools/showcase/README.md` (short): how to record and publish, how to add a scenario.

## Acceptance criteria
- [ ] `python tools/showcase/record.py --all` produces 4 GIFs (each ≤ 12 s, ≤ 5 MB) in an ignored output dir; the orchestrator/QA can look at frames (extract a few PNG frames with ffmpeg and look at them): the game is visible, not a black/other window.
- [ ] Running record while another "GTA-like" window is open records the right window (unique title).
- [ ] The shared target exe is never left locked (the copy runs), and no game or ffmpeg process is left running.
- [ ] `publish.py --dry-run` shows the README section it would write and the files it would push to `media`.
- [ ] `cargo build`, `cargo clippy -- -D warnings`, `cargo test -p gta_sim`, `cargo test -p gta_like --bin gta_like` stay green (build with the shared CARGO_TARGET_DIR)
- [ ] Existing tests pass

### Owner addition (2026-09-23) — placement in the README
Owner: "гифки можно прям с СТАТУС запихивать и одну обязательно прямо в начало".
- [ ] One hero GIF at the very top of README.md, right under the `# MAW-make-GTA` title (its own markers, e.g. `<!-- SHOWCASE-HERO:START/END -->`); publish picks the most representative clip (default: shooting / city).
- [ ] The rest of the gallery goes INTO the "## Статус" section, each GIF next to the status line of the slice it shows (markers inside that section), not in a separate section after the intro.
