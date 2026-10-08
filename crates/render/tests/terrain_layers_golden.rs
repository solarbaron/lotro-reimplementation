// SPDX-License-Identifier: GPL-3.0-or-later
//! The terrain layer pipeline (M4.5, `docs/specs/terrain-texturing.md` §5–§10,
//! `docs/specs/render.md` §5.2):
//!
//! - `terrain_layers.png`: a 3 × 3 patch of synthetic landblocks splatted with three layers
//!   (an opaque base, a rock layer with masks, the masked-diffuse opacity rule, a normal map
//!   and gloss from the normal alpha, a sand path with the mask rule and an alpha test),
//!   one block with a light map, under a sun with its shadow, ambient and fog;
//! - pixel checks of the "over" blending, the four opacity rules, the discard threshold,
//!   the light map, the mask orientation (row 0 south) and the normal-map rotation against
//!   the spec's formulas, and hostile input;
//! - the stored vertex normal used as it is for the ambient and sun terms (§4.10), a
//!   downward (ceiling) normal, the specular term of §9.4 (row L5's sheen) and the
//!   baked-landscape lights left out of the chunks' light lists (`lighting.md` §8.3, §8.5).
//!
//! All data is generated here (no game data). Regenerate with
//! `TESSERA_BLESS=1 cargo test -p tessera-render --test terrain_layers_golden`.

#[path = "support/golden.rs"]
mod golden;
#[path = "support/gpu.rs"]
mod gpu;

use std::path::Path;

use tessera_render::glam::{Mat4, Vec3};
use tessera_render::{
    Camera, ColorSpace, Filter, Fog, FrameLighting, Light, LightReach, MipChainUpload, MipLevel,
    OffscreenFormat, Perspective, RenderError, Renderer, RgbaImage, SunLight, SunShadow,
    TerrainChunk, TerrainChunkData, TerrainDraw, TerrainGloss, TerrainLayer, TerrainLayerTextures,
    TerrainOpacity, TerrainProjection, TerrainSpecular, TerrainTransform, TerrainVertices,
    TextureId, TextureUpload, Wrap,
};

const WIDTH: u32 = 320;
const HEIGHT: u32 = 200;
const CLEAR: [f32; 4] = [0.55, 0.62, 0.72, 1.0];
const SIDE: usize = 33;
const SPACING: f32 = 5.0;
const BLOCK: f32 = 160.0;

/// `size × size` RGBA8 texels from a function of (x, y).
fn texels(size: u32, f: impl Fn(u32, u32) -> [u8; 4]) -> Vec<u8> {
    (0..size * size)
        .flat_map(|i| f(i % size, i / size))
        .collect()
}

/// A full mip chain by 2 × 2 box filtering.
fn mip_chain(size: u32, base: Vec<u8>) -> Vec<(u32, Vec<u8>)> {
    let mut levels = vec![(size, base)];
    while levels.last().is_some_and(|(s, _)| *s > 1) {
        let (s, prev) = levels.last().cloned().unwrap_or_default();
        let h = s / 2;
        let mut next = vec![0u8; (h * h * 4) as usize];
        for y in 0..h {
            for x in 0..h {
                for c in 0..4 {
                    let at = |xx: u32, yy: u32| u32::from(prev[((yy * s + xx) * 4 + c) as usize]);
                    let sum = at(2 * x, 2 * y)
                        + at(2 * x + 1, 2 * y)
                        + at(2 * x, 2 * y + 1)
                        + at(2 * x + 1, 2 * y + 1);
                    next[((y * h + x) * 4 + c) as usize] = ((sum + 2) / 4) as u8;
                }
            }
        }
        levels.push((h, next));
    }
    levels
}

fn upload_mips(
    r: &mut Renderer<'static>,
    size: u32,
    data: Vec<u8>,
    wrap: Wrap,
) -> Result<TextureId, RenderError> {
    let chain = mip_chain(size, data);
    let levels: Vec<MipLevel<'_>> = chain
        .iter()
        .map(|(s, d)| MipLevel {
            width: *s,
            height: *s,
            rgba: d,
        })
        .collect();
    r.upload_texture_mips(&MipChainUpload {
        color_space: ColorSpace::Linear,
        filter: Filter::Linear,
        wrap,
        ..MipChainUpload::new(&levels)
    })
}

fn upload(r: &mut Renderer<'static>, size: u32, data: &[u8]) -> Result<TextureId, RenderError> {
    r.upload_texture(&TextureUpload {
        color_space: ColorSpace::Linear,
        filter: Filter::Linear,
        wrap: Wrap::Clamp,
        ..TextureUpload::rgba8(size, size, data)
    })
}

/// A smooth value noise in 0..1 (deterministic, tileable over `period` cells).
fn noise(x: f32, y: f32, period: u32) -> f32 {
    let hash = |i: i32, j: i32| {
        let (i, j) = (
            i.rem_euclid(period as i32) as u32,
            j.rem_euclid(period as i32) as u32,
        );
        let mut h = i.wrapping_mul(374_761_393) ^ j.wrapping_mul(668_265_263);
        h = (h ^ (h >> 13)).wrapping_mul(1_274_126_177);
        ((h ^ (h >> 16)) & 0xffff) as f32 / 65_535.0
    };
    let (xi, yi) = (x.floor() as i32, y.floor() as i32);
    let (fx, fy) = (x - x.floor(), y - y.floor());
    let s = |t: f32| t * t * (3.0 - 2.0 * t);
    let (sx, sy) = (s(fx), s(fy));
    let a = hash(xi, yi) + sx * (hash(xi + 1, yi) - hash(xi, yi));
    let b = hash(xi, yi + 1) + sx * (hash(xi + 1, yi + 1) - hash(xi, yi + 1));
    a + sy * (b - a)
}

/// Heights of the whole patch in world units (x, y in 0..480).
fn height(x: f32, y: f32) -> f32 {
    24.0 * (x / 70.0).sin() * (y / 55.0).cos() + 9.0 * ((x + 0.6 * y) / 38.0).sin() + 30.0
}

fn normal(x: f32, y: f32) -> Vec3 {
    let e = 0.5;
    let dx = (height(x + e, y) - height(x - e, y)) / (2.0 * e);
    let dy = (height(x, y + e) - height(x, y - e)) / (2.0 * e);
    Vec3::new(-dx, -dy, 1.0).normalize()
}

/// Rock coverage: a broad patch over the hills' steeper parts, continuous across blocks.
fn rock_mask(x: f32, y: f32) -> f32 {
    let c = Vec3::new(190.0, 215.0, 0.0);
    let d = Vec3::new(x, y, 0.0).distance(c) / 70.0;
    (1.7 - d + 0.6 * (noise(x / 14.0, y / 14.0, 64) - 0.5)).clamp(0.0, 1.0)
}

/// A sand path across the patch.
fn path_mask(x: f32, y: f32) -> f32 {
    let centre = 120.0 + 60.0 * (x / 90.0).sin();
    let w = (1.0 - ((y - centre).abs() - 10.0) / 8.0).clamp(0.0, 1.0);
    w * (0.8 + 0.2 * noise(x / 6.0, y / 6.0, 128))
}

struct Block {
    positions: Vec<[f32; 3]>,
    normals: Vec<[f32; 3]>,
    tangents: Vec<[f32; 3]>,
    binormals: Vec<[f32; 3]>,
    sets: [Vec<[f32; 2]>; 4],
    triangles: Vec<[u16; 3]>,
}

