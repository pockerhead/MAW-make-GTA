//! Sidewalk graph and graph steering for NPCs (GDD §6.2, §6.6).

use crate::combat::aim_yaw;
use crate::flow::{GameState, NEW_CITY, NpcSystems};
use crate::perception::{AiSystems, sight_blocked};
use crate::world::{City, CityParamsRes};
use avian3d::prelude::*;
use bevy::prelude::*;
use citygen::WalkGraph;
use serde::Deserialize;

/// Path of the navigation config, relative to the assets root.
pub const NAVIGATION_CONFIG: &str = "npc/navigation.ron";

#[derive(Resource, Deserialize, Clone, Debug)]
#[serde(deny_unknown_fields)]
pub struct NavigationConfig {
    /// A walker this close (horizontally) to its target node takes the next edge, m.
    pub arrive_radius: f32,
    /// Walkers keep this far right of the edge line, so opposing walkers pass without contact, m.
    pub keep_right: f32,
    /// Gangs and police seek a visible target straight within this, else they route (GDD §6.6), m.
    pub direct_seek_distance: f32,
    /// A* searches per fixed tick, at most (GDD §11).
    pub route_requests_per_tick: u32,
    /// A route to a moving goal is re-planned at most this often, s.
    pub route_refresh_seconds: f32,
    /// Length of the probe ray ahead of a direct seek, m.
    pub avoid_distance: f32,
    /// Detour headings are tried at ±1, ±2, ±3 of this step, degrees.
    pub avoid_step_deg: f32,
}

impl NavigationConfig {
    pub fn validate(&self) -> Result<(), String> {
        if !(self.arrive_radius.is_finite() && self.arrive_radius > 0.0) {
            return Err(format!(
                "arrive_radius {} must be finite and > 0",
                self.arrive_radius
            ));
        }
        if !(self.keep_right.is_finite() && self.keep_right >= 0.0) {
            return Err(format!(
                "keep_right {} must be finite and >= 0",
                self.keep_right
            ));
        }
        for (field, value) in [
            ("direct_seek_distance", self.direct_seek_distance),
            ("route_refresh_seconds", self.route_refresh_seconds),
            ("avoid_distance", self.avoid_distance),
            ("avoid_step_deg", self.avoid_step_deg),
        ] {
            if !(value.is_finite() && value > 0.0) {
                return Err(format!("{field} {value} must be finite and > 0"));
            }
        }
        if self.route_requests_per_tick < 1 {
            return Err("route_requests_per_tick must be >= 1".into());
        }
        // Three steps either way must stay within ±180°.
        if self.avoid_step_deg >= 60.0 {
            return Err(format!(
                "avoid_step_deg {} must be < 60",
                self.avoid_step_deg
            ));
        }
        Ok(())
    }
}

/// Walkable sidewalk graph in world space (node y = sidewalk top).
#[derive(Resource, Debug)]
pub struct SidewalkGraph {
    nodes: Vec<Vec3>,
    edges: Vec<(u32, u32)>,
    adjacency: Vec<Vec<u32>>,
}

impl SidewalkGraph {
    /// Node ids are kept; an out-of-range index or a self-loop is an error, a duplicate edge is kept once.
    pub fn new(nodes: Vec<Vec3>, edges: &[(u32, u32)]) -> Result<Self, String> {
        let mut adjacency = vec![Vec::new(); nodes.len()];
        let mut kept = Vec::with_capacity(edges.len());
        for &(a, b) in edges {
            if a == b || a as usize >= nodes.len() || b as usize >= nodes.len() {
                return Err(format!(
                    "edge ({a}, {b}) is a self-loop or names a node outside 0..{}",
                    nodes.len()
                ));
            }
            if adjacency[a as usize].contains(&b) {
                continue;
            }
            adjacency[a as usize].push(b);
            adjacency[b as usize].push(a);
            kept.push((a, b));
        }
        Ok(Self {
            nodes,
            edges: kept,
            adjacency,
        })
    }

    /// Layout point (x, y) becomes world (x, `y`, y).
    pub fn from_walk_graph(graph: &WalkGraph, y: f32) -> Result<Self, String> {
        let nodes = graph.nodes.iter().map(|p| Vec3::new(p.x, y, p.y)).collect();
        Self::new(nodes, &graph.edges)
    }

