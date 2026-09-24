# IMPL_SUMMARY — TASK-014 (GDD T13): sound and juice

Cost of error: mixed. The silent class (voice leak, endless-loop memory, a `.ogg` that panics at play,
mirrored pan, mirrored arc, `Time<Virtual>` writes, trauma rows that never fire) has real gates, each
flip-RED'd. The owner class (how it sounds and hits) gets its mechanism, QA evidence and the owner checklist below.

Pre-flight: every file, symbol and API the plan names exists with the shape it assumes (bevy_audio / rodio /
bevy_post_process / bevy_ui / bevy_math 0.19.1 and rodio 0.22.2 read in the registry). No PLAN_BLOCKED.

## 1. What was implemented

Sim (`gta_sim`):
- `crates/gta_sim/src/combat/hitscan.rs` (+19/−3): `TraceHit { Nothing, World, Body }`, `BulletTrace.hit`; `fire_weapons` resolves the target before writing the trace.
- `crates/gta_sim/src/combat/mod.rs` (+3/−2): re-export and `register_type::<TraceHit>()`.
- `crates/gta_sim/src/world/mod.rs` (+1/−1): re-export `dist_point_segment`.
- Tests: `shooting.rs` (+4/−2, `hit` rows Body/World), `new_city.rs` (+3/−2), `asset_manifest.rs` (+19/−6, packs, counts 26/3/3, CC0, no rig).

Assets / data:
- `assets/third_party/manifest.ron` (+65): impact-sounds, interface-sounds, music-jingles. The text matches `scratch/pr2/manifest_with_audio_final.ron` (whitespace-insensitive), and `--validate-only` gives `valid`. Installed from `scratch/kenney` with `--cache`. All three `License.txt` files carry "Creative Commons Zero" and "CC0".
- `assets/audio/mix.ron` (rewritten, 70 lines): the plan's mix with unit comments.
- `assets/juice/juice.ron` (+23): `shot/hurt/death_trauma`, `death_radius`, `camera_motion_reduced_scale`, `vignette`, `damage_arc`, `star_pulse`.
- `assets/ui/strings.ron` (+2/−1): `reduce_camera_motion: "Меньше движения камеры"`.
- `Cargo.toml` (+4): `[dev-dependencies] rodio = { version = "=0.22.2", default-features = false }`; `Cargo.lock` gains only the edge `gta_like → rodio`.

Client audio (`src/audio/`, replaces `ShotAudioPlugin`):
- `mod.rs` (23): `GameAudioPlugin` = `add_audio_source::<Synth>()` + `SoundCuesPlugin` + `SoundLoopsPlugin`.
- `config.rs` (342): `MixConfig` and its parts, `validate`, `sound_paths` (test-only), `check_sounds` (three distinct messages).
- `synth.rs` (248): `Synth` asset (Shot / Siren / City / Birds), `SynthDecoder` with the plan's formulas. Loops are endless, `total_duration = None`.
- `cues.rs` (459): `SoundClass` (7), `Sound`, `SoundStats`, `SoundBank` (FromWorld), `Pool` round-robin, the single `spawn_sound`, `spatial_gain`, mirrored `listener()`. Systems: `play_shots`, `play_impacts` (one impact per shooter and kind per frame), `play_hurt`, `play_stinger`, `play_ui_press`, `play_pause_sound` (OnEnter Paused), `play_death_sting` (OnEnter Wasted), `enforce_voice_budget`, and the listener observer on `OrbitCamera`.
- `loops.rs` (268): `AmbienceBed`, `AmbienceMix`, `SirenEmitter`, pure fns (`distance_to_polygon`, `park_weight`, `ambience_gains`, `pick_sirens`), `spawn_ambience` (Once, volume 0), `update_ambience`, `update_sirens`, `revolume_sirens`, `sync_loop_pause`.
- `gate.rs` (561): G-A1..G-A7.
- `src/main.rs` (+4/−3): `GameAudioPlugin`; preflight runs `check_sounds`.

