// SPDX-License-Identifier: GPL-3.0-or-later
//! Setups, DID prefix 0x04 (`client_general.dat`): skeleton tagfile, attachment frames,
//! collision volumes, physics bounds and the part list.
//!
//! Spec: `docs/specs/setup.md` §6.

use super::{
    Aabb, COLLISION_MESH_PREFIX, Frame, MESH_PREFIX, len_u32, put_f32s, put_tsize, put_u32,
    read_vec3,
};
use crate::did::{Did, DidKind};
use crate::error::{FormatError, Result};
use crate::reader::{Reader, TSize};
use crate::record::{Record, read_prologue};

/// The 8-byte magic every observed tagfile starts with (§6.1). The tagfile is kept opaque;
/// a different start is reported by [`Setup::first_unusual_field`], not rejected.
pub const TAGFILE_MAGIC: [u8; 8] = [0x1E, 0x0D, 0xB0, 0xCA, 0xCE, 0xFA, 0x11, 0xD0];

/// Element of frame list A (52 bytes, §6.3); probably an attachment point bound to a bone.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct BoneFrame {
    /// Key, an enum value 0x1000xxxx in observed data.
    pub key: u32,
    /// Second key; equal to `key` in every observed element.
    pub key2: u32,
    /// Bone index; −1 probably means "not bound to a bone".
    pub bone: i32,
    /// The frame.
    pub frame: Frame,
}

/// Element of frame list B (44 bytes, §6.3). What it places is OPEN.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct Placement {
    /// Identifier; 0 in every observed element.
    pub id: u32,
    /// The frame.
    pub frame: Frame,
}

/// Element of frame list C (48 bytes, §6.3). What it places is OPEN.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct NamedFrame {
    /// Key.
    pub key: u32,
    /// Second key; equal to `key` in every observed element.
    pub key2: u32,
    /// The frame.
    pub frame: Frame,
}

/// A collision box (64 bytes, §6.4): local corners, then a frame placing them.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct CollisionBox {
    /// Local minimum corner.
    pub min: [f32; 3],
    /// Local maximum corner.
    pub max: [f32; 3],
    /// Placement of the box.
    pub frame: Frame,
}

impl CollisionBox {
    /// Stored size in bytes.
    pub const SIZE: usize = 64;

    /// The box's bounds in setup space (§6.4): the eight local corners rotated by the
    /// frame's rotation (active, `v' = q v q*` with `q` read as (w, x, y, z)), then moved by
    /// the frame position. This reproduces the stored physics bounds of every box-only
    /// setup on the reference install, rotated or not (§10.1).
    ///
    /// `None` when a value is not finite, the local box is inverted (`min > max` on an
    /// axis), the rotation has zero length, or the frame scale is not (1, 1, 1): how scale
    /// combines with rotation is unconfirmed and no observed box is scaled, so no guess is
    /// made.
    #[must_use]
    pub fn aabb(&self) -> Option<Aabb> {
        let local = Aabb {
            min: self.min,
            max: self.max,
        };
        if !local.is_valid() || self.frame.scale != [1.0; 3] {
            return None;
        }
        let q = self.frame.rotation.normalized()?;
        let p = self.frame.position;
        let corners = (0..8u8).map(|i| {
            let c: [f32; 3] = std::array::from_fn(|axis| {
                if i & (1 << axis) == 0 {
                    self.min[axis]
                } else {
                    self.max[axis]
                }
            });
            let r = q.rotate(c);
            [r[0] + p[0], r[1] + p[1], r[2] + p[2]]
        });
        Aabb::from_points(corners).filter(Aabb::is_valid)
    }
}

/// A collision sphere (16 bytes, §6.4).
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct CollisionSphere {
    /// Centre.
    pub centre: [f32; 3],
    /// Radius.
    pub radius: f32,
}

impl CollisionSphere {
    /// Stored size in bytes.
    pub const SIZE: usize = 16;

    /// Bounding box of the sphere; `None` when a value is not finite or the radius is
    /// negative.
    #[must_use]
    pub fn aabb(&self) -> Option<Aabb> {
        let (c, r) = (self.centre, self.radius);
        if r < 0.0 {
            return None;
        }
        Some(Aabb {
            min: [c[0] - r, c[1] - r, c[2] - r],
            max: [c[0] + r, c[1] + r, c[2] + r],
        })
        .filter(Aabb::is_valid)
    }
}

/// A vertical collision capsule (20 bytes, §6.4).
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct CollisionCapsule {
    /// Centre of the lower cap.
    pub base: [f32; 3],
    /// Distance between the cap centres, along +Z.
    pub length: f32,
    /// Radius.
    pub radius: f32,
}

impl CollisionCapsule {
    /// Stored size in bytes.
    pub const SIZE: usize = 20;

