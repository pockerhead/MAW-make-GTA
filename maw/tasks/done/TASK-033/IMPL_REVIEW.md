# TASK-033 — IMPL_REVIEW (code-reviewer, claude opus, medium)

Reviewed: commits `6b11f66`, `ed45ab5`, `0a07686` against `064f02f` (spec `task.md`, small-fix, no plan;
`IMPL_SUMMARY.md` treated as a claim). All changed files read in full: `traffic/junction.rs`,
`traffic/drive.rs`, `traffic/graph.rs`, `traffic/mod.rs`, `traffic/spawn.rs`, `traffic/config.rs`,
`assets/traffic/traffic.ron`, `tests/config_traffic.rs`, `tests/traffic_gridlock.rs`.

## Disconfirmation (done first)

Counter-example I tested: **the lease is claimed to cap how long a standing car before its stop line holds
a grant. But after a lapse the same car re-requests in the same tick. If nobody else is waiting, it gets
the same `(connector, car)` grant back straight away.** Then (a) for an uncontested holder the "lease" is
only a renewal, and (b) the gate's stale-grant counter, which is keyed by a continuous `(c, e)` occupant
entry, keeps counting past `reservation_timeout`.

Result: **the counter-example holds in the code.** Path: `junction.rs:98` lapse removes the occupant →
`:136` the car is a head and no longer an occupant → `:140` new stamp `tick` → the grant loop in the same
call grants it again when nothing earlier conflicts → `:182` it goes back into `occupants`. The test samples
after the whole update, so it never sees the gap (`traffic_gridlock.rs:98-115`). Behaviour: harmless (no
competitor is starved). Gate: it asserts more than the code guarantees (see Issue 1). It is green today
because the only lapse the runs trigger (seed 42, 5.00 s) is contested.

Second counter-example: "a non-AI body at the lane head starves the followers, who then never queue". It does
**not** hold. `heads` comes from the AI-only occupancy. An abandoned or taken car is not in it, so the first
AI car behind it is the head. A Bailing car counts as a head, but a stopped Bailing car already blocks its
followers through the leader, and it leaves the occupancy once it is abandoned (`drive.rs:500-507`).

## 1. Verdict

**NEEDS_WORK.** The three root-cause rules are correct and the gridlock is gone. I reproduced it:
`traffic_gridlock` gave the same per-seed numbers as the summary. But the lease's own correctness assertion
has no flip-RED of its own and can go RED falsely on an uncontested re-grant. The fixer is changing the data
anyway, so the stand bound and the flips need re-deriving.

## 2. Confirmed correct

- **Head-of-lane queueing** (`junction.rs:62-75`, `:112`, `:136`): the head is the highest-`s` AI car with
  `s <= length`. Connector cars are projected past `length` in `occupancy` (`drive.rs:47-51`), so they are
  excluded. This breaks the follower-grant circular wait the summary describes (seed 1, node 83, lanes
  248/299). The implementer's flip shows it: heads off gives 59-62 s.
- **Lease** (`junction.rs:88-99`): the renewal tick updates while `speed >= hold_speed`. The grant lapses
  only on the source lane with the nose before `stop`. The `moved` map is pruned with its occupants (`:103`)
  and in `release` (`mod.rs:125`). `lease_ticks = ceil(timeout/dt)` (`drive.rs:281`). No u64 underflow:
  `last <= tick` always.
- **Stop line before the crosswalk** (`graph.rs:320-338`): per lane, from the sidewalk crossing in the lane's
  second half. citygen only makes crossings at road-edge ends (`citygen/src/graphs.rs:64`), so a mid-block
  crossing cannot pull a stop line far back. `stop` is used consistently by the stop-line obstacle
  (`drive.rs:352`), the never-run-the-line clamp (`drive.rs:430-434`), the request distance
  (`junction.rs:126`) and spawn (`spawn.rs:157, 188, 220`). The lease "past the line" test and the clamp
  use the same boundary (`s + half_length` vs `stop`).
