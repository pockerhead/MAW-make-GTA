# QA_REPORT — TASK-010 (GDD T9, gangs), round 2

Branch `feature/t09-gangs`, HEAD `bf0e216` (fixer round 2). Round 1 is `QA_REPORT.prev-1.md`.
Cost of error: mixed. Round-2 focus (orchestrator): friendly fire = 0, bystanders not hit, the group still
pressures the player, and the "circling without firing" case.

## 0. Preflight and disconfirmation

- `scratch/` listed and read as a coverage map only (round-1 QA probes `qa/`, fixer `fixer2_*` runs). I did not
  re-run any author script as evidence. My probes are new: `scratch/qa2/qa2_probe.rs` (headless) and
  `scratch/qa2/qa2_runtime.py` (BRP).
- Read TASK_FINAL, PLAN_FINAL, IMPL_SUMMARY, IMPL_REVIEW, FIX_SUMMARY (+prev), QA_REPORT.prev-1, OPEN_DECISIONS,
  log.jsonl. Dead ends triaged: the fixer's "check to target only" dead end is real (the check now runs to
  `stats.range`, `behavior.rs:494-500`); the "stateless side dithers" dead end led to `GangMember.sidestep`, and that
  committed side is where I found the new defect (Bug 2).
- **Counter-example I set out to find:** "a member holds fire forever while the group stands next to each other,
  because the hold-fire rule has no way out". Reading `fire_line_shift` (`behavior.rs:194-242`): it treats a
  sidestep as a parallel shift of the line, but the line pivots on the target. For a body BEYOND the target, moving
  the shooter to the "cheaper" side swings the far part of the line toward that body. With a groupmate beside the
  shooter, both members can commit toward each other. **It held**: two members 1 m apart at 12 m, one bystander
  37 m out behind the player → both push into each other and fire 0 shots in 30 s (Bug 2). Worked numbers for member
  A at (−12, 0.5), dummy at (25, 1): A's line reaches z = −1.04 at x = 25, so the dummy's side is +2.04 and its reach is
  0.5 + 37·tan 11° = 7.6. That blocks [−5.6, 9.6], the cheaper side is −5.6, and A walks to −z, which is into
  member B and swings the far line toward the dummy. The trace matches: A `s−1`, B `s+1`.

## 1. Environment

No docker or dev server. I used cargo directly and the windowed release build over BRP (`\\.\DISPLAY9`, 30 Hz, Fifo).

```
cargo build -j 4 ; cargo clippy -j 4 -- -D warnings
cargo clippy -p gta_sim --tests -j 4 -- -D warnings ; cargo clippy -p gta_like --bin gta_like --tests -j 4 -- -D warnings
touch crates/*/src/lib.rs && cargo test -p gta_sim -p citygen -j 4          # scratch/qa2/test_sim.txt
cargo test -p gta_like --bin gta_like -j 4                                  # x3
python tools/qa/tree_check.py ; cargo tree -p gta_sim -e normal -i bevy_render
cargo build --release --features dev -j 4
python tools/qa/scenarios/t9.py --out maw/tasks/in_progress/TASK-010/scratch/qa2/t9
python tools/qa/scenarios/t8.py --out maw/tasks/in_progress/TASK-010/scratch/qa2/t8
python maw/tasks/in_progress/TASK-010/scratch/qa2/qa2_runtime.py <out> 30 <bearing_deg>   # bearings 0, 90, 180
# headless probe: copy scratch/qa2/qa2_probe.rs to crates/gta_sim/tests/, then
cargo test -p gta_sim --test qa2_probe -j 4 -- --nocapture                               # (file removed afterwards)
```

Services: none. Every game run ended with `brp_extras/shutdown`, and `tasklist` showed no `gta_like` afterwards.

## 2. Test results

