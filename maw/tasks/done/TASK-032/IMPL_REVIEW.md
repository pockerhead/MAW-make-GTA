# IMPL_REVIEW — TASK-032 (code review, commits f95461d + 9b5c045 on feature/oncoming-lane)

Reviewer: code-reviewer (claude/opus, medium). Inputs read from disk: `TASK_FINAL.md`, `PLAN_FINAL.md`,
`IMPL_SUMMARY.md`, `REDESIGN_NOTE.md`, `OPEN_DECISIONS.md`, `log.jsonl`. Every production file of the diff
was read in full (`occupancy/{mod,query}.rs`, `traffic/{drive,junction,box_rules,pass,manoeuvre,lateral,
recover,sirens,stuck,lanes,spawn,mod}.rs`, `traffic/{graph,config}.rs` diffs, `police/{siren,spawn_sector}.rs`,
`police/{car_route,car_dispatch,mod,cars}.rs` diffs, data, GDD, `traffic.md`). Tests read: `traffic_support`,
`traffic_causes`, `traffic_junction_box`, `traffic_recovery`, `traffic_occupancy`, `police_sirens`, heads of
`traffic_go_around` and `police_spawn_sectors`, the `police_cars`/`new_city`/`traffic_gridlock` diffs.

Commands run by me (foreground, this tree @ 1ea2aef):
- `cargo test -p gta_sim --test traffic_causes --test traffic_junction_box --test traffic_occupancy --test traffic_recovery` → 9 + 3 + 8 + 5 passed.
- `cargo test -p gta_sim --test traffic_go_around --test police_sirens --test police_car_floor --test traffic_intersection --test traffic_pedestrian --test police_spawn_sectors` → 10 + 4 + 7 + 3 + 4 + 2 passed.
- `cargo clippy -p gta_sim --all-targets -- -D warnings` → clean. Largest touched files: `police/cars.rs` 716, `police/mod.rs` 661, `traffic/drive.rs` 628 (all < 750).
- Full suite, benches and runtime QA were NOT re-run by me (the summary's numbers stand as claims).

## Disconfirmation (done before the evaluation)

Counter-example written first: "the stuck cheat (`traffic/stuck.rs`) despawns the player's current car, a
police car, or a car the player can see".

Result, checked in code:
- Player's current car: HOLDS. A hijacked traffic car is `Taken` (excluded, `stuck.rs:39`); a non-traffic car
  needs `car.driver.is_none()` (`stuck.rs:40`).
- Police car: HOLDS (`Without<PoliceCar>`, `stuck.rs:26`).
- Something in view: PARTLY BROKEN. `in_frame(.., distance = bubble.in_view.despawn)` returns false for
  anything farther than 90 m (`spawn.rs:28`), so a driverless non-traffic car in a box 90-150 m away and
  inside the view cone counts as "out of frame" and pops after 45 s (issue 4). Traffic cars already pop at
  that distance under the old bubble rule, so this is new only for non-traffic cars.

While tracing claims for the same hunt I found a worse stale-state bug (issue 1).

## 1. Verdict

**NEEDS_WORK.** Stages 1-7 are built as planned and every gate I ran is green. Four things block the merge:
- G6 and R1 are still open. The orchestrator decided A and R-A, and the fixer builds them.
- A hijacked car that was mid-pass leaves a phantom pass claim that freezes the oncoming lane (major).
- The spec's city-wide "no `Dynamic` car stands > 30 s" clause is asserted in one gate only. R-A makes it
  load-bearing.

## 2. Confirmed correct

- **One snapshot, one API.** `occupancy/mod.rs:105-186` builds the snapshot each fixed tick, before `Drive`
  and `Bubble` (`traffic/mod.rs:284-291`, `occupancy/mod.rs:199-203`) and before police
  (`police/mod.rs:608-612`).
  - Disabled bodies are skipped. Standing time restarts on a teleport (`:150-156`, gated by
    `standing_restarts_after_a_teleport`).
  - `reset_occupancy` runs on `NEW_CITY`. Its only state carried across ticks (`standing`) is pruned to live
    bodies on every rebuild.
- **Strip and overlap queries.** `query.rs:79-209` matches the plan's math: Sutherland-Hodgman band clip,
  bodies behind the origin excluded, gap 0 when a body straddles the origin, opposite claims only, 1 cm
  shrink. Unit rows cover the listed edge cases.
- **The kinematic-skip hole is closed.** `manoeuvre.rs:68-72` skips `OnPathTraffic` only for a plain car on
  its line. Every off-path, dynamic, vehicle or character body is seen, and so is every oncoming claim.
  - This matches the path `Occupancy`/`leader()` coverage (`drive.rs:51-67`). The kind is taken at the same
    moment the snaps are built, so the two sets cannot disagree within a tick.
- **Lane start and spawner migrated with today's semantics** (`drive.rs:296-311`, `spawn.rs:213-220`). Spawner
  acceptance keeps the RNG draw pattern.
- **Recovery.**
  - `recover.rs` has the hysteresis sweep plus the sampled rejoin corridor. Its step constants are geometry
    law with the derivation written down (`:25-28`).
  - `stood` counts only `Dynamic` standing time, which fixed the `traffic_pedestrian` regression the log
    records.
  - A car with no free door is abandoned at once (`drive.rs:268-277`).
  - Config laws check the skin against the switch's (`config.rs:254-266`).
- **Go-around.**
  - Hold at `hold_s` only while `s <= hold_s` (`drive.rs:383, 512-519`; the log dead end explains why).
  - The claim is published at commit, and the car moves out only when the claim is empty and no oncoming car
    is too close to stop (`pass.rs:161-185`).
  - Refused near the lane end (`pass.rs:214`).
- **Junction box.** D1/D2/D3 in `junction.rs`:
  - D2 is limited to holders blocked by a body (walker-held holders keep their lease; TASK-033 rule).
  - A stale `whole` grant clears itself on the next tick when its car is gone (`junction.rs:171-185`), even
    though `release()` does not touch `whole`.
- **Sirens.**
  - `sirens_on` is Respond | Chase. The yield trigger reads heading, not velocity. `yield_gap = v²/2b` (not
    IDM's rest point).
  - No yield when the siren car has no way past (`sirens.rs:222-232`). Resume on pass or timeout, then a
    deaf period.
- **Police.**
  - `lane_frame` keeps the car's own lane while it is out in the opposite one.
  - Lane choice goes home on a tie. The corridor-strip IDM replaces the heading cast only while sirens are on.
  - `approach_clear`, `lane_costs_to` and `step_cost` were removed cleanly with their test.
  - Sector quota draws no RNG (`spawn_sector.rs:213-219`). Occlusion rays go to the 4 chassis corners within
    the same 4-ray budget (`car_dispatch.rs:190-195`, `OCCLUSION_RAYS_PER_POINT = 4`).
  - `reset_dispatcher` resets the sector fields (`police/mod.rs:654`, `new_city.rs`).
- **Data and docs.**
  - Every new traffic/police tuning key is in `traffic.ron`/`escalation.ron` with a "why" comment, strict
    loaders and validate rules.
  - GDD §5.2/§5.3 amendments are in Russian. `traffic.md` has the occupancy, box, sirens and cheat sections.
  - `t15.py --seed` and TASK-036 (`maw/tasks/pending/TASK-036/task.md`) exist.
- **G1 oracle.** `traffic_support::Footprints` is an independent 4-axis SAT on `Position`/`Rotation`. Its
  tolerances match the plan.

## 3. Issues

### Issue 1 — MAJOR — `occupancy/mod.rs:170-173` with `traffic/hijack.rs:115-117`: phantom pass claim from a hijacked car

**What happens.**
- `snapshot_road` derives a claim for every `TrafficCar`, whatever its mode.
- `derived_claim` checks only `manoeuvre == Pass` and `segment == Lane` (`pass.rs:49-54`).
- `on_hijack` sets `mode = Taken` but does not reset `manoeuvre`, `lateral`, `calm`, `stood` or `deaf`.
  `abandon()` does reset them.
- `advance_traffic` skips non-AI cars, so the `Taken` car's `segment` and `s` freeze.

**Result.** A car hijacked in `Pass { .. }` keeps publishing its pass claim at the hijack spot for as long as
the player drives it. The likely case is `go: false`: the queue head waiting behind the player's own left
car, which is the car a player takes.

**Effect of the phantom claim.**
- `first_along` returns it to every oncoming strip (`query.rs:160-177`), and `passable` never passes a claim.
  Oncoming cars stop before it and stand until the player leaves the car. This is the M1 class, silent.
- It also blocks lane starts (`ClaimFilter::All`), spawns, recovery corridors and new passes.

**Fix.**
- Reset the manoeuvre state on hijack. Simplest: call a shared `clear_manoeuvre(&mut car)` from both
  `abandon()` and `on_hijack`.
- And/or derive claims only for `car.is_ai()` in `snapshot_road`.
- Gate: a G8 row that hijacks a car in `Pass { go: false }` and asserts that `RoadOccupancy::claims()` holds
  no claim of that owner next tick, and that an oncoming car drives past the spot. Flip: remove the reset.

### Issue 2 — MAJOR (coverage the spec requires, made load-bearing by R-A) — "no Dynamic car stands > 30 s" is asserted only in `traffic_go_around.rs:188`

TASK_FINAL G3 says: "In every city gate run of this task, no AI car in `Dynamic` stands longer than 30 s".
`StandClock::worst_dynamic()` is asserted only in G2. Elsewhere:
- `traffic_causes` (a, c, d), `traffic_junction_box`, `traffic_gridlock`, `police_sirens` and
  `police_spawn_sectors` do not assert it.
- G4 checks all stands at 40 s, and cause (d) checks at 46 s. Both are looser than 30 s.

R-A turns box cars into long-lived `Dynamic` pushers with give-up disabled, so this is exactly the bound that
catches a stalled pusher.

**Fix.** Add `worst_dynamic() <= 30 s` to every city gate of this task, collected with the other violations.
Start with G4 and cause (d).

### Issue 3 — MAJOR for the fixer (R-A impact; the orchestrator asked for the list) — what R-A changes or breaks

R-A: a car held by a standing body in or at a box pushes through as `Dynamic`, with no give-up in the box or
within a car length of its entry. These are concrete code interactions R-A must handle, and the gates that
change.

**Code paths that stop a pusher today:**
1. **D2 demotion then hold.**
   - A pusher on its connector that stands contested for the lease, with a body on its path, is demoted
     (`junction.rs:147, 156-161`).
   - A car on a connector without a grant is `held`, which gives `pilot.speed = 0` (`drive.rs:427-437, 457`).
   - A `Dynamic` car on a connector never re-requests (`junction.rs:203`).
   - The demoted waiter is re-granted only when `connector_clear` passes, and the body it pushes blocks
     that (`junction.rs:314-325`).

   So a pusher that stalls once stands forever, now with no give-up. It needs a grant that D1/D2 do not
   revoke, such as the whole-box grant, or an exemption from `held`.
2. **Entry needs a grant.** At the stop line, `reproject` moves a `Dynamic` car onto its connector only with
   a grant (`drive.rs:148`), and the autopilot target stops at the lane end without one (`drive.rs:449`).
   D1 refuses grants through a body, so "at the box entry" needs the D1 exemption above.
3. **`lost` still abandons in the box.** It is a separate path from give-up (`drive.rs:261` → `abandon`):
   off > 4 m or heading > 60° from the path tangent. A car shoving another on a curved connector can pass
   60° and become the second body in the box again, the R1 class. Decide whether `lost` is also suspended
   inside a box (for example measured from the connector tangent with a box margin), and gate it.

**Scope limits R-A needs:**
4. **Characters are never pushed.** Cars must not shove walkers or bodies on a crosswalk (TASK-033/035). Gates:
   `traffic_intersection::contested_lease_lapses_to_the_waiter` (dummy in the box), `traffic_pedestrian` (both
   hold gates), `lethality`.
5. **"Don't block the box" stays.** A body past the box, on the destination lane start (the
   `lane_start_free`/`room` wait), must not trigger a push. Otherwise
   `traffic_intersection::abandoned_car_past_the_box_holds_no_car_inside` goes RED and the TASK-016 invariant
   is lost. Scope R-A to bodies on the connector path or inside `in_junction`.
6. **Push speed.** A car pushing the player's occupied car or a police car deals impact damage above
   `damage.ron threshold_speed` 5 m/s. Keep the pusher's speed below it, or exclude an occupied player car.
7. **No `Manoeuvre::Pass` tag.** Do not mark a push-through as `Pass`. G2 asserts 0 `CollisionStart` for any
   car in `Pass` across the whole city run (`traffic_go_around.rs`), so a box push there would fail G2 for
   the wrong reason.

**Gates whose meaning changes (update or rewrite):**
- `traffic_causes::d_car_left_on_a_connector_out_of_view`. It asserts the left car is gone
  (`traffic_causes.rs:321-323`). A pushed car can leave the box, and then the in-box cheat no longer applies,
  so the row goes RED for the wrong reason. Rewrite it as a stand bound (30 s, plus `worst_dynamic`).
- `traffic_causes::d_car_left_on_a_connector_in_view_is_the_residue`. The residue acceptance is withdrawn, but
  the row asserts no liveness at all (`:329-339`). It should become the headless R1: in view, no traffic car
  near the box stands > 30 s. The runtime R1 stays the QA check.
- `traffic_junction_box` (G4): contact is now expected. The D2 flip meaning changes. The extra row (car behind
  the body on its connector) now tests push-through, not re-pick or box pass. G1 kinematic × dynamic depth
  (0.0625 m) now sees shoved bodies near kinematic stop-line cars. It relies on the TTC switch predicting
  them; run G4 3× and report the max depth.
- `traffic_gridlock` (4 seeds): expect drift in switch counts and stands if a walker-switched car in a box is
  now pushed; report as usual.
- Docs to update after R-A:
  - GDD §5.2 "В кадре ничего не исчезает" / residue wording.
  - `traffic.md` "A lock that no pass fits around ... is left to the stuck cheat" and "a rare lock the player
    looks at is accepted".
  - The `stuck.rs:3` module doc.

### Issue 4 — MINOR — `traffic/stuck.rs:43-46`: "out of frame" means "farther than 90 m" for non-traffic cars

`in_frame(.., distance = in_view.despawn)` treats a body beyond 90 m as off frame even inside the view cone.
The snapshot radius is 150 m, so `road.body` exists up to there. A driverless non-traffic car in a box
90-150 m ahead in plain view (for example the player's own car he walked away from and looks back at)
despawns after 45 s.

Also new: an `Abandoned` traffic car within 25 m but behind the camera now despawns after 45 s. The car the
player just hijacked and left is the typical case, and the old bubble rule kept it within 25 m.

**Fix.**
- Non-traffic branch: pass `f32::INFINITY` as the distance, as the spawner does (`spawn.rs:182`), so only the
  cone decides.
- Decide explicitly (owner run) whether the player's last car is exempt.

### Issue 5 — MINOR — `traffic/pass.rs:135`: literal `30.0` reach in `cannot_stop`

`let reach = 30.0f32.max(cfg.sense_distance)` is an unexplained magic distance in gameplay logic. It is safe
today: 16²/(2·8) + 2 = 18 m < 30. But it is neither data nor derived (data-first law). Derive it:
`max lane v0² / (2·idm.max_deceleration) + idm.min_gap`, with a comment. `REPICK_WITHIN = 1.0`
(`junction.rs:53`) also needs a one-line geometric derivation. The two connectors of one lane end diverge by
about s²/2r, 0.17 m at r = 3, which is the sideways snap `repick` accepts.

### Issue 6 — MINOR — `traffic/manoeuvre.rs:162-181` with `drive.rs:408-411`: a whole-box claim from `plan()` without its own grant lapses the next tick

On the connector branch, `plan()` sets `junction.whole = (me, to_lane)` even when the car holds no occupant
entry, for example after a D2 demotion. `junction::update` then finds no `moved[e]`, so `idle` is true and the
whole grant is cleared the next tick (`junction.rs:172`). The car is `held` again mid-pass, with a lateral
offset, until the grant path re-grants it. The code converges but thrashes.

**Fix.** Set `whole` only when the car also holds (or is given) the occupant entry, or insert `moved[e] = tick`
at the same time. Low priority if R-A replaces the box pass.

### Issue 7 — MINOR (coverage honesty) — `traffic_junction_box.rs`: the placed body can be out of frame, so the cheat can hide late locks

The busiest node is chosen within 60 m of the spawn regardless of the view (`:46-79`). If the body is out of
frame, the stuck cheat despawns it at 20 + 45 = 65 s. After that the remaining 55 s of the run measure a box
without the body, and a lock that starts more than 5 s after placement is never seen at 40 s.

**Fix.** Assert the body still exists at the end (`GATE BROKEN` otherwise), or place the player so the box is
in frame.

## 4. Missing coverage

- A hijack during `Pass { go: false }`: no phantom claim, and the oncoming lane keeps flowing (issue 1).
- `worst_dynamic() <= 30 s` in every city gate of the task (issue 2).
- A headless in-view box gate asserting a stand bound. This is the R1 case; today only the runtime R1 covers it
  (issue 3).
- A passer, box passer, yielder or recovering car making contact with a character. The premise amendment says
  "no one gets run over by the go-around manoeuvre". G1 checks vehicles only, and G2 counts `CollisionStart`
  against `Vehicle` only. Add a character to the G2 contact filter, or add a row with a walker stepping into
  the claim.
- A yield into the avenue curb lane with a parked car 10-20 m ahead of the yielder. The curb check covers only
  ±(half length + jam gap) around the car's current `s` (`sirens.rs:217-224`).
- The stuck cheat's two sides at the in-frame distance bound: a non-traffic car in a box at 100 m in the cone
  is kept (issue 4).
- G6 per decision A (fixer).

## 5. Nits

- `traffic_junction_box.rs:191`: `let _ = timeout;` is dead code (the timeout is read again in `check`).
- `stuck.rs:37-42`: `graph.in_junction` (a scan over every box) runs for every driverless non-traffic
  vehicle in the city each tick, before the cheap `road.body(entity)` test that limits it to 150 m. Check
  `standing` first.
- `car_dispatch.rs:181-184`: `position(|p| p.0 == points[k]).expect(..)`. It is correct because the points
  come from the candidates, but `pick_spawn` over an index vector would drop the float-equality round trip.
- Module layering: `occupancy` calls `traffic::derived_claim` and `police::sirens_on`, while both of those
  call `occupancy`. This is a domain cycle. It works in Rust and the plan asked for it, but claims could be
  pushed by `traffic` (as `insert_claim` already is) instead of pulled by the snapshot.
- `IMPL_SUMMARY.md:5-6` says the work is uncommitted on top of 193ca54. It is committed as 9b5c045.
- `police/siren.rs`: on entering a box `SirenLane` resets to 0, so a siren car out in the opposite lane gets a
  3.25 m sideways autopilot jump at the box edge. This is owner-run feel; flag it for the owner run.
- Frame cost: the summary's bench growth (+0.16 / +0.33 ms at the minimum) is within the host's run-to-run
  spread (±0.3 ms), so it neither proves nor disproves the 0.5 ms grid threshold. A `trace.py` exclusive-time
  number for `snapshot_road` + `advance_traffic` + `junction::update` at the 5★ bench would settle it.

children: 0 launched / 0 reported.
