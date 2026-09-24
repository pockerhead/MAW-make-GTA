# PLAN_FINAL — TASK-014 (GDD T13): sound and juice

Reviewer: plan-reviewer-2 (claude opus, medium). Inputs: `TASK_FINAL.md` (incl. updated Q4), `PLAN_V2.md` (wins on every
conflict), `PLAN.md` (detail source), `OPEN_DECISIONS.md` (rodio dev-dependency approved). Every engine claim below was
re-read in `~/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/` at the `Cargo.lock` versions: bevy/bevy_audio/
bevy_ui/bevy_math/bevy_time/bevy_app/bevy_post_process 0.19.1, rodio 0.22.2, bevy-settings 0.19.1. Builds use `-j 4`.

Cost of error: **mixed.** Silent class, real gates: voice leak with no audio device, loop memory, an `.ogg` that panics
at play (`AudioSource::decoder()` unwraps, `bevy_audio-0.19.1/src/audio_source.rs:96-102`), mirrored spatial pan,
mirrored damage arc, `Time<Virtual>` writes, trauma rows that never fire, unlisted/unfetched sound files. Owner class
(how it sounds and feels, whether the Kenney picks fit): mechanism + QA evidence + owner checklist, no machinery.

## 0. Disconfirmation (done first)

**Counter-example chosen:** V2's central fix R1 (mirror the listener ears) is wrong if anything between
`SpatialListener` and rodio already swaps the ears; then the "fix" itself would ship a mirrored pan.
**Search:** `bevy_audio-0.19.1/src/audio_output.rs:54-69,135-139` passes `(emitter, left_ear*scale, right_ear*scale)` in
order to `rodio::SpatialPlayer::connect_new`; `rodio-0.22.2/src/spatial_player.rs:27-38,67-75` forwards them in order to
`Spatial::new`/`set_positions`; `bevy_audio sinks.rs:338-341` keeps the order on updates; `spatial.rs:57-60` gives channel 0
the factor `((left_dist - right_dist)/max_diff + 1)/4 + 0.5`, i.e. MORE left gain when the source is nearer the right
ear. Upstream master has the two expressions swapped (fetched today:
`left_diff_modifier = (((right_dist - left_dist) ...`). **Result: the counter-example did not hold; R1 is real**, the
mirror is required. Also confirmed R2 (`channel_volume.rs:74-80` sums channels, the `/num_channels` `map` is discarded).

## 1. Summary

Presentation-only slice on top of existing sim messages. The only sim changes are `BulletTrace.hit: TraceHit`
(what a pellet stopped on) and one re-export (`dist_point_segment`). Audio (`src/audio/`, data `assets/audio/mix.ron`):
one procedural `Synth` asset (shots with a low body, siren wail, city noise, park birds; endless decoders played
`Once`), Kenney CC0 `.ogg` files (impacts, UI, wanted stinger, death sting, player-hurt thud) through the third-party
manifest, a per-class voice budget that steals the oldest voice, a `SoundStats` resource for QA, spatial emitters
(NPC shots, impacts, sirens on the 2 nearest live cops) heard by a mirrored `SpatialListener` on the camera, city/park
ambience gains, loop pause. Juice (`src/juice/`, `src/hud/stars.rs`, data `assets/juice/juice.ron`): trauma rows for
shot/hurt/nearby death, red vignette, damage-direction arc, star pulse, "Меньше движения камеры" setting. Gates: sim
test rows, manifest counts, headless client gates G-A1..G-A7 and G-J1..G-J3, runtime QA `t13.py`, owner checklist.

---

## 2. Implementation steps

Order = dependency order. Each step names its check. Line numbers were verified on HEAD `164a797` but may drift;
symbols are authoritative.

### Step 1. Sim: `BulletTrace.hit` — `crates/gta_sim/src/combat/hitscan.rs`, `combat/mod.rs`, tests
- Add next to `BulletTrace` (`hitscan.rs:66-73`):
  `#[derive(Reflect, Clone, Copy, Debug, PartialEq, Eq)] pub enum TraceHit { Nothing, World, Body }` and field
  `pub hit: TraceHit` on `BulletTrace` with doc `/// What the pellet stopped on; `Body` = a character that is not `Dead`.`
- `fire_weapons` (`:228-248`): miss branch (`:235`) writes `hit: TraceHit::Nothing`. Hit branch: move the
  `let (target, headshot) = colliders.get(hit.entity).map_or(...)` line ABOVE the `traces.write`, then
  `hit: if targets.contains(target) { TraceHit::Body } else { TraceHit::World }` (`targets: Query<&mut Health, Without<Dead>>`
  is already a parameter). A corpse is `World` (accepted).
- `combat/mod.rs`: add `TraceHit` to the `pub use hitscan::{...}` list (`:8-11`) and `.register_type::<TraceHit>()` next to
  `.register_type::<BulletTrace>()` (`:63`).
- `crates/gta_sim/src/world/mod.rs:9-12`: add `dist_point_segment` to the `pub use citygen::{...}` list (the client has no
  `citygen` dependency; `citygen::dist_point_segment` is `pub`, `citygen/src/lib.rs:17`).
- `crates/gta_sim/tests/new_city.rs:323-327`: add `hit: TraceHit::Nothing` (import from `gta_sim::combat`).
  `grep -rn "BulletTrace {" crates src tests` must show only these three constructors.
- `crates/gta_sim/tests/shooting.rs`: in `pistol_hits_dummy_at_10m_for_table_damage` add
  `assert_eq!(shots.trace_log[0].hit, TraceHit::Body);`; in `wall_between_muzzle_and_target_blocks`, after the first
  (walled) shot, `assert_eq!(shots.trace_log[0].hit, TraceHit::World);`.
- Check: `cargo test -p gta_sim -j 4` green. Flip: write `TraceHit::World` unconditionally → the dummy assertion RED; restore.

