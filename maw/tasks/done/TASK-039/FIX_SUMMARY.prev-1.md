# FIX_SUMMARY — TASK-039, fixer round 1

Acted on: `IMPL_REVIEW.md` (NEEDS_WORK: §6 design, C1, M2-M6, m1-m2) and the last `OPEN_DECISIONS.md`
entry (binding). Commit `d446e80` on `feature/box-uturn` (pushed). Evidence: `scratch/fixer/`
(Windows logs, `flips.txt` + `flip.py`, `wsl/` Linux logs and scripts, `qa/` runtime sessions).

**Status:** every gate green on Windows and on Linux (WSL Ubuntu 22.04, toolchain 1.95.0), including
`c_character_in_the_lane`, `dummy_street_seed_7` and `r1_car_left_in_the_box_seed_1`. R1 seed 1 is
un-ignored (27.8 s on both platforms, bound 30 s), so no residue had to be documented. Runtime stage 5
ran 2x per repro: no traffic car stood > 30 s. CI: see the end.

## Preflight: the review claim most likely to break correct code if applied verbatim

C1(i): "the `Stay` blocker ignores bodies wholly behind the car's rear". In `recover.rs`,
`nobody_coming` / `corridor_clear` feed both the reported blocker and the recovery decision
(`clear = coming.is_none() && corridor.ok()`). Filtering behind-bodies inside those functions would let a
`Dynamic` car recover to kinematic while a resting car still sits in its rest skin behind it. Checked in
code: real. So the filter is applied only to the reported blocker (`blocker_ahead`), and the recovery
decision is unchanged. The second risky prescription, §6(b) taken literally, puts M1's D -> P
before S -> D. The reviewer had already scoped it to W's own path, and the orchestrator made that scope
binding. The M1 row confirms S -> D goes first (13.97 s), then D -> P (17.8 s).

## 1. Fixed

