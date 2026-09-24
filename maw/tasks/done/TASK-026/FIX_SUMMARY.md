# TASK-026 FIX_SUMMARY (fixer, claude opus, medium)

Preflight: scratch/ listed (implementer probes, backups, QA runs; used only as a coverage map). Review read.
The review claim most likely to break correct code if applied verbatim: the missing-coverage recipe "remove the
`already_fleeing` filter -> the crouch refreshes every 4 ticks and no call comes before the corpse despawns".
Checked in `perception/mod.rs::perceive`: a corpse within `corpse_sight` is offered to every non-reporting
civilian on each perception slot, and the `(Cower, Some)` arm re-rolls `left`. The claim is real, and the flip below
confirmed it (RED, witness still cowering with `left` 5.15 s after 648 ticks).

## 1. Fixed

- **I-1 (gate margin)**: `witness_city.rs` now runs 100 seeds (1000..1099) with a floor of 85/100.
  Measured on the same 100 seeds (each run sequential, `--nocapture`):

  | `call_after_flee` | stars within 15 s |
  |---|---|
  | 0.6 (implementer) | 87/100 (misses 1000 1019 1027 1031 1033 1034 1046 1054 1059 1061 1074 1081 1097) |
  | 0.7 | 91/100 |
  | **0.8 (shipped now)** | **94/100** (misses 1033 1034 1054 1059 1074 1081) |
  | 1.0 | 95/100 |
  | old rules (`call_after_flee` 0, `report` 0.8) | 53/100 |

  0.6 was below 0.9, so per the orchestrator note I raised `assets/npc/civilian.ron` `call_after_flee` to 0.8
  (the lowest measured value with margin; 0.7 at 91 sits too close to the floor). The threshold was not lowered.
  At 0.8 the time to star is 4.0-4.05 s for 64 kills (direct callers) and 7.2-14.7 s for 30 (28 first calls
  came from a witness who fled or cowered first). Flip `street_kill_under_old_rules_misses` (`#[ignore]`,
  run with `--include-ignored`): 53 < 85, so the main gate would be RED under the old rules. Decision logged.
  Also updated the config anchor `call_after_flee_is_a_chance` (0.6 -> 0.8 source string) and the number in
  GDD §6.2 (`(0.6)` -> `(0.8)`), so the doc matches the data. **Orchestrator: the GDD edit is one number; revert
  it if you want to own that line.**
- **I-2 (call-delay bound from the crime)**: confirmed. `forget` drops an incident `incident_memory_seconds`
  after its `last` (kill time), while a body is a stimulus until `corpse_seconds` (30 s).
  `CivilianConfig::longest_call_delay` now takes `corpse_seconds` and returns the bound from the crime
  (30 + 0.0625 + 6 + 13.33 + 4 = 53.40 s shipped, memory 60 s, still valid). `compose_sim` passes
  `population.corpse_seconds`; the wanted error text names it. `wanted_support::longest_call_ticks` uses the
  same bound (the unwitnessed-kill window grows to 3418 + 16 ticks, test still ~3 s).
  - `config.rs::incident_memory_must_outlast_a_civilian_call` re-anchored: expected 53.40 s; the fixture memory
    is 40 s, and the test asserts (as a GATE BROKEN precondition) that the old per-stimulus bound (23.4 s) accepts
    40 s while the new one rejects it.
  - New `config.rs::compose_counts_the_call_delay_from_the_crime`: copies every shipped `.ron` to a temp
    root, sets `incident_memory_seconds: 40.0`, and asserts `compose_sim` fails on `wanted.ron` naming the field.
    **Flip-RED**: `lib.rs` passing `0.0` instead of `population.corpse_seconds` -> the test FAILED
    (compose accepted 40 s); restored -> GREEN.
  - The optional flee-speed margin (a slowed fleer) is not added; see Skipped.
- **I-3**: `pick_victim` doc now says `(victim, player chest, others within radius)`.
- **I-4**: format string fixed (`{fled}; fewest others ...`).
- **Nit, TICK_HZ**: removed; ticks come from `ticks_in(&app, s)` and time to star from `Time<Fixed>::timestep()`.
- **Nit, "Upper bound" wording**: covered by I-2 (the doc now says "from a crime").
- **Missing coverage 1, cowered at a corpse then called** (`wanted.rs::cowering_witness_calls_once_the_crouch_ends`,
  correctness): `graph_app(10)`, cow-prone witness (flee 0.2, cower 1.5, report 0.1) 12 m from the victim, within
  `panic_distance` and corpse sight; named mutation `call_after_flee = 1.0`. Precondition: after 2 perception
  cycles the witness is in `Cower { about: Body(victim) }`. Then, with the body in view all the time, it must stay in
  Cower/Report (never flee) and exactly one call `(witness, Body(victim))` must arrive within
  `cower_seconds.1 + call_seconds` + 2 cycles (648 ticks, asserted < `corpse_seconds`); heat 40.
  **Flip-RED**: removed the `threat.filter(already_fleeing)` line -> FAILED "no call within 648 ticks: Cower { left:
  5.15, about: Body }"; restored -> GREEN.
- **Missing coverage 2, flight from an aimed gun is never phoned in**
  (`wanted.rs::flight_from_an_aimed_gun_is_never_phoned_in`, correctness): flee-prone civilian 10 m away, player
  aims at it with a pistol, `call_after_flee = 1.0`. Precondition: it flees. Aim is released; until the flight ends
  (calm again, bounded by `longest_call_ticks`) it never enters `Report`, no `PoliceCall`, heat 0.
  **Flip-RED**: `witnessed` maps `Aimed` to `Some(Cause::Attack(u32::MAX))` -> FAILED "a flight from an aimed
  gun started a call"; restored -> GREEN. (The first version asserted `about: None` in the precondition, so the
  flip hit GATE BROKEN instead of the rule. I loosened the precondition to `Flee { .. }` and re-ran the flip.)

## 2. Skipped

- **Car flight gate**: not added. `Car` goes through the same `witnessed -> None` arm as `Aimed`, and perception
  offers it with `cause: None` too. A headless car on a sidewalk needs vehicle fixtures and adds no new mechanism
  to cover.
- **"Delayed call interrupted by a new gunshot"** (review marked it low priority): the behaviour comes from
  `react(.., false)` from before this task. It is not part of TASK-026 acceptance.
- **Flee-speed margin in the bound (I-2, optional)**: `left` counts measured metres, so a blocked fleer never
  ends its flight and never calls (the implementer noted this in PCTX). No finite margin covers that. The shipped
  slack is 60 - 53.4 = 6.6 s.

## 3. Test results

- `cargo test -p gta_sim -p citygen -j 4`: every target `ok` (config 48, wanted 15, witness_city 1 + 1 ignored
  in 32.6 s, lib 67, civilians 10, vehicle_hits 14, ...).
- `cargo test -p gta_sim --test witness_city -j 4 -- --include-ignored --nocapture`: 2 passed (new 94/100, old 53/100).
- `cargo clippy --workspace --all-targets -j 4 -- -D warnings` and `cargo clippy -p gta_sim -p citygen --all-targets -j 4 -- -D warnings`: clean.
- `cargo test -p gta_like --bin gta_like -j 4`, 3 runs: 77 passed each time.
- Runtime t10/t11 and the street-kill probe were not rerun. The runtime change is one data value (0.8) plus the
  compose check, which accepts the shipped configs (every composed headless test is green). Left to QA.
- `rustfmt --edition 2024` only on files I edited; `git status --short` shows only the files listed above plus
  `log.jsonl` (one `decision` entry). No game process was launched.

children: 0 launched / 0 reported
