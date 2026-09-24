# FIX_SUMMARY — TASK-010, fixer round 3 (candidate-spot redesign)

Round 2 is in `FIX_SUMMARY.prev-2.md`. This round follows the binding orchestrator note and the last
`OPEN_DECISIONS.md` entry: drop the sidestep and replace it with candidate spots, add a melee check at impact, and
gate it on QA round 2's probe layouts. It also acts on QA_REPORT.prev-2 Bugs 1 and 2. I closed the IMPL_REVIEW.md
items in round 1 and found nothing new in it.

**Verdict of this round: STOP triggered.** One QA2 layout still starves a member after the redesign (details and the
proposed simpler rules are in the "STOP" section). Every other layout passes. I did not patch the redesign further.

## Preflight

- `scratch/` listed and used only as a coverage map: QA2's `qa2/qa2_probe.rs`, `probe_output.txt`, the runtime
  probe and the earlier fixer runs. I did not re-run QA's probe as evidence. I ported its layouts into a committed
  gate and ran them through that gate.
- Claim most likely to break correct code if applied verbatim: the orchestrator's "gang punches deal no damage to
  non-hostile characters". I checked `crates/gta_sim/tests/gangs.rs` `disabled_matrix_gangs_do_not_attack_each_other`.
  Its half 1 (a punch with the matrix off) takes a `DamageDealt { A → B }` from a gang-0 punch on a gang-1 member as
  its *precondition*. Applied verbatim, the rule makes that acceptance gate fail with "GATE BROKEN". So the rule is
  kept and the gate is restated (see Fixed 3). I dropped QA2's prescription (a pivot factor on the old parallel shift),
  because the orchestrator replaced the model.

## Fixed

1. **Sidestep replaced by candidate spots (QA2 Bug 2, orchestrator items 1–3).**
   - The new file `crates/gta_sim/src/gang/behavior/fire_line.rs` (160 lines) holds `FireLine::blocked`. This is the
     old test, applied to any start point: a body blocks when it is ahead within weapon range and inside
     `capsule_radius + fire_line_margin + along·tan(aim_error + max weapon spread)`.
     - `clear_spot` evaluates, nearest first, both sides of the line at each `reposition_offsets` (1.5/3/4.5 m) plus
       one `reposition_step` (2 m) back and forward. For each spot it tests the real line from that spot to the
       target. A line that runs on past the target, which the old parallel shift got wrong, is tested the same way.
     - `usable` rejects a spot that another body sits on the way to, or that is behind a wall. The body check counts
       a body only if the walk gets closer to it. The wall check is one World-mask ray, counted in `RouteLoad.rays`,
       the same primitive as `avoid_offset`.
     - `unblock` walks the member to the spot (`reposition_gait`) and re-checks the spot on each AI slot. It keeps the
       spot while it is still usable and picks again when it is not. If no spot is clear, the member closes in on the
       target (route or direct seek), so it never stands still indefinitely.
   - Tie-break in `gang_fsm`: when two shooting members block each other, the one with the lower `Entity::index_u32`
     moves and the other one holds. Shooting here means gun out, sees the target, target alive.
   - The hold-fire rule stays as before: no pull while the member's own line is blocked. The Retreat hold uses the
     same path. `GangMember.sidestep: f32` became `reposition: Option<Vec3>`.
   - Data: `gangs.ron` `sidestep_gait` was replaced by `reposition_offsets`, `reposition_step` and `reposition_gait`,
     with validation (finite > 0) and two sabotage fixtures. No new `const`.
   - `behavior.rs` is 658 lines, down from 695, because the fire-line code moved into the child module.
   - Two defects in my own first version showed up in traces and were fixed before gating (logged as a `dead_end`):
     - The walk check rejected every spot of a member touching its neighbour (0.6 m = 2 radii apart).
     - Re-picking from scratch on every slot flipped a member between sides in place (walking at 1.5 m/s, zero net
       motion).
2. **Melee checked at impact (QA2 Bug 1, orchestrator item 4).**
   - `combat/melee.rs` `apply_strikes` drops a strike when `GangConfig::spares(attacker faction, target faction)`
     holds. That is the case when the attacker is a gang member and the target has no faction or is not hostile to it.
     The check happens at the impact tick, after the sweep chose the body, so a groupmate who steps into the swing
     during the wind-up takes no damage.
   - The round-2 pull-time melee line check and sidestep were removed. The Melee branch is back to its pre-round-2
     form.
   - The player's own punches are unaffected, because `spares` needs a `Gang` attacker.
3. **Matrix acceptance gate restated.** In `disabled_matrix_gangs_do_not_attack_each_other` half 1, the precondition
   is now "A's fist met a body" (`Swing.landed`). The gate asserts no `DamageDealt` from A, and B is never in `Attack`.
   Halves 2–4 are unchanged. Consequence: the round-1 flip "provoke_gangs ignores the matrix" now turns only half 3
   RED. A non-hostile punch can no longer produce the `DamageDealt` that half 1 used to feed.
