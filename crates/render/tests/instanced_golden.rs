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
    Submesh, SunLight, SunShadow, TextureId, TextureUpload, WorldModel, WorldSurface,
    WorldSurfaceOptions, Wrap,
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
                light_object: None,
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
    // Rule 4 measures the squared distance from the camera to the group's bounding box (the
    // box around the instances' spheres), not to its centre.
    let red = params.reduction.unwrap_or(FrillReduction {
        distance: 40.0,
        granule: 4,
    });
    let sphere = s.grass.info().sphere;
    let (mut lo, mut hi) = (Vec3::splat(f32::INFINITY), Vec3::splat(f32::NEG_INFINITY));
    for i in &mid {
        let t = sphere.transformed(i.transform);
        lo = lo.min(t.center - t.radius);
        hi = hi.max(t.center + t.radius);
    }
    let box_d2 = ((lo - cam.eye).max(Vec3::ZERO).max(cam.eye - hi)).length_squared();
    let expected = tessera_render::frill_reduced_count(40, box_d2, 97.0, red);
    let centre = sphere.transformed(mid[0].transform).center;
    let by_centre =
        tessera_render::frill_reduced_count(40, centre.distance_squared(cam.eye), 97.0, red);
    assert_ne!(
        expected, by_centre,
        "the check must tell the two distances apart"
    );
    assert_eq!(stats.instances as usize, expected, "{stats:?}");
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
    let hostile = FrillParams {
        far_distance: 97.0,
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
    };
    let stats = draw_batches(
        &mut r,
        &camera(),
        vec![
            // A finite far distance, so the other hostile fields are read: fade length,
            // fade-in, sway and terrain intensities, reduction, the sway range and the
            // non-opaque (blended) override.
            InstanceBatch::frills(s.grass.clone(), inst.clone(), hostile),
            InstanceBatch::frills(
                s.grass.clone(),
                inst.clone(),
                FrillParams {
                    far_distance: f32::NAN,
                    ..hostile
                },
            ),
            InstanceBatch {
                max_distance: Some(f32::NAN),
                ..InstanceBatch::new(s.rock.clone(), inst)
            },
        ],
    )?;
    // The non-finite transform is dropped from the frill group and the rock batch (4 drawn
    // each); the frill group with a NaN far distance is culled whole.
    assert_eq!(stats.instances, 8, "{stats:?}");
    assert_eq!(stats.instances_culled, 2 + 5, "{stats:?}");
    // A group beyond D stays culled with a non-finite instance: that instance does not
    // stretch the group's box (its sphere falling back to the model's, at the origin, near
    // the camera).
    let mut far = row(3, 1.0, Vec3::new(0.0, 150.0, 0.0));
    far.push(Instance::new(Mat4::from_cols_array(&[f32::NAN; 16])));
    let stats = draw_batches(
        &mut r,
        &camera(),
        vec![InstanceBatch::frills(
            s.grass.clone(),
            far,
            FrillParams::default(),
        )],
    )?;
    assert_eq!(
        (stats.instances, stats.instances_culled),
        (0, 4),
        "{stats:?}"
    );
    // Only non-finite instances: nothing to draw, nothing fails.
    let stats = draw_batches(
        &mut r,
        &camera(),
        vec![InstanceBatch::frills(
            s.grass.clone(),
            vec![Instance::new(Mat4::from_cols_array(&[f32::NAN; 16])); 2],
            hostile,
        )],
    )?;
    assert_eq!(
        (stats.instances, stats.instances_culled),
        (0, 2),
        "{stats:?}"
    );
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

// ---- conformance fixes (render.md §2.3, §4.3, §5.5) ------------------------------------------

/// A top-down camera over the origin (x right, y up on screen) and its 32 × 32 target.
fn overhead() -> Camera {
    Camera {
        eye: Vec3::new(0.0, 0.0, 10.0),
        target: Vec3::ZERO,
        up: Vec3::Y,
        projection: Perspective {
            fov_y: 60f32.to_radians(),
            near: 0.1,
            far: 100.0,
        },
    }
}

