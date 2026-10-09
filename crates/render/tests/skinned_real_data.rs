// SPDX-License-Identifier: GPL-3.0-or-later
//! Real-data check of the skinned path: the bone streams of every skinned mesh record
//! (level 0, flattened by `tessera-mesh` with four influences per vertex) must pass
//! `MeshSkin::validate` with a full palette, and a sample of them drawn with an identity
//! palette must equal the static path pixel for pixel. Skipped unless `TESSERA_GAME_DIR`
//! is set; the install is opened read-only and only counts are printed (never geometry or
//! pixels). Run with `--release -- --nocapture` (`TESSERA_MESH_LIMIT` caps the records).

#[path = "support/gpu.rs"]
mod gpu;

use std::collections::BTreeMap;

use tessera_dat::{ArchiveSet, SetOptions};
use tessera_mesh::{DecodeOptions, GeometryLimits, Indices as SrcIndices, Mesh, mesh_dids};
use tessera_render::glam::Mat4;
use tessera_render::{
    Camera, Indices, MAX_PALETTE_BONES, Material, MeshData, MeshDraw, MeshSkin, OffscreenFormat,
    OrbitCamera, RenderError, Submesh,
};

/// Meshes drawn on the GPU (bind pose versus static).
const GPU_SAMPLE: usize = 24;

fn to_render(geo: &tessera_mesh::LevelGeometry) -> Option<(MeshData, MeshSkin)> {
    let joints = geo.joints.clone()?;
    let weights = geo.weights.clone()?;
    let mut m = MeshData::new(geo.positions.clone(), Vec::new());
    m.indices = match &geo.indices {
        SrcIndices::U16(v) => Indices::U16(v.clone()),
        SrcIndices::U32(v) => Indices::U32(v.clone()),
    };
    m.normals = Some(geo.normals.clone());
    m.tangents = geo.tangents.clone();
    m.binormals = geo.binormals.clone();
    m.uvs = geo.uv_sets.first().cloned();
    m.submeshes = geo
        .parts
        .iter()
        .map(|p| Submesh {
            first_index: p.first_index,
            index_count: p.index_count,
        })
        .collect();
    let palette_size = u16::try_from(MAX_PALETTE_BONES).unwrap_or(u16::MAX);
    Some((
        m,
        MeshSkin {
            joints,
            weights,
            palette_size,
        },
    ))
}

#[test]
fn real_skin_streams_validate_and_bind_pose_matches_static() -> Result<(), RenderError> {
    let Some(dir) = std::env::var_os("TESSERA_GAME_DIR") else {
        eprintln!("skipping: TESSERA_GAME_DIR is not set");
        return Ok(());
    };
    let limit: usize = std::env::var("TESSERA_MESH_LIMIT")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(usize::MAX);
    let set = ArchiveSet::open(&dir, SetOptions::default()).expect("open archive set");
    let dids = mesh_dids(&set).expect("list mesh DIDs");
    let mut renderer = gpu::headless_renderer([0.0, 0.0, 0.0, 1.0]);

    let (mut records, mut skinned, mut mixed, mut accepted, mut drawn) = (0, 0, 0, 0, 0);
    let mut vertices = 0u64;
    let mut rejected: BTreeMap<String, (u64, Vec<u32>)> = BTreeMap::new();
    let mut influences = [0u64; 5];
    let mut max_joint = 0u8;
    for did in dids.iter().take(limit) {
        records += 1;
        let Ok(bytes) = set.read(did.0) else { continue };
        let Ok(mesh) = Mesh::decode(&bytes, None, &DecodeOptions::render_only()) else {
            continue;
        };
        let Ok(level) = mesh.skinned_level(0, &GeometryLimits::default(), None) else {
            continue;
        };
        if !level.is_skinned() {
            continue;
        }
        skinned += 1;
        if level.parts.iter().any(|p| !p.skinned) {
            // Unskinned parts carry weight 0 (`tessera-mesh`); draw them as static meshes.
            mixed += 1;
            continue;
        }
        let Some((data, skin)) = to_render(&level.geometry) else {
            continue;
        };
        match skin.validate(data.positions.len()) {
            Ok(info) => {
                accepted += 1;
                vertices += data.positions.len() as u64;
                influences[usize::from(info.max_influences)] += 1;
                max_joint = max_joint.max(info.max_joint);
            }
            Err(e) => {
                // Group by the message without the vertex number and values.
                let msg = e.to_string();
                let key = msg
                    .split(':')
                    .nth(2)
                    .map(|s| s.split_whitespace().take(3).collect::<Vec<_>>().join(" "))
                    .unwrap_or(msg);
                let entry = rejected.entry(key).or_default();
                entry.0 += 1;
                if entry.1.len() < 5 {
                    entry.1.push(did.0);
                }
                continue;
            }
        }
        // GPU: the identity palette reproduces the static path exactly.
        if drawn < GPU_SAMPLE
            && let Some(r) = renderer.as_mut()
        {
            let still = r.upload_mesh(&data)?;
            let posed = r.upload_skinned_mesh(&data, &skin)?;
            let sphere = r.mesh_info(still).map(|i| i.sphere).expect("live mesh");
            let mut orbit = OrbitCamera::default();
            orbit.frame_sphere(sphere, 1.0);
            let cam: Camera = orbit.camera();
            let target = r.create_offscreen_target(96, 96, OffscreenFormat::Rgba8Unorm)?;
            let mut images = Vec::new();
            for (mesh_id, skinned_draw) in [(still, false), (posed, true)] {
                let mut frame = r.begin_offscreen_frame(&target)?;
                let palette = frame.skin_palette(&[Mat4::IDENTITY; MAX_PALETTE_BONES])?;
                let draw = MeshDraw::new(mesh_id, Mat4::IDENTITY, Material::default());
                let draw = if skinned_draw {
                    draw.with_skin(palette)
                } else {
                    draw
                };
                frame
                    .world_pass("real", &cam)
                    .clear([0.0, 0.0, 0.0, 1.0])
                    .mesh(draw);
                let stats = r.end_frame(frame)?;
                assert_eq!(stats.skinned_draws > 0, skinned_draw);
                images.push(r.read_pixels(&target)?);
            }
            assert!(
                images[0].data == images[1].data,
                "mesh {:#010x}: identity palette differs from the static path",
                did.0
            );
            r.destroy_mesh(still);
            r.destroy_mesh(posed);
            drawn += 1;
        }
    }
    eprintln!(
        "skinned real data: {records} records, {skinned} skinned level-0 meshes ({mixed} with \
         unskinned parts skipped), {accepted} accepted ({vertices} vertices), rejected {:?}; \
         meshes by max influences {influences:?}; largest bone index {max_joint}; {drawn} \
         drawn bind-pose == static",
        rejected
    );
    assert!(
        rejected.is_empty(),
        "real skin streams rejected: {rejected:?}"
    );
    Ok(())
}
