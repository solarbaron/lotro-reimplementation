// SPDX-License-Identifier: GPL-3.0-or-later
//! Helpers shared by the gameplay record families.
//!
//! | Item | Purpose | Spec |
//! |---|---|---|
//! | [`ObjectRoot`] | Header-only read of an object record: own DID and root class (offset 4) | `wstate.md` §3.1, §12 |
//! | [`GameObjectRecord`], [`decode_object_properties`] | An object record's root class with its property set at DID + 0x09000000 | `property-system.md` §6.2 |
//! | [`ClassParents`], [`ClassTree`], [`Lineage`] | Class ancestry over the class library's parent map, bounded against cycles | `wlib.md` §9 |
//! | [`ClassTest`], [`classify_class`] | Ordered "class is / descends from" rule tables | `combat-data.md` §2.1, `items.md` §2.4 |
//! | [`ClassIndex`] | DID → root class and class → DIDs, queries by ancestor | `items.md` §2.4, `creatures.md` §2.2 |
//! | [`Pid`], [`property_ids!`](crate::property_ids) | Property IDs resolved from dictionary labels at run time | `property-system.md` §2 |
//! | [`PropertyAccess`], [`ValueExt`], [`PropertyDefaults`] | Typed access to sets and structs; dictionary defaults for absent values | `property-system.md` §2.2, §4 |
//! | [`Signature`], [`classify_by_signature`] | Kinds recognised by the properties they carry | `economy.md` §3.4, `quests.md` §2.1, `map-data.md` §2.3 |
//! | [`DidReference`], [`did_references`], [`ReverseDidIndex`] | Every non-null DID value of a set with its path; inverted graph with a bounded walk | `items.md` §8, `economy.md` §10.2 |
//! | [`enum_mapper_of`], root DID-map keys | Where a property's enum labels and the well-known singletons live | `property-system.md` §7.1, §8.1 |
//! | [`build`] | Small constructors for synthetic property trees (tests, fuzzing) | — |
//!
//! Class indices, PIDs and DIDs are build-specific. Nothing here hard-codes one: callers
//! pass the parent map, resolve labels against the install's dictionary and look singletons
//! up by DID-map label.
//!
//! The class library itself is decoded by `tessera-wstate`, which depends on this crate;
//! [`ClassParents`] is the seam (`tessera-wstate` implements it for its class library).

use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet, VecDeque};

use crate::did::Did;
use crate::error::{FormatError, Result};
use crate::property::{
    Position, PropertyArray, PropertyDictionary, PropertyEntry, PropertyNames, PropertySchema,
    PropertyStruct, PropertyType, PropertyValue,
};
use crate::property_set::{PropertySet, PropertySetRecord, object_property_set_did};
use crate::reader::Reader;
use crate::string_info::StringInfo;

// ---------------------------------------------------------------------------------------------
// Object records
// ---------------------------------------------------------------------------------------------

/// Length of the part of an object record that [`ObjectRoot::read`] reads: DID and root class.
pub const OBJECT_ROOT_LEN: usize = 8;

/// Name used in errors for object-state records.
const OBJECT_RECORD: &str = "object-state record";

/// Whether `did` is in the object-state range 0x70000000–0x77FFFFFF (`wstate.md` §2).
#[must_use]
pub fn is_object_did(did: impl Into<Did>) -> bool {
    matches!(did.into().prefix(), 0x70..=0x77)
}

/// The first two fields of an object-state record (`wstate.md` §3.1): its own DID and the
/// class index of its root item. Most gameplay readers need nothing else from the 0x70
/// record; every gameplay value is in the property set (`wstate.md` §12, "two decode
/// depths").
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ObjectRoot {
    /// The record's DID (prefix 0x70–0x77).
    pub did: Did,
    /// Class index of the root item (offset 4). Build-specific; interpret it only through
    /// the parent map ([`ClassParents`]).
    pub root_class: u32,
}

impl ObjectRoot {
    /// Reads the DID and root class from the start of an object-state payload. The rest of
    /// the payload is not examined.
    ///
    /// Fails with [`FormatError::Truncated`] on fewer than [`OBJECT_ROOT_LEN`] bytes and with
    /// [`FormatError::WrongRecordType`] when the DID is outside 0x70–0x77.
    pub fn read(payload: &[u8]) -> Result<Self> {
        let mut r = Reader::new(payload);
        let did = r.did()?;
        if !is_object_did(did) {
            return Err(FormatError::WrongRecordType {
                did: did.0,
                expected: OBJECT_RECORD,
            });
        }
        let root_class = r.u32()?;
        Ok(Self { did, root_class })
    }

    /// [`ObjectRoot::read`], then checks that the record's DID is the one looked up
    /// ([`FormatError::DidMismatch`] otherwise).
    pub fn read_expecting(payload: &[u8], expected: impl Into<Did>) -> Result<Self> {
        let expected = expected.into();
        let root = Self::read(payload)?;
        if root.did != expected {
            return Err(FormatError::DidMismatch {
                expected: expected.0,
                found: root.did.0,
            });
        }
        Ok(root)
    }

    /// The DID of the record's property set (DID + 0x09000000), or `None` for prefix 0x77
    /// ([`object_property_set_did`], `property-system.md` §6.2). Singletons (0x76) get a
    /// 0x7F DID that no archive holds (`wstate.md` §2): a lookup miss means "no properties".
    #[must_use]
    pub fn property_set_did(&self) -> Option<Did> {
        object_property_set_did(self.did)
    }
}

/// Decodes the property set of game object `object` from its 0x79 payload, checking that the
/// payload's DID is `object` + 0x09000000.
///
/// Fails with [`FormatError::WrongRecordType`] when `object` has no property-set DID (not a
/// 0x70–0x76 DID), and with the errors of [`PropertySetRecord::parse_expecting`].
pub fn decode_object_properties<S: PropertySchema + ?Sized>(
    object: impl Into<Did>,
    payload: &[u8],
    schema: &S,
) -> Result<PropertySet> {
    let object = object.into();
    let Some(set_did) = object_property_set_did(object) else {
        return Err(FormatError::WrongRecordType {
            did: object.0,
            expected: "game object with a property set",
        });
    };
    PropertySetRecord::parse_expecting(payload, set_did, schema).map(|rec| rec.set)
}

/// A game object as gameplay readers see it: the root class of its 0x70 record and its
/// property set (`items.md` §3.1, `combat-data.md` §3.1, `economy.md` §1).
#[derive(Debug, Clone, PartialEq)]
pub struct GameObjectRecord {
    /// DID and root class.
    pub root: ObjectRoot,
    /// The property set at DID + 0x09000000; empty when the object has none.
    pub properties: PropertySet,
}

impl GameObjectRecord {
    /// Decodes the header of `object` (a 0x70 payload) and, when given, its property-set
    /// payload. A missing property set gives an empty set ("no properties",
    /// `property-system.md` §6.2).
    pub fn decode<S: PropertySchema + ?Sized>(
        object: &[u8],
        property_set: Option<&[u8]>,
        schema: &S,
    ) -> Result<Self> {
        let root = ObjectRoot::read(object)?;
        let properties = match property_set {
            Some(p) => decode_object_properties(root.did, p, schema)?,
            None => PropertySet::default(),
        };
        Ok(Self { root, properties })
    }

    /// The record's DID.
    #[must_use]
    pub fn did(&self) -> Did {
        self.root.did
    }

    /// The record's root class.
    #[must_use]
    pub fn root_class(&self) -> u32 {
        self.root.root_class
    }
}

// ---------------------------------------------------------------------------------------------
// Class ancestry
// ---------------------------------------------------------------------------------------------

/// Parent-map value meaning "no parent" (`wlib.md` §9).
pub const NO_PARENT: u32 = 0xFFFF_FFFF;

