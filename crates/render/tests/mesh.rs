// SPDX-License-Identifier: GPL-3.0-or-later
//! Behaviour of mesh uploads and mesh draws on a real (or software) adapter: the shared
//! buffer arena, handle checks, and pixel-level checks of each material feature. Every
//! test skips (with a printed reason) when no GPU adapter is available.

#[path = "support/gpu.rs"]
mod gpu;

use std::f32::consts::PI;

use tessera_render::glam::{Mat4, Vec3};
use tessera_render::{
    AlphaMode, Camera, ColorSpace, Dye, DyeMask, Filter, Frame, Lighting, Material, MeshConfig,
    MeshData, MeshDebug, MeshDraw, MeshId, MeshLimits, MeshShading, NormalLines, OffscreenFormat,
    PassSpace, RenderError, Renderer, RgbaImage, TextureUpload, Wrap,
};

const BG: [f32; 4] = [0.0, 0.0, 0.0, 1.0];
const SIZE: u32 = 64;

fn renderer() -> Option<gpu::Headless> {
    gpu::headless_renderer(BG)
}

/// Renderer with a custom mesh configuration (same adapter search as `gpu.rs`).
fn renderer_with(meshes: MeshConfig) -> Option<gpu::Headless> {
    gpu::headless_renderer_with(|c| {
        c.clear_color = BG;
        c.meshes = meshes;
    })
}

