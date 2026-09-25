# PLAN_V2 — TASK-017 (GDD T16): bench scene, trace-driven performance, bug bash, §1 evidence

Reviewer: plan-reviewer-1. Inputs: `TASK_FINAL.md` (binding orchestrator decisions: fire buffer, Q1 `overshoot_margin`
rule instead of flank spots, Q2 N = 3/10/20, Q3 honest bench), `PLAN.md`, the code at `feature/t16-final`.

---

## 0. Disconfirmation (done first)

**Counter-example I looked for:** an existing fire-line gate whose zero-damage or hold-fire assertion needs a spared body
standing **past the target by more than `overshoot_margin`** and inside weapon reach. The Q1 rule stops guarding such a
body, so the gate either goes RED by construction or starts depending on luck.

**It holds, and more strongly than the Q1 text assumes:**

- `crates/gta_sim/tests/police_range_edge.rs:111-137` `cops_hold_fire_at_the_range_edge_across_the_player`: two SWAT
  45.3 m apart with the player in the middle. It asserts `fired == [0, 0]`. Under the new rule each cop's partner is
  22.65 m past its target, well beyond 8 m, so both fire. **RED by construction.**
- `police_range_edge.rs:141-226`, five `surrounding_swat_never_hit_each_other_street_*` tests: 12 SWAT in a walled
  street, 6 at each end, with `assert_eq!(friendly, 0)`. SWAT `keep_distance` is `(5.0, 12.0)` (`escalation.ron:14`).
  Cops that settle 9-12 m from the player on opposite sides are 9-12 m past each other's target. The rule stops guarding
  them, and a dense column of 6 bodies sits in the spread cone (police SMG cone 2° + 7° = 9°). Cop-on-cop hits are
  **likely** here. This is the canonical 4-5★ encirclement, not a rare layout.
- Gang gates with opposite-side bodies past the zone (`gang_fire_lines.rs`): `crossfire pair` (10 m / 10 m),
  `surround4 mixed` (6 m vs 10 m), `surround4 cross` (9 m vs 13 m, 11 m vs 7 m), `west pair + groupmate east` (12 m vs
  15 m), and the two `west pair … + dummy east` rows (a dummy 25 m past the target, 37 m from the shooters, within SMG
  and pistol range). Each asserts `friendly.is_empty()` / `bystander_hits.is_empty()`. Under the new rule these are
  exposed to stray hits.
- `gang_fire_lines.rs:476-519` `of_two_members_blocking_each_other_the_lower_index_moves`: members at z = ±10. Each is
  10 m past the other's target, beyond 8 + 0.865 m, so neither blocks, `low_planned` stays false, and the gate is **RED
  by construction**.
- `tools/qa/scenarios/t9.py:337-342` asserts that no HQ member lost health (friendly fire). The t9 failure geometry
  (a second group or cops 9.4-9.5 m past the player) becomes a crossfire the rule now allows. The post-14 group can
  therefore hit the HQ members, and t9 can fail on its **friendly-fire** assertion instead of the starvation one.

PLAN.md's step 2 (flank spots) is superseded by Q1 anyway. But `TASK_FINAL` Q1 only says "the existing same-side
'0 friendly damage' gates stay green". It does not say what happens to the opposite-side gates listed above, or to the
police. PLAN_V2 settles that below (§3.2, Q-A).

---

## 1. Review notes (issues in PLAN.md, with evidence)

1. **Step 2 (flank spots) is obsolete.** Q1 replaces it with the `overshoot_margin` rule. Rewritten in full (§3.2, Step 2).
2. **The root-cause table in §1.2 is wrong: the starvation is not "only with the second gang group".**
   `scratch/planner/t9_run3/probe.json` is a FAIL with no `g0p14` in any wedge. Its blockers are three `"other"`
   bodies: in `t9_probe.py:53` that label means a living `Character` that is neither a civilian nor a `GangMember`, so
   **cops**. The player fired a shot (`summary.json` magazine 12 → 11), which brought the police. They walk from 30.9 m
   to 9.4 m past the player, and both HQ members sit at `trigger_left = 0`, `sees = true`, magazine 30 for 6.8 s. So
   the class is "any spared body approaching past the target": a second group, cops, and potentially civilians. The
   Q1 rule covers all of them, because the gang's `spares()` spares police and civilians (`gang/mod.rs:350-355`). The
   **residual** also includes cops walking in to **arrest** at 1★. They cross the guarded zone and end up pressed
   (≤ 2.5 m, `pressed_distance` = gang `melee_distance.1`), which pins the gunman by the accepted human-shield policy
   (TASK-010 lesson). t9 ×20 can still fail on that. It must be reported, not patched.
