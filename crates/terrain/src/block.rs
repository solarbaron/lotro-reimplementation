// SPDX-License-Identifier: GPL-3.0-or-later
//! One landblock's terrain surface: the 33 × 33 height grid, the split diagonal of each
//! quad, per-LOD error bounds and CPU height queries.
//!
//! Spec: `landblock.md` §3.3 (grid, axes), §3.4 (scale), §3.6 (split diagonal), §3.11
//! (height at a point).

use std::sync::OnceLock;

use tessera_formats::landblock::{
    GRID_SAMPLES, GRID_SIDE, GRID_SPACING, HeightGrid, LANDBLOCK_SIZE, LandblockData, QUAD_COUNT,
    QUAD_SIDE, VertexFlags, height_from_raw,
};

use crate::lod::{Lod, Side};

/// Height in world units of the largest stored sample (`landblock.md` §3.4).
const HEIGHT_RANGE: f32 = 2000.0;
/// The f32 nearest 1 / 65,535, the second factor of the height rule (`landblock.md` §3.4).
const INV_MAX_RAW: f32 = 1.0 / 65_535.0;
/// World units per stored step, 2,000 / 65,535 (`landblock.md` §3.4), in f64: the factor
/// for slopes, normals and error bounds (`terrain.md` §3, §5), where no stored value is
/// converted and the f32 rounding steps of the height rule do not apply.
pub(crate) const UNITS_PER_STEP: f64 = 2000.0 / 65_535.0;

/// Converts a height in stored steps (f64, possibly interpolated) to world units with the
/// steps of the height rule (`landblock.md` §3.4, `terrain.md` §1): the steps times 2,000.0
/// rounded to f32, times the f32 nearest 1 / 65,535 rounded to f32. For an integer `steps`
/// in 0–65,535 the first product is exact in f64, so its f32 rounding equals the f32
/// product of the rule and the result is bit-identical to [`height_from_raw`]: queries,
/// bounds and mesh vertices agree exactly at the samples. Both steps are monotone, so a
/// value clamped to the samples' range stays within their heights.
fn units(steps: f64) -> f32 {
    let scaled = (steps * f64::from(HEIGHT_RANGE)) as f32;
    scaled * INV_MAX_RAW
}

/// Vertex number of grid sample (i, j), i along X: **33 · j + i** (Y-major). This is the
/// order of every vertex stream of this crate and the order of the vertex numbers in
/// surface-layer index lists (`landblock.md` §3.8.3), so layer triangles index the
/// full-grid vertex set directly.
///
/// Precondition: `i` and `j` are in 0–32. The function does not check them (outside that
/// range two samples alias, e.g. (33, 0) and (0, 1), and a huge `j` overflows); debug
/// builds assert the range. Range-check values derived from data first, or use the
/// checked accessors ([`TerrainBlock::raw`], [`crate::VertexNormals::get`]).
#[must_use]
pub fn vertex_index(i: usize, j: usize) -> usize {
    debug_assert!(
        i < GRID_SIDE && j < GRID_SIDE,
        "vertex_index({i}, {j}) outside 0–32"
    );
    GRID_SIDE * j + i
}

/// Words of the split-diagonal bit set (one bit per quad).
const SPLIT_WORDS: usize = QUAD_COUNT.div_ceil(64);

/// Height and surface normal of the terrain at one point.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct GroundSample {
    /// Height in world units.
    pub height: f32,
    /// Unit normal of the triangle under the point (z > 0).
    pub normal: [f32; 3],
}

/// The terrain of one landblock, ready for meshing and height queries.
///
/// Built from the decoded height grid and vertex flags of a landblock-data record. Heights
/// are kept as the stored u16 values (converted with `stored × 2,000 / 65,535` on use, see
/// [`height_from_raw`]), so the shared edge samples of two neighbours (HM7) produce
/// bit-identical vertices.
///
/// About 2.3 KiB per block: 2,178 bytes of heights on the heap plus a 128-byte bit set of
/// split diagonals. The per-level error bounds ([`Self::lod_error`]) are only needed for
/// rendering, so they are computed on first use (about 30 µs); height-only users (ground
/// clamping, a server) never pay for them. A streamer that selects levels on its render
/// thread calls [`Self::lod_errors`] once on the worker that builds the block.
#[derive(Clone)]
pub struct TerrainBlock {
    /// Stored heights in vertex order (33·j + i).
    raw: Box<[u16; GRID_SAMPLES]>,
    /// Split diagonal of quad (i, j) at bit 32·j + i: set = south-west to north-east.
    split: [u64; SPLIT_WORDS],
    min_raw: u16,
    max_raw: u16,
    /// Largest vertical deviation of each LOD's regular surface from the level-0 surface,
    /// computed on first use.
    lod_error: OnceLock<[f32; Lod::COUNT]>,
}

