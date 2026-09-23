# TASK-022 FIX_SUMMARY (fixer round 2, after QA)

Cost of error: medium. A spawn or recycle the player can see, or a gate that goes red at random, breaks
silently, so each item got a headless gate with a flip-RED. Clumping and how the street feels stay with the
owner.

## Preflight

- Scratch read as a coverage map: QA probes (`scratch/qa/qa_probe_task022.rs`, `qa_runtime.py`, runtime
  screenshots), round-1 fixer logs, flip-RED logs. I did not rerun the author's scripts as verification. I wrote
  my own probe (`scratch/fixer2/fixer2_churn_probe.rs`).
- Review read: `IMPL_REVIEW.md`. Its one Major item (20 s window) was closed in round 1 and is still green
  (below). This round follows the orchestrator note (QA 4.1-4.4) and the last OPEN_DECISIONS entry.
- **The claim that would break correct code if applied verbatim:** QA 4.1's fix, "recycling frees only the
  deficit it will refill". `despawn_far` recycles only when `alive >= max_civilians`
  (`population/mod.rs::despawn_far`), and there the spawner's deficit is 0 by definition
  (`spawn_civilians`: `deficit = max - alive`). Applied verbatim, it recycles nothing and turns recycling off,
  which is the round-1 RED (sliding min 1.85). The orchestrator's version (a per-tick budget) is safe but
  **does not fix the churn**. I measured it: see 1.1.

## 1. Fixed

### 1.1 (QA 4.1) Recycling burst and churn
- New RON field `recycles_per_tick: 1` (`assets/npc/population.ron`). `validate()` requires >= 1.
  `despawn_far` collects the recyclable civilians, sorts them farthest first and despawns at most the budget.
  The budget is 1 because the spawner refills 1 per tick (`spawns_per_tick`).
- **A budget alone did not change the churn** (probe, seed 1, standing, spawns per 10 s):

  | variant | 0-10 s | 10-20 s | 20-30 s | max recycled/tick |
  |---|---|---|---|---|
  | before (round 1) | 148 | 88 | 42 | 22 |
  | budget 1, farthest first | 144 | 97 | 61 | 1 |
  | budget 2 / 4 / 8 | 146/142/150 | 91/85/91 | 40/51/57 | 2/4/8 |
  | **budget 1 + behind only (shipped)** | **62** (40 = fill) | **5** | **5** | **1** |

  The loop works like this: the spawner puts the freed slot at a side point just outside the cone beyond
  50 m, and 2 s later that civilian is recyclable again. The fix follows GDD §6.1 as the orchestrator already
  wrote it ("спокойные мирные ... **позади** ... по нескольку за тик"): recycle only when the flat offset from
  the player, dotted with the flat look direction, is <= 0. Round 1 recycled anything off-frame. The spawner
  sorts by `2 cos + U`, so it almost never places a spawn behind the player, and the loop is gone.
- **New gate `street_spawn::standing_still_does_not_churn`**: seed 1, the player stands 20 s looking along the
  street. Asserts: at most `recycles_per_tick` recycled in any tick, and spawns in 10-20 s <= `max_civilians / 2`
  (a full turnover of the crowd in 10 s would be >= 40). Measured `[62, 5]`, max 1.
- **Clumping at crossings, re-measured** (probe, stand 30 s, then run 60 s, in the cone within 60 m):

  | seed | before: mean / max / seconds >= 25 | after: mean / max / seconds >= 25 |
  |---|---|---|
  | 1 | 14.00 / 27 / 7 | 11.93 / 28 / 6 |
  | 2 | n/a | 11.73 / 29 / 4 |
  | 3 | n/a | 12.68 / 27 / 5 |

  **The bursts were not the cause of the clumps.** With max 1 recycled per tick, the crossings still reach
  25-29 in view. The clumps come from the forward order plus occluded spots at the cross street, and they
  stay an owner item.
- Street-life gate still green: mean 11.88, 20 s windows `[5.56, 15.98, 14.10]`, sliding 20 s min **5.56**, so
  every 20 s window of the run meets the original >= 5 target. Round 1 had mean 13.29 and sliding min 7.66.
  Baseline before TASK-022: 0.07 (QA).
- Trade-off, for the orchestrator or owner: the street is emptier while **standing still**. In the gate's first
  15 s after load, 0-4 are in view. After 30 s standing it is 8 (round 1: 14). Round 1's churn was what
  refilled the view while standing.

