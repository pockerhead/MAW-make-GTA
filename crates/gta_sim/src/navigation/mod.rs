//! Sidewalk graph and graph steering for NPCs (GDD §6.2, §6.6).

use crate::combat::aim_yaw;
use crate::flow::{GameState, NpcSystems};
use crate::world::{City, CityParamsRes};
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

pub struct NavigationPlugin;

impl Plugin for NavigationPlugin {
    fn build(&self, app: &mut App) {
        app.register_type::<GraphWalker>()
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
            );
    }
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
