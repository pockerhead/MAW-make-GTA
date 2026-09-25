# IMPL_REVIEW — TASK-016 (GDD T15, traffic and police cars)

Reviewer: code-reviewer (claude opus, effort high). Reviewed `318936a` on `feature/t15-traffic` against
`TASK_FINAL.md`, `PLAN_FINAL.md` and `IMPL_SUMMARY.md`. I read in full every new sim file: `traffic/*`,
`police/{cars,car_route,car_dispatch}.rs`, `vehicle/autopilot.rs`. I read the diffs of every touched sim file:
`vehicle/{chassis,impact,seat,mod,config}`, `police/{dispatch,arrest,behavior,mod}`, `wanted/*`, `tactics/*`,
`gang/behavior`, `civilian`, `population` and `lib.rs`. I also read the client diffs (visuals, sirens, minimap)
and the key gates.

The measurements come from a scratch probe crate at `scratch/cr_probe/`. It uses the production composition
and the real test helpers through `#[path]`, and builds into the shared `target/`. Its logs are
`scratch/cr_police_response.log`, `scratch/cr_route_diag.log` and `scratch/cr_traffic_probe.log`. I made no
project edits.

## 1. Verdict

**NEEDS_WORK.** The traffic core is correct and well gated, and every gate I ran is green. Two owner-visible
regressions have cheap, measured fixes:
- **Police response.** By car, the first cop reaches the park centre after 63 s. The main cause is a
  routing rule, not traffic.
- **Jams can lock a whole intersection.** A car that stands still just past a crossing, or inside it,
  blocks the entire box. This comes from the reservation `room()` check, which only sees AI cars.

Neither defect is silent state corruption. Both show up in the first minute of play and have cheap,
bounded fixes.

### Disconfirmation (done first)
**Counter-example tested:** the player stands on foot at the park centre at 2 stars. The F6 seat reservation
holds every foot slot empty while the police cars are held behind traffic queues. Stop-and-go traffic keeps
resetting `PoliceCar.blocked`, so no cop ever arrives.

**Checked:**
- `dispatch.rs` (`reserved = (row.cars − active)·crew`), `car_route.rs:235-239` (`blocked`) and
  `car_route.rs:267-301` (route) in code.
- The probe `police_response.rs`, seed 1 with the `police_city` setup.

**It held in part:**
- The 63 s is reproduced: first reach 63.08 s, first cop within 18 m at 69.7 s.
- `blocked` did fire. `blocked_max` reached 6.0 s on both cars, with no stop-and-go reset.
- F6 is not the binding cause. Row 2 is 4 units = 2 cars × 2 crew, so no foot seat exists once both cars are
  out; `reserved` is 0 from t ≈ 10 s.
- The dominant cause is the route detour (issue 1). Traffic adds a 6-8.5 s wait, then a far dismount (issue 2).

## 2. Confirmed correct (file refs)

**IDM and kinematic stepping**
- The IDM kernel and the ballistic integrator follow Treiber & Kanagaraj, with the worked rows as unit tests
  (`traffic/idm.rs`).
- The kinematic car is actuated in closed loop: velocity is set from the next pose (`drive.rs:390-436`), and
  a driver never runs an ungranted stop line (`drive.rs:405-409`).

**Intersection reservations**
- FCFS order is (tick, entity bits) with "don't block the box" room accounting (`junction.rs:108-147`).
- Deviation 1 (`+2·s0` in `request_distance`) is right. With `+ s0` the threshold equals IDM's rest point,
  so a car at rest never asks.

**Kinematic-to-dynamic switch** (`contact.rs`)
- It runs in FixedPostUpdate before `PhysicsSystems::First`.
- The relative-sweep SAT counts sleeping bodies (F4). `CollisionStart` is kept as a backstop.
- There are unit rows for the rear-end, crossing, oncoming, parked and 45° corner cases.

**Hijack, bail-out and the traffic bubble**
- Spawns insert `RigidBody` and `Name` after the spawn bundle, so there is no duplicate-bundle panic (F1):
  `spawn.rs:60-74`, `car_dispatch.rs:174-192`.
