# TASK-028: CI — релизы под Linux и Windows через GitHub Actions

Type: infra
Mode: full
Priority: high
Branch: infra/release-ci
Domains: gates

## Description
Owner (2026-09-25): "после Т16 еще надо сделать CI который будет сам релизы собирать на linux and windows через github actions".

GitHub Actions workflow in `pockerhead/MAW-make-GTA` that builds release binaries for Windows (x86_64-pc-windows-msvc) and Linux (x86_64-unknown-linux-gnu) and publishes a GitHub Release with zips that contain the exe + the full `assets/` tree (binary CC0 assets are not in git: fetch them in CI with `tools/fetch_assets.py` per `assets/third_party/manifest.ron`, verifying sha256; cache the downloaded packs — kenney.nl TLS is flaky, so retry + cache via actions/cache; licences/attribution files included in the zip).

Decide and document (ADR): trigger (tag `v*` push → release; plus manual `workflow_dispatch`), toolchain pin (the repo's Rust version), Linux system deps for Bevy (alsa, udev, x11/wayland libs), build profile (`--release`, no `dev`/`fast` features; the `fast` dynamic-linking feature must not be in release builds), rust-lld/linker config portability (`.cargo/config.toml` is Windows-tuned — check it does not break Linux), sccache/Swatinem rust-cache for build time, artifact naming, zip layout (exe next to `assets/`), and a smoke check in CI (headless `cargo test -p gta_sim -p citygen` on Linux; optionally a `--version`/headless boot of the built binary). Workflow must not leak secrets and uses only GITHUB_TOKEN.

## Acceptance criteria
- [ ] `.github/workflows/release.yml` (and a PR/push check workflow if cheap): on a `v*` tag, both OS builds succeed and a GitHub Release is created with two zips; on `workflow_dispatch`, artifacts are uploaded.
- [ ] Zips verified: unpack → exe + `assets/` (incl. fetched third-party assets + licences) → the game boots to the main menu (Linux: headless/xvfb boot smoke or at least asset-load check; Windows: verified locally by QA from the downloaded artifact).
- [ ] Headless test job green on Linux in CI.
- [ ] A real run on GitHub proven: link to the successful workflow run and the created release (a pre-release tag like `v0.1.0-rc1` is fine).
- [ ] README "Запуск" updated: where to download releases.

## Dependencies
- blocked by TASK-017
- blocked by TASK-029 (reuses its CI setup: toolchain, caches, asset fetch)