3. **Margin is thin at the observed geometry.** With the orchestrator's ~8 m the guarded zone ends at
   `D + 8 + overreach`, and overreach = |(0.25, −0.45)| + max(0.3, 0.35) = 0.5148 + 0.35 = **0.865 m** (`aim.ron:1`,
   `locomotion.ron:12,19`). The t9 blockers stood 9.4-10.3 m past the player (run3), so they are clear of the zone by
   only 0.5-1.4 m. An opposite member standing at its band's near edge (`keep_distance.0` = 8 m) stays guarded, so
   the old unblock path still handles that case. This is acceptable only if it is measured (gate row s = 8.5 below).
4. **The crossfire fixture in PLAN §2.1 collides with the test area.** Arrivals at `polar(31, 180)` = (0, 0, 31) and
   `polar(33, 184)` stand **behind the test-area wall** at (0, 2, 14), 12 × 4 × 0.5 m (`world/test_area.rs:21`). They
   cannot see the player, which is gates lesson TASK-012 exactly. The same wall shields the `+z` fixtures
   `pair + dummy behind the player` (0, 0, 25), `sidewalk far behind` and `cross street behind` from stray bullets.
   That is a lucky property. New crossfire fixtures go along the **x axis** (z ≈ 0), which is clear from x = −40 to
   +40 on the 80 m floor.
5. **The boundary sits exactly on existing fixtures.** The dummy at (0, 0, 8) in `members_fire_past_bystanders_…`
   (`gang_fire_lines.rs:333`) and in `cops_never_hit_bystanders` (`police_fire_lines.rs:182`) is 8 m past the target.
   If the zone were `D + overshoot` without overreach and overshoot = 8.0, that dummy sits **on** the boundary (f32,
   gates lesson TASK-007). The rule must include overreach (bullet reach to the near surface, TASK-012 lesson). Then
   the dummy is 0.865 m inside the zone, with every jitter checked in §3.2.
6. **`FireLine.reach` has a second consumer.** `nearby_cars` (`fire_line.rs:163-178`) filters cars by `s.line.reach`.
   The zone limit depends on the target distance, but that filter must not change. Keep `range + overreach` for the car
   filter and add the per-call limit only in `blockers`.
7. **No config fixture gates for the new fields.** `fire_buffer_seconds` and `overshoot_margin` each need a strict-loader
   sabotage fixture that sits strictly on the failing side. The precedent is `tests/config.rs:613-620`
   `fire_line_margin_is_not_negative`.
8. **Minor, verified correct:** hitscan drop comment and flow (`hitscan.rs:187-203`); `tick_loadouts` → melee →
   `fire_weapons` chain (`combat/mod.rs:91-106`); `select_weapon` resets `reload_left` (`weapons.rs:356-380`);
   `WeaponsConfig` is `deny_unknown_fields` with the finite list at `:171-184`; `Loadout` is `Reflect + Default` and
   test literals use `..default()` (`respawn.rs:86`); `brp.py:43` hard-codes `-j 4`; `WindowResolution::new(u32, u32)`
   and `with_scale_factor_override(f32)` exist in `bevy_window-0.19.1/src/window.rs:922-935`; `TRACE_CHROME` and
   `FlushGuard` are in `bevy_log-0.19.1/src/lib.rs:63-71,322-343`; `get_diagnostics` gives only average and smoothed
   (`bevy_brp_extras-0.22.6/src/diagnostics.rs:45-72`); `.github/workflows/repo-checks.yml:37-40` runs `test_brp.py`.
9. **Bench driving vs the client input.** `write_drive_intent` (`src/input/mod.rs:306-328`) **overwrites**
   `DriveIntent` every `Update` frame, and with no cursor capture it writes zeros. This is harmless only because the
   bench writes the intent in `FixedUpdate` `.before(VehicleSystems::Drive)` on every fixed tick (`FixedUpdate` runs
   before `Update` in the main schedule). The plan must say so and must not move the bench write to `Update`.

---

## 2. Updated understanding (corrected)

### 2.1 Fire-line discipline today (`tactics/`)
- `FireLine { reach = range + overreach, cone = aim_error + max(base + max_bloom, spread_deg), clearance =
  capsule_radius + fire_line_margin }` (`fire_line.rs:27-52`). `blockers(from, to, bodies)` flags a body with
  `0 < along < reach` and lateral ≤ `clearance + along·tan(cone)` (`:61-76`). The limit is **the full weapon reach
  regardless of target distance**.
