# Showcase clips

Short GIFs of the real windowed game for the repo README. Windows only (ffmpeg `gdigrab` + Win32 calls).

## Record

```
python tools/fetch_assets.py                       # once, if the packs are not installed
python tools/showcase/record.py --all              # every scenario
python tools/showcase/record.py city melee         # just these
python tools/showcase/record.py --all --no-build   # reuse the last `--release --features dev` build
```

What it does: builds `--release --features dev`, copies the exe to a temp dir and runs the copy (the
build output is never locked), with `--seed`, a unique `--window-title` and a free BRP port, so another
"GTA-like" window or QA run does not interfere. The window is made topmost and ffmpeg grabs its
screen region at 30 fps while the scenario drives the game over BRP (`tools/qa/brp.py`). The capture
becomes a GIF of at most 12 s and 5 MB (720 px / 15 fps first, smaller presets if it is too big).

Output: `<cargo target dir>/showcase/` (`*.gif`, raw `*.mkv`, game and ffmpeg logs, `record.json`).
Do not cover the game window with a topmost window while it records: the capture is of the screen.

## Publish

```
python tools/showcase/publish.py --dry-run         # files + raw URLs + README diff; local branch media-dry-run
python tools/showcase/publish.py                   # force-push branch `media`, rewrite README.md
python tools/showcase/publish.py --hero city       # another clip under the title (default: shooting)
```

`media` is an orphan branch with one commit that is replaced on every publish, so GIFs never enter the
main history. README links carry `?v=<blob hash>` so GitHub's image cache picks up new clips. The hero
clip goes between `<!-- SHOWCASE-HERO:START/END -->` under the title; every other clip goes under its
slice line in "## Статус" between `<!-- SHOWCASE:<name>:START/END -->`. Commit README.md yourself.
Run record and publish with the same `CARGO_TARGET_DIR` (or pass `--gifs <dir>`).

## Add a scenario

Drop `tools/showcase/scenarios/<name>.py` (files starting with `_` are helpers, not scenarios):

```python
CAPTION = "one-line caption under the clip"
ANCHOR = "- TASK-0NN (TN)"   # start of the slice line in README "## Статус"
SEED = 1

def prepare(game):  # not recorded: teleport, pick up, aim
    ...

def play(game):     # recorded, keep it under 12 s; move the camera, do not show a static frame
    ...
```

`game` is a `brp.Game`; helpers live in `scenarios/_common.py` (`look`, `pan`, `stand`, `landmark`)
and in the QA scenarios `tools/qa/scenarios/t5.py`..`t7.py` (both folders are on `sys.path`).