| Review item | What was done |
|---|---|
| §6 (c) squeeze speed | `ProgressConfig.speed` is removed, along with its validator rule, the sabotage row and the RON key. The cap in `drive.rs` is now `cfg.pass.speed`. Row a) reads `pass.speed`. GDD §5.2 and `traffic.md` are updated. |
| §6 (a) one squeezer per blocker | `accept` keeps `slots` (blocker, approach = lane or connector's source lane, passer) from every relaxation, in both phases. A second passer from the same approach waits. `candidates` drops W when W's path leader is relaxed against B or overlaps B (W is not the nearest car). `release_stalled`: a planning relaxation whose car stands for a queue or a body (not a grant) for `grace_seconds`, with `tick - since >= grace`, keeps only its physics exemption. |
| §6 (b), scoped | `candidates` drops W -> B when B is itself due and stands on W's own path ahead (same segment with larger `s`, or `drive::successor`). M1 order is kept: S -> D first. |
| C1 (i) | `recover::blocker_ahead`: the reported `Stay` blocker skips bodies wholly behind the car. **Deviation:** "behind" means behind the car's middle, not its rear bumper. In the Linux `c_character` pile, yawed `Dynamic` cars still reported the car pressed at their rear bumper (2078 -> 2088, 1999 -> 2078; `scratch/fixer/wsl/c_dbg.log`). |
| C1 (ii) | `candidates` skips a candidate whose blocker is already `past` the car. Every accepted relaxation therefore has its blocker ahead, and the `past` end rule is safe for `Dynamic` cars. |
| C1 (iii) | A car whose relaxation is `physics_only` against the same blocker it now has a `Body` edge to becomes a candidate again. On acceptance it gets a fresh `since` and plans again. |
| C1 (iv) | New `TrafficStats.stalled_relaxed` counts relaxed AI cars standing past `wait_seconds`, per tick. The city rows print its maximum. |
| C1 new rows | `dynamic_car_pinned_between_two_parked_cars` and `dynamic_car_pinned_behind_a_dummy`: both spawn orders, the rear past the body ahead within 30 s, the `Dynamic` stand at most 30 s, never relaxed against the body behind, dummy unhurt. `squeeze_plans_again_when_left_in_the_way` covers C1(iii). All three are in `traffic_progress_plumbing.rs`. |
| M1 (review) stalled counted only for Body | Folded into (a). A planning relaxation whose car stands for any reason, a grant included, no longer counts as "passing" in `accept`. The Linux trace showed a grant-waiting passer starving 2078 for 18 s. |
| M2 | The waiter's own clock applies only when neither the edge target nor the sink is a person. New row `a_person_keeps_his_own_clock`: the car squeezes past the second dummy at 15.98 s after it steps in (wait 16 s). |
| M3 | `pass::waits_for_traffic` returns the moving car in the claim, the owner of a claim against it, or the oncoming car too close to stop (`cannot_stop` now returns it). `edge_of` turns such a `Pass { go: false }` into `Follow` to that car. A standing body in the claim keeps `Body -> obstacle`. |
| M4 | `third_body.rs` `record_relaxations` runs every tick in every `Footprints` user. A relaxation must start against a body that was standing (<= `hold_speed`, read from the previous record, before the step of the tick that set it) and apart from it (<= `dynamic_tolerance` deep). The pair must leave the relaxed set within `max_seconds + SEPARATION_S` (10 s). `assert_clean` and the city rows (`scene_failures`, R1, G4, gridlock, go-around) fail on it. The oracle row `oracle_checks_the_start_and_end_of_a_relaxation` shows each check firing: moving blocker, start inside the body, held past a shrunk limit, and a clean control. |
| M5 cycle | Not built, per the orchestrator's condition. After the fixes `c_character` is green on Linux. Its cycles were `Stay` edges to cars behind, which `Stay` no longer reports. `traffic.md` records this. |
| M6 in-box first | Kept. Without it R1 seed 1 stands 41.0 s; with it 27.8 s (`scratch/fixer/r1s1_win_no_m6.txt`). New floor row `traffic_junction_box::a_waiter_in_the_box_behind_a_body_holds_nothing`, with a no-body control. |
| m1 | `settle` turns `Follow` into `Body` only for a target that is a traffic snap (a bailing car). A parked car or other body at an exit-lane start stays `Follow`. |
| m2 | `recover_dynamic` runs `nobody_coming` / `corridor_clear` only for a car at rest, and runs the blocker search only when it is not clear. |
| m3 (partly) | `upkeep` returns early when no car is relaxed or marked. `overlap` is computed after the cheap drops. |
| m4 | `contact::rim` / `past` / `touches` are shared. `progress.rs` lost its copies and `Vec` allocations. |
| m5 | `detect` is split into `settle`, `release_stalled`, `candidates` and `accept`. |
| m6 | `traffic_progress.rs` (city rows, 259 lines) and `traffic_progress_plumbing.rs` (oracle + mechanism rows, 716 lines) are separate. Shared helpers moved to `tests/traffic_support/progress.rs` (`pub mod`, imported only by the progress files, so other test binaries get no unused-import warning). |
| Nits | `Reasons.leader` no longer carries a speed. The `rank` doc names the tie order. The config comment is fixed. |

**Found while re-measuring on Linux; not in the review (the second patch on `c_character`, and the one that closed it).**
After (a)-(c) and C1, `c_character_in_the_lane` still stood 36.5 s in `Dynamic` on Linux. The cause:
the passer 2088, relaxed through 2078, stopped inside 2078's nose at its stop line and waited 20 s for a
grant. 2078's recovery saw 2088 in its corridor. Fix: the relaxed pair is exempt both ways.
- `recover_dynamic` ignores the car's own passers (`squeezes` in `drive.rs`).
- The contact switch (predictive and backstop) skips the pair on either side.
- D2 already said that neither body of the pair pushes the other; the switch was one-sided before.

The same trace also moved "a relaxed car waiting for a grant" out of `active`.

## 2. Skipped / not independently observable

- **(a) slots, (b) own turn, (a).3 release_stalled.** Built as the binding design, but no row goes RED
  without them. Flipped on Windows (M1, N1, all `traffic_causes`) and on Linux (`c_character_in_the_lane`):
  all stay green once the symmetric exemption is in. They are defence in depth for the starvation
  mechanism. The orchestrator may keep them or ask for a dedicated floor pile row.
- **C1 (ii) past guard.** Flipped off alone, it stays green: the `behind` filter already keeps behind
  bodies out of `Stay`. With the filter off the guard does act: the car is never relaxed against the body
  behind, but it stays stuck (flip `behind_off`).
- **M3.** Report-level only. With the `Follow` edge off, Linux `dummy_street_seed_7` stands 27.8 s,
  against 19.8 s with it (bound 30). The original 30.1 s regression came mostly from the 3 m/s squeeze,
  which (c) removed.
- **m1.** Report-level only: `traffic_gridlock` has 0 relaxations with and without it.
- **§4 "hijacked relaxed car" row.** Not built. `upkeep` ends planning when `!ai`, and the pair stays
  exempt until the footprints separate (row b gates that end rule). The player's car is not AI, so it is
  not a waiter.
- **§4 "Dynamic relaxed car through a character".** Covered by `dynamic_car_pinned_behind_a_dummy`:
  dummy unhurt, no knock-down, no lift.

## 3. Test results

Windows (`-j 2`, run in groups because of the 10-minute call limit; files `scratch/fixer/win_suite_*.txt`):
- `cargo test -p gta_sim -p citygen`: **636 passed, 0 failed, 7 ignored** (630 before, plus R1 seed 1
  un-ignored and the new rows). The M2 row was added afterwards and passes on its own.
- `cargo clippy --locked --workspace --all-targets -- -D warnings`: clean (one `manual_contains` fixed).
- `cargo test -p gta_like --bin gta_like`: 82 passed.
- `python tools/qa/tree_check.py`: passed. `cargo tree -p gta_sim -e normal -i bevy_render`: empty.
- Benches:
  - traffic mean 2.13 ms, p50 2.16;
  - SWAT mean 1.96 ms;
  - civilians: passed in the suite (one earlier run failed at mean 10.5 ms with p50 2.58 ms and max 258 ms,
    while Defender used about 2 cores; the rerun passed).
- `rustfmt --check` on every touched file: clean.

Linux (WSL, `scratch/fixer/wsl/*.log`, same final code as the commit):
- Full `gta_sim` + `citygen`: **637 passed, 0 failed, 7 ignored**, including every target run on its own
  and `group_c.log`. Before the symmetric-exemption fix `c_character` had failed there (36.5 s).

Key rows (Windows = Linux where both ran):

| Row | Result |
|---|---|
| M1 | worst stand 18.2 s (D `Dynamic` 18.2); order S->D 13.97 s, D->P 17.8 s, N->P 20.75 s; depth 1.80 m; 8 / 13 past per lane |
| N1 | worst stand 19.9 s; 6 relaxations against the player; lift 0.018 m; unhurt; depth 1.45 m |
| R1 seed 1 | worst 27.8 s; `Dynamic` 16.3 s (un-ignored) |
| R1 seed 7 | worst 16.9 s |
| G4 seed 7 liveness | worst 24.8 s (bound 40) |
| b3 | rear past the dummy at 18.7 s (removed at 28 s) |
| Linux `c_character` | green; 87 s stands 60 m away are printed, as on HEAD |
| Linux `dummy_street_seed_7` | 19.8 s (was 30.1 s) |

Flips (`scratch/fixer/flips.txt`; input perturbed, RED seen, source restored by the script):

| Flip | Result |
|---|---|
| `behind_off` | pinned between two parked cars, body behind spawned first: RED (40 s `Dynamic`) |
| `rearm_off` | `squeeze_plans_again_when_left_in_the_way` RED |
| `cap_off` | row a) RED (7.21 m/s > 6) |
| `own_clock_person` | `a_person_keeps_his_own_clock` RED |
| `blocked_holds` (waiter held at its exit start pushed into `blocking`) | M6 row RED |
| `symmetric_recover_off` | Linux `c_character_in_the_lane` RED (36.5 s) |
| M6 reverted | R1 seed 1 RED (41.0 s) |
| oracle relaxation checks | each check RED against its own case (moving 7.73 m/s, 1.5 m deep, held 65 ticks > 64) |
| `past_guard_off`, `slots_off`, `own_turn_off`, `release_off`, `pass_wait_off`, `settle_all` | green; see §2 |