const OVERHEAD: u32 = 32;

/// The pixel of [`overhead`] where world point `p` lands.
fn overhead_pixel(p: Vec3) -> (u32, u32) {
    let c = overhead().view_proj(1.0).project_point3(p);
    let f = OVERHEAD as f32;
    (
        ((c.x * 0.5 + 0.5) * f).clamp(0.0, f - 1.0) as u32,
        ((0.5 - c.y * 0.5) * f).clamp(0.0, f - 1.0) as u32,
    )
}

/// Draws `batches` with [`overhead`] under `lighting` (and the table lights' `reach`).
fn draw_overhead(
    r: &mut Renderer<'static>,
    batches: Vec<InstanceBatch>,
    lighting: FrameLighting,
    reach: Vec<tessera_render::LightReach>,
) -> Result<(RgbaImage, tessera_render::FrameStats), RenderError> {
    let target = r.create_offscreen_target(OVERHEAD, OVERHEAD, OffscreenFormat::Rgba8Unorm)?;
    let mut frame = r.begin_offscreen_frame(&target)?;
    let pass = frame
        .world_pass("overhead", &overhead())
        .clear([0.0, 0.0, 0.0, 1.0])
        .frame_lighting(lighting)
        .light_reach(reach);
    for b in batches {
        pass.instanced(b);
    }
    let stats = r.end_frame(frame)?;
    Ok((r.read_pixels(&target)?, stats))
}

fn at(img: &RgbaImage, p: Vec3) -> [u8; 4] {
    let (x, y) = overhead_pixel(p);
    img.pixel(x, y).unwrap_or_default()
}

/// No light: unlit surfaces show their colour as it is.
fn dark() -> FrameLighting {
    FrameLighting {
        ambient: [0.0; 3],
        hemispheric: [0.0; 3],
        sun: None,
        ..FrameLighting::default()
    }
}

/// A one-sided tile scaled to 1 × 1 at `p`, with `extra` applied first (mirrors).
fn small_tile(p: Vec3, extra: Mat4) -> Instance {
    Instance::new(Mat4::from_translation(p) * Mat4::from_scale(Vec3::splat(0.25)) * extra)
}

/// render.md §2.3 (Mirrored transforms), through §5.5 "as the lit mesh pipeline": an instance
/// with a negative determinant culls the other face.
#[test]
fn mirrored_instances_swap_culling() -> Result<(), RenderError> {
    let Some(mut r) = gpu::headless_renderer(CLEAR) else {
        return Ok(());
    };
    let surface = WorldSurface {
        unlit: true,
        base_color: [1.0, 1.0, 1.0, 1.0],
        ..WorldSurface::default()
    };
    let one_sided = r.upload_world_model(&tile(), &[surface])?;
    let two_sided = r.upload_world_model(
        &tile(),
        &[WorldSurface {
            two_sided: true,
            ..surface
        }],
    )?;
    let mirror_x = Mat4::from_scale(Vec3::new(-1.0, 1.0, 1.0));
    let mirror_z = Mat4::from_scale(Vec3::new(1.0, 1.0, -1.0));
    let (a, b, c) = (
        Vec3::new(-2.0, 0.0, 0.0),
        Vec3::new(0.0, 0.0, 0.0),
        Vec3::new(2.0, 0.0, 0.0),
    );
    // One batch: upright, mirrored in x (still facing up: drawn), mirrored in z (the same
    // points, now facing down: its front face is away from the camera, culled).
    let batch = |model: &WorldModel| {
        InstanceBatch::new(
            model.clone(),
            vec![
                small_tile(a, Mat4::IDENTITY),
                small_tile(b, mirror_x),
                small_tile(c, mirror_z),
            ],
        )
    };
    let (img, stats) = draw_overhead(&mut r, vec![batch(&one_sided)], dark(), vec![])?;
    assert_eq!(stats.instances, 3, "{stats:?}");
    let white = [255, 255, 255, 255];
    let black = [0, 0, 0, 255];
    assert_eq!(at(&img, a), white, "upright");
    assert_eq!(at(&img, b), white, "mirrored in x, facing the camera");
    assert_eq!(at(&img, c), black, "mirrored in z, facing away");
    // Two-sided: no culling, all three drawn.
    let (img, _) = draw_overhead(&mut r, vec![batch(&two_sided)], dark(), vec![])?;
    for p in [a, b, c] {
        assert_eq!(at(&img, p), white, "two-sided at {p}");
    }
    Ok(())
}

