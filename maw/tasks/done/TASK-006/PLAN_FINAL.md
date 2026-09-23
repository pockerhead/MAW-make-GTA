# PLAN_FINAL — TASK-006 (GDD T5): здоровье, урон, смерть и возрождение

Источник правды при конфликте: этот файл > PLAN_V2.md > PLAN.md. Все ссылки на API проверены по пиненным исходникам в `~/.cargo/registry/src/*/` (bevy 0.19.1, bevy_state 0.19.1, bevy_time 0.19.1, bevy_ecs 0.19.1, bevy_text 0.19.1, bevy_render 0.19.1, bevy_remote 0.19.1, bevy_transform_interpolation 0.5.0, bevy-tnua 0.32.0); строки указаны ниже там, где на них опирается решение.

## 1. Summary

Слайс T5 добавляет в `gta_sim` компонент `Health` (здоровье + броня, урон сначала съедает броню), регенерацию в стиле V (только ниже 50% и только до 50%, через 5 с без урона), buffered `Message` `DebugDamage` (пишут BRP `world.write_message` и клавиша F5 под фичей `debug`, читает `FixedUpdate`), `GameState::Wasted` с sub-state `WastedPhase { SlowMo, Screen }`: `Time<Virtual>` 0.3× на 1.5 с реального времени, затем экран "ПОТРАЧЕНО" 3 с при 1.0×, затем телепорт той же сущности игрока к больнице с полным здоровьем; таймер фаз идёт в `Update` по `Time<Real>`. Точка у больницы (`HospitalSpawn`) вычисляется новой чистой функцией `citygen::sidewalk_anchor` на кольце тротуара квартала больницы, layout и golden-хэши не меняются. Пикапы аптечки и брони стоят на тротуаре по обе стороны от точки возрождения (домен `combat`), минимальный `WantedLevel` сбрасывается при смерти (домен `wanted`). Хазард повторного входа в `Playing` закрыт переносом одноразовых спавнов (игрок, пикапы, HUD, клиентская сборка мешей города) с `OnEnter(Playing)` на `OnTransition { exited: Loading, entered: Playing }`. Клиент получает HUD-полосы, экран "ПОТРАЧЕНО" поверх обесцвеченного кадра, русский экран загрузки со шрифтом Inter 4.1 (SIL OFL 1.1, ставится через манифест ассетов; схема манифеста расширена парой OFL + GitHub-релиз), визуал пикапов. Все настраиваемые числа лежат в файлах GDD §12: `assets/character/health.ron`, `assets/flow/respawn.ron`, `assets/ui/strings.ron`, `assets/world/render.ron`. Новых крейтов нет, `Cargo.lock` не меняется.

Цена ошибки: молчаливые дефекты (двойной город или игрок после возврата в `Playing`, таймер `Wasted` по виртуальному времени, `Time<Virtual>` застрял на 0.3, неверный порядок брони и здоровья, реген выше 50%, пикап съедается при возрождении) получают headless-гейты. Вид полос, экрана "ПОТРАЧЕНО", читаемость slow-mo и кириллицы проверяет владелец плюс скриншоты BRP.

## 2. Implementation steps

Порядок: A данные → B citygen → C sim → D гейты sim → E манифест и шрифт → F клиент → G гейт презентации и QA → H проверка готовности.

### A. Данные

**A1. `assets/character/health.ron`** (новый, UTF-8 без BOM):
```ron
(
    max_health: 100.0,
    max_armor: 100.0,
    regen_delay: 5.0,
    regen_rate: 5.0,
    regen_cap: 0.5,
    debug_damage: 25.0,
    pickups: (health: 50.0, armor: 50.0, radius: 1.0, respawn: 30.0, spacing: 6.0),
)
```
Значения здоровья, регенерации и +50 пикапов взяты из GDD §3.4. `respawn` (30 с, кулдаун пикапа) и `spacing` (6 м от точки возрождения вдоль тротуара) это стартовые данные, их подбирает владелец. Комментарии в RON не нужны: имена полей говорят сами.

**A2. `assets/flow/respawn.ron`** (новый): `(wasted_time_scale: 0.3, wasted_slowmo: 1.5, wasted_screen: 3.0)`.

**A3. `assets/ui/strings.ron`** (новый, UTF-8 без BOM; Q1 принят: числа HUD и экрана смерти живут здесь, `hud.ron` не создаём):
```ron
(
    font: "third_party/inter/Inter-Regular.ttf",
    title_font: "third_party/inter/InterDisplay-Black.ttf",
    loading: "Генерация города (seed {seed})",
    loading_size: 32.0,
    wasted: "ПОТРАЧЕНО",
    wasted_size: 96.0,
    wasted_color: (0.78, 0.08, 0.08),
    wasted_backdrop: (0.0, 0.0, 0.0, 0.35),
    wasted_saturation: 0.0,
    hud: (margin: 24.0, bar_width: 200.0, bar_height: 12.0, bar_gap: 6.0,
          health_color: (0.80, 0.15, 0.15), armor_color: (0.25, 0.45, 0.90), back_color: (0.0, 0.0, 0.0, 0.55)),
)
```

**A4. `assets/world/render.ron`**: добавить в корневую структуру поле `pickups: (size: 0.5, lift: 0.5, health_color: (0.90, 0.95, 0.90), armor_color: (0.25, 0.45, 0.90)),` (после `props`).

### B. citygen

**B1. `crates/citygen/src/graphs.rs`** — добавить две функции (ничего существующего не менять):

```rust
/// Point on the sidewalk centre line of the building's block facing `building`, and the unit sidewalk
/// direction there. The point is at least `margin` from both ends of its sidewalk side, so
/// `point ± dir * margin` stays on the same side. `None` for a bad index or no side long enough.
pub fn sidewalk_anchor(layout: &CityLayout, params: &CityParams, building: usize, margin: f32) -> Option<(Vec2, Vec2)>
```
Алгоритм (без паник, индексы через `.get()?`):
1. `b = layout.buildings.get(building)?`; `lot = layout.lots.get(b.lot as usize)?`; `block = layout.blocks.get(lot.block as usize)?`.
2. `offsets[k] = walk_offset(params, class_k)`, где `class_k = layout.roads.edges.get(block.sides[k] as usize)?.class`.
3. `ring = geom::inset(&road_polygon(&layout.roads, block), &offsets)?`. Это то же кольцо, что строит `sidewalks()` (строка 41): центральная линия тротуара. Сторона кольца `k` = `ring[k] → ring[(k+1) % n]` лежит на прямой стороны дороги `k`, сдвинутой внутрь на `walk_offset` (`geom::inset`, строки 36-49: вершина `i` это пересечение прямых `i-1` и `i`). Знак сдвига внутрь уже закреплён в `inset` и тестом `inward_normals_of_unit_square`.
4. Кандидаты: для каждого `k`, где `class_k != RoadClass::Alley`, `anchor_on_segment(ring[k], ring[(k+1) % n], b.center, margin)`.
5. Вернуть кандидата с минимальным `point.distance_squared(b.center)`; `Iterator::min_by` с `total_cmp` при равенстве возвращает первый, то есть меньший `k` (детерминированно).

```rust
/// Closest point to `target` on segment `a → c`, kept `margin` away from both ends; `None` when the
/// segment is shorter than `2 * margin`.
fn anchor_on_segment(a: Vec2, c: Vec2, target: Vec2, margin: f32) -> Option<(Vec2, Vec2)>
```
`len = a.distance(c)`; если `!(len >= 2.0 * margin) || len <= 0.0` → `None`; `d = (c - a) / len`; `t = (target - a).dot(d).clamp(margin, len - margin)`; `Some((a + d * t, d))`.

Почему кольцо, а не "середина стороны + perp * walk_offset" (PLAN.md B1) и не "проекция на ось дороги + perp" (PLAN_V2 §3): любая точка кольца по построению лежит на центральной линии тротуара этой стороны и внутри всех полуплоскостей квартала, поэтому ни якорь, ни оба пикапа (`point ± d * margin`, они тоже на отрезке кольца) не попадают ни на проезжую часть, ни в перекрёсток, ни в лот (лоты внутри `block.inner`, сдвинутого на `half + sidewalk` > `walk_offset`). Запас `margin` приходит из данных (`pickups.spacing`), новой константы нет.

