# IMPL_SUMMARY — TASK-038 (small-fix): two traffic gates red on Linux CI after TASK-032

**Verdict: PARTIAL, a decision is needed (§6).** Both target gates are reproduced, explained and green on
Linux and Windows. The G1 pass-through is fixed in its mechanism (the conflict table) and gated. That fix
turns `traffic_causes::rb_a_box_car_seen_from_afar_is_cleared` RED on **both** platforms. Its cause is the
node-141 class (walkers pinned at a `Dynamic` grant holder), which TASK-036 item 1 and TASK-037 own. The
full suite is not green, so this is not mergeable as is.

Cost of error: silent (a pass-through or a stuck car that shows on some machines only), so full evidence layer.

## 1. Reproduction on Linux (acceptance 1: done)

Setup: WSL Ubuntu 22.04, `rustc 1.95.0` (the CI toolchain), CI env (`CARGO_INCREMENTAL=0`,
`CARGO_PROFILE_{DEV,TEST}_DEBUG=0`), `-j 2`. The sources are mirrored to the WSL file system with the
Linux target under `~/gta038/target` (`scratch/wsl_sync.sh`). Docker Desktop was not running.

```
wsl -d Ubuntu-22.04 -- bash scratch/wsl_sync.sh cargo test --locked -j 2 -p gta_sim --test traffic_causes --test traffic_go_around --no-run
wsl -d Ubuntu-22.04 -- bash scratch/linux_run.sh traffic_go_around dummy_street_seed_1 go_around 2
wsl -d Ubuntu-22.04 -- bash scratch/linux_run.sh traffic_causes a_left_car_seed_1 causes 1
```

Both gates were RED with output byte-identical to CI run 36276095952 (`scratch/linux_go_around_run{1,2}.log`,
`scratch/linux_causes_run1.log`):
- G1 `((1943v2, 1919v1), 0.18426514, 9334, (true, true), ...)`;
- "an AI car stood 71.0 s in Dynamic".

On Windows, at the same commit, both were green, and two runs gave identical output (`scratch/win_*_run*.log`).

## 2. Why Windows and Linux diverge (acceptance 4: PCTX branch)

The cause is not nondeterministic iteration. Each platform repeats its own run bit for bit across
processes, even though std `HashMap` / `RandomState` (seeded per process) is used in `traffic/junction.rs`,
`drive.rs` and `occupancy/`.

Per-tick state hashes (`scratch/probe_038.rs`) differ from the first dump right after city load. There,
186 bodies differ by 1-2 ULP in `Rotation` (`scratch/probe_{win,linux}_d3.txt`, for example `1818v0` w
`3f362a79` vs `3f362a7b`).

`scratch/libm_probe.rs` ran 1e6 inputs on both platforms. On MSVC UCRT and glibc, `f32::atan2`, `sin`,
`cos`, `atan` and `acos` give different results, while `sqrt` gives the same results
(`scratch/libm_{win,linux}.txt`). Spawn yaw goes through `f32::atan2` (`combat/hitscan.rs:146` `aim_yaw`)
and glam `sin_cos`, so a 1-ULP seed grows into a different 150 s traffic history.

Making the platforms agree bit for bit would need glam/bevy_math `libm` plus replacing every std
transcendental in `gta_sim`. That is out of scope and is written up in `PCTX_PROPOSALS.md` (bevy-ecs lesson).

The platform difference only moved the trajectory onto two latent bugs.

## 3. G1 pass-through: root cause and fix (acceptance 2: done)

**Evidence.** Seed 1, node 140, tick 9334 (`scratch/probe_linux_g1b.txt`, `scratch/decode_pair.py`):
- Car 1943v2 on `Connector(1284)` (lane 458 → 506) and car 1919v1 on `Connector(1293)` (lane 507 → 504)
  both hold grants and are both kinematic.
- Their yaws are 25° and 19° off the lane lines, and their bodies overlap by 0.184 m.

**Root cause.** Before the fix, `traffic/graph.rs:244-257` marks two connectors as conflicting only when
their centre lines come within `2 × half width + conflict_margin` (2.7 m). These two centre lines stay
3.25 m apart, but the corners of a 4.08 m body swinging through a turn reach past that band.