/// lighting.md §8.5 (row L32): each instance gets its own light list, against its own world
/// box, with the parent switches by its object handle (render.md §4.3, R-D9 retired).
#[test]
fn instances_get_their_own_lights() -> Result<(), RenderError> {
    let Some(mut r) = gpu::headless_renderer(CLEAR) else {
        return Ok(());
    };
    let model = r.upload_world_model(
        &tile(),
        &[WorldSurface {
            base_color: [1.0, 1.0, 1.0, 1.0],
            ..WorldSurface::default()
        }],
    )?;
    let (left, right) = (Vec3::new(-2.0, 0.0, 0.0), Vec3::new(2.0, 0.0, 0.0));
    // A light 1 above the right tile, with a long range (it would light the left tile too)
    // but a selection radius of 1.5: only the right tile's box is within it.
    let light = tessera_render::Light::point(right + Vec3::Z, 100.0, 0.0, [0.5; 3]);
    let lighting = FrameLighting {
        lights: vec![light],
        ..dark()
    };
    let reach = tessera_render::LightReach {
        radius: Some(1.5),
        ..tessera_render::LightReach::default()
    };
    let instances = vec![
        small_tile(left, Mat4::IDENTITY),
        small_tile(right, Mat4::IDENTITY),
    ];
    let (img, _) = draw_overhead(
        &mut r,
        vec![InstanceBatch::new(model.clone(), instances.clone())],
        lighting.clone(),
        vec![reach],
    )?;
    assert!(at(&img, right)[0] > 100, "right {:?}", at(&img, right));
    assert_eq!(at(&img, left), [0, 0, 0, 255], "left");
    // The parent switches act per instance: a light that reaches only its parent (object
    // 7) lights the instance carrying that handle, and no other, even within reach.
    let only_parent = tessera_render::LightReach {
        radius: Some(100.0),
        only_influence_parent: true,
        ..tessera_render::LightReach::default()
    }
    .with_parent(7);
    let tagged = vec![
        Instance {
            light_object: Some(7),
            ..instances[0]
        },
        Instance {
            light_object: Some(8),
            ..instances[1]
        },
    ];
    let (img, _) = draw_overhead(
        &mut r,
        vec![InstanceBatch::new(model.clone(), tagged)],
        lighting.clone(),
        vec![only_parent],
    )?;
    assert!(at(&img, left)[0] > 20, "parent {:?}", at(&img, left));
    assert_eq!(at(&img, right), [0, 0, 0, 255], "not the parent");
    // An explicit list applies to every instance.
    let listed = InstanceBatch {
        lights: tessera_render::LightSelection::List {
            indices: [0; 8],
            count: 1,
        },
        ..InstanceBatch::new(model, instances)
    };
    let (img, _) = draw_overhead(&mut r, vec![listed], lighting, vec![reach])?;
    assert!(at(&img, left)[0] > 20, "listed left {:?}", at(&img, left));
    assert!(
        at(&img, right)[0] > 100,
        "listed right {:?}",
        at(&img, right)
    );
    Ok(())
}

