// SPDX-License-Identifier: GPL-3.0-or-later
//! Skeletons (`docs/specs/havok-data.md` §4.2, decode rule §9.1; structure kind K6 of §11)
//! on arbitrary bytes.
//!
//! The first byte picks a mode:
//!
//! - 0: raw tagfile bytes after the mode byte (also tried as a 0x04 setup prefix);
//! - 1: a skeleton described by the input and written by `tessera_havok::testkit`: bone
//!   count, parents (valid or not), names, lock flags, poses (finite or not, unit
//!   quaternions or not), member overrides that change lengths or drop members; then
//!   damaged by edits and a cut.
//!
//! Nothing may panic. An undamaged description must decode exactly when it satisfies §9.1,
//! and then reproduce its bones.

#![no_main]

use libfuzzer_sys::fuzz_target;
use tessera_havok::testkit::{
    BoneSpec, ByteSource, SkeletonSpec, WValue, damage, quat_from_angles,
};
use tessera_havok::{
    FailureCategory, Skeleton, Transform, classify_setup_record, classify_skeleton_tagfile,
    quat_length,
};

fn check(bytes: &[u8]) {
    let c = classify_skeleton_tagfile(bytes);
    match Skeleton::parse(bytes) {
        Ok(s) => {
            assert!(c.is_ok());
            assert!(!s.bones.is_empty());
            for (i, b) in s.bones.iter().enumerate() {
                assert!(b.parent.is_none_or(|p| p < i));
                assert!(b.reference.is_finite());
                assert!((quat_length(b.reference.rotation) - 1.0).abs() <= 1e-3);
                if let Some(n) = &b.name {
                    assert_eq!(s.find_bone(n).map(|k| k <= i), Some(true));
                }
            }
        }
        Err(e) => assert_eq!(c.failure(), Some(e.category())),
    }
    let _ = classify_setup_record(bytes);
}

fn spec(src: &mut ByteSource<'_>) -> (SkeletonSpec, bool) {
    let n = src.below(40);
    let mut valid = n > 0;
    let mut parents_valid = true;
    let names = src.u8() % 3;
    let mut bones = Vec::with_capacity(n);
    for i in 0..n {
        let parent = match src.u8() % 16 {
            0 => {
                let p = i32::from(src.u8() as i8);
                parents_valid &= p == -1 || (p >= 0 && (p as usize) < i);
                p
            }
            _ => src.below(i + 1) as i32 - 1,
        };
        let rotation = match src.u8() % 8 {
            0 => {
                let q = [src.f32(), src.f32(), src.f32(), src.f32()];
                valid &= q.iter().all(|v| v.is_finite()) && (quat_length(q) - 1.0).abs() <= 1e-3;
                q
            }
            _ => quat_from_angles(src.small_f32(), src.small_f32(), src.small_f32()),
        };
        let translation = [src.f32(), src.f32(), src.f32()];
        let scale = if src.u8().is_multiple_of(4) {
            [src.f32(), src.f32(), src.f32()]
        } else {
            [1.0; 3]
        };
        let reference = Transform {
            translation,
            rotation,
            scale,
        };
        valid &= reference.is_finite();
        bones.push(BoneSpec {
            name: match names {
                0 => None,
                1 => Some(i.to_string()),
                _ => Some(format!("rig_{}", src.u8())),
            },
            parent,
            reference,
            lock_translation: src.u8().is_multiple_of(8),
        });
    }
    let mut s = SkeletonSpec {
        bones,
        overrides: Vec::new(),
    };
    match src.u8() % 10 {
        0 => {
            let k = src.below(5);
            s.overrides
                .push(("parentIndices".into(), WValue::IntArray(vec![-1; k])));
            // All roots is a valid parent array when the length still matches.
            parents_valid = k == n;
        }
        1 => {
            s.overrides.push((
                "referencePose".into(),
                WValue::Vec12Array(vec![[0.0; 12]; src.below(5)]),
            ));
            valid = false;
        }
        2 => {
            s.overrides
                .push(("name".into(), WValue::CString("skeleton".into())));
        }
        3 => {
            s.overrides.push((
                "referenceFloats".into(),
                WValue::RealArray(vec![src.f32(); src.below(4)]),
            ));
        }
        _ => {}
    }
    (s, valid && parents_valid)
}

fuzz_target!(|data: &[u8]| {
    let Some((&mode, rest)) = data.split_first() else {
        return;
    };
    if mode.is_multiple_of(2) {
        check(rest);
        return;
    }
    let mut src = ByteSource::new(rest);
    let (s, valid) = spec(&mut src);
    let mut bytes = s.to_tagfile();
    match Skeleton::parse(&bytes) {
        Ok(k) => {
            assert!(valid, "accepted an invalid description");
            assert_eq!(k.bones.len(), s.bones.len());
            for (b, bs) in k.bones.iter().zip(&s.bones) {
                assert_eq!(b.reference, bs.reference);
                assert_eq!(b.lock_translation, bs.lock_translation);
            }
        }
        Err(e) => {
            assert!(!valid, "rejected a valid description: {e}");
            // A column-less bone array longer than the bytes left is refused by the
            // container's count bound (§3.8) before the length check.
            assert!(
                matches!(
                    e.category(),
                    FailureCategory::ObjectInconsistent | FailureCategory::TagfileTruncated
                ),
                "{e}"
            );
        }
    }
    let prefix = s.to_setup_prefix(0x0400_0001);
    let _ = classify_setup_record(&prefix);
    damage(&mut bytes, &mut src);
    check(&bytes);
});
