use super::{GameState, WastedPhase};
use crate::character::{ActionIntent, CharacterBody, Dead, Health, HealthConfig};
use crate::player::{DebugDamage, Player};
use crate::world::HospitalSpawn;
use avian3d::prelude::*;
use bevy::prelude::*;
use serde::Deserialize;

/// Path of the death/respawn flow config, relative to the assets root.
pub const RESPAWN_CONFIG: &str = "flow/respawn.ron";

/// Timing of `GameState::Wasted` in real seconds (GDD §3.4).
#[derive(Resource, Deserialize, Clone, Debug)]
#[serde(deny_unknown_fields)]
pub struct RespawnConfig {
    /// `Time<Virtual>` speed during the slow-motion phase.
    pub wasted_time_scale: f32,
    pub wasted_slowmo: f32,
    pub wasted_screen: f32,
}

impl RespawnConfig {
    pub fn validate(&self) -> Result<(), String> {
        for (field, value) in [
            ("wasted_time_scale", self.wasted_time_scale),
            ("wasted_slowmo", self.wasted_slowmo),
            ("wasted_screen", self.wasted_screen),
        ] {
            if !value.is_finite() {
                return Err(format!("{field} is not finite"));
            }
        }
        if !(self.wasted_time_scale > 0.0 && self.wasted_time_scale <= 1.0) {
            return Err("wasted_time_scale must be in (0, 1]".into());
        }
        if self.wasted_slowmo < 0.0 {
            return Err("wasted_slowmo must be >= 0".into());
        }
        if self.wasted_screen < 0.0 {
            return Err("wasted_screen must be >= 0".into());
        }
        Ok(())
    }
}

/// Real seconds spent in the current `GameState::Wasted`.
#[derive(Resource, Default)]
pub struct WastedClock(pub f32);

#[allow(clippy::type_complexity)]
pub(super) fn detect_player_death(
    mut commands: Commands,
    players: Query<(Entity, &Health), (With<Player>, Without<Dead>)>,
    mut next: ResMut<NextState<GameState>>,
) {
    for (entity, health) in &players {
        if health.current > 0.0 {
            continue;
        }
        commands.entity(entity).insert(Dead);
        next.set(GameState::Wasted);
    }
}

pub(super) fn enter_wasted(
    cfg: Res<RespawnConfig>,
    mut time: ResMut<Time<Virtual>>,
    mut clock: ResMut<WastedClock>,
) {
    time.set_relative_speed(cfg.wasted_time_scale);
    clock.0 = 0.0;
}

// Update + Time<Real>: under slow motion FixedUpdate runs less often and would stretch the timer.
pub(super) fn advance_wasted(
    cfg: Res<RespawnConfig>,
    real: Res<Time<Real>>,
    phase: Res<State<WastedPhase>>,
    mut clock: ResMut<WastedClock>,
    mut next_phase: ResMut<NextState<WastedPhase>>,
    mut next_state: ResMut<NextState<GameState>>,
) {
    clock.0 += real.delta_secs();
    if *phase.get() == WastedPhase::SlowMo && clock.0 >= cfg.wasted_slowmo {
        next_phase.set(WastedPhase::Screen);
    }
    if clock.0 >= cfg.wasted_slowmo + cfg.wasted_screen {
        next_state.set(GameState::Playing);
    }
}

/// Runs on `SlowMo -> Screen` and also when `Wasted` is left straight from `SlowMo`.
pub(super) fn restore_time_scale(mut time: ResMut<Time<Virtual>>) {
    time.set_relative_speed(1.0);
}

/// Teleports the same player entity to the hospital with full health, so its components survive.
#[allow(clippy::type_complexity)]
pub(super) fn respawn_player(
    mut commands: Commands,
    spawn: Res<HospitalSpawn>,
    cfg: Res<HealthConfig>,
    mut players: Query<
        (
            Entity,
            &mut Position,
            &mut Transform,
            &mut LinearVelocity,
            &mut Health,
            &CharacterBody,
        ),
        With<Player>,
    >,
) {
    for (entity, mut position, mut transform, mut velocity, mut health, body) in &mut players {
        let at = spawn.point + Vec3::Y * body.float_height;
        position.0 = at;
        transform.translation = at;
        velocity.0 = Vec3::ZERO;
        *health = Health::full(&cfg);
        commands.entity(entity).try_remove::<Dead>();
    }
}

// The damage reader runs only in PlayingSystems: a message left from Wasted would hit the respawned player.
pub(super) fn drop_queued_damage(mut queued: ResMut<Messages<DebugDamage>>) {
    queued.clear();
}

// Weapon systems skip a Dead player: a trigger/reload/weapon latch from Wasted would act at the hospital.
pub(super) fn drop_queued_input(mut players: Query<&mut ActionIntent, With<Player>>) {
    for mut action in &mut players {
        *action = ActionIntent::default();
    }
}