Client juice / HUD / settings:
- `src/juice/config.rs` (+111/−6): new fields, validation, sabotage test for `vignette.max`, `star_pulse.scale` and `shake.death_radius`.
- `src/juice/mod.rs` (+70/−8): `PlayerHurt`, `StarsRaised`, `detect_player_hurt`, `detect_stars_raised`, recoil × `camera_motion_reduced_scale`, plugin wiring.
- `src/juice/shake.rs` (+47/−12): `CameraShake` is Reflect. `add_trauma` handles four row types that stack: the player's shot, melee landed or taken, hurt, and a kill within `death_radius`.
- `src/juice/vignette.rs` (52): bevy `Vignette` on the camera, `VignetteLevel`, "no flashes" → 0.
- `src/juice/damage_arc.rs` (130): `DamageArc`, `arc_angle`, spawn/refresh, follow the attacker, fade on real time.
- `src/juice/feedback_gate.rs` (360): G-J1, G-J2, G-J3 (arc).
- `src/hud/stars.rs` (+52/−3), `src/hud/mod.rs` (+4/−1): `StarPulse`, `star_pulse_scale` (BackOut), `pulse_stars`, and the G-J3 pulse rows.
- Settings: `settings/mod.rs` (+4/−1), `menu/widgets.rs` (+1), `menu/screens.rs` (+7; the `_ => continue` arm is covered), `menu/menu_config.rs` (+2), `menu/mod.rs` (+1, `pub(crate) use widgets::MenuAction`).

QA: `tools/qa/scenarios/t13.py` (402).

## 2. Deviations from plan

1. `PlayerHurt` and `StarsRaised` have no fields. `amount` and `stars` were never read, and `clippy -D warnings` failed on dead_code (`src/juice/mod.rs:27-33`).
2. `SoundBank` is built by `FromWorld` in `SoundCuesPlugin::build` instead of a Startup system. `spawn_ambience` then needs no cross-plugin ordering. `SynthSourcePlugin` is inlined as `add_audio_source::<Synth>()` in `GameAudioPlugin` (`src/audio/mod.rs:17-22`).
3. The G-J3 pulse rows live in `src/hud/stars.rs` tests, because `star_pulse_scale` is `pub(super)` in the private `hud::stars` module. The arc rows stay in `juice/feedback_gate.rs`.
4. G-A3 checks "newest survive" by position. Shooter *i* stands at `pos_i`, messages are written in *i* order, so the survivors must be exactly the last 12 positions. Serial order alone could not tell "keep oldest" from "keep newest".
5. `t13.py` changes against the plan text:
   - The ambience check uses `AmbienceMix.park >= 0.9·park_volume`. `AmbienceMix` holds gains, so the plan's `> 0.9` could never pass.
   - The ambience is measured on the park range after melee. A teleport to `park_center` lands on the SMG pickup and arms the player; melee now asserts the player is unarmed first.
   - The pistol is selected with Digit2 if the pickup did not select it.
   - The pause phase runs last and the run ends paused. The Escape hold cannot release while `Time<Virtual>` is paused (`bevy_brp_extras keys.rs:149-155` ticks `Res<Time>`), and `NextState<GameState>` is not registered for BRP.
   - The hurt screenshot is taken about 0.1 s after the damage, while the vignette is still strong.
6. G-J2 row "punch taken 0.45" is not a separate case. The rows are tested one by one and they stack by construction.

## 3. Test results

- `cargo build -j 4`: clean, no warnings.
- `cargo clippy -j 4 -- -D warnings`, `cargo clippy -j 4 --features dev -- -D warnings`, `cargo clippy -j 4 -p gta_like --tests -- -D warnings`, `cargo clippy -j 4 -p gta_sim --all-targets -- -D warnings`: clean.
- `cargo test -p gta_sim -j 4`: 35 test binaries, all `ok`. Log in `scratch/gta_sim_tests.txt`.
- `cargo test -p citygen -j 4`: all `ok`.
- `cargo test -p gta_like --bin gta_like -j 4`, run 4 times: `63 passed; 0 failed` every time. G-A2 decoded the real files; there was no SKIP line.
- `cargo tree -p gta_sim -e normal -i bevy_render`: empty. `python tools/qa/tree_check.py`: passed. `python tools/fetch_assets.py --check`: match. `python tools/qa/font_check.py`: 0 missing glyphs.
- Every touched file is under 750 lines (largest: `src/audio/gate.rs`, 561).

Flip-RED, via `scratch/flip_gates.py` (output in `scratch/flip_gates.out.txt`). The runner applies each sabotage, runs the gate, and restores the file. All 14 flips went RED:

