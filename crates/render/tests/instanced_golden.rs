// SPDX-License-Identifier: GPL-3.0-or-later
//! Instanced scenery and frills (M4.5, `docs/specs/scenery.md` §8.3, §8.5, §9;
//! `docs/specs/render.md` §5.5):
//!
//! - `frill_sector.png`: a 10 × 10-unit frill sector with 625 alpha-tested grass clumps
//!   (one instanced draw) that sway, blend toward the terrain colour beneath them and scale
//!   out toward the far distance, next to three instanced rocks casting the sun's shadow;
//! - checks of group culling at the far distance, distance reduction, the per-instance
//!   distance limit, frustum culling, one draw per batch and submesh, thousands of
//!   instances per frame, and hostile input.
//!
//! Regenerate with `TESSERA_BLESS=1 cargo test -p tessera-render --test instanced_golden`.

#[path = "support/golden.rs"]
mod golden;
#[path = "support/gpu.rs"]
mod gpu;

use std::path::Path;
use std::time::Instant;

use tessera_render::glam::{Mat4, Quat, Vec3};
use tessera_render::{
    AlphaMode, Camera, ColorSpace, Filter, Fog, FrameLighting, FrillParams, FrillReduction,
    Instance, InstanceBatch, LitSurface, MAX_INSTANCES_PER_PASS, Material, MeshData, MeshDraw,
    OffscreenFormat, Opacity, Perspective, RenderError, Renderer, RgbaImage, SkyDraw, SkyObject,
    Submesh, SunLight, SunShadow, TextureId, TextureUpload, WorldModel, WorldSurface, Wrap,
};

const WIDTH: u32 = 320;
const HEIGHT: u32 = 200;
const CLEAR: [f32; 4] = [0.62, 0.70, 0.80, 1.0];

fn texels(w: u32, h: u32, f: impl Fn(u32, u32) -> [u8; 4]) -> Vec<u8> {
    (0..w * h).flat_map(|i| f(i % w, i / w)).collect()
}

fn upload(
    r: &mut Renderer<'static>,
    w: u32,
    h: u32,
    data: &[u8],
    wrap: Wrap,
) -> Result<TextureId, RenderError> {
    r.upload_texture(&TextureUpload {
        color_space: ColorSpace::Linear,
        filter: Filter::Linear,
        wrap,
        ..TextureUpload::rgba8(w, h, data)
    })
}

/// A small deterministic generator (xorshift).
struct Rng(u32);

impl Rng {
    fn next(&mut self) -> f32 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 17;
        self.0 ^= self.0 << 5;
        (self.0 >> 8) as f32 / (1u32 << 24) as f32
    }
}

/// Two crossed vertical cards, 1 wide and 1.2 tall, standing on z = 0 (a grass clump).
fn clump() -> MeshData {
    let mut positions = Vec::new();
    let mut normals = Vec::new();
    let mut uvs = Vec::new();
    let mut indices = Vec::new();
    for (dir, n) in [(Vec3::X, Vec3::Y), (Vec3::Y, -Vec3::X)] {
        let base = positions.len() as u32;
        for (u, v, p) in [
            (0.0, 1.0, -0.5 * dir),
            (1.0, 1.0, 0.5 * dir),
            (1.0, 0.0, 0.5 * dir + Vec3::Z * 1.2),
            (0.0, 0.0, -0.5 * dir + Vec3::Z * 1.2),
        ] {
            positions.push(p.to_array());
            // Normals lean up so both sides of a card catch the sun alike.
            normals.push((n * 0.3 + Vec3::Z).normalize().to_array());
            uvs.push([u, v]);
        }
        indices.extend([base, base + 1, base + 2, base, base + 2, base + 3]);
    }
    let mut m = MeshData::new(positions, indices);
    m.normals = Some(normals);
    m.uvs = Some(uvs);
    m
}

/// A unit cube on z = 0 (a rock, scaled per instance).
fn rock() -> MeshData {
    let mut m = MeshData::cube(1.0);
    // The procedural shapes are Y-up; turn it Z-up and stand it on the ground.
    let turn =
        Mat4::from_translation(Vec3::Z * 0.5) * Mat4::from_rotation_x(std::f32::consts::FRAC_PI_2);
    for p in &mut m.positions {
        *p = turn.transform_point3(Vec3::from_array(*p)).to_array();
    }
    if let Some(n) = &mut m.normals {
        for v in n {
            *v = turn.transform_vector3(Vec3::from_array(*v)).to_array();
        }
    }
    m
}

