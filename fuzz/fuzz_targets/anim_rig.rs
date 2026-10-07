// SPDX-License-Identifier: GPL-3.0-or-later
//! `tessera-anim` on untrusted plain data (`docs/specs/skeleton-anim.md` §4, §5, §6, §8):
//! skeletons from parent arrays and transforms, holding-location tables and bone maps from
//! setup-style entries, skinning inputs (bone indices, weights, palettes of any size),
//! blends, scalar-blend weights, clip time and motion algebra. Every value comes from the
//! input, including NaN, infinities, negative parents, cycles and out-of-range bones.
//! Nothing may panic, and validated structures keep their invariants.

#![no_main]

use libfuzzer_sys::fuzz_target;
use tessera_anim::glam::{Mat4, Quat, Vec3};
use tessera_anim::{
    AttachFrame, AttachPointDesc, AttachTable, BoneMap, CompactPalette, MotionDelta, Skeleton,
    Wrap, blend_transform, check_joints, clip_time, influence_list, layer_over,
    scalar_blend_weights, skin_direction, skin_position, skinning_matrices, write_palette_mat4,
    write_palette_rows3x4,
};
use tessera_havok::Transform;
use tessera_havok::testkit::ByteSource;

fn transform(src: &mut ByteSource<'_>) -> Transform {
    Transform {
        translation: [src.f32(), src.f32(), src.f32()],
        rotation: [src.f32(), src.f32(), src.f32(), src.f32()],
        scale: if src.u8().is_multiple_of(8) {
            [src.f32(), src.f32(), src.f32()]
        } else {
            [1.0; 3]
        },
    }
}