/// Camera on +Z looking at the origin; a unit plane at the origin fills the middle.
fn front_camera() -> Camera {
    Camera {
        eye: Vec3::new(0.0, 0.0, 3.0),
        target: Vec3::ZERO,
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

fn center(img: &RgbaImage) -> [u8; 4] {
    img.pixel(SIZE / 2, SIZE / 2).unwrap_or([0; 4])
}

fn near(a: [u8; 4], b: [u8; 4], tol: u8) -> bool {
    a.iter().zip(b).all(|(x, y)| x.abs_diff(y) <= tol)
}

fn luma(p: [u8; 4]) -> u32 {
    u32::from(p[0]) + u32::from(p[1]) + u32::from(p[2])
}

/// One full-pass draw of `mesh` with `material`, unlit by default.
fn draw_one(
    r: &mut Renderer<'static>,
    mesh: MeshId,
    transform: Mat4,
    material: Material,
    lighting: Lighting,
) -> Result<RgbaImage, RenderError> {
    let cam = front_camera();
    render(r, |f| {
        f.world_pass("one", &cam)
            .clear(BG)
            .lighting(lighting)
            .mesh(MeshDraw::new(mesh, transform, material));
    })
}

#[test]
fn upload_destroy_and_stats() -> Result<(), RenderError> {
    let Some(mut r) = renderer() else {
        return Ok(());
    };
    let before = r.mesh_stats();
    assert_eq!(before.meshes, 0);
    let cube = r.upload_mesh(&MeshData::cube(0.5))?;
    let sphere = r.upload_mesh(&MeshData::uv_sphere(1.0, 8, 4))?;
    let stats = r.mesh_stats();
    assert_eq!((stats.meshes, stats.pages), (2, 1));
    assert_eq!(stats.vertex_bytes_used, (24 + 9 * 5) * 40);
    let info = r.mesh_info(cube).expect("cube info");
    assert_eq!((info.vertex_count, info.index_count), (24, 36));
    assert_eq!(
        info.edge_index_count,
        6 * 5 * 2,
        "5 unique edges per face quad"
    );
    assert!((info.sphere.radius - 0.75f32.sqrt()).abs() < 1e-5);
    assert!(r.destroy_mesh(cube));
    assert!(!r.destroy_mesh(cube), "double destroy");
    assert!(r.mesh_info(cube).is_none());
    // Invalid uploads allocate nothing.
    let bad = MeshData::new(vec![[0.0; 3]; 3], vec![0, 1, 7]);
    assert!(matches!(
        r.upload_mesh(&bad),
        Err(RenderError::InvalidMesh(_))
    ));
    assert_eq!(r.mesh_stats().meshes, 1);
    // The freed range is reused: a new cube lands where the old one was.
    let again = r.upload_mesh(&MeshData::cube(0.5))?;
    assert_ne!(again, cube, "stale handle never aliases");
    assert_eq!(r.mesh_stats().pages, 1);
    assert!(r.destroy_mesh(sphere) && r.destroy_mesh(again));
    assert_eq!(r.mesh_stats().vertex_bytes_used, 0);
    Ok(())
}

#[test]
fn arena_pages_and_caps() -> Result<(), RenderError> {
    // Pages of 64 vertices / 256 indices; a cube (24 / 36 + 60 edges) fits twice.
    let config = MeshConfig {
        page_vertex_bytes: 64 * 40,
        page_index_bytes: 256 * 4,
        max_total_bytes: 2 * (64 * 40 + 256 * 4) + 600_000,
        ..MeshConfig::default()
    };
    let Some(mut r) = renderer_with(config) else {
        return Ok(());
    };
    let a = r.upload_mesh(&MeshData::cube(0.5))?;
    let b = r.upload_mesh(&MeshData::cube(0.5))?;
    assert_eq!(r.mesh_stats().pages, 1);
    let c = r.upload_mesh(&MeshData::cube(0.5))?;
    assert_eq!(r.mesh_stats().pages, 2, "third cube opens a second page");
    // Larger than a default page: gets a page sized for it.
    let big = r.upload_mesh(&MeshData::uv_sphere(1.0, 63, 63))?;
    assert_eq!(r.mesh_stats().pages, 3);
    // Beyond the byte cap.
    let huge = MeshData::plane(1.0, 1.0, 200, 200);
    assert!(matches!(
        r.upload_mesh(&huge),
        Err(RenderError::LimitExceeded { .. })
    ));
    // Emptying a non-first page releases it.
    assert!(r.destroy_mesh(big));
    assert_eq!(r.mesh_stats().pages, 2);
    assert!(r.destroy_mesh(c));
    assert_eq!(r.mesh_stats().pages, 1);
    // The first page is kept even when empty.
    assert!(r.destroy_mesh(a) && r.destroy_mesh(b));
    let s = r.mesh_stats();
    assert_eq!((s.pages, s.meshes, s.vertex_bytes_used), (1, 0, 0));
    // Per-mesh limits.
    let limited = MeshConfig {
        limits: MeshLimits {
            max_vertices: 10,
            ..MeshLimits::default()
        },
        ..MeshConfig::default()
    };
    let Some(mut r) = renderer_with(limited) else {
        return Ok(());
    };
    assert!(matches!(
        r.upload_mesh(&MeshData::cube(1.0)),
        Err(RenderError::LimitExceeded { .. })
    ));
    Ok(())
}

#[test]
fn draws_from_several_pages_render() -> Result<(), RenderError> {
    let config = MeshConfig {
        page_vertex_bytes: 30 * 40,
        page_index_bytes: 100 * 4,
        ..MeshConfig::default()
    };
    let Some(mut r) = renderer_with(config) else {
        return Ok(());
    };
    let left = r.upload_mesh(&MeshData::plane(1.0, 2.0, 1, 1))?;
    let right = r.upload_mesh(&MeshData::cube(0.5))?;
    assert_eq!(r.mesh_stats().pages, 2);
    let cam = front_camera();
    let unlit = |c| Material {
        unlit: true,
        ..Material::color(c)
    };
    let img = render(&mut r, |f| {
        f.world_pass("pages", &cam)
            .clear(BG)
            .mesh(MeshDraw::new(
                left,
                Mat4::from_translation(Vec3::new(-0.6, 0.0, 0.0)),
                unlit([1.0, 0.0, 0.0, 1.0]),
            ))
            .mesh(MeshDraw::new(
                right,
                Mat4::from_translation(Vec3::new(0.6, 0.0, 0.0)),
                unlit([0.0, 0.0, 1.0, 1.0]),
            ));
    })?;
    let px = |x| img.pixel(x, SIZE / 2).unwrap_or([0; 4]);
    assert!(near(px(SIZE / 2 - 12), [255, 0, 0, 255], 2), "{:?}", px(20));
    assert!(near(px(SIZE / 2 + 12), [0, 0, 255, 255], 2), "{:?}", px(44));
    Ok(())
}

#[test]
fn stale_foreign_and_misplaced_draws_are_skipped() -> Result<(), RenderError> {
    let Some(mut r) = renderer() else {
        return Ok(());
    };
    let Some(mut other) = renderer() else {
        return Ok(());
    };
    let foreign = other.upload_mesh(&MeshData::cube(0.5))?;
    let stale = r.upload_mesh(&MeshData::cube(0.5))?;
    assert!(r.destroy_mesh(stale));
    assert!(
        r.mesh_info(foreign).is_none(),
        "foreign handle not resolved"
    );
    assert!(!r.destroy_mesh(foreign));
    let live = r.upload_mesh(&MeshData::cube(0.5))?;
    let cam = front_camera();
    let target = r.create_offscreen_target(SIZE, SIZE, OffscreenFormat::Rgba8Unorm)?;
    let mut frame = r.begin_offscreen_frame(&target)?;
    frame
        .world_pass("w", &cam)
        .clear(BG)
        .mesh(MeshDraw::new(stale, Mat4::IDENTITY, Material::default()))
        .mesh(MeshDraw::new(foreign, Mat4::IDENTITY, Material::default()))
        .mesh(MeshDraw::new(live, Mat4::IDENTITY, Material::default()).with_submesh(5))
        .mesh(MeshDraw::new(live, Mat4::IDENTITY, Material::default()));
    frame.pass("screen", PassSpace::Screen).mesh(MeshDraw::new(
        live,
        Mat4::IDENTITY,
        Material::default(),
    ));
    let stats = r.end_frame(frame)?;
    assert_eq!(stats.mesh_draws, 1, "only the live, whole-mesh draw");
    assert_eq!(stats.triangles, 12);
    // A non-finite transform is drawn (and clipped away) without errors.
    let img = draw_one(
        &mut r,
        live,
        Mat4::from_cols_array(&[f32::NAN; 16]),
        Material::default(),
        Lighting::default(),
    )?;
    assert_eq!(center(&img), [0, 0, 0, 255]);
    Ok(())
}

#[test]
fn alpha_test_threshold_is_a_uniform() -> Result<(), RenderError> {
    let Some(mut r) = renderer() else {
        return Ok(());
    };
    // Alpha ramp along u (0 at the left edge, 1 at the right).
    let ramp: Vec<u8> = (0..16 * 16)
        .flat_map(|i| [255, 255, 255, ((i % 16) * 17) as u8])
        .collect();
    let tex = r.upload_texture(&TextureUpload {
        filter: Filter::Nearest,
        wrap: Wrap::Clamp,
        ..TextureUpload::rgba8(16, 16, &ramp)
    })?;
    let plane = r.upload_mesh(&MeshData::plane(2.0, 2.0, 1, 1))?;
    let lit_columns = |r: &mut Renderer<'static>, cutoff: f32| -> Result<u32, RenderError> {
        let img = draw_one(
            r,
            plane,
            Mat4::IDENTITY,
            Material {
                alpha: AlphaMode::Mask { cutoff },
                unlit: true,
                ..Material::textured(tex)
            },
            Lighting::default(),
        )?;
        Ok((0..SIZE)
            .filter(|&x| img.pixel(x, SIZE / 2).is_some_and(|p| p[0] > 128))
            .count() as u32)
    };
    let low = lit_columns(&mut r, 0.2)?;
    let mid = lit_columns(&mut r, 0.5)?;
    let high = lit_columns(&mut r, 0.8)?;
    assert!(low > mid && mid > high && high > 0, "{low} {mid} {high}");
    Ok(())
}

#[test]
fn two_sided_shows_back_faces_with_flipped_normals() -> Result<(), RenderError> {
    let Some(mut r) = renderer() else {
        return Ok(());
    };
    let plane = r.upload_mesh(&MeshData::plane(2.0, 2.0, 1, 1))?;
    let behind = Mat4::from_rotation_y(PI);
    let light = Lighting {
        direction: Vec3::Z,
        color: [1.0; 3],
        ambient: [0.0; 3],
    };
    let one = draw_one(&mut r, plane, behind, Material::default(), light)?;
    assert_eq!(center(&one), [0, 0, 0, 255], "back face culled");
    let two = draw_one(
        &mut r,
        plane,
        behind,
        Material {
            two_sided: true,
            ..Material::default()
        },
        light,
    )?;
    // The back face's flipped normal faces the camera and the light: fully lit.
    assert!(
        near(center(&two), [255, 255, 255, 255], 3),
        "{:?}",
        center(&two)
    );
    // Mirrored transforms flip the winding; they are still drawn front-facing.
    let mirrored = draw_one(
        &mut r,
        plane,
        Mat4::from_scale(Vec3::new(-1.0, 1.0, 1.0)),
        Material::default(),
        light,
    )?;
    assert!(
        near(center(&mirrored), [255, 255, 255, 255], 3),
        "{:?}",
        center(&mirrored)
    );
    Ok(())
}

#[test]
fn mirrored_cube_matches_unmirrored() -> Result<(), RenderError> {
    let Some(mut r) = renderer() else {
        return Ok(());
    };
    let cube = r.upload_mesh(&MeshData::cube(0.5))?;
    let spin = Mat4::from_rotation_y(0.6) * Mat4::from_rotation_x(0.4);
    let plain = draw_one(&mut r, cube, spin, Material::default(), Lighting::default())?;
    let mirrored = draw_one(
        &mut r,
        cube,
        spin * Mat4::from_scale(Vec3::new(-1.0, 1.0, 1.0)),
        Material::default(),
        Lighting::default(),
    )?;
    let differing = plain
        .data
        .as_chunks::<4>()
        .0
        .iter()
        .zip(mirrored.data.as_chunks::<4>().0)
        .filter(|(a, b)| a.iter().zip(*b).any(|(x, y)| x.abs_diff(*y) > 3))
        .count();
    assert!(differing <= 8, "{differing} pixels differ");
    Ok(())
}

#[test]
fn dye_tints_masked_texels() -> Result<(), RenderError> {
    let Some(mut r) = renderer() else {
        return Ok(());
    };
    let plane = r.upload_mesh(&MeshData::plane(2.0, 2.0, 1, 1))?;
    let mut mask = |v: u8| {
        r.upload_texture(&TextureUpload {
            color_space: ColorSpace::Linear,
            ..TextureUpload::rgba8(1, 1, &[v, 0, 0, 255])
        })
    };
    let (full, none, half) = (mask(255)?, mask(0)?, mask(128)?);
    let dyed = |r: &mut Renderer<'static>, m: DyeMask| {
        draw_one(
            r,
            plane,
            Mat4::IDENTITY,
            Material {
                unlit: true,
                dye: Some(Dye {
                    color: [1.0, 0.0, 0.0],
                    mask: m,
                }),
                ..Material::default()
            },
            Lighting::default(),
        )
        .map(|img| center(&img))
    };
    assert!(near(
        dyed(&mut r, DyeMask::Texture(full))?,
        [255, 0, 0, 255],
        1
    ));
    assert!(near(
        dyed(&mut r, DyeMask::Texture(none))?,
        [255, 255, 255, 255],
        1
    ));
    let h = dyed(&mut r, DyeMask::Texture(half))?;
    assert!(h[0] == 255 && (120..=136).contains(&h[1]), "{h:?}");
    // Base-alpha mask: alpha 0 → undyed; coverage alpha stays the factor's.
    let base = r.upload_texture(&TextureUpload::rgba8(1, 1, &[255, 255, 255, 0]))?;
    let img = draw_one(
        &mut r,
        plane,
        Mat4::IDENTITY,
        Material {
            unlit: true,
            alpha: AlphaMode::Mask { cutoff: 0.5 },
            dye: Some(Dye {
                color: [0.0, 1.0, 0.0],
                mask: DyeMask::BaseAlpha,
            }),
            ..Material::textured(base)
        },
        Lighting::default(),
    )?;
    assert!(
        near(center(&img), [255, 255, 255, 255], 1),
        "{:?}",
        center(&img)
    );
    // A destroyed mask texture disables the dye instead of failing the frame.
    assert!(r.destroy_texture(full));
    assert!(near(
        dyed(&mut r, DyeMask::Texture(full))?,
        [255, 255, 255, 255],
        1
    ));
    Ok(())
}

