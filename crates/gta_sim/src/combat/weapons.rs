use crate::character::{ActionIntent, Character, Dead, WeaponRequest};
use avian3d::prelude::*;
use bevy::prelude::*;
use serde::Deserialize;

/// Path of the weapons config, relative to the assets root.
pub const WEAPONS_CONFIG: &str = "combat/weapons.ron";

/// Order = index into `Loadout.guns`.
#[derive(Reflect, Clone, Copy, Debug, PartialEq, Eq, Deserialize)]
pub enum Weapon {
    Pistol,
    Smg,
    Shotgun,
}

impl Weapon {
    pub const ALL: [Weapon; 3] = [Weapon::Pistol, Weapon::Smg, Weapon::Shotgun];

    pub fn index(self) -> usize {
        self as usize
    }
}

#[derive(Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
pub enum FireMode {
    /// One shot per trigger press.
    SemiAutomatic,
    /// Fires while the trigger is held.
    Automatic,
}

/// Weapon table and the shooting range (GDD §4.1).
#[derive(Resource, Deserialize, Clone, Debug)]
#[serde(deny_unknown_fields)]
pub struct WeaponsConfig {
    pub headshot_multiplier: f32,
    pub pistol: WeaponStats,
    pub smg: WeaponStats,
    pub shotgun: WeaponStats,
    pub pickups: WeaponPickupConfig,
    pub range: RangeConfig,
}

#[derive(Deserialize, Clone, Debug)]
#[serde(deny_unknown_fields)]
pub struct WeaponStats {
    pub damage: f32,
    /// Half-width of the per-hit damage factor: 0.1 gives a factor in [0.9, 1.1].
    pub damage_variance: f32,
    pub pellets: u32,
    pub fire_mode: FireMode,
    /// Seconds between shots.
    pub fire_interval: f32,
    pub magazine: u32,
    /// Seconds.
    pub reload: f32,
    /// Metres.
    pub range: f32,
    pub falloff: Option<Falloff>,
    pub spread: SpreadConfig,
    pub max_reserve: u32,
    /// Rounds given by a pickup of this gun or its ammo.
    pub pickup_ammo: u32,
}

/// Damage factor is 1 up to `start` m, then falls linearly to `min_factor` at `range`.
#[derive(Deserialize, Clone, Copy, Debug)]
#[serde(deny_unknown_fields)]
pub struct Falloff {
    pub start: f32,
    pub min_factor: f32,
}

/// Cone half-angle in degrees = `base_deg + bloom + moving_deg_per_mps * speed`.
#[derive(Deserialize, Clone, Copy, Debug)]
#[serde(deny_unknown_fields)]
pub struct SpreadConfig {
    pub base_deg: f32,
    pub per_shot_deg: f32,
    pub max_bloom_deg: f32,
    /// Seconds after a shot before the bloom starts shrinking.
    pub recovery_delay: f32,
    pub recovery_deg_per_s: f32,
    pub moving_deg_per_mps: f32,
}

#[derive(Deserialize, Clone, Copy, Debug)]
#[serde(deny_unknown_fields)]
pub struct WeaponPickupConfig {
    pub radius: f32,
    /// Seconds until a taken pickup is available again.
    pub respawn: f32,
}

/// Target dummies and pickups in the central park.
#[derive(Deserialize, Clone, Copy, Debug)]
#[serde(deny_unknown_fields)]
pub struct RangeConfig {
    pub dummies: u32,
    pub dummy_spacing: f32,
    /// Distance of the dummy row from the park centre along −Z, m.
    pub dummy_distance: f32,
    pub pickup_spacing: f32,
    /// Seconds a dead dummy lies before it is back at full health.
    pub dummy_reset: f32,
}

fn check(ok: bool, field: &str) -> Result<(), String> {
    if ok {
        Ok(())
    } else {
        Err(format!("{field} is out of range"))
    }
}

