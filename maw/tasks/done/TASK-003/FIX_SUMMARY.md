# FIX_SUMMARY — TASK-003 (fixer, attempt 1)

## Preflight

- Scratch read as a coverage map: `flip_red.py` / `flip_red_results.json` (18 plan rows), `qa_t1`, `qa_t2`
  (BRP summaries + screenshots), `layout_dump`, `pr2` probes, `probe_rng`.
- Review read: `IMPL_REVIEW.md` (3 issues + 2 missing-coverage notes).
- Claim that would do the most damage if applied verbatim: issue 1, "generate a ring mesh between curb and
  inner with a separate interior ground treatment". I checked `src/visuals/city.rs:95-105` and GDD §2.2 /
  §13 T2-T3. The diagnosis holds: lot interiors are drawn sidewalk grey. But GDD §2.2 only says "отступ
  на проезжую часть и тротуар" and does not define lot-interior surfaces. T2 asks for "простых мешей
  (цвет района)", and T3 is where "тротуары с бордюром" get done. Doing the prescription means adding a
  new colour to `render.ron` and changing a presentation choice the plan made on purpose (PLAN_FINAL
  Step 10). So it is not a correctness defect. See Skipped.

## 1. Fixed

- **Issue 2 (Major): the building collider gate accepted any `RigidBody`.** The diagnosis was right: the
  query was `(With<CityBuilding>, With<Collider>, With<RigidBody>)`. I also folded in the review's
  missing-coverage note (the gate never checked collider size or rotation), since it lives in the same
  gate and a wrong value there fails silently. Now `crates/gta_sim/tests/city.rs`
  `one_static_collider_per_building` matches each `layout.buildings` entry to a collider by its centre
  `(c.x, h/2, c.y)` and asserts:
  - `RigidBody::is_static()`
  - the collider cuboid `half_extents` equal `(hx, h/2, hz)` from the layout
  - `rotation * X == (axis.x, 0, axis.y)`. This formula is independent of the spawn's `atan2`.

  It still asserts count equality and exactly one `CityGround`. APIs checked: avian3d 0.7.0
  `RigidBody::is_static` (`dynamics/rigid_body/mod.rs:313`, derives `Copy, Debug`),
  `Collider::shape` (`collider/parry/mod.rs:536`), `Collider::cuboid` = half lengths
  (`:747`), parry3d 0.27.0 `dyn Shape::as_cuboid` (`shape/shape.rs:457`), `Cuboid::half_extents`.
  Flip-RED, each sabotage made in `crates/gta_sim/src/world/city.rs` `spawn_buildings`, then restored
  (checked with `cmp`):
  - `RigidBody::Static` → `Kinematic`: RED, "building at [-551.0142, 6.2, -505.2213] is Kinematic".
  - `RigidBody::Static` → `Dynamic`: RED at the centre match (the body moves under physics).
  - `atan2(-u.y, u.x)` → `atan2(u.y, u.x)`: RED, "local X maps to [-0.99995, 0, -0.01022], axis
    [-0.99995, 0.01022]".
  - collider `cuboid(size.z, size.y, size.x)`: RED, "collider half extents [12.93, 6.2, 8.51], expected
    [8.51, 6.2, 12.93]".
  - After restore: GREEN.
- **Issue 3 (Minor): `different_seeds_differ` could never fail.** Confirmed: `layout_hash` writes
  `layout.seed` right after the schema version (`crates/citygen/src/hash.rs:75`), so the hash differs
  for any two seeds even when the geometry is identical. In `crates/citygen/tests/golden.rs` I added a
  `geometry_hash` helper: `layout_hash` of a clone with `seed: 0`. `CityLayout` is `Clone` and `seed`
  is `pub`. The test now compares those hashes for seeds 1/2/42. Flip-RED: in `rng::stream` I replaced
  `seed.to_le_bytes()` with `0u64.to_le_bytes()` and got RED (`left != right` failed,
  11558222660673636802 on both sides). After restoring the file (checked by reading it back): GREEN.
  - `tools/qa/scenarios/t2.py:99-100`: left unchanged. At runtime it asserts each hash equals its golden
    value (`CitySeed == seed` too). The golden values for seeds 1 and 2 come from layouts that the fixed
    citygen gate now proves are different geometry, so the runtime check carries the property through
    the golden link. The inequality line there stays as a cheap sanity check.

## 2. Skipped

- **Issue 1 (sidewalk mesh fills the whole block).** Skipped as a presentation choice, not a defect.
  Reasons: the plan specified it (Step 10); the GDD doesn't define lot-interior surfaces; it's
  owner-visible on the first frame, and by project law that kind of issue is gated by the owner's run,
  not by rework here; T3 "тротуары с бордюром" redoes sidewalks anyway. I looked at `scratch/qa_t2/
  spawn_1.png`: grey paving up to the building faces reads as a plaza/sidewalk and doesn't hide streets.
  **Owner note:** lot interiors use the sidewalk colour. If that bothers you, the fix is a curb→inner
  ring mesh plus a lot-ground colour in `render.ron` (T3 or a feel round).
- **Missing coverage: client mesh creation is outside the startup budget.** Agreed with the reviewer
  that no gate is warranted. It's under the loading screen, and the owner's run covers it.

## 3. Test results

- `cargo test -j 4 -p citygen -p gta_sim`: all green. citygen lib 7, golden 3 (+1 ignored), perf 0 (+1
  ignored), properties 9; gta_sim lib 1, city 5 (+1 ignored), config 4, jump 3, movement 4, terrain 2.
- `cargo clippy -j 4 --workspace --all-targets -- -D warnings`: Finished, no warnings.
- `cargo build -j 4`: green. The first try failed with "failed to remove target\debug\gta_like.exe
  (os error 5)". No gta_like process was running, so it was a transient file lock (the exe was
  timestamped 11:23, rebuilt by some other process). The retry was green.
- `python tools/qa/tree_check.py`: "tree checks passed" (no bevy_render in gta_sim).
- Formatting: `rustfmt --edition 2024` on the two edited files only.
- The windowed client was not launched. No client or runtime code changed, only two test files.
- `git status --short`: `M crates/citygen/tests/golden.rs`, `M crates/gta_sim/tests/city.rs`, and this
  file. Sabotaged sources `crates/citygen/src/rng.rs` and `crates/gta_sim/src/world/city.rs` were
  restored byte-identical.

children: 0 launched / 0 reported.
