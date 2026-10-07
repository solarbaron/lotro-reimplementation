// SPDX-License-Identifier: GPL-3.0-or-later
//! The sky (M4.5, `docs/specs/environment.md` §11.4, §12; `docs/specs/render.md` §5.4):
//!
//! - `sky_morning.png` and `sky_evening.png`: the same synthetic sky objects at two times
//!   of day: a vertex-coloured dome, a sun disc and a drifting cloud layer, each placed by
//!   its angles (*A* along the arc, *B* its azimuth) and tinted by its keyframe tint, over
//!   lit ground under the matching sun;
//! - pixel checks of the placement rule, that the sky never covers geometry (queued before
//!   or after it, or in an earlier pass), opacity 0 hiding an object, and hostile input.
//!
//! Regenerate with `TESSERA_BLESS=1 cargo test -p tessera-render --test sky_golden`.

#[path = "support/golden.rs"]
mod golden;
#[path = "support/gpu.rs"]
mod gpu;

use std::f32::consts::{PI, TAU};
use std::path::Path;

use tessera_render::glam::{Mat4, Vec3};
use tessera_render::{
    AlphaMode, Camera, ColorSpace, Filter, Fog, FrameLighting, LitSurface, Material, MeshData,
    MeshDraw, OffscreenFormat, Opacity, Perspective, RenderError, Renderer, RgbaImage, SkyDraw,
    SkyObject, SunLight, TextureId, TextureUpload, WorldModel, WorldSurface, Wrap,
};

const WIDTH: u32 = 320;
const HEIGHT: u32 = 200;
const CLEAR: [f32; 4] = [0.0, 0.0, 0.0, 1.0];

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

/// A Z-up unit sphere whose vertex colours run from a pale horizon to a deep zenith.
fn dome() -> MeshData {
    let (segs, rings) = (48u32, 24u32);
    let mut positions = Vec::new();
    let mut colors = Vec::new();
    for j in 0..=rings {
        let phi = PI * (j as f32 / rings as f32) - PI / 2.0;
        for i in 0..=segs {
            let th = TAU * i as f32 / segs as f32;
            let p = Vec3::new(phi.cos() * th.cos(), phi.cos() * th.sin(), phi.sin());
            positions.push(p.to_array());
            let up = p.z.max(0.0).powf(0.6);
            let horizon = [0.95, 0.88, 0.80, 1.0];
            let zenith = [0.22, 0.42, 0.78, 1.0];
            colors.push(std::array::from_fn(|k| {
                horizon[k] + up * (zenith[k] - horizon[k])
            }));
        }
    }
    let mut indices = Vec::new();
    let w = segs + 1;
    for j in 0..rings {
        for i in 0..segs {
            let a = j * w + i;
            indices.extend([a, a + 1, a + w + 1, a, a + w + 1, a + w]);
        }
    }
    let mut m = MeshData::new(positions, indices);
    m.colors = Some(colors);
    m
}

/// A square card at distance `d` along +Y facing the origin, `size` across.
fn card(d: f32, size: f32) -> MeshData {
    let h = size * 0.5;
    let mut m = MeshData::new(
        vec![[-h, d, -h], [h, d, -h], [h, d, h], [-h, d, h]],
        vec![0, 1, 2, 0, 2, 3],
    );
    m.uvs = Some(vec![[0.0, 1.0], [1.0, 1.0], [1.0, 0.0], [0.0, 0.0]]);
    m
}

/// A large horizontal cloud plane at height `z`.
fn cloud_plane(z: f32, half: f32) -> MeshData {
    let mut m = MeshData::new(
        vec![
            [-half, -half, z],
            [half, -half, z],
            [half, half, z],
            [-half, half, z],
        ],
        vec![0, 1, 2, 0, 2, 3],
    );
    m.uvs = Some(vec![[0.0, 3.0], [3.0, 3.0], [3.0, 0.0], [0.0, 0.0]]);
    m
}

struct Sky {
    dome: WorldModel,
    sun: WorldModel,
    clouds: WorldModel,
    ground: tessera_render::MeshId,
}

