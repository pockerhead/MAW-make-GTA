# FIX_SUMMARY — TASK-039, fixer round 2 (cleanup)

Acted on: the orchestrator note for round 2 and the last `OPEN_DECISIONS.md` entry (binding). They say to
remove the rules that round 1 showed have no observable effect when switched off: the slot rule (a), own
turn (b), the stalled release, the standalone past guard, and m1. I re-read `IMPL_REVIEW.md` only as the
source of those rules and did not re-open anything else in it. Commit `0edd136` on `feature/box-uturn`
(pushed). Evidence is in `scratch/fixer2/`.

Status: every rule is removed. The full suite is green on Windows. The Linux traffic rows are green in WSL
(before the orchestrator switched Linux verification to CI). Clippy, the client tests and `tree_check`
are green. CI: see §4.

## Preflight: the claim most likely to break correct code if applied verbatim

The claim: "the standalone past guard has no effect" (C1(ii)). Removing the guard is safe only if no edge
can point at a body that is already wholly behind its waiter. If one could, `upkeep` would end planning on
the first tick for a `Dynamic` car, and the review's C1 hold would come back.

I checked this in the code, and every edge source points ahead:
- `sense` hits and the path leader look forward.
- The junction `blocked` bodies lie on the connector ahead.
- `settle` promotes only a `Follow` target, which is a leader or an exit-lane car.
- `Stay` goes through `recover::blocker_ahead`. It builds `own` with `half.y = 0`, so it drops any body
  wholly behind the car's middle. That zone is strictly larger than `past`'s "every corner behind the
  rear".

So the guard is redundant with the `behind` filter, which stays. Round 1's `behind_off` flip is the gate
for that filter (a pinned `Dynamic` car stands 40 s without it). The suite is green without the guard.

## 1. Fixed (removed)

All changes are in `crates/gta_sim/src/traffic/progress.rs` unless stated otherwise.

| Rule | What was removed |
|---|---|
| Slot rule (a) | The `slots` list and the `taken` check in `accept`, and the `approach()` helper. |
| Own turn (b) | `due_waiters`, the `own_turn` drop in `candidates`, and the `on_path_ahead()` helper. `drive::successor` goes back to private: round 1 had widened it only for this rule. |
| Stalled release | `release_stalled()` and its call in `detect`. The field `World.dt` existed only for it (clippy flagged it as dead), so it is gone from the struct and from its construction in `drive.rs`. |
| Past guard (C1 ii) | `past(&rect(snap), &body.shape)` in the `candidates` drop. `past` itself stays, because the `upkeep` end rule uses it. |
| m1 | `settle` is back to the pre-round-1 rule, the same as round 1's `settle_all` flip: a `Follow` target that is not "accounted" (no edge of its own and not AI-driven, a non-traffic body included) becomes `Body` after `wait_seconds`. |
| Config / validator / sabotage | None of the five had its own config value, validator rule or `config_traffic` sabotage row (checked: `ProgressConfig` has only `wait_seconds`, `grace_seconds` and `max_seconds`, all still used). Nothing to remove. |
| Docs | Module doc and the `detect` / `accept` docs. `docs/architecture/traffic.md` Progress section: the own-turn, past-guard, slot and stalled-release sentences and the m1 "body off the path" clause are gone. The `upkeep` "past" line now says why it holds for a `Dynamic` car (every edge points ahead, `Stay` included). `docs/design/GDD.md` §5.2: the sentence about "the standing car goes first, one car per approach" is gone. |