impl WeaponStats {
    fn validate(&self, name: &str) -> Result<(), String> {
        let s = &self.spread;
        let falloff = self.falloff.map_or([0.0; 2], |f| [f.start, f.min_factor]);
        for (field, value) in [
            ("damage", self.damage),
            ("damage_variance", self.damage_variance),
            ("fire_interval", self.fire_interval),
            ("reload", self.reload),
            ("range", self.range),
            ("falloff.start", falloff[0]),
            ("falloff.min_factor", falloff[1]),
            ("spread.base_deg", s.base_deg),
            ("spread.per_shot_deg", s.per_shot_deg),
            ("spread.max_bloom_deg", s.max_bloom_deg),
            ("spread.recovery_delay", s.recovery_delay),
            ("spread.recovery_deg_per_s", s.recovery_deg_per_s),
            ("spread.moving_deg_per_mps", s.moving_deg_per_mps),
        ] {
            if !value.is_finite() {
                return Err(format!("{name}.{field} is not finite"));
            }
            check(value >= 0.0, &format!("{name}.{field}"))?;
        }
        let field = |f: &str| format!("{name}.{f}");
        check(self.damage > 0.0, &field("damage"))?;
        check(self.damage_variance < 1.0, &field("damage_variance"))?;
        check(self.pellets >= 1, &field("pellets"))?;
        check(self.fire_interval > 0.0, &field("fire_interval"))?;
        check(self.magazine >= 1, &field("magazine"))?;
        // A reload in progress is `reload_left > 0`: a zero reload would never transfer ammo.
        check(self.reload > 0.0, &field("reload"))?;
        check(self.range > 0.0, &field("range"))?;
        if let Some(f) = self.falloff {
            check(
                f.start > 0.0 && f.start < self.range,
                &field("falloff.start"),
            )?;
            check(f.min_factor <= 1.0, &field("falloff.min_factor"))?;
        }
        check(self.pickup_ammo >= 1, &field("pickup_ammo"))?;
        check(self.max_reserve >= self.pickup_ammo, &field("max_reserve"))
    }
}

impl WeaponsConfig {
    pub fn validate(&self) -> Result<(), String> {
        for weapon in Weapon::ALL {
            self.stats(weapon).validate(weapon_name(weapon))?;
        }
        let (p, r) = (&self.pickups, &self.range);
        for (field, value) in [
            ("headshot_multiplier", self.headshot_multiplier),
            ("pickups.radius", p.radius),
            ("pickups.respawn", p.respawn),
            ("range.dummy_spacing", r.dummy_spacing),
            ("range.dummy_distance", r.dummy_distance),
            ("range.pickup_spacing", r.pickup_spacing),
            ("range.dummy_reset", r.dummy_reset),
        ] {
            if !value.is_finite() {
                return Err(format!("{field} is not finite"));
            }
        }
        check(self.headshot_multiplier >= 1.0, "headshot_multiplier")?;
        check(p.radius > 0.0, "pickups.radius")?;
        check(p.respawn >= 0.0, "pickups.respawn")?;
        check(r.dummies >= 1, "range.dummies")?;
        check(r.dummy_spacing > 0.0, "range.dummy_spacing")?;
        check(r.dummy_distance > 0.0, "range.dummy_distance")?;
        check(r.dummy_reset > 0.0, "range.dummy_reset")?;
        // Pickups closer than their radius would be taken together.
        check(r.pickup_spacing > p.radius, "range.pickup_spacing")
    }

    pub fn stats(&self, weapon: Weapon) -> &WeaponStats {
        match weapon {
            Weapon::Pistol => &self.pistol,
            Weapon::Smg => &self.smg,
            Weapon::Shotgun => &self.shotgun,
        }
    }
}

fn weapon_name(weapon: Weapon) -> &'static str {
    match weapon {
        Weapon::Pistol => "pistol",
        Weapon::Smg => "smg",
        Weapon::Shotgun => "shotgun",
    }
}

/// Ammo and firing state of one gun; cooldown and bloom stay with the gun across switches.
#[derive(Reflect, Default, Clone, Copy, Debug, PartialEq)]
pub struct GunSlot {
    pub owned: bool,
    pub magazine: u32,
    pub reserve: u32,
    /// Seconds until this gun can fire again.
    pub cooldown: f32,
    pub bloom_deg: f32,
    /// Seconds after the last shot before the bloom starts shrinking.
    pub recovery_wait: f32,
}

/// Guns and firing state of a character; survives death (GDD §3.4).
#[derive(Component, Reflect, Default, Clone, Debug)]
#[reflect(Component, Default)]
pub struct Loadout {
    pub held: Option<Weapon>,
    /// Indexed by `Weapon::index`.
    pub guns: [GunSlot; 3],
    /// Seconds left of the current reload; 0 when not reloading.
    pub reload_left: f32,
    /// Current cone half-angle of the held gun; derived, written only by the weapon systems.
    pub spread_deg: f32,
}

impl Loadout {
    pub fn owned(&self) -> [bool; 3] {
        self.guns.map(|slot| slot.owned)
    }
}

