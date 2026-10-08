// SPDX-License-Identifier: GPL-3.0-or-later
//! Collision shapes of objects in their own frame, and their placed copies.
//!
//! A [`ShapeTemplate`] is what a setup collides with (`collision.md` §4, §8): the triangles
//! of its 0x66 collision mesh (setup frame, two-sided, §4.2), its boxes (as 12 triangles
//! each, placed by the box frame, `setup.md` §6.4, and as solid volumes, [`BoxVolume`]),
//! and its spheres and vertical capsules.
//! [`ShapeTemplate::from_triangle_list`] builds one from any triangle list, for example a
//! cell's physics mesh (§5.3) or a render mesh level's collision mesh (`mesh.md` §9,
//! `collision.md` §7; such shapes are for picking and the camera, not for movement).
//!
//! Placing a template ([`ShapeTemplate::place`]) applies a [`Transform`]: scale per axis,
//! rotate, translate. Triangles transform exactly under any scale. Spheres and capsules
//! stay round: their centres transform exactly and their radius is multiplied by the
//! largest scale component (exact for the uniform scales of all observed data; a
//! conservative over-estimate otherwise, Tessera Decision).

use tessera_formats::Setup;
use tessera_formats::collision::CollisionMesh;
use tessera_formats::setup::{CollisionBox, Frame};

use super::math;
use super::primitives::{Aabb, Capsule, Sphere, Transform};
use crate::error::{PhysicsError, Result};

/// Most triangles one template may hold (0x66 meshes have up to 7,918; mesh levels up to
/// about 96,000).
pub const MAX_TEMPLATE_TRIANGLES: usize = 1 << 20;

/// Most spheres plus capsules one template may hold.
pub const MAX_TEMPLATE_PRIMITIVES: usize = 1 << 16;

/// The 12 triangles of a box, as corner numbers (bit 0: x max, bit 1: y max, bit 2: z max),
/// counter-clockwise seen from outside.
const BOX_TRIANGLES: [[usize; 3]; 12] = [
    [0, 2, 3],
    [0, 3, 1], // -z
    [4, 5, 7],
    [4, 7, 6], // +z
    [0, 1, 5],
    [0, 5, 4], // -y
    [2, 6, 7],
    [2, 7, 3], // +y
    [0, 4, 6],
    [0, 6, 2], // -x
    [1, 3, 7],
    [1, 7, 5], // +x
];

/// A box volume of a template: its 8 corners (corner *k*: bit 0 x max, bit 1 y max, bit 2
/// z max) and where its 12 triangles start in [`ShapeTemplate::triangles`]. Boxes are
/// solid (`collision.md` §8.2, BIN; Tessera's representation, §8.3 Decision): a capsule
/// whose core is inside one is
/// reported as penetrating it and pushed out through its nearest face, like a sphere or
/// capsule volume, instead of being held by the inner faces of its triangles.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct BoxVolume {
    /// The corners, in the template's frame.
    pub corners: [[f32; 3]; 8],
    /// Index of the box's first triangle (its 12 follow in the face order −z, +z, −y, +y,
    /// −x, +x, two per face).
    pub first_triangle: usize,
}

/// An object's collision in its own frame.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct ShapeTemplate {
    /// Triangles: the mesh's first (in its stored order, so triangle *k* is the mesh's
    /// triangle *k*), then 12 per box.
    pub triangles: Vec<[[f32; 3]; 3]>,
    /// How many of `triangles` came from the mesh.
    pub mesh_triangles: usize,
    /// Spheres.
    pub spheres: Vec<Sphere>,
    /// Capsules.
    pub capsules: Vec<Capsule>,
    /// The boxes among `triangles`, as solids.
    pub boxes: Vec<BoxVolume>,
    /// Elements dropped because a value was not finite or a box was inverted.
    pub dropped: usize,
}