fn sky_models(r: &mut Renderer<'static>) -> Result<Sky, RenderError> {
    let disc = upload(
        r,
        64,
        &texels(64, |x, y| {
            let d = ((x as f32 - 31.5).powi(2) + (y as f32 - 31.5).powi(2)).sqrt() / 31.5;
            let core = (1.0 - (d - 0.35).max(0.0) / 0.65).clamp(0.0, 1.0).powi(3);
            [255, 250, 235, (core * 255.0) as u8]
        }),
        Wrap::Clamp,
    )?;
    let cloud = upload(
        r,
        64,
        &texels(64, |x, y| {
            let n = 0.6 * noise(x as f32 / 8.0, y as f32 / 8.0, 8)
                + 0.4 * noise(x as f32 / 3.0, y as f32 / 3.0, 21);
            let a = ((n - 0.45) / 0.3).clamp(0.0, 1.0);
            [255, 255, 255, (a * 220.0) as u8]
        }),
        Wrap::Repeat,
    )?;
    let dome = r.upload_world_model(
        &dome(),
        &[WorldSurface {
            unlit: true,
            vertex_colors: true,
            two_sided: true,
            ..WorldSurface::default()
        }],
    )?;
    let sun = r.upload_world_model(
        &card(800.0, 110.0),
        &[WorldSurface {
            unlit: true,
            alpha: AlphaMode::Additive,
            ..WorldSurface::textured(disc)
        }],
    )?;
    let clouds = r.upload_world_model(
        &cloud_plane(250.0, 1500.0),
        &[WorldSurface {
            unlit: true,
            alpha: AlphaMode::Blend,
            ..WorldSurface::textured(cloud)
        }],
    )?;
    // Ground, tessellated so the per-vertex fog (lighting.md §10) follows the distance.
    let ground = r.upload_mesh(&{
        let n = 60u32;
        let step = 6000.0 / n as f32;
        let mut positions = Vec::new();
        for j in 0..=n {
            for i in 0..=n {
                positions.push([-3000.0 + i as f32 * step, -3000.0 + j as f32 * step, 0.0]);
            }
        }
        let mut indices = Vec::new();
        for j in 0..n {
            for i in 0..n {
                let a = j * (n + 1) + i;
                indices.extend([a, a + 1, a + n + 2, a, a + n + 2, a + n + 1]);
            }
        }
        let mut m = MeshData::new(positions, indices);
        m.normals = Some(vec![[0.0, 0.0, 1.0]; m.positions.len()]);
        m
    })?;
    Ok(Sky {
        dome,
        sun,
        clouds,
        ground,
    })
}

struct TimeOfDay {
    /// Sun (A, B) in degrees, its tint.
    sun: (f32, f32, [f32; 4]),
    dome_tint: [f32; 4],
    clouds: (f32, [f32; 4]),
    light: [f32; 3],
    fog: [f32; 3],
}

fn morning() -> TimeOfDay {
    TimeOfDay {
        sun: (14.0, 150.0, [1.0, 0.85, 0.6, 1.0]),
        dome_tint: [1.0, 0.92, 0.88, 1.0],
        clouds: (20.0, [1.0, 0.9, 0.82, 0.85]),
        light: [0.55, 0.5, 0.42],
        fog: [0.9, 0.82, 0.74],
    }
}

fn evening() -> TimeOfDay {
    TimeOfDay {
        sun: (6.0, 205.0, [1.0, 0.55, 0.3, 1.0]),
        dome_tint: [0.85, 0.6, 0.55, 1.0],
        clouds: (55.0, [0.95, 0.55, 0.45, 0.9]),
        light: [0.5, 0.32, 0.22],
        fog: [0.8, 0.55, 0.45],
    }
}

fn camera() -> Camera {
    Camera {
        eye: Vec3::new(0.0, 0.0, 12.0),
        target: Vec3::new(0.0, -100.0, 22.0),
        up: Vec3::Z,
        projection: Perspective {
            fov_y: 55f32.to_radians(),
            near: 0.5,
            far: 5000.0,
        },
    }
}

