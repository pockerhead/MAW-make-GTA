# TASK-007: GDD T6 — Стрельба

Type: feature
Mode: full
Priority: high
Branch: feature/t06-shooting
Domains: bevy-ecs, gates, game-design

## Description
Implement slice **T6** of the APPROVED design document `docs/design/GDD.md` (§13), together with every
GDD section the slice touches (read them: world §2, player §3, combat §4, vehicles §5, NPC §6, UI §7,
audio/juice §8, content §9, stack §10, performance §11, workspace/plugin/data map §12). The GDD is the
scope law: its data files (§12), crate/plugin boundaries and verified crate versions (§10.1) are binding.

Goal (GDD §13): оружие из `weapons.ron` (пистолет, SMG, дробовик), режим прицела (камера и strafe), `AimIntent` из клиента, hitscan двумя лучами, сенсор головы в слое `Hitbox` (сначала проверить, что сенсор земли Tnua его не видит), разброс, магазин, перезарядка, смена оружия, пикапы оружия и патронов, манекены-мишени, HUD патронов, прицел, хит-маркер, вспышка, трассер, отдача камеры.

Known issue from TASK-006 QA (B1): a damage message written in the last frame of `Wasted` hits the already-respawned player (70/100 instead of 100). With real weapon damage in this slice, damage must be dropped for a target that is `Dead` or when the game is not in `Playing`, and gated: damage queued during Wasted does not reach the respawned player.

## Dependencies
- blocked by TASK-006 — GDD slice T5 must land first

## Acceptance criteria
- [ ] Headless: луч по манекену на 10 м → урон по таблице
- [ ] Headless: стена между дулом и целью блокирует
- [ ] Headless: попадание в сенсор головы → ×2
- [ ] Headless: перезарядка по времени
- [ ] Headless: дробовик даёт 10 лучей
- [ ] Headless: разброс растёт при серии и сжимается со временем.
- [ ] Runtime QA (GDD D1): the scenario `tools/qa/scenarios/t6.py` exists and passes via `tools/qa/brp.py` — телепорт к манекену, наведение `move_mouse`, `send_mouse_button` ЛКМ, чтение `Health` манекена и патронов игрока, скриншот с трассером и хит-маркером.
- [ ] Owner-run criterion recorded as an owner checklist in QA_REPORT.md: стрельба ощущается (отдача, звук-заглушка, трассер), попадания читаются.
- [ ] Every new tuning value lives in its GDD §12 data file, not in a `const`
- [ ] `cargo build`, `cargo clippy -- -D warnings`, `cargo test -p gta_sim` (and `-p citygen` where touched) are green
- [ ] Existing tests pass

### Resolved questions

Planner's open questions, answered by the orchestrator (owner delegation, 2026-09-23):
- Q1 shooting range location: **A** — centre of the central park (`CityLandmarks.park_center`).
- Q2 weapon models: **A** — primitives. Kenney Blaster Kit is sci-fi toy blasters and does not fit a GTA-like; no weapon assets in T6.
- Q3 fire mode: **B** — pistol and shotgun are semi-automatic (one shot per press), SMG fires while held; one field in `weapons.ron`.
- Q4 speed while aiming: **B** — aiming caps movement at run speed (no sprint while RMB is held), GTA V-like; one field in `locomotion.ron`.

### Owner addition (2026-09-23, during plan review) — floating damage numbers ("juice"), IN SCOPE for T6

