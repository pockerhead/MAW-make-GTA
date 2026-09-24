# ADR-001: локальный патч bevy-tnua-avian3d

## Контекст

`bevy-tnua-avian3d` 0.12.1 безусловно включает `avian3d/debug-plugin`. Он тянет `bevy_gizmos` и `bevy_render` в headless `gta_sim`, нарушая границу GDD §12. Код адаптера не использует debug API.

## Решение

Используем копию версии 0.12.1 в `vendor/` через `[patch.crates-io]`. Изменения относительно опубликованного крейта:

1. Удалена строка `"debug-plugin",` из `[dependencies.avian3d].features`. Это утверждённый вариант Q1=A задачи TASK-002.
2. `continue` вместо `return` в `apply_motors_system` (`src/lib.rs`, TASK-015): труп с `TnuaToggle::Disabled` молча отключал моторы всех персонажей, итерируемых после него. Upstream main проверен 2026-09-24, баг там есть. Гейт: `crates/gta_sim/tests/tnua_motor.rs`.

## Альтернативы

Ослабление headless-гейта скрывает утечку рендера. Копия примерно 450 строк интеграции в `gta_sim` и смена контроллера дороже поддержки однострочного патча.

## Обновление

При новой версии скопировать архив крейта целиком, применить оба изменения, сверить diff с опубликованным крейтом и выполнить `cargo tree -p gta_sim -e normal -i bevy_render` и `cargo test -p gta_sim --test tnua_motor`. Если upstream сделает `debug-plugin` опциональным, удалить `vendor/` и `[patch.crates-io]`.
