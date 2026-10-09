// SPDX-License-Identifier: GPL-3.0-or-later
//! Generic forms: empty markers (§3.2), DID sets (§3.3), the manager template (§3.4),
//! slot schedules (§4.2) and the compiled-only tables of §4.10.

use std::collections::BTreeSet;

use tessera_formats::{Did, StringInfo};
use tessera_props::{PropertyRegistry, PropertySet};

use crate::character::check_len;
use crate::error::{Result, WStateError};
use crate::graph::Resolved;
use crate::native::Native;
use crate::state::ObjectState;

/// Payload size of an empty marker's object state (§2.1 row 5, §3.2).
pub const EMPTY_MARKER_STATE_LEN: usize = 36;

/// Payload size of an empty property set: the set's DID and a TSize with count 0
/// (§3.2, `property-system.md` §5.1).
pub const EMPTY_PROPERTY_SET_LEN: usize = 6;

/// Name hash of the DID-set root's reference attribute (§3.3; observed in all six).
pub const DID_SET_ATTRIBUTE: u32 = 0x0DBF_22C3;

/// Name hash of the class-849 root's reference attribute (§4.10).
pub const ADVANCEMENT_ATTRIBUTE: u32 = 0x0FF5_18EC;

/// Name hash of the class-852 root's reference attribute (§4.10).
pub const MODIFIER_ATTRIBUTE: u32 = 0x0B4C_CFD2;

/// The empty-marker test of §2.1 row 5 and §3.2: an object state of
/// [`EMPTY_MARKER_STATE_LEN`] bytes that holds exactly one item, a script object with no
/// attributes, and a property set of [`EMPTY_PROPERTY_SET_LEN`] bytes with no entries.
#[must_use]
pub fn is_empty_marker(
    state_len: usize,
    state: &ObjectState,
    set_len: usize,
    set: &PropertySet,
) -> bool {
    state_len == EMPTY_MARKER_STATE_LEN
        && set_len == EMPTY_PROPERTY_SET_LEN
        && set.is_empty()
        && has_one_empty_script_object(state)
}

/// Whether the state holds exactly one item, a script object with no attributes (the
/// structural half of the empty-marker test, GCI2).
#[must_use]
pub(crate) fn has_one_empty_script_object(state: &ObjectState) -> bool {
    state.is_complete()
        && state.item_count() == 1
        && state.root_object().is_some_and(|r| r.is_empty())
}

/// The root's reference attributes and the native bodies they lead to, checking that the
/// root is a script object whose attributes are exactly `count` references.
fn root_native_targets<'s>(
    state: &'s ObjectState,
    count: usize,
    what: &'static str,
) -> Result<Vec<(u32, &'s Native)>> {
    let did = state.did;
    let root = state.root_object().ok_or(WStateError::WrongKind {
        did,
        expected: what,
    })?;
    if root.len() != count {
        return Err(WStateError::WrongKind {
            did,
            expected: what,
        });
    }
    let mut out = Vec::with_capacity(count);
    for a in root.attributes() {
        let bad = |problem| WStateError::BadAttribute {
            did,
            hash: a.hash,
            problem,
        };
        let r = a
            .value
            .as_reference()
            .ok_or_else(|| bad("is not a reference"))?;
        let native = match state.resolve(r) {
            Resolved::Item(v) => v
                .as_native()
                .ok_or_else(|| bad("does not lead to a native object"))?,
            Resolved::Null => return Err(bad("is null")),
            _ => return Err(bad("does not resolve")),
        };
        out.push((a.hash, native));
    }
    Ok(out)
}

/// A DID set (§3.3): a class-18 set of object DIDs behind the root's single reference
/// attribute.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct DidSet {
    /// Name hash of the root's reference attribute ([`DID_SET_ATTRIBUTE`] observed).
    pub attribute: u32,
    /// The members, sorted by DID (the stored bucket order carries no meaning, §11.5).
    pub members: Vec<Did>,
    /// How many stored elements repeated an earlier one (0 observed).
    pub duplicates: usize,
    /// Number of items of the object state (2 observed).
    pub item_count: usize,
}

