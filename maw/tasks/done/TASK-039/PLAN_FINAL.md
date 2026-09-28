# PLAN_FINAL — TASK-039: universal traffic progress guarantee

Reviewer: plan-reviewer-2. Bases: `PLAN.md` (48af323, detail source) and `PLAN_V2.md` (502db71,
corrections). V2 corrections win over PLAN, except where §5 of this file shows a V2 item wrong from code.
Line numbers are as of 502db71; locate code by function name.

Cost of error: silent (gridlock that shows over minutes, in front of the player). Full evidence layer:
headless rows with flip-RED on Windows and Linux, the third-body oracle, runtime QA on the TASK-040
repros.

## 0. Disconfirmation (done first, before evaluating the plan)

Counter-example written down: "with contacts turned off for the relaxed pair (avian `modify_contacts`
returns `false`), the blocker is still pushed through a channel the hook does not cover."

Search and result: the counter-example HOLDS, partly.
- The contact channel is covered, as V2 said. `avian3d-0.7.0/src/collision/narrow_phase/system_param.rs:771-781`:
  a `false` clears the manifolds and `TOUCHING` every step. Waking happens only on started touching
  with constraints (`:240-262`, `:322-354`), so a sleeping blocker stays asleep.
- Spatial queries are not covered.
  1. The Tnua ground sensor (vendored `bevy-tnua-avian3d-0.12.1/src/lib.rs:253-255`) is fixed by
     `TnuaNotPlatform`. V2 already made it mandatory.
  2. Wheel suspension rays of every awake `Dynamic` vehicle are NOT covered. `vehicle/chassis.rs:127-143`
     casts `solid = true` rays on `[World, Vehicle]` and excludes only the car itself. The mount sits
     `mount_height -0.49` below a rest centre at 1.1596 m (`assets/vehicle/sedan.ron:17,20`), so at
     0.67 m. That is inside another sedan's hull, which spans 0.44..2.08 m (half extents
     (1.2, 0.92, 2.04), underbody lift 0.2). The mount is inside the passer's hull as soon as the pair
     overlaps ≥ 1.2 - 0.72 = 0.48 m across. parry 0.27 returns time of impact 0 for a solid ray that
     starts inside a convex shape (`query/ray/ray_support_map.rs:63-67`). The suspension then reads full
     travel 0.3 m against the static 0.110 m: +5.1 kN of spring plus about 1.1 kN of damper (clamped at
     0.5 m/s) per wheel, on a 1200 kg car that weighs 11.8 kN. The blocker is lifted or rolled.
  - When is the blocker awake? A car touching a `Dynamic` traffic car shares its island, and `Dynamic`
    traffic cars carry `SleepingDisabled` (`contact.rs:232`, removed at `drive.rs:648`). That is class D's
    left car, and in M1 the Dynamic car D itself, through which the westbound car S passes.
  - V2's row 2.11 uses a sleeping parked car. The wheel loop skips sleeping bodies (`chassis.rs:105`),
    so that row cannot see this.
- Fix: step 2.4 (`vehicle::PassingThrough` plus a ray predicate) and row 2.10d, which flips RED without it.
- The head hitbox (`character/mod.rs:146-147`, `CollisionLayers::new(Hitbox, NONE)`) never forms pairs,
  so `apply_impacts` cannot fire through it.

## 1. Summary

A new traffic module `crates/gta_sim/src/traffic/progress.rs` adds one universal progress rule for AI
traffic cars. Every tick, each standing AI car (`Kinematic`/`Dynamic`) gets at most one out-edge, the
reason it stands: `Follow` (queue, oncoming passer, room past the box), `Grant` (a conflicting grant) or
`Body` (a body on its path: vehicle, character, left car in the box, or a `Dynamic` car's recovery
blocker). The edges form a functional wait-for graph. A pointer walk finds each chain's sink and every
cycle.

When a chain's sink has stood `progress.wait_seconds` (or every member of a cycle has stood
`progress.cycle_seconds`), `Body` edges become candidates, if their waiter and target have stood
`progress.grace_seconds`. Candidates are taken in order of least overlap with their blocker (binding
resolved question 1), and a car that is currently being passed is never moved. The chosen car gets
`TrafficCar.relaxed = Relax { blocker, since, physics_only }`. For that pair only:
- sensing, the path leader, the box path checks and recovery ignore the blocker;
- an avian `CollisionHooks::modify_contacts` returns `false`, so there is no push, no `CollisionStart`,
  no damage and no switch;
- the blocker's and the passer's wheel rays skip each other;
- `TnuaNotPlatform` keeps a relaxed character from standing on the car.

A clean way is always tried first: a lane pass that can go at once, a box pass around a character when
alone in the box, or the exit of least overlap. Otherwise the car drives its own line through the body at
most at `progress.speed`. The planning part ends when the car is past the body, the body moved or left,
the car left AI, or after `max_seconds`. The physics exemption lasts until the two footprints (grown by
`recover.skin`) separate.

All tuning lives in a new `progress` block of `assets/traffic/traffic.ron`. There is no new crate and
`Cargo.lock` is unchanged. The U-turn (D9) and "one body owner" (D10) are not built.

## 2. Implementation steps

### 2.A Shared definitions (normative for every step below)

**Phases of a relaxation.** `planning-active` = `relaxed.is_some_and(|r| !r.physics_only)`.

| Consumer | Kinematic car, planning-active | Dynamic car, planning-active | physics_only |
|---|---|---|---|
| `manoeuvre::sense` skip + `relaxed_hit`, `drive::leader` skip, `progress.speed` cap, clean-first in `plan` | yes | **no** (it brakes for B) | no |
| `box_rules::connector_clear` / `repick` `ignore`, `repick_relaxed` (junction) | yes | yes | no |
| `recover::nobody_coming` / `corridor_clear` (+ `resting_in`) skip | – | yes | no |
| `contact::switch_to_dynamic` skip (predictive and backstop) | yes | yes | yes |
| `TrafficHooks::modify_contacts` returns `false` | yes | yes | yes |
| markers `vehicle::PassingThrough(B)` (always) and `TnuaNotPlatform` (B is a `Character`) | yes | yes | yes |

Why a `Dynamic` car does not skip B in sensing: its autopilot speed comes from the same IDM
(`drive.rs:462-482`). If sensing skipped B, it would drive into B as a dynamic body with contacts off.
It leaves `Dynamic` through recovery, and only then does it pass, kinematic.

**Edges** (at most one per AI car in `Kinematic`/`Dynamic`, not `abandon`, whose `road.body(e).standing > 0`):

| Source (who) | Condition | Target | Kind | Gap for "nearest wins" |
|---|---|---|---|---|
| Kinematic | path leader (`leader`, now also returns its index) within `pass.trigger_gap` | leader car | `Follow` | leader gap |
| Kinematic | nearest `sense` hit (`ahead`/`beside`) within `pass.trigger_gap`, `hit.claim == true` | `hit.entity` (= claim owner, `occupancy/query.rs:170`) | `Follow` | `hit.gap` |
| Kinematic | the same, `hit.claim == false` | `hit.entity` | `Body` | `hit.gap` |
| Kinematic | `Manoeuvre::Pass { go: false, obstacle, hold_s, .. }` | `obstacle` | `Body` | `hold_s - s` |
| Kinematic waiter at its stop line or on its connector (from `junction::update`) | its connector path blocked (`blocked` map) | the body | `Body` | stop-line distance on a lane, 0 on a connector |
| same | first conflicting occupant or earlier waiter in `blocking`, or the `whole` holder | that car | `Grant` (never relaxed) | same |
| same | no room (`room < need`) or `!lane_start_free` | exit lane's first car, or the body/claim owner `lane_start_free` found | `Follow` | same |
| Dynamic | `Recovery::Stay { blocker: Some(b) }` | `b` | `Body` | 0 (always wins) |
| Dynamic | else path leader within `2 * idm.min_gap` | leader car | `Follow` | leader gap |

- Ties go `Body` > `Grant` > `Follow`.
- Post-pass: a `Follow` edge whose target has no out-edge and has stood ≥ `wait_seconds` becomes `Body`.
- Bailing, Taken, Abandoned and police cars, characters and non-traffic vehicles have no out-edge
  (sinks).
