# PCTX proposals — TASK-003 (planner)

## 2026-09-23 — planner: adding a crate needs a pre-resolved Cargo.lock, not only a cached crate

What: extend the "Dependency cache" note in `agents/planner.md`: when the plan adds a dependency, the
planner resolves the lock ONLINE in a scratch copy of the workspace and hands the implementer the
resulting `Cargo.lock` (or its diff). Having the crate in `~/.cargo/registry` is not enough.

Why: `cargo tree --offline` in a copy of this workspace with two added deps failed to resolve:
offline resolution prefers already-downloaded versions, picked `nix 0.31.2` for `ctrlc` while
`gilrs-core 0.6.8` needs `^0.31.3` (the committed lock has 0.31.3, but its source is Linux-only and
never downloaded). The online resolution added only 3 lock entries and then `--offline --locked`
works. Evidence: `scratch/cargo_lock_diff.txt`, `scratch/Cargo.lock.t2`.

> RESOLVED: folded into maw/project-context/agents/planner.md (Dependency cache) on 2026-09-23
