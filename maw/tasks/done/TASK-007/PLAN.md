# PLAN — TASK-007 (GDD T6): стрельба

Все API ниже сверены с пиненными исходниками в `~/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/` (bevy 0.19.1, bevy_ecs 0.19.1, bevy_audio 0.19.1, bevy_light 0.19.1, avian3d 0.7.0, bevy_enhanced_input 0.26.0, bevy_brp_extras 0.22.6, rodio 0.22.2, glam 0.32.1) и с vendored `vendor/bevy-tnua-avian3d-0.12.1`. Поведение движка, на котором стоит план, подтверждено исполняемой пробой `scratch/probe/tests/probe.rs` (вывод ниже, раздел 1.6).

Цена ошибки. Молчаливые дефекты: урон, пережив `Wasted`, бьёт возрождённого игрока (B1, уже воспроизведён пробой); хедшот, который никогда не срабатывает; стена, которая не блокирует; сенсор головы, который Tnua принимает за пол; разброс и перезарядка с неверной арифметикой тиков. Они получают headless-гейты. Отдача, трассер, вспышка, звук, прицел, хит-маркер, ощущение strafe видны владельцу с первого кадра: механика + скриншоты BRP + чек-лист владельца, без отдельной машинерии.

---

## 1. Understanding (что есть сейчас)

### 1.1 Симуляция (`crates/gta_sim`)
- `lib.rs:25-68` `compose_sim`: грузит `locomotion.ron`, `health.ron`, `respawn.ron`, `city.ron` через `load_config` + `validate()`, вставляет ресурсы и плагины `FlowPlugin, PhysicsPlugins, TnuaAvian3dPlugin(FixedUpdate), CharacterPlugin, WorldPlugin, PlayerPlugin, CombatPlugin, WantedPlugin`. Единственная точка композиции (игра и тесты).
- `character/mod.rs:27-30` `Character` требует `MoveIntent, JumpBuffer, AnimState`. `:95-117` `character_components`: `RigidBody::Dynamic`, капсула `capsule(r, h-2r)`, `LockedAxes::ROTATION_LOCKED.unlock_rotation_y()`, Tnua-контроллер, сенсор-цилиндр. Слоёв коллизий нет нигде в проекте (grep `CollisionLayers|PhysicsLayer` пуст): всё в слое 0 по умолчанию (`CollisionLayers::DEFAULT = {memberships: 1, filters: ALL}`, avian `collision/collider/layers.rs:373-376`).
- `character/mod.rs:119-171` `drive_characters`: `desired_motion = move_direction(axis, yaw) * speed`, `desired_forward = direction` (тело смотрит по движению). `:71-80` цепочка `HealthSystems::{Damage, Regen, Pickup, Death}`.
- `character/health.rs:83-121` `Health {current, armor, since_damage}`, `Dead`, чистые `apply_damage`, `regenerate`.
- `character/intent.rs` `MoveIntent {axis, yaw, gait, jump_held, jump_requested}` — образец "Update ставит флаг, FixedUpdate потребляет".
- `player/mod.rs:13-18,62-77` `DebugDamage` (Message, Reflect) читается в `apply_debug_damage` внутри `PlayingSystems` + `HealthSystems::Damage`, цель `With<Player>, Without<Dead>`. `:46-60` `spawn_player` на `OnTransition{Loading→Playing}`.
- `flow/mod.rs:37-59`: `PlayingSystems.run_if(in_state(Playing))` на `FixedUpdate`; `OnExit(Wasted)` → `respawn_player` (`flow/wasted.rs:97-123`: та же сущность, полное здоровье, снимает `Dead`).
- `combat/mod.rs:1-29`, `combat/pickups.rs:1-81`: только аптечка/броня (`Pickup {kind, cooldown}`) у `HospitalSpawn`; сбор по расстоянию ног ≤ `pickups.radius`.
- `world/city.rs:169-178` `CityLandmarks {tower_roof, plaza_center, park_center}` (Reflect, только в мире `City`); `world/test_area.rs` фикстурный уровень headless-гейтов (пол 80×80 вокруг 0, свободна зона x≈−20, z∈[15,35]).

### 1.2 Клиент (`src/`)
- `input/mod.rs:52-109`: контекст BEI `OnFoot` (Move, Look, Sprint, Walk, Jump), `write_move_intent` в `Update` после `apply_mouse_look`; `cursor_toggle` — Esc отпускает, ЛКМ захватывает курсор.
- `camera/mod.rs:53-69` `apply_mouse_look` (мышь вправо уменьшает yaw), `:71-122` `follow_player` в `PostUpdate`: pivot `feet + pivot_height`, сдвиг плеча и дистанция через `cast_shape` сферой с фильтром `from_excluded_entities([player])` — **маска ALL**. `camera.ron` без параметров прицела.
- `hud/mod.rs` полосы здоровья/брони (`UiConfig.hud` из `assets/ui/strings.ron`), `menu/mod.rs` `UiFonts`.
- `visuals/character.rs:68-88` модель Kenney вешается наблюдателем на **любой** `CharacterBody` (манекены получат гуманоида бесплатно). `visuals/pickups.rs` кубы пикапов по цвету из `render.ron`. `visuals/character_gate.rs:193-229` клиентский гейт ждёт ровно одну `CharacterModel` в `TestArea`.
- `main.rs:120-199`: грузит `camera.ron`, `render.ron`, `visual.ron`, `strings.ron`, `preflight`, добавляет плагины. Звука, VFX, juice нет.
- `tools/qa/brp.py`: `send_keys`, `move_mouse`, `screenshot`, нет `send_mouse_button`. `tools/qa/scenarios/t5.py` — образец сценария (release, `--seed 1`, телепорт через `world.mutate_components` Position).

### 1.3 B1 (TASK-006 QA) — корень найден пробой
`apply_debug_damage` стоит в `PlayingSystems`: во время `Wasted` он не читает, сообщения остаются в буфере (в slow-mo `FixedUpdate` редко идёт, а ротация буферов ждёт сигнала fixed: `bevy_ecs-0.19.1/src/message/update.rs:20-53`). В кадре выхода `StateTransition` сначала выполняет `OnExit(Wasted)` → `respawn_player` (100 HP), потом `FixedUpdate` уже в `Playing` читает старое сообщение. Проба: одно `DebugDamage(30)`, записанное **только перед последним** апдейтом `Wasted`, даёт 70/100. Проверка состояния в момент чтения не поможет: в момент чтения состояние уже `Playing`.

### 1.4 Tnua и сенсор головы (задача GDD "сначала проверить")
`vendor/bevy-tnua-avian3d-0.12.1/src/lib.rs`: датчик земли пропускает (а) коллайдеры, чей `ColliderOf.body` — сам персонаж (`:222-226`), (б) любой `Sensor` у сущности или её тела (`:248, :307`), (в) сущности, с которыми слои владельца не взаимодействуют (`:295-307`). Сенсор головы с `Sensor` и слоем `Hitbox` с `filters = NONE` отсекается трижды. Ниже (шаг D7) — фальсифицируемый гейт, а не только чтение кода.

### 1.5 avian 0.7.0 — факты для hitscan
- `SpatialQuery::cast_ray_predicate(origin, Dir3, max, solid, &filter, &dyn Fn(Entity)->bool) -> Option<RayHitData{entity, distance, normal}>` (`spatial_query/system_param.rs:176-184`), ближайшее попадание.
- `SpatialQueryFilter::test` смотрит только `memberships` коллайдера против `mask` фильтра (`spatial_query/query_filter.rs:95-99`); `filters` коллайдера для запросов роли не играют → сенсор с `filters = NONE` виден лучу, если маска включает `Hitbox`.
- `ColliderOf { body }` есть и у коллайдера на самом теле (указывает на себя), и у дочернего (проба).
- Сенсоры не дают массы (GDD §4.1, `collision/collider/mod.rs:402`).

