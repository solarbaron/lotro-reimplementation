// SPDX-License-Identifier: GPL-3.0-or-later
//! Surface layers resolved against the region's terrain type table: draw records for the
//! layer pipeline (`terrain-texturing.md` §4, §7, §8, §8.1, §8.2).
//!
//! # Rules (from the spec)
//!
//! - A layer's `material` is the `key` of an entry of the region's terrain type table
//!   (prefix 0x19, region-descriptor slot 8; §4.1). Entries are looked up by key.
//! - The stored order is the draw order, bottom layer first; it is not re-sorted (§4.2).
//! - A layer with `whole` = 0 and no triangles is skipped and does not count as drawn
//!   (§4.3). A `whole` layer covers all 2,048 grid triangles, the level-0 triangulation of
//!   its surface (bit-7 diagonal, counter-clockwise for the terrain; bit-0 diagonal,
//!   clockwise for the ceiling), ignoring its stored window.
//! - The **first drawn** layer has no opacity input and no alpha test: it is opaque.
//!   Every later layer is blended "over" (source α, destination 1 − α) after the discard
//!   test, with α from the opacity rule of §7 ([`OpacityRule`], [`SplatInputs`]).
//! - Grid triangles covered by no layer are not drawn (holes, §4.5;
//!   [`ChunkLayers::uncovered`]). Collision keeps them (`collision.md` §6.5).
//! - The layer's features (overlay, opacity, normal map, gloss from normal, specular,
//!   baked light, emissive/glow) follow §8.1 ([`LayerFeatures`]); the material parameters
//!   of §8.2 are [`LayerParams`].
//! - The quad grid (§4.6) holds the dominant visible type per quad and is not used for
//!   drawing ([`quad_terrain_type`]); the 17 × 17 coarse grid is the scenery word and plays
//!   no part in texturing (§4.7): resolution never reads it.
//!
//! # Tessera Decisions
//!
//! - A layer whose key is not in the table is skipped and logged (one warning per chunk),
//!   the rest of the chunk is drawn (§8, T38: the original stops the chunk). The first
//!   layer that is actually drawn is the opaque one.
//! - An entry without a diffuse texture (`diffuse` = 0, 15 entries) is still drawn
//!   ([`LayerTextures::diffuse`] is `None`); the renderer binds the terrain shader records'
//!   default for parameter 3000, or plain white (§8).
//! - The client's normal-map setting is on and a texture stage for the light map is always
//!   free (§8.1); [`ResolveOptions::normal_maps`] can turn normal maps off.
//! - A non-A8 mask is dropped with a logged warning and the layer drawn as if it had none
//!   (§5.1; [`DrawLayer::check_mask`]).
//! - Stored triangles that are not level-0 grid triangles wound as their surface is, or
//!   that repeat within a layer, are not drawn (hostile data; none in the install), so
//!   what is drawn always equals the coverage, the holes and the CPU reference.
//! - Non-finite stored floats count as 0 (hostile data; none in the install).
//! - Level-of-detail meshes ([`ChunkLayers::layer_indices_for`]): see that method.

use std::cmp::Ordering;
use std::sync::Arc;

use tessera_formats::landblock::{
    GRID_SIDE, GRID_SPACING, GRID_TRIANGLE_COUNT, GridSurface, LandblockData, LayerKind, LightMaps,
    QUAD_COUNT, QUAD_SIDE, SurfaceChunk, VertexFlags, grid_triangle_key, surface_quad_triangles,
};
use tessera_formats::terrain_type::{Argb, TerrainType, TerrainTypeTable, TextureProjection};
use tessera_formats::{Did, Surface};

use crate::error::{Result, TerrainError};
use crate::mask::AlphaMask;
use crate::splat::splat_layers;
use crate::texcoords::{CoordinateSet, LayerOffsets, LayerTransform, finite_or_zero};

/// Threshold above which `alpha_test_threshold` turns the alpha test on for later layers
/// (§7, §8.2; decision 0012).
pub const ALPHA_TEST_MIN: f32 = 0.0002;

/// A set of grid triangles of one surface, by collision-tree key 0–2,047
/// (`collision.md` §6.2: 2 · (32 · j + i) for the triangle west of quad (i, j)'s diagonal,
/// plus 1 for the other).
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub struct TriangleSet([u64; GRID_TRIANGLE_COUNT / 64]);

impl Default for TriangleSet {
    fn default() -> Self {
        Self::EMPTY
    }
}

impl std::fmt::Debug for TriangleSet {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "TriangleSet({} of 2048)", self.len())
    }
}

impl TriangleSet {
    /// No triangles.
    pub const EMPTY: Self = Self([0; GRID_TRIANGLE_COUNT / 64]);
    /// All 2,048 triangles.
    pub const ALL: Self = Self([u64::MAX; GRID_TRIANGLE_COUNT / 64]);

    /// Whether triangle `key` is in the set (`false` for a key ≥ 2,048).
    #[must_use]
    pub fn contains(&self, key: u16) -> bool {
        let k = usize::from(key);
        k < GRID_TRIANGLE_COUNT && self.0[k / 64] & (1 << (k % 64)) != 0
    }

    /// Adds triangle `key` (ignored when ≥ 2,048).
    pub fn insert(&mut self, key: u16) {
        let k = usize::from(key);
        if k < GRID_TRIANGLE_COUNT {
            self.0[k / 64] |= 1 << (k % 64);
        }
    }

    /// Number of triangles in the set.
    #[must_use]
    pub fn len(&self) -> usize {
        self.0.iter().map(|w| w.count_ones() as usize).sum()
    }

    /// Whether the set is empty.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.0.iter().all(|&w| w == 0)
    }

    /// The union of two sets.
    #[must_use]
    pub fn union(&self, other: &Self) -> Self {
        Self(std::array::from_fn(|i| self.0[i] | other.0[i]))
    }

    /// The triangles not in the set.
    #[must_use]
    pub fn complement(&self) -> Self {
        Self(self.0.map(|w| !w))
    }

    /// Whether either triangle of quad (i, j), 0 ≤ i, j ≤ 31, is in the set.
    #[must_use]
    pub fn touches_quad(&self, i: usize, j: usize) -> bool {
        i < QUAD_SIDE && j < QUAD_SIDE && {
            let q = QUAD_SIDE * j + i;
            self.0[q / 32] >> (2 * (q % 32)) & 0b11 != 0
        }
    }

    /// The keys in the set, ascending.
    pub fn iter(&self) -> impl Iterator<Item = u16> + '_ {
        (0..GRID_TRIANGLE_COUNT as u16).filter(|&k| self.contains(k))
    }
}

/// The diagonals of one surface's 32 × 32 quads (bit 7 of the vertex flags for the
/// terrain, bit 0 for the ceiling, `landblock.md` §3.6).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Diagonals([u64; QUAD_COUNT / 64]);

impl Diagonals {
    fn new(flags: &VertexFlags, surface: GridSurface) -> Self {
        let mut d = [0u64; QUAD_COUNT / 64];
        for j in 0..QUAD_SIDE {
            for i in 0..QUAD_SIDE {
                if flags.surface_split_sw_ne(surface, i, j) {
                    let q = QUAD_SIDE * j + i;
                    d[q / 64] |= 1 << (q % 64);
                }
            }
        }
        Self(d)
    }

    /// Whether quad (i, j) (in range) is split south-west to north-east.
    fn sw_ne(&self, i: usize, j: usize) -> bool {
        let q = QUAD_SIDE * j + i;
        self.0[q / 64] & (1 << (q % 64)) != 0
    }

    /// The key of the grid triangle with exactly these three vertex numbers (any order,
    /// any winding), or `None` when they are not one triangle of the surface's level-0
    /// triangulation.
    fn key_of(&self, tri: [u16; 3]) -> Option<u16> {
        let xy = tri.map(|v| (usize::from(v) % GRID_SIDE, usize::from(v) / GRID_SIDE));
        if tri.iter().any(|&v| usize::from(v) >= GRID_SIDE * GRID_SIDE) {
            return None;
        }
        let i = xy.iter().map(|p| p.0).min()?;
        let j = xy.iter().map(|p| p.1).min()?;
        if i >= QUAD_SIDE || j >= QUAD_SIDE {
            return None;
        }
        let mut corners = 0u8;
        for (x, y) in xy {
            let (dx, dy) = (x - i, y - j);
            if dx > 1 || dy > 1 {
                return None;
            }
            corners |= 1 << (dx + 2 * dy); // SW = 1, SE = 2, NW = 4, NE = 8
        }
        let west = match (self.sw_ne(i, j), corners) {
            (true, 0b1101) | (false, 0b0111) => true,
            (true, 0b1011) | (false, 0b1110) => false,
            _ => return None,
        };
        grid_triangle_key(i, j, west)
    }

    /// [`Self::key_of`] for a stored layer triangle, which must also be wound as its
    /// surface is: counter-clockwise from above for the terrain, clockwise for the ceiling
    /// (`landblock.md` §3.5, `terrain-texturing.md` §4.8). A wrongly wound triangle would be
    /// back-face culled, so it covers nothing.
    fn key_of_wound(&self, tri: [u16; 3], surface: GridSurface) -> Option<u16> {
        let key = self.key_of(tri)?;
        (winding(tri) == surface_winding(surface)).then_some(key)
    }

