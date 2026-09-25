# FIX_SUMMARY — TASK-017 (GDD T16), fixer round 1

Input: `IMPL_REVIEW.md` (NEEDS_WORK) plus the orchestrator note (7 items). Tree: `feature/t16-final`.
Evidence: `scratch/fix/` (flip script and outputs, sweep log, runs).

Preflight claim checked first: Issue 1's prescription "add `&mut Loadout` to `enter_exit`'s player query". Risk: a
query conflict or a player fixture without `Loadout` dropping out of the system. Checked `seat.rs:209-300`: `vehicles`
is `Without<Player>`, and nothing else in the system touches `Loadout`, so there is no conflict. I used
`Option<&mut Loadout>` so a player without a loadout still enters and exits cars.

## 1. Fixed

1. **Issue 1 (MAJOR), a queued press fires from the seat.** Confirmed with the reviewer's probe logic. `seat::enter_exit`
   now clears `Loadout.fire_queued` next to the `ActionIntent` reset. It clears only when the flag is true, so there are
   no spurious change ticks. New gate `vehicle_seat::a_queued_press_does_not_fire_from_the_seat`: a pistol shot at
   tick 0, a press on the last tick whose cooldown is above 2·dt (derived; checked to lie inside the buffer window),
   F on the next tick, 32 ticks seated, then F again and 32 ticks on foot. It asserts 0 shots and 0 own-car hits both
   seated and on the way out, full `VehicleHealth`, and the magazine down by exactly 1.
   Flip: remove the clear → RED `left: (1, 1) right: (0, 0)` ("the queued press fired from the seat").
2. **Missing clear gates (orchestrator item 2).**
   - Stagger: `shooting::a_queued_press_is_dropped_on_a_stagger` (a 6·dt stagger one tick after the queue). Flip: remove
     the clear in `fire_weapons`' `is_active` branch → RED, "fired at [23]".
   - Reload: `shooting::a_queued_press_is_dropped_on_a_reload` (R one tick after the queue; the window is the reload
     plus 24 ticks, derived). Flip: remove the clear in the `reload_left > 0` branch → RED, "fired at [95]".
   - Busted: `respawn.rs` now runs one helper in two rows, `a_queued_press_does_not_fire_after_respawn` (Wasted, as
     before) and `a_queued_press_does_not_fire_after_an_arrest` (new). After the respawn the pistol is handed back,
     because an arrest confiscates it, so a surviving queue has a gun to fire.
     Flip Busted: keep `fire_queued` across the confiscation in `busted.rs` → RED, 2 shots vs 1.
     Flip Wasted: replace `reset_fire_queue` on `OnExit(Wasted)` with a no-op → RED, 2 shots vs 1. The refactored row
     still bites.
   - **Registrations removed rather than gated** (decision, logged): I removed `reset_fire_queue` on `OnExit(Busted)`
     and on `NEW_CITY`. No gate can observe them. `busted::respawn_at_station` does `*loadout = Loadout::default()`,
     and a new city despawns every `Loadout` holder: `Character` requires `CityScoped`, and `despawn_city` runs on
     `NEW_CITY`. Removing either registration alone stays GREEN, which is the case now. The Busted property is gated
     by the row above. For NEW_CITY no gate is possible, because the entity no longer exists.
     `combat/mod.rs` now matches `main` on those two lines; the `reset_fire_queue` doc says why only Wasted needs it.
