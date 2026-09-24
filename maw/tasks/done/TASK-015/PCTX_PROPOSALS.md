# PCTX proposals — TASK-015 (planner)

## 2026-09-24 — bevy-ecs risk lesson: TnuaToggle stops the motor loop for everyone after it
`bevy-tnua-avian3d` 0.12.1 `apply_motors_system` (`vendor/bevy-tnua-avian3d-0.12.1/src/lib.rs:419-431`, same in the
registry copy) does `return` (not `continue`) at the first entity with `TnuaToggle::Disabled | SenseOnly`: every
character iterated after it gets no motor force that tick. `population::corpse_components` already inserts
`TnuaToggle::Disabled`. Proposed rule: never use `TnuaToggle` to park a live character (disable the body with
`RigidBodyDisabled` instead); a follow-up task should patch the vendored crate (`continue`) with a gate "a live
character spawned after a corpse still floats". Trigger: token `TnuaToggle`.

## 2026-09-24 — bevy-ecs risk lesson: avian impact speed needs pre-step velocities
Probe `maw/tasks/in_progress/TASK-015/scratch/probe_collision` (avian3d 0.7.0): `CollisionStart` is written in the
same step as the velocity jump; the post-step closing speed is ~0. Impact damage must use velocities recorded in
`FixedPostUpdate` before `PhysicsSystems::First` (after Tnua motors, which write `LinearVelocity` in `FixedUpdate`)
and the manifold normal (world space, collider1 → collider2). Trigger: tokens `CollisionStart` / `CollisionEventsEnabled`.

## 2026-09-24 — bevy-ecs fact: `ColliderDisabled` is per entity
avian3d 0.7.0 `ColliderDisabled` "only applies to the entity it is attached to, not its children"
(`collision/collider/mod.rs:380-394`): disabling a character's body leaves its head hitbox sensor hittable. Trigger:
token `ColliderDisabled`.

## 2026-09-24 (plan-reviewer-1) — gates lesson: "A iterated before B" is table creation order, not spawn order
bevy_ecs 0.19.1 `QueryState` pushes matched tables/archetypes in creation order (`query/state.rs:649-658`) and
iterates them in that order. A gate whose sabotage depends on one entity being iterated before another (the Tnua
`return` bug: a corpse must come before the walker) is vacuous when the walker's table predates the corpse's table
(a second civilian spawned "after" the corpse joins the OLD civilian table and is iterated first). Pattern: move the
probe entity into a fresh table after the precondition exists (insert a test-only marker) and assert the iteration
order in the test (`GATE BROKEN` otherwise). Supersedes the planner's proposal above ("never use TnuaToggle") once
the vendored `continue` fix lands. Trigger: tokens `TnuaToggle` / `iter_mut()` order in a gate's flip.

## 2026-09-24 (plan-reviewer-2) — gates lesson: a latch flip needs a weapon that reads the latch
`fire_weapons` (`combat/hitscan.rs`) fires a `SemiAutomatic` gun only on `fire_requested`; `fire_held` is read only by
`Automatic` guns. A gate that "latches fire_held" with a pistol and flips by removing a reset stays GREEN by
construction. Pick the weapon whose fire mode reads the latch under test (SMG for `fire_held`). Trigger: tokens
`fire_held` / `fire_requested` in a gate's flip.

## 2026-09-24 (implementer) — gates lesson: query iteration order is not creation order
bevy_ecs 0.19.1 `QueryState::update_archetypes` (`query/state.rs:575-600`): with required components, new
archetypes are matched through `component_index().get(..).keys()` (a HashMap), so a fresh `world.query` that sees
many archetypes at once iterates them in hash order. A system's long-lived state appends tables in the order it
first sees them (generation by generation). An order-dependent gate (TASK-015 `tnua_motor.rs`: corpse before the
walker) builds its precondition from a `QueryState` created before the fixture changes, never from a fresh
`world.query`. Trigger: a gate that asserts iteration order (`iter_mut`/`iter` + `position`).

## 2026-09-24 (implementer) — gates lesson: a rolling car re-hits a knocked-down body
A knocked-down character keeps its upright capsule and Tnua brakes it at ~45 m/s² (zero walk basis), so a car still
rolling catches it again 10-20 ticks later: a per-impact damage formula must be gated on the first hit, not on total
health loss (TASK-015 G4: 6 m/s gave 36 + 12). Trigger: tokens `CollisionStart` + `Health::take`.

## 2026-09-25 (fixer) — bevy-ecs / gates: a raycast car's chassis box within the speculative margin of a convex-hull curb
Owner report: at 20+ m/s the car hit a 0.15 m city curb, lost 470..890 hp and bounced back. Headless repro: the
chassis box bottom sat 0.09 m above the curb top; avian's speculative contact between the box's front-bottom edge and
the curb hull edge has a diagonal normal and stops the car before any wheel ray reaches the curb (a cuboid test slab
did NOT reproduce it; the city uses `Collider::convex_hull` blocks). Rule: a vehicle collider keeps its underside
clear of the tallest step the wheels must climb by more than the per-tick travel margin (here 0.29 m over the curb),
and a curb gate uses the city's collider type (convex hull), not a cuboid stand-in. Trigger: `Collider::` on a
`Vehicle` bundle, `curb_height` in `city.ron`.

## 2026-09-25 (fixer) — game-design / bevy-ecs: sight through cars
Sight rays that include `GameLayer::Vehicle` must skip a car that contains either end of the line: the seated driver's
eye and a car's own centre are inside a car box (a literal Vehicle mask made cops blind to drivers and killed the Car
threat). Movement tests (direct seek, avoidance probes, spawn cover) stay World-only (`wall_blocked`): with cars in
`dest_clear` a t11 cop stalled 11 s in Respond behind a parked car. Trigger: `SpatialQueryFilter::from_mask` with
`GameLayer::Vehicle` outside `vehicle/`.

## 2026-09-25 (qa) — gates lesson: a new enum variant breaks older runtime scenarios that index it
TASK-015 added `SoundClass::Engine` (COUNT 8 → 9). `tools/qa/scenarios/t13.py` keeps its own `CLASSES` list and cap
list by index, so `check_peaks` crashed with `IndexError` on the 9-long `SoundStats.peak_alive`; every headless test and
t14.py stayed green, only the t13 regression run caught it. Rule: when a stage changes a reflected enum or a
fixed-size stats array (`SoundStats`, `SoundClass::COUNT`, marker kinds), grep `tools/qa/scenarios/` for the variant
names and rerun those scenarios. Trigger: `COUNT` const or a new variant in a type read by a `tools/qa` scenario.