/// Read access to the class library's parent map (`wlib.md` §9).
///
/// Implementations must return `None` for "no parent" (including [`NO_PARENT`]). Walks are
/// bounded by [`ClassParents::step_limit`], so a cyclic map cannot loop.
pub trait ClassParents {
    /// The parent of `class`, if any.
    fn parent(&self, class: u32) -> Option<u32>;

    /// Upper bound on the length of any ancestor chain: the number of classes in the map.
    fn step_limit(&self) -> usize;

    /// `class` itself, then its ancestors, nearest first.
    fn lineage(&self, class: u32) -> Lineage<'_, Self> {
        Lineage {
            parents: self,
            next: Some(class),
            left: self.step_limit().saturating_add(1),
        }
    }

    /// Whether `class` is `ancestor` or derives from it.
    fn is_a(&self, class: u32, ancestor: u32) -> bool {
        self.lineage(class).any(|c| c == ancestor)
    }

    /// Whether `class` derives from `ancestor` (strictly: `class` ≠ `ancestor`).
    fn is_below(&self, class: u32, ancestor: u32) -> bool {
        self.lineage(class).skip(1).any(|c| c == ancestor)
    }

    /// The deepest class that is an ancestor-or-self of every class in `classes`
    /// (`items.md` §2.4 *P*, `creatures.md` §2.2 step 3). `None` for an empty input or
    /// classes with no common ancestor.
    fn common_ancestor(&self, classes: impl IntoIterator<Item = u32>) -> Option<u32> {
        let distinct: BTreeSet<u32> = classes.into_iter().collect();
        let mut it = distinct.into_iter();
        let mut candidates: Vec<u32> = self.lineage(it.next()?).collect();
        for c in it {
            let line: HashSet<u32> = self.lineage(c).collect();
            let keep = candidates.iter().position(|a| line.contains(a))?;
            candidates.drain(..keep);
        }
        candidates.first().copied()
    }

    /// The child of `ancestor` on the chain from `class` up to `ancestor` (`items.md` §2.4
    /// *I*: the child of *P* whose subtree holds a reference). `None` when `class` is
    /// `ancestor` or does not derive from it.
    fn child_toward(&self, ancestor: u32, class: u32) -> Option<u32> {
        let mut prev = None;
        for c in self.lineage(class) {
            if c == ancestor {
                return prev;
            }
            prev = Some(c);
        }
        None
    }

    /// Number of ancestors of `class` (0 for a root or an unknown class).
    fn depth(&self, class: u32) -> usize {
        self.lineage(class).count().saturating_sub(1)
    }
}

impl<P: ClassParents + ?Sized> ClassParents for &P {
    fn parent(&self, class: u32) -> Option<u32> {
        (**self).parent(class)
    }
    fn step_limit(&self) -> usize {
        (**self).step_limit()
    }
}

impl<P: ClassParents + ?Sized> ClassParents for std::sync::Arc<P> {
    fn parent(&self, class: u32) -> Option<u32> {
        (**self).parent(class)
    }
    fn step_limit(&self) -> usize {
        (**self).step_limit()
    }
}

impl ClassParents for BTreeMap<u32, u32> {
    fn parent(&self, class: u32) -> Option<u32> {
        self.get(&class).copied().filter(|&p| p != NO_PARENT)
    }
    fn step_limit(&self) -> usize {
        self.len()
    }
}

impl<S: std::hash::BuildHasher> ClassParents for HashMap<u32, u32, S> {
    fn parent(&self, class: u32) -> Option<u32> {
        self.get(&class).copied().filter(|&p| p != NO_PARENT)
    }
    fn step_limit(&self) -> usize {
        self.len()
    }
}

/// `class`, then its ancestors, nearest first ([`ClassParents::lineage`]). Stops after
/// [`ClassParents::step_limit`] + 1 classes, so a cyclic map ends.
#[derive(Debug)]
pub struct Lineage<'p, P: ?Sized> {
    parents: &'p P,
    next: Option<u32>,
    left: usize,
}

impl<P: ClassParents + ?Sized> Iterator for Lineage<'_, P> {
    type Item = u32;

    fn next(&mut self) -> Option<u32> {
        if self.left == 0 {
            if self.next.take().is_some() {
                tracing::debug!("class parent chain exceeds the class count; cycle assumed");
            }
            return None;
        }
        self.left -= 1;
        let c = self.next.take()?;
        self.next = self.parents.parent(c);
        Some(c)
    }
}

/// An owned parent map: class → parent (`wlib.md` §9). For tests, tools and callers that
/// copy the map out of the class library.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ClassTree {
    parents: BTreeMap<u32, u32>,
}

impl ClassTree {
    /// An empty map (every class is a root).
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Builds the map from parent-map entries: entry *i* is the parent of class *i*,
    /// [`NO_PARENT`] for none.
    #[must_use]
    pub fn from_parent_map(entries: impl IntoIterator<Item = u32>) -> Self {
        let parents = (0u32..)
            .zip(entries)
            .filter(|&(_, p)| p != NO_PARENT)
            .collect();
        Self { parents }
    }

    /// Builds the map from (class, parent) pairs; a later pair for the same class wins.
    #[must_use]
    pub fn from_edges(edges: impl IntoIterator<Item = (u32, u32)>) -> Self {
        let parents = edges.into_iter().filter(|&(_, p)| p != NO_PARENT).collect();
        Self { parents }
    }

    /// Sets the parent of `class` ([`NO_PARENT`] removes it).
    pub fn set_parent(&mut self, class: u32, parent: u32) {
        if parent == NO_PARENT {
            self.parents.remove(&class);
        } else {
            self.parents.insert(class, parent);
        }
    }

    /// Number of classes that have a parent.
    #[must_use]
    pub fn len(&self) -> usize {
        self.parents.len()
    }

    /// Whether no class has a parent.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.parents.is_empty()
    }
}

impl ClassParents for ClassTree {
    fn parent(&self, class: u32) -> Option<u32> {
        self.parents.get(&class).copied()
    }
    fn step_limit(&self) -> usize {
        self.parents.len()
    }
}

/// One class test of an ordered rule table (`combat-data.md` §2.1).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ClassTest {
    /// The class is exactly this index ("*c* = 827").
    Is(u32),
    /// The class is this index or derives from it ("698 is *c* or an ancestor").
    IsA(u32),
    /// The class derives from this index and is not it ("3530 is an ancestor").
    Below(u32),
}

impl ClassTest {
    /// Whether `class` passes the test.
    #[must_use]
    pub fn matches<P: ClassParents + ?Sized>(self, parents: &P, class: u32) -> bool {
        match self {
            Self::Is(c) => class == c,
            Self::IsA(a) => parents.is_a(class, a),
            Self::Below(a) => parents.is_below(class, a),
        }
    }
}

/// The kind of the first rule whose test `class` passes; rules are tried in order.
#[must_use]
pub fn classify_class<K: Copy, P: ClassParents + ?Sized>(
    parents: &P,
    class: u32,
    rules: &[(ClassTest, K)],
) -> Option<K> {
    rules
        .iter()
        .find(|(t, _)| t.matches(parents, class))
        .map(|&(_, k)| k)
}

/// Root classes of object records, indexed both ways. Built by the caller from the records
/// it reads (for example every 0x70 record of the gamelogic archive).
#[derive(Debug, Clone, Default)]
pub struct ClassIndex {
    by_did: HashMap<Did, u32>,
    by_class: BTreeMap<u32, Vec<Did>>,
}

impl ClassIndex {
    /// An empty index.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Records the root class of `did`. A DID inserted twice keeps its latest class.
    pub fn insert(&mut self, did: Did, root_class: u32) {
        if let Some(old) = self.by_did.insert(did, root_class) {
            if old == root_class {
                return;
            }
            if let Some(v) = self.by_class.get_mut(&old) {
                v.retain(|&d| d != did);
                if v.is_empty() {
                    self.by_class.remove(&old);
                }
            }
        }
        self.by_class.entry(root_class).or_default().push(did);
    }