Kept, as the note says: M3, the symmetric exemption, C1's behind filter and re-arm, M2, M6, the M4 oracle
checks, and the speed cap at `pass.speed`. I also kept the `nearer` precondition ("the squeezer is the car
nearest B"). It was part of the (a) design, but it is not the slot rule, was never flipped, and is not on
the removal list.

How it was verified: I removed the five together and ran the whole suite on the result. It is green, so
no rule needed to be put back and no bisection or new floor row was needed.

## 2. Skipped / notes

- **An observable difference that is not a gate failure (for the orchestrator and the owner run):**
  without the own-turn rule, M1 changes on both platforms.
  - The eastbound follower now also squeezes through D (1994) at 13.97 s, at the same time as S (1962).
  - Relaxations go from 3 to 4, and eastbound cars past go from 13 to 12.
  - The worst stand is unchanged at 18.2 s, and the depth is still 1.80 m.
  - This matches round 1's `own_turn_off` flip exactly. In the look, this is one more brush through the
    `Dynamic` car D.
- **Linux: most of it comes from CI.** I ran the traffic rows in WSL, then the orchestrator switched Linux
  verification to CI. The WSL run of the remaining targets was stopped part-way, with 440 passed and
  0 failed at that point (`scratch/fixer2/wsl/rest.log`, partial). After that I ran `wsl --shutdown`. The
  full Linux set is the CI "sim gates" run.

## 3. Test results

Windows (`-j 2`, in groups because of the 10-minute call limit; `scratch/fixer2/win_*.txt`). I ran it
after checking that the only running cargo processes belonged to other projects.

| Group | Result |
|---|---|
| `cargo test -p gta_sim -p citygen --lib` + every `gta_sim` test target except the heavy ones below (`win_suite_light.txt`) | 572 passed, 0 failed, 4 ignored |
| `traffic_causes` + `traffic_progress` (`win_heavy1.txt`) | 15 passed, 0 failed (R1 seed 1 worst 27.8 s; c) green) |
| `traffic_gridlock` + `traffic_junction_box` + `traffic_go_around` (`win_heavy2.txt`) | 23 passed, 0 failed (G4 seed 7 worst 24.8 s; gridlock seed 1 31.4 s under 40; `dummy_street_seed_7` 19.8 s) |
| `traffic_bench` / `police_bench` / `civilian_bench` (`win_bench.txt`) | 4 passed. Means: traffic 2.04 ms, SWAT 1.82 ms, civilians 1.99 ms |
| `cargo test -p citygen` (`win_citygen.txt`) | 32 passed, 0 failed, 3 ignored |
| `traffic_progress` + `traffic_progress_plumbing` (`win_progress.txt`) | 12 passed. M1 worst 18.2 s, depth 1.80 m. N1 worst 19.8 s, 6 relaxations, lift 0.018 m, unhurt |
| `cargo clippy --locked --workspace --all-targets -- -D warnings` (`win_clippy.txt`) | clean |
| `cargo test -p gta_like --bin gta_like` (`win_client.txt`) | 82 passed |
| `python tools/qa/tree_check.py` | passed |
| `cargo tree -p gta_sim -e normal -i bevy_render` | empty |
| `rustfmt --edition 2024 --check` on `progress.rs` and `drive.rs` | clean |

Linux, WSL Ubuntu 22.04, toolchain 1.95.0 (`scratch/fixer2/wsl/*.log`):

| Target | Result |
|---|---|
| `traffic_progress` | 2 passed. M1 18.2 s, N1 19.8 s |
| `traffic_progress_plumbing` | 10 passed |
| `traffic_causes` | 13 passed. `c_character_in_the_lane` green, with its 87 s stands 60 m away printed as before; R1 seed 1 27.8 s |
| `traffic_junction_box` | 7 passed |
| `traffic_gridlock` | 5 passed |
| `traffic_go_around` | 11 passed. `dummy_street_seed_7` 19.8 s |
| `traffic_bench` | 1 passed |
| Remaining targets | partial (440 passed, 0 failed) before the stop; the full set is CI "sim gates" |

## 4. CI (push of `0edd136` to `feature/box-uturn`)

At hand-back, 3 of 5 had passed and 2 were still running. The orchestrator must confirm the last two:
- clippy: success (https://github.com/pockerhead/MAW-make-GTA/actions/runs/36392719604)
- citygen gates: success (https://github.com/pockerhead/MAW-make-GTA/actions/runs/36392719648)
- repo checks: success (https://github.com/pockerhead/MAW-make-GTA/actions/runs/36392719833)
- client gates: in progress (https://github.com/pockerhead/MAW-make-GTA/actions/runs/36392719614)
- sim gates (Linux verification): in progress (https://github.com/pockerhead/MAW-make-GTA/actions/runs/36392719784)

Confirm with `gh run watch 36392719784` and `gh run watch 36392719614`.

children: 0 launched / 0 reported
