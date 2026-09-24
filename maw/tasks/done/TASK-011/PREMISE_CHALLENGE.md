## Counter-example tested

A single corpse can generate repeated completed civilian calls without a stable incident identifier, so heat rises multiple times for one crime even while the listed witness, threshold, and decay checks pass.

## Primary-source investigation

(Appended by the orchestrator from the stage's final message. The codex process lost file writes after a Rust linker failure under host memory pressure, and its wrapper was reaped.)

- The code allows a civilian to call again about the same corpse after a completed call: `crates/gta_sim/src/perception/mod.rs:259-270`, `crates/gta_sim/src/civilian/mod.rs:279-305`.
- `crates/gta_sim/src/wanted/mod.rs:4-18` is the existing wanted stub.
- TASK-011 explicitly requires counting one report per corpse or incident (orchestrator note from TASK-009), so the counter-example is a requirement the task already carries. It does not show a false premise.
- The attempted test produced no behavioral output because the linker failed.

## Did it hold

No.

## Verdict

PREMISE HOLDS — the verdict rests on code reading only (`perception/mod.rs:259-270`, `civilian/mod.rs:279-305`, `wanted/mod.rs:4-18`); no test ran.