impl PartialEq for TerrainBlock {
    /// Blocks are equal when their heights and split diagonals are (everything else is
    /// derived from those, and the lazily computed error bounds must not matter).
    fn eq(&self, other: &Self) -> bool {
        self.raw == other.raw && self.split == other.split
    }
}

impl std::fmt::Debug for TerrainBlock {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("TerrainBlock")
            .field("min_height", &self.min_height())
            .field("max_height", &self.max_height())
            .field("lod_error", &self.lod_error.get())
            .finish_non_exhaustive()
    }
}

impl Default for TerrainBlock {
    fn default() -> Self {
        Self::flat(0)
    }
}

impl TerrainBlock {
    /// A block from a decoded height grid (X-major, `landblock.md` §3.3) and its vertex
    /// flags (bit 7 = split diagonal, §3.6).
    #[must_use]
    pub fn new(heights: &HeightGrid, flags: &VertexFlags) -> Self {
        let src = heights.raw();
        let mut raw = Box::new([0u16; GRID_SAMPLES]);
        for i in 0..GRID_SIDE {
            for j in 0..GRID_SIDE {
                raw[vertex_index(i, j)] = src[GRID_SIDE * i + j];
            }
        }
        let mut split = [0u64; SPLIT_WORDS];
        for i in 0..QUAD_SIDE {
            for j in 0..QUAD_SIDE {
                if flags.split_sw_ne(i, j) {
                    let k = QUAD_SIDE * j + i;
                    split[k / 64] |= 1 << (k % 64);
                }
            }
        }
        Self::finish(raw, split)
    }

    /// The terrain of a decoded landblock-data record (its ceiling is not terrain).
    #[must_use]
    pub fn from_landblock(data: &LandblockData) -> Self {
        Self::new(&data.heights, &data.vertex_flags)
    }

    /// The **ceiling** surface of a cave landblock as a block (`landblock.md` §3.5,
    /// `terrain-texturing.md` §4.8): the ceiling height grid, with each quad split by
    /// **bit 0** of its vertex-flag byte (the ceiling diagonal, §3.6), so
    /// [`Self::height_at`] follows the ceiling triangles. Zero samples ("no ceiling here")
    /// stay at height 0, as in the original's ceiling mesh (`collision.md` §6.4).
    ///
    /// Queries return upward facet normals ([`Self::normal_at`]); the ceiling itself faces
    /// down (its triangles are clockwise from above). The level-of-detail meshing of this
    /// crate winds triangles counter-clockwise and is meant for the terrain; ceiling layers
    /// are drawn from their stored triangles. `None` without a ceiling.
    #[must_use]
    pub fn from_ceiling(data: &LandblockData) -> Option<Self> {
        let ceiling = data.ceiling.as_ref()?;
        let mut flags = VertexFlags::default();
        for (dst, &src) in flags.raw_mut().iter_mut().zip(data.vertex_flags.raw()) {
            if src & VertexFlags::CEILING_SPLIT_SW_NE != 0 {
                *dst = VertexFlags::SPLIT_SW_NE;
            }
        }
        Some(Self::new(&ceiling.heights, &flags))
    }

    /// A flat block at stored height `raw`, every quad split south-east to north-west.
    #[must_use]
    pub fn flat(raw: u16) -> Self {
        Self::finish(Box::new([raw; GRID_SAMPLES]), [0; SPLIT_WORDS])
    }

    fn finish(raw: Box<[u16; GRID_SAMPLES]>, split: [u64; SPLIT_WORDS]) -> Self {
        let (min_raw, max_raw) = raw
            .iter()
            .fold((u16::MAX, 0), |(lo, hi), &h| (lo.min(h), hi.max(h)));
        Self {
            raw,
            split,
            min_raw,
            max_raw,
            lod_error: OnceLock::new(),
        }
    }

    /// Stored height of sample (i, j), i along X; `None` outside 0–32.
    #[must_use]
    pub fn raw(&self, i: usize, j: usize) -> Option<u16> {
        (i < GRID_SIDE && j < GRID_SIDE).then(|| self.raw[vertex_index(i, j)])
    }

    /// Height of sample (i, j) in world units; `None` outside 0–32.
    #[must_use]
    pub fn height(&self, i: usize, j: usize) -> Option<f32> {
        self.raw(i, j).map(height_from_raw)
    }

    /// All stored heights in vertex order (33·j + i, see [`vertex_index`]).
    #[must_use]
    pub fn raw_heights(&self) -> &[u16; GRID_SAMPLES] {
        &self.raw
    }

    /// The stored samples along `side`, in counter-clockwise order around the block
    /// (south: west to east; east: south to north; north: east to west; west: north to
    /// south). Two neighbours share an edge when one's side equals the other's opposite
    /// side reversed (HM7).
    #[must_use]
    pub fn edge(&self, side: Side) -> [u16; GRID_SIDE] {
        std::array::from_fn(|k| {
            let (i, j) = match side {
                Side::South => (k, 0),
                Side::East => (QUAD_SIDE, k),
                Side::North => (QUAD_SIDE - k, QUAD_SIDE),
                Side::West => (0, QUAD_SIDE - k),
            };
            self.raw[vertex_index(i, j)]
        })
    }

