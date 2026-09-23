# TASK-022 QA report, round 2: street life, spawning around the corner and ahead

Cost of error: medium. A spawn in clear view, a recycle loop or a flaky gate breaks silently, so those are
checked with headless gates plus my own probes and a runtime run. How dense or clumped the street looks is
something the owner sees in the first minute of play, so it goes to the owner checklist.

## 0. Preflight

- Scratch was read as a coverage map only: round-1 QA probes (`scratch/qa/`), fixer-2 probe and flip logs
  (`scratch/fixer2/`), implementer baselines. I did not rerun any earlier author's script as verification.
  My probes and logs are in `scratch/qa2/`.
- I read task.md, IMPL_SUMMARY.md, IMPL_REVIEW.md, FIX_SUMMARY.md, QA_REPORT.prev-1.md, OPEN_DECISIONS.md,
  PCTX_PROPOSALS.md and log.jsonl, plus the round-2 diff `3b24059..108ea01` (population/mod.rs,
  population.ron, civilian_gate.rs, street_spawn.rs, config.rs, civilian_bench.rs).
- **The counter-example I looked for:** behind-only recycling removes churn only while the view stays fixed.
  A player who looks around turns the crowd that was ahead into a crowd that is "behind" and recyclable, so
  the loop should come back. Second target: a civilian that spawns half-visible at a building corner even
  with the new 4-ray body-width check.
  **Result:** the first case **holds** as a mechanism: standing and looking around keeps about 75-85 spawns
  per 10 s (4.1). It is invisible and costs nothing measurable, so it is Minor/informational, not a spec
  violation. The second case **did not hold**: 0 half-visible spawns among 75 in-cone first sightings at
  runtime, and 0 at the 0.3 m capsule width among 288 in-cone spawns in the headless probes.
- Dead-end triage: fixer-2 dead_end "a budget alone does not reduce standing churn" was checked against the
  code (`despawn_far`: the budget is applied after the behind filter) and against my flip 2 (`behind = true`
  gives `[152, 119]` spawns, so the budget alone does not help). Round-1 QA dead_end "runtime walk stalls at
  a crossing" was reproduced: this time the player stalls against a streetlight pole at (6.8, 1.2, -19.6)
  (`scratch/qa2/runtime/periodic_16_B.png`). This is TASK-023 and out of scope.

## 1. Environment

No docker and no dev server. I used cargo directly with the headless Bevy `App` (production `compose_sim`
via `city_app(seed)`), and the real windowed release build driven over BRP (`tools/qa/brp.py`,
`--features dev`, `--seed 1`). Host display: present mode Fifo, switched to `AutoNoVsync` over BRP for the
frame-cost numbers. Window 1280x720.

Reproduce:
```
touch crates/gta_sim/src/lib.rs crates/citygen/src/lib.rs
cargo clippy --workspace --all-targets -- -D warnings
cargo test -p gta_sim -p citygen
for i in $(seq 10); do cargo test -p gta_like --bin gta_like; done
cargo tree -p gta_sim -e normal -i bevy_render
# QA probe: copy scratch/qa2/qa2_probe.rs into crates/gta_sim/tests/, run, then delete it
cargo test -p gta_sim --test qa2_probe -- --nocapture --test-threads=3
# runtime (from scratch/qa2): walk/turn, and the look-around corner run
QA_OUT=runtime python qa2_runtime.py
MODE=look QA_OUT=runtime_look2 python qa2_runtime.py
python tools/qa/scenarios/t8.py --out maw/tasks/in_progress/TASK-022/scratch/qa2/t8
```
Cleanup: no containers or services were started. Every game run ended with `brp_extras/shutdown`, and
`tasklist` shows no `gta_like`. The temporary `crates/gta_sim/tests/qa2_probe.rs` was removed (copy in
`scratch/qa2/`). Sources perturbed for flip-RED were restored and verified by sha256.

## 2. Test results

