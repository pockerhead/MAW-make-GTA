# FIX_SUMMARY — TASK-009

Preflight: `scratch/` read as a coverage map (flip_red.log/py, probes, t8_run1, look_around). Review read from
`IMPL_REVIEW.md` (one Major issue, two missing-coverage notes).

Riskiest prescription checked first: "Distinguish a newly perceived threat from the one that started the call"
implemented the literal way (the FSM's `Report + Some(Corpse)` branch ignores the corpse) would break correct code:
`perceive` keeps only the NEAREST threat, so a body nearer than a new gunshot masks the gunshot and the call is no
longer interrupted by a real scare. Verified with flip (b) below: that prescription makes the new interrupt gate RED.

## Fixed

1. **Major: a corpse in sight cancels its own witness call** — confirmed in code. `perceive`
   (`crates/gta_sim/src/perception/mod.rs`) offered the nearest visible corpse on every slot tick; `next_state`
   (`crates/gta_sim/src/civilian/mod.rs`) maps `Report + Some(threat)` to `react(.., allow_report = false)`, so the
   same stationary body turned the call into Flee/Cower one perception cycle (4 ticks) later.
   Fix (perception side, 3 lines): a civilian in `Report` is not offered corpses at all. Sounds, aimed guns and hits
   still reach it and still interrupt the call (GDD §6.2: "убить или напугать"). Doing it in `perceive` instead of the
   FSM keeps a nearer body from masking a new gunshot, and skips the LOS ray for callers.
2. **Missing coverage: persistent corpse through a whole call** — new gates in `crates/gta_sim/tests/civilians.rs`
   (production composition, `corpse_witness_app` helper: witness at 6 m from a body it saw die, temperament of reaction
   row 6, preconditions derived through `choose_reaction` and the shipped radii, "GATE BROKEN" otherwise):
   - `corpse_in_sight_does_not_cancel_its_own_call`: progress is exactly `k/256` for every tick 1..255 with the body in
     sight, calm at tick 256.
   - `a_shot_still_interrupts_a_corpse_call`: 24 ticks into the call, a shot into the air (muzzle farther than the
     body, inside hearing) → Flee/Cower within one perception cycle.

   Flip-RED (each restored, GREEN after):
   - (a) fix removed (`reporting = false`): both new tests RED (`civilians.rs:424`, `:444`).
   - (b) review's literal prescription instead (FSM keeps Report on a `Corpse` threat, perception unchanged):
     `a_shot_still_interrupts_a_corpse_call` RED (the nearer body masks the shot).
   - (c) Report never interrupted by any threat: `a_shot_still_interrupts_a_corpse_call` RED.

## Skipped

- **Missing coverage: integrated witness-bar check driven by a real sim `Report`.** The sim side (Report stays alive
  and completes with a body in sight) is now gated in `gta_sim`; the bar only reads `Report.progress`, and its
  lifecycle is gated in `src/hud/witness_gate.rs`. An extra client gate would re-test the same sim path through the UI.
  Bar look/placement stays an owner-run item.

## Concerns (not fixed, not in the review; for the orchestrator/owner)

- After a completed call the witness returns to `Wander`; if the body is still in sight within `corpse_sight`, the
  next perception cycle scores it again and a report-prone civilian may call again (up to ~7 calls over the 30 s corpse
  life). Harmless in T8 (no heat yet); T10 must count one call per body/incident or this needs per-civilian memory.
- A civilian that chose `Cower` near a visible body gets its crouch timer refreshed every perception cycle, so it may
  crouch until the body despawns (30 s). Owner-visible feel item; left as is (commitment rule of the plan).

## Test results

- `cargo test -p gta_sim -j 4`: all green — lib 29, anim_state 4, asset_manifest 3, city 6 (+1 ignored, pre-existing),
  civilian_bench 1, civilian_city 6, civilians 10 (8 old + 2 new), config 25, health 6, jump 3, melee 19, movement 4,
  respawn 5, shooting 16, terrain 2.
- `cargo test -p gta_like --bin gta_like -j 4`: 38 passed.
- `cargo clippy -p gta_sim --tests -j 4 -- -D warnings`: clean. `cargo clippy -j 4 -- -D warnings`: clean.
- `rustfmt --edition 2024` on the two edited files; `git status --short` shows only
  `crates/gta_sim/src/perception/mod.rs` and `crates/gta_sim/tests/civilians.rs` outside the task dir.
