# TASK-035 QA_REPORT (qa, claude opus)

Code under test: `6b4c844` (implementer `106382e` plus fixer round 1), branch `bugfix/lethality-balance`.
The working tree is clean apart from the untracked task-dir artifacts. My two flips were restored, and sha256 confirmed each restore.

## 0. Preflight and disconfirmation

- Scratch coverage map: the author covered headless gates, flips and a traffic probe table. Nobody ran BRP runtime before me.
- The counterexample I wrote down first: in the real game at 1★, a player who shoots a civilian while 2 patrol cops walk up
  is still shot, because of some path other than `police_alert` (heat jumping to 2★ from the civilian kill,
  `CopSenses.hostile` set elsewhere, and so on). I also checked a second one against decision 3: "police rams still
  kill". The pursuit speed is 20 m/s, so a player victim takes (20−3)·12·0.4 = 82.
  - Result 1: **held (no bug).** Seed 1, release, runtime: the cops were 9.7 and 11.5 m away at the click, the civilian was hit
    (100 → 73, damage number 27 on screen), and heat went 40 → 80, still 1★. `PoliceAlert` stayed 0. No cop entered
    Attack, the player kept 100 HP, and the result was BUSTED 2.9 s later (`scratch/qa/m3/summary.json` "A",
    `m3/a_civilian_shot.png`). A civilian kill gives 40 + 10 heat, and 2★ needs 180, so the 1★ window survives about 3 kills.
  - Result 2: **partly holds (minor, owner feel).** Police cars have no ram mechanic (grep shows no "ram" in src/GDD). A
    pursuit car at 20 m/s into a standing full-health player does about 82 and does not kill. The player running head-on
    into it (24.5 m/s closing) takes 103 and dies. "Deaths from cars stay possible at high speed" holds only for closing
    speeds ≥ about 23.8 m/s.
- Dead ends in the log: the accuracy knob was rejected, and the damage_scale-0.01 ceiling flip did not go RED because of the 1 hp floor. I checked
  the refs: `roll_damage` floors at 1 (weapons.rs), and `FireLine::of` widens by the cone. Both claims match the code.
- Fix claims checked in the code (`git diff main...HEAD`): victim scoping is on `target` after `ColliderOf.body`
  (hitscan.rs:307-309, 350), so headshots on the player are scaled. `BulletHitVehicle.damage` is unscaled, and the cabin wound
  is scaled only for a `Player` driver (impact.rs:203). `per_mps` is back to 12, with `player_share` 0.4 validated in (0,1]. All fixes are real.

## 1. Environment

Direct: this checkout, shared `target/`, no docker and no mocks. Commands:
- `cargo test -p gta_sim -p citygen --no-fail-fast` → `scratch/qa/test_sim_citygen.txt`
- `cargo test -p gta_like --bin gta_like` → `scratch/qa/test_client.txt`
- `cargo clippy --locked --workspace --all-targets -- -D warnings` → `scratch/qa/clippy.txt`
- BRP, release `--features dev`, `--settings-id .qa` (brp.py `QA_SETTINGS_ID`), one scenario at a time:
  `python tools/qa/scenarios/t{9,11,12,15}.py --out maw/tasks/in_progress/TASK-035/scratch/qa/t<N>`,
  plus t15 twice more (`t15_run2`, `t15_run3`).
- My own runtime probes: `python maw/tasks/in_progress/TASK-035/scratch/qa/m3_probe.py` (M3 counterexamples,
  output in `scratch/qa/m3/`, final run log `m3_probe_run3.log`) and `scratch/qa/ttk_probe.py` (runtime 1★ TTK, output in `scratch/qa/ttk/`).
- Monitor `\\.\DISPLAY9` 30 Hz, Fifo: 30 FPS shipped, no-vsync frame cost 2.7-3.0 ms (t9/t11 frame_report).
- The owner's game was not running. No game process is left after my runs (checked).

## 2. Test results

| Suite | Result |
|---|---|
| sim + citygen | 66 result lines, **535 passed, 0 failed**, 5 ignored |
| client `gta_like` bin | 82 passed |
| clippy `-D warnings` | clean |
| t9 (gang aggro, firefight) | exit 0, all asserts pass; 3 members in Attack, no crossfire, log clean |
| t11 (1★ arrest, 4★ SWAT) | exit 0; cop in reach 6.8 s, BUSTED 9.0 s; SWAT at 9.4 m after 4.8 s; stuck list matches the TASK-016 baseline (1 Respond cop at ~31 m, 1 at 12 m) |
| t12 (minimap, pause, new city) | PASS |
| t15 (traffic, hijack, 2★ chase) | PASS ×3; **pressure 3/3** (police car within 16.1-16.7 m at 12.55-12.69 s, no escape), dismount reached, crew 4 |

My flips (each restored, sha256 OK):
- `escalation.ron` stars[1].damage_scale 0.15 → 0.3: `two_two_star_cops_take_8_s` goes **RED** (median 4.61 s < 8).
- `police_alert` near-miss radius ×100 (every player trace counts as a shot at police):
  `one_star_cops_arrest_a_civilian_shooter` and `..._shoots_a_civilian_in_front_of_them` go **RED**.

Runtime M3 comparison (seed 1, release, my probes, against TASK-031 M3):

