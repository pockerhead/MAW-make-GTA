# IMPL_REVIEW — TASK-017 (GDD T16)

Reviewer: code-reviewer (claude opus, effort high). Tree: `feature/t16-final` @ `02133f8`, clean.
Loaded: `TASK_FINAL.md`, `PLAN_FINAL.md`, `IMPL_SUMMARY.md`, `OPEN_DECISIONS.md`, `PCTX_PROPOSALS.md`, `log.jsonl`,
the full `main...HEAD` diff (33 files) and the scratch evidence under `scratch/impl/`.

## 1. Verdict

**NEEDS_WORK.** A press queued in the new fire buffer survives getting into a car and fires from the driver's
seat into the player's own car (reproduced, see Issue 1). Fixing it takes one line plus one gate. The rest is sound:
the overshoot rule, police bit-identity, bench confinement, `trace.py` and the flip evidence all hold.

### Disconfirmation (done before the review)

Counter-example tested: "a press queued in `Loadout.fire_queued` lives through a transition that must not fire,
and fires later". The obvious candidates are Wasted, Busted, a new city, a weapon switch, a stagger and a reload, and
the implementer covered all of them. Vehicle entry is the one missed. `seat::enter_exit` (`crates/gta_sim/src/vehicle/seat.rs:291-292`)
resets `ActionIntent` with the comment "Held fire or a queued request must not fire from the seat on the enter tick".
It does not touch the new `Loadout.fire_queued`. `fire_weapons` has no `Without<Driving>` filter, and `held` stays
`Some(gun)` while the player drives.

**It held.** Probe `scratch/cr/probe_seat_queue.rs` uses the production headless app, the `common` and
`vehicle_support` fixtures and the crate's own test rlibs, built with rustc into scratch, so no project file was
touched. Steps: pistol shot at tick 0, press at tick 17 (queued), F at tick 18. Output:

```
press tick 17; shots from the seat 1; hits on own car 1; car health 1000 -> 975; magazine 8
assertion `left == right` failed: the queued press fired from the seat  left: 1 right: 0
```

## 2. Confirmed correct

- **Fire buffer core** (`combat/hitscan.rs:186-219`): the queue is set only while `0 < cooldown <= fire_buffer_seconds`
  and a request is present. It is consumed by `mem::take` on the first tick the cooldown is 0, and each tick fires at
  most one shot, because firing sets `cooldown = fire_interval`. So there is no double fire, including SMG
  `fire_held` plus a queued press. The queue is cleared on stagger (`:190-195`), on reload (`:201-204`), on weapon
  switch (`weapons.rs:387`), on `OnExit(Wasted)`, `OnExit(Busted)` and `NEW_CITY` (`combat/mod.rs:74-82`, `:143-150`),
  and only where it is true, so change ticks stay quiet. An empty magazine plus a queued press starts a reload, the
  same as a click. `tick_loadouts` runs right before `fire_weapons` in one `.chain()`, so the window is judged on the
  post-decrement cooldown, which matches the gate's `cooldown_after`.
- **Police behaviour is the same as before** at `overshoot_margin: 60`. Police guns: pistol range 60, SMG 45
  (`weapons.ron`). So `range.min(D + 60) == range` for every D >= 0, and `range + overreach` is the same f32 addition as
  the old `reach` (`fire_line.rs:80`). The car filter keeps the full reach through `reach()` (`:187`). The buffer never
  queues for police either: the trigger minimum is 0.4 s, above the pistol interval of 0.3 s, and the SMG interval is
  0.08 s. No police test file changed. I re-ran `police_fire_lines`, `police_range_edge`, `car_fire_lines`,
  `gang_fire_lines` and `gang_combat`: all green.
- **The overshoot rule is one code path**: `FireLine::blockers` (`fire_line.rs:79-89`). It has a value per faction in
  data, validated `positive` (`gang/mod.rs:315`, `police/mod.rs:365`), and all 4 `FireLine::of` sites pass the value.
  The unit test's 7 rows sit off the boundary. Flip (c), `D -> 0`, went RED in `scratch/impl/flips_step2.txt`.
