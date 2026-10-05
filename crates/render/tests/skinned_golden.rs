// SPDX-License-Identifier: GPL-3.0-or-later
//! Golden images of GPU skinning: a bent cylinder at two palettes, rigid versus blended
//! (two- and four-slot) weights, an alpha-tested skinned card, dye on a skinned mesh and
//! the debug overlays following the skinned surface.
//!
//! Meshes, skins, palettes and textures are generated here (no game data). Regenerate with
//! `TESSERA_BLESS=1 cargo test -p tessera-render --test skinned_golden` and review the PNGs
//! in `tests/golden/` (see `tests/golden/README.md`); comparison as in `mesh_golden.rs`.

#[path = "support/golden.rs"]
mod golden;
#[path = "support/gpu.rs"]
mod gpu;
#[path = "support/skinning.rs"]
mod skinning;

use std::f32::consts::FRAC_PI_2;
use std::path::Path;

use tessera_render::glam::{Mat4, Vec3};
use tessera_render::{
    AlphaMode, Camera, ColorSpace, Dye, DyeMask, Filter, Frame, Lighting, Material, MeshData,
    MeshDebug, MeshDraw, MeshId, MeshShading, MeshSkin, NormalLines, OffscreenFormat, RenderError,
    Renderer, RgbaImage, Specular, TextureId, TextureUpload, Wrap,
};

const WIDTH: u32 = 256;
const HEIGHT: u32 = 160;
const CLEAR: [f32; 4] = [0.03, 0.035, 0.05, 1.0];

fn texels(size: u32, f: impl Fn(u32, u32) -> [u8; 4]) -> Vec<u8> {
    (0..size * size)
        .flat_map(|i| f(i % size, i / size))
        .collect()
}

fn upload(
    r: &mut Renderer<'static>,
    size: u32,
    space: ColorSpace,
    data: &[u8],
) -> Result<TextureId, RenderError> {
    r.upload_texture(&TextureUpload {
        color_space: space,
        filter: Filter::Nearest,
        wrap: Wrap::Repeat,
        ..TextureUpload::rgba8(size, size, data)
    })
}

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

