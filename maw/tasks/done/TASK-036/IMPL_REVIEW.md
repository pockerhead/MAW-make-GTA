# IMPL_REVIEW — TASK-036 (commit 64ad5bf)

Reviewer: code-reviewer (claude opus, medium). Inputs read from disk: TASK_FINAL.md, PLAN_FINAL.md,
IMPL_SUMMARY.md, OPEN_DECISIONS.md, log.jsonl, the full production diff and the three gate files.
The three open items are the orchestrator's decisions (last OPEN_DECISIONS entry); this review does not
re-argue them. For item 3 it checks the guard and derives its distance (section 6).

## 1. Verdict

**NEEDS_WORK.** The three production changes do what the plan says, and I found no correctness bug in them.
The tree is still red, though: `traffic_causes::r1_car_left_in_the_box_seed_7` fails on Windows and Linux,
and `walk_arrival::a2_spot_b_heading_90` fails on Linux. The C fix also has a known G1 (0.479 m). All three
are waiting for the fixer. Two gates also need their preconditions hardened before the guard lands
(I-3, I-4).

## Disconfirmation (done before the review)

Counter-example I looked for (focus a): **"`civilian::reached` lets a walker take its next edge at a node
that is NOT under a car, or pass a crosswalk edge without crossing."**

- Non-covered node: **does not hold.** The covered branch needs `car_blocks(target, target, car, clearance)`,
  which means the lane target is inside that same car's rect grown by 0.3 m, and the walker is inside that
  same car grown by 1.3 m (`civilian/mod.rs:429-442`). With the degenerate segment, `car_entry` is a
  point-in-grown-rect test (`fire_line.rs:124-140`: `q = 0`, so only `|p| > h` rejects). A target outside
  every standing car falls back to the plain `arrive_radius` test. Unit rows (e) and (f) pin both parts.
  `standing_cars` keeps a car when its centre is within `2.0 + |half|` = 4.37 m of the walker. The reach zone
  is at most `|half + 1.3|` = 4.17 m from the centre, so the car is always in the list when the branch
  could fire.
- Crosswalk: **holds only in one narrow geometry, not reproduced.** An edge can be "passed without moving"
  only if both of its lane targets lie under one car. The car grown by 0.3 m measures 4.68 x 3.0 m. The only
  edges that short are the 4.5 m alley-mouth crossings (`graphs.rs:65-78`, alley walk offset 2.25 m; street
  crossings are >= 9.5 m). They lie on the street sidewalk line, so this needs a car parked on the sidewalk
  across an alley mouth, aligned with the crossing. A Wander walker then passes both nodes and walks round
  the car to the next node, which is harmless. A Flee walker could ping-pong: `flee_next` does not exclude
  `from` (`navigation/mod.rs:169-181`), and `Flee.left` only drains with speed. See I-6 (minor). In every
  other case, the next leg starts up to ~4.2 m from the node (the reach zone), which is a diagonal start and
  not a skipped crossing. Traffic sees walkers as bodies, not by crosswalk.

## Log triage

The only `dead_end` is planner #1 (counter-flow refuted). It holds on the Windows trajectory. The Linux
A2 red is a *new* head-on case at a corner target, which the fix exposed (OPEN_DECISIONS 2). It does not
contradict #1. I checked the decision entries #11 (B flips) and #12 (C stop) against the tree and the
scratch outputs. Both are consistent.

## 2. Confirmed correct

- **A `reached` / `arrive`** (`civilian/mod.rs:427-470`, `:547-576`): implemented as the plan specifies.
  `cars` is computed once and shared by `arrive` and `around_cars`. `around_cars` reads the new edge's target
  after `arrive`. No new tuning value: `clearance` and `corner` are the same expressions `around_cars` gets.
  The import is correct (`tactics/mod.rs:6-7` re-exports `car_blocks`). Unit rows (a)-(f) match hand
  computations through `car_entry`.
- **(a) early arrival:** none at a non-covered node (see disconfirmation).
- **B `connector_body`** (`box_rules.rs:14-40`): real half extents, `from_s.clamp(0, length)` up to and
  including `length`, steps of at most `CORRIDOR_LATERAL_STEP`, each subdivided by
  `ceil(turn / 7.5°)`. The loop terminates (`step > 0` while `s < length`). `connector_rects` is gone and
  has no other user (grep). All three `junction.rs` callers pass `from_s`. `repick` reuses `from`, which is
  valid within `REPICK_WITHIN`.
