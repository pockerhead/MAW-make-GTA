# Premise challenge — TASK-009

## Counter-example tested

If gunfire in the existing game is only a client-side audiovisual effect, with no headless simulation signal for a shot, then the required headless “shot at 20 m → Flee/Cower” criterion could pass through a test-only stimulus while real gunfire leaves civilians unchanged.

## Primary-source investigation

- `crates/gta_sim/src/combat/hitscan.rs:55-62` defines `ShotFired` as a simulation `Message` with shooter and muzzle position. The production `fire_weapons` system writes it after a valid trigger pull at `crates/gta_sim/src/combat/hitscan.rs:155-225`.
- `crates/gta_sim/src/combat/mod.rs:44-49,80-104` registers that message and schedules `fire_weapons` in `FixedUpdate`. `crates/gta_sim/src/lib.rs:83-91` adds `CombatPlugin` to the shared simulation composition.
- `crates/gta_sim/tests/common/mod.rs:36-52` constructs the headless app with `MinimalPlugins` and that same composition. `crates/gta_sim/tests/shooting.rs:63-68,192-208` requests a real shot through `ActionIntent`, advances one tick, and asserts one `ShotFired` message.
- Ran `cargo test -p gta_sim --test shooting shotgun_fires_ten_pellets -- --exact`. Output: `test shotgun_fires_ten_pellets ... ok`; `1 passed; 0 failed`.

## Did it hold

No. Gunfire already produces a headless `ShotFired` message through the production combat system, and the executable headless test observes it. The tested counter-example provides no evidence that the T8 premise is mis-framed.

## Verdict

PREMISE HOLDS — `crates/gta_sim/src/combat/hitscan.rs:221-225`, `crates/gta_sim/src/combat/mod.rs:80-104`, and `cargo test -p gta_sim --test shooting shotgun_fires_ten_pellets -- --exact` (`1 passed; 0 failed`).
