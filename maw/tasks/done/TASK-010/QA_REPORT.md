# QA_REPORT — TASK-010 (GDD T9, gangs), round 3 (final)

Branch `feature/t09-gangs`, HEAD `ae867c8` (fixer round 4). Earlier rounds: `QA_REPORT.prev-1.md`, `QA_REPORT.prev-2.md`.
Cost of error: mixed. Silent class (friendly fire, starvation, tie-break) → headless probes of my own. Owner class
(lethality, how "holding behind a scrum" reads) → owner checklist.

## 0. Preflight and disconfirmation

- `scratch/` listed and read as a coverage map only: fixer rounds 3–4 (`fixer3/`, `fixer4/` flips, t8/t9 runs), QA2's
  probe. I re-ran none of them as evidence. My material is new: `scratch/qa3/qa3_probe.rs` (headless, 30+ layouts the
  fixers did not use) and `scratch/qa3/qa3_runtime.py` (BRP, a moving player at the HQ).
- Read: TASK_FINAL, PLAN_FINAL, IMPL_SUMMARY, IMPL_REVIEW, FIX_SUMMARY (round 4) + FIX_SUMMARY.prev-3, OPEN_DECISIONS,
  log.jsonl, QA_REPORT.prev-2. Code read in full: `gang/behavior.rs`, `gang/behavior/fire_line.rs`,
  `combat/melee.rs::apply_strikes`, `combat/hitscan.rs::fire_weapons`, `tests/gang_fire_lines.rs`, `tests/common/mod.rs`.
- Dead ends triaged against the code: round-4 "literal engaged-anywhere rule reds 5 gates" — consistent with
  `pinned` in `fire_line.rs:148-154` using `yielding` = within `melee_distance.1` of the target
  (`behavior.rs:461-465`). Round-3 "re-pick every slot flips sides" — the keep-while-usable rule is at
  `fire_line.rs:170-174`.
- **Counter-example I set out to find:** "a member with its line blocked by a groupmate that is NOT pressed against the
  target, and no usable candidate spot, is starved for the whole fight, because the close-in fallback is a direct seek
  that only avoids walls (`avoid_offset` casts World-mask rays) and so walks into the groupmate standing on the path."
  Code path: `unblock` → `clear_spot` = None (every side spot behind a wall) → `pinned` false (the blocker is 10 m from
  the target) → `Motion::Seek(target, direct)`. **It held**: in a 2.4 m or 3.0 m wide corridor with an SMG in front at
  10 m and a pistol behind it at 14 m, the pistol fires **0 shots in 30 s** (Bug 1). A 4.0 m corridor lets it pass
  and it fires 24. This is outside the pinned-scrum case the orchestrator excluded.

## 1. Environment

No docker, no dev server. Cargo directly; windowed release build over BRP (`\\.\DISPLAY9`, 30 Hz, Fifo).

```
touch crates/*/src/lib.rs && cargo build -j 4
cargo clippy -j 4 -- -D warnings
cargo clippy -p gta_sim --tests -j 4 -- -D warnings
cargo clippy -p gta_like --bin gta_like --tests -j 4 -- -D warnings
cargo test -p gta_sim -p citygen -j 4                      # scratch/qa3/test_sim.txt
cargo test -p gta_like --bin gta_like -j 4                 # x3
python tools/qa/tree_check.py ; cargo tree -p gta_sim -e normal -i bevy_render
# headless probe: copy scratch/qa3/qa3_probe.rs to crates/gta_sim/tests/qa3_probe.rs, then
cargo test -p gta_sim --test qa3_probe -j 4 -- --nocapture  # scratch/qa3/probe_output.txt; file removed afterwards
cargo build --release --features dev -j 4
python tools/qa/scenarios/t9.py --out maw/tasks/in_progress/TASK-010/scratch/qa3/t9
python tools/qa/scenarios/t8.py --out maw/tasks/in_progress/TASK-010/scratch/qa3/t8
python maw/tasks/in_progress/TASK-010/scratch/qa3/qa3_runtime.py maw/tasks/in_progress/TASK-010/scratch/qa3/rt_move 24
```

Services started: none. Every game run ended with `brp_extras/shutdown`; `tasklist` shows no `gta_like` afterwards.
The probe file was deleted from `crates/gta_sim/tests/`; `git status --short` is clean outside the task dir.

## 2. Test results