    /// Whether this block's `side` edge carries the same samples as `neighbour`'s opposite
    /// edge (`landblock.md` §3.3, HM7). Terrain meshes of the two are crack-free only then.
    #[must_use]
    pub fn edge_matches(&self, side: Side, neighbour: &Self) -> bool {
        let mut theirs = neighbour.edge(side.opposite());
        theirs.reverse();
        self.edge(side) == theirs
    }

    /// Whether quad (i, j) of the full grid (0 ≤ i, j ≤ 31) is split from south-west to
    /// north-east (vertex-flag bit 7 set). Out-of-range quads report `false`.
    #[must_use]
    pub fn split_sw_ne(&self, i: usize, j: usize) -> bool {
        if i >= QUAD_SIDE || j >= QUAD_SIDE {
            return false;
        }
        let k = QUAD_SIDE * j + i;
        self.split[k / 64] >> (k % 64) & 1 != 0
    }

    /// Split diagonal of the level-`lod` quad whose south-west sample is (i, j) (both
    /// multiples of the level's step). Level 0 uses the stored diagonal. Coarser levels use
    /// the diagonal whose midpoint is closer to the sample at the quad's centre (ties:
    /// south-west to north-east), computed on stored integers so it is deterministic.
    pub(crate) fn lod_split_sw_ne(&self, lod: Lod, i: usize, j: usize) -> bool {
        let s = lod.step();
        if s == 1 {
            return self.split_sw_ne(i, j);
        }
        let h = |a: usize, b: usize| i32::from(self.raw[vertex_index(a, b)]);
        let centre = 2 * h(i + s / 2, j + s / 2);
        let sw_ne = (centre - h(i, j) - h(i + s, j + s)).abs();
        let se_nw = (centre - h(i + s, j) - h(i, j + s)).abs();
        sw_ne <= se_nw
    }

    /// Smallest terrain height in world units.
    #[must_use]
    pub fn min_height(&self) -> f32 {
        height_from_raw(self.min_raw)
    }

    /// Largest terrain height in world units.
    #[must_use]
    pub fn max_height(&self) -> f32 {
        height_from_raw(self.max_raw)
    }

    /// Whether all 1,089 samples are equal (void, ocean or flat dungeon blocks).
    #[must_use]
    pub fn is_flat(&self) -> bool {
        self.min_raw == self.max_raw
    }

    /// Geometric error of level `lod` in world units: the largest vertical distance between
    /// the level's regular surface ([`Self::lod_height_at`]) and the level-0 surface
    /// ([`Self::height_at`]) anywhere on the block, made non-decreasing over the levels
    /// (and rounded up to the next f32). 0 for level 0. Stitch strips along a coarser
    /// neighbour's edge are not part of the regular surface and are not covered.
    ///
    /// Computed with [`Self::lod_errors`] on first use.
    #[must_use]
    pub fn lod_error(&self, lod: Lod) -> f32 {
        self.lod_errors()[usize::from(lod.level())]
    }

    /// [`Self::lod_error`] of every level, finest first. The first call computes them
    /// (about 30 µs in a release build); later calls are free.
    #[must_use]
    pub fn lod_errors(&self) -> &[f32; Lod::COUNT] {
        self.lod_error.get_or_init(|| {
            let mut out = [0.0f32; Lod::COUNT];
            let mut worst = 0.0f64;
            for lod in Lod::all().skip(1) {
                // Monotone by construction: a coarser level never reports less error.
                worst = worst.max(self.measure_lod_error(lod));
                let bound = worst * UNITS_PER_STEP;
                let mut e = bound as f32;
                if f64::from(e) < bound {
                    e = e.next_up();
                }
                out[usize::from(lod.level())] = e;
            }
            out
        })
    }

