# IMPL_SUMMARY — TASK-010 (GDD T9, gangs)

Pre-flight: every file, symbol and API the plan names exists with the assumed shape (checked against
the code and `~/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/` for bevy 0.19.1, bevy_gltf/pbr
0.19.1, pathfinding 4.16.0). Two mechanical mismatches were adapted; they are listed under Deviations.

## 1. What was implemented

Data (every new tuning value is in a data file, no new tuning `const`):

| File | Change |
|---|---|
| `assets/gang/gangs.ron` (new, 45) | gangs (tint, weapons), complete faction matrix, groups, hostility, combat |
| `assets/npc/population.ron` (+1) | `max_gang_members: 12` |
| `assets/combat/weapons.ron` (+1/-1) | `pickups.drop_seconds: 60.0` |
| `assets/npc/navigation.ron` (+5) | `direct_seek_distance`, `route_requests_per_tick`, `route_refresh_seconds`, `avoid_distance`, `avoid_step_deg` |
| `assets/character/visual.ron` (+6) | `gang_models` (male-d, male-f, female-c) |

Sim (`gta_sim`):

| File | Lines | What |
|---|---|---|
| `Cargo.toml`, `Cargo.lock` | +1, +36 | `pathfinding = "=4.16.0"` (+ `deprecate-until`, `integer-sqrt`), offline from the cache |
| `src/gang/mod.rs` (new) | 503 | `GangConfig` + strict validation, `Faction`, `GangState`, `GangMember`, `GangHeat`, `PlayerTerritory`, `GangRng` (stream 2), `GangSystems`, `gang_member_bundle`, `GangPlugin` |
| `src/gang/territory.rs` (new) | 278 | `GangTerritories::{new, from_layout, territory_at}`, `build_gang_territories` (one-shot on Loading→Playing); tests incl. the 33-seed sweep |
| `src/gang/fsm.rs` (new) | 435 | pure `next_state`, `choose_tactic`, `band_move`; tables from the plan (17 + 12 + 5 rows) and the shipped matrix |
| `src/gang/behavior.rs` (new) | 581 | `decay_gang_heat`, `locate_player`, `provoke_gangs`, `gang_fsm` (senses, tactic, intents, head-for with routing/avoidance), `gang_death` (corpse + dropped gun) |
| `src/population/gangs.rs` (new) | 161 | `despawn_far_gangs`, `spawn_gangs` (whole groups, HQ first, ring, hidden, shared ray budget, cap) |
| `src/population/mod.rs` | +13 | `max_gang_members`, registration after `spawn_civilians` in `PopulationSystems` + `GangSystems` |
| `src/navigation/mod.rs` | +262 | config fields + validation, `nearest_node`, `find_route` (A*, integer cm), `Route`, `plan_route`, `route_point`, `yaw_forward`, `avoid_offset`, `RouteLoad` + reset in `AiSystems::Perceive`; unit tests |
| `src/combat/pickups.rs`, `combat/mod.rs`, `combat/weapons.rs` | +46/-3, +3/-1, +4 | `Dropped`, `dropped_gun`, taken-once drops, `expire_dropped` after `collect_weapon_pickups`; `drop_seconds` + validation |
| `src/lib.rs`, `src/player/mod.rs` | +9, +2 | load/validate `GangConfig`, `GangPlugin` after `CivilianPlugin`; `Faction::Player` on the player |

Client (`gta_like`):

| File | Lines | What |
|---|---|---|
| `src/visuals/character_config.rs` | +16/-1 | `gang_models`, validated non-empty, resolved like civilian models |
| `src/main.rs` | +7 | preflight: every gang model listed in the manifest |
| `src/visuals/character.rs` | +60/-21 (640) | graphs chain `gang_models`; `model_key`/`body_tint` replaced by `body_look` (gang: key `1 + C + a % G`, tint from `GangConfig`) |
| `src/visuals/weapons.rs` | +12/-13 | held gun for every `Loadout` holder |

