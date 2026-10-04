// SPDX-License-Identifier: GPL-3.0-or-later
//! Property sets: stand-alone and object property-set records (DID prefixes 0x78–0x7F) and
//! the embedded form used inside other records.
//!
//! Decoding is dictionary-driven: every PID's value type comes from a [`PropertySchema`],
//! normally the [`crate::PropertyDictionary`]. The result is a raw typed tree of
//! [`PropertyEntry`] values with no names; name and label resolution belong to higher
//! layers.
//!
//! Spec: `docs/specs/property-system.md` §5 (layout, algorithm, limits) and §6 (where sets
//! live, the +0x09000000 rule).

use crate::did::Did;
use crate::error::{FormatError, Result};
use crate::property::{PropertyEntry, PropertySchema, PropertyValue, ValueDecoder};
use crate::reader::{Reader, TSize, keys_in_bucket_order};

/// Offset from a game-object DID to its property-set DID (`property-system.md` §6.2).
pub const OBJECT_PROPERTY_SET_OFFSET: u32 = 0x0900_0000;

/// Whether `did`'s prefix is in the property-set range 0x78–0x7F (`property-system.md` §6.1).
///
/// No record with a prefix in 0x7A–0x7F has been observed, so [`Did::kind`] (which follows
/// the `records.md` §2.1 census) reports those prefixes as [`crate::DidKind::Unknown`] and
/// [`crate::is_catalogued_prefix`] returns false for them, although this function accepts
/// them.
#[must_use]
pub fn is_property_set_did(did: impl Into<Did>) -> bool {
    matches!(did.into().prefix(), 0x78..=0x7F)
}

/// The DID of a game object's property set: `did` + 0x09000000 for game-object prefixes
/// 0x70–0x76, otherwise `None` (`property-system.md` §6.2).
///
/// The result is always a property-set DID ([`is_property_set_did`]). The spec states the
/// rule for 0x70–0x77, but 0x77 would map to prefix 0x80, which holds landblock records,
/// so 0x77 is excluded and yields `None` ("no properties").
///
/// The target may not exist (no 0x7F record exists for 0x76 objects); a lookup miss means
/// "no properties". [`Did::kind`] reports a 0x7A–0x7F target as unknown (see
/// [`is_property_set_did`]). Never apply the rule to its own result.
#[must_use]
pub fn object_property_set_did(did: impl Into<Did>) -> Option<Did> {
    let did = did.into();
    if !matches!(did.prefix(), 0x70..=0x77) {
        return None;
    }
    Some(Did(did.0.wrapping_add(OBJECT_PROPERTY_SET_OFFSET))).filter(|&t| is_property_set_did(t))
}

/// Where an entry sits in a property tree, for [`PropertySet::walk`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EntryContext {
    /// A top-level entry of the set.
    TopLevel,
    /// A field of the struct value of property `parent`.
    StructField {
        /// PID of the enclosing struct property.
        parent: u32,
    },
    /// An element of the array value of property `parent`.
    ArrayElement {
        /// PID of the enclosing array property.
        parent: u32,
    },
}

/// One visited entry of a property tree.
#[derive(Debug, Clone, Copy)]
pub struct EntryVisit<'a> {
    /// Nesting depth of the entry's value: 0 for top-level entries.
    pub depth: usize,
    /// Where the entry sits.
    pub context: EntryContext,
    /// The entry.
    pub entry: &'a PropertyEntry,
}

/// A property set: a TSize count followed by entries (`property-system.md` §5.1, the
/// embedded form without a DID).
#[derive(Debug, Clone, PartialEq, Default)]
pub struct PropertySet {
    /// Header: bucket-count hint and entry count (null entries included).
    pub size: TSize,
    /// Top-level entries in stored (bucket) order. Each PID occurs at most once (PS-U1).
    pub entries: Vec<PropertyEntry>,
}

