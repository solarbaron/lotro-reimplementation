// SPDX-License-Identifier: GPL-3.0-or-later
//! Skinned meshes on a real (or software) adapter: skin uploads in the shared arena, the
//! bind pose equal to the static path for every material mode and debug view, posed
//! palettes, palette fallbacks and per-frame palette counts. Every test skips (with a
//! printed reason) when no GPU adapter is available.

#[path = "support/gpu.rs"]
mod gpu;
#[path = "support/skinning.rs"]
mod skinning;

use tessera_render::glam::{Mat4, Vec3};
use tessera_render::{
    AlphaMode, Camera, ColorSpace, Dye, DyeMask, Filter, Frame, Lighting, MAX_PALETTE_BONES,
    Material, MeshConfig, MeshData, MeshDebug, MeshDraw, MeshId, MeshShading, MeshSkin,
    NormalLines, OffscreenFormat, RenderError, Renderer, RgbaImage, SkinPalette, Specular,
    TextureId, TextureUpload, Wrap,
};

const BG: [f32; 4] = [0.0, 0.0, 0.0, 1.0];
const SIZE: u32 = 96;

fn renderer() -> Option<gpu::Headless> {
    gpu::headless_renderer(BG)
}

fn camera() -> Camera {
    Camera {
        eye: Vec3::new(0.4, 1.2, 3.2),
        target: Vec3::new(0.0, 0.6, 0.0),
        up: Vec3::Y,
        ..Camera::default()
    }
}

fn render(
    r: &mut Renderer<'static>,
    build: impl FnOnce(&mut Frame),
) -> Result<RgbaImage, RenderError> {
    let target = r.create_offscreen_target(SIZE, SIZE, OffscreenFormat::Rgba8Unorm)?;
    let mut frame = r.begin_offscreen_frame(&target)?;
    build(&mut frame);
    r.end_frame(frame)?;
    r.read_pixels(&target)
}

fn coverage(img: &RgbaImage) -> usize {
    img.data
        .as_chunks::<4>()
        .0
        .iter()
        .filter(|p| p[0] > 0 || p[1] > 0 || p[2] > 0)
        .count()
}

/// Upload a stripe texture usable as base colour, dye mask, normal map and specular mask.
fn stripes(r: &mut Renderer<'static>, space: ColorSpace) -> Result<TextureId, RenderError> {
    let size = 32u32;
    let data: Vec<u8> = (0..size * size)
        .flat_map(|i| {
            let (x, y) = (i % size, i / size);
            let on = (x / 4 + y / 8) % 2 == 0;
            if on {
                [200, 160, 255, 255]
            } else {
                [60, 90, 128, 120]
            }
        })
        .collect();
    r.upload_texture(&TextureUpload {
        color_space: space,
        filter: Filter::Nearest,
        wrap: Wrap::Repeat,
        ..TextureUpload::rgba8(size, size, &data)
    })
}

/// Every material mode and feature the static path has.
fn materials(base: TextureId, mask: TextureId) -> Vec<Material> {
    let lit = Material::textured(base);
    vec![
        lit,
        Material {
            alpha: AlphaMode::Mask { cutoff: 0.6 },
            ..lit
        },
        Material {
            alpha: AlphaMode::Blend,
            ..lit
        },
        Material {
            two_sided: true,
            ..lit
        },
        Material {
            alpha: AlphaMode::Blend,
            two_sided: true,
            ..lit
        },
        Material {
            dye: Some(Dye {
                color: [0.9, 0.2, 0.1],
                mask: DyeMask::Texture(mask),
            }),
            ..lit
        },
        Material {
            dye: Some(Dye {
                color: [0.1, 0.3, 0.9],
                mask: DyeMask::BaseAlpha,
            }),
            ..lit
        },
        Material {
            normal_texture: Some(mask),
            ..lit
        },
        Material {
            specular: Some(Specular {
                color: [1.0, 0.9, 0.8],
                shininess: 24.0,
                texture: Some(mask),
            }),
            ..lit
        },
        Material { unlit: true, ..lit },
    ]
}

