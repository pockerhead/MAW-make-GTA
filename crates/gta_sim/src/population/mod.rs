//! Population bubble around the player: off-camera spawning on sidewalk nodes, despawn far away
//! and off-frame, corpse lifetime (GDD §6.1, §4.3).

use crate::character::{CharacterControlConfig, Dead, HealthConfig, LocomotionConfig};
use crate::civilian::{Civilian, CivilianConfig, CivilianState, civilian_bundle, roll_temperament};
use crate::combat::unit_f32;
use crate::navigation::{GraphWalker, SidewalkGraph, flat_distance, wander_next};
use crate::perception::sight_blocked;
use crate::player::Player;
use avian3d::prelude::*;
use bevy::prelude::*;
use bevy_tnua::TnuaToggle;
use rand_chacha::{
    ChaCha8Rng,
    rand_core::{Rng, SeedableRng},
};
use serde::Deserialize;

/// Path of the population config, relative to the assets root.
pub const POPULATION_CONFIG: &str = "npc/population.ron";

/// Population bubble tuning (GDD §6.1).
#[derive(Resource, Deserialize, Clone, Debug)]
#[serde(deny_unknown_fields)]
pub struct PopulationConfig {
    pub max_civilians: u32,
    /// Steady-state spawn ring around the player (inner, outer), m.
    pub spawn_ring: (f32, f32),
    pub spawns_per_tick: u32,
    /// Inner radius of the one-shot fill at load; its outer radius is `spawn_ring.1`, m.
    pub initial_inner_radius: f32,
    pub initial_spawns_per_tick: u32,
    /// Minimum distance of a spawn node to any civilian or corpse, m.
    pub spawn_min_separation: f32,
    /// Spawns stay outside the camera cone widened by this, degrees.
    pub spawn_view_margin_deg: f32,
    pub despawn_distance: f32,
    pub despawn_offscreen_seconds: f32,
    pub corpse_seconds: f32,
    pub corpse_limit: u32,
}

impl PopulationConfig {
    pub fn validate(&self) -> Result<(), String> {
        let (inner, outer) = self.spawn_ring;
        for (field, value) in [
            ("spawn_ring", inner),
            ("spawn_ring", outer),
            ("initial_inner_radius", self.initial_inner_radius),
            ("spawn_min_separation", self.spawn_min_separation),
            ("spawn_view_margin_deg", self.spawn_view_margin_deg),
            ("despawn_distance", self.despawn_distance),
            ("despawn_offscreen_seconds", self.despawn_offscreen_seconds),
            ("corpse_seconds", self.corpse_seconds),
        ] {
            if !value.is_finite() {
                return Err(format!("{field} is not finite"));
            }
        }
        if !(0.0 < self.initial_inner_radius && self.initial_inner_radius < inner) {
            return Err(format!(
                "initial_inner_radius {} must satisfy 0 < initial_inner_radius < spawn_ring.0 ({inner})",
                self.initial_inner_radius
            ));
        }
        if !(inner < outer && outer < self.despawn_distance) {
            return Err(format!(
                "spawn_ring {:?} must satisfy spawn_ring.0 < spawn_ring.1 < despawn_distance ({})",
                self.spawn_ring, self.despawn_distance
            ));
        }
        if self.spawns_per_tick < 1 {
            return Err("spawns_per_tick must be >= 1".into());
        }
        if self.initial_spawns_per_tick < 1 {
            return Err("initial_spawns_per_tick must be >= 1".into());
        }
        if self.spawn_min_separation <= 0.0 {
            return Err("spawn_min_separation must be > 0".into());
        }
        if !(0.0..90.0).contains(&self.spawn_view_margin_deg) {
            return Err("spawn_view_margin_deg must be in [0, 90)".into());
        }
        if self.despawn_offscreen_seconds < 0.0 {
            return Err("despawn_offscreen_seconds must be >= 0".into());
        }
        if self.corpse_seconds <= 0.0 {
            return Err("corpse_seconds must be > 0".into());
        }
        Ok(())
    }
}

