# PCTX proposals — TASK-038

## 2026-09-27 — bevy-ecs, risk lesson: the sim is deterministic per platform, not across platforms

**What.** Add to `domains/bevy-ecs.md` (risk lessons):

> **Windows and Linux runs of one seed diverge (TASK-038).** Each platform repeats itself bit for bit
> (std `HashMap` `RandomState` reseeds per process and changed nothing), but std `f32` `atan2` / `sin` /
> `cos` differ by 1-2 ULP between MSVC UCRT and glibc (`sqrt` is identical), so car rotations differ right
> after city load and a 150 s traffic scene ends in a different place. A city gate green on Windows can
> be red on the Linux CI runner for a latent bug the other trajectory reaches (G1 pass-through, a node-141
> lock). Reproduce CI reds in WSL Ubuntu 22.04 with toolchain 1.95.0 (mirror the sources to the WSL file
> system, `CARGO_TARGET_DIR` there), not by rerunning on Windows. Trigger: a city/traffic gate red only in CI.

**Why.** Measured: `scratch/libm_probe.rs` (1e6 inputs, per-function hash: atan2/sin/cos/atan/acos
differ, sqrt equal), `scratch/probe_*_d3.txt` (first difference at tick -3: 1-2 ULP in parked/traffic
car `Rotation`), two runs per platform identical (`scratch/*_go_around_run{1,2}.log`). Making the
platforms agree would need glam/bevy_math `libm` (avian `enhanced-determinism`) *and* replacing every
std `f32` transcendental in `gta_sim` (`combat::aim_yaw` is `f32::atan2`, a lot of `sin_cos`); that is a
project-wide choice with a perf cost, out of this hotfix. Until then the rule is: CI is the second
platform, and a Linux-only red is a real bug on an untested trajectory, never "flaky".

## 2026-09-27 — gates, risk lesson: conflicts are about bodies, not centre lines

**What.** Add to `domains/gates.md` (risk lessons):

> **Junction conflicts from centre lines miss turning corners (TASK-038).** Centre lines 3.25 m apart
> (`2 x 1.2 + 0.3` clear) still let two yawed 4.08 m bodies touch: 198 of 2340 non-conflicting connector
> pairs on seed 1 (depth up to 0.48 m), found by an unmargined oracle (`traffic_graph::cars_granted_together_never_touch`).
> Any "can these two paths be used at once" rule is checked with the swept body rectangles, nose-in to
> rear-out. Trigger: a geometric predicate over `points` / centre-line distance used for two bodies.

## 2026-09-27 (fixer) — gates, risk lesson: a swept-body sampling step bounds the centre, not the corners

**What.** Add to `domains/gates.md` (risk lessons):

> **Sampling a turning body every X m along its path moves its corners much further (TASK-038).** At a
> 0.2 m centre step on a citygen right turn (~2 m connector, radius ~1.7 m) the corners of a 4.08 m car
> move 0.83-0.98 m between samples, so "the gap between samples is thinner than the margin" is false as
> an argument. Here the table still holds (unmargined oracle at 0.03 m: 0 touches on seeds 1..8, even with
> a 0.01 m margin), but the guarantee is the fine oracle gate, not the step. When a sampled sweep claims a
> bound, measure the corner displacement between samples. Trigger: a `*_SAMPLE_STEP` / sweep over poses.

**Why.** `scratch/zz_fix038_step.rs` (`probe_step`, `probe_margin_calibration`),
`scratch/fix_margin_calibration.log`. The review's prescription (a config floor on `conflict_margin`
tied to the step) had no derivable number: the rigorous bound would reject the shipped 0.3, the
measurement shows no margin dependence down to 0.01.
