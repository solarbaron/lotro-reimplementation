// SPDX-License-Identifier: GPL-3.0-or-later
//! Per-instance collision flags and the pass-through rule (`collision.md` §9.1–§9.4).
//!
//! The physics properties of a placed object are resolved like its draw flags
//! (`entity-descriptor.md` §9): the static entity's own value wins, then the 0x47 chain
//! from the named object upward (first value found). [`CollisionFlags::or`] layers them;
//! [`CollisionFlags::of_static`] does so for a static entity, `Entity_CreationType`
//! included (§9.3 item 3, C29: the SED's own value first, then the chain, as
//! `entity-descriptor.md` §4.9 E24). [`EntityLookups`] does the same for
//! `Entity_ContentLayers`, `Physics_AdjustableScale` and `Model_Scale` (the paragraph after
//! §9.3 item 3); [`physics_scale`] turns the two scale vectors into the uniform physics scale
//! of §8.1 (BIN, C42).
//!
//! Value types follow the client's readers (§9.1, §9.3, C41): the booleans only as a Boolean
//! (another type counts as absent, Tessera rule confirmed there), the ethereality values only
//! as a Bitfield32 (another type reads as no categories), the creation type only as an enum
//! or property-ID value (another type reads as 0), the scales only as vectors.
//!
//! Ethereality values are bitfields over the labels of enum mapper 0x230000BA: bit *k* − 1
//! stands for key *k* (`property-system.md` §7.3). A player character belongs to **Player**
//! (key 0x1E, bit 29), the camera to **Camera** (key 2, bit 1). An object does not block a
//! mover when its effective `Physics_EtherealToType` contains a category the mover belongs
//! to (§9.2, Probable; the exact test is OPEN there).

use tessera_formats::landblock::MAX_SCALE;
use tessera_formats::{Did, PropertySet, PropertyValue, Setup};

/// The scale tolerance of `collision.md` §8.1 (C31, C42): a vector whose x differs from its
/// y or z by more than this is non-uniform; a physics scale below it becomes 1; a scale
/// within it of 1 counts as 1.
pub const SCALE_TOLERANCE: f32 = 0.0002;

/// Tessera's upper bound on the physics scale (`collision.md` §9.3 point 2, hostile input,
/// decision 0013): a larger product becomes 1. The client has none.
pub const MAX_PHYSICS_SCALE: f32 = MAX_SCALE;

/// The uniform physics scale *s* of `collision.md` §8.1 steps 1–4 (BIN, C42) from the
/// `Physics_AdjustableScale` and `Model_Scale` vectors found for the entity (`None`: not
/// found or not a vector, read as (1, 1, 1)): a non-uniform vector (x differing from y or z
/// by more than [`SCALE_TOLERANCE`]) is logged and its z used, *s* = z × z; a product that
/// is not finite, below [`SCALE_TOLERANCE`] (logged) or above [`MAX_PHYSICS_SCALE`] (Tessera
/// bound) becomes 1, and one within [`SCALE_TOLERANCE`] of 1 is 1 (step 4: the initial scale
/// 1 is replaced only by a value that differs from it by more than that).
#[must_use]
pub fn physics_scale(adjustable: Option<[f32; 3]>, model: Option<[f32; 3]>) -> f32 {
    let z_of = |v: Option<[f32; 3]>, name: &'static str| {
        let [x, y, z] = v.unwrap_or([1.0; 3]);
        if !((x - y).abs() <= SCALE_TOLERANCE && (x - z).abs() <= SCALE_TOLERANCE) {
            tracing::debug!(
                property = name,
                ?v,
                "non-uniform scale: its z is used (collision.md §8.1)"
            );
        }
        z
    };
    let s = z_of(adjustable, "Physics_AdjustableScale") * z_of(model, "Model_Scale");
    if !s.is_finite() || s > MAX_PHYSICS_SCALE {
        return 1.0;
    }
    if s < SCALE_TOLERANCE {
        tracing::debug!(
            scale = s,
            "physics scale below 0.0002: 1 is used (collision.md §8.1)"
        );
        return 1.0;
    }
    if (s - 1.0).abs() <= SCALE_TOLERANCE {
        1.0
    } else {
        s
    }
}

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

