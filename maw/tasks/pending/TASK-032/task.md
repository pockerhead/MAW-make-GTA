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
