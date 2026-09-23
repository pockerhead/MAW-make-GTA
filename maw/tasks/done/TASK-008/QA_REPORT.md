# QA_REPORT — TASK-008 (GDD T7, melee) — QA round 2

QA agent a2 (claude opus, effort medium). Checkout `D:/test-gta-like`, HEAD `ed3a1ba` (fix commit `4b81e50`).
Round 1 (`QA_REPORT.prev-1.md`) was NEEDS_FIXES for Bug 1: a wall or bystander touching the attacker's side or back
ate the punch. This round checks the fix (`swing_reaches` + `shape_hits_callback`) and tries to break it.

## 0. Disconfirmation (done first)

The counter-example I looked for: **a target in front that is still not punched because a body touching the attacker
passes the new `swing_reaches` rule.** The rule accepts a start-overlap hit when `normal1 · dir < -1e-3`, so a flush
wall beside the attacker is accepted as soon as the swing leans into it by more than about 0.06 deg.

**Result: it holds, but only in the geometry the fixer already put on the owner checklist.** It is not a defect of the fix.
- Wall flush on +X (gap 0 / 0.03 / 0.049 m from the capsule), dummy 1 m ahead: aim 0 deg or 2 deg away from the wall
  hits the dummy at T0+8. Aim 0.1 / 0.5 / 1 / 2 / 5 / 10 deg into the wall: the swing is spent on the wall.
- This is the same answer an honest sweep gives. A sphere that moves into a wall at angle θ reaches it at
  `gap / sin θ`. The dummy's contact is at 0.35 m, so the wall wins when the sphere-to-wall gap is `< 0.35 · sin θ`.
  As the gap goes to 0 this means "any θ > 0". Raw hits confirm it (`scratch/qa_a2/qa_a2_diag.log`): 1 mm off the sphere
  at 0.5 deg the wall is at 0.115 m (wall wins); at 0.1 deg it is at 0.573 m (dummy at 0.350 m wins).
  So the rule is the continuous limit of the sweep. The real cause is that `cast_radius` 0.35 is wider than the capsule
  (0.3), which is GDD §4.2 tuning.
- Correction to FIX_SUMMARY's owner note: "the sweep does the same to a wall a few cm away" is true only for small
  angles and gaps. At 1 deg the sweep ignores a wall more than 6 mm from the sphere. At 5 deg the limit is 3 cm.

## 1. Environment

- No docker-compose, no dev server. I ran `cargo` directly on the checkout, one command at a time.
- Runtime: `tools/qa/scenarios/t7.py` and `t6.py` through `tools/qa/brp.py`. They use the real windowed release build with
  `--features dev` and BRP on `127.0.0.1:15702`. Both runs ended with `brp_extras/shutdown`. `tasklist` shows no `gta_like`
  process left. I started no containers or services.
- Reproduce:
  ```
  cargo build
  cargo clippy --workspace --all-targets -- -D warnings
  cargo test --workspace -j 4
  # probes: copy scratch/qa_a2/qa_a2_probe.rs (and qa_a2_diag.rs, scratch/qa_a1/qa_a1_probe.rs) to crates/gta_sim/tests/
  cargo test -p gta_sim -j 4 --test qa_a2_probe -- --nocapture --test-threads 1
  python tools/qa/scenarios/t7.py --out maw/tasks/in_progress/TASK-008/scratch/qa_a2/t7
  python tools/qa/scenarios/t6.py --out maw/tasks/in_progress/TASK-008/scratch/qa_a2/t6
  ```

## 2. Test results

### Existing suite
- `cargo build`: green. `cargo clippy --workspace --all-targets -- -D warnings`: green (`scratch/qa_a2/clippy.log`).
- `cargo test --workspace -j 4`: 0 failed (`scratch/qa_a2/cargo_test_ws.log`). citygen 9 + 3 (+1 ignored) + 0 (+1 ignored) + 13;
  gta_like bin 32; gta_sim lib 19, anim_state 4, asset_manifest 3, city 6 (+1 ignored), config 19, health 6, jump 3,
  **melee 19** (round 1: 14, plus the 5 new gates), movement 4, respawn 5, shooting 16, terrain 2. Round 1 had the same
  names plus 5 new melee names. There are no new failures.
- Stability: the rebuilt `melee` test binaries ran 30 times in a row (15 per build hash), 30/30 green.

### Round-1 probe re-run on HEAD (`scratch/qa_a2/qa_a1_probe_rerun.log`)
| Probe | Round 1 | Round 2 |
|---|---|---|
| Wall face 0 / 0.02 / 0.04 m behind the capsule, dummy 1 m ahead | 0 hits | 1 hit at T0+8 each |
| Walked sideways into a wall (capsule flush, x 0.300), dummy 1 m ahead | 0 hits | 1 hit |
| Bystander touching the back (0.620 m) | bystander hit | only the front dummy hit (T0+8), bystander `Steady` |