- A car with an out-edge that is currently being passed by a planning-active car keeps its edge. The
  exclusion below handles it.

**Eligibility and choice** (`progress::detect`, deterministic):
1. Pointer walk (visited/in-stack marks, O(n)) gives each node its root: a sink S, or a cycle C.
2. Candidates are `Body` edges W→X with, all of:
   - (sink tree) `standing(S) ≥ wait_seconds`, `standing(W) ≥ grace_seconds` and, if X ≠ S,
     `standing(X) ≥ grace_seconds`;
   - (cycle) the edge lies on C and every member of C has stood ≥ `cycle_seconds`. Edges of trees
     that hang off a cycle are not candidates;
   - W has `relaxed == None`.
3. `overlap(W, X)`: W's footprint sampled every `CORRIDOR_LATERAL_STEP` (0.3 m) along its path from its
   current `s` until its rear is past X's far extent. Samples are taken at two laterals: the current
   effective lateral (the current pose for a `Dynamic` car) and 0, the path line (a `Dynamic` car and a
   `Rejoin` return to it). The value is the max `contact::penetration` against X's footprint.
   - For a head that may still re-pick (a lane head at its stop line, or on its connector with
     `s <= REPICK_WITHIN`), use the min over its exits clear of every other body (`repick_relaxed`),
     else its current exit (`connector_body` samples).
4. Sort candidates by `(overlap.total_cmp, W.to_bits())`. Accept in order, skipping a candidate if W is
   the blocker of an active (planning-active or just accepted) relaxation, or if X is the waiter of one.
   Each accepted W gets `relaxed = Some(Relax { blocker: X, since: tick, physics_only: false })`. If
   W's manoeuvre is `Pass { go: false, obstacle: X, .. }`, it becomes `Manoeuvre::None`.
5. `TrafficStats.progress_by_trigger[0]` (sink) or `[1]` (cycle) += 1 per acceptance.
   `unexplained` = AI cars (Kinematic/Dynamic, `relaxed == None`) whose road standing >
   `wait_seconds` and that have no out-edge (per tick).

**End rules** (`progress::upkeep`, a system that runs before `advance_traffic` every tick, over every
`TrafficCar` with `relaxed.is_some()`, whatever its mode):
- B does not exist (neither a `TrafficCar` nor a `Position` with `Vehicle`/`Character`): `relaxed = None`.
- If planning-active, set `physics_only = true` when any of:
  - the car is not `Kinematic`/`Dynamic`;
  - `road.body(B).is_some_and(|b| b.standing == 0.0)` (B moved);
  - the car is kinematic (`RigidBody::Kinematic`) AND past B (every corner of B's rect, or the four rim
    points of B's circle along the car's forward/right, has `(q - rear)·forward < 0`, with
    `rear = centre - forward * half.z`);
  - `tick - since >= ceil(max_seconds / dt)`.
- If `physics_only` (including just set): when the car's footprint grown by `recover.skin` no longer
  touches B's footprint, set `relaxed = None`.
  (`swept_rect_hits_rect(.., Vec2::ZERO, ..)` / `swept_circle_hits_rect(.., Vec2::ZERO, ..)`.)
- Markers: `PassingThrough(B)` present iff `relaxed.is_some()`; `TnuaNotPlatform` present iff
  `relaxed.is_some()` and B has `Character`. Use `try_insert` / `try_remove` only on a change (no
  per-frame churn).

Why "past B" needs the car to be kinematic: a `Dynamic` car whose blocker is behind it (the cycle pair,
example 2) would be "past" on the first tick. Planning would then end before the 1.5 s recovery window,
and the relaxation would restart every tick.

**Worked examples** (sedan half extents 1.2 x 2.04, street pitch 3.25; estimates, measured in stage 0/3):
1. **M1.** Eastbound line = 0, the westbound side negative.
   - Footprints: P at +0.6 spans -0.6..1.8. D at -1.85 spans -3.05..-0.65 (gap 0.05 m). S on the
     westbound line -3.25 spans -4.45..-2.05.
   - overlap(S,D) = 1.0. overlap(D,P) = max(current 0, line -1.2..1.2 against -0.6..1.8 → 1.8) = 1.8.
     N→D is `Follow`, since D has an out-edge.
   - P stands from 0 s, D from about 2 s (teleport), S from about 4 s. At 18 s, `standing(P) ≥ 18`.
     Candidates S→D (1.0) and D→P (1.8). S→D is accepted, and D→P is skipped because D is S's blocker.
   - S passes at ≤ 3 m/s over about 10 m (about 4.5 s). At about 22.5 s D is accepted: S2 has just
     moved up, so its standing is below grace.
   - D recovers after the 1.5 s calm (about 24 s). D's `Dynamic` stand is about 22 s, a margin of about
     8 s under 30. D rejoins through P, sliding 1.85 m at `rate_at_rest` 0.8 m/s, then drives about 5 m
     past P.
   - N's stand is about 21 s. Then N→P is `Body` (vehicle, `passable`) and N lane-passes P when the
     westbound band is free.
   - Under V2's per-edge rule ("X stood ≥ T"), D→P would be eligible at 18 s and S→D only at 20 s, so
     the chain end D would go first. This is the case binding question 1 forbids.
2. **Cycle pair.** X at s 20, Y nose-to-tail 0.05 m ahead on `two_way_street(60.0)` lane 0, both
   `Dynamic`.
   - Each fails `nobody_coming` on the other: the rest-skin rect is half + 0.1, and the gap is 0.05 m.
     So `Stay { Some(other) }`, giving X→Y and Y→X `Body`, a 2-cycle. `led` is true for X and neither is
     off lane, so neither gives up. On HEAD both stand `Dynamic` forever.
   - At 8 s: overlap(X,Y) is about 2.4 m (X drives into Y), overlap(Y,X) = 0 (Y drives away). Y is
     accepted, and X is skipped as Y's blocker.
   - Y recovers at about 9.5 s. It is then kinematic and past X, so physics_only. It separates after
     0.35 m and `relaxed = None`.
   - X then recovers normally at about 11-12 s. `progress_by_trigger == [0, 1]`.
3. **N1 and b3.**
   - N1: holder H1 stops at the player at about 3 s and is relaxed at 18 s (stand about 15 s).
     - A waiter W2 conflicting with H1 (`Grant` edge) is granted when H1 leaves (about 22 s), stops at
       the player and waits grace 6 s, so it is relaxed at about 29 s.
     - A third-approach waiter conflicting with both waits from arrival (about 5 s) to about 33 s:
       28 s, the tight case. Stage 0.4 derives it from the trace, and stage 3.6 has the fallback.
   - b3: the dummy is spawned at the nose right after the switch.
     - Relaxed at about 18 s, recovered at 19.5 s or later, so never out of `Dynamic` before
       `wait_seconds`.
     - Driven through the dummy at ≤ 3 m/s: rear past it at about 23 s. With the clean pass instead:
       2.0 m lateral at 0.8 m/s plus about 5.5 m at ≤ 3 m/s, about 26 s.
     - The dummy is removed at `wait_seconds + 12` = 30 s, so the margin is ≥ 4 s.

### 2.B Stage 0: traces on HEAD (no production change)

0.1 Probe crate `maw/tasks/in_progress/TASK-039/scratch/probe/ws/probe`, copied from
`maw/tasks/done/TASK-037/scratch/probe/ws/probe`: the same `Cargo.toml` patches, and `#[path]` to
`crates/gta_sim/tests/common/mod.rs` and `crates/gta_sim/tests/traffic_support/mod.rs`. It is read-only
and recomputes from public state (`RoadOccupancy::first_along`/`blocked`, `TrafficIntersections`,
`TrafficCar`, `Position`/`Rotation`). Output goes under `scratch/stage0/`.

