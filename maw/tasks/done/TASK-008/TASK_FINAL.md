# TASK-008: GDD T7 — Ближний бой

Type: feature
Mode: full
Priority: high
Branch: feature/t07-melee
Domains: bevy-ecs, gates, game-design

## Description
Implement slice **T7** of the APPROVED design document `docs/design/GDD.md` (§13), together with every
GDD section the slice touches (read them: world §2, player §3, combat §4, vehicles §5, NPC §6, UI §7,
audio/juice §8, content §9, stack §10, performance §11, workspace/plugin/data map §12). The GDD is the
scope law: its data files (§12), crate/plugin boundaries and verified crate versions (§10.1) are binding.

Goal (GDD §13): комбо кулаков, бита, окна атак из `melee.ron`, stagger, knockback через `TnuaBuiltinKnockback`, knockdown, локальная заморозка анимации 50 мс (презентация, `Time<Real>`) и trauma.

## Dependencies
- blocked by TASK-006 — GDD slice T5 must land first
- prefer after TASK-007 — shares code with GDD slice T6

## Acceptance criteria
- [ ] Headless: удар в окне попадает, вне окна нет
- [ ] Headless: третий удар комбо сбивает с ног
- [ ] Headless: knockback двигает цель в направлении удара (3 направленных случая: цель по −Z, по +X, по +Z от атакующего)
- [ ] Headless: hit-stop не меняет `Time<Virtual>` и число фиксированных тиков.
- [ ] Runtime QA (GDD D1): the scenario `tools/qa/scenarios/t7.py` exists and passes via `tools/qa/brp.py` — телепорт к манекену, три клика ЛКМ, чтение состояния knockdown манекена, серия скриншотов.
- [ ] Owner-run criterion recorded as an owner checklist in QA_REPORT.md: драка "хлёсткая", тряска не тошнит.
- [ ] Every new tuning value lives in its GDD §12 data file, not in a `const`
- [ ] `cargo build`, `cargo clippy -- -D warnings`, `cargo test -p gta_sim` (and `-p citygen` where touched) are green
- [ ] Existing tests pass

### Resolved questions

Planner's open questions, answered by the orchestrator (owner delegation, 2026-09-23) — defaults accepted:
- Q1 jab knockback 2 m/s (not GDD's 3-6 m/s): the probe shows the finisher misses from 1 m at 3 m/s. Values live in melee.ron.
- Q2 bat selection: pressing `1` again toggles fists <-> bat.
- Q3 screen shake by camera rotation (not pivot offset).
