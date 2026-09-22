# TASK-001 — рецензия и исправленный черновик GDD

Статус: **DRAFT — не APPROVED.** Документ информирует решение; утверждение и перенос в `docs/design/` выполняет оркестратор по делегированию владельца.

## 1. Review notes

### Проверенный контрпример

Самый опасный контрпример: владелец мог уже включить транспорт и трафик в обязательное ядро, тогда как отчёт продолжает считать их вырезаемыми. Проверка буквального текста подтвердила конфликт: `TASK_FINAL.md:52` выбирает **Q1=C** и прямо запрещает вырезать T14/T15, а `PLAN.md:251-259,628-633,660,670` рекомендует B, называет T15 опциональным и предлагает вырезать оба транспортных слайса. Контрпример **подтвердился**. Результат пробы сохранён в `scratch/plan_reviewer_1_disconfirmation_results.txt`.

### Существенные проблемы исходного отчёта

1. **Не сведены решения владельца.** Помимо Q1, отчёт оставляет Q1-Q7 открытыми и пишет `Resolved questions: (пусто)`, хотя `TASK_FINAL.md:50-65` уже фиксирует ответы Q1-Q7 и обязательную QA-инфраструктуру D1. Особенно пропущены целевая машина i9-11900K/RTX 4070 Ti/32 ГБ/1080p, строка «ПОТРАЧЕНО» и обязательный BRP-контур с T1.
2. **Определение готовности противоречит обязательному скоупу.** В `PLAN.md:111` езда условна, а трафик и полицейские машины вообще не входят в чек-лист. Исправлено: управляемая машина, гражданский трафик и автомобильное преследование входят в `prototype done`.
3. **T14/T15 неверно помечены опциональными.** Исправлено без изменения порядка: они остаются поздними из-за риска, но не являются кандидатами на вырезание. При нехватке времени режется глубина механик, а не обязательные capability.
4. **D1 отсутствует полностью.** В исходном техническом стеке нет `RemotePlugin`, `bevy_brp_extras`, `bevy_brp_mcp`, фич `dev`/`profile`, `tools/qa/brp.py` и сценариев с машинными скриншотами/диагностикой. Они добавлены в стек, T1 и приёмку последующих слайсов.
5. **Глобальный hit-stop нарушает headless-first.** `PLAN.md:394` предлагает менять `Time<Virtual>`, тем самым замораживая `FixedUpdate` и саму симуляцию ради визуального эффекта. Это делает презентацию владельцем геймплейного времени. Исправление: короткая локальная заморозка анимации/визуала атакующего и цели, камера и VFX на реальном времени; gameplay state продолжает тикать, ввод буферизуется. Замедление после смерти допустимо только после перехода симуляции в терминальное состояние и управляется `flow`, не `juice`.
6. **Нарушен принцип одного источника конфигурации.** `PLAN.md:166-168` одновременно назначает камере `assets/camera/camera.ron` и переносит общие значения в `assets/combat/aim.ron`. Исправлено: положение/геометрия камеры принадлежат `camera.ron`; `aim.ron` содержит лишь боевые параметры луча/assistance и читает готовую `AimOrigin` из состояния камеры.
7. **Часть tuning ошибочно объявлена законом.** Размер render-чанка 128 м в `PLAN.md:557` — изменяемый параметр производительности, следовательно это данные в `assets/world/render.ron`, не `const`. Законы: единицы, оси, фиксированный тик и collision layers.
8. **Заявление «город точно помещается без стриминга» сильнее доказательств.** Бенчмарк Bevy `bevy_city` показывает потенциал рендера, но не доказывает бюджет конкретной сцены с физикой, AI и skinned NPC. Исправлено: без geometry streaming — исходная гипотеза; чанковая структура сохраняется, а решение подтверждается T2/T16 на целевой машине.
9. **Источник Kenney переоценён.** Официальная страница Mini Characters подтверждает CC0, наличие анимации и 25 файлов, но не 12 персонажей/14 костей/32 клипа. Эти точные числа допустимы только как результат локального разбора скачанного архива (`scratch/research_procgen_assets.md:96-113`) и должны сопровождаться хэшем проверенного zip в asset-манифесте.
10. **Слабые источники не маркированы.** GTA wiki/моддинг-wiki полезны как жанровые референсы, но это вторичные community sources. Они не должны выдавать точные пороги/радиусы за «правильные» значения — числа остаются стартовым tuning в RON и принимаются прогоном владельца.
11. **Аудио-путь Freesound не автономен.** Даже превью требуют API token, оригиналы — OAuth2. Основной автономный путь должен быть Kenney CC0 + процедурные placeholder-звуки; Freesound — только ручной импорт с зафиксированной лицензией.
12. **Тестовая декомпозиция конфликтует с последовательностью.** Stretch «банда против полиции» записан в T9, хотя полиция появляется в T11. Исправлено: матрица фракций закладывается в T9; включение stretch происходит при интеграции T11, без обратной зависимости.
13. **Состояние репозитория описано почти верно.** Кода, `Cargo.toml` и `Cargo.lock` нет; `docs/design/` существует и пуст. Поэтому 0.19.1 пока не pinned workspace-ом — это обязательный pin будущего T1, заданный спецификацией задачи.

