// SPDX-License-Identifier: GPL-3.0-or-later
//! Synthetic skinned meshes and palettes shared by the skinning tests, golden images and
//! the skinning benchmark. Everything is generated here; nothing comes from game data.
//!
//! Included with `#[path]` by more than one target, so unused items are allowed.
#![allow(dead_code)]

use std::f32::consts::TAU;

use tessera_render::glam::{Mat4, Vec3};
use tessera_render::{MeshData, MeshSkin};

/// A closed-side cylinder (no caps) along +Y from `y = 0` to `y = height`, with `rings`
/// rows of quads and `segments` columns, outward normals, UVs (`u` around from +Z towards
/// +X, `v` from the top, 0, to the bottom, 1) and tangents.
#[must_use]
pub fn cylinder(radius: f32, height: f32, segments: u32, rings: u32) -> MeshData {
    let (segments, rings) = (segments.max(3), rings.max(1));
    let mut positions = Vec::new();
    let mut normals = Vec::new();
    let mut uvs = Vec::new();
    for r in 0..=rings {
        let v = r as f32 / rings as f32;
        let y = (1.0 - v) * height;
        for s in 0..=segments {
            let u = s as f32 / segments as f32;
            let (sin, cos) = (u * TAU).sin_cos();
            positions.push([sin * radius, y, cos * radius]);
            normals.push([sin, 0.0, cos]);
            uvs.push([u, v]);
        }
    }
    let row = segments + 1;
    let mut indices = Vec::new();
    for r in 0..rings {
        for s in 0..segments {
            let a = r * row + s;
            let b = a + 1;
            let c = a + row;
            let d = c + 1;
            // Counter-clockwise seen from outside: top-left, bottom-left, bottom-right.
            indices.extend_from_slice(&[a, c, d, a, d, b]);
        }
    }
    let mut m = MeshData::new(positions, indices);
    m.label = Some("skinned-cylinder".into());
    m.normals = Some(normals);
    m.uvs = Some(uvs);
    m.compute_tangents();
    m
}

/// Two-bone skin for a mesh along +Y: bone 0 below the joint at `y = joint`, bone 1 above,
/// blended linearly over `joint ± blend` (`blend = 0`: rigid). Bone 1 is stored in slot 0
/// where it dominates, so both slot orders are exercised. Palette size `palette_size`.
#[must_use]
pub fn two_bone_skin(mesh: &MeshData, joint: f32, blend: f32, palette_size: u16) -> MeshSkin {
    let mut skin = MeshSkin {
        palette_size,
        ..MeshSkin::default()
    };
    for p in &mesh.positions {
        let w1 = if blend > 0.0 {
            ((p[1] - (joint - blend)) / (2.0 * blend)).clamp(0.0, 1.0)
        } else if p[1] > joint {
            1.0
        } else {
            0.0
        };
        if w1 >= 0.5 {
            skin.joints.push([1, 0, 0, 0]);
            skin.weights.push([w1, 1.0 - w1, 0.0, 0.0]);
        } else {
            skin.joints.push([0, 1, 0, 0]);
            skin.weights.push([1.0 - w1, w1, 0.0, 0.0]);
        }
    }
    skin
}

/// Three-bone chain skin along +Y with joints at `j1 < j2`, blended over `± blend` around
/// each joint; vertices near both joints (when the zones overlap) get three influences, and
/// every vertex carries a fourth slot (bone `extra`, weight 0.0 or a small share) so all
/// four slots are used.
#[must_use]
pub fn three_bone_skin(mesh: &MeshData, j1: f32, j2: f32, blend: f32, extra: u8) -> MeshSkin {
    let mut skin = MeshSkin {
        palette_size: 4,
        ..MeshSkin::default()
    };
    let ramp = |y: f32, j: f32| ((y - (j - blend)) / (2.0 * blend)).clamp(0.0, 1.0);
    for p in &mesh.positions {
        let a = ramp(p[1], j1);
        let b = ramp(p[1], j2);
        // Partition of unity over three bones.
        let w0 = 1.0 - a;
        let w2 = b * a;
        let w1 = a - w2;
        // Move 5 % of bone 1's share to the fourth slot, a bone whose palette entry
        // equals bone 1's (no visual change, but the fourth slot is read).
        let moved = w1 * 0.05;
        skin.joints.push([0, 1, 2, extra]);
        skin.weights.push([w0, w1 - moved, w2, moved]);
    }
    skin
}

/// Skinning matrix of a bone that rotates by `angle` about the Z axis through `pivot`
/// (bind pose → posed, `skeleton-anim.md` §4.4 with an identity rest rotation).
#[must_use]
pub fn bend(pivot: Vec3, angle: f32) -> Mat4 {
    Mat4::from_translation(pivot) * Mat4::from_rotation_z(angle) * Mat4::from_translation(-pivot)
}

/// Palette for [`two_bone_skin`]: bone 0 fixed, bone 1 bent by `angle` at `y = joint`.
#[must_use]
pub fn two_bone_palette(joint: f32, angle: f32) -> Vec<Mat4> {
    vec![Mat4::IDENTITY, bend(Vec3::new(0.0, joint, 0.0), angle)]
}

/// Palette for [`three_bone_skin`]: each joint bends by `angle`, bone 2 follows bone 1.
#[must_use]
pub fn three_bone_palette(j1: f32, j2: f32, angle: f32) -> Vec<Mat4> {
    let b1 = bend(Vec3::new(0.0, j1, 0.0), angle);
    // Bone 2's posed transform = bone 1's, then its own bend at the (bind-pose) joint 2.
    let b2 = b1 * bend(Vec3::new(0.0, j2, 0.0), angle);
    vec![Mat4::IDENTITY, b1, b2, b1]
}
