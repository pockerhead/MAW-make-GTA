# FIX_SUMMARY — TASK-004 (fixer)

## Preflight: the riskiest prescription in the review

Claim checked first: issue 1 prescribes "move to a roof edge and face the city". Taken literally (teleport
to the roof edge) this can drop the player off the tower. The tower is rotated to its street frontage and
`CityLandmarks` carries only the roof centre, so the script cannot know where the edge is. Checked in
`crates/citygen/src/lots.rs:200-220` (`tiers`): a tier is pushed only when both half extents are
`>= massing.setback_min_half` (6.0 m in `assets/world/city.ron:26`). So any point within 6 m of the roof
centre is on the top tier for any tower yaw. The fix uses a 4.5 m offset (6 m minus the 0.3 m capsule minus
margin) instead of the edge, and keeps the "standing on the roof" assertion after every teleport. The
diagnosis was right, the literal prescription was not safe.

## Fixed

1. **Major, `tools/qa/scenarios/t3.py`: roof FPS/screenshots did not show the city.** Confirmed: the old
   `scratch/qa/t3/roof_0.png` shows only roof and sky, `roof_down.png` mostly roof. The camera sits 3.8 m
   behind a player standing at the roof centre and cannot see past the edge.
   Change: for each world direction (east, south, west, north) the player is teleported 4.5 m from the roof
   centre towards it. The roof assertion is checked, then `OrbitCamera.yaw/pitch` are set over BRP
   (`face()`: `yaw = atan2(-dx, -dz)`, since forward = R_y(yaw)·(−Z); pitch −45°) and a screenshot
   `roof_<dir>.png` is taken. `OrbitCamera` is `Reflect` + registered (`src/camera/mod.rs:12-27`), and
   `apply_mouse_look` only writes yaw/pitch while the cursor is captured, so the BRP value holds. The old
   `move_mouse` approach depended on cursor capture. FPS is sampled after that, facing north over the edge.
   `summary.json` records the view as `roof_fps_view`. Pitch −25° was tried first: the city was only a thin
   band above the roof. −45° shows the streets, the park and the downtown towers
   (`scratch/qa/t3_fixer/roof_north.png`, `roof_west.png`).
   Before/after evidence: the old screenshots in `scratch/qa/t3/` show no city, the new ones in
   `scratch/qa/t3_fixer/` do. Whether the city "looks like a city" is still for the owner to judge.
2. **Major, `tools/fetch_assets.py` `extract()`: a failed publish left the pack missing.** Confirmed with my
   own probe `scratch/fixer_probe_publish_rollback.py <fetch_assets.py>`. It injects an `OSError` into the
   second `os.replace` and asserts: the old pack is back in place, no `.old-<name>`, no `.tmp-<name>`, and a
   retry installs the new pack.
   Change: both renames are now inside one `try`. On failure, if the old pack was moved aside it goes back
   to `final`, `tmp` is removed, and the error is re-raised.
   Flip-RED: the probe against HEAD's copy (`scratch/fetch_assets_head.py`) gives
   `{'old_pack_restored': False, 'old_dir_gone': False, 'tmp_gone': False}`, exit 1. Against the fixed file
   it gives all True, exit 0.

## Skipped

- **Missing coverage: the mesh gate does not catch missing sidewalk/curb/marking geometry.** Not a defect
  in this diff. The review itself leaves it to the owner run, and the project context says visuals the
  owner sees on the first frame are gated by the owner, not by extra machinery. No change.
- **Missing coverage: no failure-injection test for the publish transition.** `fetch_assets.py` is a
  stdlib tool with no Python test harness in the repo, and adding one is out of scope. The rollback is
  covered by the scratch probe above (RED on HEAD, GREEN on the fix).

## Test results

- `python maw/tasks/in_progress/TASK-004/scratch/fixer_probe_publish_rollback.py tools/fetch_assets.py`:
  all True, exit 0. The same probe on the HEAD copy: all False, exit 1.
- Real reinstall path: flipped one byte in `assets/third_party/city-kit-roads/light-square.glb`, then
  `python tools/fetch_assets.py --check` gave exit 1 naming the file and both hashes.
  `python tools/fetch_assets.py --cache maw/tasks/in_progress/TASK-004/scratch/kenney` reinstalled roads
  and skipped the other two, exit 0. `--check` then gave "third-party packs match the manifest", exit 0.
  `assets/third_party/` holds only the 3 packs and `manifest.ron`.
- `python tools/fetch_assets.py --validate-only assets/third_party/manifest.ron`: valid.
- `python tools/qa/scenarios/t3.py --out maw/tasks/in_progress/TASK-004/scratch/qa/t3_fixer` (release,
  `--features dev`, seed 1): passed. Results: hash `0xd2158922f1e5cd6f`, 100 chunks, 4237 props, 4 roof
  positions at y ≈ 157.05, park at y 1.20, `log_errors: []`. FPS facing north at −45°: min 134.8,
  avg 140.6, max frame time 7.42 ms (Fifo). This is owner evidence, not a gate. The game process has exited.
- `python tools/qa/tree_check.py`: tree checks passed.
- `cargo test -p gta_sim --test asset_manifest`: 3 passed (packs present, so the full file check ran).
- No Rust source changed, so the earlier cargo build/clippy/test results from the implementer and reviewer
  still apply.

`git status --short`: only `tools/fetch_assets.py`, `tools/qa/scenarios/t3.py` and this file. No binary
assets are tracked.

children: 0 launched / 0 reported.