- Data first: `reservation_timeout` and `crossing_clearance` are in `traffic.ron`, validated `> 0`, with
  keyword rows in `config_traffic.rs`. No new tuning `const` in src.
- Reproduced here: `cargo test -j 2 -p gta_sim --test traffic_gridlock` gives 5/5 green with seed 1 31.7 s,
  seed 2 34.8 s, seed 7 18.7 s, seed 42 38.8 s (stale 5.00 s). Identical to the summary.
  `cargo clippy -j 2 -p gta_sim --all-targets -D warnings` is clean.
- Per the orchestrator note, the log's decision to keep the grant past the stop line (kinematic casts skip
  kinematic cars, `drive.rs:327-333` / `contact.rs`) is sound.

## Orchestrator question: does `idm.acceleration: 1.5` break an existing traffic gate?

**No, measured.** I set `acceleration: 0.73 → 1.5` in `traffic.ron` temporarily, ran the tests, and restored
with `git checkout`. The sha256 `4c11da45…81d8e` is identical before and after, and `git status` is clean for
the file. All of these were green at 1.5:

| Gate | Result at a = 1.5 | Why it could have moved |
|---|---|---|
| `traffic_idm` (ring, 8 seeds × 6400) | ok; travelled min 411-425 m (loop 291 m) | a/b = 0.9 lowers string stability; no overlap/reverse seen |
| `traffic_intersection` (2 tests, 6 seeds) | ok; delivered 6-12 per approach | faster discharge only helps |
| `traffic_contact` (6) | ok | TTC switch reach is speed-based (`switch.reach` comment uses max_speed + v0, not a) |
| `police_car_floor` (7), incl. `police_car_pulls_away_behind_a_leader` | ok; top 4.51 m/s vs wanted 3.00 | **the only gate that reads `idm.acceleration`** (`police_car_floor.rs:435`, `0.5·a·4 s`): its threshold doubles, 1.5x margin left |
| `police_cars` (12), `police_pull_out` (7), `police_stopped_driver` (3), `police_range_edge` (6) | ok | `follow_speed` scales with a; the autopilot keeps up |
| `traffic_bubble`, `traffic_bailout`, `traffic_hijack`, `traffic_parked`, `traffic_pedestrian`, `traffic_graph`, `config_traffic` | ok | — |
| `traffic_gridlock` at 1.5 | ok: seed 1 18.8 s, seed 2 **34.3 s**, seed 7 18.0 s, seed 42 27.7 s; seed 42 stale 5.00 s (the lease still fires) | — |

The unit tests in `idm.rs:42-66` build their own config with 0.73, so they are unaffected. Not run by me:
`traffic_bench`, `vehicle_*`, the full `-p gta_sim -p citygen` sweep, and the client gates. The fixer runs
them. Note for the 40 s bound: seed 2 at 34.3 s leaves ~14 % margin. City stand times are phase-sensitive
(gates lesson TASK-022), so treat 40 s as the minimum, not a target, and print the margin.

Also, GDD §5.2 (`GDD.md:246`) names 0.73 as the **start** value ("Стартово"), so moving it is a data change
within the law. The `traffic.ron:4-5` comment ("IDM start values (GDD §5.2 …)") should say the value was
retuned in TASK-033 and why.

## 3. Issues

1. **major — `crates/gta_sim/tests/traffic_gridlock.rs:98-115, 162-167` vs `junction.rs:98, 136-140, 182`.**
   The stale-grant assertion measures one continuous `(c, e)` occupant entry. The code re-grants an
   uncontested lapsed holder in the same tick, so a car held > 5 s before its stop line by walkers, with no
   competing waiter at that node, turns the gate RED while the code behaves correctly. This is a false RED.
   The gate also over-claims the AC ("no reservation held > timeout by a non-progressing car"), which the
   code deliberately does not guarantee when nobody competes. Fix, pick one and say which in the summary:
   (a) the gate counts a hold as stale only while another car is waiting at the same node for a conflicting
   connector (read `Junction::waiters` + `TrafficGraph::connector(..).conflicts`); that is the property the
   lease exists for. Or (b) the code does not re-grant a lapsed holder in the tick it lapsed, and the gate
   stays as is. (a) is the smaller change and matches the design intent.
