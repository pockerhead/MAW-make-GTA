# IMPL_REVIEW — TASK-038 (code review of e33ba8e)

Reviewer: code-reviewer (claude opus, medium). Diff: `git diff main...e33ba8e`. Tree state is the same as e33ba8e plus
the task artifacts (4dd5069). Probe sources: `scratch/zz_cr038_{graph,gridlock,box}.rs`. They were copied into
`crates/gta_sim/tests/` only for the run and removed after. `git status` is clean.

## 1. Verdict

**NEEDS_WORK.** The swept-body conflict fix is correct, its gate is honest, and all callers are updated. But the
suite is still red (`rb_a_box_car_seen_from_afar_is_cleared`, on both platforms, which I confirmed on Linux).
Decision A is still to be applied by the fixer. Two points should be written down before merge: the fix costs
~43 % of junction concurrency, and `connector_clear` leaves the same pass-through class open through another path
(§3, I2).

## Disconfirmation (done before the evaluation)

The counter-example I looked for: **"the sweep makes opposite straights (or other normally-free pairs) conflict,
junction capacity collapses and a new gridlock appears."**

Probe `zz_cr038_graph.rs` (seeds 1..8 and 42). It classifies every pair that the new clause adds:
- **Opposite straights newly conflicting: 0 on every seed.** That part of the counter-example did not hold.
- But **every added pair involves a right turn**. Seed 1 has `R+R perp 409, R+S opp 197, R+S perp 197,
  L+R perp 214`. Right-turn connectors are **2.6 m long** (mean 5.6 m; left turns are 10.9 m), a radius of ~1.7 m.
  A 4.08 m body pivots almost in place and its nose and rear sweep into the neighbouring lanes. At seed-1 node 14,
  after the fix, the only concurrent pairs left are the two opposite straights (73/78, 77/82) and the opposite
  right turns (74/79, 75/83).
- Pairs allowed to hold grants together fall **from 2340 to 1323 on seed 1 (57 % kept)**, 52-58 % on all seeds.
  The summary reports "+23 % conflict entries". That is arithmetically true (entries count both directions), but
  it understates the cost. The table is symmetric and a superset of the old one (asserted in the probe).
- Gridlock with and without the sweep clause (sabotage `|| (false && sweeps_touch(..))`; sha256 of graph.rs was
  identical before and after the restore, `47ed8c3f…`). Logs: `scratch/cr_fix_gridlock_box.log` and
  `scratch/cr_nosweep_gridlock.log`, Windows.

| seed | worst stand, no sweep → sweep (s) | grants | stands > 20 s |
|---|---|---|---|
| 1 | 26.4 → 27.9 | 133 → 131 | 8 → 14 |
| 2 | 27.2 → 27.2 | 152 → 153 | 8 → 11 |
| 3 | **102.4 → 98.2 (RED on both)** | 101 → 95 | 14 → 19 |
| 4 | 13.6 → 17.1 | 126 → 117 | 0 → 0 |
| 5, 6, 7, 42 | identical | identical | identical |
| 8 | 29.1 → **34.8** (margin 5.2 s under 40) | 164 → 162 | 3 → 8 |

No deadlock appears. The stands are longer and the throughput is somewhat lower. Seed 3 gridlocks **before the
fix too**: a `Dynamic` stand of 70-102 s, the same node-141 class as TASK-037. So extending the gridlock gate to
seeds 1..8 would be red whatever this fix does. The junction-box G4 rows (non-liveness) on seeds 2,3,4,5,6,8 are
all green, with G1 max depth 0.000. Their liveness prints show `Dynamic` stands of 57-98 s, the known TASK-037
lock, which is ignored in the shipped rows.

Conclusion: the counter-example does not hold as a deadlock. It does hold as a real capacity cost, which the
summary understated (see I1).

## Log triage (dead_end entries)

- HashMap as the divergence source is rejected. That is plausible: each platform repeated itself, and the
  `libm_*` files show atan2/sin/cos differing while sqrt matches. I did not re-run the libm probe.
- "Swept table alone turns rb red": confirmed on Linux (`scratch/cr_linux_targets.log`: go_around 10/10,
  traffic_graph 2/2, causes 10 passed / 1 failed rb at traffic_causes.rs:528).
- Experiment B was not shipped, and the diff has no trace of it. Correct.

## 2. Confirmed correct

