# TASK-027: Рампы для прыжков на машине

Type: feature
Mode: full
Priority: medium
Branch: feature/stunt-ramps
Domains: bevy-ecs, gates, game-design

## Description
Owner (2026-09-25): "надо будет добавить рампы". GTA-style stunt ramps for the T14 car: the generator places a handful of ramps where a car can build speed and fly (plazas, park edges, wide avenue medians, a few at dead-end streets / next to low buildings), deterministic per seed. Ramps are static colliders on the World layer with a Kenney-style/procedural look consistent with the city. Placement must not block sidewalk graph edges, spawn points, parking spots or future T15 lanes (traffic must not drive onto them — keep them off carriageways or on medians/plazas); the minimap may show them (optional).

Tuning in data (`city.ron` ramps: count per city, length, height/angle, placement rules). citygen golden hashes change (schema bump, geometry-unchanged proof for everything except the ramps).

## Acceptance criteria
- [ ] citygen: ramps generated per seed, property tests over many seeds: never overlapping roads' traffic lanes, sidewalks graph edges, parking spots, buildings, spawn points; reachable approach run-up of ≥ N m (data).
- [ ] Headless physics gate: the T14 sedan hitting a ramp at 20-28 m/s gets airborne (all four wheel rays ungrounded for ≥ X s), lands on its wheels (no rollover) in ≥ K of M approach angles; takes 0 damage on a normal landing (scrape rule); no tunnelling through the ramp.
- [ ] Pedestrians/NPC navigation unaffected (existing population/police gates green).
- [ ] Runtime QA scenario: drive to the nearest ramp in seed 1 and jump; screenshots mid-air.
- [ ] clippy, `cargo test -p gta_sim -p citygen -j 4`, client gates green.

## Dependencies
- blocked by TASK-015

## Status
Parked 2026-09-25: owner "ладно давай пока не надо". Not in the current run order.