- Hijack spawns one driver only for AI modes (F5, `hijack.rs:90`).
- `abandon()` releases every reservation and removes the AI components (`traffic/mod.rs:188-202`).
- The bubble follows the Vermeij bands and the 2 s rule, and `Taken` cars are never despawned
  (`spawn.rs:78-120`).
- Drivers draw from `TrafficRng` stream 4 and police cars from stream 5 (TASK-010 rule).
  `roll_temperament` makes the same draws as before.

**Seats, pull-out and cabin hits**
- Forced eject never lands on a wall top (`seat.rs` `exit_spot`, `forced` path).
- Pull-out only goes through a clear left door (F7, `seat.rs` `pull_out`, `arrest.rs` `pull_out_driver`).
- A cabin hit writes `CabinHit`. `Health::take` goes through armour, and `DamageDealt` carries `killed`
  (`impact.rs`). `Bullets.before(Death)` makes the kill land in the same tick.

**Police dispatch and cars**
- The car dispatcher counts foot units plus crews aboard, and keeps `dispatcher.cars` current
  (`car_dispatch.rs:104-117`).
- `next_car_state` has one table case per row, including Abandoned and the re-board timeout (`cars.rs:349-571`).

**Cars in the fire line (O1)**
- The check is a segment-only rectangle test (`fire_line.rs` `car_blocks`), wired into `usable`, `pinned`
  and `hold_fire`. `queue_slot` stays shields-only (F11).

**Schedule**
- `Drive.after(PoliceSystems)` and `Bullets.before(Death)` add no cycle: every test builds the app.

**Data first**
- The only new `const` is `IDM_DELTA`, which is a law. Every other tuning value is in RON with strict
  loaders and validation. The exception is one literal (issue 1).

**Boundaries**
- File sizes: the largest is `police/cars.rs` at 572 lines (limit 750).
- Headless boundary kept: `gta_sim` gained no render dependency.

**Tests I ran, all green (`-j 4`)**
- `police_cars` (11), `traffic_intersection`, `traffic_idm`, `traffic_contact` (6), `traffic_bubble` (4),
  `traffic_hijack`, `traffic_bailout`, `police_pull_out` (5), `police_city`.
- `new_city`, `sensor_leak`, `vehicle_hits`, `vehicle_seat`, `car_fire_lines`, `config_traffic`,
  `traffic_graph`, `traffic_parked`.
- Branch CI for `318936a`: all five jobs green, including sim gates (run 36097346915).

## 3. Issues

### 1. MAJOR: the A* goal radius makes police cars loop blocks
**Where:** `crates/gta_sim/src/police/car_route.rs:271`

**What goes wrong:**
- `radius = nearest.max(c.dismount_distance * 0.75)`. When the target is far from any lane (park centre:
  nearest lane 43.3 m), the only accepted goal is the one directed lane within 43.3 m. A* then drives around
  blocks to reach it.
- Measured over all 130 lane points of the 60-120 m car ring around the park, route length is a median of
  376 m (4.1× the straight distance), p90 508 m, max 639 m.
- The two cars of the run planned 473 m and 578 m routes from spawns 60 m away. Even with traffic off, first
  reach takes 36.5 s.
- The factor 0.75 is also an inline tuning literal (data-first rule).

**Fix:**
- Accept any lane within `nearest + car.goal_margin` (a new `escalation.ron` field, 10.0).
- Measured route: median 91 m (0.9×), p90 216 m. With `nearest + 5` it is 133 m.
- This also removes the 0.75 literal.

### 2. MAJOR: a held-up car waits 6 s, then drops its crew 58-63 m away
**Where:** `car_route.rs:235-239` together with `assets/police/escalation.ron` `car.blocked_seconds: 6.0`

**What goes wrong:**
- In the park run both cars sat behind traffic, 6.0 s and 8.5 s, until `blocked` fired.
- They dismounted 58.2 m and 63.3 m from the player, at 24.5 s and 42.6 s.
- Rows 1-4 are all seats: units = cars × crew. So nobody is on foot until a dismount.

**Measured at first reach / first cop within 18 m, in seconds:**

