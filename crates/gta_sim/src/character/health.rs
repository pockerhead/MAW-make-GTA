use bevy::prelude::*;
use serde::Deserialize;

/// Path of the health config, relative to the assets root.
pub const HEALTH_CONFIG: &str = "character/health.ron";

/// Health, armour and regeneration tuning (GDD §3.4).
#[derive(Resource, Deserialize, Clone, Debug)]
#[serde(deny_unknown_fields)]
pub struct HealthConfig {
    pub max_health: f32,
    pub max_armor: f32,
    /// Seconds without damage before regeneration starts.
    pub regen_delay: f32,
    /// Health points per second.
    pub regen_rate: f32,
    /// Regeneration stops at this fraction of `max_health`.
    pub regen_cap: f32,
    /// Damage of one debug hit (F5 in the `debug` client).
    pub debug_damage: f32,
    pub pickups: PickupConfig,
}

/// Medkit and armour pickups.
#[derive(Deserialize, Clone, Copy, Debug)]
#[serde(deny_unknown_fields)]
pub struct PickupConfig {
    pub health: f32,
    pub armor: f32,
    pub radius: f32,
    /// Seconds until a taken pickup is available again.
    pub respawn: f32,
    /// Distance of each pickup from the respawn point along the sidewalk (m).
    pub spacing: f32,
}

fn check(ok: bool, field: &str) -> Result<(), String> {
    if ok {
        Ok(())
    } else {
        Err(format!("{field} is out of range"))
    }
}

impl HealthConfig {
    pub fn validate(&self) -> Result<(), String> {
        let p = &self.pickups;
        for (field, value) in [
            ("max_health", self.max_health),
            ("max_armor", self.max_armor),
            ("regen_delay", self.regen_delay),
            ("regen_rate", self.regen_rate),
            ("regen_cap", self.regen_cap),
            ("debug_damage", self.debug_damage),
            ("pickups.health", p.health),
            ("pickups.armor", p.armor),
            ("pickups.radius", p.radius),
            ("pickups.respawn", p.respawn),
            ("pickups.spacing", p.spacing),
        ] {
            if !value.is_finite() {
                return Err(format!("{field} is not finite"));
            }
        }
        check(self.max_health > 0.0, "max_health")?;
        check(self.max_armor > 0.0, "max_armor")?;
        check(self.regen_cap > 0.0 && self.regen_cap <= 1.0, "regen_cap")?;
        check(self.regen_delay >= 0.0, "regen_delay")?;
        check(self.regen_rate >= 0.0, "regen_rate")?;
        check(self.debug_damage > 0.0, "debug_damage")?;
        check(p.health > 0.0, "pickups.health")?;
        check(p.armor > 0.0, "pickups.armor")?;
        check(p.radius > 0.0, "pickups.radius")?;
        check(p.respawn >= 0.0, "pickups.respawn")?;
        // A pickup within reach of the respawn point would be taken on the first tick of `Playing`.
        if p.spacing <= p.radius {
            return Err("pickups.spacing must exceed pickups.radius".into());
        }
        Ok(())
    }
}

#[derive(Component, Reflect, Clone, Copy, Debug, PartialEq)]
#[reflect(Component)]
pub struct Health {
    pub current: f32,
    pub armor: f32,
    /// Seconds since the last damage.
    pub since_damage: f32,
}

impl Health {
    /// Full health, no armour (GDD §3.4).
    pub fn full(cfg: &HealthConfig) -> Self {
        Self {
            current: cfg.max_health,
            armor: 0.0,
            since_damage: 0.0,
        }
    }
}

/// Marks a character whose health reached zero; removed on respawn.
#[derive(Component, Reflect, Default)]
#[reflect(Component)]
pub struct Dead;

/// Armour absorbs damage first; returns `(current, armor)` after `amount` of damage.
pub fn apply_damage(current: f32, armor: f32, amount: f32) -> (f32, f32) {
    let absorbed = armor.min(amount);
    ((current - (amount - absorbed)).max(0.0), armor - absorbed)
}

/// Health after `dt` of regeneration: only below the cap, only after `regen_delay`, never past the cap.
pub fn regenerate(current: f32, since_damage: f32, dt: f32, cfg: &HealthConfig) -> f32 {
    let cap = cfg.max_health * cfg.regen_cap;
    if current <= 0.0 || current >= cap || since_damage < cfg.regen_delay {
        return current;
    }
    (current + cfg.regen_rate * dt).min(cap)
}

/// Order of the health steps inside one fixed tick.
#[derive(SystemSet, Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum HealthSystems {
    Damage,
    Regen,
    Pickup,
    Death,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cfg() -> HealthConfig {
        HealthConfig {
            max_health: 100.0,
            max_armor: 100.0,
            regen_delay: 5.0,
            regen_rate: 5.0,
            regen_cap: 0.5,
            debug_damage: 25.0,
            pickups: PickupConfig {
                health: 50.0,
                armor: 50.0,
                radius: 1.0,
                respawn: 30.0,
                spacing: 6.0,
            },
        }
    }

    #[test]
    fn damage_table() {
        for ((current, armor, amount), expected) in [
            ((100.0, 50.0, 30.0), (100.0, 20.0)),
            ((100.0, 20.0, 30.0), (90.0, 0.0)),
            ((100.0, 0.0, 30.0), (70.0, 0.0)),
            ((10.0, 0.0, 30.0), (0.0, 0.0)),
            ((100.0, 50.0, 150.0), (0.0, 0.0)),
        ] {
            assert_eq!(
                apply_damage(current, armor, amount),
                expected,
                "({current}, {armor}, {amount})"
            );
        }
    }

    #[test]
    fn regen_table() {
        let cfg = cfg();
        let dt = 1.0 / 64.0;
        for ((current, since), expected) in [
            ((30.0, 4.99), 30.0),
            ((30.0, 5.0), 30.078125),
            ((49.99, 5.0), 50.0),
            ((60.0, 10.0), 60.0),
            ((0.0, 10.0), 0.0),
        ] {
            let got = regenerate(current, since, dt, &cfg);
            assert!(
                (got - expected).abs() < 1e-5,
                "({current}, {since}): {got}, expected {expected}"
            );
        }
    }
}
