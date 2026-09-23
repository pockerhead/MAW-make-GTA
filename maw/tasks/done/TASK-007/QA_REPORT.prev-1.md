# QA_REPORT — TASK-007 (GDD T6 shooting + damage numbers + fixer round)

Tested commit: `95eeb56` (branch `feature/t06-shooting`), base for comparison `361b43a`.

## Disconfirmation (done first)

Counter-example I went looking for: **input raised while the game is not in `Playing` leaks into the first
`Playing` tick**, which is the same class of bug as B1 that this task was asked to close.
`write_action_intent` (`src/input/mod.rs`) runs in `Update` with no state gate, and the cursor stays
captured during `Wasted` (only `cursor_toggle` writes `CursorCaptured`). A left click on the Wasted
screen sets `ActionIntent.fire_requested = true`. `tick_loadouts` / `fire_weapons` are `PlayingSystems` +
`Without<Dead>`, so nothing consumes the latch. `release_held_actions` on `OnEnter(Wasted)` clears only
`fire_held` / `aiming`, not latches raised later.

**It held. The bug is real (B1 below):**
- Headless: `scratch/qa/qa_probe.rs::trigger_pulled_during_wasted_does_not_fire_after_respawn` →
  pistol magazine 12 → **11** right after respawn (a shot fired at the hospital).
- Runtime (BRP, release `dev`): `scratch/qa/probe/probe.json` `wasted_ghost_shot` →
  `magazine_before 12, during_wasted 12, after_respawn 11`.

## 1. Environment

- No docker-compose, no dev server. Existing infra: `cargo test` + BRP driver `tools/qa/brp.py`.
- Windowed runs: release build with `--features dev` (BRP on 127.0.0.1:15702), city seed 1.
- Services started: only the game process launched by `brp.py` (4 launches, each ended by `brp_extras/shutdown`;
  `tasklist` shows no `gta_like` left). No containers.
- Reproduce:
  ```
  cargo build; cargo clippy -- -D warnings
  cargo clippy -p gta_sim --tests -- -D warnings; cargo clippy -p gta_like --tests --features dev -- -D warnings
  cargo test -p gta_sim; cargo test -p gta_like --bin gta_like; cargo test -p citygen
  cargo tree -p gta_sim -e normal -i bevy_render; cargo tree -p gta_sim -e features -i bevy_render
  python tools/qa/scenarios/t6.py --out maw/tasks/in_progress/TASK-007/scratch/qa/t6
  python tools/qa/scenarios/t5.py --out maw/tasks/in_progress/TASK-007/scratch/qa/t5
  python maw/tasks/in_progress/TASK-007/scratch/qa/qa_probe_runtime.py maw/tasks/in_progress/TASK-007/scratch/qa/probe
  # headless probe: copy scratch/qa/qa_probe.rs to crates/gta_sim/tests/, cargo test -p gta_sim --test qa_probe
  ```

## 2. Test results

Existing suites (all green, zero failures, so zero new failures against the base):
- `cargo build`: ok. `cargo clippy -- -D warnings`: clean. Clippy `--tests` for `gta_sim` and for `gta_like --features dev`: clean.
- `cargo test -p gta_sim`: lib 11, anim_state 4, asset_manifest 3, city 6 (+1 ignored, pre-existing), config 14,
  health 6, jump 3, movement 4, respawn 4, shooting 14, terrain 2. All ok.
- `cargo test -p gta_like --bin gta_like`: 23 passed.
- `cargo test -p citygen`: ok (not touched).
- `cargo tree -p gta_sim` `-e normal` and `-e features` `-i bevy_render`: empty.

Flip-RED done by QA (sha256 of `crates/gta_sim/src/combat/hitscan.rs` before and after restore:
`fb964fef392263e08624f54559ee7638516a4ac64498e358148e390cdc3b7c9a`, identical):
| Perturbation | Gate | Result |
|---|---|---|
| headshot multiplier replaced by `1.0` | `head_sensor_doubles_damage` | RED (others green) |
| ray filter without `GameLayer::World` | `wall_between_muzzle_and_target_blocks` | RED (others green) |

