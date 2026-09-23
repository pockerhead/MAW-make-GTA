# TASK-018 QA report, round 2

Verdict: **NEEDS_FIXES**.

Both spec items still work, and their gates were flipped RED by me. The round-1 regression B1 is closed for the geometry the fixer gated (0.8 m crate, 2 m deep). Round-1 finding B2 is closed. But the same class of bug (stale launch anchor plus a barrier window that refreshes itself forever) still reproduces when the crate is narrower than `capsule_radius + ledge_assist_clearance` (0.35 m). That case is B3 below.

## 0. Disconfirmation (done first)

Counter-example I wrote down before testing: the round-2 re-anchor casts **one ray down from the capsule centre** and accepts the height only if that ray hits the Tnua support entity at `float_height ± 0.05` (`ledge.rs:40-50`). Tnua stands on a **cylinder sensor** of radius 0.29 (`character/mod.rs:97`). So Tnua can stand on a surface the centre ray misses. The barrier holds the centre at `r + clearance = 0.35 m` from the wall face. A step flush against the wall and shallower than 0.35 m therefore holds the body while the ray hits the ground below it. The anchor then stays at the ground, `rise` stays above the limit, the barrier keeps refreshing the window, and the character is pinned the same way as in B1.

**Result: the counter-example held.** See B3.

## 1. Environment

- Direct cargo in `D:/test-gta-like`, branch `bugfix/ledge-assist-angle-and-space`, HEAD `9a170d2`. No docker, no mocks. One cargo command at a time, `-j 4`.
- Commands:
  - `cargo build -j 4`
  - `cargo clippy --workspace --all-targets -j 4 -- -D warnings`
  - `cargo test --workspace -j 4`
  - `python tools/qa/scenarios/t1.py --out maw/tasks/in_progress/TASK-018/scratch/qa/t1r2`
- Probe harness `scratch/qa/run_probe.py <variant> [filter | --target=ledge]`. It swaps `ledge.rs` for a variant and copies `qa_probe.rs` into `crates/gta_sim/tests/`. After the run it restores the source, checks it against sha256 `6e8ac79a…fe62d`, and deletes the probe. The restore was checked after every run and `git status` stays clean.
- Variants:
  - Carried over from round 1: `head`, `old` (`2ade7af`, before the task), `oldgate`, `nofree`, `norefresh`.
  - New this round: `nosupport` deletes the round-2 support re-anchor block (`ledge.rs:40-51`).
- New probe `qa_geo_sweep` / `qa_geo_trace` in `scratch/qa/qa_probe.rs`:
  - A crate of height `ch` and depth `cd` sits flush against a wall of height `wh`, with the wall face at z = -3.
  - The character runs at it from z = 1 and jumps at tick 0.
  - Five input paths:
    - walk-only
    - buffered tap at tick 28
    - buffered tap at tick 32
    - fresh tap at tick 70
    - jump held throughout
- Round-1 probe outputs were copied to `scratch/qa/r1/`.

## 2. Test results

**Existing and committed suite.** `cargo test --workspace` gives 19/19 green:
- lib: 1
- config: 2
- jump: 3
- ledge: 7
- movement: 4
- terrain: 2

The 14 pre-existing `gta_sim` tests are all present by name and green. Clippy `-D warnings` on the whole workspace, all targets: green. Build: OK.

**My flip-RED of the committed gates** (`--target=ledge`, outputs in `scratch/qa/flip_<variant>.txt`). In each case the other 5 or 6 tests stayed green, and GREEN came back after the restore (sha verified).

| Perturbation | RED test(s) |
|---|---|
| `oldgate` (angle-gated barrier restored) | `configured_limit_blocks_oblique_approaches`: "above-limit climbs at [62.0, 75.0]". The 75° row is now load-bearing, so round-1 B2 is closed. |
| `nosupport` (support re-anchor deleted) | `walking_from_crate_clears_short_step` and `buffered_jump_from_crate_clears_short_step` |
| `nofree` (capsule intersection deleted) | `low_ceiling_blocks_pull_up_snap` |
| `norefresh` (barrier window refresh deleted) | `configured_limit_blocks_oblique_approaches` [62, 75]. This confirms the refresh is load-bearing for the barrier. |

