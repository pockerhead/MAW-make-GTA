# PCTX proposals — TASK-017

## 2026-09-25 (planner) — game-design risk lesson (evidence: scratch/planner/t9_findings.txt)
The t9 flake is not "fire-line starvation among groupmates": in 20 runtime runs every failure (3/20) had a SECOND
group of the same gang (post 14, heat > 0, sight 40 m) engage from the far side of the player; runs without it
fired first at 0.6-0.9 s, runs with it at 1.4-1.8 s or never / friendly fire. The fire discipline is one-sided
(a shooter holds fire over a spared body up to weapon reach PAST the target) with no positional counterpart
(a body does not avoid standing in a friendly's line of fire, Killzone-style line-of-fire costs), so two groups on
opposite sides of a target block each other by construction. Proposed lesson line for domains/game-design.md:
"Hold-fire past the target needs a positional rule (stay out of friendly lines / flank), or opposite groups
deadlock; runtime scenarios that watch only the provoked group miss the second group that causes it."

## 2026-09-25 (implementer) — bevy-ecs / gates risk lesson (evidence: scratch/impl/t16_try2.txt, tools/qa/trace.py)
Bevy 0.19.1 `trace_chrome` (feature `profile`) at the 5-star bench scene writes 100-240 MB/s: 9-16 GB for a
~2 min session, even with `RUST_LOG=error,bevy_ecs::system::function_system=info,bevy_ecs::schedule::schedule=info,bevy_app::sub_app=info`
(unfiltered, `par_for_each` and executor spans are half the volume); the FlushGuard drop at shutdown takes > 15 s.
Parse a window by bisection on `ts` (events are written in time order), count EXCLUSIVE time per span on its
thread (schedule runners like `run_fixed_main_schedule` / `run_physics_schedule` contain the systems they run
inline), delete the file after the summary. Proposed line for domains/gates.md.

## 2026-09-25 (implementer) — gates risk lesson (evidence: scratch/impl/flips_step2.txt)
A "the member moved to unblock" gate over the whole 30 s fight (`moved_max`) is vacuous: a gang member that runs
dry closes in to punch under any rule (2.7-14.5 m moved with the rule sabotaged). Measure displacement while the
member still has rounds (`moved_armed`). Proposed line for domains/gates.md.
