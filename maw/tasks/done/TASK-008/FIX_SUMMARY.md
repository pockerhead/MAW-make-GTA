# FIX_SUMMARY — TASK-008 (fixer a1, on QA_REPORT.prev-1.md)

Input: `IMPL_REVIEW.md` is PASS with no issues. The orchestrator pointed the fixer at the QA findings
(`QA_REPORT.prev-1.md`: Bug 1 and a Low item) and the QA probe `scratch/qa_a1/qa_a1_probe.rs`.
Scratch was read first, as a coverage map only. My own evidence is in `scratch/fixer_a1/`.

## Preflight: the claim that would break code if applied verbatim

QA fix option 1 says to use "`cast_shape_predicate` rejecting a distance-0 hit whose `point1` lies behind the origin".
In avian3d 0.7.0 the predicate is `&dyn Fn(Entity) -> bool` (`spatial_query/system_param.rs:531`). It runs before the
narrow-phase cast (`:555`), so it never sees `distance` or `point1`. Taken literally, it can only drop an entity from
the whole sweep, including when that entity is really hit ahead. A "behind" test alone also leaves the side-wall case
open, because a flush side wall sits at 90°, where the sign is float noise (measured below). Option 3 (sweep radius
≤ capsule radius) would override the GDD §4.2 tuning value `cast_radius: 0.35`. The QA diagnosis is right. I recomputed
the prescription.

## Fixed

### Bug 1 (medium-high): a body touching the attacker's side or back takes or blocks the punch
Verified real. `swing_melee` used `cast_shape` with `ShapeCastConfig::from_max_distance`, where
`ignore_origin_penetration` is false (`shape_caster.rs:458-465`). The r 0.35 sphere starts on the axis of a
r 0.3 capsule, so every collider within 0.05 m of the capsule is returned at distance 0, whatever its direction.
I added gates first. On the unfixed code, 3 of them were RED (see Test results).

Fix in `crates/gta_sim/src/combat/melee.rs`:
- `cast_shape` became `shape_hits_callback` (same shape, origin, mask filter and reach). It keeps the nearest hit
  that passes a new `swing_reaches(hit, dir)`: either the swing met the body along the way (`distance > 0`), or the
  sphere overlaps it at the start and the swing drives into it (`normal1 · dir < -INTO_TOLERANCE`). `normal1` is the
  outward normal of the hit collider, so for a body in front it points against the swing. This is parry's own
  "separating at time 0" rule (`shape_cast_support_map_support_map.rs:40`, `normal_vel >= 0` → discard) plus a
  float tolerance. It matches what the sweep already does for a body just outside the sphere.
- `const INTO_TOLERANCE: f32 = 1e-3` is a numeric law, not a tuning value. Measured start-overlap dot products for
  flush side walls are in the ±1e-6 range (probe output below), so 1e-3 (≈0.06°) is three orders above the noise.
  The tolerance does real work. With `< 0.0`, flush walls at 37° and 83° yaw ate the punch
  (`scratch/fixer_a1/probe_rotated_no_tolerance.log`: 6 of 30 cases fail; `probe_rotated_tolerance.log`: 0 fail).
- Why I rejected `ignore_origin_penetration: true`: parry discards only `normal_vel >= 0`, so the same noise keeps
  some side walls (see above), and `cast_shape` returns only the closest hit, so a kept side wall still hides the
  target. The explicit filter over all hits handles both problems.
- Hit order is unchanged for every existing case: a hit at a positive distance is always accepted, and the nearest
  accepted hit wins, as `cast_shape` did.

New gates in `crates/gta_sim/tests/melee.rs` (production `headless_app()`; `setup` now delegates to `setup_with`,
which spawns extra bodies before the settling ticks):
- `wall_behind_does_not_block_the_punch`: wall face 0 / 0.02 / 0.04 m behind the capsule; the dummy 1 m ahead is hit
  exactly once, at T0+8.
- `wall_alongside_does_not_block_the_punch`: the player walks sideways into a wall until it stops them
  (precondition asserted: gap < 0.05 m, "GATE BROKEN" otherwise), then punches a dummy 1 m ahead: hit at T0+8.
- `rotated_wall_alongside_does_not_block_the_punch`: a flush wall along the swing at yaw 37° and 83° (the noise
  angles from the probe): hit at T0+8.
