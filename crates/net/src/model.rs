// SPDX-License-Identifier: GPL-3.0-or-later
//! The neutral world data model shared by every backend.
//!
//! These types are Tessera's own vocabulary. They deliberately do not mirror
//! any wire format: a protocol adapter translates between its wire
//! representation and these types, so the rest of the client never sees
//! protocol details.
//!
//! # Coordinate convention
//!
//! Positions are in metres in a right-handed world frame with **+Z up**
//! (+X east, +Y north in the user's data).
//!
//! **Heading.** Every `heading` in this crate is in radians about +Z, `0`
//! pointing along **+X (east)**, increasing counter-clockwise seen from
//! above: an entity with heading `θ` faces `(cos θ, sin θ)`. This is *not*
//! the data's model convention, where the identity rotation leaves a
//! character's forward axis (model +Y) pointing north
//! (`docs/specs/coordinates.md` §11.3): a +Y-forward model facing `θ` is drawn
//! with [`Rotation::facing`]`(θ)`, not [`Rotation::about_z`]`(θ)`. The `/loc`
//! compass heading (0 = north, clockwise, §11.4) converts with
//! [`heading_from_compass_degrees`] and [`compass_degrees_from_heading`].
//!
//! A world may consist of several separate **regions** whose coordinate
//! ranges overlap ([`RegionId`]). The player's own position travels as a
//! precise [`Location`] (`f64`, region-qualified; `docs/ARCHITECTURE.md`
//! §9.1): [`Command::Move`](crate::Command::Move) and
//! [`Event::LocationReport`](crate::Event::LocationReport). [`Transform`]
//! positions (other entities, [`Event::WorldEntered`](crate::Event::WorldEntered),
//! corrections) are `f32` metres in the frame of the player's current region,
//! the one named by the latest location report for the player; near 40 km
//! their precision is about 4 mm.
//!
//! **Landblocks.** A region is a grid of [`Landblock`]s
//! (`docs/specs/coordinates.md` §4). A position normally belongs to the block
//! whose square contains it, but an interior position may lie far outside its
//! owning block's square and must never be re-homed to a neighbour (§7); such
//! a position carries an [`Anchor`] naming its owner.

/// Opaque identifier of an entity, assigned by the world source.
///
/// Identifiers are only meaningful within one session.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct EntityId(pub u64);

/// A 3-component vector of `f32`.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct Vec3 {
    /// X component.
    pub x: f32,
    /// Y component.
    pub y: f32,
    /// Z component (up).
    pub z: f32,
}

impl Vec3 {
    /// The zero vector.
    pub const ZERO: Self = Self::new(0.0, 0.0, 0.0);

    /// Creates a vector.
    #[must_use]
    pub const fn new(x: f32, y: f32, z: f32) -> Self {
        Self { x, y, z }
    }

    /// True when every component is finite (not NaN or infinite).
    #[must_use]
    pub fn is_finite(self) -> bool {
        self.x.is_finite() && self.y.is_finite() && self.z.is_finite()
    }

    /// Euclidean length.
    #[must_use]
    pub fn length(self) -> f32 {
        (self.x * self.x + self.y * self.y + self.z * self.z).sqrt()
    }

    /// Euclidean distance to `other`.
    #[must_use]
    pub fn distance(self, other: Self) -> f32 {
        (self - other).length()
    }
}

impl std::ops::Add for Vec3 {
    type Output = Self;
    fn add(self, rhs: Self) -> Self {
        Self::new(self.x + rhs.x, self.y + rhs.y, self.z + rhs.z)
    }
}

impl std::ops::Sub for Vec3 {
    type Output = Self;
    fn sub(self, rhs: Self) -> Self {
        Self::new(self.x - rhs.x, self.y - rhs.y, self.z - rhs.z)
    }
}

impl std::ops::Mul<f32> for Vec3 {
    type Output = Self;
    fn mul(self, rhs: f32) -> Self {
        Self::new(self.x * rhs, self.y * rhs, self.z * rhs)
    }
}

/// Position plus facing of an entity.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct Transform {
    /// World position in metres.
    pub position: Vec3,
    /// Facing in radians (see the module docs for the convention).
    pub heading: f32,
}

impl Transform {
    /// Creates a transform.
    #[must_use]
    pub const fn new(position: Vec3, heading: f32) -> Self {
        Self { position, heading }
    }

