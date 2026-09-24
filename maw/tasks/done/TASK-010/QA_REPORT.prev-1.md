# QA_REPORT — TASK-010 (GDD T9, gangs)

Branch `feature/t09-gangs`, HEAD `75be889`. Cost of error: mixed. Silent rules have headless gates. Feel is checked
by the owner run.

## 0. Preflight and disconfirmation

- Read `scratch/` first, as a coverage map only: implementer flips, `qa_t8*`/`qa_t9*` runs, fixer FPS probe and trace.
  Then read TASK_FINAL, PLAN_FINAL, IMPL_SUMMARY, IMPL_REVIEW, FIX_SUMMARY, OPEN_DECISIONS and log.jsonl.
- **Counter-example tested first:** "in a 3-member firefight a member's bullet hits a groupmate
  (`DamageDealt { shooter: Gang(0) member, target: Gang(0) member }`)". `fire_weapons`
  (`combat/hitscan.rs:153-201`) casts against World|Character|Hitbox and skips only the shooter's own colliders.
  The gang aim ray starts at the eyes and goes to the target chest (`gang/behavior.rs:403-407, 433`), so a groupmate
  standing in that line is the first thing hit. **The counter-example held**: friendly fire is real. See Bug 1.
- Dead ends checked against the code. The matrix-gate flip replacement is right: `provoke_gangs` filters on
  `cfg.hostile`, and my own flip goes RED. The held-gun re-instance gate is in `src/visuals/gang_gate.rs`, and 3/3
  runs were green. The 30 FPS "dead end" is correct: the only monitor is `\\.\DISPLAY9` at 30 Hz under Fifo, and the
  no-vsync cost is ~2.6 ms (below).

## 1. Environment

No docker or dev server. I used cargo directly plus the windowed release build driven over BRP.

```
cargo build -j 4
cargo clippy -j 4 -- -D warnings
cargo clippy -p gta_sim --tests -j 4 -- -D warnings
cargo clippy -p gta_like --bin gta_like --tests -j 4 -- -D warnings
touch crates/*/src/lib.rs && cargo test -p gta_sim -p citygen -j 4        # scratch/qa_test_sim.txt
cargo test -p gta_like --bin gta_like -j 4                                # x3
python tools/qa/tree_check.py ; cargo tree -p gta_sim -e normal -i bevy_render
python tools/qa/scenarios/t9.py --out maw/tasks/in_progress/TASK-010/scratch/qa/t9
python tools/qa/scenarios/t8.py --out maw/tasks/in_progress/TASK-010/scratch/qa/t8
python maw/tasks/in_progress/TASK-010/scratch/qa/ff_runtime.py <out> 30 [inline]   # FF + lethality probe
# headless FF probe: copy scratch/qa/qa_friendly_fire.rs to crates/gta_sim/tests/, then
cargo test -p gta_sim --test qa_friendly_fire -- --nocapture                        # (file removed afterwards)
```

Services: none. Every game process ended with `brp_extras/shutdown`, and `tasklist` showed no `gta_like` left.

## 2. Test results

| Suite | Result |
|---|---|
| `cargo build` | ok |
| clippy (workspace, gta_sim tests, gta_like bin tests) `-D warnings` | clean, all three |
| `cargo test -p gta_sim -p citygen` (after `touch`) | all green: 26 result lines, 0 failed (gangs 7, gang_combat 7, gang_city 4, config 38, lib 41, every existing suite green). No failures on HEAD, so no failure list to compare with base. |
| `cargo test -p gta_like --bin gta_like` ×3 | 41/41 ×3 (gang_gate: 3 tests, including the forced re-instance held-gun gate) |
| `tree_check.py`; `cargo tree … -i bevy_render` | passed; nothing to print |
| `t9.py` (release, seed 1) | **pass**: HQ group of 3; Idle ×3 at 12 m inside territory 0; magazine 12→11; all 3 `Attack` 0.3 s later; `GangHeat` [119.34, 0]; firefight captured at 0.53 s with the player alive and 3 attacking; 8/8/7 shots in 6 s (Pistol, SMG, SMG); armour lost 165 in 6 s; no log errors |
| `t8.py` regression | pass: cap 40 in 1.8 s, scared share 0→0.909, no errors |
| Frame (t9 firefight, `Game.frame_report()`) | monitor `\\.\DISPLAY9` 30 Hz, Fifo 30 FPS = the refresh rate. **No-vsync cost 2.60–2.62 ms (~387 FPS)** with the fight running. t8 crowd: 2.37–2.67 ms. GDD §11 ≥ 60 FPS is met. |

