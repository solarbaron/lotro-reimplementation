// SPDX-License-Identifier: GPL-3.0-or-later
//! Geometric primitives shared by every part of the crate: boxes, planes, spheres,
//! capsules and placement transforms. Coordinates are metres, Z up, right-handed
//! (`collision.md` §2, `mesh.md` §11).

use tessera_formats::setup::Quaternion;

use super::math::{self, d3, f3};

/// An axis-aligned box. A valid box has finite corners with `min ≤ max` on every axis.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Aabb {
    /// Minimum corner.
    pub min: [f32; 3],
    /// Maximum corner.
    pub max: [f32; 3],
}

impl Aabb {
    /// The empty box (inverted infinities); [`Aabb::grow`] and [`Aabb::union`] start from
    /// it.
    pub const EMPTY: Self = Self {
        min: [f32::INFINITY; 3],
        max: [f32::NEG_INFINITY; 3],
    };

    /// The box spanning `points`; `None` when there are none or one is not finite.
    #[must_use]
    pub fn from_points(points: impl IntoIterator<Item = [f32; 3]>) -> Option<Self> {
        let mut b = Self::EMPTY;
        let mut any = false;
        for p in points {
            if !math::finite3(p) {
                return None;
            }
            b.grow(p);
            any = true;
        }
        any.then_some(b)
    }

    /// Extends the box to contain `p`.
    pub fn grow(&mut self, p: [f32; 3]) {
        self.min = math::min3(self.min, p);
        self.max = math::max3(self.max, p);
    }

    /// The smallest box containing both.
    #[must_use]
    pub fn union(self, other: Self) -> Self {
        Self {
            min: math::min3(self.min, other.min),
            max: math::max3(self.max, other.max),
        }
    }

    /// The box grown by `r` on every side.
    #[must_use]
    pub fn expanded(self, r: f32) -> Self {
        Self {
            min: self.min.map(|c| c - r),
            max: self.max.map(|c| c + r),
        }
    }

    /// Whether the corners are finite and ordered.
    #[must_use]
    pub fn is_valid(&self) -> bool {
        math::finite3(self.min)
            && math::finite3(self.max)
            && (0..3).all(|a| self.min[a] <= self.max[a])
    }

    /// Whether the two boxes overlap (touching counts).
    #[must_use]
    pub fn overlaps(&self, other: &Self) -> bool {
        (0..3).all(|a| self.min[a] <= other.max[a] && other.min[a] <= self.max[a])
    }

    /// Whether `p` lies inside or on the box.
    #[must_use]
    pub fn contains(&self, p: [f32; 3]) -> bool {
        (0..3).all(|a| self.min[a] <= p[a] && p[a] <= self.max[a])
    }

    /// Centre of the box.
    #[must_use]
    pub fn centre(&self) -> [f32; 3] {
        std::array::from_fn(|a| 0.5 * self.min[a] + 0.5 * self.max[a])
    }

    /// Half the surface area (the SAH cost measure); 0 for an invalid box.
    #[must_use]
    pub fn half_area(&self) -> f32 {
        if !self.is_valid() {
            return 0.0;
        }
        let e: [f32; 3] = std::array::from_fn(|a| self.max[a] - self.min[a]);
        e[0] * e[1] + e[1] * e[2] + e[2] * e[0]
    }
}

/// A plane n · p + d = 0 with a unit normal `normal` (`collision.md` §6.6: the plane of a
/// terrain triangle has n.z > 0; a ceiling triangle's faces down).
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct Plane {
    /// Unit normal (all zero for "no plane").
    pub normal: [f32; 3],
    /// Offset: n · p + d = 0 on the plane.
    pub d: f32,
}

impl Plane {
    /// The plane n = (0, 0, 0), d = 0 that the client uses for "no triangle" (`scenery.md`
    /// §7.4 step 8).
    pub const NONE: Self = Self {
        normal: [0.0; 3],
        d: 0.0,
    };

    /// The plane through `a`, `b`, `c` with the normal of their counter-clockwise winding
    /// ((b − a) × (c − a), normalised), offset from `a`. `None` for a degenerate or
    /// non-finite triangle.
    #[must_use]
    pub fn from_triangle(a: [f32; 3], b: [f32; 3], c: [f32; 3]) -> Option<Self> {
        let (a, b, c) = (d3(a), d3(b), d3(c));
        let n = math::normalize(math::cross(math::sub(b, a), math::sub(c, a)))?;
        let d = -math::dot(n, a);
        let normal = f3(n);
        #[allow(clippy::cast_possible_truncation)]
        let d = d as f32;
        (math::finite3(normal) && d.is_finite()).then_some(Self { normal, d })
    }

    /// Whether this is [`Plane::NONE`] (zero normal).
    #[must_use]
    pub fn is_none(&self) -> bool {
        self.normal == [0.0; 3]
    }

    /// n · p + d.
    #[must_use]
    pub fn signed_distance(&self, p: [f32; 3]) -> f32 {
        let n = self.normal;
        n[0] * p[0] + n[1] * p[1] + n[2] * p[2] + self.d
    }

    /// The plane's height at (x, y): −(n.x·x + n.y·y + d) / n.z, or `None` when
    /// |n.z| ≤ 0.0002 (a vertical or absent plane; the threshold of `scenery.md` §7.4
    /// step 11).
    #[must_use]
    pub fn height_at(&self, x: f32, y: f32) -> Option<f32> {
        let n = self.normal;
        (n[2].abs() > 0.0002).then(|| -(n[0] * x + n[1] * y + self.d) / n[2])
    }
}