- **Gates are not vacuous.** The implementer's flips are recorded with the perturbed input and the assertion that
  fired (`flips_step2.txt`, `flips/*`). The guarded-zone comparator reads the shipped file, not the sabotaged resource.
  Switching the side-by-side re-anchor from `moved_max` to `moved_armed` came from a real observation: `moved_max`
  stayed at 2.7-14.5 m under sabotage, while `moved_armed` went RED at 0.00 m. The `semi_auto_vs_automatic` offset is
  now asserted outside the window, not assumed.
- **Bench cheats stay inside `--bench-scene`.** `BenchScenePlugin` is added only when `bench` is set
  (`src/main.rs:295-297`). The main menu is skipped only there, and the window size comes from `render.ron`.
  `BenchFrames` defaults to `recording: false`. `drive` runs every fixed tick before `VehicleSystems::Drive`, which is
  correct against the `Update` input overwrite. `BOARD_GIVE_UP_S` is a QA timeout, not game tuning.
- **`trace.py`**: the bisection lower bound is safe (`ts < target` moves only `lo`). The stack pop on an unmatched `E`
  is correct for LIFO spans on one thread: a span opened before the offset closes only after every span opened inside
  it. Exclusive time removes the runner double count, and `RUNNER` is checked before the physics prefix. `update` is the
  main-app frame span only (`bevy_app-0.19.1/src/sub_app.rs:575-576`), which fits the counts: 1437 frames in 10 s at
  144 Hz. All 5 `test_trace.py` tests pass.
- **Build health**: `cargo clippy --workspace --all-targets -D warnings` and `-p gta_sim -p citygen` are clean. The
  touched test binaries are green (lib 86, config 50, config_police 10, respawn 6, shooting 19, vehicle_seat 19, plus
  the fire-line set above). No `unsafe` in the diff. No new tuning `const`. The new data lives in its §12 files.

## 3. Issues

### Issue 1 — MAJOR — a queued press fires from the car seat and damages the player's car
- Where: `crates/gta_sim/src/vehicle/seat.rs:291-292` (the reset does not cover the new latch) together with
  `crates/gta_sim/src/combat/hitscan.rs:212-219` (`fire_weapons` fires for a `Driving` player).
- What happens: the player clicks in the last 0.15 s of the cooldown, then presses F at a door within that 0.15 s.
  When the cooldown expires, a `ShotFired` goes out from the seat and hits the player's own car (probe: 1 shot,
  1 hit, `VehicleHealth` 1000 -> 975, 1 round spent). This breaks silently: it damages the car, spends ammo, and can
  count as a player shot for the wanted rules. It is exactly the class the existing
  `vehicle_seat::held_fire_does_not_carry_into_the_seat` guards against. That gate cannot see this path because its
  cooldown is 0.
- Fix: in `enter_exit`, clear the latch right next to `*action = ActionIntent::default()`. That needs `&mut Loadout`
  in the player query (or a `Mut<Loadout>` via a separate query). This is the system that already owns "nothing
  carries into the seat". Do NOT fix it only with `Without<Driving>` on `fire_weapons`: the queue would then survive
  the drive and fire on the tick the player exits.
- Gate: add a `vehicle_seat.rs` test along the lines of the probe (tick numbers derived as in `late_press_tick`,
  RED-then-GREEN with the clear removed).

### Issue 2 — MINOR — the owner-facing "nothing to fix by trace" is stated for a scene lighter than §11's worst case
- Where: `IMPL_SUMMARY.md` §3 Step 5, `scratch/impl/t16_table.txt`.
- The numbers support the verdict with about 2x headroom. At 1080p with no vsync: frame p99 at most 7.55 ms against
  16.7 ms; AI at most 0.70 ms/tick against 1.5; physics + AI at most 1.39 ms against 4. But the summary gives the
  composition as "traffic peak 24", while the table shows **15-16 traffic cars at the end of the measured window**
  (peak 24 was reached only during the composition wait). The table also shows 0 gang members (§11 says 12), and the
  car stood still in 44-47 % of the samples. Also, `car_stuck_share` (`t16.py:226-228`) mixes speed samples from the
  composition wait with samples from the measured window, so it is not "~45 % of the measured window". The single
  50.5 ms frame in run 2 (min FPS 20) was not diagnosed; session A has no trace.
