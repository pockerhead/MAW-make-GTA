# FIX_SUMMARY — TASK-032, fixer round 1

Inputs: `IMPL_REVIEW.md`, the orchestrator note (G6 → A, R1 → R-A, stop rule → R-B), `OPEN_DECISIONS.md`.
Evidence of this round: `scratch/fixer/` (R-A diff and debug traces), `scratch/runtime_fix/` (R1, t15).

**Preflight claim** (the one that would not work if done verbatim): issue 6's alternative "insert
`moved[e] = tick` at the same time". `junction::update` runs `moved.retain(|e, _| occupants...)`
(`junction.rs:166`) before the whole-box idle check (`:172`), so a `moved` entry with no occupant entry
is dropped before it is read. Done instead: the whole-box grant also gets its occupant entry.

**Headline for the orchestrator:**
- MAJOR 1 (phantom claim after hijack) is fixed and gated. Each of the two defences goes RED alone.
- **R-A (push through the box) failed twice with the same class** (the box stays locked in view). I
  took the stop rule: R-B is built and all R-A code is reverted (diff kept in
  `scratch/fixer/r_a_attempt.diff`). **R1 stays RED** for a car left in the box the player watches from
  within 40 m.
- MAJOR 2 is done: "no `Dynamic` stand > 30 s" is asserted in every city gate of the task through one
  helper.
- **G4 was green only because the body was out of frame** (issue 7 confirmed: the body stood 41 m
  behind the player, so the cheat removed it at 65 s). With the body watched in frame, G4 liveness is
  RED (89-100 s), the same class as R1. G4 now asserts correctness (lease, G1, the `Dynamic` bound, body
  watched). The liveness rows are `#[ignore]`d with the reason.
- t15 runtime: pressure in 6/6 runs. Hijack and "at most 2 active cars" pass in all six.

## 1. Fixed

| Review item | What was done |
|---|---|
| **Issue 1 MAJOR**, phantom pass claim | New `traffic::clear_ai_state` (next, waiting, lateral, manoeuvre, calm, stood, deaf). `abandon()` and `on_hijack` both call it. `snapshot_road` derives claims only for `car.is_ai()`. New G8 row `traffic_occupancy::a_hijacked_passer_claims_nothing`: a car in `Pass { go: false }` is hijacked through the real enter path. Asserts: the taken car has `Manoeuvre::None`; no claim of it is in `RoadOccupancy`; an oncoming car on the opposite lane drives past the claim spot. Second part (named mutation): the taken car's fields are set back to the pass, and it still claims nothing. |
| **Issue 2 MAJOR**, `Dynamic` stand coverage | `StandClock::dynamic_violation()` plus `DYNAMIC_STAND_S = 30` in `traffic_support`. It is collected with the other violations in `traffic_causes` (a, b1-b4, c, d, R1/R-B rows), `traffic_junction_box`, `traffic_gridlock` (new `StandClock`; worst `Dynamic` stand printed: 1.7 / 0.5 / 0.0 / 0.8 s on seeds 1/2/7/42), `police_sirens` (both rows) and `traffic_go_around` (it replaced the inline check). |
| **Issue 3 / orchestrator R-A** | Built, failed twice, reverted. Fallback R-B built (section 3). |
| **Issue 4 MINOR**, "in frame" limited to 90 m | Replaced by R-B. A driverless car in or at a box counts as seen only within `bubble.stuck_in_view_distance` (40 m) with its centre line in the cone. Past that, as the orchestrator's R-B asks, it counts as out of frame. The review's `f32::INFINITY` would have kept it forever. Traffic cars keep the bubble's 90 m. The second half of issue 4 (the player's last hijacked car, behind the camera, within 25 m, removed after 45 s) is left for the owner run, as the review says. |
| **Issue 5 MINOR**, `cannot_stop` 30.0 | `pass.rs`: reach = v0max²/(2·idm.max_deceleration) + idm.min_gap, where v0max is the larger desired speed, with a comment. |
| **Issue 5 MINOR**, `REPICK_WITHIN` | The derivation is a comment, **measured** with a probe (seeds 1 and 7, every lane end): two exits of one lane end lie at most 0.413 / 0.424 m apart 1 m along the connector. That is inside the 0.425 m in-lane slack, so the margin is thin. The reviewer's estimate (0.17 m, r = 3) was too low, and so was my first analytic one (0.21 m). The value stays 1.0. |
| **Issue 6 MINOR**, whole box lapses the next tick | `drive.rs`: when `plan()` returns `whole_box`, the car is also put in `occupants` (and removed from waiters), `moved[e] = tick`, `waiting = None`. |
| **Issue 7 MINOR**, G4 body out of frame | G4 now puts the player on the sidewalk `WATCH_M` = 25 m from the junction nearest the spawn, looking at it (`nearest_box` / `sidewalk_at`, moved to `traffic_support`). It counts every tick the body is out of frame, past `stuck_in_view_distance` or gone, and reports `GATE BROKEN` if that count is non-zero. The busiest-node search is replaced by "the most used connector of that box over 20 s". |
| Nit: `let _ = timeout;` | Replaced by the `GATE BROKEN` watched-body assert. |
| Nit: `stuck.rs` cheap check first | The `road.body(..).standing` check now runs before `in_junction`. |
| Docs: withdraw the "accepted residue" | GDD §5.2, `traffic.md` (box section and stuck cheat) and the `stuck.rs` module doc now describe R-B. A car in a box watched from within 40 m is marked **open (R1)**, not "accepted". `traffic.md` claims paragraph: only AI-driven cars claim road. |
| IMPL_SUMMARY "uncommitted" | Now says: committed as 9b5c045. |

