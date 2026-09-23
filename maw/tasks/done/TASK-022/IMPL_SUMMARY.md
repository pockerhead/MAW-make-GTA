# TASK-022 IMPL_SUMMARY: street life, spawning around the corner and ahead

Cost of error: medium. A spawn in clear view is something the owner sees right away. Ray cost and phase
logic can break silently, so those have headless gates. How dense and clumped the street feels is for the
owner run.

## 1. What was implemented

| File | Change |
|---|---|
| `crates/gta_sim/src/population/mod.rs` (+151/-36, 554 lines) | New rule in `spawn_civilians`: a point is valid if it is outside the cone (existing check) OR inside the cone and occluded (`occluded`: a head ray at `occlusion_ray_height`, then a feet ray at +0.1 m; both must hit `GameLayer::World`, via `perception::sight_blocked`). This now applies to the steady ring too; before, only the initial fill used it. Forward order: candidates are sorted by `spawn_forward_weight * cos(angle to the flat look) + U[0,1)` in the steady phase, and the initial fill keeps a random order (weight 0). The ray budget is `occlusion_rays_per_tick`: a point is checked only if 2 rays are left, and the rays cast are published in the new `PopulationLoad { rays }` (Reflect resource, reset every tick, same pattern as `PerceptionLoad`). The initial fill ends as "exhausted" only if no candidate was skipped for lack of rays. New `spawn_points()`: nodes plus interior points every `spawn_point_spacing` along each edge (see deviations). A spawn on an edge heads toward the edge end nearer the player. |
| `assets/npc/population.ron` | `spawn_ring: (30.0, 120.0)` (GDD §6.1 amendment). New fields: `spawn_point_spacing: 8.0`, `spawn_forward_weight: 2.0`, `occlusion_ray_height: 1.8`, `occlusion_rays_per_tick: 16`. `validate()` checks each of them. |
| `crates/gta_sim/tests/street_spawn.rs` (new, 325 lines) | The 3 acceptance gates (section 3). |
| `crates/gta_sim/tests/civilian_city.rs` (+32/-17) | `steady_spawns_in_the_ring_off_frame` is now `..._hidden` (outside the cone or occluded). Spawn positions are checked "on a sidewalk edge" instead of "on a node". Occlusion uses `occlusion_ray_height`. The initial-fill precondition looks for sidewalk points closer than the ring (there are no nodes in 20-30 m). |
| `crates/gta_sim/tests/config.rs` (+25/-1) | The `spawn_ring` fixture is now `(30.0, 120.0)`. Two new validation fixtures: `occlusion_rays_per_tick: 1` and `spawn_point_spacing: 2.0`, each rejected by name. |

The ray budget is 16 rays per fixed tick, so at most 8 in-cone points get checked. Probe
`scratch/ray_cost_probe.rs`: 0.27 us per 30-120 m ray in the seed-1 city, so 16 rays cost about 4.3 us per tick.
Rays are cast only on ticks with a deficit.

## 2. Deviations from the spec

- **Spawn points are nodes plus points along edges, not nodes only** (`spawn_points`, `spawn_point_spacing: 8.0`).
  The seed-1 sidewalk graph has nodes only at block corners (about 90 m apart along a street). On a
  straight street every node ahead within 60 m is a corner of the next crossing, and it is in clear view.
  I measured the spec rule as written (nodes, ring 30-120, occlusion, forward order): street-life mean
  **0.00**. `civilian_bundle` already accepts `walker + t`, so an edge point is an ordinary spawn.
  GDD §6.1 still says "на узлах графа тротуаров". It needs a wording fix, which I did not make: design
  docs are not in my write scope. Logged in `log.jsonl` (dead_end + decision).
- **A spawn on an edge heads toward the end nearer the player** (before: random `wander_next` from a node).
  With a random direction the 40 s gate gave 3.78, with this change 6.59 (same config). Nodes still use `wander_next`.
- **The street-life gate measures 40 s, starting when the last initial-fill civilian has despawned** (47.5 s
  into the run), not 20 s from t=15. The reason: cap 40 plus the 150 m despawn rule mean there is no
  turnover at all until the fill crowd is 150 m behind the player. From t=15 the spawner has had no deficit
  yet, so the result was 0.2-0.9 for any mechanism. After that point the in-view count cycles 0..21 with
  each cross street (period about 20 s at run speed). 20 s windows inside the 40 s give **3.26..8.60** for
  the same code. The gate prints the spread of the 20 s windows next to the mean.
- **Gait = Run** (4.5 m/s), because W in the client is Run (`src/input/mod.rs:152-156`). Route: from the
  player spawn, the direction with the longest free shape cast (740 m). The player moves through
  `MoveIntent`, and the gate checks the distance actually covered.
- In the initial fill, forward preference is off (weight 0), so it stays uniform as the spec requires. The
  ray budget and the in-cone occlusion rule were already part of it. The only change there is that the fill
  now also uses edge points.

