# Stage: premise-challenge

Where this project's primary sources live (the ONLY thing this file declares):

- **Code at `file:line`** — the repo itself; implementation over comments. The repo may still be empty: absence of code is a fact to report, not a gap to fill with assumptions.
- **Headless executable repro** — `cargo test` (gameplay runs in a headless Bevy app); for a build claim `cargo build`. Run it yourself; do not trust a quoted result.
- **Engine truth** — the pinned crate source under `~/.cargo/registry/src/*/` (version from `Cargo.lock`) and docs.rs for that exact version; for a crate not yet in the workspace, its `Cargo.toml` on crates.io / its repository.
- **Owner decisions** — `docs/design/` documents marked `APPROVED`, and `### Resolved questions` in the task's own spec.

LSP-first to reach code at `file:line` (`hover`, `goToDefinition`, `findReferences`, `documentSymbol`). Read-only skills only: `rust-symbol-analyzer`, `rust-code-navigator`, `rust-call-graph`. Do NOT load `m01`–`m15` or refactor skills — they are solution-space.
