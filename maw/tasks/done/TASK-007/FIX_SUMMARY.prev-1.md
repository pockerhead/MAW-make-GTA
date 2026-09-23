# FIX_SUMMARY — TASK-007 (fixer)

Inputs: `IMPL_REVIEW.md` (3 issues), the orchestrator's owner-proxy findings in `TASK_FINAL.md` (aim camera,
tracer), and two owner findings added during this round (no gun in the hands; weapon holding pose).

## Preflight: the claim checked first

Most dangerous prescription if applied verbatim: review issue 3, "require an accepted aim-and-tracer capture"
inside `t6.py`. A hard image/pixel assertion on a 60 ms transient would make the runtime gate flaky (screenshot
latency vs effect lifetime) and put a visual judgement into a numeric gate, against the project rule that visuals
are owner-gated. Checked in `tools/qa/scenarios/t6.py:284-300`: the diagnosis is real (the script passed while both
screenshots failed the owner-proxy items), the prescription is not taken as written. What was done instead is in
Fixed #3.

## 1. Fixed

1. **Review #1 / owner-proxy: aim camera hides the target** (real; confirmed in `scratch/qa_t6/aim_burst_1.png`
   and my own capture `scratch/fixer_aim/before/aim_idle_10m.png`: the Kenney head spans x 220..615 of 1280, the
   crosshair is at 640). Data-only fix in `assets/camera/camera.ron`: `aim_shoulder_offset 0.55 -> 1.0`,
   `aim_distance 2.0 -> 2.6` (tried 0.9/2.4 first: head edge ~90 px from the crosshair; 1.0/2.6 gives ~110 px).
   Rejected: fading/hiding the player model (alpha materials on the glTF, more code for the same result).
   Evidence: `scratch/fixer_aim/v2/aim_burst_5m_1.png`, `aim_burst_10m_0.png`, `scratch/qa_t6/aim_burst_1.png` —
   body in the left third, the dummy at the crosshair fully visible. Note: GDD §3.2 table still says 0.55 / 2.0;
   `camera.ron` is the single source and the owner decides (checklist below).
