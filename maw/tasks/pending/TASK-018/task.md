# TASK-018: Ledge assist — angle-independent barrier and free-space check

Type: bugfix
Mode: small-fix
Priority: high
Branch: bugfix/ledge-assist-angle-and-space
Domains: bevy-ecs, gates

## Description
Follow-up from TASK-002 QA round 2 (`maw/tasks/done/TASK-002/QA_REPORT.md`, B6 and the snap observation).
1. B6: the approach-angle condition at `crates/gta_sim/src/character/ledge.rs:87` skips the WHOLE ledge assist, including the "too high" barrier, when the jump approaches the wall at more than ~60 degrees from its normal; then Tnua's natural float reach decides again (with `ledge_assist_max_height = 1.2` a 1.3 m ledge is climbed at a 62 degree approach). The barrier must hold for every approach angle; only the pull-up itself may depend on the angle.
2. The pull-up snap places the capsule on the ledge top without checking that the capsule fits there. In the city (T2) a ledge under a low overhang or next to a wall must not snap the body into geometry: check free space at the target (shape cast / intersection with the character capsule) and skip the snap when blocked.
Keep B7 (one-tick pull-up) as is — the owner judges it in his run.

## Dependencies
- prefer after TASK-002 — fixes the ledge assist it introduced

## Acceptance criteria
- [ ] Headless: with `ledge_assist_max_height` lowered below the natural Tnua reach, a ledge above the limit is NOT climbed at approach angles 0, 45, 62 and 75 degrees (table test); flip-RED by restoring the angle-gated barrier
- [ ] Headless: a reachable ledge with a ceiling slab leaving less than the capsule height above its top does NOT snap the body into the slab; flip-RED by removing the free-space check
- [ ] All existing `gta_sim` tests (14) stay green; clippy `-D warnings` green; `tools/qa/scenarios/t1.py` passes
- [ ] Existing tests pass