    pub fn node(&self, id: u32) -> Vec3 {
        self.nodes[id as usize]
    }

    pub fn nodes(&self) -> &[Vec3] {
        &self.nodes
    }

    pub fn neighbors(&self, id: u32) -> &[u32] {
        &self.adjacency[id as usize]
    }

    pub fn edges(&self) -> &[(u32, u32)] {
        &self.edges
    }

    pub fn is_edge(&self, a: u32, b: u32) -> bool {
        self.adjacency
            .get(a as usize)
            .is_some_and(|next| next.contains(&b))
    }
}

/// A walker on edge `from -> to`, heading for `node(to)`. Invariant: `(from, to)` is a graph edge.
#[derive(Component, Reflect, Clone, Copy, Debug, PartialEq)]
#[reflect(Component)]
pub struct GraphWalker {
    pub from: u32,
    pub to: u32,
}

fn flat(v: Vec3) -> Vec3 {
    Vec3::new(v.x, 0.0, v.z)
}

/// Horizontal distance.
pub fn flat_distance(a: Vec3, b: Vec3) -> f32 {
    flat(a - b).length()
}

/// Next node after arriving at `to`: uniform over the neighbours except `from`; `from` only on a dead end.
/// `u` is uniform in `[0, 1)`.
pub fn wander_next(graph: &SidewalkGraph, from: u32, to: u32, u: f32) -> u32 {
    let options = graph
        .neighbors(to)
        .iter()
        .copied()
        .filter(|&n| n != from)
        .collect::<Vec<_>>();
    if options.is_empty() {
        return from;
    }
    let pick = ((u * options.len() as f32) as usize).min(options.len() - 1);
    options[pick]
}

/// Neighbour of `to` farthest (horizontally) from `threat`; ties go to the lower node id.
pub fn flee_next(graph: &SidewalkGraph, to: u32, threat: Vec3) -> u32 {
    let mut best = to;
    let mut best_distance = f32::NEG_INFINITY;
    for &n in graph.neighbors(to) {
        let d = flat_distance(graph.node(n), threat);
        if d > best_distance || (d == best_distance && n < best) {
            best = n;
            best_distance = d;
        }
    }
    best
}

/// Keeps the heading if `node(to)` is farther from the threat than `node(from)`, else turns around.
pub fn flee_start(graph: &SidewalkGraph, walker: GraphWalker, threat: Vec3) -> GraphWalker {
    let ahead = flat_distance(graph.node(walker.to), threat);
    let behind = flat_distance(graph.node(walker.from), threat);
    if ahead > behind {
        return walker;
    }
    GraphWalker {
        from: walker.to,
        to: walker.from,
    }
}

/// Point a walker heads for: `node(to)` shifted `keep_right` to the right of the travel direction.
pub fn lane_target(graph: &SidewalkGraph, walker: GraphWalker, keep_right: f32) -> Vec3 {
    let to = graph.node(walker.to);
    let forward = flat(to - graph.node(walker.from)).normalize_or_zero();
    to + forward.cross(Vec3::Y) * keep_right
}

/// Move yaw (the `MoveIntent` convention) from `position` towards `target`; `None` when on top of it.
pub fn steer(position: Vec3, target: Vec3) -> Option<f32> {
    let d = flat(target - position);
    if d.length_squared() == 0.0 {
        return None;
    }
    Some(aim_yaw(d))
}

/// The node with at least one edge nearest (horizontally) to `p`; ties go to the lower id.
pub fn nearest_node(graph: &SidewalkGraph, p: Vec3) -> Option<u32> {
    let mut best: Option<(u32, f32)> = None;
    for id in 0..graph.nodes().len() as u32 {
        if graph.neighbors(id).is_empty() {
            continue;
        }
        let d = flat_distance(graph.node(id), p);
        if best.is_none_or(|(_, b)| d < b) {
            best = Some((id, d));
        }
    }
    best.map(|(id, _)| id)
}

/// Integer centimetres: `astar` needs an `Ord` cost.
fn cost_cm(a: Vec3, b: Vec3) -> u32 {
    (flat_distance(a, b) * 100.0) as u32
}

