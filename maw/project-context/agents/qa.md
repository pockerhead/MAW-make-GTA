# Stage: qa

Loop:
1. `cargo build` — broken → REJECT, stop.
2. `cargo clippy -- -D warnings` — new warnings in touched code → NEEDS_FIXES.
3. `cargo test` — compare the failure LIST by name against the same run shape on the base commit; zero NEW failures is the bar (a count match can hide a swap).
4. Acceptance criteria that are gameplay rules → confirm each has a headless test that exercises it through the production plugin composition, and flip-RED at least one of them yourself (commit first; restore verified by sha256).
5. Runtime check of the real build (mandatory once the game has a window): drive it through the Bevy Remote Protocol, see "Runtime QA" below. Look at the screenshots yourself (Read the PNG) and judge them against the criteria.
6. Criteria that stay subjective after that (feel of controls/camera, audio, animation smoothness) → owner checklist: what to launch, what to do, what they should see. Do not fake them with a test.

## Runtime QA (Bevy Remote Protocol)

- Build with `cargo build --features dev` (enables `bevy_remote` + `bevy_brp_extras`, JSON-RPC on `http://127.0.0.1:15702`).
- Preferred driver: `python tools/qa/brp.py --help` (launch, wait-ready, keys, mouse, screenshot, diagnostics, query, shutdown) and the slice's scenario `tools/qa/scenarios/<slice>.py`. If the driver does not exist yet, call JSON-RPC directly, e.g.
  `curl -s -X POST http://127.0.0.1:15702 -H "Content-Type: application/json" -d '{"jsonrpc":"2.0","id":1,"method":"brp_extras/screenshot","params":{"path":"<abs path>.png"}}'`.
  Methods: `rpc.discover` (list all), `world.query`, `world.get_components`, `world.mutate_components`, `brp_extras/screenshot`, `brp_extras/send_keys`, `brp_extras/move_mouse`, `brp_extras/click_mouse`, `brp_extras/get_diagnostics`, `brp_extras/shutdown`. Confirm params with `rpc.discover` — do not guess them.
- Screenshots and scenario output go to the task's `scratch/qa/`; cite them in QA_REPORT.md by path, with what you saw in each.
- FPS: read `brp_extras/get_diagnostics` after >= 5 s of warm-up, in a `--release` build; report min/avg frame time, not one sample.
- FPS under vsync (`PresentMode::Fifo`) equals the display refresh, not the game cost: this host's only
  winit monitor was a 30 Hz virtual display and read exactly 30 FPS (TASK-002). Always report the monitor
  refresh and present mode with an FPS number, and measure frame COST with `AutoNoVsync` (switch it via BRP
  `world.mutate_components` on the window — no production change).
- Profiling when a criterion is about performance: `cargo run --release --features dev,profile` writes a Chrome trace (`trace_chrome`); summarise the top systems by total time yourself.
- Always end with `brp_extras/shutdown`; then make sure no game process is left (kill it by name if it hangs). A crash or hang during a scenario is a finding with its log, not a retry.

Cargo discipline:
- Commands: `cargo check`, `cargo build`, `cargo test`, `cargo clippy -- -D warnings`. Trust their output, never IDE diagnostics. A cold Bevy build takes minutes on Windows — give the command a long timeout (10 min) instead of backgrounding it.
- Never run two cargo commands at once. Under the codex sandbox run ONE named target per invocation (`--lib`, `--test <name>`, `--bin <name>`) with `-j 4` (parallel linkers exhausted the sandbox and killed process creation on this machine in another project).
- Format only the files you created or edited (`rustfmt --edition 2024 <file>` — match the edition in `Cargo.toml`); never `cargo fmt` on the whole workspace.
- Before writing your summary run `git status --short` and account for every path outside the task dir; a path you cannot explain is reverted.
- Write your stage report EARLY (first complete version as soon as results exist) and update it — an environment failure must never cost more than the last edit.
- If you launched the windowed game, make sure no game process is left running when you finish.

Verdict: all criteria covered and PASS (headless + runtime) → SHIP · any owner-only criterion → SHIP-PENDING-RUNTIME with the explicit owner checklist · a covered criterion FAILS → NEEDS_FIXES · build broken → REJECT. "MAW SHIP" is not "runtime SHIP": keep SHIP-PENDING-RUNTIME honest.

Sub-agent discipline: the harness launches every `Agent` call asynchronously and the report arrives later as a hand-back message. Count your launches and do not end your turn until every one has reported; state `children: N launched / N reported` before your final hand-back. A `Bash` command whose result your deliverable needs runs in the FOREGROUND — never `run_in_background=true` for it, and never end your turn waiting on a background run.
