# Metrics — TASK-028

| # | Step | Agent | Provider | Model | Effort | Outcome | Tool uses | In-tok | Out-tok | Total-tok | Duration |
|---|------|-------|----------|-------|--------|---------|-----------|--------|---------|-----------|----------|
| 1 | 2.5 | premise-challenge | claude | opus | medium | PREMISE HOLDS | 9 | — | — | 79939 | 1m 17s |
| 2 | 3 | planner | claude | opus | high | ok | 43 | — | — | 189198 | 12m 38s |
| 3 | 4 | plan-reviewer-1 | claude | opus | medium | ok | 25 | — | — | 128796 | 6m 9s |
| 4 | 5 | plan-reviewer-2 | claude | opus | high | ok | 28 | — | — | 156383 | 7m 29s |
| 5 | 6 | implementer | claude | opus | high | ok (tag pending on TASK-038) | 58 | — | — | 173051 | 39m 37s |
| 6 | 7 | code-reviewer | claude | opus | medium | PASS | 18 | — | — | 146702 | 4m 30s |
| 7 | 8 | fixer | claude | opus | medium | ok | 38 | — | — | 138679 | 21m 23s |
| 8 | 9 | qa | claude | opus | medium | SHIP | 49 | — | — | 184825 | 60m 46s |
| **SUBTOTAL codex** | | | codex | | | | n/a | 0 | 0 | 0 | |
| **SUBTOTAL claude** | | | claude | | | | 268 | — | — | 1197573 | |
| **TOTAL** | | 8 spawns | | | | | 268 | | | | 2h 33m |
