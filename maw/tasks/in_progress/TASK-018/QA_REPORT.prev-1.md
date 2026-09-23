# TASK-018 QA report

Verdict: **NEEDS_FIXES**. All four acceptance criteria pass as written. But this diff still ships a regression. It is the review's issue 1, and the fix closed only one of its paths.

## 1. Environment

- Direct cargo in `D:/test-gta-like`, branch `bugfix/ledge-assist-angle-and-space`, HEAD `a216237`. No docker or mocks. One cargo command ran at a time, all with `-j 4`.
- Commands:
  - `cargo build -j 4`
  - `cargo clippy --workspace --all-targets -j 4 -- -D warnings`
  - `cargo test --workspace -j 4`
  - `python tools/qa/scenarios/t1.py --out maw/tasks/in_progress/TASK-018/scratch/qa/t1`
- Independent probes are in `scratch/qa/`:
  - `qa_probe.rs` is a headless test that uses the production `headless_app()` / `compose_sim`.
  - `run_probe.py <variant> [filter | --target=ledge]` copies the probe into `crates/gta_sim/tests/`, swaps `ledge.rs` for a variant and runs it. It then restores `ledge.rs` and checks sha256 `8444bced…7864`, and deletes the probe. The restore was checked after every run, and `git status` stays clean apart from this report.
  - Variants:
    - `head`
    - `old` is `2ade7af`, before the task.
    - `c03d819` is the implementer's version.
    - `oldgate` puts the angle-gated barrier back.
    - `nobarrier`
    - `nofree` removes the free-space check.
    - `noreanchor` puts the pre-fix re-anchor guard back.
    - `norefresh` removes `assist.remaining = window` from the barrier.
  - Output goes to `scratch/qa/probe_*.txt` and `flip_*.txt`.
- Runtime: `t1.py` built and launched the `dev` client over BRP. The shutdown passed and no `gta_like` process was left running.

## 2. Test results

- Build: OK. Clippy with `-D warnings`, workspace and all targets: green.
- `cargo test --workspace`: 17 of 17 green, no failures:
  - lib: 1
  - config: 2
  - jump: 3
  - ledge: 5 (2 old, plus 3 new: the angle table, the ceiling test and the crate re-jump test)
  - movement: 4
  - terrain: 2
- Flip-RED, rerun by me on the committed tests:
  - `oldgate`: `configured_limit_blocks_oblique_approaches` goes RED, "above-limit climbs at [62.0]". Only the 62° row goes red.
  - `nofree`: `low_ceiling_blocks_pull_up_snap` goes RED.
  - `noreanchor`: `landing_on_crate_allows_next_jump_over_short_step` goes RED. The other tests stay green in each case. All three were GREEN again after the restore.
- My own probes:
  - **Angle sweep**. Ledge 1.3 m, `max_height` 1.2, angles 0/30/45/62/70/75/80°, jump at 0.4/0.7/1.0/1.5 m from the wall.
    - HEAD: nothing climbs in any row.
    - `nobarrier`: every airborne approach from 0° to 75° climbs. This shows 1.3 m is inside the natural Tnua reach, so the test is real.
    - `oldgate`: 62°, 70° and 75° climb when the jump is at 1.0 m or closer.
    - HEAD never pushed the capsule into the wall. The smallest center-to-face gap was 0.19 m at 75°, where the capsule's rounded part rests on the edge.
  - **Ceiling sweep**. Ledge 1.4 m, slab gap 0.9 to 3.0 m. Maximum capsule penetration into the slab:

    | gap | HEAD | `nofree` |
    |---|---|---|
    | 1.25 m | 0.000 | 0.300 |
    | 1.49 m | 0.018 | 0.248 |
    | 1.6 m | 0.000 | 0.138 |

    At gaps of 1.85 m and up, HEAD still climbs, so the check does not block the snap when there is room. At 1.75 m both variants show 0.05 m of penetration. That is Tnua's float spring pressing into the slab, not the snap.
  - **Crate re-jump**. Crate 0.8 m, wall 1.5 m behind it, so a 0.7 m step from the crate. See bug B1.
- Runtime `t1.py`: passed.
  - Movement Δz = -4.39.
  - Yaw Δ = -0.419.
  - FPS 30.1 with `Fifo` vsync on this host's 30 Hz display. That is the refresh rate, not a measure of cost.
  - Shutdown passed.
  - Screenshot `scratch/qa/t1/t1.png`: the blue capsule stands on the grey floor, with the ramp and the stairs of the test arena visible and no artifacts. t1 is a liveness check only; it does not touch ledges.

## 3. Acceptance criteria

