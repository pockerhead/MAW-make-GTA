# QA_REPORT — TASK-017 (GDD T16), final QA

QA: claude opus (maw-qa, medium). Tree: `feature/t16-final` @ `7be596e`. The only dirty file is
`maw/tasks/in_progress/TASK-017/metrics.md`, which is the orchestrator's edit and was left as found. No code was
changed. QA files live under `scratch/qa/` (gitignored like every scratch dir; force-add them if they should be kept).

Loaded before any testing: `scratch/` (as a coverage map), `TASK_FINAL.md`, `PLAN_FINAL.md`, `IMPL_SUMMARY.md`,
`IMPL_REVIEW.md`, `FIX_SUMMARY.md` (round 2), `FIX_SUMMARY.prev-1.md` (round 1), `OPEN_DECISIONS.md`, `log.jsonl`.
The log has one `dead_end` entry (premise, t13 screenshot latency). It does not affect the current code.

Per the orchestrator note, I did not repeat the t16 numbers (fixer round 2, 3 runs) or the implementer's full
×N scenario sweep.

## 0. Disconfirmation (done first)

**Counter-example:** "the player clicks fire rapidly (a press lands in the 0.15 s buffer, or LMB is held with the
SMG) and presses F at a car door. A shot leaves from the driver's seat, on the enter tick or up to 0.15 s after it,
or the queued press fires on exit."

Code path: `seat::enter_exit` (`crates/gta_sim/src/vehicle/seat.rs:291-296`) resets `ActionIntent` and clears
`Loadout.fire_queued`. `fire_weapons` (`combat/hitscan.rs:186-219`) has no `Driving` filter, so everything rests on
those two resets and on the client's `OnFoot`/`InVehicle` context switch (`src/input/mod.rs:191-222`). I also
suspected a one-frame hole: `sync_contexts` runs in `Update`, after `Action<Fire>` has already been evaluated with
`OnFoot` active. Only a runtime test could settle that.

