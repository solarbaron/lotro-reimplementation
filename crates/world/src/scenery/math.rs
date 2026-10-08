// SPDX-License-Identifier: GPL-3.0-or-later
//! Rotations of `scenery.md` §7.4 step 10 in the quaternion convention of
//! `coordinates.md` §11.1–§11.2: (w, x, y, z), active, Hamilton product. Computed in f32.

use tessera_formats::setup::Quaternion;

/// The 0.0002 tolerance of `scenery.md` §7.4 (alignment special cases, plane heights).
pub const EPS: f32 = 0.0002;

/// Hamilton product a · b (apply b first, then a).
#[must_use]
pub fn mul(a: Quaternion, b: Quaternion) -> Quaternion {
    Quaternion {
        w: a.w * b.w - a.x * b.x - a.y * b.y - a.z * b.z,
        x: a.w * b.x + a.x * b.w + a.y * b.z - a.z * b.y,
        y: a.w * b.y - a.x * b.z + a.y * b.w + a.z * b.x,
        z: a.w * b.z + a.x * b.y - a.y * b.x + a.z * b.w,
    }
}

/// Right-handed rotation by `deg` degrees about +Z.
#[must_use]
pub fn rot_z(deg: f32) -> Quaternion {
    let (s, c) = (deg.to_radians() * 0.5).sin_cos();
    Quaternion {
        w: c,
        x: 0.0,
        y: 0.0,
        z: s,
    }
}

/// Right-handed rotation by `deg` degrees about +X.
#[must_use]
pub fn rot_x(deg: f32) -> Quaternion {
    let (s, c) = (deg.to_radians() * 0.5).sin_cos();
    Quaternion {
        w: c,
        x: s,
        y: 0.0,
        z: 0.0,
    }
}

/// Right-handed rotation by `deg` degrees about +Y.
#[must_use]
pub fn rot_y(deg: f32) -> Quaternion {
    let (s, c) = (deg.to_radians() * 0.5).sin_cos();
    Quaternion {
        w: c,
        x: 0.0,
        y: s,
        z: 0.0,
    }
}

/// The rotation whose matrix has the columns `cx`, `cy`, `cz` (the images of model X, Y
/// and Z), assumed orthonormal and right-handed. The result has w ≥ 0 where the matrix
/// allows.
#[must_use]
pub fn from_columns(cx: [f32; 3], cy: [f32; 3], cz: [f32; 3]) -> Quaternion {
    let (m00, m01, m02) = (cx[0], cy[0], cz[0]);
    let (m10, m11, m12) = (cx[1], cy[1], cz[1]);
    let (m20, m21, m22) = (cx[2], cy[2], cz[2]);
    let trace = m00 + m11 + m22;
    if trace > 0.0 {
        let s = (trace + 1.0).sqrt() * 2.0;
        Quaternion {
            w: 0.25 * s,
            x: (m21 - m12) / s,
            y: (m02 - m20) / s,
            z: (m10 - m01) / s,
        }
    } else if m00 > m11 && m00 > m22 {
        let s = (1.0 + m00 - m11 - m22).max(0.0).sqrt() * 2.0;
        let q = Quaternion {
            w: (m21 - m12) / s,
            x: 0.25 * s,
            y: (m01 + m10) / s,
            z: (m02 + m20) / s,
        };
        positive_w(q)
    } else if m11 > m22 {
        let s = (1.0 + m11 - m00 - m22).max(0.0).sqrt() * 2.0;
        let q = Quaternion {
            w: (m02 - m20) / s,
            x: (m01 + m10) / s,
            y: 0.25 * s,
            z: (m12 + m21) / s,
        };
        positive_w(q)
    } else {
        let s = (1.0 + m22 - m00 - m11).max(0.0).sqrt() * 2.0;
        let q = Quaternion {
            w: (m10 - m01) / s,
            x: (m02 + m20) / s,
            y: (m12 + m21) / s,
            z: 0.25 * s,
        };
        positive_w(q)
    }
}

fn positive_w(q: Quaternion) -> Quaternion {
    if q.w < 0.0 {
        Quaternion {
            w: -q.w,
            x: -q.x,
            y: -q.y,
            z: -q.z,
        }
    } else {
        q
    }
}

/// Q_align(n) of `scenery.md` §7.4 step 10: model Z goes to the unit normal `n`, model X
/// to world +X projected onto the slope. The identity when |n.x| and |n.y| are both below
/// 0.0002; the vertical-plane frame when |n.z| ≤ 0.0002.
#[must_use]
pub fn align_to_normal(n: [f32; 3]) -> Quaternion {
    let [nx, ny, nz] = n;
    if nx.abs() < EPS && ny.abs() < EPS {
        return Quaternion::IDENTITY;
    }
    if nz.abs() <= EPS {
        let l = (nx * nx + ny * ny).sqrt();
        let cx = [-ny / l, nx / l, 0.0];
        let cy = [0.0, 0.0, 1.0];
        let cz = [nx / l, ny / l, 0.0];
        return from_columns(cx, cy, cz);
    }
    let l = (ny * ny + nz * nz).sqrt();
    let cx = [l, -nx * ny / l, -nx * nz / l];
    let cy = [0.0, nz / l, -ny / l];
    from_columns(cx, cy, n)
}

/// The compass bearing in degrees, [0, 360), of a plane normal's horizontal part:
/// (450° − atan2(n.y, n.x)) mod 360 (`scenery.md` §7.4 step 13b).
#[must_use]
pub fn aspect_degrees(n: [f32; 3]) -> f32 {
    let a = (450.0 - n[1].atan2(n[0]).to_degrees()).rem_euclid(360.0);
    if a >= 360.0 { 0.0 } else { a }
}

/// The difference of two bearings in degrees, wrapped to [0, 180].
#[must_use]
pub fn bearing_difference(a: f32, b: f32) -> f32 {
    let d = (a - b).rem_euclid(360.0);
    if d > 180.0 { 360.0 - d } else { d }
}

/// Whether `q` has unit length within `tol`.
#[must_use]
pub fn is_unit(q: Quaternion, tol: f32) -> bool {
    (q.length() - 1.0).abs() <= tol
}
