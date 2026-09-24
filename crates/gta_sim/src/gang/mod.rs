//! Gangs: territories, the faction matrix, provocation, group aggro and the fight FSM (GDD §6.3).

mod behavior;
pub mod fsm;
mod territory;

pub use territory::{GangTerritories, Turf};

use crate::character::Character;
use crate::character::{
    CharacterSchemeConfig, Gait, Health, HealthConfig, HealthSystems, LocomotionConfig,
    character_components,
};
use crate::combat::{Loadout, Weapon, WeaponsConfig, acquire, unit_f32};
use crate::flow::{GameState, NEW_CITY, NpcSystems};
use crate::navigation::Route;
use crate::perception::{AiSystems, Perception};
use crate::population::{Appearance, Offscreen};
use crate::tactics::Discipline;
use crate::world::{City, CitySeed};
use bevy::prelude::*;
use rand_chacha::{
    ChaCha8Rng,
    rand_core::{Rng, SeedableRng},
};
use serde::Deserialize;

/// Path of the gang config, relative to the assets root.
pub const GANG_CONFIG: &str = "gang/gangs.ron";

/// Who is hostile to whom (GDD §6.3); `Police` exists so the matrix states the off pairs (T11).
#[derive(Component, Reflect, Deserialize, Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[reflect(Component)]
pub enum Faction {
    Player,
    Gang(u8),
    Police,
}

/// Gang tuning (GDD §6.3).
#[derive(Resource, Deserialize, Clone, Debug)]
#[serde(deny_unknown_fields)]
pub struct GangConfig {
    /// Index = gang id.
    pub gangs: Vec<GangSpec>,
    /// Symmetric faction matrix; every gang pair, gang–player and gang–police pair is listed.
    pub factions: Vec<FactionPair>,
    pub groups: GroupConfig,
    pub hostility: HostilityConfig,
    pub combat: GangCombatConfig,
}

#[derive(Deserialize, Clone, Debug)]
#[serde(deny_unknown_fields)]
pub struct GangSpec {
    /// Multiplier of the body colour (client).
    pub tint: (f32, f32, f32),
    /// A member carries one of these, rolled at spawn.
    pub weapons: Vec<Weapon>,
}

#[derive(Deserialize, Clone, Copy, Debug)]
#[serde(deny_unknown_fields)]
pub struct FactionPair {
    pub a: Faction,
    pub b: Faction,
    pub hostile: bool,
}

#[derive(Deserialize, Clone, Debug)]
#[serde(deny_unknown_fields)]
pub struct GroupConfig {
    /// Members per group (min, max).
    pub size: (u32, u32),
    /// Distance of each member from the post, m.
    pub spread: f32,
    /// Distance between posts, m.
    pub post_spacing: f32,
    /// The HQ post sits on a sidewalk side at least twice this long, m.
    pub hq_margin: f32,
}

#[derive(Deserialize, Clone, Debug)]
#[serde(deny_unknown_fields)]
pub struct HostilityConfig {
    pub warn_distance: f32,
    /// Seconds within `warn_distance` in the territory before a warning.
    pub warn_seconds: f32,
    pub warn_release_distance: f32,
    /// Warning members walk up to this distance, m.
    pub warn_keep_distance: f32,
    /// A shot this close to a member (from the muzzle) provokes it, m.
    pub shot_radius: f32,
    /// A provocation puts the gang's members this close to the provoked one into `Attack`, m.
    pub group_radius: f32,
    /// `GangHeat` memory, s.
    pub heat_seconds: f32,
    /// While the gang is heated, members attack the player on sight within this in their turf, m.
    pub sight_distance: f32,
    /// A member gives up a target this far from its post, m.
    pub leash_distance: f32,
}