### 1.6 Вывод пробы (`scratch/probe`, собрана с `CARGO_TARGET_DIR=D:/test-gta-like/target`, `--offline`)
```
PROBE B1: updates in Wasted=288, health right after Playing=70, 4 updates later=70
PROBE B1 single-last-frame: wasted updates=288, health=70
PROBE head: body rest y=1.0499984 (expected float_height 1.05)
PROBE ColliderOf: body->Some(175v0) head->Some(175v0) (body=175v0, head=176v0)
PROBE ray head-height, mask ALL: Some(("HEAD", 9.653597))
PROBE ray head-height 1.45, mask ALL: Some(("HEAD", 9.683775))
PROBE ray chest 1.0, mask ALL: Some(("BODY", 9.6998825))
PROBE ray head-height, mask World|Character only: Some(("BODY", 9.740102))
```
Сфера головы r = 0.35 с центром 1.6 м над ногами (капсула r = 0.3, верх 1.8) охватывает капсулу выше ≈1.42 м, поэтому ближайшим попаданием становится голова. Сфера внутри капсулы проиграла бы всегда: луч сначала входит в капсулу.

### 1.7 Исследование (веб)
- Два луча в TPS (камера → точка прицела, дуло → точка; луч камеры игнорирует владельца, иначе "стреляет в затылок"): https://rmcphersonnarrativedesign.wordpress.com/2014/07/22/using-hitscan-weapons-with-a-third-person-camera-in-unity-c/ , https://discussions.unity.com/t/raycast-in-third-person-shooter/251922 .
- Bloom: прибавка за выстрел, потолок, восстановление в единицах/с, зависимость от движения: https://planetside.fandom.com/wiki/Weapon_Mechanics , https://deeprockgalactic.fandom.com/wiki/Accuracy , https://nalerian.com/blog/understanding-bloom-and-shot-dispersion/ . Отсюда задержка восстановления: без неё у SMG (0.08 с) восстановление съедает рост и "разброс растёт при серии" не выполняется.
- Kenney Blaster Kit 2.1: CC0, 40 файлов, `https://kenney.nl/media/pages/assets/blaster-kit/261d80a716-1753959510/kenney_blaster-kit_2.1.zip` (страница пакета). В этом слайсе не берём (раздел 5, Q2).

---

## 2. Approach

1. **Слои коллизий (закон, `const`/enum):** новый `gta_sim::layers::GameLayer { #[default] World, Character, Hitbox }` (`#[derive(PhysicsLayer)]`). Статика без компонента остаётся `World`. Тело персонажа `CollisionLayers::new(Character, LayerMask::ALL)`. Голова `CollisionLayers::new(Hitbox, LayerMask::NONE)` + `Sensor`: физических пар нет, лучи с маской `Hitbox` её видят. Остальные слои GDD (`Vehicle, Prop, Trigger, Pickup`) добавят их слайсы.
2. **Хитбокс головы** дочерней сферой в `character_components` (у всех персонажей: игрок, манекены, будущие NPC). Геометрия — данные `locomotion.ron` (`head_height`, `head_radius`) с валидацией "голова охватывает верх капсулы" (иначе хедшот невозможен, раздел 1.6).
3. **Интенты:** `AimIntent {origin, direction, aiming}` и `ActionIntent {fire_held, fire_requested, reload_requested, select, cycle}` в `character/intent.rs`, требуются `Character`. Клиент пишет в `Update`/`PostUpdate`, `FixedUpdate` потребляет защёлки (правило `just_pressed`).
4. **Оружие** — домен `combat/`: `Weapon {Pistol, Smg, Shotgun}`, компонент `Loadout` (выбранное оружие, по слоту на ствол: `owned/magazine/reserve`, таймеры `cooldown/reload_left/since_shot`, `bloom_deg`, производное `spread_deg` для HUD). Две системы в `PlayingSystems` + `HealthSystems::Damage`: `tick_loadouts` (выбор, таймеры, перезарядка, восстановление разброса) → `fire_weapons` (выстрел).
5. **Hitscan двумя лучами** (GDD §4.1): луч 1 из `AimIntent.origin` по `direction` до `max_aim_distance` → точка P (или дальняя точка). Дуло = позиция тела + `R_y(aim_yaw)·muzzle_offset`. Для каждой дробины: направление = конус вокруг (P − дуло) с половинным углом `spread_deg`, луч 2 из дула на `range`. Оба луча с маской `World|Character|Hitbox` и предикатом "коллайдер не принадлежит стрелку" (`ColliderOf.body != shooter`). Разрешение попадания: `ColliderOf.body` → цель; `Has<HeadHitbox>` → ×`headshot_multiplier`; урон × спад дробовика; применение **сразу** к `Health` цели `Without<Dead>` внутри системы (нет очереди урона — нет B1 для оружия, известен факт убийства для красного хит-маркера).
6. **B1:** `Messages<DebugDamage>::clear()` на `OnExit(GameState::Wasted)` (`bevy_ecs-0.19.1/src/message/messages.rs:228`). Урон по `Dead` уже отбрасывается фильтром. Гейт — ровно сценарий пробы.
7. **Разброс:** `spread = base + bloom + moving_deg_per_mps · |v_xz|`; выстрел использует разброс до прибавки; `bloom = min(bloom + per_shot, max_bloom)`; восстановление `recovery_deg_per_s` только после `recovery_delay` без выстрелов. Случайность — `rand_chacha::ChaCha8Rng` (крейт GDD §2.5/§10.1, уже в `Cargo.lock`) в ресурсе `SpreadRng` с фиксированным seed: тесты воспроизводимы, а все ожидания гейтов верны для **любого** сэмпла внутри конуса (выведено геометрически, раздел 3 D).
8. **Манекены и пикапы оружия** спавнятся только в мире `City` на `CityLandmarks.park_center` (плоский газон без коллайдеров пропов, блоки ≥ 70 м). В `TestArea` полигона нет: там гейты ставят цели сами через production-бандл `dummy_bundle`, а клиентский гейт с одной `CharacterModel` не ломается. Манекен — обычный `Character` + `Health` + `Dummy {reset_left}`: при 0 HP получает `Dead`, через `range.dummy_reset` с воскресает.
9. **Презентация** (клиент, все числа в данных): режим прицела камеры (плечо 0.55, дистанция 2.0, FOV 55°, переход 0.15 с ease-out, чувствительность ×0.7 — GDD §3.2), `AimIntent` из камеры **без** отдачи; отдача pitch как визуальное смещение с возвратом (`juice.ron`, на луч не влияет — GDD §4.1); вспышка (unlit-квад + `PointLight`, 50 мс) и трассер (60 мс) по сообщениям; звук-заглушка — процедурный шумовой всплеск через `bevy_audio::Decodable` (без нового крейта, `wav` не нужен); HUD: патроны `12 / 48`, прицел (точка; при ПКМ крестик с раскрытием по `spread_deg`), хит-маркер 0.1 с (красный при убийстве). Простой бокс-ствол в руке и боксы пикапов.

Почему так. Прямое применение урона против общего `Damage`-сообщения: сообщение пришлось бы чистить на каждом выходе из `Wasted`, а красный хит-маркер требовал бы обратного канала "убил ли"; прямое применение проще и закрыто теми же условиями (`PlayingSystems`, `Without<Dead>`). Когда появятся NPC-стрелки (T9/T11), `fire_weapons` уже работает для любого `Character` с `Loadout`.

---

## 3. Steps