Owner, verbatim: "при попаданиях в точке куда попало должны появляться циферки урона и быть слегка разными каждый раз как бы рандомизируя урон оружия, а при попадании в голову они же но больше и красные и надпись CRIT. ну и они должны по juice-овой анимации лететь вверх и растворяться, и надо их либо 3d banner label либо просто banner label ну итпа чтобы на камеру были направлены".
- [ ] Damage variance: every hit's damage is the weapon's base damage times a random factor in a range from `weapons.ron` (e.g. ±10 %, per weapon or global); the applied damage and the number shown are the SAME value. Gameplay-side, headless-gated: over N hits the values stay inside the configured range and are not all equal; the shotgun's per-pellet hits each roll. RNG is seeded/owned by gta_sim (no thread_rng in gameplay).
- [ ] Hit event carries (position of the hit point, damage, is_headshot) from gta_sim to the client (a Message/Event the client reads) — the client never recomputes damage.
- [ ] Floating number at the hit point, always facing the camera: either a world-space billboard or a UI label anchored to the projected world point — the plan picks one, with its reason. Integer value.
- [ ] Headshot: the same number but larger, red, with the word "CRIT" (text from `strings.ron`).
- [ ] Juice animation: quick pop-in scale, then rise upward with ease-out and fade out, slight random sideways drift so stacked hits do not overlap exactly; lifetime and all numbers in `juice.ron`; entities despawn after the lifetime (gate: no leak after many hits).
- [ ] Runtime QA: the t6 scenario screenshot shows a damage number after a body hit and a red "CRIT" number after a headshot.

### Orchestrator finding from the implementer's runtime screenshots (2026-09-23, owner-proxy) — IN SCOPE for the fixer

`scratch/qa_t6/aim_burst_1.png`: in aim mode (RMB) the player's head fills about a third of the screen (left-centre) and hides the target next to the crosshair. That is a first-frame visual defect the owner would reject.
- [ ] Aim camera: the character must not cover the crosshair or the area around it; move the aim framing (shoulder offset to the right / distance / height in `camera.ron`, and/or fade/hide the player model when the camera is closer than a threshold) so the body sits in the left part of the screen and the crosshair has a clear view. Verify with a BRP screenshot in aim mode: the target at the crosshair is fully visible.
- [ ] Tracer is invisible in every screenshot (60 ms, seen end-on): make it readable (width/lifetime in data), and show one in a t6 screenshot.

### Owner finding (2026-09-23, watching the fixer's runs) — no gun visible in the hands, IN SCOPE for the fixer

Owner: "там нету оружия в руках совсем". Code: `src/visuals/weapons.rs:90-108` spawns the held box as a child of the Player body at `aim.muzzle_offset() + Z*length/2`. Likely (verify, do not assume): that point lies inside the Kenney model's oversized head/torso, or behind the body (+Z is backward), so the box is hidden.
- [ ] The held gun is clearly visible in the character's hand (right side, in front of the torso, pointing forward) in normal third-person view and in aim mode; placement values (hand offset, rotation) live in `render.ron`, not derived from the gameplay muzzle point.
- [ ] Muzzle flash and tracer start at the visible barrel end (presentation only — gameplay rays keep using `AimConfig`).
- [ ] BRP screenshots (normal + aim) show the gun; look at the PNGs yourself.

### Owner finding (2026-09-23) — weapon holding pose, IN SCOPE for the fixer

Owner: "и позы удержания ствола тоже кажется" — the character holds a gun with the default idle/run arms.
The pinned Kenney pack already has the clips (see `assets/third_party/manifest.ron` rig): `holding-right`, `holding-right-shoot` (one hand — pistol), `holding-both`, `holding-both-shoot` (two hands — SMG, shotgun). The rig has separate `arm-left` / `arm-right` joints.
- [ ] While armed, the arms play the matching holding clip (pistol: right; SMG/shotgun: both) layered over locomotion — legs/torso keep walk/run/sprint/jump/fall. Use AnimationGraph mask groups on the arm joints (verify the Bevy 0.19.1 mask API in the pinned source); a fallback is acceptable only if masks do not work, and must be named.
- [ ] On a shot, the matching `*-shoot` clip plays once on the arms (presentation only).
- [ ] The visible held gun (previous finding) sits in the hand of that pose.
- [ ] Client gate: the arm layer selects the right clip per weapon / unarmed (pure function table); BRP screenshots of pistol and SMG holding poses, looked at.
