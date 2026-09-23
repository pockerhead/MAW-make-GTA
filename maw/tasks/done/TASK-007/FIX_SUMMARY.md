# FIX_SUMMARY — TASK-007 fixer round 2

Input: `QA_REPORT.prev-1.md` (verdict NEEDS_FIXES) and its probes in `scratch/qa/`; the orchestrator note
asks for B1, L1, L2, L4. Round 1 is `FIX_SUMMARY.prev-1.md`.

## Preflight: the claim checked first

The prescription most likely to break correct code if applied verbatim was L1, "make cooldown and bloom per
gun slot". Two ways to get it wrong: (a) tick only the held gun's cooldown, so a holstered shotgun freezes
mid-cooldown and swapping pistol -> shotgun -> pistol lets you skip it or never recover; (b) keep the
`since_shot` counter per slot, which grows every tick and breaks `death_wasted_respawn_at_hospital`, since that
test compares `Loadout.guns` before and after death with `assert_eq`. I checked both against the code
(`weapons.rs` `tick_loadouts`, `tests/respawn.rs:131-136`). The diagnosis holds (shared `Loadout.cooldown` /
`bloom_deg`). I recomputed the fix: every slot ticks, and `since_shot` became a countdown
(`recovery_wait`), so a slot that never fired stays at its default values.

B1's prescription (reset `ActionIntent` on `OnExit(Wasted)`) holds only if the reset runs before the first
Playing fixed tick. Checked in the pinned source: `bevy_state-0.19.1/src/app.rs:335` puts `StateTransition`
right after `PreUpdate`, and `bevy_app-0.19.1/src/main_schedule.rs:224-232` runs `RunFixedMainLoop` after
that. So the reset lands before `fire_weapons` consumes the latches.

## 1. Fixed

1. **B1 (blocker): input from the Wasted screen acted after respawn.** Real. Headless repro
   (`scratch/qa/qa_probe.rs`) and runtime repro (`scratch/qa/probe/probe.json`, 12 -> 11) both confirmed it.
   Fix: new `wasted::drop_queued_input` in `crates/gta_sim/src/flow/wasted.rs` resets every player's
   `ActionIntent` to default. It runs on `OnExit(GameState::Wasted)` next to `drop_queued_damage`
   (`flow/mod.rs`).
   Why this and not gating `write_action_intent` to Playing: the sim reset is gated headlessly and covers
   every writer of the intent (client, BRP, future AI). A client-only gate would also leave latches raised
   before the state switch. The client is unchanged. If LMB is still held after respawn, the SMG fires,
   which is input given in Playing.
   Gate: `tests/respawn.rs::input_raised_during_wasted_is_dropped` (correctness). Pistol 10/20. On the Wasted
   screen it raises `fire_requested`, `reload_requested` and `select = Unarmed`. After respawn and 8 ticks it
   expects `(held, magazine, reserve, reload_left) == (Pistol, 10, 20, 0.0)`. Each latch moves a different
   element of that tuple.
   Runtime: QA's `qa_probe_runtime.py`, re-run → `scratch/fixer2_probe/probe.json`
   `wasted_ghost_shot: before 12, during_wasted 12, after_respawn 12` (it was 11 before the fix).
2. **L1: cooldown and bloom were shared across guns.** Real. `GunSlot` now carries `cooldown`, `bloom_deg`
   and `recovery_wait` (seconds before the bloom starts shrinking). `Loadout.cooldown`, `since_shot` and
   `bloom_deg` are removed. `Loadout.spread_deg` stays as the derived spread of the held gun.
   `tick_loadouts` calls `recover_gun` for all three slots every tick. `fire_weapons` checks and writes the
   held slot only. The tick arithmetic is unchanged: recovery still starts on the tick where
   `k·dt >= recovery_delay`. D6 `spread_grows_in_series_and_recovers` passes with its derived ticks
   unchanged. Only the field paths changed in D6/D12 and in the `GunSlot` literals (`..default()`).
   Gate: `tests/shooting.rs::cooldown_and_bloom_stay_with_their_gun` (behaviour). Fire the shotgun, then
   select the pistol. The pistol's `spread_deg` must be `base_deg`, and it must fire on the next request.
   Then switch back to the shotgun. It must still have `bloom > 0` and `cooldown > 0`, and it must not fire.
   That last check guards against a "reset on switch" exploit.
