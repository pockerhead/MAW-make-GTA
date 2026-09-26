# PCTX proposals — TASK-033 (implementer)

## 2026-09-26 — game-design risk lesson (traffic junctions)
The seed-1 "gridlock with nobody playing" had three causes that only show together in the real city:
(1) a follower got a junction grant behind an ungranted lane head whose connector conflicted (circular
wait); (2) citygen puts the crosswalk centre line 0.01-2.95 m BEFORE every lane end, so a car waiting at
the old stop line (lane end) stood on the crosswalk and walkers pressed into its nose while it waited for
them; (3) no grant ever expired. Rules now: only the first car of a lane queues, a grant is a lease
(`reservation_timeout`, lapses only before the stop line), the stop line is derived per lane from the
crossing (`crossing_clearance`). Trigger: `TrafficIntersections`, `lane.stop`, `junction::update`.

## 2026-09-26 — gates risk lesson (traffic liveness)
Traffic-health probes must include a stationary player facing a junction with the production
population: the Vermeij bubble keeps in-frame cars and churns off-frame ones, so ~15 of 22 cars pile up
in view and the junction in front saturates (the TASK-016 probes had a moving player/police and missed
the gridlock). A "queue behind a moving leader" excuse cannot tell a lease livelock (grant, stand, lapse,
re-queue) from a saturated queue: gate gridlock by the stand-duration bound derived from the unfixed runs
(87-117 s) vs the fixed ones (<= 39 s), and gate the lease and the stop-line geometry separately.
