# TASK-034 IMPL_SUMMARY — hide the player model when the camera is pushed into it

Cost of error: the owner sees it on the first frame. So this is the mechanism, one client gate with flips, and a runtime screenshot check. Nothing more.

## 1. What was implemented

Mechanism: **hide, not fade.** A new camera-domain system `hide_player_near_camera` sets `Visibility::Hidden` on the **Player body root** when the camera-to-body distance drops below `hide_player_distance`. It sets `Visibility::Inherited` again only once the distance goes past `hide_player_distance + hide_player_band` (hysteresis). The hysteresis state is the current `Visibility` itself, so no extra field or component is needed. The glTF model (`CharacterModel`) and the held gun/bat hide together with the body, because the gun is parented to the hand joint inside the model (`visuals/weapons.rs::attach_held_gun`) and inherits visibility. The decision and the rejected alternatives are in `log.jsonl`:
- material-alpha fade: rejected. The gun materials are shared by every armed NPC, and fading would need per-player material clones plus blend-mode sorting.
- writing `CharacterModel` Visibility: rejected. `visuals/vehicle.rs::hide_driver_model` already owns it, and two writers would fight.

Schedule: `PostUpdate`, `.after(follow_player).before(TransformSystems::Propagate)`. In Bevy 0.19.1 `VisibilitySystems::VisibilityPropagate` runs after `TransformSystems::Propagate` (`bevy_camera-0.19.1/src/visibility/mod.rs:504-508`). So the hide takes effect in the same frame the collision snaps the camera in, not one frame late. A `Single` with no match is skipped, not a panic (`bevy_ecs-0.19.1/src/system/system_param.rs:377`). That means apps without the visuals plugin, where the player has no `Visibility`, just skip the system.

Distance: `|camera.translation - player body Transform.translation|`. This is the same quantity the TASK-031 probe measures (`cam_distance`: CameraView origin to player position).

| File | Change |
|---|---|
| `assets/camera/camera.ron` | +4: `hide_player_distance: 1.4`, `hide_player_band: 0.3`, with a comment |
| `src/camera/config.rs` | +6: two fields, and both go into the existing `finite > 0` validation |
| `src/camera/mod.rs` | +25: the system, its registration, `#[cfg(test)] mod hide_gate;` (311 lines total) |
| `src/camera/hide_gate.rs` | new, 181 lines: gate G-C1 |

Where the values come from (worked from TASK-031 orbit tables and this run): the pushed-in poses sit at 0.50 m (90-150 deg), 0.96 m (180 deg) and 1.15-1.2 m (210 deg). The 210 deg one still covers a third of the frame in the TASK-031 shots. The aim camera sits at about 2.83 m, the open camera at about 3.9 m, and 240 deg at 2.2-2.6 m. So hiding below 1.4 and showing above 1.7 covers every pushed-in pose and never touches the aim or open camera.

## 2. Deviations / not implemented

- There is no fade: the model is hidden, which the spec allows ("fades out (or is hidden)"). No alpha data value was added, since there is no alpha.
- A hidden player also casts no shadow. GTA does the same, and I did not treat it as a defect.
- While hidden, tracers and muzzle flash start at the sim muzzle instead of the barrel. That is the existing fallback in `vfx::barrel_end`, which checks `InheritedVisibility` of the held gun.
- No plan file (small-fix). No PLAN_BLOCKED findings: every entity the spec names exists (`camera.ron`, `camera_wall_probe`, `tools/qa/brp.py`).

## 3. Test results