**Result: the counter-example did not hold.**
- Headless flip (my own, with sha256 restore): I replaced the seat clear with a no-op. The result was
  `vehicle_seat::a_queued_press_does_not_fire_from_the_seat` RED, `left: (1, 1) right: (0, 0)` ("the queued press
  fired from the seat"). After the restore the sha256 matched (`f22a8c45…a6b0`) and the test was GREEN (20/20).
- Runtime probe with real OS input: `scratch/qa/probe_fire_seat.py`, results in `probe_fire_seat.json` and `.txt`.
  8 reps × 4 trials. The detector takes atomic BRP snapshots (one `world.query` returns the player's `Loadout` plus
  `has Driving`), about 7 ms apart with a max gap of 32 ms (2 ticks). A magazine drop between two `Driving`
  snapshots counts as a definite seat shot.

| Trial | What the input did | Valid runs | Seat shots | Shots on exit |
|---|---|---|---|---|
| control (no F) | click, +0.20 s click | 7/8 on foot | – | – |
| queue_then_F | click, +0.20 s click (queued), +0.02 s F | 8/8 entered | **0** | **0** |
| burst_across_F | pistol, 12 clicks at 45 ms, F after the 6th | 7/8 entered | **0** (1 ambiguous drop on the enter tick) | **0** |
| smg_hold_across_F | SMG, LMB held, F while held, held 0.6 s more | 7/8 entered | **0** | **0** |

  The control fired **2 shots in 7/7 on-foot runs**, so the second click really is buffered. With F the same input
  fired **1 shot in 7/7 clean runs**, so the queued press was dropped when the player got in. The runs marked
  invalid are state drift in my probe (the previous exit had not finished, or there was a 766 ms hitch). The one
  "ambiguous" drop falls between the last on-foot snapshot and the first `Driving` one, which is a shot on the enter
  tick itself. It cannot be told apart from an on-foot shot, and no definite seat shot was seen in 29 valid trials.
  The one-frame context hole I suspected did not show up at runtime.

## 1. Environment

- No docker or dev server. I ran the direct cargo test infrastructure plus the windowed game over BRP.
- Host: i9-11900K, RTX 4070 Ti, Windows 11. Every cargo command ran alone in the foreground with `-j 2`. The game
  was always started with `--settings-id com.github.pockerhead.maw-make-gta.qa`, which is built into `brp.py`.
- Runtime used release `--features dev` builds, driven by `tools/qa/brp.py`, `tools/qa/repeat.py` and
  `tools/qa/osinput.py`.
- Reproduce:
  - `cargo build -j 2`, `cargo clippy --workspace --all-targets -j 2 -- -D warnings`,
    `cargo clippy -p gta_sim -p citygen --all-targets -j 2 -- -D warnings`
  - `cargo test -p gta_sim -j 2`, `cargo test -p citygen -j 2`, `cargo test -p gta_like --bin gta_like -j 2`
  - `python tools/qa/repeat.py t16_s1 --runs 3 --out target/qa/qa17`
  - `python tools/qa/repeat.py tN --runs 1 --out target/qa/qa17sweep` for N = 1..15
  - `python maw/tasks/in_progress/TASK-017/scratch/qa/probe_fire_seat.py 8`
  - `python maw/tasks/in_progress/TASK-017/scratch/qa/probe_p10.py 10 [--after-p7]`
- Services started: only game processes, and each one ended through `brp_extras/shutdown` or on its own. No game
  process is left (checked with `tasklist`).

## 2. Test results

### 2.1 Headless
| Command | Result |
|---|---|
| `cargo build -j 2` | ok (2 m 25 s) |
| `cargo clippy --workspace --all-targets -j 2 -- -D warnings` | clean. The first run finished in 2.6 s on stale fingerprints (the TASK-009 phantom class), so I touched `src/main.rs` and `crates/*/src/lib.rs` and re-ran it: 19 s, clean |
| `cargo clippy -p gta_sim -p citygen --all-targets -j 2 -- -D warnings` | clean |
| `cargo test -p gta_sim -j 2` | 58 binaries, **478 passed, 0 failed** (`scratch/qa/sim_tests.txt`) |
| `cargo test -p citygen -j 2` | 32 passed, 0 failed |
| `cargo test -p gta_like --bin gta_like -j 2` | 81 passed, 0 failed |
| Own flip-RED | see §0. Seat clear removed → RED; restored with sha256 identical → GREEN |

The failure list is empty, the same as the fixer's `scratch/fix/r2c_sim.txt` (478/0), so nothing was swapped. I
checked the fix claims against the code: the seat clear (`seat.rs:293-296`), `validate_overshoot` chained in
`compose_sim` (`lib.rs:120`, `police/mod.rs:399-411`), and `reset_fire_queue` now registered only on
`OnExit(Wasted)`. All present.

### 2.2 Runtime: t16_s1 ×3 (focus 1)
`scratch/qa/t16_s1_x3.txt`: **2/3 passed.** Run 3 failed on phase **P10_settings**:
`world.get_components: Entity 10297v2 not found`. The "Настройки" button entity was despawned between the
`texts()` query and `ui_center()`, and the only input between those two calls was `osinput.focus()`. All other
phases passed in all 3 runs.

To measure the flake rate I made 5 extra runs (`scratch/qa/t16_s1_x5_extra.txt`): 4/5. Run 4 failed on **P10**
again with a different symptom: the click on "Настройки" did not open the settings screen (still `Paused`/`Main`).
The script then crashed while printing, because `print(json.dumps(..., ensure_ascii=False))` hit a `★` in the error
message on the cp1251 console. The crash hides the assertion message, but `summary.json` was already written.
In total, **P10 failed in 2 of 8 runs; P1, P2, P3/P5, P7, P8 and P9 passed 8/8.**

I isolated it with `scratch/qa/probe_p10.py`, which runs P10's exact sequence (focus+Esc → Paused → focus+click
"Настройки" → `PauseMenu::Settings`):
- fresh session: 10/10 with the shipped `osinput.focus` (Alt tap every call) and 10/10 without the Alt tap
  (`probe_p10.txt`);
- after the P7 state (in a car on the range, `--after-p7`): 10/10 and 10/10 (`probe_p10_after_p7.txt`).

So the flake is 0/40 in isolation and 2/8 inside the full scenario. I could not pin the root cause. Both failures
happen right after `osinput.focus()`. That function taps Alt on every call, even when the window is already
foreground, and a bare Alt tap puts a Win32 window into menu mode, which swallows the next click. That fits
symptom 2, but not symptom 1 (menu torn down with no click sent). One more observation: during the first
`--after-p7` probe attempt the game logged `No windows are open, exiting` (window closed, no error, no panic) about
80 s in. It did not reproduce on the rerun, and the cause is unknown. It happened while the probe was sending OS
Alt/Esc/clicks. Nothing in the logs points at the game, and the paths through `PauseMenu` work 40/40.

### 2.3 Runtime: t1..t15 ×1, post-fix regression sweep (focus 2)
`scratch/qa/sweep_t1_t8.txt`, `sweep_t9_t13.txt`, `sweep_t14_t15.txt`. Summaries and logs are in
`scratch/qa/evidence/tN/`.

