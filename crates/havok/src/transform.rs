// SPDX-License-Identifier: GPL-3.0-or-later
//! Local bone transforms (translation, rotation quaternion, scale) and small quaternion
//! helpers.

/// A transform relative to the parent bone, in the reference-pose layout of
/// `havok-data.md` §4.2: translation, rotation quaternion (x, y, z, w; w last) and scale.
///
/// Scale applies first, then rotation, then translation (Reported; composition belongs to
/// `skeleton-anim.md`).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Transform {
    /// Translation x, y, z.
    pub translation: [f32; 3],
    /// Rotation quaternion x, y, z, w.
    pub rotation: [f32; 4],
    /// Scale x, y, z.
    pub scale: [f32; 3],
}

impl Transform {
    /// The identity transform: no translation, identity rotation, unit scale.
    pub const IDENTITY: Transform = Transform {
        translation: [0.0; 3],
        rotation: [0.0, 0.0, 0.0, 1.0],
        scale: [1.0; 3],
    };

    /// Builds a transform from one 12-float element (§4.2): floats 0–2 translation, 4–7
    /// rotation, 8–10 scale; floats 3 and 11 are padding and ignored.
    pub fn from_vec12(v: &[f32; 12]) -> Self {
        Transform {
            translation: [v[0], v[1], v[2]],
            rotation: [v[4], v[5], v[6], v[7]],
            scale: [v[8], v[9], v[10]],
        }
    }

    /// The 12-float element of this transform, with zero padding.
    pub fn to_vec12(&self) -> [f32; 12] {
        let [tx, ty, tz] = self.translation;
        let [rx, ry, rz, rw] = self.rotation;
        let [sx, sy, sz] = self.scale;
        [tx, ty, tz, 0.0, rx, ry, rz, rw, sx, sy, sz, 0.0]
    }

    /// Whether every component is finite.
    pub fn is_finite(&self) -> bool {
        self.translation
            .iter()
            .chain(&self.rotation)
            .chain(&self.scale)
            .all(|v| v.is_finite())
    }
}

impl Default for Transform {
    fn default() -> Self {
        Self::IDENTITY
    }
}

/// Dot product of two quaternions.
pub fn quat_dot(a: [f32; 4], b: [f32; 4]) -> f32 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2] + a[3] * b[3]
}

/// Length of a quaternion.
pub fn quat_length(q: [f32; 4]) -> f32 {
    quat_dot(q, q).sqrt()
}

/// Normalised linear interpolation from `a` to `b` by `alpha`, after aligning `b`'s sign to
/// `a` (§7.8). Falls back to the nearer endpoint when the blend is degenerate.
pub fn quat_nlerp(a: [f32; 4], b: [f32; 4], alpha: f32) -> [f32; 4] {
    let b = if quat_dot(a, b) < 0.0 {
        [-b[0], -b[1], -b[2], -b[3]]
    } else {
        b
    };
    let mut q = [0.0f32; 4];
    for i in 0..4 {
        q[i] = a[i] + (b[i] - a[i]) * alpha;
    }
    let len = quat_length(q);
    if len.is_finite() && len >= 1e-6 {
        q.map(|c| c / len)
    } else if alpha < 0.5 {
        a
    } else {
        b
    }
}