/// One landblock at block (bx, by): positions block-local, sets as §6.1 with the anchor at
/// block (0, 0).
fn block(bx: u32, by: u32) -> Block {
    let mut b = Block {
        positions: Vec::new(),
        normals: Vec::new(),
        tangents: Vec::new(),
        binormals: Vec::new(),
        sets: Default::default(),
        triangles: Vec::new(),
    };
    let (ox, oy) = (bx as f32 * BLOCK, by as f32 * BLOCK);
    for j in 0..SIDE {
        for i in 0..SIDE {
            let (x, y) = (i as f32 * SPACING, j as f32 * SPACING);
            let h = height(ox + x, oy + y);
            let n = normal(ox + x, oy + y);
            // §6.4: T = normalise(E - (E . N) N), B = N x T.
            let t = (Vec3::X - n * n.x).normalize();
            let bn = n.cross(t);
            b.positions.push([x, y, h]);
            b.normals.push(n.to_array());
            b.tangents.push(t.to_array());
            b.binormals.push(bn.to_array());
            b.sets[0].push([x + ox, y + oy]);
            b.sets[1].push([x + ox, h]);
            b.sets[2].push([y + oy, h]);
            b.sets[3].push([x / BLOCK, y / BLOCK]);
        }
    }
    for j in 0..SIDE - 1 {
        for i in 0..SIDE - 1 {
            let a = (j * SIDE + i) as u16;
            let (r, u, ur) = (a + 1, a + SIDE as u16, a + SIDE as u16 + 1);
            b.triangles.push([a, r, ur]);
            b.triangles.push([a, ur, u]);
        }
    }
    b
}

/// A 64 × 64 mask of block (bx, by): value in alpha, row 0 = south edge (§5.2).
fn mask_texels(bx: u32, by: u32, f: impl Fn(f32, f32) -> f32) -> Vec<u8> {
    let w = 64u32;
    texels(w, |c, r| {
        let x = bx as f32 * BLOCK + (c as f32 + 0.5) * BLOCK / w as f32;
        let y = by as f32 * BLOCK + (r as f32 + 0.5) * BLOCK / w as f32;
        [255, 255, 255, (f(x, y) * 255.0).round() as u8]
    })
}

/// Triangles of `b` whose centroid has some coverage by `mask`.
fn covered(b: &Block, bx: u32, by: u32, mask: impl Fn(f32, f32) -> f32) -> Vec<[u16; 3]> {
    b.triangles
        .iter()
        .copied()
        .filter(|t| {
            t.iter().any(|&v| {
                let p = b.positions[usize::from(v)];
                mask(bx as f32 * BLOCK + p[0], by as f32 * BLOCK + p[1]) > 0.0
            }) || {
                let (sx, sy) = t.iter().fold((0.0, 0.0), |(sx, sy), &v| {
                    let p = b.positions[usize::from(v)];
                    (sx + p[0] / 3.0, sy + p[1] / 3.0)
                });
                mask(bx as f32 * BLOCK + sx, by as f32 * BLOCK + sy) > 0.0
            }
        })
        .collect()
}

fn lighting(center: Vec3) -> FrameLighting {
    let toward = Vec3::new(0.65, -0.45, 0.42).normalize();
    FrameLighting {
        ambient: [0.22, 0.24, 0.27],
        hemispheric: [0.06, 0.06, 0.04],
        sun: Some(SunLight {
            direction: toward,
            color: [0.55, 0.52, 0.45],
            specular: [0.7, 0.7, 0.65],
        }),
        fog: Some(Fog {
            near: 450.0,
            far: 1500.0,
            color: [CLEAR[0], CLEAR[1], CLEAR[2]],
        }),
        shadow: Some(SunShadow::fit(toward, center, 360.0, 1200.0, 2048)),
        ..FrameLighting::default()
    }
}

fn camera() -> Camera {
    Camera {
        eye: Vec3::new(20.0, -90.0, 170.0),
        target: Vec3::new(250.0, 250.0, 10.0),
        up: Vec3::Z,
        projection: Perspective {
            fov_y: 55f32.to_radians(),
            near: 1.0,
            far: 3000.0,
        },
    }
}

fn patch_scene(r: &mut Renderer<'static>) -> Result<RgbaImage, RenderError> {
    // Layer textures (stored values; alpha meaningful where the layer reads it).
    let grass = upload_mips(
        r,
        64,
        texels(64, |x, y| {
            let n = noise(x as f32 / 8.0, y as f32 / 8.0, 8);
            let g = noise(x as f32 / 3.0, y as f32 / 3.0, 21);
            [
                (70.0 + 30.0 * n) as u8,
                (105.0 + 40.0 * g) as u8,
                (45.0 + 20.0 * n) as u8,
                255,
            ]
        }),
        Wrap::Repeat,
    )?;
    // Rock: grey with an opacity pattern in alpha (masked-diffuse rule).
    let rock = upload_mips(
        r,
        64,
        texels(64, |x, y| {
            let n = noise(x as f32 / 6.0, y as f32 / 6.0, 10);
            let v = 80.0 + 70.0 * n;
            [
                v as u8,
                (v * 0.93) as u8,
                (v * 0.85) as u8,
                (110.0 + 140.0 * n) as u8,
            ]
        }),
        Wrap::Repeat,
    )?;
    // Rock bumps: a tangent-space normal map from a height field, gloss in alpha.
    let bumps = upload_mips(
        r,
        64,
        texels(64, |x, y| {
            let hf = |x: f32, y: f32| noise(x / 5.0, y / 5.0, 12) * 6.0;
            let (fx, fy) = (x as f32, y as f32);
            let dx = hf(fx + 1.0, fy) - hf(fx - 1.0, fy);
            // Image rows grow downward: the height difference along +v.
            let dy = hf(fx, fy + 1.0) - hf(fx, fy - 1.0);
            let n = Vec3::new(-dx, dy, 2.0).normalize();
            let e = |c: f32| ((c * 0.5 + 0.5) * 255.0).round() as u8;
            let gloss = if noise(fx / 4.0, fy / 4.0, 16) > 0.55 {
                230
            } else {
                40
            };
            [e(n.x), e(n.y), e(n.z), gloss]
        }),
        Wrap::Repeat,
    )?;
    let sand = upload_mips(
        r,
        64,
        texels(64, |x, y| {
            let n = noise(x as f32 / 2.5, y as f32 / 2.5, 25);
            [
                (190.0 + 30.0 * n) as u8,
                (165.0 + 25.0 * n) as u8,
                (115.0 + 20.0 * n) as u8,
                255,
            ]
        }),
        Wrap::Repeat,
    )?;
    // Light map of the centre block: a warm pool of baked light (RGB added to Sigma, §10).
    let light_map = upload(
        r,
        32,
        &texels(32, |c, row| {
            let (x, y) = ((c as f32 + 0.5) / 32.0, (row as f32 + 0.5) / 32.0);
            let d = ((x - 0.3).powi(2) + (y - 0.25).powi(2)).sqrt();
            let k = (1.0 - d / 0.25).clamp(0.0, 1.0);
            [(140.0 * k) as u8, (90.0 * k) as u8, (40.0 * k) as u8, 255]
        }),
    )?;

    let mut chunks = Vec::new();
    let mut drawn_layers = 0;
    for by in 0..3 {
        for bx in 0..3 {
            let b = block(bx, by);
            let rock_mask_tex = upload(r, 64, &mask_texels(bx, by, rock_mask))?;
            let path_mask_tex = upload(r, 64, &mask_texels(bx, by, path_mask))?;
            let rock_tris = covered(&b, bx, by, rock_mask);
            let path_tris = covered(&b, bx, by, path_mask);
            let layers = [
                TerrainLayer {
                    triangles: &b.triangles,
                    projection: TerrainProjection::Xy,
                    transform: TerrainTransform {
                        u_scale: 0.05,
                        v_scale: 0.05,
                        ..TerrainTransform::default()
                    },
                    opacity: TerrainOpacity::Opaque,
                    diffuse_color: [1.0, 1.0, 1.0],
                    textures: TerrainLayerTextures {
                        diffuse: Some(grass),
                        ..TerrainLayerTextures::default()
                    },
                    ..TerrainLayer::default()
                },
                TerrainLayer {
                    triangles: &rock_tris,
                    projection: TerrainProjection::Xy,
                    transform: TerrainTransform {
                        u_scale: 0.04,
                        v_scale: 0.04,
                        rotation_degrees: 30.0,
                        offset: [0.25, 0.5],
                    },
                    opacity: TerrainOpacity::MaskedDiffuse {
                        alpha_blend_max: 0.6,
                    },
                    alpha_test_ref: 0.05,
                    diffuse_color: [0.95, 0.92, 0.9],
                    specular: Some(TerrainSpecular {
                        color: [0.5, 0.5, 0.5],
                        power: 18.0,
                        gloss: TerrainGloss::NormalAlpha,
                    }),
                    textures: TerrainLayerTextures {
                        diffuse: Some(rock),
                        normal: Some(bumps),
                        mask: Some(rock_mask_tex),
                        ..TerrainLayerTextures::default()
                    },
                    ..TerrainLayer::default()
                },
                TerrainLayer {
                    triangles: &path_tris,
                    projection: TerrainProjection::Xy,
                    transform: TerrainTransform {
                        u_scale: 0.1,
                        v_scale: 0.1,
                        ..TerrainTransform::default()
                    },
                    opacity: TerrainOpacity::Mask,
                    alpha_test_ref: 0.1,
                    diffuse_color: [1.0, 1.0, 1.0],
                    textures: TerrainLayerTextures {
                        diffuse: Some(sand),
                        mask: Some(path_mask_tex),
                        ..TerrainLayerTextures::default()
                    },
                    ..TerrainLayer::default()
                },
            ];
            let data = TerrainChunkData {
                label: Some("synthetic block"),
                vertices: TerrainVertices {
                    positions: &b.positions,
                    normals: &b.normals,
                    tangents: &b.tangents,
                    binormals: &b.binormals,
                    sets: [&b.sets[0], &b.sets[1], &b.sets[2], &b.sets[3]],
                },
                layers: &layers,
                light_map: ((bx, by) == (1, 1)).then_some(light_map),
            };
            let chunk = r.upload_terrain_chunk(&data)?;
            drawn_layers += layers.iter().filter(|l| !l.triangles.is_empty()).count() as u32;
            chunks.push((chunk, bx, by));
        }
    }

    let target = r.create_offscreen_target(WIDTH, HEIGHT, OffscreenFormat::Rgba8Srgb)?;
    let mut frame = r.begin_offscreen_frame(&target)?;
    {
        let pass = frame
            .world_pass("terrain", &camera())
            .clear(CLEAR)
            .frame_lighting(lighting(Vec3::new(240.0, 240.0, 20.0)));
        for (chunk, bx, by) in &chunks {
            pass.terrain(TerrainDraw::new(
                chunk.clone(),
                Mat4::from_translation(Vec3::new(*bx as f32 * BLOCK, *by as f32 * BLOCK, 0.0)),
            ));
        }
    }
    let stats = r.end_frame(frame)?;
    assert_eq!(stats.terrain_layers, drawn_layers, "{stats:?}");
    assert!(
        drawn_layers > 18,
        "the masks should reach most blocks: {drawn_layers}"
    );
    assert_eq!(stats.shadow_passes, 1);
    assert!(stats.shadow_draws >= 9, "{stats:?}");
    r.read_pixels(&target)
}