### Проверка источников и альтернатив

- Официальный релиз Bevy 0.19 подтверждает дату 2026-06-19, `bevy_city` с 55 000 сущностей и 19.3→11.8 мс, а docs.rs 0.19.1 подтверждает fixed timestep 64 Гц: https://bevy.org/news/bevy-0-19/ и https://docs.rs/bevy/0.19.1/bevy/time/struct.Fixed.html.
- Опубликованные `Cargo.toml` в `scratch/crates/` подтверждают: avian3d 0.7.0 → Bevy 0.19.0; bevy-tnua 0.32.0 → `^0.19`; bevy-tnua-avian3d 0.12.1 → avian `^0.7`, Bevy `^0.19`; bevy_enhanced_input 0.26.0 → Bevy 0.19.0; bevy_rapier3d 0.36.0 → Bevy 0.19.0.
- Сравнения процедурной сети (grid / Parish–Müller / tensor field), физики (Avian / Rapier), character controller (Tnua / Ahoy / свой), AI (FSM / BT / utility / GOAP) и навигации (граф / rerecast-landmass / vleue_navigator) достаточно широки. Рекомендации сохранены, но заявления о производительности считаются гипотезами до измерения.

## 2. Updated understanding

Проект находится в pre-design: нет игрового кода и утверждённого GDD. Стек обязателен: Rust + Bevy 0.19.1. Вся gameplay-логика должна работать в headless `App`; рендер, камера, UI, audio и juice только потребляют состояние. Тюнинг хранится в строгих RON-файлах, законы — в коде. Владелец делегировал оркестратору утверждение дизайна и уже решил:

- город 1.2×1.2 км (`city.ron`), seed-детерминированный;
- Kenney Mini Characters;
- смерть сохраняет всё, арест изымает оружие;
- «ПОТРАЧЕНО» — строка данных;
- банды атакуют игрока; межфракционные стычки — stretch;
- управляемые машины, трафик и полицейские машины обязательны;
- цель: ≥60 FPS, 1080p, i9-11900K + RTX 4070 Ti + 32 ГБ;
- T1 обязан дать QA-агенту BRP-доступ, screenshot/input/diagnostics/shutdown и profile features.

Цена ошибки: неверный scope или скрытая связь presentation→simulation размножится по всем последующим задачам; это высокий и труднообратимый риск. Feel и визуал заметны сразу и принимаются прогоном владельца, без искусственных числовых гейтов.

## 3. Revised approach

1. Зафиксировать capability ядра, но минимизировать глубину: машина едет и сталкивается; трафик держит полосу; полиция преследует на машине. Разрушение машин, сложная модель шин, дорожные аварии, тюнинг нескольких классов транспорта — вне прототипа.
2. Строить систему вертикально и headless-first: domain state + тестируемые правила, затем downstream presentation и BRP-сценарий.
3. Начать город с perturbed grid. Parish–Müller и tensor field оставить альтернативами v2; их выбор не должен менять downstream `CityLayout`.
4. Начать навигацию с производного графа тротуаров/полос + A*. Навмеш вводить только при воспроизводимом провале погони вне графа.
5. Выбрать Avian 0.7 + Tnua 0.32 по совместимой экосистеме, не по недоказанному превосходству скорости; Rapier остаётся named fallback.
6. Хранить gameplay time независимо от feedback. Shake/flash/hit-stop работают на presentation clock; любое pause/slow-motion gameplay — явное состояние домена `flow`.
7. Каждому слайсу с визуальной приёмкой дать BRP QA-сценарий; оценку feel всё равно оставлять владельцу.
8. Все точные значения ниже — стартовые предложения в RON с единицами, не требования к «правильному GTA».

## 4. Revised steps — полный исправленный GDD

### 1. Столпы и критерий «прототип готов»

