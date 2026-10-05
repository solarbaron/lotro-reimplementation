// SPDX-License-Identifier: GPL-3.0-or-later
//! Pose blending (`skeleton-anim.md` §6.5, §6.6) and scalar-blend weights
//! (`script-table.md` §7.3).

use tessera_havok::{Transform, quat_nlerp};

use crate::error::Result;
use crate::skeleton::check_len;

fn clamp01(w: f32) -> f32 {
    if w.is_nan() { 0.0 } else { w.clamp(0.0, 1.0) }
}

/// Blends one transform towards another by `alpha` (0 = `a`, 1 = `b`): translation and
/// scale linearly, rotation by normalised linear interpolation after negating `b`'s
/// quaternion when its dot product with `a`'s is negative (shortest path, §6.5).
#[must_use]
pub fn blend_transform(a: &Transform, b: &Transform, alpha: f32) -> Transform {
    let alpha = clamp01(alpha);
    if alpha == 0.0 {
        return *a;
    }
    if alpha == 1.0 {
        // Still sign-aligned and normalised like every other blend result.
        return Transform {
            rotation: quat_nlerp(b.rotation, b.rotation, 0.0),
            ..*b
        };
    }
    Transform {
        translation: std::array::from_fn(|k| {
            a.translation[k] + (b.translation[k] - a.translation[k]) * alpha
        }),
        rotation: quat_nlerp(a.rotation, b.rotation, alpha),
        scale: std::array::from_fn(|k| a.scale[k] + (b.scale[k] - a.scale[k]) * alpha),
    }
}

/// Two-clip blend (a cross-fade step, or two locomotion clips): `out[j]` = blend of `a[j]`
/// and `b[j]` by `alpha` for every track of `a`.
///
/// # Errors
///
/// [`AnimError::BufferTooSmall`](crate::AnimError::BufferTooSmall) when `b` or `out` is
/// shorter than `a`.
pub fn blend_poses(
    a: &[Transform],
    b: &[Transform],
    alpha: f32,
    out: &mut [Transform],
) -> Result<()> {
    check_len("blend input", a.len(), b.len())?;
    check_len("blend output", a.len(), out.len())?;
    for ((o, x), y) in out.iter_mut().zip(a).zip(b) {
        *o = blend_transform(x, y, alpha);
    }
    Ok(())
}

/// Layers `layer` over `base` in place (§6.6): each track becomes
/// base + *w_j* · (layer − base) with *w_j* = `weight` × the track's weight (1 when
/// `track_weights` is `None` or too short), using the rotation blend of §6.5. With weight 1
/// and no track weights the result is the layer pose.
///
/// # Errors
///
/// [`AnimError::BufferTooSmall`](crate::AnimError::BufferTooSmall) when `layer` is shorter
/// than `base`.
pub fn layer_over(
    base: &mut [Transform],
    layer: &[Transform],
    weight: f32,
    track_weights: Option<&[f32]>,
) -> Result<()> {
    check_len("layer pose", base.len(), layer.len())?;
    let weight = clamp01(weight);
    if weight == 0.0 {
        return Ok(());
    }
    for (j, (b, l)) in base.iter_mut().zip(layer).enumerate() {
        let tw = track_weights
            .and_then(|w| w.get(j).copied())
            .map_or(1.0, clamp01);
        *b = blend_transform(b, l, weight * tw);
    }
    Ok(())
}