fn ground_mesh(half: f32, cells: u32) -> MeshData {
    let step = 2.0 * half / cells as f32;
    let mut positions = Vec::new();
    let mut uvs = Vec::new();
    for j in 0..=cells {
        for i in 0..=cells {
            let (x, y) = (-half + i as f32 * step, -half + j as f32 * step);
            positions.push([x, y, 0.0]);
            uvs.push([x * 0.25, y * 0.25]);
        }
    }
    let mut indices = Vec::new();
    for j in 0..cells {
        for i in 0..cells {
            let a = j * (cells + 1) + i;
            indices.extend([a, a + 1, a + cells + 2, a, a + cells + 2, a + cells + 1]);
        }
    }
    let mut m = MeshData::new(positions, indices);
    m.normals = Some(vec![[0.0, 0.0, 1.0]; m.positions.len()]);
    m.uvs = Some(uvs);
    m
}

/// The terrain colour of the synthetic ground under (x, y).
fn terrain_color(x: f32, y: f32) -> [f32; 3] {
    let t = (0.5 + 0.5 * (x * 0.35).sin() * (y * 0.27).cos()).clamp(0.0, 1.0);
    [0.35 + 0.3 * t, 0.42 + 0.1 * t, 0.18]
}

struct Scene {
    grass: WorldModel,
    rock: WorldModel,
    ground: tessera_render::MeshId,
    ground_tex: TextureId,
}

fn scene_assets(r: &mut Renderer<'static>) -> Result<Scene, RenderError> {
    // Grass blades: vertical strips, alpha-tested, tips lighter.
    let blades = upload(
        r,
        32,
        32,
        &texels(32, 32, |x, y| {
            let blade = (x * 7 + (x * x) % 5) % 4 == 0 || (x * 3 + 1) % 7 == 0;
            let top = 1.0 - y as f32 / 31.0;
            let height_ok = top < 0.55 + 0.45 * (((x * 37) % 11) as f32 / 10.0);
            let a = if blade && height_ok { 255 } else { 0 };
            [
                (60.0 + 90.0 * top) as u8,
                (120.0 + 80.0 * top) as u8,
                (40.0 + 30.0 * top) as u8,
                a,
            ]
        }),
        Wrap::Clamp,
    )?;
    let grass = r.upload_world_model(
        &clump(),
        &[WorldSurface {
            alpha: AlphaMode::Mask { cutoff: 0.5 },
            two_sided: true,
            ..WorldSurface::textured(blades)
        }],
    )?;
    let rock = r.upload_world_model(
        &rock(),
        &[WorldSurface {
            base_color: [0.55, 0.53, 0.5, 1.0],
            specular: Some(tessera_render::Specular {
                color: [0.3; 3],
                shininess: 12.0,
                texture: None,
            }),
            ..WorldSurface::default()
        }],
    )?;
    let ground = r.upload_mesh(&ground_mesh(30.0, 30))?;
    let ground_tex = upload(
        r,
        16,
        16,
        &texels(16, 16, |x, y| {
            let v = if (x / 4 + y / 4) % 2 == 0 { 150 } else { 130 };
            [v, (v as f32 * 0.85) as u8, (v as f32 * 0.6) as u8, 255]
        }),
        Wrap::Repeat,
    )?;
    Ok(Scene {
        grass,
        rock,
        ground,
        ground_tex,
    })
}

/// 25 × 25 clumps jittered over the sector (0..10, 0..10).
fn sector_instances() -> Vec<Instance> {
    let mut rng = Rng(0x2468_ace1);
    let mut out = Vec::with_capacity(625);
    for j in 0..25 {
        for i in 0..25 {
            let x = (i as f32 + rng.next()) * 0.4;
            let y = (j as f32 + rng.next()) * 0.4;
            let yaw = rng.next() * std::f32::consts::TAU;
            let s = 0.7 + 0.6 * rng.next();
            out.push(Instance {
                transform: Mat4::from_scale_rotation_translation(
                    Vec3::new(s, s, s * (0.8 + 0.4 * rng.next())),
                    Quat::from_rotation_z(yaw),
                    Vec3::new(x, y, 0.0),
                ),
                tint: [1.0; 4],
                terrain_color: terrain_color(x, y),
            });
        }
    }
    out
}

