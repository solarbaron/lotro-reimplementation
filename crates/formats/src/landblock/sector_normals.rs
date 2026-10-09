// SPDX-License-Identifier: GPL-3.0-or-later
//! The rule by which the stored normals of a distant-landscape sector were made
//! (`distant-landscape.md` §4.3, DL7, DL23), for tools that write or check sector records.
//! The client never recomputes them: a reader decodes the stored bytes
//! ([`super::SectorSample::normal_xyz`]).
//!
//! For one sample the rule takes the vertex normal of `landblock.md` §3.14 (terrain: *s* =
//! +1, terrain heights, bit-7 diagonals; ceiling: *s* = −1, the stored ceiling grid with its
//! zero samples, bit-0 diagonals, X offsets mirrored), with these sector-specific details:
//! corners at index −1 or 33 come from the neighbouring landblock (sample 31 or 1); a quad
//! at index −1 reads the neighbour's flag byte at 31 and a quad at index 32 the neighbour's
//! flag byte at 0; a triangle with a corner in a landblock without a record (or, for the
//! ceiling, without a ceiling grid) is left out; a quad's bit counts as set only when its
//! landblock has no record. The mean is normalised and each component *c* encoded as
//! ⌊(*c* + 1) × 127.5⌋, in the byte order X, Z (up), Y.
//!
//! Everything here works on decoded records; nothing indexes by a data-derived value.

use super::{GRID_SIDE, LandblockData, VertexFlags, height_from_raw};

/// Grid spacing in units (5 = 160 / 32).
const SPACING: f64 = 5.0;

/// The landblocks around a sample's landblock: `get(dx, dy)` for −1 ≤ *dx*, *dy* ≤ 1
/// returns the landblock-data record at that offset, or `None` when the region has none.
pub trait NormalNeighbours {
    /// The record at offset (*dx*, *dy*) from the sample's landblock.
    fn get(&self, dx: i32, dy: i32) -> Option<&LandblockData>;
}

/// A 3 × 3 window of optional records around a landblock, index [dy + 1][dx + 1].
#[derive(Debug, Clone, Copy, Default)]
pub struct NeighbourWindow<'a>(pub [[Option<&'a LandblockData>; 3]; 3]);

impl NormalNeighbours for NeighbourWindow<'_> {
    fn get(&self, dx: i32, dy: i32) -> Option<&LandblockData> {
        let row = usize::try_from(dy + 1).ok()?;
        let col = usize::try_from(dx + 1).ok()?;
        *self.0.get(row)?.get(col)?
    }
}

/// Splits a grid index −1 ..= 33 into (landblock offset, index inside that landblock) for
/// a corner (§4.3 step 2): −1 → (−1, 31), 33 → (+1, 1), otherwise (0, index).
fn corner_index(a: i32) -> Option<(i32, usize)> {
    match a {
        -1 => Some((-1, GRID_SIDE - 2)),
        33 => Some((1, 1)),
        0..=32 => usize::try_from(a).ok().map(|v| (0, v)),
        _ => None,
    }
}

/// Splits a quad index −1 ..= 32 (§4.3 step 3): −1 → (−1, 31), 32 → (+1, 0), otherwise
/// (0, index).
fn quad_index(q: i32) -> Option<(i32, usize)> {
    match q {
        -1 => Some((-1, GRID_SIDE - 2)),
        32 => Some((1, 0)),
        0..=31 => usize::try_from(q).ok().map(|v| (0, v)),
        _ => None,
    }
}

/// Which grid a normal is computed for.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NormalSurface {
    /// The terrain mesh (*s* = +1, terrain heights, bit 7).
    Terrain,
    /// The ceiling mesh (*s* = −1, ceiling heights including zero samples, bit 0).
    Ceiling,
}

impl NormalSurface {
    fn s(self) -> i32 {
        match self {
            Self::Terrain => 1,
            Self::Ceiling => -1,
        }
    }

