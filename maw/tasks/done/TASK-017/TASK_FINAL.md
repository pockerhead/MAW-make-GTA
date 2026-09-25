# TASK-017: GDD T16 — Производительность и финальная приёмка

Type: feature
Mode: full
Priority: high
Branch: feature/t16-performance-final-acceptance
Domains: bevy-ecs, gates, game-design

## Description
Implement slice **T16** of the APPROVED design document `docs/design/GDD.md` (§13), together with every
GDD section the slice touches (read them: world §2, player §3, combat §4, vehicles §5, NPC §6, UI §7,
audio/juice §8, content §9, stack §10, performance §11, workspace/plugin/data map §12). The GDD is the
scope law: its data files (§12), crate/plugin boundaries and verified crate versions (§10.1) are binding.

Goal (GDD §13): `--bench-scene` (худшая сцена раздела 11), замер, исправление узких мест только по трейсу (разрезка, слияние мешей, occlusion culling по замеру, кинематические дальние NPC при нужде), чек-лист раздела 1, баг-баш.

## Dependencies
- blocked by TASK-002 — GDD slice T1 must land first
- blocked by TASK-003 — GDD slice T2 must land first
- blocked by TASK-004 — GDD slice T3 must land first
- blocked by TASK-005 — GDD slice T4 must land first
- blocked by TASK-006 — GDD slice T5 must land first
- blocked by TASK-007 — GDD slice T6 must land first
- blocked by TASK-008 — GDD slice T7 must land first
- blocked by TASK-009 — GDD slice T8 must land first
- blocked by TASK-010 — GDD slice T9 must land first
- blocked by TASK-011 — GDD slice T10 must land first
- blocked by TASK-012 — GDD slice T11 must land first
- blocked by TASK-013 — GDD slice T12 must land first
- blocked by TASK-014 — GDD slice T13 must land first
- blocked by TASK-015 — GDD slice T14 must land first
- blocked by TASK-016 — GDD slice T15 must land first

## Acceptance criteria
- [ ] Headless: все бенч-тесты T2/T8/T15 зелёные
- [ ] Headless: `cargo test -p gta_sim`, `-p citygen` зелёные
- [ ] Headless: `cargo clippy` без предупреждений.
- [ ] Runtime QA (GDD D1): the scenario `tools/qa/scenarios/t16.py` exists and passes via `tools/qa/brp.py` — `--bench-scene --features dev,profile`, `get_diagnostics` 30 с (средний и минимальный FPS, frame time), трейс `trace_chrome` с топом систем, скриншоты. Результат это доказательство для владельца, не автоматический гейт.
- [ ] Owner-run criterion recorded as an owner checklist in QA_REPORT.md: проходит все 11 пунктов раздела 1 на своей машине, включая угон, трафик и автомобильную погоню.
- [ ] Every new tuning value lives in its GDD §12 data file, not in a `const`
- [ ] `cargo build`, `cargo clippy -- -D warnings`, `cargo test -p gta_sim` (and `-p citygen` where touched) are green
- [ ] Existing tests pass

## Orchestrator notes (carried, binding)
- Runtime scenario flakes to make deterministic before final acceptance: t9 (gang member in Attack does not fire within 6 s, ~1/3; fire-line starvation class), t13 (one OS mouse click lost in a 6-click burst, magazine 12→7, ~2/3 in TASK-015 fixer round 3). Every t*.py must pass N consecutive runs.
- From TASK-016: traffic cannot go around a car stopped in its lane (abandoned / dismounted police car); the lane waits until the bubble despawns it (player 25 m away and out of view for 2 s). Owner-run judgement item; if it reads as a gridlock bug, a go-around needs an oncoming-lane reservation honoured by oncoming IDM, junction room() and the spawner (fixer estimate 150-200 lines; silent kinematic pass-through risk).
- From TASK-016: intersection throughput ~1 car / 4 s (whole-connector reservation vs GDD "conflict points") — owner-run item.

### Orchestrator decisions after the premise challenge (2026-09-25, owner delegation)
- The t13 flake is a GAME behaviour, not the harness: a click during the weapon cooldown is dropped (semi-auto, fire_interval 0.3 s → 20 ticks = 0.3125 s at 64 Hz). A player clicking ~3/s silently loses shots. Fix in the game: buffer one fire request during the cooldown for a data window (`fire_buffer_seconds`, e.g. 0.15 s) and fire on the first tick it expires — gate it (a click 30 ms before the cooldown ends fires on expiry; a click 0.5 s early does not queue). Do NOT widen the script's click gap to hide it.
- The go-around note is superseded by TASK-032 (redesign); drop it from T16.
- The §1 owner-run acceptance is replaced by the TASK-031 agent playtest (owner decision). T16's AC becomes: every §1 point has a runtime scenario or probe that EXERCISES it (evidence for TASK-031), not a checklist.
- Performance: per GDD §11, no automatic pass/fail on FPS; `--bench-scene` + trace + a written verdict "nothing to fix by trace" is an acceptable outcome at the measured costs.
- The t9 flake (gang member in Attack not firing within 6 s) stays in the bug bash: diagnose root cause first (same class as the fire-line starvation), fix if it is a game bug.

### Planner questions (orchestrator, 2026-09-25)
- Q1 (t9 crossfire starvation): choose the RULE change, not the flank-points patch — per the new redesign rule (a universal rule beats another special case). The hold-fire line currently extends to full weapon range beyond the target, so two groups on opposite sides starve each other. New rule: the line is checked from muzzle to target + `overshoot_margin` (data, gangs.ron/escalation.ron shared via tactics; derive a value from the spread cone, ~8 m), not to full range. Stray hits far beyond the target are accepted as rare, realistic crossfire (GTA-like). Gates: the crossfire layout fires within N s (flip RED with full-range check); the existing same-side "0 friendly damage" gates stay green; report the measured friendly-hit rate in the crossfire layout (non-zero allowed, must be small). t9 ×20.
- Q2: N = 3 for all scenarios, 10 for t13, 20 for t9 — accepted.
- Q3: measure honestly; the bench scene reports the actual gang count in Downtown (0) — the §11 "12 gang members" is a worst case the city rarely produces; add an optional `--bench-scene gangs` variant only if cheap, otherwise document.
- Q-A (PR1): police `overshoot_margin: 60` (≥ longest police gun range) — police keep today's disciplined no-crossfire behaviour bit for bit (TASK-012 fixed cops shooting each other across the player); gangs get 8 m — sloppy crossfire is their fantasy. The rule stays universal (one parameter per faction in data), not a special case.
- Q-B (PR1): "small" stray-hit rate = ≤ 5 % of the layout's shots and ≤ 2× the measured value.
