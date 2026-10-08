// SPDX-License-Identifier: GPL-3.0-or-later
//! The UI offer protocol (`keymap.md` §7.8, decision 0014 D5, D7): the
//! synchronous sink through which the game UI sees, in order, the events the
//! mapper resolves, decides who owns each hold, and asks for band changes.
//!
//! `tessera-input` defines these types and does not depend on `tessera-ui`;
//! the client implements [`UiSink`] and copies each [`OfferedEvent`] into the
//! UI's own command type. Everything runs on the main thread.

use std::sync::Arc;
use std::time::Duration;

use crate::action::{ActionId, ContextId};
use crate::device::{Button, Modifiers};

/// Most band requests one offer may append; further ones are dropped with a
/// warning (a resource bound, not a spec rule: normal use needs a handful).
pub const MAX_BAND_REQUESTS_PER_OFFER: usize = 1024;
/// Most entries in the UI band (§3.2); a push beyond it is refused.
pub const MAX_BAND_ENTRIES: usize = 256;

/// Identity of one hold or pulse, unique for the mapper's lifetime and
/// increasing in the order holds began. A Start, its Repeats and its End
/// carry the same id.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct HoldId(pub u64);

/// Which part of a hold an action offer is.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum OfferPhase {
    /// The hold begins (its verdict decides who owns it, §7.8.3).
    Start,
    /// An OS auto-repeat of a repeatable hold (§7.8.6); the verdict is
    /// ignored by the mapper.
    Repeat,
    /// The hold ends; every offered Start gets exactly one (§7.8.4).
    End,
}

/// One offered Start, Repeat or End of a digital hold or pulse (§7.8.2).
#[derive(Debug, Clone, PartialEq)]
pub struct ActionOffer {
    /// Start, Repeat or End.
    pub phase: OfferPhase,
    /// The hold (shared by its Start, Repeats and End).
    pub hold: HoldId,
    /// The action.
    pub action: ActionId,
    /// The context whose binding made the hold.
    pub context: ContextId,
    /// The InputActions command name (§2.2); an offered hold always has one.
    pub command: Arc<str>,
    /// The command's InputActions ID when the client supplied the table
    /// ([`InputMapper::set_command_ids`](crate::InputMapper::set_command_ids)).
    pub command_id: Option<u32>,
    /// The absolute entry value: 1.0 for button actions and plain entries
    /// (§2.2).
    pub value: f32,
    /// The button whose press (or release, for a release-class pulse) made
    /// the hold.
    pub source: Button,
    /// The mapper's cursor position in physical pixels, or `None` (unknown).
    pub position: Option<(f64, f64)>,
    /// Modifiers held when offered.
    pub modifiers: Modifiers,
    /// Timestamp of the raw event (for Ends from the end queue, of the last
    /// raw event the mapper received).
    pub time: Duration,
    /// Sequence number of the call that offers it (§7.1).
    pub sequence: u64,
}

/// Everything the mapper offers to the UI (§7.8.2).
#[derive(Debug, Clone, PartialEq)]
pub enum OfferedEvent {
    /// A Start, Repeat or End of a hold or pulse with a command name.
    Action(ActionOffer),
    /// A typed character routed to the UI (§7.6).
    Text {
        /// The character.
        ch: char,
        /// Modifiers held.
        modifiers: Modifiers,
        /// Timestamp of the raw event.
        time: Duration,
        /// Sequence number of the call.
        sequence: u64,
    },
    /// The coalesced pointer move (§7.8.7), at the last recorded position.
    PointerMoved {
        /// Physical pixels, client area, origin top-left.
        x: f64,
        /// Physical pixels.
        y: f64,
        /// Timestamp of the last cursor-position event.
        time: Duration,
        /// Sequence number of the call that offers it.
        sequence: u64,
    },
    /// The cursor left the window.
    PointerLeft {
        /// Timestamp of the raw event.
        time: Duration,
        /// Sequence number of the call.
        sequence: u64,
    },
    /// The window lost focus (offered after the Ends it caused).
    FocusLost {
        /// Timestamp of the raw event.
        time: Duration,
        /// Sequence number of the call.
        sequence: u64,
    },
}

impl OfferedEvent {
    /// The action offer, if this is one.
    #[must_use]
    pub fn as_action(&self) -> Option<&ActionOffer> {
        match self {
            Self::Action(a) => Some(a),
            _ => None,
        }
    }