#[test]
fn terrain_layers_golden() -> Result<(), RenderError> {
    let Some(mut r) = gpu::headless_renderer(CLEAR) else {
        return Ok(());
    };
    let img = patch_scene(&mut r)?;
    let again = patch_scene(&mut r)?;
    assert!(img.data == again.data, "render is not deterministic");
    golden::check(
        &Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/golden/terrain_layers.png"),
        &img,
        golden::Tolerance::default(),
    );
    Ok(())
}

// ---- pixel checks -------------------------------------------------------------------------

/// A flat 4 × 4-unit chunk seen straight down, filling the target.
struct Flat {
    positions: Vec<[f32; 3]>,
    normals: Vec<[f32; 3]>,
    tangents: Vec<[f32; 3]>,
    binormals: Vec<[f32; 3]>,
    set0: Vec<[f32; 2]>,
    set3: Vec<[f32; 2]>,
    triangles: Vec<[u16; 3]>,
}

fn flat() -> Flat {
    let positions = vec![
        [0.0, 0.0, 0.0],
        [4.0, 0.0, 0.0],
        [0.0, 4.0, 0.0],
        [4.0, 4.0, 0.0],
    ];
    Flat {
        set0: positions.iter().map(|p| [p[0], p[1]]).collect(),
        set3: positions.iter().map(|p| [p[0] / 4.0, p[1] / 4.0]).collect(),
        normals: vec![[0.0, 0.0, 1.0]; 4],
        tangents: vec![[1.0, 0.0, 0.0]; 4],
        binormals: vec![[0.0, 1.0, 0.0]; 4],
        positions,
        triangles: vec![[0, 1, 3], [0, 3, 2]],
    }
}

/// Orthographic top-down view of the 4 × 4 square (x right, y up on screen).
fn top_down() -> Mat4 {
    let proj = tessera_render::glam::camera::rh::proj::directx::orthographic(
        0.0, 4.0, 0.0, 4.0, -10.0, 10.0,
    );
    // Reversed depth: z' = 1 - z.
    Mat4::from_cols_array(&[
        1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, -1.0, 0.0, 0.0, 0.0, 1.0, 1.0,
    ]) * proj
}

/// Ambient 0.5 only: colour = 2 · C · (M · 0.5) = C · M.
fn neutral() -> FrameLighting {
    FrameLighting {
        ambient: [0.5; 3],
        hemispheric: [0.0; 3],
        sun: None,
        ..FrameLighting::default()
    }
}