    /// Exact largest |level-`lod` surface − level-0 surface| in stored steps.
    ///
    /// Both surfaces are piecewise linear, so their difference is linear on every cell of
    /// their common refinement and peaks at one of its vertices. Those are the grid samples
    /// (coarse edges along grid lines meet fine edges only there) and, where a coarse
    /// quad's diagonal crosses a fine quad split the other way, that fine quad's centre.
    /// Both kinds are evaluated; the centres are evaluated for every fine quad on the
    /// coarse diagonal (where the fine split agrees, the centre lies on a shared edge and
    /// adds nothing).
    fn measure_lod_error(&self, lod: Lod) -> f64 {
        let s = lod.step();
        // Exact: s is a power of two.
        let inv = 1.0 / s as f64;
        let mut worst = 0.0f64;
        for qj in (0..QUAD_SIDE).step_by(s) {
            for qi in (0..QUAD_SIDE).step_by(s) {
                let split = self.lod_split_sw_ne(lod, qi, qj);
                let corners = self.quad_corners(qi, qj, s);
                for b in 0..=s {
                    for a in 0..=s {
                        let z = plane_height(corners, split, a as f64 * inv, b as f64 * inv);
                        let h = f64::from(self.raw[vertex_index(qi + a, qj + b)]);
                        worst = worst.max((z - h).abs());
                    }
                }
                for k in 0..s {
                    // Fine quad (qi + a, qj + b) on the coarse diagonal.
                    let (a, b) = if split { (k, k) } else { (k, s - 1 - k) };
                    let (fi, fj) = (qi + a, qj + b);
                    let fine = plane_height(
                        self.quad_corners(fi, fj, 1),
                        self.split_sw_ne(fi, fj),
                        0.5,
                        0.5,
                    );
                    let z = plane_height(
                        corners,
                        split,
                        (a as f64 + 0.5) * inv,
                        (b as f64 + 0.5) * inv,
                    );
                    worst = worst.max((z - fine).abs());
                }
            }
        }
        worst
    }

    /// Stored heights (as f64) of the corners of the quad of side `s` at (i, j):
    /// [south-west, south-east, north-west, north-east].
    fn quad_corners(&self, i: usize, j: usize, s: usize) -> [f64; 4] {
        let h = |a: usize, b: usize| f64::from(self.raw[vertex_index(a, b)]);
        [h(i, j), h(i + s, j), h(i, j + s), h(i + s, j + s)]
    }

    /// Locates block-local (x, y) on level `lod`'s quad grid: quad origin (i, j) in samples
    /// and the fractions inside it. `None` for NaN; coordinates are clamped to [0, 160].
    fn locate(lod: Lod, x: f32, y: f32) -> Option<(usize, usize, f64, f64)> {
        if x.is_nan() || y.is_nan() {
            return None;
        }
        let size = f64::from(GRID_SPACING) * lod.step() as f64;
        let last = (lod.quads_per_side() - 1) as f64;
        let gx = f64::from(x.clamp(0.0, LANDBLOCK_SIZE)) / size;
        let gy = f64::from(y.clamp(0.0, LANDBLOCK_SIZE)) / size;
        let qi = gx.floor().min(last);
        let qj = gy.floor().min(last);
        let s = lod.step();
        Some((qi as usize * s, qj as usize * s, gx - qi, gy - qj))
    }

    /// Terrain height in world units at block-local (x, y) (`landblock.md` §3.11): the
    /// plane of the full-grid triangle containing the point, with the quad split by the
    /// stored diagonal. This is exactly the surface of the level-0 mesh (unstitched edges):
    /// at a grid sample it returns the mesh vertex's z bit for bit. Coordinates are clamped
    /// to [0, 160]; `None` for NaN.
    #[must_use]
    pub fn height_at(&self, x: f32, y: f32) -> Option<f32> {
        self.lod_height_at(Lod::FINEST, x, y)
    }

    /// Height of level `lod`'s regular surface (its interior triangulation, without edge
    /// stitching) at block-local (x, y). Level 0 equals [`Self::height_at`].
    #[must_use]
    pub fn lod_height_at(&self, lod: Lod, x: f32, y: f32) -> Option<f32> {
        let (i, j, fx, fy) = Self::locate(lod, x, y)?;
        let split = self.lod_split_sw_ne(lod, i, j);
        let z = plane_height(self.quad_corners(i, j, lod.step()), split, fx, fy);
        Some(self.clamped_units(z))
    }

    /// Bilinear interpolation of the four samples around block-local (x, y). Smoother than
    /// [`Self::height_at`] but not the rendered surface (`landblock.md` §3.11 notes the
    /// triangulated surface fits placed objects better). `None` for NaN.
    #[must_use]
    pub fn height_bilinear(&self, x: f32, y: f32) -> Option<f32> {
        let (i, j, fx, fy) = Self::locate(Lod::FINEST, x, y)?;
        let [h00, h10, h01, h11] = self.quad_corners(i, j, 1);
        let z = h00 * (1.0 - fx) * (1.0 - fy)
            + h10 * fx * (1.0 - fy)
            + h01 * (1.0 - fx) * fy
            + h11 * fx * fy;
        Some(self.clamped_units(z))
    }

    /// An interpolated height in stored steps, converted to world units. The surface lies
    /// within the samples' range, so f64 rounding is clamped away: results never leave
    /// [`Self::min_height`]–[`Self::max_height`].
    fn clamped_units(&self, steps: f64) -> f32 {
        units(steps.clamp(f64::from(self.min_raw), f64::from(self.max_raw)))
    }

