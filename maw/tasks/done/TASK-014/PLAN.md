# PLAN — TASK-014 (GDD T13): sound and juice

Planner: claude opus (high). Branch `feature/t13-audio-juice` (HEAD `e279b60`). Every Bevy symbol below was checked in
`~/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/` (bevy_audio 0.19.1, rodio 0.22.2, bevy_post_process 0.19.1,
bevy_ui 0.19.1, bevy_math 0.19.1, bevy_app 0.19.1, bevy_ecs 0.19.1, bevy-settings 0.19.1). **No new crate, no
`Cargo.toml` / `Cargo.lock` change**: the client builds `bevy` with default features (`Cargo.toml:37`), and the
`default` feature set contains `audio = ["bevy_audio", "vorbis"]` (`bevy-0.19.1/Cargo.toml:2645-2648, 2742-2746`), so
`.ogg` loads through `AudioLoader` (`bevy_audio-0.19.1/src/audio_source.rs:37-58`).

Cost of error: **mixed**.
- Silent class (real gates): unbounded sound entities (no audio device, SMG from 12 cops), unbounded memory of a
  procedural loop (see A.3), a Kenney file that rodio cannot decode (the decoder `.unwrap()`s at play time,
  `audio_source.rs:88-96`: the game panics on the first impact), a manifest/mix path that is not fetched (silent
  missing sound), any presentation system writing `Time<Virtual>` (GDD §8, test plan), a mirrored damage-direction
  arc (sign error the owner can misread in a fight), trauma rows that never fire.
- Owner class (mechanism + QA evidence + owner checklist): how shots, impacts, sirens, ambience and the stinger
  sound; how shake, vignette, arc and star pulse feel; whether the chosen Kenney files fit.

---

## 1. Understanding (what exists today)

`PREMISE_CHALLENGE.md` holds: T12 settings, the T6 shot stub and T7 melee trauma exist; seven of ten T13 items do not.

| Area | Where | State |
|---|---|---|
| Shot sound | `src/audio/mod.rs:1-179` | `ShotAudioPlugin`: one `ShotSound` asset per weapon (`Decodable`, white noise × exp envelope, `:71-133`); `play_shots` (`:163-179`) spawns `AudioPlayer<ShotSound>` + `PlaybackSettings::DESPAWN` for **every** `ShotFired`, player or NPC, non-spatial, no pitch variation, no cap. |
| Mix data | `assets/audio/mix.ron` (7 lines), `MixConfig` `src/audio/mod.rs:15-69` | only `shot.{pistol,smg,shotgun}.{seconds,decay,volume}` |
| Juice data | `assets/juice/juice.ron`, `src/juice/config.rs:63-248` | recoil, hit-stop, `shake` (melee only), flash, tracer, damage numbers |
| Trauma | `src/juice/shake.rs:17-57` | `add_melee_trauma` (player lands/takes `MeleeHit`), `shake_camera` on `Time<Real>`, `reduce_shake` → `reduced_scale`; rotation applied in `camera/mod.rs:452-456` after the aim ray is written. Tests `:78-141` build `ShakeConfig` literally. |
| Recoil | `src/juice/mod.rs:40-54` `kick_camera` | player shots only, `Time<Real>` |
| Hit-stop gate | `src/visuals/character_gate.rs:607-686` | asserts `Time<Virtual>` speed 1 / unpaused / one fixed step per update around a melee hit (HitStopPlugin only) |
| Settings | `src/settings/mod.rs:15-40` `GameSettings { mouse_sensitivity, volume, invert_y, reduce_shake, no_flashes }`; volume → `GlobalVolume` `:81-84` | "уменьшить движение камеры" (GDD §8 line 384) missing |
| Settings UI | `src/menu/widgets.rs:13-19` `SettingKey`, `:22-31` `MenuAction`; `src/menu/screens.rs:104-118` (texts), `:141-147` (rows), `:293-303` (toggle); `src/menu/menu_config.rs:20-24,70-72`; `assets/ui/strings.ron:372` | 5 rows |
| Stars HUD | `src/hud/stars.rs:138-247` | row + blink; no pulse |
| Muzzle flash | `src/vfx/mod.rs:216-247` | `no_flashes` suppresses it |
| Sim messages | `crates/gta_sim/src/combat/hitscan.rs:56-89` `ShotFired{shooter,weapon,muzzle,attack}`, `BulletTrace{shooter,from,to}`, `DamageDealt{shooter,shot,target,point,damage,headshot,killed}`; `combat/melee.rs:282-291` `MeleeHit{attacker,target,point,knockdown,attack}`; `player/mod.rs:16-21` `DebugDamage` | `BulletTrace` does not say what the pellet stopped on (`hitscan.rs:231-248`) |
| Wanted | `crates/gta_sim/src/wanted/mod.rs:157-169` `WantedLevel{heat,stars,..}` (Reflect resource) | |
| Police | `crates/gta_sim/src/police/mod.rs:287-321` `PoliceUnit{state: CopState,..}` | foot cops only (cars are T15) |
| Parks | `citygen` `Block.is_park`, `curb: Vec<Vec2>` (`crates/citygen/src/layout.rs:52-59`); `world::City(CityLayout)` resource (`crates/gta_sim/src/world/city.rs:25-27`), absent in TestArea | |
| Manifest | `assets/third_party/manifest.ron` (5 packs, no audio), `crates/gta_sim/src/config/manifest.rs`, `tools/fetch_assets.py` (schema mirrors `validate`, retries 2/4/8 s, `--cache DIR`) | test `crates/gta_sim/tests/asset_manifest.rs:80-138` asserts the exact pack name set and per-pack file counts |
| Preflight | `src/main.rs:65-163` | checks props, models, fonts are manifest files; mix has no file references yet |
| QA | `tools/qa/brp.py` (`Game`, `query`, `component_path`, `resource_path`), helpers in `t5.py` (`damage`, `log_errors`, `ERROR_WORDS=("font","asset","Failed to load")`), `t6.py` (`aim_at`, `dummies`, `teleport`, `rows`), `t7.py` (`face_dummy`, `wait_reaction`), `t11.py` (`set_heat`, `cops`) | no t13 |