impl DidSet {
    /// Reads a DID-set object state.
    ///
    /// # Errors
    ///
    /// [`WStateError::WrongKind`] when the root is not a script object with exactly one
    /// attribute, [`WStateError::BadAttribute`] when that attribute is not a reference to
    /// a `set_u32` native object, [`WStateError::TooMany`] past [`super::MAX_VIEW_ENTRIES`].
    pub fn from_state(state: &ObjectState) -> Result<Self> {
        let targets = root_native_targets(state, 1, "DID set")?;
        let Some(&(attribute, native)) = targets.first() else {
            return Err(WStateError::WrongKind {
                did: state.did,
                expected: "DID set",
            });
        };
        let Native::SetU32(_, values) = native else {
            return Err(WStateError::BadAttribute {
                did: state.did,
                hash: attribute,
                problem: "does not lead to a set_u32 object",
            });
        };
        check_len("DID set", values.len())?;
        let unique: BTreeSet<u32> = values.iter().copied().collect();
        Ok(Self {
            attribute,
            duplicates: values.len() - unique.len(),
            members: unique.into_iter().map(Did).collect(),
            item_count: state.item_count(),
        })
    }

    /// Number of members.
    #[must_use]
    pub fn len(&self) -> usize {
        self.members.len()
    }

    /// Whether the set has no members.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.members.is_empty()
    }

    /// Whether `did` is a member.
    #[must_use]
    pub fn contains(&self, did: Did) -> bool {
        self.members.binary_search(&did).is_ok()
    }
}

/// A trait slot schedule (class 1490 here, §4.2): three counted lists of u32 behind the
/// root's three reference attributes. Their meaning is OPEN (Q4).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct SlotSchedule {
    /// (attribute name hash, list), in the root's serialization order.
    pub lists: Vec<(u32, Vec<u32>)>,
}

impl SlotSchedule {
    /// Reads a slot-schedule object state.
    ///
    /// # Errors
    ///
    /// [`WStateError::WrongKind`] when the root does not have exactly three attributes,
    /// [`WStateError::BadAttribute`] when one is not a reference to an `array_u32`,
    /// [`WStateError::TooMany`] past [`super::MAX_VIEW_ENTRIES`].
    pub fn from_state(state: &ObjectState) -> Result<Self> {
        let mut lists = Vec::with_capacity(3);
        for (hash, native) in root_native_targets(state, 3, "slot schedule")? {
            let values = native.as_array_u32().ok_or(WStateError::BadAttribute {
                did: state.did,
                hash,
                problem: "does not lead to an array_u32 object",
            })?;
            check_len("slot schedule", values.len())?;
            lists.push((hash, values.to_vec()));
        }
        Ok(Self { lists })
    }

    /// The list lengths, in order.
    #[must_use]
    pub fn lengths(&self) -> Vec<usize> {
        self.lists.iter().map(|(_, l)| l.len()).collect()
    }
}

/// An advancement table (class 849 here, §4.10): a counted list of u64 behind the
/// root's single reference attribute. Reading (index rule OPEN, Q5): value *i* is the
/// cumulative amount needed to reach rank or level *i*.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct AdvancementTable {
    /// Name hash of the root's reference attribute ([`ADVANCEMENT_ATTRIBUTE`] observed).
    pub attribute: u32,
    /// The values, stored order.
    pub values: Vec<u64>,
}

impl AdvancementTable {
    /// Reads a class-849 object state.
    ///
    /// # Errors
    ///
    /// [`WStateError::WrongKind`] or [`WStateError::BadAttribute`] for another layout,
    /// [`WStateError::TooMany`] past [`super::MAX_VIEW_ENTRIES`].
    pub fn from_state(state: &ObjectState) -> Result<Self> {
        let targets = root_native_targets(state, 1, "advancement table")?;
        let Some(&(attribute, native)) = targets.first() else {
            return Err(WStateError::WrongKind {
                did: state.did,
                expected: "advancement table",
            });
        };
        let values = native.as_array_u64().ok_or(WStateError::BadAttribute {
            did: state.did,
            hash: attribute,
            problem: "does not lead to an array_u64 object",
        })?;
        check_len("advancement table", values.len())?;
        Ok(Self {
            attribute,
            values: values.to_vec(),
        })
    }

