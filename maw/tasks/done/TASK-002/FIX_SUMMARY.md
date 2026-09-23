# FIX_SUMMARY — TASK-002, fixer round 2

## Fixed

- QA B1: Added an owned jump-only ledge assist with `ledge_assist_max_height` and its probe/clearance/window values in `assets/character/locomotion.ron`. A too-high ledge is kept outside the capsule even where Tnua's natural float spring would otherwise climb it. Renamed the old Tnua ground-sensor/spring values so they no longer claim to set mantle reach. Headless correctness gate covers a ledge at the configured reach, one 0.3 m higher, and lowering the configured reach against the same ledge.
- QA B3: On a reachable ledge, the headless sim places the capsule fully on the top once the jumping body clears the lip. A gate limits settling after edge clearance to 19 fixed ticks (about 0.30 s) and visible foot penetration to 6 ticks (about 0.09 s).
- QA B2: Ground tap gate now requires rise in 0.15–0.35 m, pinning the short hop.
- QA B4: Investigation in progress; BRP now records window focus and present mode with the FPS sample.

## Skipped

- IMPL_REVIEW I2's ordinary walking step-up suggestion: the later owner correction explicitly requires arena boxes to block walking; the new assist triggers only during a jump.
- QA B5: untracked Python cache cleanup is unrelated to the requested B1–B4 fixes.

## Test results

- `cargo test -p gta_sim --offline -j 4`: 14/14 tests passed (1 unit, 13 integration).
- Flip-RED B1: omitting the too-high barrier made `configured_reach_climbs_and_above_reach_blocks` fail because lowering max height no longer stopped the same ledge. Restored: green.
- Flip-RED B3: omitting the snap made `pull_up_settles_quickly_without_burying_feet` fail: 38 ticks from edge clearance to standing. Restored: green.
- Flip-RED B2: omitting the `ActionStarted` buffer clear made `tapped_jump_fires` fail with ground tap rise `0.7281376` m. Restored: green.

Further lint and runtime results will be appended after verification.

## Orchestrator note (appended after the spawn ended)

The fixer's final message (codex `last-message.md`) states verbatim: "B4 remains unverified. The runtime run rebuilt the game but Windows then returned `0xC0000142` before launch. Subsequent shell processes and file writes also failed, so I could not obtain a focused-window FPS sample, rerun runtime QA, or update the report with that final failure." The host was under memory pressure at the time (orchestrator wrapper was reaped, ~9 GB free). Clippy and the runtime scenario were NOT re-run in this round; QA round 2 must run them.