### Existing suites
| Command | Result |
|---|---|
| `cargo clippy --workspace --all-targets -- -D warnings` | clean |
| `cargo test -p gta_sim -p citygen` (after touching both lib.rs) | all green, 0 failed (`scratch/qa2/test_sim_citygen.txt`); street_spawn 5, civilian_city 7, civilian_bench 2, config 29 |
| `cargo test -p gta_like --bin gta_like`, **10 consecutive runs** | 10/10 green, 38 passed each time (`scratch/qa2/gta_like_10_runs.txt`) |
| `civilian_gate::every_civilian_model_animates_from_its_own_clips` alone | 30/30 green |
| `cargo tree -p gta_sim -e normal -i bevy_render` | "nothing to print" |
| `civilian_bench` | 64 civ mean 807 us, p95 948 us, max 1.42 ms (limit 8 ms); turnover: deficit ticks (65) mean 1.01 ms, max 16 occlusion rays/tick (`scratch/qa2/civilian_bench.txt`) |
| `street_spawn --nocapture` | corner 4686/19 points; turning 211 spawns / 148 in cone / max 16 rays; standing `[62, 5]` max 1 recycled; street mean 11.88, windows `[5.56, 15.98, 14.10]`, ahead 94/94 (`scratch/qa2/street_spawn.txt`) |
| `python tools/qa/scenarios/t8.py` | exit 0, `log_errors: []` (`scratch/qa2/t8_stdout.txt`) |

### Flip-RED done by me (`scratch/qa2/flip_red.txt`)
| Perturbation | Result | Restore |
|---|---|---|
| `occluded` casts `[head, feet]` only (side head rays removed) | `corner_peek_in_cone_rejected` RED at `street_spawn.rs:248`; the other 4 street_spawn gates stay green | sha256 `8e87b6ce...f253` matches |
| `behind = true` in `despawn_far` (recycle anything off-frame) | `standing_still_does_not_churn` RED: "119 spawns in the second 10 s" (`[152, 119]`) | same sha256 |
| `spawn_forward_weight: 0.0` in population.ron | `street_ahead_stays_populated` RED: "only 0.39 of the run spawns ahead" (mean 4.55, sliding min 2.38) | sha256 `1c302457...dfa2` matches |

Round-1 finding 4.2 (forward preference had no RED gate) is closed: flip 3 turns the gate RED.

### New QA probe (`scratch/qa2/qa2_probe.rs`, output `scratch/qa2/probe_output.txt`)
Independent silhouette check: for every spawn inside the cone, rays at heights 0.1/0.6/1.0/1.4/1.8 m at the
centre and at +-half-width, for half-widths 0.3 (capsule), 0.45 and 0.6 m.

| Probe | Result |
|---|---|
| Standing 60 s, seeds 1/2/3, spawns per 10 s | `[62,5,5,5,5,1]`, `[59,1,4,2,3,6]`, `[62,6,4,5,3,2]`; max 1 recycled per tick; 0 spawns behind the player after 10 s |
| Standing, in view within 60 m (seed 1) | 0 for the first 11 s, 1-4 until 17 s, 8 at 30 s, 10-11 at 60 s. Seeds 2/3 are similar (0-6 in the first 15 s) |
| **Look-around** (seed 1, view flips 180 deg every 5 s) | `[98, 74, 76, 78, 83, 87]` spawns per 10 s, max 1 recycled per tick (4.1) |
| Walk, seeds 1/2/3: forward share | 94/94, 94/94, 44/44 ahead (seed 3 stalls after 115 m) |
| Walk seed 1, first 20 s of the run (1 Hz) | mean **5.10**; 12 of the 20 s show 0-3 in view, the mean is carried by a crossing burst of 7-27 (4.2) |
| Walk, clumping | max in view 31/28/29, seconds with >= 25: 9/5/7 (seeds 1/2/3) |
| Silhouette of in-cone spawns (all probes: 13+15+27+22+92+85+56 = 310; 288 excluding look-around) | 0 with any clear ray at 0.3 m; at 0.45 m and 0.6 m: 1 of 92 on the seed-1 walk (a point wider than the collider) |

