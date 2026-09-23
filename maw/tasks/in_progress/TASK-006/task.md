# TASK-006: GDD T5 — Здоровье, урон, смерть и возрождение

Type: feature
Mode: full
Priority: high
Branch: feature/t05-health-death-respawn
Domains: bevy-ecs, gates, game-design

## Description
Implement slice **T5** of the APPROVED design document `docs/design/GDD.md` (§13), together with every
GDD section the slice touches (read them: world §2, player §3, combat §4, vehicles §5, NPC §6, UI §7,
audio/juice §8, content §9, stack §10, performance §11, workspace/plugin/data map §12). The GDD is the
scope law: its data files (§12), crate/plugin boundaries and verified crate versions (§10.1) are binding.

Goal (GDD §13): здоровье/броня, регенерация до 50%, `GameState::Wasted`, slow-mo в `flow`, экран "ПОТРАЧЕНО" из `strings.ron`, возрождение у больницы, HUD здоровья и брони, пикапы аптечки и брони, сообщение `DebugDamage` (для отладки и BRP `world.write_message`).

Known hazard from TASK-004 QA: city visuals and colliders are spawned on `OnEnter(GameState::Playing)`. This slice adds `Wasted` and a return to `Playing` — re-entering `Playing` must NOT spawn the city a second time (make city spawning one-shot / keyed to the loaded layout, and gate it: die, respawn, count city chunks and building colliders unchanged).

## Dependencies
- blocked by TASK-003 — GDD slice T2 must land first
- blocked by TASK-005 — GDD slice T4 must land first

## Acceptance criteria
- [ ] Headless: броня поглощает до нуля, потом здоровье
- [ ] Headless: регенерация не превышает 50%
- [ ] Headless: смерть → `Wasted` → по `Time<Real>` через заданное время `Playing` у больницы, оружие сохранено, розыск 0
- [ ] Headless: slow-mo восстанавливает `Time<Virtual>` 1.0 при выходе из `Wasted`.
- [ ] Runtime QA (GDD D1): the scenario `tools/qa/scenarios/t5.py` exists and passes via `tools/qa/brp.py` — `world.write_message` с `DebugDamage` до нуля здоровья, скриншот экрана "ПОТРАЧЕНО", ожидание, чтение `GameState` и позиции игрока (рядом с больницей).
- [ ] Owner-run criterion recorded as an owner checklist in QA_REPORT.md: получает урон, видит полосы, умирает, возрождается; slow-mo и экран смерти читаются.
- [ ] Every new tuning value lives in its GDD §12 data file, not in a `const`
- [ ] `cargo build`, `cargo clippy -- -D warnings`, `cargo test -p gta_sim` (and `-p citygen` where touched) are green
- [ ] Existing tests pass
