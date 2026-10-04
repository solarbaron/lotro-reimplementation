// SPDX-License-Identifier: GPL-3.0-or-later
//! Camera-relative rendering frame and visibility tests.
//!
//! Positions are kept as f64 region-global coordinates (`coordinates.md` §2) and turned
//! into small f32 values relative to an **anchor** landblock's south-west corner
//! (`coordinates.md` §12) only for rendering. The anchor follows the viewer one landblock
//! at a time.

use tessera_render::glam::{Mat4, Vec3, Vec4};
use tessera_world::coords::MAX_BLOCK;
use tessera_world::{LANDBLOCK_SIZE, LandblockKey, WorldPos};

/// The render frame's origin: an anchor landblock of one region.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Anchor {
    /// Region.
    pub region: u8,
    /// Anchor landblock X.
    pub x: u8,
    /// Anchor landblock Y.
    pub y: u8,
}

impl Anchor {
    /// The anchor whose block contains `pos` (clamped to the grid).
    #[must_use]
    pub fn containing(pos: &WorldPos) -> Self {
        let idx = |g: f64| -> u8 {
            if g.is_finite() {
                (g / LANDBLOCK_SIZE)
                    .floor()
                    .clamp(0.0, f64::from(MAX_BLOCK)) as u8
            } else {
                0
            }
        };
        Self {
            region: pos.region,
            x: idx(pos.x),
            y: idx(pos.y),
        }
    }

    /// Region-global origin of the anchor block.
    #[must_use]
    pub fn origin(&self) -> [f64; 2] {
        [
            f64::from(self.x) * LANDBLOCK_SIZE,
            f64::from(self.y) * LANDBLOCK_SIZE,
        ]
    }

    /// Translation of landblock `key`'s local frame in the render frame (`None` across
    /// regions).
    #[must_use]
    pub fn offset_of(&self, key: LandblockKey) -> Option<Vec3> {
        if key.region() != self.region {
            return None;
        }
        let d = |a: u8, b: u8| (f32::from(a) - f32::from(b)) * LANDBLOCK_SIZE as f32;
        Some(Vec3::new(d(key.x(), self.x), d(key.y(), self.y), 0.0))
    }

    /// `pos` in the render frame (`None` across regions or for non-finite input).
    #[must_use]
    pub fn to_render(self, pos: &WorldPos) -> Option<Vec3> {
        if pos.region != self.region || !pos.is_finite() {
            return None;
        }
        let [ox, oy] = self.origin();
        Some(Vec3::new(
            (pos.x - ox) as f32,
            (pos.y - oy) as f32,
            pos.z as f32,
        ))
    }
}

/// Six clip planes of a view-projection matrix (`a·x + b·y + c·z + d ≥ 0` inside).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Frustum {
    planes: [Vec4; 6],
}

impl Frustum {
    /// Planes of `view_proj` (column vectors, clip z in 0..1 either way round).
    #[must_use]
    pub fn from_view_proj(m: Mat4) -> Self {
        let (r0, r1, r2, r3) = (m.row(0), m.row(1), m.row(2), m.row(3));
        let mut planes = [r3 + r0, r3 - r0, r3 + r1, r3 - r1, r2, r3 - r2];
        for p in &mut planes {
            let n = p.truncate().length();
            if n.is_finite() && n > 0.0 {
                *p /= n;
            }
        }
        Self { planes }
    }

    /// Whether a sphere may be visible (conservative; non-finite input counts as visible).
    #[must_use]
    pub fn sphere_visible(&self, center: Vec3, radius: f32) -> bool {
        if !center.is_finite() || !radius.is_finite() {
            return true;
        }
        self.planes
            .iter()
            .all(|p| p.truncate().dot(center) + p.w >= -radius)
    }

    /// Whether an axis-aligned box may be visible (conservative).
    #[must_use]
    pub fn aabb_visible(&self, min: Vec3, max: Vec3) -> bool {
        if !min.is_finite() || !max.is_finite() {
            return true;
        }
        self.planes.iter().all(|p| {
            let n = p.truncate();
            let v = Vec3::new(
                if n.x >= 0.0 { max.x } else { min.x },
                if n.y >= 0.0 { max.y } else { min.y },
                if n.z >= 0.0 { max.z } else { min.z },
            );
            n.dot(v) + p.w >= 0.0
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tessera_render::{Camera, Perspective};

    #[test]
    fn anchor_round_trip() {
        let p = WorldPos::new(1, 16_040.5, 24_080.25, 399.99);
        let a = Anchor::containing(&p);
        assert_eq!((a.x, a.y), (100, 150));
        let r = a.to_render(&p).unwrap();
        assert!((r.x - 40.5).abs() < 1e-4 && (r.y - 80.25).abs() < 1e-4);
        let k = LandblockKey::new(1, 101, 149).unwrap();
        assert_eq!(a.offset_of(k), Some(Vec3::new(160.0, -160.0, 0.0)));
        assert_eq!(a.offset_of(LandblockKey::new(2, 1, 1).unwrap()), None);
        assert_eq!(
            Anchor::containing(&WorldPos::new(1, f64::NAN, -5.0, 0.0)),
            Anchor {
                region: 1,
                x: 0,
                y: 0
            }
        );
    }

    #[test]
    fn frustum_culls_behind_and_keeps_ahead() {
        let cam = Camera {
            eye: Vec3::ZERO,
            target: Vec3::X,
            up: Vec3::Z,
            projection: Perspective {
                fov_y: 1.0,
                near: 0.5,
                far: 1000.0,
            },
        };
        let f = Frustum::from_view_proj(cam.view_proj(1.5));
        assert!(f.sphere_visible(Vec3::new(50.0, 0.0, 0.0), 1.0));
        assert!(!f.sphere_visible(Vec3::new(-50.0, 0.0, 0.0), 1.0));
        assert!(!f.sphere_visible(Vec3::new(5000.0, 0.0, 0.0), 1.0));
        assert!(f.aabb_visible(Vec3::new(10.0, -1.0, -1.0), Vec3::new(12.0, 1.0, 1.0)));
        assert!(!f.aabb_visible(Vec3::new(-12.0, -1.0, -1.0), Vec3::new(-10.0, 1.0, 1.0)));
        assert!(f.sphere_visible(Vec3::NAN, 1.0));
    }
}