- **Mechanism.** `graph.rs:108-160` `body_sweep` / `sweeps_touch`, and `graph.rs:301-315`:
  - The conflict clause is ORed after the cheap checks, so it is only ever added.
  - The result is symmetric (checked for all pairs across 9 seeds).
  - The extent (nose at the connector start → rear at the connector end) matches the grant release rule
    (`junction.rs:156`, released when `car.s - half_length >= 0` on the exit lane).
  - `FlatRect` convention: `axis = right_of(tangent)` with `half.x` along it, the same as `FlatRect::of`.
- **Target gates.** `dummy_street_seed_1` and `a_left_car_seed_1` are green on Linux (my run) and Windows (the
  suite log).
- **`contact::rects_overlap`** (`contact.rs:115-124`): a correct 4-axis SAT with no allocation.
- **Callers of the new signature** (review item 3). Grep over `*.rs` for `TrafficGraph::new` / `from_layout`:
  - production `graph.rs:520`;
  - unit tests `graph.rs:574,629` and `lanes.rs:86`;
  - `tests/traffic_graph.rs:46,157` and `tests/traffic_support/mod.rs:46`;
  - client `src/bench/mod.rs:330`.

  All pass `Vec2(half.x, half.z)` or literals with 2.04 for the length. `cargo clippy --locked --workspace
  --all-targets -D warnings` is clean (my run).
