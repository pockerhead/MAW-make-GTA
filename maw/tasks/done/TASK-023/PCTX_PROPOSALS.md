# PCTX proposals (TASK-023, implementer)

## 2026-09-24 — gates domain, risk lesson
bevy_brp_extras 0.22.6 `send_keys` gives every call its own release timer (`TimedKeyRelease`, not
reflected, cannot be cancelled over BRP). Re-pressing a key before its previous hold ends lets the OLD
timer release it right after the new press: TASK-022 QA "held W" by re-sending 5000 ms every 4.8 s and
filed a phantom "player stalls at a crossing" bug (TASK-023; runtime showed `MoveIntent.axis == 0` at
the stall). Hold once for the whole window (max 60000 ms); `tools/qa/brp.py::send_keys` now refuses
overlapping holds. Before filing a runtime movement stall, sample `MoveIntent` next to `Position`.
