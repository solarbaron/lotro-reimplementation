// SPDX-License-Identifier: GPL-3.0-or-later
//! The `/logout` client built-in of the dispatcher (decision 0020 items 1–2, "Client
//! options"), registered under Tessera's own name before any plugin loads: it asks to leave
//! the world for character selection, keeping the session (`Command::LeaveWorld`,
//! `official-messages-play.md` §7.2 item 1).
//!
//! Only the player's own typed line runs it (device input): a `--command` start-up line is
//! refused with a line and never queued (`official-entry.md` §4.2: no synthetic input in
//! official play; decision 0034: automation stays offline). The handler queues the request;
//! the frame loop takes it ([`LogoutQueue::take`], `crate::runtime::Runtime::tick`) and calls
//! `crate::net::World::leave_world`, which posts the session's refusal as a system line.
//! Quitting the client in an official world (closing the window, the character screens'
//! quit) ends the session with `Disconnect` as before (§7.2 item 4), not with `/logout`.

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use super::registry::Help;
use super::{LocalLine, Origin};

/// The line of a `/logout` start-up line (Tessera's wording).
pub const STARTUP_REFUSED: &str = "Start-up line not run: /logout needs the player's own input.";

/// The line of `/logout` with arguments (Tessera's wording).
pub const USAGE: &str = "Usage: /logout";

/// The `/logout` request waiting for the frame loop (one at a time: a second line before
/// the frame takes the first is the same request).
#[derive(Debug, Clone, Default)]
pub struct LogoutQueue(Arc<AtomicBool>);

impl LogoutQueue {
    /// Queues the request.
    pub fn push(&self) {
        self.0.store(true, Ordering::SeqCst);
    }

    /// Takes the request, if one is queued.
    #[must_use]
    pub fn take(&self) -> bool {
        self.0.swap(false, Ordering::SeqCst)
    }
}

/// The help text.
#[must_use]
pub fn help() -> Help {
    Help::Text {
        short: "leave the world for character selection".to_owned(),
        long: "/logout: leaves the world and goes back to character selection, keeping the \
               connection."
            .to_owned(),
    }
}

/// Runs `/logout` (module docs).
pub fn run(arguments: &str, origin: Origin, queue: &LogoutQueue, out: &mut Vec<LocalLine>) {
    if origin == Origin::Startup {
        tracing::warn!("start-up line /logout refused (device input only)");
        out.push(LocalLine::error(STARTUP_REFUSED));
        return;
    }
    if !arguments.trim().is_empty() {
        out.push(LocalLine::error(USAGE));
        return;
    }
    queue.push();
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_a_typed_line_queues_the_request() {
        let q = LogoutQueue::default();
        let mut out = Vec::new();
        run("", Origin::Startup, &q, &mut out);
        assert!(!q.take(), "a start-up line never leaves the world");
        assert_eq!(out, [LocalLine::error(STARTUP_REFUSED)]);
        out.clear();
        run("now", Origin::Chat, &q, &mut out);
        assert!(!q.take());
        assert_eq!(out, [LocalLine::error(USAGE)]);
        out.clear();
        run("", Origin::Chat, &q, &mut out);
        assert!(out.is_empty());
        assert!(q.take());
        assert!(!q.take(), "taken once");
    }
}
