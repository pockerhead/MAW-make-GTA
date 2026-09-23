# IMPL_SUMMARY — TASK-009 (GDD T8: civilians)

Status: IMPLEMENTED. All headless gates green, every new gate flipped RED and back to GREEN. The runtime
BRP scenario `t8.py` was written and run by the implementer; see "Runtime self-check" below.

Pre-flight: every file/type/API the plan names was checked against the checkout and the pinned sources
(`TnuaToggle` via the `bevy_tnua::*` glob re-export `lib.rs:202`, `set_stream` `rand_chacha-0.10.0/src/chacha.rs:179`,
`SystemCondition::or` `condition.rs:537`, `WalkGraph`, `roads.curb_height`, `street.sidewalk`, `MeleeHit.point`,
`HudLayout`, `CharacterAnimations`, `follow_player`, `place_damage_numbers` slots). No design contradiction.

## 1. What was implemented

Data (every new tuning value lives here, no new tuning `const`):
- `assets/npc/population.ron` (new, 13 lines), `assets/npc/perception.ron` (new, 8), `assets/npc/navigation.ron`
  (new, 4; also `keep_right`, see deviations), `assets/npc/civilian.ron` (new, 17).
- `assets/character/visual.ron` (+12): `civilian_models` (6 Kenney models), `civilian_tints`, `death: "die"`, `cower: "crouch"`.
- `assets/ui/strings.ron` (+3/-1): `hud.witness_bar`.

Sim (`crates/gta_sim`):
- `src/navigation/mod.rs` (new, 306): `NavigationConfig`, validated `SidewalkGraph`, `GraphWalker`, pure
  `wander_next` / `flee_next` / `flee_start` / `lane_target` / `steer`, graph built on Loading→Playing; unit tests.
- `src/perception/mod.rs` (new, 286): `PerceptionConfig`, `Perception { slot, pending }`, `Threat`, `ThreatKind`,
  `AiClock`, slot observer, `StimulusLog` (kept `slots` ticks), unsliced `Hurt`, sliced hearing / corpse sight / aimed
  gun with one LOS ray each, `PerceptionLoad`, `sight_blocked`, set wiring
  `(Perceive, Decide, PopulationSystems).chain().in_set(NpcSystems).after(HealthSystems::Death)` + `Decide.before(TnuaUserControlsSystems)`.
- `src/civilian/mod.rs` (new, 404) + `reaction.rs` (new, 103): configs, `Civilian { state, temperament }`
  (`require(Character, Perception, Offscreen)`), `CivilianState` enum FSM with one helper per transition,
  `civilian_bundle`, `roll_temperament`, `civilian_death` (same tick as the fatal hit), pure `choose_reaction` +
  the 9-row worked table test.
- `src/population/mod.rs` (new, 425): `PopulationConfig`, `ViewCone` (frustum bounding cone) + `CameraView(Option)`,
  `Offscreen`, `Corpse`, `Appearance`, `PopulationPhase`, `NpcRng` (stream 1), `corpse_components`, `outside_cone`,
  `occluded`, systems `age_corpses → despawn_far → spawn_civilians` (initial fill `[20, 120] m`, cone OR occluded;
  steady ring `[60, 120] m`, cone only; node spawning, shuffled candidates); unit tests of cone angles/membership.
- `src/flow/mod.rs` (+10): `NpcSystems` running in `Playing` or `Wasted`.
- `src/combat/hitscan.rs` (1 line) `unit_f32` → `pub(crate)`; `src/combat/mod.rs` (+1) re-export.
- `src/lib.rs` (+36): four modules, four configs loaded + validated, four plugins.

Sim gates:
- `tests/config.rs` (+109): shipped loads, unknown field, `spawn_ring`, `initial_inner_radius`, `slots`, `temperament_spread` fixtures.
- `tests/common/mod.rs` (+83): `test_graph`, `spawn_civilian`, `civilian_state`, `set_view`, `set_population`, ...
- `tests/civilians.rs` (new, 520): gunshot-in-one-cycle (acceptance), hurt same tick, fatal hit same tick,
  report interrupted, report completes at +256 ticks, corpse conversion, corpse limit + lifetime, NPCs through Wasted.
- `tests/civilian_city.rs` (new, 424): graph = city, wander stays on graph (acceptance), no spawn before view,
  initial fill closer than the ring, steady ring off-frame, despawn 150 m / 2 s (acceptance).
- `tests/civilian_bench.rs` (new, 167): 64 NPC × 640 ticks (acceptance) + 32 comparison, bounded perception work.

Client (`gta_like`):
- `src/camera/mod.rs` (+25): `publish_camera_view` (PostUpdate after `follow_player`, only with a player).
- `src/visuals/character_config.rs` (+64/-): civilian fields, `death`/`cower` clips, per-civilian-model rig
  resolution that must equal the player's clips, validation.