| Suite | Result |
|---|---|
| `cargo build`, clippy ×3 `-D warnings` | ok / clean |
| `cargo test -p gta_sim -p citygen` (after `touch`) | 26 result lines, all `ok`, 0 failed (gang_combat 9, gangs 7, gang_city 4, config 39, lib 41, …). No failures, so there was no failure list to compare with base. |
| `cargo test -p gta_like --bin gta_like` ×3 | 41/41 each run |
| `tree_check.py`; `cargo tree … -i bevy_render` | passed; nothing to print |
| No new tuning `const` in round 2 (`git diff 75be889 HEAD -- '*.rs' \| grep '+.*const'`) | none. `fire_line_margin`, `sidestep_gait` are in `gangs.ron`; the margin has a sabotage fixture |
| `t9.py` | **exit 0**. This run spawned an HQ group of 2 (SMG, Pistol). Idle ×2, then `Attack` ×2, heat 119.34, capture at 0.58 s alive, both at 100 HP. **The Pistol member fired 0 shots in the 6 s window** (SMG 9). Frame cost no-vsync 2.35–2.41 ms. |
| `t8.py` | exit 0: cap 40 in 1.8 s, scared share 0 → 0.909, no-vsync 2.29–2.40 ms, no log errors |

### Flip-RED (mine)

`fire_line_shift`: `widen = cone.tan()` → `0.0 * cone.tan()` (spread cone ignored). `members_never_shoot_their_own_group`
went RED ("ring3 smg 12 m: friendly hits […26 dmg headshot…]") and `members_do_not_shoot_through_a_bystander` went RED.
I restored the file, sha256 `492a873c…15ce1` matched the original, and both went GREEN.

### Headless probe (mine, production composition: `gang_floor` + `gang_member_bundle`, 30 s each)

Full output: `scratch/qa2/probe_output.txt`. The layouts are new; none of them is in the fixer's gate:

| Layout (player at origin) | shots / member | ranged FF | melee FF | bystander hits | longest no-shot gap |
|---|---|---|---|---|---|
| mixed4 inline 5/9/14/22 m (SMG, Pistol, Shotgun of gang 1, SMG) ×3 jitters | 25/22/12/0 · 28/24/12/16 · 28/24/11/4 | 0 | **1** (j1: out-of-ammo shotgun member punches a groupmate, 10 dmg) | – | **30 s** (22 m SMG) |
| surround4 mixed 6/10/4/16 m, 4 bearings ×3 | 19/13/12/1 · 18/14/12/0 · 21/15/12/0 | 0 | 0 | – | **26–30 s** (16 m SMG) |
| surround4 cross 9/11/13/7 m ×3 | 30/23/28/24 … | 0 | 0 | – | 5–12.7 s |
| crossfire pair 10 m opposite ×3 | 33/31 | 0 | 0 | – | 2.5–3.4 s |
| static bystanders: dummies at (0,−6), (0,8), (0.9,3) + idle rival gang member ×3 | 23–29 / 24 | 0 | 0 | **0**; rival stays `Idle` | 6.4–11.2 s |
| walking civilians on sidewalks: parallel behind / between / along the street (V1, V2, V4, V5) | 21–31 each | 0 | 0 | **0** | 2.8–12.8 s |
| 2 fist members next to the player + SMG at 12 m, player static | 0/0/0 (66 punches land on the player) | 0 | 0 | – | SMG **30 s**, blocked 30 s |
| same, player walking a circle | 0/0/4 · 0/0/6 | 0 | **2 · 3** (punches between the two fist members) | – | SMG blocked 26–28 s |

Lethality, no armour, static player: ring3 Pistol/SMG/SMG at 12 m → Wasted **5.6 s**; ring4 gang-1
Pistol/Shotgun ×2 → **1.95 s**; ring2 SMG → **10.3 s**; surround4 (pistol at 4 m) → **2.6 s**.

### Runtime probe (mine, `scratch/qa2/rt_b0`, `rt_b90`, `rt_b180`, `result.json` + screenshots)

