# FIX_SUMMARY — TASK-039, fixer round 3 (QA B1: two standing bodies in a row)

Scope: the orchestrator note of round 3 (QA_REPORT.prev-1.md B1 and the last OPEN_DECISIONS.md entry are
binding). IMPL_REVIEW.md (round 0, commit 4699d94) was read; its items were closed by rounds 1-2 and are
not re-opened here (see Skipped).

Preflight, the review claim that would break correct code if applied verbatim: IMPL_REVIEW §0/C1 says
the plan's "past only for a kinematic car" guard must come back. Checked in `progress.rs::upkeep`: the
guard was removed in round 2 by measurement (OPEN_DECISIONS 2026-09-28: "past guard alone ... no
observable effect on both platforms"); restoring it would be a change without evidence. Not applied.

## Fixed

**B1 (MAJOR): the car relaxed against body 1 stops inside it before body 2 and stands 69-80 s.**

Root cause (verified by running the QA probe as a gate on HEAD, `scratch/fixer3_chain_head.txt`): two
holes, not one.
1. `candidates` `free` allowed a new relaxation only with no relaxation or a physics-only one against
   the SAME body. The car's edge points at body 2, so it is never a candidate. In the 1 m case the
   relaxation against body 1 never even leaves planning (the car is not past it, body 1 stands), so the
   orchestrator's literal "physics_only against X" alone would wait for the 30 s `max_seconds` stale
   guard: a ~28 s stand before the re-target.
2. After a re-target, sensing and the path leader skipped only the new blocker. With a 1 m gap body 1
   still reaches past the car's nose, so the car senses it ahead and stops again (measured: re-targeted
   at 26.2 s, still standing at 90 s).

Change (`crates/gta_sim/src/traffic/`):
- `mod.rs`: `Relax.trailing: Option<Entity>`, the previous blocker the car is still inside;
  `Relax::exempts(other)` = blocker or trailing.
- `progress.rs`
  - `candidates` `free`: a car may accept a relaxation against a different body when its relaxation is
    physics-only, or when it is already inside its blocker (`penetration > 0`, the stuck-inside case).
    Same-body re-planning is unchanged.
  - `accept`: the old blocker becomes `trailing` of the new relaxation.
  - `upkeep`: `trailing` is kept while the skin-grown footprint touches it and dropped when they
    separate (or it is gone); if the main relaxation ends while `trailing` still touches, `trailing`
    becomes a physics-only relaxation of its own, so contacts never resume mid-overlap. The markers
    carry both bodies; `TnuaNotPlatform` if either is a person.
  - `exempt_pair` (contact hook, switch skip) uses `exempts`.
  - New `sensing_skips(snap, other)`: while the car plans, sensing and the path leader skip its blocker
    and its trailing body (it can only drive out of it forward).
