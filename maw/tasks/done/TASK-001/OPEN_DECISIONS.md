# Open decisions — TASK-001

- 2026-09-23: native subagents receive a short prompt pointing at the full assembled spawn prompt (path header, shared-memory block, task, PCTX overlay) in C:/Users/user/AppData/Local/Temp/maw-prompts/, instead of inlining 17 KB into the Task call. Alternative: inline verbatim. Flip if an agent is seen skipping the file.
- 2026-09-23: plan-reviewer-2 additionally receives the path of PLAN.md (the 110 KB original) because PLAN_V2.md (31 KB) kept the corrections but dropped per-slice detail and numbers. Instruction: V2 corrections win, PLAN.md is a detail source only. Alternative: give only PLAN_V2 and lose detail. Flip if PR2 reintroduces something V2 corrected.
