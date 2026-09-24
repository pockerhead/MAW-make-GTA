# TASK-026 IMPL_REVIEW (code-reviewer, claude opus, medium)

## 1. Verdict

**NEEDS_WORK.** The mechanism is correct and every gate the spec asks for exists and is green (I reproduced them). One cheap item is left: the city gate passes at 18/20 against a floor of 17. Any change to how `NpcRng` is consumed reshuffles the seeds, and at the measured rate that turns the gate RED about 13 % of the time with no real regression. Widen the seed set (see I-1). The other findings are minor.

## Disconfirmation (done before the review)

Counter-example I looked for: **a delayed caller who can still see the body.** A cowerer stays next to the corpse. When its crouch ends and it switches to `Report { about: Body }`, the corpse is offered again on the next perception cycle (every 4 ticks). That would hit the `(Report, Some)` arm, `react(.., allow_report=false)` would run, the call would be interrupted, and a cowerer would never finish a call.
**Result: the bug is not there.** `perception/mod.rs:307-311` hides corpses from anyone in `Report` (`filter(|_| !reporting)`), and a gunshot stays in `StimulusLog` for only `slots` ticks, so each agent hears it once. The seeds where the first call came after a flight or crouch (1011, 1014, 1015, 1017) show the delayed path finishing calls in the city.

Second candidate: **`already_fleeing` swallowing a real new threat.** It filters only when `about.is_some() && witnessed(threat) == about`. So a new shot, a new body, or an Aimed/Car/Hurt threat still refreshes, and an `about: None` flight always refreshes. It holds.

## Dead-end triage

`log.jsonl` has no `dead_end` entries, only 3 `decision` entries. I checked each against the code:
- `already_fleeing` exists and does what it says (`civilian/mod.rs:340-346, 365`).
- `pick_victim` sorts ascending by `others` and takes the sparsest victim (`witness_city.rs:77`).
- A cowerer that misses its roll flees with `about: None`, and the body re-arms it (`mod.rs:410` plus the Flee refresh arm at `:374-376`). Confirmed.

## 2. Confirmed correct

- `assets/npc/civilian.ron`: `call_after_flee: 0.6` is data. `report: 1.0 = flee`. The math holds: past `panic_distance` the cower weight is 0 (the gate asserts `report_min_distance > panic_distance`), and R, F iid means P = 1/2. The old value checks out too: P(F < 0.8R) = 0.306.
- `civilian/mod.rs:118-123`: validation of `call_after_flee` in [0, 1] has a config gate that names the field (`config.rs::call_after_flee_is_a_chance`).
- `call_later` (`mod.rs:349-354`) draws `rng.unit()` only after `about?`, so a calm city's `NpcRng` sequence is unchanged. The Cower-miss path draws the same way as before.
- `witnessed()` limits later calls to Gunshot, Fight and Corpse. Aimed, Hurt and Car give `None`, as spec §1 says. On a refresh, `witnessed(&threat).or(about)` keeps the known crime when an Aimed/Hurt threat interrupts.
- One heat per incident is untouched. Calls still go `take_calls -> resolve -> report`, and the `reported` guard dedupes (`wanted/crimes.rs:109-116, 313-333`).
- `lib.rs:132-135`: the incident-memory check uses the new `longest_call_delay` (23.396 s shipped, memory 60 s). `config.rs` re-anchors the sabotage to 20 s, which the old 4.06 s bound would have accepted, so it is a real flip.
- Gates, all rerun by me:
  - `cargo test -p gta_sim -p citygen -j 4`: all targets `ok` (`wanted` 13, `civilians` 10, `config` 47, lib 67, `witness_city` 1 + 1 ignored).
  - `witness_city --include-ignored --nocapture`: new rules 18/20, same per-seed list as the summary; old rules 11/20. Total time 14 s.
  - `cargo clippy -p gta_sim --all-targets -j 4 -- -D warnings`: clean.
- Orchestrator question, rust-analyzer's `field others is never read` at `witness_city.rs:36`: **stale IDE diagnostic, not a defect.** `Run::others` is read at `witness_city.rs:206` (`runs.iter().map(|r| r.others).min()`). rustc and clippy with `-D warnings` report nothing.
- `delayed_calls_about_one_kill_count_once` (`wanted.rs:125-176`) is a real gate:
  - It asserts the precondition: each witness is in `Flee { about: Body(victim) }`.
  - `run_until_calls` asserts that 3 calls arrive (`wanted_support/mod.rs:217-229`).
  - The implementer's flip of the `reported` guard reads as correct (120 vs 40).
- Surgical: the `about: None` additions at the 3 literal state sites are the minimal change. `civilian/mod.rs` is 508 lines.

## 3. Issues

