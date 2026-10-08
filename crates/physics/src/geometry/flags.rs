// SPDX-License-Identifier: GPL-3.0-or-later
//! Per-instance collision flags and the pass-through rule (`collision.md` §9.1–§9.4).
//!
//! The physics properties of a placed object are resolved like its draw flags
//! (`entity-descriptor.md` §9): the static entity's own value wins, then the 0x47 chain
//! from the named object upward (first value found). [`CollisionFlags::or`] layers them;
//! [`CollisionFlags::of_static`] does so for a static entity, taking `Entity_CreationType`
//! from the object's chain alone (§9.3, Tessera reading; OPEN there).
//!
//! Ethereality values are bitfields over the labels of enum mapper 0x230000BA: bit *k* − 1
//! stands for key *k* (`property-system.md` §7.3). A player character belongs to **Player**
//! (key 0x1E, bit 29), the camera to **Camera** (key 2, bit 1). An object does not block a
//! mover when its effective `Physics_EtherealToType` contains a category the mover belongs
//! to (§9.2, Probable; the exact test is OPEN there).

use tessera_formats::{Did, PropertySet, PropertyValue, Setup};

/// Ethereality category bit of key `key` (bit *k* − 1), or `None` for keys outside 1–64.
#[must_use]
pub fn ethereality_bit(key: u32) -> Option<u64> {
    (1..=64).contains(&key).then(|| 1u64 << (key - 1))
}

/// `Default` (key 1, bit 0).
pub const ETHEREAL_DEFAULT: u64 = 1 << 0;
/// `Camera` (key 2, bit 1).
pub const ETHEREAL_CAMERA: u64 = 1 << 1;
/// `Player` (key 0x1E, bit 29): the category of a player character (§9.2).
pub const ETHEREAL_PLAYER: u64 = 1 << 29;

/// `Entity_CreationType` 7, UniqueDynamicMobile (`collision.md` §9.3).
pub const CREATION_TYPE_UNIQUE_DYNAMIC_MOBILE: u32 = 7;
/// `Entity_CreationType` 8, UniqueDynamicImmobile (`collision.md` §9.3).
pub const CREATION_TYPE_UNIQUE_DYNAMIC_IMMOBILE: u32 = 8;

/// The property IDs this crate reads, looked up by name in the property dictionary at run
/// time (`collision.md` §9.1). Missing names leave fields `None`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct PhysicsPids {
    /// `Physics_NoPhysics`.
    pub no_physics: Option<u32>,
    /// `Physics_AdjustableScale`.
    pub adjustable_scale: Option<u32>,
    /// `Physics_EtherealityType`.
    pub ethereality_type: Option<u32>,
    /// `Physics_EtherealToType`.
    pub ethereal_to_type: Option<u32>,
    /// `Physics_PlacementEtherealToType`.
    pub placement_ethereal_to_type: Option<u32>,
    /// `Physics_AlwaysUncontrolledSurface`.
    pub always_uncontrolled_surface: Option<u32>,
    /// `Physics_SurfacePropertiesToApply`.
    pub surface_properties_to_apply: Option<u32>,
    /// `Physics_CanLedgeClimb`.
    pub can_ledge_climb: Option<u32>,
    /// `Entity_ContentLayers` (`entity-descriptor.md` §4.5.1).
    pub content_layers: Option<u32>,
    /// `Entity_CreationType` (`collision.md` §9.3: statics of creation types 7 and 8 get no
    /// body).
    pub creation_type: Option<u32>,
}

