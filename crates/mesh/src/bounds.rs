// SPDX-License-Identifier: GPL-3.0-or-later
//! Bounding volumes (`mesh.md` §6.1).

/// An axis-aligned box in object space (Z-up, metres, `mesh.md` §11).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Aabb {
    /// Minimum corner.
    pub min: [f32; 3],
    /// Maximum corner.
    pub max: [f32; 3],
}

impl Aabb {
    /// The box around `points`, skipping points with a non-finite coordinate. `None` when
    /// no point is finite.
    #[must_use]
    pub fn from_points<'a>(points: impl IntoIterator<Item = &'a [f32; 3]>) -> Option<Self> {
        let mut out: Option<Self> = None;
        for p in points {
            if p.iter().all(|c| c.is_finite()) {
                match &mut out {
                    None => out = Some(Self { min: *p, max: *p }),
                    Some(b) => b.include(*p),
                }
            }
        }
        out
    }

    /// Grows the box to include `p`.
    pub fn include(&mut self, p: [f32; 3]) {
        for (i, &c) in p.iter().enumerate() {
            self.min[i] = self.min[i].min(c);
            self.max[i] = self.max[i].max(c);
        }
    }

    /// The smallest box containing both.
    #[must_use]
    pub fn union(&self, other: &Self) -> Self {
        let mut out = *self;
        out.include(other.min);
        out.include(other.max);
        out
    }

    /// Centre point.
    #[must_use]
    pub fn center(&self) -> [f32; 3] {
        [0, 1, 2].map(|i| (self.min[i] + self.max[i]) * 0.5)
    }

    /// Edge lengths.
    #[must_use]
    pub fn extent(&self) -> [f32; 3] {
        [0, 1, 2].map(|i| self.max[i] - self.min[i])
    }

    /// Whether every coordinate is finite and `min ≤ max` on every axis.
    #[must_use]
    pub fn is_valid(&self) -> bool {
        (0..3).all(|i| {
            self.min[i].is_finite() && self.max[i].is_finite() && self.min[i] <= self.max[i]
        })
    }

    /// Whether the two boxes are identical bit for bit (the MI4 comparison).
    #[must_use]
    pub fn bit_eq(&self, other: &Self) -> bool {
        self.min
            .iter()
            .chain(&self.max)
            .zip(other.min.iter().chain(&other.max))
            .all(|(a, b)| a.to_bits() == b.to_bits())
    }
}

/// A sphere containing a set of points.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct BoundingSphere {
    /// Centre.
    pub center: [f32; 3],
    /// Radius (≥ 0).
    pub radius: f32,
}

impl BoundingSphere {
    /// A sphere around `points` centred on `aabb`'s centre, with the largest distance from
    /// it as radius (rounded up slightly so every point is inside despite f32 rounding).
    /// Non-finite points are skipped.
    #[must_use]
    pub fn around<'a>(aabb: &Aabb, points: impl IntoIterator<Item = &'a [f32; 3]>) -> Self {
        let c = aabb.center().map(f64::from);
        let mut r2 = 0f64;
        for p in points {
            if p.iter().all(|v| v.is_finite()) {
                let d: f64 = (0..3).map(|i| (f64::from(p[i]) - c[i]).powi(2)).sum();
                r2 = r2.max(d);
            }
        }
        let r = r2.sqrt();
        Self {
            center: aabb.center(),
            radius: (r * (1.0 + 1e-6) + 1e-6) as f32,
        }
    }
}

/// The bounds of one detail level (or of the whole mesh, level 0).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Bounds {
    /// Box around every finite position drawn by the level.
    pub aabb: Aabb,
    /// Sphere around the same positions.
    pub sphere: BoundingSphere,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn box_and_sphere() {
        let pts = [
            [0.0, 0.0, 0.0],
            [2.0, 4.0, -2.0],
            [f32::NAN, 9.0, 9.0],
            [1.0, 1.0, 1.0],
        ];
        let b = Aabb::from_points(&pts).unwrap();
        assert_eq!(b.min, [0.0, 0.0, -2.0]);
        assert_eq!(b.max, [2.0, 4.0, 1.0]);
        assert_eq!(b.center(), [1.0, 2.0, -0.5]);
        assert!(b.is_valid());
        let s = BoundingSphere::around(&b, &pts);
        for p in &pts[..2] {
            let d: f32 = (0..3)
                .map(|i| (p[i] - s.center[i]).powi(2))
                .sum::<f32>()
                .sqrt();
            assert!(d <= s.radius);
        }
        assert!(Aabb::from_points(&[[f32::INFINITY, 0.0, 0.0]]).is_none());
        let u = b.union(&Aabb {
            min: [-1.0; 3],
            max: [0.0; 3],
        });
        assert_eq!(u.min, [-1.0, -1.0, -2.0]);
        assert!(b.bit_eq(&b));
        assert!(!b.bit_eq(&u));
    }
}
