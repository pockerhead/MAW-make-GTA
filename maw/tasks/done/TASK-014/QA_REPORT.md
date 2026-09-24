# QA_REPORT — TASK-014 (GDD T13): sound and juice

QA: claude opus, medium. Commit under test: `eaecc09` on `feature/t13-audio-juice` (tree clean at start). All cargo
commands used `-j 4`, one at a time. The game always ran through `tools/qa/brp.py` `Game`, which passes
`--settings-id com.github.pockerhead.maw-make-gta.qa`. FPS was read only through `Game.frame_report()`.

## 0. Preflight and disconfirmation

- I read `scratch/` first, as a coverage map only: `flip_gates.py/.out.txt` (implementer), `fix/fix_flips.py/.out.txt`
  (fixer), `gta_sim_tests.txt`, and the Kenney/jingle probes. I did not re-run any author script as verification.
  My own probes are under `scratch/qa/`.
- I read TASK_FINAL, PLAN_FINAL, IMPL_SUMMARY, IMPL_REVIEW, FIX_SUMMARY, OPEN_DECISIONS, PCTX_PROPOSALS and log.jsonl.
  There were two `dead_end` entries, both from the implementer's t13.py work (the pause cannot be released over BRP,
  and the park_center teleport arms the player). Both are visible in `t13.py` as it stands: the pause phase runs last,
  and the melee phase checks that the player is unarmed.
- **The counter-example I chose:** in the real game, a Busted arrest while wearing armour still plays the hurt thud and
  lights the vignette. The respawn keeps the same entity and `Health::full` drops the armour. If the M1 fix only works
  in the headless harness, this shows up at runtime.
  **Search:** `detect_player_hurt` (`src/juice/mod.rs:89-107`) returns and forgets its pool outside `Playing`. The
  respawn runs in `OnExit(Busted)` (`crates/gta_sim/src/flow/mod.rs:139-146`), which is before the first `Playing`
  Update, so that frame re-seeds the pool. **Runtime result (probe A):** armour 1e6, arrested at 1 star, then back to
  Playing with armour 0. Over 2 s I took 19 polls: `SoundStats.spawned[Hurt]` delta 0, vignette max 0.0, trauma max
  0.0. Liveness check: `DebugDamage 10` right after gave Hurt +1. **The counter-example did not hold.**

## 1. Environment

- No docker-compose file and no dev server. I used the Cargo test runner and the real windowed release build, driven
  over BRP (`cargo build --features dev --release`, JSON-RPC on 127.0.0.1:15702).
- Host: one monitor `\\.\DISPLAY1` at 144 Hz, shipped present mode `Fifo`. An audio device was present: no
  "No audio device" warning in any run.
- Assets: `python tools/fetch_assets.py --check` gave "third-party packs match the manifest".
- Reproduce:
  - `cargo build -j 4`
  - `cargo clippy -j 4 -- -D warnings`, `cargo clippy -j 4 --features dev -- -D warnings`, `cargo clippy -j 4 -p gta_like --tests -- -D warnings`
  - `cargo test -j 4 -p gta_sim`, `cargo test -j 4 -p citygen`, `cargo test -j 4 -p gta_like --bin gta_like` (3 times)
  - `python maw/tasks/in_progress/TASK-014/scratch/qa/qa_flips.py` (my flip-RED runner; it restores each file and checks sha256)
  - `python tools/qa/scenarios/t13.py --out <dir>` (twice)
  - `python maw/tasks/in_progress/TASK-014/scratch/qa/qa_probes.py` (probes A-D)
  - `python maw/tasks/in_progress/TASK-014/scratch/qa/probe_five_stars_cost.py <label>` (twice)
  - `python maw/tasks/in_progress/TASK-014/scratch/qa/probe_settings.py` (settings screen, real OS click)
  - `python tools/qa/scenarios/t{8,9,10,11,12}.py --out <dir>`
- Services I started: only `gta_like.exe` instances, each closed with `brp_extras/shutdown` through `Game`. After
  every batch, `tasklist | grep gta_like` was empty.

## 2. Test results

### Existing suites
| Suite | Result |
|---|---|
| `cargo build -j 4` | OK |
| clippy, 3 configurations, `-D warnings` | clean |
| `cargo test -p gta_sim` | 35 binaries, all `ok`, 0 failed (`scratch/qa/gta_sim_tests.txt`) |
| `cargo test -p citygen` | all `ok` |
| `cargo test -p gta_like --bin gta_like`, 3 runs | `68 passed; 0 failed` in each run |
| `cargo tree -p gta_sim -e normal -i bevy_render` | "nothing to print" |
| `tools/qa/tree_check.py` / `font_check.py` / `fetch_assets.py --check` | passed / 0 missing glyphs / match |
| File sizes | largest touched file is `src/audio/gate.rs` at 567 lines, below 750 |

