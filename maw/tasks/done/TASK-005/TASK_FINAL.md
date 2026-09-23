# TASK-005: GDD T4 — Гуманоид с анимациями

Type: feature
Mode: full
Priority: high
Branch: feature/t04-humanoid-animations
Domains: bevy-ecs, gates, game-design

## Description
Implement slice **T4** of the APPROVED design document `docs/design/GDD.md` (§13), together with every
GDD section the slice touches (read them: world §2, player §3, combat §4, vehicles §5, NPC §6, UI §7,
audio/juice §8, content §9, stack §10, performance §11, workspace/plugin/data map §12). The GDD is the
scope law: its data files (§12), crate/plugin boundaries and verified crate versions (§10.1) are binding.

Goal (GDD §13): скачать и проверить архив Kenney Mini Characters (SHA-256; перепроверить 12 персонажей / 14 костей / 32 клипа и записать в манифест), привязка модели к Tnua-капсуле, анимационный граф по `AnimState` (idle/walk/sprint/jump/fall), тинт одежды, масштаб под 1.8 м.

## Dependencies
- blocked by TASK-002 — GDD slice T1 must land first

## Acceptance criteria
- [ ] Headless: `AnimState` выводится из скорости и опоры чистой функцией (таблица случаев: стоит, идёт, бежит, спринт, в воздухе вверх, падение).
- [ ] Runtime QA (GDD D1): the scenario `tools/qa/scenarios/t4.py` exists and passes via `tools/qa/brp.py` — серия `send_keys` (W, W+Shift, Space) со скриншотами каждые 200 мс и чтением `AnimState` через `world.get_components`.
- [ ] Owner-run criterion recorded as an owner checklist in QA_REPORT.md: персонаж бегает с правильными анимациями, ноги заметно не скользят.
- [ ] Every new tuning value lives in its GDD §12 data file, not in a `const`
- [ ] `cargo build`, `cargo clippy -- -D warnings`, `cargo test -p gta_sim` (and `-p citygen` where touched) are green
- [ ] Existing tests pass

### Resolved questions

Premise challenge (2026-09-23) returned PREMISE SUSPECT on a FACT, not on the slice: the GDD's "12 characters / 14 bones / 32 clips" for Kenney Mini Characters does not match the SHA-256-pinned archive — its audit (`scratch/audit_asset.py`) finds 7 unique joint nodes shared by two skins per character GLB. Orchestrator decision (owner delegation): the slice stands; the criterion becomes "verify and RECORD in the manifest what the pinned archive actually contains (character count, joints per skin, clip names/count)", and the animation graph is built on the real clip set. The GDD numbers are not a gate.

Planner's open questions, answered by the orchestrator (owner delegation, 2026-09-23) — all defaults accepted:
- GDD facts: YES — correct the Kenney Mini Characters numbers in docs/design/GDD.md (two factual lines: 7 joints shared by two skins, 32 clips, no "run" clip); no scope change.
- Tint via base_color multiplier on body-mesh (also tints the hands): accepted.
- Run gait plays the sprint clip at the derived rate (x0.63 by default): accepted.
- Player model: male-a.