    /// True when position and heading are finite.
    #[must_use]
    pub fn is_finite(self) -> bool {
        self.position.is_finite() && self.heading.is_finite()
    }
}

/// Broad classification of an entity, enough for the client to pick a
/// presentation. Finer detail belongs in data looked up by the client.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum EntityKind {
    /// Another player character.
    Player,
    /// A non-player character (vendor, quest giver, ...).
    NonPlayer,
    /// A creature or monster.
    Creature,
    /// An inanimate interactive object.
    Object,
}

/// A chat channel in Tessera's own model.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum ChatChannel {
    /// Local speech, heard nearby.
    Say,
    /// Shouted speech, heard further away.
    Yell,
    /// An emote ("waves").
    Emote,
    /// A private message to one named character.
    Tell {
        /// Recipient (on send) or sender (on receive).
        peer: String,
    },
    /// Messages from the world source itself (notices, errors).
    System,
}

/// The heading (module convention: radians, 0 = +X, counter-clockwise) of the
/// `/loc` compass heading `degrees` (0 = north, clockwise, east = 90;
/// `docs/specs/coordinates.md` §11.4, whose clockwise sense is still OPEN
/// there). The result is in `(-π, π]`; non-finite input gives NaN.
#[must_use]
pub fn heading_from_compass_degrees(degrees: f32) -> f32 {
    // Facing (sin h, cos h) = (cos θ, sin θ)  =>  θ = 90° − h.
    let d = f64::from(90.0 - degrees).rem_euclid(360.0);
    let d = if d > 180.0 { d - 360.0 } else { d };
    #[allow(clippy::cast_possible_truncation)] // in (-π, π]
    let r = d.to_radians() as f32;
    r
}

/// The `/loc` compass heading in degrees, `[0, 360)`, of `heading` (module
/// convention); the inverse of [`heading_from_compass_degrees`]. Non-finite
/// input gives NaN.
#[must_use]
pub fn compass_degrees_from_heading(heading: f32) -> f32 {
    let d = (90.0 - f64::from(heading).to_degrees()).rem_euclid(360.0);
    #[allow(clippy::cast_possible_truncation)] // in [0, 360]
    let d = d as f32;
    // Rounding to f32 may land exactly on 360.
    if d >= 360.0 { 0.0 } else { d }
}

/// A character the account may enter the world with.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CharacterSummary {
    /// Display name, also used to select the character in
    /// [`Command::EnterWorld`](crate::Command::EnterWorld).
    pub name: String,
}

/// Why a session ended.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum DisconnectReason {
    /// The client asked to disconnect.
    ClientRequested,
    /// The world source closed the session, with a human-readable reason.
    ServerClosed(String),
    /// The connection timed out or the peer stopped responding.
    TimedOut,
    /// The transport failed (I/O error or protocol violation), with detail.
    TransportFailed(String),
}

/// Maximum length in bytes of a chat message accepted by [`Session`](crate::Session).
pub const MAX_CHAT_LEN: usize = 1024;

/// Maximum length in bytes of a character name accepted by [`Session`](crate::Session).
pub const MAX_NAME_LEN: usize = 64;

/// Whether `name` is acceptable as a character name or tell recipient:
/// 1..=[`MAX_NAME_LEN`] bytes and no control characters, so backends can
/// embed it in their own wire or log formats without escaping surprises.
#[must_use]
pub fn is_valid_name(name: &str) -> bool {
    !name.is_empty() && name.len() <= MAX_NAME_LEN && !name.chars().any(char::is_control)
}

/// `text` cut to at most `max_len` bytes on a character boundary, with every
/// control character (line breaks, escape sequences, ...) replaced by a
/// space, so that text from an untrusted world source is safe to log and
/// show. Borrows when nothing needs changing.
#[must_use]
pub fn sanitize_text(text: &str, max_len: usize) -> std::borrow::Cow<'_, str> {
    if text.len() <= max_len && !text.chars().any(char::is_control) {
        return std::borrow::Cow::Borrowed(text);
    }
    let mut out = String::with_capacity(text.len().min(max_len));
    for c in text.chars() {
        let c = if c.is_control() { ' ' } else { c };
        if out.len() + c.len_utf8() > max_len {
            break;
        }
        out.push(c);
    }
    std::borrow::Cow::Owned(out)
}