fn sky_scene(r: &mut Renderer<'static>, t: &TimeOfDay) -> Result<RgbaImage, RenderError> {
    let sky = sky_models(r)?;
    let (a, b, tint) = t.sun;
    // The sun's direction: the card's +Y axis turned by A then B (render.md §5.4).
    let toward = SkyObject::new(sky.sun.clone(), a, b).rotation() * Vec3::Y;
    let lighting = FrameLighting {
        ambient: [0.3, 0.3, 0.32],
        hemispheric: [0.05, 0.05, 0.05],
        sun: Some(SunLight {
            direction: toward,
            color: t.light,
            specular: [0.0; 3],
        }),
        fog: Some(Fog {
            near: 60.0,
            far: 2500.0,
            color: t.fog,
        }),
        ..FrameLighting::default()
    };
    let target = r.create_offscreen_target(WIDTH, HEIGHT, OffscreenFormat::Rgba8Srgb)?;
    let mut frame = r.begin_offscreen_frame(&target)?;
    frame
        .world_pass("world", &camera())
        .clear(CLEAR)
        .frame_lighting(lighting)
        .mesh(
            MeshDraw::new(
                sky.ground,
                Mat4::IDENTITY,
                Material::color([0.42, 0.45, 0.3, 1.0]),
            )
            .with_lit(LitSurface {
                opacity: Opacity::One,
                ..LitSurface::default()
            }),
        )
        .sky(SkyDraw {
            objects: vec![
                SkyObject {
                    tint: t.dome_tint,
                    scale: 1000.0,
                    ..SkyObject::new(sky.dome.clone(), 0.0, 0.0)
                },
                SkyObject {
                    tint,
                    ..SkyObject::new(sky.sun.clone(), a, b)
                },
                SkyObject {
                    tint: t.clouds.1,
                    ..SkyObject::new(sky.clouds.clone(), 0.0, t.clouds.0)
                },
            ],
        });
    let stats = r.end_frame(frame)?;
    assert_eq!(stats.sky_objects, 3, "{stats:?}");
    r.read_pixels(&target)
}

fn check(name: &str, t: fn() -> TimeOfDay) -> Result<(), RenderError> {
    let Some(mut r) = gpu::headless_renderer(CLEAR) else {
        return Ok(());
    };
    let img = sky_scene(&mut r, &t())?;
    let again = sky_scene(&mut r, &t())?;
    assert!(
        img.data == again.data,
        "{name}: render is not deterministic"
    );
    golden::check(
        &Path::new(env!("CARGO_MANIFEST_DIR")).join(format!("tests/golden/{name}.png")),
        &img,
        golden::Tolerance::default(),
    );
    Ok(())
}

#[test]
fn sky_morning_golden() -> Result<(), RenderError> {
    check("sky_morning", morning)
}

#[test]
fn sky_evening_golden() -> Result<(), RenderError> {
    check("sky_evening", evening)
}

// ---- pixel checks -------------------------------------------------------------------------

fn red_card(r: &mut Renderer<'static>) -> Result<WorldModel, RenderError> {
    r.upload_world_model(
        &card(100.0, 40.0),
        &[WorldSurface {
            unlit: true,
            two_sided: true,
            ..WorldSurface {
                base_color: [1.0, 0.0, 0.0, 1.0],
                ..WorldSurface::default()
            }
        }],
    )
}

fn look(target: Vec3) -> Camera {
    Camera {
        eye: Vec3::ZERO,
        target,
        up: if target.x == 0.0 && target.y == 0.0 {
            Vec3::Y
        } else {
            Vec3::Z
        },
        projection: Perspective {
            fov_y: 30f32.to_radians(),
            near: 0.5,
            far: 50.0,
        },
    }
}

fn centre(
    r: &mut Renderer<'static>,
    camera: &Camera,
    objects: Vec<SkyObject>,
) -> Result<[u8; 4], RenderError> {
    let target = r.create_offscreen_target(16, 16, OffscreenFormat::Rgba8Unorm)?;
    let mut frame = r.begin_offscreen_frame(&target)?;
    frame
        .world_pass("sky", camera)
        .clear([0.0, 0.0, 0.0, 1.0])
        .frame_lighting(FrameLighting::default())
        .sky(SkyDraw { objects });
    r.end_frame(frame)?;
    Ok(r.read_pixels(&target)?.pixel(8, 8).unwrap_or_default())
}