    /// Sequence number of the call that offered it.
    #[must_use]
    pub fn sequence(&self) -> u64 {
        match self {
            Self::Action(a) => a.sequence,
            Self::Text { sequence, .. }
            | Self::PointerMoved { sequence, .. }
            | Self::PointerLeft { sequence, .. }
            | Self::FocusLost { sequence, .. } => *sequence,
        }
    }
}

/// The UI's answer to one offer. Only the verdict of a Start is used
/// (§7.8.3).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum Verdict {
    /// The UI used it: a Start makes the hold UI-owned (never in the game
    /// snapshot).
    Consumed,
    /// The UI did not use it: a Start makes a normal game hold.
    #[default]
    NotConsumed,
}

impl Verdict {
    /// `Consumed` when `consumed` is true.
    #[must_use]
    pub fn from_bool(consumed: bool) -> Self {
        if consumed {
            Self::Consumed
        } else {
            Self::NotConsumed
        }
    }

    /// Whether it is [`Verdict::Consumed`].
    #[must_use]
    pub fn is_consumed(self) -> bool {
        self == Self::Consumed
    }
}

/// One band change asked for by the sink during an offer (§3.2, §7.8.1).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum BandRequest {
    /// Push the entry (context, owner, priority).
    Push {
        /// UI-band context.
        context: ContextId,
        /// Opaque owner (0 is reserved for code).
        owner: u64,
        /// Priority.
        priority: i32,
    },
    /// Pop every entry of (context, owner).
    Pop {
        /// UI-band context.
        context: ContextId,
        /// Owner.
        owner: u64,
    },
}

/// The mapper-owned band request list handed to the sink with each offer,
/// empty at the start of the offer (§7.8.1). The mapper applies it, in
/// order, when the offer returns.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct BandRequests {
    list: Vec<BandRequest>,
    dropped: usize,
}

impl BandRequests {
    /// Ask for a push (applied after the offer returns).
    pub fn push(&mut self, context: ContextId, owner: u64, priority: i32) {
        self.add(BandRequest::Push {
            context,
            owner,
            priority,
        });
    }

    /// Ask for a pop (applied after the offer returns).
    pub fn pop(&mut self, context: ContextId, owner: u64) {
        self.add(BandRequest::Pop { context, owner });
    }

    /// Append a request; past [`MAX_BAND_REQUESTS_PER_OFFER`] it is dropped.
    pub fn add(&mut self, request: BandRequest) {
        if self.list.len() < MAX_BAND_REQUESTS_PER_OFFER {
            self.list.push(request);
        } else {
            self.dropped = self.dropped.saturating_add(1);
        }
    }

    /// The requests so far, in order.
    #[must_use]
    pub fn as_slice(&self) -> &[BandRequest] {
        &self.list
    }

    /// Whether no request was made.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.list.is_empty()
    }

    pub(crate) fn take(&mut self) -> (Vec<BandRequest>, usize) {
        let d = std::mem::take(&mut self.dropped);
        (std::mem::take(&mut self.list), d)
    }
}

/// One entry of the UI band: (context, owner, priority), §3.2.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct BandEntry {
    /// UI-band context.
    pub context: ContextId,
    /// Opaque owner (0 = code).
    pub owner: u64,
    /// Priority (higher first).
    pub priority: i32,
}

/// The synchronous UI sink (§7.8.1). The mapper calls it for every offer and
/// uses only a Start's verdict. The sink cannot reach the mapper; it asks
/// for band changes through `requests`. It may call window operations
/// (cursor grab, hide, warp) during the call.
pub trait UiSink {
    /// Offer one event.
    fn offer(&mut self, event: &OfferedEvent, requests: &mut BandRequests) -> Verdict;
}

impl<F> UiSink for F
where
    F: FnMut(&OfferedEvent, &mut BandRequests) -> Verdict,
{
    fn offer(&mut self, event: &OfferedEvent, requests: &mut BandRequests) -> Verdict {
        self(event, requests)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn requests_are_bounded() {
        let mut r = BandRequests::default();
        for i in 0..(MAX_BAND_REQUESTS_PER_OFFER + 10) {
            r.pop(ContextId(0), i as u64);
        }
        assert_eq!(r.as_slice().len(), MAX_BAND_REQUESTS_PER_OFFER);
        let (list, dropped) = r.take();
        assert_eq!(list.len(), MAX_BAND_REQUESTS_PER_OFFER);
        assert_eq!(dropped, 10);
        assert!(r.is_empty());
        assert!(Verdict::from_bool(true).is_consumed());
        assert!(!Verdict::default().is_consumed());
    }
}
