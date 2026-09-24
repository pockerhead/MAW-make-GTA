//! Population bubble around the player: spawning on sidewalk points the player cannot see (off the
//! camera cone or behind buildings), ahead of the view first; despawn far away and off-frame (at the
//! cap also calm civilians behind the view, off-frame past `recycle_distance`), corpse lifetime
//! (GDD §6.1, §4.3).

mod gangs;

use crate::character::{CharacterControlConfig, Dead, HealthConfig, LocomotionConfig};
use crate::civilian::{Civilian, CivilianConfig, CivilianState, civilian_bundle, roll_temperament};
use crate::combat::unit_f32;
use crate::flow::{GameState, NEW_CITY};
use crate::gang::GangSystems;
use crate::navigation::{GraphWalker, SidewalkGraph, flat_distance, wander_next};
use crate::perception::sight_blocked;
use crate::player::Player;
use crate::world::CitySeed;
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
    /// Alive gang members at most; 0 disables gangs, below `groups.size.0` nothing spawns.
    pub max_gang_members: u32,
    /// Steady-state spawn ring around the player (inner, outer), m.
    pub spawn_ring: (f32, f32),
    pub spawns_per_tick: u32,
    /// Inner radius of the one-shot fill at load; its outer radius is `spawn_ring.1`, m.
    pub initial_inner_radius: f32,
    pub initial_spawns_per_tick: u32,
    /// Minimum distance of a spawn node to any civilian or corpse, m.
    pub spawn_min_separation: f32,
    /// Spawns stay outside the camera cone widened by this, degrees, unless occluded.
    pub spawn_view_margin_deg: f32,
    /// Spawn points lie on the nodes and every this many metres along each sidewalk edge, m.
    pub spawn_point_spacing: f32,
    /// Weight of "ahead of the view" in the spawn order; 0 = random order, 1 = ahead on par with chance.
    pub spawn_forward_weight: f32,
    /// Height above a node of the camera ray that must hit a building to spawn inside the cone, m.
    pub occlusion_ray_height: f32,
    /// Occlusion rays cast per fixed tick, at most (`OCCLUSION_RAYS_PER_POINT` per checked point).
    pub occlusion_rays_per_tick: u32,
    pub despawn_distance: f32,
    /// At the cap, calm civilians behind the view and off-frame this far away are despawned so the
    /// budget refills ahead, m.
    pub recycle_distance: f32,
    /// Civilians recycled per fixed tick at most, farthest first.
    pub recycles_per_tick: u32,
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
            ("spawn_point_spacing", self.spawn_point_spacing),
            ("spawn_forward_weight", self.spawn_forward_weight),
            ("occlusion_ray_height", self.occlusion_ray_height),
            ("despawn_distance", self.despawn_distance),
            ("recycle_distance", self.recycle_distance),
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
        if !(inner <= self.recycle_distance && self.recycle_distance <= self.despawn_distance) {
            return Err(format!(
                "recycle_distance {} must satisfy spawn_ring.0 ({inner}) <= recycle_distance <= despawn_distance ({})",
                self.recycle_distance, self.despawn_distance
            ));
        }
        if self.spawns_per_tick < 1 {
            return Err("spawns_per_tick must be >= 1".into());
        }
        if self.recycles_per_tick < 1 {
            return Err("recycles_per_tick must be >= 1".into());
        }
        if self.initial_spawns_per_tick < 1 {
            return Err("initial_spawns_per_tick must be >= 1".into());
        }
        if self.spawn_min_separation <= 0.0 {
            return Err("spawn_min_separation must be > 0".into());
        }
        if self.spawn_point_spacing < self.spawn_min_separation {
            return Err(format!(
                "spawn_point_spacing {} must be >= spawn_min_separation ({})",
                self.spawn_point_spacing, self.spawn_min_separation
            ));
        }
        if self.spawn_forward_weight < 0.0 {
            return Err("spawn_forward_weight must be >= 0".into());
        }
        if self.occlusion_ray_height <= 0.1 {
            return Err("occlusion_ray_height must be > 0.1 (the feet ray)".into());
        }
        if self.occlusion_rays_per_tick < OCCLUSION_RAYS_PER_POINT {
            return Err(format!(
                "occlusion_rays_per_tick must be >= {OCCLUSION_RAYS_PER_POINT} (one spawn point)"
            ));
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

/// Occlusion rays `spawn_civilians` cast in the current tick.
#[derive(Resource, Reflect, Default, Clone, Copy, Debug)]
#[reflect(Resource)]
pub struct PopulationLoad {
    pub rays: u32,
}

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

/// Rays `occluded` casts per spawn point, at most.
pub const OCCLUSION_RAYS_PER_POINT: u32 = 4;

/// World geometry hides a body at `feet` from the camera across its width: rays to the head
/// (`ray_height`) at the centre and `half_width` to either side, and to the feet (+0.1 m); adds the
/// rays cast to `rays`.
pub fn occluded(
    spatial: &SpatialQuery,
    view: &ViewCone,
    feet: Vec3,
    ray_height: f32,
    half_width: f32,
    rays: &mut u32,
) -> bool {
    // Buildings stand on the ground: a blocked ray to the top of a vertical line hides the line below.
    let side = (feet - view.origin)
        .with_y(0.0)
        .cross(Vec3::Y)
        .normalize_or_zero()
        * half_width;
    let head = feet + Vec3::Y * ray_height;
    [head, head + side, head - side, feet + Vec3::Y * 0.1]
        .into_iter()
        .all(|target| {
            *rays += 1;
            sight_blocked(spatial, view.origin, target)
        })
}

pub struct PopulationPlugin {
    /// Seed of `NpcRng` (the combat seed).
    pub seed: u64,
}

impl Plugin for PopulationPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<CameraView>()
            .init_resource::<PopulationPhase>()
            .init_resource::<PopulationLoad>()
            .insert_resource(NpcRng::seeded(self.seed))
            .register_type::<CameraView>()
            .register_type::<ViewCone>()
            .register_type::<Offscreen>()
            .register_type::<Corpse>()
            .register_type::<Appearance>()
            .register_type::<PopulationPhase>()
            .register_type::<PopulationLoad>()
            .add_systems(
                FixedUpdate,
                (age_corpses, despawn_far, spawn_civilians)
                    .chain()
                    .in_set(PopulationSystems),
            )
            .add_systems(
                FixedUpdate,
                (gangs::despawn_far_gangs, gangs::spawn_gangs)
                    .chain()
                    .after(spawn_civilians)
                    .in_set(PopulationSystems)
                    .in_set(GangSystems),
            )
            .add_systems(NEW_CITY, reset_population)
            .add_systems(
                OnEnter(GameState::Loading),
                reseed_population.run_if(resource_exists::<CitySeed>),
            );
    }
}