- `bystander_behind_is_not_punched`: a dummy pressed against the player's back (precondition: < 0.65 m, inside
  the sphere). The only `DamageDealt` is `(T0+8, dummy ahead)`, and the bystander stays `Steady`.
- `body_touching_the_front_is_punched`: a dummy touching the front (precondition: < 0.65 m, a start overlap) is
  hit at T0+8. This keeps "genuine contact in front is still hit".
- Class: all five are correctness gates.

Flip-RED (each one on the fixed code, restored by copy, sha256 `6d11d530…1d78` identical before and after;
log `scratch/fixer_a1/flip_red.log`):
1. `swing_reaches` → `true` (the old behaviour: take any hit) → `wall_behind…`, `wall_alongside…`,
   `bystander_behind…` RED. On the unfixed commit the same three were RED (bystander took `(8, bystander)`,
   walls `[]`); `rotated…` was added later and is covered by flip 3.
2. `swing_reaches` → `hit.distance > 0.0` (ignore every start overlap) → `body_touching_the_front_is_punched` RED.
3. `INTO_TOLERANCE` → 0 (`dot < 0.0`) → `rotated_wall_alongside_does_not_block_the_punch` RED ("wall at 37 deg").
All GREEN after restore.

Owner note: the sphere (0.35) is wider than the body (0.3). If you stand flush against a wall and aim even slightly
INTO it, the punch is still spent on the wall. The sweep does the same to a wall a few cm away, so the behaviour is
consistent. It is feel, so it goes on the owner checklist, not into a gate.

### Low: shake noise time on f32 `elapsed_secs()`
Verified real (`src/juice/shake.rs` `shake_camera`). f32 elapsed time loses sub-frame resolution after many hours.
Changed to `real.elapsed_secs_wrapped() * cfg.noise_hz` (bevy_time 0.19.1 `time.rs:329`, 1 h wrap, `:207`).
Trade-off: one noise discontinuity per hour, visible only when trauma > 0 at that frame. It is cosmetic. I added no
gate, because the evidence weight rule applies (owner-visible and harmless). I did no flip for the same reason.

## Skipped
- IMPL_REVIEW.md: no issues listed, so nothing to act on.
- QA "Notes, not bugs" (one buffered click when mashing, subtle shake by data, the victim slides during hit-stop):
  QA confirms these are by design or owner feel. I changed nothing.

## Test results (fresh, after the fixes, one cargo command at a time)
- `cargo build` → Finished (`scratch/fixer_a1/build.log`).
- `cargo clippy --workspace --all-targets -- -D warnings` → Finished, 0 warnings (`clippy.log`).
- `cargo test -p gta_sim -j 4` → all green: lib 19, anim_state 4, asset_manifest 3, city 6 (+1 ignored), config 19,
  health 6, jump 3, **melee 19** (14 + 5 new), movement 4, respawn 5, shooting 16, terrain 2 (`test_gta_sim.log`).
  The melee binary was re-run 10 times: 10/10 green.
- `cargo test -p gta_like --bin gta_like -j 4` → 32 passed (`test_gta_like.log`).
- Runtime: `python tools/qa/scenarios/t7.py --out scratch/fixer_a1/t7` → exit 0. Combo 100 → 60, `KnockedDown`,
  `Steady` again after 0.80 s; bat 100 → 75, knocked down after 0.23 s; `log_errors` empty. I looked at the
  screenshots: `combo_2.png` shows "10"/"20" over the middle dummy with the player mid-strike, and
  `bat_knockdown.png` shows "25" with the player in a swing pose. No game process left running (`tasklist`).
- Temporary probe `crates/gta_sim/tests/fixer_probe.rs` was removed. `git status --short`: only
  `crates/gta_sim/src/combat/melee.rs`, `crates/gta_sim/tests/melee.rs`, `src/juice/shake.rs`, and this file.
  File sizes: `melee.rs` 714 lines, `tests/melee.rs` 646 lines (both < 750).

## Owner checklist addition (Russian)
- [ ] Встать вплотную к стене боком или спиной, ударить манекен перед собой: удар попадает. Если целиться чуть-чуть в стену, удар уходит в стену. Это ожидаемо при сфере 0.35 м, но проверь, не раздражает ли.

Log: one `decision` entry appended. children: 0 launched / 0 reported.