/// lighting.md §5.2.4: the stored tangent is used as given (normalised, not made orthogonal
/// to the normal).
#[test]
fn instanced_tangents_are_used_as_given() -> Result<(), RenderError> {
    let Some(mut r) = gpu::headless_renderer(CLEAR) else {
        return Ok(());
    };
    // Tangent-space normal tilted toward +T.
    let texel = [204u8, 128, 230];
    let tilt = upload(
        &mut r,
        1,
        1,
        &[texel[0], texel[1], texel[2], 255],
        Wrap::Repeat,
    )?;
    let mut mesh = tile();
    let t = Vec3::new(1.0, 0.0, 1.0).normalize();
    mesh.tangents = Some(vec![[t.x, t.y, t.z, 1.0]; 4]);
    let model = r.upload_world_model(
        &mesh,
        &[WorldSurface {
            normal_texture: Some(tilt),
            ..WorldSurface::default()
        }],
    )?;
    let lighting = FrameLighting {
        sun: Some(SunLight {
            direction: Vec3::X,
            color: [0.5; 3],
            specular: [0.0; 3],
        }),
        ..dark()
    };
    let (img, _) = draw_overhead(
        &mut r,
        vec![InstanceBatch::new(
            model,
            vec![Instance::new(Mat4::IDENTITY)],
        )],
        lighting,
        vec![],
    )?;
    let n = Vec3::from_array(texel.map(|b| f32::from(b) / 255.0 * 2.0 - 1.0)).normalize();
    // N = n.x T − n.y B + n.z N0 with B = cross(N0, T) normalised; colour = 2 × 0.5 × N · L.
    let b = Vec3::Z.cross(t).normalize();
    let shading = (n.x * t - n.y * b + n.z * Vec3::Z).normalize();
    let want = (shading.x.max(0.0) * 255.0).round();
    let px = at(&img, Vec3::ZERO);
    assert!(
        (f32::from(px[0]) - want).abs() <= 2.0,
        "{px:?} vs {want} (made orthogonal it would be {})",
        (n.x * 255.0).round()
    );
    Ok(())
}

/// render.md §5.5 "as the lit mesh pipeline", lighting.md §7 code 0x00: an unlit world surface
/// is fogged only when its fog switch is on; a lit one always is (§2.1).
#[test]
fn unlit_world_surfaces_follow_their_fog_switch() -> Result<(), RenderError> {
    let Some(mut r) = gpu::headless_renderer(CLEAR) else {
        return Ok(());
    };
    // Fog complete at 20 from the eye, starting 10,000 before it: the tile, 10 below the
    // eye, is fogged by 0.999.
    let fogged = FrameLighting {
        ambient: [0.5; 3],
        fog: Some(Fog {
            near: -10_000.0,
            far: 20.0,
            color: [1.0, 0.0, 0.0],
        }),
        ..dark()
    };
    let mut draw = |unlit: bool, fog: bool| {
        let surface = WorldSurface {
            unlit,
            base_color: [0.0, 1.0, 0.0, 1.0],
            ..WorldSurface::default()
        };
        let model =
            r.upload_world_model_with(&tile(), &[surface], &[WorldSurfaceOptions { fog }])?;
        let batch = InstanceBatch::new(model, vec![Instance::new(Mat4::IDENTITY)]);
        let (img, _) = draw_overhead(&mut r, vec![batch], fogged.clone(), vec![])?;
        Ok::<_, RenderError>(at(&img, Vec3::ZERO))
    };
    let red = [255, 0, 0, 255];
    assert_eq!(draw(true, true)?, red, "unlit, fog on");
    assert_eq!(draw(true, false)?, [0, 255, 0, 255], "unlit, fog off");
    assert_eq!(draw(false, true)?, red, "lit, fog on");
    assert_eq!(draw(false, false)?, red, "lit surfaces are always fogged");
    // upload_world_model uses the default options: fog on.
    let model = r.upload_world_model(
        &tile(),
        &[WorldSurface {
            unlit: true,
            ..WorldSurface::default()
        }],
    )?;
    let batch = InstanceBatch::new(model, vec![Instance::new(Mat4::IDENTITY)]);
    let (img, _) = draw_overhead(&mut r, vec![batch], fogged, vec![])?;
    assert_eq!(at(&img, Vec3::ZERO), red, "default options");
    Ok(())
}

