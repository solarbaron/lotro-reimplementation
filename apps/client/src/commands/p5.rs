// SPDX-License-Identifier: GPL-3.0-or-later
//! The P5 client built-ins of the dispatcher (decision 0020 items 1–2, "Client options";
//! decision 0024 P5), registered under Tessera's own names before any plugin loads:
//!
//! | Name | Line | Spec |
//! |---|---|---|
//! | `ride` | `/ride`, `/ride SPEC`, `/ride off`, `/ride list [PAGE]` | `walker.md` §16.3 |
//! | `findplace` | `/findplace TEXT` (without TEXT: removes the lookup marker) | `map-windows.md` §7.3 |
//! | `leaveinstance` | `/leaveinstance` | `travel-windows.md` §6.6 rule 2 |
//!
//! The handlers parse their arguments (a `/ride` argument that does not parse posts one
//! `Error` line with the reason) and queue a [`P5Request`]; the frame loop hands the
//! requests to the walker, the map window and the world session
//! ([`crate::plugins::Plugins::take_p5_requests`]), which post their own lines.

use std::collections::VecDeque;
use std::sync::{Arc, Mutex, PoisonError};

use crate::walker::mount::spec::{RideCommand, parse_ride};

use super::LocalLine;
use super::registry::Help;

/// Most requests queued between two frames (Tessera bound; further ones are dropped with a
/// line).
pub const MAX_REQUESTS: usize = 32;

/// Longest `findplace` text kept, in characters (`tessera-gamedata` compares at most 256).
pub const MAX_LOOKUP_CHARS: usize = 256;

/// A P5 built-in command word.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum P5Word {
    /// `ride`.
    Ride,
    /// `findplace`.
    FindPlace,
    /// `leaveinstance`.
    LeaveInstance,
}

impl P5Word {
    /// Every word.
    pub const ALL: [P5Word; 3] = [Self::Ride, Self::FindPlace, Self::LeaveInstance];

    /// The Tessera name.
    #[must_use]
    pub fn name(self) -> &'static str {
        match self {
            Self::Ride => "ride",
            Self::FindPlace => "findplace",
            Self::LeaveInstance => "leaveinstance",
        }
    }

    /// The help texts (Tessera's wording; `walker.md` §16.3 for `ride`).
    #[must_use]
    pub fn help(self) -> Help {
        let (short, long) = match self {
            Self::Ride => (
                "mount, dismount or list mounts",
                "/ride [SPEC | off | list [PAGE]]: toggles riding, mounts SPEC, dismounts, or \
                 lists this body's mounts.",
            ),
            Self::FindPlace => (
                "find a place on the map",
                "/findplace [TEXT]: looks up map coordinates, a /loc line, a place key or the \
                 name of a map, area, territory, region or dungeon and shows the first result \
                 on the map; without TEXT, removes the lookup marker.",
            ),
            Self::LeaveInstance => (
                "leave the instance",
                "/leaveinstance: asks the session to take you out of the instance.",
            ),
        };
        Help::Text {
            short: short.to_owned(),
            long: long.to_owned(),
        }
    }
}

/// What a P5 built-in asks the frame loop to do.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum P5Request {
    /// `/ride …` for the walker.
    Ride(RideCommand),
    /// `/findplace TEXT` for the map window (empty: remove the lookup marker).
    FindPlace(String),
    /// `/leaveinstance`: send `LeaveInstance`.
    LeaveInstance,
}

/// The queue the handlers fill and the frame loop drains (shared, so a dispatcher that
/// runs from a plugin call or a start-up line queues the same way).
#[derive(Debug, Clone, Default)]
pub struct P5Queue(Arc<Mutex<VecDeque<P5Request>>>);

impl P5Queue {
    /// Queues a request; `false` when the queue is full.
    pub fn push(&self, r: P5Request) -> bool {
        let mut q = self.0.lock().unwrap_or_else(PoisonError::into_inner);
        if q.len() >= MAX_REQUESTS {
            return false;
        }
        q.push_back(r);
        true
    }

    /// Takes every request, in order.
    pub fn take(&self) -> Vec<P5Request> {
        let mut q = self.0.lock().unwrap_or_else(PoisonError::into_inner);
        q.drain(..).collect()
    }
}

/// Runs a P5 built-in with its argument text: queues the request, or posts an `Error` line.
pub fn run(word: P5Word, arguments: &str, queue: &P5Queue, out: &mut Vec<LocalLine>) {
    let request = match word {
        P5Word::Ride => match parse_ride(arguments) {
            Ok(c) => P5Request::Ride(c),
            Err(e) => {
                out.push(LocalLine::error(format!("/ride: {e}.")));
                return;
            }
        },
        P5Word::FindPlace => {
            P5Request::FindPlace(arguments.trim().chars().take(MAX_LOOKUP_CHARS).collect())
        }
        P5Word::LeaveInstance => P5Request::LeaveInstance,
    };
    if !queue.push(request) {
        out.push(LocalLine::error("Too many requests at once; try again."));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn requests_queue_and_errors_post_lines() {
        let q = P5Queue::default();
        let mut out = Vec::new();
        run(P5Word::Ride, "", &q, &mut out);
        run(P5Word::Ride, "steed:medium@lock=canter", &q, &mut out);
        run(P5Word::Ride, "list 2", &q, &mut out);
        run(P5Word::Ride, "nonsense!", &q, &mut out);
        run(P5Word::FindPlace, "  Bree ", &q, &mut out);
        run(P5Word::LeaveInstance, "", &q, &mut out);
        assert_eq!(out.len(), 1, "{out:?}");
        let got = q.take();
        assert_eq!(got.len(), 5);
        assert_eq!(got[0], P5Request::Ride(RideCommand::Toggle));
        assert_eq!(got[2], P5Request::Ride(RideCommand::List(2)));
        assert_eq!(got[3], P5Request::FindPlace("Bree".into()));
        assert_eq!(got[4], P5Request::LeaveInstance);
        assert!(q.take().is_empty());
    }

    #[test]
    fn the_queue_is_bounded() {
        let q = P5Queue::default();
        let mut out = Vec::new();
        for _ in 0..=MAX_REQUESTS {
            run(P5Word::LeaveInstance, "", &q, &mut out);
        }
        assert_eq!(q.take().len(), MAX_REQUESTS);
        assert_eq!(out.len(), 1);
    }
}