Порядок: A данные → B sim (слои, голова, интенты, B1) → C sim (оружие) → D гейты sim → E клиент → F QA → G проверка.

### A. Данные (все новые числа — только здесь)

**A1. `assets/character/locomotion.ron`** — добавить в конец:
```ron
    head_height: 1.6,
    head_radius: 0.35,
```
(центр сферы над ногами и её радиус, м).

**A2. `assets/combat/weapons.ron`** (новый, UTF-8 без BOM). Урон, темп, магазин, перезарядка, базовый разброс (половинный угол конуса), дальность — таблица GDD §4.1; остальное стартовые данные:
```ron
(
    headshot_multiplier: 2.0,
    pistol: (damage: 25.0, pellets: 1, fire_interval: 0.3, magazine: 12, reload: 1.2, range: 60.0, falloff: None,
             spread: (base_deg: 1.0, per_shot_deg: 1.0, max_bloom_deg: 3.0, recovery_delay: 0.35, recovery_deg_per_s: 6.0, moving_deg_per_mps: 0.3),
             max_reserve: 120, pickup_ammo: 24),
    smg:    (damage: 12.0, pellets: 1, fire_interval: 0.08, magazine: 30, reload: 1.8, range: 45.0, falloff: None,
             spread: (base_deg: 3.0, per_shot_deg: 0.4, max_bloom_deg: 4.0, recovery_delay: 0.15, recovery_deg_per_s: 8.0, moving_deg_per_mps: 0.4),
             max_reserve: 300, pickup_ammo: 60),
    shotgun: (damage: 8.0, pellets: 10, fire_interval: 0.9, magazine: 6, reload: 2.5, range: 25.0,
             falloff: Some((start: 10.0, min_factor: 0.3)),
             spread: (base_deg: 6.0, per_shot_deg: 2.0, max_bloom_deg: 3.0, recovery_delay: 1.0, recovery_deg_per_s: 4.0, moving_deg_per_mps: 0.2),
             max_reserve: 48, pickup_ammo: 12),
    pickups: (radius: 1.0, respawn: 30.0),
    range: (dummies: 3, dummy_spacing: 3.0, dummy_distance: 10.0, pickup_spacing: 2.0, dummy_reset: 3.0),
)
```
Инвариант данных (проверяется `validate`, см. C1): `recovery_delay > fire_interval` у каждого ствола, иначе при удержании огня разброс не растёт.

**A3. `assets/combat/aim.ron`** (новый): `(max_aim_distance: 200.0, min_aim_distance: 0.5, muzzle_offset: (0.25, 0.35, -0.45))` — дальность луча 1; минимальная глубина точки P перед дулом, ближе — стреляем по направлению прицела; дуло относительно центра тела в кадре yaw прицела (x вправо, y вверх, z вперёд = −Z). Геометрии камеры здесь нет (GDD §3.2).

**A4. `assets/camera/camera.ron`** — добавить (значения GDD §3.2): `aim_shoulder_offset: 0.55, aim_distance: 2.0, aim_fov_deg: 55.0, aim_transition: 0.15, aim_sensitivity_scale: 0.7`.

**A5. `assets/juice/juice.ron`** (новый):
```ron
(
    recoil_deg: (pistol: 1.0, smg: 0.5, shotgun: 2.0),
    recoil_half_life: 0.08,
    flash: (seconds: 0.05, size: 0.25, color: (1.0, 0.8, 0.4), light_intensity: 100000.0, light_range: 6.0),
    tracer: (seconds: 0.06, width: 0.02, color: (1.0, 0.9, 0.6)),
)
```
(отдача 0.5-2°, вспышка 50 мс, трассер 60 мс — GDD §8; интенсивность в люменах, `bevy_light-0.19.1/src/point_light.rs:53`, подбирает владелец).

**A6. `assets/audio/mix.ron`** (новый): `(shot: (pistol: (seconds: 0.18, decay: 22.0, volume: 0.5), smg: (seconds: 0.10, decay: 35.0, volume: 0.35), shotgun: (seconds: 0.35, decay: 12.0, volume: 0.7)))` — длина всплеска, коэффициент экспоненты огибающей (1/с), громкость.

**A7. `assets/ui/strings.ron`**, секция `hud` — добавить: `ammo_size: 22.0, ammo_color: (1.0, 1.0, 1.0), crosshair_dot: 4.0, crosshair_arm: 8.0, crosshair_thickness: 2.0, crosshair_color: (1.0, 1.0, 1.0, 0.9), hit_marker_size: 28.0, hit_marker_seconds: 0.1, hit_marker_color: (1.0, 1.0, 1.0), kill_marker_color: (0.9, 0.1, 0.1)` (хит-маркер 0.1 с — GDD §7).

**A8. `assets/world/render.ron`** — новая секция `weapons: (held_size: (0.08, 0.12, 0.35), pickup_size: (0.15, 0.2, 0.6), ammo_size: 0.3, pistol_color: (0.2, 0.2, 0.22), smg_color: (0.35, 0.3, 0.2), shotgun_color: (0.45, 0.25, 0.12), ammo_color: (0.85, 0.75, 0.2))`.

### B. Симуляция: слои, голова, интенты, B1

**B1. `crates/gta_sim/src/layers.rs`** (новый) + `pub mod layers;` в `lib.rs`: `#[derive(PhysicsLayer, Default, Clone, Copy, Debug)] pub enum GameLayer { #[default] World, Character, Hitbox }` и `///`-строка: закон GDD §12, `World` — слой по умолчанию всей статики.

**B2. `character/locomotion.rs`**: поля `head_height`, `head_radius` в `LocomotionConfig`; в `validate()` после проверки скоростей — `head_radius > capsule_radius`, `head_height + head_radius > float_height + capsule_height / 2.0`, `head_height < float_height + capsule_height / 2.0`, конечность. Сообщение называет `head_radius`/`head_height` и причину ("иначе луч всегда попадает в капсулу раньше головы").

**B3. `character/mod.rs`**:
- `#[derive(Component, Reflect, Default)] #[reflect(Component)] pub struct HeadHitbox;` + `register_type`.
- `pub fn head_hitbox(cfg: &LocomotionConfig) -> impl Bundle` = `(HeadHitbox, Name::new("Head hitbox"), Collider::sphere(cfg.head_radius), Sensor, CollisionLayers::new(GameLayer::Hitbox, LayerMask::NONE), Transform::from_xyz(0.0, cfg.head_height - cfg.float_height, 0.0))`.
- В `character_components` (`:95-117`) добавить `CollisionLayers::new(GameLayer::Character, LayerMask::ALL)` и `children![head_hitbox(cfg)]`.
- `#[require(...)]` у `Character` (`:29`): добавить `AimIntent, ActionIntent`; `register_type` обоих.
- `drive_characters` (`:119-171`): в запрос `&AimIntent`; `desired_forward`: если `aim.aiming` — `Dir3::new(Vec3::new(aim.direction.x, 0.0, aim.direction.z)).ok()` (тело смотрит по yaw прицела, движение остаётся камерным → strafe, GDD §3.2), иначе как сейчас. Ветку `dead` не трогать.

**B4. `character/intent.rs`**: 
```rust
#[derive(Component, Reflect, Default, Clone, Debug)] #[reflect(Component, Default)]
pub struct AimIntent { pub origin: Vec3, pub direction: Vec3, pub aiming: bool }
#[derive(Component, Reflect, Default, Clone, Debug)] #[reflect(Component, Default)]
pub struct ActionIntent { pub fire_held: bool, pub fire_requested: bool, pub reload_requested: bool,
                          pub select: Option<WeaponRequest>, pub cycle: i32 }
#[derive(Reflect, Clone, Copy, Debug, PartialEq, Eq)]
pub enum WeaponRequest { Unarmed, Gun(Weapon) }
```
Клавиша 1 → `Unarmed`, 2-4 → `Gun(..)`. `Weapon` определён в `combat/weapons.rs` (C1) и импортируется сюда (`crate::combat::Weapon`; взаимный импорт модулей внутри одного крейта допустим). Один источник типа.
Реэкспорт в `character/mod.rs:11`.

