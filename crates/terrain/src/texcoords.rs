// SPDX-License-Identifier: GPL-3.0-or-later
//! Per-vertex texture-coordinate sets, the per-block texture offset, the layer texture
//! transform and the normal-map tangent frame (`terrain-texturing.md` §6, §10).
//!
//! # Coordinate sets (§6.1)
//!
//! Every grid vertex (i, j) at block-local (x, y) = (5i, 5j) with height *h* (world units,
//! `landblock.md` §3.4) carries four 2-component sets; (*o*ₓ, *o*ᵧ) is the per-block offset
//! of §6.2 ([`TextureFrame`]):
//!
//! | Set | Components | Used by |
//! |---|---|---|
//! | 0 ([`CoordinateSet::Xy`]) | (x + *o*ₓ, y + *o*ᵧ) | layers with projection 3 (XY, top-down) |
//! | 1 ([`CoordinateSet::Xz`]) | (x + *o*ₓ, *h*) | layers with projection 1 (XZ) |
//! | 2 ([`CoordinateSet::Yz`]) | (y + *o*ᵧ, *h*) | layers with projection 2 (YZ) |
//! | 3 ([`CoordinateSet::Block`]) | (i / 32, j / 32) = (x / 160, y / 160) | masks (§5.2) and the light map (§10) |
//!
//! The fifth set of the original (the landscape-shadow texture, §9.5) is not produced: the
//! feature is treated as off (Tessera Decision of §9.5).
//!
//! Layer textures (diffuse, normal, emissive, glow) use the set their projection selects,
//! transformed by [`LayerTransform`] (§6.3). Masks and light maps use set 3 unchanged,
//! with stored row 0 on the south edge, so a renderer that uploads rows in stored order
//! samples them at set 3 directly (no v flip, unlike the block UVs of
//! [`crate::TerrainMesh::uvs`]).
//!
//! # Precision
//!
//! Sets 0–2 are exact in f32: x, y are multiples of 5 and the offset is a multiple of 160
//! (plus x or y). The transform is evaluated in f64 and rounded once to f32; rotations by a
//! multiple of 90° use exact sines and cosines (Tessera's choice; the error of the generic
//! path is below 10⁻¹⁵ relative).

use tessera_formats::landblock::{
    GRID_SAMPLES, GRID_SIDE, GRID_SPACING, QUAD_SIDE, height_from_raw,
};
use tessera_formats::terrain_type::{FloatCurve, TerrainType, TextureProjection};

use crate::block::{TerrainBlock, vertex_index};
use crate::normals::VertexNormals;

/// Units per landblock side used by the per-block offset (§6.2; decision 0012).
pub const BLOCK_UNITS: f32 = 160.0;
/// A reference-landblock step of this many units or more on either axis resets the
/// accumulated shift (§6.2; decision 0012).
pub const OFFSET_JUMP_UNITS: f32 = 480.0;
/// The accumulated shift resets when either component reaches this many units (§6.2;
/// decision 0012).
pub const OFFSET_RESET_UNITS: f32 = 2048.0;
/// A tangent whose length before normalising is at most this is zero (§6.4).
pub const TANGENT_EPSILON: f64 = 1e-6;

/// One of the texture-coordinate sets of a terrain vertex (§6.1).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum CoordinateSet {
    /// Set 0: (x + *o*ₓ, y + *o*ᵧ), for projection 3 (XY).
    Xy,
    /// Set 1: (x + *o*ₓ, *h*), for projection 1 (XZ).
    Xz,
    /// Set 2: (y + *o*ᵧ, *h*), for projection 2 (YZ).
    Yz,
    /// Set 3: (x / 160, y / 160), for masks and the light map.
    Block,
}

impl CoordinateSet {
    /// The set's number, 0–3.
    #[must_use]
    pub fn index(self) -> usize {
        match self {
            Self::Xy => 0,
            Self::Xz => 1,
            Self::Yz => 2,
            Self::Block => 3,
        }
    }