fn draw_flat(
    r: &mut Renderer<'static>,
    layers: &[TerrainLayer<'_>],
    light_map: Option<TextureId>,
    lighting: FrameLighting,
) -> Result<RgbaImage, RenderError> {
    let chunk = flat_chunk(r, layers, light_map, true)?;
    draw_chunk(r, TerrainDraw::new(chunk, Mat4::IDENTITY), lighting)
}

/// The flat square as a chunk, with or without its tangent frame.
fn flat_chunk(
    r: &mut Renderer<'static>,
    layers: &[TerrainLayer<'_>],
    light_map: Option<TextureId>,
    frame: bool,
) -> Result<TerrainChunk, RenderError> {
    let f = flat();
    let data = TerrainChunkData {
        label: None,
        vertices: TerrainVertices {
            positions: &f.positions,
            normals: &f.normals,
            tangents: if frame { &f.tangents } else { &[] },
            binormals: if frame { &f.binormals } else { &[] },
            sets: [&f.set0, &[], &[], &f.set3],
        },
        layers,
        light_map,
    };
    r.upload_terrain_chunk(&data)
}

/// Draws one chunk top-down into a 16 × 16 target.
fn draw_chunk(
    r: &mut Renderer<'static>,
    draw: TerrainDraw,
    lighting: FrameLighting,
) -> Result<RgbaImage, RenderError> {
    draw_chunk_reach(r, draw, lighting, Vec::new())
}

/// [`draw_chunk`] with the reach of the table lights (`Pass::light_reach`).
fn draw_chunk_reach(
    r: &mut Renderer<'static>,
    draw: TerrainDraw,
    lighting: FrameLighting,
    reach: Vec<LightReach>,
) -> Result<RgbaImage, RenderError> {
    let target = r.create_offscreen_target(16, 16, OffscreenFormat::Rgba8Unorm)?;
    let mut frame = r.begin_offscreen_frame(&target)?;
    frame
        .pass(
            "flat",
            tessera_render::PassSpace::World {
                view_proj: top_down(),
            },
        )
        .eye(Vec3::new(2.0, 2.0, 10.0))
        .clear([0.0, 0.0, 0.0, 1.0])
        .frame_lighting(lighting)
        .light_reach(reach)
        .terrain(draw);
    r.end_frame(frame)?;
    r.read_pixels(&target)
}

/// The flat square with every vertex normal set to `normal` (not normalised).
fn flat_chunk_with_normal(
    r: &mut Renderer<'static>,
    layers: &[TerrainLayer<'_>],
    normal: [f32; 3],
) -> Result<TerrainChunk, RenderError> {
    let f = flat();
    let normals = vec![normal; f.positions.len()];
    let data = TerrainChunkData {
        label: None,
        vertices: TerrainVertices {
            positions: &f.positions,
            normals: &normals,
            tangents: &f.tangents,
            binormals: &f.binormals,
            sets: [&f.set0, &[], &[], &f.set3],
        },
        layers,
        light_map: None,
    };
    r.upload_terrain_chunk(&data)
}

fn solid(r: &mut Renderer<'static>, rgba: [u8; 4]) -> Result<TextureId, RenderError> {
    upload(r, 1, &rgba)
}

/// Left half (x < 2) mask `left`, right half `right`; 2 × 1 texels.
fn halves(r: &mut Renderer<'static>, left: u8, right: u8) -> Result<TextureId, RenderError> {
    r.upload_texture(&TextureUpload {
        color_space: ColorSpace::Linear,
        filter: Filter::Nearest,
        wrap: Wrap::Clamp,
        ..TextureUpload::rgba8(2, 1, &[255, 255, 255, left, 255, 255, 255, right])
    })
}

fn near(px: Option<[u8; 4]>, want: [f32; 3], tol: f32) -> bool {
    px.is_some_and(|p| (0..3).all(|i| (f32::from(p[i]) - want[i] * 255.0).abs() <= tol))
}

#[test]
fn layers_blend_over_by_their_opacity_rule() -> Result<(), RenderError> {
    let Some(mut r) = gpu::headless_renderer(CLEAR) else {
        return Ok(());
    };
    let f = flat();
    let red = solid(&mut r, [255, 0, 0, 255])?;
    // Diffuse alpha 0.5.
    let green = solid(&mut r, [0, 255, 0, 128])?;
    let mask = halves(&mut r, 51, 204)?; // m = 0.2 | 0.8
    let base = TerrainLayer {
        triangles: &f.triangles,
        diffuse_color: [1.0; 3],
        textures: TerrainLayerTextures {
            diffuse: Some(red),
            ..TerrainLayerTextures::default()
        },
        ..TerrainLayer::default()
    };
    let over = |opacity, tau, mask: Option<TextureId>| TerrainLayer {
        opacity,
        alpha_test_ref: tau,
        textures: TerrainLayerTextures {
            diffuse: Some(green),
            mask,
            ..TerrainLayerTextures::default()
        },
        ..base
    };
    let a = 128.0 / 255.0;
    // (rule, τ, mask, expected α on the left, on the right; None = discarded)
    type Case = (
        TerrainOpacity,
        f32,
        Option<TextureId>,
        Option<f32>,
        Option<f32>,
    );
    let cases: [Case; 6] = [
        (TerrainOpacity::Mask, 0.0, Some(mask), Some(0.2), Some(0.8)),
        (TerrainOpacity::Mask, 0.5, Some(mask), None, Some(0.8)),
        (TerrainOpacity::Diffuse, 0.0, None, Some(a), Some(a)),
        (TerrainOpacity::One, 0.0, None, Some(1.0), Some(1.0)),
        (
            TerrainOpacity::MaskedDiffuse {
                alpha_blend_max: 0.25,
            },
            0.0,
            Some(mask),
            Some(a * 0.2 + 0.04 * 0.25),
            Some(a * 0.8 + 0.64 * 0.25),
        ),
        // A mask texture that does not exist reads as 1.
        (TerrainOpacity::Mask, 0.0, None, Some(1.0), Some(1.0)),
    ];
    for (rule, tau, mask, left, right) in cases {
        let img = draw_flat(&mut r, &[base, over(rule, tau, mask)], None, neutral())?;
        for (x, alpha) in [(3, left), (12, right)] {
            let a = alpha.unwrap_or(0.0);
            let want = [1.0 - a, a, 0.0];
            let px = img.pixel(x, 8);
            assert!(
                near(px, want, 3.0),
                "{rule:?} τ={tau} x={x}: {px:?} vs {want:?}"
            );
        }
    }
    Ok(())
}

#[test]
fn light_map_is_added_and_masks_start_in_the_south() -> Result<(), RenderError> {
    let Some(mut r) = gpu::headless_renderer(CLEAR) else {
        return Ok(());
    };
    let f = flat();
    let grey = solid(&mut r, [128, 128, 128, 255])?;
    // Light map: stored row 0 (south) dark blue, row 1 (north) black.
    let light = r.upload_texture(&TextureUpload {
        color_space: ColorSpace::Linear,
        filter: Filter::Nearest,
        wrap: Wrap::Clamp,
        ..TextureUpload::rgba8(1, 2, &[0, 0, 64, 255, 0, 0, 0, 255])
    })?;
    let layer = TerrainLayer {
        triangles: &f.triangles,
        diffuse_color: [1.0; 3],
        textures: TerrainLayerTextures {
            diffuse: Some(grey),
            ..TerrainLayerTextures::default()
        },
        ..TerrainLayer::default()
    };
    let img = draw_flat(&mut r, &[layer], Some(light), neutral())?;
    let c = 128.0 / 255.0;
    // South (bottom of the image): 2 C (0.5 + L) with L = 64/255 in blue.
    let south = img.pixel(8, 13);
    let north = img.pixel(8, 2);
    let lb = 64.0 / 255.0;
    assert!(
        near(south, [c, c, (2.0 * c * (0.5 + lb)).min(1.0)], 3.0),
        "{south:?}"
    );
    assert!(near(north, [c, c, c], 3.0), "{north:?}");
    Ok(())
}

#[test]
fn normal_map_turns_with_the_texture_rotation() -> Result<(), RenderError> {
    let Some(mut r) = gpu::headless_renderer(CLEAR) else {
        return Ok(());
    };
    let f = flat();
    let white = solid(&mut r, [255, 255, 255, 255])?;
    // A texel tilted toward +x in tangent space: n = (0.6, 0, 0.8).
    let tilt = solid(&mut r, [204, 128, 230, 255])?;
    let toward = Vec3::new(1.0, 0.0, 0.0);
    let lighting = |dir: Vec3| FrameLighting {
        ambient: [0.0; 3],
        hemispheric: [0.0; 3],
        sun: Some(SunLight {
            direction: dir,
            color: [0.5; 3],
            specular: [0.0; 3],
        }),
        ..FrameLighting::default()
    };
    let layer = |deg: f32| TerrainLayer {
        triangles: &f.triangles,
        diffuse_color: [1.0; 3],
        transform: TerrainTransform {
            rotation_degrees: deg,
            ..TerrainTransform::default()
        },
        textures: TerrainLayerTextures {
            diffuse: Some(white),
            normal: Some(tilt),
            ..TerrainLayerTextures::default()
        },
        ..TerrainLayer::default()
    };
    let n = Vec3::new(
        204.0 / 255.0 * 2.0 - 1.0,
        128.0 / 255.0 * 2.0 - 1.0,
        230.0 / 255.0 * 2.0 - 1.0,
    )
    .normalize();
    // No rotation: the normal tilts toward +x (= T): lit by a sun from +x.
    let img = draw_flat(&mut r, &[layer(0.0)], None, lighting(toward))?;
    let want = (2.0 * 0.5 * n.x.max(0.0)).min(1.0);
    assert!(
        near(img.pixel(8, 8), [want; 3], 3.0),
        "{:?} vs {want}",
        img.pixel(8, 8)
    );
    // Rotated 90°: c0 = (0, 1), so n' = (n.x c0.x, n.x c0.y) = (0, n.x): along -B = -y
    // (the frame (T, -B, N)), lit by a sun from -y, dark under one from +x.
    let img = draw_flat(
        &mut r,
        &[layer(90.0)],
        None,
        lighting(Vec3::new(0.0, -1.0, 0.0)),
    )?;
    assert!(
        near(img.pixel(8, 8), [want; 3], 3.0),
        "{:?} vs {want}",
        img.pixel(8, 8)
    );
    let img = draw_flat(&mut r, &[layer(90.0)], None, lighting(toward))?;
    assert!(
        near(img.pixel(8, 8), [0.0; 3], 3.0),
        "{:?}",
        img.pixel(8, 8)
    );
    Ok(())
}

#[test]
fn hostile_chunks_and_unlit_passes() -> Result<(), RenderError> {
    let Some(mut r) = gpu::headless_renderer(CLEAR) else {
        return Ok(());
    };
    let f = flat();
    let bad = [[0u16, 1, 7]];
    let layers = [TerrainLayer {
        triangles: &bad,
        ..TerrainLayer::default()
    }];
    let data = TerrainChunkData {
        vertices: TerrainVertices {
            positions: &f.positions,
            normals: &f.normals,
            ..TerrainVertices::default()
        },
        layers: &layers,
        ..TerrainChunkData::default()
    };
    assert!(matches!(
        r.upload_terrain_chunk(&data),
        Err(RenderError::InvalidMesh(_))
    ));
    // Unknown textures and hostile floats draw (white, sanitised) instead of failing.
    let gone = solid(&mut r, [1, 2, 3, 4])?;
    assert!(r.destroy_texture(gone));
    let layers = [TerrainLayer {
        triangles: &f.triangles,
        diffuse_color: [f32::NAN; 3],
        transform: TerrainTransform {
            u_scale: f32::INFINITY,
            ..TerrainTransform::default()
        },
        textures: TerrainLayerTextures {
            diffuse: Some(gone),
            normal: Some(gone),
            mask: Some(gone),
            ..TerrainLayerTextures::default()
        },
        ..TerrainLayer::default()
    }];
    let img = draw_flat(&mut r, &layers, Some(gone), neutral())?;
    assert!(
        near(img.pixel(8, 8), [1.0; 3], 3.0),
        "{:?}",
        img.pixel(8, 8)
    );
    // A pass without frame lighting skips terrain (with a warning) and draws the rest.
    let chunk = r.upload_terrain_chunk(&TerrainChunkData {
        layers: &layers,
        ..TerrainChunkData {
            vertices: TerrainVertices {
                positions: &f.positions,
                normals: &f.normals,
                ..TerrainVertices::default()
            },
            ..data
        }
    })?;
    let target = r.create_offscreen_target(8, 8, OffscreenFormat::Rgba8Unorm)?;
    let mut frame = r.begin_offscreen_frame(&target)?;
    frame
        .world_pass("unlit", &Camera::default())
        .clear([0.0, 0.0, 1.0, 1.0])
        .terrain(TerrainDraw::new(chunk.clone(), Mat4::IDENTITY));
    let stats = r.end_frame(frame)?;
    assert_eq!(stats.terrain_layers, 0);
    assert_eq!(r.read_pixels(&target)?.pixel(4, 4), Some([0, 0, 255, 255]));
    // A chunk of another renderer is skipped.
    if let Some(mut other) = gpu::headless_renderer(CLEAR) {
        let target = other.create_offscreen_target(8, 8, OffscreenFormat::Rgba8Unorm)?;
        let mut frame = other.begin_offscreen_frame(&target)?;
        frame
            .world_pass("foreign", &Camera::default())
            .frame_lighting(neutral())
            .terrain(TerrainDraw::new(chunk, Mat4::IDENTITY));
        let stats = other.end_frame(frame)?;
        assert_eq!(stats.terrain_layers, 0);
    }
    Ok(())
}

// ---- real data ----------------------------------------------------------------------------

mod real {
    //! Real landblocks of the user's install (read-only, `TESSERA_GAME_DIR`): resolved by
    //! `tessera-terrain` and drawn top-down through the layer pipeline. Only counts are
    //! printed; nothing read from the install is written anywhere.

    use std::collections::{BTreeMap, HashMap};

    use tessera_dat::{ArchiveKind, ArchiveSet, SetOptions};
    use tessera_formats::landblock::{GridSurface, LightMaps, REGION_DESCRIPTOR_PREFIX};
    use tessera_formats::terrain_type::TerrainTypeTable;
    use tessera_formats::{Did, LandblockData, Record, RegionDescriptor, Surface, TextureList};
    use tessera_render::glam::{Mat4, Vec3};
    use tessera_render::{
        ColorSpace, Filter, FrameLighting, MipChainUpload, MipLevel, OffscreenFormat, PassSpace,
        RenderError, Renderer, TerrainChunkData, TerrainDraw, TerrainGloss, TerrainLayer,
        TerrainLayerTextures, TerrainOpacity, TerrainProjection, TerrainSpecular, TerrainTransform,
        TerrainVertices, TextureId, TextureUpload, Wrap,
    };
    use tessera_terrain::{
        CoordinateSet, LayerVertices, Neighbours, OpacityRule, ResolveOptions, TerrainBlock,
        VertexNormals, fixed_block_offset, resolve_landblock,
    };

    use super::gpu;

    /// Largest texture uploaded (texels of level 0).
    const MAX_TEXELS: u64 = 1024 * 1024;

    struct Textures<'s> {
        set: &'s ArchiveSet,
        cache: HashMap<u32, Option<TextureId>>,
        missing: usize,
    }

    impl Textures<'_> {
        /// A 0x40 texture list's active chain (all stored levels), uploaded with mips.
        fn list(&mut self, r: &mut Renderer<'static>, did: Option<Did>) -> Option<TextureId> {
            let did = did?;
            if let Some(t) = self.cache.get(&did.0) {
                return *t;
            }
            let set = self.set;
            let id = (|| {
                let bytes = set.read(did.0).ok()?;
                let list = TextureList::parse_expecting(&bytes, did.0).ok()?;
                let chain = list.effective_chain(|d| {
                    let b = set.read(d.0).ok()?;
                    Surface::parse_expecting(&b, d.0)
                        .and_then(|s| s.to_rgba8())
                        .ok()
                });
                let first = chain.first()?;
                if u64::from(first.level.width) * u64::from(first.level.height) > MAX_TEXELS {
                    return None;
                }
                let levels: Vec<MipLevel<'_>> = chain
                    .iter()
                    .map(|c| MipLevel {
                        width: c.level.width,
                        height: c.level.height,
                        rgba: &c.level.pixels,
                    })
                    .collect();
                r.upload_texture_mips(&MipChainUpload {
                    color_space: ColorSpace::Linear,
                    ..MipChainUpload::new(&levels)
                })
                .ok()
            })();
            self.missing += usize::from(id.is_none());
            self.cache.insert(did.0, id);
            id
        }

        /// A 0x41 surface (an A8 mask decodes to (255, 255, 255, a)).
        fn surface(&mut self, r: &mut Renderer<'static>, did: Option<Did>) -> Option<TextureId> {
            let did = did?;
            if let Some(t) = self.cache.get(&did.0) {
                return *t;
            }
            let id = (|| {
                let b = self.set.read(did.0).ok()?;
                let img = Surface::parse_expecting(&b, did.0)
                    .and_then(|s| s.to_rgba8())
                    .ok()?;
                r.upload_texture(&TextureUpload {
                    color_space: ColorSpace::Linear,
                    filter: Filter::Linear,
                    wrap: Wrap::Clamp,
                    ..TextureUpload::rgba8(img.width, img.height, &img.pixels)
                })
                .ok()
            })();
            self.missing += usize::from(id.is_none());
            self.cache.insert(did.0, id);
            id
        }
    }

    fn region_tables(set: &ArchiveSet) -> BTreeMap<u32, TerrainTypeTable> {
        let mut out = BTreeMap::new();
        let lo = u32::from(REGION_DESCRIPTOR_PREFIX) << 24;
        for m in set.members() {
            for e in m.archive.entries_range(lo..=lo | 0x00FF_FFFF).flatten() {
                let Ok(payload) = m.archive.read_entry(&e) else {
                    continue;
                };
                let Ok(d) = RegionDescriptor::parse_expecting(&payload, e.did) else {
                    continue;
                };
                let Some(tdid) = d.terrain_type_table() else {
                    continue;
                };
                let Ok(bytes) = set.read(tdid.0) else {
                    continue;
                };
                if let Ok(t) = TerrainTypeTable::parse_expecting(&bytes, tdid) {
                    out.entry(d.region).or_insert(t);
                }
            }
        }
        out
    }

    #[test]
    fn real_landblocks_draw_through_the_layer_pipeline() -> Result<(), RenderError> {
        let Some(dir) = std::env::var_os("TESSERA_GAME_DIR").filter(|d| !d.is_empty()) else {
            eprintln!("skipping: TESSERA_GAME_DIR is not set");
            return Ok(());
        };
        let Some(mut r) = gpu::headless_renderer([0.0, 0.0, 0.0, 1.0]) else {
            return Ok(());
        };
        let set = ArchiveSet::open(&dir, SetOptions::default()).expect("open archive set");
        let tables = region_tables(&set);
        let want = if cfg!(debug_assertions) { 24 } else { 240 };
        let mut tex = Textures {
            set: &set,
            cache: HashMap::new(),
            missing: 0,
        };
        let target = r.create_offscreen_target(64, 64, OffscreenFormat::Rgba8Unorm)?;
        let (mut blocks, mut layers, mut masked, mut normal_mapped) = (0usize, 0u32, 0, 0);
        let (mut hole_px, mut hole_tris) = (0usize, 0usize);
        'regions: for (i, m) in set.members().iter().enumerate() {
            let ArchiveKind::Cell(region) = m.id.kind else {
                continue;
            };
            let Some(table) = tables.get(&region) else {
                continue;
            };
            let entries: Vec<_> = m
                .archive
                .entries_range(0x8000_0000..=0x800F_FFFF)
                .flatten()
                .collect();
            // Spread the sample over the region.
            let stride = (entries.len() / want.max(1)).max(1);
            for e in entries.iter().step_by(stride) {
                if blocks >= want {
                    break 'regions;
                }
                let Ok(payload) = set.member(i).archive.read_entry(e) else {
                    continue;
                };
                let Ok(data) = LandblockData::parse_expecting(&payload, e.did, Some(e.stamp))
                else {
                    continue;
                };
                let Ok(chunk) = resolve_landblock(
                    &data,
                    GridSurface::Terrain,
                    table,
                    &LightMaps::default(),
                    &ResolveOptions::default(),
                ) else {
                    continue;
                };
                if chunk.layers().is_empty() {
                    continue;
                }
                let Some(lb) = data.landblock() else { continue };
                let block = TerrainBlock::from_landblock(&data);
                let normals = VertexNormals::compute(&block, &Neighbours::NONE);
                let v = LayerVertices::build(&block, &normals, fixed_block_offset(lb.x, lb.y));
                let mut out = Vec::with_capacity(chunk.layers().len());
                for l in chunk.layers() {
                    let p = l.params();
                    let f = l.features();
                    let t = l.textures();
                    let opacity = match l.splat().opacity {
                        OpacityRule::Opaque => TerrainOpacity::Opaque,
                        OpacityRule::MaskedDiffuse { alpha_blend_max } => {
                            TerrainOpacity::MaskedDiffuse { alpha_blend_max }
                        }
                        OpacityRule::Mask => TerrainOpacity::Mask,
                        OpacityRule::Diffuse => TerrainOpacity::Diffuse,
                        OpacityRule::One => TerrainOpacity::One,
                    };
                    let projection = match l.coordinate_set() {
                        CoordinateSet::Xz => TerrainProjection::Xz,
                        CoordinateSet::Yz => TerrainProjection::Yz,
                        _ => TerrainProjection::Xy,
                    };
                    let textures = TerrainLayerTextures {
                        diffuse: tex.list(&mut r, t.diffuse),
                        normal: tex.list(&mut r, t.normal),
                        emissive: tex.list(&mut r, t.emissive),
                        glow: tex.list(&mut r, t.glow),
                        mask: tex.surface(&mut r, t.mask),
                    };
                    masked += usize::from(textures.mask.is_some());
                    normal_mapped += usize::from(textures.normal.is_some());
                    let rgb = |c: [f32; 4]| [c[0], c[1], c[2]];
                    out.push(TerrainLayer {
                        triangles: l.triangles(),
                        projection,
                        transform: TerrainTransform {
                            u_scale: p.transform.u_scale,
                            v_scale: p.transform.v_scale,
                            rotation_degrees: p.transform.rotation_degrees,
                            offset: p.offsets.constant().unwrap_or([0.0; 2]),
                        },
                        opacity,
                        alpha_test_ref: p.alpha_test_ref,
                        diffuse_color: rgb(p.diffuse_colour),
                        specular: f.specular.then_some(TerrainSpecular {
                            color: rgb(p.specular_colour),
                            power: p.specular_power,
                            gloss: if f.gloss_from_normal {
                                TerrainGloss::NormalAlpha
                            } else {
                                TerrainGloss::DiffuseAlpha
                            },
                        }),
                        glow_color: rgb(p.glow_colour),
                        textures,
                    });
                }
                let sets = [
                    v.set(CoordinateSet::Xy),
                    v.set(CoordinateSet::Xz),
                    v.set(CoordinateSet::Yz),
                    v.light_map_uvs(),
                ];
                let gpu_chunk = r.upload_terrain_chunk(&TerrainChunkData {
                    label: None,
                    vertices: TerrainVertices {
                        positions: &v.positions,
                        normals: &v.normals,
                        tangents: &v.tangents,
                        binormals: &v.binormals,
                        sets,
                    },
                    layers: &out,
                    light_map: None,
                })?;
                let drawn = out.iter().filter(|l| !l.triangles.is_empty()).count() as u32;
                // Top-down over the block (x 0..160 east, y 0..160 north), all heights.
                let proj = tessera_render::glam::camera::rh::proj::directx::orthographic(
                    0.0, 160.0, 0.0, 160.0, -5_000.0, 5_000.0,
                );
                let flip = Mat4::from_cols_array(&[
                    1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, -1.0, 0.0, 0.0, 0.0, 1.0, 1.0,
                ]);
                let mut frame = r.begin_offscreen_frame(&target)?;
                frame
                    .pass(
                        "real",
                        PassSpace::World {
                            view_proj: flip * proj,
                        },
                    )
                    .eye(Vec3::new(80.0, 80.0, 4_000.0))
                    .clear([1.0, 0.0, 1.0, 1.0])
                    .frame_lighting(FrameLighting::default())
                    .terrain(TerrainDraw::new(gpu_chunk, Mat4::IDENTITY));
                let stats = r.end_frame(frame)?;
                assert_eq!(stats.terrain_layers, drawn, "{:#010x}", e.did);
                let img = r.read_pixels(&target)?;
                let clear = img
                    .data
                    .as_chunks::<4>()
                    .0
                    .iter()
                    .filter(|p| **p == [255, 0, 255, 255])
                    .count();
                // Pixels left uncovered can only come from uncovered (hole) triangles.
                let holes = chunk.uncovered().len();
                if holes == 0 {
                    assert!(clear <= 4, "{:#010x}: {clear} uncovered pixels", e.did);
                }
                hole_px += clear;
                hole_tris += holes;
                blocks += 1;
                layers += drawn;
            }
        }
        eprintln!(
            "real terrain: {blocks} landblocks, {layers} layers drawn ({masked} with masks, \
             {normal_mapped} with normal maps), {} textures, {} unavailable, {hole_tris} hole \
             triangles, {hole_px} uncovered pixels",
            tex.cache.len(),
            tex.missing
        );
        assert!(blocks > 0, "no landblock with layers found");
        Ok(())
    }
}

