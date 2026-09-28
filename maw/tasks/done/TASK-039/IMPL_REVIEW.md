# IMPL_REVIEW — TASK-039 (universal traffic progress rule)

Reviewed: commit 4699d94 on `feature/box-uturn` (all 24 changed files read in full or as full diffs),
against `TASK_FINAL.md`, `PLAN_FINAL.md` (with the binding amendment) and `IMPL_SUMMARY.md`, plus the
last `OPEN_DECISIONS.md` entry (orchestrator decisions (a)-(c), not re-argued here).

## 0. Disconfirmation (done first)

Counter-example written down before evaluating: "a relaxation can outlive its purpose: a car whose
relaxation is in `physics_only` while it still touches its blocker is permanently ineligible for a new
relaxation (`detect` requires `relaxed.is_none()`), so if it stops inside or next to a blocker that never
leaves, it stands forever, invisible to `unexplained`."

Search result: **the counter-example HOLDS**, through two concrete paths (derived from code, not executed):
1. **A `Dynamic` car whose recovery blocker is BEHIND it.** `recover_dynamic` (recover.rs:231-233) returns
   `Stay { blocker: corridor.err().flatten().or(coming) }`. Both `corridor_clear` (via `resting_in` over the
   grown current pose, k = 0) and `nobody_coming` (rest skin around the whole rect) report a resting
   vehicle standing behind the rear bumper. `edge_of` (progress.rs:246-248) takes `stay` before anything
   sensed ahead, so the edge is `Body -> car behind`. Its `overlap` is ~0 (the reach loop only walks
   forward), so it is accepted first. On the next tick `upkeep` (progress.rs:161-169) finds it `past` at once
   (IMPL deviation 2 removed the plan's "past only for a kinematic car" guard, PLAN_FINAL 2.A "Why past B
   needs the car to be kinematic"), sets `physics_only`; the pair still touches the skin-grown rect, so the
   relaxation stays; recovery no longer ignores that body, the body ahead still brakes the car, and the car
   can never be relaxed again. If the body behind is a non-AI sink (the TASK-037 QA "second shape": the
   player's car pressed against an approach head) this is a permanent `Dynamic` stand, the exact D4 class.
   b3 escapes only because its front blocker is a character: `corridor_clear` checks non-resting bodies
   before `resting_in`, so the dummy wins over the pusher by check order, not by design. In the Linux
   `c_dbg.log` 2078's Stay blocker flips between 2088 (behind, s 92.7 vs 96.9, t 36-44) and 1999 (ahead).
2. **B moved, then stood again in the car's path; or `stale` fires mid-overlap.** Both set `physics_only`
   while the car may be inside B; sensing sees B again, the car stops inside it, and nothing re-arms.

## 1. Verdict

**NEEDS_WORK** — the mechanism (hook, wheel rays, Tnua guard, oracle, edges) is sound and well gated on
Windows, but the task is not done: two Linux regressions, R1 seed 1 still ignored, runtime QA not run.
There are also two defects the summary does not name: a physics_only relaxation can hold a car forever,
and a `Dynamic` car can be relaxed against the body behind it. The orchestrator's (a)-(c) address the
starvation. (b) needs scoping or it breaks M1 (section 6).

## 2. Confirmed correct

- **Contact hook** (progress.rs:36-53, lib.rs:194, spawn.rs `ActiveCollisionHooks::MODIFY_CONTACTS`).
  Verified in `avian3d-0.7.0/src/collision/narrow_phase/system_param.rs:774-781`: a `false` clears the
  manifolds and `TOUCHING` every step, so there is no `CollisionStart` and `apply_impacts`
  (vehicle/impact.rs:50, the only `VehicleHit` writer) cannot fire. The pair flag comes from the proxy union
  (`bvh_broad_phase.rs:194`), so the flag on the traffic car is enough. It covers already-touching pairs,
  unlike `filter_pairs`. Body-level resolution (`body1.unwrap_or(collider1)`) covers child colliders. The
  head hitbox (`CollisionLayers::new(Hitbox, NONE)`, character/mod.rs:147) forms no pairs.
- **Wheel rays** (chassis.rs:135-150). The only vehicle collider is the chassis hull on the body entity
  (vehicle/mod.rs:163-176), so the predicate's collider entity is the body. The mask `[World, Vehicle]`
  excludes characters. The predicate is symmetric (own marker, or the other's marker naming me). Flip d)
  (awake parked car rolled 180°) is recorded.
- **Tnua guard.** The vendored sensor skips `TnuaNotPlatform` hits (`vendor/bevy-tnua-avian3d-0.12.1/src/lib.rs:160, 249-251`).
  The marker is inserted only when B is a `Character`, only on a change, with `try_insert`/`try_remove`
  (progress.rs:187-194). Its one-tick latency after acceptance is harmless: acceptance happens with the pair
  apart, from rest.
- **D3 for the relaxed pair.** No channel other than `CollisionStart` hurts a character from a car. The
  physics exemption lasts until the skin-grown footprints separate (progress.rs:170-174), so contacts never
  resume mid-overlap. Row b) gates this, with a flip.
- **Occupancy semantics kept in `corridor_clear`.** The claim check moved to a second `blocked` call with
  `|_| true`. Verified in occupancy/query.rs:183-210: `skip` never applies to claims, so the claim check is
  unchanged.
- **Switch skips** (contact.rs predictive filter before `find_map`, backstop `exempt_pair`) are as planned.
- **Oracle row** `oracle_sees_a_car_through_a_dummy` has four falsifiable cases. The relax-all flip went
  RED in M1 and G4 seed 7.
- **Config**: one source, `deny_unknown_fields`, a validator rule per relation, a sabotage row per rule.
  There is no new tuning `const` (`REPICK_WITHIN` only changed visibility; the test consts are fixture law).
- Bevy/avian APIs used (`CollisionHooks`, `with_collision_hooks`, `cast_ray_predicate`, `MessageCursor`)
  exist in the pinned sources. `bevy_render` stays absent from `gta_sim` (summary; no new dependency).

## 3. Issues

### Critical

**C1. A physics_only relaxation can hold a car forever, and a `Dynamic` car can be relaxed against the body behind it.**
progress.rs:161-176 (`past` applied to any mode), progress.rs:246-248 (`stay` wins over the reason
ahead), progress.rs:515 (`snap.car.relaxed.is_none()`), recover.rs:231-233 (blocker order). Mechanism: §0.
- Why not gated: no row has a resting vehicle behind a `Dynamic` car with a stationary body ahead. b3's
  pusher loses to the character by check order. `unexplained` excludes relaxed cars (progress.rs:500), so
  the stat cannot see it either.
- Suggested fix:
  - (i) The `Stay` blocker ignores bodies wholly behind the car's rear. Reuse `past(&own, &shape)`: move it
    to `contact.rs` next to `penetration`. When the only corridor or coming body is behind, return the
    sensed body ahead, or `None`. The follower behind then gets its own edge to this car and squeezes
    through it, which frees the skin.
  - (ii) Put back "past B ends planning only when B was ahead at acceptance". For example, store
    `ahead: bool` in `Relax` at acceptance, or evaluate `past` only for a kinematic car as the plan had it.
  - (iii) Re-arm a physics_only relaxation against the SAME blocker. In `detect`, a car whose relaxation is
    `physics_only`, whose edge this tick is `Body -> relaxed.blocker`, and which has stood ≥ `grace_seconds`
    is a candidate again. On acceptance it gets a fresh `since` and `physics_only = false`. Never re-target
    a car that still overlaps another blocker (that would drop an exemption mid-overlap: D3).
  - (iv) Count relaxed-but-standing cars in a second stat (`stalled_relaxed`), so a stuck relaxation is
    visible to the gates.
- New row (see §4): a `Dynamic` car with a parked car 0.1 m behind and a dummy 0.35 m ahead. It must leave
  `Dynamic` in ≤ 30 s. Swap the order of the two bodies in `road.bodies()` (spawn order) to show that the
  outcome no longer depends on it.

### Major

**M1. The "stalled" exception counts only `Body` edges** (progress.rs:533-541). In the Linux trace, 2124
was relaxed against 2078 at 55.78 s (`since` 3570). From t 69 its edge was `Follow -> 2088` (a third car
between it and its blocker), so it stayed "passing". 2078 was excluded as "being passed" until about 80.5 s,
and its `Dynamic` stand reached 46.7 s (`c_dbg.log` lines 184, 246, 287). This is the concrete starvation
behind the `c_character_in_the_lane` regression. The fix belongs in (a), see §6.

**M2. `due` lets the waiter's own clock override the sink clock** (progress.rs:512, deviation 3).
`w_stood >= wait && s_stood >= grace` means that a car which queued 16 s for anything (a grant, a light
queue) and then gets a body that stood only 6 s in front squeezes through that body after 6 s. That drops
the D7 contract ("waited on a stationary body longer than T") for any long-queued car. The walker case is
the worst: a pedestrian pausing 6 s on the crosswalk in front of a head that waited 16 s for its grant gets
squeezed. The motivating case (a nudge restarting the sink clock) is narrower. Suggested fix: measure the
waiter's time behind the SAME target (a per-car `(target, since_tick)` kept on the snap or in
`TrafficCar`), and keep the OR only for that. Or accept the nudge restart and re-measure R1 seed 1 under
(a)-(c) first, since (c) may make it moot.

**M3. The squeeze pre-empts a clean lane pass that waits for a gap** (progress.rs:556-560 drops any
`Pass` around the blocker; config.rs allows `grace_seconds == pass.character_seconds`, and both are 6.0).
A car holding `Pass { go: false }` for moving oncoming traffic is a `Body` waiter (edge_of:273-281). It is
relaxed the moment its grace equals the character wait, and it drives through the pedestrian or dummy.
That goes against D2 ("paths that clear the blocker are preferred"). It is the second half of the Linux
`dummy_street_seed_7` regression: HEAD waited and passed cleanly at 6 m/s, while here 6 cars squeezed
(`go_dbg.log` ACCEPT lines against 1931). (c) shortens the squeeze but does not change who squeezes.
Suggested fix: in `edge_of`, a `Pass { go: false }` whose `may_go` fails only because of moving oncoming
traffic or claims gets a `Follow` edge to that car or claim owner (the car waits for moving traffic, which
is progress), not `Body -> obstacle`. Alternatively make the validator strict (`grace > character_seconds`
with a named margin). Re-run `traffic_go_around` on Linux.

**M4. The third-body oracle is not independent for the exempt pair** (third_body.rs:15-21, 33-40;
mod.rs:311). The exemption set is read from the production `TrafficCar.relaxed`. If production relaxes
against the wrong body, keeps a relaxation too long, or starts one mid-overlap, the oracle exempts it. The
relax-all flip (d) catches "consumers skip everything", not "relaxed against the wrong body". Suggested
fix, cheap and independent:
- record, per exempt pair, the tick it first appeared;
- assert that at that tick the blocker's `LinearVelocity` ≤ `hold_speed` (it was standing);
- assert that the pair's depth then was ≤ the G1 tolerance (the relaxation starts apart);
- assert that the pair leaves the exempt set within `max_seconds` + a separation bound.
Each assertion needs its own flip: accept against a moving body; accept mid-overlap.

**M5. The cycle the amendment called nonexistent occurred in a city gate.** In `c_dbg.log` t 35-44,
2078 -> 2088 and 2088 -> 2078 are both `Body` with root `None`, together with 1999 and 2124. It dissolved
by itself after about 9 s. The binding amendment says a real cycle "gets its own task with that evidence".
The summary mentions it only in passing. The orchestrator should file it and not fold it into the fixer's
(a)-(c). Note that C1(i) removes the "behind" half of this cycle's edges.

**M6. Junction arbitration changed off-plan** (junction.rs:346-351: `(!in_box, stamp, bits)`). Every
city trajectory moves, the only justification is R1 seed 1 (41 -> 33 s), and that row is still ignored.
Demotion happens only with a body on the path (`stuck`, junction.rs:179-190), so re-granting the demoted
car first cannot livelock the lease. The risk is low, but the lease clause lost its only test signal (IMPL
item 13: "cannot go RED on the fixed code"). Suggested fix: keep it only if R1 seed 1 needs it after
(a)-(c), otherwise revert. If kept, add a floor row: an in-box demoted holder with a body on its path does
not take the grant back from the conflicting waiter.

### Minor

- **m1. A relaxation with no consumer.** A room / `lane_start_free` `Follow` edge to a non-snap body
  becomes `Body` after `wait_seconds` (progress.rs:482-494; "reasonless" = not a snap). The waiter is then
  relaxed, but `lane_start_free` (drive.rs:318-334) has no `ignore`, so nothing changes. The relaxation
  holds the pair for up to `max_seconds` and blocks that body's own turn. Out of scope (binding Q3), but do
  not relax it: keep such edges `Follow`, or skip non-path `Body` edges in `detect`.
- **m2. Recovery work is no longer gated.** `recover_dynamic` now runs `nobody_coming` and
  `corridor_clear` for every `Dynamic` car every tick (recover.rs:230-233). HEAD ran them only when
  upright, at rest, on its path and with nobody coming. This costs more in crash piles and chases.
  Compute the blocker only when the car is at rest (or only when an edge is needed).
- **m3. Frame cost of progress.rs is fine.** Each candidate is O(reach/0.3 × 2 laterals × exits) plus
  ≤ 3 `connector_clear`. Candidates are rare, and the bench shows 2.08 ms against a 19 ms limit. Two cheap
  wins: compute `overlap` after the exclusion check, since excluded candidates recompute every tick (2078:
  about 25 s); and return early from `upkeep` when no car is relaxed (it builds a `HashMap` of every car
  each tick). `roots` uses `stack.contains` (O(L²) per chain), which is fine at these lengths.
- **m4. `past` and `outline` duplicate the corner code** (progress.rs:77-93, 437-455; `occupancy::query::corners`
  exists) and allocate a `Vec` per call. Return `[Vec2; 4]`.
- **m5. `detect` is about 100 lines with nested closures.** Split it into `candidates()` and `accept()`
  before (a)/(b) land, or it will grow past readability.
- **m6. traffic_progress.rs is 838 lines** (over the 750 warning). The fixer adds rows for C1, (a) and
  (b), which puts it past 950. Split now:
  - `traffic_progress.rs`: the city rows M1, N1 and the new city rows;
  - `traffic_progress_plumbing.rs`: the oracle row and plumbing rows a)-d);
  - the shared `Unhurt`, `Contacts`, `watch`, `scene_failures` move to `traffic_support/progress.rs`,
    since `traffic_support/mod.rs` is already at 747 lines.