Screenshots I looked at: `scratch/qa/t9/hq.png` shows the purple group down the sidewalk in front of the player,
~12 m away. `scratch/qa/t9/firefight.png` shows the same group while the fight runs, player alive, blue armour bar
full, 11/12. `scratch/qa/ff2/ff_0.png` and `ff_1.png` show the inline pose with members in view. The group is small
on screen at 12 m and partly hidden by the player model. Readability is for the owner to judge.

### New tests and probes (mine)

1. **Flip-RED done by me** (restore checked by sha256):
   - `decay_gang_heat` decays `0.5·dt` → `gang_heat_decays_in_120s` and `gang_heat_keeps_decaying_while_wasted`
     go RED. Restored `behavior.rs` sha256 `6cf11dc3…a7d8d0` matches the original.
   - `GangConfig::hostile` ignores the `hostile` flag → `disabled_matrix_gangs_do_not_attack_each_other` goes RED.
     Restored `gang/mod.rs` sha256 `65f7e83f…643986` matches.
   - `provoke_gangs` radius `group_radius·1.1` → `attack_on_a_member_aggroes_the_group_within_30m` goes RED at
     `gangs.rs:221` (C at 31 m is aggroed). Restored, sha256 matches.
2. **Headless friendly-fire probe** `scratch/qa/qa_friendly_fire.rs`. Production composition (`gang_floor` +
   `gang_member_bundle`). Player armour 1e6, all members provoked, 1920 ticks = 30 s, `DamageDealt` logged every
   tick. The output is deterministic (run twice, identical): `scratch/qa/friendly_fire_probe.txt`.

| Layout (player at origin) | member shots | hits on player | **FF hits** | FF dmg | **FF kills (time)** | first FF | end states |
|---|---|---|---|---|---|---|---|
| ring of 3 SMG, post 12 m (spawner geometry) | 72 | 24 | 5 | 112 | 1 (4.00 s) | tick 3 (first volley) | Attack, Attack, Dead |
| ring 3: Pistol, SMG, SMG @12 m | 60 | 23 | 7 | 158 | 1 (4.00 s) | tick 3 | one survivor at 53 HP |
| ring 4: Shotgun, Pistol ×2, Shotgun @12 m | 62 | 39 | 14 | 314 | 2 (1.72 s, 21.5 s) | tick 3 | 2 Dead, one at 30 HP |
| ring 3 SMG @ ~20 m | 100 | 21 | 2 | 51 | 0 | tick 3 | one at 49 HP |
| file of 3 SMG in depth | 72 | 24 | 5 | 110 | 1 (2.30 s) | tick 3 | front one Dead |
| spread 3 (cadence fixture, 7 m apart) | 69 | 53 | 6 | 73 | 0 | tick 1537 | Shotgun member 27 HP → **Retreat** |

   In every layout: `non_player_target_ticks = 0` (no member ever targets anything but the player), and
   `heat_end = 90.000` (120 − 30 s of pure decay, never reset or raised).
3. **Runtime FF + lethality probe** `scratch/qa/ff_runtime.py` (t9 setup, then 30 s where the player never fires
   again, then armour 0 / health 100 until Wasted):
   - `ff1` (t9 pose, 12 m on the approach line): no FF in 30 s. All members stayed at 100 HP and in `Attack`. Heat
     went 119.28 → 89.73 (pure decay). The line of fire passes between the two front members, ~0.87 m from each.
   - `ff2` (`inline` pose: player on the line far member → near member): the near member went 100 → 74 (0.7 s) →
     52 → 26 (**Retreat**, 2.2 s) → 3 → **Dead at 3.9 s**, shot by its own group. The other two stayed
     `Attack { target: player }`. Heat was pure decay (119.28 → 115.61 over 3.66 s). Its gun dropped (rounds 0).
   - Lethality, no armour, 3 SMG members at 12 m (`ff1` phase B): first hit at 2.7 s after the reset, then
     100 → 88 → 77 → 65 → 54 → 41 → 28 → 16 → 0, **Wasted 7.5 s after the reset, ~4.8 s after the first hit**
     (~12 per SMG hit). With 2 members left (`ff2`), Wasted at 10.3 s. The implementer saw pistol+SMG take
     100 → 40 in ~0.5 s. My t9 run took 165 armour in 6 s. Balance goes to the owner.

## 3. Acceptance criteria

