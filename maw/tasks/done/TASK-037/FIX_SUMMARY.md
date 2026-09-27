# FIX_SUMMARY — TASK-037, fixer round 1

Verdict: every review item the orchestrator assigned is fixed and gated. `traffic_go_around::avenue_seed_2`
is green on Windows and Linux. R1 seed 7 and the G4 extra row also turned green on both platforms, each
held by a mechanism flip, so they are un-ignored. Still red and ignored for TASK-039: G4 seed 7 liveness
(class E, 61.3 s) and R1 seed 1 (class D, 113 s).

## 0. Preflight

- `scratch/` was read as a coverage map: the stage0-5 logs, `flip.py` with `flips_stage{1..4}.json`, the
  `wsl/` scripts, and the reviewer's replica `cr/around_cars_dither.py`. I re-ran the replica: 1133 and
  1122 target changes in 20 s, and the walker never arrives, as the review says.
- I read `IMPL_REVIEW.md`. The orchestrator note and the last `OPEN_DECISIONS.md` entry are binding.
- **The review claim most likely to break correct code if applied verbatim:** M2's fix, "in `sense`,
  skip `OnPathTraffic` only if the body is not an ungranted car standing on a connector", applied to
  every plain car, lane strips included. The risk: a car queued behind the held car, or a car at its
  stop line whose straight strip crosses the box, gets a standing traffic hit. It brakes for it and
  plans a pass around a queue member.
- **Checked, and the risk is not real.** `pass::passable` returns `false` for
  `BodyKind::OnPathTraffic` (`pass.rs:37`), so no pass is planned on such a hit. The braking is also
  what the same cars got before TASK-037, when they stood `Dynamic` (kind `Traffic`, never skipped).
  So the exception applies in both arms: the lane strip and the connector sweep.

## 1. Fixed

| Review item | What was done | Gate (class) | Flip -> RED |
|---|---|---|---|
| **C1** / orchestrator (1): avenue seed 2 contact | Heading cap on lane manoeuvres, detailed below. | `traffic_go_around::a_pass_keeps_the_body_in_its_band` (correctness, floor): curb false/true, widest away side 1.615 m (limit 1.635), pass side 4.950 m (claim 4.950); unit `lateral::yaw_cap_rows` | `f1_no_heading_cap`: away side 1.759 m, pass side 5.501 m |
| **M1** / orchestrator (2): `around_cars` dither | `around_cars` scores a corner by the shortest way round the car, detailed below. | `fire_line::around_cars_near_diagonal_target_arrives` (correctness): 0.6 m past corner A it picks B; walker and cop (both callers use 0.3 / 0.8) arrive in < 8 s with <= 3 target changes | `f2_straight_corner_metric` (old metric) |
| **M2** / orchestrator (3): held car invisible to crossing cars | New `drive::held_in_box` (the old inline `held` predicate, now shared). A per-tick `held_cars` set goes to `sense`, and plain cars do not skip `OnPathTraffic` bodies in that set. | `traffic_intersection::a_crossing_car_stops_for_a_car_held_in_the_box` (correctness, `plus()`, detailed below): rests 2.04 m of travel short, G1 clean | `f3_held_car_skipped`: 19 kinematic interpenetration ticks, depth 0.07 m |
| **m1** / orchestrator (4) | The prescribed test failed; recomputed as a sweep, detailed below. | unit `recover::resting_in_rows` | `f4_rest_skin_closing_ignored` (zero sweep) |
| **m2** / orchestrator (4) | Connector `on_path` also needs `|(position - point)·tangent| <= conflict_margin / 2`. | `f_connector_car_out_of_the_table_band_stays_dynamic`, second case: on the line, 0.5 m back from s 0.2, so 0.3 m behind the start (along 0.300, reach 1.200) | `f5_no_along_rule`: kinematic at 1.47 s |
| **m3** / orchestrator (4) | Ignore reasons and docs point to TASK-039, detailed below. | — | — |
| **m5** (trivial) | `rest_skin(.., slides)`: no heading-swing term on a connector, where the rejoin slides. | No dedicated gate; stage-4 flips re-run below | — |
| Orchestrator (5) | Created `maw/tasks/pending/TASK-039/task.md`, detailed below. | — | — |
| Nits | Reflowed the long doc lines (`recover.rs` module doc, the rb doc in `traffic_causes.rs`, `traffic.md`). | — | — |

