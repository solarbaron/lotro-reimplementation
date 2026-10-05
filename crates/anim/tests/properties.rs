// SPDX-License-Identifier: GPL-3.0-or-later
//! Property tests on random skeletons and poses, plus the asset-store decoders.

mod common;

use std::sync::Arc;
use std::time::Duration;

use common::*;
use glam::{Mat4, Vec3};
use proptest::prelude::*;
use tessera_anim::{
    BoundClip, ClipDecoder, Skeleton, SkeletonDecoder, blend_transform, skin_position,
    skinning_matrices,
};
use tessera_assets::{AssetStore, MemorySource, PoolConfig, Priority, StoreConfig, WorkerPool};
use tessera_havok::Transform;
use tessera_havok::testkit::quat_from_angles;

fn arb_transform() -> impl Strategy<Value = Transform> {
    (
        prop::array::uniform3(-2.0f32..2.0),
        prop::array::uniform3(-3.2f32..3.2),
        prop::array::uniform3(0.5f32..1.5),
    )
        .prop_map(|(t, a, s)| Transform {
            translation: t,
            rotation: quat_from_angles(a[0], a[1], a[2]),
            scale: s,
        })
}

fn arb_skeleton() -> impl Strategy<Value = (Vec<i32>, Vec<Transform>)> {
    (1usize..40).prop_flat_map(|n| {
        (
            prop::collection::vec(any::<u16>(), n),
            prop::collection::vec(arb_transform(), n),
        )
            .prop_map(|(raw, t)| {
                let parents = raw
                    .iter()
                    .enumerate()
                    .map(|(i, r)| {
                        if i == 0 || r % 5 == 0 {
                            -1
                        } else {
                            i32::from(*r) % i as i32
                        }
                    })
                    .collect();
                (parents, t)
            })
    })
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(64))]

    #[test]
    fn compose_and_skin_agree_with_definitions((parents, reference) in arb_skeleton(),
                                               pose_seed in prop::collection::vec(arb_transform(), 40)) {
        let s = Skeleton::from_parts(&parents, &reference).unwrap();
        let n = s.bone_count();
        // B_b · B_b⁻¹ = I.
        for (m, inv) in s.reference_model().iter().zip(s.inverse_bind()) {
            prop_assert!((*m * *inv).abs_diff_eq(Mat4::IDENTITY, 1e-2));
        }
        let local: Vec<Transform> = pose_seed[..n].to_vec();
        let mut model = vec![Mat4::IDENTITY; n];
        s.compose(&local, &mut model).unwrap();
        for b in 0..n {
            let l = tessera_anim::local_matrix(&local[b]);
            let expect = match s.parent(b) { Some(p) => model[p] * l, None => l };
            prop_assert!(model[b].abs_diff_eq(expect, 1e-3));
        }
        let mut skin = vec![Mat4::IDENTITY; n];
        skinning_matrices(&s, &model, &mut skin).unwrap();
        // A vertex rigidly bound to bone b follows bone b exactly: W_b · B_b⁻¹ · v.
        let v = Vec3::new(0.3, -0.2, 0.7);
        for (b, (m, inv)) in model.iter().zip(s.inverse_bind()).enumerate() {
            let p = skin_position(&skin, &[b as u8], &[1.0], v);
            let expect = m.transform_point3(inv.transform_point3(v));
            prop_assert!(p.abs_diff_eq(expect, 1e-2 * (1.0 + expect.length())));
        }
    }

    #[test]
    fn blends_stay_between_endpoints(a in arb_transform(), b in arb_transform(), alpha in -1.0f32..2.0) {
        let m = blend_transform(&a, &b, alpha);
        let al = alpha.clamp(0.0, 1.0);
        for k in 0..3 {
            let lo = a.translation[k].min(b.translation[k]) - 1e-5;
            let hi = a.translation[k].max(b.translation[k]) + 1e-5;
            prop_assert!((lo..=hi).contains(&m.translation[k]));
        }
        prop_assert!((tessera_havok::quat_length(m.rotation) - 1.0).abs() < 1e-4);
        // Rotation stays on the shortest arc between a and b.
        let to_b = quat_angle(a.rotation, b.rotation);
        let am = quat_angle(a.rotation, m.rotation);
        let mb = quat_angle(m.rotation, b.rotation);
        prop_assert!(am <= to_b + 2e-3 && mb <= to_b + 2e-3);
        prop_assert!((am + mb - to_b).abs() < 2e-3);
        if al == 0.0 { prop_assert!(am < 2e-3); }
    }
}

#[test]
fn decoders_run_on_the_asset_store() {
    let pool = WorkerPool::new(PoolConfig {
        workers: 2,
        ..PoolConfig::default()
    });
    let spec = tree_spec(10);
    let s = skeleton(&spec);
    let clip_rec = spline_clip(61, 10, 60.0, cyclic(&s, 61, 0.5)).to_bytes();
    let mut src = MemorySource::new();
    src.insert(0x0500_0001, clip_rec);
    src.insert(0x0500_0002, vec![1, 2, 3]);
    let clips = AssetStore::new(&pool, src, ClipDecoder, StoreConfig::default());
    let h = clips.request(0x0500_0001, Priority::Visible);
    let clip = h
        .wait(Duration::from_secs(20))
        .ready()
        .cloned()
        .expect("decoded");
    let bound = BoundClip::bind(clip, s.clone()).unwrap();
    assert_eq!(bound.frame_count(), 61);
    assert!(clips.stats().cache_bytes > 0);
    let bad = clips.request(0x0500_0002, Priority::Visible);
    assert!(bad.wait(Duration::from_secs(20)).ready().is_none());

    let mut src = MemorySource::new();
    src.insert(0x0400_0001, spec.to_setup_prefix(0x0400_0001));
    let skels = AssetStore::new(&pool, src, SkeletonDecoder, StoreConfig::default());
    let h = skels.request(0x0400_0001, Priority::Visible);
    let sk = h
        .wait(Duration::from_secs(20))
        .ready()
        .cloned()
        .expect("decoded");
    assert_eq!(*sk, *s);
    let _: Arc<Skeleton> = sk;
}