Same setup as t9 at the gang-0 HQ. Then 30 s with armour 1e6 and no player fire, then armour 0 until Wasted.
- Bearing 0 (SMG + Pistol, on the approach line): 27 and 24 rounds spent, both at 100 HP (no FF). The Pistol member
  ran dry at about 22 s and switched to fists. After that the SMG was blocked (sidestep) for 4.75 s. **Wasted 7.3 s**
  after the reset, the last hits being 10-dmg punches.
- Bearing 180 (3 SMG): 33–35 rounds each, blocked < 1 s in total, 100 HP each. **Wasted ≈ 4.3 s** (the last health
  read was 3 HP at 4.14 s, then state `Wasted`).
- Bearing 90 is invalid as evidence. The teleport spot was behind cover, the members lost LOS and stood at
  `last_seen`, 0 shots. This is not the hold-fire rule (`sidestep` was 0 throughout).
- Runtime FF in 3 runs + t9: 0.

## 3. Acceptance criteria

| Criterion | Test performed | Result |
|---|---|---|
| Headless: outside the territory members are neutral | `outside_the_territory_members_stay_neutral` green; round-1 flips still apply (logic untouched in round 2) | PASS |
| Headless: attack on a member → group in 30 m in `Attack` | `attack_on_a_member_aggroes_the_group_within_30m` green (round-1 QA flip radius ×1.1 → RED) | PASS |
| Headless: `GangHeat` decays in 120 s | `gang_heat_decays_in_120s`, `…_while_wasted` green | PASS |
| Headless: matrix off → gangs do not attack each other | `disabled_matrix_gangs_do_not_attack_each_other` green. In my probes the rival gang (matrix off) was never hit and stayed `Idle` next to a firefight. | PASS |
| Runtime: `t9.py` exists, passes via `brp.py` | ran, exit 0 (§2) | PASS |
| Owner checklist recorded | §6 | PASS (recorded) |
| New tuning values in data files | round-2 values in `gangs.ron` (+ fixture for the margin); no new `const` | PASS |
| build / clippy / `cargo test -p gta_sim` (+citygen) green | §2 | PASS |
| Existing tests pass | §2, t8 green | PASS |
| **Orchestrator round-2 rule: zero friendly hits** (OPEN_DECISIONS, gate "0 friendly hits") | ranged: 0 in all 27 headless layouts and 4 runtime runs. **Melee: 1–3 punch hits on groupmates** in scrum layouts | **FAIL (melee part)**: Bug 1 |
| **Orchestrator: members sidestep so they do not just freeze** | deadlock of two adjacent members, sidestep into geometry, bystander behind the player: members fire 0 shots for 30 s | **FAIL**: Bug 2 |
| Orchestrator: bystanders in the line of fire are not hit | 0 hits on dummies, civilians (walking and fleeing) and the idle rival in every layout | PASS |
| Orchestrator: the group still pressures the player | Wasted 2–10 s headless, 4.3–7.3 s runtime without armour (round-1 QA: 7.5 s) | PASS |

## 4. Bugs found

### Bug 1 — Minor/Medium: groupmates still punch each other (melee friendly fire)

- **Cause:** the hold-punch check runs only on the pull tick (`behavior.rs:552-561`), but a punch lands in the
  active window 0.12–0.22 s later (`melee.ron` fists) along the direction fixed at the click
  (`melee.rs:swing_melee`, sphere cast r = 0.35 m). A groupmate that steps into that sweep during the wind-up is hit.
  The fixer's "melee too" claim is only true at the pull tick.
- **Repro:** `scratch/qa2/qa2_probe.rs` (`qa2_probe`). Layouts "melee flank/inline, circle_player=true" (two
  out-of-ammo members next to a moving player) give FF 2 and 3. In "mixed4 j1" the shotgun member runs dry and
  punches the pistol member at (0.05, 1.3, −1.6) for 10 dmg.