| t1 | t2 | t3 | t4 | t5 | t6 | t7 | t8 | t9 | t10 | t11 | t12 | t13 | t14 | t15 |
|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|
| pass | pass | pass | pass | pass | pass | pass | pass | pass | pass | pass | pass | pass | pass | pass |

**15/15 pass.**

### 2.4 Fire buffer at runtime (focus 3)
See §0: 0 seat shots and 0 shots on exit over 29 valid trials. The buffer is live at runtime (control 2 shots,
7/7), and getting into a car drops a queued press (1 shot, 7/7).

### 2.5 Screenshots I looked at (all in `scratch/qa/evidence/`)
- `t16_s1_run1/p2_into_wall.png`: the camera is pulled in to the player's head at the city-edge wall (boom 0.64 m).
  `p2_away.png`: full boom (3.76 m), player in frame, sky past the edge. The edge wall is **invisible**, so P2 proves
  the camera sweep against a boundary collider. It does not prove it against a building wall (see §4, Q-1).
- `t16_s1_run1/p7_run_over.png`: the car on the range, the dummies knocked aside, damage numbers 39 and 67 on hit.
- `t16_s1_run1/p9_hud.png`: health bar, ammo `12 / 36`, stars, the minimap with the search circle and dots; the
  player holds the pistol.
- `t16_s1_run1/p10_settings.png`: the "Настройки" screen with every row, "Инверсия Y выкл", the toggles and "Назад".
- `t15/dismount.png`: 2 stars, a street with traffic, a police crew next to its car.

## 3. Acceptance criteria

| Criterion | Test performed | Result |
|---|---|---|
| Headless: T2/T8/T15 bench tests green | part of `cargo test -p gta_sim` (`civilian_bench`, `police_bench`, `traffic_bench`, `city`) and `-p citygen` (`perf`): 478/0 and 32/0. Release bench numbers were not re-measured (the orchestrator scoped that out) | PASS |
| Headless: `cargo test -p gta_sim`, `-p citygen` green | 478/0, 32/0 | PASS |
| Headless: `cargo clippy` clean | workspace + sim/citygen, `-D warnings`, after the touch-rebuild | PASS |
| t16 scenario exists and passes (`--bench-scene`, diagnostics 30 s, trace, screenshots) | not re-run (orchestrator: already verified by fixer round 2, 3/3, `scratch/fix/t16_r2c/run{1,2,3}.summary.json`). I checked that the file and its summaries exist | PASS (inherited evidence) |
| §1 point → scenario/probe that exercises it (replaces the owner checklist; TASK-031) | table §5; t16_s1 ×3 + t1..t15 ×1 | PASS with caveats: P10 is flaky (2/8), and some points are covered only headless or partially (§5) |
| Every new tuning value in its §12 data file, no `const` | `fire_buffer_seconds` (weapons.ron), `overshoot_margin` ×2 (gangs.ron, escalation.ron), `bench_resolution`/`bench` (render.ron); diff has no new tuning `const` (`BOARD_GIVE_UP_S` is a QA timeout) | PASS |
| `cargo build`, clippy, `cargo test -p gta_sim` (`-p citygen`) green | §2.1 | PASS |
| Existing tests pass | §2.1 + t1..t15 15/15 | PASS |
| Orchestrator (binding): every t*.py passes N consecutive runs | t1..t15 ×1 pass (the ×N was done by the implementer and not repeated here); **t16_s1 2/3 (P10)**, 6/8 over 8 runs | **FAIL (t16_s1 P10)** |
| Orchestrator: fire buffer (a click 30 ms before the cooldown ends fires on expiry, 0.5 s early does not queue) | headless gates green (in the 478); runtime control 2 shots at a 0.20 s gap | PASS |
| Review Issue 1 (queued press fires from the seat) fixed | own flip RED→GREEN + runtime 0 seat shots / 29 trials | PASS |
| Owner checklist | replaced by TASK-031 (orchestrator decision); see §5 | referenced |

## 4. Bugs found

### B-1 — MINOR (QA instrument, TASK-031 evidence) — t16_s1 P10 is flaky: 2 failures in 8 runs
- Repro: `python tools/qa/repeat.py t16_s1 --runs 8`. In my runs P10 failed in run 3 of the first batch and in run 4
  of the second.
- Expected: P10 opens "Настройки" and flips `GameSettings.invert_y` there and back in every run.
- Actual: (a) `Entity … not found` for the settings button right after `osinput.focus()`, so the pause menu was torn
  down with no click sent; (b) the click on "Настройки" was not taken (still `PauseMenu::Main`).