### New probes (`scratch/qa_a2/qa_a2_probe.rs`, log `qa_a2_probe.log`; production `headless_app()`, removed from the crate afterwards)
| Probe | Result |
|---|---|
| Dummy touching (0.62 m) at 0/30/60/75/85/89 deg off the swing | hit at T0+8 in every case |
| Same at 91/100/135/180 deg | no hit, the swing is not spent (`landed false`) |
| Wall behind at gap 0.049 / 0.0499 / 0.05 / 0.0501 / 0.051 m (the sphere/capsule boundary) | dummy ahead hit in every case |
| Two dummies in line (0.8 + 1.4 m; 0.62 touching + 1.2 m) | only the near one hit |
| Flush wall on +X plus a dummy touching at -20 deg (front-left), aim 0/2/5 deg into the wall | the touching dummy is hit in all three (a tie at distance 0 goes to the first hit found, see Notes) |
| Side wall, gap 0-0.051 m, aim -2..10 deg into it | see §0: blocked when leaning into it, hit when parallel or away. At exactly gap 0.05 (sphere just touching) results alternate between aims because Tnua float drifts by less than 1 mm (boundary noise, not a defect) |

### Flip-RED I did (not the fixer's perturbations)
Both flips were done on committed code. After each one I restored with `git checkout` and checked sha256
`6d11d530…1d78` for `melee.rs`, the same as before.
1. `swing_reaches` sign inverted (`normal1·dir > +tol`, so it accepts overlaps that move away) →
   `body_touching_the_front_is_punched`, `bystander_behind_is_not_punched`, `wall_behind_does_not_block_the_punch` **RED**
   (`scratch/qa_a2/flip1_sign.log`).
2. Nearest-hit selection flipped to farthest (`hit.distance > n.distance`) → `wall_blocks_the_punch` **RED** (dummy hit
   through the wall) (`flip2_farthest.log`). So the "nearest wins" half of the new code is also gated.
Both were GREEN after the restore and a rebuild.

### Verification of the fix claims
- Bug 1: verified fixed (the round-1 probe above, plus my own probes). The mechanism is as claimed. avian3d 0.7.0
  `shape_hits_callback` (`system_param.rs:740-805`) passes `stop_at_penetration: true`. parry3d 0.27.0
  `cast_shapes_support_map_support_map` then computes a real contact normal for a toi below 1e-5
  (`shape_cast_support_map_support_map.rs:34-50`). `normal1` is the outward normal of the hit collider. The raw hits
  confirm it: a flush parallel wall reports `n·dir = 0`, and at 0.1 deg it reports -1.745e-3.
- The `INTO_TOLERANCE` const is a numeric law (float noise), not a tuning value. That is acceptable.
- Low (shake time): verified. `shake_camera` now uses `real.elapsed_secs_wrapped()`.
- Files: `melee.rs` 714 lines, `tests/melee.rs` 646 lines (both < 750).

### Runtime (BRP, real build)
- `t7.py` (`scratch/qa_a2/t7/summary.json`, `t7_run.log`): **PASS**, exit 0. Fist combo 100 → 60, `KnockedDown` 0.06 s
  after the third click, `Steady` 0.69 s later. Bat picked up, key 1 toggles `Fists`/`Bat`. Bat 100 → 75 and knocked down
  after 0.25 s. `log_errors` empty.
- `t6.py` (`scratch/qa_a2/t6/summary.json`): **PASS**, exit 0. Shooting is unchanged, `log_errors` empty.
- Screenshots I looked at:
  - `t7/combo_2.png`: player mid-kick, "10" fading and "20" over the middle dummy. The side dummies stand.
  - `t7/knockdown.png`: the middle dummy's legs lie on the ground behind the player's right side (knocked down), "20".
  - `t7/bat_knockdown.png`: "25" over the target, which is thrown back behind the player. The player is in the swing pose.
  - `t6/crit_hit.png`: "51 CRIT", ammo HUD 10/12, pistol held. The T6 visuals are intact.

## 3. Acceptance criteria

| Criterion | Test performed | Result |
|---|---|---|
| Headless: удар в окне попадает, вне окна нет | `hit_lands_only_in_window` (T0+8, none after the window); the 5 new geometry gates; my probes (walls behind/beside, bystander, diagonal contact, targets in line); 30/30 repeat runs | PASS |
| Headless: третий удар комбо сбивает с ног | `third_combo_hit_knocks_down`, `combo_resets_after_window` | PASS |
| Headless: knockback по направлению удара (−Z, +X, +Z) | `knockback_pushes_along_the_blow` (flipped RED in round 1 with an x/z swap) | PASS |
| Headless: hit-stop не меняет `Time<Virtual>` и число тиков | `character_gate::hit_stop_freezes_only_the_pair_and_not_time` (flipped RED in round 1) | PASS |
| Runtime QA: `t7.py` exists and passes | my run, exit 0, screenshots read | PASS |
| Owner-run: драка "хлёсткая", тряска не тошнит | owner checklist §6 | PENDING (owner) |
| Every new tuning value in its data file | fix diff: only `INTO_TOLERANCE` (numeric law) added; round-1 grep of the slice already PASS | PASS |
| `cargo build`, `clippy -D warnings`, `cargo test -p gta_sim` green | §2 | PASS |
| Existing tests pass | workspace green, no new failure names | PASS |