/// material.md §11.3, rule for unlit records: a fixed blend's factors read the opacity source
/// (here the base alpha, or 1 when that alpha is the gloss); M.alpha and the instance tint's
/// alpha do not enter. `Additive` keeps opacity × M.alpha.
#[test]
fn fixed_blends_of_world_surfaces_read_the_opacity_source() -> Result<(), RenderError> {
    let Some(mut r) = gpu::headless_renderer(CLEAR) else {
        return Ok(());
    };
    let half = upload(&mut r, 1, 1, &[255, 255, 255, 128], Wrap::Repeat)?;
    let mut draw = |alpha, gloss: bool, m_alpha: f32, tint_alpha: f32| {
        let surface = WorldSurface {
            unlit: true,
            alpha,
            base_color: [1.0, 1.0, 1.0, m_alpha],
            gloss_from_base_alpha: gloss,
            ..WorldSurface::textured(half)
        };
        let model = r.upload_world_model(&tile(), &[surface])?;
        let instance = Instance {
            tint: [1.0, 1.0, 1.0, tint_alpha],
            ..Instance::new(Mat4::IDENTITY)
        };
        let (img, _) = draw_overhead(
            &mut r,
            vec![InstanceBatch::new(model, vec![instance])],
            dark(),
            vec![],
        )?;
        Ok::<_, RenderError>(at(&img, Vec3::ZERO)[0])
    };
    let fixed = AlphaMode::Fixed(tessera_render::FixedBlend::ADDITIVE);
    let near = |got: u8, want: u8, what: &str| {
        assert!(got.abs_diff(want) <= 2, "{what}: {got} vs {want}");
    };
    near(draw(fixed, false, 0.5, 0.5)?, 128, "Fixed: base alpha only");
    near(
        draw(fixed, true, 0.5, 0.5)?,
        255,
        "Fixed: no opacity source, 1",
    );
    near(
        draw(AlphaMode::Additive, false, 0.5, 1.0)?,
        64,
        "Additive: × M.alpha",
    );
    near(
        draw(AlphaMode::Additive, true, 0.5, 0.5)?,
        64,
        "Additive: M.alpha × tint alpha",
    );
    Ok(())
}

/// material.md §11.2–§11.3: a base alpha that is the gloss is not an opacity: no alpha test,
/// output alpha M.alpha.
#[test]
fn gloss_in_the_base_alpha_is_not_opacity() -> Result<(), RenderError> {
    let Some(mut r) = gpu::headless_renderer(CLEAR) else {
        return Ok(());
    };
    // White texels with alpha 0.3.
    let glossy = upload(&mut r, 1, 1, &[255, 255, 255, 77], Wrap::Repeat)?;
    let draw = |r: &mut Renderer<'static>, surface: WorldSurface, frill: bool| {
        let model = r.upload_world_model(&tile(), &[surface])?;
        let instances = vec![Instance::new(Mat4::IDENTITY)];
        let batch = if frill {
            InstanceBatch::frills(
                model,
                instances,
                FrillParams {
                    opaque: false,
                    terrain_color_intensity: 0.0,
                    ..FrillParams::default()
                },
            )
        } else {
            InstanceBatch::new(model, instances)
        };
        let (img, _) = draw_overhead(r, vec![batch], dark(), vec![])?;
        Ok::<_, RenderError>(at(&img, Vec3::ZERO))
    };
    let surface = |alpha, gloss| WorldSurface {
        unlit: true,
        alpha,
        gloss_from_base_alpha: gloss,
        ..WorldSurface::textured(glossy)
    };
    let white = [255, 255, 255, 255];
    let black = [0, 0, 0, 255];
    let test = AlphaMode::TestBlend { cutoff: 0.5 };
    // Opacity from the base alpha: 0.3 ≤ 0.5 is discarded; blended by 0.3.
    assert_eq!(draw(&mut r, surface(test, false), false)?, black);
    let px = draw(&mut r, surface(AlphaMode::Blend, false), false)?;
    assert!(px[0].abs_diff(77) <= 2, "{px:?}");
    // Gloss in the base alpha: no opacity source, kept and opaque.
    assert_eq!(draw(&mut r, surface(test, true), false)?, white);
    assert_eq!(draw(&mut r, surface(AlphaMode::Blend, true), false)?, white);
    assert_eq!(
        draw(
            &mut r,
            surface(AlphaMode::Mask { cutoff: 0.5 }, true),
            false
        )?,
        white
    );
    // A non-opaque frill group draws a Mask surface blended (render.md §5.5): its opacity
    // is still the base alpha unless that alpha is the gloss.
    let mask = AlphaMode::Mask { cutoff: 0.1 };
    let px = draw(&mut r, surface(mask, false), true)?;
    assert!(px[0].abs_diff(77) <= 2, "{px:?}");
    assert_eq!(draw(&mut r, surface(mask, true), true)?, white);
    Ok(())
}

