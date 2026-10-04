// SPDX-License-Identifier: GPL-3.0-or-later
//! Object assembly records: entity descriptors (0x47), visual templates (0x1F) and setups
//! (0x04), the chain that leads from an object to its part meshes (0x06).
//!
//! ```text
//! 0x47 entity ─► (0x47 parent)* ─► 0x1F template ─► (0x1F parent)? ─► 0x04 setup ─► 0x06 parts
//! ```
//!
//! - [`EntityDescriptor`] / [`EntityHeader`]: per-object class data. The header (up to the
//!   template link) decodes without a schema; the full record embeds a property set and
//!   needs the property dictionary.
//! - [`VisualTemplate`]: scale-like values, an optional appearance and the setup link.
//! - [`Setup`]: Havok tagfile (opaque here), attachment frames, collision volumes, physics
//!   bounds and the part list.
//! - [`resolve_setup_did`] / [`resolve_setup`]: follow the chain through a caller-supplied
//!   fetch function, with a hop limit and cycle detection, to the setup's DID (without
//!   reading the setup) or to the decoded setup. They never touch an archive themselves.
//!
//! Floats (frames, collision volumes, bounds) are returned as stored and are **not
//! validated**: a corrupt record can hold NaN, infinities or inverted boxes. The derived
//! bounds ([`CollisionBox::aabb`], [`Setup::volume_bounds`], ...) return `None` for such
//! values; anything else read from the raw fields must be checked (e.g. with
//! [`Aabb::is_valid`]) before it feeds a camera or a matrix.
//!
//! Parts of a static setup are drawn in the setup's own frame: the part list carries no
//! transforms (`setup.md` §7).
//!
//! Spec: `docs/specs/setup.md` §2–§7, §9–§10. Prefix 0x01 records are stand-alone collision
//! geometry, not setups (§2.2), and are not decoded here.

mod entity;
mod record;
mod resolve;
mod template;

pub use entity::{
    ENTITY_FLAGS_KNOWN, ENTITY_FLAGS_REQUIRED, EntityBlock, EntityDescriptor, EntityHeader,
    TemplateLink,
};
pub use record::{
    BoneFrame, CollisionBox, CollisionCapsule, CollisionSphere, NamedFrame, PhysicsBounds,
    Placement, Setup, TAGFILE_MAGIC,
};
pub use resolve::{MAX_CHAIN_HOPS, ResolveError, SetupChain, resolve_setup, resolve_setup_did};
pub use template::{
    APPEARANCE_SECTION_HEADER, SETUP_SECTION_DID_LIST, SetupSection, SetupSectionEntry,
    TEMPLATE_FLAGS_KNOWN, TemplateAppearance, VisualTemplate,
};

use crate::error::Result;
use crate::reader::Reader;

/// Type prefix of entity descriptors.
pub const ENTITY_PREFIX: u8 = 0x47;
/// Type prefix of visual templates.
pub const TEMPLATE_PREFIX: u8 = 0x1F;
/// Type prefix of setups.
pub const SETUP_PREFIX: u8 = 0x04;
/// Type prefix of part meshes.
pub const MESH_PREFIX: u8 = 0x06;
/// Type prefix of collision meshes named by setups (`setup.md` §6.5).
pub const COLLISION_MESH_PREFIX: u8 = 0x66;
/// Type prefix of stand-alone property sets named by entity descriptors (§4.4).
pub const PROPERTY_SET_PREFIX: u8 = 0x78;

/// A rotation stored as a quaternion in **(w, x, y, z)** order (`setup.md` §6.7).
///
/// Note the order when converting to a math library: glam's `Quat::from_array` and most
/// GPU conventions expect (x, y, z, w), which [`Quaternion::to_xyzw`] returns. Applying a
/// stored rotation as the active rotation `v' = q v q*` ([`Quaternion::rotate`]) reproduces
/// the stored physics bounds of rotated collision boxes (§10.1). The default is the
/// identity.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Quaternion {
    /// Scalar part.
    pub w: f32,
    /// X component.
    pub x: f32,
    /// Y component.
    pub y: f32,
    /// Z component.
    pub z: f32,
}

impl Default for Quaternion {
    fn default() -> Self {
        Self::IDENTITY
    }
}

impl Quaternion {
    /// The identity rotation, stored as (1, 0, 0, 0).
    pub const IDENTITY: Self = Self {
        w: 1.0,
        x: 0.0,
        y: 0.0,
        z: 0.0,
    };

    /// Whether this is exactly the stored identity (1, 0, 0, 0) or its negation.
    #[must_use]
    pub fn is_identity(self) -> bool {
        self.x == 0.0 && self.y == 0.0 && self.z == 0.0 && self.w.abs() == 1.0
    }

    /// The components in stored order, **(w, x, y, z)**.
    #[must_use]
    pub fn to_wxyz(self) -> [f32; 4] {
        [self.w, self.x, self.y, self.z]
    }

    /// The components in **(x, y, z, w)** order, as glam's `Quat::from_array` expects.
    #[must_use]
    pub fn to_xyzw(self) -> [f32; 4] {
        [self.x, self.y, self.z, self.w]
    }

    /// This rotation scaled to unit length, or `None` when a component is not finite or
    /// the length is zero (or too small to divide by). Stored rotations are unit length.
    #[must_use]
    pub fn normalized(self) -> Option<Self> {
        let q = self.to_wxyz();
        if !q.iter().all(|c| c.is_finite()) {
            return None;
        }
        let len = q.iter().map(|c| f64::from(*c).powi(2)).sum::<f64>().sqrt();
        if !len.is_normal() {
            return None;
        }
        #[allow(clippy::cast_possible_truncation)]
        let n = |c: f32| (f64::from(c) / len) as f32;
        Some(Self {
            w: n(self.w),
            x: n(self.x),
            y: n(self.y),
            z: n(self.z),
        })
    }

