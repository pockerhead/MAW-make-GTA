# TASK-031: Плейтест прототипа агентом-игроком (однократно)

Type: research
Mode: playtest (one-off, orchestrator-run; not a MAW stage chain)
Priority: high
Branch: none (report only; no code changes)
Domains: game-design, gates

## Description
Owner (2026-09-25): "после 16 эту стадию как раз добавим однократно чтобы плейтестер поиграл в прототип и записал находки". The experiment is full autonomy, so the owner's "five minutes of play" (which found the curb bug, the empty street, the missing wanted for a kill, 63 s police response) is replaced by an agent that plays the release build as a player.

The playtester:
- does NOT read the source code or the task history (only README "Запуск"/controls, GDD §1 (the 11 player-facing points) and GDD §2-§8 player-visible rules);
- plays seed 1 and two other seeds through BRP (keys/mouse held like a player, camera turns, screenshots every few seconds), 10-15 min of in-game time per seed, following scripted-but-natural sessions: walk the city, fight on foot, shoot, get wanted, escape/arrest/die, steal a car, drive through traffic, crash, hop curbs, chase with police, gangs on their turf, pause/new city/settings;
- records player-side metrics per session: civilians/cars in view, time to first star after a witnessed crime, police arrival time, deaths/arrests causes, stuck states (player, NPC, car), jams, frame cost (frame_report), anything that looks wrong on screenshots;
- judges each finding against GDD expectations and "GTA-like feel" and assigns severity (blocker / major / minor / polish) with evidence (screenshot path, metric, repro steps).

Artifact: `maw/tasks/in_progress/TASK-031/PLAYTEST_REPORT.md` (Russian), evidence under `scratch/`. The orchestrator triages findings into follow-up tasks (decides, does not ask the owner).

## Acceptance criteria
- [ ] ≥ 3 seeds × ≥ 10 min in-game sessions played; per-session metric table.
- [ ] Every GDD §1 point exercised and rated (works / partly / broken) with evidence.
- [ ] Findings list with severity, evidence and repro; screenshots referenced.
- [ ] No source code read or changed by the playtester.

## Dependencies
- blocked by TASK-017

## Playtester design (from online best practices, 2026-09-25)
Sources: kevinnie2003/playtest-agent (personas + generic oracles + reproducible findings), emergentmind "LLM agents as game testers" survey, GBQA benchmark (arXiv 2604.02648), GamingAgent (ICLR 2026).
- **Personas, not one policy.** Run each seed with 3 personas: *cautious tourist* (walks, observes city/NPC life, obeys traffic), *reckless* (shoots, fights, runs over, never heals, max wanted), *hoarder/explorer* (every pickup, every car, edges of the map, pause/new city/settings). Personas raised bug recall 0.38→1.0 in the reference agent.
- **Generic invariant oracles on every sample, not LLM judgement:** position in bounds / not inside geometry; health/ammo/heat never negative or NaN; input changes something (player responds); no entity count growth; no NPC/car stuck > N s; frame cost under budget; no panics/errors in the log. Oracles flag; the LLM triages.
- **Player-side metrics (the "owner's eye"):** civilians/cars in view, time-to-star after a witnessed crime, police arrival time, deaths/arrests by cause, stuck durations, jam lengths — compared to GDD expectations.
- **Reproducible findings:** every finding stores the exact scripted action list + seed from reset and is replayed once to confirm; unconfirmed = reported as "not reproduced", never as a bug.
- **Bounded LLM use:** scripted exploration drives the session; the model is consulted at stalls, at periodic check-ins (screenshot + state summary), and once at the end to rank severity, dedupe by kind and write root-cause hypotheses.
- **False-positive control:** before judging, run the oracles on a calm 2-min baseline (no player input) — anything firing there is an oracle bug, not a game bug.
- **Visual judgement via screenshots with a checklist** (readability, clipping, HUD, T-poses, floating props), multimodal; hallucination guard: a visual finding needs a second screenshot or a metric that agrees.
- **No code reading** (black-box), structured report: severity, persona, seed, repro, evidence (screenshot path / metric), GDD §1 point affected.
