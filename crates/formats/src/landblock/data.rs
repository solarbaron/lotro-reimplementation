// SPDX-License-Identifier: GPL-3.0-or-later
//! Landblock data `0x800RXXYY`: height grid, ceiling, vertex flags, coarse grid, surface
//! layers, collision trees and the box list.
//!
//! Spec: `docs/specs/landblock.md` §3.

use std::ops::Range;

use super::{
    COARSE_SAMPLES, COARSE_SIDE, GRID_SAMPLES, GRID_SIDE, GRID_SPACING, GRID_TRIANGLE_COUNT,
    INFO_DID_OFFSET, LANDBLOCK_SIZE, LayoutRule, QUAD_COUNT, QUAD_SIDE, height_from_raw, put,
    read_landblock_did, tree_build_byte_rule, vertex_xy,
};
use crate::did::{Did, LandblockId, LandblockKind};
use crate::error::{FormatError, Result};
use crate::reader::{Reader, write_vle};
use crate::record::Record;

/// Stored size of one surface layer.
const LAYER_SIZE: usize = 17;
/// Stored size of one box-list entry.
const BOX_SIZE: usize = 24;
/// The u32 1,089 that follows a ceiling tree when it has no `build_byte` (§3.9 fallback).
const VERTEX_FLAG_COUNT_BYTES: [u8; 4] = (GRID_SAMPLES as u32).to_le_bytes();

/// A 33 × 33 grid of stored u16 height samples (terrain or ceiling), **X-major**: sample
/// (i, j) at index 33·i + j, i along X (east), j along Y (north) (`landblock.md` §3.3).
///
/// Convert samples with [`height_from_raw`] (stored × 2,000 / 65,535 in f32, §3.4).
#[derive(Clone, PartialEq, Eq)]
pub struct HeightGrid(Box<[u16; GRID_SAMPLES]>);

impl std::fmt::Debug for HeightGrid {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let (lo, hi) = self.min_max();
        f.debug_struct("HeightGrid")
            .field("min", &lo)
            .field("max", &hi)
            .finish_non_exhaustive()
    }
}

impl Default for HeightGrid {
    fn default() -> Self {
        Self::flat(0)
    }
}

impl HeightGrid {
    /// A grid with every sample equal to `raw`.
    #[must_use]
    pub fn flat(raw: u16) -> Self {
        Self(Box::new([raw; GRID_SAMPLES]))
    }

    /// A grid from 1,089 samples in stored (X-major) order.
    #[must_use]
    pub fn from_raw(raw: [u16; GRID_SAMPLES]) -> Self {
        Self(Box::new(raw))
    }

    /// A grid whose sample (i, j) is `f(i, j)`.
    #[must_use]
    pub fn from_fn(mut f: impl FnMut(usize, usize) -> u16) -> Self {
        let mut g = Self::flat(0);
        for i in 0..GRID_SIDE {
            for j in 0..GRID_SIDE {
                g.0[GRID_SIDE * i + j] = f(i, j);
            }
        }
        g
    }

    /// All samples in stored (X-major) order.
    #[must_use]
    pub fn raw(&self) -> &[u16; GRID_SAMPLES] {
        &self.0
    }

    /// Mutable access to the samples in stored order.
    pub fn raw_mut(&mut self) -> &mut [u16; GRID_SAMPLES] {
        &mut self.0
    }

    /// Stored sample (i, j), i along X; `None` outside 0–32.
    #[must_use]
    pub fn get(&self, i: usize, j: usize) -> Option<u16> {
        (i < GRID_SIDE && j < GRID_SIDE).then(|| self.0[GRID_SIDE * i + j])
    }

    /// Sample (i, j) in world units; `None` outside 0–32.
    #[must_use]
    pub fn height(&self, i: usize, j: usize) -> Option<f32> {
        self.get(i, j).map(height_from_raw)
    }

    /// Whether all 1,089 samples are equal (void, ocean or flat dungeon blocks, §2.3).
    #[must_use]
    pub fn is_constant(&self) -> bool {
        self.0.iter().all(|&h| h == self.0[0])
    }

    /// Smallest and largest stored sample.
    #[must_use]
    pub fn min_max(&self) -> (u16, u16) {
        self.0
            .iter()
            .fold((u16::MAX, 0), |(lo, hi), &h| (lo.min(h), hi.max(h)))
    }

    fn read(r: &mut Reader<'_>) -> Result<Self> {
        let raw = r.bytes(2 * GRID_SAMPLES)?;
        r.charge(2 * GRID_SAMPLES)?;
        let mut g = Self::flat(0);
        for (dst, src) in g.0.iter_mut().zip(raw.as_chunks::<2>().0) {
            *dst = u16::from_le_bytes(*src);
        }
        Ok(g)
    }

    fn write(&self, out: &mut Vec<u8>) {
        for &h in self.0.iter() {
            put::u16(out, h);
        }
    }
}

/// One byte per height sample, in the height grid's (X-major) order (§3.6).
///
/// Bit 7 of byte (i, j) selects the split diagonal of the **terrain** quad whose south-west
/// corner is sample (i, j); bit 0 selects the diagonal of the **ceiling** quad there (§3.5,
/// §3.6, `collision.md` §6.4). Bits 1–6 are OPEN; they are all set or all clear in every
/// observed byte, and Tessera ignores them.
#[derive(Clone, PartialEq, Eq)]
pub struct VertexFlags(Box<[u8; GRID_SAMPLES]>);

impl std::fmt::Debug for VertexFlags {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let set = |bit: u8| self.0.iter().filter(|&&b| b & bit != 0).count();
        f.debug_struct("VertexFlags")
            .field("sw_ne_splits", &set(Self::SPLIT_SW_NE))
            .field("ceiling_sw_ne_splits", &set(Self::CEILING_SPLIT_SW_NE))
            .finish_non_exhaustive()
    }
}

impl Default for VertexFlags {
    fn default() -> Self {
        Self::filled(0)
    }
}

impl VertexFlags {
    /// Bit 7: the terrain quad's diagonal runs south-west to north-east.
    pub const SPLIT_SW_NE: u8 = 0x80;
    /// Bit 0: the ceiling quad's diagonal runs south-west to north-east (§3.5, L6a).
    pub const CEILING_SPLIT_SW_NE: u8 = 0x01;

    /// Every byte equal to `value`.
    #[must_use]
    pub fn filled(value: u8) -> Self {
        Self(Box::new([value; GRID_SAMPLES]))
    }

