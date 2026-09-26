# PREMISE_CHALLENGE — TASK-032

## 1. Counter-example tested

(Written before investigating.) In repro R1 (seed 1, car left at (2.4, -92.2) before junction (7.9, -81.3)),
the traffic car that stands 146 s is NOT stopped by the abandoned player car through any path the task fixes
(in-lane obstacle sensing, junction grant, bumped-Dynamic state), but by something else; and the control run
(`leave=0`, car driven 150 m on) is green because the player moved away, not because no car was left. If true,
the R1 evidence does not show "an abandoned car freezes traffic", and the R1 success predicate can fail after a
correct fix (or pass for an unrelated reason).

## 2. Primary-source investigation

Raw artifacts: `maw/tasks/done/TASK-031/scratch/sessions/repro_r1_leave/` and `repro_r1_control/`
(`summary.json`, `samples.json`, `actions.json`, `shots/`), script
`maw/tasks/done/TASK-031/scratch/tools/repro_abandoned_car.py`. `stopped_detail` fields are
`[x, z, seconds standing, waiting, entity]` (`tools/pt.py:491-493`).

a. Per-car stand durations, leave run (max over all samples, python over `samples.json`):
```
25769798564 (146.1 s, car at (16.4, -81.0), waiting None, t 184.31)
47244633971 (11.4 s, (-0.6, -69.5))
34359733052 ( 9.6 s, (-22.7, -79.3))  ... every other car <= 9.6 s
player at end: (11.95, 1.11, -80.14)
```
The ONLY car standing longer than 11.4 s is the 146 s car (the one the task cites). It is 4.5 m from the
player and ~18 m from the abandoned car at (2.4, -92.2). The player stands still at (11.95, -80.14) from the
t=38.23 sample on; that same sample shows the 146 s car with stand time 0.0, and it stands from then to the end
(18.4 s at t=56.6, 37.1 at 75.3, ... 129.5 at 167.8). Its `waiting` is `None` (not waiting for a junction
grant). Queues on the west arm (x -16..-53) cycle with stands <= 10 s.

b. The player is on the roadway, not the sidewalk: `repro_abandoned_car.py` walks to `me + (12, 12)` with a
10 s timeout ("to sidewalk", `actions.json` target (12.8, -79.4)); screenshots
`shots/0007_0033s_repro_watch.jpg` and `0018_0080s_repro_watch.jpg` show the character standing on asphalt
next to the lane marking, crosswalk and junction ahead.

c. Traffic sensing stops for characters: `crates/gta_sim/src/traffic/drive.rs:305` casts against
`[GameLayer::Character, GameLayer::Vehicle]`; `drive.rs:364-377` turns the hit into an IDM obstacle with the
hit body's speed (0 for a standing player). Car z -81.0 vs player z -80.14: 0.86 m lateral offset, inside the
car's slab width, so a car heading to the junction on that arm has the player in its cast.

d. Control run is not like-for-like: control `car_left_at` (-7.7, -199.4), player at end (1.67, -189.6),
~108 m from the junction. `spawn.rs:98-121` despawns anything past `in_view.despawn` = 90 m
(`assets/traffic/traffic.ron:29`), and in-view spawns happen only at 70-90 m. Result in the artifacts: cars
within 45 m of the junction, mean 1.60 (control, histogram 1:59, 2:62, 3:8) vs 7.71 (leave). The control
removes the player from the junction together with the car, and thins the measured zone about 5x.

e. Cross-check that M1 is not wholly wrong: in `s1_tourist`, `s7_tourist`, `s7_reckless`, `s42_reckless` there
are 5-13 cars standing > 60 s (up to 159 s) on several approaches, with 3-6 `Dynamic` traffic cars alive at the
time (`tstats.dynamic`). Those sessions do support the bumped/abandoned-car gridlock class; I did not attribute
each of those standers.

## 3. Did it hold

The counter-example held for R1. The task's named repro R1, its cited "a traffic car stands 146 s", and the R1
acceptance predicate rest on one car that is held by the stationary player standing in its lane (character hit
by the forward cast), not by the abandoned car. Nothing else in R1 stood over 11.4 s. The control differs from
the leave run in the player's position (108 m away, outside the 90 m bubble), so "control: no car > 20 s" does
not isolate the abandoned car. The task's standing-obstacle list in item C (abandoned/parked car, dismounted
police car, unrecovered bumped car, wreck) does not include a character, so a correct A-E implementation can
still fail R1 as written (the same car stands behind the same player), or R1 can go green only because QA's
re-pointed script puts the player somewhere else. The broader M1 class (bumped Dynamic cars, multi-approach
freezes) is supported by the other sessions and is not refuted.

## 4. Verdict

PREMISE SUSPECT — `repro_r1_leave/samples.json`: the only R1 car over 11.4 s (entity 25769798564, 146.1 s at
(16.4, -81.0)) starts standing when the player settles 4.5 m away on the asphalt at (11.95, -80.14)
(`shots/0018_0080s_repro_watch.jpg`) and 18 m from the abandoned car, with traffic casts hitting characters
(`crates/gta_sim/src/traffic/drive.rs:305,364-377`); the control puts the player 108 m away, outside the 90 m
despawn bubble (`spawn.rs:98-121`, `traffic.ron:29`), near-junction cars 1.6 vs 7.7 ; smallest implied reframing:
R1 does not demonstrate the abandoned-car freeze — the R1 repro/gate must place the stationary player off the
roadway (same spot for leave and control) before it can stand as evidence and success predicate for M1, and M1's
causal evidence should come from the s1/s7/s42 sessions, not R1.