### Runtime (release, `--features dev`, seed 1, AutoNoVsync)
Probe `scratch/qa2/qa2_runtime.py` (mine). Every new civilian entity is logged. Each in-cone first sighting
gets a screenshot about 0.06-0.18 s later and a zoom crop around the projected body. In `runtime_look2`,
every in-cone civilian's projected head is also marked with its distance, so a figure in the crop can be
matched to an entity.

- **Churn standing (4.1 re-check):** `scratch/qa2/runtime/log.json`, new entities per 10 s while standing
  0-40 s: `[55, 1, 2, 3]` (the 55 include the 40 of the fill). Round 1 had 148/88/42. **Fixed.**
- **Churn while looking around:** `runtime_look`/`runtime_look2` (60 deg yaw step every 2.5 s after 15 s):
  `[55, 15, 51, 78, 60, 44, 74, 16]` and `[55, 16, 54, 82, 61, 50, 80, 15]` new per 10 s. After a yaw step
  there are 6-7 new spawns in the new view within about 0.1 s (for example t=17.62, 7 new, all hidden). This
  matches the headless look-around probe (4.1).
- **Forward share (4.2 re-check):** phase B (hold W): 11/11 new civilians ahead of the player. The run is short
  because the player stalls at a streetlight after 21 m (TASK-023). The headless gate carries this claim.
- **Corner spawns (4.3 re-check), zoom method from round 1 plus attribution:** 34 in-cone first sightings in
  the walk run (all at 66-123 m, `runtime/crops_sheet.png`), 144 in `runtime_look` (8 within 60 m,
  `runtime_look/sheet_le60.png`), 152 in `runtime_look2` (33 within 75 m, `runtime_look2/attrib_0..4.png`).
  I looked at every crop within 75 m and every walk-run crop. **Half-visible spawns: 0.** Each red spawn mark
  lies on a facade or behind a foreground body. One false alarm: `runtime_look/zoom_05.png` shows a small
  figure at the building corner right next to a 57.8 m spawn mark. The same corner seen from the same yaw in
  `runtime_look2/attrib_0.png` (tile 3, t=17.78) and `attrib_2.png` (t=49.14) shows that figure is a
  civilian **103.9 m / 91.7 m** away on the far sidewalk. `runtime_look/zoom_same_point.png` shows the same
  spawn point in two later cycles with nothing at the corner. The overlay markers line up with visible
  figures (for example 47.6 m and 34.8 m), which checks the projection.
- **Standing right after load:** runtime in view within 60 m was 3-5 in the first 5 s after BRP ready, 8 at
  10 s and 11-13 after 15 s. Runtime starts counting after load, so it reads a little higher than headless
  (0 for the first 11 s).
- **Street behind after turning around (phase C):** mean 4.39 in view (min 2, max 6), against 9.72 looking
  ahead while standing. The player had run only 21 m.
- **Frame cost:** the 120-frame average frame time over each run was 2.32-2.83 ms (walk), 2.30-2.62 ms (look),
  2.31-2.56 ms (look2). There is no step when the look-around bursts happen.
- `t8.py`: exit 0, no log errors, `first_fill.count 40`.

## 3. Acceptance criteria

