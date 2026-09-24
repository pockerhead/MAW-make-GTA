# TASK-026 IMPL_SUMMARY: witnesses of a kill really call

Cost of error: a silent one (a kill that never raises wanted reads as a bug only after many plays), so a
city-level headless gate with flip-RED, plus the runtime probe.

## 1. What was implemented

Numbers from `git diff --stat` (+/-):

- `assets/npc/civilian.ron` (+2/-1): new `call_after_flee: 0.6`; `reaction.report` 0.8 -> 1.0.
- `crates/gta_sim/src/civilian/mod.rs` (+70/-15, 508 lines now):
  - `CivilianConfig.call_after_flee` (validated to [0, 1]) and `CivilianConfig::longest_call_delay`.
  - `CivilianState::Flee` / `Cower` carry `about: Option<Cause>`, the crime this civilian can phone in later.
  - `witnessed(threat)`: Gunshot / Fight / Corpse give their cause; Aimed / Hurt / Car give `None`.
  - Flee then call: a flight that ends (`left <= 0`) or a crouch that ends (`cower_seconds`) with `about`
    set rolls `call_after_flee` once (`call_later`) and starts `Report { about }`. On a miss the flight
    goes to Wander and the crouch goes to a flight as before (`about: None`). The roll draws from `NpcRng`
    only when `about` is set, so a calm city draws exactly as before.
  - `already_fleeing`: a flight or crouch that sees the same crime again (a body still in sight) no longer
    restarts. Without it, the corpse (offered to the witness every perception cycle) restarted the crouch
    every 4 ticks, so a cowering witness stayed down until the corpse despawned at 30 s and never got to
    "cower_seconds elapsing". A new cause (a new shot, a body after a shot) still refreshes, as before.
  - One heat per incident is not touched: calls still go through `take_calls` -> `Crimes::report`.
- `crates/gta_sim/src/lib.rs` (+3/-2): the `incident_memory_seconds` check uses the new delay bound
  (perception + full crouch + full flight at `flee_gait` speed + call = 23.4 s shipped; memory 60 s).
- `crates/gta_sim/src/wanted/mod.rs` (+1/-1): error text names the new bound.
- `crates/gta_sim/src/civilian/reaction.rs` (+27): gate `report_band_calls_half_the_time`.
- Tests: new `crates/gta_sim/tests/witness_city.rs` (227 lines); `wanted.rs` (+59/-7), `wanted_support/mod.rs`
  (+12, `longest_call_ticks`), `config.rs` (+26/-6); the `about` field added to 3 literal state sites
  (`tests/civilian_city.rs`, `src/hud/witness_gate.rs`, `src/visuals/civilian_gate.rs`, +1 each).

### Report weight math (design item 2)

Past `panic_distance` (15 m; the report band starts at 25 m) the cower weight is 0. Report wins iff
`report * R > flee * F` with R, F iid uniform on [1 - s, 1 + s], s = 0.5. With `report = flee` the event
`R > F` has probability exactly 1/2 by symmetry (ties have measure 0 and go to Flee). The old 0.8 gave
P(F < 0.8 R) = integral over R in [0.625, 1.5] of (0.8 R - 0.5) dR = 0.74375 - 0.4375 = 0.306. So
`report: 1.0`. Gate `reaction::tests::report_band_calls_half_the_time`: 20 000 rolls through the real
`roll_temperament` + `choose_reaction`, share in [0.47, 0.53].

## 2. Deviations / not implemented

- `already_fleeing` (same-crime re-perception does not restart the flight/crouch) is not in the task
  text; it is what makes "after cower_seconds elapse" reachable while the body is in view. Logged as a
  decision in `log.jsonl`.
- A cowerer whose roll misses flees with `about: None`; if the body is still in sight, the new flight
  picks `about = Body` and rolls again at its end (two chances, 0.84 total). Accepted, logged.
- A fleer that is physically stuck (jammed head-on with another civilian) never ends its flight, so it
  never calls. Pre-existing locomotion behaviour, not touched. Noted in `PCTX_PROPOSALS.md`.
