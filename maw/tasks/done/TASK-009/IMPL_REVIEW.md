# Implementation review — TASK-009

## Verdict

**NEEDS_WORK** — a witness who reports a visible corpse interrupts their own call on the next perception cycle, so that call cannot finish while the corpse remains visible.

## Disconfirmation tested

Counterexample: a shot arrives just after a civilian's perception slot and expires before that slot runs again. This **did not hold**. `collect_stimuli` retains a shot for `slots` fixed ticks (`crates/gta_sim/src/perception/mod.rs:186-194`), and `perceive` checks one slot each tick (`:227-230`), so each assigned slot sees it once within four ticks with the shipped config.

## Confirmed correct

- The four NPC configs are loaded and validated in the shared headless composition; the domain plugins are registered there (`crates/gta_sim/src/lib.rs:65-132`). Tuning values are in `assets/npc/*.ron`, with no new gameplay tuning constant in production code.
- Initial spawning waits for a camera view, uses graph nodes, caps living civilians, and checks the visibility rule; far despawn requires both distance and offscreen time (`crates/gta_sim/src/population/mod.rs:299-424`). The runtime evidence in `scratch/t8_run1/summary.json` records 40 civilians, four closer than 60 m during first fill, and a 9-person scatter after one shot.
- Death converts a civilian to an inert corpse in the death set; perception, decisions, and population are chained after it (`crates/gta_sim/src/civilian/mod.rs:207-232`, `crates/gta_sim/src/perception/mod.rs:136-153`). The headless death and lifetime tests cover this path.
- Civilian animation graphs load clips from each model's own GLB, and the real-GLB test checks joint motion (`src/visuals/character.rs:113-188`, `src/visuals/civilian_gate.rs:142-235`). The witness bar reads sim-owned report progress (`src/hud/witness.rs:47-106`).
- The logged `PreUpdate` dead end is borne out by Bevy 0.19.1's pinned `main_schedule.rs`: `PreUpdate` precedes `StateTransition` and the fixed loop. The current camera view is published in `PostUpdate` after `follow_player` (`src/camera/mod.rs:43-53`, `:158-173`). The shared-target log entry is an environment warning, not evidence of a code defect; fingerprint directories exist, but it does not change this review's source findings.

## Issues

### Major — repeated corpse sight cancels its own witness call

**Location:** `crates/gta_sim/src/perception/mod.rs:259-269`; `crates/gta_sim/src/civilian/mod.rs:283-285`.

`perceive` offers the nearest visible corpse every time the civilian's slot runs. A report-prone civilian can enter `Report` on that corpse. Four fixed ticks later, the same stationary corpse is offered again; the `Report + Some(threat)` branch calls `react(..., false)`, which excludes reporting and chooses `Flee` or `Cower`. The four-second call and its progress bar therefore end after about 62.5 ms while the corpse remains visible. The GDD requires a four-second call interrupted by being killed or frightened; merely continuing to see the original corpse is neither. This also leaves T10 without a usable completed corpse-witness call.

**Suggested fix:** Distinguish a newly perceived threat from the one that started the call, or otherwise prevent the same persistent corpse sighting from interrupting its own report. Preserve interruption for a genuinely new gunshot, aimed weapon, or injury. Add a production-composition headless test with a visible corpse: the witness remains in `Report` and reaches completion after `call_seconds`; then verify a new threat still interrupts it.

## Missing coverage

- A persistent corpse in line of sight throughout a full witness call. Existing `report_completes_after_call_seconds` and `report_is_interrupted_by_a_new_threat` use transient gunshots (`crates/gta_sim/tests/civilians.rs:294-363`), so neither exercises the issue above.
- An integrated witness-bar check driven by a real sim `Report` transition and completion. The current bar gate sets `Civilian.state` directly in a small UI app (`src/hud/witness_gate.rs:77-112`); it verifies the bar's local lifecycle, not that gameplay keeps the call alive.

The owner-run feel and visual checklist belongs in the later `QA_REPORT.md`; this review does not substitute for that run.

children: 0 launched / 0 reported.
