# QA_REPORT — TASK-012 (GDD T11): police on foot and arrest

QA: claude/opus, medium. Tree `feature/t11-police` @ `45591d4` (clean; my temporary probe test file was moved to
`scratch/qa/` after the runs, `git status --short` empty at the end).

## 0. Preflight

- `scratch/` read first as a coverage map (implementer flips, corridor geometry, N1 plaza probe, fixer flips,
  earlier t9/t10/t11 runs). Nothing of it counted as verification; every number below is from my own runs.
- Read from disk: TASK_FINAL.md, PLAN_FINAL.md (all 750 lines), IMPL_SUMMARY.md, IMPL_REVIEW.md, FIX_SUMMARY.md,
  OPEN_DECISIONS.md, log.jsonl, PCTX_PROPOSALS.md.
- Fixer claims checked in the code: Issue 1 (`police_city.rs` plaza = `citygen::sidewalk_anchor` of the tower plus a
  `GATE BROKEN` sunk-player assert, lines 41 and 84-90), Issue 2 (`tactics/mod.rs::head_for` skips the age refresh
  only while `walked && nearest_node(chest) == goal`), Issue 3 (the leg past the last node uses `avoid_offset` on the
  AI slot), Issue 4 (`fire_line.rs:203` `on_slot.then(|| queue_slot(..))`), P9 doc comment. All present as described.
- Dead-end log triage: the only `dead_end` is the P9 single-guard flip. The reviewer confirmed the second guard in
  `police_fsm`, and `arrest.rs:32` / `behavior.rs` show both guards. No action needed.

### Disconfirmation (done first)

The counter-example I chose was **cops on opposite sides of the player shoot each other through the player**. The
police `hold_fire` builds its line from the unit's chest and counts a spared body only when `along < weapon.range`
(`tactics/fire_line.rs:63`). The bullet starts at the muzzle, about 0.45 m ahead of the chest, and hits a capsule
surface 0.3 m before its centre. A cop just past `range` along the line is therefore not a shield, yet the bullet
still reaches it. The surround rows (3-5 stars) put units on opposite bearings, so this layout comes from the design,
not from bad luck.

**It held.** See Bug 1: 12 SWAT in a street on the test floor, cop-to-cop damage in 4 of 5 layouts, every hit about
45 m across the player (SMG range 45 m). Extending the line's range by 1 m (in-memory probe, file restored and checked
by sha256) removed every long-range hit. That confirms the mechanism.

## 1. Environment

- No docker-compose, no dev server. Direct: cargo test runners plus the real windowed release build driven over BRP
  (`tools/qa/brp.py`, `cargo build -p gta_like --features dev --release`, seed 1). No mocks.
- Host monitor (read only through `Game.frame_report()`): `\\.\DISPLAY1` **144 Hz**, primary, present mode **Fifo**, in
  all three runtime sessions (t11, t9, my probe). No 30 Hz display today.
- Reproduce:
  ```
  cargo build -j 4
  cargo clippy --workspace --all-targets -j 4 -- -D warnings
  cargo test -p gta_sim -j 4
  cargo test -p citygen -j 4
  cargo test -p gta_like --bin gta_like -j 4          # x3
  python tools/qa/scenarios/t11.py --out <dir>
  python tools/qa/scenarios/t10.py --out <dir>; python tools/qa/scenarios/t9.py --out <dir>
  python maw/tasks/in_progress/TASK-012/scratch/qa/qa_runtime.py --out <dir>      # QA runtime probe
  # headless QA probes: copy scratch/qa/zz_qa_probe.rs to crates/gta_sim/tests/, then
  cargo test -p gta_sim -j 4 --test zz_qa_probe -- --nocapture --test-threads 1
  ```
- Services started: only the game process, launched and shut down by each script (`brp_extras/shutdown`).
  `tasklist` shows no `gta_like` left. No containers.

## 2. Test results

### Existing suites

| Command | Result |
|---|---|
| `cargo build -j 4` | ok |
| `cargo clippy --workspace --all-targets -j 4 -- -D warnings` (after `touch crates/*/src/lib.rs`) | clean |
| `cargo test -p gta_sim -j 4` | **292 passed, 0 failed, 1 ignored** (`city_startup_budget`, `#[ignore]` since TASK-003); 31 binaries (`scratch/qa/sim_tests.txt`) |
| `cargo test -p citygen -j 4` | 9 + 3 + 14 passed, 0 failed, 2 ignored (pre-existing) (`scratch/qa/citygen_tests.txt`) |
| `cargo test -p gta_like --bin gta_like -j 4` x3 | 45 passed x3 (`scratch/qa/client_x3.txt`) |
| `cargo tree -p gta_sim -e normal -i bevy_render` | nothing to print |
| `python tools/qa/tree_check.py` | passed |

