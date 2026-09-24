# FIX_SUMMARY — TASK-014 (GDD T13), fixer round 2

Fixer: claude opus, medium. Base commit `49f8f5b`. Every cargo command used `-j 4`. The game always ran through
`tools/qa/brp.py` `Game`, which passes the QA `--settings-id`. Inputs: the orchestrator note (4 items), QA_REPORT.md,
FIX_SUMMARY.prev-1.md, the last OPEN_DECISIONS.md entry (binding). IMPL_REVIEW.md was handled in round 1.

## 0. Preflight

- I read `scratch/` only as a coverage map: `fix/`, `qa/` (with `qa_probe_shotgun_blast_arcs.rs`), `pr2/`, `kenney/`.
  I did not re-run any author script. My runners and probes are in `scratch/fix2/`.
- **The claim most likely to mislead if applied verbatim:** QA §6, "at intensity 0.34 no red edge is visible; maybe
  `per_hurt 0.35` / `radius 0.9` are too small". I checked the shader (`bevy_post_process-0.19.1/src/effect_stack/vignette.wgsl`:
  `mix(color, vignette_color, (1 - (1 - (d/r)^2)^s) * intensity)`) and measured it at runtime (`scratch/fix2/vignette_probe.py`).
  With the OLD values, on the street at spawn, intensity 0.24 gives **+55.6** edge redness (R − (G+B)/2, 0..255) over the
  no-hit frame, and the red is plain to see (`vignette_before/single_hit.png`). So "invisible at 0.34" is not true in
  general. The real problems: one hit is gone in 0.44 s (decay 0.8/s), so a capture that lands late sees almost nothing,
  and on bright park grass red blends to brown. Under sustained fire (a hit every 0.37 s) the level only reached
  0.04 → 0.23 and never came near the cap 0.6. Raising only `per_hurt` or `radius` would not fix the sustained case. The
  fix below changes the decay and the cap as well.
- The QA B1 prescription (a per-call `HashSet` of shooters) is correct as written. I checked it against the code: the
  refresh branch `arcs.iter_mut().find(..)` cannot see arcs spawned in the same call.

## 1. Fixed

| Item | What was done |
|---|---|
| **1. B1: one damage arc per shooter per call** | `src/juice/damage_arc.rs` `spawn_or_refresh_arcs`: a local `HashSet<Entity>` of shooters that got an arc in this call. A second pellet from the same shooter is skipped (the new arc already has `left = seconds`). New gate `juice::feedback_gate::one_arc_per_shooter_per_blast`: 8 pellets from a shotgun stand-in and 1 from a pistol stand-in in one update must give exactly one arc per shooter, `[shotgun, pistol]`. QA's probe gave 8 on the old code. |
| **2. Vignette strength** | `assets/juice/juice.ron` vignette: `per_hurt 0.35→0.45`, `max 0.6→0.65`, `decay_per_s 0.8→0.45`, `radius 0.9→0.85`, `smoothness 3.0→2.0`. One hit now stays readable for ~1 s. Lower smoothness keeps the tint at the edges and out of the centre. Under ~2.7 hits/s the level reaches the cap 0.65 on the second hit and holds a floor of about 0.47 between hits. "No flashes" still forces 0: t13 hurt phase `no_flashes_max_vignette = 0.0`, and the headless G-J2 row is still green. Numbers are in §3. |
| **3. Hurt cue spam** | `mix.ron` `hurt.min_interval: 0.5` (real s), `HurtMix.min_interval`, validated `positive`. `src/audio/cues.rs` `play_hurt` keeps the `Time<Real>` time of the last cue in a `Local` and skips cues inside the interval. `PlayerHurt` itself is not throttled, so the vignette and trauma still add on every hit. New gate `audio::event_gate::hurt_cue_keeps_its_min_interval`: a `PlayerHurt` on each of 60 updates (1/64 s each). The rows are worked out ahead: cues at update 0 and update 32 (0.5 s); update 64 would be past the end. The expected `SoundStats.spawned[Hurt]` delta is **2**, and the boundary update 32 counts as 2 whether the check is `<` or `<=`. After 40 quiet updates (0.625 s), one hit must give +1 at once. A GATE BROKEN assert pins the 0.5 s / 64 Hz premise. |
| **4. Siren hysteresis** | `mix.ron` `siren.switch_margin: 10.0` (m), `SirenConfig.switch_margin`, validated `positive`. `src/audio/loops.rs` `pick_sirens(listener, cops, carried, max, audible, margin)`: cops outside `audible` are dropped. Current carriers that are live and in range keep their siren. Free slots go to the nearest others (ties by entity bits). A carrier yields only to a cop closer to the listener by more than `margin`, with the farthest carrier replaced first. `update_sirens` passes the current siren parents as `carried`. A cop that keeps its siren keeps the same entity, so its sweep is not restarted. This was already true for kept cops, and it is now gated. Gates: (a) `audio::gate::pure_tables` gets 4 hysteresis rows: hold against 25 vs 30 m, yield to 10 vs 30 m, a carrier beyond `audible` is dropped, a free slot is filled, and the old rows get `carried = []` with results unchanged. (b) New system gate `audio::event_gate::sirens_hold_their_cop` through the production `update_sirens`: cops a at 2 m, b at 14 m, c at 30 m, so sirens go on a and b. c moves to 8 m (6 m closer than b): after a full repick the `(cop, siren entity)` pairs are identical. c moves to 3 m (11 m closer): b's siren moves to c, and a keeps the same siren entity. |

