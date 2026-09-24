# Implementation review — TASK-024

## Verdict

**NEEDS_WORK** — the wanted-star HUD loses the earned count for half of each blink cycle on the tested light wall, contrary to the star-readability acceptance criterion.

## Disconfirmation tested

Counterexample: a civilian 18–20 m from a player punch receives the fight stimulus but never completes a report, leaving heat at zero. I traced `MeleeHit` into `StimulusLog`, the 20 m fight radius in `perceive`, and the fight-specific 15 m report threshold in `choose_reaction` (`crates/gta_sim/src/perception/mod.rs:203`, `:264`; `crates/gta_sim/src/civilian/reaction.rs:24`). The production-composition gate in `crates/gta_sim/tests/wanted.rs:150` checks the completed call and exactly 5 heat. Its recorded flip to the old 25 m threshold failed and its restored run passed (`scratch/flips_result.txt`). This counterexample did **not** hold for the gate's report-prone civilian; ordinary civilians still choose among reactions by temperament, as designed.

## Confirmed correct

- The new star colors and shadow are data in `assets/ui/strings.ron:23`, validated in `src/menu/config.rs:73` and applied by the presentation system in `src/hud/stars.rs:69`. `TextShadow` has the used `offset` and `color` fields in pinned `bevy_ui` 0.19.1 source. The supplied bright-phase sky, wall and street screenshots show a stronger earned/empty distinction; the logged 0.82-grey dead end is visible in `scratch/stars_try_gray082/zoom_wall_heat180_0.png` and the 0.72 result in `scratch/stars_after/zoom_wall_heat180_0.png`.
- `crates/gta_sim/tests/wanted_search.rs:142` covers row 2 with player geometry, and `:200` covers rows 3–5 with controlled circle centres and exact clear times. The recorded `rows[0]` and first-row-clear-time flips failed these gates (`scratch/flips_result.txt`).
- The shipped fight radius and separate report threshold are in `assets/npc/perception.ron:4` and `assets/npc/civilian.ron:16`. The headless punch gate checks a real melee hit, a completed call, and 5 heat (`crates/gta_sim/tests/wanted.rs:150`). The changed private-punch gate still checks that a later body call does not report the earlier punch (`:171`).
- `tools/qa/scenarios/t10.py:127` screens civilians near the shot line, and `:208` retries a missed kill from another stand point. The supplied `scratch/t10_runs.txt` records five consecutive exit-0 runs; each recorded a kill, a completed call, heat 50 and no log errors. The supplied `scratch/test_sim_citygen.txt` and `scratch/test_client_x3.txt` show the required test suites green, including three client runs. The extra spaces in the call-delay error are removed at `crates/gta_sim/src/wanted/mod.rs:136` and checked at `crates/gta_sim/tests/config.rs:702`.

## Issues

1. **Major — `src/hud/stars.rs:34`, `:100`: earned stars become identical to empty slots in the dark blink phase.** `star_look` returns `Off` for both an unearned slot and an earned but currently dark slot. `update_stars` then gives both the same `off_color` and `Color::NONE` shadow. The supplied `scratch/stars_after/zoom_wall_heat180_2.png` shows no countable earned stars on the light wall, while `zoom_wall_heat180_0.png` shows two in the bright phase. This repeats for a 0.25 s phase every 0.5 s until police see the player, so the earned count is unavailable half the time. **Suggested fix:** retain a distinct outline or faint shadow for earned stars through the dark phase while preserving the grey blink, then capture and inspect both phases on all three backgrounds with the owner.

## Missing coverage

- The star evidence measures the best contrast frame. Add dark-phase screenshots to the acceptance evidence and have the owner judge whether the earned count remains readable; this is a visual acceptance check, not a headless color test.
- The new fight threshold is validated as finite/nonnegative in `crates/gta_sim/src/civilian/mod.rs:88`, but no shipped-config check enforces `fight_report_min_distance < fight_hearing_radius`. A future data edit can silently make punches unreportable again. Add a cross-config validation or a gate that perturbs this relationship across the failure boundary.

children: 0 launched / 0 reported.
