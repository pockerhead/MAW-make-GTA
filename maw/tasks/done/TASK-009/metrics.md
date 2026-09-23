# Metrics — TASK-009

| # | Step | Agent | Provider | Model | Effort | Outcome | Tool uses | In-tok | Out-tok | Total-tok | Duration |
|---|------|-------|----------|-------|--------|---------|-----------|--------|---------|-----------|----------|
| 1 | 2.5 | premise-challenge | codex | gpt-6-sol | medium | PREMISE HOLDS | n/a | 365775 | 2759 | 368534 | 2m 44s |
| 2 | 3 | planner | claude | opus | high | ok | 66 | — | — | 365206 | 19m 6s |
| 3 | 4 | plan-reviewer-1 | codex | gpt-6-sol | medium | ok | n/a | 1843650 | 12250 | 1855900 | 8m 23s |
| 4 | 5 | plan-reviewer-2 | claude | opus | medium | ok | 47 | — | — | 228439 | 14m 57s |
| 5 | 6 | implementer | claude | opus | medium | ok | 125 | — | — | 425098 | 43m 8s |
| 6 | 7 | code-reviewer | codex | gpt-6-sol | medium | NEEDS_WORK | n/a | 3647155 | 9522 | 3656677 | 6m 36s |
| 7 | 8 | fixer | claude | opus | medium | ok | 24 | — | — | 139254 | 5m 22s |
| 8 | 9 | qa | claude | opus | medium | SHIP-PENDING-RUNTIME | 57 | — | — | 202468 | 10m 52s |
| **SUBTOTAL codex** | | | codex | | | | n/a | 5856580 | 24531 | 5881111 | |
| **SUBTOTAL claude** | | | claude | | | | 319 | — | — | 1360465 | |
| **TOTAL** | | 8 spawns | | | | | 319 | | | | 1h 51m |
