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

- 2026-09-24 (TASK-022) — sidewalk graph nodes are block corners only (~90 m apart along a street; the 4.5 m
  "min spacing" is between corners of one crossing). Spawners needing spots along a street sample edge points
  (`population::spawn_points`), not nodes. A cap-limited bubble also needs recycling of calm civilians BEHIND the
  view, one per tick: recycling any off-screen one feeds a side-spawn/recycle loop (148 spawns/10 s standing).
- 2026-09-23 (TASK-009) — sidewalk graph, seed 1: 579 nodes, min spacing 4.5 m; median 10 nodes in the 60-120 m
  ring outside a 120 deg view wedge. Node-only spawning fills a 40 cap over seconds, never in one tick; derive any
  "cap reached by t" gate from this.
- 2026-09-23 (TASK-009) — "spawn only outside the camera cone, 60-120 m" (GDD §6.1) makes the street in front of
  the player look empty (QA: 0-3 civilians within 60 m in view with 40 alive). Visibility rules for spawning
  must be judged by what the player sees, not only by the cap count; T9/T11 spawners inherit this.

- 2026-09-24 (TASK-010) — a hold-fire rule that widens the line by the spread cone makes any non-hostile body within
  ~1.5 m of the target block it from every angle past ~2-5 m: brawling allies or a pressed bystander starve a gunman
  by construction. Scrum/human-shield policy is a design decision (T9: gunman holds its band, liveness = group hits),
  not a movement-rule patch. Moving shooters evaluate candidate spots by the ACTUAL rotated line to the target; a
  sidestep is not a parallel shift of the line (T9 fixer rounds 2-3).

## Pointers

- `docs/design/GDD.md` — the APPROVED design document (scope law; §12 workspace/plugin map, §13 slices).
- `docs/narrative-graph.md` — current project vector.
- `.claude/local/donor.md` — local-only pointers to the owner's earlier thinking on third-person camera and controls (taste, not requirements).