New QA tests (headless probe, `scratch/qa/qa_probe.rs`, run as a temporary test file and then removed from the tree):
| Test | Result |
|---|---|
| trigger pulled during Wasted must not fire after respawn | **FAIL** (magazine 12 → 11), bug B1 |
| empty magazine auto-reloads a partial reserve (0/5 → 5/0 after `reload`) | PASS |
| switching weapon mid-reload cancels it (3/20 stays 3/20) | PASS |
| shotgun at 17.5 m: every pellet within the falloff band of its own distance (head ×2 included), health drop == sum of numbers | PASS (hits 11, 12 head; 6 body) |
| SMG spread peaks above base after a burst and returns to base after 2 s | PASS (peak 5.8°, rest 3.0°, base 3.0°) |

Runtime:
- `t6.py` → exit 0 (`scratch/qa/t6/summary.json`, `scratch/qa/t6_run.log`): body drop 26 == number 26 in [23, 28],
  magazine 12 → 11; head drop 52 == red number 52 in [45, 55]; SMG aimed burst `aiming true`, 30 → 17, fresh dummy
  100 → 0 (Dead); tracers alive around both burst captures (2/1, 1/1); reload → 30; 0 numbers left after 2 s; no log errors.
- `t5.py` → exit 0 (`scratch/qa/t5/`), respawn at the hospital, 100 HP, no log errors.
- QA probe `qa_probe_runtime.py` → `scratch/qa/probe/probe.json`: pistol/SMG/shotgun selected by keys 2/3/4;
  shotgun blast at 4 m: 8 numbers (8,8,8,9,9,9,8,8), sum 67 == health drop 67, magazine 6 → 5; Wasted ghost shot
  reproduced. Diagnostics: 145 FPS average, 6.9 ms average frame, present mode `Fifo` (vsync, so this is display
  refresh, not frame cost; no perf criterion in this task).

Screenshots I looked at:
- `scratch/qa/t6/body_hit.png`: normal view, white "26" on the middle dummy, thin tracer from the right hand to the
  target, dark gun box at the right hand. The player's head is left of centre and does not cover the target.
- `scratch/qa/t6/crit_hit.png`: larger red "52 CRIT" over the dummy's head, tracer visible.
- `scratch/qa/t6/aim_burst_1.png`, `aim_burst_2.png`: aim view (crosshair arms), the body takes the left third
  (x about 175..510 of 1280), the dummy at the crosshair is fully visible, the fan of tracers from the gun to the
  target is readable, muzzle glow at the barrel, hit marker "×", stacked numbers 11-13.
- `scratch/qa/probe/pistol_side.png`: right arm forward holding the gun, left arm down (one-hand pose).
- `scratch/qa/probe/smg_side.png`, `shotgun_side.png`: both arms forward, gun in the hands, pointing forward.
- `scratch/qa/probe/shotgun_blast.png`: fan of 10 tracers from the barrel; the 8 numbers pile up into one unreadable
  blob over the head (owner item).

## 3. Acceptance criteria