/// Bounding cone of the camera frustum.
#[derive(Reflect, Clone, Copy, Debug, PartialEq)]
pub struct ViewCone {
    pub origin: Vec3,
    /// Unit length.
    pub forward: Vec3,
    pub half_angle: f32,
}

impl ViewCone {
    /// The cone through the frustum corners of a perspective camera (`fov_y` vertical, radians).
    pub fn from_perspective(origin: Vec3, forward: Dir3, fov_y: f32, aspect: f32) -> Self {
        let t = (fov_y / 2.0).tan();
        Self {
            origin,
            forward: forward.as_vec3(),
            half_angle: (t * t + (aspect * t).powi(2)).sqrt().atan(),
        }
    }

    pub fn contains(&self, point: Vec3, margin_rad: f32) -> bool {
        let d = point - self.origin;
        d == Vec3::ZERO || d.angle_between(self.forward) <= self.half_angle + margin_rad
    }
}

/// The camera view the client published; `None` until the first publish (no spawning, no off-frame ageing).
#[derive(Resource, Reflect, Default, Clone, Copy, Debug)]
#[reflect(Resource)]
pub struct CameraView(pub Option<ViewCone>);

/// Seconds a civilian has spent outside the camera view.
#[derive(Component, Reflect, Default)]
#[reflect(Component)]
pub struct Offscreen(pub f32);

/// A dead NPC body; despawned after `corpse_seconds` or when over `corpse_limit` (oldest first).
#[derive(Component, Reflect)]
#[reflect(Component)]
pub struct Corpse {
    pub age: f32,
}

/// Random look roll; the client maps it to a model and a tint (the sim never names an asset).
#[derive(Component, Reflect, Clone, Copy, Debug)]
#[reflect(Component)]
pub struct Appearance(pub u32);

/// Spawning mode: the one-shot fill at load, then the steady ring.
#[derive(Resource, Reflect, Default, PartialEq, Eq, Debug, Clone, Copy)]
#[reflect(Resource)]
pub enum PopulationPhase {
    #[default]
    InitialFill,
    Steady,
}

/// Sim-owned RNG of NPC decisions; a different stream than `CombatRng` of the same seed.
#[derive(Resource)]
pub struct NpcRng(pub ChaCha8Rng);

impl NpcRng {
    pub fn seeded(seed: u64) -> Self {
        let mut rng = ChaCha8Rng::seed_from_u64(seed);
        rng.set_stream(1);
        Self(rng)
    }

    /// Uniform in `[0, 1)`.
    pub fn unit(&mut self) -> f32 {
        unit_f32(&mut self.0)
    }

    pub fn next_u32(&mut self) -> u32 {
        self.0.next_u32()
    }
}

#[derive(SystemSet, Debug, Clone, PartialEq, Eq, Hash)]
pub struct PopulationSystems;

/// Turns a character into an inert corpse: no controller, no collisions, not hit by rays.
pub fn corpse_components() -> impl Bundle {
    (
        Dead,
        Corpse { age: 0.0 },
        TnuaToggle::Disabled,
        RigidBody::Static,
        CollisionLayers::NONE,
    )
}

/// Both the feet (+0.1 m) and the head of a body at `feet` lie outside the cone widened by `margin_rad`.
pub fn outside_cone(view: &ViewCone, feet: Vec3, head_height: f32, margin_rad: f32) -> bool {
    !view.contains(feet + Vec3::Y * 0.1, margin_rad)
        && !view.contains(feet + Vec3::Y * head_height, margin_rad)
}

/// World geometry blocks both the feet (+0.1 m) and the head of a body at `feet` from the camera.
pub fn occluded(spatial: &SpatialQuery, view: &ViewCone, feet: Vec3, head_height: f32) -> bool {
    sight_blocked(spatial, view.origin, feet + Vec3::Y * 0.1)
        && sight_blocked(spatial, view.origin, feet + Vec3::Y * head_height)
}