#[test]
fn normal_map_tilts_lighting_in_tangent_space() -> Result<(), RenderError> {
    let Some(mut r) = renderer() else {
        return Ok(());
    };
    let plane = r.upload_mesh(&MeshData::plane(2.0, 2.0, 1, 1))?;
    let mut no_tangents = MeshData::plane(2.0, 2.0, 1, 1);
    no_tangents.tangents = None;
    let no_tangents = r.upload_mesh(&no_tangents)?;
    // Constant tangent-space normals, tilted 45° along +u, -u, +v.
    let encode = |n: Vec3| {
        let n = n.normalize();
        let e = |c: f32| ((c * 0.5 + 0.5) * 255.0).round() as u8;
        [e(n.x), e(n.y), e(n.z), 255]
    };
    let mut map = |n: Vec3| {
        r.upload_texture(&TextureUpload {
            color_space: ColorSpace::Linear,
            ..TextureUpload::rgba8(1, 1, &encode(n))
        })
    };
    let plus_u = map(Vec3::new(1.0, 0.0, 1.0))?;
    let minus_u = map(Vec3::new(-1.0, 0.0, 1.0))?;
    let plus_v = map(Vec3::new(0.0, 1.0, 1.0))?;
    let from = |d: Vec3| Lighting {
        direction: d,
        color: [1.0; 3],
        ambient: [0.0; 3],
    };
    let mut shade = |mesh, tex, green_up, light| -> Result<u32, RenderError> {
        let img = draw_one(
            &mut r,
            mesh,
            Mat4::IDENTITY,
            Material {
                normal_texture: Some(tex),
                normal_green_up: green_up,
                ..Material::default()
            },
            light,
        )?;
        Ok(luma(center(&img)))
    };
    // Light from +X (+u on the plane) and the camera side.
    let right = from(Vec3::new(1.0, 0.0, 1.0));
    let flat = shade(no_tangents, plus_u, false, right)?;
    let towards = shade(plane, plus_u, false, right)?;
    let away = shade(plane, minus_u, false, right)?;
    assert!(towards > flat && flat > away, "{towards} {flat} {away}");
    // +v runs down the image (-Y on this plane): lit from below by default, from
    // above with the green-up convention.
    let below = from(Vec3::new(0.0, -1.0, 1.0));
    let above = from(Vec3::new(0.0, 1.0, 1.0));
    assert!(shade(plane, plus_v, false, below)? > shade(plane, plus_v, false, above)?);
    assert!(shade(plane, plus_v, true, above)? > shade(plane, plus_v, true, below)?);
    Ok(())
}

