// SPDX-License-Identifier: GPL-3.0-or-later
//! The water pipeline (M4.5, `docs/specs/water.md` §8–§9, `docs/specs/render.md` §5.3):
//!
//! - `water.png`: a terrain basin with a pillar, flooded by two water quads at the water
//!   height (one flat, one subdivided with vertex waves), with three scrolling normal-map
//!   layers, refraction of the scene drawn before (a copy of the colour target), Fresnel,
//!   the sun's specular and sparkle, and distance fog;
//! - pixel checks of the refraction composite (water colour, overbright, under-water fog
//!   tint and the shoreline blend of §8.4 step 3) against the spec's formulas, distant
//!   water drawn as many quads in one draw, and hostile input.
//!
//! Regenerate with `TESSERA_BLESS=1 cargo test -p tessera-render --test water_golden`.

#[path = "support/golden.rs"]
mod golden;
#[path = "support/gpu.rs"]
mod gpu;

use std::path::Path;

use tessera_render::glam::{Mat4, Vec3};
use tessera_render::{
    AlphaMode, Camera, ColorSpace, Filter, Fog, FrameLighting, LitSurface, Material, MeshData,
    MeshDraw, OffscreenFormat, Opacity, PassSpace, Perspective, RenderError, Renderer, RgbaImage,
    SunLight, SunShadow, TerrainChunkData, TerrainDraw, TerrainLayer, TerrainLayerTextures,
    TerrainTransform, TerrainVertices, TextureId, TextureUpload, WaterDraw, WaterParams, WaterQuad,
    Wrap, water_fresnel,
};

const WIDTH: u32 = 320;
const HEIGHT: u32 = 200;
const CLEAR: [f32; 4] = [0.60, 0.68, 0.78, 1.0];

fn texels(size: u32, f: impl Fn(u32, u32) -> [u8; 4]) -> Vec<u8> {
    (0..size * size)
        .flat_map(|i| f(i % size, i / size))
        .collect()
}

fn upload(
    r: &mut Renderer<'static>,
    size: u32,
    data: &[u8],
    wrap: Wrap,
) -> Result<TextureId, RenderError> {
    r.upload_texture(&TextureUpload {
        color_space: ColorSpace::Linear,
        filter: Filter::Linear,
        wrap,
        ..TextureUpload::rgba8(size, size, data)
    })
}

/// A smooth tileable value noise in 0..1.
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

/// A tileable water normal map: ripples from a noise height field.
fn ripples() -> Vec<u8> {
    let size = 64u32;
    let hf = |x: f32, y: f32| noise(x / 6.0, y / 6.0, size / 6 + 1) * 3.0;
    texels(size, |x, y| {
        let (fx, fy) = (x as f32, y as f32);
        let dx = hf(fx + 1.0, fy) - hf(fx - 1.0, fy);
        let dy = hf(fx, fy + 1.0) - hf(fx, fy - 1.0);
        let n = Vec3::new(-dx, dy, 2.0).normalize();
        let e = |c: f32| ((c * 0.5 + 0.5) * 255.0).round() as u8;
        [e(n.x), e(n.y), e(n.z), 255]
    })
}

const SIDE: usize = 33;

fn basin_height(x: f32, y: f32) -> f32 {
    let d2 = (x - 80.0).powi(2) / 4200.0 + (y - 95.0).powi(2) / 2600.0;
    2.0 + 9.0 * d2 + 1.2 * noise(x / 20.0, y / 20.0, 16)
}