    /// Bounding box of the capsule: z from `base − r` to `base + length + r`. `None` when
    /// a value is not finite or the radius or length is negative.
    #[must_use]
    pub fn aabb(&self) -> Option<Aabb> {
        let (b, r) = (self.base, self.radius);
        if r < 0.0 || self.length < 0.0 {
            return None;
        }
        Some(Aabb {
            min: [b[0] - r, b[1] - r, b[2] - r],
            max: [b[0] + r, b[1] + r, b[2] + self.length + r],
        })
        .filter(Aabb::is_valid)
    }
}

/// Collision bounds of a setup (§6.5). All zero when the setup has no collision. They
/// describe collision, not the visible geometry. Stored values, not validated: check
/// [`Aabb::is_valid`] before use.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct PhysicsBounds {
    /// Bounding box of the collision.
    pub aabb: Aabb,
    /// Height: max z − min z.
    pub height: f32,
    /// Largest distance of the collision from the vertical axis through the origin.
    pub radius: f32,
}

impl PhysicsBounds {
    /// Whether all eight values are zero (no collision).
    #[must_use]
    pub fn is_zero(&self) -> bool {
        self.aabb
            .min
            .iter()
            .chain(&self.aabb.max)
            .all(|&v| v == 0.0)
            && self.height == 0.0
            && self.radius == 0.0
    }
}

/// A decoded setup record (§6.1).
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Setup {
    /// The record's own DID.
    pub did: Did,
    /// The Havok binary tagfile (skeleton; decoded in part 2 of the spec), kept verbatim.
    pub tagfile: Vec<u8>,
    /// Header of the attachment map. Only the hint is written by [`Setup::to_bytes`]; the
    /// count is taken from `attach_map`.
    pub attach_map_size: TSize,
    /// Attachment map (key, value) pairs in stored order; meaning OPEN (§6.2).
    pub attach_map: Vec<(u32, u32)>,
    /// TSize hint of frame list A's header; 0 and unused when the list is empty. (The
    /// header is absent for an empty list and repeats the list's u32 count, so only the
    /// hint is kept.)
    pub bone_frames_hint: u8,
    /// Frame list A.
    pub bone_frames: Vec<BoneFrame>,
    /// Frame list B.
    pub placements: Vec<Placement>,
    /// TSize hint of frame list C's header; 0 and unused when the list is empty.
    pub named_frames_hint: u8,
    /// Frame list C.
    pub named_frames: Vec<NamedFrame>,
    /// Collision boxes.
    pub boxes: Vec<CollisionBox>,
    /// Collision spheres.
    pub spheres: Vec<CollisionSphere>,
    /// Collision capsules.
    pub capsules: Vec<CollisionCapsule>,
    /// Collision mesh (0x66 DID), or the null DID.
    pub collision_mesh: Did,
    /// Collision bounds.
    pub physics_bounds: PhysicsBounds,
    /// Part meshes (0x06 DIDs), drawn in the setup's own frame (§7).
    pub parts: Vec<Did>,
}

impl Setup {
    /// Whether the tagfile starts with [`TAGFILE_MAGIC`].
    #[must_use]
    pub fn tagfile_has_magic(&self) -> bool {
        self.tagfile.starts_with(&TAGFILE_MAGIC)
    }

    /// Whether the setup has any collision: a mesh or a volume.
    #[must_use]
    pub fn has_collision(&self) -> bool {
        !self.collision_mesh.is_null()
            || !self.boxes.is_empty()
            || !self.spheres.is_empty()
            || !self.capsules.is_empty()
    }

    /// Union of the bounding boxes of the collision volumes (boxes, spheres, capsules; see
    /// their `aabb` methods). `None` when there are no volumes or any volume has no valid
    /// bounds (non-finite or negative values, a scaled box), so a returned box is always
    /// [valid](Aabb::is_valid). The collision mesh is not included (its layout is not
    /// decoded). For volume-only setups this equals the stored physics bounds (§6.4).
    #[must_use]
    pub fn volume_bounds(&self) -> Option<Aabb> {
        let volumes = self
            .boxes
            .iter()
            .map(CollisionBox::aabb)
            .chain(self.spheres.iter().map(CollisionSphere::aabb))
            .chain(self.capsules.iter().map(CollisionCapsule::aabb));
        let mut out: Option<Aabb> = None;
        for b in volumes {
            let b = b?;
            out = Some(out.map_or(b, |o| o.union(b)));
        }
        out
    }

