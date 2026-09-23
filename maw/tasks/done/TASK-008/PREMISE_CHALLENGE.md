## Counter-example tested

The existing left-click input path fires the equipped gun, so the proposed three-click runtime check could observe a dummy knockdown without ever exercising the melee combo.

## Primary-source investigation

- `src/input/mod.rs:104` binds left click to `Fire`; `src/input/mod.rs:191-199` turns each press into `ActionIntent.fire_requested`.
- `crates/gta_sim/src/combat/hitscan.rs:150-155` consumes that request but exits when `Loadout.held` is `None`.
- `crates/gta_sim/src/combat/weapons.rs:225-233` defines the held gun as `Option<Weapon>`, and `src/input/mod.rs:204-209` maps key 1 to `WeaponRequest::Unarmed`.
- `crates/gta_sim/src/combat/range.rs:81-95` shows the dummy's existing lethal-hit path: it gains `Dead` and later resets `Health`; that path does not assign a knockdown state.

## Did it hold

No. The current code confirms that left click feeds gun fire when a gun is held, but an unarmed player does not fire a gun. The existing dummy death/reset path does not produce knockdown. These sources do not show that the proposed three-click knockdown observation can be satisfied by firearm hits. The possible ambiguity of a future QA script alone is not positive evidence of a mis-framed premise.

## Verdict

PREMISE HOLDS — `src/input/mod.rs:104,191-209`; `crates/gta_sim/src/combat/hitscan.rs:150-155`; `crates/gta_sim/src/combat/range.rs:81-95` show the tested firearm counter-example does not currently yield an unarmed dummy knockdown.
