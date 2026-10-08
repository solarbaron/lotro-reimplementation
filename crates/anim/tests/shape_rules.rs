// SPDX-License-Identifier: GPL-3.0-or-later
//! Figure shape rules on synthetic data: deformer times and skipped deformers
//! (`appearance.md` §6.4 step 1, `script-table.md` §6.9) and held clip times with the end
//! crop (`script-table.md` §6.7 step 2).

mod common;

use std::sync::Arc;

use common::*;
use tessera_anim::{
    BindDeformation, DeformClip, MAX_HELD_CLIPS, Skeleton, bind_shape, deformer_time,
    held_clip_fraction,
};
use tessera_formats::Did;
use tessera_havok::Transform;
use tessera_mesh::assembly::shape::{BodyShapeClip, FigureShape, HeldClip, ShapeDriver};

fn at_z(z: f32) -> Transform {
    Transform {
        translation: [0.0, 0.0, z],
        ..Transform::IDENTITY
    }
}

/// One bone; a 2-second clip whose translation z runs linearly from 0 to 2.
fn ramp() -> (Arc<Skeleton>, Arc<tessera_anim::BoundClip>) {
    let s = Arc::new(Skeleton::from_parts(&[-1], &[Transform::IDENTITY]).unwrap());
    let clip = bind(&interleaved_clip(3, 1, 2.0, |f, _| at_z(f as f32)), &s);
    (s, clip)
}

fn deformed_z(s: &Skeleton, clips: &[DeformClip]) -> f32 {
    let d = BindDeformation::new(s, clips, None).unwrap();
    d.local_pose()[0].translation[2]
}

/// `appearance.md` §6.4 step 1 (BIN4): the time is duration × fraction limited to
/// [0, duration] after the multiplication: +∞ gives the end, −∞ gives 0, NaN reads 0.
#[test]
fn deformer_time_limits_after_the_multiplication() {
    assert_eq!(deformer_time(0.25, 2.0), 0.5);
    assert_eq!(deformer_time(f32::INFINITY, 2.0), 2.0);
    assert_eq!(deformer_time(f32::NEG_INFINITY, 2.0), 0.0);
    assert_eq!(deformer_time(f32::NAN, 2.0), 0.0);
    assert_eq!(deformer_time(3.0, 2.0), 2.0);
    assert_eq!(deformer_time(-3.0, 2.0), 0.0);
    assert_eq!(deformer_time(0.5, 0.0), 0.0);
    assert_eq!(deformer_time(0.5, f32::NAN), 0.0);

    let (s, clip) = ramp();
    let at = |fraction: f32| {
        deformed_z(
            &s,
            &[DeformClip {
                clip: Arc::clone(&clip),
                fraction,
                use_bone_weights: false,
            }],
        )
    };
    assert!((at(0.25) - 0.5).abs() < 1e-5);
    assert!((at(f32::INFINITY) - 2.0).abs() < 1e-5, "+∞ is the end");
    assert!(at(f32::NEG_INFINITY).abs() < 1e-6);
    assert!(at(f32::NAN).abs() < 1e-6);
}

/// `appearance.md` §6.4 step 1, `script-table.md` §6.9: a deformer clip whose track count
/// differs from the bone count contributes nothing (the others still do); with none left
/// the pose is the reference pose. Clips beyond the Tessera bound are dropped, not fatal.
#[test]
fn unfit_and_surplus_deformers_contribute_nothing() {
    let (s, clip) = ramp();
    let two = Arc::new(Skeleton::from_parts(&[-1, 0], &[Transform::IDENTITY; 2]).unwrap());
    let other = bind(&interleaved_clip(2, 2, 1.0, |_, _| at_z(9.0)), &two);
    let fits = DeformClip {
        clip: Arc::clone(&clip),
        fraction: 0.5,
        use_bone_weights: false,
    };
    let unfit = DeformClip {
        clip: other,
        fraction: 0.5,
        use_bone_weights: false,
    };
    assert!((deformed_z(&s, &[unfit.clone(), fits.clone()]) - 1.0).abs() < 1e-5);
    let alone = BindDeformation::new(&s, &[unfit], None).unwrap();
    assert!(alone.is_identity(1e-6));

    let mut many = vec![fits; BindDeformation::MAX_CLIPS];
    many.push(DeformClip {
        clip,
        fraction: 1.0,
        use_bone_weights: false,
    });
    // The 17th clip (at the end, z = 2) is dropped: the average stays at z = 1.
    assert!((deformed_z(&s, &many) - 1.0).abs() < 1e-5);
}

fn held(fraction: f32, end_crop: f32) -> HeldClip {
    HeldClip {
        clip: Did(0x0500_0001),
        layer: 110,
        fraction,
        end_crop,
        use_bone_opacity: true,
        driver: Some(ShapeDriver::BoneScale),
    }
}

/// `script-table.md` §6.7 step 2: the held clip is shown at fraction × (duration − end
/// crop), limited to that length; 1/30 s for a one-shot node with `LoopCount` 1.
#[test]
fn held_clips_use_the_played_length() {
    let (s, clip) = ramp();
    let crop = 1.0 / 30.0;
    assert_eq!(held_clip_fraction(&held(0.25, 0.0), &clip), 0.25);
    let f = held_clip_fraction(&held(1.0, crop), &clip);
    assert!((f * 2.0 - (2.0 - crop)).abs() < 1e-6, "{f}");
    let f = held_clip_fraction(&held(0.25, crop), &clip);
    assert!((f * 2.0 - 0.491_666_7).abs() < 1e-6, "{f}");

    // Through the shared path; more than the Tessera bound of held clips is noted.
    let shape = FigureShape {
        held: vec![held(1.0, crop); MAX_HELD_CLIPS + 2],
        body_shape: vec![BodyShapeClip {
            clip: Did(0x0500_0001),
            fraction: 0.5,
            use_bone_weights: false,
        }],
        ..FigureShape::default()
    };
    let bound = bind_shape(&shape, &s, |_| Ok::<_, String>(Arc::clone(&clip)));
    assert_eq!(bound.held.len(), MAX_HELD_CLIPS);
    assert!(
        bound.notes.iter().any(|n| n.contains("dropped")),
        "{:?}",
        bound.notes
    );
    let f = bound.held[0].clip_fraction;
    assert!((f * 2.0 - (2.0 - crop)).abs() < 1e-6, "{f}");
    assert!(bound.deformation.is_some());
}