/// A sphere.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Sphere {
    /// Centre.
    pub centre: [f32; 3],
    /// Radius (≥ 0).
    pub radius: f32,
}

impl Sphere {
    /// Bounds; `None` for non-finite or out-of-range values (beyond
    /// [`super::MAX_COORDINATE`]) or a negative radius.
    #[must_use]
    pub fn aabb(&self) -> Option<Aabb> {
        (math::bounded3(self.centre) && math::bounded_len(self.radius)).then(|| Aabb {
            min: self.centre.map(|c| c - self.radius),
            max: self.centre.map(|c| c + self.radius),
        })
    }
}

/// A capsule: every point within `radius` of the segment `a`–`b`. A capsule with `a = b`
/// is a sphere.
///
/// A setup's capsule (`setup.md` §6.4) is vertical in the setup frame: `a` is the lower cap
/// centre and `b = a + (0, 0, length)` ([`Capsule::vertical`]); placed with a rotated
/// transform it may tilt.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Capsule {
    /// One end of the core segment.
    pub a: [f32; 3],
    /// The other end.
    pub b: [f32; 3],
    /// Radius (≥ 0).
    pub radius: f32,
}

impl Capsule {
    /// A vertical capsule with lower cap centre `base`, cap-centre distance `length` and
    /// `radius` (the layout of `setup.md` §6.4 and `collision.md` §10.6).
    #[must_use]
    pub fn vertical(base: [f32; 3], length: f32, radius: f32) -> Self {
        Self {
            a: base,
            b: [base[0], base[1], base[2] + length],
            radius,
        }
    }

    /// Whether the values are finite and within ±[`super::MAX_COORDINATE`] and the
    /// radius is non-negative (and below that bound).
    #[must_use]
    pub fn is_valid(&self) -> bool {
        math::bounded3(self.a) && math::bounded3(self.b) && math::bounded_len(self.radius)
    }

    /// Bounds; `None` when [`Capsule::is_valid`] fails.
    #[must_use]
    pub fn aabb(&self) -> Option<Aabb> {
        if !self.is_valid() {
            return None;
        }
        Some(Aabb {
            min: math::min3(self.a, self.b).map(|c| c - self.radius),
            max: math::max3(self.a, self.b).map(|c| c + self.radius),
        })
    }

    /// The capsule moved by `delta`.
    #[must_use]
    pub fn translated(&self, delta: [f32; 3]) -> Self {
        let add = |p: [f32; 3]| std::array::from_fn(|i| p[i] + delta[i]);
        Self {
            a: add(self.a),
            b: add(self.b),
            radius: self.radius,
        }
    }
}

/// How an object is placed: per-axis scale, then an active rotation, then a translation
/// (`entity-descriptor.md` §5, §7; the same order as
/// [`tessera_formats::landblock::Placement::transform`]).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Transform {
    /// Translation (landblock frame for statics and generated objects).
    pub position: [f32; 3],
    /// Unit rotation, (w, x, y, z), applied as `v' = q v q*`.
    pub rotation: Quaternion,
    /// Per-axis scale (`Physics_AdjustableScale`; uniform in all observed data).
    pub scale: [f32; 3],
}

impl Default for Transform {
    fn default() -> Self {
        Self::IDENTITY
    }
}

impl Transform {
    /// Position 0, identity rotation, scale 1.
    pub const IDENTITY: Self = Self {
        position: [0.0; 3],
        rotation: Quaternion::IDENTITY,
        scale: [1.0; 3],
    };

    /// A transform with a uniform scale.
    #[must_use]
    pub fn new(position: [f32; 3], rotation: Quaternion, scale: f32) -> Self {
        Self {
            position,
            rotation,
            scale: [scale; 3],
        }
    }

    /// The transform with its rotation normalised, or `None` when a value is not finite or
    /// beyond ±[`super::MAX_COORDINATE`], the rotation has zero length or a scale component
    /// is zero or negative.
    #[must_use]
    pub fn validated(&self) -> Option<Self> {
        if !math::bounded3(self.position)
            || !math::bounded3(self.scale)
            || self.scale.iter().any(|s| *s <= 0.0)
        {
            return None;
        }
        let rotation = self.rotation.normalized()?;
        rotation.is_finite().then_some(Self { rotation, ..*self })
    }

    /// Places `v`: scale per axis, rotate, translate.
    #[must_use]
    pub fn apply(&self, v: [f32; 3]) -> [f32; 3] {
        let s = [
            v[0] * self.scale[0],
            v[1] * self.scale[1],
            v[2] * self.scale[2],
        ];
        let r = self.rotation.rotate(s);
        [
            r[0] + self.position[0],
            r[1] + self.position[1],
            r[2] + self.position[2],
        ]
    }

    /// Whether the three scale components are equal.
    #[must_use]
    pub fn is_uniform(&self) -> bool {
        self.scale[0] == self.scale[1] && self.scale[1] == self.scale[2]
    }

    /// The largest scale component (the radius factor for spheres and capsules).
    #[must_use]
    pub fn max_scale(&self) -> f32 {
        self.scale[0].max(self.scale[1]).max(self.scale[2])
    }
}