fn debug_views() -> Vec<MeshDebug> {
    let mut v: Vec<MeshDebug> = [
        MeshShading::Lit,
        MeshShading::BaseColor,
        MeshShading::Normals,
        MeshShading::UvChecker,
    ]
    .into_iter()
    .map(|shading| MeshDebug {
        shading,
        ..MeshDebug::default()
    })
    .collect();
    v.push(MeshDebug {
        shading: MeshShading::Hidden,
        wireframe: Some([0.2, 1.0, 0.4, 1.0]),
        normals: None,
    });
    v.push(MeshDebug {
        shading: MeshShading::Lit,
        wireframe: Some([1.0, 1.0, 0.2, 1.0]),
        normals: Some(NormalLines {
            length: 0.15,
            color: [0.3, 0.6, 1.0, 1.0],
        }),
    });
    v
}

/// Draw `mesh` once per (material, debug view); `skin` decides whether it gets a palette.
fn draw_all(
    r: &mut Renderer<'static>,
    mesh: MeshId,
    material: Material,
    debug: MeshDebug,
    palette: Option<&[Mat4]>,
) -> Result<RgbaImage, RenderError> {
    let cam = camera();
    let transform = Mat4::from_rotation_y(0.4) * Mat4::from_scale(Vec3::new(1.0, 1.0, 1.2));
    let mut failure = None;
    let img = render(r, |f| {
        let draw = MeshDraw::new(mesh, transform, material);
        let draw = match palette.map(|p| f.skin_palette(p)) {
            Some(Ok(p)) => draw.with_skin(p),
            Some(Err(e)) => {
                failure = Some(e);
                draw
            }
            None => draw,
        };
        f.world_pass("skin", &cam)
            .clear(BG)
            .lighting(Lighting::default())
            .mesh_debug(debug)
            .mesh(draw);
    })?;
    match failure {
        Some(e) => Err(e),
        None => Ok(img),
    }
}

#[test]
fn bind_pose_equals_static_path_for_every_mode() -> Result<(), RenderError> {
    let Some(mut r) = renderer() else {
        return Ok(());
    };
    let base = stripes(&mut r, ColorSpace::Srgb)?;
    let mask = stripes(&mut r, ColorSpace::Linear)?;
    let mesh = skinning::cylinder(0.35, 1.2, 16, 8);
    let skin = skinning::two_bone_skin(&mesh, 0.6, 0.2, 2);
    let static_mesh = r.upload_mesh(&mesh)?;
    let skinned = r.upload_skinned_mesh(&mesh, &skin)?;
    let identity = vec![Mat4::IDENTITY; 2];
    let mut checked = 0;
    for material in materials(base, mask) {
        for debug in debug_views() {
            let reference = draw_all(&mut r, static_mesh, material, debug, None)?;
            assert!(coverage(&reference) > 200, "{material:?} {debug:?} draws");
            let bind = draw_all(&mut r, skinned, material, debug, Some(&identity))?;
            assert!(
                reference.data == bind.data,
                "identity palette differs from the static path: {material:?} {debug:?}"
            );
            let no_palette = draw_all(&mut r, skinned, material, debug, None)?;
            assert!(
                reference.data == no_palette.data,
                "skinned mesh without a palette differs from the static path: {material:?}"
            );
            checked += 1;
        }
    }
    assert_eq!(checked, 10 * 6);
    Ok(())
}

