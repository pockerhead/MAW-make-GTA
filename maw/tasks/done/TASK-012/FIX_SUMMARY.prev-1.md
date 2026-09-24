# FIX_SUMMARY — TASK-012 (GDD T11): police on foot and arrest

Fixer: claude/opus, medium. Tree `feature/t11-police` @ `5d59f1b` + working-tree edits below. Binding input: last
OPEN_DECISIONS.md entry (fix all four issues + P9 doc comment, re-measure N1, gate the head_for change).

## Pre-flight

- `scratch/` read as a coverage map (implementer probes, flips, N1 plaza probe). Nothing of it re-run as evidence.
- Claim checked first, the one most likely to break correct code if applied verbatim: Issue 4 "gate `queue_slot`
  on `on_slot` like `clear_spot`". The risk: off-slot ticks with no plan would lose the queue slot and the rear
  shooter would walk into the front one again (the TASK-010 starvation). Checked in `tactics/fire_line.rs::unblock`:
  a slot found on the AI slot comes back as `kept_spot`. Off-slot, `plan` is kept with no re-check (`_ if !on_slot =>
  plan`), so the slot holds between slots. Only a shooter with no plan runs the fallback off-slot, and it gets the
  pre-task close-in seek for at most 3 ticks. The claim is safe. The 5 gang corridor gates stay green, and they
  still flip RED without the slot (F5 below).
- Second claim checked: Issue 1 names `landmarks.tower`. `CityLandmarks` (gta_sim) has no tower index. The index
  lives in `citygen::Landmarks::tower` (`layout.landmarks.tower`), so I used that.
- Issue 2's guard `walked && nearest_node(chest) == goal` was checked for oscillation. Voronoi cells are convex, and
  `dest` lies in the goal node's cell, so the straight leg goal → dest stays in that cell and the guard does not
  re-open the re-plan loop. The one gap left is a `dest` on a cell border (a tie between two nodes); see the Risks
  section.

## Fixed

1. **Issue 1 (major): N1 plaza spot inside the tower** (`crates/gta_sim/tests/police_city.rs`). The plaza spot is
   now the `citygen::sidewalk_anchor` of `layout.landmarks.tower`, using the hospital/station margin
   (`health.pickups.spacing`) at curb height. `chase` also asserts `GATE BROKEN` when the settled player is more
   than 0.1 m from where it was put. Flip F4 (plaza back to `plaza_center`): RED, `GATE BROKEN: plaza: player put
   at [40.85, 1.20, -41.06] stands at [40.85, 0.658, -41.06]`, which is the reviewer's sunk player. N1 was
   re-measured with the fixed code (`scratch/fixer_n1.txt`):
   - hospital: all 4 reached (2.48 / 3.78 / 5.56 / 7.39 s)
   - plaza: all 4 reached (1.61 / 3.94 / 5.33 / 5.41 s)
   - park: all 4 reached (1.28 / 1.92 / 2.20 / 6.31 s)

   No stuck cop at any spot. IMPL_SUMMARY §3 now has these numbers, and its old plaza numbers and plaza probe are
   marked void. The plaza line of the owner checklist is deleted. The corrected checklist is in IMPL_SUMMARY §4,
   and the QA stage copies it from there.
2. **Issue 2 (minor): re-plan hole after displacement** (`crates/gta_sim/src/tactics/mod.rs::head_for`). The age
   refresh is skipped only when the route is walked **and** `nearest_node(graph, chest) == Some(goal)`, as the
   reviewer suggested and OPEN_DECISIONS requires. `nearest_node(chest)` is evaluated lazily, only once the route
   has reached `route_refresh_seconds` of age.
3. **Issue 3 (minor): the straight leg after the last route node** (same function). The orchestrator asked to
   "bound" this leg. I read that as the reviewer's prescription: on this leg the walker uses `avoid_offset`,
   refreshed on its AI slot, exactly like a direct seek, and it no longer forces `avoid = 0` into a wall. The route
   leg and the no-route fallback still zero `avoid`. Alternative rejected (log `decision`): capping the leg length
   would force a re-plan back to the goal node, which is the oscillation the head_for change removed.
4. **Issue 4 (minor): queue slot rays every tick** (`crates/gta_sim/src/tactics/fire_line.rs::unblock`).
   `queue_slot` now runs only on the AI slot (`on_slot.then(..)`), and the `unblock` doc says so. `pinned` casts no
   rays (it is pure geometry over `spots`), so it is unchanged.