    fn bit(self) -> u8 {
        match self {
            Self::Terrain => VertexFlags::SPLIT_SW_NE,
            Self::Ceiling => VertexFlags::CEILING_SPLIT_SW_NE,
        }
    }

    fn raw_height(self, d: &LandblockData, i: usize, j: usize) -> Option<u16> {
        match self {
            Self::Terrain => d.heights.get(i, j),
            Self::Ceiling => d.ceiling.as_ref()?.heights.get(i, j),
        }
    }
}

fn cross(a: [f64; 3], b: [f64; 3]) -> [f64; 3] {
    [
        a[1] * b[2] - a[2] * b[1],
        a[2] * b[0] - a[0] * b[2],
        a[0] * b[1] - a[1] * b[0],
    ]
}

fn sub(a: [f64; 3], b: [f64; 3]) -> [f64; 3] {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}

fn length(v: [f64; 3]) -> f64 {
    (v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt()
}

/// Encodes one component: ⌊(*c* + 1) × 127.5⌋, clamped to a byte.
fn encode(c: f64) -> u8 {
    let v = ((c + 1.0) * 127.5).floor().clamp(0.0, 255.0);
    // In range after the clamp; `as` on a finite value in 0..=255 is exact.
    v as u8
}

/// The unit normal (x, y, z; z up) of the §4.3 rule at sample (*i*, *j*) of the landblock
/// at offset (0, 0) of `nb`, before encoding. `None` when no triangle contributes, `i` or
/// `j` is outside 0–32, or the sample's own landblock has no record (or, for the ceiling,
/// no ceiling grid).
#[must_use]
pub fn rule_normal(
    nb: &impl NormalNeighbours,
    surface: NormalSurface,
    i: usize,
    j: usize,
) -> Option<[f64; 3]> {
    let i = i32::try_from(i).ok().filter(|v| (0..=32).contains(v))?;
    let j = i32::try_from(j).ok().filter(|v| (0..=32).contains(v))?;
    let own = nb.get(0, 0)?;
    surface.raw_height(own, 0, 0)?;
    let s = surface.s();
    // Corner (a, b) as a point in units, or None when it cannot be read.
    let corner = |a: i32, b: i32| -> Option<[f64; 3]> {
        let (dx, ia) = corner_index(a)?;
        let (dy, jb) = corner_index(b)?;
        let raw = surface.raw_height(nb.get(dx, dy)?, ia, jb)?;
        Some([
            SPACING * f64::from(a),
            SPACING * f64::from(b),
            f64::from(height_from_raw(raw)),
        ])
    };
    // A quad's diagonal bit; set when its landblock has no record.
    let quad_bit = |qi: i32, qj: i32| -> bool {
        let (Some((dx, ii)), Some((dy, jj))) = (quad_index(qi), quad_index(qj)) else {
            return true;
        };
        match nb.get(dx, dy) {
            None => true,
            Some(d) => d
                .vertex_flags
                .get(ii, jj)
                .is_some_and(|f| f & surface.bit() != 0),
        }
    };
    let o = (i, j);
    let sc = (i, j - 1);
    let nc = (i, j + 1);
    let w = (i - s, j);
    let e = (i + s, j);
    let sw = (i - s, j - 1);
    let se = (i + s, j - 1);
    let nw = (i - s, j + 1);
    let ne = (i + s, j + 1);
    type C = (i32, i32);
    let mut triangles: Vec<[C; 3]> = Vec::with_capacity(8);
    if quad_bit(i - 1, j - 1) {
        triangles.push([o, w, sw]);
        triangles.push([sw, sc, o]);
    } else {
        triangles.push([sc, o, w]);
    }
    if quad_bit(i, j - 1) {
        triangles.push([e, o, sc]);
    } else {
        triangles.push([se, e, o]);
        triangles.push([o, sc, se]);
    }
    if quad_bit(i - 1, j) {
        triangles.push([w, o, nc]);
    } else {
        triangles.push([nw, w, o]);
        triangles.push([o, nc, nw]);
    }
    if quad_bit(i, j) {
        triangles.push([o, e, ne]);
        triangles.push([ne, nc, o]);
    } else {
        triangles.push([nc, o, e]);
    }
    let mut sum = [0.0f64; 3];
    let mut count = 0u32;
    for [a, b, c] in triangles {
        let (Some(pa), Some(pb), Some(pc)) = (corner(a.0, a.1), corner(b.0, b.1), corner(c.0, c.1))
        else {
            continue;
        };
        let n = cross(sub(pb, pa), sub(pc, pa));
        let len = length(n);
        if len == 0.0 || !len.is_finite() {
            continue;
        }
        for (s, v) in sum.iter_mut().zip(n) {
            *s += v / len;
        }
        count += 1;
    }
    if count == 0 {
        return None;
    }
    let mean = sum.map(|v| v / f64::from(count));
    let len = length(mean);
    if len == 0.0 || !len.is_finite() {
        return None;
    }
    Some(mean.map(|v| v / len))
}

/// The encoded normal bytes of the §4.3 rule (X, Z up, Y), as a sector record stores them.
#[must_use]
pub fn rule_normal_bytes(
    nb: &impl NormalNeighbours,
    surface: NormalSurface,
    i: usize,
    j: usize,
) -> Option<[u8; 3]> {
    rule_normal(nb, surface, i, j).map(|[x, y, z]| [encode(x), encode(z), encode(y)])
}

/// Counts of a comparison of stored normals with the rule (DL-I14).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct NormalCheck {
    /// Samples compared.
    pub samples: u64,
    /// Samples equal to the rule in all three bytes.
    pub exact: u64,
    /// Samples within one step per component but not exact.
    pub within_one: u64,
    /// Samples more than one step away in some component (DL-I14 fails).
    pub beyond: u64,
    /// Samples the rule cannot compute (no contributing triangle).
    pub uncomputable: u64,
    /// Ceiling samples whose middle (up) byte is above 126 (DL-I14 fails).
    pub ceiling_up: u64,
}