#[test]
fn specular_brightens_the_highlight() -> Result<(), RenderError> {
    let Some(mut r) = renderer() else {
        return Ok(());
    };
    let plane = r.upload_mesh(&MeshData::plane(2.0, 2.0, 1, 1))?;
    let light = Lighting {
        direction: Vec3::Z,
        color: [0.5; 3],
        ambient: [0.0; 3],
    };
    let base = Material::color([0.5, 0.5, 0.5, 1.0]);
    let matte = draw_one(&mut r, plane, Mat4::IDENTITY, base, light)?;
    let shiny = draw_one(
        &mut r,
        plane,
        Mat4::IDENTITY,
        Material {
            specular: Some(tessera_render::Specular {
                color: [1.0; 3],
                shininess: 16.0,
                texture: None,
            }),
            ..base
        },
        light,
    )?;
    assert!(luma(center(&shiny)) > luma(center(&matte)) + 100);
    Ok(())
}

#[test]
fn debug_views_and_overlays() -> Result<(), RenderError> {
    let Some(mut r) = renderer() else {
        return Ok(());
    };
    let plane = r.upload_mesh(&MeshData::plane(2.0, 2.0, 1, 1))?;
    let cam = front_camera();
    let view = |r: &mut Renderer<'static>, debug: MeshDebug| {
        render(r, |f| {
            f.world_pass("debug", &cam)
                .clear(BG)
                .mesh_debug(debug)
                .mesh(MeshDraw::new(
                    plane,
                    Mat4::IDENTITY,
                    Material::color([0.0, 0.0, 0.0, 1.0]),
                ));
        })
    };
    // Normals: +Z → (0.5, 0.5, 1.0).
    let n = view(
        &mut r,
        MeshDebug {
            shading: MeshShading::Normals,
            ..MeshDebug::default()
        },
    )?;
    assert!(
        near(center(&n), [128, 128, 255, 255], 2),
        "{:?}",
        center(&n)
    );
    // Hidden + wireframe: only the edges (the border and one diagonal).
    let w = view(
        &mut r,
        MeshDebug {
            shading: MeshShading::Hidden,
            wireframe: Some([0.0, 1.0, 0.0, 1.0]),
            normals: None,
        },
    )?;
    let green = w
        .data
        .as_chunks::<4>()
        .0
        .iter()
        .filter(|p| p[1] > 200)
        .count();
    assert!(green > 4 * 20 && green < 400, "{green} wire pixels");
    assert_eq!(w.pixel(SIZE / 2 - 8, SIZE / 2 + 8), Some([0, 0, 0, 255]));
    // UV checker: two neighbouring cells differ.
    let uv = view(
        &mut r,
        MeshDebug {
            shading: MeshShading::UvChecker,
            ..MeshDebug::default()
        },
    )?;
    let cells: Vec<u32> = (0..8)
        .filter_map(|i| uv.pixel(SIZE / 2 - 10 + i * 3, SIZE / 2 - 1).map(luma))
        .collect();
    assert!(cells.iter().max() > cells.iter().min(), "{cells:?}");
    // Normal lines on a black plane add coloured pixels; base colour view is black.
    let lines = view(
        &mut r,
        MeshDebug {
            shading: MeshShading::BaseColor,
            wireframe: None,
            normals: Some(NormalLines {
                length: 0.5,
                color: [1.0, 0.0, 1.0, 1.0],
            }),
        },
    )?;
    let magenta = lines
        .data
        .as_chunks::<4>()
        .0
        .iter()
        .filter(|p| p[0] > 200 && p[2] > 200)
        .count();
    assert!(magenta >= 4, "{magenta} normal-line pixels");
    assert_eq!(center(&lines), [0, 0, 0, 255]);
    Ok(())
}

