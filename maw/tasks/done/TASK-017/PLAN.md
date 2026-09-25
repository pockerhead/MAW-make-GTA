# PLAN — TASK-017 (GDD T16): bench scene, trace-driven performance, bug bash, §1 evidence

Binding inputs: `task.md` + `TASK_FINAL.md` (orchestrator decisions after the premise challenge), GDD §1, §4.1, §6.3,
§10.5, §11, §12, §13 T16. Cost of error, named per item: the fire buffer and the t9 fix change gameplay state that
breaks **silently** (lost shots, friendly fire) → real headless gates with flip-RED; the bench scene and the §1 sweep
are QA instruments whose failure is **seen on the first run** → runtime assertions only, no extra machinery.

---

## 1. Understanding (what exists today)

### 1.1 Firing path (t13 flake = game behaviour)
- `crates/gta_sim/src/combat/hitscan.rs:186-216` `fire_weapons`: `fire_requested` is `mem::take`n every tick
  (`:188`, comment "A request during cooldown or reload is dropped, not buffered"); semi-auto fires only on a fresh
  request (`:196-199`); `:202` skips while `reload_left > 0` or `slot.cooldown > 0`; `:215` sets `cooldown = fire_interval`.
- `crates/gta_sim/src/combat/weapons.rs:303-354` `tick_loadouts` runs right before `fire_weapons`
  (`combat/mod.rs:91-106`, one `.chain()` in `HealthSystems::Damage`) and decrements `cooldown` (`recover_gun`, `:346`).
  `select_weapon` (`:356-380`) resets `reload_left` on a held-gun change.
- Worked example (64 Hz, dt = 0.015625 s, pistol `fire_interval` 0.3 from `assets/combat/weapons.ron:3`): shot at tick 0;
  after tick k's decrement the cooldown is `0.3 − k·dt` for k ≤ 19 (k = 19 → 0.003125), 0 at k = 20. A request is
  consumed in the tick after it lands; today any request landing at k ≤ 19 is lost.
- Client: `src/input/mod.rs:274-282` raises `fire_requested` on `ActionEvents::START` in `Update`.
- NPCs raise the same latch: `gang/behavior.rs:122-137` `pull`, `police/behavior.rs:75-90` `pull`; their trigger
  timers are `(0.5, 1.1)` s (`gangs.ron:36`) and `(0.4, 0.9)` s (`escalation.ron:45`), so only the gang shotgun
  (0.9 s interval) ever pulls inside a cooldown.
- Melee also reads the latch (`combat/melee.rs:417-430`), but only while unarmed, and runs before `fire_weapons`.
- Existing gates that touch the rule: `crates/gta_sim/tests/shooting.rs:478-514` `semi_auto_vs_automatic`
  (second request 5 ticks after the shot must NOT fire: remaining 0.2219 s) and `:640-693`
  `cooldown_and_bloom_stay_with_their_gun`.
- Premise evidence: `scratch/premise/run2.txt` (all six presses delivered, one dropped at a 0.328 s gap).

### 1.2 t9 flake — root cause measured by this planner (new evidence)
Probe: `scratch/planner/t9_probe.py` wraps `tools/qa/scenarios/t9.py` unchanged and, every firefight poll, records
each HQ member's `GangMember` (sees, trigger_left, reposition), `Loadout`, and every living body inside the member's
`FireLine` wedge (math of `tactics/fire_line.rs:54-76`: reach = range + 0.865 m, cone = 4° + max(base + max_bloom,
spread_deg), clearance 0.5 m). 20 runs (12 × 6 s window, 8 × 30 s window), summary `scratch/planner/t9_findings.txt`:

| runs | second gang-0 group (post 14) engaged from the far side of the player | first HQ-group shot | failures |
|---|---|---|---|
| 15 | no | 0.61-0.92 s | 0 |
| 5 | yes (`g0p14:Attack:sees=True` in the wedge) | 1.4-1.8 s in 2 runs, never in 2 runs, 1 run with friendly fire | 3 |

Failing run `scratch/planner/t9_run12/probe.json`: both HQ members (SMG, 11 m and 13 m from the player) in `Attack`,
`sees = true`, `trigger_left = 0`, `cooldown = 0`, magazine 30 for 7 s, `reposition = None`, distances constant; the only
wedge blockers are the two post-14 members walking from 31 m to ~9.5 m BEHIND the player. `armor_lost = 0` — nobody
fired at all. Mechanism:
- `gang/behavior.rs:386-436`: a member pulls only if `hold_fire` finds the line clear.
- `tactics/mod.rs:170-234` `hold_fire` + `fire_line.rs:54-76`: a spared body (own gang) anywhere in the cone **up to
  weapon reach past the target** blocks ("a miss flies on"). Two groups on opposite sides of the player are each in
  the other's wedge.
- `tactics/mod.rs:207-219`: of two shooters blocking each other, the higher index stands; `fire_line.rs:313-350`
  `unblock`: the mover's candidate spots (`spots`, `:243-254`: ±1.5/3.0/4.5 m sideways, ±2 m along) cannot clear a line
  whose blocker is ~10 m past the target (needs ~5 m sideways at 11 m for an 11° SMG cone), so the mover falls back to
  "close in on the target", which never clears a body collinear behind the target.
- The second group engages because `gangs.ron:30` `sight_distance: 40.0` + heat makes members "attack on sight in
  their territory" — not because of the provocation radius (`group_radius` 30 m).