#[test]
fn rigid_palette_matches_moved_static_mesh() -> Result<(), RenderError> {
    let Some(mut r) = renderer() else {
        return Ok(());
    };
    let mesh = skinning::cylinder(0.3, 1.0, 12, 2);
    let static_mesh = r.upload_mesh(&mesh)?;
    // Every vertex on bone 3 of a 4-bone palette.
    let skinned = r.upload_skinned_mesh(&mesh, &MeshSkin::rigid(mesh.positions.len(), 3, 4))?;
    let moved = Mat4::from_translation(Vec3::new(0.5, -0.2, 0.0)) * Mat4::from_rotation_z(0.3);
    let material = Material::color([0.8, 0.7, 0.3, 1.0]);
    let cam = camera();
    let reference = render(&mut r, |f| {
        f.world_pass("static", &cam)
            .clear(BG)
            .mesh(MeshDraw::new(static_mesh, moved, material));
    })?;
    let mut stats = None;
    let target = r.create_offscreen_target(SIZE, SIZE, OffscreenFormat::Rgba8Unorm)?;
    let mut frame = r.begin_offscreen_frame(&target)?;
    let palette = frame.skin_palette(&[Mat4::ZERO, Mat4::ZERO, Mat4::ZERO, moved])?;
    frame
        .world_pass("skinned", &cam)
        .clear(BG)
        .mesh(MeshDraw::new(skinned, Mat4::IDENTITY, material).with_skin(palette));
    stats.replace(r.end_frame(frame)?);
    let posed = r.read_pixels(&target)?;
    let stats = stats.expect("stats");
    assert_eq!((stats.skinned_draws, stats.palettes), (1, 1));
    // The same transform reached through the palette: equal up to rounding.
    let differing = reference
        .data
        .as_chunks::<4>()
        .0
        .iter()
        .zip(posed.data.as_chunks::<4>().0)
        .filter(|(a, b)| a.iter().zip(b.iter()).any(|(x, y)| x.abs_diff(*y) > 2))
        .count();
    assert!(
        differing <= 8,
        "{differing} pixels differ between palette and transform"
    );
    assert!(coverage(&posed) > 300);
    Ok(())
}

#[test]
fn bent_palette_moves_only_the_upper_bone() -> Result<(), RenderError> {
    let Some(mut r) = renderer() else {
        return Ok(());
    };
    let mesh = skinning::cylinder(0.25, 1.2, 12, 12);
    let skin = skinning::two_bone_skin(&mesh, 0.6, 0.15, 2);
    let id = r.upload_skinned_mesh(&mesh, &skin)?;
    let cam = Camera {
        eye: Vec3::new(0.0, 0.6, 4.0),
        target: Vec3::new(0.0, 0.6, 0.0),
        up: Vec3::Y,
        ..Camera::default()
    };
    let draw = |r: &mut Renderer<'static>, angle: f32| {
        render(r, |f| {
            let p = f
                .skin_palette(&skinning::two_bone_palette(0.6, angle))
                .expect("palette");
            f.world_pass("bend", &cam)
                .clear(BG)
                .mesh(MeshDraw::new(id, Mat4::IDENTITY, Material::color([1.0; 4])).with_skin(p));
        })
    };
    let straight = draw(&mut r, 0.0)?;
    let bent = draw(&mut r, 1.2)?;
    // Screen rows of the part below the blend zone (y = 0 .. 0.4, bone 0 only).
    let view_proj = cam.view_proj(1.0);
    let row = |y: f32| {
        let ndc = view_proj.project_point3(Vec3::new(0.0, y, 0.0));
        ((1.0 - ndc.y) * 0.5 * SIZE as f32) as u32
    };
    let (top, bottom) = (row(0.4) + 1, row(0.0).min(SIZE));
    assert!(bottom > top + 4, "rows {top}..{bottom}");
    let rows = |img: &RgbaImage, y0: u32, y1: u32| -> Vec<u8> {
        (y0..y1)
            .flat_map(|y| (0..SIZE).flat_map(move |x| img.pixel(x, y).unwrap_or([0; 4])))
            .collect()
    };
    assert_eq!(rows(&straight, top, bottom), rows(&bent, top, bottom));
    assert!(coverage(&straight) > 200);
    assert_ne!(straight.data, bent.data, "the upper bone moved");
    Ok(())
}

