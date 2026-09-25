# IMPL_SUMMARY — TASK-029: test CI on GitHub Actions + README badges

Branch `infra/test-ci`, commits c8deca3 (workflows, badges, AGENTS), 583f275 (README "Что проверяет CI"),
fadeb1f (`--no-fail-fast`). Pre-flight: every presupposition of PLAN_FINAL checked (files, markers, README
sections, fetch_assets `--check`, SKIP branch at `src/audio/gate.rs:199-205`, golden file readers, font_check
inputs). No mismatch.

## 1. What was implemented

| file | lines | what |
|---|---|---|
| `.github/actions/setup/action.yml` | 39 | composite: toolchain 1.95.0 + clippy, optional apt (alsa/udev/wayland/xkbcommon), `Swatinem/rust-cache@v2` (cache-on-failure), Python 3.12, optional asset cache + `fetch_assets.py` + `fetch_assets.py --check` |
| `.github/workflows/clippy.yml` | 31 | job `clippy`: workspace clippy + `-p gta_sim -p citygen` clippy, both `-D warnings` |
| `.github/workflows/sim-gates.yml` | 39 | job `sim`: `cargo test --locked --no-fail-fast -p gta_sim \| tee`, counts to step summary |
| `.github/workflows/citygen-gates.yml` | 37 | job `citygen`: same for citygen |
| `.github/workflows/client-gates.yml` | 40 | job `client`: apt + assets, `cargo test ... -p gta_like --bin gta_like` |
| `.github/workflows/repo-checks.yml` | 37 | job `repo`: fonttools 4.59.1, `tree_check.py`, `font_check.py`, `unittest tools/qa/test_brp.py` as separate steps |
| `README.md` | +18 | one badge line right after `<!-- SHOWCASE-HERO:END -->` (outside the markers); `## Что проверяет CI` between "Статус" and `<!-- COST:START -->` with counts |
| `AGENTS.md` | +3/-1 | CI mention in "Сборка"; post-merge CI check paragraph after the README-sync rule |

Shared header in every workflow: push (`branches-ignore: [media]`), pull_request, workflow_dispatch; `permissions: contents: read`;
concurrency `${{ github.workflow }}-${{ github.ref }}` with cancel-in-progress; env `CARGO_TERM_COLOR=always`,
`CARGO_INCREMENTAL=0`, `CARGO_PROFILE_{DEV,TEST}_DEBUG=0`. No secrets (AC5): only the implicit GITHUB_TOKEN.
Every `run:` has `shell: bash`; test steps also `set -o pipefail` explicitly.

## 2. Deviations from plan

- **`--no-fail-fast` added** to the three `cargo test` steps (fadeb1f). Flip (b) showed that plain `cargo test` stops at the first
  failing test binary (sim stopped at `city`), so a red run would hide the rest of the suite from the step summary, while Q2
  asks for the failing test list. Local proof it still reds through the pipe: `scratch/nofailfast_local.txt` (pipeline exit 101,
  all 6 citygen binaries ran). Run 3 on fadeb1f is green. Logged as a `decision` in log.jsonl.
- Nothing else. No Linux-only failures, so no follow-up task was needed (R1/Q2). No apt package was missing (R4). No disk
  trouble (R8). Bench gates passed on the 4-vCPU runner (R2).

## 3. Test / CI results

