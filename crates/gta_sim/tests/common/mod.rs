#![allow(dead_code)]

use avian3d::prelude::*;
use bevy::{
    asset::AssetPlugin,
    ecs::{message::MessageCursor, query::QueryFilter, system::RunSystemOnce},
    prelude::*,
    state::app::StatesPlugin,
    time::TimeUpdateStrategy,
};
use gta_sim::{
    character::{
        ActionIntent, AimIntent, CharacterControlConfig, Health, HealthConfig, LocomotionConfig,
        MoveIntent,
    },
    civilian::{Civilian, CivilianState, Temperament, civilian_bundle},
    combat::{
        BulletTrace, DamageDealt, GunSlot, Loadout, ShotFired, Weapon, WeaponsConfig, dummy_bundle,
    },
    compose_sim,
    config::ConfigRoot,
    flow::{GameState, WastedPhase},
    gang::{
        Faction, GangConfig, GangHeat, GangMember, GangState, GangTerritories, Turf,
        gang_member_bundle,
    },
    layers::GameLayer,
    navigation::{GraphWalker, SidewalkGraph},
    player::{DebugDamage, Player},
    population::{Appearance, CameraView, PopulationConfig, ViewCone},
    world::{CityParams, CityParamsRes, PlayerSpawn, WorldSource},
};
use std::{
    path::Path,
    time::{Duration, Instant},
};

pub fn assets_root() -> ConfigRoot {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../assets");
    ConfigRoot(
        path.canonicalize()
            .unwrap_or_else(|_| panic!("GATE BROKEN: assets root not found at {}", path.display())),
    )
}

pub fn composed_app(source: WorldSource) -> App {
    let mut app = App::new();
    app.add_plugins((
        MinimalPlugins,
        TransformPlugin,
        AssetPlugin::default(),
        StatesPlugin,
    ))
    .insert_resource(TimeUpdateStrategy::FixedTimesteps(1));
    compose_sim(&mut app, assets_root(), source).expect("GATE BROKEN: compose_sim failed");
    app.finish();
    app.cleanup();
    app
}

pub fn headless_app() -> App {
    composed_app(WorldSource::TestArea)
}

/// Headless app with a generated city, updated until `GameState::Playing`.
pub fn city_app(seed: u64) -> App {
    let mut app = composed_app(WorldSource::City { seed });
    let deadline = Instant::now() + Duration::from_secs(120);
    while *app.world().resource::<State<GameState>>().get() != GameState::Playing {
        app.update();
        if let Some(exit) = app.should_exit() {
            panic!("city generation failed: {exit:?}");
        }
        assert!(
            Instant::now() < deadline,
            "city generation did not finish in 120 s"
        );
        std::thread::sleep(Duration::from_millis(1));
    }
    app
}

/// Golden layout hash of `seed` from citygen's `golden_hashes.txt`; line-ending independent.
pub fn golden(seed: u64) -> u64 {
    include_str!("../../../citygen/tests/golden_hashes.txt")
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty() && !line.starts_with('#'))
        .filter_map(|line| line.split_once(' '))
        .find(|(s, _)| s.parse() == Ok(seed))
        .map(|(_, hash)| {
            u64::from_str_radix(hash.trim().trim_start_matches("0x"), 16)
                .unwrap_or_else(|_| panic!("GATE BROKEN: bad golden hash for seed {seed}"))
        })
        .unwrap_or_else(|| panic!("GATE BROKEN: no golden for seed {seed}"))
}

pub fn city_params(app: &App) -> &CityParams {
    &app.world().resource::<CityParamsRes>().0
}

pub fn place_player(app: &mut App, at: Vec3) {
    let entity = player(app);
    app.world_mut().get_mut::<Position>(entity).unwrap().0 = at;
    app.world_mut()
        .get_mut::<Transform>(entity)
        .unwrap()
        .translation = at;
}