    /// Records an [`ObjectRoot`].
    pub fn insert_root(&mut self, root: ObjectRoot) {
        self.insert(root.did, root.root_class);
    }

    /// Number of records.
    #[must_use]
    pub fn len(&self) -> usize {
        self.by_did.len()
    }

    /// Whether the index is empty.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.by_did.is_empty()
    }

    /// The root class of `did`, if indexed.
    #[must_use]
    pub fn root_class(&self, did: Did) -> Option<u32> {
        self.by_did.get(&did).copied()
    }

    /// The records whose root class is exactly `class`, in insertion order.
    #[must_use]
    pub fn dids_of_class(&self, class: u32) -> &[Did] {
        self.by_class.get(&class).map_or(&[], Vec::as_slice)
    }

    /// Every root class that occurs, ascending, with its record count.
    pub fn classes(&self) -> impl Iterator<Item = (u32, usize)> + '_ {
        self.by_class.iter().map(|(&c, v)| (c, v.len()))
    }

    /// The root classes that are `ancestor` or derive from it, ascending.
    #[must_use]
    pub fn classes_under<P: ClassParents + ?Sized>(&self, parents: &P, ancestor: u32) -> Vec<u32> {
        self.by_class
            .keys()
            .copied()
            .filter(|&c| parents.is_a(c, ancestor))
            .collect()
    }

    /// The records whose root class is `ancestor` or derives from it, grouped by class
    /// (ascending), each group in insertion order.
    pub fn dids_under<'a, P: ClassParents + ?Sized>(
        &'a self,
        parents: &'a P,
        ancestor: u32,
    ) -> impl Iterator<Item = Did> + 'a {
        self.by_class
            .iter()
            .filter(move |&(&c, _)| parents.is_a(c, ancestor))
            .flat_map(|(_, v)| v.iter().copied())
    }

    /// The deepest common ancestor-or-self of the root classes of `dids` (unindexed DIDs are
    /// ignored); see [`ClassParents::common_ancestor`].
    #[must_use]
    pub fn common_ancestor_of<P: ClassParents + ?Sized>(
        &self,
        parents: &P,
        dids: impl IntoIterator<Item = Did>,
    ) -> Option<u32> {
        parents.common_ancestor(dids.into_iter().filter_map(|d| self.root_class(d)))
    }
}

// ---------------------------------------------------------------------------------------------
// Property IDs by label
// ---------------------------------------------------------------------------------------------

/// A property ID found by its dictionary label at run time (`property-system.md` §2). The
/// label is kept for error messages; the ID is `None` when the install's dictionary does not
/// define the label, in which case no set can hold the property.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Pid {
    id: Option<u32>,
    label: &'static str,
}

impl Pid {
    /// Looks `label` up in `names` (normally the [`PropertyDictionary`]).
    #[must_use]
    pub fn resolve<N: PropertyNames + ?Sized>(names: &N, label: &'static str) -> Self {
        Self {
            id: names.pid_of(label).filter(|&p| p != 0),
            label,
        }
    }

    /// A label that is not defined (for defaults and tests).
    #[must_use]
    pub const fn unresolved(label: &'static str) -> Self {
        Self { id: None, label }
    }

    /// A label with a known ID (for synthetic tests; real code resolves labels).
    #[must_use]
    pub const fn with_id(label: &'static str, id: u32) -> Self {
        Self {
            id: if id == 0 { None } else { Some(id) },
            label,
        }
    }

    /// The property ID, if the label is defined.
    #[must_use]
    pub const fn id(self) -> Option<u32> {
        self.id
    }

    /// The dictionary label.
    #[must_use]
    pub const fn label(self) -> &'static str {
        self.label
    }

    /// Whether the label is defined in the dictionary.
    #[must_use]
    pub const fn is_resolved(self) -> bool {
        self.id.is_some()
    }

    /// Whether this is property `pid`.
    #[must_use]
    pub fn is(self, pid: u32) -> bool {
        self.id == Some(pid)
    }

    /// [`FormatError::UnknownName`] unless the label is defined.
    pub fn require_defined(self) -> Result<u32> {
        self.id.ok_or(FormatError::UnknownName {
            what: "property",
            name: self.label,
        })
    }
}

