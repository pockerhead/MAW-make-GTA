# FIX_SUMMARY — TASK-016 fixer round 2 (QA bugs 1-5)

Status: **INCOMPLETE — verification interrupted.** All five fixes are written, and each has a gate that
passes and goes RED under its flip. The full `cargo test -p gta_sim -j 4` run was stopped by Claude Code
while it was still compiling, because the host ran critically low on memory. Per the harness notice I
did not restart it. Also not done: presentation gates ×3, t15 ×5, t11, commit, push and CI. The changes
are uncommitted in the working tree of `feature/t15-traffic`.

Inputs: `QA_REPORT.prev-1.md` bugs 1-5 (binding per the orchestrator note), `FIX_SUMMARY.prev-1.md`.

## 0. Preflight: the claim that would break correct code if applied verbatim
QA bug 1 suggests changing `driven_off` only at `car_route.rs:372` (the sense). But four other places
use the same distance-only test:
- the re-board timer (`car_route.rs:287`);
- the retry dismount (`:396`);
- `board_police_cars` (`cars.rs:283`);
- the cop's walk back to the door (`behavior.rs:317`).

With the sense alone, a car would drop its crew for a stopped driver 30 m away, and the crew would walk
straight back and re-board. That is the round-1 flip-flop again, just slower. The claim is real, so all
five sites now use one predicate.

## 1. Fixed

**Bug 1: a stopped or stuck driver is treated as "driven off".**
- New `PoliceCar::driven_off(cfg, driving, distance)` = `driving && stopped < stopped_seconds && distance > reboard_distance`.
- The threshold is data: `stopped` accumulates while the player's car is at or below `exit_max_speed`,
  so it has the 1 s `stopped_seconds` hysteresis.
- It is used at all five sites.
- Gate: new `tests/police_stopped_driver.rs`.
  - `a_stopped_driver_is_approached_and_busted`: 1★, hospital / plaza / park lanes. It asserts that no
    police car stands in Respond at ≤ exit speed beyond `reboard_distance` for `blocked_seconds + 1` s
    or more, and that the player ends BUSTED.
  - `a_driver_stuck_at_a_wall_is_approached`: 5★, wall in front, throttle held. Same wait assertion,
    and all 5 cars let their crews out.

| row | now | old condition (flip) |
|---|---|---|
| 1★ park | worst wait 1.97 s | 4.34 s → RED |
| 5★ wall | 2.08 s, 5/5 cars dismount | 53.6 s at 25.2 m, 3/5 → RED |

- `fix_chase.rs` re-run as `scratch/cr_probe/tests/fix_chase_r2.rs`, which also counts state
  transitions: 4 transitions and 2 dismounts in 25 s, so no flip-flop (round 1 had 1132).
- Side effect, for the owner: in that probe the player stopped 120 m from two cars that were held up in
  traffic, and both let their crews out there (the `blocked` rule now applies to a stopped driver just as
  it does to a player on foot).

**Bug 2: the pull-out deadlock.** Two changes:
- `pull_out` excludes every cop in `Arrest` from the exit-spot blockers. The pulling crew never blocks
  its own door.
- New data `arrest.pull_give_up_seconds: 3.0` (validated > 0). If the left door is still blocked after
  that (a wall, a bystander), the driver is pulled out at any clear exit and `ArrestAttempt` is reset.
  The foot arrest then starts over, and a cop at the far door never counts as a break-free (F7 kept).

The give-up is also what stops a forever retry: if no exit exists at all, the attempt restarts every
4 s, and in that case the player cannot get out either.

I rejected the alternative (cops step clear of the door spot). It needs a new motion rule, and walls
would still block forever.