    /// The key of the grid triangle that contains block-local point (x, y), clamped to
    /// the block; `None` for a non-finite coordinate. Points on the diagonal belong to the
    /// west triangle.
    fn key_at(&self, x: f32, y: f32) -> Option<u16> {
        if !x.is_finite() || !y.is_finite() {
            return None;
        }
        let cell = |p: f32| -> (usize, f64) {
            let g = (f64::from(p) / f64::from(GRID_SPACING)).clamp(0.0, QUAD_SIDE as f64);
            let i = (g.floor() as usize).min(QUAD_SIDE - 1);
            (i, g - i as f64)
        };
        let (i, fx) = cell(x);
        let (j, fy) = cell(y);
        let west = if self.sw_ne(i, j) {
            fy >= fx
        } else {
            fx + fy <= 1.0
        };
        grid_triangle_key(i, j, west)
    }

    /// The full level-0 triangulation of the surface in key order, wound as the surface is.
    fn full_grid(&self, surface: GridSurface) -> Vec<[u16; 3]> {
        let mut out = Vec::with_capacity(GRID_TRIANGLE_COUNT);
        let v = |(x, y): (usize, usize)| (GRID_SIDE * y + x) as u16;
        for j in 0..QUAD_SIDE {
            for i in 0..QUAD_SIDE {
                let split = self.sw_ne(i, j);
                let tris = surface_quad_triangles(surface, i, j, split);
                // quad_triangles lists the west triangle second for the SW-NE diagonal,
                // first for the SE-NW diagonal; keys are west = even.
                let (west, east) = if split {
                    (tris[1], tris[0])
                } else {
                    (tris[0], tris[1])
                };
                out.push(west.map(v));
                out.push(east.map(v));
            }
        }
        out
    }
}

/// How a layer's per-pixel opacity α is formed (§7). *m* is the mask value at the pixel,
/// *a* the diffuse texture's alpha at the layer's (u, v).
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum OpacityRule {
    /// The first drawn layer: no opacity input, α = 1, no alpha test.
    Opaque,
    /// A later layer with a mask and `use_opacity_mask`:
    /// α = clamp(*a* · *m* + *m*² · `alpha_blend_max`, 0, 1).
    MaskedDiffuse {
        /// `alpha_blend_max` (material parameter 1203).
        alpha_blend_max: f32,
    },
    /// A later layer with a mask, without `use_opacity_mask`: α = *m*.
    Mask,
    /// A later layer without a mask, with `use_opacity_mask`: α = *a*.
    Diffuse,
    /// A later layer without a mask or `use_opacity_mask`: α = 1 (still blended and
    /// alpha-tested).
    One,
}

impl OpacityRule {
    /// The rule for a layer (§7).
    #[must_use]
    pub fn select(first_drawn: bool, has_mask: bool, entry: &TerrainType) -> Self {
        match (first_drawn, has_mask, entry.use_opacity_mask) {
            (true, _, _) => Self::Opaque,
            (false, true, true) => Self::MaskedDiffuse {
                alpha_blend_max: finite_or_zero(entry.alpha_blend_max),
            },
            (false, true, false) => Self::Mask,
            (false, false, true) => Self::Diffuse,
            (false, false, false) => Self::One,
        }
    }

    /// α for mask value `m` and diffuse alpha `a` (both clamped to 0–1, NaN as 0).
    #[must_use]
    pub fn opacity(&self, m: f32, a: f32) -> f32 {
        let unit = |v: f32| if v.is_nan() { 0.0 } else { v.clamp(0.0, 1.0) };
        let (m, a) = (unit(m), unit(a));
        match *self {
            Self::Opaque | Self::One => 1.0,
            Self::MaskedDiffuse { alpha_blend_max } => {
                unit(a.mul_add(m, m * m * finite_or_zero(alpha_blend_max)))
            }
            Self::Mask => m,
            Self::Diffuse => a,
        }
    }

    /// Whether the rule reads the mask.
    #[must_use]
    pub fn uses_mask(&self) -> bool {
        matches!(self, Self::MaskedDiffuse { .. } | Self::Mask)
    }

    /// Whether the rule reads the diffuse alpha.
    #[must_use]
    pub fn uses_diffuse_alpha(&self) -> bool {
        matches!(self, Self::MaskedDiffuse { .. } | Self::Diffuse)
    }
}

/// The splat inputs of one layer: opacity rule and alpha test (§7, §8.2). The blend
/// state is the same for every layer: source α, destination 1 − α ("over"), depth write
/// on; the first drawn layer is opaque because its α is 1.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SplatInputs {
    /// Whether this is the first drawn layer of its chunk.
    pub first_drawn: bool,
    /// The opacity rule.
    pub opacity: OpacityRule,
    /// The discard threshold τ: pixels with α ≤ τ are dropped. `alpha_test_threshold`
    /// for later layers whose threshold exceeds [`ALPHA_TEST_MIN`], else 0 (only fully
    /// transparent pixels are dropped).
    pub alpha_test_ref: f32,
}

impl SplatInputs {
    /// The inputs of a layer (§7, §8.2).
    #[must_use]
    pub fn select(first_drawn: bool, has_mask: bool, entry: &TerrainType) -> Self {
        let threshold = finite_or_zero(entry.alpha_test_threshold);
        let alpha_test = !first_drawn && threshold > ALPHA_TEST_MIN;
        Self {
            first_drawn,
            opacity: OpacityRule::select(first_drawn, has_mask, entry),
            alpha_test_ref: if alpha_test { threshold } else { 0.0 },
        }
    }

    /// Whether the alpha test is on (material parameter 4004).
    #[must_use]
    pub fn alpha_test(&self) -> bool {
        self.alpha_test_ref > 0.0
    }

    /// The output α of a pixel with mask value `m` and diffuse alpha `a`, or `None` when
    /// the pixel is discarded (α ≤ τ). The first drawn layer never discards.
    #[must_use]
    pub fn alpha(&self, m: f32, a: f32) -> Option<f32> {
        let alpha = self.opacity.opacity(m, a);
        (self.first_drawn || alpha > self.alpha_test_ref).then_some(alpha)
    }

    /// Blends `src` over `dst` for a pixel with mask value `m` and diffuse alpha `a`
    /// (§8 step 4): `src` · α + `dst` · (1 − α), or `dst` when the pixel is discarded.
    #[must_use]
    pub fn blend(&self, dst: [f32; 3], src: [f32; 3], m: f32, a: f32) -> [f32; 3] {
        match self.alpha(m, a) {
            Some(alpha) => std::array::from_fn(|k| src[k] * alpha + dst[k] * (1.0 - alpha)),
            None => dst,
        }
    }
}

/// What the colour texture's alpha channel means for a layer (§8.1).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ColourAlphaRole {
    /// Unused (`ColorTexSRT` records).
    Unused,
    /// Opacity (opacity switch on; `ColorAndOpacityTexSRT`).
    Opacity,
    /// Gloss (specular on without gloss from normal; `ColorAndGlossTexSRT`).
    Gloss,
}

/// Availability of a layer's optional textures, as the renderer knows it (§8.1: "names a
/// texture, it loads").
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct Availability {
    /// The normal texture loads and normal maps are on.
    pub normal: bool,
    /// The emissive or the glow texture loads.
    pub emissive_glow: bool,
    /// The block has a light map for this surface (and a stage is free for it).
    pub light_map: bool,
}

/// The seven feature switches of a layer (§8.1), from which the original picks one of its
/// 192 terrain shader records; Tessera writes its own shaders from the switches.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct LayerFeatures {
    /// Not the first drawn layer, and names a mask (`Overlay_` records, parameter 3020).
    pub overlay: bool,
    /// Not the first drawn layer, and `use_opacity_mask`.
    pub opacity: bool,
    /// The normal texture is used.
    pub normal_map: bool,
    /// Gloss from the normal texture's alpha: `use_specular`, `use_gloss_specular`,
    /// `specular_from_normal` and the normal-map switch.
    pub gloss_from_normal: bool,
    /// Specular: `use_specular`, `use_gloss_specular`, and gloss from normal on or the
    /// opacity switch off.
    pub specular: bool,
    /// The light map is added (§10).
    pub baked_light: bool,
    /// Emissive and glow terms (§9.5).
    pub emissive_glow: bool,
}

impl LayerFeatures {
    /// The switches of a layer (§8.1).
    #[must_use]
    pub fn select(
        entry: &TerrainType,
        first_drawn: bool,
        has_mask: bool,
        available: Availability,
    ) -> Self {
        let overlay = !first_drawn && has_mask;
        let opacity = !first_drawn && entry.use_opacity_mask;
        let normal_map = available.normal;
        let both = entry.use_specular && entry.use_gloss_specular;
        let gloss_from_normal = both && entry.specular_from_normal && normal_map;
        let specular = both && (gloss_from_normal || !opacity);
        Self {
            overlay,
            opacity,
            normal_map,
            gloss_from_normal,
            specular,
            baked_light: available.light_map,
            emissive_glow: available.emissive_glow,
        }
    }