| Criterion | Test performed | Result |
|---|---|---|
| Headless: ray at a dummy at 10 m → table damage | `pistol_hits_dummy_at_10m_for_table_damage` green; band derived from `roll_damage` | PASS |
| Headless: wall between muzzle and target blocks | `wall_between_muzzle_and_target_blocks` green; QA flip-RED (no World layer) → RED | PASS |
| Headless: head sensor → ×2 | `head_sensor_doubles_damage` green; QA flip-RED (multiplier 1.0) → RED | PASS |
| Headless: reload by time | `reload_takes_configured_time` green; QA partial-reserve and cancel-on-switch probes pass | PASS |
| Headless: shotgun gives 10 rays | `shotgun_fires_ten_pellets` green; QA 17.5 m probe: 10 traces per blast | PASS |
| Headless: spread grows in a series and shrinks over time | `spread_grows_in_series_and_recovers` green; QA probe 5.8° → 3.0° | PASS |
| Runtime t6.py via brp.py (teleport, move_mouse, send_mouse_button, Health, ammo, screenshot with tracer + hit marker) | ran it myself, exit 0; PNGs looked at | PASS |
| Owner-run feel checklist recorded | section 6 below | PASS (recorded; owner run pending) |
| Every new tuning value in its data file | grep of added `const` in the diff: only paths, `Weapon::ALL`, sample rate, golden angle, mask group ids, test fixtures; client literals are math (0.0/1.0/2.0, halves) | PASS |
| build, clippy -D warnings, test gta_sim (+citygen) green | ran | PASS |
| Existing tests pass | all suites green | PASS |
| B1: damage queued during Wasted does not reach the respawned player | `damage_queued_during_wasted_is_dropped` green | PASS for damage; **sibling leak B1 below (fire input)** |
| Damage variance in band, not all equal, shotgun pellets each roll, seeded RNG | `damage_variance_stays_in_band_and_varies` green; runtime 11/12/13 SMG, 8/9 shotgun | PASS |
| Hit message (point, damage, headshot) sim → client, client never recomputes | `DamageDealt` written in `fire_weapons`, number text from `damage` (`label_matches_message`), runtime drop == number | PASS |
| Floating number camera-facing, integer | UI label at `world_to_viewport`; screenshots | PASS |
| Headshot: larger, red, "CRIT" from strings.ron | `label_matches_message`; `crit_hit.png` | PASS |
| Juice animation from juice.ron, despawn, no leak | `pose_worked_examples`, `numbers_despawn_after_lifetime`; runtime 0 left after 2 s | PASS (look = owner) |
| Runtime screenshot: body number and red CRIT | `body_hit.png`, `crit_hit.png` | PASS |
| Aim camera does not cover the crosshair; target at crosshair fully visible | `aim_burst_1/2.png` | PASS |
| Tracer readable, shown in a t6 screenshot | `body_hit.png`, `crit_hit.png`, `aim_burst_1.png` | PASS |
| Held gun visible in the hand, normal + aim; placement in render.ron | `body_hit.png` (small, right hand), `aim_burst_*.png`, side views; `render.ron` `hand_offset/yaw/pitch` | PASS (normal view gun is small: owner item) |
| Flash and tracer start at the visible barrel | `src/vfx/mod.rs` `barrel_end`; tracers leave the gun in the PNGs | PASS |
| Arms play holding clip per weapon over locomotion (masks) | `armed_animator_layers_arm_clips`, `graph_nodes_follow_manifest_clips`; side PNGs: pistol one hand, SMG/shotgun two hands | PASS |
| Shoot clip plays once per shot | `armed_animator_layers_arm_clips` (behaviour gate); not visible in a still | PASS headless, look = owner |
| Client gate: arm clip per weapon / unarmed (pure table) | `arm_pose_per_weapon` | PASS |

## 4. Bugs found

**B1 — Medium — a trigger pull during Wasted fires a ghost shot on respawn.**
- Repro (runtime): pistol held, kill the player (BRP `Health.current = 0`), click LMB on the Wasted screen, wait for
  `Playing` → magazine drops by one at the hospital (`scratch/qa/probe/probe.json`: 12 → 12 during Wasted → 11).
  Headless repro: `scratch/qa/qa_probe.rs::trigger_pulled_during_wasted_does_not_fire_after_respawn`.
- Expected: input given while `Wasted` has no effect after respawn (same rule as B1 damage). Actual: the
  `fire_requested` latch survives `Wasted` and `fire_weapons` consumes it on the first `Playing` tick. The same holds
  for `reload_requested`, `select` and `cycle` (a wheel scroll on the Wasted screen switches the weapon at respawn).
- Cause: `write_action_intent` has no state gate and `release_held_actions` runs only on `OnEnter(Wasted)`; the sim
  systems that clear latches are `PlayingSystems` + `Without<Dead>`.
- Suggested fix (small): reset the player's `ActionIntent` to default on `OnExit(GameState::Wasted)` in the sim
  (next to `drop_queued_damage`), so the headless app gates it; turn the QA probe into a gate in
  `tests/respawn.rs` and flip it (remove the reset → 11).
