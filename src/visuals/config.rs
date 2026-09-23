use bevy::{camera::PerspectiveProjection, prelude::*};
use serde::Deserialize;

pub const RENDER_CONFIG: &str = "world/render.ron";

/// sRGB colour as stored in `render.ron`.
pub(super) type Rgb = (f32, f32, f32);

#[derive(Resource, Deserialize, Clone)]
#[serde(deny_unknown_fields)]
pub struct RenderConfig {
    pub(super) ambient_brightness: f32,
    pub(super) sun_illuminance: f32,
    pub(super) sun_pitch_deg: f32,
    pub(super) sun_yaw_deg: f32,
    pub(super) shadows: ShadowConfig,
    /// Edge of a square render chunk (m); the city is merged into one mesh per chunk.
    pub(super) chunk_size: f32,
    pub(super) spawn_budget: SpawnBudget,
    pub(super) fog: FogConfig,
    pub(super) sky: SkyConfig,
    pub(super) district_colors: DistrictColors,
    pub(super) tower_color: Rgb,
    pub(super) hospital_color: Rgb,
    pub(super) police_color: Rgb,
    pub(super) gang_hq_color: Rgb,
    pub(super) road_color: Rgb,
    pub(super) sidewalk_color: Rgb,
    pub(super) park_color: Rgb,
    pub(super) curb_color: Rgb,
    pub(super) plaza_color: Rgb,
    pub(super) lot_colors: DistrictColors,
    /// Lift of flat surface layers above the ground against z-fighting (m).
    pub(super) surface_layer_step: f32,
    pub(super) markings: MarkingsConfig,
    pub(super) facade: FacadeConfig,
    pub(super) props: PropsConfig,
    pub(super) pickups: PickupVisuals,
}

/// sRGB colours per district kind.
#[derive(Deserialize, Clone)]
#[serde(deny_unknown_fields)]
pub(super) struct DistrictColors {
    pub(super) downtown: Rgb,
    pub(super) commercial: Rgb,
    pub(super) residential: Rgb,
    pub(super) industrial: Rgb,
}

#[derive(Deserialize, Clone)]
#[serde(deny_unknown_fields)]
pub(super) struct ShadowConfig {
    pub(super) map_size: usize,
    pub(super) cascades: usize,
    pub(super) first_cascade_far_bound: f32,
    pub(super) maximum_distance: f32,
}

/// Entities spawned per frame while the city is applied.
#[derive(Deserialize, Clone)]
#[serde(deny_unknown_fields)]
pub(super) struct SpawnBudget {
    pub(super) chunks_per_frame: usize,
    pub(super) props_per_frame: usize,
}

#[derive(Deserialize, Clone)]
#[serde(deny_unknown_fields)]
pub(super) struct FogConfig {
    pub(super) color: Rgb,
    pub(super) start: f32,
    pub(super) end: f32,
    pub(super) sun_glow: (f32, f32, f32, f32),
    pub(super) sun_glow_exponent: f32,
}

#[derive(Deserialize, Clone)]
#[serde(deny_unknown_fields)]
pub(super) struct SkyConfig {
    pub(super) zenith: Rgb,
    pub(super) radius: f32,
    pub(super) gradient_exponent: f32,
    pub(super) sectors: u32,
    pub(super) stacks: u32,
}

#[derive(Deserialize, Clone)]
#[serde(deny_unknown_fields)]
pub(super) struct MarkingsConfig {
    pub(super) color: Rgb,
    pub(super) center_color: Rgb,
    pub(super) width: f32,
    pub(super) dash: f32,
    pub(super) gap: f32,
    pub(super) crosswalk_depth: f32,
    pub(super) crosswalk_stripe: f32,
    pub(super) crosswalk_gap: f32,
}

/// Window grid of the facade shader; window sizes and the sill are fractions of a bay/floor cell.
#[derive(Deserialize, Clone)]
#[serde(deny_unknown_fields)]
pub(super) struct FacadeConfig {
    pub(super) bay_width: f32,
    pub(super) window_width: f32,
    pub(super) window_height: f32,
    pub(super) window_sill: f32,
    pub(super) glass_color: Rgb,
    pub(super) glass_roughness: f32,
    pub(super) wall_roughness: f32,
}