/// Next weapon on the ring `[None, Pistol, Smg, Shotgun]` restricted to owned guns; `None` is
/// always on the ring.
pub fn cycle_weapon(held: Option<Weapon>, owned: [bool; 3], step: i32) -> Option<Weapon> {
    let ring: Vec<Option<Weapon>> = std::iter::once(None)
        .chain(
            Weapon::ALL
                .into_iter()
                .filter(|w| owned[w.index()])
                .map(Some),
        )
        .collect();
    let current = ring.iter().position(|w| *w == held).unwrap_or(0) as i32;
    let len = ring.len() as i32;
    ring[(current + step).rem_euclid(len) as usize]
}

/// Damage factor at `distance` m: 1 up to `falloff.start`, linear to `min_factor` at `range`.
pub fn falloff_factor(stats: &WeaponStats, distance: f32) -> f32 {
    let Some(f) = stats.falloff else {
        return 1.0;
    };
    let t = ((distance - f.start) / (stats.range - f.start)).clamp(0.0, 1.0);
    1.0 + (f.min_factor - 1.0) * t
}

/// Adds a picked-up gun (`gun = true`) or its ammo to `slot`; `false` if nothing changed.
pub fn acquire(slot: &mut GunSlot, stats: &WeaponStats, gun: bool) -> bool {
    if gun && !slot.owned {
        slot.owned = true;
        slot.magazine = stats.magazine.min(stats.pickup_ammo);
        slot.reserve = (stats.pickup_ammo - slot.magazine).min(stats.max_reserve);
        return true;
    }
    let reserve = (slot.reserve + stats.pickup_ammo).min(stats.max_reserve);
    let changed = reserve != slot.reserve;
    slot.reserve = reserve;
    changed
}

/// Damage of one hit: `round(base · (1 + variance · (2u − 1)))`, at least 1; `u ∈ [0, 1)`.
/// `base` already includes falloff and the head multiplier.
pub fn roll_damage(base: f32, variance: f32, u: f32) -> u32 {
    (base * (1.0 + variance * (2.0 * u - 1.0))).round().max(1.0) as u32
}

/// Recomputes the cone half-angle from the bloom and the horizontal speed.
pub(super) fn spread_deg(stats: &WeaponStats, bloom_deg: f32, velocity: Vec3) -> f32 {
    let speed = Vec2::new(velocity.x, velocity.z).length();
    stats.spread.base_deg + bloom_deg + stats.spread.moving_deg_per_mps * speed
}

/// Weapon selection, cooldown, reload and bloom recovery; runs right before `fire_weapons`.
#[allow(clippy::type_complexity)]
pub(super) fn tick_loadouts(
    cfg: Res<WeaponsConfig>,
    time: Res<Time<Fixed>>,
    mut loadouts: Query<
        (&mut Loadout, &mut ActionIntent, &LinearVelocity),
        (With<Character>, Without<Dead>),
    >,
) {
    let dt = time.delta_secs();
    for (mut loadout, mut action, velocity) in &mut loadouts {
        select_weapon(&mut loadout, &mut action);
        for weapon in Weapon::ALL {
            recover_gun(&mut loadout.guns[weapon.index()], cfg.stats(weapon), dt);
        }
        let Some(weapon) = loadout.held else {
            loadout.spread_deg = 0.0;
            action.reload_requested = false;
            continue;
        };
        let stats = cfg.stats(weapon);
        if loadout.reload_left > 0.0 {
            loadout.reload_left -= dt;
            if loadout.reload_left <= 0.0 {
                let slot = &mut loadout.guns[weapon.index()];
                let moved = (stats.magazine - slot.magazine).min(slot.reserve);
                slot.magazine += moved;
                slot.reserve -= moved;
                loadout.reload_left = 0.0;
            }
        }
        if std::mem::take(&mut action.reload_requested) {
            let slot = loadout.guns[weapon.index()];
            if loadout.reload_left <= 0.0 && slot.magazine < stats.magazine && slot.reserve > 0 {
                loadout.reload_left = stats.reload;
            }
        }
        let bloom = loadout.guns[weapon.index()].bloom_deg;
        loadout.spread_deg = spread_deg(stats, bloom, velocity.0);
    }
}

/// Cooldown and bloom recovery of one gun, held or not.
fn recover_gun(slot: &mut GunSlot, stats: &WeaponStats, dt: f32) {
    slot.cooldown = (slot.cooldown - dt).max(0.0);
    if slot.recovery_wait > 0.0 {
        slot.recovery_wait = (slot.recovery_wait - dt).max(0.0);
        if slot.recovery_wait > 0.0 {
            return;
        }
    }
    slot.bloom_deg = (slot.bloom_deg - stats.spread.recovery_deg_per_s * dt).max(0.0);
}

