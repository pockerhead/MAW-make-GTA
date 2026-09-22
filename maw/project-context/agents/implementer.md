# Stage: implementer

LSP-first for Rust (`LSP` tool → rust-analyzer): `documentSymbol` / `workspaceSymbol` for orientation, `goToDefinition` / `hover` for every symbol you name, `findReferences` before any rename or removal. Grep for non-symbol strings (`.md`, `.ron`, `.wgsl`, comments) or as a fallback.

Bevy API check: every Bevy / ecosystem symbol you rely on is confirmed in the pinned source (`~/.cargo/registry/src/*/<crate>-<version>/`, version from `Cargo.lock`) or docs.rs for that exact version — not from memory. A mechanical mismatch with the plan (renamed function, moved module, changed signature between Bevy versions) with exactly one obvious local adaptation is NOT a block: adapt and record it under `## Deviations from plan` with file:line. PLAN_BLOCKED is for design contradictions — report all of them at once, after a full pre-flight of the whole plan.

Cargo discipline:
- Commands: `cargo check`, `cargo build`, `cargo test`, `cargo clippy -- -D warnings`. Trust their output, never IDE diagnostics. A cold Bevy build takes minutes on Windows — give the command a long timeout (10 min) instead of backgrounding it.
- Never run two cargo commands at once. Under the codex sandbox run ONE named target per invocation (`--lib`, `--test <name>`, `--bin <name>`) with `-j 4` (parallel linkers exhausted the sandbox and killed process creation on this machine in another project).
- Format only the files you created or edited (`rustfmt --edition 2024 <file>` — match the edition in `Cargo.toml`); never `cargo fmt` on the whole workspace.
- Before writing your summary run `git status --short` and account for every path outside the task dir; a path you cannot explain is reverted.
- Write your stage report EARLY (first complete version as soon as results exist) and update it — an environment failure must never cost more than the last edit.
- If you launched the windowed game, make sure no game process is left running when you finish.

New tuning values go into `assets/<domain>/*.ron` through the domain's loader, never as a `const` (data-first invariant).

Skills: `game-feel` when tuning controls / camera / vehicle constants; `rust-refactor-helper` for renames, extracts, splitting files over 750 lines; `m06-error-handling` for loader/validation errors.

Sub-agent discipline: the harness launches every `Agent` call asynchronously and the report arrives later as a hand-back message. Count your launches and do not end your turn until every one has reported; state `children: N launched / N reported` before your final hand-back. A `Bash` command whose result your deliverable needs runs in the FOREGROUND — never `run_in_background=true` for it, and never end your turn waiting on a background run.