impl PhysicsPids {
    /// Looks every PID up with `pid_of(name)`.
    #[must_use]
    pub fn from_names(pid_of: impl Fn(&str) -> Option<u32>) -> Self {
        Self {
            no_physics: pid_of("Physics_NoPhysics"),
            adjustable_scale: pid_of("Physics_AdjustableScale"),
            ethereality_type: pid_of("Physics_EtherealityType"),
            ethereal_to_type: pid_of("Physics_EtherealToType"),
            placement_ethereal_to_type: pid_of("Physics_PlacementEtherealToType"),
            always_uncontrolled_surface: pid_of("Physics_AlwaysUncontrolledSurface"),
            surface_properties_to_apply: pid_of("Physics_SurfacePropertiesToApply"),
            can_ledge_climb: pid_of("Physics_CanLedgeClimb"),
            content_layers: pid_of("Entity_ContentLayers"),
            creation_type: pid_of("Entity_CreationType"),
        }
    }

    /// Looks every PID up in the master property dictionary.
    #[must_use]
    pub fn from_dictionary(dict: &tessera_formats::PropertyDictionary) -> Self {
        Self::from_names(|n| dict.pid(n))
    }
}

/// The collision properties of §9.1 as found on one property set or layered along a chain
/// (`None` = not present).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct CollisionFlags {
    /// `Physics_NoPhysics`: true means no physics shape (no collision).
    pub no_physics: Option<bool>,
    /// `Physics_EtherealityType`: the categories the object belongs to.
    pub ethereality: Option<u64>,
    /// `Physics_EtherealToType`: the categories of movers that pass through it.
    pub ethereal_to: Option<u64>,
    /// `Physics_PlacementEtherealToType`: as above, for placement tests.
    pub placement_ethereal_to: Option<u64>,
    /// `Physics_AlwaysUncontrolledSurface`: never walkable ground (§11.3).
    pub always_uncontrolled_surface: Option<bool>,
    /// `Physics_SurfacePropertiesToApply`: the 0x78 surface material set.
    pub surface_properties: Option<Did>,
    /// `Physics_CanLedgeClimb`.
    pub can_ledge_climb: Option<bool>,
    /// `Entity_CreationType` (an enum key; `collision.md` §9.3 resolves it along the same
    /// chain, `scenery.md` §9.2 rule 2).
    pub creation_type: Option<u32>,
}

impl CollisionFlags {
    /// Reads the properties from one set.
    #[must_use]
    pub fn from_set(set: &PropertySet, pids: &PhysicsPids) -> Self {
        let get = |pid: Option<u32>| pid.and_then(|p| set.get(p));
        Self {
            no_physics: get(pids.no_physics).and_then(value_bool),
            ethereality: get(pids.ethereality_type).and_then(value_bits),
            ethereal_to: get(pids.ethereal_to_type).and_then(value_bits),
            placement_ethereal_to: get(pids.placement_ethereal_to_type).and_then(value_bits),
            always_uncontrolled_surface: get(pids.always_uncontrolled_surface).and_then(value_bool),
            surface_properties: get(pids.surface_properties_to_apply).and_then(|v| match v {
                PropertyValue::DidRef(d) if !d.is_null() => Some(*d),
                _ => None,
            }),
            can_ledge_climb: get(pids.can_ledge_climb).and_then(value_bool),
            creation_type: get(pids.creation_type).and_then(value_key),
        }
    }

    /// Fills the fields still `None` from `other` (first value found wins).
    #[must_use]
    pub fn or(self, other: Self) -> Self {
        Self {
            no_physics: self.no_physics.or(other.no_physics),
            ethereality: self.ethereality.or(other.ethereality),
            ethereal_to: self.ethereal_to.or(other.ethereal_to),
            placement_ethereal_to: self.placement_ethereal_to.or(other.placement_ethereal_to),
            always_uncontrolled_surface: self
                .always_uncontrolled_surface
                .or(other.always_uncontrolled_surface),
            surface_properties: self.surface_properties.or(other.surface_properties),
            can_ledge_climb: self.can_ledge_climb.or(other.can_ledge_climb),
            creation_type: self.creation_type.or(other.creation_type),
        }
    }

