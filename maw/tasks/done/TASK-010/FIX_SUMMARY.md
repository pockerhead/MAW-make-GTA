# FIX_SUMMARY — TASK-010, fixer round 4 (scrum hold, option (a))

Round 3 is in `FIX_SUMMARY.prev-3.md`. This round covers only the binding orchestrator note and the last
`OPEN_DECISIONS.md` entry: implement option (a), rewrite the ignored scrum gate as a group-pressure gate, and
flip-RED both parts. I closed the `IMPL_REVIEW.md` items in round 1 and found nothing new in it for this round.

## Preflight

- `scratch/` listed as a coverage map (round-3 `fixer3/`: flips runner, gate output, t9 run). I re-ran nothing of
  theirs as evidence.
- Claim most likely to break correct code if applied verbatim: the note's "hold `keep_distance` when every blocker
  is **a groupmate already engaged with the same target**". I checked it in `gang/behavior/fire_line.rs` `unblock`.
  In the mutual-lock / surround / crossfire layouts every blocker is a gunman in `Attack` on the same target, and
  there the close-in fallback is what clears the line. **Confirmed by running it:** the literal rule turned 4 gates
  RED (`west pair + groupmate east` Smg 0 shots, `surround4 mixed j0` Smg 0, `of_two_members_…` "never picked a
  spot", inline scrum). The flip G6 below reproduces this. So "engaged" is implemented as **pressed against the
  target** (within `combat.melee_distance.1` = 2.5 m, the shipped stop-punching reach). That covers the brawling
  groupmate and the human shield, and leaves a distant gunman blocker to the existing spots or close-in.

## Fixed

1. **Option (a): a pinned gunman holds its band.** Files: `crates/gta_sim/src/gang/behavior/fire_line.rs` and
   `behavior.rs`.
   - `FireLine::blockers` yields the indices of the bodies blocking a line. `blocked` is now `blockers(..).next()`,
     same test, same results.
   - `Blocked.yielding`: one flag per shield, true when the shield is pressed against the target. This applies to
     any spared body (groupmate, idle rival, civilian, dummy).
   - `pinned`: every blocker of the member's own line and of every candidate spot's line (the same `spots` that
     `clear_spot` walks) is yielding.
   - `unblock` returns `Option<Motion>`. When no usable spot exists and the member is `pinned`, it returns `None`:
     no close-in. The caller then applies the ordinary Shoot band (`band_move(distance, keep_distance)`:
     Approach > 15 m, BackOff < 8 m, Hold). In Retreat the caller stands.
   - Fire is held through a separate `line_blocked` flag (it used to be `clearing.is_none()`), so a pinned member
     holds fire while it walks its band.
   - Re-evaluation is unchanged: the line check runs every tick and the spots on every AI slot, so the gunman fires
     the moment a line clears.
   - Data: no new field and no new `const`. The pressed radius reuses `combat.melee_distance.1`.
2. **Gate rewritten: `gunman_holds_its_band_behind_a_fist_scrum`** (`crates/gta_sim/tests/gang_fire_lines.rs`,
   `#[ignore]` removed; the old `gunman_behind_a_flanking_fist_scrum_keeps_firing` is gone). The layouts are the
   production `gang_floor` + `gang_member_bundle`, 30 s, player armour 1e6.
   - **Flank scrum and inline scrum with the player standing:**
     - 0 member→member damage.
     - Group hits on the player ≥ `MIN_GROUP_HITS` = 8. Measured 15 on the flank and 10 inline, deterministic.
     - The gunman's closest approach to the player ≥ `keep_distance.0 − BAND_TOLERANCE`, with `keep_distance.0`
       read from `GangConfig` (8 m) and a tolerance of 0.5 m. Measured 12.0 and 11.30.
   - **Human shield** (a dummy 0.8 m in front of the player, a lone SMG at 12 m): 0 bystander damage, and the gunman
     stays ≥ 7.5 m (measured 12.0).
   - `Outcome` gains `closest` (per-member minimum flat distance to the player over the fight) and `keep_near`.
3. **Re-anchored: "fist scrum inline, player stands"** moved from the per-member gate
   `gunman_behind_a_fist_scrum_keeps_firing` into the group-pressure gate above. Why:
   - With a body 1.56 m off the target, the SMG line (0.5 m + d·tan 11°) is blocked from every side beyond ~5.4 m.
     That is inside the 8 m near edge of the band.
   - Round 3's gunman fired 21 shots here only because the close-in fallback took it under 5 m, into the scrum.
     Rule (a) forbids exactly that.
   - The last `OPEN_DECISIONS` entry says scrum liveness is group hits, not per-member shots. That puts this layout
     in the same class as the flank one.
   - The two **circling** scrum layouts stay per-member (≥ 6 shots): the SMG fires 30 / 30 (round 3: 29 / not
     measured), because the brawlers lag the moving player and lines open.
   - This is the one place where "all other gang gates stay green" and "implement (a)" contradict. I resolved it by
     the binding decision's own liveness definition and did not weaken the rule.

## Flip-RED record (`scratch/fixer4/flips.py`, output `scratch/fixer4/flips_out.txt`)

Each flip was applied to the final code, `gang_fire_lines` was run, and the file was restored. `restored: True`
(sha256) every time.

| # | Perturbation | RED |
|---|---|---|
| G1 | `pinned` off (old close-in) | `gunman_holds_its_band…`: "flank: the gunman came within 1.46 m of the player (band edge 8 m)" — **band part** |
| G2 | only groupmates pin (bystanders never yield) | `gunman_holds_its_band…`: "the gunman came within 1.37 m of the shielded player" — **human-shield part** |
| G3 | the group stops punching (melee pull off) | `gunman_holds_its_band…`: "flank: the group hit the player 0 < 8 times" — **group-hits part** (+ 2 per-member gates) |
| G4 | fist impact filter off (`apply_strikes`) | `gunman_holds_its_band…`: "flank: friendly damage [..]" — **0-friendly part** (+ 5 other gates) |
| G5 | hold fire off (`armed_pull && !line_blocked` → `armed_pull`, both sites) | `gunman_holds_its_band…` flank friendly damage from the SMG (+ 6 other gates) |
| G6 | every blocker yields (the literal "engaged anywhere" reading) | 5 gates: `west pair + groupmate east` Smg 0 < 6, `surround4 mixed j0` Smg 0, `dummies + idle rival j0` Smg 0, `mixed4 inline j1` Smg 0, tie-break "never picked a spot". This is why yielding means *pressed* |

## Skipped

- Nothing in `IMPL_REVIEW.md` is open (it was closed in round 1). Nothing outside the note was touched.
- The literal "groupmate engaged with the same target (anywhere)" reading was not used: see Preflight and G6.

## Test results

- `touch crates/*/src/lib.rs && cargo test -p gta_sim -p citygen -j 4`: exit 0. 27 result lines, **228 passed, 0
  failed, 3 ignored**. The 3 ignored are pre-existing; round 3 had 227 / 4, and the scrum gate is now un-ignored.
  `gang_fire_lines`: 9 passed, 0 ignored. Log: `scratch/fixer4/test_gta_sim.txt`.
- `cargo clippy -j 4 -- -D warnings` and `cargo clippy -p gta_sim --tests -j 4 -- -D warnings`: clean.
- `cargo test -p gta_like --bin gta_like -j 4`: 41 passed. `python tools/qa/tree_check.py`: passed.
- Runtime (`cargo build --release --features dev`):
  - `python tools/qa/scenarios/t9.py --out scratch/fixer4/t9`: **exit 0**. HQ group of 3 all in `Attack`; 8 / 7 /
    6 shots; every member at 100 HP and still in `Attack`; `Playing`; no log errors. No-vsync frame 2.37–2.57 ms
    (the monitor is 30 Hz Fifo, as before). Log: `scratch/fixer4/t9_run.txt`.
  - `python tools/qa/scenarios/t8.py --out scratch/fixer4/t8`: **exit 0**, `Playing`, no log errors. Log:
    `scratch/fixer4/t8_run.txt`.
  - No game process left running.
- Sizes: `behavior.rs` 665 lines, `fire_line.rs` 191, `gang_fire_lines.rs` 585 (all under the 750-line warning).

## Owner checklist (replaces round 3's STOP item)

- [ ] Двое бандитов бьют тебя кулаками вплотную. Третий, со стволом, держится в 8–15 м с оружием наготове, в
      свалку не лезет и не стреляет, пока свои или прохожий закрывают линию. Стоит отойти так, чтобы линия
      открылась, и он сразу стреляет. Оцени, читается ли это как "прикрывает", а не как "завис".
- [ ] Прохожий прижат к тебе вплотную: бандит со стволом не стреляет сквозь него и не подходит ближе 8 м.

## Changed files

- `crates/gta_sim/src/gang/behavior/fire_line.rs`: `blockers`, `spots`, `pinned`, `Blocked.yielding`, and
  `unblock` → `Option<Motion>`.
- `crates/gta_sim/src/gang/behavior.rs`: the `yielding` flags, the `line_blocked` hold-fire flag, and `Option`
  clearing.
- `crates/gta_sim/tests/gang_fire_lines.rs`: `Outcome.closest`/`keep_near`, the new group-pressure gate, and the
  circling scrum layouts kept per-member.
- Task dir: this file, `log.jsonl` (1 `dead_end`, 2 `decision`), `scratch/fixer4/` (flips runner and output, test
  log, t9/t8 runs).

children: 0 launched / 0 reported.