- GDD §6.2 text about witnesses is not updated (design docs are not the implementer's artifact).

## 3. Test results

- `cargo test -p gta_sim -p citygen -j 4`: all green (every target `ok`, incl. `wanted` 13, `civilians` 10,
  `config` 47, lib 67, `witness_city` 1 + 1 ignored flip).
- `cargo test -p gta_like --bin gta_like -j 4`, 3 runs: 77 passed each time.
- `cargo clippy -j 4 -- -D warnings`, `cargo clippy --workspace --all-targets -j 4 -- -D warnings`,
  `cargo clippy -p gta_sim -p citygen --all-targets -j 4 -- -D warnings`: clean.

### Gate 1: street kill gets a star (`witness_city.rs::street_kill_gets_a_star`, correctness)

Seed-1 city, production composition and population, 20 NPC RNG seeds (1000..1019), 6 s of fill. The
victim is the calm civilian with the FEWEST (>= 3) calm others within 40 m (a crowd calls under any rule:
picking the most crowded victim gave 20/20 at 4.0 s under the old rules too, 16-22 witnesses). Player
teleported 6 m away along the victim's sidewalk, clear line, one pistol kill, no cop exists at the shot;
the star must come from a civilian `PoliceCall`.

- New rules: **18/20** stars within 15 s. Times (s): 4.0 x5, 4.02 x5, 4.03 x3, 4.05, 10.91, 11.20, 11.67,
  13.0. The 4 late ones are calls by witnesses who fled or crouched first. Deterministic: 3 runs gave the
  same list. The 2 misses (seeds 1000, 1019, from a temporary diagnostic print, since removed): of the 3
  others within 40 m of the victim only 2 were within 45 m of the muzzle and both fled. Seed 1000: one was
  mid-call at 15 s (progress 0.48, would land at ~17 s), the other's flight restarted when it saw the body
  (new cause) and was still running. Seed 1019: both ran past 80 m without a call in the window.
- Flip-RED (`street_kill_under_old_rules_misses`, `#[ignore]`, run by hand with `--include-ignored`):
  named mutation `call_after_flee = 0`, `reaction.report = 0.8` -> **11/20** (< 17, the flip test passes,
  i.e. the main gate would be RED). Perturbed input: those two config values.
- Margin note: 18 vs the 17 floor. The gate is deterministic, but any change to NPC RNG consumption
  reshuffles the seeds; a drop to 16 would then be a real signal to look at, not noise.

### Gate 2: no witness -> 0 heat (`wanted.rs::unwitnessed_kill_is_zero_heat`)

Existing gate, now runs `longest_call_ticks + 16` (1514 ticks, was 528) so any delayed call would land
inside the window. Green: heat 0, no calls.

### Gate 3: one corpse, several delayed calls -> heat once (`wanted.rs::delayed_calls_about_one_kill_count_once`)

Test floor; 3 flee-prone witnesses on separate dead-end runs 14-17 m east of the body (the graph_app(10)
square jams fleers at the (10,0,10) test box), named mutation `call_after_flee = 1.0`. Asserts each
witness is in `Flee { about: Body(victim) }` 5 ticks after the kill, then 3 delayed calls, all from the
witnesses, and heat == 40 (+10 only if the shooting incident was reported). Green: 3 `Body` calls, heat 40.
Flip-RED: removed the `if incident.reported { return None; }` guard in `Crimes::report` -> heat 120, RED;
restored (backup `scratch/crimes.rs.orig`).

### Config gates (`config.rs`)

- `call_after_flee_is_a_chance`: 1.5 is rejected naming the field.
- `incident_memory_must_outlast_a_civilian_call`: the bound is now 23.396 s; the sabotage memory is 20 s,
  which the old 4.06 s bound would have accepted.

### Runtime (release `--features dev`, QA settings id through `tools/qa/brp.py`)

- `python tools/qa/scenarios/t10.py --out scratch/qa_t10`: exit 0, star 3.86 s after the kill, heat 80,
  log clean.
- `python tools/qa/scenarios/t11.py --out scratch/qa_t11`: exit 0, log clean.
- Probe `scratch/probe_street_kill.py` (kill on a busy street, no cops, only mutation: victim idle at 1 HP;
  no temperament touched): 3 kills, 3 stars at 3.94 / 4.00 / 3.97 s, 4 / 28 / 17 others within 40 m, 0 cops
  at every shot, heat 50 each (kill + shooting), log clean. Output `scratch/probe_street_kill/summary.json`.
- No game process left running.

## 4. How to verify manually

1. `cargo test -p gta_sim --test witness_city -j 4 -- --nocapture` prints per-seed time to star;
   add `--include-ignored` for the old-rules flip (expects < 17).
2. `cargo test -p gta_sim --test wanted -j 4 -- delayed unwitnessed`.
3. Owner run: `cargo run --features fast`, shoot a civilian on a street with people around, no cops; a
   star should appear in about 4 s (a direct caller) or 7-15 s (someone who ran first). Watch the flee then
   phone animation on witnesses that ran: they stop and show the call bar after the flight.