| Criterion | Test performed | Result |
|---|---|---|
| Ledge above the lowered limit is not climbed at 0/45/62/75° (table), flip-RED by restoring the angle gate | Committed table is green. It goes RED under `oldgate` at 62°. My sweep shows HEAD blocks every angle from 0° to 75° at every jump distance, and `oldgate` climbs at 62/70/75° from ≤1.0 m | PASS, with a test gap (B2) |
| Reachable ledge under a slab with less than capsule height of room does not snap into the slab, flip-RED by removing the check | Committed test is green and goes RED under `nofree`. My sweep: HEAD penetration ≤0.018 m, `nofree` 0.14–0.30 m | PASS |
| All existing gta_sim tests stay green, clippy green, t1.py passes | 17/17 green, clippy green, t1 passed (see above) | PASS |
| Existing tests pass | `cargo test --workspace` | PASS |

## 4. Bugs found

### B1. major: a character that lands on a crate stays blocked from a climbable step (review issue 1 only partly fixed)

The fix re-anchors `launch_feet_y` only on `ActionStarted`, and only while `standing_on_entity()` is `Some`. The barrier still refreshes `assist.remaining` on every tick it fires (`ledge.rs:93`). A character that lands on an intermediate surface inside the window, next to a wall whose top is more than `ledge_assist_max_height` above the ground launch, gets these results:

- The barrier fires every tick while they press toward the wall. `position.y` (1.83) is above `top_y` (1.5), and `rise` is still measured from the ground.
- Each firing extends the window, so the window never expires.
- Walking up the 0.7 m step never happens. Before the task, the window expired and the character stepped up.
- A jump pressed a few ticks before landing on the crate (buffered) starts on the landing tick. At that tick `standing_on` is still `None`, so there is no re-anchor and the jump is pushed back.

To reproduce: run `python scratch/qa/run_probe.py <head|old> qa_crate_walk`. The runs last 400 ticks.

| Case | HEAD | pre-task 2ade7af |
|---|---|---|
| Jump onto crate, then only hold forward | never climbs | climbs at tick 108 |
| Jump tapped at tick 28 (buffered, fires on landing) | never climbs | climbs at tick 76 |
| Jump tapped at tick 60, after landing | climbs at tick 93 | climbs at tick 93 |

The trace (`qa_crate_trace`) shows HEAD creeping against the wall at z ≈ -2.2 to -2.56 at 0.47 m/s through tick 130. At the same point the old code steps up at ticks 78–87.

- Expected: a 0.7 m step reachable from where the character stands is not blocked, as before the task.
- Actual: the character is held off the step for as long as they push. Only a fresh jump pressed after landing gets them over.

The committed regression test only covers that last case (re-tap after standing), so it cannot see this.

Direction (diagnosis verified, the exact fix is up to the fixer): dropping the refresh does not work. My `norefresh` probe fails the committed angle table and climbs the 1.3 m ledge at 0° to 75°, because Tnua's natural reach takes over once the window lapses. So the refresh is load-bearing. The anchor has to follow the surface the character actually stands on. For example, re-anchor, or end the window, whenever the basis is grounded, not only on `ActionStarted`. Add two headless gates:
- walk-only from the crate
- buffered tap on landing

Both must be RED on `a216237`.

### B2. minor (test gap): the 75° row of the committed angle table does not test anything

`configured_limit_blocks_oblique_approaches` jumps when `z < -0.8`, which is 1.2 m from the wall face. At 75° the character reaches the wall on the ground, after landing. `reached_wall` (`z < -1.6`) is then satisfied on the ground, so the row passes with or without the barrier. My sweep shows the old gate climbs at 75° only when the jump is at ≤1.0 m. The fixer's note admits this. Review issue 4 is still open.

Fix: set the jump distance per row so the approach is airborne (for example, jump at a perpendicular distance of ≤0.7 m), or assert that the character is airborne when it reaches the wall. Record RED per row.

### Observations, no action required

- Review issue 5 (the free-space query also hits sensors) is unchanged. There is no fixture for it yet, so it stays a T-mission note.
- `git ls-files --eol` reports `w/mixed` for `ledge.rs` while every line ends in CRLF. This is harmless.
- Owner-feel items belong to the owner's run: the one-tick pull-up (B7), and sliding along a too-high wall now that the tangential velocity is kept (review issue 2, verified in the code at `apply_ledge`, and the angle sweep shows motion along the wall is kept).

## 5. Verdict

**NEEDS_FIXES.** The two spec items work and both have honest flip-RED gates. The build, clippy, the tests and t1 are green. But the self-refreshing barrier window still pins a character on a crate against a step they could climb before this task. It fails in both cases: walking, and a buffered jump on landing. This is a silent gameplay-state regression caused by this diff, and the review asked for exactly this path to be fixed. The committed crate gate misses it. B2 should be fixed in the same pass.

Owner checklist for after the fix: `cargo run --features dev`, jump at a too-high wall from the front and at an angle, and judge the push-back and the slide along the wall. Jump onto the arena ramp or box next to a higher surface and check you can step or jump up. Judge the one-tick pull-up (B7).

Services started: none persistent. The t1 client was shut down over BRP, and no process was left running.