Gates and QA:

| File | Lines | Gates |
|---|---|---|
| `crates/gta_sim/tests/gangs.rs` (new) | 472 | the four acceptance claims + warning + Q1 + Q3 |
| `crates/gta_sim/tests/gang_combat.rs` (new) | 324 | fire, 8–15 m band, route around the wall, retreat, melee, dropped gun |
| `crates/gta_sim/tests/gang_city.rs` (new) | 309 | territories from the generator, hidden ring spawns under a turning view (+ cap 4), HQ group behind a static view, far despawn |
| `crates/gta_sim/tests/common/mod.rs` | +132/-1 (573) | `gang_floor`, `TurfLayout`, `spawn_member`, `provoke`, `set_matrix`, … |
| `crates/gta_sim/tests/config.rs` | +118 | shipped gang config, unknown field, 7 sabotage fixtures |
| `src/visuals/civilian_gate.rs` | +339/-37 (726) | `every_gang_model_animates_from_its_own_clips`, `gang_held_gun_follows_loadout_and_owner`, `gang_models_are_validated`; `models()`/`require_glbs` cover gang models |
| `tools/qa/scenarios/t9.py` (new) | 277 | runtime scenario |

## 2. Deviations from plan

1. **`GangMember.sees_focus: Option<Entity>` (new field)** — `crates/gta_sim/src/gang/behavior.rs` (`gang_fsm`,
   step 2). The plan refreshes line of sight only on the AI slot. A freshly provoked member then has
   `sees = false` for up to 3 ticks and routes toward a target in clear view, which breaks the plan's own
   `members_hold_the_8_to_15m_band` "searches == 0 on every tick" gate. Now a new focus entity is looked at
   at once (one ray), then on the slot. `aimed_at` stays slot-only.
2. **`TurfBlock` is not `Reflect`/registered** — `crates/gta_sim/src/gang/territory.rs`. citygen uses glam 0.32.1,
   bevy glam 0.33.8, so the curb stays `citygen::Vec2` (direct `contains_convex`/`dist_point_segment` calls)
   and `GangTerritories.blocks` is `#[reflect(ignore)]`. BRP still reads `GangTerritories.gangs[*].posts` (t9
   uses it).
3. **Flip replacements** (the planned perturbation could not go RED; recorded in `log.jsonl`):
   - "drop the turf check from `dwell`" is a no-op because `next_state` also requires `player_in_turf` for
     Idle→Warn; replaced by dropping `player_in_turf` from the Idle→Warn rule, and (a) of the neutrality gate
     now asserts `Idle` on every tick (with only the final state checked, a Warn/Idle flicker passed).
   - "drop the gang filter → R RED" stays GREEN: the leaked gang-1 member is released by `gang_fsm` in the same
     tick (target is the player, heat[1] = 0). Replaced by an assertion in the matrix-on punch half: the
     attacker's own gang is never set on it (RED).
   - `chase_routes_around_the_wall` asserts `sees` and the shot before "some search", so the always-direct flip
     goes RED on the behaviour, not on the search counter.
4. **`groups_spawn_hidden_in_the_ring`**: the plan's precondition "some post closer than `spawn_ring.0`" only
   holds for the ring post under the player, which the crowding rule always skips; the player now stands 6 m
   from the ring post along a sidewalk edge (6 m > 4 m separation + 1 m spread), so the no-inner-edge flip is
   visible (RED: "group at 6.0 m").
5. **`gang_held_gun_follows_loadout_and_owner` waits for the state** (≤ 60 updates for all guns, ≤ 8 for the
   drawn gun) instead of "after 1 update": in ~1 of 6 full-suite runs a model's children (hand joint + gun) were
   replaced one update after the model was wired, the model root staying. It also asserts never more than one
   gun per owner. Despawn check stays "after 1 update".
