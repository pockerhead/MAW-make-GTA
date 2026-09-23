# Open decisions — TASK-009

- 2026-09-23: code review NEEDS_WORK (corpse sighting cancels its own witness call) accepted; fixer's variant taken over the review's recipe (corpses are not offered to a civilian in Report, filtered in perception rather than FSM) because perception keeps only the nearest threat and an FSM filter would let the corpse mask a new shot — proven by flip (b).
- 2026-09-23: fixer-found repeat calls (a witness still seeing the body can call again, up to ~7 calls per 30 s corpse) are not fixed here: counting is T10's job. Added an acceptance note to TASK-011 (T10 wanted): one wanted-level contribution per corpse/incident.
- 2026-09-23: fixer-found long cower near a visible corpse (timer refreshed every perception cycle, up to 30 s) goes to the owner-run checklist in QA, not fixed blind: it is a feel question.
- 2026-09-23: integrated client witness-bar gate skipped (bar only reads Report.progress; sim path gated, bar lifecycle gated in witness_gate.rs).
- 2026-09-23: PCTX_PROPOSALS folded: glTF root-name → bevy-ecs; GLB pose gate, mean-tick gate, shared-target phantom → gates; node density, empty-street spawn rule → game-design; brp.py resource cast → agents/qa.md.
- 2026-09-23: QA SHIP-PENDING-RUNTIME accepted as SHIP (no acceptance criterion violated). Empty street in view (QA finding 1) is a GDD §6.1 consequence, fixed in follow-up TASK-022 (small-fix) before TASK-010.
