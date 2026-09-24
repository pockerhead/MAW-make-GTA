# FIX_SUMMARY — TASK-010 (fixer)

## Preflight

- `scratch/` read as a coverage map only (implementer probes, flip scripts, qa_t8/qa_t9 runs).
- The claim most likely to break correct code if applied verbatim: review issue 3, "after the first success assert
  exactly one attached, visible gun for each living owner on every update". Checked in `src/visuals/weapons.rs` and
  bevy_world_serialization 0.19.1 `world_asset_spawner.rs:358-380, 262-270`: a re-instance despawns the joints
  recursively (the gun under the hand goes too) and `attach_held_gun` re-attaches on the next `Update`. Measured in
  the harness: after a re-instance the gun is missing for one update and `Hidden` for one more, because
  `attach_held_gun` and `show_held_gun` are unordered (production registers them the same way in `visuals/mod.rs`).
  The claim is real: "exactly one and visible on every update" fails on correct self-repairing code (seen 3 of 8 full
  runs while I tried it). The diagnosis stays, the prescription was recomputed (below).

## Fixed

1. **Issue 1: performance. The 30 FPS is the display cap, not frame cost.** Measured at the gang-0 HQ camera pose,
   seed 1, release `--features dev` (`scratch/fixer_fps_probe.py`):
   - winit sees one monitor, `\\.\DISPLAY9`, 30000 mHz (30 Hz), primary. Window `present_mode: Fifo`, focused.
   - Fifo: 30.0-30.2 FPS / 33.3-33.6 ms in every case. That is the refresh rate.
   - AutoNoVsync set over BRP, same pose, 1 s averages:
     | Case | frame time avg | FPS |
     |---|---|---|
     | gangs off (`max_gang_members: 0`, file restored after) | 2.41-2.60 ms | 388-418 |
     | gangs on, group idle at 12 m | 2.34-2.64 ms | 385-430 |
     | gangs on, live firefight (player alive, armour 1e6, 1-3 members in `Attack`) | 2.36-2.47 ms | 410-428 |
   - CPU/render split (`--features dev,profile` chrome trace, last 4 s, firefight, no vsync; the trace build itself is
     slower, 4.2-5.2 ms/frame): render sub-app 3.86 ms mean (p95 4.68), extract 1.14 ms, `Main` schedule
     ~3.3 ms/frame, `FixedUpdate` 0.48 ms per 64 Hz tick. All gang, navigation and population systems together take
     about 2.3 ms per wall-clock second, about 0.04 ms per tick (`scratch/fixer_trace_on.txt`,
     `scratch/fixer_trace_summary.py`). The 5 GB trace was deleted from `target/qa/`.
   - No optimisation needed: gangs add no measurable frame cost. The GDD §11 ≥60 FPS target is met with room to
     spare (~400 FPS without vsync on this host). On the owner's 144 Hz monitor, Fifo will read 144.
   - **Measurement fix so it cannot mislead again:** new `Game.frame_report()` in `tools/qa/brp.py` records monitors
     (name, refresh Hz, primary), the present mode, FPS as shipped, then switches to AutoNoVsync over BRP and samples
     the frame cost. `t8.py` uses it instead of the bare `diagnostics` in the crowd step, and `t9.py` uses it in the
     firefight step.
2. **Issue 2: `t9.py` firefight evidence.** The player's armour is raised to 1e6 over BRP right before the provoking
   shot (a named QA mutation, stated in the docstring; it does not touch provocation). The camera is levelled after the
   sky shot. `firefight.png` is captured at the first 0.25 s poll where the game is `Playing`, the player is alive, a
   member is in `Attack` and a member has spent a round. No such moment within 6 s fails the run ("firefight
   evidence incomplete"). The summary reports shots per member (rounds carried before minus after; a reload moves
   rounds and never loses them; a dead member reports `null` because its gun is dropped), each member's final state
   and health, armour lost, drawn guns and `RouteLoad`.
   Run (`scratch/qa_t9_fixer/summary.json`, exit 0): three SMG members, capture at 0.56 s with player health 100 and
   3 attacking, **8 / 8 / 8 shots per member in 6 s**, armour lost 35, all still `Attack`, `game_state: Playing`, no
   log errors. `firefight.png` checked by eye: the purple group stands down the street in front of the player. Its
   frame report: `\\.\DISPLAY9` 30 Hz, Fifo 29.8-30.0 FPS, no vsync 2.45-2.91 ms.