/// Side length of a landblock in world units (`docs/specs/coordinates.md` §4).
pub const LANDBLOCK_SIZE: f64 = 160.0;
/// Largest landblock index on either axis (`docs/specs/coordinates.md` §4, CO1).
pub const MAX_LANDBLOCK: u8 = 254;
/// Region-global extent of the grid on either axis, in units.
pub const GRID_EXTENT: f64 = (MAX_LANDBLOCK as f64 + 1.0) * LANDBLOCK_SIZE;
/// Largest absolute block-local X or Y of a position owned by a landblock
/// (an [`Anchor`]ed position, a landblock spawn point). Interior positions
/// reach about ±960 units outside their block's square
/// (`docs/specs/coordinates.md` §7); this leaves a margin.
pub const MAX_LOCAL_OFFSET: f64 = 1280.0;

/// A landblock: one square of a region's grid (`docs/specs/coordinates.md`
/// §4). Region-global coordinates are `G = 160 · index + local`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Landblock {
    /// Region.
    pub region: RegionId,
    /// Index along X (east), 0..=[`MAX_LANDBLOCK`].
    pub x: u8,
    /// Index along Y (north), 0..=[`MAX_LANDBLOCK`].
    pub y: u8,
}

impl Landblock {
    /// A landblock, or `None` when an index exceeds [`MAX_LANDBLOCK`].
    #[must_use]
    pub fn new(region: RegionId, x: u8, y: u8) -> Option<Self> {
        (x <= MAX_LANDBLOCK && y <= MAX_LANDBLOCK).then_some(Self { region, x, y })
    }

    /// The landblock whose square contains `location`, or `None` when the
    /// location lies outside the grid (or is not finite). A point exactly on
    /// a shared edge belongs to the block east or north of it, except at the
    /// far edge of the grid (`docs/specs/coordinates.md` §4 OPEN).
    ///
    /// This is right for outdoor positions only: an interior position belongs
    /// to its [`Anchor`]'s block wherever it lies.
    #[must_use]
    pub fn containing(location: &Location) -> Option<Self> {
        let index = |g: f64| -> Option<u8> {
            if !(0.0..=GRID_EXTENT).contains(&g) {
                return None;
            }
            let i = (g / LANDBLOCK_SIZE).floor().min(f64::from(MAX_LANDBLOCK));
            #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)] // 0..=254
            Some(i as u8)
        };
        Some(Self {
            region: location.region,
            x: index(location.x)?,
            y: index(location.y)?,
        })
    }

    /// Region-global coordinates of the block's south-west corner.
    #[must_use]
    pub fn origin(self) -> [f64; 2] {
        [
            f64::from(self.x) * LANDBLOCK_SIZE,
            f64::from(self.y) * LANDBLOCK_SIZE,
        ]
    }

    /// The location at block-local `offset` (x, y from the south-west
    /// corner, z absolute).
    #[must_use]
    pub fn location(self, offset: [f64; 3]) -> Location {
        let [ox, oy] = self.origin();
        Location::new(self.region, ox + offset[0], oy + offset[1], offset[2])
    }

    /// Block-local X and Y of `location` (which may lie outside the square),
    /// ignoring its region.
    #[must_use]
    pub fn local(self, location: &Location) -> [f64; 2] {
        let [ox, oy] = self.origin();
        [location.x - ox, location.y - oy]
    }

    /// Whether `location`'s block-local X and Y are both inside the square
    /// `[0, 160]`, ignoring its region.
    #[must_use]
    pub fn contains_local(self, location: &Location) -> bool {
        let [x, y] = self.local(location);
        (0.0..=LANDBLOCK_SIZE).contains(&x) && (0.0..=LANDBLOCK_SIZE).contains(&y)
    }

    /// Chebyshev distance in blocks, or `None` across regions.
    #[must_use]
    pub fn chebyshev(self, other: Self) -> Option<u32> {
        (self.region == other.region)
            .then(|| u32::from(self.x.abs_diff(other.x)).max(u32::from(self.y.abs_diff(other.y))))
    }