### 1.2 (QA 4.3) Body width at building corners
- `population::occluded` now casts rays to the head (`occlusion_ray_height`) at the centre **and at
  `capsule_radius` to either side**, perpendicular to the flat camera ray, plus the feet ray. That is
  `OCCLUSION_RAYS_PER_POINT = 4`, a law of the check and not tuning. The rays short-circuit on the first
  clear one. There are no side feet rays because a building standing on the ground that blocks the ray to the
  top of a vertical line also blocks every lower point of that line: the rays share one xz projection, and
  the lower ray is lower at every crossing. That is the comment in the code.
- Budget: `occlusion_rays_per_tick: 16` is unchanged, so at most 4 in-cone points per tick. Asserted every tick
  in `no_spawn_in_clear_view_over_60s_turning` (max 16 observed). `validate()` requires >= 4 (a whole point),
  and the fixture moved to `3` (strictly on the failing side of the new bound).
- The test helper `hidden_behind_geometry` now checks the side head rays too. Every spawn in the two
  existing occlusion gates is held to full body width.
- **New gate `street_spawn::corner_peek_in_cone_rejected`** (real seed-1 city): spawner points (nodes plus edge
  points) in the ring, seen from the chase camera at 36 yaws. 4686 are hidden across the whole width and 19
  are hidden at the centre with a side ray clear. For the first "peek" point the test sets a thin ring through
  it, a 10 deg cone at it and forward weight 1e6, so it is the first candidate. It asserts that nobody spawns
  there. Positive control: a fully hidden point from the same camera, set up the same way, **is** spawned at
  (otherwise GATE BROKEN).
- Not verified at runtime by me (no windowed build this round). The half-visible-at-corner look goes to the
  owner and QA.

### 1.3 (QA 4.2) Forward preference gated
- `street_ahead_stays_populated` now records every spawn during the run (through `tick_spawns`, which also
  keeps the on-graph assert). It asserts that >= 0.75 of them lie ahead of the player (flat offset · run
  direction > 0), plus a GATE BROKEN floor of >= 20 run spawns. Measured: shipped **1.00** (94/94), weight 0
  **0.39** (140/360).

### 1.4 (QA 4.4) Flaky `civilian_gate`, root cause
- I added a probe print to the unchanged test (8 runs). Asset loading took **3 to 11** updates, so the walk
  clip's phase at the first sample varies. The gate compared `leg-left` at two samples 32 updates apart. A
  swinging leg passes the same angle on both sides of an extreme, so the two-sample angle depends on phase.
  Measured two-sample angles: 0.018 (the failing run, model 2), then 0.185 ... 1.02 in green runs.
- Fix (`src/visuals/civilian_gate.rs`): sample every update and assert the **largest turn from the first
  pose** over the same 32-update window. Measured 0.89 ... 1.03 rad for every model in every run. The gate
  still proves what it did before: a real GLB with its own clips rotates the leg, and a model in bind pose
  gives 0.
- Rejected alternative: pausing `Time<Virtual>` while waiting for the load. Loading still ends at a random
  update, so the phase stays random (log.jsonl decision).
- 10/10 consecutive runs of `cargo test -p gta_like --bin gta_like`: 38 passed each time, plus 20/20 extra runs
  of the gate alone (`scratch/fixer2/gta_like_10_runs.txt`).

### Other
- `civilian_bench::street_turnover_bench`: the precondition (">64 deficit ticks") fell to a margin of 1 (65)
  under behind-only recycling. I lowered it to >48 with a measured comment. Recycling off gives 29, which is
  RED (flip below). Bench numbers: 64 civ mean 790 us, p95 1.01 ms. Turnover mean 780 us, deficit ticks (65)
  mean 965 us, max 16 occlusion rays per tick.
- New config fixture `recycling_needs_a_budget` (`recycles_per_tick: 0`).
- `PCTX_PROPOSALS.md`: a gates lesson (a two-snapshot pose check is phase-dependent).
- `log.jsonl`: 1 dead_end + 4 decision entries.

## 2. Flip-RED (each observed RED, restored, observed GREEN; logs in `scratch/fixer2/`)