#[test]
fn unresolved_masks_and_gloss_follow_the_switches() -> Result<(), RenderError> {
    let Some(mut r) = gpu::headless_renderer(CLEAR) else {
        return Ok(());
    };
    let f = flat();
    let black = solid(&mut r, [0, 0, 0, 255])?;
    // White, diffuse alpha 0.5.
    let white_half = solid(&mut r, [255, 255, 255, 128])?;
    let gone = solid(&mut r, [9, 9, 9, 9])?;
    assert!(r.destroy_texture(gone));
    let base = TerrainLayer {
        triangles: &f.triangles,
        diffuse_color: [1.0; 3],
        textures: TerrainLayerTextures {
            diffuse: Some(black),
            ..TerrainLayerTextures::default()
        },
        ..TerrainLayer::default()
    };
    let over = |opacity, mask: Option<TextureId>| TerrainLayer {
        opacity,
        textures: TerrainLayerTextures {
            diffuse: Some(white_half),
            mask,
            ..TerrainLayerTextures::default()
        },
        ..base
    };
    let a = 128.0 / 255.0;
    // §5.1 Tessera Decision: a mask that does not resolve draws the layer as if it had no
    // mask (§7: α = a with use_opacity_mask, else 1).
    let masked = TerrainOpacity::MaskedDiffuse {
        alpha_blend_max: 0.25,
    };
    for (rule, want) in [(masked, a), (TerrainOpacity::Mask, 1.0)] {
        let img = draw_flat(&mut r, &[base, over(rule, Some(gone))], None, neutral())?;
        assert!(
            near(img.pixel(8, 8), [want; 3], 3.0),
            "{rule:?}: {:?} vs {want}",
            img.pixel(8, 8)
        );
    }

    // §8.1: gloss from the normal needs the normal texture; without it the specular switch
    // stays on only when the opacity switch is off.
    let sun = FrameLighting {
        ambient: [0.0; 3],
        hemispheric: [0.0; 3],
        sun: Some(SunLight {
            direction: Vec3::Z,
            color: [0.25; 3],
            specular: [1.0; 3],
        }),
        ..FrameLighting::default()
    };
    let spec = TerrainSpecular {
        color: [1.0; 3],
        power: 1.0,
        gloss: TerrainGloss::NormalAlpha,
    };
    let lit = |specular, normal| TerrainLayer {
        specular,
        textures: TerrainLayerTextures {
            normal,
            ..over(TerrainOpacity::Diffuse, None).textures
        },
        ..over(TerrainOpacity::Diffuse, None)
    };
    let with = draw_flat(
        &mut r,
        &[base, lit(Some(spec), Some(gone))],
        None,
        sun.clone(),
    )?;
    let without = draw_flat(&mut r, &[base, lit(None, None)], None, sun.clone())?;
    assert!(
        near(
            with.pixel(8, 8),
            without
                .pixel(8, 8)
                .map_or([0.0; 3], |p| p.map(|c| f32::from(c) / 255.0)[..3]
                    .try_into()
                    .unwrap_or([0.0; 3])),
            2.0
        ),
        "{:?} vs {:?}",
        with.pixel(8, 8),
        without.pixel(8, 8)
    );
    // A chunk without tangent streams still takes the gloss from the normal texture's alpha
    // (the frame only removes the normal's effect, §6.4): a flat normal with alpha 0 gives
    // no highlight on an opaque first layer whose diffuse alpha is 1.
    let white = solid(&mut r, [255, 255, 255, 255])?;
    let flat_normal = solid(&mut r, [128, 128, 255, 0])?;
    let first = |specular| TerrainLayer {
        specular,
        textures: TerrainLayerTextures {
            diffuse: Some(white),
            normal: Some(flat_normal),
            ..TerrainLayerTextures::default()
        },
        ..base
    };
    for frame in [true, false] {
        let chunk = flat_chunk(&mut r, &[first(Some(spec))], None, frame)?;
        let img = draw_chunk(&mut r, TerrainDraw::new(chunk, Mat4::IDENTITY), sun.clone())?;
        // Diffuse only: 2 · 1 · 0.25 = 0.5.
        assert!(
            near(img.pixel(8, 8), [0.5; 3], 3.0),
            "frame {frame}: {:?}",
            img.pixel(8, 8)
        );
    }
    Ok(())
}