impl NormalCheck {
    /// Adds `o`.
    pub fn merge(&mut self, o: Self) {
        self.samples += o.samples;
        self.exact += o.exact;
        self.within_one += o.within_one;
        self.beyond += o.beyond;
        self.uncomputable += o.uncomputable;
        self.ceiling_up += o.ceiling_up;
    }

    /// Whether DL-I14 holds for every compared sample.
    #[must_use]
    pub fn holds(&self) -> bool {
        self.beyond == 0 && self.uncomputable == 0 && self.ceiling_up == 0
    }

    /// Compares one stored sample with the rule.
    pub fn add(&mut self, stored: [u8; 3], rule: Option<[u8; 3]>, surface: NormalSurface) {
        self.samples += 1;
        if surface == NormalSurface::Ceiling && stored[1] > 126 {
            self.ceiling_up += 1;
        }
        match rule {
            None => self.uncomputable += 1,
            Some(r) if r == stored => self.exact += 1,
            Some(r) if r.iter().zip(stored).all(|(&a, b)| a.abs_diff(b) <= 1) => {
                self.within_one += 1;
            }
            Some(_) => self.beyond += 1,
        }
    }
}

/// [`NormalCheck`]s of one or more sector records, terrain and ceiling apart.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct SectorNormalCheck {
    /// Terrain samples.
    pub terrain: NormalCheck,
    /// Ceiling samples.
    pub ceiling: NormalCheck,
}

impl SectorNormalCheck {
    /// Adds `o`.
    pub fn merge(&mut self, o: Self) {
        self.terrain.merge(o.terrain);
        self.ceiling.merge(o.ceiling);
    }

    /// Whether DL-I14 holds for every compared sample.
    #[must_use]
    pub fn holds(&self) -> bool {
        self.terrain.holds() && self.ceiling.holds()
    }

    /// Samples compared, terrain and ceiling.
    #[must_use]
    pub fn samples(&self) -> u64 {
        self.terrain.samples + self.ceiling.samples
    }
}
