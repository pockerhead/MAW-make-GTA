# TASK-026 QA_REPORT (qa, claude opus, medium)

Commit under test: `7ac693b` (branch `fix/witness-calls`), working tree clean before QA.

## Preflight

- scratch/ read as a coverage map: implementer probe `probe_street_kill.py` (it picks the MOST crowded victim and
  needs a caller in the 27-36 m band, i.e. the easy case), backups, t10/t11 runs from before the 0.8 retune. Not
  reused as evidence: I wrote my own probe `scratch/qa/qa_probe.py`.
- Read: task.md, IMPL_SUMMARY.md, IMPL_REVIEW.md, FIX_SUMMARY.md, OPEN_DECISIONS.md, PCTX_PROPOSALS.md, log.jsonl.
- log.jsonl has no `dead_end` entries; the 4 `decision` refs were checked in code (`already_fleeing`, `pick_victim`
  sparsest-first, cower-miss re-arm via the Flee refresh arm, `call_after_flee: 0.8` in `assets/npc/civilian.ron`).

## Disconfirmation

Counter-example I went after: **a delayed call lands after Busted/Wasted and puts a star on a player who was just
reset.** The call delay grew from ~4 s to up to 53.4 s after the crime, so a witness still running when the player
is busted would phone in afterwards. Checked `wanted/mod.rs:225-243`: `reset_wanted` runs on `OnEnter(Wasted)`,
`OnExit(Busted)` and new city and does `crimes.clear()`; `take_calls` resolves `call.about` against `Crimes`, so a
late call about a cleared crime resolves to nothing and adds 0. Queued `PoliceCall` messages are cleared too.
**Did not hold.** At runtime my first probe run was in fact busted 20 s into a wave with calls still pending; no
heat came back after respawn.

Second candidate: the Flee refresh arm re-rolling `left` forever while a body is in view (only filtered when
`about` is already set). A cower-miss flight starts with `about: None`, the first body sighting sets `about = Body`,
then `already_fleeing` filters. One extra refresh, not a loop. Holds.

## 1. Environment

Direct, no docker. Windows host, cargo `-j 4`, shared `target/`.

- Headless: `cargo test -p gta_sim -p citygen -j 4`, `cargo test -p gta_like --bin gta_like -j 4` (x3),
  `cargo clippy --workspace --all-targets -j 4 -- -D warnings`, `cargo clippy -p gta_sim -p citygen --all-targets -j 4 -- -D warnings`.
- Runtime: release `--features dev` via `tools/qa/brp.py` `Game` (always `--settings-id com.github.pockerhead.maw-make-gta.qa`), seed 1.
  - `python tools/qa/scenarios/t10.py --out maw/tasks/in_progress/TASK-026/scratch/qa/t10`
  - `python tools/qa/scenarios/t11.py --out maw/tasks/in_progress/TASK-026/scratch/qa/t11`
  - `python maw/tasks/in_progress/TASK-026/scratch/qa/qa_probe.py --out maw/tasks/in_progress/TASK-026/scratch/qa/probe`
- No owner `gta_like.exe` was running at any point; every game I launched shut down via BRP; `tasklist` empty after.

## 2. Test results

### Existing suite
- `cargo test -p gta_sim -p citygen -j 4`: exit 0, every target `ok`, 0 failures (wanted 15, config 48, civilians 10,
  civilian_city 7, lib 67, witness_city 1 + 1 ignored, ...). Output: scratchpad `sim_tests.txt`.
- `cargo test -p gta_like --bin gta_like -j 4`, 3 runs: 77 passed / 0 failed each time.
- clippy (both commands above): clean.
- No failure list to compare against base: zero failures on HEAD.

### City gate, reproduced
`cargo test -p gta_sim --test witness_city -j 4 -- --include-ignored --nocapture`: 2 passed in 68 s.
New rules **94/100** (misses 1033 1034 1054 1059 1074 1081, all with 3-5 others), same list as FIX_SUMMARY.
Time to star: 64 kills at 4.0-4.05 s (direct callers), 30 kills at 7.2-14.7 s; 28 first calls from a witness who fled
or cowered first. Old-rules flip test passes (old rules < 85).

### My own flip (independent of the author's)
`assets/npc/civilian.ron` `call_after_flee: 0.0` only (report weight left at 1.0): main gate **RED, 66/100**.
Restored with `git checkout`, sha256 verified. So flee-then-call carries ~28 points of the 94, the weight change the rest.