    /// The role of the colour texture's alpha (§8.1).
    #[must_use]
    pub fn colour_alpha(&self) -> ColourAlphaRole {
        if self.opacity {
            ColourAlphaRole::Opacity
        } else if self.specular && !self.gloss_from_normal {
            ColourAlphaRole::Gloss
        } else {
            ColourAlphaRole::Unused
        }
    }
}

/// The textures a layer binds, by DID (§8.2). A `None` is not bound.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct LayerTextures {
    /// 3000 DiffuseMap: the entry's `diffuse` 0x40 list. `None` for an entry without one;
    /// the layer is still drawn, with the record default or white (§8, Tessera Decision).
    pub diffuse: Option<Did>,
    /// 3011 NormalMap (normal-map switch on).
    pub normal: Option<Did>,
    /// 3018 EmissiveMap (emissive/glow switch on and named).
    pub emissive: Option<Did>,
    /// 3013 GlowMap (emissive/glow switch on and named).
    pub glow: Option<Did>,
    /// 3020 OpacityAlphaMap: the layer's 0x41 A8 mask (overlay records only; sampled at
    /// coordinate set 3, clamp, bilinear; §5.2).
    pub mask: Option<Did>,
    /// 3019 BakedLightingMap: the block's 0x41 DXT1 light map for this surface (baked-light
    /// switch on; set 3, clamp, bilinear; §10).
    pub light_map: Option<Did>,
}

/// The material parameters the original sets for a layer (§8.2), as values.
#[derive(Debug, Clone, PartialEq)]
pub struct LayerParams {
    /// 2000 DiffuseColor: `diffuse_scale` with alpha 1 (linear 0–1 channels, byte / 255).
    pub diffuse_colour: [f32; 4],
    /// 2001 SpecularColor: `specular_colour` when `use_specular`, else white.
    pub specular_colour: [f32; 4],
    /// 1000 SpecularPower: `specular_power` when `use_specular`, else 0.
    pub specular_power: f32,
    /// 2014 GlowColor: `glow_colour` when the emissive/glow switch is on, else
    /// (0, 0, 0, 1).
    pub glow_colour: [f32; 4],
    /// 4004 AlphaTest.
    pub alpha_test: bool,
    /// 1043 AlphaTestRef: τ when the alpha test is on, else 0.
    pub alpha_test_ref: f32,
    /// 1203 `c_AlphaBlendMax`: `alpha_blend_max` for layers that are not the first drawn,
    /// else 0.
    pub alpha_blend_max: f32,
    /// 1001, 1002, 1033: scale and rotation (constants).
    pub transform: LayerTransform,
    /// 1017, 1018: the offset curves.
    pub offsets: LayerOffsets,
}

fn colour(c: Argb) -> [f32; 4] {
    c.to_rgba_f32()
}

/// Options of layer resolution.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ResolveOptions {
    /// The normal-map setting (§8.1); on by default (Tessera Decision).
    pub normal_maps: bool,
}

impl Default for ResolveOptions {
    fn default() -> Self {
        Self { normal_maps: true }
    }
}

/// One drawn layer of a chunk: everything the layer pipeline needs to draw it.
///
/// The fields are read through accessors so that the triangles and their coverage, and the
/// draw index and the chunk's layer list, cannot drift apart. The renderer changes a layer
/// only through [`Self::drop_mask`], [`Self::check_mask`] and [`Self::set_available`], and
/// skips one through [`ChunkLayers::skip_layer`].
#[derive(Debug, Clone, PartialEq)]
pub struct DrawLayer {
    ordinal: usize,
    draw_index: usize,
    key: u32,
    whole: bool,
    mask: Option<Did>,
    light_map: Option<Did>,
    entry: TerrainType,
    available: Availability,
    /// Shared by all `whole` layers of a chunk (one full-grid list per chunk).
    triangles: Arc<[[u16; 3]]>,
    coverage: TriangleSet,
}

impl DrawLayer {
    /// Position of the layer in the chunk's stored list.
    #[must_use]
    pub fn ordinal(&self) -> usize {
        self.ordinal
    }

    /// Position among the drawn layers: the draw order, 0 = bottom (opaque).
    #[must_use]
    pub fn draw_index(&self) -> usize {
        self.draw_index
    }

    /// The terrain type key (the layer's `material`).
    #[must_use]
    pub fn key(&self) -> u32 {
        self.key
    }

    /// The stored `whole` flag.
    #[must_use]
    pub fn whole(&self) -> bool {
        self.whole
    }

    /// The layer's mask DID (0x41 A8), `None` when the stored DID is 0 or the mask was
    /// dropped ([`Self::drop_mask`], [`Self::check_mask`]).
    #[must_use]
    pub fn mask(&self) -> Option<Did> {
        self.mask
    }

    /// The surface's light map, if the block has one.
    #[must_use]
    pub fn light_map(&self) -> Option<Did> {
        self.light_map
    }

    /// The resolved terrain type entry.
    #[must_use]
    pub fn entry(&self) -> &TerrainType {
        &self.entry
    }

    /// Texture availability used for the feature switches.
    #[must_use]
    pub fn available(&self) -> Availability {
        self.available
    }

    /// The triangles to draw, as grid vertex numbers (33 · j + i) in the surface's winding
    /// (counter-clockwise from above for the terrain, clockwise for the ceiling): the
    /// stored window, or the full grid in key order for a `whole` layer. Every triangle is
    /// a level-0 grid triangle of [`Self::coverage`], each at most once: stored triangles
    /// off the grid, wrongly wound or repeated are left out (hostile data,
    /// [`ChunkLayers::rejected_triangles`]).
    #[must_use]
    pub fn triangles(&self) -> &[[u16; 3]] {
        &self.triangles
    }

    /// Whether this is the first drawn (opaque) layer of its chunk.
    #[must_use]
    pub fn first_drawn(&self) -> bool {
        self.draw_index == 0
    }

    /// The entry's draw-order value (`DepthIndex`, §4.2), for reference: the stored order
    /// decides.
    #[must_use]
    pub fn draw_order(&self) -> u32 {
        self.entry.draw_order
    }

    /// The projection plane of the layer textures (§6.1).
    #[must_use]
    pub fn projection(&self) -> TextureProjection {
        self.entry.projection
    }

    /// The vertex coordinate set the layer textures read (§6.1).
    #[must_use]
    pub fn coordinate_set(&self) -> CoordinateSet {
        CoordinateSet::for_projection(self.entry.projection)
    }

    /// The grid triangles the layer covers (exact, by key): the triangles of
    /// [`Self::triangles`].
    #[must_use]
    pub fn coverage(&self) -> &TriangleSet {
        &self.coverage
    }

    /// The feature switches (§8.1).
    #[must_use]
    pub fn features(&self) -> LayerFeatures {
        LayerFeatures::select(
            &self.entry,
            self.first_drawn(),
            self.mask.is_some(),
            self.available,
        )
    }

    /// The opacity rule and alpha test (§7).
    #[must_use]
    pub fn splat(&self) -> SplatInputs {
        SplatInputs::select(self.first_drawn(), self.mask.is_some(), &self.entry)
    }

    /// The textures to bind (§8.2).
    #[must_use]
    pub fn textures(&self) -> LayerTextures {
        let f = self.features();
        let some = |d: Did| (!d.is_null()).then_some(d);
        LayerTextures {
            diffuse: some(self.entry.diffuse),
            normal: f.normal_map.then(|| some(self.entry.normal)).flatten(),
            emissive: f.emissive_glow.then(|| some(self.entry.emissive)).flatten(),
            glow: f.emissive_glow.then(|| some(self.entry.glow)).flatten(),
            mask: if f.overlay { self.mask } else { None },
            light_map: if f.baked_light { self.light_map } else { None },
        }
    }

    /// The material parameters (§8.2).
    #[must_use]
    pub fn params(&self) -> LayerParams {
        let e = &self.entry;
        let f = self.features();
        let s = self.splat();
        let mut diffuse = colour(e.diffuse_scale);
        diffuse[3] = 1.0;
        LayerParams {
            diffuse_colour: diffuse,
            specular_colour: if e.use_specular {
                colour(e.specular_colour)
            } else {
                [1.0; 4]
            },
            specular_power: if e.use_specular {
                finite_or_zero(e.specular_power)
            } else {
                0.0
            },
            glow_colour: if f.emissive_glow {
                colour(e.glow_colour)
            } else {
                [0.0, 0.0, 0.0, 1.0]
            },
            alpha_test: s.alpha_test(),
            alpha_test_ref: s.alpha_test_ref,
            alpha_blend_max: if self.first_drawn() {
                0.0
            } else {
                finite_or_zero(e.alpha_blend_max)
            },
            transform: LayerTransform::from_entry(e),
            offsets: LayerOffsets::from_entry(e),
        }
    }

