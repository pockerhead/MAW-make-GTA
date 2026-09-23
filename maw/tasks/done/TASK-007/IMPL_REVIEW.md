# Implementation review — TASK-007

## Verdict

**NEEDS_WORK** — the headless shooting slice works, but the explicitly added aim-camera and visible-tracer acceptance criteria fail in the captured runtime evidence.

## Disconfirmation tested first

Counterexample: an RMB-aimed SMG burst at a dummy could leave the player's head covering the crosshair area while no tracer is visible. I checked `scratch/qa_t6/aim_burst_1.png` and `aim_burst_2.png` against `src/camera/mod.rs`, `assets/camera/camera.ron`, `src/vfx/mod.rs`, and `assets/juice/juice.ron`. **It holds:** the head occupies much of the left-centre view and crowds the target/crosshair, and neither capture shows a readable tracer. The implementation summary independently describes both defects, but the screenshots and code are the evidence.

The only `dead_end` log entry points to overlapping screenshot requests. `tools/qa/scenarios/t6.py:286-288` now spaces the two calls by 0.15 s; the two PNGs exist, so that logging pointer is resolved for this review.

## Confirmed correct

- `crates/gta_sim/src/combat/hitscan.rs:129-244` implements the two-ray muzzle shot, collider filtering, one trace per pellet, and a single rolled integer passed to `Health::take` and `DamageDealt`. `crates/gta_sim/tests/shooting.rs` covers body hits, walls, head hits, reload, shotgun rays, spread, pickups, and variance through the production composition.
- `crates/gta_sim/src/character/mod.rs:95-127` installs a child head sensor with separate collision layers. The headless ground-sensor test passes.
- `crates/gta_sim/src/flow/wasted.rs:126-129` clears queued debug damage on exit from Wasted; `crates/gta_sim/tests/respawn.rs:260-296` reproduces the last-frame case and passes.
- `src/juice/damage_numbers.rs:82-161` uses the sim's reported value, formats headshots with the configured CRIT string, and despawns labels on real time. The client gates and the runtime health/number checks pass.
- Fresh commands in this review: `cargo test -p gta_sim` and `cargo test -p gta_like --bin gta_like` passed; `cargo tree -p gta_sim -e normal -i bevy_render` found no render dependency.

## Issues

1. **Major — `src/camera/mod.rs:117-143`, `assets/camera/camera.ron:12-13`: aim framing obscures the sight line.** The code uses a 0.55 m aim shoulder offset and a 2 m camera distance without any close-player fade/hide. In both `aim_burst` captures the player's head dominates the left-centre and overlaps the area immediately beside the crosshair and target. This directly fails the owner-proxy addition in `TASK_FINAL.md`. Adjust the data-driven framing or close-camera player visibility, then capture an aimed screenshot with the target fully visible at the crosshair.
2. **Major — `src/vfx/mod.rs:79-104`, `assets/juice/juice.ron:5`: the tracer is not readable.** A 2 cm cuboid lasting 60 ms is viewed nearly end-on during the recorded burst; neither burst capture shows one. This directly fails the explicit requirement to show a tracer in a T6 screenshot. Tune width/lifetime and, if needed, the visual geometry in the client, then demonstrate a visible tracer in the runtime capture. Keep tuning values in `juice.ron`.
3. **Major — `tools/qa/scenarios/t6.py:284-300`: runtime QA can pass with both visual failures.** The script saves two screenshots and checks aim state, ammo, and health, but does not inspect or require a clear target or visible tracer. Its recorded `summary.json` says the run passed despite the failures above. Add a reviewable image check/owner-proxy assessment to the scenario handoff and require at least one accepted aim-and-tracer capture before reporting T6 complete. Pixel assertions are unnecessary; the visual acceptance must still be enforced.

## Missing coverage

- A successful aimed BRP capture showing the entire target clear at the crosshair.
- A successful T6 capture with a readable tracer. Existing screenshots establish the opposite result.
- The owner-run feel checklist belongs in the later `QA_REPORT.md` stage; the current review does not substitute for that run.

No sub-agents were launched: **children: 0 launched / 0 reported**.