### Step 2. Manifest: three audio packs — `assets/third_party/manifest.ron`, `crates/gta_sim/tests/asset_manifest.rs`
- Append the three pack records from **`scratch/pr2/manifest_snippet_final.ron`** verbatim (fix indentation to the
  file's 8-space pack level). It is `scratch/kenney/manifest_snippet.ron` plus one music-jingles line:
  `(archive: "Audio/Sax jingles/jingles_SAX01.ogg", path: "jingles_SAX01.ogg", sha256: "272b8160616d0f924f447df3f8543ab83440c5e41168107bd5abe1140894122b")`.
  The combined manifest `scratch/pr2/manifest_with_audio_final.ron` passes `python tools/fetch_assets.py --validate-only` → `valid`.
- `shipped_manifest_is_valid` (`asset_manifest.rs:78-131`): add `"impact-sounds"`, `"interface-sounds"`,
  `"music-jingles"` to the expected name set; exclude the three from the "4 files, no rig" loop filter; assert
  explicitly: impact-sounds **26** files (25 `.ogg` + License), interface-sounds **3** (click_001, toggle_001, License),
  music-jingles **3** (jingles_HIT00, jingles_SAX01, License); each `rig.is_none()` and `license == AssetLicense::CC0`.
- Install: `python tools/fetch_assets.py --cache maw/tasks/in_progress/TASK-014/scratch/kenney` (zips match the pinned
  archive sha256; no network). All three `License.txt` contain both CC0 markers (`Creative Commons Zero`, `CC0`).
- Check: `python tools/fetch_assets.py --check`; `cargo test -p gta_sim --test asset_manifest -j 4` green (G-M),
  including `local_assets_match_manifest` (goes RED "partial install" until the fetch ran — expected). Flip: drop one
  impact file line → count RED; restore.

### Step 3. `mix.ron` + `MixConfig` — new `src/audio/config.rs`, `assets/audio/mix.ron`, `src/main.rs`
Move `MIX_CONFIG`, `MixConfig` and validation from `src/audio/mod.rs:11-69` into `src/audio/config.rs` (`mod.rs` keeps
plugin wiring and `pub use config::{MIX_CONFIG, MixConfig};`). All structs `#[derive(Deserialize, Clone, Debug)]
#[serde(deny_unknown_fields)]`. New `assets/audio/mix.ron` (every number with a unit comment):
```ron
(
    listener_ear_gap: 0.3,        // m between the camera listener's ears
    min_gain: 0.01,               // spatial one-shots quieter than this at the listener are not spawned
    voices: (shot: 12, impact: 12, hurt: 2, ui: 4, stinger: 1),   // alive one-shots per class; the oldest is stolen
    shot: (
        pitch_variants: [0.95, 1.0, 1.05],               // playback speed, cycled by attack id (GDD §8: ±5%)
        npc_ref_distance: 8.0,                           // m of full volume around an NPC muzzle, then (ref/d)^2
        pistol:  (seconds: 0.18, decay: 22.0, volume: 0.5,  body_hz: 140.0, body_decay: 30.0, body_level: 0.6),
        smg:     (seconds: 0.10, decay: 35.0, volume: 0.35, body_hz: 170.0, body_decay: 45.0, body_level: 0.4),
        shotgun: (seconds: 0.35, decay: 12.0, volume: 0.7,  body_hz: 90.0,  body_decay: 18.0, body_level: 0.8),
    ),
    // 0.4: rodio 0.22.2 downmixes stereo files as L+R on spatial emitters (up to 2x); all impact files are stereo.
    impacts: (
        volume: 0.4,
        ref_distance: 6.0,        // m of full volume, then (ref/d)^2
        bullet_body: [
            "third_party/impact-sounds/impactSoft_medium_000.ogg", "third_party/impact-sounds/impactSoft_medium_001.ogg",
            "third_party/impact-sounds/impactSoft_medium_002.ogg", "third_party/impact-sounds/impactSoft_medium_003.ogg",
            "third_party/impact-sounds/impactSoft_medium_004.ogg",
        ],
        bullet_world: [
            "third_party/impact-sounds/impactGeneric_light_000.ogg", "third_party/impact-sounds/impactGeneric_light_001.ogg",
            "third_party/impact-sounds/impactGeneric_light_002.ogg", "third_party/impact-sounds/impactGeneric_light_003.ogg",
            "third_party/impact-sounds/impactGeneric_light_004.ogg",
        ],
        punch: [
            "third_party/impact-sounds/impactPunch_medium_000.ogg", "third_party/impact-sounds/impactPunch_medium_001.ogg",
            "third_party/impact-sounds/impactPunch_medium_002.ogg", "third_party/impact-sounds/impactPunch_medium_003.ogg",
            "third_party/impact-sounds/impactPunch_medium_004.ogg",
        ],
        heavy: [
            "third_party/impact-sounds/impactPunch_heavy_000.ogg", "third_party/impact-sounds/impactPunch_heavy_001.ogg",
            "third_party/impact-sounds/impactPunch_heavy_002.ogg", "third_party/impact-sounds/impactPunch_heavy_003.ogg",
            "third_party/impact-sounds/impactPunch_heavy_004.ogg",
        ],
        death: [
            "third_party/impact-sounds/impactSoft_heavy_000.ogg", "third_party/impact-sounds/impactSoft_heavy_001.ogg",
            "third_party/impact-sounds/impactSoft_heavy_002.ogg", "third_party/impact-sounds/impactSoft_heavy_003.ogg",
            "third_party/impact-sounds/impactSoft_heavy_004.ogg",
        ],
    ),
    // The player's own body (GDD §8 "Урон игроку"): non-spatial, no stereo sum; no CC0 voice exists, so a heavy thud.
    hurt: (
        volume: 0.6,
        pool: [
            "third_party/impact-sounds/impactPunch_heavy_000.ogg", "third_party/impact-sounds/impactPunch_heavy_001.ogg",
            "third_party/impact-sounds/impactPunch_heavy_002.ogg", "third_party/impact-sounds/impactPunch_heavy_003.ogg",
            "third_party/impact-sounds/impactPunch_heavy_004.ogg",
        ],
    ),
    interface: (volume: 0.6, press: "third_party/interface-sounds/click_001.ogg",
                pause: "third_party/interface-sounds/toggle_001.ogg"),
    stinger: (
        volume: 0.7,
        wanted: "third_party/music-jingles/jingles_HIT00.ogg",   // wanted level rises
        death: "third_party/music-jingles/jingles_SAX01.ogg",    // player dies ("глухой sting"): descending sax jingle
    ),
    ambience: (city_volume: 0.25, park_volume: 0.35,
               city_duck_in_park: 0.5,    // share of the city bed removed at park weight 1
               park_fade: 25.0,           // m outside a park over which the park weight falls 1 -> 0
               city_lowpass_hz: 300.0,
               birds: (chirps_per_s: 1.5, lo_hz: 2500.0, hi_hz: 5000.0, chirp_seconds: 0.08)),
    siren: (volume: 0.6, ref_distance: 15.0, max_emitters: 2,
            audible: 150.0,               // m from the listener; farther cops carry no siren
            repick_seconds: 1.0,          // real s between re-choosing which cops carry sirens
            height: 1.0,                  // m above the cop's body centre
            lo_hz: 800.0, hi_hz: 1700.0, period: 4.9),
)
```
- `MixConfig::validate()`: every number finite and > 0 unless stated; `pitch_variants` non-empty, each in (0.5, 2];
  all `volume`s in (0, 1]; `lo_hz < hi_hz` (siren, birds); `max_emitters ≥ 1`; `voices.*` ≥ 1; `city_duck_in_park`
  and `min_gain` in [0, 1]; `siren.height` finite ≥ 0. Error format as today: `"<field> must be ..., got <value>"`.
- `pub fn sound_paths(&self) -> impl Iterator<Item = &str>`: the five `impacts` pools, `hurt.pool`, `interface.press`,
  `interface.pause`, `stinger.wanted`, `stinger.death`.
- `pub fn check_sounds(&self, manifest: &ThirdPartyManifest) -> Result<(), Vec<String>>`, distinct messages:
  `"{MIX_CONFIG}: {field} {path} is not listed in {THIRD_PARTY_MANIFEST}"`, `"{MIX_CONFIG}: {field} {path} must be an .ogg file"`,
  `"{MIX_CONFIG}: {field} pool is empty"`.
- `src/main.rs` `preflight` (`:100-140`): after the font loop, `if let Err(errors) = mix_config.check_sounds(&manifest)
  { unlisted.extend(errors); }`. Imports at `:16` follow the move.
- Check: G-A1 (Step 13), `cargo build -j 4`.

### Step 4. `Synth` asset — new `src/audio/synth.rs` (replaces `ShotSound`/`NoiseBurstDecoder`, `mod.rs:71-136`)
- `#[derive(Asset, TypePath)] pub enum Synth { Shot(ShotSynth), Siren(SirenSynth), City(CitySynth), Birds(BirdSynth) }`;
  each `*Synth` is a plain struct built from its `mix.ron` config. `impl Decodable for Synth { type Decoder = SynthDecoder; }`;
  `SynthDecoder` is an enum with one state struct per kind; `impl Iterator<Item = f32>` and `impl Source`
  (`bevy::audio::{Source, ChannelCount, SampleRate}`, mono, `current_span_len() = None`).
- `const SAMPLE_RATE: u32 = 44_100;` moves here (law of the synth, not tuning).
- Shot: `noise·exp(-decay·t) + body_level·sin(2π·body_hz·t)·exp(-body_decay·t)`, divided by `1 + body_level` so the output
  stays in [-1, 1]; xorshift32 noise as today; finite, `total_duration = Some(seconds)`.
- Siren: phase accumulator `phase = (phase + 2π·f(t)/SR) % 2π`, `f(t) = lo + (hi-lo)(0.5 - 0.5·cos(2π t/period))`, `t`
  wrapped at `period`; output `sin(phase)`.
- City: white noise through a one-pole low-pass: `y += a·(x - y)`, `a = 1 - exp(-2π·lowpass_hz/SR)`.
- Birds: per sample a xorshift draw `< chirps_per_s / SR` starts a chirp (if none active); frequency sweeps `hi → lo` linearly
  over `chirp_seconds`, amplitude `0.5 - 0.5·cos(2π·τ/chirp_seconds)`; silence between chirps.
- Siren/City/Birds: `next()` never returns `None`, `total_duration() = None`. `pub fn is_endless(&self) -> bool` (all but `Shot`).
- Check: G-A5.

### Step 5. Cues (one-shots) — new `src/audio/cues.rs`
- Types (all registered with `register_type`):
  - `#[derive(Reflect, Clone, Copy, PartialEq, Eq, Debug)] pub enum SoundClass { Shot, Impact, Hurt, Ui, Stinger, Siren, Ambience }`
    with `pub const COUNT: usize = 7` and `pub fn index(self) -> usize`.
  - `#[derive(Component, Reflect)] #[reflect(Component)] pub struct Sound { pub class: SoundClass, pub serial: u64 }`.
  - `#[derive(Resource, Reflect, Default)] #[reflect(Resource)] pub struct SoundStats { pub spawned: [u64; 7], pub peak_alive: [u32; 7] }`.
  - `#[derive(Resource)] pub struct SoundBank`: `Handle<Synth>` per weapon (`[_; 3]`, `Weapon::index()`), siren, city,
    birds; `Vec<Handle<AudioSource>>` per pool (bullet_body, bullet_world, punch, heavy, death, hurt) via
    `AssetServer::load`; `press`, `pause`, `wanted`, `death` handles; one round-robin `usize` per pool; `next_serial: u64`.
    Built in `Startup` by `build_sound_bank` (the handles also preload the files).
- **One spawn helper** used by every spawner (one-shots and loops):
  `fn spawn_sound(commands: &mut Commands, bank: &mut SoundBank, stats: &mut SoundStats, class: SoundClass, bundle: impl Bundle) -> Entity`
  — assigns `serial = bank.next_serial++`, increments `stats.spawned[class.index()]`, spawns `(Sound{class, serial}, bundle)`.
- Pure fns: `pub fn spatial_gain(distance: f32, ref_distance: f32) -> f32` = `1` if `distance <= ref_distance` else
  `(ref/d)²` (rodio law `(1/dist_sq).min(1)` with `spatial_scale = 1/ref`); `fn spatial(ref: f32) -> PlaybackSettings` =
  `PlaybackSettings::DESPAWN.with_spatial(true).with_spatial_scale(SpatialScale::new(1.0 / ref))`;
  `pub fn listener(gap: f32) -> SpatialListener` =
  `SpatialListener { left_ear_offset: Vec3::X * gap / 2.0, right_ear_offset: Vec3::X * gap / -2.0 }` with the comment
  `// rodio 0.22.2 Spatial swaps the ear gains (spatial.rs:57-60); mirrored ears restore left/right.`
- Systems in `Update`, `in_set(SoundCues)`:
  - `play_shots` (`MessageReader<ShotFired>`): shooter is `Player` → non-spatial; else spatial at
    `Transform::from_translation(muzzle)` with `spatial(shot.npc_ref_distance)`, culled when
    `spatial_gain(|muzzle - listener|, ref) < min_gain`. Both: `.with_speed(pitch_variants[attack as usize % n])`,
    `.with_volume(Volume::Linear(weapon.volume))`, `AudioPlayer(bank.shot[weapon.index()])`, class `Shot`.
  - `play_impacts`: `BulletTrace` with `hit` `Body`/`World` → `bullet_body`/`bullet_world` pool at `to`; at most one per
    `(shooter, kind)` per frame (`Local<HashSet<(Entity, bool)>>` cleared each run); `MeleeHit` → `heavy` if `knockdown` else
    `punch` at `point`; `DamageDealt.killed` → `death` at `point`. All spatial with `impacts.ref_distance`, volume
    `impacts.volume`, `min_gain` cull, class `Impact`.
  - `play_hurt` (`MessageReader<PlayerHurt>`): non-spatial `PlaybackSettings::DESPAWN` + `hurt.volume`, next `hurt.pool`
    handle, class `Hurt`.
  - `play_stinger` (`MessageReader<StarsRaised>`): non-spatial `wanted` handle, `stinger.volume`, class `Stinger`.
  - `play_ui_press`: `Query<&Interaction, (Changed<Interaction>, With<MenuAction>)>`, `Interaction::Pressed` only, `press`,
    class `Ui`. Make `MenuAction` visible: `src/menu/mod.rs` gains `pub(crate) use widgets::MenuAction;`.
  - Listener position for culling: `Query<&GlobalTransform, With<SpatialListener>>`, first item; none → no culling.
- `OnEnter(GameState::Paused)`: `play_pause_sound` (non-spatial `pause`, class `Ui`).
- `OnEnter(GameState::Wasted)`: `play_death_sting` (non-spatial `death` handle, `stinger.volume`, class `Stinger`).
- `enforce_voice_budget` (`Update`, `.after(SoundCues)`; Bevy's auto sync point applies the spawns first):
  `Query<(Entity, &Sound)>`; per one-shot class (`Shot, Impact, Hurt, Ui, Stinger`) sort alive `(serial, entity)` and
  `commands.entity(e).try_despawn()` the oldest `count - cap`; then for every class
  `peak_alive[i] = max(peak_alive[i], alive_after_trim)`. (It does NOT touch `spawned`.)
- Observer `On<Add, OrbitCamera>` → `commands.entity(event.entity).insert(listener(mix.listener_ear_gap))`.
- `PlayerHurt`/`StarsRaised` are defined in `src/juice/mod.rs` (Step 9); `SoundCuesPlugin` calls `add_message` for both
  (idempotent, `bevy_app-0.19.1/src/sub_app.rs:390-399`).
- Check: G-A3, G-A4, G-A7.

### Step 6. Loops — new `src/audio/loops.rs`
- `Startup` `spawn_ambience`: two entities through `spawn_sound(.., Ambience, (AudioPlayer(city|birds),
  PlaybackSettings::ONCE.with_volume(Volume::Linear(0.0))))`, session-lived (not `CityScoped`); marker enum
  `AmbienceBed { City, Park }` component to tell them apart.
- `#[derive(Resource, Reflect, Default)] #[reflect(Resource)] pub struct AmbienceMix { pub city: f32, pub park: f32 }`, registered.
- `#[derive(Component)] pub struct SirenEmitter;`
- Pure fns: `pub fn distance_to_polygon(p: Vec2, poly: &[Vec2]) -> f32` = `0` if `contains_convex(poly, p, 0.0)` else min
  `dist_point_segment(p, poly[i], poly[(i+1)%n])` (both from `gta_sim::world`); `pub fn park_weight(distance, fade) =
  (1 - distance/fade).max(0.0)`; `pub fn ambience_gains(park: f32, cfg: &AmbienceConfig) -> (f32, f32)` =
  `(city_volume·(1 - city_duck_in_park·park), park_volume·park)`; `pub fn pick_sirens(listener: Vec3, cops: &[(Entity, Vec3)],
  max: usize, audible: f32) -> Vec<Entity>` = drop `d > audible`, sort by `(d, entity.to_bits())`, take `max`.
- `update_ambience` (`Update`): park weight from the player's xz (`Query<&Transform, With<Player>>`) against every
  `City.0.blocks` with `is_park` (`curb` polygon) → max weight; 0 without `City` or player. Gains `(0, 0)` unless
  state is `Playing | Wasted | Busted`. Writes `AmbienceMix`, then for each ambience entity with `&mut AudioSink`:
  `sink.set_volume(Volume::Linear(gain) * global.volume)` (`Res<GlobalVolume>`; `set_volume` replaces the volume, so the
  global factor is re-applied here).
- `update_sirens` (`Update`, `.run_if(in_state(Playing).or(in_state(Wasted)).or(in_state(Busted)))`; `Local<f32>` timer
  on `Time<Real>`): `stars == 0` → `try_despawn` every `SirenEmitter`, reset timer to 0, return. Else when the timer ≤ 0:
  timer = `repick_seconds`; `pick_sirens(listener pos, cops, max_emitters, audible)` over `Query<(Entity, &GlobalTransform,
  &PoliceUnit)>` with `state` not `Dead`/`Leave`; diff with `Query<(Entity, &ChildOf), With<SirenEmitter>>`; `try_despawn`
  sirens whose parent is not picked; for each picked cop without one, `spawn_sound(.., Siren, (SirenEmitter, ChildOf(cop),
  Transform::from_xyz(0.0, siren.height, 0.0), AudioPlayer(bank.siren), PlaybackSettings::ONCE.with_spatial(true)
  .with_spatial_scale(SpatialScale::new(1.0 / siren.ref_distance)).with_volume(Volume::Linear(siren.volume))))`.
  Sirens stay through `Paused` (paused by `sync_loop_pause`); `NEW_CITY` removes them with their `CityScoped` cops
  (`Children` is `linked_spawn`).
- `revolume_sirens` `.run_if(resource_changed::<GlobalVolume>)`: `Query<&mut SpatialAudioSink, With<SirenEmitter>>` →
  `set_volume(Volume::Linear(siren.volume) * global.volume)`.
- `sync_loop_pause` (`Update`): for `Sound` of class `Ambience | Siren` with `AudioSink` or `SpatialAudioSink`
  (two queries): `State<GameState> == Paused && !is_paused()` → `pause()`; `!= Paused && is_paused()` → `play()`.
- Check: G-A5, G-A6.

### Step 7. Plugin wiring — `src/audio/mod.rs`, `src/main.rs`
- `mod config; mod synth; mod cues; mod loops; #[cfg(test)] mod gate;`
- `pub struct GameAudioPlugin` adds `SynthSourcePlugin` (`app.add_audio_source::<Synth>()`, needs bevy's `AudioPlugin`
  from `DefaultPlugins`, which `main.rs:190` adds first), `SoundCuesPlugin` (Step 5 + `init_resource::<SoundStats>()`,
  `register_type::<SpatialListener>()`) and `SoundLoopsPlugin` (Step 6).
- `src/main.rs:16,281`: `ShotAudioPlugin` → `GameAudioPlugin`. Delete `ShotAudioPlugin`, `ShotSound`, `ShotSounds`,
  `create_sounds`, old `play_shots`, `NoiseBurstDecoder`.
- Check: `cargo build -j 4`; manual `cargo run --release -- --seed 1`: shots, a siren and ambience are audible (owner).

### Step 8. Juice config — `src/juice/config.rs`, `assets/juice/juice.ron`
- `ShakeConfig` gains `shot_trauma: 0.1`, `hurt_trauma: 0.2`, `death_trauma: 0.4` (each in (0, 1], same check as
  `melee_trauma`), `death_radius: 12.0` (m, `positive`). Doc comments per field.
- `JuiceConfig` gains:
  - `camera_motion_reduced_scale: 0.3` — share of the recoil kick left with "Меньше движения камеры", [0, 1].
  - `vignette: VignetteConfig { color: Rgb (0.75, 0.0, 0.0), per_hurt: 0.35, max: 0.6, decay_per_s: 0.8, radius: 0.9, smoothness: 3.0 }`
    (`per_hurt`, `decay_per_s`, `radius`, `smoothness` positive; `max` in (0, 1]; `unit_rgb` color).
  - `damage_arc: DamageArcConfig { seconds: 0.6, radius_px: 150.0, thickness_px: 6.0, color: (0.9, 0.1, 0.1) }` (positive, `unit_rgb`).
  - `star_pulse: StarPulseConfig { scale: 1.3, seconds: 0.35 }` (`scale > 1`, `seconds` positive).
- `juice.ron`: add the values above with unit comments.
- Update the `ShakeConfig` literal in `src/juice/shake.rs` tests (`fn cfg()`, ~`:82-92`) with the four new fields.
- Tests in `config.rs`: `shipped_juice_validates` green; new sabotage tests, each strictly on the failing side with its own
  keyword: `vignette.max = 1.5` → `"vignette.max"`, `star_pulse.scale = 0.9` → `"star_pulse.scale"`,
  `shake.death_radius = -1.0` → `"shake.death_radius"`.

### Step 9. Trauma, hurt, stars-raised — `src/juice/mod.rs`, `src/juice/shake.rs`
- `src/juice/mod.rs`: client messages
  `#[derive(Message, Clone, Copy, Debug)] pub struct PlayerHurt { pub amount: f32 }` and
  `#[derive(Message, Clone, Copy, Debug)] pub struct StarsRaised { pub stars: u8 }`; `JuicePlugin` `add_message`s both.
- `detect_player_hurt(players: Query<(Entity, &Health), With<Player>>, mut last: Local<Option<(Entity, f32)>>, writer)`:
  pool = `current + armor`; if `last` is `Some((same entity, prev))` and `pool < prev` → write `PlayerHurt{amount: prev - pool}`;
  always store `(entity, pool)`. A new player entity (respawn swap, `NEW_CITY`) never fires; regen/heal only raises.
- `detect_stars_raised(wanted: Res<WantedLevel>, mut last: Local<u8>, writer)`: `stars > *last` → `StarsRaised{stars}`; store.
- `shake.rs`: `CameraShake` gets `#[derive(Resource, Default, Reflect)] #[reflect(Resource)]`, registered by `JuicePlugin`.
  `add_melee_trauma` → `add_trauma` reading `ShotFired`, `MeleeHit`, `PlayerHurt`, `DamageDealt`:
  player `ShotFired` → `+shot_trauma`; `MeleeHit` the player lands or takes → `+melee_trauma`; each `PlayerHurt` →
  `+hurt_trauma`; `DamageDealt { killed: true }` with `target` not the player and `|point - player.translation| ≤ death_radius`
  → `+death_trauma` (no player → skip the row). Each addition clamps at 1; rows stack (a punch taken = 0.25 + 0.2).
- `JuicePlugin` `Update`: `(kick_camera, (detect_player_hurt, detect_stars_raised, shake::add_trauma, shake::shake_camera).chain())`.
- `kick_camera` (`mod.rs:40-54`): kick × `camera_motion_reduced_scale` when `settings.reduce_camera_motion` (add `Res<GameSettings>`).
- Check: G-J2.

### Step 10. Vignette + damage arc — new `src/juice/vignette.rs`, `src/juice/damage_arc.rs`
- `vignette.rs`: observer `On<Add, OrbitCamera>` inserts `bevy::post_process::effect_stack::Vignette { intensity: 0.0,
  color: Color::srgb(r,g,b), radius, smoothness, ..default() }`. `#[derive(Resource, Default)] struct VignetteLevel(f32)`.
  `update_vignette` (`Update`, after `detect_player_hurt`): `level = min(max, level + per_hurt)` per `PlayerHurt`, then
  `level = max(0, level - decay_per_s·real_dt)`; for every `&mut Vignette` (Query, not `Single`):
  `intensity = if settings.no_flashes { 0.0 } else { level }`. `register_type::<Vignette>()`.
- `damage_arc.rs`: `#[derive(Component, Reflect)] #[reflect(Component)] pub struct DamageArc { pub shooter: Entity, pub angle: f32, pub left: f32 }`, registered.
  `pub fn arc_angle(yaw: f32, player: Vec3, attacker: Vec3) -> Option<f32>`: `d = attacker - player` (xz), `None` if
  `|d| < 1e-3`; `forward = (-sin yaw, 0, -cos yaw)`, `right = (cos yaw, 0, -sin yaw)` (matches `camera/mod.rs:122-124`);
  `Some(atan2(d·right, d·forward))`, positive = clockwise from screen-up.
  `spawn_or_refresh_arcs`: per `DamageDealt` with `target` = player and `shooter` ≠ player: an arc with that shooter
  exists → `left = seconds`; else spawn `(DamageArc{..}, CityScoped, Node { position_type: Absolute, width/height:
  px(2·radius_px), left/top: 50% minus radius_px (`Val::Percent(50.0)` + `UiTransform.translation` or margin -radius),
  border: UiRect::top(px(thickness_px)), border_radius: BorderRadius::MAX, .. }, BorderColor { top: color, ..transparent },
  UiTransform::from_rotation(Rot2::radians(angle)))`.
  `update_arcs` (`Update`, `Time<Real>`): `left -= dt`; `≤ 0` → `try_despawn`; shooter still has a `Transform` → recompute
  `angle` from `OrbitCamera.yaw`, else keep (frozen); `UiTransform.rotation = Rot2::radians(angle)`;
  `BorderColor.top` alpha = `left / seconds`.
- `src/juice/mod.rs`: `mod vignette; mod damage_arc;` and add their systems/observer to `JuicePlugin`.
- Check: G-J2 vignette rows, G-J3.

### Step 11. Star pulse — `src/hud/stars.rs`, `src/hud/mod.rs`
- `#[derive(Component, Reflect)] #[reflect(Component)] pub struct StarPulse { pub left: f32 }`, registered by `HudPlugin`;
  `spawn_stars` (`stars.rs:52-92`) adds `StarPulse { left: 0.0 }` to the row entity.
- `pub(super) fn star_pulse_scale(elapsed: f32, cfg: &StarPulseConfig) -> f32` =
  `1 + (scale - 1)·(1 - EaseFunction::BackOut.sample_clamped(elapsed / seconds))`.
- `pulse_stars` added to the `HudPlugin` `Update` tuple (`hud/mod.rs:31-40`): on any `StarsRaised` set `left = seconds`;
  `left = max(0, left - real_dt)`; `UiTransform.scale = Vec2::splat(star_pulse_scale(seconds - left, &juice.star_pulse))`.
  `HudPlugin` calls `add_message::<StarsRaised>()`. No test harness registers `HudPlugin` (grep verified), so the new
  `Res<JuiceConfig>` param breaks nothing.
- Check: G-J3 rows.

### Step 12. Setting "Меньше движения камеры"
- `src/settings/mod.rs:18-39`: `/// Recoil kick scaled by `juice.ron` `camera_motion_reduced_scale`.` `pub reduce_camera_motion: bool`,
  default `false`. `sanitize` keeps it via `..s.clone()`. Old settings files load (bevy-settings applies fields one by one,
  `bevy-settings-0.19.1/src/lib.rs:513-533`). The two test literals (`settings/mod.rs:105`, `screens.rs:382`) use `..default()`: no change.
- `src/menu/widgets.rs:13-19`: `SettingKey::ReduceCameraMotion`.
- `src/menu/screens.rs`: `setting_text` (`:116-117`) `ReduceCameraMotion => flag(settings.reduce_camera_motion)`; row list
  (`:141-147`) `(SettingKey::ReduceCameraMotion, &menu.reduce_camera_motion, false)` after `ReduceShake`; toggle match
  (`:295-299`) `SettingKey::ReduceCameraMotion => &mut s.reduce_camera_motion` — this match ends in `_ => continue`, so a
  missing arm fails silently: add it.
- `src/menu/menu_config.rs`: `pub reduce_camera_motion: String` (`:23-24`) and `("menu.reduce_camera_motion", &self.reduce_camera_motion)` in the non-empty list (`:71-72`).
- `assets/ui/strings.ron:46`: `reduce_camera_motion: "Меньше движения камеры"`.
- Check: `sanitize_table`, `step_at_a_bound_leaves_settings_unmarked` green; `python tools/qa/font_check.py` passes.

### Step 13. Headless client gates (`cargo test -p gta_like --bin gta_like -j 4`)
Every presentation gate is run **3 times** by the stage that reports it (TASK-022 lesson).

**Root `Cargo.toml`** — new section (approved, `OPEN_DECISIONS.md`):
```toml
[dev-dependencies]
rodio = { version = "=0.22.2", default-features = false }
```
Exactly `default-features = false`, no features: rodio's default set enables `symphonia-vorbis`, which would switch the
test build's `.ogg` decoder away from production's lewton path (`rodio-0.22.2/src/decoder/mod.rs:77` cfg
`all(feature = "lewton", not(feature = "symphonia-vorbis"))`) and make G-A2 test the wrong decoder. `Cargo.lock` gains
only the edge `gta_like → rodio`.

**`src/audio/gate.rs`** (`#[cfg(test)] mod gate;`). Harness `audio_app()`:
`MinimalPlugins + TransformPlugin + AssetPlugin::default() + StatesPlugin`, `TimeUpdateStrategy::FixedTimesteps(1)`,
`init_asset::<Synth>()`, `init_asset::<AudioSource>()`, `init_resource::<GlobalVolume>()` (only `AudioPlugin` inserts it;
without it `update_ambience`/`revolume_sirens` fail param validation), `compose_sim(&mut app, assets_root(),
WorldSource::TestArea)` (`GATE BROKEN` on error), shipped `MixConfig` (validated, `GATE BROKEN` on error),
`GameSettings::default()`, `SoundCuesPlugin + SoundLoopsPlugin`, `app.finish(); app.cleanup();`, then `update()` until a
`Player` exists (≤ 20, else `GATE BROKEN`). No `AudioPlugin`: no device, sinks never form (the leak condition G-A3 needs).
Message fields use spawned stand-in entities with a matching `Transform`, never `Entity::PLACEHOLDER` (sim readers of
`ShotFired`/`DamageDealt` run in `compose_sim`).
- `mix_sounds_are_manifest_oggs` (G-A1, correctness): shipped mix + shipped manifest → `Ok`. Mutated clones, each a
  different error: an unlisted path in `punch` → contains `"is not listed"`; `stinger.death` ending `.wav` (listed name
  with extension changed) → `"must be an .ogg"`; empty `hurt.pool` → `"pool is empty"`.
- `mix_oggs_decode` (G-A2, correctness): for each `sound_paths()` read `assets/<path>`; `fn decodes(bytes: &[u8]) ->
  Result<usize, String>` = `catch_unwind(AssertUnwindSafe(|| { let d = AudioSource { bytes: bytes.into() }.decoder();
  (d.channels().get(), d.count()) }))`; assert channels ∈ {1, 2} and decoded sample count > 0 (full decode, clips ≤ 1 s).
  If a pack directory is absent: `eprintln!("SKIP decode check: packs not fetched; run python tools/fetch_assets.py")`
  and return (precedent `local_assets_match_manifest`). Flip: `decodes(&bytes[..100])` → `Err` (asserted in the same test).
- `voice_budget_holds` (G-A3, correctness): 30 stand-in shooters with `Transform` 20 m from the player. Over 4 updates write
  per update 30 `ShotFired{weapon: Smg, shooter: s_i, muzzle: pos_i, attack: k}` + 30 `BulletTrace{hit: World, shooter: s_i,
  to: pos_i}`. After update k (1-based): alive `Sound{Shot}` == `voices.shot` (12), alive `Impact` == `voices.impact`
  (12), survivors are exactly the top-12 serials of their class, `SoundStats.spawned[Shot] == 30·k`,
  `spawned[Impact] == 30·k`, `peak_alive[Shot] == 12`. Flip: remove `enforce_voice_budget` from the plugin → alive 30·k, RED.
- `emitters_are_placed` (G-A4, correctness): NPC shot at muzzle `M` → one `Sound{Shot}` with `settings.spatial`,
  `Transform.translation == M`, `settings.spatial_scale.map(|s| s.0)` ≈ `Vec3::splat(1/npc_ref_distance)` (1e-6;
  `SpatialScale` has no `PartialEq`); player shot → `!spatial`; `BulletTrace{hit: World, to: P}` → `Impact` at `P` with a
  `bullet_world` handle; `MeleeHit{knockdown: true}` → a `heavy` handle; `PlayerHurt{amount: 5}` → one `Sound{Hurt}`,
  `!spatial`, handle in `hurt.pool`; `DebugDamage{amount: 10_000}` then updates until `GameState::Wasted` (≤ 10) → exactly one
  new `Sound{Stinger}` whose `AudioPlayer` handle == the `death` handle. Flip: swap the player/NPC branch → RED; swap
  `death`/`wanted` in `play_death_sting` → RED.
- `loops_are_endless_once` (G-A5, correctness): for Siren/City/Birds `Synth` built from the shipped mix,
  `decoder().total_duration() == None` and `decoder().take(441_000).count() == 441_000`; Shot synth total_duration is
  `Some`. The two ambience entities have `matches!(settings.mode, PlaybackMode::Once)` (`PlaybackMode` has no `PartialEq`).
  Flip: spawn ambience with `PlaybackSettings::LOOP` → RED.
- `pure_tables` (G-A6): `spatial_gain(4,8)=1`, `(8,8)=1`, `(16,8)=0.25`, `(24,8)=1/9`; CCW square
  `[(0,0),(10,0),(10,10),(0,10)]`: `distance_to_polygon((5,5))=0`, `((15,5))=5`, `((13,14))=5` (corner, 3-4-5);
  `park_weight(0,25)=1`, `(12.5,25)=0.5`, `(30,25)=0`; `ambience_gains(1, shipped) = (0.25·0.5, 0.35)`,
  `ambience_gains(0, shipped) = (0.25, 0)`; `pick_sirens` with cops at 10/30/200 m (listener at origin, `audible` 150):
  max 2 → [10 m, 30 m], max 1 → [10 m], all three beyond 150 → []; two cops at equal distance → lower `to_bits` first in
  both input orders.
- `listener_pans_right_to_right` (G-A7, correctness, upgrade tripwire): helper
  `fn channel_means(listener: &SpatialListener, cam: Transform, emitter: Vec3, scale: f32) -> (f32, f32)`: ears =
  `GlobalTransform::from(cam).transform_point(offset) * scale` (as `audio_output.rs:54-69,135-139`), emitter × scale,
  `rodio::source::Spatial::new(rodio::buffer::SamplesBuffer::new(ChannelCount::new(1).unwrap(),
  SampleRate::new(44_100).unwrap(), vec![0.5 as rodio::Sample; 64]), e, l, r)`; collect 128 interleaved samples; mean
  |even| = channel 0 (left), mean |odd| = channel 1 (right). Rows with `listener(0.3)`, `scale = 1/8`:
  yaw 0, emitter `(10,0,0)` → right > left (worked: scaled ears x = ±0.01875, left_diff 0.5, right_diff 1.0);
  yaw 0, `(-10,0,0)` → left > right; yaw 90° (`Quat::from_rotation_y(FRAC_PI_2)`, right = (0,0,-1)), emitter `(0,0,-10)`
  → right > left. Flip: `SpatialListener::new(0.3)` → all rows RED. When a Bevy upgrade brings a fixed rodio this gate goes
  RED: then remove the mirror from `listener()`, not the gate.

**`src/juice/feedback_gate.rs`** (`#[cfg(test)] mod feedback_gate;` in `juice/mod.rs`). Harness: the same base as
`audio_app` minus the audio plugins, plus `JuicePlugin` with the shipped `JuiceConfig`, shipped `UiConfig`,
`UiFonts { regular: Handle::default(), title: Handle::default() }` (as `damage_numbers_gate.rs:37-58`),
`GameSettings::default()`, and a stand-in `(OrbitCamera { yaw: 0.0, pitch: 0.0, distance: 3.8, pivot: None, aim_blend: 0.0 },
Transform::default())` spawned before `finish()` so the vignette observer fires.
- `juice_never_writes_virtual_time` (G-J1, correctness): walk `CARGO_MANIFEST_DIR/src` recursively; each `.rs` line
  (`lines()`, CRLF-safe) → `normalise(line)` = remove all whitespace, remove every `'<ident>,` lifetime, remove
  `bevy::time::` and `bevy::prelude::` path prefixes. `forbidden(norm)` = contains any of `ResMut<Time<Virtual>>`,
  `ResMut<Time<Fixed>>`, `ResMut<Time>`, `resource_mut::<Time<Virtual>>`, `resource_mut::<Time<Fixed>>`,
  `set_relative_speed(`. Every token literal AND every self-test input in the gate file is built with `concat!` split
  across the token (`concat!("ResMut<", "Time<Virtual>>")`), so the gate's own source never matches. Failure lists
  `file:line`. Self-test: `mut t: ResMut<Time<Virtual>>`, `pub t: ResMut<'w, Time<Virtual>>`,
  `t: ResMut<bevy::time::Time<Virtual>>`, `w.resource_mut::<Time<Fixed>>()`, `t.set_relative_speed(0.5)` → each matches;
  `world.resource::<Time<Virtual>>()` and `mut s: ResMut<TimeUpdateStrategy>` → no match. `.pause()` is deliberately not a
  token (sinks call it); a `Time<Virtual>` pause needs a `ResMut` that is caught. Flip: add `fn _p(_: ResMut<Time<Virtual>>) {}`
  to `src/juice/shake.rs` → RED; restore. (Today `src/` has no writer: all are in `crates/gta_sim/src/flow/`.)
- `trauma_rows_and_time_untouched` (G-J2, correctness + time liveness): one case per row; before each case set
  `CameraShake.trauma = 0` and run one update. dt = `Time<Fixed>.timestep()` (64 Hz → 0.015625 s; `FixedTimesteps(1)`
  advances `Time<Real>` by one timestep, `bevy_time-0.19.1/src/lib.rs:181-183`). Rows (expected from shipped `juice.ron`):
  player `ShotFired` → +0.1; NPC `ShotFired` (stand-in shooter) → no rise; `MeleeHit{attacker: player, target: stand-in}` → +0.25;
  `DebugDamage{10}` → +0.2 (one `PlayerHurt`) and `Vignette.intensity > 0` on the stand-in camera;
  `DamageDealt{killed: true, target: stand-in, point: player + (death_radius − 1)·X}` → +0.4; same at `death_radius + 1` → no rise.
  Bounds per row: `before + row − decay_per_s·dt − 1e-4 ≤ after ≤ before + row + 1e-4`; "no rise": `after ≤ before + 1e-4`.
  `no_flashes = true` + another `DebugDamage{10}` → `intensity == 0`. Recoil: reset `CameraRecoil.pitch = 0`, player pistol
  shot, read pitch `p1`; reset, set `reduce_camera_motion = true`, same shot, `p2`; `p2 / p1 == 0.3 ± 1e-4` (same dt, decay
  cancels). After EVERY update: `Time<Virtual>::relative_speed() == 1.0`, `!is_paused()`, `Time<Fixed>.elapsed()` advanced by
  exactly one timestep (pattern `character_gate.rs:618-634`). Flip: drop the `death_radius` check → the +1 m row RED.
- `arc_angle_table` (G-J3): θ=0: `(0,0,-10)` → 0, `(10,0,0)` → +π/2. θ=π/2: `(-10,0,0)` → 0, `(0,0,-10)` → +π/2,
  `(10,0,0)` → |angle| = π. θ=π: `(0,0,10)` → 0, `(-10,0,0)` → +π/2, `(10,0,0)` → −π/2 (player at origin, tolerance 1e-5).
  Coincident → `None`. `star_pulse_scale(0) == 1.3`, `(0.35) == 1.0`, `(3.5) == 1.0`, min over 1000 samples of
  [0, 0.35] ∈ [0.96, 0.98] (BackOut peak 1.100 at t ≈ 0.58 → min scale 0.970).

### Step 14. QA scenario — new `tools/qa/scenarios/t13.py`
`--out DIR`, release build, `--seed 1`, t7/t11 structure. Reuse t5 (`screenshot`, `damage`, `resource_value`,
`game_state`, `wait_chunks`, `poll`), t6 (`rows`, `aim_at`, `dummies`, `teleport`, `camera_config`, `weapon_pickups`),
t7 (`face_dummy`, `me`), t11 (`set_heat`, `cops`, `wanted`, `arm_player`). Caps (`voices.*`, `siren.max_emitters`,
`ambience.city_volume`) are read from `assets/audio/mix.ron` with a regex; a failed read is `GATE BROKEN`. Class index
order = `Shot, Impact, Hurt, Ui, Stinger, Siren, Ambience`. `panics(game)` = any `"panicked at"` in
`game.log_tail(2_000_000)` (the log holds stderr, `brp.py:59`); after each phase assert no panic and `game_state` answers.
Record a `No audio device` warning in `summary.json` if present. Screenshots ≥ 0.15 s apart. `stats()` = `SoundStats`.
1. `fetch_assets.py --check`; launch; golden hash; `wait_chunks`.
2. Listener: exactly one entity with `SpatialListener`; it also has `OrbitCamera`; its `right_ear_offset.x < 0` (R1 mirror live).
3. Ambience: exactly two `Sound` with class `Ambience`; log `AmbienceMix`. Teleport to `CityLandmarks.park_center`, wait
   1 s: `AmbienceMix.park > 0.9` and `city < city_volume`.
4. Melee (unarmed): face the middle dummy, 3 clicks 0.3 s apart, poll 50 ms for 1.5 s: `spawned[Impact]` delta ≥ 1,
   every `peak_alive[i] ≤ cap[i]`, `CameraShake.trauma > 0` in some poll. Screenshot.
5. Guns: pick up a pistol (t6 path), 6 shots at a dummy 0.35 s apart: `spawned[Shot]` delta ≥ 6 (NPC gunfire may add)
   and the magazine dropped by exactly 6; `spawned[Impact]` delta ≥ 1. Screenshot mid-series. One shot at a building face →
   `spawned[Impact]` delta ≥ 1.
6. Hurt: `damage(game, 10)` → within 0.3 s `Vignette.intensity > 0` on the camera, `trauma > 0` in some poll,
   `spawned[Hurt]` delta ≥ 1; screenshot. Set `GameSettings.no_flashes = true`, damage again → intensity ≤ 1e-4 for 0.3 s.
   Reset to false.
7. Arc: `world.write_message` `DamageDealt{shooter: <rightmost dummy>, target: <player>, point: <player pos>, damage: 0,
   shot: 0, headshot: false, killed: false}`. Within 0.2 s a `DamageArc` exists with `angle` within 0.15 rad of Python
   `atan2(d·right, d·forward)` from `OrbitCamera.yaw` (wiring check) and `> 0` for a dummy on the camera's right.
   Screenshot (visual direction, owner).
8. Wanted: `arm_player`/raise armour (t11 pattern); `set_heat(0)`, wait until `wanted().stars == 0`; `set_heat(<star-2 heat>)`
   → within 0.5 s `spawned[Stinger]` delta == 1; within 0.4 s some poll has `StarPulse.left > 0` on `Wanted stars` with
   `|UiTransform.scale.x − 1| > 1e-3` in the same poll (scale > 1.05 lasts only ~0.09 s, too short for BRP polling).
   Screenshot.
9. Sirens: wait ≤ 90 s for cops; then 1 ≤ alive `Siren` ≤ `max_emitters`, each siren's `ChildOf` parent is a `PoliceUnit`
   not `Dead`/`Leave`; log distances. `set_heat(0)` → within 3 s alive `Siren` == 0.
10. UI: `game.send_keys(["Escape"], 100)` → `spawned[Ui]` delta ≥ 1 within 0.5 s; screenshot of the pause menu; Escape again
    (wait out the key hold first).
11. Death: `damage(game, 10_000)` → state `Wasted` within 1 s and `spawned[Stinger]` delta == 1 within 1 s; wait for
    `Playing` again (≤ 15 s).
12. No panics, `shutdown`. `summary.json`: `SoundStats` series, per-class peaks, deltas per phase, screenshots.
Hard pass/fail rests on `SoundStats`, counts, parents, `AmbienceMix`, `Vignette.intensity`, `DamageArc.angle`,
`StarPulse`, the listener mirror and state transitions. Sound itself is not checked (agents cannot hear; sinks are not
reflected).

### Step 15. QA_REPORT owner checklist (AC "игра звучит и бьёт"), recorded by the QA stage
- [ ] Pan direction: a cop or NPC firing on the right is heard on the right (rodio 0.22.2 mirror fix).
- [ ] Shots per weapon: body, pitch variation; NPC shots fade with distance.
- [ ] Body/wall/punch/knockdown/kill impacts readable, no clipping (spatial stereo is summed; `impacts.volume` 0.4).
- [ ] Player hurt thud (`impactPunch_heavy`, non-spatial) + red vignette: reads as "меня бьют"; not spammy under SMG fire
      (cap 2; if spammy, a cooldown is the next knob).
- [ ] Wanted stinger `jingles_HIT00` + star pulse (alternatives `HIT01..16`: one `mix.ron` + manifest line).
- [ ] Death sting `jingles_SAX01` on "ПОТРАЧЕНО" (picked by descending pitch contour, not by ear; alternatives with the same
      falling contour: `jingles_PIZZI01`, `jingles_NES11`, `jingles_STEEL01`, evidence `scratch/pr2/jingle_contour.txt`).
- [ ] Sirens follow the cops, not grating with 2, pause with the game.
- [ ] City and park ambience, crossfade into the central park.
- [ ] Shake on shot/hurt/nearby death; stacked rows on a punch taken (0.45) and a melee kill nearby (0.65); damage arc
      direction; "бьёт", no nausea. Toggles "Уменьшить тряску", "Меньше движения камеры", "Без вспышек" work.
- [ ] Menu click and pause sound.
- [ ] No vocal "вскрик": by decision (no CC0 voice; synthesized would be worse). Owner may import one later (GDD §8).

### Step 16. Final checks
`cargo build -j 4`; `cargo clippy -j 4 -- -D warnings` and `cargo clippy -j 4 --features dev -- -D warnings`;
`cargo test -p gta_sim -j 4`; `cargo test -p citygen -j 4`; `cargo test -p gta_like --bin gta_like -j 4` (3×);
`cargo tree -p gta_sim -e normal -i bevy_render` empty; `python tools/qa/tree_check.py`;
`python tools/fetch_assets.py --check`; `python tools/qa/font_check.py`;
`python tools/qa/scenarios/t13.py --out target/qa/t13`. Every touched file < 750 lines.

---

## 3. Test plan

| Gate | Class | Where | Carries |
|---|---|---|---|
| shooting.rs `hit` rows | correctness | `-p gta_sim` | `TraceHit` Body/World |
| G-M | correctness | `asset_manifest.rs` | packs, counts 26/3/3, CC0 |
| G-A1 | correctness | `audio/gate.rs` | mix paths listed, `.ogg`, pools non-empty (AC "манифест звуков валиден") |
| G-A2 | correctness | same | every referenced `.ogg` decodes on the production decoder (no panic at play) |
| G-A3 | correctness | same | voice cap, newest survive, `SoundStats.spawned` exact |
| G-A4 | correctness | same | emitter placement/spatiality/pool per event incl. hurt and death sting |
| G-A5 | correctness | same | endless loops played `Once` (no unbounded memory) |
| G-A6 | pure math | same | gains, polygon distance, park weight, siren pick |
| G-A7 | correctness | same | pan side through the real rodio `Spatial` (tripwire) |
| G-J1 | correctness | `juice/feedback_gate.rs` | no `Time<Virtual>` writer in `src/` (AC) |
| G-J2 | correctness + liveness | same | trauma rows, vignette, recoil scale, time untouched each update |
| G-J3 | pure math | same | arc angle 3-yaw table, pulse curve |
| t13.py | runtime liveness | QA | AC runtime scenario |
| Owner checklist | owner | QA_REPORT | "звучит и бьёт" |

Each new/re-anchored gate is flip-RED'd as listed in its step; the implementer records the perturbed input per gate.
Declined gates (owner run): loudness balance, timbre of shots/sirens/ambience, shake amount, vignette colour, arc look.

## 4. Rollout notes

- **Assets:** after Step 2, `cargo run` and `local_assets_match_manifest` need the three packs on disk: run
  `python tools/fetch_assets.py` (network) or `--cache` with the scratch zips. Other checkouts/worktrees go RED ("partial
  install" / preflight "missing third-party asset") until they fetch. New pack dirs are git-ignored by
  `/assets/third_party/*` (verified with `git check-ignore -v`).
- **Cargo:** one dev-dependency `rodio =0.22.2`, `default-features = false`; no normal dependency change, `gta_sim` untouched
  in deps. On a future Bevy upgrade, G-A7 decides whether the ear mirror stays.
- **Settings file:** `reduce_camera_motion` is additive; old files load with the default `false`.
- **Sim API:** `BulletTrace` gains a field (all constructors updated in Step 1).
- No feature flags, no migrations, no env vars. No audio device → silent, one-shots bounded by the cap, QA still passes on
  `SoundStats` (record the warning).
- PCTX: rodio spatial lesson appended to `PCTX_PROPOSALS.md` (fold after landing).

## 5. Review notes (changes vs PLAN_V2 / PLAN)

1. **Q4 folded in (orchestrator):** death sting = `jingles_SAX01` (music-jingles has no file named "lose"; ranked all 102
   jingles by energy-weighted pitch contour, `scratch/pr2/jingle_contour.py/.txt`; SAX01 falls 0.81 oct over 0.89 s,
   mellow timbre = "глухой"), played `OnEnter(GameState::Wasted)`. Player hurt = non-spatial `hurt` pool reusing the five
   `impactPunch_heavy` files (the only heavy body variants besides `impactSoft_heavy`, which is the NPC death thud).
   Manifest counts: impact-sounds **26** (unchanged), interface-sounds **3**, music-jingles **3** (was 2). New validated
   snippet `scratch/pr2/manifest_snippet_final.ron`. The old checklist item "content gap: вскрик / глухой sting" is replaced.
2. **New `SoundClass::Hurt` (7 classes, cap 2):** in `Impact` the hurt thud would be stolen within ~80 ms under NPC fire; in
   `Stinger` it would steal the wanted stinger.
3. **Bug in V2 `SoundStats`:** V2 counted `spawned` in `enforce_voice_budget` after trimming, so at most `cap` per frame
   would be seen and G-A3's `spawned == 30·k` could never pass. Counting moves to the single `spawn_sound` helper.
4. **Compile errors V2 would hit:** `SpatialScale` and `PlaybackMode` have no `PartialEq` (`bevy_audio audio.rs:9-11,202-204`):
   G-A4 compares `spatial_scale.map(|s| s.0)` approximately, G-A5 uses `matches!`. `citygen::dist_point_segment` is not
   reachable from the client (no `citygen` dependency): Step 1 re-exports it from `gta_sim::world`.
5. **G-J1 self-match:** V2 built only the token literals with `concat!`; its self-test input strings (`"mut t: ResMut<Time<Virtual>>"`)
   would make the scanner flag its own file. All such strings are split with `concat!`. Added path-qualified
   `ResMut<bevy::time::Time<Virtual>>` to the normaliser.
6. **Dev-dependency features:** made `default-features = false` load-bearing (default rodio switches the test decoder to
   symphonia and G-A2 would stop testing production's lewton path).
7. **Death row geometry:** uses `DamageDealt.point` instead of the target's `Transform` (same place, no missing-transform
   branch, matches the death sound position).
8. **QA robustness:** star-pulse check moved to reflected `StarPulse.left` (scale > 1.05 lasts ~0.09 s, below BRP polling);
   Shot delta is `≥ 6` + magazine == −6 (NPC gunfire can add); wanted reset to 0 before the stinger check; new death phase;
   `send_keys` takes `ms` (`brp.py:135`).
9. **G-A7 strengthened:** goes through the camera transform like `play_queued_audio_system`, adds a yaw-90° row.
10. **Restored from PLAN.md** where V2 compressed: full `mix.ron` with written-out pools, synth formulas, decoder structure,
    the `_ => continue` trap in the settings toggle, plugin deletion list, BRP registrations, QA helper list.
11. Kept from V2 unchanged: R1 mirror (re-verified, §0), R2 volume 0.4, R3 `GlobalVolume` in harness, R5 `sync_loop_pause`,
    R6 re-volume on `GlobalVolume`, R7 `siren.height` data, R9 panic scan, R11 stacked rows, R12 stand-ins, R13.

children: 0 launched / 0 reported.
