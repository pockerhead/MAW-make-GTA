//! NPC hearing and sight, time-sliced over `slots` fixed ticks (GDD §6.2, §11).

use crate::character::{AimIntent, Dead, LocomotionConfig};
use crate::civilian::{Civilian, CivilianState};
use crate::combat::{DamageDealt, Loadout, MeleeHit, ShotFired};
use crate::layers::GameLayer;
use crate::population::Corpse;
use avian3d::prelude::*;
use bevy::prelude::*;
use serde::Deserialize;

/// Path of the perception config, relative to the assets root.
pub const PERCEPTION_CONFIG: &str = "npc/perception.ron";

#[derive(Resource, Deserialize, Clone, Debug)]
#[serde(deny_unknown_fields)]
pub struct PerceptionConfig {
    /// Agents perceive in turn, one slot per fixed tick.
    pub slots: u8,
    pub hearing_radius: f32,
    pub fight_hearing_radius: f32,
    pub corpse_sight: f32,
    pub aimed_distance: f32,
    /// Half-angle between an aim ray and the civilian's chest, degrees.
    pub aimed_cone_deg: f32,
}

impl PerceptionConfig {
    pub fn validate(&self) -> Result<(), String> {
        if self.slots < 1 {
            return Err("slots must be >= 1".into());
        }
        for (field, value) in [
            ("hearing_radius", self.hearing_radius),
            ("fight_hearing_radius", self.fight_hearing_radius),
            ("corpse_sight", self.corpse_sight),
            ("aimed_distance", self.aimed_distance),
        ] {
            if !(value.is_finite() && value > 0.0) {
                return Err(format!("{field} {value} must be finite and > 0"));
            }
        }
        if !(self.aimed_cone_deg > 0.0 && self.aimed_cone_deg < 90.0) {
            return Err(format!(
                "aimed_cone_deg {} must be in (0, 90)",
                self.aimed_cone_deg
            ));
        }
        Ok(())
    }
}

#[derive(Reflect, Clone, Copy, Debug, PartialEq, Eq)]
pub enum ThreatKind {
    Gunshot,
    Fight,
    Corpse,
    Aimed,
    Hurt,
}

/// What produced a threat: an attack id (`AttackSerial`) or a corpse.
#[derive(Reflect, Clone, Copy, Debug, PartialEq, Eq)]
pub enum Cause {
    Attack(u32),
    Body(Entity),
}

/// A perceived threat: where it is and how far from the perceiver.
#[derive(Reflect, Clone, Copy, Debug, PartialEq)]
pub struct Threat {
    pub kind: ThreatKind,
    pub at: Vec3,
    pub distance: f32,
    /// `None` for an aimed gun: aiming is no crime.
    pub cause: Option<Cause>,
}

/// Perception state of one NPC; `pending` is consumed by its decision system in the same tick.
#[derive(Component, Reflect, Default)]
#[reflect(Component)]
pub struct Perception {
    pub slot: u8,
    pub pending: Option<Threat>,
}

/// Fixed ticks of NPC time.
#[derive(Resource, Reflect, Default)]
#[reflect(Resource)]
pub struct AiClock {
    pub tick: u64,
}

/// Next slot handed to a new `Perception`.
#[derive(Resource, Default)]
struct SlotCursor(u8);

/// Sounds of the last `slots` ticks: `(tick, kind, point, attack)`.
#[derive(Resource, Default)]
pub struct StimulusLog(pub Vec<(u64, ThreatKind, Vec3, u32)>);

/// Work `perceive` did in the current tick.
#[derive(Resource, Reflect, Default, Clone, Copy, Debug)]
#[reflect(Resource)]
pub struct PerceptionLoad {
    pub agents: u32,
    pub rays: u32,
}

#[derive(SystemSet, Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum AiSystems {
    Perceive,
    Decide,
}

/// A World-layer hit closer than `to` blocks the line from `from`.
pub fn sight_blocked(spatial: &SpatialQuery, from: Vec3, to: Vec3) -> bool {
    let Ok((direction, distance)) = Dir3::new_and_length(to - from) else {
        return false;
    };
    let filter = SpatialQueryFilter::from_mask(GameLayer::World);
    spatial
        .cast_ray(from, direction, distance, true, &filter)
        .is_some_and(|hit| hit.distance < distance)
}

pub struct PerceptionPlugin;

impl Plugin for PerceptionPlugin {
    fn build(&self, app: &mut App) {
        use crate::character::HealthSystems;
        use crate::flow::{NEW_CITY, NpcSystems};
        use crate::population::PopulationSystems;
        use bevy_tnua::prelude::TnuaUserControlsSystems;
        app.init_resource::<AiClock>()
            .init_resource::<SlotCursor>()
            .init_resource::<StimulusLog>()
            .init_resource::<PerceptionLoad>()
            .register_type::<Perception>()
            .register_type::<Threat>()
            .register_type::<ThreatKind>()
            .register_type::<Cause>()
            .register_type::<AiClock>()
            .register_type::<PerceptionLoad>()
            .add_observer(assign_slot)
            .configure_sets(
                FixedUpdate,
                (AiSystems::Perceive, AiSystems::Decide, PopulationSystems)
                    .chain()
                    .in_set(NpcSystems)
                    .after(HealthSystems::Death),
            )
            .configure_sets(
                FixedUpdate,
                AiSystems::Decide.before(TnuaUserControlsSystems),
            )
            .add_systems(
                FixedUpdate,
                (advance_clock, collect_stimuli, perceive)
                    .chain()
                    .in_set(AiSystems::Perceive),
            )
            .add_systems(NEW_CITY, clear_stimuli);
    }
}