/// `terrain-texturing.md` §4.10, §9.1: the ambient (hemispheric) term and the sun's
/// N · L₀ take the stored vertex normal without normalising it; a ceiling's downward normal
/// receives A₀ − H₀ and no sun.
#[test]
fn stored_normals_shade_as_they_are() -> Result<(), RenderError> {
    let Some(mut r) = gpu::headless_renderer(CLEAR) else {
        return Ok(());
    };
    let f = flat();
    let grey = solid(&mut r, [128, 128, 128, 255])?;
    let layer = TerrainLayer {
        triangles: &f.triangles,
        diffuse_color: [1.0; 3],
        textures: TerrainLayerTextures {
            diffuse: Some(grey),
            ..TerrainLayerTextures::default()
        },
        ..TerrainLayer::default()
    };
    let lighting = FrameLighting {
        ambient: [0.25; 3],
        hemispheric: [0.125; 3],
        sun: Some(SunLight {
            direction: Vec3::Z,
            color: [0.25; 3],
            specular: [1.0; 3],
        }),
        ..FrameLighting::default()
    };
    let c = 128.0 / 255.0;
    // (stored normal, Σ = sat(A₀ + (N · U) H₀) + sat(N · L₀) E₀)
    let cases: [([f32; 3], f32); 4] = [
        ([0.0, 0.0, 1.0], 0.375 + 0.25),
        // Length 0.5: both terms see N · U = N · L₀ = 0.5.
        ([0.0, 0.0, 0.5], 0.3125 + 0.125),
        // A ceiling normal (down): A₀ − H₀, the sun from above adds nothing.
        ([0.0, 0.0, -1.0], 0.125),
        ([0.0, 0.0, -0.8], 0.25 - 0.1),
    ];
    for (normal, sigma) in cases {
        let chunk = flat_chunk_with_normal(&mut r, &[layer], normal)?;
        let img = draw_chunk(
            &mut r,
            TerrainDraw::new(chunk, Mat4::IDENTITY),
            lighting.clone(),
        )?;
        let want = 2.0 * c * sigma;
        assert!(
            near(img.pixel(8, 8), [want; 3], 2.0),
            "normal {normal:?}: {:?} vs {want}",
            img.pixel(8, 8)
        );
    }
    // A uniform scale of the chunk keeps the stored length (the normal matrix is scaled to
    // determinant 1).
    let chunk = flat_chunk_with_normal(&mut r, &[layer], [0.0, 0.0, 0.5])?;
    let scaled = Mat4::from_scale(Vec3::splat(0.5));
    let target = r.create_offscreen_target(16, 16, OffscreenFormat::Rgba8Unorm)?;
    let mut frame = r.begin_offscreen_frame(&target)?;
    frame
        .pass(
            "scaled",
            tessera_render::PassSpace::World {
                view_proj: top_down(),
            },
        )
        .eye(Vec3::new(2.0, 2.0, 10.0))
        .clear([0.0, 0.0, 0.0, 1.0])
        .frame_lighting(lighting)
        .terrain(TerrainDraw::new(chunk, scaled));
    r.end_frame(frame)?;
    let img = r.read_pixels(&target)?;
    let want = 2.0 * c * (0.3125 + 0.125);
    assert!(
        near(img.pixel(4, 12), [want; 3], 2.0),
        "{:?}",
        img.pixel(4, 12)
    );
    Ok(())
}

