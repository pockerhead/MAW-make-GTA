# TASK-033: Гридлок трафика без игрока — резервация перекрёстка у машины в физике

Type: bug
Mode: small-fix
Priority: high
Branch: fix/traffic-gridlock
Domains: bevy-ecs, gates, game-design

## Description
Playtest TASK-031 finding F1 (MAJOR, §1 point 8), reproduced twice with NO player input: `--seed 1`, stand at the spawn — by t≈29 s 19 of ~22 traffic cars stop for the rest of the run on the street z≈−81 and the junction (7.9, −81.3); a second jam at (−2.0, −10.3) in the tourist session (one car stopped 213 s); civilians in Wander wait at the crossing up to 139 s. At the centre of the jam sits a traffic car in `Dynamic` mode with a non-empty `waiting`. Hypothesis (playtester): a car switched to physics keeps its junction reservation and never releases it. Evidence: `maw/tasks/in_progress/TASK-031/scratch/baseline_s1/summary.json`, `baseline_s1_replay/samples.json`, screenshots listed in PLAYTEST_REPORT.md F1.

Headless city gates missed it: the traffic-health probes in TASK-016 QA ran with a player/police present or other seeds/poses. Fix the ROOT cause with a universal rule (per the redesign-over-patch rule): **a reservation is a lease** — it is released when its holder leaves the kinematic traffic mode, despawns, or makes no progress along its connector for `reservation_timeout` (data) — plus whatever the root-cause diagnosis shows (e.g. a dynamic car must settle back to kinematic or be abandoned/despawned by the bubble). Diagnose first with a headless reproduction on the real seed-1 city.

## Acceptance criteria
- [ ] Headless reproduction RED first: seed-1 city, production population, no player input, 120 s — stopped traffic cars (< 0.5 m/s for > 20 s) ≥ K before the fix.
- [ ] After the fix the same gate over seeds 1, 2, 7, 42: no car stopped > 20 s except in a queue behind a moving leader; no junction reservation held > `reservation_timeout` by a non-progressing car. Flip-RED by disabling the lease.
- [ ] Existing traffic/police gates green; traffic bench within budget.
- [ ] Runtime: the playtester's baseline (`maw/tasks/in_progress/TASK-031/scratch/tools/`) on seed 1 for 2 min shows no jam (stopped_cars series reported).
- [ ] clippy -D warnings, `cargo test -p gta_sim -p citygen -j 2`, client gates green; CI green.

## Dependencies
- none (blocks the TASK-031 continuation)