fn lighting() -> FrameLighting {
    let toward = Vec3::new(0.45, 0.75, 0.5).normalize();
    FrameLighting {
        ambient: [0.27, 0.29, 0.32],
        hemispheric: [0.08, 0.08, 0.06],
        sun: Some(SunLight {
            direction: toward,
            color: [0.55, 0.52, 0.46],
            specular: [0.6, 0.6, 0.55],
        }),
        fog: Some(Fog {
            near: 20.0,
            far: 60.0,
            color: [CLEAR[0], CLEAR[1], CLEAR[2]],
        }),
        shadow: Some(SunShadow::fit(
            toward,
            Vec3::new(5.0, 6.0, 0.5),
            12.0,
            60.0,
            1024,
        )),
        time: 1.3,
        ..FrameLighting::default()
    }
}

fn camera() -> Camera {
    Camera {
        eye: Vec3::new(-2.5, -3.5, 2.6),
        target: Vec3::new(5.5, 6.0, 0.0),
        up: Vec3::Z,
        projection: Perspective {
            fov_y: 55f32.to_radians(),
            near: 0.1,
            far: 200.0,
        },
    }
}

fn frill_scene(r: &mut Renderer<'static>) -> Result<RgbaImage, RenderError> {
    let s = scene_assets(r)?;
    let grass = InstanceBatch::frills(
        s.grass.clone(),
        sector_instances(),
        FrillParams {
            far_distance: 15.0,
            fade_length: 5.0,
            reduction: None,
            animation_intensity: 0.2,
            terrain_color_intensity: 0.8,
            ..FrillParams::default()
        },
    );
    let rocks = InstanceBatch::new(
        s.rock.clone(),
        vec![
            Instance::new(Mat4::from_scale_rotation_translation(
                Vec3::new(1.4, 0.9, 0.8),
                Quat::from_rotation_z(0.4),
                Vec3::new(3.0, 7.5, 0.0),
            )),
            Instance::new(Mat4::from_scale_rotation_translation(
                Vec3::new(0.7, 0.8, 1.6),
                Quat::from_rotation_z(-0.3),
                Vec3::new(6.5, 3.2, 0.0),
            )),
            Instance {
                tint: [0.8, 0.9, 1.0, 1.0],
                ..Instance::new(Mat4::from_scale_rotation_translation(
                    Vec3::splat(0.6),
                    Quat::from_rotation_z(1.0),
                    Vec3::new(1.2, 2.0, 0.0),
                ))
            },
        ],
    );
    let target = r.create_offscreen_target(WIDTH, HEIGHT, OffscreenFormat::Rgba8Srgb)?;
    let mut frame = r.begin_offscreen_frame(&target)?;
    frame
        .world_pass("frills", &camera())
        .clear(CLEAR)
        .frame_lighting(lighting())
        .mesh(
            MeshDraw::new(s.ground, Mat4::IDENTITY, Material::textured(s.ground_tex)).with_lit(
                LitSurface {
                    opacity: Opacity::One,
                    ..LitSurface::default()
                },
            ),
        )
        .instanced(grass)
        .instanced(rocks);
    let stats = r.end_frame(frame)?;
    // One draw for the grass, one for the rocks (plus the ground and the rocks' shadow).
    assert_eq!(
        stats.instances,
        625 + 3 - stats.instances_culled,
        "{stats:?}"
    );
    assert!(stats.instances_culled < 100, "{stats:?}");
    r.read_pixels(&target)
}

#[test]
fn frill_sector_golden() -> Result<(), RenderError> {
    let Some(mut r) = gpu::headless_renderer(CLEAR) else {
        return Ok(());
    };
    let img = frill_scene(&mut r)?;
    let again = frill_scene(&mut r)?;
    assert!(img.data == again.data, "render is not deterministic");
    golden::check(
        &Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/golden/frill_sector.png"),
        &img,
        golden::Tolerance::default(),
    );
    Ok(())
}

// ---- behaviour ----------------------------------------------------------------------------