    /// All bytes in stored (X-major) order.
    #[must_use]
    pub fn raw(&self) -> &[u8; GRID_SAMPLES] {
        &self.0
    }

    /// Mutable access to the bytes in stored order.
    pub fn raw_mut(&mut self) -> &mut [u8; GRID_SAMPLES] {
        &mut self.0
    }

    /// Byte (i, j), i along X; `None` outside 0–32.
    #[must_use]
    pub fn get(&self, i: usize, j: usize) -> Option<u8> {
        (i < GRID_SIDE && j < GRID_SIDE).then(|| self.0[GRID_SIDE * i + j])
    }

    /// Whether terrain quad (i, j) (corners (i, j)–(i + 1, j + 1), 0 ≤ i, j ≤ 31) is split
    /// from south-west to north-east (bit 7 set); `false` means south-east to north-west.
    /// Out-of-range quads report `false`.
    #[must_use]
    pub fn split_sw_ne(&self, i: usize, j: usize) -> bool {
        self.surface_split_sw_ne(GridSurface::Terrain, i, j)
    }

    /// Whether ceiling quad (i, j) is split from south-west to north-east (bit 0 set;
    /// `landblock.md` §3.5, `collision.md` §6.4); `false` means south-east to north-west.
    /// Bit 7 must not be used for ceilings. Out-of-range quads report `false`.
    #[must_use]
    pub fn ceiling_split_sw_ne(&self, i: usize, j: usize) -> bool {
        self.surface_split_sw_ne(GridSurface::Ceiling, i, j)
    }

    /// [`VertexFlags::split_sw_ne`] or [`VertexFlags::ceiling_split_sw_ne`] by `surface`.
    #[must_use]
    pub fn surface_split_sw_ne(&self, surface: GridSurface, i: usize, j: usize) -> bool {
        i < QUAD_SIDE && j < QUAD_SIDE && self.0[GRID_SIDE * i + j] & surface.diagonal_bit() != 0
    }

    fn read(r: &mut Reader<'_>) -> Result<Self> {
        let raw = r.bytes(GRID_SAMPLES)?;
        r.charge(GRID_SAMPLES)?;
        let mut f = Self::filled(0);
        f.0.copy_from_slice(raw);
        Ok(f)
    }
}

/// Which grid surface of a landblock a query is about: the terrain (height grid, diagonal
/// from vertex-flag bit 7, triangles counter-clockwise from above) or the ceiling (ceiling
/// grid, diagonal from bit 0, triangles clockwise from above; `landblock.md` §3.5–§3.6).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum GridSurface {
    /// The terrain surface (faces up).
    #[default]
    Terrain,
    /// The ceiling surface of a cave landblock (faces down).
    Ceiling,
}

impl GridSurface {
    /// The vertex-flag bit that selects this surface's quad diagonal: 0x80 for the terrain,
    /// 0x01 for the ceiling.
    #[must_use]
    pub fn diagonal_bit(self) -> u8 {
        match self {
            Self::Terrain => VertexFlags::SPLIT_SW_NE,
            Self::Ceiling => VertexFlags::CEILING_SPLIT_SW_NE,
        }
    }
}

/// The two **terrain** triangles of quad (i, j) as grid sample coordinates (i along X, j
/// along Y), counter-clockwise seen from above (`landblock.md` §3.6 table). `split_sw_ne`
/// is bit 7 of the vertex-flag byte (i, j) ([`VertexFlags::split_sw_ne`]).
///
/// With the south-west/north-east diagonal, element 1 is the triangle west of the
/// diagonal; with the other diagonal, element 0 is ([`grid_triangle_key`]).
#[must_use]
pub fn quad_triangles(i: usize, j: usize, split_sw_ne: bool) -> [[(usize, usize); 3]; 2] {
    if split_sw_ne {
        [
            [(i, j), (i + 1, j), (i + 1, j + 1)],
            [(i, j), (i + 1, j + 1), (i, j + 1)],
        ]
    } else {
        [
            [(i, j), (i + 1, j), (i, j + 1)],
            [(i + 1, j), (i + 1, j + 1), (i, j + 1)],
        ]
    }
}

/// The two **ceiling** triangles of quad (i, j): the same split as [`quad_triangles`] for the
/// diagonal, with each triangle's corners in reverse order, so they are clockwise seen from
/// above (they face down; `landblock.md` §3.5). `split_sw_ne` is bit 0 of the vertex-flag
/// byte (i, j) ([`VertexFlags::ceiling_split_sw_ne`]).
#[must_use]
pub fn ceiling_quad_triangles(i: usize, j: usize, split_sw_ne: bool) -> [[(usize, usize); 3]; 2] {
    quad_triangles(i, j, split_sw_ne).map(|[a, b, c]| [c, b, a])
}

/// [`quad_triangles`] or [`ceiling_quad_triangles`] by `surface`.
#[must_use]
pub fn surface_quad_triangles(
    surface: GridSurface,
    i: usize,
    j: usize,
    split_sw_ne: bool,
) -> [[(usize, usize); 3]; 2] {
    match surface {
        GridSurface::Terrain => quad_triangles(i, j, split_sw_ne),
        GridSurface::Ceiling => ceiling_quad_triangles(i, j, split_sw_ne),
    }
}

/// The key of a grid triangle in the numbering of terrain and ceiling collision trees
/// (`collision.md` §6.2, `landblock.md` §3.9): 2 · (32 · j + i) for the triangle **west** of
/// quad (i, j)'s diagonal, plus 1 for the other. Quads are numbered Y-major although the
/// height grid is stored X-major. `None` outside 0 ≤ i, j ≤ 31.
#[must_use]
pub fn grid_triangle_key(i: usize, j: usize, west: bool) -> Option<u16> {
    if i >= QUAD_SIDE || j >= QUAD_SIDE {
        return None;
    }
    u16::try_from(2 * (QUAD_SIDE * j + i) + usize::from(!west)).ok()
}

/// The inverse of [`grid_triangle_key`]: (i, j, west) of key `key`, or `None` for a key
/// ≥ 2,048.
#[must_use]
pub fn grid_triangle_quad(key: u16) -> Option<(usize, usize, bool)> {
    let key = usize::from(key);
    if key >= GRID_TRIANGLE_COUNT {
        return None;
    }
    let quad = key / 2;
    Some((quad % QUAD_SIDE, quad / QUAD_SIDE, key % 2 == 0))
}

