# IMPL_SUMMARY — TASK-012 (GDD T11): police on foot and arrest

Pre-flight: every file, function and API the plan names exists with the assumed shape (gang behaviour/fire_line line
refs, `search_step`, `spawn_points`, `Loadout`/`acquire`, flow/wasted, `HospitalSpawn`, perception set chain,
client `body_look`/`LookQuery`/`ShownAction::Cower`, hud/menu/input/camera hooks, QA helpers). No PLAN_BLOCKED.

## 1. What was implemented (numstat: added / removed)

Data (Step 1)
- `assets/police/escalation.ron` (new, 36) — rows, unit specs, spawn ring, arrest block, combat discipline.
- `assets/wanted/wanted.ron` (+4/−2) `punch_cop 45, wound_cop 80, kill_cop 150`; `assets/flow/respawn.ron` (+1/−1)
  `busted_arrest 2.0, busted_screen 3.0`; `assets/ui/strings.ron` (+2) `busted`, `busted_color`;
  `assets/character/visual.ron` (+8) `police_models`, `police_tint`, `swat_tint`.

Shared gunfight layer (Steps 2-3)
- `crates/gta_sim/src/tactics/mod.rs` (new, 217): `Ctx`, `Seek`, `Motion`, `Discipline`, `head_for` (takes
  `avoid: &mut f32`), `walk`, `apply_motion`, `select`, `hold_fire` (+`Aim`, `Hold`).
- `crates/gta_sim/src/tactics/fire_line.rs` (git mv from `gang/behavior/fire_line.rs`, +71/−42): `Discipline`
  instead of `GangCombatConfig`, `Shooter.faction`, `queue_slot` + its call in `unblock`.
- `crates/gta_sim/src/gang/behavior.rs` (+40/−139, now 566 lines), `gang/mod.rs` (+16: `discipline()`),
  `lib.rs` (+13: `tactics`, `police`, police config load/validate, `PolicePlugin` before `WantedPlugin`).

Wanted / flow / character / world / population (Steps 4-7)
- `wanted/mod.rs` (+13/−1), `wanted/crimes.rs` (+37/−6): cop crimes, `Victim::Cop`, always-reported branch,
  `OnExit(Busted)` reset.
- `flow/mod.rs` (+38/−3): `GameState::Busted`, `BustedPhase`, `BustedSystems`, `NpcSystems` also in Busted.
  `flow/busted.rs` (new, 71). `flow/wasted.rs` (+40/−8): shared `respawn_at`, `busted_*` in `RespawnConfig`.
- `character/mod.rs` (+11/−3) `Cuffed`; `combat/mod.rs` (+1) melee reset on `OnExit(Busted)`.
- `world/mod.rs` (+14) `PoliceStationSpawn`; `world/city.rs` (+33/−1) `station_spawn`; `world/test_area.rs` (+3)
  `STATION_SPAWN`.
- `population/mod.rs` (+8/−5): `SpawnPoint`/`spawn_points` `pub(crate)`.

Police domain (Step 8): `crates/gta_sim/src/police/` — `mod.rs` 463, `fsm.rs` 384 (incl. 5 unit tables),
`behavior.rs` 423, `dispatch.rs` 164, `arrest.rs` 95.

Client (Step 9): `src/visuals/character_config.rs` (+27/−4), `character.rs` (+25/−9), `civilian_gate.rs` (+1),
`gang_gate.rs` (+3/−3, helpers `pub(super)`), `mod.rs` (+2), `police_gate.rs` (new, 147); `src/hud/wasted.rs`
(+41/−6, `title_screen` builder), `hud/mod.rs` (+6/−3), `menu/config.rs` (+6), `input/mod.rs` (+2/−1),
`camera/mod.rs` (+1), `main.rs` (+7).

Runtime QA (Step 10): `tools/qa/scenarios/t11.py` (new, 319).

Gates (Steps 11-12): `tests/config_police.rs` (128), `tests/police_support/mod.rs` (206), `tests/police_arrest.rs`
(515), `tests/police_dispatch.rs` (231), `tests/police_crimes.rs` (121), `tests/police_fire_lines.rs` (209),
`tests/police_city.rs` (121), `tests/police_bench.rs` (58); `tests/gang_fire_lines.rs` (+50, 5 corridor cases);
`tests/common/mod.rs` (+43/−2: `sabotaged_load`, `sabotaged`), `tests/config.rs` (+5/−32);
`crates/citygen/tests/properties.rs` (+73/−40: `assert_anchor_on_sidewalk`, `police_station_anchor_on_sidewalk`).

Every touched/new `.rs` < 750 lines (largest: `tests/config.rs` 748, `src/visuals/character.rs` 656).

### Order of every `WantedLevel` reader/writer per fixed tick
`police_fsm` (AiSystems::Decide, reads last tick's `stars`/`last_known`) → `arrest_player` (after Decide, before
WantedSystems; may write `heat` on break-free) → `WantedSystems` chain `record_crimes` → `take_calls` (write heat) →
`track_search` (recomputes `stars`, `last_known`) → `forget_crimes` → `PoliceSystems` (`despawn_police`,
`dispatch_police` reads fresh `stars`). `police_alert` runs in Perceive, `police_death` in HealthSystems::Death.