| Suite | Result |
|---|---|
| `cargo build`, clippy ×3 `-D warnings` | ok / clean |
| `cargo test -p gta_sim -p citygen` (after `touch`) | 27 result lines, **228 passed, 0 failed, 3 ignored** (the 3 pre-existing ignores). No failure list to diff against base. `gang_fire_lines` 9/9, `gang_combat` 9, `gangs` 7, `config` 40. |
| `cargo test -p gta_like --bin gta_like` ×3 | 41/41 every run |
| `tree_check.py`; `cargo tree … -i bevy_render` | passed; nothing to print |
| New tuning `const` since QA2 (`git diff 092be45 HEAD -- '*.rs' \| grep '+.*const'`) | only test constants (`FIGHT_TICKS`, `MIN_SHOTS`, `MIN_GROUP_HITS`, `BAND_TOLERANCE`, fixtures). The yielding radius reuses `combat.melee_distance.1`. |
| File sizes | `behavior.rs` 665, `fire_line.rs` 191, `gang_fire_lines.rs` 585, `melee.rs` 722 (all < 750) |
| `t9.py` | **exit 0.** HQ group of 3 SMG; `Idle` ×3 → `Attack` ×3; heat 119.34; 7/7/7 shots in the window; every member 100 HP and `Attack`; `Playing`; 0 log errors. `frame_report`: monitor `\\.\DISPLAY9` 30 Hz, `Fifo`, 30.1 FPS as shipped; **no-vsync frame cost 2.37–2.39 ms**. |
| `t8.py` | exit 0: scared share 0 → 1.0 (Flee 10, Cower 1), `Playing`, 0 log errors, no-vsync 2.29–2.89 ms |

### Flip-RED (mine, both different from the fixers' G1–G6 / F1–F10)

| Perturbation (`behavior.rs`) | Gate | Result |
|---|---|---|
| yielding radius `<= c.melee_distance.1` → `<= 0.5 * c.melee_distance.0` (0.75 m) | `gunman_holds_its_band_behind_a_fist_scrum` | **RED**: "flank: the gunman came within 1.46 m of the player (band edge 8 m)" (`scratch/qa3/flip1_red.txt`) |
| line clearance `radius + fire_line_margin` → `fire_line_margin` (body radius dropped) | `gang_combat::members_never_shoot_their_own_group` | **RED** (`scratch/qa3/flip2_red.txt`) |

Both restored with `git checkout`; sha256 `21ec3412…0f36261` equal to the original each time; suites GREEN again.

### Headless probe (mine; production `gang_floor` + `gang_member_bundle`, 30 s, armour 1e6 unless stated)

Full output: `scratch/qa3/probe_output.txt` (the corridor and tie-break sections were run as separate invocations and
are summarised here).

**Friendly and bystander damage, liveness outside the scrum case**

| Layout | shots / member | FF | bystander | note |
|---|---|---|---|---|
| L1 gang-1 shotgun ring 6 m ×3 + pistol 10 m, player stands | 12/12/12/19 | 0 | 0 | shotguns run dry (12) → fists; punches 42–62 per member, all spared on groupmates |
| L2 same, player **runs** a circle | 12/12/12/21 | 0 | 0 | moving spread covered |
| L3 **both gangs** provoked (SMG, Pistol, Shotgun, Pistol) + dummies 4–5 m, player runs | 16/15/12/18 | 0 | 0 | gang 0 and gang 1 spare each other |
| L4 tight 2×2 cluster 0.7 m at 10 m + dummy behind the player | 30/24/30/24 | 0 | 0 | |
| L5 L-shape (W, N, NW, NE), player walks | 29/24/30/24 | 0 | 0 | |
| L6 exact column 9/12/15 m | 26/24/25 | 0 | 0 | |
| L9 dummy 3.0 m beside the player (not yielding) | 35 | 0 | 0 | |
| L12 civilians walking 1.5 m past the player, player fires into the air every 2 s | 30/24 | 0 | 0 | civilians within 2.5 m for 0.5 s only (they scatter) |
| L11 1 brawler + 3 gunmen, player walks | 0 (fists)/8/11/8 | 0 | 0 | brawler 22 punches; gunmen gaps 7.5–19 s (pinned while the brawler is on the player) |
| **L7 / V1 corridor 2.4 m / 3.0 m, SMG 10 m in front, pistol 14 m behind** | 35 / **0** | 0 | 0 | **Bug 1** |
| V6 same, pistol 18 m behind | 35 / **0** | 0 | 0 | Bug 1 |
| V3 corridor 2.4 m, pistol in front, SMG behind | 24 / **0** | 0 | 0 | SMG 0 for the first 10 s (Bug 1), then pinned behind the dry pistol brawling |
| V2 corridor 4.0 m | 29/24 | 0 | 0 | rear member squeezes past |
| V4 corridor 2.4 m, player walks a circle | 30/24 | 0 | 0 | the moving target opens the line |
| V5 one wall + groupmate beside the rear member | 29/24/28 | 0 | 0 | the groupmate steps aside, then the rear one moves |

