# TASK-034: Hide the player model when the camera is pushed into it

Type: bugfix
Mode: small-fix
Priority: medium
Branch: bugfix/camera-near-wall-occlusion
Domains: bevy-ecs, gates, game-design

## Description
TASK-031 playtest M2 (confirmed on 6 walls, seeds 1/7/42): with the player's back to a wall, camera collision correctly stops the camera at the wall, but in the 90-150° sector the camera ends up 0.50 m from the player and the character model fills the whole frame (brown-black screen, only HUD visible; `D:/test-gta-like/maw/tasks/done/TASK-031/scratch/sessions/s1_tourist/shots/0026_0107s_wall_cam_150deg.jpg`). Fighting next to a wall is routine, so the player loses sight.

Fix in the GTA way: when the camera-to-player distance drops below a data threshold (in `assets/camera/camera.ron`), the player model fades out (or is hidden) and comes back when the camera pulls away, with no flicker at the threshold (hysteresis or a fade band). Camera collision itself stays as is: the camera must not go through walls. Repro: `camera_wall_probe` in `D:/test-gta-like/maw/tasks/done/TASK-031/scratch/tools/personas.py`.

Cost of error: the owner sees it on the first frame → mechanics + one honest gate + a screenshot check.

## Acceptance criteria
- [ ] Threshold and fade band are data in `camera.ron`, not new consts.
- [ ] Client presentation gate: camera distance below the threshold → the player model is not visible (or alpha ≤ the data value); above the band → fully visible; a distance oscillating around the threshold does not toggle every frame. Flip-tested RED with the fade disabled.
- [ ] Runtime check: the wall probe at 90/120/150° on seed 1 shows the scene, not the model (screenshots cited in QA_REPORT.md); at 0° and 270° the model is visible as before.
- [ ] Existing tests pass.
