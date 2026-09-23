# QA_REPORT — TASK-009 (GDD T8: civilians)

Branch `feature/t08-civilians` at `851e765` (fixer commit), working tree `D:/test-gta-like` (no other worktree exists:
`git worktree list` shows only this one, so the shared-`target/` dead end from `log.jsonl` no longer applies; `lib.rs`
files were touched before the first build anyway).

## 1. Environment

No docker-compose, no dev server. Test infrastructure used directly: `cargo` headless gates plus the real windowed
release build driven over BRP (`--features dev`, JSON-RPC `127.0.0.1:15702`) through `tools/qa/brp.py`.

Reproduce:
```
cargo build && cargo clippy -- -D warnings
cargo clippy -p gta_sim --tests -- -D warnings && cargo clippy -p gta_like --bin gta_like --tests -- -D warnings
cargo test -p gta_sim -p citygen
cargo test -p gta_like --bin gta_like
python tools/qa/tree_check.py ; cargo tree -p gta_sim -e normal -i bevy_render
cargo build --release --features dev
python tools/qa/scenarios/t8.py --out maw/tasks/in_progress/TASK-009/scratch/qa/t8_qa
python maw/tasks/in_progress/TASK-009/scratch/qa/qa_street_life.py
python maw/tasks/in_progress/TASK-009/scratch/qa/qa_fps.py
# QA headless probes: copy scratch/qa/qa_t8_probe.rs to crates/gta_sim/tests/, then
cargo test -p gta_sim --test qa_t8_probe -- --nocapture --test-threads 1
```
Services started: none besides the game process (launched and shut down by each BRP script; `tasklist` shows no
`gta_like` left). The temporary test file `crates/gta_sim/tests/qa_t8_probe.rs` was moved into `scratch/qa/`;
`git status --short` is clean.

## 2. Test results

### Existing and new suites
| Command | Result |
|---|---|
| `cargo build` | OK |
| `cargo clippy -- -D warnings` (+ `-p gta_sim --tests`, `-p gta_like --bin gta_like --tests`) | clean |
| `cargo test -p gta_sim -p citygen` | all green: citygen 9 + golden 3 (1 ignored, pre-existing) + perf 0 (1 ignored) + properties 13; gta_sim lib 29, anim_state 4, asset_manifest 3, city 6 (1 ignored, pre-existing), civilian_bench 1, civilian_city 6, civilians 10, config 25, health 6, jump 3, melee 19, movement 4, respawn 5, shooting 16, terrain 2 |
| `cargo test -p gta_like --bin gta_like` | 38 passed |
| `tree_check.py` / `cargo tree -p gta_sim -e normal -i bevy_render` | passed / empty |

Failure list vs base: zero failures now; the only removed line in pre-existing test files is
`character_gate.rs` `.get(&animations.graph)` → `graphs[0]` (the API change the plan names), so no existing test was
dropped or renamed.

Bench (`civilian_bench`, test profile, my run): 64 civilians × 640 ticks mean 0.713 ms, p50 0.694, p95 0.838, max
1.80 ms; alive 64..64; max perception agents 16/tick, rays 3/tick. 32 civilians: mean 0.680 ms.

### Flip-RED done by QA (each restored, sha256 verified OK)
| Perturbation | Gate | Result |
|---|---|---|
| perception fix removed (`.filter(\|_\| !reporting \|\| true)`) | `corpse_in_sight_does_not_cancel_its_own_call`, `a_shot_still_interrupts_a_corpse_call` | both RED (`civilians.rs:424`, `:444`) |
| despawn distance × 0.8 in `despawn_far` | `despawn_after_2s_offscreen_beyond_150m` (acceptance) | RED (`civilian_city.rs:420`, B despawned) |
| hearing radius × 0.4 in `perceive` | `gunshot_at_20m_flee_or_cower_within_one_cycle` (acceptance) | RED ("a civilian within hearing never reacted") |

### New QA probes (independent of the author's gates; `scratch/qa/qa_t8_probe.rs`)
| Probe | What | Result |
|---|---|---|
| `qa_real_spawner_crowd_stays_on_graph` | seed-1 city, the REAL spawner fills to 40, 60 s (3840 ticks), shot into the air at 30 s; every 16 ticks every live civilian (all states incl. Flee/Cower) checked: on an edge, ≤ 1.5 m (narrowest sidewalk half-width) off its segment; a Wander walker that moved < 1 m over 5 s = stuck | PASS: max alive 40, 0 violations, worst 0.73 m off-edge, 0 stuck |
| `qa_hurt_interrupts_report_same_tick` | pending `Hurt` on a caller → Flee/Cower in that tick | PASS |
| `qa_corpse_repeat_calls_and_cower_duration` (measurement) | witness 6 m from a fresh body, 29 s | caller: **8 calls** back to back (Report 256 ticks, Wander 4 ticks, Report again) standing next to the body; cowerer (and a caller at 2.4 m, where cower outweighs report): **crouches the whole 29 s** |