0.2 Traces, once per second, for every AI car standing > 5 s within 45 m of the scene:
- segment, s, mode, manoeuvre, lateral, grant/waiter/whole, speed, road `standing`;
- the would-be edge (table 2.A: kind and target) and the root (sink or cycle);
- for each candidate, `overlap(W, X)` (2.A.3);
- for a head at a stop line, per exit: clear of third bodies (y/n) and overlap with the chain's sink.

Rows: class E (`traffic_junction_box` seed-7 scene, `traffic_causes` R1 seed 7), class D (R1 seed 1),
M1, N1, b3, and the cycle pair.

0.3 Fixtures, built in the probe first and each RED on HEAD. They move into
`crates/gta_sim/tests/traffic_progress.rs` in stage 3.
- **M1** (seed 1, `city_app(1)`).
  - Lanes: `TrafficGraph::nearest` at (34.4, -77.5) (eastbound) and (40.2, -80.4) (westbound).
    `GATE BROKEN` unless the two lanes are opposite (`dir` dot < -0.99).
  - `clear_spot((34.5, y, -78.5), 10.0)`.
  - D = `spawn_traffic_car` on the eastbound lane with its centre at the along-lane position of x 34.0.
    It is switched through a real contact: the `bumped` pusher pattern of `traffic_causes.rs:149-170`,
    a parked car 1 m behind held at 2 m/s until `mode == Dynamic`, then despawned.
  - Then D is `teleport`ed (named mutation) to the eastbound line point moved 1.85 m towards the
    westbound line, keeping its heading.
  - P = `park_car` at the eastbound line point at the along-position of x 34.9, moved 0.6 m away from
    the westbound line, heading along the lane.
  - The player: `stand_player` on the north sidewalk (`sidewalk_at` nearest to (35.0, -70.5)), facing
    (34.5, -78.5).
  - After 2 s, `GATE BROKEN` unless all of:
    - D is `Dynamic`;
    - D and P are within 0.5 m of (34.0, -79.2) / (34.9, -76.9);
    - `obb_overlap(D, P)` is false;
    - both lanes are blocked: a rect of the eastbound pass claim beside P, and one of the westbound
      lane from S's spot through D, each hold a body (`RoadOccupancy::blocked`).
  - Feeders `Feeder::new(lane, 4 * HZ, 8)` on both lanes, 120 s.
- **N1** (seed 1).
  - `stand_player` at (7.9, -81.3), facing the box's busiest approach (`connector_use`-like count over
    the first 20 s).
  - `GATE BROKEN` unless `graph.in_junction(p, 0.0)`, and at least one connector of that node has a
    body sweep (`connector_body` equivalent, 0.1 m steps like `traffic_junction_box::on_path`) that
    overlaps the capsule circle.
  - The player gets no input and is held ≥ 75 s. Run 100 s.
- **Cycle pair** (floor `two_way_street(60.0)`, example 2): X at `Segment::Lane(0)`, s 20.0, and Y at
  s `20.0 + 2 * half.z + 0.05`, both speed 0.
  - After 2 ticks both are switched by hand (named mutation, the switch is not the subject): a local
    copy of `traffic_recovery.rs:73-86` `switch_by_hand`.
  - `GATE BROKEN` unless, after 1 s, `recover::nobody_coming` would fail on each: recompute the
    rest-skin rect half + `switch.skin` in the probe and check it touches the other.
  - HEAD: both `Dynamic` > 30 s. If this pose does not stand > 30 s on HEAD, stop and write a note:
    the cycle flip cannot be met.

0.4 Record in the stage-0 summary:
- N1: where the waiters stand, and how many holders pass the player per minute on HEAD.
- Class D: whether 1853v0 is inside the conflict-table band (`recover_dynamic` `on_path`: reach across
  ≤ half.x + `conflict_margin`/2 and along ≤ `conflict_margin`/2). Outside, stop with a redesign note
  before stage 2: relaxed recovery cannot fire.
- M1: the measured overlap(S,D), overlap(D,P) along the line, D→P at its current offset, and whether
  D's current offset meets the westbound band.
- The worst N1 stand at T 18 with grace 6, worked from the trace: holders in a row × (grace + pass
  time) + a conflicting waiter's wait. Target ≤ 27 s.
- Whether any HEAD row shows a cycle.

### 2.C Stage 1: third-body oracle, before any behaviour change

1.1 `crates/gta_sim/src/traffic/mod.rs`:
- Add
  ```rust
  /// The progress rule lets the car past `blocker` since fixed tick `since`; `physics_only`: the
  /// manoeuvre is over and only the contact exemption remains until the two separate.
  #[derive(Reflect, Clone, Copy, Debug, PartialEq)]
  pub struct Relax { pub blocker: Entity, pub since: u64, pub physics_only: bool }
  ```