### Runtime
- **t10**: exit 0, one star from a civilian call, clears, heat mutations 2 stars / 0 stars, log clean.
  `scratch/qa/t10/hud_wanted.png`: one lit star top right, minimap search circle; looks right.
- **t11**: exit 0, log clean; frame cost no-vsync 2.6-2.9 ms (display 30 Hz Fifo gives 30 FPS, as known).
- **Street-kill probe** (`scratch/qa/probe/summary.json`), 6 pistol kills, no cop within 150 m at any shot
  (one far cop existed at kill 6), victims alternating sparsest (3-4 others within 40 m) and busiest (7-13):

| # | street | others 40 m | time to star | Report bars | calls done | delayed callers (after Flee/Cower) | heat increments |
|---|---|---|---|---|---|---|---|
| 1 | sparse | 3 | 3.73 s | 2 | 2 | 0 | +50 once |
| 2 | busy | 13 | 3.75 s | 13 | 13 | 7 | +50 once |
| 3 | sparse | 4 | 3.74 s | 3 | 3 | 0 | +50 once |
| 4 | busy | 13 | 3.73 s | 11 | 11 | 5 | +50 once |
| 5 | sparse | 3 | 3.73 s | 3 | 3 | 1 | +50 once |
| 6 | busy | 7 | 3.73 s | 6 | 6 | 3 | +50 once |

  Heat-once method: at the first star the probe resets heat to 0 (keeps cops from busting the player mid-wave),
  then keeps watching 30 s; any later report of the same kill would show as a new +40. Across 6 kills, 38 completed
  calls, the only increment was the first +50 (kill 40 + shooting 10). An earlier run without the reset had
  3 bars / 3 calls / +50 once, then Busted at 20 s, with no heat back after respawn.
- **Unwitnessed kill**: victim moved to a spot 78 m from the nearest civilian, no cop, one kill, watched 60 s
  (> the 53.4 s call bound): heat 0, no Report, no flight. Log clean.

## 3. Acceptance criteria

| Criterion | Test performed | Result |
|---|---|---|
| City gate >= 17/20 (now 85/100) within 15 s, distribution, flip-RED | Reran witness_city: 94/100, distribution above; author flip 53/100; my flip (`call_after_flee` 0 only) 66/100 RED | PASS |
| No witness, no cop -> 0 heat | `wanted.rs::unwitnessed_kill_is_zero_heat` green (window now 3434 ticks); runtime lonely kill: 0 heat over 60 s | PASS |
| One corpse -> heat once with several delayed calls | `delayed_calls_about_one_kill_count_once` + `cowering_witness_calls_once_the_crouch_ends` green; runtime: 38 calls over 6 kills, one +50 each | PASS |
| Runtime t10/t11 pass; probe reports time to star | t10 exit 0, t11 exit 0; probe 6/6 stars at 3.73-3.75 s | PASS |
| clippy -D warnings, sim suite, client gates | all green, client gates 3x 77/77 | PASS |

## 4. Bugs found

None blocking. Notes:

1. (low, observation) In the runtime probe every star came from a direct caller at ~3.7 s; the delayed path
   produced calls (16 delayed callers) but never the first one. The headless gate is where the delayed path decides
   the outcome (30/94 stars). Runtime does not contradict it, it just picks easier streets.
2. (low, pre-existing, noted by the implementer) A fleer that gets physically stuck never ends its flight and never
   calls. Not touched by this task.
3. (info) The unwitnessed runtime case did not exercise a passer-by finding the body later (the nearest civilian came
   to ~15 m of the pre-shot spot but did not react; the body may have ended up out of sight). By design such a
   discovery would be phoned in (body calls exist since TASK-011).

## 5. Verdict

**SHIP-PENDING-RUNTIME.** Every covered criterion passes headless and at runtime; the remaining part is how the
flee-then-call wave reads on screen, which is the owner's call.

### Чек-лист для владельца
1. `cargo run --features fast`, взять пистолет, выйти на улицу, где рядом 3+ прохожих и нет копов.
2. Убить прохожего. Ожидание: звезда примерно через 4 с (кто-то сразу звонит, над ним полоска звонка).
3. На людной улице посмотреть на волну: часть свидетелей сначала убегает или приседает, потом останавливается и
   звонит (полоска звонка появляется уже после бега). Выглядит ли это естественно или как "толпа телефонистов"?
   В пробе на людной улице звонили 6-13 человек из 7-13.
4. Убить прохожего в пустом месте (никого в 40 м): звезды быть не должно, минуту подождать.
5. После ареста/смерти не должно быть звезды "из прошлого" от поздних звонков.

children: 0 launched / 0 reported