    /// Drops the mask (a mask surface that is not A8, §5.1 Tessera Decision): the layer is
    /// then drawn as if it had no mask (opacity from the diffuse alpha or 1).
    ///
    /// This does not log; §5.1 asks for a logged warning, which [`Self::check_mask`] emits.
    /// A caller that drops a mask for another reason logs it itself.
    pub fn drop_mask(&mut self) {
        self.mask = None;
    }

    /// Checks the decoded surface of this layer's mask ([`AlphaMask::check`], §5.1). When
    /// it is not usable (not A8, or an unusable size), logs a warning naming the DID and
    /// the reason, drops the mask ([`Self::drop_mask`]) and returns the error; the layer is
    /// then drawn as if it had no mask. Nothing happens for a layer without a mask.
    ///
    /// # Errors
    ///
    /// [`TerrainError::MaskFormat`] or [`TerrainError::MaskSize`], after the mask was
    /// dropped.
    pub fn check_mask(&mut self, surface: &Surface<'_>) -> Result<()> {
        if self.mask.is_none() {
            return Ok(());
        }
        AlphaMask::check(surface).inspect_err(|err| {
            tracing::warn!(
                mask = %surface.did,
                ordinal = self.ordinal,
                key = self.key,
                %err,
                "terrain layer mask rejected; layer drawn without a mask"
            );
            self.drop_mask();
        })
    }

    /// Sets the texture availability the feature switches use (§8.1: a texture counts
    /// only when it loads), for example when the normal or emissive texture fails to load.
    pub fn set_available(&mut self, available: Availability) {
        self.available = available;
    }
}

/// A skipped layer that names a key the table does not have (§8, T38).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct UnresolvedLayer {
    /// Position in the stored list.
    pub ordinal: usize,
    /// The missing key.
    pub key: u32,
}

/// The resolved layers of one surface chunk (terrain or ceiling) of one landblock.
///
/// Read through accessors; [`Self::skip_layer`] removes a layer and renumbers the rest, so
/// the first remaining layer is the opaque one.
#[derive(Debug, Clone, PartialEq)]
pub struct ChunkLayers {
    surface: GridSurface,
    layers: Vec<DrawLayer>,
    empty: Vec<usize>,
    unresolved: Vec<UnresolvedLayer>,
    rejected_triangles: usize,
    diagonals: Diagonals,
}

impl ChunkLayers {
    /// Which surface the chunk belongs to. Ceiling triangles face down (clockwise from
    /// above).
    #[must_use]
    pub fn surface(&self) -> GridSurface {
        self.surface
    }

    /// The drawn layers in draw order.
    #[must_use]
    pub fn layers(&self) -> &[DrawLayer] {
        &self.layers
    }

    /// Mutable access to one drawn layer (its mask and availability, [`DrawLayer`]).
    #[must_use]
    pub fn layer_mut(&mut self, index: usize) -> Option<&mut DrawLayer> {
        self.layers.get_mut(index)
    }

    /// Stored positions of the layers skipped for having no geometry (§4.3).
    #[must_use]
    pub fn empty(&self) -> &[usize] {
        &self.empty
    }

    /// Layers skipped because their key is not in the table (Tessera Decision, §8).
    #[must_use]
    pub fn unresolved(&self) -> &[UnresolvedLayer] {
        &self.unresolved
    }

    /// Stored layer triangles left out of [`DrawLayer::triangles`] because they are not a
    /// triangle of the surface's level-0 grid, are wound against the surface, or repeat a
    /// triangle of the same layer (hostile data; none in the install, `terrain.md` TR3,
    /// TR11). Leaving them out keeps what is drawn equal to the coverage, the holes and
    /// [`Self::composite`].
    #[must_use]
    pub fn rejected_triangles(&self) -> usize {
        self.rejected_triangles
    }

    /// Skips drawn layer `index` (for example one whose textures cannot be set up, §8
    /// Tessera Decision "skip only the offending layer") and renumbers the layers above
    /// it, so the first remaining layer is the opaque one. Its triangles become holes
    /// unless another layer covers them. Returns the removed layer, `None` for an index
    /// out of range.
    pub fn skip_layer(&mut self, index: usize) -> Option<DrawLayer> {
        if index >= self.layers.len() {
            return None;
        }
        let removed = self.layers.remove(index);
        for (k, l) in self.layers.iter_mut().enumerate().skip(index) {
            l.draw_index = k;
        }
        Some(removed)
    }

    /// Whether no layer is drawn (the original draws nothing for such a chunk).
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.layers.is_empty()
    }

    /// The grid triangles some drawn layer covers.
    #[must_use]
    pub fn covered(&self) -> TriangleSet {
        self.layers
            .iter()
            .fold(TriangleSet::EMPTY, |acc, l| acc.union(&l.coverage))
    }

    /// The grid triangles no drawn layer covers: they are not drawn (§4.5). For a chunk
    /// without layers this is every triangle.
    #[must_use]
    pub fn uncovered(&self) -> TriangleSet {
        self.covered().complement()
    }

    /// The key of the grid triangle that contains block-local (x, y) on this surface
    /// (clamped to the block; points on a diagonal belong to the west triangle).
    #[must_use]
    pub fn triangle_at(&self, x: f32, y: f32) -> Option<u16> {
        self.diagonals.key_at(x, y)
    }

    /// The drawn layers that contain grid triangle `key`, bottom first.
    pub fn layers_at(&self, key: u16) -> impl Iterator<Item = &DrawLayer> + '_ {
        self.layers.iter().filter(move |l| l.coverage.contains(key))
    }

    /// CPU reference of the layer stack at block-local (x, y) (§8): the repeated "over"
    /// of the layers that contain the point's grid triangle, bottom first, each with the
    /// colour, mask value and diffuse alpha `sample` gives for it. `None` in a hole.
    ///
    /// `sample(layer)` returns (colour, *m*, *a*); the colour is the layer's final colour
    /// before blending (§9).
    #[must_use]
    pub fn composite(
        &self,
        x: f32,
        y: f32,
        mut sample: impl FnMut(&DrawLayer) -> ([f32; 3], f32, f32),
    ) -> Option<[f32; 3]> {
        let key = self.triangle_at(x, y)?;
        let mut out: Option<[f32; 3]> = None;
        for layer in self.layers_at(key) {
            let (c, m, a) = sample(layer);
            let s = layer.splat();
            out = Some(s.blend(out.unwrap_or([0.0; 3]), c, m, a));
        }
        out
    }

    /// The triangles of layer `index` (a position in [`Self::layers`]) for another
    /// triangulation of the same surface, for example a level-of-detail list from
    /// [`crate::triangulate`] (grid vertex numbers, three per triangle). **Tessera
    /// Decision** (the original's distance bands are OPEN, `terrain-texturing.md` §12):
    ///
    /// - a triangle that is a level-0 grid triangle is kept exactly when the layer covers
    ///   that triangle (so the unstitched level-0 list gives the stored triangles back);
    /// - any other triangle (a coarser or stitched triangle) is kept when the layer covers
    ///   a grid triangle of any quad inside the triangle's bounding box. Masks and
    ///   blending then decide what shows; lower layers may cover a little more than in the
    ///   original, where later layers hide it.
    ///
    /// The kept triangles keep their order and are written in the **surface's** winding:
    /// counter-clockwise from above for the terrain, clockwise (facing down) for the
    /// ceiling, reversing the corner order of an input triangle wound the other way (the
    /// lists of [`crate::triangulate`] are always counter-clockwise). Degenerate triangles
    /// keep their order. Empty for an index out of range.
    ///
    /// Ceilings are drawn at level 0 from [`DrawLayer::triangles`] only (`terrain.md` §7,
    /// Tessera Decision (3)); for a ceiling this method serves checks, and its level-0
    /// list over [`crate::TerrainBlock::from_ceiling`] equals the stored triangles.
    #[must_use]
    pub fn layer_indices_for(&self, index: usize, triangulation: &[u16]) -> Vec<u16> {
        let Some(layer) = self.layers.get(index) else {
            return Vec::new();
        };
        let mut out = Vec::new();
        for tri in triangulation.as_chunks::<3>().0 {
            if tri.iter().any(|&v| usize::from(v) >= GRID_SIDE * GRID_SIDE) {
                continue;
            }
            let keep = match self.diagonals.key_of(*tri) {
                Some(key) => layer.coverage.contains(key),
                None => {
                    let xs = tri.map(|v| usize::from(v) % GRID_SIDE);
                    let ys = tri.map(|v| usize::from(v) / GRID_SIDE);
                    let (i0, i1) = (min3(xs), max3(xs).min(QUAD_SIDE));
                    let (j0, j1) = (min3(ys), max3(ys).min(QUAD_SIDE));
                    (j0..j1.max(j0 + 1).min(QUAD_SIDE)).any(|j| {
                        (i0..i1.max(i0 + 1).min(QUAD_SIDE))
                            .any(|i| layer.coverage.touches_quad(i, j))
                    })
                }
            };
            if keep {
                let want = surface_winding(self.surface);
                let w = winding(*tri);
                if w == want || w == Ordering::Equal {
                    out.extend_from_slice(tri);
                } else {
                    out.extend_from_slice(&[tri[0], tri[2], tri[1]]);
                }
            }
        }
        out
    }
}