/// render.md §5.5 (`upload_world_model`): validated as for `upload_mesh`, under the
/// renderer's own mesh limits.
#[test]
fn world_models_use_the_renderers_mesh_limits() -> Result<(), RenderError> {
    let Some(mut r) = gpu::headless_renderer_with(|c| {
        c.clear_color = CLEAR;
        c.meshes.limits.max_vertices = 3;
    }) else {
        return Ok(());
    };
    // Four vertices: over the configured limit for both paths.
    assert!(matches!(
        r.upload_mesh(&tile()),
        Err(RenderError::LimitExceeded { .. })
    ));
    assert!(matches!(
        r.upload_world_model(&tile(), &[]),
        Err(RenderError::LimitExceeded { .. })
    ));
    // Three vertices: accepted by both.
    let tri = MeshData::new(
        vec![[0.0; 3], [1.0, 0.0, 0.0], [0.0, 1.0, 0.0]],
        vec![0, 1, 2],
    );
    assert!(r.upload_mesh(&tri).is_ok());
    assert!(r.upload_world_model(&tri, &[]).is_ok());
    Ok(())
}

/// render.md §5.5 Culling (implementer note): a swaying frill is culled by its sphere grown
/// by the largest sway, so its swaying tip does not pop at the screen edge.
#[test]
fn swaying_frills_are_culled_with_their_sway() -> Result<(), RenderError> {
    let Some(mut r) = gpu::headless_renderer(CLEAR) else {
        return Ok(());
    };
    // A thin vertical card 1 high (sway height h = 1).
    let mut card = MeshData::new(
        vec![
            [-0.05, 0.0, 0.0],
            [0.05, 0.0, 0.0],
            [0.05, 0.0, 1.0],
            [-0.05, 0.0, 1.0],
        ],
        vec![0, 1, 2, 0, 2, 3],
    );
    card.normals = Some(vec![[0.0, -1.0, 0.0]; 4]);
    let model = r.upload_world_model(
        &card,
        &[WorldSurface {
            two_sided: true,
            ..WorldSurface::default()
        }],
    )?;
    let sphere = model.info().sphere;
    let half_fov = 30f32.to_radians();
    let cam = Camera {
        eye: Vec3::new(0.0, 0.0, 0.5),
        target: Vec3::new(0.0, 10.0, 0.5),
        up: Vec3::Z,
        projection: Perspective {
            fov_y: 2.0 * half_fov,
            near: 0.1,
            far: 100.0,
        },
    };
    // The static sphere 0.3 outside the right frustum plane (aspect 1) at depth 10.
    let depth = 10.0;
    let x = depth * half_fov.tan() + (sphere.radius + 0.3) / half_fov.cos() - sphere.center.x;
    let at = Instance::new(Mat4::from_translation(Vec3::new(x, depth, 0.0)));
    let frill = |intensity: f32| {
        InstanceBatch::frills(
            model.clone(),
            vec![at],
            FrillParams {
                animation_intensity: intensity,
                ..FrillParams::default()
            },
        )
    };
    let count = |r: &mut Renderer<'static>, b: InstanceBatch| {
        let target = r.create_offscreen_target(32, 32, OffscreenFormat::Rgba8Unorm)?;
        let mut frame = r.begin_offscreen_frame(&target)?;
        frame
            .world_pass("sway", &cam)
            .frame_lighting(lighting())
            .instanced(b);
        let s = r.end_frame(frame)?;
        Ok::<_, RenderError>((s.instances, s.instances_culled))
    };
    // Still: culled. Swaying with intensity 1 (reach 1.118 × h): kept.
    assert_eq!(count(&mut r, frill(0.0))?, (0, 1));
    assert_eq!(count(&mut r, frill(1.0))?, (1, 0));
    // Scenery does not sway: culled as before.
    assert_eq!(
        count(&mut r, InstanceBatch::new(model.clone(), vec![at]))?,
        (0, 1)
    );
    Ok(())
}

