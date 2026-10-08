// SPDX-License-Identifier: GPL-3.0-or-later
//! The terrain's far band (`walker.md` §14.4, Tessera Decision; the original's distance
//! bands are OPEN, `terrain-texturing.md` §12): beyond a distance, a block's layer stack is
//! drawn as **one** layer whose texture is the stack composited on the CPU, once, into a
//! top-down image of the block ([`bake`]). The composite is the CPU reference of the layer
//! pipeline's colour without its light terms (`terrain-texturing.md` §7–§9: every layer's
//! albedo *C* × `diffuse_scale` *M*, blended "over" by its opacity rule after the discard
//! test), and the baked layer is lit with *M* = 1. This matches the full stack for the
//! ambient, sun and placed-light terms, which carry *M* there (§9: *M* is pre-multiplied
//! into every light and ambient term). The light-map term *B*ₗ does not carry *M* in the
//! full stack (2 × *C* × *B*ₗ, `terrain-texturing.md` §9.1), so the far layer must not take
//! it from the albedo composite (2 × *C* × *M* × *B*ₗ). When the block has a light map and
//! some layer's *M* is not white, [`bake`] also composites the stack **without** *M* and
//! multiplies it by the light map, texel by texel ([`Baked::light`]: Σ *w* *C* × *B*ₗ); the
//! far layer then draws that image through the layer pipeline's emissive term with
//! *G* = 2 and without the light map, which gives 2 × Σ *w* *C* × *B*ₗ, the full stack's
//! light-map term (`walker.md` §14.4; the second baked image the OPEN there names).
//! Where every *M* is white, or the block has no light map, the albedo composite with the
//! light map is already exact and no second image is made. Per-layer normal maps and
//! specular highlights are not kept. The composites are stored in 8 bits, limited to 0–1;
//! that limit never acts on valid data, because *C* ≤ 1, *M* ≤ 1 and *B*ₗ ≤ 1 per channel
//! (`diffuse_scale` is stored as colour bytes, `terrain-texturing.md` §3.3) and "over"
//! blending keeps the result between its inputs.
//!
//! Layer textures are sampled from their small CPU levels (at most
//! `objects::SMALL_LEVEL` (64) texels) at the level whose texel best matches the bake's
//! texel footprint; masks bilinearly with clamp addressing (§5.2). Rows of the result run
//! from the south edge (row 0) northwards, like masks and light maps, so the far layer
//! samples it at (x / 160, y / 160).

use std::collections::HashMap;

use tessera_formats::terrain_type::TextureProjection;
use tessera_formats::{Did, RgbaImage};
use tessera_terrain::{AlphaMask, ChunkLayers, TerrainBlock};

/// Edge of a baked block image in texels (0.625 units per texel).
pub const BAKE_SIZE: u32 = 256;
/// Largest mask edge kept for baking (masks are reduced to it).
pub const BAKE_MASK_MAX: u32 = 128;
/// Landblock edge in units.
const BLOCK: f32 = 160.0;

/// What a bake reads for one layer texture: its small levels, largest first.
pub type SmallChain = std::sync::Arc<[RgbaImage]>;

/// A mask reduced to at most [`BAKE_MASK_MAX`] texels per side (box filter), for baking.
#[must_use]
pub fn reduce_mask(mask: &AlphaMask) -> AlphaMask {
    let (w, h) = (mask.width(), mask.height());
    let f = (w.max(h) / BAKE_MASK_MAX).max(1);
    if f == 1 {
        return mask.clone();
    }
    let (nw, nh) = ((w / f).max(1), (h / f).max(1));
    let src = mask.texels();
    let mut out = Vec::with_capacity((nw * nh) as usize);
    for r in 0..nh {
        for c in 0..nw {
            let mut sum = 0u32;
            let mut n = 0u32;
            for dy in 0..f {
                for dx in 0..f {
                    let (x, y) = (c * f + dx, r * f + dy);
                    if let Some(&b) = src.get((y * w + x) as usize) {
                        sum += u32::from(b);
                        n += 1;
                    }
                }
            }
            out.push(u8::try_from(sum / n.max(1)).unwrap_or(u8::MAX));
        }
    }
    AlphaMask::from_texels(mask.did(), nw, nh, out).unwrap_or_else(|_| mask.clone())
}