- Add the field `pub relaxed: Option<Relax>` to `TrafficCar` (doc: "a progress relaxation, see
  `progress`"), and `.register_type::<Relax>()`.
- `clear_ai_state` (`mod.rs:256-264`) does NOT touch `relaxed`: a relaxation outlives hijack or
  abandon until separation.
- `crates/gta_sim/src/traffic/spawn.rs:63-75` literal: add `relaxed: None`. This is the only literal
  (`rg "TrafficCar \{"`).
- Nothing writes it yet. No BRP/QA script writes `TrafficCar`: t14/t15/t16 and the TASK-040 tools only
  read it.

1.2 `crates/gta_sim/tests/traffic_support/`:
- `mod.rs`: `mod third_body; pub use third_body::*;`, and fields on `Footprints`:
  `pub relaxed_max_depth: f32`, `third_bodies: bool`, `third_violations: Vec<Violation>` (initialized
  in `new`). `mod.rs` stays < 750 lines (735 today; about 6 lines added).
- New `third_body.rs` holds an `impl Footprints` block with the rest.
- `Footprints::record` (`mod.rs:279-322`): before a pair counts, it is exempt when either car's
  `TrafficCar.relaxed` names the other, read each tick, both phases. An exempt pair adds its depth to
  `relaxed_max_depth` and never to `violations`.

1.3 Opt-in `Footprints::with_third_bodies(mut self) -> Self` in `third_body.rs`. `record` then also
checks, every tick:
- Kinematic vehicle × `Character` (every character within 150 m of the player, the player included):
  depth = capsule radius (`LocomotionConfig::capsule_radius`) minus the distance from the capsule
  centre to the chassis rect (own circle-to-OBB). Tolerance `dynamic_tolerance`.
- Kinematic vehicle × static world: `SpatialQuery::shape_intersections` of
  `Collider::cuboid(2hx - 0.04, 2hy - 0.2 - 0.04, 2hz - 0.04)` at `position + Y * 0.1` with the car's
  rotation, on `SpatialQueryFilter::from_mask(GameLayer::World)`. This is the box of
  `traffic_support/mod.rs:114-129`, shrunk 0.02 m per side. Any hit is a violation, depth reported as
  0.02.
- The 1.2 exemption applies to characters too.
- `assert_clean` also fails on `third_violations`.

1.4 New `crates/gta_sim/tests/traffic_progress.rs`, floor row `oracle_sees_a_car_through_a_dummy`:
- Floor `two_way_street(60.0)`. A traffic car at `Lane(0)` s 10, whose `mode` is set to `Taken` by hand
  (named mutation: it stays kinematic and the switch ignores it; the oracle is the subject).
- A dummy (`spawn_dummy`) on its line 10 m ahead.
- Drive the car's `Position` and `Transform` by hand at 2 m/s through the dummy (named mutation).
- Four passes:
  - (a) `relaxed = None` gives a character violation;
  - (b) `relaxed = Some(dummy)`, re-set by hand every tick (the upkeep of stage 2 would end it), gives
    no violation and `relaxed_max_depth > 0`;
  - (c) `relaxed` naming another entity is RED;
  - (d) the same car driven into a wall of `world/test_area.rs` gives a static violation.
- Check each fixture point against the test area (`GATE BROKEN` on a displaced fixture, TASK-012
  lesson).

1.5 HEAD baseline: `with_third_bodies()` in report-only mode over `traffic_gridlock` seeds 1/2/7/42,
`traffic_junction_box`, `traffic_causes` R1/rb/spot C and `traffic_go_around`, through the probe.
- Record counts and depths in `scratch/stage1/baseline.txt`.
- Trace every HEAD violation (pair, tick, what moved).
- A real one goes to the orchestrator as a finding BEFORE stage 2: the spec's "0 against any third
  body" cannot then be met by this task alone. Never add a tolerance.

### 2.D Stage 2: config and relaxation plumbing (no trigger yet)

2.0 Config (moved here from PLAN 3.1/3.2: the speed cap and the end rules need it).
- `crates/gta_sim/src/traffic/config.rs`:
  ```rust
  /// The progress rule (TASK-039): see `progress`.
  #[derive(Deserialize, Clone, Debug)]
  #[serde(deny_unknown_fields)]
  pub struct ProgressConfig {
      pub wait_seconds: f32, pub grace_seconds: f32, pub cycle_seconds: f32,
      pub max_seconds: f32, pub speed: f32,
  }
  ```
- Add the field `pub progress: ProgressConfig` to `TrafficConfig`, after `sirens`.
- `validate`: add the five fields to the `positive` list. Then, each with its own message naming both
  fields:
  - `wait_seconds > grace_seconds` ("the blocker's clock dominates");
  - `grace_seconds >= pass.character_seconds` (a new waiter gets the clean planners first);
  - `cycle_seconds > reservation_timeout` (the lease resolves grant stand-offs first);
  - `speed <= pass.speed` (a relaxed pass is never faster than a clean one).
- Export `ProgressConfig` in `traffic/mod.rs:23-26`.
- `assets/traffic/traffic.ron`: add
  `progress: (wait_seconds: 18.0, grace_seconds: 6.0, cycle_seconds: 8.0, max_seconds: 30.0, speed: 3.0)`,
  with a comment block:
  - wait: after this, a car stuck behind a body that stands, with no way around, squeezes past it with
    collision relaxed against that body only. It is above the 14 s "must still wait" windows of
    `traffic_recovery` c/e and leaves 12 s under the 30 s stand bound.
  - grace: every new waiter first gets the clean pass, the repick and the box pass.
  - cycle: a cyclic wait held this long is broken by the least-overlap member.
  - max: stale guard for a relaxation that never got past its body.
  - speed: a squeeze past a body is slow (owner run).
- `crates/gta_sim/tests/config_traffic.rs` (`traffic_rules_fire_with_their_keyword`): one sabotage per
  new rule, strictly on the failing side (TASK-007 lesson). For example: `wait_seconds: 18.0` to `5.0`
  (< grace 6), `grace_seconds: 6.0` to `5.5` (< 6), `cycle_seconds: 8.0` to `4.5` (< 5),
  `speed: 3.0` to `6.5` (> 6), and `max_seconds: 30.0` to `-1.0`. Each asserts its own keyword.

2.1 New `crates/gta_sim/src/traffic/progress.rs` (declare `mod progress;` in `traffic/mod.rs`, plus
`pub(crate) use progress::TrafficHooks;`):
```rust
#[derive(SystemParam)]
pub struct TrafficHooks<'w, 's> { cars: Query<'w, 's, &'static TrafficCar> }
impl CollisionHooks for TrafficHooks<'_, '_> {
    fn modify_contacts(&self, contacts: &mut ContactPair, _: &mut Commands) -> bool {
        let a = contacts.body1.unwrap_or(contacts.collider1);
        let b = contacts.body2.unwrap_or(contacts.collider2);
        !(self.exempt(a, b) || self.exempt(b, a))
    }
}
```
- `exempt(car, other)` = `cars.get(car).is_ok_and(|c| c.relaxed.is_some_and(|r| r.blocker == other))`.
  `filter_pairs` keeps its default. `filter_pairs` runs only for new pairs, so it would miss a pair
  already touching (class D, b3).
- Helpers:
  - `pub(super) fn exempt_pair(car: &TrafficCar, other: Entity) -> bool` (both phases);
  - `pub(super) fn planning_blocker(car: &TrafficCar) -> Option<Entity>` (planning-active only);
  - `pub(super) fn sensing_blocker(snap: &Snap) -> Option<Entity>` (planning-active and
    `!snap.dynamic`).
- API verified in the pinned source: `CollisionHooks: ReadOnlySystemParam + Send + Sync`
  (`avian3d-0.7.0/src/collision/hooks.rs:150`); `with_collision_hooks<H: CollisionHooks + 'static>`
  (`lib.rs:701`); `ContactPair.body1/body2: Option<Entity>`.

2.2 `crates/gta_sim/src/lib.rs:194`: `PhysicsPlugins::default()` becomes
`PhysicsPlugins::default().with_collision_hooks::<traffic::TrafficHooks>()`.
- This is the one composition, used by the game and every headless test.
- No other hooks exist in the workspace (only one set per app is allowed): future hooks must extend
  `TrafficHooks`.

2.3 `crates/gta_sim/src/traffic/spawn.rs::spawn_traffic_car`: add `ActiveCollisionHooks::MODIFY_CONTACTS`
to the spawned tuple, next to `TrafficCar`.
- The chassis collider is on the body entity (`vehicle/mod.rs:158-176`), and the pair flag is set from
  either collider's `ActiveCollisionHooks` (`collider_tree/tree.rs:93`), so the flag must be there from
  spawn.
- `spawn_traffic_car` is the only traffic spawn path.

2.4 Wheel rays.
- `crates/gta_sim/src/vehicle/mod.rs`: add
  ```rust
  /// This car drives through `.0` with contacts off (traffic progress, TASK-039): neither car's
  /// wheel rays see the other.
  #[derive(Component, Clone, Copy, Debug, PartialEq)]
  pub struct PassingThrough(pub Entity);
  ```
- `crates/gta_sim/src/vehicle/chassis.rs`: the wheel system gets `passing: Query<&PassingThrough>`.
  Replace `spatial.cast_ray(mount, down, reach, true, &filter)` with
  `spatial.cast_ray_predicate(mount, down, reach, true, &filter, &|c| !through(entity, c))`, where
  ```rust
  let own = passing.get(entity).ok().map(|p| p.0);
  let through = |me: Entity, c: Entity| own == Some(c) || passing.get(c).is_ok_and(|p| p.0 == me);
  ```
  The collider is the body entity for vehicles. `cast_ray_predicate` is verified in
  `avian3d-0.7.0/src/spatial_query/system_param.rs:176-184`: `true` keeps the collider.
- Reason: §0. The vehicle domain gets a neutral component instead of importing traffic.

2.5 `progress::upkeep` (the end rules and markers of 2.A).
- Signature:
  ```rust
  upkeep(commands, Res<TrafficConfig>, Res<VehicleConfig>, Res<LocomotionConfig>, Res<Time<Fixed>>,
         Res<RoadOccupancy>,
         cars: Query<(Entity, &mut TrafficCar, &Position, &Rotation, &RigidBody,
                      Has<PassingThrough>, Has<TnuaNotPlatform>)>,
         others: Query<(&Position, &Rotation, Has<Vehicle>), Without<TrafficCar>>,
         characters: Query<(), With<Character>>)
  ```
- Collect the footprints of all traffic cars first (read pass), then mutate.
- `tick` as in `advance_traffic` (`drive.rs:231-232`).
- `TnuaNotPlatform` is `bevy_tnua::TnuaNotPlatform` (glob re-export, `bevy-tnua-0.32.0/src/lib.rs:202`).
- Registration in `traffic/mod.rs`: replace `drive::advance_traffic.in_set(TrafficSystems::Drive)` with
  `(progress::upkeep, drive::advance_traffic).chain().in_set(TrafficSystems::Drive)`. The markers'
  commands apply before `VehicleSystems::Drive` (auto sync point). A relaxation starts with the pair
  apart, so a one-tick marker latency is harmless.

2.6 Kinematic consumers.
- `manoeuvre::sense` (`manoeuvre.rs:124-169`) returns `Sensed { ahead, beside, relaxed_hit }`
  (`drive.rs:410` is the only caller).
  - With `sensing_blocker(snap) == Some(B)`, `skip` also drops B.
  - `relaxed_hit` is the target strip/sweep recomputed with a skip that keeps only B.
- `drive::leader` (`drive.rs:102-129`):
  - Returns `Option<(f32, f32, usize)>` (gap, speed, index) and skips the index whose entity is
    `sensing_blocker(snap)` (V2 R6: `leader` brakes for any AI car in the path occupancy).
  - Implementation: pass `skip: Option<Entity>`; in the `filter`, `k != me && Some(snaps[k].entity) != skip`.
- Speed cap in `advance_traffic` (`drive.rs:381-384`): `if sensing_blocker(snap).is_some() { v0 = v0.min(cfg.progress.speed) }`.
  Binding resolved question 4: the pass speed stays low, as a data value.
- `manoeuvre::plan` (`manoeuvre.rs:181-264`) gets `relaxed_hit: Option<Hit>`. New branch right after
  the published-pass branch (line 233), for `idle` cars with `relaxed_hit = Some(hit)`:
  1. Lane pass that can go now: `plan_pass(.., &hit, ..)`. If `Some((Pass { .. }, claim))`, build
     `probe = Snap { car: TrafficCar { manoeuvre: pass, ..snap.car }, ..*snap }`. If
     `may_go(road, graph, &probe, cfg, vcfg)`, return the pass with `go: true` and its claim.
     `Snap` gains `#[derive(Clone, Copy)]` (all fields are `Copy`).
  2. Else the box pass: the existing lines 243-263 with `hit`, where the refusal
     `hit.kind == BodyKind::Character` is lifted for this branch only (`alone_in_box` still required).
     This covers binding resolved question 2: a clean in-box offset first.
  3. Else `None`: the relaxed drive on its own line (or its current offset) through B.
  - Why "can go now": a relaxed car with a published `Pass { go: false }` stands on its hold. If the
    oncoming lane never clears, the relaxation would drop and republish forever (V2 left this as a
    risk). V2's `idle` widening for `Rejoin` cars is not built: a `Pass { go: false }` targets lateral
    0 (`lateral.rs:21-31`), so for D in M1 it would not be a clean path. Its rejoin through P is
    accepted (binding question 4).

2.7 Box rules and junction.
- `box_rules::connector_clear` (`box_rules.rs:46-64`) and `repick` (`:69-84`) get `ignore: Option<Entity>`,
  added to `skip` (`b.entity == requester || Some(b.entity) == ignore || ...`).
- Call sites pass `planning_blocker(&snaps[k].car)` of the requester:
  - `junction.rs:127-129` (`body_blocked`);
  - `:244` (head stuck check);
  - `:248-257` (`repick`);
  - `:305` (`blocked` map).
- New `box_rules::repick_relaxed(road, graph, junction, lane, current, from_s, requester, half, blocker) -> Option<u32>`:
  - Over `[current]` followed by `graph.lane(lane).out` in order (current not repeated), keep the exits
    whose `connector_clear(.., ignore = Some(blocker))` is `None`.
  - Return the one of least `max(penetration(rect, blocker_footprint))` over its `connector_body` rects.
    Ties keep list order.
- In `junction::update`'s request loop (`junction.rs:241-273`), for a head (`at_start` or a lane head)
  with `planning_blocker == Some(B)`: call `repick_relaxed` instead of the `stuck`/`repick` branch. If it
  returns an exit ≠ `c`, switch exactly as the existing branch does (`next`, `at_start` segment switch +
  `release`, waiter removal).
- New `contact::penetration(a: &FlatRect, b: &Footprint) -> f32`, next to `rects_overlap` (`contact.rs:116`):
  - rect × rect: min over the four axes of `a1.min(b1) - a0.max(b0)`;
  - circle: `radius - distance(centre, rect)`, where the distance is negative inside, i.e. radius plus
    the distance to the nearest edge;
  - clamped at ≥ 0. Also used by 2.A.3.

2.8 `crates/gta_sim/src/traffic/recover.rs`:
- `nobody_coming` returns `Option<Entity>` (the first failing body, `None` = nobody coming), and skips
  the relaxed blocker.
- `corridor_clear` returns `Result<f32, Option<Entity>>` (`Err(Some(body))` for a body in `blocked` or
  `resting_in`, `Err(None)` for a claim), and skips the relaxed blocker in both the `road.blocked` skip
  and the `resting_in` loop.
- `recover_dynamic` passes `planning_blocker(&snap.car)`. `Recovery::Stay` becomes
  `Stay { blocker: Option<Entity> }`, carrying the corridor body, else the coming body. `led` is
  unchanged: a relaxed car never gives up.
- Update the match at `drive.rs:279-294` and store the blocker per snap:
  `stay: Vec<Option<Entity>>`, indexed like `snaps`.
- Update the unit test `resting_in_rows` only if its signatures change (it calls `resting_in` only, so
  it should not).

2.9 `contact::switch_to_dynamic` (`contact.rs:131-246`):
- Predictive: insert `.filter(|&other| !exempt_pair(car, other))` before `find_map` at `:187-188`
  (`find_map` takes the first hit).
- Backstop (`:209-224`): skip when `exempt_pair(traffic, other)`.
- Both phases: this is a physics consumer.

2.10 Plumbing floor rows in `traffic_progress.rs` (relaxation set by hand: named plumbing, the detector
is not the subject).
- Floor for a) to d): `let (lanes, connectors) = loop_lanes(v0); traffic_floor(lanes, &connectors, &[])`,
  with `v0` = `desired_speed.street` (`traffic_support/mod.rs:24,55`). Its lanes
  have no neighbour, so `plan_pass` returns `None` (`pass.rs:204`, `lane.left_gap?`), and a lane has no
  box pass. The relaxed car must drive through; on `two_way_street` the clean-first branch would take
  the empty oncoming lane instead.