## 4. Bugs found

No new bug in the code under test.

### Notes (not bugs; for the owner or later slices)
- **Flush wall plus a swing leaning into it (owner feel).** See §0. A character pressed against a wall (Tnua leaves the
  capsule flush) spends the punch on the wall if the camera yaw points into the wall by 0.06 deg or more. In practice that is
  roughly half of the punches along a wall. This follows from `cast_radius` 0.35 > capsule 0.3 (GDD tuning). The fixer's
  owner note describes it correctly. Its "a wall a few cm away does the same" consistency claim is overstated (§0). If the
  owner dislikes it, the lever is data (`cast_radius` ≤ 0.3) or a later rule change. No action is needed for T7.
- **Tie at distance 0.** When two bodies overlap the sphere at the start and both pass the rule (for example a flush wall
  and a character touching the front), the first hit the BVH returns wins, because of the strict `<`. In my probe the
  character won. Another layout could let the wall win. A tie-break (prefer `Character`) would make this deterministic by
  design. Low, T9 crowd material.
- **Hygiene (not this stage):** `scratch/probe_target/` is a second cargo target dir (2.5 GB), left over from an earlier
  stage's probe workspace (`scratch/probe_ws`). It is git-ignored, and it goes against the "never a second `target/`"
  rule. The orchestrator can delete it at task close.

## 5. Verdict

**SHIP-PENDING-RUNTIME.**

Bug 1 is fixed. All three round-1 reproductions now hit the target in front. The new rule survived my attempts to
break it: diagonal contact up to 89 deg is hit, anything behind 90 deg is ignored, the sphere/capsule boundary gaps are
fine, and the nearer of two targets in line wins. Both of my independent flips of the new code went RED. Build, clippy
and the workspace suite are green, and the melee gates are stable across 30 runs. `t7.py` and `t6.py` pass on the real
build, and the screenshots match. The only remaining item is the owner-feel checklist below. It includes the
wall-lean behaviour, which is a tuning consequence, not a code defect.

## 6. Owner checklist (Russian)

`cargo run --release -- --seed 1`, тир в парке (манекены в 10 м от центра парка по −Z, бита в 2 м от центра по +Z):
- [ ] Без оружия (клавиша 1) три ЛКМ по манекену с интервалом ~0.3 с: читается комбо (правый, левый, пинок), третий удар сбивает с ног, манекен лежит и встаёт примерно через 1.2 с.
- [ ] Драка "хлёсткая": заморозка 50 мс на ударе заметна, но не выглядит как лаг.
- [ ] Тряска камеры на ударах заметна и не тошнит, в том числе при серии из трёх ударов.
- [ ] Быстрое "долбление" ЛКМ не раздражает (клик внутри идущего удара запоминается только один).
- [ ] Бита подбирается. Клавиша 1 без оружия переключает кулаки и биту. Удар битой тяжелее и сразу сбивает с ног. Бита в руке выглядит сносно.
- [ ] После нокдауна поза манекена полностью возвращается, ноги при переходе в idle не застывают.
- [ ] Встать вплотную к стене спиной, ударить манекен перед собой: удар попадает. Сосед, прижатый к спине, удар не получает.
- [ ] Встать вплотную к стене боком и бить вдоль стены. Если камера смотрит хоть немного в стену, удар уходит в стену. Так бывает примерно в половине случаев. Если это раздражает, есть рычаг в данных: `cast_radius` в `melee.ron` не больше 0.3 (радиус капсулы).

Tuning: `assets/combat/melee.ron`, `assets/juice/juice.ron` (`hit_stop_seconds`, `shake`), `assets/character/visual.ron`, `assets/world/render.ron`.

## 7. Cleanup and accounting
- Temporary files in the checkout: `crates/gta_sim/tests/qa_a1_probe.rs`, `qa_a2_probe.rs`, `qa_a2_diag.rs` were copied
  in, run, and deleted. The sources are in `scratch/qa_a1/` and `scratch/qa_a2/`. Both flips were restored with
  `git checkout`, and the sha256 matched. `git status --short` shows only this report. Scratch is git-ignored, and no
  binary asset is tracked.
- Game processes: none left (`tasklist`). No containers or services.
- Log: one `decision` entry appended.
- children: 0 launched / 0 reported.
