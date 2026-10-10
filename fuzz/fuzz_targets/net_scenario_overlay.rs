// SPDX-License-Identifier: GPL-3.0-or-later
//! The overlay backend of `tessera-net` (`docs/specs/scenario-feed.md` §12.5): a scenario
//! that passes the overlay check, played over a hostile stand-in for the test-server
//! backend. The input is split at the first NUL byte: before it the scenario, after it the
//! bytes that drive the stand-in (what it says about `APPEARANCE_CHANGE`, the player's
//! appearances with arbitrary equipment, slots and looks, answers, refusals, closes, and the
//! poll times). Asserts: no panic; at most one `AppearanceSet` in flight and never two
//! within 250 ms; every `AppearanceSet` carries at most 32 items.

#![no_main]

use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use libfuzzer_sys::fuzz_target;
use tessera_net::domain::ItemId;
use tessera_net::scenario::{
    Candidate, CandidateOutcome, CandidateStatus, Family, LabelContext, OverlayBackend,
    OverlayServer, Scenario, SelectorQuery, SelectorResolver,
};
use tessera_net::{
    Appearance, Backend, Body, Command, DataId, DisconnectReason, EntityId, Equipped, Event,
    InventoryCommand, Look, LookEntry, NetError, PropertyValue, Session, Transform, Vec3,
};

/// `did` selectors return their DIDs.
struct Dids;

impl SelectorResolver for Dids {
    fn has_install(&self) -> bool {
        true
    }

    fn candidates(
        &mut self,
        query: &SelectorQuery<'_>,
        limit: usize,
        out: &mut Vec<Candidate>,
    ) -> CandidateOutcome {
        if query.family != Family::Did {
            return CandidateOutcome::new(CandidateStatus::Unsupported);
        }
        out.extend(query.dids.iter().take(limit).map(|&did| Candidate {
            did,
            row: 0,
            location: None,
            anchor: None,
        }));
        CandidateOutcome::new(CandidateStatus::Ready)
    }

    fn enum_value(&mut self, _: LabelContext, _: &str) -> Option<u32> {
        Some(17)
    }

    fn display_name(&mut self, _: DataId, _: Option<u32>) -> Option<String> {
        None
    }
}

#[derive(Debug, Default)]
struct Sent {
    times: Vec<Instant>,
    in_flight: bool,
    violations: Vec<String>,
}

/// A hostile stand-in for the test-server backend, driven by bytes.
#[derive(Debug)]
struct Stand {
    bytes: Vec<u8>,
    pos: usize,
    shared: bool,
    entered: bool,
    queue: Vec<Event>,
    sent: Arc<Mutex<Sent>>,
}

impl Stand {
    fn u8(&mut self) -> u8 {
        let v = self.bytes.get(self.pos).copied().unwrap_or(0);
        self.pos = self.pos.saturating_add(1);
        v
    }

    fn appearance(&mut self) -> Appearance {
        let mut a = Appearance::new(Body::Character {
            species: u32::from(self.u8()),
            sex: 1,
        });
        for _ in 0..usize::from(self.u8() % 40) {
            let slot = self.u8();
            a.equipment.push(Equipped {
                slot: (slot & 1 == 1).then_some(slot >> 1),
                ..Equipped::new(DataId(0x7000_0000 | u32::from(self.u8())))
            });
        }
        if self.u8() & 1 == 1 {
            let n = usize::from(self.u8() % 60);
            a.look = Some(Box::new(Look {
                entries: (0..n)
                    .map(|i| LookEntry {
                        property: u32::try_from(i).unwrap_or(0) + u32::from(self.u8() & 1),
                        value: PropertyValue::Int(i32::from(self.u8())),
                    })
                    .collect(),
            }));
        }
        a
    }
}

impl Backend for Stand {
    fn name(&self) -> &str {
        "stand-in"
    }

