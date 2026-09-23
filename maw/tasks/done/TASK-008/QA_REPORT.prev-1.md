# QA_REPORT — TASK-008 (GDD T7, melee)

QA agent a1 (claude opus, effort medium). Checkout `D:/test-gta-like`, branch `feature/t07-melee`, HEAD `f8f19ae`
(feature commit `3666475`, base `382e799`). Tree clean at start and at end, apart from this report and `scratch/qa_a1/`.

## 0. Disconfirmation (done before the rest of the review)

The counter-example I set out to find: **a punch in the attack window that does not land on a target standing in reach
in front of the attacker.** AC 1 ("удар в окне попадает") is gated only with the attacker in open space. `swing_melee`
sweeps a 0.35 m sphere from the attacker's body axis. The capsule radius is 0.3 m, and
`ShapeCastConfig::from_max_distance` keeps `ignore_origin_penetration: false`. So anything within 0.05 m of the
attacker's capsule, on ANY side, overlaps the sphere at the origin and comes back as the hit at distance 0.

**The counter-example held.** It is reproduced headless through the production `compose_sim` app. See Bug 1.

## 1. Environment

- No docker-compose, no dev server. Existing test infrastructure: `cargo test`, used directly on the checkout.
- Runtime: the real windowed build `cargo build -p gta_like --release --features dev` (via `tools/qa/brp.py`),
  driven over BRP at `127.0.0.1:15702`. The game was started only by the scripts below and closed with
  `brp_extras/shutdown`. `tasklist` shows no `gta_like` process left. No containers or services were started.
- Reproduce:
  ```
  cargo build
  cargo clippy --workspace --all-targets -- -D warnings
  cargo test --workspace -j 4
  python tools/qa/scenarios/t7.py --out maw/tasks/in_progress/TASK-008/scratch/qa_a1/t7
  python maw/tasks/in_progress/TASK-008/scratch/qa_a1/probe_knockdown_view.py <out dir>
  # Bug 1 probe: copy scratch/qa_a1/qa_a1_probe.rs to crates/gta_sim/tests/, then
  cargo test -p gta_sim --test qa_a1_probe -- --nocapture --test-threads 1
  ```

## 2. Test results

### Existing suite
- `cargo build`: green.
- `cargo clippy --workspace --all-targets -- -D warnings`: green.
- `cargo test --workspace -j 4`: all green, 0 failed (`scratch/qa_a1/cargo_test_ws.log`). citygen 9+3(+1 ignored)+0(+1 ignored)+13;
  gta_like bin 32; gta_sim lib 19, anim_state 4, asset_manifest 3, city 6 (+1 ignored), config 19, health 6, jump 3,
  **melee 14**, movement 4, respawn 5, shooting 16, terrain 2. There are zero failures, so there are no new failures
  compared with the base. I did not rebuild the base, because a second target dir or worktree is forbidden.
- Flakiness: the implementer logged a nondeterministic one-tick ordering bug, so I ran the rebuilt binaries repeatedly.
  `melee` test binary 20/20 green; `gta_like` bin tests 20/20 green.

### Flip-RED done by me (not the author's perturbations)
Both flips were done on committed code and restored by copy. I verified the restore with sha256
(`melee.rs` c1c0ed78…b1ad, `hit_stop.rs` efa21839…74d, same before and after).
1. `apply_strikes` shove with x/z swapped (`Vec3::new(d.z, 0, d.x)`) → `knockback_pushes_along_the_blow` **RED**
   ("[0, 0, -1]: pushed 0 m along the blow", lateral 0.177 m). GREEN after restore.
2. `start_hit_stop` also calls `Time<Virtual>::set_relative_speed(0.5)` on a hit → `hit_stop_freezes_only_the_pair_and_not_time`
   **RED** at `character_gate.rs:624` ("hit-stop scaled Time<Virtual>"). GREEN after restore.

### New probes (independent; `scratch/qa_a1/qa_a1_probe.rs`, run in the production headless app, then removed from the crate)
| Probe | Result |
|---|---|
| Wall 0.2 m thick, its face `gap` m behind the player's capsule; dummy 1 m in front; one click | gap 0 / 0.02 / 0.04 m: **0 hits**; gap 0.2 m: 1 hit at T0+8 (control) |
| Player walks sideways (+X) into a wall until stopped, then punches a dummy 1 m in front (−Z, parallel to the wall) | with wall: **0 hits** (player axis 0.300 m from the wall face); without wall: 1 hit |
| Bystander dummy touching the player's back (0.609 m axis to axis); target dummy 1 m in front | **the bystander behind is hit** (Staggered, 10 dmg); the front dummy is not hit |
| Three clicks at ticks 0, 3, 6 (mashing inside the first swing) | 2 swings (hits at 8 and 31): one buffered click, as the plan's table (g) says. Design, not a bug |