- Isolation: the same sequence passes 40/40 in `probe_p10.py` (fresh and after P7, with and without the Alt tap).
  The game path works. The flake depends on context and happens inside the OS-input step. Suspect
  `osinput.focus()`, which taps Alt even when the window is already foreground (Win32 menu mode eats the next
  click). That does not explain (a). Not proven.
- Suggested fix, not applied (QA does not change code): skip the Alt tap when `GetForegroundWindow() == hwnd`,
  re-query the button immediately before the click, and on a failure record the `GameState`/`PauseMenu` and a
  screenshot, so that the next failure explains itself. Do not add a blind retry: it would hide a real
  "click not taken" bug.

### B-2 — MINOR (QA instrument) — t16_s1 crashes while printing its summary when an error contains `★`
- Where: `tools/qa/scenarios/t16_s1.py:464`, `print(json.dumps(summary, indent=2, ensure_ascii=False))` on a cp1251
  console.
- Effect: a failing run ends in `UnicodeEncodeError` instead of `AssertionError: failed phases [...]`, so
  `repeat.py` reports the encoding error and hides the real failure. `summary.json` still holds the truth.
- Fix: `ensure_ascii=True` for stdout, or `sys.stdout.reconfigure(encoding="utf-8")`.

### Q-1 — note for TASK-031 (not a bug) — §1 evidence that is weaker than its label
- §1.2 "камера не проходит сквозь стены": P2 uses the **invisible** city-edge wall. It proves the boom collides with
  a World collider, but no building wall is exercised at runtime.
- §1.6 "со 2-й звезды стреляет": no runtime scenario asserts that cops fire at 2★. It is covered headless (the
  police fire-line and police tests) and seen in the t15 chase screenshots.
- §1.8 "уступают на перекрёстках": P8 accepts any `waiting` car that later moves to another segment, which also
  counts queueing behind a car (the review nit, confirmed from the code).
- §1.1 "в паузе нажать «Новый город»": t12 confirms the seed field with Enter, not with a click on the button.
- §1.11: FPS was measured on the bench scene with a ghost car on rails (a named cheat) and 0-2 gang members near the
  centre. §11's 12 gang members were never reached (Q3, documented).

### Observation — unexplained, not reproduced
During my own OS-input probe the game window closed once (`No windows are open, exiting`, no error). It did not
happen on the rerun. It is recorded so that TASK-031 knows about it if it recurs.

## 5. §1 evidence table (for TASK-031)

`E` = `D:/test-gta-like/maw/tasks/in_progress/TASK-017/scratch/qa/evidence`. Results are from this QA unless marked.