    /// The flags of a static entity: its own values (`own`, from the SED's property set)
    /// over its object's (`object`, layered along the 0x47 chain), first value found wins
    /// (§9.1), except `Entity_CreationType`, which comes from the object's chain alone
    /// (§9.3: "resolved along the chain as `scenery.md` §9.2 rule 2"; Tessera reading,
    /// recorded as an OPEN in `collision.md` §9.3: it reproduces the CO12 counts, while
    /// the SED-first precedence would drop no SED of the install).
    #[must_use]
    pub fn of_static(own: Self, object: Self) -> Self {
        Self {
            creation_type: object.creation_type,
            ..own.or(object)
        }
    }

    /// Whether the effective `Entity_CreationType` is 7 (UniqueDynamicMobile) or 8
    /// (UniqueDynamicImmobile): the server creates such entities, and the client drops
    /// landblock-info statics of these types on the static-creation path (`collision.md`
    /// §9.3, §9.4, C25, BIN). Generated objects are not filtered by it (§9.4).
    #[must_use]
    pub fn is_server_created(&self) -> bool {
        matches!(
            self.creation_type,
            Some(CREATION_TYPE_UNIQUE_DYNAMIC_MOBILE | CREATION_TYPE_UNIQUE_DYNAMIC_IMMOBILE)
        )
    }

    /// Whether the effective `Physics_NoPhysics` is true (the instance has no shape).
    #[must_use]
    pub fn has_no_physics(&self) -> bool {
        self.no_physics == Some(true)
    }

    /// The effective `Physics_EtherealToType` (0 when absent).
    #[must_use]
    pub fn ethereal_to_bits(&self) -> u64 {
        self.ethereal_to.unwrap_or(0)
    }

    /// Whether a mover of `categories` passes through the object (§9.2).
    #[must_use]
    pub fn passes(&self, categories: u64) -> bool {
        self.ethereal_to_bits() & categories != 0
    }

    /// Whether the effective `Physics_AlwaysUncontrolledSurface` is true (default false).
    #[must_use]
    pub fn is_always_uncontrolled(&self) -> bool {
        self.always_uncontrolled_surface == Some(true)
    }
}

/// A boolean-like value: boolean, or a non-zero integer or enum key.
fn value_bool(v: &PropertyValue) -> Option<bool> {
    match v {
        PropertyValue::Bool(b) => Some(*b),
        PropertyValue::Int(i) => Some(*i != 0),
        PropertyValue::Enum(k) => Some(*k != 0),
        _ => None,
    }
}

/// An enum-like value as its key: an enum key, or a non-negative integer.
fn value_key(v: &PropertyValue) -> Option<u32> {
    match v {
        PropertyValue::Enum(k) | PropertyValue::Bitfield32(k) => Some(*k),
        PropertyValue::Int(i) => u32::try_from(*i).ok(),
        PropertyValue::Int64(i) => u32::try_from(*i).ok(),
        _ => None,
    }
}

/// A bitfield-like value as its low 64 bits.
fn value_bits(v: &PropertyValue) -> Option<u64> {
    match v {
        PropertyValue::Bitfield32(b) | PropertyValue::Enum(b) => Some(u64::from(*b)),
        PropertyValue::Bitfield64(b) => Some(*b),
        #[allow(clippy::cast_sign_loss)]
        PropertyValue::Int(i) => Some(u64::from(*i as u32)),
        #[allow(clippy::cast_sign_loss)]
        PropertyValue::Int64(i) => Some(*i as u64),
        PropertyValue::BitfieldAny(b) => Some(
            b.set_bits()
                .take_while(|&i| i < 64)
                .fold(0u64, |acc, i| acc | (1 << i)),
        ),
        _ => None,
    }
}

/// What collision a setup has, for the rules of §9.3–§9.4 (`setup.md` §6.5).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum SetupCollision {
    /// No collision mesh and no volume.
    #[default]
    None,
    /// A 0x66 collision mesh (with or without volumes: the 10 mixtures with a mesh count
    /// here).
    Mesh,
    /// Volumes only (boxes, spheres, capsules).
    Volumes,
}

