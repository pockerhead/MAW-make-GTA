# FIX_SUMMARY — TASK-010, fixer round 2 (after QA)

Round 1 is `FIX_SUMMARY.prev-1.md`. This round acts on the orchestrator note (binding `OPEN_DECISIONS.md` entry:
option (a) hold fire + step aside, not (b) hitscan skipping allies) and on QA Bug 1 / minor notes in
`QA_REPORT.prev-1.md`. `IMPL_REVIEW.md` items were closed in round 1; nothing new in it for this round.

## Preflight

- `scratch/` listed and read as a coverage map: QA's `qa/qa_friendly_fire.rs` probe and `qa/friendly_fire_probe.txt`,
  `qa/ff_runtime.py` + `ff1/ff2`, round-1 fixer FPS/trace files. I did not re-run QA's scripts as proof; the probe
  layouts were ported into a committed gate and run through it.
- Claim most likely to break correct code if applied verbatim: QA option (b), "the NPC hitscan predicate skips
  same-`Faction` bodies". Checked `combat/hitscan.rs` `fire_weapons`: the `visible` predicate is shared by the aim ray
  and every pellet ray of every shooter (player included). Skipping same-faction bodies there would let bullets pass
  through allies (the orchestrator rejected it) and would put faction logic into the shared weapon path the player
  and T11 police use. Not done. The diagnosis (ray hits the first Character, excludes only the shooter, gang aim goes
  eyes → target chest) is confirmed in the code.
- A second claim checked: "hold fire while a groupmate is inside the aim **segment**". Verified wrong in a run: with
  the check limited to the segment up to the target, `ring4` still had a friendly headshot at z = +1.36, behind the
  player (a miss flies on up to the weapon range). The check therefore runs to the weapon `range`, not to the target.

## Fixed

1. **Friendly fire (QA Bug 1, orchestrator item 1).** `crates/gta_sim/src/gang/behavior.rs`:
   - `fire_line_shift(from, to, range, cone, clearance, bodies)`: in the ground plane, a body blocks while it is
     ahead of the member (`0 < along < range`) and its side offset is within `clearance + along · tan(cone)`, where
     `clearance = capsule_radius + combat.fire_line_margin` and `cone = aim_error_deg + max(base_deg + max_bloom_deg,
     loadout.spread_deg)` (half-angles: the gang error cone is applied to the aim, the weapon cone around it in
     `fire_weapons`). Returns `None` when clear, else the signed sideways shift to the nearest clear spot: each body
     blocks an interval of shifts `[side − reach, side + reach]`, and the walk goes out of the chain of overlapping
     intervals on the cheaper side (so a member does not try to squeeze between two close groupmates).
   - Bodies that block: every living `Character` except the member and its target whose `Faction` is absent
     (civilians) or not hostile to the member's gang (groupmates, the other gang while the matrix keeps them at peace).
   - `gang_fsm` computes the shift in Attack/Shoot and in the Retreat hold (only when `sees && in_range`); a blocked
     line suppresses the pull (`trigger_left` stays 0, so the member fires the tick the line clears) and replaces the
     band move with a sidestep perpendicular to the line at `combat.sidestep_gait`.
   - `GangMember.sidestep` (new field, sign of the committed side, 0 while clear): the side is chosen once and kept
     until the line clears. Without it a member between two groupmates dithered (yaw flipping every ~6 ticks at the
     boundary where the two blocked intervals stop overlapping) and fired 0-3 times in 30 s — observed in a run.
   - Melee too (the gate counts every `DamageDealt`, and QA's `spread 3` layout ended with two out-of-ammo members
     punching each other next to the player): the same test with `range = fists.range + cast_radius`, `cone = 0`,
     `clearance + cast_radius` holds a punch and sidesteps. Needs `Res<MeleeConfig>` in `gang_fsm` (always inserted by
     `compose_sim`; `gang_fsm` is registered only by `GangPlugin`).
   - Data (`assets/gang/gangs.ron`, `GangCombatConfig`): `fire_line_margin: 0.2` (m, validated finite ≥ 0) and
     `sidestep_gait: Walk`. Body radius comes from `character/locomotion.ron` `capsule_radius`, the cone from
     `gangs.ron` + `weapons.ron`. No new `const`. Lethality knobs untouched.