    /// Unit normal of the full-grid triangle under block-local (x, y) (the facet normal,
    /// not the smoothed vertex normal used for shading). `None` for NaN.
    #[must_use]
    pub fn normal_at(&self, x: f32, y: f32) -> Option<[f32; 3]> {
        let (i, j, fx, fy) = Self::locate(Lod::FINEST, x, y)?;
        let [h00, h10, h01, h11] = self.quad_corners(i, j, 1).map(|h| h * UNITS_PER_STEP);
        let d = f64::from(GRID_SPACING);
        // Plane gradient (dz/dx, dz/dy) of the triangle containing the point.
        let (gx, gy) = if self.split_sw_ne(i, j) {
            if fx >= fy {
                (h10 - h00, h11 - h10)
            } else {
                (h11 - h01, h01 - h00)
            }
        } else if fx + fy <= 1.0 {
            (h10 - h00, h01 - h00)
        } else {
            (h11 - h01, h11 - h10)
        };
        Some(unit_normal(gx / d, gy / d))
    }

    /// Height and facet normal at block-local (x, y).
    #[must_use]
    pub fn sample(&self, x: f32, y: f32) -> Option<GroundSample> {
        Some(GroundSample {
            height: self.height_at(x, y)?,
            normal: self.normal_at(x, y)?,
        })
    }

    /// Euclidean distance from block-local point `p` (the camera, in this block's frame)
    /// to the block's bounding box (0–160 × 0–160 × min–max height); 0 inside. NaN input
    /// gives NaN, infinite or overflowing input +∞ (see [`crate::LodSelector::select`]).
    #[must_use]
    pub fn distance_to(&self, p: [f32; 3]) -> f32 {
        let gap = |v: f32, lo: f32, hi: f32| {
            if v < lo {
                lo - v
            } else if v > hi {
                v - hi
            } else if v.is_nan() {
                f32::NAN
            } else {
                0.0
            }
        };
        let dx = gap(p[0], 0.0, LANDBLOCK_SIZE);
        let dy = gap(p[1], 0.0, LANDBLOCK_SIZE);
        let dz = gap(p[2], self.min_height(), self.max_height());
        (dx * dx + dy * dy + dz * dz).sqrt()
    }
}

/// Height (in the corners' unit) of the triangle plane containing (fx, fy) ∈ [0, 1]² of a
/// quad with corners [sw, se, nw, ne], split per `sw_ne` (`landblock.md` §3.11 steps 4–5).
pub(crate) fn plane_height(c: [f64; 4], sw_ne: bool, fx: f64, fy: f64) -> f64 {
    let [h00, h10, h01, h11] = c;
    if sw_ne {
        if fx >= fy {
            // (i, j), (i + 1, j), (i + 1, j + 1)
            h00 + fx * (h10 - h00) + fy * (h11 - h10)
        } else {
            // (i, j), (i + 1, j + 1), (i, j + 1)
            h00 + fx * (h11 - h01) + fy * (h01 - h00)
        }
    } else if fx + fy <= 1.0 {
        // (i, j), (i + 1, j), (i, j + 1)
        h00 + fx * (h10 - h00) + fy * (h01 - h00)
    } else {
        // (i + 1, j), (i + 1, j + 1), (i, j + 1)
        h11 + (1.0 - fx) * (h01 - h11) + (1.0 - fy) * (h10 - h11)
    }
}

/// Unit normal of a surface with gradient (dz/dx, dz/dy).
pub(crate) fn unit_normal(gx: f64, gy: f64) -> [f32; 3] {
    let len = (gx * gx + gy * gy + 1.0).sqrt();
    if !len.is_finite() || len == 0.0 {
        return [0.0, 0.0, 1.0];
    }
    [(-gx / len) as f32, (-gy / len) as f32, (1.0 / len) as f32]
}

#[cfg(test)]
mod tests {
    use super::*;
    use tessera_formats::Did;

    /// A rough synthetic grid (deterministic, integer only).
    pub(crate) fn rough(seed: u32) -> HeightGrid {
        HeightGrid::from_fn(|i, j| {
            let mut h = (i as u32)
                .wrapping_mul(0x9E37_79B9)
                .wrapping_add((j as u32).wrapping_mul(0x85EB_CA6B))
                .wrapping_add(seed.wrapping_mul(0xC2B2_AE35));
            h ^= h >> 15;
            h = h.wrapping_mul(0x2C1B_3C6D);
            h ^= h >> 12;
            (20_000 + (h % 4_000)) as u16
        })
    }

    fn flags(seed: u32) -> VertexFlags {
        let mut f = VertexFlags::default();
        for (k, b) in f.raw_mut().iter_mut().enumerate() {
            if (k as u32 ^ seed).wrapping_mul(2_654_435_761) >> 31 == 1 {
                *b = 0x80;
            }
        }
        f
    }

