# TASK-032: Встречная полоса — объезд и обгон (трафик и полиция)

Type: feature
Mode: full
Priority: medium
Branch: feature/oncoming-lane
Domains: bevy-ecs, gates, game-design

## Description
Two STOPs in TASK-016 share one missing mechanism: an oncoming-lane reservation honoured by oncoming IDM, junction `room()` and the spawner, so that a car may briefly use the opposite inner lane.
- Traffic go-around of a car stopped in its lane (abandoned car, dismounted police car): today the lane waits until the bubble despawns it.
- Police overtaking / traffic yielding to sirens: today police cars stuck behind queues lose sight and dismount ~100 m away.
Risk (from the TASK-016 fixers): silent kinematic pass-through (kinematic cars' forward casts skip kinematic traffic), estimate 150-200 sim lines + gates.

Run only if the TASK-031 playtest rates gridlock behind stopped cars or police chases as major; otherwise stays parked.

## Acceptance criteria
- [ ] Oncoming-lane reservation with a gate proving no kinematic pass-through (independent rectangle overlap check) over many seeds.
- [ ] Go-around gate: 8 cars behind an abandoned car in view all pass it within T s, 0 collisions with oncoming traffic.
- [ ] Police overtake / yield gate: a police car behind a traffic queue closes to ≤ X m of a fleeing player in ≥ K/M seeds.
- [ ] Traffic bench within budget; existing traffic/police gates green.

## Dependencies
- blocked by TASK-031

## Redesign direction (orchestrator, 2026-09-25, after the owner's "рескоуп и редизайн, а не стоп")
Do NOT build a general oncoming-lane reservation first. Redesign around two ideas:
1. **One shared obstacle/occupancy rule.** T15's seam bugs all came from each system having its own notion of "what blocks": fire line (characters only), NPC walk avoidance (walls only), junction room (own agents only), sight (World only), kinematic casts (skip kinematic traffic). Introduce one spatial occupancy truth (bodies = characters + vehicles, static + dynamic + kinematic) with one query API, and migrate these consumers to it. Universal rule instead of special cases (systemic design: Harvey Smith / Deus Ex "global patterns, not special use-cases").
2. **Police as a drama system, not a traffic participant.** The GTA fantasy is an aggressive chase, not a lawful traffic sim: sirens have priority — AI traffic ahead of a siren within N m pulls to the curb side of its own lane and stops (a kinematic-safe move, no oncoming reservation); police cars may use any lane while sirens are on (they are dynamic and collide physically); police spawn ahead / beside the fleeing player's heading as well as behind (a deliberate cheat, tuned in data). Traffic go-around of an abandoned car then reuses "pull to curb + pass" with the shared occupancy rule.
Acceptance adds: the chase metric from t15 over seeds 1-3 (pressure rate ≥ X/Y), plus the shared-occupancy migration gates (each consumer sees cars and characters).
- From TASK-017 bench: the bench car sits in traffic queues ~45 % of the measured window (no go-around/overtake) — evidence for the redesign.
- From TASK-033 (known residue): the junction lease lapses only on the source lane before the stop line; a holder standing ON its connector or with its nose past the stop line (e.g. behind a player-parked car in the box, outside the AI occupancy) keeps its grant indefinitely — the shared occupancy rule should cover it.