#[test]
fn wireframe_without_edges_is_skipped() -> Result<(), RenderError> {
    let Some(mut r) = renderer_with(MeshConfig {
        wireframe_edges: false,
        ..MeshConfig::default()
    }) else {
        return Ok(());
    };
    let plane = r.upload_mesh(&MeshData::plane(2.0, 2.0, 1, 1))?;
    assert_eq!(r.mesh_info(plane).map(|i| i.edge_index_count), Some(0));
    let cam = front_camera();
    let img = render(&mut r, |f| {
        f.world_pass("w", &cam)
            .clear(BG)
            .mesh_debug(MeshDebug {
                shading: MeshShading::Hidden,
                wireframe: Some([1.0; 4]),
                normals: None,
            })
            .mesh(MeshDraw::new(plane, Mat4::IDENTITY, Material::default()));
    })?;
    assert!(
        img.data
            .as_chunks::<4>()
            .0
            .iter()
            .all(|p| *p == [0, 0, 0, 255])
    );
    Ok(())
}

#[test]
fn material_texture_that_is_the_frame_target_falls_back_to_white() -> Result<(), RenderError> {
    let Some(mut r) = renderer() else {
        return Ok(());
    };
    let plane = r.upload_mesh(&MeshData::plane(2.0, 2.0, 1, 1))?;
    let target = r.create_offscreen_target(SIZE, SIZE, OffscreenFormat::Rgba8Unorm)?;
    let tex = r.register_target_texture(&target, Filter::Linear, Wrap::Clamp)?;
    let cam = front_camera();
    let mut frame = r.begin_offscreen_frame(&target)?;
    frame.world_pass("self", &cam).clear(BG).mesh(MeshDraw::new(
        plane,
        Mat4::IDENTITY,
        Material {
            unlit: true,
            ..Material::textured(tex)
        },
    ));
    r.end_frame(frame)?;
    let img = r.read_pixels(&target)?;
    assert_eq!(center(&img), [255, 255, 255, 255]);
    Ok(())
}