**Round-1 crate probes, re-run** (`probe_head_full.txt` vs `probe_old.txt`). Crate 0.8 m, 2 m deep; wall 1.5 m.

| Path | HEAD 9a170d2 | pre-task 2ade7af | round-1 HEAD a216237 |
|---|---|---|---|
| walk-only | tick 78 | 108 | never |
| buffered tap 28 | 90 | 76 | never |
| tap after landing 60 | 73 | 93 | 93 |
| jump held | 78 | 108 | n/a |
| single tap at every tick 25..109 | 0 failures | 0 failures | n/a |

On the committed geometry B1 is fixed.

**Angle sweep.** Ledge 1.3 m, `max_height` 1.2, angles 0/30/45/62/70/75/80°, jump gaps 0.4/0.7/1.0/1.5 m.
- HEAD: no climb in any row.
- The closest the capsule centre came to the wall face was 0.19 m at 75°, where the rounded part rests on the edge. It was never pushed into the wall.
- The barrier holds at 0-75°.

**Ceiling sweep** (HEAD):

| Gap | Result |
|---|---|
| 1.25 m | max penetration 0.000 |
| 1.49 m | max penetration 0.018 |
| 1.6 m | max penetration 0.000 |
| ≥ 1.85 m | still climbs |

This is the same as round 1.

**New geometry sweep** (`qa_geo_sweep`). Clear tick; `None` means the character did not clear the wall in 400 ticks.

| crate / depth / wall | path | HEAD | 2ade7af |
|---|---|---|---|
| 0.9 / 0.25 / 1.6 | walk-only | **None** | 116 |
| 0.9 / 0.25 / 1.6 | buffered 28 | **None** | 116 |
| 0.9 / 0.25 / 1.6 | buffered 32 | **None** | 145 |
| 0.8 / 0.25 / 1.5 | buffered 32 | **None** | 147 |
| 0.5 / 0.25 / 1.45 | buffered 32 | **None** | 150 |
| 0.8 / 0.35 / 1.5 | jump held | None | None (pre-existing, not a regression) |
| depth ≥ 0.35, all heights | all paths | climbs | climbs |
| crate 0.6 / wall 1.3 (rise 1.3 < 1.4 limit) | all paths | climbs | climbs |

Every row where HEAD fails and 2ade7af climbs has depth 0.25 m, below the 0.35 m threshold.

**Runtime.** `t1.py` passed. Output is in `scratch/qa/t1r2/summary.json`:
- movement Δz -4.39
- yaw Δ -0.419
- FPS 30.17 under `Fifo` vsync on this host's 30 Hz virtual display (display refresh, not frame cost)
- `shutdown: passed`

I looked at the screenshot `scratch/qa/t1r2/t1.png`. The blue capsule stands upright on the grey floor with its shadow. The arena ramp is on the left and the stairs block is on the right. There are no artifacts and nothing is buried. No `gta_like` process was left running (checked with `tasklist`). t1 is a liveness check and does not exercise ledges.

## 3. Acceptance criteria

| Criterion | Test performed | Result |
|---|---|---|
| A ledge above the lowered limit is not climbed at 0/45/62/75° (table); flip-RED by restoring the angle-gated barrier | The committed table is green. My `oldgate` flip turns it RED at 62° and 75°. My sweep over 0-80° and 4 jump gaps shows no climb at HEAD. | PASS |
| A reachable ledge under a slab with less than capsule height of room does not snap into the slab; flip-RED by removing the free-space check | The committed test is green; my `nofree` flip turns it RED. Ceiling sweep penetration ≤ 0.018 m, and the ledge still climbs when there is room. | PASS |
| All existing gta_sim tests (14) stay green; clippy green; t1.py passes | 19/19 green (the 14 old ones by name), clippy workspace all-targets green, t1 passed and the screenshot was checked | PASS |
| Existing tests pass | `cargo test --workspace` | PASS |
| Implicit (orchestrator bar for this round): no regression against 2ade7af on the walk-off-crate and buffered-landing paths | Round-1 probes: PASS. New geometry sweep: FAIL on crates < 0.35 m deep (B3) | **FAIL** |

