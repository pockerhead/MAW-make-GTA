# PREMISE_CHALLENGE — TASK-015 (GDD T14, drivable car)

## 1. Counter-example tested

Written before any investigation:

The task treats GDD §13 T14 as a self-contained slice whose prerequisites already exist in the repo.
Concrete counter-case: **"припаркованные машины из генератора"**: `crates/citygen` produces no
parked-car placements, and the GDD (§12/§13) puts that output in a slice the task does not depend on
and that has not landed yet. If so, the task claims a generator output that does not exist. A second
form of the same case: the pedestrian-damage formula in GDD §5 gives zero damage at 10 m/s
(a threshold above 10 m/s), so the "урон по формуле" gate would pass while collisions deal no damage.

## 2. Primary-source investigation

- `crates/citygen/src/layout.rs:5-19`: `CityLayout` has roads, districts, blocks, lots, buildings,
  sidewalks (`WalkGraph`), lanes (`LaneGraph`, :97), `player_spawn`, landmarks. It has no parked-car
  or vehicle spawn field. `grep -rn -i "parked|parking|vehicle|spawn_point|pickup" crates/citygen/src`
  returned nothing.
- `docs/design/GDD.md:71` (§2, item 7): the generator's output includes "Точки спавна пикапов, NPC и
  припаркованных машин". `GDD.md:248` (§5.2): "Припаркованные машины: точки из генератора вдоль улиц,
  динамические тела в покое". `GDD.md:584-588` (T3 goal): no parked-car points. `GDD.md:650-651`: parked
  cars from the generator are listed in the **T14 goal itself**. No earlier slice owns them. The task
  text copies the T14 goal verbatim, and its gate line already says "`-p citygen` where touched".
- Dependencies: `GDD.md:650` T14 depends on T2, T5, T8, T12, which are TASK-003/006/009/013 by the
  slice->task mapping. `ls maw/tasks/done` shows TASK-003, 006, 009, 013 (and TASK-014, the T13 audio
  that the engine hum builds on) all done.
- Damage formula: `GDD.md:237` says "Пешеход получает урон от относительной скорости удара (формула в
  `damage.ron`)". The GDD gives no threshold. `assets/vehicle/` does not exist (`ls assets`), so the
  formula is this task's data to author. No written threshold makes the 10 m/s case zero.
- Other premises the task takes for granted, checked against the code:
  - No vehicle code exists yet. `ls crates/gta_sim/src src` has no `vehicle/` module; `grep -rli
    "vehicle|sedan"` over `crates/gta_sim/src src assets` finds nothing. The task starts from scratch,
    as framed.
  - Input contexts: `src/input/mod.rs:15,76,101-102` has only `OnFoot`, managed by
    `bevy_enhanced_input`. `InVehicle` is new work, as GDD `:166` says.
  - Collision layers: `crates/gta_sim/src/layers.rs:5-9` has `World, Character, Hitbox`. GDD `:564`
    lists `Vehicle` as a law layer. Adding it falls inside this slice.
  - Tunnelling gate: `crates/gta_sim/src/world/test_area.rs:20` has a 12 x 4 x **0.5 m** wall. At
    28 m/s and 64 Hz the car moves 0.4375 m per tick (GDD `:239`). That is close to the wall's
    thickness, so a gate against it is falsifiable, not trivially green.
  - avian3d 0.7.0 source (`~/.cargo/registry/src/*/avian3d-0.7.0/src/dynamics/ccd/mod.rs:308,389`)
    has `SpeculativeMargin` and `SweptCcd` at the lines the GDD cites. No vehicle/suspension controller
    exists in `src/` (the only "wheel" hits are `joints/revolute.rs` and `interpolation.rs`). This
    matches the premise "пишем сами".
  - The acceptance list matches GDD `:652-654` line for line.

## 3. Did it hold

No. The counter-example does not break the premise. The generator really has no parked-car points
(`layout.rs:5-19`). But the GDD gives that work to T14 itself (`GDD.md:651`), not to a missing
dependency, and every declared dependency is done. The 10 m/s damage case has no GDD threshold that
would force zero damage, because the formula is data this task writes in `damage.ron`.

A side observation that does not change the verdict: the goal says "урон машины ... от относительной
скорости" (the car stalls at zero, `GDD.md:237`). The headless acceptance list (task and GDD `:652`
alike) gates only pedestrian damage. So the car-damage/stall rule has no named headless criterion.
Both documents agree on this, and the goal still requires the rule. The acceptance set is incomplete
for one sub-feature. The problem statement is not mis-framed.

## 4. Verdict

PREMISE HOLDS — `crates/citygen/src/layout.rs:5-19` confirms that no parked-car points exist, but `docs/design/GDD.md:650-651` puts them in T14's own goal. Dependencies TASK-003/006/009/013 are in `maw/tasks/done`. `src/input/mod.rs:15` (only `OnFoot`), `crates/gta_sim/src/layers.rs:5-9` (no `Vehicle` layer) and the absence of any `vehicle/` module match the greenfield framing. The 0.5 m wall at `crates/gta_sim/src/world/test_area.rs:20` makes the tunnelling predicate falsifiable at 0.4375 m/tick.