Разобранные примеры для `anchor_on_segment` (они же unit-тест, числа выведены по формуле выше):
| # | a | c | target | margin | len, d | t сырой → после clamp | результат |
|---|---|---|---|---|---|---|---|
| 1 | (−50, 0) | (50, 0) | (35, 10) | 6 | 100, (1, 0) | 85 → 85 | ((35, 0), (1, 0)) |
| 2 | (50, 0) | (−50, 0) | (35, 10) | 6 | 100, (−1, 0) | 15 → 15 | ((35, 0), (−1, 0)) |
| 3 | (0, −50) | (0, 50) | (−10, 49) | 6 | 100, (0, 1) | 99 → 94 | ((0, 44), (0, 1)) |
| 4 | (−50, 0) | (50, 0) | (−60, 5) | 6 | 100, (1, 0) | −10 → 6 | ((−44, 0), (1, 0)) |
| 5 | (0, 0) | (10, 0) | (5, 1) | 6 | 10 < 12 | — | `None` |

Пример 1 исключает старую середину стороны `(0, 0)`; 2 проверяет обратное направление обхода; 3 и 4 проверяют clamp у обоих концов; 5 короткую сторону.

Добавить в конец `graphs.rs` `#[cfg(test)] mod tests` с тестом `anchor_on_segment_examples` по таблице (допуск `1e-4`).

**B2. `crates/citygen/src/lib.rs`**: `pub use graphs::sidewalk_anchor;`.

**B3. `crates/citygen/tests/properties.rs`**: тест `hospital_anchor_on_sidewalk` для всех `layouts()` (seeds 1, 2, 42 и 0..32):
- `margin` читается из `assets/character/health.ron` локальной тестовой структурой `#[derive(Deserialize)] struct HealthFile { pickups: PickupsFile } struct PickupsFile { spacing: f32 }` (без `deny_unknown_fields`, лишние поля игнорируются); ошибка чтения или разбора → `panic!("GATE BROKEN: ...")`. `serde` уже обычная зависимость `citygen`, `ron` уже в `[dev-dependencies]`, `Cargo.toml` не меняется. Один источник числа, тест не дублирует 6.0.
- Ровно одно здание `BuildingKind::Hospital`; `(point, along) = sidewalk_anchor(layout, &params, idx, margin).expect(...)`.
- `(along.length() - 1.0).abs() < 1e-4`.
- Для каждой из точек `point`, `point + along * margin`, `point - along * margin`: (а) на тротуаре: есть не-alley ребро с `dist_point_segment` в `[half, half + sidewalk]` (та же проверка, что `player_spawn_on_sidewalk`, строки 268-293); (б) для **всех** рёбер (включая alley) `dist >= half_carriageway(class) - 1e-3`: не на проезжей части; (в) не внутри ни одного `lot.polygon` (`contains_convex(.., 0.0)`).
- Якорь перед больницей: `(b.center - point).dot(along).abs() <= margin + 1e-3`. Сырой `t` отличается от зажатого не больше чем на `margin`, если проекция центра больницы попала на отрезок кольца; середина стороны (старая формула) даёт здесь до половины длины стороны, 30-80 м. Если утверждение падает на каком-то seed, это находка (больница в углу квартала), её фиксируют в отчёте и эскалируют, порог молча не ослабляют.

Flip-RED (записать в stage summary): (1) в `anchor_on_segment` заменить `t` на `len / 2.0` → RED и `anchor_on_segment_examples` (пример 1 даёт `(0, 0)`), и `hospital_anchor_on_sidewalk` (проверка "перед больницей"); (2) в `sidewalk_anchor` взять `road_polygon` вместо кольца `inset` → RED (точка на оси дороги, проверки (а) и (б)). Откатить, GREEN.

`CityLayout`, `layout_hash`, `golden_hashes.txt` не трогаем: `cargo test -p citygen` golden остаётся GREEN.

### C. Sim (`crates/gta_sim`)

**C1. `src/character/health.rs`** (новый, ~140 строк):
- `pub const HEALTH_CONFIG: &str = "character/health.ron";` (путь, не tuning).
- `#[derive(Resource, Deserialize, Clone, Debug)] #[serde(deny_unknown_fields)] pub struct HealthConfig { pub max_health: f32, pub max_armor: f32, pub regen_delay: f32, pub regen_rate: f32, pub regen_cap: f32, pub debug_damage: f32, pub pickups: PickupConfig }` и `#[derive(Deserialize, Clone, Copy, Debug)] #[serde(deny_unknown_fields)] pub struct PickupConfig { pub health: f32, pub armor: f32, pub radius: f32, pub respawn: f32, pub spacing: f32 }`.
- `HealthConfig::validate(&self) -> Result<(), String>`: все поля конечны; `max_health > 0`, `max_armor > 0`, `0 < regen_cap <= 1`, `regen_delay >= 0`, `regen_rate >= 0`, `debug_damage > 0`; `pickups.health > 0`, `pickups.armor > 0`, `pickups.radius > 0`, `pickups.respawn >= 0`, `pickups.spacing > pickups.radius` (иначе пикап окажется в радиусе подбора точки возрождения и съестся на первом тике `Playing`). Текст ошибки содержит имя поля (`regen_cap`, `pickups.spacing` и т.д.).
- `#[derive(Component, Reflect, Clone, Copy, Debug, PartialEq)] #[reflect(Component)] pub struct Health { pub current: f32, pub armor: f32, pub since_damage: f32 }`; `impl Health { pub fn full(cfg: &HealthConfig) -> Self { Self { current: cfg.max_health, armor: 0.0, since_damage: 0.0 } } }` (GDD §3.4: броня 0-100, старт без брони).
- `#[derive(Component, Reflect, Default)] #[reflect(Component)] pub struct Dead;` (редкое событие, archetype move допустим).
- `pub fn apply_damage(current: f32, armor: f32, amount: f32) -> (f32, f32)`: `absorbed = armor.min(amount)`; `((current - (amount - absorbed)).max(0.0), armor - absorbed)`.
- `pub fn regenerate(current: f32, since_damage: f32, dt: f32, cfg: &HealthConfig) -> f32`: `cap = cfg.max_health * cfg.regen_cap`; если `current <= 0.0 || current >= cap || since_damage < cfg.regen_delay` → `current`; иначе `(current + cfg.regen_rate * dt).min(cap)`.
- `#[derive(SystemSet, Debug, Clone, Copy, PartialEq, Eq, Hash)] pub enum HealthSystems { Damage, Regen, Pickup, Death }` — порядок шагов тика здоровья, чтобы `flow` не зависел от `combat` напрямую.
- `#[cfg(test)]` таблицы (`cfg` с полями A1): `apply_damage`: `(100, 50, 30) → (100, 20)`, `(100, 20, 30) → (90, 0)`, `(100, 0, 30) → (70, 0)`, `(10, 0, 30) → (0, 0)`, `(100, 50, 150) → (0, 0)`; `regenerate`: `(30, 4.99, 1/64) → 30`, `(30, 5.0, 1/64) → 30.078125`, `(49.99, 5.0, 1/64) → 50.0`, `(60, 10, 1/64) → 60`, `(0, 10, 1/64) → 0`.

**C2. `src/character/mod.rs`**:
- `mod health; pub use health::{Dead, Health, HealthConfig, HealthSystems, HEALTH_CONFIG, PickupConfig, apply_damage, regenerate};`
- В `CharacterPlugin::build`: `.register_type::<Health>().register_type::<Dead>()` и `.configure_sets(FixedUpdate, (HealthSystems::Damage, HealthSystems::Regen, HealthSystems::Pickup, HealthSystems::Death).chain())`.
- В `drive_characters`: в query добавить `Has<Dead>` (`bevy_ecs-0.19.1/src/query/fetch.rs:3263`), в начале тела цикла:
```rust
if dead {
    intent.jump_requested = false;
    buffer.remaining = 0.0;
    controller.initiate_action_feeding();
    controller.basis = TnuaBuiltinWalk { desired_motion: Vec3::ZERO, desired_forward: None };
    continue;
}
```
`basis` у Tnua постоянное поле (`bevy-tnua-0.32.0/src/controller.rs:150`), без явной записи нуля тело продолжит идти. Сброс `jump_requested` и буфера нужен, иначе прыжок, нажатый во время смерти, сработает сразу после возрождения (в PLAN.md этого не было). Клиентский ввод продолжает писать `MoveIntent`, тело мёртвого стоит.

