// SPDX-License-Identifier: GPL-3.0-or-later
//! Walkable, sliding and wall contacts (`collision.md` §11.3).
//!
//! The angles, the normal-merge tolerance and the support radius are constants of the
//! client (BIN), which decision 0012 allows here.

use crate::geometry::{CollisionQuery, Contact};

use super::state::Support;

/// Walkable limit for player characters, degrees (§11.3).
pub const PLAYER_WALKABLE_DEGREES: f32 = 45.0;
/// Walkable limit for other movers, degrees (§11.3).
pub const OTHER_WALKABLE_DEGREES: f32 = 70.0;
/// Beyond this angle from up a contact is a wall, not support, degrees (§11.3).
pub const WALL_DEGREES: f32 = 85.0;
/// cos 45°.
pub const COS_PLAYER_WALKABLE: f32 = 0.707_106_77;
/// cos 70°.
pub const COS_OTHER_WALKABLE: f32 = 0.342_020_14;
/// cos 85°.
pub const COS_WALL: f32 = 0.087_155_74;
/// Contact normals that differ by at most this much in every component are merged into one
/// plane (§11.3).
pub const NORMAL_MERGE_TOLERANCE: f32 = 0.035;
/// The support search radius is at least this (§11.3), m.
pub const SUPPORT_MIN_RADIUS: f32 = 0.3;

/// How one contact counts (§11.3 table).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ContactClass {
    /// The contacted object has `Physics_AlwaysUncontrolledSurface`: ignored as support.
    Ignored,
    /// The chosen normal points down (u < 0): an overhang, ignored as support.
    Overhang,
    /// u > cos α: walkable ("controlled") support.
    Walkable,
    /// cos 85° < u ≤ cos α: steep ("uncontrolled") support; the mover slides.
    Steep,
    /// Otherwise: a wall, not support.
    Wall,
}

impl ContactClass {
    /// The support this contact gives.
    #[must_use]
    pub fn support(self) -> Support {
        match self {
            Self::Walkable => Support::Walkable,
            Self::Steep => Support::Steep,
            Self::Ignored | Self::Overhang | Self::Wall => Support::None,
        }
    }
}

/// The cosine of the walkable limit: 45° for player characters, 70° for other movers.
#[must_use]
pub fn walkable_cos(is_player: bool) -> f32 {
    if is_player {
        COS_PLAYER_WALKABLE
    } else {
        COS_OTHER_WALKABLE
    }
}

/// Whether the mover with 64-bit instance ID `iid` is a player character: (iid ≫ 56) = 2
/// (§11.3, BIN). An offline client has no server IDs; Tessera treats the local avatar as a
/// player character (the controller's `is_player`).
#[must_use]
pub fn is_player_instance(iid: u64) -> bool {
    iid >> 56 == 2
}

/// The normal §11.3 classifies: the unit contact normal, or the triangle's face normal
/// (turned to the mover's side) when that is steeper than the walkable limit and less
/// upward than the contact normal.
#[must_use]
pub fn chosen_normal(
    normal: [f32; 3],
    face_normal: Option<[f32; 3]>,
    cos_walkable: f32,
) -> [f32; 3] {
    match face_normal {
        Some(f) if f.iter().all(|c| c.is_finite()) && f[2] <= cos_walkable && f[2] < normal[2] => f,
        _ => normal,
    }
}

/// Classifies one contact (§11.3 table).
#[must_use]
pub fn classify(
    normal: [f32; 3],
    face_normal: Option<[f32; 3]>,
    always_uncontrolled: bool,
    cos_walkable: f32,
) -> ContactClass {
    if always_uncontrolled {
        return ContactClass::Ignored;
    }
    let u = chosen_normal(normal, face_normal, cos_walkable)[2];
    if !u.is_finite() || u < 0.0 {
        ContactClass::Overhang
    } else if u > cos_walkable {
        ContactClass::Walkable
    } else if u > COS_WALL {
        ContactClass::Steep
    } else {
        ContactClass::Wall
    }
}

/// Whether the body behind a contact has `Physics_AlwaysUncontrolledSurface` true.
pub(crate) fn always_uncontrolled<Q: CollisionQuery + ?Sized>(
    q: &Q,
    set: u32,
    body: crate::geometry::BodyId,
) -> bool {
    q.body(set, body)
        .is_some_and(|b| b.flags.always_uncontrolled_surface == Some(true))
}

/// The best support among `contacts` (walkable over steep over none).
pub fn best_support<Q: CollisionQuery + ?Sized>(
    q: &Q,
    contacts: &[Contact],
    cos_walkable: f32,
) -> Support {
    contacts
        .iter()
        .map(|c| {
            classify(
                c.normal,
                c.face_normal,
                always_uncontrolled(q, c.set, c.body),
                cos_walkable,
            )
            .support()
        })
        .max()
        .unwrap_or(Support::None)
}

/// Whether two unit normals merge into one plane (every component within
/// [`NORMAL_MERGE_TOLERANCE`]).
#[must_use]
pub fn normals_merge(a: [f32; 3], b: [f32; 3]) -> bool {
    (0..3).all(|i| (a[i] - b[i]).abs() <= NORMAL_MERGE_TOLERANCE)
}