#[test]
fn foreign_or_short_palettes_fall_back_to_the_bind_pose() -> Result<(), RenderError> {
    let Some(mut r) = renderer() else {
        return Ok(());
    };
    let mesh = skinning::cylinder(0.3, 1.0, 12, 2);
    let static_mesh = r.upload_mesh(&mesh)?;
    let skinned = r.upload_skinned_mesh(&mesh, &MeshSkin::rigid(mesh.positions.len(), 3, 4))?;
    let material = Material::color([0.5, 0.8, 0.3, 1.0]);
    let cam = camera();
    let reference = render(&mut r, |f| {
        f.world_pass("static", &cam).clear(BG).mesh(MeshDraw::new(
            static_mesh,
            Mat4::IDENTITY,
            material,
        ));
    })?;
    let away = Mat4::from_translation(Vec3::new(5.0, 0.0, 0.0));
    // A palette issued by an earlier frame.
    let target = r.create_offscreen_target(SIZE, SIZE, OffscreenFormat::Rgba8Unorm)?;
    let mut old = r.begin_offscreen_frame(&target)?;
    let stale: SkinPalette = old.skin_palette(&[away; 4])?;
    r.end_frame(old)?;
    let mut frame = r.begin_offscreen_frame(&target)?;
    // Too short: the mesh uses bone 3.
    let short = frame.skin_palette(&[away; 3])?;
    assert_eq!(short.len(), 3);
    frame
        .world_pass("fallback", &cam)
        .clear(BG)
        .mesh(MeshDraw::new(skinned, Mat4::IDENTITY, material).with_skin(stale))
        .mesh(MeshDraw::new(skinned, Mat4::IDENTITY, material).with_skin(short));
    let stats = r.end_frame(frame)?;
    assert_eq!(stats.skinned_draws, 0);
    assert_eq!(stats.mesh_draws, 2);
    assert_eq!(r.read_pixels(&target)?.data, reference.data);
    // A palette on a static mesh is ignored.
    let img = render(&mut r, |f| {
        let p = f.skin_palette(&[away; 4]).expect("palette");
        f.world_pass("static+palette", &cam)
            .clear(BG)
            .mesh(MeshDraw::new(static_mesh, Mat4::IDENTITY, material).with_skin(p));
    })?;
    assert_eq!(img.data, reference.data);
    Ok(())
}

#[test]
fn palette_limits() -> Result<(), RenderError> {
    let Some(mut r) = renderer() else {
        return Ok(());
    };
    let target = r.create_offscreen_target(8, 8, OffscreenFormat::Rgba8Unorm)?;
    let mut frame = r.begin_offscreen_frame(&target)?;
    assert!(matches!(
        frame.skin_palette(&[]),
        Err(RenderError::InvalidSkin(_))
    ));
    assert!(matches!(
        frame.skin_palette(&vec![Mat4::IDENTITY; MAX_PALETTE_BONES + 1]),
        Err(RenderError::InvalidSkin(_))
    ));
    let full = frame.skin_palette(&vec![Mat4::IDENTITY; MAX_PALETTE_BONES])?;
    assert_eq!(full.len(), MAX_PALETTE_BONES);
    let rows =
        frame.skin_palette_rows(&[[1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0]])?;
    assert_eq!(rows.len(), 1);
    assert_ne!(full, rows);
    let stats = r.end_frame(frame)?;
    assert_eq!(stats.palettes, 2);
    assert!(stats.palette_bytes >= (MAX_PALETTE_BONES as u64 + 1) * 48);
    Ok(())
}

#[test]
fn many_instances_share_one_palette_buffer() -> Result<(), RenderError> {
    let Some(mut r) = renderer() else {
        return Ok(());
    };
    let mesh = skinning::cylinder(0.12, 0.3, 6, 4);
    let skin = skinning::two_bone_skin(&mesh, 0.15, 0.05, 89);
    let id = r.upload_skinned_mesh(&mesh, &skin)?;
    let cam = Camera {
        eye: Vec3::new(0.0, 3.0, 9.0),
        target: Vec3::ZERO,
        up: Vec3::Y,
        ..Camera::default()
    };
    for frame_no in 0..3 {
        let target = r.create_offscreen_target(SIZE, SIZE, OffscreenFormat::Rgba8Unorm)?;
        let mut frame = r.begin_offscreen_frame(&target)?;
        let mut draws = Vec::new();
        for i in 0..200u32 {
            let mut palette = vec![Mat4::IDENTITY; 89];
            palette[1] = skinning::bend(Vec3::new(0.0, 0.15, 0.0), (i + frame_no) as f32 * 0.01);
            let p = frame.skin_palette(&palette)?;
            let at = Mat4::from_translation(Vec3::new(
                (i % 20) as f32 * 0.4 - 4.0,
                (i / 20) as f32 * 0.4 - 2.0,
                0.0,
            ));
            draws.push(MeshDraw::new(id, at, Material::color([0.7, 0.6, 0.5, 1.0])).with_skin(p));
        }
        let pass = frame.world_pass("crowd", &cam);
        pass.clear(BG);
        for d in draws {
            pass.mesh(d);
        }
        let stats = r.end_frame(frame)?;
        assert_eq!((stats.palettes, stats.skinned_draws), (200, 200));
        let covered = coverage(&r.read_pixels(&target)?);
        assert!(covered > 500, "{covered} pixels covered");
    }
    Ok(())
}

