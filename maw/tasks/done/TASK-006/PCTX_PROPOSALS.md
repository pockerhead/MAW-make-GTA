# PCTX proposals — TASK-006

## 2026-09-23 (planner) — bevy-ecs: исключение для таймеров по `Time<Real>`

Инвариант "таймеры геймплея в `FixedUpdate`" неверен для таймеров, которые по дизайну идут по реальному времени (GDD §3.4: `Wasted`/`Busted` по `Time<Real>`, чтобы slow-mo их не растягивал). Под `set_relative_speed(0.3)` `FixedUpdate` выполняется ~0.3 раза за кадр (проба TASK-006: 86 фиксированных тиков за 288 кадров), и таймер на `Time<Real>::delta()` внутри `FixedUpdate` считает то 0, то двойную дельту. Предложение: дописать в bevy-ecs "таймер по `Time<Real>` живёт в `Update` под set-level `run_if(in_state(..))`; таймер по игровому времени — в `FixedUpdate`".

## 2026-09-23 (planner) — bevy-ecs: одноразовый спавн не на `OnEnter` состояния, в которое возвращаются

`OnEnter(GameState::Playing)` вызывался для спавна игрока и мешей города; с появлением `Wasted → Playing` оба спавна задвоились бы (город — тихо, 200 чанков вместо 100). Предложение-урок: одноразовая инициализация после загрузки идёт на `OnTransition { exited: Loading, entered: Playing }` (проба TASK-006: срабатывает один раз, на возврат из `Wasted` — нет), а гейт презентации считает сущности после полного цикла смерти.

## 2026-09-23 (planner) — gates: манифест ассетов больше не Kenney-only

TASK-006 добавляет пак шрифта из GitHub-релиза под SIL OFL 1.1 (у Kenney Fonts нет кириллицы). Если план пройдёт, в gates/pointers стоит отметить: источник пака = Kenney или GitHub release, лицензия = CC0 или OFL, проверка текста лицензии идёт по маркеру лицензии, а не по "CC0" для всех паков.

## 2026-09-23 — implementer (gates): error-keyword fixtures and grid-aligned test numbers

Proposed risk lesson for `domains/gates.md`: (1) a fixture gate that asserts "the error mentions
KEYWORD" is only as strong as KEYWORD is unique: `"page"` matched the `/media/pages/` inside a Kenney
url, so an `OFL accepts Kenney page` flip stayed GREEN until the keyword became `"OFL requires page"`.
Flip each fixture with a sabotage that yields a *different* error, not only "no error". (2) A cap/clamp
gate driven by step sizes that divide the distance exactly (30 -> 50 in 5/64 steps) never overshoots,
so removing the clamp stays GREEN; include one off-grid start. Evidence: TASK-006 `scratch/flip_red.log`.

> RESOLVED 2026-09-23: Time<Real> timer exception and one-shot-spawn rule -> domains/bevy-ecs.md (Invariants); fixture-keyword and off-grid clamp lessons -> domains/gates.md; manifest no longer Kenney-only is already reflected in the code/manifest schema (no PCTX change needed).