**C3. `src/flow/mod.rs`** (≤ 70 строк, логика в `wasted.rs`):
- `mod wasted; pub use wasted::{RESPAWN_CONFIG, RespawnConfig, WastedClock};`
- `#[derive(States, Default, Clone, PartialEq, Eq, Hash, Debug, Reflect)] pub enum GameState { #[default] Loading, Playing, Wasted }`.
- `#[derive(SubStates, Default, Clone, PartialEq, Eq, Hash, Debug, Reflect)] #[source(GameState = GameState::Wasted)] pub enum WastedPhase { #[default] SlowMo, Screen }`.
- `#[derive(SystemSet, Debug, Clone, PartialEq, Eq, Hash)] pub struct PlayingSystems;` и `#[derive(SystemSet, Debug, Clone, PartialEq, Eq, Hash)] pub struct WastedSystems;`.
- `FlowPlugin::build`: `init_state::<GameState>()`, `add_sub_state::<WastedPhase>()`, `register_type_state::<GameState>()`, `register_type_state::<WastedPhase>()` (`bevy_state-0.19.1/src/app.rs:218`: регистрирует `State<S>` с `#[reflect(Resource)]`, `resources.rs:54-56`), `configure_sets(FixedUpdate, PlayingSystems.run_if(in_state(GameState::Playing)))`, `configure_sets(Update, WastedSystems.run_if(in_state(GameState::Wasted)))`, системы из C4.

**C4. `src/flow/wasted.rs`** (новый, ~120 строк):
- `pub const RESPAWN_CONFIG: &str = "flow/respawn.ron";` `#[derive(Resource, Deserialize, Clone, Debug)] #[serde(deny_unknown_fields)] pub struct RespawnConfig { pub wasted_time_scale: f32, pub wasted_slowmo: f32, pub wasted_screen: f32 }`; `validate()`: всё конечно, `0 < wasted_time_scale <= 1`, `wasted_slowmo >= 0`, `wasted_screen >= 0`; ошибка называет поле.
- `#[derive(Resource, Default)] pub struct WastedClock(pub f32);` (`init_resource` в плагине).
- `detect_player_death` (`FixedUpdate`, `.in_set(PlayingSystems).in_set(HealthSystems::Death)`): `Query<(Entity, &Health), (With<Player>, Without<Dead>)>`; если `current <= 0.0` → `commands.entity(e).insert(Dead)`, `next.set(GameState::Wasted)`.
- `enter_wasted` (`OnEnter(GameState::Wasted)`): `ResMut<Time<Virtual>>::set_relative_speed(cfg.wasted_time_scale)` (`bevy_time-0.19.1/src/virt.rs:188`), `clock.0 = 0.0`.
- `advance_wasted` (`Update`, `.in_set(WastedSystems)`): `clock.0 += Res<Time<Real>>::delta_secs()`; если фаза `SlowMo` и `clock >= wasted_slowmo` → `NextState<WastedPhase>::set(Screen)`; если `clock >= wasted_slowmo + wasted_screen` → `NextState<GameState>::set(Playing)`. Одна строка комментария: `Update` + `Time<Real>`, потому что под slow-mo `FixedUpdate` крутится реже и растянул бы таймер (GDD §3.4).
- `restore_time_scale` на **`OnExit(WastedPhase::SlowMo)`**: `set_relative_speed(1.0)`. Это расписание срабатывает и при `SlowMo → Screen`, и когда `GameState` уходит из `Wasted` прямо из `SlowMo`: при выходе родителя sub-state вычисляется в `None`, пишется `StateTransitionEvent { exited: Some(SlowMo), entered: None }`, и `run_exit` запускает `OnExit(SlowMo)` (`bevy_state-0.19.1/src/state/transitions.rs:190-205, 262-275`). Одна система закрывает оба пути: экранная фаза идёт при 1.0× (GDD §3.4), и выход из `Wasted` всегда оставляет 1.0×.
- `respawn_player` (`OnExit(GameState::Wasted)`): `Query<(&mut Position, &mut Transform, &mut LinearVelocity, &mut Health, &CharacterBody, Entity), With<Player>>`, `Res<HospitalSpawn>`, `Res<HealthConfig>`, `Commands`: `at = spawn.point + Vec3::Y * body.float_height`; `Position.0 = at`; `Transform.translation = at`; `LinearVelocity.0 = Vec3::ZERO`; `*health = Health::full(&cfg)`; `commands.entity(e).try_remove::<Dead>()`. Запись `Transform` вне фиксированного цикла сбрасывает сглаживание avian (`bevy_transform_interpolation-0.5.0/src/lib.rs:485-496`, `reset_easing_states_on_transform_change`), шлейфа не будет.
- Сбрасывать `WantedLevel` здесь не нужно, это делает `wanted` (C8).

**C5. `src/world/mod.rs` + `src/world/city.rs`**:
- В `mod.rs`: `#[derive(Resource, Reflect, Clone, Copy, Debug)] #[reflect(Resource)] pub struct HospitalSpawn { pub point: Vec3, pub along: Vec3 }`. В `WorldPlugin::build` рядом с `PlayerSpawn(Vec3::ZERO)`: `.insert_resource(HospitalSpawn { point: Vec3::ZERO, along: Vec3::X }).register_type::<HospitalSpawn>()`. Для TestArea это окончательное значение: больница = точка спавна `(0, 0, 0)`, пикапы встанут на `(±6, 0, 0)`; эти точки свободны от блоков `test_area.rs:6-34`.
- В `city.rs` `apply_city_generation`: новый параметр `health: Res<HealthConfig>` (система регистрируется только в `WorldPlugin`; `compose_sim` вставляет `HealthConfig` до плагинов, C9). После `spawn_buildings(...)`: `match hospital_spawn(&layout, &params.0, curb, health.pickups.spacing)` → `Ok(s)` → `commands.insert_resource(s)`; `Err(err)` → существующий путь `error!("city generation failed: {err}"); exit.write(AppExit::error()); return;` (до `next.set(Playing)`).
- `fn hospital_spawn(layout: &CityLayout, params: &CityParams, curb: f32, margin: f32) -> Result<HospitalSpawn, String>`: индекс единственного `BuildingKind::Hospital` (нет → `Err("no hospital")`), `citygen::sidewalk_anchor(layout, params, idx, margin).ok_or_else(|| format!("hospital {idx}: no sidewalk side of length >= {}", 2.0 * margin))?`; `point = Vec3::new(p.x, curb, p.y)`, `along = Vec3::new(d.x, 0.0, d.y)`.

**C6. `src/player/mod.rs`**:
- `spawn_player` → `.add_systems(OnTransition { exited: GameState::Loading, entered: GameState::Playing }, spawn_player)`; в бандл добавить `Health::full(&health)` (новый `health: Res<HealthConfig>`). `grep spawn_player` по `crates/` и `src/` находит только регистрацию в плагине; все харнессы идут через `compose_sim`.
- `#[derive(Message, Reflect, Clone, Copy, Debug)] #[reflect(Message)] pub struct DebugDamage { pub amount: f32 }` (урон игроку; type path `gta_sim::player::DebugDamage`; `ReflectMessage` = `bevy_ecs-0.19.1/src/reflect/message.rs:16`; путь BRP `bevy_remote-0.19.1/src/builtin_methods.rs:1519-1552`). В плагине: `.add_message::<DebugDamage>().register_type::<DebugDamage>()`.
- `apply_debug_damage` (`FixedUpdate`, `.in_set(PlayingSystems).in_set(HealthSystems::Damage)`): читает все сообщения; `!amount.is_finite() || amount <= 0.0` → `warn!` и пропуск; иначе для `Query<&mut Health, (With<Player>, Without<Dead>)>`: `(current, armor) = apply_damage(...)`, `since_damage = 0.0`.
- `regenerate_player` (`FixedUpdate`, `.in_set(PlayingSystems).in_set(HealthSystems::Regen)`): `dt = Res<Time<Fixed>>::delta_secs()`; `since_damage += dt`, затем `current = regenerate(current, since_damage, dt, &cfg)`. Порядок "сначала `+=`, потом проверка" фиксирован, от него выведены числа D2.2.
- Тип `PlayingSystems` берётся из `crate::flow`, `HealthSystems` из `crate::character`.

