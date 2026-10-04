// SPDX-License-Identifier: GPL-3.0-or-later
//! Golden images of the mesh pipelines, one scene per pipeline: opaque, alpha-test,
//! alpha-blend, two-sided, dyeable, normal-mapped and the debug views.
//!
//! Every mesh comes from the renderer's own procedural shapes and every texture is
//! generated here, so the images contain no game data. Regenerate with
//! `TESSERA_BLESS=1 cargo test -p tessera-render --test mesh_golden` and review the PNGs in
//! `tests/golden/` (see `tests/golden/README.md` for the drivers they are checked on).

#[path = "support/golden.rs"]
mod golden;
#[path = "support/gpu.rs"]
mod gpu;

use std::f32::consts::{FRAC_PI_2, FRAC_PI_4};
use std::path::Path;

use tessera_render::glam::{Mat4, Quat, Vec3};
use tessera_render::{
    AlphaMode, Camera, ColorSpace, Dye, DyeMask, Filter, Frame, Lighting, Material, MeshData,
    MeshDebug, MeshDraw, MeshId, MeshShading, NormalLines, OffscreenFormat, OrbitCamera,
    OrbitConfig, RenderError, Renderer, RgbaImage, Specular, Submesh, TextureId, TextureUpload,
    UpAxis, Wrap,
};

const WIDTH: u32 = 256;
const HEIGHT: u32 = 160;
const CLEAR: [f32; 4] = [0.03, 0.035, 0.05, 1.0];

/// `size × size` RGBA8 texture from a per-texel function of `(x, y)`.
fn texels(size: u32, f: impl Fn(u32, u32) -> [u8; 4]) -> Vec<u8> {
    (0..size * size)
        .flat_map(|i| f(i % size, i / size))
        .collect()
}

fn upload(
    r: &mut Renderer<'static>,
    size: u32,
    space: ColorSpace,
    filter: Filter,
    data: &[u8],
) -> Result<TextureId, RenderError> {
    r.upload_texture(&TextureUpload {
        color_space: space,
        filter,
        wrap: Wrap::Repeat,
        ..TextureUpload::rgba8(size, size, data)
    })
}

/// Two-tone checker with `cells × cells` squares.
fn checker(size: u32, cells: u32, a: [u8; 4], b: [u8; 4]) -> Vec<u8> {
    let cell = (size / cells).max(1);
    texels(size, |x, y| {
        if ((x / cell) + (y / cell)).is_multiple_of(2) {
            a
        } else {
            b
        }
    })
}

fn camera(eye: Vec3, target: Vec3) -> Camera {
    Camera {
        eye,
        target,
        up: Vec3::Y,
        ..Camera::default()
    }
}

fn at(x: f32, y: f32, z: f32) -> Mat4 {
    Mat4::from_translation(Vec3::new(x, y, z))
}

/// Render one frame into a fresh target and read it back.
fn render(
    r: &mut Renderer<'static>,
    build: impl FnOnce(&mut Frame),
) -> Result<RgbaImage, RenderError> {
    let target = r.create_offscreen_target(WIDTH, HEIGHT, OffscreenFormat::Rgba8Srgb)?;
    let mut frame = r.begin_offscreen_frame(&target)?;
    build(&mut frame);
    r.end_frame(frame)?;
    r.read_pixels(&target)
}