3. **L2: shotgun numbers piled into one blob.** Real (`scratch/qa/probe/shotgun_blast.png`).
   - `DamageDealt` gains `shot: u32`. This is a per-trigger-pull counter (`Local<u32>` in `fire_weapons`),
     and every pellet of one blast carries the same value. Per-pellet gameplay damage is unchanged: each
     pellet still rolls and is applied on its own.
   - Client (`src/juice/damage_numbers.rs`): `sum_per_shot_and_target` merges messages by
     `(shooter, shot, target)` in first-hit order. The damage is summed, the label is CRIT if any pellet hit
     the head, and the point is the mean of the pellet points. One label is spawned per group.
   - Why a shot id instead of merging per frame: when a frame spans two SMG intervals (0.08 s), a per-frame
     merge would fuse two shots into one number.

   Sim gates:
   - `shotgun_fires_ten_pellets` now also asserts that all 10 pellets share one `shot`.
   - `damage_variance_stays_in_band_and_varies` asserts that 3 blasts have 3 distinct ids.

   Client gate: `damage_numbers_gate::pellets_show_one_sum_per_shot_and_target` (correctness). Shot 7 on
   target A deals 8 + 9(head) + 8, shot 7 on B deals 9, shot 8 on A deals 7. It expects exactly the labels
   `25 CRIT`, `9`, `7`. The existing client gates now pass distinct shot ids.

   `t6.py` step 7 (new): pick up the shotgun, press key 4, stand 4 m from dummy 0, aim at the chest and fire
   once. Hard checks: magazine −1; exactly one `DamageNumber`; its value == health drop, or on a lethal
   blast the value is >= the health before the shot.
   Runtime: drop 81 == one number 81 (`scratch/fixer2_t6/summary.json`). QA probe: drop 73 == one number 73.
4. **L4: every launch rolled the same damage sequence.** Real (`CombatRng::default()` = seed 0).
   `CombatPlugin { seed }`. `compose_sim` passes the city seed for `WorldSource::City { seed }` and 0 for
   `TestArea`, so headless gates stay deterministic. `CombatRng::seeded(seed)` replaces `Default`. I did
   not read `Res<CitySeed>`: that resource does not exist in `TestArea`.
   Gate: `tests/shooting.rs::combat_rng_follows_city_seed` (correctness). The first 4 draws are equal for
   seed 1 twice, different for seed 1 vs 2, and equal for `TestArea` twice. Runtime (seed 1): the first body
   hit is now 27 and the first headshot 51. With seed 0 they were 26 and 52 in every earlier run.

Flip-RED. Script `scratch/fixer2_flip_red.py`, logs `scratch/fixer2_flip_red_sim.log` and
`scratch/fixer2_flip_red_client.log`. Each perturbed file is restored and its sha256 is checked:

| Gate | Perturbation | Result |
|---|---|---|
| `input_raised_during_wasted_is_dropped` | `drop_queued_input` removed from `OnExit(Wasted)` | RED |
| `input_raised_during_wasted_is_dropped` | reset keeps `fire_requested` only | RED |
| `cooldown_and_bloom_stay_with_their_gun` | fire copies cooldown/bloom/recovery_wait to all slots (the old shared behaviour) | RED |
| `combat_rng_follows_city_seed` | `CombatRng::seeded(0)` whatever the seed | RED |
| `damage_variance_stays_in_band_and_varies` (shot-id part) | `shot: 0` for every message | RED |
| `pellets_show_one_sum_per_shot_and_target` | no merging | RED |
| `pellets_show_one_sum_per_shot_and_target` | merge key without `shot` | RED |

