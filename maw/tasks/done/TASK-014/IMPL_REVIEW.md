# IMPL_REVIEW — TASK-014 (GDD T13): sound and juice

Reviewer: code-reviewer (claude opus, medium). Reviewed commit `ab2bdb7` on `feature/t13-audio-juice`, tree clean.
Inputs read: `TASK_FINAL.md`, `PLAN_FINAL.md`, `IMPL_SUMMARY.md`, `log.jsonl`, every changed Rust file in full,
`mix.ron`, `juice.ron`, the flip runner and its output.

## 0. Disconfirmation

**Counter-example chosen:** the voice budget does nothing in the real game. `enforce_voice_budget` `try_despawn`s the
oldest one-shot, but if despawning an `AudioSink` detaches the rodio player, the sound keeps playing and the "cap" only
holds in the headless gate (where there are no sinks).
**Search:** `bevy_audio-0.19.1` has no `Drop` impl and never calls `detach`. `rodio-0.22.2/src/player.rs:345-353`
`impl Drop for Player` sets `controls.stopped = true` unless `detached`. **Result: the counter-example fails.**
Despawning a voice stops it, so the budget is real at runtime.

I also re-derived the load-bearing R1 claim (mirrored ears). `rodio-0.22.2/src/source/spatial.rs` `set_positions` gives
channel 0 `((left_dist - right_dist)/max_diff + 1)/4 + 0.5`, so a source nearer the right ear gets MORE left gain.
`bevy_audio audio_output.rs:50-69,125-145` passes the ears in order. The mirror is needed and `listener()` applies it.

## 1. Verdict

**NEEDS_WORK.** The slice is complete, clean and well gated. One real logic bug remains: `detect_player_hurt` fires a
spurious hurt (thud, red vignette, +0.2 trauma) on a Busted respawn when the player had armour. The fix is small.

## 2. Confirmed correct

- Builds and gates, re-run by me on the committed tree: `cargo test -p gta_like --bin gta_like -j 4` gave 63 passed
  in 3 runs. G-A2 printed no `SKIP` line, so it decoded the real files. `cargo test -p gta_sim -j 4` is all ok.
  `cargo clippy -j 4 -- -D warnings` and `cargo clippy -p gta_like --tests -- -D warnings` are clean.
- Flip-RED, re-run on the current tree through `scratch/flip_gates.py`: G-A3 "oldest survive", G-A4 "player/NPC
  swapped", G-A4 "death sting plays wanted", G-J2 "no death radius" and G-A7 "unmirrored ears" all go RED. Before and
  after, the sha256 of every tracked `src/` file is identical, and `git status` is clean. The line numbers in the
  implementer's `flip_gates.out.txt` are about 6 to 12 lines off the committed gate files, so that output came from an
  earlier revision of the gates. My re-run closes that gap.
- Sim change: `crates/gta_sim/src/combat/hitscan.rs:249-266`. `TraceHit` is resolved before the trace is written. A
  corpse counts as `World`, as the plan accepts. All three `BulletTrace {` constructors are updated, and both shooting
  rows assert Body and World.
- The one spawn helper `spawn_sound` (`src/audio/cues.rs:161-172`) counts `spawned` at spawn time. This fixes the V2 bug.
- `enforce_voice_budget` (`cues.rs:450-482`) sorts by serial, drains the oldest `len - cap` and only covers one-shot
  classes. `peak_alive` is measured after the trim.
- Loops are endless decoders played `Once` (`loops.rs:120`, `synth.rs:508-515`), so rodio `Repeat` is avoided.
- The spatial listener mirror has an upgrade tripwire (G-A7), and a yaw-90 row runs through the camera transform.
- `update_ambience` applies `GlobalVolume` again after `set_volume` (`loops.rs:164-165`). `revolume_sirens` does the
  same for sirens. `sync_loop_pause` is idempotent.
- Sirens are children of the cops (`loops.rs:212-231`), so a despawned cop takes its siren along (`linked_spawn`). The
  emitter position is updated by bevy's `update_emitter_positions` (`bevy_audio lib.rs:92-93`).
- The vignette uses bevy's `effect_stack::Vignette`, and intensity ≤ 1e-4 skips the pass (`vignette.rs` extract).
  "No flashes" forces it to 0.
- The damage arc's sign is right twice over. The math: `right = (cos yaw, 0, -sin yaw)` matches the 7-row table. The
  UI: `Rot2` in bevy_ui's y-down space turns clockwise on screen. The QA screenshot `target/qa/t13/damage_arc.png`
  shows the arc tilted toward the dummy on the right.
- Data-first: every new tuning value is in `mix.ron` or `juice.ron`, with a strict `deny_unknown_fields` loader and
  validation. The new consts (`SAMPLE_RATE`, `NOISE_SEED`, `DT`) are synth laws. There is no `Time<Virtual>` writer in
  `src/`, and G-J1 is flip-verified.