## 4. Missing coverage

- C1: a `Dynamic` car between a resting car behind (inside the rest skin) and a stationary body ahead
  leaves `Dynamic` in ≤ 30 s, with both spawn orders.
- C1(iii): "B moved, then stopped in the car's path". A dummy teleported 1 m forward onto the nose while
  the car overlaps it. The car must be past within a bound, and the relaxation must end.
- M1 / (a): a same-lane pile where a follower is relaxed behind another car. Stalled relaxations must end,
  and the blocker gets its turn within the bound. The floor version of `c_character`.
- M3: `traffic_go_around` dummy street, where the queue takes the clean lane pass when the oncoming gap
  comes, with 0 relaxations while oncoming traffic moves.
- M4: the oracle's independent start and end checks.
- A `Dynamic` relaxed car driving through a character (deviation 2 makes this reachable) with no
  `VehicleHit` and no knock-down on a floor row. b3 covers only a dummy pressed at the bumper.
- A hijacked relaxed car (`Taken`, physics_only while overlapping). The exemption must end on separation
  and the player's car must not pass through B after that.
- Linux: every row of the plan's 4.4 list after the fix. Runtime QA (stage 5) is still owed.

## 5. Nits

- `Reasons.leader` carries a speed that `edge_of` never reads (progress.rs:228, 257).
- The `EdgeKind::rank` comment says "a body first, then a grant". Fine, but name the tie rule in the
  module doc.
