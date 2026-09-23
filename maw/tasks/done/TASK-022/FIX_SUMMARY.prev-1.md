# TASK-022 FIX_SUMMARY

How costly a mistake here is: medium. Recycling a civilian the player can see, or a scared one, is a silent correctness bug, so it has a headless gate. Clumping and how the street feels are left to the owner run.

## Preflight: the review claim that would break correct code if applied verbatim

The candidate cap that the orchestrator note gives as an example ("nearest K") is the one I checked. `population/mod.rs:412-430` filters every spawn point to the ring and then sorts by `forward_weight * ahead + U[0,1)`. The K points nearest the player sit at the ring's inner edge (30 m) all the way around the player. Capping to them would throw away the forward points 60-120 m out and quietly undo the forward preference. The review's diagnosis (the scan is not measured under deficit) was right, so I measured it. The cost is small (see below), so no cap was added.

The review's Major issue (the density gate measures 40 s after the fill crowd despawns, and 20 s windows go down to 3.26) was real. I checked it against `street_spawn.rs:263-324`. OPEN_DECISIONS resolves it (last entry, binding), and I implemented that decision.

## 1. Fixed

| Review item | What was done |
|---|---|
| Major: the density gate does not show the 20 s experience | **Recycling** (`population/mod.rs::despawn_far`). When alive civilians >= `max_civilians`, a civilian is despawned only if it meets all of these: state `Wander` or `Idle` (never Flee/Cower/Report/Dead, so corpses are excluded too), off-frame (outside the cone, the existing `Offscreen` timer) for >= `despawn_offscreen_seconds`, and flat distance > `recycle_distance`. The existing spawner then refills the freed budget ahead. New RON field `recycle_distance: 50.0`, plus `validate()`: finite and `spawn_ring.0 <= recycle_distance <= despawn_distance`. Setting it to `despawn_distance` turns recycling off. |
| Major: gate shape | **`street_ahead_stays_populated` replaced.** The player stands with the camera looking ahead until t=15 s (the initial fill is at the cap by about 1.5 s), then runs along the longest open seed-1 street for 60 s. Samples at 4 Hz. Gate: every sliding 20 s window mean >= 4 (the strictest reading of "every 20 s window"), and the overall mean >= 5. Consecutive thirds are reported too. |
| Missing coverage: candidate enumeration/sort cost under deficit | **New `street_turnover_bench`** (`civilian_bench.rs`). Warmup 15 s, then a 40 s run looking along the street. Recycling forces turnover. Each tick is timed and classified as deficit (the spawner spawned or is still short of the cap) or at-cap. Both the overall mean and the deficit-tick mean are gated `< 8 ms` (the existing `MEAN_LIMIT`). The test also asserts > 64 deficit ticks, so turnover really happens. |
| Orchestrator: recycling correctness | **New `recycle_at_cap_only_calm_and_far`** (`civilian_city.rs`). Real seed-1 sidewalk spots and the camera looking up, so everything is off-frame. Spawning is disabled by a huge separation. Four bodies: calm far (60-140 m), cowering far, calm near (30-45 m), and a corpse. Below the cap nothing is recycled. At the cap only the calm far one goes. |
| `validate()` | **New fixture `recycling_stays_outside_the_spawn_ring_inner_edge`** (`config.rs`): `recycle_distance: 20.0` is rejected, and the error names `recycle_distance`. |

Test plumbing: `chase_view` and `open_street` moved from `street_spawn.rs` to `tests/common/mod.rs` (the bench needs them too). Their bodies are unchanged.

### Measurements

Density gate (`scratch/fixer_street_spawn.txt`), recycle_distance swept (log.jsonl decision):