Столпы: (1) процедурный читаемый город реагирует на хаос; (2) бой и физика дают ясную обратную связь; (3) полиция, банды, мирные и трафик образуют системную песочницу; (4) автомобиль — обязательный способ перемещения; (5) low-poly CC0 стиль важнее реализма.

Прототип готов, когда владелец может с seed запустить город, пройти и проехать его; стрелять и драться; спровоцировать реакцию мирных/банды/полиции; получить и сбросить 1–5 звёзд; пережить пешую и автомобильную погоню; угнать машину из трафика; умереть/быть арестованным; пользоваться HUD/мини-картой/паузой; и удержать ≥60 FPS в worst-case сцене на целевой машине. У игры нет миссий, экономики, интерьеров, cover-system, вертолётов и полного ragdoll.

### 2. Мир

Рекомендация: perturbed grid → районы → кварталы → лоты → здания → тротуарный и полосный граф. Альтернативы: Parish–Müller лучше для органики, tensor field — для плавных сетей, WFC — только для локальных деталей. Интерфейс `generate(seed, params) -> CityLayout` не зависит от Bevy.

Город: 1.2×1.2 км, 1 unit = 1 м, Y вверх. Downtown/commercial/residential/industrial, парки, центральная площадь, больница, участок, два gang HQ. Лоты делятся OBB-правилами, здания — extruded massing + часть Kenney props/buildings. Источники подходов: Parish & Müller 2001 https://dl.acm.org/doi/10.1145/383259.383292, Chen et al. 2008 https://www.sci.utah.edu/~chengu/street_sig08/street_project.htm, lot subdivision https://onlinelibrary.wiley.com/doi/10.1111/j.1467-8659.2012.03047.x.

Geometry streaming сначала нет, но layout и render meshes чанкуются. Решение подтверждается замером, не предположением. NPC/traffic стримятся пузырём. Детерминизм: `ChaCha8Rng`, stage sub-seeds, golden layout hashes и topology properties. Все размеры, jitter, доли районов/парков, LOD и chunk size — `assets/world/city.ron`/`render.ron`.

### 3. Игрок и камера

MVP: walk 1.8 м/с, run 4.5, sprint 6.8, jump 1.0 м; без crouch/cover/stamina. Tnua управляется intent-компонентами. Камера orbit over-shoulder: 3.8 м normal, 2.0 м aim, FOV 70°/55°, collision sphere 0.25 м; значения в `assets/camera/camera.ron`. `aim.ron` не дублирует геометрию камеры.

Клавиатура/мышь обязательны, gamepad поддерживается теми же action bindings. `Update` собирает edge-input, `FixedUpdate` потребляет intents. Здоровье/броня, регенерация только до 50%, смерть сохраняет loadout, арест изымает оружие. Значения: `assets/character/locomotion.ron`, `health.ron`, `assets/flow/respawn.ron`.

Проверенные направления: yaw 0°: W→−Z, D→+X; yaw +90°: W→−X, D→−Z; yaw 180°: W→+Z, D→−X. Эти три примера закрепляют знак поворота; feel камеры принимает владелец.

### 4. Бой

Огнестрел — hitscan двумя лучами: camera ray выбирает point of aim, muzzle ray проверяет реальную преграду. Projectile нужен лишь будущим гранатам. Пистолет, SMG, дробовик; ammo/reload/spread/falloff — `assets/combat/weapons.ron`. Fists + bat используют shape cast только в data-driven active windows (`melee.ron`). Headshot в MVP определяется простым отдельным head collider, а не процентом высоты капсулы: это яснее и не ломается при knockdown.

Полного ragdoll нет: death animation + лежащее тело с лимитом. Knockback/knockdown принадлежат gameplay. Hit marker, muzzle flash, tracer, camera recoil и local visual freeze принадлежат presentation.

### 5. Транспорт

Решение владельца: **вариант C обязателен**. Игрок входит/выходит, управляет одним классом sedan; трафик и полиция используют тот же lane graph. Player vehicle: Avian dynamic body + raycast suspension, arcade tire forces, data-driven limits. Fallback при нестабильности — более простой force-based chassis, но не удаление машин.

Traffic MVP: lane following, safe headway, stop/yield на конфликтной точке перекрёстка, spawn/despawn вне кадра. Полицейская машина едет к игроку/last-known position и выпускает пеших копов; сложные PIT, блокпосты и traffic-law AI вне MVP. Все массы, скорости, suspension, damage curves, spawn radii и IDM-like параметры — `assets/vehicle/*.ron` и `assets/traffic/traffic.ron`.

