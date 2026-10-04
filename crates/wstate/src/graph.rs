// SPDX-License-Identifier: GPL-3.0-or-later
//! Typed views over a decoded object graph and reference resolution inside one record.
//!
//! Spec: `docs/specs/wstate.md` §8.

use std::collections::VecDeque;

use tessera_formats::{Did, Position, StringInfo, name_hash};

use crate::library::ValueType;
use crate::native::{Native, Ref};
use crate::state::{Body, Item, ObjectState, ScriptObject, Value, item_references, script_layout};

/// Result of resolving a reference inside one bundle (`wstate.md` §8.2).
#[derive(Debug, Clone, Copy)]
pub enum Resolved<'a> {
    /// The null reference.
    Null,
    /// The item the label names.
    Item(ItemView<'a>),
    /// The label exists but its item was not decoded (partial record, §5.5).
    Undecoded {
        /// Item index of the label.
        index: usize,
    },
    /// No item has this label.
    Dangling {
        /// The label.
        label: u32,
    },
}

impl<'a> Resolved<'a> {
    /// The item, if the reference resolved to a decoded one.
    #[must_use]
    pub fn item(self) -> Option<ItemView<'a>> {
        match self {
            Self::Item(v) => Some(v),
            _ => None,
        }
    }

    /// Whether this is the null reference.
    #[must_use]
    pub fn is_null(&self) -> bool {
        matches!(self, Self::Null)
    }
}

/// A view of one item of a bundle.
#[derive(Debug, Clone, Copy)]
pub struct ItemView<'a> {
    state: &'a ObjectState,
    index: usize,
    item: &'a Item,
}

impl ObjectState {
    /// A view of item `index`, if it was decoded.
    #[must_use]
    pub fn item(&self, index: usize) -> Option<ItemView<'_>> {
        self.bundle.items.get(index).map(|item| ItemView {
            state: self,
            index,
            item,
        })
    }

    /// The root item (item 0).
    #[must_use]
    pub fn root(&self) -> Option<ItemView<'_>> {
        self.item(0)
    }

    /// The root script object, if the root item is one.
    #[must_use]
    pub fn root_object(&self) -> Option<ScriptView<'_>> {
        self.root().and_then(ItemView::as_script)
    }

    /// The item whose label is `label`.
    #[must_use]
    pub fn item_by_label(&self, label: u32) -> Option<ItemView<'_>> {
        self.bundle.index_of(label).and_then(|i| self.item(i))
    }

    /// Resolves a reference value (`wstate.md` §8.2 steps 1–2).
    #[must_use]
    pub fn resolve(&self, r: Ref) -> Resolved<'_> {
        let Some(label) = r.label() else {
            return Resolved::Null;
        };
        match self.bundle.index_of(label) {
            None => Resolved::Dangling { label },
            Some(i) => match self.item(i) {
                Some(v) => Resolved::Item(v),
                None => Resolved::Undecoded { index: i },
            },
        }
    }

    /// Item indices reachable from `start` (included), breadth first, each visited once.
    /// Cycles are safe (`wstate.md` §8.2 step 4). DID items are leaves: following them
    /// needs another record ([`crate::ObjectStore`]).
    #[must_use]
    pub fn reachable(&self, start: usize) -> Vec<usize> {
        let n = self.bundle.items.len();
        if start >= n {
            return Vec::new();
        }
        let mut seen = vec![false; n];
        let mut order = Vec::new();
        let mut queue = VecDeque::from([start]);
        seen[start] = true;
        while let Some(i) = queue.pop_front() {
            order.push(i);
            let Some(item) = self.bundle.items.get(i) else {
                continue;
            };
            for r in item_references(&self.bundle, item) {
                if let Some(j) = r.label().and_then(|l| self.bundle.index_of(l))
                    && j < n
                    && !std::mem::replace(&mut seen[j], true)
                {
                    queue.push_back(j);
                }
            }
        }
        order
    }

    /// How many references point at each item (by item index).
    #[must_use]
    pub fn reference_counts(&self) -> Vec<u32> {
        let mut counts = vec![0u32; self.bundle.labels.len()];
        for item in &self.bundle.items {
            for r in item_references(&self.bundle, item) {
                if let Some(c) = r
                    .label()
                    .and_then(|l| self.bundle.index_of(l))
                    .and_then(|j| counts.get_mut(j))
                {
                    *c = c.saturating_add(1);
                }
            }
        }
        counts
    }
}

