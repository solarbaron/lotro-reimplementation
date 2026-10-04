// SPDX-License-Identifier: GPL-3.0-or-later
//! Property sets with PID and name lookup (`property-system.md` §5).

use std::collections::HashMap;

use tessera_formats::{Did, PropertyEntry, PropertySetRecord, Reader};

use crate::error::{PropsError, Result};
use crate::registry::PropertyRegistry;
use crate::value::{Property, Value};

/// Sets with at most this many entries are searched linearly; larger ones keep a PID
/// index. Most structs have a handful of fields; top-level sets reach several hundred.
const INDEX_THRESHOLD: usize = 12;

/// A collection of properties, each PID at most once (`property-system.md` §5.5), kept in
/// stored order.
///
/// Used for top-level property sets and for struct values. Lookups are by PID
/// ([`PropertySet::get`]) or by name through a [`PropertyRegistry`]
/// ([`PropertySet::get_by_name`]).
#[derive(Debug, Clone, Default)]
pub struct PropertySet {
    entries: Vec<Property>,
    /// PID → position in `entries`; empty when `entries` has at most
    /// [`INDEX_THRESHOLD`] entries.
    index: HashMap<u32, usize>,
}

impl PartialEq for PropertySet {
    fn eq(&self, other: &Self) -> bool {
        self.entries == other.entries
    }
}

impl PropertySet {
    /// An empty set.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Builds a set from properties in the given order. A repeated PID keeps its first
    /// position and its last value, as inserting into a map does (§5.5). Linear in the
    /// number of properties.
    #[must_use]
    pub fn from_properties(props: impl IntoIterator<Item = Property>) -> Self {
        let props = props.into_iter();
        let mut entries: Vec<Property> = Vec::with_capacity(props.size_hint().0);
        let mut positions: HashMap<u32, usize> = HashMap::with_capacity(entries.capacity());
        let mut dupes = false;
        for p in props {
            match positions.get(&p.pid).and_then(|&i| entries.get_mut(i)) {
                Some(slot) => {
                    slot.value = p.value;
                    dupes = true;
                }
                None => {
                    positions.insert(p.pid, entries.len());
                    entries.push(p);
                }
            }
        }
        if dupes {
            tracing::debug!("property set built with repeated PIDs; last values kept");
        }
        if entries.len() <= INDEX_THRESHOLD {
            positions = HashMap::new();
        }
        Self {
            entries,
            index: positions,
        }
    }

    /// Converts a raw decoded set from `tessera-formats`, consuming it.
    pub fn from_raw(raw: tessera_formats::PropertySet) -> Result<Self> {
        Self::from_raw_entries(raw.entries)
    }

    pub(crate) fn from_raw_entries(raw: Vec<PropertyEntry>) -> Result<Self> {
        let mut entries = Vec::with_capacity(raw.len());
        for e in raw {
            entries.push(Property::new(e.pid, Value::from_raw_pid(e.value, e.pid)?));
        }
        // The raw decoder already rejects repeated PIDs (PS-U1, PS-U2); a set from another
        // source goes through the same de-duplication as `from_properties`.
        if has_duplicates(&entries) {
            return Ok(Self::from_properties(entries));
        }
        let mut set = Self {
            entries,
            index: HashMap::new(),
        };
        set.rebuild_index();
        Ok(set)
    }

    /// Decodes a complete property-set record (prefix 0x78–0x7F), checking that its DID
    /// is `expected`. Value types come from `registry`.
    pub fn parse_record(data: &[u8], expected: Did, registry: &PropertyRegistry) -> Result<Self> {
        let rec = PropertySetRecord::parse_expecting(data, expected, registry)
            .map_err(|e| PropsError::decode(expected, PropertySetRecord::NAME, e))?;
        Self::from_raw(rec.set)
    }

    /// Like [`PropertySet::parse_record`], but bytes after the set are allowed: returns
    /// the set and the number of trailing bytes. For tools that list records with
    /// unexplained trailing data (`property-system.md` §11) instead of rejecting them.
    pub fn parse_record_prefix(
        data: &[u8],
        expected: Did,
        registry: &PropertyRegistry,
    ) -> Result<(Self, usize)> {
        let decode = |e| PropsError::decode(expected, PropertySetRecord::NAME, e);
        let (rec, trailing) = PropertySetRecord::parse_prefix(data, registry).map_err(decode)?;
        if rec.did != expected {
            return Err(decode(tessera_formats::FormatError::DidMismatch {
                expected: expected.0,
                found: rec.did.0,
            }));
        }
        Ok((Self::from_raw(rec.set)?, trailing))
    }