#[derive(Deserialize, Clone, Debug)]
#[serde(deny_unknown_fields)]
pub struct GangCombatConfig {
    /// Distance band an armed member keeps (min, max), m.
    pub keep_distance: (f32, f32),
    /// Start / stop punching, m.
    pub melee_distance: (f32, f32),
    /// Cone added on top of the weapon spread, degrees.
    pub aim_error_deg: f32,
    /// Seconds between trigger pulls or punches (min, max).
    pub trigger_seconds: (f32, f32),
    /// Share of max health below which a member retreats.
    pub retreat_health: f32,
    /// A retreating member stops this far from its target, m.
    pub retreat_distance: f32,
    /// Clearance kept between the line of fire and a body that must not be hit, on top of the body
    /// radius and the spread cone at that distance, m.
    pub fire_line_margin: f32,
    pub tactics: TacticWeights,
    pub warn_gait: Gait,
    pub chase_gait: Gait,
    pub back_off_gait: Gait,
    pub retreat_gait: Gait,
    /// Sideways offsets (each on both sides of the line) of the spots a member with a blocked line of
    /// fire tries, m.
    pub reposition_offsets: Vec<f32>,
    /// Step back or forward along the line, the other spots a blocked member tries, m.
    pub reposition_step: f32,
    /// Gait of a member walking to a spot with a clear line of fire.
    pub reposition_gait: Gait,
}

/// Utility weights of the fight tactics; the highest available wins.
#[derive(Deserialize, Clone, Debug)]
#[serde(deny_unknown_fields)]
pub struct TacticWeights {
    pub retreat: f32,
    pub melee: f32,
    pub shoot: f32,
    pub chase: f32,
}

fn positive(field: &str, value: f32) -> Result<(), String> {
    if value.is_finite() && value > 0.0 {
        Ok(())
    } else {
        Err(format!("{field} {value} must be finite and > 0"))
    }
}

impl GangConfig {
    pub fn validate(&self) -> Result<(), String> {
        if self.gangs.len() != 2 {
            return Err(format!(
                "gangs must list exactly 2 gangs: citygen assigns 2 territories (found {})",
                self.gangs.len()
            ));
        }
        for (i, gang) in self.gangs.iter().enumerate() {
            if gang.weapons.is_empty() {
                return Err(format!("gangs[{i}].weapons must not be empty"));
            }
            let (r, g, b) = gang.tint;
            if ![r, g, b].iter().all(|c| c.is_finite() && *c >= 0.0) {
                return Err(format!(
                    "gangs[{i}].tint {:?} must be finite and >= 0",
                    gang.tint
                ));
            }
        }
        self.validate_factions()?;
        self.validate_groups()?;
        self.validate_hostility()?;
        self.validate_combat()
    }

    fn validate_factions(&self) -> Result<(), String> {
        let mut seen: Vec<(Faction, Faction)> = Vec::new();
        for pair in &self.factions {
            if pair.a == pair.b {
                return Err(format!(
                    "factions: pair ({:?}, {:?}) names one faction twice",
                    pair.a, pair.b
                ));
            }
            for f in [pair.a, pair.b] {
                if let Faction::Gang(i) = f
                    && i as usize >= self.gangs.len()
                {
                    return Err(format!("factions: {f:?} is not a listed gang"));
                }
            }
            if seen
                .iter()
                .any(|&(a, b)| same_pair((a, b), (pair.a, pair.b)))
            {
                return Err(format!(
                    "factions: pair ({:?}, {:?}) listed twice",
                    pair.a, pair.b
                ));
            }
            seen.push((pair.a, pair.b));
        }
        let gangs = self.gangs.len() as u8;
        let mut required = Vec::new();
        for i in 0..gangs {
            for j in i + 1..gangs {
                required.push((Faction::Gang(i), Faction::Gang(j)));
            }
            required.push((Faction::Gang(i), Faction::Player));
            required.push((Faction::Gang(i), Faction::Police));
        }
        for need in required {
            if !seen.iter().any(|&p| same_pair(p, need)) {
                return Err(format!(
                    "factions: missing pair ({:?}, {:?})",
                    need.0, need.1
                ));
            }
        }
        Ok(())
    }