| # | Criterion | Test performed | Result |
|---|---|---|---|
| 1 | Occluded in-cone node accepted, clear in-cone node rejected, real seed-1 nodes, flip-RED by disabling the ray | `occluded_in_cone_accepted_clear_in_cone_rejected` (14 occluded / 10 clear, 40 hidden spawns); round-1 QA flip (occluded -> true) RED; my flip 1 (side rays removed) turns the new corner gate RED | PASS |
| 2 | No spawn at an in-cone point with a clear camera ray over 60 s turning, every spawn checked | `no_spawn_in_clear_view_over_60s_turning` (211 spawns, 148 in cone, full-width check); my probe: 310 in-cone spawns, 0 clear rays at 0.3 m, heights 0.1-1.8 m; runtime: 0 half-visible of 75 in-cone first sightings checked by eye | PASS |
| 3 | Street-life: 20 s forward walk, mean >= 5 in view within 60 m, clearly above baseline | gate: sliding 20 s min 5.56, mean 11.88; my probe: first 20 s of the run 5.10 / 6.75 / 6.75 (seeds 1/2/3); baseline 0.07 (round-1 QA, pre-change code) | PASS (thin margin on seed 1, see 4.2) |
| 4 | `civilian_bench` green; occlusion rays per tick bounded and reported | bench green (807 us mean, turnover deficit ticks 1.01 ms); budget 16 rays = 4 points/tick, asserted every tick, observed max 16 | PASS |
| 5 | Runtime probe (release, seed 1) before/after with screenshots; `t8.py` passes | standing 0-1 (TASK-009) -> 3-13; phase B 5-12; screenshots and crops in `scratch/qa2/runtime*/`; `t8.py` exit 0 | PASS (runtime walk cut short by the TASK-023 stall) |
| 6 | clippy clean, sim+citygen green, `gta_like --bin` green, bevy_render tree empty | all green; `gta_like` 10/10 runs, gate alone 30/30 | PASS |
| - | Spec rule 2: prefer spawning ahead | code: `weight * cos + U` sort; the gate asserts ahead share >= 0.75 (1.00 shipped); my flip 3 RED at 0.39 | PASS |
| - | Spec rule 3: tuning in RON, ray budget stated, World layer | `recycles_per_tick`, ring, weight, ray height, ray budget, spacing, recycle distance in `population.ron`, each checked by `validate()`; `OCCLUSION_RAYS_PER_POINT = 4` is the law of the check (not tuning); `sight_blocked` filters `GameLayer::World` | PASS |
| R1-4.1 | Recycling burst/churn | max 1 recycled per tick (gate + probe + runtime); standing churn 5/10 s headless, 1-3/10 s runtime | FIXED (for a fixed view; see 4.1) |
| R1-4.2 | Forward preference ungated | flip 3 RED | FIXED |
| R1-4.3 | Half-visible at a corner | new 4-ray check + gate (flip 1 RED); runtime 0/75 | FIXED |
| R1-4.4 | Flaky `civilian_gate` | 10/10 suite runs, 30/30 alone; the new max-over-window assert still proves a real GLB clip (fixer flip with the wrong-root graph: 0 rad, RED) | FIXED |

## 4. Bugs and findings

### 4.1 Minor (informational): the recycle loop comes back when the player looks around
`despawn_far` recycles calm civilians that are behind the **current** look direction. When the view flips or
steps, the crowd that was ahead and off to the side (already off-frame for >= 2 s) becomes "behind" at once,
and it is recycled at 1 per tick (64/s) into the new view. Repro: `qa2_probe.rs::qa2_look_around` (view
flips every 5 s): `[98, 74, 76, 78, 83, 87]` spawns per 10 s. Runtime `runtime_look2` (60 deg every 2.5 s):
50-82 per 10 s, 6-7 spawns within about 0.1 s after each turn. Expected under the round-2 intent ("standing
does not churn"): low turnover. Actual: turnover follows camera motion. Impact: invisible (every spawn was
hidden, 0 visible in 152 in-cone sightings), frame time flat at 2.3-2.6 ms, max 1 recycle per tick holds.
The side effect for the player is that after each look-around the view refills with a new crowd and the old
one is gone. That is the "thinner street behind" owner item. The `standing_still_does_not_churn` gate covers
only a fixed view. Not a spec violation, no fix required for this task. If the owner dislikes it, the knobs
are `recycles_per_tick` and `recycle_distance`, or a minimum age before a civilian can be recycled.