- Fix: qualify the verdict in the QA/owner text: "for the measured scene: 5 police cars, 12 SWAT, 40 civilians,
  15-24 traffic cars, 0 gangs, car moving about 55 % of the time". Compute the stuck share over the measured window
  only. Neither blocks the verdict.

### Issue 3 — MINOR — the police "full reach" depends on an unchecked cross-file invariant
- Where: `assets/police/escalation.ron:47`, `police/mod.rs:365`.
- `overshoot_margin: 60` reproduces the old police rule only while every police gun's range is at most 60. That link
  lives in a comment. If someone raises the pistol range in `weapons.ron`, cops silently start firing past bodies
  beyond D + 60.
- Fix: validate at load time (police `overshoot_margin >= max range of the police guns`, the same check wherever both
  configs are available), or add one `config_police` test that loads the shipped files and asserts it.

### Issue 4 — MINOR — a queued NPC shotgun shot skips the fire-line re-check
- Where: `hitscan.rs:212-219` with `gang/behavior.rs:131`.
- A gang shotgun pull that lands at 0.75-0.9 s into its 0.9 s cooldown now fires up to 0.15 s later with no fresh
  hold-fire check, and even if the member has left `Attack` in between. That is about 25 % of shotgun pulls, which
  used to be dropped, so gang shotgun DPS also rises a little. The plan accepted this as R4, and the fire-line gates
  stay green. I record it so the behaviour change is explicit (the rollout notes mention only "semi-auto presses").
- Fix, if wanted: clear `fire_queued` for NPCs whose FSM holds fire on the next tick, or accept it and name it in the
  QA report. No action required for merge.

### Issue 5 — MINOR — the t9 crossfire classification ignores `overreach`
- Where: `tools/qa/scenarios/t9.py:108` (`along > distance + overshoot`).
- The sim guards up to `D + overshoot + overreach` (0.865 m more). So a member standing 8.0-8.865 m past the player,
  still inside the guarded zone, is marked `crossfire_seen`, and any HP it loses is excused. `crossfire_seen` is also
  sticky for the whole fight. The runtime gate is a little more permissive than the rule it reports on.
- Fix: add `overreach`, computed from `aim.ron` and `locomotion.ron` the same way as `guarded_zone_params`.

## 4. Missing coverage

- A queued press followed by entering a vehicle (Issue 1): must not fire from the seat, and must not fire on exit.
- A queued press followed by a stagger (`HitReaction` active): the clear at `hitscan.rs:190-195` has no gate.
- A queued press followed by R (reload): the clear at `:201-204` has no gate.
- The `OnExit(Busted)` and `NEW_CITY` registrations of `reset_fire_queue`: flip (d) covered only `OnExit(Wasted)`.
- An SMG tap (a press with `fire_held` released) inside the 0.08 s cooldown now fires one buffered shot. That is a
  behaviour change with no gate either way.
- The police invariant from Issue 3.

## 5. Nits

- `gang_fire_lines.rs` `assert_strays_within`: `max(2 * STRAY_MEASURED, 2)` is always 8, so the "floor of 2 hits"
  in the doc comment is dead. The effective bound is `min(floor(5 % of shots), 8)`. "west pair + groupmate east"
  passes exactly at its bound (4/80 against 4); any shift in combat RNG consumption turns it RED without a bug.
- `tactics/mod.rs:169-171`: the doc comment was re-wrapped mid-sentence ("and move to" / "clear it").
- `shooting.rs` 880 lines and `gang_fire_lines.rs` 902 lines: over the 750 warning, under the 950 limit (already
  disclosed).
- The P8 "junction yield" in `t16_s1.py` accepts any `waiting` car that later changes segment, which also counts
  queueing behind another car. That is enough for TASK-031 evidence, but its label is stronger than what it asserts.

children: 0 launched / 0 reported.