    fn validate_groups(&self) -> Result<(), String> {
        let g = &self.groups;
        let (lo, hi) = g.size;
        if !(1 <= lo && lo <= hi) {
            return Err(format!(
                "groups.size ({lo}, {hi}) must satisfy 1 <= lo <= hi"
            ));
        }
        positive("groups.spread", g.spread)?;
        positive("groups.post_spacing", g.post_spacing)?;
        positive("groups.hq_margin", g.hq_margin)?;
        if g.post_spacing <= 2.0 * g.spread {
            return Err(format!(
                "groups.post_spacing {} must exceed 2 * groups.spread ({})",
                g.post_spacing, g.spread
            ));
        }
        Ok(())
    }

    fn validate_hostility(&self) -> Result<(), String> {
        let h = &self.hostility;
        for (field, value) in [
            ("hostility.warn_distance", h.warn_distance),
            ("hostility.warn_release_distance", h.warn_release_distance),
            ("hostility.warn_keep_distance", h.warn_keep_distance),
            ("hostility.shot_radius", h.shot_radius),
            ("hostility.group_radius", h.group_radius),
            ("hostility.heat_seconds", h.heat_seconds),
            ("hostility.sight_distance", h.sight_distance),
            ("hostility.leash_distance", h.leash_distance),
        ] {
            positive(field, value)?;
        }
        if !(h.warn_seconds.is_finite() && h.warn_seconds >= 0.0) {
            return Err(format!(
                "hostility.warn_seconds {} must be finite and >= 0",
                h.warn_seconds
            ));
        }
        if !(h.warn_keep_distance <= h.warn_distance && h.warn_distance < h.warn_release_distance) {
            return Err(format!(
                "hostility: warn_keep_distance {} <= warn_distance {} < warn_release_distance {} must hold",
                h.warn_keep_distance, h.warn_distance, h.warn_release_distance
            ));
        }
        Ok(())
    }

    fn validate_combat(&self) -> Result<(), String> {
        let c = &self.combat;
        let (m0, m1) = c.melee_distance;
        let (k0, k1) = c.keep_distance;
        let all = [m0, m1, k0, k1];
        if !(all.iter().all(|v| v.is_finite()) && 0.0 < m0 && m0 < m1 && m1 < k0 && k0 < k1) {
            return Err(format!(
                "combat.melee_distance {:?} / combat.keep_distance {:?} must satisfy 0 < melee.0 < melee.1 < keep.0 < keep.1",
                c.melee_distance, c.keep_distance
            ));
        }
        if !(c.aim_error_deg.is_finite() && (0.0..90.0).contains(&c.aim_error_deg)) {
            return Err(format!(
                "combat.aim_error_deg {} must be in [0, 90)",
                c.aim_error_deg
            ));
        }
        let (t0, t1) = c.trigger_seconds;
        if !(t0.is_finite() && t1.is_finite() && 0.0 < t0 && t0 <= t1) {
            return Err(format!(
                "combat.trigger_seconds {:?} must satisfy 0 < lo <= hi",
                c.trigger_seconds
            ));
        }
        if !(c.retreat_health > 0.0 && c.retreat_health < 1.0) {
            return Err(format!(
                "combat.retreat_health {} must be in (0, 1)",
                c.retreat_health
            ));
        }
        positive("combat.retreat_distance", c.retreat_distance)?;
        if !(c.fire_line_margin.is_finite() && c.fire_line_margin >= 0.0) {
            return Err(format!(
                "combat.fire_line_margin {} must be finite and >= 0",
                c.fire_line_margin
            ));
        }
        if c.reposition_offsets.is_empty()
            || !c
                .reposition_offsets
                .iter()
                .all(|o| o.is_finite() && *o > 0.0)
        {
            return Err(format!(
                "combat.reposition_offsets {:?} must list at least one finite offset > 0",
                c.reposition_offsets
            ));
        }
        positive("combat.reposition_step", c.reposition_step)?;
        let w = &c.tactics;
        for (field, value) in [
            ("combat.tactics.retreat", w.retreat),
            ("combat.tactics.melee", w.melee),
            ("combat.tactics.shoot", w.shoot),
            ("combat.tactics.chase", w.chase),
        ] {
            if !(value.is_finite() && value >= 0.0) {
                return Err(format!("{field} {value} must be finite and >= 0"));
            }
        }
        Ok(())
    }