fn draw_batches(
    r: &mut Renderer<'static>,
    camera: &Camera,
    batches: Vec<InstanceBatch>,
) -> Result<tessera_render::FrameStats, RenderError> {
    let target = r.create_offscreen_target(64, 64, OffscreenFormat::Rgba8Srgb)?;
    let mut frame = r.begin_offscreen_frame(&target)?;
    let pass = frame
        .world_pass("batches", camera)
        .clear(CLEAR)
        .frame_lighting(lighting());
    for b in batches {
        pass.instanced(b);
    }
    r.end_frame(frame)
}

fn row(n: usize, spacing: f32, start: Vec3) -> Vec<Instance> {
    (0..n)
        .map(|i| Instance::new(Mat4::from_translation(start + Vec3::X * spacing * i as f32)))
        .collect()
}

#[test]
fn frill_groups_are_culled_and_reduced_by_distance() -> Result<(), RenderError> {
    let Some(mut r) = gpu::headless_renderer(CLEAR) else {
        return Ok(());
    };
    let s = scene_assets(&mut r)?;
    let cam = Camera {
        eye: Vec3::new(0.0, -5.0, 2.0),
        target: Vec3::new(0.0, 30.0, 0.0),
        up: Vec3::Z,
        projection: Perspective {
            fov_y: 70f32.to_radians(),
            near: 0.1,
            far: 500.0,
        },
    };
    let params = FrillParams {
        far_distance: 97.0,
        fade_length: 30.0,
        reduction: Some(FrillReduction {
            distance: 40.0,
            granule: 4,
        }),
        ..FrillParams::default()
    };
    // A group of 40 clumps 1 unit wide, near: all drawn.
    let near = row(40, 0.02, Vec3::new(-0.4, 5.0, 0.0));
    let stats = draw_batches(
        &mut r,
        &cam,
        vec![InstanceBatch::frills(s.grass.clone(), near.clone(), params)],
    )?;
    assert_eq!(stats.instances, 40, "{stats:?}");
    // The same group beyond D: not drawn at all.
    let far: Vec<Instance> = row(40, 0.02, Vec3::new(-0.4, 150.0, 0.0));
    let stats = draw_batches(
        &mut r,
        &cam,
        vec![InstanceBatch::frills(s.grass.clone(), far, params)],
    )?;
    assert_eq!(
        (stats.instances, stats.instances_culled),
        (0, 40),
        "{stats:?}"
    );
    // Half way between Rd and D in squared distance: half the granules (rule 4).
    let d = ((40.0f32 * 40.0 + 97.0 * 97.0) * 0.5).sqrt();
    let mid = row(40, 0.0, Vec3::new(0.0, -5.0 + d + 0.6, 0.0));
    let stats = draw_batches(
        &mut r,
        &cam,
        vec![InstanceBatch::frills(s.grass.clone(), mid.clone(), params)],
    )?;
    let expected = tessera_render::frill_reduced_count(
        40,
        (d - 0.1).powi(2),
        97.0,
        params.reduction.unwrap_or(FrillReduction {
            distance: 40.0,
            granule: 4,
        }),
    );
    assert!(
        stats.instances <= 24 && stats.instances >= 16,
        "{stats:?} vs about {expected}"
    );
    // Not reducible: everything.
    let stats = draw_batches(
        &mut r,
        &cam,
        vec![InstanceBatch::frills(
            s.grass.clone(),
            mid,
            FrillParams {
                reduction: None,
                ..params
            },
        )],
    )?;
    assert_eq!(stats.instances, 40, "{stats:?}");
    // Scenery: the distance limit and frustum culling act per instance.
    let mut rocks: Vec<Instance> = (0..10)
        .map(|i| {
            Instance::new(Mat4::from_translation(Vec3::new(
                0.0,
                10.0 + 10.0 * i as f32,
                0.0,
            )))
        })
        .collect();
    rocks.extend(row(5, 1.0, Vec3::new(0.0, -50.0, 0.0))); // behind the camera
    let stats = draw_batches(
        &mut r,
        &cam,
        vec![InstanceBatch {
            max_distance: Some(35.0),
            ..InstanceBatch::new(s.rock.clone(), rocks)
        }],
    )?;
    // Rocks at y = 10, 20, 30 (15, 25 and 35 units away) start within 35 units.
    assert_eq!(stats.instances, 3, "{stats:?}");
    assert_eq!(stats.instances_culled, 12, "{stats:?}");
    Ok(())
}