- Place every body on a straight stretch of the loop, clear of the rounded corners and of every
  test-area block (`GATE BROKEN` otherwise).
- a) `relaxed_car_drives_through_a_standing_player`.
  - A kinematic traffic car on a straight loop lane at speed 5; the player (`stand_player`) on its line
    12 m ahead. `relaxed = Some(player)` set once.
  - Messages are read with the cursor pattern of `lethality.rs:345-350` (`get_cursor_current` +
    `read`), each tick.
  - Asserts:
    - the car's centre passes the player's position along the lane;
    - no `VehicleHit` / `DamageDealt` targets the player;
    - `Health` unchanged;
    - `HitReaction` is never `KnockedDown`/`Staggered`;
    - the player's `Position.y` stays within 0.1 m of its standing height;
    - the car stays `Kinematic`;
    - after the pass `relaxed == None` (upkeep).
  - Flip: `TnuaNotPlatform` not inserted makes the height assertion RED. If it stays GREEN, record that
    the sensor case was not exercised and keep the component anyway (binding question 2).
  - Control: a second car with `relaxed = None` switches to `Dynamic`.
- b) `relaxed_pair_ends_only_when_separated` (V2 R3).
  - As a), with a dummy (`spawn_dummy`). When the car's centre is within 0.3 m of the dummy along the
    lane, teleport the dummy 0.3 m sideways (named mutation: "B moved"). The car then stops inside it:
    planning is over and it senses the dummy again.
  - 2 s later, teleport the dummy 3 m clear of the car.
  - Asserts:
    - planning ended (`physics_only`) at the first teleport;
    - while overlapped, no `CollisionStart` for the pair, no damage, no switch;
    - the dummy is displaced only by the two teleports (± 0.05 m);
    - after the second teleport, `relaxed == None`.
  - Flip: end rule "B moved → `None` at once" gives a `CollisionStart` and a backstop switch (RED).
- c) `relaxed_pair_has_no_contact`.
  - A parked car (`park_car`) on the line 15 m ahead, asleep: insert `Sleeping` as
    `traffic_recovery.rs:307` does, and assert it is present. Hooks run only when one side is awake
    and not static (`hooks.rs:180`): the moving kinematic car is.
  - The car drives through it, relaxed.
  - Asserts: no `CollisionStart` for the pair; the parked car moves < 0.02 m; no switch; after,
    `relaxed == None`.
  - Flip: the hook returns `true`. The parked car is shoved ≥ 0.02 m (RED). The switch stays off
    because the skip is `exempt_pair`.