fn basin(r: &mut Renderer<'static>) -> Result<tessera_render::TerrainChunk, RenderError> {
    let mut positions = Vec::new();
    let mut normals = Vec::new();
    let mut set0 = Vec::new();
    for j in 0..SIDE {
        for i in 0..SIDE {
            let (x, y) = (i as f32 * 5.0, j as f32 * 5.0);
            positions.push([x, y, basin_height(x, y)]);
            let e = 0.5;
            let dx = (basin_height(x + e, y) - basin_height(x - e, y)) / (2.0 * e);
            let dy = (basin_height(x, y + e) - basin_height(x, y - e)) / (2.0 * e);
            normals.push(Vec3::new(-dx, -dy, 1.0).normalize().to_array());
            set0.push([x, y]);
        }
    }
    let mut triangles = Vec::new();
    for j in 0..SIDE - 1 {
        for i in 0..SIDE - 1 {
            let a = (j * SIDE + i) as u16;
            triangles.push([a, a + 1, a + SIDE as u16 + 1]);
            triangles.push([a, a + SIDE as u16 + 1, a + SIDE as u16]);
        }
    }
    let stones = upload(
        r,
        64,
        &texels(64, |x, y| {
            let n = noise(x as f32 / 4.0, y as f32 / 4.0, 16);
            let cell = ((x / 16) + (y / 16)) % 2 == 0;
            let base = if cell { 170.0 } else { 120.0 };
            let v = base + 50.0 * n;
            [v as u8, (v * 0.9) as u8, (v * 0.7) as u8, 255]
        }),
        Wrap::Repeat,
    )?;
    let layers = [TerrainLayer {
        triangles: &triangles,
        transform: TerrainTransform {
            u_scale: 0.08,
            v_scale: 0.08,
            ..TerrainTransform::default()
        },
        diffuse_color: [1.0; 3],
        textures: TerrainLayerTextures {
            diffuse: Some(stones),
            ..TerrainLayerTextures::default()
        },
        ..TerrainLayer::default()
    }];
    r.upload_terrain_chunk(&TerrainChunkData {
        label: Some("basin"),
        vertices: TerrainVertices {
            positions: &positions,
            normals: &normals,
            sets: [&set0, &[], &[], &[]],
            ..TerrainVertices::default()
        },
        layers: &layers,
        light_map: None,
    })
}

/// A box from `lo` to `hi` (Z up), counter-clockwise outward faces.
fn cuboid(lo: Vec3, hi: Vec3) -> MeshData {
    let c = |x: bool, y: bool, z: bool| {
        [
            if x { hi.x } else { lo.x },
            if y { hi.y } else { lo.y },
            if z { hi.z } else { lo.z },
        ]
    };
    let faces: [[[f32; 3]; 4]; 5] = [
        [
            c(false, false, false),
            c(true, false, false),
            c(true, false, true),
            c(false, false, true),
        ],
        [
            c(true, false, false),
            c(true, true, false),
            c(true, true, true),
            c(true, false, true),
        ],
        [
            c(true, true, false),
            c(false, true, false),
            c(false, true, true),
            c(true, true, true),
        ],
        [
            c(false, true, false),
            c(false, false, false),
            c(false, false, true),
            c(false, true, true),
        ],
        [
            c(false, false, true),
            c(true, false, true),
            c(true, true, true),
            c(false, true, true),
        ],
    ];
    let mut positions = Vec::new();
    let mut uvs = Vec::new();
    let mut indices = Vec::new();
    for f in faces {
        let base = positions.len() as u32;
        positions.extend(f);
        uvs.extend([[0.0, 1.0], [1.0, 1.0], [1.0, 0.0], [0.0, 0.0]]);
        indices.extend([base, base + 1, base + 2, base, base + 2, base + 3]);
    }
    let mut m = MeshData::new(positions, indices);
    m.uvs = Some(uvs);
    m
}