| variant | park | hospital | plaza |
|---|---|---|---|
| shipped | 63.1 / 69.7 | 6.45 / 6.45 | 8.38 / 8.38 |
| `blocked_seconds 2` | 11.8 / 17.0 | 6.45 / 6.45 | 7.72 / 11.45 |
| `spawn_ring (30,60)` | 49.9 / 53.6 | 2.28 / 2.72 | 1.28 / 3.5 |
| both of the above | 8.1 / 14.6 | 2.28 / 2.72 | 1.28 / 3.5 |
| foot only (pre-T15) | 1.28 / 5.0 | 2.48 / never | 1.61 / 6.30 |

**Fix (smallest that keeps the GDD car table):**
- `blocked_seconds: 2.0` (data) plus issue 1's radius fix.
- The ring change alone does not fix the park (49.9 s). I do not recommend it, because it also shrinks the
  hidden spawn set.
- `blocked` counts from spawn, when v = 0 (`car_route.rs:235`). At 2 s, a car spawned behind a queue drops
  its crew on its hidden spawn point (probe: 1.98 s, 60 m out). In effect that is a hidden foot spawn, and it
  is acceptable. If the fixer wants to avoid it, count `blocked` only after the car first exceeds exit speed.
- After the fix, re-measure the three spots with the `police_city` prints and put the numbers in the summary.

### 3. MAJOR: `room()` ignores non-AI cars, so one stopped car can lock an entire intersection
**Where:** `crates/gta_sim/src/traffic/junction.rs:29-35` and `:133-134`

This is derived from the code; I did not measure it end to end.

**What goes wrong:**
- `room()` reads only the occupancy of AI cars (`drive.rs:40-56` builds it from snaps of AI modes). Several
  kinds of stopped car are not in it:
  - an `Abandoned` car
  - a `Taken` car the player parked
  - a Dismounted police car
  - a bailed-out car
- When one of these stands within about 6 m of a destination lane's start, the waiter is granted anyway.
- On the connector its forward cast only reaches `turn_sense_distance` (6 m) and stops it at s0 behind the
  obstacle. Its rear is then still inside the connector.
- The release rule keeps the reservation until the rear leaves (`junction.rs:65`).
- About 67 % of connector pairs conflict (46 of 66 at every 4-way node, probe). So every conflicting movement
  of that node is locked until the obstacle despawns.
- An obstacle that stays in view never despawns (see focus item 2).
- `:128-129` also pushes a room-denied waiter into `blocking`, which spreads the jam upstream. That is FCFS by
  design, but it makes this case worse.

**Fix and gate:**
- Before a grant, test the first `(queued + 1)·spacing` m of `to_lane` with a `shape_intersections` box on
  `Vehicle`, excluding kinematic AI cars (they are already in occupancy).
- Or project the non-AI traffic cars and police cars onto the lanes and add them to occupancy as static
  entries.
- Gate on the AC2 floor: put an abandoned car 3 m into one out-lane. No car may be granted into that lane, and
  the other approaches keep delivering (liveness). Flip: `room()` on AI occupancy only goes RED.

### 4. MAJOR: police cars park in the traffic lane, even inside an intersection, and stay there
**Where:** `crates/gta_sim/src/police/car_route.rs:342-362` (`hold` → speed 0) and `:389` (Dismounted → speed 0)

**What goes wrong:**
- A police car stops wherever `route_done`, `blocked` or `hold` fires. That can be on a connector, since
  `route_done` counts remaining path. It is always on an inner traffic lane.
- It stays `Dismounted` while its crew is out. With the player on foot the crew never re-boards
  (`cars.rs:158-162`, `board_police_cars` requires `driven_off`).
- It is not despawned while it is within 150 m (`car_dispatch.rs:71-74`).
- Result at 2+ stars on foot: a permanent lane block per car. On a connector it also becomes the case of
  issue 3, a locked box, and no reservation exists for it.

**Fix:**
- Never stop on a connector: keep `turn_speed` until the car is on a lane, then stop.
- With issue 3 fixed, the box stays passable.
- The remaining lane block needs the pass-around decision in section 6, item 2.

