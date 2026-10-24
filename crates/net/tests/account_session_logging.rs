// SPDX-License-Identifier: GPL-3.0-or-later
//! The logging rule of the Account group (`docs/specs/official-entry.md` §8.2): events the
//! session drops are logged without their fields, and at most once per kind in 10 s.
//!
//! Its own test binary, with this one test: `tracing` caches whether a log call site has any
//! subscriber when the call site is first reached. In a binary whose other tests reach the
//! same call site on other threads without a subscriber, that first registration can race
//! the scoped capture subscriber below and leave the call site switched off for it (the test
//! then saw no log lines in about one run in three).

use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use tessera_net::account::{
    AccountEvent, WorldEntry, WorldId, WorldLoad, WorldRefusal, WorldStatus,
};
use tessera_net::{Backend, Command, Event, NetError, Session};

/// A backend that emits whatever the test queues.
#[derive(Debug, Default, Clone)]
struct Scripted {
    queue: Arc<Mutex<Vec<Event>>>,
}

impl Scripted {
    fn push(&self, e: Event) {
        self.queue.lock().expect("queue").push(e);
    }
}

impl Backend for Scripted {
    fn name(&self) -> &str {
        "scripted"
    }

    fn submit(&mut self, _now: Instant, _command: Command) -> Result<(), NetError> {
        Ok(())
    }

    fn poll(&mut self, _now: Instant, out: &mut Vec<Event>) {
        out.append(&mut self.queue.lock().expect("queue"));
    }
}

fn world(id: u32, name: &str) -> WorldEntry {
    WorldEntry {
        id: WorldId(id),
        name: name.to_owned(),
        status: WorldStatus::Open,
        load: Some(WorldLoad::Low),
        note: None,
    }
}

/// A `tracing` subscriber that keeps every event's fields as text.
#[derive(Default)]
struct Capture(Mutex<String>);

struct Fields<'a>(&'a mut String);

impl tracing::field::Visit for Fields<'_> {
    fn record_debug(&mut self, field: &tracing::field::Field, value: &dyn std::fmt::Debug) {
        self.0.push_str(&format!("{}={value:?} ", field.name()));
    }
}

impl tracing::Subscriber for Capture {
    fn enabled(&self, _: &tracing::Metadata<'_>) -> bool {
        true
    }
    fn new_span(&self, _: &tracing::span::Attributes<'_>) -> tracing::span::Id {
        tracing::span::Id::from_u64(1)
    }
    fn record(&self, _: &tracing::span::Id, _: &tracing::span::Record<'_>) {}
    fn record_follows_from(&self, _: &tracing::span::Id, _: &tracing::span::Id) {}
    fn event(&self, event: &tracing::Event<'_>) {
        let mut text = self.0.lock().expect("capture");
        event.record(&mut Fields(&mut text));
        text.push('\n');
    }
    fn enter(&self, _: &tracing::span::Id) {}
    fn exit(&self, _: &tracing::span::Id) {}
}

#[test]
fn dropped_account_events_are_logged_without_fields_and_rate_limited() {
    let cap = Arc::new(Capture::default());
    tracing::subscriber::with_default(cap.clone(), || {
        let b = Scripted::default();
        let mut s = Session::new(b.clone());
        let now = Instant::now();
        // Idle: both dropped, logged once each.
        b.push(Event::Account(AccountEvent::WorldList {
            worlds: vec![world(1, "SecretWorldName")],
        }));
        b.push(Event::Account(AccountEvent::WorldRefused {
            world: WorldId(1),
            reason: WorldRefusal::Full,
            detail: Some("SecretDetail".into()),
        }));
        // A second WorldList within 10 s: not logged again.
        b.push(Event::Account(AccountEvent::WorldList {
            worlds: vec![world(2, "SecretWorldName")],
        }));
        let _ = s.poll(now);
        b.push(Event::Account(AccountEvent::WorldList {
            worlds: vec![world(3, "SecretWorldName")],
        }));
        let _ = s.poll(now + Duration::from_secs(11));
    });
    let text = cap.0.lock().expect("capture").clone();
    assert_eq!(text.matches("dropping account event").count(), 3, "{text}");
    assert!(
        !text.contains("SecretWorldName") && !text.contains("SecretDetail"),
        "{text}"
    );
}
