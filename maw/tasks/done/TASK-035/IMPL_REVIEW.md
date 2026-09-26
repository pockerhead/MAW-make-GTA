# TASK-035 IMPL_REVIEW (code-reviewer, claude opus)

## 1. Verdict

**NEEDS_WORK**: the 1★ rule, the TTK gates and the traffic gate work and are flip-proven. But both knobs are global, not scoped to the player as victim, and each one changes a mechanic the summary does not mention: police gunfire now barely damages the player's car, and the player's car no longer kills pedestrians at city speeds.

Commit reviewed: `106382e` (the diff `main...HEAD`, 21 files). I read every changed src/asset/doc file in full diff and all of `tests/lethality.rs`.

### Disconfirmation (done first)
Counter-example I wrote down: "NPCs only aim at the player" (IMPL_SUMMARY §2) is false. If gangs fight gangs or police, `DamageScale` stretches those fights 3-6x and the T9/T11 oracles move.
- Result: **held for body damage, failed for vehicles.**
  - `assets/gang/gangs.ron:8-14`: gang-gang and gang-police are `hostile: false`, so gangs only take provocations from the player (`gang/behavior.rs:75,88`). Police `Shooter`s target only `live_player` (`police/behavior.rs:193-203`). So NPC-on-NPC body damage happens only through stray hits and gang crossfire.
  - But cops DO shoot at a player who is driving (`player` query carries `Option<&Driving>`, the target is the player chest). Those pellets stop on the car body and go out as `BulletHitVehicle.damage`, which is now scaled (`combat/hitscan.rs:329`). `apply_bullet_hits` (`vehicle/impact.rs:175`) subtracts it from `VehicleHealth`, and at 0 the car stalls (`vehicle/chassis.rs:114`). See issue M1.

### Commands run (by me, this tree)
- `cargo test -p gta_sim -p citygen --no-fail-fast`: 66 result lines, 527 passed, 0 failed.
- `cargo test -p gta_sim --test lethality --test traffic_pedestrian --test police_arrest --test vehicle_hits -- --nocapture`: all green. Printed medians: 1★ hostile 11.17 s, 2★ 11.17 s (per-seed numbers are identical), gang 0 14.00 s, gang 1 8.41 s (min 6.39), traffic `16 m/s lane: hit speeds [15.39], health 19`. They match the summary.
- `cargo clippy --locked --workspace --all-targets -- -D warnings`: clean.
- The flip logs in `scratch/flip/` match the claimed RED results (old alert rule, no near miss, no cop_hit, old damage.ron, HEAD TTK baseline).

## 2. Confirmed correct
- **1★ rule** (`police/behavior.rs:39-72`): `PoliceAlert` is set only by a player `DamageDealt` on a cop, or by a player `BulletTrace` within `arrest.near_miss_distance` of a live cop. Witnessed attacks on others no longer count. Every reader is drained (`.count()`, not `.any()`). `to` is the hit point for body hits and full range for misses (`hitscan.rs:295-316`), so the segment test is correct. From 2★ `next_state` is unchanged.
- The gates for the 1★ rule are each flip-RED: `one_star_cops_arrest_a_civilian_shooter` (old rule gives RED at tick 0), near miss, and a separate melee gate for the `cop_hit` path. The summary explains correctly why the gun-hit gate cannot catch a missing `cop_hit` clause.
- **DamageScale** (`combat/hitscan.rs:115-124, 215`): `#[require]` on `PoliceUnit`/`GangMember` (`police/mod.rs:449`, `gang/mod.rs:400`). The FSMs write it before any trigger pull, and only when the value changes (`police/behavior.rs:259-263`, `gang/behavior.rs:277-279`), so there are no archetype moves. The player has no component and uses 1.0. It is reflected and registered.
- **Data first**: `damage_scale` per star row, `combat.damage_scale`, `near_miss_distance`, `per_mps`, `shove_scale` are all in RON with `positive(..)` validation and new sabotage rows. There are no new tuning `const`s in src. The test consts (`POLICE_TTK`, `GANG_TTK`, `TTK_CAP`) restate the spec.
- **GDD**: the 1★ row is amended in §6.4, table "Эскалация" (`docs/design/GDD.md:310`), and a sentence on the knob was added (`:318`). The spec says "§7", but the row lives in §6.4. That is correct.
- Traffic gate `a_cruising_car_does_not_kill_a_full_health_player`: real traffic car, precondition asserts the hit speed is ≥ v0-1, flip RED on the old damage.ron. The analysis of the re-hit into the knocked-down body (per_mps alone fails, shove ≥ 0.8 fixes it) is backed by `scratch/traffic_hit_probe.md`.
- The regexes in the BRP scenarios still match the reordered rows (`t11.py:56`, `t16.py:93`). t9/t11/t12/t15 oracles do not depend on NPC damage: t11 gives the player armour, t15 "pressure" is distance/state, and the t9 "crossfire killed a member" check only gets more lenient.
- The unused `witnesses` re-export was removed with its only caller (`wanted/mod.rs:8`). That is cleanup of debris this change created, which is fine.
- File sizes are all under 750 lines.

## 3. Issues