### Flip-RED (runner `scratch/fix2/flips.py`, output `scratch/fix2/flips.out.txt`)
Each flip changes one line, runs the named gate with `--exact`, restores the file byte for byte and checks sha256.

| Flip (the input perturbed) | Gate | Result |
|---|---|---|
| Arc dedupe off (`!spawned.insert(..) && false`) | `one_arc_per_shooter_per_blast` | RED, `feedback_gate.rs:452` (one arc per shooter) |
| Hurt interval ignored (`false && last.is_some_and(..)`) | `hurt_cue_keeps_its_min_interval` | RED, `event_gate.rs:76` (cues under 60 hits) |
| Margin ignored (`challenger.0 >= farthest.0`) | `pure_tables` | RED, `gate.rs:502` (hold row) |
| same | `sirens_hold_their_cop` | RED, `event_gate.rs:282` (siren left its cop for one 6 m closer) |
| `update_sirens` passes `&[]` as carriers | `sirens_hold_their_cop` | RED, same line |
| Carriers never recognised (`partition(.. && false)`) | `sirens_hold_their_cop` | RED, same line |

All 6 flips went RED, and each file was restored and verified (sha256). The full suites below ran after them.

## 2. Skipped / deviations

- **Vignette "edge-luminance delta" (orchestrator wording):** a red tint barely moves luminance. It darkens bright
  grass (−3.4) and brightens a grey street (+8), so luminance alone does not show the effect. I report both luminance and
  redness (R − (G+B)/2). Redness is the metric that carries the claim.
- **No headless gate for the vignette tuning:** the look is owner-class (GDD feel), so it has runtime evidence plus the
  owner checklist and no new machinery. The mechanism (per-hit add, cap, decay, no_flashes) is unchanged and still covered by G-J2.
- **t13.py:** the first run went `GATE BROKEN: six clicks did not fire six shots` (magazine 12→7, one of six OS clicks
  was not registered). That is input plumbing, not the code under test, because the gun phase does not touch anything I
  changed. The second run passed (exit 0). Both are kept: `scratch/fix2/t13_run1_gate_broken{,.log}`, `scratch/fix2/t13{,.log}`.
- Not changed (not asked): the nits from round 1 (`add_message::<StarsRaised>` in three places, the `MixConfig` clone in
  `SoundBank::from_world`), and the t9 flake (OPEN_DECISIONS: a note for T16).

## 3. Test results

**Runtime vignette probe** (`python scratch/fix2/vignette_probe.py <label> [street|park]`, release `--features dev`,
seed 1, armour 1e6. It takes a no-hit frame, one hit about 0.1 s before capture, and 8 hits 0.37 s apart. Edge band =
left and right 6 % strips, rows 25-70 %, clear of the HUD. Deltas are against the no-hit frame in the same pose.
Output is in `scratch/fix2/vignette_<label>/` as PNG plus `result.json`):

