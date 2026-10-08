// SPDX-License-Identifier: GPL-3.0-or-later
//! The frill ring: the set of frill sectors kept around the ring centre, separate from the
//! landblock ring (`scenery.md` §8.2–§8.3).
//!
//! The ring centre lies between the camera and the player by `PlayerPositionBlend`
//! (Probable, §8.2). Its shape is Tessera's (the spec gives the radius, not the shape):
//! every sector whose centre is within `radius + extra` sectors of the centre sector's
//! centre is wanted; a held sector is dropped once it is more than one further sector
//! away (hysteresis). New sectors are handed out nearest first, ring by ring around the
//! centre sector (the client's spiral order).

use std::collections::BTreeSet;

use super::sector::FrillSectorKey;
use super::settings::FrillSettings;
use crate::coords::WorldPos;

/// Largest radius in sectors the ring accepts (the client's largest level is 16, plus the
/// pre-cache rings).
pub const MAX_RING_RADIUS: u32 = 64;

/// What changed in one ring update.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct FrillRingDiff {
    /// Sectors to generate, nearest first.
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

/// The ring centre: camera + blend · (player − camera) (§8.2, Probable), in the camera's
/// region. A non-finite blend counts as 1.
#[must_use]
pub fn ring_centre(camera: &WorldPos, player: &WorldPos, blend: f32) -> WorldPos {
    let t = if blend.is_finite() {
        f64::from(blend)
    } else {
        1.0
    };
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
    /// §8.4), capped at [`MAX_RING_RADIUS`]. A radius of 0 holds nothing.
    #[must_use]
    pub fn new(settings: &FrillSettings) -> Self {
        let r = settings.radius_sectors();
        let radius = if r == 0 {
            0
        } else {
            r.saturating_add(settings.extra_precache_levels)
                .min(MAX_RING_RADIUS)
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
        let dist2 = |k: &FrillSectorKey| {
            let dx = i64::from(k.gx) - i64::from(c.gx);
            let dy = i64::from(k.gy) - i64::from(c.gy);
            dx * dx + dy * dy
        };
        let left: Vec<FrillSectorKey> = self
            .held
            .iter()
            .filter(|k| k.region != c.region || dist2(k) > keep * keep)
            .copied()
            .collect();
        for k in &left {
            self.held.remove(k);
        }
        diff.left = left;
        let mut entered: Vec<FrillSectorKey> = wanted(c, self.radius)
            .into_iter()
            .filter(|k| !self.held.contains(k))
            .collect();
        entered.sort_by_key(|k| {
            let dx = i64::from(k.gx) - i64::from(c.gx);
            let dy = i64::from(k.gy) - i64::from(c.gy);
            (dx.abs().max(dy.abs()), dist2(k), k.gy, k.gx)
        });
        self.held.extend(entered.iter().copied());
        diff.entered = entered;
        diff
    }
}

/// The sectors within `radius` sectors (centre to centre) of `c`, inside the region grid.
fn wanted(c: FrillSectorKey, radius: u32) -> Vec<FrillSectorKey> {
    if radius == 0 {
        return Vec::new();
    }
    let r = i64::from(radius);
    let mut out = Vec::new();
    for dy in -r..=r {
        for dx in -r..=r {
            if dx * dx + dy * dy > r * r {
                continue;
            }
            let (Ok(gx), Ok(gy)) = (
                u16::try_from(i64::from(c.gx) + dx),
                u16::try_from(i64::from(c.gy) + dy),
            ) else {
                continue;
            };
            let k = FrillSectorKey {
                region: c.region,
                gx,
                gy,
            };
            if k.landblock().is_some() {
                out.push(k);
            }
        }
    }
    out
}
