# TASK-035 IMPL_SUMMARY: lethality balance (1-star police, gang TTK, traffic hits)

Pre-flight (small-fix, the spec is the plan): every named entity exists with the assumed shape
(`police_alert`/`PoliceAlert`/`arrest.hostile_seconds`, `escalation.ron`, `gangs.ron`, `damage.ron`
`pedestrian`, `CopSenses.hostile`). One naming mismatch, not a block: the 1-star row the task calls "GDD §7"
is in GDD §6.4 (table "Эскалация"). I amended it there.

Cost of error: tuning the owner will feel on the first run. So the change is data plus one small read path
(`DamageScale`), and the claims are carried by headless TTK and rule gates.

## 1. What was implemented

### 1-star fire rule (decision 1)
- `crates/gta_sim/src/police/behavior.rs` (+34/-28): `police_alert` no longer reacts to witnessed player
  attacks on anyone. It sets `PoliceAlert.hostile_left = arrest.hostile_seconds` only when
  (a) a player `DamageDealt` lands on a cop (gun, melee, run-over; this path existed before), or
  (b) a player `BulletTrace` passes within `arrest.near_miss_distance` of a live cop's body centre
  (3D point-to-segment distance), i.e. the player shot at police and missed.
  A player who shoots a civilian at 1 star now gets arrest attempts, not gunfire. From 2 stars
  (`arrest: false` rows) cops fire on sight as before, because `next_state` is unchanged.
- `police/mod.rs`: new `ArrestConfig.near_miss_distance`, validated `> 0`, and updated docs.
  `police/fsm.rs`: one doc line. `wanted/mod.rs`: the `witnesses` re-export was left unused by this
  change, so I removed it (crimes.rs imports it directly).
- `assets/police/escalation.ron`: `near_miss_distance: 1.5`, and the `hostile_seconds` comment is reworded.
- `docs/design/GDD.md` §6.4: the 1-star row now reads "стреляют только если игрок атакует полицию: ранил копа
  или стрелял в него (пуля прошла ближе `arrest.near_miss_distance`); за стрельбу по мирным арест, не огонь".
  I also added one sentence naming the lethality knob and the TTK targets.

### TTK (decision 2): the knob is damage, not accuracy
- `combat/hitscan.rs` (+20/-4): new component `DamageScale(f32)` (default 1.0, reflected, registered in
  `combat/mod.rs`). `fire_weapons` multiplies the gun's base damage by it, for body hits and for
  `BulletHitVehicle.damage`, so cabin wounds follow too. The player has no `DamageScale` and stays at 1.0.
- `PoliceUnit` and `GangMember` `#[require(DamageScale)]`. `police_fsm` writes the current star row's
  `damage_scale` and `gang_fsm` writes `combat.damage_scale`. Each writes only when the value differs, and
  before any trigger pull in that tick.
- Data: `escalation.ron` `stars[].damage_scale` = 0.15 / 0.15 / 0.2 / 0.25 / 0.3 (the field sits before
  `cars` so the t16.py row regex still matches). `gangs.ron` `combat.damage_scale` = 0.2. Both are validated `> 0`.
- Why damage and not accuracy (measured, `scratch/flip/ttk_baseline_before_change.txt`, log dead_end):
  - Accuracy widens the hold-fire wedge by the same cone (`FireLine::of`: aim error + spread; police
    overshoot 60 m). A 9 deg police cone that reaches an 8 s TTK makes any body within ~7 m of the line at
    30 m block a cop. In a city crowd, 1-2 star police would almost stop firing, which is the "police
    pressure" risk named in the acceptance criteria.
  - Accuracy TTK depends strongly on distance and seed. At 9 deg the median is 12.9 s, but seeds range
    from 5.4 s to 30 s.
  - Accuracy cannot tame shotguns. A shotgun trio (gang 1) stays at a 4.5 s median even at 12 deg, and
    single blasts with head pellets kill in the same tick (0.00 s entries).
  - A damage scale does not depend on distance and leaves the fire discipline and every existing
    cone gate untouched. Hits stay readable: the player sees each hit land, it just does less.

### Traffic-car hit (decision 3)
- `assets/vehicle/damage.ron` `pedestrian`: `per_mps` 12 -> 6.5 and `shove_scale` 0.6 -> 1.0.
  `per_mps` alone was not enough. A real traffic car rolls on into the knocked-down body and hits it again
  (contacts at 15.39, 8.48 and 5.18 m/s), and the hits sum to a kill even at `per_mps` 5. With shove
  >= 0.8 x the closing speed the body is thrown clear and takes one hit (probe table in
  `scratch/traffic_hit_probe.md`). New curve: 10 m/s -> 46, cruise 16 m/s -> 85 (one hit, measured 81),
  police pursuit 20 m/s -> 111, 28 m/s -> 163. Deaths at high speed stay possible.
- No new tuning consts in game code. The TTK floors (8 s / 6 s) are test-only consts that restate
  task decision 2.

### Files (numstat +/-)
assets: gangs.ron +1, escalation.ron +9/-7, damage.ron +5/-3. src: combat/hitscan.rs +20/-4,
combat/mod.rs +3/-2, gang/behavior.rs +7/-1, gang/mod.rs +5/-2, police/behavior.rs +34/-28, police/fsm.rs
+1/-1, police/mod.rs +10/-4, wanted/mod.rs +1/-1. Tests: new `tests/lethality.rs` (313 lines),
traffic_pedestrian.rs +75/-2, police_arrest.rs +8/-1, config_police.rs +21/-1, config.rs +10,
config_traffic.rs +4/-4, config_vehicle.rs +2/-2, vehicle_hits.rs +4/-4. docs/design/GDD.md +3/-1.