### 5. MINOR: the `police_city` window was doubled to absorb the regression
**Where:** `crates/gta_sim/tests/police_city.rs:20-22`, `CAR_SPOT_TICKS = 5120`

**What goes wrong:**
- The liveness gate now passes at 63 s, so it can no longer catch this regression.
- The doc comment on `chase` still says "40 s".

**Fix:** after issues 1 and 2, go back to `SPOT_TICKS` for both variants. Print the first-reach tick per spot,
as the probe does.

### 6. MINOR: two cops of a dismounted crew froze near the park
**Where:** found by measurement.

**What goes wrong:**
- Both cops of car A got out 58 m from the player and stood still from t ≈ 35 s to 80 s. One stood 44.8 m and
  the other 50.9 m from the player, at (89.6, -170.6) and (102.7, -173.5).
- Both were in `Respond` with `dest = player`, and `sees` was false.
- This looks like an older dead spot of the sidewalk routing, exposed because crews now get out far from the
  player.

**Fix:** check again after issues 1 and 2, which change where crews get out. If it persists, open a small
follow-up with the probe's positions.

### 7. MINOR: a crew member can get out onto the roof
**Where:** `crates/gta_sim/src/police/cars.rs:186-188`

**What goes wrong:** `dismount` zips the crew with every clear `exit_spots` candidate, and index 2 is the roof.
With one door blocked, the second cop spawns standing on the car roof.

**Fix:** filter out `k == 2` for crew spawns. A cop with no door keeps its seat, which the code already
handles.

### 8. MINOR: with no route, a car steers straight at the goal through blocks
**Where:** `crates/gta_sim/src/police/car_route.rs:359`

**What goes wrong:** `None => (goal_point, c.pursuit_speed)`. With an empty route, the car steers straight at
the goal, through blocks. This happens for one tick while `routes_per_tick` is exhausted, and permanently if
A* finds nothing.

**Fix:** use speed 0 (hold) until a route exists.

## 4. Orchestrator focus items

### 1. Police response time
- **Cause, in order:**
  - Route detour from the goal radius (issue 1): about 34-42 s even with traffic off.
  - Queues behind traffic until `blocked_seconds` (6 s), after which the crew gets out about 60 m away
    (issue 2).
  - The crews' walk of 43 m or more, with one crew stuck (issue 6).
- **Not the cause:**
  - Spawn distance: 60 m, the ring's inner edge.
  - The F6 reservation as such: row 2 has no foot seat once both cars exist.
- **Smallest fix:**
  - Radius `nearest + goal_margin` (10 m, data).
  - `blocked_seconds: 2.0`.
  - The GDD car table is unchanged.
  - Measured with blocked 2 alone: park 11.8 s, hospital 6.45 s, plaza 7.7 s. The radius fix removes the
    remaining detour.
- **"Foot units fill the reserved seats until the cars arrive":** the stand-in (row 2 `cars = 1`) gives park
  1.28 s and hospital 2.48 s. But it changes the car count per row, which is GDD table data. It is the
  owner's call if the car-only fix is not enough.

### 2. Gridlock
- **Abandoned cars do not despawn fast enough while the player stays near.**
  - Probe: an abandoned car on a lane 30 m from the player, in view. The queue behind it grows 0 → 3 at 10 s
    → 5 at 20 s, and holds 5 of the 8 AI cars until 60 s.
  - After the player moved 40 m away and looked off, it despawned 1.98 s later and the queue cleared in 5 s.
  - The R4 rule works as written (`spawn.rs:100-113`). It is Vermeij's rule, and it never removes a car in
    view.
- **Pass-around.** A minimal version (use the opposite inner lane when the oncoming lane is clear over the
  obstacle length plus margins) is the only real cure for a blocked lane. The GDD (§5.2) has no lane change,
  so this is a scope decision for the orchestrator/owner, not a fixer patch.
- **Required in this slice, in scope:** issues 3 and 4. They keep one blocked lane from locking an entire
  intersection.
- **Recommended:** a follow-up task for the pass-around, if the owner run confirms that lane blocks by
  Dismounted police cars and abandoned cars matter in play.

