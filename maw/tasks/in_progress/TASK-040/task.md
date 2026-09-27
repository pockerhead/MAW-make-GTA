# TASK-040: Second agent playtest (regression pass after TASK-032..038)

Type: research
Mode: playtest (one-off, orchestrator-run; not a MAW stage chain)
Priority: high
Branch: none (report only; no code changes)
Domains: game-design, gates

## Description
TASK-031 (the first agent playtest) found M1-M4 and P1-P3. Since then TASK-032, 033, 034, 035, 036, 037 and 038 changed traffic, police, camera and lethality. Before TASK-039 (the U-turn for fully blocked exits) or a final v0.1.0, measure what a player now sees. The orchestrator decides the next step from this report (the owner delegated all decisions).

Same playtester design as TASK-031 (maw/tasks/done/TASK-031/task.md, "Playtester design"): black-box, three personas (tourist, reckless, explorer), generic oracles, reproducible findings, false-positive baseline. Reuse the harness in D:/test-gta-like/maw/tasks/done/TASK-031/scratch/tools/ (pt.py, personas.py, analyze.py, repro scripts). Copy it into this task's scratch and fix its paths.

Scope: seeds 1, 7 and 42, each persona about 8-10 min of in-game time (shorter than TASK-031; this is a regression pass). Build from main HEAD (release + dev features). Use --settings-id .qa, take BRP screenshots one at a time (concurrent screenshots crash the game), run one game at a time, and never kill the owner's game.

Focus (compare with TASK-031's numbers in maw/tasks/done/TASK-031/PLAYTEST_REPORT.md):
- M1: an abandoned or bumped car freezing traffic (TASK-031: columns up to 16 cars, 100-160 s). Tourist: leave the car mid-lane AND inside a junction, watch 90 s from the sidewalk.
- M2: the camera at walls (the model should hide).
- M3: lethality (1★ police do not fire unless attacked; standing TTK; traffic car hits).
- Police chase feel with sirens (traffic yields, police use any lane, spawn ahead).
- Walkers near standing cars (no orbiting, no pinning).
- Anything new that looks wrong, including the car sliding sideways during passes (TASK-037 heading cap) as a feel item.

Artifact: maw/tasks/in_progress/TASK-040/PLAYTEST_REPORT.md (Russian), with a per-finding before/after comparison against TASK-031. Evidence goes under scratch/.

## Acceptance criteria
- [ ] 3 seeds × 3 personas played; a per-session metric table.
- [ ] Each TASK-031 finding (M1-M4, P1-P3) is re-rated: fixed / improved / same / worse, with evidence.
- [ ] New findings carry a severity, evidence and a repro replayed once.
- [ ] No source code read or changed.
