# TASK-012: GDD T11 — Полиция пешком + арест

Type: feature
Mode: full
Priority: high
Branch: feature/t11-police-on-foot-arrest
Domains: bevy-ecs, gates, game-design

## Description
Implement slice **T11** of the APPROVED design document `docs/design/GDD.md` (§13), together with every
GDD section the slice touches (read them: world §2, player §3, combat §4, vehicles §5, NPC §6, UI §7,
audio/juice §8, content §9, stack §10, performance §11, workspace/plugin/data map §12). The GDD is the
scope law: its data files (§12), crate/plugin boundaries and verified crate versions (§10.1) are binding.

Goal (GDD §13): `PoliceDispatcher`, эскалация 1-5 из `escalation.ron` (пешие юниты), спавн вне кадра, FSM копа (Respond/Arrest/Attack/Search), арест → `Busted` → экран "BUSTED" (переиспользует виджет экрана смерти) → участок и изъятие оружия, SWAT на 4-5, поиск по кругу. Решение по навмешу по факту застреваний. Опционально включить stretch Q2=B.

## Dependencies
- blocked by TASK-010 — GDD slice T9 must land first
- blocked by TASK-011 — GDD slice T10 must land first

## Acceptance criteria
- [ ] Headless: 1 звезда + игрок рядом и пассивен 1.5 с → `Busted`, оружие изъято
- [ ] Headless: число юнитов ≤ таблицы
- [ ] Headless: при потере видимости копы идут к `LastKnownPosition`
- [ ] Headless: "вырваться" даёт +1 звезду.
- [ ] Runtime QA (GDD D1): the scenario `tools/qa/scenarios/t11.py` exists and passes via `tools/qa/brp.py` — `world.mutate_resources` heat до 1 звезды, ожидание копа, пассивность, скриншот "BUSTED", чтение `GameState` и инвентаря; отдельный прогон на 4 звёздах со скриншотом SWAT.
- [ ] Owner-run criterion recorded as an owner checklist in QA_REPORT.md: полная петля "набедокурил → погоня → ушёл или арестован/убит".
- [ ] Every new tuning value lives in its GDD §12 data file, not in a `const`
- [ ] `cargo build`, `cargo clippy -- -D warnings`, `cargo test -p gta_sim` (and `-p citygen` where touched) are green
- [ ] Existing tests pass

## Orchestrator note (from TASK-010)

- Police reuse the gang fire-line rules (`gang/behavior/fire_line.rs`: hold fire on a non-hostile in the widened line, candidate reposition, pinned-scrum hold). Known gap from TASK-010 QA round 3: in a corridor <= 3 m wide the rear shooter's close-in fallback walks into a non-pinned groupmate and never fires (`maw/tasks/done/TASK-010/QA_REPORT.md` Bug 1). Fix it for both roles here (e.g. the close-in path treats groupmates as avoidance obstacles / picks a lateral queue slot) and gate it with the corridor layout.

### Resolved questions

Planner's open questions, answered by the orchestrator (owner delegation, 2026-09-24):
- Q2=B (gangs ↔ police hostile) stays OFF: GDD marks it a stretch "only if there is budget"; this slice is already the largest NPC slice.
- Break free = run farther than 3 m from the arresting cop during the hold (GTA IV), no separate button.
- Wanted and crimes reset on EXIT from Busted (the arresting cop must not vanish mid-scene).
- "Fewer civilians at 5 stars" deferred (not in this slice's acceptance).