/// Declares a struct of [`Pid`](crate::gameplay::common::Pid) fields resolved from
/// dictionary labels.
///
/// ```
/// use tessera_formats::property_ids;
///
/// property_ids! {
///     /// Properties a reader needs.
///     pub struct ExamplePids {
///         /// The record's name.
///         name = "Example_Name",
///         level = "Example_Level",
///     }
/// }
///
/// let names = |label: &str| (label == "Example_Name").then_some(7u32);
/// let pids = ExamplePids::resolve(&names);
/// assert!(pids.name.is(7));
/// assert_eq!(pids.missing(), vec!["Example_Level"]);
/// assert_eq!(ExamplePids::LABELS, &["Example_Name", "Example_Level"]);
/// ```
///
/// The generated struct derives `Debug`, `Clone`, `Copy`, `PartialEq` and `Eq` and has
/// `LABELS`, `resolve(&names)` (any [`PropertyNames`](crate::PropertyNames)),
/// `unresolved()`, `all()` and `missing()`.
#[macro_export]
macro_rules! property_ids {
    (
        $(#[$meta:meta])*
        $vis:vis struct $name:ident {
            $(
                $(#[$fmeta:meta])*
                $field:ident = $label:literal
            ),* $(,)?
        }
    ) => {
        $(#[$meta])*
        #[derive(Debug, Clone, Copy, PartialEq, Eq)]
        $vis struct $name {
            $(
                $(#[$fmeta])*
                #[doc = concat!("Property `", $label, "`.")]
                pub $field: $crate::gameplay::common::Pid,
            )*
        }

        impl $name {
            /// The labels, in declaration order.
            pub const LABELS: &'static [&'static str] = &[$($label),*];

            /// Resolves every label against `names` (normally the property dictionary).
            #[must_use]
            pub fn resolve<N: $crate::PropertyNames + ?Sized>(names: &N) -> Self {
                Self {
                    $($field: $crate::gameplay::common::Pid::resolve(names, $label),)*
                }
            }

            /// Every label unresolved (no property can match).
            #[must_use]
            pub const fn unresolved() -> Self {
                Self {
                    $($field: $crate::gameplay::common::Pid::unresolved($label),)*
                }
            }

            /// Every field, in declaration order.
            #[must_use]
            pub fn all(&self) -> ::std::vec::Vec<$crate::gameplay::common::Pid> {
                ::std::vec![$(self.$field),*]
            }

            /// The labels the dictionary does not define, in declaration order.
            #[must_use]
            pub fn missing(&self) -> ::std::vec::Vec<&'static str> {
                self.all()
                    .into_iter()
                    .filter(|p| !p.is_resolved())
                    .map(|p| p.label())
                    .collect()
            }
        }
    };
}

// ---------------------------------------------------------------------------------------------
// Typed property access
// ---------------------------------------------------------------------------------------------

/// Typed extraction from one [`PropertyValue`]. Each accessor returns `None` for any other
/// value type; no accessor converts between types except where documented.
pub trait ValueExt {
    /// The value itself.
    fn value_ref(&self) -> &PropertyValue;

    /// Type 15 integer.
    fn int(&self) -> Option<i32> {
        match self.value_ref() {
            PropertyValue::Int(v) => Some(*v),
            _ => None,
        }
    }

    /// Type 15 or type 19 integer, widened to 64 bits.
    fn integer(&self) -> Option<i64> {
        match self.value_ref() {
            PropertyValue::Int(v) => Some(i64::from(*v)),
            PropertyValue::Int64(v) => Some(*v),
            _ => None,
        }
    }

    /// Type 9 float.
    fn float(&self) -> Option<f32> {
        match self.value_ref() {
            PropertyValue::Float(v) => Some(*v),
            _ => None,
        }
    }

    /// Type 21 boolean.
    fn boolean(&self) -> Option<bool> {
        match self.value_ref() {
            PropertyValue::Bool(v) => Some(*v),
            _ => None,
        }
    }

    /// Type 8 enum key.
    fn enum_key(&self) -> Option<u32> {
        match self.value_ref() {
            PropertyValue::Enum(v) => Some(*v),
            _ => None,
        }
    }

    /// Type 18 or 14 bitfield, widened to 64 bits (type 22: [`PropertyValue::as_bits`]).
    fn bitfield(&self) -> Option<u64> {
        match self.value_ref() {
            PropertyValue::Bitfield32(v) => Some(u64::from(*v)),
            PropertyValue::Bitfield64(v) => Some(*v),
            _ => None,
        }
    }

    /// Type 20 DID reference, including the null DID 0 ("none").
    fn did_ref(&self) -> Option<Did> {
        match self.value_ref() {
            PropertyValue::DidRef(d) => Some(*d),
            _ => None,
        }
    }

    /// Type 20 DID reference, `None` for the null DID ("none", `property-system.md` §4.9).
    fn did_nonnull(&self) -> Option<Did> {
        self.did_ref().filter(|d| !d.is_null())
    }

    /// Type 10 property reference (a PID; 0 means "none").
    fn property_ref(&self) -> Option<u32> {
        match self.value_ref() {
            PropertyValue::PropertyId(v) => Some(*v),
            _ => None,
        }
    }

    /// Type 1 string.
    fn string(&self) -> Option<&str> {
        match self.value_ref() {
            PropertyValue::String(s) => Some(s),
            _ => None,
        }
    }

    /// Type 13 StringInfo (unresolved; text belongs to `tessera-text`).
    fn string_info(&self) -> Option<&StringInfo> {
        match self.value_ref() {
            PropertyValue::StringInfo(s) => Some(s),
            _ => None,
        }
    }

    /// Type 17 position.
    fn position(&self) -> Option<&Position> {
        match self.value_ref() {
            PropertyValue::Position(p) => Some(p),
            _ => None,
        }
    }

    /// Type 6 vector.
    fn vector3(&self) -> Option<[f32; 3]> {
        match self.value_ref() {
            PropertyValue::Vector3(v) => Some(*v),
            _ => None,
        }
    }
}

impl ValueExt for PropertyValue {
    fn value_ref(&self) -> &PropertyValue {
        self
    }
}

impl ValueExt for PropertyEntry {
    fn value_ref(&self) -> &PropertyValue {
        &self.value
    }
}

/// A short name of a value type, for [`FormatError::PropertyTypeMismatch`].
fn type_name(ty: PropertyType) -> &'static str {
    match ty {
        PropertyType::Int => "int",
        PropertyType::Int64 => "int64",
        PropertyType::Float => "float",
        PropertyType::Bool => "bool",
        PropertyType::Enum => "enum",
        PropertyType::DidRef => "DID",
        PropertyType::PropertyId => "property ID",
        PropertyType::String => "string",
        PropertyType::StringInfo => "StringInfo",
        PropertyType::Position => "position",
        PropertyType::Struct => "struct",
        PropertyType::Array => "array",
        PropertyType::Bitfield32 | PropertyType::Bitfield64 | PropertyType::BitfieldAny => {
            "bitfield"
        }
        _ => "value",
    }
}

/// Typed, label-aware access to the properties of a set or struct.
///
/// Optional accessors return `None` when the property is absent, its label is undefined, or
/// it holds another type. `require*` accessors turn those cases into
/// [`FormatError::MissingProperty`] and [`FormatError::PropertyTypeMismatch`], for
/// properties a kind always has.
pub trait PropertyAccess {
    /// The value of property `pid`, if present.
    fn value_of(&self, pid: u32) -> Option<&PropertyValue>;

    /// The value of `pid`, if its label is defined and the property present.
    fn get_pid(&self, pid: Pid) -> Option<&PropertyValue> {
        self.value_of(pid.id()?)
    }

    /// Whether `pid` is present.
    fn has(&self, pid: Pid) -> bool {
        self.get_pid(pid).is_some()
    }

    /// See [`ValueExt::int`].
    fn int(&self, pid: Pid) -> Option<i32> {
        self.get_pid(pid)?.int()
    }
    /// See [`ValueExt::integer`].
    fn integer(&self, pid: Pid) -> Option<i64> {
        self.get_pid(pid)?.integer()
    }
    /// See [`ValueExt::float`].
    fn float(&self, pid: Pid) -> Option<f32> {
        self.get_pid(pid)?.float()
    }
    /// See [`ValueExt::boolean`].
    fn boolean(&self, pid: Pid) -> Option<bool> {
        self.get_pid(pid)?.boolean()
    }
    /// See [`ValueExt::enum_key`].
    fn enum_key(&self, pid: Pid) -> Option<u32> {
        self.get_pid(pid)?.enum_key()
    }
    /// See [`ValueExt::bitfield`].
    fn bitfield(&self, pid: Pid) -> Option<u64> {
        self.get_pid(pid)?.bitfield()
    }
    /// See [`ValueExt::did_ref`] (null DIDs included).
    fn did(&self, pid: Pid) -> Option<Did> {
        self.get_pid(pid)?.did_ref()
    }
    /// See [`ValueExt::did_nonnull`].
    fn did_nonnull(&self, pid: Pid) -> Option<Did> {
        self.get_pid(pid)?.did_nonnull()
    }
    /// See [`ValueExt::property_ref`].
    fn property_ref(&self, pid: Pid) -> Option<u32> {
        self.get_pid(pid)?.property_ref()
    }
    /// See [`ValueExt::string`].
    fn string(&self, pid: Pid) -> Option<&str> {
        self.get_pid(pid)?.string()
    }
    /// See [`ValueExt::string_info`].
    fn string_info(&self, pid: Pid) -> Option<&StringInfo> {
        self.get_pid(pid)?.string_info()
    }
    /// See [`ValueExt::position`].
    fn position(&self, pid: Pid) -> Option<&Position> {
        self.get_pid(pid)?.position()
    }
    /// See [`ValueExt::vector3`].
    fn vector3(&self, pid: Pid) -> Option<[f32; 3]> {
        self.get_pid(pid)?.vector3()
    }
    /// The struct value of `pid`.
    fn structure(&self, pid: Pid) -> Option<&PropertyStruct> {
        self.get_pid(pid)?.as_struct()
    }
    /// The array value of `pid`.
    fn array(&self, pid: Pid) -> Option<&PropertyArray> {
        self.get_pid(pid)?.as_array()
    }

    /// The elements of array `pid` in stored order; empty when absent or not an array.
    fn elements(&self, pid: Pid) -> &[PropertyEntry] {
        self.array(pid).map_or(&[], |a| a.elements.as_slice())
    }

    /// The struct elements of array `pid`, in stored order (other elements skipped).
    fn structs(&self, pid: Pid) -> impl Iterator<Item = &PropertyStruct> {
        self.elements(pid)
            .iter()
            .filter_map(|e| e.value.as_struct())
    }

    /// The DID elements of array `pid`, in stored order, null DIDs skipped.
    fn dids(&self, pid: Pid) -> impl Iterator<Item = Did> {
        self.elements(pid).iter().filter_map(ValueExt::did_nonnull)
    }

    /// The value of `pid` or [`FormatError::MissingProperty`].
    fn require(&self, pid: Pid, what: &'static str) -> Result<&PropertyValue> {
        self.get_pid(pid).ok_or(FormatError::MissingProperty {
            what,
            label: pid.label(),
        })
    }

    /// The value of `pid` converted by `f`; absent → [`FormatError::MissingProperty`],
    /// `f` returning `None` → [`FormatError::PropertyTypeMismatch`] naming `expected`.
    fn require_as<'s, T>(
        &'s self,
        pid: Pid,
        what: &'static str,
        expected: PropertyType,
        f: impl FnOnce(&'s PropertyValue) -> Option<T>,
    ) -> Result<T> {
        let v = self.require(pid, what)?;
        f(v).ok_or(FormatError::PropertyTypeMismatch {
            what,
            label: pid.label(),
            expected: type_name(expected),
            found: v.property_type(),
        })
    }

    /// Required type 15 integer.
    fn require_int(&self, pid: Pid, what: &'static str) -> Result<i32> {
        self.require_as(pid, what, PropertyType::Int, ValueExt::int)
    }
    /// Required type 9 float.
    fn require_float(&self, pid: Pid, what: &'static str) -> Result<f32> {
        self.require_as(pid, what, PropertyType::Float, ValueExt::float)
    }
    /// Required type 21 boolean.
    fn require_bool(&self, pid: Pid, what: &'static str) -> Result<bool> {
        self.require_as(pid, what, PropertyType::Bool, ValueExt::boolean)
    }
    /// Required type 8 enum key.
    fn require_enum(&self, pid: Pid, what: &'static str) -> Result<u32> {
        self.require_as(pid, what, PropertyType::Enum, ValueExt::enum_key)
    }
    /// Required type 20 DID (null allowed).
    fn require_did(&self, pid: Pid, what: &'static str) -> Result<Did> {
        self.require_as(pid, what, PropertyType::DidRef, ValueExt::did_ref)
    }
    /// Required struct.
    fn require_struct(&self, pid: Pid, what: &'static str) -> Result<&PropertyStruct> {
        self.require_as(pid, what, PropertyType::Struct, PropertyValue::as_struct)
    }
    /// Required array.
    fn require_array(&self, pid: Pid, what: &'static str) -> Result<&PropertyArray> {
        self.require_as(pid, what, PropertyType::Array, PropertyValue::as_array)
    }

    /// The stored value of `pid`, else its dictionary default (`property-system.md` §2.2;
    /// `economy.md` §1 "absent field").
    fn value_or_default<'a, D: PropertyDefaults + ?Sized>(
        &'a self,
        pid: Pid,
        defaults: &'a D,
    ) -> Option<&'a PropertyValue> {
        let id = pid.id()?;
        self.value_of(id).or_else(|| defaults.default_value(id))
    }
}