No failures at HEAD, so no failure list differs from the base commit. I did not rebuild the base.

### My flip-RED

`police_alert`: `if cop_hit || witnessed` became `if cop_hit`, so a witnessed player shot no longer makes 1-star cops
hostile. `police_arrest::attacking_player_is_shot_not_arrested` went **RED** ("the cop kept arresting an attacker",
`police_arrest.rs:243`). After restoring, sha256 was OK and the suite was **GREEN** (9/9). The mechanism under test is
the P4 claim (a player attack makes the arrest row shoot). The implementer's flips cover P1..P9, D1..D9, C1..C3 and
the corridors. The fixer's F1..F5 cover `head_for` and the plaza fixture.

### Runtime (release, BRP, seed 1)

| Run | Result |
|---|---|
| `t11.py` (`scratch/qa/t11_run1/`) | **PASS.** 1 star: cops' state-reach 2.7 / 4.0 s, cop in arrest reach at **9.9 s**, Busted at **11.8 s**, BUSTED → Playing 4.73 s (poll latency; headless P1 is exact: Screen at update 128, Playing at 320), respawn **0.0 m** from the station, guns and bat confiscated, heat 0. 4 stars: SWAT at 28.9 m after 2.6 s (8 units, 4 SWAT). **stuck `{}` in both runs.** `log_errors` empty. Frame: 144 Hz Fifo 144 FPS; no-vsync cost 2.43-2.61 ms |
| `t10.py` (`scratch/qa/t10_run/`) | PASS (call after 4.08 s, blinking, clear after 10.09 s, 2-star mutation, reset) |
| `t9.py` (`scratch/qa/t9_run/`) | PASS (HQ group 2, both Attack, 6 + 7 shots, members at full health, drawn 2); frame 144 Hz Fifo, no-vsync 2.42-2.46 ms |
| QA probe `qa_runtime.py` (`scratch/qa/runtime_probe/summary.json`) | see below |

Note on the stuck evidence: t9 and t10 never exercise cop arrival (t10 teleports the player 809 m away, t9 has no
police heat). t11's `Tracker` counts a cop as "reached" once it is in `Arrest`/`Attack`, whatever its distance, and the
SWAT run ends 2.6 s after the heat write, which is under `STUCK_S` = 5 s. So t11's `stuck: {}` cannot show a stuck
cop. My probe measures physical distance instead.

**Cop arrival at 2 stars** (4 patrol units, player armoured and passive, 45 s per spot; "reach" = flat distance
<= 18 m, the patrol band edge):