| Gate | Perturbation | RED |
|---|---|---|
| `corner_peek_in_cone_rejected` | `occluded` rays `[head, feet]` only (side rays removed) | panics at the peek assert, `street_spawn.rs:245` (`flip_red_side_rays_removed.txt`). The other 4 street_spawn gates stay green, so only the new gate sees the corner |
| `standing_still_does_not_churn` | `.take(usize::MAX)` instead of the budget | `14 recycled in one tick, budget 1` |
| same | `behind = true` | `standing still churns: 119 spawns in the second 10 s` (spawns `[152, 119]`) |
| `street_ahead_stays_populated` (forward) | `spawn_forward_weight: 0.0` in RON | `only 0.39 of the run spawns ahead of the player: no forward preference` (the density windows would also fail: min 2.38) |
| `civilian_gate::every_civilian_model_animates_from_its_own_clips` | `build_graph` given `config.model` for every model (the TASK-009 wrong-root bug) | `character-female-a.glb (1): leg-left turned 0 rad while walking (T-pose)` |
| `street_turnover_bench` precondition | `recycle_distance: 150.0` | `GATE BROKEN: only 29 deficit ticks, turnover not forced` |
| `recycling_needs_a_budget` | `recycles_per_tick < 1` check disabled | `unwrap_err()` on `Ok` |
| `occlusion_ray_budget_checks_a_whole_point` (fixture now 3) | bound back to `< 2` | `unwrap_err()` on `Ok` |

Sources were restored by copy and checked with `diff` (empty) and `git diff --stat`.

## 3. Skipped / not done

- **"Recycle only the deficit it will refill"** (QA 4.1 prescription): not done as written. At the cap the
  deficit is 0 (Preflight). The budget plus behind-only rule does what it meant.
- **"Farthest first" has no own gate.** The budget gate proves the bound, not the order. Which behind,
  off-frame civilian goes first cannot be seen. Cost of error is low, so I declined the extra machinery.
- **Rays at head and feet on both sides** (orchestrator example): only the head sides are cast. The feet sides
  are implied by the geometry (1.2). If props or overhangs on the World layer ever break that, the centre feet
  ray still stands.
- **Runtime rerun** (`qa_runtime.py`, `t8.py`): not run this round. The changes are sim rules covered by
  headless gates. The corner look and the standing-density trade-off need QA/owner runtime. No game process
  was started.
- 4.5 (crossing stall): untouched, TASK-023.

## 4. Test results

- `cargo clippy --workspace --all-targets -- -D warnings`: clean.
- `cargo test -p gta_sim -p citygen` (after `touch crates/gta_sim/src/lib.rs`): 22 result lines, all ok, 0
  failed (`scratch/fixer2/test_gta_sim_citygen.txt`). street_spawn 5, civilian_city 7, civilian_bench 2,
  config 29.
- `cargo test -p gta_sim --test street_spawn -- --nocapture` (`scratch/fixer2/street_spawn.txt`):
  corner 4686/19 points, 211 spawns / 148 in the cone / max 16 rays, 40 hidden spawns, standing `[62, 5]` max 1,
  street mean 11.88 / sliding min 5.56 / ahead 1.00.
- `cargo test -p gta_like --bin gta_like`: 10/10 runs, 38 passed.
- `cargo tree -p gta_sim -e normal -i bevy_render`: "nothing to print".
- rustfmt `--edition 2024` on the 5 edited .rs files. `git diff --stat` shows only those files and the RON.

## 5. Owner checklist

1. Clumps at crossings (25-29 in view) did not change. The recycling burst was not the cause. Knob:
   `spawn_forward_weight`.
2. Standing still right after load: 0-4 in view for the first ~15 s, ~8 after 30 s (round 1: ~14, bought
   with 148 spawns per 10 s). Is that acceptable?
3. Corners: a civilian appearing at a building corner should not show a body sliver on the first frame
   anymore (full-width rays). Check at a crossing with the camera along the street.

## git status

Modified: `assets/npc/population.ron`, `crates/gta_sim/src/population/mod.rs`,
`crates/gta_sim/tests/{street_spawn,config,civilian_bench}.rs`, `src/visuals/civilian_gate.rs`. Task dir:
`FIX_SUMMARY.md` (the previous one archived by the orchestrator as `FIX_SUMMARY.prev-1.md`), `log.jsonl` (+5),
`PCTX_PROPOSALS.md` (+1), `scratch/fixer2/*`. The temporary probe copy in `crates/gta_sim/tests/` was removed.

children: 0 launched / 0 reported.
