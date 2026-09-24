# PCTX proposals — TASK-014 (planner)

## 2026-09-24 — bevy-ecs domain, risk lesson candidate (audio)

bevy_audio 0.19.1 `PlaybackMode::Loop` wraps the decoder in rodio 0.22.2 `repeat_infinite`, and rodio
`Repeat` keeps a `Buffered` clone of the source start (`rodio-0.22.2/src/source/repeat.rs:9-17`): every
sample ever played stays in memory. An endless procedural decoder (`Decodable` whose iterator never ends)
played with `Loop` grows memory without bound and nothing fails. Rule: endless decoders use
`PlaybackMode::Once`; `Loop` only for finite clips. Also: `AudioSinkPlayback::set_volume` replaces the
sink volume, `GlobalVolume` is applied only when the sink is created (`audio_output.rs`), so runtime volume
changes multiply the global volume themselves. Trigger: `PlaybackSettings::LOOP` / `set_volume(`.
Fold only after TASK-014 lands and the gate exists.

## 2026-09-24 — bevy-ecs domain, risk lesson candidate (plan-reviewer-2, rodio spatial)

bevy_audio 0.19.1 passes the listener's left/right ears straight into rodio 0.22.2 `Spatial`, whose
`set_positions` (`rodio-0.22.2/src/source/spatial.rs:57-60`) gives channel 0 (left) MORE gain when the
source is nearer the RIGHT ear (fixed upstream on master by swapping the two expressions). A default
`SpatialListener::new(gap)` therefore pans every spatial sound to the wrong side, silently. Also
`ChannelVolume::next` sums stereo input as L+R (the `/ num_channels` result is discarded): stereo files on
spatial emitters are up to 2x louder. Rule: the listener is built with mirrored ear offsets behind one
function and gated by a rodio-level pan test that goes RED when a Bevy upgrade brings a fixed rodio.
Trigger: `SpatialListener`. Fold after TASK-014 lands with gate G-A7.

## 2026-09-24 — gates domain, risk lesson candidate (implementer, BRP and pause)

`bevy_brp_extras` 0.22.6 releases held keys and mouse buttons on `Res<Time>` in `Update`
(`keyboard/keys.rs:149-155`, `mouse/button.rs:114-120`), which is `Time<Virtual>`: while `GameState::Paused`
a hold never releases, so Escape cannot unpause over BRP. `NextState<GameState>` is not registered for
reflection either, so BRP cannot set it. A runtime scenario that pauses either ends in the pause (t13) or
needs a named registration in `QaRemotePlugin`. Trigger: `send_keys(["Escape"]` in `tools/qa/scenarios/`.
Also: `CityLandmarks.park_center` is on the SMG pickup of the park range; a teleport there arms the player.

## 2026-09-24 — fixer (TASK-014 review M1) — domain bevy-ecs, risk lesson
Respawn in this project keeps the SAME player entity (`flow/wasted.rs respawn_at`) and resets `Health::full`
(armour 0). A presentation detector that diffs a component across frames per entity ("pool fell → hurt") fires
falsely on a Busted respawn with armour. Frame-diff detectors compare only inside `GameState::Playing` and forget
their baseline in every other state (`juice::detect_player_hurt`, gate `busted_respawn_is_not_a_hurt`).
Trigger: `Local<Option<(Entity, f32)>>` or any `Local` baseline over `Health` / `WantedLevel` in `src/`.

## 2026-09-24 — QA (TASK-014) — domain bevy-ecs, risk lesson candidate
A presentation spawner that reads N messages per frame and "refreshes an existing entity, else spawns one" cannot see
its own `commands.spawn` from the same run: its `Query` only sees entities that existed before the system ran. One
shotgun blast (10 `DamageDealt`, same shooter) spawned 8 overlapping `DamageArc`s (`juice/damage_arc.rs
spawn_or_refresh_arcs`, QA probe `scratch/qa/qa_probe_shotgun_blast_arcs.rs`). Rule: dedupe by key inside the run
(a local set of keys spawned this frame) before spawning. Trigger: `.iter_mut().find(` followed by `commands.spawn(`
in one `MessageReader` loop.

## 2026-09-24 (fixer round 2) — gates: judging a decaying screen effect from one screenshot
A "not visible" verdict on a short-lived effect (hurt vignette, decays in ~0.4 s) from a single QA screenshot mixes
capture latency, decay and scene colour: the same 0.24 intensity gave +55 edge redness on a grey street and looked
faint on bright park grass. Measure such effects as a pixel delta (edge band vs a no-hit frame in the same pose) in
two scenes, and read the intensity at the moment of capture. Domain: gates (risk lesson).
