# Metrics — TASK-013

| # | Step | Agent | Provider | Model | Effort | Outcome | Tool uses | In-tok | Out-tok | Total-tok | Duration |
|---|------|-------|----------|-------|--------|---------|-----------|--------|---------|-----------|----------|
| 1 | 2.5 | premise-challenge | claude | opus | medium | PREMISE HOLDS | 14 | — | — | 83958 | 1m 34s |
| 2 | 3 | planner | claude | opus | high | ok | 105 | — | — | 487637 | 28m 57s |
| 3 | 4 | plan-reviewer-1 | claude | opus | medium | ok | 76 | — | — | 264115 | 12m 55s |
| 4 | 5 | plan-reviewer-2 | claude | opus | medium | ok | 80 | — | — | 272232 | 14m 29s |
| 5 | 6 | implementer | claude | opus | medium | stopped (session end) | — | — | — | — | — |
| 6 | 6 | implementer | claude | opus | medium | ok | 31 | — | — | 183891 | 11m 29s |
| 7 | 7 | code-reviewer | claude | opus | medium | PASS | 14 | — | — | 218065 | 5m 38s |
| 8 | 8 | fixer | claude | opus | low | ok | 32 | — | — | 123391 | 6m 27s |
| 9 | 9 | qa | claude | opus | medium | SHIP-PENDING-RUNTIME | 58 | — | — | 243650 | 32m 40s |
| **SUBTOTAL codex** | | | codex | | | | n/a | 0 | 0 | 0 | |
| **SUBTOTAL claude** | | | claude | | | | 410 | — | — | 1876939 | |
| **TOTAL** | | 9 spawns | | | | | 410 | | | | 1h 54m |
