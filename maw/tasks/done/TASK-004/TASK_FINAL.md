# TASK-004: GDD T3 — Облик города

Type: feature
Mode: full
Priority: high
Branch: feature/t03-city-look
Domains: bevy-ecs, gates, game-design

## Description
Implement slice **T3** of the APPROVED design document `docs/design/GDD.md` (§13), together with every
GDD section the slice touches (read them: world §2, player §3, combat §4, vehicles §5, NPC §6, UI §7,
audio/juice §8, content §9, stack §10, performance §11, workspace/plugin/data map §12). The GDD is the
scope law: its data files (§12), crate/plugin boundaries and verified crate versions (§10.1) are binding.

Goal (GDD §13): тротуары с бордюром, разметка, фасады (шейдер окон), уступы высоток, Kenney-пропы, ориентиры (башня, площадь, центральный парк), солнце с тенями, небо, `DistanceFog`, слияние мешей по чанкам (размер в `render.ron`), `VisibilityRange` для пропов, `tools/fetch_assets` + манифест.

## Dependencies
- blocked by TASK-003 — GDD slice T2 must land first

## Acceptance criteria
- [ ] Headless: манифест валиден (SHA-256, лицензия, ожидаемый состав на каждый пакет)
- [ ] Headless: число сущностей-чанков соответствует размеру города и размеру чанка из `render.ron` (ловит забытое слияние).
- [ ] Runtime QA (GDD D1): the scenario `tools/qa/scenarios/t3.py` exists and passes via `tools/qa/brp.py` — телепорт на крышу самой высокой башни и в парк, скриншоты; `get_diagnostics` на обзоре с крыши записывает FPS (доказательство для владельца, не автоматический гейт).
- [ ] Owner-run criterion recorded as an owner checklist in QA_REPORT.md: город выглядит как город, районы различимы.
- [ ] Every new tuning value lives in its GDD §12 data file, not in a `const`
- [ ] `cargo build`, `cargo clippy -- -D warnings`, `cargo test -p gta_sim` (and `-p citygen` where touched) are green
- [ ] Existing tests pass

### Resolved questions

Premise challenge (2026-09-23) returned PREMISE SUSPECT: the chunk-entity count alone does not prove building meshes were merged, because today every building has its own mesh-bearing entity (`src/visuals/city.rs:73-75`, `crates/gta_sim/src/world/city.rs:116,127`) and chunk entities could be added alongside them. Orchestrator decision (owner delegation): the premise of the slice stands; the acceptance gate is sharpened:
- [ ] Mesh-merge gate: the number of mesh-bearing (`Mesh3d`) entities that draw buildings/sidewalks equals the number of render chunks derived from the city size and `render.ron` chunk size; there is NO per-building `Mesh3d` left. Physics colliders stay one static collider per building (gameplay, T2 gate) — only the render side is merged. Flip-RED: re-enable per-building meshes and watch the gate fail.

Answers to the planner's open questions (orchestrator, owner delegation, 2026-09-23):
- Q1 prop collision: **visual-only props until T14** (as recommended).
- Q2 ready-made Kenney buildings: **no** (not in T3's goal).
- Q3 sky: **gradient dome + ClearColor = fog colour + linear DistanceFog**, not `Atmosphere` (as recommended).

OWNER RULE OVERRIDE — binary assets are NEVER committed to git (owner, 2026-09-23: "ассеты не хранятся, но в релизах в zip мы их приложим"). The plan's `!/assets/third_party/**` .gitignore exception and the `binary` .gitattributes line are REJECTED. Instead:
- `assets/third_party/` stays git-ignored (add `/assets/third_party/` to .gitignore explicitly); only `assets/third_party/manifest.ron` (URLs, pack versions, archive + per-file SHA-256, license) is tracked — add a `!/assets/third_party/manifest.ron` exception if needed.
- `tools/fetch_assets.py` downloads the packs per manifest, verifies SHA-256, extracts the selected files into `assets/third_party/<pack>/` together with each pack's License.txt; idempotent, cached zips.
- The strict manifest test validates the manifest itself always, and file hashes only when the files are present (skip with a clear message when absent — a fresh clone has not fetched yet). The game must fail with a clear message naming `tools/fetch_assets.py` when a required asset is missing.
- The files already downloaded by the planner in `scratch/third_party/` / `scratch/kenney/` may be used by the implementer to populate `assets/third_party/` locally (network may be unavailable), but never committed.
