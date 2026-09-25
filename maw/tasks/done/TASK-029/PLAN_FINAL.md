# PLAN_V2 — TASK-029: test CI on GitHub Actions + README badges

Reviewer: plan-reviewer-1 (plan-reviewer-2 skipped for this infra task, so this file is self-contained and becomes PLAN_FINAL).

Cost of error: a red or missing CI shows up on the first push, and no game code changes. Two failure modes are silent:
(1) CI goes green without running a gate, for example an asset gate taking its SKIP branch or a job that runs 0 tests;
(2) the caches look configured but never warm up. The plan gates both explicitly and keeps everything else light.

## 0. Disconfirmation (done before the review)

Counter-examples tested, looking for ways a clean Linux clone could make the plan wrong:

| hypothesis | probe | result |
|---|---|---|
| asset paths only resolve on case-insensitive NTFS | scanned 218 literal asset paths in `src/`, `crates/`, `assets/**/*.ron` against the files on disk, case-sensitively | 0 mismatches, **did not hold** |
| CRLF-dependent tests or golden files differ on a Linux checkout | `.gitattributes` = `* text=auto eol=lf` (binaries marked) | working trees are already LF everywhere, **did not hold** |
| `fetch_assets.py --check` does not actually make the SKIP branches unreachable | read `check()` / `pack_problems()` / `main()` | `--check` returns 1 unless every pack dir exists with exact sha256 per file, **did not hold** (the plan is right) |
| `tree_check.py` behaves differently on Linux (`cargo tree` resolves host-target deps only; winit/wayland crates appear) | ran all its queries with `--target x86_64-unknown-linux-gnu`, `scratch/pr1_linux_tree.txt` | bevy_render empty, image 0.25.9, bevy_egui 0.40.1, no critical duplicates, **did not hold** |
| **five workflows sharing one rust-cache key** | read `Swatinem/rust-cache` v2 `src/config.ts` | key = `v0-rust-${GITHUB_JOB}-${os}-${arch}-${envhash}-${lockhash}`, **no workflow name**. **HELD**: if the five files reuse a job id (`test`/`build`, the default a copy-paste yields), they overwrite each other's cache. See N1. |

## 1. Review notes (issues in PLAN.md, with evidence)

- **N1 (major, silent): rust-cache key collision across workflows.** PLAN §2.4 says "its default key already includes
  job id". That is true, but the job id is `GITHUB_JOB`, the YAML key of the job, and it is **not** unique across
  workflow files. rust-cache `src/config.ts`: `key += \`-${job}\`` with `job = process.env.GITHUB_JOB`, then OS/arch,
  then a hash of rustc version + `CARGO*`/`RUST*` env + manifests. All five workflows share the env and `Cargo.lock`,
  so identical job ids give one key. Then the first job to finish (citygen, the smallest) saves; sim/client/clippy get
  an *exact hit* on the wrong target dir, rebuild from scratch, and never save (an exact hit skips the save). The
  "caches effective on the second run" AC would be RED for the heavy jobs, or look green by luck. PLAN never names the
  job ids. **Fix:** a distinct job id per workflow (`clippy`, `sim`, `citygen`, `client`, `repo`), and QA checks in the
  warm run that each job's rust-cache "Cache Configuration" prints a different key.