    #[test]
    fn axis_order_and_edges() {
        let g = HeightGrid::from_fn(|i, j| (i * 100 + j) as u16);
        let b = TerrainBlock::new(&g, &VertexFlags::default());
        assert_eq!(b.raw(3, 7), Some(307));
        assert_eq!(b.raw_heights()[vertex_index(3, 7)], 307);
        assert_eq!(b.raw(33, 0), None);
        assert_eq!(b.edge(Side::South)[5], 500);
        assert_eq!(b.edge(Side::East)[5], 3205);
        assert_eq!(b.edge(Side::North)[0], 3232);
        assert_eq!(b.edge(Side::West)[0], 32);
        assert_eq!(b.min_height(), 0.0);
        assert!(!b.is_flat());
        assert!(TerrainBlock::flat(7).is_flat());
    }

    #[test]
    fn edge_matching_follows_hm7() {
        // Two blocks cut from one global field share their edge.
        let f = |gi: usize, gj: usize| ((gi * 31 + gj * 17) % 1000) as u16;
        let a = TerrainBlock::new(&HeightGrid::from_fn(f), &VertexFlags::default());
        let east = TerrainBlock::new(
            &HeightGrid::from_fn(|i, j| f(i + 32, j)),
            &VertexFlags::default(),
        );
        let north = TerrainBlock::new(
            &HeightGrid::from_fn(|i, j| f(i, j + 32)),
            &VertexFlags::default(),
        );
        assert!(a.edge_matches(Side::East, &east));
        assert!(east.edge_matches(Side::West, &a));
        assert!(a.edge_matches(Side::North, &north));
        assert!(!a.edge_matches(Side::South, &north));
    }

    #[test]
    fn height_at_matches_the_decoder_rule() {
        for seed in 0..4 {
            let g = rough(seed);
            let fl = flags(seed);
            let b = TerrainBlock::new(&g, &fl);
            let mut data = LandblockData::from_heights(Did(0x8001_0000), g.clone());
            data.vertex_flags = fl.clone();
            let mut k = 0u32;
            for xi in 0..=64 {
                for yi in 0..=64 {
                    k = k.wrapping_mul(1_103_515_245).wrapping_add(12_345);
                    let jitter = (k >> 16) as f32 / 65_536.0;
                    let x = (xi as f32 * 2.5 + jitter).min(160.0);
                    let y = (yi as f32 * 2.5 + 1.0 - jitter).min(160.0);
                    let ours = b.height_at(x, y).unwrap();
                    let theirs = data.terrain_height(x, y).unwrap();
                    assert!((ours - theirs).abs() < 2e-3, "{x},{y}: {ours} vs {theirs}");
                }
            }
        }
    }

    #[test]
    fn height_at_hits_samples_and_clamps() {
        let g = rough(9);
        let b = TerrainBlock::new(&g, &flags(9));
        for i in 0..GRID_SIDE {
            for j in 0..GRID_SIDE {
                let z = b.height_at(i as f32 * 5.0, j as f32 * 5.0).unwrap();
                assert!((z - b.height(i, j).unwrap()).abs() < 1e-3);
            }
        }
        assert_eq!(b.height_at(f32::NAN, 0.0), None);
        assert_eq!(b.height_at(-50.0, 1e9), b.height_at(0.0, 160.0));
        assert_eq!(
            b.height_at(f32::INFINITY, f32::NEG_INFINITY),
            b.height_at(160.0, 0.0)
        );
        assert!(b.height_bilinear(f32::NAN, 1.0).is_none());
        assert!(b.normal_at(1.0, f32::NAN).is_none());
    }

    #[test]
    fn bilinear_and_triangles_agree_on_planes() {
        // A tilted plane: every interpolation is exact.
        let g = HeightGrid::from_fn(|i, j| (1000 + 30 * i + 70 * j) as u16);
        let b = TerrainBlock::new(&g, &flags(3));
        for (x, y) in [(1.0, 2.0), (77.7, 13.3), (159.9, 0.1), (42.0, 150.0)] {
            let t = b.height_at(x, y).unwrap();
            let l = b.height_bilinear(x, y).unwrap();
            assert!((t - l).abs() < 1e-3);
            let expect = (1000.0 + 6.0 * f64::from(x) + 14.0 * f64::from(y)) * UNITS_PER_STEP;
            assert!((f64::from(t) - expect).abs() < 1e-3);
            let n = b.normal_at(x, y).unwrap();
            let gx = 6.0 * UNITS_PER_STEP;
            let gy = 14.0 * UNITS_PER_STEP;
            let len = (gx * gx + gy * gy + 1.0f64).sqrt();
            assert!((f64::from(n[0]) + gx / len).abs() < 1e-6);
            assert!((f64::from(n[1]) + gy / len).abs() < 1e-6);
            for lod in Lod::all() {
                let z = b.lod_height_at(lod, x, y).unwrap();
                assert!((z - t).abs() < 1e-3, "{lod}");
            }
        }
        for lod in Lod::all() {
            assert!(b.lod_error(lod) < 1e-3, "{lod}: {}", b.lod_error(lod));
        }
    }