#[test]
fn skin_uploads_use_the_arena() -> Result<(), RenderError> {
    let Some(mut r) = renderer() else {
        return Ok(());
    };
    let mesh = skinning::cylinder(0.3, 1.0, 8, 2);
    let n = mesh.positions.len();
    let a = r.upload_mesh(&MeshData::cube(0.5))?;
    let before = r.mesh_stats();
    assert_eq!((before.skinned_meshes, before.skin_bytes_used), (0, 0));
    let s = r.upload_skinned_mesh(&mesh, &MeshSkin::rigid(n, 0, 1))?;
    let stats = r.mesh_stats();
    assert_eq!(stats.pages, 1, "skinned mesh shares the static mesh's page");
    assert_eq!(stats.skinned_meshes, 1);
    assert_eq!(stats.skin_bytes_used, n as u64 * 12);
    assert!(
        stats.capacity_bytes > before.capacity_bytes,
        "skin buffer counted"
    );
    let info = r.mesh_info(s).and_then(|i| i.skin).expect("skin info");
    assert_eq!(
        (info.palette_size, info.max_joint, info.max_influences),
        (1, 0, 1)
    );
    assert!(r.mesh_info(a).is_some_and(|i| i.skin.is_none()));
    // Bad skins allocate nothing and name the problem.
    let mut bad = MeshSkin::rigid(n, 0, 1);
    bad.weights[3] = [0.6, 0.0, 0.0, 0.0];
    match r.upload_skinned_mesh(&mesh, &bad) {
        Err(RenderError::InvalidSkin(m)) => assert!(m.contains("vertex 3"), "{m}"),
        other => panic!("expected InvalidSkin, got {other:?}"),
    }
    let mut out_of_range = MeshSkin::rigid(n, 0, 2);
    out_of_range.joints[5] = [2, 0, 0, 0];
    assert!(matches!(
        r.upload_skinned_mesh(&mesh, &out_of_range),
        Err(RenderError::InvalidSkin(_))
    ));
    assert!(matches!(
        r.upload_skinned_mesh(&mesh, &MeshSkin::rigid(n - 1, 0, 1)),
        Err(RenderError::InvalidSkin(_))
    ));
    assert_eq!(r.mesh_stats().meshes, 2);
    // The warm page 0 keeps its skin buffer after its last skinned mesh goes.
    assert!(r.destroy_mesh(s));
    let after = r.mesh_stats();
    assert_eq!((after.skinned_meshes, after.skin_bytes_used), (0, 0));
    assert_eq!(after.capacity_bytes, stats.capacity_bytes);
    // Prepared elsewhere, uploaded later.
    let prepared = mesh.prepare_skinned(&MeshSkin::rigid(n, 0, 1), &MeshConfig::default())?;
    assert_eq!(prepared.gpu_bytes() % 4, 0);
    let p = r.upload_prepared(prepared)?;
    assert!(r.mesh_info(p).is_some_and(|i| i.skin.is_some()));
    Ok(())
}

#[test]
fn skinned_meshes_respect_the_byte_cap() -> Result<(), RenderError> {
    // Pages of 256 vertices; the cap leaves room for one page but not its skin buffer.
    let config = MeshConfig {
        page_vertex_bytes: 256 * 40,
        page_index_bytes: 1024 * 4,
        max_total_bytes: 256 * 40 + 1024 * 4 + 100,
        wireframe_edges: false,
        ..MeshConfig::default()
    };
    let Some(mut r) = gpu::headless_renderer_with(|c| {
        c.clear_color = BG;
        c.meshes = config;
    }) else {
        return Ok(());
    };
    let mesh = skinning::cylinder(0.3, 1.0, 8, 2);
    r.upload_mesh(&MeshData::cube(0.5))?;
    let n = mesh.positions.len();
    assert!(matches!(
        r.upload_skinned_mesh(&mesh, &MeshSkin::rigid(n, 0, 1)),
        Err(RenderError::LimitExceeded { .. })
    ));
    assert_eq!(r.mesh_stats().skinned_meshes, 0);
    assert_eq!(r.mesh_stats().pages, 1);
    Ok(())
}

