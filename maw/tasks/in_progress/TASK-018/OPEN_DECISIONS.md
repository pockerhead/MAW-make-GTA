# Open decisions — TASK-018

- 2026-09-23: QA NEEDS_FIXES (B1 regression: walk-off-crate and buffered-landing paths still cannot climb a 0.7 m step from a 0.8 m crate; B2 75-degree row vacuous). Orchestrator decision: fixer round 2 with an explicit mechanism to verify (anchor follows the support surface), not another path-by-path patch. Flip: if round 2 fails on the same class, stop patching and re-plan ledge assist as a full-mode task.