/// The new city gets its load-time fill; a stale view would place it against the old camera.
fn reset_population(mut phase: ResMut<PopulationPhase>, mut view: ResMut<CameraView>) {
    *phase = PopulationPhase::InitialFill;
    *view = CameraView(None);
}

fn reseed_population(seed: Res<CitySeed>, mut rng: ResMut<NpcRng>) {
    *rng = NpcRng::seeded(seed.0);
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
    mut civilians: Query<(Entity, &Position, &mut Offscreen, &Civilian)>,
) {
    let Some(view) = view.0 else {
        return;
    };
    let Ok(player) = player.single() else {
        return;
    };
    let dt = time.delta_secs();
    let alive = civilians
        .iter()
        .filter(|(.., c)| c.state != CivilianState::Dead)
        .count() as u32;
    let at_cap = alive >= cfg.max_civilians;
    let look = view.forward.with_y(0.0).normalize_or_zero();
    let mut recyclable = Vec::new();
    for (entity, position, mut offscreen, civilian) in &mut civilians {
        let feet = position.0 - Vec3::Y * loco.float_height;
        // Occlusion is ignored: a civilian behind a building counts as in frame (never a pop-out).
        if outside_cone(&view, feet, loco.head_height, 0.0) {
            offscreen.0 += dt;
        } else {
            offscreen.0 = 0.0;
        }
        if offscreen.0 < cfg.despawn_offscreen_seconds {
            continue;
        }
        let distance = flat_distance(position.0, player.0);
        if distance > cfg.despawn_distance {
            commands.entity(entity).try_despawn();
            continue;
        }
        // Scared civilians and corpses are part of a scene the player made; only calm ones recycle.
        // Only from behind: a spawn off to the side would be recyclable again, and churn.
        let behind = (position.0 - player.0).with_y(0.0).dot(look) <= 0.0;
        if at_cap
            && behind
            && distance > cfg.recycle_distance
            && matches!(
                civilian.state,
                CivilianState::Wander | CivilianState::Idle { .. }
            )
        {
            recyclable.push((distance, entity));
        }
    }
    recyclable.sort_by(|a, b| b.0.total_cmp(&a.0));
    for &(_, entity) in recyclable.iter().take(cfg.recycles_per_tick as usize) {
        commands.entity(entity).try_despawn();
    }
}