This is the same class as TASK-010 fixer rounds 2-3 and TASK-012 (`overreach`): the discipline is one-sided. There is
a rule "never fire through a friendly" and no positional rule "do not stand in a friendly's line / flank".
Killzone's position evaluation adds exactly that cost (lines of fire of friendlies) to position picking
([Straatman, Killzone's AI](http://cse.unl.edu/~choueiry/Documents/straatman_remco_killzone_ai.pdf),
[Guerrilla write-up](https://www.guerrilla-games.com/read/killzones-ai-dynamic-procedural-tactics)).
Flat-floor gates already have a "crossfire pair" (`tests/gang_fire_lines.rs:254-261`) and pass, but they give 30 s
and assert only `MIN_SHOTS = 6`; no gate bounds the time to the first shot.

### 1.3 Bench scene and measurement
- `--bench-scene` exists nowhere (grep). CLI parsing: `src/main.rs:37-61` (`flag_value`, `cli_seed`); `--seed` skips the
  main menu (`:203-205`). Window: `src/main.rs:194-197` (default 1280×720; `WindowResolution::new` takes PHYSICAL
  pixels in `bevy_window-0.19.1/src/window.rs:923`, `with_scale_factor_override` `:932`).
- The worst scene of §11 already exists headless: `crates/gta_sim/tests/traffic_bench.rs:69-181` (seed 1, parked car
  nearest the centre facing it, `drive_in`, heat = `stars[4].heat`, `hidden = 0`/`last_known` pinned each tick,
  `cruise` along the lane at 6 m/s, waits for ≥ 20 traffic cars and 5 active police cars ≤ 3840 ticks, then 640 ticks;
  TASK-016 mean 1.86 ms/tick). Runtime QA of the same state: frame cost with no vsync **3.0-3.7 ms** at 5★ with
  21-24 traffic cars (`maw/tasks/done/TASK-016/QA_REPORT.prev-1.md:93`), i.e. far inside 16.7 ms.
- Pure driving helpers are public: `vehicle::pursuit_steer`, `vehicle::speed_throttle` (`vehicle/autopilot.rs:22,41`),
  `vehicle::door_point` (`vehicle/mod.rs:149`), `TrafficGraph::lanes/lane/connector` (`traffic/graph.rs:319-331`),
  `TrafficLane { from, dir, length, v0, out }` (`graph.rs:17-29`). The seated driver's `DriveIntent` wins in
  `chassis::drive_vehicles` (`chassis.rs:109-115`); the drive runs in `VehicleSystems::Drive` (FixedUpdate,
  `vehicle/mod.rs:223-248`); the client writes `DriveIntent` in `Update` (`src/input/mod.rs:306-328`).
- `WantedLevel { heat, stars, last_known, hidden }` (`wanted/mod.rs:174-185`), `City(pub CityLayout)` with `parking`
  (`world/city.rs:27`), `PlayingSystems` (`flow/mod.rs:47`), `CameraView` (`population/mod.rs:175`).
- Profiling: feature `profile = ["bevy/trace_chrome"]` (`Cargo.toml`), which pulls `trace` + bevy `debug`
  (`bevy-0.19.1/Cargo.toml:2871-2875`). `bevy_log-0.19.1/src/lib.rs:322-343`: the chrome layer writes to `$TRACE_CHROME`
  (else `./trace-<µs>.json`), flushed when the `FlushGuard` resource drops (clean exit). `tracing-chrome-0.7.2/src/lib.rs:
  300-400`: one JSON object per line, `B`/`E` pairs per `tid`, `ts` in µs, final `\n]` only on a clean drop. Span names
  (bevy_log name_fn): `system: name="…"` (`bevy_ecs-0.19.1/src/system/function_system.rs:52`), `schedule: name=…`
  (`schedule/schedule.rs:562`), `update` per frame (`bevy_app-0.19.1/src/sub_app.rs:576`). Per the Bevy profiling doc
  the level must stay ≥ info ([bevy docs/profiling.md](https://github.com/bevyengine/bevy/blob/main/docs/profiling.md)).
- `get_diagnostics` (`bevy_brp_extras-0.22.6/src/diagnostics.rs:30-80`) returns current / average / smoothed over a
  rolling history only: a true minimum or 1 % low over 30 s is NOT observable through it. Min FPS from a single frame is
  noisy; the 1 % low (mean of the slowest 1 % of frames) is the stable metric
  ([SuperTuxKart blog](https://blog.supertuxkart.net/2024/07/why-average-fps-and-1-low-fps-are.html),
  [CapFrameX metrics](https://www.capframex.com/blog/post/Explanation%20of%20different%20performance%20metrics)).
- `tools/qa/brp.py:186-220` `frame_report` (monitors, present mode, as-shipped vs `AutoNoVsync` cost; the host has a
  30 Hz virtual display trap, TASK-010).
- Headless benches for the AC "T2/T8/T15": `crates/citygen/tests/perf.rs` (`#[ignore]`, release), `tests/city.rs:250`
  `city_startup_budget` (`#[ignore]`, release), `tests/civilian_bench.rs`, `tests/police_bench.rs`, `tests/traffic_bench.rs`.

### 1.4 §1 coverage by existing runtime scenarios (docstrings read)
| §1 | Exercised today by | Gap |
|---|---|---|
| 1 seed / loading / new city | t2 (`--seed`, golden hash), t12 (pause, typed seed → new city) | start without `--seed` from the main menu (random seed) through the loading screen |
| 2 walk/run/sprint/jump, camera vs walls | t1 (yaw), t4 (W/Shift/Alt/Space, AnimState) | camera collision never exercised at runtime |
| 3 pickups, drops, guns, melee, reload | t6 (range pickups, pistol/SMG/shotgun, reload), t7 (fists, bat) | picking up a gun dropped by a killed NPC |
| 4 civilians walk/flee/call | t8, t10 | — |
| 5 gang threat + group fire | t9 (Attack) | `Warn` ("угрожают") never exercised |
| 6 wanted 1-5, arrest, SWAT, escape | t10, t11 (1★ arrest, 4★ SWAT), t15 (2★ cars, shots), t13 (heat) | 5★ only via the new bench |
| 7 car enter/hijack/drive/run over/exit | t14 (parked, wall crash, exit), t15 (hijack, driver flees) | running over a pedestrian at runtime |
| 8 traffic yields, car chase | t15 (traffic, chase, dismount reported) | junction yield not observed at runtime |
| 9 Wasted keeps gear / Busted strips | t5 (Wasted, hospital; predates guns), t11 (Busted, no guns) | death keeping the guns |
| 10 HUD, pause, settings | t5/t6/t10/t12/t14 HUD parts, t12 pause, t13 `GameSettings` via BRP | settings changed through the UI |
| 11 FPS in the worst scene | none | the bench (this task) |
Real OS clicks for the paused settings screen have precedent (`maw/tasks/done/TASK-013/scratch/qa/osinput.py`, 55
lines, stdlib ctypes; used by TASK-014 `probe_settings.py`), because BRP holds never release while `Time<Virtual>` is paused.

---

## 2. Approach

1. **Fire buffer (game fix, GDD §8 "ввод буферизуется").** One request that lands while `0 < cooldown ≤
   fire_buffer_seconds` is remembered in the sim (`Loadout.fire_queued`) and fires on the first tick the cooldown is 0.
   Everything else is dropped as today. The memory lives in `Loadout` (sim state), not in `ActionIntent` (client-owned
   intent, also read by melee). It applies to every shooter (one rule). Input-buffer windows in action games are
   typically 80-250 ms. Buffers must be cleared on context change or they feel sticky
   ([Wayline, input buffering](https://www.wayline.io/blog/input-buffering-responsive-game-feel),
   [Moonjump forum](https://moonjump.com/forum/game-dev/input-buffering-in-action-games-how-precise-is-precise-enough-and-what-s-your-actual-window-dbe216)).
   So the queue is cleared on weapon switch, reload, stagger, Wasted/Busted exit and new city. Data: `weapons.ron`
   top-level `fire_buffer_seconds: 0.15`.
2. **t9: one bounded fix, then stop.** Add the missing positional half of the discipline in the shared tactics layer:
   **flank spots**. A blocked mover also tries spots rotated about the target by ±`reposition_flank_deg` at its current
   distance (data in `gangs.ron` and `escalation.ron`, shared `Discipline`). They are tested by the same `usable` check
   (own line clear, walkable, not through bodies or cars). The "lower index moves" rule already makes one side stand.
   The flank takes the mover out of the stander's wedge, and both lines clear. Gate: a headless crossfire-arrival layout
   that bounds the time to the first shot (RED today). **Stop rule** (process rule "second failure of the same class →
   redesign"): if that gate cannot be made green by this one mechanism, or t9 ×20 still fails, revert the tactics change
   and ship a redesign note (§3 step 2.6) instead of a second patch. Q1 lets the orchestrator skip the attempt.
3. **Bench scene = a client-side QA mode, not gameplay.** `src/bench/` (client domain) runs when `--bench-scene` is
   given. It boards the player into the parked car nearest the city centre (the `traffic_bench` rule), pins 5 stars
   (heat = `stars[4].heat`, `hidden = 0`, `last_known = player`), keeps the player and the car alive, and drives the car.
   Driving is scripted input into the player's `DriveIntent`, the same role as `input/`: pure pursuit on the current lane
   at `traffic.ron turn_speed`, always taking the rightmost connector, so the car circles a Downtown block and stays in
   the §11 "центр Downtown". No new sim domain, no new sim tuning. The bench resolution (1920×1080, GDD §11/Q6) goes to
   `render.ron`. A `BenchFrames` resource records per-frame real deltas on demand, so t16 reports mean FPS, 1 % low,
   min, p50/p99 frame ms over 30 s. BRP diagnostics cannot give those.
4. **Two measurement sessions in `t16.py`.** Session A (`--features dev`, release): composition, screenshots,
   `frame_report`, 30 s `get_diagnostics` polling plus the `BenchFrames` capture with no vsync. Session B
   (`--features dev,profile`, release, `TRACE_CHROME=<out>/trace.json`): composition, a 10 s window, clean BRP shutdown,
   then a streaming parse of the last 10 s of the trace (`tools/qa/trace.py`, stdlib, line by line). It reports the top
   systems per frame, FixedMain per tick, and the AI / physics shares against the §11 budgets. Tracing inflates costs,
   so FPS numbers come only from A. Session B also records `get_diagnostics`, labelled "with tracing" (literal AC).
5. **Perf verdict rule (GDD §11, TASK_FINAL).** Fix only if the trace shows a budget exceeded: frame cost at 1080p with
   no vsync > 16.7 ms, or FixedMain > 4 ms per tick, or AI > 1.5 ms per tick. Otherwise write "nothing to fix by trace"
   with the table. Expected outcome from TASK-016 numbers: nothing to fix.
6. **§1 evidence, not a checklist.** A coverage table (§1 point → scenario/phase → assertion → result) plus
   `tools/qa/scenarios/t16_s1.py` that exercises every gap in §1.4. `tools/qa/repeat.py` runs any scenario N times, so
   "N consecutive passes" is reproducible (TASK-031 reuses it).

Rejected: widening t13's click gap (orchestrator: forbidden); making t9 avoid the second group by choosing another stand
spot (hides a game bug in the harness); a general target-sector allocator / squad coordinator (redesign-sized, goes
into the note if the bounded fix fails); a game-side frame-stats plugin for all modes (only the bench needs it).

---

## 3. Steps

Host rules for every cargo command: one foreground cargo at a time, `-j 2`. `brp.py` hard-codes `-j 4` in
`Game.start` (`tools/qa/brp.py:43`): change it to `-j 2` (one token, host memory). Build `--features dev,profile
--release` once early (step 4.0): it is a new bevy feature set (`trace` + `debug`), expect a long first build.

### Step 1 — Fire buffer (sim, gated)
1.1 `crates/gta_sim/src/combat/weapons.rs`: `WeaponsConfig` gains `pub fire_buffer_seconds: f32` ("seconds before the
cooldown ends in which a trigger press is kept and fires on expiry"). `validate()`: finite and `>= 0.0` (add it to the
finite list at `:171-184`). `Loadout` gains `pub fire_queued: bool` (doc: "a press kept during the cooldown's last
`fire_buffer_seconds`; fires on expiry"). `select_weapon` (`:376-379`): clear `fire_queued` on a held change, next to
`reload_left = 0.0`.
1.2 `assets/combat/weapons.ron`: `fire_buffer_seconds: 0.15, // s: a press this close to the end of the cooldown fires
when it ends`. Grep every `WeaponsConfig {` literal in tests and add the field (the strict loader rejects a missing one).
1.3 `crates/gta_sim/src/combat/hitscan.rs:186-216`: replace the comment at `:187` ("A request during the last
`fire_buffer_seconds` of the cooldown is kept; any other request during cooldown or reload is dropped"). Logic, in order:
take `requested`; `reaction.is_active()` → clear `fire_queued`, continue; no held gun → continue;
`reload_left > 0` → clear `fire_queued`, continue; `slot.cooldown > 0` → if `requested && slot.cooldown <=
cfg.fire_buffer_seconds` set `fire_queued`, continue; else `let queued = mem::take(&mut loadout.fire_queued)`;
`wants` = semi: `requested || queued`, automatic: `fire_held || requested || queued`. The rest is unchanged (an empty
magazine with a queued press starts the reload, like a click). Borrow note: take `fire_queued` through the same
`let loadout = &mut *loadout;` rebinding as `slot` (disjoint fields).
1.4 `crates/gta_sim/src/combat/mod.rs`: a `reset_fire_queue` system (`Query<&mut Loadout>` → `fire_queued = false`) on
`OnExit(GameState::Wasted)`, `OnExit(GameState::Busted)` (next to `melee::reset_player_melee`, `:74-75`) and in `NEW_CITY`
(`:76`). Reason: `PlayingSystems` freezes the cooldown in Wasted, so a queued press would fire at the hospital.
1.5 Gates in `crates/gta_sim/tests/shooting.rs` (test area, `armed_app`). Derive every tick from
`Time<Fixed>::timestep()` and `WeaponsConfig`, never literals. Helper `ticks_left(k) = interval − k·dt`.
- `a_press_in_the_buffer_window_fires_when_the_cooldown_ends`: pistol shot at tick 0. Press so that the remaining
  cooldown after that tick's decrement is in `(0, buffer)`, well away from both edges. Shipped data: k = 17, remaining
  0.034375 s (the "30 ms before" case). Assert exactly one more `ShotFired`, on the tick the cooldown first reaches 0
  (k = 20 with shipped data), magazine 12 → 10.
- `a_press_long_before_the_cooldown_ends_is_dropped`: shotgun (0.9 s). Press with 0.49375 s left (k = 26, the "0.5 s
  early" case). No second shot through k = 70 (the cooldown ends at k = 58).
- `a_queued_press_is_dropped_on_weapon_switch`: queue a pistol press (k = 17), select SMG at k = 18. No shot from
  either gun through k = 30.
- In `tests/respawn.rs` (next to `:295`): a press queued in the tick of death does not fire after the respawn
  (`ShotFired` count unchanged through 64 ticks of `Playing`).
- Update `semi_auto_vs_automatic` (`:478-514`): compute the second press offset so the remaining cooldown is
  `> fire_buffer_seconds + 2·dt` (still dropped), and reword the message to "a press outside the buffer window".
  With shipped data this is the existing offset (0.2219 s > 0.15 s), so the gate keeps its meaning if the buffer changes.
- Flip-RED (record in IMPL_SUMMARY): (a) test-local `fire_buffer_seconds = 0.0` → the first gate RED; (b) sabotage
  `mem::take(&mut loadout.fire_queued)` to `false` → RED; (c) remove the `select_weapon` clear → the switch gate RED.
  Restore → GREEN.
1.6 NPC effect check: `cargo test -p gta_sim --test gang_fire_lines --test police_fire_lines --test gang_combat
--test gangs -j 2` must stay green (only the gang shotgun can queue, ≤ 0.15 s after its line check). If a friendly or
bystander hit appears, do NOT special-case the player. Stop and report, and the orchestrator decides (Risk R2).
1.7 Runtime: `tools/qa/scenarios/t13.py` unchanged (click gap 0.35 s stays). Run it ×10 in step 7.

### Step 2 — t9 crossfire (bounded, gated, with a stop rule)
2.1 Reproduce headless first (RED expected). New test in `crates/gta_sim/tests/gang_fire_lines.rs`, reusing `Layout` /
`run`. `run` must also return per-member `first_shot_s` (tick of the first `ShotFired` / 64; it already tracks
`last_shot`). Layout `crossfire_arrival`, from the probe geometry (`t9_run12`): HQ pair `(0, polar(11.0, 0), Smg)`,
`(0, polar(13.0, 6), Smg)`; arriving pair `(0, polar(31.0, 180), Smg)`, `(0, polar(33.0, 184), Pistol)`; a wall along one
side 3 m off the HQ→player line (`walls: [(Vec3::new(3.0, 1.5, -8.0), Vec3::new(0.3, 3.0, 16.0))]`, the sidewalk
building; check it against `world/test_area.rs` fixtures, gates lesson TASK-012), ×3 `JITTERS`. Assert: every armed
member's first shot ≤ `FIRST_SHOT_S`, zero friendly and zero bystander damage, `MIN_SHOTS` in 30 s. `FIRST_SHOT_S` is
**derived**: before the fix, record the RED (members with no shot in 6 s). After the fix, set it to the measured worst
+ 1 s, capped at 4.0 s (t9 window 6 s minus BRP latency). Write both numbers in the doc comment.
If the flat layout does not reproduce the starvation (the city geometry matters), also add a diagnostic case in
`tests/gang_city.rs` (seed 1, player at the probe position `(103.81, 1.2, -485.05)`, provoke the HQ group, let the post-14
group engage through heat) and use it as the RED instead. Record which one reproduced.
2.2 `crates/gta_sim/src/tactics/mod.rs:44-54` `Discipline` gains `flank_degs: &'a [f32]`.
`gang/mod.rs:360-370` and `police/mod.rs:177-187` fill it from new data fields.
2.3 Data: `assets/gang/gangs.ron` combat `reposition_flank_deg: [30.0, 60.0], // deg: spots rotated about the target
when no sideways spot clears (a friendly behind the target)`. Same field in `assets/police/escalation.ron` combat (police
share `tactics/`; their 3★ "окружение" hits the same geometry). Config structs: `GangCombatConfig` and
`PoliceCombatConfig` get `reposition_flank_deg: Vec<f32>`. `validate()`: every value finite, `0 < v < 180`.
2.4 `crates/gta_sim/src/tactics/fire_line.rs:241-254` `spots(b, d)`: after the sideways/along offsets, append for each
`deg` in `d.flank_degs` and each sign the spot `b.to + R_y(±deg)·(b.chest − b.to)` (same flat distance to the target).
Keep the existing nearest-first sort by `(spot − chest).length_squared()`. Rotation per GDD §3.2
(`R_y(θ)(x,y,z) = (x·cosθ + z·sinθ, y, −x·sinθ + z·cosθ)`, i.e. `Quat::from_rotation_y`). Worked examples for a unit
test `flank_spots_keep_the_distance_and_turn_both_ways` (target at the origin):
(1) chest (0,0,−10), +90° → (−10, 0, 0); (2) chest (10,0,0), +90° → (0, 0, −10); (3) chest (0,0,−10), −30° →
(5, 0, −8.660). Each keeps |p| = 10. Both signs are always candidates, so a sign error changes only the order. The
distance check catches a wrong axis. `pinned` (`:294-304`) iterates `spots(b, d)` too, so the flank spots join the
"pinned" judgement automatically. That is intended: a member is pinned only if no flank clears either.
2.5 Gates: 2.1 GREEN; every existing `gang_fire_lines`, `police_fire_lines`, `gang_combat`, `gangs`, `gang_city`,
`police_*` test green. Flip-RED: `reposition_flank_deg: []` in a test-local config → the crossfire gate RED. Restore →
GREEN. Runtime: t9 ×20 (step 7) with 0 failures, and the summary JSON of each run kept.
2.6 **Stop rule.** If 2.5 is not reached with this one mechanism (no second tactics patch), revert 2.2-2.4, keep the
RED test as `#[ignore = "t9 crossfire: redesign pending"]` with the numbers, and write
`maw/tasks/in_progress/TASK-017/REDESIGN_gang_crossfire.md` (≤ 1 page). Content: player fantasy ("provoke a gang →
bullets from every member within a second, sloppy but alive"); the missing shared rule ("positions respect friendly
lines of fire", Killzone line-of-fire cost, not only "never fire through a friendly"); the option of accepting rare
stray hits beyond the target in exchange for dropping the full-reach wedge (changes t9's zero-friendly-fire gate); a
rescope proposal (fold into TASK-032 shared occupancy, or a new task). t9 is then reported as a known flake with the
measured rate (3/20) and root cause.
2.7 The unexplained friendly-fire run (`t9_long1`: a member at 22 HP by 1.8 s with the post-14 group engaging;
bullet vs a run-over not attributed) is watched by the t9 ×20 runs. If it recurs, capture `DamageDealt`-level
attribution before any fix (report only).

### Step 3 — Bench scene (client domain `src/bench/`)
3.1 `src/bench/mod.rs` (new, < 300 lines), `pub struct BenchScenePlugin`, added in `src/main.rs` only when
`--bench-scene` is on the command line (`std::env::args().any(|a| a == "--bench-scene")`, next to `flag_value`).
`main.rs`: skip the main menu when bench is on (`if cli.is_none() && !bench { insert_state(MainMenu) }`). Load
`RenderConfig` before `App::new()` so the window can use `WindowResolution::new(w, h).with_scale_factor_override(1.0)`
from the bench resolution when bench is on (the default window otherwise). Seed: `--seed N` if given, else the clock
seed as today (t16 always passes `--seed 1`).
3.2 Data: `assets/world/render.ron` `bench_resolution: (1920, 1080), // --bench-scene window, physical px (GDD §11:
1080p)`. `src/visuals/config.rs` `RenderConfig` gains `pub(super) bench_resolution: (u32, u32)` + a pub accessor.
`validate()`: both > 0. Fix `city_gate.rs:33 render_config()` if it builds the struct literally.
3.3 Systems (FixedUpdate, `.in_set(PlayingSystems)`), state in `#[derive(Resource)] enum BenchPhase { Board, Chase }`:
- `board` (`Board`, `.before(VehicleSystems::Enter)`): if the player has `Driving` → `Chase`. Else pick the parked
  `Vehicle` (no `TrafficCar`/`PoliceCar`, `driver == None`) nearest the parking spot chosen like `traffic_bench.rs:75-89`
  (the `City.0.parking` spot nearest the origin whose heading points to the centre). Place the player at
  `door_point(cfg.door, …)` + float height (set `Position` AND `Transform`, gates lesson TASK-011), set
  `ActionIntent.vehicle_requested = true`. Retry each tick. After 5 s without success, log an `error!` "bench: could not
  board", which t16 turns into a failure.
- `pin` (`Chase` and `Board`): `WantedLevel.heat = WantedConfig.stars[last].heat`, `hidden = 0.0`,
  `last_known = Some(player)`; player `Health` current/armor to `HealthConfig` max; the driven car's `VehicleHealth` to
  `DamageConfig` max. These are named bench cheats (doc comment), bench-only.
- `drive` (`Chase`, `.before(VehicleSystems::Drive)`): pure fn `bench_target(graph, lane_pick, position, forward, speed,
  lookahead) -> Vec3`. Current lane = the lane with `dir·forward > 0.7` nearest the car (as `traffic_bench.rs:36-46`).
  Target = a point `lookahead` ahead along it, where `lookahead = autopilot.lookahead_min + lookahead_per_mps·speed`
  (`sedan.ron:60`, no new constant). Past the lane end, continue on the **rightmost** connector's `to_lane`: maximise
  `to_lane.dir · lane.dir.cross(Vec3::Y)`. Worked examples for the unit test: dir (0,0,−1) → right (1,0,0); dir (1,0,0)
  → right (0,0,1); dir (0,0,1) → right (−1,0,0) (Bevy: forward −Z, right +X). Then `DriveIntent { steer:
  pursuit_steer(…), throttle: speed_throttle(cfg, &cfg.autopilot, traffic.turn_speed, forward_speed), handbrake: false }`
  on the PLAYER (the seated driver's intent wins, `chassis.rs:109-115`). No stuck recovery: a stuck car is reported by
  t16, not hidden.
- `BenchFrames` (`#[derive(Resource, Reflect, Default)] #[reflect(Resource)] { recording: bool, frame_ms: Vec<f32> }`,
  registered): a `Last` system pushes `Time<Real>::delta_secs() * 1000.0` while `recording`. Default `false`, so an owner
  run without BRP never grows it. t16 flips it over BRP.
3.4 Unit tests in `src/bench/mod.rs` (`cargo test -p gta_like --bin gta_like`): rightmost-connector rows (the three
worked examples plus a T-junction with no right turn → the straightest), and the target staying on the lane before its
end. No headless city gate: a broken bench is visible on the first t16 run (cost-of-error rule).
3.5 `README.md` "Запуск": `cargo run --release -- --bench-scene [--seed 1]` (worst scene §11, 1080p window, add
`--features profile` for a chrome trace, `profile-tracy` for Tracy).

### Step 4 — `tools/qa/scenarios/t16.py`, `tools/qa/trace.py`
4.0 Build `cargo build -p gta_like --bin gta_like --release --features dev,profile -j 2` once. If it fails, that is a
finding (the feature was never built since T1); fix only the build.
4.1 `tools/qa/trace.py` (stdlib): `summarize(path, window_s=10.0)`. Read the last ~64 KB to find the max `ts`. Stream the
file line by line (strip `[`, `]`, leading `,`; tolerate a truncated last line). Keep per-`tid` stacks of `B` events and
aggregate the matching `E` only if `B.ts ≥ max_ts − window`. Returns: frames (`update` spans: count, mean / p50 / p99 /
max ms), FixedMain (`schedule: name=FixedMain`: count, mean ms per tick), `systems` (total ms per system name, and per
frame = total / frames). Shares by name prefix: AI = the `gta_sim::` modules from `crates/gta_sim/src/lib.rs` `pub mod`
minus an explicit non-AI list `{character, combat, config, flow, layers, player, vehicle, world}`, read from the file
(TASK-015 lesson: derive, do not hard-code enum lists); physics = `avian3d::` / `bevy_tnua`; an unmatched `gta_sim::`
module is reported as "unclassified". `tools/qa/test_trace.py` (unittest, offline): a synthetic 3-frame trace with two
threads, a nested system span and a truncated last line → exact totals. Add
`python -m unittest tools/qa/test_trace.py` to `.github/workflows/repo-checks.yml` next to `test_brp.py`.
4.2 `tools/qa/scenarios/t16.py` (`--out`), reusing `t5.game_state/wait_chunks/screenshot/resource_value`, `t6.rows`,
`t15` helpers, `t11.wanted`:
- Session A: `Game(features=("dev",), args=("--seed", "1", "--bench-scene"), release=True)`. Wait for `Playing`, golden
  hash, chunks. Composition deadline 120 s: player `Driving`, stars == 5, active `PoliceCar` == escalation row-5 `cars`,
  live `PoliceUnit` == row-5 `units` (both read from `escalation.ron`), `TrafficStats.cars ≥ 20` (the `traffic_bench`
  liveness bar). Otherwise `AssertionError("GATE BROKEN: bench scene never reached …")`. Record civilians alive, gang
  members alive, units by kind, cars, `Window` physical size, the car speed every 1 s. Three screenshots ≥ 0.5 s apart.
  `frame_report()` (switches to `AutoNoVsync`). Then set `BenchFrames.recording = true` and poll `get_diagnostics` every
  0.5 s for 30 s. Stop recording, read `frame_ms`, and compute mean FPS, 1 % low FPS (mean of the slowest 1 % of frames),
  min FPS, p50/p99/max frame ms, and the diagnostics averages. Log: no `ERROR` lines (`ERROR_WORDS` of t5).
- Session B: env `TRACE_CHROME=<out>/trace.json` (extend `Game` with an optional `env` dict, one line in `start`).
  Features `("dev", "profile")`, same args, same composition wait, 10 s window with 20 `get_diagnostics` samples
  (labelled "with tracing"), `game.shutdown()`, `process.wait(15)`. The trace file must end with `]`, otherwise report
  "trace truncated" and still parse. `trace.summarize(out/"trace.json", 10.0)`.
- `summary.json` + stdout: composition, window size, monitors/present mode, A metrics, B trace top-15 systems per frame,
  FixedMain per tick, AI and physics ms per tick vs the §11 budgets (4 ms / 1.5 ms), and verdict fields (numbers only;
  the words are written by QA). Hard failures: composition not reached, a crash, `ERROR` in the log, trace missing or
  unparsable. FPS is never a pass/fail (GDD §11).

### Step 5 — Measure and decide (trace only)
Run t16 ×3. Write in IMPL_SUMMARY a table from the worst of the three (mean FPS, 1 % low, p99 ms, FixedMain / AI /
physics per tick, top-5 systems). Rule (§2.5): if no budget is exceeded → verdict "nothing to fix by trace", no code.
If one is exceeded, fix only the top offending system with the §13 levers (time slicing via an existing cursor/slot,
render chunk / merge, `OcclusionCulling` + `DepthPrepass` measured before/after, kinematic far NPCs). One lever per
round, re-run t16 before/after. Any new knob goes to its §12 data file.

### Step 6 — §1 evidence sweep `tools/qa/scenarios/t16_s1.py`
6.0 Promote `maw/tasks/done/TASK-013/scratch/qa/osinput.py` verbatim to `tools/qa/osinput.py` (stdlib ctypes; Windows only:
the phase that needs it is skipped with a reported reason elsewhere).
6.1 Phases (each writes its own `summary.json` section and screenshots; hard assertions only on components; seed 1 unless
noted):
- **P1 menu:** launch WITHOUT `--seed` (own `Game` session). `GameState == MainMenu`, screenshot. `send_keys(["Enter"],
  100)` (empty field → clock seed, `menu/screens.rs:250-259`). Poll `Loading` (screenshot if caught), then `Playing`.
  Assert `CitySeed != 1` or a non-golden layout hash, and one player.
- **P2 camera vs wall:** teleport the player 1.0 m from the city-edge wall, facing inward (t14 finds that wall). Set
  `OrbitCamera.yaw` so the boom points into the wall, wait 0.5 s, and read the camera `Transform` and the player
  `Position`. Assert boom length (pivot → camera) < `camera.ron` distance − 0.5 m and the camera on the player's side of
  the wall. Screenshot. Repeat with the boom away from the wall: full distance.
- **P5 gang Warn → P3 drop pickup:** t9 helpers (HQ group, approach post). Heat 0, stand 6 m from the group (< 8 m
  `warn_distance`), wait `warn_seconds` + 1 s. Assert a member in `Warn`. Then named QA mutation: `Health.current = 0` on
  one member (no shot, so no provocation). Wait for a `Dropped` `WeaponPickup` near it and teleport the player onto it.
  Assert the `Loadout` gun owned or its reserve grew.
- **P7 run over:** park-range dummy (`Dummy`, `CityLandmarks.park_center`; beware the SMG pickup at the centre, gates
  lesson TASK-014). Put the nearest parked car 15 m from the dummy facing it (`Position` + `Rotation` mutation, t14
  precedent), F at its door, hold W 2 s. Assert the dummy's `Health` dropped, or its `HitReaction` knocked down within 3 s.
  Screenshot.
- **P8 junction yield:** sample `TrafficCar` rows every 0.25 s for 15 s. Assert ≥ 1 car seen with `waiting.is_some()` and
  speed < 0.5 m/s, and the same car later moving on another segment. Screenshot at a junction.
- **P9 death keeps guns:** pistol from the range. `DebugDamage` (t5 helper) to 0 → `Wasted` → `Playing`. Assert the
  `Loadout` pistol still owned with the same magazine + reserve, and the position within the t5 hospital radius. Before
  dying, set heat to 2★ and take one full-HUD screenshot (health, armour, weapon and ammo, stars, minimap).
- **P10 settings UI:** Esc (one BRP key). `osinput.click` "Настройки" (button found by its text child, TASK-014
  `probe_settings.py` method), screenshot, click one toggle. Assert the `GameSettings` field flipped. Esc back.
6.2 Coverage table (IMPL_SUMMARY, copied by QA into QA_REPORT): every §1 point 1-11 → scenario and phase → what is asserted
→ last result. Points 4, 6 and the rest link the existing t*.py. Point 11 → t16. No owner checklist (TASK_FINAL).

### Step 7 — Repeat runner and full gates
7.1 `tools/qa/repeat.py` (stdlib, ~40 lines): `python tools/qa/repeat.py t9 --runs 20 --out target/qa/rep`. It runs
`tools/qa/scenarios/<name>.py --out <out>/run<k>` sequentially, prints `k pass/fail` and the first `AssertionError` line,
and exits 1 if any run failed.
7.2 Headless: `cargo build -j 2`; `cargo clippy --workspace --all-targets -j 2 -- -D warnings`;
`cargo clippy -p gta_sim -p citygen --all-targets -j 2 -- -D warnings`; `cargo test -p gta_sim -j 2`;
`cargo test -p citygen -j 2`; `cargo test -p gta_like --bin gta_like -j 2` (3× for any touched presentation gate, gates
lesson TASK-022); benches: `cargo test -p citygen --release --test perf -j 2 -- --ignored --nocapture`,
`cargo test -p gta_sim --release --test city -j 2 -- --ignored city_startup_budget --nocapture`,
`--test civilian_bench --test police_bench --test traffic_bench --nocapture` (record means); `python tools/qa/tree_check.py`;
`python -m unittest tools/qa/test_brp.py tools/qa/test_trace.py`.
7.3 Runtime (release, `--features dev`): every `t1..t15`, `t16`, `t16_s1` ×3 via `repeat.py`; `t13` ×10 (P(10 passes | the
old 1/3 flake) ≈ 1.7 %); `t9` ×20 (old rate 3/20). Record pass counts, not "one green run".

### Step 8 — Docs (surgical)
- `docs/design/GDD.md` §4.1 after the roster table, one line: "Нажатие огня в последние `fire_buffer_seconds` кулдауна
  ствола запоминается и стреляет по его окончании (`weapons.ron`); остальные нажатия в кулдауне и перезарядке
  теряются." If step 2 lands: §6.3 "Бой" gets one line on flank spots (`reposition_flank_deg`).
- README "Запуск" (step 3.5). The narrative graph and status are the orchestrator's job after closing.

---

## 4. Risk areas

- **R1 Buffer changes NPC fire timing.** Only the gang shotgun can queue (0.9 s interval vs 0.5-1.1 s trigger). A queued
  shot fires ≤ 0.15 s after its line check. The fire-line gates are the judge (step 1.6). Fallback is the orchestrator's
  call, not a player special case.
- **R2 Stale latch leaks.** The Wasted/Busted/new-city clears are gated (1.5 respawn case). Pause keeps a queued press
  and fires it on resume (accepted: a press from before the pause).
- **R3 Flank spots move behaviour for police too.** Shared `spots()`: every `police_*` gate must stay green. More
  candidates also mean more `usable` rays (`RouteLoad.rays`), bounded by the AI slot. Check `police_bench` /
  `traffic_bench` means.
- **R4 The headless layout may not reproduce the runtime deadlock** (city geometry, arrival timing). Use the seed-1 city
  fixture (2.1). If neither reproduces, the stop rule applies (no blind fix).
- **R5 Trace size and build.** At 144 Hz a trace grows ~10-20 MB/s. The session-B wait for the composition (≤ 120 s)
  can produce 1-2 GB. D: has ~489 GB free. Parse by streaming only (never `json.load`). The trace needs a clean BRP
  shutdown (`FlushGuard` drop). A kill leaves no closing `]`, which the parser tolerates. The first `dev,profile` release
  build is long and adds a second bevy artifact set to `target/`. `target/release/gta_like.exe` is overwritten per
  feature set: always build right before launch (`Game.start` does).
- **R6 Bench car gets stuck** (rear-ends kinematic traffic at a stop line, a pile-up). It is reported as speed samples.
  The composition (5★, cars, units) still holds. No recovery logic (YAGNI). If it makes the scene unrepresentative in
  all 3 runs, that is a finding.
- **R7 1080p window on this host.** The physical size is forced with `with_scale_factor_override(1.0)`. If the
  monitor is smaller, Windows still creates it. t16 records the real `Window` physical size, and a mismatch is reported,
  not hidden.
- **R8 Gangs in the worst scene.** §11 lists 12 gang members, but territories are Residential/Industrial, not Downtown.
  t16 reports the actual count (the headless `traffic_bench` has some near the seed-1 centre). No scene hacking to
  force 12.
- **R9 OS clicks (P10)** move the real mouse and need the window in front (osinput `focus`, Alt tap). The phase is
  isolated so it cannot poison the other phases.
- **R10 Scope creep in the bug bash.** Anything beyond the two named flakes and the §1 gaps goes to IMPL_SUMMARY as a
  finding for the orchestrator (TASK-031 / TASK-032), not into code (e.g. go-around, intersection throughput: TASK-032).

## 5. Open questions (для оркестратора, решения по делегированию владельца)

**Q1. t9: пробовать ограниченный фикс или сразу редизайн?** Причина найдена замером: вторая группа той же банды с
дальней стороны от игрока (3/20 прогонов, все провалы только при ней).
- A (по умолчанию). Один механизм "обход по дуге вокруг цели" (`reposition_flank_deg`, общий `tactics/` для банд и
  полиции) + headless-гейт времени до первого выстрела. Не прошло → откат и записка о редизайне (шаг 2.6). Цена:
  ~40 строк sim + гейт. Риск: поведение полиции тоже сдвигается (гейты полиции судят).
- B. Сразу записка о редизайне ("позиции уважают линии огня своих", Killzone). t9 остаётся флаки с известной причиной
  и частотой. Цена ноль сейчас, но AC "каждый t*.py N подряд" для t9 не выполнится.
- C. Разрешить редкий огонь по своим за целью (урезать клин до `target + N м`). Меняется гейт t9 "ноль дружественного
  огня". По оценке ~0.6 шальных попаданий за 6 с перестрелки. Не рекомендую без решения по фантазии игрока.

**Q2. Сколько раз подряд = "N".** По умолчанию: все t*.py ×3, t13 ×10, t9 ×20 (старые частоты 1/3 и 3/20; 10 зелёных
при 1/3 — 1.7 % шанс случайности). Альтернатива: ×5 для всех (дольше примерно в 1.7 раза).

**Q3. Бенч-сцена без банд.** В центре Downtown по GDD банд нет (территории Residential/Industrial), а §11 перечисляет
12 бандитов. По умолчанию: мерить честную сцену центра и записать фактическое число. Альтернатива: стартовать бенч на
границе Downtown и территории банды (новая логика выбора места, другой "центр").

children: 0 launched / 0 reported.
