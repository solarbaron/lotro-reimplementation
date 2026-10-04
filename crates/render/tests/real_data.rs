// SPDX-License-Identifier: GPL-3.0-or-later
//! Real-data check of the mesh pipelines: render surfaces decoded from the user's install
//! are used as base, normal, dye-mask and specular textures of procedural meshes, drawn
//! through every material pipeline and debug view. Skipped unless `TESSERA_GAME_DIR` is
//! set; the install is opened read-only and nothing derived from it is written anywhere.
//! Only counts are printed (never pixels or names).

#[path = "support/gpu.rs"]
mod gpu;

use tessera_dat::{ArchiveSet, SetOptions};
use tessera_formats::{Record, Surface};
use tessera_render::glam::{Mat4, Vec3};
use tessera_render::{
    AlphaMode, Camera, ColorSpace, Dye, DyeMask, Material, MeshData, MeshDebug, MeshDraw,
    MeshShading, NormalLines, OffscreenFormat, RenderError, Specular, TextureId, TextureUpload,
};

const SURFACES: std::ops::Range<u32> = 0x4100_0000..0x4200_0000;
/// Distinct surfaces to use.
const WANT: usize = 24;
/// Directory entries examined at most.
const MAX_ATTEMPTS: usize = 4000;
/// Skip surfaces larger than this many texels (keeps the test quick).
const MAX_TEXELS: u64 = 1024 * 1024;

#[test]
fn real_surfaces_through_every_mesh_pipeline() -> Result<(), RenderError> {
    let Some(dir) = std::env::var_os("TESSERA_GAME_DIR") else {
        eprintln!("skipping: TESSERA_GAME_DIR is not set");
        return Ok(());
    };
    let Some(mut r) = gpu::headless_renderer([0.0, 0.0, 0.0, 1.0]) else {
        return Ok(());
    };
    let set = ArchiveSet::open(&dir, SetOptions::default()).expect("open archive set");

    // Collect decodable surfaces (routed archives first).
    let mut textures: Vec<(TextureId, TextureId)> = Vec::new();
    let (mut attempts, mut decode_errors, mut too_big) = (0usize, 0usize, 0usize);
    let routed = set.candidates(SURFACES.start);
    let order: Vec<usize> = routed
        .iter()
        .copied()
        .chain((0..set.members().len()).filter(|i| !routed.contains(i)))
        .collect();
    'outer: for i in order {
        let Some(m) = set.get(i) else { continue };
        for entry in m.archive.entries_range(SURFACES) {
            let Ok(entry) = entry else { break };
            attempts += 1;
            if attempts > MAX_ATTEMPTS || textures.len() >= WANT {
                break 'outer;
            }
            let Ok(bytes) = m.archive.read_entry(&entry) else {
                decode_errors += 1;
                continue;
            };
            let image = Surface::parse_expecting(&bytes, entry.did).and_then(|s| s.to_rgba8());
            let Ok(image) = image else {
                decode_errors += 1;
                continue;
            };
            if u64::from(image.width) * u64::from(image.height) > MAX_TEXELS {
                too_big += 1;
                continue;
            }
            // The same texels as colour data and as linear data (normal / mask use).
            let srgb = r.upload_texture(&TextureUpload::rgba8(
                image.width,
                image.height,
                &image.pixels,
            ))?;
            let linear = r.upload_texture(&TextureUpload {
                color_space: ColorSpace::Linear,
                ..TextureUpload::rgba8(image.width, image.height, &image.pixels)
            })?;
            textures.push((srgb, linear));
        }
    }
    eprintln!(
        "surfaces: {} used, {attempts} entries examined, {decode_errors} not decodable, \
         {too_big} over the size cap",
        textures.len()
    );
    assert!(!textures.is_empty(), "no decodable surface found");

    let meshes = [
        r.upload_mesh(&MeshData::cube(0.4))?,
        r.upload_mesh(&MeshData::uv_sphere(0.45, 24, 12))?,
        r.upload_mesh(&MeshData::tube(0.35, 0.8, 16))?,
        r.upload_mesh(&MeshData::plane(0.9, 0.9, 2, 2))?,
    ];
    let materials = |base: TextureId, lin: TextureId| -> [Material; 7] {
        let textured = Material::textured(base);
        [
            textured,
            Material {
                alpha: AlphaMode::Mask { cutoff: 0.5 },
                two_sided: true,
                ..textured
            },
            Material {
                alpha: AlphaMode::Blend,
                ..textured
            },
            Material {
                dye: Some(Dye {
                    color: [0.8, 0.2, 0.1],
                    mask: DyeMask::Texture(lin),
                }),
                ..textured
            },
            Material {
                dye: Some(Dye {
                    color: [0.1, 0.4, 0.9],
                    mask: DyeMask::BaseAlpha,
                }),
                ..textured
            },
            Material {
                normal_texture: Some(lin),
                ..textured
            },
            Material {
                specular: Some(Specular {
                    color: [1.0; 3],
                    shininess: 24.0,
                    texture: Some(lin),
                }),
                ..textured
            },
        ]
    };

    let target = r.create_offscreen_target(256, 256, OffscreenFormat::Rgba8Srgb)?;
    let cam = Camera {
        eye: Vec3::new(0.0, 3.0, 9.0),
        target: Vec3::ZERO,
        up: Vec3::Y,
        ..Camera::default()
    };
    let mut drawn = 0u32;
    for shading in [
        MeshShading::Lit,
        MeshShading::BaseColor,
        MeshShading::Normals,
        MeshShading::UvChecker,
    ] {
        let mut frame = r.begin_offscreen_frame(&target)?;
        let p = frame.world_pass("real", &cam);
        p.clear([0.0, 0.0, 0.0, 1.0]).mesh_debug(MeshDebug {
            shading,
            wireframe: (shading == MeshShading::Lit).then_some([0.0, 0.0, 0.0, 1.0]),
            normals: (shading == MeshShading::Normals).then_some(NormalLines {
                length: 0.1,
                color: [1.0; 4],
            }),
        });
        for (i, &(srgb, lin)) in textures.iter().enumerate() {
            for (k, material) in materials(srgb, lin).into_iter().enumerate() {
                let x = (k as f32 - 3.0) * 1.1;
                let y = (i % 6) as f32 * 1.1 - 2.5;
                p.mesh(MeshDraw::new(
                    meshes[(i + k) % meshes.len()],
                    Mat4::from_translation(Vec3::new(x, y, -((i / 6) as f32)))
                        * Mat4::from_rotation_y(0.3 * k as f32),
                    material,
                ));
            }
        }
        let stats = r.end_frame(frame)?;
        drawn += stats.mesh_draws;
        let img = r.read_pixels(&target)?;
        let lit = img
            .data
            .chunks_exact(4)
            .filter(|p| p[0] > 8 || p[1] > 8 || p[2] > 8)
            .count();
        assert!(lit > 1000, "{shading:?}: only {lit} non-background pixels");
    }
    eprintln!(
        "mesh draws: {drawn} over 4 shading modes, arena {:?}",
        r.mesh_stats()
    );
    Ok(())
}