impl PropertySet {
    /// Decodes an embedded property set at the reader's position.
    ///
    /// Fails on an unknown PID ([`FormatError::UnknownProperty`]), a PID-echo mismatch, a
    /// duplicate top-level or struct PID, nesting past [`crate::property::PROPERTY_MAX_DEPTH`],
    /// or truncation. A failing value is wrapped in [`FormatError::PropertyValue`].
    pub fn read<S: PropertySchema + ?Sized>(r: &mut Reader<'_>, schema: &S) -> Result<Self> {
        let size = r.tsize()?;
        let entries = ValueDecoder::new(schema).entries(r, size.count, true, 0)?;
        Ok(Self { size, entries })
    }

    /// Decodes a buffer holding exactly one embedded property set (no leading DID).
    pub fn parse<S: PropertySchema + ?Sized>(data: &[u8], schema: &S) -> Result<Self> {
        let mut r = Reader::new(data);
        let set = Self::read(&mut r, schema)?;
        r.expect_end("property set")?;
        r.log_unknown_hints("property set");
        Ok(set)
    }

    /// The value of top-level property `pid`, if present.
    #[must_use]
    pub fn get(&self, pid: u32) -> Option<&PropertyValue> {
        self.entries.iter().find(|e| e.pid == pid).map(|e| &e.value)
    }

    /// Number of top-level entries (excluding null entries).
    #[must_use]
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Whether the set has no top-level entries.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Iterates the top-level entries in stored order.
    pub fn iter(&self) -> std::slice::Iter<'_, PropertyEntry> {
        self.entries.iter()
    }

    /// Number of null (PID 0) top-level entries the header counted.
    #[must_use]
    pub fn null_entries(&self) -> usize {
        (self.size.count as usize).saturating_sub(self.entries.len())
    }

    /// Whether the top-level entries are in bucket order for the header's hint (PS-O1);
    /// `None` for a hint with no known bucket count.
    #[must_use]
    pub fn is_bucket_ordered(&self) -> Option<bool> {
        keys_in_bucket_order(self.size.hint, self.entries.iter().map(|e| e.pid))
    }

    /// Calls `f` for every entry of the tree, depth first, parents before children.
    pub fn walk<'a>(&'a self, mut f: impl FnMut(EntryVisit<'a>)) {
        for e in &self.entries {
            walk_entry(e, 0, EntryContext::TopLevel, &mut f);
        }
    }

    /// Number of values in the tree, nested values included.
    #[must_use]
    pub fn value_count(&self) -> usize {
        let mut n = 0usize;
        self.walk(|_| n += 1);
        n
    }

    /// Whether every struct in the tree has its fields in bucket order (PS-O2). Structs with
    /// a hint of unknown bucket count are skipped.
    #[must_use]
    pub fn structs_bucket_ordered(&self) -> bool {
        let mut ok = true;
        self.walk(|v| {
            if let PropertyValue::Struct(s) = &v.entry.value
                && s.is_bucket_ordered() == Some(false)
            {
                ok = false;
            }
        });
        ok
    }
}

fn walk_entry<'a>(
    e: &'a PropertyEntry,
    depth: usize,
    context: EntryContext,
    f: &mut impl FnMut(EntryVisit<'a>),
) {
    f(EntryVisit {
        depth,
        context,
        entry: e,
    });
    match &e.value {
        PropertyValue::Struct(s) => {
            for c in &s.fields {
                walk_entry(c, depth + 1, EntryContext::StructField { parent: e.pid }, f);
            }
        }
        PropertyValue::Array(a) => {
            for c in &a.elements {
                walk_entry(
                    c,
                    depth + 1,
                    EntryContext::ArrayElement { parent: e.pid },
                    f,
                );
            }
        }
        _ => {}
    }
}

impl<'t> IntoIterator for &'t PropertySet {
    type Item = &'t PropertyEntry;
    type IntoIter = std::slice::Iter<'t, PropertyEntry>;

    fn into_iter(self) -> Self::IntoIter {
        self.entries.iter()
    }
}