### Runtime (release, `--features dev`, seed 1)
- `t8.py` → PASSED (`scratch/qa/t8_qa/summary.json`): first fill 0.234 s after `Playing`, 20 civilians, 4 closer
  than 60 m; cap 40 after 7.1 s; 6 models in use (2..12 each); pistol 12 → 11; 9 in hearing range, Wander 9 → Flee 6 +
  Cower 3 (share 0 → 1); `PerceptionLoad` 11 agents / 0 rays; no ERROR lines.
- FPS at 40 civilians (`scratch/qa/fps.json`): window `present_mode: Fifo`, 144 FPS = the monitor refresh (144 Hz), frame
  6.94 ms. Frame COST with `AutoNoVsync` set over BRP: avg frame time 2.41-3.00 ms over five 1 s samples (≈ 350-419 FPS).
- Street life (`scratch/qa/qa_street_life.py`, `scratch/qa/street_life/log.json`): live civilians inside the published
  camera cone (occlusion ignored, so the real visible count is lower): standing 21 s after load: 1-6 in the cone, of
  them 0-1 within 60 m; turning 90° steps: 6/16/16/20 in the cone, 1-3 within 60 m; walking forward 8 s: 5 → 0.

Screenshots I looked at:
- `t8_qa/first_fill.png`, `t8_qa/crowd.png`: the view down the spawn street, no civilian visible at all (by design:
  spawns are outside the cone), even with 40 alive in `crowd.png`.
- `t8_qa/scatter.png`: camera pitched into the sky, HUD `11 / 12` — only confirms the shot; the scatter is in the numbers.
- `street_life/yaw_180.png`: three or four tiny civilians far down the street (≈ 50-100 m), walking, no T-pose visible
  at that size. `yaw_270.png`: one civilian far left across the road. `after_walk.png`: one civilian cut off at the
  right edge next to the player, the street ahead empty.

## 3. Acceptance criteria