No test failed, so I had no failure list to compare against the base commit.

### Flip-RED I ran myself (`scratch/qa/qa_flips.py`, output `scratch/qa/qa_flips.out.txt`)
Each run ran the whole client suite, restored the file byte for byte and verified its sha256. `git status` was clean
afterwards.
| Sabotage | Result |
|---|---|
| vignette ignores `no_flashes` | RED `trauma_rows_and_time_untouched` |
| hurt thud spawned as `SoundClass::Impact` | RED `emitters_are_placed` |
| `detect_player_hurt` fires on `pool <= before` | RED `trauma_rows_and_time_untouched`, `busted_respawn_is_not_a_hurt` |
| sirens on `max_emitters + 1` cops | RED `sirens_ride_live_cops` |
| loops never pause (`sync_loop_pause`) | GREEN: no gate (sinks do not exist headless; runtime/owner only) |
| ambience gains non-zero in Paused/Loading | GREEN: no gate |
| death sting class uncapped | GREEN: no gate |

The three GREEN rows are coverage notes, not bugs. The pause of loops cannot be gated headless, because no sink is
ever created there. It stays on the owner checklist.

### Runtime: t13.py (updated by the fixer, never run before QA), 2 runs, both exit 0
Output is in `scratch/qa/t13_run1/`, `scratch/qa/t13_run2/` (`summary.json` + PNG files).
| Phase | Run 1 | Run 2 |
|---|---|---|
| listener | 1, on the camera, `right_ear_x = -0.15` (mirror live) | same |
| ambience | start city 0.25 / park 0; on range city 0.125 / park 0.35 | same |
| melee | impact +3, trauma max 0.108 | +3, 0.117 |
| guns | shots +6, magazine 12→6, impacts +7, ground +1 | same |
| hurt | vignette 0.344, trauma 0.175, hurt +1; no_flashes → 0.0 | 0.339, 0.167, +1; 0.0 |
| arc | 0.70238 vs Python 0.70238 | same |
| wanted | stinger +1, pulse scale 1.158 → 0.973 | +1, 1.089 → 0.972 |
| sirens | 1 on a `Respond` cop at 38.0 m; 0 after heat 0 | 1 at 38.6 m |
| death | DeathSting +1, back to Playing | same |
| ui | pause sound +1 | same |
| peaks | Shot 1, Impact 2, Hurt 2, Stinger 1, DeathSting 1, Siren 2, Ambience 2 | Siren 1, rest same |
| frame cost, no vsync | 2.74-3.04 ms (Fifo 144 Hz as shipped: 144 FPS) | 2.74-3.01 ms |

What I saw in the screenshots:
- `damage_arc.png`: a red arc tilted about 40° clockwise from screen-up, toward the dummy on the right. The direction
  is correct.
- `wanted.png`: two filled stars.
- `hurt.png`: taken about 0.1 s after the hit at intensity 0.34. No red edge is visible to my eye (owner item).
- `pause.png`: the pause menu.

### Runtime probes I wrote (`scratch/qa/qa_probes.py`, results `scratch/qa/probes/probes.json`)
- **A. Busted with armour:** PASS (see §0).
- **B. Gang firefight (t9 pose, 10 s, 78 BRP samples of live `Sound` entities per class):** live peaks Shot 2,
  Impact 2, Hurt 2, Ambience 2, and 0 samples over a cap. Spawned in the window: Shot 25, Impact 22, Hurt 16.
  frame_report during the fight: no vsync 2.73-3.66 ms.
- **C. Five stars (t11 pose, 30 s, 130 samples):** live peaks Impact 4, Shot 3, Siren 2, Hurt 2, Ambience 2, and 0
  samples over a cap. There was never a siren on a Dead/Leave cop or a missing cop (checked every sample). 12 live
  cops. Spawned: Shot 99, Impact 108, **Hurt 96** (about 2.7 per s under fire), Siren 10 (sirens move between cops as
  the nearest pair changes).
- **D. Death + stinger at the same moment:** `set_heat(2 stars)` and `DebugDamage 10000` sent back to back. Stinger +1
  and DeathSting +1. At 0.27 s both were alive. The stinger ended at 0.33 s and the death sting played until about
  1.2 s. PASS. In the real flow the stinger is always the older one, because stars are only recomputed in `Playing`.
  The reverse order (the case m1 was about) is covered by the headless gate `stingers_keep_the_death_sting`.