#[test]
fn palettes_of_another_renderer_fall_back_to_the_bind_pose() -> Result<(), RenderError> {
    // Regression: palettes carried only a per-renderer frame serial, so a palette of
    // renderer A's frame N passed renderer B's check in B's frame N and B bound A's byte
    // offset into its own palette buffer (a GPU validation error that lost the frame when
    // the offset was past B's buffer, or B's unrelated palette data when it was not).
    let Some(mut a) = renderer() else {
        return Ok(());
    };
    let Some(mut b) = renderer() else {
        return Ok(());
    };
    let mesh = skinning::cylinder(0.3, 1.0, 12, 2);
    let static_mesh = b.upload_mesh(&mesh)?;
    let skinned = b.upload_skinned_mesh(&mesh, &MeshSkin::rigid(mesh.positions.len(), 0, 1))?;
    let material = Material::color([0.5, 0.8, 0.3, 1.0]);
    let cam = camera();
    let away = Mat4::from_translation(Vec3::new(5.0, 0.0, 0.0));
    let target_a = a.create_offscreen_target(8, 8, OffscreenFormat::Rgba8Unorm)?;
    let target_b = b.create_offscreen_target(SIZE, SIZE, OffscreenFormat::Rgba8Unorm)?;
    // Both renderers are at frame 0: A's first and last palettes (offset 0, and far
    // beyond B's 12 KiB palette buffer).
    let mut fa = a.begin_offscreen_frame(&target_a)?;
    let first = fa.skin_palette(&[away])?;
    let mut last = first;
    for _ in 0..400 {
        last = fa.skin_palette(&[away])?;
    }
    let mut fb = b.begin_offscreen_frame(&target_b)?;
    // B's own palette at offset 0 also moves the mesh away, so drawing A's offset-0
    // palette "successfully" would show up as missing coverage.
    let _own = fb.skin_palette(&[away])?;
    fb.world_pass("foreign", &cam)
        .clear(BG)
        .mesh(MeshDraw::new(skinned, Mat4::IDENTITY, material).with_skin(first))
        .mesh(MeshDraw::new(skinned, Mat4::IDENTITY, material).with_skin(last));
    let stats = b.end_frame(fb)?;
    assert_eq!(stats.skinned_draws, 0, "foreign palettes are not used");
    assert_eq!(stats.mesh_draws, 2);
    let foreign = b.read_pixels(&target_b)?;
    a.end_frame(fa)?;
    let reference = render(&mut b, |f| {
        f.world_pass("static", &cam).clear(BG).mesh(MeshDraw::new(
            static_mesh,
            Mat4::IDENTITY,
            material,
        ));
    })?;
    assert_eq!(foreign.data, reference.data, "bind pose drawn");
    Ok(())
}