**C7. `src/combat/mod.rs` + `src/combat/pickups.rs`** (новые, ~100 строк):
- `#[derive(Reflect, Clone, Copy, Debug, PartialEq, Eq)] pub enum PickupKind { Health, Armor }`; `#[derive(Component, Reflect, Debug)] #[reflect(Component)] pub struct Pickup { pub kind: PickupKind, pub cooldown: f32 }`; `impl Pickup { pub fn available(&self) -> bool { self.cooldown <= 0.0 } }`.
- `spawn_pickups` на `OnTransition { exited: Loading, entered: Playing }`: `Health` в `point + along * spacing`, `Armor` в `point - along * spacing` (`HospitalSpawn`, `HealthConfig.pickups.spacing`), `Transform::from_translation`, `Name::new("Pickup Health" / "Pickup Armor")`. Коллайдера нет.
- `collect_pickups` (`FixedUpdate`, `.in_set(PlayingSystems).in_set(HealthSystems::Pickup)`): для каждого пикапа `cooldown = (cooldown - dt).max(0.0)`; если `available()`: ноги игрока `Position.0 - Vec3::Y * body.float_height`, `distance(feet, pickup.translation) <= radius` → `Health`: если `current < max_health` → `current = (current + pickups.health).min(max_health)`, `cooldown = respawn`; `Armor`: если `armor < max_armor` → `armor = (armor + pickups.armor).min(max_armor)`, `cooldown = respawn`. При полном запасе пикап не тратится. Игрок `Without<Dead>`.
- `CombatPlugin`: `register_type::<Pickup>()`, `register_type::<PickupKind>()`, системы выше.

**C8. `src/wanted/mod.rs`** (новый, ~25 строк): `#[derive(Resource, Reflect, Default, Debug)] #[reflect(Resource)] pub struct WantedLevel { pub stars: u8 }` (`///`: T10 владеет heat и звёздами); `WantedPlugin`: `init_resource::<WantedLevel>()`, `register_type::<WantedLevel>()`, `add_systems(OnEnter(GameState::Wasted), reset_wanted)` (`stars = 0`).

**C9. `src/lib.rs`**: `pub mod combat; pub mod wanted;`. В `compose_sim` до `add_plugins`: `load_config::<HealthConfig>(&root, HEALTH_CONFIG)?` + `validate()` с `ConfigError { path: root.path(HEALTH_CONFIG), message }`, то же для `RespawnConfig`/`RESPAWN_CONFIG` (образец `LocomotionConfig`, строки 24-28); `insert_resource` обоих; в кортеж плагинов добавить `CombatPlugin, WantedPlugin`. Другой точки композиции не появляется.

### D. Headless-гейты sim (`cargo test -p gta_sim`)

Все тесты идут через `composed_app` / `headless_app` / `city_app` (production `compose_sim` + `MinimalPlugins`, `finish()/cleanup()`, `TimeUpdateStrategy::FixedTimesteps(1)`: каждый `update()` после первого даёт ровно 1 фиксированный тик и `Time<Real>` += 1/64 с, `bevy_time-0.19.1/src/lib.rs:102-103`).

**Важно:** `common::run_ticks` делает не больше `count + 4` update и паникует "fixed loop stalled". Под slow-mo 0.3 фиксированный тик приходит раз в ~3.3 update, поэтому **внутри `Wasted` двигать время только `app.update()` по счёту, никогда `run_ticks`**. После возврата в `Playing` `run_ticks` снова корректен.

**D1. `tests/common/mod.rs`**: хелперы `health(app) -> Health`, `set_health(app, impl FnOnce(&mut Health))`, `write_damage(app, amount)` (`app.world_mut().write_message(DebugDamage { amount })`), `game_state(app) -> GameState`, `wasted_phase(app) -> Option<WastedPhase>` (`world.get_resource::<State<WastedPhase>>()`), `count<F: QueryFilter>(app) -> usize`.

**D2. `tests/health.rs`** (TestArea, `headless_app()` + `settle`). Класс каждого гейта указан.
1. `armor_absorbs_then_health` (корректность): `set_health(armor = 50)`; `write_damage(30)`, `run_ticks(1)` → `(100, 20)`; ещё 30 → `(90, 0)`; ещё 30 → `(60, 0)`. Flip-RED: в `apply_damage` вычитать сначала из здоровья → RED.
2. `regen_waits_then_stops_at_cap` (корректность; числа из C6): `write_damage(70)`, `run_ticks(1)` → 30, `since = 1/64` (тик урона обнуляет, реген того же тика прибавляет dt); `run_ticks(318)` → ровно 30 (`since = 319/64 < 5`); `run_ticks(1)` → `30.078125 ± 1e-4` (`since = 320/64 = 5.0`, суммы 2⁻⁶ точны в f32, `+5/64`); затем 640 тиков по одному с записью максимума → в конце ровно `50.0`, максимум `<= 50.0` (нужно 255 тиков, запас есть). Второй случай в том же тесте после `set_health(current = 100, since = 0)`: урон 40 → 60, 640 тиков → 60. Flip-RED: (а) убрать `.min(cap)` → RED (выше 50); (б) проверять `since` до `+=` → RED на границе 318/319 тиков.
3. `pickups_heal_armor_and_respawn` (корректность): урон 70 → 30; `place_player(HospitalSpawn.point + along * spacing + Y * float_height)` (= `(6, float_height, 0)` в TestArea), `run_ticks(2)` → 80, у аптечки `cooldown > 0`; `place_player` в `(0, fh, 0)`, `run_ticks(2)`, снова на аптечку, `run_ticks(2)` → по-прежнему 80 (неактивна); `place_player` на броню (`point - along * spacing`), `run_ticks(2)` → `armor == 50`; вернуться в `(0, fh, 0)`, `run_ticks(respawn * 64 + 2)` → аптечка `available()`; `set_health(current = max_health)`, встать на аптечку, `run_ticks(2)` → `cooldown` остался `0` (при полном здоровье не тратится). Flip-RED: убрать проверку `available()` в `collect_pickups` → RED (второе касание даёт 100).
4. `respawn_point_is_clear_of_pickups` (корректность, защита от контрпримера "пикап в радиусе возрождения"): в TestArea `HospitalSpawn.point.distance(pickup.translation) > radius` для обоих пикапов (для города то же проверяет D3.1). Механизм, который это держит, это `validate()` (`spacing > radius`), его flip-RED в D4 `pickup_spacing_must_exceed_radius`; этот тест сам по себе liveness данных.
5. `debug_damage_through_reflection` (liveness пути BRP): `AppTypeRegistry` → `ReflectMessage` для `DebugDamage`, `DynamicStruct` с полем `amount = 20.0` → `write_message(world, &dynamic, &registry)`; `run_ticks(1)` → `current == 80`. Flip-RED: убрать `#[reflect(Message)]` → RED (нет type data).
6. `dead_player_cannot_walk` (корректность): вставить `Dead` игроку напрямую (`Health` остаётся 100, поэтому `Wasted` не наступает и `run_ticks` работает), `set_intent(axis = Vec2::Y, jump_requested = true)`, `run_ticks(64)` → горизонтальный сдвиг `< 0.1 м`, `MoveIntent.jump_requested == false`, высота в пределах `±0.05` от стоячей. Flip-RED: убрать ветку `if dead` → RED (идёт и прыгает).

