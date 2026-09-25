# PREMISE_CHALLENGE — TASK-016 (T15 traffic + police cars)

## 1. Counter-example tested

Binding note O2 asserts: "a player in a car can be neither arrested nor wounded, so driving is a safe haven".
Counter-example that would falsify it: in the current code a seated driver already takes damage (e.g. a bullet
ray from a cop hits the player's capsule/head hitbox while seated, or vehicle-impact damage reaches the driver),
OR the arrest/Busted path already fires for a player in a vehicle. If either is true, O2 misstates the problem and
the success predicate "bullet in the cabin wounds the driver" could be met while the real gap lies elsewhere.
Secondary: the task assumes the lane graph, forced-eject feet ray and fire-line car-blocker behaviour exist as
described in the TASK-015 notes.

## 2. Primary-source investigation

Driver invulnerability (O2):
- `crates/gta_sim/src/vehicle/seat.rs:197-210` — on entry the player gets `Driving`, `RigidBodyDisabled`,
  `ColliderDisabled`, and every `HeadHitbox` child also gets `ColliderDisabled`.
- avian3d 0.7.0 `src/collider_tree/update.rs:198,204` — `ColliderDisabled` add/remove removes/re-adds the
  collider in the collider tree that spatial queries use; so hitscan rays (`combat/hitscan.rs:234,262`,
  `cast_ray_predicate`) and melee shape casts (`combat/melee.rs:410-468`) cannot reach a seated driver.
- `crates/gta_sim/src/vehicle/impact.rs:150-160` — a bullet that hits a car (`BulletHitVehicle`) only reduces
  `VehicleHealth`; no path forwards damage to the driver. Grep for `explo|Wreck|burn|VehicleHealth` in
  `gta_sim/src` finds no car-destruction path that hurts the driver.
- `crates/gta_sim/src/police/arrest.rs:33-36` — `arrest_player` queries the player
  `(With<Player>, Without<Dead>, Without<Driving>)`: a driver can never be arrested.

Forced eject (note 3):
- `crates/gta_sim/src/vehicle/seat.rs:73-89` — the normal path filters candidates by
  `(feet.y - level).abs() <= step`; the `None if forced` fallback uses `feet(candidates[0])` (left door ray) with
  no level check. `eject_all` (`seat.rs:228-256`) calls `exit_spot(.., true)`; it is wired to
  `OnEnter(Wasted)` / `OnEnter(Busted)` at `vehicle/mod.rs:243-244`. Matches the note.

Cars in the fire line (O1):
- `crates/gta_sim/src/tactics/fire_line.rs:56-102` — `blocked`/`blockers` and `Blocked { shields, bodies }`
  take point bodies (`&[Vec3]`) only; `grep Vehicle|shields` in `police/behavior.rs`, `police/fsm.rs` finds no
  vehicle in the shield set. Cars are not treated as hold-fire blockers. Matches the note.

Slice premises:
- Lane graph exists: `crates/citygen/src/layout.rs:105-120` (`LaneGraph { lanes, connectors }`, connectors carry
  `intersection`), built in `crates/citygen/src/graphs.rs:83`. No explicit "conflict point" type exists; the
  intersection id on connectors is what reservation can key on. Nothing in `gta_sim` consumes lanes yet
  (grep `lane|Lane` in `gta_sim/src`: no hits) — consistent with T15 being the slice that adds traffic.
- Star table: `docs/design/GDD.md:304-305` — 1 star = 1 car, 2 stars = 2 cars; the "≤ 2 at 2 stars" gate
  matches the table. Spawn/despawn 70/90, 15/25, 2 s rule and cap 24: `GDD.md:246`. Kinematic-to-dynamic
  switch on contact: `GDD.md:243`. Acceptance list equals `GDD.md:659-661` verbatim.

## 3. Did it hold

The counter-example did not hold. A seated driver has every collider (capsule + head hitbox) disabled, car bullet
hits only touch `VehicleHealth`, and arrest explicitly excludes `Driving`. So "in a car = neither wounded nor
arrested" is literally true in the code, and the other two TASK-015 notes (forced eject skips the feet-height
check; fire line ignores cars) describe real code behaviour. The slice goal and gates match the APPROVED GDD.

One observation, not a mis-framing: `GDD.md:312` says a player in a car is not arrested and that being pulled out
is "вне MVP". The O2 note (pull-out arrest at 1 star, cabin-hit wounds) therefore adds scope beyond the GDD. The
task text itself authorises it as a binding orchestrator note under the delegated design authority, so it is a
scope extension to be named in the plan, not a wrong premise. Also: "Q2(b)" in O2 does not refer to GDD Q2
(`GDD.md:278`, gang hostility); it refers to a TASK-015 option, so the planner should not look it up in the GDD.

## 4. Verdict

PREMISE HOLDS — `crates/gta_sim/src/vehicle/seat.rs:197-210` (driver capsule and head get `ColliderDisabled`,
which avian 0.7.0 `collider_tree/update.rs:204` removes from spatial queries), `vehicle/impact.rs:150-160`
(car bullet hits touch only `VehicleHealth`) and `police/arrest.rs:35` (`Without<Driving>`) confirm the stated
safe-haven gap; `seat.rs:84-88` and `tactics/fire_line.rs:56-102` confirm the other two notes; `GDD.md:243-252,
304-305, 657-661` match the slice goal and gates.