**Gate G-C1** `camera::hide_gate::player_hides_when_the_camera_is_pushed_into_it` (correctness, client crate).
- Composition: `MinimalPlugins` + `compose_sim` (test area) + the production `CameraPlugin`, plus stand-in resources (`CursorCaptured`, `GameSettings`, `CameraRecoil`, `CameraShake`) and a stand-in `Visibility` on the player. The visuals plugin normally inserts that `Visibility` with the model.
- After the player settles, time is frozen (`ManualDuration(0)`). The camera is then placed through `OrbitCamera.distance`, so the production `follow_player` puts it at an exact camera-to-body distance. The orbit distance is derived from the measured shoulder offset. There is a `GATE BROKEN` precondition if the measured distance misses the target by more than 0.02 m (for example, a wall behind the camera).
- Assertions, all from `camera.ron`:
  - the open camera shows the player;
  - `d = t - 0.2` hides it;
  - 12 frames alternating `t + b/2` and `t - 0.1` (crossing the threshold every frame) never show it;
  - `t + b + 0.1` shows it;
  - coming back to `t + b/2` from outside keeps it shown.
- Flip-RED (script: `scratch/flip_hide_gate.py`):
  - system unregistered: RED at `hide_gate.rs:156`, "camera at 1.1999999 m, left: Inherited, right: Hidden";
  - band removed (`Hidden => hide_player_distance`): RED, "camera hovering across 1.4 m inside the band showed the player: frame 0/2/4/6/8/10 at 1.55 m";
  - both restored, GREEN.
- Ran 3 times: `cargo test -j 2 -p gta_like --bin gta_like camera::hide_gate`, 3/3 ok.

**Suites:**
- `cargo test -j 2 -p gta_like --bin gta_like`: 82 passed, 0 failed.
- `cargo test -j 2 -p gta_sim -p citygen`: every test binary ok, 0 failed. That code was not touched; it was run for regression only.
- `cargo clippy -j 2 -p gta_like --all-targets -- -D warnings`: clean.
- `rustfmt --edition 2024 src/camera/mod.rs`: only my files changed.

**Runtime check** (seed 1, release `--features dev`, `--settings-id .qa`, screenshots taken one at a time). Script: `scratch/wall_probe_hide.py` (TASK-031 `camera_wall_probe` plus the player `Visibility` read over BRP at each step). Output: `scratch/runtime/s1/summary.json`, console log `scratch/runtime/s1_console.log`. The wall was at (-15.1, 1.2, -35.7).

| orbit | cam m | Visibility | shot |
|---|---|---|---|
| 0 | 3.84 | Inherited | `scratch/runtime/s1/shots/0010_0056s_hide_cam_0deg.jpg`: model visible against the wall |
| 90 | 0.50 | Hidden | `.../0012_0059s_hide_cam_90deg.jpg`: street, no model |
| 120 | 0.50 | Hidden | `.../0013_0061s_hide_cam_120deg.jpg`: street, no model |
| 150 | 0.52 | Hidden | `.../0015_0062s_hide_cam_150deg.jpg`: street, no model (TASK-031 had the brown-black frame here) |
| 180 | 0.96 | Hidden | `.../0016_0064s_hide_cam_180deg.jpg` |
| 210 | 1.15 | Hidden | `.../0017_0065s_hide_cam_210deg.jpg`: street, no model |
| 240 | 2.38 | Inherited | (no shot) |
| 270 | 3.69 | Inherited | `.../0019_0068s_hide_cam_270deg.jpg`: model visible, as before |

Hover at 120 deg: 30 BRP polls over 3 s were all `Hidden` at 0.50 m. The game log has no ERROR or panic lines. The game process was shut down afterwards.

## 4. How to verify manually

1. `cargo test -p gta_like --bin gta_like camera::hide_gate`
2. `python maw/tasks/in_progress/TASK-034/scratch/wall_probe_hide.py 1 <out>` and look at `<out>/shots/*hide_cam_*deg.jpg`.
3. Owner run: `cargo run --features fast`, stand with your back to a building and orbit the mouse. When the camera hits the wall the character disappears, and it comes back once the camera pulls away (past 1.7 m), with no blinking. Aiming (RMB) in the open never hides it. Tuning is in `assets/camera/camera.ron` (`hide_player_distance`, `hide_player_band`).

For the owner: whether hiding feels right, compared with a fade, is decided by feel. If a fade is wanted, it is a separate task (material alpha, per-player materials).