2. **Gates (orchestrator item 2).**
   - `members_never_shoot_their_own_group` (`crates/gta_sim/tests/gang_combat.rs`): QA's six probe layouts (ring of 3
     SMG, ring Pistol/SMG/SMG, ring of 4 Shotgun/Pistol/Pistol/Shotgun, ring of 3 SMG at 20 m, file of 3 in depth,
     the spread-3 cadence fixture), each in a fresh `gang_floor` app, production `gang_member_bundle`, player armour
     1e6, all provoked, 1920 ticks (30 s). Asserts zero `DamageDealt` whose shooter and target are both members, and
     liveness: every member ≥ 8 `ShotFired` (lowest seen 11, a shotgun limited by its ammo). Shots per member now:
     `[33,33,35] [24,26,28] [11,20,20,12] [33,34,34] [36,33,32] [24,24,12]`.
   - `members_do_not_shoot_through_a_bystander`: a Gang(1) member stands idle 6 m in front of an SMG Gang(0) member
     attacking the player at 12 m (precondition: the shipped matrix keeps Gang(0)/Gang(1) at peace). 960 ticks: 0 hits
     on the bystander, the bystander stays `Idle`, the member fires ≥ 3 shots. This covers the "non-hostile character"
     half of the rule.
   - `fire_line_margin_is_not_negative` (`tests/config.rs`): sabotage fixture `-0.1`.
   - `tools/qa/scenarios/t9.py`: after the firefight, every member must be present and at `max_health` (read from
     `character/health.ron`); the player only fires into the sky, so any loss is friendly fire.
   - Cadence and firefight gates (`each_member_fires_no_faster_than_its_trigger_cadence`,
     `attackers_open_fire_at_the_player`) unchanged and green.
3. **Held gun (orchestrator item 3).** `src/visuals/mod.rs`: `(attach_held_gun, show_held_gun).chain()` (chained
   systems get the auto `apply_deferred`, so a gun spawned by `attach_held_gun` is shown in the same update).
   `src/visuals/gang_gate.rs` `gang_held_gun_follows_loadout_and_owner` tightened: the harness chains the pair like
   `VisualsPlugin`; after the forced re-instance the gun must be back and drawn **one** update after the old hand is
   gone (was: within 8); afterwards "unseen" at most 1 consecutive update (was 2).

## Flip-RED record

| Gate | Perturbation | Result | Restored |
|---|---|---|---|
| `members_never_shoot_their_own_group`, `members_do_not_shoot_through_a_bystander` | hold-fire check removed (`&& shift.is_none()` deleted at all 3 pull sites) | both RED: "ring3 smg 12 m: friendly hits [...25 dmg headshot...]"; bystander hit | sha256 `492a873c…15ce1` restored, GREEN |
| same two | sidestep disabled (`sidestep` returns `Some(None)`: blocked member stands) | both RED: "ring3 smg 12 m: Smg member fired 0 < 8 shots in 30 s"; "M fired only 0 shots: it froze behind the bystander" | sha256 restored, GREEN |
| `members_never_shoot_their_own_group` | (development runs, earlier code states) check only up to the target; no melee hold; no side commitment | RED: ring4 friendly headshot behind the player; spread 3 friendly punches (10 dmg); ring3 mixed SMG 3 < 6 shots | fixed forward |
| `gang_held_gun_follows_loadout_and_owner` | harness pair unchained (`(attach_held_gun, show_held_gun)`) | RED 5/5: "the gun was not back and drawn one update after the model re-instance" | GREEN |
| `fire_line_margin_is_not_negative` | `fire_line_margin` validation replaced by `if false` | RED: `unwrap_err()` on `Ok` | GREEN |