/// Bilinear sample of `img` at (u, v) with repeat addressing (u along the row, v down the
/// rows, texel centres at (k + ½) / n); RGBA in 0–1. White for an inconsistent image.
#[must_use]
pub fn sample_repeat(img: &RgbaImage, u: f32, v: f32) -> [f32; 4] {
    let (w, h) = (img.width as usize, img.height as usize);
    if w == 0 || h == 0 || img.pixels.len() != w * h * 4 || !u.is_finite() || !v.is_finite() {
        return [1.0; 4];
    }
    let axis = |p: f32, n: usize| -> (usize, usize, f32) {
        let f = p * n as f32 - 0.5;
        let lo = f.floor();
        let frac = f - lo;
        let k0 = (lo as i64).rem_euclid(n as i64) as usize;
        (k0, (k0 + 1) % n, frac)
    };
    let (c0, c1, fx) = axis(u, w);
    let (r0, r1, fy) = axis(v, h);
    let px = |c: usize, r: usize, k: usize| f32::from(img.pixels[(r * w + c) * 4 + k]) / 255.0;
    std::array::from_fn(|k| {
        let top = px(c0, r0, k) * (1.0 - fx) + px(c1, r0, k) * fx;
        let bottom = px(c0, r1, k) * (1.0 - fx) + px(c1, r1, k) * fx;
        top * (1.0 - fy) + bottom * fy
    })
}

/// Bilinear sample of `img` at (u, v) with clamp addressing (texel centres at
/// (k + ½) / n, coordinates outside the image take the edge texels), RGBA in 0–1: the
/// renderer's sampler for masks and light maps (`render.md` §5.2). `None` for an
/// inconsistent image or a coordinate that is not finite.
#[must_use]
pub fn sample_clamp(img: &RgbaImage, u: f32, v: f32) -> Option<[f32; 4]> {
    let (w, h) = (img.width as usize, img.height as usize);
    if w == 0 || h == 0 || img.pixels.len() != w * h * 4 || !u.is_finite() || !v.is_finite() {
        return None;
    }
    let axis = |p: f32, n: usize| -> (usize, usize, f32) {
        let f = (p * n as f32 - 0.5).clamp(0.0, (n - 1) as f32);
        let lo = f.floor();
        let k0 = lo as usize;
        (k0, (k0 + 1).min(n - 1), f - lo)
    };
    let (c0, c1, fx) = axis(u, w);
    let (r0, r1, fy) = axis(v, h);
    let px = |c: usize, r: usize, k: usize| f32::from(img.pixels[(r * w + c) * 4 + k]) / 255.0;
    Some(std::array::from_fn(|k| {
        let top = px(c0, r0, k) * (1.0 - fx) + px(c1, r0, k) * fx;
        let bottom = px(c0, r1, k) * (1.0 - fx) + px(c1, r1, k) * fx;
        top * (1.0 - fy) + bottom * fy
    }))
}

/// What a far-band bake produces (module docs).
#[derive(Debug, Clone, PartialEq)]
pub struct Baked {
    /// The stack's albedo × `diffuse_scale` composite (the far layer's diffuse texture).
    pub albedo: RgbaImage,
    /// The stack's albedo composite without `diffuse_scale`, multiplied by the block's
    /// light map (the far layer's emissive texture, drawn with *G* = 2 and no light map);
    /// `None` when the block has no light map or every layer's `diffuse_scale` is white.
    pub light: Option<RgbaImage>,
}

/// The emissive colour *G* the far layer draws [`Baked::light`] with: the factor 2 of
/// `terrain-texturing.md` §9.1 step 4.
pub const LIGHT_GLOW: [f32; 3] = [2.0; 3];