    /// The block `(dx, dy)` away, if it exists.
    #[must_use]
    pub fn offset(self, dx: i32, dy: i32) -> Option<Self> {
        let step = |v: u8, d: i32| -> Option<u8> {
            let n = i32::from(v).checked_add(d)?;
            u8::try_from(n).ok().filter(|n| *n <= MAX_LANDBLOCK)
        };
        Some(Self {
            region: self.region,
            x: step(self.x, dx)?,
            y: step(self.y, dy)?,
        })
    }
}

impl std::fmt::Display for Landblock {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "r{} ({}, {})", self.region.0, self.x, self.y)
    }
}

/// The owner of a position whose coordinates alone do not say where it
/// belongs: an interior position, which may lie up to about ±960 units outside
/// its landblock's square and is never re-homed to the block its coordinates
/// fall in (`docs/specs/coordinates.md` §7).
///
/// An anchored position is never snapped to the terrain, and the sandbox
/// streams the statics around the anchor's block, not around the block under
/// the coordinates.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Anchor {
    /// The landblock that owns the position.
    pub block: Landblock,
    /// The interior cell of that block (`docs/specs/coordinates.md` §7), or
    /// `0` when the cell is not known (for example a landblock spawn point
    /// with an out-of-square offset).
    pub cell: u16,
}

impl Anchor {
    /// Creates an anchor.
    #[must_use]
    pub const fn new(block: Landblock, cell: u16) -> Self {
        Self { block, cell }
    }

    /// Whether `location` can belong to this anchor: same region and
    /// block-local X and Y within ±[`MAX_LOCAL_OFFSET`]. A player who moves
    /// further away has left the interior; the anchor is then dropped.
    #[must_use]
    pub fn owns(&self, location: &Location) -> bool {
        let [x, y] = self.block.local(location);
        location.region == self.block.region
            && x.abs() <= MAX_LOCAL_OFFSET
            && y.abs() <= MAX_LOCAL_OFFSET
    }
}

/// Identifier of a region: a separate world frame. Positions in different
/// regions are not comparable.
///
/// The value is chosen by the world source; with the user's data it is the
/// data set's region number (`docs/specs/coordinates.md` §4). `RegionId(0)`
/// is the abstract region of a sandbox that has no world data.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Default)]
pub struct RegionId(pub u16);

/// Largest absolute value of a [`Location`] coordinate accepted anywhere in
/// this crate, in metres. Bounds every narrowing to `f32`.
pub const MAX_LOCATION_COORDINATE: f64 = 1.0e7;

/// A precise, region-qualified position: metres in the region's frame
/// (+Z up), `f64` so that positions tens of kilometres from the origin keep
/// sub-millimetre precision.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct Location {
    /// The region whose frame the coordinates are in.
    pub region: RegionId,
    /// X (east in the user's data).
    pub x: f64,
    /// Y (north in the user's data).
    pub y: f64,
    /// Z (up), an absolute height.
    pub z: f64,
}

impl Location {
    /// Creates a location.
    #[must_use]
    pub const fn new(region: RegionId, x: f64, y: f64, z: f64) -> Self {
        Self { region, x, y, z }
    }

    /// A location from an `f32` position in `region`'s frame.
    #[must_use]
    pub fn from_position(region: RegionId, position: Vec3) -> Self {
        Self::new(
            region,
            f64::from(position.x),
            f64::from(position.y),
            f64::from(position.z),
        )
    }

    /// The position narrowed to `f32` (precision about 4 mm at 40 km).
    #[must_use]
    #[allow(clippy::cast_possible_truncation)] // narrowing is the point
    pub fn position(&self) -> Vec3 {
        Vec3::new(self.x as f32, self.y as f32, self.z as f32)
    }

    /// True when every coordinate is finite and within
    /// ±[`MAX_LOCATION_COORDINATE`].
    #[must_use]
    pub fn is_valid(&self) -> bool {
        [self.x, self.y, self.z]
            .iter()
            .all(|c| c.is_finite() && c.abs() <= MAX_LOCATION_COORDINATE)
    }

    /// Horizontal (X/Y) distance to `other`, ignoring regions.
    #[must_use]
    pub fn distance_xy(&self, other: &Self) -> f64 {
        (self.x - other.x).hypot(self.y - other.y)
    }
}

/// An orientation as a unit quaternion (scalar `w` first): the active
/// rotation from an object's own frame into the region frame, Hamilton
/// product (`docs/specs/coordinates.md` §11.2).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Rotation {
    /// Scalar part.
    pub w: f32,
    /// X of the vector part.
    pub x: f32,
    /// Y of the vector part.
    pub y: f32,
    /// Z of the vector part.
    pub z: f32,
}