2. **major — gates flip-RED gap, `traffic_gridlock.rs:155-167`.** The lease's correctness assertion was never
   shown RED on its own. In `scratch/flip_lease_off.txt`, seeds 1/2/7 are byte-identical to the fixed run
   (the lease never fires there). Seed 42 fails on the stop-bound assert at line 155, **before** the stale
   assert at line 162 runs. So the lease is caught only by the liveness bound, and on one seed. Fix: evaluate
   both asserts before panicking (collect failures), re-run the lease-off flip, and record that the stale
   assertion goes RED. Better, add a deterministic small-fixture gate on the `plus()` floor: a granted head
   held before its stop line by a static body, plus one conflicting waiter. Assert that the waiter is granted
   within `reservation_timeout` + 1 tick and the holder re-queues. It does not depend on city phase and it is
   cheap.
3. **major (process, required by the orchestrator decision) — `traffic_gridlock.rs:31` `MAX_STOP = 60`.**
   With the 40 s bound and `a = 1.5`, all three flips (lease off, heads off, stop line at the lane end) must
   be re-run and recorded. The old flip numbers were taken at a = 0.73 against a 60 s bound. The heads-off
   flip was marginal then (62.1 s). At a = 1.5 it may drop under 40 s, and then that rule has no gate. If it
   goes green, say so and gate the head rule directly: in the seed runs, assert that no non-head car is in
   `waiters` or `occupants` at request time.
4. **minor — AC "runtime baseline on seed 1 shows no jam" is open.** `scratch/baseline_s1/summary.json` at
   a = 0.73: stopped_cars 6-18 and 27 jam-oracle hits. Re-run `baseline.py` after a = 1.5 and report the
   series. If the oracle still fires, name that as saturation for the owner, not as a pass.
5. **minor — lease scope vs the spec** (`junction.rs:93-99`): a holder stuck on a connector, or with its nose
   past the stop line, keeps its grant forever. An example is a player-parked car in the box ahead of it,
   which is not in the occupancy. That was the pre-existing behaviour, and the implementer logged it as a
   decision. Accepted, but name it in the summary as a known residual, since TASK-032 inherits it.

## 4. Missing coverage

- A deterministic lease unit gate (Issue 2): the holder lapses after the timeout, a conflicting waiter is
  granted, and the holder re-queues behind it. Plus the uncontested case: re-grant allowed, no starvation.
- A head-rule gate: a follower with a non-conflicting connector does not queue while its head is on the lane.
  After the head enters the connector, the follower queues within 1 tick.
- A lower bound in `stop_lines_leave_the_crosswalk_free`: `stop >= crossing − clearance − ε`. Today a stop
  line pulled far back by a stray sidewalk edge would pass. The test also shares the production `t > 0.5`
  filter (`:221` vs `graph.rs:331`), so that filter itself is unchecked.

## 5. Nits

- `junction::update` takes `lease: (u64, f32)` to stay under the argument count. A small named struct, or
  passing `&VehicleConfig`/`hold_speed` explicitly, reads better. Clippy already allows `too_many_arguments`.
- `Junction::moved` is a `pub` field whose invariant (keys ⊆ occupants) is kept only by `update`/`release`.
  Acceptable in the crate, worth a `///` line.
- `traffic.ron:20-22` repeats `keep_right 0.5` and `capsule 0.3` from other configs in a comment. It will go
  stale. The geometry gate already reads them from data.

children: 0 launched / 0 reported.