#[derive(Deserialize, Clone)]
#[serde(deny_unknown_fields)]
pub(super) struct PropModel {
    pub(super) model: String,
    pub(super) texture: String,
    pub(super) scale: f32,
}

#[derive(Deserialize, Clone)]
#[serde(deny_unknown_fields)]
pub(super) struct PropsConfig {
    pub(super) visibility_range: f32,
    pub(super) fade: f32,
    pub(super) roughness: f32,
    pub(super) lamp: PropModel,
    pub(super) traffic_light: PropModel,
    pub(super) tree_large: PropModel,
    pub(super) tree_small: PropModel,
    pub(super) container_a: PropModel,
    pub(super) container_c: PropModel,
    /// Raw container model footprint: (width along X, length along Z) in model units.
    pub(super) container_raw_size: (f32, f32),
    pub(super) lamp_spacing: f32,
    pub(super) lamp_corner_clearance: f32,
    pub(super) lamp_curb_offset: f32,
    pub(super) tree_curb_offset: f32,
    pub(super) traffic_light_offset: f32,
    pub(super) park_tree_spacing: f32,
    pub(super) park_tree_jitter: f32,
    pub(super) park_tree_margin: f32,
    pub(super) container_gap: f32,
}

/// Placeholder cubes of the medkit and armour pickups.
#[derive(Deserialize, Clone, Debug)]
#[serde(deny_unknown_fields)]
pub(super) struct PickupVisuals {
    /// Cube edge (m).
    pub(super) size: f32,
    /// Height of the cube centre above the pickup point (m).
    pub(super) lift: f32,
    pub(super) health_color: Rgb,
    pub(super) armor_color: Rgb,
}

impl PropsConfig {
    /// Models in `PropKind` order.
    pub(super) fn models(&self) -> [&PropModel; 6] {
        [
            &self.lamp,
            &self.traffic_light,
            &self.tree_large,
            &self.tree_small,
            &self.container_a,
            &self.container_c,
        ]
    }
}

fn check(ok: bool, name: &str, rule: &str) -> Result<(), String> {
    if ok {
        Ok(())
    } else {
        Err(format!("{name} {rule}"))
    }
}

fn positive(name: &str, value: f32) -> Result<(), String> {
    check(
        value.is_finite() && value > 0.0,
        name,
        "must be finite and positive",
    )
}

fn unit(name: &str, value: f32) -> Result<(), String> {
    check((0.0..=1.0).contains(&value), name, "must be in [0, 1]")
}

impl RenderConfig {
    /// Asset paths (model and texture) of every prop.
    pub fn prop_asset_paths(&self) -> impl Iterator<Item = &str> {
        self.props
            .models()
            .into_iter()
            .flat_map(|m| [m.model.as_str(), m.texture.as_str()])
    }

