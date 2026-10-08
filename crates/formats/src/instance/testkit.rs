// SPDX-License-Identifier: GPL-3.0-or-later
//! Synthetic vocabulary and schema for tests and fuzzing: every label of
//! [`super::labels::ALL`] gets an invented PID, plus generic PIDs for array elements and
//! for struct fields that the spec names by suffix only. Nothing here comes from game data.

use std::collections::{BTreeMap, HashMap};

use super::labels;
use crate::did::Did;
use crate::property::{PropertyArray, PropertyEntry, PropertyStruct, PropertyType, PropertyValue};
use crate::property_set::PropertySet;
use crate::reader::TSize;

/// First invented PID of the labels (label *i* gets `LABEL_PID_BASE + i`).
pub const LABEL_PID_BASE: u32 = 0x1000_0000;
/// Generic element / field PIDs, one per value type.
pub const P_DID: u32 = 0x2000_0001;
/// Generic enum PID.
pub const P_ENUM: u32 = 0x2000_0002;
/// Generic float PID.
pub const P_FLOAT: u32 = 0x2000_0003;
/// Generic int PID.
pub const P_INT: u32 = 0x2000_0004;
/// Generic PID-valued PID.
pub const P_PID: u32 = 0x2000_0005;
/// Generic struct PID.
pub const P_STRUCT: u32 = 0x2000_0006;
/// Generic vector PID.
pub const P_VECTOR: u32 = 0x2000_0007;
/// Generic boolean PID.
pub const P_BOOL: u32 = 0x2000_0008;
/// A second statistic enum PID (a conversion-hash field).
pub const P_STAT: u32 = 0x2000_0009;

/// The type used for labels the spec gives no type for.
fn fallback(label: &str) -> PropertyType {
    match label {
        labels::STAT_DERIVED | labels::DBU_REMOVAL_SCRIPT => PropertyType::Enum,
        labels::DBU_REMOVED => PropertyType::Array,
        labels::DBU_REMOVAL_COUNT => PropertyType::Int,
        labels::FC_SPACING | labels::FC_ACTUAL_WIDTH => PropertyType::Float,
        _ => PropertyType::Bool,
    }
}

/// Names → PIDs for every label.
#[must_use]
pub fn names() -> BTreeMap<String, u32> {
    labels::ALL
        .iter()
        .zip(LABEL_PID_BASE..)
        .map(|((l, _), pid)| ((*l).to_owned(), pid))
        .collect()
}

/// PID → type for every label and generic PID.
#[must_use]
pub fn schema() -> HashMap<u32, PropertyType> {
    let mut m: HashMap<u32, PropertyType> = labels::ALL
        .iter()
        .zip(LABEL_PID_BASE..)
        .map(|((l, ty), pid)| (pid, ty.unwrap_or_else(|| fallback(l))))
        .collect();
    for (pid, ty) in [
        (P_DID, PropertyType::DidRef),
        (P_ENUM, PropertyType::Enum),
        (P_FLOAT, PropertyType::Float),
        (P_INT, PropertyType::Int),
        (P_PID, PropertyType::PropertyId),
        (P_STRUCT, PropertyType::Struct),
        (P_VECTOR, PropertyType::Vector3),
        (P_BOOL, PropertyType::Bool),
        (P_STAT, PropertyType::Enum),
    ] {
        m.insert(pid, ty);
    }
    m
}

/// A vocabulary over [`names`], with [`P_STAT`] counted as a statistic enum and
/// [`P_ENUM`] as a group-size enum.
#[must_use]
pub fn vocab() -> super::InstanceVocab {
    let mut v = super::InstanceVocab::from_names(&names());
    v.stat_enums.insert(P_STAT);
    v.size_enums.insert(P_ENUM);
    v
}

/// The invented PID of `label` (panics on a label outside [`labels::ALL`]; test use only).
#[must_use]
pub fn pid(label: &str) -> u32 {
    labels::ALL
        .iter()
        .zip(LABEL_PID_BASE..)
        .find(|((l, _), _)| *l == label)
        .map(|(_, p)| p)
        .unwrap_or_else(|| panic!("unknown label {label}"))
}

/// An entry of `label`.
#[must_use]
pub fn entry(label: &str, value: PropertyValue) -> PropertyEntry {
    PropertyEntry {
        pid: pid(label),
        value,
    }
}

/// An entry with a raw PID.
#[must_use]
pub fn raw(pid: u32, value: PropertyValue) -> PropertyEntry {
    PropertyEntry { pid, value }
}

/// A property set of `entries`.
#[must_use]
pub fn set(entries: Vec<PropertyEntry>) -> PropertySet {
    PropertySet {
        size: TSize {
            hint: 0,
            count: u32::try_from(entries.len()).unwrap_or(u32::MAX),
        },
        entries,
    }
}

/// A struct value of `fields`.
#[must_use]
pub fn strukt(fields: Vec<PropertyEntry>) -> PropertyValue {
    PropertyValue::Struct(PropertyStruct {
        size: TSize {
            hint: 0,
            count: u32::try_from(fields.len()).unwrap_or(u32::MAX),
        },
        fields,
    })
}

/// An array value of `elements`.
#[must_use]
pub fn array(elements: Vec<PropertyEntry>) -> PropertyValue {
    PropertyValue::Array(PropertyArray {
        count: u32::try_from(elements.len()).unwrap_or(u32::MAX),
        elements,
    })
}

/// An array of DIDs (generic element PID).
#[must_use]
pub fn did_array(dids: &[u32]) -> PropertyValue {
    array(
        dids.iter()
            .map(|d| raw(P_DID, PropertyValue::DidRef(Did(*d))))
            .collect(),
    )
}

/// An array of enum keys with element PID `elem`.
#[must_use]
pub fn enum_array(elem: u32, keys: &[u32]) -> PropertyValue {
    array(
        keys.iter()
            .map(|k| raw(elem, PropertyValue::Enum(*k)))
            .collect(),
    )
}