    /// The set a layer with `projection` reads its layer textures from (§6.1: 1 → set 1,
    /// 2 → set 2, 3 → set 0).
    #[must_use]
    pub fn for_projection(projection: TextureProjection) -> Self {
        match projection {
            TextureProjection::Xz => Self::Xz,
            TextureProjection::Yz => Self::Yz,
            TextureProjection::Xy => Self::Xy,
        }
    }
}

/// The per-block offset with Tessera's fixed-region-coordinate simplification (§6.2,
/// Tessera Decision): anchor landblock (0, 0), offset (160 · X, 160 · Y). It differs from
/// the original's frame by whole blocks only.
#[must_use]
pub fn fixed_block_offset(x: u8, y: u8) -> [f32; 2] {
    [BLOCK_UNITS * f32::from(x), BLOCK_UNITS * f32::from(y)]
}

/// The landscape renderer's texture frame: a reference landblock and the accumulated
/// shift that together give each block's texture offset (§6.2).
///
/// For a block (X, Y) of the reference's region the offset is
/// (160 · (X − X_ref) + *a*ₓ, 160 · (Y − Y_ref) + *a*ᵧ). When the reference moves by
/// (*d*ₓ, *d*ᵧ) = 160 · (ΔX, ΔY) within one region and both |*d*| < 480, the step is
/// added to (*a*ₓ, *a*ᵧ), which then resets to (0, 0) once either component reaches
/// 2,048 in magnitude; any other move (three or more blocks, another region, no previous
/// reference) resets it. Between resets the offsets do not change as the reference moves;
/// they equal 160 · (X − X_a) for the reference at the last reset.
///
/// Which landblock the original uses as the reference is OPEN (§6.2); Tessera's decision
/// is the landblock that contains the camera ([`Self::set_reference`] each frame or on
/// block change). Blocks whose texture coordinates were built with an older offset must
/// be rebuilt (or their offset uniform updated) when [`Self::set_reference`] reports a
/// change of frame.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct TextureFrame {
    reference: Option<(u32, u8, u8)>,
    shift: [f32; 2],
}

impl TextureFrame {
    /// A frame without a reference landblock.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            reference: None,
            shift: [0.0; 2],
        }
    }

    /// The reference landblock (region, X, Y), if set.
    #[must_use]
    pub fn reference(&self) -> Option<(u32, u8, u8)> {
        self.reference
    }

    /// The accumulated shift (*a*ₓ, *a*ᵧ).
    #[must_use]
    pub fn shift(&self) -> [f32; 2] {
        self.shift
    }

    /// Makes landblock (`x`, `y`) of `region` the reference, applying the accumulator
    /// rule. Returns whether the block offsets changed (a reset of the shift; a move
    /// between resets keeps every offset).
    pub fn set_reference(&mut self, region: u32, x: u8, y: u8) -> bool {
        // The offset of the new reference block before the move; after it, the offset of
        // the reference block is the shift itself.
        let before = self.block_offset(region, x, y);
        let reset = match self.reference {
            Some((r, x0, y0)) if r == region => {
                let dx = BLOCK_UNITS * f32::from(i16::from(x) - i16::from(x0));
                let dy = BLOCK_UNITS * f32::from(i16::from(y) - i16::from(y0));
                if dx.abs() < OFFSET_JUMP_UNITS && dy.abs() < OFFSET_JUMP_UNITS {
                    self.shift[0] += dx;
                    self.shift[1] += dy;
                    self.shift[0].abs() >= OFFSET_RESET_UNITS
                        || self.shift[1].abs() >= OFFSET_RESET_UNITS
                } else {
                    true
                }
            }
            _ => true,
        };
        self.reference = Some((region, x, y));
        if reset {
            self.shift = [0.0; 2];
        }
        before != Some(self.shift)
    }

    /// The texture offset (*o*ₓ, *o*ᵧ) of landblock (`x`, `y`) of `region`; `None` without
    /// a reference or for another region (where the original returns a fixed far-away
    /// value, §6.2).
    #[must_use]
    pub fn block_offset(&self, region: u32, x: u8, y: u8) -> Option<[f32; 2]> {
        let (r, x0, y0) = self.reference?;
        if r != region {
            return None;
        }
        Some([
            BLOCK_UNITS * f32::from(i16::from(x) - i16::from(x0)) + self.shift[0],
            BLOCK_UNITS * f32::from(i16::from(y) - i16::from(y0)) + self.shift[1],
        ])
    }
}