- The settings toggle's `_ => continue` trap is covered (`screens.rs:305`). Every touched file is under 750 lines.

## 3. Issues

### Major

**M1. A spurious PlayerHurt fires on a Busted respawn with armour.** `src/juice/mod.rs:91-107`
- `detect_player_hurt` compares `current + armor` of the same entity across frames. Its doc comment says "a new player
  entity (respawn, new city) never fires". That premise is false. `flow/wasted.rs:118-132` `respawn_at` "teleports
  the SAME player entity" and sets `*health = Health::full(cfg)`, which is full health with `armor: 0.0`
  (`character/health.rs:93-99`). `busted::respawn_at_station` (`flow/busted.rs:46-70`) uses it.
- Here is a worked case. The player picks up armour 50 and gets busted with `current = 100`. The pool before the
  respawn is 150, and after it is 100. The next `Update` writes `PlayerHurt`, so the player gets a hurt thud, a red
  vignette at 0.35 and +0.2 trauma just as the BUSTED screen hands back control. This happens whenever
  `current + armor > max_health` at the arrest. Wasted is safe, because armour is spent before health, so the pool is 0
  at death.
- Suggested fix: compare only while `GameState::Playing` and reset `*last = None` in every other state (Busted, Wasted,
  Paused, Loading). The first Playing frame after any respawn then re-seeds the pool. Fix the doc comment too. An
  alternative is to key on the sim's own damage messages to the player (`DamageDealt`/`MeleeHit`/`DebugDamage`), but
  the state gate is smaller.
- Missing gate: add a G-J2 row. Give the player armour, drive `Busted -> Playing` through the real flow (or call the
  respawn path), then assert no `PlayerHurt`, trauma not raised, vignette 0. This row is RED on the current code.

### Minor

**m1. The death sting and the wanted stinger share `SoundClass::Stinger` with cap 1.** `cues.rs:432-446`, `mix.ron:4`
- `play_death_sting` runs in `StateTransition` (`OnEnter(Wasted)`) before `Update`. If `StarsRaised` lands in the same
  frame, for example killing a cop raises heat on the tick you die, `play_stinger` spawns a newer Stinger and the
  budget steals the death sting. This is rare, and the owner would hear it.
- Suggested fix: ignore `StarsRaised` while not Playing, or give the death sting its own class or cap.

**m2. The impact de-dup is per frame, not per fixed tick.** `cues.rs:302-323`
- The `heard` set is cleared once per `Update`. When 2 fixed ticks run in one frame (low FPS), two separate shots from
  one shooter collapse into one impact. This is harmless to the ear, but the comment says "a shotgun blast is one thud"
  and the code keys by frame.
- Suggested fix: key on `(shooter, body)` plus the `ShotFired.attack` id if the trace carried it. Otherwise, reword the
  comment.

**m3. Two sirens start phase-locked.** `synth.rs:419-423`
- Every siren decoder starts at `t = 0`, so two cops' wails sweep in unison, and a new siren restarts at the bottom of
  the sweep on each repick. This is owner feel. The next knob would be a per-emitter start offset (`start_position`
  or a seeded phase). Put it on the owner checklist rather than fixing it now.

## 4. Missing coverage

- A respawn row for `detect_player_hurt` (see M1): the same entity healed or armour stripped must not fire.
- `update_sirens` in the headless `audio_app`: with stars > 0, a `SirenEmitter` is spawned as `ChildOf` a live cop,
  never more than `max_emitters`. With stars 0 they are despawned, and none are left on `Dead`/`Leave` cops. Today only
  the pure `pick_sirens` table and the runtime QA cover it. A siren that leaks or is never removed is silent-class and
  cheap to gate headless: cops can spawn in the test area, or use stand-ins with `PoliceUnit`.
- `detect_stars_raised` / `play_stinger`: a headless row where stars go from 0 to 2 gives exactly one Stinger, a
  second tick at the same stars gives no new one, and 2 to 1 to 2 gives one more. Today this is only checked in t13.py.
- The shotgun de-dup in `play_impacts` (8 `BulletTrace`, same shooter and kind, in one update, gives 1 Impact) has no row.
- The G-J2 "punch taken" stacking row (melee + hurt = 0.45) is only argued "by construction". That is acceptable, and
  I note it here.

## 5. Nits

- `src/juice/mod.rs:43-45` / `hud/mod.rs:24` / `audio/cues.rs:214-215` register `add_message::<StarsRaised>` three
  times. This is idempotent and harmless, but one owner (JuicePlugin) plus a comment would be clearer.
- `SoundBank::from_world` clones the whole `MixConfig` to dodge the borrow on `Assets<Synth>`. It is fine at startup.
  `resource_scope` would avoid the clone.
- The rows in `flip_gates.out.txt` cite stale line numbers. For the record, the flips were re-verified on `ab2bdb7`
  (see §2).

children: 0 launched / 0 reported.