| Case | TASK-031 M3 (before) | Now |
|---|---|---|
| 1★, shoot a civilian, 2 cops 10-12 m away | cops kill in ~3 s | no fire, alert 0, 100 HP, BUSTED at 2.9 s |
| 1★, shot at a cop (near miss), then stand | dead in ~3 s | cops fire for the 5 s alert: 100 → 36 HP, then arrest, BUSTED at 7.7 s (not killed) |
| 1★ pair, alert held by mutation (runtime TTK) | ~3 s | **11.3 / 8.9 / 9.4 s** (cops fire from 7.4-11.6 m) |
| 2★ full row, stand | 13 s from heat set | 16.8-20.4 s from heat set; ~7.6 s from first damage to death |
| Step in front of traffic (11-15 m/s cars, 3.4-4.8 m ahead of centre) | 36-85 HP lost, one kill | 0-20 HP lost, knocked down in 6 of 10 (80-100 HP left); cars braked before contact at runtime; the full-cruise hit is carried by the headless gate (41 HP standing, 24 running) |

Screenshots (read by me): `m3/a_civilian_shot.png` (hit number 27 on the civilian, 1★, full health bar),
`m3/b_end.png` (BUSTED greyscale, cop at arm's length, health bar about 1/3), `m3/d_traffic_hit.png` (player on the lane next to
the braked car, health bar full), t9/t11/t12/t15 PNGs next to their summaries.

## 3. Acceptance criteria

| Criterion | Test performed | Result |
|---|---|---|
| 1★ rule: civilian shooter near 2 patrol cops → 0 police shots while the arrest proceeds; shooting a cop → fire; flip RED | Headless `lethality::one_star_*` (5 gates) green; my own near-miss flip RED; runtime probe A (no fire, BUSTED) and B (near miss → Attack for 5 s) | PASS |
| TTK: 2 patrol ≥ 8 s at 1★-after-attack and 2★; 3 gang at 10 m ≥ 6 s; medians reported | Headless: 11.17 / 11.17 / gang 14.00 & 8.41 (min 6.39); my flip of stars[1] RED; runtime 1★ held-alert TTK 8.9-11.3 s | PASS |
| Traffic hit at max cruise leaves a 100 HP player alive; tuning in RON | `traffic_pedestrian::cruise_hit` (standing 41 HP, running 24 HP) green; runtime: 80-100 HP left, knocked down | PASS |
| GDD 1★ row amended; no new tuning consts | GDD §6.4 row 1 plus the knob sentence (the row sits in §6.4, not "§7"); diff has no new `const` in src (the test-only floors restate the spec) | PASS |
| Existing tests pass; t11/t12/t15 still show police pressure | 535 + 82 green, clippy clean; t9/t11/t12 pass; t15 pressure 3/3 | PASS |

## 4. Bugs and findings

No blocking bug.

- **Minor / owner feel: a shot at arrest range counts as an attack on police.** Runtime probe A run 2: the click landed with the cops
  already arresting at 0.6-1.0 m. A shot at a civilian 6 m away, away from the cops, set the alert, and the cops fired for 5 s
  (100 → 40 HP). This is the accepted m1 / OPEN_DECISIONS behaviour and is stated in the GDD. A human shooting "during" an
  arrest will see it.
- **Minor / owner feel: close-range damage rate at 1★ is higher than the gate distance.** In the 5 s alert window at 1-7 m the pair
  did about 14 HP/s (B: 100 → 36 in about 4.5 s). The runtime TTK with the alert held is still 8.9-11.3 s, because the cops back off to
  7-12 m (keep_distance). A player who keeps shooting at point-blank cops may die in about 7 s. That is not a spec failure (the target
  is met at the distances the cops actually fight from), but it is a number to feel.
- **Minor / spec wording: "police rams" rarely kill.** There is no ram mechanic. A pursuit car at 20 m/s does about 82 to a standing player.
  One hit kills the player only from about 23.8 m/s closing speed, for example when he runs head-on into a pursuit car. The player's own car cannot hit him.
- **Nit:** `tests/vehicle_hits.rs` is 799 lines (over the 750 warning, under the 950 limit). The fixer disclosed this.

## 5. Verdict

**SHIP.** Per the orchestrator's rule, no scenario oracle regressed (t9/t11/t12/t15 all pass) and police keep pressure at 2★
(t15 3/3 pressure, better than the ~3/5 of before). All five criteria pass headless, with my own flips RED, and hold at runtime
against the M3 counterexamples.

### Owner checklist (numbers to feel, `cargo run --release`)
1. 1★: shoot a civilian while 2 cops approach from 10 m. They should come to arrest you, not shoot.
2. 1★: fire next to or at a cop. They shoot for 5 s (expect to lose 50-65 HP at close range), then arrest.
3. 2★ standing in the open: about 7-8 s from the first hit to death (full row, 4 cops); 5★ is about 2.5 s (headless).
4. Step in front of a traffic car: you are thrown clear with most of your health; running into a car head-on at cruise
   leaves about 24 HP. The stronger run-over knockback (`shove_scale` 1.0) also applies to civilians you hit.
5. Police cars at 20 m/s do not kill a standing full-health player.

children: 0 launched / 0 reported