All gates were GREEN again after restore (full suites below).

## 2. Skipped / notes

- L3 (GDD drift in `camera.ron` / `juice.ron`): the orchestrator did not ask for it. It stays with the owner.
- `MoveIntent.jump_requested` belongs to the same class but is not touched. `drive_characters` is not gated
  to Playing and clears the jump latch while the player is `Dead` (`character/mod.rs:158-161`). The only
  gap left is a jump pressed in the last Wasted frame when that frame runs 0 fixed ticks. That is
  low-risk, not in this round's list, and it would give a hop, not damage.
- The client `write_action_intent` is still not state-gated (see B1 for why).
- Screenshot `scratch/fixer2_t6/shotgun_blast.png`: one white "81" over the dummy. It overlaps the "×" hit
  marker, because the anchor of a chest shot sits near the crosshair. Legible. Size and placement are an
  owner item.

Owner checklist additions (for QA_REPORT.md):
- [ ] Shotgun: is one summed number per blast and target readable (CRIT if any pellet hit the head)?
- [ ] Switching guns: the pistol is ready right after a shotgun blast; the shotgun keeps its own cooldown.
- [ ] Damage rolls now differ between `--seed` values (same seed, same sequence).

## 3. Test results

- `cargo test -p gta_sim` → all ok: lib 11, anim_state 4, asset_manifest 3, city 6 (+1 ignored, as
  before), config 14, health 6, jump 3, movement 4, respawn **5** (+1), shooting **16** (+2), terrain 2.
- `cargo test -p gta_like --bin gta_like` → `ok. 24 passed; 0 failed` (+1).
- `cargo test -p citygen` → ok (not touched).
- `cargo build` → ok. `cargo clippy -- -D warnings`, `cargo clippy -p gta_sim --tests -- -D warnings` and
  `cargo clippy -p gta_like --tests --features dev -- -D warnings` → clean.
- `cargo tree -p gta_sim -e normal -i bevy_render` → nothing to print. `python tools/qa/tree_check.py` →
  passed.
- `python tools/qa/scenarios/t6.py --out maw/tasks/in_progress/TASK-007/scratch/fixer2_t6` → exit 0
  (`scratch/fixer2_t6_run.log`): body 27 == number 27, magazine 12 -> 11; head 51 == red "51 CRIT";
  SMG aimed burst 30 -> 17, dummy Dead, tracers alive at both captures; reload 30; shotgun 6 -> 5,
  drop 81 == single number 81; 0 numbers left; no log errors. PNGs checked: `crit_hit.png`,
  `shotgun_blast.png`.
- `python tools/qa/scenarios/t5.py --out .../scratch/fixer2_t5` → exit 0, no log errors.
- QA runtime probe re-run → `scratch/fixer2_probe/probe.json`: ghost shot gone (12/12/12), shotgun one
  number 73 == drop 73.
- `tasklist` after every run: no `gta_like` process left.

Changed files: `crates/gta_sim/src/{lib.rs, combat/mod.rs, combat/hitscan.rs, combat/weapons.rs,
flow/mod.rs, flow/wasted.rs}`, `crates/gta_sim/tests/{respawn.rs, shooting.rs}`,
`src/juice/{damage_numbers.rs, damage_numbers_gate.rs}`, `tools/qa/scenarios/t6.py`. Task dir:
`log.jsonl` (+4 decision entries), `FIX_SUMMARY.md`. Scratch (ignored): `fixer2_flip_red.py`, the logs,
`fixer2_t6/`, `fixer2_t5/`, `fixer2_probe/`. rustfmt was run only on the edited files; `git diff --stat`
shows no other file. No binary assets outside the ignored `scratch/`.

children: 0 launched / 0 reported.
