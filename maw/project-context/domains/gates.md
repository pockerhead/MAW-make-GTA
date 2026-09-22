# Domain: gates
# NORMATIVE when active — a constraint to satisfy, not a claim for you to audit.
# Covers the CRAFT of executable gates: headless tests, scanners, fixtures, flip-RED discipline.
# Not "what to test" (that is the task) — "how a gate stays honest".

## Invariants

- **Flip-RED or it is not a gate.** Every new or re-anchored gate is shown to fail: break the
  mechanism under test (an input, the kernel, a threshold, the fixture), observe RED, restore, observe
  GREEN, and record which input was perturbed in the stage summary. A sabotage hook that edits the
  verdict or a value the same function just wrote proves nothing.
- **A gate fails on the CODE, never on its own plumbing.** A missing asset or output directory is not
  the property under test; resolve inputs through a helper that fails with a message naming the GATE
  as broken.
- **A parity gate that feeds both sides identical inputs is a tautology.** So is an identity between
  two numbers that both move with the defect. A falsifiable gate compares against a quantity that stays
  fixed while the defect moves the other one.
- **Test numbers are derived, not intended.** Every expected value in a test plan comes from a worked
  example through the real code path (which branch, which counter, which order), not from the design.
- **Gameplay tests run in the headless app** built by the same composition function as the game, with
  a controlled clock (advance `Time` / run `FixedUpdate` a known number of times). A test that
  constructs systems outside the production plugin composition says nothing about the game.
- A text fixture read via `include_str!` is line-ending independent: match per line (`lines()` +
  `trim()`) or normalise `\r\n` first — this checkout has `core.autocrlf=true`.
- Say which class a gate is: liveness ("the feature ran") is not correctness ("the output is right").
  Name the gate carrying each claim separately.

## Risk lessons

- 2026-09-23 (inherited from the owner's previous project, 60-entry calibration ledger) — half of all review findings were
  about the checking apparatus, not the game, and 5 of 6 concrete "fix the gate this way" prescriptions
  were wrong while their diagnoses were right. Act on a reviewer's diagnosis; recompute its prescription.

## Pointers

- `.claude/local/donor.md` — local-only pointers to the owner's previous Bevy project (may be absent on a fresh clone).
