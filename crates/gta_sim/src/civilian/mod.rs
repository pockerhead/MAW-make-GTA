//! Civilians: enum FSM over the sidewalk graph, utility reaction to threats, witnesses (GDD §6.2).

mod reaction;

pub use reaction::{Reaction, choose_reaction};

use crate::character::{
    Character, CharacterSchemeConfig, Gait, Health, HealthConfig, HealthSystems, LocomotionConfig,
    MoveIntent, character_components,
};
use crate::flow::NpcSystems;
use crate::navigation::{
    GraphWalker, NavigationConfig, SidewalkGraph, flat_distance, flee_next, flee_start,
    lane_target, steer, wander_next,
};
use crate::perception::{AiSystems, Cause, Perception, Threat, ThreatKind};
use crate::population::{Appearance, NpcRng, Offscreen, corpse_components};
use avian3d::prelude::*;
use bevy::prelude::*;
use serde::Deserialize;

/// Path of the civilian config, relative to the assets root.
pub const CIVILIAN_CONFIG: &str = "npc/civilian.ron";

/// Civilian behaviour tuning (GDD §6.2).
#[derive(Resource, Deserialize, Clone, Debug)]
#[serde(deny_unknown_fields)]
pub struct CivilianConfig {
    pub wander_gait: Gait,
    pub flee_gait: Gait,
    /// Chance to stop at a node while wandering.
    pub idle_chance: f32,
    pub idle_seconds: (f32, f32),
    /// Path length of one flight, m.
    pub flee_distance: (f32, f32),
    pub cower_seconds: (f32, f32),
    /// Duration of a police call (`Report`), s.
    pub call_seconds: f32,
    /// Chance that a witness of a crime calls once its flight or crouch ends.
    pub call_after_flee: f32,
    pub reaction: ReactionConfig,
}

/// Weights of the reaction scorer.
#[derive(Deserialize, Clone, Debug)]
#[serde(deny_unknown_fields)]
pub struct ReactionConfig {
    pub flee: f32,
    pub cower: f32,
    pub report: f32,
    /// Each temperament factor is rolled in `[1 - spread, 1 + spread]`.
    pub temperament_spread: f32,
    /// The cower weight falls to 0 at this threat distance, m.
    pub panic_distance: f32,
    /// A gunshot closer than this is never phoned in, m.
    pub report_min_distance: f32,
    /// A fight closer than this is never phoned in, m; below `fight_hearing_radius`, or no punch is.
    pub fight_report_min_distance: f32,
}

fn range_ok(field: &str, (lo, hi): (f32, f32)) -> Result<(), String> {
    if lo.is_finite() && hi.is_finite() && 0.0 <= lo && lo <= hi {
        Ok(())
    } else {
        Err(format!(
            "{field} ({lo}, {hi}) must be finite with 0 <= lo <= hi"
        ))
    }
}

impl CivilianConfig {
    pub fn validate(&self) -> Result<(), String> {
        range_ok("idle_seconds", self.idle_seconds)?;
        range_ok("flee_distance", self.flee_distance)?;
        range_ok("cower_seconds", self.cower_seconds)?;
        if !(0.0..=1.0).contains(&self.idle_chance) {
            return Err(format!(
                "idle_chance {} must be in [0, 1]",
                self.idle_chance
            ));
        }
        if !(0.0..=1.0).contains(&self.call_after_flee) {
            return Err(format!(
                "call_after_flee {} must be in [0, 1]",
                self.call_after_flee
            ));
        }
        if !(self.call_seconds.is_finite() && self.call_seconds > 0.0) {
            return Err(format!(
                "call_seconds {} must be finite and > 0",
                self.call_seconds
            ));
        }
        let r = &self.reaction;
        for (field, value) in [
            ("reaction.flee", r.flee),
            ("reaction.cower", r.cower),
            ("reaction.report", r.report),
            ("reaction.report_min_distance", r.report_min_distance),
            (
                "reaction.fight_report_min_distance",
                r.fight_report_min_distance,
            ),
        ] {
            if !(value.is_finite() && value >= 0.0) {
                return Err(format!("{field} {value} must be finite and >= 0"));
            }
        }
        if !(0.0..1.0).contains(&r.temperament_spread) {
            return Err(format!(
                "reaction.temperament_spread {} must be in [0, 1)",
                r.temperament_spread
            ));
        }
        if !(r.panic_distance.is_finite() && r.panic_distance > 0.0) {
            return Err(format!(
                "reaction.panic_distance {} must be finite and > 0",
                r.panic_distance
            ));
        }
        Ok(())
    }

