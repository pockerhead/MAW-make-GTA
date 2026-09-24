# PLAN_V2 — TASK-014 (GDD T13): sound and juice

Reviewer: plan-reviewer-1 (claude opus, medium). Base: `PLAN.md` (planner, HEAD `e279b60`/`52348b6`). Every engine
claim below was re-read in `~/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/` (versions from `Cargo.lock`:
bevy 0.19.1, bevy_audio 0.19.1, rodio 0.22.2, bevy_post_process 0.19.1, bevy_ui 0.19.1, bevy_math 0.19.1, bevy_time
0.19.1, bevy-settings 0.19.1).

Cost of error: **mixed** (same split as PLAN.md). Silent class gets real gates: voice leak, loop memory,
undecodable `.ogg` (panic at play), mirrored spatial pan, mirrored damage arc, `Time<Virtual>` writes, trauma rows that
never fire. Owner class (how it sounds and feels) gets mechanism, QA evidence and the owner checklist.

---

## 0. Disconfirmation (done before the review)

**Counter-example tested:** G-J1 scans all of `src/**/*.rs` for `ResMut<Time`, `resource_mut::<Time` and
`set_relative_speed`. If the pause flow or the wasted slow-mo (which legitimately write `Time<Virtual>`), or any
client test, lived in `src/`, the gate would be RED on day one, or the planner would have to whitelist and so weaken it.
**Search:** `Grep "ResMut<Time|resource_mut::<Time|set_relative_speed|Time<Virtual>|.pause()|unpause()"` over
`src/**` and `crates/**`. **Result: did not hold.** All writers are in the sim: `crates/gta_sim/src/flow/mod.rs:83-88`
(`pause_time`/`resume_time`) and `flow/wasted.rs:78-81,104-105`. `src/` has only reads (`character_gate.rs:626`).
The scan scope `src/` is correct. The token set still has a real hole: `ResMut<'w, Time<Virtual>>` inside a
`#[derive(SystemParam)]` struct does not contain `ResMut<Time`, and `ResMut<Time` also matches an unrelated
`ResMut<TimeUpdateStrategy>`. Fixed in Step 13 below.

A second, unplanned counter-example came up while checking the spatial-audio claims, and it **held**: see R1.

---

## 1. Review notes (issues in PLAN.md, with evidence)