pub struct PopulationPlugin {
    /// Seed of `NpcRng` (the combat seed).
    pub seed: u64,
}

impl Plugin for PopulationPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<CameraView>()
            .init_resource::<PopulationPhase>()
            .insert_resource(NpcRng::seeded(self.seed))
            .register_type::<CameraView>()
            .register_type::<ViewCone>()
            .register_type::<Offscreen>()
            .register_type::<Corpse>()
            .register_type::<Appearance>()
            .register_type::<PopulationPhase>()
            .add_systems(
                FixedUpdate,
                (age_corpses, despawn_far, spawn_civilians)
                    .chain()
                    .in_set(PopulationSystems),
            );
    }
}

fn age_corpses(
    mut commands: Commands,
    cfg: Res<PopulationConfig>,
    time: Res<Time<Fixed>>,
    mut corpses: Query<(Entity, &mut Corpse)>,
) {
    let dt = time.delta_secs();
    let mut kept = Vec::new();
    for (entity, mut corpse) in &mut corpses {
        if corpse.age >= cfg.corpse_seconds {
            commands.entity(entity).try_despawn();
            continue;
        }
        corpse.age += dt;
        kept.push((corpse.age, entity));
    }
    let extra = kept.len().saturating_sub(cfg.corpse_limit as usize);
    kept.sort_by(|a, b| b.0.total_cmp(&a.0));
    for &(_, entity) in &kept[..extra] {
        commands.entity(entity).try_despawn();
    }
}