/// `Entity_CreationType` 0, `Invalid`: the loader rejects the SED (`collision.md` §9.3
/// item 3, `entity-descriptor.md` §4.9). Also the value of a SED whose lookup finds no
/// value, or a value that is not an enum (type 8) or property-ID (type 10) value.
pub const CREATION_TYPE_INVALID: u32 = 0;
/// `Entity_CreationType` 3, SharedStaticImmobile (`entity-descriptor.md` §4.9, class
/// Static).
pub const CREATION_TYPE_SHARED_STATIC_IMMOBILE: u32 = 3;
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
    /// `Model_Scale` (property 0x3C3; the second factor of the physics scale, §8.1).
    pub model_scale: Option<u32>,
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
    /// `Entity_CreationType` (`collision.md` §9.3 item 3: statics of creation type 0, 7 or
    /// 8 get no body, and so do all statics when the dictionary lacks the name).
    pub creation_type: Option<u32>,
}

impl PhysicsPids {
    /// Looks every PID up with `pid_of(name)`.
    #[must_use]
    pub fn from_names(pid_of: impl Fn(&str) -> Option<u32>) -> Self {
        Self {
            no_physics: pid_of("Physics_NoPhysics"),
            adjustable_scale: pid_of("Physics_AdjustableScale"),
            model_scale: pid_of("Model_Scale"),
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
    /// `Entity_CreationType` as found (`None`: not found at this level or along the
    /// chain). A found value that is not an enum (type 8) or property-ID (type 10) value
    /// reads as [`CREATION_TYPE_INVALID`] and still ends the lookup (`collision.md` §9.3
    /// item 3 and point 1, C41).
    pub creation_type: Option<u32>,
}

impl CollisionFlags {
    /// Reads the properties from one set.
    #[must_use]
    pub fn from_set(set: &PropertySet, pids: &PhysicsPids) -> Self {
        let get = |pid: Option<u32>| pid.and_then(|p| set.get(p));
        Self {
            no_physics: get(pids.no_physics).and_then(value_bool),
            ethereality: get(pids.ethereality_type).map(value_bits),
            ethereal_to: get(pids.ethereal_to_type).map(value_bits),
            placement_ethereal_to: get(pids.placement_ethereal_to_type).map(value_bits),
            always_uncontrolled_surface: get(pids.always_uncontrolled_surface).and_then(value_bool),
            surface_properties: get(pids.surface_properties_to_apply).and_then(|v| match v {
                PropertyValue::DidRef(d) if !d.is_null() => Some(*d),
                _ => None,
            }),
            can_ledge_climb: get(pids.can_ledge_climb).and_then(value_bool),
            creation_type: get(pids.creation_type).map(value_creation_type),
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
    /// (§9.1), `Entity_CreationType` included (§9.3 item 3, C29, CO14: the SED's own value
    /// first, else the chain's).
    #[must_use]
    pub fn of_static(own: Self, object: Self) -> Self {
        own.or(object)
    }

    /// The effective `Entity_CreationType`: the value found, or [`CREATION_TYPE_INVALID`]
    /// when the lookup found none (`collision.md` §9.3 item 3).
    #[must_use]
    pub fn creation_type_value(&self) -> u32 {
        self.creation_type.unwrap_or(CREATION_TYPE_INVALID)
    }

    /// Whether the effective `Entity_CreationType` is 7 (UniqueDynamicMobile) or 8
    /// (UniqueDynamicImmobile), the Dynamic class: the client does not create such
    /// entities from the data (`collision.md` §9.3 item 3, `entity-descriptor.md` §4.9).
    #[must_use]
    pub fn is_server_created(&self) -> bool {
        matches!(
            self.creation_type_value(),
            CREATION_TYPE_UNIQUE_DYNAMIC_MOBILE | CREATION_TYPE_UNIQUE_DYNAMIC_IMMOBILE
        )
    }

    /// Whether the client's static-creation path refuses a landblock-info static with these
    /// flags (`collision.md` §9.3 item 3, C29): its effective `Entity_CreationType` is 0
    /// (`Invalid`, also when no value was found or the value found is not an enum or
    /// property-ID value), 7 or 8. Generated objects are not filtered by it (§9.4).
    #[must_use]
    pub fn refuses_static_creation(&self) -> bool {
        self.creation_type_value() == CREATION_TYPE_INVALID || self.is_server_created()
    }

    /// Whether the effective `Physics_NoPhysics` is true (the instance has no shape).
    #[must_use]
    pub fn has_no_physics(&self) -> bool {
        self.no_physics == Some(true)
    }

    /// The flags a cell's physics-mesh body carries (`collision.md` §5.3, Tessera
    /// Decision): only `Physics_SurfacePropertiesToApply` and `Physics_CanLedgeClimb` of
    /// the cell record's own property set. Neither changes whether the body blocks a mover,
    /// so a cell body is never passed through, always counts as support (§11.3) and is
    /// built whatever `Physics_NoPhysics` says.
    #[must_use]
    pub fn cell_body(self) -> Self {
        Self {
            surface_properties: self.surface_properties,
            can_ledge_climb: self.can_ledge_climb,
            ..Self::default()
        }
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

/// A boolean value. A value of another type counts as not present, so the lookup goes on
/// along the chain (`collision.md` §9.1 "Value types", C41: Tessera rule confirmed; the
/// client reads it as false, the same result under the dictionary's types).
fn value_bool(v: &PropertyValue) -> Option<bool> {
    match v {
        PropertyValue::Bool(b) => Some(*b),
        _ => None,
    }
}

/// A found `Entity_CreationType` value: an enum (type 8) or property-ID (type 10) value as
/// stored; anything else, a 32-bit or 64-bit integer included, counts as 0 (`collision.md`
/// §9.3 item 3 and point 1, BIN, C41).
fn value_creation_type(v: &PropertyValue) -> u32 {
    match v {
        PropertyValue::Enum(k) | PropertyValue::PropertyId(k) => *k,
        _ => CREATION_TYPE_INVALID,
    }
}

/// An ethereality value (`collision.md` §9.1 "Value types", BIN, C41): a Bitfield32
/// (type 18) as its 32 bits; a value of any other type, the 64-bit and variable-width
/// bitfields included, reads as no categories and ends the lookup, as in the client.
fn value_bits(v: &PropertyValue) -> u64 {
    match v {
        PropertyValue::Bitfield32(b) => u64::from(*b),
        _ => 0,
    }
}

/// A scale vector (type 6); a value of another type reads as (1, 1, 1) (`collision.md`
/// §8.1 step 1, C41).
fn value_vector(v: &PropertyValue) -> [f32; 3] {
    match v {
        PropertyValue::Vector3(c) => *c,
        _ => [1.0; 3],
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
    layer_list_of(set.get(pids.content_layers?)?)
}

/// The list of a found `Entity_ContentLayers` value (`None`: not an array, "no list",
/// `entity-descriptor.md` §4.5.1 item 1).
fn layer_list_of(v: &PropertyValue) -> Option<Vec<u32>> {
    let PropertyValue::Array(a) = v else {
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

/// The other properties the client reads "of a SED" with the lookup of
/// `entity-descriptor.md` §4.9 (E24): the SED's own set first, then its 0x47 chain, first
/// level that has the property wins (`collision.md` §9.3, the paragraph after item 3).
/// [`EntityLookups::or`] layers them like [`CollisionFlags::or`]; the resolver fills them
/// along the chain ([`super::ObjectCollision::lookups`]).
#[derive(Debug, Clone, PartialEq, Default)]
pub struct EntityLookups {
    /// `Entity_ContentLayers` as first found: `None` when no level has it; `Some(None)`
    /// when the value found is not an array (no list, the SED exists); `Some(Some(list))`
    /// otherwise.
    pub content_layers: Option<Option<Vec<u32>>>,
    /// `Physics_AdjustableScale` as first found: the vector when it is one, else (1, 1, 1);
    /// a found value of another type still ends the lookup (`collision.md` §9.3 point 2,
    /// C41). [`EntityLookups::physics_scale`] applies the range rules of §8.1.
    pub adjustable_scale: Option<[f32; 3]>,
    /// `Model_Scale` as first found, read like `adjustable_scale` (§8.1 step 1).
    pub model_scale: Option<[f32; 3]>,
}

impl EntityLookups {
    /// Reads the properties from one set.
    #[must_use]
    pub fn from_set(set: &PropertySet, pids: &PhysicsPids) -> Self {
        let get = |pid: Option<u32>| pid.and_then(|p| set.get(p));
        Self {
            content_layers: get(pids.content_layers).map(layer_list_of),
            adjustable_scale: get(pids.adjustable_scale).map(value_vector),
            model_scale: get(pids.model_scale).map(value_vector),
        }
    }

    /// Fills the fields still `None` from `other` (first value found wins).
    #[must_use]
    pub fn or(self, other: Self) -> Self {
        Self {
            content_layers: self.content_layers.or(other.content_layers),
            adjustable_scale: self.adjustable_scale.or(other.adjustable_scale),
            model_scale: self.model_scale.or(other.model_scale),
        }
    }

    /// The effective layer list (`None`: no list, the content-layer test passes).
    #[must_use]
    pub fn layer_list(&self) -> Option<&[u32]> {
        self.content_layers.as_ref()?.as_deref()
    }

    /// The uniform physics scale of `collision.md` §8.1 ([`physics_scale`] of the two
    /// vectors found; 1 when nothing was found).
    #[must_use]
    pub fn physics_scale(&self) -> f32 {
        physics_scale(self.adjustable_scale, self.model_scale)
    }

    /// The scale an object's shape is placed with: the physics scale on every axis
    /// (`collision.md` §8.1, §8.3 "Scale").
    #[must_use]
    pub fn scale(&self) -> [f32; 3] {
        [self.physics_scale(); 3]
    }

    /// Approximate heap bytes.
    #[must_use]
    pub fn heap_size(&self) -> usize {
        self.content_layers
            .as_ref()
            .and_then(Option::as_ref)
            .map_or(0, |l| 4 * l.capacity())
    }
}

/// Whether a static with these flags and setup collision blocks a player-character walker
/// once it is created: effective `Physics_NoPhysics` not true, a setup with a 0x66 mesh or
/// volumes, and an effective `Physics_EtherealToType` without Player (`collision.md` §9.3,
/// the paragraph after item 3). This is the collision test of the §9.3 data census, which
/// applies neither run-time filter; [`static_blocks_walker`] adds them.
#[must_use]
pub fn static_has_walker_collision(flags: &CollisionFlags, collision: SetupCollision) -> bool {
    !flags.has_no_physics() && collision.has_collision() && !flags.passes(ETHEREAL_PLAYER)
}

/// The walker-blocking rule for a static entity (`collision.md` §9.3, BIN C25, C29): a SED
/// blocks a player-character walker when it is a Physics SED with a position (item 1), it
/// passes the content-layer test (item 2, the list found by [`EntityLookups`]), the client
/// creates it ([`CollisionFlags::refuses_static_creation`] is false: effective
/// `Entity_CreationType` not 0, 7 or 8, item 3), and [`static_has_walker_collision`] holds.
/// `flags` are the SED's, layered with [`CollisionFlags::of_static`]. Draw flags play no
/// part.
#[must_use]
pub fn static_blocks_walker(
    is_physics_with_position: bool,
    content_layers_pass: bool,
    flags: &CollisionFlags,
    collision: SetupCollision,
) -> bool {
    is_physics_with_position
        && content_layers_pass
        && !flags.refuses_static_creation()
        && static_has_walker_collision(flags, collision)
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