/// How a surface layer is used (`landblock.md` §3.8.2 "Layer kinds").
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum LayerKind {
    /// Triangles drawn through the layer's alpha mask.
    Masked,
    /// Triangles drawn fully opaque (no mask).
    Opaque,
    /// `whole` = 1: the layer covers **all 2,048 grid triangles** of the block (the full
    /// triangulation of §3.6: the bit-7 diagonal and counter-clockwise winding in a terrain
    /// chunk, the bit-0 diagonal and clockwise winding in a ceiling chunk, §3.5), drawn
    /// through the mask, or with mask value 1 when there is none. The stored index window
    /// (`start`, `prims`, `min_vertex`, `vertex_span`, all 0) is ignored (§3.8.2;
    /// `terrain-texturing.md` §4.3). [`LandblockData::layer_triangles`] lists them.
    Whole,
    /// No geometry: skip. (A mask named by such a layer does not exist in the install.)
    Empty,
}

/// One texture layer of a surface chunk (17 bytes; `landblock.md` §3.8.2).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct SurfaceLayer {
    /// Terrain-type key: the `key` of an entry of the region's terrain type table (prefix
    /// 0x19, region-descriptor slot 8; §3.8.2, `terrain-texturing.md` §3, §4.1).
    pub material: u32,
    /// A 0x41 render surface holding the layer's alpha mask, or 0.
    pub mask: Did,
    /// `whole` flag: the layer covers every grid triangle of the block and its index
    /// window is ignored (see [`LayerKind::Whole`]).
    pub whole: bool,
    /// First index (not triangle) of this layer's triangles in the chunk's index list.
    pub start: u16,
    /// Number of triangles.
    pub prims: u16,
    /// Smallest vertex number used by the triangles.
    pub min_vertex: u16,
    /// Number of vertex numbers spanned from `min_vertex`.
    pub vertex_span: u16,
}

impl SurfaceLayer {
    /// How the layer is used.
    #[must_use]
    pub fn kind(&self) -> LayerKind {
        match (self.whole, self.prims, self.mask.is_null()) {
            (true, _, _) => LayerKind::Whole,
            (false, 0, _) => LayerKind::Empty,
            (false, _, false) => LayerKind::Masked,
            (false, _, true) => LayerKind::Opaque,
        }
    }

    /// The layer's window in the chunk's index list: `start .. start + 3·prims`.
    #[must_use]
    pub fn index_range(&self) -> Range<usize> {
        let start = usize::from(self.start);
        start..start + 3 * usize::from(self.prims)
    }

    fn read(r: &mut Reader<'_>) -> Result<Self> {
        Ok(Self {
            material: r.u32()?,
            mask: r.did()?,
            whole: r.bool8()?,
            start: r.u16()?,
            prims: r.u16()?,
            min_vertex: r.u16()?,
            vertex_span: r.u16()?,
        })
    }

    fn write(&self, out: &mut Vec<u8>) {
        put::u32(out, self.material);
        put::u32(out, self.mask.0);
        out.push(u8::from(self.whole));
        put::u16(out, self.start);
        put::u16(out, self.prims);
        put::u16(out, self.min_vertex);
        put::u16(out, self.vertex_span);
    }
}

/// The texture layers of one surface (the terrain or the ceiling) and the triangles each
/// covers (`landblock.md` §3.8.1).
///
/// The index list holds **Y-major** vertex numbers v = 33·y + x (§3.8.3); convert with
/// [`super::vertex_xy`] or [`super::vertex_to_height_index`]. A decoded chunk has every
/// index < 1,089 and every layer window inside the list.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct SurfaceChunk {
    /// Layers in stored order.
    pub layers: Vec<SurfaceLayer>,
    /// Shared triangle list over grid vertices (three numbers per triangle).
    pub indices: Vec<u16>,
    /// One `material` (terrain-type key) per quad (32 × 32), present exactly when there are
    /// layers: quad (qx, qy) at index 32·qy + qx (§3.8.4, order Confirmed). Probably the
    /// type most visible in the quad; not used for drawing.
    pub quads: Option<Box<[u16; QUAD_COUNT]>>,
}

impl SurfaceChunk {
    /// The vertex numbers of `layer`'s stored index window, or `None` when the window lies
    /// outside the index list (never for a decoded chunk). A [`LayerKind::Whole`] layer has
    /// an empty window although it covers the whole block; use
    /// [`LandblockData::layer_triangles`] for the triangles a layer actually covers.
    #[must_use]
    pub fn layer_indices(&self, layer: &SurfaceLayer) -> Option<&[u16]> {
        self.indices.get(layer.index_range())
    }

    /// The quad-grid entry of quad (qx, qy): index 32·qy + qx (§3.8.4). `None` without a
    /// quad grid or outside 0 ≤ qx, qy ≤ 31.
    #[must_use]
    pub fn quad_material(&self, qx: usize, qy: usize) -> Option<u16> {
        let q = self.quads.as_ref()?;
        (qx < QUAD_SIDE && qy < QUAD_SIDE).then(|| q[QUAD_SIDE * qy + qx])
    }

    fn read(r: &mut Reader<'_>) -> Result<Self> {
        let count = r.u32()?;
        let layers_at = r.position();
        let mut layers = r.alloc_vec(count, LAYER_SIZE, usize::MAX)?;
        for _ in 0..count {
            let l = SurfaceLayer::read(r)?;
            r.push(&mut layers, l, count)?;
        }
        let index_count = r.vle()?;
        let index_at = r.position();
        r.ensure_elements(index_count, 2)?;
        let n = usize::try_from(index_count).unwrap_or(usize::MAX);
        let raw = r.bytes(n.saturating_mul(2))?;
        r.charge(raw.len())?;
        let mut indices = Vec::new();
        indices
            .try_reserve_exact(n)
            .map_err(|_| FormatError::MemoryLimit {
                offset: index_at,
                limit: r.budget(),
            })?;
        for (k, c) in raw.as_chunks::<2>().0.iter().enumerate() {
            let v = u16::from_le_bytes([c[0], c[1]]);
            if usize::from(v) >= GRID_SAMPLES {
                return Err(FormatError::IndexOutOfRange {
                    what: "surface vertex number",
                    offset: index_at + 2 * k,
                    index: u64::from(v),
                    limit: GRID_SAMPLES as u64,
                });
            }
            indices.push(v);
        }
        let quad_at = r.position();
        let quad_count = r.vle()?;
        let quads = match quad_count {
            0 => None,
            q if q as usize == QUAD_COUNT => {
                let raw = r.bytes(2 * QUAD_COUNT)?;
                r.charge(2 * QUAD_COUNT)?;
                let mut grid = Box::new([0u16; QUAD_COUNT]);
                for (dst, c) in grid.iter_mut().zip(raw.as_chunks::<2>().0) {
                    *dst = u16::from_le_bytes(*c);
                }
                Some(grid)
            }
            other => {
                return Err(FormatError::CountMismatch {
                    what: "surface quad-grid count",
                    offset: quad_at,
                    expected: QUAD_COUNT as u32,
                    found: other,
                });
            }
        };
        for (k, l) in layers.iter().enumerate() {
            let end = l.index_range().end;
            if end > indices.len() {
                return Err(FormatError::IndexOutOfRange {
                    what: "surface layer window end",
                    // The layer's `start` field.
                    offset: layers_at + LAYER_SIZE * k + 9,
                    index: end as u64,
                    limit: indices.len() as u64,
                });
            }
        }
        Ok(Self {
            layers,
            indices,
            quads,
        })
    }

