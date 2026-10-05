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

/// Element of frame list A (52 bytes, §6.3): a **holding location** (`setup.md` part 2,
/// `skeleton-anim.md` §8.2). See [`Setup::holding_location`] for the typed view.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct BoneFrame {
    /// `HoldingLocation` enum key (`RHand`, `Wpn_RHand`, `Sheath_Back_2H`, …); unique per
    /// setup.
    pub key: u32,
    /// Second key; equal to `key` in every observed element.
    pub key2: u32,
    /// Bone index into the setup's skeleton, or −1 for the setup origin.
    pub bone: i32,
    /// The frame, in the bone's space: location = bone model transform · translate ·
    /// rotate · scale.
    pub frame: Frame,
}

impl BoneFrame {
    /// What the frame is relative to; `None` for a bone value below −1 (never observed).
    #[must_use]
    pub fn anchor(&self) -> Option<Anchor> {
        match self.bone {
            -1 => Some(Anchor::Origin),
            b => u32::try_from(b).ok().map(Anchor::Bone),
        }
    }
}

/// What a holding-location frame is relative to (`setup.md` §6.3).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Anchor {
    /// The setup's origin (stored bone −1).
    Origin,
    /// A bone of the setup's skeleton.
    Bone(u32),
}

/// A holding location of a setup (frame list A, `setup.md` §6.3, §13): where a held item's
/// origin goes.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct HoldingLocation {
    /// The `HoldingLocation` enum key.
    pub key: u32,
    /// What the frame is relative to.
    pub anchor: Anchor,
    /// The frame in the anchor's space.
    pub frame: Frame,
}

/// An `attach_map` element (`setup.md` §6.2): a `BoneType` enum key and the bone it names.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct BoneAttachment {
    /// The `BoneType` enum key (`head`, `Rweaponholder`, face-slider bones, …).
    pub bone_type: u32,
    /// Bone index into the setup's skeleton.
    pub bone: u32,
}

/// Element of frame list B (44 bytes, §6.3). What it places is OPEN.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct Placement {
    /// Identifier; 0 in every observed element.
    pub id: u32,
    /// The frame.
    pub frame: Frame,
}

/// Element of frame list C (48 bytes, §6.3): an object offset for a UI viewport, keyed by
/// `UIViewport_Type` (`Paperdoll`, `Opponent`, `MountedPaperdoll`, …). World rendering
/// ignores it.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct NamedFrame {
    /// `UIViewport_Type` enum key.
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
    /// Attachment map (key, value) pairs in stored order: `BoneType` enum key → bone index
    /// into the setup's skeleton (§6.2). See [`Setup::attachments`] and [`Setup::bone_for`].
    pub attach_map: Vec<(u32, u32)>,
    /// TSize hint of frame list A's header; 0 and unused when the list is empty. (The
    /// header is absent for an empty list and repeats the list's u32 count, so only the
    /// hint is kept.)
    pub bone_frames_hint: u8,
    /// Frame list A: holding locations (§6.3). See [`Setup::holding_location`].
    pub bone_frames: Vec<BoneFrame>,
    /// Frame list B: unscaled, unrotated positions on large props (meaning OPEN, §6.3).
    pub placements: Vec<Placement>,
    /// TSize hint of frame list C's header; 0 and unused when the list is empty.
    pub named_frames_hint: u8,
    /// Frame list C: UI-viewport offsets keyed by `UIViewport_Type` (§6.3).
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

    /// The `attach_map` as typed elements, in stored order (§6.2).
    pub fn attachments(&self) -> impl Iterator<Item = BoneAttachment> + '_ {
        self.attach_map
            .iter()
            .map(|&(bone_type, bone)| BoneAttachment { bone_type, bone })
    }

    /// The bone a `BoneType` key names on this skeleton, or `None` when the skeleton has no
    /// such bone (callers fall back to the object origin; `skeleton-anim.md` §8.1).
    #[must_use]
    pub fn bone_for(&self, bone_type: u32) -> Option<u32> {
        self.attach_map
            .iter()
            .find(|(k, _)| *k == bone_type)
            .map(|&(_, b)| b)
    }

    /// The holding location `key` (frame list A), or `None` when the setup lacks it (a held
    /// item at a missing location is not drawn; `appearance.md` §10) or its bone value is
    /// below −1.
    #[must_use]
    pub fn holding_location(&self, key: u32) -> Option<HoldingLocation> {
        self.bone_frames
            .iter()
            .find(|f| f.key == key)
            .and_then(Self::typed_location)
    }

    /// Every holding location, in stored order (elements with a bone below −1 skipped).
    pub fn holding_locations(&self) -> impl Iterator<Item = HoldingLocation> + '_ {
        self.bone_frames.iter().filter_map(Self::typed_location)
    }

    fn typed_location(f: &BoneFrame) -> Option<HoldingLocation> {
        Some(HoldingLocation {
            key: f.key,
            anchor: f.anchor()?,
            frame: f.frame,
        })
    }

    /// The UI-viewport frame for `UIViewport_Type` key `key` (frame list C).
    #[must_use]
    pub fn viewport_frame(&self, key: u32) -> Option<&Frame> {
        self.named_frames
            .iter()
            .find(|f| f.key == key)
            .map(|f| &f.frame)
    }

    /// The first violation of the part-2 bone invariants against the setup's skeleton of
    /// `bone_count` bones (SI7, SI8): `attach_map` keys and values unique and below the bone
    /// count, list A keys unique and bones −1 or below the bone count. `None` when all hold.
    /// Callers report violations as data-quality issues (§14).
    #[must_use]
    pub fn bone_issue(&self, bone_count: usize) -> Option<&'static str> {
        if let Some(why) = self.attachment_issue() {
            return Some(why);
        }
        let below = |b: u32| usize::try_from(b).is_ok_and(|b| b < bone_count);
        if self.attach_map.iter().any(|&(_, b)| !below(b)) {
            return Some("attach_map bone index not below the bone count");
        }
        if self.bone_frames.iter().any(|f| {
            !matches!(f.anchor(), Some(Anchor::Origin))
                && !matches!(f.anchor(), Some(Anchor::Bone(b)) if below(b))
        }) {
            return Some("frame list A bone neither -1 nor below the bone count");
        }
        None
    }

    /// The skeleton-independent part of SI7 and SI8: unique `attach_map` keys and values,
    /// unique list A keys, list A bones ≥ −1.
    fn attachment_issue(&self) -> Option<&'static str> {
        fn has_dup(mut v: Vec<u32>) -> bool {
            v.sort_unstable();
            v.windows(2).any(|w| w[0] == w[1])
        }
        if has_dup(self.attach_map.iter().map(|e| e.0).collect()) {
            return Some("attach_map keys not unique");
        }
        if has_dup(self.attach_map.iter().map(|e| e.1).collect()) {
            return Some("attach_map bone indices not unique");
        }
        if has_dup(self.bone_frames.iter().map(|f| f.key).collect()) {
            return Some("frame list A keys not unique");
        }
        if self.bone_frames.iter().any(|f| f.bone < -1) {
            return Some("frame list A bone below -1");
        }
        None
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
    /// prefixes, §6.3 key echoes, the skeleton-independent parts of SI7 and SI8), or
    /// `None`. Such records still decode. [`Setup::bone_issue`] checks against a skeleton.
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
        if let Some(why) = self.attachment_issue() {
            return Some(why);
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
