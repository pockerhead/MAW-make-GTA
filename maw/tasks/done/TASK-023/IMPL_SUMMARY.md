# TASK-023 IMPL_SUMMARY (small-fix, implementer)

Cost of error: low for the game (nothing is broken in it), high for the process. A phantom stall made QA
cut the TASK-022 runtime walks short and sent this task toward the city collision code.

## Root cause (AC1)

**There is no obstacle. The QA harness drops the W key.** The TASK-022 probes (`qa/qa_runtime.py`,
`qa2/qa2_runtime.py`) hold W with `send_keys(["KeyW"], 5000)` and send it again every 4.8 s.
bevy_brp_extras 0.22.6 (`src/keyboard/keys.rs:26-31`, `:128-138`, `:147-167`) creates one
`TimedKeyRelease` entity with its own timer for each call. The second press lands about 0.2 s before
the first timer ends, and that old timer then releases W. The player stops. Every later re-press gives
only a 0.2 s creep of 0.2-0.6 m, which is the "creeping" you see in the TASK-022 logs.

Evidence:
- **Runtime, TASK-022 key pattern** (`scratch/runtime_key_overlap.py`, `MODE=overlap`,
  `scratch/runtime_overlap.json`): release, seed 1, yaw pi-0.0349. The player runs for 5 s, then stops
  at (6.41, 1.2, -17.71). In **137 of the 141 stalled intervals, `MoveIntent.axis` = (0, 0)**, so no
  input reached the sim. The other 4 intervals are the start-up turn. Every stop comes right after a
  re-press (presses at 0, 4.94, 9.8, 14.74, ...).
- **Runtime, one 30 s hold** (`MODE=hold`, `scratch/runtime_hold.json`): same build, spawn and yaw.
  The player runs from (7.2, -40.23) to **(3.29, 1.11, 71.76)**. That is 112 m, past both reported
  stall points (-19.6 and 4.5). Stalled intervals: 0.
- **Timing matches.** Each TASK-022 run segment lasts one hold (5 s x 4.5 m/s run speed = 22.5 m).
  The runtime_nociv run had 2 full holds, about 45 m: that is the "about 42 m", ending at (5.6, 4.5).
  The qa2 run had 1 hold, ending at -19.6.
- **The drift is the heading, not a deflection.** `move_direction((0,1), pi-0.0349)` =
  (-0.0349, 0, 0.9994), so x drops by 0.035 m per metre of z. 7.2 - 0.035 x 44.7 = 5.64, exactly the
  "x 7.2 -> 5.6" in the report.
- **The "lamp post" cannot block.** City props are visual only: `CityProp` has no collider
  (`src/visuals/props.rs:26`, "visual only, no collider until T14"). The static colliders are the
  ground, edge walls, block curb prisms and buildings (`crates/gta_sim/src/world/city.rs`). In
  `qa2/runtime/periodic_16_B.png` the player just stands in front of a mesh it would have run through.
- **Headless, same path:** the gate below passes both points at full run speed. It is at
  x = 5.637 when z = 4.44, the same spot where the runtime run stopped.

## What was implemented

| File | Change |
|---|---|
| `crates/gta_sim/tests/crossing_run.rs` (new, 58 lines) | Gate `player_runs_through_seed1_crossings_on_qa_path`. Real seed-1 city, `max_civilians = 0`, settle at the spawn, intent axis (0,1), yaw pi-0.0349 (the QA path with its drift), 20 s. Every 1 s window after the first must cover > 0.8 x `run_speed`, and the end z must be > 30. |
| `tools/qa/brp.py` (+12 lines) | `Game.send_keys` records how long each key is held (plus a 0.25 s margin for game time lagging the wall clock). It raises `RuntimeError` on a re-press of a key still held by an earlier call. This fixes the root cause: the TASK-022 pattern now fails loudly instead of producing a phantom stall. |

No game code, city collision or prop placement changed, so the `citygen` golden hashes are unchanged
(no citygen file touched).

## Deviations from the spec / not implemented

- The spec expected a physical obstacle and a fix in "city collision or prop placement". The evidence
  says there is no obstacle, so no game fix exists. I fixed the root cause in the QA harness
  (`tools/qa/brp.py`). The alternative was cancelling the older release timer over BRP. That is
  impossible because `TimedKeyRelease` is not reflected (`keys.rs:26`, plain `#[derive(Component)]`).
- AC2 "fails before the fix": the product had no bug, so the gate is GREEN on both the old and the new
  code. I showed it can go RED by planting an obstacle on the path (flip-RED below). This makes it a
  correctness gate that the QA path stays passable, and it locks in the "no obstacle" finding.
- AC3 (placement-rule gate over 3 seeds) does not apply: the cause is not placement.
- Owner-visible, out of scope: lamp posts have no collider, so the player runs **through** them
  visually. On the QA path the player passes about 0.3-0.5 m from a pole (`periodic_16_B.png`). This
  is the known "no collider until T14" state, not this bug.
- `scratch/runtime_overlap.json` was recorded before the guard existed. With the guard,
  `MODE=overlap` now raises on the second press, and that is the intended behaviour.

## Flip-RED (gate)

| Perturbation | Result |
|---|---|
| Static 0.3 x 3 x 0.3 m pole (lamp-post size) at (6.47, 1.5, -19.6), on the headless path | RED: "second 4: moved 2.49 m ... -20.05" (`scratch/flip_red_pole.txt`) |
| Same pole at (5.64, 1.5, 4.44), the second stall point | RED: "second 10: moved 0.00 m" at z 3.99 (`scratch/flip_red_pole_z4.txt`) |
| Restored (file copied back, no FLIP line left) | GREEN (`scratch/flip_restored.txt`) |

Harness guard check (offline, `call` stubbed): `scratch/brp_send_keys_guard_check.py` refuses an
overlapping KeyW press, allows another key, and allows KeyW again after the hold plus margin. Output: OK.

## Test results

- `cargo clippy --workspace --all-targets -- -D warnings`: clean.
- `cargo test -p gta_sim -p citygen`: exit 0, every suite ok, including `crossing_run` 1 passed
  (`scratch/test_gta_sim_citygen.txt`).
- `cargo test -p gta_like --bin gta_like`: 38 passed (`scratch/test_gta_like_bin.txt`). Run once;
  this task touches no presentation gate.
- `python tools/qa/scenarios/t1.py --out scratch/t1`: exit 0, shutdown passed. It runs on the
  patched `brp.py`.
- `python tools/qa/scenarios/t8.py --out scratch/t8`: exit 0, `log_errors: []`.
- Runtime QA probe: `MODE=hold` runs 112 m past both crossings (above). No game process was left
  running.

## How to verify by hand

1. `cargo test -p gta_sim --test crossing_run` shows it GREEN.
2. `cd maw/tasks/in_progress/TASK-023/scratch; MODE=hold python runtime_key_overlap.py`: the end z is
   above 60 and "stalled ... 0".
3. `MODE=overlap python runtime_key_overlap.py` now fails at once with
   `send_keys: ['KeyW'] still held by an earlier call`. Before the guard it reproduced the stall with
   axis 0 (`runtime_overlap.json`).
4. In game (owner): seed 1, hold W from the spawn along the street. The run goes through the
   crossings. The character passes through lamp-post meshes because props have no collider (T14).

children: 0 launched / 0 reported