- **N2 (major, gate honesty): flip-RED covers only 3 of 5 jobs.** PLAN §6.3 sabotages clippy and the golden hash
  (citygen + sim). `client gates` and `repo checks` are never shown red, so a client job that compiles but runs 0
  tests, or a repo job whose steps swallow a failure, would pass unnoticed (gates domain: "Flip-RED or it is not a
  gate"). **Fix:** one sabotage per job, with the predicted red/green set written down before each push (Step 7).
- **N3 (minor): clippy scope versus local practice.** Local MAW stages run `cargo clippy --workspace --all-targets -- -D warnings`
  AND `cargo clippy -p gta_sim -p citygen --all-targets -- -D warnings`. The second catches lints that only appear
  under gta_sim's own feature set, because `--workspace` unifies bevy default features into gta_sim. The spec names only the
  first. **Decision:** add the second as another step in `clippy.yml`. It is cheap (gta_sim's small bevy set) and mirrors the
  local gate.
- **N4 (minor): cache scoping.** GitHub caches are branch-scoped: a branch can read its own caches and the default
  branch's, never another branch's. So the first run on `main` after the merge is **cold**, even though `infra/test-ci` is warm.
  PLAN does not say this. QA must not read a cold main run as "cache broken". The AC timing pair is run 1 and run 2 on
  `infra/test-ci`. After the merge, the task branches of future tasks restore from main.
- **N5 (minor): nested actions inside a local composite action.** PLAN puts `Swatinem/rust-cache` and `actions/cache`
  inside `.github/actions/setup`. Runners have supported `uses:` and post-steps (the cache save) in composites for years,
  but web sources are mixed, and a missing save is again silent. **Fix:** Step 6 checks in the run-1 logs that
  "Post Run ... rust-cache" / "Post ... cache" actually saved (the `Cache saved with key` line). If they did not, move those two
  `uses:` steps from the composite into each workflow.
- **N6 (minor): `shell: bash` must be explicit on the `| tee` steps.** On `ubuntu-*`, a `run:` without `shell:` runs as
  `bash -e {0}` (no pipefail), so `cargo test | tee` would go green on a red test. PLAN states this correctly. Keep it as
  a hard rule, and make it part of the flip-RED proof (a red test must turn the job red *through* the tee).
- **Verified correct (kept):** no `.github/` yet; `rust-version = "1.95.0"`, and there is no `rust-toolchain` file; the WSL probe
  shows 1.89 cannot build `bevy_macro_utils` 0.19.1, so CI pins 1.95.0 (`dtolnay/rust-toolchain` branch `1.95.0` exists);
  action versions checked via `gh api` today: checkout v7.0.1, cache v6.1.0, setup-python v7.0.0, rust-cache v2.9.2
  (tags `v7`/`v6`/`v7`/`v2` exist). Apt set = Bevy v0.19.1's own `install-linux-deps` (alsa, udev, wayland, plus xkb).
  gta_sim/citygen need no apt. The SKIP branches are at `asset_manifest.rs:185-189` and `src/audio/gate.rs:199-205`.
  `civilian_gate.rs:76` panics with `GATE BROKEN` on a missing asset. `set_hero` rewrites only between the HERO markers.
  `set_status` ends `## Статус` at the next `## ` line. `fonttools==4.59.1` exists on PyPI (py>=3.9). kenney.nl serves the
  zips via nginx (no bot challenge) and Inter comes from GitHub releases. The only non-ignored wall-clock gates are the
  two bench files (`MEAN_LIMIT` 8 ms in civilian_bench). `city_startup_budget` and citygen `perf.rs` are `#[ignore]`.
- `branches-ignore: [media]` alone also means tag pushes do not trigger (GitHub runs a branch-filtered push workflow
  only for branch refs). That is the intent.

## 2. Updated understanding

- Repo: `git@github.com:pockerhead/MAW-make-GTA.git` (public), default branch `main`, GIF branch `media` (no Cargo
  workspace; `tools/showcase/publish.py` pushes it). `gh` is logged in read-only (SG-all): `gh run list/view/watch` and
  `gh api` work, `gh pr create` / `workflow_dispatch` do not. Pushes go over SSH as pockerhead.
- Workspace: root bin `gta_like` (bevy `default-features = true` + `bevy_settings`) plus `crates/gta_sim` (bevy with no
  defaults: std, multi_threaded, bevy_state, bevy_log, bevy_asset, serialize, reflect_auto_register, and `debug` for dev) plus
  `crates/citygen` (no bevy). `[patch]` points to tracked `vendor/*`. `.cargo/config.toml` only sets the Windows linker.
  Rust >= 1.90 already uses rust-lld on x86_64 Linux. Dev profile: opt-level 1, deps opt-level 3.
- gta_sim has 40 integration test files (40 test binaries, each linking bevy+avian). citygen has 4 test files. The client bin
  has presentation gates in `src/**/…_gate.rs` running on MinimalPlugins + a GLB harness: no window, no GPU, and no audio device
  (`src/audio/gate.rs` uses rodio decoding only, with no `AudioPlugin`).
- Assets: `/assets/third_party/*` is ignored except `manifest.ron` (9 packs). `fetch_assets.py` skips a pack whose files all match
  their sha256, stores downloaded zips in `target/asset-cache`, and retries 4 times. `--check` is offline and exits 1 on any missing,
  unexpected or mismatched file.
- `tools/qa/tree_check.py` covers the spec's "`cargo tree -p gta_sim -e normal -i bevy_render` empty" (plus normal,dev,
  image/egui pins and critical duplicates). It passes against the Linux target (`scratch/pr1_linux_tree.txt`).
  `font_check.py` needs fontTools plus the Inter TTFs. `test_brp.py` is stdlib only.
- citygen golden hashes match on x86_64 Linux (`scratch/wsl_citygen_golden.txt`). The gta_sim physics gates have never run on Linux.
- Test counts (the implementer takes exact numbers from CI): `#[test]` grep gives 313 in `gta_sim/tests` plus unit tests in `src/`,
  about 35 in citygen, about 77 in the client bin.

## 3. Revised approach

1. **Five workflow files, one badge each** (badges are per workflow): `clippy.yml`, `sim-gates.yml`, `citygen-gates.yml`,
   `client-gates.yml`, `repo-checks.yml`. **Each has exactly one job with a distinct job id** (N1). Shared setup is in one
   composite action `.github/actions/setup/action.yml`.
2. **Triggers:** `push` with `branches-ignore: [media]`, `pull_request`, `workflow_dispatch`. Push on task branches is how
   the pipeline gets pre-merge runs (it cannot open PRs). Concurrency: `group: ${{ github.workflow }}-${{ github.ref }}`,
   `cancel-in-progress: true`.
3. **Linux only (`ubuntu-latest`)**, resolved Q1. Toolchain pinned to 1.95.0.
4. **Caching:** `Swatinem/rust-cache@v2` (`cache-on-failure: true`) per job with a distinct job id. Env `CARGO_INCREMENTAL=0`,
   `CARGO_PROFILE_DEV_DEBUG=0`, `CARGO_PROFILE_TEST_DEBUG=0`. No sccache. Assets go in `actions/cache@v6` keyed on
   `hashFiles('assets/third_party/manifest.ron')`, then `fetch_assets.py`, then `fetch_assets.py --check` in the **same job**
   before any `cargo test`. That structurally closes the SKIP branches. No `save-if` restriction yet: it would stop the branch's
   own run 2 from being warm (AC). Revisit per R3.
5. **Permissions** `contents: read`. No secrets, only the implicit GITHUB_TOKEN.

## 4. Revised steps

### Step 1: composite action `.github/actions/setup/action.yml`
`runs.using: composite`. Inputs `linux-deps` (default `"false"`) and `assets` (default `"false"`). Every `run:` step has `shell: bash`.
1. `uses: dtolnay/rust-toolchain@1.95.0`, `with: components: clippy`.
2. `if: inputs.linux-deps == 'true'`: `sudo apt-get update && sudo apt-get install --no-install-recommends -y libasound2-dev libudev-dev libwayland-dev libxkbcommon-dev`.
3. `uses: Swatinem/rust-cache@v2`, `with: cache-on-failure: true`. It must come after the toolchain step, because the key hashes rustc.
4. `uses: actions/setup-python@v7`, `python-version: "3.12"`.
5. `if: inputs.assets == 'true'`: `uses: actions/cache@v6` with
   `path: |` / `assets/third_party/*` / `!assets/third_party/manifest.ron` and `key: third-party-${{ hashFiles('assets/third_party/manifest.ron') }}`.
   Then `python tools/fetch_assets.py`, then `python tools/fetch_assets.py --check`. Its "third-party packs match the manifest" line is the liveness evidence.

Checkout runs before `uses: ./.github/actions/setup`. Workflow-level `env:` reaches composite steps through the runner process env.
Verify in run 1 that the nested post-steps saved (N5).
→ check: run 1 logs show `Cache saved with key: v0-rust-<jobid>-Linux-X64-…` for each rust job and the `third-party-…` key for one asset job.

### Step 2: the five workflows in `.github/workflows/`
Shared header:
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
One job each: `runs-on: ubuntu-latest`, `timeout-minutes: 60`, steps `actions/checkout@v7` then `./.github/actions/setup`.
The top-level `name:` is the badge label.

| file | `name:` | **job id** | setup inputs | run steps (`shell: bash` on every run) |
|---|---|---|---|---|
| `clippy.yml` | `clippy` | `clippy` | linux-deps | `cargo clippy --locked --workspace --all-targets -- -D warnings`; `cargo clippy --locked -p gta_sim -p citygen --all-targets -- -D warnings` |
| `sim-gates.yml` | `sim gates` | `sim` | assets | `cargo test --locked -p gta_sim 2>&1 \| tee test.log` |
| `citygen-gates.yml` | `citygen gates` | `citygen` | none | `cargo test --locked -p citygen 2>&1 \| tee test.log` |
| `client-gates.yml` | `client gates` | `client` | linux-deps, assets | `cargo test --locked -p gta_like --bin gta_like 2>&1 \| tee test.log` |
| `repo-checks.yml` | `repo checks` | `repo` | assets | `pip install fonttools==4.59.1`; `python tools/qa/tree_check.py`; `python tools/qa/font_check.py`; `python -m unittest tools/qa/test_brp.py` (separate steps, so a failing one names itself) |

Test workflows: an extra step with `if: always()` and `shell: bash` appends the `test result:` lines and the summed
`passed`/`ignored`/`failed` to `$GITHUB_STEP_SUMMARY`. Strip ANSI first (`sed 's/\x1b\[[0-9;]*m//g'`), because
`CARGO_TERM_COLOR` is set. One grep/awk pipeline, no script file. This is where the README counts come from.
→ check: `actionlint` is not required. `python -c "import yaml,sys; [yaml.safe_load(open(f)) for f in sys.argv[1:]]" .github/workflows/*.yml .github/actions/setup/action.yml`
parses (use PyYAML if installed, otherwise skip), and run 1 is the real check.

### Step 3: README badges at the top only
Right after `<!-- SHOWCASE-HERO:END -->` (README.md l.7), outside the markers (`set_hero` rewrites only the lines inside them),
add one `<p align="center">` line with five
`<a href="https://github.com/pockerhead/MAW-make-GTA/actions/workflows/<file>?query=branch%3Amain"><img src="https://github.com/pockerhead/MAW-make-GTA/actions/workflows/<file>/badge.svg?branch=main" alt="<name>"></a>`
in the order clippy, sim gates, citygen gates, client gates, repo checks. Use HTML, not markdown, inside `<p>`.
→ check: `python tools/showcase/publish.py --dry-run` is not needed. Grep shows the badge line sits outside the HERO marker pair.

### Step 4: README section `## Что проверяет CI`
Put it after `## Статус` ends (the line `- План и порядок: …`) and before `<!-- COST:START -->`. It holds a table with one row per workflow:
what it runs, and the test count from the first green run's step summary ("gta_sim: N passed, M ignored"). Add one sentence:
Linux (ubuntu-latest), Rust 1.95.0, runs on every push and PR, counts as of TASK-029, and MAW stages still run everything locally on Windows. No badges here.
The section is written in Russian like the rest of the README, plain style.

### Step 5: post-task routine in `AGENTS.md`
- In "Planning artifacts", next to "После каждой закрытой задачи": add one bullet. After the merge to main, `gh run list --branch main --limit 10`
  must show all five workflows `success` on the merge commit (`gh run watch <id>` while they run). A red one means the task is not closed.
  Refresh the counts in README "Что проверяет CI" from the step summaries.
- "Сборка" in "Проект": add a short mention that CI = `.github/workflows/*.yml` (Linux, 5 workflows).
Surgical: no other AGENTS.md or README edits besides the status lines the post-task routine already owns.

### Step 6: real runs, cache evidence (implementer, then QA)
1. Push `infra/test-ci` for run 1 (cold). Linux-only red: follow R1/Q2 (no tolerance loosening; record it and open a follow-up task with
   evidence; the job's summary lists the failing tests; never `#[ignore]`/`cfg` it away silently).
2. Check N5: every rust job's log has `Cache saved with key`, and the five keys are pairwise different (N1).
3. Push a second commit (the README counts) for run 2 (warm). Report per job and in total the wall time for both runs from
   `gh run view <id> --json jobs --jq '.jobs[]|{name,startedAt,completedAt,conclusion}'`. Total = first `startedAt` to last `completedAt`
   across the five runs of one commit. Also report each job's rust-cache `Cache restored from key` line and the asset cache hit.
   Expect run 2 to be clearly faster for sim/client/clippy. If a heavy job is not faster, suspect N1/N5 before anything else.

### Step 7: flip-RED at CI level (all five jobs)
Branch `ci/flip-red-029` from the green head. One sabotage per commit, one push per commit, and each push's run completes before the next
(concurrency would cancel it otherwise). Write the predicted red set in the summary **before** pushing, and derive each prediction by running
the same sabotage locally first (the gates domain says numbers and verdicts are derived, not intended):
- (a) clippy: `let unused_ci_flip = 1;` inside a function in `src/main.rs`. Predict: `clippy` red, the other four green. In `client gates`
  this is only a warning.
- (b) citygen + sim: change one hex digit of seed 1 in `crates/citygen/tests/golden_hashes.txt`. Predict: `citygen gates` red
  (`golden_hashes_match`), `sim gates` red (`city.rs::runtime_hash_matches_golden` reads the same file), the rest green. This also proves the
  `| tee` pipeline propagates a failed test (N6).
- (c) client gates: break a presentation mechanism, not a test. Candidate: point one sound path in `assets/audio/mix.ron`
  at a nonexistent file, so `src/audio/gate.rs` hits `GATE BROKEN: <path>`. Grep shows no gta_sim source/test referencing the sound packs,
  so predict: `client gates` red and the rest green. Confirm locally, and pick another client-only mechanism if anything else reds.
- (d) repo checks: add one glyph missing from Inter (for example U+2603) to a string literal in `assets/ui/strings.ron`. Predict: `font_check.py` red.
  Confirm locally whether any client/sim gate parses that string, and record it.

For each: run link, red jobs, the failing test/step name. A prediction that misses is a finding, not something to wave away.
Then `git push origin --delete ci/flip-red-029`. Nothing from it is merged.

### Step 8: after the merge
The first `main` run is cold (N4). Its link goes in QA_REPORT with all five green. Open the README on GitHub and confirm all five badges render and
link to the filtered runs pages.

## 5. Risk areas

- **R1: Linux-only test failures** (avian/glam float paths, platform libm, timing). citygen golden is proven identical. Physics is unproven.
  Per Q2: record the test and the value delta, open a follow-up task, and do not loosen tolerances or add `#[ignore]`/`cfg(target_os)` here.
- **R2: bench gates on 4-vCPU runners.** `civilian_bench`/`police_bench` assert mean tick < 8/11 ms, about 0.7-1 ms locally. If flaky, run those
  two test binaries in a separate step with `-- --test-threads=1` (their tests in one binary compete for cores). Do not raise `MEAN_LIMIT`.
- **R3: cache cap (10 GB per repo, LRU).** About 3 heavy rust caches per branch plus assets. Watch `gh cache list` or the Actions caches page. If near the cap,
  add `save-if: ${{ github.ref == 'refs/heads/main' }}` (task branches then restore from main). Do not add it in this task: it would break the run-2 AC.
- **R4: missing apt package.** If a `-sys` crate names a missing `.pc` (`xkbcommon`, `x11`, `fontconfig`), add that `-dev` package in Step 1.2.
- **R5: rust-cache key collision (N1)** comes back if someone later copies a workflow and keeps the job id. The job-id column in Step 2 is the rule.
- **R6: nested post-steps not saving (N5).** Detected in Step 6.2. The fallback is to inline the two cache `uses:` steps into each workflow.
- **R7: new Linux-only clippy lints.** Fix them in this task: a real warning under `-D warnings`.
- **R8: disk.** 40 gta_sim test binaries plus deps at opt-level 3, debuginfo 0. If "No space left on device", free the preinstalled SDKs first
  (`sudo rm -rf /usr/share/dotnet /usr/local/lib/android /opt/ghc`).
- **R9: toolchain drift.** CI stays on 1.95.0 while the local toolchain may move. A newer local clippy can flag lints CI does not, and the reverse.
  Bumping the pin is a deliberate one-line change later, not part of this task.
- **R10: asset download from runners.** kenney.nl (nginx) plus GitHub releases, 4 retries, and a loud failure. This only matters on a manifest change (cache miss).

## 6. Open questions

None blocking. Q1 (Linux only) and Q2 (no tolerance loosening, follow-up task) are resolved in TASK_FINAL. The second clippy step (N3) is a reviewer
decision logged in `log.jsonl`. The owner can drop it. README counts are static and refreshed by the post-task routine, because auto-generation would need a
CI push, and so a write token.

Evidence: `scratch/wsl_citygen_golden.txt`, `scratch/wsl_gta_sim_build.txt`, `scratch/pr1_linux_tree.txt`. No Rust dependency changes, and no Cargo.lock work.
