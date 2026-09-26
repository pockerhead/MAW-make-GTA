# PCTX proposals (TASK-035, implementer)

## 2026-09-26 — game-design risk lesson: NPC lethality knob

Tuning NPC time-to-kill by aim error (accuracy) is a trap here: the hold-fire line check widens by the same
cone (`FireLine::of`: aim error + weapon spread, police overshoot 60 m), so a 9 deg police cone makes any
body within ~7 m of the line at 30 m block the cop, and shotgun blasts with head pellets still kill in one
tick at 12 deg. Lethality is `damage_scale` (escalation.ron per star, gangs.ron), gated by
`tests/lethality.rs` (median TTK over 7 seeds). Trigger: a change to `aim_error_deg` meant to change TTK.

## 2026-09-26 — gates risk lesson: car-pedestrian damage sums over contacts

`apply_impacts` charges every `CollisionStart`; a car that rolls on into a knocked-down body hits it again
(traffic car at 16 m/s: contacts at 15.4, 8.5, 5.2 m/s). A "one hit does not kill" gate must run the whole
roll-out (not one formula row), and lowering `per_mps` alone does not converge; `shove_scale` >= 0.8 throws
the body clear. Trigger: edits to `damage.ron` `pedestrian`.

## 2026-09-26 (fixer) — gates risk lesson: lethality knobs scope by victim, and hits floor at 1 hp

A per-shooter damage multiplier leaks into every victim: `BulletHitVehicle` (car stall 6.7x slower) and NPC
strays. Scale only damage whose resolved body (`ColliderOf.body`) is the player; the car and NPCs take full
damage (`lethality::police_damage_scale_applies_only_to_the_player`, `vehicle_hits::a_patrol_bullet_dents_...`).
`roll_damage` floors a hit at 1 hp, so a tiny `damage_scale` does not flip a TTK ceiling gate (SMG SWAT still kill
in 10 s): flip it with the fire rate too. In car-contact gates the player's velocity in the tick before the
`VehicleHit` is already the contact push; the approach speed is the sample one tick earlier.
Trigger: `DamageScale`, `player_share`, TTK gates in `tests/lethality.rs`.