    fn submit(&mut self, _now: Instant, command: Command) -> Result<(), NetError> {
        match command {
            Command::Connect => self.queue.push(Event::Connected {
                characters: vec![tessera_net::CharacterSummary { name: "A".into() }],
            }),
            Command::EnterWorld { character } => {
                self.entered = true;
                self.queue.push(Event::WorldEntered {
                    player: EntityId(1),
                    name: character,
                    transform: Transform::new(Vec3::ZERO, 0.0),
                });
            }
            _ => {}
        }
        Ok(())
    }

    fn poll(&mut self, _now: Instant, out: &mut Vec<Event>) {
        out.append(&mut self.queue);
        if !self.entered {
            return;
        }
        match self.u8() % 8 {
            0 | 1 => {
                let a = self.appearance();
                if let Ok(mut s) = self.sent.lock() {
                    s.in_flight = false;
                }
                out.push(Event::AppearanceChanged {
                    id: EntityId(1),
                    appearance: Box::new(a),
                });
            }
            2 => out.push(Event::AppearanceChanged {
                id: EntityId(2),
                appearance: Box::new(Appearance::new(Body::Object(DataId(5)))),
            }),
            3 if self.u8() == 0xFF => out.push(Event::Disconnected {
                reason: DisconnectReason::TimedOut,
            }),
            _ => {}
        }
    }
}

impl OverlayServer for Stand {
    fn appearance_change(&self) -> bool {
        self.shared
    }

    fn send_appearance(&mut self, appearance: &Appearance) -> Result<(), NetError> {
        if self.u8() & 3 == 3 {
            // Not sent: the overlay must not count it as in flight.
            return Err(NetError::SendQueueFull);
        }
        let now = NOW.with(std::cell::Cell::get);
        if let Ok(mut s) = self.sent.lock() {
            if let (Some(last), Some(now)) = (s.times.last().copied(), now) {
                let gap = now.saturating_duration_since(last);
                if gap < Duration::from_millis(250) {
                    s.violations.push("closer than 250 ms".into());
                }
                // Unanswered and not yet timed out (10 s): a second one in flight.
                if s.in_flight && gap < Duration::from_secs(10) {
                    s.violations.push("two in flight".into());
                }
            }
            if appearance.equipment.len() > 32 {
                s.violations.push("more than 32 items".into());
            }
            if let Some(now) = now {
                s.times.push(now);
            }
            s.in_flight = true;
        }
        Ok(())
    }
}

thread_local! {
    static NOW: std::cell::Cell<Option<Instant>> = const { std::cell::Cell::new(None) };
}

fuzz_target!(|data: &[u8]| {
    let (text, rest) = match data.iter().position(|&b| b == 0) {
        Some(i) => (&data[..i], &data[i + 1..]),
        None => (data, &[][..]),
    };
    let Ok(scenario) = Scenario::parse(text) else {
        return;
    };
    if scenario.check_overlay().is_err() {
        return;
    }
    let Ok(resolved) = scenario.resolve(&mut Dids) else {
        return;
    };
    let sent = Arc::new(Mutex::new(Sent::default()));
    let mut stand = Stand {
        bytes: rest.to_vec(),
        pos: 0,
        shared: false,
        entered: false,
        queue: Vec::new(),
        sent: Arc::clone(&sent),
    };
    stand.shared = stand.u8() & 1 == 1;
    let mut session = Session::new(OverlayBackend::new(stand, Arc::new(resolved)));
    let mut now = Instant::now();
    NOW.with(|c| c.set(Some(now)));
    let mut events = Vec::new();
    let _ = session.send(now, Command::Connect);
    session.poll_into(now, &mut events);
    let _ = session.send(
        now,
        Command::EnterWorld {
            character: "A".into(),
        },
    );
    for frame in 0..600u32 {
        events.clear();
        session.poll_into(now, &mut events);
        if frame % 5 == 2 {
            let _ = session.send(
                now,
                Command::Inventory(InventoryCommand::EquipItem {
                    item: ItemId(u64::from(frame % 3) + 1),
                    slot: Some(17),
                }),
            );
        }
        now += Duration::from_millis(u64::from(frame % 7) * 20 + 10);
        NOW.with(|c| c.set(Some(now)));
    }
    let s = sent.lock().unwrap();
    assert!(s.violations.is_empty(), "{:?}", s.violations);
});
