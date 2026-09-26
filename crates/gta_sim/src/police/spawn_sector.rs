//! Police cars spawning ahead of and beside a fleeing driver, not only behind (GDD §5.3, TASK-032): a
//! deliberate cheat, by data shares of the three sectors around the driven car's heading.

use bevy::prelude::*;
use serde::Deserialize;

/// Sector shares (sum 1) and bounds: ahead within `ahead_deg` of the heading, behind past
/// `behind_deg`, beside between.
#[derive(Deserialize, Clone, Debug)]
#[serde(deny_unknown_fields)]
pub struct SectorConfig {
    pub ahead: f32,
    pub beside: f32,
    pub behind: f32,
    pub ahead_deg: f32,
    pub behind_deg: f32,
}

/// Sector indices of `PoliceDispatcher::sector_spawns`.
pub const AHEAD: usize = 0;
pub const BESIDE: usize = 1;
pub const BEHIND: usize = 2;

impl SectorConfig {
    pub fn validate(&self) -> Result<(), String> {
        let shares = [self.ahead, self.beside, self.behind];
        if shares.iter().any(|s| !(s.is_finite() && *s >= 0.0))
            || (shares.iter().sum::<f32>() - 1.0).abs() > 1e-3
        {
            return Err(format!(
                "car.spawn_sectors shares {shares:?} must be finite, >= 0 and sum to 1"
            ));
        }
        if !(0.0 < self.ahead_deg && self.ahead_deg < self.behind_deg && self.behind_deg < 180.0) {
            return Err(format!(
                "car.spawn_sectors must satisfy 0 < ahead_deg {} < behind_deg {} < 180",
                self.ahead_deg, self.behind_deg
            ));
        }
        Ok(())
    }

    fn shares(&self) -> [f32; 3] {
        [self.ahead, self.beside, self.behind]
    }

    /// Sector of a spawn point at `offset` from the driver heading `heading` (flat).
    pub fn sector(&self, offset: Vec3, heading: Vec3) -> usize {
        let (a, b) = (offset.with_y(0.0), heading.with_y(0.0));
        let angle = a.angle_between(b).to_degrees();
        if angle <= self.ahead_deg {
            AHEAD
        } else if angle > self.behind_deg {
            BEHIND
        } else {
            BESIDE
        }
    }

    /// Sectors by deficit against their shares after `counts` spawns (largest share x (n + 1) -
    /// count first; no random draw).
    pub fn order(&self, counts: [u32; 3]) -> [usize; 3] {
        let n = counts.iter().sum::<u32>() as f32;
        let deficit = |k: usize| self.shares()[k] * (n + 1.0) - counts[k] as f32;
        let mut order = [AHEAD, BESIDE, BEHIND];
        order.sort_by(|&a, &b| deficit(b).total_cmp(&deficit(a)).then(a.cmp(&b)));
        order
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cfg() -> SectorConfig {
        SectorConfig {
            ahead: 0.3,
            beside: 0.3,
            behind: 0.4,
            ahead_deg: 45.0,
            behind_deg: 135.0,
        }
    }

    #[test]
    fn sector_rows() {
        let c = cfg();
        let heading = Vec3::NEG_Z;
        assert_eq!(c.sector(Vec3::new(0.0, 0.0, -40.0), heading), AHEAD);
        assert_eq!(c.sector(Vec3::new(40.0, 0.0, -10.0), heading), BESIDE);
        assert_eq!(c.sector(Vec3::new(-40.0, 0.0, 0.0), heading), BESIDE);
        assert_eq!(c.sector(Vec3::new(5.0, 0.0, 40.0), heading), BEHIND);
    }

    #[test]
    fn quota_rows() {
        let c = cfg();
        // Nothing spawned: behind has the largest share.
        assert_eq!(c.order([0, 0, 0])[0], BEHIND);
        // One behind: ahead (0.6 - 0) before beside (0.6 - 0, ties by index) before behind (0.8 - 1).
        assert_eq!(c.order([0, 0, 1]), [AHEAD, BESIDE, BEHIND]);
        assert_eq!(c.order([1, 0, 1])[0], BESIDE);
        // Ten spawns at the shares: each sector keeps its quota.
        let mut counts = [0; 3];
        for _ in 0..10 {
            counts[c.order(counts)[0]] += 1;
        }
        assert_eq!(counts, [3, 3, 4]);
    }
}