- **Expected:** 0 friendly hits (orchestrator rule). **Actual:** 1–3 punches of 10 dmg per 30 s in a scrum. No
  provocation and no infighting follow (same faction is never hostile). The committed gate misses it because its
  layouts never get two members into a melee scrum around a moving player.

### Bug 2 — Medium (owner-visible, liveness): members can hold fire for the whole fight

The hold-fire rule has three ways to lock a member out of shooting for 30 s or more. In each one the member keeps
sidestepping and never fires:
1. **Side-by-side deadlock** (deterministic, clear ground): members at (−12, 0, ±0.5), a bystander dummy at
   (25, 0, 1) behind the player. Both commit toward each other (`sidestep` −1 / +1), push, drift backwards and fire
   **0 shots in 30 s** (`probe_output.txt`, "west pair 1m apart"). The same happened in V6 (cross street with
   civilians 20–26 m behind the player: 12/2/0 shots). Cause: see §0. `fire_line_shift` models the sidestep as a
   parallel shift, so for bodies beyond the target the chosen side is often the wrong one. The committed side is
   never re-evaluated when the member is not moving. Groups spawn 1 m around a post, and civilians 20–45 m behind the
   player are the normal street picture, so this is not an exotic layout.
2. **Sidestep into geometry:** the move is a bare yaw with no `avoid_offset` (`behavior.rs:532-534, 605`). The 22 m
   SMG in "mixed4" walked into the test-floor stairs and stood there with 0 shots for 30 s. In the city a building
   wall does the same.
3. **Scrum next to the player** (the case the orchestrator asked about): while two groupmates punch the player,
   the ranged member **never resolves** against a static player (blocked 30 s, 0 shots). Against a moving player it
   fires 4–6 shots in 30 s (blocked 26–28 s). It does not freeze, it circles at walk speed. Pressure stays high here
   because the punches land (66 in 30 s).
- Related: a bystander pressed against the player (V3: a civilian pushed the player 14 m) makes the whole group hold
  fire and circle for 26+ s, a "human shield". That is a design question for the owner, not a defect.
- Wide cones behind the target also cost time. SMG half-cone = 4° + 3° + 4° = 11°, so at 37 m the blocked band is
  ±7.6 m. Members in surround layouts spend 8–14 s without a shot while repositioning. The t9 runtime run with a
  Pistol member at 0 shots in 6 s is consistent with this, but one 6 s window does not prove it.
- **Expected** (OPEN_DECISIONS round 2: "sidestep to clear the line, so members don't just freeze"): a blocked
  member gets a clear line within a few seconds. **Actual:** unbounded in the three cases above. The committed
  liveness gate (`>= 8` shots per member, fixer layouts only) cannot see it.
- Direction for the fix (diagnosis-level; recompute before applying): compute the blocked shift for a body beyond
  the target with the pivot factor −(along − T)/T, or evaluate both candidate spots directly. Re-pick or flip the
  committed side when the member made no lateral progress for a short time or a groupmate sits on that side. Route
  the sidestep through `avoid_offset`. A gate: my "west pair 1 m + dummy" layout must give every member ≥ N shots.

### Notes

- Ranged friendly fire is fixed: 0 in every layout I tried, including cross-fire through the player and 4 members
  around the player at varied radii.
- One small residual in the geometry: the muzzle sits 0.25 m sideways (`aim.ron`), which is more than
  `fire_line_margin` 0.2. A pellet at the edge of the full cone can still clip a body just outside the checked band.
  I did not see it happen in any run; I note it only as a margin.
- The check ignores walls. A bystander behind a wall still blocks the line, and a member holds fire although the
  bullet would stop at the wall. This is conservative, and it costs only liveness.

## 5. Verdict