## 2. R-A: two failures, then fallback R-B

**Attempt 1.** Loose (driverless, non-police) bodies were skipped by D1/D2, a car whose strip hit one in or
at the box ignored it in the IDM at `pass.push_speed` 2.5 m/s, and the TTC switch made the pusher Dynamic.
There was no give-up and no `lost` abandon at the box, and Dynamic cars on connectors re-requested their
grants. Headless R1 (the car at the box hub, the player facing it): seeds 1 and 7 both RED, 146 s.
- Cause 1: `follow_speed` from rest opens the throttle to about 33 %, so the car crawls.
- Cause 2: the straight strip still saw the body after it had left the curved connector band.

**Attempt 2.** Push at full `push_speed`; any loose body in the box counts as pushable; no at-start
re-pick for Dynamic cars. Result: seed 7 improved from 138 s to a 72 s worst stand, seed 1 stayed at
138 s. Traces are in `scratch/fixer/r_a_r1_debug_*.txt`. Two causes of the same class:
- Physics: an unmanned sedan holds its wheels below `hold_speed`, and its tyres grip sideways at about
  µ·m·g ≈ 16 kN. The sedan's drive force is m·a ≈ 6.9 kN. The pusher only moves the body by ramming: the
  autopilot gets stuck, reverses, rams again, and reverses into the car behind it.
- Walkers: people on the crosswalk get pinned between the pusher and the left car at gap 0. Both cars
  then wait for them forever. This is the TASK-036 walk-avoidance class, now confirmed.

**Rejected alternative:** a kinematic "bulldozer" pusher. A pinned body would be driven through, which is
the G1 pass-through class by construction.

**R-B** (`traffic/stuck.rs`, data `bubble.stuck_in_view_distance: 40.0` with a strict loader and two
config rows):
- The rule applies to a car nobody drives (a driverless non-police car, or an `Abandoned` traffic car)
  that stands in a junction box or within a car length of it.
- The clock runs while that car is out of frame, **or** farther than 40 m from the player, **or** only
  a corner of it is in frame (its centre line is outside the view cone).
- After `stuck_despawn_seconds` (45 s) the car despawns.
- Validation law: the distance must be less than `in_view.despawn + look_ahead`, the road snapshot radius.
- The "within a car length" margin covers the cars given up at the box entry, where no pass reaches.

New rows in `traffic_causes`:
- `rb_a_box_car_seen_from_nearby_stays`: player 24.8 m away, the car stays.
- `rb_a_box_car_seen_from_afar_is_cleared`: player 59.7 m away, the car is despawned, `Dynamic` bound
  holds, G1 clean. Stands are printed: worst 85 s, from a car given up about 15 m before the box. That
  car is outside R-B's reach and outside the pass's reach, because a pass is refused within about 18 m of
  the stop line (stage 4 step 6 was never built).
- `r1_car_left_in_the_box_seed_{1,7}` (headless R1, player 31 m away): `#[ignore]`d, reason open R1.
  Worst stands 143 s on both seeds (`--include-ignored` prints them).

The "only a corner in frame" branch has no gate of its own. It is a geometry predicate; the look is for
the owner run.

## 3. Skipped / not done, and why

- **R1 criterion: not met.** R-A failed twice (section 2), and R-B does not reach a car watched from
  within 40 m.
- **G4 liveness: not met.** Same class. `seed_{1,7}_box_keeps_moving_liveness` are ignored with the
  reason. G4 was green before only through the cheat (issue 7).
- **Review "missing coverage" items outside the orchestrator list: not built.**
  - Character contact with passers or yielders.
  - The avenue curb-yield look-ahead.
  - The stuck cheat at 100 m in the cone. R-B redefines this: past 40 m the cheat applies by design.
- **Review nits: not changed** (not asked, no bug):
  - `car_dispatch.rs` float round trip.
  - The occupancy ↔ traffic/police module cycle.
  - The `SirenLane` reset at the box edge (owner run).
  - The frame-cost `trace.py` measurement.
