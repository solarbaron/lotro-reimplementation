// SPDX-License-Identifier: GPL-3.0-or-later
//! The pin registry (`item-tooltips.md` §10): pinning is a toggle keyed by (instance,
//! record), with at most 20 pinned tooltips at once.

use super::input::ExamKey;

/// Most pinned tooltips at once (`item-tooltips.md` §10, IT15).
pub const MAX_PINNED: usize = 20;

/// What a pin request did.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PinOutcome {
    /// A new pinned tooltip was opened.
    Pinned,
    /// The item was pinned already: its pinned tooltip was closed.
    Unpinned,
    /// [`MAX_PINNED`] tooltips are pinned: the request is refused (the client shows a UI
    /// message).
    Refused,
}

/// The pinned items and what each one's window is, in pin order.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct PinRegistry<T> {
    entries: Vec<(ExamKey, T)>,
}

impl<T> PinRegistry<T> {
    /// An empty registry.
    #[must_use]
    pub fn new() -> Self {
        Self {
            entries: Vec::new(),
        }
    }

    /// Whether `key` is pinned.
    #[must_use]
    pub fn contains(&self, key: ExamKey) -> bool {
        self.entries.iter().any(|(k, _)| *k == key)
    }

    /// Number of pinned tooltips.
    #[must_use]
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Whether nothing is pinned.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// The pinned entries in pin order.
    pub fn iter(&self) -> impl Iterator<Item = &(ExamKey, T)> {
        self.entries.iter()
    }

    /// The toggle decision for `key`: [`PinOutcome::Unpinned`] when it is pinned,
    /// [`PinOutcome::Refused`] when the registry is full, else [`PinOutcome::Pinned`].
    #[must_use]
    pub fn decide(&self, key: ExamKey) -> PinOutcome {
        if self.contains(key) {
            PinOutcome::Unpinned
        } else if self.entries.len() >= MAX_PINNED {
            PinOutcome::Refused
        } else {
            PinOutcome::Pinned
        }
    }

    /// Records a pinned tooltip. Returns `false` (and drops `value`) when `key` is pinned
    /// already or the registry is full.
    pub fn insert(&mut self, key: ExamKey, value: T) -> bool {
        if self.decide(key) != PinOutcome::Pinned {
            return false;
        }
        self.entries.push((key, value));
        true
    }

    /// Removes `key`, returning its value.
    pub fn remove(&mut self, key: ExamKey) -> Option<T> {
        let i = self.entries.iter().position(|(k, _)| *k == key)?;
        Some(self.entries.remove(i).1)
    }

    /// Removes every entry for which `drop` returns true, returning their values.
    pub fn remove_where(&mut self, mut drop: impl FnMut(&ExamKey, &T) -> bool) -> Vec<T> {
        let mut out = Vec::new();
        let mut i = 0;
        while i < self.entries.len() {
            if drop(&self.entries[i].0, &self.entries[i].1) {
                out.push(self.entries.remove(i).1);
            } else {
                i += 1;
            }
        }
        out
    }
}
