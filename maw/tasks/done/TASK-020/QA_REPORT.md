# QA_REPORT — TASK-020 (remove custom ledge assist)

## Preflight

- `scratch/` held only `.gitignore` (no author probes). `log.jsonl` is empty, so there were no dead_end entries to triage.
- Read: task.md, IMPL_SUMMARY.md, IMPL_REVIEW.md, OPEN_DECISIONS.md (fixer skipped, no FIX_SUMMARY).
- Counter-example I went looking for: a leftover reference to the removed assist that breaks at runtime and not at compile time. Three candidates: (a) a `ledge_assist_*` key left in `locomotion.ron`, which the strict `deny_unknown_fields` loader would reject at startup; (b) `tools/qa/scenarios/t1.py` or `brp.py` querying a `LedgeAssist` component over BRP; (c) the stairs test having passed only because of the assist. Result: none of them holds. `rg -n -i ledge` over the repo outside `maw/` finds nothing. t1.py has no ledge reference. `walking_climbs_stairs` passes without the assist.

## 1. Environment

Direct, on the branch `refactor/remove-ledge-assist` @ 5a76fe3 (code commit 4c3a561), in the checkout `D:/test-gta-like`. There is no docker-compose. For runtime I used the windowed dev build through the project BRP driver. I started no services and left no processes (`tasklist | grep -ci gta_like` returns 0).

Reproduce:
```
cargo build -j 4
cargo clippy --workspace --all-targets -j 4 -- -D warnings
cargo test --workspace -j 4
rg -n "ledge" crates src assets
python tools/qa/scenarios/t1.py --out maw/tasks/in_progress/TASK-020/scratch/qa
```

## 2. Test results

- `cargo build` (whole workspace, including the `gta_like` client): green.
- `cargo clippy --workspace --all-targets -- -D warnings`: green.
- `cargo test --workspace`: 12 passed, 0 failed. lib `camera_relative_axes`. config `shipped_locomotion_config_loads`, `unknown_field_names_file_and_field`. jump `tapped_jump_fires`, `jump_apex_matches_jump_height`, `late_tap_is_buffered_until_landing`. movement `forward_yaw_0/90/180_*`, `sprint_uses_sprint_speed`. terrain `walking_into_arena_box_stays_blocked`, `walking_climbs_stairs`.
- Compared with the base 92b3b75 by name: the only tests that are gone are the two in the deleted `tests/ledge.rs` (`configured_reach_climbs_and_above_reach_blocks`, `pull_up_settles_quickly_without_burying_feet`). The spec asks for that removal. There are no new failures.
- `cargo tree -p gta_sim -e normal -i bevy_render`: empty, so the headless crate invariant holds.
- Flip-RED (my own): I added `ledge_assist_window: 1.2,` back into `assets/character/locomotion.ron`. `shipped_locomotion_config_loads` went RED with `Unexpected field named ledge_assist_window in LocomotionConfig`. I then ran `git checkout` on the file, and sha256 restore verified OK. This shows the strict loader guards against a half-done removal (struct field gone, RON key left behind).
- Runtime `t1.py` (dev build, BRP): passed. Movement delta (0.0, 0.0, -4.39) for 1 s of W. Camera yaw delta -0.419. Screenshot PNG valid. FPS 174.9 under `Fifo` (a liveness number, not a frame-cost measurement). Shutdown passed. Output is in `scratch/qa/summary.json`.
- Screenshot `scratch/qa/t1.png`: blue capsule player standing on the grey ground with a proper contact shadow. It is not sunk or floating. The ramp box is on the left and the stairs block is on the right. No render artefacts.

## 3. Acceptance criteria

| Criterion | Test performed | Result |
|---|---|---|
| `rg -n "ledge" crates src assets` finds no ledge-assist code, config or test | Ran it: exit 1, no matches. Also ran a case-insensitive repo-wide rg (outside `maw/`): no matches. | PASS |
| Remaining gta_sim tests green (config, jump apex/tap/buffered, movement yaw 0/90/180 + sprint, terrain box + stairs) | `cargo test --workspace`: all 12 named tests pass | PASS |
| `cargo clippy -p gta_sim --all-targets -- -D warnings` green; `cargo build` green | Workspace clippy with all targets, and a full workspace build | PASS |
| No windowed build under codex sandbox; QA runs `tools/qa/scenarios/t1.py` | Implementer built only `--lib`. I ran t1.py (this is not the codex sandbox), and it passed. | PASS |
| Existing tests pass | Same as above. No new failures compared with base, checked by name. | PASS |

## 4. Bugs found

None.

Nits (no action needed): IMPL_SUMMARY says "Deviations from plan: None" in small-fix mode, where there is no plan. `tools/qa/brp.py --help` does not print help. It launches the game and prints diagnostics (this is a tooling issue from before this task, outside its scope).

## 5. Verdict

**SHIP.** All five criteria pass on my own runs. The removal is exact: 5 files, +1/-258, matching the spec list. I flip-REDed one gate myself, and the runtime scenario passed.

Owner note (not a gate, the spec says so): launch `cargo run --features dev`, then walk and jump at the arena box and the stairs. You should see plain Tnua behaviour. The character can no longer mantle up onto ledges about 1.4 m high, and it steps up stairs through the float spring. Whether that feels acceptable is your call.