#[test]
fn thousands_of_instances_per_frame() -> Result<(), RenderError> {
    let Some(mut r) = gpu::headless_renderer(CLEAR) else {
        return Ok(());
    };
    let s = scene_assets(&mut r)?;
    // 64 sectors of 625 clumps: 40,000 instances in 64 groups, plus 2,000 rocks.
    let base = sector_instances();
    let mut batches = Vec::new();
    for sy in 0..8 {
        for sx in 0..8 {
            let shift =
                Mat4::from_translation(Vec3::new(sx as f32 * 10.0 - 40.0, sy as f32 * 10.0, 0.0));
            let inst: Vec<Instance> = base
                .iter()
                .map(|i| Instance {
                    transform: shift * i.transform,
                    ..*i
                })
                .collect();
            batches.push(InstanceBatch::frills(
                s.grass.clone(),
                inst,
                FrillParams {
                    far_distance: 1_000.0,
                    fade_length: 30.0,
                    reduction: None,
                    animation_intensity: 0.15,
                    ..FrillParams::default()
                },
            ));
        }
    }
    let rocks: Vec<Instance> = (0..2_000)
        .map(|i| {
            Instance::new(Mat4::from_translation(Vec3::new(
                (i % 50) as f32 * 1.6 - 40.0,
                (i / 50) as f32 * 2.0,
                0.0,
            )))
        })
        .collect();
    batches.push(InstanceBatch::new(s.rock.clone(), rocks));
    let cam = Camera {
        eye: Vec3::new(0.0, -20.0, 30.0),
        target: Vec3::new(0.0, 40.0, 0.0),
        up: Vec3::Z,
        projection: Perspective {
            fov_y: 80f32.to_radians(),
            near: 0.1,
            far: 500.0,
        },
    };
    // Warm up pipelines, then time the CPU side of a frame: queuing the batches and
    // `end_frame` (culling, packing, recording, submission; the GPU is not waited for).
    draw_batches(&mut r, &cam, batches.clone())?;
    let target = r.create_offscreen_target(64, 64, OffscreenFormat::Rgba8Srgb)?;
    let mut best = f64::INFINITY;
    let mut stats = tessera_render::FrameStats::default();
    for _ in 0..5 {
        let t = Instant::now();
        let mut frame = r.begin_offscreen_frame(&target)?;
        let pass = frame
            .world_pass("timed", &cam)
            .clear(CLEAR)
            .frame_lighting(lighting());
        for b in &batches {
            pass.instanced(b.clone());
        }
        stats = r.end_frame(frame)?;
        best = best.min(t.elapsed().as_secs_f64() * 1e3);
    }
    eprintln!(
        "instanced: {} instances ({} culled) in {} draw calls, CPU {:.2} ms per frame (best of 5) on {}",
        stats.instances,
        stats.instances_culled,
        stats.draw_calls,
        best,
        r.adapter_info().name
    );
    assert!(stats.instances > 30_000, "{stats:?}");
    // 64 grass groups + the rocks (+ the rocks' shadow draw).
    assert!(stats.draw_calls <= 66, "{stats:?}");
    Ok(())
}

