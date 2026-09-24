use super::AttackSerial;
use super::hitscan::DamageDealt;
use super::weapons::Loadout;
use crate::character::{
    ActionIntent, AimIntent, Character, CharacterBody, CharacterScheme, CharacterSchemeActionState,
    Dead, Health,
};
use crate::gang::{Faction, GangConfig};
use crate::layers::GameLayer;
use crate::player::Player;
use avian3d::prelude::*;
use bevy::prelude::*;
use bevy_tnua::builtins::{
    TnuaBuiltinKnockback, TnuaBuiltinKnockbackConfig, TnuaBuiltinKnockbackMemory,
};
use bevy_tnua::prelude::*;
use serde::Deserialize;

/// Path of the melee config, relative to the assets root.
pub const MELEE_CONFIG: &str = "combat/melee.ron";

/// Fist combo, bat swing, attack windows and hit reactions (GDD §4.2).
#[derive(Resource, Deserialize, Clone, Debug)]
#[serde(deny_unknown_fields)]
pub struct MeleeConfig {
    /// Radius of the sphere swept from the chest, m.
    pub cast_radius: f32,
    /// Height of the sweep above the feet, m.
    pub cast_height: f32,
    /// Seconds after a swing ends in which a click continues the combo.
    pub combo_window: f32,
    /// Share of the gait speed kept while swinging.
    pub swing_move_scale: f32,
    /// Seconds a hit victim is staggered.
    pub stagger: f32,
    /// Seconds a knocked-down victim lies.
    pub knockdown: f32,
    pub fists: MeleeWeaponStats,
    pub bat: MeleeWeaponStats,
    pub knockback_tuning: KnockbackTuning,
}

#[derive(Deserialize, Clone, Debug)]
#[serde(deny_unknown_fields)]
pub struct MeleeWeaponStats {
    /// Sweep length of the sphere, m.
    pub range: f32,
    /// Combo steps in order; a single entry is a single swing.
    pub hits: Vec<MeleeHitStats>,
}

/// One swing: times in seconds from its start.
#[derive(Deserialize, Clone, Debug)]
#[serde(deny_unknown_fields)]
pub struct MeleeHitStats {
    /// Exactly the amount passed to `Health::take`.
    pub damage: u32,
    pub duration: f32,
    pub active_from: f32,
    pub active_to: f32,
    /// Shove speed along the blow, m/s.
    pub knockback: f32,
    pub knockdown: bool,
}

/// Tnua pushover after a shove (`TnuaBuiltinKnockbackConfig`).
#[derive(Deserialize, Clone, Copy, Debug)]
#[serde(deny_unknown_fields)]
pub struct KnockbackTuning {
    pub no_push_timeout: f32,
    pub barrier_strength_diminishing: f32,
    pub acceleration_limit: f32,
    pub air_acceleration_limit: f32,
}

impl KnockbackTuning {
    pub fn tnua(&self) -> TnuaBuiltinKnockbackConfig {
        TnuaBuiltinKnockbackConfig {
            no_push_timeout: self.no_push_timeout,
            barrier_strength_diminishing: self.barrier_strength_diminishing,
            acceleration_limit: self.acceleration_limit,
            air_acceleration_limit: self.air_acceleration_limit,
        }
    }
}

fn check(ok: bool, field: &str) -> Result<(), String> {
    if ok {
        Ok(())
    } else {
        Err(format!("{field} is out of range"))
    }
}

fn finite(field: &str, value: f32) -> Result<(), String> {
    if value.is_finite() {
        Ok(())
    } else {
        Err(format!("{field} is not finite"))
    }
}

impl MeleeWeaponStats {
    fn validate(&self, name: &str) -> Result<(), String> {
        finite(&format!("{name}.range"), self.range)?;
        check(self.range > 0.0, &format!("{name}.range"))?;
        if self.hits.is_empty() {
            return Err(format!("{name}.hits is empty"));
        }
        for (i, hit) in self.hits.iter().enumerate() {
            let field = |f: &str| format!("{name}.hits[{i}].{f}");
            for (f, value) in [
                ("duration", hit.duration),
                ("active_from", hit.active_from),
                ("active_to", hit.active_to),
                ("knockback", hit.knockback),
            ] {
                finite(&field(f), value)?;
            }
            check(hit.damage >= 1, &field("damage"))?;
            check(
                hit.active_from >= 0.0 && hit.active_from < hit.active_to,
                &field("active_from"),
            )?;
            check(hit.active_to < hit.duration, &field("active_to"))?;
            check(hit.knockback >= 0.0, &field("knockback"))?;
        }
        Ok(())
    }
}