- Consumers of `blockers`/`blocked`: `hold_fire` own line (`mod.rs:203`), the mutual-block `yields` rule (`mod.rs:208-215`,
  both lines), `usable` (`fire_line.rs:228`), `queue_slot` (`:271-289`), `pinned` (`:294-304`). All of them pass
  `(from, to)`, so a per-call limit computed from `|to − from|` applies uniformly.
- Construction sites: `FireLine::of` in `gang/behavior.rs:239` (the shared `Shooter` list) and `:395` (own line), and
  in `police/behavior.rs:205` and `:414`. The `Discipline` builders are `gang/mod.rs:360-370` and `police/mod.rs:177-187`.
- The police spare everything except the player (`police/behavior.rs` `|f| f != Some(Faction::Player)`). Gangs spare
  every non-hostile faction, cops and civilians included.
- Hitscan (`hitscan.rs:233-300`): the aim point is a ray from the **eyes** along the aim (NPC aim carries the role's
  error cone, `gang/behavior.rs:122-137`). The pellet axis goes from the muzzle to that point, and pellets are cast
  over `range`. A miss flies on to `range` and hits the first body. A spared body collinear behind the target is
  mostly **shadowed** by the target. Muzzle parallax (0.25 m right) and lateral offsets expose it.

### 2.2 t9 flake (corrected)
Evidence: `scratch/planner/t9_*`. Three FAILs out of 20: run12 (post-14 gang group behind the player), run3 (cops
behind the player), long1 (unexplained friendly damage while the post-14 group engaged). The mechanism is the full-reach
wedge plus the sidestep pivot, which cannot clear a body about 10 m past the target from about 11 m out, plus the
mutual `yields` stand. The fix is the Q1 rule.

### 2.3 Everything else
PLAN.md §1.1 (firing path), §1.3 (bench / measurement), §1.4 (§1 coverage) are accurate. See note 8 for what I checked.

---

## 3. Revised approach

