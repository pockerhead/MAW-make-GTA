# Open decisions — TASK-004

- 2026-09-23: PREMISE SUSPECT treated as a gate-strength finding, not a wrong premise: acceptance criterion sharpened to count mesh-bearing entities (no per-building Mesh3d); pipeline continues to the planner without re-running the premise stage. Flip: if the planner finds merging conflicts with T2 colliders, re-open.
- 2026-09-23: planner planned to commit Kenney GLB/PNG into git; overridden by the owner rule (binary assets never in git, shipped in release zips). Assets are fetched by tools/fetch_assets.py per a tracked manifest. Open questions Q1-Q3 answered as recommended.