    #[test]
    fn lod_error_is_monotone_and_bounds_the_surface() {
        let b = TerrainBlock::new(&rough(5), &flags(5));
        assert_eq!(b.lod_error(Lod::FINEST), 0.0);
        let mut last = 0.0;
        for lod in Lod::all() {
            let e = b.lod_error(lod);
            assert!(e >= last);
            last = e;
            for i in 0..GRID_SIDE {
                for j in 0..GRID_SIDE {
                    let z = b
                        .lod_height_at(lod, i as f32 * 5.0, j as f32 * 5.0)
                        .unwrap();
                    assert!((z - b.height(i, j).unwrap()).abs() <= e + 1e-3);
                }
            }
        }
        assert!(b.lod_error(Lod::COARSEST) > 1.0);
    }

    #[test]
    fn distance_to_box() {
        let b = TerrainBlock::flat(3277); // ≈ 100 units
        let z = b.min_height();
        assert_eq!(b.distance_to([80.0, 80.0, z]), 0.0);
        assert!((b.distance_to([80.0, 80.0, z + 10.0]) - 10.0).abs() < 0.01);
        assert!((b.distance_to([-30.0, 200.0, z]) - 50.0).abs() < 0.01);
        assert!(b.distance_to([f32::NAN, 0.0, 0.0]).is_nan());
    }

    /// Largest |lod_height_at − height_at| per level on a raster of `step` units.
    fn raster_deviation(b: &TerrainBlock, step: f32) -> [f32; Lod::COUNT] {
        let n = (LANDBLOCK_SIZE / step).round() as usize;
        let mut out = [0.0f32; Lod::COUNT];
        for lod in Lod::all() {
            for yi in 0..=n {
                for xi in 0..=n {
                    let (x, y) = (xi as f32 * step, yi as f32 * step);
                    let d =
                        (b.lod_height_at(lod, x, y).unwrap() - b.height_at(x, y).unwrap()).abs();
                    out[usize::from(lod.level())] = out[usize::from(lod.level())].max(d);
                }
            }
        }
        out
    }

    /// Checkerboard 2 × 2 folds: every level-1 quad's samples lie on its own plane pair,
    /// but every stored diagonal runs against the level-1 diagonal.
    fn folds() -> TerrainBlock {
        const H: usize = 2_000;
        let corner = |i: usize, j: usize| H * ((i / 2 + j / 2) % 2);
        let g = HeightGrid::from_fn(|i, j| {
            let h = match (i % 2, j % 2) {
                (0, 0) => corner(i, j),
                (1, 1) => corner(i - 1, j - 1),
                _ => H / 2,
            };
            (20_000 + h) as u16
        });
        TerrainBlock::new(&g, &VertexFlags::default())
    }

    #[test]
    fn lod_error_is_exact_against_the_level_zero_surface() {
        // Regression: the error was measured only at grid samples, so the fold block
        // reported 0 at level 1 although the surfaces differ by ~30 units at the centres
        // of the fine quads.
        let f = folds();
        let l1 = Lod::new(1).unwrap();
        assert!(f.lod_error(l1) > 25.0, "{}", f.lod_error(l1));
        // Far enough that level 1 was accepted with the old bound, it is now rejected.
        let sel = crate::LodSelector::default();
        assert_eq!(sel.select(&f, 1000.0, None), Lod::FINEST);

        let mut blocks = vec![f];
        for seed in 0..4 {
            blocks.push(TerrainBlock::new(&rough(seed), &flags(seed)));
            // Ridges along the level-1 diagonals over stored diagonals that cross them.
            blocks.push(TerrainBlock::new(
                &HeightGrid::from_fn(|i, j| {
                    let ridge = if (i + j) % 2 == 0 { 3_000 } else { 0 };
                    (10_000 + ridge + 37 * ((i * 13 + j * 7 + seed as usize) % 11)) as u16
                }),
                &flags(seed),
            ));
        }
        for b in &blocks {
            // The 2.5-unit raster contains every sample and every fine-quad centre, i.e.
            // every vertex of the two surfaces' common refinement: its maximum is exact.
            let exact = raster_deviation(b, 2.5);
            let dense = raster_deviation(b, 0.5);
            let mut running = 0.0f32;
            for lod in Lod::all() {
                let l = usize::from(lod.level());
                let e = b.lod_error(lod);
                running = running.max(exact[l]);
                assert!(
                    dense[l] <= e + 1e-3,
                    "{lod}: dense {} > bound {e}",
                    dense[l]
                );
                assert!((running - e).abs() <= 1e-3, "{lod}: {running} vs {e}");
            }
        }
    }

