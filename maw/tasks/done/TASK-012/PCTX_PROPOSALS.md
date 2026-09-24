
## 2026-09-24 (TASK-012, plan-reviewer-2) — gates domain, risk lesson proposal

- Test-floor fixtures collide with `world/test_area.rs` more often than planners expect: the 4x5x4 box at
  (-10, 2.5, -22.66), the 30 deg ramp at (-10, *, -16.43) and the wall at z = 14 (x in [-6, 6], 4 m high) blocked
  two walk edges of a planned search square and occluded four "open floor" spawn points of a planned off-frame
  gate (probe: `maw/tasks/in_progress/TASK-012/scratch/pr2_d6_occlusion.py`). Proposed rule: a plan that places
  graph edges, walk paths or camera lines on the test floor checks them against `TEST_AREA` with a probe.
- `WantedLevel.stars` is recomputed only in `track_search` (WantedSystems, after Decide): a fixture that writes
  `heat` and spawns an FSM consumer in the same update meets `stars == 0` on the first Decide. Proposed trigger:
  `set_heat(` followed by a spawn without `run_ticks` between them.

## 2026-09-24 (TASK-012, implementer) — bevy-ecs / game-design risk lessons

- Shared `tactics::head_for` re-planned a route by age even after it was walked to its end: from the node nearest
  to the destination a re-plan is `[that node]`, so the walker went back to the node every `route_refresh_seconds`
  and oscillated 30-45 m short of an open plaza (seed 1, `scratch/probe_police_nav_plaza.txt`). Fixed by not
  re-planning a walked route unless its goal node changes. Proposed trigger: any new `Seek` user whose
  destination is not a graph node (cops, gangs chasing off the sidewalk) — check the last leg in the city, not
  only on the test floor.
- A "death beats arrest" style guard can be doubled by an unrelated rule (the cop FSM stops seeing a `Dead` player
  in the same tick, so the arrest cancels before `arrest_player`); a flip of one guard stays GREEN. Proposed rule:
  when a single-guard flip stays GREEN, find the second guard and flip both, and record both.

## 2026-09-24 (TASK-012, qa) — gates / game-design risk lesson proposal

- A fire-line (hold-fire) test must bound a spared body by the BULLET's reach, not the shooter's centre-to-centre
  distance: hitscan casts `range` from the muzzle (~0.45 m ahead of the chest) and stops on the capsule surface
  (0.3 m before the centre), so a body 45.0-45.75 m along an SMG line is "clear" yet gets hit. Surround spawns put
  units on opposite bearings ~2x band apart, so misses flying past the player land on the far unit (12-SWAT street
  probe: cop-to-cop hits in 4/5 layouts). Proposed trigger: `along < range` / `range` compared with a centre distance
  in any line-of-fire predicate; gate with a victim just past `range` whose capsule is inside the bullet's reach.
- Runtime "stuck" trackers must use physical distance: a tracker that counts a unit as "reached" by its FSM state
  (`Attack`/`Arrest` = sees the player) hides units that see the player from 30 m and never close in (t11.py).