2. **Review #2 / owner-proxy: tracer invisible** (real). Root cause found in the screenshots: the tracer's muzzle
   end projected inside the player's head silhouette, and 2 cm width. Fixes: the wider aim shoulder removes the
   occlusion; `assets/juice/juice.ron` tracer `width 0.02 -> 0.05`, `seconds 0.06 -> 0.1` (>= SMG `fire_interval`
   0.08, so a burst always shows one; GDD §8 says 60 ms, owner decides). With the held-gun fix (#5) the tracer and
   flash now start at the visible barrel. Evidence: `scratch/qa_t6/aim_burst_1.png` (tracer from the gun to the
   target, flash glow at the barrel, hit marker, damage numbers), `crit_hit.png` (normal view, tracer from the gun).
3. **Review #3: t6 can pass with both visual failures** (diagnosis real, prescription recomputed). `t6.py` now
   brackets each burst screenshot with a BRP count of live `Name("Tracer")` entities (`tracers`, `burst_capture`)
   and fails if no capture had tracers alive on both sides — a liveness check that a tracer was on screen when the
   PNG was taken; the look (framing, tracer readability) stays an eye review, which I did (owner-proxy, PNGs above).
   Burst lengthened 800 -> 1200 ms and captures spaced 0.25 s (a 0.17 s spacing hit "A screenshot capture is
   already in progress" once in my probe). Flip-RED: `tracer.seconds 0.001` -> `AssertionError: no burst capture
   was taken while tracers were alive ... tracers_before 0, tracers_after 0`; restored -> PASS.
4. **Owner: no gun visible in the hands** (real). Verified cause: the held box was a body child at
   `muzzle_offset + Z*len/2` = (0.25, 0.35, -0.275) m from the body centre, i.e. 1.40 m above the feet; the Kenney
   head mesh (from the GLB: raw x ±0.227, y 0.343..0.671, z ±0.17, ×2.68 scale) spans x ±0.61, y 0.92..1.80,
   z ±0.46 m, so the gun was inside the head. Fix (`src/visuals/weapons.rs`): `attach_held_gun` (now an `Update`
   system, was an `On<Add, Player>` observer) parents the gun to the model's hand joint (`hand_joint: "arm-right"`
   in `visual.ron`, found by `Name` once the glTF instance exists) with `render.ron` `hand_offset (-0.87, -0.04,
   0.11)`, `hand_yaw_deg -60`, `hand_pitch_deg 0` (derived from the clip data: `holding-*` rotates `arm-right` by
   +60° about Y, the arm mesh runs along the joint's −X; −60° points the barrel at model forward), scale
   `1/model_scale` to undo the joint's ×2.68. `held_size (0.08,0.12,0.35) -> (0.12,0.2,0.55)` (the first size was
   hidden inside the Kenney fist). `HeldGun { owner, barrel }` is exported; `src/vfx/mod.rs` starts the flash and
   the tracer at `GlobalTransform × barrel` of the shooter's visible gun, falling back to the sim muzzle
   (gameplay rays unchanged). Normal-view visibility needed the camera too: from straight behind a forward-pointing
   gun is seen end-on behind a 1.2 m-wide head, so `camera.ron` `shoulder_offset 0.45 -> 0.8` (also stops the
   normal-view player covering the crosshair, as in the implementer's `body_hit.png`). Evidence (looked at):
   `scratch/fixer_gun/v2/pistol_side.png`, `smg_side.png` (gun in the fist, pointing forward),
   `v2/pistol_aim.png`, `v2/smg_aim_shot.png` (gun right of the body, flash + tracer from the barrel),
   `v3/pistol_normal.png`, `v3/smg_normal.png` (normal view: gun beside the body, still small from behind — see
   Skipped/owner).
5. **Owner: weapon holding pose** (real: arms used idle/run clips). Bevy 0.19.1 mask API verified in
   `bevy_animation-0.19.1/src/graph.rs:81-130,492,672` (bit N set = node does not animate group N; a target in no
   group is animated by every node; `Blend` normalises weights, `animation_curves.rs:660-664`). Node masks live in
   the shared graph asset, so toggling them per character would arm the dummies too. Graph now holds: `nodes`
   (locomotion, mask 0, unarmed), `legs` (same clips, mask ARMS), `hold`/`shoot` (`holding-right`/`-both`,
   `holding-right-shoot`/`-both-shoot`, mask BODY). Mask groups are filled from the spawned nodes'
   `AnimationTargetId` + `Name` at model ready (`arm_joints` in `visual.ron`; everything else BODY). Per animator:
   armed -> locomotion transitions to `legs[state]`, arms loop `hold[pose]`; a `ShotFired` of that character
   restarts `shoot[pose]` once, which hands back to hold when finished; unarmed -> full-body locomotion, arm clip
   stopped. `arm_pose`: Pistol -> one hand, SMG/Shotgun -> two hands. `visual.ron` clip/joint names are resolved and
   validated against the manifest rig (`CharacterClips { locomotion, hold, shoot }`). Evidence:
   `fixer_gun/v2|v3/pistol_side.png` (right arm forward, left arm idle), `smg_side.png` (both arms forward), aim and
   burst captures above (arms forward while the legs keep their clip).

New / re-anchored client gates (`cargo test -p gta_like --bin gta_like`), flip-RED via
`scratch/fixer_flip_red.py` (log `scratch/fixer_flip_red.log`):

| Gate | Class | Perturbation | Result |
|---|---|---|---|
| `arm_pose_per_weapon` | correctness (table) | Pistol -> TwoHands | RED |
| `graph_nodes_follow_manifest_clips` (re-anchored: all 20 nodes, clip + mask) | correctness | hold clips unmasked; legs keep arm mask 0 | RED ×2 |
| `armed_animator_layers_arm_clips` | behaviour | shot ignored; armed keeps full-body locomotion; old arm clip not stopped | RED ×3 |
| `unknown_arm_joints_are_rejected` | config | joint check removed | RED |
| `character_visuals_reference_manifest_rig` (re-anchored: hold [13,15], shoot [16,18]) | config | one-hand hold = `holding-left` (index 14) | RED |
| t6 tracer liveness | liveness | `tracer.seconds 0.001` | RED |

Baseline of the same runs: GREEN. Not headless-gated (runtime only, by screenshots): return from shoot to hold
needs `AnimationPlugin` to advance clips; joint lookup and gun placement need the real GLB.

## 2. Skipped / left to the owner

- Review "missing coverage: owner-run checklist belongs to QA_REPORT.md" — agreed, not mine; items below.
- No pixel/image assertion in `t6.py` (see preflight): look is owner-gated by project law.
- Normal view from straight behind: the gun reads as a small dark box at the right hand. The Kenney head is
  1.2 m wide and the gun points away from the camera; more would need a larger normal shoulder offset or a
  camera pitch change — owner call via `camera.ron`.
- GDD §3.2 (0.45 / 0.55 m shoulder, 2.0 m aim distance) and §8 (tracer 60 ms) now differ from `camera.ron` /
  `juice.ron`. I did not edit the GDD (owner document).
- Shoot clips also rotate `torso`; torso stays with locomotion by the owner's spec ("legs/torso keep
  walk/run/..."), so that recoil twitch of the torso is not shown.

## 3. Test results

- `cargo test -p gta_like --bin gta_like` -> `test result: ok. 23 passed; 0 failed` (20 before + 3 new).
- `cargo test -p gta_sim` -> all suites ok (lib 11, anim_state 4, asset_manifest 3, city 6 + 1 ignored, config 14,
  health 6, jump 3, movement 4, respawn 4, shooting 14, terrain 2); sim code untouched.
- `cargo build` -> ok. `cargo clippy -- -D warnings` -> clean; `cargo clippy -p gta_like --tests --features dev --
  -D warnings` -> clean.
- `cargo tree -p gta_sim -e normal -i bevy_render` -> empty ("nothing to print"). `python tools/qa/tree_check.py`
  -> passed.
- `python tools/qa/scenarios/t6.py --out maw/tasks/in_progress/TASK-007/scratch/qa_t6` -> exit 0: body drop 26 ==
  number, head drop 52 == red CRIT number, SMG burst tracers alive at both captures (1/2, 2/2), reload 30, 0 numbers
  left, no log errors (`scratch/fixer_t6_run.log`, `scratch/qa_t6/summary.json`).
- `python tools/qa/scenarios/t5.py --out maw/tasks/in_progress/TASK-007/scratch/qa_t5` -> exit 0, no log errors.
- No game process left running (`tasklist` checked after every run).

Owner checklist additions (for QA_REPORT.md):
- [ ] Aim camera (shoulder 1.0 m, 2.6 m, FOV 55°): body on the left, target clear at the crosshair; acceptable?
- [ ] Normal camera shoulder 0.8 m (was 0.45): acceptable, and is the gun visible enough from behind?
- [ ] Gun in the right hand for pistol, both hands forward for SMG/shotgun; shoot twitch on each shot.
- [ ] Tracer 0.1 s / 5 cm from the barrel and the muzzle flash at the barrel: readable, not too heavy?
- Knobs: `camera.ron` (shoulder/aim), `juice.ron` (tracer), `render.ron` `weapons.hand_*` and `held_size`,
  `visual.ron` (`arm_joints`, `hand_joint`, `one_hand`, `two_hands`).

Changed files: `assets/camera/camera.ron`, `assets/juice/juice.ron`, `assets/world/render.ron`,
`assets/character/visual.ron`, `src/visuals/{character,character_config,character_gate,config,mod,weapons}.rs`,
`src/vfx/mod.rs`, `tools/qa/scenarios/t6.py`; task dir: `log.jsonl` (+6 decision entries), `PCTX_PROPOSALS.md`
(+1 entry), `FIX_SUMMARY.md`. Scratch probes: `fixer_aim_probe.py`, `fixer_gun_probe.py`, `fixer_flip_red.py`.
No binary assets added outside ignored `scratch/`.

children: 0 launched / 0 reported.