- `manoeuvre.rs::sense` and `drive.rs::leader` use `sensing_skips` (leader's `skip` is now a predicate).
- `drive.rs`: the (blocker, passer) list for recovery includes the trailing body.
- `vehicle/mod.rs`: `PassingThrough { blocker, trailing }` with `names(e)`; `vehicle/chassis.rs` wheel
  rays skip both.
- Tests: `third_body.rs::relaxed_pairs` lists (car, trailing) too, so the oracle exempts the pair while it
  is still overlapping; the four hand-set `Relax` literals get `trailing: None`.
- Limitation: one trailing body. A third standing body inside the car's length at the second re-target
  would drop the first from the exemption (not seen in any row).

Log: one `decision` entry (trailing field, and why the re-target also fires while planning inside the
blocker).

**Gate: `crates/gta_sim/tests/traffic_progress_chain.rs::car_gets_past_two_standing_bodies_in_a_row`**
(liveness + correctness), from the QA probe `scratch/qa/final/qa039_two_in_a_row.rs`.
- Scenes: loop floor (no neighbour lane) with body 2 a parked car or a dummy at 1 and 3 m; the two-way
  street with a feeder on the oncoming lane every 3 s: 1 m car, 3 m dummy. `GATE BROKEN` if a body is
  more than 0.3 m off its spot after spawn.
- Asserts (all collected before the panic): `scene_failures` (no AI car > 30 s standing within 45 m, no
  AI car > 30 s `Dynamic`, G1, third bodies, relaxation start/end checks); at least one car relaxed
  against body 1; every car relaxed against body 1 has its rear past body 2 within the derived bound
  after that relaxation.
- Bound, derived from config (not from the measurement): leg 1 from `min_gap` behind body 1 to `min_gap`
  before body 2 (`d/v + v/a + v/b`), `grace_seconds`, leg 2 until the rear is past body 2 (`d/v + v/a`),
  with `v` = `pass.speed` (the squeeze cap), `a`/`b` the IDM acceleration and comfortable deceleration.
  19.6-20.5 s; measured 10.2-15.7 s.

Measured (Windows, fixed code):

| scene | relaxed vs body 1 | rear past body 2 | worst stand near |
|---|---|---|---|
| loop 1 m car | 15.9 s | 30.1 s | 9.4 s |
| loop 3 m car | 15.9 s | 30.9 s | 9.3 s |
| loop 1 m dummy | 16.0 s | 29.3 s | 9.4 s |
| loop 3 m dummy | 16.0 s | 30.1 s | 9.3 s |
| street 1 m car | 3 cars: 51.6/65.0/116.2 s | 67.3/80.0/130.4 s | 17.1 s |
| street 3 m dummy | 3 cars: 51.6/63.8/117.9 s | 67.1/74.0/132.3 s | 15.0 s |

Flip-RED (each restored and re-run GREEN; outputs in `scratch/`):
- Old `free` condition (`physics_only && blocker == edge.target`), i.e. HEAD behaviour: RED in all 6
  scenes: the loop car stands 69-70 s inside body 1, the "still relaxed 2561 ticks" check fires, the
  street car stands 80 s (`fixer3_flip_old_free.txt`; HEAD itself: `fixer3_chain_head.txt`, same).
- Sensing skips only the new blocker (not `trailing`): RED in the 3 scenes with a 1 m gap (the car
  re-targets but senses body 1 and never moves) (`fixer3_flip_sensing_trailing.txt`).
- Physics exemption without `trailing` (`exempt_pair` only the blocker): RED in 5 of 6: contacts with
  body 1 resume mid-overlap, the car switches `Dynamic` and stands 37-73 s
  (`fixer3_flip_trailing_contacts.txt`).

## Skipped

- **The street scene with a 3 m gap and a parked car as body 2** is not in the gate. It is RED on HEAD
  and on the fixed code the same way, and the cause is not B1: a feeder car (238) makes a lane pass
  around body 1 in the oncoming lane; body 2 sits in its merge zone (merge_s 26.6, body 2 at s 27.1-31.1),
  so it keeps the oncoming offset (-2.83) to the lane end, enters the U connector off its path, goes
  `Dynamic` and stands 109 s there (trace `scratch/fixer3_trace_street3.txt`, t 46 s on). It is never a
  relaxation candidate (no out-edge). Per the orchestrator ("this is the last traffic change; anything
  further goes to known limitations") this is a **finding for known limitations**: a lane pass whose
  merge zone holds a second standing body can run to the lane end in the oncoming lane. In the city it
  would reach the junction box at an offset. Not fixed here.
- IMPL_REVIEW.md items (C1 and the rest): written against 4699d94 and dispositioned in rounds 1-2 and by
  the orchestrator (OPEN_DECISIONS 2026-09-28). This round is scoped to B1 by the orchestrator note.
- QA B2 (symmetric exemption has no Windows gate): the orchestrator kept it without a new flip
  (OPEN_DECISIONS, item (1)). Not touched.

## Test results

Code commit: `1a011e8` on `feature/box-uturn` (pushed). FIX_SUMMARY.md and log.jsonl are left
uncommitted for the orchestrator; `scratch/` is git-ignored (the flip and trace outputs are on disk).

- Full suite, Windows, once: `cargo test -j 2 -p gta_sim -p citygen` -> exit 0, 638 passed, 0 failed,
  7 ignored (`scratch/fixer3_win_full.txt`; QA's round had 637, +1 the new row). Includes
  `traffic_bench`/`police_bench`/`civilian_bench` under their `MEAN_LIMIT`.
- `cargo test -j 2 -p gta_like --bin gta_like`: 82 passed, 0 failed.
- `python tools/qa/tree_check.py`: tree checks passed.
- CI (Linux) on `1a011e8`: 5/5 success. Sim gates run the new row on Linux.
  - repo checks https://github.com/pockerhead/MAW-make-GTA/actions/runs/36405969204
  - citygen gates https://github.com/pockerhead/MAW-make-GTA/actions/runs/36405969238
  - clippy https://github.com/pockerhead/MAW-make-GTA/actions/runs/36405969242
  - client gates https://github.com/pockerhead/MAW-make-GTA/actions/runs/36405969349
  - sim gates https://github.com/pockerhead/MAW-make-GTA/actions/runs/36405969290

- Traffic targets, Windows (`cargo test -j 2 -p gta_sim --test <each of 20 traffic targets>`): all green,
  `scratch/fixer3_win_traffic.txt` (config_traffic 6, bailout 3, box_overhang 8, bubble 4, causes 13,
  contact 6, go_around 11, graph 2, gridlock 5, hijack 2, idm 1, intersection 7, junction_box 7,
  occupancy 10, parked 1, pedestrian 5, progress 2, progress_chain 1, progress_plumbing 10, recovery 10;
  0 failed, 0 ignored).
- `cargo clippy --locked -p gta_sim -p citygen --all-targets -- -D warnings`: clean.
- `cargo clippy --locked --workspace --all-targets -- -D warnings`: clean.
- `cargo fmt -p gta_sim --check`: clean.

## Children

children: 0 launched / 0 reported