### 3.1 Fire buffer (unchanged from PLAN §2.1, plus a config gate)
One request that lands while `0 < cooldown ≤ fire_buffer_seconds` sets `Loadout.fire_queued`. It fires on the first
tick the cooldown is 0. It is cleared on reload, stagger, held-gun change, Wasted/Busted exit and new city. Data:
`weapons.ron` `fire_buffer_seconds: 0.15`. Input buffers are typically 80-250 ms and must clear on a context change
(PLAN's sources).

### 3.2 Q1 rule: guarded zone = muzzle → target + `overshoot_margin`
**Rule (one code path in `tactics/`, value per role in data):**

```
limit(from, to) = min(range, flat|to − from| + overshoot_margin) + overreach
blocked body  ⇔  0 < along < limit  ∧  lateral ≤ clearance + along·tan(cone)
```

`FireLine` stores `range`, `overreach` and `overshoot`. `reach()` = `range + overreach` stays the car-filter radius
(note 6). A line to a target farther than `range − overshoot` reduces to today's behaviour.

This is the same design as a "muzzle to fuze point" line-of-fire check, where friendlies past the aim point are not
considered ([starwards PR #2275](https://github.com/starwards/starwards/pull/2275),
[issue #2268](https://github.com/starwards/starwards/issues/2268)). Stray hits past the zone are accepted, as the
orchestrator decided.

**Value derivation (gang, `gangs.ron` `combat.overshoot_margin: 8.0`).**
- Lower bound: the zone must keep guarding the fixtures that stand for "a bystander right behind the player". The dummy
  at (0, 0, 8) against a shooter 12 m out: guarded iff `20 < 12 + m + 0.865`, i.e. **m > 7.135**. At m = 8.0 it is
  0.865 m inside. With jitter j1 the shooter is at (0.37, −12.21): D = 12.216, along = 20.20 < 21.08. With j2 at
  (−0.53, −11.56): D = 11.572, along = 19.55 < 20.44. The pistol member at (2, −11): D = 11.18, along = 19.06 < 20.05.
- Upper side, and the observed residual: an opposite shooter stays guarded while it is closer than 8.865 m to the target.
  The t9 blockers stood at 9.4-10.3 m, clear by 0.5-1.4 m. The band's near edge (8 m) is guarded, and the old unblock
  path (mutual yield → lower index moves) still handles it. Gate row s = 8.5 measures it.
- Spread context, reported and not a threshold: for a 0.6 m body inside the cone and unshadowed, P(hit | miss) is
  roughly `0.6 / (2·(D + s)·tan cone)`. For the gang SMG (11°, tan 0.194) at D = 11 that is 14 % at s = 0 and 8 % at
  s = 8. For the gang pistol (8°) it is 11 % at s = 8. m = 8 is where the cone is about 7× a body width.
- m = 8.0 is the orchestrator's ~8 m and satisfies the lower bound with a 0.865 m buffer. **Do not use 7.x**: that
  would silently unguard the (0, 0, 8) fixtures.

**Police (Q-A, recommended default: keep today's behaviour).** `escalation.ron` `combat.overshoot_margin: 60.0`, with
the comment "≥ the longest police gun range (pistol 60 m): cops keep full-reach discipline". Then
`min(range, D + 60) = range` for every D ≥ 0, so police behaviour is **bit-identical** and every police gate stays as
it is. The rule is still universal: the same code path, with the role difference in data.

Reasons:
(a) the t9 flake is about **gang** lines; cops only appear as blockers, and the gang's margin governs that;
(b) with 8 m for police, `cops_hold_fire_at_the_range_edge_across_the_player` goes RED by construction and the five
SWAT-street zero-friendly gates are likely RED in the main 4-5★ encirclement. That is a silent gameplay change in the
last slice, with no runtime scenario judging cop-on-cop fire.
If the orchestrator wants 8 m for police too (option B), follow the steps in §4 Step 2.8.

### 3.3 Bench scene, trace, §1 sweep, repeat runner
Unchanged from PLAN §2.3-2.6, with the corrections in notes 9 and 4.

---

## 4. Revised steps (complete)

Host rules: one foreground cargo at a time, always `-j 2`. `tools/qa/brp.py:43`: `-j 4` → `-j 2`. Build
`--features dev,profile --release` once early (Step 4.0).

### Step 1 — Fire buffer (sim, gated)
1.1 `crates/gta_sim/src/combat/weapons.rs`: `WeaponsConfig` gains `pub fire_buffer_seconds: f32` (doc: seconds before
the cooldown ends in which a trigger press is kept and fires on expiry). Add it to the `validate()` finite list and add
`check(fire_buffer_seconds >= 0.0, …)`. `Loadout` gains `pub fire_queued: bool` (doc). `select_weapon`: clear
`fire_queued` next to `reload_left = 0.0` on a held change.
1.2 `assets/combat/weapons.ron`: top level `fire_buffer_seconds: 0.15, // s: a press this close to the end of the
cooldown fires when it ends`.
1.3 `hitscan.rs` `fire_weapons`: new comment at `:187`. Order: take `requested`; `reaction.is_active()` → clear
`fire_queued`, `continue`; no held gun → `continue`; `reload_left > 0` → clear, `continue`; `slot.cooldown > 0` → if
`requested && slot.cooldown <= cfg.fire_buffer_seconds` set `fire_queued`, `continue`; otherwise
`let queued = mem::take(&mut loadout.fire_queued)`; semi: `wants = requested || queued`; automatic:
`fire_held || requested || queued`. Everything else is unchanged. Use the existing `let loadout = &mut *loadout;` for
the disjoint field borrows.
1.4 `combat/mod.rs`: `reset_fire_queue` (`Query<&mut Loadout>` → false) on `OnExit(Wasted)`, `OnExit(Busted)` (next to
`melee::reset_player_melee`, `:74-75`) and in `NEW_CITY` (`:76`).
1.5 Gates (`tests/shooting.rs`; every tick derived from `Time<Fixed>::timestep()` + `WeaponsConfig`):
- `a_press_in_the_buffer_window_fires_when_the_cooldown_ends`. Pistol shot at tick 0. Press at the tick where the
  remaining cooldown after the decrement is in `(0, buffer)` away from both edges; with shipped data that is k = 17,
  0.034375 s. Exactly one more `ShotFired`, on k = 20. Magazine 12 → 10.
- `a_press_long_before_the_cooldown_ends_is_dropped`. Shotgun, press with 0.49375 s left (k = 26). No shot through
  k = 70.
- `a_queued_press_is_dropped_on_weapon_switch`. Queue at k = 17, select SMG at k = 18. No shot through k = 30.
- `tests/respawn.rs`: a press queued in the tick of death does not fire after the respawn (64 `Playing` ticks).
- `semi_auto_vs_automatic` (`:478-514`): derive the offset so the remaining cooldown is `> buffer + 2·dt`. Reword the
  message.
- **New** `tests/config.rs` `fire_buffer_seconds_is_not_negative` (replace `fire_buffer_seconds: 0.15` with `-0.1`;
  the error names the field), following the `:613-620` pattern.
- Flip-RED (record in IMPL_SUMMARY): (a) test-local `fire_buffer_seconds = 0.0`, gate 1 RED; (b) `mem::take` → `false`,
  RED; (c) remove the `select_weapon` clear, switch gate RED. Restore, GREEN.
1.6 NPC check: `gang_fire_lines`, `police_fire_lines`, `gang_combat`, `gangs` stay green. Only the gang shotgun
(0.9 s vs trigger 0.5-1.1 s) can queue, and it fires ≤ 0.15 s after its line check. A new friendly or bystander hit →
stop and report. No player special case.
1.7 t13 unchanged (click gap stays), ×10 in Step 7.

### Step 2 — Q1 fire-line rule (sim, gated). Replaces PLAN step 2 entirely
2.1 **RED first (headless).** New test `crossfire_fires_from_the_post` in `tests/gang_fire_lines.rs`, using `Layout`/`run`.
`run` also returns per-member `first_shot_s` (first `ShotFired` tick / 64) and `moved_max` (max flat displacement from
the spawn point). Layouts go along the **x axis** (clear of `test_area.rs`; check every point against lines 9-34 and add
a `GATE BROKEN` if a fixture is displaced after one tick, TASK-011/012 lessons). Player at the origin. Rows, each ×3
`JITTERS`:
- `R95`: SMG member at (−11, 0, 0), dummy at (9.5, 0, 0) (probe geometry, collinear). Old rule: the pivot sidestep
  cannot clear it (4.5·9.5/11 = 3.9 m < 0.5 + 20.5·0.194 = 4.47 m), so the member must walk. New rule: along 20.5 >
  11 + 8 + 0.865 = 19.865, clear from the post.
- `R12`: the same with the dummy at (12, 0, 0).
- `R95off`: pistol member at (−11, 0, 1.2), dummy at (9.5, 0, 0) (unshadowed lateral case; also feeds the rate report).
- `R85` (residual, diagnostic): SMG member at (−11, 0, 0), dummy at (8.5, 0, 0). Guarded under both rules (19.5 <
  19.865). Assert only `MIN_SHOTS` in 30 s and zero bystander damage. Report `first_shot_s`.
Assertions for R95/R12/R95off: `first_shot_s ≤ FIRST_SHOT_S` **and** `moved_max < 0.5 m` (fires from its post), zero
bystander damage. `FIRST_SHOT_S` is **derived**: measure under the new rule (worst + 0.25 s, never above
`trigger_seconds.1 + 0.5`). Write both measurements in the doc comment. The displacement assertion is the falsifiable
half: the old rule must move the member to clear. Measure the old rule first (the test with `overshoot_margin` ≥ 60
test-locally, or before the change) and record per row: RED on displacement or on time. A row that is not RED under
the old rule is liveness only and is labelled so.
2.2 `tactics/fire_line.rs`: `FireLine { range, overreach, overshoot, cone, clearance }`. `of(…, overreach, overshoot)`
(7 params, within clippy's limit). `pub(crate) fn reach(&self) -> f32 { self.range + self.overreach }` for
`nearby_cars` (`:174`). In `blockers`: `let limit = self.range.min(flat2(to - from).length() + self.overshoot) +
self.overreach;` and use `along < limit`. Update the doc comments at `:18-20, :25, :54-55` and `tactics/mod.rs:167`
("a miss flies on to `overshoot_margin` past the target; stray hits beyond are accepted").
2.3 `Discipline` gains `overshoot_margin: f32` (`tactics/mod.rs:44-54`). It is filled in `gang/mod.rs:360-370` and
`police/mod.rs:177-187` and passed at the 4 `FireLine::of` sites.
2.4 Data + config: `GangCombatConfig.overshoot_margin` (`gangs.ron` combat: `overshoot_margin: 8.0, // m past the target
a miss is still guarded against: a spared body farther along the line does not hold fire (stray hits there are
accepted)`). `PoliceCombatConfig.overshoot_margin` (`escalation.ron` combat: `overshoot_margin: 60.0, // >= the longest
police gun range (pistol 60 m): cops keep full-reach discipline`, default per Q-A). `validate()`: finite and `> 0`.
Config sabotage fixtures in `tests/config.rs` / `tests/config_police.rs`: `0.0` and `-1.0` rejected, error names the
field.
2.5 Unit test `fire_line.rs` `overshoot_limits_the_guarded_zone` (pure; `FireLine` built directly: range 45, overreach
0.865, overshoot 8, cone 11°, clearance 0.5; from (0, 0, 0), to (0, 0, −11)). Worked rows:
(1) body (0, 0, −20.5), along 20.5 vs limit 19.865 → **not** blocked;
(2) body (0, 0, −19.5) → blocked;
(3) body (3, 0, −15), lateral 3 ≤ 0.5 + 15·0.1944 = 3.416 → blocked;
(4) to (0, 0, −40), body (0, 0, −45.5): limit = min(45, 48) + 0.865 = 45.865 → blocked (the range caps);
(5) same `to`, body (0, 0, −46.0) → not blocked;
(6) overshoot 60, to (0, 0, −11), body (0, 0, −40) → blocked (police default = old behaviour).
Rows 1/2 straddle the boundary 0.365/0.635 m away from it, not on it.
2.6 **Re-anchor the gates the rule breaks by construction, and classify the rest.** Worked geometry for each is in §0
and §3.2:
- `of_two_members_blocking_each_other_the_lower_index_moves`: move the members to (0, 0, ±7.5) (each 7.5 m past the
  other's target < 8.865, so they block each other). Add a `GATE BROKEN` precondition computed from `GangConfig`: each
  member's partner is inside `D + overshoot + overreach`. The high index must still stand. The low index moving because
  of the band (7.5 < 8) is covered by the existing `low_planned` assertion.
- `assert_keeps_firing` gains a **guarded-zone assertion** for every layout. It is the correctness gate of the rule,
  a fixed comparator read from the **shipped** config before any test-local sabotage. For each friendly or bystander
  `DamageDealt` d, take the matching `ShotFired` (same `attack`/`shot`) for its muzzle m. Violation if
  `flat|d.point − m| < flat|player − m| + overshoot_margin − (head_radius + |flat muzzle_offset|)`: the hit body was
  inside the guarded zone.
- The blanket `friendly.is_empty()` / `bystander_hits.is_empty()` stays for layouts whose spared bodies are all inside
  the zone, on the segment, or wall-shielded: `inline_file`, `wall beside the member`, the corridors, `side_by_side`
  rows 3-4 (the dummy at z = 25 is behind the test-area wall), `members_fire_past_bystanders_…` (the dummy at (0, 0, 8)
  is guarded, see §3.2; streets at z = 6/−6 are guarded; z ≥ 20 is wall-shielded), both fist scrums, human shield,
  `gang_combat::members_never_shoot_their_own_group`.
- For the **exposed** layouts (`crossfire pair`, `surround4 mixed`, `surround4 cross`, `west pair + groupmate east`,
  `west pair 1 m / 0.7 m + dummy east`), the zero assertion becomes the guarded-zone assertion plus a **rate bound**:
  friendly + bystander hits ≤ 5 % of that layout's shots, and ≤ 2× the measured value if that is lower. Print the rate.
  Deterministic sim: record the measured rates in the doc comment. If a layout I call "guarded" shows any hit, that is
  a rule bug. Stop and report, and do not relax the gate.
- Flip-RED: (a) test-local gang `overshoot_margin = 0.5` → the guarded-zone assertion RED in
  `members_fire_past_bystanders_…` (pistol member at (2, −11) sees the dummy (0, 0, 8) unshadowed at 1.43 m lateral,
  inside its 8° cone); (b) test-local `overshoot_margin = 60` (old rule) → 2.1 rows RED on displacement/time;
  (c) the unit test with `flat2(to − from).length()` replaced by `0.0` → rows 1-3 RED. Restore, GREEN.
2.7 Full sim sweep: `cargo test -p gta_sim -j 2`. Deterministic count gates (`MIN_GROUP_HITS`, `UNRULED_GANG_CAR_HITS`,
`gang_city`, `witness_city`) may move because the combat RNG stream shifts. Re-derive such a number only with a
before/after measurement written into its comment, and never below its liveness meaning. Police tests must be
**unchanged**: they are the proof that 60 m reproduces the old behaviour.
2.8 **Only if the orchestrator picks option B (8 m for police):** re-anchor `cops_hold_fire_at_the_range_edge_…`. Keep
the cops at x = ∓22.65 (gap 45.3) and move the player to x = 17.65 (5 m before the east cop). West D = 40.3, limit =
min(45, 48.3) + 0.8648 = 45.865 > 45.3, so the west cop holds. For gap 46.2: east at 23.55, player at 18.55, limit
45.865 < 46.2, so the west cop fires. Assert only the **west** cop's count (the east cop's target is 5 m away and it
fires in both cases). Check x = 17-24 against the test-area boxes at x = 10-16, z = 10: clear. Convert the five
SWAT-street `== 0` assertions to the guarded-zone assertion plus a measured rate, and add a t11 4★ runtime report of
cop-on-cop HP loss.
2.9 **Runtime t9.** `tools/qa/scenarios/t9.py` friendly-fire assertion (`:337-342`), amend surgically. Each firefight
poll records whether a **crossfire geometry** exists: a same-gang member in `Attack` farther than `D + overshoot_margin`
(read from `gangs.ron` by `ron_number`, no literal) past the player on the opposite side of an HQ member. If it never
existed, "no HQ HP lost" stays a hard assertion. If it did, the HP loss is **reported** (`friendly_hp_lost`,
`crossfire: true`), and the hard assertion becomes "no HQ member killed by it". The headless rate bound (2.6) carries
correctness. Then t9 ×20 (Step 7) with 0 failures. A failure whose blockers are cops arresting (≤ 2.5 m, pinned) is
reported with the probe data as the known human-shield residual. **No second tactics patch** (the TASK_FINAL
redesign-over-patch rule).
2.10 `docs/design/GDD.md` §6.3 "Бой": one line, "Линия огня охраняется до цели + `overshoot_margin` (банды 8 м); тело
дальше не удерживает огонь, шальное попадание туда допустимо. Полиция держит полную дальность (`escalation.ron`)."

### Step 3 — Bench scene (client domain `src/bench/`)
Same as PLAN Step 3.1-3.5, with these corrections:
- 3.1: load `RenderConfig` with `load_config(&root, RENDER_CONFIG)` **before** `App::new()`; `root` is already built
  before it (`main.rs:192`). Bench on → `resolution: WindowResolution::new(w, h).with_scale_factor_override(1.0)`.
- 3.3 `drive`: `FixedUpdate`, `.before(VehicleSystems::Drive)`, writes the player's `DriveIntent` **every fixed tick**.
  Reason, as a comment: `input::write_drive_intent` overwrites it in `Update` (note 9). The `board` system writes
  `ActionIntent.vehicle_requested` in `FixedUpdate` before `VehicleSystems::Enter`. The client only ever sets that
  latch to true, and `seat.rs:235` takes it, so there is no clobbering.
- 3.2: fix every `RenderConfig` literal (grep `RenderConfig {` in `src/`) after adding `bench_resolution`.
- Everything else is as PLAN: `BenchPhase`, `pin` (named bench cheats), `bench_target` with the rightmost connector
  (the three worked examples plus a T-junction row), `BenchFrames` (`Reflect`, registered, `recording` default false),
  unit tests in the `gta_like` bin, README "Запуск".

### Step 4 — `tools/qa/trace.py`, `tools/qa/scenarios/t16.py`
As PLAN Step 4.0-4.2 (streaming parse, `test_trace.py` in CI, session A FPS from `BenchFrames` with no vsync, session B
trace with `TRACE_CHROME` via the new `Game(env=…)`). Addition: `summary.json` records `gang_members_alive` near the
bench centre (Q3: honest count, no scene hacking). The `--bench-scene gangs` variant is **not** built. It is written
up as "not cheap: needs a different centre-selection rule".

### Step 5 — Measure and decide by trace only
As PLAN Step 5 (t16 ×3, worst-of-three table, verdict rule: frame > 16.7 ms at 1080p with no vsync, FixedMain > 4 ms
per tick, or AI > 1.5 ms per tick → fix the top offender with one §13 lever per round; otherwise "nothing to fix by
trace").

### Step 6 — §1 evidence sweep `tools/qa/scenarios/t16_s1.py`
As PLAN Step 6.0-6.2 (P1 menu, P2 camera vs wall, P5 Warn → P3 drop pickup, P7 run over, P8 junction yield, P9 death
keeps guns + full HUD, P10 settings UI via `osinput`, coverage table). Additions:
- P2: before relying on "the city-edge wall t14 finds", confirm that helper exists by grep. If not, use the nearest
  building face from `City` and say so.
- P5: the named QA mutation `Health.current = 0` must not count as a provocation. Assert gang heat unchanged after it,
  otherwise report.

### Step 7 — Repeat runner and full gates
7.1 `tools/qa/repeat.py` as PLAN.
7.2 Headless: `cargo build -j 2`; `cargo clippy --workspace --all-targets -j 2 -- -D warnings`;
`cargo clippy -p gta_sim -p citygen --all-targets -j 2 -- -D warnings`; `cargo test -p gta_sim -j 2`;
`cargo test -p citygen -j 2`; `cargo test -p gta_like --bin gta_like -j 2` (3× for any touched presentation gate);
the release benches as PLAN (`perf`, `city_startup_budget`, `civilian_bench`, `police_bench`, `traffic_bench`, record
means); `python tools/qa/tree_check.py`; `python -m unittest tools/qa/test_brp.py tools/qa/test_trace.py`.
7.3 Runtime (release, `--features dev`): every t1..t15, t16, t16_s1 ×3; t13 ×10; t9 ×20. Also t11 ×3 with the 4★ SWAT
phase, whose cop-on-cop HP loss is reported (it must be 0 under Q-A default). Record pass counts. Budget: about 2-3 h of
wall clock. Run them sequentially and never in parallel with cargo.

### Step 8 — Docs (surgical)
GDD §4.1 buffer line (PLAN Step 8). GDD §6.3 line (2.10). README "Запуск" (bench). Narrative graph and status are the
orchestrator's job.

---

## 5. Risk areas

- **R1 Rule changes gang crossfire outcomes (intended).** Stray hits past 8.865 m now happen. The guarded-zone assertion
  carries correctness, the rate bound carries "small", and t9 reports HP loss under crossfire. If the headless rate in
  the exposed layouts exceeds 5 %, stop and report to the orchestrator with numbers. Do not tune silently.
- **R2 Police.** The default (60 m) keeps police identical. That is proven by the untouched police gates. Option B risks
  are listed in 2.8.
- **R3 Residual t9 failures.** (a) An opposite member or cop stopping at 8.0-8.865 m (guarded, old unblock path, row
  R85 measures it); (b) 1★ cops walking in to arrest, pressed to the player, pins the gunman (accepted human-shield
  policy); (c) long1's unexplained friendly damage. Each is reported with probe data. No second patch.
- **R4 Buffer changes NPC timing.** Only the gang shotgun can queue. A queued shot skips a fresh line check for ≤ 0.15 s.
  The fire-line gates judge it.
- **R5 Stale latch.** Wasted/Busted/new-city clears are gated. A press from before a pause fires on resume (accepted).
- **R6 Deterministic count gates drift** when the combat RNG stream shifts (2.7). Re-derive only from before/after
  measurement.
- **R7 Fixture geometry.** The test-area wall at z = 14 shields `+z` fixtures, and the new rows use the x axis.
  Every new point is checked against `test_area.rs` with a `GATE BROKEN` guard.
- **R8 Trace size / build.** As PLAN R5 (streaming parse; a clean shutdown for `]`; the first `dev,profile` build is
  long).
- **R9 Bench car stuck / 1080p window / no gangs downtown / OS clicks.** As PLAN R6-R9.
- **R10 Scope creep.** Anything beyond the two flakes and the §1 gaps goes to IMPL_SUMMARY as a finding (TASK-031/032).

## 6. Open questions (для оркестратора)

**Q-A. Полиция и `overshoot_margin`.** Правило общее (`tactics/`), значение по ролям в данных.
- A (по умолчанию): банды 8 м, полиция 60 м (не меньше самой дальней полицейской пушки, то есть старая дисциплина).
  Полиция не меняется ни на бит, все полицейские гейты остаются как есть. Флак t9 чинится, потому что это линии банды.
- B: 8 м и для полиции. `cops_hold_fire_at_the_range_edge_across_the_player` краснеет по построению (переякорение в
  2.8). Пять гейтов "SWAT в улице, 0 попаданий по своим" скорее всего краснеют: при 4-5★ окружении спецназ на краю
  полосы 9-12 м начнёт попадать друг в друга. Это видимое изменение геймплея в последнем слайсе.

**Q-B. Порог "мало" для шальных попаданий в headless-раскладках с перекрёстным огнём.** По умолчанию: не больше 5 %
выстрелов раскладки и не больше двух замеренных значений. Альтернатива: только отчёт без порога. Тогда корректность
держит только гейт охраняемой зоны.

children: 0 launched / 0 reported.