pub fn run_ticks(app: &mut App, count: u32) {
    let initial = app.world().resource::<Time<Fixed>>().elapsed();
    let step = app.world().resource::<Time<Fixed>>().timestep();
    for _ in 0..count + 4 {
        if app.world().resource::<Time<Fixed>>().elapsed() - initial >= step * count {
            return;
        }
        app.update();
    }
    panic!("GATE BROKEN: fixed loop stalled");
}

pub fn player(app: &mut App) -> Entity {
    app.world_mut()
        .query_filtered::<Entity, With<Player>>()
        .single(app.world())
        .expect("GATE BROKEN: expected exactly one Player")
}

pub fn position(app: &mut App) -> Vec3 {
    let entity = player(app);
    app.world()
        .get::<Position>(entity)
        .expect("GATE BROKEN: player missing Position")
        .0
}

pub fn settle(app: &mut App) {
    run_ticks(app, 64);
    let y = position(app).y;
    let expected = app.world().resource::<PlayerSpawn>().0.y
        + app.world().resource::<LocomotionConfig>().float_height;
    assert!(
        (y - expected).abs() < 0.05,
        "GATE BROKEN: player not resting on floor at spawn, y={y}"
    );
}

pub fn set_intent(app: &mut App, update: impl FnOnce(&mut MoveIntent)) {
    let entity = player(app);
    update(
        app.world_mut()
            .get_mut::<MoveIntent>(entity)
            .expect("GATE BROKEN: missing MoveIntent")
            .as_mut(),
    );
}

pub fn health(app: &mut App) -> Health {
    let entity = player(app);
    *app.world()
        .get::<Health>(entity)
        .expect("GATE BROKEN: player missing Health")
}

pub fn set_health(app: &mut App, update: impl FnOnce(&mut Health)) {
    let entity = player(app);
    update(
        app.world_mut()
            .get_mut::<Health>(entity)
            .expect("GATE BROKEN: player missing Health")
            .as_mut(),
    );
}

pub fn write_damage(app: &mut App, amount: f32) {
    app.world_mut().write_message(DebugDamage { amount });
}

pub fn game_state(app: &App) -> GameState {
    app.world().resource::<State<GameState>>().get().clone()
}

pub fn wasted_phase(app: &App) -> Option<WastedPhase> {
    app.world()
        .get_resource::<State<WastedPhase>>()
        .map(|s| s.get().clone())
}

pub fn count<F: QueryFilter>(app: &mut App) -> usize {
    app.world_mut()
        .query_filtered::<(), F>()
        .iter(app.world())
        .count()
}

/// Aims the player from `origin` at `target`.
pub fn set_aim(app: &mut App, origin: Vec3, target: Vec3) {
    let entity = player(app);
    let mut aim = app
        .world_mut()
        .get_mut::<AimIntent>(entity)
        .expect("GATE BROKEN: player missing AimIntent");
    aim.origin = origin;
    aim.direction = (target - origin).normalize();
}

pub fn set_action(app: &mut App, update: impl FnOnce(&mut ActionIntent)) {
    let entity = player(app);
    update(
        app.world_mut()
            .get_mut::<ActionIntent>(entity)
            .expect("GATE BROKEN: player missing ActionIntent")
            .as_mut(),
    );
}

pub fn loadout(app: &mut App) -> Loadout {
    let entity = player(app);
    app.world()
        .get::<Loadout>(entity)
        .expect("GATE BROKEN: player missing Loadout")
        .clone()
}

pub fn set_loadout(app: &mut App, update: impl FnOnce(&mut Loadout)) {
    let entity = player(app);
    update(
        app.world_mut()
            .get_mut::<Loadout>(entity)
            .expect("GATE BROKEN: player missing Loadout")
            .as_mut(),
    );
}

