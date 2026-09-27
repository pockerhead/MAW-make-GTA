# IMPL_REVIEW — TASK-037 (in-view box lock)

Reviewer: code-reviewer (claude opus, medium). Reviewed commit `a66814a` (artifacts `2e6d174`), branch
`feature/box-lock-in-view`. Loaded: `TASK_FINAL.md`, `PLAN_FINAL.md`, `IMPL_SUMMARY.md`, `OPEN_DECISIONS.md`,
`log.jsonl`, the full diff of all 15 files. The orchestrator's decisions on open items (1)-(4) are taken as
given and are not re-argued here.

## 0. Disconfirmation (done first)

Counter-example chosen: **the `around_cars` "reached corner" fix (b) does not end the corner stall for a
target that is near and diagonal past the car (the police arrest point, the driver's door). A walker
that leaves the reached corner A towards B re-picks A 0.5 m later, because `via(A) = |from-A| + |A-to|`
counts a line A -> to that the car still blocks.**

**Held.** An exact replica of `around_cars` at `a66814a`
(`scratch/cr/around_cars_dither.py`: same `car_entry`, same corner and clearance values, the sedan's
half extents, walk 1.8 m/s, run both as instant velocity and with Tnua-like 30 m/s^2 acceleration):

| Scene | old code | new code |
|---|---|---|
| unit-test target (0.5, -4.0), walker from (-4, 6) | stalls on corner A (-2.0, 2.84) | dithers at (-2.00, 2.28), 0.5 m from A, target flips about every tick |
| cop from the passenger side (5, 0) to the door (-1.7, -0.3) | stalls on corner (2.0, -2.84) | dithers at (1.51, -2.84) |
| the new gate's crossing (target 8 m past the car) | arrives | arrives |

Derivation for the door case: from the point s along A -> B, A wins while
`2s < |AB| + |B-to| - |A-to| = 4.0 + 2.56 - 4.49`, so for s < 1.03 m. The reached radius is
`corner - clearance` = 0.5 m, so the walker turns back at s = 0.5 and never leaves the window
(overshoot at 30 m/s^2 is about 0.05 m). This is **not a regression**: the old code stalled on A in the
same scenes. But the claim "(b) fixes the re-pick" is only true when the target is far (the gate and
the G4 row). Police door approach from the passenger side stalls before and after. See issue M1.

## 1. Verdict

**NEEDS_WORK.** The stage 1-3 mechanisms are correct and well gated, and I re-ran and confirmed the green
rows. But `traffic_go_around::avenue_seed_2` is red on Windows (re-run here, item 4, already assigned to
the fixer). Two major issues also need a fix or an explicit deferral:
- M1: `around_cars` stalls/dithers for near diagonal targets.
- M2: a recovered, ungranted car in the box becomes `OnPathTraffic`, so plain crossing cars stop
  seeing it.

## 2. Confirmed correct (verified in code and by running tests)

- **Connector recovery band** (`recover.rs:185-195`, `reach_across` `:133-141`). I checked the
  containment argument (review focus d). Take the reach `|δ| + hx|cosθ| + hz|sinθ| <= hx + m/2` = 1.35.
  It caps the yaw error at about 4.8 deg when δ = 0. The along overhang `hx·sinθ` is then at most 0.10 m,
  under the 0.15 m growth. So the footprint lies inside the table's grown body posed at `s`, and against
  co-granted connectors G1 holds by construction. The limit comes from existing data (`conflict_margin`);
  no new const.
- **Rejoin stepped, held and sliding on connectors.** `holds_offset` is `manoeuvre.rs:45-48`. In
  `drive.rs` it is used at `:477`, `:534` and `:538`. The rejoin survives the connector end (`:510-514`),
  and the slide is `:546-551`. The (f) rows assert the band every tick while rejoining. Flips A-D are
  recorded in `scratch/flips/`.
- **Path-wise connector sensing** (`manoeuvre.rs:54-115`, `query.rs:212-280`). Review focus (c):
  - The nose rule is the TASK-016 rule for both circles and rects.
  - The corridor step law is shared (consts moved to `lateral.rs:9-12`, law, not tuning).
  - The bisection keeps `rects[0]` as the first rect, so the "claim already inside" filter stays the
    same as in the full query. Sample 0 cannot hit during bisection, because the outer query found
    nothing there.
  - A caveat on the nose rule: it is measured once, at d = 0. On a U-turn connector (test floors only;
    the city graph has none, per OPEN_DECISIONS item 1) a body on the exit lane just past the U lies
    behind the start nose line and is not sensed until the car is about half way round. For turns up
    to 90 deg (the city), an exit-lane body always reaches past the nose line: its fwd extent is
    r + 1.2 = 2.83 m against a nose at 2.04-2.54 m.
- **Frame cost** (review focus c). A 6 m sweep on a 90 deg turn at the Bezier minimum radius is about
  30-40 samples. `first_in` culls bodies once per call. The bisection makes about 5 extra calls, each
  filtering all bodies again. That is O(connector cars x (samples x near + 6 x bodies)) OBB tests, a
  few thousand per tick. The recorded benches (traffic 1.48 ms vs 1.52 before, limit 19) agree. There
  is no unbounded work.
- **Walkers around standing cars** (`civilian/mod.rs:457-548`). `standing_cars` filters to standing
  `Rect` bodies (unit row with flip s3_C). `civilian_fsm` gains two `Res` of resources that are
  `init_resource`d by the composed plugins. No harness registers the system by hand.
- **`around_cars` fix (a)**, clearance-wide view first then the centre-line fallback
  (`fire_line.rs:167-178`). It is correct and widens the police arrest path too: cops now keep
  `clearance` from a car's side instead of grazing it.
- **Police regression check** (review focus a). I re-ran `police_arrest` (9), `police_pull_out` (7) and
  `police_stopped_driver` (3) on Windows: all green. The only behavioural change for cops is fix (a)
  and the M1 dither, which replaces an equal stall.
- **Rest skin** (Q4 option B, `recover.rs:44-55`) is capped at `recover.skin`, so it only ever relaxes
  the old rule and never makes recovery stricter. It is G1-sound, because the predictive switch still
  guards contact. See m1 for its "at rest" premise.
- **Spot C fixture** (review focus e, `traffic_causes.rs:574-588`). Filling behind the last AI car on
  the lane, moving or standing, removes a real fixture fault: spawning into a rolling car's spot. The
  gate still tests its claim. If the fill cannot place cars (a rolling car near the lane start), the
  `longest >= 8` and `switched >= 4` GATE BROKEN preconditions fire. It does not pass vacuously.
- **Tests re-run here (Windows):**
  - `traffic_recovery` 10/10.
  - `traffic_intersection` 6/6.
  - `traffic_pedestrian` 5/5.
  - `gta_sim --lib` 95/95 (incl. `standing_cars_rows` and `around_cars_rows`).
  - `traffic_junction_box::seed_1_box_keeps_moving_liveness`: green, worst stand 24.3 s, G1 0.000.
  - `traffic_go_around`: 9/10, **`avenue_seed_2` FAILED** (`passing cars touched vehicles:
    [(8315, 1576v1, 1828v0)]`). This matches the summary.
- No `unsafe`, no `unwrap` in production paths, no new tuning const. All files are under 750 lines
  (largest: `traffic_recovery.rs` 707, `traffic_intersection.rs` 680).

## 3. Issues

### Critical (C1): `traffic_go_around::avenue_seed_2` red on Windows (known item 4, fixer's scope)

`crates/gta_sim/src/traffic/drive.rs:543-553`. Main cannot go red. Below is the concrete heading-cap rule
the orchestrator asked for (review focus f).

**Rule: cap the manoeuvre heading so that the body's across extent stays inside the band the
manoeuvre may use.** It is the same geometry as `reach_across`. The stage-1 slide is its special case
with cap 0.

1. The across half-extent of the car body at yaw error θ from the path tangent is
   `e(θ) = hx·cosθ + hz·sinθ = R·sin(θ + φ)`, where `R = |half|` (2.367 m for the sedan) and
   `φ = atan2(hx, hz)` (30.47 deg). It is symmetric: when the nose swings to one side, the rear swings
   equally to the other. So both band edges bind.
2. The band `[lo, hi]` across the lane line, per lane:
   - `pitch = lane.left_gap.unwrap_or(2·hx)`, the same fallback as `off_lane`.
   - Base band: `±pitch/2`.
   - Pass (`offset o`, side σ): the σ side extends to
     `max(pitch/2, |o| + hx + pass.clearance)`, which is the claim band `pass.rs:235-241`.
   - Any other manoeuvre with target `t ≠ 0` (siren yield): the target side extends to
     `max(pitch/2, |t| + hx)`.
3. The room at the current lateral `l` is `m = min(hi − l, l − lo)`. The yaw cap is
   `θmax = max(0, asin(min(1, m/R)) − φ)`.
4. The rate limit (`yaw_rate_deg` 60 deg/s) makes the actual yaw lag the cap. So evaluate `m` at a
   look-ahead lateral: `l' = l` stepped towards the target by `rate(v) · |θnow| / ω` (the lateral
   covered while the yaw can turn back). Then clamp the desired heading:
   `heading = yaw_t + clamp(wrap(heading_yaw(..) − yaw_t), −θmax, θmax)`, where
   `yaw_t = aim_yaw(tangent)`.
   - Why the look-ahead is needed: `dθmax/dl = 1/(R·cos(θ+φ))`, 0.58 rad/m at l = 0. At lateral rate
     `0.8 + 0.15v` this needs more than 60 deg/s above v ≈ 6.7 m/s.
5. Only the heading changes. The lateral law is unchanged, so progress and the pass timing do not move
   (the position is set from `offset_pose`).
6. Keep the connector slide as it is: on a connector the band would be the table band, which gives a
   cap of about 4.3 deg at l = 0. Unifying is optional.

Numbers for the failing case (avenue inner lane, merging back from the curb lane, pitch 3.25, hx 1.2):
- At l = 0: `m = 1.625`, cap 12.9 deg (today about 31 deg, which reaches 0.18-0.45 m into the oncoming
  lane).
- At l = 0.5: cap 33.4 deg, so the early merge barely changes.
- Owner-visible: a slower nose swing in the last half metre of a merge. Mark it for the owner run.

Gate for the fixer:
- A floor row in `traffic_go_around` (or a lane row on `two_way_street`): a pass around a parked car,
  where the test records the footprint's across reach per tick against the lane line while
  `manoeuvre != None`.
  - Assert the reach stays at most `pitch/2 + 0.01` on the side away from the pass.
  - Also assert the claim band on the pass side.
  - Flip: remove the cap. It goes RED at about 1.8-2.08 m.
- Plus `avenue_seed_2` green on Windows and Linux, and G1 clean in `traffic_go_around`,
  `traffic_recovery` and `traffic_gridlock`.

### Major

**M1. `around_cars` stalls or dithers when the target is near and diagonal past the car.**
`crates/gta_sim/src/tactics/fire_line.rs:162-178`.
- The corner metric `via(c) = |from−c| + |c−to|` assumes the line `c → to` is clear. For a corner that
  does not see `to`, it underestimates the path. The "reached" radius (`corner − clearance`, 0.5 m)
  moves the stall 0.5 m off the corner, as shown in section 0.
- Affected, before and after this change:
  - a cop walking to the driver's door from the passenger side (`police/behavior.rs:399-407`);
  - a civilian whose `lane_target` lies about 1-2 m past a car standing across its crosswalk. That is
    the exact walker-pinned class this task claims to close; for D = 1 m past the far side the window
    is s < 0.63 m.
- Not caught by the new gate, whose target is 8 m past the car (Δ = 3.91 > |AB| − 1).
- **Fix:** score a corner by the length of the shortest path around the car:
  - `via(c) = |from−c| + |c−to|` when `car_entry(c, to, car, clearance)` is `None`;
  - else `|from−c| + min over the adjacent corners c'` that see `to` of `|c−c'| + |c'−to|`.
  - At most 2 corners on a convex rectangle.
  - Keep the reached radius, which breaks the tie on the corner itself.
- Gate rows in `around_cars_rows`: the door case and the unit-test target from a point 0.6 m along A→B
  must pick B. Add a headless walk that arrives, with the flip being the old metric.
- If the orchestrator prefers to defer this, TASK-036 (walk avoidance) is the natural home. The police
  door stall is pre-existing.

**M2. A recovered, ungranted car in the box turns invisible to plain crossing cars.**
- Code refs: `occupancy/mod.rs:87-103`, `traffic/manoeuvre.rs:146`, `traffic/junction.rs:302-328`.
- Mechanism:
  1. Once its rejoin ends (`drive.rs:558-564`, lateral = 0, manoeuvre None), a car recovered on a
     connector without a grant (2017v0, 1853v0 in the traces) is `OnPathTraffic`.
  2. A plain car skips `OnPathTraffic` in `sense`.
  3. The ungranted car is only a waiter. Its connector is not pushed into `blocking` when it waits for
     room or for a body on its own path (`junction.rs:311-326`: the box-lock scene).
  4. So a conflicting connector can be granted. Only `connector_clear` then keeps it apart, and that
     has no corner overhang (TASK-036 item 4).
  5. If it misses, two kinematic bodies pass through each other (the TASK-016 lesson): G1, silent
     class.
- Before this task the same car stayed `Dynamic`, which is `Traffic` kind and always sensed. So this
  change widens the exposure, beyond "the known overhang applies to more cars" (plan §6).
- Not reproduced by execution here; every touched city gate reports G1 0.000.
- **Fix (cheap, parameter-free):** in `sense`, skip `OnPathTraffic` only if the body is not an
  ungranted car standing on a connector. Build a per-tick `HashSet` of ungranted connector cars from
  `TrafficIntersections` in `advance_traffic` and pass it in.
- **Gate:** on `plus()`, a car recovered on connector X without a grant, placed where its corner
  overlaps Y's swept body but not Y's `connector_rects`, and a granted car on Y:
  - Assert G1 clean and that the Y car stops.
  - Flip: drop the new exception. It goes RED as a pass-through.
- The orchestrator may fold this into TASK-036 item 4, but it should be recorded as a TASK-037 side
  effect.

### Minor

**m1. The rest-skin premise "a body at rest cannot close the gap" is not what `standing > 0.0` means.**
- Refs: `recover.rs:32-35`, `docs/architecture/traffic.md` Modes.
- `standing` counts from the first tick at or below `hold_speed` (0.5 m/s). A vehicle rolling at
  0.45 m/s towards the car is a "resting vehicle":
  - `nobody_coming` still sweeps its motion over 0.5 s against the reduced rect;
  - `corridor_clear` checks it statically.
- G1 stays guarded by the predictive switch, so the result is at worst a switch/recover flip-flop at the
  1.5 s cadence.
- **Fix (no new number):** `resting_vehicle(b) = Rect && standing > 0 &&
  (b.velocity − v)·(own.centre − rect.centre) <= 0` (not closing). Correct the doc sentence too.
- Missing gate: the (g) row with the parked car replaced by a car creeping towards it at 0.4 m/s:
  no re-switch within 2 s of the recovery.

**m2. Along offset at the connector start is not bounded.**
- Refs: `drive.rs:143-145` (`s.max(0.0)`), `graph.rs:218-230` (clamped projection), `recover.rs:133-141`.
- A connector car pushed back behind its connector start keeps `Segment::Connector` with `s = 0`.
  `reach_across` and `corridor_clear` measure only the across component.
- On recovery the kinematic target `offset_pose(s = 0)` makes the body jump forward by the along error
  in one tick (`velocity = (target − position)/dt`). That jump is not covered by the corridor.
- 2017v0 stood at s 0.43, so a 0.5 m shove is enough.
- **Fix:** in `on_path` for connectors also require
  `|(position − point)·tangent| <= conflict_margin / 2`.

**m3. The ignore reasons and the docs point to TASK-037 `OPEN_DECISIONS.md` for items the orchestrator
moved to TASK-039.**
- Refs: `traffic_junction_box.rs:310,316`, `traffic_causes.rs:462,468`, the module docs,
  `traffic.md` "Junction box".
- Retarget them to TASK-039 so the pointers stay true after this task closes.

**m4. A lane or siren-yield rejoin that reaches a connector at speed now crabs sideways.**
- Ref: `drive.rs:546-551`.
- The slide applies to every `Rejoin` on a connector, not only a recovered car at rest, so the car
  moves at `0.8 + 0.15v` m/s with the heading on the tangent. It was plan 1.6 "measure, not design
  around". Not a correctness issue.
- Mark it for the owner run next to the sliding recovery.

**m5. The rest skin keeps the at-rest heading-swing term on connectors, where the rejoin slides.**
- Ref: `recover.rs:44-55`.
- That makes it more conservative than needed there (less help for class D in the box). Optional:
  set `swing = 0` when the segment is a connector.

## 4. Missing coverage

- `around_cars`: a target near and diagonal past the car (door case and the unit-test target from 0.6 m
  along A→B); a headless cop walking to the door from the passenger side (M1).
- An ungranted recovered car in the box against a granted conflicting crossing car (M2).
- The rest skin against a vehicle closing at or below `hold_speed` (m1).
- A connector recovery with the centre behind the connector start (m2).
- The heading-cap row for C1, as specified above.
- Runtime R1 (spots A/B/C + control) was not run by the implementer. It is now QA's, per the
  orchestrator.

## 5. Nits

- `recover.rs:1-7`: the module doc line 6 runs to about 130 columns (rustfmt does not reflow comments).
  The same applies to `traffic.md` line 73 and to the rb doc in `traffic_causes.rs:484-486`.
- `rejoin()` returns a 5-tuple and is called twice per car per tick (`recover_dynamic` and
  `corridor_clear`). A small struct or passing the result through would read better.
- `sweep_ahead` evaluates `pose()` three times per step (`:91`, `:96`). `first_in` rebuilds `near` and
  `claims` on each of the about 5 bisection calls. This is cheap today; hoist it if a bench moves.
- `traffic_intersection.rs`: the drift bound hard-codes `0.707 * 1.625` instead of deriving it from the
  floor's lane offset.

## Evidence

- Replica and results: `maw/tasks/in_progress/TASK-037/scratch/cr/around_cars_dither.py`, stdout
  reproduced in section 0.
- Commands run (Windows, `-j 2`, one at a time):
  - `cargo test -p gta_sim --test police_arrest --test police_pull_out --test police_stopped_driver --test traffic_go_around`;
  - `cargo test -p gta_sim --test traffic_go_around -- --exact avenue_seed_2 --nocapture`;
  - `cargo test -p gta_sim --lib --test traffic_recovery --test traffic_intersection --test traffic_pedestrian`;
  - `cargo test -p gta_sim --test traffic_junction_box -- --exact seed_1_box_keeps_moving_liveness --nocapture`.
- Not re-run by me: the Linux (WSL) suite, the benches, the full `-p gta_sim -p citygen` run, clippy.
  For those I rely on the implementer's logs in `scratch/wsl/` and `scratch/stage5/`.

children: 0 launched / 0 reported