- **TASK-036 text: not edited** (task-board edits belong to the orchestrator). Findings for it: walkers
  pinned against cars in the box are confirmed (runtime civilians within 8 m of the left car: 15 / 11 / 14,
  longest 136 s); and a car given up near a lane end cannot be passed (pass refused within about 18 m of
  the stop line).

## 4. Runtime QA (release `--features dev`, BRP, `--settings-id` QA; one run at a time)

The R1 script got an optional leave target (`x,z`). Player at STAND (7.2, -50), 31 m from the junction.

| Run | Car left at | Traffic within 45 m standing > 30 s | Longest | Civilians within 8 m (count / longest s) |
|---|---|---|---|---|
| leave A (target 7.9,-81.0) | (8.2, -82.5) | 2 (one `Abandoned` at the east entry) | 148.8 s | 15 / 27.7 |
| leave B (target 6.3,-84.9) | (5.3, -84.5) | **0** (0 over 20 s) | 15.4 s | 11 / 135.9 |
| leave C (target 5.9,-86.7) | (0.7, -86.7) | 6 (two `Abandoned` at the south entry, 38 m from the player: R-B needs > 40 m) | 143.2 s | 14 / 106.4 |
| control (leave=0) | (87.8, -180.3) | 0 (0 over 20 s) | 12.5 s | 0 |
| extra: B first try, the teleport left the player 58 m away | (8.5, -77.9) | 0 (R-B cleared it) | 24.0 s | 25 / 18.5 |
| extra: C first try, entering failed, **no car left** | none | **9 (35-86 s)** | 85.8 s | 0 |

Stage 7 had 15 / 12 / 11 cars at about 150 s; this round gives 2 / 0 / 6. R1 is still RED.

The last row matters. With no car left and the player at the same spot, a queue on the north approach
stood 86 s. The cause is not identified (screenshots in `runtime_fix/r1_leave_c_no_car/shots`).

t15, seeds 1-3, two runs each (`runtime_fix/t15_s*`): pressure in **6/6** runs (criterion ≥ 5/6), all
PASS, hijack passes, at most 2 active cars.

| Run | Pressure |
|---|---|
| s1 a / b | car at 14.7 / 14.6 s |
| s2 a / b | on foot at 7.3 / 7.3 s |
| s3 a / b | car at 10.5 / 4.2 s |

No game process was left running.

## 5. Flips (each restored, then green)

- **Hijack row:**
  - `clear_ai_state` call removed from `on_hijack`: RED ("the taken car kept Pass {..}").
  - `is_ai()` filter removed from the snapshot: RED ("a car the AI does not drive claims road from its
    fields").
  - Both removed: RED on all four assertions; the oncoming car stops at x 21.0.
- **Dynamic-stand helper:** `recover.give_up_seconds: 1000000.0`. `b3` goes RED with "an AI car stood
  59.8 s in Dynamic (> 30 s)".
- **R-B distance:** set to `INFINITY`, the far row goes RED (the car is still there). Set to `0.0`, the
  near row goes RED (the car is despawned in view at 24.8 m).
- **G4:**
  - D2 demotion disabled: the extra row goes RED (stale holder 14.47 s against a 5 s lease).
  - Player facing away: `GATE BROKEN` (body out of frame for 6400 ticks).
- **G8 junction row, R-A only (evidence, now reverted):** with loose bodies skipped by D1, the original
  row (a driverless car in the box) went RED. Showing this is why R-A needed that row re-anchored. After
  the revert the row is unchanged from before.

## 6. Test results

- `cargo test -p gta_sim -p citygen -j 2` (once, at the end; `scratch/fixer/suite_final.txt`): 74 binaries,
  **586 passed, 0 failed, 11 ignored**. The ignored ones: G6 ×2 (earlier), R1 headless ×2 and G4 liveness ×2
  (this round, reason in the attribute), and the ones that were already there.
- Rows run separately during the round:
  - `traffic_junction_box` 3× (identical numbers: worst stale 5.00 / 5.00 / 0.67 s, G1 max depth 0.000;
    stands 89-100 s printed).
  - `traffic_gridlock` (4 seeds, G1 clean, `Dynamic` worst 1.7 / 0.5 / 0.0 / 0.8 s).
  - `police_sirens`, `traffic_go_around` (10), `traffic_causes` (10 + 2 ignored), `traffic_occupancy` (9),
    `traffic_intersection`, `traffic_hijack`, `config_traffic`.
- `cargo clippy --workspace --all-targets -j 2 -- -D warnings`: clean.
- `cargo test -p gta_like --bin gta_like` 3×: 82 passed each time.
- `python tools/qa/tree_check.py`: tree checks passed.
- The 5 CI workflows run after the merge (not run here).
- File sizes: `drive.rs` 638, `junction.rs` 343, `stuck.rs` 85 (all under 750).