#[test]
fn hostile_instances_and_limits() -> Result<(), RenderError> {
    let Some(mut r) = gpu::headless_renderer(CLEAR) else {
        return Ok(());
    };
    let s = scene_assets(&mut r)?;
    let mut inst = row(3, 1.0, Vec3::new(0.0, 5.0, 0.0));
    inst.push(Instance::new(Mat4::from_cols_array(&[f32::NAN; 16])));
    inst.push(Instance {
        tint: [f32::NAN; 4],
        terrain_color: [f32::INFINITY; 3],
        ..Instance::new(Mat4::from_translation(Vec3::new(0.0, 6.0, 0.0)))
    });
    let stats = draw_batches(
        &mut r,
        &camera(),
        vec![
            InstanceBatch::frills(
                s.grass.clone(),
                inst.clone(),
                FrillParams {
                    far_distance: f32::NAN,
                    fade_length: f32::INFINITY,
                    fade_in: f32::NAN,
                    animation_intensity: 9.0,
                    terrain_color_intensity: -3.0,
                    reduction: Some(FrillReduction {
                        distance: f32::NAN,
                        granule: 0,
                    }),
                    opaque: false,
                    sway_z_range: Some([f32::NAN, f32::INFINITY]),
                },
            ),
            InstanceBatch {
                max_distance: Some(f32::NAN),
                ..InstanceBatch::new(s.rock.clone(), inst)
            },
        ],
    )?;
    // The non-finite transform is dropped from the rock batch; the frill group with a NaN
    // far distance is culled whole.
    assert_eq!(stats.instances, 4, "{stats:?}");
    // Surfaces past the model's list reuse the last; an empty list uses the default.
    let model = r.upload_world_model(&clump(), &[])?;
    assert_eq!(model.surface_count(), 1);
    let too_many = vec![WorldSurface::default(); tessera_render::MAX_MODEL_SURFACES + 1];
    assert!(matches!(
        r.upload_world_model(&clump(), &too_many),
        Err(RenderError::LimitExceeded { .. })
    ));
    assert!(matches!(
        r.upload_world_model(&MeshData::new(vec![[0.0; 3]; 3], vec![0, 1, 7]), &[]),
        Err(RenderError::InvalidMesh(_))
    ));
    Ok(())
}

// ---- regressions ---------------------------------------------------------------------------

/// A horizontal 4 × 4 quad at z = 0 facing up, with tangents along +x.
fn tile() -> MeshData {
    let mut m = MeshData::new(
        vec![
            [-2.0, -2.0, 0.0],
            [2.0, -2.0, 0.0],
            [2.0, 2.0, 0.0],
            [-2.0, 2.0, 0.0],
        ],
        vec![0, 1, 2, 0, 2, 3],
    );
    m.normals = Some(vec![[0.0, 0.0, 1.0]; 4]);
    m.tangents = Some(vec![[1.0, 0.0, 0.0, 1.0]; 4]);
    m.uvs = Some(vec![[0.0, 1.0], [1.0, 1.0], [1.0, 0.0], [0.0, 0.0]]);
    m
}

/// A 20 × 20 vertical wall 5 units north of the origin, facing south.
fn wall() -> MeshData {
    let mut m = MeshData::new(
        vec![
            [-10.0, 5.0, -10.0],
            [10.0, 5.0, -10.0],
            [10.0, 5.0, 10.0],
            [-10.0, 5.0, 10.0],
        ],
        vec![0, 1, 2, 0, 2, 3],
    );
    m.normals = Some(vec![[0.0, -1.0, 0.0]; 4]);
    m
}

fn looking_north() -> Camera {
    Camera {
        eye: Vec3::ZERO,
        target: Vec3::Y,
        up: Vec3::Z,
        ..Camera::default()
    }
}