fn despawn_far(
    mut commands: Commands,
    cfg: Res<PopulationConfig>,
    view: Res<CameraView>,
    loco: Res<LocomotionConfig>,
    time: Res<Time<Fixed>>,
    player: Query<&Position, With<Player>>,
    mut civilians: Query<(Entity, &Position, &mut Offscreen), With<Civilian>>,
) {
    let Some(view) = view.0 else {
        return;
    };
    let Ok(player) = player.single() else {
        return;
    };
    let dt = time.delta_secs();
    for (entity, position, mut offscreen) in &mut civilians {
        let feet = position.0 - Vec3::Y * loco.float_height;
        // Occlusion is ignored: a civilian behind a building counts as in frame (never a pop-out).
        if outside_cone(&view, feet, loco.head_height, 0.0) {
            offscreen.0 += dt;
        } else {
            offscreen.0 = 0.0;
        }
        if flat_distance(position.0, player.0) > cfg.despawn_distance
            && offscreen.0 >= cfg.despawn_offscreen_seconds
        {
            commands.entity(entity).try_despawn();
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn spawn_civilians(
    mut commands: Commands,
    cfg: Res<PopulationConfig>,
    civilian_cfg: Res<CivilianConfig>,
    view: Res<CameraView>,
    mut phase: ResMut<PopulationPhase>,
    graph: Res<SidewalkGraph>,
    mut rng: ResMut<NpcRng>,
    spatial: SpatialQuery,
    characters: (
        Res<LocomotionConfig>,
        Res<HealthConfig>,
        Res<CharacterControlConfig>,
    ),
    player: Query<&Position, With<Player>>,
    civilians: Query<(&Position, &Civilian)>,
) {
    let Some(view) = view.0 else {
        return;
    };
    let Ok(player) = player.single() else {
        return;
    };
    let (loco, health, handle) = characters;
    let alive = civilians
        .iter()
        .filter(|(_, c)| c.state != CivilianState::Dead)
        .count() as u32;
    let deficit = cfg.max_civilians.saturating_sub(alive);
    if deficit == 0 {
        *phase = PopulationPhase::Steady;
        return;
    }
    let initial = *phase == PopulationPhase::InitialFill;
    let ((inner, outer), budget) = if initial {
        (
            (cfg.initial_inner_radius, cfg.spawn_ring.1),
            cfg.initial_spawns_per_tick,
        )
    } else {
        (cfg.spawn_ring, cfg.spawns_per_tick)
    };
    let separation = cfg.spawn_min_separation;
    let taken = civilians.iter().map(|(p, _)| p.0).collect::<Vec<_>>();
    let mut candidates = (0..graph.nodes().len() as u32)
        .filter(|&n| {
            let at = graph.node(n);
            let d = flat_distance(at, player.0);
            inner <= d && d <= outer && !graph.neighbors(n).is_empty()
        })
        .filter(|&n| {
            let at = graph.node(n);
            taken.iter().all(|&p| flat_distance(p, at) >= separation)
        })
        .collect::<Vec<_>>();
    for i in (1..candidates.len()).rev() {
        let j = ((rng.unit() * (i + 1) as f32) as usize).min(i);
        candidates.swap(i, j);
    }
    let margin = cfg.spawn_view_margin_deg.to_radians();
    let wanted = budget.min(deficit) as usize;
    let mut spawned: Vec<Vec3> = Vec::new();
    for node in candidates {
        if spawned.len() == wanted {
            break;
        }
        let at = graph.node(node);
        if spawned.iter().any(|&p| flat_distance(p, at) < separation) {
            continue;
        }
        let hidden = outside_cone(&view, at, loco.head_height, margin)
            || (initial && occluded(&spatial, &view, at, loco.head_height));
        if !hidden {
            continue;
        }
        let walker = GraphWalker {
            from: node,
            to: wander_next(&graph, node, node, rng.unit()),
        };
        let temperament = roll_temperament(&mut rng, civilian_cfg.reaction.temperament_spread);
        let appearance = Appearance(rng.next_u32());
        commands.spawn(civilian_bundle(
            &loco,
            handle.0.clone(),
            &health,
            &graph,
            walker,
            0.0,
            temperament,
            appearance,
        ));
        spawned.push(at);
    }
    if initial && (spawned.len() < budget as usize || spawned.len() as u32 == deficit) {
        *phase = PopulationPhase::Steady;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn deg(v: f32) -> f32 {
        v.to_degrees()
    }

    #[test]
    fn frustum_bounding_cone() {
        for (fov, aspect, expected) in [
            (70.0_f32, 16.0 / 9.0, 55.00),
            (55.0, 16.0 / 9.0, 46.72),
            (90.0, 1.0, 54.74),
        ] {
            let cone =
                ViewCone::from_perspective(Vec3::ZERO, Dir3::NEG_Z, fov.to_radians(), aspect);
            assert!(
                (deg(cone.half_angle) - expected).abs() < 0.01,
                "{fov} {aspect}: {}",
                deg(cone.half_angle)
            );
        }
    }

    #[test]
    fn cone_membership() {
        let cone =
            ViewCone::from_perspective(Vec3::ZERO, Dir3::NEG_Z, 70f32.to_radians(), 16.0 / 9.0);
        assert!(cone.contains(Vec3::new(0.0, 0.0, -10.0), 0.0));
        assert!(cone.contains(Vec3::new(-5.0, 0.0, -10.0), 0.0));
        assert!(!cone.contains(Vec3::new(10.0, 0.0, 0.0), 0.0));
        assert!(!cone.contains(Vec3::new(0.0, 0.0, 10.0), 0.0));
        assert!(cone.contains(Vec3::ZERO, 0.0));
        let up = ViewCone::from_perspective(
            Vec3::new(0.0, 2.0, 0.0),
            Dir3::Y,
            70f32.to_radians(),
            16.0 / 9.0,
        );
        for p in [
            Vec3::new(100.0, 2.0, 0.0),
            Vec3::new(0.0, 0.0, -30.0),
            Vec3::new(-5.0, -1.0, 5.0),
        ] {
            assert!(!up.contains(p, 0.0), "{p}");
        }
    }
}