| Spot | Arrival (s after the unit's first poll) | Held without 1 m progress >= 5 s before arrival |
|---|---|---|
| hospital sidewalk | 10.4 / 14.7 / 23.3 / 33.6 | 2 units, **in `Attack`** at 28.7 m and 31.0 m |
| police station | 6.5 / 9.5 / 20.3 / 20.3 | 2 units, **in `Attack`** at 31.6 m and 34.6 m |
| gang-1 territory post | 5.7 / 6.1 / 6.5 / 6.5 | none |

**No cop stuck in `Respond`/`Search` at any spot.** All 12 reached the band. Every stall happened in `Attack`: units
stood 30-35 m out, holding their line, for 5-13 s before closing in. Navmesh input: graph-only navigation plus the
fixer's walked-leg avoidance got every cop to the player. The stalls are fire-discipline holds, not navigation.

**5 stars at the hospital sidewalk** (12 SWAT, 40 s): 7 of 12 reached 5-12 m (21-33 s). **5 of 12 stood in `Attack`
at 21.8-23.0 m for the whole window** (z ≈ 559.8 and z ≈ 515.0 around the player at z 537.8, the same sidewalk line,
about 45 m apart). No cop health was lost in this runtime window. Frame at 5 stars: 144 Hz Fifo 144 FPS, no-vsync
2.45-2.49 ms.

**Arrest next to a gang fight** (gang-0 HQ, player 12 m from the post, `GangHeat[0]` = 120 by named mutation, 4
members in `Attack`, heat to 1 star): two cops came in `Arrest`, the hold started at t 11.7 s, **Busted at 13.7 s**.
Cop health lost: none. After respawn: heat 0, no gun owned. Screenshot `runtime_probe/gang_arrest_busted.png`: the
camera sits inside a body (a grey close-up fills the frame, BUSTED text on top). This is an owner look item.

Screenshots I looked at:
- `t11_run1/busted.png`: blue BUSTED over a desaturated frame, the player from behind, star HUD at 1. The arresting
  cop is hidden behind the player, and the kneel pose is not readable from this angle.
- `t11_run1/swat.png`: 4 stars, one SWAT about 29 m down the sidewalk, a few pixels tall. The look cannot be judged
  from it. A tracer line crosses the frame.
- `runtime_probe/swat_5star.png`: 5 stars, a cluster of dark-blue figures about 20 m down the sidewalk. Still small.
- `runtime_probe/gang_arrest_busted.png`: see above (camera inside a body).

### New headless probes (`scratch/qa/zz_qa_probe.rs`, output `scratch/qa/probe_output.txt`; production composition)

| Probe | Result |
|---|---|
| Arrest while a gang member shoots the passive player from the side (armour 1e6 / armour 0) | Busted at tick 222 in both. The gang fired 3 shots and hit the player 3 times. Cop: 0 damage, stayed `Arrest`. Heat stayed 40 (gang fire is not the player's crime). PASS |
| Arrest with the gang member directly behind the cop (the cop in the line) | Gang fires 0 shots: the pressed cop yields the target, the gang spares police. Busted at 222, cop 0 damage. PASS |
| Break free: sprint away at hold 1/64 s | broke free after 66 ticks, heat 180 (P3 reproduced) |
| Break free: sprint sideways | broke free after 65 ticks |
| Sprint away at hold 1.30 s / 1.44 s | Busted after 13 / 4 ticks. The gap never passed 1.36 m, so the hold never paused. Fair: a late escape fails |
| Sprint through the cop | Busted (the gap stayed 0.60-1.29 m) |
| **Run away** (default gait, 4.5 m/s = the cop's chase speed) | **Busted after 95 ticks.** The gap stayed 1.05-1.30 m the whole time. See Bug 2 |
| Walk away | Busted after 95 ticks |
| SWAT in a corridor (2.4 / 3.0 / 4.0 m, 4 SWAT in a file; 2.4 m with 3 SWAT from 30 m) | every SWAT fires (29-45 shots in 30 s), 0 cop-to-cop damage. PASS |
| **12 SWAT from both ends of a street** (widths 8 / 10 / 11 / 14 m, `Attack`; 11 m from `Respond`) | **cop-to-cop damage in 4 of 5 layouts** (1-3 hits, 12-25 each). See Bug 1 |
| 12 SWAT, street 6 m / 3 m | 0 friendly fire; **5/12 and 7/12 SWAT fired 0 shots in 40 s** (Bug 3) |

## 3. Acceptance criteria

| Criterion | Test performed | Result |
|---|---|---|
| Headless: 1★ + player near and passive 1.5 s → `Busted`, weapons confiscated | `police_arrest::passive_player_is_busted_and_disarmed` (exact tick s+95, Screen 128, Playing 320, confiscated, station < 0.05 m) green; P2/P5/P9 green; my gang-fight probes Busted at tick 222 with a gang shooting | PASS |
| Headless: unit count <= table | `police_dispatch` D1..D5 (every tick for 320 ticks, per row), D9 after a reset green; runtime t11 polls and my probe (max active 4 at 2★, 12 SWAT at 5★ = row) | PASS |
| Headless: on lost sight cops go to `LastKnownPosition` | `police_arrest::lost_player_is_searched_at_last_known` (P6) green (`dest == last_known`, Search, 2+ points) | PASS |
| Headless: "break free" gives +1 star | P3 green; my matrix: sprint away / sideways → heat 180, 2★ in 65-66 ticks | PASS (see Bug 2 on the Run gait) |
| Runtime QA: `t11.py` exists and passes via `brp.py` (heat to 1★, cop, passivity, BUSTED screenshot, `GameState`, inventory; separate 4★ run with a SWAT screenshot) | ran it: PASS, screenshots checked | PASS |
| Owner-run criterion recorded as an owner checklist | §6 below (Russian) | PASS (recorded; the run itself is the owner's) |
| Every new tuning value in its GDD §12 data file, not a `const` | diff grep: new numbers are in `escalation.ron` / `wanted.ron` / `respawn.ron` / `visual.ron` / `strings.ron`. The only new consts are the `POLICE_CONFIG` path and the test-area `STATION_SPAWN` fixture. The queue-slot offset is derived (radius + clearance) | PASS |
| `cargo build`, `cargo clippy -D warnings`, `cargo test -p gta_sim`, `-p citygen` green | §2 | PASS |
| Existing tests pass | 292 / citygen / client x3 green | PASS |

## 4. Bugs found

### Bug 1 — major — cops shoot each other across the player at the edge of weapon range

- Where: `crates/gta_sim/src/tactics/fire_line.rs:63` (`FireLine::blockers`, shared by gangs and police):
  `along > 0.0 && along < range && perp <= clearance + along * widen`, with `along` measured from the shooter's
  **chest** to the other body's **centre**. The hitscan (`combat/hitscan.rs:233`) casts `stats.range` from the
  **muzzle** (about 0.45 m ahead of the chest in every logged shot) and stops on the capsule **surface** (0.3 m before
  the centre). A body whose centre is 45.0-45.75 m along the line (SMG) is not a shield, but the bullet still reaches
  it.
- Repro: `scratch/qa/zz_qa_probe.rs::qa_swat_twelve` / `qa_swat_twelve_more` (test floor, player armour 1e6 at
  (0, 0, 32), two 78 m walls making a street of width w along x, 6 SWAT at x = +22..+37 and 6 at x = −22..−37, all
  `Attack`, 2560 ticks). Logged hits (shipped code):
  ```
  FF tick 159: shooter at (-20.92,31.07) -> cop at (24.09,31.97), muzzle (-20.49,1.40,31.35), hit (23.84,0.45,31.87), dmg 13
  FF tick 1856: shooter at (33.82,33.36) -> cop at (-11.32,28.70), muzzle (33.39,1.40,33.07), hit (-11.02,0.66,28.67), dmg 12
  FF tick 331 (8 m street): shooter (-20.47,30.81) -> cop (24.42,33.90), dmg 24
  ```
  Friendly hits: widths 8 m → 2, 10 m → 1, 11 m → 3, 14 m → 3, 11 m from `Respond` → 3. Only 6 m and 3 m were clean.
- Expected: a cop never damages another cop. The plan's test table says "0 friendly damage", and the owner checklist
  line reads "копы не стреляют сквозь ... друг друга". Actual: cop-to-cop damage from misses that fly past the player.
- Why it matters: the 3-5★ rows spawn with `surround: true`, which puts units on opposite bearings, and the SWAT band
  (5-12 m) plus the stall at about 22 m (Bug 3) leaves pairs about 45 m apart, which is exactly the SMG range. The
  pistol has the same edge at 60 m. The gang role has had the same defect since TASK-010, and now two roles use this
  code.
- Diagnosis check: `along < range + 1.0` (in-memory, restored, sha256 OK) → the 11 m layout goes 3 → 0 friendly hits.
  Prescription for the fixer (recompute it, don't copy it): compare the reach of the bullet against the near surface of
  the body. For example, a body counts while `along - radius < range + muzzle_offset`, or the line is measured from the
  muzzle. Gate it with a deterministic two-cop layout: the victim's centre sits just past `range` along the line, and
  its capsule is inside the bullet's reach.

### Bug 2 — minor (design/feel, owner) — running away at the default gait never breaks free

- Repro: `qa_break_free_matrix`, "run away": the player uses `Gait::Run` (the default gait of the input,
  `src/input/mod.rs:157`) straight away from the arresting cop. The cop chases at `chase_gait: Run`, the same
  4.5 m/s. The gap stays 1.05-1.30 m, inside `arrest.distance` 1.5, so the hold keeps counting and the player is Busted
  after the same 95 ticks as a passive one.
- This follows the GDD rule ("игрок не атакует 1.5 с"), so it is not a spec violation. The result is that only Sprint
  escapes, and a player who runs without Shift is "passive". Put to the owner (checklist line).

### Bug 3 — minor (liveness, owner-visible) — rear/opposite SWAT stand idle at about 22 m

- Runtime, 5★ at the hospital sidewalk: 5 of 12 SWAT stood in `Attack` at 21.8-23.0 m for the whole 40 s, on the
  same sidewalk line on both sides of the player. Headless 12-SWAT street: 1/12 idle at 8-14 m widths, **5/12 at 6 m,
  7/12 at 3 m** (0 shots in 40 s).
- Likely cause (not proven): with units on both sides, each unit's line past the player is blocked by the opposite
  unit, so the fallback yields or holds. This is the plan's accepted risk ("three shooters in a 2.4 m file may still
  starve") showing up at the 12-unit scale in open streets. The owner sees a SWAT squad that half stands around.

### Observation — gate weakness (no product bug)

`t11.py`'s `Tracker` sets `reached` by state (`Arrest`/`Attack`) whatever the distance, and the SWAT loop exits at the
first SWAT within 30 m (2.6 s). Its `stuck: {}` is therefore not evidence for the navmesh decision. My probe's
distance-based numbers (§2) are.

### Observation — owner look

In the BUSTED phase next to a cop, the orbit camera can sit inside the arresting cop's body
(`runtime_probe/gang_arrest_busted.png`). The camera does not collide with characters. Owner call.

## 5. Verdict

**NEEDS_FIXES.**

Every acceptance criterion passes, headless and at runtime: arrest, unit bound, last-known search, break free, t11,
data-first, green build, clippy and tests. Arrest next to a gang fight and SWAT in a corridor of up to 4 units behave.
Bug 1 is a silent, headless-reproducible correctness defect in the new shared `tactics` layer. It contradicts the
plan's own safety claim, and the surround rows produce its geometry by design. The fix is small and gateable. Bugs 2
and 3 go to the owner and do not block by themselves. After Bug 1 is fixed and gated, the expected verdict is
SHIP-PENDING-RUNTIME with the checklist below.

## 6. Owner checklist (для владельца)

`cargo run --release -- --seed 1`, взять пистолет.

Полная петля "набедокурил → погоня → ушёл или арестован/убит":
- [ ] Набедокурить при свидетеле → 1 звезда → через ~10-20 с приходят 2 копа со стволами.
- [ ] **Вид полиции.** Отличаются ли копы (синий тинт) от мирных? Особенно проверь голубоватого мирного на модели
      `male-c`: эта же модель есть и у полиции. На скриншотах QA копы мелкие, по ним это не решить.
- [ ] **Арест.** Стой спокойно: коп подходит вплотную, через 1.5 с игрок встаёт на колени (сцена 2 с), экран BUSTED
      3 с, появление у участка без оружия, без брони и без звёзд. Читается ли поза на коленях? Камера не залезает
      внутрь копа? (QA видел кадр, где камера внутри тела.) Хорош ли синий цвет надписи BUSTED?
- [ ] **Вырваться.** На 1 звезде, когда коп вплотную, рвани со Shift (спринт) → 2 звезды, копы стреляют. Понятно ли,
      что это "вырвался"? Имей в виду: обычный бег без Shift не спасает, коп бежит с той же скоростью и арестует через
      1.5 с, как стоящего. Так и задумано?
- [ ] Выстрел рядом с копом на 1 звезде → копы стреляют; через 5 с спокойствия снова пытаются арестовать.
- [ ] Спрятаться за домом: звёзды мигают, копы идут к последней точке и бродят по кругу; уйти за круг → розыск спадает.
- [ ] **SWAT на 4-5 звёздах** (убийство копа = 150 heat): тёмные SWAT с SMG, держатся ближе (5-12 м). QA видел, что
      на 5 звёздах часть SWAT (5 из 12) стоит на ~22 м и не стреляет, пока остальные бьют вплотную. Это нормально
      выглядит? На 5 звёздах после 4: 4 патрульных остаются, добавляются SWAT. Нормально?
- [ ] **Летальность.** На 2 звёздах 4 патрульных сняли с брони QA-игрока 2100-3700 урона за 45 с (пистолет 25 за
      попадание). Без брони (100 HP) после подхода копов живёшь 1-2 с. На 5 звёздах за 40 с было 826. Это нужная
      сложность? Смерть → ПОТРАЧЕНО, как раньше.
- [ ] Копы не стреляют сквозь прохожих и друг друга, не толпятся гуськом в узком проходе (после фикса Bug 1).
- [ ] Арест посреди перестрелки с бандой: коп всё равно арестует пассивного игрока, бандиты не стреляют в копа.
      Нормально ли это по ощущению?
- [ ] После ареста или смерти старые копы уходят и не возвращаются; новая звезда приводит новых копов по таблице.
- [ ] Ручки: `assets/police/escalation.ron` (число копов, дистанции, 1.5 с, 3 м, скорости), `wanted.ron` (heat за
      копов), `respawn.ron` (2 с + 3 с), `visual.ron` (цвета), `strings.ron` (BUSTED).

children: 0 launched / 0 reported.