impl<'a> ItemView<'a> {
    /// The record this item belongs to.
    #[must_use]
    pub fn state(&self) -> &'a ObjectState {
        self.state
    }

    /// Item index.
    #[must_use]
    pub fn index(&self) -> usize {
        self.index
    }

    /// The item's label.
    #[must_use]
    pub fn label(&self) -> u32 {
        self.state
            .bundle
            .labels
            .get(self.index)
            .copied()
            .unwrap_or(u32::MAX)
    }

    /// The raw item.
    #[must_use]
    pub fn item(&self) -> &'a Item {
        self.item
    }

    /// Class index of an embedded object.
    #[must_use]
    pub fn class(&self) -> Option<u32> {
        self.item().class()
    }

    /// The DID, if this is a DID item (a link to another object record).
    #[must_use]
    pub fn as_did(&self) -> Option<Did> {
        self.item().as_did()
    }

    /// The native body, if this is a native object.
    #[must_use]
    pub fn as_native(&self) -> Option<&'a Native> {
        self.item().as_native()
    }

    /// A script-object view, if this is a script object.
    #[must_use]
    pub fn as_script(self) -> Option<ScriptView<'a>> {
        match self.item() {
            Item::Object(o) => match &o.body {
                Body::Script(s) => Some(ScriptView {
                    item: self,
                    class: o.class,
                    object: s,
                }),
                Body::Native(_) => None,
            },
            _ => None,
        }
    }

    /// The position, for a `position` native object.
    #[must_use]
    pub fn as_position(&self) -> Option<&'a Position> {
        self.as_native().and_then(Native::as_position)
    }

    /// The StringInfo, for a `string_info` native object.
    #[must_use]
    pub fn as_string_info(&self) -> Option<&'a StringInfo> {
        self.as_native().and_then(Native::as_string_info)
    }

    /// The references this item holds, in stored order.
    pub fn references(&self) -> impl Iterator<Item = Ref> + 'a {
        item_references(&self.state.bundle, self.item())
    }

    /// Resolves every reference this item holds.
    pub fn referenced(&self) -> impl Iterator<Item = Resolved<'a>> + 'a {
        let state = self.state;
        self.references().map(move |r| state.resolve(r))
    }
}

/// A view of a script object with typed attribute access by name hash.
#[derive(Debug, Clone, Copy)]
pub struct ScriptView<'a> {
    item: ItemView<'a>,
    class: u32,
    object: &'a ScriptObject,
}

/// One attribute of a [`ScriptView`].
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct AttributeValue {
    /// Name hash.
    pub hash: u32,
    /// Declared type.
    pub ty: ValueType,
    /// Value.
    pub value: Value,
}

impl<'a> ScriptView<'a> {
    /// The underlying item view.
    #[must_use]
    pub fn item(&self) -> ItemView<'a> {
        self.item
    }

    /// Class index.
    #[must_use]
    pub fn class(&self) -> u32 {
        self.class
    }

    /// Attributes in serialization order.
    pub fn attributes(&self) -> impl Iterator<Item = AttributeValue> + 'a {
        script_layout(&self.item.state.bundle, self.object)
            .iter()
            .zip(&self.object.values)
            .map(|(&(hash, ty), &value)| AttributeValue { hash, ty, value })
    }

    /// Number of attributes.
    #[must_use]
    pub fn len(&self) -> usize {
        self.object.values.len()
    }

    /// Whether the object has no attributes.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.object.values.is_empty()
    }

    /// The value of the attribute with name hash `hash`.
    #[must_use]
    pub fn get(&self, hash: u32) -> Option<Value> {
        self.attributes().find(|a| a.hash == hash).map(|a| a.value)
    }

    /// The value of the attribute whose name hashes (`records.md` §1.5) to `name`'s hash.
    /// Attribute names are not in the data; callers supply names they can verify.
    #[must_use]
    pub fn get_named(&self, name: &str) -> Option<Value> {
        self.get(name_hash(name.as_bytes()))
    }

    /// An integer attribute.
    #[must_use]
    pub fn int(&self, hash: u32) -> Option<u32> {
        self.get(hash).and_then(|v| v.as_int())
    }

    /// A float attribute.
    #[must_use]
    pub fn float(&self, hash: u32) -> Option<f32> {
        self.get(hash).and_then(|v| v.as_float())
    }

    /// A 64-bit integer attribute.
    #[must_use]
    pub fn int64(&self, hash: u32) -> Option<u64> {
        self.get(hash).and_then(|v| v.as_int64())
    }

    /// A double attribute.
    #[must_use]
    pub fn double(&self, hash: u32) -> Option<f64> {
        self.get(hash).and_then(|v| v.as_double())
    }

    /// A reference attribute, resolved. `None` if there is no reference attribute `hash`.
    #[must_use]
    pub fn follow(&self, hash: u32) -> Option<Resolved<'a>> {
        let r = self.get(hash)?.as_reference()?;
        Some(self.item.state.resolve(r))
    }

    /// The reference attributes, in serialization order.
    pub fn reference_attributes(&self) -> impl Iterator<Item = (u32, Ref)> + 'a {
        self.attributes()
            .filter_map(|a| a.value.as_reference().map(|r| (a.hash, r)))
    }
}
