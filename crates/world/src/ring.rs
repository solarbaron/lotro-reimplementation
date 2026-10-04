// SPDX-License-Identifier: GPL-3.0-or-later
//! Streaming rings with hysteresis (ARCH §9.2).
//!
//! The [`RingTracker`] keeps the set of landblocks that should be resident around a focus
//! point, classified into concentric rings by the distance from the focus to the nearest
//! point of each landblock square:
//!
//! | Ring | Default radius | Meant for |
//! |---|---|---|
//! | [`Ring::Near`] | 480 units (3 blocks) | full terrain, static objects, collision |
//! | [`Ring::Mid`] | 960 units | terrain and large objects |
//! | [`Ring::Far`] | 1,600 units | distant terrain |
//!
//! A landblock **enters** when its distance drops to the far radius and **leaves** only
//! when it exceeds the far radius plus the hysteresis margin (one landblock by default), so
//! a focus moving back and forth across a boundary does not thrash. Demotion to an outer
//! ring uses the same margin: a member moving outwards lands in the innermost ring whose
//! radius plus the margin still contains it, so two members at the same distance can
//! differ by at most "not demoted yet". Promotion is immediate. The radii are settings,
//! not constants.
//!
//! An update visits the current members plus the candidate square of side 2 · far around
//! the focus (widened by one landblock on the west and south sides, so a block whose east
//! or north edge lies exactly at the far radius is found like its mirror image), a few
//! thousand landblocks at most with [`MAX_RING_RADIUS`], so it is cheap enough for the main
//! thread every frame.

use std::collections::HashMap;

use crate::coords::{LANDBLOCK_SIZE, LandblockKey, MAX_BLOCK, WorldPos};
use crate::error::{Result, WorldError};

/// Largest accepted far radius plus hysteresis, in world units (40 landblocks).
pub const MAX_RING_RADIUS: f64 = 40.0 * LANDBLOCK_SIZE;

/// A streaming ring. Ordered from the focus outwards.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Ring {
    /// Closest: everything.
    Near,
    /// Middle: terrain and large objects.
    Mid,
    /// Outermost: distant terrain.
    Far,
}

impl Ring {
    /// All rings, inside out.
    pub const ALL: [Ring; 3] = [Ring::Near, Ring::Mid, Ring::Far];

    /// Short lowercase name.
    #[must_use]
    pub fn name(self) -> &'static str {
        match self {
            Self::Near => "near",
            Self::Mid => "mid",
            Self::Far => "far",
        }
    }
}

/// Ring radii in world units, measured from the focus to the nearest point of a
/// landblock's square.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RingConfig {
    /// Outer radius of the near ring.
    pub near: f64,
    /// Outer radius of the mid ring.
    pub mid: f64,
    /// Outer radius of the far ring: landblocks within it are desired.
    pub far: f64,
    /// Slack before a landblock is demoted or released.
    pub hysteresis: f64,
}

impl Default for RingConfig {
    fn default() -> Self {
        Self {
            near: 3.0 * LANDBLOCK_SIZE,
            mid: 6.0 * LANDBLOCK_SIZE,
            far: 10.0 * LANDBLOCK_SIZE,
            hysteresis: LANDBLOCK_SIZE,
        }
    }
}

impl RingConfig {
    /// Checks 0 ≤ near ≤ mid ≤ far, hysteresis ≥ 0, all finite, and far + hysteresis ≤
    /// [`MAX_RING_RADIUS`].
    ///
    /// # Errors
    ///
    /// [`WorldError::Config`] naming the first violated rule.
    pub fn validate(&self) -> Result<()> {
        let all = [self.near, self.mid, self.far, self.hysteresis];
        if !all.iter().all(|v| v.is_finite() && *v >= 0.0) {
            return Err(WorldError::Config(
                "ring radii must be finite and non-negative",
            ));
        }
        if !(self.near <= self.mid && self.mid <= self.far) {
            return Err(WorldError::Config(
                "ring radii must satisfy near <= mid <= far",
            ));
        }
        if self.far + self.hysteresis > MAX_RING_RADIUS {
            return Err(WorldError::Config(
                "far radius plus hysteresis is too large",
            ));
        }
        Ok(())
    }

    /// The ring for distance `d` without hysteresis, or `None` beyond the far radius.
    #[must_use]
    pub fn ring_for(&self, d: f64) -> Option<Ring> {
        if d <= self.near {
            Some(Ring::Near)
        } else if d <= self.mid {
            Some(Ring::Mid)
        } else if d <= self.far {
            Some(Ring::Far)
        } else {
            None
        }
    }