/// Exact sine and cosine of `degrees` at multiples of 90°, f64 otherwise; non-finite
/// angles count as 0.
fn sin_cos_degrees(degrees: f32) -> (f64, f64) {
    if !degrees.is_finite() {
        return (0.0, 1.0);
    }
    let r = f64::from(degrees).rem_euclid(360.0);
    if r == 0.0 {
        (0.0, 1.0)
    } else if r == 90.0 {
        (1.0, 0.0)
    } else if r == 180.0 {
        (0.0, -1.0)
    } else if r == 270.0 {
        (-1.0, 0.0)
    } else {
        r.to_radians().sin_cos()
    }
}

/// Non-finite stored floats count as 0 (hostile data; every stored value in the install is
/// finite). Tessera Decision.
pub(crate) fn finite_or_zero(v: f32) -> f32 {
    if v.is_finite() { v } else { 0.0 }
}

/// A result as a finite `f32`: NaN becomes 0, anything beyond the `f32` range (infinities
/// included) the largest finite `f32` of its sign. Tessera Decision for hostile values.
fn finite_f32(v: f64) -> f32 {
    if v.is_nan() {
        0.0
    } else {
        v.clamp(-f64::from(f32::MAX), f64::from(f32::MAX)) as f32
    }
}

/// A layer's texture transform (§6.3): from the selected pair (p, q),
/// u = cos θ · (`u_scale` · p) − sin θ · (`v_scale` · q) + *t*ᵤ and
/// v = sin θ · (`u_scale` · p) + cos θ · (`v_scale` · q) + *t*ᵥ, with θ = `rotation`
/// degrees counter-clockwise and (*t*ᵤ, *t*ᵥ) the offset curves evaluated at the material
/// clock. The early translation (parameters 1070/1071) is 0 for terrain and omitted.
///
/// Applies to the layer textures only (diffuse, normal, emissive, glow); masks and light
/// maps use set 3 unchanged. A negative scale mirrors.
///
/// This is the rule of `material.md` §8.4 specialised to terrain layers (no early
/// translation, exact sines and cosines at multiples of 90°, `terrain.md` §7 Tessera
/// Decision (5)); `tessera_material::TextureTransform` is the general form. Like it, every
/// output is finite: a non-finite stored value counts as 0, and a non-finite result
/// element becomes 0 (NaN) or the largest finite `f32` of its sign.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct LayerTransform {
    /// `u_scale` (material parameter 1001): repeats per world unit along p.
    pub u_scale: f32,
    /// `v_scale` (parameter 1002): repeats per world unit along q.
    pub v_scale: f32,
    /// `rotation` in degrees, counter-clockwise (parameter 1033).
    pub rotation_degrees: f32,
}

impl LayerTransform {
    /// The transform of terrain type `entry`. Non-finite stored values count as 0.
    #[must_use]
    pub fn from_entry(entry: &TerrainType) -> Self {
        Self {
            u_scale: finite_or_zero(entry.u_scale),
            v_scale: finite_or_zero(entry.v_scale),
            rotation_degrees: finite_or_zero(entry.rotation),
        }
    }

    fn rows(&self) -> [[f64; 2]; 2] {
        let (s, c) = sin_cos_degrees(self.rotation_degrees);
        let (su, sv) = (
            f64::from(finite_or_zero(self.u_scale)),
            f64::from(finite_or_zero(self.v_scale)),
        );
        [[c * su, -s * sv], [s * su, c * sv]]
    }

