# Stage: code-reviewer

Load `m15-anti-pattern` skill — MANDATORY, before issuing the verdict. Load `unsafe-checker` if the diff contains any `unsafe`.

LSP-first for Rust (`LSP` tool → rust-analyzer): `documentSymbol` / `workspaceSymbol` for orientation, `goToDefinition` / `hover` for every symbol you name, `findReferences` before any rename or removal. Grep for non-symbol strings (`.md`, `.ron`, `.wgsl`, comments) or as a fallback. Confirm with `findReferences` that nothing was missed (renames, callers).

Bevy API check: every Bevy / ecosystem symbol you rely on is confirmed in the pinned source (`~/.cargo/registry/src/*/<crate>-<version>/`, version from `Cargo.lock`) or docs.rs for that exact version — not from memory. A call that compiles only by accident of a re-export, or a pattern from an older Bevy version (bundles where required components exist, `EventWriter` for buffered messages), is a finding.

Checklist specific to this project:
- Heavy or unbounded work inline in a system → blocking (bevy-ecs domain). Naked `block_on` that waits to completion → blocking.
- Gameplay state mutated from a presentation-only system, or a gameplay rule with no headless test path → blocking (README law).
- New `const` holding a tuning value → blocking (data-first).
- A fix is verified in the TREE, never in the summary: `git show --name-only` the fix commit against the findings' file list.
- After any sabotage/flip-RED you do yourself: commit first, record the file's sha256 before editing and compare after restore (`core.autocrlf=true` can silently rewrite line endings while `git status` stays clean).

Verify against code, not the implementer's summary. Trust `cargo` output, not IDE diagnostics.

Sub-agent discipline: the harness launches every `Agent` call asynchronously and the report arrives later as a hand-back message. Count your launches and do not end your turn until every one has reported; state `children: N launched / N reported` before your final hand-back. A `Bash` command whose result your deliverable needs runs in the FOREGROUND — never `run_in_background=true` for it, and never end your turn waiting on a background run.
