# Open decisions — TASK-007

- 2026-09-23: owner asked for floating damage numbers + damage variance + CRIT on headshots mid-planning; added to TASK_FINAL as an owner addition. plan-reviewer-1 was already running on the old spec; plan-reviewer-2 reads the updated TASK_FINAL and must fold it into PLAN_FINAL (told explicitly in its prompt).
- 2026-09-23: QA NEEDS_FIXES (B1 ghost shot after Wasted). Orchestrator: fixer round 2 with B1 + L1 (per-weapon cooldown/bloom) + L2 (aggregate shotgun pellet numbers) + L4 (combat RNG seeded from the city seed); L3 (GDD values drifted) is updated by the orchestrator at wrap-up.