3. **Issue 3 + held-gun coverage: `gang_held_gun_follows_loadout_and_owner`.** Recomputed prescription. After the
   drawn-gun check the gate:
   (a) waits for 4 updates with no `AssetEvent<WorldAsset>`. In the headless harness `LoadedWithDependencies` and
   `Modified` keep arriving for a while as scenes stream in. They re-instance models on their own, which explains the
   implementer's "1 in 6" dead end, and they debounce a forced `Modified` for 3 frames;
   (b) forces a real re-instance of the armed member's model via `Assets<WorldAsset>::get_mut` plus an actual
   `DerefMut` (`AssetMut` emits `Modified` only on `DerefMut`, bevy_asset 0.19.1 `assets.rs:668-689`), retries up to
   5 times, and requires the old hand joint to be despawned (proof it happened);
   (c) requires the gun back and drawn within 8 updates;
   (d) over 32 more updates asserts at most one gun per owner (`gun_of`) and a drawn gun never unseen (missing or
   `Hidden`) for more than 2 consecutive updates. That checks self-repair, not a loop, and matches the one-update
   loss plus one-update `Hidden` measured above.
   "Assert a second `WorldInstanceReady` reattaches it": a headless test cannot trigger `WorldInstanceReady` by hand
   (bevy-ecs lesson), so it goes through the real asset path, which emits it. Moved to `src/visuals/gang_gate.rs`
   with the other gang presentation gates (see Decisions).
4. **Missing coverage: cadence.** New `each_member_fires_no_faster_than_its_trigger_cadence` in
   `crates/gta_sim/tests/gang_combat.rs`: Pistol, SMG and Shotgun members at 12 m (0,−12), (−7,−9.75), (7,−9.75), all
   provoked, player armour 1e6, 384 fixed ticks logged via `Shots`. Asserts at most one `ShotFired` per member per tick,
   each member still `Attack` at the end (GATE BROKEN otherwise), ≥ 3 shots each, and every consecutive pair of a
   member's shots at least `max(trigger_seconds.0, fire_interval(gun))` apart. That is 0.5 s for pistol and SMG and
   0.9 s for the shotgun (a pull inside the cooldown is dropped). Derivation: `pull` re-rolls
   `trigger_left ≥ trigger_seconds.0`, the Decide tick subtracts 1/64 before the `== 0` check, so the next pull is
   ≥ 32 ticks later; each pull is one `ShotFired` in the next Damage set.
5. **Missing coverage: matched performance sample with and without gangs, profile while alive.** Covered by item 1.

## Flip-RED record

| Gate | Perturbation | Result | Restored |
|---|---|---|---|
| cadence | `pull` also sets `action.fire_held = true` | RED: "Smg: shots at ticks 3 and 9 are 0.09375 s apart < 0.5 s" | GREEN |
| cadence | `pull` sets `trigger_left = 0.0` instead of a roll | RED: "Pistol: shots at ticks 3 and 23 are 0.3125 s apart < 0.5 s" | GREEN |
| held gun | `attach_held_gun` attaches each character only once (`Local<HashSet<Entity>>`) | RED 4/4 ("drawn gun lost"; the old gate passed this about 5 runs in 6) | GREEN |

Not flipped: a real persistent re-instance loop. I found no code change that produces one without editing the test.
The `unseen <= 2` bound is a direct assertion of it.

## Skipped