    /// The ring of a member currently in `current` at distance `d`, with hysteresis:
    /// promotion is immediate, demotion needs `d` beyond the ring's radius plus the
    /// margin, and `None` (release) needs `d` beyond far plus the margin.
    #[must_use]
    pub fn ring_with_hysteresis(&self, current: Ring, d: f64) -> Option<Ring> {
        if !d.is_finite() || d > self.far + self.hysteresis {
            return None;
        }
        let plain = self.ring_for(d).unwrap_or(Ring::Far);
        if plain <= current {
            return Some(plain);
        }
        // Moving outwards: the innermost ring from `current` outwards whose radius plus the
        // margin still contains `d` (Far always does: d <= far + hysteresis above).
        if current <= Ring::Near && d <= self.near + self.hysteresis {
            Some(Ring::Near)
        } else if current <= Ring::Mid && d <= self.mid + self.hysteresis {
            Some(Ring::Mid)
        } else {
            Some(Ring::Far)
        }
    }
}

/// A landblock entering the desired set.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Entered {
    /// The landblock.
    pub key: LandblockKey,
    /// Its ring.
    pub ring: Ring,
    /// Distance from the focus to its square.
    pub distance: f64,
}

/// What one [`RingTracker::update`] changed.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct RingDiff {
    /// Newly desired landblocks, nearest first.
    pub entered: Vec<Entered>,
    /// Members whose ring changed: (key, old, new).
    pub changed: Vec<(LandblockKey, Ring, Ring)>,
    /// Members released (beyond far + hysteresis, or in another region).
    pub left: Vec<LandblockKey>,
}

impl RingDiff {
    /// Whether nothing changed.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.entered.is_empty() && self.changed.is_empty() && self.left.is_empty()
    }
}

/// The desired landblock set around a moving focus (module docs).
#[derive(Debug, Clone)]
pub struct RingTracker {
    config: RingConfig,
    region: Option<u8>,
    members: HashMap<LandblockKey, Ring>,
}

impl RingTracker {
    /// An empty tracker.
    ///
    /// # Errors
    ///
    /// [`RingConfig::validate`]'s.
    pub fn new(config: RingConfig) -> Result<Self> {
        config.validate()?;
        Ok(Self {
            config,
            region: None,
            members: HashMap::new(),
        })
    }

    /// The configuration.
    #[must_use]
    pub fn config(&self) -> &RingConfig {
        &self.config
    }

    /// Current members and their rings.
    #[must_use]
    pub fn members(&self) -> &HashMap<LandblockKey, Ring> {
        &self.members
    }

    /// The ring of `key`, if it is a member.
    #[must_use]
    pub fn ring_of(&self, key: LandblockKey) -> Option<Ring> {
        self.members.get(&key).copied()
    }

    /// Releases every member.
    pub fn clear(&mut self) -> RingDiff {
        let mut left: Vec<LandblockKey> = self.members.drain().map(|(k, _)| k).collect();
        left.sort_unstable();
        self.region = None;
        RingDiff {
            left,
            ..RingDiff::default()
        }
    }

