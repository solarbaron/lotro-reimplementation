// SPDX-License-Identifier: GPL-3.0-or-later
//! The official-entry presentation (`apps/client/src/official/login.rs`, included by path;
//! `docs/specs/official-entry.md` §8, §10.3): arbitrary sequences of Account events,
//! `Connected` and `Disconnected`, from a backend through a `tessera_net::Session` into the
//! login presenter, with the user's actions and the clock interleaved. Nothing may panic, the
//! state stays bounded (at most 256 rows, a selection inside the list), and after the login
//! ended nothing changes it.

#![no_main]

use std::collections::VecDeque;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use libfuzzer_sys::fuzz_target;
use tessera_net::account::{
    AccountEvent, LoginRefusal, LoginStage, WorldEntry, WorldId, WorldLoad, WorldRefusal,
    WorldStatus,
};
use tessera_net::{Backend, CharacterSummary, Command, DisconnectReason, Event, NetError, Session};

#[allow(dead_code)]
#[path = "../../apps/client/src/official/login.rs"]
mod login;

#[derive(Debug, Default)]
struct Fed {
    queue: Arc<Mutex<VecDeque<Event>>>,
}

impl Backend for Fed {
    fn name(&self) -> &str {
        "fuzz"
    }

    fn submit(&mut self, _now: Instant, _command: Command) -> Result<(), NetError> {
        Ok(())
    }

    fn poll(&mut self, _now: Instant, out: &mut Vec<Event>) {
        if let Ok(mut q) = self.queue.lock() {
            out.extend(q.drain(..));
        }
    }
}

/// A byte reader that yields zeros at the end.
struct Bytes<'a>(&'a [u8]);

impl Bytes<'_> {
    fn u8(&mut self) -> u8 {
        match self.0.split_first() {
            Some((&b, rest)) => {
                self.0 = rest;
                b
            }
            None => 0,
        }
    }

    fn u32(&mut self) -> u32 {
        u32::from_le_bytes([self.u8(), self.u8(), self.u8(), self.u8()])
    }

    fn text(&mut self, max: usize) -> String {
        let n = usize::from(self.u8()) % (max + 2);
        let take = n.min(self.0.len());
        let (t, rest) = self.0.split_at(take);
        self.0 = rest;
        String::from_utf8_lossy(t).into_owned()
    }

    fn opt_u32(&mut self) -> Option<u32> {
        (self.u8() & 1 == 1).then(|| self.u32())
    }

    fn opt_text(&mut self, max: usize) -> Option<String> {
        (self.u8() & 1 == 1).then(|| self.text(max))
    }
}

fn event(b: &mut Bytes<'_>) -> Event {
    match b.u8() % 9 {
        0 => Event::Account(AccountEvent::LoginProgress {
            stage: match b.u8() % 3 {
                0 => LoginStage::Authenticating,
                1 => LoginStage::ListingWorlds,
                _ => LoginStage::JoiningWorld,
            },
        }),
        1 => {
            let n = usize::from(b.u8()) * 2 % 300;
            let worlds = (0..n)
                .map(|_| WorldEntry {
                    id: WorldId(u32::from(b.u8())),
                    name: b.text(70),
                    status: match b.u8() % 6 {
                        0 => WorldStatus::Open,
                        1 => WorldStatus::Busy,
                        2 => WorldStatus::Full,
                        3 => WorldStatus::Closed,
                        4 => WorldStatus::Restricted,
                        _ => WorldStatus::Unknown,
                    },
                    load: match b.u8() % 4 {
                        0 => Some(WorldLoad::Low),
                        1 => Some(WorldLoad::Medium),
                        2 => Some(WorldLoad::High),
                        _ => None,
                    },
                    note: b.opt_text(140),
                })
                .collect();
            Event::Account(AccountEvent::WorldList { worlds })
        }
        2 => Event::Account(AccountEvent::LoginQueueChanged {
            position: b.opt_u32(),
            length: b.opt_u32(),
            wait_seconds: b.opt_u32(),
        }),
        3 => Event::Account(AccountEvent::WorldRefused {
            world: WorldId(u32::from(b.u8())),
            reason: match b.u8() % 4 {
                0 => WorldRefusal::Full,
                1 => WorldRefusal::Closed,
                2 => WorldRefusal::NotAllowed,
                _ => WorldRefusal::Other,
            },
            detail: b.opt_text(270),
        }),
        4 => Event::Account(AccountEvent::LoginRefused {
            reason: match b.u8() % 9 {
                0 => LoginRefusal::BadCredentials,
                1 => LoginRefusal::AccountNotAllowed,
                2 => LoginRefusal::UnsupportedStep,
                3 => LoginRefusal::ClientNotAccepted,
                4 => LoginRefusal::RateLimited,
                5 => LoginRefusal::ServiceUnavailable,
                6 => LoginRefusal::NotAvailable,
                7 => LoginRefusal::InsecureService,
                _ => LoginRefusal::Other,
            },
            retry_after_seconds: b.opt_u32(),
            detail: b.opt_text(270),
        }),
        5 => Event::Connected {
            characters: (0..b.u8() % 4)
                .map(|_| CharacterSummary { name: b.text(70) })
                .collect(),
        },
        6 => Event::Disconnected {
            reason: match b.u8() % 4 {
                0 => DisconnectReason::ClientRequested,
                1 => DisconnectReason::TimedOut,
                2 => DisconnectReason::ServerClosed(b.text(100)),
                _ => DisconnectReason::TransportFailed(b.text(100)),
            },
        },
        7 => Event::ReturnedToCharacterSelect {
            characters: (0..b.u8() % 4)
                .map(|_| CharacterSummary { name: b.text(70) })
                .collect(),
        },
        _ => Event::StaticsCleared,
    }
}

fuzz_target!(|data: &[u8]| {
    let mut b = Bytes(data);
    let queue = Arc::new(Mutex::new(VecDeque::new()));
    let mut session = Session::new(Fed {
        queue: Arc::clone(&queue),
    });
    let t0 = Instant::now();
    let mut now = t0;
    let _ = session.send(now, Command::Connect);
    let last = (b.u8() & 1 == 1).then(|| b.text(80));
    let mut p = login::LoginPresenter::new(now, last);
    let mut events = Vec::new();
    for _ in 0..64 {
        if b.0.is_empty() {
            break;
        }
        match b.u8() % 10 {
            0..=3 => {
                if let Ok(mut q) = queue.lock() {
                    q.push_back(event(&mut b));
                }
            }
            4 => now += Duration::from_millis(u64::from(b.u32() % 120_000)),
            5 => p.move_selection(i32::from(b.u8() as i8)),
            6 => p.select(usize::from(b.u8())),
            7 => p.join(now),
            8 => match b.u8() % 3 {
                0 => p.cancel(),
                1 => p.leave_queue(now),
                _ => p.dismiss_message(),
            },
            _ => {}
        }
        let ended_before = p.end().cloned();
        events.clear();
        session.poll_into(now, &mut events);
        for e in &events {
            p.on_event(e, now);
        }
        p.tick(now);
        for c in p.take_commands() {
            let _ = session.send(now, c);
        }
        let _ = p.take_remember();
        if let Some(before) = ended_before {
            assert_eq!(p.end(), Some(&before), "the end never changes");
        }
        assert!(p.rows().len() <= 256);
        if let Some(i) = p.selected() {
            assert!(i < p.rows().len() && p.rows()[i].choosable);
        }
        for r in p.rows() {
            assert!(!r.name.is_empty() && r.name.len() <= 64);
            assert!(r.note.len() <= 128);
        }
    }
});