    fn write(&self, out: &mut Vec<u8>) {
        put::u32(out, put::len_u32(self.layers.len()));
        for l in &self.layers {
            l.write(out);
        }
        write_vle(out, put::len_u32(self.indices.len()));
        for &v in &self.indices {
            put::u16(out, v);
        }
        match &self.quads {
            None => write_vle(out, 0),
            Some(q) => {
                write_vle(out, QUAD_COUNT as u32);
                for &v in q.iter() {
                    put::u16(out, v);
                }
            }
        }
    }

    /// First deviation from invariants HM5/HM6 (§3.12), or `None`. `flags` gives the split
    /// diagonals. Every triangle must lie in one quad and use the diagonal `surface`'s bit
    /// gives (7 for terrain, 0 for ceiling), counter-clockwise from above for the terrain
    /// and clockwise for the ceiling (§3.5, §3.6).
    fn first_unusual(&self, flags: &VertexFlags, surface: GridSurface) -> Option<&'static str> {
        if self.quads.is_some() != !self.layers.is_empty() {
            return Some("quad grid present iff layers present (HM5)");
        }
        let prims: usize = self.layers.iter().map(|l| usize::from(l.prims)).sum();
        if 3 * prims != self.indices.len() {
            return Some("index count is not 3 × Σ prims (HM5)");
        }
        for l in &self.layers {
            let Some(tris) = self.layer_indices(l) else {
                return Some("layer window outside index list (HM5)");
            };
            let lo = usize::from(l.min_vertex);
            let hi = lo + usize::from(l.vertex_span);
            if tris.iter().any(|&v| !(lo..hi).contains(&usize::from(v))) {
                return Some("layer vertex outside its vertex window (HM5)");
            }
            for t in tris.as_chunks::<3>().0 {
                if let Some(why) = triangle_unusual([t[0], t[1], t[2]], flags, surface) {
                    return Some(why);
                }
            }
        }
        None
    }
}

/// Checks one stored triangle against HM6: inside one quad, using the quad's diagonal for
/// `surface`, counter-clockwise (terrain) or clockwise (ceiling) seen from above.
fn triangle_unusual(
    t: [u16; 3],
    flags: &VertexFlags,
    surface: GridSurface,
) -> Option<&'static str> {
    let mut p = [(0usize, 0usize); 3];
    for (dst, &v) in p.iter_mut().zip(&t) {
        let Some(xy) = vertex_xy(v) else {
            return Some("layer vertex number out of range");
        };
        *dst = xy;
    }
    let i = p.iter().map(|q| q.0).min().unwrap_or(0);
    let j = p.iter().map(|q| q.1).min().unwrap_or(0);
    if i >= QUAD_SIDE
        || j >= QUAD_SIDE
        || p.iter().any(|&(x, y)| x > i + 1 || y > j + 1)
        || p[0] == p[1]
        || p[1] == p[2]
        || p[0] == p[2]
    {
        return Some("layer triangle not inside one grid quad (HM6)");
    }
    let diagonal = if flags.surface_split_sw_ne(surface, i, j) {
        [(i, j), (i + 1, j + 1)]
    } else {
        [(i + 1, j), (i, j + 1)]
    };
    let terrain = surface == GridSurface::Terrain;
    if !diagonal.iter().all(|d| p.contains(d)) {
        return Some(if terrain {
            "terrain triangle does not use the bit-7 diagonal (HM6)"
        } else {
            "ceiling triangle does not use the bit-0 diagonal (HM6)"
        });
    }
    let s = |v: usize| v as i64;
    let cross = (s(p[1].0) - s(p[0].0)) * (s(p[2].1) - s(p[0].1))
        - (s(p[1].1) - s(p[0].1)) * (s(p[2].0) - s(p[0].0));
    match (terrain, cross > 0) {
        (true, false) => Some("terrain triangle not counter-clockwise from above (HM6)"),
        (false, true) => Some("ceiling triangle not clockwise from above (HM6)"),
        _ => None,
    }
}

/// A terrain or ceiling collision tree block (`landblock.md` §3.9): the container of
/// `collision.md` §3.1 without its presence byte (`has_terrain_tree` / `has_ceiling_tree`
/// play that role). This decoder only frames the block; the quantised bounding-volume
/// program in [`CollisionTree::code`] is walked by the collision module's tree reader
/// (`collision.md` §3.3–§3.7).
///
/// The program's keys are grid triangle numbers ([`grid_triangle_key`], `collision.md`
/// §6.2): a terrain tree indexes the 2,048 triangles of the height grid with the bit-7
/// diagonals, a ceiling tree those of the ceiling grid with the bit-0 diagonals
/// (`collision.md` §6.4). The triangles themselves come from the grids
/// ([`LandblockData::grid_triangle_positions`]); the tree is not needed to collide with
/// them (`collision.md` §6.3).
#[derive(Debug, Clone, PartialEq, Default)]
pub struct CollisionTree {
    /// Minimum corner of the quantisation box, block-local units (`collision.md` §3.2).
    pub origin: [f32; 3],
    /// Integer units per world unit (`collision.md` §3.2, §3.3).
    pub scale: f32,
    /// The program bytes (`code_length` of them), not interpreted here.
    pub code: Vec<u8>,
    /// The trailing byte (value 1) of records with stamp ≥ 38, else `None`.
    pub build_byte: Option<u8>,
}

impl CollisionTree {
    /// Stored size of the block in bytes, without the presence byte: origin, scale,
    /// `code_length`, code and the `build_byte` if present.
    #[must_use]
    pub fn stored_len(&self) -> usize {
        4 * 3 + 4 + 4 + self.code.len() + usize::from(self.build_byte.is_some())
    }