**D3. `tests/respawn.rs`** (город, `city_app(1)` + `settle`).
1. `death_wasted_respawn_at_hospital` (корректность). Подготовка: `e = player(app)`; вставить тестовый `#[derive(Component)] struct Loadout(u32)` = `Loadout(7)` (прокси "оружие сохранено": T6 кладёт оружие компонентами на игрока); `WantedLevel.stars = 3`; `write_damage(1000)`; `app.update()` до `game_state == Wasted` (не больше 3 update; проба: 2).
   Отсчёт: `k = 0` на update, после которого впервые видно `Wasted` (назовём его U1: в нём уже прошёл `OnEnter(Wasted)` и первый `advance_wasted`, `clock = 1/64`). Далее каждый `app.update()` это `k += 1`, после каждого читаем состояние и считаем приросты `Time<Fixed>::elapsed() / timestep`.
   Проверки в `SlowMo` (сразу при `k = 0`): `Time<Virtual>::relative_speed() == wasted_time_scale`, `Dead` есть, `WantedLevel.stars == 0`, `wasted_phase == Some(SlowMo)`.
   Ожидания (выведены: в U1 часы уже `1/64`, `set(Screen)` при `clock >= 1.5`, то есть в update с `clock = 96/64`, это `k = 95`; переход применяется в `StateTransition` следующего update → первое наблюдение `Screen` при `k = 96`; аналогично `Playing` при `k = 288`; проба `scratch/probe_wasted_timing.txt` дала 96 и 288):
   - `Screen` впервые при `k ∈ [95, 97]` (ожидается 96);
   - `Playing` впервые при `k ∈ [287, 289]` (ожидается 288); по виртуальному времени было бы 320 + 192 = 512 → RED;
   - фиксированных тиков, пока фаза `SlowMo`: `∈ [27, 31]` (вывод: U1 добавляет полный dt, так как скорость меняется после `First`, → 1 тик; далее ~95 update по 0.3·dt → 28.5; без slow-mo было бы 96);
   - на первом update, где видно `Screen`, и далее: `relative_speed() == 1.0`; фиксированных тиков в фазе `Screen`: `∈ [189, 193]` (192 update по 1.0, минус/плюс один на границе; без восстановления было бы ~58).
   Проверки после `Playing`: `relative_speed() == 1.0`; `player(app) == e` (та же сущность, ровно один `Player`); `Loadout(7)` на месте; `Health == Health::full` (`(100, 0, 0)`); `Dead` снят; горизонтальное расстояние `Position` до `HospitalSpawn.point` `< 0.05 м`; затем `run_ticks(64)`: `|y - (HospitalSpawn.point.y + float_height)| < 0.05` и горизонтальный дрейф `< 0.1 м` (стоит на тротуаре, Tnua не подбросил); оба пикапа дальше `radius` от точки (D2.4) и `cooldown == 0` (не съедены при возрождении).
   Flip-RED: (а) `advance_wasted` на `Res<Time>` (виртуальное) → `Playing` при k≈512 → RED; (б) удалить `restore_time_scale` → RED (скорость в `Screen` 0.3, тиков Screen ~58, после выхода 0.3); (в) заменить телепорт на despawn + spawn нового игрока → RED (`Loadout` пропал / другая `Entity`); (г) вернуть `enter_wasted` без `set_relative_speed` → RED (тиков SlowMo 96).
2. `wasted_abort_from_slowmo_restores_time` (корректность второго пути выхода): смерть → `Wasted` (`SlowMo`, скорость 0.3) → `NextState<GameState>::set(Playing)` вручную → 2 update → `relative_speed() == 1.0`, `Playing`, игрок у больницы. Flip-RED: удалить `restore_time_scale` → RED. Это гейт критерия "slow-mo восстанавливает `Time<Virtual>` 1.0 при выходе из `Wasted`" для пути, где `Screen` не наступил.
3. `respawn_keeps_world_one_shot` (корректность хазарда, sim-сторона): до смерти посчитать через `count::<With<..>>` (не через `player()`, он паникует на двух): `Player`, `CityBuilding`, `CityBlock`, `CityGround`, `CityEdgeWall`, `Pickup`; полный цикл смерть → `Playing` + `run_ticks(64)`; все числа равны, `Player == 1`, `Pickup == 2`. Flip-RED: вернуть `spawn_player` (и отдельно `spawn_pickups`) на `OnEnter(Playing)` → `Player == 2` (`Pickup == 4`) → RED.

**D4. `tests/config.rs`**: `shipped_health_config_loads` и `shipped_respawn_config_loads` (load + validate); `health_regen_cap_is_validated` по образцу `locomotion_thresholds_are_validated` (временная копия, `regen_cap: 0.5,` → `regen_cap: 1.5,`, ошибка содержит `regen_cap`; `GATE BROKEN`, если в файле нет исходной строки); `pickup_spacing_must_exceed_radius` (та же схема, `spacing: 6.0)` → `spacing: 0.5)`, ошибка содержит `spacing`). Flip-RED: убрать соответствующее условие из `validate()` → RED.

### E. Манифест и шрифт

Хэши проверены на `scratch/Inter-4.1.zip` при ревью: архив `9883fdd4a49d4fb66bd8177ba6625ef9a64aa45899767dde3d36aa425756b11e`, `extras/ttf/Inter-Regular.ttf` `40d692fce188e4471e2b3cba937be967878f631ad3ebbbdcd587687c7ebe0c82`, `extras/ttf/InterDisplay-Black.ttf` `25460b0d5b3afd9764d63cf838f94145af624f8d4c9d20e0f74116be42878b32`, `LICENSE.txt` `262481e844521b326f5ecd053e59b98c8b2da78c8ee1bdbb6e8174305e54935a`; `LICENSE.txt` начинается с "This Font Software is licensed under the SIL Open Font License, Version 1.1".

**E1. `assets/third_party/manifest.ron`**: добавить пак:
```ron
(
    name: "inter",
    version: "4.1",
    page: "https://github.com/rsms/inter",
    url: "https://github.com/rsms/inter/releases/download/v4.1/Inter-4.1.zip",
    archive_sha256: "9883fdd4a49d4fb66bd8177ba6625ef9a64aa45899767dde3d36aa425756b11e",
    license: OFL,
    license_file: "LICENSE.txt",
    files: [
        (archive: "extras/ttf/Inter-Regular.ttf", path: "Inter-Regular.ttf", sha256: "40d692fce188e4471e2b3cba937be967878f631ad3ebbbdcd587687c7ebe0c82"),
        (archive: "extras/ttf/InterDisplay-Black.ttf", path: "InterDisplay-Black.ttf", sha256: "25460b0d5b3afd9764d63cf838f94145af624f8d4c9d20e0f74116be42878b32"),
        (archive: "LICENSE.txt", path: "LICENSE.txt", sha256: "262481e844521b326f5ecd053e59b98c8b2da78c8ee1bdbb6e8174305e54935a"),
    ],
),
```

**E2. `crates/gta_sim/src/config/manifest.rs`**:
- `pub enum AssetLicense { CC0, OFL }`.
- В `AssetPack::validate` заменить блок page/url (строки 85-98) на вызов `self.check_source()?`:
  - `AssetLicense::CC0` → текущие правила Kenney без изменений (`page == https://kenney.nl/assets/{name}`, `url` с префиксом `https://kenney.nl/media/pages/assets/{name}/` и `.zip`), тексты ошибок те же.
  - `AssetLicense::OFL` → GitHub-релиз, закреплённый версией: `page` = `https://github.com/{owner}/{repo}`, ровно два непустых сегмента из `[A-Za-z0-9._-]`; `url` начинается с `{page}/releases/download/v{version}/` и кончается `.zip`. Ошибки: `"pack {name}: license OFL requires page https://github.com/<owner>/<repo>, got {page:?}"` и `"pack {name}: url {url:?} must start with {prefix:?} and end with .zip"`.
  - Пара лицензия ↔ источник жёсткая: CC0 только с Kenney, OFL только с GitHub-релизом. Общую схему источников не вводить.
