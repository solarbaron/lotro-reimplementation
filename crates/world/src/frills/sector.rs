// SPDX-License-Identifier: GPL-3.0-or-later
//! Frill sectors and their generation (`scenery.md` §8.2, §8.6).

use std::hash::{BuildHasher, RandomState};

use tessera_formats::Did;
use tessera_formats::setup::Quaternion;

use super::settings::{FRILL_SECTOR_SIZE, SECTORS_PER_BLOCK};
use crate::coords::{LANDBLOCK_SIZE, LandblockKey, WorldPos};
use crate::scenery::{SceneryInput, SceneryStats, frill_vertex, sum_stats};

/// Landblock edge in f32 units.
const BLOCK_F32: f32 = 160.0;

/// A frill sector: a 10 × 10-unit cell named by its region-global sector coordinates
/// (gx = 16 · X + sx, gy = 16 · Y + sy).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct FrillSectorKey {
    /// Region number.
    pub region: u8,
    /// Global sector column (east).
    pub gx: u16,
    /// Global sector row (north).
    pub gy: u16,
}

impl FrillSectorKey {
    /// Sector (sx, sy), 0–15, of a landblock; `None` outside 0–15.
    #[must_use]
    pub fn new(key: LandblockKey, sx: u8, sy: u8) -> Option<Self> {
        let n = u8::try_from(SECTORS_PER_BLOCK).ok()?;
        (sx < n && sy < n).then(|| Self {
            region: key.region(),
            gx: u16::from(key.x()) * 16 + u16::from(sx),
            gy: u16::from(key.y()) * 16 + u16::from(sy),
        })
    }

    /// The sector containing a position; `None` outside the region grid or for non-finite
    /// input.
    #[must_use]
    pub fn at(pos: &WorldPos) -> Option<Self> {
        let key = pos.landblock()?;
        let s = f64::from(FRILL_SECTOR_SIZE);
        let block = LANDBLOCK_SIZE;
        let lx = pos.x - f64::from(key.x()) * block;
        let ly = pos.y - f64::from(key.y()) * block;
        #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
        let (sx, sy) = (
            ((lx / s).floor().clamp(0.0, 15.0)) as u8,
            ((ly / s).floor().clamp(0.0, 15.0)) as u8,
        );
        Self::new(key, sx, sy)
    }

    /// The landblock the sector belongs to.
    #[must_use]
    pub fn landblock(&self) -> Option<LandblockKey> {
        let x = u8::try_from(self.gx / 16).ok()?;
        let y = u8::try_from(self.gy / 16).ok()?;
        LandblockKey::new(self.region, x, y).ok()
    }

    /// The sector's (sx, sy) within its landblock.
    #[must_use]
    pub fn local(&self) -> (u8, u8) {
        #[allow(clippy::cast_possible_truncation)]
        ((self.gx % 16) as u8, (self.gy % 16) as u8)
    }

    /// Region-global (x, y) of the sector's centre.
    #[must_use]
    pub fn centre(&self) -> [f64; 2] {
        let s = f64::from(FRILL_SECTOR_SIZE);
        [
            (f64::from(self.gx) + 0.5) * s,
            (f64::from(self.gy) + 0.5) * s,
        ]
    }
}

/// The coarse vertices whose frill pass fills sector (sx, sy) (§8.2): (sx, sy), plus
/// (16, sy) in the last column, (sx, 16) in the last row and (16, 16) in the corner.
#[must_use]
pub fn sector_vertices(sx: u8, sy: u8) -> Vec<(usize, usize)> {
    let (x, y) = (usize::from(sx), usize::from(sy));
    let mut v = vec![(x, y)];
    if x == 15 {
        v.push((16, y));
    }
    if y == 15 {
        v.push((x, 16));
    }
    if x == 15 && y == 15 {
        v.push((16, 16));
    }
    v
}