/// Constant tangent-space normal map texture (`n` need not be normalised).
fn constant_normal_map(
    r: &mut Renderer<'static>,
    n: Vec3,
) -> Result<tessera_render::TextureId, RenderError> {
    let n = n.normalize();
    let e = |c: f32| ((c * 0.5 + 0.5) * 255.0).round() as u8;
    r.upload_texture(&TextureUpload {
        color_space: ColorSpace::Linear,
        ..TextureUpload::rgba8(1, 1, &[e(n.x), e(n.y), e(n.z), 255])
    })
}

/// Centre pixel of `mesh` drawn at `transform` in the world-normals debug view, seen
/// from the front camera (+Z).
fn world_normal_at_center(
    r: &mut Renderer<'static>,
    mesh: MeshId,
    transform: Mat4,
    material: Material,
) -> Result<[u8; 4], RenderError> {
    let cam = front_camera();
    let img = render(r, |f| {
        f.world_pass("normals", &cam)
            .clear(BG)
            .mesh_debug(MeshDebug {
                shading: MeshShading::Normals,
                ..MeshDebug::default()
            })
            .mesh(MeshDraw::new(mesh, transform, material));
    })?;
    Ok(center(&img))
}

/// `n * 0.5 + 0.5` as the debug view encodes it.
fn encoded(n: Vec3) -> [u8; 4] {
    let e = |c: f32| ((c * 0.5 + 0.5) * 255.0).round() as u8;
    [e(n.x), e(n.y), e(n.z), 255]
}

#[test]
fn two_sided_back_faces_negate_the_mapped_normal() -> Result<(), RenderError> {
    // Regression: the back-face flip used to happen before the tangent frame was
    // built, so the tangent (red) component of a normal map kept its front-side
    // direction on back faces.
    let Some(mut r) = renderer() else {
        return Ok(());
    };
    // Plane facing +Z: tangent +X, bitangent (+v) -Y.
    let plane = r.upload_mesh(&MeshData::plane(2.0, 2.0, 1, 1))?;
    let behind = Mat4::from_rotation_y(PI);
    for (tn, front_world) in [
        (Vec3::X, Vec3::X),
        (Vec3::Y, -Vec3::Y),
        (Vec3::new(1.0, 1.0, 1.0), Vec3::new(1.0, -1.0, 1.0)),
    ] {
        let map = constant_normal_map(&mut r, tn)?;
        let material = Material {
            two_sided: true,
            normal_texture: Some(map),
            ..Material::default()
        };
        let front = world_normal_at_center(&mut r, plane, Mat4::IDENTITY, material)?;
        let want_front = encoded(front_world.normalize());
        assert!(
            near(front, want_front, 3),
            "front {tn}: {front:?} != {want_front:?}"
        );
        // Rotated half a turn the camera sees the back face: the shown normal is the
        // rotated front normal, negated.
        let back = world_normal_at_center(&mut r, plane, behind, material)?;
        let want_back = encoded(-(behind.transform_vector3(front_world.normalize())));
        assert!(
            near(back, want_back, 3),
            "back {tn}: {back:?} != {want_back:?}"
        );
    }
    Ok(())
}