impl MeleeConfig {
    pub fn validate(&self) -> Result<(), String> {
        let t = &self.knockback_tuning;
        let tuning = [
            ("knockback_tuning.no_push_timeout", t.no_push_timeout),
            (
                "knockback_tuning.barrier_strength_diminishing",
                t.barrier_strength_diminishing,
            ),
            ("knockback_tuning.acceleration_limit", t.acceleration_limit),
            (
                "knockback_tuning.air_acceleration_limit",
                t.air_acceleration_limit,
            ),
        ];
        for (field, value) in [
            ("cast_radius", self.cast_radius),
            ("cast_height", self.cast_height),
            ("combo_window", self.combo_window),
            ("swing_move_scale", self.swing_move_scale),
            ("stagger", self.stagger),
            ("knockdown", self.knockdown),
        ]
        .into_iter()
        .chain(tuning)
        {
            finite(field, value)?;
        }
        for (field, value) in [
            ("cast_radius", self.cast_radius),
            ("cast_height", self.cast_height),
            ("stagger", self.stagger),
            ("knockdown", self.knockdown),
        ]
        .into_iter()
        .chain(tuning)
        {
            check(value > 0.0, field)?;
        }
        check(self.combo_window >= 0.0, "combo_window")?;
        check(
            (0.0..=1.0).contains(&self.swing_move_scale),
            "swing_move_scale",
        )?;
        self.fists.validate("fists")?;
        self.bat.validate("bat")
    }

    pub fn stats(&self, weapon: MeleeWeapon) -> &MeleeWeaponStats {
        match weapon {
            MeleeWeapon::Fists => &self.fists,
            MeleeWeapon::Bat => &self.bat,
        }
    }

    pub fn hit(&self, weapon: MeleeWeapon, step: u8) -> &MeleeHitStats {
        let stats = self.stats(weapon);
        &stats.hits[step as usize % stats.hits.len()]
    }
}

/// What empty hands swing.
#[derive(Reflect, Default, Clone, Copy, Debug, PartialEq, Eq, Deserialize)]
pub enum MeleeWeapon {
    #[default]
    Fists,
    Bat,
}

impl MeleeWeapon {
    pub fn other(self) -> Self {
        match self {
            Self::Fists => Self::Bat,
            Self::Bat => Self::Fists,
        }
    }
}

/// Melee state of a character.
#[derive(Component, Reflect, Default, Clone, Debug)]
#[reflect(Component, Default)]
pub struct Melee {
    /// The running swing, if any.
    pub swing: Option<Swing>,
    /// Combo step the next swing takes while `combo_left > 0`.
    pub next_step: u8,
    /// Seconds left in which a click continues the combo.
    pub combo_left: f32,
    /// A click during the running swing starts the next one when it ends.
    pub queued: bool,
}

impl Melee {
    pub fn cancel(&mut self) {
        *self = Self::default();
    }
}

/// One swing in progress.
#[derive(Reflect, Clone, Copy, Debug, PartialEq)]
pub struct Swing {
    pub weapon: MeleeWeapon,
    /// Combo step (index into the weapon's hits).
    pub step: u8,
    /// Seconds since the swing started.
    pub elapsed: f32,
    /// Seconds the swing lasts (copied from the config at the start).
    pub duration: f32,
    /// Horizontal unit direction of the blow.
    pub direction: Vec3,
    /// The sweep hit something; the swing deals no more damage.
    pub landed: bool,
    /// Attack id carried in `DamageDealt.shot`.
    pub attack: u32,
}