impl Default for Rotation {
    fn default() -> Self {
        Self::IDENTITY
    }
}

impl Rotation {
    /// No rotation.
    pub const IDENTITY: Self = Self::new(1.0, 0.0, 0.0, 0.0);

    /// Creates a rotation from its components (not normalised).
    #[must_use]
    pub const fn new(w: f32, x: f32, y: f32, z: f32) -> Self {
        Self { w, x, y, z }
    }

    /// A turn of `angle` radians about +Z, counter-clockwise seen from above.
    ///
    /// This turns model +X to `angle`; it is **not** the rotation that makes
    /// a data model (forward = model +Y) face a heading. Use
    /// [`facing`](Self::facing) for that.
    #[must_use]
    pub fn about_z(angle: f32) -> Self {
        let (s, c) = (angle * 0.5).sin_cos();
        Self::new(c, 0.0, 0.0, s)
    }

    /// The rotation that makes a model whose forward axis is model +Y (the
    /// data's convention, `docs/specs/coordinates.md` §11.3) face `heading`
    /// (module convention: 0 = +X, counter-clockwise):
    /// `about_z(heading − π/2)`.
    #[must_use]
    pub fn facing(heading: f32) -> Self {
        Self::about_z(heading - std::f32::consts::FRAC_PI_2)
    }

    /// True when every component is finite.
    #[must_use]
    pub fn is_finite(self) -> bool {
        self.w.is_finite() && self.x.is_finite() && self.y.is_finite() && self.z.is_finite()
    }

    /// Euclidean length of the four components.
    #[must_use]
    pub fn length(self) -> f32 {
        (self.w * self.w + self.x * self.x + self.y * self.y + self.z * self.z).sqrt()
    }

    /// The rotation scaled to unit length, or `None` when it is non-finite
    /// or too close to zero to have a direction.
    #[must_use]
    pub fn normalized(self) -> Option<Self> {
        let len = self.length();
        if !self.is_finite() || !len.is_finite() || len < 1.0e-6 {
            return None;
        }
        Some(Self::new(
            self.w / len,
            self.x / len,
            self.y / len,
            self.z / len,
        ))
    }

    /// The heading (module convention: radians, 0 = +X, counter-clockwise, in
    /// `(-π, π]`) a model whose forward axis is model +Y faces under this
    /// rotation: the inverse of [`facing`](Self::facing) for turns about +Z.
    /// A tilted rotation gives the heading of its forward axis projected onto
    /// the ground; when that axis points straight up or down, the heading of
    /// the model's +X axis turned by π/2 is used instead. `None` when the
    /// rotation is not finite or has no direction.
    #[must_use]
    pub fn heading(self) -> Option<f32> {
        let q = self.normalized()?;
        let f = q.rotate(Vec3::new(0.0, 1.0, 0.0));
        if f.x * f.x + f.y * f.y > 1.0e-8 {
            return Some(f.y.atan2(f.x));
        }
        let r = q.rotate(Vec3::new(1.0, 0.0, 0.0));
        let h = r.y.atan2(r.x) + std::f32::consts::FRAC_PI_2;
        // Back into (-π, π].
        Some(if h > std::f32::consts::PI {
            h - std::f32::consts::TAU
        } else {
            h
        })
    }

    /// Rotates `v` (model frame) into the region frame.
    #[must_use]
    pub fn rotate(self, v: Vec3) -> Vec3 {
        // v' = v + 2 w (q × v) + 2 q × (q × v), q the vector part.
        let q = Vec3::new(self.x, self.y, self.z);
        let cross = |a: Vec3, b: Vec3| {
            Vec3::new(
                a.y * b.z - a.z * b.y,
                a.z * b.x - a.x * b.z,
                a.x * b.y - a.y * b.x,
            )
        };
        let t = cross(q, v) * 2.0;
        v + t * self.w + cross(q, t)
    }
}

/// Largest uniform scale of a valid [`Placement`].
pub const MAX_STATIC_SCALE: f32 = 1.0e4;

/// Largest deviation from 1 of the length of a valid [`Placement`]'s
/// rotation (stored rotations are unit within 0.001,
/// `docs/specs/coordinates.md` CO3).
pub const ROTATION_UNIT_TOLERANCE: f32 = 1.0e-3;

