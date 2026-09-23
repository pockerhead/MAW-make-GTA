# IMPL_SUMMARY — TASK-005 (GDD T4): гуманоид с анимациями

Status: DONE. Pre-flight of PLAN_FINAL passed (every named file, type and API exists with the assumed shape;
checked in bevy_gltf / bevy_world_serialization / bevy_animation 0.19.1, avian3d 0.7.0, bevy-tnua 0.32.0 sources).
One behavioural deviation was found at runtime and fixed locally (see "Deviations").

## 1. What was implemented

Modified (git diff --stat):
| File | +/- | Step |
|---|---|---|
| `assets/third_party/manifest.ron` | +41 | 1: pack `mini-characters` (14 files) + `rig` record |
| `crates/gta_sim/src/config/manifest.rs` | +71 | 2: `PackRig`, `AssetPack.rig`, `check_names`, `PackRig::validate/clip_index`, `ThirdPartyManifest::rig_for` |
| `crates/gta_sim/tests/asset_manifest.rs` | +46/-7 | 3: rig fixtures in `manifest_fixtures_are_judged`; `shipped_manifest_is_valid` pins 12/2/7/32 and clip indices 1..5, `run` absent |
| `tools/fetch_assets.py` | +92/-3 | 4: `Some`/`None` parsing, rig schema (same messages as Rust), GLB JSON parse, rig vs GLB bytes in `pack_problems` (`--check`) and `extract` |
| `assets/character/locomotion.ron` | +1 | 5: `anim_idle_speed: 0.2` |
| `crates/gta_sim/src/character/locomotion.rs` | +23 | 5: field + `LocomotionConfig::validate` |
| `crates/gta_sim/src/lib.rs` | +4 | 5: validate in `compose_sim` |
| `crates/gta_sim/tests/config.rs` | +33/-1 | 5: shipped config validates; `locomotion_thresholds_are_validated` |
| `crates/gta_sim/src/character/mod.rs` | +9/-1 | 6: `mod anim`, re-exports, `#[require(.., AnimState)]`, registration, `FixedPostUpdate.after(PhysicsSystems::Last)` |
| `src/visuals/mod.rs` | +7/-37 | 9: new modules/plugin; placeholder capsule `visualize_character` removed |
| `src/main.rs` | +51/-9 | 10: load `visual.ron`, `preflight` validates it, checks model listed, resolves clips; resources inserted before plugins |
| `docs/design/GDD.md` | 2 lines | 13: §9.2 and §13 T4 facts (7 joints / two skins, 32 clips, no `run`) |

New:
| File | Lines | Step |
|---|---|---|
| `crates/gta_sim/src/character/anim.rs` | 105 | 6: `AnimState`, `anim_state`, `is_airborne`, `update_anim_state`, unit table |
| `crates/gta_sim/tests/anim_state.rs` | 116 | 7: 4 integration gates |
| `crates/gta_sim/tests/fixtures/manifest/{valid_rig,valid_rig_none,bad_rig_model,bad_rig_duplicate_clip}.ron` | 18 each | 3 |
| `assets/character/visual.ron` | 14 | 8 |
| `src/visuals/character_config.rs` | 112 | 8: `CharacterVisualConfig`, `CharacterClips`, `validate`, `resolve` |
| `src/visuals/character.rs` | 229 | 9: `CharacterVisualsPlugin` (FromWorld graph, spawn observer, `on_model_ready`, tint, driver, `playback_rate`) |
| `src/visuals/character_gate.rs` | 289 | 11: 7 headless client gates |
| `tools/qa/scenarios/t4.py` | 243 | 12: runtime BRP scenario |

Every new tuning number is in `assets/character/locomotion.ron` (`anim_idle_speed`) or `assets/character/visual.ron`.
The only new `const` is `MODEL_YAW = PI` (coordinate-convention law: glTF +Z vs body -Z). Test-local `FULL_JUMP_TICKS`
/`SHORT_HOP_TICKS` are test inputs, not tuning.

## 2. Deviations from plan