- d) `relaxed_car_passes_an_awake_parked_car` (new, §0).
  - As c), with `SleepingDisabled` inserted on the parked car instead (named mutation: "awake, as when
    touching a `Dynamic` traffic car").
  - Asserts, while overlapped: the parked car's `Position` moves < 0.02 m, and its tilt from up stays
    < 1°.
  - Flip: the `cast_ray_predicate` restored to `cast_ray`. All four wheels read full travel, about
    24 kN up against 11.8 kN weight, so the car is lifted (RED by a wide margin).

### 2.E Stage 3: detector, triggers, rows

3.1 Edges.
- `junction::update` returns `HashMap<Entity, (Entity, progress::EdgeKind)>` for waiters left ungranted
  this tick:
  - `blocking` becomes `Vec<(u32, Entity)>`;
  - the `blocked` value becomes `Option<(Entity, f32)>`;
  - a waiter skipped because `whole` is set gets `Grant` to the whole holder;
  - `lane_start_free` becomes `&dyn Fn(u32, f32) -> Option<Entity>` (the blocking body or claim owner,
    `None` = free). The closure at `drive.rs:311-326` returns `road.blocked(..)`. The only use is
    `junction.rs:333`.
  - The room case uses the exit lane's first car from `occupancy`.
- In the per-car loop (`drive.rs:353-442`), collect `edges: Vec<Option<(Entity, EdgeKind, f32)>>` with
  `progress::edge_of(..)` (the 2.A table). It gets: the snap, its road standing, `leader`'s result,
  `Sensed`, the junction map, `stay[k]`, `cfg`, `half_length`. Bailing snaps `continue` before this:
  they are sinks.
- `drive.rs` must stay < 750 lines: the table logic lives in `progress.rs`.

3.2 `progress::detect(snaps: &mut [Snap], edges, road, graph, junctions, cfg, half, tick, stats: &mut TrafficStats)`
implements 2.A (eligibility, overlap, order, exclusions, `Pass` drop, counts).
- Called once after the per-car loop (after `drive.rs:442`), before motion. A relaxation set there is
  first seen by consumers the next tick, which counts in the latency (3.6).
- `TrafficStats` (`mod.rs:196-210`) gains:
  - `pub progress_by_trigger: [u32; 2]` (sink, cycle);
  - `pub progress_recoveries: u32` (relaxed `Dynamic` cars recovered, counted at `drive.rs:640-656`);
  - `pub unexplained: u32` (per tick).
- The first two are carried in `counts` like `switches_by_cause` (`drive.rs:237-243`); otherwise they
  read 0 every tick (V2 R8).

3.3 Rows in `crates/gta_sim/tests/traffic_progress.rs`: production city unless named, a stationary
player facing the scene, production population, no input.
- Each row collects all violations before panicking, builds `Footprints::new(..).with_third_bodies()`
  and a `StandClock`, and prints the worst relaxed overlap depth, `progress_by_trigger`, the max
  `unexplained`, and the first pair chosen.
- `m1_two_bodies_block_both_lanes_seed_1` (fixture 0.3):
  - no AI car within 45 m of (34.5, -78.5) stands > 30 s;
  - `dynamic_violation()` is `None`;
  - the oracle is clean;
  - at least 4 fed cars pass the scene on each lane (their rear past x of the far body);
  - `progress_by_trigger[0] >= 1`.
  - Report the first accepted pair (expected S→D, example 1), D's path (rejoin through P) and its depth.
- `n1_player_standing_in_the_box_seed_1` (fixture 0.3):
  - stands within 45 m ≤ 30 s; no `Dynamic` > 30 s; the oracle is clean;
  - the player takes no damage (`Health` unchanged, no `VehicleHit` with target the player), is never
    `KnockedDown`/`Staggered`, and `Position.y` stays within 0.1 m;
  - liveness: a relaxation against the player occurred, or a clean box pass or repick around him did.
    Report which.
- `cycle_two_pinned_dynamic_cars` (floor, fixture 0.3, example 2):
  - both leave `Dynamic` within 30 s;
  - `progress_by_trigger == [0, 1]`;
  - the breaker is the least-overlap member (Y, the front car), with the lower entity breaking a tie;
  - the oracle is clean, with no exempt pair overlapping.
- b3 re-anchored in `crates/gta_sim/tests/traffic_causes.rs` (V2 R10):
  - `bumped(label, pressed_s: u32, after)`; b1/b2/b4 pass `PRESSED_S`; b3 passes
    `(cfg.progress.wait_seconds as u32) + 12` read from the shipped config.
  - The b3 closure keeps its dummy.
  - The b3-only assertions (a flag argument or a b3-specific wrapper):
    - before `wait_seconds` s after the press it stays `Dynamic`, never given up (the existing check,
      bounded by `wait_seconds` instead of the whole press);
    - it is never `Abandoned`/`Bailing` in the run;
    - its `StandClock` worst ≤ 30 s;
    - its rear passes the dummy before the dummy is removed (example 3);
    - the dummy is never `KnockedDown` and takes no `VehicleHit`.
  - `spot_c` keeps `PRESSED_S`: its cars relax at 18 s and recover into the queue, and recovery is not a
    give-up.

3.4 Un-ignore `traffic_junction_box::seed_7_box_keeps_moving_liveness` (`:326`) and
`traffic_causes::r1_car_left_in_the_box_seed_1` / `_seed_7` (`:462`, `:468`).
- Update the module docs' "ignored" lines (`traffic_causes.rs:13-15`, `traffic_junction_box.rs:17-18`).
- `left_in_the_box` (`traffic_causes.rs:418`) and `run` in `traffic_junction_box.rs` build the oracle
  `with_third_bodies()` and print `relaxed_max_depth`.
- Re-anchor the G4 lease clause (`traffic_junction_box.rs:204-221`): a holder whose `relaxed` (planning
  phase) names the body is not "held by the body", because its path is not blocked by it (D2). Read
  `TrafficCar.relaxed` in the `cars` map.
- Flip: the lease rule off, on a non-relaxed holder, still turns the clause RED (record).
- The G4 liveness bound stays `MAX_STOP` 40 s; R1 stays 30 s.

3.5 Flips, each recorded in the stage summary (input perturbed, RED seen, restored, GREEN):
- (a) the sink trigger off (`detect` accepts no sink candidates): M1, N1, class D (R1 seed 1) and
  class E (R1 seed 7, G4 seed 7) RED;
- (b) the cycle break off: the cycle row RED;
- (c) the relaxation off (all consumers ignore `relaxed`, the hook returns `true`, no markers): the rows
  RED, through stands or G1;
- (d) relax-all (consumers skip every body while `relaxed.is_some()`): the third-body oracle RED in at
  least one row;
- (e) `wait_seconds` 40 in the shipped RON: M1 RED on the 30 s bound;
- (f) the least-overlap order replaced by the chain end, or by V2's per-edge "X stood ≥ T": the M1
  report shows D→P chosen first with the deeper overlap. This is report-level evidence for binding
  question 1; the depth is not a pass bound;
- (g) the grace rule replaced by `min(standing(W), standing(B)) >= T`: N1 RED (V2 R4);
- (h) "past B" end allowed for a `Dynamic` car: the cycle row RED (Y never recovers, example 2);
- (i) the wheel-ray predicate off: row 2.10d RED (already in stage 2, listed for completeness).

3.6 Latency.
- Record, per row: trigger tick, end of the waiter's stand, and the longest stand of any car in the
  scene.
- If any row's longest stand is > 27 s: lower `wait_seconds` to no less than 16 (the 14 s windows plus
  2) and rerun. Otherwise stop.
- A second failure of the same class: stop with a redesign note (the second-failure rule).
- `grace_seconds` cannot go below `pass.character_seconds` (validator).

### 2.F Stage 4: regression, Linux, docs

4.1 Run:
- `cargo test -p gta_sim -p citygen`: every G1 gate, `traffic_gridlock` seeds 1/2/7/42;
- `traffic_bench` / `police_bench` / `civilian_bench` under `MEAN_LIMIT` (the hook runs per touching
  flagged pair, the upkeep per relaxed car);