/// Shortest node path `from ..= to` over the graph edges (A*), `None` when unreachable.
pub fn find_route(graph: &SidewalkGraph, from: u32, to: u32) -> Option<Vec<u32>> {
    let goal = graph.node(to);
    pathfinding::prelude::astar(
        &from,
        |&n| {
            let at = graph.node(n);
            graph
                .neighbors(n)
                .iter()
                .map(move |&m| (m, cost_cm(at, graph.node(m))))
        },
        |&n| cost_cm(graph.node(n), goal),
        |&n| n == to,
    )
    .map(|(path, _)| path)
}

/// The current graph route of one NPC; mutated in place.
#[derive(Component, Reflect, Default, Clone, Debug)]
#[reflect(Component)]
pub struct Route {
    /// Node the route leads to.
    pub goal: Option<u32>,
    pub nodes: Vec<u32>,
    /// Index into `nodes` of the node being walked to.
    pub next: usize,
    /// Seconds since the route was planned.
    pub age: f32,
}

/// Plans a route from the node nearest to `from` to `goal`, dropping the first node when `from` is
/// already past it towards the second; `false` (no nodes) when unreachable.
pub fn plan_route(graph: &SidewalkGraph, route: &mut Route, from: Vec3, goal: u32) -> bool {
    route.goal = Some(goal);
    route.next = 0;
    route.age = 0.0;
    route.nodes.clear();
    let Some(start) = nearest_node(graph, from) else {
        return false;
    };
    let Some(mut path) = find_route(graph, start, goal) else {
        return false;
    };
    if path.len() >= 2 {
        let (first, second) = (graph.node(path[0]), graph.node(path[1]));
        if flat_distance(from, second) < flat_distance(first, second) {
            path.remove(0);
        }
    }
    route.nodes = path;
    true
}

/// Point to walk to: the next route node not yet reached (within `arrive_radius`), else `destination`.
pub fn route_point(
    graph: &SidewalkGraph,
    route: &mut Route,
    from: Vec3,
    destination: Vec3,
    arrive_radius: f32,
) -> Vec3 {
    while route.next < route.nodes.len()
        && flat_distance(from, graph.node(route.nodes[route.next])) <= arrive_radius
    {
        route.next += 1;
    }
    route
        .nodes
        .get(route.next)
        .map_or(destination, |&n| graph.node(n))
}

/// Flat unit vector of a move yaw (GDD §3.2 convention).
pub fn yaw_forward(yaw: f32) -> Vec3 {
    Vec3::new(-yaw.sin(), 0.0, -yaw.cos())
}

/// Yaw offset of the first clear heading among `0, +s, −s, +2s, −2s, +3s, −3s` (no World geometry
/// within `avoid_distance` of `chest`); 0 when all are blocked. Adds the rays cast to `rays`.
pub fn avoid_offset(
    spatial: &SpatialQuery,
    chest: Vec3,
    yaw: f32,
    cfg: &NavigationConfig,
    rays: &mut u32,
) -> f32 {
    let step = cfg.avoid_step_deg.to_radians();
    for k in [0.0, 1.0, -1.0, 2.0, -2.0, 3.0, -3.0] {
        let offset = k * step;
        *rays += 1;
        let probe = chest + yaw_forward(yaw + offset) * cfg.avoid_distance;
        if !sight_blocked(spatial, chest, probe) {
            return offset;
        }
    }
    0.0
}

/// Route searches and avoidance rays of the current fixed tick.
#[derive(Resource, Reflect, Default, Clone, Copy, Debug)]
#[reflect(Resource)]
pub struct RouteLoad {
    pub searches: u32,
    pub rays: u32,
}

pub struct NavigationPlugin;

impl Plugin for NavigationPlugin {
    fn build(&self, app: &mut App) {
        app.register_type::<GraphWalker>()
            .init_resource::<RouteLoad>()
            .register_type::<Route>()
            .register_type::<RouteLoad>()
            .add_systems(FixedUpdate, reset_route_load.in_set(AiSystems::Perceive))
            .configure_sets(
                FixedUpdate,
                NpcSystems.run_if(resource_exists::<SidewalkGraph>),
            )
            .add_systems(
                OnTransition {
                    exited: GameState::Loading,
                    entered: GameState::Playing,
                },
                build_sidewalk_graph.run_if(resource_exists::<City>),
            )
            .add_systems(NEW_CITY, drop_sidewalk_graph);
    }
}

fn drop_sidewalk_graph(mut commands: Commands) {
    commands.remove_resource::<SidewalkGraph>();
}

