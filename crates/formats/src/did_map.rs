// SPDX-License-Identifier: GPL-3.0-or-later
//! DID maps, prefix 0x28 (spec §8).

use std::collections::HashMap;
use std::fmt;
use std::sync::Arc;

use crate::did::{Did, DidKind};
use crate::error::{FormatError, Result};
use crate::reader::{PREALLOC_CAP, Reader, TSize, hash_index_bytes, index_u32};
use crate::record::{Record, read_prologue};

/// Largest number of groups accepted in one DID map.
///
/// Spec §8.1 describes a main group and, in some maps, a second one. This is a resource
/// limit, not a format fact: without it, a payload of empty 4-byte groups would decode to
/// many times its own size.
pub const MAX_DID_MAP_GROUPS: usize = 64;

/// One labelled entry of a DID map.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DataIdEntry {
    /// Key shared by the map and label lists.
    pub key: u32,
    /// Target DID (0 for the "undefined" sentinel).
    pub value: Did,
    /// Human-readable label, shared with the group's label list.
    pub label: Arc<str>,
    /// Index of the group the entry came from (0 = main table).
    pub group: usize,
}

/// Raw contents of one group (spec §8.1), before joining.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DataIdGroup {
    /// TSize of the map list.
    pub map_size: TSize,
    /// (key, DID) pairs in stored order.
    pub map: Vec<(u32, Did)>,
    /// TSize of the label list.
    pub labels_size: TSize,
    /// (key, label) pairs in stored order.
    pub labels: Vec<(u32, Arc<str>)>,
}

/// A decoded DID map: all groups, joined and merged.
///
/// The decoded lists are read-only so that the lookup indexes always agree with them.
#[derive(Clone, PartialEq, Eq)]
pub struct DataIdMap {
    /// Record DID.
    pub did: Did,
    groups: Vec<DataIdGroup>,
    entries: Vec<DataIdEntry>,
    by_label: HashMap<Arc<str>, u32>,
    by_key: HashMap<u32, u32>,
}

impl DataIdMap {
    /// DID for a label.
    #[must_use]
    pub fn by_label(&self, label: &str) -> Option<Did> {
        self.by_label
            .get(label)
            .and_then(|&i| self.entries.get(i as usize))
            .map(|e| e.value)
    }

    /// Entry for a key.
    #[must_use]
    pub fn get(&self, key: u32) -> Option<&DataIdEntry> {
        self.by_key
            .get(&key)
            .and_then(|&i| self.entries.get(i as usize))
    }

    /// Raw groups in stored order.
    #[must_use]
    pub fn groups(&self) -> &[DataIdGroup] {
        &self.groups
    }

    /// Joined entries of all groups, in stored order; keys without a label are skipped.
    #[must_use]
    pub fn entries(&self) -> &[DataIdEntry] {
        &self.entries
    }

    /// Iterates the joined entries in stored order.
    pub fn iter(&self) -> std::slice::Iter<'_, DataIdEntry> {
        self.entries.iter()
    }

    /// Number of joined entries.
    #[must_use]
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Whether the map has no joined entries.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
}

impl<'m> IntoIterator for &'m DataIdMap {
    type Item = &'m DataIdEntry;
    type IntoIter = std::slice::Iter<'m, DataIdEntry>;

    fn into_iter(self) -> Self::IntoIter {
        self.entries.iter()
    }
}

impl fmt::Debug for DataIdMap {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("DataIdMap")
            .field("did", &self.did)
            .field("groups", &self.groups)
            .field("entries", &self.entries)
            .finish_non_exhaustive()
    }
}

fn read_group(r: &mut Reader<'_>) -> Result<DataIdGroup> {
    let map_size = r.tsize()?;
    let mut map = r.alloc_vec(map_size.count, 8, PREALLOC_CAP)?;
    for _ in 0..map_size.count {
        let pair = (r.u32()?, r.did()?);
        r.push(&mut map, pair, map_size.count)?;
    }
    let labels_at = r.position();
    let labels_size = r.tsize()?;
    if labels_size.count != map_size.count {
        return Err(FormatError::malformed(
            DataIdMap::NAME,
            labels_at,
            format!(
                "label count {} differs from map count {}",
                labels_size.count, map_size.count
            ),
        ));
    }
    let mut labels = r.alloc_vec(labels_size.count, 5, PREALLOC_CAP)?;
    for _ in 0..labels_size.count {
        let key = r.u32()?;
        let text = r.short_string()?;
        // The shared label adds two reference counts to the string's bytes.
        r.charge(2 * std::mem::size_of::<usize>())?;
        r.push(&mut labels, (key, Arc::from(text)), labels_size.count)?;
    }
    Ok(DataIdGroup {
        map_size,
        map,
        labels_size,
        labels,
    })
}

impl Record<'_> for DataIdMap {
    const NAME: &'static str = "DID map";

    fn did(&self) -> Did {
        self.did
    }

    fn parse(data: &[u8]) -> Result<Self> {
        let mut r = Reader::new(data);
        let did = read_prologue(&mut r, Self::NAME, |k| k == DidKind::DataIdMap)?;
        let mut groups = Vec::new();
        loop {
            if groups.len() == MAX_DID_MAP_GROUPS {
                return Err(FormatError::malformed(
                    Self::NAME,
                    r.position(),
                    format!("more than {MAX_DID_MAP_GROUPS} groups"),
                ));
            }
            groups.push(read_group(&mut r)?);
            if r.is_empty() {
                break;
            }
        }
        let total = groups.iter().map(|g| g.map.len()).sum::<usize>();
        let total = u32::try_from(total).unwrap_or(u32::MAX);
        let mut entries = Vec::new();
        let mut unlabelled = 0usize;
        for (gi, group) in groups.iter().enumerate() {
            // Transient per-group join table; charged although it is freed again.
            r.charge(hash_index_bytes::<u32, &Arc<str>>(group.labels.len()))?;
            let mut labels: HashMap<u32, &Arc<str>> = HashMap::with_capacity(group.labels.len());
            for (k, l) in &group.labels {
                labels.insert(*k, l);
            }
            for &(key, value) in &group.map {
                let Some(&label) = labels.get(&key) else {
                    unlabelled += 1;
                    continue;
                };
                let entry = DataIdEntry {
                    key,
                    value,
                    label: Arc::clone(label),
                    group: gi,
                };
                r.push(&mut entries, entry, total)?;
            }
        }
        r.log_unknown_hints(Self::NAME);
        // Indexes are built once at their final size, so they never grow by doubling.
        r.charge(
            hash_index_bytes::<Arc<str>, u32>(entries.len()).saturating_add(hash_index_bytes::<
                u32,
                u32,
            >(
                entries.len()
            )),
        )?;
        let mut by_label: HashMap<Arc<str>, u32> = HashMap::with_capacity(entries.len());
        let mut by_key = HashMap::with_capacity(entries.len());
        for (i, e) in entries.iter().enumerate() {
            if !by_label.contains_key(&*e.label) {
                by_label.insert(Arc::clone(&e.label), index_u32(i));
            }
            by_key.entry(e.key).or_insert(index_u32(i));
        }
        if unlabelled != 0 {
            tracing::debug!(%did, count = unlabelled, "DID-map keys without label skipped");
        }
        Ok(Self {
            did,
            groups,
            entries,
            by_label,
            by_key,
        })
    }
}
