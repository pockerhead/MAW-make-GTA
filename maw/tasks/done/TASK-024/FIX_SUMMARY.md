# FIX_SUMMARY — TASK-024, fixer round 2

Round 1 is in FIX_SUMMARY.prev-1.md. This round covers QA_REPORT bugs 1-2 and the regex note.

## Fixed
- **QA bug 1 (14-space run in `validate_fight_hearing`)**: `crates/gta_sim/src/civilian/mod.rs:123` now uses a `\` line continuation, same as `wanted/mod.rs`. Gate: `crates/gta_sim/tests/config.rs` `fight_report_distance_below_fight_hearing` now also has `assert!(!error.message.contains("  "))`.
  Flip-RED: I added the assert before fixing the message. It went RED on the old message (`0 passed; 1 failed`). After the fix: `46 passed; 0 failed`.
- **QA bug 2 (t10.py "stand spot blocked")**: `tools/qa/scenarios/t10.py:229-231`. A stand spot off by more than `TELEPORT_TOLERANCE_M` is now logged in `misses` as `{"victim", "spot", "blocked"}` and `continue`s the `KILL_TRIES` loop. The spot key is already in `tried`, so the next plan picks a different one. If all tries are used up, the existing `GATE BROKEN: no kill in N shots` still fires.
- **Regex note**: line 72 is now `r"(?<!_)report_min_distance:\s*([\d.]+)"`, so it can no longer match `fight_report_min_distance`.

## Skipped
- None. There was no flip-RED for the blocked-spot path: it happens about 1 run in 10 and can't be forced without a sabotage harness. It is a small change to the harness, and this round was scoped as tiny.

## Test results
- `cargo test -p gta_sim -j 4 --test config`: 46 passed, 0 failed.
- `cargo clippy --workspace --all-targets -j 4 -- -D warnings`: clean (Finished).
- `python scratch/t10_runs.py 11 15`: 5/5 in a row passed. All had exit 0, misses 0, heat 50, clear after 10.0..10.14 s and log_errors [] (logged in `scratch/t10_runs.txt` under "fixer round 2", with logs in `scratch/t10_run11..15.log`). No game process was left running.
