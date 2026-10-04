// SPDX-License-Identifier: GPL-3.0-or-later
//! Back/forward navigation history.

/// Largest number of entries kept behind the current one; the oldest are dropped.
pub const MAX_HISTORY: usize = 512;

/// A browser-style history: a current entry with entries behind and ahead of it.
///
/// [`History::visit`] makes a new current entry and discards the forward entries;
/// visiting the current entry again is ignored.
#[derive(Debug, Clone)]
pub struct History<T> {
    back: Vec<T>,
    current: Option<T>,
    forward: Vec<T>,
}

impl<T> Default for History<T> {
    fn default() -> Self {
        Self {
            back: Vec::new(),
            current: None,
            forward: Vec::new(),
        }
    }
}

impl<T: Clone + PartialEq> History<T> {
    /// An empty history.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// The current entry.
    #[must_use]
    pub fn current(&self) -> Option<&T> {
        self.current.as_ref()
    }

    /// Makes `entry` current. Returns `false` (and changes nothing) if it already is.
    pub fn visit(&mut self, entry: T) -> bool {
        if self.current.as_ref() == Some(&entry) {
            return false;
        }
        if let Some(cur) = self.current.take() {
            self.back.push(cur);
            if self.back.len() > MAX_HISTORY {
                let excess = self.back.len() - MAX_HISTORY;
                self.back.drain(..excess);
            }
        }
        self.forward.clear();
        self.current = Some(entry);
        true
    }

    /// Replaces the current entry without recording a step (for example when only the
    /// view of the same DID changes).
    pub fn replace_current(&mut self, entry: T) {
        self.current = Some(entry);
    }

    /// Whether [`History::back`] would move.
    #[must_use]
    pub fn can_back(&self) -> bool {
        !self.back.is_empty()
    }

    /// Whether [`History::forward`] would move.
    #[must_use]
    pub fn can_forward(&self) -> bool {
        !self.forward.is_empty()
    }

    /// Steps back; returns the new current entry.
    pub fn back(&mut self) -> Option<&T> {
        let prev = self.back.pop()?;
        if let Some(cur) = self.current.replace(prev) {
            self.forward.push(cur);
        }
        self.current.as_ref()
    }

    /// Steps forward; returns the new current entry.
    pub fn forward(&mut self) -> Option<&T> {
        let next = self.forward.pop()?;
        if let Some(cur) = self.current.replace(next) {
            self.back.push(cur);
        }
        self.current.as_ref()
    }

    /// Number of entries behind the current one.
    #[must_use]
    pub fn back_len(&self) -> usize {
        self.back.len()
    }

    /// Number of entries ahead of the current one.
    #[must_use]
    pub fn forward_len(&self) -> usize {
        self.forward.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn back_and_forward() {
        let mut h = History::new();
        assert!(!h.can_back());
        assert!(h.visit(1));
        assert!(h.visit(2));
        assert!(!h.visit(2));
        assert!(h.visit(3));
        assert_eq!(h.back(), Some(&2));
        assert_eq!(h.back(), Some(&1));
        assert_eq!(h.back(), None);
        assert_eq!(h.current(), Some(&1));
        assert_eq!(h.forward(), Some(&2));
        // A new visit drops the forward entries.
        assert!(h.visit(9));
        assert!(!h.can_forward());
        assert_eq!(h.back(), Some(&2));
        h.replace_current(7);
        assert_eq!(h.current(), Some(&7));
        assert_eq!(h.forward(), Some(&9));
    }

    #[test]
    fn bounded() {
        let mut h = History::new();
        for i in 0..(MAX_HISTORY + 100) {
            h.visit(i);
        }
        assert_eq!(h.back_len(), MAX_HISTORY);
        assert_eq!(h.forward_len(), 0);
    }
}
