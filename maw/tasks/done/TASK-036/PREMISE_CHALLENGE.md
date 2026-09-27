# PREMISE_CHALLENGE — TASK-036 (rescoped A/B/C)

## 1. Counter-example tested

(Written before any code was read.) Item B claims: a body the conflict table does not know (a demoted stuck
holder, a box passer, a left car) standing in the 0.87-0.91 m corner overhang past `connector_rects`
(centre line +- half width) does NOT stop a grant on a conflicting connector, and a kinematic car drives
through it. Concrete counter-example: the grant path does not decide "clear" from `connector_rects` alone.
It asks `RoadOccupancy` with the standing body's real footprint rectangle, and the granted car's own
drive-time sensing on a connector sweeps its real body rectangle along the curve. If either covers the
overhang, the "drives through it" case cannot happen, and item B is framed on a non-issue.

## 2. Primary-source investigation

- `crates/gta_sim/src/traffic/box_rules.rs:13-27` `connector_rects`: the requester's connector path is the
  polyline as rectangles `half_width` either side, no end caps, no body length, no yaw swing.
- `box_rules.rs:32-48` `connector_clear`: `road.blocked(r, skip, ..)` for each of those rects.
  `occupancy/query.rs:183-200` `blocked` + `:109-126` `overlaps`: the OTHER body is tested with its real
  `Footprint::Rect`. So the standing body's own extent is covered. The requester's extent is not: its path
  is the band only.
- `traffic/drive.rs:341`: `half_width = half.x + conflict_margin / 2.0` (margin 0.3 in `assets/traffic/*.ron`
  line 17), i.e. the band is the car half width + 0.15 m. The body's pivoting corners are not in it.
- `traffic/junction.rs:104-115`, `:157-161`: demotion of a stuck holder on its connector uses the same
  `connector_clear`. A demoted car keeps standing on its connector.
- Drive-time sensing, `traffic/manoeuvre.rs:55-116` `sweep_ahead`: on a connector the car sweeps its real
  body rect along the path, BUT `:75-84` + `:85` skip every body behind the nose line
  ("one at the flank or swept by the rear on a curve never holds the car", doc at `:51-54`). The rear swing
  of a right-turn pivot is exactly the part the premise names ("its nose and rear sweep the neighbouring
  lanes"), and sensing ignores it by construction. On a lane (`manoeuvre.rs:160`) the cast is a straight
  strip that does not follow the turn. `drive.rs:139-149` `held_in_box` + `manoeuvre.rs:145-147`: a demoted
  holder (no grant) is not skipped, so a body AHEAD of the nose is sensed; stopping from `turn_speed` 6 m/s
  at `max_deceleration` 8 m/s^2 still needs 2.25 m, on 2-2.6 m right-turn connectors.
- Item C: `traffic/lateral.rs:47-61` `effective_lateral` decays a residual `lateral` linearly over the
  connector (non-zero at entry); `traffic/graph.rs:115-139` `body_sweep` builds conflicts at lateral 0.
  The gap the premise names exists in code.
- Item A: the probe exists (`maw/tasks/done/TASK-037/scratch/probe/ws/probe/tests/qa_walkers.rs`, 5034 B).
  Its stall metric counts only `Wander | Flee` walkers (probe line ~66), and `civilian/mod.rs:449-451` puts
  deliberate pauses in a separate `Idle` state, so the stall is not a designed idle. No crossing-wait rule
  exists in `civilian/` or `navigation/` (grep for crosswalk/crossing: no hits), so a walker standing
  70-118 s next to a car is not an intended wait either. I did not run the probe (150 s city replay); the
  rescope already requires reproducing it headless first and dropping it if it does not reproduce.

## 3. Did it hold

The counter-example fails. The standing body is modelled by its real rect, but the granted car's swinging
corners are not modelled at grant time (band only), and drive-time sensing explicitly ignores bodies at the
flank or behind the nose, which is where a right-turn pivot's rear swings. So the premise's mechanism
("a body in that overhang does not stop a grant ... a kinematic car drives through it") is consistent with
the code, and the task already treats it as unreproduced and conditional (fixture first, drop if clean).
Items A and C: the gaps they assume exist at the cited lines; both are also framed causal-first.

## 4. Verdict

PREMISE HOLDS — `box_rules.rs:13-27,46-48` tests the requester's connector as a centre-line band only, and `manoeuvre.rs:75-85` skips bodies behind the nose on a connector, so a body in the rear-swing overhang is seen by neither the grant nor sensing; `lateral.rs:55-57` vs `graph.rs:115-139` confirm item C's gap; `civilian/mod.rs:449-451` and the probe's Wander/Flee filter confirm item A's stall is not a designed pause.