#[test]
fn placement_follows_the_angles() -> Result<(), RenderError> {
    let Some(mut r) = gpu::headless_renderer(CLEAR) else {
        return Ok(());
    };
    let card = red_card(&mut r)?;
    // The card sits on +Y (north) at A = B = 0; far beyond the camera's far plane (100 >
    // 50), it is still drawn (at the far plane).
    let red = [255, 0, 0, 255];
    let black = [0, 0, 0, 255];
    assert_eq!(
        centre(
            &mut r,
            &look(Vec3::Y),
            vec![SkyObject::new(card.clone(), 0.0, 0.0)]
        )?,
        red
    );
    // A = 90: at the zenith.
    assert_eq!(
        centre(
            &mut r,
            &look(Vec3::Z),
            vec![SkyObject::new(card.clone(), 90.0, 0.0)]
        )?,
        red
    );
    assert_eq!(
        centre(
            &mut r,
            &look(Vec3::Y),
            vec![SkyObject::new(card.clone(), 90.0, 0.0)]
        )?,
        black
    );
    // B = 90: east, like a heading.
    assert_eq!(
        centre(
            &mut r,
            &look(Vec3::X),
            vec![SkyObject::new(card.clone(), 0.0, 90.0)]
        )?,
        red
    );
    assert_eq!(
        centre(
            &mut r,
            &look(-Vec3::X),
            vec![SkyObject::new(card.clone(), 0.0, 90.0)]
        )?,
        black
    );
    // Opacity 0 hides the object; half opacity blends.
    let hidden = SkyObject {
        tint: [1.0, 1.0, 1.0, 0.0],
        ..SkyObject::new(card.clone(), 0.0, 0.0)
    };
    assert_eq!(centre(&mut r, &look(Vec3::Y), vec![hidden])?, black);
    let half = SkyObject {
        tint: [1.0, 1.0, 1.0, 0.5],
        ..SkyObject::new(card.clone(), 0.0, 0.0)
    };
    let px = centre(&mut r, &look(Vec3::Y), vec![half])?;
    assert!(px[0].abs_diff(128) <= 2 && px[1] == 0, "{px:?}");
    // Hostile angles and scale.
    let bad = SkyObject {
        tint: [f32::NAN; 4],
        scale: f32::INFINITY,
        ..SkyObject::new(card, f32::NAN, f32::INFINITY)
    };
    assert_eq!(centre(&mut r, &look(Vec3::Y), vec![bad])?, red);
    Ok(())
}

#[test]
fn sky_never_covers_geometry() -> Result<(), RenderError> {
    let Some(mut r) = gpu::headless_renderer(CLEAR) else {
        return Ok(());
    };
    let card = red_card(&mut r)?;
    // A green wall in front of the camera, nearer than the far plane.
    let wall = r.upload_mesh(&MeshData::new(
        vec![
            [-50.0, 10.0, -50.0],
            [50.0, 10.0, -50.0],
            [50.0, 10.0, 50.0],
            [-50.0, 10.0, 50.0],
        ],
        vec![0, 2, 1, 0, 3, 2],
    ))?;
    let green = MeshDraw::new(
        wall,
        Mat4::IDENTITY,
        Material {
            unlit: true,
            two_sided: true,
            ..Material::color([0.0, 1.0, 0.0, 1.0])
        },
    );
    let target = r.create_offscreen_target(16, 16, OffscreenFormat::Rgba8Unorm)?;
    // Sky in the same pass as the wall.
    let mut frame = r.begin_offscreen_frame(&target)?;
    frame
        .world_pass("one pass", &look(Vec3::Y))
        .clear([0.0, 0.0, 0.0, 1.0])
        .frame_lighting(FrameLighting::default())
        .sky(SkyDraw {
            objects: vec![SkyObject::new(card.clone(), 0.0, 0.0)],
        })
        .mesh(green);
    r.end_frame(frame)?;
    assert_eq!(r.read_pixels(&target)?.pixel(8, 8), Some([0, 255, 0, 255]));
    // Sky in its own pass first, the world after it.
    let mut frame = r.begin_offscreen_frame(&target)?;
    frame
        .world_pass("sky", &look(Vec3::Y))
        .clear([0.0, 0.0, 0.0, 1.0])
        .frame_lighting(FrameLighting::default())
        .sky(SkyDraw {
            objects: vec![SkyObject::new(card, 0.0, 0.0)],
        });
    frame
        .world_pass("world", &look(Vec3::Y))
        .clear_depth()
        .frame_lighting(FrameLighting::default())
        .mesh(green);
    r.end_frame(frame)?;
    assert_eq!(r.read_pixels(&target)?.pixel(8, 8), Some([0, 255, 0, 255]));
    // Too many objects is an error.
    let model = r.upload_world_model(&card_mesh_small(), &[])?;
    let mut frame = r.begin_offscreen_frame(&target)?;
    frame
        .world_pass("many", &look(Vec3::Y))
        .frame_lighting(FrameLighting::default())
        .sky(SkyDraw {
            objects: vec![SkyObject::new(model, 0.0, 0.0); tessera_render::MAX_SKY_OBJECTS + 1],
        });
    assert!(matches!(
        r.end_frame(frame),
        Err(RenderError::LimitExceeded { .. })
    ));
    Ok(())
}

fn card_mesh_small() -> MeshData {
    card(10.0, 1.0)
}