- `impl AssetLicense { pub fn markers(self) -> &'static [&'static [u8]] }`: CC0 → `[b"Creative Commons Zero", b"CC0"]` (как сейчас), OFL → `[b"SIL Open Font License"]`.

**E3. `tools/fetch_assets.py`**: зеркало E2 в `check_schema` (строки 190-198): ветка по `pack["license"]`: `"CC0"` → текущие Kenney-проверки; `"OFL"` → регулярка `https://github\.com/[A-Za-z0-9._-]+/[A-Za-z0-9._-]+` полным совпадением для `page`, префикс `f"{page}/releases/download/v{version}/"` и `.zip` для `url`; иначе `ManifestError(f"pack {name}: license {license!r} is not CC0 or OFL")`. Докстринг модуля: упомянуть GitHub-релизы для OFL. Остальной код (кэш по basename URL, распаковка, `--check`) не менять: basename `Inter-4.1.zip` совпадает с файлом в `scratch/`.

**E4. Фикстуры `crates/gta_sim/tests/fixtures/manifest/`** (новые): `valid_github_ofl.ron` (пак `inter` из E1, один файл + `LICENSE.txt`); `bad_github_url.ron` (OFL, `url` другого репозитория → ошибка содержит `url`); `bad_license_source.ron` (Kenney page/url с `license: OFL` → ошибка содержит `page`); `bad_ofl_version.ron` (OFL, `url` с `v4.0` при `version: "4.1"` → содержит `url`). `bad_license.ron` (MIT) остаётся ошибкой парсера.

**E5. `crates/gta_sim/tests/asset_manifest.rs`**:
- `manifest_fixtures_are_judged`: `valid_github_ofl.ron` в список valid; три новые bad-фикстуры в список семантических с ключевыми словами из E4.
- `shipped_manifest_is_valid`: множество имён + `"inter"`; фильтр 4-файловых паков исключает `mini-characters` **и** `inter`; для `inter`: `files.len() == 3`, `license == AssetLicense::OFL`, `rig.is_none()`.
- `local_assets_match_manifest` (строки 186-195): иглы из `pack.license.markers()` вместо жёсткого CC0-списка.
- Flip-RED: временно вернуть Kenney-only проверку в `validate` → `valid_github_ofl` RED; временно разрешить OFL с Kenney-источником → `bad_license_source` RED.
- Паритет Python: `python tools/fetch_assets.py --validate-only <fixture>` для каждой новой фикстуры даёт тот же вердикт (valid / ошибка); записать вывод в stage summary.

**E6. Установка**: `python tools/fetch_assets.py --cache maw/tasks/in_progress/TASK-006/scratch` (архив берётся из кэша, сеть не нужна), затем `python tools/fetch_assets.py --check`. `git add -A --dry-run` и `git check-ignore -v assets/third_party/inter/Inter-Regular.ttf`: TTF и `LICENSE.txt` пака игнорируются правилом `.gitignore:64` (`/assets/third_party/*`), `manifest.ron` остаётся отслеживаемым.

### F. Клиент (`src/`)

**F1. `src/visuals/city.rs:24`**: `OnEnter(GameState::Playing)` → `OnTransition { exited: GameState::Loading, entered: GameState::Playing }`. Одна строка.

**F2. `src/menu/config.rs`** (новый): `pub const UI_CONFIG: &str = "ui/strings.ron";` `#[derive(Resource, Deserialize, Clone, Debug)] #[serde(deny_unknown_fields)] pub struct UiConfig { font, title_font, loading, loading_size, wasted, wasted_size, wasted_color: (f32, f32, f32), wasted_backdrop: (f32, f32, f32, f32), wasted_saturation, hud: HudLayout }` и `HudLayout { margin, bar_width, bar_height, bar_gap, health_color, armor_color, back_color }` (`deny_unknown_fields`). `validate()`: строки непусты, `loading` содержит `{seed}`, размеры `> 0`, компоненты цветов в `[0, 1]`, `wasted_saturation >= 0`, всё конечно. `pub fn font_paths(&self) -> [&str; 2]`.

**F3. `src/menu/mod.rs`**: `mod config; pub use config::{UI_CONFIG, UiConfig};`. `#[derive(Resource)] pub struct UiFonts { pub regular: Handle<Font>, pub title: Handle<Font> }` через `FromWorld` (`AssetServer::load` путей из `UiConfig`), `init_resource::<UiFonts>()` в `MenuPlugin::build` (экран загрузки стартует на первом `StateTransition`, раньше `Startup`; тот же приём, что `CharacterAnimations`). `spawn_loading_screen`: `Text::new(ui.loading.replace("{seed}", &seed.0.to_string()))` + `TextFont { font: FontSource::Handle(fonts.regular.clone()), font_size: FontSize::from(ui.loading_size), ..default() }` (`bevy_text-0.19.1/src/text.rs:282-291, 487, 565`). Удалить устаревший комментарий строки 12 (наш мусор: T5 его закрывает).

**F4. `src/hud/mod.rs`** (новый, ~100 строк): `mod wasted;` `HudPlugin`. `spawn_hud` на `OnTransition { Loading → Playing }`: абсолютный `Node` справа сверху (`top: px(margin)`, `right: px(margin)`, `flex_direction: Column`, `row_gap: px(bar_gap)`), две подложки `px(bar_width) × px(bar_height)` с `BackgroundColor(back_color)`, в каждой заливка с `HudFill(HudBar::Health | HudBar::Armor)` и цветом из `hud`. `update_bars` (`Update`): `Query<&Health, With<Player>>` через let-else (в `Loading` игрока нет → return); `max` из `Res<HealthConfig>`; для каждой заливки `width = percent(100 * value / max)`; писать `node.width` только если значение отличается от текущего. `Changed<Health>` не использовать: `since_damage` меняется каждый фиксированный тик, фильтр был бы всегда истинным.

**F5. `src/hud/wasted.rs`** (новый, ~70 строк): `OnEnter(WastedPhase::Screen)` → полноэкранный `Node` с `BackgroundColor(wasted_backdrop)`, по центру `Text(wasted)` с `TextFont { font: FontSource::Handle(fonts.title.clone()), font_size: FontSize::from(wasted_size) }`, `TextColor(wasted_color)`, `DespawnOnExit(WastedPhase::Screen)` (для sub-state работает: `add_sub_state` включает state-scoped сущности, `bevy_state-0.19.1/src/app.rs:210`; проба подтвердила). `OnEnter(GameState::Wasted)` → для `Query<&mut ColorGrading, With<Camera3d>>` `global.post_saturation = wasted_saturation` (`bevy::render::view::ColorGrading`, `bevy_render-0.19.1/src/view/mod.rs:401-463`; `Camera3d` требует `ColorGrading`, `camera.rs:67`). `OnExit(GameState::Wasted)` → `post_saturation = ColorGradingGlobal::default().post_saturation` (камера создаётся с дефолтом; хранить исходное значение незачем, пока его никто не меняет).

**F6. `src/visuals/config.rs` + `src/visuals/pickups.rs`** (новый, ~50 строк): `#[derive(Deserialize, Clone, Debug)] #[serde(deny_unknown_fields)] pub struct PickupVisuals { pub size: f32, pub lift: f32, pub health_color: (f32, f32, f32), pub armor_color: (f32, f32, f32) }`, поле `pickups: PickupVisuals` в `RenderConfig`, в `validate()`: `size > 0`, `lift > 0`, цвета в `[0, 1]`. В `pickups.rs`: наблюдатель `On<Add, Pickup>` → дочерний `Mesh3d(Cuboid::from_length(size))` + `MeshMaterial3d` цвета по виду, `Transform::from_xyz(0, lift, 0)`; система `Update` ставит `Visibility::Hidden`, пока `!pickup.available()`, иначе `Inherited` (две сущности, каждый кадр, без `Changed<Pickup>`: кулдаун меняется каждый тик). В `VisualsPlugin` добавить наблюдатель и систему. Файл `config.rs` 340 → ~360 строк.

**F7. `src/camera/mod.rs`**: система `reset_pivot` на `OnExit(GameState::Wasted)`: `orbit.pivot = None` (после телепорта `follow_player` в `PostUpdate` берёт новую голову без пролёта через город).