| recycle_distance | mean | 20 s thirds | sliding 20 s min | verdict |
|---|---|---|---|---|
| 150 (recycling off) | 7.20 | 1.85 / 9.50 / 10.24 | 1.85 | **RED** |
| 30 | 14.98 | 7.94 / 20.21 / 16.79 | 7.94 | green |
| 40 | 13.73 | 7.90 / 17.50 / 15.80 | 7.90 | green |
| **50 (shipped)** | **13.29** | **7.66 / 17.34 / 14.88** | **7.66** | green |
| 60 | 12.22 | 7.51 / 15.59 / 13.55 | 7.51 | green |
| 80 | 10.73 | 5.04 / 12.95 / 14.21 | 5.04 | green |

The baseline before TASK-022 was 0.00 (implementer, `scratch/baseline_street_life_*.txt`). Before this fix (implementer code, recycling off) the gate's first 20 s window is 1.85, and that is what the review flagged.

Per-second count in view (shipped config, t=15..75): `4 4 5 7 8 8 8 7 6 4 3 1 1 1 1 1 2 7 12 25 30 33 33 34 33 30 28 26 22 14 11 9 7 5 5 8 8 9 12 13 13 14 14 14 14 11 11 8 5 3 3 4 9 13 17 22 25 27 27 27`. The window means pass with a wide margin, but the count still swings: a 5 s dip to 1 around t=26-30 and bursts of 25-34 of 40 at cross streets. This is the clumping the implementer already reported, now stronger. It is for the owner to judge (section 4).

Turnover bench (`scratch/fixer_civilian_bench.txt`): mean **820 us** over 2560 ticks. Deficit ticks: 78, mean **962 us**. At-cap ticks: 2482, mean 816 us. 6783 spawn points in the graph, max 16 occlusion rays per tick (budget 16), max 14 perception agents per tick. The whole spawner pass on a deficit tick (enumerating 6783 points, separation against about 40 bodies, sort, up to 16 rays, one spawn) costs about 0.15-0.2 ms. That is ~2% of the 8 ms gate, and even if every tick were a deficit tick the mean would stay under 1 ms. **No candidate cap**: the cost does not justify one, and nearest-K would break the forward preference (see Preflight). The existing `civilian_bench` (64 civilians) is unchanged: mean 689 us.

### Flip-RED (each observed RED, then restored and observed GREEN)

| Gate | Perturbed input | RED output |
|---|---|---|
| `street_ahead_stays_populated` | `recycle_distance: 150.0` in RON (recycling off) | `a 20 s window has mean in view 1.85 < 4` |
| `street_turnover_bench` deficit mean | 10 ms sleep inserted in `spawn_civilians` after the deficit check | `mean deficit tick 11.355135ms >= 8ms` (the overall mean was 1.08 ms and would have stayed green, which is why the deficit mean has its own assert) |
| `street_turnover_bench` turnover precondition | `recycle_distance: 150.0` | `GATE BROKEN: only 28 deficit ticks, turnover not forced` |
| `recycle_at_cap_only_calm_and_far` | `at_cap` replaced by `true` | `calm far recycled below the cap` |
| same | state filter replaced by `true \|\| matches!(..)` | `scared far recycled at the cap` |
| same | `distance > recycle_distance` replaced by `distance >= 0.0` | `calm near recycled at the cap` |
| same | `recyclable = false && ..` (no recycling) | `calm civilian off-frame beyond 50 m not recycled at the cap` |
| `recycling_stays_outside_the_spawn_ring_inner_edge` | the `recycle_distance` check in `validate()` replaced by `if false` | panics at `unwrap_err` (config accepted) |

## 2. Skipped / changed from the prescription

- **"Cap the candidates (nearest K / partial sort)"**: not done. It was measured and is not needed (0.15-0.2 ms per deficit tick), and nearest K is wrong for this spawner (Preflight).
- **Two existing tests now set `recycle_distance = despawn_distance`**:
  - `civilian_bench::bench`: its perception bound `ceil(alive/slots)+1` failed under recycling (`tick 200: 18 agents perceived, bound 17`). Perception slots are handed out round-robin at spawn (`perception/mod.rs::assign_slot`), so despawn churn unbalances them. The bench is about a fixed 64-body crowd, so recycling is off there, and the bound was **not** loosened.
  - `despawn_after_2s_offscreen_beyond_150m`: it uses `max_civilians = 0` as "no spawning", which now also means "at the cap". It tests the plain 150 m rule, so recycling is off there. Recycling has its own gate.
