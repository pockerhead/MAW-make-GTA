# REDESIGN NOTE — TASK-032 continuation: G6 (police close in) and R1 (box, in view)

Written by the implementer (continuation run) after the pre-decided G6 fallback chain ran out. No
further patch on this class. G5, G7 and the rest of stage 6/7 went on.

## What failed

G6 as specified: the player's car flees at 12 m/s, three AI cars queued behind it, a responding police
car 40 m behind the last one; a police unit within 18 m within 25 s in >= 8/10 seeds.
Fixture: `crates/gta_sim/tests/police_close_in.rs` (both rows `#[ignore]`d with this note as reason).
Numbers: `scratch/g6/*.txt`.

| Road | Traffic | Mechanisms | Pressure (<= 18 m in 25 s) |
|---|---|---|---|
| street | production | yield + any lane (shipped) | **0 / 10** |
| street | none (control) | shipped | 10 / 10 (all at ~6.8 s) |
| avenue | production | yield + any lane (shipped) | **2 / 10** (seeds 1, 3) |
| avenue | none (control) | shipped | 7 / 10 (seeds 2, 5, 8 never under 40 m) |
| avenue | production | flip: `yield_distance 0`, `lane_offsets []` | 0 / 10 |
| street | production | same flip | 0 / 4 (seeds 1-4; the run stopped at seed 5 on a G1 violation, below) |

So the mechanisms do work in isolation (G5 green on seeds 1/7, floor row green, the flip is worse on
avenues), but they are not enough to meet the number.

Fallback chain from TASK_FINAL, all tried:
1. Streets: oncoming lane for sirens + oncoming traffic yields to its own curb (built).
2. Avenue-only G6 (built): 2/10.

## Why (observed in per-tick traces, seeds 1 and 2)

- **Streets, physical limit** (named in the plan): a street is 6.5 m of asphalt. A car yielded to its
  curb by the 0.425 m slack and an oncoming car yielded to its own curb leave 1.7 m between them; the
  police car is 2.4 m wide. Wherever the two rows overlap along the street the police car weaves at
  2-5 m/s or stands until the 8 s yield timeout.
- **Avenues, the police car's own driving**: with no traffic at all 3/10 seeds never close in. The
  Respond route slows to `turn_speed` (8 m/s) in every junction box, straight connectors included, and
  inside a box the lane choice is off (heading cast), so a yielded car just past a box holds the police
  car in the box. With traffic, yielded cars in the home lane and oncoming yielders whose curb lane is
  taken stand in both inner lanes.
- **The bubble**: with the camera looking ahead (normal play), traffic more than 25 m behind the
  driver is despawned off frame, so the queue this gate is about mostly vanishes in real play. The
  fixture has to turn the camera back to keep it (named mutation).

## Fixes made while diagnosing (kept: each is a mechanism bug, each gate still green)

- `police/siren.rs` `lane_frame`: the reference lane is the nearest lane pointing the car's way within
  1.5 pitches (before: nearest lane, so a car out in the opposite lane lost its frame and swerved back);
  box margin 0.
- `police/car_route.rs`: lanes compared from the tail (a car still alongside keeps the lane taken), over
  `max(sense_distance, v * lane_hold_seconds + v^2 / 2b)` at speed.
- `traffic/sirens.rs`: no yield without a way past (no curb lane free and no opposite lane); no yield
  within a car length + jam gap of the lane start (the siren car would stand in the box behind it).
- `traffic/sirens.rs` `yield_gap` = v^2 / 2b (the plan's `+ s0` is IDM's rest point: the car crept
  forever and never stopped); `drive.rs`: a yielding car shifts first (at most `pass.speed`), then stops.

## Options for the orchestrator

A. **Rescope G6 to the drama system as the player meets it** (recommended): dispatcher on, production
   police (spawn ahead/beside by sector shares, G7), measure "any unit within 18 m in 25 s". This is
   what t15 measures at runtime (TASK_FINAL keeps t15 >= 5/6 as the player-facing criterion). The
   yield/any-lane mechanisms keep their own gates (G5, the floor row
   `police_car_gets_past_a_traffic_queue`, both flip-RED). Cost: G6 no longer isolates the yield
   mechanism; the spawn-ahead cheat may dominate.