(A first attempt at the sidestep flip was stacked on a not-yet-restored hold-fire flip because a backup path was
wrong; I noticed, restored from the real backup, checked the sha256 and re-ran the flip alone. The table shows the
clean run.)

## Skipped

- QA option (b), hitscan skipping same-faction bodies: rejected by the binding decision and by the check above.
- Retuning lethality: not touched (orchestrator).
- A committed runtime FF probe (`ff_runtime.py`): the t9 full-HP assertion carries the runtime claim.

## Findings for QA / owner (not fixed)

- While groupmates are in melee right next to the player, a ranged member cannot clear its line (the blocked
  interval pivots with the player) and keeps circling at walk speed without firing (seen in `spread 3`: the SMG
  member orbits from (−7, −9.75) to (−13, +3.6)). That is "hold fire", not a freeze; how it reads is the owner's call.
- Pistol and shotgun members run dry after ~20 s of continuous fire and switch to fists (existing behaviour).
- Owner checklist addition: "бандиты не стреляют сквозь своих и прохожих, а шагают в сторону и стреляют".

## Test results

- `cargo build -j 4`: Finished.
- `cargo clippy -j 4 -- -D warnings`, `cargo clippy -p gta_sim --tests -j 4 -- -D warnings`,
  `cargo clippy -p gta_like --bin gta_like --tests -j 4 -- -D warnings`: clean.
- `touch crates/*/src/lib.rs && cargo test -p gta_sim -j 4`: 21 result lines, all `ok`, 193 passed, 0 failed
  (190 before + 3 new; `scratch/fixer2_test_gta_sim.txt`). `citygen` not touched.
- `cargo test -p gta_like --bin gta_like -j 4` ×3: 41 passed each time.
- `python tools/qa/tree_check.py`: passed. `cargo tree -p gta_sim -e normal -i bevy_render`: nothing to print.
- `python tools/qa/scenarios/t9.py --out scratch/fixer2_t9`: exit 0. HQ group of 3 SMG, `Idle` ×3 before, all
  `Attack` after the sky shot, `GangHeat` [119.34, 0]; firefight captured at 0.53 s with the player alive and 3
  attacking; 8 / 8 / 9 shots per member in 6 s, **every member at 100 HP and still `Attack`** (the new assertion),
  armour lost 110, `Playing`, no log errors. Frame report: `\\.\DISPLAY9` 30 Hz Fifo ~30 FPS, no vsync 2.38-2.59 ms.
- QA's bad-bearing runtime probe, run once as evidence (`python scratch/qa/ff_runtime.py scratch/fixer2_ff_inline 30
  inline`): before the fix a member was killed by its own group at 3.9 s; now all three stay at 100 HP and in `Attack`
  for 30 s while spending 30-33 rounds each. Phase B (no armour): Wasted 7.9 s after the reset (QA before: 7.5 s with
  3 members), so the hold-fire rule does not blunt the group.
- `python tools/qa/scenarios/t8.py --out scratch/fixer2_t8`: exit 0 (regression).
- No game process left running (`tasklist`).

## Changed files

- `assets/gang/gangs.ron`, `crates/gta_sim/src/gang/mod.rs` (2 config fields + validation, `GangMember.sidestep`),
  `crates/gta_sim/src/gang/behavior.rs` (695 lines, under the 750 warning).
- `crates/gta_sim/tests/gang_combat.rs` (2 gates), `crates/gta_sim/tests/config.rs` (1 fixture).
- `src/visuals/mod.rs` (chain), `src/visuals/gang_gate.rs` (tightened gate).
- `tools/qa/scenarios/t9.py` (full-HP assertion).
- Task dir: this file, `log.jsonl` (1 dead_end, 1 decision), `scratch/fixer2_*` (test log, t9/t8/ff runs).

children: 0 launched / 0 reported.
