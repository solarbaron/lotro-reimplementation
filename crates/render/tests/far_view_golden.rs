// SPDX-License-Identifier: GPL-3.0-or-later
//! The normal-mapped far-terrain template and the near/far seam overlay
//! (`docs/specs/far-view.md` §6.4, §6.5, worked example §14.4):
//!
//! - `far_view_seam.png`: a 600 × 600-unit patch seen from straight above with the eye at its
//!   centre. An opaque "near terrain" (a plain-template draw) is covered by the overlay, a
//!   normal-mapped draw of the same triangles: invisible within 80 units of the eye, fading
//!   in to full at 160 units on level ground (left half) and to 0.81 on the slope normals of
//!   the right half (*n*_up = 0.9), and full everywhere outside the near-view square (the
//!   north and east strips; mask *m* = 1). The tiled normal map's relief fades in from 80 to
//!   240 units (normal strength *t*).
//! - pixel checks of the coverage *w* and the normal strength *t* against the spec's
//!   formulas and the numbers of §14.4 (*h* = 120 and 200 units, *n*_up = 0.9);
//! - the overlay's premise: drawn through the far-terrain program over a chunk of the
//!   terrain layer pipeline with the same triangles and transform, it has the same depth
//!   (no z-fail, no z-fight) under a perspective camera, so at full coverage it hides the
//!   chunk at every pixel, and still loses to a surface half a unit in front of it.
//!
//! All data is generated here (no game data). Regenerate with
//! `TESSERA_BLESS=1 cargo test -p tessera-render --test far_view_golden`.

#[path = "support/golden.rs"]
mod golden;
#[path = "support/gpu.rs"]
mod gpu;

use std::path::Path;

use tessera_render::glam::{Mat4, Vec3};
use tessera_render::{
    Camera, ColorSpace, FarTerrainData, FarTerrainDraw, FarTerrainMesh, Filter, FrameLighting,
    OffscreenFormat, PassSpace, Perspective, RenderError, Renderer, RgbaImage, SunLight,
    TerrainChunkData, TerrainDraw, TerrainLayer, TerrainLayerTextures, TerrainVertices, TextureId,
    TextureUpload, Wrap,
};

const CLEAR: [f32; 4] = [0.55, 0.62, 0.72, 1.0];

/// The slope normal of §14.4: *n*_up = 0.9 (unit length, tilted towards +x).
const SLOPE: [f32; 3] = [0.435_889_9, 0.0, 0.9];

/// Orthographic top-down view of x ∈ [`l`, `r`], y ∈ [`b`, `t`] (x right, y up on screen),
/// reversed depth like the world passes.
fn top_down(l: f32, r: f32, b: f32, t: f32) -> Mat4 {
    let proj =
        tessera_render::glam::camera::rh::proj::directx::orthographic(l, r, b, t, -100.0, 100.0);
    // Reversed depth: z' = 1 - z.
    Mat4::from_cols_array(&[
        1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, -1.0, 0.0, 0.0, 0.0, 1.0, 1.0,
    ]) * proj
}