- **Open concern (not fixed, outside this task)**: slot imbalance under turnover. With 40 alive and the default slots, the turnover bench saw at most 14 perception agents in one tick (the balanced bound would be about 11). The cost stays bounded by N and is tiny (included in the 820 us mean), but rebalancing on despawn or assigning a slot by least occupancy would restore the per-tick bound. Proposed for the owner/orchestrator. Not changed.
- The review's second missing-coverage point (the head/legs silhouette at spawn) is an owner-run visual check, as before.

## 3. Test results

- `cargo clippy --workspace --all-targets -- -D warnings`: clean.
- `cargo test -p gta_sim -p citygen`: all green (`scratch/fixer_test_gta_sim_citygen.txt`). civilian_bench 2, civilian_city 7, config 28, street_spawn 3, all others as before.
- `cargo test -p gta_like --bin gta_like`: 38 passed.
- `cargo tree -p gta_sim -e normal -i bevy_render`: "nothing to print".
- `cargo test -p gta_sim --test street_spawn`: `no_spawn_in_clear_view_over_60s_turning` gives 241 spawns in 60 s, 171 of them in the cone behind buildings, none in clear view, and max 16 rays per tick (budget 16). Recycling roughly doubles turnover under a turning camera, and the occlusion rule still holds.

Runtime (release, `--features dev`, seed 1):
- `python maw/.../scratch/qa_street_life_rerun.py`: `scratch/street_life/log.json` plus screenshots. The implementer's run is kept as `street_life/log_implementer.json`. In the cone within 60 m:

  | phase | TASK-009 | implementer | fixer |
  |---|---|---|---|
  | stand (10 samples) | 0-1 | 1-4 (mean 2.8) | 1-4 (mean 2.8) |
  | yaw 0/90/180/270 | 1-3 | 4,1,7,3 | 4,0,0,1 |
  | short walk (4) | 3→0 | 3,3,1,1 | 4,3,2,2 |
  | long walk (20 x 2.2 s) | n/a | mean 6.55, first 4 samples 0 | mean **11.35**, first 4 samples 1,1,1,2 |

  `long_walk_19.png`: civilians on the sidewalk and at the crossing ahead. The yaw row is one snapshot per heading. It is lower than before, which fits recycling removing calm civilians behind the camera beyond 50 m. Owner check 2 below.
- `python tools/qa/scenarios/t8.py --out scratch/t8`: exit 0, `log_errors: []` (`scratch/fixer_t8_stdout.txt`).
- No game process left running.

## 4. Owner checklist

`cargo run --release --features dev -- --seed 1`, run straight along a street for 60+ s:
1. Clumping: a crowd of up to 25-34 at some cross streets, and a few seconds of 1-2 in between. Is that acceptable, or should the spawner spread spawns out (for example with a per-second spawn budget)?
2. Turning around after a run: calm civilians beyond 50 m behind you have been recycled, so the street behind looks thinner than before. `recycle_distance` in `assets/npc/population.ron` sets this (30-60 m all pass the gate).
3. As before: no head or legs popping into view at a spawn.

## git status

Changed: `assets/npc/population.ron`, `crates/gta_sim/src/population/mod.rs`, `crates/gta_sim/tests/{civilian_bench,civilian_city,config,street_spawn}.rs`, `crates/gta_sim/tests/common/mod.rs`. Task dir: `log.jsonl` (+3 decision entries), `FIX_SUMMARY.md`, `scratch/fixer_*.txt`, `scratch/street_life/*` and `scratch/t8/*` rerun outputs (scratch is gitignored). Nothing else.

children: 0 launched / 0 reported.
