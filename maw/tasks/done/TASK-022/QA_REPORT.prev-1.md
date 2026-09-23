# TASK-022 QA report: street life, spawning around the corner and ahead

Cost of error: medium. A spawn in clear view or a recycled civilian the player can see breaks silently, so
those have headless gates. Clumping, the thinner street behind and a body sliver at a corner are things the
owner sees in the first minute of play, so they go to the owner checklist.

## 0. Preflight

- Scratch read as a coverage map (the author's probes, baselines, flip-RED logs, runtime logs). I did not
  re-run the author's runtime script as verification. I wrote my own probes (`scratch/qa/`).
- Artifacts read: task.md, IMPL_SUMMARY.md, IMPL_REVIEW.md, FIX_SUMMARY.md, OPEN_DECISIONS.md, PCTX_PROPOSALS.md,
  log.jsonl (5 implementer + 3 fixer entries), the full diff `865969c..11e72ec`.
- **Counter-example I set out to find:** a civilian that spawns inside the rendered frame with part of the body
  visible, even though the two point rays (head 1.8 m, feet 0.1 m) are blocked. Examples: the camera snaps
  to a new yaw between ticks, the chest is visible while head and feet are hidden, or a shoulder sticks out
  past a building corner. Result: **did not hold as a rule violation.** 0 of 709 in-cone spawns had a clear
  chest ray (in my walk and snap-camera probes). Head/chest/feet rays were cast independently of
  `sight_blocked`. For boxes that stand on the ground this follows from geometry: the head ray and the feet
  ray lie in one vertical plane, so if a building blocks the head ray it also blocks everything below it. The
  shoulder case does happen at a low rate: 2 of 113 in-cone spawns on seed 1 had a clear ray to a point 0.25 m
  to the side at chest height. At runtime I caught one civilian half-visible at a building corner 0.08 s after
  it was first seen at 51 m (see 4.3). That is a feel item for the owner, not a rule violation.
- Dead-end triage: implementer entry "node-only = 0.00" checked against the code and against my baseline run
  on the pre-change code (0.07 mean, see below). Fixer entries (recycle_distance 50, no candidate cap, recycling
  off in two old tests) checked against the code and the bench output.

## 1. Environment

No docker or dev server. Direct cargo + headless Bevy `App` (production `compose_sim`, `city_app(seed)`),
plus the real windowed release build driven over BRP (`tools/qa/brp.py`, `--features dev`, seed 1).
Host: i9-11900K, RTX 4070 Ti, Vulkan, window 1280x720, default present mode Fifo, switched to
`AutoNoVsync` over BRP for frame-cost numbers.

Reproduce:
```
cargo clippy --workspace --all-targets -- -D warnings
cargo test -p gta_sim -p citygen
cargo test -p gta_like --bin gta_like            # run several times, see 4.4
cargo tree -p gta_sim -e normal -i bevy_render
# QA probes: copy scratch/qa/qa_probe_task022.rs into crates/gta_sim/tests/, then
cargo test -p gta_sim --test qa_probe_task022 -- --nocapture
python maw/tasks/in_progress/TASK-022/scratch/qa/qa_runtime.py          # env QA_OUT / QA_STAND / QA_END
python tools/qa/scenarios/t8.py --out maw/tasks/in_progress/TASK-022/scratch/qa/t8
```
Nothing to clean up: I started no containers and no services. Every game process ended with
`brp_extras/shutdown`, and `tasklist` shows no `gta_like`. The temporary probe test files were removed from
`crates/gta_sim/tests/`. Copies are in `scratch/qa/`. `git status --short` shows only task-dir files (QA_REPORT.md, log.jsonl, PCTX_PROPOSALS.md).

## 2. Test results

### Existing suites
| Command | Result |
|---|---|
| `cargo clippy --workspace --all-targets -- -D warnings` | clean |
| `cargo test -p gta_sim -p citygen` (after `touch crates/gta_sim/src/lib.rs`) | all green, 0 failed (`scratch/qa/test_sim_citygen.txt`); street_spawn 3, civilian_city 7, civilian_bench 2, config 28 |
| `cargo test -p gta_like --bin gta_like` | **flaky**: 37/38 in 3 of 7 runs on HEAD, 3 of 8 on base `865969c` (same test, same message). Pre-existing, not caused by TASK-022 (4.4) |
| `cargo tree -p gta_sim -e normal -i bevy_render` | "nothing to print" (empty) |
| `rustfmt --check --edition 2024` on the changed files | clean |
| `civilian_bench` | 64 civ: mean 815 us, p95 1.0 ms, max 1.25 ms (limit 8 ms); turnover bench: mean 843 us, deficit ticks (78) mean 1.04 ms, max 16 occlusion rays/tick |
| `python tools/qa/scenarios/t8.py` | exit 0, `log_errors: []` (`scratch/qa/t8_stdout.txt`) |

### Flip-RED done by me (source restored and checked with sha256)
| Perturbation | Result |
|---|---|
| `occluded()` returns `true` right after the first ray count (occlusion ray disabled) | `occluded_in_cone_accepted_clear_in_cone_rejected` RED "spawned in clear view inside the cone", `no_spawn_in_clear_view_over_60s_turning` RED "tick 0: spawned in the cone in clear view". Restored: sha256 `e638cad0...863d` matches |
| `spawn_forward_weight: 0.0` in RON | `street_ahead_stays_populated` **stays GREEN** (mean 6.21, min 20 s window 4.59). Restored: sha256 `d50cc9dd...221f` matches. See 4.2 |

### New QA probes (`scratch/qa/qa_probe_task022.rs`, output `scratch/qa/qa_probe_output.txt`)
| Probe | Result |
|---|---|
| Seed-1 run (stand 15 s, then run 60 s along the longest open street), counted each second | mean 13.15; **first 20 s of the run 7.05**; sliding 20 s min 7.05 |
| Same, seeds 2 / 3 / 7 (not required, robustness) | mean 11.85 / 13.03 / 11.83; first 20 s 7.00 / 7.15 / 15.80; all >= 5 |
| **Baseline:** same probe on pre-change code (`865969c` population/mod.rs + population.ron) | mean **0.07**, first 20 s 0.20 (`scratch/qa/qa_baseline_task022.rs`) |
| Same with recycling off (`recycle_distance 150`) | mean 7.12, first 20 s 1.65: recycling is what makes the first 20 s work |
| Independent occlusion check on every in-cone spawn during the runs (raw `cast_ray`, World mask, head/chest/feet) | 0 violations out of 113+99+99+65 in-cone spawns |
| Snapping camera: random yaw every 0.5 s for 60 s while walking | 372 spawns, 333 in the cone, 0 with a clear chest ray |
| Shoulder peek (chest height +-0.25 m sideways) | 2/113 (seed 1), 0 on seeds 2/3/7 |
| Recycling burst | up to **22-23 civilians recycled in a single tick**, 273 recycled in 75 s on seed 1 |
| Standing still for 120 s looking along the street | spawns per 10 s with recycling `[148, 88, 42, 27, 11, 6, 7, 8, 5, 12, 10, 9]`, without `[40, 0, 0, 1, 2, 2, 1, 0, 0, 0, 0, 4]`; headless max tick 1.6-2.1 ms in the churn phase (one 8 ms OS-type spike later) |

### Runtime (release, `--features dev`, seed 1, AutoNoVsync)
`scratch/qa/qa_runtime.py`, logs `scratch/qa/runtime/log.json`, `scratch/qa/runtime2/log.json` (with player position).
- Standing 30 s with the camera looking along the long street, count in the cone within 60 m: grows 3 -> 19
  (mean 12.0 / 12.5 over the two runs). TASK-009 measured 0-1 here.
- Running: the first ~10 s of the run show 20-22 in view. After ~42 m the player **stalls at a crossing near
  (5.6, 1.1, 5)**. It also stalls with `max_civilians: 0` (`scratch/qa/runtime_nociv/log.json`), so the stall
  is not TASK-022 (4.5). The rest of the "run" samples (3-8 in view) come from a player standing still and
  do not measure street life. The density criterion is carried by the headless gate and my probe.
- Frame cost: diagnostics average frame time 2.2-3.9 ms (255-440 FPS) at every 5 s sample, including the
  first 10 s of churn after load. No hitch in the 120-frame averages. The present mode switched from Fifo to
  AutoNoVsync.
- 59 + 63 civilians first seen already inside the cone. Each was marked on the next screenshot (`*_marked.png`).
  Crops in `scratch/qa/runtime/close_spawn_crops.png`: the civilians at 47-53 m sit behind building facades,
  with nothing visible at the mark.
- `spawn_13_zoom.png` / `spawn_14_zoom.png`: a civilian first seen at 51 m at t=15.58 is half visible at the
  building corner at t=15.66 (a head and a body sliver) and fully out at t=16.17. It spawned right behind the
  corner and walks toward the player. The spawn is "not seen" by the rule, but it shows up at once. This is
  owner item 3.
- `t8.py`: exit 0, no log errors.

## 3. Acceptance criteria

| # | Criterion | Test performed | Result |
|---|---|---|---|
| 1 | Occluded in-cone node accepted, unoccluded rejected, real seed-1 nodes, flip-RED by disabling the ray | `occluded_in_cone_accepted_clear_in_cone_rejected` (14 occluded / 10 clear ring nodes, 40 spawns all hidden); my own flip-RED (occluded -> true) gave RED | PASS |
| 2 | No spawn at an in-cone point with a clear camera ray over 60 s with the camera turning, every spawn checked | `no_spawn_in_clear_view_over_60s_turning` (241 spawns, 171 in the cone, 0 clear); my probes: random snapping camera 333 in-cone spawns plus 4 walks, independent head/chest/feet rays, 0 violations | PASS |
| 3 | Street-life: 20 s forward walk, mean in the cone within 60 m >= 5, clearly above the baseline | gate: 60 s run, sliding 20 s min 7.66, mean 13.29; my probe: first 20 s of the run 7.05 (seeds 2/3/7: 7.00/7.15/15.80); my baseline on the pre-change code: 0.07 (first 20 s 0.20) | PASS |
| 4 | `civilian_bench` green; occlusion rays per tick bounded and reported | bench green (815 us mean); budget 16/tick, asserted every tick in the turning gate, observed max 16; turnover bench deficit mean 1.04 ms. Note: `civilian_bench` now runs with recycling off (it models a fixed crowd; the perception bound broke under churn). This was justified and disclosed, and the bound was not loosened | PASS |
| 5 | Runtime probe (release, seed 1) before/after with screenshots; `t8.py` passes | standing 0-1 (TASK-009) -> 3-19; first 10 s of the run 20-22; the longer run is blocked by an unrelated stall (4.5); screenshots in `scratch/qa/runtime*/`; `t8.py` exit 0 | PASS (runtime walk partial, headless carries density) |
| 6 | clippy clean, `cargo test -p gta_sim -p citygen` green, `cargo test -p gta_like --bin gta_like` green, bevy_render tree empty | all as required, except the pre-existing flaky `civilian_gate` in `gta_like` (fails the same way on the base commit) | PASS (with pre-existing flake noted) |
| - | Spec rule 2: spawning prefers points ahead of the view | code: sort by `weight * cos + U[0,1)` in steady phase, separation/cap kept (verified in code); **no gate goes RED if it is removed** | PASS by code, gate gap (4.2) |
| - | Spec rule 3: tuning in `population.ron`, ray budget stated, World layer | ring, forward weight, ray height, ray budget, spacing and recycle distance in RON with `validate()`; `sight_blocked` filters `GameLayer::World` | PASS |

## 4. Bugs and findings

### 4.1 Medium: recycling frees every eligible civilian in one tick, which causes churn after load and bursty refills
`population/mod.rs::despawn_far` computes `at_cap` once per tick. At the cap it then despawns **every** calm,
off-frame civilian beyond 50 m, not just enough to open room. Repro: probe `qa_walk_seed1` and
`qa_standing_churn`. Up to 22-23 recycled in one tick. Standing still after load: 148 spawns in the first
10 s, 88 in the next 10 s, settling only after about 40 s (without recycling: 40 then about 0). The spawner
refills with outside-cone points at 50-120 m, and 2 s later those points are recyclable again, so the loop
feeds itself until the in-cone occluded spots are taken. Expected: recycling frees only the budget the
spawner needs (for example, only the deficit it will refill, farthest first). Actual: bulk despawn and
thrash. Impact today: invisible (everything is off-frame), headless tick cost stays under 2 ms, and runtime
frame averages show no hitch. But this is the likely mechanism behind the 25-34 in-view bursts at crossings
(owner item 1): 20+ slots open at once and get refilled within about 0.5 s into the best-scoring spots ahead.
It also discards the uniform initial fill about 2 s after load, which weakens "initial fill stays as it
is" in practice. Not a spec violation on paper. Recommended follow-up, decided by the orchestrator or owner.

### 4.2 Minor (gate gap): the forward preference, spec rule 2, has no RED gate any more
With `spawn_forward_weight: 0.0`, `street_ahead_stays_populated` stays green (6.21 mean, min 20 s window 4.59).
The implementer's flip-RED "weight 0 -> 0.81 RED" was made against the earlier 40 s gate. After the fix
(recycling, new gate shape), it no longer holds, and FIX_SUMMARY does not say so. The mechanism works in code
(13.29 vs 6.21), but a regression that drops the forward order would pass CI. Suggested gate: assert the
mean `cos(angle to view)` of steady-phase spawns is above the mean for random order, or tighten the
street gate threshold between 6.2 and 13.3.

### 4.3 Owner-visible: civilians appear right at building corners
Rays go to the body centre line only. In 2 of 113 in-cone spawns (seed 1), a point 0.25 m to the side at chest height was
visible. At runtime, a civilian was caught half visible at a corner 0.08 s after first detection
(`scratch/qa/runtime/spawn_13_zoom.png`, then `spawn_14_zoom.png`). Owner item 3.

### 4.4 Pre-existing, not TASK-022: flaky presentation gate
`visuals::civilian_gate::every_civilian_model_animates_from_its_own_clips` fails about 35% of runs
("leg-left turned 0.02 rad ... (T-pose)") on both HEAD and base `865969c`. IMPL/FIX summaries reported
"38 passed" from one run. Proposal added to `PCTX_PROPOSALS.md`.

### 4.5 Pre-existing, not TASK-022: runtime player stalls at a seed-1 crossing
Holding W from the spawn with camera yaw pi-0.035, the player stops near (5.6, 1.1, 5) after about 42 m. The
same happens with 0 civilians (`scratch/qa/runtime_nociv/log.json`). The headless run along the same street
covers 270 m, so the runtime path drifts (x 7.2 -> 5.6) into some obstacle at the crossing (curb or prop).
Worth a separate look (T1-T3 movement/city domain).

### 4.6 Documentation drift: GDD §6.1 does not describe recycling
The task says "despawn rules stay as they are". OPEN_DECISIONS authorized recycling at the cap
(`recycle_distance`), but GDD §6.1 still lists only "despawn beyond 150 m after >= 2 s off-frame". The GDD
needs one sentence.

## 5. Owner checklist (Russian, for the owner run)

Запуск: `cargo run --release --features dev -- --seed 1`. Лучше в сторону длинной улицы от точки спавна
(на seed 1 это примерно +Z). Бежать (W) 60+ с, потом развернуться.

1. **Комки на перекрёстках.** Фиксер видел 25-34 мирных в кадре на части перекрёстков, между ними 1-2 за
   несколько секунд. QA нашёл вероятную причину (4.1): переработка за один тик освобождает до 22 мест сразу.
   Устраивает ли? Если нет, поправить переработку (по одному или по дефициту) — отдельная задача.
   Ручки сейчас: `recycle_distance`, `spawn_forward_weight` в `assets/npc/population.ron`.
2. **Улица позади после бега.** Спокойные мирные дальше 50 м вне кадра удаляются, поэтому при развороте
   улица сзади реже, чем раньше. Нормально ли это? `recycle_distance` 30-60 проходит гейт.
3. **Появление у угла.** Стоя у перекрёстка и глядя вдоль улицы, смотреть на углы зданий в 30-60 м.
   Мирный появляется за углом и сразу выходит к игроку. Иногда в первый кадр видна полоска тела или голова
   (скриншоты `scratch/qa/runtime/spawn_13_zoom.png`, `spawn_14_zoom.png`). Режет ли глаз "выскакивание"?
4. Первые ~40 с после загрузки за кадром идёт интенсивный респавн (до 15 появлений в секунду). По
   замерам заметных просадок кадра нет (2.2-3.9 мс в среднем). Если будут подёргивания в первые секунды
   на слабой машине — это оно.

## 6. Verdict

**SHIP-PENDING-RUNTIME.**

All six acceptance criteria pass on headless gates that run through the production composition. I
reproduced the occlusion flip-RED myself. My probes reproduced the no-clear-view rule on 709 in-cone spawns
with independent rays, and I re-measured the baseline on the pre-change code (0.07 vs 13.29 mean, first 20 s
of the run 7.05 >= 5). Runtime shows standing density up from 0-1 to up to 19, `t8.py` passes, and frame cost
is flat. Open items, none blocking:
- 4.1 recycling burst/churn: medium, invisible today, the likely cause of the clumping; follow-up.
- 4.2 forward-preference gate gap: minor, should be closed in the follow-up.
- 4.3 corner peeks and owner items 1-3: feel, for the owner run.
- 4.4 and 4.5 are pre-existing and outside this task. 4.6 is a one-line GDD edit.

children: 0 launched / 0 reported.