Runtime stage 5 (release dev build, `--settings-id` QA by the harness, one game at a time, BRP screenshots
one at a time; `scratch/fixer/qa/`):
- `m1_fixed.py` (TASK-040 bot), 2 runs:
  - run 1: the car jammed on a lamp post 25.7 m from the spot;
  - run 2: car entry failed;
  - no verdict from either (harness).
- `m1_snap.py` (new, deterministic): BRP puts two parked cars across both lanes at (34.9, -76.9) and
  (34.0, -79.2), named mutation. Worst stand 14.0 s in both runs, 1 relaxation each (westbound car
  through Q at about 16 s), 0 stands > 30 s. Brush-through screenshot:
  `scratch/fixer/qa/m1_snap_1/shots/0008_0020s_m1_relaxed.jpg`.
- `repro_player_in_junction.py`, 2 runs: in-box max stop 15.4 s in both (TASK-040: 74.2 s), 20-22 samples
  with a car moving through the box, player health full.
- `repro_abandoned_car.py`, 2 runs: bot failure again (run 1 car entry failed, run 2 left the car 27 m
  short). `m1_snap.py <out> r1` puts a parked car in the middle of box 83 with the player 31 m away:
  worst stand 14.0 / 17.0 s, 25 / 28 relaxations, 0 stands > 30 s.