    /// Texture coordinates (u, v) of the coordinate pair `pq` with translation
    /// `translation` = (*t*ᵤ, *t*ᵥ) (the evaluated offset curves, [`LayerOffsets`]).
    #[must_use]
    pub fn apply(&self, pq: [f32; 2], translation: [f32; 2]) -> [f32; 2] {
        let m = self.rows();
        let (p, q) = (f64::from(pq[0]), f64::from(pq[1]));
        [
            finite_f32(m[0][0] * p + m[0][1] * q + f64::from(translation[0])),
            finite_f32(m[1][0] * p + m[1][1] * q + f64::from(translation[1])),
        ]
    }

    /// The 2 × 3 affine matrix of [`Self::apply`], rows (u, v):
    /// `u = m[0][0] p + m[0][1] q + m[0][2]`, `v = m[1][0] p + m[1][1] q + m[1][2]`.
    #[must_use]
    pub fn matrix(&self, translation: [f32; 2]) -> [[f32; 3]; 2] {
        let m = self.rows();
        let t = translation.map(|c| finite_f32(f64::from(c)));
        [
            [finite_f32(m[0][0]), finite_f32(m[0][1]), t[0]],
            [finite_f32(m[1][0]), finite_f32(m[1][1]), t[1]],
        ]
    }

    /// The first two columns of the matrix, each normalised (§6.4):
    /// *c*₀ ∝ (`u_scale` cos θ, `u_scale` sin θ), *c*₁ ∝ (−`v_scale` sin θ, `v_scale` cos θ).
    /// A zero column (zero scale) stays zero.
    #[must_use]
    pub fn normal_columns(&self) -> [[f32; 2]; 2] {
        let m = self.rows();
        let unit = |a: f64, b: f64| -> [f32; 2] {
            let len = a.hypot(b);
            if len > 0.0 && len.is_finite() {
                [(a / len) as f32, (b / len) as f32]
            } else {
                [0.0, 0.0]
            }
        };
        [unit(m[0][0], m[1][0]), unit(m[0][1], m[1][1])]
    }

    /// Turns a tangent-space normal *n* = 2 · texel.rgb − 1 by the transform (§6.4):
    /// *n*′ = (*n*.x *c*₀.x + *n*.y *c*₁.x, *n*.x *c*₀.y + *n*.y *c*₁.y, *n*.z), normalised.
    /// For positive scales this rotates (*n*.x, *n*.y) by θ. A zero result stays zero.
    #[must_use]
    pub fn rotate_normal(&self, n: [f32; 3]) -> [f32; 3] {
        let [c0, c1] = self.normal_columns();
        let v = [
            f64::from(n[0]) * f64::from(c0[0]) + f64::from(n[1]) * f64::from(c1[0]),
            f64::from(n[0]) * f64::from(c0[1]) + f64::from(n[1]) * f64::from(c1[1]),
            f64::from(n[2]),
        ];
        normalise(v).unwrap_or([0.0; 3])
    }
}

/// The two texture translations of a layer (material parameters 1017 and 1018, §6.3):
/// float curves (`material.md` §5.3) evaluated at the material clock. Evaluation of
/// curves other than mode 1 belongs to `tessera-material` (`material.md` §5.3.1), which
/// the renderer calls; this crate only exposes the constant case.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct LayerOffsets {
    /// `u_offset`.
    pub u: FloatCurve,
    /// `v_offset`.
    pub v: FloatCurve,
}

impl LayerOffsets {
    /// The offsets of terrain type `entry`.
    #[must_use]
    pub fn from_entry(entry: &TerrainType) -> Self {
        Self {
            u: entry.u_offset.clone(),
            v: entry.v_offset.clone(),
        }
    }

    /// (*t*ᵤ, *t*ᵥ) when both curves are constants (mode 1; 4,660 of 4,688 entries); a
    /// non-finite constant counts as 0. `None` when one of them varies with time.
    #[must_use]
    pub fn constant(&self) -> Option<[f32; 2]> {
        Some([
            finite_or_zero(self.u.constant()?),
            finite_or_zero(self.v.constant()?),
        ])
    }
}