- `src/visuals/character.rs` (474 → 601): one graph per model from its own clips (same build order ⇒ same node
  indices), preloaded scenes, `ModelKey`, `model_key` / `body_tint`, mask groups per model graph, `Death` / `Cower`
  actions (death does not re-fall after a knockdown with the same clip).
- `src/hud/witness.rs` (new, 130) + `WitnessBarPlugin` in `HudPlugin`; `src/menu/config.rs` (+27) `WitnessBarConfig`.
- `src/main.rs` (+7): preflight lists every civilian model in the manifest.
- Gates: `src/visuals/civilian_gate.rs` (new, 418; real-GLB joint-motion gate, clip paths per model, death/cower
  nodes, validation), `src/hud/witness_gate.rs` (new, 112), `character_gate.rs` (2 lines: new `CharacterClips`
  fields, `graphs[0]`).

QA: `tools/qa/scenarios/t8.py` (new, 245).

## 2. Deviations from plan

- `navigation.ron` gained `keep_right: 0.5` and walkers steer to `lane_target` = `node(to)` shifted right of the travel
  direction (`navigation/mod.rs lane_target`; arrival is measured against the same point). Found by the plan's own
  gate: `wander_stays_on_the_graph` failed with two opposing walkers colliding head-on on one edge (0.6 m apart) and one
  pushed 1.58 m off the line (> 1.5 m sidewalk half-width). Logged as a decision in `log.jsonl`.
- `wander_stays_on_the_graph` liveness: "changed `to` ≥ 1 time" instead of ≥ 2. Seed-1 sidewalk edges reach 50+ m, so
  two node changes in 30 s at 1.8 m/s (minus idles) is not guaranteed; walkers start ≤ 8 m (or 4 m) before their
  target node and must also travel ≥ 20 m. Edge points instead of edge midpoints (12 midpoints ≥ 6 m apart within 40 m
  do not exist at the seed-1 spawn).
- `flee_next(graph, to, threat)` without the plan's unused `from` parameter.
- `unit_f32` needs a `pub(crate) use hitscan::unit_f32` in `combat/mod.rs` (the `hitscan` module is private).
- `SystemCondition::or` is deprecated in 0.19.1 (`clippy -D warnings`): used `or_else` (same semantics, `condition.rs:508`).
- A `Flee` refreshed by a new threat also re-runs `flee_start` (turns away from the new threat point).
- `fatal_hit_kills_in_the_same_tick` flip: the plan's flip ("order `civilian_death` after `AiSystems::Decide`") stays
  GREEN — death still lands in the same tick. Used `civilian_death.before(HealthSystems::Damage)` (death one tick late).
- `death_makes_a_corpse`: the `CollisionLayers == NONE` equality assert was dropped so the flip exercises the chest ray
  (the behaviour) instead of the sabotaged value.
- Witness bar anchor: capsule top (`CharacterBody.height / 2` above the body centre, = 1.8 m model head top) + `head_offset`,
  so the HUD needs no visual config. Witness gate app is `MinimalPlugins + WitnessBarPlugin` (like the damage-number gate),
  not `compose_sim`.
- `every_civilian_model_animates_from_its_own_clips` sets `CivilianConfig.idle_chance = 0` (named test-side mutation:
  an idle stop in the 32-update sample window would show the idle clip).

## 3. Test results

- `cargo build` — OK. `cargo clippy -- -D warnings` — clean. `cargo clippy --workspace --all-targets -- -D warnings` — clean.
- `cargo test -p gta_sim` — all green: lib 29, civilian_bench 1, civilian_city 6, civilians 8, config 25, plus every
  existing binary (anim_state 4, asset_manifest 3, city 6+1 ignored, health 6, jump 3, melee 19, movement 4, respawn 5,
  shooting 16, terrain 2).
- `cargo test -p gta_like --bin gta_like` — 38 passed.
- `cargo test -p citygen` — green (citygen not touched).
- `python tools/qa/tree_check.py` — passed; `cargo tree -p gta_sim -e normal -i bevy_render` — empty ("nothing to print").
- Bench (test profile): 64 civilians × 640 ticks mean 0.83 ms, p50 0.81 ms, p95 1.02 ms, max 2.15 ms; alive 64..64;
  max perception agents 16/tick, rays 3/tick. 32 civilians: mean 0.77 ms (comparison only).

Environment note: `D:/test-gta-like/target` is shared with `.worktrees/showcase`; its `gta_sim`/`citygen` rlibs have the
same metadata hash, so cargo sometimes linked the other tree's stale lib ("could not find `civilian` in `gta_sim`",
a spurious citygen failure). Touching `crates/*/src/lib.rs` before each cargo command fixes it (log.jsonl dead_end).

### Flip-RED record (`scratch/flip_red.py`, results in `scratch/flip_red.log`; each restored and re-run GREEN in the full suites above)

