# Stage: fixer

LSP-first for Rust (`LSP` tool → rust-analyzer): `documentSymbol` / `workspaceSymbol` for orientation, `goToDefinition` / `hover` for every symbol you name, `findReferences` before any rename or removal. Grep for non-symbol strings (`.md`, `.ron`, `.wgsl`, comments) or as a fallback.

Fix only what the review verifiably flagged — do not rewrite correct code "to be safe". Uncertain → document the concern, do not blind-edit. A review's diagnosis is usually right and its prescription often wrong: recompute the prescription against the code before applying it.

Bevy API check: every Bevy / ecosystem symbol you rely on is confirmed in the pinned source (`~/.cargo/registry/src/*/<crate>-<version>/`, version from `Cargo.lock`) or docs.rs for that exact version — not from memory.

Every re-anchored test or gate: flip-RED observed and recorded in FIX_SUMMARY, no exceptions.

Cargo discipline:
- Commands: `cargo check`, `cargo build`, `cargo test`, `cargo clippy -- -D warnings`. Trust their output, never IDE diagnostics. A cold Bevy build takes minutes on Windows — give the command a long timeout (10 min) instead of backgrounding it.
- Never run two cargo commands at once. Under the codex sandbox run ONE named target per invocation (`--lib`, `--test <name>`, `--bin <name>`) with `-j 4` (parallel linkers exhausted the sandbox and killed process creation on this machine in another project).
- Format only the files you created or edited (`rustfmt --edition 2024 <file>` — match the edition in `Cargo.toml`); never `cargo fmt` on the whole workspace.
- Before writing your summary run `git status --short` and account for every path outside the task dir; a path you cannot explain is reverted.
- Write your stage report EARLY (first complete version as soon as results exist) and update it — an environment failure must never cost more than the last edit.
- If you launched the windowed game, make sure no game process is left running when you finish.

Skill `game-feel` for feel-round fixes: it maps the owner's words ("ватный", "дёргается", "плывёт") to the attack/release/lag constant that has to move.

Runtime self-check: once the game has a window, you may launch the `--features dev` build and drive it via `tools/qa/brp.py` (Bevy Remote Protocol: screenshot, keys, diagnostics) to confirm your change visibly works before handing off; artifacts go to the task's `scratch/`. Always shut the game down afterwards.

Write only your own stage artifact. `QA_REPORT.md`, `IMPL_REVIEW.md` and plan files belong to other stages (TASK-002: an implementer wrote QA_REPORT.md); an owner checklist goes into your own summary.

Sub-agent discipline: the harness launches every `Agent` call asynchronously and the report arrives later as a hand-back message. Count your launches and do not end your turn until every one has reported; state `children: N launched / N reported` before your final hand-back. A `Bash` command whose result your deliverable needs runs in the FOREGROUND — never `run_in_background=true` for it, and never end your turn waiting on a background run.
