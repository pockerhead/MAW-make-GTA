# PREMISE_CHALLENGE — TASK-013 (GDD T12)

## 1. Counter-example tested

Задача требует настройки через `SettingsPlugin` (фича `bevy_settings`, reverse-domain id,
`SaveSettingsDeferred` + сохранение при выходе). Если в пиненной Bevy 0.19.1 (по `Cargo.lock`)
нет фичи `bevy_settings`, типа `SettingsPlugin` или `SaveSettingsDeferred` (или они устроены иначе,
например без reverse-domain id), то критерий "настройки сохраняются между запусками" нельзя выполнить
так, как сформулирована цель, и премиса опирается на несуществующий API.
Вторично проверяю: есть ли в репо `CityLayoutHash`, `type_text` в `tools/qa/brp.py`, и закрыты ли
зависимости TASK-003/011/012.

## 2. Primary-source investigation

- `Cargo.lock:448-450` — `bevy` `0.19.1` (пин).
- `~/.cargo/registry/src/*/bevy-0.19.1/Cargo.toml:2698` — `bevy_settings = ["bevy_internal/bevy_settings"]`;
  `bevy_internal-0.19.1/Cargo.toml:160` — `bevy_settings = ["bevy-settings"]`.
- `bevy-settings-0.19.1/src/lib.rs:45-51` — документированное требование reverse-domain id;
  `:82` `pub struct SettingsPlugin`, `:89` `pub fn new(app_name: &str)`; `:243` `pub struct SaveSettingsDeferred(pub Duration)`,
  `:251` `impl Command for SaveSettingsDeferred`; `:72-77` — рекомендация deferred save + `SaveSettingsSync::IfChanged`
  перед выходом (`:213` `impl Command for SaveSettingsSync`). Сохранение при выходе не автоматическое, но выполнимо.
- `Cargo.toml:38` — клиент уже включает `features = ["bevy_settings"]`.
- `crates/gta_sim/src/world/city.rs:23` — `pub struct CityLayoutHash(pub u64)`, вставляется в `:125`.
- `Cargo.lock:759-760` — `bevy_brp_extras 0.22.6`; `bevy_brp_extras-0.22.6/src/constants.rs:30` — RPC `type_text` есть
  (в `tools/qa/brp.py` обёртки нет, но `Game.call` на `:73` дёргает любой метод; `src/remote/mod.rs:8` подключает `BrpExtrasPlugin`).
- Входы мини-карты существуют: `gang/behavior.rs:5` `GangTerritories`; `wanted/mod.rs:56` `search_radius`, `wanted/search.rs:87`
  круг поиска; `assets/wanted/wanted.ron:19` `cop_view_cone_deg: 110.0` (`wanted/mod.rs:78`).
- Зависимости: `maw/tasks/done/` содержит TASK-003, TASK-011, TASK-012.
- `docs/design/GDD.md:349` (мини-карта вращается за камерой → "впереди камеры = вверх" согласовано с AC проекции),
  `:360` (спецификация настроек совпадает с API выше), `:638-642` (T12 дословно совпадает с задачей).

## 3. Did it hold

Контрпример не подтвердился. Фича `bevy_settings`, `SettingsPlugin::new`, `SaveSettingsDeferred`, reverse-domain id и
явное сохранение при выходе существуют в пиненном `bevy-settings 0.19.1` ровно так, как задача их называет. QA-примитивы
(`type_text`, `CityLayoutHash`) и геймплейные источники меток (территории, круг поиска, конусы копов) уже есть в коде,
блокеры закрыты. Мелочь без влияния на премису: ветка задачи в task.md `feature/t12-minimap-menu-settings`, фактическая
`feature/t12-minimap-menu`.

## 4. Verdict

PREMISE HOLDS — `bevy-settings-0.19.1/src/lib.rs:82,89,243,251` (SettingsPlugin::new, SaveSettingsDeferred как Command) + `bevy-0.19.1/Cargo.toml:2698` (фича `bevy_settings`) + `bevy_brp_extras-0.22.6/src/constants.rs:30` (`type_text`) + `crates/gta_sim/src/world/city.rs:23` (`CityLayoutHash`); попытка опровергнуть через несуществующий API и отсутствующие входы мини-карты не нашла расхождений.
