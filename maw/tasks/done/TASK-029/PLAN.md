# PLAN — TASK-029: test CI on GitHub Actions + README badges

Cost of error: a red or missing CI is visible on the first push, and no game code changes. The one silent
failure mode is a CI that goes green without running the gate, for example an asset-dependent test taking
its SKIP branch. The plan closes that one explicitly and keeps everything else light.

## 1. Understanding

- There is no `.github/` directory. Remote: `git@github.com:pockerhead/MAW-make-GTA.git` (public).
- Toolchain: `Cargo.toml` pins `rust-version = "1.95.0"` and edition 2024. The local compiler is 1.95.0 and
  there is no `rust-toolchain` file. A probe showed an older rustc cannot build Bevy 0.19.1
  (`bevy_macro_utils` uses `rwlock_downgrade`, see `scratch/wsl_gta_sim_build.txt`), so CI must pin 1.95.0.
- `.cargo/config.toml` only has `[target.x86_64-pc-windows-msvc] linker = "rust-lld.exe"`, so it does nothing
  on Linux. Rust >= 1.90 already links with rust-lld by default on `x86_64-unknown-linux-gnu`
  ([Rust blog, 1.90 LLD](https://blog.rust-lang.org/2025/09/01/rust-lld-on-1.90.0-stable)). No Linux linker config is needed.
- Workspace: root package `gta_like` (bin) plus `crates/gta_sim` and `crates/citygen`. The `[patch]` entries point
  at `vendor/*`, which is tracked in git (8 files each), so resolution works on a fresh clone. The `fast`
  feature (`bevy/dynamic_linking`) is never enabled in CI.
- Linux system deps: `gta_like` uses bevy `default-features = true`, which pulls audio (alsa), gilrs (udev),
  and winit `x11` + `wayland` (bevy-0.19.1 `Cargo.toml` `[features] default_platform`). Bevy's own CI installs
  `libasound2-dev libudev-dev libwayland-dev` (plus optional `libxkbcommon-dev`)
  (`bevyengine/bevy/.github/actions/install-linux-deps/action.yml`). `gta_sim` uses bevy with
  `default-features = false` and a small feature list, and `citygen` has no bevy at all, so neither needs apt packages.
- Assets: `/assets/third_party/*` is gitignored except `manifest.ron` (9 packs: 4 Kenney 3D kits, car-kit,
  3 Kenney sound packs, `inter` fonts from a GitHub release). Nothing else under `assets/` is ignored (verified
  with `git ls-files --others --ignored --exclude-standard assets`). `tools/fetch_assets.py` is idempotent:
  a pack that already matches its sha256 is skipped (`fetch()`, ~l.375), downloads retry 4x (`RETRY_PAUSES`),
  and the final `check()` fails loudly.
- **Silent-skip gates:** `crates/gta_sim/tests/asset_manifest.rs:185-189` ("SKIP file check") and
  `src/audio/gate.rs:199-205` ("SKIP decode check") return green when the packs are absent, and cargo
  captures their `eprintln!`. CI therefore has to guarantee the packs are present before `cargo test`.
- Client gates that load GLBs or fonts panic with `GATE BROKEN` on a missing asset
  (`src/visuals/civilian_gate.rs:76`), so they fail loudly.
- `tools/qa/tree_check.py` already runs `cargo tree -p gta_sim -e normal[,dev] -i bevy_render` and fails if the
  output is non-empty (l.22-25). That covers the task's "cargo tree ... empty" item. It also runs `cargo tree`
  with `--features dev,debug`, which is resolution only, no compile.
- `tools/qa/font_check.py` needs `fontTools` (4.59.1 locally) and `assets/third_party/inter/*.ttf`.
  `tools/qa/test_brp.py` is stdlib only (7 tests, <1 s, passes locally).
- Cross-platform determinism: citygen golden hashes (`crates/citygen/tests/golden_hashes.txt`, also read by
  `crates/gta_sim/tests/city.rs:25`) **match on x86_64 Linux**. Probe: WSL Ubuntu 22.04,
  `cargo test -p citygen --test golden` gave 3 passed (`scratch/wsl_citygen_golden.txt`). The gta_sim physics
  tests could not be pre-run on Linux (see the toolchain note above).
- Timing gates: `civilian_bench.rs` (`MEAN_LIMIT` 8 ms) and `police_bench.rs` (11 ms) assert the mean tick time.
  Past QA measured about 0.7-1.0 ms locally, an ~8x margin. Other `Instant` uses are 120 s "GATE BROKEN" deadlines.
- README: the hero GIF sits between `<!-- SHOWCASE-HERO:START/END -->`. `tools/showcase/publish.py:85-93`
  (`set_hero`) replaces only the lines between those markers, so badges placed outside them survive. Sections:
  `## Как это работает` (l.57), `## Статус` (l.80), then `<!-- COST:START -->` / `## Сколько это стоит`, then `## Запуск`.
  `publish.py` finds the end of `## Статус` by looking for the next `## ` line, so a new `## ` section right after it is safe.
- Test counts (`#[test]` grep; the implementer takes the exact numbers from the CI output): gta_sim ~380 (2 ignored),
  citygen ~35 (3 ignored), client bin ~77.
- `gh` is read-only (SG-all): it can `gh run list/view/watch` but cannot open PRs or `workflow_dispatch` runs.
  Pushes go over SSH as pockerhead.

## 2. Approach

1. **Five workflow files, one badge each.** GitHub status badges exist per workflow, not per job
   ([docs: add a status badge](https://docs.github.com/en/actions/how-tos/monitor-workflows/add-a-status-badge)).
   So each badge is its own workflow: `clippy.yml`, `sim-gates.yml`, `citygen-gates.yml`, `client-gates.yml`,
   `repo-checks.yml`. The shared setup lives in one local composite action, `.github/actions/setup/action.yml`,
   so the five files stay about 25 lines each.
2. **Triggers:** `push` to every branch except `media` (the GIF branch has no Cargo workspace), `pull_request`,
   and `workflow_dispatch`. Push on task branches is needed because the pipeline cannot open PRs (read-only gh).
   It also gives the pre-merge real run and the flip-RED branch. Concurrency is
   `group: ${{ github.workflow }}-${{ github.ref }}` with `cancel-in-progress: true`.
3. **Linux only (`ubuntu-latest`).** Every MAW stage already runs all gates on Windows locally, so a Windows CI
   job would mostly repeat the owner's platform. The new information CI adds is a second OS and a clean clone.
   A Windows matrix would double the heavy builds against the 10 GB free repo cache cap
   ([changelog 2025-11-20](https://github.blog/changelog/2025-11-20-github-actions-cache-size-can-now-exceed-10-gb-per-repository/))
   and runs about 2x slower. Standard runners are free for a public repo, so the cost is wall time and cache,
   not money. This is recorded as a decision in the log and repeated as open question Q1.
4. **Caching:**
   - `Swatinem/rust-cache@v2` per job (its default key already includes job id, rustc version and `Cargo.lock`),
     with `cache-on-failure: true` so the first Linux run keeps its compile even if a test is red.
   - Env `CARGO_INCREMENTAL=0`, `CARGO_PROFILE_DEV_DEBUG=0`, `CARGO_PROFILE_TEST_DEBUG=0` (Bevy's own CI uses the
     same set) to shrink the target and the cache.
   - No sccache. The jobs build disjoint crate sets, and rust-cache is enough and has fewer moving parts.
   - Assets: `actions/cache@v6` on `assets/third_party/*` minus `manifest.ron`, keyed
     `third-party-${{ hashFiles('assets/third_party/manifest.ron') }}`, then `fetch_assets.py` (a no-op on a hit,
     sha256-verified either way), then `fetch_assets.py --check`. `--check` makes the SKIP branches unreachable in CI.
5. **Pinned action majors (checked with `gh api repos/<r>/releases/latest` on 2026-09-25):**
   `actions/checkout@v7` (v7.0.1), `actions/cache@v6` (v6.1.0), `actions/setup-python@v7` (v7.0.0),
   `Swatinem/rust-cache@v2` (v2.9.2), `dtolnay/rust-toolchain@1.95.0` (the ref names the toolchain; `components: clippy`).
   Bevy's own CI uses the same checkout v7.0.1 and cache v6.1.0.
6. **Permissions:** `permissions: contents: read` in every workflow. No secrets are used, only the implicit GITHUB_TOKEN.

## 3. Steps

### Step 1: composite action `.github/actions/setup/action.yml`
Inputs: `linux-deps` (default `"false"`) and `assets` (default `"false"`). Steps, all `shell: bash`:
1. `dtolnay/rust-toolchain@1.95.0` with `components: clippy`.
2. If `linux-deps == 'true'`: `sudo apt-get update && sudo apt-get install --no-install-recommends -y libasound2-dev libudev-dev libwayland-dev libxkbcommon-dev`.
3. `Swatinem/rust-cache@v2` with `cache-on-failure: true`.
4. `actions/setup-python@v7` with `python-version: "3.12"`, so `python` exists on the runner (no PEP 668 pip issue).
5. If `assets == 'true'`: `actions/cache@v6` with `path: |\n assets/third_party/*\n !assets/third_party/manifest.ron` and the key above.
   Then `python tools/fetch_assets.py`, then `python tools/fetch_assets.py --check`.

A composite action cannot see the workflow's `env:`. That is fine, because the `CARGO_*` env is set at workflow level
and applies to the runner steps anyway. Checkout must run before `uses: ./.github/actions/setup`.

### Step 2: the five workflows in `.github/workflows/`
Shared header in each file:
```yaml
on:
  push:
    branches-ignore: [media]
  pull_request:
  workflow_dispatch:
permissions:
  contents: read
concurrency:
  group: ${{ github.workflow }}-${{ github.ref }}
  cancel-in-progress: true
env:
  CARGO_TERM_COLOR: always
  CARGO_INCREMENTAL: 0
  CARGO_PROFILE_DEV_DEBUG: 0
  CARGO_PROFILE_TEST_DEBUG: 0
```
Each has one job on `ubuntu-latest` with `timeout-minutes: 60` and runs `actions/checkout@v7`, then
`./.github/actions/setup`. The `name:` is the badge label.

| file | `name:` | setup inputs | run |
|---|---|---|---|
| `clippy.yml` | `clippy` | linux-deps | `cargo clippy --locked --workspace --all-targets -- -D warnings` |
| `sim-gates.yml` | `sim gates` | assets | `cargo test --locked -p gta_sim` |
| `citygen-gates.yml` | `citygen gates` | none | `cargo test --locked -p citygen` |
| `client-gates.yml` | `client gates` | linux-deps, assets | `cargo test --locked -p gta_like --bin gta_like` |
| `repo-checks.yml` | `repo checks` | assets | `pip install fonttools==4.59.1`, then `python tools/qa/tree_check.py`, `python tools/qa/font_check.py`, `python -m unittest tools/qa/test_brp.py` |

The three test workflows run with `shell: bash` (which gives `-eo pipefail`) as
`cargo test ... 2>&1 | tee test.log`, followed by a step with `if: always()` that appends the
`^test result:` lines and their summed passed/ignored counts to `$GITHUB_STEP_SUMMARY`. This is where the README
counts come from. Keep it to one `grep`/`awk` line and do not write a script file.

`sim-gates` fetches assets so that `asset_manifest.rs` runs its file check instead of SKIP. `citygen` needs no assets.

### Step 3: README badges at the top only
Right after `<!-- SHOWCASE-HERO:END -->` (README.md l.7) and outside the markers, add one
`<p align="center">` line with five `<a href=".../actions/workflows/<file>?query=branch%3Amain"><img src=".../actions/workflows/<file>/badge.svg?branch=main" alt="<name>"></a>`
links, in this order: clippy, sim gates, citygen gates, client gates, repo checks. The repo URL is
`https://github.com/pockerhead/MAW-make-GTA`. Use HTML, not markdown, because markdown does not render inside `<p>`.

### Step 4: README section `## Что проверяет CI`
Add it between the end of `## Статус` (the line `- План и порядок: ...`) and `<!-- COST:START -->`. It holds a short
table with one row per workflow: what it runs and the test count from the first green main run's step summary,
for example "gta_sim: N тестов, M ignored". Add one sentence: Linux (ubuntu-latest), Rust 1.95.0, on every push and PR,
the counts were updated after TASK-0NN, and MAW stages still run everything locally on Windows. No badges go in this section.

### Step 5: post-task routine in `AGENTS.md`
In "Planning artifacts" → "После каждой закрытой задачи", add one bullet: after the merge to main, check CI
(`gh run list --branch main --limit 5` shows five `success` on the merge commit, or `gh run watch`). If one is red,
the task is not closed. Update the counts in README "Что проверяет CI" from the run's step summary. Also add one
line to "Сборка" in the "Проект" section: CI = `.github/workflows/*.yml` (Linux).

### Step 6: real runs and evidence (implementer, then QA)
1. Push `infra/test-ci`. This is run 1 (cold). If a gate fails on Linux only, see R1: do not fix the test in this task
   without a decision.
2. Push a second commit (for example the README counts from Step 4). This is run 2 (warm). Report the per-job wall time
   for both runs from `gh run view <id> --json jobs --jq '.jobs[]|{name,startedAt,completedAt,conclusion}'`,
   plus the rust-cache "restored from cache" line.
3. **flip-RED at CI level:** create branch `ci/flip-red-029` from the head and push two separate commits, each a
   separate run:
   - (a) `let unused_ci_flip = 1;` inside a function in `src/main.rs`: expect `clippy` red and the rest green;
   - (b) change one hex digit of seed 1 in `crates/citygen/tests/golden_hashes.txt`: expect `citygen gates` red
     (`golden_hashes_match`) and `sim gates` red (`city.rs::runtime_hash_matches_golden`, which reads the same file),
     with clippy, client and repo checks green.

   Record run links and the red job per sabotage. Then `git push origin --delete ci/flip-red-029`. Nothing from it is merged.
4. After the merge to main, the run on main is green. Put the link in QA_REPORT, and check that the badges render
   (open the README on GitHub).

## 4. Risk areas

- **R1: Linux-only test failures** (avian/glam float paths through platform libm, or timing). The citygen golden hashes
  are already proven identical. Physics tests are not. If a gate is red only on Linux, it is a real finding. Report the
  test and the value delta, and take a decision (Q2) before touching tolerances. Never add `#[cfg(not(target_os))]` or `#[ignore]`.
- **R2: bench gates on 4 vCPU runners.** The margin is ~8x, and the tests in a binary share the cores. If
  `civilian_bench`/`police_bench` flake, the fix is to run those two test binaries with `-- --test-threads=1` as a separate
  step in `sim-gates.yml`, not to raise `MEAN_LIMIT`.
- **R3: cache cap.** Four heavy caches per branch (clippy, sim, client, plus the registry), and branches from every task.
  LRU eviction removes old task branches first. If `gh cache list` (or the Actions caches page) shows the total near 10 GB,
  add `save-if: ${{ github.ref == 'refs/heads/main' }}` to rust-cache.
- **R4: missing apt package.** If the build error names a missing `.pc` (for example `xkbcommon`, `x11`), add that `-dev`
  package in Step 1.2. Tests use MinimalPlugins and need no display or audio device.
- **R5: kenney.nl flakes on a cache miss.** `fetch_assets.py` already retries 4x and the job fails loudly. A rerun needs
  write access, so a push is required. This only happens when the manifest changes.
- **R6: new clippy lints only on Linux** (for example a `cfg`-dependent lint). Fix it in this task: it is a real warning
  under `-D warnings`.
- **R7: disk space.** A full Bevy test build with debuginfo=0 fits in the hosted runner's free space. If it fails with
  "No space left", free the preinstalled toolchains (`sudo rm -rf /usr/share/dotnet /usr/local/lib/android`) as a first step.

## 5. Open questions

- **Q1 (decided by the planner, the owner can overturn it):** Linux only. Windows CI would cover the owner's platform,
  which local MAW runs already cover, at about 2x wall time and cache. If the owner wants it, add
  `strategy.matrix.os: [ubuntu-latest, windows-latest]` to `sim-gates` and `client-gates` only.
- **Q2:** if R1 fires, is a Linux-only float divergence fixed in this task (tolerance plus a comment on why) or in a
  follow-up task, with the job temporarily marked `continue-on-error`? Recommendation: a follow-up task, and report it to the owner.
- Counts in the README are static. They are updated by the post-task routine from the CI step summary and are not
  generated automatically, because a generator would need a push from CI and so a write token.

Evidence: `scratch/wsl_citygen_golden.txt`, `scratch/wsl_gta_sim_build.txt`. No Rust dependency is added, so no
Cargo.lock work is needed.