    /// Decodes a buffer holding exactly one embedded property set (a TSize count and
    /// entries, no DID; `property-system.md` §5.1); trailing bytes are an error. `did` is
    /// the containing record, for error messages. For a set followed by more data, use
    /// [`PropertySet::parse_embedded_prefix`] or [`PropertySet::read_embedded`].
    pub fn parse_embedded(data: &[u8], did: Did, registry: &PropertyRegistry) -> Result<Self> {
        let raw = tessera_formats::PropertySet::parse(data, registry)
            .map_err(|e| PropsError::decode(did, "embedded property set", e))?;
        Self::from_raw(raw)
    }

    /// Decodes an embedded property set at the start of `data`, which may continue with
    /// other fields of the containing record (§6.3). Returns the set and the number of
    /// bytes it occupied.
    pub fn parse_embedded_prefix(
        data: &[u8],
        did: Did,
        registry: &PropertyRegistry,
    ) -> Result<(Self, usize)> {
        let mut r = Reader::new(data);
        let set = Self::read_embedded(&mut r, did, registry)?;
        Ok((set, r.position()))
    }

    /// Decodes an embedded property set at the reader's position and advances past it,
    /// for decoders of records that contain a set among other fields.
    pub fn read_embedded(
        r: &mut Reader<'_>,
        did: Did,
        registry: &PropertyRegistry,
    ) -> Result<Self> {
        let raw = tessera_formats::PropertySet::read(r, registry)
            .map_err(|e| PropsError::decode(did, "embedded property set", e))?;
        Self::from_raw(raw)
    }

    fn rebuild_index(&mut self) {
        self.index.clear();
        if self.entries.len() > INDEX_THRESHOLD {
            self.index = self
                .entries
                .iter()
                .enumerate()
                .map(|(i, e)| (e.pid, i))
                .collect();
        }
    }

    fn position(&self, pid: u32) -> Option<usize> {
        if self.index.is_empty() {
            return self.entries.iter().position(|e| e.pid == pid);
        }
        self.index.get(&pid).copied()
    }

    /// The value of property `pid`, if present.
    #[must_use]
    pub fn get(&self, pid: u32) -> Option<&Value> {
        self.position(pid)
            .and_then(|i| self.entries.get(i))
            .map(|e| &e.value)
    }

    /// The value of the property named `name` in `registry`, if defined and present.
    #[must_use]
    pub fn get_by_name(&self, registry: &PropertyRegistry, name: &str) -> Option<&Value> {
        registry.pid(name).and_then(|pid| self.get(pid))
    }

    /// Whether property `pid` is present.
    #[must_use]
    pub fn contains(&self, pid: u32) -> bool {
        self.position(pid).is_some()
    }

    /// Sets property `pid`, returning the previous value. A new PID is appended.
    /// Constant time on average.
    pub fn insert(&mut self, pid: u32, value: Value) -> Option<Value> {
        if let Some(slot) = self.position(pid).and_then(|i| self.entries.get_mut(i)) {
            return Some(std::mem::replace(&mut slot.value, value));
        }
        let i = self.entries.len();
        self.entries.push(Property::new(pid, value));
        if !self.index.is_empty() {
            self.index.insert(pid, i);
        } else if self.entries.len() > INDEX_THRESHOLD {
            self.rebuild_index();
        }
        None
    }

    /// Removes property `pid`, returning its value; later properties keep their order.
    /// Linear in the number of properties.
    pub fn remove(&mut self, pid: u32) -> Option<Value> {
        let i = self.position(pid)?;
        let p = self.entries.remove(i);
        if self.entries.len() <= INDEX_THRESHOLD {
            self.index = HashMap::new();
        } else {
            self.index.remove(&pid);
            for pos in self.index.values_mut() {
                if *pos > i {
                    *pos -= 1;
                }
            }
        }
        Some(p.value)
    }

