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

- **Presentation gates live in the client crate.** A gate over presentation ECS state (`Mesh3d`,
  materials, `VisibilityRange`) runs in a headless `App` inside `cargo test -p gta_like`, built from the
  production presentation plugin plus `init_asset` stand-ins for render assets. It can never live in
  `gta_sim` (no `bevy_render` there by law). Counting sim-side entities proves nothing about meshes (TASK-004).

## Risk lessons

- 2026-09-23 (TASK-002) — a headless test that drives `app.update()` by hand calls `app.finish();
  app.cleanup();` first (plugins such as avian 0.7 register resources in `Plugin::finish`; `App::run` does
  this, manual updates do not). Under `TimeUpdateStrategy::FixedTimesteps(n)` the FIRST `update()` runs 0
  fixed ticks — count ticks via `Time<Fixed>`, never by counting updates.

- 2026-09-23 (inherited from the owner's previous project, 60-entry calibration ledger) — half of all review findings were
  about the checking apparatus, not the game, and 5 of 6 concrete "fix the gate this way" prescriptions
  were wrong while their diagnoses were right. Act on a reviewer's diagnosis; recompute its prescription.

- 2026-09-23 (TASK-004) — to track one file inside an ignored directory, ignore the directory's
  CHILDREN (`/assets/third_party/*`) and negate the file (`!/assets/third_party/manifest.ron`); ignoring the
  directory itself silently ignores the negated file too. Verify ignore rules with `git add -A --dry-run` /
  `git check-ignore -v`, never by reading them.

- 2026-09-23 (TASK-006) — a fixture gate asserting "the error mentions KEYWORD" is only as strong as the
  keyword is unique (`"page"` matched `/media/pages/` in a Kenney URL). Flip each fixture with a sabotage
  that yields a DIFFERENT error, not only "no error". A clamp gate driven by steps that divide the distance
  exactly never overshoots, so removing the clamp stays GREEN — include one off-grid start value.

- 2026-09-23 (TASK-007) — a config-sabotage fixture sits strictly on the failing side of a rule, never on
  its boundary (1.6 + 0.2 = 1.8000001 passed in f32). A hitbox child inside its own capsule is never the
  closest ray hit; spatial queries name their `SpatialQueryFilter` mask explicitly (hitbox sensors exist).
- 2026-09-23 (TASK-007) — `bevy_brp_extras` screenshot publishes the PNG before its capture flag clears:
  space consecutive screenshots >= 0.15 s. A visual shorter than capture latency (a 60 ms tracer) is gated
  by its mechanism (entity count around the capture), its look by the owner run.

- 2026-09-23 (TASK-008) — bevy_animation 0.19.1 `AnimationPlayer::all_paused()` is true on an EMPTY player:
  a pause gate over it is a tautology. Assert `animation(node).is_some_and(|a| a.is_paused())` on a node the
  test started and asserted active.

- 2026-09-23 (TASK-009) — a headless client test CAN load real Kenney GLBs and prove a pose: `MinimalPlugins +
  TransformPlugin + AssetPlugin{file_path} + ImagePlugin + MeshPlugin + AnimationPlugin + WorldSerializationPlugin +
  GltfPlugin` + `init_asset::<StandardMaterial>()` (no bevy_render). Assert a joint (`leg-left`) actually rotates;
  structure-only graph tests cannot see the wrong-root clip bug. Precedent: `src/visuals/civilian_gate.rs`.
- 2026-09-23 (TASK-009) — tick-time gates on NPC load: gate the MEAN fixed-tick time only; per-tick max has OS spikes
  of 6-8 ms. Measured: Tnua walker ~3 us/character/tick in the seed-1 city, 20 m avian ray ~0.6 us.
- 2026-09-23 (TASK-009) — phantom red: cargo builds of `gta_sim`/`citygen` from two trees sharing `target/` get the
  same metadata hash, and one tree can link the other's rlib ("could not find `civilian` in `gta_sim`", a failure
  in untouched code). Before trusting red from a shared target, `touch crates/*/src/lib.rs` and rebuild.

- 2026-09-24 (TASK-022) — density/count gates over one deterministic walk are phase-sensitive (in-view count
  cycles 0..21 per cross street; 20 s windows 3.3..8.6 for the same code). Gate every sliding window plus the
  mean, start after the initial wave is gone, report the window spread next to the mean.