    /// The block as stored, without the presence byte (for a reader that takes the
    /// container bytes).
    #[must_use]
    pub fn to_bytes(&self) -> Vec<u8> {
        let mut out = Vec::with_capacity(self.stored_len());
        self.write(&mut out);
        out
    }

    /// Reads origin, scale and code; the `build_byte` is decided by the caller.
    fn read_core(r: &mut Reader<'_>) -> Result<Self> {
        let origin = [r.f32()?, r.f32()?, r.f32()?];
        let scale = r.f32()?;
        let len = r.u32()?;
        let code = r.bytes(usize::try_from(len).unwrap_or(usize::MAX))?;
        r.charge(code.len())?;
        Ok(Self {
            origin,
            scale,
            code: code.to_vec(),
            build_byte: None,
        })
    }

    fn write(&self, out: &mut Vec<u8>) {
        put::f32s(out, &self.origin);
        put::f32s(out, &[self.scale]);
        put::u32(out, put::len_u32(self.code.len()));
        out.extend_from_slice(&self.code);
        if let Some(b) = self.build_byte {
            out.push(b);
        }
    }
}

/// One box-list entry: six floats (`landblock.md` §3.10, meaning OPEN). In inspected entries
/// they read as (min x, min y, min z, max x, max y, max z) in block-local units.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct LandblockBox {
    /// The six floats as stored.
    pub values: [f32; 6],
}

impl LandblockBox {
    /// First three floats (the minimum corner under the observed reading).
    #[must_use]
    pub fn min(&self) -> [f32; 3] {
        [self.values[0], self.values[1], self.values[2]]
    }

    /// Last three floats (the maximum corner under the observed reading).
    #[must_use]
    pub fn max(&self) -> [f32; 3] {
        [self.values[3], self.values[4], self.values[5]]
    }
}

/// The ceiling of a cave landblock (`landblock.md` §3.5): present exactly when the block
/// map gives `Landblock_UseCeiling` = 1.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Ceiling {
    /// Ceiling heights, same order and scale as the terrain; stored 0 means "no ceiling at
    /// this vertex".
    pub heights: HeightGrid,
    /// The ceiling collision tree, if any.
    pub tree: Option<CollisionTree>,
    /// The ceiling's surface layers (stored after the terrain chunk).
    pub surface: SurfaceChunk,
}

impl Ceiling {
    /// Whether ceiling sample (i, j) holds a ceiling (stored value ≠ 0, §3.5). `false`
    /// outside 0–32. Samples without a ceiling still take part in the ceiling collision mesh,
    /// at height 0 (`collision.md` §6.4).
    #[must_use]
    pub fn has_sample(&self, i: usize, j: usize) -> bool {
        self.heights.get(i, j).is_some_and(|h| h != 0)
    }
}

/// A decoded landblock-data record `0x800RXXYY` (`landblock.md` §3), consumed to the last
/// byte.
#[derive(Debug, Clone, PartialEq)]
pub struct LandblockData {
    /// Own DID.
    pub did: Did,
    /// The landblock-info DID named by the header (own DID + 0x00200000), or `None` when
    /// the header holds 0 (no landblock-info record exists then).
    pub info: Option<Did>,
    /// Header offset 8; 0 in every observed record.
    pub reserved: u32,
    /// Terrain heights.
    pub heights: HeightGrid,
    /// Ceiling grid, tree and surface (cave blocks).
    pub ceiling: Option<Ceiling>,
    /// Vertex flags (bit 7: terrain split diagonal; bit 0: ceiling split diagonal).
    pub vertex_flags: VertexFlags,
    /// The 17 × 17 coarse grid of per-vertex scenery words (`landblock.md` §3.7): word
    /// (i, j) is at index 17 · i + j with i along X and belongs to height sample (2i, 2j)
    /// (see [`LandblockData::coarse_word`]). Bits 0–7: terrain-type id for scenery; bits
    /// 8–14: scene slot; bits 15–19: encounter slot; bits 20–31: unused. Bits 15–31 are 0
    /// in every observed record (`scenery.md` SC6).
    pub coarse: Box<[u32; COARSE_SAMPLES]>,
    /// The terrain's surface layers.
    pub terrain: SurfaceChunk,
    /// The terrain collision tree, if any.
    pub terrain_tree: Option<CollisionTree>,
    /// The box list: `None` when `has_boxes` = 0, else the (possibly empty) list.
    pub boxes: Option<Vec<LandblockBox>>,
    /// How stamp-dependent details (tree `build_byte`s) were decided.
    pub layout_rule: LayoutRule,
}

impl LandblockData {
    /// Human-readable record type name, used in errors.
    pub const NAME: &'static str = "landblock data";

    /// Coarse-grid word (i, j), i along X, for 0 ≤ i, j ≤ 16 (`landblock.md` §3.7): the
    /// scenery word of height sample (2i, 2j). `None` outside the grid.
    #[must_use]
    pub fn coarse_word(&self, i: usize, j: usize) -> Option<u32> {
        if i >= COARSE_SIDE || j >= COARSE_SIDE {
            return None;
        }
        self.coarse.get(COARSE_SIDE * i + j).copied()
    }

    /// A minimal record for `did` with the given heights: no info, no ceiling, all vertex
    /// flags 0 (every quad split south-east to north-west), an all-zero coarse grid, no
    /// layers, trees or boxes. Meant for synthetic tests of higher layers.
    #[must_use]
    pub fn from_heights(did: Did, heights: HeightGrid) -> Self {
        Self {
            did,
            info: None,
            reserved: 0,
            heights,
            ceiling: None,
            vertex_flags: VertexFlags::default(),
            coarse: Box::new([0; COARSE_SAMPLES]),
            terrain: SurfaceChunk::default(),
            terrain_tree: None,
            boxes: None,
            layout_rule: LayoutRule::Stamp,
        }
    }

    /// Decodes a complete payload with the record's directory `stamp` (or `None` to use the
    /// §3.9 fallback for tree `build_byte`s). The payload must be consumed exactly.
    pub fn parse(data: &[u8], stamp: Option<u16>) -> Result<Self> {
        let mut r = Reader::new(data);
        let rec = Self::read(&mut r, stamp)?;
        r.expect_end(Self::NAME)?;
        Ok(rec)
    }

    /// [`LandblockData::parse`], also checking the record's DID against the one looked up.
    pub fn parse_expecting(
        data: &[u8],
        expected: impl Into<Did>,
        stamp: Option<u16>,
    ) -> Result<Self> {
        let expected = expected.into();
        let rec = Self::parse(data, stamp)?;
        if rec.did != expected {
            return Err(FormatError::DidMismatch {
                expected: expected.0,
                found: rec.did.0,
            });
        }
        Ok(rec)
    }

