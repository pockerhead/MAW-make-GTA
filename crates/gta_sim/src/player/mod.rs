use crate::character::{
    CharacterControlConfig, Dead, Health, HealthConfig, HealthSystems, LocomotionConfig,
    character_components, regenerate,
};
use crate::combat::Loadout;
use crate::flow::{GameState, PlayingSystems};
use crate::gang::Faction;
use crate::world::PlayerSpawn;
use bevy::prelude::*;

#[derive(Component, Reflect, Default)]
#[reflect(Component)]
pub struct Player;

/// Damage to the player; written by the debug key and by QA over BRP (`world.write_message`).
#[derive(Message, Reflect, Clone, Copy, Debug)]
#[reflect(Message)]
pub struct DebugDamage {
    pub amount: f32,
}

pub struct PlayerPlugin;

impl Plugin for PlayerPlugin {
    fn build(&self, app: &mut App) {
        app.register_type::<Player>()
            .add_message::<DebugDamage>()
            .register_type::<DebugDamage>()
            // Once per session: returning from `Wasted` to `Playing` must not spawn a second player.
            .add_systems(
                OnTransition {
                    exited: GameState::Loading,
                    entered: GameState::Playing,
                },
                spawn_player,
            )
            .add_systems(
                FixedUpdate,
                (
                    apply_debug_damage.in_set(HealthSystems::Damage),
                    regenerate_player.in_set(HealthSystems::Regen),
                )
                    .in_set(PlayingSystems),
            );
    }
}

fn spawn_player(
    mut commands: Commands,
    spawn: Res<PlayerSpawn>,
    config: Res<LocomotionConfig>,
    health: Res<HealthConfig>,
    handle: Res<CharacterControlConfig>,
) {
    commands.spawn((
        Player,
        Name::new("Player"),
        Transform::from_translation(spawn.0 + Vec3::Y * config.float_height),
        character_components(&config, handle.0.clone()),
        Health::full(&health),
        Loadout::default(),
        Faction::Player,
    ));
}

fn apply_debug_damage(
    mut messages: MessageReader<DebugDamage>,
    mut players: Query<&mut Health, (With<Player>, Without<Dead>)>,
) {
    for message in messages.read() {
        let amount = message.amount;
        if !amount.is_finite() || amount <= 0.0 {
            warn!("ignoring DebugDamage with amount {amount}");
            continue;
        }
        for mut health in &mut players {
            health.take(amount);
        }
    }
}

fn regenerate_player(
    cfg: Res<HealthConfig>,
    time: Res<Time<Fixed>>,
    mut players: Query<&mut Health, (With<Player>, Without<Dead>)>,
) {
    let dt = time.delta_secs();
    for mut health in &mut players {
        health.since_damage += dt;
        health.current = regenerate(health.current, health.since_damage, dt, &cfg);
    }
}