    /// GCI5: the first value is 0 and the list never decreases.
    #[must_use]
    pub fn is_cumulative(&self) -> bool {
        self.values.first() == Some(&0)
            && self
                .values
                .windows(2)
                .all(|w| matches!(w, [a, b] if a <= b))
    }
}

/// A modifier table (class 852 here, §4.10): a hashed map of u32 keys to float bit
/// patterns behind the root's single reference attribute. Reading: key → multiplier
/// (what the key counts is OPEN, Q5).
#[derive(Debug, Clone, PartialEq, Default)]
pub struct ModifierTable {
    /// Name hash of the root's reference attribute ([`MODIFIER_ATTRIBUTE`] observed).
    pub attribute: u32,
    /// (key, value), sorted by key (bucket order carries no meaning).
    pub entries: Vec<(u32, f32)>,
}

impl ModifierTable {
    /// Reads a class-852 object state.
    ///
    /// # Errors
    ///
    /// [`WStateError::WrongKind`] or [`WStateError::BadAttribute`] for another layout,
    /// [`WStateError::TooMany`] past [`super::MAX_VIEW_ENTRIES`].
    pub fn from_state(state: &ObjectState) -> Result<Self> {
        let targets = root_native_targets(state, 1, "modifier table")?;
        let Some(&(attribute, native)) = targets.first() else {
            return Err(WStateError::WrongKind {
                did: state.did,
                expected: "modifier table",
            });
        };
        let map = native.as_map_u32().ok_or(WStateError::BadAttribute {
            did: state.did,
            hash: attribute,
            problem: "does not lead to a map_u32 object",
        })?;
        check_len("modifier table", map.len())?;
        let mut entries: Vec<(u32, f32)> = map
            .iter()
            .map(|&(k, bits)| (k, f32::from_bits(bits)))
            .collect();
        entries.sort_by_key(|&(k, _)| k);
        Ok(Self { attribute, entries })
    }

    /// GCI5: every value is a finite float.
    #[must_use]
    pub fn all_finite(&self) -> bool {
        self.entries.iter().all(|(_, v)| v.is_finite())
    }

    /// The value for `key`.
    #[must_use]
    pub fn get(&self, key: u32) -> Option<f32> {
        self.entries
            .binary_search_by_key(&key, |&(k, _)| k)
            .ok()
            .and_then(|i| self.entries.get(i))
            .map(|&(_, v)| v)
    }
}

/// The label of the name property of the manager entities and the three server entities
/// (§3.4, GC8, GCI19): their one textual property, and among the 13 managers other than
/// RepositoryManager the only property whose value differs. Looked up through the
/// dictionary, never by PID.
pub const ENTITY_NAME_LABEL: &str = "Name";

/// The name of a manager or server entity (§3.4): `Name` of its property half.
///
/// # Errors
///
/// [`crate::WStateError::BadProperty`] when `Name` is not a StringInfo.
pub fn entity_name(set: &PropertySet, reg: &PropertyRegistry) -> Result<Option<StringInfo>> {
    crate::character::Fields::new(set, reg, "manager entity").string_info("Name")
}

/// The differences between two property sets compared property by property (the
/// manager template of §3.4, GCI4).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct TemplateDiff {
    /// PIDs present only in the first set, ascending.
    pub only_first: Vec<u32>,
    /// PIDs present only in the second set, ascending.
    pub only_second: Vec<u32>,
    /// PIDs present in both with different values, ascending.
    pub different: Vec<u32>,
}

impl TemplateDiff {
    /// Compares `a` with `b`.
    #[must_use]
    pub fn between(a: &PropertySet, b: &PropertySet) -> Self {
        let mut out = Self::default();
        for p in a.iter() {
            match b.get(p.pid) {
                None => out.only_first.push(p.pid),
                Some(v) if *v != p.value => out.different.push(p.pid),
                Some(_) => {}
            }
        }
        for p in b.iter() {
            if !a.contains(p.pid) {
                out.only_second.push(p.pid);
            }
        }
        for v in [
            &mut out.only_first,
            &mut out.only_second,
            &mut out.different,
        ] {
            v.sort_unstable();
            v.dedup();
        }
        out
    }

    /// Whether the sets are identical.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.only_first.is_empty() && self.only_second.is_empty() && self.different.is_empty()
    }
}