    /// Reads only the header and the height grid (§3.1 parts 1–2), for readers that need
    /// heights alone. The rest of the payload is not examined.
    pub fn parse_heights(data: &[u8]) -> Result<(Did, Option<Did>, HeightGrid)> {
        let mut r = Reader::new(data);
        let (did, info, _) = Self::read_header(&mut r)?;
        Ok((did, info, HeightGrid::read(&mut r)?))
    }

    fn read_header(r: &mut Reader<'_>) -> Result<(Did, Option<Did>, u32)> {
        let did = read_landblock_did(r, LandblockKind::Heightmap, Self::NAME)?;
        let info = r.did()?;
        let expected_info = Did(did.0.wrapping_add(INFO_DID_OFFSET));
        if !info.is_null() && info != expected_info {
            return Err(FormatError::DidMismatch {
                expected: expected_info.0,
                found: info.0,
            });
        }
        let reserved = r.u32()?;
        let count_at = r.position();
        let count = r.u32()?;
        if count as usize != GRID_SAMPLES {
            return Err(FormatError::CountMismatch {
                what: "height-sample count",
                offset: count_at,
                expected: GRID_SAMPLES as u32,
                found: count,
            });
        }
        Ok((did, (!info.is_null()).then_some(info), reserved))
    }

    fn read(r: &mut Reader<'_>, stamp: Option<u16>) -> Result<Self> {
        let (did, info, reserved) = Self::read_header(r)?;
        let heights = HeightGrid::read(r)?;
        let (rule, mut layout_rule) = tree_build_byte_rule(stamp);
        let has_ceiling = r.bool8()?;
        let mut ceiling_parts = None;
        if has_ceiling {
            read_grid_count(r, "ceiling-sample count", GRID_SAMPLES)?;
            let ceiling_heights = HeightGrid::read(r)?;
            let tree = if r.bool8()? {
                let mut t = CollisionTree::read_core(r)?;
                let present = match rule {
                    Some(p) => p,
                    // A ceiling tree is followed by the vertex-flag count unless a build
                    // byte comes first.
                    None => r.peek(4).ok() != Some(&VERTEX_FLAG_COUNT_BYTES[..]),
                };
                if present {
                    t.build_byte = Some(r.u8()?);
                }
                Some(t)
            } else {
                None
            };
            ceiling_parts = Some((ceiling_heights, tree));
        }
        read_grid_count(r, "vertex-flag count", GRID_SAMPLES)?;
        let vertex_flags = VertexFlags::read(r)?;
        read_grid_count(r, "coarse-grid count", COARSE_SAMPLES)?;
        let raw = r.bytes(4 * COARSE_SAMPLES)?;
        r.charge(4 * COARSE_SAMPLES)?;
        let mut coarse = Box::new([0u32; COARSE_SAMPLES]);
        for (dst, c) in coarse.iter_mut().zip(raw.as_chunks::<4>().0) {
            *dst = u32::from_le_bytes(*c);
        }
        let terrain = SurfaceChunk::read(r)?;
        let ceiling = match ceiling_parts {
            Some((heights, tree)) => Some(Ceiling {
                heights,
                tree,
                surface: SurfaceChunk::read(r)?,
            }),
            None => None,
        };
        let terrain_tree = if r.bool8()? {
            let mut t = CollisionTree::read_core(r)?;
            let present = match rule {
                Some(p) => p,
                None => {
                    // Present exactly when consuming it lets the rest parse as parts 13–14
                    // with nothing left over.
                    let mut probe = r.clone();
                    probe.u8().is_ok() && read_boxes(&mut probe).is_ok() && probe.is_empty()
                }
            };
            if present {
                t.build_byte = Some(r.u8()?);
            }
            Some(t)
        } else {
            None
        };
        let boxes = read_boxes(r)?;
        if stamp.is_some()
            && ceiling.as_ref().is_none_or(|c| c.tree.is_none())
            && terrain_tree.is_none()
        {
            // No tree: nothing depended on the stamp, so an unobserved stamp is not
            // unusual. Without a stamp the rule stays `Fallback` (see `LayoutRule`).
            layout_rule = LayoutRule::Stamp;
        }
        Ok(Self {
            did,
            info,
            reserved,
            heights,
            ceiling,
            vertex_flags,
            coarse,
            terrain,
            terrain_tree,
            boxes,
            layout_rule,
        })
    }

    /// Re-encodes the record. Tree `build_byte`s are written when present in the value, so
    /// a decoded record re-encodes byte for byte whatever its stamp.
    #[must_use]
    pub fn to_bytes(&self) -> Vec<u8> {
        let mut out = Vec::with_capacity(4456);
        put::u32(&mut out, self.did.0);
        put::u32(&mut out, self.info.map_or(0, |d| d.0));
        put::u32(&mut out, self.reserved);
        put::u32(&mut out, GRID_SAMPLES as u32);
        self.heights.write(&mut out);
        out.push(u8::from(self.ceiling.is_some()));
        if let Some(c) = &self.ceiling {
            put::u32(&mut out, GRID_SAMPLES as u32);
            c.heights.write(&mut out);
            out.push(u8::from(c.tree.is_some()));
            if let Some(t) = &c.tree {
                t.write(&mut out);
            }
        }
        put::u32(&mut out, GRID_SAMPLES as u32);
        out.extend_from_slice(&self.vertex_flags.0[..]);
        put::u32(&mut out, COARSE_SAMPLES as u32);
        for &v in self.coarse.iter() {
            put::u32(&mut out, v);
        }
        self.terrain.write(&mut out);
        if let Some(c) = &self.ceiling {
            c.surface.write(&mut out);
        }
        out.push(u8::from(self.terrain_tree.is_some()));
        if let Some(t) = &self.terrain_tree {
            t.write(&mut out);
        }
        out.push(u8::from(self.boxes.is_some()));
        if let Some(b) = &self.boxes {
            put::u32(&mut out, put::len_u32(b.len()));
            for e in b {
                put::f32s(&mut out, &e.values);
            }
        }
        out
    }

    /// The landblock this record describes.
    #[must_use]
    pub fn landblock(&self) -> Option<LandblockId> {
        LandblockId::from_did(self.did)
    }

    /// Whether every terrain sample is equal (§2.3).
    #[must_use]
    pub fn is_constant_height(&self) -> bool {
        self.heights.is_constant()
    }

