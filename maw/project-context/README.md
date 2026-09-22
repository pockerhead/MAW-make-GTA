## Orientation

Working title "GTA-like": a single-player third-person action game in the spirit of the GTA series,
built on **Rust + Bevy** (the whole engine: ECS, rendering, audio, input — there is NO Godot and no
other engine in this project). Experimental project, driven by MAW agents; the owner is the only human.

Stage of the project: **pre-design.** There is no code yet and no approved design document. Game scope
(open world or not, vehicles, combat, missions, UI/UX) is being collected into `docs/design/` by the
first research task. Until a design doc is marked APPROVED by the owner, no gameplay feature is law —
do not invent scope, and do not treat a mechanic as required because "GTA has it".

Architectural law nobody breaks: **gameplay logic runs and is testable in a headless Bevy `App`**
(no window, no GPU, no renderer — `MinimalPlugins` + the game's own plugins). Rendering, camera
visuals, audio and UI are downstream consumers of gameplay state, never its owner.

Planning artifacts: `maw/ROADMAP.md` = machine-derived dependency graph (do not hand-edit, no prose).
`docs/narrative-graph.md` = hand-curated project vector (North Star, what next and why) — read it for
planning context, never as the canonical graph.

## Universal invariants

- **Bevy API is verified against the pinned version, never recalled.** Bevy breaks its API every
  minor release (0.17 split `Event` into buffered `Message` + observer `Event`; bundles gave way to
  required components; many renames). Your training data mixes versions. Before planning or writing
  any Bevy / ecosystem-crate call, check it in the exact source under
  `~/.cargo/registry/src/*/<crate>-<version>/` (version from `Cargo.lock`) or docs.rs for that version.
  An ecosystem crate (physics, input, UI, character controller) is usable only if its release declares
  support for the Bevy version this workspace pins — check its `Cargo.toml`, not its README.
- Verify against CODE, not comments; trust `cargo check` / `cargo clippy` / `cargo test` output, never
  stale IDE / rust-analyzer diagnostics.
- Headless-first: every gameplay rule (movement intent, damage, wanted level, AI decisions, mission
  state) has a headless test path. `cargo test` stays green. A system that only works with a window is
  presentation, and must not own gameplay state.
- Golden Path: let-else + early return, linear flow, nesting < 2. YAGNI — solve the real problem of
  this task, not a future one; every abstraction answers "what breaks if I delete this?".
- Domain-driven layout: code grouped by game domain (`player/`, `vehicle/`, `combat/`, `camera/`,
  `world/`), NOT by layer (`systems/`, `components/`). One Bevy `Plugin` per domain. File < 750 lines
  = warning, < 950 = hard limit.
- **Data first:** a number the owner may want to turn (speeds, accelerations, camera distances,
  damage, cooldowns, spawn rates, prices) is DATA in `assets/<domain>/*.ron` with a strict loader, not
  a `const`. A LAW (fixed tick rate, unit scale, collision layers, coordinate conventions) stays a
  `const` in code. A new `const` for a tuning value is a review BLOCK. One source per config.
- Surgical changes: every changed line traces to the task; clean up only debris your change created.
  Work IN-PLACE in the checkout you were spawned into — never `git worktree add`, `git clone`, or a
  second `target/` directory.
- Comments: only for a non-trivial algorithm, a hack/workaround, a non-obvious side effect, or an
  invariant the type system cannot express — one short line, no task narrative. `///` for public API.
- Evidence weight is proportional to the COST OF THE ERROR. A defect that breaks SILENTLY (state
  corruption, frame budget, save data, physics tunnelling at speed) earns a real gate. A defect the
  owner sees on the first frame (camera feel, animation, visuals, "floaty" controls) is gated by the
  owner's run: say so in your artifact and mark it for the owner, do not build machinery around it.
  Declining to add a gate is a valid finding.

## Domain catalog
<!-- trigger MUST be zero-knowledge-observable (path glob / literal token), never a concept. -->

- trigger: Rust code with the tokens `Query<` / `Commands` / `Res<` / `ResMut<` / `#[derive(Component` / `#[derive(Resource` / `impl Plugin` / `add_systems` / `Changed<` / `With<` / `Without<` / `MessageWriter` / `MessageReader` / `Observer` / `On<` / `FixedUpdate` / `AsyncComputeTaskPool`, OR a change to a `bevy` dependency line in any `Cargo.toml` → {PCTX}/domains/bevy-ecs.md
- trigger: any file under `crates/*/tests/**` or `tests/**`, OR the tokens `#[test]` / `flip-RED` / `flip_red` in a task's acceptance criteria, OR writing/re-anchoring any gate, scanner, fixture or baseline → {PCTX}/domains/gates.md
- trigger: any file under `docs/design/**`, OR a task whose `Mode:` is `brainstorm` or `deep-research`, OR a task whose text names a game mechanic, UI/UX, controls, camera feel, missions, open world, vehicles, weapons, NPCs, police or economy → {PCTX}/domains/game-design.md

HARD RULE: before you plan or write any part whose work matches a trigger
above — even if this task was not framed as being in that domain — you MUST
Read the mapped module first and treat it as normative. Do not proceed on
that part without it.