### M1 (major): `DamageScale` scales car-body damage, so police gunfire almost no longer stalls the player's car
`crates/gta_sim/src/combat/hitscan.rs:215,329` together with `vehicle/impact.rs:175`, `vehicle/chassis.rs:114`.
The car has 1000 HP (`damage.ron:8`). Before the change: 40 patrol pistol hits (25 dmg) or 84 SWAT SMG hits (12 dmg) stalled it. Now it takes 267 hits at 1-2★ (3.75 dmg) and 278 at 5★ (3.6 dmg). Stopping a fleeing car by gunfire is a police pressure lever, and t15 already records about 2/5 escapes. The summary says the shooter-scaled damage "matters only for stray hits". For cars that is wrong, and there is no gate or measurement.
Orchestrator question, "scale only when the victim is the player?": **yes, scope by victim.** Evidence: with the shipped faction matrix, body damage on NPCs is strays and crossfire only, so victim scoping changes nothing measurable there. It removes the car side effect. It also stops a later data-only change (`hostile: true` for gang-gang or gang-police in gangs.ron) from silently stretching those fights 3-6x without any gate noticing. The task's targets are all defined with the player as victim.
Suggested fix: in `fire_weapons`, apply `scale` to a body hit only when the target has `Player`. For `BulletHitVehicle`, keep the unscaled damage for `VehicleHealth` and carry the scale (or a scaled `driver_damage`) so that `apply_bullet_hits` scales only the cabin wound when the seated driver is the player. The alternative is to keep it as it is and get an explicit owner/orchestrator sign-off, with the car-stall numbers above in the GDD sentence.

### M2 (major): `pedestrian.per_mps` 12 → 6.5 is global, so the player's car stops killing NPCs at city speeds
`assets/vehicle/damage.ron:19`, used by `vehicle/impact.rs:16-24` for every victim.
All characters have 100 HP (`character/health.ron:2`). One hit now kills only at ≥ 18.4 m/s closing speed (before: 11.3 m/s). With `shove_scale` 1.0 the body is thrown clear, so no re-hit adds damage. A civilian or gang member the player drives into at 15 m/s (54 km/h) takes 78 damage and gets up. Running people over is a core GTA-like verb, and it also feeds the run-over murder crimes and the witness/corpse flow (TASK-026). The summary lists the stronger knockback as owner feel, but not this lethality drop. Decision 3 of the spec is about the player as victim ("Deaths from cars stay possible ... the player's own car").
Suggested fix: scope by victim like M1, for example a `pedestrian.player_share` (or a separate player row) in damage.ron, and restore `per_mps` 12 for NPC victims. The alternative is an explicit owner sign-off with the 18.4 m/s kill threshold stated.

### m1 (minor): during an arrest, any player shot counts as "shooting at police"
`police/behavior.rs:57-66`. The segment starts at the muzzle, which is the player position plus rotated `(0.25, 0.35, -0.45)` (`combat/aim.ron`). An arresting cop stands at `stand_distance` 1.0 m. Even a shot aimed straight away from the cop starts about 1.49 m from the cop's centre, which is ≤ 1.5. So at arrest range every shot fires the alert, including a shot at a civilian. This is no worse than the old rule and is defensible (a gun fired next to a cop). But the new GDD text "за стрельбу по мирным арест, не огонь" does not hold at arrest range. Either state it in the GDD row, or skip the first `arrest.distance` metres of the segment.

### m2 (minor): upper bound of lethality not measured
The orchestrator asked about this. The gates are floors only: a `damage_scale` of 0.01 passes everything. My analytic estimate, from the measured 2-cop median of 11.17 s: the full 2★ row of 4 cops ≈ 5.6 s; 3★ (6 cops × 0.2) ≈ 2.8 s; 5★ 12 SWAT SMG × 0.3 is much faster. All are well under 30 s for a standing player, but none is measured. The sentence "roughly 5 s against 4 cops" in the summary is also an estimate, not a run. Fix: add a ceiling assert to the existing gates (for example 2-cop median ≤ 20 s), and optionally one full-row 2★ run (4 units).

### m3 (minor): thin margin on the traffic gate; moving toward the car kills
`pedestrian_damage` adds the pedestrian's velocity along the normal (`impact.rs:20`). Standing: 81 dmg, 19 HP left. Walking head-on (1.8 m/s): about 96. Running head-on (4.5 m/s): about 114, which kills. Crossing the street perpendicular is fine. This is an acceptable reading of the spec ("a hit at cruise speed"), but mark it for the owner run.

## 4. Missing coverage
- The 1★ rule with a civilian standing next to a cop (orchestrator question). Current behaviour, from the code: a bullet that stops on a civilian within 1.5 m of a cop's centre, or passes within 1.5 m of it, counts as an attack on police and the cops fire. A civilian 2+ m in front of the cop on the line does not count, because the trace ends at the civilian. The gate puts the civilian behind the player, so none of this is exercised. That is acceptable, but a case with the civilian at about 1 m from a cop would pin the documented reading.
- The shot-during-arrest case (m1).
- A ceiling / full-row TTK (m2).
- M1: police bullets vs the player's car health (if kept as it is, a gate that pins how many hits stall the car).
- M2: a run-over of an NPC at 15 m/s kills (after a victim-scoped fix).
- The 1★ and 2★ TTK gates give identical per-seed results: same scene, both rows 0.15, only the 2 manually spawned units. The 2★ gate therefore does not add a distinct case. It is still worth keeping as a row guard (per TASK-011: one case per row).

## 5. Nits
- `police/behavior.rs:57`: `cops.iter()` inside the per-trace filter is O(traces × cops). This is fine at N ≤ 12 × 10 pellets.
- `vehicle/impact.rs` unit test `pedestrian()` keeps the old 12/0.6 values. It is a pure-formula fixture, so this is fine, but it no longer documents the shipped curve.
- m15 anti-pattern pass: no unwrap/clone/unsafe issues in the diff.

children: 0 launched / 0 reported
