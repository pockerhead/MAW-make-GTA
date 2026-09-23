# TASK-020: Remove the custom ledge assist, back to plain Tnua

Type: refactor
Mode: small-fix
Priority: high
Branch: refactor/remove-ledge-assist
Domains: bevy-ecs, gates

## Description
Owner decision (2026-09-23): "давай обычное поведение пока, важнее пойти дальше". The custom jump ledge assist added in TASK-002 fixer round 2 is removed; the character goes back to plain Tnua float/jump behaviour (whatever natural pull-up the Tnua float spring gives stays — it is not ours to gate). History: `maw/tasks/blocked/TASK-018/`, `maw/tasks/blocked/TASK-019/`.
Remove: `crates/gta_sim/src/character/ledge.rs`, its `mod ledge;`, the `ledge::LedgeAssist` required component and the `(ledge::assist_ledge, ledge::apply_ledge)` system registration in `crates/gta_sim/src/character/mod.rs`; the four `ledge_assist_*` fields in `crates/gta_sim/src/character/locomotion.rs` and `assets/character/locomotion.ron`; `crates/gta_sim/tests/ledge.rs`. Keep everything else from TASK-002 (jump buffer, tap gate, sprint gate, terrain gates, Tnua values in RON).

## Acceptance criteria
- [ ] `rg -n "ledge" crates src assets` finds no ledge-assist code, config or test
- [ ] Remaining `gta_sim` tests green: config, jump (apex, tap, buffered late tap), movement (yaw 0/90/180, sprint), terrain (box blocks walking, stairs climbed)
- [ ] `cargo clippy -p gta_sim --all-targets -- -D warnings` green; `cargo build` green
- [ ] No windowed-client build under the codex sandbox; QA runs `tools/qa/scenarios/t1.py`
- [ ] Existing tests pass