- Why it matters: silent today (one round lost, a flash at the hospital), but once T8+ puts NPCs near the hospital it
  becomes damage and wanted heat from an input the player gave while dead.

**L1 — Low (owner feel) — cooldown and bloom are shared across weapons.** `Loadout.cooldown` / `bloom_deg` are not
per gun: after a shotgun blast, switching to the pistol blocks firing for up to 0.9 s; SMG bloom 4° carries into the
pistol (above the pistol's `max_bloom_deg` 3°) until it decays. Not a criterion; worth an owner look.

**L2 — Low (owner) — shotgun numbers pile up.** 8-10 numbers per blast land in one unreadable blob
(`scratch/qa/probe/shotgun_blast.png`); `drift_px` 24 is too small for 10 labels at once. The plan left "sum per target"
to the owner.

**L3 — Info — GDD drift.** `camera.ron` (shoulder 0.8 / aim 1.0, aim distance 2.6) and `juice.ron` tracer (0.1 s, 5 cm)
differ from GDD §3.2 / §8. The fixer says so; owner decides whether to update the GDD.

**L4 — Info — deterministic roll sequence.** `CombatRng` is seeded 0 at startup, so every launch gives the same damage
sequence (first pistol body shot 26, first headshot 52 in both the implementer's and my runs). Hits still vary within a
session, which is what the owner asked for.

## 5. Verdict

**NEEDS_FIXES.** Every listed acceptance criterion passes headless and at runtime (I ran t6/t5 myself and looked at
all PNGs), and the fixer's four owner findings (aim framing, tracer, visible held gun, arm poses) hold in my own
screenshots. The one blocker is B1: an input leak from `Wasted` into `Playing` of exactly the class this task was
asked to close for damage. The fix is a few lines plus a gate. After that fix the verdict becomes
SHIP-PENDING-RUNTIME with the owner checklist below.

## 6. Owner checklist (subjective, run by the owner)

`cargo run --features fast`, walk to the central park (3 dummies 10 m along −Z of its centre, 6 pickups in a row):
- [ ] Pick up pistol / SMG / shotgun and ammo; keys 1-4 and the wheel switch; R reloads; ammo counter right.
- [ ] Fire with and without RMB: recoil noticeable, not nauseating; placeholder noise-burst sound tolerable.
- [ ] Tracer (0.1 s, 5 cm) and muzzle flash from the barrel readable, not too heavy.
- [ ] Hit marker visible; red when the hit kills.
- [ ] Aim camera (shoulder 1.0 m, 2.6 m, FOV 55°): body left, target clear. Normal camera shoulder 0.8 m acceptable?
- [ ] Gun visible enough from behind in normal view (it reads as a small dark box at the right hand).
- [ ] Pistol held in the right hand, SMG/shotgun in both; a twitch on each shot; legs keep walk/run while armed.
- [ ] Strafe while aiming comfortable; no sprint while RMB is held.
- [ ] Dummies drop to 0, lie 3 s, come back.
- [ ] Damage numbers: size, pop/rise/fade timing, drift spacing; CRIT readable and red; shotgun: 10 numbers per
      blast or one sum per target (L2)?
- [ ] Weapon switch right after a shotgun blast: is the shared cooldown/bloom (L1) acceptable?
Knobs: `weapons.ron`, `aim.ron`, `camera.ron`, `juice.ron`, `mix.ron`, `strings.ron`, `locomotion.ron`,
`render.ron` (`weapons.hand_*`, `held_size`), `visual.ron` (`arm_joints`, `hand_joint`).

## Cleanup / tree

- Temporary test `crates/gta_sim/tests/qa_probe.rs` removed from the tree (copy kept in `scratch/qa/qa_probe.rs`).
- Sabotaged `hitscan.rs` restored by `git checkout`, sha256 identical.
- `git status --short`: only `maw/tasks/in_progress/TASK-007/log.jsonl` (+1 qa decision) and this file. No binaries
  outside ignored `scratch/`. No game process left. No containers started.

children: 0 launched / 0 reported.