    /// Upper bound from a crime to the end of a call about it: its body stays in sight up to `corpse_seconds`,
    /// then perception, a full crouch and a full flight, the call.
    pub fn longest_call_delay(
        &self,
        corpse_seconds: f32,
        perception_seconds: f32,
        flee_speed: f32,
    ) -> f32 {
        corpse_seconds
            + perception_seconds
            + self.cower_seconds.1
            + self.flee_distance.1 / flee_speed
            + self.call_seconds
    }

    /// A fight is heard up to `fight_hearing_radius` m; a report threshold at or past it phones in no punch.
    pub fn validate_fight_hearing(&self, fight_hearing_radius: f32) -> Result<(), String> {
        let min = self.reaction.fight_report_min_distance;
        if min < fight_hearing_radius {
            return Ok(());
        }
        Err(format!(
            "reaction.fight_report_min_distance {min} must be < perception fight_hearing_radius \
             {fight_hearing_radius}, or no punch is ever phoned in"
        ))
    }
}

#[derive(Reflect, Clone, Copy, Debug, PartialEq)]
pub enum CivilianState {
    Wander,
    Idle {
        left: f32,
    },
    /// `about`: the crime this civilian may phone in when the flight ends.
    Flee {
        from: Vec3,
        left: f32,
        about: Option<Cause>,
    },
    /// `about`: the crime this civilian may phone in when the crouch ends.
    Cower {
        from: Vec3,
        left: f32,
        about: Option<Cause>,
    },
    /// Phoning the police; `progress` goes 0 -> 1 over `call_seconds`, `about` names the crime
    /// being phoned in.
    Report {
        progress: f32,
        about: Option<Cause>,
    },
    Dead,
}

/// A completed witness call (GDD §6.2); `wanted` turns it into heat.
#[derive(Message, Reflect, Clone, Copy, Debug)]
#[reflect(Message)]
pub struct PoliceCall {
    pub caller: Entity,
    pub about: Cause,
}

/// Per-civilian multipliers of the reaction weights.
#[derive(Reflect, Clone, Copy, Debug, PartialEq)]
pub struct Temperament {
    pub flee: f32,
    pub cower: f32,
    pub report: f32,
}

#[derive(Component, Reflect)]
#[reflect(Component)]
#[require(Character, Perception, Offscreen)]
pub struct Civilian {
    pub state: CivilianState,
    pub temperament: Temperament,
}

/// A wandering civilian on `walker`'s edge at fraction `t` from `node(from)`.
#[allow(clippy::too_many_arguments)]
pub fn civilian_bundle(
    loco: &LocomotionConfig,
    handle: Handle<CharacterSchemeConfig>,
    health: &HealthConfig,
    graph: &SidewalkGraph,
    walker: GraphWalker,
    t: f32,
    temperament: Temperament,
    appearance: Appearance,
) -> impl Bundle {
    let feet = graph.node(walker.from).lerp(graph.node(walker.to), t);
    (
        Civilian {
            state: CivilianState::Wander,
            temperament,
        },
        walker,
        appearance,
        Name::new("Civilian"),
        Transform::from_translation(feet + Vec3::Y * loco.float_height),
        character_components(loco, handle),
        Health::full(health),
    )
}

pub fn roll_temperament(rng: &mut NpcRng, spread: f32) -> Temperament {
    let mut factor = || 1.0 + spread * (2.0 * rng.unit() - 1.0);
    Temperament {
        flee: factor(),
        cower: factor(),
        report: factor(),
    }
}

fn roll(rng: &mut NpcRng, (lo, hi): (f32, f32)) -> f32 {
    lo + (hi - lo) * rng.unit()
}

pub struct CivilianPlugin;

impl Plugin for CivilianPlugin {
    fn build(&self, app: &mut App) {
        app.register_type::<Civilian>()
            .register_type::<CivilianState>()
            .register_type::<Temperament>()
            .register_type::<PoliceCall>()
            .add_message::<PoliceCall>()
            .add_systems(
                FixedUpdate,
                (
                    civilian_death
                        .in_set(HealthSystems::Death)
                        .in_set(NpcSystems),
                    civilian_fsm.in_set(AiSystems::Decide),
                ),
            );
    }
}