/// A target dummy built by the production bundle, feet at `feet`.
pub fn spawn_dummy(app: &mut App, feet: Vec3) -> Entity {
    let world = app.world();
    let loco = world.resource::<LocomotionConfig>().clone();
    let health = world.resource::<HealthConfig>().clone();
    let handle = world.resource::<CharacterControlConfig>().0.clone();
    app.world_mut()
        .spawn(dummy_bundle(&loco, handle, &health, feet))
        .id()
}

pub fn health_of(app: &App, entity: Entity) -> Health {
    *app.world()
        .get::<Health>(entity)
        .expect("GATE BROKEN: target missing Health")
}

pub fn set_health_of(app: &mut App, entity: Entity, update: impl FnOnce(&mut Health)) {
    update(
        app.world_mut()
            .get_mut::<Health>(entity)
            .expect("GATE BROKEN: target missing Health")
            .as_mut(),
    );
}

/// A static cuboid fixture.
pub fn spawn_wall(app: &mut App, center: Vec3, size: Vec3) -> Entity {
    app.world_mut()
        .spawn((
            RigidBody::Static,
            Collider::cuboid(size.x, size.y, size.z),
            Transform::from_translation(center),
        ))
        .id()
}

/// Weapon messages written while ticking through `Shots::run`, each read exactly once.
pub struct Shots {
    fired: MessageCursor<ShotFired>,
    traces: MessageCursor<BulletTrace>,
    dealt: MessageCursor<DamageDealt>,
    pub shots: Vec<ShotFired>,
    pub trace_log: Vec<BulletTrace>,
    pub dealt_log: Vec<DamageDealt>,
}

impl Shots {
    pub fn new(app: &App) -> Self {
        let world = app.world();
        Self {
            fired: world.resource::<Messages<ShotFired>>().get_cursor_current(),
            traces: world
                .resource::<Messages<BulletTrace>>()
                .get_cursor_current(),
            dealt: world
                .resource::<Messages<DamageDealt>>()
                .get_cursor_current(),
            shots: Vec::new(),
            trace_log: Vec::new(),
            dealt_log: Vec::new(),
        }
    }

    /// Runs `ticks` fixed ticks one at a time, reading the messages after each.
    pub fn run(&mut self, app: &mut App, ticks: u32) {
        for _ in 0..ticks {
            run_ticks(app, 1);
            let world = app.world();
            self.shots.extend(
                self.fired
                    .read(world.resource::<Messages<ShotFired>>())
                    .copied(),
            );
            self.trace_log.extend(
                self.traces
                    .read(world.resource::<Messages<BulletTrace>>())
                    .copied(),
            );
            self.dealt_log.extend(
                self.dealt
                    .read(world.resource::<Messages<DamageDealt>>())
                    .copied(),
            );
        }
    }

    /// Clears the logs, keeping the cursors.
    pub fn clear(&mut self) {
        self.shots.clear();
        self.trace_log.clear();
        self.dealt_log.clear();
    }
}

/// Replaces the sidewalk graph with a test graph.
pub fn test_graph(app: &mut App, nodes: Vec<Vec3>, edges: &[(u32, u32)]) {
    let graph = SidewalkGraph::new(nodes, edges).expect("GATE BROKEN: invalid test graph");
    app.world_mut().insert_resource(graph);
}

/// A wandering civilian built by the production bundle on `walker`'s edge at fraction `t`.
pub fn spawn_civilian(
    app: &mut App,
    walker: GraphWalker,
    t: f32,
    temperament: Temperament,
) -> Entity {
    let world = app.world();
    let loco = world.resource::<LocomotionConfig>().clone();
    let health = world.resource::<HealthConfig>().clone();
    let handle = world.resource::<CharacterControlConfig>().0.clone();
    let graph = app
        .world_mut()
        .remove_resource::<SidewalkGraph>()
        .expect("GATE BROKEN: no SidewalkGraph");
    let bundle = civilian_bundle(
        &loco,
        handle,
        &health,
        &graph,
        walker,
        t,
        temperament,
        Appearance(0),
    );
    let entity = app.world_mut().spawn(bundle).id();
    app.world_mut().insert_resource(graph);
    entity
}