4. **Gates (orchestrator: commit QA2's layouts, N shots, 0 friendly damage).** New file
   `crates/gta_sim/tests/gang_fire_lines.rs` (523 lines). It uses the production `gang_floor` + `gang_member_bundle`,
   30 s per layout, player armour 1e6, and asserts three things per layout: 0 member→member `DamageDealt` (guns and
   fists), 0 hits on bystanders, and every member with a gun ≥ `MIN_SHOTS` = 6.
   - Layouts:
     - mixed4 inline (the QA2 "stairs" case) ×3 jitters
     - surround4 mixed / cross ×3
     - crossfire pair ×3
     - dummies + idle rival ×3
     - five civilian streets (QA V1, V2, V4, V5, V6)
     - side-by-side pairs with a dummy behind the player, including QA's **mutual lock** "west pair 1 m apart + dummy
       east" and "west pair + groupmate east"
     - fist scrum: flank with a circling player, inline with a standing player, inline with a circling player
       (**circling vs a standing player**)
     - a new **wall** layout: the nearest clear spot is behind a wall
   - Plus `of_two_members_blocking_each_other_the_lower_index_moves`: the crossfire pair, where the higher index holds
     still for 48 ticks and the lower one moves more than 0.5 m.
   - Plus `gang_punches_spare_non_hostile_characters_at_impact`: a real swing at a groupmate, at the other gang at
     peace and at a faction-less dummy gives 0 damage. The positive control on the player gives damage.
   - N = 6, from measurement: members starved by the old sidestep fired 0–5 shots in 30 s (QA2). With candidate spots
     the fewest over the gated layouts is **9** (the lone SMG in "west pair + groupmate east"). Out-of-ammo caps are
     pistol 24 and shotgun 12. Counts are deterministic (two runs, identical output). Per-layout numbers:
     `scratch/fixer3/gang_fire_lines_output.txt`.

   | Layout (shots per member) | before (QA2) | now |
   |---|---|---|
   | west pair 1 m + dummy east (mutual lock) | 0 / 0 | 30 / 24 |
   | pair 0.6 m + dummy behind player | 0 / 0 | 24 / 30 |
   | mixed4 inline j0 (22 m SMG vs stairs) | 25/22/12/**0** | 30/24/12/20 |
   | surround4 mixed j0 (16 m SMG) | 19/13/12/**1** | 28/24/12/15 |
   | V6 cross street behind player | 12/2/0 | 30/24/29 |
   | fist scrum inline, player stands | SMG 0 | SMG 21 |
   | fist scrum flank, player circles | SMG 4 | SMG 29 |
   | **fist scrum flank, player stands** | SMG 0 | **SMG 2 (open)** |

## STOP — the layout that still starves a member

- **Layout:** two out-of-ammo members punching at (±1.2, 0, −1) beside a standing player at the origin, and an SMG
  member at (0, 0, −12). Result: the SMG fires 2 shots and lands 2 hits in 30 s (brawlers: 6 and 7 hits).
- **Why no spot fixes it:** each brawler is 1.56 m from the target. The widened line is 0.5 m + d·tan 11° for the SMG,
  so beyond about 2.5 m every line to the player passes within reach of one of them. This holds from every side,
  whether the brawler is in front of the target or behind it, because a miss flies on to the range. The fallback
  (close in) brings the gunman to melee range, where it holsters and its punches land on the spared groupmates.
- This is structural. The human-shield case (QA V3, a civilian pressed against the player) is the same geometry and
  is also 0 shots, by design.
- **Committed as** `#[ignore = "open: …"]` `gunman_behind_a_flanking_fist_scrum_keeps_firing`: it runs RED with
  `--ignored`, and the rest of the suite stays green.
- **Proposed simpler rules** (pick one; I recommend (a)):
  - (a) **Brawl yields the target.** A gunman whose line is blocked by a groupmate that is itself in melee with the
    same target holds at `keep_distance` and does not close in. Liveness for such a layout is gated per group (group
    hits on the target per 30 s), not per member. The owner sees two brawlers and a gunman covering them. This rule
    also settles the human-shield case (hold, never fire through). Cost: one condition in `unblock`, and one gate
    change.
  - (b) **Narrow check for brawlers.** For a body within melee reach of the target, test only the aim ray widened
    by the body radius, not the spread cone, and accept the rare graze. This breaks "0 friendly damage" by design.
    Not recommended, because the orchestrator's rule is 0.
  - (c) **Brawlers step back.** When a gunman of the same group is blocked, fist members give the target one side
    (orbit to the far side). This couples two FSM tactics and is the kind of patching the note forbids.

## Flip-RED record (final code; `scratch/fixer3/flips.py`, output `scratch/fixer3/flips_final.txt`)

Each flip was applied to the final code, the target was run, and the file was restored. The sha256 matched the
original every time (`restored: True`).

| # | Perturbation | Gate(s) RED |
|---|---|---|
| F1 | hold fire removed (`armed_pull && clearing.is_none()` → `armed_pull`, both sites) | 6 of `gang_fire_lines` (friendly damage, e.g. mixed4 26 dmg; crossfire pair); `gang_combat` `members_never_shoot_their_own_group` + `members_do_not_shoot_through_a_bystander` |
| F2 | a blocked member stands (no spot, no close-in) | 7 of `gang_fire_lines` ("west pair 1 m: Smg fired 0 < 6", wall 0, mixed4 Pistol 0, …) |
| F3 | wall ray removed from `usable` | `blocked_member_does_not_walk_into_a_wall` (0 shots), `surround4 mixed j2` (16 m SMG 0) |
| F4 | tie-break off (`yields && false`) | `of_two_members_blocking_each_other_the_lower_index_moves` ("tick 0: the higher index moves") |
| F5 | impact filter off in `apply_strikes` | `gang_punches_spare_non_hostile_characters_at_impact` (groupmate 10 dmg), 4 layout gates (friendly punches), `gangs` `disabled_matrix_…` half 1 |
| F6 | no close-in fallback (stand) | 6 of `gang_fire_lines` (0 shots) |
| F7 | re-pick every slot (no keep) | `members_fire_past_bystanders_and_walking_civilians` (dummies + idle rival j2: Pistol 5 < 6) |
| F9 | offsets validation off | `config` `reposition_spots_lie_away_from_the_member` |
| F10 | `reposition_step` validation off | same fixture |

The "arrival re-pick" that I added while debugging went GREEN under its flip. It turned out to be redundant (the
spot is re-checked from the spot itself), so I removed it: whatever the gates cannot fail is not code (YAGNI).

## Skipped

- QA2's pivot-factor prescription for `fire_line_shift`: superseded by the orchestrator's redesign. The whole
  function is gone.
- A spiral or orbit fallback for the flank scrum: this is patching after the STOP condition (orchestrator note,
  TASK-018/019 lesson). It is listed above as option (c) at most.
- QA2's note that the muzzle offset (0.25 m) exceeds `fire_line_margin` (0.2 m): no hit was observed in any layout
  here or in QA's. It stays a margin note, not touched.

## Test results

- `cargo build -j 4`: Finished. The following were all clean:
  - `cargo clippy -j 4 -- -D warnings`
  - `cargo clippy -p gta_sim --tests -j 4 -- -D warnings`
  - `cargo clippy -p gta_like --bin gta_like --tests -j 4 -- -D warnings`
- `touch crates/*/src/lib.rs && cargo test -p gta_sim -p citygen -j 4`: 27 result lines, **227 passed, 0 failed, 4
  ignored** (3 pre-existing plus the open STOP gate). Log: `scratch/fixer3/test_gta_sim.txt`. `gang_fire_lines`: 8
  passed, 1 ignored. `gangs` 7, `gang_combat` 9, `config` 40, all ok.
- `cargo test -p gta_like --bin gta_like -j 4`: 41 passed. No presentation code was touched.
- `python tools/qa/tree_check.py`: passed. `cargo tree -p gta_sim -e normal -i bevy_render`: nothing to print.
- Runtime, `cargo build --release --features dev` + `python tools/qa/scenarios/t9.py --out scratch/fixer3/t9`: exit 0.
  - HQ group of 3: `Attack` ×3 after the sky shot, 7 / 7 / 7 shots in 6 s, every member at 100 HP and still
    `Attack`, `Playing`, no log errors.
  - No-vsync frame 2.52–2.55 ms (30 Hz Fifo monitor again).
  - Log: `scratch/fixer3/t9_run.txt`. No game process left running.
- `t8.py` was not re-run: the sim change touches only gang members and gang-attacker strikes.

## Owner checklist additions

- [ ] Бандит, которому свой или прохожий закрывает линию, отходит на пару метров в сторону (или подходит ближе) и
      стреляет; двое рядом не толкаются друг в друга.
- [ ] Двое бандитов бьют тебя кулаками с двух сторон, третий со стволом подходит вплотную и тоже машет кулаками
      почти впустую (открытый случай STOP). Оцени, как это читается.
- [ ] Бандиты в свалке не калечат друг друга кулаками.

## Changed files

- `assets/gang/gangs.ron`
- `crates/gta_sim/src/gang/mod.rs`: config fields, validation, `spares`, `GangMember.reposition`
- `crates/gta_sim/src/gang/behavior.rs`
- `crates/gta_sim/src/gang/behavior/fire_line.rs` (new)
- `crates/gta_sim/src/combat/melee.rs` (impact filter)
- `crates/gta_sim/tests/gang_fire_lines.rs` (new)
- `crates/gta_sim/tests/gangs.rs` (half 1 restated)
- `crates/gta_sim/tests/config.rs` (one fixture pair)
- Task dir:
  - this file
  - `log.jsonl` (1 `dead_end`, 2 `decision`)
  - `PCTX_PROPOSALS.md` (one entry: the scrum/human-shield geometry for T11)
  - `scratch/fixer3/`: flips runner and output, trace probe `zz_trace.rs` (not in `tests/`), test log, t9 run

children: 0 launched / 0 reported.