/// How a character reacts to melee hits.
#[derive(Component, Reflect, Default, Clone, Copy, Debug, PartialEq)]
#[reflect(Component, Default)]
pub enum HitReaction {
    #[default]
    Steady,
    /// Cannot move or attack for `left` seconds.
    Staggered { left: f32 },
    /// Lying for `left` seconds; the head hitbox is off.
    KnockedDown { left: f32 },
}

impl HitReaction {
    pub fn is_active(&self) -> bool {
        *self != Self::Steady
    }

    pub fn is_knocked_down(&self) -> bool {
        matches!(self, Self::KnockedDown { .. })
    }

    /// A knockdown always (re)starts; a stagger never shortens a knockdown.
    pub fn escalate(&mut self, knockdown: bool, cfg: &MeleeConfig) {
        if knockdown {
            *self = Self::KnockedDown {
                left: cfg.knockdown,
            };
        } else if !self.is_knocked_down() {
            *self = Self::Staggered { left: cfg.stagger };
        }
    }
}

/// One applied melee hit (presentation: hit-stop, shake).
#[derive(Message, Reflect, Clone, Copy, Debug)]
#[reflect(Message)]
pub struct MeleeHit {
    pub attacker: Entity,
    pub target: Entity,
    pub point: Vec3,
    pub knockdown: bool,
}

/// A sweep that reached a live target; applied in the same fixed tick.
#[derive(Message, Clone, Copy, Debug)]
pub(super) struct Strike {
    attacker: Entity,
    target: Entity,
    point: Vec3,
    direction: Vec3,
    damage: u32,
    knockback: f32,
    knockdown: bool,
    attack: u32,
}

/// Advances the swing state by one tick; `true` while the attack window is open and the swing has
/// not landed yet.
pub fn advance_swing(
    melee: &mut Melee,
    clicked: bool,
    weapon: MeleeWeapon,
    direction: Vec3,
    next_attack: impl FnOnce() -> u32,
    dt: f32,
    cfg: &MeleeConfig,
) -> bool {
    if let Some(swing) = melee.swing.as_mut() {
        melee.queued |= clicked;
        swing.elapsed += dt;
        if swing.elapsed < swing.duration {
            return window_open(swing, cfg);
        }
        // The end tick sets the full combo window; it starts counting down on the next idle tick.
        let hits = cfg.stats(swing.weapon).hits.len();
        melee.next_step = ((swing.step as usize + 1) % hits) as u8;
        melee.combo_left = cfg.combo_window;
        melee.swing = None;
        if !std::mem::take(&mut melee.queued) {
            return false;
        }
    } else {
        melee.combo_left = (melee.combo_left - dt).max(0.0);
        if !clicked {
            return false;
        }
    }
    let step = if melee.combo_left > 0.0 {
        (melee.next_step as usize % cfg.stats(weapon).hits.len()) as u8
    } else {
        0
    };
    let swing = Swing {
        weapon,
        step,
        elapsed: 0.0,
        duration: cfg.hit(weapon, step).duration,
        direction,
        landed: false,
        attack: next_attack(),
    };
    melee.swing = Some(swing);
    window_open(&swing, cfg)
}

fn window_open(swing: &Swing, cfg: &MeleeConfig) -> bool {
    let hit = cfg.hit(swing.weapon, swing.step);
    !swing.landed && hit.active_from <= swing.elapsed && swing.elapsed <= hit.active_to
}

/// Counts stagger and knockdown down (dead characters too, so a stale reaction heals itself).
pub(super) fn recover_from_hits(time: Res<Time<Fixed>>, mut reactions: Query<&mut HitReaction>) {
    let dt = time.delta_secs();
    for mut reaction in &mut reactions {
        let (left, knocked_down) = match *reaction {
            HitReaction::Steady => continue,
            HitReaction::Staggered { left } => (left - dt, false),
            HitReaction::KnockedDown { left } => (left - dt, true),
        };
        *reaction = match (left > 0.0, knocked_down) {
            (false, _) => HitReaction::Steady,
            (true, false) => HitReaction::Staggered { left },
            (true, true) => HitReaction::KnockedDown { left },
        };
    }
}