| Scene / config | Single hit: intensity at capture, edge redness Δ, edge luminance Δ | Sustained: levels (0.37 s after each hit), redness Δ, luminance Δ |
|---|---|---|
| Street, old values | 0.24, **+55.6**, +6.9 | 0.04 → 0.23 (never near the cap 0.6), +50.3, +6.4 |
| Street, new values | 0.38, **+70.6**, +8.0 | ~0.43-0.48 floor, cap 0.65 hit, **+79.9**, +8.6 |
| Park grass, old values | 0.26, **+40.4**, −2.0 | ≤ 0.15 at capture, +32.0, −1.5 |
| Park grass, new values | 0.40, **+50.8**, −3.4 | 0.27, then 0.47-0.48 held, **+59.1**, −4.7 |

The centre square (middle 20 %) rises by only +1.4 to +4.0 redness in every run, so the tint stays at the edges. Screenshots for the owner:
`vignette_after_park/single_hit.png` (one hit: red sky edges, brown grass edges), `vignette_after/sustained.png`
(saturated under fire: strong red frame, clear centre). The "before" images are next to them.

**t13.py** (`python tools/qa/scenarios/t13.py --out maw/tasks/in_progress/TASK-014/scratch/fix2/t13`): run 2 exit 0.
listener mirrored (−0.15), 2 ambience beds, park 0.35 / city 0.125, melee impacts +3, guns shot +6 / magazine 12→6 /
impacts +7 / ground +1, **hurt max vignette 0.444** (was 0.34), trauma 0.166, hurt +1, no_flashes 0.0, arc 0.70238 vs
0.70238, stinger +1, star pulse live, 1 siren on a live cop, death sting +1, pause sound +1. Peaks are within caps,
there are no panics, and no "No audio device" warning. Frame cost without vsync is 2.7-3.5 ms (144 Hz Fifo host).
`tasklist` shows no `gta_like` left.

**Cargo gates:**
- `cargo build -j 4`: OK.
- `cargo clippy -j 4 -- -D warnings`, `cargo clippy -j 4 --features dev -- -D warnings`,
  `cargo clippy -j 4 -p gta_like --tests -- -D warnings`: clean. The first test-clippy run caught an unused `mut` in my
  new gate, which I fixed.
- `cargo test -j 4 -p gta_like --bin gta_like`, 3 runs: `test result: ok. 71 passed; 0 failed` each time (68 before, plus 3 new gates).
- `cargo test -j 4 -p gta_sim`: every binary `ok`, 0 failed. `cargo test -j 4 -p citygen`: ok (untouched).
- `cargo tree -p gta_sim -e normal -i bevy_render`: nothing to print. `python tools/qa/tree_check.py`: passed.
  `python tools/qa/font_check.py`: 0 missing glyphs. `python tools/fetch_assets.py --check`: packs match.
- Formatting: `rustfmt --edition 2024 --config skip_children=true` on the 7 edited `.rs` files only.
- File sizes: `gate.rs` 595, `feedback_gate.rs` 474, `event_gate.rs` 303, `loops.rs` 304 (all < 750).

## 4. Files touched

`assets/audio/mix.ron`, `assets/juice/juice.ron`, `src/audio/{config,cues,loops,gate,event_gate}.rs`,
`src/juice/{damage_arc,feedback_gate}.rs`. Task dir: `FIX_SUMMARY.md`, `log.jsonl` (+3 `decision`),
`PCTX_PROPOSALS.md` (+1 gates lesson), `scratch/fix2/` (flip runner and output, vignette probe and results, t13 runs).
`git status --short` shows nothing outside these.

## 5. Owner checklist (updates to QA §6)

- [ ] Vignette: one hit gives a red frame that lasts about 1 s. Under fire it stays saturated (cap 0.65). Too strong or
      too weak? The knobs are `juice.ron vignette.per_hurt/max/decay_per_s`. On bright grass it reads more brown than red.
- [ ] Hurt thud: at most one per 0.5 s (`mix.ron hurt.min_interval`). Check that it still reads as "меня бьют" and no longer spams.
- [ ] Sirens: at 5 stars a siren now stays on its cop until the cop dies, leaves 150 m, or another cop is 10 m closer
      (`siren.switch_margin`). There should be fewer jumps and restarts of the wail.
- [ ] Gang shotgun at the player: one arc per shooter, fading smoothly (B1).

children: 0 launched / 0 reported.
