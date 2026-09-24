//! Gang territories from the generator: the gang of every city block and the posts groups stand at.

use super::{GangConfig, GroupConfig};
use crate::navigation::flat_distance;
use crate::world::{City, CityParamsRes};
use bevy::prelude::*;
use citygen::{
    BuildingKind, CityLayout, CityParams, Vec2 as LayoutVec2, contains_convex, dist_point_segment,
    sidewalk_anchor,
};

/// Posts of one gang; `posts[0]` is the HQ, y = sidewalk top.
#[derive(Reflect, Clone, Debug)]
pub struct Turf {
    pub posts: Vec<Vec3>,
}

/// One curb polygon (layout (x, z), counter-clockwise) and the gang it belongs to.
#[derive(Clone, Debug)]
struct TurfBlock {
    curb: Vec<LayoutVec2>,
    gang: Option<u8>,
}

/// The two gang territories (GDD §6.3); present from the first `Playing` of a city run.
#[derive(Resource, Reflect, Debug)]
#[reflect(Resource)]
pub struct GangTerritories {
    pub gangs: Vec<Turf>,
    #[reflect(ignore)]
    blocks: Vec<TurfBlock>,
}

fn signed_area(poly: &[LayoutVec2]) -> f32 {
    (0..poly.len())
        .map(|i| poly[i].perp_dot(poly[(i + 1) % poly.len()]))
        .sum::<f32>()
        / 2.0
}

impl GangTerritories {
    /// Every block needs ≥ 3 points in counter-clockwise order; every gang ≥ 1 post.
    pub fn new(
        gangs: Vec<Turf>,
        blocks: Vec<(Vec<LayoutVec2>, Option<u8>)>,
    ) -> Result<Self, String> {
        for (i, (curb, _)) in blocks.iter().enumerate() {
            if curb.len() < 3 || signed_area(curb) <= 0.0 {
                return Err(format!(
                    "block {i}: curb polygon must have >= 3 points in counter-clockwise order"
                ));
            }
        }
        if let Some(g) = gangs.iter().position(|t| t.posts.is_empty()) {
            return Err(format!("gang {g} has no post"));
        }
        Ok(Self {
            gangs,
            blocks: blocks
                .into_iter()
                .map(|(curb, gang)| TurfBlock { curb, gang })
                .collect(),
        })
    }

    /// Blocks and gangs of a generated city: the HQ post on the HQ's sidewalk, then every sidewalk
    /// node of the territory at least `post_spacing` from the posts kept before it.
    pub fn from_layout(
        layout: &CityLayout,
        params: &CityParams,
        groups: &GroupConfig,
    ) -> Result<Self, String> {
        let blocks = layout
            .blocks
            .iter()
            .filter(|b| b.curb.len() >= 3)
            .map(|b| {
                let gang = layout
                    .gang_districts
                    .iter()
                    .position(|&d| d == b.district)
                    .map(|g| g as u8);
                (b.curb.clone(), gang)
            })
            .collect();
        let mut territories = Self::new(
            vec![
                Turf {
                    posts: vec![Vec3::ZERO]
                };
                layout.gang_districts.len()
            ],
            blocks,
        )?;
        let y = params.roads.curb_height;
        let mut degree = vec![0u32; layout.sidewalks.nodes.len()];
        for &(a, b) in &layout.sidewalks.edges {
            degree[a as usize] += 1;
            degree[b as usize] += 1;
        }
        for g in 0..territories.gangs.len() {
            let hq = layout
                .buildings
                .iter()
                .position(|b| b.kind == BuildingKind::GangHq(g as u8))
                .ok_or_else(|| format!("no HQ for gang {g}"))?;
            let (anchor, _) =
                sidewalk_anchor(layout, params, hq, groups.hq_margin).ok_or_else(|| {
                    format!(
                        "gang {g} HQ {hq}: no sidewalk side of length >= {}",
                        2.0 * groups.hq_margin
                    )
                })?;
            let mut posts = vec![Vec3::new(anchor.x, y, anchor.y)];
            for (id, node) in layout.sidewalks.nodes.iter().enumerate() {
                let at = Vec3::new(node.x, y, node.y);
                if degree[id] == 0 || territories.territory_at(at) != Some(g as u8) {
                    continue;
                }
                if posts
                    .iter()
                    .all(|&p| flat_distance(p, at) >= groups.post_spacing)
                {
                    posts.push(at);
                }
            }
            territories.gangs[g].posts = posts;
        }
        Ok(territories)
    }

    /// Gang of the block containing `p`, else of the block whose curb is nearest (ties: lower index).
    pub fn territory_at(&self, p: Vec3) -> Option<u8> {
        let q = LayoutVec2::new(p.x, p.z);
        if let Some(block) = self
            .blocks
            .iter()
            .find(|b| contains_convex(&b.curb, q, 0.0))
        {
            return block.gang;
        }
        let mut best: Option<(f32, Option<u8>)> = None;
        for block in &self.blocks {
            let n = block.curb.len();
            let d = (0..n)
                .map(|i| dist_point_segment(q, block.curb[i], block.curb[(i + 1) % n]))
                .fold(f32::INFINITY, f32::min);
            if best.is_none_or(|(b, _)| d < b) {
                best = Some((d, block.gang));
            }
        }
        best.and_then(|(_, gang)| gang)
    }
}