/// Keeps the direction of a start overlap out of float noise: a body exactly beside the swing.
const INTO_TOLERANCE: f32 = 1e-3;

/// A body met on the way, or one the sphere already overlaps at the start that the swing drives into.
/// The sphere is wider than the capsule, so a wall or character touching the attacker's side or back
/// overlaps it at the start while the swing moves along or away from it.
fn swing_reaches(hit: &ShapeHitData, dir: Dir3) -> bool {
    hit.distance > 0.0 || hit.normal1.dot(*dir) < -INTO_TOLERANCE
}

/// Swings of unarmed characters: advances the swing and sweeps the sphere while the window is open.
#[allow(clippy::type_complexity, clippy::too_many_arguments)]
pub(super) fn swing_melee(
    cfg: Res<MeleeConfig>,
    time: Res<Time<Fixed>>,
    spatial: SpatialQuery,
    mut serial: ResMut<AttackSerial>,
    mut strikes: MessageWriter<Strike>,
    mut attackers: Query<
        (
            Entity,
            &Position,
            &Rotation,
            &CharacterBody,
            &AimIntent,
            &mut ActionIntent,
            &Loadout,
            &mut Melee,
            &HitReaction,
        ),
        (With<Character>, Without<Dead>),
    >,
    colliders: Query<&ColliderOf>,
    targets: Query<(), (With<Character>, With<Health>, Without<Dead>)>,
) {
    let dt = time.delta_secs();
    let shape = Collider::sphere(cfg.cast_radius);
    for (attacker, position, rotation, body, aim, mut action, loadout, mut melee, reaction) in
        &mut attackers
    {
        // A held gun owns the click (`fire_weapons`).
        if loadout.held.is_some() {
            melee.cancel();
            continue;
        }
        if reaction.is_active() {
            melee.cancel();
            action.fire_requested = false;
            continue;
        }
        if melee.swing.is_some_and(|s| s.weapon != loadout.melee) {
            melee.cancel();
        }
        let clicked = std::mem::take(&mut action.fire_requested);
        let flat = |v: Vec3| Vec3::new(v.x, 0.0, v.z).normalize_or_zero();
        let mut direction = flat(aim.direction);
        if direction == Vec3::ZERO {
            direction = flat(rotation.0 * Vec3::NEG_Z);
        }
        let weapon = loadout.melee;
        if !advance_swing(
            &mut melee,
            clicked,
            weapon,
            direction,
            || serial.next_id(),
            dt,
            &cfg,
        ) {
            continue;
        }
        let Some(swing) = melee.swing.as_mut() else {
            continue;
        };
        let Ok(dir) = Dir3::new(swing.direction) else {
            continue;
        };
        let origin = position.0 + Vec3::Y * (cfg.cast_height - body.float_height);
        // Explicit mask: head sensors on `Hitbox` are not melee targets.
        let filter = SpatialQueryFilter::from_mask([GameLayer::World, GameLayer::Character])
            .with_excluded_entities([attacker]);
        let reach = ShapeCastConfig::from_max_distance(cfg.stats(swing.weapon).range);
        let mut nearest: Option<ShapeHitData> = None;
        spatial.shape_hits_callback(
            &shape,
            origin,
            Quat::IDENTITY,
            dir,
            &reach,
            &filter,
            |hit| {
                if swing_reaches(&hit, dir) && nearest.is_none_or(|n| hit.distance < n.distance) {
                    nearest = Some(hit);
                }
                true
            },
        );
        let Some(hit) = nearest else {
            continue;
        };
        swing.landed = true;
        let target = colliders.get(hit.entity).map_or(hit.entity, |of| of.body);
        if !targets.contains(target) {
            continue;
        }
        let stats = cfg.hit(swing.weapon, swing.step);
        strikes.write(Strike {
            attacker,
            target,
            point: hit.point1,
            direction: swing.direction,
            damage: stats.damage,
            knockback: stats.knockback,
            knockdown: stats.knockdown,
            attack: swing.attack,
        });
    }
}

