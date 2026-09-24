//! Wanted level: witnessed player crimes become heat, heat becomes stars, the search circle
//! clears them (GDD §6.4).

mod crimes;
mod search;

pub use crimes::{Crime, Crimes, Incident};
pub(crate) use search::{cop_sees, eye, witnesses};

use crate::civilian::PoliceCall;
use crate::flow::{GameState, NEW_CITY, PlayingSystems};
use crate::perception::AiSystems;
use bevy::prelude::*;
use serde::Deserialize;

/// Path of the wanted config, relative to the assets root.
pub const WANTED_CONFIG: &str = "wanted/wanted.ron";

/// Number of wanted stars (GDD §6.4 "у нас 5 звёзд"): law, not tuning.
pub const STARS: usize = 5;

/// Heat of a witnessed crime by the player.
#[derive(Deserialize, Clone, Debug)]
#[serde(deny_unknown_fields)]
pub struct HeatTable {
    pub punch_civilian: u32,
    pub shooting_near_people: u32,
    pub wound_civilian: u32,
    pub kill_person: u32,
    /// Reported always: the cop is the witness.
    pub punch_cop: u32,
    pub wound_cop: u32,
    pub kill_cop: u32,
}

impl HeatTable {
    pub(crate) fn of(&self, crime: Crime) -> u32 {
        match crime {
            Crime::Punch => self.punch_civilian,
            Crime::Shooting => self.shooting_near_people,
            Crime::Wound => self.wound_civilian,
            Crime::Kill => self.kill_person,
            Crime::PunchCop => self.punch_cop,
            Crime::WoundCop => self.wound_cop,
            Crime::KillCop => self.kill_cop,
        }
    }
}

/// One star: its heat threshold and how to lose it.
#[derive(Deserialize, Clone, Debug)]
#[serde(deny_unknown_fields)]
pub struct StarRow {
    pub heat: u32,
    /// Radius of the search circle around `WantedLevel::last_known`, m.
    pub search_radius: f32,
    /// Seconds unseen and outside the circle that clear the wanted level.
    pub clear_seconds: f32,
}

/// Wanted tuning (GDD §6.4).
#[derive(Resource, Deserialize, Clone, Debug)]
#[serde(deny_unknown_fields)]
pub struct WantedConfig {
    pub heat: HeatTable,
    pub stars: [StarRow; STARS],
    /// A person this close to the muzzle at the shot makes the shot a crime, m.
    pub shooting_radius: f32,
    /// A shot this soon after the offender's last one joins that shooting incident, s.
    pub shooting_merge_seconds: f32,
    /// An incident untouched this long is forgotten, s.
    pub incident_memory_seconds: f32,
    /// A cop witnesses a crime within this distance with a clear line, m.
    pub cop_witness_distance: f32,
    /// A cop on foot sees the player within this distance, m.
    pub cop_view_distance: f32,
    /// Full view cone of a cop, degrees.
    pub cop_view_cone_deg: f32,
}

fn positive(field: &str, value: f32) -> Result<(), String> {
    if value.is_finite() && value > 0.0 {
        Ok(())
    } else {
        Err(format!("{field} {value} must be finite and > 0"))
    }
}

impl WantedConfig {
    pub fn validate(&self) -> Result<(), String> {
        let h = &self.heat;
        for (field, value) in [
            ("heat.punch_civilian", h.punch_civilian),
            ("heat.shooting_near_people", h.shooting_near_people),
            ("heat.wound_civilian", h.wound_civilian),
            ("heat.kill_person", h.kill_person),
            ("heat.punch_cop", h.punch_cop),
            ("heat.wound_cop", h.wound_cop),
            ("heat.kill_cop", h.kill_cop),
        ] {
            if value == 0 {
                return Err(format!("{field} must be > 0"));
            }
        }
        let mut previous = 0;
        for (i, row) in self.stars.iter().enumerate() {
            if row.heat <= previous {
                return Err(format!(
                    "stars[{i}].heat {} must be > {previous}: thresholds are positive and increasing",
                    row.heat
                ));
            }
            previous = row.heat;
            positive(&format!("stars[{i}].search_radius"), row.search_radius)?;
            positive(&format!("stars[{i}].clear_seconds"), row.clear_seconds)?;
        }
        positive("shooting_radius", self.shooting_radius)?;
        positive("cop_witness_distance", self.cop_witness_distance)?;
        positive("cop_view_distance", self.cop_view_distance)?;
        let merge = self.shooting_merge_seconds;
        if !(merge.is_finite() && merge >= 0.0) {
            return Err(format!(
                "shooting_merge_seconds {merge} must be finite and >= 0"
            ));
        }
        let memory = self.incident_memory_seconds;
        if !(memory.is_finite() && memory > merge) {
            return Err(format!(
                "incident_memory_seconds {memory} must be finite and > shooting_merge_seconds"
            ));
        }
        if !(self.cop_view_cone_deg > 0.0 && self.cop_view_cone_deg < 360.0) {
            return Err(format!(
                "cop_view_cone_deg {} must be in (0, 360)",
                self.cop_view_cone_deg
            ));
        }
        Ok(())
    }

    /// A call completes up to `call_delay` s after its crime; forgetting the incident sooner drops the heat.
    pub fn validate_call_delay(&self, call_delay: f32) -> Result<(), String> {
        let memory = self.incident_memory_seconds;
        if memory > call_delay {
            return Ok(());
        }
        Err(format!(
            "incident_memory_seconds {memory} must be > the longest civilian call delay {call_delay} s \
             (perception slots + call_seconds)"
        ))
    }
}