- **Siren desync:** this cannot be observed at runtime, because `AudioPlayer<Synth>` is not reflected (BRP lists no
  `AudioPlayer` type). Mechanism: `SoundBank::next_siren` round-robin gives two sirens spawned in the same frame
  different sweep starts (0 and 2.45 s). Sirens spawned in different frames differ by their spawn time. The headless
  gate `sirens_ride_live_cops` asserts that the first 4410 samples differ, and it goes RED with start forced to 0
  (fixer flip; I re-read that code). How it sounds is for the owner.
- **Frame cost at 5 stars (dedicated runs, `probe_five_stars_cost.py`, 2 runs × 2 × 10 samples, 12 cops):**
  3.00-4.27 ms without vsync, idle 3.06-4.51 ms. In the probe-C session, one frame_report sample averaged 23.6 ms
  (neighbours 9.6 and 6.3 ms), right after 30 s of dense BRP polling. It did not come back in the 40 dedicated
  samples. I recorded it as a one-off, not a finding. The earlier baseline without audio (TASK-013 QA, idle) was
  2.6 ms.
- **Settings screen (`probe_settings.py`, real OS click):** `probes/settings.png` shows all 6 rows ("Чувствительность
  мыши", "Громкость", "Инверсия Y", "Уменьшить тряску", "Меньше движения камеры", "Без вспышек") plus "Назад". They
  fit at 1280×720, and "Назад" sits right above the minimap row. The OS click on "Настройки" gave `spawned[Ui]` +1
  (menu click sound).

### Regressions t8-t12 (`scratch/qa/regress/`)
| Scenario | Result |
|---|---|
| t8 | PASS |
| t9 | 1 of 3 RED ("firefight evidence incomplete": both members in `Attack`, 0 rounds fired in 6 s), then 2 PASS |
| t10 | PASS |
| t11 | PASS (includes Busted with armour 1e6) |
| t12 | PASS |

The t9 red is a pre-existing timing flake of the hold-fire class (TASK-010 history), not caused by this task. The only
sim change is that `BulletTrace` gains `hit`/`attack`, plus a pure read moved above the write in `fire_weapons`. No AI
or sim system reads `BulletTrace` (grep). This is logged as a `decision`.

## 3. Acceptance criteria

| Criterion | Test performed | Result |
|---|---|---|
| Headless: the sound manifest is valid | `asset_manifest::shipped_manifest_is_valid` (26/3/3 files, CC0), G-A1 `mix_sounds_are_manifest_oggs` (3 distinct errors), G-A2 `mix_oggs_decode` (real files, no SKIP); `fetch_assets --check` | PASS |
| Headless: no juice system writes `Time<Virtual>` | G-J1 scanner green; my grep of `src/` found no `ResMut<Time<Virtual>>`/`set_relative_speed`; `step()` asserts speed 1 / not paused every update | PASS |
| Runtime t13.py exists and passes via brp.py | 2 runs, both exit 0 (§2) | PASS |
| Owner checklist in QA_REPORT | §6 | PASS (recorded; acceptance is the owner's) |
| Every new tuning value in its GDD §12 data file | new consts are only `SAMPLE_RATE`, `NOISE_SEED`, `DT` (synth laws), `COUNT`/`ONE_SHOTS`, `MIX_CONFIG`; every tuning number is in `mix.ron`/`juice.ron` (GDD §8, §12 list both); strict loaders with `deny_unknown_fields` | PASS |
| `cargo build`, clippy `-D warnings`, `cargo test -p gta_sim` (+citygen) green | §2 | PASS |
| Existing tests pass | gta_sim 35/35 binaries, client 68/68 ×3, citygen ok; t8-t12 regressions pass (t9 flake, see §2) | PASS |
| Orchestrator: Busted with armour → no hurt cue | probe A at runtime + headless `busted_respawn_is_not_a_hurt` (my flip also turns it RED) | PASS |
| Orchestrator: the death sting survives a stinger | probe D at runtime + headless `stingers_keep_the_death_sting` | PASS |
| Orchestrator: sirens desynchronised | headless gate + code; not observable over BRP | PASS (mechanism); sound → owner |
| Orchestrator: voice peaks within caps (t9 pose, 5 stars) | probes B, C: 0 over-cap samples; sirens ≤ 2, all on live cops | PASS |
| Orchestrator: frame cost with sound on | idle 2.7-3.0 ms, gang fight ≤ 3.7 ms, 5 stars 3.0-4.3 ms (no vsync, 144 Hz Fifo host) | PASS |

## 4. Bugs found

**B1 (minor, visual): one shotgun blast draws one damage arc per pellet.** `src/juice/damage_arc.rs:40-99`
`spawn_or_refresh_arcs` looks for an existing arc with `arcs.iter_mut().find(..)`. Arcs spawned earlier in the same
run are `commands.spawn` and are not in the query yet, so every `DamageDealt` of one blast (up to 10 pellets) spawns
its own arc. The same happens for two shots from one shooter inside one frame (2 fixed ticks).
- Repro: `scratch/qa/qa_probe_shotgun_blast_arcs.rs`. Append it to `src/juice/feedback_gate.rs`, run
  `cargo test -j 4 -p gta_like --bin gta_like qa_probe_shotgun_blast_arcs`, then restore the file. The output is
  `QA PROBE: arcs after one 8-pellet blast = 8`, and the test is RED. I ran it and restored the file (git clean).
- Expected: one arc per shooter. Actual: N overlapping arcs. The overlapping translucent layers compound alpha, so the
  fade barely shows until the end. Later refreshes touch only the first arc, and there is extra UI entity churn.
  Gang 0 carries shotguns (`assets/gang/gangs.ron:5`), so the player meets this in play.
- Suggested fix: keep a per-run `HashSet<Entity>` of shooters spawned in this call, or collect the hits per shooter
  first and then spawn once.

I found nothing else that fails. Coverage notes (not bugs): loop pause, the ambience state gating and the DeathSting cap
have no headless gate (§2 flips).

## 5. Verdict

**SHIP-PENDING-RUNTIME.** Every stated criterion is covered and passes, headless and at runtime (t13 ×2, my probes
A-D, t8-t12). Sound and feel are accepted only by the owner (checklist below). B1 is a minor visual defect in the
damage-arc juice and fails no stated criterion. It is a cheap fix, and the orchestrator decides whether it goes in
before merge or as a follow-up.

## 6. Owner checklist (для владельца; решает оркестратор)

Запуск: `cargo run --release -- --seed 1`.
- [ ] Звук и feel в целом: игра "звучит" и "бьёт". Выстрелы по оружию с телом и вариацией высоты, выстрелы NPC тише вдалеке, удары и попадания в тело и в стену читаются, нет клиппинга.
- [ ] Панорама: коп или NPC, стреляющий справа, слышен справа (зеркальные уши из-за rodio 0.22.2).
- [ ] Урон игроку: тупой удар `impactPunch_heavy` плюс красная виньетка. **Сила виньетки:** на `hurt.png` при intensity 0.34 (через 0.1 с после удара) красного края на глаз не видно. Возможно, мало `per_hurt 0.35` / `radius 0.9` в `juice.ron`. **Частота:** на 5 звёздах под огнём 96 ударов за ~35 с (~2.7/с), в перестрелке с бандой 16 за 10 с. Не спам ли это (следующая ручка: кулдаун).
- [ ] Стингер розыска `jingles_HIT00` плюс пульс звёзд.
- [ ] Death sting `jingles_SAX01` на "ПОТРАЧЕНО": подходит ли (выбран по падающему контуру высоты, не на слух; альтернативы `PIZZI01`, `NES11`, `STEEL01`).
- [ ] Сирены: две не воют в унисон (старты 0 и 2.45 с из 4.9 с), следуют за копами, паузятся с игрой. На 5 звёздах сирена переезжает между копами (10 спавнов за 35 с) и каждый раз начинает свип с начала. Не дёргано ли это.
- [ ] Эмбиент города и парка, переход в центральном парке.
- [ ] Тряска на выстрел, урон и смерть рядом. Дуга направления урона. Нет тошноты.
- [ ] **Экран настроек (6 строк):** `scratch/qa/probes/settings.png` влезает в 1280×720. Проверить на своём разрешении. Переключатели "Уменьшить тряску", "Меньше движения камеры", "Без вспышек" работают.
- [ ] Клик в меню и звук паузы (клик по "Настройки" дал звук, `Ui` +1).
- [ ] Вскрика нет, по решению (нет CC0-голоса).
- [ ] (B1) Дробовик банды по игроку: дуга должна быть одна и плавно гаснуть.

## 7. Files

- QA scratch: `scratch/qa/` (`qa_flips.py/.out.txt`, `qa_probes.py/.log`, `probe_five_stars_cost.py`,
  `probe_settings.py`, `qa_probe_shotgun_blast_arcs.rs`, `probes/`, `t13_run1/`, `t13_run2/`, `regress/`,
  `gta_sim_tests.txt`).
- Task dir: `log.jsonl` (+2 `decision`), `PCTX_PROPOSALS.md` (+1 lesson: same-run spawn dedupe).
- `git status --short` outside the task dir: empty. No source file changed.

children: 0 launched / 0 reported.