/// A spawn point: node `from` itself, or the point at fraction `t` of edge `from -> to`.
pub(crate) struct SpawnPoint {
    pub(crate) from: u32,
    pub(crate) edge: Option<(u32, f32)>,
    pub(crate) at: Vec3,
}

/// Every node with an edge, plus points every `spacing` m inside each edge (none closer than
/// `spacing / 2` to an end).
pub(crate) fn spawn_points(
    graph: &SidewalkGraph,
    spacing: f32,
) -> impl Iterator<Item = SpawnPoint> + '_ {
    let nodes = (0..graph.nodes().len() as u32)
        .filter(|&n| !graph.neighbors(n).is_empty())
        .map(|n| SpawnPoint {
            from: n,
            edge: None,
            at: graph.node(n),
        });
    let inner = graph.edges().iter().flat_map(move |&(a, b)| {
        let (pa, pb) = (graph.node(a), graph.node(b));
        let length = flat_distance(pa, pb);
        let count = (length / spacing - 0.5).floor().max(0.0) as u32;
        (1..=count).map(move |k| {
            let t = k as f32 * spacing / length;
            SpawnPoint {
                from: a,
                edge: Some((b, t)),
                at: pa.lerp(pb, t),
            }
        })
    });
    nodes.chain(inner)
}

#[allow(clippy::too_many_arguments)]
fn spawn_civilians(
    mut commands: Commands,
    cfg: Res<PopulationConfig>,
    civilian_cfg: Res<CivilianConfig>,
    view: Res<CameraView>,
    mut phase: ResMut<PopulationPhase>,
    mut load: ResMut<PopulationLoad>,
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
    *load = PopulationLoad::default();
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
    // The one-shot fill stays uniform around the player; only the steady ring prefers the view.
    let forward_weight = if initial {
        0.0
    } else {
        cfg.spawn_forward_weight
    };
    let look = view.forward.with_y(0.0).normalize_or_zero();
    let mut candidates = spawn_points(&graph, cfg.spawn_point_spacing)
        .filter(|point| {
            let d = flat_distance(point.at, player.0);
            inner <= d && d <= outer
        })
        .filter(|point| {
            taken
                .iter()
                .all(|&p| flat_distance(p, point.at) >= separation)
        })
        .map(|point| {
            let ahead = (point.at - player.0)
                .with_y(0.0)
                .normalize_or_zero()
                .dot(look);
            (point, forward_weight * ahead + rng.unit())
        })
        .collect::<Vec<_>>();
    candidates.sort_by(|a, b| b.1.total_cmp(&a.1));
    let margin = cfg.spawn_view_margin_deg.to_radians();
    let wanted = budget.min(deficit) as usize;
    let mut spawned: Vec<Vec3> = Vec::new();
    let mut ray_starved = false;
    for (point, _) in candidates {
        if spawned.len() == wanted {
            break;
        }
        let at = point.at;
        if spawned.iter().any(|&p| flat_distance(p, at) < separation) {
            continue;
        }
        if !outside_cone(&view, at, loco.head_height, margin) {
            if load.rays + OCCLUSION_RAYS_PER_POINT > cfg.occlusion_rays_per_tick {
                ray_starved = true;
                continue;
            }
            if !occluded(
                &spatial,
                &view,
                at,
                cfg.occlusion_ray_height,
                loco.capsule_radius,
                &mut load.rays,
            ) {
                continue;
            }
        }
        let (walker, t) = match point.edge {
            None => (
                GraphWalker {
                    from: point.from,
                    to: wander_next(&graph, point.from, point.from, rng.unit()),
                },
                0.0,
            ),
            Some((to, t))
                if flat_distance(graph.node(to), player.0)
                    <= flat_distance(graph.node(point.from), player.0) =>
            {
                (
                    GraphWalker {
                        from: point.from,
                        to,
                    },
                    t,
                )
            }
            Some((to, t)) => (
                GraphWalker {
                    from: to,
                    to: point.from,
                },
                1.0 - t,
            ),
        };
        let temperament = roll_temperament(&mut rng, civilian_cfg.reaction.temperament_spread);
        let appearance = Appearance(rng.next_u32());
        commands.spawn(civilian_bundle(
            &loco,
            handle.0.clone(),
            &health,
            &graph,
            walker,
            t,
            temperament,
            appearance,
        ));
        spawned.push(at);
    }
    let exhausted = spawned.len() < budget as usize && !ray_starved;
    if initial && (exhausted || spawned.len() as u32 == deficit) {
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