### I-1 (major, gate robustness): `crates/gta_sim/tests/witness_city.rs:23-24`, 18/20 against a floor of 17

The gate is deterministic for a fixed `NpcRng` stream. But TASK-010's lesson is that consumption shifts often (an extra draw anywhere in civilian/population). Each shift is effectively a new random sample of 20 kills. At the measured per-kill rate p ≈ 0.9:
- P(< 17/20) = **0.133**. About one unrelated change in 8 turns this gate RED.
- At p = 0.85 (inside the 18/20 confidence interval) it is 0.35.

Old rules measure 11/20, so p ≈ 0.55. Cost: ~7 s per 20 seeds on 4 threads.

| Seeds / floor | False RED at p=0.9 | False RED at p=0.85 | Old rules pass (p=0.55) |
|---|---|---|---|
| 20 / 17 (now) | 0.133 | 0.35 | 0.005 |
| 60 / 51 (85 %) | 0.073 | 0.41 | ~1e-6 |
| 60 / 48 (80 %) | 0.006 | 0.11 | 5e-5 |
| 100 / 85 (85 %) | 0.040 | 0.43 | ~0 |

**Suggested fix:**
1. Run 100 seeds (1000..1099, ~35 s) and measure the real rate first.
2. If it is ~0.9 or higher, keep the spec's 85 % (85/100). That drops the false-RED rate from ~13 % to ~4 % and keeps the flip decisive.
3. If the measured rate is closer to 0.85, the spec's 85 % target sits on the mechanism's own mean, and no N makes the gate stable. In that case either the orchestrator lowers the floor to 80 % (a design call, record it), or the mechanism is tuned up (e.g. `call_after_flee` 0.6 -> 0.8).

Keep printing the time-to-star distribution and the old-rules flip on the same seed set. (Per the gates domain: act on this diagnosis, recompute the numbers on the real run.)

### I-2 (minor, silent-loss bound): `crates/gta_sim/src/civilian/mod.rs:125-130` + `lib.rs:132-135`, `longest_call_delay` is not an upper bound from the crime

The bound is measured from the last stimulus. A body stays a stimulus until `corpse_seconds` (30 s, `population.ron:18`). A witness who first sees the body at ~29.9 s calls up to 23.4 s later, i.e. ~53 s after the kill. The Kill incident's `last` is the kill time, so it must outlive `corpse_seconds + delay`, not `delay`.

- Shipped numbers fit: 53.4 < 60.
- The failure is silent. If the owner raises `flee_distance` to 120 m, the bound becomes 36.7 s and still validates, while a body-sight call at 66 s is lost.
- The bound also assumes a full-speed flight. `left` counts measured speed, so a slowed fleer takes longer.

This gap existed before (30 + 4.06), but this task made the bound the main guard and cut the slack from ~26 s to ~6.6 s.

Fix: include `corpse_seconds` in the value passed to `validate_call_delay` (compose_sim has the population config), or write down why it is excluded. A flee-speed margin is optional.

### I-3 (minor): `witness_city.rs:61-62`, stale doc comment

The doc says `(victim, player chest)`, but the function returns `(victim, chest, others)`.

### I-4 (minor): `witness_city.rs:208`, broken format string

The literal contains `{fled};          fewest` (a run of spaces, apparently a rustfmt-wrapped string that got glued back together). Output only; fix the text.

## 4. Missing coverage

- **The cower-ends-then-calls path with the body in sight.** This is the scenario `already_fleeing` was added for, and no focused test covers it. Only the city gate covers it implicitly, and `first_caller_fled` does not tell Flee from Cower.
  - Needed: a floor test with a cow-prone witness < `panic_distance` from the body and `call_after_flee = 1.0`. Assert a `PoliceCall` about `Body(victim)` within `cower_seconds.1 + call_seconds + slack`.
  - Flip: remove the `threat.filter(already_fleeing)` line. The crouch then refreshes every 4 ticks and no call comes before the corpse despawns.
- **A flight from a non-crime (Aimed or Car) never produces a delayed call.** `witnessed` returns `None` for these, but no test pins it. Suggested: aim at a flee-prone civilian with `call_after_flee = 1.0` and assert no `PoliceCall` after `longest_call_ticks`.
- **A delayed call interrupted by a new gunshot** gives no call about the old crime until the new flight ends. Behaviour is inherited through `react(.., false)`. Low priority.

## 5. Nits

- `witness_city.rs:22`, `TICK_HZ = 64` is hard-coded. `wanted_support::longest_call_ticks` already reads `Time<Fixed>::timestep()`; converting ticks to seconds the same way would be consistent.
- `civilian/mod.rs:125`: the doc says "Upper bound". After I-2 it is an upper bound per stimulus, not per crime. Adjust the wording if I-2 is not fixed.

children: 0 launched / 0 reported
