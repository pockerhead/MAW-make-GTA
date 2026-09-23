# Open decisions — TASK-008

- 2026-09-23: plan-reviewer-1 (codex) exited rc=1 with no events.jsonl capture, but PLAN_V2.md is complete (all 5 sections, final children line). Accepted instead of a retry that would recompute the same review; recorded as a transport failure in metrics.
- 2026-09-23: code review PASS with zero confirmed issues (one optional future gate noted); the fixer stage is skipped.
- 2026-09-23: QA NEEDS_FIXES (Bug 1: melee sphere cast starts inside the attacker capsule, so any wall/character within 0.05 m on ANY side is hit at distance 0 and eats the strike). The fixer was skipped after the PASS review; it now runs on the QA report (fixer round 1), then QA round 2.