| Criterion | Test performed | Result |
|---|---|---|
| Headless: outside the territory members are neutral | `outside_the_territory_members_stay_neutral` (Idle on every tick for 204 ticks while aimed at from 7 m outside, then a shot next to them → still Idle; positive control Warn at tick 192, Attack on a shot inside). Green in my run. Implementer flips checked in `flip_red_log.txt`. | PASS |
| Headless: attack on a member → the whole group within 30 m is in `Attack` | `attack_on_a_member_aggroes_the_group_within_30m` (exact hit tick; M2 at 29 m in, C at 31 m out, rival gang out). **My flip (radius ×1.1) → RED.** | PASS |
| Headless: `GangHeat` decays in 120 s | `gang_heat_decays_in_120s` (exact values at T+1, +3840, +7679, +7680 and attack-on-sight memory), plus `…_while_wasted`. **My flip (0.5·dt) → RED.** | PASS |
| Headless: with the matrix off, gangs do not attack each other | `disabled_matrix_gangs_do_not_attack_each_other` (punch and shot, each with a fresh positive control). **My flip (ignore `hostile`) → RED.** Caveat: stray bullets still hurt members of the SAME gang (Bug 1). That is not a matrix leak: same faction is never hostile by law, and no member ever targets a groupmate. | PASS |
| Runtime: `t9.py` exists and passes via `brp.py` (teleport to HQ, shot nearby, `Attack` read, screenshots) | Ran it myself, exit 0, output above. | PASS |
| Owner checklist recorded | §6 below | PASS (recorded; the owner still has to run it) |
| New tuning values in data files, no tuning `const` | Grepped the diff: the only new `const` is the path `GANG_CONFIG`. The numbers live in `gangs.ron`, `population.ron`, `weapons.ron`, `navigation.ron`, `visual.ron`, each with a strict loader and 7 sabotage fixtures green. | PASS |
| `cargo build`, clippy `-D warnings`, `cargo test -p gta_sim` (+citygen) green | §2 | PASS |
| Existing tests pass | §2 (every existing gta_sim/citygen/gta_like suite green; t8 green) | PASS |

## 4. Bugs found

### Bug 1 — MAJOR (owner-visible, systemic): gang members shoot and kill their own groupmates

The orchestrator asked me to confirm or rule this out. **Confirmed** in headless and runtime.

- **Cause:** `fire_weapons` (`crates/gta_sim/src/combat/hitscan.rs:193-201, 229-231`) hits the first
  Character/Hitbox on the ray and excludes only the shooter. Gang aim (`gang/behavior.rs:403-435`) goes eyes →
  target chest, and LOS (`sight_blocked`) checks only the World layer. A member therefore fires straight through a
  groupmate who stands in the line. Groups spawn in a 1 m ring around the post (`population/gangs.rs:111-116`), so
  for most player bearings one member stands between another and the player.
- **Repro (headless):** copy `scratch/qa/qa_friendly_fire.rs` to `crates/gta_sim/tests/`, then
  `cargo test -p gta_sim --test qa_friendly_fire -- --nocapture`. A ring of 3 SMG at 12 m hits a groupmate on the
  **first volley (tick 3)** and kills one at **4.0 s**. 4 of 6 layouts lose a member within 30 s. FF is 5–14 of
  60–100 shots at 12 m (7–23 %).
- **Repro (runtime):** `python scratch/qa/ff_runtime.py <out> 30 inline`. A member is killed by its own group
  3.9 s into the fight. On the t9 bearing it did not happen in 30 s: it depends on geometry.
- **Expected:** a group fighting the player does not wear itself down. Nothing in the GDD asks for friendly fire,
  and GDD §1 "банда мстит за своих" reads badly when the bandits are the ones killing their own.
  **Actual:** stray fire hurts and kills groupmates.
- **What FF does and does not do** (as asked):
  - It does **not** provoke. `hostile(Gang(g), Gang(g))` is false by law (`gang/mod.rs:311-317`), so
    `provoke_gangs` drops the hit.
  - It does **not** change heat. Only a `Faction::Player` attacker sets heat. Measured: pure decay in every run.
  - It does **not** make members fight each other. The target is the player on every tick (0 non-player-target
    ticks in 6 × 1920 ticks).
  - It **does** push a wounded member into `Retreat` (below 30 % HP). Retreat walks straight away from the player,
    which on an inline bearing is further down the same line of fire. That is how the `ff2` victim died while
    retreating.
  - It **does** kill: the victim becomes a corpse and drops its gun. That lowers the group's lethality, and the
    owner sees bandits dropping without being shot.
  - Hit reactions (stagger/knockback) on the victim also interrupt its own fire (`reaction.is_active()`).
