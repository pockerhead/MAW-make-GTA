# Stage: planner

LSP-first for Rust (`LSP` tool → rust-analyzer): `documentSymbol` / `workspaceSymbol` for orientation, `goToDefinition` / `hover` for every symbol you name, `findReferences` before any rename or removal. Grep for non-symbol strings (`.md`, `.ron`, `.wgsl`, comments) or as a fallback.

Bevy API check: every Bevy / ecosystem symbol you rely on is confirmed in the pinned source (`~/.cargo/registry/src/*/<crate>-<version>/`, version from `Cargo.lock`) or docs.rs for that exact version — not from memory. For a task that adds an ecosystem crate, the plan names the crate version and quotes the line of its `Cargo.toml` that declares the supported `bevy` version.

Thematic skills (load when the task touches the area):
- `game-feel` — controls, camera, vehicle handling, "floaty vs tight": plan feel values in named units against this vocabulary; the owner's run is the final oracle.
- `game-designer` — feedback and readability (hit reactions, telegraphing, juice).
- `m05-type-driven` — state machines, typestate; `m07-concurrency` — async tasks, Send/Sync, parallel systems; `m10-performance` — frame budget, archetypes.
- `m11-ecosystem` / `rust-learner` — crate choice and versions.

Math-heavy plan (camera rigs, vehicle physics, coordinate transforms) → walk 3 worked directional examples before sealing; sign errors pass magnitude tests.

Every tuning number the plan introduces names its data file (`assets/<domain>/*.ron`), per the data-first invariant.

Sub-agent discipline: the harness launches every `Agent` call asynchronously and the report arrives later as a hand-back message. Count your launches and do not end your turn until every one has reported; state `children: N launched / N reported` before your final hand-back. A `Bash` command whose result your deliverable needs runs in the FOREGROUND — never `run_in_background=true` for it, and never end your turn waiting on a background run.