pub fn calm() -> Temperament {
    Temperament {
        flee: 1.0,
        cower: 1.0,
        report: 1.0,
    }
}

pub fn civilian_state(app: &App, entity: Entity) -> CivilianState {
    app.world()
        .get::<Civilian>(entity)
        .expect("GATE BROKEN: civilian missing")
        .state
}

pub fn set_civilian_state(app: &mut App, entity: Entity, state: CivilianState) {
    app.world_mut()
        .get_mut::<Civilian>(entity)
        .expect("GATE BROKEN: civilian missing")
        .state = state;
}

pub fn set_view(app: &mut App, view: Option<ViewCone>) {
    app.world_mut().resource_mut::<CameraView>().0 = view;
}

pub fn set_population(app: &mut App, update: impl FnOnce(&mut PopulationConfig)) {
    update(app.world_mut().resource_mut::<PopulationConfig>().as_mut());
}

pub fn civilians(app: &mut App) -> Vec<Entity> {
    app.world_mut()
        .query_filtered::<Entity, With<Civilian>>()
        .iter(app.world())
        .collect()
}

pub fn position_of(app: &App, entity: Entity) -> Vec3 {
    app.world()
        .get::<Position>(entity)
        .expect("GATE BROKEN: entity missing Position")
        .0
}

/// The orbit camera at rest (`camera.ron`: distance 3.8 m, pivot 1.55 m, fov 70), looking along the
/// flat `dir`, 16:9.
pub fn chase_view(feet: Vec3, dir: Vec3) -> ViewCone {
    ViewCone::from_perspective(
        feet + Vec3::Y * 1.55 - dir * 3.8,
        Dir3::new(dir).unwrap(),
        70f32.to_radians(),
        16.0 / 9.0,
    )
}

/// The flat direction from `from` with the longest clear run for a character, and its length.
pub fn open_street(app: &mut App, from: Vec3) -> (Vec3, f32) {
    app.world_mut()
        .run_system_once(move |spatial: SpatialQuery| {
            let shape = Collider::sphere(0.6);
            let filter = SpatialQueryFilter::from_mask(GameLayer::World);
            (0..360)
                .map(|deg| {
                    let yaw = (deg as f32).to_radians();
                    let dir = Vec3::new(-yaw.sin(), 0.0, -yaw.cos());
                    let length = spatial
                        .cast_shape(
                            &shape,
                            from + Vec3::Y,
                            Quat::IDENTITY,
                            Dir3::new(dir).unwrap(),
                            &ShapeCastConfig::from_max_distance(1200.0),
                            &filter,
                        )
                        .map_or(1200.0, |hit| hit.distance);
                    (dir, length)
                })
                .max_by(|a, b| a.1.total_cmp(&b.1))
                .unwrap()
        })
        .expect("GATE BROKEN: cast system failed")
}

/// Gang turf on the 80 x 80 m test floor.
#[derive(Clone, Copy, Debug)]
pub enum TurfLayout {
    /// Gang 0 owns x in [-40, 0]; x in [2, 40] belongs to no gang.
    WestHalf,
    /// Gang 0 owns the whole floor.
    WholeFloor,
}

fn square(x0: f32, x1: f32) -> Vec<citygen::Vec2> {
    [(x0, -40.0), (x1, -40.0), (x1, 40.0), (x0, 40.0)]
        .map(|(x, z)| citygen::Vec2::new(x, z))
        .to_vec()
}

