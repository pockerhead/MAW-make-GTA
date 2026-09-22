# Stage: plan-reviewer-2

LSP-first for Rust (`LSP` tool → rust-analyzer): `documentSymbol` / `workspaceSymbol` for orientation, `goToDefinition` / `hover` for every symbol you name, `findReferences` before any rename or removal. Grep for non-symbol strings (`.md`, `.ron`, `.wgsl`, comments) or as a fallback. Verify the plan's blast radius with `findReferences` / `incomingCalls` on every symbol it changes.

Bevy API check: every Bevy / ecosystem symbol you rely on is confirmed in the pinned source (`~/.cargo/registry/src/*/<crate>-<version>/`, version from `Cargo.lock`) or docs.rs for that exact version — not from memory. A plan that cites a Bevy API or crate version you have not confirmed in the pinned source is a major finding — Bevy APIs from a different version are the most likely hallucination in this project.

Check the data-first invariant: every new tuning number in the plan has a named data file; a new `const` for a tuning value is a block.

Math-heavy plan → walk 3 worked directional examples before PASS. Thematic skills: `m05-type-driven`, `m07-concurrency`, `m10-performance`, `game-feel` for feel tasks.

A reviewer's prescription for fixing a gate is the least reliable thing a review produces — recompute it with numbers before writing it into the plan.

Sub-agent discipline: the harness launches every `Agent` call asynchronously and the report arrives later as a hand-back message. Count your launches and do not end your turn until every one has reported; state `children: N launched / N reported` before your final hand-back. A `Bash` command whose result your deliverable needs runs in the FOREGROUND — never `run_in_background=true` for it, and never end your turn waiting on a background run.