**C1 details.** The cap is `lateral::manoeuvre_band` plus `lateral::yaw_cap`, applied in
`drive.rs` on lanes only.
- The band is ±pitch/2. A pass widens its side to `|offset| + half.x + pass.clearance`, a yield to
  `|offset| + half.x`.
- The cap is `asin(room / |half|) − atan2(hx, hz)`.
- The room is taken at the worse of two offsets: the current one, and the look-ahead (the lateral stepped
  towards the target over `|yaw error| / yaw_rate`).
- **Deviation from the review:** it used the look-ahead only. That relaxes the band edge the car moves
  away from while its yaw is still growing.
- The connector rejoin keeps its slide. Only the desired heading is clamped; the lateral law is
  unchanged.

**M1 details.**
- A corner that sees the target scores `|from - c| + |c - to|`.
- Otherwise it scores `|from - c|` plus the way round via the adjacent corner that sees `to`, or via the
  far corner.
- The reached radius is kept.
- The rule is stateless and stable: `scratch/fixer/around_cars_geodesic.py` shows 1 and 2 target changes
  (arrives in 6.1 s and 5.2 s), against 1133 and 1122 for the old metric.

**M2 gate details.** The scene is on `plus()`:
- The north left turn is pre-granted at `S0`.
- A kinematic car sits on the east straight at the first spot (s 0.8) whose body lies on the turn's
  sweep and more than 0.5 m clear of the turning car. It is ungranted and on its line.

**m1 details.** The prescribed test `(b.v − v)·(centre − rect.centre) <= 0` turned
`g_car_at_rest_inside_the_skin_lets_it_recover` RED.
- The switched car's own velocity noise (about 1e-7 m/s) gives a dot of +1e-14 on 184 of 709 samples.
- The sleeping parked car then drops out of the rest skin and `calm` resets. This was probed with a
  temporary eprintln, since removed.
- **Recomputed:** in `corridor_clear` each resting vehicle is swept by its own velocity over
  `recover.horizon_seconds` against the rest-skin rectangle (`resting_in`). This is continuous: noise
  moves it by micrometres, and a body creeping in at 0.4 m/s closes 0.2 m.
- `nobody_coming` already swept relative motion.
- The car's own velocity is left out: after recovery it moves by the rejoin law, which the rest skin
  already counts.
- The module doc is corrected.

**m3 details.**
- Reasons: `traffic_junction_box.rs` (seed 7) and `traffic_causes.rs` (R1 seed 1).
- Module docs: `traffic_junction_box.rs`, `traffic_causes.rs`.
- `traffic.md` "Junction box": Open (TASK-039) and Open (TASK-036 item 4).

**TASK-039 details.**
- Mode full, priority medium, branch `feature/box-uturn`, domains bevy-ecs, gates, game-design.
- Blocked by TASK-037; prefer after TASK-036.
- Content: class E (G4 seed 7) with the U-turn / reverse-out fallback kept out of the random exit
  choice, the class D residue on R1 seed 1, and evidence paths.
- The extra row and R1 seed 7 are recorded as resolved here (next paragraph).

**Rows un-ignored beyond the note.** R1 seed 7 and the G4 extra row were ignored when the note was
written, while they were red. Now:
- R1 seed 7: green on Windows (worst stand 23.6 s, Dynamic 6.3 s) and on Linux (20.2 s, Dynamic 0.0 s),
  G1 0.
  - It is held by M1: `f10_straight_corner_metric_r1_seed_7` is RED, with stands of 105.8 s.
  - Not by the cap or M2: `f8` and `f6` stay GREEN, and so does `f11` (m5).
- G4 extra row: green on both platforms (worst stand 30.8 s < 40), G1 0.
  - It is held by M2: `f7_held_car_skipped_extra` is RED, 55.2 s. `f9` (no cap) stays GREEN.
- Each platform repeats itself bit for bit (the TASK-038 lesson), so one run per platform is the
  evidence.
- The orchestrator may re-ignore them. The reasons are in `log.jsonl`.

Docs: `docs/architecture/traffic.md` covers:
- Modes: the rest-skin sweep, the along bound, and the lane heading cap.
- One tick item 5: the held-car exception.
- Junction box: the shortest-way-round corner rule and the TASK-039 / TASK-036 open items.

Owner-visible changes (owner run, not gated):
- A pass or yield turns its nose at most about 12.9 deg while on the lane line. It was 31-39 deg, so it
  now crabs more at low speed.