impl PropertyAccess for PropertySet {
    fn value_of(&self, pid: u32) -> Option<&PropertyValue> {
        self.get(pid)
    }
}

impl PropertyAccess for PropertyStruct {
    fn value_of(&self, pid: u32) -> Option<&PropertyValue> {
        self.get(pid)
    }
}

impl PropertyAccess for GameObjectRecord {
    fn value_of(&self, pid: u32) -> Option<&PropertyValue> {
        self.properties.get(pid)
    }
}

/// Dictionary defaults (`property-system.md` §2.2).
pub trait PropertyDefaults {
    /// The default value of `pid`, if its definition has one.
    fn default_value(&self, pid: u32) -> Option<&PropertyValue>;
}

impl PropertyDefaults for PropertyDictionary {
    fn default_value(&self, pid: u32) -> Option<&PropertyValue> {
        self.definition(pid)?.default.as_ref()
    }
}

impl<S: std::hash::BuildHasher> PropertyDefaults for HashMap<u32, PropertyValue, S> {
    fn default_value(&self, pid: u32) -> Option<&PropertyValue> {
        self.get(&pid)
    }
}

impl PropertyDefaults for BTreeMap<u32, PropertyValue> {
    fn default_value(&self, pid: u32) -> Option<&PropertyValue> {
        self.get(&pid)
    }
}

// ---------------------------------------------------------------------------------------------
// Signatures
// ---------------------------------------------------------------------------------------------

/// A kind recognised by the top-level properties a set carries (`economy.md` §3.4,
/// `quests.md` §2.1, `map-data.md` §2.3).
///
/// A set matches when it holds every property of `all`, at least one of `any` (unless `any`
/// is empty) and none of `none`. An undefined label in `all` or `any` cannot be present, so
/// it never matches; an undefined label in `none` is absent everywhere.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Signature<'a> {
    /// Properties that must all be present.
    pub all: &'a [Pid],
    /// Properties of which at least one must be present (no constraint when empty).
    pub any: &'a [Pid],
    /// Properties that must be absent.
    pub none: &'a [Pid],
}

impl<'a> Signature<'a> {
    /// Every property of `all` present.
    #[must_use]
    pub const fn all(all: &'a [Pid]) -> Self {
        Self {
            all,
            any: &[],
            none: &[],
        }
    }

    /// At least one property of `any` present.
    #[must_use]
    pub const fn any(any: &'a [Pid]) -> Self {
        Self {
            all: &[],
            any,
            none: &[],
        }
    }

    /// Adds properties that must be present (at least one).
    #[must_use]
    pub const fn and_any(mut self, any: &'a [Pid]) -> Self {
        self.any = any;
        self
    }

    /// Adds properties that must be absent.
    #[must_use]
    pub const fn and_none(mut self, none: &'a [Pid]) -> Self {
        self.none = none;
        self
    }

    /// Whether `set` matches. A signature with no constraint matches every set.
    #[must_use]
    pub fn matches<A: PropertyAccess + ?Sized>(&self, set: &A) -> bool {
        self.all.iter().all(|&p| set.has(p))
            && (self.any.is_empty() || self.any.iter().any(|&p| set.has(p)))
            && !self.none.iter().any(|&p| set.has(p))
    }
}

/// The kind of the first signature `set` matches; rules are tried in order.
#[must_use]
pub fn classify_by_signature<K: Copy, A: PropertyAccess + ?Sized>(
    set: &A,
    rules: &[(Signature<'_>, K)],
) -> Option<K> {
    rules.iter().find(|(s, _)| s.matches(set)).map(|&(_, k)| k)
}

// ---------------------------------------------------------------------------------------------
// DID references
// ---------------------------------------------------------------------------------------------

/// One non-null DID value in a property tree, with where it sits.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct DidReference {
    /// PID of the top-level property that holds the value (equal to `pid` at top level).
    pub top: u32,
    /// PID of the struct or array directly enclosing the value; `None` at top level.
    pub parent: Option<u32>,
    /// PID of the value's own entry (the "leaf" label, e.g. an element or field PID).
    pub pid: u32,
    /// Nesting depth: 0 for a top-level value.
    pub depth: usize,
    /// The referenced DID (never null).
    pub target: Did,
}

/// Calls `f` for every non-null DID value of `set`, nested values included, depth first in
/// stored order (`items.md` §8, `economy.md` §10.1). Depth is bounded by the decoder's
/// nesting limit.
pub fn for_each_did_reference(set: &PropertySet, mut f: impl FnMut(DidReference)) {
    for e in &set.entries {
        visit_dids(e, e.pid, None, 0, &mut f);
    }
}

fn visit_dids(
    e: &PropertyEntry,
    top: u32,
    parent: Option<u32>,
    depth: usize,
    f: &mut impl FnMut(DidReference),
) {
    match &e.value {
        PropertyValue::DidRef(d) if !d.is_null() => f(DidReference {
            top,
            parent,
            pid: e.pid,
            depth,
            target: *d,
        }),
        PropertyValue::Struct(_) | PropertyValue::Array(_) => {
            for c in e.value.children() {
                visit_dids(c, top, Some(e.pid), depth + 1, f);
            }
        }
        _ => {}
    }
}

/// Whether `set` holds property `pid` at any nesting depth: as a top-level property, a
/// struct field or an array element, arrays inside arrays included (`items.md` §2.1
/// "nested"). `false` when the label is undefined. Depth is bounded by the decoder's nesting
/// limit.
#[must_use]
pub fn contains_pid(set: &PropertySet, pid: Pid) -> bool {
    fn visit(e: &PropertyEntry, id: u32) -> bool {
        e.pid == id || e.value.children().iter().any(|c| visit(c, id))
    }
    let Some(id) = pid.id() else {
        return false;
    };
    set.entries.iter().any(|e| visit(e, id))
}

/// Every non-null DID value of `set`; see [`for_each_did_reference`].
#[must_use]
pub fn did_references(set: &PropertySet) -> Vec<DidReference> {
    let mut out = Vec::new();
    for_each_did_reference(set, |r| out.push(r));
    out
}

/// One incoming edge of a [`ReverseDidIndex`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Referrer {
    /// The record whose property names the target.
    pub source: Did,
    /// The top-level property of the source that holds the reference.
    pub top: u32,
    /// The PID of the reference value itself.
    pub pid: u32,
}