fn water_scene(r: &mut Renderer<'static>) -> Result<RgbaImage, RenderError> {
    let ground = basin(r)?;
    let ripples = upload(r, 64, &ripples(), Wrap::Repeat)?;
    let stripes = upload(
        r,
        16,
        &texels(16, |_, y| {
            if (y / 2) % 2 == 0 {
                [200, 60, 50, 255]
            } else {
                [235, 225, 210, 255]
            }
        }),
        Wrap::Repeat,
    )?;
    let pillar = r.upload_mesh(&cuboid(
        Vec3::new(70.0, 85.0, 0.0),
        Vec3::new(82.0, 97.0, 22.0),
    ))?;
    let level = 7.5;
    // West half flat; east half subdivided with vertex waves.
    let flat = [WaterQuad {
        origin: [0.0, 20.0],
        size: [80.0, 140.0],
        height: level,
    }];
    let wavy = [WaterQuad {
        origin: [80.0, 20.0],
        size: [80.0, 140.0],
        height: level,
    }];
    let params = WaterParams {
        water_color: [0.75, 0.9, 0.95],
        fog_color: [0.05, 0.16, 0.22],
        fog_amount: 0.55,
        shoreline_color: [0.9, 1.0, 1.0],
        ..WaterParams::default()
    };
    let toward = Vec3::new(0.25, 0.9, 0.35).normalize();
    let lighting = FrameLighting {
        ambient: [0.3, 0.32, 0.35],
        hemispheric: [0.05, 0.05, 0.05],
        sun: Some(SunLight {
            direction: toward,
            color: [0.5, 0.48, 0.42],
            specular: [0.9, 0.85, 0.7],
        }),
        fog: Some(Fog {
            near: 120.0,
            far: 420.0,
            color: [CLEAR[0], CLEAR[1], CLEAR[2]],
        }),
        shadow: Some(SunShadow::fit(
            toward,
            Vec3::new(80.0, 80.0, 5.0),
            120.0,
            400.0,
            1024,
        )),
        time: 12.5,
        ..FrameLighting::default()
    };
    let camera = Camera {
        eye: Vec3::new(70.0, -25.0, 70.0),
        target: Vec3::new(88.0, 100.0, 0.0),
        up: Vec3::Z,
        projection: Perspective {
            fov_y: 55f32.to_radians(),
            near: 0.5,
            far: 1000.0,
        },
    };
    let target = r.create_offscreen_target(WIDTH, HEIGHT, OffscreenFormat::Rgba8Srgb)?;
    let mut frame = r.begin_offscreen_frame(&target)?;
    frame
        .world_pass("water", &camera)
        .clear(CLEAR)
        .frame_lighting(lighting)
        .terrain(TerrainDraw::new(ground, Mat4::IDENTITY))
        .mesh(
            MeshDraw::new(pillar, Mat4::IDENTITY, Material::textured(stripes)).with_lit(
                LitSurface {
                    opacity: Opacity::One,
                    ..LitSurface::default()
                },
            ),
        )
        .water(WaterDraw::new(flat, Some(ripples), params))
        .water(WaterDraw {
            subdivisions: 32,
            ..WaterDraw::new(
                wavy,
                Some(ripples),
                WaterParams {
                    wave_amplitude: 0.6,
                    wave_frequency: 0.35,
                    ..params
                },
            )
        });
    let stats = r.end_frame(frame)?;
    assert_eq!(stats.water_quads, 2, "{stats:?}");
    assert_eq!(stats.scene_copies, 1);
    r.read_pixels(&target)
}

#[test]
fn water_golden() -> Result<(), RenderError> {
    let Some(mut r) = gpu::headless_renderer(CLEAR) else {
        return Ok(());
    };
    let img = water_scene(&mut r)?;
    let again = water_scene(&mut r)?;
    assert!(img.data == again.data, "render is not deterministic");
    golden::check(
        &Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/golden/water.png"),
        &img,
        golden::Tolerance::default(),
    );
    Ok(())
}

// ---- pixel checks -------------------------------------------------------------------------

/// A unit-colour quad from (0, 0) to (4, 4) at height z (Z up).
fn floor(z: f32) -> MeshData {
    let mut m = MeshData::new(
        vec![[0.0, 0.0, z], [4.0, 0.0, z], [4.0, 4.0, z], [0.0, 4.0, z]],
        vec![0, 1, 2, 0, 2, 3],
    );
    m.normals = Some(vec![[0.0, 0.0, 1.0]; 4]);
    m
}