    /// Number of properties.
    #[must_use]
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Whether the set is empty.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// The properties in stored order.
    #[must_use]
    pub fn as_slice(&self) -> &[Property] {
        &self.entries
    }

    /// Iterates the properties in stored order.
    pub fn iter(&self) -> std::slice::Iter<'_, Property> {
        self.entries.iter()
    }

    /// Iterates (PID, name, value) in stored order; the name is `None` for a PID the
    /// registry does not name.
    pub fn iter_named<'a>(
        &'a self,
        registry: &'a PropertyRegistry,
    ) -> impl Iterator<Item = (u32, Option<&'a str>, &'a Value)> + 'a {
        self.entries
            .iter()
            .map(move |p| (p.pid, registry.name(p.pid), &p.value))
    }

    /// Visits every property of the tree depth-first in stored order: top-level
    /// properties at depth 0, struct fields and array elements one level below their
    /// parent.
    pub fn walk<'a>(&'a self, mut f: impl FnMut(usize, &'a Property)) {
        // Explicit stack: decoded values are shallow, but a caller-built tree may be deep
        // (see the stack note on `Value`).
        let mut stack: Vec<(usize, std::slice::Iter<'a, Property>)> = vec![(0, self.iter())];
        while let Some((depth, it)) = stack.last_mut() {
            let depth = *depth;
            match it.next() {
                Some(p) => {
                    f(depth, p);
                    let kids = p.value.children();
                    if !kids.is_empty() {
                        stack.push((depth + 1, kids.iter()));
                    }
                }
                None => {
                    stack.pop();
                }
            }
        }
    }

    /// Total number of properties in the tree, nested ones included.
    #[must_use]
    pub fn value_count(&self) -> usize {
        let mut n = 0;
        self.walk(|_, _| n += 1);
        n
    }

    /// Nesting depth of the tree: 0 for an empty set, 1 when no property holds a
    /// non-empty struct or array, and one more per level below. Computed without
    /// recursion, so it is safe on any tree.
    #[must_use]
    pub fn depth(&self) -> usize {
        let mut max = 0;
        self.walk(|d, _| max = max.max(d + 1));
        max
    }
}

impl<'a> IntoIterator for &'a PropertySet {
    type Item = &'a Property;
    type IntoIter = std::slice::Iter<'a, Property>;

    fn into_iter(self) -> Self::IntoIter {
        self.entries.iter()
    }
}

impl FromIterator<Property> for PropertySet {
    fn from_iter<T: IntoIterator<Item = Property>>(iter: T) -> Self {
        Self::from_properties(iter)
    }
}

