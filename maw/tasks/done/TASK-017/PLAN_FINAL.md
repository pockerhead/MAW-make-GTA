# PLAN_FINAL — TASK-017 (GDD T16): fire buffer, overshoot fire-line rule, bench scene, trace verdict, §1 evidence

Reviewer: plan-reviewer-2. Inputs: `TASK_FINAL.md` (binding: fire buffer; Q1 rule; Q2 N = 3/10/20; Q3 honest
bench; **Q-A police `overshoot_margin` 60, gangs 8**; **Q-B stray hits ≤ 5 % of a layout's shots and ≤ 2× measured**),
`PLAN_V2.md` (wins on every conflict with `PLAN.md` unless this file says otherwise), `PLAN.md` (detail source), the
code at `feature/t16-final`. Evidence written by this stage: `scratch/pr2/layout_geometry.py` + `.txt` (static
fire-line geometry of every `gang_fire_lines` layout under old and new rule).

Cost of error, per item: the fire buffer and the fire-line rule change gameplay state that breaks **silently** (lost
shots, friendly fire, starvation) → real headless gates with flip-RED. The bench scene, trace and §1 sweep are QA
instruments whose failure is **seen on the first run** → runtime assertions only, no extra machinery.

Host rules for every cargo command: one foreground cargo at a time, always `-j 2`. Runtime scenarios never run in
parallel with cargo.

---

## 1. Summary

Four pieces. (1) **Fire buffer**: a trigger press that lands in the last `fire_buffer_seconds` (0.15 s, `weapons.ron`)
of a gun's cooldown is kept in `Loadout.fire_queued` and fires on the first tick the cooldown is 0; everything else is
still dropped. This fixes the t13 flake in the game, not in the script. (2) **Q1 fire-line rule**: the hold-fire wedge
of `tactics::FireLine` stops at `min(range, |target − shooter| + overshoot_margin) + overreach` instead of the full
weapon reach. `overshoot_margin` is data per role: gangs 8.0 m (`gangs.ron`), police 60.0 m (`escalation.ron`, ≥ the
longest police gun, so police stay bit-identical). This removes the t9 crossfire starvation. Gang gates whose zero-hit
assertion relied on the full-reach wedge are re-anchored or converted to "guarded-zone assertion + stray-rate bound".
(3) **`--bench-scene`**: a client-only QA mode (`src/bench/`) that boards the player into a car near the Downtown
centre, pins 5 stars and drives a block loop at 1920×1080; `t16.py` measures FPS (no vsync, from a per-frame
`BenchFrames` capture) and a chrome trace (`tools/qa/trace.py`), and the verdict is "fix only by trace". (4) **§1
evidence**: `t16_s1.py` exercises every §1 gap not covered by t1..t15, and `tools/qa/repeat.py` runs every scenario N
times (3; t13 10; t9 20).

---

## 2. Implementation steps

### Step 0 — Host prep
0.1 `tools/qa/brp.py:43`: `"-j", "4"` → `"-j", "2"` (one token; host memory).
0.2 Build once early (foreground, no other cargo running):
`cargo build -p gta_like --bin gta_like --release --features dev,profile -j 2`. It is a new bevy feature set
(`trace_chrome` → `trace` + `debug`); expect a long first build. A build failure here is a finding: fix only the build.

### Step 1 — Fire buffer (sim, gated)

1.1 `crates/gta_sim/src/combat/weapons.rs`
- `WeaponsConfig` gains `pub fire_buffer_seconds: f32` with doc "Seconds before a gun's cooldown ends in which a
  trigger press is kept and fires when the cooldown ends."
- `validate()`: add `("fire_buffer_seconds", self.fire_buffer_seconds)` to the finite list at `:171-180`, and after it
  `check(self.fire_buffer_seconds >= 0.0, "fire_buffer_seconds")?;`.
- `Loadout` (`:230`) gains `pub fire_queued: bool` with doc "A press kept during the last `fire_buffer_seconds` of the
  held gun's cooldown; fires when it ends." (`Reflect + Default` already derived; test literals use `..default()`.)
- `select_weapon` (`:376-379`): inside `if held != loadout.held { … }` add `loadout.fire_queued = false;` next to
  `loadout.reload_left = 0.0;`.

1.2 `assets/combat/weapons.ron`: top level, after `headshot_multiplier`:
`fire_buffer_seconds: 0.15, // s: a press this close to the end of the cooldown fires when it ends`.
Grep `WeaponsConfig {` in `crates/` and `src/` (today: only the struct definition) and add the field to any literal.

1.3 `crates/gta_sim/src/combat/hitscan.rs` `fire_weapons` (`:186-216`). Replace the comment at `:187` with
"A request in the last `fire_buffer_seconds` of the cooldown is kept and fires when it ends; any other request during
the cooldown or a reload is dropped." New order inside the loop:
1. `let requested = std::mem::take(&mut action.fire_requested);`
2. `if reaction.is_active() { loadout.fire_queued = false; continue; }`
3. `let Some(weapon) = loadout.held else { continue; };`
4. `let loadout = &mut *loadout; let slot = &mut loadout.guns[weapon.index()];` (existing rebinding; `fire_queued` and
   `guns` are disjoint fields).
5. `if loadout.reload_left > 0.0 { loadout.fire_queued = false; continue; }`
6. `if slot.cooldown > 0.0 { if requested && slot.cooldown <= cfg.fire_buffer_seconds { loadout.fire_queued = true; } continue; }`
7. `let queued = std::mem::take(&mut loadout.fire_queued);`
8. `wants`: semi-auto `requested || queued`; automatic `action.fire_held || requested || queued`. `if !wants { continue; }`
9. Everything after is unchanged (an empty magazine with a queued press starts the reload, like a click).

1.4 `crates/gta_sim/src/combat/mod.rs`: add `fn reset_fire_queue(mut loadouts: Query<&mut Loadout>)` that sets
`fire_queued = false` (only where it is true, to avoid spurious change ticks). Register it in `.add_systems(OnExit(GameState::Wasted), …)`
and `OnExit(GameState::Busted)` next to `melee::reset_player_melee` (`:74-75`, tuple them) and in `NEW_CITY` next to
`clear_combat_messages` (`:76`). Reason: `PlayingSystems` freezes the cooldown in Wasted, so a queued press would fire
at the hospital (bevy-ecs lesson TASK-006/007).

1.5 Gates. Every tick below is **derived** in the test from `Time<Fixed>::timestep()` and `WeaponsConfig`, never a
literal. Worked numbers for shipped data (dt = 1/64 = 0.015625 s): after the tick-0 shot the pistol cooldown after
tick k's decrement is `0.3 − k·dt`; it is in the buffer window for k ≥ 10 (0.14375 s), k = 17 → 0.034375 s,
k = 19 → 0.003125 s, 0 at k = 20.
- `tests/shooting.rs` `a_press_in_the_buffer_window_fires_when_the_cooldown_ends`: pistol shot at tick 0. Pick the
  first k with `interval − k·dt < buffer − 2·dt` and `interval − k·dt > 2·dt` (shipped: k = 17; the "30 ms before"
  case). Press at k. Assert exactly one more `ShotFired`, on the first tick whose cooldown reaches 0 (shipped: k = 20),
  none between; magazine 12 → 10.
- `a_press_long_before_the_cooldown_ends_is_dropped`: shotgun (0.9 s). Press when the remaining cooldown is
  `≈ 0.5 s` (derive k from `0.9 − k·dt ≈ 0.49375`, shipped k = 26). No second shot through the tick the cooldown ends
  + 12 (shipped: ends at k = 58, check through k = 70).
- `a_queued_press_is_dropped_on_weapon_switch`: queue a pistol press at k = 17, request `WeaponRequest::Gun(Smg)` at
  k = 18 (SMG owned via `set_loadout`). No `ShotFired` from either gun through k = 30.
- `tests/respawn.rs` (next to `:295`): pistol shot, press at k = 17 (queued), lethal `DebugDamage` in the same tick →
  `Wasted` → `Playing`. Assert the `ShotFired` count is unchanged through 64 fixed ticks of `Playing`.
- `semi_auto_vs_automatic` (`:478-514`): derive the second-press offset so the remaining cooldown is
  `> fire_buffer_seconds + 2·dt` (shipped: the existing 5 ticks, 0.2219 s), and reword the message to "semi-auto fired
  from a held trigger or a press outside the buffer window".
- `tests/config.rs` `fire_buffer_seconds_is_not_negative`: `sabotaged::<WeaponsConfig>(WEAPONS_CONFIG, "fire_buffer",
  "fire_buffer_seconds: 0.15,", "fire_buffer_seconds: -0.1,", WeaponsConfig::validate)`; assert the error contains
  `fire_buffer_seconds` (pattern `:198-207`).
- Flip-RED (record the perturbed input in IMPL_SUMMARY): (a) test-local `fire_buffer_seconds = 0.0` → the first gate
  RED; (b) replace `mem::take(&mut loadout.fire_queued)` by `false` → RED; (c) remove the `select_weapon` clear →
  switch gate RED; (d) remove the `OnExit(Wasted)` registration of `reset_fire_queue` → respawn gate RED. Restore,
  GREEN.

1.6 NPC check: `cargo test -p gta_sim --test gang_fire_lines --test police_fire_lines --test gang_combat --test gangs -j 2`
stays green after Step 1 alone (run it before Step 2). Only the gang shotgun (0.9 s interval vs 0.5-1.1 s trigger) can
queue, and it fires ≤ 0.15 s after its line check. A new friendly or bystander hit → stop and report. No player
special case.

1.7 Runtime: `tools/qa/scenarios/t13.py` unchanged (click gap stays), ×10 in Step 7.

### Step 2 — Q1 fire-line rule (sim, gated)

**Rule** (one code path in `tactics/`, value per role in data):
```
limit(from, to) = min(range, flat|to − from| + overshoot) + overreach
blocked body ⇔ 0 < along < limit ∧ lateral ≤ clearance + along·tan(cone)
```
overreach = |flat muzzle_offset (0.25, −0.45)| + max(capsule 0.3, head 0.35) = 0.5148 + 0.35 = **0.8648 m**.
For a body collinear past the target at `s` m, it is guarded iff `s < overshoot + overreach` (gang: **8.865 m**).
Police: `min(range, D + 60) = range` for every D ≥ 0 because every police gun has range ≤ 60 (pistol 60, SMG 45,
`weapons.ron`), and the sum `range + overreach` is the same f32 operation as today → **bit-identical**.

2.0 **Write the new and re-anchored gate code first and run it on the unchanged rule** (record results; see 2.6 for
what is expected RED/GREEN before the change).

2.1 New test `crossfire_fires_from_the_post` in `crates/gta_sim/tests/gang_fire_lines.rs`.
- Extend `run`/`Outcome` (keep the old fields): `first_shot_s: Vec<Option<f32>>` (tick of the member's first
  `ShotFired` / 64), `moved_max: Vec<f32>` (max flat displacement from the member's position after the settle tick),
  and `shot_log: Vec<(Entity, u32, Vec3, Vec3)>` = (shooter, `attack`, `muzzle`, player position read after that tick)
  from `shots.shots[seen..]` in the tick loop.
- Rows, each ×3 `JITTERS` added to the **member only**; player at the origin; all points on the x axis
  (z ≈ 0), clear of every `world/test_area.rs:9-37` block (checked: ramp/tower x −12..−8 at z ≤ −11.9; boxes at
  z = 10 and z ≈ −17; stairs x 8.5-11.5 at z −12..−14.5):
  - `R95`: SMG member (−11, 0, 0), dummy (9.5, 0, 0). New rule: along 20.5 > 11 + 8.865 = 19.865 → unguarded by
    0.635 m (same margin ±0.01 for every jitter). Old rule: guarded; the best side spot (4.5 m) leaves the dummy at
    lateral 3.60 < 0.5 + 20.68·tan 11° = 4.52 → the member must close in.
  - `R12`: same with the dummy at (12, 0, 0) (unguarded by 3.1 m).
  - `R95off`: pistol member (−11, 0, 1.2), dummy (9.5, 0, 0). New: along 20.51 > 19.93 → unguarded; old: lateral 1.03
    ≤ 0.5 + 20.51·tan 8° = 3.38 → blocked. Unshadowed by the player: feeds the stray rate.
  - `R85` (residual, diagnostic): SMG member (−11, 0, 0), dummy (8.5, 0, 0): guarded under both rules
    (19.5 < 19.865). Assert only `MIN_SHOTS` in 30 s and zero bystander damage; print `first_shot_s`.
- Precondition per row (`GATE BROKEN`): after the settle tick every fixture sits within 0.1 m of its spawn x/z
  (TASK-011/012 lessons).
- Assertions for R95/R12/R95off: `first_shot_s ≤ FIRST_SHOT_S` **and** `moved_max < 0.5` m (fires from its post, the
  member is inside its 8-15 m band) **and** the guarded-zone assertion (2.6) **and** the stray bound (2.6). Do **not**
  assert zero bystander damage on these rows: the dummy is past the zone by design (R95off is unshadowed).
- `FIRST_SHOT_S` is derived: the measured worst under the new rule + 0.25 s. If that worst exceeds
  `trigger_seconds.1 + 0.5` (1.6 s), stop and report (the member is doing something other than firing from its post).
  Doc comment records both the new-rule worst and the old-rule result per row.
- Before the change (2.0), record per row under the old rule: RED on displacement and/or time. A row not RED under the
  old rule is liveness only and is labelled so in its comment.

2.2 `crates/gta_sim/src/tactics/fire_line.rs`
- `FireLine { range, overreach, overshoot, cone, clearance }` (replace `reach`).
- `of(aim_error_deg, weapons, gun, loadout, clearance, overreach, overshoot)` (7 params; clippy's
  `too_many_arguments` fires above 7).
- `pub(crate) fn reach(&self) -> f32 { self.range + self.overreach }` — used only by `nearby_cars` (`:174`:
  `s.line.reach()`). The car filter radius must not depend on target distance.
- `blockers`: `let limit = self.range.min(flat2(to - from).length() + self.overshoot) + self.overreach;` and
  `along < limit`. Everything else unchanged.
- Doc comments: `:18-20` (overreach: "past the guarded end of the line"), `:25` (struct doc: "guarded reach: the
  weapon range, cut to `overshoot` past the target"), `:54-55` (`blocked`: "ahead within the guarded reach: a miss is
  guarded against up to `overshoot` past the target").
- `crates/gta_sim/src/tactics/mod.rs:167` and `crates/gta_sim/src/gang/behavior.rs:393`: replace "(a miss flies on to
  the weapon range)" with "(a miss is guarded against up to `overshoot_margin` past the target)".

2.3 `tactics/mod.rs:44-54` `Discipline` gains `/// Metres past the target a miss is still guarded against.`
`pub(crate) overshoot_margin: f32`. Fill it in `gang/mod.rs:360-370` and `police/mod.rs:177-187`; pass
`d.overshoot_margin` at the 4 `FireLine::of` sites (`gang/behavior.rs:239, :395`, `police/behavior.rs:205, :414`).

2.4 Data + config.
- `GangCombatConfig` (`gang/mod.rs:106`) gains `pub overshoot_margin: f32` (doc as 2.3). `assets/gang/gangs.ron`
  combat, after `fire_line_margin`: `overshoot_margin: 8.0, // m past the target a miss is still guarded against: a
  spared body farther along the line does not hold fire (stray hits there are accepted)`. Validate:
  `positive("combat.overshoot_margin", c.overshoot_margin)?;` next to the `fire_line_margin` check (`:307`).
- `PoliceCombatConfig` (`police/mod.rs:155`) gains the same field. `assets/police/escalation.ron` combat, after
  `fire_line_margin`: `overshoot_margin: 60.0, // >= the longest police gun range (pistol 60 m): cops keep full-reach
  discipline`. Validate: `positive("combat.overshoot_margin", c.overshoot_margin)?;` after `:361`.
- Fixtures: `tests/config.rs` `gang_overshoot_margin_is_positive` via `gang_error("overshoot", "overshoot_margin:
  8.0,", "overshoot_margin: -1.0,")`, error contains `overshoot_margin`; a second row `0.0`. `tests/config_police.rs`
  `police_overshoot_margin_is_positive` via `police_error(…, "overshoot_margin: 60.0,", "overshoot_margin: -1.0,")`,
  plus `0.0`.
- Value derivation (gang 8.0), recorded in IMPL_SUMMARY: the (0, 0, 8) dummy of `members_fire_past_bystanders_…` must
  stay guarded: `20 < 12 + m + 0.865` ⇔ m > 7.135; at 8.0 it is guarded by 0.865 m (0.87-1.04 m over the jitters and
  both shooters, `scratch/pr2/layout_geometry.txt`), and from any rotated line its along is ≤ D' + 8, so it stays
  guarded wherever the member moves. **Do not use 7.x.**

2.5 Unit test `fire_line.rs` `overshoot_limits_the_guarded_zone` (pure; `FireLine` built directly: range 45,
overreach 0.865, overshoot 8, cone 11° in radians, clearance 0.5; from (0,0,0), to (0,0,−11) unless stated):
(1) body (0,0,−20.5): along 20.5 vs limit 19.865 → **not** blocked; (2) body (0,0,−19.5) → blocked;
(3) body (3,0,−15): along 15, lateral 3 ≤ 0.5 + 15·0.19438 = 3.416 → blocked; (4) to (0,0,−40), body (0,0,−45.5):
limit min(45, 48) + 0.865 = 45.865 → blocked (range caps); (5) same `to`, body (0,0,−46.0) → not blocked;
(6) overshoot 60, body (0,0,−40) → blocked (police default = old behaviour); (7) `reach()` == 45.865 for overshoot 8
(car filter unchanged). Rows 1/2 sit 0.635/0.365 m from the boundary, not on it.

2.6 **Re-anchor and reclassify the existing gang gates.** Classification from `scratch/pr2/layout_geometry.txt`
(static spawn geometry; members move, so a class is the start condition and the assertions below are what hold it):

**(A) RED by construction → re-anchor.**
- `of_two_members_blocking_each_other_the_lower_index_moves` (`:476-517`): at z = ±10 each member is 10 m past the
  other's target (> 8.865), no mutual block, the low never plans → `low_planned` RED. Move the members to
  **(0, 0, ±8.4)**: partner 8.4 m past the target → guarded by 0.465 m; 8.4 ≥ `keep_distance.0` (8.0) + 0.4, so
  `band_move` is `Hold` and nobody backs off during the gun draw. (Not ±7.5: 7.5 < 8 → `BackOff`, both walk while
  drawing, and the high index's `moved < 0.1` can fail for a reason unrelated to `yields`.) Worked: the low's 4.5 m
  side spot (4.5, −8.4) leaves the high at lateral 3.97 > 0.5 + 16.93·0.19438 = 3.79 → usable, `low_planned` true;
  in 48 ticks the low walks ≤ 1.35 m, the high still sees it at lateral ≈ 1.3 < 3.7 inside its zone, so `yields`
  keeps the high standing. Add `GATE BROKEN` preconditions from shipped config (loaded from the file, not the app
  resource): `2·8.4 < 8.4 + overshoot_margin + overreach − 0.2` and `8.4 ≥ keep_distance.0 + 0.2`. Update the test
  comment. Flip-RED: test-local gang `overshoot_margin = 0.5` → `low_planned` RED.

**(B) Premise lost (the blocking bystander is past the zone; the rows go vacuous, not RED) → re-anchor.**
The four `side_by_side_pairs_do_not_lock_each_other` dummy rows (`:269-301`). Their comment says "a bystander far
behind the player blocks both lines"; under the rule a dummy 25 m past the player blocks nothing (the +z rows are also
wall-shielded, so they would test nothing at all). Move each dummy to **6 m past the player** on the pair's axis:
"west pair 1 m" → (6, 0, 1); "west pair 0.7 m" → (6, 0, 0); both "+ dummy behind the player" rows → (0, 0, 6).
Geometry: guarded by 2.83-2.91 m for every member (layout_geometry REANCHOR rows); side spots cannot clear a body
6 m behind from 12 m (pivot), so the pair closes in and clears from nearer, which is the unblock path the row is about.
Add per member: `moved_max ≥ 0.5` (the unblock path ran) next to `MIN_SHOTS` and the blanket zero. Update the comment
("a bystander close behind the player, inside the guarded zone, blocks both lines"). Flip-RED: test-local gang
`overshoot_margin = 0.5` → the dummy is unguarded, nobody moves → `moved_max` RED.
**If these rows are RED on the unchanged rule (2.0)**, the geometry the rule cannot touch already starves a pair:
revert them to the old dummies with an updated comment ("since T16 this dummy is past the guarded zone: liveness +
exposure row", stray bound instead of the blanket zero) and report the s = 6 m result to the orchestrator. Do not
search for a dummy distance that passes.

**(C) Exposed (a spared body is past the zone, in the cone, not wall-shielded) → guarded-zone assertion + stray
bound, no blanket zero.**
`crossfire pair` (10/10 m), `surround4 mixed` (SMG 6 m vs pistol 10 m past), `surround4 cross` (13, 9 and 11 m past;
the 9 m pair straddles the 8.865 m boundary across jitters: j0 exposed by 0.14 m, j2 guarded by 0.41 m — the bound
absorbs it), `west pair + groupmate east` (15 m / 12 m past), and — **corrected from PLAN_V2** — `street along the
line` (civilians at x = 6, z ≈ 9-13.7: past the zone, inside the far SMG member's cone, in front of the z = 14 wall)
and `cross street behind the player` (civilians at x ≈ 7.5-9, z = 20/26: inside the (−1, −12.8) SMG cone, the ray
passes the wall end at x ≈ 6.35). Civilians flee only along their graph edges (`civilian::flee` → `flee_start`), so
these segments are the whole exposure.

**(D) Unchanged (blanket zero stays, plus the guarded-zone assertion).** `inline_file`, `wall beside the member`,
`dummies + idle rival` ×3 (every spared body inside the zone: (0,0,8) by 0.87-1.04 m, (0.9,0,3) by ~6 m),
`sidewalk behind the player` (z = 6, guarded by ≥ 2.6 m), `sidewalk between`, `sidewalk far behind` (x = 6, z ≥ 20:
wall-shielded for every in-cone sample), both fist-scrum tests and the human shield (pressed bodies are always inside
the zone), the five corridors, the punch tests, `gang_combat::members_never_shoot_their_own_group` (all shooters on
one side, nothing past the target), `car_fire_lines` (cars use `car_blocks`, which never looks past the target). If a
(D) row shows any friendly/bystander hit after the change, that is a rule bug: stop and report, do not relax.

**The assertions (in `gang_fire_lines.rs`).**
- `assert_guarded_zone(name, out)`, run **first** in every layout helper. Read `overshoot_margin` from the shipped
  file (`load_config::<GangConfig>(&assets_root(), GANG_CONFIG)`) and `overreach` from shipped `AimConfig` +
  `LocomotionConfig`, never from the app resource (so a test-local sabotage cannot move the comparator). For every
  friendly or bystander `DamageDealt` d, find its `shot_log` row (same shooter, `attack == d.shot`) with muzzle m and
  player position p. Violation iff `flat|d.point − m| < flat|p − m| + overshoot_margin − overreach`. Derivation: a body
  whose centre is past the zone (along ≥ D + m + 0.865 from the chest) is hit at a point ≥ D + m from the muzzle (muzzle
  ≤ 0.515 ahead of the chest, surface ≤ 0.35 before the centre) and `|p − m| ≤ D + 0.515`, so accepted strays are never
  flagged; the price is a blind band of about 0.9-1.4 m at the inner edge of the zone. Missing `shot_log` row →
  `GATE BROKEN`.
- `assert_keeps_firing` (classes A-D minus C): guarded-zone assertion, then the existing blanket zeros and `MIN_SHOTS`.
- `assert_keeps_firing_exposed` (class C): guarded-zone assertion, `MIN_SHOTS`, and the stray bound:
  `stray = |distinct (shooter, shot) among friendly ∪ bystander DamageDealt|`, `shots = Σ out.shots`,
  `assert!(stray ≤ min((0.05·shots).floor(), max(2·STRAY_MEASURED, 2)))`, printing `stray/shots` for every row.
  `STRAY_MEASURED` = the maximum measured over all class-C rows after the change (deterministic sim), with the per-row
  measurements in its doc comment. The floor of 2 is this plan's reading of Q-B when a row measures 0 (a literal
  "2 × 0" would forbid the non-zero stray hits Q1 accepts); flagged in §5.
  **If any class-C row measures > 5 %, stop and report with numbers. Do not tune `overshoot_margin` or the layouts.**
- Flip-RED for the assertions: (a) test-local gang `overshoot_margin = 0.5` (write the `GangConfig` resource after
  `gang_floor*`): the guarded-zone assertion must fire in `dummies + idle rival` (dummy (0.9, 0, 3): hit ~14.5 m from
  the muzzle vs threshold ≈ 18.7 m) or `sidewalk behind the player` (civilians at s = 6). The (0, 0, 8) dummy does NOT
  prove it: its 0.865 m zone margin is inside the assertion's blind band. Record which layout and which assertion went
  RED; if no guarded-zone violation appears, repeat with 0.0 and report. (b) test-local gang `overshoot_margin = 60`
  (old rule) → the 2.1 rows RED on displacement/time as recorded in 2.0. (c) in `blockers` replace
  `flat2(to - from).length()` by `0.0` → unit rows 2, 3 and 4 RED (row 1 stays unblocked, row 6 stays blocked).
  Restore, GREEN.

2.7 Full sim sweep: `cargo test -p gta_sim -j 2`. Deterministic count gates (`MIN_GROUP_HITS`, `UNRULED_GANG_CAR_HITS`,
`gang_city`, `witness_city`) may move because shot timing and the combat RNG stream shift. Re-derive such a number only
from a before/after measurement written into its comment, and never below its liveness meaning. **Every police test
(`police_*`, `wanted*`, `car_fire_lines`) must pass unchanged**: that is the proof that 60 m reproduces the old rule.

2.8 Runtime t9 (`tools/qa/scenarios/t9.py`), surgical.
- Read once: `overshoot = ron_number(ron_text("gang/gangs.ron"), r"overshoot_margin:\s*([\d.]+)")`,
  `aim_error` (`aim_error_deg`) and `fire_line_margin` from `gangs.ron`, `capsule_radius` from
  `character/locomotion.ron`, and the widest spread `max(base_deg + max_bloom_deg)` over the three `spread:` tuples of
  `combat/weapons.ron` (regex `findall`). No literals.
- Every firefight poll (both loops at `:269-298`): fetch all gang-0 members (`members(game)`: entity, position, state,
  held). For each HQ member h in `ids` and each other member o in `Attack` with a gun held: `D = flat|player − o|`,
  `u = unit(player − o)`, `along = (h − o)·u`, `lateral = |cross|`. Mark `crossfire_seen[h] = True` if
  `along > D + overshoot` and `lateral ≤ capsule_radius + fire_line_margin + along·tan(aim_error + widest)`.
- Verdict (`:337-342`): split `hurt` into `crossfire_hurt` (h with `crossfire_seen`) and `unexplained_hurt`. Fail if
  `unexplained_hurt` is non-empty (message unchanged "friendly fire: …"), if a member of `crossfire_hurt` ended `Dead`
  or at ≤ 0 HP ("crossfire killed a member"), or if `len(left) != len(start)`. Record
  `summary["firefight"]["crossfire_hp_lost"]` per member and `crossfire_seen`. Update the module docstring sentence
  "so every member must end the fight at full health" to "any loss is friendly fire unless a groupmate stood past the
  guarded zone on the far side (accepted crossfire, reported)".
- Gang strays can now also hit cops or civilians beyond the zone at runtime; nothing asserts on them. Report counts if
  seen, no assertion.
- t9 ×20 (Step 7) with 0 failures. A failure whose blockers are cops arresting (≤ 2.5 m, pressed → pinned by the
  accepted human-shield policy) or a body stopping at 8.0-8.865 m past the player (guarded) is reported with probe data
  (`scratch/planner/t9_probe.py`) as the known residual. **No second tactics patch.**

2.9 `docs/design/GDD.md` §6.3, after the "Бой:" bullet (line 280), one bullet: "- Линия огня охраняется от стрелка до
цели + `overshoot_margin` (банды 8 м, `gangs.ron`): тело дальше не удерживает огонь, шальное попадание туда допустимо.
Полиция держит полную дальность оружия (`escalation.ron`, 60 м)."

### Step 3 — Bench scene (client domain `src/bench/`)

3.1 `src/main.rs`
- `let bench = std::env::args().any(|a| a == "--bench-scene");` next to `flag_value`/`cli_seed`.
- Move the `load_config::<RenderConfig>(&root, RENDER_CONFIG)` block (today after `compose_sim`) to right after `root`
  is built (`:192`) and before `App::new()`; the later use keeps the same variable.
- Window: `primary_window: Some(Window { title, resolution: <bench res>, ..default() })` only when `bench`, with
  `WindowResolution::new(w, h).with_scale_factor_override(1.0)` (verified in `bevy_window-0.19.1/src/window.rs:922-935`:
  `new(physical_width: u32, physical_height: u32)`); otherwise unchanged.
- Main menu: `if cli.is_none() && !bench { app.insert_state(GameState::MainMenu); }`. Seed: `--seed N` if given, else the
  clock seed as today (t16 always passes `--seed 1`).
- `if bench { app.add_plugins(bench::BenchScenePlugin); }` and `mod bench;`.

3.2 Data: `assets/world/render.ron` `bench_resolution: (1920, 1080), // --bench-scene window, physical px (GDD §11:
1080p)`. `src/visuals/config.rs` `RenderConfig` gains `pub(super) bench_resolution: (u32, u32)` + `pub fn
bench_resolution(&self) -> (u32, u32)`; `validate()`: both > 0. Grep `RenderConfig {` in `src/` (today only the
definition; `city_gate.rs:33 render_config()` loads the file — confirm and fix if it builds a literal).

3.3 `src/bench/mod.rs` (new, < 300 lines). `pub struct BenchScenePlugin`. Systems in `FixedUpdate`,
`.in_set(PlayingSystems)`; state `#[derive(Resource, Default)] enum BenchPhase { #[default] Board, Chase }` plus a
`Local`/resource board timer.
- `board` (`Board`, `.before(VehicleSystems::Enter)`): if the player has `Driving` → `Chase`. Else choose the parked
  spot like `crates/gta_sim/tests/traffic_bench.rs:75-89` (the `City.0.parking` spot with `heading·(−position) > 0`
  nearest the origin), then the parked `Vehicle` (no `TrafficCar`/`PoliceCar`, `driver == None`) nearest it. Place the
  player at `vehicle::door_point(cfg.door, position, rotation)` + float height (set `Position` AND `Transform`, lesson
  TASK-011), set `ActionIntent.vehicle_requested = true`. Retry each tick. After 5 s without `Driving`, `error!("bench:
  could not board")` once (t16 turns log errors into a failure). The client only ever sets that latch to true and
  `seat.rs:235` takes it: no clobbering.
- `pin` (`Board` and `Chase`), named bench cheats (doc comment: "bench-only, keeps the §11 worst scene alive"):
  `WantedLevel.heat = WantedConfig.stars[last].heat`, `hidden = 0.0`, `last_known = Some(player position)`; player
  `Health` current/armor to `HealthConfig` max; the driven car's `VehicleHealth` to `DamageConfig` max.
- `drive` (`Chase`, `.before(VehicleSystems::Drive)`), writes the player's `DriveIntent` **every fixed tick**, with the
  comment "input::write_drive_intent overwrites DriveIntent in Update; FixedUpdate runs first in the main schedule".
  Never move it to `Update`. Pure fn `bench_target(graph: &TrafficGraph, position, forward, lookahead) -> Vec3`:
  current lane = lane with `dir·forward > 0.7` nearest the car (as `traffic_bench.rs:36-46`); `s` = projection on it;
  `lookahead = autopilot.lookahead_min + lookahead_per_mps·speed` (`sedan.ron:60`, no new constant). If
  `lane.length − s ≥ lookahead` → point on the lane. Else pick the **rightmost** connector of `lane.out`: maximise
  `to_lane.dir · lane.dir.cross(Vec3::Y)`; the remaining distance is walked along `connector.points` by
  `connector.cumulative`, then along `to_lane` from its `from`. Then `DriveIntent { steer: pursuit_steer(cfg, position,
  forward, forward_speed, target), throttle: speed_throttle(cfg, &cfg.autopilot, traffic.turn_speed, forward_speed),
  handbrake: false }` on the PLAYER (the seated driver's intent wins, `chassis.rs:109-115`). No stuck recovery: a stuck
  car is reported by t16, not hidden.
- `BenchFrames` (`#[derive(Resource, Reflect, Default)] #[reflect(Resource)] { recording: bool, frame_ms: Vec<f32> }`,
  registered): a `Last` system pushes `Time<Real>::delta_secs() * 1000.0` while `recording`. Default false, so an owner
  run without BRP never grows it.

3.4 Unit tests in `src/bench/mod.rs` (`cargo test -p gta_like --bin gta_like`): the rightmost-connector choice with
worked rows (Bevy: forward −Z, right = `dir.cross(Y)`): dir (0,0,−1) → right (1,0,0); dir (1,0,0) → right (0,0,1);
dir (0,0,1) → right (−1,0,0); a T-junction with no right turn (straight and left only) → straight, because the right
dot ranks straight (0) above left (−1). Plus: the target stays on the
lane while `lane.length − s ≥ lookahead`. No headless city gate (a broken bench is visible on the first t16 run).

3.5 `README.md` "Запуск": `cargo run --release -- --bench-scene [--seed 1]` (худшая сцена §11, окно 1080p; `--features
profile` для chrome-трейса, `profile-tracy` для Tracy).

### Step 4 — `tools/qa/trace.py`, `tools/qa/scenarios/t16.py`

4.1 `tools/qa/trace.py` (stdlib): `summarize(path, window_s=10.0)`. Read the last ~64 KB to find the max `ts`. Stream
the file line by line (strip `[`, `]`, leading `,`; tolerate a truncated last line; never `json.load` the file). Keep
per-`tid` stacks of `B` events, aggregate the matching `E` only if `B.ts ≥ max_ts − window`. Returns: frames (`update`
spans, `bevy_app-0.19.1/src/sub_app.rs:576`: count, mean/p50/p99/max ms), FixedMain (`schedule: name=FixedMain`,
`schedule.rs:562`: count, mean ms per tick), systems (`system: name="…"`, `function_system.rs:52`: total ms per name and
per frame). Shares by name prefix: AI = `gta_sim::` modules from `crates/gta_sim/src/lib.rs` `pub mod` minus the non-AI
list `{character, combat, config, flow, layers, player, vehicle, world}` read from the file (TASK-015 lesson); physics
= `avian3d::` / `bevy_tnua`; unmatched `gta_sim::` modules reported as "unclassified".
`tools/qa/test_trace.py` (unittest, offline): synthetic 3-frame trace, two threads, a nested system span, a truncated
last line → exact totals. Add `python -m unittest tools/qa/test_trace.py` to `.github/workflows/repo-checks.yml` next to
`test_brp.py` (`:37-40`).

4.2 `tools/qa/brp.py` `Game.__init__` gains `env=None`; `start` merges it into the `env` copy (`:54`). One line each.

4.3 `tools/qa/scenarios/t16.py --out <dir>`, reusing `t5.game_state/wait_chunks/screenshot/resource_value`, `t6.rows`,
`t15` helpers, `t11.wanted`:
- Session A: `Game(features=("dev",), args=("--seed", "1", "--bench-scene"), release=True)`. Wait for `Playing`,
  golden hash, chunks. Composition deadline 120 s: player `Driving`, stars == 5, active `PoliceCar` == escalation
  row-5 `cars`, live `PoliceUnit` == row-5 `units` (both parsed from `escalation.ron`), `TrafficStats.cars ≥ 20`.
  Otherwise `AssertionError("GATE BROKEN: bench scene never reached …")` naming the missing part. Record civilians
  alive, gang members alive near the bench centre (Q3: honest count, no scene hacking), units by kind, cars, `Window`
  physical size, car speed every 1 s. Three screenshots ≥ 0.5 s apart. `frame_report()` (switches to `AutoNoVsync`).
  Then `BenchFrames.recording = true`, poll `get_diagnostics` every 0.5 s for 30 s, stop recording, read `frame_ms`:
  mean FPS, 1 % low FPS (mean of the slowest 1 % of frames), min FPS, p50/p99/max frame ms, diagnostics averages. Log:
  no `ERROR` lines (t5 `ERROR_WORDS`).
- Session B: `env={"TRACE_CHROME": str(out/"trace.json")}`, features `("dev", "profile")`, same args and composition
  wait, a 10 s window with 20 `get_diagnostics` samples (labelled "with tracing"), `game.shutdown()`,
  `process.wait(15)`. The file must end with `]` (clean `FlushGuard` drop), else report "trace truncated" and still
  parse. `trace.summarize(out/"trace.json", 10.0)`.
- `summary.json` + stdout: composition, window size, monitors/present mode, A metrics, B top-15 systems per frame,
  FixedMain per tick, AI and physics ms per tick vs the §11 budgets (4 ms / 1.5 ms), verdict numbers. Hard failures:
  composition not reached, a crash, `ERROR` in the log, trace missing or unparsable. FPS is never pass/fail (GDD §11).
- The `--bench-scene gangs` variant is **not** built (Q3): written up as "not cheap: needs a different centre rule".

### Step 5 — Measure and decide by trace only
Run t16 ×3. IMPL_SUMMARY table from the worst of the three: mean FPS, 1 % low, p99 ms, FixedMain / AI / physics per
tick, top-5 systems. Rule: fix only if a budget is exceeded (frame > 16.7 ms at 1080p with no vsync, FixedMain > 4 ms
per tick, AI > 1.5 ms per tick): the top offender, one §13 lever per round (time slicing via an existing cursor/slot,
render chunk / merge, `OcclusionCulling` + `DepthPrepass` measured before/after, kinematic far NPCs), t16 before/after,
any new knob in its §12 data file. Otherwise the verdict is "nothing to fix by trace" with the table (expected from
TASK-016: 3.0-3.7 ms frames at 5★).

### Step 6 — §1 evidence sweep `tools/qa/scenarios/t16_s1.py`
6.0 Promote `maw/tasks/done/TASK-013/scratch/qa/osinput.py` verbatim to `tools/qa/osinput.py` (stdlib ctypes,
Windows only; the phase needing it is skipped elsewhere with a reported reason).
6.1 Phases (own `summary.json` section and screenshots each; hard assertions only on components; seed 1 unless noted):
- **P1 menu:** own `Game` session WITHOUT `--seed`. `GameState == MainMenu`, screenshot. `send_keys(["Enter"], 100)`
  (empty field → clock seed, `menu/screens.rs:250-259`). Poll `Loading` (screenshot if caught), then `Playing`. Assert
  `CitySeed != 1` or a non-golden layout hash, and exactly one player.
- **P2 camera vs wall:** use the city-edge wall of t14 (`from t14 import WALL_FACE_Z`, z = 700). Teleport the player
  1.0 m from the face, facing inward; set `OrbitCamera.yaw` so the boom points into the wall; wait 0.5 s; read the
  camera `Transform` and player `Position`. Assert boom length (pivot → camera) < `camera.ron` distance − 0.5 m and the
  camera on the player's side of the wall. Screenshot. Repeat with the boom away from the wall: full distance.
- **P5 gang Warn → P3 drop pickup:** t9 helpers (HQ group, approach post). Heat 0, stand 6 m from the group
  (< `warn_distance` 8 m), wait `warn_seconds` + 1 s → a member in `Warn`. Named QA mutation `Health.current = 0` on one
  member (no shot); assert gang heat unchanged after it (else report). Wait for a `Dropped` `WeaponPickup` near it,
  teleport onto it, assert the gun owned or its reserve grew.
- **P7 run over:** a `Dummy` at the range (`CityLandmarks.park_center`; mind the SMG pickup there, lesson TASK-014).
  Nearest parked car 15 m from the dummy facing it (`Position` + `Rotation` mutation, t14 precedent), F at its door,
  hold W 2 s. Assert the dummy's `Health` dropped or its `HitReaction` knocked down within 3 s. Screenshot.
- **P8 junction yield:** sample `TrafficCar` rows every 0.25 s for 15 s; assert ≥ 1 car with `waiting.is_some()` and
  speed < 0.5 m/s that later moves on another segment. Screenshot at a junction.
- **P9 death keeps guns:** pistol from the range; heat to 2★, one full-HUD screenshot (health, armour, weapon/ammo,
  stars, minimap); `DebugDamage` (t5 helper) to 0 → `Wasted` → `Playing`. Assert the pistol still owned with the same
  magazine + reserve and the position within the t5 hospital radius.
- **P10 settings UI:** Esc (one BRP key); `osinput.click` "Настройки" (button found by its text child, TASK-014
  `probe_settings.py` method), screenshot, click one toggle; assert the `GameSettings` field flipped; Esc back.
6.2 Coverage table in IMPL_SUMMARY (QA copies it): §1 points 1-11 → scenario/phase → what is asserted → last result.
Points already covered link their t*.py (PLAN.md §1.4 table); point 11 → t16. No owner checklist (TASK_FINAL).

### Step 7 — Repeat runner and full gates
7.1 `tools/qa/repeat.py` (stdlib, ~40 lines): `python tools/qa/repeat.py t9 --runs 20 --out target/qa/rep` runs
`tools/qa/scenarios/<name>.py --out <out>/run<k>` sequentially, prints `k pass/fail` and the first `AssertionError`
line, exits 1 if any run failed.
7.2 Headless: `cargo build -j 2`; `cargo clippy --workspace --all-targets -j 2 -- -D warnings`;
`cargo clippy -p gta_sim -p citygen --all-targets -j 2 -- -D warnings`; `cargo test -p gta_sim -j 2`;
`cargo test -p citygen -j 2`; `cargo test -p gta_like --bin gta_like -j 2` (3× for any touched presentation gate);
benches (record means): `cargo test -p citygen --release --test perf -j 2 -- --ignored --nocapture`,
`cargo test -p gta_sim --release --test city -j 2 -- --ignored city_startup_budget --nocapture`,
`cargo test -p gta_sim --release --test civilian_bench --test police_bench --test traffic_bench -j 2 -- --nocapture`
(not `#[ignore]`d; they also run in the debug sweep); `python tools/qa/tree_check.py`;
`python -m unittest tools/qa/test_brp.py tools/qa/test_trace.py`. Before trusting any red in untouched code from the
shared `target/`, `touch crates/*/src/lib.rs` and rebuild (lesson TASK-009).
7.3 Runtime (release, `--features dev`) via `repeat.py`: every t1..t15, t16, t16_s1 ×3; t13 ×10; t9 ×20. Record pass
counts, never "one green run". Budget ~2-3 h wall clock, sequential.

### Step 8 — Docs (surgical)
- `docs/design/GDD.md` §4.1, after the line "Патроны: …" (after the roster table): "Нажатие огня в последние
  `fire_buffer_seconds` кулдауна ствола запоминается и стреляет по его окончании (`weapons.ron`); остальные нажатия в
  кулдауне и перезарядке теряются."
- GDD §6.3 line (2.9). README "Запуск" (3.5). Narrative graph and status: the orchestrator's job after closing.

---

## 3. Test plan

| What | How | Expected |
|---|---|---|
| Buffer fires on expiry | `shooting::a_press_in_the_buffer_window_fires_when_the_cooldown_ends` | 1 extra shot exactly on k = 20 (shipped), magazine 12 → 10 |
| Early press dropped | `a_press_long_before_the_cooldown_ends_is_dropped` | no shot through k = 70 |
| Switch clears queue | `a_queued_press_is_dropped_on_weapon_switch` | no shot through k = 30 |
| Death clears queue | `respawn.rs` new gate | `ShotFired` unchanged over 64 `Playing` ticks |
| Semi-auto meaning kept | `semi_auto_vs_automatic` (derived offset) | 1 shot, then 2 |
| Config | `fire_buffer_seconds_is_not_negative`, gang/police `overshoot_margin` fixtures | errors name the field |
| Rule kernel | `fire_line::overshoot_limits_the_guarded_zone` rows 1-7 | as worked in 2.5 |
| Crossfire starvation fixed | `crossfire_fires_from_the_post` R95/R12/R95off (×3 jitters) | first shot ≤ `FIRST_SHOT_S`, `moved_max < 0.5`, stray ≤ bound; RED under old rule (flip b) |
| Residual | R85 | `MIN_SHOTS`, zero bystander damage; `first_shot_s` printed |
| Yields rule | re-anchored lower-index test at ±8.4 | low plans and moves > 0.5 m, high stands < 0.1 m |
| Pair unblock | re-anchored side_by_side rows (dummy 6 m past) | `MIN_SHOTS`, `moved_max ≥ 0.5`, zero hits |
| Correctness of the rule | `assert_guarded_zone` on every layout | no hit inside the zone; flip a RED on (0.9,0,3) dummy or z = 6 civilians |
| "Small" crossfire | class-C rows | stray ≤ min(5 % of shots, max(2·measured, 2)); rates printed |
| Police unchanged | all `police_*`, `wanted*`, `car_fire_lines` | pass without edits |
| Whole sim | `cargo test -p gta_sim`, `-p citygen`, clippy `-D warnings`, `gta_like --bin` | green |
| Bench/perf | t16 ×3, `trace.py` + `test_trace.py` | composition reached, metrics + trace table, no errors |
| §1 evidence | t16_s1 ×3 | each phase asserts its component |
| Flakes | t13 ×10, t9 ×20, all others ×3 | all pass; t9 residuals reported with probe data |

Every flip-RED (1.5 a-d, 2.6 A/B flips, 2.6 a-c) is recorded in IMPL_SUMMARY with the perturbed input and the
assertion that fired.

---

## 4. Rollout notes

- **Data (strict loaders, `deny_unknown_fields`)**: `weapons.ron` `fire_buffer_seconds`, `gangs.ron` and
  `escalation.ron` `combat.overshoot_margin`, `render.ron` `bench_resolution`. No new tuning `const`.
- **Features/flags**: `--bench-scene` CLI flag (client only). `profile` cargo feature already exists
  (`bevy/trace_chrome`); `TRACE_CHROME` env var picks the file (`bevy_log-0.19.1/src/lib.rs:325`).
- **Behaviour changes**: gangs now fire past groupmates/bystanders standing > 8.865 m beyond the player (accepted
  crossfire, GDD §6.3 line). Police unchanged. Semi-auto presses in the last 0.15 s of a cooldown now fire.
- **No migrations, no saves.** `Loadout` gains a reflected field: BRP scripts mutate by path only (checked t6/t7/t9/t11),
  no full-struct inserts.
- **CI**: `repo-checks.yml` runs `test_trace.py`.
- Disk: session-B traces can reach 1-2 GB at high FPS; `target/qa` is gitignored; parse by streaming only.

---

## 5. Review notes (changes from PLAN_V2, with evidence)

Disconfirmation tested first: "an existing gang gate that PLAN_V2 files as guarded/zero-safe but whose spared bodies
are past the 8.865 m zone, in the cone and not wall-shielded". **It held**: `scratch/pr2/layout_geometry.py` finds
civilians of `street along the line` (x = 6, z ≈ 11) and `cross street behind the player` (x = 8, z = 20) in the far
SMG member's cone, past the zone, unshielded. V2 called z ≥ 20 wall-shielded; the ray from (−1, −12.8) to (8, 20)
passes z = 14 at x = 6.35, beyond the wall end.

1. **Q-A and Q-B folded** (binding). Police 60, gangs 8; V2's option-B step 2.8 and the open-question section are
   dropped. V2's t11 cop-on-cop instrumentation is dropped (police are bit-identical by construction and by the
   unchanged police gates; new t11 code would be scope creep).
2. **Lower-index re-anchor ±8.4, not ±7.5.** 7.5 < `keep_distance.0` = 8 → `band_move` `BackOff` (`gang/fsm.rs:88-96`):
   both members walk during the gun draw, and the high index's `moved < 0.1` can go RED for a reason unrelated to
   `yields`. ±8.4 is in the band (Hold) and guarded by 0.465 m; worked spot/yields numbers in 2.6 A.
3. **Two street layouts moved to the exposed class** (see disconfirmation). With the blanket zero they could go RED on
   an accepted stray.
4. **The four side_by_side dummy rows are re-anchored (dummy 6 m past), not kept as exposure rows.** Under the rule
   none of them is blocked any more, so "pairs do not lock each other" tested nothing (the +z rows are also
   wall-shielded, so they would not even be exposure evidence). Exposure evidence stays in class C and the new R rows.
   A fallback is specified if the re-anchored rows are RED on the unchanged rule.
5. **Guarded-zone assertion flip fixed.** V2's flip (a) named the (0, 0, 8) dummy seen by the pistol member. The
   assertion's own slack (overreach 0.865 + muzzle advance ~0.45 m) exceeds that dummy's 0.865 m zone margin, so a hit
   on it is never flagged: the RED would come only from the blanket zero. The flip now names the (0.9, 0, 3) dummy and
   the z = 6 sidewalk civilians, and the assertion runs before the blanket zeros. The comparator reads the shipped file,
   not the (sabotaged) app resource.
6. **R rows no longer assert zero bystander damage.** V2 asserted zero on R95off while calling it the unshadowed
   rate-feeding case: contradictory by design. R95/R12/R95off assert the guarded-zone assertion and the stray bound.
7. **Unit-test flip (c) corrected**: replacing D by 0 turns rows 2, 3 and 4 RED, not "1-3" (row 1 stays unblocked).
   Added row 7 pinning `reach()` for the car filter.
8. **Stray bound made executable**: unit = distinct (shooter, shot) attacks that hit a spared body over `ShotFired`
   count, per row; `STRAY_MEASURED` = max measured over class C. **Interpretation flagged for the orchestrator:** Q-B's
   "≤ 2× measured" gets a floor of 2 hits when a row measures 0, otherwise it collapses to a zero-hit gate that
   contradicts Q1's "non-zero allowed". If the orchestrator wants the letter, set the floor to 0.
9. **t9 runtime verdict specified**: crossfire geometry from positions with the widest gang cone derived from ron
   files, split of `hurt` into crossfire vs unexplained, "killed by crossfire" fails, docstring updated.
10. **Missed comment site**: `gang/behavior.rs:393` also says "a miss flies on to the weapon range".
11. **Restored from PLAN.md where V2 compressed without correcting**: exact fire-buffer worked ticks and flip (d) for the
    respawn clear, bench systems (`board`/`pin`/`drive`/`BenchFrames`) with the connector walk made concrete
    (`connector.points` + `cumulative`), t16 session A/B, trace parser, §1 phases (P2 now uses t14 `WALL_FACE_Z`, which
    exists), repeat runner, full command list, docs lines.
12. Verified, unchanged from V2: overreach 0.8648 m; the 4 `FireLine::of` sites; `nearby_cars` is the only other
    consumer of the reach; gang/police `positive()` validators exist; `WindowResolution::new(u32, u32)` and
    `with_scale_factor_override` in `bevy_window-0.19.1`; `profile = ["bevy/trace_chrome"]`; `brp.py:43` `-j 4`;
    gang strays at cops/civilians raise no player crime (`wanted/crimes.rs` counts only player shooters); civilians
    flee only along graph edges; melee spares (`gangs.spares`) apply to fists only, bullets deal damage.

Risks carried: R1 class-C stray rate > 5 % → stop and report; R2 re-anchored pair rows RED on the old rule → fallback
+ report; R3 t9 residuals (arrest-pressed cops, bodies at 8.0-8.865 m, long1's unattributed damage) → report with
probe data; R4 the buffer lets a gang shotgun fire ≤ 0.15 s after its line check (fire-line gates judge it);
R5 deterministic count gates drift (re-derive from before/after only); R6 trace size / first `dev,profile` build;
R7 bench car stuck / 1080p window on a smaller monitor / no gangs downtown / OS clicks (all reported, not hidden);
R8 scope creep → findings for TASK-031/032.

children: 0 launched / 0 reported.