- **Not fixed here.** The orchestrator decides the fix. Cheap options, for context only: (a) a member holds fire
  while a live same-faction body is within the capsule radius of its aim segment (one shape/ray cast on the fire
  tick); (b) the NPC hitscan predicate skips same-`Faction` bodies (GTA-like "no friendly fire" for AI). A gate for
  either: my probe with "FF hits == 0" as the assertion.

### Minor / notes (no fix required for this slice)

- `attach_held_gun` and `show_held_gun` are unordered. After a model re-instance the held gun is hidden for one
  frame (the fixer found this). It is presentation only. Owner item: the gun should not flicker.
- `t9.py` hard-passes only on the aggro snapshot. It reports member health but does not flag a member hurt by
  friendly fire. If Bug 1 is fixed, adding "every member health == 100 at the end" to t9 is a free runtime gate
  (the player never fires at them).
- The HQ group is small and far on screen at 12 m and partly hidden by the player model (`hq.png`). Tint
  readability is for the owner.

## 5. Verdict

**SHIP-PENDING-RUNTIME**, with Bug 1 raised to the orchestrator as a Major finding to decide before the owner run.

All acceptance criteria pass: headless through the production composition (3 flips by me went RED and restored to
matching sha256) and runtime (`t9.py` and `t8.py` green, frame cost ~2.6 ms). Build and clippy are clean, and every
existing test is green. Bug 1 does not break any stated criterion: the matrix gate is about inter-faction hostility,
and friendly fire causes no provocation, no heat change and no infighting. It is still a real, systemic defect that
the owner will very likely see during "спровоцировал, получил перестрелку" (a groupmate down in ~2–4 s on common
bearings). My recommendation is to fix it before the owner run. The fix is small and gateable with the probe above.
If the orchestrator treats it as in scope for T9, the verdict becomes NEEDS_FIXES.

## 6. Owner checklist (runtime, owner)

Launch: `cargo run --release -- --seed 1`. Take the pistol at the range, go to the gang-0 HQ around
(116, 0.15, −486) (gang 1 HQ around (−491, −308)).

- [ ] Зашёл к бандитам, спровоцировал, получил перестрелку.
- [ ] Обе банды читаются по тинту (фиолетовые / красные) и отличаются от мирных, в том числе с 12 м.
- [ ] Предупреждение читается как угроза: постоял рядом 3 с или прицелился, бандит достаёт ствол, целится,
      подходит. Отошёл дальше 12 м, он убирает ствол.
- [ ] Группы не появляются в кадре.
- [ ] **Летальность:** три бандита на 12 м. Без брони в прогонах QA игрок терял 100 → 40 HP примерно за 0.5 с
      (пистолет + SMG, прогон имплементера) и умирал за ~5 с после первого попадания (3 SMG). Решить, не слишком
      ли быстро. Ручки: `combat.trigger_seconds`, `combat.aim_error_deg` в `assets/gang/gangs.ron`.
- [ ] **Friendly fire:** бандиты попадают в своих и убивают их. В QA один из группы погибал от своих за 2–4 с,
      если игрок стоит так, что бандиты выстраиваются в линию. Посмотреть, падают ли бандиты без твоих попаданий
      и как это выглядит (решение по фиксу за оркестратором).
- [ ] В перестрелке держат 8–15 м, мажут чаще игрока, бьют в упор, раненые (< 30 %) отходят.
- [ ] Погоня обходит угол дома и не трётся о стены.
- [ ] Убитый бандит роняет ствол, его можно поднять, через 60 с ствол исчезает.
- [ ] Ствол в руке бандита не мигает и не пропадает.
- [ ] FPS рядом с перестрелкой на своём мониторе (здесь 30 Hz Fifo даёт 30 FPS, без vsync кадр ~2.6 мс).

Note: `GangHeat` counts fixed-time seconds, so during the Wasted slow motion it runs slower in wall-clock time.

## 7. Files

- Probes and evidence: `scratch/qa/qa_friendly_fire.rs`, `scratch/qa/friendly_fire_probe.txt`,
  `scratch/qa/ff_runtime.py`, `scratch/qa/ff1/`, `scratch/qa/ff2/` (`ff_runtime.json`, screenshots),
  `scratch/qa/t9/`, `scratch/qa/t8/`, `scratch/qa_test_sim.txt`, `scratch/qa/*_run.txt`.
- `log.jsonl`: one `decision` entry (how FF was measured).
- `git status --short`: clean outside the task dir. The temporary test file was moved out of
  `crates/gta_sim/tests/`, and every flipped source was restored (sha256 verified).

children: 0 launched / 0 reported.
