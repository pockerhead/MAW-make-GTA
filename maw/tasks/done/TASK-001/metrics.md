# Metrics — TASK-001

| # | Step | Agent | Provider | Model | Effort | Outcome | Tool uses | In-tok | Out-tok | Total-tok | Duration |
|---|------|-------|----------|-------|--------|---------|-----------|--------|---------|-----------|----------|
| 1 | 2.5 | premise-challenge | codex | gpt-5.6-sol | medium | PREMISE HOLDS | n/a | 192167 | 2906 | 195073 | 2m 37s |
| 2 | 3 | planner | claude | opus | high | ok | 56 | — | — | 249853 | 27m 17s |
| 3 | 4 | plan-reviewer-1 | codex | gpt-5.6-sol | medium | ok | n/a | 2503355 | 17390 | 2520745 | 15m 30s |
| 4 | 5 | plan-reviewer-2 | claude | opus | high | ok | 53 | — | — | 229893 | 13m 8s |
| **SUBTOTAL codex** | | | codex | | | | n/a | 2695522 | 20296 | 2715818 | |
| **SUBTOTAL claude** | | | claude | | | | 109 | — | — | 479746 | |
| **TOTAL** | | 4 spawns / 4 agents | | | | | 109 | | | | 58m 32s |