/// render.md §2.3, §4.1 through §5.5 "as the lit mesh pipeline": a mirrored frill instance
/// flips the bitangent sign as scenery does, so a normal map tilted along **B** shades the
/// same upright and mirrored in x (the texture is mirrored with the geometry).
#[test]
fn mirrored_frills_flip_the_bitangent() -> Result<(), RenderError> {
    let Some(mut r) = gpu::headless_renderer(CLEAR) else {
        return Ok(());
    };
    // Tangent-space normal tilted along +B (green above 0.5): N = −n.y B + n.z N0 tilts
    // toward −B = −y for the upright tile (lighting.md §5.2.4).
    let tilt = upload(&mut r, 1, 1, &[128, 204, 230, 255], Wrap::Repeat)?;
    let model = r.upload_world_model(
        &tile(),
        &[WorldSurface {
            normal_texture: Some(tilt),
            two_sided: true,
            ..WorldSurface::default()
        }],
    )?;
    let lighting = FrameLighting {
        sun: Some(SunLight {
            direction: Vec3::new(0.0, -1.0, 1.0).normalize(),
            color: [0.5; 3],
            specular: [0.0; 3],
        }),
        ..dark()
    };
    let mirror_x = Mat4::from_scale(Vec3::new(-1.0, 1.0, 1.0));
    let (a, b) = (Vec3::new(-1.5, 0.0, 0.0), Vec3::new(1.5, 0.0, 0.0));
    let instances = vec![small_tile(a, Mat4::IDENTITY), small_tile(b, mirror_x)];
    let params = FrillParams {
        animation_intensity: 0.0,
        terrain_color_intensity: 0.0,
        ..FrillParams::default()
    };
    let frills = InstanceBatch::frills(model.clone(), instances.clone(), params);
    let (img, stats) = draw_overhead(&mut r, vec![frills], lighting.clone(), vec![])?;
    assert_eq!(stats.instances, 2, "{stats:?}");
    let (up, mirrored) = (at(&img, a), at(&img, b));
    // Flat it would be 2 × 0.5 × cos 45° = 0.707 (180); tilted toward the light it is
    // brighter, tilted away darker.
    assert!(up[0] > 200, "upright {up:?}");
    assert!(
        up[0].abs_diff(mirrored[0]) <= 2,
        "upright {up:?}, mirrored {mirrored:?}"
    );
    // Scenery shades the same.
    let (img, _) = draw_overhead(
        &mut r,
        vec![InstanceBatch::new(model, instances)],
        lighting,
        vec![],
    )?;
    assert!(
        at(&img, a)[0].abs_diff(up[0]) <= 2,
        "scenery {:?}",
        at(&img, a)
    );
    assert!(
        at(&img, b)[0].abs_diff(up[0]) <= 2,
        "scenery {:?}",
        at(&img, b)
    );
    Ok(())
}