fn reset_route_load(mut load: ResMut<RouteLoad>) {
    *load = RouteLoad::default();
}

fn build_sidewalk_graph(
    mut commands: Commands,
    city: Res<City>,
    params: Res<CityParamsRes>,
    mut exit: MessageWriter<AppExit>,
) {
    match SidewalkGraph::from_walk_graph(&city.0.sidewalks, params.0.roads.curb_height) {
        Ok(graph) => commands.insert_resource(graph),
        Err(e) => {
            error!("sidewalk graph invalid: {e}");
            exit.write(AppExit::error());
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::character::move_direction;

    fn star() -> SidewalkGraph {
        SidewalkGraph::new(
            vec![
                Vec3::ZERO,
                Vec3::new(10.0, 0.0, 0.0),
                Vec3::new(-10.0, 0.0, 0.0),
                Vec3::new(0.0, 0.0, 10.0),
            ],
            &[(0, 1), (0, 2), (0, 3)],
        )
        .unwrap()
    }

    #[test]
    fn new_rejects_bad_edges_naming_them() {
        let nodes = vec![Vec3::ZERO, Vec3::X, Vec3::Z];
        let e = SidewalkGraph::new(nodes.clone(), &[(0, 1), (0, 0)]).unwrap_err();
        assert!(e.contains("(0, 0)"), "{e}");
        let e = SidewalkGraph::new(nodes.clone(), &[(0, 9)]).unwrap_err();
        assert!(e.contains("(0, 9)"), "{e}");
        let g = SidewalkGraph::new(nodes, &[(0, 1), (1, 0), (1, 2)]).unwrap();
        assert_eq!(g.edges(), &[(0, 1), (1, 2)]);
        assert_eq!(g.neighbors(1), &[0, 2]);
        assert!(g.is_edge(2, 1) && !g.is_edge(0, 2));
    }

    #[test]
    fn wander_never_turns_back_unless_dead_end() {
        let g = star();
        for k in 0..100 {
            let u = k as f32 / 100.0;
            let next = wander_next(&g, 1, 0, u);
            assert!(next == 2 || next == 3, "u={u}: {next}");
        }
        assert_eq!(wander_next(&g, 0, 1, 0.5), 0, "dead end turns back");
    }

    #[test]
    fn flee_picks_the_farthest_neighbour() {
        // Distances from (8, 0, 1): (10,0,0) 2.24, (-10,0,0) 18.03, (0,0,10) 12.04.
        let g = star();
        assert_eq!(flee_next(&g, 0, Vec3::new(8.0, 0.0, 1.0)), 2);
    }

    #[test]
    fn flee_start_turns_away_from_the_threat() {
        let g = star();
        let w = GraphWalker { from: 0, to: 1 };
        assert_eq!(flee_start(&g, w, Vec3::new(-20.0, 0.0, 0.0)), w);
        assert_eq!(
            flee_start(&g, w, Vec3::new(20.0, 0.0, 0.0)),
            GraphWalker { from: 1, to: 0 }
        );
    }

    #[test]
    fn lane_target_keeps_right() {
        let g =
            SidewalkGraph::new(vec![Vec3::ZERO, Vec3::new(0.0, 0.0, -10.0)], &[(0, 1)]).unwrap();
        let ahead = lane_target(&g, GraphWalker { from: 0, to: 1 }, 0.5);
        assert!(
            (ahead - Vec3::new(0.5, 0.0, -10.0)).length() < 1e-6,
            "{ahead}"
        );
        let back = lane_target(&g, GraphWalker { from: 1, to: 0 }, 0.5);
        assert!((back - Vec3::new(-0.5, 0.0, 0.0)).length() < 1e-6, "{back}");
    }

    /// n0 (0,0,0), n1 (0,0,−20), n2 (−20,0,−20), n3 (−20,0,0); edges 0-1, 1-2, 2-3, no 3-0.
    fn open_square() -> SidewalkGraph {
        SidewalkGraph::new(
            vec![
                Vec3::ZERO,
                Vec3::new(0.0, 0.0, -20.0),
                Vec3::new(-20.0, 0.0, -20.0),
                Vec3::new(-20.0, 0.0, 0.0),
            ],
            &[(0, 1), (1, 2), (2, 3)],
        )
        .unwrap()
    }

    fn same_yaw(a: f32, b: f32) -> bool {
        let diff = (a - b).rem_euclid(std::f32::consts::TAU);
        diff.min(std::f32::consts::TAU - diff) < 1e-5
    }

    #[test]
    fn route_goes_around_the_missing_edge() {
        assert_eq!(find_route(&open_square(), 0, 3), Some(vec![0, 1, 2, 3]));
    }

    #[test]
    fn route_turns_north_west_south() {
        let g = open_square();
        let mut route = Route::default();
        let destination = Vec3::new(-20.0, 0.0, 5.0);
        assert!(plan_route(&g, &mut route, Vec3::new(0.0, 0.0, 0.3), 3));
        assert_eq!(route.nodes, vec![0, 1, 2, 3], "n0 kept: 20.3 >= 20");
        let half = std::f32::consts::FRAC_PI_2;
        let pi = std::f32::consts::PI;
        for (at, yaw) in [
            (Vec3::new(0.0, 0.0, 0.3), 0.0),
            (Vec3::new(0.0, 0.0, -20.0), half),
            (Vec3::new(-20.0, 0.0, -20.0), pi),
            (Vec3::new(-20.0, 0.0, 0.0), pi),
        ] {
            let target = route_point(&g, &mut route, at, destination, 0.5);
            let got = steer(at, target).unwrap();
            assert!(same_yaw(got, yaw), "{at}: target {target}, yaw {got}");
        }
        assert_eq!(route.next, 4, "every node used");
    }

    #[test]
    fn plan_route_skips_a_passed_corner() {
        let g = open_square();
        let mut route = Route::default();
        assert!(plan_route(&g, &mut route, Vec3::new(0.0, 0.0, -5.0), 3));
        assert_eq!(route.nodes, vec![1, 2, 3], "15 m to n1 < 20 m n0-n1");
        assert_eq!(route.goal, Some(3));
    }

    #[test]
    fn no_route_on_a_disconnected_graph() {
        let g = SidewalkGraph::new(
            vec![
                Vec3::ZERO,
                Vec3::new(10.0, 0.0, 0.0),
                Vec3::new(50.0, 0.0, 0.0),
                Vec3::new(60.0, 0.0, 0.0),
            ],
            &[(0, 1), (2, 3)],
        )
        .unwrap();
        assert_eq!(find_route(&g, 0, 3), None);
        let mut route = Route {
            nodes: vec![7, 8],
            ..default()
        };
        assert!(!plan_route(&g, &mut route, Vec3::ZERO, 3));
        assert!(route.nodes.is_empty());
        assert_eq!(route.goal, Some(3));
    }

    #[test]
    fn nearest_node_prefers_the_lower_id_on_a_tie() {
        // n0 has no edge and sits on the query point; n1 and n2 are both 5 m away.
        let g = SidewalkGraph::new(
            vec![
                Vec3::ZERO,
                Vec3::new(5.0, 0.0, 0.0),
                Vec3::new(-5.0, 0.0, 0.0),
            ],
            &[(1, 2)],
        )
        .unwrap();
        assert_eq!(nearest_node(&g, Vec3::ZERO), Some(1));
        assert_eq!(nearest_node(&g, Vec3::new(-4.0, 3.0, 0.0)), Some(2));
        let edgeless = SidewalkGraph::new(vec![Vec3::ZERO], &[]).unwrap();
        assert_eq!(nearest_node(&edgeless, Vec3::ZERO), None);
    }

    #[test]
    fn steer_matches_move_direction() {
        let half = std::f32::consts::FRAC_PI_2;
        for (target, yaw, direction) in [
            (Vec3::NEG_Z, 0.0, Vec3::NEG_Z),
            (Vec3::NEG_X, half, Vec3::NEG_X),
            (Vec3::Z, std::f32::consts::PI, Vec3::Z),
            (Vec3::X, -half, Vec3::X),
        ] {
            let got = steer(Vec3::ZERO, target * 5.0).unwrap();
            let diff = (got - yaw).rem_euclid(std::f32::consts::TAU);
            assert!(
                diff.min(std::f32::consts::TAU - diff) < 1e-5,
                "{target}: {got}"
            );
            let d = move_direction(Vec2::Y, got);
            assert!((d - direction).length() < 1e-5, "{target}: {d}");
        }
        assert_eq!(steer(Vec3::ONE, Vec3::new(1.0, 5.0, 1.0)), None);
    }
}