## 4. Bugs found

### B3. major: the stale-anchor pin still happens on narrow steps (same class as B1)

Mechanism, verified in the code and a trace:
- `ledge.rs:40-51` re-anchors `launch_feet_y` only when a ray **from the capsule centre straight down** hits the Tnua support entity at `float_height ± ledge_assist_clearance`.
- Tnua's sensor is a cylinder of radius 0.29 (`character/mod.rs:97`). It holds the body on a step the centre ray does not touch.
- The barrier (`ledge.rs:105-116`) keeps the centre at `capsule_radius + clearance = 0.35 m` from the wall face. So on any step flush against the wall and shallower than 0.35 m, the centre ray hits the ground and the anchor stays at the ground.
- `rise` stays above `ledge_assist_max_height` and the barrier fires every tick. `assist.remaining = window` keeps the window alive forever. The one ActionStarted re-anchor then needs a fresh jump started while standing.

Reproduce: `python maw/tasks/in_progress/TASK-018/scratch/qa/run_probe.py head qa_geo_trace` (output in `scratch/qa/probe_head.txt`). Setup: crate 0.9 m high and 0.25 m deep, spanning z -2.75..-3, flush against a 1.6 m wall. The character jumps from the ground and then holds forward only. From tick ~150 to tick 400 it sits at `pos=(25.0, 1.951, -2.654)` with `vel.z = -0.47`:
- y = 1.95 is the crate top plus `float_height`, so Tnua is standing on the crate.
- z = -2.654 is 0.346 m from the face, so the centre is off the crate's footprint.

It never clears. `2ade7af` clears the same input at tick 116. Buffered tap 32 on 0.8/0.25/1.5 behaves the same way, pinned at y = 1.85.

- **Expected:** a 0.7 m step above the surface the character stands on is climbed by walking or by a buffered jump, as before the task.
- **Actual:** the character is held off the step for as long as the player pushes. Only a fresh tap after landing gets over (tap70 climbs at HEAD).

Why it matters: a 0.25 m deep step, bench, curb or planter against a wall is ordinary city geometry for T2. The committed gates cannot see this, because the crate is 2 m deep, so the centre is always over it.

Direction (the diagnosis is verified; the fix is for the fixer or planner to decide):
- The anchor has to follow what Tnua actually stands on, not what the centre ray hits. For example, use Tnua's own proximity-sensor output (the sensor hit point or height) or a shape cast with the sensor cylinder instead of a centre ray.
- Or let the barrier not keep itself alive while the character is grounded.
- Add one headless gate with a 0.25 m deep crate (walk-only and buffered-32). It must be RED at `9a170d2`.

Per `OPEN_DECISIONS.md`: this is round 2 failing on the same class. The orchestrator's pre-committed flip says stop patching and re-plan the ledge assist as a full-mode task.

### Observations, no action required

- Walk-only from a 1 m deep crate climbs slower than before (137 vs 118 ticks) but still climbs. From a 2 m crate it is faster (78 vs 108). Owner feel.
- Review issue 5 (the free-space query also counts sensors) is unchanged. There is no fixture for it yet, so it stays a note for the T-mission task.
- `nosupport` flip: the committed crate gates are the only tests pinning the re-anchor. They cover the deep-crate case only (see B3).

## 5. Owner checklist (subjective, cannot be gated)

`cargo run --features dev`, then:
- Jump at a wall that is too high, from the front and at an angle. Judge the push-back and the slide along the wall.
- Land on a box next to a higher surface, then walk or jump up.
- Judge the one-tick pull-up (B7).

## 6. Verdict

**NEEDS_FIXES.**
- Spec items 1 and 2 pass with honest flip-RED gates (I reproduced the RED on all four).
- Build, clippy, 19/19 tests and t1 (runtime plus screenshot) are all green.
- Round-1 B1 and B2 are closed on their fixtures.
- But the silent pinning regression against `2ade7af` survives on narrow flush steps (B3): the fix re-anchors on a centre ray that does not match Tnua's support sensor.

Services started: none persistent. The t1 client was shut down over BRP, and `tasklist` shows no `gta_like` process. Children: 0 launched / 0 reported.
