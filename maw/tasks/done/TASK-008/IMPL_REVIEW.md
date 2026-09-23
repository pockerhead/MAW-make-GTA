# Implementation review — TASK-008

## Verdict

**PASS** — the T7 melee slice follows the approved plan, satisfies the headless and client gates, and has no confirmed blocking defect.

## Disconfirmation tested first

Counterexample: an LMB press with a gun equipped also starts or queues a melee swing. It did **not** hold. `tick_loadouts` runs before melee, `swing_melee` skips an equipped gun without consuming `fire_requested`, and `fire_weapons` consumes it afterward (`crates/gta_sim/src/combat/mod.rs:84-95`, `melee.rs:404-417`, `hitscan.rs:155-167`).

## Confirmed correct

- Melee damage, hit windows, stagger, knockdown, and knockback are owned by the headless sim. The `melee.ron` loader validates ranges and unknown fields (`crates/gta_sim/src/combat/melee.rs:17-190`, `crates/gta_sim/src/lib.rs:53-60`). No new production tuning constant was introduced.
- The recorded ordering dead end is resolved in code: `recover_from_hits → swing_melee → apply_strikes` is chained before Tnua controls, and the integration gate asserts the first hit at T0+8 (`crates/gta_sim/src/combat/mod.rs:84-95`, `crates/gta_sim/tests/melee.rs:143-169`). The flip-RED log records a test failure when `apply_strikes` is moved ahead of the sweep.
- The hit-stop changes only animator pause state on `Time<Real>`; the client gate confirms the attacker and target pause, a bystander does not, and `Time<Virtual>` plus fixed ticks continue (`src/juice/hit_stop.rs:26-52`, `src/visuals/character.rs:408-425`, `src/visuals/character_gate.rs:608-659`). Camera shake is applied after the aim ray is written (`src/camera/mod.rs:153-155`).
- The runtime scenario checks the three-click damage total, knockdown and recovery, bat pickup and toggle, and bat damage (`tools/qa/scenarios/t7.py:100-211`). Its saved run reports 100 → 60 HP for the fist combo, 100 → 75 HP for the bat, and no game-log errors (`scratch/qa/t7/summary.json`). Screenshots and feel remain for owner inspection.
- Fresh local commands passed: `cargo build`, `cargo clippy -- -D warnings`, `cargo test -p gta_sim` (101 passed), and `cargo test -p gta_like --bin gta_like` (32 passed). `cargo tree -p gta_sim -e normal -i bevy_render` printed no reverse dependencies.

## Issues

None confirmed.

## Missing coverage

- The owner-run feel and visual checklist belongs in the later `QA_REPORT.md`: punch impact, 50 ms freeze, shake comfort, stand-up pose, and bat grip need the owner's running-game judgment. The implementation summary contains this checklist; it has not yet been recorded in a QA report.
- There is no focused same-tick regression gate for a gun-equipped LMB press staying exclusively with gunfire. The code path was checked directly above; this is a useful future gate if input or system ordering changes.

No sub-agents were used (children: 0 launched / 0 reported).