- No car visibly drives through a third body in the screenshots. At this distance that is a weak check;
  the headless third-body oracle carries it.
- The game was shut after each session (the harness closes it); no `gta_like` process is left.

CI: see the last section (filled after the run).

## Owner checklist (not gated)

- Squeeze look at `pass.speed` (6 m/s, was 3): M1 (D rejoins through P, depth about 1.8 m), a car past the
  player on the crosswalk (1.45 m), the box car in R1 (25-28 squeezes in 150 s).
- The symmetric exemption: a blocker recovered while its passer is still inside it may stand overlapped for
  a moment (contacts are off between them).

## Files

Production: `traffic/{progress,recover,contact,drive,pass,config,mod}.rs`, `assets/traffic/traffic.ron`.
Gates: `tests/traffic_progress.rs`, `tests/traffic_progress_plumbing.rs` (new), `tests/traffic_support/{progress.rs (new), third_body.rs, mod.rs}`,
`tests/{traffic_causes, traffic_junction_box, traffic_go_around, traffic_gridlock, config_traffic}.rs`.
Docs: `docs/architecture/traffic.md` (Progress section), `docs/design/GDD.md` §5.2.
`junction.rs` is unchanged by this round (M6 kept as the implementer left it).

## CI (push of `d446e80` to `feature/box-uturn`)

`gh run list --repo pockerhead/MAW-make-GTA --branch feature/box-uturn`:
- clippy: success (run 36385751818);
- client gates: success (run 36385751797);
- citygen gates: success (run 36385751805);
- repo checks: success (run 36385751769);
- sim gates: still in progress after 22 min when last checked (run 36385751765,
  https://github.com/pockerhead/MAW-make-GTA/actions/runs/36385751765). The full suite was green on Linux
  (WSL, toolchain 1.95.0) on the same code. The orchestrator should confirm the final status with
  `gh run watch 36385751765`.

children: 0 launched / 0 reported