### Runtime (BRP)
- `tools/qa/scenarios/t7.py` (my own run, `scratch/qa_a1/t7/summary.json`, log `scratch/qa_a1/t7_run.log`): **PASS**.
  The fist combo took the dummy from 100 to 60 HP; `KnockedDown`, then `Steady` 0.80 s later. Damage numbers {10, 20} were live.
  The bat was picked up, and key 1 toggled `Fists`/`Bat`. The bat swing took 100 to 75 HP and knocked the dummy down 0.28 s after the click.
  `log_errors` was empty and the scenario exited cleanly.
- Screenshots I looked at:
  - `scratch/qa_a1/t7/combo_1.png`: player mid-jab, "10" over the middle dummy, the two side dummies standing.
  - `scratch/qa_a1/t7/combo_2.png`: "10" fading and "20" shown, the player's arm extended.
    The middle dummy is hidden behind the player from this camera.
  - `scratch/qa_a1/t7/bat_knockdown.png`: "25" over the target, player in a swing pose.
    The bat mesh cannot be seen clearly from behind (owner item).
  - `scratch/qa_a1/view2/knocked_down_from_behind.png` (extra probe `probe_knockdown_view.py`, player stepped back 3 m while the
    dummy was `KnockedDown`): the middle dummy lies at ground level, partly hidden by the player.
  - `scratch/qa_a1/view2/standing_again.png`: the same dummy is upright again after `Steady`. Knockdown and stand-up both read in the real build.
- Diagnostics after about 3 s of warm-up in release: avg frame time 6.95 ms (about 144 FPS). The present mode and monitor refresh
  were not measured. This is not a performance criterion, so it is only a sanity number, not a frame cost.

## 3. Acceptance criteria

| Criterion | Test performed | Result |
|---|---|---|
| Headless: удар в окне попадает, вне окна нет | `melee::hit_lands_only_in_window` (exact tick T0+8, window close at T0+15 → no hit), unit table 3.1; 20/20 repeat runs. QA probe: **fails when the attacker touches a wall or character from the side/back** (Bug 1) | **FAIL** (open-space PASS; wall/crowd case FAIL) |
| Headless: третий удар комбо сбивает с ног | `third_combo_hit_knocks_down` (hits 8/31/54, KnockedDown at 54, Steady at 131), `combo_resets_after_window` control | PASS |
| Headless: knockback в направлении удара (−Z, +X, +Z) | `knockback_pushes_along_the_blow`, 3 cases; my x/z-swap flip goes RED | PASS |
| Headless: hit-stop не меняет `Time<Virtual>` и число фиксированных тиков | `character_gate::hit_stop_freezes_only_the_pair_and_not_time` (client crate, production `compose_sim` + `HitStopPlugin`); my `Time<Virtual>` flip goes RED | PASS |
| Runtime QA: `tools/qa/scenarios/t7.py` exists and passes | my own run, PASS, screenshots read (see §2) | PASS |
| Owner-run: драка "хлёсткая", тряска не тошнит | owner checklist in §6 | PENDING (owner) |
| Every new tuning value in its data file | diff grep: the only new consts are `MELEE_CONFIG` (path), test-only `DT`/`A`, and hash constants in `smooth_noise` (algorithm). Tuning lives in `melee.ron`, `juice.ron`, `visual.ron`, `render.ron` | PASS |
| `cargo build`, `clippy -D warnings`, `cargo test -p gta_sim` green | see §2 | PASS |
| Existing tests pass | workspace suite green | PASS |

## 4. Bugs found

### Bug 1 — MEDIUM-HIGH: a character touching a wall or another character from the side or back cannot punch forward
- Where: `crates/gta_sim/src/combat/melee.rs` `swing_melee`, the `spatial.cast_shape` call. The sphere `cfg.cast_radius`
  (0.35) starts at `position + Y·(cast_height − float_height)`, on the body axis. The capsule radius is 0.3, and
  `ShapeCastConfig::from_max_distance(range)` has `ignore_origin_penetration: false`.
- Mechanism: any World/Character collider within 0.05 m of the attacker's capsule overlaps the sphere at the origin.
  The cast returns it at distance 0, whatever the direction, and `swing.landed = true` spends the swing on it.
  A wall gets no strike and the target in front gets nothing. A character (a bystander behind) gets the `Strike`
  and is staggered.