**B5. `character/health.rs`**: `impl Health { pub fn take(&mut self, amount: f32) -> bool }` — `(current, armor) = apply_damage(...)`, `since_damage = 0`, возвращает `current <= 0.0` (убит этим ударом). `player/mod.rs:72-75` `apply_debug_damage` переходит на `health.take(amount)` (одно правило брони для отладки и оружия).

**B6. B1 — `flow/wasted.rs`**: `pub(super) fn drop_queued_damage(mut queued: ResMut<Messages<DebugDamage>>) { queued.clear(); }` с однострочным комментарием "читатель урона стоит в PlayingSystems: без очистки сообщение из Wasted бьёт возрождённого игрока". `flow/mod.rs:58`: `OnExit(GameState::Wasted)` → `(wasted::respawn_player, wasted::drop_queued_damage)`.

**B7. `lib.rs` `compose_sim`**: загрузка+валидация `WeaponsConfig` (`combat/weapons.ron`) и `AimConfig` (`combat/aim.ron`) по образцу `:35-44`, вставка ресурсов до `add_plugins`.

**B8. `gta_sim/Cargo.toml`**: `rand_chacha = { version = "=0.10.0", default-features = false }` (как в `crates/citygen/Cargo.toml`). Проверено офлайн: `cargo metadata --offline` в копии манифестов (`scratch/lockprobe/`) резолвится, diff `Cargo.lock` — одна строка `"rand_chacha",` в зависимостях `gta_sim`. Готовый lock: `D:/test-gta-like/maw/tasks/in_progress/TASK-007/scratch/Cargo.lock.with_rand_chacha` (или просто `cargo build --offline` обновит lock сам: версия уже закреплена и в кэше).

### C. Симуляция: оружие (`crates/gta_sim/src/combat/`)

**C1. `combat/weapons.rs`** (новый):
- `pub const WEAPONS_CONFIG: &str = "combat/weapons.ron";`
- `#[derive(Reflect, Clone, Copy, Debug, PartialEq, Eq, Deserialize?)] pub enum Weapon { Pistol, Smg, Shotgun }` + `pub const ALL: [Weapon; 3]`; `///` "порядок = индекс `Loadout.guns`".
- `WeaponsConfig {headshot_multiplier, pistol, smg, shotgun: WeaponStats, pickups: WeaponPickupConfig, range: RangeConfig}`, `WeaponStats {damage, pellets: u32, fire_interval, magazine: u32, reload, range, falloff: Option<Falloff>, spread: SpreadConfig, max_reserve: u32, pickup_ammo: u32}`, `Falloff {start, min_factor}`, `SpreadConfig {base_deg, per_shot_deg, max_bloom_deg, recovery_delay, recovery_deg_per_s, moving_deg_per_mps}`, `WeaponPickupConfig {radius, respawn}`, `RangeConfig {dummies: u32, dummy_spacing, dummy_distance, pickup_spacing, dummy_reset}` — все `#[serde(deny_unknown_fields)]`. `fn stats(&self, Weapon) -> &WeaponStats`. `validate()`: конечность; `damage > 0`, `pellets ≥ 1`, `fire_interval > 0`, `magazine ≥ 1`, `reload ≥ 0`, `range > 0`, разбросы ≥ 0, `recovery_delay > fire_interval`, `0 < falloff.start < range`, `min_factor ∈ [0,1]`, `headshot_multiplier ≥ 1`, `pickup_ammo ≥ 1`, `max_reserve ≥ pickup_ammo`, `pickups.radius > 0`, `respawn ≥ 0`, `range.dummies ≥ 1`, расстояния > 0, `pickup_spacing > pickups.radius` (соседний пикап не забирается вместе). Сообщение называет поле с префиксом ствола (`smg.spread.recovery_delay`).
- `#[derive(Reflect, Default, Clone, Copy, Debug, PartialEq)] pub struct GunSlot { pub owned: bool, pub magazine: u32, pub reserve: u32 }`.
- `#[derive(Component, Reflect, Default, Clone, Debug)] #[reflect(Component, Default)] pub struct Loadout { pub held: Option<Weapon>, pub guns: [GunSlot; 3], pub cooldown: f32, pub reload_left: f32, pub since_shot: f32, pub bloom_deg: f32, pub spread_deg: f32 }` (`spread_deg` — производное состояние для HUD и гейта, пишет только `tick_loadouts`/`fire_weapons`).
- Чистые функции с `///`: `cycle_weapon(held, owned: [bool; 3], step: i32) -> Option<Weapon>` (кольцо `[None, Pistol, Smg, Shotgun]` по владению, `None` всегда в кольце); `falloff_factor(stats, distance) -> f32` (1 до `start`, линейно до `min_factor` на `range`); `acquire(slot, stats, gun: bool) -> bool` (ствол: `owned = true`, если впервые — `magazine = min(magazine_size, pickup_ammo)`, остаток в `reserve`; патроны: `reserve = min(reserve + pickup_ammo, max_reserve)`; `false`, если ничего не изменилось).
- Система `tick_loadouts(cfg, time: Res<Time<Fixed>>, Query<(&mut Loadout, &mut ActionIntent, &LinearVelocity), (With<Character>, Without<Dead>)>)`, порядок внутри тика (от него зависят числа гейтов D):
  1. `select`/`cycle` → новый `held` (только владеемый ствол), при смене `reload_left = 0`; защёлки сбросить.
  2. `since_shot += dt`; `cooldown = (cooldown - dt).max(0)`.
  3. если `reload_left > 0`: `reload_left -= dt`; при `≤ 0` — перенос `min(magazine_size - magazine, reserve)` из запаса, `reload_left = 0`.
  4. если `since_shot ≥ recovery_delay`: `bloom = (bloom - recovery_deg_per_s·dt).max(0)`.
  5. `reload_requested` (сбросить): если ствол, не перезаряжается, магазин не полон, запас > 0 → `reload_left = reload`.
  6. `spread_deg = base_deg + bloom_deg + moving_deg_per_mps · |v_xz|` (0 без ствола).

**C2. `combat/hitscan.rs`** (новый):
- `pub const AIM_CONFIG: &str = "combat/aim.ron";` `AimConfig {max_aim_distance, min_aim_distance, muzzle_offset: (f32, f32, f32)}` + `validate`.
- Сообщения (buffered, FixedUpdate → Update, `add_message`): `ShotFired { shooter: Entity, weapon: Weapon, muzzle: Vec3 }` (одно на спуск: вспышка, звук, отдача) и `BulletTrace { shooter: Entity, from: Vec3, to: Vec3, hit: BulletHit }` (одно на дробину: трассер, хит-маркер), `enum BulletHit { Nothing, Surface, Body { killed: bool }, Head { killed: bool } }`.
- `#[derive(Resource)] pub struct SpreadRng(ChaCha8Rng)` c `Default` = `ChaCha8Rng::seed_from_u64(0)` (rand_core 0.10: `Rng`, `SeedableRng` — как `crates/citygen/src/rng.rs:1-4`).
- `pub fn aim_yaw(direction: Vec3) -> f32 { f32::atan2(-direction.x, -direction.z) }`, `pub fn muzzle(position, direction, offset) -> Vec3 = position + Quat::from_rotation_y(aim_yaw(direction)) * offset`. Три направленных примера (`R_y(θ)·(x,y,z) = (x cosθ + z sinθ, y, −x sinθ + z cosθ)`, offset (0.25, 0.35, −0.45)):
  1. dir (0,0,−1): yaw 0 → (0.25, 0.35, −0.45): вперёд −Z, вправо +X.
  2. dir (−1,0,0): yaw +90° → (0.25·0 + (−0.45)·1, 0.35, −0.25·1 + (−0.45)·0) = (−0.45, 0.35, −0.25): вперёд −X, вправо −Z (как D при yaw 90°, GDD §3.2).
  3. dir (0,0,1): yaw 180° → (−0.25, 0.35, 0.45): вперёд +Z, вправо −X.
  Юнит-тест на эти три случая.