#[test]
fn full_palette_budget_on_a_small_device_keeps_rendering() -> Result<(), RenderError> {
    // Regression for two end_frame bugs on a device whose buffers are capped at 1 MiB:
    // `skin_palette` accepted palettes whose upload buffer exceeded the budget (by up to
    // one 12 KiB binding), and that palette-buffer failure came after the draw-uniform
    // buffer had been replaced, leaving the draw bind group on the old, smaller buffer so
    // every later frame with many draws failed GPU validation.
    let Some(mut r) = gpu::headless_renderer_with(|c| {
        c.clear_color = BG;
        c.context.required_limits = Some(tessera_render::wgpu::Limits {
            max_buffer_size: 1 << 20,
            ..tessera_render::wgpu::Limits::downlevel_webgl2_defaults()
        });
    }) else {
        return Ok(());
    };
    let cube = r.upload_mesh(&MeshData::cube(0.05))?;
    let cam = camera();
    let material = Material::color([0.9, 0.4, 0.2, 1.0]);
    let draws = |pass: &mut tessera_render::Pass| {
        for i in 0..200u32 {
            let at = Mat4::from_translation(Vec3::new(
                (i % 20) as f32 * 0.1 - 1.0,
                (i / 20) as f32 * 0.1,
                0.0,
            ));
            pass.mesh(MeshDraw::new(cube, at, material));
        }
    };
    let target = r.create_offscreen_target(SIZE, SIZE, OffscreenFormat::Rgba8Unorm)?;
    let mut frame = r.begin_offscreen_frame(&target)?;
    let full = vec![Mat4::IDENTITY; MAX_PALETTE_BONES];
    let mut accepted = 0;
    loop {
        match frame.skin_palette(&full) {
            Ok(_) => accepted += 1,
            Err(RenderError::LimitExceeded { .. }) => break,
            Err(e) => return Err(e),
        }
    }
    assert!(accepted > 10, "{accepted} palettes");
    assert!(frame.palette_bytes() <= 1 << 20);
    draws(frame.world_pass("crowd", &cam).clear(BG));
    let stats = r.end_frame(frame)?;
    assert_eq!((stats.palettes, stats.mesh_draws), (accepted, 200));
    for _ in 0..2 {
        let img = render(&mut r, |f| draws(f.world_pass("crowd", &cam).clear(BG)))?;
        assert!(coverage(&img) > 100);
    }
    Ok(())
}

#[test]
fn the_warm_page_keeps_its_skin_buffer() -> Result<(), RenderError> {
    // Regression: the skin buffer (sized for the whole page) was dropped with the page's
    // last skinned mesh and re-created on the next skinned upload, so a character
    // streaming in and out reallocated it every cycle.
    let Some(mut r) = renderer() else {
        return Ok(());
    };
    let mesh = skinning::cylinder(0.3, 1.0, 8, 2);
    let n = mesh.positions.len();
    r.upload_mesh(&MeshData::cube(0.5))?;
    let s = r.upload_skinned_mesh(&mesh, &MeshSkin::rigid(n, 0, 1))?;
    let with_skin = r.mesh_stats().capacity_bytes;
    assert!(r.destroy_mesh(s));
    for _ in 0..20 {
        let after = r.mesh_stats();
        assert_eq!((after.skinned_meshes, after.skin_bytes_used), (0, 0));
        assert_eq!(
            after.capacity_bytes, with_skin,
            "skin buffer kept on page 0"
        );
        let s = r.upload_skinned_mesh(&mesh, &MeshSkin::rigid(n, 0, 1))?;
        assert_eq!(r.mesh_stats().capacity_bytes, with_skin, "no new buffer");
        assert_eq!(r.mesh_stats().pages, 1);
        assert!(r.destroy_mesh(s));
    }
    Ok(())
}

#[test]
fn idle_skin_buffers_yield_to_the_byte_cap() -> Result<(), RenderError> {
    // Pages of 256 vertices; the cap holds two pages, or one page plus its skin buffer
    // and not a second page.
    let page = 256 * 40 + 1024 * 4;
    let config = MeshConfig {
        page_vertex_bytes: 256 * 40,
        page_index_bytes: 1024 * 4,
        max_total_bytes: 2 * page,
        wireframe_edges: false,
        ..MeshConfig::default()
    };
    let Some(mut r) = gpu::headless_renderer_with(|c| {
        c.clear_color = BG;
        c.meshes = config;
    }) else {
        return Ok(());
    };
    let mesh = skinning::cylinder(0.3, 1.0, 8, 2);
    let n = mesh.positions.len();
    let s = r.upload_skinned_mesh(&mesh, &MeshSkin::rigid(n, 0, 1))?;
    assert!(r.destroy_mesh(s));
    let idle = r.mesh_stats().capacity_bytes;
    assert_eq!(idle, page + 256 * 12, "page 0 and its kept skin buffer");
    // Fill page 0 with static meshes, then need a second page: the idle skin buffer is
    // released to make room.
    let big = MeshData::cube(0.5);
    let per = big.positions.len() as u64;
    for _ in 0..256 / per {
        r.upload_mesh(&big)?;
    }
    r.upload_mesh(&big)?;
    let stats = r.mesh_stats();
    assert_eq!(stats.pages, 2);
    assert_eq!(stats.capacity_bytes, 2 * page);
    Ok(())
}