/// The winding of a triangle of grid vertex numbers seen from above (+Z): `Greater` for
/// counter-clockwise, `Less` for clockwise, `Equal` for a degenerate triangle. Vertex
/// numbers ≥ 1,089 wrap into rows beyond the grid and give some answer; callers check the
/// range first.
fn winding(tri: [u16; 3]) -> Ordering {
    let p = tri.map(|v| {
        let v = i64::from(v);
        (v % GRID_SIDE as i64, v / GRID_SIDE as i64)
    });
    let cross = (p[1].0 - p[0].0) * (p[2].1 - p[0].1) - (p[1].1 - p[0].1) * (p[2].0 - p[0].0);
    cross.cmp(&0)
}

/// The winding of a surface's triangles from above: counter-clockwise for the terrain,
/// clockwise for the ceiling (`landblock.md` §3.5).
fn surface_winding(surface: GridSurface) -> Ordering {
    match surface {
        GridSurface::Terrain => Ordering::Greater,
        GridSurface::Ceiling => Ordering::Less,
    }
}

fn min3(v: [usize; 3]) -> usize {
    v[0].min(v[1]).min(v[2])
}

fn max3(v: [usize; 3]) -> usize {
    v[0].max(v[1]).max(v[2])
}

/// Resolves the layers of `chunk` (a terrain or ceiling chunk, `surface`) against the
/// region's terrain type `table`, with the block's vertex `flags` (for the diagonals of a
/// `whole` layer and of the coverage) and the surface's `light_map`.
///
/// Hostile data that is not an error, each logged with **one** warning per chunk (the
/// details are in the result): layers whose key is missing from the table
/// ([`ChunkLayers::unresolved`]); stored triangles that are off the level-0 grid, wound
/// against the surface or repeated within a layer, which are not drawn
/// ([`ChunkLayers::rejected_triangles`]); a key named by more than one layer (§4.2 says no
/// chunk does). All `whole` layers of a chunk share one full-grid triangle list.
///
/// # Errors
///
/// The hostile-chunk errors of [`splat_layers`]: more than
/// [`crate::splat::MAX_SPLAT_LAYERS`] layers, a window outside the index list, a vertex
/// number ≥ 1,089, or overlapping windows. Missing keys are not errors (they are
/// skipped and listed in [`ChunkLayers::unresolved`]).
pub fn resolve_chunk(
    chunk: &SurfaceChunk,
    flags: &VertexFlags,
    surface: GridSurface,
    table: &TerrainTypeTable,
    light_map: Option<Did>,
    options: &ResolveOptions,
) -> Result<ChunkLayers> {
    let splat = splat_layers(chunk)?;
    let diagonals = Diagonals::new(flags, surface);
    let mut out = ChunkLayers {
        surface,
        layers: Vec::with_capacity(splat.len()),
        empty: Vec::new(),
        unresolved: Vec::new(),
        rejected_triangles: 0,
        diagonals,
    };
    let mut full: Option<Arc<[[u16; 3]]>> = None;
    let mut seen_keys = std::collections::BTreeSet::new();
    let mut repeated_keys = 0usize;
    let mut splat = splat.into_iter().peekable();
    for (ordinal, layer) in chunk.layers.iter().enumerate() {
        if !seen_keys.insert(layer.material) {
            repeated_keys += 1;
        }
        // The client looks every key up, empty layers included (§8, T38).
        let entry = table.get(layer.material);
        if entry.is_none() {
            out.unresolved.push(UnresolvedLayer {
                ordinal,
                key: layer.material,
            });
        }
        let drawable = splat.next_if(|s| s.ordinal == ordinal);
        let Some(s) = drawable else {
            out.empty.push(ordinal);
            continue;
        };
        let Some(entry) = entry else { continue };
        let (triangles, coverage) = if s.kind == LayerKind::Whole {
            let tris = full.get_or_insert_with(|| diagonals.full_grid(surface).into());
            (Arc::clone(tris), TriangleSet::ALL)
        } else {
            let mut tris = Vec::with_capacity(s.indices.len() / 3);
            let mut cov = TriangleSet::EMPTY;
            for t in s.indices.as_chunks::<3>().0 {
                match diagonals.key_of_wound(*t, surface) {
                    Some(k) if !cov.contains(k) => {
                        cov.insert(k);
                        tris.push(*t);
                    }
                    _ => out.rejected_triangles += 1,
                }
            }
            (tris.into(), cov)
        };
        let some = |d: Did| (!d.is_null()).then_some(d);
        let available = Availability {
            normal: options.normal_maps && some(entry.normal).is_some(),
            emissive_glow: some(entry.emissive).is_some() || some(entry.glow).is_some(),
            light_map: light_map.is_some(),
        };
        out.layers.push(DrawLayer {
            ordinal,
            draw_index: out.layers.len(),
            key: layer.material,
            whole: layer.whole,
            mask: s.mask,
            light_map,
            entry: entry.clone(),
            available,
            triangles,
            coverage,
        });
    }
    if let Some(first) = out.unresolved.first() {
        tracing::warn!(
            count = out.unresolved.len(),
            first_ordinal = first.ordinal,
            first_key = first.key,
            table = %table.did,
            ?surface,
            "terrain layers name keys missing from their terrain type table; layers skipped"
        );
    }
    if out.rejected_triangles > 0 {
        tracing::warn!(
            count = out.rejected_triangles,
            ?surface,
            "terrain layer triangles off the level-0 grid, wound against the surface or \
             repeated; not drawn"
        );
    }
    if repeated_keys > 0 {
        tracing::warn!(
            count = repeated_keys,
            ?surface,
            "terrain chunk names a terrain type key in more than one layer"
        );
    }
    Ok(out)
}

/// [`resolve_chunk`] for the terrain or ceiling chunk of a decoded landblock, with its
/// light maps (`landblock.md` §6.4).
///
/// # Errors
///
/// [`TerrainError::NoCeiling`] for the ceiling of a block without one; otherwise as
/// [`resolve_chunk`].
pub fn resolve_landblock(
    data: &LandblockData,
    surface: GridSurface,
    table: &TerrainTypeTable,
    light_maps: &LightMaps,
    options: &ResolveOptions,
) -> Result<ChunkLayers> {
    let chunk = data.surface_chunk(surface).ok_or(TerrainError::NoCeiling)?;
    resolve_chunk(
        chunk,
        &data.vertex_flags,
        surface,
        table,
        light_maps.for_surface(surface),
        options,
    )
}