    /// The two terrain triangles of quad (i, j), 0 ≤ i, j ≤ 31, as height-grid sample
    /// coordinates, counter-clockwise from above (§3.6, §3.11). `None` outside the grid.
    #[must_use]
    pub fn quad_triangles(&self, i: usize, j: usize) -> Option<[[(usize, usize); 3]; 2]> {
        (i < QUAD_SIDE && j < QUAD_SIDE)
            .then(|| quad_triangles(i, j, self.vertex_flags.split_sw_ne(i, j)))
    }

    /// Terrain height in world units at block-local (x, y) (`landblock.md` §3.11): the
    /// plane of the triangle containing the point, with the quad split by bit 7 of the
    /// vertex flags. x and y are clamped to [0, 160]; `None` for a NaN coordinate.
    #[must_use]
    pub fn terrain_height(&self, x: f32, y: f32) -> Option<f32> {
        triangulated_height(
            &self.heights,
            &self.vertex_flags,
            GridSurface::Terrain,
            x,
            y,
        )
    }

    /// Ceiling height in world units at block-local (x, y): the §3.11 rule applied to the
    /// ceiling grid with the quad split by **bit 0** of the vertex flags (§3.5). Samples
    /// without a ceiling (stored 0) count as height 0, as in the ceiling collision mesh
    /// (`collision.md` §6.4); use [`Ceiling::has_sample`] to tell them apart. `None` without
    /// a ceiling or for a NaN coordinate.
    #[must_use]
    pub fn ceiling_height(&self, x: f32, y: f32) -> Option<f32> {
        let c = self.ceiling.as_ref()?;
        triangulated_height(&c.heights, &self.vertex_flags, GridSurface::Ceiling, x, y)
    }

    /// The grid of `surface`: the terrain heights, or the ceiling heights (`None` without a
    /// ceiling).
    #[must_use]
    pub fn surface_heights(&self, surface: GridSurface) -> Option<&HeightGrid> {
        match surface {
            GridSurface::Terrain => Some(&self.heights),
            GridSurface::Ceiling => self.ceiling.as_ref().map(|c| &c.heights),
        }
    }

    /// The collision tree block of `surface`, if the record stores one (§3.9).
    #[must_use]
    pub fn surface_tree(&self, surface: GridSurface) -> Option<&CollisionTree> {
        match surface {
            GridSurface::Terrain => self.terrain_tree.as_ref(),
            GridSurface::Ceiling => self.ceiling.as_ref()?.tree.as_ref(),
        }
    }

    /// The two ceiling triangles of quad (i, j), 0 ≤ i, j ≤ 31, as grid sample
    /// coordinates, split by bit 0 and clockwise from above (§3.5, §3.6). `None` outside the
    /// grid or without a ceiling.
    #[must_use]
    pub fn ceiling_quad_triangles(&self, i: usize, j: usize) -> Option<[[(usize, usize); 3]; 2]> {
        self.surface_quad_triangles(GridSurface::Ceiling, i, j)
    }

    /// The two triangles of quad (i, j) of `surface` ([`LandblockData::quad_triangles`] or
    /// [`LandblockData::ceiling_quad_triangles`]). `None` outside the grid, or for the
    /// ceiling of a block without one.
    #[must_use]
    pub fn surface_quad_triangles(
        &self,
        surface: GridSurface,
        i: usize,
        j: usize,
    ) -> Option<[[(usize, usize); 3]; 2]> {
        self.surface_heights(surface)?;
        (i < QUAD_SIDE && j < QUAD_SIDE).then(|| {
            surface_quad_triangles(
                surface,
                i,
                j,
                self.vertex_flags.surface_split_sw_ne(surface, i, j),
            )
        })
    }

    /// The surface chunk of `surface`: the terrain chunk, or the ceiling chunk (`None`
    /// without a ceiling).
    #[must_use]
    pub fn surface_chunk(&self, surface: GridSurface) -> Option<&SurfaceChunk> {
        match surface {
            GridSurface::Terrain => Some(&self.terrain),
            GridSurface::Ceiling => self.ceiling.as_ref().map(|c| &c.surface),
        }
    }

    /// The triangles `layer` of `surface`'s chunk covers, as Y-major vertex numbers
    /// (v = 33·y + x, like the chunk's index list; `landblock.md` §3.8.2–§3.8.3):
    ///
    /// - [`LayerKind::Masked`] and [`LayerKind::Opaque`]: the stored window, three numbers
    ///   per triangle;
    /// - [`LayerKind::Whole`]: all 2,048 grid triangles in collision-key order
    ///   (see [`grid_triangle_key`]), split and wound as the surface is (bit 7 and
    ///   counter-clockwise for the terrain, bit 0 and clockwise for the ceiling, §3.5–§3.6);
    /// - an [`LayerKind::Empty`] layer: none.
    ///
    /// `None` for the ceiling of a block without one, or a window outside the index list
    /// (never for a decoded record). At most 2,048 triangles for a whole layer; a stored
    /// window is bounded by the decoded index list.
    #[must_use]
    pub fn layer_triangles(
        &self,
        surface: GridSurface,
        layer: &SurfaceLayer,
    ) -> Option<Vec<[u16; 3]>> {
        let chunk = self.surface_chunk(surface)?;
        match layer.kind() {
            LayerKind::Empty => Some(Vec::new()),
            LayerKind::Masked | LayerKind::Opaque => {
                Some(chunk.layer_indices(layer)?.as_chunks::<3>().0.to_vec())
            }
            LayerKind::Whole => {
                let vertex = |(i, j): (usize, usize)| u16::try_from(GRID_SIDE * j + i).ok();
                let mut out = Vec::with_capacity(GRID_TRIANGLE_COUNT);
                for key in 0..GRID_TRIANGLE_COUNT {
                    let tri = self.grid_triangle(surface, u16::try_from(key).ok()?)?;
                    out.push([vertex(tri[0])?, vertex(tri[1])?, vertex(tri[2])?]);
                }
                Some(out)
            }
        }
    }

    /// Grid triangle `key` of `surface` in the collision-tree numbering (`collision.md`
    /// §6.2, [`grid_triangle_key`]), as grid sample coordinates in the surface's winding
    /// (counter-clockwise from above for the terrain, clockwise for the ceiling). `None`
    /// for a key ≥ 2,048 or the ceiling of a block without one.
    #[must_use]
    pub fn grid_triangle(&self, surface: GridSurface, key: u16) -> Option<[(usize, usize); 3]> {
        let (i, j, west) = grid_triangle_quad(key)?;
        let split = self.vertex_flags.surface_split_sw_ne(surface, i, j);
        let tris = self.surface_quad_triangles(surface, i, j)?;
        // quad_triangles lists the west triangle second for the SW-NE diagonal, first for
        // the SE-NW diagonal.
        Some(tris[usize::from(split == west)])
    }

