// SPDX-License-Identifier: GPL-3.0-or-later
//! One spline block (`docs/specs/havok-data.md` §5.2–§5.7, §7.3, §7.5, §7.6; structure kinds
//! K9 and K10 of §11) and the curve evaluator on arbitrary bytes.
//!
//! The first byte picks a mode:
//!
//! - 0: block numbers (tracks, frames, declared length) from the next bytes, raw block bytes
//!   after them;
//! - 1: a block described by the input and encoded by `tessera_havok::testkit`: per track,
//!   default, constant or spline position and scale channels (8- or 16-bit, any degree,
//!   uniform or raw knot vectors) and identity, static or dynamic rotations (including
//!   sign-flipped neighbours); then damaged;
//! - 2: a bare curve (degree, knots with zero-length spans, control points) evaluated at
//!   chosen frames.
//!
//! Decoded blocks are sampled at chosen local frames. Nothing may panic. An undamaged
//! description with valid curves must decode to exactly its declared length.

#![no_main]

use libfuzzer_sys::fuzz_target;
use tessera_havok::testkit::{
    BlockSpec, ByteSource, ComponentSpec, RotationSpec, TrackSpec, VectorSpec, damage,
    encode_block, quat_from_angles, uniform_knots,
};
use tessera_havok::{BlockParams, Curve, SplineBlock, Transform, quat_length};

fn sample(block: &SplineBlock, src: &mut ByteSource<'_>) {
    let mut out = vec![Transform::IDENTITY; block.tracks.len()];
    for _ in 0..4 {
        let u = src.time(block.frames as f32);
        assert_eq!(block.sample(u, &mut out), block.tracks.len());
        let _ = block.sample_track(src.below(block.tracks.len() + 1), u);
    }
    for u in [f32::NAN, f32::INFINITY, -1.0, 0.0, block.frames as f32] {
        block.sample(u, &mut out);
    }
}

/// Knots for a curve: uniform (valid) or raw from the input (`valid` cleared when they
/// break §5.4).
fn knots(src: &mut ByteSource<'_>, p: u8, points: usize, last: u8, valid: &mut bool) -> Vec<u8> {
    if !src.u8().is_multiple_of(8) {
        return uniform_knots(p, points, last);
    }
    *valid = false;
    (0..points + usize::from(p) + 1)
        .map(|_| src.u8() % (last.saturating_add(2)))
        .collect()
}

fn vector(src: &mut ByteSource<'_>, frames: u32, valid: &mut bool) -> VectorSpec {
    match src.u8() % 3 {
        0 => VectorSpec::DEFAULT,
        1 => VectorSpec::Constant([0, 1, 2].map(|_| (src.u8() & 1 == 1).then(|| src.f32()))),
        _ => {
            if frames < 2 {
                return VectorSpec::DEFAULT;
            }
            let last = (frames - 1).min(255) as u8;
            let p = 1 + src.below(3) as u8;
            let p = p.min(last);
            let points = (usize::from(p) + 1 + src.below(6)).min(usize::from(last) + 1);
            let mut components = [ComponentSpec::Default; 3];
            let mut dims = 0;
            for c in &mut components {
                *c = match src.u8() % 3 {
                    0 => ComponentSpec::Default,
                    1 => ComponentSpec::Static(src.f32()),
                    _ => {
                        dims += 1;
                        ComponentSpec::Dynamic {
                            min: src.small_f32(),
                            max: src.small_f32(),
                        }
                    }
                };
            }
            if dims == 0 {
                components[0] = ComponentSpec::Dynamic { min: 0.0, max: 1.0 };
                dims = 1;
            }
            let k = knots(src, p, points, last, valid);
            VectorSpec::Spline {
                degree: p,
                knots: k,
                components,
                eight_bit: src.u8().is_multiple_of(4),
                quantised: (0..points * dims).map(|_| src.u16()).collect(),
            }
        }
    }
}