impl ShapeTemplate {
    /// The shape of `setup` (`collision.md` §8): `mesh` (the setup's 0x66 record, when it
    /// names one and it was read) plus its boxes, spheres and capsules. Non-finite or
    /// inverted elements are dropped and counted.
    ///
    /// # Errors
    ///
    /// [`PhysicsError::TooLarge`] beyond [`MAX_TEMPLATE_TRIANGLES`] or
    /// [`MAX_TEMPLATE_PRIMITIVES`].
    pub fn from_setup(setup: &Setup, mesh: Option<&CollisionMesh>) -> Result<Self> {
        let mut t = match mesh {
            Some(m) => Self::from_collision_mesh(m)?,
            None => Self::default(),
        };
        for b in &setup.boxes {
            t.push_box(b)?;
        }
        check(
            "template primitives",
            setup.spheres.len() + setup.capsules.len(),
            MAX_TEMPLATE_PRIMITIVES,
        )?;
        for s in &setup.spheres {
            let s = Sphere {
                centre: s.centre,
                radius: s.radius,
            };
            if s.aabb().is_some() {
                t.spheres.push(s);
            } else {
                t.dropped += 1;
            }
        }
        for c in &setup.capsules {
            let c = Capsule::vertical(c.base, c.length, c.radius);
            if c.is_valid() && c.b[2] >= c.a[2] {
                t.capsules.push(c);
            } else {
                t.dropped += 1;
            }
        }
        Ok(t)
    }

    /// The triangles of a 0x66 record or a cell's physics mesh (`collision.md` §4.1,
    /// §5.3), in stored order. A triangle with an index out of range or a non-finite
    /// corner is replaced by nothing and counted in `dropped` (decoded records have
    /// neither), so triangle numbers after it shift; use
    /// [`ShapeTemplate::from_triangle_list`] directly to keep numbering with gaps.
    ///
    /// # Errors
    ///
    /// [`PhysicsError::TooLarge`].
    pub fn from_collision_mesh(mesh: &CollisionMesh) -> Result<Self> {
        Self::from_triangle_list(&mesh.vertices, &mesh.triangles)
    }

    /// A template from positions and index triples (two-sided). Out-of-range indices and
    /// non-finite corners drop the triangle (counted in `dropped`).
    ///
    /// # Errors
    ///
    /// [`PhysicsError::TooLarge`].
    pub fn from_triangle_list(positions: &[[f32; 3]], triangles: &[[u16; 3]]) -> Result<Self> {
        check(
            "template triangles",
            triangles.len(),
            MAX_TEMPLATE_TRIANGLES,
        )?;
        let mut t = Self {
            triangles: Vec::with_capacity(triangles.len()),
            ..Self::default()
        };
        for tri in triangles {
            let corners = tri.map(|i| positions.get(usize::from(i)).copied());
            match corners {
                [Some(a), Some(b), Some(c)]
                    if math::bounded3(a) && math::bounded3(b) && math::bounded3(c) =>
                {
                    t.triangles.push([a, b, c]);
                }
                _ => t.dropped += 1,
            }
        }
        t.mesh_triangles = t.triangles.len();
        Ok(t)
    }

    /// A template from a render mesh level's collision mesh (`mesh.md` §9): `positions`
    /// are the resolved collision-vertex positions (`None` where a reference does not
    /// resolve) and `triangles` the collision triangles' vertex triples. For picking and
    /// camera queries only (`collision.md` §7).
    ///
    /// # Errors
    ///
    /// [`PhysicsError::TooLarge`].
    pub fn from_level_collision(
        positions: &[Option<[f32; 3]>],
        triangles: &[[u16; 3]],
    ) -> Result<Self> {
        check(
            "template triangles",
            triangles.len(),
            MAX_TEMPLATE_TRIANGLES,
        )?;
        let mut t = Self {
            triangles: Vec::with_capacity(triangles.len()),
            ..Self::default()
        };
        for tri in triangles {
            let corners = tri.map(|i| positions.get(usize::from(i)).copied().flatten());
            match corners {
                [Some(a), Some(b), Some(c)]
                    if math::bounded3(a) && math::bounded3(b) && math::bounded3(c) =>
                {
                    t.triangles.push([a, b, c]);
                }
                _ => t.dropped += 1,
            }
        }
        t.mesh_triangles = t.triangles.len();
        Ok(t)
    }

    /// Adds a box as 12 triangles: local corners, scaled by the box frame's scale per axis
    /// (always 1 in the data), rotated by its rotation, moved by its position (`setup.md`
    /// §6.4). An inverted or non-finite box is dropped.
    ///
    /// # Errors
    ///
    /// [`PhysicsError::TooLarge`].
    pub fn push_box(&mut self, b: &CollisionBox) -> Result<()> {
        check(
            "template triangles",
            self.triangles.len() + 12,
            MAX_TEMPLATE_TRIANGLES,
        )?;
        let Some(corners) = box_corners(b.min, b.max, &b.frame) else {
            self.dropped += 1;
            return Ok(());
        };
        self.boxes.push(BoxVolume {
            corners,
            first_triangle: self.triangles.len(),
        });
        for t in BOX_TRIANGLES {
            self.triangles.push(t.map(|k| corners[k]));
        }
        Ok(())
    }

