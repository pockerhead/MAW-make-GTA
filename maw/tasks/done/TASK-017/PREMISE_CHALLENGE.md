# PREMISE_CHALLENGE — TASK-017 (GDD T16)

## 1. Counter-example tested

The carried note attributes the t13 flake to "one OS mouse click lost in a 6-click burst" (an
environment cause, fix = make the scenario deterministic). Counter-example: if the game reads the
fire trigger as an edge (`just_pressed`) sampled once per frame / consumed in `FixedUpdate`, or the
scenario spaces clicks closer than a frame / the pistol cooldown, then a click is lost by the GAME's
input path or the script's timing, not by the OS — and "make t*.py pass N runs" could be met by
slowing the script while a real dropped-shot defect stays in the game.

(Investigation below; written before reading any code.)

## 2. Primary-source investigation

Code:
- `tools/qa/scenarios/t13.py:35-36` — `SHOT_GAP_S = 0.35`, `CLICK_MS = 80`; burst loop `t13.py:278-291`
  (screenshot after click 3, then `mag_before - mag_after != 6` -> `GATE BROKEN`).
- `assets/combat/weapons.ron:3` — pistol `fire_mode: SemiAutomatic, fire_interval: 0.3`.
- `crates/gta_sim/src/combat/hitscan.rs:187-188` — "A request during cooldown or reload is dropped, not
  buffered." `fire_requested` is `mem::take`n every tick; `:201` skips when `slot.cooldown > 0.0`; `:215` sets
  `cooldown = fire_interval`.
- `crates/gta_sim/src/combat/weapons.rs:346` — `cooldown -= dt` in `tick_loadouts`, which runs in
  `FixedUpdate` chained before `fire_weapons` (`combat/mod.rs:91-105`). At 64 Hz (GDD §11, dt = 15.625 ms)
  0.3 - 19*dt = 0.0031 > 0, so the next shot is possible only 20 ticks = 0.3125 s later. Nominal script
  margin: 0.35 - 0.3125 = 37.5 ms, about one frame on this host's 30 Hz display (gates domain, TASK-010).
- `~/.cargo/registry/src/*/bevy_brp_extras-0.22.6/src/mouse/support.rs:122-124`, `button.rs:116-125` — the
  click is injected as a Bevy `MouseButtonInput` message, not an OS event. There is no OS input path to lose.
- `src/input/mod.rs:279-281` — `fire_requested = true` on `ActionEvents::START`, read in Update, consumed by
  the fixed tick (so arrival is quantised to frames and ticks).

Executable: release `--features dev` build (`cargo build -p gta_like --bin gta_like -j 2 --features dev
--release`), then an instrumented copy of t13 (`scratch/premise/t13_probe.py`, only change: logs send
times of each click relative to burst start and the screenshot latency), 3 runs:

```
run1 exit 0  PROBE_BURST [(0.0,0.0),(0.359,0.359),(0.703,0.734),('png_latency',0.047),(1.063,1.078),(1.406,1.406),(1.766,1.766)]  mag 12->6
run2 exit 1  PROBE_BURST [(0.0,0.016),(0.359,0.359),(0.719,0.75),('png_latency',0.079),(1.047,1.047),(1.422,1.437),(1.766,1.766)]  mag 12->7
             GATE BROKEN: six clicks did not fire six shots: {'shot_delta': 5, 'impact_delta': 6, 'magazine': (12, 7), ...}
run3 exit 0  PROBE_BURST [(0.0,0.0),(0.36,0.36),(0.704,0.704),('png_latency',0.047),(1.047,1.047),(1.407,1.407),(1.766,1.766)]  mag 12->6
```
(raw: `scratch/premise/run{1,2,3}.txt`)

My first sub-hypothesis (the screenshot inside the burst delays click 4 into click 3's cooldown) is
refuted: screenshot latency is 47-79 ms. The flake still reproduced 1/3 (12->7, same signature as TASK-015
fixer3 `t13.txt`/`t13b.txt`). In the failing run the tightest send gap was 0.328 s (0.719 -> 1.047), in the
passing runs 0.343-0.344 s; all six RPCs returned, so all six presses were delivered to the game.

Other carried notes and scope, checked at source:
- `--bench-scene` does not exist anywhere in `src/`, `crates/`, `tools/` (grep): the §13 goal is real work.
  Existing headless benches: `crates/gta_sim/tests/{civilian,police,traffic}_bench.rs`. GDD §11
  (`docs/design/GDD.md:507-521`) makes FPS a measurement on the owner's machine, "no automatic FPS pass/fail",
  and occlusion culling "only after measurement" — consistent with the task's "fix only by trace". With the
  low measured costs the caller quoted, "fix by trace" may legitimately find nothing; that is allowed by §11.
- Go-around note is stale: `maw/tasks/pending/TASK-032/task.md:1-19` already owns the traffic go-around
  (with its own gates and a redesign away from the oncoming-lane reservation the TASK-017 note describes).
  It is no longer a T16 owner-run judgement item.
- Owner-run AC (§1 checklist "на своей машине"): `maw/tasks/pending/TASK-031/task.md:10,22` records the owner
  replacing the owner run with an agent playtest that rates every §1 point. The TASK-017 AC still asks for an
  owner checklist; it is satisfiable (write the checklist) but no one will run it before TASK-031.
- Throughput (~1 car / 4 s) not measured by me; left unverified.

## 3. Did it hold

Partly. The T16 goal itself (bench scene, measure, fix only by trace, §1 checklist, bug bash) matches
GDD §11/§13 and the bench scene is genuinely missing. But the carried t13 root cause is wrong: the click is
not an OS event and is not lost — the game receives it and its own semi-auto rule drops a press that lands
inside a 20-tick (0.3125 s) cooldown, and the scenario leaves only ~37 ms (≈ one 30 Hz frame) of margin.
The failing run had the tightest measured gap. "Make t13 pass N runs" can therefore be met by changing the
scenario's timing while the game-side behaviour (a player clicking at ~3/s silently loses shots) stays
exactly as is; whether that behaviour is intended is a design question the note hides by calling it an OS
loss. The go-around note is superseded by TASK-032.

## 4. Verdict

PREMISE SUSPECT — t13 "OS mouse click lost" is contradicted by `bevy_brp_extras-0.22.6/src/mouse/support.rs:122-124` (Bevy-message injection, no OS path) + `crates/gta_sim/src/combat/hitscan.rs:187-188,201` (in-cooldown press dropped by design) + `weapons.rs:346`/`combat/mod.rs:91-105` (effective cooldown 20 ticks = 0.3125 s vs `t13.py:35` gap 0.35 s), reproduced 1/3 in `scratch/premise/run2.txt` with all six presses delivered and the tightest gap 0.328 s; also the go-around note is superseded by `maw/tasks/pending/TASK-032/task.md:10-19` ; smallest implied reframing: the t13 flake is the game's own semi-auto cooldown dropping a press at a ~1-frame margin (a script-timing vs input-buffering decision to make explicitly), not an OS event loss, and go-around leaves T16's carried notes for TASK-032, while the rest of T16 (bench scene, measure, fix only by trace, bug bash) stands.