Kinematic × kinematic pairs get no solver response and no switch (the switch reacts only to dynamic
bodies, `contact.rs:179`). This is the TASK-036 item 4 finding (seed 5 in TASK-032).

Scale: on seed 1, 198 of the 2340 connector pairs that the old table let hold grants together touch, by up
to 0.48 m. This happens at ordinary junctions too (nodes 14, 15, 18).

**Fix** (`traffic/graph.rs`):
- Two connectors also conflict when car bodies driven along them touch.
- `body_sweep` places the chassis rectangle (half extents + `conflict_margin / 2`) every
  `BODY_SAMPLE_STEP` (0.2 m). It runs from the nose at the connector start (centre on the source lane) to
  the rear at the connector end (centre on the exit lane).
- `sweeps_touch` prunes by AABB and centre distance, then calls the new `contact::rects_overlap`, a static
  SAT with no allocation.
- `TrafficGraph::new` / `from_layout` now take the car half width and half length (`Vec2`).

Measured on seed 1: conflict entries go from 8928 to 10962 (+23 %), and graph build time from 16 to 54 ms
(dev profile, one-shot at `Loading → Playing`).

## 4. The (a) `Dynamic` stand: root cause (seed 1, node 141)

Sources: `scratch/probe_linux_causes_{dyn,j,j2,hit,walk}.txt` and `scratch/rec_1937.log` (a temporary
eprintln in `recover_dynamic`, reverted). The sequence:

1. Car 1937v0 holds `Connector(1302)` and stops on the crosswalk for a walker. A walker brushes it and it
   turns `Dynamic` (tick 2637).
2. At the same time, car 1988v1 holds `Connector(1305)`. It brakes too late for a walker on its exit
   crosswalk, turns `Dynamic` (tick 2651), gets lost and is abandoned at the start of lane 508.
3. The two bodies leave a 0.6 m gap on the crosswalk. Three walkers stand pinned in it for 60 s.
4. 1937v0 never recovers: its skin corridor always holds a pinned walker, `nobody_coming` is false
   (`recover.rs:145`), and it is `led`, so it never gives up.

This is the node-141 case of TASK-037 plus TASK-036 item 1.

After the fix, 1302 and 1305 conflict: their bodies pass within the 0.3 m margin without touching, so this
pinch no longer forms. `a_left_car_seed_1` is green on Linux with its bound unchanged. The class itself is
not fixed, which is what §6 is about.

## 5. What the fix broke: `rb_a_box_car_seen_from_afar_is_cleared` (both platforms)

With the new table, the rb scene takes another path.
- Grant holder 1922v0 (`Connector(719)`, box 83) leaves the box after the left car is cleared. It stops
  with its rear still in the box (lane 249, s 0.69). That keeps its grant, so 4 waiters of conflicting
  connectors stay blocked.
- It stands `Dynamic` for 98.8 s. Walkers are pinned at its nose on the crosswalk: the diagnostics show
  `coming_clear false, led true` for 6000+ ticks, with blockers 2089v0, 2042v0 and 2118v0
  (`scratch/rec_rb_1922.log`, `scratch/probe_win_rb*.txt`).
- The numbers are nearly identical on Linux and Windows, so this is a plain regression of the new
  trajectory, not a platform effect.

The same class has now shown up in three rows: `(c)` (already scoped, TASK-037), `(a)` on Linux, and `rb`
after this fix. A city-wide 30 s `Dynamic` bound breaks whenever a trajectory change parks a car at a
crosswalk with walkers.

## 6. Decision needed

Any one of these makes the suite green:
- **(A) Scope the `Dynamic` bound of `rb` (and `(a)`) like `(c)`.** The bound would cover only the
  scene's own approaches, the city-wide stand would be printed, and TASK-037's acceptance would gain these
  rows. The spec forbids a relaxed bound, so this needs an explicit yes.
- **(B) The resting-walker rule: a character at rest relative to the car is not a contact.**
  - What changes: the Character switch in `contact.rs` ignores relative speed ≤ `hold_speed`. Recovery
    (`nobody_coming`, `corridor_clear`) ignores such characters too.
  - Experiment results (reverted, `scratch/expB_*`), on Windows: rb, a_left_car_seed_1/7, c, b3,
    traffic_contact 6/6, go_around 10/10, junction_box 5/5 and gridlock 3/3 are all green.
  - What breaks: `spot_c_queue_behind_a_box_car_is_never_given_up` goes `GATE BROKEN`, because its
    pressed dummies no longer switch cars. That fixture needs re-anchoring.
  - It changes the TASK-032 contact contract, so it is full-mode work, not this hotfix.