/// `terrain-texturing.md` §9.4 and row L5: the highlight is g · sat(n · H)^p · F₀ · S,
/// added after the factor 2 and not multiplied by the albedo; a diffuse alpha of 0.09
/// gives a sheen of about 0.09 at the centre of the reflection.
#[test]
fn specular_is_a_sheen_scaled_by_the_gloss() -> Result<(), RenderError> {
    let Some(mut r) = gpu::headless_renderer(CLEAR) else {
        return Ok(());
    };
    let f = flat();
    // C = 0.2, diffuse alpha (gloss) 0.09.
    let paving = solid(&mut r, [51, 51, 51, 23])?;
    let lighting = FrameLighting {
        ambient: [0.1; 3],
        hemispheric: [0.0; 3],
        // The sun straight above and the eye above the centre: n · H = 1 there.
        sun: Some(SunLight {
            direction: Vec3::Z,
            color: [0.2; 3],
            specular: [1.0; 3],
        }),
        ..FrameLighting::default()
    };
    let layer = |specular| TerrainLayer {
        triangles: &f.triangles,
        diffuse_color: [1.0; 3],
        specular,
        textures: TerrainLayerTextures {
            diffuse: Some(paving),
            ..TerrainLayerTextures::default()
        },
        ..TerrainLayer::default()
    };
    let spec = TerrainSpecular {
        color: [1.0; 3],
        power: 8.0,
        gloss: TerrainGloss::DiffuseAlpha,
    };
    let c = 51.0 / 255.0;
    let g = 23.0 / 255.0;
    let diffuse = 2.0 * c * (0.1 + 0.2);
    let plain = draw_flat(&mut r, &[layer(None)], None, lighting.clone())?;
    assert!(
        near(plain.pixel(8, 8), [diffuse; 3], 2.0),
        "{:?}",
        plain.pixel(8, 8)
    );
    let lit = draw_flat(&mut r, &[layer(Some(spec))], None, lighting.clone())?;
    assert!(
        near(lit.pixel(8, 8), [diffuse + g; 3], 2.0),
        "{:?} vs {}",
        lit.pixel(8, 8),
        diffuse + g
    );
    // The specular colour (2001) tints it; the light's specular colour scales it.
    let tinted = TerrainSpecular {
        color: [1.0, 0.0, 0.5],
        ..spec
    };
    let img = draw_flat(&mut r, &[layer(Some(tinted))], None, lighting)?;
    assert!(
        near(
            img.pixel(8, 8),
            [diffuse + g, diffuse, diffuse + 0.5 * g],
            2.0
        ),
        "{:?}",
        img.pixel(8, 8)
    );
    Ok(())
}