    #[test]
    fn queries_and_mesh_heights_use_one_conversion() {
        // Regression: queries once used a different conversion from sample heights, bounds
        // and mesh vertices (1 ulp apart for most values). `terrain.md` §1: all of them equal
        // the decoder's `height_from_raw` (`landblock.md` §3.4) bit for bit.
        for raw in 0..=u16::MAX {
            let b = TerrainBlock::flat(raw);
            let z = b.height_at(80.0, 80.0).unwrap();
            assert_eq!(z.to_bits(), height_from_raw(raw).to_bits(), "{raw}");
            assert_eq!(z, b.height(16, 16).unwrap());
            assert_eq!(z, b.min_height());
            assert_eq!(z, b.max_height());
            assert_eq!(b.height_bilinear(33.3, 71.1), Some(z));
            assert_eq!(b.lod_height_at(Lod::COARSEST, 1.0, 2.0), Some(z));
        }
        let b = TerrainBlock::new(&rough(11), &flags(11));
        for j in 0..GRID_SIDE {
            for i in 0..GRID_SIDE {
                let (x, y) = (i as f32 * 5.0, j as f32 * 5.0);
                assert_eq!(b.height_at(x, y), b.height(i, j));
                assert_eq!(b.height_bilinear(x, y), b.height(i, j));
            }
        }
        for k in 0..2_000u32 {
            let x = (k * 37 % 1_601) as f32 / 10.0;
            let y = (k * 91 % 1_601) as f32 / 10.0;
            for z in [b.height_at(x, y), b.height_bilinear(x, y)] {
                let z = z.unwrap();
                assert!(z >= b.min_height() && z <= b.max_height());
            }
        }
    }

    #[test]
    fn lod_errors_are_lazy_and_do_not_affect_equality() {
        let a = TerrainBlock::new(&rough(2), &flags(2));
        let b = a.clone();
        let _ = a.lod_errors();
        assert_eq!(a, b);
        assert_eq!(a.lod_errors(), b.lod_errors());
        assert_ne!(a, TerrainBlock::new(&rough(2), &flags(3)));
        // Split bits round-trip through the bit set.
        let fl = flags(7);
        let c = TerrainBlock::new(&rough(7), &fl);
        for j in 0..QUAD_SIDE {
            for i in 0..QUAD_SIDE {
                assert_eq!(c.split_sw_ne(i, j), fl.split_sw_ne(i, j));
            }
        }
        assert!(!c.split_sw_ne(32, 0));
    }

    /// Review finding: the ceiling surface splits its quads by bit 0 (the ceiling
    /// diagonal) and ignores bit 7 (the terrain diagonal).
    #[test]
    fn from_ceiling_uses_the_bit_zero_diagonal() {
        use tessera_formats::landblock::Ceiling;
        let mut data = LandblockData::from_heights(Did(0x8001_0000), HeightGrid::flat(100));
        assert!(TerrainBlock::from_ceiling(&data).is_none());
        // A bump at the north-east corner of quads (0, 0) and (2, 0).
        let heights = HeightGrid::from_fn(|i, j| match (i, j) {
            (1, 1) | (3, 1) => 5_000,
            _ => 1_000,
        });
        let mut flags = VertexFlags::default();
        flags.raw_mut()[0] = VertexFlags::CEILING_SPLIT_SW_NE; // quad (0, 0): bit 0
        flags.raw_mut()[2 * GRID_SIDE] = VertexFlags::SPLIT_SW_NE; // quad (2, 0): bit 7
        data.vertex_flags = flags;
        data.ceiling = Some(Ceiling {
            heights,
            tree: None,
            surface: Default::default(),
        });
        let c = TerrainBlock::from_ceiling(&data).unwrap();
        assert!(c.split_sw_ne(0, 0));
        assert!(!c.split_sw_ne(2, 0));
        assert_eq!(c.raw(1, 1), Some(5_000));
        assert_eq!(c.raw(0, 0), Some(1_000));
        let near =
            |z: Option<f32>, raw: f64| (f64::from(z.unwrap()) - raw * UNITS_PER_STEP).abs() < 1e-3;
        // Quad (0, 0), SW-NE: (1.25, 1.25) lies on the diagonal, a quarter of the way up.
        assert!(near(c.height_at(1.25, 1.25), 2_000.0));
        // Quad (2, 0), SE-NW despite bit 7: (11.25, 1.25) is in the flat south-west triangle.
        assert!(near(c.height_at(11.25, 1.25), 1_000.0));
        // The terrain block reads bit 7 instead.
        let t = TerrainBlock::from_landblock(&data);
        assert!(!t.split_sw_ne(0, 0));
        assert!(t.split_sw_ne(2, 0));
    }

    #[test]
    #[cfg(debug_assertions)]
    #[should_panic(expected = "outside 0–32")]
    fn vertex_index_asserts_its_range() {
        let _ = vertex_index(33, 0);
    }
}
