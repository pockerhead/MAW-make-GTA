# IMPL_REVIEW — TASK-025: утечка сенсоров Tnua

Ревьюируемый коммит: `193b876` (ветка `fix/tnua-sensor-leak`). Режим small-fix, сверка идёт напрямую со спекой `task.md`.

## 1. Verdict

**PASS.** Причина починена в корне одним observer'ом на отношении Tnua. Все четыре пути из критериев закрыты гейтами, flip-RED я воспроизвёл сам, clippy чистый.

## Disconfirmation

Контрпример, который сломал бы фикс: в Bevy 0.19.1 `Despawn`-observer срабатывает ПОСЛЕ `on_discard`-хука `RelationshipTarget`. Тогда `TnuaSensorsSet` к этому моменту уже пустой или снят, `sets.get()` уходит в `return`, и утечка остаётся.

Как проверял: `bevy_ecs-0.19.1/src/world/entity_access/world_mut.rs:1645-1680`, `despawn_no_free_with_caller`. Порядок такой: сначала `trigger_raw(DESPAWN)`, потом `trigger_on_despawn`, `Discard`, `trigger_on_discard` и в конце `Remove`. Observer видит компонент целым. Хук `on_discard` (`relationship/mod.rs:306-326`) только ставит в очередь `try_remove::<TnuaSensorOf>` на источники. Эта команда идёт после нашего `try_despawn` и на уже удалённой сущности ничего не делает. **Контрпример не подтвердился.**

Второй кандидат: bevy-tnua сам вызывает `commands.entity(sensor).despawn()` (не `try_`) в `ensure_not_existing` (`bevy-tnua-0.32.0/src/sensor_sets.rs:111-124`). Если он совпадёт в одном тике с деспавном персонажа, будет двойной деспавн и ошибка команды. Путь вызывается только из `walk.rs:544`, когда `config.headroom` переходит из `Some` в `None`. В `crates/gta_sim/src` нет ни `headroom`, ни `Crouch`, ни `ObstacleRadar` (grep пустой), поэтому такой сенсор никогда не создаётся. Сегодня гонки нет.

## 2. Confirmed correct

- `crates/gta_sim/src/character/mod.rs:110`, `:123-136`. Глобальный observer `On<Despawn, TnuaSensorsSet>`: let-else, early return, `try_despawn`. По семантике это ровно то, что сделал бы `linked_spawn`. Он привязан к отношению Tnua, а не к нашему маркеру и не к местам вызова, как и просит спека ("surgical, no per-call-site patches").
- Проверил утверждения из pre-flight: `TnuaSensorsSet` без `linked_spawn` (`bevy-tnua-physics-integration-layer-0.13.0/src/data_for_backends.rs:62-68`), спавн через `with_related_entities::<TnuaSensorOf>` (`sensor_sets.rs:98-107`). Оба верны.
- Выбор между observer и `On<Remove, Character>` обоснован и записан в log.jsonl.
- `crates/gta_sim/tests/sensor_leak.rs`. `all_entities` считает ВСЕ сущности через `iter_entities()` без `IsResource`, а не рефлектируемые запросы. Это соответствует риск-уроку TASK-013. У каждого пути есть liveness-проверка `GATE BROKEN` и два утверждения корректности: мёртвых сенсоров 0 и множество сущностей совпадает с baseline.
- Прогнал сам: `cargo test -p gta_sim --test sensor_leak -j 4` дал 5/5, idle 1675/1676, 63 персонажа деспавнены.
- Сам сделал flip-RED: закомментировал `.add_observer(despawn_tnua_sensors)`. Упали все 5 тестов: recycle 8/8, corpse 8/8, police 4/4, new city 44/44, idle 1707-1716 против 1728-1738. Откатил, sha256 файла совпал.
- `cargo clippy --workspace --all-targets -j 4 -- -D warnings` у меня: exit 0.
- Рантайм-пруф в `scratch/idle_entity_probe/summary.json`: 7382 сущности во всех 19 замерах за 180 с, 81 деспавн мирных, в переписи 0 сирот. Скрипт проверяет наличие `TnuaSensorOf` у каждой сущности с `TnuaProximitySensor` и падает при сироте.

## 3. Issues

Блокирующих и major нет.

- **minor**, `crates/gta_sim/tests/sensor_leak.rs:28` (`SLACK = 10`). Запас долгого гейта против утечки примерно x2: утечка даёт +22, допуск 10. Если утечка станет вдвое медленнее, например из-за меньшего темпа рецикла, гейт её пропустит, и liveness `>= 30` этого не поймает. Реальную защиту несут четыре точных гейта по путям, где разница с baseline должна быть ровно 0. Долгий гейт здесь дополнительный. Можно ничего не менять. Если усиливать, стоит сравнивать всё окно с разбросом 1, например `late.max <= early.max + 2`.

## 4. Missing coverage

- Нет отдельного гейта для gang far-despawn и для лимита трупов. Их покрывают тот же observer и city-гейты: new city и idle. В критериях этих путей нет, реализация честно это указывает. Не блокирует.
- Нет гейта на то, что сенсор не пропадает у ЖИВОГО персонажа, то есть что observer не срабатывает по ошибке. Фактически это покрыто: idle-гейт и все прочие тесты персонажей зелёные при 44 живых со своими сенсорами.

## 5. Nits

- `sensor_leak.rs:243` и `:249-253`: `set_state` и `characters()` дублируют запрос из `new_city_leaves_no_entity`, там можно было вызвать `characters(&mut app)`. Косметика.