/// The level of `chain` (largest first) whose texel best matches a footprint of
/// `repeats` texture repeats per bake texel: the first level with at most about one
/// texel per bake texel, else the smallest.
#[must_use]
pub fn pick_level(chain: &[RgbaImage], repeats: f32) -> Option<&RgbaImage> {
    let r = if repeats.is_finite() {
        repeats.abs()
    } else {
        0.0
    };
    chain
        .iter()
        .find(|l| (l.width.max(l.height) as f32) * r <= 1.5)
        .or_else(|| chain.last())
}

/// Composites the layer stack of `layers` (the terrain chunk of block `key`, heights
/// `block`) into [`BAKE_SIZE`]² images (module docs). `diffuse` gives each layer
/// texture's small levels by DID (absent: white, the renderer's rule for a missing
/// diffuse texture); `masks` each mask (absent: drawn as if the layer had no mask, §5.1);
/// `light_map` the block's terrain light map (rows from the south edge, sampled at
/// (x / 160, y / 160) like the layer pipeline's set 3). Holes are left black with alpha 0.
/// Rows from the south edge.
#[must_use]
pub fn bake(
    layers: &ChunkLayers,
    block: &TerrainBlock,
    offset: [f32; 2],
    diffuse: &HashMap<Did, SmallChain>,
    masks: &HashMap<Did, AlphaMask>,
    light_map: Option<&RgbaImage>,
    size: u32,
) -> Baked {
    let n = size.max(1);
    let texel = BLOCK / n as f32;
    let mut pixels = vec![0u8; (n * n * 4) as usize];
    // The second image only where it changes something: a light map and a layer whose
    // `diffuse_scale` is not white.
    let light_map = light_map.filter(|_| {
        layers
            .layers()
            .iter()
            .any(|l| l.params().diffuse_colour[..3].iter().any(|c| *c != 1.0))
    });
    let mut light_pixels = light_map.map(|_| vec![0u8; (n * n * 4) as usize]);
    // The layers drawn over each grid triangle, cached per triangle key.
    let mut by_tri: Vec<Option<Vec<usize>>> = vec![None; 2048];
    let ls = layers.layers();
    // Per layer: its splat inputs, transform, translation, colour, chain level and mask.
    struct Prep<'a> {
        splat: tessera_terrain::SplatInputs,
        transform: tessera_terrain::LayerTransform,
        translation: [f32; 2],
        projection: TextureProjection,
        colour: [f32; 3],
        level: Option<&'a RgbaImage>,
        mask: Option<&'a AlphaMask>,
    }
    let prep: Vec<Prep<'_>> = ls
        .iter()
        .map(|l| {
            let p = l.params();
            let t = p.transform;
            let repeats = texel * t.u_scale.abs().max(t.v_scale.abs());
            let level = l
                .textures()
                .diffuse
                .and_then(|d| diffuse.get(&d))
                .and_then(|c| pick_level(c, repeats));
            let mask = l.textures().mask.and_then(|d| masks.get(&d));
            // A missing mask: drawn as if the layer had no mask (§5.1 Tessera Decision).
            let mut splat = l.splat();
            if l.textures().mask.is_some() && mask.is_none() {
                splat.opacity = match splat.opacity {
                    tessera_terrain::OpacityRule::MaskedDiffuse { .. } => {
                        tessera_terrain::OpacityRule::Diffuse
                    }
                    tessera_terrain::OpacityRule::Mask => tessera_terrain::OpacityRule::One,
                    o => o,
                };
            }
            Prep {
                splat,
                transform: t,
                translation: p.offsets.constant().unwrap_or([0.0; 2]),
                projection: l.projection(),
                colour: [
                    p.diffuse_colour[0],
                    p.diffuse_colour[1],
                    p.diffuse_colour[2],
                ],
                level,
                mask,
            }
        })
        .collect();
    for j in 0..n {
        let y = (j as f32 + 0.5) * texel;
        for i in 0..n {
            let x = (i as f32 + 0.5) * texel;
            let Some(key) = layers.triangle_at(x, y) else {
                continue;
            };
            // Borrowed, not cloned: one list per grid triangle, not one per texel.
            let Some(slot) = by_tri.get_mut(usize::from(key)) else {
                continue;
            };
            let list: &[usize] = slot.get_or_insert_with(|| {
                ls.iter()
                    .enumerate()
                    .filter(|(_, l)| l.coverage().contains(key))
                    .map(|(k, _)| k)
                    .collect()
            });
            if list.is_empty() {
                continue;
            }
            let h = block.height_at(x, y).unwrap_or(0.0);
            let mut col = [0.0f32; 3];
            let mut plain = [0.0f32; 3];
            for &k in list {
                let Some(p) = prep.get(k) else { continue };
                let pq = match p.projection {
                    TextureProjection::Xz => [x + offset[0], h],
                    TextureProjection::Yz => [y + offset[1], h],
                    _ => [x + offset[0], y + offset[1]],
                };
                let [u, v] = p.transform.apply(pq, p.translation);
                let tex = p.level.map_or([1.0; 4], |img| sample_repeat(img, u, v));
                let m = p.mask.map_or(1.0, |mk| mk.sample(x / BLOCK, y / BLOCK));
                let src = [
                    tex[0] * p.colour[0],
                    tex[1] * p.colour[1],
                    tex[2] * p.colour[2],
                ];
                col = p.splat.blend(col, src, m, tex[3]);
                if light_pixels.is_some() {
                    plain = p.splat.blend(plain, [tex[0], tex[1], tex[2]], m, tex[3]);
                }
            }
            let o = ((j * n + i) * 4) as usize;
            for c in 0..3 {
                pixels[o + c] = unit_byte(col[c]);
            }
            pixels[o + 3] = 255;
            if let (Some(lp), Some(lm)) = (light_pixels.as_mut(), light_map) {
                // A light map that does not sample adds nothing (the renderer binds none).
                let b = sample_clamp(lm, x / BLOCK, y / BLOCK).unwrap_or([0.0; 4]);
                for c in 0..3 {
                    lp[o + c] = unit_byte(plain[c] * b[c]);
                }
                lp[o + 3] = 255;
            }
        }
    }
    Baked {
        albedo: RgbaImage {
            width: n,
            height: n,
            pixels,
        },
        light: light_pixels.map(|pixels| RgbaImage {
            width: n,
            height: n,
            pixels,
        }),
    }
}