/// A record found by [`ReverseDidIndex::walk_referrers`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ReferrerHit {
    /// The referring record.
    pub source: Did,
    /// The record it names on the way to the start (the start itself at depth 1).
    pub via: Did,
    /// Steps from the start (1 = names the start directly).
    pub depth: usize,
    /// The edge from `source` to `via`.
    pub edge: Referrer,
}

/// Target DID → records that name it (`economy.md` §10.2 steps 2–4, `items.md` §8.2).
#[derive(Debug, Clone, Default)]
pub struct ReverseDidIndex {
    map: HashMap<Did, Vec<Referrer>>,
    edges: usize,
}

impl ReverseDidIndex {
    /// An empty index.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Adds one edge `source --(top, pid)--> target`. Null targets are ignored.
    pub fn add_edge(&mut self, target: Did, referrer: Referrer) {
        if target.is_null() {
            return;
        }
        self.map.entry(target).or_default().push(referrer);
        self.edges += 1;
    }

    /// Adds every non-null DID value of `set` as an edge from `source`. With `filter`, only
    /// references for which it returns true.
    pub fn add_record(
        &mut self,
        source: Did,
        set: &PropertySet,
        mut filter: impl FnMut(&DidReference) -> bool,
    ) {
        for_each_did_reference(set, |r| {
            if filter(&r) {
                self.add_edge(
                    r.target,
                    Referrer {
                        source,
                        top: r.top,
                        pid: r.pid,
                    },
                );
            }
        });
    }

    /// The records naming `target`, in insertion order.
    #[must_use]
    pub fn referrers(&self, target: Did) -> &[Referrer] {
        self.map.get(&target).map_or(&[], Vec::as_slice)
    }

    /// Number of distinct targets.
    #[must_use]
    pub fn targets(&self) -> usize {
        self.map.len()
    }

    /// Number of edges.
    #[must_use]
    pub fn len(&self) -> usize {
        self.edges
    }

    /// Whether the index has no edges.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.edges == 0
    }

    /// Breadth-first walk from `start` towards the records that name it, keeping a visited
    /// set (`economy.md` §10.2 step 4). Every referrer found is reported once, at its
    /// smallest depth; the walk continues through a referrer only when `expand` returns true
    /// for it and its depth is below `max_depth`.
    pub fn walk_referrers(
        &self,
        start: Did,
        max_depth: usize,
        mut expand: impl FnMut(Did) -> bool,
    ) -> Vec<ReferrerHit> {
        let mut out = Vec::new();
        let mut seen: HashSet<Did> = HashSet::from([start]);
        let mut queue = VecDeque::from([(start, 0usize)]);
        while let Some((node, depth)) = queue.pop_front() {
            if depth >= max_depth {
                continue;
            }
            for &edge in self.referrers(node) {
                if !seen.insert(edge.source) {
                    continue;
                }
                let hit = ReferrerHit {
                    source: edge.source,
                    via: node,
                    depth: depth + 1,
                    edge,
                };
                out.push(hit);
                if expand(edge.source) {
                    queue.push_back((edge.source, depth + 1));
                }
            }
        }
        out
    }
}

// ---------------------------------------------------------------------------------------------
// DID maps and enum mappers
// ---------------------------------------------------------------------------------------------

/// Key of the WEENIECONTENT DID map (0x2800000E here) in the root DID map
/// (`property-system.md` §8.1). Its labels name well-known game objects.
pub const WEENIECONTENT_ROOT_KEY: u32 = 0x0A;

/// Key of the UNIQUEDB DID map (0x28000002 here) in the root DID map
/// (`property-system.md` §8.1). Its labels name singleton records of other types.
pub const UNIQUEDB_ROOT_KEY: u32 = 0x0F;

/// Label of the WEENIECONTENT DID map in the root DID map.
pub const WEENIECONTENT_LABEL: &str = "WEENIECONTENT";

/// Label of the UNIQUEDB DID map in the root DID map.
pub const UNIQUEDB_LABEL: &str = "UNIQUEDB";

/// The enum mapper (prefix 0x23) that holds the labels of enum or bitfield property `pid`:
/// the `data` DID of its definition (`property-system.md` §7.1). `None` for other types,
/// undefined labels and definitions that name a DID map instead.
#[must_use]
pub fn enum_mapper_of(dict: &PropertyDictionary, pid: Pid) -> Option<Did> {
    let def = dict.definition(pid.id()?)?;
    let enum_like = matches!(
        def.ty,
        PropertyType::Enum
            | PropertyType::Bitfield32
            | PropertyType::Bitfield64
            | PropertyType::BitfieldAny
    );
    (enum_like && def.data.prefix() == 0x23).then_some(def.data)
}

// ---------------------------------------------------------------------------------------------
// Synthetic trees
// ---------------------------------------------------------------------------------------------

/// Constructors for synthetic property trees and object headers (tests and fuzzing; the
/// TSize hints are 0).
pub mod build {
    use crate::did::Did;
    use crate::property::{PropertyArray, PropertyEntry, PropertyStruct, PropertyValue};
    use crate::property_set::PropertySet;
    use crate::reader::TSize;

    fn count(n: usize) -> u32 {
        u32::try_from(n).unwrap_or(u32::MAX)
    }

    /// One entry.
    #[must_use]
    pub fn entry(pid: u32, value: PropertyValue) -> PropertyEntry {
        PropertyEntry { pid, value }
    }

    /// A property set holding `entries` in the given order.
    #[must_use]
    pub fn set(entries: Vec<PropertyEntry>) -> PropertySet {
        PropertySet {
            size: TSize {
                hint: 0,
                count: count(entries.len()),
            },
            entries,
        }
    }

    /// A struct value holding `fields` in the given order.
    #[must_use]
    pub fn structure(fields: Vec<PropertyEntry>) -> PropertyValue {
        PropertyValue::Struct(PropertyStruct {
            size: TSize {
                hint: 0,
                count: count(fields.len()),
            },
            fields,
        })
    }

    /// An array value holding `elements` in the given order.
    #[must_use]
    pub fn array(elements: Vec<PropertyEntry>) -> PropertyValue {
        PropertyValue::Array(PropertyArray {
            count: count(elements.len()),
            elements,
        })
    }

    /// A DID value.
    #[must_use]
    pub fn did(raw: u32) -> PropertyValue {
        PropertyValue::DidRef(Did(raw))
    }

    /// The first [`super::OBJECT_ROOT_LEN`] bytes of an object-state record: DID, root class.
    #[must_use]
    pub fn object_root(did: u32, root_class: u32) -> Vec<u8> {
        let mut out = Vec::with_capacity(super::OBJECT_ROOT_LEN);
        out.extend_from_slice(&did.to_le_bytes());
        out.extend_from_slice(&root_class.to_le_bytes());
        out
    }
}

#[cfg(test)]
mod tests {
    use super::build::{array, did, entry, object_root, set, structure};
    use super::*;

    // A synthetic class tree:
    //   1 ── 2 ── 4 ── 6
    //   │    └─── 5
    //   └─── 3
    //   9 (separate root)
    fn tree() -> ClassTree {
        ClassTree::from_edges([(2, 1), (3, 1), (4, 2), (5, 2), (6, 4)])
    }