| Criterion | Test performed | Result |
|---|---|---|
| Headless: wander не уходит с графа | `wander_stays_on_the_graph` (12 walkers, 30 s) green; QA probe with the real spawner, 40 civilians, 60 s, incl. a flee phase: 0 violations, worst 0.73 m | PASS |
| Headless: выстрел в 20 м → Flee/Cower в пределах одного цикла | `gunshot_at_20m_flee_or_cower_within_one_cycle` green; QA flip (hearing radius × 0.4) RED | PASS |
| Headless: деспавн за 150 м после 2 с вне кадра | `despawn_after_2s_offscreen_beyond_150m` green; QA flip (distance × 0.8) RED | PASS |
| Headless: бенч 64 NPC × 640 тиков, порог на порядок | `civilian_bench` green, mean 0.71 ms < 8 ms, bounded perception work asserted | PASS |
| Runtime QA: `t8.py` exists and passes via `brp.py` (counts by state, shot into the air, share of Flee/Cower up, screenshot, diagnostics at 40) | ran it on the release dev build | PASS (numbers above) |
| Owner-run criterion recorded as an owner checklist | section 6 below | PASS (recorded; the run itself is the owner's) |
| Every new tuning value lives in its GDD §12 data file | `git diff 60f42ae..HEAD` grep for new `const` / float literals in production code: only config paths; the `0.1` m feet sample point in `outside_cone`/`occluded` is geometry, not tuning | PASS |
| `cargo build`, `clippy -D warnings`, `cargo test -p gta_sim` (+ citygen) green | ran all | PASS |
| Existing tests pass | full suites, no dropped test | PASS |
| (review fix) a corpse in sight does not cancel its own call; a new shot still interrupts | fix read in `perception/mod.rs:259-266`; both gates green; QA flip RED | PASS |

## 4. Bugs found

No defect against the task's acceptance criteria. Findings for the owner and the next slices:

1. **Medium (feel, owner-visible; likely to fail "улицы живые")**: with 40 civilians in a 20-120 m bubble and
   off-camera spawning, the camera shows almost nobody nearby. Measured: 0-3 live civilians inside the camera cone
   within 60 m at any moment; walking forward for 8 s drops the in-cone count from 5 to 0 (the ring in front of the
   camera never refills, by the GDD §6.1 cone rule). Repro: `python maw/tasks/in_progress/TASK-009/scratch/qa/qa_street_life.py`.
   Expected by the owner criterion: people visible on the street around the player. Actual: the street looks empty,
   figures only in the distance. This is the GDD rule plus the data, not a code defect; the knobs are `population.ron`
   (`max_civilians`, `spawn_ring`, `initial_inner_radius`, `spawn_view_margin_deg`). Owner decides.
2. **Low (feel, known, OPEN_DECISIONS)**: a civilian that picks `Cower` near a visible corpse crouches for the whole
   corpse life (measured 29 s of 29 s), because every perception cycle refreshes the crouch timer.
3. **Low (known, deferred to T10)**: a report-prone witness near a visible body makes back-to-back calls (measured 8
   calls in 29 s, 4 wander ticks between them) and stands next to the body the whole time; the witness bar reappears
   each time. T10 must count one heat contribution per body/incident (note already added to TASK-010).
4. **Info (tooling)**: `tools/qa/brp.py` `Game.resource()` casts every resource to `int`; reading a struct resource
   such as `CameraView` needs `game.call("world.get_resources", ...)` (logged as a dead end).

## 5. Verdict

**SHIP-PENDING-RUNTIME.** Build, clippy and every suite are green; every headless acceptance criterion has a
production-composition gate, three of them flipped RED by me and restored; the review fix holds (flip RED without it);
the runtime scenario passes and frame cost at 40 civilians is ≈ 2.4 ms. What is left is owner-only: street life and
crowd feel, where my measurements (finding 1) say the streets will read as sparse. That is a data/design call, not a
code blocker.

## 6. Owner checklist (запуск владельцем)

Запуск: `cargo run --release` (или `--features fast`), сид по умолчанию или `--seed 1`.

1. **Улицы живые.** Первые секунды после загрузки: люди видны на тротуарах, ходят, останавливаются, переходят дорогу,
   держатся правой стороны, толпа не застревает на углу. Повернуть камеру по кругу и пройти квартал вперёд.
   Внимание: замер QA показал 0-3 мирных в кадре ближе 60 м и пустую улицу впереди при ходьбе (находка 1). Если
   мало — крутить `assets/npc/population.ron` (`max_civilians`, `spawn_ring`, `initial_inner_radius`).
2. **Выстрел в воздух разгоняет толпу.** Взять пистолет, подойти к группе, выстрелить вверх: ближние бегут или
   приседают, никто не остаётся стоять. Дальние (25-40 м) могут "звонить в полицию" — над головой полоска, заполняется
   ~4 с и пропадает, если звонящего напугать или убить.
3. Разные модели и оттенки мирных, нет T-позы ни у одной модели.
4. Смерть: убитый мирный играет `die` и лежит тело ~30 с, потом исчезает; не больше 20 тел.
5. `crouch` читается как "испугался и присел".
6. **Долгое приседание у трупа (до 30 с).** Убить мирного рядом с другим: свидетель, выбравший приседание, может
   сидеть у тела всё время, пока тело не исчезнет (до 30 с). Нормально это или нужно ограничение — решение владельца.
7. Свидетель-"звонарь" у тела может звонить повторно (до ~8 раз за 30 с), полоска появляется снова — ожидаемо до T10.
8. FPS с 40 мирными: у QA 144 FPS при Fifo на 144 Гц, чистая стоимость кадра ≈ 2.4 мс.

Файлы для подстройки ощущений: `assets/npc/civilian.ron`, `population.ron`, `navigation.ron`, `perception.ron`,
`assets/character/visual.ron`, `assets/ui/strings.ron`.

## Disconfirmation

Counter-example I went looking for: "the fix filters corpses for callers, so either (a) a persistent body still cancels
the call some other way, or (b) a genuinely new threat no longer interrupts a corpse call". Checked the code path
(`perceive` nearest-threat selection, `next_state` `Report + Some(threat)`), the fixer's two gates, a QA flip removing
the fix (both gates RED), and a QA probe for `Hurt` on a caller (interrupts the same tick). It did not hold: calls now
complete with a body in sight and shots/hits still interrupt. It exposed the known repeat-call and long-cower behaviour
(findings 2-3), which are measured above.

children: 0 launched / 0 reported.