#[test]
fn offscreen_casters_still_cast() -> Result<(), RenderError> {
    let Some(mut r) = gpu::headless_renderer(CLEAR) else {
        return Ok(());
    };
    let s = scene_assets(&mut r)?;
    // The sun is behind the camera: a pillar behind the camera, outside the view, throws
    // its shadow forward onto the visible ground (render.md §4.4: casters are not culled
    // by the view).
    let toward = Vec3::new(0.0, -1.0, 0.15).normalize();
    let lighting = FrameLighting {
        ambient: [0.2; 3],
        hemispheric: [0.0; 3],
        sun: Some(SunLight {
            direction: toward,
            color: [1.0; 3],
            specular: [0.0; 3],
        }),
        shadow: Some(SunShadow::fit(
            toward,
            Vec3::new(0.0, 5.0, 0.0),
            30.0,
            80.0,
            1024,
        )),
        ..FrameLighting::default()
    };
    let cam = Camera {
        eye: Vec3::new(0.0, -10.0, 6.0),
        target: Vec3::new(0.0, 10.0, 0.0),
        up: Vec3::Z,
        projection: Perspective {
            fov_y: 60f32.to_radians(),
            near: 0.1,
            far: 200.0,
        },
    };
    // A tall pillar, far enough behind the camera that its scaled bounding sphere (radius ×
    // the largest scale) is outside the view.
    let pillar = Instance::new(Mat4::from_scale_rotation_translation(
        Vec3::new(1.0, 1.0, 10.0),
        Quat::IDENTITY,
        Vec3::new(0.0, -80.0, 0.0),
    ));
    let mut shade =
        |casters: Vec<Instance>| -> Result<(u8, tessera_render::FrameStats), RenderError> {
            let target = r.create_offscreen_target(64, 64, OffscreenFormat::Rgba8Unorm)?;
            let mut frame = r.begin_offscreen_frame(&target)?;
            frame
                .world_pass("caster", &cam)
                .clear(CLEAR)
                .frame_lighting(lighting.clone())
                .mesh(
                    MeshDraw::new(
                        s.ground,
                        Mat4::IDENTITY,
                        Material::color([0.7, 0.7, 0.7, 1.0]),
                    )
                    .with_lit(LitSurface {
                        opacity: Opacity::One,
                        cast_shadow: false,
                        ..LitSurface::default()
                    }),
                )
                .instanced(InstanceBatch::new(s.rock.clone(), casters));
            let stats = r.end_frame(frame)?;
            let px = r.read_pixels(&target)?.pixel(32, 40).unwrap_or_default();
            Ok((px[0], stats))
        };
    let (lit, _) = shade(vec![])?;
    let (shadowed, stats) = shade(vec![pillar])?;
    assert_eq!(
        (stats.instances, stats.instances_culled),
        (0, 1),
        "{stats:?}"
    );
    assert!(stats.shadow_draws >= 1, "{stats:?}");
    assert!(
        shadowed + 20 < lit,
        "ground {lit} without the caster, {shadowed} with it"
    );
    Ok(())
}

#[test]
fn unknown_normal_maps_render_as_none() -> Result<(), RenderError> {
    let Some(mut r) = gpu::headless_renderer(CLEAR) else {
        return Ok(());
    };
    let gone = upload(&mut r, 1, 1, &[1, 2, 3, 4], Wrap::Repeat)?;
    assert!(r.destroy_texture(gone));
    let tilted = upload(&mut r, 1, 1, &[230, 128, 200, 255], Wrap::Repeat)?;
    let cam = Camera {
        eye: Vec3::new(0.0, 0.0, 10.0),
        target: Vec3::ZERO,
        up: Vec3::Y,
        ..Camera::default()
    };
    let lighting = FrameLighting {
        ambient: [0.0; 3],
        hemispheric: [0.0; 3],
        sun: Some(SunLight {
            direction: Vec3::Z,
            color: [0.4; 3],
            specular: [0.0; 3],
        }),
        ..FrameLighting::default()
    };
    let mut centre = |normal: Option<TextureId>| -> Result<[u8; 4], RenderError> {
        let model = r.upload_world_model(
            &tile(),
            &[WorldSurface {
                normal_texture: normal,
                two_sided: true,
                ..WorldSurface::default()
            }],
        )?;
        let target = r.create_offscreen_target(16, 16, OffscreenFormat::Rgba8Unorm)?;
        let mut frame = r.begin_offscreen_frame(&target)?;
        frame
            .world_pass("normal", &cam)
            .clear([0.0, 0.0, 0.0, 1.0])
            .frame_lighting(lighting.clone())
            .instanced(InstanceBatch::new(
                model,
                vec![Instance::new(Mat4::IDENTITY)],
            ));
        r.end_frame(frame)?;
        Ok(r.read_pixels(&target)?.pixel(8, 8).unwrap_or_default())
    };
    let none = centre(None)?;
    let unknown = centre(Some(gone))?;
    let mapped = centre(Some(tilted))?;
    assert!(
        (0..3).all(|i| none[i].abs_diff(unknown[i]) <= 1),
        "no map {none:?} vs unknown map {unknown:?}"
    );
    // A real map does change the shading (the check is not vacuous).
    assert!(none[0].abs_diff(mapped[0]) > 10, "{none:?} vs {mapped:?}");
    Ok(())
}