3. **Issue 3, the police overshoot invariant.** New `EscalationConfig::validate_overshoot(&WeaponsConfig)`: it requires
   `combat.overshoot_margin` ≥ the longest range among the patrol and SWAT guns. It is chained in `compose_sim` after
   `validate_ring` (the same cross-config pattern). The shipped check was added to
   `shipped_police_config_loads_and_validates`. New fixture
   `config_police::police_overshoot_covers_the_longest_police_gun`: margin 60 → 30 (strictly below the pistol's 60), and
   the error must contain "longest police gun range". Flip: comparator disabled (`&& false`) → RED (the sabotaged
   config validates).
4. **Nit, the dead "floor of 2" in the stray bound.** `(2 * STRAY_MEASURED).max(2)` → `2 * STRAY_MEASURED`, and the doc
   comment was fixed. Behaviour is identical (STRAY_MEASURED = 4). Not fixed, only noted: "west pair + groupmate
   east" still sits exactly at its bound (4/80 vs floor(5 %) = 4). The Q-B bound is binding, and I did not retune it.
5. **Issue 5, t9 ignores overreach.** `t9.py crossfire_geometry()` now adds `overreach` = |flat `muzzle_offset`|
   (`aim.ron`) + max(`capsule_radius`, `head_radius`) (`locomotion.ron`), the same as `tactics::overreach` and
   `guarded_zone_params`. The key was renamed `overshoot` → `guarded`. Computed from shipped data: 8.8648 m (the plan
   says 8.865). No runtime run of t9 in this round (not requested). Stickiness of `crossfire_seen` was left as is.
6. **The 50 ms frame (run 2).** Unexplained. Session A has no trace, `BenchFrames.frame_ms` is summarised without
   timestamps, `log_errors_a` is empty, and the game log of that run is overwritten. The smoothed diagnostics max in
   that window was 6.2 ms, so it was one isolated frame. It is recorded as unexplained, a single spike, not a budget
   breach.
7. **Perf verdict wording and stuck share by window.**
   - `t16.py`: `car_speed_mps` and `car_stuck_share` are now split `{composition, measured}`. The verdict text is
     prefixed with the measured scene, for example "at the measured scene (17 traffic cars, 0 gang members near the
     centre, 5 stars, 5 police cars, 4 units, car standing 34% of the measured window): nothing to fix by trace: every
     budget holds".
   - **Finding while re-deriving the old runs**: I split the stored `car_speed_mps` of the implementer's 3 runs (the
     last 29-30 samples are the measured window). The car stood still for **100 %** of the measured window in every
     run. The shares were 0.05-0.17 during the composition wait. The reviewer's "car moving ~55 %" was an artefact of
     mixing the two windows. The old verdict therefore held for a standing car. In the fixer run the car drove for
     66 % of the window (see test results), and the budgets still hold with margin. The bench car getting boxed in
     after the police arrive is still the TASK-032 class. It is phase-dependent, so a standing window can recur.
   - `IMPL_SUMMARY.md` §Step 5 verdict paragraph was rewritten with this qualification and marked "corrected by the
     fixer" (the orchestrator asked for it). README has no verdict text, only the run command, so it was not changed.

## 2. Skipped

- **Issue 4 (queued NPC shotgun shot skips the fire-line re-check).** The reviewer marked it "no action required".
  It is plan risk R4, and the fire-line gates are green. Recorded as an accepted behaviour change: gang shotgun pulls in
  the last 0.15 s of the 0.9 s cooldown fire up to 0.15 s late, without a fresh hold-fire check. The orchestrator did
  not ask for a change.
- **SMG tap inside the 0.08 s cooldown now buffers one shot** (missing coverage, no gate either way). This is the
  intended buffer behaviour for any gun. Not gated, because the orchestrator did not list it.
- **Nits**: the `tactics/mod.rs:169-171` doc re-wrap, file length (`shooting.rs` is now 931 lines, still < 950; the
  new gates were kept compact), and the P8 label in `t16_s1.py`. None change behaviour; left for QA to note.
- **`--bench-scene` keeping the car moving** is out of scope (a new finding, not in the review). Reported above for
  the orchestrator.

## 3. Test results

Every cargo command ran alone with `-j 2`.

- `cargo clippy --workspace --all-targets -j 2 -- -D warnings` → `Finished`, 0 warnings.
- `cargo test -p gta_sim -j 2` → 58/58 test binaries `ok`, exit 0 (`scratch/fix/sweep_gta_sim.txt`). Touched
  binaries: config_police 11, vehicle_seat 20, shooting 21, respawn 7, gang_fire_lines green.
- `cargo test -p gta_like --bin gta_like -j 2` ×3 → 81 passed ×3 (`scratch/fix/client_gates_x3.txt`).
- Flip-RED (all restored, then GREEN): `scratch/fix/flips.txt`, `scratch/fix/flip.py` (seat, stagger, reload, busted,
  wasted, police).
- `rustfmt --edition 2024` on the edited files only; `git diff --stat` shows no foreign files.
- Runtime, release `--features dev`:
  - `python tools/qa/scenarios/t13.py --out target/qa/fix/t13` → exit 0, 6 clicks, magazine 12 → 6, shot_delta 6.
  - `python tools/qa/scenarios/t16.py --out target/qa/fix/t16` → exit 0, no log errors in A or B. 1920×1080; mean
    319 FPS, 1 % low 162, min 116, p50/p99/max 2.84/5.18/8.63 ms. FixedMain 2.49 ms/tick (p99 2.85), AI
    0.77 ms/tick, physics 0.71 ms/tick. Scene at the end of the window: 17 traffic cars, 0 gang, 5★, 5 police cars,
    4 units (12 during composition). Stuck share: composition 0.26, measured 0.34. Verdict: nothing to fix by trace.
- No game process is left running.

children: 0 launched / 0 reported.
