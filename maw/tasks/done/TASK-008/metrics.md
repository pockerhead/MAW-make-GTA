# Metrics — TASK-008

| # | Step | Agent | Provider | Model | Effort | Outcome | Tool uses | In-tok | Out-tok | Total-tok | Duration |
|---|------|-------|----------|-------|--------|---------|-----------|--------|---------|-----------|----------|
| 1 | 2.5 | premise-challenge | codex | gpt-6-sol | medium | PREMISE HOLDS | n/a | 567527 | 3400 | 570927 | 3m 5s |
| 2 | 3 | planner | claude | opus | high | ok | 85 | — | — | 410290 | 18m 58s |
| 3 | 4 | plan-reviewer-1 | codex | gpt-6-sol | medium | rc=1, no event capture; complete artifact accepted | n/a | n/a | n/a | n/a | 8m 4s |
| 4 | 5 | plan-reviewer-2 | claude | opus | medium | ok | 53 | — | — | 251537 | 14m 17s |
| 5 | 6 | implementer | claude | opus | medium | ok | 119 | — | — | 381418 | 38m 4s |
| 6 | 7 | code-reviewer | codex | gpt-6-sol | medium | PASS | n/a | 3526353 | 10057 | 3536410 | 6m 29s |
| 7 | 9 | qa | claude | opus | medium | NEEDS_FIXES | 50 | — | — | 179116 | 12m 7s |
| 8 | 8 | fixer | claude | opus | medium | ok | 44 | — | — | 162378 | 12m 30s |
| 9 | 9 | qa (re-spawn 1) | claude | opus | medium | SHIP-PENDING-RUNTIME | 42 | — | — | 157730 | 11m 30s |
| **SUBTOTAL codex** | | | codex | | | | n/a | 4093880 | 13457 | 4107337 | |
| **SUBTOTAL claude** | | | claude | | | | 393 | — | — | 1542469 | |
| **TOTAL** | | 9 spawns | | | | | 393 | | | | 2h 5m |
