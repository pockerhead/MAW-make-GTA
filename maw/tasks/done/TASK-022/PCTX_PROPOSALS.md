# PCTX proposals — TASK-022

## 2026-09-24 (implementer) — domain game-design, risk lesson

The seed-1 sidewalk graph has 579 nodes, but they are block corners only: along a street the next node is
the next intersection (~90 m), the "min spacing 4.5 m" is the gap between corners of one crossing. Any
"spawn on a node" rule can only use intersections, and on a straight street every node ahead within 60 m
is in clear view. TASK-022 had to spawn on points every `spawn_point_spacing` m along edges
(`population::spawn_points`). Proposed wording: "sidewalk graph nodes = block corners (~90 m apart along a
street); spawners that need spots along a street sample edge points, not nodes (TASK-022)."

## 2026-09-24 (implementer) — domain gates, risk lesson

A density gate over one deterministic walk is phase-sensitive: with cap 40 and despawn at 150 m the
in-view count cycles 0..21 with each cross street (~20 s at run speed), and the first despawn happens only
once the fill crowd is 150 m behind (~45-60 s). A 20 s window gave 3.3..8.6 for the same code; tuning grids
were non-monotonic. Measure after the initial wave has despawned (track its entities) over at least two
cycles, and report the window spread next to the mean.

## 2026-09-24 (qa) — domain gates, risk lesson

`visuals::civilian_gate::every_civilian_model_animates_from_its_own_clips` (TASK-009) is flaky: 3 of 8
runs RED on the TASK-022 base commit 865969c and about the same on HEAD ("leg-left turned 0.02 rad ... (T-pose)").
Several stage summaries reported "38 passed" from a single lucky run. Proposed rule: a stage that reports a
presentation-gate count runs `cargo test -p gta_like --bin gta_like` at least 3 times, or the gate is fixed
to be deterministic (its 32 updates depend on something non-fixed).

## 2026-09-24 (fixer round 2) — domain gates, risk lesson

A pose gate that compares a joint at two moments ("leg-left turned > 0.1 rad in 32 updates") is
phase-dependent: a swinging joint passes the same angle on both sides of an extreme. The clip phase
depended on how many updates asset loading took (3..11), so `civilian_gate` failed ~35 % of runs with
0.018 rad. Assert the largest deviation from the first pose over the window (measured 0.89..1.03 rad,
30/30 green). Proposed wording: "animation gates sample every update and assert the range, never two
snapshots (TASK-022)."