    /// Rotates `v` by this rotation, `v' = q v q*` (the active rotation). The quaternion is
    /// used as is; call [`Quaternion::normalized`] first unless it is known to be unit
    /// length (a non-unit `q` also scales by |q|²).
    #[must_use]
    pub fn rotate(self, v: [f32; 3]) -> [f32; 3] {
        let (w, u) = (self.w, [self.x, self.y, self.z]);
        let cross = |a: [f32; 3], b: [f32; 3]| {
            [
                a[1] * b[2] - a[2] * b[1],
                a[2] * b[0] - a[0] * b[2],
                a[0] * b[1] - a[1] * b[0],
            ]
        };
        let dot = |a: [f32; 3], b: [f32; 3]| a[0] * b[0] + a[1] * b[1] + a[2] * b[2];
        // q v q* = (w² − u·u) v + 2 (u·v) u + 2 w (u × v)
        let (uu, uv, c) = (dot(u, u), dot(u, v), cross(u, v));
        std::array::from_fn(|i| (w * w - uu) * v[i] + 2.0 * uv * u[i] + 2.0 * w * c[i])
    }

    pub(crate) fn read(r: &mut Reader<'_>) -> Result<Self> {
        Ok(Self {
            w: r.f32()?,
            x: r.f32()?,
            y: r.f32()?,
            z: r.f32()?,
        })
    }

    pub(crate) fn write(self, out: &mut Vec<u8>) {
        put_f32s(out, &self.to_wxyz());
    }
}

/// A placement frame (40 bytes): position, scale, rotation (`setup.md` §6.3).
///
/// The floats are stored values and are not validated (see the module docs). How scale
/// combines with rotation is not confirmed; every observed collision box has scale 1. The
/// default is [`Frame::IDENTITY`].
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Frame {
    /// Translation.
    pub position: [f32; 3],
    /// Per-axis scale; (1, 1, 1) in most elements.
    pub scale: [f32; 3],
    /// Rotation, (w, x, y, z) on disk.
    pub rotation: Quaternion,
}

impl Default for Frame {
    fn default() -> Self {
        Self::IDENTITY
    }
}

impl Frame {
    /// Stored size in bytes.
    pub const SIZE: usize = 40;

    /// Position 0, scale 1, identity rotation.
    pub const IDENTITY: Self = Self {
        position: [0.0; 3],
        scale: [1.0; 3],
        rotation: Quaternion::IDENTITY,
    };

    pub(crate) fn read(r: &mut Reader<'_>) -> Result<Self> {
        Ok(Self {
            position: read_vec3(r)?,
            scale: read_vec3(r)?,
            rotation: Quaternion::read(r)?,
        })
    }

    pub(crate) fn write(&self, out: &mut Vec<u8>) {
        put_f32s(out, &self.position);
        put_f32s(out, &self.scale);
        self.rotation.write(out);
    }
}

/// An axis-aligned box.
///
/// A box read from a record (e.g. [`PhysicsBounds::aabb`]) is not validated; check
/// [`Aabb::is_valid`] before using it for framing or culling.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct Aabb {
    /// Minimum corner.
    pub min: [f32; 3],
    /// Maximum corner.
    pub max: [f32; 3],
}

impl Aabb {
    /// The smallest box holding every point, or `None` for no points or a non-finite one.
    #[must_use]
    pub fn from_points(points: impl IntoIterator<Item = [f32; 3]>) -> Option<Self> {
        let mut out: Option<Self> = None;
        for p in points {
            if !p.iter().all(|c| c.is_finite()) {
                return None;
            }
            let b = Self { min: p, max: p };
            out = Some(out.map_or(b, |o| o.union(b)));
        }
        out
    }

    /// Whether every coordinate is finite and `min <= max` on every axis.
    #[must_use]
    pub fn is_valid(&self) -> bool {
        self.min.iter().chain(&self.max).all(|c| c.is_finite())
            && (0..3).all(|i| self.min[i] <= self.max[i])
    }

    /// The smallest box holding both. Meant for valid boxes ([`Aabb::is_valid`]): with a
    /// NaN coordinate the result depends on the argument order.
    #[must_use]
    pub fn union(self, other: Self) -> Self {
        let mut out = self;
        for i in 0..3 {
            out.min[i] = out.min[i].min(other.min[i]);
            out.max[i] = out.max[i].max(other.max[i]);
        }
        out
    }

    /// Whether every coordinate differs from `other`'s by at most `eps`.
    #[must_use]
    pub fn approx_eq(self, other: Self, eps: f32) -> bool {
        self.min
            .iter()
            .chain(&self.max)
            .zip(other.min.iter().chain(&other.max))
            .all(|(a, b)| (a - b).abs() <= eps)
    }
}

pub(crate) fn read_vec3(r: &mut Reader<'_>) -> Result<[f32; 3]> {
    Ok([r.f32()?, r.f32()?, r.f32()?])
}

pub(crate) fn put_u32(out: &mut Vec<u8>, v: u32) {
    out.extend_from_slice(&v.to_le_bytes());
}

pub(crate) fn put_f32s(out: &mut Vec<u8>, v: &[f32]) {
    for x in v {
        out.extend_from_slice(&x.to_le_bytes());
    }
}

pub(crate) fn put_tsize(out: &mut Vec<u8>, hint: u8, count: usize) {
    out.push(hint);
    crate::reader::write_vle(out, u32::try_from(count).unwrap_or(u32::MAX));
}

pub(crate) fn len_u32(len: usize) -> u32 {
    u32::try_from(len).unwrap_or(u32::MAX)
}