fn texture(
    r: &mut Renderer<'static>,
    size: u32,
    wrap: Wrap,
    f: impl Fn(u32, u32) -> [u8; 4],
) -> Result<TextureId, RenderError> {
    let data: Vec<u8> = (0..size * size)
        .flat_map(|i| f(i % size, i / size))
        .collect();
    r.upload_texture(&TextureUpload {
        color_space: ColorSpace::Linear,
        filter: Filter::Linear,
        wrap,
        ..TextureUpload::rgba8(size, size, &data)
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

/// A grid of `nx` × `ny` cells of `step` units from (`x0`, `y0`), at height 0, with
/// per-vertex normals `normal(x, y)` and colour coordinates over the whole grid.
fn grid(
    r: &mut Renderer<'static>,
    origin: [f32; 2],
    cells: [u32; 2],
    step: f32,
    normal: impl Fn(f32, f32) -> [f32; 3],
) -> Result<FarTerrainMesh, RenderError> {
    let (nx, ny) = (cells[0], cells[1]);
    let mut positions = Vec::new();
    let mut normals = Vec::new();
    let mut uvs = Vec::new();
    for j in 0..=ny {
        for i in 0..=nx {
            let x = origin[0] + i as f32 * step;
            let y = origin[1] + j as f32 * step;
            positions.push([x, y, 0.0]);
            normals.push(normal(x, y));
            uvs.push([i as f32 / nx as f32, 1.0 - j as f32 / ny as f32]);
        }
    }
    let mut indices = Vec::new();
    let at = |i: u32, j: u32| j * (nx + 1) + i;
    for j in 0..ny {
        for i in 0..nx {
            // Counter-clockwise seen from above (front faces; the grid culls back faces).
            indices.extend([at(i, j), at(i + 1, j), at(i + 1, j + 1)]);
            indices.extend([at(i, j), at(i + 1, j + 1), at(i, j + 1)]);
        }
    }
    r.upload_far_terrain(&FarTerrainData {
        label: "synthetic far terrain",
        positions: &positions,
        normals: &normals,
        uvs: &uvs,
        indices: &indices,
        stitch_start: indices.len(),
    })
}

/// Draws `draws` (in order) top-down into a `w` × `h` target over the given rectangle with
/// the eye above (`eye_x`, `eye_y`).
#[allow(clippy::too_many_arguments)]
fn draw(
    r: &mut Renderer<'static>,
    size: [u32; 2],
    rect: [f32; 4],
    eye: Vec3,
    clear: [f32; 4],
    format: OffscreenFormat,
    lighting: FrameLighting,
    draws: Vec<FarTerrainDraw>,
) -> Result<RgbaImage, RenderError> {
    let target = r.create_offscreen_target(size[0], size[1], format)?;
    let mut frame = r.begin_offscreen_frame(&target)?;
    {
        let pass = frame
            .pass(
                "far view",
                PassSpace::World {
                    view_proj: top_down(rect[0], rect[1], rect[2], rect[3]),
                },
            )
            .eye(eye)
            .clear(clear)
            .frame_lighting(lighting);
        for d in draws {
            pass.far_terrain(d);
        }
    }
    r.end_frame(frame)?;
    r.read_pixels(&target)
}

fn sun_lighting() -> FrameLighting {
    FrameLighting {
        ambient: [0.25, 0.26, 0.28],
        hemispheric: [0.08, 0.08, 0.06],
        sun: Some(SunLight {
            direction: Vec3::new(0.6, -0.3, 0.75),
            color: [0.75, 0.72, 0.62],
            specular: [0.0; 3],
        }),
        ..FrameLighting::default()
    }
}

fn seam_scene(r: &mut Renderer<'static>) -> Result<RgbaImage, RenderError> {
    // The "near terrain" under the overlay: plain template, opaque green.
    let green = texture(r, 1, Wrap::Clamp, |_, _| [70, 120, 55, 255])?;
    // The sector's colour list: a warm brown gradient with a darker band.
    let colour = texture(r, 64, Wrap::Clamp, |x, y| {
        let band = if (x / 8 + y / 8) % 2 == 0 { 0 } else { 18 };
        [
            (150 + x - band) as u8,
            (110 + y / 2 - band) as u8,
            (70 - band / 2) as u8,
            255,
        ]
    })?;
    // Detail: mid grey with a little noise (colour + detail − 0.5).
    let detail = texture(r, 64, Wrap::Repeat, |x, y| {
        let v = (118.0 + 20.0 * noise(x as f32 / 4.0, y as f32 / 4.0, 16)) as u8;
        [v, v, v, 255]
    })?;
    // The tiled normal map: strong bumps from a height field.
    let normal = texture(r, 64, Wrap::Repeat, |x, y| {
        let hf = |x: f32, y: f32| noise(x / 6.0, y / 6.0, 11) * 14.0;
        let (fx, fy) = (x as f32, y as f32);
        let dx = hf(fx + 1.0, fy) - hf(fx - 1.0, fy);
        let dy = hf(fx, fy + 1.0) - hf(fx, fy - 1.0);
        let n = Vec3::new(-dx, dy, 2.0).normalize();
        // Stored as (2R − 1, 2G − 1, 2B − 1); §6.4 step 1 subtracts 2 from 2B.
        let e = |c: f32| ((c * 0.5 + 0.5) * 255.0).round() as u8;
        [e(n.x), e(n.y), e(n.z), 255]
    })?;
    let mesh = grid(r, [-300.0, -300.0], [60, 60], 10.0, |x, _| {
        if x > 0.0 { SLOPE } else { [0.0, 0.0, 1.0] }
    })?;
    let mut base = FarTerrainDraw::new(mesh.clone(), Mat4::IDENTITY);
    base.colour = Some(green);
    let mut seam = FarTerrainDraw::new(mesh, Mat4::IDENTITY);
    seam.colour = Some(colour);
    seam.detail = Some(detail);
    seam.normal_map = Some(normal);
    // `c_NearViewOrigin` (Inferred, §6.4: the viewer landblock's south-west corner) at
    // (−160, −160): the mask square spans x, y from −319 to 159, so the north and east
    // strips of the patch lie outside it.
    seam.near_view_origin = Some([-160.0, -160.0]);
    draw(
        r,
        [240, 240],
        [-300.0, 300.0, -300.0, 300.0],
        Vec3::new(0.0, 0.0, 60.0),
        CLEAR,
        OffscreenFormat::Rgba8Srgb,
        sun_lighting(),
        vec![base, seam],
    )
}

#[test]
fn far_view_seam_golden() -> Result<(), RenderError> {
    let Some(mut r) = gpu::headless_renderer(CLEAR) else {
        return Ok(());
    };
    let img = seam_scene(&mut r)?;
    let again = seam_scene(&mut r)?;
    assert!(img.data == again.data, "render is not deterministic");
    golden::check(
        &Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/golden/far_view_seam.png"),
        &img,
        golden::Tolerance::default(),
    );
    Ok(())
}

// ---- pixel checks -------------------------------------------------------------------------

fn sat(v: f32) -> f32 {
    v.clamp(0.0, 1.0)
}

/// §6.5: the overlay's coverage on near-field geometry (*m* = 0).
fn coverage(h: f32, n_up: f32) -> f32 {
    sat((h - 80.0) / 80.0) * (n_up * n_up).min(1.0)
}

/// §6.4: the normal strength.
fn strength(h: f32) -> f32 {
    sat((h - 80.0) / 160.0)
}

/// Red channel (0–1) of pixel (`x`, `y`).
fn red(img: &RgbaImage, x: u32, y: u32) -> f32 {
    let i = ((y * img.width + x) * 4) as usize;
    f32::from(img.data[i]) / 255.0
}

/// A strip along +x (x 0–340, y −2..2), one vertex column per unit, seen from above with
/// the eye at the origin; one pixel per unit with pixel *i*'s centre at x = *i*.
fn strip(
    r: &mut Renderer<'static>,
    normal: [f32; 3],
    lighting: FrameLighting,
    setup: impl Fn(&mut FarTerrainDraw),
) -> Result<RgbaImage, RenderError> {
    let mesh = grid(r, [0.0, -2.0], [340, 4], 1.0, |_, _| normal)?;
    let white = texture(r, 1, Wrap::Clamp, |_, _| [255, 255, 255, 255])?;
    let mut d = FarTerrainDraw::new(mesh, Mat4::IDENTITY);
    d.colour = Some(white);
    setup(&mut d);
    draw(
        r,
        [341, 4],
        [-0.5, 340.5, -2.0, 2.0],
        Vec3::new(0.0, 0.0, 10.0),
        [0.0, 0.0, 0.0, 1.0],
        OffscreenFormat::Rgba8Unorm,
        lighting,
        vec![d],
    )
}

/// §14.4 and §6.5 (FV-I7, FV-I8): over a black background a white overlay shows its
/// coverage *w*: 0 within 80 units, 0.405 at 120 units and 0.81 from 160 units on the slope
/// *n*_up = 0.9, and 1 one unit outside the near-view square (*m* = 1).
#[test]
fn seam_coverage_matches_the_worked_example() -> Result<(), RenderError> {
    let Some(mut r) = gpu::headless_renderer(CLEAR) else {
        return Ok(());
    };
    let flat_normal = texture(&mut r, 1, Wrap::Repeat, |_, _| [128, 128, 255, 255])?;
    // Ambient 0.5 only: colour + grey detail − 0.5 = 1.0, × 0.5, × 2 = white.
    let lighting = FrameLighting {
        ambient: [0.5; 3],
        hemispheric: [0.0; 3],
        sun: None,
        ..FrameLighting::default()
    };
    // c_NearViewOrigin (0, −160): the mask square holds x ≤ 319 and every y of the strip.
    let img = strip(&mut r, SLOPE, lighting.clone(), |d| {
        d.normal_map = Some(flat_normal);
        d.near_view_origin = Some([0.0, -160.0]);
    })?;
    let row = 1;
    // The worked example: h = 120 → 0.405, h = 200 → 0.81.
    assert!((coverage(120.0, 0.9) - 0.405).abs() < 1e-6);
    assert!((coverage(200.0, 0.9) - 0.81).abs() < 1e-6);
    // The strip's pixel rows lie 0.5 units off the axis: h = sqrt(x² + 0.25).
    for x in [20u32, 79, 90, 100, 120, 140, 159, 160, 200, 250, 300, 318] {
        let h = (x as f32).hypot(0.5);
        let want = coverage(h, 0.9);
        let got = red(&img, x, row);
        assert!(
            (got - want).abs() <= 3.0 / 255.0,
            "x {x}: coverage {got}, spec {want}"
        );
    }
    // Outside the square (p ≥ 320, m = 1): full coverage.
    for x in [321u32, 330, 339] {
        let got = red(&img, x, row);
        assert!(got >= 254.0 / 255.0, "x {x}: {got} outside the near view");
    }
    // Without a near-view origin the mask is 1 everywhere (far batches, §6.4).
    let img = strip(&mut r, SLOPE, lighting.clone(), |d| {
        d.normal_map = Some(flat_normal)
    })?;
    for x in [10u32, 120, 300] {
        assert!(red(&img, x, row) >= 254.0 / 255.0, "x {x}");
    }
    // The plain template has no coverage term: opaque everywhere.
    let img = strip(&mut r, SLOPE, lighting, |_| {})?;
    for x in [10u32, 120] {
        assert!(red(&img, x, row) >= 254.0 / 255.0, "x {x}");
    }
    Ok(())
}

/// §6.4 pass 0x3F (FV-I9): the sun per pixel through the normal map with strength *t*: the
/// flat normal within 80 units, the full map from 240 units on. Lit with ambient *A* = 0.2
/// and a white sun towards (1, 0, 1) / √2, a texel (1, 0.5, 1) tilts the normal towards +x.
#[test]
fn normal_strength_ramps_from_80_to_240_units() -> Result<(), RenderError> {
    let Some(mut r) = gpu::headless_renderer(CLEAR) else {
        return Ok(());
    };
    let tilted = texture(&mut r, 1, Wrap::Repeat, |_, _| [255, 128, 255, 255])?;
    let grey = texture(&mut r, 1, Wrap::Clamp, |_, _| [128, 128, 128, 255])?;
    let lighting = FrameLighting {
        ambient: [0.2; 3],
        hemispheric: [0.0; 3],
        sun: Some(SunLight {
            direction: Vec3::new(1.0, 0.0, 1.0),
            color: [1.0; 3],
            specular: [0.0; 3],
        }),
        ..FrameLighting::default()
    };
    let img = strip(&mut r, [0.0, 0.0, 1.0], lighting, |d| {
        d.normal_map = Some(tilted);
        d.colour = Some(grey);
        d.detail = Some(grey);
    })?;
    // The spec's light (§6.4 steps 1–3) for strength t, L = (1, 0, 1) / √2 in the tangent
    // frame of a level vertex (FD5: tangent east, bitangent north).
    let g = 128.0 / 255.0;
    let light = |t: f32| {
        let l = Vec3::new(1.0, 0.0, 1.0).normalize();
        let n = t * Vec3::new(1.0, 2.0 * g - 1.0, 0.0) + Vec3::Z;
        let e = sat(n.dot(l));
        let e1 = sat(2.0 * e - 0.9);
        let mu = l.max_element();
        let e2 = e + mu * (e1 - e);
        0.2 + e2
    };
    // (colour + detail − 0.5) × Σ, × 2; no fog; mask 1 (no near-view origin).
    let expect = |h: f32| (2.0 * (2.0 * g - 0.5) * light(strength(h))).min(1.0);
    assert!((strength(120.0) - 0.25).abs() < 1e-6, "§14.4");
    assert!((strength(200.0) - 0.75).abs() < 1e-6, "§14.4");
    let row = 1;
    for x in [40u32, 80, 120, 160, 200, 240, 300] {
        let h = (x as f32).hypot(0.5);
        let want = expect(h);
        let got = red(&img, x, row);
        assert!(
            (got - want).abs() <= 3.0 / 255.0,
            "x {x}: {got}, spec {want} (t {})",
            strength(h)
        );
    }
    // The ramp changes the picture: flat near, tilted far.
    assert!((red(&img, 40, row) - red(&img, 300, row)).abs() > 0.05);
    Ok(())
}

/// §6.5's premise (`terrain-texturing.md` §12): the overlay draws the near chunk's own
/// triangles again with the far-terrain program, after the layers, depth test "nearer or
/// equal". A hilly 33 × 33-vertex chunk (opaque red through the terrain layer pipeline)
/// and the same triangles as a far-terrain draw (white, no near-view origin: *m* = 1, so
/// *w* = 1), both moved by the same transform, seen obliquely in perspective: every pixel
/// the chunk covers is white. A depth bias or another order of the position maths in
/// either program, or a position not marked invariant, lets red through (z-fail or
/// z-fight: without the mark this test fails with red stripes on the reference machine's
/// driver). A second chunk lifted by half a unit stays in front of the overlay.
#[test]
fn seam_overlay_has_the_layer_pipelines_depth() -> Result<(), RenderError> {
    let Some(mut r) = gpu::headless_renderer(CLEAR) else {
        return Ok(());
    };
    const SIDE: u32 = 33;
    let step = 160.0 / (SIDE - 1) as f32;
    let height = |x: f32, y: f32| 18.0 * (x / 23.0).sin() * (y / 31.0).cos() + 0.07 * x;
    let mut positions = Vec::new();
    let mut normals = Vec::new();
    let mut uvs = Vec::new();
    for j in 0..SIDE {
        for i in 0..SIDE {
            let (x, y) = (i as f32 * step, j as f32 * step);
            positions.push([x, y, height(x, y)]);
            let dx = height(x + 0.5, y) - height(x - 0.5, y);
            let dy = height(x, y + 0.5) - height(x, y - 0.5);
            normals.push(Vec3::new(-dx, -dy, 1.0).normalize().to_array());
            uvs.push([x / 480.0, 1.0 - y / 480.0]);
        }
    }
    let at = |i: u32, j: u32| j * SIDE + i;
    let mut tris16: Vec<[u16; 3]> = Vec::new();
    for j in 0..SIDE - 1 {
        for i in 0..SIDE - 1 {
            // Counter-clockwise seen from above.
            tris16.push([at(i, j), at(i + 1, j), at(i + 1, j + 1)].map(|v| v as u16));
            tris16.push([at(i, j), at(i + 1, j + 1), at(i, j + 1)].map(|v| v as u16));
        }
    }
    let indices: Vec<u32> = tris16.iter().flatten().map(|&v| u32::from(v)).collect();
    let set0: Vec<[f32; 2]> = positions.iter().map(|p| [p[0], p[1]]).collect();
    let tangents = vec![[1.0, 0.0, 0.0]; positions.len()];
    let binormals = vec![[0.0, 1.0, 0.0]; positions.len()];
    let red = texture(&mut r, 1, Wrap::Clamp, |_, _| [255, 0, 0, 255])?;
    let white = texture(&mut r, 1, Wrap::Clamp, |_, _| [255, 255, 255, 255])?;
    let flat_normal = texture(&mut r, 1, Wrap::Repeat, |_, _| [128, 128, 255, 255])?;
    let layer = TerrainLayer {
        triangles: &tris16,
        diffuse_color: [1.0; 3],
        textures: TerrainLayerTextures {
            diffuse: Some(red),
            ..TerrainLayerTextures::default()
        },
        ..TerrainLayer::default()
    };
    let chunk = r.upload_terrain_chunk(&TerrainChunkData {
        label: Some("near chunk"),
        vertices: TerrainVertices {
            positions: &positions,
            normals: &normals,
            tangents: &tangents,
            binormals: &binormals,
            sets: [&set0, &[], &[], &set0],
        },
        layers: &[layer],
        light_map: None,
    })?;
    let overlay = r.upload_far_terrain(&FarTerrainData {
        label: "seam overlay",
        positions: &positions,
        normals: &normals,
        uvs: &uvs,
        indices: &indices,
        stitch_start: indices.len(),
    })?;
    // The chunk's block lies 320 east and 160 north of the render origin.
    let tf = Mat4::from_translation(Vec3::new(320.0, 160.0, 0.0));
    let camera = Camera {
        eye: Vec3::new(330.0, 110.0, 110.0),
        target: Vec3::new(400.0, 240.0, 0.0),
        up: Vec3::Z,
        projection: Perspective {
            fov_y: 60f32.to_radians(),
            near: 1.0,
            far: 3000.0,
        },
    };
    // Ambient 0.5 only: both draws show their colour as it is.
    let lighting = FrameLighting {
        ambient: [0.5; 3],
        hemispheric: [0.0; 3],
        sun: None,
        ..FrameLighting::default()
    };
    let lifted = Mat4::from_translation(Vec3::new(0.0, 0.0, 0.5)) * tf;
    let render = |r: &mut Renderer<'static>, overlay_on: bool, in_front: bool| {
        let target = r.create_offscreen_target(256, 192, OffscreenFormat::Rgba8Unorm)?;
        let mut frame = r.begin_offscreen_frame(&target)?;
        {
            let pass = frame
                .world_pass("seam depth", &camera)
                .clear([0.0, 0.0, 1.0, 1.0])
                .frame_lighting(lighting.clone());
            pass.terrain(TerrainDraw::new(chunk.clone(), tf));
            if in_front {
                pass.terrain(TerrainDraw::new(chunk.clone(), lifted));
            }
            if overlay_on {
                let mut d = FarTerrainDraw::new(overlay.clone(), tf);
                d.colour = Some(white);
                d.normal_map = Some(flat_normal);
                pass.far_terrain(d);
            }
        }
        r.end_frame(frame)?;
        r.read_pixels(&target)
    };
    let is_red = |p: &[u8; 4]| p[0] > 128 && p[1] < 128 && p[2] < 128;
    let is_white = |p: &[u8; 4]| p.iter().take(3).all(|&c| c >= 250);
    // The chunk alone covers much of the picture in red.
    let under: RgbaImage = render(&mut r, false, false)?;
    let covered: Vec<usize> = under
        .data
        .as_chunks::<4>()
        .0
        .iter()
        .enumerate()
        .filter(|(_, p)| is_red(p))
        .map(|(i, _)| i)
        .collect();
    assert!(
        covered.len() > 256 * 192 / 4,
        "the chunk covers {} pixels",
        covered.len()
    );
    // With the overlay every one of them is white.
    let over = render(&mut r, true, false)?;
    let px = over.data.as_chunks::<4>().0;
    let failed = covered.iter().filter(|&&i| !is_white(&px[i])).count();
    assert_eq!(
        failed,
        0,
        "{failed} of {} pixels not covered",
        covered.len()
    );
    assert!(
        px.iter().all(|p| !is_red(p)),
        "red shows through the overlay"
    );
    // A surface half a unit above hides the overlay (only slivers along its edge show).
    let hidden = render(&mut r, true, true)?;
    let white_px = hidden
        .data
        .as_chunks::<4>()
        .0
        .iter()
        .filter(|p| is_white(p))
        .count();
    assert!(
        white_px * 50 < covered.len(),
        "the overlay shows through a surface in front of it: {white_px} pixels"
    );
    Ok(())
}
