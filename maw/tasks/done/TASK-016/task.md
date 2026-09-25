# TASK-016: GDD T15 — Трафик и полицейские машины (обязательно, Q1=C)

Type: feature
Mode: full
Priority: high
Branch: feature/t15-traffic-police-cars
Effort: planner=high, code-reviewer=high
Domains: bevy-ecs, gates, game-design

## Description
Implement slice **T15** of the APPROVED design document `docs/design/GDD.md` (§13), together with every
GDD section the slice touches (read them: world §2, player §3, combat §4, vehicles §5, NPC §6, UI §7,
audio/juice §8, content §9, stack §10, performance §11, workspace/plugin/data map §12). The GDD is the
scope law: its data files (§12), crate/plugin boundaries and verified crate versions (§10.1) are binding.

Goal (GDD §13): трафик по графу полос (кинематика + IDM, переход в динамику при контакте), резервация перекрёстков, спавн/деспавн 70/90 и 15/25 м с правилом 2 с, угон из трафика с выбросом водителя, полицейские машины по таблице звёзд, преследование по графу полос и высадка копов, сирены.

## Dependencies
- blocked by TASK-012 — GDD slice T11 must land first
- blocked by TASK-015 — GDD slice T14 must land first

## Acceptance criteria
- [ ] Headless: IDM не даёт отрицательной скорости и наложений на тестовой полосе из 10 машин за 6400 тиков
- [ ] Headless: на перекрёстке одна машина в точке конфликта одновременно
- [ ] Headless: деспавн только после ≥ 2 с вне кадра
- [ ] Headless: кинематическая машина становится динамической при контакте
- [ ] Headless: при 2 звёздах диспетчер держит ≤ 2 полицейские машины.
- [ ] Runtime QA (GDD D1): the scenario `tools/qa/scenarios/t15.py` exists and passes via `tools/qa/brp.py` — подсчёт машин трафика и их скоростей, угон ближайшей машины (F), `mutate_resources` heat до 2 звёзд, ожидание, чтение расстояния от полицейских машин до игрока (уменьшается), скриншоты погони, `get_diagnostics`.
- [ ] Owner-run criterion recorded as an owner checklist in QA_REPORT.md: улицы с движением, угнал машину из потока, пережил погоню на машинах.
- [ ] Every new tuning value lives in its GDD §12 data file, not in a `const`
- [ ] `cargo build`, `cargo clippy -- -D warnings`, `cargo test -p gta_sim` (and `-p citygen` where touched) are green
- [ ] Existing tests pass

## Orchestrator notes (from TASK-015, binding)
- O2 (QA TASK-015): a player in a car can be neither arrested nor wounded, so driving is a safe haven. Implement Q2(b) here: a bullet hitting the car's cabin/window zone wounds the driver (data-driven share), and cops can pull a driver out of a car stopped (≤ exit speed) for an arrest at 1 star (GTA-like). Gate both.
- O1: the hold-fire rule spares bystanders but not cars: a cop whose line grazes a parked car's corner wastes shots into it. Treat non-target cars in the widened fire line like other non-hostile blockers (reposition), gated.
- Forced eject on Wasted/Busted still uses the left door's feet ray and can land on a low wall top (TASK-015 FIX_SUMMARY round 2 §1); apply the same feet-height rule to the forced path.