| Gate | Sabotage | Result |
|---|---|---|
| G-A1 | manifest check disabled | RED |
| G-A3 | `enforce_voice_budget` removed | RED "alive Shot" |
| G-A3 | keep the oldest | RED "survivors are not the newest" |
| G-A4 | player/NPC branch swapped | RED |
| G-A4 | death sting plays `wanted` | RED |
| G-A5 | ambience `LOOP` | RED |
| G-A6 | siren tie-break reversed | RED |
| G-A7 | `SpatialListener::new(gap)` | RED |
| G-J1 | `fn _p(_: ResMut<Time<Virtual>>) {}` in shake.rs | RED |
| G-J2 | no `death_radius` | RED "kill far away 0 -> 0.38" |
| G-J2 | hurt row missing | RED |
| G-J2 | reduced recoil ignored | RED |
| G-J3 | arc mirrored | RED |
| G-J3 | pulse Linear | RED |

- Step 1: `hit` is always `World` → `pistol_hits_dummy...` RED (left World, right Body).
- Step 2: dropping one impact line → `shipped_manifest_is_valid` RED ("impact-sounds file count").
- G-A2 carries its own flip: a truncated file must not decode.
- G-A1 also carries three mutations, each with a different message.

Runtime: `python tools/qa/scenarios/t13.py --out target/qa/t13` exits 0 (last run). Summary is in `target/qa/t13/summary.json`:
- Listener: 1 entity, on the camera, `right_ear_x = -0.15`.
- Ambience: 2 beds. At start city 0.25, park 0. On the range city 0.125, park 0.35.
- Melee: impact +3, trauma 0.13.
- Guns: shots +6, magazine 12→6, impacts +7, ground shot impact +1.
- Hurt: vignette 0.33, trauma 0.12, hurt +1. With no_flashes the vignette stays 0.
- Arc: angle 0.7024 against Python 0.7024 (> 0 for the dummy on the right).
- Wanted: stinger +1; pulse `left > 0` with scale 1.089 → 0.97 (BackOut dip).
- Sirens: 1 siren on a `Respond` cop at 38.6 m; 0 sirens after heat 0.
- Death: sting +1, back to Playing.
- UI: pause sound +1.
- Peaks within caps: Shot 1, Impact 2, Hurt 2, Stinger 1, Siren 1, Ambience 2.
- No panics. No "No audio device" warning.
- `frame_report`: Fifo 144 Hz as shipped, no-vsync worst average 2.68 ms.
- The game process is shut down.

## 4. How to verify manually (owner checklist, "звучит и бьёт")

Run `cargo run --release -- --seed 1`.
- [ ] Pan direction: a cop or NPC firing on your right is heard on the right (rodio 0.22.2 mirror fix).
- [ ] Shots per weapon have a body and pitch variation; NPC shots fade with distance.
- [ ] Body, wall, punch, knockdown and kill impacts are readable with no clipping (`impacts.volume` 0.4).
- [ ] Player hurt thud plus red vignette reads as "меня бьют" and is not spammy under SMG fire (cap 2; if spammy, a cooldown is the next knob). The vignette at 0.35 per hit is subtle; see `target/qa/t13/hurt.png`.
- [ ] Wanted stinger `jingles_HIT00` plus star pulse (alternatives `HIT01..16`: one `mix.ron` line plus one manifest line).
- [ ] Death sting `jingles_SAX01` on "ПОТРАЧЕНО" (picked by falling pitch contour; alternatives `PIZZI01`, `NES11`, `STEEL01`).
- [ ] Sirens follow the cops, are not grating with 2, and pause with the game.
- [ ] City and park ambience, with a crossfade into the central park.
- [ ] Shake on shot, hurt and nearby death; stacked rows feel right; the damage arc points the right way (`target/qa/t13/damage_arc.png`); "бьёт" without nausea. The toggles "Уменьшить тряску", "Меньше движения камеры" and "Без вспышек" work (the settings screen now has 6 rows; check that it fits).
- [ ] Menu click and pause sound.
- [ ] No vocal "вскрик", by decision (no CC0 voice exists).

Screenshots: `target/qa/t13/{melee,gun_series,hurt,damage_arc,wanted,pause}.png`.

children: 0 launched / 0 reported.