**F8. `src/debug/mod.rs`**: отдельная система `debug_damage_key` в `Update`: `keys.just_pressed(KeyCode::F5)` → `MessageWriter<DebugDamage>::write(DebugDamage { amount: health.debug_damage })`. Ввод читается в `Update` (правило "не `just_pressed` в `FixedUpdate`" соблюдено). Весь модуль уже под `#[cfg(feature = "debug")]`.

**F9. `src/main.rs`**: `mod hud;`; загрузить `UiConfig` тем же `match`-паттерном; `preflight(root, &render_config, &character_config, &ui_config)`: `ui_config.validate()` (ошибка с путём `UI_CONFIG`) и для каждого из `font_paths()` `manifest.contains_asset` (ошибка `"{UI_CONFIG}: font {path} is not listed in {THIRD_PARTY_MANIFEST}"`); наличие файлов уже проверяет `missing_files`. `insert_resource(ui_config)` до `add_plugins`; добавить `hud::HudPlugin`.

### G. Гейт презентации и runtime QA

**G1. `src/visuals/city_gate.rs`**: тест `city_is_built_once_across_respawn` (корректность, гейт клиента): `city_visuals_app(1)` (production `CityVisualsPlugin`); записать множества `Entity` для `(With<CityChunk>, With<Mesh3d>)` и `With<CityProp>`; `write_message(DebugDamage { amount: 1000.0 })`; `update()` до `Wasted` (≤ 3), затем до `Playing` (≤ 300; счётчик как в D3.1); затем ещё 30 update. На **каждом** update после первой сборки: `!contains_resource::<CityMeshTask>() && !contains_resource::<PendingCitySpawn>()`, иначе флаг. Ассерты: флаг не поднялся; множества `Entity` чанков и пропов совпадают с исходными; `count::<With<Player>>() == 1`. Flip-RED: откатить F1 → `CityMeshTask` появляется на первом update в `Playing` → RED (без ожидания второй сборки).

**G2. `tools/qa/scenarios/t5.py`** (по образцу `t3.py`/`t4.py`: `--out`, `release=True`, `--seed 1`, `features=("dev",)`):
1. `fetch_assets.py --check`; `wait_resource("CityLayoutHash")` == golden seed 1; дождаться стабильного числа `CityChunk` (два одинаковых подсчёта подряд), запомнить `chunks0`.
2. Прочитать `HospitalSpawn` (`world.get_resources`), строки `Pickup` + `Transform`, `CharacterBody.float_height`, `Health` игрока == `(100, 0)`.
3. `world.write_message {"message": "gta_sim::player::DebugDamage", "value": {"amount": 40.0}}` → через 0.5 с `current == 60`; скриншот `hud_damaged.png`.
4. Телепорт на броню (`mutate_component` `Position`, path `""`, значение = пикап + Y·`float_height`) → через 0.5 с `armor == 50`; урон 30 → `(60, 20)`.
5. Урон 1000 → опрос `State<gta_sim::flow::GameState>` до `Wasted` (≤ 2 с). Затем опрос `State<gta_sim::flow::WastedPhase>` (ресурс существует только в `Wasted`, ошибки `resource_path` до появления глотать) до `Screen`; **сразу** `screenshot(wasted.png)` через хелпер, ждущий публикации PNG (как `t3.py:26-32`); после публикации проверить, что `GameState` всё ещё `Wasted`, иначе провал "скриншот опоздал". Форму значения `State<…>` разбирать защитно (строка или обёртка) и писать сырое значение в `summary.json`.
6. Опрос до `Playing` (таймаут 10 с), записать реальное время от `Wasted` (ожидание ≈ 4.5 с, мягкий диапазон 3.5-8 с). Горизонтальное расстояние игрока до `HospitalSpawn.point` < 1.0 м; `Health == (100, 0)`; скриншот `respawned.png`.
7. Через 5 с: число `CityChunk == chunks0`, строк `Player == 1` (рантайм-проверка хазарда).
8. Лог без `ERROR` со словами `font`, `asset`, `Failed to load`; `shutdown`; `summary.json`.

**G3. Owner-чеклист для `QA_REPORT.md`** (QA-агент вписывает, владелец проходит): `python tools/fetch_assets.py`; `cargo run --features fast,debug`; экран загрузки по-русски, кириллица рендерится; F5 несколько раз: красная полоса убывает; подобрать броню у больницы: синяя полоса, F5 сначала съедает броню; дождаться регенерации ниже 50%: останавливается на половине; умереть: slow-mo читается, кадр серый, "ПОТРАЧЕНО" видно и не режется; возрождение у больницы, камера сразу на игроке, город не мигает и не двоится; нет анимации смерти (Q2, ожидаемо).

### H. Проверка готовности

`cargo build`; `cargo clippy -- -D warnings` и `cargo clippy --features dev,debug -- -D warnings`; `cargo test -p gta_sim -p citygen`; `cargo test -p gta_like --bin gta_like`; `python tools/qa/tree_check.py`; `cargo tree -p gta_sim -e normal -i bevy_render` и `cargo tree -p gta_sim -e features -i bevy_render` пусты; `python tools/fetch_assets.py --check`; `python tools/qa/scenarios/t5.py --out target/qa/t5`. Для каждого нового гейта из B3, C1, D2-D4, E5, G1 записать в stage summary: какой вход ломали, RED, откат, GREEN.

## 3. Test plan

| Гейт | Где | Класс | Утверждение | Ожидание | Flip-RED |
|---|---|---|---|---|---|
| `anchor_on_segment_examples` | citygen unit | корректность | 5 примеров таблицы B1 | точные точки, `None` для короткой стороны | `t = len/2` |
| `hospital_anchor_on_sidewalk` | citygen properties, 34 seed | корректность | якорь и оба пикапа на тротуаре, не на дороге, не в лоте, напротив больницы | GREEN на всех seed | `t = len/2`; `road_polygon` вместо кольца |
| golden hashes | citygen | регресс | layout не изменился | без изменений | — |
| C1 unit-таблицы | gta_sim unit | корректность | `apply_damage`, `regenerate` | таблицы C1 | переставить порядок, убрать `.min(cap)` |
| D2.1 броня | gta_sim | корректность | броня до нуля, потом здоровье | `(100,20)→(90,0)→(60,0)` | порядок в `apply_damage` |
| D2.2 реген | gta_sim | корректность | задержка 5 с, cap 50% | 30 / 30 / 30.078125 / 50.0; 60 остаётся | `.min(cap)`; проверка до `+=` |
| D2.3 пикапы | gta_sim | корректность | +50, кулдаун, не тратится при полном | 80, armor 50, cooldown 0 | убрать `available()` |
| D2.5 BRP-путь | gta_sim | liveness | `ReflectMessage` доставляет урон | 80 | убрать `#[reflect(Message)]` |
| D2.6 мёртвый стоит | gta_sim | корректность | нет шага и прыжка | сдвиг < 0.1 м | убрать ветку `if dead` |
| D3.1 цикл смерти | gta_sim, город | корректность | фазы по `Time<Real>`, скорость 0.3→1.0→1.0, больница, та же сущность, розыск 0 | Screen k≈96, Playing k≈288, тики SlowMo 27-31, Screen 189-193 | `Res<Time>`; без restore; despawn/spawn; без slow-mo |
| D3.2 выход из SlowMo | gta_sim, город | корректность | 1.0 при выходе из `Wasted` без `Screen` | 1.0 | без restore |
| D3.3 one-shot мир | gta_sim, город | корректность | ни игрок, ни коллайдеры, ни пикапы не дублируются | равные счётчики, Player 1 | `OnEnter(Playing)` |
| D4 конфиги | gta_sim | корректность | загрузка и валидация | ошибки с именем поля | убрать условие |
| E5 манифест | gta_sim | корректность | CC0↔Kenney, OFL↔GitHub-релиз, лицензия на диске | фикстуры судятся верно | Kenney-only; OFL+Kenney |
| G1 город один раз | gta_like bin | корректность (презентация) | нет второй сборки мешей | те же Entity, нет `CityMeshTask` | откат F1 |
| G2 t5.py | runtime BRP | liveness + скриншоты | весь сценарий вживую | см. G2 | — |
| G3 | владелец | feel | полосы, slow-mo, экран, кириллица | чеклист | — |