| Gate | Perturbed input | RED message |
|---|---|---|
| gunshot_at_20m | `perceive` ignores the slot | reactions not time-sliced: [81, 81, 81, 81] |
| gunshot_at_20m | log retention 1 tick | a civilian within hearing never reacted |
| gunshot_at_20m | hearing radius +1e6 | a civilian out of hearing reacted |
| hurt_reacts_same_tick | `Hurt` not written (only the sliced gunshot path) | did not react in the hit tick: Idle |
| fatal_hit_kills_in_the_same_tick | `civilian_death.before(HealthSystems::Damage)` | state != Dead in the kill tick |
| report_is_interrupted | `allow_report` ignored | call not interrupted |
| report_completes | `progress > 1.0` | call not completed after 256 ticks |
| death_makes_a_corpse | `CollisionLayers::NONE` removed | a corpse still stops bullets |
| corpse_limit_and_lifetime | youngest-first eviction | the oldest corpse survived the limit |
| npcs_live_through_wasted | `NpcSystems` only in `Playing` | walker 1.02 m off the edge during Wasted |
| wander_stays_on_the_graph | steer at target + (3,0,0) | 1.60 m off edge |
| wander_stays_on_the_graph | `wander_next` returns any node | not an edge |
| no_spawn_before_the_view_is_known | `None` treated as "nothing visible" | spawned without a view |
| initial_fill_closer_than_the_ring | initial radius = `spawn_ring.0` | no initial spawn closer than 60 m |
| initial_fill_closer_than_the_ring | hidden check skipped | spawned in view |
| steady_spawns_in_the_ring_off_frame | cone check skipped | spawned in the cone |
| despawn_after_2s | `||` instead of `&&` | A despawned before the grace |
| despawn_after_2s | no `Offscreen` reset in view | C (in view) despawned |
| civilian_bench | 10 ms sleep in `civilian_fsm` | mean tick 11.36 ms >= 8 ms |
| civilian_bench | a ray to every corpse | 66 rays for 16 agents |
| civilian_bench | slot ignored | 64 agents perceived, bound 17 |
| every_civilian_model_animates | every animator gets `graphs[0]` | animates with another model's graph |
| every_civilian_model_animates | every graph built from male-a clips | leg-left turned 0 rad (T-pose) |
| graph_clips_come_from_their_own_model | graphs built from model 0 | plays a clip of character-male-a |
| dead_and_cower_select_their_nodes | `Dead` branch disabled | dead body left the death clip |
| bar_lives_exactly_as_long_as_the_call | bar despawn skipped | bar kept after the call was interrupted |

Config fixtures (`tests/config.rs`) assert their own unique field keyword; not flipped individually.

### Runtime self-check (implementer, release `--features dev`, seed 1)

`python tools/qa/scenarios/t8.py --out maw/tasks/in_progress/TASK-009/scratch/t8_run1` — PASSED
(`scratch/t8_run1/summary.json`): first fill 0.156 s after `Playing` with 20 civilians, 4 of them closer than 60 m;
cap 40 reached 6.9 s after `Playing`; all 6 civilian models in use (2..12 each); FPS at 40 civilians ≈ 147 (frame
7.0 ms avg); pistol magazine 12 → 11; 9 civilians in hearing range went Wander 9 → Flee 6 + Cower 3 (share 0 → 1);
`PerceptionLoad` 9 agents / 0 rays in the sampled tick; no ERROR lines. Extra look-around probe
(`scratch/probe_look_around.py`, `scratch/look_around/*.png`): after 15 s civilians are visible only in the distance —
the spawn rules keep them out of the current view, so the street right around the player looks sparse. That is the
GDD rule working; whether it feels "alive" enough is for the owner (knobs: `population.ron` `initial_inner_radius`,
`max_civilians`, `spawn_ring`). No game process left running.

## 4. How to verify manually / owner checklist

- `python tools/qa/scenarios/t8.py` (release, dev feature): first fill ≤ 1 s, cap reached, shot in the air scatters.
- Owner checklist (for QA_REPORT.md): streets alive in the first seconds; civilians walk the sidewalks, stop, cross
  roads, keep right, no crowd stuck on a corner; a shot into the air makes nearby civilians run or crouch, none stays
  standing; several distinct models and tints, no T-pose; `die` clip and a body lying 30 s; `crouch` reads as
  cowering; witness bar fills over ~4 s over a far witness (25-40 m from the shot) and vanishes when the caller is
  scared or killed; FPS with 40 civilians. Feel values: `civilian.ron`, `population.ron`, `navigation.ron`,
  `visual.ron`, `strings.ron`.
- Known owner-visible limits: after the Wasted respawn the hospital area fills only via the 60-120 m ring; a corpse in
  view keeps nearby witnesses reporting (one call per cycle) until it despawns (T10 consumes calls).

children: 0 launched / 0 reported.
