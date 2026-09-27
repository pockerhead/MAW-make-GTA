# FIX_SUMMARY — TASK-038, fixer round 2 (QA F1)

Fixer: claude opus, medium. Inputs: QA_REPORT.prev-1.md F1 and the last OPEN_DECISIONS.md entry (binding), and
IMPL_REVIEW.md (acted on in round 1, FIX_SUMMARY.prev-1.md). Code commit: 9ac877f on `bugfix/linux-traffic-gates`.

## Preflight

The most dangerous prescription in IMPL_REVIEW if applied verbatim is I2 ("swap `connector_rects` for
`body_sweep` shapes"). It would change repick/stuck behaviour, and the reviewer itself keeps it out of the hotfix.
It stays out: it is recorded for TASK-036 (round 1). For F1 I checked the prescription against the code first:
- `dynamic_bound_on` is still used by (c) (`traffic_causes.rs:~294`) and rb (`~531`), so it stays;
- `approach` in `abandoned_car` had no other use;
- `clock.dynamic_violation()` returns `Option<String>`, collected the same way as in (b) (line 217).

## 1. Fixed

- **QA F1 → `crates/gta_sim/tests/traffic_causes.rs` `abandoned_car`.** (a) is back on the city-wide
  `clock.dynamic_violation()`. The `approach` + `dynamic_bound_on("(a)", ..)` lines are gone. Module header:
  the "(a) ... the `Dynamic` bound covers the scene lane and its approach only" clause is removed, and the scoped
  list is now "(c) and rb". rb and (c) keep the scene scope, as decided.
- **`maw/tasks/pending/TASK-037/task.md`, "Added by TASK-038".** Removed "and in (a) on Linux", which is no longer
  true with the fix, and "(a)" from the acceptance item. It now reads "restore the full-city `Dynamic` 30 s bound in
  causes (c), rb and the G4 liveness rows".

Flip-RED of the re-anchored (a) gate (Linux, WSL Ubuntu-22.04, rustc 1.95.0, CI env, `-j 2`), from PowerShell:
`wsl -d Ubuntu-22.04 -- bash <scratch>/linux_test.sh -p gta_sim --test traffic_causes <filter> -- --nocapture`

| Run | Result | Log |
|---|---|---|
| Fix on, `a_left_car` (seeds 1, 7) | **GREEN**, 2 passed. Seed 1 worst stand 33.7 s (not `Dynamic`, nothing behind the car); seed 7 20.6 s | `scratch/fix2_linux_a_citywide_on.log` |
| Swept-body clause off (`graph.rs:314` → `\|\| (false && sweeps_touch(..))`), `a_left_car_seed_1 --exact` | **RED** at traffic_causes.rs:129: `seed 1: ["an AI car stood 71.0 s in Dynamic (> 30 s)"]`, the original CI message | `scratch/fix2_linux_a_citywide_nosweep.log` |
| Restore | `git checkout` on graph.rs + mtime touched; sha256 `f17725d2…` equals the pre-flip value | — |

The perturbed input is the mechanism under test (the conflict table's sweep clause), not the verdict.

## 2. Skipped

- IMPL_REVIEW I0-I5: handled in round 1 (FIX_SUMMARY.prev-1.md). Nothing new was asked of them this round.
- QA: nothing else was open. F1 was the only finding.

## 3. Test results

- Windows, `cargo test -p gta_sim -p citygen -j 2`: exit 0, **590 passed, 0 failed, 12 ignored** (sum over all
  `test result` lines); `a_left_car_seed_1/7 ... ok`. Log `scratch/fix2_win_full_suite.log`.
- `rustfmt --edition 2024 --check crates/gta_sim/tests/traffic_causes.rs`: clean.
- GitHub CI on 9ac877f: 5/5 success (section 4).

## 4. CI

All 5 workflows are **success** on branch head 9ac877f (`gh run list --repo pockerhead/MAW-make-GTA --branch
bugfix/linux-traffic-gates`):
- sim gates 36290019573: https://github.com/pockerhead/MAW-make-GTA/actions/runs/36290019573
- citygen gates 36290019531, client gates 36290019471, clippy 36290019589, repo checks 36290019498
  (`https://github.com/pockerhead/MAW-make-GTA/actions/runs/<id>`).

Main CI after the merge is still pending (the orchestrator merges).

## Paths

- `git status`: `metrics.md` (orchestrator's, pre-existing) plus this report and the new scratch logs. No code
  changes are left uncommitted.
- No game process, no containers.

children: 0 launched / 0 reported.