- config.rs: the `wait > grace` comment "The blocker's clock dominates" no longer describes M2's OR rule.

## 6. Orchestrator decisions (a)-(c): do they address the mechanism, and concrete designs

Starvation mechanism (verified in `c_dbg.log`):
1. several followers on the same lane are relaxed through one standing `Dynamic` AI car (2088 and 2124
   through 2078);
2. a passer that stalls behind a third car on a `Follow` edge keeps the blocker "being passed" for up to
   `max_seconds` (M1);
3. meanwhile the blocker's own eligible candidate (2078 -> 1999: sink stood 19-22 s at 55.78 s) loses the
   least-overlap sort to a passer through it, and is then excluded by `b == me`.

(a) and (b) together cut all three links. (c) shortens how long each squeeze holds the lane and the box.
It does not change who squeezes (see M3).

**(a) One squeezer per blocker** (in `detect`):
1. `busy: HashMap<Entity, Entity>` (blocker -> passer) from every snap with `relaxed.is_some()`, BOTH
   phases. In physics_only the passer is still in the blocker's recovery skin, so a second passer only
   lengthens the blocker's wait.
2. Skip a candidate `W -> B` while `busy` contains `B`, and insert on acceptance.
3. Release a stalled passer instead of letting it hold the slot. A planning relaxation whose edge this
   tick targets anything other than its blocker (`Follow` or `Body`: fixes M1), with `standing(W) ≥
   grace_seconds` and `tick - since ≥ grace` ticks, gets `physics_only = true` in `detect`. `upkeep` then
   ends it on separation, which is D3-safe. The implementer rejected "end stalled after grace" because the
   head re-picks D every 6 s. Step 4 plus (b) remove that re-pick.