fn normalise(v: [f64; 3]) -> Option<[f32; 3]> {
    let len = (v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt();
    (len > 0.0 && len.is_finite()).then(|| {
        [
            (v[0] / len) as f32,
            (v[1] / len) as f32,
            (v[2] / len) as f32,
        ]
    })
}

/// Tangent **T** and binormal **B** of a vertex (§6.4).
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct TangentFrame {
    /// **T** = normalise(**E** − (**E** · **N**) **N**), **E** = east (1, 0, 0); zero when
    /// the length before normalising is at most 10⁻⁶.
    pub tangent: [f32; 3],
    /// **B** = **N** × **T** (zero with **T**).
    pub binormal: [f32; 3],
}

/// The tangent frame of unit vertex normal `n` (§6.4; Tessera axes x east, y north, z up).
/// For flat ground **T** = east and **B** = north. The same frame serves all three
/// projections. A non-finite normal gives the zero frame.
#[must_use]
pub fn tangent_frame(n: [f32; 3]) -> TangentFrame {
    let n = n.map(f64::from);
    if n.iter().any(|c| !c.is_finite()) {
        return TangentFrame::default();
    }
    let d = n[0];
    let t = [1.0 - d * n[0], -d * n[1], -d * n[2]];
    let len = (t[0] * t[0] + t[1] * t[1] + t[2] * t[2]).sqrt();
    if len <= TANGENT_EPSILON {
        return TangentFrame::default();
    }
    let t = t.map(|c| c / len);
    let b = [
        n[1] * t[2] - n[2] * t[1],
        n[2] * t[0] - n[0] * t[2],
        n[0] * t[1] - n[1] * t[0],
    ];
    TangentFrame {
        tangent: t.map(|c| c as f32),
        binormal: b.map(|c| c as f32),
    }
}

/// Engine-neutral vertex streams of one terrain or ceiling surface for the layer pipeline:
/// all 1,089 grid vertices in grid order (vertex number 33 · j + i), so the triangles of
/// [`crate::layers::DrawLayer`] (grid vertex numbers) index them directly, as do the
/// per-level lists of [`crate::triangulate`].
///
/// Positions are block-local (x, y in 0–160, z absolute). A ceiling surface
/// (§4.8) uses the ceiling heights for positions and for the height component of sets 1
/// and 2, and the ceiling normals ([`VertexNormals::ceiling`]); its triangles face down.
#[derive(Debug, Clone, PartialEq)]
pub struct LayerVertices {
    /// The per-block offset (*o*ₓ, *o*ᵧ) the sets 0–2 were built with.
    pub offset: [f32; 2],
    /// Block-local positions.
    pub positions: Vec<[f32; 3]>,
    /// Unit vertex normals.
    pub normals: Vec<[f32; 3]>,
    /// Tangents **T** (§6.4).
    pub tangents: Vec<[f32; 3]>,
    /// Binormals **B** = **N** × **T** (§6.4).
    pub binormals: Vec<[f32; 3]>,
    /// Sets 0–3 (§6.1), indexed by [`CoordinateSet::index`].
    pub sets: [Vec<[f32; 2]>; 4],
}

impl LayerVertices {
    /// Streams for the surface whose heights `heights` holds (the terrain block, or
    /// [`TerrainBlock::from_ceiling`]) with `normals` computed for it and the per-block
    /// offset `offset` (§6.2; [`TextureFrame::block_offset`] or [`fixed_block_offset`]).
    #[must_use]
    pub fn build(heights: &TerrainBlock, normals: &VertexNormals, offset: [f32; 2]) -> Self {
        let raw = heights.raw_heights();
        let all = normals.as_slice();
        let mut positions = Vec::with_capacity(GRID_SAMPLES);
        let mut out_normals = Vec::with_capacity(GRID_SAMPLES);
        let mut tangents = Vec::with_capacity(GRID_SAMPLES);
        let mut binormals = Vec::with_capacity(GRID_SAMPLES);
        let mut sets: [Vec<[f32; 2]>; 4] =
            std::array::from_fn(|_| Vec::with_capacity(GRID_SAMPLES));
        let side = QUAD_SIDE as f32;
        for j in 0..GRID_SIDE {
            for i in 0..GRID_SIDE {
                let g = vertex_index(i, j);
                let (x, y) = (i as f32 * GRID_SPACING, j as f32 * GRID_SPACING);
                let h = height_from_raw(raw[g]);
                let n = all[g];
                let frame = tangent_frame(n);
                positions.push([x, y, h]);
                out_normals.push(n);
                tangents.push(frame.tangent);
                binormals.push(frame.binormal);
                let (px, py) = (x + offset[0], y + offset[1]);
                sets[0].push([px, py]);
                sets[1].push([px, h]);
                sets[2].push([py, h]);
                sets[3].push([i as f32 / side, j as f32 / side]);
            }
        }
        Self {
            offset,
            positions,
            normals: out_normals,
            tangents,
            binormals,
            sets,
        }
    }

    /// The stream of coordinate set `set`.
    #[must_use]
    pub fn set(&self, set: CoordinateSet) -> &[[f32; 2]] {
        &self.sets[set.index()]
    }

    /// Light-map (and mask) coordinates: set 3 (§10).
    #[must_use]
    pub fn light_map_uvs(&self) -> &[[f32; 2]] {
        self.set(CoordinateSet::Block)
    }

    /// Rebuilds sets 0–2 for a new per-block offset (after a reset of the
    /// [`TextureFrame`]); positions, normals and sets 3 are unchanged. Only the vertices
    /// present in [`Self::positions`] and all three sets are rewritten (all 1,089 unless a
    /// caller resized the public streams).
    pub fn set_offset(&mut self, offset: [f32; 2]) {
        self.offset = offset;
        let [s0, s1, s2, _] = &mut self.sets;
        for (((p, a), b), c) in self
            .positions
            .iter()
            .zip(s0.iter_mut())
            .zip(s1.iter_mut())
            .zip(s2.iter_mut())
        {
            let (px, py) = (p[0] + offset[0], p[1] + offset[1]);
            *a = [px, py];
            *b = [px, p[2]];
            *c = [py, p[2]];
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::normals::Neighbours;

    #[test]
    fn worked_example_offsets_and_transforms() {
        // terrain-texturing.md §14: block (3, 7), point (12.5, 3.0).
        let o = fixed_block_offset(3, 7);
        assert_eq!(o, [480.0, 1120.0]);
        let pq = [12.5 + o[0], 3.0 + o[1]];
        assert_eq!(pq, [492.5, 1123.0]);
        let key0 = LayerTransform {
            u_scale: 0.1,
            v_scale: 0.1,
            rotation_degrees: 0.0,
        };
        assert_eq!(key0.apply(pq, [0.0, 0.0]), [49.25, 112.3]);
        let key1 = LayerTransform {
            u_scale: 0.2,
            v_scale: 0.2,
            rotation_degrees: 90.0,
        };
        assert_eq!(key1.apply(pq, [0.0, 0.0]), [-224.6, 98.5]);
        // Anchored at (2, 7): (172.5, 3.0); key 0's u changes by 32 whole repeats.
        let mut frame = TextureFrame::new();
        frame.set_reference(1, 2, 7);
        let o2 = frame.block_offset(1, 3, 7).unwrap();
        let pq2 = [12.5 + o2[0], 3.0 + o2[1]];
        assert_eq!(pq2, [172.5, 3.0]);
        let (a, b) = (key0.apply(pq, [0.0; 2]), key0.apply(pq2, [0.0; 2]));
        assert!((a[0] - b[0] - 32.0).abs() < 1e-4);
    }

    #[test]
    fn frame_accumulates_and_resets() {
        let mut f = TextureFrame::new();
        assert_eq!(f.block_offset(1, 0, 0), None);
        f.set_reference(1, 10, 10);
        let anchor = f.block_offset(1, 12, 9).unwrap();
        assert_eq!(anchor, [320.0, -160.0]);
        // Moves of up to two blocks keep every offset until the shift reaches 2,048.
        let mut steps = 0;
        loop {
            let (_, x, _) = f.reference().unwrap();
            let changed = f.set_reference(1, x + 2, 10);
            steps += 1;
            if changed {
                break;
            }
            assert_eq!(f.block_offset(1, 12, 9), Some(anchor));
        }
        // 320 per step: the shift reaches 2,048 at the 7th step (2,240) and resets.
        assert_eq!(steps, 7);
        assert_eq!(f.shift(), [0.0, 0.0]);
        assert_eq!(f.block_offset(1, 24, 10), Some([0.0, 0.0]));
        // A jump of three blocks resets; so does another region.
        f.set_reference(1, 30, 10);
        f.set_reference(1, 31, 10);
        assert_eq!(f.shift(), [160.0, 0.0]);
        f.set_reference(1, 34, 10);
        assert_eq!(f.shift(), [0.0, 0.0]);
        f.set_reference(1, 35, 9);
        assert!(f.set_reference(2, 35, 9));
        assert_eq!(f.shift(), [0.0, 0.0]);
        assert_eq!(f.block_offset(1, 35, 9), None);
        // Negative steps.
        f.set_reference(2, 5, 5);
        f.set_reference(2, 3, 3);
        assert_eq!(f.shift(), [-320.0, -320.0]);
        assert_eq!(f.block_offset(2, 5, 5), Some([0.0, 0.0]));
    }

    #[test]
    fn transform_rotation_direction_and_mirroring() {
        let t = LayerTransform {
            u_scale: 1.0,
            v_scale: 1.0,
            rotation_degrees: 90.0,
        };
        // Counter-clockwise: (1, 0) → (0, 1).
        assert_eq!(t.apply([1.0, 0.0], [0.0; 2]), [0.0, 1.0]);
        assert_eq!(t.apply([1.0, 0.0], [0.5, -0.25]), [0.5, 0.75]);
        let m = t.matrix([0.5, -0.25]);
        assert_eq!(m, [[0.0, -1.0, 0.5], [1.0, 0.0, -0.25]]);
        let n = t.rotate_normal([1.0, 0.0, 1.0]);
        let s = std::f32::consts::FRAC_1_SQRT_2;
        assert!((n[0]).abs() < 1e-7 && (n[1] - s).abs() < 1e-6 && (n[2] - s).abs() < 1e-6);
        let mirror = LayerTransform {
            u_scale: -0.5,
            v_scale: 0.25,
            rotation_degrees: 0.0,
        };
        assert_eq!(mirror.apply([2.0, 4.0], [0.0; 2]), [-1.0, 1.0]);
        assert_eq!(mirror.normal_columns(), [[-1.0, 0.0], [0.0, 1.0]]);
        assert_eq!(mirror.rotate_normal([0.6, 0.0, 0.8]), [-0.6, 0.0, 0.8]);
        let generic = LayerTransform {
            u_scale: 0.3,
            v_scale: 0.3,
            rotation_degrees: 30.0,
        };
        let uv = generic.apply([10.0, 0.0], [0.0; 2]);
        assert!((uv[0] - 3.0 * 0.866_025_4).abs() < 1e-5 && (uv[1] - 1.5).abs() < 1e-5);
        let zero = LayerTransform::default();
        assert_eq!(zero.normal_columns(), [[0.0; 2]; 2]);
        assert_eq!(zero.rotate_normal([0.0, 0.0, 0.0]), [0.0; 3]);
        let hostile = LayerTransform {
            u_scale: f32::NAN,
            v_scale: f32::INFINITY,
            rotation_degrees: f32::NAN,
        };
        assert_eq!(hostile.apply([1.0, 1.0], [0.0; 2]), [0.0, 0.0]);
        // Review finding: outputs stay finite for huge values too.
        let huge = LayerTransform {
            u_scale: 3e38,
            v_scale: -3e38,
            rotation_degrees: 0.0,
        };
        assert_eq!(huge.apply([1000.0, 0.0], [0.0; 2]), [f32::MAX, 0.0]);
        assert_eq!(huge.apply([0.0, 1000.0], [0.0; 2]), [0.0, -f32::MAX]);
        let m = huge.matrix([f32::NAN, f32::INFINITY]);
        assert!(m.iter().flatten().all(|c| c.is_finite()));
        assert_eq!((m[0][2], m[1][2]), (0.0, f32::MAX));
        assert_eq!(
            huge.apply([f32::NAN, 1.0], [f32::NEG_INFINITY, 0.0]),
            [0.0, 0.0]
        );
    }

    #[test]
    fn tangent_frames() {
        let f = tangent_frame([0.0, 0.0, 1.0]);
        assert_eq!(f.tangent, [1.0, 0.0, 0.0]);
        assert_eq!(f.binormal, [0.0, 1.0, 0.0]);
        assert_eq!(tangent_frame([1.0, 0.0, 0.0]), TangentFrame::default());
        assert_eq!(tangent_frame([-1.0, 0.0, 0.0]), TangentFrame::default());
        assert_eq!(tangent_frame([f32::NAN, 0.0, 1.0]), TangentFrame::default());
        let s = std::f32::consts::FRAC_1_SQRT_2;
        let f = tangent_frame([-s, 0.0, s]);
        assert!((f.tangent[0] - s).abs() < 1e-6 && (f.tangent[2] - s).abs() < 1e-6);
        assert!((f.binormal[1] - 1.0).abs() < 1e-6);
        // A downward normal (ceiling): T stays east, B = N × T points south.
        let f = tangent_frame([0.0, 0.0, -1.0]);
        assert_eq!(f.tangent, [1.0, 0.0, 0.0]);
        assert_eq!(f.binormal, [0.0, -1.0, 0.0]);
    }

    #[test]
    fn vertex_streams() {
        use tessera_formats::landblock::{HeightGrid, VertexFlags};
        let block = TerrainBlock::new(
            &HeightGrid::from_fn(|i, j| (1000 + 7 * i + 3 * j) as u16),
            &VertexFlags::default(),
        );
        let normals = VertexNormals::compute(&block, &Neighbours::NONE);
        let mut v = LayerVertices::build(&block, &normals, [480.0, 1120.0]);
        assert_eq!(v.positions.len(), GRID_SAMPLES);
        let k = vertex_index(4, 2);
        let h = height_from_raw(1000 + 28 + 6);
        assert_eq!(v.positions[k], [20.0, 10.0, h]);
        assert_eq!(v.set(CoordinateSet::Xy)[k], [500.0, 1130.0]);
        assert_eq!(v.set(CoordinateSet::Xz)[k], [500.0, h]);
        assert_eq!(v.set(CoordinateSet::Yz)[k], [1130.0, h]);
        assert_eq!(v.light_map_uvs()[k], [0.125, 0.0625]);
        assert_eq!(v.light_map_uvs()[GRID_SAMPLES - 1], [1.0, 1.0]);
        let n = v.normals[k];
        let t = v.tangents[k];
        let dot = n[0] * t[0] + n[1] * t[1] + n[2] * t[2];
        assert!(dot.abs() < 1e-6);
        v.set_offset([0.0, 0.0]);
        assert_eq!(v.set(CoordinateSet::Xy)[k], [20.0, 10.0]);
        assert_eq!(v.set(CoordinateSet::Yz)[k], [10.0, h]);
        assert_eq!(
            CoordinateSet::for_projection(TextureProjection::Xz),
            CoordinateSet::Xz
        );
        assert_eq!(
            CoordinateSet::for_projection(TextureProjection::Xy).index(),
            0
        );
        // Review finding: resized public streams must not make set_offset panic.
        let mut grown = v.clone();
        grown.positions.push([0.0; 3]);
        grown.set_offset([160.0, 0.0]);
        assert_eq!(grown.set(CoordinateSet::Xy)[k], [180.0, 10.0]);
        assert_eq!(grown.sets[0].len(), GRID_SAMPLES);
        let mut shrunk = v.clone();
        shrunk.sets[2].truncate(3);
        shrunk.set_offset([160.0, 0.0]);
        assert_eq!(
            shrunk.set(CoordinateSet::Yz)[2],
            [0.0, height_from_raw(1000 + 14)]
        );
        assert_eq!(shrunk.sets[2].len(), 3);
    }
}