    /// Whether the template holds nothing.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.triangles.is_empty() && self.spheres.is_empty() && self.capsules.is_empty()
    }

    /// Bounds in the template's frame; `None` when empty.
    #[must_use]
    pub fn bounds(&self) -> Option<Aabb> {
        let mut b: Option<Aabb> = None;
        let mut add = |x: Aabb| b = Some(b.map_or(x, |o| o.union(x)));
        for t in &self.triangles {
            if let Some(x) = Aabb::from_points(t.iter().copied()) {
                add(x);
            }
        }
        for s in &self.spheres {
            if let Some(x) = s.aabb() {
                add(x);
            }
        }
        for c in &self.capsules {
            if let Some(x) = c.aabb() {
                add(x);
            }
        }
        b
    }

    /// The template placed by `transform` (module docs). Elements that become non-finite
    /// are dropped. `InvalidInput` for an invalid transform (non-finite, zero rotation,
    /// non-positive scale).
    ///
    /// # Errors
    ///
    /// [`PhysicsError::InvalidInput`].
    pub fn place(&self, transform: &Transform) -> Result<Self> {
        let t = transform
            .validated()
            .ok_or(PhysicsError::InvalidInput("object transform"))?;
        let r = t.max_scale();
        let mut out = Self {
            triangles: Vec::with_capacity(self.triangles.len()),
            mesh_triangles: 0,
            spheres: Vec::with_capacity(self.spheres.len()),
            capsules: Vec::with_capacity(self.capsules.len()),
            boxes: Vec::with_capacity(self.boxes.len()),
            dropped: self.dropped,
        };
        // Box starts in triangle order, to renumber them after dropped triangles.
        let mut box_starts = self.boxes.iter().peekable();
        for (k, tri) in self.triangles.iter().enumerate() {
            while let Some(b) = box_starts.next_if(|b| b.first_triangle <= k) {
                let corners = b.corners.map(|v| t.apply(v));
                // A box whose corners all stay in range keeps all 12 triangles (the same
                // corners, transformed the same way).
                if b.first_triangle == k && corners.iter().all(|v| math::bounded3(*v)) {
                    out.boxes.push(BoxVolume {
                        corners,
                        first_triangle: out.triangles.len(),
                    });
                }
            }
            let p = tri.map(|v| t.apply(v));
            if p.iter().all(|v| math::bounded3(*v)) {
                out.triangles.push(p);
                if k < self.mesh_triangles {
                    out.mesh_triangles += 1;
                }
            } else {
                out.dropped += 1;
            }
        }
        for s in &self.spheres {
            let s = Sphere {
                centre: t.apply(s.centre),
                radius: s.radius * r,
            };
            if s.aabb().is_some() {
                out.spheres.push(s);
            } else {
                out.dropped += 1;
            }
        }
        for c in &self.capsules {
            let c = Capsule {
                a: t.apply(c.a),
                b: t.apply(c.b),
                radius: c.radius * r,
            };
            if c.is_valid() {
                out.capsules.push(c);
            } else {
                out.dropped += 1;
            }
        }
        Ok(out)
    }

    /// Approximate heap bytes.
    #[must_use]
    pub fn heap_size(&self) -> usize {
        self.triangles.capacity() * 36
            + self.spheres.capacity() * 16
            + self.capsules.capacity() * 28
            + self.boxes.capacity() * std::mem::size_of::<BoxVolume>()
    }
}

/// The 8 corners of a box (corner *k*: bit 0 x max, bit 1 y max, bit 2 z max) placed by
/// `frame`; `None` when inverted, non-finite or with a zero rotation.
fn box_corners(min: [f32; 3], max: [f32; 3], frame: &Frame) -> Option<[[f32; 3]; 8]> {
    if !math::bounded3(min) || !math::bounded3(max) || (0..3).any(|a| min[a] > max[a]) {
        return None;
    }
    let t = Transform {
        position: frame.position,
        rotation: frame.rotation,
        scale: frame.scale,
    }
    .validated()?;
    let corners: [[f32; 3]; 8] = std::array::from_fn(|k| {
        let c: [f32; 3] = std::array::from_fn(|a| if k & (1 << a) == 0 { min[a] } else { max[a] });
        t.apply(c)
    });
    corners
        .iter()
        .all(|c| math::bounded3(*c))
        .then_some(corners)
}

fn check(what: &'static str, count: usize, limit: usize) -> Result<()> {
    if count > limit {
        Err(PhysicsError::TooLarge { what, count, limit })
    } else {
        Ok(())
    }
}