### 3. Intersection throughput, about 1 car per 4 s
- **This is a real cap of the design, not a tuning artifact.**
  - At a 4-way node 67 % of connector pairs conflict, and reservation covers the whole connector. It starts at
    request distance (`v²/2b + 2·s0 + half`, 49 m before the line at 12 m/s) and ends when the rear clears.
    So about one car is in the box at a time.
- **Measured:**
  - AC2 box, saturated: 1 car per 3.85-4.76 s, grant lifetime 5.4-5.7 s.
  - The same box with a = 2.0: 1 car per 2.9-3.2 s.
- **From rest, analytic:**
  - Median connector: 6.3 s at a = 0.73, 4.0 s at a = 2.0.
- **The acceleration value is GDD data** (§5.2, a = 0.73). Raising it only softens the cap.
- **Design mismatch:** the GDD says "одна резервация точки конфликта на машину" (a conflict point); the plan
  chose whole connectors.
- **If the owner run finds crossings slow**, the structural fix is:
  - a grant only within the braking distance plus s0 (not the whole approach);
  - release of the reservation once the car passes the last conflict point it shares with each waiter.

  This should not block the slice.

### 4. The deviations in IMPL_SUMMARY §2

| # | Verdict | Reason |
|---|---|---|
| 1 | **Accept** | Correct fix of the IDM rest-point equality (explained above). |
| 2 | **Accept** | `to_stop_line > 0` keeps the room; the start speed can always stop at b. |
| 3 | **Accept** | Standard approach cap; the plan's version braked at −8 inside the connector. |
| 4 | **Accept** | Needed for the flee origin. |
| 5 | **Accept, with conditions** | See the table below. |
| 6 | **Accept** | Test geometry only; the AC is unchanged. |
| 7 | **Accept** | A ratio gate (waste ≤ half of rule-off, police 8 vs 24, gang 6 vs 17) is falsifiable. Its empty-list flip is RED. Zero hits is unreachable without the spread cone, which PR1 rejected for starving shooters. O1 is reduced about 3×, not removed: note it for the owner run. |
| 8 | **Accept** | |
| 9 | **Accept** | Liveness is derived by measurement as the plan asked. AC2 ≥ 3 per approach matches the cap above. Bench mean 1.86 ms, within GDD §11. |
| 10 | **Split** | See below. |
| 11 | **Accept** | Correct reasoning on both flips. |

Deviation 5, part by part:

| Part | Verdict |
|---|---|
| Route radius | **Replace** (issue 1). |
| `route_done` by remaining path | **Accept.** |
| Slow before turns | **Accept.** |
| Turn cast | **Accept.** |
| Nearer door | **Accept.** |
| Chase ends on a stopped driver | **Accept.** |
| `blocked_seconds` | **Accept the mechanism, change the value** (issue 2). |

Deviation 10:
- **Accept** `max_cars = 0` for `street_spawn` and `civilian_bench`. Their subject is civilians, and plan step
  27 allows it.
- **Reject** the `police_city` 80 s window (issue 5).

## 5. Missing coverage
- **Police response time.** No gate or recorded number binds it. After the fix, `police_city` should print
  first-reach per spot, and I recommend a loose bound (for example ≤ 20 s) at all three spots on seed 1. It is
  a liveness gate: measure the spread over a few seeds before choosing the bound (gates domain: derived, not
  intended).
- **Box not blocked by non-AI cars (issue 3).** No gate. The row and its flip are described in issue 3.
- **Police car stopping.** No row asserts that a `Dismounted`/held police car never stands on a
  `Segment::Connector` (issue 4).
- **Siren height and the police/taxi scale.** These go to the owner run only; declining a gate here is right.

## 6. Nits
- `police_city.rs`: the `chase` doc comment says "40 s" and the window is 80 s.
- `traffic/spawn.rs` computes `clear` twice (`:187`, `:200`); one helper would do.
- `car_dispatch.rs:138-161`: a candidate that fails the `hidden` check is consumed with `continue`. That is
  correct, because the list shrinks, but a one-line comment would stop a reader from suspecting a spin.

children: 1 launched / 1 reported.
