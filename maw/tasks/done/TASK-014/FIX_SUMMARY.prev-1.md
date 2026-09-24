# FIX_SUMMARY — TASK-014 (GDD T13): fixes after IMPL_REVIEW

Fixer: claude opus, medium. Base commit `74c9f2f`. All cargo commands used `-j 4`.

## 0. Preflight

- `scratch/` was read only as a coverage map (the author's `flip_gates.py` and its output, the Kenney probes). I did
  not rerun the author's script. My flips are in my own runner, `scratch/fix/fix_flips.py`.
- **The claim most likely to break correct code if applied verbatim:** m2's prescription, "key on `(shooter, body)`
  plus the `ShotFired.attack` id if the trace carried it". `BulletTrace` has no attack id
  (`hitscan.rs:74-82` on the base commit), so applied as written it does not compile. The orchestrator's version,
  "dedupe per fixed tick", cannot be done from `Update` at all: two ticks' messages are indistinguishable there. So
  the diagnosis is right and the prescription had to be recomputed (see m2).
- I checked M1 against the code, and it is real. `respawn_at` (`flow/wasted.rs:117-130`) teleports the same player
  entity and sets `Health::full` (armour 0, `character/health.rs:93-99`). It runs in `OnExit(GameState::Busted)`
  (`flow/mod.rs:139-142`), so on the first Playing frame the pool drops from 150 to 100 and the old code fired
  `PlayerHurt`. The new gate reproduces this (flip rows below).

## 1. Fixed

| Review item | What was done |
|---|---|
| **M1** false `PlayerHurt` on a Busted respawn with armour | `src/juice/mod.rs` `detect_player_hurt` now takes `Res<State<GameState>>`. Outside `Playing` it sets `*last = None` and returns. The doc comment is rewritten, because the old premise ("a new player entity never fires") was false. New gate `juice::feedback_gate::busted_respawn_is_not_a_hurt`: player armour 50, `Busted -> Playing` through the real `OnExit(Busted)` respawn. It asserts armour is 0 (GATE BROKEN otherwise), 0 `PlayerHurt` messages (read through a `MessageCursor`), trauma ≤ 1e-4 and vignette 0. A later `DebugDamage{10}` must still give exactly 1 hurt, which is the liveness check. |
| **m1** a wanted stinger can steal the death sting | New `SoundClass::DeathSting`, placed after `Stinger` (COUNT is now 8, and it is in `ONE_SHOTS`). Its cap is data: `mix.ron voices.death_sting: 1`, `Voices.death_sting`, validated `>= 1`. `play_death_sting` spawns this class. `t13.py`: `CLASSES` gains `DeathSting`, the caps regex reads `death_sting`, and the death phase counts `DeathSting`. New gate `audio::event_gate::stingers_keep_the_death_sting`: `StarsRaised` gives exactly one Stinger with the `wanted` handle. Then the player is killed, `Wasted` is entered, and a `StarsRaised` lands in the next frame. The death sting must still be alive, with the death handle. |
| **m2** impact de-dup was per frame | The diagnosis is kept and the prescription recomputed. The sim `BulletTrace` gains `attack: u32` (= `ShotFired.attack`, written in both branches of `fire_weapons`). `play_impacts` now de-dups on `(attack, hit a body)`. One attack's pellets are always written in one tick, so this is exact per blast and never merges two ticks. There is no FixedUpdate presentation code. Constructors are updated in `tests/new_city.rs` and `audio/gate.rs`. New sim row in `shooting.rs::shotgun_fires_ten_pellets`: every pellet trace carries the blast's attack id. New gate `audio::event_gate::pellets_share_one_impact`: 8 Body + 2 World pellets of attack 50 and 8 Body pellets of attack 51 from the same shooter in one update give exactly 3 impacts. |
| **m3** sirens are phase-locked | `SirenSynth` gains `start` (seconds into the sweep, used as the decoder's initial `t`). `SoundBank.sirens` holds `max_emitters` synths starting at `period·i/max_emitters`, and `SoundBank::next_siren()` hands them out round-robin. There is no new tuning number. It is covered by the siren gate below (the first 4410 samples of the two sirens differ). |
| **Missing coverage:** `update_sirens` | New gate `audio::event_gate::sirens_ride_live_cops`: a listener, heat 180 (2 stars, GATE BROKEN otherwise), stand-in `PoliceUnit` cops: Dead at 2 m, live at 4, 6 and 8 m. Within ≤ 80 updates (one repick) it expects exactly 2 sirens, parented to the 4 m and 6 m cops, at local `(0, height, 0)`. The Dead cop must still be Dead (GATE BROKEN otherwise) and carry no siren. At heat 0, all sirens are gone after 2 updates. |
| **Missing coverage:** `detect_stars_raised` / `play_stinger` | New gate `juice::feedback_gate::stars_raised_once_per_rise` over heat rows 180/180/40/180/0 (stars 2/2/1/2/0). It expects `StarsRaised` counts 1/0/0/1/0, and every row checks the stars value (GATE BROKEN otherwise). `play_stinger` is covered by the first row of `stingers_keep_the_death_sting`. |
| **Missing coverage:** pellet de-dup | See m2. |
| File size | Keeping the new audio gates in `gate.rs` would have taken it to 750 lines. They are in a new file `src/audio/event_gate.rs` (197 lines), which reuses the `gate.rs` helpers made `pub(super)` (`mix`, `audio_app`, `player_at`, `sounds`, `new_sounds`, `entity_set`, `stand_in`, `SoundRow`). `gate.rs` is now 575 lines, `feedback_gate.rs` 433. |

### Flip-RED (my runner `scratch/fix/fix_flips.py`, output `scratch/fix/fix_flips.out.txt`; each file restored byte-for-byte)

| Flip (the input perturbed) | Gate | Result |
|---|---|---|
| Remove the Playing gate from `detect_player_hurt` | `busted_respawn_is_not_a_hurt` | RED at "the respawn fired PlayerHurt" |
| Gate only `Paused`, compare in Busted (no reset) | same | RED at the same line |
| `stars > last` → `stars != last` | `stars_raised_once_per_rise` | RED, row heat 40 |
| `stars > last` → `stars >= last && stars > 0` | same | RED, row heat 180 (hold) |
| Death sting spawned as `SoundClass::Stinger` | `stingers_keep_the_death_sting` | RED, "the death sting survives a wanted stinger" |
| `play_stinger` plays the death handle | same | RED, first-row handle mismatch |
| De-dup key `(0, body)` (per frame, the old behaviour for one shooter) | `pellets_share_one_impact` | RED |
| No de-dup (`|| true`) | same | RED |
| Siren filter keeps `Dead` cops | `sirens_ride_live_cops` | RED, the dead cop carries a siren |
| No despawn at 0 stars | same | RED, "sirens outlive the wanted level" |
| Siren start forced to 0 | same | RED, "two sirens wail in unison" |
| `fire_weapons` hit branch writes `attack: 0` | `shooting::shotgun_fires_ten_pellets` | RED, "pellet traces do not carry the blast's attack id" |

All 12 flips went RED and all are GREEN after restore (the runs in §3). `git status` after the runner shows only my
intended edits.

## 2. Skipped / deviations

- **m2 "per fixed tick" (orchestrator) / "(shooter, body) + attack" (review):** replaced by the per-attack key above.
  Reason: from `Update` a per-tick key is impossible without a sim tick stamp or a FixedUpdate presentation system,
  which breaks the domain rule that presentation runs in `Update`. The per-attack key covers the case the reviewer
  worried about (two shots in one frame at low FPS) and is exact. Logged as a `decision`.
- **m3 via `PlaybackSettings.start_position`:** not used. bevy_audio 0.19.1 applies it with rodio `skip_duration`
  (`audio_output.rs:168-176`), and `rodio-0.22.2/src/source/skip.rs` skips eagerly by pulling samples on the main
  thread, up to 216k for a 4.9 s sweep, on every siren spawn. The start-offset synths cost nothing at runtime.
- **M1 nuance:** I followed the prescription as given ("reset in any other state", Paused included). The cost is that
  a hit landing in the very first fixed tick after a resume or respawn is not reported (the baseline is re-seeded
  after it). This is noted, and not worth a second branch.
- **Nits (not asked):** `add_message::<StarsRaised>` is still registered three times (idempotent). `SoundBank::from_world`
  still clones `MixConfig` (startup only). I left both unchanged.
- **Review §4 "punch taken" stacking row:** the reviewer already accepted it as argued by construction. No change.
- **Runtime `t13.py`:** not run by me (a release build plus a window). I changed its `CLASSES`/regex/death phase and
  checked offline that `mix_caps()` parses the new `mix.ron` into 8 caps matching 8 classes
  (`[12, 12, 2, 4, 1, 1, 2, 2]`). The full run belongs to the QA stage.

## 3. Test results

- `cargo build -j 4`: OK.
- `cargo test -p gta_like --bin gta_like -j 4`, run 3 times: `test result: ok. 68 passed; 0 failed` each time
  (it was 63; +5 new gates).
- `cargo test -p gta_sim -j 4`: all binaries `ok`, 0 failed (incl. `shooting` with the new attack-id row).
  `citygen` is untouched and was not rerun.
- `cargo clippy -j 4 -- -D warnings`, `cargo clippy -j 4 -p gta_like --tests -- -D warnings`,
  `cargo clippy -j 4 --features dev -- -D warnings`: all clean.
- `cargo tree -p gta_sim -e normal -i bevy_render`: empty ("nothing to print"). `python tools/qa/tree_check.py`: passed.
- Formatting: `rustfmt --edition 2024` on the edited files only (`--config skip_children=true` for `mod.rs` files);
  the diff contains only intended files.

## 4. Files touched

`crates/gta_sim/src/combat/hitscan.rs`, `crates/gta_sim/tests/{new_city,shooting}.rs`, `assets/audio/mix.ron`,
`src/audio/{config,cues,gate,loops,mod,synth}.rs`, new `src/audio/event_gate.rs`, `src/juice/{mod,feedback_gate}.rs`,
`tools/qa/scenarios/t13.py`. Task dir: `FIX_SUMMARY.md`, `PCTX_PROPOSALS.md` (+1 lesson), `log.jsonl` (+4 decisions),
`scratch/fix/`.

## 5. Owner checklist additions

- Two sirens now start at opposite points of the sweep (0 and 2.45 s of 4.9 s). Listen whether this reads as "two
  cars", or whether a small pitch detune is wanted as well (it would go in `mix.ron`).
- The death sting now always plays whole over "ПОТРАЧЕНО", even if the stars rise in the same frame.

children: 0 launched / 0 reported.