/// Orthographic top-down view of x, y in 0..4 (reversed depth).
fn top_down() -> Mat4 {
    let proj = tessera_render::glam::camera::rh::proj::directx::orthographic(
        0.0, 4.0, 0.0, 4.0, -50.0, 50.0,
    );
    Mat4::from_cols_array(&[
        1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, -1.0, 0.0, 0.0, 0.0, 1.0, 1.0,
    ]) * proj
}

/// Draws a floor of colour `floor_rgb` (unlit) at z = −1, water at z = 0 seen from an eye
/// at height `eye_z`, and returns the centre pixel.
fn water_over(
    r: &mut Renderer<'static>,
    floor_rgb: [f32; 3],
    params: WaterParams,
    eye_z: f32,
) -> Result<[u8; 4], RenderError> {
    water_over_with(
        r,
        floor_rgb,
        params,
        eye_z,
        None,
        FrameLighting {
            sun: None,
            ..FrameLighting::default()
        },
    )
}

/// [`water_over`] with a normal map and the frame's lighting.
fn water_over_with(
    r: &mut Renderer<'static>,
    floor_rgb: [f32; 3],
    params: WaterParams,
    eye_z: f32,
    normal_map: Option<TextureId>,
    lighting: FrameLighting,
) -> Result<[u8; 4], RenderError> {
    let floor = r.upload_mesh(&floor(-1.0))?;
    let target = r.create_offscreen_target(16, 16, OffscreenFormat::Rgba8Unorm)?;
    let mut frame = r.begin_offscreen_frame(&target)?;
    frame
        .pass(
            "water",
            PassSpace::World {
                view_proj: top_down(),
            },
        )
        .eye(Vec3::new(2.0, 2.0, eye_z))
        .clear([0.0, 0.0, 0.0, 1.0])
        .frame_lighting(lighting)
        .mesh(
            MeshDraw::new(
                floor,
                Mat4::IDENTITY,
                Material {
                    unlit: true,
                    alpha: AlphaMode::Opaque,
                    ..Material::color([floor_rgb[0], floor_rgb[1], floor_rgb[2], 1.0])
                },
            )
            .with_lit(LitSurface {
                fog: false,
                ..LitSurface::default()
            }),
        )
        .water(WaterDraw::new(
            [WaterQuad {
                origin: [0.0, 0.0],
                size: [4.0, 4.0],
                height: 0.0,
            }],
            normal_map,
            params,
        ));
    let stats = r.end_frame(frame)?;
    assert_eq!(stats.scene_copies, 1);
    assert_eq!(stats.water_quads, 1);
    r.destroy_mesh(floor);
    Ok(r.read_pixels(&target)?.pixel(8, 8).unwrap_or_default())
}

fn close(px: [u8; 4], want: [f32; 3]) -> bool {
    (0..3).all(|i| (f32::from(px[i]) - want[i] * 255.0).abs() <= 3.0)
}

#[test]
fn refraction_composite_follows_section_8_4() -> Result<(), RenderError> {
    let Some(mut r) = gpu::headless_renderer(CLEAR) else {
        return Ok(());
    };
    let floor = [0.8, 0.4, 0.2];
    // Flat water seen straight down: |v . n| = 1, so the Fresnel factor is the bias, and
    // the reflection is the unmodified refraction sample F (Tessera rule).
    let clear_water = WaterParams {
        fog_amount: 0.0,
        ..WaterParams::default()
    };
    // Eye 100 units up: the shoreline weight w is 1, U = F (no fog): colour = F.
    let px = water_over(&mut r, floor, clear_water, 100.0)?;
    assert!(close(px, floor), "{px:?}");
    // Water colour and overbright multiply the composite.
    let tinted = WaterParams {
        water_color: [0.5, 1.0, 1.0],
        overbright: 1.5,
        ..clear_water
    };
    let px = water_over(&mut r, floor, tinted, 100.0)?;
    assert!(close(px, [0.6, 0.6, 0.3]), "{px:?}");
    // Under-water fog: U = F + amount (fog − F); refraction = F S + w (U − F S); colour =
    // refraction + f (F − refraction) with f = the Fresnel bias.
    let foggy = WaterParams {
        fog_amount: 0.5,
        fog_color: [0.0, 0.0, 1.0],
        shoreline_color: [1.0, 0.5, 1.0],
        ..WaterParams::default()
    };
    let f = water_fresnel(&foggy, 1.0);
    for (eye, w) in [(100.0, 1.0), (10.0, 0.5), (3.0, 0.0)] {
        let px = water_over(&mut r, floor, foggy, eye)?;
        let want: Vec<f32> = (0..3)
            .map(|i| {
                let fi = floor[i];
                let u = fi + 0.5 * (foggy.fog_color[i] - fi);
                let s = fi * foggy.shoreline_color[i];
                let refr = s + w * (u - s);
                refr + f * (fi - refr)
            })
            .collect();
        assert!(
            close(px, [want[0], want[1], want[2]]),
            "eye {eye}: {px:?} vs {want:?}"
        );
    }
    Ok(())
}

