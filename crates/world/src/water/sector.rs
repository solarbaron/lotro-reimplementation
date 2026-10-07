// SPDX-License-Identifier: GPL-3.0-or-later
//! Distant-landscape sector water (`water.md` §9): the (type, *h*) pairs of a 3 × 3 group
//! of landblocks, derived from the block maps with §6.2 (the Tessera rule there) instead of
//! decoding the kind-5 records.

use tessera_formats::landblock::{SECTOR_SIDE, SectorWaterPair};

use super::decide::LandblockWater;
use crate::coords::LandblockKey;

/// The landblock at the south-west corner of `key`'s distant sector: X and Y rounded down
/// to multiples of 3 (`water.md` §9).
#[must_use]
pub fn sector_origin(key: LandblockKey) -> LandblockKey {
    let snap = |c: u8| c - c % SECTOR_SIDE;
    // Rounding down keeps X and Y within 0–254, so the key is always valid.
    LandblockKey::new(key.region(), snap(key.x()), snap(key.y())).unwrap_or(key)
}

/// The water of one distant sector (`water.md` §9): a list of distinct (type, *h*) pairs in
/// the order their first landblock is met (X-major, west to east, then south to north
/// within a column), and per landblock the index of its pair, or `None` when dry.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct SectorWater {
    /// The sector's south-west landblock.
    pub origin: Option<LandblockKey>,
    /// Distinct pairs.
    pub pairs: Vec<SectorWaterPair>,
    /// `blocks[dx][dy]`: the pair index of landblock (X₀ + dx, Y₀ + dy).
    pub blocks: [[Option<u8>; 3]; 3],
}

impl SectorWater {
    /// Builds the sector of `key` (any landblock of it): `water(k)` gives each landblock's
    /// water (§6.2), `None` for dry or absent landblocks. It is called once for each of the
    /// nine landblocks, X-major. The grid has 255 landblocks per side, a multiple of 3, so
    /// every sector (the last one is 252–254) lies wholly on the grid.
    pub fn build(
        key: LandblockKey,
        mut water: impl FnMut(LandblockKey) -> Option<LandblockWater>,
    ) -> Self {
        let origin = sector_origin(key);
        let mut out = Self {
            origin: Some(origin),
            ..Self::default()
        };
        for dx in 0..SECTOR_SIDE {
            for dy in 0..SECTOR_SIDE {
                // Never `None` (see above); skipped rather than unwrapped all the same.
                let Some(k) = origin.offset(i32::from(dx), i32::from(dy)) else {
                    continue;
                };
                let Some(w) = water(k) else { continue };
                let pair = w.sector_pair();
                let index = match out.pairs.iter().position(|p| {
                    p.water_type == pair.water_type && p.height.to_bits() == pair.height.to_bits()
                }) {
                    Some(i) => i,
                    None => {
                        out.pairs.push(pair);
                        out.pairs.len() - 1
                    }
                };
                // At most nine pairs, so the index fits a byte.
                out.blocks[usize::from(dx)][usize::from(dy)] = u8::try_from(index).ok();
            }
        }
        out
    }

    /// Whether any landblock of the sector has water.
    #[must_use]
    pub fn has_water(&self) -> bool {
        !self.pairs.is_empty()
    }
}