Gates in `tests/police_pull_out.rs`:
- `a_second_arresting_cop_at_the_door_does_not_block_the_pull`: the second cop is in Arrest and held
  0.44 m off the door point (QA's measurement). Pulled out at the left door after `pull_out_seconds`,
  then BUSTED. Flip `pullers → &[]` → RED.
- `a_blocked_left_door_gives_up_to_another_exit` replaces `no_pull_through_a_blocked_left_door`. The
  old gate asserted "never pulled", which is exactly the infinite retry. The new one checks:
  - not pulled out before the give-up (±2 ticks);
  - pulled out at another exit, not at the blocked door;
  - heat unchanged for 20 s (no break-free);
  - attempt unbound afterwards.

  Flip `give_up = false` → RED.
- BUSTED is not asserted after a give-up. On this floor the cop loses sight behind the car and goes to
  Search, which is ordinary foot-arrest behaviour.

**Bug 3: the t15 hijack flake.** `tools/qa/scenarios/t15.py` waits for the player's `HitReaction ==
Steady` twice: after the car stops for him and before F. Not run yet (see status).

**Bug 4: a car standing inside a junction because of a pedestrian.** Two bounded root causes, both fixed
in `traffic/drive.rs`:
- (a) The forward cast started at the car's centre, so a walker pressed against the flank counted as a
  body in the way. The car and the walker then waited on each other. The cast now starts at the nose,
  and gaps for bodies ahead are the same as before.
- (b) A dynamic traffic car's autopilot target was `v + a·dt`, so the throttle was `a·dt·gain`: 0.3 m in
  5 s from rest. It is now `v + a/(acceleration·speed_gain)` while a > 0, so the throttle is
  `a/acceleration`. Braking is unchanged: any shortfall still brakes fully.

Gate: new `tests/traffic_pedestrian.rs`.
- `a_walker_at_the_flank_does_not_hold_the_car`: the walker presses at (0.88, −1.50) from the car
  centre, the QA sample. The car must move 8.13 m in 5 s against a limit of > 3 m, and the walker must
  be unhurt. Flips:
  - cast from the centre → 0.00 m, RED;
  - old speed target → 0.31 m, RED.
- `a_walker_ahead_of_the_bumper_still_holds_the_car`: a guard row, the car moves < 0.1 m.

Not measured again in the city (QA's 3-13 s). That is for QA or the owner run.

**Bug 5: the `cars` column is never tested.**
- New `cars_cap_binds_with_spare_units` in `police_cars.rs`. Named mutation: row 2 gets 12 units.
- GATE BROKEN unless there is a tick with 2 cars out and room for another crew.
- Flip `active < row.cars → <=`: RED at tick 1 ("3 cars > 2"). The other `cars_follow_row_*` rows stay
  green.

Docs:
- `docs/architecture/traffic.md` (cast start, dynamic target speed, `driven_off`, pull give-up).
- One line in GDD §6.4 (give-up).
- Comment in `escalation.ron`.

Log: 3 `decision` entries.

## 2. Skipped
- Nothing skipped on purpose. What is not done is verification (see the status line).

## 3. Test results (partial)
- `cargo clippy -j 4 -p gta_sim --all-targets -- -D warnings`: clean.
- Targeted runs, all green on the final code:
  - `--test police_pull_out`: 6/6;
  - `--test police_stopped_driver`: 2/2;
  - `--test traffic_pedestrian`: 2/2;
  - `--test police_cars cars_`: 6/6.
- Flip runs: each RED as listed above, files restored with `cp` from backup.
- **Not run:**
  - full `cargo test -p gta_sim -j 4` (killed during compile for host memory,
    `scratch/fixer2/test_sim_1.log` is empty of results);
  - `config_traffic` (a new sabotage row was added);
  - `cargo test -p gta_like --bin gta_like` ×3;
  - `t15.py` ×5 and `t11.py`;
  - the workspace clippy;
  - push and CI.
- Risk to check first:
  - bug 4 (b) changes how dynamic traffic accelerates, so `traffic_contact`, `traffic_bench`,
    `traffic_parked` and `sensor_leak` may move;
  - bug 1 makes cars stop and drop crews during t15's 1.5 s coast between W bursts, which can affect
    t15 step 3 ("the police cars close in").

children: 0 launched / 0 reported.
