# TASK-019: Re-plan the jump ledge assist on Tnua's real support

Type: bugfix
Mode: full
Priority: high
Branch: bugfix/ledge-assist-redesign
Effort: planner=high, code-reviewer=high
Domains: bevy-ecs, gates, game-design

## Description
The jump "ledge assist" (owner found the auto pull-up onto ledges fun; kept as a feature in TASK-002) has now failed review/QA four times on the same class of defect: the reach limit and its barrier are anchored to something that does not match what the Tnua controller actually stands on. Patching path by path stopped (see `maw/tasks/blocked/TASK-018/OPEN_DECISIONS.md`). Re-plan the mechanism from the evidence instead.

Starting point: this task branches from TASK-018's branch, so the current code already has an angle-independent barrier, a free-space check before the snap, a support re-anchor by a single centre ray, and gates for angles, ceiling and crate paths. Evidence to read first:
- `maw/tasks/done/TASK-002/QA_REPORT.md`, `QA_REPORT.prev-2.md` (B1 no-op RON values, B3 slow crawl, B6 angle leak, B7 one-tick snap)
- `maw/tasks/blocked/TASK-018/IMPL_REVIEW.md`, `QA_REPORT.prev-1.md`, `QA_REPORT.md` (stale anchor; walk-off-crate and buffered-landing regressions; B3: centre ray vs the 0.29 m cylinder sensor, narrow 0.25 m steps pin the character forever)
- probes: `maw/tasks/blocked/TASK-018/scratch/qa/` (`run_probe.py`, `qa_probe.rs` incl. `qa_geo_sweep`, `qa_geo_trace`) and the pre-task baseline commit `2ade7af`

The planner may also conclude that a simpler design is better (e.g. anchor on Tnua's own sensor/standing output, or bound the reach purely by jump physics and named values), as long as every behaviour below holds.

## Acceptance criteria
- [ ] Reach is governed by named values in `assets/character/locomotion.ron`; a ledge at the limit is climbed and one ~0.1-0.3 m above it is not, at approach angles 0-75 degrees (table gate, each row flip-RED)
- [ ] Nothing that the pre-task baseline `2ade7af` could climb below the limit becomes unclimbable: walk-off and buffered-landing paths from crates 0.25 m, 0.35 m and 2 m deep (heights 0.5-0.9 m, walls 1.45-1.6 m) all climb; gate each and show it RED on the TASK-018 head `9a170d2` where that head fails
- [ ] No state where the character is pinned against a wall indefinitely while standing (a gate that fails if horizontal progress stalls > 1 s while grounded and pressing toward a climbable step)
- [ ] The pull-up does not snap into geometry (ceiling/overhang gate stays green) and completes quickly (settle gate stays green)
- [ ] All existing `gta_sim` tests green (or explicitly replaced with a stronger gate, named in the summary); clippy `-D warnings` green; `tools/qa/scenarios/t1.py` passes (run by QA — the codex stages must not build the windowed client)
- [ ] Existing tests pass
