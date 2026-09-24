use super::config::RenderConfig;
use bevy::{camera::visibility::VisibilityRange, gltf::GltfAssetLabel, prelude::*};
use gta_sim::world::{
    BuildingKind, CityLayout, CityParams, CityScoped, DistrictKind, RoadClass, contains_convex,
};
use std::collections::HashMap;

/// Visual-only city prop kinds, in `PropsConfig::models` order.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum PropKind {
    Lamp,
    TrafficLight,
    TreeLarge,
    TreeSmall,
    ContainerA,
    ContainerC,
}

#[derive(Clone, Copy, Debug)]
pub(super) struct PropPlacement {
    pub kind: PropKind,
    pub position: Vec3,
    pub yaw: f32,
}

/// Marker of a spawned prop (visual only, no collider until T14).
#[derive(Component, Reflect)]
#[reflect(Component)]
#[require(CityScoped)]
pub struct CityProp;

#[derive(Resource)]
pub(super) struct PropAssets {
    meshes: [Handle<Mesh>; 6],
    materials: [Handle<StandardMaterial>; 6],
    scales: [f32; 6],
}

pub(super) fn load_prop_assets(
    mut commands: Commands,
    config: Res<RenderConfig>,
    asset_server: Res<AssetServer>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    let props = &config.props;
    let mut by_texture = HashMap::new();
    let models = props.models();
    let meshes = models.map(|m| {
        asset_server.load::<Mesh>(
            GltfAssetLabel::Primitive {
                mesh: 0,
                primitive: 0,
            }
            .from_asset(m.model.clone()),
        )
    });
    let materials = models.map(|m| {
        by_texture
            .entry(m.texture.clone())
            .or_insert_with(|| {
                materials.add(StandardMaterial {
                    base_color_texture: Some(asset_server.load(m.texture.clone())),
                    perceptual_roughness: props.roughness,
                    ..default()
                })
            })
            .clone()
    });
    commands.insert_resource(PropAssets {
        meshes,
        materials,
        scales: models.map(|m| m.scale),
    });
}

/// Spawn bundle of one prop; the fade-out range lives on the mesh entity itself.
pub(super) fn prop_bundle(
    assets: &PropAssets,
    config: &RenderConfig,
    prop: PropPlacement,
) -> impl Bundle {
    let i = prop.kind as usize;
    let range = config.props.visibility_range;
    (
        CityProp,
        Mesh3d(assets.meshes[i].clone()),
        MeshMaterial3d(assets.materials[i].clone()),
        Transform::from_translation(prop.position)
            .with_rotation(Quat::from_rotation_y(prop.yaw))
            .with_scale(Vec3::splat(assets.scales[i])),
        VisibilityRange {
            start_margin: 0.0..0.0,
            end_margin: (range - config.props.fade)..range,
            use_aabb: false,
        },
    )
}