- **(b) real body vs G1 for co-granted pairs:** not affected. `connector_clear` skips AI cars granted at
  the node (`box_rules.rs:55-60`), so co-granted pairs are still guarded only by the conflict table
  (`body_sweep`, grown by margin/2, TASK-038). The real body only decides against *ungranted* bodies. There
  the oracle tolerance (0.02 m) equals the two `blocked` shrinks (`query.rs:109-126`). Stop-line queue heads:
  the rv2 probe gives 0 Q hits at grow 0 and 0.05 on the + floor and seeds 1/2/7/42
  (`scratch/rv2/neighbour.txt`), so the real body keeps at least 0.05 m of slack to every other lane's head.
  B5 flips on it.
- **(c) cost:** I re-ran `traffic_bench` 4 times on the fixed tree: 1.872 / 2.146 / 2.018 / 1.895 ms. The
  pre-change 1.885 ms falls inside that spread, and the implementer's 2.088 ms is one sample from the same
  distribution. So no whole-tick regression is measurable (limit 19 ms). Per call the sweep has ~10-45 rects
  against ~7, and each rect scans all bodies (`query.rs:182-195`). This is a possible future optimisation,
  not a finding (N-3).
- **(d) end of yield on a connector:** no pose jump. `Yield` and `None` both have
  `holds_offset == false` (`manoeuvre.rs:47-49`), so `effective_lateral` uses the same decay before and after
  the switch (`lateral.rs:47-60`). `before` and after are computed in the same motion step with the updated
  manoeuvre (`drive.rs:486`, `:547`). On a connector, `car.lateral` is not stepped for either
  (`drive.rs:543`). `manoeuvring` stays true while `lateral != 0`, so the heading keeps the rate-limited
  `turn_towards`, and no yaw cap applies on connectors in either state. At the connector end `lateral` is
  zeroed (`drive.rs:519-521`); the decay is 0 there. The only step change is `v0` (no `pass.speed` cap), plus
  one tick of yield braking (named, as the plan asks). The C trace shows the transition as continuous
  (`scratch/stage3/c_red_cell_trace.txt`: lat 3.25 on both sides of `Yield`→`None`).
- **Gates:** each row builds its own `App` (independent). The `GATE BROKEN` preconditions are real: A1 car
  in occupancy, standing, covering all three targets, never moving; A2 covered node found, not hard-coded,
  and a walker targeting it; B1-B3 conflict, turn class, queue stamps; B4 window < `character_seconds`, F
  within half a car of the connector start, room < 2 cars; C1 granted before the yield; C2 curb lane with
  pitch 3.25. The flips are recorded per row in `scratch/stage1|2|3/*flip*`.
- `traffic_junction_box` `on_path` (`:59-80`) mirrors the new rule, and it is stricter (0.1 m steps, no
  shrink), so it cannot pass a holder that production would treat as blocked.
- Files stay under 750 lines. No `unsafe`, no `unwrap` in production, no new `const`. Only `gta_sim` changes.

## 3. Issues

