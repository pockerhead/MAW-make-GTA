# TASK-001: Research and write the game design document

Type: chore
Mode: deep-research
Priority: high
Branch: chore/game-design-document
Effort: planner=high, plan-reviewer-2=high
Domains: game-design

## Description
Produce the design document (GDD) for a single-player third-person GTA-like action game built on
Rust + Bevy 0.19, plus a decomposition of it into implementation tasks. The owner's brief: an open,
procedurally generated, varied city world (streets, buildings, parks and other districts); physics;
shooting and melee combat; third-person camera; police, gangs/bandits and civilians; "roughly like
GTA"; the goal is a good PLAYABLE PROTOTYPE built in one continuous autonomous run by MAW agents
(no human artists, no hand-made levels). The repo is empty — there is no code yet.

The report (PLAN_FINAL.md) is the GDD draft itself, written in Russian, and must cover:
1. Pillars and the "prototype done" definition: what the owner can do in the running game at the end.
2. World: procedural city generation approach (road graph / blocks / lots / districts / parks),
   scale in metres, streaming or not, landmarks, variety levers, determinism by seed.
3. Player: on-foot movement (walk/run/sprint/jump/crouch?), camera rig, controls (keyboard+mouse,
   gamepad optional), health/armor, death and respawn.
4. Combat: ranged (hitscan vs projectile, aiming, weapons roster for the prototype, ammo),
   melee (fists + one melee weapon, hit detection, reactions), ragdoll or not.
5. Vehicles: whether they are in the prototype and how (enter/exit, driving model, traffic).
6. NPCs: civilians (wander, react/flee), gangs (territory, hostility), police (wanted level 1-5,
   pursuit, escalation, losing the wanted level); AI architecture (state machines / utility / BT).
7. UI/UX: HUD (health, ammo, wanted stars, minimap?), menus (pause, death screen), feedback.
8. Audio, VFX and "juice" minimum for the prototype.
9. Content strategy without artists: procedural/primitive geometry vs CC0 asset packs (Kenney,
   Quaternius, etc.) — licenses, formats (glTF), animation source for humanoids.
10. Tech stack decisions for Bevy 0.19: physics crate (avian3d vs bevy_rapier3d), character
    controller, input handling, UI approach, navigation/pathfinding, debug tooling — for each crate
    the version whose Cargo.toml declares Bevy 0.19 support (verified), or the fallback if none.
11. Performance targets (FPS, NPC counts, draw distance) and the frame-budget approach.
12. Workspace/crate layout and plugin-per-domain module map.
13. Decomposition into 10-16 vertical-slice implementation tasks in dependency order: title, goal,
    acceptance criteria (headless-testable part + owner-run part), dependencies, suggested MAW mode.

## Acceptance criteria
- [ ] PLAN_FINAL.md covers all 13 sections above, in Russian, with a clear recommended choice in every section (alternatives compared where a real choice exists)
- [ ] Every ecosystem crate recommendation cites the version and the evidence of Bevy 0.19 compatibility (crates.io / Cargo.toml / release notes URL)
- [ ] Genre references (GTA III/VC/SA/IV/V or others) are cited with sources where used
- [ ] The task decomposition is ordered, each task has acceptance criteria and dependencies, and the first task yields a runnable window with a controllable character
- [ ] Scope is honest for an AI-only team: every system names its smallest playable version
