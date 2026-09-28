# PREMISE_CHALLENGE — TASK-039

## 1. Counter-example tested

Written before any investigation:

Class D (`traffic_causes::r1_car_left_in_the_box_seed_1`): car 1853v0 stands 113 s in `Dynamic`
0.06-0.26 m from a car at rest. If the reason it never leaves `Dynamic` is a concrete defect in the
EXISTING recover / give-up path (for example recovery gated so that it can never fire while the obstacle
is inside `recover.skin`, or suppressed in frame), and not the absence of a wait-for relation, then the
premise "there is no general progress guarantee, so each residue needs a universal wait-for + relaxed
pass" is mis-framed for at least class D: the stand is a bug in a rule that already claims to cover it,
and the new rows could go green through the new pass while the recover defect keeps producing
`Dynamic` stands elsewhere (e.g. against a body that is not "stationary" by the detector's definition).

(investigation below)

## 2. Primary-source investigation

- `crates/gta_sim/src/traffic/recover.rs` (whole file, 281 lines) — the only exit from `Dynamic`
  besides the stuck cheat:
  - `recover_dynamic` (`recover.rs:~205-250`) returns `Recover` only when `corridor_clear` holds for
    `recover.seconds`. `corridor_clear` samples the rejoin corridor from the CURRENT pose (`k = 0`,
    `centre = point + right * lateral`) and fails if any resting vehicle is inside `rest_skin`
    (`resting_in(b, &tight, horizon)`), where `rest_skin >= switch.skin` (0.1, `traffic.ron:41`) up to
    `recover.skin` 0.4 (`traffic.ron:57`). A car standing 0.06-0.26 m from a car at rest fails the very
    first sample, every tick.
  - `GiveUp` needs `may_give_up = at_rest && off_lane(..) && !led(..)`; `led` is true when any body is on
    the lane line within `2 * idm.min_gap` of the nose. The module doc says this is deliberate: "a car in
    a queue or behind a standing body waits (a given-up car there is one more standing body...)".
  - So the existing rule has no defect that misfires: it is DESIGNED to wait indefinitely behind a
    standing body, with no escalation. That is the premise's "no general progress guarantee", not a
    counter-example to it.
- `crates/gta_sim/src/traffic/stuck.rs:37-60` — the stuck cheat skips a driverless car in/near a box when
  it is within `bubble.stuck_in_view_distance` (40 m, `traffic.ron:38`) and its middle is in frame, and
  skips any in-frame traffic car inside `bubble.in_view.despawn`. Confirms the premise's "the only
  universal escape is forbidden in frame near the player".
- `crates/gta_sim/src/traffic/box_rules.rs:46-63` (`connector_clear`) skips `BodyKind::Character`, and
  `pass.rs:27-39` (`passable`) handles `Vehicle`/`Traffic`(dynamic)/`Character` only on lanes: in-box
  and cross-kind cases are indeed per-geometry, as the premise states.
- Executed the raw class D row:
  `cargo test -p gta_sim --test traffic_causes r1_car_left_in_the_box_seed_1 -- --ignored --nocapture`
  Output (verbatim, trimmed): `seed 1: box 83 at (-2.6, -79.9), the player 31.0 m from it; stands > 30 s
  within 45 m: [(143.25, ...(-0.64,-69.55)), (139.91, ...), (135.11, ...), (113.03, ...(-0.59,-75.64))];
  ... worst dynamic 113.0 s; left car now at (-2.53, -79.90); G1 max depth 0.000` then panic
  `"an AI car stood 113.0 s in Dynamic (> 30 s)", "traffic stood > 30 s near the box: [...]"`.
  The residue is real on HEAD (Windows), the left car is still in the box after the run (player 31 m <
  40 m, so the cheat never fires), and the queue behind the `Dynamic` car stands 135-143 s.

Side finding (not a premise break, a constraint the planner must see):
`crates/gta_sim/tests/traffic_causes.rs:38` `PRESSED_S = 20` and `:176-189, :211-215` — row `b3_dummy_pressed_at_the_bumper`
asserts that a bumped `Dynamic` car does NOT leave `Dynamic` while a character dummy stands pressed at its
bumper for 20 s ("left Dynamic at ... while a body stood in its way"). The new stationary-wait trigger
(D4: progress must also end a `Dynamic` stand; D3: characters are relaxable blockers) with a threshold
T < 20 s would turn this existing green row RED. T (D7) therefore has a floor near 20 s from b3 and a
ceiling under 30 s from the acceptance bound minus the manoeuvre time, or b3 must be re-anchored
explicitly. This narrows the design space; it does not show the problem or the root cause is mis-framed.

## 3. Did it hold

No. The counter-example assumed a defect in the existing recover path; the primary source shows the
recover path is deliberately "wait behind a standing body forever" (`recover.rs` doc + `led` gate) and
the stuck cheat is deliberately off in frame (`stuck.rs`). The only escalation missing is exactly the one
the premise names. The executed class D row reproduces the 113 s `Dynamic` stand on HEAD with the left
car in frame at 31 m. The premise's problem, assumed root cause (no general progress rule; per-geometry
rules plus an out-of-frame-only cheat) and success predicate (stand bounds + third-body oracle +
un-ignored rows) are consistent with the code.

## 4. Verdict

PREMISE HOLDS — `recover.rs` `recover_dynamic`/`corridor_clear`/`led` make a `Dynamic` car behind a resting body wait with no escalation by design, `stuck.rs:37-60` disables the only universal escape in frame near the player, and the executed `r1_car_left_in_the_box_seed_1 --ignored` run reproduces "an AI car stood 113.0 s in Dynamic" with the left car still in the box 31 m from the player; I found no defect in an existing rule that would make the new universal rule unnecessary or mis-aimed (note for the planner: `traffic_causes.rs:38,176-215` row b3 bounds T from below at about 20 s).