1. **Airborne signal (design-level, runtime-found).** Plan step 6 used `controller.is_airborne().unwrap_or(false)`.
   First `t4.py` run: the body hopped (y 1.20 -> 2.02 -> 1.20) but `AnimState` stayed `Idle` for the whole jump
   (`scratch/implementer/t4_run1.log`). Cause: Tnua's walk basis sets `airborne_timer` only when the ground sensor
   loses the floor; the sensor reaches `float_height + cling_distance` = 2.05 m, so any hop under ~1 m is "grounded"
   (`bevy-tnua-0.32.0/src/builtins/walk.rs:404-440`). Probe `scratch/implementer/probe_short_hop.rs`: hold 5 ticks ->
   rise 0.63 m, 0 airborne ticks; hold 10 -> 0.85 m, 0; hold 30 -> 1.06 m, 21 (the plan's headless jump gate passed
   only because the full jump barely clears 2.05 m). Fix: new `pub fn is_airborne(controller)` in `anim.rs` =
   `is_airborne().unwrap_or(false) || action_discriminant() == Some(Jump)`; the jump action stays active until landing
   (`jump.rs:452-490`). `anim_state` (pure fn) is unchanged. Gate: `jump_goes_up_then_falls_then_lands` now also runs a
   10-tick short hop; gate 4 uses the shared `is_airborne`. Logged as `decision` in log.jsonl; lesson in PCTX_PROPOSALS.md.
   Consequence for the owner: stepping off a curb lower than ~1 m still shows no `Fall` (Tnua clings to the ground),
   which reads as intended; a real drop shows `Fall` after coyote 0.12 s.
2. Unit table row `(0, 0, -3.14)` became `(0, 0, -3.145)`: clippy `approx_constant` (deny) flags 3.14 as PI. Same
   boundary (3.15) is tested.