impl SetupCollision {
    /// Classifies a setup.
    #[must_use]
    pub fn of(setup: &Setup) -> Self {
        if !setup.collision_mesh.is_null() {
            Self::Mesh
        } else if !setup.boxes.is_empty() || !setup.spheres.is_empty() || !setup.capsules.is_empty()
        {
            Self::Volumes
        } else {
            Self::None
        }
    }

    /// Whether there is any collision.
    #[must_use]
    pub fn has_collision(self) -> bool {
        self != Self::None
    }
}

/// The content-layer test of `entity-descriptor.md` §4.5.1. The entity exists when it has
/// no list (`None`), an empty list, or an element *e* with *e* = 2, *e* = 1 and *I* = 0
/// (whatever the active layers), or, for some active layer *L* in `active`, *e* = 1 and
/// *L* = 1, or *e* = *L* (the union over active layers, Tessera convention; an empty
/// `active` means no layer is active; Tessera's offline default is `active` = `[1]`,
/// `instance` = 0).
#[must_use]
pub fn content_layers_pass(list: Option<&[u32]>, active: &[u32], instance: u32) -> bool {
    match list {
        None | Some([]) => true,
        Some(l) => l.iter().any(|&e| {
            e == 2
                || (e == 1 && instance == 0)
                || active
                    .iter()
                    .any(|&layer| e == layer || (e == 1 && layer == 1))
        }),
    }
}

/// The `Entity_ContentLayers` list of a property set: the array's integer-like elements,
/// or `None` when the property is absent or not an array (both count as "no list").
#[must_use]
pub fn content_layer_list(set: &PropertySet, pids: &PhysicsPids) -> Option<Vec<u32>> {
    let PropertyValue::Array(a) = set.get(pids.content_layers?)? else {
        return None;
    };
    Some(
        a.elements
            .iter()
            .filter_map(|el| match &el.value {
                PropertyValue::Enum(k) | PropertyValue::Bitfield32(k) => Some(*k),
                #[allow(clippy::cast_sign_loss)]
                PropertyValue::Int(i) => Some(*i as u32),
                PropertyValue::DidRef(d) => Some(d.0),
                _ => None,
            })
            .collect(),
    )
}

/// The walker-blocking rule for a static entity (`collision.md` §9.3, Decision from the
/// census and BIN): a SED blocks a player-character walker when it is a Physics SED with a
/// position, it passes the content-layer test, its effective `Physics_NoPhysics` is not
/// true, its setup has a 0x66 mesh or volumes, its effective `Physics_EtherealToType` does
/// not contain Player, and its effective `Entity_CreationType` is not 7 or 8
/// ([`CollisionFlags::is_server_created`]). `flags` are the SED's, layered with
/// [`CollisionFlags::of_static`]. Draw flags play no part. (The §9.3 data census
/// applies neither the content-layer nor the creation-type filter: pass `true` and flags
/// without a creation type to count it.)
#[must_use]
pub fn static_blocks_walker(
    is_physics_with_position: bool,
    content_layers_pass: bool,
    flags: &CollisionFlags,
    collision: SetupCollision,
) -> bool {
    is_physics_with_position
        && content_layers_pass
        && !flags.has_no_physics()
        && collision.has_collision()
        && !flags.passes(ETHEREAL_PLAYER)
        && !flags.is_server_created()
}

/// The walker-blocking rule for a procedurally generated object (`collision.md` §9.4,
/// Tessera Decision): effective `Physics_NoPhysics` (along its 0x47 chain) not true, setup
/// with a mesh or volumes, `Physics_EtherealToType` without Player. No content-layer or
/// position filter. Generated objects of client-only creation types get bodies too
/// (Tessera Decision of the §9.4 OPEN).
#[must_use]
pub fn generated_blocks_walker(flags: &CollisionFlags, collision: SetupCollision) -> bool {
    !flags.has_no_physics() && collision.has_collision() && !flags.passes(ETHEREAL_PLAYER)
}