**NEEDS_FIXES.** Every stated acceptance criterion of TASK-010 passes: headless, runtime t9/t8, frame cost
~2.4 ms, clean build and clippy, and my flip went RED and restored by sha256. Ranged friendly fire is 0 and bystanders
are never hit. The round-2 fix also has two reproducible defects against the orchestrator's own decision:
groupmates still punch each other (Bug 1), and the sidestep can lock a member out of firing for the whole fight
(Bug 2.1 is deterministic on clear ground with a normal group spacing). The owner would see Bug 2 as "a bandit walks
around next to his buddy and never shoots". If the orchestrator rates Bug 2 as owner feel for this slice, the verdict
drops to SHIP-PENDING-RUNTIME with §6. In that case add a pointed owner item and file a follow-up task, because T11
police will reuse this rule.

## 6. Owner checklist (runtime, owner)

Launch: `cargo run --release -- --seed 1`. Take the pistol at the range and go to the gang-0 HQ around
(116, 0.15, −486). The gang-1 HQ is around (−491, −308).

- [ ] Зашёл к бандитам, спровоцировал, получил перестрелку.
- [ ] Обе банды читаются по тинту (фиолетовые / красные) и отличаются от мирных, в том числе с 12 м.
- [ ] Предупреждение читается как угроза: бандит достаёт ствол, целится, подходит; отошёл дальше 12 м, он убирает ствол.
- [ ] Группы не появляются в кадре.
- [ ] **Летальность:** без брони в прогонах QA смерть наступала через 4.3–7.3 с (рантайм, 2–3 бандита на 12 м) и
      через 2–10 с в headless (пистолет в упор 2.6 с, два SMG 10 с). Решить, не слишком ли быстро. Ручки:
      `combat.trigger_seconds`, `combat.aim_error_deg` в `assets/gang/gangs.ron`.
- [ ] **Свои не стреляют в своих и в прохожих:** в перестрелке бандиты не падают без твоих попаданий, прохожие на
      линии огня не получают пуль; бандит, которому мешает свой или прохожий, шагает в сторону и стреляет.
- [ ] **Кружение без стрельбы:** встань так, чтобы двое бандитов стояли рядом друг с другом, а за тобой по улице шли
      прохожие. Посмотри, не топчутся ли они часами, упираясь друг в друга и не стреляя (баг 2 в QA). Второй случай:
      двое без патронов бьют тебя кулаками, третий со стволом ходит кругами и не стреляет, пока ты стоишь. Это
      поведение "не стреляю в своих". Оцени, как это читается.
- [ ] **Кончились патроны → кулаки:** у бандитов с пистолетом или дробовиком патроны кончаются примерно через 20 с
      непрерывной стрельбы, дальше они идут в рукопашную. В QA такие бандиты иногда задевали кулаком своего (10 урона,
      баг 1). Посмотри, как это выглядит.
- [ ] В перестрелке держат 8–15 м, мажут чаще игрока, бьют в упор, раненые (< 30 %) отходят.
- [ ] Погоня обходит угол дома и не трётся о стены; шаг в сторону не упирает бандита в стену.
- [ ] Убитый бандит роняет ствол, его можно поднять, через 60 с ствол исчезает.
- [ ] Ствол в руке бандита не мигает и не пропадает.
- [ ] FPS рядом с перестрелкой на своём мониторе. Здесь 30 Hz Fifo даёт 30 FPS, без vsync кадр ~2.4 мс.

## 7. Files

- Probes and evidence: `scratch/qa2/qa2_probe.rs`, `scratch/qa2/probe_output.txt`, `scratch/qa2/qa2_runtime.py`,
  `scratch/qa2/rt_b0|rt_b90|rt_b180/` (`result.json`, screenshots), `scratch/qa2/t9/`, `scratch/qa2/t8/`,
  `scratch/qa2/t9_run.txt`, `scratch/qa2/t8_run.txt`, `scratch/qa2/test_sim.txt`.
- `log.jsonl`: one `decision` entry (how liveness was measured).
- `git status --short`: clean. The probe test file was moved out of `crates/gta_sim/tests/`, and the flipped
  `behavior.rs` was restored and matched by sha256.

children: 0 launched / 0 reported.