    /// Grid triangle `key` of `surface` as three block-local positions (5 · i, 5 · j,
    /// height), heights by §3.4, in the order of [`LandblockData::grid_triangle`]. This is
    /// the triangle of the terrain or ceiling collision mesh with that key (`collision.md`
    /// §6.1, §6.4); every key 0–2,047 has one.
    #[must_use]
    pub fn grid_triangle_positions(&self, surface: GridSurface, key: u16) -> Option<[[f32; 3]; 3]> {
        let grid = self.surface_heights(surface)?;
        let tri = self.grid_triangle(surface, key)?;
        let mut out = [[0.0; 3]; 3];
        for (dst, (i, j)) in out.iter_mut().zip(tri) {
            #[allow(clippy::cast_precision_loss)]
            let (x, y) = (i as f32 * GRID_SPACING, j as f32 * GRID_SPACING);
            *dst = [x, y, grid.height(i, j)?];
        }
        Some(out)
    }

    /// The first deviation from the invariants observed on every real record (§3.12
    /// HM2–HM6 within the record, plus the census facts of §3.5–§3.10), or `None`. Such
    /// records still decode.
    #[must_use]
    pub fn first_unusual_field(&self) -> Option<&'static str> {
        if self.reserved != 0 {
            return Some("header reserved word is not 0 (HM2)");
        }
        if self.layout_rule == LayoutRule::UnobservedStamp {
            return Some("tree build byte decided by fallback for an unobserved stamp");
        }
        let trees = self
            .terrain_tree
            .iter()
            .chain(self.ceiling.as_ref().and_then(|c| c.tree.as_ref()));
        for t in trees {
            if t.build_byte.is_some_and(|b| b != 1) {
                return Some("tree build byte is not 1");
            }
        }
        if self
            .vertex_flags
            .0
            .iter()
            .any(|&b| !matches!(b & 0x7F, 0 | 1 | 126 | 127))
        {
            return Some("vertex-flag low bits outside {0, 1, 126, 127}");
        }
        // landblock.md §3.7, scenery.md SC6: the encounter slot (bits 15–19) and bits
        // 20–31 are 0 in every observed record.
        if self.coarse.iter().any(|&v| v >> 15 != 0) {
            return Some("coarse-grid bits 15–31 set");
        }
        if self.boxes.is_some() && self.ceiling.is_none() {
            return Some("box list without a ceiling");
        }
        if let Some(why) = self
            .terrain
            .first_unusual(&self.vertex_flags, GridSurface::Terrain)
        {
            return Some(why);
        }
        if let Some(c) = &self.ceiling
            && let Some(why) = c
                .surface
                .first_unusual(&self.vertex_flags, GridSurface::Ceiling)
        {
            return Some(why);
        }
        None
    }
}

impl Record<'_> for LandblockData {
    const NAME: &'static str = LandblockData::NAME;

    /// Decodes without a stamp (fallback rules, §3.9).
    fn parse(data: &[u8]) -> Result<Self> {
        LandblockData::parse(data, None)
    }

    fn did(&self) -> Did {
        self.did
    }
}

/// §3.11 for any grid with flags; `surface` selects the diagonal bit.
pub(crate) fn triangulated_height(
    heights: &HeightGrid,
    flags: &VertexFlags,
    surface: GridSurface,
    x: f32,
    y: f32,
) -> Option<f32> {
    if x.is_nan() || y.is_nan() {
        return None;
    }
    let gx = x.clamp(0.0, LANDBLOCK_SIZE) / GRID_SPACING;
    let gy = y.clamp(0.0, LANDBLOCK_SIZE) / GRID_SPACING;
    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
    let (i, j) = (
        (gx.floor() as usize).min(QUAD_SIDE - 1),
        (gy.floor() as usize).min(QUAD_SIDE - 1),
    );
    #[allow(clippy::cast_precision_loss)]
    let (fx, fy) = (gx - i as f32, gy - j as f32);
    let h = |a: usize, b: usize| heights.height(a, b).unwrap_or(0.0);
    let (h00, h10, h01, h11) = (h(i, j), h(i + 1, j), h(i, j + 1), h(i + 1, j + 1));
    // The plane of the chosen triangle, evaluated with barycentric weights (≥ 0, sum 1) so
    // that f32 rounding cannot leave the range of the three corner heights, and a query at
    // a grid sample returns that sample exactly (a corner weight of 1, the others 0), as
    // the neighbouring landblock does at the shared edge.
    let tri = if flags.surface_split_sw_ne(surface, i, j) {
        if fx >= fy {
            [(1.0 - fx, h00), (fx - fy, h10), (fy, h11)]
        } else {
            [(1.0 - fy, h00), (fx, h11), (fy - fx, h01)]
        }
    } else if fx + fy <= 1.0 {
        [(1.0 - fx - fy, h00), (fx, h10), (fy, h01)]
    } else {
        [(fx + fy - 1.0, h11), (1.0 - fx, h01), (1.0 - fy, h10)]
    };
    let lo = tri.iter().map(|t| t.1).fold(f32::INFINITY, f32::min);
    let hi = tri.iter().map(|t| t.1).fold(f32::NEG_INFINITY, f32::max);
    let sum: f32 = tri.iter().map(|&(w, v)| w.max(0.0) * v).sum();
    Some(sum.clamp(lo, hi))
}

fn read_grid_count(r: &mut Reader<'_>, what: &'static str, expected: usize) -> Result<()> {
    let at = r.position();
    let found = r.u32()?;
    if found as usize != expected {
        return Err(FormatError::CountMismatch {
            what,
            offset: at,
            expected: expected as u32,
            found,
        });
    }
    Ok(())
}

fn read_boxes(r: &mut Reader<'_>) -> Result<Option<Vec<LandblockBox>>> {
    if !r.bool8()? {
        return Ok(None);
    }
    let count = r.u32()?;
    let mut boxes = r.alloc_vec(count, BOX_SIZE, usize::MAX)?;
    for _ in 0..count {
        let mut values = [0f32; 6];
        for v in &mut values {
            *v = r.f32()?;
        }
        r.push(&mut boxes, LandblockBox { values }, count)?;
    }
    Ok(Some(boxes))
}