fn splitmix64(mut x: u64) -> u64 {
    x = x.wrapping_add(0x9e37_79b9_7f4a_7c15);
    x = (x ^ (x >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
    x = (x ^ (x >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
    x ^ (x >> 31)
}

fn hash(seed: u64, a: u64, b: u64, c: u64) -> u64 {
    splitmix64(splitmix64(splitmix64(splitmix64(seed) ^ a) ^ b) ^ c)
}

fn unit(h: u64) -> f32 {
    (h >> 40) as f32 / (1u64 << 24) as f32
}

/// Yaw that turns the model's -Z towards `o` (layout x, y = world x, z).
fn yaw_facing_neg_z(o: Vec2) -> f32 {
    f32::atan2(-o.x, -o.y)
}

/// Yaw that lays the model's +Z along `u`.
fn yaw_along_z(u: Vec2) -> f32 {
    f32::atan2(u.x, u.y)
}

/// Lamp positions along a curb edge of length `length`: `t_j` from the edge start.
fn lamp_stations(length: f32, clearance: f32, spacing: f32) -> Vec<f32> {
    let usable = length - 2.0 * clearance;
    if usable < 0.0 {
        return Vec::new();
    }
    let count = (usable / spacing).floor() as u32 + 1;
    let first = clearance + (usable - (count - 1) as f32 * spacing) / 2.0;
    (0..count).map(|j| first + j as f32 * spacing).collect()
}

/// Deterministic prop placement for a city layout (pure function).
pub(super) fn place_props(
    layout: &CityLayout,
    params: &CityParams,
    config: &RenderConfig,
) -> Vec<PropPlacement> {
    let p = &config.props;
    let y = params.roads.curb_height;
    let seed = layout.seed;
    let at = |v: Vec2| Vec3::new(v.x, y, v.y);
    let tree = |h: u64| {
        if h & 1 == 0 {
            PropKind::TreeLarge
        } else {
            PropKind::TreeSmall
        }
    };
    let mut out = Vec::new();
    for (id, block) in layout.blocks.iter().enumerate() {
        let n = block.curb.len();
        if n < 3 {
            continue;
        }
        let residential =
            layout.districts[block.district as usize].kind == DistrictKind::Residential;
        let class = |k: usize| layout.roads.edges[block.sides[k] as usize].class;
        for k in 0..n {
            if class(k) == RoadClass::Alley {
                continue;
            }
            let (a, b) = (block.curb[k], block.curb[(k + 1) % n]);
            let d = (b - a).normalize();
            let inward = d.perp();
            let stations = lamp_stations((b - a).length(), p.lamp_corner_clearance, p.lamp_spacing);
            for &t in &stations {
                out.push(PropPlacement {
                    kind: PropKind::Lamp,
                    position: at(a + d * t + inward * p.lamp_curb_offset),
                    yaw: yaw_facing_neg_z(-inward),
                });
            }
            if residential || block.is_park {
                for (j, pair) in stations.windows(2).enumerate() {
                    let t = (pair[0] + pair[1]) / 2.0;
                    out.push(PropPlacement {
                        kind: tree(hash(seed, id as u64, k as u64, j as u64)),
                        position: at(a + d * t + inward * p.tree_curb_offset),
                        yaw: 0.0,
                    });
                }
            }
            let prev = (k + n - 1) % n;
            if class(prev) == RoadClass::Avenue && class(k) == RoadClass::Avenue {
                let inward_prev = (a - block.curb[prev]).normalize().perp();
                let bisector = (inward_prev + inward).normalize();
                out.push(PropPlacement {
                    kind: PropKind::TrafficLight,
                    position: at(a + bisector * p.traffic_light_offset),
                    yaw: yaw_facing_neg_z(-bisector),
                });
            }
        }
        if block.is_park && !block.inner.is_empty() {
            park_trees(&mut out, &block.inner, id as u64, seed, config, y);
        }
    }
    containers(&mut out, layout, config, y);
    out
}

fn park_trees(
    out: &mut Vec<PropPlacement>,
    inner: &[Vec2],
    block: u64,
    seed: u64,
    config: &RenderConfig,
    y: f32,
) {
    let p = &config.props;
    let (lo, hi) = inner.iter().fold(
        (Vec2::splat(f32::INFINITY), Vec2::splat(f32::NEG_INFINITY)),
        |(lo, hi), &v| (lo.min(v), hi.max(v)),
    );
    let (i0, i1) = (
        (lo.x / p.park_tree_spacing).floor() as i64,
        (hi.x / p.park_tree_spacing).ceil() as i64,
    );
    let (j0, j1) = (
        (lo.y / p.park_tree_spacing).floor() as i64,
        (hi.y / p.park_tree_spacing).ceil() as i64,
    );
    let n = inner.len();
    for i in i0..=i1 {
        for j in j0..=j1 {
            let h = hash(seed, block ^ 0x5041_524b, i as u64, j as u64);
            let jitter = Vec2::new(unit(h), unit(splitmix64(h))) * 2.0 - 1.0;
            let point =
                Vec2::new(i as f32, j as f32) * p.park_tree_spacing + jitter * p.park_tree_jitter;
            let inside = (0..n).all(|k| {
                let (a, b) = (inner[k], inner[(k + 1) % n]);
                (point - a).dot((b - a).normalize().perp()) >= p.park_tree_margin
            });
            if inside {
                let kind = if h & 2 == 0 {
                    PropKind::TreeLarge
                } else {
                    PropKind::TreeSmall
                };
                out.push(PropPlacement {
                    kind,
                    position: Vec3::new(point.x, y, point.y),
                    yaw: 0.0,
                });
            }
        }
    }
}

/// A row of containers behind every generic industrial building, kept only where it fits the lot.
fn containers(out: &mut Vec<PropPlacement>, layout: &CityLayout, config: &RenderConfig, y: f32) {
    let p = &config.props;
    let scale = p.container_a.scale.max(p.container_c.scale);
    let (w, len) = (
        p.container_raw_size.0 * scale,
        p.container_raw_size.1 * scale,
    );
    for (index, b) in layout.buildings.iter().enumerate() {
        let lot = &layout.lots[b.lot as usize];
        let district = layout.districts[layout.blocks[lot.block as usize].district as usize].kind;
        if b.kind != BuildingKind::Generic || district != DistrictKind::Industrial {
            continue;
        }
        let (u, v) = (b.axis, b.axis.perp());
        let row = b.center + v * (b.half_extents.y + p.container_gap + w / 2.0);
        let pitch = len + p.container_gap;
        let count = (2.0 * b.half_extents.x / pitch).floor() as u32;
        for i in 0..count {
            let c = row + u * ((i as f32 - (count - 1) as f32 / 2.0) * pitch);
            let fits = [(1.0, 1.0), (1.0, -1.0), (-1.0, 1.0), (-1.0, -1.0)]
                .into_iter()
                .all(|(su, sv)| {
                    contains_convex(
                        &lot.polygon,
                        c + u * (su * len / 2.0) + v * (sv * w / 2.0),
                        0.0,
                    )
                });
            if !fits {
                continue;
            }
            let h = hash(layout.seed, 0x434f_4e54 ^ index as u64, i as u64, 0);
            out.push(PropPlacement {
                kind: if h & 1 == 0 {
                    PropKind::ContainerA
                } else {
                    PropKind::ContainerC
                },
                position: Vec3::new(c.x, y, c.y),
                yaw: yaw_along_z(u),
            });
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn close(a: Vec3, b: Vec3) -> bool {
        (a - b).length() < 1e-5
    }

    #[test]
    fn lamp_yaw_turns_arm_outwards() {
        for o in [
            Vec2::new(0.0, -1.0),
            Vec2::new(1.0, 0.0),
            Vec2::new(0.0, 1.0),
        ] {
            let arm = Quat::from_rotation_y(yaw_facing_neg_z(o)) * Vec3::NEG_Z;
            assert!(close(arm, Vec3::new(o.x, 0.0, o.y)), "o {o}: arm {arm}");
        }
        assert!(yaw_facing_neg_z(Vec2::new(0.0, -1.0)).abs() < 1e-6);
        assert!((yaw_facing_neg_z(Vec2::new(1.0, 0.0)) + std::f32::consts::FRAC_PI_2).abs() < 1e-6);
    }

    #[test]
    fn container_yaw_lays_length_along_u() {
        for u in [
            Vec2::new(1.0, 0.0),
            Vec2::new(0.0, 1.0),
            Vec2::new(-1.0, 0.0),
        ] {
            let long = Quat::from_rotation_y(yaw_along_z(u)) * Vec3::Z;
            assert!(
                close(long, Vec3::new(u.x, 0.0, u.y)),
                "u {u}: long axis {long}"
            );
        }
    }

    #[test]
    fn lamp_stations_worked_example() {
        assert_eq!(lamp_stations(80.0, 4.0, 24.0), [4.0, 28.0, 52.0, 76.0]);
        assert!(lamp_stations(7.0, 4.0, 24.0).is_empty());
    }
}
