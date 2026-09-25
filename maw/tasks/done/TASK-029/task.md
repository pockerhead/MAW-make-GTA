# TASK-029: CI — тесты на каждый push/PR и бейджи в README

Type: infra
Mode: full
Priority: high
Branch: infra/test-ci
Domains: gates

## Description
Owner (2026-09-25): "в целом тесты гонять, надо CI сделать чтобы не ждать прогонов локально и чтобы повесить лычки в ридми — такие-то тесты гоняются и зелёные".

GitHub Actions workflow on every push to main and on PRs (and `workflow_dispatch`):
- `cargo clippy --workspace --all-targets -- -D warnings`;
- headless gates: `cargo test -p gta_sim` and `cargo test -p citygen` (Linux; Windows too if affordable — the owner's platform);
- presentation gates `cargo test -p gta_like --bin gta_like` (they run on MinimalPlugins + GLB harness; they need the fetched CC0 assets → run `tools/fetch_assets.py` with a cache; decide per-OS);
- `tools/qa/tree_check.py`, `tools/qa/font_check.py`, `python -m unittest tools/qa/test_brp.py`, `cargo tree -p gta_sim -e normal -i bevy_render` empty;
- separate jobs so each has its own status; rust-cache/sccache; concurrency group cancels superseded runs.
README: badges at the top next to the hero GIF (one per job/workflow: clippy, sim gates, citygen gates, client gates, repo checks) linking to the workflow runs — badges ONLY at the top. The "Что проверяет CI" section goes LOWER in the README (owner, 2026-09-25: "не надо туда же раздел что проверяет сиай, лучше ниже"), its own section after "Как это работает"/"Статус", with test counts (derive from `cargo test` output in CI or state them statically and keep them updated after each task).
Pipeline integration: MAW stages keep running tests locally; after each merged task the orchestrator checks the CI run on main is green (gh CLI) as part of the post-task routine.

## Acceptance criteria
- [ ] Workflow(s) in `.github/workflows/`; a real run on GitHub is green on main (link in IMPL_SUMMARY / QA_REPORT).
- [ ] A deliberately broken commit on a throwaway branch/PR turns the matching job red (flip-RED at CI level), then is dropped.
- [ ] README badges (top only) render and link to the runs; a separate "Что проверяет CI" section lower in the README.
- [ ] Wall time of the full CI reported; caches effective on the second run (time reported).
- [ ] Nothing in the workflow needs secrets beyond GITHUB_TOKEN.

## Dependencies
- blocked by TASK-026