- Reproduction (headless, production app): `scratch/qa_a1/qa_a1_probe.rs`
  - `probe_walked_into_side_wall`: walk into a wall on +X until stopped (Tnua leaves the capsule flush), dummy 1 m at −Z,
    click → 0 hits. Without the wall → 1 hit.
  - `probe_wall_behind_attacker`: wall face 0 / 0.02 / 0.04 m behind the capsule → 0 hits; 0.2 m → 1 hit.
  - `probe_bystander_behind_attacker`: a dummy touching the player's back takes the punch aimed at the dummy in front.
- Expected: the punch reaches a live target in front within reach. Actual: it whiffs, silently.
  This is the normal case in a city (fighting along a building wall, walking into a wall and then turning to punch,
  crowds in T9), and no gate covers it.
- Fix direction (for the fixer to recompute, not a prescription): the sweep must not count colliders it starts inside
  that are not in front. Options: `cast_shape_predicate` rejecting a distance-0 hit whose `point1` lies behind the
  origin along `dir`; or `ignore_origin_penetration: true` plus a separate front-half overlap check for a target
  already touching the attacker; or a sweep radius ≤ the capsule radius enforced by config validation. A target
  touching the attacker in front must still be hit (it is today). Turn the two wall probes and the bystander probe into gates
  and flip them.

### Low — noise time uses `f32` `Time<Real>::elapsed_secs()`
`src/juice/shake.rs` `shake_camera` feeds `real.elapsed_secs() * noise_hz` to `smooth_noise`. f32 precision on
elapsed time falls with session length, so the shake noise gets coarser (steppier) over many hours of play.
It is cosmetic. `elapsed_secs_wrapped()` would avoid it. No action required for T7.

### Notes, not bugs
- Rapid mashing (3 clicks within 0.1 s) gives 2 swings: one buffered click by design (plan 3.1 (g)). The owner may feel
  it as "dropped" clicks. This belongs on the feel checklist.
- The shake is subtle by data: one hit gives 0.25 trauma → 0.0625 × 3° ≈ 0.19° yaw. Three hits in a row reach ≈ 1.7°. Owner feel.
- The hit-stop pauses only the animator. The sim keeps sliding the victim through the 50 ms freeze (by design, GDD §8).

## 5. Verdict

**NEEDS_FIXES.**

Build, clippy and the full suite are green. Every headless gate I checked goes through the production composition,
two independent flips went RED, and the runtime scenario passes on the real build with sane screenshots. But the
core rule of AC 1 ("a punch in the window lands") fails in a common geometry: an attacker standing against a wall
or touching another character. It fails silently, and every gate of the slice sits in open space, so nothing catches it. The fix is local
(the sweep in `swing_melee`) and needs one new gate with a collider touching the attacker from the side or back.
After the fix, the owner checklist below still applies (SHIP-PENDING-RUNTIME).

## 6. Owner checklist (Russian)

`cargo run --release -- --seed 1`, тир в парке (манекены в 10 м от центра парка по −Z, бита в 2 м от центра по +Z):
- [ ] Без оружия (клавиша 1) три ЛКМ по манекену с интервалом ~0.3 с: читается комбо (правый, левый, пинок), третий удар сбивает с ног, манекен лежит и встаёт примерно через 1.2 с.
- [ ] Драка ощущается "хлёсткой": заморозка 50 мс на ударе заметна, но не выглядит как лаг.
- [ ] Тряска камеры на ударах заметна и не тошнит (при серии из трёх ударов подряд тоже).
- [ ] Быстрое "долбление" ЛКМ: не раздражает, что клик внутри уже идущего удара запоминается только один.
- [ ] Бита: подбирается, клавиша 1 без оружия переключает кулаки и биту, удар битой тяжелее и сразу сбивает с ног; бита в руке выглядит сносно (хват настроен под пистолет).
- [ ] После нокдауна поза манекена полностью возвращается (слой rest-pose), и ноги при переходе в idle не застывают странно.
- [ ] После фикса Bug 1: встать вплотную к стене здания боком или спиной и ударить манекен/NPC перед собой, удар должен попасть.

Tuning: `assets/combat/melee.ron`, `assets/juice/juice.ron` (`hit_stop_seconds`, `shake`), `assets/character/visual.ron`, `assets/world/render.ron`.

## 7. Cleanup and accounting
- Temporary files in the checkout: `crates/gta_sim/tests/qa_a1_probe.rs` (moved to `scratch/qa_a1/`) and the two
  flip edits (restored, sha256 verified). `git diff --quiet HEAD` is clean. `git status --short` shows only this report
  (and scratch, whose PNGs are git-ignored).
- Game processes: none left (`tasklist`). No containers or services.
- Log: one `decision` entry appended. PCTX proposal appended (sweep-from-inside-the-caster lesson).
- children: 0 launched / 0 reported.