fn civilian_death(
    mut commands: Commands,
    mut civilians: Query<(
        Entity,
        &Health,
        &mut Civilian,
        &mut Perception,
        &mut MoveIntent,
    )>,
) {
    for (entity, health, mut civilian, mut perception, mut intent) in &mut civilians {
        if civilian.state == CivilianState::Dead || health.current > 0.0 {
            continue;
        }
        civilian.state = CivilianState::Dead;
        perception.pending = None;
        intent.axis = Vec2::ZERO;
        commands.entity(entity).insert(corpse_components());
    }
}

/// What one civilian knows this tick.
struct Context<'a> {
    cfg: &'a CivilianConfig,
    graph: &'a SidewalkGraph,
    dt: f32,
    /// Horizontal speed, m/s.
    speed: f32,
}

/// Wander / Idle meeting a threat.
fn react(
    threat: Threat,
    civilian: &Civilian,
    walker: &mut GraphWalker,
    allow_report: bool,
    ctx: &Context,
    rng: &mut NpcRng,
) -> CivilianState {
    let reaction = choose_reaction(
        &threat,
        &civilian.temperament,
        &ctx.cfg.reaction,
        allow_report,
    );
    match reaction {
        Reaction::Flee => flee(threat.at, witnessed(&threat), walker, ctx, rng),
        Reaction::Cower => CivilianState::Cower {
            from: threat.at,
            left: roll(rng, ctx.cfg.cower_seconds),
            about: witnessed(&threat),
        },
        Reaction::Report => CivilianState::Report {
            progress: 0.0,
            about: threat.cause,
        },
    }
}

fn flee(
    from: Vec3,
    about: Option<Cause>,
    walker: &mut GraphWalker,
    ctx: &Context,
    rng: &mut NpcRng,
) -> CivilianState {
    *walker = flee_start(ctx.graph, *walker, from);
    CivilianState::Flee {
        from,
        left: roll(rng, ctx.cfg.flee_distance),
        about,
    }
}

/// The crime a threat shows: a shot, a fight or a body; aim, a hit on oneself and cars are not phoned in later.
fn witnessed(threat: &Threat) -> Option<Cause> {
    match threat.kind {
        ThreatKind::Gunshot | ThreatKind::Fight | ThreatKind::Corpse => threat.cause,
        ThreatKind::Aimed | ThreatKind::Hurt | ThreatKind::Car => None,
    }
}

/// The crime a flight or crouch is already about, seen again (a body in sight): it runs its course.
fn already_fleeing(state: CivilianState, threat: &Threat) -> bool {
    let (CivilianState::Flee { about, .. } | CivilianState::Cower { about, .. }) = state else {
        return false;
    };
    about.is_some() && witnessed(threat) == about
}

/// A calmed-down witness of `about` starts a call with chance `call_after_flee`.
fn call_later(about: Option<Cause>, ctx: &Context, rng: &mut NpcRng) -> Option<CivilianState> {
    let about = about?;
    (rng.unit() < ctx.cfg.call_after_flee).then_some(CivilianState::Report {
        progress: 0.0,
        about: Some(about),
    })
}