### Gates (all headless, production composition)
`crates/gta_sim/tests/lethality.rs`:
- `one_star_cops_arrest_a_civilian_shooter` (correctness). Two patrol cops arrest the player, he wounds a
  civilian behind him (hit asserted), and the stars stay at 1. Until the arrest attempt starts (156 ticks),
  no cop enters Attack and there are 0 police `ShotFired`. Flip: the HEAD `police_alert` (witnessed-attack
  rule) makes it RED ("tick 0: a 1-star cop attacks a civilian shooter").
- `one_star_cops_fire_at_a_player_who_shoots_a_cop` (liveness of the hit path through the new rule).
- `one_star_cops_fire_at_a_player_who_shoots_past_a_cop` (correctness of the near miss). A shot 1 m beside
  the cop's chest hits nobody (asserted), yet the cops fire. Flip: dropping the `shot_at` clause makes it RED.
- `one_star_cops_fire_at_a_player_who_punches_a_cop` (correctness of `cop_hit`, melee). Flip: dropping the
  `cop_hit` clause makes it RED. The gun-hit test stays green under that flip, because a hitting trace is
  also a near miss. That is why this separate gate exists.
- `two_one_star_cops_after_an_attack_take_8_s` / `two_two_star_cops_take_8_s`: two patrol cops at 12.4 m,
  a standing unarmoured 100 HP player, 7 seeds (PoliceRng/CombatRng/GangRng reseeded). The 1-star case holds
  `PoliceAlert` by named mutation. Median TTK **11.17 s** (range 10.59-12.62) for both rows. Flip: the HEAD
  data gives 1.31 s (RED); a probe with `damage_scale` 0.2 gave 7.88 s (RED), so 0.15 ships.
- `a_gang_trio_at_10_m_takes_6_s`: three members 1.5 m apart at 10 m, guns cycled from each gang's
  arsenal. Gang 0 median **14.00 s**, gang 1 (shotgun) median **8.41 s** (min 6.39). Flip: HEAD gives
  1.66 s for gang 0 (RED).

`crates/gta_sim/tests/traffic_pedestrian.rs::a_cruising_car_does_not_kill_a_full_health_player`: a real
traffic car at the fastest shipped v0 (`traffic.ron` max(avenue, street) = 16 m/s). The player is placed on
its lane 1 m ahead of the bumper; IDM brakes and the car hits at 15.39 m/s (a precondition asserts
>= v0 - 1). Result: one hit, health 19, knocked down, alive. Flip: the HEAD `damage.ron` gives health 0
(RED); per_mps 6.5 with shove 0.6 gives two hits and a kill (RED).

Re-anchored existing gates:
- `police_arrest::attacking_player_is_shot_not_arrested` fired into the air as its "attack". Under
  decision 1 that no longer makes 1-star cops shoot, so it now fires 1 m past the cop (a near miss that
  hits nobody, asserted). The rest of the test is unchanged and still passes: Attack, fire, back to
  Arrest after `hostile_seconds`.
- `vehicle_hits::run_over_at_{10,6}_mps` ranges re-derived: (closing - 3) x 6.5 with closing = kick minus
  up to 0.25 m/s gives 44..=46 and 18..=20 (measured 45 and 18).
- Config sabotage strings that quoted the edited RON rows (`config_police`, `config_traffic`,
  `config_vehicle`). New sabotage rows: `stars[1].damage_scale`, `arrest.near_miss_distance`,
  `combat.damage_scale`.

## 2. Not implemented / deviations
- The GDD row is §6.4, not "§7" as the task says (same row).
- Star rows 3-5 `damage_scale` (0.2 / 0.25 / 0.3) have no gate; the task sets targets only for 1-2 stars.
  I chose them so escalation mostly comes from more units (6/8/12, SWAT armour and SMG). **Owner feel.**
- The knockback of a run-over person is stronger (`shove_scale` 1.0): civilians the player hits are
  thrown about 1.7x faster. This is visible on the first run. **Owner feel.**
- NPC bullets now also do less to civilians and cars (the scale belongs to the shooter, not the target).
  NPCs only aim at the player, so this matters only for stray hits.
- Runtime BRP scenarios t11/t12/t15 were not run by me; they are left to QA. Reasoning: they measure
  police approach, state and distance ("pressure" in t15 is a distance/state metric) and never player
  damage. The 1-star path in t11 uses a heat mutation and expects an arrest, which is unchanged. The t16.py
  and t11.py escalation regexes still match the reordered rows (checked with Python against the shipped file).

## 3. Test results
- `cargo test -p gta_sim -p citygen --no-fail-fast`: 64 binaries, all `ok`, 527 passed, 5 ignored
  (`scratch/test_sim_citygen.txt`).
- `cargo test -p gta_like --bin gta_like`: 82 passed (`scratch/test_client.txt`).
- `cargo clippy --locked --workspace --all-targets -- -D warnings`: clean.
- New and touched gates were run 3 times with identical output (`scratch/gates_3_runs.txt`; seeded,
  deterministic). Flip outputs are in `scratch/flip/`.

## 4. How to verify manually
- `cargo test -p gta_sim --test lethality --test traffic_pedestrian --test police_arrest -- --nocapture`
  prints the median TTK lines and `16 m/s lane: hit speeds [15.39], health 19`.
- In game (`cargo run --features fast`):
  - At 1 star, shoot a civilian next to a patrol pair: they walk up to arrest you and do not shoot.
  - Shoot at them or past them within 1.5 m: they open fire for 5 s.
  - At 2 stars, standing still in the open, you last roughly 5 s against the full row of 4 cops (about
    11 s against 2).
  - Step in front of a 16 m/s traffic car: you are thrown clear and survive with about 20 HP.
