# IMPL_REVIEW — TASK-012 (GDD T11): police on foot and arrest

Reviewer: code-reviewer (claude/opus, medium). Tree: `feature/t11-police` @ `61275e6`.

## 1. Verdict

**NEEDS_WORK.** The police domain, arrest/Busted flow, cop crimes and the corridor queue slot are correct and
well gated. Two things need fixing. First, the N1 "plaza" spot puts the player inside the 32 x 32 m tower, so
the plaza numbers the navmesh decision and the owner checklist rely on are false. Second, the unplanned
`head_for` change leaves one case where a displaced walker never re-plans.

### Disconfirmation (done first)

Counter-example I picked: *after the `head_for` change, a walker whose route is finished never re-plans, so
when the target moves it stays stuck at the old route end.* I checked it in `tactics/mod.rs:76-92` and
`navigation/mod.rs:286-302`. **It does not hold in the simple form.** After the last node, `route_point`
returns the live `seek.dest`, so the walker follows a moving target in a straight line. If the target moves
far enough that its nearest node changes, `route.goal != Some(goal)` forces a re-plan. **It does hold in a
narrower form** (Issue 2): the walker is displaced away from the finished route, the goal node stays the same,
and then it heads straight at `dest` from anywhere with `avoid = 0`, forever.

## 2. Orchestrator questions

### Q1 — N1 plaza: routing defect or honest geometry? **Neither. The fixture is broken.**
Probe (temporary untracked `tests/zz_cr_probe.rs`, deleted afterwards, `git status` clean; output kept in the
reviewer scratchpad):
```
tower center Vec2(40.853294, -41.059013)  (axis, half_extents) = (.., Vec2(16.0, 16.0))
player [40.852707, 1.1999999, -41.059155] after settle [40.852707, 0.658, -41.059155]
t39 1689v0 Respond at (47.1,-57.2) d 17.3 sees false ...
t39 1692v0 Respond at (48.2,-57.2) d 17.7 sees false ...
t39 1695v0 Respond at (53.5,-57.0) d 20.3 sees false ...   <- the "stuck at 19.9 m" cop
t39 1698v0 Respond at (48.8,-57.1) d 17.9 sees false ...
```
- `CityLandmarks::plaza_center` is the centroid of the plaza block (`world/city.rs:207`). The plaza block is a
  single lot that holds the tower (`citygen/src/lots.rs:51-60`), so the centroid is the tower's centre.
  `police_city.rs:79-81` stands the player **inside the tower** (chest sinks to y 0.658).
- All four cops press against the tower's south face at z ≈ −57.1 (= −41.06 − 16). None ever sees the player.
  The three cops that "reached" only crossed the `keep_distance.1 = 18 m` threshold (`police_city.rs:53`) while
  sliding along the wall. The 19.9 m cop is pressed at a point of the face that is farther than 18 m from the
  centre. The 24-38 s "reach times" are wall-slide times.
- So the plaza says nothing about routing, the direct-seek limit or avoidance. It is not a navmesh signal
  either: the target is unreachable. The hospital and park spots are valid (sidewalk anchor / open park).

### Q2 — can the shared `head_for` change strand a gang member or cop? **Yes, in one narrow case** (Issue 2).

