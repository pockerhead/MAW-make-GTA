# FIX_SUMMARY — TASK-017 fixer round 3 (script-only)

Round 2 is archived as `FIX_SUMMARY.prev-2.md` (it already existed; no `FIX_SUMMARY.md` was present to archive).
No game code changed. Files: `tools/qa/osinput.py`, `tools/qa/scenarios/t16_s1.py`.

## Fixed
- B-1 (P10 flake):
  - `osinput.focus()`: returns at once when the window already holds the foreground; the Alt tap only runs when it has to take focus.
  - P10 re-queries the "Настройки" button after the focus step, right before the click. It also re-queries the invert-Y toggle before the second click (with a settle), because the row can be rebuilt after a change.
  - Each OS input step records whether the game window held the foreground (`P10_settings.foreground`). If it did not, the step fails loudly with that reason instead of clicking blind.
  - On any P10 failure (Esc not taken, settings click not taken, toggle not taken, focus lost), `failure_state` gets GameState, PauseMenu, the UI texts and `p10_failure.png`. There is no retry.
- B-2: stdout summary uses `ensure_ascii=True` (summary.json stays UTF-8).

## Diagnosis from the intermediate runs (evidence, target/qa/fix3_rep*/)
- Batch 1 (focus fix + button re-query + ensure_ascii only): 3/5. The failures were a toggle click not taken and "Paused not reached" (a lost Esc).
- Batch 2 (+ foreground and state dump): 3/5.
  - run3: the window lost the foreground between the settings click and the toggle, and `focus()` did not get it back. The OS clicks went to another window. Who took the focus is not identified.
  - run1: the foreground was held, and the first toggle click flipped `invert_y` but the second one, sent 0.45 s later, was lost.
  - Also: the `QA_SETTINGS_ID` settings dir is shared between runs, so a failed run leaves `invert_y` flipped for the next one. The assertion is relative to `before`, so this does no harm.
- Batch 3 (+ toggle re-query and settle, loud foreground check): 5/5.

Residual risk: the focus theft is external and unexplained. If it comes back, it now fails with "game window lost the foreground before X" plus a screenshot, not a silent lost click. This is a TASK-031 watch item.

## Skipped
- None of B-1/B-2. There is no blind retry (orchestrator note).

## Test results
`python tools/qa/repeat.py t16_s1 --runs 5 --out target/qa/fix3_rep3`
```
t16_s1 run 1/5: pass (48 s)
t16_s1 run 2/5: pass (48 s)
t16_s1 run 3/5: pass (49 s)
t16_s1 run 4/5: pass (47 s)
t16_s1 run 5/5: pass (48 s)
t16_s1: 5/5 passed
```