    /// `a` and `b` fight; one faction is never hostile to itself.
    pub fn hostile(&self, a: Faction, b: Faction) -> bool {
        a != b
            && self
                .factions
                .iter()
                .any(|p| p.hostile && same_pair((p.a, p.b), (a, b)))
    }

    /// A gang member's blow does no harm to a character its gang is not hostile to: groupmates,
    /// bystanders without a faction, the other gang while the matrix keeps the peace.
    pub fn spares(&self, attacker: Option<Faction>, target: Option<Faction>) -> bool {
        let Some(gang @ Faction::Gang(_)) = attacker else {
            return false;
        };
        !target.is_some_and(|t| self.hostile(gang, t))
    }
}

impl GangCombatConfig {
    /// Fire-line tuning of a member; a spared body within `melee_distance.1` of the target yields it.
    pub(crate) fn discipline(&self) -> Discipline<'_> {
        Discipline {
            aim_error_deg: self.aim_error_deg,
            fire_line_margin: self.fire_line_margin,
            pressed_distance: self.melee_distance.1,
            reposition_offsets: &self.reposition_offsets,
            reposition_step: self.reposition_step,
            reposition_gait: self.reposition_gait,
            chase_gait: self.chase_gait,
        }
    }
}

fn same_pair(p: (Faction, Faction), q: (Faction, Faction)) -> bool {
    p == q || (p.0 == q.1 && p.1 == q.0)
}

#[derive(Reflect, Clone, Copy, Debug, PartialEq)]
pub enum GangState {
    Idle,
    /// Gun drawn and aimed at the player, walking up.
    Warn,
    Attack {
        target: Entity,
    },
    Retreat {
        from: Entity,
    },
    Dead,
}

#[derive(Component, Reflect, Clone, Debug)]
#[reflect(Component)]
#[require(Character, Perception, Offscreen, Route)]
pub struct GangMember {
    pub gang: u8,
    /// Index into `GangTerritories.gangs[gang].posts` (0 = HQ).
    pub post: u16,
    /// Own standing point (feet) at the post.
    pub spot: Vec3,
    pub gun: Weapon,
    pub state: GangState,
    /// Seconds the player has stood in this gang's territory within `warn_distance`.
    pub dwell: f32,
    /// Line of sight to the current focus (target, else the player), refreshed on the member's AI slot.
    pub sees: bool,
    /// Entity `sees` refers to; a new focus is looked at at once, off the slot.
    pub sees_focus: Option<Entity>,
    pub last_seen: Vec3,
    /// Line of sight from the chest to `spot`, refreshed on the AI slot while away from it.
    pub home_clear: bool,
    /// Yaw offset of the current direct seek chosen by `avoid_offset`, refreshed on the AI slot.
    pub avoid: f32,
    /// Seconds until the next trigger pull or punch may happen (clamped at 0).
    pub trigger_left: f32,
    /// Spot (chest) with a clear line of fire the member walks to while its own line is blocked,
    /// re-checked on its AI slot; `None` while the line is clear or no spot around it clears.
    pub reposition: Option<Vec3>,
}

/// Seconds of memory per gang since the player last provoked it (GDD §6.3).
#[derive(Resource, Reflect, Debug)]
#[reflect(Resource)]
pub struct GangHeat {
    pub left: Vec<f32>,
}

/// Gang whose territory the live player stands in.
#[derive(Resource, Reflect, Default, Debug, PartialEq)]
#[reflect(Resource)]
pub struct PlayerTerritory(pub Option<u8>);

/// Sim-owned RNG of gang rolls; a different stream than `NpcRng` and `CombatRng` of the same seed.
#[derive(Resource)]
pub struct GangRng(pub ChaCha8Rng);