/// Police wanted level of the player.
#[derive(Resource, Reflect, Default, Debug, Clone, Copy, PartialEq)]
#[reflect(Resource)]
pub struct WantedLevel {
    /// Heat of the reported crimes; QA and later slices change this, never `stars`.
    pub heat: u32,
    /// `stars_for(heat)`, recomputed every fixed tick.
    pub stars: u8,
    /// GDD `LastKnownPosition`: centre of the search circle; `None` while `heat == 0`.
    pub last_known: Option<Vec3>,
    /// A cop sees the player this tick.
    pub seen: bool,
    /// Seconds the player has been continuously unseen and outside the search circle.
    pub hidden: f32,
}

/// Stars earned by `heat`: the number of thresholds it reaches.
pub fn stars_for(heat: u32, rows: &[StarRow; STARS]) -> u8 {
    rows.iter().filter(|row| heat >= row.heat).count() as u8
}

/// The row whose search rule applies at `stars`; heat below the first star uses the first row.
pub fn search_row(rows: &[StarRow; STARS], stars: u8) -> &StarRow {
    &rows[usize::from(stars.max(1)) - 1]
}

impl WantedLevel {
    /// Centre and radius of the search circle; `None` without stars or a last known position.
    pub fn search_circle(&self, rows: &[StarRow; STARS]) -> Option<(Vec3, f32)> {
        if self.stars == 0 {
            return None;
        }
        let centre = self.last_known?;
        Some((centre, search_row(rows, self.stars).search_radius))
    }
}

#[derive(SystemSet, Debug, Clone, PartialEq, Eq, Hash)]
pub struct WantedSystems;

pub struct WantedPlugin;

impl Plugin for WantedPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<WantedLevel>()
            .init_resource::<Crimes>()
            .register_type::<WantedLevel>()
            .configure_sets(
                FixedUpdate,
                WantedSystems
                    .after(AiSystems::Decide)
                    .in_set(PlayingSystems),
            )
            .add_systems(
                FixedUpdate,
                (
                    crimes::record_crimes,
                    crimes::take_calls,
                    search::track_search,
                    crimes::forget_crimes,
                )
                    .chain()
                    .in_set(WantedSystems),
            )
            .add_systems(OnEnter(GameState::Wasted), reset_wanted)
            .add_systems(OnExit(GameState::Wasted), drop_queued_calls)
            .add_systems(OnExit(GameState::Busted), (reset_wanted, drop_queued_calls))
            .add_systems(NEW_CITY, (reset_wanted, drop_queued_calls));
    }
}

/// A first report of crimes worth `heat`, last seen at `at`.
fn apply_report(wanted: &mut WantedLevel, heat: u32, at: Vec3) {
    wanted.heat = wanted.heat.saturating_add(heat);
    wanted.last_known = Some(at);
    wanted.hidden = 0.0;
}

fn reset_wanted(mut wanted: ResMut<WantedLevel>, mut crimes: ResMut<Crimes>) {
    *wanted = WantedLevel::default();
    crimes.clear();
}

/// Calls completed while wasted must not be read after the respawn.
fn drop_queued_calls(mut calls: ResMut<Messages<PoliceCall>>) {
    calls.clear();
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stars_table() {
        let cfg: WantedConfig = ron::from_str(include_str!("../../../../assets/wanted/wanted.ron"))
            .unwrap_or_else(|e| panic!("GATE BROKEN: wanted.ron: {e}"));
        assert_eq!(
            cfg.stars.each_ref().map(|row| row.heat),
            [40, 180, 550, 1200, 2400],
            "GATE BROKEN: shipped thresholds changed"
        );
        for (heat, stars) in [
            (0, 0),
            (39, 0),
            (40, 1),
            (179, 1),
            (180, 2),
            (549, 2),
            (550, 3),
            (1199, 3),
            (1200, 4),
            (2399, 4),
            (2400, 5),
            (u32::MAX, 5),
        ] {
            assert_eq!(stars_for(heat, &cfg.stars), stars, "heat {heat}");
        }
    }

    #[test]
    fn search_circle_per_star() {
        let cfg: WantedConfig = ron::from_str(include_str!("../../../../assets/wanted/wanted.ron"))
            .unwrap_or_else(|e| panic!("GATE BROKEN: wanted.ron: {e}"));
        let radii = cfg.stars.each_ref().map(|row| row.search_radius);
        for i in 1..STARS {
            assert!(
                !radii[..i].contains(&radii[i]),
                "GATE BROKEN: shipped search radii must differ per star: {radii:?}"
            );
        }
        let centre = Vec3::new(12.0, 0.2, -7.0);
        let at = |stars: u8, last_known: Option<Vec3>| WantedLevel {
            heat: 1,
            stars,
            last_known,
            ..default()
        };
        assert_eq!(
            at(1, Some(centre)).search_circle(&cfg.stars),
            Some((centre, radii[0]))
        );
        assert_eq!(
            at(2, Some(centre)).search_circle(&cfg.stars),
            Some((centre, radii[1]))
        );
        assert_eq!(
            at(3, Some(centre)).search_circle(&cfg.stars),
            Some((centre, radii[2]))
        );
        assert_eq!(
            at(4, Some(centre)).search_circle(&cfg.stars),
            Some((centre, radii[3]))
        );
        assert_eq!(
            at(5, Some(centre)).search_circle(&cfg.stars),
            Some((centre, radii[4]))
        );
        assert_eq!(at(0, Some(centre)).search_circle(&cfg.stars), None);
        assert_eq!(at(3, None).search_circle(&cfg.stars), None);
    }
}
