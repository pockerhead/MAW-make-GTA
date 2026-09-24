# TASK-026: Свидетели убийства реально звонят — розыск за убийство на улице

Type: fix
Mode: small-fix
Priority: high
Branch: fix/witness-calls
Domains: bevy-ecs, gates, game-design

## Description
Owner (2026-09-24): "убийство гражданского розыск не вешает — так и должно быть?" Orchestrator analysis of the shipped data (`assets/npc/civilian.ron`, `perception.ron`, `crates/gta_sim/src/civilian/reaction.rs::choose_reaction`):
- gunshots closer than `report_min_distance` 25 m are never phoned in (they flee/cower); only civilians 25-40 m away can call;
- Report wins only when `report·t.report > flee·t.flee` → with weights 0.8 vs 1.0 and temperament spread ±0.5, P ≈ 31 %;
- corpse sight ≤ 20 m, but everyone nearby already fled from the shot and a fleeing civilian does not re-evaluate a corpse.
Result: killing a civilian on a street with people around rarely raises wanted — reads as a bug. GTA-like target: a kill in view of people almost always gets a star within seconds.

Design (orchestrator decision, owner delegated):
1. **Flee, then call.** A civilian who fled/cowered from a gunshot, fight or corpse that is reportable (a player crime), after reaching `flee_distance` (or `cower_seconds` elapsing) and still alive/unfrightened, starts a `Report` about that same cause with probability `call_after_flee` (data, e.g. 0.6). Keep the one-heat-per-incident accounting (TASK-011).
2. Raise `reaction.report` so a witness in the 25-40 m band chooses Report roughly half the time (derive the weight from the temperament distribution; show the math).
3. Keep all existing wanted gates green (incident dedupe, body-call attribution, no-witness = 0).

## Acceptance criteria
- [ ] Headless gate on the seed-1 city with the production population: the player kills a civilian on a street with ≥ 3 other civilians within 40 m; over 20 seeds of the NPC RNG (or 20 placements) wanted reaches ≥ 1 star within 15 s in ≥ 17/20 runs; report the distribution (time to star). Flip-RED with call_after_flee = 0 and old weights.
- [ ] Gate: a kill with NO civilian within hearing/sight and no cop → 0 heat (unchanged rule).
- [ ] Gate: one corpse → heat counted once even with several delayed calls.
- [ ] Runtime: t10/t11 still pass; a runtime probe (kill on a busy street, no cops) reports time-to-star.
- [ ] clippy -D warnings, `cargo test -p gta_sim -p citygen -j 4`, `cargo test -p gta_like --bin gta_like` green.

## Dependencies
- blocked by TASK-015
