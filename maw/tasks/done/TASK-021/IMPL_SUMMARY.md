# IMPL_SUMMARY: TASK-021, showcase clips (small-fix)

Pre-flight: no separate plan. I checked what the spec relies on: `tools/qa/brp.py` (`Game`, `send_keys`,
`move_mouse`, `send_mouse_button`, `mutate_component`), the helpers in the QA scenarios t5/t6/t7, `BRP_EXTRAS_PORT`
(bevy_brp_extras 0.22.6 `plugin.rs:171`), `CityLandmarks` (`crates/gta_sim/src/world/city.rs:53`), ffmpeg 8.0 with libx264.
One claim in the spec was wrong: ".gitignore already ignores gif/mp4". Only `*.mp4` was ignored, so I added `*.gif`.

## 1. What was implemented

| File | Lines | What |
|---|---|---|
| `src/main.rs` | +23 / -8 | `--window-title <text>` flag. The default title "GTA-like" is unchanged. `--seed` now uses the shared `flag_value` helper |
| `.gitignore` | +1 | `*.gif` |
| `tools/showcase/record.py` | 268 | build, copy exe to a temp dir, launch it (unique title, free BRP port, `--seed`), wait for BRP and city chunks, `prepare`, raise window, ffmpeg capture at 30 fps, `play`, stop ffmpeg with "q", shut the game down, GIF |
| `tools/showcase/publish.py` | 175 | orphan commit with only the GIFs (`hash-object` / `mktree` / `commit-tree`, index and work tree untouched), `push --force origin <sha>:refs/heads/media`, README rewrite; `--dry-run` sets local branch `media-dry-run` and prints the README diff |
| `tools/showcase/scenarios/_common.py` | 53 | `look`, `pan` (smoothstep camera turn at 30 Hz via `OrbitCamera.yaw/pitch`), `tilt`, `stand`, `landmark` |
| `tools/showcase/scenarios/city.py` | 24 | sprint down the street from the spawn point while the camera swings, then a cut to the edge of the 156 m tower roof and a pan over the city |
| `tools/showcase/scenarios/shooting.py` | 61 | pistol and SMG picked up in prepare; RMB aim, 2 body shots, headshot with a red CRIT (up to 3 tries), switch to SMG, burst at another dummy |
| `tools/showcase/scenarios/melee.py` | 41 | fist combo (10, 10, 20 and a knockdown), toggle to the bat, bat swing. The camera is tilted to -30° because at eye level the player hides the dummy |
| `tools/showcase/scenarios/wasted.py` | 25 | walk, 40 damage (HUD bar drops), lethal hit, slow motion and "ПОТРАЧЕНО", hospital respawn |
| `tools/showcase/README.md` | 54 | how to record, publish and add a scenario |

A scenario is one file with `CAPTION`, `ANCHOR` (the start of its slice line in README "## Статус"), `SEED`,
`prepare(game)` (not recorded) and `play(game)` (recorded). Files starting with `_` are helpers.