6. **Tint gate needs a stand-in glTF handler**: without `PbrPlugin` glTF meshes get no
   `MeshMaterial3d<StandardMaterial>`; the gang tint test registers `StandardMaterialStandIn` (base colour from
   the file) and `register_type::<MeshMaterial3d<StandardMaterial>>()`. Source colour is read from the file's
   `GltfMaterial`, independent of the code under test. PCTX proposal filed.
7. The GLB harness of `every_civilian_model_animates_from_its_own_clips` was extracted into `glb_app` /
   `wait_wired` / `animator_graphs` / `leg_turns` to share it; the civilian test's assertions are unchanged.
8. `navigation/mod.rs` is 568 lines (plan target ≤ 480; ~190 are tests). `civilian_gate.rs` is 726 (< 750).
9. `gang::fsm` is a `pub mod` (pure types used by behavior and the tables).

## 3. Test results

- `cargo build` — ok. `cargo clippy -- -D warnings`, `cargo clippy -p gta_sim --tests -- -D warnings`,
  `cargo clippy -p gta_like --bin gta_like --tests -- -D warnings` — clean.
- `cargo test -p gta_sim` (after `touch crates/*/src/lib.rs`) — all green: lib 41, config 38, gangs 7,
  gang_combat 6, gang_city 4, every existing suite unchanged and green.
- `cargo test -p citygen` — green (untouched).
- `cargo test -p gta_like --bin gta_like` — 41/41, run 8 times after the last client change (5 + 3), all green.
- `cargo tree -p gta_sim -e normal -i bevy_render` — "nothing to print". `python tools/qa/tree_check.py` — passed.
- `street_ahead_stays_populated`: mean **11.88**, 20 s windows **[5.56, 15.98, 14.10]** (min 5.56). HEAD without
  this change, measured by `git stash -u`: identical 11.88 / [5.56, 15.98, 14.10]. So T9 moves nothing; the
  TASK-022 value 7.63 / 3.26..8.60 had already been superseded on main before this task (finding, not
  re-anchored).
- Civilian bench, HEAD → T9: 64 civilians mean 757 → 803 us (p95 892 → 915 us); turnover 782 → 853 us
  (deficit ticks 949 → 1055 us). +6..9 %, far under the 8 ms gate; one run each, noise not separated.
- Runtime `python tools/qa/scenarios/t9.py --out scratch/qa_t9` (release, seed 1) — **pass**: HQ group of 3 at
  the gang-0 HQ post (approach post 44.1 m), before-states Idle ×3 at 12 m inside territory 0, magazine 12→11,
  all 3 members within 30 m `Attack` 0.3 s later, `GangHeat` [119.34, 0], zero `ERROR` lines. Soft evidence:
  3 guns drawn, FPS ~30 (release, this machine, with the fight), screenshots `hq.png`, `aggro.png`,
  `firefight.png`. Output: `scratch/qa_t9_run1.txt`, `scratch/qa_t9/summary.json`.
- `t8.py` regression — pass (cap in 1.7 s, scared share 0 → 0.81, no log errors). `scratch/qa_t8_run1.txt`.
- No game process left running.

Finding for the owner (feel, data-tunable): in the t9 run the player (100 HP, no armour) went 100 → 40 within
~0.5 s of the shot and was Wasted before the 3 s firefight capture (so `firefight.png` shows the Wasted
screen). Three pistol/SMG members at 12 m kill fast; knobs: `combat.trigger_seconds`, `combat.aim_error_deg`.

### Flip-RED record (`scratch/flip_red_log.txt`; runner `scratch/flip_red.py`, batches `scratch/flips_*.py`)