    /// The first deviation from the invariants observed on every real record (SI3, SI4
    /// prefixes, §6.3 key echoes), or `None`. Such records still decode.
    #[must_use]
    pub fn first_unusual_field(&self) -> Option<&'static str> {
        if !self.tagfile_has_magic() {
            return Some("tagfile does not start with the observed magic");
        }
        if self.parts.iter().any(|d| d.prefix() != MESH_PREFIX) {
            return Some("part is not a 0x06 DID");
        }
        if !self.collision_mesh.is_null() && self.collision_mesh.prefix() != COLLISION_MESH_PREFIX {
            return Some("collision mesh is not a 0x66 DID");
        }
        if self.bone_frames.iter().any(|f| f.key2 != f.key) {
            return Some("frame list A key2 differs from key");
        }
        if self.named_frames.iter().any(|f| f.key2 != f.key) {
            return Some("frame list C key2 differs from key");
        }
        if !self.has_collision() && !self.physics_bounds.is_zero() {
            return Some("physics bounds without collision");
        }
        let b = &self.physics_bounds;
        if !(b.aabb.is_valid() && b.height.is_finite() && b.radius.is_finite()) {
            return Some("physics bounds are not finite or inverted");
        }
        if self.boxes.iter().any(|b| b.aabb().is_none())
            || self.spheres.iter().any(|s| s.aabb().is_none())
            || self.capsules.iter().any(|c| c.aabb().is_none())
        {
            return Some("collision volume with non-finite, negative or scaled size");
        }
        None
    }

    /// Encodes the record. Fails if a list does not fit its count field (more than 255
    /// boxes, spheres, capsules or parts).
    pub fn to_bytes(&self) -> Result<Vec<u8>> {
        let mut out = Vec::new();
        put_u32(&mut out, self.did.0);
        put_u32(&mut out, len_u32(self.tagfile.len()));
        out.extend_from_slice(&self.tagfile);
        put_tsize(&mut out, self.attach_map_size.hint, self.attach_map.len());
        for &(k, v) in &self.attach_map {
            put_u32(&mut out, k);
            put_u32(&mut out, v);
        }
        put_u32(&mut out, len_u32(self.bone_frames.len()));
        if !self.bone_frames.is_empty() {
            put_tsize(&mut out, self.bone_frames_hint, self.bone_frames.len());
            for f in &self.bone_frames {
                put_u32(&mut out, f.key);
                put_u32(&mut out, f.key2);
                out.extend_from_slice(&f.bone.to_le_bytes());
                f.frame.write(&mut out);
            }
        }
        put_u32(&mut out, len_u32(self.placements.len()));
        if !self.placements.is_empty() {
            put_u32(&mut out, len_u32(self.placements.len()));
            for p in &self.placements {
                put_u32(&mut out, p.id);
                p.frame.write(&mut out);
            }
        }
        put_u32(&mut out, len_u32(self.named_frames.len()));
        if !self.named_frames.is_empty() {
            put_tsize(&mut out, self.named_frames_hint, self.named_frames.len());
            for f in &self.named_frames {
                put_u32(&mut out, f.key);
                put_u32(&mut out, f.key2);
                f.frame.write(&mut out);
            }
        }
        let count = |n: usize, what: &'static str, at: usize| {
            u8::try_from(n).map_err(|_| FormatError::malformed(Self::NAME, at, what))
        };
        out.push(count(self.boxes.len(), "more than 255 boxes", out.len())?);
        for b in &self.boxes {
            put_f32s(&mut out, &b.min);
            put_f32s(&mut out, &b.max);
            b.frame.write(&mut out);
        }
        out.push(count(
            self.spheres.len(),
            "more than 255 spheres",
            out.len(),
        )?);
        for s in &self.spheres {
            put_f32s(&mut out, &s.centre);
            put_f32s(&mut out, &[s.radius]);
        }
        out.push(count(
            self.capsules.len(),
            "more than 255 capsules",
            out.len(),
        )?);
        for c in &self.capsules {
            put_f32s(&mut out, &c.base);
            put_f32s(&mut out, &[c.length, c.radius]);
        }
        put_u32(&mut out, self.collision_mesh.0);
        out.push(0);
        let b = &self.physics_bounds;
        put_f32s(&mut out, &b.aabb.min);
        put_f32s(&mut out, &b.aabb.max);
        put_f32s(&mut out, &[b.height, b.radius]);
        out.push(count(self.parts.len(), "more than 255 parts", out.len())?);
        for d in &self.parts {
            put_u32(&mut out, d.0);
        }
        Ok(out)
    }
}

/// Reads a frame-list count and its repeated header (§6.3). Returns the count and the
/// header's TSize hint (0 for list B, whose header is a plain u32).
fn read_list_header(r: &mut Reader<'_>, what: &'static str, tsize: bool) -> Result<(u32, u8)> {
    let n = r.u32()?;
    if n == 0 {
        return Ok((0, 0));
    }
    let at = r.position();
    let (hint, echo) = if tsize {
        let t = r.tsize()?;
        (t.hint, t.count)
    } else {
        (0, r.u32()?)
    };
    if echo != n {
        return Err(FormatError::CountMismatch {
            what,
            offset: at,
            expected: n,
            found: echo,
        });
    }
    Ok((n, hint))
}