Across all 30+ probe layouts: **0 friendly damage (guns and fists), 0 bystander damage.**

**Pinned-scrum hold and "fires once the line clears"** (per-shot tick log)

| Case | gunman shots before the event | first shot after the line clears | band |
|---|---|---|---|
| C1 dummy 0.8 m in front of the player, SMG 12 m; dummy removed at 10 s | 0 in 10 s | **1 tick (0.016 s)**; 26 shots by 30 s | closest 12.0 m |
| C1b dummy 1.5 m at the flank, pistol 11.7 m; removed at 10 s | 0 | **1 tick**; 24 shots | closest 11.66 m |
| C2 player steps 6 m away from the shield at 10 s | 0 | **1 tick**; 26 shots | closest 12.0 m |
| C3 flank fist scrum; player runs +X for 3 s at 10 s | 0 all fight | never: the brawlers run with the player, the line never opens | closest 9.26 m; group 34 hits |
| C4 inline scrum, gunman off-axis; player runs 2 s | 0 | 2 shots in the 3 s after the dash | closest 9.49 m; group 31 hits |

The gunman holds its band (≥ 9.26 m in every scrum/shield case) and fires on the very next tick once the line is clear.

**Tie-break** — an independent re-statement of the line test (flat, 0.5 m + along·tan(aim_error + base + max_bloom),
weapon range) evaluated per tick; counted ticks where both lines block each other:

| Layout | mutual ticks | higher index moved | lower index moved | shots (low / high) | FF |
|---|---|---|---|---|---|
| T1 pistol 8 m NE / SMG 14 m SW | 81 | **0** | 81 | 24 / 22 | 0 |
| T1r same, spawn order swapped | 187 | **0** | 187 | 25 / 24 | 0 |
| T2 SMG 12 m E / SMG 9 m W | 193 | **0** | 193 | 30 / 32 | 0 |
| T3 pistols 10 m N / S | 120 | **0** | 120 | – | – |

In T1r the higher index moved in 74 ticks while the lower had a plan, all of them outside mutual block (it was then
clearing its own line). Rule holds.

**Lethality (no armour)**: gang-0 SMG+Pistol+SMG at 12 m → Wasted 5.95 s (standing) / 6.58 s (running a circle);
gang-1 Pistol+2 Shotguns at 9–12 m → **3.0 s** standing / **1.53 s** running a circle (running into shotgun range).

### Runtime probe (mine, `scratch/qa3/rt_move/result.json`, screenshots `before.png`, `moving_0..2.png`, `end_a.png`)

Seed-1 gang-0 HQ, group of 3 SMG, one sky shot, armour 1e6, then the player keeps moving for 24 s (D 6 s, A 6 s, W 3 s,
D 6 s, S 3 s, one hold per key; moved 23 m net):
- Every member spent **30 rounds**, longest no-fire gap **0.69–0.72 s** (reloads), all at **100 HP** (0 friendly
  fire), all still `Attack`. The player took 376 damage (armour) in 24 s. The members kept 5.3–15.5 m.
- Phase B (armour 0, strafing): first hit at 4.4 s, **Wasted at 10.95 s**.
- `moving_1.png`: the three purple members stand shoulder to shoulder on the sidewalk, guns up, aiming at the running
  player. `t9/firefight.png`: the player mid-hit-reaction on the sidewalk, the group 12 m ahead with guns drawn. 0 log errors.

## 3. Acceptance criteria