Engine facts this plan depends on (all read in source):
- **Playback** (`bevy_audio-0.19.1/src/audio_output.rs`): `play_queued_audio_system<T>` runs in `PostUpdate` after
  `TransformSystems::Propagate`, only if an output device exists (`audio_output_available`); spatial sink emitter =
  `GlobalTransform.translation() * scale`, ears from the first `SpatialListener` (`left/right_ear_offset`, default gap 4)
  times scale; `DESPAWN` entities are despawned only when their sink drains. **Without a device no sink is ever made and
  `DESPAWN` entities live forever** (today's shot stub leaks on such a host).
- `PlaybackSettings { mode, volume, speed, spatial, spatial_scale: Option<SpatialScale>, .. }` (`audio.rs:34-80`);
  `speed` changes pitch; `SpatialScale::new(f32)` (`audio.rs:218-221`). `SpatialListener::new(gap)` requires
  `Transform` (`audio.rs:171-200`). `AudioPlayer<T>` requires `PlaybackSettings` (`audio.rs:253-257`).
- **Attenuation law** (rodio `source/spatial.rs:55-64`): per ear `gain = min(1, 1/dist_sq)` of the *scaled* distance,
  so with `spatial_scale = 1/ref_m` a source is at full volume inside `ref_m` and falls as `(ref_m/d)²` beyond.
- **Loop buffering** (rodio `source/repeat.rs:9-17`): `PlaybackMode::Loop` → `repeat_infinite` → `Repeat` keeps a
  `Buffered` clone of the start, so **every played sample of an endless source stays in memory**.
- `AudioSinkPlayback::set_volume` replaces the sink volume (`sinks.rs:166-173`); `GlobalVolume` is multiplied in only
  at sink creation (`audio_output.rs`, `sink.set_volume(settings.volume * global_volume.volume)`). `pause()`/`play()`
  exist on the trait. `AudioSink`/`SpatialAudioSink` are **not** `Reflect` (BRP cannot see them).
- `Vignette { intensity, radius, smoothness, roundness, center, edge_compensation, color }`, Reflect + Component,
  camera component; intensity ≤ 1e-4 skips the pass (`bevy_post_process-0.19.1/src/effect_stack/vignette.rs`);
  added by `PostProcessPlugin` → `EffectStackPlugin` (default plugins, `bevy_internal-0.19.1/src/default_plugins.rs:60-61`),
  path `bevy::post_process::effect_stack::Vignette`.
- `UiTransform { translation, scale, rotation: Rot2 }` "rotate clockwise" (`bevy_ui-0.19.1/src/ui_transform.rs:130-137`);
  `Node` requires `UiTransform` (`ui_node.rs:472-485`); `Node.border_radius: BorderRadius` (`:738`), `BorderRadius::MAX`
  (`:2546`); `BorderColor { top, right, bottom, left }` (`:2256-2261`).
- `EaseFunction::BackOut` = `1 + 2.70158(t-1)³ + 1.70158(t-1)²` (`bevy_math-0.19.1/src/curve/easing.rs:639-642`),
  `impl Curve<f32>` with domain `[0,1]` (`:1350-1360`), `sample_clamped` (`curve/mod.rs:349`).
- `App::add_message` is idempotent (`bevy_app-0.19.1/src/sub_app.rs:390-399`).
- `Children` is `linked_spawn` (`bevy_ecs-0.19.1/src/hierarchy.rs:148`): a child siren despawns with its cop.
- bevy-settings applies a saved TOML **field by field** onto the default (`bevy-settings-0.19.1/src/lib.rs:513-533`):
  a new `GameSettings` field is backward compatible with the owner's existing file.

## 2. Approach

One idea: **presentation reacts to sim messages and sim state; nothing new in the sim except one field on
`BulletTrace`**. All numbers go to `mix.ron` (sound) and `juice.ron` (camera/screen feedback), one source each.

### A. Audio (`src/audio/`, data `assets/audio/mix.ron`)

A.1 **Sound classes and the voice budget.** Every sound entity we spawn carries a reflected marker
`Sound { class: SoundClass, serial: u64 }`, `SoundClass = Shot | Impact | Ui | Stinger | Siren | Ambience`.
This is what QA counts per class over BRP (orchestrator note 2: the T6 stub already satisfies a naive
`AudioPlayer` count; also `AudioPlayer<Synth>` and `AudioPlayer<AudioSource>` are different component types).
One-shot classes (`Shot`, `Impact`, `Ui`, `Stinger`) have a cap in `mix.ron voices`; a system ordered after all
spawners in `Update` despawns the **oldest** (smallest `serial`) of a class above its cap, so extra entities never
reach `PostUpdate` playback. This is the standard "discard oldest instance" voice limit (Wwise playback limiting:
https://www.audiokinetic.com/en/courses/wwise251/?id=Lesson3_Playback_Limiting_and_Kill_Voice/). It also bounds the
no-device leak: stealing keeps the newest sounds alive. Chosen over `bevy_seedling` (GDD §8/§10.1 fallback) because
the cap is ~40 lines and adds no crate. Spatial one-shots whose `spatial_gain` at the listener is below
`mix.min_gain` are not spawned at all (NPC SMG fire across the city).

A.2 **Procedural sounds = one asset `Synth`** (`Decodable`, one `add_audio_source::<Synth>()`):
`Synth::Shot(ShotSynth)` (the T6 noise burst plus a decaying low "body" sine: `body_hz`, `body_decay`, `body_level`
per weapon), `Synth::Siren(SirenSynth)` (wail: frequency `lo + (hi-lo)·(0.5-0.5·cos(2πt/period))`, phase accumulator;
defaults 800-1700 Hz, period 4.9 s, measured wail in https://www.sciencedirect.com/science/article/abs/pii/S0003682X1200357X),
`Synth::City(CitySynth)` (white noise through a one-pole low-pass, `lowpass_hz`), `Synth::Birds(BirdSynth)` (random
down-chirp sine sweeps, `chirps_per_s`, `lo_hz`, `hi_hz`, `chirp_seconds`, own xorshift state). Shots stay procedural
(GDD §8, R14): the Impact Sounds archive listing has no gunshot (`scratch/kenney/ogginfo.txt` + zip listing).

A.3 **Loops never use `PlaybackMode::Loop`.** City, birds and siren decoders are endless (`next()` never returns
`None`, `total_duration() == None`) and are played with `PlaybackMode::Once`; the decoder state is O(1). `Loop` would
buffer every played sample (rodio `repeat.rs:9-17`): ~176 KB/s per mono loop, forever, silent. Gated (G-A5).

A.4 **Kenney CC0 files** through the manifest and `tools/fetch_assets.py` (verified online today, zips in
`scratch/kenney/`, sha256 of archives and files computed from those zips, the combined manifest validated by
`python tools/fetch_assets.py --validate-only scratch/kenney/manifest_with_audio.ron` → `valid`):

| Pack (page) | Archive sha256 | License.txt | Files used |
|---|---|---|---|
| `impact-sounds` (https://kenney.nl/assets/impact-sounds, "Impact Sounds (1.0)") | `029d734a…77f8` | CC0, both markers present | `impactSoft_medium_000..004` (bullet into a body), `impactGeneric_light_000..004` (bullet into world), `impactPunch_medium_000..004` (melee hit), `impactPunch_heavy_000..004` (knockdown hit), `impactSoft_heavy_000..004` (a kill: body falls) |
| `interface-sounds` ("Interface Sounds (1.0)") | `f2193d07…1232` | CC0 | `click_001` (menu button press), `toggle_001` (pause opens) |
| `music-jingles` (no version in License.txt; archive pinned by sha256; version string "1.0") | `b729ba57…97b8` | CC0 | `jingles_HIT00` (wanted stinger) |

All are Ogg Vorbis 44.1 kHz, mono or stereo (parsed headers, `scratch/kenney/ogginfo.txt`). Rodio's spatial source
downmixes to mono, so stereo files work as spatial emitters. Ready-to-paste manifest entries:
`scratch/kenney/manifest_snippet.ron`. The only gap is the "вскрик" (player scream) in GDD §8: Kenney has no voice
grunt, so player damage reuses the body-hit impact. That goes on the owner checklist (Freesound import by the owner, GDD §8).

A.5 **Event → sound map** (all from messages read in `Update`, like the existing readers):

| Event | Source | Sound | Spatial |
|---|---|---|---|
| `ShotFired` by the player | `Synth::Shot(weapon)` | pitch `pitch_variants[attack % n]` via `speed` | no |
| `ShotFired` by an NPC | same | same | yes, at `muzzle`, `ref = shot.npc_ref_distance` |
| `BulletTrace.hit == Body` | `impacts.bullet_body` pool | at `to` | yes |
| `BulletTrace.hit == World` | `impacts.bullet_world` pool | at `to` | yes |
| `MeleeHit` (`knockdown` false / true) | `impacts.punch` / `impacts.heavy` | at `point` | yes |
| `DamageDealt.killed` | `impacts.death` | at `point` | yes |
| wanted stars rise (`StarsRaised`) | `stinger.wanted` | — | no |
| menu button `Interaction::Pressed` / `OnEnter(GameState::Paused)` | `interface.press` / `interface.pause` | — | no |
| stars > 0, nearest active cops | `Synth::Siren` loop | child of the cop | yes |
| always (gain by state and park weight) | `Synth::City`, `Synth::Birds` loops | — | no |

Pools are chosen round-robin (a counter per pool). Per frame, at most one `bullet_body` and one `bullet_world` sound per
shooter: a shotgun blast is one impact, not ten.

A.6 **Listener** = the camera: observer `On<Add, OrbitCamera>` inserts `SpatialListener::new(mix.listener_ear_gap)`.
The camera's local +X is right, which matches the listener's ear axis. Attenuation is counted from the camera,
3.8 m behind the player; `ref_distance` values ≥ 4 m keep the player's own impacts at full volume.

A.7 **Loop gain and pause.** The ambience gains come from a pure function of the park weight (A.8) and the game
state (0 in `MainMenu`/`Loading`). Each frame they are written with `set_volume(gain × GlobalVolume)`, because
`set_volume` replaces the global factor. Siren sinks are re-volumed when `GameSettings` changes. Loop sinks
(`Ambience`, `Siren`) are `pause()`d `OnEnter(GameState::Paused)` and `play()`ed `OnExit(Paused)`. A reflected
resource `AmbienceMix { city: f32, park: f32 }` exposes the gains to QA.

A.8 **Parks.** `park_weight = 1` inside a park block's `curb` polygon, else `max(0, 1 - d/park_fade)` with `d` the
distance from the player (xz) to the nearest park polygon. Computed each frame by iterating `City.0.blocks` with
`is_park` (tens of polygons, a few µs). `0` when `City` is absent (TestArea, loading). City gain =
`city_volume × (1 - city_duck_in_park × park)`, park gain = `park_volume × park`.

A.9 **Sirens** (GDD §8 "пространственные сирены от полицейских юнитов и машин"; cars arrive in T15): while
`WantedLevel.stars > 0` and the state is `Playing|Wasted|Busted`, the nearest `siren.max_emitters` cops whose
`state ∉ {Dead, Leave}` within `siren.audible` of the listener each carry one child siren entity
(`ChildOf(cop)`, `Transform` at head height, spatial, `ref = siren.ref_distance`). The choice is re-made at most every
`siren.repick_seconds` of real time. Without this hysteresis two cops at equal distance would swap every frame and
restart the wail. With stars at 0, or in another state, all sirens despawn. The despawns use `try_despawn`, because a
cop can die first and its child siren goes with it (`linked_spawn`).

### B. Juice (`src/juice/`, `src/hud/stars.rs`, data `assets/juice/juice.ron`)

B.1 **Shared client messages** (registered by `JuicePlugin`, and again by the audio plugin, which is safe because
`add_message` is idempotent):
`StarsRaised { stars: u8 }` is written by `detect_stars_raised` when `WantedLevel.stars` rises above the previous
frame's value. It is keyed by nothing: resets to 0 at `NEW_CITY`/wasted/busted only lower it.
`PlayerHurt { amount: f32 }` is written by `detect_player_hurt` when the player's `health.current + health.armor`
falls, keyed by the player entity in a `Local<Option<(Entity, f32)>>`, so a new player after `NEW_CITY` or the respawn
heal never count. This catches every damage path, including `DebugDamage`, which QA drives deterministically.

B.2 **Trauma rows** (GDD §8 table): player `ShotFired` → `shake.shot_trauma` (0.1). A `MeleeHit` the player lands or
takes → `melee_trauma` (0.25, as today). `PlayerHurt` → `hurt_trauma` (0.2). `DamageDealt.killed` with the target
within `death_radius` of the player (and the target not the player) → `death_trauma` (0.4). All rows clamp at 1. SMG
check: +0.1 every 0.08 s against a decay of 1.2 × 0.08 = 0.096 is +0.004 net per shot, about 0.22 trauma after a 30-round
magazine, shake ≈ 0.05 × max. So the flat GDD value holds.

B.3 **Red vignette**: `On<Add, OrbitCamera>` inserts `Vignette { intensity: 0, color: vignette.color, radius, smoothness,
..default() }`. The vignette level rises by `vignette.per_hurt` on `PlayerHurt` (clamped at `vignette.max`) and decays at
`decay_per_s` of real time. With `no_flashes` it stays at 0. Rising and holding instead of flashing per hit keeps it
under the three-flashes-per-second guidance (WCAG 2.3.1, red flashes are the most provocative:
https://www.w3.org/WAI/WCAG20/Understanding/three-flashes-or-below-threshold).

B.4 **Damage direction arc** (GDD §7 "дуга 0.6 с"): for each `DamageDealt` whose target is the player and whose
shooter is not, one arc per shooter. A new hit from the same shooter refreshes its arc. The arc is a UI ring node
(`2·radius_px` square, centred, `border` only on top in `color`, `border_radius: BorderRadius::MAX`) rotated by
`UiTransform.rotation = Rot2::radians(arc_angle)`. Alpha fades over `damage_arc.seconds` of real time. The angle is
recomputed each frame while the shooter exists and frozen after it is gone. Reflected component
`DamageArc { shooter: Entity, angle: f32, left: f32 }` lets QA read it.
**Angle (pure fn, clockwise from screen-up):** with camera yaw θ, `forward = (-sinθ, 0, -cosθ)`,
`right = (cosθ, 0, -sinθ)` (GDD §3.2 convention), `d = attacker - player` (xz), `angle = atan2(d·right, d·forward)`.
In UI space y points down, so `Mat2::from(Rot2::radians(+π/2))` sends screen-up `(0,-R)` to `(R,0)` = right; positive
means clockwise, which is what the `UiTransform` doc says. Worked examples (the gate table):
1. θ = 0: attacker at `(0,0,-10)` → f = 10, r = 0 → **0** (top). At `(10,0,0)` → **+π/2** (right).
2. θ = +90°: forward `(-1,0,0)`, right `(0,0,-1)`. Attacker `(-10,0,0)` → **0**. `(0,0,-10)` → **+π/2**.
   `(10,0,0)` → f = -10, r = 0 → **±π** (bottom).
3. θ = 180°: forward `(0,0,1)`, right `(-1,0,0)`. `(0,0,10)` → **0**. `(-10,0,0)` → **+π/2**. `(10,0,0)` → **-π/2** (left).

B.5 **Star pulse** (GDD §8 "scale 1.3 → 1, ease-back"): on `StarsRaised` the star row's `UiTransform.scale` =
`1 + (star_pulse.scale - 1)·(1 - BackOut(t/seconds))` on real time. At t = 0 it is 1.3, at t ≥ `seconds` exactly 1.
BackOut overshoots, so the row dips about 3 % below 1 near the end. That is the intended "ease-back".

B.6 **Accessibility** (GDD §8 line 384): new `GameSettings.reduce_camera_motion` (default false) multiplies the recoil
kick by `juice.camera_motion_reduced_scale`. Recoil is the only camera motion in the game other than shake, which has
its own toggle. Xbox Accessibility Guideline 117 names camera movement as a motion-sickness setting
(https://learn.microsoft.com/en-us/gaming/accessibility/xbox-accessibility-guidelines/117). `no_flashes` also turns off
the red vignette. Existing toggles are unchanged.

### C. Gates (named class each; details in Steps)

- **G-M** (correctness, `-p gta_sim`): shipped third-party manifest, with the three audio packs, validates and has the
  exact pack set and file counts. (AC "манифест звуков валиден", part 1.)
- **G-A1** (correctness, `-p gta_like`): every sound path in `mix.ron` is a manifest file ending `.ogg`, and every pool
  is non-empty. Fixture sabotages give distinct errors. (AC part 2; also enforced at startup by `preflight`.)
- **G-A2** (correctness): every `.ogg` that `mix.ron` names decodes through `AudioSource::decoder()` and yields
  samples. It is skipped with a message when the packs are not fetched, the same rule as `local_assets_match_manifest`.
  This catches the panic-at-play class.
- **G-A3** (correctness): voice budget. Across a burst of NPC shots/impacts in the production cue plugin, the alive
  `Sound` count per class stays ≤ cap on every update, and the survivors are the newest serials.
- **G-A4** (correctness): emitter placement. An NPC shot → `spatial == true` at `muzzle`; a player shot →
  `spatial == false`; a wall trace → `Impact` at `to`; the `spatial_scale` equals `1/ref`.
- **G-A5** (correctness): loop kinds → `PlaybackMode::Once`, and the decoder is endless (`total_duration()` is `None`,
  `take(441_000).count() == 441_000`).
- **G-A6** (pure math): `spatial_gain`, `park_weight`, `distance_to_polygon`, `ambience_gains`, `pick_sirens` tables.
- **G-J1** (correctness, AC "juice never writes `Time<Virtual>`"): a source scan of all of `src/**/*.rs` for
  write-forms (`ResMut<Time`, `resource_mut::<Time`, `set_relative_speed`). A scanner self-test on in-memory lines
  proves each token is caught.
- **G-J2** (correctness + time liveness): trauma table in the production `JuicePlugin` over `compose_sim(TestArea)`,
  one case per row. Every update also asserts `Time<Virtual>` speed 1, not paused, and one fixed step per update.
  Vignette rows: hurt → intensity > 0; with `no_flashes` → 0. Recoil row: with `reduce_camera_motion` the kick is the
  reduced one.
- **G-J3** (pure math): `arc_angle` 3-yaw table (B.4), `star_pulse_scale` endpoints.
- **QA t13** (runtime liveness + evidence): see Step 14.
- **Declined** (owner's run): loudness balance, how shots and sirens sound, whether the pan direction sounds right,
  the shake amount, the vignette colour, the arc look. BRP cannot hear audio or see `AudioSink`.

## 3. Steps

Order = dependency order. Each step names its check.

**1. Sim: `BulletTrace.hit`** — `crates/gta_sim/src/combat/hitscan.rs`.
Add `#[derive(Reflect, Clone, Copy, Debug, PartialEq, Eq)] pub enum TraceHit { Nothing, World, Body }` and field
`pub hit: TraceHit` on `BulletTrace` (`:69-73`, doc: "what the pellet stopped on; `Body` = a live character (not
`Dead`)"). In `fire_weapons` (`:231-248`): the miss branch writes `Nothing`. In the hit branch, compute
`(target, headshot)` **before** writing the trace, then `hit = if targets.contains(target) { Body } else { World }`
(`targets: Query<&mut Health, Without<Dead>>`, which is already a parameter). A corpse therefore counts as `World`,
which is acceptable. Register `TraceHit` next to `BulletTrace` in `combat/mod.rs:64` (`register_type`). Update the
literal in `crates/gta_sim/tests/new_city.rs:323-327` (`hit: TraceHit::Nothing`). Re-export `TraceHit` from
`gta_sim::combat` (`combat/mod.rs:9`).
Check: `cargo test -p gta_sim -j 4` green; `crates/gta_sim/tests/shooting.rs` gets one assertion in its existing
wall-blocks test that the trace's `hit == World`, and in the dummy-hit test that `hit == Body`. The expected values come
from the fixture: the wall is `World`, the dummy is `Body`.

**2. Manifest: three audio packs** — `assets/third_party/manifest.ron`: append the three pack records from
`scratch/kenney/manifest_snippet.ron` (verbatim; archive paths keep the `Audio/` / `Audio/Hit jingles/` prefix, flat
`path`s). `crates/gta_sim/tests/asset_manifest.rs:80-104`: add `"impact-sounds"`, `"interface-sounds"`,
`"music-jingles"` to the name set. Exclude them from the "4 files, no rig" loop (`:95-104`), and assert their counts
explicitly: impact-sounds 26, interface-sounds 3, music-jingles 2, each `rig.is_none()` and `license == CC0`.
Install: `python tools/fetch_assets.py --cache maw/tasks/in_progress/TASK-014/scratch/kenney`. The zips there match the
manifest sha256, so there is no network, and kenney.nl TLS is flaky. `fetch_assets.py` itself needs no change: its
schema is generic. Check: `python tools/fetch_assets.py --check` → "third-party packs match the manifest";
`cargo test -p gta_sim --test asset_manifest` green (G-M; flip: drop one file line → RED on the count, restore).

**3. `mix.ron` + `MixConfig`** — move `MixConfig` and validation to new `src/audio/config.rs`
(`src/audio/mod.rs` keeps the plugin wiring). New `assets/audio/mix.ron`, every number commented with its unit:
```ron
(
    listener_ear_gap: 0.3,        // m between the camera listener's ears
    min_gain: 0.01,               // spatial one-shots quieter than this at the listener are not spawned
    voices: (shot: 12, impact: 12, ui: 4, stinger: 1),   // alive one-shots per class; the oldest is stolen
    shot: (
        pitch_variants: [0.95, 1.0, 1.05],               // playback speed, cycled by attack id (GDD §8: ±5%)
        npc_ref_distance: 8.0,                           // m of full volume around an NPC muzzle, then (ref/d)²
        pistol:  (seconds: 0.18, decay: 22.0, volume: 0.5,  body_hz: 140.0, body_decay: 30.0, body_level: 0.6),
        smg:     (seconds: 0.10, decay: 35.0, volume: 0.35, body_hz: 170.0, body_decay: 45.0, body_level: 0.4),
        shotgun: (seconds: 0.35, decay: 12.0, volume: 0.7,  body_hz: 90.0,  body_decay: 18.0, body_level: 0.8),
    ),
    impacts: (volume: 0.8, ref_distance: 6.0,
        bullet_body:  [5 × "third_party/impact-sounds/impactSoft_medium_00N.ogg"],
        bullet_world: [5 × impactGeneric_light], punch: [5 × impactPunch_medium],
        heavy: [5 × impactPunch_heavy], death: [5 × impactSoft_heavy]),
    interface: (volume: 0.6, press: "third_party/interface-sounds/click_001.ogg",
                pause: "third_party/interface-sounds/toggle_001.ogg"),
    stinger: (volume: 0.7, wanted: "third_party/music-jingles/jingles_HIT00.ogg"),
    ambience: (city_volume: 0.25, park_volume: 0.35, city_duck_in_park: 0.5, park_fade: 25.0,
               city_lowpass_hz: 300.0,
               birds: (chirps_per_s: 1.5, lo_hz: 2500.0, hi_hz: 5000.0, chirp_seconds: 0.08)),
    siren: (volume: 0.6, ref_distance: 15.0, max_emitters: 2, audible: 150.0, repick_seconds: 1.0,
            lo_hz: 800.0, hi_hz: 1700.0, period: 4.9),
)
```
(In the file the pools are written out in full, five paths each.) `MixConfig::validate()` checks: positive finite
numbers, `pitch_variants` non-empty and each in (0.5, 2], volumes in (0, 1], `lo_hz < hi_hz`, `max_emitters ≥ 1`,
`voices.* ≥ 1`, `city_duck_in_park` and `min_gain` in [0, 1]. New `MixConfig::sound_paths() -> impl Iterator<Item=&str>`
(all pools + interface + stinger), plus
`MixConfig::check_sounds(&self, manifest: &ThirdPartyManifest) -> Result<(), Vec<String>>`, where each error names the
field and says either "is not listed in third_party/manifest.ron" or "must be an .ogg file", and an empty pool fails.
`src/main.rs` `preflight` (`:100-143`) pushes `check_sounds` errors into `unlisted` (the same shape as fonts). Check:
G-A1 in step 12.

**4. `Synth` asset** — new `src/audio/synth.rs` (replaces `ShotSound`/`NoiseBurstDecoder`, `src/audio/mod.rs:71-136`):
`#[derive(Asset, TypePath)] pub enum Synth { Shot(ShotSynth), Siren(SirenSynth), City(CitySynth), Birds(BirdSynth) }`,
`impl Decodable for Synth { type Decoder = SynthDecoder; }`. `SynthDecoder` is an enum with one state struct per kind,
mono, `SAMPLE_RATE = 44_100` (a law const of the synth, not tuning), `current_span_len() = None`. Shot: noise ×
`exp(-decay t)` + `body_level · sin(2π body_hz t) · exp(-body_decay t)`, output scaled into [-1, 1]; finite
(`total_duration = Some`). Siren/City/Birds: endless, `total_duration = None`. Siren phase:
`phase += 2π f(t)/SR`, wrapped at 2π. Birds: a chirp starts when a xorshift draw < `chirps_per_s / SR`; the frequency
sweeps `hi → lo` over `chirp_seconds` under a raised-cosine window.
`pub fn is_endless(&self) -> bool` (all except `Shot`). Each synth struct is built from its `mix.ron` config.
Check: G-A5.

**5. Cues (one-shots)** — new `src/audio/cues.rs`:
- `#[derive(Component, Reflect)] #[reflect(Component)] pub struct Sound { pub class: SoundClass, pub serial: u64 }`
  and `#[derive(Reflect, Clone, Copy, PartialEq, Eq, Debug)] pub enum SoundClass { .. }`, registered by the plugin
  (BRP).
- `#[derive(Resource)] struct SoundBank` (built in `Startup` from `MixConfig`): `Handle<Synth>` per weapon, siren,
  city, birds; `Vec<Handle<AudioSource>>` per pool from `AssetServer::load`; the interface and stinger handles; a
  round-robin counter per pool; a `next_serial: u64`.
- Pure fns: `pub fn spatial_gain(distance: f32, ref_distance: f32) -> f32 { (ref/d)².min(1) }` (rodio law with
  `spatial_scale = 1/ref`; `d ≤ 0` → 1). `fn spatial(ref) -> PlaybackSettings` =
  `PlaybackSettings::DESPAWN.with_spatial(true).with_spatial_scale(SpatialScale::new(1.0 / ref))`.
- Systems in `Update`, in a set `SoundCues`: `play_shots` (player → non-spatial; NPC → spatial at `muzzle` with
  `Transform::from_translation`; speed `pitch_variants[attack as usize % n]`; volume per weapon), `play_impacts`
  (`BulletTrace.hit` World/Body with the per-shooter-per-frame dedup, `MeleeHit`, `DamageDealt.killed`),
  `play_stinger` (`StarsRaised`), `play_ui_press` (`Query<&Interaction, (Changed<Interaction>, With<MenuAction>)>`,
  `Pressed` only; make `MenuAction` `pub(crate)` via `src/menu/mod.rs` re-export). `OnEnter(GameState::Paused)` runs
  `play_pause_sound`. The listener position for `min_gain` culling is the `GlobalTransform` of the single
  `SpatialListener`; with none (tests), nothing is culled.
- `enforce_voice_budget` (`Update`, `.after(SoundCues)`, so Bevy's auto sync point applies the spawns first): per
  one-shot class, sort alive `(serial, entity)` and `try_despawn` the oldest `count - cap`.
- Observer `On<Add, OrbitCamera>` → insert `SpatialListener::new(mix.listener_ear_gap)`.
Check: G-A3, G-A4.

**6. Loops** — new `src/audio/loops.rs`:
- `Startup`: spawn the two ambience entities `(Sound{Ambience}, AudioPlayer(city|birds), PlaybackSettings::ONCE
  .with_volume(Volume::Linear(0.0)))`. They live for the session, not `CityScoped`; their gain handles state.
- Pure fns: `pub fn distance_to_polygon(p: Vec2, poly: &[Vec2]) -> f32` (0 inside, using `gta_sim::world::contains_convex`;
  else min distance to the edges), `pub fn park_weight(distance: f32, fade: f32) -> f32`,
  `pub fn ambience_gains(park: f32, cfg: &AmbienceMixConfig) -> (f32, f32)`,
  `pub fn pick_sirens(listener: Vec3, cops: &[(Entity, Vec3)], max: usize, audible: f32) -> Vec<Entity>` (nearest first,
  ties broken by entity bits so the choice is stable).
- `update_ambience` (`Update`, not in `Paused`): computes `AmbienceMix` (0,0 unless `Playing|Wasted|Busted`), then
  `sink.set_volume(Volume::Linear(gain * global.volume.to_linear()))` on the two `AudioSink`s (`Query<(&Sound, &mut
  AudioSink)>`; no sink yet or no device → nothing to do).
- `update_sirens` (`Update`, run in `Playing|Wasted|Busted`; timer `Local<f32>` on `Time<Real>`): despawn all sirens
  when `stars == 0`. Otherwise every `repick_seconds` compute `pick_sirens` over `PoliceUnit` (state not Dead/Leave)
  and diff it with existing sirens (`Query<(Entity, &ChildOf), With<SirenEmitter>>`). Despawn the dropped ones
  (`try_despawn`) and spawn a child for each new cop: `(SirenEmitter, Sound{Siren}, ChildOf(cop),
  Transform::from_xyz(0, head, 0), AudioPlayer(siren), PlaybackSettings::ONCE.with_spatial(true)
  .with_spatial_scale(SpatialScale::new(1/ref)).with_volume(Volume::Linear(siren.volume)))`. The head offset is a
  presentation-local `const` law (1.0 m above the body centre), not tuning. Remaining sirens re-volume on
  `resource_changed::<GameSettings>` (`set_volume(siren.volume × global)`).
- `OnEnter(GameState::Paused)`: `pause()` every `AudioSink`/`SpatialAudioSink` whose `Sound.class` is `Ambience` or
  `Siren`. `OnExit(Paused)`: `play()`.
- Reflected resource `#[derive(Resource, Reflect, Default)] #[reflect(Resource)] pub struct AmbienceMix { pub city: f32,
  pub park: f32 }`, registered.
Check: G-A5, G-A6.

**7. Plugin wiring** — `src/audio/mod.rs`: `pub struct GameAudioPlugin` = `SynthSourcePlugin`
(`app.add_audio_source::<Synth>()`, the only piece that needs bevy's `AudioPlugin`) + `SoundCuesPlugin` (step 5) +
`SoundLoopsPlugin` (step 6). `src/main.rs:16,281`: replace `ShotAudioPlugin` with `GameAudioPlugin`. The test harness
adds `SoundCuesPlugin`/`SoundLoopsPlugin` with `init_asset::<Synth>()` and `init_asset::<AudioSource>()` stand-ins. That
follows the render stand-in precedent (`character_gate.rs:68-74`) and keeps the tests off the real sound device, which
`AudioPlugin` would open (`AudioOutput::default`, `audio_output.rs`). Check: `cargo build -j 4` and a manual run where
shots are heard.

**8. Juice config** — `src/juice/config.rs` + `assets/juice/juice.ron`:
`ShakeConfig` gains `shot_trauma: 0.1`, `hurt_trauma: 0.2`, `death_trauma: 0.4` (each in (0, 1]), and
`death_radius: 12.0` (m, > 0). `JuiceConfig` gains `camera_motion_reduced_scale: 0.3` ([0,1], the recoil share left with
"reduce camera motion"), `vignette: (color: (0.75, 0.0, 0.0), per_hurt: 0.35, max: 0.6, decay_per_s: 0.8, radius: 0.9,
smoothness: 3.0)`, `damage_arc: (seconds: 0.6, radius_px: 150.0, thickness_px: 6.0, color: (0.9, 0.1, 0.1))`,
`star_pulse: (scale: 1.3, seconds: 0.35)` (scale > 1). Validation follows the file's existing helpers. Update the
literal `ShakeConfig` in `src/juice/shake.rs` tests (`:88-98`). Check: `shipped_juice_validates` green, plus one
sabotage test per new block, sitting strictly on the failing side (`vignette.max = 1.5`, `star_pulse.scale = 0.9`).

**9. Trauma, hurt, stars-raised** — `src/juice/shake.rs` + `src/juice/mod.rs`:
`add_melee_trauma` becomes `add_trauma` (ShotFired player rows, MeleeHit, PlayerHurt, DamageDealt.killed within
`death_radius` of the player's `Transform`, target ≠ player; target transform via `Query<&Transform>`, missing → skip).
New `detect_player_hurt` and `detect_stars_raised` (B.1) in `src/juice/mod.rs`, which writes `PlayerHurt` and
`StarsRaised`. They are ordered before `add_trauma`/vignette in the `Update` chain:
`(detect_player_hurt, detect_stars_raised, add_trauma, shake_camera).chain()`.
`kick_camera` (`:40-54`) multiplies the kick by `camera_motion_reduced_scale` when `settings.reduce_camera_motion`.
Check: G-J2 rows.

**10. Vignette + damage arc** — new `src/juice/vignette.rs` (observer `On<Add, OrbitCamera>` inserts `Vignette`; a
`VignetteLevel` resource; `update_vignette` on `Time<Real>`, writes `vignette.intensity = if no_flashes {0} else
{level}`) and new `src/juice/damage_arc.rs` (`DamageArc` reflected component, `pub fn arc_angle(yaw, player, attacker)
-> Option<f32>` (`None` when the xz distance < 1e-3), `spawn_or_refresh_arcs` from `DamageDealt`, `update_arcs`
(angle from `OrbitCamera.yaw`, player `Transform`, shooter `Transform` if it still exists; alpha =
`left/seconds` on `BorderColor.top`; `try_despawn` at 0). Arcs carry `CityScoped`. Register `Vignette` and `DamageArc`
types for BRP. Check: G-J2 vignette rows, G-J3.

**11. Star pulse** — `src/hud/stars.rs`: `StarRow` gets a `StarPulse { left: f32 }` component at spawn (`:187-203`).
`pulse_stars` (added to `HudPlugin` `Update`, `:31-40`) resets `left = star_pulse.seconds` on `StarsRaised`, ticks on
`Time<Real>`, and writes `UiTransform.scale = Vec2::splat(star_pulse_scale(elapsed, &cfg))`.
`pub(super) fn star_pulse_scale(elapsed: f32, cfg: &StarPulseConfig) -> f32` uses `EaseFunction::BackOut
.sample_clamped(elapsed / seconds)`. Check: G-J3 endpoint rows (0 → `scale`, ≥ seconds → 1.0).

**12. Settings: "reduce camera motion"** — `src/settings/mod.rs:15-40`: field `pub reduce_camera_motion: bool` (doc:
"Recoil kick scaled by `juice.ron` `camera_motion_reduced_scale`"), default false. Old files still load, because the
fields are applied one by one (`bevy-settings lib.rs:513-533`). `src/menu/widgets.rs:13-19`: `SettingKey::ReduceCameraMotion`.
`src/menu/screens.rs`: text (`:115-117`), row (`:141-147`, after `ReduceShake`), toggle (`:295-298`).
`src/menu/menu_config.rs`: `pub reduce_camera_motion: String` + non-empty check. `assets/ui/strings.ron:372`:
`reduce_camera_motion: "Меньше движения камеры"`. Run `python tools/qa/font_check.py`. Check: the existing
`sanitize_table` / `step_at_a_bound_leaves_settings_unmarked` stay green; `font_check.py` passes.

**13. Headless gates** (`cargo test -p gta_like --bin gta_like -j 4`):
- `src/audio/gate.rs` (`#[cfg(test)] mod gate;` in `src/audio/mod.rs`). The harness `audio_app()` = `MinimalPlugins +
  TransformPlugin + AssetPlugin::default() + StatesPlugin`, `FixedTimesteps(1)`, stand-ins
  `init_asset::<Synth>()`, `init_asset::<AudioSource>()`, `compose_sim(TestArea)`, shipped `MixConfig`,
  `GameSettings::default()`, `SoundCuesPlugin + SoundLoopsPlugin`, then `finish/cleanup`, then updates until the
  `Player` exists (the `character_gate` pattern). No `AudioPlugin`: no device, so sinks never form, which is exactly
  the leak condition G-A3 needs.
  - `mix_sounds_are_manifest_oggs` (G-A1): shipped mix + shipped manifest → Ok. Fixtures (mutated clones): an
    unlisted path → error contains `"not listed"`; `".wav"` → `"must be an .ogg"`; an empty `death` pool → `"empty"`.
    Each sabotage gives a different error.
  - `mix_oggs_decode` (G-A2): for each `sound_paths()`, read `assets/<path>`, build
    `AudioSource { bytes: bytes.into() }`, `decoder()`, and assert `sample_rate() > 0`, `channels() ∈ {1,2}` and
    `take(1024).count() > 0`. Skip with `eprintln!("SKIP ...fetch_assets")` if the pack directories are absent. The
    check is a helper `fn decodes(bytes: &[u8]) -> Result<(), String>` that catches the decoder panic with
    `std::panic::catch_unwind` (bevy's `decoder()` unwraps). Flip: feed it the first 100 bytes of a real file → `Err`
    → RED; the full file → `Ok`.
  - `voice_budget_holds` (G-A3): spawn a stand-in shooter entity with `Transform` 20 m from the player. Over 4 updates,
    write 30 `ShotFired` (NPC, SMG) + 30 `BulletTrace{hit: World}` from distinct shooters (so the dedup does not hide
    them) per update. After every update: `count(Shot) ≤ voices.shot`, `count(Impact) ≤ voices.impact`, and the
    surviving serials are the top-`cap` ones. Flip: remove `enforce_voice_budget` from the plugin → RED (counts 120);
    restore.
  - `emitters_are_placed` (G-A4): NPC shot at muzzle `M` → one `Sound{Shot}` with `spatial`, `Transform.translation == M`,
    and `spatial_scale == Some(1/npc_ref_distance)`; player shot → `spatial == false`; `BulletTrace{hit: World, to: P}`
    → `Impact` at `P`; `MeleeHit{knockdown: true}` → the handle comes from the `heavy` pool. Flip: swap the
    player/NPC branch → RED.
  - `loops_are_endless_once` (G-A5): for each endless `Synth`, `decoder().total_duration() == None` and
    `take(441_000).count() == 441_000`. The two ambience entities have `mode == Once`. Flip: `LOOP` → RED.
  - `pure_tables` (G-A6): `spatial_gain(4,8)=1`, `(8,8)=1`, `(16,8)=0.25`, `(24,8)=1/9`; `distance_to_polygon` on the
    square `(0,0)-(10,10)`: inside `(5,5)` → 0, `(15,5)` → 5, `(13,14)` → 5 (corner, 3-4-5); `park_weight(0,25)=1`,
    `(12.5,25)=0.5`, `(30,25)=0`; `ambience_gains(1, cfg)` = `(city·(1-duck), park)`; `pick_sirens` with cops at 10,
    30, 200 m, `max=2`, `audible=150` → `[10 m, 30 m]`; with `max=1` → `[10 m]`; with all beyond `audible` → `[]`.
- `src/juice/feedback_gate.rs` (`#[cfg(test)]` in `src/juice/mod.rs`). Harness: the same base as above with
  `JuicePlugin` (it needs shipped `JuiceConfig`, `UiConfig`, `UiFonts{Handle::default()×2}` as in
  `damage_numbers_gate.rs:37-58`, plus `GameSettings`), and a stand-in camera entity `(OrbitCamera{..},
  Transform::default())` so the vignette observer fires.
  - `juice_never_writes_virtual_time` (G-J1): walk `CARGO_MANIFEST_DIR/src` recursively, and for every `.rs` line
    (`lines()`, CRLF-safe) search the forbidden tokens built with `concat!` so the gate file does not match itself:
    `"ResMut<" "Time"`, `"resource_mut::<" "Time"`, `"set_relative_speed"`. Report `file:line`. A self-test runs
    `forbidden(line)` on three in-memory lines, one per token → each `Some`, and on
    `world.resource::<Time<Virtual>>()` → `None` (test reads stay allowed). Flip: add `fn _p(_: ResMut<Time<Virtual>>) {}`
    to `src/juice/shake.rs` → RED; restore.
  - `trauma_rows_and_time_untouched` (G-J2): one case per row, trauma reset between cases. Player shot → +0.1. NPC shot
    → +0. Melee with the player as attacker → +0.25. `DebugDamage{10}` (one fixed tick) → +0.2 and vignette intensity
    > 0. Kill of a stand-in target at `death_radius − 1` → +0.4. At `death_radius + 1` → +0. With
    `GameSettings.no_flashes` another hurt leaves intensity 0. With `reduce_camera_motion` a player pistol shot gives
    `CameraRecoil.pitch ≈ recoil_deg.pistol.to_radians() × 0.3 × (decay of one frame)`, compared against the value
    without the setting (the ratio is 0.3 ± 1e-4, so the decay cancels). After **every** update: `Time<Virtual>`
    `relative_speed() == 1.0`, `!is_paused()`, and `Time<Fixed>.elapsed()` advanced by exactly one step (the
    `character_gate.rs:621-634` pattern). Expected deltas are computed from the shipped `juice.ron`. Compare before and
    after the same update, and let the decay run on `Time<Real>` with `TimeUpdateStrategy::FixedTimesteps(1)`.
    Real-time delta per update is the tick, so assert `trauma_after ≥ trauma_before + row - decay_per_s·dt - 1e-4` and
    `≤ trauma_before + row + 1e-4`. For the zero rows, assert no rise. Flip: drop the `death_radius` check → the
    "+1 m outside" row goes RED.
  - `arc_angle_table` (G-J3): the nine rows of B.4 (±π accepted as either sign for "bottom"), plus `None` for a
    coincident attacker. `star_pulse_scale(0) == 1.3`, `(seconds) == 1.0`, `(10·seconds) == 1.0`.
  Every presentation gate is run 3 times by the stage that reports it (domain lesson TASK-022).

**14. QA scenario** — new `tools/qa/scenarios/t13.py` (`--out`, release, `--seed 1`, the t7/t11 structure). Reuse
`t5` (`screenshot`, `damage`, `log_errors`, `resource_value`, `game_state`, `wait_chunks`), `t6` (`rows`, `aim_at`,
`dummies`, `teleport`, `camera_config`, `weapon_pickups`), `t7` (`face_dummy`, `me`), `t11` (`set_heat`, `cops`). Read
the caps (`voices`, `siren.max_emitters`) from `mix.ron` with a regex, failing as `GATE BROKEN` if not found. Extend
the log check to also match `"ogg"`, `"audio"`, `"Decoder"`, `"panicked"`.
1. `fetch_assets.py --check`; launch; golden hash; chunks.
2. Listener: rows of `SpatialListener` → exactly 1, and that entity also has `OrbitCamera`.
3. Ambience: `Sound` rows with class `Ambience` == 2. `AmbienceMix` at the spawn is logged. Teleport to
   `CityLandmarks.park_center`, wait 1 s: `AmbienceMix.park > 0.9` and `city < city_volume`.
4. Melee (the player starts unarmed): face the middle dummy, 3 clicks 0.3 s apart. Poll `Sound` rows every 50 ms for
   1.5 s: `Impact` seen ≥ 1, max per class ≤ cap. Read `CameraShake.trauma > 0` (make `CameraShake`
   `#[derive(Reflect)] #[reflect(Resource)]` + register in step 9). Screenshot.
5. Guns: pick up a pistol (t6 path), 6 shots at a dummy 0.35 s apart, polling: `Shot` seen ≥ 1, `Impact` seen ≥ 1, max
   per class ≤ cap. A screenshot mid-series (tracer, flash, shake). Then one shot into a wall
   (aim at a building face): `Impact` appears (world).
6. Hurt: `damage(game, 10)` (`DebugDamage`) → within 0.3 s, `Vignette.intensity > 0` on the camera and trauma > 0;
   screenshot. Mutate `GameSettings.no_flashes = true` → `damage` again → intensity stays ≤ 1e-4. Reset to false.
7. Arc: write `DamageDealt { shooter: <right dummy>, target: <player>, point: <player>, damage: 0, shot: 0,
   headshot: false, killed: false }` via `world.write_message`. Within 0.2 s there is a `DamageArc` whose `angle`
   matches Python `atan2(d·right, d·forward)` from `OrbitCamera.yaw` and the positions within 0.15 rad (sign check at
   runtime); screenshot.
8. Wanted: raise armour (t11 pattern), `set_heat` to star 2. Within 0.5 s `Stinger` count == 1, and the `UiTransform`
   of the entity named `Wanted stars` has `scale.x > 1.05` in at least one poll within 0.3 s; screenshot.
9. Sirens: wait ≤ 90 s for cops. Then 1 ≤ `Siren` count ≤ `max_emitters`, and every siren's `ChildOf` parent is a
   `PoliceUnit` in state not Dead/Leave; log the distances. `set_heat(0)` → within 3 s `Siren` count == 0.
10. UI: `send_keys(["Escape"])` → a `Ui` sound appears within 0.3 s; screenshot of the pause; Escape again.
11. Log clean, `shutdown`. `summary.json` holds every count series, the max per class, and the screenshots.
Hard pass/fail rests on counts, parents, positions, `AmbienceMix`, `Vignette.intensity`, `DamageArc.angle` and scale.
Screenshots are evidence for the owner (space them ≥ 0.15 s, domain lesson TASK-007). Sound itself is **not** checked:
agents cannot hear, and `AudioSink` is not reflected.

**15. QA_REPORT owner checklist** (the QA stage records it; the AC "игра звучит и бьёт"):
- [ ] Shots per weapon: body, pitch variation, NPC shots come from their side and fade with distance.
- [ ] Body hit / wall hit / punch / knockdown / kill thud are readable and not too loud against the shots.
- [ ] Wanted stinger (`jingles_HIT00`) and the star pulse on a rise: right mood? Other candidates are in the pack
      (`jingles_HIT01..16`); swapping one is a `mix.ron` + manifest line.
- [ ] Sirens: the wail pans with the cops and is not grating with 2 emitters.
- [ ] City and park ambience: audible but not tiring; the crossfade walking into the central park.
- [ ] Shake on shot/hurt/nearby death, the red vignette, the damage arc: "бьёт", no nausea. The toggles "Уменьшить
      тряску", "Меньше движения камеры" and "Без вспышек" do what they say.
- [ ] Menu click and pause sound.
- [ ] Missing by content: a player "вскрик" (no CC0 voice grunt in Kenney packs; Freesound import is the owner's call,
      GDD §8).

**16. Final checks:** `cargo build -j 4`, `cargo clippy -j 4 -- -D warnings` (also `--features dev`),
`cargo test -p gta_sim -j 4`, `cargo test -p citygen -j 4` (untouched, run anyway), `cargo test -p gta_like --bin
gta_like -j 4`, `python tools/qa/tree_check.py`, `python tools/fetch_assets.py --check`,
`python tools/qa/font_check.py`, `python tools/qa/scenarios/t13.py --out target/qa/t13`. File sizes: every touched file
stays < 750 lines (`src/audio/*` is split into config/synth/cues/loops for that reason).

## 4. Risk areas

- **No audio device on a QA or CI host.** Nothing plays, and one-shot entities never drain. The voice cap bounds them
  (G-A3), and QA counts entities, not sinks, so a deviceless host would still pass while silent. The QA stage records
  the `No audio device found` warning if the log has it.
- **Kenney `.ogg` decode.** rodio/lewton is not exercised by the build: G-A2 decodes every referenced file headless. If a
  file fails, enable bevy feature `symphonia-vorbis` (`bevy-0.19.1/Cargo.toml:2855`). That is a `Cargo.toml` feature
  change and needs a lock re-resolve; it is not planned.
- **Loop memory** (A.3) is guarded by G-A5, but a future implementer adding a Kenney *loop* file must use `LOOP` with a
  finite clip. That is fine, because the memory is the clip size.
- **`set_volume` loses the global volume** (sinks.rs); loops re-apply it (A.7). If this is missed, the volume slider
  "does nothing" for ambience and sirens. That is owner-visible, covered by the owner checklist.
- **Listener on the camera, not the player**: panning follows the camera yaw (what the player sees), and distance is
  +3.8 m. That is standard for third person. If the owner wants the player's ears, move `SpatialListener` to a child of
  the player with the camera's rotation (one observer change).
- **Siren churn**: hysteresis `repick_seconds`; a cop that dies despawns its child siren (linked_spawn), so there is no
  dangling emitter. Siren entities are `CityScoped` through their parent.
- **`BulletTrace` field** is an API change in `gta_sim`: two constructors (hitscan, `tests/new_city.rs`) are updated in
  step 1. `grep -rn "BulletTrace {"` must show no others.
- **`PlayerHurt` from pooled health**: regeneration only raises the pool, so it never fires; the respawn heal raises it;
  `NEW_CITY` swaps the entity (keyed). Armour absorption still counts as hurt (intended: the player was hit).
- **`StarsRaised` on a heat mutation 0 → 4** is one event: one stinger and one pulse (fine).
- **Update readers of fixed-tick messages** follow the existing pattern (`kick_camera`, `damage_numbers`). With
  `Time<Virtual>` paused, no new sim messages arrive, and UI sounds still work.
- **BRP type paths**: `Vignette` might not be auto-registered. Step 10 registers it explicitly, as it does
  `CameraShake`, `Sound`, `DamageArc` and `AmbienceMix`.
- **Frame budget**: the per-frame park scan is O(park polygons); siren picking is O(cops) once per second; spawn/trim
  churn is at most ~150 entities/s under full SMG fire, with inaudible ones culled before spawn. All small; not gated.

## 5. Open questions

Under the full-autonomy run the orchestrator answers these. The recommended defaults are already in the plan.

1. **Сирены у пеших копов (до T15).** GDD §8 говорит "от полицейских юнитов и машин", но машины появятся только в T15.
   - A (по умолчанию): сирена на 2 ближайших живых копах, пока звёзд > 0. Улица "звучит" погоней уже сейчас.
   - B: пешие копы без сирен, сирены только с машинами в T15. Тогда в T13 сирен нет, и пункт цели не выполнен.
   - C: одна "дальняя" сирена без позиции, пока звёзд > 0. Проще, но теряется пространственность из цели.
2. **Что делает "Меньше движения камеры".** A (по умолчанию): отдача камеры ×0.3 (другого движения камеры, кроме
   тряски, в игре пока нет). B: A + мгновенный переход в прицел без плавной смены FOV. В T14 сюда же войдёт lag камеры машины.
3. **"Без вспышек" и красная виньетка.** A (по умолчанию): переключатель выключает и виньетку, дуга направления
   остаётся. B: виньетка остаётся, но слабее (множитель в `juice.ron`).
4. **Stinger и контент.** По умолчанию `jingles_HIT00` (0.28 с). Остальные `HIT01..16` в том же архиве, замена стоит
   одну строку в `mix.ron` и в манифесте. "Вскрик" игрока и "глухой sting" смерти (§8) не входят в список цели T13, и
   CC0-голоса у Kenney нет. Оставить на ручной импорт владельцем?

---
Evidence for this plan (task scratch): `scratch/kenney/{kenney_impact-sounds.zip, kenney_interface-sounds.zip,
kenney_music-jingles.zip}` (downloaded 2026-09-24 with Python urllib), `ogginfo.txt` (channels, rate, duration, sha256
of the candidates), `manifest_snippet.ron`, `manifest_with_audio.ron` (validated by `fetch_assets.py --validate-only`),
`*.html` (pack pages with the zip URLs and the CC0 statement). Sources consulted: Wwise playback limiting (URL above),
Xbox Accessibility Guideline 117, WCAG 2.3.1, siren wail measurements (ScienceDirect, above).

children: 0 launched / 0 reported.