- Retuning lethality: not touched, the orchestrator left it to the owner. Evidence for the owner: 3 SMG members spent
  8 rounds each in 6 s and took 35 armour; the earlier implementer run (no armour) died within ~3 s.
- Ordering `attach_held_gun` before `show_held_gun` (`.chain()`), which would cut the one-update `Hidden` after a
  re-attach: not in the review. It is a one-frame presentation detail and the gate does not depend on it. Owner
  visible, mentioned only.
- Root cause of the `WorldAsset` event stream while scenes load. It settles; I found no project-side `get_mut` on
  `WorldAsset`. Proposed as a gates lesson in PCTX_PROPOSALS.md.

## Findings for QA / owner (not fixed)

- In `scratch/fixer_fps_on.json` and `fixer_fps_on_profile.json` a group member's carried rounds dropped to 0 during
  the fight (24 → 0 pistol, 60 → 0 SMG) while the attack count fell by one. A cleared gun slot means `gang_death` ran,
  and the only damage source in that fight was the other members. This is most likely friendly fire inside the group:
  bullets are not faction-filtered, and members shoot at the player through each other. Unconfirmed. The new t9 run
  had no death. Owner-run item: "бандиты убивают своих в перестрелке?"
- Owner checklist addition: "в перестрелке ствол в руке бандита не мигает и не пропадает".

## Decisions

- Gang presentation gates moved from `civilian_gate.rs` into `src/visuals/gang_gate.rs` (+ `#[cfg(test)] mod
  gang_gate;`). The extended test would have put `civilian_gate.rs` at 787 lines, over the 750 warning, and
  PLAN_FINAL step 24 prescribes this exact split. The code is moved unchanged except the held-gun test. Shared
  harness helpers became `pub(super)` and the moved imports were removed. Now 457 + 358 lines.
- Logged in `log.jsonl`: two dead ends (the 30 FPS alarm; the verbatim held-gun prescription) and two decisions (the
  gate split; the t9 armour mutation).

## Test results

- `cargo build -j 4`: Finished.
- `cargo clippy -j 4 -- -D warnings`: clean. `cargo clippy -p gta_sim --tests -j 4 -- -D warnings`: clean.
  `cargo clippy -p gta_like --bin gta_like --tests -j 4 -- -D warnings`: clean.
- `touch crates/*/src/lib.rs && cargo test -p gta_sim -j 4`: 21 result lines all `ok`, 190 passed, 0 failed
  (`scratch/fixer_test_gta_sim.txt`). `citygen` not touched.
- `cargo test -p gta_like --bin gta_like -j 4`: 41 passed, run **10 times** after the final edit, 10/10 green.
- `python tools/qa/scenarios/t9.py --out scratch/qa_t9_fixer`: exit 0 (summary above).
- `python tools/qa/scenarios/t8.py --out scratch/qa_t8_fixer`: exit 0. 40 civilians, scared share 0 → 0.909, no
  errors. Frame report: `\\.\DISPLAY9` 30 Hz, Fifo ~30 FPS, no vsync 2.27-2.34 ms.
- `python tools/qa/tree_check.py`: tree checks passed.
- No game process left running (`tasklist` checked).

## Changed files

- `crates/gta_sim/tests/gang_combat.rs`: cadence gate.
- `src/visuals/gang_gate.rs` (new), `src/visuals/civilian_gate.rs`, `src/visuals/mod.rs`: gate split and held-gun
  re-instance gate.
- `tools/qa/brp.py` (`frame_report`), `tools/qa/scenarios/t8.py`, `tools/qa/scenarios/t9.py`.
- Task dir: `FIX_SUMMARY.md`, `log.jsonl` (4 appended entries), `PCTX_PROPOSALS.md` (2 proposals), `scratch/fixer_*`,
  `scratch/qa_t9_fixer/`, `scratch/qa_t8_fixer/`.

children: 0 launched / 0 reported.