/// Applies strikes in message order: damage, reaction, shove, then the messages of an applied hit.
/// A gang member's blow spares whoever its gang is not hostile to, judged at impact.
pub(super) fn apply_strikes(
    cfg: Res<MeleeConfig>,
    gangs: Res<GangConfig>,
    factions: Query<&Faction>,
    mut strikes: MessageReader<Strike>,
    mut victims: Query<
        (
            &mut Health,
            &mut HitReaction,
            &mut TnuaController<CharacterScheme>,
        ),
        Without<Dead>,
    >,
    mut dealt: MessageWriter<DamageDealt>,
    mut hits: MessageWriter<MeleeHit>,
) {
    for strike in strikes.read() {
        let faction = |e: Entity| factions.get(e).ok().copied();
        if gangs.spares(faction(strike.attacker), faction(strike.target)) {
            continue;
        }
        let Ok((mut health, mut reaction, mut controller)) = victims.get_mut(strike.target) else {
            continue;
        };
        // `current > 0` covers a target killed earlier this tick whose `Dead` is still deferred.
        if health.current <= 0.0 {
            continue;
        }
        let killed = health.take(strike.damage as f32);
        reaction.escalate(strike.knockdown, &cfg);
        knock_back(&mut controller, strike.direction * strike.knockback);
        dealt.write(DamageDealt {
            shooter: strike.attacker,
            shot: strike.attack,
            target: strike.target,
            point: strike.point,
            damage: strike.damage,
            headshot: false,
            killed,
        });
        hits.write(MeleeHit {
            attacker: strike.attacker,
            target: strike.target,
            point: strike.point,
            knockdown: strike.knockdown,
        });
    }
}

/// Shoves the character by `shove` m/s through Tnua's knockback action; it turns to face the blow.
pub fn knock_back(controller: &mut TnuaController<CharacterScheme>, shove: Vec3) {
    let Ok(forward) = Dir3::new(-shove) else {
        return;
    };
    // An interrupt on a running knockback only swaps its input; without the reset the new shove is dropped.
    if let Some(CharacterSchemeActionState::Knockback(state)) = controller.current_action.as_mut() {
        state.memory = TnuaBuiltinKnockbackMemory::Shove;
    }
    controller.action_interrupt(CharacterScheme::Knockback(TnuaBuiltinKnockback {
        shove,
        force_forward: Some(forward),
    }));
}

