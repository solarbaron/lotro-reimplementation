// SPDX-License-Identifier: GPL-3.0-or-later
//! The structural kinds of the WEENIECONTENT entries (`game-controls.md` §2.1).

use std::collections::BTreeMap;
use std::sync::Arc;

use tessera_formats::Did;
use tessera_props::RecordSource;

use super::generic::is_empty_marker;
use crate::error::Result;
use crate::library::ClassLibrary;
use crate::state::{ObjectHeader, is_object_did};
use crate::store::{ObjectStore, WEENIE_MAP_LABEL};

/// The kind of a WEENIECONTENT entry, in the order of the rule table of §2.1.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum EntryKind {
    /// Row 1: the entry's DID is 0.
    Null,
    /// Row 2: the DID has prefix 0x07 (a script table, `script-table.md`).
    ScriptTable,
    /// Row 3: the root class is the DID-set class (§3.3).
    DidSet,
    /// Row 4: the root class's parent is the dialog base (§3.2).
    DialogMarker,
    /// Row 5: a 36-byte object state of one script object with no attributes and a 6-byte
    /// property set with no entries (§3.2).
    EmptyMarker,
    /// Row 6: the root class is the skill class (`combat-data.md` §7).
    Skill,
    /// Row 7: the root class is the effect base or derives from it (`combat-data.md` §6).
    Effect,
    /// Row 8: the root class's parent is the manager base (§3.4).
    Manager,
    /// Row 9: the entity base is an ancestor of the root class.
    Entity,
    /// Row 10: the plain-object base is an ancestor of the root class (§3.1).
    PropertyControl,
}

impl EntryKind {
    /// Every kind, in rule order.
    pub const ALL: [Self; 10] = [
        Self::Null,
        Self::ScriptTable,
        Self::DidSet,
        Self::DialogMarker,
        Self::EmptyMarker,
        Self::Skill,
        Self::Effect,
        Self::Manager,
        Self::Entity,
        Self::PropertyControl,
    ];

    /// The row of the rule table (1–10).
    #[must_use]
    pub fn row(self) -> u8 {
        match self {
            Self::Null => 1,
            Self::ScriptTable => 2,
            Self::DidSet => 3,
            Self::DialogMarker => 4,
            Self::EmptyMarker => 5,
            Self::Skill => 6,
            Self::Effect => 7,
            Self::Manager => 8,
            Self::Entity => 9,
            Self::PropertyControl => 10,
        }
    }

    /// A short name for reports.
    #[must_use]
    pub fn name(self) -> &'static str {
        match self {
            Self::Null => "null entry",
            Self::ScriptTable => "script table",
            Self::DidSet => "DID set",
            Self::DialogMarker => "dialog marker",
            Self::EmptyMarker => "other empty marker",
            Self::Skill => "skill",
            Self::Effect => "effect",
            Self::Manager => "manager entity",
            Self::Entity => "other entity or item",
            Self::PropertyControl => "property control",
        }
    }
}

/// The class indices the kind rule needs (§2.1 "Class indices"). They are
/// build-specific; [`ClassIndices::OBSERVED`] holds the values the spec observed, and a
/// new build replaces the table. [`ClassIndices::check`] runs the structural
/// cross-checks against a class library.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ClassIndices {
    /// The common ancestors of all object entries: the root of the parent map, its child
    /// and that child's child.
    pub common_ancestors: [u32; 3],
    /// The plain-object base (rows 3–7 and 10 descend from it).
    pub plain_object_base: u32,
    /// The entity base (rows 8 and 9 descend from it).
    pub entity_base: u32,
    /// The DID-set class (row 3).
    pub did_set: u32,
    /// The dialog base (row 4, by parent).
    pub dialog_base: u32,
    /// The skill class (row 6).
    pub skill: u32,
    /// The effect base (row 7).
    pub effect_base: u32,
    /// The manager base (row 8, by parent).
    pub manager_base: u32,
}

impl ClassIndices {
    /// The class indices observed on the spec's install (2026-10-07).
    pub const OBSERVED: Self = Self {
        common_ancestors: [484, 485, 89],
        plain_object_base: 486,
        entity_base: 223,
        did_set: 3399,
        dialog_base: 1728,
        skill: 827,
        effect_base: 698,
        manager_base: 788,
    };

