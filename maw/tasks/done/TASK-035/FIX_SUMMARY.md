# TASK-035 FIX_SUMMARY (fixer round 1)

Inputs: IMPL_REVIEW.md (NEEDS_WORK), OPEN_DECISIONS.md (victim scope; m1 accepted), orchestrator note.
All changes are uncommitted in the working tree on `bugfix/lethality-balance` (on top of `106382e`).
Scratch evidence: `scratch/fixer/` (red_on_head.txt, gates_3_runs.txt, test_sim_citygen.txt, test_client.txt, clippy.txt).

## 0. Preflight: the review claim that would break things if applied verbatim

I checked these before fixing anything:
- M1 prescription: "apply `scale` to a body hit only when the target has `Player`". The risk: `hit.entity` can be
  the head hitbox child, which has no `Player`. A check on `hit.entity` would leave every player headshot unscaled
  (6.25 → 50 per pistol headshot). In `combat/hitscan.rs`, `target` is already resolved to `ColliderOf.body`
  before the damage branch, so the check goes on `target`, not on `hit.entity`.
- m1 numbers: the review says a shot aimed straight away from a cop 1 m away starts about 1.49 m from the cop.
  Recomputed with `muzzle_offset (0.25, 0.35, -0.45)`: with the cop straight behind it is 1.51 m (outside 1.5);
  with the cop to the side it is about 0.94 m (inside). The diagnosis holds (at arrest range many shots count);
  the number is slightly off. The review's alternative, "skip the first `arrest.distance` m of the segment", would
  also drop a point-blank miss at a cop, so I did not apply it. The orchestrator chose the GDD sentence instead.

## 1. Fixed

**M1: `DamageScale` scaled car-body damage (verified real: HEAD gate showed 3 cop hits → car lost 11.25).**
- `combat/hitscan.rs`: `fire_weapons` multiplies a body hit by the shooter's `DamageScale` only when the resolved
  body has `Player` (`Query<(&mut Health, Has<Player>)>`). `BulletHitVehicle.damage` is unscaled again. A new field
  `player_scale` carries the shooter's scale.
- `vehicle/impact.rs::apply_bullet_hits`: the car loses the full `damage · bullet_scale`. The cabin wound is
  `cabin_wound(damage · player_scale, share)` only when the seated driver is the `Player`. NPC drivers take the full
  wound. `CabinHit.damage` (read by traffic bail) is unscaled, the same as on main.
- Docs: `DamageScale`, `EscalationRow.damage_scale`, `GangCombatConfig.damage_scale` and the RON comments now say
  "to the player".
- Gates (each RED on HEAD source, GREEN after the fix):
  - `vehicle_hits::a_patrol_bullet_dents_the_car_by_the_full_pistol_damage`: a real 2-star patrol cop in Attack
    shoots the player's car. Every cop `BulletHitVehicle.damage` equals the pistol's 25 (falloff None), and the car
    loses `n · 25 · bullet_scale`. HEAD: 3 hits, car lost 11.25 → RED. Fixed: car lost 75.
  - `vehicle_hits::a_police_cabin_pellet_wounds_the_player_driver_by_the_scale`: a dummy with the 2-star police
    `DamageScale` shoots through the side window. The car loses 25; the player driver loses
    `round(25 · 0.15 · 0.5) = 2`. HEAD fails on the car half ("the car took a scaled pellet").
  - `lethality::police_damage_scale_applies_only_to_the_player`: the same dummy shoots a bystander dummy, then the
    player. The civilian stray keeps full damage (≥ floor(25 · 0.9)); the player takes ≤ ceil(25 · 0.15 · 1.1).
    HEAD: "civilian took 4" → RED. Fixed: civilian 26, player 4.
  - Why a dummy and not a real cop for the stray gate: police hold fire when a body is on the fire line (60 m
    overshoot), so a real cop never hits a bystander by design. The gate covers the mechanism (`DamageScale` in
    `fire_weapons`) with the shipped police row value.

**M2: `per_mps` 6.5 was global (verified real: an NPC hit at 15 m/s survived on HEAD).**
- `assets/vehicle/damage.ron`: `per_mps` is back to 12.0. `shove_scale` stays 1.0. New
  `player_share: 0.4`, validated in (0, 1] in `vehicle/config.rs`, with no const. `pedestrian_damage` takes a `share`
  argument: 1.0 for NPCs, `player_share` for a victim with `Player` (`apply_impacts`).
- Why 0.4: the player running head-on into a cruising car closes at up to 16 + 4.5 = 20.5 m/s, and
  (20.5 − 3) · 12 · share < 100 needs share < 0.476. At 0.5 that hit does 105 and kills. With 0.4: standing at cruise
  gives 59 (measured health 41), running gives 76 (measured 18.75 m/s, health 24). One hit kills from about
  23.8 m/s closing (a 28 m/s police ram does 120), so car deaths are still possible at high speed (decision 3).
  NPC run-over is back to its old lethality: one-hit kill from 11.3 m/s.
- Gates:
  - `vehicle_hits::run_over_at_15_mps_kills_a_civilian_in_one_hit` (first hit ≥ 100). RED on HEAD.
  - `vehicle_hits::run_over_at_{10,6}_mps_costs_the_formula`: ranges restored to main's 82..=84 / 34..=36, and the
    heat-fixture comment restored. RED on HEAD, GREEN now.
  - `traffic_pedestrian::a_cruising_car_does_not_kill_a_player_running_into_it` (new, review m3): the player runs at
    `run_speed` toward a car at max v0 (16) from 4 m ahead of the bumper. Preconditions: the approach speed in the
    tick before the contact is ≥ 0.9 · run (measured 4.50), and the car's own speed is ≥ cruise − 2 (measured
    14.25, because IDM brakes for a body closing on it). HEAD: health 0 → RED. Fixed: health 24.
  - `traffic_pedestrian::a_cruising_car_does_not_kill_a_full_health_player` (standing) is now one of two cases of
    `cruise_hit(running)`. Its assertions are unchanged, plus the approach precondition. Fixed: health 41.
  - `config_vehicle`: the per_mps sabotage string is back to `12.0`. New `player_share` rows: 0.0 and 1.5, both
    strictly outside (0, 1].
  - `impact.rs` unit test: an extra row with `player_share` 0.5 (84 → 42).

