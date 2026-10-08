// SPDX-License-Identifier: GPL-3.0-or-later
//! The frill ring: the set of frill sectors kept around the ring centre, separate from the
//! landblock ring (`scenery.md` §8.2–§8.3).
//!
//! - **Ring centre** ([`ring_centre`], [`ring_centre_of`]; §8.2, S47, and its Tessera
//!   Decision): the camera position C when there is no player or the blend
//!   `FrillEngine.PlayerPositionBlend` is 0 or less (−∞ included); otherwise
//!   C + t · (P − C), not clamped (a NaN or +∞ blend counts as 1); with the blend above 0
//!   and camera and player in different regions, the player's position.
//! - **Sector set** (§8.2, S47): the client's square, every sector whose column and row each
//!   differ from the centre sector's by at most `radius + extra` (Chebyshev distance),
//!   inside the region grid. Above [`MAX_RING_RADIUS`] the square is capped, never below
//!   [`far_reach_sectors`] of the far distance D (only non-default settings get there).
//! - **Order**: new sectors are handed out in the client's order, ring by ring outward
//!   (ring r = the sectors at Chebyshev distance r); each ring starts at its south-west
//!   corner and runs north along its west column, east along its north row, south along
//!   its east column and west along its south row. (The Tessera Decision of §8.2 leaves the
//!   order free; it changes only when a sector appears.)
//! - **Drop policy** (Tessera's, allowed by that Decision): a held sector is dropped once it
//!   is more than one sector outside the square (hysteresis), or when the centre moves to
//!   another region or off the grid. Every sector of the square is held while the centre
//!   is still.

use std::collections::BTreeSet;

use super::sector::FrillSectorKey;
use super::settings::{FRILL_SECTOR_SIZE, FrillSettings};
use crate::coords::WorldPos;

/// Radius cap in sectors for the client's square of `radius + extra` sectors (the client's
/// largest level is 16, plus the pre-cache rings). The cap never cuts below
/// [`far_reach_sectors`] of the far distance D, so a capped ring still holds every sector
/// within D of the centre sector, which the Tessera Decision of `scenery.md` §8.2 accepts
/// in place of the square (implementer note in `scenery.md` §13.2).
pub const MAX_RING_RADIUS: u32 = 64;

/// The Chebyshev radius in sectors of the smallest square that holds every sector with some
/// point within `far` units of some point of the centre sector (`scenery.md` §8.2 Tessera
/// Decision, the distance between the two squares, boundary included): a sector at
/// Chebyshev offset k lies at least (k − 1) · S away, so k ≤ ⌊D / S⌋ + 1. A distance that
/// is not finite or not above 0 gives 0.
#[must_use]
pub fn far_reach_sectors(far: f32) -> u32 {
    if !far.is_finite() || far <= 0.0 {
        return 0;
    }
    let k = (f64::from(far) / f64::from(FRILL_SECTOR_SIZE)).floor() + 1.0;
    // Bounded above, so the conversion cannot overflow; D is at most 1,000 units (§8.3).
    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
    let k = k.min(f64::from(u16::MAX)) as u32;
    k
}

/// What changed in one ring update.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct FrillRingDiff {
    /// Sectors to generate, in the client's ring order (module docs).
    pub entered: Vec<FrillSectorKey>,
    /// Sectors to drop.
    pub left: Vec<FrillSectorKey>,
}

/// The frill sectors held around a moving centre.
#[derive(Debug, Clone, Default)]
pub struct FrillRing {
    radius: u32,
    held: BTreeSet<FrillSectorKey>,
    centre: Option<FrillSectorKey>,
}

/// The ring centre with a player (§8.2): [`ring_centre_of`] with `Some(player)`.
#[must_use]
pub fn ring_centre(camera: &WorldPos, player: &WorldPos, blend: f32) -> WorldPos {
    ring_centre_of(camera, Some(player), blend)
}

/// The ring centre of `scenery.md` §8.2 (BIN, S47) and its Tessera Decision:
/// - no player (`None`, for example the free-fly camera): the camera position;
/// - a blend that is NaN or +∞ counts as 1; a blend of 0 or less (−∞ included): the camera
///   position;
/// - otherwise, with camera and player in different regions: the player's position;
/// - otherwise camera + blend · (player − camera) on each axis, in the camera's region; the
///   blend is not clamped (2 puts the centre as far beyond the player as the camera is
///   behind it).
#[must_use]
pub fn ring_centre_of(camera: &WorldPos, player: Option<&WorldPos>, blend: f32) -> WorldPos {
    let Some(player) = player else {
        return *camera;
    };
    let t = if blend.is_nan() || blend == f32::INFINITY {
        1.0
    } else {
        f64::from(blend)
    };
    if t <= 0.0 {
        return *camera;
    }
    if camera.region != player.region {
        return *player;
    }
    WorldPos::new(
        camera.region,
        camera.x + t * (player.x - camera.x),
        camera.y + t * (player.y - camera.y),
        camera.z + t * (player.z - camera.z),
    )
}

