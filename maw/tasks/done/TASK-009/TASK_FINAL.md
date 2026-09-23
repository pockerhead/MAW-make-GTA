# TASK-009: GDD T8 — Мирные жители

Type: feature
Mode: full
Priority: high
Branch: feature/t08-civilians
Domains: bevy-ecs, gates, game-design

## Description
Implement slice **T8** of the APPROVED design document `docs/design/GDD.md` (§13), together with every
GDD section the slice touches (read them: world §2, player §3, combat §4, vehicles §5, NPC §6, UI §7,
audio/juice §8, content §9, stack §10, performance §11, workspace/plugin/data map §12). The GDD is the
scope law: its data files (§12), crate/plugin boundaries and verified crate versions (§10.1) are binding.

Goal (GDD §13): пузырь населения, спавн вне кадра, FSM (Wander/Idle/Flee/Cower/Report/Dead), восприятие с разрезкой, utility-выбор реакции, смерть с анимацией, лимит трупов. Стресс-замер: 64 Tnua-NPC.

## Dependencies
- blocked by TASK-003 — GDD slice T2 must land first
- blocked by TASK-005 — GDD slice T4 must land first
- blocked by TASK-006 — GDD slice T5 must land first

## Acceptance criteria
- [ ] Headless: wander не уходит с графа
- [ ] Headless: выстрел в 20 м → Flee/Cower в пределах одного цикла восприятия
- [ ] Headless: деспавн за 150 м после 2 с вне кадра
- [ ] Headless: бенч 64 NPC × 640 тиков пишет время тика, жёсткий порог только на порядок (ловит O(N²) и зависания).
- [ ] Runtime QA (GDD D1): the scenario `tools/qa/scenarios/t8.py` exists and passes via `tools/qa/brp.py` — подсчёт мирных по состояниям (`world.query`), выстрел в воздух, повторный подсчёт (доля Flee/Cower выросла), скриншот, `get_diagnostics` при 40 мирных.
- [ ] Owner-run criterion recorded as an owner checklist in QA_REPORT.md: улицы живые, выстрел в воздух разгоняет толпу.
- [ ] Every new tuning value lives in its GDD §12 data file, not in a `const`
- [ ] `cargo build`, `cargo clippy -- -D warnings`, `cargo test -p gta_sim` (and `-p citygen` where touched) are green
- [ ] Existing tests pass

### Resolved questions

Planner's open questions, answered by the orchestrator (owner delegation, 2026-09-23):
- Q1 civilian look: **separate models, not one tinted model.** The owner's brief asks for a varied world; identical clones kill it, and gangs (T9) will need their own looks. Every Kenney GLB carries all 32 clips, so build one animation graph PER MODEL from that model's own clips (the glTF root-name path issue makes cross-model clips silently dead). Use several of the 12 character models for civilians (keep the player on male-a), plus the tint for extra variety; model list in data (`visual.ron`/population data). Gate: every civilian's animator plays a clip from its own model (no silent T-pose).
- Q2 empty streets for ~30 s after loading: **not acceptable.** At load, do an initial fill closer than the 60 m inner ring edge (e.g. from ~20 m) but outside the camera view cone / line of sight, so the city is alive from the first frame; steady-state spawning keeps the ring rule.
- Q3 witnesses closer than 25 m never phone the police: accepted (close witnesses flee).