**m2: no upper bound on lethality.**
- `lethality.rs`: `assert_ttk` now also asserts median ≤ `TTK_CEILING` (30 s) for every TTK gate (2-cop 1★/2★,
  gang trios).
- New gates through the real AI, 7 seeds each, on the same floor with manually spawned units (counts pinned by
  `assert_shipped_police`):
  - `the_full_two_star_row_kills_within_30_s`: 4 patrol cops. Median **4.50 s**
    [4.50, 3.92, 4.09, 5.94, 4.50, 4.95, 4.12].
  - `the_full_five_star_row_kills_within_30_s`: 12 SWAT with SMGs. Median **2.47 s**
    [3.16, 2.47, 2.08, 2.56, 2.97, 2.12, 2.05].
- Flip: `damage_scale` 0.01 on all rows alone stayed GREEN (2★ 23.95 s, 5★ 10.00 s), because `roll_damage`
  floors every hit at 1 hp (`combat/weapons.rs:299`). Adding `combat.trigger_seconds (2.0, 3.0)` makes both RED
  (2★ 40.00 s at the cap, 5★ 37.19 s > 30). Restored, then GREEN.
- Note for the owner/orchestrator: the full 2★ row kills a standing player in about 4.5 s and 5★ in about 2.5 s.
  The task sets only the 2-unit targets. These medians are reported, not tuned.

**m1: the near-miss rule during an arrest.**
- GDD §6.4 row 1 gets one sentence: "выстрел ближе `near_miss_distance` к копу считается атакой на полицию, в том
  числе во время ареста".
- `lethality::one_star_cops_arrest_a_player_who_shoots_a_civilian_in_front_of_them` (orchestrator item 4): a
  civilian (dummy) stands 2.5 m in front of cop 0 on the line from the player. The player shoots it (hit asserted),
  and the trace ends 2.75 m from the cop. Over 128 ticks no cop enters Attack and police fire 0 shots. **The police
  do not fire**, so there was nothing to decide. Flip: `arrest.near_miss_distance` mutated to 3.0 in the test →
  RED at tick 0. Restored, then GREEN.

**GDD**: the sentence on the knob now says the share applies only to damage on the player (body and cabin share),
that cars and NPCs take full damage, and it names `pedestrian.player_share`.

## 2. Skipped
- m1 alternative ("skip the first `arrest.distance` m of the segment"): rejected by OPEN_DECISIONS; it would also
  drop point-blank misses. I did the GDD amendment instead.
- m3 "mark it for the owner run": replaced by a real gate (the orchestrator asked for one). The margin is now 24 HP
  running and 41 standing. A **sprint** head-on (6.8 m/s) is analytically (16 + 6.8 − 3) · 4.8 = 95: it survives,
  just barely. There is no gate for it; the orchestrator named run speed.
- Nit, O(traces × cops) in `police_alert`: fine at this N, as the review says.
- Nit, the 2★ 2-cop gate duplicates the 1★ numbers: kept as a row guard (TASK-011 lesson), same as the review.
- `vehicle_hits.rs` is now 799 lines: over the 750 warning, under the 950 hard limit. I did not split it; the new car
  and cabin gates need its local helpers (`cabin_range`, `fire_at_car`, `CarHits`).
- Runtime BRP scenarios t11/t12/t15: not run (no windowed build in this stage). No tool under `tools/` reads
  `damage.ron` pedestrian fields (grep). Left to QA.

## 3. Test results
- Targeted gates ×3 (lethality, traffic_pedestrian, vehicle_hits, police_arrest, police_crimes, police_fire_lines,
  police_pull_out, police_range_edge): identical output in all 3 runs, all ok (`scratch/fixer/gates_3_runs.txt`).
  - `cargo test -j 2 -p gta_sim --test lethality --test traffic_pedestrian --test vehicle_hits --test police_arrest --test police_crimes --test police_fire_lines --test police_pull_out --test police_range_edge --no-fail-fast -- --nocapture`
  - lethality 11 passed (medians: 1★ hostile 11.17, 2★ pair 11.17, gang 0 14.00, gang 1 8.41, full 2★ 4.50,
    full 5★ 2.47); traffic_pedestrian 4 passed; vehicle_hits 19 passed; police_* 9/4/7/7/6 passed.
- `cargo test -j 2 -p gta_sim -p citygen --no-fail-fast`: 66 result lines, **535 passed, 0 failed, 5 ignored**.
- `cargo test -j 2 -p gta_like --bin gta_like`: 82 passed.
- `cargo clippy -j 2 --locked --workspace --all-targets -- -D warnings`: clean.
- rustfmt (edition 2024) only on the edited .rs files; `git diff --stat` shows only the files below.

Files changed in this round: assets/vehicle/damage.ron, assets/police/escalation.ron (comment), assets/gang/gangs.ron
(comment), crates/gta_sim/src/combat/hitscan.rs, crates/gta_sim/src/vehicle/{impact.rs,config.rs},
crates/gta_sim/src/{police,gang}/mod.rs (doc lines), crates/gta_sim/tests/{lethality.rs,vehicle_hits.rs,
traffic_pedestrian.rs,config_vehicle.rs}, docs/design/GDD.md.

children: 0 launched / 0 reported