5. **P9 doc comment** (`crates/gta_sim/tests/police_arrest.rs`). It names both guards (`Without<Dead>` in
   `arrest_player` and the FSM's dead-player filter), says either one is enough, and says the flip removes both.
6. **Gate for the head_for change** (new `crates/gta_sim/tests/route_walk.rs`, 3 tests). The walker is a patrol cop
   in `Respond` on the test floor, with graph X(-30,-34)–Y(-20,-34)–Z(-20,-10)–G(-30,-10). The player (last known
   position) is at (-30, 35): 45 m past G, which is beyond `direct_seek_distance` 25, so the cop walks the route.
   "Reached" means the cop sees the player (`cop_view_distance` 35 m) or comes within 20 m of it. Every shipped
   number used is asserted `GATE BROKEN` (navigation and police).
   - `walker_goes_on_past_the_goal_node`: the cop starts at G and must get past it within 10 s. This is the
     oscillation fix.
   - `displaced_walker_replans`: the orchestrator's case. The cop finishes its route (checked with `GATE BROKEN`:
     goal G, `next >= len`, still `Respond`). A named teleport then moves it into a dead-end pocket at (-30, -24).
     The pocket's back wall faces the player and its side walls are 1.5 m away, so every `avoid_offset` heading is
     blocked. Its nearest node is X, and the goal node stays G. The cop must get out within 30 s.
   - `walker_past_the_goal_node_avoids_walls`: a 6 m wall across the leg at z = 0. The cop stays nearest to G the
     whole time, so only avoidance helps. It must reach the player within 20 s. This gates Issue 3.

   Flip record (`scratch/fixer_flips.py`, output `scratch/fixer_flips.txt`; each flip is an in-memory swap of one
   mechanism, then the file is restored):

   | Flip | Perturbed input | Result |
   |---|---|---|
   | F1 | head_for refresh reverted to the pre-task `route.age >= refresh` (the head_for change undone) | `walker_goes_on_past_the_goal_node` RED (closest 40.7 m: the cop oscillates at G); `..._avoids_walls` RED; `displaced_walker_replans` GATE BROKEN (it can never finish its route) |
   | F2 | guard without `nearest_node(chest) == goal` (the implementer's version) | `displaced_walker_replans` RED (stuck in the pocket, closest 57.8 m); the other two green |
   | F3 | no avoidance past the last node | `walker_past_the_goal_node_avoids_walls` RED (pressed into the wall at 35.5 m); the other two green |
   | F4 | N1 plaza back to `plaza_center` | `GATE BROKEN` sunk-player assert RED (y 0.658) |
   | F5 | `queue_slot` result dropped (with on-slot gating) | gang corridors 5/5 RED |

   All green after restore (full suite below).

## Skipped

- Nit: `police_fsm` is about 275 lines, and the reviewer suggests an Attack-arm helper. It mirrors `gang_fsm`, the
  code is correct and no gate asks for the change, so I left it (surgical-change rule).
- Nit: `unit.sees &= live_player.is_some()`. This is cosmetic. `behavior.rs` was not otherwise touched, so it stays
  as it is.

## Test results

| Command | Result |
|---|---|
| `cargo test -p gta_sim -j 4 --test route_walk` | 3 passed |
| `cargo test -p gta_sim -j 4 --test police_city -- --nocapture` | 2 passed; N1 printout above (`scratch/fixer_n1.txt`) |
| `cargo test -p gta_sim -j 4` | **292 passed, 0 failed** (289 + 3 new) (`scratch/fixer_sim_tests.txt`); bench 12 SWAT + 40 civ mean 0.909 ms, p95 1.05 ms |
| `cargo test -p citygen -j 4` | all green (9 + 3 + 14 + ..., 0 failed) |
| `cargo test -p gta_like --bin gta_like -j 4` ×3 | 45 passed ×3 (`scratch/fixer_client_x3.txt`) |
| `cargo clippy --workspace --all-targets -j 4 -- -D warnings` | clean |
| `cargo build -j 4` | ok |
| `rustfmt --edition 2024 --check` on every edited/new file | clean |
| `python tools/qa/tree_check.py` | passed |

File sizes: `tactics/mod.rs` 227, `fire_line.rs` 222, `route_walk.rs` 159, `police_city.rs` 132,
`police_arrest.rs` 517 (all < 750).

Not re-run by the fixer: `t11.py`/`t10.py`/`t9.py` runtime QA (windowed release build; QA stage). The head_for
avoidance on the walked leg changes how gangs and cops walk in the city, so QA should re-run t11 and watch the
`stuck` list.

## Risks left (not gated)

- If `dest` lies exactly on the border between the goal node's cell and a neighbour's (a tie in `nearest_node`),
  the walker near `dest` can drift into the neighbour's cell. The walker then re-plans back to G once per refresh.
  This only happens next to `dest` (a cop enters `Search` within 3 m of it).
- Avoidance on the walked leg can take the walker around a building into another node's cell. It then re-plans
  along the graph, which is the pre-task behaviour and not a wall press.

`git status --short`: `crates/gta_sim/src/tactics/{mod,fire_line}.rs`, `crates/gta_sim/tests/{police_arrest,
police_city}.rs`, new `crates/gta_sim/tests/route_walk.rs`, `maw/tasks/in_progress/TASK-012/{log.jsonl,
IMPL_SUMMARY.md, FIX_SUMMARY.md}`. Every path is accounted for.

children: 0 launched / 0 reported.