fn render(
    r: &mut Renderer<'static>,
    build: impl FnOnce(&mut Frame) -> Result<(), RenderError>,
) -> Result<RgbaImage, RenderError> {
    let target = r.create_offscreen_target(WIDTH, HEIGHT, OffscreenFormat::Rgba8Srgb)?;
    let mut frame = r.begin_offscreen_frame(&target)?;
    build(&mut frame)?;
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

/// A 1.4-high cylinder on the floor, jointed at mid height.
const LIMB_HEIGHT: f32 = 1.4;
const JOINT: f32 = 0.7;

fn limb() -> MeshData {
    skinning::cylinder(0.22, LIMB_HEIGHT, 24, 14)
}

// ---- bent cylinder at two palettes ------------------------------------------------------

fn bend_scene(r: &mut Renderer<'static>) -> Result<RgbaImage, RenderError> {
    let tex = upload(
        r,
        32,
        ColorSpace::Srgb,
        &checker(32, 4, [0xe0, 0xb0, 0x50, 0xff], [0x30, 0x58, 0x88, 0xff]),
    )?;
    let ground = floor(r)?;
    let mesh = limb();
    let skin = skinning::two_bone_skin(&mesh, JOINT, 0.18, 2);
    let id = r.upload_skinned_mesh(&mesh, &skin)?;
    let cam = camera(Vec3::new(0.0, 1.1, 3.0), Vec3::new(0.0, 0.65, 0.0));
    render(r, |f| {
        // One mesh, three palettes in the same frame: straight, 40° and 85°.
        let palettes = [
            f.skin_palette(&skinning::two_bone_palette(JOINT, 0.0))?,
            f.skin_palette(&skinning::two_bone_palette(JOINT, 0.7))?,
            f.skin_palette(&skinning::two_bone_palette(JOINT, -1.48))?,
        ];
        let p = f.world_pass("bend", &cam);
        p.clear(CLEAR).mesh(floor_draw(ground, 0.0));
        for (x, palette) in [-1.5, 0.0, 1.3].into_iter().zip(palettes) {
            p.mesh(
                MeshDraw::new(
                    id,
                    at(x, 0.0, 0.0) * Mat4::from_rotation_y(0.3),
                    Material {
                        specular: Some(Specular {
                            color: [0.6; 3],
                            shininess: 32.0,
                            texture: None,
                        }),
                        ..Material::textured(tex)
                    },
                )
                .with_skin(palette),
            );
        }
        Ok(())
    })
}

#[test]
fn skinned_bend_matches_golden() -> Result<(), RenderError> {
    check_golden("skinned_bend", bend_scene)
}

// ---- weights: rigid, two-bone blend, four slots ------------------------------------------

fn weights_scene(r: &mut Renderer<'static>) -> Result<RgbaImage, RenderError> {
    let tex = upload(
        r,
        32,
        ColorSpace::Srgb,
        &checker(32, 8, [0xd8, 0xd8, 0xd0, 0xff], [0x70, 0x30, 0x30, 0xff]),
    )?;
    let ground = floor(r)?;
    let mesh = limb();
    let rigid = r.upload_skinned_mesh(&mesh, &skinning::two_bone_skin(&mesh, JOINT, 0.0, 2))?;
    let smooth = r.upload_skinned_mesh(&mesh, &skinning::two_bone_skin(&mesh, JOINT, 0.3, 2))?;
    let (j1, j2) = (0.45, 0.95);
    let chain = r.upload_skinned_mesh(&mesh, &skinning::three_bone_skin(&mesh, j1, j2, 0.3, 3))?;
    let cam = camera(Vec3::new(0.0, 0.9, 3.0), Vec3::new(0.0, 0.7, 0.0));
    render(r, |f| {
        let two = f.skin_palette(&skinning::two_bone_palette(JOINT, 1.2))?;
        let three = f.skin_palette(&skinning::three_bone_palette(j1, j2, 0.55))?;
        let p = f.world_pass("weights", &cam);
        p.clear(CLEAR).mesh(floor_draw(ground, 0.0));
        let material = Material::textured(tex);
        p.mesh(MeshDraw::new(rigid, at(-1.6, 0.0, 0.0), material).with_skin(two))
            .mesh(MeshDraw::new(smooth, at(-0.2, 0.0, 0.0), material).with_skin(two))
            .mesh(MeshDraw::new(chain, at(1.1, 0.0, 0.0), material).with_skin(three));
        Ok(())
    })
}

#[test]
fn skinned_weights_matches_golden() -> Result<(), RenderError> {
    check_golden("skinned_weights", weights_scene)
}

// ---- alpha-tested skinned card -----------------------------------------------------------

fn alpha_test_scene(r: &mut Renderer<'static>) -> Result<RgbaImage, RenderError> {
    // Cut-out: a disc pattern of holes, alpha 0 or 255.
    let leaf = texels(64, |x, y| {
        let (u, v) = (x as f32 / 63.0 - 0.5, y as f32 / 63.0 - 0.5);
        let inside = u.abs() < 0.46 && v.abs() < 0.48;
        let cu = (u * 4.0).rem_euclid(1.0) - 0.5;
        let cv = (v * 6.0).rem_euclid(1.0) - 0.5;
        let hole = cu * cu + cv * cv < 0.06;
        let a = if inside && !hole { 255 } else { 0 };
        [70, 140 + (y * 100 / 63) as u8, 80, a]
    });
    let leaf = upload(r, 64, ColorSpace::Srgb, &leaf)?;
    let ground = floor(r)?;
    // A card standing on the floor (y = 0 .. 1.4), folded forward about X at mid height.
    let mut card = MeshData::plane(1.2, LIMB_HEIGHT, 6, 14);
    for p in &mut card.positions {
        p[1] += LIMB_HEIGHT * 0.5;
    }
    let skin = skinning::two_bone_skin(&card, JOINT, 0.2, 2);
    let card = r.upload_skinned_mesh(&card, &skin)?;
    let cam = camera(Vec3::new(1.8, 1.3, 2.4), Vec3::new(0.0, 0.6, 0.0));
    let fold = |angle: f32| {
        let pivot = Vec3::new(0.0, JOINT, 0.0);
        vec![
            Mat4::IDENTITY,
            Mat4::from_translation(pivot)
                * Mat4::from_rotation_x(angle)
                * Mat4::from_translation(-pivot),
        ]
    };
    render(r, |f| {
        let forward = f.skin_palette(&fold(0.9))?;
        let back = f.skin_palette(&fold(-0.6))?;
        let p = f.world_pass("alpha-test", &cam);
        p.clear(CLEAR).mesh(floor_draw(ground, 0.0));
        let material = Material {
            alpha: AlphaMode::Mask { cutoff: 0.5 },
            two_sided: true,
            ..Material::textured(leaf)
        };
        p.mesh(MeshDraw::new(card, at(-0.9, 0.0, 0.0), material).with_skin(forward))
            .mesh(MeshDraw::new(card, at(0.9, 0.0, -0.4), material).with_skin(back));
        Ok(())
    })
}

#[test]
fn skinned_alpha_test_matches_golden() -> Result<(), RenderError> {
    check_golden("skinned_alpha_test", alpha_test_scene)
}

// ---- dye on a skinned mesh ---------------------------------------------------------------

fn dye_scene(r: &mut Renderer<'static>) -> Result<RgbaImage, RenderError> {
    let base = texels(64, |x, y| {
        let seam = x % 16 == 0 || y % 16 == 0;
        let v = if seam { 110 } else { 225 };
        [v, v, v, 255]
    });
    let base = upload(r, 64, ColorSpace::Srgb, &base)?;
    // Mask (linear, red): horizontal bands around the limb.
    let mask = texels(64, |_, y| {
        let m = if (y / 8).is_multiple_of(2) { 255 } else { 0 };
        [m, 0, 0, 255]
    });
    let mask = upload(r, 64, ColorSpace::Linear, &mask)?;
    // Mask in the base alpha: the upper half dyeable.
    let base_alpha = texels(64, |x, y| {
        let a = if y < 32 { 255 } else { 0 };
        let v = if x % 16 == 0 { 120 } else { 210 };
        [v, v, v, a]
    });
    let base_alpha = upload(r, 64, ColorSpace::Srgb, &base_alpha)?;
    let ground = floor(r)?;
    let mesh = limb();
    let id = r.upload_skinned_mesh(&mesh, &skinning::two_bone_skin(&mesh, JOINT, 0.2, 2))?;
    let cam = camera(Vec3::new(0.0, 1.1, 3.2), Vec3::new(0.0, 0.65, 0.0));
    render(r, |f| {
        let p1 = f.skin_palette(&skinning::two_bone_palette(JOINT, 0.8))?;
        let p2 = f.skin_palette(&skinning::two_bone_palette(JOINT, -0.8))?;
        let p = f.world_pass("dye", &cam);
        p.clear(CLEAR).mesh(floor_draw(ground, 0.0));
        for (x, color, palette) in [
            (-1.8, [0.85, 0.12, 0.1], p1),
            (-0.6, [0.1, 0.7, 0.2], p2),
            (0.6, [0.15, 0.3, 0.95], p1),
        ] {
            p.mesh(
                MeshDraw::new(
                    id,
                    at(x, 0.0, 0.0),
                    Material {
                        dye: Some(Dye {
                            color,
                            mask: DyeMask::Texture(mask),
                        }),
                        ..Material::textured(base)
                    },
                )
                .with_skin(palette),
            );
        }
        p.mesh(
            MeshDraw::new(
                id,
                at(1.8, 0.0, 0.0),
                Material {
                    dye: Some(Dye {
                        color: [0.95, 0.75, 0.1],
                        mask: DyeMask::BaseAlpha,
                    }),
                    specular: Some(Specular {
                        color: [0.8; 3],
                        shininess: 40.0,
                        texture: None,
                    }),
                    ..Material::textured(base_alpha)
                },
            )
            .with_skin(p2),
        );
        Ok(())
    })
}

#[test]
fn skinned_dye_matches_golden() -> Result<(), RenderError> {
    check_golden("skinned_dye", dye_scene)
}

// ---- debug views -------------------------------------------------------------------------

fn debug_frame(
    f: &mut Frame,
    id: MeshId,
    cam: &Camera,
    debug: MeshDebug,
    x: f32,
) -> Result<(), RenderError> {
    let palette = f.skin_palette(&skinning::two_bone_palette(JOINT, 1.0))?;
    f.world_pass("debug", cam)
        .lighting(Lighting::default())
        .mesh_debug(debug)
        .mesh(MeshDraw::new(id, at(x, 0.0, 0.0), Material::default()).with_skin(palette));
    Ok(())
}

fn debug_scene(r: &mut Renderer<'static>) -> Result<RgbaImage, RenderError> {
    let mesh = skinning::cylinder(0.22, LIMB_HEIGHT, 12, 7);
    let id = r.upload_skinned_mesh(&mesh, &skinning::two_bone_skin(&mesh, JOINT, 0.2, 2))?;
    let cam = camera(Vec3::new(0.0, 0.9, 3.0), Vec3::new(0.0, 0.65, 0.0));
    render(r, |f| {
        f.world_pass("clear", &cam).clear(CLEAR);
        debug_frame(
            f,
            id,
            &cam,
            MeshDebug {
                shading: MeshShading::Hidden,
                wireframe: Some([0.3, 1.0, 0.5, 1.0]),
                normals: None,
            },
            -1.5,
        )?;
        debug_frame(
            f,
            id,
            &cam,
            MeshDebug {
                shading: MeshShading::Normals,
                ..MeshDebug::default()
            },
            -0.2,
        )?;
        debug_frame(
            f,
            id,
            &cam,
            MeshDebug {
                shading: MeshShading::UvChecker,
                wireframe: None,
                normals: Some(NormalLines {
                    length: 0.18,
                    color: [1.0, 0.85, 0.2, 1.0],
                }),
            },
            1.1,
        )
    })
}

#[test]
fn skinned_debug_matches_golden() -> Result<(), RenderError> {
    check_golden("skinned_debug", debug_scene)
}

/// The rigid skin of the weights scene binds whole rings: a hard crease at the joint.
#[test]
fn rigid_skin_has_one_influence() {
    let mesh = limb();
    let skin: MeshSkin = skinning::two_bone_skin(&mesh, JOINT, 0.0, 2);
    let info = skin.validate(mesh.positions.len()).expect("valid skin");
    assert_eq!((info.max_influences, info.max_joint), (1, 1));
    let chain = skinning::three_bone_skin(&mesh, 0.45, 0.95, 0.3, 3);
    let info = chain.validate(mesh.positions.len()).expect("valid chain");
    assert_eq!((info.max_influences, info.max_joint), (4, 3));
}