/// Next state of a live civilian; `threat` is this tick's perception.
fn next_state(
    civilian: &Civilian,
    threat: Option<Threat>,
    walker: &mut GraphWalker,
    ctx: &Context,
    rng: &mut NpcRng,
) -> CivilianState {
    let cfg = ctx.cfg;
    let threat = threat.filter(|t| !already_fleeing(civilian.state, t));
    match (civilian.state, threat) {
        (CivilianState::Wander | CivilianState::Idle { .. }, Some(threat)) => {
            react(threat, civilian, walker, true, ctx, rng)
        }
        (CivilianState::Report { .. }, Some(threat)) => {
            react(threat, civilian, walker, false, ctx, rng)
        }
        // Commitment: a new threat only refreshes the running flight or crouch.
        (CivilianState::Flee { about, .. }, Some(threat)) => {
            flee(threat.at, witnessed(&threat).or(about), walker, ctx, rng)
        }
        (CivilianState::Cower { about, .. }, Some(threat)) => CivilianState::Cower {
            from: threat.at,
            left: roll(rng, cfg.cower_seconds),
            about: witnessed(&threat).or(about),
        },
        (CivilianState::Idle { left }, None) => {
            let left = left - ctx.dt;
            if left <= 0.0 {
                CivilianState::Wander
            } else {
                CivilianState::Idle { left }
            }
        }
        (CivilianState::Report { progress, about }, None) => {
            let progress = progress + ctx.dt / cfg.call_seconds;
            if progress >= 1.0 {
                CivilianState::Wander
            } else {
                CivilianState::Report { progress, about }
            }
        }
        (CivilianState::Flee { from, left, about }, None) => {
            let left = left - ctx.speed * ctx.dt;
            if left > 0.0 {
                return CivilianState::Flee { from, left, about };
            }
            call_later(about, ctx, rng).unwrap_or(CivilianState::Wander)
        }
        (CivilianState::Cower { from, left, about }, None) => {
            let left = left - ctx.dt;
            if left > 0.0 {
                return CivilianState::Cower { from, left, about };
            }
            call_later(about, ctx, rng).unwrap_or_else(|| flee(from, None, walker, ctx, rng))
        }
        (state @ CivilianState::Wander, None) | (state @ CivilianState::Dead, _) => state,
    }
}

/// Takes the next edge on arrival at the lane target of `to`; may stop a wanderer there.
fn arrive(
    state: CivilianState,
    walker: &mut GraphWalker,
    position: Vec3,
    nav: &NavigationConfig,
    ctx: &Context,
    rng: &mut NpcRng,
) -> CivilianState {
    let target = lane_target(ctx.graph, *walker, nav.keep_right);
    if flat_distance(position, target) > nav.arrive_radius {
        return state;
    }
    let next = match state {
        CivilianState::Wander => wander_next(ctx.graph, walker.from, walker.to, rng.unit()),
        CivilianState::Flee { from, .. } => flee_next(ctx.graph, walker.to, from),
        _ => return state,
    };
    *walker = GraphWalker {
        from: walker.to,
        to: next,
    };
    if state == CivilianState::Wander && rng.unit() < ctx.cfg.idle_chance {
        return CivilianState::Idle {
            left: roll(rng, ctx.cfg.idle_seconds),
        };
    }
    state
}

#[allow(clippy::type_complexity, clippy::too_many_arguments)]
fn civilian_fsm(
    cfg: Res<CivilianConfig>,
    nav: Res<NavigationConfig>,
    graph: Res<SidewalkGraph>,
    time: Res<Time<Fixed>>,
    mut rng: ResMut<NpcRng>,
    mut calls: MessageWriter<PoliceCall>,
    mut civilians: Query<(
        Entity,
        &mut Civilian,
        &mut Perception,
        &mut GraphWalker,
        &mut MoveIntent,
        &Position,
        &LinearVelocity,
    )>,
) {
    let dt = time.timestep().as_secs_f32();
    for (entity, mut civilian, mut perception, mut walker, mut intent, position, velocity) in
        &mut civilians
    {
        let threat = perception.pending.take();
        if civilian.state == CivilianState::Dead {
            continue;
        }
        let ctx = Context {
            cfg: &cfg,
            graph: &graph,
            dt,
            speed: Vec2::new(velocity.x, velocity.z).length(),
        };
        let state = next_state(&civilian, threat, &mut walker, &ctx, &mut rng);
        // Report -> Wander only by completion: an interrupted call goes through `react`, never to Wander.
        if let (
            CivilianState::Report {
                about: Some(about), ..
            },
            CivilianState::Wander,
        ) = (civilian.state, state)
        {
            calls.write(PoliceCall {
                caller: entity,
                about,
            });
        }
        let state = arrive(state, &mut walker, position.0, &nav, &ctx, &mut rng);
        civilian.state = state;
        let gait = match state {
            CivilianState::Wander => cfg.wander_gait,
            CivilianState::Flee { .. } => cfg.flee_gait,
            _ => {
                intent.axis = Vec2::ZERO;
                continue;
            }
        };
        intent.axis = Vec2::Y;
        intent.gait = gait;
        if let Some(yaw) = steer(position.0, lane_target(&graph, *walker, nav.keep_right)) {
            intent.yaw = yaw;
        }
    }
}