- `pub fn cone_sample(axis: Dir3, half_angle: f32, u: f32, v: f32) -> Dir3`: `cosθ = 1 − u(1 − cos α)`, `φ = 2πv`, `(b1, b2) = axis.any_orthonormal_pair()` (glam 0.32.1 `Vec3::any_orthonormal_pair`, `f32/vec3.rs:1227`), `d = a·cosθ + (b1 cosφ + b2 sinφ)·sinθ` — равномерно по сферической шапке, угол к оси ≤ α.
- Система `fire_weapons(cfg, aim_cfg, spatial: SpatialQuery, mut rng: ResMut<SpreadRng>, shooters: Query<(Entity, &Position, &AimIntent, &mut ActionIntent, &mut Loadout, &LinearVelocity), (With<Character>, Without<Dead>)>, colliders: Query<(&ColliderOf, Has<HeadHitbox>)>, mut targets: Query<&mut Health, Without<Dead>>, mut fired: MessageWriter<ShotFired>, mut traces: MessageWriter<BulletTrace>)`. Golden Path: на каждого стрелка
  1. `wants = fire_held || fire_requested`; `fire_requested = false`. Нет `held`, `!wants`, `reload_left > 0`, `cooldown > 0` → continue. `let Ok(dir) = Dir3::new(aim.direction) else continue` (пустой интент не тратит патрон).
  2. Магазин 0 → если запас > 0, `reload_left = reload` (автоперезарядка), continue.
  3. `magazine -= 1; cooldown = fire_interval; since_shot = 0`; спред выстрела = текущий `spread_deg`; затем `bloom = min(bloom + per_shot, max_bloom)`, `spread_deg` пересчитать.
  4. Фильтр `SpatialQueryFilter::from_mask([World, Character, Hitbox])`, предикат `|e| colliders.get(e).map_or(true, |(c, _)| c.body != shooter)`. Луч 1: `cast_ray_predicate(aim.origin, dir, max_aim_distance, true, ..)` → `P`. Дуло `m = muzzle(position, dir, offset)`. Ось дробин: `(P − m)`, если `(P − m)·dir > min_aim_distance`, иначе `dir`.
  5. `ShotFired`. На каждую из `pellets`: `d = cone_sample(axis, spread.to_radians(), rng, rng)`; луч 2 `cast_ray_predicate(m, d, range, true, ..)`. Нет попадания → `BulletTrace{hit: Nothing, to: m + d·range}`. Есть: `(body, head) = colliders.get(hit.entity)` (без `ColliderOf` → body = entity, head = false); `targets.get_mut(body)` → `Ok` (живой персонаж с `Health`): `amount = damage × falloff_factor × (head ? headshot_multiplier : 1)`, `killed = health.take(amount)`, `Body/Head{killed}`; `Err` (стена, мёртвое тело) → `Surface`. `to = m + d·hit.distance`.
- Мёртвый манекен останавливает луч, но урона не получает (GDD: голова отключается в knockdown/смерти — для T6 эквивалентно, т.к. `Dead` не получает урон; knockdown — T7).

**C3. `combat/pickups.rs`** — рядом с существующим `Pickup` (его не трогать: гейты T5 и `t5.py` считают ровно 2 `Pickup` и используют `kind` как ключ словаря):
- `#[derive(Component, Reflect, Debug)] #[reflect(Component)] pub struct WeaponPickup { pub weapon: Weapon, pub ammo_only: bool, pub cooldown: f32 }` + `available()`.
- `collect_weapon_pickups(cfg, time, pickups, players: Query<(&Position, &CharacterBody, &mut Loadout), (With<Player>, Without<Dead>)>)` по образцу `collect_pickups:49-81`: ноги в `pickups.radius` → `acquire(...)`; если `true` — `cooldown = respawn`; если ствол и `held == None` → `held = Some(weapon)` (автовыбор, как в GTA).

**C4. `combat/range.rs`** (новый):
- `#[derive(Component, Reflect, Default)] #[reflect(Component)] pub struct Dummy { pub reset_left: f32 }`.
- `pub fn dummy_bundle(loco: &LocomotionConfig, handle: Handle<CharacterSchemeConfig>, health: &HealthConfig, feet: Vec3) -> impl Bundle` = `(Dummy::default(), Name::new("Dummy"), Transform::from_translation(feet + Vec3::Y * loco.float_height), character_components(loco, handle), Health::full(health))`. Используется и полигоном, и гейтами.
- `spawn_range(...)` на `OnTransition{Loading→Playing}` с `.run_if(resource_exists::<CityLandmarks>)` (одноразовый спавн, не state-гейт): центр `c = park_center`; манекены `c + X·(i − (n−1)/2)·dummy_spacing − Z·dummy_distance`; ряд пикапов по X через `pickup_spacing` с центром в `c`: ствол и патроны на каждое оружие (6 шт.), `Transform::from_translation(point)`.
- `dummy_life(cfg, health_cfg, time, dead_now: Query<(Entity, &Health, &mut Dummy), Without<Dead>>, dead: Query<(Entity, &mut Health, &mut Dummy), With<Dead>>, commands)` в `HealthSystems::Death`: `current <= 0` → `insert(Dead)`, `reset_left = dummy_reset`; у `Dead`: `reset_left -= dt`, `≤ 0` → `Health::full`, `try_remove::<Dead>()`. (Две выборки с `With/Without<Dead>` — без конфликта.)

**C5. `combat/mod.rs`**: модули, реэкспорты (`Weapon, WeaponsConfig, WEAPONS_CONFIG, Loadout, GunSlot, AimConfig, AIM_CONFIG, ShotFired, BulletTrace, BulletHit, WeaponPickup, Dummy, dummy_bundle, cone_sample, falloff_factor, cycle_weapon, muzzle, aim_yaw`), `init_resource::<SpreadRng>()`, `add_message::<ShotFired>()`, `add_message::<BulletTrace>()`, `register_type` (`Loadout, GunSlot, Weapon, WeaponPickup, Dummy`), системы:
```rust
.add_systems(FixedUpdate, (
    (weapons::tick_loadouts, hitscan::fire_weapons).chain().in_set(HealthSystems::Damage),
    pickups::collect_weapon_pickups.in_set(HealthSystems::Pickup),
    range::dummy_life.in_set(HealthSystems::Death),
).in_set(PlayingSystems))
.add_systems(OnTransition { exited: Loading, entered: Playing }, range::spawn_range.run_if(resource_exists::<CityLandmarks>))
```
Файлы < 400 строк каждый.

**C6. `player/mod.rs:46-60`** `spawn_player`: добавить `Loadout::default()` (игрок стартует с кулаками). `respawn_player` не трогает `Loadout` → оружие сохраняется при смерти (GDD §3.4, Q3=A).

### D. Гейты `cargo test -p gta_sim` (каждый — flip-RED, возмущение указано)