3. `CharacterVisualConfig.model` is `pub(crate)` (plan let the implementer choose) so `main.rs::preflight` reads it.
4. `on_model_ready` carries `#[allow(clippy::too_many_arguments)]` (9 params, as the plan's signature has).
5. `t4.py`: failures from all phases are collected and raised at the end (summary.json always written); a phase
   helper runs the poll/screenshot loop for gait and jump phases alike. Criteria and windows are the plan's.

Not implemented: nothing. Step 14 (README/AGENTS/narrative-graph) is the orchestrator's, untouched.

## 3. Test results

| Command | Result |
|---|---|
| `python tools/fetch_assets.py --cache maw/tasks/in_progress/TASK-005/scratch` | mini-characters installed from the cached zip; city packs skipped |
| `python tools/fetch_assets.py --check` | `third-party packs match the manifest` (includes rig vs GLB bytes for 12 models) |
| `--validate-only` on 5 fixtures | valid_minimal / valid_rig / valid_rig_none: valid; bad_rig_model: `rig model 'missing.glb' is not listed in files`; bad_rig_duplicate_clip: `rig clips has duplicate 'idle'` |
| `cargo build` | ok |
| `cargo clippy -- -D warnings` | ok |
| `cargo clippy -p gta_sim --tests -- -D warnings` | ok |
| `cargo clippy -p gta_like --tests -- -D warnings` | ok (no pre-existing warnings either) |
| `cargo test -p gta_sim -p citygen` | all green: citygen 8+3+0+11, gta_sim lib 2, anim_state 4, asset_manifest 3, city 6 (1 ignored, pre-existing), config 5, jump 3, movement 4, terrain 2 |
| `cargo test -p gta_like --bin gta_like` | 14 passed (7 new `character_gate`) |
| `python tools/qa/tree_check.py` | tree checks passed |
| `cargo tree -p gta_sim -e features -i bevy_render` | nothing to print (empty) |
| `python tools/qa/scenarios/t4.py --out .../scratch/implementer/t4` | **PASS** (run 2, `scratch/implementer/t4_run2.log`, `t4/summary.json`, 34 PNGs) |

t4.py run 2 facts: model rows 1, wired AnimationPlayer rows 1; run window 400..1100 ms all `Run` (shift 2.95 m),
sprint all `Sprint` (4.25 m), walk all `Walk` (1.24 m); jump: Idle -> Jump (94 ms) -> Fall (219 ms) -> Idle (500 ms
onward); no gltf/asset/animation ERROR lines. Screenshot intervals are 125..282 ms (target 200; one BRP call per
poll adds jitter), recorded per shot in summary.json. Game process shut down (none left running).

### Flip-RED (script `scratch/implementer/flip_red.py`, results `scratch/implementer/flip_red_results.jsonl`; every file restored)
| # | Sabotage | Observed |
|---|---|---|
| 1a | `velocity.y > 0.0` -> `>= 0.0` | RED `anim_state_table` |
| 1b | horizontal -> `velocity.length()` | RED `anim_state_table` |
| 1c | walk/run split -> `run_speed` | RED `anim_state_table` |
| 2 | `update_anim_state` not registered | RED `gaits_map_to_states`, `jump_goes_up_then_falls_then_lands`, `anim_state_matches_post_step_velocity`; `idle_after_settle` stays GREEN (default is `Idle`: it is liveness of composition only, the plan's "all four RED" is not reachable) |
| 3 | system moved to `FixedUpdate.after(TnuaUserControlsSystems)` | RED `anim_state_matches_post_step_velocity` (per-tick assert) — the order gate does observe the physics step |
| 3b | `FixedPostUpdate.before(PhysicsSystems::First)` (extra) | RED same test |
| 4 | `anim_idle_speed: 2.0` | RED all 4 `anim_state` tests (compose_sim refuses with the field name) |
| 5a | drop `"crouch"` from rig clips | RED `shipped_manifest_is_valid` and `--check` |
| 5b | swap `"walk"`/`"sprint"` | RED `shipped_manifest_is_valid` and `--check` |
| 6 | joint `"head"` -> `"neck"` | `--check` RED (`mini-characters: character-female-a.glb: rig joints [...], manifest [... 'neck']`); Rust `asset_manifest` GREEN (responsibility boundary) |
| 7a | `bad_rig_model.ron` lists `missing.glb` | RED `manifest_fixtures_are_judged` |
| 7b | `bad_rig_duplicate_clip.ron` without the dup | RED `manifest_fixtures_are_judged` |
| 7c | Python `None` branch removed | `--validate-only valid_rig_none.ron` fails (`rig: expected a struct`) |
| 8 | `MODEL_YAW = 0.0` | RED `model_faces_body_forward` |
| 9 | `run: (clip: "run", ..)` | RED `character_visuals_reference_manifest_rig` (+3 app tests via GATE BROKEN); debug binary exits before the window: `character/visual.ron: clip "run" is not in the rig of third_party/mini-characters/character-male-a.glb` |
| 10 | no `add_observer(spawn_character_model)` | RED `character_model_spawns_under_player` |
| 11 | `Animation(index + 1)` | RED `graph_nodes_follow_manifest_clips` |
| 12a | no `set_speed` | RED `animator_follows_anim_state` |
| 12b | `node = nodes[0]` | RED `animator_follows_anim_state` (main animation) |
| 13 | airborne = Tnua basis only (the plan's original) | RED `jump_goes_up_then_falls_then_lands` (short hop) |

Gate classes: `anim_state_table`, `gaits_map_to_states`, `anim_state_matches_post_step_velocity`, rig/config/client
tests = correctness; `idle_after_settle`, `character_model_spawns_under_player`, t4 model/wiring step = liveness;
`jump_goes_up_then_falls_then_lands` = transition order. `on_model_ready` (player/graph wiring, tint) has no headless
gate (WorldInstanceReady cannot be triggered by hand); t4.py covers wiring, the tint is owner-run.

## 4. How to verify manually (owner checklist — for QA_REPORT.md, open until the owner runs it)

- `python tools/fetch_assets.py` (or `--cache maw/tasks/in_progress/TASK-005/scratch`), `cargo run --features fast`.
- Instead of the capsule there is a Kenney humanoid ~1.8 m tall, feet on the ground (not sunk, not floating).
  Implementer note: in the t4 screenshots the walking/running feet touch the shadow; in idle frames from the default
  camera the short legs are hidden behind the torso, so check feet from the side.
- W: run, model faces the direction of travel (t4 screenshots show the back while moving away: correct), feet do not
  visibly slide. Shift+W: sprint; Alt+W: walk; feet do not slide.
- Space: jump pose going up, fall pose coming down, back to idle on landing (a short tap now shows both).
- Transitions without snaps (`blend_seconds` 0.15 s). Run<->Sprint share the sprint clip and restart through a
  cross-fade: a light hitch is possible, not a blocker.
- Tint: set `tint: (1.0, 0.35, 0.35)` in `assets/character/visual.ron`, restart: clothes (and hands) turn red, head
  does not; restore `(1.0, 1.0, 1.0)`. Not exercised by any automated gate.
- Knobs if feet slide: `walk/run/sprint.native_speed` (lower -> faster legs; fitting range 0.92..1.28 and 1.51..2.66),
  `run.clip` (`"sprint"` or `"walk"`), `anim_idle_speed` in `locomotion.ron`.

Binary assets: `git add -A --dry-run` lists only text files; `assets/third_party/mini-characters/` stays ignored.
Scratch left for later stages: `scratch/implementer/{probe_short_hop.rs, flip_red.py, flip_red_results.jsonl,
t4_run1.log, t4_run2.log, t4/, mini_pack.ron.txt}`.
