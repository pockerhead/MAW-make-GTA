# TASK-023 FIX_SUMMARY (fixer, small-fix)

Scope: only the QA harness (`tools/qa/brp.py`) and its new offline gate (`tools/qa/test_brp.py`).
No Rust code changed in this stage; `crates/gta_sim/tests/crossing_run.rs` is untouched.

## Preflight: the review claim that would break things if applied verbatim

Issue 3 says "update the reservation only after a confirmed successful press". Taken literally, a
timed-out RPC leaves the key free, a retry is allowed, and if the timed-out press did reach the game its
release timer cuts the retry: exactly the TASK-022 overlap the guard exists to stop. I checked `call`
(`brp.py`): a JSON-RPC error reply raises `RuntimeError` (the game answered, nothing pressed); a timeout /
connection error raises something else (press state unknown). So the fix splits the two cases instead
of dropping the reservation on every failure. The review's own second sentence ("handle an ambiguous
timeout by invalidating ...") agrees with this.

## Fixed

1. **Major, response latency defeats the guard (`brp.py` send_keys).** Verified: the reservation was
   `now_before_call + ms + margin`; with a 0.4 s reply and a 100 ms hold it had already expired on return.
   Now the reservation is taken after the successful reply: `monotonic() after call + ms + margin`. The
   game starts its timer no later than it replies, so this is an upper bound (plus the existing 0.25 s
   game-time margin). Gate: `test_hold_counts_from_delayed_response`.
2. **Major, no committed gate exercises the fix.** Verified: `crossing_run.rs` writes `MoveIntent`
   directly and never touches `brp.py`. Added `tools/qa/test_brp.py` (stdlib `unittest`, `call` and
   `time.monotonic` stubbed, no game, no network). Seven cases: press sent and result returned; the
   TASK-022 overlap (5000 ms re-sent at 4.8 s) refused before any RPC; another key independent; press
   accepted after hold + margin; delayed reply; JSON-RPC error reply leaves the key free for a retry;
   transport failure blocks the key. `crossing_run.rs` stays as the separate collision gate.
3. **Minor, a failed RPC poisons the key.** Verified for the JSON-RPC error case. Now: `RuntimeError`
   from `call` (the game answered with an error) reserves nothing, a retry goes through. Any other
   exception (timeout, connection, bad JSON) marks the key `math.inf`: the press may still land with a
   release timer nobody can see or cancel (`TimedKeyRelease` is not reflected), so every later press of
   that key raises until a new `Game` session. That is the "invalidate the session" option of the review,
   scoped to the affected keys.

Missing coverage from the review (success / overlap / delayed reply / failed call + retry, including
the ambiguous timeout) is all in `test_brp.py`.

## Flip-RED (test_brp.py)

| Perturbation | Result | Evidence |
|---|---|---|
| Guard removed (`if busy:` -> `if False:`) | RED: overlap, delayed reply, transport failure fail (3) | `scratch/fix_flip_red_guard.txt` |
| Implementer's pre-fix `brp.py` (HEAD, reservation before the call) | RED: delayed reply, transport failure fail, rpc-error retry errors (3) | `scratch/fix_flip_red_prefix.txt` |
| Restored | GREEN, 7 passed | `scratch/fix_test_brp_restored.txt` |

## Skipped

- Nothing from the review. Its "use a protocol mechanism that tracks the game-side release" option is
  not available: `TimedKeyRelease` in bevy_brp_extras 0.22.6 is a plain `#[derive(Component)]`, not
  reflected (checked in the pinned source, `bevy_brp_extras-0.22.6/src/keyboard/keys.rs:26`).

## Test results

- `python -m unittest tools/qa/test_brp.py -v`: `Ran 7 tests ... OK` (`scratch/fix_test_brp_green.txt`).
- `python tools/qa/scenarios/t1.py --out scratch/fix_t1`: exit 0, `"shutdown": "passed"` (`scratch/fix_t1_stdout.txt`).
- `python tools/qa/scenarios/t8.py --out scratch/fix_t8`: exit 0, `"log_errors": []` (`scratch/fix_t8_stdout.txt`).
- `cargo test -p gta_sim --test crossing_run`: 1 passed (unchanged gate, rerun as a sanity check).
- No Rust file changed in this stage, so clippy and the full `cargo test -p gta_sim -p citygen` /
  `-p gta_like --bin gta_like` results from the implementer and reviewer still stand; not rerun.
- No game process left running (`tasklist` shows no `gta_like`).

`git status --short`: `M tools/qa/brp.py`, `?? tools/qa/test_brp.py`, `?? FIX_SUMMARY.md` (task dir).
Scratch evidence lives in the task's `scratch/`.