/// Weights of a scalar blend (`script-table.md` §7.3): for parameter `p` the two anchors
/// that bracket it get weights by linear interpolation, every other anchor 0; outside the
/// anchors' range the nearest anchor gets weight 1. Anchors need not be sorted; among equal
/// anchors the first listed wins. `out` receives one weight per anchor (extra entries are
/// set to 0); a NaN `p` or no finite anchor gives all zeros. Every weight is in `0 ..= 1`
/// and the weights sum to 1 or 0.
pub fn scalar_blend_weights(anchors: &[f32], p: f32, out: &mut [f32]) {
    for o in out.iter_mut() {
        *o = 0.0;
    }
    if p.is_nan() {
        return;
    }
    // Nearest anchor at or below p, and nearest strictly above p.
    let mut lo: Option<(usize, f32)> = None;
    let mut hi: Option<(usize, f32)> = None;
    for (i, &a) in anchors.iter().enumerate() {
        if !a.is_finite() {
            continue;
        }
        if a <= p {
            if lo.is_none_or(|(_, v)| a > v) {
                lo = Some((i, a));
            }
        } else if hi.is_none_or(|(_, v)| a < v) {
            hi = Some((i, a));
        }
    }
    let mut set = |i: usize, w: f32| {
        if let Some(o) = out.get_mut(i) {
            *o = w;
        }
    };
    match (lo, hi) {
        (Some((i, a)), Some((k, b))) => {
            // In f64: with f32 anchors near ±3e38 the differences overflow f32 and the
            // ratio would be inf / inf = NaN. Here a < b, so the divisor is positive.
            let alpha = ((f64::from(p) - f64::from(a)) / (f64::from(b) - f64::from(a)))
                .clamp(0.0, 1.0) as f32;
            set(i, 1.0 - alpha);
            set(k, alpha);
        }
        (Some((i, _)), None) | (None, Some((i, _))) => set(i, 1.0),
        (None, None) => {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rot_z(deg: f32) -> [f32; 4] {
        let h = deg.to_radians() * 0.5;
        [0.0, 0.0, h.sin(), h.cos()]
    }

    fn angle_z(q: [f32; 4]) -> f32 {
        (2.0 * q[2].atan2(q[3])).to_degrees()
    }

    /// `skeleton-anim.md` §14.6.
    #[test]
    fn partial_clip_example_14_6() {
        let mut base = vec![Transform::IDENTITY; 4];
        let mut layer = vec![Transform::IDENTITY; 4];
        layer[2].rotation = rot_z(90.0);
        layer[3].rotation = rot_z(90.0);
        layer[0].translation = [1.0, 2.0, 3.0];
        layer[1].scale = [2.0; 3];
        layer_over(&mut base, &layer, 1.0, Some(&[1.0, 1.0, 0.01, 0.5])).unwrap();
        assert!((angle_z(base[3].rotation) - 45.0).abs() < 1e-4);
        let a2 = angle_z(base[2].rotation);
        assert!((a2 - 0.8126).abs() < 1e-3, "{a2}");
        assert_eq!(base[0].translation, [1.0, 2.0, 3.0]);
        assert_eq!(base[1].scale, [2.0; 3]);
    }

    #[test]
    fn blend_weights_and_shortest_path() {
        let a = Transform::IDENTITY;
        let mut b = Transform::IDENTITY;
        // −q is the same rotation: the blend must stay at identity.
        b.rotation = [0.0, 0.0, 0.0, -1.0];
        b.translation = [2.0, 0.0, 0.0];
        let m = blend_transform(&a, &b, 0.5);
        assert!((m.rotation[3].abs() - 1.0).abs() < 1e-6);
        assert_eq!(m.translation, [1.0, 0.0, 0.0]);
        assert_eq!(blend_transform(&a, &b, f32::NAN), a);
        assert_eq!(blend_transform(&a, &b, -3.0), a);
        assert_eq!(blend_transform(&a, &b, 7.0).translation, [2.0, 0.0, 0.0]);
        let mut out = vec![Transform::IDENTITY; 1];
        assert!(blend_poses(&[a, a], &[b], 0.5, &mut out).is_err());
        blend_poses(&[a], &[b], 0.25, &mut out).unwrap();
        assert_eq!(out[0].translation, [0.5, 0.0, 0.0]);
    }

    #[test]
    fn scalar_blend() {
        let anchors = [-1.0, 0.0, 1.0];
        let mut w = [9.0; 4];
        scalar_blend_weights(&anchors, 0.25, &mut w);
        assert_eq!(w, [0.0, 0.75, 0.25, 0.0]);
        scalar_blend_weights(&anchors, -5.0, &mut w);
        assert_eq!(w, [1.0, 0.0, 0.0, 0.0]);
        scalar_blend_weights(&anchors, 5.0, &mut w);
        assert_eq!(w, [0.0, 0.0, 1.0, 0.0]);
        scalar_blend_weights(&anchors, 0.0, &mut w);
        assert_eq!(w, [0.0, 1.0, 0.0, 0.0]);
        scalar_blend_weights(&[6.0, 0.0, 2.0], 4.0, &mut w);
        assert_eq!(w, [0.5, 0.0, 0.5, 0.0]);
        scalar_blend_weights(&anchors, f32::NAN, &mut w);
        assert_eq!(w, [0.0; 4]);
        scalar_blend_weights(&[f32::NAN], 1.0, &mut w);
        assert_eq!(w, [0.0; 4]);
        let mut short = [0.0; 1];
        scalar_blend_weights(&anchors, 0.5, &mut short);
        assert_eq!(short, [0.0]);
    }

    /// Review finding: anchors whose span overflows f32 gave NaN weights.
    #[test]
    fn scalar_blend_huge_anchors() {
        let mut w = [9.0f32; 2];
        for p in [
            2.0e38f32,
            1.0e38,
            0.0,
            -3.0e38,
            3.0e38,
            f32::MAX,
            f32::INFINITY,
        ] {
            scalar_blend_weights(&[-3.0e38, 3.0e38], p, &mut w);
            assert!(w.iter().all(|x| (0.0..=1.0).contains(x)), "{p}: {w:?}");
            assert!((w[0] + w[1] - 1.0).abs() < 1e-6, "{p}: {w:?}");
        }
        scalar_blend_weights(&[-f32::MAX, f32::MAX], 0.0, &mut w);
        assert_eq!(w, [0.5, 0.5]);
    }
}