## 3. Tests

Gates in `crates/gta_sim/tests/street_spawn.rs` (production composition, `city_app(1)`, seed 1):

| Gate | Class | Result | Flip-RED |
|---|---|---|---|
| `occluded_in_cone_accepted_clear_in_cone_rejected`: a cone with half-angle PI, so every point is in the cone. Real city: 14 ring nodes occluded, 10 clear. 64 steady ticks with forward order off. Every spawn is checked with an independent head+feet ray. | correctness | 40 spawns, all behind buildings | A1 (`occluded` always true): RED "spawned in clear view". A2 (always false): RED "nothing spawned behind a building" |
| `no_spawn_in_clear_view_over_60s_turning`: 3840 ticks, the camera turns 360 deg every 8 s, one civilian killed per second. Every spawn event is checked: if it is in the cone, the camera ray must be blocked. Also asserts `PopulationLoad.rays <= budget` on every tick. | correctness + bound | 107 spawns, 64 of them in the cone behind buildings; max 15 rays/tick (budget 16) | A1: RED at tick 0. A2: RED "no in-cone spawn exercised the ray" |
| `street_ahead_stays_populated`: the player runs along the street, the camera looks ahead. Mean number of civilians in the cone within 60 m (the measure from `qa_street_life.py`). | liveness/density | **7.63** (20 s windows 3.26..8.60) | A2: 0.00 RED. `spawn_forward_weight: 0`: 0.81 RED |

**Baseline** (the same gate on the code before the change, `git stash`): **0.00** for the whole 100 s run
(`scratch/baseline_street_life_final_gate.txt`, probe `scratch/street_baseline_probe.rs`). Earlier gate
shapes on the baseline also gave 0.00 (`baseline_street_life_warmup{15,30}.txt`).

**The density gate is fragile, and I have to say so.** It is one deterministic run, and the number is
non-monotonic in the tuning. `scratch/tuning_grid_street_life.txt` (an earlier 40 s version of the gate):
rays 8 gives 3.7-4.6, rays 16 gives 6.4-6.6, rays 32 gives 3.9-7.1, depending on the weight. More rays
place more of the fill crowd ahead, so it despawns later and turnover starts later. The shipped config
(16 rays, weight 2) has a margin of +2.6. A future change to movement, the city or wandering can move this
number either way without any bug.

Commands (all green):
- `cargo test -p gta_sim -p citygen`: 169 passed, 0 failed (`scratch/test_gta_sim_citygen.txt`)
- `cargo test -p gta_sim --test civilian_bench -- --nocapture`: mean 673 us (limit 8 ms), p95 756 us. The
  bench looks straight up (nothing in the cone), so the spawner casts no rays there. The ray bound is gated
  in the turning test.
- `cargo test -p gta_like --bin gta_like`: 38 passed
- `cargo clippy --workspace --all-targets -- -D warnings`: clean
- `cargo tree -p gta_sim -e normal -i bevy_render`: "nothing to print" (empty)

Runtime (release, `--features dev`, seed 1):
- `scratch/qa_street_life_rerun.py` (the TASK-009 probe plus a 40 s walk) gives `scratch/street_life/log.json`
  and screenshots. In the cone within 60 m: standing **1-4** (TASK-009 got 0-1), turning 1-7 (1-3), short
  walk 1-3 (3→0). In the long walk the mean is **6.55**, and 10.5 over its last 10 samples. No crowd yet in
  the first 8 s of walking: that is the fill phase (see the deviations). `long_walk_19.png`: people on the
  sidewalks and at the crossing ahead.
- `python tools/qa/scenarios/t8.py --out scratch/t8`: exit 0, `log_errors: []`. `first_fill.closer_than_ring = 1`:
  the check is weaker now, because the ring starts at 30 m instead of 60 m.
- No game process was left running.

## 4. Manual check (owner)

`cargo run --release --features dev -- --seed 1`, then run straight along a street for 60+ s.
Expected: civilians come out of side streets ahead and walk toward the player. What to judge by eye:
1. Clumping: at a cross street up to 15-20 civilians show up at once, then the street is fairly empty for
   5-10 s until the next crossing. This comes from the cap of 40 and the despawn rule, which stayed as they were.
2. A head or legs peeking out at the moment of spawning (the ray goes to 1.8 m and to the feet).
3. For the first ~45 s after load the crowd is the one from the fill, and the refill ahead starts once it
   is far behind.

## git status

Changed: `assets/npc/population.ron`, `crates/gta_sim/src/population/mod.rs`,
`crates/gta_sim/tests/{civilian_city,config}.rs`, new `crates/gta_sim/tests/street_spawn.rs`, and in the
task dir `log.jsonl` (5 appends), `PCTX_PROPOSALS.md`, `IMPL_SUMMARY.md`, `scratch/*`. Nothing else was touched.