    pub fn validate(&self) -> Result<(), String> {
        let colors = [
            self.district_colors.downtown,
            self.district_colors.commercial,
            self.district_colors.residential,
            self.district_colors.industrial,
            self.lot_colors.downtown,
            self.lot_colors.commercial,
            self.lot_colors.residential,
            self.lot_colors.industrial,
            self.tower_color,
            self.hospital_color,
            self.police_color,
            self.gang_hq_color,
            self.road_color,
            self.sidewalk_color,
            self.park_color,
            self.curb_color,
            self.plaza_color,
            self.fog.color,
            self.sky.zenith,
            self.markings.color,
            self.markings.center_color,
            self.facade.glass_color,
        ];
        let g = self.fog.sun_glow;
        let floats = [
            self.ambient_brightness,
            self.sun_illuminance,
            self.sun_pitch_deg,
            self.sun_yaw_deg,
            self.surface_layer_step,
            self.fog.sun_glow_exponent,
            self.sky.gradient_exponent,
            g.0,
            g.1,
            g.2,
            g.3,
        ];
        check(
            colors
                .iter()
                .flat_map(|c| [c.0, c.1, c.2])
                .chain(floats)
                .all(f32::is_finite),
            "colours, sun, ambient, surface_layer_step and exponents",
            "must be finite",
        )?;
        positive("chunk_size", self.chunk_size)?;
        check(
            self.spawn_budget.chunks_per_frame >= 1,
            "spawn_budget.chunks_per_frame",
            "must be at least 1",
        )?;
        check(
            self.spawn_budget.props_per_frame >= 1,
            "spawn_budget.props_per_frame",
            "must be at least 1",
        )?;
        let s = &self.shadows;
        check(
            s.map_size.is_power_of_two(),
            "shadows.map_size",
            "must be a power of two",
        )?;
        check(s.cascades >= 1, "shadows.cascades", "must be at least 1")?;
        positive("shadows.first_cascade_far_bound", s.first_cascade_far_bound)?;
        check(
            s.first_cascade_far_bound < s.maximum_distance && s.maximum_distance.is_finite(),
            "shadows.maximum_distance",
            "must exceed first_cascade_far_bound",
        )?;
        let far = PerspectiveProjection::default().far;
        positive("fog.start", self.fog.start)?;
        check(
            self.fog.start < self.fog.end
                && self.fog.end < self.sky.radius
                && self.sky.radius < far,
            "fog.start < fog.end < sky.radius < camera far",
            "must hold",
        )?;
        check(self.sky.sectors >= 3, "sky.sectors", "must be at least 3")?;
        check(self.sky.stacks >= 2, "sky.stacks", "must be at least 2")?;
        let p = &self.props;
        positive("props.visibility_range", p.visibility_range)?;
        check(
            p.fade >= 0.0 && p.fade < p.visibility_range,
            "props.fade",
            "must be in [0, visibility_range)",
        )?;
        unit("props.roughness", p.roughness)?;
        for (name, model) in [
            ("lamp", &p.lamp),
            ("traffic_light", &p.traffic_light),
            ("tree_large", &p.tree_large),
            ("tree_small", &p.tree_small),
            ("container_a", &p.container_a),
            ("container_c", &p.container_c),
        ] {
            positive(&format!("props.{name}.scale"), model.scale)?;
        }
        for (name, value) in [
            ("props.container_raw_size.0", p.container_raw_size.0),
            ("props.container_raw_size.1", p.container_raw_size.1),
            ("props.lamp_spacing", p.lamp_spacing),
            ("props.park_tree_spacing", p.park_tree_spacing),
            ("props.container_gap", p.container_gap),
            ("markings.width", self.markings.width),
            ("markings.dash", self.markings.dash),
            ("markings.gap", self.markings.gap),
            ("markings.crosswalk_depth", self.markings.crosswalk_depth),
            ("markings.crosswalk_stripe", self.markings.crosswalk_stripe),
            ("markings.crosswalk_gap", self.markings.crosswalk_gap),
            ("facade.bay_width", self.facade.bay_width),
        ] {
            positive(name, value)?;
        }
        for (name, value) in [
            ("props.lamp_corner_clearance", p.lamp_corner_clearance),
            ("props.lamp_curb_offset", p.lamp_curb_offset),
            ("props.tree_curb_offset", p.tree_curb_offset),
            ("props.traffic_light_offset", p.traffic_light_offset),
            ("props.park_tree_jitter", p.park_tree_jitter),
            ("props.park_tree_margin", p.park_tree_margin),
        ] {
            check(
                value.is_finite() && value >= 0.0,
                name,
                "must be finite and non-negative",
            )?;
        }
        let f = &self.facade;
        check(
            f.window_width > 0.0 && f.window_width < 1.0,
            "facade.window_width",
            "must be in (0, 1)",
        )?;
        check(
            f.window_height > 0.0 && f.window_height < 1.0,
            "facade.window_height",
            "must be in (0, 1)",
        )?;
        check(
            f.window_sill >= 0.0 && f.window_sill + f.window_height < 1.0,
            "facade.window_sill",
            "must be >= 0 with window_sill + window_height < 1",
        )?;
        unit("facade.glass_roughness", f.glass_roughness)?;
        unit("facade.wall_roughness", f.wall_roughness)?;
        let k = &self.pickups;
        positive("pickups.size", k.size)?;
        positive("pickups.lift", k.lift)?;
        for (name, (r, g, b)) in [
            ("pickups.health_color", k.health_color),
            ("pickups.armor_color", k.armor_color),
        ] {
            for value in [r, g, b] {
                unit(name, value)?;
            }
        }
        Ok(())
    }
}
