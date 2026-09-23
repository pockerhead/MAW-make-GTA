# QA_REPORT — TASK-007 round 2 (GDD T6 shooting + damage numbers)

Tested commit: `a667902` (fix commit `0c7277f`), branch `feature/t06-shooting`. Round 1: `QA_REPORT.prev-1.md`
(NEEDS_FIXES for B1, plus L1, L2, L4).

## Disconfirmation (done first)

The counter-example I went after: **a latch raised during Wasted still acts after respawn** despite the new
`drop_queued_input`. Two routes seemed possible: (a) the reset runs too late (after the first Playing
`FixedUpdate`), or (b) the client keeps its own latch and raises it again after the reset.
- (a) `drop_queued_input` runs on `OnExit(GameState::Wasted)` in `StateTransition`, which runs before
  `RunFixedMainLoop`. That matches the fixer's citation of the pinned bevy_state/bevy_app sources, and the
  headless tests below show it working.
- (b) `src/input/mod.rs:168-220` `write_action_intent` has no client-side latch. `fire_requested`, `reload`,
  `select` and `cycle` come from per-frame `ActionEvents::START` / wheel values. The only `Local` is
  `wait_release`, which can only suppress firing, never cause it.
- My probe raises the latches on **every** Wasted frame, including the last one before the switch:
  `fire_requested`, `reload_requested` and `cycle += 1`. Test `latches_raised_every_wasted_frame_are_dropped`
  → pistol 12/12, held Pistol, no reload. **The fix holds.**
- Runtime: I pressed LMB, `Digit4` and `KeyR` on the Wasted screen. After respawn: magazine 12 → 12 → 12, held
  `Pistol` (`scratch/qa/probe2/probe.json`).

## 1. Environment

- No docker-compose, no dev server. I used cargo tests plus the BRP driver `tools/qa/brp.py` (release build,
  `--features dev`, JSON-RPC 127.0.0.1:15702, city seed 1). No containers or mocks.
- The owner was running `cargo run --features fast` (PID 17964, debug exe) when I started. Because of it, `cargo build`
  compiled everything but could not replace `target/debug/gta_like.exe` (os error 5). I did not kill the owner's
  process. It later exited by itself. The release build used by the scenarios built and ran fine.
- Reproduce:
  ```
  cargo build; cargo clippy -- -D warnings
  cargo clippy -p gta_sim --tests -- -D warnings; cargo clippy -p gta_like --tests --features dev -- -D warnings
  cargo test -p gta_sim; cargo test -p gta_like --bin gta_like; cargo test -p citygen
  cargo tree -p gta_sim -e normal -i bevy_render
  # headless probe: copy scratch/qa/qa_probe.rs to crates/gta_sim/tests/, cargo test -p gta_sim --test qa_probe, remove it
  python tools/qa/scenarios/t6.py --out maw/tasks/in_progress/TASK-007/scratch/qa/t6
  python tools/qa/scenarios/t5.py --out maw/tasks/in_progress/TASK-007/scratch/qa/t5
  python maw/tasks/in_progress/TASK-007/scratch/qa/qa_probe_runtime.py maw/tasks/in_progress/TASK-007/scratch/qa/probe2
  ```

## 2. Test results

