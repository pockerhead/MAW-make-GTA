# TASK-035: Lethality balance: 1★ police and traffic hits

Type: bugfix
Mode: small-fix
Priority: medium
Branch: bugfix/lethality-balance
Domains: bevy-ecs, gates, game-design

## Description
TASK-031 playtest M3: the player dies in seconds and never reaches 3★ by play. Two 1★ patrol cops kill a standing player in ~3 s (25-27 per pistol hit), a gang trio in 3.4 s point-blank, and a traffic car once killed the player from full health (`assets/vehicle/damage.ron` pedestrian: 10 m/s → 84, so ~11.3 m/s kills). Most of 22 deaths came 3-14 s after the first shot. GTA chaos lasts longer.

Decisions (orchestrator, binding for this task):
1. GDD §7 1★ row says "пытаются арестовать, стреляют только если игрок атакует". Read "атакует" as "attacks police": at 1★ cops open fire only after the player damages or shoots at a police officer; a player who shot a civilian gets arrest attempts, not gunfire. From 2★ fire on sight as before. Amend the GDD row with this reading.
2. Time-to-kill targets, as data (police/gang accuracy or damage per star in their existing RON files): a standing unarmoured player at 100 HP survives at least 8 s against 2 patrol cops firing (1★ after attacking police, and 2★), and at least 6 s against 3 gang members at 10 m. Pick the knob (accuracy spread vs damage) yourself and justify it in IMPL_SUMMARY.
3. A traffic-car hit on a pedestrian player from full health does not kill at the traffic cruise speed; it still hurts and knocks down. Deaths from cars stay possible at high speed (the player's own car, police rams).

Cost of error: tuning the owner feels on first play → data + headless TTK gates, no new machinery.

## Acceptance criteria
- [ ] 1★ rule: headless gate — player shoots a civilian near 2 patrol cops → 0 police shots at the player during the 1★ window while arrest attempts proceed; player shoots a cop → police fire. Flip RED against the current rule.
- [ ] TTK gates (headless, several seeds): 2 patrol vs standing player ≥ 8 s at 1★-after-attack and 2★; 3 gang at 10 m ≥ 6 s. Each reports the measured median TTK.
- [ ] Traffic-car hit at the max traffic cruise speed leaves a 100 HP player alive (gate); tuning lives in RON.
- [ ] GDD §7 1★ row amended; no new tuning consts.
- [ ] Existing tests pass (t11/t12/t15 scenarios still show police pressure).

## Dependencies
- prefer after TASK-034 — one pipeline at a time; no code overlap
