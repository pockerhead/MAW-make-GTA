# TASK-038: Main CI red after TASK-032: two traffic gates fail on Linux

Type: bugfix
Mode: small-fix
Priority: high
Branch: bugfix/linux-traffic-gates
Domains: bevy-ecs, gates

## Description
The TASK-032 merge (main 7779605) left "sim gates" red on the Linux CI runner (run 36276095952), while the full suite was green on Windows locally (589 passed):
- `traffic_causes::a_left_car_seed_1` panics at traffic_causes.rs:127: a car stood 71.0 s in `Dynamic` (bound 30 s), near (395.9, 486.4).
- `traffic_go_around::dummy_street_seed_1` panics at traffic_go_around.rs:195: G1 oracle, two bodies interpenetrate 0.184 m (entities 1943v2 / 1919v1, flags (true, true), around (365.9, 366.0)-(369.5, 366.3), tick 9334). That is the pass-through class, which is a real correctness bug if kinematic × kinematic.

The simulation diverges between platforms: float, iteration order or hashing. Reproduce on Linux locally (Docker Desktop and WSL Ubuntu 22.04 are available, and CI uses toolchain 1.95.0 on ubuntu), find the root cause, and fix the mechanism rather than the number. Record why Windows and Linux diverge (e.g. HashMap iteration order, parallel query order, libm differences). If the divergence source is nondeterministic iteration, make it deterministic.

Cost of error: a pass-through or a stuck-car bug that only shows on some machines, the "silent for a month" class. Needs a full evidence layer.

## Acceptance criteria
- [ ] Both gates reproduced RED on Linux locally, with the command recorded.
- [ ] Root cause named with file:line; the fix is in the mechanism, not a relaxed bound.
- [ ] Both gates green on Linux (local container) and on Windows. The full `cargo test -p gta_sim -p citygen` is green on both.
- [ ] If the root cause is a nondeterminism source, a gate or assertion makes the two platforms agree, or a determinism note is written in the bevy-ecs domain as a PCTX proposal.
- [ ] After merge, main CI shows all 5 workflows success.
- [ ] Existing tests pass.