Хелперы в `tests/common/mod.rs`: `set_aim(app, origin, target)`, `set_action(app, |a| ..)`, `loadout(app)`, `set_loadout(app, |l| ..)`, `spawn_dummy(app, feet) -> Entity` (через `combat::dummy_bundle` и ресурсы приложения), `health_of(app, e)`, `traces(app) -> Vec<BulletTrace>` (свежий курсор `Messages::get_cursor()`, `messages.rs:176`, читать сразу после тика выстрела), `spawn_wall(app, center, size)` (статический кубоид — фикстура). Позиции ниже — в свободной зоне `TestArea` (x = −20); после спавна целей `run_ticks(8)`.

Геометрия (из данных A1-A3): ноги стрелка (−20, 0, 30), центр тела y = 1.05, дуло (yaw 0) = (−19.75, 1.40, 29.55). Манекен: ноги (−20, 0, 20), капсула r 0.3 от 0.3 до 1.8 м, голова — сфера r 0.35 с центром 1.6 (охватывает капсулу выше ≈1.42 м).

**D1. `tests/shooting.rs::pistol_hits_dummy_at_10m_for_table_damage`** (AC1). Пистолет в руке (12/0), `AimIntent` из (−20, 1.05, 30) в грудь (−20, 1.0, 20), `fire_requested`, 1 тик. Вывод: P на передней поверхности капсулы (z ≈ 20.3); отклонение дробины ≤ 9.3 м·tan 1° = 0.16 м < 0.3 и по высоте 1.0 ± 0.16 < 1.42 → только тело. Ожидание: `Health.current == 100 − weapons.pistol.damage` (75), магазин 11, одна `BulletTrace` с `Body{killed:false}`. RED: урон из чужой строки таблицы / множитель головы для тела.

**D2. `wall_between_muzzle_and_target_blocks`** (AC2). Стена: центр (−20, 0.8, 29.0), размер (2.0, 1.6, 0.2) (z 28.9..29.1, верх 1.6). `AimIntent.origin` = (−20, 2.5, 33) (камера сзади-сверху) в (−20, 1.0, 20). Луч 1 над стеной: при z = 29.1 высота 2.5 − 1.5·(3.9/13) = 2.05 > 1.6 → P на манекене. Луч 2 из дула (1.40, z 29.55): через 0.45 м высота ≈1.38 < 1.6 → стена. Ожидание: здоровье 100, трейс `Surface` с `to.z ≈ 29.1`. Отрицательный контроль в том же тесте: `despawn` стены, `run_ticks(20)` (кулдаун 0.3 с = 20 тиков, C1), выстрел → 75 (установка вообще способна попасть). RED: луч 2 заменить точкой P луча 1.

**D3. `head_sensor_doubles_damage`** (AC3). Как D1, цель (−20, 1.65, 20). P на сфере головы; отклонение ≤ 0.16 м от точки на высоте ~1.65 — все точки в пределах 0.23 м от центра сферы (< 0.35) → `HeadHitbox`. Ожидание: 100 − 25·2 = 50, `Head{killed:false}`. RED: убрать `Has<HeadHitbox>` из разрешения или поставить голову внутрь капсулы (`head_radius` 0.25 во временном корне) — второе ловится ещё и валидацией B2.

**D4. `reload_takes_configured_time`** (AC4). Пистолет, магазин 3, запас 20; `reload_requested`; `run_ticks(1)` (тик R: шаг 5 ставит 1.2). Вывод по порядку C1: на тике R+k `1.2 − k/64 ≤ 0` ⇔ k ≥ 76.8. `run_ticks(76)` → всё ещё 3/20 и удержание огня не стреляет; `run_ticks(1)` → 12/11. Ожидаемые тики вычислять в тесте из `weapons.ron` (`ceil(reload·64)`), не хардкодить 77. RED: завершать перезарядку сразу / убрать проверку `reload_left > 0` в `fire_weapons`.

**D5. `shotgun_fires_ten_pellets`** (AC5). Дробовик, ноги стрелка (−20, 0, 22.0), цель грудь (−20, 1.0, 20). Вывод: дуло z 21.55, до оси манекена ≈1.55 м; отклонение ≤ 1.55·tan 6° = 0.163 м, плюс боковой сдвиг дула к оси ≈0.06 → ≤ 0.22 < 0.3; по высоте в пределах 0.3..1.42 → все дробины в тело, расстояние < `falloff.start`. Ожидание: `traces().len() == weapons.shotgun.pellets` (10), все `Body`, здоровье 100 − 10·8 = 20. RED: цикл на 1 дробину.

**D6. `spread_grows_in_series_and_recovers`** (AC6). SMG, стрелок стоит, прицел в пустоту (−Z), `fire_held = true`. Вывод по C1 (dt = 1/64): кулдаун 0.08 − k/64 ≤ 0 ⇔ k ≥ 5.12 → выстрел каждые 6 тиков; восстановление требует `since_shot ≥ 0.15` ⇔ k ≥ 9.6 → между выстрелами его нет. После n-го выстрела `bloom = min(0.4n, 4.0)`: строго растёт до 10-го выстрела, дальше 4.0 (допуск 1e-4); `spread_deg == base + bloom`. Затем `fire_held = false`: после последнего выстрела на k = 9 bloom 4.0, на k = 10 — 3.875, на k = 41 — 0 (4.0 − 32·0.125). Числа тест берёт из `weapons.ron` и проверяет выведенные моменты. Дополнительно юнит `cone_sample`: 1000 сэмплов с α = 6° — угол ≤ α + 1e-4; α = 0 → ось. RED: убрать `recovery_delay`/`bloom +=`.

**D7. `tests/shooting.rs::head_hitbox_is_not_ground`** (GDD: "сначала проверить, что сенсор земли Tnua его не видит"). Фикстура: статический кубоид 2×1.0×2 с центром (−20, 1.3, 25) (верх 1.8) и дочерний `character::head_hitbox(cfg)` (центр 1.3 + 0.55 = 1.85, верх 2.2). Игрока ставим над кубоидом, `run_ticks(128)`. Ожидание: `y ≈ 1.8 + float_height = 2.85` (±0.05). Если Tnua видит голову — 2.2 + 1.05 = 3.25. RED: в тестовой копии бандла без `Sensor` и со слоями по умолчанию.

**D8. `tests/respawn.rs::damage_queued_during_wasted_is_dropped`** (B1). `headless_app()`, `settle`, `kill`, затем пока `Wasted`: `write_damage(30)` перед каждым `app.update()`. После входа в `Playing`: `health == (max, 0)`, и после `run_ticks(4)` тоже. RED: убрать `drop_queued_damage` → 70 (проба 1.6). В `death_wasted_respawn_at_hospital` заменить заглушку `Loadout(7)` (`:12-14, :481, :517`) настоящим `combat::Loadout` (пистолет, 7/20) и сравнивать его до/после; `world_counts` расширить `Dummy` и `WeaponPickup` (одноразовость полигона на city seed 1).

**D9. `tests/shooting.rs::aiming_turns_body_to_aim_yaw`** (strafe). `MoveIntent.axis = (0,1)`, yaw 0 (движение −Z), `AimIntent {direction: (−1, 0, 0), aiming: true}`, 64 тика. Ожидание: `Rotation * −Z` · (−1,0,0) > 0.99 и смещение по −Z (движение не повернулось). RED: убрать ветку `aiming`.

**D10. `weapon_and_ammo_pickups`**: `WeaponPickup{Pistol, ammo_only:false}` на (−20, 0, 35), встать ногами → `owned`, `magazine = min(12, 24) = 12`, `reserve = 12`, `held == Some(Pistol)`, кулдаун > 0; `ammo_only` пикап → запас +24; `run_ticks(respawn·64 + 2)` → доступен. `dummy_dies_and_resets`: `spawn_dummy`, `Health.current = 0` → через тик `Dead`; через `ceil(dummy_reset·64)` тиков — полное здоровье, без `Dead`.