fn clear_stimuli(mut log: ResMut<StimulusLog>) {
    log.0.clear();
}

fn assign_slot(
    add: On<Add, Perception>,
    cfg: Res<PerceptionConfig>,
    mut cursor: ResMut<SlotCursor>,
    mut perceptions: Query<&mut Perception>,
) {
    let Ok(mut perception) = perceptions.get_mut(add.entity) else {
        return;
    };
    perception.slot = cursor.0;
    cursor.0 = (cursor.0 + 1) % cfg.slots;
}

fn advance_clock(mut clock: ResMut<AiClock>) {
    clock.tick += 1;
}

/// Logs this tick's sounds; a hit on a civilian is felt at once (no slicing).
#[allow(clippy::too_many_arguments)]
fn collect_stimuli(
    cfg: Res<PerceptionConfig>,
    clock: Res<AiClock>,
    mut log: ResMut<StimulusLog>,
    mut shots: MessageReader<ShotFired>,
    mut fights: MessageReader<MeleeHit>,
    mut damage: MessageReader<DamageDealt>,
    positions: Query<&Position>,
    mut victims: Query<(&Civilian, &mut Perception)>,
) {
    let tick = clock.tick;
    let slots = u64::from(cfg.slots);
    log.0.retain(|&(then, ..)| then + slots > tick);
    log.0.extend(
        shots
            .read()
            .map(|shot| (tick, ThreatKind::Gunshot, shot.muzzle, shot.attack)),
    );
    log.0.extend(
        fights
            .read()
            .map(|hit| (tick, ThreatKind::Fight, hit.point, hit.attack)),
    );
    for hit in damage.read() {
        let Ok((civilian, mut perception)) = victims.get_mut(hit.target) else {
            continue;
        };
        if civilian.state == CivilianState::Dead {
            continue;
        }
        let at = positions.get(hit.shooter).map_or(hit.point, |p| p.0);
        perception.pending = Some(Threat {
            kind: ThreatKind::Hurt,
            at,
            distance: 0.0,
            cause: Some(Cause::Attack(hit.shot)),
        });
    }
}

#[allow(clippy::type_complexity, clippy::too_many_arguments)]
fn perceive(
    cfg: Res<PerceptionConfig>,
    loco: Res<LocomotionConfig>,
    clock: Res<AiClock>,
    log: Res<StimulusLog>,
    spatial: SpatialQuery,
    mut load: ResMut<PerceptionLoad>,
    mut agents: Query<(Entity, &Civilian, &Position, &mut Perception)>,
    corpses: Query<(Entity, &Position), With<Corpse>>,
    aimers: Query<(Entity, &Position, &AimIntent, &Loadout), Without<Dead>>,
) {
    *load = PerceptionLoad::default();
    let slot = (clock.tick % u64::from(cfg.slots)) as u8;
    let aim_cone = cfg.aimed_cone_deg.to_radians();
    for (entity, civilian, position, mut perception) in &mut agents {
        if perception.slot != slot || civilian.state == CivilianState::Dead {
            continue;
        }
        load.agents += 1;
        if perception
            .pending
            .is_some_and(|threat| threat.kind == ThreatKind::Hurt)
        {
            continue;
        }
        let chest = position.0;
        let eyes = chest - Vec3::Y * loco.float_height + Vec3::Y * loco.head_height;
        let mut nearest: Option<Threat> = None;
        let mut offer = |kind: ThreatKind, at: Vec3, distance: f32, cause: Option<Cause>| {
            if nearest.is_none_or(|n| distance < n.distance) {
                nearest = Some(Threat {
                    kind,
                    at,
                    distance,
                    cause,
                });
            }
        };
        for &(_, kind, at, attack) in &log.0 {
            let radius = match kind {
                ThreatKind::Gunshot => cfg.hearing_radius,
                ThreatKind::Fight => cfg.fight_hearing_radius,
                _ => continue,
            };
            let distance = chest.distance(at);
            if distance <= radius {
                offer(kind, at, distance, Some(Cause::Attack(attack)));
            }
        }
        // A caller is already reporting the bodies in sight; only a new sound, aim or hit interrupts.
        let reporting = matches!(civilian.state, CivilianState::Report { .. });
        let corpse = corpses
            .iter()
            .filter(|_| !reporting)
            .map(|(body, p)| (body, p.0, chest.distance(p.0)))
            .filter(|&(_, _, d)| d <= cfg.corpse_sight)
            .min_by(|a, b| a.2.total_cmp(&b.2));
        if let Some((body, at, distance)) = corpse {
            load.rays += 1;
            if !sight_blocked(&spatial, eyes, at) {
                offer(ThreatKind::Corpse, at, distance, Some(Cause::Body(body)));
            }
        }
        for (aimer, at, aim, loadout) in &aimers {
            if aimer == entity || !aim.aiming || loadout.held.is_none() {
                continue;
            }
            let distance = chest.distance(at.0);
            let to_chest = chest - aim.origin;
            if distance > cfg.aimed_distance || aim.direction.angle_between(to_chest) > aim_cone {
                continue;
            }
            load.rays += 1;
            if !sight_blocked(&spatial, eyes, at.0) {
                offer(ThreatKind::Aimed, at.0, distance, None);
            }
        }
        perception.pending = nearest;
    }
}