    /// The structural cross-checks of §2.1 that the class library alone can answer:
    /// the first common ancestor is a root of the parent map, the second its child and
    /// the third the second's child; every base class descends from the third; the
    /// plain-object and entity bases are not related. Classes need not have a class-table
    /// definition (a base class may appear only in the parent map). An empty result means
    /// all hold.
    #[must_use]
    pub fn check(&self, lib: &ClassLibrary) -> Vec<IndexProblem> {
        let mut out = Vec::new();
        let [root, child, grandchild] = self.common_ancestors;
        if lib.parent(root).is_some() {
            out.push(IndexProblem::NotRoot(root));
        }
        if lib.parent(child) != Some(root) {
            out.push(IndexProblem::WrongParent {
                class: child,
                expected: root,
            });
        }
        if lib.parent(grandchild) != Some(child) {
            out.push(IndexProblem::WrongParent {
                class: grandchild,
                expected: child,
            });
        }
        for class in [
            self.plain_object_base,
            self.entity_base,
            self.did_set,
            self.dialog_base,
            self.skill,
            self.effect_base,
            self.manager_base,
        ] {
            if class == grandchild || !lib.is_a(class, grandchild) {
                out.push(IndexProblem::NotDescendant {
                    class,
                    ancestor: grandchild,
                });
            }
        }
        for class in [self.did_set, self.dialog_base, self.skill, self.effect_base] {
            if !lib.is_a(class, self.plain_object_base) {
                out.push(IndexProblem::NotDescendant {
                    class,
                    ancestor: self.plain_object_base,
                });
            }
        }
        if !lib.is_a(self.manager_base, self.entity_base) {
            out.push(IndexProblem::NotDescendant {
                class: self.manager_base,
                ancestor: self.entity_base,
            });
        }
        if lib.is_a(self.plain_object_base, self.entity_base)
            || lib.is_a(self.entity_base, self.plain_object_base)
        {
            out.push(IndexProblem::BasesRelated);
        }
        out
    }
}

impl Default for ClassIndices {
    fn default() -> Self {
        Self::OBSERVED
    }
}

/// A failed cross-check of a [`ClassIndices`] table.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum IndexProblem {
    /// The class has a parent but should be the root.
    NotRoot(u32),
    /// The class's parent is not the expected one.
    WrongParent {
        /// The class.
        class: u32,
        /// The expected parent.
        expected: u32,
    },
    /// The class does not derive from the expected ancestor.
    NotDescendant {
        /// The class.
        class: u32,
        /// The expected ancestor.
        ancestor: u32,
    },
    /// The plain-object and entity bases derive one from the other.
    BasesRelated,
}

/// Applies the ordered rule of §2.1 to one entry: `did` is the entry's DID, `root_class`
/// the root class of its object record (`None` when the DID is not an object DID or the
/// record is absent), and `empty_marker` answers the row-5 test (called only when rows
/// 1–4 do not match and a root class is known). `Ok(None)` means no row matched (an
/// invariant failure, GCI15).
///
/// # Errors
///
/// Whatever `empty_marker` returns.
pub fn classify(
    did: Did,
    root_class: Option<u32>,
    lib: &ClassLibrary,
    idx: &ClassIndices,
    empty_marker: impl FnOnce() -> Result<bool>,
) -> Result<Option<EntryKind>> {
    if did.is_null() {
        return Ok(Some(EntryKind::Null));
    }
    if did.prefix() == 0x07 {
        return Ok(Some(EntryKind::ScriptTable));
    }
    let Some(c) = root_class else {
        return Ok(None);
    };
    let parent = lib.parent(c);
    if c == idx.did_set {
        return Ok(Some(EntryKind::DidSet));
    }
    if parent == Some(idx.dialog_base) {
        return Ok(Some(EntryKind::DialogMarker));
    }
    if empty_marker()? {
        return Ok(Some(EntryKind::EmptyMarker));
    }
    if c == idx.skill {
        return Ok(Some(EntryKind::Skill));
    }
    if lib.is_a(c, idx.effect_base) {
        return Ok(Some(EntryKind::Effect));
    }
    if parent == Some(idx.manager_base) {
        return Ok(Some(EntryKind::Manager));
    }
    if c != idx.entity_base && lib.is_a(c, idx.entity_base) {
        return Ok(Some(EntryKind::Entity));
    }
    if c != idx.plain_object_base && lib.is_a(c, idx.plain_object_base) {
        return Ok(Some(EntryKind::PropertyControl));
    }
    Ok(None)
}