/// Test floor with synthetic gang territories (gang posts (-30,0,-30) / (30,0,30)), the sidewalk
/// graph `nodes`/`edges`, and the player settled at the origin holding a pistol with a full magazine.
pub fn gang_floor(turf: TurfLayout, nodes: Vec<Vec3>, edges: &[(u32, u32)]) -> App {
    let mut app = headless_app();
    test_graph(&mut app, nodes, edges);
    let blocks = match turf {
        TurfLayout::WestHalf => vec![(square(-40.0, 0.0), Some(0)), (square(2.0, 40.0), None)],
        TurfLayout::WholeFloor => vec![(square(-40.0, 40.0), Some(0))],
    };
    let gangs = [Vec3::new(-30.0, 0.0, -30.0), Vec3::new(30.0, 0.0, 30.0)]
        .map(|post| Turf { posts: vec![post] })
        .to_vec();
    let territories =
        GangTerritories::new(gangs, blocks).expect("GATE BROKEN: invalid test territories");
    app.world_mut().insert_resource(territories);
    settle(&mut app);
    let size = app.world().resource::<WeaponsConfig>().pistol.magazine;
    set_loadout(&mut app, |l| {
        l.held = Some(Weapon::Pistol);
        l.guns[Weapon::Pistol.index()] = GunSlot {
            owned: true,
            magazine: size,
            reserve: 0,
            ..default()
        };
    });
    app
}

/// `gang_floor` with a graph far from every fixture: (30,0,30) - (35,0,30).
pub fn gang_floor_default(turf: TurfLayout) -> App {
    gang_floor(
        turf,
        vec![Vec3::new(30.0, 0.0, 30.0), Vec3::new(35.0, 0.0, 30.0)],
        &[(0, 1)],
    )
}

/// An idle gang member built by the production bundle (post 0, facing -Z), feet at `spot`.
pub fn spawn_member(app: &mut App, gang: u8, spot: Vec3, gun: Weapon) -> Entity {
    let world = app.world();
    let loco = world.resource::<LocomotionConfig>().clone();
    let health = world.resource::<HealthConfig>().clone();
    let weapons = world.resource::<WeaponsConfig>().clone();
    let handle = world.resource::<CharacterControlConfig>().0.clone();
    app.world_mut()
        .spawn(gang_member_bundle(
            &loco,
            handle,
            &health,
            &weapons,
            gang,
            0,
            spot,
            0.0,
            gun,
            Appearance(0),
        ))
        .id()
}

pub fn member(app: &App, entity: Entity) -> GangMember {
    app.world()
        .get::<GangMember>(entity)
        .expect("GATE BROKEN: gang member missing")
        .clone()
}

pub fn gang_state(app: &App, entity: Entity) -> GangState {
    member(app, entity).state
}

pub fn heat(app: &App, gang: usize) -> f32 {
    app.world().resource::<GangHeat>().left[gang]
}

pub fn gang_cfg(app: &App) -> GangConfig {
    app.world().resource::<GangConfig>().clone()
}

/// Named test mutation: the member attacks the player as if just provoked, the gang heated.
pub fn provoke(app: &mut App, entity: Entity) {
    let target = player(app);
    let at = position(app);
    let seconds = gang_cfg(app).hostility.heat_seconds;
    let mut member = app
        .world_mut()
        .get_mut::<GangMember>(entity)
        .expect("GATE BROKEN: gang member missing");
    member.state = GangState::Attack { target };
    member.last_seen = at;
    let gang = member.gang as usize;
    app.world_mut().resource_mut::<GangHeat>().left[gang] = seconds;
}

pub fn set_player_armor(app: &mut App, armor: f32) {
    set_health(app, |h| h.armor = armor);
}

/// Named test mutation: flips one pair of the faction matrix.
pub fn set_matrix(app: &mut App, a: Faction, b: Faction, hostile: bool) {
    let mut cfg = app.world_mut().resource_mut::<GangConfig>();
    let pair = cfg
        .factions
        .iter_mut()
        .find(|p| (p.a, p.b) == (a, b) || (p.a, p.b) == (b, a))
        .expect("GATE BROKEN: faction pair not in the matrix");
    pair.hostile = hostile;
}