    #[test]
    fn object_root_reads_header_only() {
        let mut bytes = object_root(0x7000_0042, 17);
        bytes.extend_from_slice(&[0xFF; 3]); // the rest is never examined
        let root = ObjectRoot::read(&bytes).unwrap();
        assert_eq!(root.did, Did(0x7000_0042));
        assert_eq!(root.root_class, 17);
        assert_eq!(root.property_set_did(), Some(Did(0x7900_0042)));
        assert!(ObjectRoot::read_expecting(&bytes, 0x7000_0042).is_ok());
        assert!(matches!(
            ObjectRoot::read_expecting(&bytes, 0x7000_0043),
            Err(FormatError::DidMismatch { .. })
        ));
    }

    #[test]
    fn object_root_rejects_bad_input() {
        assert!(matches!(
            ObjectRoot::read(&object_root(0x7900_0001, 1)),
            Err(FormatError::WrongRecordType { .. })
        ));
        assert!(matches!(
            ObjectRoot::read(&[0, 0, 0, 0x70, 1]),
            Err(FormatError::Truncated { .. })
        ));
        assert!(ObjectRoot::read(&[]).is_err());
        // Singletons (0x76) are object records; the +0x09000000 rule still names a DID
        // (0x7F...), which no archive holds (wstate.md §2), so a lookup misses.
        let s = ObjectRoot::read(&object_root(0x7600_0005, 5)).unwrap();
        assert_eq!(s.property_set_did(), Some(Did(0x7F00_0005)));
        assert!(matches!(
            ObjectRoot::read(&object_root(0x7700_0001, 1)).map(|r| r.property_set_did()),
            Ok(None)
        ));
    }

    #[test]
    fn game_object_record_pairs_header_and_set() {
        // Property set record: DID, TSize (hint 0, count 1), PID, echo, int value.
        let mut set_bytes = Vec::new();
        set_bytes.extend_from_slice(&0x7900_0001u32.to_le_bytes());
        set_bytes.extend_from_slice(&[0, 1]);
        set_bytes.extend_from_slice(&5u32.to_le_bytes());
        set_bytes.extend_from_slice(&5u32.to_le_bytes());
        set_bytes.extend_from_slice(&42i32.to_le_bytes());
        let schema = |pid: u32| (pid == 5).then_some(PropertyType::Int);
        let rec = GameObjectRecord::decode(&object_root(0x7000_0001, 3), Some(&set_bytes), &schema)
            .unwrap();
        assert_eq!(rec.root_class(), 3);
        assert_eq!(rec.int(Pid::with_id("A", 5)), Some(42));

        // Missing set: empty properties.
        let rec = GameObjectRecord::decode(&object_root(0x7000_0001, 3), None, &schema).unwrap();
        assert!(rec.properties.is_empty());

        // Set of another object.
        assert!(matches!(
            GameObjectRecord::decode(&object_root(0x7000_0002, 3), Some(&set_bytes), &schema),
            Err(FormatError::DidMismatch { .. })
        ));
        // Object DID without a property-set DID.
        assert!(decode_object_properties(Did(0x7700_0001), &set_bytes, &schema).is_err());
    }

    #[test]
    fn lineage_and_tests() {
        let t = tree();
        assert_eq!(t.lineage(6).collect::<Vec<_>>(), vec![6, 4, 2, 1]);
        assert_eq!(t.lineage(9).collect::<Vec<_>>(), vec![9]);
        assert!(t.is_a(6, 6) && t.is_a(6, 1) && !t.is_a(3, 2));
        assert!(t.is_below(6, 2) && !t.is_below(2, 2));
        assert_eq!(t.depth(6), 3);
        assert_eq!(t.depth(1), 0);
        assert_eq!(t.child_toward(1, 6), Some(2));
        assert_eq!(t.child_toward(2, 6), Some(4));
        assert_eq!(t.child_toward(6, 6), None);
        assert_eq!(t.child_toward(3, 6), None);
    }

    #[test]
    fn common_ancestor() {
        let t = tree();
        assert_eq!(t.common_ancestor([6, 5]), Some(2));
        assert_eq!(t.common_ancestor([6, 5, 3]), Some(1));
        assert_eq!(t.common_ancestor([6, 4]), Some(4));
        assert_eq!(t.common_ancestor([6]), Some(6));
        assert_eq!(t.common_ancestor([6, 9]), None);
        assert_eq!(t.common_ancestor(std::iter::empty()), None);
    }

    #[test]
    fn cycles_terminate() {
        let t = ClassTree::from_edges([(1, 2), (2, 3), (3, 1)]);
        assert!(t.lineage(1).count() <= t.step_limit() + 1);
        assert!(!t.is_a(1, 99));
        let _ = t.common_ancestor([1, 2]);
        let self_loop = ClassTree::from_edges([(4, 4)]);
        assert!(self_loop.lineage(4).count() <= 2);
    }

    #[test]
    fn parent_map_form() {
        // Entry i is the parent of class i; NO_PARENT for roots.
        let t = ClassTree::from_parent_map([NO_PARENT, 0, 1, NO_PARENT]);
        assert_eq!(t.lineage(2).collect::<Vec<_>>(), vec![2, 1, 0]);
        assert_eq!(t.parent(3), None);
        let map: BTreeMap<u32, u32> = [(1, 0), (2, NO_PARENT)].into();
        assert_eq!(map.parent(1), Some(0));
        assert_eq!(map.parent(2), None);
    }

    #[test]
    fn class_rules_in_order() {
        #[derive(Debug, Clone, Copy, PartialEq, Eq)]
        enum K {
            Exact,
            Sub,
            Family,
        }
        let t = tree();
        let rules = [
            (ClassTest::Is(5), K::Exact),
            (ClassTest::Below(4), K::Sub),
            (ClassTest::IsA(2), K::Family),
        ];
        assert_eq!(classify_class(&t, 5, &rules), Some(K::Exact));
        assert_eq!(classify_class(&t, 6, &rules), Some(K::Sub));
        assert_eq!(classify_class(&t, 4, &rules), Some(K::Family));
        assert_eq!(classify_class(&t, 3, &rules), None);
    }

    #[test]
    fn class_index_queries() {
        let t = tree();
        let mut idx = ClassIndex::new();
        idx.insert(Did(0x7000_0001), 6);
        idx.insert(Did(0x7000_0002), 5);
        idx.insert(Did(0x7000_0003), 3);
        idx.insert_root(ObjectRoot {
            did: Did(0x7000_0004),
            root_class: 6,
        });
        // Re-insert with another class moves the DID.
        idx.insert(Did(0x7000_0003), 9);
        assert_eq!(idx.len(), 4);
        assert_eq!(idx.root_class(Did(0x7000_0003)), Some(9));
        assert_eq!(idx.dids_of_class(3), &[] as &[Did]);
        assert_eq!(idx.dids_of_class(6), &[Did(0x7000_0001), Did(0x7000_0004)]);
        assert_eq!(idx.classes_under(&t, 2), vec![5, 6]);
        assert_eq!(
            idx.dids_under(&t, 2).collect::<Vec<_>>(),
            vec![Did(0x7000_0002), Did(0x7000_0001), Did(0x7000_0004)]
        );
        assert_eq!(
            idx.common_ancestor_of(&t, [Did(0x7000_0001), Did(0x7000_0002), Did(9)]),
            Some(2)
        );
        assert_eq!(
            idx.classes().collect::<Vec<_>>(),
            vec![(5, 1), (6, 2), (9, 1)]
        );
    }

    crate::property_ids! {
        /// Test table.
        struct TestPids {
            /// A number.
            number = "Test_Number",
            list = "Test_List",
            nested = "Test_Struct",
            leaf = "Test_Leaf",
            absent = "Test_Undefined",
        }
    }

    fn names(label: &str) -> Option<u32> {
        match label {
            "Test_Number" => Some(10),
            "Test_List" => Some(11),
            "Test_Struct" => Some(12),
            "Test_Leaf" => Some(13),
            _ => None,
        }
    }