/// One frill instance: an object drawn by the frill engine at a frame. Frills never
/// collide.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct FrillInstance {
    /// The 0x47 object.
    pub object: Did,
    /// Position in the landblock frame.
    pub position: [f32; 3],
    /// Unit rotation, (w, x, y, z).
    pub rotation: Quaternion,
    /// Per-axis scale.
    pub scale: [f32; 3],
}

/// The frill instances of one sector.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct FrillSector {
    /// The sector: `None` when [`generate_sector`] was asked for an (sx, sy) outside 0–15
    /// (the sector is then empty), and in a default value.
    pub key: Option<FrillSectorKey>,
    /// Instances, in generation order.
    pub instances: Vec<FrillInstance>,
    /// Records created after a frill layer and discarded (§8.6 Tessera Decision).
    pub late_discarded: u32,
    /// Records outside the vertex's landblock (§8.2).
    pub outside: u32,
    /// Records thinned by the density (§8.2).
    pub thinned: u32,
    /// Generation counters of the sector's vertices.
    pub stats: SceneryStats,
}

/// The density test's random source (§8.2): global and unseeded in the client, so frills
/// below density 1 differ each session. Tessera seeds each thinner randomly; a fixed seed
/// is available for tests.
#[derive(Debug, Clone)]
pub struct DensityThinner {
    state: u64,
}

impl Default for DensityThinner {
    fn default() -> Self {
        Self::new()
    }
}

impl DensityThinner {
    /// A thinner with a random seed.
    #[must_use]
    pub fn new() -> Self {
        Self::with_seed(RandomState::new().hash_one(0x5EED_u64))
    }

    /// A thinner with a fixed seed (tests).
    #[must_use]
    pub fn with_seed(seed: u64) -> Self {
        Self {
            state: seed ^ 0x9E37_79B9_7F4A_7C15,
        }
    }

    /// A uniform number in [0, 1).
    pub fn next_unit(&mut self) -> f32 {
        // splitmix64
        self.state = self.state.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.state;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^= z >> 31;
        #[allow(clippy::cast_precision_loss)]
        let u = (z >> 40) as f32 / (1u64 << 24) as f32;
        u
    }

    /// Whether a record is kept at `density` (u ≤ density; every record at density ≥ 1).
    pub fn keep(&mut self, density: f32) -> bool {
        if density >= 1.0 {
            return true;
        }
        self.next_unit() <= density
    }
}

/// Generates sector (sx, sy) of `input`'s landblock (§8.2): the frill pass of the sector's
/// vertices; records created after a frill layer are discarded (§8.6); a record is kept
/// only if it lies in the landblock (0 ≤ x, y < 160) and passes the density test. An
/// (sx, sy) outside 0–15 gives an empty sector whose [`FrillSector::key`] is `None`.
#[must_use]
pub fn generate_sector(
    input: &SceneryInput<'_>,
    sx: u8,
    sy: u8,
    density: f32,
    thinner: &mut DensityThinner,
) -> FrillSector {
    let mut out = FrillSector {
        key: FrillSectorKey::new(input.key, sx, sy),
        ..FrillSector::default()
    };
    if out.key.is_none() {
        return out;
    }
    let mut stats = Vec::new();
    for (i, j) in sector_vertices(sx, sy) {
        let (records, s) = frill_vertex(input, i, j);
        stats.push(s);
        for r in records {
            if r.late {
                out.late_discarded += 1;
                continue;
            }
            let p = r.object.position;
            if !((0.0..BLOCK_F32).contains(&p[0]) && (0.0..BLOCK_F32).contains(&p[1])) {
                out.outside += 1;
                continue;
            }
            if !thinner.keep(density) {
                out.thinned += 1;
                continue;
            }
            out.instances.push(FrillInstance {
                object: r.object.object,
                position: p,
                rotation: r.object.rotation,
                scale: r.object.scale,
            });
        }
    }
    out.stats = sum_stats(&stats);
    out
}