fn quat(src: &mut ByteSource<'_>) -> [f32; 4] {
    let r = quat_from_angles(src.small_f32(), src.small_f32(), src.small_f32());
    if src.u8().is_multiple_of(4) {
        r.map(|c| -c)
    } else {
        r
    }
}

fn rotation(src: &mut ByteSource<'_>, frames: u32, valid: &mut bool) -> RotationSpec {
    match src.u8() % 3 {
        0 => RotationSpec::Identity,
        1 => RotationSpec::Static(quat(src)),
        _ => {
            if frames < 2 {
                return RotationSpec::Identity;
            }
            let last = (frames - 1).min(255) as u8;
            let p = (1 + src.below(3) as u8).min(last);
            let points = (usize::from(p) + 1 + src.below(6)).min(usize::from(last) + 1);
            let pts = (0..points).map(|_| quat(src)).collect();
            let k = knots(src, p, points, last, valid);
            RotationSpec::Dynamic {
                degree: p,
                knots: k,
                points: pts,
            }
        }
    }
}

fn synthetic(src: &mut ByteSource<'_>) {
    let frames = 1 + u32::from(src.u8());
    let tracks = src.below(6);
    let mut valid = true;
    let block = BlockSpec {
        tracks: (0..tracks)
            .map(|_| {
                let position = vector(src, frames, &mut valid);
                let rotation = rotation(src, frames, &mut valid);
                let scale = vector(src, frames, &mut valid);
                TrackSpec::new(position, rotation, scale)
            })
            .collect(),
    };
    let mut bytes = encode_block(&block);
    let params = BlockParams {
        block: 0,
        tracks,
        float_tracks: 0,
        frames,
        transform_length: bytes.len(),
    };
    // The block may extend into the 16-byte padding.
    let mut padded = bytes.clone();
    padded.resize(bytes.len().next_multiple_of(16), 0);
    match SplineBlock::parse(&padded, &params) {
        Ok(b) => {
            assert_eq!(b.tracks.len(), tracks);
            sample(&b, src);
            let mut out = vec![Transform::IDENTITY; tracks];
            b.sample(src.time(frames as f32), &mut out);
            for t in &out {
                if t.rotation.iter().all(|c| c.is_finite()) {
                    assert!(
                        (quat_length(t.rotation) - 1.0).abs() < 1e-4,
                        "{:?}",
                        t.rotation
                    );
                }
            }
        }
        Err(e) => assert!(!valid, "valid block rejected: {e}"),
    }
    damage(&mut bytes, src);
    if let Ok(b) = SplineBlock::parse(&bytes, &params) {
        sample(&b, src);
    }
}

fn raw(src: &mut ByteSource<'_>) {
    let params = BlockParams {
        block: src.below(4),
        tracks: src.below(16),
        float_tracks: src.below(2),
        frames: u32::from(src.u16() % 300),
        transform_length: usize::from(src.u16() % 512),
    };
    if let Ok(b) = SplineBlock::parse(src.rest(), &params) {
        assert_eq!(b.tracks.len(), params.tracks);
        let mut times = ByteSource::new(&[]);
        sample(&b, &mut times);
    }
}

fn curve(src: &mut ByteSource<'_>) {
    let p = src.below(5);
    let dims = 1 + src.below(4);
    let n = src.below(12);
    let knots: Vec<u8> = (0..n + p + 2).map(|_| src.u8() % 16).collect();
    let points: Vec<f32> = (0..(n + 1) * dims).map(|_| src.f32()).collect();
    if let Some(c) = Curve::new(p, &knots, &points, dims) {
        for _ in 0..4 {
            let u = src.time(16.0);
            let mut out = [0.0f32; 4];
            c.evaluate(u, &mut out);
            let _ = c.evaluate_rotation(u);
        }
    }
}

fuzz_target!(|data: &[u8]| {
    let Some((&mode, rest)) = data.split_first() else {
        return;
    };
    let mut src = ByteSource::new(rest);
    match mode % 3 {
        0 => raw(&mut src),
        1 => synthetic(&mut src),
        _ => curve(&mut src),
    }
});