## 2. Deviations from plan (and why)

1. **Shared navigation fix, not in the plan** (`tactics/mod.rs::head_for`): a route walked to its end is no longer
   re-planned by age. N1 was RED on the plan's code: cops oscillated between the graph node nearest the player and
   the player every `route_refresh_seconds` (probe `scratch/probe_police_nav.rs`, output
   `scratch/probe_police_nav_plaza.txt`: plaza cops stuck 30-45 m away [fixer: plaza probe void, player inside the tower]; hospital 3/4 stuck at ~39 m). This is a
   re-plan loop, not missing navmesh geometry. Gang suites green after the change. Log: decision entry.
2. `PoliceUnit.dest`/`search_point` are chest-height points; `dest_clear = !sight_blocked(eyes, dest)` (plan:
   `d + Y·float_height`, which would double the height for `last_known`/player points).
3. Cop `BackOff` uses `combat.reposition_gait` (no back-off gait in the plan's config).
4. `police_fsm` clears `sees` every tick while the player is dead or absent (plan: only on the AI slot). Effect: the
   P9 single-guard flip stays GREEN; P9 is flipped by removing both guards (recorded below).
5. N2 asserts equality with `citygen::sidewalk_anchor` of the first station (the plan's "within 30 m of the centre"
   was RED on correct code: 31.7 m for seed 1's large station footprint).
6. Police corridor gates: patrol pair + patrol-front/SWAT-rear are the starvation gates; the plan's
   SWAT-front/patrol-rear order is NOT starved without the queue slot (rear fires 24), kept as a no-FF case only.
7. `ArrestAttempt` lives in `police/arrest.rs` (re-exported from `police`), not `police/mod.rs`.
8. `arrest_player`: when the stored attempt's cop is no longer a live `Arrest` cop, it cancels and in the same tick
   looks for the nearest `Arrest` cop within `arrest.distance` (plan left the same-tick restart open).
9. P1 flip "Busted on the first in-range tick" goes RED on the `hold == 1/64` assert of the start tick (the stored
   hold never appears), one assert before the plan's tick assert.

## 3. Test results

| Command | Result |
|---|---|
| `cargo test -p gta_sim -j 4 --test gang_fire_lines --test gang_combat --test gangs --test gang_city` right after the pure move (Step 2 checkpoint) | 29 passed incl. `of_two_members_blocking_each_other_the_lower_index_moves` (`scratch/step2_gang_checkpoint.txt`) |
| `cargo test -p gta_sim -j 4` (final) | 289 passed, 0 failed, 31 binaries (`scratch/sim_tests_full_final.txt`) |
| `cargo test -p citygen -j 4` | all green (14 properties incl. `police_station_anchor_on_sidewalk`) (`scratch/citygen_tests.txt`) |
| `cargo test -p gta_like --bin gta_like -j 4` ×3 | 45 passed ×3 (`scratch/client_tests_x3.txt`) |
| `cargo clippy --workspace --all-targets -j 4 -- -D warnings` | clean |
| `cargo build -j 4`; `cargo build -p gta_like --bin gta_like --features dev --release -j 4` | ok (debug `--features dev` checked with `cargo check`) |
| `cargo tree -p gta_sim -e normal -i bevy_render` | empty ("nothing to print") |
| `python tools/qa/tree_check.py` | passed |
| exact-tick gates P1, P3, P9, D7 ×3 | identical, green; break free at tick 66 each run (`scratch/exact_tick_x3.txt`) |
| `python tools/qa/scenarios/t11.py --out scratch/t11` | PASS: cop in reach 10.1 s, Busted 11.5 s, BUSTED→Playing 5.08 s, respawn 0.0 m from station, confiscated, heat 0; SWAT 28.9 m at 2.5 s (8 units, 4 SWAT); stuck `{}` both runs; `log_errors` empty; frame_report: 144 Hz Fifo, no-vsync 2.76-2.79 ms |
| `python tools/qa/scenarios/t10.py`, `t9.py` (re-run, cops now exist) | both PASS without adaptation (`scratch/t10_run.txt`, `scratch/t9_run.txt`) |

Headless gates (all green): P1 `passive_player_is_busted_and_disarmed` (Busted set at tick s+95, visible one update
later; Screen at update 128, Playing at 320 after Busted; confiscated, station < 0.05 m, WantedLevel default, Crimes
empty, no Cuffed, ArrestAttempt default), P2 `cuffed_player_cannot_move`, P3 `breaking_free_adds_a_star` (heat 40 →
180, stars 2 same update, cop Attack), P4 `attacking_player_is_shot_not_arrested` (Attack at once, fires, back to
Arrest 319 ticks after the shot), P5 knockdown, P6 `lost_player_is_searched_at_last_known` (reached L in 132 ticks,
2nd search point at 290), P7/P8 busted drops queued damage / world one-shot, P9 death beats arrest; D1..D5 rows,
D6 off-frame, D7 exactly 640 ticks, D8, D9; C1..C4 (45 / 80 / 150 / 10); corridors G ×5, P ×6 + P-B1; N1, N2;
config gates ×9; fsm unit tables ×5; `classify_table` +3 rows.

Bench (`tests/police_bench.rs`, 12 SWAT + 40 civilians, seed 1): mean 1.05 / 0.96 / 0.95 ms over three runs
(p95 ≤ 1.40 ms, max ≤ 2.43 ms); `MEAN_LIMIT` 11 ms = 10× probe.

N1 navmesh evidence (corrected by the fixer; the implementer's plaza numbers were void: the plaza spot stood the
player inside the tower, IMPL_REVIEW Issue 1). 4 units at 2 stars, 40 s per spot, plaza spot = sidewalk anchor in
front of the tower, `GATE BROKEN` on a player that does not stand where it was put: hospital — all 4 reached
(2.5 / 3.8 / 5.6 / 7.4 s); plaza — all 4 reached (1.6 / 3.9 / 5.3 / 5.4 s); park — all 4 reached (1.3 / 1.9 / 2.2 /
6.3 s) (`scratch/fixer_n1.txt`). t11 runtime: no stuck cop in either run. Before the head_for fix: hospital 1/4.
Navmesh decision input: no spot shows a stuck cop on graph-only navigation.

### Flip-RED record (`scratch/flips.py` + `scratch/flips_main.py`, log `scratch/flips.txt`)
All RED unless noted: P1 hold completes at once; P1 no confiscation; P2 Cuffed ignored; P3 no BrokeFree; P4 hostile
ignored; P5 no knockdown branch; **P9 single guard (`Without<Dead>` removed) GREEN** → P9b both guards removed RED;
P6 Respond heads for the player; D1-5 `units > row.units` (5/5 RED); D6 hidden check skipped (spawn at A(30,-27));
D7 no cooldown; D8 Leave never despawns; D9 Leave not terminal (13 engaged); C1-C3 no always-report (3/3 RED, C4
green as expected); gang corridors without queue slot 5/5 RED (`scratch/step3_gang_corridor_flip.txt`); police
corridors without queue slot: patrol pair ×2 and patrol-front/SWAT-rear ×2 RED; P-B1 spare nobody; N2 hospital index;
config `swat <= units` off; citygen station margin 10000 RED (`scratch/citygen_station_flip.txt`); police_gate tint
swap RED (`scratch/police_gate_flip.txt`); N1 RED before the head_for fix (plaza 0/4 — void, plaza fixture was inside the tower; the head_for revert is now gated by `tests/route_walk.rs`, see FIX_SUMMARY).

## 4. How to verify manually (owner checklist, Russian — for QA_REPORT.md)

`cargo run --release -- --seed 1`, взять пистолет.
- [ ] Набедокурить при свидетеле → 1 звезда → через ~10-20 с приходят 2 синих копа со стволами.
- [ ] Стоять спокойно: коп подходит вплотную, через 1.5 с — сцена ареста (игрок на коленях), экран BUSTED 3 с,
      появление у участка без оружия, без брони и без звёзд.
- [ ] На 1 звезде, когда коп вплотную, рвануть спринтом прочь → 2 звезды, копы открывают огонь. Понятно ли, что это "вырвался"?
- [ ] Выстрел рядом с копом на 1 звезде → копы стреляют; через 5 с спокойствия снова пытаются арестовать.
- [ ] Спрятаться за домом: звёзды мигают, копы идут к последней точке и бродят по кругу; уйти за круг → розыск спадает.
- [ ] Поднять до 4 звёзд (убийство копа = 150): тёмные SWAT с SMG, держатся ближе. Смерть → ПОТРАЧЕНО, как раньше.
- [ ] Копы не стреляют сквозь прохожих и друг друга, не толпятся гуськом в узком проходе.
- [ ] Отличаются ли копы (синий тинт) от мирных, особенно от голубоватого мирного на модели male-c?
- [ ] После ареста или смерти старые копы уходят и не возвращаются; новая звезда приводит новых копов по таблице.
- [ ] На 5 звёздах после 4: 4 патрульных остаются, добавляются SWAT (8 SWAT + 4 патруля, а не 12 SWAT) — нормально?
- [ ] Ручки: `assets/police/escalation.ron`, `wanted.ron` (heat за копов), `respawn.ron` (2 с + 3 с),
      `visual.ron` (цвета), `strings.ron` (BUSTED).

Screenshots for the owner: `scratch/t11/busted.png` (blue BUSTED over the desaturated kneeling player),
`scratch/t11/swat.png` (SWAT at 28.9 m, small in frame).

Known risks kept (owner-visible, not gated): NPC `fire_requested` set during Busted fires once on the first Playing
tick; three shooters in a 2.4 m file may still starve (pairs gated); route budget shared with gangs at 12 cops;
`male-c` is both a civilian and a police model.

children: 0 launched / 0 reported.