/// `lighting.md` §8.3, §8.5 (L30, L31): lights flagged as baked into the landscape are
/// never in a chunk's automatic light list; other lights reach a chunk when the squared
/// distance to its box is below ρ².
#[test]
fn baked_lights_skip_terrain_chunks() -> Result<(), RenderError> {
    let Some(mut r) = gpu::headless_renderer(CLEAR) else {
        return Ok(());
    };
    let f = flat();
    let white = solid(&mut r, [255, 255, 255, 255])?;
    let layer = TerrainLayer {
        triangles: &f.triangles,
        diffuse_color: [1.0; 3],
        textures: TerrainLayerTextures {
            diffuse: Some(white),
            ..TerrainLayerTextures::default()
        },
        ..TerrainLayer::default()
    };
    // One point light 1 m above the centre, range 10, no attenuation, colour 0.25.
    let lighting = FrameLighting {
        ambient: [0.0; 3],
        hemispheric: [0.0; 3],
        sun: None,
        lights: vec![Light::point(Vec3::new(2.0, 2.0, 1.0), 10.0, 0.0, [0.25; 3])],
        ..FrameLighting::default()
    };
    let lit = 2.0 * 0.25;
    let baked = LightReach {
        baked_landscape: true,
        ..LightReach::default()
    };
    let near_reach = LightReach {
        radius: Some(1.5),
        ..LightReach::default()
    };
    let far_reach = LightReach {
        radius: Some(1.0),
        ..LightReach::default()
    };
    // (reach, selection, expected at the centre)
    let cases = [
        (Vec::new(), TerrainDraw::new, lit),
        (vec![baked], TerrainDraw::new, 0.0),
        // ρ = 1.5 > 1 m above the box; ρ = 1 is not below the distance (strict test).
        (vec![near_reach], TerrainDraw::new, lit),
        (vec![far_reach], TerrainDraw::new, 0.0),
    ];
    for (reach, new, want) in cases {
        let chunk = flat_chunk(&mut r, &[layer], None, true)?;
        let img = draw_chunk_reach(
            &mut r,
            new(chunk, Mat4::IDENTITY),
            lighting.clone(),
            reach.clone(),
        )?;
        assert!(
            near(img.pixel(8, 8), [want; 3], 2.0),
            "{reach:?}: {:?} vs {want}",
            img.pixel(8, 8)
        );
    }
    // An explicit list is the caller's choice and is kept.
    let chunk = flat_chunk(&mut r, &[layer], None, true)?;
    let mut draw = TerrainDraw::new(chunk, Mat4::IDENTITY);
    draw.lights = tessera_render::LightSelection::List {
        indices: [0; 8],
        count: 1,
    };
    let img = draw_chunk_reach(&mut r, draw, lighting, vec![baked])?;
    assert!(
        near(img.pixel(8, 8), [lit; 3], 2.0),
        "{:?}",
        img.pixel(8, 8)
    );
    Ok(())
}

#[test]
fn destroyed_textures_and_scrolling_offsets() -> Result<(), RenderError> {
    let Some(mut r) = gpu::headless_renderer(CLEAR) else {
        return Ok(());
    };
    let f = flat();
    let red = solid(&mut r, [255, 0, 0, 255])?;
    let light = solid(&mut r, [0, 0, 128, 255])?;
    let layer = TerrainLayer {
        triangles: &f.triangles,
        diffuse_color: [1.0; 3],
        textures: TerrainLayerTextures {
            diffuse: Some(red),
            ..TerrainLayerTextures::default()
        },
        ..TerrainLayer::default()
    };
    let chunk = flat_chunk(&mut r, &[layer], Some(light), true)?;
    let draw = TerrainDraw::new(chunk.clone(), Mat4::IDENTITY);
    let img = draw_chunk(&mut r, draw.clone(), neutral())?;
    assert!(
        near(img.pixel(8, 8), [1.0, 0.0, 0.0], 3.0),
        "{:?}",
        img.pixel(8, 8)
    );
    // The chunk's textures are destroyed while it is still drawn: the frame succeeds (a
    // destroyed texture would fail the whole submission) and the layer draws as if they
    // were missing: white diffuse, no light map.
    assert!(r.destroy_texture(red));
    assert!(r.destroy_texture(light));
    let img = draw_chunk(&mut r, draw.clone(), neutral())?;
    assert!(
        near(img.pixel(8, 8), [1.0; 3], 3.0),
        "{:?}",
        img.pixel(8, 8)
    );

    // Scrolling offsets: a texture black on the left half, white on the right, over u
    // 0..1 (eight texels, so the bilinear layer sampler blurs only at the edges).
    let row: Vec<u8> = (0..8u8)
        .flat_map(|i| if i < 4 { [0, 0, 0, 255] } else { [255; 4] })
        .collect();
    let stripes = r.upload_texture(&TextureUpload {
        color_space: ColorSpace::Linear,
        filter: Filter::Nearest,
        wrap: Wrap::Repeat,
        ..TextureUpload::rgba8(8, 1, &row)
    })?;
    let scroll = TerrainLayer {
        transform: TerrainTransform {
            u_scale: 0.25,
            v_scale: 0.25,
            ..TerrainTransform::default()
        },
        textures: TerrainLayerTextures {
            diffuse: Some(stripes),
            ..TerrainLayerTextures::default()
        },
        ..layer
    };
    let chunk = flat_chunk(&mut r, &[scroll, scroll], None, true)?;
    let left = |img: &RgbaImage| img.pixel(3, 8);
    let img = draw_chunk(
        &mut r,
        TerrainDraw::new(chunk.clone(), Mat4::IDENTITY),
        neutral(),
    )?;
    assert!(near(left(&img), [0.0; 3], 3.0), "{:?}", left(&img));
    // Both layers shifted by half a repeat: the left turns white, and stays so in later
    // frames without offsets.
    let shifted = TerrainDraw {
        layer_offsets: Some(vec![[0.5, 0.0]; 2].into()),
        ..TerrainDraw::new(chunk.clone(), Mat4::IDENTITY)
    };
    for draw in [shifted, TerrainDraw::new(chunk.clone(), Mat4::IDENTITY)] {
        let img = draw_chunk(&mut r, draw, neutral())?;
        assert!(near(left(&img), [1.0; 3], 3.0), "{:?}", left(&img));
    }
    // A list of another length applies to the layers it covers and does not fail.
    for offsets in [vec![], vec![[0.0, 0.0]; 5]] {
        let draw = TerrainDraw {
            layer_offsets: Some(offsets.into()),
            ..TerrainDraw::new(chunk.clone(), Mat4::IDENTITY)
        };
        draw_chunk(&mut r, draw, neutral())?;
    }
    Ok(())
}