**I-1 (major, decided, fixer): the suite is red.** `crates/gta_sim/tests/traffic_causes.rs:468`
(`r1_car_left_in_the_box_seed_7`) is red on both platforms and `walk_arrival.rs:280` (A2 heading 90) is red on
Linux, so CI would be red. Fix per OPEN_DECISIONS 1 and 2. For item 2 (a `keep_right` shift on the corner
target), note the geometry: `around_cars` puts corners `corner` = 0.8 m out. A shift to the right of travel
moves the corner *towards* the car for a walker that has the car on its right: 0.8 - 0.5 = 0.3 m, which is
exactly the line clearance and puts the capsule against the body. Shift only outward, away from the car
(the sign of the corner's perpendicular to the travel), or cap the shift at `corner - clearance`. The
"corner within `corner - clearance` counts as reached" rule (`fire_line.rs:194`) measures to the
*unshifted* corner. Gate the new row on both platforms and flip it (shift 0).

**I-2 (major, decided, fixer): C-G1 0.479 m.** `sirens.rs:45-49` + the missing lane-end guard. The
derivation is in section 6.

**I-3 (major, gate honesty): the C sweep and the C rows bypass the start rule, so the guard will not be
exercised by them.** `scratch/probe/ws/probe/tests/c_sweep.rs:178` and `traffic_box_overhang.rs:421-428` set
`Manoeuvre::Yield` directly and never go through `sirens::update`'s start decision. After the guard lands:
- The Stage 3.3 re-run as it is still starts curb yields 6-12 m before the stop line, inside the guard.
  It stays red for a state production can no longer reach (or goes vacuous if someone "fixes" the
  fixture). The sweep needs a real siren car (or a call into the start rule) and must span the guard boundary
  (e.g. `need - 5 .. need + 10` m). Each cell records which offset was chosen (curb / slack / none) as a
  precondition.
- C1/C2 only assert `entered` the connector (`:453-456`). They do not assert that the car entered it *while
  in `Yield`*. If the guard or a "keep" rule ends the yield before the box, the C flip (plain `return None`)
  can stay GREEN silently. Add: the car was on connector `c` with `Manoeuvre::Yield` for at least one tick.
  Then re-flip.
- C2 (a curb yield begun 10.2 m before the stop line) becomes a state production cannot create. Keep it
  as a direct-state row for the "yield ends in the box" rule, say so in the header, and add a row through
  `sirens::update` that shows the curb yield refused (slack chosen) inside the guard.

**I-4 (minor, gate precondition): B4 does not assert that W actually contests the lease.**
`traffic_box_overhang.rs:314-318` checks only that c5 conflicts with c1. If W never queues (for example after
a `request_distance` change), `contested` stays false, H is never stale, and the `from_s = 0` flip stays
GREEN. Add `GATE BROKEN` unless W is a waiter for c5 at the node from the first tick through the window.

**I-5 (minor): `from_s` is 0 for a holder whose nose is on its exit lane.** `junction.rs:57-64` with
`:115-121`. An occupant stays on `Lane(to_lane)` while `s - half < 0` (`:174`). There `from_s` returns 0, and
the sweep runs over the whole connector *behind* the car. Today this is harmless: `stuck` is read only in the
source-lane and connector arms (`:163-172`). But `body_blocked` is computed for nothing, and the next reader
may take it as meaningful. Return `length` (or skip) for `Segment::Lane(to_lane)`.

**I-6 (minor, not reproduced): Flee ping-pong across a two-node covered edge.** This needs a car on the
street sidewalk across an alley mouth, aligned with the 4.5 m crossing (both lane targets inside the car
grown by 0.3 m). `flee_next` may return `from`, and each arrival fires the next tick beside the same car, so
`GraphWalker` can flip p↔q every tick. `Flee.left` drains only with speed (`civilian/mod.rs:409-414`), so a
dithering walker could flee for a long time. This is rare and the player causes it. Record it; a probe
(park a car across an alley crossing, one fleeing walker) decides it. A possible cheap rule, if it
reproduces: the covered branch does not fire when `from`'s lane target is under the same car.

## 4. Missing coverage

- A row where the ungranted standing body stays standing for more than 3 s (X's own path held by a
  dummy), so that Y's real-body sweep is checked over time and not only for the first ticks before X leaves.
  In B1-B3 X is granted in tick 1. The long-stand path (Y body-blocked, then whole-box) is only covered
  indirectly by R1 seed 7, now ignored.
- The lane-end guard rows (section 6): the start refused or downgraded to slack inside `need`, the curb
  yield stops before the stop line outside it, and the replay unit row.
- A flip for the corner `keep_right` row on Linux (I-1).
- I-3's "entered while in `Yield`" precondition on C1/C2.

## 5. Nits

- N-1: `arrive` (8 parameters) and `repick` (8) gained `#[allow(clippy::too_many_arguments)]`. The pairs
  `cars, (clearance, corner)` and `from_s, half` could be one small struct each. Style only.
- N-2: `standing_cars` now runs for every living civilian, including Idle, Cower and Report
  (`civilian/mod.rs:547`), and `reached` runs before `arrive` bails out for non-moving states (`:456-462`).
  This is cheap (civilian bench shows no change), but it could be gated on Wander/Flee.
- N-3: `connector_body` calls `pose(s)` twice per step (`box_rules.rs:34-36`), and `blocked` scans every body
  per rect. If the bench ever moves, the first lever is a one-pass candidate filter per call (bounding
  circle of the sweep), or caching the `from_s = 0` sweeps per connector (static geometry, most callers).
- N-4: A1's stand check uses a 10 s window inside an 11.7 s run, so it can only fire if a walker stands from
  t < 1.7 s. It is nearly vacuous; the arrival assertion carries the gate. Say so in the header, or shorten
  the window for A1.
- N-5: Real body + 0.01 m shrinks + corridor sampling: the worst unseen overlap is the 0.02 m shrink plus the
  corner sagitta, ~0.008 m on a 1.7 m right turn (corner radius ~3.55 m, half-step 3.75°), so ~0.028 m
  against a 0.02 m oracle tolerance. It is only theoretical. PLAN §7 already names it; `traffic.md` could
  say "guaranteed by the oracle gates, not by the step".
- N-6: `traffic.md` has three "Open (TASK-036, for the orchestrator)" paragraphs. The fixer must rewrite
  them to the decided state (ignored under TASK-039, corner shift, guard).

## 6. Item 3: is the lane-end guard the right root fix, and its distance

**Root fix: yes.** The conflict table and the `traffic_graph` oracle assume a car enters its connector on
its line, within the lane slack. The static thresholds (`scratch/geometry_c.txt`) say co-granted bodies touch
only from an entry offset of 2.29-2.46 m (decay) or 2.74-2.89 m (stepped Rejoin). Only the avenue curb
yield (3.25 m) exceeds them; the slack yield (0.425 m) touches nothing. The red cell trace shows A granted
*before* its yield (`c_red_cell_trace.txt`, t 0.00 `granted true`), so a grant-side check would not catch
it. Refusing the curb offset near the box restores the table's assumption at its source, keeps the C lock
fix, and leaves the slack yield, which is safe in the box. Build it as a condition on the curb branch
(`sirens.rs:103-113`): `curb_free && room >= need` picks `pitch`, else the existing slack branch. It should
not be "no yield".

**Distance, from the code path (per tick, `drive.rs` order):**
- The offset moves by `rate(v)·dt` per tick, with `rate(v) = rate_at_rest + slope·v` (`config.rs:89-91`,
  `lateral.rs:35-42`), 0.8 + 0.15 v.
- During a Yield `v0 <= pass.speed` (6 m/s, `drive.rs:381-383`), so the speed never exceeds
  `v_hi = max(v_start, pass.speed)`.
- Shift distance `D_shift = ∫v dt = ∫ (v / rate(v)) dl <= offset · v_hi / rate(v_hi)`, because `v/(r0 + k v)`
  increases with v (`idle` guarantees the start is at lateral 0).
- The yield stop starts once `|lateral - offset| < 0.05` (`drive.rs:399`), against a virtual obstacle at
  `yield_gap(v) = v²/2b`. With the shipped IDM, `s*/g >= 1.055 + 5.01/v + 6.68/v²` gives a deceleration
  >= b at every v. So the tail is `<= yield_gap(v_hi)`: a proven bound, but loose.
- Reference point: the nose to the stop line, `room = lane.stop - (car.s + half.z)`. The car then never
  stands on the crosswalk, and never reaches the connector (the centre enters at `lane.length >= lane.stop`).

Replay of that exact tick law (`scratch/cr/yield_settle.py` → `scratch/cr/yield_settle.txt`, f64, 64 Hz),
curb offset 3.25 m:

| v_start m/s | replay: distance to stand | proven `need` = shift(v_hi) + yield_gap(v_hi) | tight `need` = shift(v_hi) + yield_gap(pass.speed) |
|---|---|---|---|
| 0-6 | 7.15-13.74 | 22.25 | 22.25 |
| 10 | 15.18 | 44.07 | 24.91 |
| 16 | 18.34 | 92.90 | 27.03 |

(The replay's tail after the shift is at most 3.86 m at v_start 16.) The C-G1 red cell began at about
11.1 m of travel before the lane end at ~6 m/s, which is inside `need` either way.

**Recommendation:** use the tight form, `need = offset · v_hi / lateral.rate(v_hi) + yield_gap(pass.speed)`,
with a unit row that replays the real `idm_acceleration` / `ballistic_step` / `step_lateral` for
`v_start ∈ [0, max lane v0]` (0.5 m/s steps) and asserts `replay <= need`. That way the number is derived
through the code path (gates domain), not intended. Flip: drop the `yield_gap` term and the row goes RED.
The proven form is safe too, but it disables curb yields within 44-93 m of the stop line for fast cars.
It needs no separate "keep" check: the start bound is conservative for the whole manoeuvre. A "keep" rule
that ends a curb yield mid-shift near the box would produce a Rejoin entering at up to 3.25 m, so do not
add one. A car that stood at its curb offset right at the stop line and then rejoins from rest travels
>= `half.z` (2.04 m) before the connector: >= 1.65 s at 1.5 m/s², so >= 1.32 m of rejoin, leaving a
residual <= 1.93 m, which is under the 2.74 m stepped threshold.

children: 0 launched / 0 reported