fuzz_target!(|data: &[u8]| {
    let mut src = ByteSource::new(data);
    let n = src.below(300);
    let parents: Vec<i32> = (0..n)
        .map(|i| match src.u8() % 8 {
            0 => src.u32() as i32,
            1 => -1,
            _ => src.below(i.max(1)) as i32 - i32::from(i == 0),
        })
        .collect();
    let reference: Vec<Transform> = (0..n.min(src.below(301).max(n.saturating_sub(1))))
        .map(|_| transform(&mut src))
        .collect();
    let skeleton = Skeleton::from_parts(&parents, &reference);
    let bones = skeleton.as_ref().map_or(src.below(64), |s| s.bone_count());
    if let Ok(s) = &skeleton {
        for i in 0..s.bone_count() {
            if let Some(p) = s.parent(i) {
                assert!(p < i, "parents precede children");
            }
        }
        let mut model = vec![Mat4::IDENTITY; bones];
        let local: Vec<Transform> = (0..bones).map(|_| transform(&mut src)).collect();
        if s.compose(&local, &mut model).is_ok() {
            assert!(
                model.iter().all(Mat4::is_finite),
                "composed model matrices are finite"
            );
        }
        let mut skin = vec![Mat4::IDENTITY; src.below(bones + 2)];
        if skinning_matrices(s, &model, &mut skin).is_ok() {
            assert!(
                skin.iter().all(Mat4::is_finite),
                "skinning matrices are finite"
            );
        }
        let mut pal = vec![[0.0f32; 16]; src.below(300)];
        let _ = write_palette_mat4(&skin, &mut pal);
        let mut rows = vec![[0.0f32; 12]; src.below(300)];
        let _ = write_palette_rows3x4(&skin, &mut rows);
        let _ = s.content_key();
        let _ = s.roots().count();
    }

    // Holding locations and bone map.
    let entries: Vec<AttachPointDesc> = (0..src.below(40))
        .map(|_| AttachPointDesc {
            key: u32::from(src.u8() % 16),
            bone: src.u8() as i32 - 4,
            frame: AttachFrame {
                position: [src.f32(), src.f32(), src.f32()],
                scale: [src.f32(), src.f32(), src.f32()],
                rotation_wxyz: [src.f32(), src.f32(), src.f32(), src.f32()],
            },
        })
        .collect();
    let model: Vec<Mat4> = (0..src.below(bones + 2))
        .map(|_| Mat4::from_translation(Vec3::new(src.f32(), src.f32(), src.f32())))
        .collect();
    if let Ok(t) = AttachTable::new(&entries, bones) {
        for w in t.points().windows(2) {
            assert!(w[0].key < w[1].key);
        }
        for p in t.points() {
            assert!(p.bone.is_none_or(|b| usize::from(b) < bones));
        }
        let _ = t.world_transform(u32::from(src.u8() % 16), &model, Mat4::IDENTITY);
    }
    let (t, issues) = AttachTable::new_skipping_invalid(&entries, bones);
    assert!(t.len() <= entries.len());
    drop(issues);
    let mut out = Vec::new();
    t.model_transforms(&model, &mut out);
    let pairs: Vec<(u32, u32)> = (0..src.below(20))
        .map(|_| (u32::from(src.u8()), u32::from(src.u8())))
        .collect();
    if let Ok(m) = BoneMap::new(&pairs, bones) {
        for (_, b) in m.entries() {
            assert!(usize::from(*b) < bones);
        }
        let _ = m.model_transform(u32::from(src.u8()), &model);
    }

    // Skinning inputs.
    let k = src.below(6);
    let verts = src.below(40);
    let joints: Vec<u8> = (0..verts * k).map(|_| src.u8()).collect();
    let weights: Vec<f32> = (0..verts * k + src.below(3)).map(|_| src.f32()).collect();
    let _ = influence_list(&joints, &weights, k);
    let j4: Vec<[u8; 4]> = (0..verts)
        .map(|_| [src.u8(), src.u8(), src.u8(), src.u8()])
        .collect();
    let w4: Vec<[f32; 4]> = (0..verts)
        .map(|_| [src.f32(), src.f32(), src.f32(), src.f32()])
        .collect();
    let cp = CompactPalette::from_vertices(&j4, &w4);
    let skin: Vec<Mat4> = (0..src.below(260))
        .map(|_| Mat4::from_quat(Quat::from_rotation_z(src.f32())))
        .collect();
    let mut pal = vec![[0.0f32; 16]; src.below(260)];
    let _ = cp.write_mat4(&skin, &mut pal);
    let mut remapped = j4.clone();
    cp.remap_joints(&mut remapped);
    for j in &remapped {
        for &x in j {
            assert!(usize::from(x) < cp.len().max(1));
        }
    }
    let _ = check_joints(&j4, &w4, bones);
    for (j, w) in j4.iter().zip(&w4) {
        let p = Vec3::new(src.f32(), src.f32(), src.f32());
        let _ = skin_position(&skin, j, w, p);
        let _ = skin_direction(&skin, j, w, p);
    }

    // Blending, scalar blends, clip time, motion algebra.
    let a = transform(&mut src);
    let b = transform(&mut src);
    let _ = blend_transform(&a, &b, src.f32());
    let mut base: Vec<Transform> = (0..src.below(8)).map(|_| transform(&mut src)).collect();
    let layer: Vec<Transform> = (0..src.below(8)).map(|_| transform(&mut src)).collect();
    let tw: Vec<f32> = (0..src.below(8)).map(|_| src.f32()).collect();
    let _ = layer_over(&mut base, &layer, src.f32(), Some(&tw));
    let anchors: Vec<f32> = (0..src.below(8)).map(|_| src.f32()).collect();
    let mut w = vec![0.0f32; src.below(10)];
    scalar_blend_weights(&anchors, src.f32(), &mut w);
    let s: f32 = w.iter().sum();
    assert!(s <= 1.0 + 1e-5, "weights sum to at most 1");
    assert!(
        w.iter().all(|x| (0.0..=1.0).contains(x)),
        "weights in 0..=1"
    );
    for wrap in [Wrap::Loop, Wrap::Once] {
        let d = src.f32();
        let t = clip_time(src.f32(), d, wrap);
        assert!(t.is_finite() && t >= 0.0);
        if d.is_finite() && d > 0.0 {
            assert!(t <= d);
        }
    }
    let m = MotionDelta {
        translation: Vec3::new(src.f32(), src.f32(), src.f32()),
        rotation: Quat::from_axis_angle(Vec3::Z, src.f32()),
    };
    let _ = m
        .then(m.inverse())
        .repeat(u64::from(src.u32()))
        .scaled(src.f32())
        .yaw();
});