| Criterion | Test performed | Result |
|---|---|---|
| Headless: outside the territory members are neutral | `outside_the_territory_members_stay_neutral` green (logic untouched since round 1; round-1 flips apply) | PASS |
| Headless: attack on a member → group within 30 m in `Attack` | `attack_on_a_member_aggroes_the_group_within_30m` green | PASS |
| Headless: `GangHeat` decays in 120 s | `gang_heat_decays_in_120s`, `…_while_wasted` green | PASS |
| Headless: matrix off → gangs do not attack each other | `disabled_matrix_gangs_do_not_attack_each_other` green; in L3 both gangs fought the player side by side with 0 damage to each other | PASS |
| Runtime: `t9.py` exists and passes via `brp.py` | ran, exit 0 (§2) | PASS |
| Owner checklist recorded | §6 | PASS (recorded) |
| New tuning values in data files | no new tuning `const`; yielding radius reuses `melee_distance.1` | PASS |
| build / clippy / `cargo test -p gta_sim` (+citygen) green | §2 | PASS |
| Existing tests pass | 228/0/3, client 41 ×3, t8 exit 0 | PASS |
| Orchestrator: 0 friendly or bystander damage, guns and fists | 0 in every headless layout (30+) and at runtime (t9 + moving run) | PASS |
| Orchestrator: no member starved outside the pinned-scrum case | open ground, clusters, columns, both gangs, walking civilians: every gun member ≥ 8 shots / 30 s. **Corridor ≤ 3 m: the rear member fires 0** | **FAIL (Minor)**: Bug 1 |
| Orchestrator: gunman holds its band behind a scrum and fires once the line clears | band ≥ 9.26 m in every scrum/shield case; first shot 1 tick after the clear (C1, C1b, C2); my flip of the yielding radius turns the gate RED | PASS |
| Orchestrator: tie-break | independent mutual-block count: higher index moved in 0 mutual ticks over 4 layouts incl. swapped indices | PASS |
| Orchestrator: runtime at the HQ with a moving player | t9 + my 24 s moving run: 30 rounds per member, 0 FF, gaps < 0.75 s | PASS |
| Frame cost | `frame_report`: 30 Hz Fifo monitor; no-vsync 2.37–2.39 ms (t9), 2.29–2.89 ms (t8) | PASS |

## 4. Bugs found

### Bug 1 — Minor (liveness, deterministic): the rear member of a file in a narrow passage never fires

- **Where:** `fire_line.rs::unblock` → close-in fallback `Motion::Seek { dest: target, direct }` →
  `behavior.rs::head_for`, whose only obstacle avoidance is `avoid_offset` (World-mask rays).
- **Repro:** `scratch/qa3/qa3_probe.rs`, `qa3_corridor_variants` / "L7 corridor". Two walls 14 m long at x = ±1.35
  (clear width 2.4 m) or ±1.65 (3.0 m), from z = −20 to −6; SMG member at (0, 0, −10), pistol member at (0, 0, −14),
  player standing at the origin; 30 s.
- **Expected:** every gun member ≥ 6 shots in 30 s when nothing is pressed against the target (orchestrator: "no member
  starved of shots outside the pinned-scrum case").
- **Actual:** the pistol fires **0 shots in 30 s** (longest gap 30 s) and stands pressed behind the SMG (closest 8.33 m).
  Every side spot is behind a wall, the back spot is still in line, the forward spot bumps the SMG, and the SMG is 10 m
  from the target, so `pinned` is false and the pistol direct-seeks into the SMG, which holds its band and never gives way.
  The contact is symmetric, so the capsules do not slide. At 4.0 m width it squeezes past and fires 24.
- **Scope:** it needs confinement on both sides within ~1.8 m of the member's line (two walls, or a wall and a static
  obstacle). The group still pressures the player (the front SMG fires 35 shots). A moving player breaks it (V4: 30/24).
  I did not find such a passage around the seed-1 HQ in the runtime run. So it is Minor, not a SHIP blocker for this
  slice. T11 police reuse the same fallback, so I filed a PCTX proposal and recommend a follow-up task. Possible fix
  directions, to recompute before applying: let a member holding its band step aside when a groupmate behind it is
  blocked by it, or treat a non-yielding groupmate on the close-in path as a body for the avoidance.

### Notes (not defects)

- **Human shield by design:** any non-hostile body within 2.5 m of the player pins SMG and shotgun members, and pistols
  too closer than ~2 m. Measured: a dummy 0.8 m in front → 0 gang shots until it is removed. A cowering civilian next to
  the player does the same. Walking civilians scatter within about 0.5 s (L12), so in the city it is transient. This is
  the accepted rule (OPEN_DECISIONS round 3); it goes to the owner.
