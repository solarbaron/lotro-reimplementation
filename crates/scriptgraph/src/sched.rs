// SPDX-License-Identifier: GPL-3.0-or-later
//! The one client-wide scheduler of `script-runtime.md` §20.1: a clock and a queue of
//! events ordered by due time, ties in insertion order.

use std::cmp::Ordering;
use std::collections::BTreeMap;

/// A due time with a total order (times are always finite).
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct Due(pub f64);

impl Eq for Due {}

impl PartialOrd for Due {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for Due {
    fn cmp(&self, other: &Self) -> Ordering {
        self.0.total_cmp(&other.0)
    }
}

/// The key of a scheduled event: due time and insertion number.
pub(crate) type EventKey = (Due, u64);

/// The scheduler.
#[derive(Debug, Clone)]
pub(crate) struct Scheduler<E> {
    /// Current time, seconds (never goes backwards).
    pub now: f64,
    /// Target time of the current frame (the time of the last advance outside one).
    pub frame_time: f64,
    seq: u64,
    queue: BTreeMap<EventKey, E>,
}

impl<E> Default for Scheduler<E> {
    fn default() -> Self {
        Self {
            now: 0.0,
            frame_time: 0.0,
            seq: 0,
            queue: BTreeMap::new(),
        }
    }
}

impl<E> Scheduler<E> {
    /// Schedules `event` at `due` (a non-finite time counts as now) and returns its key.
    pub fn schedule(&mut self, due: f64, event: E) -> EventKey {
        let due = if due.is_finite() { due } else { self.now };
        let key = (Due(due), self.seq);
        self.seq = self.seq.wrapping_add(1);
        self.queue.insert(key, event);
        key
    }

    /// Removes a scheduled event (no effect when it already fired).
    pub fn cancel(&mut self, key: EventKey) {
        self.queue.remove(&key);
    }

    /// The earliest event due at or before `t`, removed from the queue.
    pub fn pop_due(&mut self, t: f64) -> Option<(f64, E)> {
        let (&(Due(due), _), _) = self.queue.first_key_value()?;
        if due > t {
            return None;
        }
        let ((Due(due), _), e) = self.queue.pop_first()?;
        Some((due, e))
    }

    /// Number of pending events.
    pub fn len(&self) -> usize {
        self.queue.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn time_order_then_insertion_order() {
        let mut s = Scheduler::default();
        s.schedule(2.0, 'a');
        s.schedule(1.0, 'b');
        let k = s.schedule(1.0, 'c');
        s.schedule(1.0, 'd');
        s.cancel(k);
        assert_eq!(s.pop_due(1.5), Some((1.0, 'b')));
        assert_eq!(s.pop_due(1.5), Some((1.0, 'd')));
        assert_eq!(s.pop_due(1.5), None);
        assert_eq!(s.pop_due(2.0), Some((2.0, 'a')));
        assert_eq!(s.len(), 0);
    }
}