/// A knockdown or buffered swing from before death must not continue at the hospital.
pub(super) fn reset_player_melee(mut players: Query<(&mut Melee, &mut HitReaction), With<Player>>) {
    for (mut melee, mut reaction) in &mut players {
        *melee = Melee::default();
        *reaction = HitReaction::Steady;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const DT: f32 = 1.0 / 64.0;

    fn cfg() -> MeleeConfig {
        ron::from_str(include_str!("../../../../assets/combat/melee.ron"))
            .expect("GATE BROKEN: shipped melee.ron does not parse")
    }

    /// Runs ticks `0..ticks` of `weapon` with clicks on `clicks`; returns the window flag per tick.
    fn run(
        melee: &mut Melee,
        weapon: MeleeWeapon,
        clicks: &[u32],
        ticks: std::ops::Range<u32>,
        cfg: &MeleeConfig,
    ) -> Vec<bool> {
        let mut serial = 0;
        ticks
            .map(|k| {
                advance_swing(
                    melee,
                    clicks.contains(&k),
                    weapon,
                    Vec3::NEG_Z,
                    || {
                        serial += 1;
                        serial
                    },
                    DT,
                    cfg,
                )
            })
            .collect()
    }

    fn open_ticks(flags: &[bool], first: u32) -> Vec<u32> {
        flags
            .iter()
            .enumerate()
            .filter(|(_, open)| **open)
            .map(|(i, _)| first + i as u32)
            .collect()
    }

    #[test]
    fn shipped_config_validates() {
        cfg().validate().unwrap();
    }

    #[test]
    fn single_jab_window_and_end() {
        let cfg = cfg();
        let mut melee = Melee::default();
        let flags = run(&mut melee, MeleeWeapon::Fists, &[0], 0..23, &cfg);
        assert_eq!(open_ticks(&flags, 0), (8..=14).collect::<Vec<_>>());
        assert!(melee.swing.is_some());
        let end = run(&mut melee, MeleeWeapon::Fists, &[], 23..24, &cfg);
        assert_eq!(end, [false]);
        assert_eq!(melee.swing, None);
        assert_eq!(
            melee.combo_left, 0.4,
            "set on the end tick, not decremented"
        );
        assert_eq!(melee.next_step, 1);
    }

    #[test]
    fn queued_clicks_chain_the_combo() {
        let cfg = cfg();
        let mut melee = Melee::default();
        let flags = run(&mut melee, MeleeWeapon::Fists, &[0, 10, 30], 0..70, &cfg);
        let open = open_ticks(&flags, 0);
        let expected: Vec<u32> = (8..=14).chain(31..=37).chain(54..=60).collect();
        assert_eq!(open, expected);
        assert_eq!(melee.swing, None, "third swing ends at 69");
        assert_eq!(melee.next_step, 0);

        let mut melee = Melee::default();
        run(&mut melee, MeleeWeapon::Fists, &[0, 10], 0..24, &cfg);
        let swing = melee.swing.expect("second swing starts on the end tick 23");
        assert_eq!((swing.step, swing.elapsed), (1, 0.0));
        run(&mut melee, MeleeWeapon::Fists, &[30], 24..47, &cfg);
        let swing = melee.swing.expect("third swing starts at 46");
        assert_eq!((swing.step, swing.elapsed), (2, 0.0));
        assert!(cfg.hit(MeleeWeapon::Fists, 2).knockdown);
    }

    #[test]
    fn combo_grace_boundary() {
        let cfg = cfg();
        for (click, step) in [(24, 1), (48, 1), (49, 0)] {
            let mut melee = Melee::default();
            run(
                &mut melee,
                MeleeWeapon::Fists,
                &[0, click],
                0..click + 1,
                &cfg,
            );
            let swing = melee.swing.expect("a click while idle starts a swing");
            assert_eq!(swing.step, step, "click at {click}");
        }
    }

    #[test]
    fn landed_swing_closes_the_window() {
        let cfg = cfg();
        let mut melee = Melee::default();
        let flags = run(&mut melee, MeleeWeapon::Fists, &[0], 0..9, &cfg);
        assert!(flags[8]);
        melee.swing.as_mut().unwrap().landed = true;
        assert_eq!(
            run(&mut melee, MeleeWeapon::Fists, &[], 9..10, &cfg),
            [false]
        );
    }

    #[test]
    fn two_clicks_in_one_swing_queue_one() {
        let cfg = cfg();
        let mut melee = Melee::default();
        run(&mut melee, MeleeWeapon::Fists, &[0, 5, 6], 0..24, &cfg);
        assert_eq!(melee.swing.map(|s| s.step), Some(1), "swing 2 at 23");
        run(&mut melee, MeleeWeapon::Fists, &[], 24..47, &cfg);
        assert_eq!(melee.swing, None, "nothing left queued at 46");
    }

    #[test]
    fn bat_window_and_end() {
        let cfg = cfg();
        let mut melee = Melee::default();
        let flags = run(&mut melee, MeleeWeapon::Bat, &[0], 0..39, &cfg);
        assert_eq!(open_ticks(&flags, 0), (13..=20).collect::<Vec<_>>());
        assert!(melee.swing.is_some());
        run(&mut melee, MeleeWeapon::Bat, &[], 39..40, &cfg);
        assert_eq!(melee.swing, None);
    }

    #[test]
    fn escalate_table() {
        let cfg = cfg();
        let mut reaction = HitReaction::KnockedDown { left: 0.5 };
        reaction.escalate(false, &cfg);
        assert_eq!(reaction, HitReaction::KnockedDown { left: 0.5 });
        let mut reaction = HitReaction::Staggered { left: 0.1 };
        reaction.escalate(true, &cfg);
        assert_eq!(reaction, HitReaction::KnockedDown { left: 1.2 });
        let mut reaction = HitReaction::Steady;
        reaction.escalate(false, &cfg);
        assert_eq!(reaction, HitReaction::Staggered { left: 0.35 });
    }
}