- 2026-09-24 (TASK-022) — pose gates sample every update and assert the RANGE of a joint, never two snapshots:
  a swinging leg passes the same angle on both sides of an extreme, and asset-load latency (3..11 updates)
  randomizes the phase (`civilian_gate` was RED ~35 % of runs while summaries said "38 passed" from one run).
  A stage that reports a new or touched presentation gate runs it at least 3 times.

- 2026-09-24 (TASK-010) — headless GLB harness without `PbrPlugin` spawns glTF meshes with NO
  `MeshMaterial3d<StandardMaterial>` (bevy_pbr makes them in its glTF extension handler); tint/material gates need the
  `StandardMaterialStandIn` handler from `src/visuals/civilian_gate.rs`.
- 2026-09-24 (TASK-010) — glTF instances re-instance on `AssetEvent<WorldAsset>::Modified` (several events while
  scenes stream in; a 3-frame debounce). Joint-parented children (held gun) vanish and come back. Gates over such
  children wait for quiet updates first; to force a re-instance use `Assets::get_mut` + a real `DerefMut`.
- 2026-09-24 (TASK-010) — 30 FPS in QA on this host is the only monitor `\.\DISPLAY9` at 30 Hz under Fifo (recurred
  after TASK-002). Read FPS only through `Game.frame_report()` (refresh + present mode + no-vsync frame cost).

- 2026-09-24 (TASK-011) — avian3d 0.7 requires `Position -> Transform` and copies `GlobalTransform` into
  `Position` every step: a test fixture spawned with only `Position` snaps to the origin (cop gates passed
  vacuously). A posed fixture carries a matching `Transform`; assert its position after one tick.
- 2026-09-24 (TASK-011) — gates that exercise only the first row of a table (heat <= 50 → 1 star) leave rows 2..N
  untested: a flip to `rows[0]` stayed green. Table-driven rules get one case per row.

- 2026-09-24 (TASK-012) — test-floor fixtures collide with `world/test_area.rs` (4x5x4 box, ramp, wall) more
  often than planners expect: check each fixture point against it; a player placed inside a building sinks
  (plaza fixture put the player inside the tower → fake "navmesh" data). Add `GATE BROKEN` on a displaced fixture.
- 2026-09-24 (TASK-012) — `WantedLevel.stars` is recomputed only in `track_search`: after `set_heat` run one tick
  (`raise_heat` helper) before spawning anything that reads stars.
- 2026-09-24 (TASK-012) — runtime "stuck"/"arrived" trackers use physical distance, never FSM state (t11 counted a
  cop in Attack at 30 m as arrived).

- 2026-09-24 (TASK-025) — leak gates diff ALL entities (`World::iter_entities`) minus `IsResource` (in Bevy 0.19
  resources are entities); a component query missed the Tnua sensor orphans. "New city" is `Paused -> Loading`:
  pause first, a direct `Playing -> Loading` keeps the old city alive and fails for the wrong reason.

- 2026-09-24 (TASK-014) — judge a short-lived screen effect (vignette) by a pixel delta (edge band vs a no-hit
  frame, same pose, two scenes) with the intensity read at capture time, never by one screenshot.
- 2026-09-24 (TASK-014) — `CityLandmarks.park_center` sits on the SMG pickup: a teleport there arms the player.

- 2026-09-25 (TASK-015) — "A iterated before B" gates: bevy_ecs 0.19.1 query order is neither spawn nor table
  creation order (component-index HashMap); pin the precondition with a QueryState created before B's table and
  assert the order (`GATE BROKEN`). A latch flip needs a weapon whose mode reads the latch (SMG, not the
  semi-auto pistol). Runtime scenarios never hard-code enum lists: derive them from source/data (t13 broke on a new
  SoundClass).

- 2026-09-25 (TASK-026) — test graph `graph_app(10)` corner (10,0,10) sits on the test-area 1 m box: fleeing
  civilians jam there and a stuck fleer never ends its flight. Delayed-call fixtures use dead-end runs east of x=20.

## Pointers

- `.claude/local/donor.md` — local-only pointers to the owner's previous Bevy project (may be absent on a fresh clone).