- **(C) TASK-036 item 1 (walk avoidance) first,** then remeasure rb.

I did not relax any bound or change the contact contract without that decision. Also left untouched:
`box_rules::connector_rects` (the body-vs-connector-path check) still uses centre line ± half width, which
is the same flaw. That is TASK-036 item 4, "conflict-point reservation".

## 7. Gates

- **New:** `traffic_graph::cars_granted_together_never_touch`, seeds 1..=8 (correctness).
  - For every pair of connectors at a node that do not conflict, an independent oracle finds no overlap.
    The oracle takes car corners from `graph.pose` every 0.1 m, uses no margin and citygen's
    `convex_overlap`.
  - Flip-RED: with the body-sweep clause disabled (`|| (false && sweeps_touch(..))`) it goes RED:
    "seed 1: 198 of 2340 pairs granted together touch ... (74, 75, 14, 0.361) ...", see
    `scratch/flip_red_graph_gate.txt`. Restored, it is GREEN.
- **Existing:** `traffic_go_around::dummy_street_seed_1` (G1 oracle, max depth 0.000) and
  `traffic_causes::a_left_car_seed_1` are green on Linux and Windows.

## 8. Files changed (`git diff --stat`)

| File | Lines |
|---|---|
| `crates/gta_sim/src/traffic/graph.rs` | +73 / -8 (`body_sweep`, `sweeps_touch`, `BODY_SAMPLE_STEP`, `new` / `from_layout` take `half: Vec2`) |
| `crates/gta_sim/src/traffic/contact.rs` | +11 (`rects_overlap`) |
| `crates/gta_sim/tests/traffic_graph.rs` | +70 (the oracle gate) |
| `crates/gta_sim/src/traffic/lanes.rs`, `tests/traffic_support/mod.rs`, `src/bench/mod.rs` | constructor argument |
| `docs/architecture/traffic.md` | conflict rule paragraph |
| `assets/traffic/traffic.ron` | `conflict_margin` comment, no value change |

Task dir: `IMPL_SUMMARY.md`, `PCTX_PROPOSALS.md` (two lessons), `log.jsonl` (+5 entries), and `scratch/`
(probes, scripts, logs). Temporary probe tests were removed from `crates/`; their copies are in `scratch/`.

Third-party assets were copied from the main checkout into the worktree's gitignored
`assets/third_party/` so the client gates could run.

## 9. Test results (current tree: the conflict fix only)

| Command | Linux (WSL, 1.95.0) | Windows |
|---|---|---|
| `cargo test --no-fail-fast -p gta_sim -p citygen` | 589 passed, **1 failed** (`rb_a_box_car_seen_from_afar_is_cleared`), 12 ignored, 74 binaries (`scratch/linux_full_suite.log`) | 589 passed, **1 failed** (same row), 12 ignored (`scratch/win_full_suite.log`) |
| the two target gates | green | green |
| `traffic_graph` | green | green |
| `cargo clippy --locked -p gta_sim -p citygen --all-targets -- -D warnings` | n/a | clean |
| `cargo clippy --locked --workspace --all-targets -- -D warnings` | n/a | clean |
| `cargo test -p gta_like --bin gta_like` | n/a | 82 passed (`scratch/win_client_tests.log`) |

Acceptance status:
- Repro: done.
- Root cause and mechanism: done for G1. For (a), explained; its class is §6.
- Both gates green on both platforms: done.
- Full suite green: **no** (rb).
- Determinism: PCTX note.
- CI after merge: pending.

## 10. How to verify

- `cargo test -p gta_sim --test traffic_graph`: the oracle is green. Flip it by the sabotage in §7.
- In WSL, run `bash scratch/linux_test.sh -p gta_sim --test traffic_go_around --test traffic_causes`:
  dummy_street_seed_1 and a_left_car_seed_1 are green, and rb is red with "Dynamic 98.8 s".
- `git apply scratch/expB_resting_walkers.patch` plus the switch hunk in `scratch/expB_contact_full.diff`
  reproduces option B's measurements.
