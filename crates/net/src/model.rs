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
//! Positions are in metres in a right-handed world frame with **+Z up**.
//! `heading` is in radians about +Z, `0` pointing along +X, increasing
//! counter-clockwise. Mapping to region/cell/landblock coordinates of any
//! particular data set is the adapter's job.

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