B. **Police driving with sirens** (new mechanics, not in this plan): straight connectors at pursuit
   speed, lane choice kept through the box. Addresses the 3/10 no-traffic avenue failures only; the
   street limit stays.
C. **Derive G6 against the no-traffic control per seed**: the fixed quantity is the police car's own
   drive on an empty road; gate "with traffic, the police car closes in within the control time + X s"
   on seeds whose control closes in. Needs an OPEN_DECISIONS entry for X.
D. **Cheat**: a siren car out of the player's view is not held by traffic (ghosting or catch-up speed),
   like the stuck-despawn cheat. Cheap, invisible, GTA-like (rubber-banding).

## Separate finding: G1 violation outside TASK-032's code (junction conflict table)

In the street flip run (seed 5, `yield_distance 0`, `lane_offsets []`, i.e. siren features off) the
G1 oracle caught two kinematic traffic cars interpenetrating by 0.34 m on connectors 126 and 127 of one
junction, both granted at once (trace `scratch/g6/g1_flip_trace.txt`). Both are left turns from adjacent
approaches; the conflict table (`traffic/graph.rs`, polyline distance < 2 x half width + margin) does
not count the corners of a car swinging out on a curve. That rule predates TASK-032 (TASK-016/033) and
none of this task's changes touch it; every production-config run of this task is clean (G1 depth 0).
Candidate follow-up: conflicts from swept car rectangles, gated against `traffic_gridlock` throughput.

## R1 runtime: the accepted in-view box residue is the R1 scenario itself

The orchestrator's stage-5 note accepted "a rare lock the player is looking at" (the box class hit the
stop rule twice at stage 5). R1 at runtime is exactly that case, and it is not rare: the script drives
the car into the junction (7.9, -81.3) in front of the spawn, leaves it there, and the player watches
from 31 m. With the final code (`scratch/runtime/`, per-car stands of traffic within 45 m):

| Run | Car left at | Cars standing > 30 s | Longest |
|---|---|---|---|
| leave, run 1 | (7.9, -81.0), box centre | 15 | 149.1 s |
| leave, run 2 | (6.3, -84.9) | 12 | 149.7 s |
| leave, final code | (5.9, -86.7) | 11 | 149.6 s |
| control, final code | (-16.7, -115.4) | 0 (none over 20 s) | 9.5 s |
| baseline leave (pre-TASK-032, stage 1) | (6.3, -84.6) | 0 | 9.2 s |

So at the baseline spot R1 is **worse than before TASK-032**. What the traces show: before, a car that
touched the left car switched to `Dynamic` and its autopilot pushed through (shoving the left car); now
the D1 rule grants no connector whose path a body stands on, and an approach car that did touch it and
stood `recover.give_up_seconds` without a clear rejoin corridor is given up (`Abandoned`: in all three
leave runs the east approach head at (14.1-14.4, -81.0) is `Abandoned` for ~150 s). Two bodies then
stand in or at the box, and no pass fits (the stage-5 class). The stuck cheat does not apply in view.

Options:
- R-A. **Push through in the box** (the pre-TASK-032 behaviour, GTA-like): a car held by a standing
  body in the box goes `Dynamic` and its autopilot shoves through instead of waiting; the give-up rule
  does not abandon a car inside a box (it keeps driving dynamic until out). Cost: G4 and cause (d) rows
  change meaning (contact expected); G1 counts only kinematic interpenetration, so it stays valid.
- R-B. **Cheat in view at distance**: extend the stuck cheat to bodies in frame but farther than X m or
  at the frame edge. Visible pop-out risk; the owner judges.
- R-C. **Accept** R1 as the residue and change the R1 criterion (the orchestrator's stage-5 rescope
  taken literally). The M1 finding (TASK-031) stays partly open for this case.

Civilians within 8 m of the left car (reported, not asserted): 23 / 18 / 9 civilians in the three leave
runs, longest 114 s / 11 s / 16 s; 0 in the control.
