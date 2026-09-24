use crate::menu::{Rgb, Rgba, positive, unit};
use serde::Deserialize;

/// The round minimap in the bottom-left corner (GDD §7); lives in `ui/strings.ron` with the HUD.
#[derive(Deserialize, Clone, Debug)]
#[serde(deny_unknown_fields)]
pub struct MinimapConfig {
    /// Diameter on screen, px.
    pub size: f32,
    /// World distance from the centre to the rim, m.
    pub view_radius: f32,
    /// Raster resolution, texels per metre (<= 2: a 1400 m city is 2800² texels).
    pub raster_px_per_m: f32,
    pub road: Rgb,
    pub sidewalk: Rgb,
    pub block: Rgb,
    pub park: Rgb,
    pub building: Rgb,
    /// Beyond the ground square.
    pub outside: Rgba,
    /// Opacity of a gang tint over its territory.
    pub territory_alpha: f32,
    pub rim_color: Rgba,
    pub rim_px: f32,
    pub search_fill: Rgba,
    pub search_ring: Rgba,
    pub search_ring_px: f32,
    /// View cone of a cop (range and angle come from `wanted/wanted.ron`).
    pub cone_color: Rgba,
    pub arrow_color: Rgb,
    /// Player arrow length, px.
    pub arrow_px: f32,
    /// Diameter of an NPC or pickup dot, px.
    pub dot_px: f32,
    pub pickup_color: Rgb,
    /// Landmark glyphs (the font must have them) and colours.
    pub hospital: (String, Rgb),
    pub station: (String, Rgb),
    pub glyph_size: f32,
}

impl MinimapConfig {
    pub fn validate(&self) -> Result<(), String> {
        positive("hud.minimap.size", self.size)?;
        positive("hud.minimap.view_radius", self.view_radius)?;
        positive("hud.minimap.raster_px_per_m", self.raster_px_per_m)?;
        if self.raster_px_per_m > 2.0 {
            return Err(format!(
                "hud.minimap.raster_px_per_m must be <= 2, got {}",
                self.raster_px_per_m
            ));
        }
        positive("hud.minimap.rim_px", self.rim_px)?;
        positive("hud.minimap.search_ring_px", self.search_ring_px)?;
        positive("hud.minimap.arrow_px", self.arrow_px)?;
        positive("hud.minimap.dot_px", self.dot_px)?;
        positive("hud.minimap.glyph_size", self.glyph_size)?;
        unit("hud.minimap.territory_alpha", &[self.territory_alpha])?;
        for (field, (r, g, b)) in [
            ("hud.minimap.road", self.road),
            ("hud.minimap.sidewalk", self.sidewalk),
            ("hud.minimap.block", self.block),
            ("hud.minimap.park", self.park),
            ("hud.minimap.building", self.building),
            ("hud.minimap.arrow_color", self.arrow_color),
            ("hud.minimap.pickup_color", self.pickup_color),
            ("hud.minimap.hospital", self.hospital.1),
            ("hud.minimap.station", self.station.1),
        ] {
            unit(field, &[r, g, b])?;
        }
        for (field, (r, g, b, a)) in [
            ("hud.minimap.outside", self.outside),
            ("hud.minimap.rim_color", self.rim_color),
            ("hud.minimap.search_fill", self.search_fill),
            ("hud.minimap.search_ring", self.search_ring),
            ("hud.minimap.cone_color", self.cone_color),
        ] {
            unit(field, &[r, g, b, a])?;
        }
        if self.hospital.0.is_empty() || self.station.0.is_empty() {
            return Err("hud.minimap landmark glyphs must not be empty".into());
        }
        Ok(())
    }
}