| Gate | Perturbation | Result |
|---|---|---|
| every_sweep_seed_has_two_territories | keep nodes of every district / skip spacing | RED / RED |
| outside_the_territory_members_stay_neutral | no turf in Idle→Warn; no `territory_at(muzzle)`; `dwell > warn_seconds` | RED (tick 3) / RED / RED (tick 192) |
| aiming_at_a_member_warns_within_one_cycle | ignore `aimed_at` | RED |
| attack_on_a_member_aggroes_the_group_within_30m | aggro victim only; ignore radius | RED (M1) / RED (C) |
| disabled_matrix_gangs_do_not_attack_each_other | matrix ignored for hits / shots; no gang filter | RED / RED / RED |
| hit_outside_the_territory_still_provokes | hits need turf | RED |
| gang_heat_decays_in_120s | decay 2·dt; attack on sight without heat | RED / RED (G) |
| gang_heat_keeps_decaying_while_wasted | decay in `PlayingSystems` | RED (12 ticks) |
| attackers_open_fire_at_the_player | aim chest + 5 m X | RED |
| members_hold_the_8_to_15m_band | band always Hold; route even when direct | RED / RED |
| chase_routes_around_the_wall | always direct seek | RED (no LOS) |
| wounded_member_retreats / close_member_punches | `tactics.retreat = 0` / `melee = 0` | RED / RED |
| dead_member_drops_its_gun | drop keeps cooldown; `left < 0` | RED / RED (3840) |
| groups_spawn_hidden_in_the_ring | no visibility check; no inner ring edge; no cap (max 4 run) | RED / RED / RED |
| hq_group_spawns_behind_a_static_view | HQ post never offered | RED |
| far_gang_member_despawns_after_2s_offscreen | `||` for `&&` | RED |
| config fixtures (7) | each validation rule removed | RED ×7 |
| every_gang_model_animates_from_its_own_clips | graph 0 for gangs; gang graphs built from the player GLB (wrong root); `config.tint` | RED / RED (leg 0 rad) / RED |
| gang_held_gun_follows_loadout_and_owner | `With<Player>` | RED |
| gang_models_are_validated | gang models not resolved; empty allowed | RED / RED |

Every flip was restored and the suites re-run green afterwards.

## 4. How to verify manually

- `cargo run --features fast -- --seed 1`, pick up the pistol on the range, walk to the gang-0 HQ (seed 1:
  around (116, 0.15, -486); gang 1 HQ around (-491, -308)). Groups of 2–4 purple (gang 0) or red (gang 1)
  members stand at the HQ and at block corners; they appear only off-screen.
- Stand within 8 m for 3 s or aim at one inside the turf: it draws, aims and walks up (Warn). Leave or back off
  beyond 12 m: it holsters.
- Shoot near them or hit one: every member of that gang within 30 m attacks; they keep 8–15 m, punch in close,
  wounded ones (< 30 % HP) run to 25 m, chasers route around blocks. A killed member drops its gun (60 s).
- BRP: `GangMember`, `GangHeat`, `PlayerTerritory`, `GangTerritories`, `RouteLoad`, `Dropped` are readable;
  `python tools/qa/scenarios/t9.py --out <dir>`.
- `GangHeat` counts fixed-time seconds; during the Wasted slow motion they pass slower in wall-clock time.

### Owner checklist (for QA_REPORT.md)

- [ ] зашёл к бандитам, спровоцировал, получил перестрелку
- [ ] обе банды читаются по тинту (фиолетовые / красные) и отличаются от мирных
- [ ] предупреждение читается как угроза (достали ствол, целятся, подходят)
- [ ] группы не появляются в кадре
- [ ] в перестрелке держат дистанцию, мажут чаще игрока, бьют в упор, раненые отходят (в прогоне t9 игрок
      умер за ~3 с от трёх бандитов на 12 м — оценить, не слишком ли смертельно)
- [ ] погоня обходит угол дома, не трётся о стены
- [ ] убитый бандит роняет ствол, его можно поднять
- [ ] FPS рядом с перестрелкой (t9: ~30 FPS release на этой машине)

children: 0 launched / 0 reported.