impl FrillRing {
    /// A ring of `radius + extra_precache_levels` sectors for `settings` (§8.3 rule 1,
    /// §8.4): the client's square. When that exceeds [`MAX_RING_RADIUS`] the radius is
    /// capped there, but never below [`far_reach_sectors`] of the far distance D (§8.3
    /// rule 2), so the ring always holds either the client's square or every sector within
    /// D of the centre sector (§8.2 Tessera Decision). A radius of 0 holds nothing.
    #[must_use]
    pub fn new(settings: &FrillSettings) -> Self {
        let r = settings.radius_sectors();
        let radius = if r == 0 {
            0
        } else {
            let cap = MAX_RING_RADIUS.max(far_reach_sectors(settings.distances().far));
            r.saturating_add(settings.extra_precache_levels).min(cap)
        };
        Self {
            radius,
            held: BTreeSet::new(),
            centre: None,
        }
    }

    /// The generation radius in sectors.
    #[must_use]
    pub fn radius(&self) -> u32 {
        self.radius
    }

    /// The sectors held.
    pub fn sectors(&self) -> impl Iterator<Item = &FrillSectorKey> + '_ {
        self.held.iter()
    }

    /// Number of sectors held.
    #[must_use]
    pub fn len(&self) -> usize {
        self.held.len()
    }

    /// Whether nothing is held.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.held.is_empty()
    }

    /// The current centre sector.
    #[must_use]
    pub fn centre(&self) -> Option<FrillSectorKey> {
        self.centre
    }

    /// Moves the ring to `centre`. A centre outside the region grid drops everything.
    pub fn update(&mut self, centre: &WorldPos) -> FrillRingDiff {
        let mut diff = FrillRingDiff::default();
        let Some(c) = FrillSectorKey::at(centre) else {
            diff.left = std::mem::take(&mut self.held).into_iter().collect();
            self.centre = None;
            return diff;
        };
        self.centre = Some(c);
        let keep = i64::from(self.radius) + 1;
        let left: Vec<FrillSectorKey> = self
            .held
            .iter()
            .filter(|k| k.region != c.region || chebyshev(k, c) > keep)
            .copied()
            .collect();
        for k in &left {
            self.held.remove(k);
        }
        diff.left = left;
        let entered: Vec<FrillSectorKey> = wanted(c, self.radius)
            .into_iter()
            .filter(|k| !self.held.contains(k))
            .collect();
        self.held.extend(entered.iter().copied());
        diff.entered = entered;
        diff
    }
}

/// Chebyshev distance in sectors between `k` and `c` (same region assumed).
fn chebyshev(k: &FrillSectorKey, c: FrillSectorKey) -> i64 {
    let dx = i64::from(k.gx) - i64::from(c.gx);
    let dy = i64::from(k.gy) - i64::from(c.gy);
    dx.abs().max(dy.abs())
}

/// The client's square of `radius` sectors around `c` (§8.2), inside the region grid, in
/// the client's order: ring 0, then each ring r from its south-west corner north along the
/// west column, east along the north row, south along the east column and west along the
/// south row.
fn wanted(c: FrillSectorKey, radius: u32) -> Vec<FrillSectorKey> {
    if radius == 0 {
        return Vec::new();
    }
    let r = i64::from(radius);
    let mut out = Vec::new();
    let mut push = |dx: i64, dy: i64| {
        let (Ok(gx), Ok(gy)) = (
            u16::try_from(i64::from(c.gx) + dx),
            u16::try_from(i64::from(c.gy) + dy),
        ) else {
            return;
        };
        let k = FrillSectorKey {
            region: c.region,
            gx,
            gy,
        };
        if k.landblock().is_some() {
            out.push(k);
        }
    };
    push(0, 0);
    for ring in 1..=r {
        // West column, south to north (from the south-west corner).
        for dy in -ring..=ring {
            push(-ring, dy);
        }
        // North row, west to east (after the north-west corner).
        for dx in (-ring + 1)..=ring {
            push(dx, ring);
        }
        // East column, north to south (after the north-east corner).
        for dy in (-ring..ring).rev() {
            push(ring, dy);
        }
        // South row, east to west (after the south-east corner, up to the south-west one).
        for dx in ((-ring + 1)..ring).rev() {
            push(dx, -ring);
        }
    }
    out
}