/// A channel value in 0–1 as a byte (limited to 0–1, rounded; NaN gives 0).
fn unit_byte(v: f32) -> u8 {
    (v.clamp(0.0, 1.0) * 255.0).round() as u8
}

/// The mip chain of a baked image: the image, then 2 × 2 box reductions down to 1 × 1.
#[must_use]
pub fn mip_chain(img: RgbaImage) -> Vec<RgbaImage> {
    let mut out = vec![img];
    while let Some(last) = out.last() {
        if last.width <= 1 && last.height <= 1 {
            break;
        }
        let next =
            super::geometry::downscale_to(last.clone(), (last.width.max(last.height) / 2).max(1));
        if next.width == last.width && next.height == last.height {
            break;
        }
        out.push(next);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use tessera_formats::landblock::{GridSurface, SurfaceChunk, SurfaceLayer, VertexFlags};
    use tessera_formats::terrain_type::{Argb, TerrainType, TerrainTypeTable};
    use tessera_terrain::{ResolveOptions, resolve_chunk};

    fn solid(w: u32, h: u32, rgba: [u8; 4]) -> RgbaImage {
        RgbaImage {
            width: w,
            height: h,
            pixels: rgba
                .iter()
                .copied()
                .cycle()
                .take((w * h * 4) as usize)
                .collect(),
        }
    }

    #[test]
    fn repeat_sampling_wraps_and_filters() {
        let mut img = solid(2, 1, [0, 0, 0, 255]);
        img.pixels[4..8].copy_from_slice(&[255, 255, 255, 255]);
        // Texel centres at u = 0.25 (black) and 0.75 (white).
        assert_eq!(sample_repeat(&img, 0.25, 0.5)[0], 0.0);
        assert_eq!(sample_repeat(&img, 0.75, 0.5)[0], 1.0);
        assert!((sample_repeat(&img, 0.5, 0.5)[0] - 0.5).abs() < 1e-6);
        // Wraps: u = 1.25 is u = 0.25; between 0.75 and 1.25 it blends through the edge.
        assert_eq!(sample_repeat(&img, 1.25, 0.5)[0], 0.0);
        assert!((sample_repeat(&img, 1.0, 0.5)[0] - 0.5).abs() < 1e-6);
        assert_eq!(sample_repeat(&img, -0.75, 7.0)[0], 0.0);
        let bad = RgbaImage {
            width: 3,
            height: 3,
            pixels: vec![0; 2],
        };
        assert_eq!(sample_repeat(&bad, 0.0, 0.0), [1.0; 4]);
        assert_eq!(sample_repeat(&img, f32::NAN, 0.0), [1.0; 4]);
    }

    #[test]
    fn levels_follow_the_footprint() {
        let chain = [
            solid(64, 64, [1; 4]),
            solid(8, 8, [2; 4]),
            solid(1, 1, [3; 4]),
        ];
        assert_eq!(pick_level(&chain, 0.01).map(|l| l.width), Some(64));
        assert_eq!(pick_level(&chain, 0.1).map(|l| l.width), Some(8));
        assert_eq!(pick_level(&chain, 5.0).map(|l| l.width), Some(1));
        assert_eq!(pick_level(&chain, f32::NAN).map(|l| l.width), Some(64));
        assert!(pick_level(&[], 1.0).is_none());
    }

    #[test]
    fn mip_chains_halve_to_one_texel() {
        let c = mip_chain(solid(8, 4, [10, 20, 30, 255]));
        let sizes: Vec<(u32, u32)> = c.iter().map(|l| (l.width, l.height)).collect();
        assert_eq!(sizes, [(8, 4), (4, 2), (2, 1), (1, 1)]);
        assert!(c.iter().all(|l| l.pixels[..4] == [10, 20, 30, 255]));
    }

    #[test]
    fn masks_reduce_by_box_filter() {
        let texels: Vec<u8> = (0..256 * 256)
            .map(|i| if i % 2 == 0 { 0 } else { 255 })
            .collect();
        let m = AlphaMask::from_texels(Did(0x4100_0001), 256, 256, texels).unwrap();
        let r = reduce_mask(&m);
        assert_eq!((r.width(), r.height()), (128, 128));
        assert!(r.texels().iter().all(|&b| b == 127));
        let small = AlphaMask::from_texels(Did(0x4100_0002), 64, 64, vec![9; 64 * 64]).unwrap();
        assert_eq!(reduce_mask(&small), small);
    }

    /// The composite carries `diffuse_scale` *M* (`terrain-texturing.md` §14 bytes 0x20,
    /// 0x40, 0x80: R 0.502, G 0.251, B 0.125) unclamped: a white texture bakes to
    /// round(255 × *M*).
    #[test]
    fn bake_folds_in_the_diffuse_scale() {
        let block = TerrainBlock::flat(10_000);
        let mut chunk = SurfaceChunk::default();
        chunk.layers.push(SurfaceLayer {
            material: 0,
            whole: true,
            ..SurfaceLayer::default()
        });
        let table = TerrainTypeTable::new(
            Did(0x1900_0000),
            vec![TerrainType {
                key: 0,
                diffuse: Did(0x4000_0001),
                diffuse_scale: Argb(0xFF80_4020),
                u_scale: 0.1,
                v_scale: 0.1,
                ..TerrainType::default()
            }],
        )
        .unwrap();
        let layers = resolve_chunk(
            &chunk,
            &VertexFlags::default(),
            GridSurface::Terrain,
            &table,
            None,
            &ResolveOptions::default(),
        )
        .unwrap();
        let mut diffuse: HashMap<Did, SmallChain> = HashMap::new();
        diffuse.insert(Did(0x4000_0001), vec![solid(4, 4, [255; 4])].into());
        let img = bake(
            &layers,
            &block,
            [0.0; 2],
            &diffuse,
            &HashMap::new(),
            None,
            4,
        )
        .albedo;
        assert!(
            img.pixels.chunks(4).all(|p| p == [128, 64, 32, 255]),
            "{:?}",
            &img.pixels[..4]
        );
    }

    /// A base layer of one colour and a masked second layer of another over the north half:
    /// the bake shows the base south, the overlay north (rows from the south edge).
    #[test]
    fn bake_composites_the_stack() {
        let block = TerrainBlock::flat(10_000);
        let mut chunk = SurfaceChunk::default();
        chunk.layers.push(SurfaceLayer {
            material: 0,
            whole: true,
            ..SurfaceLayer::default()
        });
        chunk.layers.push(SurfaceLayer {
            material: 1,
            whole: true,
            mask: Did(0x4100_0010),
            ..SurfaceLayer::default()
        });
        let entry = |key: u32, diffuse: u32| TerrainType {
            key,
            diffuse: Did(diffuse),
            diffuse_scale: Argb(0xFFFF_FFFF),
            u_scale: 0.1,
            v_scale: 0.1,
            ..TerrainType::default()
        };
        let table = TerrainTypeTable::new(
            Did(0x1900_0000),
            vec![entry(0, 0x4000_0001), entry(1, 0x4000_0002)],
        )
        .unwrap();
        let layers = resolve_chunk(
            &chunk,
            &VertexFlags::default(),
            GridSurface::Terrain,
            &table,
            None,
            &ResolveOptions::default(),
        )
        .unwrap();
        let mut diffuse: HashMap<Did, SmallChain> = HashMap::new();
        diffuse.insert(Did(0x4000_0001), vec![solid(4, 4, [255, 0, 0, 255])].into());
        diffuse.insert(Did(0x4000_0002), vec![solid(4, 4, [0, 0, 255, 255])].into());
        // Mask: south half 0, north half 1 (row 0 = south).
        let texels: Vec<u8> = (0..16 * 16)
            .map(|i| if i / 16 < 8 { 0 } else { 255 })
            .collect();
        let mut masks = HashMap::new();
        masks.insert(
            Did(0x4100_0010),
            AlphaMask::from_texels(Did(0x4100_0010), 16, 16, texels).unwrap(),
        );
        let img = bake(&layers, &block, [0.0; 2], &diffuse, &masks, None, 32).albedo;
        assert_eq!((img.width, img.height), (32, 32));
        let px = |i: u32, j: u32| {
            let o = ((j * 32 + i) * 4) as usize;
            [
                img.pixels[o],
                img.pixels[o + 1],
                img.pixels[o + 2],
                img.pixels[o + 3],
            ]
        };
        assert_eq!(px(5, 2), [255, 0, 0, 255], "south: base layer");
        assert_eq!(px(5, 29), [0, 0, 255, 255], "north: masked overlay");
        // Without the mask the overlay is drawn as if it had none: α = 1 everywhere.
        let img = bake(
            &layers,
            &block,
            [0.0; 2],
            &diffuse,
            &HashMap::new(),
            None,
            8,
        )
        .albedo;
        assert!(img.pixels.chunks(4).all(|p| p == [0, 0, 255, 255]));
    }

    /// One whole layer of key 0 with `diffuse_scale` `scale` and a white texture.
    fn one_layer(scale: u32) -> (ChunkLayers, HashMap<Did, SmallChain>) {
        let mut chunk = SurfaceChunk::default();
        chunk.layers.push(SurfaceLayer {
            material: 0,
            whole: true,
            ..SurfaceLayer::default()
        });
        let table = TerrainTypeTable::new(
            Did(0x1900_0000),
            vec![TerrainType {
                key: 0,
                diffuse: Did(0x4000_0001),
                diffuse_scale: Argb(scale),
                u_scale: 0.1,
                v_scale: 0.1,
                ..TerrainType::default()
            }],
        )
        .unwrap();
        let layers = resolve_chunk(
            &chunk,
            &VertexFlags::default(),
            GridSurface::Terrain,
            &table,
            None,
            &ResolveOptions::default(),
        )
        .unwrap();
        let mut diffuse: HashMap<Did, SmallChain> = HashMap::new();
        diffuse.insert(
            Did(0x4000_0001),
            vec![solid(4, 4, [200, 100, 50, 255])].into(),
        );
        (layers, diffuse)
    }

    /// `terrain-texturing.md` §9.1: the light-map term is 2 × *C* × *B*ₗ, without *M*. The
    /// second image holds *C* × *B*ₗ (drawn with *G* = 2), the first *C* × *M*.
    #[test]
    fn the_light_map_term_is_baked_without_the_diffuse_scale() {
        let block = TerrainBlock::flat(10_000);
        let (layers, diffuse) = one_layer(0xFF80_4020);
        // A light map dark in the south half and bright in the north half (row 0 south).
        let lm = RgbaImage {
            width: 2,
            height: 2,
            pixels: vec![
                64, 64, 64, 255, 64, 64, 64, 255, // south row
                255, 255, 255, 255, 255, 255, 255, 255, // north row
            ],
        };
        let b = bake(
            &layers,
            &block,
            [0.0; 2],
            &diffuse,
            &HashMap::new(),
            Some(&lm),
            8,
        );
        let at = |img: &RgbaImage, i: u32, j: u32| {
            let o = ((j * 8 + i) * 4) as usize;
            [img.pixels[o], img.pixels[o + 1], img.pixels[o + 2]]
        };
        // Albedo × M: 200 × 0.502 = 100, 100 × 0.251 = 25, 50 × 0.125 = 6.
        assert_eq!(at(&b.albedo, 3, 3), [100, 25, 6]);
        let light = b.light.as_ref().expect("a light map and a non-white scale");
        // North edge (clamped to the bright row): C × 1, not C × M × 1.
        assert_eq!(at(light, 3, 7), [200, 100, 50]);
        // South edge: C × 64 / 255.
        assert_eq!(at(light, 3, 0), [50, 25, 13]);
        // Rows run from the south edge: the light grows northwards.
        assert!(at(light, 3, 2)[0] < at(light, 3, 5)[0]);
        // White scale, or no light map: the albedo composite is exact, no second image.
        let (white, diffuse) = one_layer(0xFFFF_FFFF);
        assert!(
            bake(
                &white,
                &block,
                [0.0; 2],
                &diffuse,
                &HashMap::new(),
                Some(&lm),
                8
            )
            .light
            .is_none()
        );
        assert!(
            bake(
                &layers,
                &block,
                [0.0; 2],
                &diffuse,
                &HashMap::new(),
                None,
                8
            )
            .light
            .is_none()
        );
        // A light map that cannot be sampled adds nothing.
        let broken = RgbaImage {
            width: 2,
            height: 2,
            pixels: vec![0; 3],
        };
        let b = bake(
            &layers,
            &block,
            [0.0; 2],
            &diffuse,
            &HashMap::new(),
            Some(&broken),
            8,
        );
        assert!(
            b.light
                .unwrap()
                .pixels
                .chunks(4)
                .all(|p| p == [0, 0, 0, 255])
        );
    }

    #[test]
    fn clamp_sampling_holds_the_edges() {
        let mut img = solid(2, 1, [0, 0, 0, 255]);
        img.pixels[4..8].copy_from_slice(&[255, 255, 255, 255]);
        let s = |u: f32| sample_clamp(&img, u, 0.5).unwrap()[0];
        assert_eq!(s(0.0), 0.0, "clamped to the first texel");
        assert_eq!(s(0.25), 0.0);
        assert!((s(0.5) - 0.5).abs() < 1e-6);
        assert_eq!(s(1.0), 1.0, "clamped to the last texel");
        assert_eq!(s(7.0), 1.0, "no wrap");
        assert!(sample_clamp(&img, f32::NAN, 0.0).is_none());
    }
}
