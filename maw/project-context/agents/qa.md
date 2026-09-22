# Stage: qa

Loop:
1. `cargo build` — broken → REJECT, stop.
2. `cargo clippy -- -D warnings` — new warnings in touched code → NEEDS_FIXES.
3. `cargo test` — compare the failure LIST by name against the same run shape on the base commit; zero NEW failures is the bar (a count match can hide a swap).
4. Acceptance criteria that are gameplay rules → confirm each has a headless test that exercises it through the production plugin composition, and flip-RED at least one of them yourself (commit first; restore verified by sha256).
5. Acceptance criteria that are visual / feel / camera / animation / audio → you cannot see the game. Do not fake them with a test. List them as an owner checklist: what to launch, what to do, what they should see.

Cargo discipline:
- Commands: `cargo check`, `cargo build`, `cargo test`, `cargo clippy -- -D warnings`. Trust their output, never IDE diagnostics. A cold Bevy build takes minutes on Windows — give the command a long timeout (10 min) instead of backgrounding it.
- Never run two cargo commands at once. Under the codex sandbox run ONE named target per invocation (`--lib`, `--test <name>`, `--bin <name>`) with `-j 4` (parallel linkers exhausted the sandbox and killed process creation on this machine in another project).
- Format only the files you created or edited (`rustfmt --edition 2024 <file>` — match the edition in `Cargo.toml`); never `cargo fmt` on the whole workspace.
- Before writing your summary run `git status --short` and account for every path outside the task dir; a path you cannot explain is reverted.
- Write your stage report EARLY (first complete version as soon as results exist) and update it — an environment failure must never cost more than the last edit.
- If you launched the windowed game, make sure no game process is left running when you finish.

Verdict: all criteria covered and PASS → SHIP · any owner-only criterion → SHIP-PENDING-RUNTIME with the explicit owner checklist · a covered criterion FAILS → NEEDS_FIXES · build broken → REJECT. "MAW SHIP" is not "runtime SHIP": keep SHIP-PENDING-RUNTIME honest.

Sub-agent discipline: the harness launches every `Agent` call asynchronously and the report arrives later as a hand-back message. Count your launches and do not end your turn until every one has reported; state `children: N launched / N reported` before your final hand-back. A `Bash` command whose result your deliverable needs runs in the FOREGROUND — never `run_in_background=true` for it, and never end your turn waiting on a background run.