**D11. Юниты в модулях**: `cycle_weapon` (владение {Pistol, Shotgun}: None +1 → Pistol; Shotgun +1 → None; None −1 → Shotgun), `falloff_factor` дробовика (5 м → 1.0; 17.5 м → 0.65; 25 м → 0.3), `muzzle` (3 примера C2), `acquire`.

**D12. `tests/config.rs`**: `shipped_weapons_config_loads`, `shipped_aim_config_loads`, `head_must_enclose_capsule_top` (замена `head_radius: 0.35` на `0.25` во временном корне → ошибка содержит `head_radius`), `smg_recovery_delay_validated` (замена `recovery_delay: 0.15` на `0.05` → ошибка содержит `smg.spread.recovery_delay`). Шаблон `health_error` (`config.rs:125-147`).

Классы гейтов: D1-D5, D8 — корректность (числа выведены); D6 — корректность арифметики разброса + юнит границы конуса; D7 — корректность фильтрации Tnua; D9, D10 — поведенческая живость.

### E. Клиент (`src/`)

**E1. `camera/config.rs` + `camera/mod.rs`**:
- 5 полей A4 в `CameraConfig`.
- `OrbitCamera`: добавить `aim_blend: f32` (0..1).
- `apply_mouse_look`: чувствительность × `lerp(1, aim_sensitivity_scale, eased(aim_blend))`.
- `follow_player`: `aim_blend` к цели (`AimIntent.aiming` игрока) линейно со скоростью `1/aim_transition` по `Time<Real>`, ease-out `1 − (1 − t)²`; плечо, дистанция и FOV (`Projection::Perspective`) интерполируются между обычными и прицельными. Фильтр обоих `cast_shape` → `SpatialQueryFilter::from_mask(GameLayer::World).with_excluded_entities([entity])` (**обязательно**: иначе сфера из pivot внутри собственной головы-сенсора даёт попадание на дистанции 0 и камера схлопывается; персонажи камеру тоже не толкают — конвенция TPS).
- После позиционирования записать `AimIntent.origin = camera_translation`, `direction = rotation * −Z` **до** применения отдачи; затем `camera_transform.rotation = rotation * Quat::from_rotation_x(recoil.pitch)` (визуал). Отдача на луч не влияет (GDD §4.1).

**E2. `input/mod.rs`**: действия в `OnFoot`: `Fire` (`MouseButton::Left`, bool), `Aim` (`MouseButton::Right`, bool), `Reload` (`KeyCode::KeyR`), `Slot1..Slot4` (`Digit1..Digit4`), `CycleWeapon` (`f32`, `(Binding::mouse_wheel(), SwizzleAxis::YXZ)`, `bevy_enhanced_input-0.26.0/src/binding.rs:61-94`). Система `write_action_intent` в `Update`: если `!CursorCaptured` — ничего (клик захвата курсора не стреляет); `fire_held = ***fire`; `START` → `fire_requested/reload_requested = true`; `Slot1` → `select = Unarmed`, `Slot2..4` → `Gun(Pistol/Smg/Shotgun)`; `cycle += sign(wheel)`; `AimIntent.aiming = ***aim`. Защёлки только поднимаются, опускает `FixedUpdate`.

**E3. `src/juice/mod.rs`** (новый) + `JuiceConfig` (`juice/juice.ron`, `validate`): ресурс `CameraRecoil { pitch }`; система в `Update` читает `ShotFired` игрока → `pitch += recoil_deg(weapon).to_radians()`, затем `pitch.smooth_nudge(&0.0, LN_2 / recoil_half_life, real_dt)`. Камера (E1) читает ресурс.

**E4. `src/vfx/mod.rs`** (новый): общие хэндлы (единичный кубоид, unlit-материалы вспышки и трассера) в `FromWorld`-ресурсе; `ShotFired` → квад вспышки в дуле + `PointLight { intensity, range, shadow_maps_enabled: false }` на `flash.seconds`; `BulletTrace` → кубоид, растянутый `from→to` (`Transform` looking_to + scale (width, width, len)) на `tracer.seconds`; компонент `Lifetime(f32)` и система, уменьшающая его по `Time<Real>` и делающая `despawn`. Пул не заводим: ≤ ~20 короткоживущих сущностей/с (SMG 12.5 выстр./с, дробовик 10 дробин/0.9 с) при общих хэндлах; пул — по замеру (GDD §11 "не гадай").

**E5. `src/audio/mod.rs`** (новый) + `MixConfig` (`audio/mix.ron`): `#[derive(Asset, TypePath)] struct ShotSound {seconds, decay, volume}` с `Decodable` → `NoiseBurstDecoder` (xorshift32-шум × `exp(−decay·t)`, моно, 44 100 Гц — технический `const`, конец через `None` после `seconds·rate` сэмплов, `total_duration = Some`). Образец API — `bevy-0.19.1/examples/audio/decodable.rs` (`ChannelCount::new(1)`, `SampleRate::new(44_100)`, `Source::{current_span_len, channels, sample_rate, total_duration}`). `app.add_audio_source::<ShotSound>()`; хэндлы трёх звуков в ресурсе; `ShotFired` → `commands.spawn((AudioPlayer(handle), PlaybackSettings::DESPAWN.with_volume(Volume::Linear(volume))))` (`bevy_audio-0.19.1/src/audio.rs:106,130`). Громкость в `mix.ron` (T13 заменит синтез, не интерфейс).

**E6. `src/hud/weapon.rs`** (новый) + `hud/mod.rs`: на `OnTransition{Loading→Playing}` — текст патронов под полосами (`UiFonts.regular`, `"{magazine} / {reserve}"`, скрыт без ствола, `set_if_neq`), прицел по центру (точка `crosshair_dot`; при `aim_blend > 0` четыре штриха, зазор `px = tan(spread_deg) / tan(fov/2) · (высота_окна / 2)`), хит-маркер (`Text "×"` размера `hit_marker_size`, цвет белый/`kill_marker_color`, `Visibility::Hidden` по таймеру `hit_marker_seconds` на `Time<Real>`) по `BulletTrace` игрока с `Body/Head`. Все числа — `UiConfig.hud` (A7), валидация в `menu/config.rs:59-97` (позитивные размеры, цвета в [0,1]).

**E7. `src/visuals/weapons.rs`** (новый) + `visuals/config.rs` (секция `weapons`, A8, `validate`): наблюдатель `On<Add, WeaponPickup>` → кубоид `pickup_size` (ствол) или куб `ammo_size` (патроны) цвета оружия/патронов, видимость по `available()` (как `visuals/pickups.rs:31-41`); ствол в руке — дочерний кубоид `held_size` игрока в точке `AimConfig.muzzle_offset + (0, 0, held_size.z/2)`, цвет по `Loadout.held`, скрыт без ствола.

**E8. `main.rs`**: загрузить `JuiceConfig`, `MixConfig` (как `:142-170`), `validate` в `preflight`, вставить ресурсы, добавить `juice::JuicePlugin, vfx::VfxPlugin, audio::ShotAudioPlugin` к `:186-193`; `mod juice; mod vfx; mod audio;`.

### F. Runtime QA (GDD D1)

**F1. `tools/qa/brp.py`**: метод `send_mouse_button(self, button, ms)` → `brp_extras/send_mouse_button` с `{"button": "Left"|"Right", "duration_ms": ms}` (`bevy_brp_extras-0.22.6/src/mouse/button.rs:26-37`, по умолчанию 100 мс, максимум 60 000).