pub(crate) fn build_gang_territories(
    mut commands: Commands,
    city: Res<City>,
    params: Res<CityParamsRes>,
    cfg: Res<GangConfig>,
    mut exit: MessageWriter<AppExit>,
) {
    match GangTerritories::from_layout(&city.0, &params.0, &cfg.groups) {
        Ok(territories) => commands.insert_resource(territories),
        Err(e) => {
            error!("gang territories invalid: {e}");
            exit.write(AppExit::error());
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn v(x: f32, z: f32) -> LayoutVec2 {
        LayoutVec2::new(x, z)
    }

    fn two_squares() -> GangTerritories {
        GangTerritories::new(
            vec![Turf {
                posts: vec![Vec3::ZERO],
            }],
            vec![
                (
                    vec![v(-40.0, -40.0), v(0.0, -40.0), v(0.0, 40.0), v(-40.0, 40.0)],
                    Some(0),
                ),
                (
                    vec![v(2.0, -40.0), v(40.0, -40.0), v(40.0, 40.0), v(2.0, 40.0)],
                    None,
                ),
            ],
        )
        .unwrap()
    }

    #[test]
    fn territory_is_the_containing_or_nearest_block() {
        let t = two_squares();
        assert_eq!(t.territory_at(Vec3::new(-10.0, 3.0, 5.0)), Some(0));
        assert_eq!(t.territory_at(Vec3::new(30.0, 0.0, 0.0)), None);
        // In the 2 m gap: 0.5 m from gang 0, 1.5 m from the neutral block.
        assert_eq!(t.territory_at(Vec3::new(0.5, 0.0, 0.0)), Some(0));
        assert_eq!(t.territory_at(Vec3::new(1.5, 0.0, 0.0)), None);
    }

    #[test]
    fn clockwise_block_is_rejected_naming_it() {
        let e = GangTerritories::new(
            vec![Turf {
                posts: vec![Vec3::ZERO],
            }],
            vec![(
                vec![v(0.0, 0.0), v(0.0, 10.0), v(10.0, 10.0), v(10.0, 0.0)],
                Some(0),
            )],
        )
        .unwrap_err();
        assert!(e.contains("block 0"), "{e}");
        let e = GangTerritories::new(vec![Turf { posts: vec![] }], vec![]).unwrap_err();
        assert!(e.contains("gang 0"), "{e}");
    }

    #[test]
    fn every_sweep_seed_has_two_territories() {
        let params: CityParams = ron::from_str(include_str!("../../../../assets/world/city.ron"))
            .expect("GATE BROKEN: world/city.ron does not parse");
        params
            .validate()
            .expect("GATE BROKEN: world/city.ron invalid");
        let cfg: GangConfig = ron::from_str(include_str!("../../../../assets/gang/gangs.ron"))
            .expect("GATE BROKEN: gang/gangs.ron does not parse");
        let groups = &cfg.groups;
        for seed in (0..32).chain([42]) {
            let layout = citygen::generate(seed, &params)
                .unwrap_or_else(|e| panic!("GATE BROKEN: seed {seed} generation failed: {e}"));
            let t = GangTerritories::from_layout(&layout, &params, groups)
                .unwrap_or_else(|e| panic!("seed {seed}: {e}"));
            assert_eq!(t.gangs.len(), 2, "seed {seed}");
            for (g, turf) in t.gangs.iter().enumerate() {
                assert!(
                    turf.posts.len() >= 2,
                    "seed {seed} gang {g}: {:?}",
                    turf.posts
                );
                for &post in &turf.posts {
                    assert_eq!(
                        t.territory_at(post),
                        Some(g as u8),
                        "seed {seed} gang {g}: post {post}"
                    );
                }
                for (i, &a) in turf.posts.iter().enumerate() {
                    for &b in &turf.posts[i + 1..] {
                        assert!(
                            flat_distance(a, b) >= groups.post_spacing,
                            "seed {seed} gang {g}: posts {a} {b}"
                        );
                    }
                }
                let hq = layout
                    .buildings
                    .iter()
                    .position(|b| b.kind == BuildingKind::GangHq(g as u8))
                    .unwrap();
                let (anchor, _) = sidewalk_anchor(&layout, &params, hq, groups.hq_margin).unwrap();
                let expected = Vec3::new(anchor.x, params.roads.curb_height, anchor.y);
                assert!(
                    turf.posts[0].distance(expected) < 0.01,
                    "seed {seed} gang {g}: HQ post {} vs {expected}",
                    turf.posts[0]
                );
            }
        }
    }
}
