# Metrics — TASK-010

| # | Step | Agent | Provider | Model | Effort | Outcome | Tool uses | In-tok | Out-tok | Total-tok | Duration |
|---|------|-------|----------|-------|--------|---------|-----------|--------|---------|-----------|----------|
| 1 | 2.5 | premise-challenge | codex | gpt-6-sol | medium | PREMISE HOLDS | n/a | 424562 | 2222 | 426784 | 2m 30s |
| 2 | 3 | planner | claude | opus | high | ok | 77 | — | — | 487147 | 26m 7s |
| 3 | 4 | plan-reviewer-1 | codex | gpt-6-sol | medium | ok | n/a | 1200298 | 8185 | 1208483 | 6m 16s |
| 4 | 5 | plan-reviewer-2 | claude | opus | medium | ok | 66 | — | — | 289138 | 16m 25s |
| 5 | 6 | implementer | claude | opus | medium | ok | 152 | — | — | 490090 | 53m 54s |
| 6 | 7 | code-reviewer | codex | gpt-6-sol | medium | NEEDS_WORK | n/a | 4927631 | 13936 | 4941567 | 8m 32s |
| 7 | 8 | fixer | claude | opus | medium | ok | 119 | — | — | 304570 | 39m 32s |
| 8 | 9 | qa | claude | opus | medium | SHIP-PENDING-RUNTIME | 54 | — | — | 231797 | 13m 3s |
| 9 | 8 | fixer | claude | opus | medium | ok | 76 | — | — | 257163 | 22m 3s |
| 10 | 9 | qa | claude | opus | medium | NEEDS_FIXES | 50 | — | — | 252971 | 18m 18s |
| 11 | 8 | fixer | claude | opus | high | STOP | 96 | — | — | 370337 | 44m 59s |
| 12 | 8 | fixer | claude | opus | medium | ok | 38 | — | — | 170647 | 16m 35s |
| 13 | 9 | qa | claude | opus | medium | SHIP-PENDING-RUNTIME | 50 | — | — | 276096 | 19m 14s |
| **SUBTOTAL codex** | | | codex | | | | n/a | 6552491 | 24343 | 6576834 | |
| **SUBTOTAL claude** | | | claude | | | | 778 | — | — | 3129956 | |
| **TOTAL** | | 13 spawns | | | | | 778 | | | | 4h 47m |