**F2. `tools/qa/scenarios/t6.py`** (по образцу `t5.py`, release, `--seed 1`, `--out`):
1. `CityLayoutHash` == golden, чанки устоялись. Прочитать `CityLandmarks.park_center`, манекены (`Dummy` + `Position` + `Health`, ожидать `range.dummies` = 3), `WeaponPickup` (6).
2. Телепорт на пикап пистолета → 0.5 с → `Loadout.guns[0].owned`, `held` = Pistol (парсер `Option`: строка `"None"`, `{"Some": x}` или `null` — толерантный, как `state_name`).
3. Телепорт в 10 м перед средним манекеном (+Z), ждать 1 с (сглаживание pivot). Наведение `move_mouse`: читать `AimIntent` игрока, считать требуемые Δyaw/Δpitch до груди манекена (1.0 м над ногами), `dx = −Δyaw° / 0.12`, `dy = −Δpitch° / 0.12` (`camera.ron`: мышь вправо уменьшает yaw, вниз уменьшает pitch), 2-3 итерации, пока ближайшее расстояние луча `AimIntent` до оси манекена на высоте груди < 0.1 м.
4. `send_mouse_button("Left", 100)` → 0.4 с → `Health` манекена == 75 (пистолет 25, `weapons.ron`), магазин 11. Жёсткий pass/fail.
5. Телепорт на пикап SMG, `send_keys(["Digit3"], 100)`, повторить наведение; `send_mouse_button("Right", 4000)` (прицел), через 0.5 с `send_mouse_button("Left", 800)`, скриншоты через ~0.25 и ~0.45 с после начала очереди (трассеры 60 мс каждые 80 мс, хит-маркер 0.1 с на каждое попадание) — `aim_burst_1.png`, `aim_burst_2.png`; `AimIntent.aiming == true` во время удержания; здоровье манекена упало.
6. `send_keys(["KeyR"], 100)`, через `reload + 0.5` с магазин SMG полон.
7. Лог без ошибок asset/font, `shutdown`. `summary.json` со всеми числами.
Скриншоты — доказательство для владельца и мультимодального QA, не pass/fail.

**F3. Чек-лист владельца (QA пишет в `QA_REPORT.md`)**: `cargo run --features fast`, в парке у центра города: подобрать пистолет/SMG/дробовик, стрелять с ПКМ и без; отдача заметна и не тошнит; звук-заглушка не режет слух; трассер и вспышка читаются; хит-маркер и красный при убийстве видны; strafe при прицеле удобен; камера прицела (плечо, 2 м, FOV 55°) устраивает; манекены падают до 0 и возвращаются. Крутилки: `weapons.ron`, `aim.ron`, `camera.ron`, `juice.ron`, `mix.ron`, `strings.ron`.

### G. Готовность
`cargo build`; `cargo clippy -- -D warnings`; `cargo test -p gta_sim` (все старые + D1-D12); `cargo test -p citygen` (не трогаем, но прогнать); `cargo test -p gta_like --bin gta_like` (клиентский гейт `character_model_spawns_under_player` должен остаться зелёным: в `TestArea` манекенов нет); `cargo tree -p gta_sim -e normal -i bevy_render` пуст (rand_chacha рендер не тянет); `python tools/qa/tree_check.py`; `python tools/qa/scenarios/t6.py`, а также `t5.py` (регрессия пикапов/B1). Grep: новых `const` с тюнингом нет (разрешены: `GameLayer`, частота дискретизации 44 100, seed `SpreadRng`).

---

## 4. Risk areas

1. **Камера и голова.** Сфера головы охватывает pivot камеры (pivot 1.55 м, центр головы 1.6, r 0.35). Без маски `World` в `follow_player` камера схлопнется в голову с первого кадра. Шаг E1 обязателен в том же изменении, что B3.
2. **Слой `Character`** меняет только то, с чем взаимодействуют запросы с явной маской. Физика персонаж↔мир и Tnua↔мир сохраняются (`Character/ALL` vs `World/ALL` взаимодействуют). Проверка: `settle()` во всех старых тестах и `jump.rs`/`movement.rs` зелёные.
3. **Числа гейтов D4/D6 зависят от порядка шагов в `tick_loadouts`/`fire_weapons`.** Реализатор обязан пересчитать ожидания по фактическому коду (урок gates: "test numbers are derived"); тесты берут значения из `weapons.ron`, не хардкодят.
4. **Первое обновление при `FixedTimesteps(1)` даёт 0 тиков** (урок TASK-002): считать тики через `run_ticks`, читать `BulletTrace` сразу после тика выстрела (два обновления — и сообщение ротировано).
5. **Случайность разброса**: ожидания D1/D3/D5 доказаны для любой точки конуса; если реализатор меняет `muzzle_offset`, дистанции или разброс в данных, геометрию D1-D5 надо пересчитать (иначе гейт станет "иногда красным").
6. **Кольцо выбора оружия и BRP**: `Loadout.held: Option<Weapon>` сериализуется в BRP как `"None"`/`{"Some": ..}` (или `null`) — парсер t6 толерантный; колесо (`cycle`) BRP-сценарием не проверяется (только юнит D11).
7. **Автоматический огонь при удержании для всех стволов** (включая пистолет с 0.3 с): выбор без GDD-указания; владелец может попросить полуавтомат — это поле данных/флаг, не переделка.
8. **Анимация**: у Kenney Mini Characters нет strafe-клипов, при боковом/обратном движении с прицелом ноги "бегут вперёд". Видно владельцу, не гейт; отдельная задача при недовольстве.
9. **Скриншот трассера** в t6 — окно 60 мс; очередь SMG даёт ~75% времени с трассером на экране, но кадр может попасть в паузу. Жёсткий pass/fail сценария только на числах; QA при пустом кадре повторяет снимок.
10. **Полигон в парке**: деревья парка — визуальные пропы без коллайдеров, могут визуально загородить манекен; физически лучи проходят. Если блок парка окажется < 25 м по стороне на каком-то seed, ряд пикапов выйдет на тротуар (не на дорогу: отступ ≤ 10 м от центроида). Видно владельцу/QA.
11. **Звук через `Decodable`**: API rodio 0.22 (`SampleRate = NonZero<u32>`, `ChannelCount = NonZero<u16>`, `rodio-0.22.2/src/common.rs:5,8`) — сверять с примером `decodable.rs`, не с памятью.
12. **`cargo test --workspace`** не использовать (унификация фич) — только `-p`.

## 5. Open questions (владельцу; у каждого есть рекомендуемый дефолт, план исполним без ответа)

**Q1. Где стоит тир (манекены + пикапы оружия)?**
- A (дефолт): центр центрального парка (`CityLandmarks.park_center`), до 300 м от точки появления. Открытое ровное место, но идти до него.
- B: у точки появления игрока. Требует направления тротуара из генератора (правка `citygen` и golden-хэшей) — дороже.
- C: оба места.

**Q2. Модели оружия.**
- A (дефолт): примитивы (боксы-стволы, боксы пикапов). GDD §9.2 допускает; ноль ассетов.
- B: Kenney Blaster Kit 2.1 (CC0, 40 файлов, проверено по странице пакета) через манифест: игрушечные бластеры в руке и на пикапах. Отдельная небольшая задача после T6.

**Q3. Стрельба при удержании ЛКМ.**
- A (дефолт): все стволы стреляют, пока кнопка зажата, со своим темпом.
- B: пистолет и дробовик полуавтомат (выстрел на каждое нажатие). Одно поле в `weapons.ron`.

**Q4. Скорость при прицеливании.**
- A (дефолт): как без прицела (GDD не задаёт); спринт с прицелом возможен.
- B: при ПКМ скорость ограничена ходьбой/бегом (как в V). Одно поле в `locomotion.ron`.

children: 0 launched / 0 reported.