#[test]
fn distant_water_quads_draw_in_one_call_and_hostile_input_is_dropped() -> Result<(), RenderError> {
    let Some(mut r) = gpu::headless_renderer(CLEAR) else {
        return Ok(());
    };
    // Distant water (§9): one quad per wet landblock of many 3 × 3 sectors, one draw.
    let mut quads: Vec<WaterQuad> = (0..30)
        .flat_map(|y| {
            (0..30).map(move |x| WaterQuad::landblock([x as f32 * 160.0, y as f32 * 160.0], 5.0))
        })
        .collect();
    quads.push(WaterQuad {
        origin: [f32::NAN, 0.0],
        size: [160.0, 160.0],
        height: 1.0,
    });
    quads.push(WaterQuad {
        origin: [0.0, 0.0],
        size: [0.0, 160.0],
        height: 1.0,
    });
    let target = r.create_offscreen_target(64, 64, OffscreenFormat::Rgba8Srgb)?;
    let camera = Camera {
        eye: Vec3::new(2400.0, -600.0, 400.0),
        target: Vec3::new(2400.0, 2400.0, 0.0),
        up: Vec3::Z,
        projection: Perspective {
            fov_y: 60f32.to_radians(),
            near: 1.0,
            far: 10_000.0,
        },
    };
    let gone = upload(&mut r, 1, &[128, 128, 255, 255], Wrap::Repeat)?;
    assert!(r.destroy_texture(gone));
    let mut frame = r.begin_offscreen_frame(&target)?;
    frame
        .world_pass("distant", &camera)
        .clear(CLEAR)
        .frame_lighting(FrameLighting::default())
        .water(WaterDraw {
            subdivisions: 1_000,
            ..WaterDraw::new(
                quads,
                Some(gone),
                WaterParams {
                    specular_power: f32::NAN,
                    water_color: [f32::INFINITY; 3],
                    ..WaterParams::default()
                },
            )
        });
    let stats = r.end_frame(frame)?;
    assert_eq!(stats.water_quads, 900);
    assert_eq!(stats.draw_calls, 1);
    // Too many quads in one pass is an error, not an allocation.
    let many = vec![WaterQuad::landblock([0.0, 0.0], 0.0); tessera_render::MAX_WATER_QUADS + 1];
    let mut frame = r.begin_offscreen_frame(&target)?;
    frame
        .world_pass("too many", &camera)
        .frame_lighting(FrameLighting::default())
        .water(WaterDraw::new(many, None, WaterParams::default()));
    assert!(matches!(
        r.end_frame(frame),
        Err(RenderError::LimitExceeded { .. })
    ));
    Ok(())
}