/// Where and how a static object sits in the world: scale, then rotate, then
/// translate (`docs/specs/coordinates.md` §11.2).
///
/// The default is a unit-scale, unrotated placement at the origin of region 0,
/// which is valid.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Placement {
    /// Position of the object's origin.
    pub location: Location,
    /// Orientation, unit length within [`ROTATION_UNIT_TOLERANCE`].
    pub rotation: Rotation,
    /// Uniform scale, in `(0, MAX_STATIC_SCALE]`.
    pub scale: f32,
}

impl Default for Placement {
    fn default() -> Self {
        Self::new(Location::default(), Rotation::IDENTITY, 1.0)
    }
}

impl Placement {
    /// Creates a placement (not validated; see [`is_valid`](Self::is_valid)).
    #[must_use]
    pub const fn new(location: Location, rotation: Rotation, scale: f32) -> Self {
        Self {
            location,
            rotation,
            scale,
        }
    }

    /// True when the location is valid, the rotation finite and of unit
    /// length within [`ROTATION_UNIT_TOLERANCE`], and the scale in
    /// `(0, MAX_STATIC_SCALE]`, so the transform it describes is finite and
    /// does not collapse the geometry.
    #[must_use]
    pub fn is_valid(&self) -> bool {
        let len = self.rotation.length();
        self.location.is_valid()
            && self.rotation.is_finite()
            && len.is_finite()
            && (len - 1.0).abs() <= ROTATION_UNIT_TOLERANCE
            && self.scale.is_finite()
            && self.scale > 0.0
            && self.scale <= MAX_STATIC_SCALE
    }
}

/// A reference to a record in the user's data set (a DID), opaque to this
/// crate. The client resolves it to geometry, sounds and so on.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct DataId(pub u32);

/// Maximum length in bytes of a place name in [`Destination::Named`].
pub const MAX_PLACE_NAME_LEN: usize = 256;

/// Whether `name` is acceptable as a place name: 1..=[`MAX_PLACE_NAME_LEN`]
/// bytes, not only whitespace, and no control characters.
#[must_use]
pub fn is_valid_place_name(name: &str) -> bool {
    !name.trim().is_empty()
        && name.len() <= MAX_PLACE_NAME_LEN
        && !name.chars().any(char::is_control)
}

/// Where a player should appear: on spawn or when teleporting.
#[derive(Debug, Clone, PartialEq)]
#[non_exhaustive]
pub enum Destination {
    /// An exact outdoor location (height included) and facing. The position
    /// belongs to the landblock whose square contains it.
    Location {
        /// Where.
        location: Location,
        /// Facing in radians (module convention).
        heading: f32,
    },
    /// An outdoor location on the ground: the world source's terrain height
    /// replaces `location.z` when it knows it (otherwise `z` is kept).
    Ground {
        /// Where; `z` is a fallback height.
        location: Location,
        /// Facing in radians (module convention).
        heading: f32,
    },
    /// An exact location owned by a landblock whatever its coordinates
    /// (an interior position, `docs/specs/coordinates.md` §7). Never snapped
    /// to the ground and never re-homed.
    Anchored {
        /// Where, in the anchor's region.
        location: Location,
        /// Facing in radians (module convention).
        heading: f32,
        /// The owning landblock and cell; must [own](Anchor::owns) `location`.
        anchor: Anchor,
    },
    /// A place the world source knows by name (an area name, map
    /// coordinates, a `/loc` line: whatever the source understands). The
    /// source decides the height, facing and anchor.
    Named(String),
}

impl Destination {
    /// True when a location is valid, its heading finite and an anchor owns
    /// it, or a name passes [`is_valid_place_name`].
    #[must_use]
    pub fn is_valid(&self) -> bool {
        match self {
            Self::Location { location, heading } | Self::Ground { location, heading } => {
                location.is_valid() && heading.is_finite()
            }
            Self::Anchored {
                location,
                heading,
                anchor,
            } => location.is_valid() && heading.is_finite() && anchor.owns(location),
            Self::Named(name) => is_valid_place_name(name),
        }
    }
}