- **Scrum while fleeing (C3):** if the player runs with two brawlers on him, they keep pace and the gunman's line never
  opens, so the gunman fires 0 and the pressure comes from punches (34 hits in 30 s). This is consistent with rule (a).
- **Clumping:** at runtime the three HQ members moved as one tight bunch and read the same distance to the player
  to 0.1 m (they all stop at the 15 m band edge). No FF resulted. Owner feel.
- **Lethality spread:** gang-1 shotguns kill in 1.5–3 s at 9–12 m, gang-0 SMGs in 6–11 s. An owner decision
  (`combat.trigger_seconds`, `combat.aim_error_deg`, weapon damage).
- QA2's muzzle-offset margin note (0.25 m lateral > 0.2 m margin): with the muzzle 0.45 m forward, the near-field gap
  closes for cones > 6.3°, and every gang cone is ≥ 8°. No hit was observed in any layout. Closed as a concern.

## 5. Verdict

**SHIP-PENDING-RUNTIME.** Every stated acceptance criterion passes headless and at runtime:
- the build and clippy are clean;
- 228/0 sim, 41 ×3 client, t9 and t8 green;
- frame cost is ~2.4 ms;
- both of my flips went RED and were restored by sha256.

The round-3/4 behaviour holds on layouts the fixers did not see:
- 0 friendly and bystander damage from guns and fists;
- the gunman holds its band behind a scrum and fires one tick after the line clears;
- the tie-break is exact.

One Minor liveness defect remains (Bug 1: a file of two in a passage ≤ 3 m wide starves the rear member). It needs a
two-sided confinement, it does not reduce group pressure, and it does not block SHIP of T9. Put it in a follow-up task
before T11 reuses the rule.

**Nothing blocks SHIP at the MAW level.** The remaining items are owner feel (§6).

## 6. Owner checklist (runtime, owner)

Запуск: `cargo run --release -- --seed 1`. Взять пистолет на стрельбище и пойти к штабу банды 0, это примерно
(116, 0.15, −486). Штаб банды 1 примерно в (−491, −308), там дробовики.

- [ ] Зашёл к бандитам, спровоцировал, получил перестрелку.
- [ ] Обе банды читаются по тинту (фиолетовые и красные) и отличаются от мирных, в том числе с 12 м.
- [ ] Предупреждение читается как угроза: бандит достаёт ствол, целится, подходит. Если отойти дальше 12 м, он убирает ствол.
- [ ] **Летальность.** Без брони в QA смерть наступала так: от трёх SMG банды 0 через 6–11 с (стоя, на бегу, в рантайме
      11 с при стрейфе); от пистолета и двух дробовиков банды 1 с 9–12 м через 3 с стоя и через 1.5 с, если бежать мимо.
      Реши, не слишком ли быстро, особенно у красных с дробовиками. Ручки: `combat.trigger_seconds`,
      `combat.aim_error_deg` в `assets/gang/gangs.ron`, урон оружия в `assets/combat/weapons.ron`.
- [ ] **Держится позади свалки.** Двое бандитов без патронов бьют тебя кулаками вплотную. Третий, со стволом, стоит в
      8–15 м с оружием наготове, в свалку не лезет и не стреляет, пока свои закрывают линию. Если убежать, кулачники
      бегут рядом, и стрелок так и не выстрелит, давят кулаки. Оцени, читается ли это как "прикрывает", а не как "завис".
- [ ] **Живой щит.** Если рядом с тобой (до 2.5 м) стоит прохожий, бандиты с SMG и дробовиками не стреляют вообще, пока он
      рядом, а с пистолетом не стреляют, если прохожий ближе ~2 м. Прохожие обычно разбегаются за полсекунды. Сжавшийся от
      страха прохожий рядом с тобой даёт укрытие. Реши, нормально ли это.
- [ ] **Патроны кончились, дальше кулаки.** Бандиты с дробовиком (12 выстрелов) и пистолетом (24) после этого идут в
      рукопашную. Своих кулаками не задевают (в QA 0 урона своим). Посмотри, как это выглядит.
- [ ] Группа из трёх держится плотной кучкой плечом к плечу и в перестрелке двигается одной кучей. Нормально ли это на вид.
- [ ] Группы не появляются в кадре. Раненые (< 30 %) отходят. Убитый бандит роняет ствол, его можно поднять.
- [ ] Погоня обходит угол дома и не трётся о стены.
