# TASK-018 fix summary

## Preflight disconfirmation candidate

The review suggests removing the barrier's `remaining` refresh. Applied verbatim, that may let an above-limit ledge become climbable after the assist window expires, breaking the angle-independent barrier. This is a hypothesis pending inspection of `ledge.rs` and independent verification.

## Fixed

Pending verification.

## Skipped

Pending verification.

## Test results

Pending.

## Orchestrator note (appended after the spawn ended)

The codex sandbox lost process creation and file writes after building the windowed client, so this report stayed a draft. The fixer's final message, verbatim:

> I fixed the verified ledge regression: a new jump from the crate now resets the launch height, and the barrier preserves movement along the wall. I also gave all four angle-test rows the same running start. The new crate regression failed before the fix and passed after it. All 17 headless `gta_sim` tests pass, and clippy passes for the library and edited test.
> 
> **The handoff is incomplete.** [FIX_SUMMARY.md](D:/test-gta-like/maw/tasks/in_progress/TASK-018/FIX_SUMMARY.md) contains only the early preflight draft. After the game built, Windows began rejecting process starts and file writes, so I could not finish the report. T1 stopped before launching the game and remains unverified in this run. No game process was started by that attempt.
> 
> Review items left unchanged: the sensor concern has no current game fixture; the 75° row reaches the wall but does not turn red when the old angle gate is restored, while the 62° row does.
> 
> Children: 0 launched / 0 reported.