- **The graph gate is a real gate.** Its flip-RED is recorded in `scratch/flip_red_graph_gate.txt` (198 pairs, the
  sabotage is on the mechanism's input). It also has a `GATE BROKEN` for `checked == 0`.
- **Load cost.** The graph build takes 49-68 ms per seed in the optimized test profile (old ~16 ms). It runs once,
  on `OnTransition{Loading→Playing}` (`traffic/mod.rs:327-331`). It is not a per-frame cost.
- **Rules.** No `unsafe`, no `unwrap` in production code. `graph.rs` is 658 lines (under 750). No new tuning const:
  `BODY_SAMPLE_STEP` is a discretisation law.

## 3. Issues

**I0 — major — `crates/gta_sim/tests/traffic_causes.rs:528` (rb). The suite is red on both platforms.** This is
known, and decision A is taken (OPEN_DECISIONS). Fix: apply A exactly as the orchestrator wrote it:
- scope the `Dynamic` bound of rb and (a) to the scene's approaches;
- print the stands outside the scene;
- add the TASK-037 item "restore the full-city bound".

Acceptance "full suite green" and "CI 5/5" can close only after that.

**I1 — major (documentation / decision record) — `IMPL_SUMMARY.md` §3 and `docs/architecture/traffic.md:13-19`.**
The cost of the fix is stated as "+23 % conflict entries". The real effect: 43 % of the concurrent pairs are gone,
all of them involving right turns, because right-turn connectors are 2.6 m long (radius ~1.7 m, an in-place pivot
of a 4.08 m body). Gridlock margins shrink (seed 8: 10.9 → 5.2 s under the 40 s bound; seed 1: long stands
8 → 14). The capacity loss is the geometric truth of the path cars drive, so the fix is right. But the root is
citygen's tiny right-turn radius, which the owner should know about. Fix:
- one line in traffic.md: "right turns (2.6 m connectors) conflict with almost everything at their node";
- a pointer in TASK-036 (connector geometry / conflict points) with these numbers.

**I2 — major, out of scope for the hotfix (review item 4) — `box_rules.rs:13-28` `connector_rects`, used by
`connector_clear` (`box_rules.rs:31-49`, `junction.rs:109`).** Yes, this is a pass-through risk of the same
class. My probe measured the unmargined body corners of a car on its connector: they reach up to **0.87-0.91 m
beyond centre line ± half width** on every seed. That is far more than the 0.48 m pair depth, and it is the
right-turn pivot again.

The path to G1: `connector_clear` is the only guard between a new grant and a body the conflict table does not
know. Examples of such bodies:
- a stuck holder demoted to a waiter while it stands **on its connector** (`junction.rs:149-153`), which stays
  kinematic;
- a box passer;
- a car left in the box.

If such a body sits in the overhang zone, a kinematic car on a conflicting connector gets its grant and drives
through it. For a dynamic body the solver pushes back, so it is a shove, not a pass-through. I did not
reproduce it in a scene, and no current gate went RED.

Recommendation: do NOT put this into the hotfix. Swapping `connector_rects` for `body_sweep` shapes changes
repick/stuck behaviour (the sweep reaches 4 m onto the exit lane, where queued cars would start blocking paths),
and that needs its own measurement. Record it in TASK-036 item 4 with the number (0.9 m) and the demoted-holder
path. Also add a G1 fixture: a kinematic car demoted on a right-turn connector, and a conflicting connector
granted.

**I3 — minor — `graph.rs:301-315` (build cost, bevy-ecs "every system is the frame").**
- The one-shot got 3.4× heavier (16 → ~54 ms inline on the transition frame).
- Cheap wins: compute each pair once (the relation is symmetric, and it is now evaluated for (i, j) and (j, i));
  hoist `bounds(b)` per sweep out of the pair loop (it is now a fold over ~35 rects for every pair); group
  connectors by node instead of the O(N²) `filter(node ==)`.
- Not blocking: it runs once at load, and the ignored `city_startup_budget` (2 s total) has room.

**I4 — minor — `graph.rs:108-111` (`BODY_SAMPLE_STEP` doc) and `tests/traffic_graph.rs:150-194` (oracle
independence, review item 2).**

Independent in the oracle:
- the SAT (citygen `convex_overlap`);
- the corner construction;
- no margin;
- a finer step.

The flip shows that it catches the mechanism.

Shared with the code under test:
- (a) `graph.pose`. This is legitimate, since kinematic cars drive exactly `offset_pose` (`drive.rs:539`).
- (b) The extent "nose at start → rear at end". It matches the release rule, but does not cover a holder that
  waits with its nose between `lane.stop` and `lane.to`. That area is outside the box and low risk.
- (c) `lateral == 0` at connector entry. `effective_lateral` (`lateral.rs:41-55`) lets a car enter a connector
  with a residual offset that decays over the connector (a yield or a lane pass ending near the box). Neither the
  table nor the oracle models it.
- (d) The oracle itself samples at 0.1 m. So the doc claim "the unmargined 0.1 m oracle holds" the 0.2 m sampling
  gap is weaker than it reads: both are discrete.

Fix: one line in the gate header naming (b)-(c) as out of the oracle's reach. A TASK-036 note for (c).

**I5 — minor — coupling `conflict_margin` ↔ `BODY_SAMPLE_STEP`.** The sampling gap between two poses is covered
only by `conflict_margin / 2` per body. If the owner sets `conflict_margin: 0.0` in `traffic.ron`, the gap is
silently uncovered. The graph gate would catch it only on seeds 1..8. Fix: a loader check (`conflict_margin >=
some minimum tied to the step`), or say it in the ron comment.

## 4. Missing coverage

- A G1 scene for I2: a kinematic body standing on a right-turn connector (a demoted holder) versus a granted car on
  a conflicting connector. Or at least a graph-level oracle: for every connector, the swept body versus
  `connector_rects` of every other connector at the node, as the `connector_clear` analogue of the new gate.
- Gridlock and junction-box gates run seeds 1,2,7,42 and 1,7 only. Seed 3 gridlocks (pre-existing, the `Dynamic`
  class). This belongs in TASK-037's acceptance as a row to restore, not in this hotfix.
- No gate pins the concurrency cost. For example, print (not assert) the count of free pairs per seed in
  `traffic_graph`, so a future table change shows its capacity delta.

## 5. Nits

- `graph.rs:301` `clear = 2*half_width + margin`: the centre-line clause is now almost always subsumed by the
  sweep (bodies grown by margin/2 each). It is kept as a cheap pre-filter. Fine, but a word in the comment would
  help.
- `graph.rs:302` comment "+-": the code base elsewhere uses "±" in docs. Cosmetic.
- `IMPL_SUMMARY.md` §3 "+23 %": replace it with the concurrency figure (I1).

## Evidence produced by this review

- `scratch/zz_cr038_graph.rs`: conflict classes, free pairs, build time, connector overhang.
- `scratch/zz_cr038_gridlock.rs` and `scratch/zz_cr038_box.rs`: seeds 3,4,5,6,8 and 2..8.
- `scratch/cr_fix_gridlock_box.log`, `scratch/cr_nosweep_gridlock.log` (Windows) and `scratch/cr_linux_targets.log`
  (WSL, 1.95.0).

children: 0 launched / 0 reported.