- `cargo clippy`;
- `cargo test -p gta_like --bin gta_like`;
- `python tools/qa/tree_check.py`;
- `cargo tree -p gta_sim -e normal -i bevy_render` still empty.

4.2 Print `unexplained` (max), `progress_by_trigger` and `progress_recoveries` for every city gate.
Trace and justify any relaxation in `traffic_gridlock`, never silence it. Run every city gate's stand
bounds (D5, TASK-032 lesson). No existing per-case rule is removed (D5).

4.3 Contact oracle sweep on the FIXED code (TASK-036 lesson): the city G1 gates through the probe with
`with_third_bodies()`. Zero violations outside exempt pairs.

4.4 Linux: WSL Ubuntu 22.04, toolchain 1.95.0, `maw/tasks/done/TASK-037/scratch/fixer/linux_run.sh` +
`scratch/wsl/`, sources mirrored to the WSL filesystem, `CARGO_TARGET_DIR` there.
- Rows: every row of `traffic_progress`, the three un-ignored rows, `traffic_gridlock`,
  `traffic_junction_box`, `traffic_causes`, `traffic_go_around`, `traffic_recovery`.
- A Linux-only red is a real bug on another trajectory: trace it, never "flaky".

4.5 Docs.
- `docs/architecture/traffic.md`: a new section "Progress: wait-for record and relaxed pass", covering:
  - the edges (the 2.A table), sinks and cycles;
  - eligibility (sink clock, grace) and the least-overlap order with the "being passed" exclusion;
  - clean-first (lane pass that goes now, box pass around a character alone, least-overlap exit);
  - the consumers by phase (the 2.A table), the hook, `PassingThrough` wheel rays, `TnuaNotPlatform`;
  - the end rule and `physics_only`, and the `progress` values;
  - one paragraph on D10 (below).
  - Replace the box section's "Open (TASK-039)" paragraph with the resolution.
  - In "Modes": a relaxed `Dynamic` car recovers past its blocker.
  - Record the exit-lane-start car as an observation (binding question 3, out of scope).
- `docs/design/GDD.md` §5.2 gets a new bullet (Russian):
  "Продвижение (TASK-039): машина, простоявшая `progress.wait_seconds` за стоящим телом (машина,
  человек, машина в коробке), когда объезда нет, медленно (`progress.speed`) протискивается мимо него:
  столкновение ослаблено только с этим телом, она может его задеть, человеку не наносит урона и не
  сбивает, на крышу не поднимает. Сначала пробуется чистый путь (объезд по полосе, объезд в коробке,
  другой выезд). Из цепочки ожидания первой едет машина с наименьшим перекрытием; циклическое ожидание
  ломается так же. Решение D1: небольшое визуальное несовершенство в кадре принято ради гарантированного
  движения; деспавн в кадре и продавливание по-прежнему запрещены."
- In the last bullet, the sentence "Машина в коробке, на которую игрок смотрит вблизи, не исчезает и
  может запереть перекрёсток (открытый вопрос TASK-032, R1)" becomes "Машина в коробке, на которую
  игрок смотрит вблизи, не исчезает; перекрёсток не запирается: машины проезжают её по правилу
  продвижения".

4.6 README "Статус", the AGENTS.md stage line and `docs/narrative-graph.md` are the orchestrator's
close-out, not this task's steps.

**D10, "one body owner"** (named option, not built):
- What it is: always-`Dynamic` traffic driven by the autopilot, or always-kinematic with a custom
  response.
- Source files: it touches `drive.rs` motion, removes `contact.rs` and `recover.rs`, and reworks
  `lateral.rs`, `manoeuvre.rs`, `pass.rs`, the `box_rules.rs` targets, the precision of
  `vehicle/autopilot.rs`, the `occupancy` kinds, `spawn.rs`, and the sirens/police interplay. About
  12-15 files.
- Gates to re-anchor: `traffic_contact` (6 rows), `traffic_recovery` (9), `traffic_idm`,
  `traffic_intersection` (conflict points with overshoot), the `traffic_graph` oracle assumptions, the
  G1 kinematic tolerance, `traffic_bench` `MEAN_LIMIT` (24 wheel-raycast bodies), and every city gate
  trajectory. About 25 gate files, 3-5 tasks.
- It still needs a progress rule: traffic cannot shove a parked car (TASK-032). Not recommended now.

**D9 U-turn:** not built unless stage 3 shows class E not cleared by the rule. If it is built: only
when every exit is blocked, never in the random `out[rng]` draw, and through the same trigger.

### 2.G Stage 5: runtime QA (QA role)

- TASK-040 repros over BRP, scripts in `maw/tasks/done/TASK-040/scratch/tools/`:
  `m1_fixed.py <out> 34.6 -79.1`, `repro_player_in_junction.py`, `repro_abandoned_car.py`.
- Pass conditions: no traffic car stands > 30 s behind the stationary body; the screenshots show no
  car through a third body; the player is not lifted in N1.
- Read `TrafficStats` (`progress_by_trigger`, `unexplained`) over BRP: the resource is reflected.
- Owner run (not gated): the squeeze past a left car; M1 (which car went first, D's path and depth, the
  speed); a car past the player standing in a junction.

## 3. Test plan

| Gate | Class | Where | Expected | Flip |
|---|---|---|---|---|
| `oracle_sees_a_car_through_a_dummy` | oracle correctness | `traffic_progress.rs` | character and static violations seen; exempt pair not | (c) of 1.4 |
| `relaxed_car_drives_through_a_standing_player` | mechanism | same | no hit, no damage, no knock-down, y ±0.1, stays kinematic | no `TnuaNotPlatform` |
| `relaxed_pair_ends_only_when_separated` | mechanism (R3) | same | no contact or switch while overlapped after B moved | end on "B moved" |
| `relaxed_pair_has_no_contact` | mechanism (hook) | same | sleeping parked car moves < 0.02 m | hook `true` |
| `relaxed_car_passes_an_awake_parked_car` | mechanism (wheel rays) | same | awake parked car moves < 0.02 m, tilt < 1° | predicate off |
| `m1_two_bodies_block_both_lanes_seed_1` | liveness + correctness | same | stands ≤ 30 s, no Dynamic > 30 s, oracle clean, ≥ 4 per lane, sink trigger | (a) (c) (e) (f) |
| `n1_player_standing_in_the_box_seed_1` | liveness + correctness | same | stands ≤ 30 s, player unhurt, not lifted | (a) (c) (g) |
| `cycle_two_pinned_dynamic_cars` | liveness (cycle) | same | both leave Dynamic ≤ 30 s, `[0, 1]`, front car breaks | (b) (h) |
| b3 re-anchored | liveness + correctness | `traffic_causes.rs` | Dynamic until T, rear past the dummy before removal, dummy unhurt | (a) |
| R1 seed 1 / seed 7, G4 seed 7 liveness (un-ignored) | liveness | `traffic_causes.rs`, `traffic_junction_box.rs` | 30 s / 40 s bounds, G1 + third-body clean | (a) (c) |
| G4 lease clause | correctness | `traffic_junction_box.rs` | relaxed holder not "held" | lease off on a non-relaxed holder |
| config sabotage | loader | `config_traffic.rs` | each new rule fires its keyword | one fixture per rule |
| full suite, benches, clippy, client gates, tree check | regression | CI + local | green, `MEAN_LIMIT` held | – |

- The flip steps are in 3.5 and 2.10.
- Presentation gates are not touched. The look of a squeeze is judged by the owner run (not gated).
- Run the new rows 3 times on Windows before reporting them green (city trajectories are
  phase-sensitive), and once on Linux.

## 4. Rollout notes

- **Config:** `TrafficConfig` gains a required `progress` block (`deny_unknown_fields`). The shipped
  RON is updated in the same commit, and any out-of-tree RON without it fails to load with a
  field-naming error. No env vars, no feature flags.
- **Physics composition:** the app's single `CollisionHooks` slot is now `TrafficHooks`. Any future
  hook must extend it.