Existing suites. Every test passed, so there are zero new failures (round 1 also had none):
- `cargo build`: compile ok (exe copy blocked by the owner's running game, see above). `cargo clippy -- -D warnings`
  and both `--tests` clippy runs: clean. `cargo tree -p gta_sim -e normal -i bevy_render`: nothing to print.
- `cargo test -p gta_sim`: lib 11, anim_state 4, asset_manifest 3, city 6 (+1 ignored, pre-existing), config 14,
  health 6, jump 3, movement 4, respawn 5, shooting 16, terrain 2. All ok.
- `cargo test -p gta_like --bin gta_like`: 24 passed. `cargo test -p citygen`: ok.

QA flip-RED on round-2 gates. The perturbations differ from the fixer's. Each file was restored with
`git checkout`, and its sha256 matched before and after:
| Gate | Perturbation | Result | sha256 after restore |
|---|---|---|---|
| `respawn::input_raised_during_wasted_is_dropped` | `drop_queued_input` resets everything except `select` | RED (respawn.rs:306), others green | `ed5e5fe5…beef` wasted.rs, identical |
| `shooting::combat_rng_follows_city_seed` | combat seed `seed.min(1)` (seeds 1 and 2 collide) | RED (shooting.rs:697), 15 others green | `d4bdd7ee…745c` lib.rs, identical |
| `damage_numbers_gate::pellets_show_one_sum_per_shot_and_target` | merged `headshot` keeps the first pellet's flag instead of any | RED (gate:181), 23 others green | `86b47d6f…bdee` damage_numbers.rs, identical |

QA headless probe `scratch/qa/qa_probe.rs`. It ran as a temporary `crates/gta_sim/tests/qa_probe.rs` that I
then removed. 8/8 pass:
| Test | Result |
|---|---|
| round 1: trigger pulled during Wasted must not fire after respawn | PASS (12 → 12; it was 11 in round 1) |
| round 1: empty magazine auto-reloads partial reserve / switch cancels reload / shotgun falloff band at 17.5 m / spread returns to base | PASS (hits 11, 12, 6; peak 5.8° → 3.0°) |
| **new** latches (fire, reload, wheel) raised on every Wasted frame incl. the last | PASS |
| **new** L1: select pistol + fire in the same tick right after a shotgun blast → pistol fires with pistol spread (2.0°); the holstered shotgun recovers cooldown and bloom to 0 within 1.75 s, then fires | PASS |
| **new** L2 sim: SMG burst, every hit has its own shot id | PASS (ids 1..7, one miss) |

Runtime (release `dev`, seed 1, run by me):
- `t6.py` → exit 0 (`scratch/qa/t6/summary.json`, `scratch/qa/t6_run.log`): body 27 == number 27 (magazine 12 → 11);
  head 51 == red "51 CRIT"; SMG aimed burst `aiming true`, 30 → 17, dummy 100 → 0 Dead, tracers alive at both
  captures; reload 30; **shotgun blast 100 → 19, exactly one number 81 == drop 81**; 0 numbers left; no log errors.
  First hits 27/51 (seed 0 gave 26/52), so L4 reaches the running game.
- `t5.py` → exit 0 (`scratch/qa/t5/`), no log errors.
- `qa_probe_runtime.py` (extended with Digit4 + KeyR during Wasted) → `scratch/qa/probe2/probe.json`: keys 2/3/4
  select pistol/SMG/shotgun; shotgun blast one number 73 == drop 73; ghost shot gone, select/reload latches from
  Wasted dropped. Diagnostics: 144 FPS average, 6.94 ms average frame time, `Fifo` (vsync, so this shows the
  display refresh, not frame cost; this task has no performance criterion).
- `tasklist` after every run: no `gta_like` left.

Screenshots I looked at:
- `scratch/qa/t6/body_hit.png`: white "27" on the middle dummy, hit marker, thin tracer from the right-hand gun.
- `scratch/qa/t6/crit_hit.png`: larger red "51 CRIT" at head height, tracer visible.
- `scratch/qa/t6/aim_burst_1.png`, `aim_burst_2.png`: aim view. The body is in the left part of the screen and the
  dummy at the crosshair is fully visible. Tracers go from the gun to the target. Stacked numbers 11/13, which vary.
- `scratch/qa/t6/shotgun_blast.png`, `scratch/qa/probe2/shotgun_blast.png`: fan of pellet tracers, **one** label
  ("81" / "73") instead of round 1's blob. The label sits on the "×" hit marker, so the marker cuts through the digits
  (still readable; owner item).
- `scratch/qa/probe2/after_respawn.png`: at the hospital, pistol in hand, HUD 12 / 12.

## 3. Acceptance criteria

| Criterion | Test performed | Result |
|---|---|---|
| Headless: ray at a dummy at 10 m → table damage | `pistol_hits_dummy_at_10m_for_table_damage` green | PASS |
| Headless: wall blocks | `wall_between_muzzle_and_target_blocks` green (QA flip-RED in round 1) | PASS |
| Headless: head sensor ×2 | `head_sensor_doubles_damage` green (QA flip-RED in round 1) | PASS |
| Headless: reload by time | `reload_takes_configured_time` + QA partial-reserve / cancel probes | PASS |
| Headless: shotgun 10 rays | `shotgun_fires_ten_pellets` (now also one shared shot id) | PASS |
| Headless: spread grows in a series and shrinks | `spread_grows_in_series_and_recovers` + QA probe 5.8° → 3.0° | PASS |
| Runtime t6.py via brp.py | ran it myself, exit 0, PNGs checked | PASS |
| Owner-run feel checklist recorded | section 6 | PASS (recorded; owner run pending) |
| Tuning values in data files | round-2 diff adds no tuning `const` (only `Local<u32>` counter, seed plumbing) | PASS |
| build / clippy / test gta_sim (+citygen) green | ran | PASS |
| Existing tests pass | all green | PASS |
| B1 (task): damage queued during Wasted does not reach the respawned player | `damage_queued_during_wasted_is_dropped` green | PASS |
| B1 (round 1 QA): input from Wasted does not act after respawn | fixer gate + QA flip-RED + QA every-frame probe + runtime probe2 | PASS |
| Damage variance in band, varies, per-pellet roll, seeded sim RNG | `damage_variance_stays_in_band_and_varies`, `combat_rng_follows_city_seed` (QA flip-RED) | PASS |
| Hit message sim → client, client never recomputes | `DamageDealt { shot, … }`; label = merged sum of sim values; runtime drop == number | PASS |
| Floating number camera-facing, integer | screenshots | PASS |
| Headshot larger, red, CRIT | `crit_hit.png`, `label_matches_message` | PASS |
| Juice animation from juice.ron, no leak | `numbers_despawn_after_lifetime`; runtime 0 left | PASS (look = owner) |
| Runtime screenshot: body number + red CRIT | `body_hit.png`, `crit_hit.png` | PASS |
| Aim camera leaves the crosshair clear | `aim_burst_1/2.png` | PASS |
| Tracer readable in a t6 screenshot | `body_hit.png`, `crit_hit.png`, `aim_burst_*.png` | PASS |
| Held gun visible; placement in render.ron | screenshots (gun at the right hand) | PASS (size = owner) |
| Flash/tracer from the visible barrel | tracers leave the gun in the PNGs | PASS |
| Arm holding clips per weapon; shoot clip once | `armed_animator_layers_arm_clips`, `arm_pose_per_weapon` green; round-1 side views | PASS (motion = owner) |

## 4. Bugs found

No new blockers. Round-1 items re-checked:
- **B1 (was Medium): fixed.** Headless and runtime evidence above.
- **L1: fixed.** Cooldown and bloom stay with each gun and keep recovering while holstered (QA probe). The pistol is
  ready right after a shotgun blast.
- **L2: fixed.** One summed label per (shooter, shot, target). Per-pellet gameplay rolls are unchanged.
- **L4: fixed.** The seed follows the city seed. TestArea stays at 0.
- L3 (GDD drift): the orchestrator owns it. Not re-checked.

New observations (none blocks):
- **O1 — Low/Info, pre-existing (not round 2): the automatic fire rate is quantised by the 64 Hz tick.** `fire_interval`
  0.08 s is 5.12 ticks. The cooldown counts down in whole ticks and drops the remainder, so the SMG fires every
  6 ticks = 0.094 s: about 640 rounds/min against the 750 in `weapons.ron`. The QA probe shows it: 7 shots in 40 ticks.
  Repro: `shot_ids_distinct_per_pull` output. Expected: the configured rate on average. Actual: about 15 % slower.
  Owner feel item. The fix would carry the negative remainder (`cooldown += interval` instead of `= interval`).
- **O2 — Info (owner): the summed shotgun label sits on the "×" hit marker** in chest shots, because the anchor is
  near the crosshair (`shotgun_blast.png`). It is readable. The fixer also noted it.
- **O3 — Info: on a lethal blast the summed label counts the full lethal pellet**, so it can exceed the HP the target
  had (t6.py accepts `value >= health before`). Pellets after the kill are skipped (`hitscan.rs` `current <= 0`).
  This is consistent with "number == sim damage value". Mentioned only for the owner.

## 5. Verdict

**SHIP-PENDING-RUNTIME.** Every covered criterion passes headless and at runtime. I ran build, clippy, all suites,
t6/t5 and my own probes, and looked at every PNG cited. The round-1 blocker B1 is fixed and gated: the gate went RED
under my own perturbation, and my stronger every-frame probe plus the runtime key/click probe both hold. L1, L2
and L4 hold in my independent probes. What remains subjective is the owner checklist below. That is why the verdict
is not plain SHIP.

## 6. Owner checklist (subjective, run by the owner)

`cargo run --features fast` (add `--seed N` to change the city and the damage rolls), walk to the central park
(3 dummies 10 m along −Z of its centre, 6 pickups in a row):
- [ ] Pick up pistol / SMG / shotgun and ammo; keys 1-4 and the wheel switch; R reloads; ammo counter right.
- [ ] Fire with and without RMB: recoil noticeable, not nauseating; the placeholder noise-burst sound is tolerable.
- [ ] Tracer and muzzle flash leave the barrel, readable, not too heavy.
- [ ] The hit marker is visible and turns red on a kill. With the shotgun it overlaps the summed number (O2): acceptable?
- [ ] Aim camera: body on the left, target at the crosshair clear. Is the normal camera shoulder 0.8 m acceptable?
- [ ] The gun is visible enough from behind in normal view (a small dark box at the right hand).
- [ ] Pistol in the right hand, SMG/shotgun in both; a twitch on each shot; legs keep walk/run while armed.
- [ ] Strafe while aiming is comfortable; no sprint while RMB is held.
- [ ] Dummies drop to 0, lie 3 s and come back.
- [ ] Damage numbers: size, pop/rise/fade timing, drift spacing; CRIT readable and red.
- [ ] Shotgun: one summed number per blast and target is readable (CRIT if any pellet hit the head).
- [ ] Right after a shotgun blast the pistol is ready; switching back, the shotgun keeps its own cooldown.
- [ ] SMG rate feels right (it actually fires about 640/min, not 750, see O1).
- [ ] Clicking or pressing keys on the "ПОТРАЧЕНО" screen does nothing after respawn.
Knobs: `weapons.ron`, `aim.ron`, `camera.ron`, `juice.ron`, `mix.ron`, `strings.ron`, `locomotion.ron`,
`render.ron`, `visual.ron`.

## Cleanup / tree

- The temporary `crates/gta_sim/tests/qa_probe.rs` was removed (a copy is kept in `scratch/qa/qa_probe.rs`). The three
  sabotaged files were restored by `git checkout`, and their sha256 matched.
- Game processes: 3 launches by brp.py, each ended with `brp_extras/shutdown`; `tasklist` shows no `gta_like`. I did not
  touch the owner's own `cargo run` session. No containers.
- `git status --short`: only this file (`QA_REPORT.md`) and `log.jsonl` (+1 qa decision entry). The new scratch
  outputs (`scratch/qa/t6`, `t5`, `probe2`, edited probes) are in the ignored `scratch/`. No binaries are tracked.

children: 0 launched / 0 reported.
