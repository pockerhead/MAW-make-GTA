# Domain: game-design
# NORMATIVE when active — a constraint to satisfy, not a claim for you to audit.
# Covers design research, the design document, and any task that decides what the game contains.

## Invariants

- **The owner decides scope; agents inform the decision.** A mechanic, feature or UI element enters the
  game only through the design document the owner approved (`docs/design/`, status line `APPROVED`).
  Before approval everything is a proposal. Research output names options with trade-offs and a
  recommendation; it never presents a choice as already made.
  Delegation (owner, 2026-09-23): for the autonomous build run the owner delegated design approval,
  answers to `## Open questions`, and task decomposition to the orchestrating session. Its `APPROVED`
  stamp and its answers under `### Resolved questions` carry the owner's authority. The owner's brief:
  an open, procedural, varied world; physics; shooting; melee; third-person camera; police, gangs and
  civilians; buildings, parks and streets — "roughly like GTA", ending in a good playable prototype.
- **Open questions go to the owner, not into assumptions.** When a design answer is missing, put it
  under `## Open questions` of your artifact (the pipeline relays it to the owner). Group questions,
  give each 2-4 concrete options with the consequence of each, and a recommended default. Do not ask
  what can be derived from the owner's earlier answers or from the design doc. Questions for the owner
  and design documents are written in Russian (the owner's language); code identifiers stay English.
- **Every mechanic is scoped by what a small AI-driven team can ship in Bevy.** For each proposed
  system state: what it needs from the engine/ecosystem (and whether a maintained crate for the pinned
  Bevy version exists), the content cost (models, animations, audio, level art — the owner has no art
  team), and the smallest playable version. Prefer mechanics that are systemic and reuse content over
  ones that need bespoke content per instance.
- **"Genre reference" is evidence, not law.** Cite how GTA III / Vice City / San Andreas / IV / V (or
  other games) solve a problem as a reference point with a source; "GTA does X" is never by itself a
  reason the game must do X.
- **Design → tasks.** A design document that is meant to drive work ends with a decomposition into
  vertical slices: each slice is playable on its own, names its acceptance by what the owner can do in
  the running game, and lists its dependencies. The first slice is the smallest thing that is fun to
  touch (typically: a character moving under a third-person camera in a test level).
- Feel (controls, camera, vehicle handling) is specified by target values with named units (m/s,
  seconds to max speed, camera distance/lag) that go into data files, and is accepted by the owner's
  run — not by a test.

## Risk lessons

(none yet — this project has no design history)

## Pointers

- `docs/design/GDD.md` — the APPROVED design document (scope law; §12 workspace/plugin map, §13 slices).
- `docs/narrative-graph.md` — current project vector.
- `.claude/local/donor.md` — local-only pointers to the owner's earlier thinking on third-person camera and controls (taste, not requirements).