### 4.2 Owner-visible: the first 20 s of running rely on a crossing burst
Seed 1, 1 Hz samples: the first 20 s of the run average 5.10, but 12 of those seconds show 0-3 in view, and
the mean comes from 7-27 at the next crossing. The gate's sliding min is 5.56 (4 Hz), a margin of +0.56 over
the original >= 5 target (the gate asserts >= 4). Clumping at crossings is unchanged: max 28-31 in view,
5-9 s per run with >= 25. Owner items 1 and 2.

### 4.3 Out of scope: runtime player stalls at a streetlight (TASK-023)
Holding W from the seed-1 spawn with yaw pi-0.035, the player stops at (6.8, 1.2, -19.6) against a streetlight
pole at the crossing (`scratch/qa2/runtime/periodic_16_B.png`). This is already filed as TASK-023. It also
limits runtime walk measurements.

### 4.4 Note: the side-ray geometry argument holds for the check as shipped
The occlusion rays skip side feet rays on the argument that a blocked ray to the top of a vertical line hides
the whole line under it (ground-standing buildings). My probe cast rays at 5 heights on the centre and both
sides for every in-cone spawn and found 0 exceptions at the collider width. At 0.45-0.6 m (wider than the
0.3 m collider, for example arms swinging) 1 of 92 in-cone spawns on the seed-1 walk had a clear ray. At
runtime nothing was visible. Leave it to the owner's eye (item 3).

## 5. Owner checklist (Russian, for the owner run)

Запуск: `cargo run --release --features dev -- --seed 1`. На seed 1 длинная улица идёт от точки спавна
примерно в +Z. На перекрёстке через ~20 м игрок упирается в фонарный столб (это TASK-023), его надо обойти.

1. **Первые ~15 с после загрузки: 0-4 мирных в кадре.** Стоять на месте и смотреть вдоль улицы. Headless:
   0 первые 11 с, 1-4 до 17 с, 8 к 30 с, 10-11 к минуте. В игре после готовности окна 3-5, к 10 с 8,
   дальше 11-13. Это принятый компромисс (отказ от переработки перед игроком, которая давала 148 появлений
   за 10 с). Устраивает ли пустая улица в первые секунды?
2. **Комки на перекрёстках: 25-29 (до 31) в кадре.** Бежать (W) вдоль улицы 60+ с. Между перекрёстками
   бывает 0-3 в кадре несколько секунд, у следующего перекрёстка сразу 20-30. Переработка по одному за тик
   это не изменила. Ручка: `spawn_forward_weight` в `assets/npc/population.ron`.
3. **Улица позади после бега и после осмотра по сторонам.** Пробежать, развернуться камерой: сзади заметно
   реже (в замере 4 против 10 впереди). Если крутить камерой туда-сюда, толпа за кадром каждый раз
   меняется на новую (невидимо, но улица "не помнит" людей). Нормально ли?
4. **Углы зданий.** Стоя у перекрёстка и глядя вдоль улицы, смотреть на углы в 30-60 м. В моих прогонах ни
   один мирный не появился наполовину видимым (0 из 75). Если заметите "выскакивание" полоски тела, это
   ширина тела больше 0.3 м (руки), ручки нет, нужен отдельный тикет.

## 6. Verdict

**SHIP-PENDING-RUNTIME.**

All six acceptance criteria pass on headless gates that run through the production composition, and the
runtime build agrees. The four round-1 findings are closed, and I checked each one myself. Standing churn
went from 148 to 5 per 10 s (headless) and 1-3 (runtime), with max 1 recycle per tick. The forward gate
goes RED under weight 0. Body-width occlusion has a RED-able gate, and 0 of 75 in-cone runtime sightings
were half-visible. `gta_like` was green 10/10, and the fixed gate alone 30/30. I did three flip-REDs myself,
each restored with a matching sha256. Open, none blocking: 4.1 (look-around churn: invisible, informational),
4.2 and the owner items (clumping, empty first seconds, thinner street behind), and 4.3 (TASK-023).

children: 0 launched / 0 reported.