4. Precondition on acceptance: W must be the nearest body to B on its own path. That is, `leader(W)`
   (skip `None`) is not a car that is itself relaxed against B or overlapping B's footprint. 2124 violated
   this.
5. Class E / N1 risk: PLAN_FINAL §5.3 kept parallel passers because serializing across approaches "stalls
   multi-approach class E / N1 queues past 30 s". Key the slot by `(blocker, W's approach)`, where the
   approach is `W.segment`'s source lane (a connector's `from_lane`). Passers from different approaches
   through a left car in the box then still go in parallel (grants separate them), and same-lane passers
   are serialized. Re-run G4 seed 7 (24.9 s now, bound 40) and R1 seed 7 (25.4 s, bound 30).

**(b) The standing AI blocker first:**
1. `self_due: HashSet<Entity>` = waiters of eligible candidates this tick, computed before sorting.
2. Drop a candidate `W -> B` when `B ∈ self_due` and `B` is on W's own path ahead. That means `B`'s snap
   segment is W's segment, or W's successor via `W.next` (`drive::successor`), with `B`'s s ahead. B's own
   candidate then goes through the normal order. The existing `waiter == blocker` exclusion keeps W from
   squeezing through B while B moves off.
3. **Do not apply it to side intruders.** Taken literally, (b) puts M1's D -> P (1.8 m) before S -> D
   (1.0 m): D is a standing AI car. That contradicts binding resolved question 1, and the recorded flip
   "order reversed" already turned M1 RED (eastbound queue 113.8 s). With the same-path scope, S -> D stays
   first (D sits on S's lane from the other lane), and the c_character pile (2088/2124 -> 2078 on lane 462)
   gets 2078 -> 1999 first. The M1 row is the check that the scope is right.
4. Combine with C1(i): a `Dynamic` blocker's own edge must point ahead, or "relax the blocker first"
   relaxes it against the car behind it, which is useless.

**(c) Squeeze speed = `pass.speed`:**
- remove `ProgressConfig.speed`, its validator rule and its `config_traffic` sabotage row, and the RON key
  and comment;
- `drive.rs` cap: `if sensing_blocker(snap).is_some() { v0 = v0.min(cfg.pass.speed) }`, the same value as
  the `Pass | Yield` cap;
- row a) reads `pass.speed`;
- update GDD §5.2 ("медленно (`progress.speed`)") and `traffic.md`.
- D3 is unaffected, since contacts are off at any speed. The Tnua marker latency stays below 0.1 m per
  tick, from rest. The look at 6 m/s goes to the owner run.

Re-measure after (a)-(c) + C1: Linux `c_character_in_the_lane`, `dummy_street_seed_7`, R1 seed 1 (the
orchestrator accepts a documented residue under the G4 40 s bound), M1 order (S -> D first), N1, G4
seed 7, then the full suite on both platforms and the stage-5 runtime QA.
