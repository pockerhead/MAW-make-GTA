# PCTX proposals — TASK-002 (planner)

## 2026-09-23 — bevy-ecs: ecosystem crates can switch on render features transitively

What: add a risk lesson to `domains/bevy-ecs.md`: before adding an ecosystem crate to `gta_sim`,
run `cargo tree -p gta_sim -e features -i bevy_render`; a crate's own manifest may enable a
render-pulling feature of a third crate (bevy-tnua-avian3d 0.12.1 enables `avian3d/debug-plugin`
-> `bevy/bevy_render` unconditionally, invisible from its README and from avian's docs).

Why: the GDD and the premise check verified each crate's `bevy` line, but the `bevy_render` leak
came through a feature of a sibling dependency. Evidence: `scratch/tree_render_unpatched.txt`.

## 2026-09-23 — gates: headless Bevy apps need `app.finish()` + `app.cleanup()`

What: add to `domains/gates.md` (or bevy-ecs): a test harness that drives `app.update()` manually
must call `app.finish(); app.cleanup();` first, because plugins (avian 0.7 diagnostics resources)
register state in `Plugin::finish`; `App::run` does this, manual updates do not. Also: the first
`app.update()` under `TimeUpdateStrategy::FixedTimesteps(1)` runs 0 fixed ticks — count ticks via
`Time<Fixed>`, not updates.

Why: probe panicked without it (`scratch/probe_run1.txt`); probe tick counts `scratch/probe_run2.txt`.

> RESOLVED: both folded on 2026-09-23 — render-feature leak into domains/bevy-ecs.md (Risk lessons), finish/cleanup + first-update tick count into domains/gates.md (Risk lessons).