README placement (the orchestrator's addition): the hero clip (default `shooting`, `--hero <name>`) goes right
under `# MAW-make-GTA` between `<!-- SHOWCASE-HERO:START/END -->`. Every other clip goes right after its slice
bullet in "## Статус", as a 2-space-indented continuation between `<!-- SHOWCASE:<name>:START/END -->`, with
`<img width=480>` and a `<sub>` caption. Rewrites are idempotent. When the hero changes, the new hero's status
block is removed and the old hero gets a status block. Image URLs are
`https://raw.githubusercontent.com/<owner>/<repo>/media/<name>.gif?v=<blob sha8>`: the filename stays the same
and the query changes whenever a GIF changes, which busts GitHub's camo cache.

## 2. Deviations and decisions

- **Capture is not `gdigrab -i title=`.** On the Bevy window that mode returned a stale frame. A 17 s capture
  showed a frozen image while BRP screenshots showed the scene moving (`scratch/probe_capture.py`,
  `scratch/probe/plaza_gdi.png` vs `plaza_brp.png`). record.py now calls `SetWindowPos(HWND_TOPMOST)` on the window
  found by its unique title and grabs that client rect from the desktop (`gdigrab -i desktop -offset_x/-offset_y
  -video_size`). ffmpeg 8.0 essentials has no `gfxcapture`. A BRP screenshot sequence is limited to about 6 fps.
  The catch: **the game window stays on top of the screen while a clip records** (about 15-20 s per clip). The
  unique title is still what picks the right window. ctypes needs `HWND` argtypes: an untyped `-1` gave
  `ERROR_INVALID_WINDOW_HANDLE`. Both findings are in log.jsonl.
- The capture waits for ffmpeg's `frame= N>0` progress line before `play` starts. The mkv file stays empty for
  seconds because of the encoder lookahead, and the first version recorded 6 s of idle lead-in.
- GIF size: presets are tried in order `(720,15,256) → (640,15,192) → (640,12,128) → (640,10,128) → (560,12,96) →
  (480,10,64)` until the GIF is ≤ 5 MB. The city clip has constant camera motion and lands on 640 px / 10 fps.
- `plaza_center` is the tower's footprint: the player teleported there is inside the tower and the camera sits
  in his head. The city clip starts from the spawn street instead.
- Output goes to `<cargo target dir>/showcase/`, which is `D:/test-gta-like/target/showcase` with the shared
  CARGO_TARGET_DIR. The worktree gets no second `target/`. `fetch_assets.py` wrote its zip cache into
  `.worktrees/showcase/target/asset-cache`, and I deleted it after the packs were installed.
- `publish.py --dry-run` leaves a local branch `media-dry-run`. Branch refs are shared by all worktrees, so it is
  also visible from the main checkout. Delete it with `git branch -D media-dry-run`.
- Not done, as the spec says: no real push and no README.md change. The orchestrator runs `publish.py` and commits README.md.

## 3. Test results (CARGO_TARGET_DIR=D:/test-gta-like/target, one cargo command at a time)

- `cargo build -j 4`: ok. `cargo clippy -j 4 -- -D warnings`: ok, no warnings.
- `cargo test -j 4 -p gta_sim`: every suite ok (19+4+3+6+19+6+3+19+4+5+16+2 passed, 1 ignored as before).
- `cargo test -j 4 -p gta_like --bin gta_like`: 32 passed. `cargo test -p citygen`: ok. `python tools/qa/tree_check.py`: passed.
- `rustfmt --edition 2024 --check src/main.rs`: clean. `git diff --stat` shows only `src/main.rs` and `.gitignore`.
- Acceptance run `scratch/run_all_with_decoy.py`: a decoy game copy titled exactly "GTA-like" (seed 7) was open
  during `record.py --all` (with the build). rc 0, 4 GIFs:
  city 4.81 MB / 10.9 s, melee 4.01 MB / 9.34 s, shooting 3.86 MB / 10.83 s, wasted 3.86 MB / 9.8 s.
  Afterwards `tasklist` showed no `gta_like`/`ffmpeg` process, and the shared `target/release/gta_like.exe`
  opened for writing (not locked). The frames show the seed-1 city, the same street as in the earlier runs,
  not the decoy's seed-7 city.
  Melee was re-recorded afterwards with the -30° tilt (4.39 MB / 10.34 s), and wasted once more after a
  small record.py refactor (3.85 MB / 9.8 s).
- Frames I looked at: `scratch/frames/*_sheet.png` (1 frame/s contact sheets, `scratch/contact_sheets.sh`) and
  `scratch/frames/melee_combo.png`. Visible in them: the street sprint and the roof pan (city); numbers 25/27,
  red "CRIT", SMG burst with numbers (shooting); 10/10/20, the dummy lying down, the bat (melee); the grey slow-mo,
  "ПОТРАЧЕНО", respawn on the hospital sidewalk (wasted).
- `publish.py --dry-run`: output in `scratch/publish_dry_run.txt` (4 files on `media-dry-run`, raw URLs, README
  diff with the hero block under the title and 3 blocks in "## Статус"). `scratch/check_readme_rewrite.py`:
  rewrite is idempotent, a hero switch leaves no duplicates, non-showcase text is unchanged.

## 4. How to verify manually

```
set CARGO_TARGET_DIR=D:/test-gta-like/target
python tools/showcase/record.py --all            # about 5 min; do not touch the topmost game window
python tools/showcase/publish.py --dry-run       # files, URLs, README diff
ffmpeg -i %CARGO_TARGET_DIR%/showcase/shooting.gif -vf fps=1,tile=3x4 -frames:v 1 sheet.png
```

For the owner (visual, the first-frame class): whether the clips look cinematic enough (camera pans, the cut to
the roof, the -30° melee view) and which clip should be the hero. The mechanics shown are already gated by t5-t7.
Real publish: `python tools/showcase/publish.py`, then commit README.md.

children: 0 launched / 0 reported.