**R1 — BLOCKER (silent sign error): spatial panning is mirrored in the pinned rodio.**
`rodio-0.22.2/src/source/spatial.rs:57-60`:
`left_diff_modifier = (((left_dist - right_dist) / max_diff + 1.0) / 4.0 + 0.5).min(1.0)` and the mirror for the right.
Worked example: source 10 m to the camera's right, `SpatialListener::new(g)` → left ear at `-X·g/2`, right at `+X·g/2`
(`bevy_audio-0.19.1/src/audio.rs` `SpatialListener::new`). Then `left_dist - right_dist ≈ +g` = `max_diff`, so
`left = (1+1)/4+0.5 = 1.0` and `right = (-1+1)/4+0.5 = 0.5`. Channel 0 (left, `ChannelVolume` interleave) is
**louder** for a source on the right. Upstream rodio master has since swapped the two expressions
(`left_diff_modifier = (((right_dist - left_dist) ...`, https://raw.githubusercontent.com/RustAudio/rodio/master/src/source/spatial.rs),
which confirms the pinned version is wrong. Users have reported this class of problem for years (rodio issues #286, #471,
https://github.com/RustAudio/rodio/issues/286, https://github.com/RustAudio/rodio/issues/471). PLAN.md A.6 says "the camera's
local +X is right, which matches the listener's ear axis" and plans `SpatialListener::new(gap)`, which ships a mirrored
pan: a cop's siren on the right sounds from the left. The owner may not catch it. It is the same class as the mirrored
damage arc the planner did gate. **Fix:** the listener is built with mirrored offsets (`left_ear_offset = +X·gap/2`,
`right_ear_offset = -X·gap/2`) behind one pure fn with a one-line "why" comment citing rodio 0.22.2. A headless gate
drives the real `rodio::source::Spatial` with those offsets (Step 13, G-A7). The gate goes RED when a Bevy upgrade brings
a fixed rodio, and that is exactly the moment the swap must be removed.

**R2 — MAJOR: stereo files are summed, not averaged, on the spatial path.** `rodio-0.22.2/src/source/channel_volume.rs`
`next()`: `self.current_sample.map(|s| s / num_channels ...)` discards its result. A stereo source is downmixed as L+R,
up to 2× amplitude (+6 dB) on every spatial emitter. All chosen impact files and `jingles_HIT00` are stereo
(`scratch/kenney/ogginfo.txt`: `(2, 44100, …)`). Only the two interface files are mono. The non-spatial path (stinger,
UI) does not go through `ChannelVolume`. With PLAN.md's `impacts.volume: 0.8`, a loud impact peak around 0.9 per channel
becomes about 1.44 at the mixer, which clips. **Fix:** `impacts.volume: 0.4` (half of the planner's intended loudness,
which compensates the 2× sum exactly). The reason goes in the `mix.ron` comment. This goes on the owner checklist.

**R3 — MAJOR: the audio harness panics without `GlobalVolume`.** `GlobalVolume` is inserted only by `AudioPlugin`
(`bevy_audio-0.19.1/src/lib.rs` `AudioPlugin::build`: `insert_resource(self.global_volume)`). The plan's test harness
deliberately omits `AudioPlugin`, but `update_ambience` and the siren re-volume read `Res<GlobalVolume>`. Their system
params fail validation, which panics by default. **Fix:** the harness does `init_resource::<GlobalVolume>()`.
`DefaultSpatialScale` is not read by our systems, so it is not needed.

**R4 — MAJOR: QA liveness races short one-shots.** With an audio device, `cleanup_finished_audio` despawns a DESPAWN
entity when its sink drains: SMG 0.10 s, click 0.10 s, toggle 0.139 s, stinger 0.284 s. QA polls BRP every 50 ms,
plus the round trip. "`Ui` seen within 0.3 s" and "`Stinger` count == 1" are therefore flaky on a host with sound, and
over-pass on a host without one (entities live forever). **Fix:** a reflected resource
`SoundStats { spawned: [u64; 6], peak_alive: [u32; 6] }`, indexed by `SoundClass`, updated by `enforce_voice_budget`
after the cap is applied. QA asserts on deltas of `spawned` and on `peak_alive <= cap`. Live `Sound` entity polling
stays only for loops (sirens, ambience), which do not drain.

**R5 — MAJOR: pause via `OnEnter/OnExit(Paused)` misses late sinks.** Sinks are created in `PostUpdate`
(`play_queued_audio_system`), after `StateTransition` ran `OnEnter(Paused)`. A siren or ambience sink created on the pause
frame, or an ambience entity whose `Synth` sink appears later, keeps playing through the pause. **Fix:** one idempotent
`Update` system `sync_loop_pause` sets `pause()`/`play()` on `AudioSink` and `SpatialAudioSink` of classes
`Ambience | Siren`, from `State<GameState> == Paused` vs `is_paused()`.

**R6 — MINOR (ordering): siren re-volume keyed on the wrong resource.** `apply_volume` (`src/settings/mod.rs:83`) writes
`GlobalVolume` in `Update` under `resource_changed::<GameSettings>`. A siren re-volume under the same condition, in
parallel, reads the stale `GlobalVolume`. **Fix:** re-volume `run_if(resource_changed::<GlobalVolume>)`.

**R7 — MINOR (data first): the siren head offset is not a law.** Where the emitter sits on a cop is a number the owner
could turn. It moves to `mix.ron siren.height` (m above the body centre). `SAMPLE_RATE` stays a const: it is a law of
the synth and already one today (`src/audio/mod.rs:13`).

**R8 — MINOR: G-J1 token set** (see §0). Normalise each line (drop whitespace and `'<ident>,` lifetimes), then match the
exact forms `ResMut<Time<Virtual>>`, `ResMut<Time<Fixed>>`, `ResMut<Time>`, `resource_mut::<Time<Virtual>>`,
`resource_mut::<Time<Fixed>>` and `set_relative_speed(`. `.pause()` stays out on purpose, because sinks legitimately
call it (R5). A `Time<Virtual>` pause is caught by the `ResMut` param it needs.

**R9 — MINOR: `log_errors` cannot see a panic.** `t5.py:145-150` keeps only lines containing `"ERROR"`, and Rust
panics print `thread '…' panicked at`. Adding `"panicked"` to `ERROR_WORDS` (PLAN.md Step 14) does nothing. **Fix:** t13
has its own `panics(game)` over the log tail (`"panicked at"`), plus a check that `game_state` still answers after each
phase.

**R10 — MINOR: stale line references** (the implementer must not trust them). `src/hud/stars.rs` has 177 lines: the
row spawn is `:53-92`, and `HudPlugin` is `src/hud/mod.rs:18-43`, not `stars.rs:138-247`/`:187-203`/`:31-40`.
`assets/ui/strings.ron` settings strings are on `:46`, not `:372`. `src/juice/shake.rs` test literal `ShakeConfig`
is `:81-91`. Symbols are correct. Only the numbers drift.

**R11 — NOTE (feel, owner): stacked trauma rows.** Melee writes both `DamageDealt` and `MeleeHit`
(`crates/gta_sim/src/combat/melee.rs:529,538`). A punch the player **takes** therefore gives `melee_trauma` 0.25 plus
`hurt_trauma` 0.2 = 0.45. A melee kill the player lands nearby gives 0.25 + 0.4. The GDD table lists rows, not
exclusivity, so stacking is kept (clamped at 1). It goes on the owner checklist as a named item, not silently.

**R12 — NOTE: the harness uses real stand-in entities for message fields.** `compose_sim` has sim readers of
`ShotFired`/`DamageDealt` (perception, wanted). Synthetic messages use spawned stand-in entities with a `Transform`
matching the intended position (TASK-011 lesson: a posed fixture carries a matching `Transform`), never
`Entity::PLACEHOLDER`.

**R13 — NOTE: the QA arc angle check partly mirrors the implementation.** Recomputing `atan2(d·right, d·forward)` in
Python proves the wiring (yaw source, positions, which shooter), not the convention. The convention is carried by the
headless G-J3 table, whose worked examples are geometric. I re-derived them against `camera/mod.rs:122`
`Quat::from_euler(YXZ, yaw, pitch, 0)`: forward `(-sinθ,0,-cosθ)`, right `(cosθ,0,-sinθ)`, all nine rows hold. The
visual direction (the `UiTransform.rotation` sign, doc "Rotate the node clockwise", `bevy_ui-0.19.1/src/ui_transform.rs:136`)
is judged on the QA screenshot, with the attacker placed on the right.

Verified and kept as in PLAN.md (no change): `PlaybackMode::Loop` → `repeat_infinite` → `Buffered` keeps every sample
(`rodio-0.22.2/src/source/repeat.rs:9-17`). No device → no sink → DESPAWN entities never despawn
(`audio_output.rs:play_queued_audio_system` early return). Dropping a stolen voice stops it (`rodio player.rs:345-353`
sets `stopped`), so stealing really frees voices. Attenuation `(1/dist_sq).min(1)` on scaled distance. `set_volume`
replaces the sink volume, and `GlobalVolume` is applied only at sink creation. `PlaybackSettings` builders
(`with_spatial`, `with_spatial_scale`, `with_speed`, `with_volume`) and `SpatialScale::new`. `Vignette` fields, the
`intensity > 1e-4` skip, the path `bevy::post_process::effect_stack::Vignette`. `Node.border_radius`,
`BorderRadius::MAX`, `BorderColor{top,..}`. `EaseFunction::BackOut` (t=0 → 0, t=1 → 1, peak ≈ 1.10 at t ≈ 0.58, so
the row dips to ≈ 0.97). bevy-settings field-by-field load (`lib.rs:513-533`). `TimeUpdateStrategy::FixedTimesteps(n)`
also advances `Time<Real>` by `n·timestep` (`bevy_time-0.19.1/src/lib.rs:181-183`), so the recoil ratio and trauma
bounds in G-J2 are deterministic. `BulletTrace` constructors: only `hitscan.rs:235,243` and `tests/new_city.rs:323`.
Manifest counts 26/3/2 (`scratch/kenney/manifest_snippet.ron`). Paths with a space (`Audio/Hit jingles/…`) pass both
`is_safe_path` (Rust `manifest.rs:76-84`) and `safe_path` (Python). City blocks are convex CCW (`citygen/src/roads.rs:127`),
so `contains_convex(poly, p, eps)` applies (note the third `eps` argument). `citygen::dist_point_segment` exists and is reused.

---

## 2. Updated understanding (existing code, corrected)

| Area | Where | State |
|---|---|---|
| Shot sound | `src/audio/mod.rs` (179 lines) | `ShotAudioPlugin`: `ShotSound` asset (noise × exp), `play_shots` spawns non-spatial `AudioPlayer<ShotSound>` + `DESPAWN` for every `ShotFired`, with no cap |
| Mix data | `assets/audio/mix.ron`, `MixConfig` in `src/audio/mod.rs:15-69` | only `shot.{pistol,smg,shotgun}.{seconds,decay,volume}` |
| Juice | `src/juice/{mod,config,shake,hit_stop,damage_numbers}.rs`, `assets/juice/juice.ron` | `kick_camera` (`mod.rs:40-54`), melee-only trauma `add_melee_trauma` (`shake.rs:17-28`), `shake_camera` on `Time<Real>`, `CameraShake` not reflected |
| Camera | `src/camera/mod.rs` | `OrbitCamera` spawned once in `Startup` (`spawn_camera`), camera distance 3.8 m (`camera.ron:4`), `follow_player` in `PostUpdate` before `TransformSystems::Propagate` (so audio, which runs after Propagate, sees the current camera) |
| Settings | `src/settings/mod.rs` | `GameSettings{mouse_sensitivity, volume, invert_y, reduce_shake, no_flashes}`, `apply_volume` writes `GlobalVolume` on change |
| Settings UI | `src/menu/widgets.rs:13-32` (`SettingKey`, `MenuAction`, `mod widgets` private), `screens.rs:116-117,145-146,297-298`, `menu_config.rs:23-24,71-72`, `assets/ui/strings.ron:46` | 5 rows |
| Stars HUD | `src/hud/stars.rs:53-92` spawn (`Name "Wanted stars"`, `StarRow`, `CityScoped`), `update_stars`; registered in `src/hud/mod.rs:18-43` | blink only |
| Sim messages | `crates/gta_sim/src/combat/hitscan.rs:56-89`, `melee.rs:284,529-538` | `BulletTrace{shooter,from,to}` does not say what it hit |
| Flow | `crates/gta_sim/src/flow/mod.rs` | `GameState{MainMenu, Loading(default), Playing, Paused, Wasted, Busted}`, `NEW_CITY = Paused→Loading` |
| Police/Wanted | `police/mod.rs:287-313` (`CopState` incl. `Leave`, `Dead`), `wanted/mod.rs:155-168` | |
| City | `world::City(CityLayout)`, `Block{curb, is_park}`, `CityLandmarks.park_center` = centroid of the park's `inner` | absent in TestArea |
| Manifest | `assets/third_party/manifest.ron`, `crates/gta_sim/src/config/manifest.rs`, test `crates/gta_sim/tests/asset_manifest.rs:78-130`; `tools/fetch_assets.py --cache DIR` | no audio packs |
| Preflight | `src/main.rs:65-163` | also requires every manifest file on disk (`missing_files`): after Step 2, `cargo run` needs `fetch_assets.py` |

---

## 3. Revised approach

Unchanged in shape from PLAN.md §2. Presentation reacts to sim messages and sim state. The only sim change is
`BulletTrace.hit`. Numbers live in `mix.ron` (sound) and `juice.ron` (screen/camera), one source each. The deltas:

- **Listener (A.6, revised):** `pub fn listener(gap: f32) -> SpatialListener` returns
  `SpatialListener { left_ear_offset: Vec3::X * gap / 2.0, right_ear_offset: Vec3::X * gap / -2.0 }`, commented
  `// rodio 0.22.2 Spatial swaps the ear gains (spatial.rs:57-60); mirrored ears restore left/right.`
  It is inserted by an observer `On<Add, OrbitCamera>` from `GameAudioPlugin`. The camera is spawned in `Startup`, and
  observers registered at build time fire for it. Gated by G-A7.
- **Voice budget (A.1)** gains `SoundStats` (R4). It is written by `enforce_voice_budget` after trimming:
  `spawned[class] += new serials seen this frame`, `peak_alive[class] = max(peak, alive)`.
- **Pause (A.7):** `sync_loop_pause` per frame (R5), replacing `OnEnter/OnExit(Paused)`.
- **Loudness (R2):** `impacts.volume: 0.4`. The stinger stays at 0.7 (non-spatial path, no sum).
- **Siren (A.9):** `siren.height` in `mix.ron` (R7). Re-volume on `resource_changed::<GlobalVolume>` (R6).
- Everything else (Synth asset with endless `Once` loops, event → sound map, park weight, siren picking with
  `repick_seconds` hysteresis, `StarsRaised`/`PlayerHurt`, trauma rows, vignette, damage arc, star pulse,
  `reduce_camera_motion`, Kenney packs via manifest) stands as in PLAN.md §2 A.2-A.5, A.8, B.1-B.6.

Why not `bevy_seedling` (GDD §8 fallback): the voice cap is about 40 lines, and seedling would replace the whole audio
stack. The rodio bugs (R1, R2) are workable with one mirrored listener and one volume. Revisit only if the owner run finds
panning or mixing unfixable.

---

## 4. Revised steps (complete)

Order = dependency order. Builds use `-j 4`.

**1. Sim: `BulletTrace.hit`**, `crates/gta_sim/src/combat/hitscan.rs`.
Add `#[derive(Reflect, Clone, Copy, Debug, PartialEq, Eq)] pub enum TraceHit { Nothing, World, Body }` and
`pub hit: TraceHit` on `BulletTrace` (doc: "what the pellet stopped on; `Body` = a character that is not `Dead`").
In `fire_weapons`, the miss branch (`:235`) writes `Nothing`. In the hit branch, compute `(target, headshot)` before
writing the trace, then `hit = if targets.contains(target) { Body } else { World }` (`targets: Query<&mut Health,
Without<Dead>>`, already a param). Register `TraceHit` next to `BulletTrace` (`combat/mod.rs:63`) and re-export it from
`gta_sim::combat`. Update `crates/gta_sim/tests/new_city.rs:323` (`hit: TraceHit::Nothing`).
Check: `cargo test -p gta_sim -j 4` green. In `crates/gta_sim/tests/shooting.rs`, the existing wall-blocks test asserts
`hit == World` and the dummy-hit test asserts `hit == Body` (values from the fixtures). Flip: always write `World` →
the dummy assertion is RED.

**2. Manifest: three audio packs.** Append the three records from `scratch/kenney/manifest_snippet.ron` verbatim to
`assets/third_party/manifest.ron`. In `crates/gta_sim/tests/asset_manifest.rs` `shipped_manifest_is_valid`: add
`"impact-sounds"`, `"interface-sounds"`, `"music-jingles"` to the name set. Exclude them from the "4 files, no rig" loop.
Assert impact-sounds 26, interface-sounds 3, music-jingles 2 files, `rig.is_none()`, `license == CC0`.
Install: `python tools/fetch_assets.py --cache maw/tasks/in_progress/TASK-014/scratch/kenney` (zip basenames match the
URLs, sha256 pinned).
Check: `python tools/fetch_assets.py --check`; `cargo test -p gta_sim --test asset_manifest -j 4` green (G-M). Flip:
drop one impact file line → count RED; restore.

**3. `mix.ron` + `MixConfig`.** Move `MixConfig` and validation to new `src/audio/config.rs` (`mod.rs` keeps wiring).
`assets/audio/mix.ron`, every number with a unit comment:
```ron
(
    listener_ear_gap: 0.3,        // m between the camera listener's ears
    min_gain: 0.01,               // spatial one-shots quieter than this at the listener are not spawned
    voices: (shot: 12, impact: 12, ui: 4, stinger: 1),   // alive one-shots per class; the oldest is stolen
    shot: (
        pitch_variants: [0.95, 1.0, 1.05],               // playback speed, cycled by attack id (GDD §8: ±5%)
        npc_ref_distance: 8.0,                           // m of full volume around an NPC muzzle, then (ref/d)^2
        pistol:  (seconds: 0.18, decay: 22.0, volume: 0.5,  body_hz: 140.0, body_decay: 30.0, body_level: 0.6),
        smg:     (seconds: 0.10, decay: 35.0, volume: 0.35, body_hz: 170.0, body_decay: 45.0, body_level: 0.4),
        shotgun: (seconds: 0.35, decay: 12.0, volume: 0.7,  body_hz: 90.0,  body_decay: 18.0, body_level: 0.8),
    ),
    // 0.4: rodio 0.22.2 downmixes stereo files as L+R on spatial emitters (up to 2x); all impact files are stereo.
    impacts: (volume: 0.4, ref_distance: 6.0,
        bullet_body: [...5 impactSoft_medium...], bullet_world: [...5 impactGeneric_light...],
        punch: [...5 impactPunch_medium...], heavy: [...5 impactPunch_heavy...], death: [...5 impactSoft_heavy...]),
    interface: (volume: 0.6, press: "third_party/interface-sounds/click_001.ogg",
                pause: "third_party/interface-sounds/toggle_001.ogg"),
    stinger: (volume: 0.7, wanted: "third_party/music-jingles/jingles_HIT00.ogg"),
    ambience: (city_volume: 0.25, park_volume: 0.35, city_duck_in_park: 0.5, park_fade: 25.0,
               city_lowpass_hz: 300.0,
               birds: (chirps_per_s: 1.5, lo_hz: 2500.0, hi_hz: 5000.0, chirp_seconds: 0.08)),
    siren: (volume: 0.6, ref_distance: 15.0, max_emitters: 2, audible: 150.0, repick_seconds: 1.0,
            height: 1.0,          // m above the cop's body centre
            lo_hz: 800.0, hi_hz: 1700.0, period: 4.9),
)
```
(The pools are written out in full in the file, five paths each: `third_party/impact-sounds/<file>.ogg`.)
`MixConfig::validate()`: positive finite numbers; `pitch_variants` non-empty, each in (0.5, 2]; volumes in (0, 1];
`lo_hz < hi_hz`; `max_emitters ≥ 1`; `voices.* ≥ 1`; `city_duck_in_park`, `min_gain` in [0, 1]; `siren.height`
finite ≥ 0. `sound_paths()` iterates pools + interface + stinger. `check_sounds(&ThirdPartyManifest) -> Result<(), Vec<String>>`
gives distinct errors: `"… is not listed in third_party/manifest.ron"`, `"… must be an .ogg file"`, `"… pool is empty"`.
`src/main.rs` `preflight` pushes `check_sounds` errors into `unlisted`, the same shape as fonts.
Check: G-A1 (Step 13).

**4. `Synth` asset**, new `src/audio/synth.rs` (replaces `ShotSound`/`NoiseBurstDecoder`).
`#[derive(Asset, TypePath)] pub enum Synth { Shot(ShotSynth), Siren(SirenSynth), City(CitySynth), Birds(BirdSynth) }`,
`impl Decodable for Synth { type Decoder = SynthDecoder; }`, mono, `SAMPLE_RATE = 44_100` (law const, moved from
`mod.rs`), `current_span_len() = None`. Shot: noise·`exp(-decay t)` + `body_level·sin(2π body_hz t)·exp(-body_decay t)`,
scaled into [-1, 1], finite (`total_duration = Some`). Siren/City/Birds are endless (`total_duration = None`). Siren
phase accumulator `phase += 2π f(t)/SR` wrapped at 2π, `f(t) = lo + (hi-lo)(0.5-0.5cos(2πt/period))` with `t` wrapped
at `period`. City: white noise through a one-pole low-pass at `city_lowpass_hz`. Birds: a xorshift chirp trigger with
probability `chirps_per_s/SR` per sample, a `hi → lo` sweep over `chirp_seconds` under a raised-cosine window.
`pub fn is_endless(&self) -> bool`.
Check: G-A5.

**5. Cues (one-shots)**, new `src/audio/cues.rs`.
- `#[derive(Component, Reflect)] #[reflect(Component)] pub struct Sound { pub class: SoundClass, pub serial: u64 }`,
  `#[derive(Reflect, Clone, Copy, PartialEq, Eq, Debug)] pub enum SoundClass { Shot, Impact, Ui, Stinger, Siren, Ambience }`
  with `fn index(self) -> usize`.
- `#[derive(Resource, Reflect, Default)] #[reflect(Resource)] pub struct SoundStats { pub spawned: [u64; 6], pub peak_alive: [u32; 6] }`.
  Registered for BRP.
- `SoundBank` resource (built in `Startup`): `Handle<Synth>` per weapon, siren, city, birds; pool `Vec<Handle<AudioSource>>`
  via `AssetServer::load`; interface and stinger handles; a round-robin counter per pool; `next_serial`.
- Pure fns: `spatial_gain(distance, ref) = (ref/d)².min(1)` (`d ≤ 0` → 1); `spatial(ref) -> PlaybackSettings` =
  `PlaybackSettings::DESPAWN.with_spatial(true).with_spatial_scale(SpatialScale::new(1.0 / ref))`;
  `pub fn listener(gap) -> SpatialListener` (mirrored, §3).
- `Update`, set `SoundCues`: `play_shots` (player → non-spatial; NPC → spatial at `muzzle` via
  `Transform::from_translation`; `with_speed(pitch_variants[attack as usize % n])`; weapon volume), `play_impacts`
  (`BulletTrace.hit` World/Body with at most one per kind per shooter per frame, `MeleeHit` punch/heavy at `point`,
  `DamageDealt.killed` death at `point`), `play_stinger` (`StarsRaised`), `play_ui_press`
  (`Query<&Interaction, (Changed<Interaction>, With<MenuAction>)>`, `Pressed` only; `MenuAction` re-exported
  `pub(crate)` from `src/menu/mod.rs`). `play_pause_sound` on `OnEnter(GameState::Paused)` (a one-shot, so the R5 issue
  does not apply). The cull uses the single `SpatialListener`'s `GlobalTransform` and `min_gain`; with no listener
  nothing is culled.
- `enforce_voice_budget` (`Update`, `.after(SoundCues)`; the auto sync point applies spawns first): per one-shot class,
  sort alive `(serial, entity)`, `try_despawn` the oldest `count - cap`, then update `SoundStats` (spawned = serials above
  last frame's max serial of the class; `peak_alive` = max(peak, alive after trim)). Every class, loops included, is counted
  in `spawned`.
- Observer `On<Add, OrbitCamera>` → `insert(listener(mix.listener_ear_gap))`.
Check: G-A3, G-A4, G-A7.

**6. Loops**, new `src/audio/loops.rs`.
- `Startup`: the two ambience entities `(Sound{Ambience}, AudioPlayer(city|birds), PlaybackSettings::ONCE
  .with_volume(Volume::Linear(0.0)))`, session-lived (not `CityScoped`).
- Pure fns: `distance_to_polygon(p, poly)` (0 if `contains_convex(poly, p, 0.0)`, else min `dist_point_segment` over
  edges), `park_weight(distance, fade) = max(0, 1 - d/fade)`, `ambience_gains(park, cfg) -> (city, park)` =
  `(city_volume·(1 - city_duck_in_park·park), park_volume·park)`, `pick_sirens(listener, cops, max, audible)`
  (nearest first, ties by `Entity::to_bits`, beyond `audible` dropped).
- `update_ambience` (`Update`): park weight from the player's xz position against `City.0.blocks` with `is_park` (0 without
  `City`); gains 0 unless `Playing | Wasted | Busted`. It writes `AmbienceMix` (reflected resource `{city, park}`) and
  `sink.set_volume(Volume::Linear(gain) * global.volume)` on the two ambience `AudioSink`s.
- `update_sirens` (`Update`, run in `Playing | Wasted | Busted`; `Local<f32>` timer on `Time<Real>`): at `stars == 0`
  despawn all sirens. Otherwise every `repick_seconds`, `pick_sirens` over `PoliceUnit` not in `Dead | Leave`, diff with
  `Query<(Entity, &ChildOf), With<SirenEmitter>>`, `try_despawn` the dropped ones, and spawn
  `(SirenEmitter, Sound{Siren}, ChildOf(cop), Transform::from_xyz(0, siren.height, 0), AudioPlayer(siren),
  PlaybackSettings::ONCE.with_spatial(true).with_spatial_scale(SpatialScale::new(1/ref)).with_volume(Volume::Linear(siren.volume)))`.
  Outside those states, a separate `despawn_sirens` on `OnExit(Playing)` into `Paused` is **not** used: sirens stay and
  are paused by `sync_loop_pause`. `NEW_CITY` removes them with their `CityScoped` cops (`Children` is `linked_spawn`).
- `revolume_sirens` `run_if(resource_changed::<GlobalVolume>)`: `SpatialAudioSink::set_volume(siren.volume × global)`
  (sirens are `SpatialAudioSink`, not `AudioSink`).
- `sync_loop_pause` (`Update`): for `Sound` of class `Ambience | Siren` with `AudioSink` or `SpatialAudioSink`, `pause()`
  when `GameState::Paused` and `!is_paused()`, `play()` otherwise when `is_paused()`.
Check: G-A5, G-A6.

**7. Plugin wiring.** `src/audio/mod.rs`: `GameAudioPlugin` = `SynthSourcePlugin` (`add_audio_source::<Synth>()`, needs
bevy's `AudioPlugin` from `DefaultPlugins`) + `SoundCuesPlugin` + `SoundLoopsPlugin`. `src/main.rs:16,281`: replace
`ShotAudioPlugin`. The test harness uses the two inner plugins plus `init_asset::<Synth>()`, `init_asset::<AudioSource>()`,
`init_resource::<GlobalVolume>()` (R3), so no device is opened.
Check: `cargo build -j 4`; a manual `cargo run --release -- --seed 1` where shots, a siren and ambience are heard (owner run).

**8. Juice config**, `src/juice/config.rs` + `assets/juice/juice.ron`. `ShakeConfig` gains `shot_trauma: 0.1`,
`hurt_trauma: 0.2`, `death_trauma: 0.4` (each in (0, 1]) and `death_radius: 12.0` (m, > 0). `JuiceConfig` gains
`camera_motion_reduced_scale: 0.3` ([0, 1]), `vignette: (color: (0.75, 0.0, 0.0), per_hurt: 0.35, max: 0.6,
decay_per_s: 0.8, radius: 0.9, smoothness: 3.0)` (`max` in (0, 1]), `damage_arc: (seconds: 0.6, radius_px: 150.0,
thickness_px: 6.0, color: (0.9, 0.1, 0.1))`, `star_pulse: (scale: 1.3, seconds: 0.35)` (scale > 1). Validation uses the
file's helpers. Update the `ShakeConfig` literal in `shake.rs` tests (`:81-91`).
Check: `shipped_juice_validates` green. One sabotage per new block, strictly on the failing side: `vignette.max = 1.5`,
`star_pulse.scale = 0.9`, `death_radius = -1.0`, each asserting a distinct keyword.

**9. Trauma, hurt, stars-raised**, `src/juice/shake.rs` + `src/juice/mod.rs`. `CameraShake` gets
`#[derive(Reflect)] #[reflect(Resource)]` and is registered (QA reads `trauma`). `add_melee_trauma` becomes `add_trauma`:
player `ShotFired` → `shot_trauma`; `MeleeHit` the player lands or takes → `melee_trauma`; `PlayerHurt` → `hurt_trauma`;
`DamageDealt.killed` with the target ≠ player and within `death_radius` of the player's `Transform` (target
`Transform` missing → skip) → `death_trauma`. Every row clamps at 1, and rows stack (R11).
`detect_player_hurt` (`Local<Option<(Entity, f32)>>` keyed by the player entity, fires when `current + armor` falls) and
`detect_stars_raised` (`Local<u8>`, fires when `WantedLevel.stars` rises) write the client messages `PlayerHurt{amount}`
and `StarsRaised{stars}`. They are registered by `JuicePlugin` and again by `SoundCuesPlugin` and `HudPlugin`
(`add_message` is idempotent). Chain: `(detect_player_hurt, detect_stars_raised, add_trauma, shake_camera).chain()`.
`kick_camera` scales the kick by `camera_motion_reduced_scale` when `settings.reduce_camera_motion`.
Check: G-J2.

**10. Vignette + damage arc.** New `src/juice/vignette.rs`: observer `On<Add, OrbitCamera>` inserts
`Vignette { intensity: 0.0, color, radius, smoothness, ..default() }`; `VignetteLevel` resource; `update_vignette` on
`Time<Real>` rises by `per_hurt` on `PlayerHurt` (clamped at `max`), decays at `decay_per_s`, writes
`intensity = if no_flashes { 0 } else { level }`. New `src/juice/damage_arc.rs`: reflected
`DamageArc { shooter: Entity, angle: f32, left: f32 }`; pure `arc_angle(yaw, player, attacker) -> Option<f32>`
(`None` under 1e-3 m xz); `spawn_or_refresh_arcs` from `DamageDealt` with the player as target and another shooter (one
arc per shooter, refreshed); `update_arcs` recomputes the angle from `OrbitCamera.yaw` while the shooter exists and
freezes it after; `UiTransform.rotation = Rot2::radians(angle)`; alpha = `left/seconds` on `BorderColor.top`;
`try_despawn` at 0. Arcs carry `CityScoped`. Register `Vignette` and `DamageArc`.
Check: G-J2 vignette rows, G-J3.

**11. Star pulse**, `src/hud/stars.rs`. `spawn_stars` (`:53-92`) adds `StarPulse { left: 0.0 }` to the row.
`pulse_stars` is added to `HudPlugin`'s `Update` tuple (`src/hud/mod.rs:31-40`). It resets `left = seconds` on
`StarsRaised`, ticks on `Time<Real>`, and writes `UiTransform.scale = Vec2::splat(star_pulse_scale(seconds - left, cfg))`.
`star_pulse_scale(elapsed, cfg) = 1 + (scale - 1)(1 - EaseFunction::BackOut.sample_clamped(elapsed/seconds))`.
Check: G-J3 endpoints.

**12. Setting "Меньше движения камеры".** `GameSettings.reduce_camera_motion: bool` (doc: "Recoil kick scaled by
`juice.ron` `camera_motion_reduced_scale`"), default false. `sanitize` keeps it via `..s.clone()`.
`SettingKey::ReduceCameraMotion` (`widgets.rs:13-19`). `screens.rs`: text (`:116-117`), row after `ReduceShake`
(`:145`), toggle (`:297-298`). `menu_config.rs`: `pub reduce_camera_motion: String` + the non-empty check (`:71-72`).
`assets/ui/strings.ron:46`: `reduce_camera_motion: "Меньше движения камеры"`.
Check: existing settings tests green; `python tools/qa/font_check.py` passes.

**13. Headless gates** (`cargo test -p gta_like --bin gta_like -j 4`). Every presentation gate is run **3 times** by the
stage that reports it (TASK-022 lesson).

`src/audio/gate.rs` (`#[cfg(test)] mod gate;`). Harness `audio_app()`: `MinimalPlugins + TransformPlugin +
AssetPlugin::default() + StatesPlugin`, `TimeUpdateStrategy::FixedTimesteps(1)`, `init_asset::<Synth>()`,
`init_asset::<AudioSource>()`, `init_resource::<GlobalVolume>()`, `compose_sim(TestArea)`, shipped `MixConfig`,
`GameSettings::default()`, `SoundCuesPlugin + SoundLoopsPlugin`, `finish(); cleanup();`, then updates until `Player`
exists. Message fields use spawned stand-in entities with a `Transform` (R12).
- `mix_sounds_are_manifest_oggs` (G-A1, correctness): shipped mix + manifest → Ok. Mutated clones: an unlisted path →
  `"not listed"`; `".wav"` → `"must be an .ogg"`; an empty `death` pool → `"pool is empty"`. Each sabotage yields a
  different error.
- `mix_oggs_decode` (G-A2, correctness): for each `sound_paths()`, read `assets/<path>` and run `decodes(bytes)`:
  `catch_unwind(AssertUnwindSafe(|| AudioSource{bytes}.decoder()))`, then `sample_rate() > 0`, `channels() ∈ {1,2}`,
  `take(1024).count() > 0`. `eprintln!("SKIP … run tools/fetch_assets.py")` when a pack directory is absent (precedent
  `local_assets_match_manifest`). Flip: the first 100 bytes of a real file → `Err`.
- `voice_budget_holds` (G-A3, correctness): over 4 updates, 30 NPC SMG `ShotFired` + 30 `BulletTrace{hit: World}` per
  update from 30 distinct stand-in shooters. After every update: alive `Shot` ≤ `voices.shot`, `Impact` ≤
  `voices.impact`, survivors = the top-`cap` serials, `SoundStats.spawned[Shot]` = 30·k, `peak_alive[Shot]` ≤ cap. Flip:
  drop `enforce_voice_budget` → RED (120 alive).
- `emitters_are_placed` (G-A4, correctness): NPC shot at muzzle `M` → one `Sound{Shot}`, `spatial`,
  `Transform.translation == M`, `spatial_scale == Some(SpatialScale::new(1/npc_ref_distance))`; player shot →
  `spatial == false`; `BulletTrace{hit: World, to: P}` → `Impact` at `P`; `MeleeHit{knockdown: true}` → a `heavy` pool
  handle. Flip: swap the player/NPC branch → RED.
- `loops_are_endless_once` (G-A5, correctness): each endless `Synth` has `total_duration() == None` and
  `take(441_000).count() == 441_000`; the two ambience entities have `mode == Once`. Flip: `LOOP` → RED.
- `pure_tables` (G-A6): `spatial_gain(4,8)=1`, `(8,8)=1`, `(16,8)=0.25`, `(24,8)=1/9`; CCW square
  `[(0,0),(10,0),(10,10),(0,10)]`: `(5,5)` → 0, `(15,5)` → 5, `(13,14)` → 5; `park_weight(0,25)=1`, `(12.5,25)=0.5`,
  `(30,25)=0`; `ambience_gains(1, cfg) = (city·(1-duck), park)`; `pick_sirens` with cops at 10/30/200 m: max 2 → [10, 30],
  max 1 → [10], all beyond `audible` → []; two cops at equal distance → the lower `to_bits` first in both input orders.
- **`listener_pans_right_to_right` (G-A7, correctness, new, R1).** Needs `[dev-dependencies] rodio = { version = "=0.22.2",
  default-features = false }` in the root `Cargo.toml` (already in `Cargo.lock` at 0.22.2; the lock only gains the
  edge, no new crate). Build `l = listener(0.3)`. Camera at the origin, yaw 0. Emitter at `(10,0,0)` (right, per
  `right = (cosθ,0,-sinθ)`), scale `1/8` applied to all three points as `play_queued_audio_system` does.
  `rodio::source::Spatial::new(SamplesBuffer::new(1ch, 44100, vec![0.5; 64]), emitter, left, right)` → the mean
  |channel 1| > the mean |channel 0|. Mirror row: emitter `(-10,0,0)` → channel 0 louder. Flip: `SpatialListener::new(0.3)`
  → both rows RED (this is also the upgrade tripwire). Before writing it, confirm that the `SamplesBuffer::new` signature
  and the `ChannelCount`/`SampleRate` NonZero types are exactly as in `rodio-0.22.2/src/buffer.rs`.

`src/juice/feedback_gate.rs` (`#[cfg(test)]`). Harness: the base above + `JuicePlugin` with shipped `JuiceConfig`,
`UiConfig`, `UiFonts{Handle::default()×2}` (as `damage_numbers_gate.rs:37-58`), `GameSettings`, and a stand-in
`(OrbitCamera{..}, Transform::default())` so both observers fire.
- `juice_never_writes_virtual_time` (G-J1, correctness): walk `CARGO_MANIFEST_DIR/src`, each `.rs` line (`lines()`,
  CRLF-safe), `normalise(line)` = drop whitespace, then drop every `'<ident>,`. `forbidden(norm)` matches
  `ResMut<Time<Virtual>>`, `ResMut<Time<Fixed>>`, `ResMut<Time>`, `resource_mut::<Time<Virtual>>`,
  `resource_mut::<Time<Fixed>>`, `set_relative_speed(`. The token literals are built with `concat!` so the gate file
  does not match itself. Report `file:line`. Self-test: `mut t: ResMut<Time<Virtual>>`,
  `pub t: ResMut<'w, Time<Virtual>>`, `w.resource_mut::<Time<Fixed>>()`, `t.set_relative_speed(0.5)` → each `Some`;
  `world.resource::<Time<Virtual>>()` and `mut s: ResMut<TimeUpdateStrategy>` → `None`. Flip: add
  `fn _p(_: ResMut<Time<Virtual>>) {}` to `src/juice/shake.rs` → RED; restore.
- `trauma_rows_and_time_untouched` (G-J2, correctness + time liveness): one case per row, trauma reset between cases.
  Player shot → +0.1; NPC shot → +0; melee with the player as attacker → +0.25; `DebugDamage{10}` → +0.2 and
  `Vignette.intensity > 0`; a killed stand-in target at `death_radius − 1` → +0.4, at `death_radius + 1` → +0. With
  `no_flashes`, another hurt leaves intensity 0. The recoil ratio with/without `reduce_camera_motion` is 0.3 ± 1e-4
  (the decay cancels: `Time<Real>` advances by exactly one timestep under `FixedTimesteps(1)`). Bounds per update:
  `before + row − decay_per_s·dt − 1e-4 ≤ after ≤ before + row + 1e-4`; zero rows assert no rise. After **every** update:
  `Time<Virtual>` `relative_speed() == 1.0`, `!is_paused()`, `Time<Fixed>.elapsed()` advanced by exactly one timestep.
  Flip: drop the `death_radius` check → the +1 m row is RED.
- `arc_angle_table` (G-J3): θ=0: `(0,0,-10)` → 0, `(10,0,0)` → +π/2. θ=90°: `(-10,0,0)` → 0, `(0,0,-10)` → +π/2,
  `(10,0,0)` → ±π. θ=180°: `(0,0,10)` → 0, `(-10,0,0)` → +π/2, `(10,0,0)` → −π/2. Coincident → `None`.
  `star_pulse_scale(0) == 1.3`, `(seconds) == 1.0`, `(10·seconds) == 1.0`, `min over [0, seconds] ∈ [0.96, 0.98]`
  (BackOut peak 1.10).

**14. QA scenario**, new `tools/qa/scenarios/t13.py` (`--out`, release, `--seed 1`, the t7/t11 structure; reuse t5
`screenshot/damage/log_errors/resource_value/game_state/wait_chunks`, t6 `rows/aim_at/dummies/teleport/camera_config/
weapon_pickups`, t7 `face_dummy/me`, t11 `set_heat/cops`). Caps (`voices`, `siren.max_emitters`) are read from `mix.ron`
with a regex; a failed read is `GATE BROKEN`. `panics(game)` = any `"panicked at"` in the log tail (R9), checked after each
phase along with `game_state` answering. The `No audio device found` warning is recorded in `summary.json`, if present.
1. `fetch_assets.py --check`; launch; golden hash; chunks.
2. Listener: exactly one `SpatialListener`, on the entity with `OrbitCamera`; its `right_ear_offset.x < 0` (the R1
   mirror is live).
3. Ambience: two `Sound{Ambience}` entities. Log `AmbienceMix` at spawn. Teleport to `CityLandmarks.park_center`, wait
   1 s: `AmbienceMix.park > 0.9` and `city < city_volume`.
4. Melee (unarmed): face the middle dummy, 3 clicks 0.3 s apart. `SoundStats.spawned[Impact]` delta ≥ 1,
   `peak_alive` ≤ caps, `CameraShake.trauma > 0` in some poll. Screenshot.
5. Guns: pick up a pistol, 6 shots at a dummy 0.35 s apart. `spawned[Shot]` delta == 6, `spawned[Impact]` delta ≥ 1.
   Screenshot mid-series. One shot into a building face → `spawned[Impact]` delta ≥ 1.
6. Hurt: `damage(game, 10)` → within 0.3 s the camera `Vignette.intensity > 0` and trauma > 0; screenshot. Set
   `GameSettings.no_flashes = true`, damage again → intensity ≤ 1e-4. Reset.
7. Arc: `world.write_message` `DamageDealt{shooter: right dummy, target: player, point: player, damage: 0, shot: 0,
   headshot: false, killed: false}`. Within 0.2 s a `DamageArc` exists whose `angle` matches Python
   `atan2(d·right, d·forward)` within 0.15 rad (a wiring check, R13) and is > 0 for the dummy on the right. Screenshot
   for the visual direction.
8. Wanted: raise armour, `set_heat` to star 2 → within 0.5 s `spawned[Stinger]` delta == 1, and the `UiTransform.scale.x`
   of `Wanted stars` > 1.05 in a poll within 0.3 s. Screenshot.
9. Sirens: wait ≤ 90 s for cops, then 1 ≤ alive `Siren` ≤ `max_emitters`, and every siren's `ChildOf` parent is a
   `PoliceUnit` not `Dead`/`Leave`. Log distances. `set_heat(0)` → within 3 s alive `Siren` == 0.
10. UI: `send_keys(["Escape"])` → `spawned[Ui]` delta ≥ 1 within 0.5 s; screenshot of the pause; Escape again.
11. Log clean (errors + panics), `shutdown`. `summary.json`: `SoundStats` series, per-class peaks, screenshots.
Hard pass/fail rests on the counts, `SoundStats`, parents, `AmbienceMix`, `Vignette.intensity`, `DamageArc.angle`, the
scale and the listener mirror. Screenshots are ≥ 0.15 s apart. Sound itself is not checked.

**15. QA_REPORT owner checklist** (AC "игра звучит и бьёт"):
- [ ] **Pan direction:** a cop or NPC gunfire on the right sounds on the right (the rodio 0.22.2 mirror fix, R1).
- [ ] Shots per weapon: body, pitch variation; NPC shots fade with distance.
- [ ] Body/wall/punch/knockdown/kill impacts are readable and do not clip (R2: spatial stereo is summed).
- [ ] Wanted stinger `jingles_HIT00` + star pulse (alternatives `HIT01..16`, a `mix.ron` + manifest line).
- [ ] Sirens: the wail follows the cops, is not grating with 2, and pauses with the game.
- [ ] City and park ambience, the crossfade into the central park.
- [ ] Shake on shot/hurt/nearby death, stacked rows on a punch taken (0.45) and a melee kill nearby (R11), red vignette,
      damage arc: "бьёт", no nausea. The toggles "Уменьшить тряску", "Меньше движения камеры", "Без вспышек" work.
- [ ] Menu click and pause sound.
- [ ] Content gap: the player "вскрик" and the death "глухой sting" (no CC0 voice in Kenney; manual import is the owner's call).

**16. Final checks:** `cargo build -j 4`, `cargo clippy -j 4 -- -D warnings` (and `--features dev`),
`cargo test -p gta_sim -j 4`, `cargo test -p citygen -j 4`, `cargo test -p gta_like --bin gta_like -j 4` (3×),
`cargo tree -p gta_sim -e normal -i bevy_render` empty (no sim dependency changed), `python tools/qa/tree_check.py`,
`python tools/fetch_assets.py --check`, `python tools/qa/font_check.py`, `python tools/qa/scenarios/t13.py --out target/qa/t13`.
Every touched file stays < 750 lines.

---

## 5. Risk areas

- **rodio 0.22.2 spatial bugs (R1, R2).** The mirrored listener depends on the pinned rodio. G-A7 goes RED when rodio is
  fixed, and then the swap is removed. **PCTX proposal (for the stage allowed to write it):** bevy-ecs risk lesson,
  "bevy_audio 0.19.1 / rodio 0.22.2 `Spatial` swaps the ear gains and sums stereo as L+R; the listener is mirrored in
  `audio::listener`; trigger `SpatialListener`." The planner's `PlaybackMode::Loop` lesson (already in
  `PCTX_PROPOSALS.md`) stands.
- **New dev-dependency** (`rodio =0.22.2`, default features off): no new crate in the graph, but `Cargo.lock` gains one
  edge, a small departure from PLAN.md's "no Cargo change". If the orchestrator rejects it, fall back to the mirrored
  listener + checklist item 1 only, and record the ungated sign risk in QA_REPORT.
- **No audio device on a QA host:** nothing plays, one-shots never drain, the cap bounds them (G-A3). `SoundStats` still
  counts spawns, so QA passes silently. The QA stage records the warning.
- **Kenney decode:** G-A2 decodes every referenced file headless. If a file fails, the bevy feature `symphonia-vorbis`
  is the fallback (not planned).
- **Loop memory:** endless decoders use `Once` (G-A5). A future finite Kenney loop may use `LOOP` (memory = clip size).
- **`set_volume` drops the global factor:** loops multiply `GlobalVolume` themselves (ambience per frame, sirens on
  `resource_changed::<GlobalVolume>`).
- **Listener on the camera:** pan follows the camera yaw, distance +3.8 m. `ref_distance` ≥ 6 m keeps the player's own
  impacts at full volume.
- **Siren churn:** `repick_seconds` hysteresis. A dead or despawned cop takes its child siren with it (`linked_spawn`);
  our despawns use `try_despawn`.
- **`BulletTrace.hit`:** a sim API change; the constructors are updated in Step 1 (`grep "BulletTrace {"`).
- **`PlayerHurt` from the pooled health:** regen and respawn heal raise it (no event); `NEW_CITY` swaps the entity
  (keyed). Armour absorption counts as hurt.
- **Stacked trauma** (R11): owner feel.
- **Frame budget:** the park scan is O(park polygons) per frame; siren picking O(cops)/s; about 150 one-shot
  spawns/s at worst under full SMG fire, capped and culled. Not gated.
- **BRP registration:** `Vignette`, `CameraShake`, `Sound`, `SoundStats`, `DamageArc`, `AmbienceMix` are registered
  explicitly. `SpatialListener` and `PlaybackSettings` are `Reflect` + `reflect(Component)` in bevy_audio 0.19.1.

## 6. Open questions

None new. Q1-Q4 are resolved in `TASK_FINAL.md`, and this revision keeps their answers. The R1 fix and the R2 volume
are defect corrections, not design choices.

---
Evidence (read-only, cited in place): `rodio-0.22.2/src/source/{spatial.rs,channel_volume.rs,repeat.rs}`,
`rodio-0.22.2/src/player.rs:345-353`, `bevy_audio-0.19.1/src/{lib.rs,audio.rs,audio_output.rs,sinks.rs,audio_source.rs}`,
`bevy_time-0.19.1/src/lib.rs:146-187`, `bevy_post_process-0.19.1/src/effect_stack/vignette.rs`,
`bevy_ui-0.19.1/src/{ui_transform.rs,ui_node.rs}`, `bevy-settings-0.19.1/src/lib.rs:495-535`; upstream fix
https://raw.githubusercontent.com/RustAudio/rodio/master/src/source/spatial.rs.

children: 0 launched / 0 reported.