#[test]
fn srgb_normal_maps_are_re_encoded() -> Result<(), RenderError> {
    let Some(mut r) = gpu::headless_renderer(CLEAR) else {
        return Ok(());
    };
    // render.md §5.1: a texture uploaded sRGB is re-encoded after sampling, so the same
    // texels give the same ripples, specular and sparkle whichever colour space they were
    // uploaded in.
    let texel = [204, 128, 230, 255];
    let mut ids = Vec::new();
    for color_space in [ColorSpace::Linear, ColorSpace::Srgb] {
        ids.push(r.upload_texture(&TextureUpload {
            color_space,
            filter: Filter::Nearest,
            wrap: Wrap::Repeat,
            ..TextureUpload::rgba8(1, 1, &texel)
        })?);
    }
    let params = WaterParams {
        fake_specular_threshold: 0.0,
        fog_amount: 0.0,
        ..WaterParams::default()
    };
    let lighting = FrameLighting {
        ambient: [0.0; 3],
        hemispheric: [0.0; 3],
        sun: Some(SunLight {
            direction: Vec3::new(0.3, 0.0, 1.0).normalize(),
            color: [0.5; 3],
            specular: [0.3; 3],
        }),
        ..FrameLighting::default()
    };
    let floor = [0.3, 0.2, 0.1];
    let linear = water_over_with(&mut r, floor, params, 100.0, Some(ids[0]), lighting.clone())?;
    let srgb = water_over_with(&mut r, floor, params, 100.0, Some(ids[1]), lighting.clone())?;
    let flat = water_over_with(&mut r, floor, params, 100.0, None, lighting)?;
    assert!(
        (0..3).all(|i| linear[i].abs_diff(srgb[i]) <= 2),
        "linear {linear:?} vs sRGB {srgb:?}"
    );
    // The normal map has a visible effect (the check is not vacuous).
    assert!(
        (0..3).any(|i| linear[i].abs_diff(flat[i]) > 10),
        "{linear:?} vs flat {flat:?}"
    );
    Ok(())
}

#[test]
fn one_draw_per_quad_stays_bounded() -> Result<(), RenderError> {
    let Some(mut r) = gpu::headless_renderer(CLEAR) else {
        return Ok(());
    };
    // Many draws of one quad each at the largest grid: one uniform buffer for all of them,
    // a few bind groups, and a per-pass vertex budget (render.md §5.3).
    let normal = upload(&mut r, 1, &[128, 128, 255, 255], Wrap::Repeat)?;
    let target = r.create_offscreen_target(32, 32, OffscreenFormat::Rgba8Srgb)?;
    let camera = Camera {
        eye: Vec3::new(1600.0, -400.0, 600.0),
        target: Vec3::new(1600.0, 1600.0, 0.0),
        up: Vec3::Z,
        projection: Perspective {
            fov_y: 60f32.to_radians(),
            near: 1.0,
            far: 20_000.0,
        },
    };
    let draws = 4_096u32;
    let mut best = f64::INFINITY;
    let mut stats = tessera_render::FrameStats::default();
    // The first frame builds the pipeline; time the CPU side of the later ones.
    for _ in 0..3 {
        let mut frame = r.begin_offscreen_frame(&target)?;
        let pass = frame
            .world_pass("many draws", &camera)
            .clear(CLEAR)
            .frame_lighting(FrameLighting::default());
        for i in 0..draws {
            pass.water(WaterDraw {
                subdivisions: 64,
                ..WaterDraw::new(
                    [WaterQuad::landblock(
                        [(i % 64) as f32 * 160.0, (i / 64) as f32 * 160.0],
                        0.0,
                    )],
                    (i % 2 == 0).then_some(normal),
                    WaterParams {
                        wave_amplitude: 0.5,
                        ..WaterParams::default()
                    },
                )
            });
        }
        let start = std::time::Instant::now();
        stats = r.end_frame(frame)?;
        best = best.min(start.elapsed().as_secs_f64() * 1e3);
    }
    eprintln!("water: {draws} draws of one quad, end_frame {best:.1} ms (best of 3 frames)");
    assert_eq!(stats.water_quads, draws);
    assert_eq!(stats.draw_calls, draws);
    Ok(())
}