- A car stops for a car held in the box.
- Walkers and cops round a car without turning back.

## 2. Skipped

- **m4** (a lane or siren rejoin that reaches a connector at speed crabs): not a correctness issue per the
  review. It stays marked for the owner run.
- **Nits:**
  - `rejoin()` still returns a 5-tuple. It is internal and read twice.
  - `sweep_ahead` / `first_in` are not hoisted: the benches did not move (traffic 1.48 ms vs 1.48).
  - `traffic_intersection.rs` drift bound `0.707 * 1.625` is not re-derived: it is the implementer's
    stage-2 row, and hard-coding does not change its verdict.
- **The m1 missing-coverage row** (a (g) scene with a car creeping at 0.4 m/s): not built as a city or
  floor scene. The predicate is gated by the unit row instead. A creeping dynamic car at rest speed is
  hard to hold still in a floor fixture without new machinery.
- **The M2 gate geometry** "its corner overlaps Y's swept body but not Y's `connector_rects`": the grant
  is pre-given instead (`granted_on`). The property under test is the sensing, not how the grant was
  obtained.
- **m5** has no dedicated gate. It only relaxes the skin on connectors. The stage-4 rest-skin flips were
  re-run against the fixed code and all stay RED:
  - `s4_A`: g car row.
  - `s4_B`: g walker row.
  - `s4_C`: e row.
  - These re-runs overwrote the implementer's `scratch/flips/s4_*.txt`.

## 3. Test results

- Windows, `cargo test -j 2 -p gta_sim -p citygen --no-fail-fast`: **605 passed, 0 failed, 11 ignored**
  (`scratch/fixer/full_suite.txt`, run before the two un-ignores).
  - It includes `traffic_go_around` 11/11 (avenue seed 2 green) and the police gates.
- Windows city gates with `--nocapture --include-ignored` (`scratch/fixer/city_gates_windows{,_2}.txt`):
  - `traffic_gridlock` 5/5: seeds 1/2/7/42 worst 31.0 / 25.9 / 17.6 / 23.5 s, all within 40 s, G1 0.000.
  - `traffic_junction_box`: 5 pass. Only `seed_7_box_keeps_moving_liveness` fails (61.3 s, ignored,
    TASK-039).
  - `traffic_causes`: 12 pass. Only `r1_car_left_in_the_box_seed_1` fails (113.0 s Dynamic, ignored,
    TASK-039).
  - (c) and rb pass with the city-wide Dynamic bound.
- Linux, WSL Ubuntu-22.04, rustc 1.95.0, CI env (`scratch/fixer/linux_run.sh`, logs in
  `scratch/fixer/wsl/`): all green.
  - `traffic_go_around` 11/11 (avenue seed 2 ok).
  - `traffic_recovery` 10/10.
  - `traffic_intersection` 7/7.
  - `traffic_pedestrian` 5/5.
  - `traffic_gridlock` 5/5.
  - `traffic_junction_box` 4 passed, 2 ignored.
  - `traffic_causes` 11 passed, 2 ignored.
  - `witness_city` 1 passed.
  - G1 max depth 0.000 everywhere.
  - Ignored rows on Linux: R1 seed 7 ok, extra row ok, G4 seed 7 red at 61.3 s, R1 seed 1 red.
- After the un-ignores, those two rows ran green on both platforms. The code did not change after those
  runs.
- Benches, release (`scratch/fixer/benches.txt`):
  - traffic 1.48 ms (limit 19);
  - police 1.48 ms (limit 11);
  - civilian 1.43 ms (limit 8).
- `cargo clippy -j 2 --workspace --all-targets -- -D warnings`: clean, after the last edit.
- `cargo test -j 2 -p gta_like --bin gta_like`: 82 passed.
- `python tools/qa/tree_check.py`: passed.
- Flip runs: specs are in `scratch/flips_fixer.json`, run with `scratch/flip.py`; outputs are in
  `scratch/flips/f*.txt`.
- File sizes, all under 750:
  - `traffic_intersection.rs` 743;
  - `traffic_recovery.rs` 721;
  - `drive.rs` 662.
- `git status`: the source and test files above, `docs/architecture/traffic.md`, the task dir, and the new
  `maw/tasks/pending/TASK-039/`. `metrics.md` was already modified before this round.

children: 0 launched / 0 reported