fn read_u8_list<T>(
    r: &mut Reader<'_>,
    size: usize,
    mut read: impl FnMut(&mut Reader<'_>) -> Result<T>,
) -> Result<Vec<T>> {
    let n = r.u8()?;
    let mut v = r.alloc_vec(u32::from(n), size, usize::from(n))?;
    for _ in 0..n {
        let item = read(r)?;
        r.push(&mut v, item, u32::from(n))?;
    }
    Ok(v)
}

impl Record<'_> for Setup {
    const NAME: &'static str = "setup";

    fn did(&self) -> Did {
        self.did
    }

    fn parse(data: &[u8]) -> Result<Self> {
        let mut r = Reader::new(data);
        let did = read_prologue(&mut r, Self::NAME, |k| k == DidKind::Setup)?;
        let tag_len = r.u32()?;
        let tagfile = r.bytes(usize::try_from(tag_len).unwrap_or(usize::MAX))?;
        r.charge(tagfile.len())?;
        let tagfile = tagfile.to_vec();

        let attach_map_size = r.tsize()?;
        let n = attach_map_size.count;
        let mut attach_map = r.alloc_vec(n, 8, usize::MAX)?;
        for _ in 0..n {
            let k = r.u32()?;
            let v = r.u32()?;
            r.push(&mut attach_map, (k, v), n)?;
        }

        let (n, bone_frames_hint) = read_list_header(&mut r, "setup frame list A", true)?;
        let mut bone_frames = r.alloc_vec(n, 12 + Frame::SIZE, usize::MAX)?;
        for _ in 0..n {
            let f = BoneFrame {
                key: r.u32()?,
                key2: r.u32()?,
                bone: r.i32()?,
                frame: Frame::read(&mut r)?,
            };
            r.push(&mut bone_frames, f, n)?;
        }

        let (n, _) = read_list_header(&mut r, "setup frame list B", false)?;
        let mut placements = r.alloc_vec(n, 4 + Frame::SIZE, usize::MAX)?;
        for _ in 0..n {
            let p = Placement {
                id: r.u32()?,
                frame: Frame::read(&mut r)?,
            };
            r.push(&mut placements, p, n)?;
        }

        let (n, named_frames_hint) = read_list_header(&mut r, "setup frame list C", true)?;
        let mut named_frames = r.alloc_vec(n, 8 + Frame::SIZE, usize::MAX)?;
        for _ in 0..n {
            let f = NamedFrame {
                key: r.u32()?,
                key2: r.u32()?,
                frame: Frame::read(&mut r)?,
            };
            r.push(&mut named_frames, f, n)?;
        }

        let boxes = read_u8_list(&mut r, CollisionBox::SIZE, |r| {
            Ok(CollisionBox {
                min: read_vec3(r)?,
                max: read_vec3(r)?,
                frame: Frame::read(r)?,
            })
        })?;
        let spheres = read_u8_list(&mut r, CollisionSphere::SIZE, |r| {
            Ok(CollisionSphere {
                centre: read_vec3(r)?,
                radius: r.f32()?,
            })
        })?;
        let capsules = read_u8_list(&mut r, CollisionCapsule::SIZE, |r| {
            Ok(CollisionCapsule {
                base: read_vec3(r)?,
                length: r.f32()?,
                radius: r.f32()?,
            })
        })?;
        let collision_mesh = r.did()?;
        let reserved_at = r.position();
        let reserved = r.u8()?;
        if reserved != 0 {
            // Zero in every record (SI3). A non-zero value might be a count of something the
            // spec does not describe, so the rest of the layout cannot be trusted.
            return Err(FormatError::Unsupported {
                what: "setup reserved byte",
                offset: reserved_at,
                value: u32::from(reserved),
            });
        }
        let physics_bounds = PhysicsBounds {
            aabb: Aabb {
                min: read_vec3(&mut r)?,
                max: read_vec3(&mut r)?,
            },
            height: r.f32()?,
            radius: r.f32()?,
        };
        let parts = read_u8_list(&mut r, 4, |r| r.did())?;
        r.expect_end(Self::NAME)?;
        r.log_unknown_hints(Self::NAME);
        Ok(Self {
            did,
            tagfile,
            attach_map_size,
            attach_map,
            bone_frames_hint,
            bone_frames,
            placements,
            named_frames_hint,
            named_frames,
            boxes,
            spheres,
            capsules,
            collision_mesh,
            physics_bounds,
            parts,
        })
    }
}