/// A property-set record (`property-system.md` §5.1): its own DID (0x78–0x7F), then one
/// property set.
#[derive(Debug, Clone, PartialEq)]
pub struct PropertySetRecord {
    /// The record's DID.
    pub did: Did,
    /// The property set.
    pub set: PropertySet,
}

impl PropertySetRecord {
    /// Human-readable record type name, used in errors.
    pub const NAME: &'static str = "property set";

    /// Decodes a complete payload, which must be consumed exactly.
    pub fn parse<S: PropertySchema + ?Sized>(data: &[u8], schema: &S) -> Result<Self> {
        let (rec, trailing) = Self::parse_prefix(data, schema)?;
        if trailing != 0 {
            return Err(FormatError::TrailingBytes {
                what: Self::NAME,
                offset: data.len() - trailing,
                count: trailing,
            });
        }
        Ok(rec)
    }

    /// Decodes a payload and checks that its DID matches the one that was looked up.
    pub fn parse_expecting<S: PropertySchema + ?Sized>(
        data: &[u8],
        expected: impl Into<Did>,
        schema: &S,
    ) -> Result<Self> {
        let expected = expected.into();
        let rec = Self::parse(data, schema)?;
        if rec.did != expected {
            return Err(FormatError::DidMismatch {
                expected: expected.0,
                found: rec.did.0,
            });
        }
        Ok(rec)
    }

    /// Decodes the record and returns it with the number of bytes left after the set.
    ///
    /// For censuses: leftover bytes mean the record is malformed or the schema does not
    /// match the data (`property-system.md` §5.3 step 3), and the last decoded entry often
    /// points at the responsible property.
    pub fn parse_prefix<S: PropertySchema + ?Sized>(
        data: &[u8],
        schema: &S,
    ) -> Result<(Self, usize)> {
        let mut r = Reader::new(data);
        let did = r.did()?;
        if !is_property_set_did(did) {
            return Err(FormatError::WrongRecordType {
                did: did.0,
                expected: Self::NAME,
            });
        }
        let set = PropertySet::read(&mut r, schema)?;
        r.log_unknown_hints(Self::NAME);
        Ok((Self { did, set }, r.remaining()))
    }

    /// The record's DID.
    #[must_use]
    pub fn did(&self) -> Did {
        self.did
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tree_nodes_stay_small() {
        // Entries are charged to the reader's budget at their real size; keeping them small
        // (positions inline, rare large values boxed) keeps real records far below it.
        assert!(std::mem::size_of::<crate::Position>() <= 36);
        assert!(std::mem::size_of::<PropertyValue>() <= 40);
        assert!(std::mem::size_of::<PropertyEntry>() <= 48);
    }

    #[test]
    fn object_rule() {
        assert_eq!(
            object_property_set_did(0x7000_1234u32),
            Some(Did(0x7900_1234))
        );
        assert_eq!(
            object_property_set_did(0x7600_0001u32),
            Some(Did(0x7F00_0001))
        );
        // 0x77 + 0x09000000 would land on a landblock DID (0x80…): no property set.
        assert_eq!(object_property_set_did(0x7701_0203u32), None);
        assert_eq!(object_property_set_did(0x77FF_FFFFu32), None);
        for prefix in 0x00..=0xFFu32 {
            if let Some(t) = object_property_set_did(prefix << 24 | 0x12_3456) {
                assert!(is_property_set_did(t), "{prefix:#x} -> {t}");
            }
        }
        assert_eq!(object_property_set_did(0x7800_0001u32), None);
        assert_eq!(object_property_set_did(0x7900_0001u32), None);
        assert_eq!(object_property_set_did(0x6F00_0001u32), None);
        assert!(is_property_set_did(0x7800_0000u32));
        assert!(is_property_set_did(0x7FFF_FFFFu32));
        assert!(!is_property_set_did(0x7700_0000u32));
        assert!(!is_property_set_did(0x8000_0000u32));
    }
}