Существующие тесты (`movement`, `jump`, `anim_state`, `city`, `terrain`, `config`, `asset_manifest`, `character_gate`, `city_gate`) остаются GREEN. Пикапы TestArea в `(±6, 0, 0)` не мешают `movement.rs` (сдвиг ≤ 4.5 м по −X, до брони остаётся 1.5 м > радиуса 1 м), и у пикапов нет коллайдера.

## 4. Rollout notes

- Миграций данных и сохранений нет. `Cargo.lock` не меняется, новых крейтов нет, фич не добавляется (F5 живёт под существующей `debug`).
- Новые обязательные data-файлы: `assets/character/health.ron`, `assets/flow/respawn.ron`, `assets/ui/strings.ron`; `assets/world/render.ron` получает обязательное поле `pickups` (`deny_unknown_fields` + без `default`: старый файл не загрузится, это ожидаемо, файл меняется в той же правке).
- Манифест получает пак `inter`. После слияния каждый чекаут с уже установленными Kenney-паками должен выполнить `python tools/fetch_assets.py` (или `--cache <dir с Inter-4.1.zip>`), иначе `local_assets_match_manifest` упадёт с "partial install: packs [\"inter\"] are missing", а клиентский `preflight` откажется стартовать с "missing third-party asset". Это честное поведение гейта; сказать об этом в QA_REPORT.
- Шрифты и `LICENSE.txt` Inter не коммитятся (игнор `/assets/third_party/*`); OFL разрешает поставку с игрой при сохранении copyright и текста лицензии, `LICENSE.txt` ставится вместе со шрифтами и проверяется E5.
- BRP-адреса для QA: `gta_sim::player::DebugDamage`, `bevy_state::state::resources::State<gta_sim::flow::GameState>`, `…State<gta_sim::flow::WastedPhase>` (существует только в `Wasted`), `gta_sim::world::HospitalSpawn`, `gta_sim::character::Health`, `gta_sim::wanted::WantedLevel`.
- Известное ограничение: `DebugDamage`, записанный во время `Wasted`, остаётся в буфере сообщений и может примениться в первом тике после возрождения (читатель не работает вне `Playing`, буфер держит последние два оборота). Касается только отладки (F5/BRP), не блокирует.
- `WantedLevel` минимальный; heat и звёзды принадлежат T10.

## 5. Review notes

Контрпример, проверенный первым: "пикап попадает в радиус подбора точки возрождения, и первый тик `Playing` меняет `Health` с `(100, 0)`". Для данных плана он не подтвердился (`spacing 6 > radius 1`), но ничто этого не гарантировало: PLAN.md разрешал `spacing >= 0`, а PLAN_V2 предлагал "уменьшать смещение" при короткой стороне. Теперь это закреплено: `validate()` требует `pickups.spacing > pickups.radius` (D4), `sidewalk_anchor` держит оба пикапа на том же отрезке тротуара через `margin = spacing` (B1), D2.4/D3.1 проверяют расстояние и `cooldown == 0` после возрождения.

Что изменено относительно PLAN_V2 (и PLAN.md) и почему:
1. **Восстановление скорости времени.** V2: `OnEnter(Screen)` + дублирующий `OnExit(Wasted)`. Дубль нельзя провалить flip-RED: его удаление ничего не ломает. Заменено одной системой на `OnExit(WastedPhase::SlowMo)`, которая по исходнику `bevy_state-0.19.1` срабатывает и на `SlowMo → Screen`, и на выходе родителя из `Wasted` прямо из `SlowMo`. Оба пути покрыты своими гейтами (D3.1, D3.2). Поправка V2 к GDD §3.4 (экранная фаза при 1.0×) сохранена.
2. **Якорь больницы.** V2 правильно нашёл дефект середины стороны, но его формула (проекция на ось дороги + `perp * walk_offset`, `margin` "выведенный из капсулы и геометрии перекрёстка") не говорила, откуда брать `margin`, и не гарантировала, что точка у конца стороны не попадёт в перекрёсток. Заменено проекцией на кольцо тротуара `geom::inset` (то же, что строит `sidewalks()`), `margin = pickups.spacing` из данных, `None` при короткой стороне. Разобранные примеры пересчитаны под новую функцию (B1). Знак `perp` в V2 был верным (полигоны CCW, `perp` смотрит внутрь), но теперь его несёт существующий и протестированный `inset`.
3. **Гейт "напротив больницы".** PLAN.md проверял `distance <= длина стороны` (V2 прав: ловит мало). Теперь `|(center − point)·along| <= margin` плюс unit-пример 1, который гарантированно RED на старой формуле.
4. **`run_ticks` внутри `Wasted`.** Ни один план не заметил, что `common::run_ticks` паникует после `count + 4` update, а под slow-mo тик приходит раз в ~3.3 update. PLAN.md D2.5 ("update до `Wasted`, потом 64 update") давал бы только ~19 тиков. D2.6 теперь вставляет `Dead` напрямую в `Playing`; D3.1 считает update и тики явно.
5. **Числа фаз пересчитаны** для поправки V2: update до `Screen` и `Playing` не меняются (96/288, по `Time<Real>`), а фиксированные тики теперь делятся на SlowMo 27-31 и Screen 189-193 вместо общих 86 из пробы. Выведено из порядка `First → StateTransition → RunFixedMainLoop → Update` и `bevy_time-0.19.1/src/lib.rs:102-103`.
6. **Прыжок мёртвого.** Ветка `if dead` из PLAN.md пропускала сброс `jump_requested`/буфера: прыжок, нажатый во время смерти, сработал бы после возрождения. Добавлен сброс и проверка в D2.6.
7. **Порядок систем.** PLAN.md упорядочивал `flow::detect_player_death.after(combat::collect_pickups)`, то есть `flow` зависел от `combat`. Введён `character::HealthSystems { Damage, Regen, Pickup, Death }` с `.chain()`.
8. **HUD без `Changed<Health>`.** Принят диагноз V2; решение: писать `node.width` только при изменении значения.
9. **Лицензия ↔ источник.** Принят диагноз V2; уточнено: OFL-URL закреплён версией (`/releases/download/v{version}/`), добавлены фикстуры `bad_license_source`, `bad_ofl_version`, проверка паритета Python и `markers()` вместо одной иглы. Хэши Inter перепроверены на архиве.
10. **Гейт G1.** Вместо ожидания завершения второй сборки (PLAN.md) флаг ставится на первом появлении `CityMeshTask`/`PendingCitySpawn`; сравниваются множества `Entity`, не только счёт (принято из V2).
11. **t5.py.** Принят диагноз V2 о `sleep(2.0)`: опрос `WastedPhase::Screen`, немедленный скриншот, подтверждение `Wasted` после публикации PNG. Число чанков сравнивается с измеренным до смерти, а не с константой 100.
12. **Rollout.** Добавлено предупреждение, что `local_assets_match_manifest` и `preflight` требуют повторного `fetch_assets.py` после слияния.

Восстановлено из PLAN.md то, что V2 сжал без исправления: точные схемы RON (A1-A4), манифест Inter с хэшами (E1), сигнатуры и производные (C1-C9), таблицы чисел тестов и flip-RED (D2-D4), клиентские шаги F1-F9, сценарий QA по шагам (G2), owner-чеклист (G3).

Открытых вопросов к владельцу нет: Q1-Q4 и шрифт Inter приняты в `TASK_FINAL.md`.

Неопределённость, оставленная реализации (помечена, не угадана): точные числа фиксированных тиков по фазам в D3.1 даны диапазонами с выводом; реализатор записывает фактические значения в stage summary. Утверждение "якорь напротив больницы" в B3 может упасть на seed, где больница стоит в углу квартала: тогда это находка для отчёта, а не повод ослабить порог.

children: 0 launched / 0 reported.