| §1 | Scenario / probe that exercises it | Asserted | Result (this QA) | Evidence |
|---|---|---|---|---|
| 1 loading screen, random seed or `--seed`, "Новый город" from pause | t16_s1 P1 (menu → Enter, clock seed → Loading → Playing, 1 player); t2 (`--seed`, golden hash); t12 (pause, typed seed + Enter → new city, no leftovers) | seed ≠ 1 / non-golden hash; golden hash; a new `CitySeed` with no old-city leftovers | P1 3/3 (8/8); t2, t12 pass | `E/t16_s1_run1/p1_menu.png`, `p1_loading.png`, `p1_city.png`, `summary.json`; `E/t2/`, `E/t12/pause.png` |
| 2 walk/run/sprint/jump, camera vs walls | t4 (AnimState W / Shift / Alt / Space), t1, t3 (roof); t16_s1 P2 | anim states; boom 0.64 m < 3.3 at the wall, 3.76 m away | t4, t1, t3 pass; P2 3/3 (8/8) | `E/t4/jump_04.png`, `sprint_03.png`; `E/t16_s1_run1/p2_into_wall.png`, `p2_away.png` (invisible edge wall, Q-1) |
| 3 pickups, drops, pistol/SMG/shotgun/bat, aim, reload, fists | t6 (pistol, SMG, reload, shotgun), t7 (fist combo, bat); t16_s1 P3 (a killed member's dropped gun picked up) | dummy HP, ammo, crit; KnockedDown; owned/reserve | t6, t7 pass; P3 3/3 (8/8) | `E/t6/crit_hit.png`, `E/t7/knockdown.png`, `E/t16_s1_run1/p3_pickup.png` |
| 4 civilians walk, flee from shots, call police (indicator) | t8 (populated street, a shot scatters them), t10 (caller indicator → stars) | Flee+Cower share rises; report → stars | t8, t10 pass | `E/t8/crowd.png`, `scatter.png`, `E/t10/hud_wanted.png` |
| 5 gang threat, group fire | t16_s1 P5 (a member in `Warn` within warn_seconds + 1 s); t9 (a shot → group `Attack`, firefight, crossfire accounting) | Warn state; Attack + gang heat, no unexplained friendly fire | P5 3/3 (8/8); t9 pass (×20 by the implementer, not repeated) | `E/t16_s1_run1/p5_warn.png`, `E/t9/firefight.png` |
| 6 wanted 1-5, cops on foot and in cars, arrest at 1★, fire from 2★, SWAT at 4-5★, lose the search | t10 (stars, clear outside the search circle), t11 (1★ arrest/BUSTED, 4★ SWAT), t15 (police cars, dismount), t16 (5★ composition) | stars and clear; arrest and SWAT; dismount; 5 cars / 12 units | t10, t11, t15 pass; t16 3/3 (fixer r2) | `E/t11/busted.png`, `swat.png`, `E/t15/dismount.png`, `scratch/fix/t16_r2c/*.json`. 2★ fire is headless-only (Q-1) |
| 7 parked car, hijack (driver flees), drive, run someone over, exit | t14 (enter, drive, crash, exit), t15 (hijack, driver flees); t16_s1 P7 (run over) | `Driving`, speed, car health, exit; fleeing driver; dummy KnockedDown | t14, t15 pass; P7 3/3 (8/8) | `E/t14/drive_3.png`, `exit.png`, `E/t15/hijack.png`, `E/t16_s1_run1/p7_run_over.png` |
| 8 traffic on lanes, yielding at junctions, car chase with police cars and dismounting cops | t15 (≥ 12 cars, speeds, chase, dismount); t16_s1 P8 (a waiting car moves on) | counts and speeds; chase samples; waiting → moved | t15 pass; P8 3/3 (8/8) | `E/t15/street.png`, `chase_9.png`, `E/t16_s1_run1/p8_traffic.png` (P8 label, Q-1) |
| 9 Wasted → hospital with gear; Busted → station without guns | t5 (Wasted → hospital), t16_s1 P9 (pistol, magazine and reserve kept, < 1 m from the hospital), t11 (Busted → station, guns confiscated) | as stated | t5, t11 pass; P9 3/3 (8/8) | `E/t5/wasted.png`, `respawned.png`, `E/t16_s1_run1/p9_respawn.png`, `E/t11/busted.png` |
| 10 HUD (health, armour, weapon/ammo, stars, minimap), pause, settings | HUD: t5/t6/t12/t14 + P9 screenshot; pause: t12, t13; settings: t16_s1 P10 (OS clicks flip "Инверсия Y") | minimap dots per cop (t12); `invert_y` flips there and back | t12, t13 pass; **P10 2/3 (6/8), B-1** | `E/t16_s1_run1/p9_hud.png`, `p10_settings.png`, `E/t12/minimap_wanted.png` |
| 11 ≥ 60 FPS at 1080p in the §11 worst scene | t16 (`--bench-scene`, sessions A/B, trace) | composition gate + moving precondition; FPS never pass/fail | 3/3 (fixer r2, not re-run): mean 289-347 FPS, p99 ≤ 5.7 ms, FixedMain ≤ 2.8 ms/tick, "nothing to fix by trace" | `scratch/fix/t16_r2c/run{1,2,3}.summary.json`, `FIX_SUMMARY.md` §3.2 |

Owner checklist: replaced by the **TASK-031** agent playtest (orchestrator decision, `TASK_FINAL.md`). This table is
its input, together with the caveats in Q-1. Feel items for TASK-031, which a gate cannot judge: how pistol clicks at
about 3/s feel with the 0.15 s buffer; gang crossfire past the 8.865 m zone (R95off 11 %, accepted in
OPEN_DECISIONS); the queued gang shotgun shot, which skips the fire-line re-check (review Issue 4, accepted as R4).

## 6. Verdict

**NEEDS_FIXES (minor, QA instrument only).**

Every gameplay criterion passes: build, clippy, 478 + 32 + 81 tests, my own flip-RED on the review's main
finding, t1..t15 15/15 at runtime, and the fire buffer at runtime with 0 shots from the seat and 0 on exit over 29
real-input trials. The binding orchestrator rule "every t*.py must pass N consecutive runs" fails for the new
`t16_s1`: P10 (settings through real OS clicks) failed in 1 of the 3 requested runs and 2 of 8 overall. I could not
reproduce it in 40 isolated cycles, so the game path looks sound and the suspect is the OS-input helper, but that is
not proven. B-2 hides such failures behind a print crash. Both fixes are in the harness and small, and no game code
needs to change. If the orchestrator accepts P10 as a known OS-input flake for TASK-031 to watch, the rest supports
SHIP.

children: 0 launched / 0 reported.