- **Reflection/BRP:** `TrafficCar` gains `relaxed` (reflected, `Option<Relax>`). Every BRP script only
  reads `TrafficCar` (t14/t15/t16, TASK-040 tools), so nothing breaks. `TrafficStats` gains three
  fields.
- **Behaviour change (player-visible, D1):** cars squeeze slowly through a standing body after 18 s.
  GDD §5.2 records it.
- **Trajectories:** hooks, the new field and the relaxations move every city trajectory (TASK-037 lesson).
  Expect latent reds elsewhere, and trace the contact before blaming the change.
- **Dependencies:** no new crate, `Cargo.lock` unchanged, `bevy_render` still absent from `gta_sim`.
- **Files:** `drive.rs` 662 lines (logic in `progress.rs`; stay < 750). `traffic_support/mod.rs` 735
  (new code in `third_body.rs`).

## 5. Review notes (what changed from PLAN_V2 and why)

1. **Wheel rays (new, MAJOR).** §0. V2 treated the hook as the whole physics relaxation. An awake
   blocker's suspension rays see the passer's hull and lift it. Added step 2.4 and row 2.10d.
   Class D's left car and M1's D are awake by construction.
2. **Eligibility clock (MAJOR, binding question 1).** V2 gated each `Body` edge on its own target's
   standing ≥ T. In M1 that makes D→P eligible 2 s before S→D, so the chain end goes first (example 1).
   Now the sink's standing ≥ T gates the whole tree, intermediate targets need grace, and least overlap
   orders the candidates. This still is "the blocker's standing plus grace" (PR1, accepted) at the
   chain level.
3. **Order and serialization (MAJOR).** V2's "at most one new relaxation per chain per tick" is
   ambiguous: chains merge in a functional graph, and a passer that moves loses its edge, so a tree
   rule does not serialize.
   - Replaced by "a car being passed is never picked as a waiter". This keeps M1's order.
   - It lets co-granted holders (N1) and several approaches (class E) go in parallel: grants still
     separate them.
   - It is also needed by the end rule: a moving B would end the passer's planning mid-overlap.
4. **The D4 guard is dropped (MAJOR).** V2's separate "recovery relaxation" (corridor/`nobody_coming`
   only) is incoherent.
   - After recovery, a `Rejoin` steps its lateral even at rest (`drive.rs:542-546`, `rate_at_rest`
     0.8). Without the hook, D slides into P and is switched again. With it, D goes through P at once,
     before S, defeating question 1.
   - `Dynamic` waiters are ordinary candidates now. Example 1 shows D's `Dynamic` stand at about 22 s
     under the 30 s bound. The M1 row asserts it.
5. **Consumers by phase (new table).** A `Dynamic` relaxed car keeps braking for B in sensing and
   `leader`, because its autopilot speed comes from that IDM. The switch skip is a physics consumer
   (V2 had it as planning in B1). Without it, a car recovered inside the switch skin flip-flops.
6. **End rule: "past B" only for a kinematic car (new).** In the cycle pair (and whenever B is behind a
   `Dynamic` car), "past" would fire on the first tick and restart the relaxation forever. Flip (h).
7. **Cycle fixture replaced.** V2's two held cars on crossing straight connectors cannot block each
   other without overlapping: a body ahead in one car's strip lies beside the other. It is replaced by
   two pinned `Dynamic` cars (example 2): realistic, RED on HEAD by `recover.rs`'s design, and the
   least-overlap breaker is the front car with 0 overlap.
8. **Clean-first made terminating.**
   - A relaxed car takes a lane pass only if `may_go` holds at once. V2's "`max_seconds` elapsed then
     relaxed drive" conflicted with its own B4, where `max_seconds` ends the relaxation.
   - V2's `idle` widening for `Rejoin` is dropped: a `Pass { go: false }` targets lateral 0
     (`lateral.rs:21-31`).
   - The box-pass refusal around characters is lifted only in the relaxed branch.
9. **Speed cap (binding question 4, after V2).** New `progress.speed` 3.0 m/s ≤ `pass.speed`. A relaxed
   car on its own line otherwise reaches `lane.v0`.
10. **Values fixed and validated.** `grace 6`, `cycle 8`, `max 30`, `speed 3` and `wait 18` are given
    with a derivation (V2 left three open). Validator rules are exact.
11. **Upkeep is a system, not a loop in `drive.rs`.** It keeps `drive.rs` < 750. It reads B's pose
    from the ECS rather than only the 150 m snapshot, so a far B is not "gone" mid-overlap. It owns
    the marker inserts, so hand-set relaxations in plumbing rows get markers too.
12. **Geometry corrected.** The sedan half width is 1.2 m (`sedan.ron:8`), not 0.9. M1's estimates are
    S→D ≈ 1.0 m and D→P ≈ 1.8 m (V2: 0.4 / 1.2). The accepted "about 1.2 m" brush may measure about
    1.8 m. The owner run judges it; the row reports it.
13. **Oracle row 1.4.** PLAN's "contact switch left on" would switch the car and make the kinematic
    oracle vacuous. The car is set `Taken` (stays kinematic), and the relaxation is re-set each tick.
14. **Line numbers.** `traffic_recovery` c/e are at `:183`/`:347`, with the 14 s loops at `:199`/`:312`
    (PLAN said 203/314). V2 R7 corrections are kept. `bumped`'s pusher is at `traffic_causes.rs:149-170`.
15. **Kept from V2:** `modify_contacts` over `filter_pairs`; `Grant` never relaxed; the physics
    exemption lasts until separation (`physics_only`); `TnuaNotPlatform` mandatory; `leader` as a
    consumer; `TrafficStats` carried; the b3 re-anchor with a pressed-time parameter; the HEAD
    third-body baseline escalated, never tolerated; D9/D10 not built.
16. **Kept from PLAN (detail V2 dropped):** fixture coordinates and `GATE BROKEN` conditions, the
    `repick_relaxed` contract, the `penetration` semantics, the probe recipe, the GDD Russian text
    (extended), the D10 estimate and the research sources.
    - SUMO `--ignore-junction-blocker` / `--time-to-teleport`: https://sumo.dlr.de/docs/Simulation/Intersections.html,
      https://sumo.dlr.de/docs/Simulation/Why_Vehicles_are_teleporting.html
    - Wait-for graph (functional graph, O(n) pointer walk): https://en.wikipedia.org/wiki/Wait-for_graph
    - Not load-bearing: the local precedent (`pass.rs::passable` keys on the blocker's standing) and the
      worked examples carry the design.

Open questions for the owner: none new. The owner run items in 2.G stand; M1's measured brush depth
(about 1.8 m by estimate, against the accepted about 1.2 m) is reported to the owner with the row.

## Orchestrator amendment after PLAN_BLOCKED (2026-09-28, binding; supersedes the conflicting parts above)
1. **Cycle trigger dropped (option a).** On HEAD a nose-to-tail Dynamic pair is not a cycle: the front car's IDM drives it off (IMPL_SUMMARY stage 0). No real wait cycle has been observed on the current code; TASK-033's node-83 cycle was ended by the lease. Building a cycle detector for a fixture that cannot lock is YAGNI. Remove the cycle trigger, `cycle_seconds`, example 2 and flips (b)/(h). The wait-for record keeps only what the sink trigger needs. If a real cycle ever shows in a city gate or at runtime, it gets its own task with that evidence.
2. **N1 re-derived on the real geometry.** The held car is the east approach's lane head (`Lane(297)`, s 63.18, grant 722) standing 4.3 m before the player on the crosswalk at the lane end. It is not in the box. N1 is therefore a `Body` edge on a lane (car → character), resolved by the same sink trigger: clean-first means a lane pass if one fits (it usually will not at the stop line), otherwise the relaxed pass through the player with the mandatory Tnua guard and zero damage (D3). Drop the in-box clean-first branch for N1. Re-derive example 3's timeline from the stage-0 trace. The N1 row uses the TASK-040 point and asserts the real head.
3. Everything else in PLAN_FINAL stands: M1 minimum-overlap order, class D inside the band before ~33 s, `physics_only` release, suspension rays skip the blocker, the third-body oracle as stage 1.