#[test]
fn mirrored_draws_keep_the_tangent_frame() -> Result<(), RenderError> {
    // A mirrored (negative determinant) draw flips the bitangent sign so that the
    // normal map is mirrored with the geometry: +v stays -Y under a mirror in X.
    let Some(mut r) = renderer() else {
        return Ok(());
    };
    let plane = r.upload_mesh(&MeshData::plane(2.0, 2.0, 1, 1))?;
    let mirror_x = Mat4::from_scale(Vec3::new(-1.0, 1.0, 1.0));
    for (tn, plain_world) in [(Vec3::X, Vec3::X), (Vec3::Y, -Vec3::Y)] {
        let map = constant_normal_map(&mut r, tn)?;
        let material = Material {
            normal_texture: Some(map),
            ..Material::default()
        };
        let plain = world_normal_at_center(&mut r, plane, Mat4::IDENTITY, material)?;
        assert!(near(plain, encoded(plain_world), 3), "{tn}: {plain:?}");
        // The mirrored image of the plain result: X flips, Y and Z do not.
        let mirrored = world_normal_at_center(&mut r, plane, mirror_x, material)?;
        let want = encoded(plain_world * Vec3::new(-1.0, 1.0, 1.0));
        assert!(
            near(mirrored, want, 3),
            "mirrored {tn}: {mirrored:?} != {want:?}"
        );
    }
    Ok(())
}

#[test]
fn mirrored_two_sided_cards_flip_only_back_faces() -> Result<(), RenderError> {
    // Mirroring reverses the screen-space winding, so the shader must swap the
    // rasteriser's front/back decision before flipping normals of two-sided draws.
    let Some(mut r) = renderer() else {
        return Ok(());
    };
    let plane = r.upload_mesh(&MeshData::plane(2.0, 2.0, 1, 1))?;
    let mirror_x = Mat4::from_scale(Vec3::new(-1.0, 1.0, 1.0));
    let material = Material {
        two_sided: true,
        ..Material::default()
    };
    let toward_camera = encoded(Vec3::Z);
    for (name, base) in [
        ("front", Mat4::IDENTITY),
        ("back", Mat4::from_rotation_y(PI)),
    ] {
        // Front or back face: the shown normal always faces the camera, mirrored
        // or not.
        let plain = world_normal_at_center(&mut r, plane, base, material)?;
        let mirrored = world_normal_at_center(&mut r, plane, base * mirror_x, material)?;
        assert!(near(plain, toward_camera, 3), "{name}: {plain:?}");
        assert!(
            near(mirrored, toward_camera, 3),
            "{name} mirrored: {mirrored:?}"
        );
    }
    Ok(())
}

#[test]
fn oversized_first_page_is_released() -> Result<(), RenderError> {
    // Regression: page 0 was always kept, even when it had been sized for one
    // large mesh, pinning that memory for the renderer's lifetime.
    let default_page = 64 * 40 + 256 * 4;
    let config = MeshConfig {
        page_vertex_bytes: 64 * 40,
        page_index_bytes: 256 * 4,
        max_total_bytes: 3_000_000,
        ..MeshConfig::default()
    };
    let Some(mut r) = renderer_with(config) else {
        return Ok(());
    };
    let big = r.upload_mesh(&MeshData::plane(1.0, 1.0, 100, 100))?;
    assert!(r.mesh_stats().capacity_bytes > 100 * default_page);
    assert!(r.destroy_mesh(big));
    let s = r.mesh_stats();
    assert_eq!((s.pages, s.capacity_bytes), (0, 0), "{s:?}");
    // A default-sized first page is still kept warm.
    let cube = r.upload_mesh(&MeshData::cube(0.5))?;
    assert!(r.destroy_mesh(cube));
    let s = r.mesh_stats();
    assert_eq!((s.pages, s.capacity_bytes), (1, default_page), "{s:?}");
    // And a later big mesh can still use the whole budget.
    let sphere = r.upload_mesh(&MeshData::uv_sphere(1.0, 200, 100))?;
    assert!(r.destroy_mesh(sphere));
    assert!(r.mesh_stats().capacity_bytes <= default_page);
    Ok(())
}