/// render.md §4.8 (departure R-D10) through §5.5: a world model with a stored binormal
/// stream uses the stored binormal as **B** (here −y, the opposite of w × cross(N₀, T) =
/// +y), for scenery and frills alike, upright and mirrored.
#[test]
fn world_models_use_the_stored_binormal() -> Result<(), RenderError> {
    let Some(mut r) = gpu::headless_renderer(CLEAR) else {
        return Ok(());
    };
    let tilt = upload(&mut r, 1, 1, &[128, 204, 230, 255], Wrap::Repeat)?;
    let surfaces = [WorldSurface {
        normal_texture: Some(tilt),
        two_sided: true,
        ..WorldSurface::default()
    }];
    let mut stored = tile();
    stored.binormals = Some(vec![[0.0, -1.0, 0.0]; 4]);
    let derived = r.upload_world_model(&tile(), &surfaces)?;
    let stored = r.upload_world_model(&stored, &surfaces)?;
    let lighting = FrameLighting {
        sun: Some(SunLight {
            direction: Vec3::new(0.0, -1.0, 1.0).normalize(),
            color: [0.5; 3],
            specular: [0.0; 3],
        }),
        ..dark()
    };
    let mirror_x = Mat4::from_scale(Vec3::new(-1.0, 1.0, 1.0));
    let (a, b) = (Vec3::new(-1.5, 0.0, 0.0), Vec3::new(1.5, 0.0, 0.0));
    let instances = vec![small_tile(a, Mat4::IDENTITY), small_tile(b, mirror_x)];
    let params = FrillParams {
        animation_intensity: 0.0,
        terrain_color_intensity: 0.0,
        ..FrillParams::default()
    };
    // N = n.x T − n.y B + n.z N₀ with n ≈ (0, 0.6, 0.8): toward −y with the derived B
    // (2 × 0.5 × N · L ≈ 0.99), toward +y with the stored one (≈ 0.14).
    let (img, _) = draw_overhead(
        &mut r,
        vec![InstanceBatch::new(derived, instances.clone())],
        lighting.clone(),
        vec![],
    )?;
    assert!(at(&img, a)[0] > 230, "derived {:?}", at(&img, a));
    for batch in [
        InstanceBatch::new(stored.clone(), instances.clone()),
        InstanceBatch::frills(stored.clone(), instances.clone(), params),
    ] {
        let (img, _) = draw_overhead(&mut r, vec![batch], lighting.clone(), vec![])?;
        let (up, mirrored) = (at(&img, a), at(&img, b));
        assert!((20..60).contains(&up[0]), "stored {up:?}");
        assert!(
            up[0].abs_diff(mirrored[0]) <= 2,
            "upright {up:?}, mirrored {mirrored:?}"
        );
    }
    Ok(())
}

/// material.md §8.3: the specular colour is multiplied by the same per-object tint as *M*.
#[test]
fn instance_tint_tints_the_highlight() -> Result<(), RenderError> {
    let Some(mut r) = gpu::headless_renderer(CLEAR) else {
        return Ok(());
    };
    let model = r.upload_world_model(
        &tile(),
        &[WorldSurface {
            specular: Some(tessera_render::Specular {
                color: [1.0; 3],
                shininess: 1.0,
                texture: None,
            }),
            ..WorldSurface::default()
        }],
    )?;
    // A sun straight above with a white highlight and no diffuse colour: the overhead
    // camera sees sat(N · H)^1 = 1 at the centre.
    let lighting = FrameLighting {
        sun: Some(SunLight {
            direction: Vec3::Z,
            color: [0.0; 3],
            specular: [1.0; 3],
        }),
        ..dark()
    };
    let tinted = Instance {
        tint: [0.5, 0.25, 1.0, 1.0],
        ..Instance::new(Mat4::IDENTITY)
    };
    let (img, _) = draw_overhead(
        &mut r,
        vec![InstanceBatch::new(model, vec![tinted])],
        lighting,
        vec![],
    )?;
    let px = at(&img, Vec3::ZERO);
    for (c, want) in px[..3].iter().zip([128u8, 64, 255]) {
        assert!(c.abs_diff(want) <= 3, "{px:?}");
    }
    Ok(())
}