/// Why an [`Event::LocationReport`](crate::Event::LocationReport) was sent.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum LocationCause {
    /// The player just entered the world here.
    Entered,
    /// The player was moved by a teleport.
    Teleported,
    /// Answer to [`Command::QueryLocation`](crate::Command::QueryLocation).
    Requested,
    /// The world source refused a [`Command::Move`](crate::Command::Move)
    /// and put the player back here (sent with the correcting
    /// [`Event::EntityMoved`](crate::Event::EntityMoved), so the precise
    /// position survives the correction).
    Corrected,
}

/// Most [`Equipped`] entries of one [`Appearance`].
pub const MAX_EQUIPMENT: usize = 32;

/// Largest [`Appearance::scale`].
pub const MAX_APPEARANCE_SCALE: f32 = 100.0;

/// Largest dye code of an [`Equipped`] item. Dye codes of the user's data are small
/// fractions and 1.0 is the base look (`docs/specs/appearance.md` §4.9).
pub const MAX_DYE_CODE: f32 = 1.0;

/// What a figure's body is, in Tessera's neutral terms. The values are the
/// data's own (enum keys and record DIDs, opaque to this crate); the client
/// resolves them to geometry and clips.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
#[non_exhaustive]
pub enum Body {
    /// A playable body chosen by species and sex: the data's species key and
    /// sex value (`docs/specs/appearance.md` §2).
    Character {
        /// Species key.
        species: u32,
        /// Sex value.
        sex: u32,
    },
    /// A data record that describes the figure itself: a game object (an NPC
    /// or creature class) or a visual entity whose setup is the body
    /// (`docs/specs/appearance.md` §7).
    Object(DataId),
}

/// One item a figure wears or holds.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Equipped {
    /// The item's data record.
    pub item: DataId,
    /// Dye code of the first dye channel; `None` keeps the item's own default.
    /// Finite, in `0 ..= MAX_DYE_CODE`.
    pub dye: Option<f32>,
    /// Dye code of the second channel, as `dye`.
    pub secondary_dye: Option<f32>,
    /// Held items: stowed (sheathed) instead of in the hand.
    pub stowed: bool,
}

impl Equipped {
    /// An item with its default dyes, in the hand when held.
    #[must_use]
    pub const fn new(item: DataId) -> Self {
        Self {
            item,
            dye: None,
            secondary_dye: None,
            stowed: false,
        }
    }

    /// True when both dye codes are absent or finite within `0 ..= MAX_DYE_CODE`.
    #[must_use]
    pub fn is_valid(&self) -> bool {
        let ok =
            |d: Option<f32>| d.is_none_or(|v| v.is_finite() && (0.0..=MAX_DYE_CODE).contains(&v));
        ok(self.dye) && ok(self.secondary_dye)
    }
}

/// How an entity looks: enough for a client with the user's data to assemble
/// and animate it (body, scale, equipment, a starting idle). Tessera's own
/// vocabulary, not a wire format (`docs/ARCHITECTURE.md` §19.1).
#[derive(Debug, Clone, PartialEq)]
pub struct Appearance {
    /// The body.
    pub body: Body,
    /// Uniform scale on top of the body's own, in `(0, MAX_APPEARANCE_SCALE]`.
    pub scale: f32,
    /// Worn and held items, at most [`MAX_EQUIPMENT`].
    pub equipment: Vec<Equipped>,
    /// The data's starting-idle animation value (an enum key), if the figure
    /// has one (`docs/specs/appearance.md` §7.2).
    pub idle: Option<u32>,
}

impl Appearance {
    /// A body at scale 1 with nothing equipped.
    #[must_use]
    pub fn new(body: Body) -> Self {
        Self {
            body,
            scale: 1.0,
            equipment: Vec::new(),
            idle: None,
        }
    }

    /// The same appearance with `item` equipped.
    #[must_use]
    pub fn wearing(mut self, item: Equipped) -> Self {
        self.equipment.push(item);
        self
    }

    /// True when the scale is finite and in range, there are at most
    /// [`MAX_EQUIPMENT`] items and every item [is valid](Equipped::is_valid).
    #[must_use]
    pub fn is_valid(&self) -> bool {
        self.scale.is_finite()
            && self.scale > 0.0
            && self.scale <= MAX_APPEARANCE_SCALE
            && self.equipment.len() <= MAX_EQUIPMENT
            && self.equipment.iter().all(Equipped::is_valid)
    }
}
