# PCTX proposals — TASK-004 (planner)

## 2026-09-23 — planner: third-party asset files must be stored byte-exact by git

What: a rule for the gates domain (or a future `content` domain, trigger `assets/third_party/**`):
every committed third-party pack directory is marked `binary` in `.gitattributes`
(`assets/third_party/*/** binary`), and `.gitignore` re-includes it (`!/assets/third_party/**`),
because the SHA-256 manifest gate hashes the working-tree bytes.

Why: Kenney `License.txt` is CRLF + CP1252 (bytes `\t\r\n`, `0x95` bullets; verified in
`scratch/third_party/city-kit-roads/License.txt`). With the repo's `* text=auto eol=lf` it would be
normalised to LF on commit, so the manifest hash passes on this `core.autocrlf=true` machine (checkout
restores CRLF) and fails on any other clone. Also `fs::read_to_string` on it fails (not UTF-8): read bytes.
The root `.gitignore` ignored `*.glb`/`*.png`, contradicting GDD §9.2 "GLB and License.txt are committed".
Probe: `scratch/gitprobe/` (`git ls-files --eol` shows `i/crlf attr/-text`, `assets/x/a.png` still ignored).

## 2026-09-23 — planner: presentation gates live in `cargo test -p gta_like`

What: a line for the gates domain: a gate over presentation ECS state (`Mesh3d`, materials, visibility
ranges) runs in a headless `App` inside the client crate (`cargo test -p gta_like`), built from the
production presentation plugin plus `init_asset` stand-ins for the render plugins. It can never live in
`gta_sim` (no `bevy_render` there by law).

Why: the T3 mesh-merge criterion counts `Mesh3d` entities; the premise challenge showed that counting
sim-side chunk entities proves nothing. The GDD §13 wording "Headless = `-p gta_sim` or `-p citygen`"
predates this case.

## 2026-09-23 — planner: kenney.nl TLS is flaky from this host

What: environment fact for planner/implementer overlays: `curl` (schannel) to kenney.nl failed once with
`SEC_E`-style handshake error and succeeded on retry; Python `urllib` worked first time. Asset tooling
must retry and accept a local zip cache.

## 2026-09-23 — plan-reviewer-2 (TASK-004)

Proposal (gates / git hygiene lesson): to track one file inside an otherwise ignored asset directory,
ignore the directory's CHILDREN (`/assets/third_party/*`) and negate the file (`!/assets/third_party/manifest.ron`).
Ignoring the directory itself (`/assets/third_party/`) silently ignores the negated file too: git cannot re-include
a file whose parent directory is excluded. TASK_FINAL's owner override literally said `/assets/third_party/`;
a probe (`scratch/r2_gitprobe/`) showed the manifest would never be committed. Verify ignore rules with
`git add -A --dry-run` / `git check-ignore -v`, not by reading them.

## 2026-09-23 — implementer (TASK-004): `rustfmt <file>` is recursive over `mod` children
The implementer rule "Format only the files you created or edited (`rustfmt --edition 2024 <file>`)"
is not enough: rustfmt on a crate root / `mod.rs` / `lib.rs` also formats every child module it
declares. Running it on `crates/citygen/src/lib.rs` reformatted the untouched `roads.rs` (one long
closure). Proposal: add "rustfmt follows `mod` declarations — after formatting, `git diff --stat` and
revert any file you did not edit (or pass `--config skip_children=true` on nightly)".

## 2026-09-23 (qa, TASK-004) — `brp.py --help` is not a help command
The qa stage context says "Preferred driver: `python tools/qa/brp.py --help` (launch, wait-ready, ...)".
`tools/qa/brp.py` has no argparse: `__main__` builds and launches the game and prints diagnostics, so
`--help` starts a windowed game. Proposal: either describe brp.py as a library (`from brp import Game`)
with the method list, or add a real `--help`. Why: an agent following the context launches the game by accident.

> RESOLVED 2026-09-23: byte-exact committed assets REJECTED (owner rule: binary assets never in git; manifest + fetch script instead). Folded: presentation gates in -p gta_like and the gitignore children/negation lesson -> domains/gates.md; kenney TLS -> agents/planner.md; rustfmt child modules -> agents/implementer.md + fixer.md; brp.py has no --help -> agents/qa.md.
