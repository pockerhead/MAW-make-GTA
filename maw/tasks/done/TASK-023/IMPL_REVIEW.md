# TASK-023 implementation review

## Verdict

**NEEDS_WORK** — the QA key overlap diagnosis is supported, and the city route passes, but the guard can miss an overlap and the committed gate does not exercise the fix.

## Disconfirmation tested

Counterexample: if the player still has forward `MoveIntent` while stationary at either reported crossing, the key release explanation is wrong. The recorded release trace (`scratch/runtime_overlap.json`) instead changes from axis `(0, 1)` to `(0, 0)` immediately after the first repeated W press near z = -17.8; a single hold (`scratch/runtime_hold.json`) retains `(0, 1)` and reaches z = 71.76. The pinned `bevy_brp_extras-0.22.6/src/keyboard/keys.rs:128-167` creates a timer per press and releases its keys independently. This counterexample did not hold. The log's rejected obstacle hypothesis is also supported by the seed-1 headless route gate and by `src/visuals/props.rs:26,75-91`, where props have no collider.

## Confirmed correct

- `crates/gta_sim/tests/crossing_run.rs:14-58` uses the production headless city composition, seed 1, the reported yaw and a drifting forward path. It passes both reported z positions. A pole inserted at either position made this gate fail; the restored gate passes. I reran `cargo test -p gta_sim --test crossing_run`: 1 passed.
- `tools/qa/brp.py:132-142` rejects a repeated key press while its locally recorded hold is active. This catches the 4.8 s re-press during a 5 s hold used by the TASK-022 probe under normal response timing.
- The supplied `t1` and `t8` results report successful shutdown and no log errors, respectively. I reran `cargo clippy --workspace --all-targets -- -D warnings` successfully. The supplied test outputs show `cargo test -p gta_sim -p citygen` and `cargo test -p gta_like --bin gta_like` green. No citygen placement file changed, so the unchanged golden hashes are consistent with the diff.

## Issues

1. **Major — `tools/qa/brp.py:136-142`: response latency can defeat the overlap guard.** `held_until` starts at the time *before* the RPC. The game timer starts after the press is processed. If the call takes longer than the 0.25 s margin, the local hold can expire before the response arrives, permitting another press while the original release timer remains live. A 400 ms stubbed call with a 100 ms hold left `held_until` 56 ms in the past when it returned (`scratch/review_guard_probe.py`). Record the reservation relative to confirmed call completion, or use a protocol mechanism that tracks the game-side release before permitting a repeat; gate a delayed response case.
2. **Major — `crates/gta_sim/tests/crossing_run.rs:14-58`: the only committed gate cannot turn red for the actual fix.** It writes `MoveIntent` directly and never calls `Game.send_keys`. Reverting the guard in `tools/qa/brp.py` leaves this test green. The pole sabotage proves collision-path correctness, not the original overlapping-timer regression required by acceptance criterion 2. Add a committed harness test that reproduces overlapping W presses and proves the guard rejects them; flip it red by removing the guard, then green after restoring it. Keep the city route test as a separate collision gate.
3. **Minor — `tools/qa/brp.py:140-142`: a failed RPC poisons local key state.** `held_until` is set before `self.call`; an immediate RPC error leaves the key marked busy, so a retry fails locally for the entire requested hold. The stubbed timeout in `scratch/review_guard_probe.py` reproduces this. Update the reservation only after a confirmed successful press, and handle an ambiguous timeout by invalidating the QA session or otherwise reconciling game state.

## Missing coverage

- A committed success, overlap rejection, and delayed RPC response test for `Game.send_keys`; the only guard check is an untracked scratch script and does not exercise response latency.
- A failed or timed-out BRP call followed by a retry, including the ambiguous case where the press may have reached the game.

The route's visual appearance and passing through lamp meshes remain owner-run observations; they do not require another automated gate for this task.

children: 0 launched / 0 reported