/// The terrain type of quad (`qx`, `qy`) of `chunk`'s quad grid (§4.6: the dominant
/// visible type, index 32 · qy + qx), for ground-type lookups such as the entry's
/// `physics_material`. Not used for drawing. `None` without a quad grid, outside
/// 0–31, or for a key the table does not have.
#[must_use]
pub fn quad_terrain_type<'t>(
    chunk: &SurfaceChunk,
    table: &'t TerrainTypeTable,
    qx: usize,
    qy: usize,
) -> Option<&'t TerrainType> {
    table.get(u32::from(chunk.quad_material(qx, qy)?))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::splat::MAX_SPLAT_LAYERS;
    use tessera_formats::landblock::SurfaceLayer;

    fn entry(key: u32) -> TerrainType {
        TerrainType {
            key,
            label: format!("t{key}"),
            draw_order: 10 * (key + 1),
            diffuse: Did(0x4000_0001 + key),
            u_scale: 0.1,
            v_scale: 0.1,
            ..TerrainType::default()
        }
    }

    fn table(n: u32) -> TerrainTypeTable {
        TerrainTypeTable::new(Did(0x1900_0000), (0..n).map(entry).collect()).unwrap()
    }

    fn layer(material: u32, mask: u32, whole: bool, start: u16, prims: u16) -> SurfaceLayer {
        SurfaceLayer {
            material,
            mask: Did(mask),
            whole,
            start,
            prims,
            min_vertex: 0,
            vertex_span: 0,
        }
    }

    fn v(i: usize, j: usize) -> u16 {
        (GRID_SIDE * j + i) as u16
    }

    #[test]
    fn keys_of_grid_triangles() {
        let mut flags = VertexFlags::default();
        flags.raw_mut()[GRID_SIDE * 3 + 4] = VertexFlags::SPLIT_SW_NE; // quad (3, 4)
        let d = Diagonals::new(&flags, GridSurface::Terrain);
        // Quad (3, 4) split SW-NE: west = SW, NE, NW.
        let k = 2 * (32 * 4 + 3);
        assert_eq!(d.key_of([v(3, 4), v(4, 5), v(3, 5)]), Some(k));
        assert_eq!(d.key_of([v(3, 4), v(4, 4), v(4, 5)]), Some(k + 1));
        // The other diagonal's triangles are not grid triangles there.
        assert_eq!(d.key_of([v(3, 4), v(4, 4), v(3, 5)]), None);
        // Quad (0, 0) split SE-NW: west = SW, SE, NW.
        assert_eq!(d.key_of([v(0, 0), v(1, 0), v(0, 1)]), Some(0));
        assert_eq!(d.key_of([v(1, 1), v(0, 1), v(1, 0)]), Some(1));
        assert_eq!(d.key_of([v(0, 0), v(2, 0), v(0, 1)]), None);
        assert_eq!(d.key_of([v(0, 0), v(0, 0), v(0, 1)]), None);
        assert_eq!(d.key_of([1089, 0, 1]), None);
        assert_eq!(d.key_of([v(32, 0), v(32, 1), v(31, 1)]), Some(2 * 31 + 1));
        // Points.
        assert_eq!(d.key_at(1.0, 1.0), Some(0));
        assert_eq!(d.key_at(4.0, 4.0), Some(1));
        assert_eq!(d.key_at(16.0, 23.0), Some(k)); // fx 0.2, fy 0.6 → west of SW-NE
        assert_eq!(d.key_at(19.0, 21.0), Some(k + 1));
        assert_eq!(d.key_at(f32::NAN, 0.0), None);
        assert_eq!(d.key_at(1e9, -1e9), Some(2 * 31)); // (160, 0) lies on the diagonal
        // The full grid matches the decoder's triangle numbering.
        let mut data = LandblockData::from_heights(
            Did(0x8001_0000),
            tessera_formats::landblock::HeightGrid::flat(100),
        );
        data.vertex_flags = flags.clone();
        let full = d.full_grid(GridSurface::Terrain);
        for (key, t) in full.iter().enumerate() {
            let expect = data
                .grid_triangle(GridSurface::Terrain, key as u16)
                .unwrap()
                .map(|(i, j)| v(i, j));
            assert_eq!(*t, expect, "key {key}");
            assert_eq!(d.key_of(*t), Some(key as u16));
        }
    }

    #[test]
    fn layers_resolve_in_stored_order() {
        let chunk = SurfaceChunk {
            layers: vec![
                layer(0, 0, false, 0, 0),           // empty, skipped
                layer(2, 0, false, 0, 2),           // first drawn: opaque
                layer(1, 0x4100_0009, false, 6, 1), // overlay
                layer(7, 0, false, 0, 0),           // missing key (empty)
                layer(9, 0, false, 9, 1),           // missing key (drawable)
                layer(3, 0, true, 0, 0),            // whole
            ],
            indices: vec![
                v(0, 0),
                v(1, 0),
                v(0, 1),
                v(1, 0),
                v(1, 1),
                v(0, 1),
                v(1, 0),
                v(2, 0),
                v(1, 1),
                v(2, 0),
                v(3, 0),
                v(2, 1),
            ],
            quads: None,
        };
        let t = table(4);
        let r = resolve_chunk(
            &chunk,
            &VertexFlags::default(),
            GridSurface::Terrain,
            &t,
            Some(Did(0x4100_7777)),
            &ResolveOptions::default(),
        )
        .unwrap();
        assert_eq!(r.layers.len(), 3);
        assert_eq!(r.empty, [0, 3]);
        assert_eq!(
            r.unresolved,
            [
                UnresolvedLayer { ordinal: 3, key: 7 },
                UnresolvedLayer { ordinal: 4, key: 9 }
            ]
        );
        let [a, b, c] = [&r.layers[0], &r.layers[1], &r.layers[2]];
        assert_eq!((a.ordinal, a.key, a.draw_index), (1, 2, 0));
        assert!(a.first_drawn());
        assert_eq!(a.splat().opacity, OpacityRule::Opaque);
        assert_eq!(a.coverage().len(), 2);
        assert_eq!((b.ordinal, b.key), (2, 1));
        assert_eq!(b.splat().opacity, OpacityRule::Mask);
        assert!(b.features().overlay);
        assert_eq!(b.textures().mask, Some(Did(0x4100_0009)));
        assert_eq!(b.textures().light_map, Some(Did(0x4100_7777)));
        assert_eq!(a.textures().mask, None);
        assert!(c.whole);
        assert_eq!(c.triangles.len(), GRID_TRIANGLE_COUNT);
        assert_eq!(c.splat().opacity, OpacityRule::One);
        assert!(r.uncovered().is_empty());
        assert_eq!(r.rejected_triangles(), 0);
        // Without the whole layer, the grid has holes.
        let mut holes = r.clone();
        assert!(holes.skip_layer(3).is_none());
        assert_eq!(holes.skip_layer(2).map(|l| l.key()), Some(3));
        assert_eq!(holes.uncovered().len(), GRID_TRIANGLE_COUNT - 3);
        assert!(!holes.uncovered().contains(0));
        assert_eq!(
            holes.composite(150.0, 150.0, |_| ([1.0; 3], 1.0, 1.0)),
            None
        );
    }

    #[test]
    fn ceiling_whole_layers_use_bit_zero_and_face_down() {
        let mut flags = VertexFlags::default();
        flags.raw_mut()[0] = VertexFlags::CEILING_SPLIT_SW_NE;
        let chunk = SurfaceChunk {
            layers: vec![layer(0, 0, true, 0, 0)],
            indices: Vec::new(),
            quads: None,
        };
        let r = resolve_chunk(
            &chunk,
            &flags,
            GridSurface::Ceiling,
            &table(1),
            None,
            &ResolveOptions::default(),
        )
        .unwrap();
        let tris = &r.layers[0].triangles;
        // Quad (0, 0) split SW-NE (bit 0), clockwise from above: reversed corner order.
        assert_eq!(tris[0], [v(0, 1), v(1, 1), v(0, 0)]);
        assert_eq!(tris[1], [v(1, 1), v(1, 0), v(0, 0)]);
        let x = |t: [u16; 3]| {
            let p = t.map(|k| {
                (
                    (k as usize % GRID_SIDE) as f64,
                    (k as usize / GRID_SIDE) as f64,
                )
            });
            (p[1].0 - p[0].0) * (p[2].1 - p[0].1) - (p[1].1 - p[0].1) * (p[2].0 - p[0].0)
        };
        assert!(tris.iter().all(|&t| x(t) < 0.0));
        assert!(!r.layers[0].features().baked_light);
    }

    #[test]
    fn features_follow_section_8_1() {
        let mut e = entry(0);
        e.use_specular = true;
        e.use_gloss_specular = true;
        e.normal = Did(0x4000_0100);
        let all = Availability {
            normal: true,
            emissive_glow: false,
            light_map: true,
        };
        let f = LayerFeatures::select(&e, true, false, all);
        assert!(f.specular && f.normal_map && !f.gloss_from_normal && f.baked_light);
        assert_eq!(f.colour_alpha(), ColourAlphaRole::Gloss);
        e.use_opacity_mask = true;
        let f = LayerFeatures::select(&e, false, true, all);
        assert!(f.overlay && f.opacity && !f.specular);
        assert_eq!(f.colour_alpha(), ColourAlphaRole::Opacity);
        e.specular_from_normal = true;
        let f = LayerFeatures::select(&e, false, true, all);
        assert!(f.specular && f.gloss_from_normal);
        let f = LayerFeatures::select(&e, false, true, Availability::default());
        assert!(!f.specular && !f.gloss_from_normal && !f.normal_map);
        // The first drawn layer has neither overlay nor opacity.
        let f = LayerFeatures::select(&e, true, true, all);
        assert!(!f.overlay && !f.opacity && f.specular);
        e.use_gloss_specular = false;
        assert!(!LayerFeatures::select(&e, true, false, all).specular);
    }

    #[test]
    fn params_follow_section_8_2() {
        let mut e = entry(1);
        e.diffuse_scale = Argb(0x0080_4020);
        e.specular_colour = Argb(0xFF10_2030);
        e.specular_power = 8.0;
        e.alpha_test_threshold = 0.1;
        e.alpha_blend_max = 0.25;
        e.glow_colour = Argb(0xFFFF_0000);
        let l = |first: bool, e: TerrainType| DrawLayer {
            ordinal: 0,
            draw_index: usize::from(!first),
            key: 1,
            whole: false,
            mask: Some(Did(0x4100_0001)),
            light_map: None,
            entry: e,
            available: Availability::default(),
            triangles: Arc::new([]),
            coverage: TriangleSet::EMPTY,
        };
        let p = l(false, e.clone()).params();
        assert_eq!(p.diffuse_colour[3], 1.0);
        assert!((p.diffuse_colour[0] - 128.0 / 255.0).abs() < 1e-6);
        assert_eq!(p.specular_colour, [1.0; 4]);
        assert_eq!(p.specular_power, 0.0);
        assert_eq!(p.glow_colour, [0.0, 0.0, 0.0, 1.0]);
        assert!(p.alpha_test);
        assert_eq!(p.alpha_test_ref, 0.1);
        assert_eq!(p.alpha_blend_max, 0.25);
        let first = l(true, e.clone()).params();
        assert!(!first.alpha_test);
        assert_eq!((first.alpha_test_ref, first.alpha_blend_max), (0.0, 0.0));
        e.use_specular = true;
        e.alpha_test_threshold = ALPHA_TEST_MIN;
        let mut layer = l(false, e);
        layer.available.emissive_glow = true;
        let p = layer.params();
        assert_eq!(p.specular_power, 8.0);
        assert!((p.specular_colour[2] - 48.0 / 255.0).abs() < 1e-6);
        assert_eq!(p.glow_colour, [1.0, 0.0, 0.0, 1.0]);
        assert!(!p.alpha_test);
        layer.drop_mask();
        assert_eq!(layer.splat().opacity, OpacityRule::One);
    }

    #[test]
    fn opacity_rules() {
        let r = OpacityRule::MaskedDiffuse {
            alpha_blend_max: 0.25,
        };
        assert!((r.opacity(0.8, 0.5) - 0.56).abs() < 1e-6);
        assert_eq!(r.opacity(1.0, 1.0), 1.0);
        assert_eq!(OpacityRule::Mask.opacity(0.3, 0.9), 0.3);
        assert_eq!(OpacityRule::Diffuse.opacity(0.3, 0.9), 0.9);
        assert_eq!(OpacityRule::One.opacity(0.0, 0.0), 1.0);
        assert_eq!(OpacityRule::Mask.opacity(f32::NAN, 0.0), 0.0);
        let s = SplatInputs {
            first_drawn: false,
            opacity: OpacityRule::Mask,
            alpha_test_ref: 0.0,
        };
        // τ = 0: only fully transparent pixels are dropped.
        assert_eq!(s.alpha(0.0, 1.0), None);
        assert_eq!(s.blend([0.2; 3], [1.0; 3], 0.0, 1.0), [0.2; 3]);
        assert_eq!(s.alpha(0.01, 1.0), Some(0.01));
    }

    #[test]
    fn lod_indices() {
        // A layer covering the west triangle of quad (0, 0) only.
        let chunk = SurfaceChunk {
            layers: vec![layer(0, 0, false, 0, 1)],
            indices: vec![v(0, 0), v(1, 0), v(0, 1)],
            quads: None,
        };
        let r = resolve_chunk(
            &chunk,
            &VertexFlags::default(),
            GridSurface::Terrain,
            &table(1),
            None,
            &ResolveOptions::default(),
        )
        .unwrap();
        let block = crate::TerrainBlock::flat(1000);
        let level0 = crate::triangulate(&block, crate::Lod::FINEST, crate::EdgeLods::NONE);
        assert_eq!(r.layer_indices_for(0, &level0), [v(0, 0), v(1, 0), v(0, 1)]);
        // At level 1 the two triangles of coarse quad (0, 0) both touch quad (0, 0).
        let level1 = crate::triangulate(&block, crate::Lod::new(1).unwrap(), crate::EdgeLods::NONE);
        let kept = r.layer_indices_for(0, &level1);
        assert_eq!(kept.len(), 6);
        assert!(
            kept.iter()
                .all(|&k| usize::from(k) % GRID_SIDE <= 2 && usize::from(k) / GRID_SIDE <= 2)
        );
        assert!(r.layer_indices_for(1, &level1).is_empty());
        assert!(r.layer_indices_for(0, &[2000, 0, 1]).is_empty());
    }

    #[test]
    fn hostile_chunks_and_quads() {
        let t = table(2);
        let chunk = SurfaceChunk {
            layers: vec![layer(0, 0, false, 0, 1)],
            indices: vec![v(0, 0), v(5, 0), v(0, 7)],
            quads: Some(Box::new([1u16; QUAD_COUNT])),
        };
        let r = resolve_chunk(
            &chunk,
            &VertexFlags::default(),
            GridSurface::Terrain,
            &t,
            None,
            &ResolveOptions::default(),
        )
        .unwrap();
        assert_eq!(r.rejected_triangles(), 1);
        assert!(r.layers[0].coverage().is_empty());
        assert!(r.layers[0].triangles().is_empty());
        assert_eq!(quad_terrain_type(&chunk, &t, 3, 4).map(|e| e.key), Some(1));
        assert!(quad_terrain_type(&chunk, &t, 32, 0).is_none());
        let bad = SurfaceChunk {
            layers: vec![layer(0, 0, false, 3, 5)],
            indices: vec![0; 9],
            quads: None,
        };
        assert!(
            resolve_chunk(
                &bad,
                &VertexFlags::default(),
                GridSurface::Terrain,
                &t,
                None,
                &ResolveOptions::default()
            )
            .is_err()
        );
        let data = LandblockData::from_heights(
            Did(0x8001_0000),
            tessera_formats::landblock::HeightGrid::flat(5),
        );
        assert_eq!(
            resolve_landblock(
                &data,
                GridSurface::Ceiling,
                &t,
                &LightMaps::default(),
                &ResolveOptions::default()
            ),
            Err(TerrainError::NoCeiling)
        );
    }

    fn area(t: [u16; 3]) -> i64 {
        match winding(t) {
            Ordering::Greater => 1,
            Ordering::Less => -1,
            Ordering::Equal => 0,
        }
    }

    fn canon(t: [u16; 3]) -> [u16; 3] {
        let k = (0..3).min_by_key(|&k| t[k]).unwrap_or(0);
        [t[k], t[(k + 1) % 3], t[(k + 2) % 3]]
    }

    fn cave(chunk: SurfaceChunk, flags: VertexFlags) -> LandblockData {
        let mut data = LandblockData::from_heights(
            Did(0x8001_0000),
            tessera_formats::landblock::HeightGrid::flat(100),
        );
        data.vertex_flags = flags;
        data.ceiling = Some(tessera_formats::landblock::Ceiling {
            heights: tessera_formats::landblock::HeightGrid::flat(900),
            tree: None,
            surface: chunk,
        });
        data
    }

    /// Review finding: on a ceiling, the level-of-detail lists must face down like the
    /// stored ceiling triangles (clockwise from above), not up as `triangulate` winds them.
    #[test]
    fn ceiling_lod_lists_face_down() {
        let mut flags = VertexFlags::default();
        flags.raw_mut()[GRID_SIDE + 1] = VertexFlags::CEILING_SPLIT_SW_NE; // quad (1, 1)
        flags.raw_mut()[GRID_SIDE + 2] = VertexFlags::SPLIT_SW_NE; // terrain only
        // Stored clockwise ceiling triangles: quad (0, 0) west (SE-NW diagonal) and quad
        // (1, 1) east (SW-NE diagonal from bit 0); plus a whole layer.
        let chunk = SurfaceChunk {
            layers: vec![layer(0, 0, false, 0, 2), layer(1, 0, true, 0, 0)],
            indices: vec![v(0, 1), v(1, 0), v(0, 0), v(2, 2), v(2, 1), v(1, 1)],
            quads: None,
        };
        let data = cave(chunk, flags);
        let r = resolve_landblock(
            &data,
            GridSurface::Ceiling,
            &table(2),
            &LightMaps::default(),
            &ResolveOptions::default(),
        )
        .unwrap();
        assert_eq!(r.rejected_triangles(), 0);
        assert_eq!(r.layers()[0].coverage().len(), 2);
        assert!(r.layers()[0].triangles().iter().all(|&t| area(t) < 0));
        let block = crate::TerrainBlock::from_ceiling(&data).unwrap();
        for lod in 0..crate::Lod::COUNT as u8 {
            let lod = crate::Lod::new(lod).unwrap();
            let list = crate::triangulate(&block, lod, crate::EdgeLods::NONE);
            assert!(list.as_chunks::<3>().0.iter().all(|&t| area(t) > 0));
            for k in 0..2 {
                let kept = r.layer_indices_for(k, &list);
                assert!(!kept.is_empty());
                assert!(
                    kept.as_chunks::<3>().0.iter().all(|&t| area(t) < 0),
                    "{lod:?} layer {k}"
                );
                if lod == crate::Lod::FINEST {
                    // Level 0 gives the stored triangles back, winding included.
                    let got: Vec<[u16; 3]> =
                        kept.as_chunks::<3>().0.iter().map(|&t| canon(t)).collect();
                    let mut want: Vec<[u16; 3]> = r.layers()[k]
                        .triangles()
                        .iter()
                        .map(|&t| canon(t))
                        .collect();
                    let mut got = got;
                    got.sort_unstable();
                    want.sort_unstable();
                    assert_eq!(got, want, "layer {k}");
                }
            }
        }
        // The terrain gets counter-clockwise lists, also from a clockwise input.
        let chunk = SurfaceChunk {
            layers: vec![layer(0, 0, true, 0, 0)],
            indices: Vec::new(),
            quads: None,
        };
        let t = resolve_chunk(
            &chunk,
            &VertexFlags::default(),
            GridSurface::Terrain,
            &table(1),
            None,
            &ResolveOptions::default(),
        )
        .unwrap();
        assert_eq!(
            t.layer_indices_for(0, &[v(0, 1), v(1, 0), v(0, 0), v(0, 0), v(0, 0), v(1, 1)]),
            [v(0, 1), v(0, 0), v(1, 0), v(0, 0), v(0, 0), v(1, 1)]
        );
    }

    /// Review finding: what is drawn must equal the coverage, so stored triangles off the
    /// grid, wound against the surface or repeated are not drawn.
    #[test]
    fn drawn_triangles_equal_coverage() {
        let chunk = SurfaceChunk {
            layers: vec![layer(0, 0, false, 0, 5)],
            indices: vec![
                v(0, 0),
                v(0, 1),
                v(32, 32), // in range, off the grid
                v(0, 0),
                v(0, 1),
                v(1, 0), // grid triangle 0, clockwise on the terrain
                v(1, 0),
                v(1, 1),
                v(0, 1), // grid triangle 1, counter-clockwise: drawn
                v(0, 1),
                v(1, 0),
                v(1, 1), // the same triangle again (rotated)
                v(0, 0),
                v(1, 0),
                v(0, 1), // grid triangle 0, counter-clockwise: drawn
            ],
            quads: None,
        };
        let r = resolve_chunk(
            &chunk,
            &VertexFlags::default(),
            GridSurface::Terrain,
            &table(1),
            None,
            &ResolveOptions::default(),
        )
        .unwrap();
        let l = &r.layers()[0];
        assert_eq!(r.rejected_triangles(), 3);
        assert_eq!(
            l.triangles(),
            [[v(1, 0), v(1, 1), v(0, 1)], [v(0, 0), v(1, 0), v(0, 1)]]
        );
        assert_eq!(l.coverage().iter().collect::<Vec<_>>(), [0, 1]);
        assert_eq!(r.uncovered().len(), GRID_TRIANGLE_COUNT - 2);
        // Only a wrongly wound triangle: nothing drawn, nothing covered.
        let wrong = SurfaceChunk {
            layers: vec![layer(0, 0, false, 0, 1)],
            indices: vec![v(0, 0), v(0, 1), v(1, 0)],
            quads: None,
        };
        let r = resolve_chunk(
            &wrong,
            &VertexFlags::default(),
            GridSurface::Terrain,
            &table(1),
            None,
            &ResolveOptions::default(),
        )
        .unwrap();
        assert!(r.layers()[0].triangles().is_empty());
        assert!(!r.covered().contains(0));
        // The same triangle is the right winding for a ceiling.
        let r = resolve_chunk(
            &wrong,
            &VertexFlags::default(),
            GridSurface::Ceiling,
            &table(1),
            None,
            &ResolveOptions::default(),
        )
        .unwrap();
        assert_eq!(r.rejected_triangles(), 0);
        assert!(r.covered().contains(0));
    }

    /// Review finding: `whole` layers share one full-grid list per chunk.
    #[test]
    fn whole_layers_share_one_grid() {
        let chunk = SurfaceChunk {
            layers: (0..256).map(|_| layer(0, 0, true, 0, 0)).collect(),
            indices: Vec::new(),
            quads: None,
        };
        let r = resolve_chunk(
            &chunk,
            &VertexFlags::default(),
            GridSurface::Terrain,
            &table(1),
            None,
            &ResolveOptions::default(),
        )
        .unwrap();
        assert_eq!(r.layers().len(), 256);
        let first = &r.layers()[0].triangles;
        assert!(r.layers().iter().all(|l| Arc::ptr_eq(&l.triangles, first)));
        assert_eq!(first.len(), GRID_TRIANGLE_COUNT);
    }

    /// Review finding: skipping a layer keeps exactly one opaque bottom layer.
    #[test]
    fn skip_layer_renumbers() {
        let chunk = SurfaceChunk {
            layers: vec![
                layer(0, 0, true, 0, 0),
                layer(1, 0x4100_0001, true, 0, 0),
                layer(2, 0, true, 0, 0),
            ],
            indices: Vec::new(),
            quads: None,
        };
        let mut r = resolve_chunk(
            &chunk,
            &VertexFlags::default(),
            GridSurface::Terrain,
            &table(3),
            None,
            &ResolveOptions::default(),
        )
        .unwrap();
        let removed = r.skip_layer(0).unwrap();
        assert_eq!((removed.key(), removed.draw_index()), (0, 0));
        assert_eq!(r.layers().len(), 2);
        assert!(r.layers()[0].first_drawn());
        assert_eq!(r.layers()[0].splat().opacity, OpacityRule::Opaque);
        assert_eq!(
            r.layers()
                .iter()
                .map(DrawLayer::draw_index)
                .collect::<Vec<_>>(),
            [0, 1]
        );
        assert_ne!(r.layers()[1].splat().opacity, OpacityRule::Opaque);
        let l = r.layer_mut(1).unwrap();
        l.set_available(Availability {
            normal: false,
            emissive_glow: true,
            light_map: false,
        });
        assert!(l.features().emissive_glow);
        assert!(r.layer_mut(2).is_none());
    }

    /// Review finding: a rejected mask is dropped with a logged warning.
    #[test]
    fn check_mask_drops_and_logs() {
        let chunk = SurfaceChunk {
            layers: vec![layer(0, 0, true, 0, 0), layer(1, 0x4100_0001, true, 0, 0)],
            indices: Vec::new(),
            quads: None,
        };
        let mut r = resolve_chunk(
            &chunk,
            &VertexFlags::default(),
            GridSurface::Terrain,
            &table(2),
            None,
            &ResolveOptions::default(),
        )
        .unwrap();
        let texels = [0u8; 16];
        let mut surface = Surface {
            did: Did(0x4100_0001),
            category: 0,
            width: 4,
            height: 4,
            format: tessera_formats::SurfaceFormat::A8,
            data: &texels,
        };
        let l = r.layer_mut(1).unwrap();
        assert_eq!(l.check_mask(&surface), Ok(()));
        assert_eq!(l.mask(), Some(Did(0x4100_0001)));
        surface.format = tessera_formats::SurfaceFormat::Dxt1;
        let (events, res) = count_warnings(|| l.check_mask(&surface));
        assert!(matches!(res, Err(TerrainError::MaskFormat { .. })));
        assert_eq!(events, 1);
        assert_eq!(l.mask(), None);
        assert_eq!(l.splat().opacity, OpacityRule::One);
        // Without a mask there is nothing to check.
        assert_eq!(l.check_mask(&surface), Ok(()));
    }

    /// Review finding: hostile chunks log once per chunk and kind, not once per layer.
    #[test]
    fn warnings_once_per_chunk() {
        let chunk = SurfaceChunk {
            layers: (0..256)
                .map(|k| layer(1000 + k, 0, k % 2 == 0, 0, 0))
                .chain(std::iter::once(layer(0, 0, false, 0, 1)))
                .chain(std::iter::once(layer(0, 0, false, 3, 1)))
                .take(MAX_SPLAT_LAYERS)
                .collect(),
            indices: vec![v(0, 0), v(0, 1), v(1, 0), v(0, 0), v(5, 0), v(0, 5)],
            quads: None,
        };
        let (events, r) = count_warnings(|| {
            resolve_chunk(
                &chunk,
                &VertexFlags::default(),
                GridSurface::Terrain,
                &table(1),
                None,
                &ResolveOptions::default(),
            )
        });
        let r = r.unwrap();
        assert_eq!(r.unresolved().len(), 256);
        assert_eq!(events, 1);
        let chunk = SurfaceChunk {
            layers: vec![layer(0, 0, false, 0, 1), layer(0, 0, false, 3, 1)],
            ..chunk
        };
        let (events, r) = count_warnings(|| {
            resolve_chunk(
                &chunk,
                &VertexFlags::default(),
                GridSurface::Terrain,
                &table(1),
                None,
                &ResolveOptions::default(),
            )
        });
        assert_eq!(r.unwrap().rejected_triangles(), 2);
        assert_eq!(events, 2); // rejected triangles, repeated key
    }

    /// Runs `f` and counts the warnings it emits on this thread.
    fn count_warnings<T>(f: impl FnOnce() -> T) -> (usize, T) {
        use std::sync::atomic::{AtomicUsize, Ordering as AtomicOrdering};
        struct Count(Arc<AtomicUsize>);
        impl tracing::Subscriber for Count {
            fn enabled(&self, _: &tracing::Metadata<'_>) -> bool {
                true
            }
            fn new_span(&self, _: &tracing::span::Attributes<'_>) -> tracing::span::Id {
                tracing::span::Id::from_u64(1)
            }
            fn record(&self, _: &tracing::span::Id, _: &tracing::span::Record<'_>) {}
            fn record_follows_from(&self, _: &tracing::span::Id, _: &tracing::span::Id) {}
            fn event(&self, e: &tracing::Event<'_>) {
                if *e.metadata().level() == tracing::Level::WARN {
                    self.0.fetch_add(1, AtomicOrdering::Relaxed);
                }
            }
            fn enter(&self, _: &tracing::span::Id) {}
            fn exit(&self, _: &tracing::span::Id) {}
        }
        let n = Arc::new(AtomicUsize::new(0));
        let out = tracing::subscriber::with_default(Count(Arc::clone(&n)), || {
            // A callsite first hit on another test thread without a subscriber caches the
            // interest "never"; rebuild it now that this thread's subscriber exists, or its
            // warnings are lost when the tests run in parallel.
            tracing::callsite::rebuild_interest_cache();
            f()
        });
        (n.load(AtomicOrdering::Relaxed), out)
    }
}