fn has_duplicates(entries: &[Property]) -> bool {
    if entries.len() < 2 {
        return false;
    }
    let mut pids: Vec<u32> = entries.iter().map(|e| e.pid).collect();
    pids.sort_unstable();
    pids.windows(2).any(|w| w.first() == w.get(1))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn big_set(n: u32) -> PropertySet {
        (0..n)
            .rev()
            .map(|i| Property::new(0x1000_0000 + i * 7, Value::Int(i as i32)))
            .collect()
    }

    #[test]
    fn lookup_small_and_indexed() {
        for n in [0, 1, 5, 12, 13, 40, 811] {
            let s = big_set(n);
            assert_eq!(s.len(), n as usize);
            for i in 0..n {
                assert_eq!(s.get(0x1000_0000 + i * 7), Some(&Value::Int(i as i32)));
            }
            assert_eq!(s.get(0x1000_0001), None);
            assert_eq!(s.get(0), None);
            assert!(!s.contains(u32::MAX));
        }
    }

    #[test]
    fn insert_remove_keep_index_consistent() {
        let mut s = big_set(12);
        assert_eq!(s.insert(5, Value::Bool(true)), None);
        assert_eq!(s.len(), 13);
        assert_eq!(s.get(5), Some(&Value::Bool(true)));
        assert_eq!(s.insert(5, Value::Bool(false)), Some(Value::Bool(true)));
        assert_eq!(s.remove(5), Some(Value::Bool(false)));
        assert_eq!(s.remove(5), None);
        assert_eq!(s.len(), 12);
        assert_eq!(s.get(0x1000_0000), Some(&Value::Int(0)));
    }

    #[test]
    fn duplicates_keep_first_position_last_value() {
        let s = PropertySet::from_properties([
            Property::new(1, Value::Int(1)),
            Property::new(2, Value::Int(2)),
            Property::new(1, Value::Int(3)),
        ]);
        assert_eq!(s.len(), 2);
        assert_eq!(s.as_slice()[0], Property::new(1, Value::Int(3)));
    }

    #[test]
    fn walk_visits_nested() {
        let inner = PropertySet::from_properties([Property::new(3, Value::Bool(true))]);
        let s = PropertySet::from_properties([
            Property::new(1, Value::from(inner)),
            Property::new(
                2,
                Value::Array(vec![
                    Property::new(4, Value::Int(1)),
                    Property::new(4, Value::Int(2)),
                ]),
            ),
        ]);
        let mut seen = Vec::new();
        s.walk(|d, p| seen.push((d, p.pid)));
        assert_eq!(seen, vec![(0, 1), (1, 3), (0, 2), (1, 4), (1, 4)]);
        assert_eq!(s.value_count(), 5);
    }

    #[test]
    fn index_stays_consistent_through_inserts_and_removes() {
        let mut s = PropertySet::new();
        for i in 0..200u32 {
            assert_eq!(s.insert(1000 - i, Value::Int(i as i32)), None);
        }
        // Remove from the middle, the front and the back; order of the rest is kept.
        for pid in [900, 1000, 801, 850] {
            assert!(s.remove(pid).is_some());
            assert!(!s.contains(pid));
        }
        assert_eq!(s.len(), 196);
        let order: Vec<u32> = s.iter().map(|p| p.pid).collect();
        let expected: Vec<u32> = (0..200u32)
            .map(|i| 1000 - i)
            .filter(|p| ![900, 1000, 801, 850].contains(p))
            .collect();
        assert_eq!(order, expected);
        for p in &expected {
            assert_eq!(s.get(*p), Some(&Value::Int((1000 - p) as i32)));
        }
        // Shrink below the threshold and grow again.
        for p in expected.iter().skip(5) {
            s.remove(*p);
        }
        assert_eq!(s.len(), 5);
        assert!(s.index.is_empty());
        for p in 1..=20u32 {
            s.insert(p, Value::Bool(true));
        }
        assert_eq!(s.len(), 25);
        assert!(!s.index.is_empty());
        assert_eq!(s.get(20), Some(&Value::Bool(true)));
        assert_eq!(s.get(expected[0]), Some(&Value::Int(1)));
    }

    #[test]
    fn building_large_sets_is_not_quadratic() {
        let n = 200_000u32;
        let t = std::time::Instant::now();
        let collected: PropertySet = (0..n)
            .map(|i| Property::new(i + 1, Value::Int(0)))
            .collect();
        let mut inserted = PropertySet::new();
        for i in 0..n {
            inserted.insert(i + 1, Value::Int(0));
        }
        // Quadratic versions took minutes at this size even in release builds.
        assert!(t.elapsed() < std::time::Duration::from_secs(20));
        assert_eq!(collected.len(), n as usize);
        assert_eq!(inserted, collected);
        assert_eq!(inserted.get(n), Some(&Value::Int(0)));
    }

    #[test]
    fn depth_counts_levels() {
        assert_eq!(PropertySet::new().depth(), 0);
        assert_eq!(big_set(3).depth(), 1);
        let inner = PropertySet::from_properties([Property::new(3, Value::Bool(true))]);
        let s = PropertySet::from_properties([Property::new(
            1,
            Value::Array(vec![Property::new(2, Value::from(inner))]),
        )]);
        assert_eq!(s.depth(), 3);
    }

    #[test]
    fn walk_survives_very_deep_caller_built_tree() {
        let mut v = Value::Int(0);
        for _ in 0..10_000 {
            v = Value::Array(vec![Property::new(1, v)]);
        }
        let s = PropertySet::from_properties([Property::new(1, v)]);
        assert_eq!(s.value_count(), 10_001);
        assert_eq!(s.depth(), 10_001);
        // Drop a deep tree without overflowing: unwind it iteratively.
        let mut s = s;
        let mut cur = s.remove(1);
        while let Some(Value::Array(mut a)) = cur {
            cur = a.pop().map(|p| p.value);
        }
    }
}