Rust code is unchanged. The local runs below are the unsabotaged baselines around the flips: client 77 passed, citygen green.
Real CI runs (https://github.com/pockerhead/MAW-make-GTA/actions/runs/<id>):

| run | commit | clippy | sim | citygen | client | repo | wall |
|---|---|---|---|---|---|---|---|
| 1 cold | c8deca3 | 36075605647 7m47s | 36075605435 9m26s | 36075607060 0m50s | 36075605394 16m21s | 36075605429 0m48s | **16m22s** |
| 2 warm | 583f275 | 36076975246 0m50s | 36076975200 7m13s | 36076975199 0m26s | 36076975240 1m51s | 36076975196 0m23s | **7m13s** |
| 3 warm | fadeb1f | 36079847684 1m01s | 36079847651 6m46s | 36079847643 0m26s | 36079847735 2m02s | 36079847625 0m23s | **6m47s** |

All green. Wall = first job start to last job completion across the five runs of one commit (`scratch/run{1,2,3}_timings.txt`).

Counts (derived from the CI logs with the same sed/awk as the summary step): gta_sim **378 passed, 2 ignored, 42 test binaries**
(unit + 40 integration + doc); citygen **32 passed, 3 ignored, 6 binaries**; client **77 passed**. These are in the README.

Cache evidence (N1/N5):
- Run 1: each rust job printed its own key: `v0-rust-{clippy,sim,citygen,client,repo}-Linux-x64-a972f308-670c1f85`, five
  different keys, and `... Saving cache ...` from the nested post-step. `gh cache list` shows all five saved (clippy 400 MiB,
  client 686 MiB, sim 350 MiB, citygen 165 MiB, repo 136 MiB). The asset cache `third-party-625c7c66…` was saved by `repo`
  (sim/client got "another job may be creating this cache", which is harmless).
- Run 2: every job logged `Restored from cache key "v0-rust-<job>-…" full match: true` and the asset jobs logged `Cache hit for:
  third-party-…` then `third-party packs match the manifest`. Build times went down: client 16m21s → 1m51s, clippy 7m47s → 0m50s.
  Sim is now bound by test execution, not the build: 1m30s compile, then about 5 min of tests (one binary takes 62-76 s, another 38-48 s).
- The nested composite `uses:` (rust-cache and actions/cache) saves correctly. The R6 fallback was not needed.
- Liveness of asset gates: `fetch_assets.py --check` ran before tests in sim/client/repo on every run. The client log has
  `mix_oggs_decode ... ok` with no `SKIP` line anywhere.

## Flip-RED at CI level (Step 7)

Predictions were written into this file before each push and derived from local runs (`scratch/flip_{a,b,c,d}_local.txt`).
Branch `ci/flip-red-029` from 583f275 got one sabotage per commit. Each commit reverted the previous sabotage, and each run
finished before the next push. **All four predictions held.**

| flip | sabotage | predicted red | CI red (run ids) | failing step / test |
|---|---|---|---|---|
| (a) b0df99f | `let unused_ci_flip = 1;` in `src/main.rs` main() | clippy | clippy 36077623939; the other 4 green | `error: unused variable: unused_ci_flip` (client gates stayed green, as a warning only) |
| (b) fb47b99 | seed 1 `0x31849ea45f8ec142` → `…143` in `crates/citygen/tests/golden_hashes.txt` | citygen, sim | citygen 36078556476, sim 36078556547; the other 3 green | `golden_hashes_match` (golden.rs:21), `runtime_hash_matches_golden` (city.rs:30); exit 101 went through `\| tee` (N6) |
| (c) 064b25b | `impactPunch_medium_000.ogg` → `_999.ogg` in `assets/audio/mix.ron` | client | client 36078809838; the other 4 green | `audio::gate::mix_oggs_decode` (`GATE BROKEN: …_999.ogg: No such file`), `mix_sounds_are_manifest_oggs`; 75 passed, 2 failed |
| (d) dceee08 | `toggle: "↔"` → `"↔☃"` in `assets/ui/strings.ron` | repo | repo 36079346675; the other 4 green | step `font_check` failed (U+2603 missing in both Inter fonts); `test_brp` skipped; client stayed green (77 passed locally and in CI) |

After the flips, `git push origin --delete ci/flip-red-029` ran and `git ls-remote --heads` confirmed the branch is gone.
The local branch is deleted too. Nothing from it was merged.
Its rust caches (branch-scoped) stay in the repo cache list until GitHub evicts them after 7 days without use. The read-only
`gh` cannot delete them. Current total is about 3.4 GiB out of 10 GiB.

## 4. How to verify manually / left for QA and the orchestrator

- Step 8 (after merge): the first `main` run is cold (N4), about 16-17 min, bound by client gates. Check that all five are `success`
  with `gh run list --repo pockerhead/MAW-make-GTA --branch main --limit 10`. The badges use `?branch=main`, so they show
  "no status" until that run completes. After that, open the README on GitHub and check that the five badges render and link to
  `actions/workflows/<file>?query=branch%3Amain`.
- I did not see the step-summary text with my own eyes: `gh` shows no job summaries, and check-run `output.summary` is null.
  The step ran green every time, and the same sed/awk pipeline was tested locally on colored and failed lines (it gives the
  same totals as above). QA: open one sim run page and look at the "Test counts" block.
- Notice from GitHub on every job: "ubuntu-latest will migrate to Ubuntu 26 beginning October 19, 2026". If the apt package
  names change, R4 applies (fix the apt line). Otherwise nothing to do now.
- R3: after the merge, `main` adds about 1.7 GiB of caches. If the total gets near 10 GiB, add `save-if` for main only
  (not in this task).

Scratch evidence: `scratch/run1_*.log`, `run2_*.log`, `run3_sim.log`, `run{1,2,3}_timings.txt`, `flip_*_local.txt`,
`flip_*_ci*.log`, `nofailfast_local.txt`, `wait_commit_runs.sh` (waits for all five runs of a commit).