fn select_weapon(loadout: &mut Loadout, action: &mut ActionIntent) {
    let owned = loadout.owned();
    let mut held = loadout.held;
    if let Some(request) = action.select.take() {
        held = match request {
            WeaponRequest::Unarmed => None,
            WeaponRequest::Gun(gun) if owned[gun.index()] => Some(gun),
            WeaponRequest::Gun(_) => held,
        };
    }
    let step = std::mem::take(&mut action.cycle);
    if step != 0 {
        held = cycle_weapon(held, owned, step);
    }
    if held != loadout.held {
        loadout.held = held;
        loadout.reload_left = 0.0;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn shotgun() -> WeaponStats {
        WeaponStats {
            damage: 8.0,
            damage_variance: 0.1,
            pellets: 10,
            fire_mode: FireMode::SemiAutomatic,
            fire_interval: 0.9,
            magazine: 6,
            reload: 2.5,
            range: 25.0,
            falloff: Some(Falloff {
                start: 10.0,
                min_factor: 0.3,
            }),
            spread: SpreadConfig {
                base_deg: 6.0,
                per_shot_deg: 2.0,
                max_bloom_deg: 3.0,
                recovery_delay: 1.0,
                recovery_deg_per_s: 4.0,
                moving_deg_per_mps: 0.2,
            },
            max_reserve: 48,
            pickup_ammo: 12,
        }
    }

    #[test]
    fn cycle_ring_over_owned_guns() {
        let owned = [true, false, true];
        assert_eq!(cycle_weapon(None, owned, 1), Some(Weapon::Pistol));
        assert_eq!(
            cycle_weapon(Some(Weapon::Pistol), owned, 1),
            Some(Weapon::Shotgun)
        );
        assert_eq!(cycle_weapon(Some(Weapon::Shotgun), owned, 1), None);
        assert_eq!(cycle_weapon(None, owned, -1), Some(Weapon::Shotgun));
        assert_eq!(cycle_weapon(None, [false; 3], 1), None);
    }

    #[test]
    fn shotgun_falloff() {
        let stats = shotgun();
        for (distance, expected) in [
            (5.0, 1.0),
            (10.0, 1.0),
            (17.5, 0.65),
            (25.0, 0.3),
            (30.0, 0.3),
        ] {
            let got = falloff_factor(&stats, distance);
            assert!((got - expected).abs() < 1e-5, "{distance}: {got}");
        }
    }

    #[test]
    fn acquire_gun_then_ammo() {
        let stats = shotgun();
        let mut slot = GunSlot::default();
        assert!(acquire(&mut slot, &stats, true));
        assert_eq!(
            slot,
            GunSlot {
                owned: true,
                magazine: 6,
                reserve: 6,
                ..default()
            }
        );
        assert!(acquire(&mut slot, &stats, false));
        assert_eq!(slot.reserve, 18);
        assert!(
            acquire(&mut slot, &stats, true),
            "a second gun pickup gives its ammo"
        );
        assert_eq!(slot.reserve, 30);
        slot.reserve = 48;
        assert!(
            !acquire(&mut slot, &stats, false),
            "a full reserve takes nothing"
        );
        let mut ammo_only = GunSlot::default();
        assert!(acquire(&mut ammo_only, &stats, false));
        assert_eq!(
            ammo_only,
            GunSlot {
                owned: false,
                magazine: 0,
                reserve: 12,
                ..default()
            }
        );
    }

    #[test]
    fn roll_damage_table() {
        for ((base, variance, u), expected) in [
            ((25.0, 0.1, 0.0), 23),
            ((25.0, 0.1, 0.5), 25),
            ((50.0, 0.1, 0.0), 45),
            ((8.0, 0.1, 0.0), 7),
            ((8.0, 0.1, 0.999_999), 9),
            ((25.0, 0.0, 0.0), 25),
            ((25.0, 0.0, 0.73), 25),
            ((0.4, 0.1, 0.0), 1),
        ] {
            assert_eq!(
                roll_damage(base, variance, u),
                expected,
                "({base}, {variance}, {u})"
            );
        }
        let top = roll_damage(25.0, 0.1, 0.999_999);
        assert!((27..=28).contains(&top), "{top}");
    }
}