    /// Moves the focus to `focus` and returns what changed. A non-finite focus, or one in
    /// an invalid region, releases everything.
    pub fn update(&mut self, focus: &WorldPos) -> RingDiff {
        if !focus.is_finite() || LandblockKey::new(focus.region, 0, 0).is_err() {
            return self.clear();
        }
        let mut diff = RingDiff::default();
        if self.region != Some(focus.region) {
            diff = self.clear();
            self.region = Some(focus.region);
        }
        let g = focus.xy();
        let cfg = self.config;

        // Existing members: re-ring or release.
        let mut left = Vec::new();
        for (key, ring) in &mut self.members {
            let d = key.distance_to(g);
            match cfg.ring_with_hysteresis(*ring, d) {
                None => left.push(*key),
                Some(r) if r != *ring => {
                    diff.changed.push((*key, *ring, r));
                    *ring = r;
                }
                Some(_) => {}
            }
        }
        for key in &left {
            self.members.remove(key);
        }
        left.sort_unstable();
        diff.left.extend(left);
        diff.changed.sort_unstable_by_key(|c| c.0);

        // Candidates within the far radius. A block whose east (north) edge is exactly at
        // distance `far` has index ceil((c - far) / 160) - 1; `ring_for` filters the extra
        // column when the division is not exact.
        let reach = cfg.far;
        let range = |c: f64| -> Option<(u8, u8)> {
            let lo = ((c - reach) / LANDBLOCK_SIZE).ceil() - 1.0;
            let hi = ((c + reach) / LANDBLOCK_SIZE).floor();
            let max = f64::from(MAX_BLOCK);
            if hi < 0.0 || lo > max {
                return None;
            }
            // Clamped into 0..=254.
            #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
            Some((lo.clamp(0.0, max) as u8, hi.clamp(0.0, max) as u8))
        };
        if let (Some((x0, x1)), Some((y0, y1))) = (range(g[0]), range(g[1])) {
            for y in y0..=y1 {
                for x in x0..=x1 {
                    let Ok(key) = LandblockKey::new(focus.region, x, y) else {
                        continue;
                    };
                    if self.members.contains_key(&key) {
                        continue;
                    }
                    let d = key.distance_to(g);
                    if let Some(ring) = cfg.ring_for(d) {
                        self.members.insert(key, ring);
                        diff.entered.push(Entered {
                            key,
                            ring,
                            distance: d,
                        });
                    }
                }
            }
        }
        diff.entered
            .sort_unstable_by(|a, b| a.distance.total_cmp(&b.distance).then(a.key.cmp(&b.key)));
        diff
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tracker() -> RingTracker {
        RingTracker::new(RingConfig {
            near: 160.0,
            mid: 320.0,
            far: 480.0,
            hysteresis: 160.0,
        })
        .unwrap()
    }

    fn at(x: f64, y: f64) -> WorldPos {
        WorldPos::new(1, x, y, 0.0)
    }

    #[test]
    fn config_validation() {
        assert!(RingConfig::default().validate().is_ok());
        let bad = |c: RingConfig| c.validate().is_err();
        let d = RingConfig::default();
        assert!(bad(RingConfig { near: -1.0, ..d }));
        assert!(bad(RingConfig { mid: 1.0, ..d }));
        assert!(bad(RingConfig { far: f64::NAN, ..d }));
        assert!(bad(RingConfig { far: 1e9, ..d }));
        assert!(bad(RingConfig {
            hysteresis: f64::INFINITY,
            ..d
        }));
    }

    #[test]
    fn hysteresis_rules() {
        let c = tracker().config;
        assert_eq!(c.ring_with_hysteresis(Ring::Near, 200.0), Some(Ring::Near));
        assert_eq!(c.ring_with_hysteresis(Ring::Near, 310.0), Some(Ring::Near));
        // Demoted from Near at 330: Mid, like a Mid member at the same distance.
        assert_eq!(c.ring_with_hysteresis(Ring::Near, 330.0), Some(Ring::Mid));
        assert_eq!(c.ring_with_hysteresis(Ring::Mid, 330.0), Some(Ring::Mid));
        assert_eq!(c.ring_with_hysteresis(Ring::Near, 480.0), Some(Ring::Mid));
        assert_eq!(c.ring_with_hysteresis(Ring::Near, 481.0), Some(Ring::Far));
        assert_eq!(c.ring_with_hysteresis(Ring::Mid, 470.0), Some(Ring::Mid));
        assert_eq!(c.ring_with_hysteresis(Ring::Mid, 490.0), Some(Ring::Far));
        assert_eq!(c.ring_with_hysteresis(Ring::Mid, 100.0), Some(Ring::Near));
        assert_eq!(c.ring_with_hysteresis(Ring::Near, 500.0), Some(Ring::Far));
        assert_eq!(c.ring_with_hysteresis(Ring::Far, 100.0), Some(Ring::Near));
        assert_eq!(c.ring_with_hysteresis(Ring::Far, 600.0), Some(Ring::Far));
        assert_eq!(c.ring_with_hysteresis(Ring::Far, 641.0), None);
        assert_eq!(c.ring_with_hysteresis(Ring::Mid, f64::NAN), None);
    }

    #[test]
    fn enter_change_leave() {
        let mut t = tracker();
        let d = t.update(&at(16_080.0, 16_080.0)); // centre of block (100, 100)
        assert!(d.left.is_empty() && d.changed.is_empty());
        // Far radius 480 from a centre point: blocks within 3 blocks plus corners.
        assert_eq!(d.entered[0].key, LandblockKey::new(1, 100, 100).unwrap());
        assert_eq!(d.entered[0].ring, Ring::Near);
        assert!(d.entered.windows(2).all(|w| w[0].distance <= w[1].distance));
        let n = t.members().len();
        assert_eq!(n, d.entered.len());
        assert!(
            t.members()
                .keys()
                .all(|k| k.chebyshev(LandblockKey::new(1, 100, 100).unwrap()) <= Some(3))
        );

        // Same focus: no change.
        assert!(t.update(&at(16_080.0, 16_080.0)).is_empty());

        // Move one block east: new blocks enter on the east side, none leave yet.
        let d = t.update(&at(16_240.0, 16_080.0));
        assert!(!d.entered.is_empty());
        assert!(d.left.is_empty(), "hysteresis keeps the west edge");
        assert!(d.entered.iter().all(|e| e.key.x() >= 101));

        // Move back: nothing enters (still members), nothing leaves.
        let d = t.update(&at(16_080.0, 16_080.0));
        assert!(d.entered.is_empty() && d.left.is_empty());

        // Move far away: everything leaves and a fresh set enters.
        let before = t.members().len();
        assert!(before > n);
        let d = t.update(&at(32_080.0, 32_080.0));
        assert_eq!(d.left.len(), before);
        assert_eq!(d.entered.len(), n);
    }

    #[test]
    fn candidates_are_symmetric_at_the_far_radius() {
        // Focus G = (16,480, 16,080): block (99, 100) has its east edge at 16,000 and block
        // (106, 100) its west edge at 16,960, both exactly 480 away.
        let mut t = tracker();
        t.update(&at(16_480.0, 16_080.0));
        let k = |x, y| LandblockKey::new(1, x, y).unwrap();
        assert_eq!(t.ring_of(k(99, 100)), Some(Ring::Far));
        assert_eq!(t.ring_of(k(106, 100)), Some(Ring::Far));
        assert_eq!(t.ring_of(k(98, 100)), None);
        assert_eq!(t.ring_of(k(107, 100)), None);
        // Same on the y axis.
        let mut t = tracker();
        t.update(&at(16_080.0, 16_480.0));
        assert_eq!(t.ring_of(k(100, 99)), Some(Ring::Far));
        assert_eq!(t.ring_of(k(100, 106)), Some(Ring::Far));
        // The desired set is mirror-symmetric about the focus.
        let mut t = tracker();
        t.update(&at(16_480.0, 16_480.0));
        // Block x covers [160 x, 160 x + 160]; its mirror about 16,480 is block 205 - x.
        for key in t.members().keys() {
            let m = k(205 - key.x(), 205 - key.y());
            assert_eq!(t.ring_of(m), t.ring_of(*key), "mirror of {key}");
        }
    }

    #[test]
    fn same_distance_same_ring_after_demotion() {
        let c = tracker().config;
        // Beyond near + hysteresis a Near member is demoted to the ring a Mid member at the
        // same distance has.
        for d in [321.0, 330.0, 400.0, 480.0, 481.0, 500.0, 640.0, 641.0] {
            assert_eq!(
                c.ring_with_hysteresis(Ring::Near, d),
                c.ring_with_hysteresis(Ring::Mid, d),
                "{d}"
            );
        }
    }

    #[test]
    fn oscillation_does_not_thrash() {
        let mut t = tracker();
        t.update(&at(16_150.0, 16_080.0));
        let mut churn = 0;
        for i in 0..100 {
            let x = if i % 2 == 0 { 16_170.0 } else { 16_150.0 };
            let d = t.update(&at(x, 16_080.0));
            churn += d.entered.len() + d.left.len();
        }
        // The first crossing may add a column; afterwards nothing moves.
        assert!(churn <= 7, "churn {churn}");
    }

    #[test]
    fn region_change_and_bad_focus() {
        let mut t = tracker();
        t.update(&at(100.0, 100.0));
        let n = t.members().len();
        assert!(n > 0);
        // Corner of the grid: candidates are clamped.
        assert!(t.members().keys().all(|k| k.x() <= 3 && k.y() <= 3));
        let d = t.update(&WorldPos::new(2, 100.0, 100.0, 0.0));
        assert_eq!(d.left.len(), n);
        assert_eq!(d.entered.len(), n);
        let d = t.update(&WorldPos::new(2, f64::NAN, 0.0, 0.0));
        assert_eq!(d.left.len(), n);
        assert!(t.members().is_empty());
        let d = t.update(&WorldPos::new(0, 1.0, 1.0, 0.0));
        assert!(d.is_empty());
        // Far outside the grid: nothing desired.
        let d = t.update(&WorldPos::new(1, -10_000.0, 1e6, 0.0));
        assert!(d.entered.is_empty());
    }
}