#[test]
fn destroyed_textures_keep_models_and_sky_drawing() -> Result<(), RenderError> {
    let Some(mut r) = gpu::headless_renderer(CLEAR) else {
        return Ok(());
    };
    let red = upload(&mut r, 1, 1, &[255, 0, 0, 255], Wrap::Repeat)?;
    let model = r.upload_world_model(
        &wall(),
        &[WorldSurface {
            unlit: true,
            two_sided: true,
            ..WorldSurface::textured(red)
        }],
    )?;
    let centre = |r: &mut Renderer<'static>, sky: bool| -> Result<[u8; 4], RenderError> {
        let target = r.create_offscreen_target(16, 16, OffscreenFormat::Rgba8Unorm)?;
        let mut frame = r.begin_offscreen_frame(&target)?;
        let pass = frame
            .world_pass("retained", &looking_north())
            .clear([0.0, 0.0, 1.0, 1.0])
            .frame_lighting(FrameLighting::default());
        if sky {
            pass.sky(SkyDraw {
                objects: vec![SkyObject::new(model.clone(), 0.0, 0.0)],
            });
        } else {
            pass.instanced(InstanceBatch::new(
                model.clone(),
                vec![Instance::new(Mat4::IDENTITY)],
            ));
        }
        r.end_frame(frame)?;
        Ok(r.read_pixels(&target)?.pixel(8, 8).unwrap_or_default())
    };
    for sky in [false, true] {
        assert_eq!(centre(&mut r, sky)?, [255, 0, 0, 255], "sky {sky}");
    }
    // Destroyed while the model is still drawn: the frame must not fail (a destroyed
    // texture in a bind group fails the whole submission); the surface draws as if the
    // texture were absent (white).
    assert!(r.destroy_texture(red));
    for sky in [false, true] {
        assert_eq!(centre(&mut r, sky)?, [255, 255, 255, 255], "sky {sky}");
    }
    Ok(())
}

#[test]
fn instances_count_once_per_batch() -> Result<(), RenderError> {
    let Some(mut r) = gpu::headless_renderer(CLEAR) else {
        return Ok(());
    };
    // Two submeshes: two draws per batch, but each instance is counted once.
    let mut m = MeshData::new(
        vec![
            [0.0, 0.0, 0.0],
            [0.5, 0.0, 0.0],
            [0.0, 0.0, 0.5],
            [0.0, 0.2, 0.0],
            [0.5, 0.2, 0.0],
            [0.0, 0.2, 0.5],
        ],
        vec![0, 1, 2, 3, 4, 5],
    );
    m.submeshes = vec![
        Submesh {
            first_index: 0,
            index_count: 3,
        },
        Submesh {
            first_index: 3,
            index_count: 3,
        },
    ];
    let model = r.upload_world_model(&m, &[WorldSurface::default()])?;
    let stats = draw_batches(
        &mut r,
        &camera(),
        vec![InstanceBatch::new(
            model,
            row(5, 0.5, Vec3::new(3.0, 4.0, 0.0)),
        )],
    )?;
    assert_eq!(stats.instances, 5, "{stats:?}");
    assert_eq!(stats.instances_culled, 0, "{stats:?}");
    Ok(())
}

#[test]
fn instance_limit_drops_instead_of_failing() -> Result<(), RenderError> {
    let Some(mut r) = gpu::headless_renderer(CLEAR) else {
        return Ok(());
    };
    let tri = r.upload_world_model(
        &MeshData::new(
            vec![[0.0; 3], [0.01, 0.0, 0.0], [0.0, 0.0, 0.01]],
            vec![0, 1, 2],
        ),
        &[WorldSurface {
            two_sided: true,
            ..WorldSurface::default()
        }],
    )?;
    // Over the per-pass limit in view: the excess is dropped and counted as culled, and
    // the frame still succeeds (render.md §5.5).
    let half = MAX_INSTANCES_PER_PASS / 2 + 5;
    let inst: std::sync::Arc<[Instance]> =
        vec![Instance::new(Mat4::from_translation(Vec3::new(3.0, 4.0, 0.0))); half].into();
    let batch = InstanceBatch {
        cast_shadow: false,
        ..InstanceBatch::new(tri, inst)
    };
    let stats = draw_batches(&mut r, &camera(), vec![batch.clone(), batch])?;
    assert_eq!(
        stats.instances as usize, MAX_INSTANCES_PER_PASS,
        "{stats:?}"
    );
    assert_eq!(stats.instances_culled, 10, "{stats:?}");
    Ok(())
}