#[test]
fn prepared_meshes_upload_from_another_thread() -> Result<(), RenderError> {
    let Some(mut r) = renderer() else {
        return Ok(());
    };
    let data = MeshData::uv_sphere(1.0, 16, 8);
    let config = MeshConfig::default();
    let worker = std::thread::spawn(move || data.prepare(&config));
    let prepared = worker
        .join()
        .map_err(|_| RenderError::Gpu("worker panicked".into()))??;
    let expected = prepared.info().clone();
    let id = r.upload_prepared(prepared)?;
    assert_eq!(r.mesh_info(id), Some(&expected));
    // A renderer without wireframe edges drops them from a prepared mesh, so its
    // summary matches `validate` under the same configuration.
    let no_edges = MeshConfig {
        wireframe_edges: false,
        ..MeshConfig::default()
    };
    let Some(mut plain) = renderer_with(no_edges) else {
        return Ok(());
    };
    let cube = MeshData::cube(1.0);
    let id = plain.upload_prepared(cube.prepare(&MeshConfig::default())?)?;
    assert_eq!(plain.mesh_info(id), Some(&cube.validate(&no_edges)?));
    assert_eq!(plain.mesh_stats().index_bytes_used, 36 * 4);
    let id = plain.upload_mesh(&cube)?;
    assert_eq!(plain.mesh_info(id).map(|i| i.edge_index_count), Some(0));
    // Limits are re-checked against the uploading renderer's configuration.
    let tight = MeshConfig {
        limits: MeshLimits {
            max_vertices: 10,
            ..MeshLimits::default()
        },
        ..MeshConfig::default()
    };
    let Some(mut small) = renderer_with(tight) else {
        return Ok(());
    };
    assert!(matches!(
        small.upload_prepared(cube.prepare(&MeshConfig::default())?),
        Err(RenderError::LimitExceeded {
            what: "mesh vertices",
            ..
        })
    ));
    assert_eq!(small.mesh_stats().meshes, 0);
    Ok(())
}

#[test]
fn oversized_readback_fails_cleanly() -> Result<(), RenderError> {
    // Regression: a target whose readback buffer exceeded max_buffer_size was
    // accepted, read_pixels failed vaguely, and the stale validation error was then
    // reported by (and destroyed) the next, unrelated mesh upload.
    let Some(mut r) = renderer() else {
        return Ok(());
    };
    let limits = r.context().device.limits();
    let w = limits.max_texture_dimension_2d;
    let row = (u64::from(w) * 4).div_ceil(256) * 256;
    let h = limits.max_buffer_size / row + 1;
    let Ok(h) = u32::try_from(h) else {
        return Ok(());
    };
    if h > limits.max_texture_dimension_2d {
        eprintln!("skipping: every target fits in one readback buffer here");
        return Ok(());
    }
    let target = match r.create_offscreen_target(w, h, OffscreenFormat::Rgba8Unorm) {
        Ok(t) => t,
        Err(RenderError::Gpu(e)) => {
            eprintln!("skipping: the device cannot allocate a {w}x{h} target: {e}");
            return Ok(());
        }
        Err(e) => return Err(e),
    };
    assert!(matches!(
        r.read_pixels(&target),
        Err(RenderError::LimitExceeded {
            what: "readback buffer bytes",
            ..
        })
    ));
    drop(target);
    let cube = r.upload_mesh(&MeshData::cube(1.0))?;
    assert!(r.mesh_info(cube).is_some());
    let img = draw_one(
        &mut r,
        cube,
        Mat4::IDENTITY,
        Material::default(),
        Lighting::default(),
    )?;
    assert_ne!(center(&img), [0, 0, 0, 255]);
    Ok(())
}

#[test]
fn many_materials_render_across_frames() -> Result<(), RenderError> {
    // More distinct texture sets than the material cache's base size, drawn on
    // consecutive frames: every draw keeps its own texture (the cache evicts only
    // entries the current frame does not use).
    let Some(mut r) = renderer() else {
        return Ok(());
    };
    let plane = r.upload_mesh(&MeshData::plane(2.0, 2.0, 1, 1))?;
    let n = 1100u32;
    let mut textures = Vec::new();
    for i in 0..n {
        let level = if i + 1 == n { 255 } else { (i % 200) as u8 };
        textures.push(r.upload_texture(&TextureUpload::rgba8(1, 1, &[level, 0, 0, 255]))?);
    }
    let cam = front_camera();
    for _ in 0..3 {
        let img = render(&mut r, |f| {
            let pass = f.world_pass("many", &cam).clear(BG);
            for (i, &t) in textures.iter().enumerate() {
                // Later draws win ties: the last texture covers the plane.
                let z = i as f32 * 1e-4;
                pass.mesh(MeshDraw::new(
                    plane,
                    Mat4::from_translation(Vec3::new(0.0, 0.0, z)),
                    Material {
                        unlit: true,
                        ..Material::textured(t)
                    },
                ));
            }
        })?;
        assert!(
            near(center(&img), [255, 0, 0, 255], 2),
            "{:?}",
            center(&img)
        );
    }
    Ok(())
}