### Q3 — P9: are both death guards needed?
They guard different layers, and neither is dead code:
- `arrest_player`'s `Without<Dead>` (`arrest.rs:32`) is the arrest's own invariant ("a dead player is never
  arrested"). Its only dependency is the Dead insert being applied before it runs (auto `apply_deferred`).
- The FSM's `live_player` filter plus `sees = false` (`behavior.rs:156-160, 225-227`) is a sense rule ("a dead
  player is not seen"). It also stops cops from aiming at or seeking a corpse. Its guarding effect on the
  arrest is indirect: it works only because `police_fsm` (Decide) runs before `arrest_player` in the same tick.
  An ordering change breaks it silently.

**Keep both.** Removing either one alone changes nothing today, but the FSM guard holds the arrest only through
system order. P9 therefore gates the pair (flip P9b), which is the honest claim. Say so in the P9 doc comment
("guards: Without<Dead> in arrest_player and the FSM's dead-player filter; either suffices"). The summary's
deviation #4 already records it. No code change needed.

## 3. Confirmed correct

- `police/fsm.rs:22-44` `next_state` matches the plan table. `Leave` is terminal, `stars == 0 → Leave`, and the
  hostile/arrest-row split is right. `spawn_kind`, `break_free_heat` and `arrest_step` match the plan's worked
  rows.
- `police/arrest.rs`: the same-tick restart after a stale cop (deviation 8) is sound. `attacking` =
  own `ShotFired` or a melee swing. `BrokeFree` writes heat before `WantedSystems`, so stars show 2 in the same
  update (P3).
- `police/dispatch.rs`: counting excludes `Dead`/`Leave`, so the table bound holds after a reset (D9).
  Reinforcement starts on `Added<Dead>`. Candidates are hidden by cone or by occlusion within the shared ray
  budget. When the budget is exhausted, points in view are treated as visible (conservative).
- `police/mod.rs:431-458`: the set order `PoliceSystems.after(PopulationSystems).after(WantedSystems)` holds, and
  so do the flat tuples (TASK-008 lesson). `PoliceRng` uses stream 3 (TASK-010 lesson).
- `flow/busted.rs` + `flow/mod.rs`: `Busted`/`BustedPhase` run on `Time<Real>` in `Update`. The wanted reset is
  on `OnExit(Busted)`. Confiscation happens via `Loadout::default()`. The respawn shares `respawn_at` with
  Wasted (no copy). `NpcSystems` also runs in Busted, while the dispatcher, fire and melee stay in Playing.
- `wanted/crimes.rs`: cop crimes are reported before the witness early-returns, and `resolve(Body)` accepts
  `KillCop`. The `classify_table` cop rows are there.
- `tactics/fire_line.rs:146-165` `queue_slot`: nearest non-yielding blocker, 0.8 m side offset (no new
  number), the same `usable` test as every spot, and only on the non-pinned fallback. The gang corridor
  flips are 5/5 RED without it.
- Data-first: every new tuning number is in `escalation.ron` / `wanted.ron` / `respawn.ron` / `visual.ron` /
  `strings.ron`. The only new `const` is the `POLICE_CONFIG` path. No `unsafe`, no `unwrap` in non-test code.
- Verified in this review: `cargo test -p gta_sim -j 4` gives **289 passed, 0 failed**. `police_city` output
  reproduced exactly as in the summary (hospital 4/4, plaza 3/4 + 19.9 m, park 4/4).

## 4. Issues

### Issue 1 — major — `crates/gta_sim/tests/police_city.rs:79-81` (+ IMPL_SUMMARY §3, owner checklist)
**The plaza spot puts the player inside the tower** (see Q1), so the N1 plaza liveness passes only because of
wall-slide distances. The summary's "Navmesh decision input: the plaza … is slow for graph-only navigation" and
the owner-checklist line "Площадь: копы доходят медленно (до ~40 с), один может застрять в ~20 м — решение по
навмешу" are false evidence and would steer the navmesh decision wrong.
**Fix:** stand the player on a sidewalk point next to the plaza, e.g. `citygen::sidewalk_anchor(layout,
params, landmarks.tower, margin)` (the same helper N2 uses), or the spawn point nearest to `plaza_center`.
Re-run N1, replace the plaza numbers in the summary, and delete or rewrite the owner-checklist line. Add a
`GATE BROKEN` assert that the player's settled chest stays within ~0.1 m of `chest(feet)`. That would have
caught the sunk player at y 0.658.

### Issue 2 — minor (shared code, no gate) — `crates/gta_sim/src/tactics/mod.rs:79-83`
`walked = route.next >= route.nodes.len()` disables the age re-plan even after the walker has been moved away
from the finished route. Motions that do not touch `Route` are `Motion::Yaw` (gang `Retreat` up to
`retreat_distance` 25 m, gang/cop `BackOff`, fire-line clearing moves) and knockback. Concrete gang path: a
member returns home along a route (goal = home node, walked). A later fight is only Yaw moves (band BackOff,
then Retreat 25 m away). The target is lost, so Idle seeks home with `home_clear == false`. The goal node is
unchanged and the route is walked, so there is no re-plan. It walks straight at the spot with `avoid = 0` and
presses into the first building for good. Before the change, the 1 s refresh re-planned it. The cop analogue is
Attack with Hold/BackOff only, then Respond to a `last_known` near the same node. That case is less likely,
because a straight line to a last-seen point is usually open.
**Fix (keeps the oscillation fix):** skip the age refresh only while the walker is still in the goal node's
region, i.e. `walked && nearest_node(ctx.graph, chest) == Some(goal)`. The oscillation came from
`plan_route` re-planning to `[goal]` while the nearest node to the walker *was* the goal. With this guard,
a displaced walker re-plans again. Gate: a headless case with a finished route, a walker teleported ~25 m
behind the wall, dest unchanged, asserting a route re-plan (`route.age` reset) within `route_refresh_seconds`.

### Issue 3 — minor — `crates/gta_sim/src/tactics/mod.rs:74-92` (pre-existing, now load-bearing)
On a non-direct seek, the leg after the last route node is a straight line with `avoid` forced to 0
(`*avoid = 0.0` at :74). Before the change this leg lasted ≤ 1 s before a re-plan. Now it lasts until arrival.
Any `dest` a few metres off the sidewalk behind an obstacle (a search point is always a sidewalk point, but
`last_known` is not) gets a wall press instead of an avoidance slide. It is not shown by any fixture today (the
plaza press is the tower, Issue 1). Suggest applying `avoid_offset` on the slot once `walked` is true, the same
as the direct branch. Also a candidate for the navmesh-decision note.

### Issue 4 — minor — `crates/gta_sim/src/tactics/fire_line.rs:199-206`
When `plan` is `None` off the AI slot, `pinned` and now `queue_slot` (2 `usable` probes with raycasts) run
**every tick** for a blocked shooter until a slot is found. The rays are counted in `load.rays` but not
capped here. This is bounded by shooters × 2, so it is cheap at 12 cops (the bench mean is 1 ms). Consider
gating `queue_slot` on `on_slot` like `clear_spot`.

## 5. Missing coverage

- N1 with a reachable plaza sidewalk spot and the settled-player precondition (Issue 1).
- `head_for` re-plan after displacement from a finished route (Issue 2), and the oscillation fix itself: there
  is no headless gate for "a walker at the goal node heads on to `dest` and is not pulled back". Only the city
  N1 printout covers it, and N1 is liveness only, so reverting the change keeps N1 GREEN at the park spot. A
  small `test_graph` case (walker at the goal node, dest 20 m past it, LOS blocked so direct is false, assert
  progress > 10 m in 5 s) would flip RED on the old code.
- P9 doc comment naming both guards (Q3). Not a new test.

## 6. Nits

- `police/behavior.rs:111-385` `police_fsm` is ~275 lines with a large match. It mirrors `gang_fsm`, so it is
  consistent with the codebase, but the Attack arm (:310-358) could be a helper.
- `behavior.rs:225-227` could be `unit.sees &= live_player.is_some();`, one line.
- The plaza line of the owner checklist in IMPL_SUMMARY §4 must go with Issue 1.

children: 0 launched / 0 reported.
