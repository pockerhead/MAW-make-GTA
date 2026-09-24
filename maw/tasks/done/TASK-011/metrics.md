# Metrics — TASK-011

| # | Step | Agent | Provider | Model | Effort | Outcome | Tool uses | In-tok | Out-tok | Total-tok | Duration |
|---|------|-------|----------|-------|--------|---------|-----------|--------|---------|-----------|----------|
| 1 | 2.5 | premise-challenge | codex | gpt-6-sol | medium | PREMISE HOLDS (orphan) | n/a | 1406010 | 6028 | 1412038 | 7m 5s |
| 2 | 3 | planner | claude | opus | high | ok | 63 | — | — | 366674 | 19m 27s |
| 3 | 4 | plan-reviewer-1 | codex | gpt-6-sol | medium | ok | n/a | 1410011 | 10933 | 1420944 | 7m 34s |
| 4 | 5 | plan-reviewer-2 | claude | opus | medium | ok | 49 | — | — | 241507 | 11m 56s |
| 5 | 6 | implementer | claude | opus | medium | ok | 107 | — | — | 366237 | 34m 2s |
| 6 | 7 | code-reviewer | codex | gpt-6-sol | medium | PASS (orphan) | n/a | 7370145 | 13290 | 7383435 | 12m 3s |
| 7 | 8 | fixer | claude | opus | low | ok | 15 | — | — | 96894 | 3m 40s |
| 8 | 9 | qa | claude | opus | medium | SHIP-PENDING-RUNTIME | 54 | — | — | 229836 | 13m 58s |
| **SUBTOTAL codex** | | | codex | | | | n/a | 10186166 | 30251 | 10216417 | |
| **SUBTOTAL claude** | | | claude | | | | 288 | — | — | 1301148 | |
| **TOTAL** | | 8 spawns | | | | | 288 | | | | 1h 49m |