### 6. NPC, банды и полиция

Мирные: FSM `Wander/Idle/Flee/Cower/Report/Dead`, perception slices, простой utility только для выбора реакции. Банды: две территории, group aggro, ranged/melee/retreat. Основной MVP — против игрока; faction matrix позволяет stretch gang↔gang/police после появления полиции.

Wanted: data-driven heat, 1–5 звёзд, свидетель или прямое наблюдение копом, `LastKnownPosition`, search radius/timer. Точные community-derived пороги — лишь стартовые значения `assets/wanted/wanted.ron`. Полиция: `Respond/Arrest/Attack/Search`; 1 звезда пытается арестовать, высокие уровни наращивают число и вооружение. Вертолёты/SWAT-модели заменяются тем же humanoid с tint/equipment.

AI: enum FSM + маленькие utility selectors. BT/GOAP избыточны для 4–7 состояний. Навигация: sidewalk/lane graph + `pathfinding` A* + separation; rerecast/landmass — fallback после воспроизводимого failure case. GTA path/wanted источники (например https://gtamods.com/wiki/Paths_(GTA_SA) и https://gtamods.com/wiki/Wanted_level) помечаются как community references.

### 7. UI/UX

Native `bevy_ui`: health, armor, ammo, crosshair, hit marker, wanted stars; procedural minimap from `CityLayout`, gang territory, police cones/search area, vehicle state. Меню: seed, pause, settings, exit. Death text строго «ПОТРАЧЕНО», arrest — `BUSTED`; строки в данных. Settings через Bevy 0.19.1 `SettingsPlugin` с уникальным app id и explicit save; документация: https://docs.rs/bevy/0.19.1/bevy/settings/struct.SettingsPlugin.html.

### 8. Audio, VFX и game feel

На важное событие 2–5 слоёв сначала, затем добавление по owner-run: SFX, flash/particles, recoil/shake, UI response. Trauma squared, smooth noise, decay; shake affects only camera visual pivot. Local hit-stop 40–60 мс freezes presentation of attacker/target once per impact and restores by `Time<Real>`; gameplay simulation and input buffering continue. Death slow-motion starts only after gameplay has entered `Wasted`.

Accessibility: reduce shake, reduce camera motion, disable flashes. Audio: built-in `bevy_audio`; Kenney CC0 and procedural placeholders are autonomous baseline. Freesound is manual-only. Все параметры — `assets/juice/juice.ron` и `assets/audio/mix.ron`.

### 9. Контент без художников

Рекомендация: Kenney CC0 для Mini Characters, City/Car/Blaster kits; procedural roads/building massing; optional Poly Haven/ambientCG CC0 textures. Quaternius UAL — запасной humanoid only after rig-path test; Mixamo исключён из автономного pipeline из-за login/raw redistribution/FBX.

Точные 12 персонажей, 14 костей и 32 клипа — локально проверенные свойства конкретного Mini Characters zip, не claim страницы. Asset manifest хранит URL, SHA-256, license, expected inventory; GLB и license commit либо reproducibly fetch. Official page: https://kenney.nl/assets/mini-characters; license: https://kenney.nl/support.

### 10. Технический стек Bevy 0.19.1

| Роль | Выбор | Совместимость |
|---|---|---|
| Engine | `bevy = "=0.19.1"` | pin задачи; official docs/release |
| Physics | `avian3d = "=0.7.0"` | published manifest: Bevy 0.19.0 |
| Physics fallback | `bevy_rapier3d = "=0.36.0"` | manifest: Bevy 0.19.0 |
| Character | `bevy-tnua = "=0.32.0"`, `bevy-tnua-avian3d = "=0.12.1"` | Bevy `^0.19`, Avian `^0.7` |
| Character fallback | `bevy_ahoy = "=0.2.0"` | Bevy 0.19, Avian 0.7 |
| Input | `bevy_enhanced_input = "=0.26.0"` | Bevy 0.19.0; Leafwing 0.21 — альтернатива |
| Paths | `pathfinding = "=4.16.0"` | engine-independent |
| Nav fallback | rerecast 0.5 + avian_rerecast 0.6 + landmass 0.12 + landmass_rerecast 0.3 | manifests declare Bevy 0.19 / Avian 0.7 |
| Data | `rand_chacha 0.10`, `ron 0.12.2`, serde | engine-independent |
| Debug | `bevy-inspector-egui 0.37.0` + compatible `bevy_egui 0.40.x` | manifests declare Bevy 0.19 |
| Agent QA | `bevy_brp_extras = "=0.22.6"`, `bevy_brp_mcp = "=0.22.6"` | task-verified: Bevy `^0.19.1`; recheck published manifests in T1 |

`dev` enables Bevy `bevy_remote` + `png`, `RemotePlugin` HTTP and `BrpExtrasPlugin` on port 15702. Exposed operations: world query/get/mutate, screenshot, input, diagnostics and shutdown. `profile` enables `trace_chrome`; `trace_tracy` remains separate for owner use. `debug` owns inspector/physics overlays. T1 must resolve any `bevy_egui` conflict with BRP extras by actual dependency tree, not guesswork.

Workspace: root client binary; `crates/citygen` pure layout; `crates/gta_sim` headless gameplay. `compose_sim` is shared. Domain plugins: flow, world, character/player, combat, perception/navigation/population, civilian, gang, wanted/police, vehicle/traffic. Presentation domains: input, camera, visuals, HUD/menu/minimap, audio/VFX/juice, debug/remote.

Laws explicitly configured in code: metre scale, axes, collision layers, 64 Hz sim tick. All other numbers are RON data. `cargo test -p gta_sim` is the render-free gate; workspace feature unification is separately checked.

### 11. Performance

Target: ≥60 FPS at 1080p on i9-11900K/RTX 4070 Ti/32 ГБ in downtown, 5-star pursuit, 40 civilians + 12 gang + 12 police + traffic. Starting budgets: gameplay fixed tick ≤4 ms, AI ≤1.5 ms, city generation ≤2 s release; these are budgets to measure, not CI wall-clock assertions.

Approach: chunked merged static meshes, GPU batching for repeated props, `VisibilityRange`, fog, population bubbles, sliced perception/path requests, async city/minimap generation. No automatic FPS pass/fail on unknown runners. BRP diagnostics records FPS/frame time on the owner target; profiling uses trace output. Bevy renderer evidence is a reference, not a guarantee: https://bevy.org/news/bevy-0-19/.

### 12. Data and domain map

Core files: `assets/world/{city,render}.ron`, `character/{locomotion,health}.ron`, `camera/camera.ron`, `combat/{weapons,melee,aim}.ron`, `npc/{population,perception,navigation,civilian}.ron`, `gang/gangs.ron`, `wanted/wanted.ron`, `police/escalation.ron`, `vehicle/{sedan,damage}.ron`, `traffic/traffic.ron`, `audio/mix.ron`, `juice/juice.ron`, `flow/respawn.ron`. Strict loader rejects unknown fields and reports path/field. One owner per value; consumers receive derived state instead of reopening another domain’s config.

### 13. Вертикальные слайсы (полный порядок)

1. **T1 — playable foundation + agent QA (`full`).** Window, test level, controllable capsule, orbit camera, strict RON, headless composition; `dev/profile/debug`; `tools/qa/brp.py` launches build, waits for BRP, sends input, captures PNG, reads FPS/components, shuts down. Acceptance: direction/state headless tests; owner can move/jump; QA script proves screenshot + diagnostics.
2. **T2 — procedural city layout (`full`, depends T1).** Grid/district/block/lot/landmarks, async load, seed. Acceptance: topology/golden layouts; owner walks two visibly different seeds; QA scenario captures both.
3. **T3 — city presentation (`full`, T2).** Roads, facades, props, fog, chunk meshes. Acceptance: asset manifest/license/hash; owner judges readability; BRP FPS/screenshot evidence.
4. **T4 — humanoid/animation (`full`, T1).** Verified Mini Characters archive, animation state presentation. Acceptance: pure `AnimState` mapping; owner checks sliding/readability; QA screenshot sequence.
5. **T5 — health/death/respawn (`full`, T2,T4).** Armor, regen, Wasted/«ПОТРАЧЕНО», hospital. Acceptance: headless damage/state transitions; BRP scenario kills and observes respawn.
6. **T6 — shooting (`full`, T5).** Aim, two-ray hitscan, pistol/SMG/shotgun, ammo/HUD/feedback. Acceptance: obstruction, damage, reload tests; owner feel run; QA shoots target and inspects state.
7. **T7 — melee (`full`, T5).** Fists/bat, active windows, stagger/knockdown, presentation-only hit-stop. Acceptance: hit-window/headless direction tests; owner feel run.
8. **T8 — civilians/population (`full`, T2,T4,T5).** Sidewalk graph, bubble, wander/flee/report. Acceptance: reaction/despawn rules and stress measurement; QA verifies crowd reaction.
9. **T9 — gangs (`full`, T6,T7,T8).** Territories, group aggro, combat, faction matrix; gang↔gang/police remains disabled stretch flag. Acceptance: territory/aggro/decay tests; owner provokes gang.
10. **T10 — wanted core (`full`, T6,T8).** Witness, heat, stars, search state/HUD. Acceptance: witness/threshold/loss tests; QA earns and loses wanted level.
11. **T11 — police on foot (`full`, T9,T10).** Dispatcher, arrest, escalation, search; optionally enable faction stretch from T9. Acceptance: arrest/caps/last-known tests; owner completes on-foot chase loop.
12. **T12 — minimap/menus/settings (`full`, T2,T10).** Raster map, markers/cones, seed/pause/settings, Busted. Acceptance: deterministic projection; QA changes seed and captures UI.
13. **T13 — audio/VFX/juice (`full`, T6,T7,T11).** Autonomous CC0/procedural audio, spatial sirens, accessibility, layered feedback. Acceptance: manifest validation; owner accepts feel; no gameplay-time ownership.
14. **T14 — drivable vehicle (`full`, T2,T5,T8,T12).** Dynamic sedan, enter/exit, camera, collision damage. Acceptance: ownership/damage/tunnelling cases; owner accepts handling; QA drives route.
15. **T15 — traffic + police cars (`full`, T11,T14; mandatory).** Lane traffic, intersections, spawn/despawn, stealing occupied car, police response/arrival. Acceptance: headway/nonnegative speed/spawn rules; owner completes vehicle pursuit; QA scenario captures it.
16. **T16 — performance/final acceptance (`full`, T1-T15).** Worst-case scene, traces, measured optimizations, complete done checklist. Acceptance: headless suites green; BRP diagnostics on target; owner passes every capability including traffic and vehicle pursuit.

Каждый owner-run слайс имеет `tools/qa/scenarios/<slice>.py`; screenshot помогает QA, но не заменяет решение владельца о feel.

### Resolved questions

Q1=C; Q2=A, B stretch; Q3=A; Q4=1.2×1.2 км; Q5=Kenney Mini Characters; Q6=i9-11900K/RTX 4070 Ti/32 ГБ/1080p/≥60 FPS; Q7=«ПОТРАЧЕНО». Открытых вопросов нет.

## 5. Risk areas

- **Scope/sequence:** T14/T15 обязательны и поздние. Мера: ограничить глубину транспорта, не вырезать capability; T16 не начинается без vehicle-pursuit loop.
- **BRP ecosystem:** `bevy_brp_extras`/MCP и egui могут конфликтовать по transitive versions. Мера: T1 проверяет published manifests, `cargo tree -d`, реальный screenshot/input/diagnostics round-trip.
- **Headless physics:** Avian+Tnua plugin set под `MinimalPlugins` ещё не исполнен в этом пустом repo. Мера: маленькая executable probe в T1 до расширения архитектуры.
- **Vehicle stability/tunnelling:** высокий silent risk. Мера: max-speed wall/corner tests, CCD or substeps only from evidence, target-machine drive run.
- **Traffic complexity:** intersections and police pursuit may dominate. Мера: single-lane conflict reservations and simple pursuit; no generalized traffic simulation.
- **Performance:** 64 Tnua/skinned NPC + physics + traffic не измерены. Мера: staged stress scenes T8/T15 and target-hardware BRP metrics; distant NPC may become kinematic presentation without changing gameplay decisions.
- **No-streaming hypothesis:** full 1.44 км² may exceed budgets. Мера: keep chunk boundaries; add activation/deactivation only after measured failure.
- **Navigation coverage:** graph may fail in courtyards/off-road pursuit. Мера: reproducible stuck scenarios; only then adopt rerecast/landmass.
- **Asset reproducibility:** web URLs and archive contents can change. Мера: SHA-256, license, expected inventory; exact Kenney counts tied to checked archive.
- **Game feel/accessibility:** shake/flash/hit-stop can nauseate or mask input. Мера: presentation-only, real-time recovery, input buffering, reduce/disable toggles, owner run.
- **Secondary genre sources:** community wikis may be stale. Мера: use them only as references; all gameplay values remain explicitly proposed data.
- **Bevy 0.20 pressure:** do not drift during the run. Pin exact 0.19.1 ecosystem versions and commit `Cargo.lock` in T1.

children: 0 launched / 0 reported.
