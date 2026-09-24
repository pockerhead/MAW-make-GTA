# TASK-024: Розыск — полировка после QA T10

Type: fix
Mode: small-fix
Priority: high
Branch: fix/wanted-polish
Domains: bevy-ecs, gates, game-design

## Description
Follow-ups from QA TASK-011 (`maw/tasks/done/TASK-011/QA_REPORT.md`, findings 1-5), so T11 police start on a clean wanted core:
1. **Star readability.** Until police exist, stars always blink grey; on a bright wall an earned star (~RGB 140) is not distinguishable from an empty slot (~163). Retune `assets/ui/strings.ron` `hud.stars` (earned vs empty contrast, outline/shadow if the HUD supports it) so earned stars are countable on sky, light walls and dark streets. Evidence: screenshots on the three backgrounds, before/after.
2. **Coverage.** Move QA's `qa_second_star_row_governs_search` (`maw/tasks/done/TASK-011/scratch/qa/qa_wanted_probe.rs`) into `crates/gta_sim/tests/wanted_search.rs`; add rows 3..5 as a table-driven case. Flip-RED with `rows[0]` in `search_step`.
3. **t10.py flake** (1 of 4: the shot did not kill the victim — likely line blocked by another civilian or a tree). Make the kill deterministic: clear line check / reposition before firing, or a named health mutation; 5 consecutive green runs.
4. **Punches never phoned in.** Fight hearing radius 15 m (`fight_hearing_radius`) < `report_min_distance` 25 m, so "punch a civilian = 5 heat" is only counted by a cop. Decide per GDD §6.2/§6.4 intent: a witness who SEES a fight (sight, not hearing) within the corpse-sight range can report it; or lower `report_min_distance` for fights. Pick the smallest data-first change consistent with the GDD, gate "a punch seen by a civilian 18-20 m away is reported → 5 heat".
5. Trivial: the 14-space run in the `validate_call_delay` error message (`crates/gta_sim/src/wanted/mod.rs`).

## Acceptance criteria
- [ ] Screenshots: earned vs empty stars distinguishable on sky, light wall, dark street (owner judges; QA reports luminance delta).
- [ ] `wanted_search` gate covers rows 2..5; flip-RED shown.
- [ ] `t10.py` green 5 runs in a row.
- [ ] Fight-report gate green with flip-RED; existing wanted gates stay green.
- [ ] `cargo clippy --workspace --all-targets -- -D warnings`, `cargo test -p gta_sim -p citygen -j 4`, `cargo test -p gta_like --bin gta_like` (3 runs) green.

## Dependencies
- blocked by TASK-011
