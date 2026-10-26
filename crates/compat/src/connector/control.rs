// SPDX-License-Identifier: GPL-3.0-or-later
//! What the live connector and its thread share besides the two channels
//! (`ARCHITECTURE.md` §19.4): the cancel flag, the credentials until their one use, the
//! newest-move slot and the count of events waiting for `poll`.

use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};

use tessera_net::Command;

use crate::credentials::Credentials;

/// A message on the command channel: a command, or a wake-up after the newest-move slot
/// was filled (at most one waits at a time).
#[derive(Debug)]
pub enum Msg {
    /// A command for the session.
    Command(Command),
    /// The newest-move slot holds a move.
    Wake,
}

/// The shared state (module docs).
#[derive(Debug, Default)]
pub struct Control {
    cancelled: AtomicBool,
    credentials: Mutex<Option<Credentials>>,
    newest_move: Mutex<Option<Command>>,
    wake_queued: AtomicBool,
    pending: AtomicUsize,
}

impl Control {
    /// A control holding `credentials` until their one use.
    #[must_use]
    pub fn new(credentials: Credentials) -> Self {
        Self {
            credentials: Mutex::new(Some(credentials)),
            ..Self::default()
        }
    }

    /// The backend let go (`Disconnect`, drop): the thread stops at its next check, and
    /// the credentials, if not yet used, are dropped (and wiped) here and now.
    pub fn cancel(&self) {
        self.cancelled.store(true, Ordering::SeqCst);
        drop(self.take_credentials());
        if let Ok(mut m) = self.newest_move.lock() {
            *m = None;
        }
    }

    /// Whether the backend let go.
    #[must_use]
    pub fn cancelled(&self) -> bool {
        self.cancelled.load(Ordering::SeqCst)
    }

    /// The credentials, once (`None` after a cancel or the first take).
    pub fn take_credentials(&self) -> Option<Credentials> {
        match self.credentials.lock() {
            Ok(mut c) => c.take(),
            Err(poisoned) => poisoned.into_inner().take(),
        }
    }

    /// Puts `command` (a move) in the newest-move slot, replacing an older one. Returns
    /// whether the thread needs a wake-up message (none is queued yet).
    pub fn put_move(&self, command: Command) -> bool {
        if self.cancelled() {
            return false;
        }
        if let Ok(mut m) = self.newest_move.lock() {
            *m = Some(command);
        }
        !self.wake_queued.swap(true, Ordering::SeqCst)
    }

    /// A wake-up message could not be queued (the queue is full: the thread is awake
    /// anyway and finds the move on its next pass).
    pub fn wake_not_queued(&self) {
        self.wake_queued.store(false, Ordering::SeqCst);
    }

    /// The newest move, if any (the thread).
    pub fn take_move(&self) -> Option<Command> {
        self.wake_queued.store(false, Ordering::SeqCst);
        self.newest_move.lock().ok().and_then(|mut m| m.take())
    }

    /// Events sent to the backend and not yet taken by `poll`.
    #[must_use]
    pub fn pending(&self) -> usize {
        self.pending.load(Ordering::SeqCst)
    }

    /// `n` more events wait.
    pub fn add_pending(&self, n: usize) {
        self.pending.fetch_add(n, Ordering::SeqCst);
    }

    /// `n` events were taken.
    pub fn take_pending(&self, n: usize) {
        let mut current = self.pending.load(Ordering::SeqCst);
        loop {
            let next = current.saturating_sub(n);
            match self.pending.compare_exchange_weak(
                current,
                next,
                Ordering::SeqCst,
                Ordering::SeqCst,
            ) {
                Ok(_) => return,
                Err(actual) => current = actual,
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_cancel_drops_the_credentials_and_the_move() {
        let c = Control::new(Credentials::new(b"a", b"b").expect("creds"));
        assert!(c.put_move(Command::Disconnect));
        assert!(!c.put_move(Command::Disconnect), "one wake at a time");
        c.cancel();
        assert!(c.cancelled());
        assert!(c.take_credentials().is_none());
        assert!(c.take_move().is_none());
        assert!(!c.put_move(Command::Disconnect));
    }

    #[test]
    fn pending_counts_never_underflow() {
        let c = Control::default();
        c.add_pending(3);
        c.take_pending(5);
        assert_eq!(c.pending(), 0);
    }
}