impl GangRng {
    pub fn seeded(seed: u64) -> Self {
        let mut rng = ChaCha8Rng::seed_from_u64(seed);
        rng.set_stream(2);
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

/// Gang AI that needs territories.
#[derive(SystemSet, Debug, Clone, PartialEq, Eq, Hash)]
pub struct GangSystems;

/// Uniform in `[lo, hi)`.
pub fn roll(rng: &mut GangRng, (lo, hi): (f32, f32)) -> f32 {
    lo + (hi - lo) * rng.unit()
}

/// An idle, holstered gang member standing at `spot` (feet) and facing `facing_yaw`.
#[allow(clippy::too_many_arguments)]
pub fn gang_member_bundle(
    loco: &LocomotionConfig,
    handle: Handle<CharacterSchemeConfig>,
    health: &HealthConfig,
    weapons: &WeaponsConfig,
    gang: u8,
    post: u16,
    spot: Vec3,
    facing_yaw: f32,
    gun: Weapon,
    appearance: Appearance,
) -> impl Bundle {
    let mut loadout = Loadout::default();
    acquire(&mut loadout.guns[gun.index()], weapons.stats(gun), true);
    (
        GangMember {
            gang,
            post,
            spot,
            gun,
            state: GangState::Idle,
            dwell: 0.0,
            sees: false,
            sees_focus: None,
            last_seen: spot,
            home_clear: true,
            avoid: 0.0,
            trigger_left: 0.0,
            reposition: None,
        },
        Faction::Gang(gang),
        appearance,
        Name::new("Gang member"),
        Transform::from_translation(spot + Vec3::Y * loco.float_height)
            .with_rotation(Quat::from_rotation_y(facing_yaw)),
        character_components(loco, handle),
        Health::full(health),
        loadout,
    )
}

pub struct GangPlugin {
    /// Seed of `GangRng` (the combat seed).
    pub seed: u64,
}

impl Plugin for GangPlugin {
    fn build(&self, app: &mut App) {
        let gangs = app.world().resource::<GangConfig>().gangs.len();
        app.insert_resource(GangHeat {
            left: vec![0.0; gangs],
        })
        .init_resource::<PlayerTerritory>()
        .insert_resource(GangRng::seeded(self.seed))
        .register_type::<Faction>()
        .register_type::<GangState>()
        .register_type::<GangMember>()
        .register_type::<GangHeat>()
        .register_type::<PlayerTerritory>()
        .register_type::<GangTerritories>()
        .register_type::<Turf>()
        .configure_sets(
            FixedUpdate,
            GangSystems
                .in_set(NpcSystems)
                .run_if(resource_exists::<GangTerritories>),
        )
        // Once per session: a `Playing` re-entry after `Wasted` keeps the territories.
        .add_systems(
            OnTransition {
                exited: GameState::Loading,
                entered: GameState::Playing,
            },
            territory::build_gang_territories.run_if(resource_exists::<City>),
        )
        .add_systems(
            FixedUpdate,
            (
                // Not in `GangSystems`: a death is handled even without territories.
                behavior::gang_death
                    .in_set(HealthSystems::Death)
                    .in_set(NpcSystems),
                (
                    behavior::decay_gang_heat,
                    behavior::locate_player,
                    behavior::provoke_gangs,
                )
                    .chain()
                    .in_set(AiSystems::Perceive)
                    .in_set(GangSystems),
                behavior::gang_fsm
                    .in_set(AiSystems::Decide)
                    .in_set(GangSystems),
            ),
        )
        .add_systems(NEW_CITY, drop_gang_city)
        .add_systems(
            OnEnter(GameState::Loading),
            reseed_gangs.run_if(resource_exists::<CitySeed>),
        );
    }
}

fn drop_gang_city(
    mut commands: Commands,
    mut heat: ResMut<GangHeat>,
    mut territory: ResMut<PlayerTerritory>,
) {
    commands.remove_resource::<GangTerritories>();
    heat.left.fill(0.0);
    *territory = PlayerTerritory::default();
}

fn reseed_gangs(seed: Res<CitySeed>, mut rng: ResMut<GangRng>) {
    *rng = GangRng::seeded(seed.0);
}