/// One WEENIECONTENT entry with its kind.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WeenieEntry {
    /// The map key (an observation of the install).
    pub key: u32,
    /// The label.
    pub label: Arc<str>,
    /// The DID the entry names (0 for the null entry).
    pub did: Did,
    /// The root class of the object record, if the DID names an existing object.
    pub root_class: Option<u32>,
    /// The kind, or `None` when no row of the rule matched.
    pub kind: Option<EntryKind>,
}

/// Every entry of the WEENIECONTENT map, classified (§2.1), in stored order.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct WeenieIndex {
    /// The entries.
    pub entries: Vec<WeenieEntry>,
}

impl WeenieIndex {
    /// Reads the WEENIECONTENT map through the root DID map and classifies every entry
    /// with `idx`. Only object headers are read, plus the payloads of entries that reach
    /// row 5 (whose test needs the record sizes and a decode of 36-byte records).
    ///
    /// # Errors
    ///
    /// A failed read, a map that does not decode, or a class library that fails to load.
    /// An object record whose header does not parse leaves the entry unclassified.
    pub fn build<S: RecordSource>(store: &ObjectStore<S>, idx: &ClassIndices) -> Result<Self> {
        let data = store.game_data();
        let Some(map) = data.did_map_by_label(WEENIE_MAP_LABEL)? else {
            tracing::warn!("no WEENIECONTENT map in this install");
            return Ok(Self::default());
        };
        let lib = store.library()?;
        let mut entries = Vec::with_capacity(map.len().min(super::MAX_VIEW_ENTRIES));
        for e in map.iter().take(super::MAX_VIEW_ENTRIES) {
            let root_class = if is_object_did(e.value) {
                match data.read(e.value)? {
                    Some(bytes) => match ObjectHeader::parse(&bytes) {
                        Ok(h) => Some(h.root_class),
                        Err(err) => {
                            tracing::warn!(did = %e.value, error = %err, "object header does not parse");
                            None
                        }
                    },
                    None => None,
                }
            } else {
                None
            };
            let kind = classify(e.value, root_class, &lib, idx, || {
                empty_marker_record(store, e.value)
            })?;
            if kind.is_none() {
                tracing::debug!(label = %e.label, did = %e.value, "WEENIECONTENT entry matches no kind");
            }
            entries.push(WeenieEntry {
                key: e.key,
                label: Arc::clone(&e.label),
                did: e.value,
                root_class,
                kind,
            });
        }
        Ok(Self { entries })
    }

    /// The entry labelled `label`.
    #[must_use]
    pub fn get(&self, label: &str) -> Option<&WeenieEntry> {
        self.entries.iter().find(|e| &*e.label == label)
    }

    /// Number of entries per kind (unclassified entries under `None`).
    #[must_use]
    pub fn counts(&self) -> BTreeMap<Option<EntryKind>, usize> {
        let mut out = BTreeMap::new();
        for e in &self.entries {
            *out.entry(e.kind).or_insert(0) += 1;
        }
        out
    }

    /// The entries of `kind`.
    pub fn of_kind(&self, kind: EntryKind) -> impl Iterator<Item = &WeenieEntry> + '_ {
        self.entries.iter().filter(move |e| e.kind == Some(kind))
    }
}

/// The row-5 test on the records of object `did`: the object payload is
/// [`super::EMPTY_MARKER_STATE_LEN`] bytes and decodes to one script object with no
/// attributes, and the property set is [`super::EMPTY_PROPERTY_SET_LEN`] bytes with no
/// entries. A record that fails to decode is not an empty marker.
fn empty_marker_record<S: RecordSource>(store: &ObjectStore<S>, did: Did) -> Result<bool> {
    let data = store.game_data();
    let Some(bytes) = data.read(did)? else {
        return Ok(false);
    };
    if bytes.len() != super::EMPTY_MARKER_STATE_LEN {
        return Ok(false);
    }
    let Some(set_did) = did.0.checked_add(0x0900_0000).map(Did) else {
        return Ok(false);
    };
    let Some(set_bytes) = data.read(set_did)? else {
        return Ok(false);
    };
    let Ok(state) = store.decode(did, &bytes) else {
        return Ok(false);
    };
    let Ok(Some(set)) = data.property_set(set_did) else {
        return Ok(false);
    };
    Ok(is_empty_marker(bytes.len(), &state, set_bytes.len(), &set))
}