/// Render twice (must be bit-identical) and compare against the golden PNG.
fn check_golden(
    name: &str,
    scene: impl Fn(&mut Renderer<'static>) -> Result<RgbaImage, RenderError>,
) -> Result<(), RenderError> {
    let Some(mut r) = gpu::headless_renderer(CLEAR) else {
        return Ok(());
    };
    let img = scene(&mut r)?;
    let again = scene(&mut r)?;
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

/// A neutral grey floor at y = `y`.
fn floor(r: &mut Renderer<'static>) -> Result<MeshId, RenderError> {
    r.upload_mesh(&MeshData::plane(12.0, 12.0, 1, 1))
}

fn floor_draw(mesh: MeshId, y: f32) -> MeshDraw {
    MeshDraw::new(
        mesh,
        at(0.0, y, 0.0) * Mat4::from_rotation_x(-FRAC_PI_2),
        Material::color([0.32, 0.33, 0.35, 1.0]),
    )
}

// ---- opaque ------------------------------------------------------------------------------

fn opaque_scene(r: &mut Renderer<'static>) -> Result<RgbaImage, RenderError> {
    let tex = upload(
        r,
        32,
        ColorSpace::Srgb,
        Filter::Nearest,
        &checker(32, 4, [0xe0, 0xb0, 0x50, 0xff], [0x30, 0x58, 0x88, 0xff]),
    )?;
    let ground = floor(r)?;
    let cube = r.upload_mesh(&MeshData::cube(0.5))?;
    // Vertex colours: a vertical gradient on a sphere.
    let mut sphere = MeshData::uv_sphere(0.6, 24, 12);
    sphere.colors = Some(
        sphere
            .positions
            .iter()
            .map(|p| {
                let t = (p[1] / 0.6) * 0.5 + 0.5;
                [0.9, 0.25 + 0.6 * t, 0.2 + 0.3 * (1.0 - t), 1.0]
            })
            .collect(),
    );
    let sphere = r.upload_mesh(&sphere)?;
    // Two submeshes of one cube: first three faces, last three faces.
    let split = r.upload_mesh(&MeshData {
        submeshes: vec![
            Submesh {
                first_index: 0,
                index_count: 18,
            },
            Submesh {
                first_index: 18,
                index_count: 18,
            },
        ],
        ..MeshData::cube(0.4)
    })?;
    let cam = camera(Vec3::new(0.0, 1.5, 3.3), Vec3::new(0.0, 0.0, 0.0));
    render(r, |f| {
        let spin = Mat4::from_quat(Quat::from_rotation_y(0.6) * Quat::from_rotation_x(0.35));
        let p = f.world_pass("opaque", &cam);
        p.clear(CLEAR)
            .mesh(floor_draw(ground, -0.6))
            .mesh(MeshDraw::new(
                cube,
                at(-1.6, 0.0, 0.0) * spin,
                Material::textured(tex),
            ))
            .mesh(MeshDraw::new(
                sphere,
                at(0.0, 0.05, 0.0),
                Material {
                    specular: Some(Specular {
                        color: [0.9, 0.9, 0.9],
                        shininess: 48.0,
                        texture: None,
                    }),
                    ..Material::default()
                },
            ))
            .mesh(
                MeshDraw::new(
                    split,
                    at(1.6, 0.0, 0.0) * spin,
                    Material::color([0.85, 0.2, 0.2, 1.0]),
                )
                .with_submesh(0),
            )
            .mesh(
                MeshDraw::new(
                    split,
                    at(1.6, 0.0, 0.0) * spin,
                    Material::color([0.2, 0.35, 0.9, 1.0]),
                )
                .with_submesh(1),
            );
    })
}

#[test]
fn mesh_opaque_matches_golden() -> Result<(), RenderError> {
    check_golden("mesh_opaque", opaque_scene)
}

// ---- alpha test --------------------------------------------------------------------------

fn alpha_test_scene(r: &mut Renderer<'static>) -> Result<RgbaImage, RenderError> {
    // Leaf-like cut-out: a disc with three round holes, alpha 0 or 255.
    let leaf = texels(64, |x, y| {
        let (u, v) = (x as f32 / 63.0 - 0.5, y as f32 / 63.0 - 0.5);
        let inside = u * u + v * v < 0.22;
        let hole = [(-0.15, -0.12), (0.17, -0.05), (0.0, 0.2)]
            .iter()
            .any(|(cx, cy)| (u - cx) * (u - cx) + (v - cy) * (v - cy) < 0.006);
        let a = if inside && !hole { 255 } else { 0 };
        [60, 150 + (y * 90 / 63) as u8, 70, a]
    });
    let leaf = upload(r, 64, ColorSpace::Srgb, Filter::Nearest, &leaf)?;
    // Horizontal alpha ramp: alpha = u.
    let ramp = texels(64, |x, _| [230, 120, 60, (x * 255 / 63) as u8]);
    let ramp = upload(r, 64, ColorSpace::Srgb, Filter::Nearest, &ramp)?;
    let ground = floor(r)?;
    let quad = r.upload_mesh(&MeshData::plane(1.6, 1.6, 1, 1))?;
    let cam = camera(Vec3::new(0.2, 0.7, 3.0), Vec3::new(0.2, 0.1, 0.0));
    render(r, |f| {
        let p = f.world_pass("alpha-test", &cam);
        p.clear(CLEAR).mesh(floor_draw(ground, -0.8));
        // Two leaves crossing (two-sided, typical foliage card).
        for yaw in [FRAC_PI_4, -FRAC_PI_4] {
            p.mesh(MeshDraw::new(
                quad,
                at(-1.3, 0.1, 0.0) * Mat4::from_rotation_y(yaw),
                Material {
                    alpha: AlphaMode::Mask { cutoff: 0.5 },
                    two_sided: true,
                    ..Material::textured(leaf)
                },
            ));
        }
        // The same ramp with two thresholds: the cut moves with the cutoff.
        for (x, cutoff) in [(0.45, 0.25), (1.65, 0.75)] {
            p.mesh(MeshDraw::new(
                quad,
                at(x, 0.1, 0.0) * Mat4::from_scale(Vec3::new(0.7, 1.0, 1.0)),
                Material {
                    alpha: AlphaMode::Mask { cutoff },
                    ..Material::textured(ramp)
                },
            ));
        }
    })
}

#[test]
fn mesh_alpha_test_matches_golden() -> Result<(), RenderError> {
    check_golden("mesh_alpha_test", alpha_test_scene)
}

// ---- alpha blend -------------------------------------------------------------------------

/// Three translucent panes and a translucent sphere around an opaque pillar. `reverse`
/// flips the submission order; back-to-front sorting makes both renders identical.
fn blend_frame(
    f: &mut Frame,
    cam: &Camera,
    meshes: (MeshId, MeshId, MeshId, MeshId),
    reverse: bool,
) {
    let (ground, pane, sphere, pillar) = meshes;
    let mut draws = vec![
        MeshDraw::new(
            pane,
            at(-0.5, 0.2, 1.0),
            Material {
                alpha: AlphaMode::Blend,
                ..Material::color([0.95, 0.2, 0.2, 0.55])
            },
        ),
        MeshDraw::new(
            pane,
            at(0.0, 0.0, 0.0),
            Material {
                alpha: AlphaMode::Blend,
                ..Material::color([0.2, 0.9, 0.3, 0.55])
            },
        ),
        MeshDraw::new(
            pane,
            at(0.5, -0.2, -1.0),
            Material {
                alpha: AlphaMode::Blend,
                ..Material::color([0.25, 0.35, 0.95, 0.55])
            },
        ),
        MeshDraw::new(
            sphere,
            at(1.7, 0.0, 0.3),
            Material {
                alpha: AlphaMode::Blend,
                two_sided: true,
                specular: Some(Specular {
                    color: [1.0; 3],
                    shininess: 40.0,
                    texture: None,
                }),
                ..Material::color([0.9, 0.85, 0.3, 0.45])
            },
        ),
    ];
    if reverse {
        draws.reverse();
    }
    let p = f.world_pass("alpha-blend", cam);
    p.clear(CLEAR)
        .mesh(floor_draw(ground, -0.9))
        .mesh(MeshDraw::new(
            pillar,
            at(-1.7, 0.0, -0.4) * Mat4::from_scale(Vec3::new(0.35, 1.6, 0.35)),
            Material::color([0.8, 0.8, 0.75, 1.0]),
        ));
    for d in draws {
        p.mesh(d);
    }
}

fn blend_meshes(
    r: &mut Renderer<'static>,
) -> Result<(MeshId, MeshId, MeshId, MeshId), RenderError> {
    Ok((
        floor(r)?,
        r.upload_mesh(&MeshData::plane(1.4, 1.1, 1, 1))?,
        r.upload_mesh(&MeshData::uv_sphere(0.55, 24, 12))?,
        r.upload_mesh(&MeshData::cube(0.5))?,
    ))
}

fn blend_camera() -> Camera {
    camera(Vec3::new(0.8, 0.9, 3.6), Vec3::new(0.1, 0.0, 0.0))
}

fn blend_scene(r: &mut Renderer<'static>) -> Result<RgbaImage, RenderError> {
    let meshes = blend_meshes(r)?;
    let cam = blend_camera();
    render(r, |f| blend_frame(f, &cam, meshes, false))
}

#[test]
fn mesh_alpha_blend_matches_golden() -> Result<(), RenderError> {
    check_golden("mesh_alpha_blend", blend_scene)
}

#[test]
fn blended_meshes_are_sorted_back_to_front() -> Result<(), RenderError> {
    let Some(mut r) = gpu::headless_renderer(CLEAR) else {
        return Ok(());
    };
    let meshes = blend_meshes(&mut r)?;
    let cam = blend_camera();
    let a = render(&mut r, |f| blend_frame(f, &cam, meshes, false))?;
    let b = render(&mut r, |f| blend_frame(f, &cam, meshes, true))?;
    assert!(a.data == b.data, "blend result depends on submission order");
    Ok(())
}

// ---- two-sided ---------------------------------------------------------------------------

fn two_sided_scene(r: &mut Renderer<'static>) -> Result<RgbaImage, RenderError> {
    let tex = upload(
        r,
        32,
        ColorSpace::Srgb,
        Filter::Nearest,
        &checker(32, 8, [0xd8, 0xd0, 0xc0, 0xff], [0x80, 0x40, 0x30, 0xff]),
    )?;
    let ground = floor(r)?;
    let tube = r.upload_mesh(&MeshData::tube(0.6, 1.2, 24))?;
    let card = r.upload_mesh(&MeshData::plane(1.0, 1.0, 1, 1))?;
    let cam = camera(Vec3::new(0.0, 2.1, 2.7), Vec3::new(0.0, 0.0, 0.0));
    render(r, |f| {
        let p = f.world_pass("two-sided", &cam);
        p.clear(CLEAR).mesh(floor_draw(ground, -0.6));
        for (x, two_sided) in [(-1.6, false), (0.0, true)] {
            p.mesh(MeshDraw::new(
                tube,
                at(x, 0.0, 0.0),
                Material {
                    two_sided,
                    ..Material::textured(tex)
                },
            ));
        }
        // Cards seen from behind: one-sided (culled away) above two-sided (visible).
        for (y, two_sided) in [(0.6, false), (-0.1, true)] {
            p.mesh(MeshDraw::new(
                card,
                at(1.6, y, 0.0) * Mat4::from_rotation_y(std::f32::consts::PI - 0.3),
                Material {
                    two_sided,
                    ..Material::color([0.3, 0.75, 0.95, 1.0])
                },
            ));
        }
    })
}

#[test]
fn mesh_two_sided_matches_golden() -> Result<(), RenderError> {
    check_golden("mesh_two_sided", two_sided_scene)
}

// ---- dye ---------------------------------------------------------------------------------

fn dye_scene(r: &mut Renderer<'static>) -> Result<RgbaImage, RenderError> {
    // Base: light grey with darker seams. Mask (linear, red channel): diagonal stripes.
    let base = texels(64, |x, y| {
        let seam = x % 16 == 0 || y % 16 == 0;
        let v = if seam { 110 } else { 225 };
        [v, v, v, 255]
    });
    let base = upload(r, 64, ColorSpace::Srgb, Filter::Nearest, &base)?;
    let mask = texels(64, |x, y| {
        let m = if ((x + y) / 8).is_multiple_of(2) {
            255
        } else {
            0
        };
        [m, 0, 0, 255]
    });
    let mask = upload(r, 64, ColorSpace::Linear, Filter::Nearest, &mask)?;
    // Base with the dye mask in its alpha: the top half dyeable.
    let base_alpha = texels(64, |_, y| {
        let a = if y < 32 { 255 } else { 0 };
        [200, 200, 200, a]
    });
    let base_alpha = upload(r, 64, ColorSpace::Srgb, Filter::Nearest, &base_alpha)?;
    let ground = floor(r)?;
    let cube = r.upload_mesh(&MeshData::cube(0.45))?;
    let cam = camera(Vec3::new(0.0, 1.4, 3.6), Vec3::new(0.0, 0.0, 0.0));
    render(r, |f| {
        let spin = Mat4::from_rotation_y(0.55) * Mat4::from_rotation_x(0.3);
        let p = f.world_pass("dye", &cam);
        p.clear(CLEAR).mesh(floor_draw(ground, -0.7));
        for (x, color) in [
            (-2.1, [0.85, 0.12, 0.1]),
            (-0.7, [0.1, 0.7, 0.2]),
            (0.7, [0.15, 0.3, 0.95]),
        ] {
            p.mesh(MeshDraw::new(
                cube,
                at(x, 0.0, 0.0) * spin,
                Material {
                    dye: Some(Dye {
                        color,
                        mask: DyeMask::Texture(mask),
                    }),
                    ..Material::textured(base)
                },
            ));
        }
        p.mesh(MeshDraw::new(
            cube,
            at(2.1, 0.0, 0.0) * spin,
            Material {
                dye: Some(Dye {
                    color: [0.95, 0.75, 0.1],
                    mask: DyeMask::BaseAlpha,
                }),
                ..Material::textured(base_alpha)
            },
        ));
    })
}

#[test]
fn mesh_dye_matches_golden() -> Result<(), RenderError> {
    check_golden("mesh_dye", dye_scene)
}

// ---- normal map --------------------------------------------------------------------------

/// Tangent-space normal map of a grid of round bumps (green along +v, the default).
fn bump_normal_map(size: u32, cells: u32) -> Vec<u8> {
    let cell = size as f32 / cells as f32;
    texels(size, |x, y| {
        let fx = (x as f32 + 0.5) % cell / cell - 0.5;
        let fy = (y as f32 + 0.5) % cell / cell - 0.5;
        // Height h = cos-shaped dome of radius 0.4; slope from its derivative.
        let d = (fx * fx + fy * fy).sqrt();
        let (dx, dy) = if d < 0.4 && d > 1e-6 {
            let s = -(d / 0.4 * std::f32::consts::PI).sin() * 1.6;
            (s * fx / d, s * fy / d)
        } else {
            (0.0, 0.0)
        };
        let n = Vec3::new(-dx, -dy, 1.0).normalize();
        let enc = |c: f32| ((c * 0.5 + 0.5) * 255.0).round() as u8;
        [enc(n.x), enc(n.y), enc(n.z), 255]
    })
}

fn normal_map_scene(r: &mut Renderer<'static>) -> Result<RgbaImage, RenderError> {
    let normals = upload(
        r,
        64,
        ColorSpace::Linear,
        Filter::Nearest,
        &bump_normal_map(64, 4),
    )?;
    let plane = r.upload_mesh(&MeshData::plane(1.6, 1.6, 1, 1))?;
    let sphere = r.upload_mesh(&MeshData::uv_sphere(0.7, 32, 16))?;
    // No tangents: the normal map must be ignored.
    let mut flat = MeshData::plane(1.6, 1.6, 1, 1);
    flat.tangents = None;
    let flat = r.upload_mesh(&flat)?;
    let cam = camera(Vec3::new(0.0, 0.3, 3.4), Vec3::new(0.0, 0.0, 0.0));
    let bumpy = |specular| Material {
        normal_texture: Some(normals),
        specular,
        ..Material::color([0.75, 0.72, 0.68, 1.0])
    };
    render(r, |f| {
        let p = f.world_pass("normal-map", &cam);
        p.clear(CLEAR).lighting(Lighting {
            // Grazing light from the upper left.
            direction: Vec3::new(-0.8, 0.5, 0.35),
            ..Lighting::default()
        });
        p.mesh(MeshDraw::new(flat, at(-1.8, 0.0, 0.0), bumpy(None)))
            .mesh(MeshDraw::new(plane, at(0.0, 0.0, 0.0), bumpy(None)))
            .mesh(MeshDraw::new(
                sphere,
                at(1.8, 0.0, 0.0),
                bumpy(Some(Specular {
                    color: [0.6; 3],
                    shininess: 32.0,
                    texture: None,
                })),
            ));
    })
}

#[test]
fn mesh_normal_map_matches_golden() -> Result<(), RenderError> {
    check_golden("mesh_normal_map", normal_map_scene)
}

// ---- debug views -------------------------------------------------------------------------

fn debug_scene(r: &mut Renderer<'static>) -> Result<RgbaImage, RenderError> {
    let sphere = r.upload_mesh(&MeshData::uv_sphere(0.6, 24, 12))?;
    let coarse = r.upload_mesh(&MeshData::uv_sphere(0.6, 12, 6))?;
    let cube = r.upload_mesh(&MeshData::cube(0.45))?;
    let info = r.mesh_info(sphere).cloned();
    let mut orbit = OrbitCamera::new(OrbitConfig {
        // The procedural shapes are Y-up.
        up: UpAxis::Y,
        pitch: 0.35,
        ..OrbitConfig::default()
    });
    orbit.yaw = 0.25;
    if let Some(info) = info {
        // Frame a sphere enclosing the four objects.
        let mut s = info.sphere;
        s.radius = 1.6;
        orbit.frame_sphere(s, WIDTH as f32 / HEIGHT as f32);
    }
    let cam = orbit.camera();
    let spin = Mat4::from_rotation_y(0.5) * Mat4::from_rotation_x(0.35);
    render(r, |f| {
        f.world_pass("wireframe", &cam)
            .clear(CLEAR)
            .mesh_debug(MeshDebug {
                shading: MeshShading::Hidden,
                wireframe: Some([0.9, 0.9, 0.5, 1.0]),
                normals: None,
            })
            .mesh(MeshDraw::new(
                coarse,
                at(-1.8, 0.0, 0.0),
                Material::default(),
            ));
        f.world_pass("normals", &cam)
            .mesh_debug(MeshDebug {
                shading: MeshShading::Normals,
                ..MeshDebug::default()
            })
            .mesh(MeshDraw::new(
                cube,
                at(-0.6, 0.0, 0.0) * spin,
                Material::default(),
            ));
        f.world_pass("uv-checker", &cam)
            .mesh_debug(MeshDebug {
                shading: MeshShading::UvChecker,
                ..MeshDebug::default()
            })
            .mesh(MeshDraw::new(
                sphere,
                at(0.6, 0.0, 0.0),
                Material::default(),
            ));
        f.world_pass("lit+overlays", &cam)
            .mesh_debug(MeshDebug {
                shading: MeshShading::Lit,
                wireframe: Some([0.05, 0.05, 0.05, 1.0]),
                normals: Some(NormalLines {
                    length: 0.25,
                    color: [1.0, 0.3, 0.8, 1.0],
                }),
            })
            .mesh(MeshDraw::new(
                cube,
                at(1.8, 0.0, 0.0) * spin,
                Material::color([0.4, 0.7, 0.9, 1.0]),
            ));
    })
}

#[test]
fn mesh_debug_matches_golden() -> Result<(), RenderError> {
    check_golden("mesh_debug", debug_scene)
}