    fn sample() -> PropertySet {
        set(vec![
            entry(10, PropertyValue::Int(7)),
            entry(
                11,
                array(vec![
                    entry(13, did(0x7000_0010)),
                    entry(13, did(0)),
                    entry(13, did(0x7000_0011)),
                    entry(12, structure(vec![entry(13, did(0x7000_0012))])),
                ]),
            ),
            entry(12, structure(vec![entry(10, PropertyValue::Float(1.5))])),
            entry(13, did(0x4100_0001)),
        ])
    }

    #[test]
    fn property_ids_resolve() {
        let p = TestPids::resolve(&names);
        assert!(p.number.is(10));
        assert_eq!(p.number.label(), "Test_Number");
        assert_eq!(p.missing(), vec!["Test_Undefined"]);
        assert_eq!(TestPids::LABELS.len(), 5);
        assert!(
            TestPids::unresolved()
                .all()
                .iter()
                .all(|p| !p.is_resolved())
        );
        assert!(matches!(
            p.absent.require_defined(),
            Err(FormatError::UnknownName { .. })
        ));
        // A dictionary that maps a label to PID 0 does not define it.
        assert!(!Pid::resolve(&|_: &str| Some(0u32), "X").is_resolved());
    }

    #[test]
    fn typed_access() {
        let p = TestPids::resolve(&names);
        let s = sample();
        assert_eq!(s.int(p.number), Some(7));
        assert_eq!(s.integer(p.number), Some(7));
        assert_eq!(s.float(p.number), None);
        assert!(s.has(p.list) && !s.has(p.absent));
        assert_eq!(s.int(p.absent), None);
        assert_eq!(s.elements(p.list).len(), 4);
        assert_eq!(s.elements(p.number).len(), 0);
        assert_eq!(
            s.dids(p.list).collect::<Vec<_>>(),
            vec![Did(0x7000_0010), Did(0x7000_0011)]
        );
        assert_eq!(s.structs(p.list).count(), 1);
        let st = s.structure(p.nested).unwrap();
        assert_eq!(st.float(p.number), Some(1.5));
        assert_eq!(s.did(p.leaf), Some(Did(0x4100_0001)));
        assert_eq!(s.did_nonnull(p.leaf), Some(Did(0x4100_0001)));

        assert_eq!(s.require_int(p.number, "test").unwrap(), 7);
        assert_eq!(
            s.require_float(p.number, "test"),
            Err(FormatError::PropertyTypeMismatch {
                what: "test",
                label: "Test_Number",
                expected: "float",
                found: PropertyType::Int,
            })
        );
        assert_eq!(
            s.require(p.absent, "test"),
            Err(FormatError::MissingProperty {
                what: "test",
                label: "Test_Undefined",
            })
        );
        assert!(s.require_array(p.list, "test").is_ok());
        assert!(s.require_struct(p.nested, "test").is_ok());
    }

    #[test]
    fn defaults_apply_only_when_absent() {
        let p = TestPids::resolve(&names);
        let defaults: BTreeMap<u32, PropertyValue> =
            [(10, PropertyValue::Int(1)), (13, PropertyValue::Int(2))].into();
        let s = set(vec![entry(10, PropertyValue::Int(7))]);
        assert_eq!(
            s.value_or_default(p.number, &defaults),
            Some(&PropertyValue::Int(7))
        );
        assert_eq!(
            s.value_or_default(p.leaf, &defaults),
            Some(&PropertyValue::Int(2))
        );
        assert_eq!(s.value_or_default(p.absent, &defaults), None);
    }

    #[test]
    fn signatures() {
        let p = TestPids::resolve(&names);
        let s = sample();
        let both = [p.number, p.list];
        let with_absent = [p.number, p.absent];
        let leaf = [p.leaf];
        let numbers = [p.number];
        assert!(Signature::all(&both).matches(&s));
        assert!(!Signature::all(&with_absent).matches(&s));
        assert!(Signature::any(&with_absent).matches(&s));
        assert!(!Signature::all(&both).and_none(&leaf).matches(&s));
        assert!(Signature::all(&both).and_none(&[p.absent]).matches(&s));
        assert!(Signature::default().matches(&s));
        let rules = [
            (Signature::all(&with_absent), 1),
            (Signature::all(&numbers).and_any(&leaf), 2),
            (Signature::default(), 3),
        ];
        assert_eq!(classify_by_signature(&s, &rules), Some(2));
        assert_eq!(classify_by_signature(&set(vec![]), &rules), Some(3));
    }

    #[test]
    fn did_reference_paths() {
        let refs = did_references(&sample());
        assert_eq!(refs.len(), 4);
        assert_eq!(
            refs[0],
            DidReference {
                top: 11,
                parent: Some(11),
                pid: 13,
                depth: 1,
                target: Did(0x7000_0010),
            }
        );
        assert_eq!(refs[2].parent, Some(12));
        assert_eq!(refs[2].top, 11);
        assert_eq!(refs[2].depth, 2);
        assert_eq!(
            refs[3],
            DidReference {
                top: 13,
                parent: None,
                pid: 13,
                depth: 0,
                target: Did(0x4100_0001),
            }
        );
    }

    #[test]
    fn reverse_index_walk() {
        // item <- list A <- group B <- mob C ; item <- list D (D not expanded)
        let item = Did(0x7000_0001);
        let (a, b, c, d) = (
            Did(0x7000_000A),
            Did(0x7000_000B),
            Did(0x7000_000C),
            Did(0x7000_000D),
        );
        let mut idx = ReverseDidIndex::new();
        idx.add_record(a, &set(vec![entry(13, did(item.0))]), |_| true);
        idx.add_record(b, &set(vec![entry(13, did(a.0))]), |_| true);
        idx.add_record(c, &set(vec![entry(13, did(b.0))]), |_| true);
        idx.add_record(
            d,
            &set(vec![entry(13, did(item.0)), entry(10, did(0))]),
            |_| true,
        );
        idx.add_record(c, &set(vec![entry(99, did(item.0))]), |r| r.pid != 99);
        assert_eq!(idx.len(), 4);
        assert_eq!(idx.targets(), 3);
        assert_eq!(idx.referrers(item).len(), 2);

        let hits = idx.walk_referrers(item, 8, |s| s != d);
        let found: Vec<(Did, usize)> = hits.iter().map(|h| (h.source, h.depth)).collect();
        assert_eq!(found, vec![(a, 1), (d, 1), (b, 2), (c, 3)]);
        assert_eq!(hits[2].via, a);

        let shallow = idx.walk_referrers(item, 2, |_| true);
        assert_eq!(shallow.len(), 3);

        // A cycle ends.
        idx.add_edge(
            c,
            Referrer {
                source: item,
                top: 1,
                pid: 1,
            },
        );
        let hits = idx.walk_referrers(item, usize::MAX, |_| true);
        assert_eq!(hits.len(), 4);
    }

    #[test]
    fn enum_mapper_lookup_rejects_unresolved() {
        // An empty dictionary defines nothing.
        let dict = crate::decode::decode_property_dictionary(&empty_dictionary()).unwrap();
        assert_eq!(enum_mapper_of(&dict, Pid::with_id("X", 5)), None);
        assert_eq!(enum_mapper_of(&dict, Pid::unresolved("X")), None);
    }

    /// The smallest valid dictionary: DID, 8 reserved bytes, empty name list, separator,
    /// empty definition list.
    fn empty_dictionary() -> Vec<u8> {
        let mut out = Vec::new();
        out.extend_from_slice(&0x3400_0000u32.to_le_bytes());
        out.extend_from_slice(&[0; 8]);
        out.extend_from_slice(&[0, 0]);
        out.extend_from_slice(&[0, 0]);
        out.extend_from_slice(&[0, 0]);
        out
    }
}
