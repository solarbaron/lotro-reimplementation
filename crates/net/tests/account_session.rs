// SPDX-License-Identifier: GPL-3.0-or-later
//! The Account group's session rules (`docs/specs/official-entry.md` §8.2, test OE-T13):
//! Account events in each session state, `ChooseWorld` with an ID of an older list,
//! out-of-bound lists, events after `LoginRefused`, and Account commands on the sandbox and
//! test-server backends.

use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use tessera_net::account::{
    AccountCommand, AccountEvent, LoginRefusal, LoginStage, WorldEntry, WorldId, WorldLoad,
    WorldRefusal, WorldStatus,
};
use tessera_net::sandbox::{SandboxBackend, SandboxConfig};
use tessera_net::{
    Backend, CharacterSummary, Command, DisconnectReason, Event, NetError, Session, SessionState,
};

/// A backend that emits whatever the test queues and records the commands it gets.
#[derive(Debug, Default, Clone)]
struct Scripted {
    queue: Arc<Mutex<Vec<Event>>>,
    got: Arc<Mutex<Vec<Command>>>,
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

    fn submit(&mut self, _now: Instant, command: Command) -> Result<(), NetError> {
        self.got.lock().expect("got").push(command);
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

fn list(ids: &[u32]) -> Event {
    Event::Account(AccountEvent::WorldList {
        worlds: ids
            .iter()
            .map(|&i| world(i, &format!("World {i}")))
            .collect(),
    })
}

fn progress() -> Event {
    Event::Account(AccountEvent::LoginProgress {
        stage: LoginStage::Authenticating,
    })
}

fn connecting() -> (Session, Scripted, Instant) {
    let b = Scripted::default();
    let mut s = Session::new(b.clone());
    let now = Instant::now();
    s.send(now, Command::Connect).expect("connect");
    (s, b, now)
}

#[test]
fn account_events_surface_only_while_connecting() {
    // Idle: a backend bug, dropped.
    let b = Scripted::default();
    let mut s = Session::new(b.clone());
    let now = Instant::now();
    b.push(progress());
    assert!(s.poll(now).is_empty());
    assert_eq!(s.state(), SessionState::Idle);

    // Connecting: surfaced.
    let (mut s, b, now) = connecting();
    b.push(progress());
    b.push(list(&[1, 2]));
    let events = s.poll(now);
    assert_eq!(events.len(), 2);
    assert_eq!(s.state(), SessionState::Connecting);

    // Connected: dropped.
    b.push(Event::Connected {
        characters: vec![CharacterSummary { name: "A".into() }],
    });
    b.push(progress());
    let events = s.poll(now);
    assert_eq!(events.len(), 1);
    assert_eq!(s.state(), SessionState::Connected);

    // Closing from Connecting: stale, dropped quietly.
    let (mut s, b, now) = connecting();
    s.send(now, Command::Disconnect).expect("disconnect");
    b.push(list(&[1]));
    b.push(Event::Disconnected {
        reason: DisconnectReason::ClientRequested,
    });
    let events = s.poll(now);
    assert_eq!(events.len(), 1);
    assert!(matches!(events.first(), Some(Event::Disconnected { .. })));

    // Closed: nothing.
    b.push(progress());
    assert!(s.poll(now + Duration::from_secs(1)).is_empty());
}

#[test]
fn choose_world_needs_an_id_of_the_latest_list() {
    let (mut s, b, now) = connecting();
    // No list yet.
    assert!(matches!(
        s.send(
            now,
            Command::Account(AccountCommand::ChooseWorld { world: WorldId(1) })
        ),
        Err(NetError::InvalidCommand(_))
    ));
    b.push(list(&[1, 2]));
    let _ = s.poll(now);
    s.send(
        now,
        Command::Account(AccountCommand::ChooseWorld { world: WorldId(2) }),
    )
    .expect("in the list");
    // A newer list replaces the older one: ID 1 is gone.
    b.push(list(&[2, 3]));
    let _ = s.poll(now);
    assert!(matches!(
        s.send(
            now,
            Command::Account(AccountCommand::ChooseWorld { world: WorldId(1) })
        ),
        Err(NetError::InvalidCommand(_))
    ));
    s.send(now, Command::Account(AccountCommand::LeaveLoginQueue))
        .expect("leave");
    let got = b.got.lock().expect("got").clone();
    assert_eq!(got.len(), 3, "{got:?}");
}

#[test]
fn account_commands_only_while_connecting() {
    let b = Scripted::default();
    let mut s = Session::new(b.clone());
    let now = Instant::now();
    assert!(matches!(
        s.send(now, Command::Account(AccountCommand::LeaveLoginQueue)),
        Err(NetError::InvalidState { .. })
    ));
    s.send(now, Command::Connect).expect("connect");
    b.push(Event::Connected { characters: vec![] });
    let _ = s.poll(now);
    assert!(matches!(
        s.send(now, Command::Account(AccountCommand::LeaveLoginQueue)),
        Err(NetError::InvalidState { .. })
    ));
}

#[test]
fn out_of_bound_events_are_dropped() {
    let (mut s, b, now) = connecting();
    b.push(Event::Account(AccountEvent::WorldList { worlds: vec![] }));
    b.push(list(&[1, 1]));
    b.push(Event::Account(AccountEvent::WorldList {
        worlds: (0..257).map(|i| world(i, "W")).collect(),
    }));
    b.push(Event::Account(AccountEvent::WorldList {
        worlds: vec![world(1, &"n".repeat(65))],
    }));
    b.push(Event::Account(AccountEvent::WorldList {
        worlds: vec![world(1, "\u{0}\u{1}  ")],
    }));
    b.push(Event::Account(AccountEvent::LoginQueueChanged {
        position: Some(1),
        length: None,
        wait_seconds: Some(86_401),
    }));
    b.push(Event::Account(AccountEvent::WorldRefused {
        world: WorldId(1),
        reason: WorldRefusal::Other,
        detail: Some("d".repeat(257)),
    }));
    assert!(s.poll(now).is_empty());
    // Within bounds: control characters replaced.
    b.push(Event::Account(AccountEvent::WorldList {
        worlds: vec![world(7, "Bad\u{7}Name")],
    }));
    let events = s.poll(now);
    let Some(Event::Account(AccountEvent::WorldList { worlds })) = events.first() else {
        panic!("{events:?}");
    };
    assert_eq!(worlds.first().map(|w| w.name.as_str()), Some("Bad Name"));
}

#[test]
fn after_login_refused_only_disconnected_follows() {
    let (mut s, b, now) = connecting();
    b.push(Event::Account(AccountEvent::LoginRefused {
        reason: LoginRefusal::BadCredentials,
        retry_after_seconds: None,
        detail: None,
    }));
    b.push(list(&[1]));
    b.push(Event::Connected { characters: vec![] });
    b.push(Event::Disconnected {
        reason: DisconnectReason::ServerClosed("bye".into()),
    });
    let events = s.poll(now);
    let names: Vec<&str> = events.iter().map(Event::name).collect();
    assert_eq!(names, ["LoginRefused", "Disconnected"]);
    assert_eq!(s.state(), SessionState::Closed);
}

#[test]
fn commands_after_login_refused_are_refused_but_disconnect() {
    let (mut s, b, now) = connecting();
    b.push(list(&[1]));
    b.push(Event::Account(AccountEvent::LoginRefused {
        reason: LoginRefusal::Other,
        retry_after_seconds: Some(5),
        detail: Some("x".into()),
    }));
    assert_eq!(s.poll(now).len(), 2);
    assert!(matches!(
        s.send(
            now,
            Command::Account(AccountCommand::ChooseWorld { world: WorldId(1) })
        ),
        Err(NetError::InvalidState { .. })
    ));
    s.send(now, Command::Disconnect)
        .expect("disconnect cancels");
}

#[test]
fn sandbox_refuses_account_commands() {
    let mut s = Session::new(SandboxBackend::new(SandboxConfig::default()).expect("sandbox"));
    let now = Instant::now();
    s.send(now, Command::Connect).expect("connect");
    // Still Connecting until the poll: the command reaches the backend, which refuses it.
    assert!(matches!(
        s.send(now, Command::Account(AccountCommand::LeaveLoginQueue)),
        Err(NetError::InvalidCommand(_))
    ));
    let events = s.poll(now);
    assert!(
        events.iter().all(|e| !matches!(e, Event::Account(_))),
        "{events:?}"
    );
}

#[test]
fn test_server_backend_refuses_account_commands() {
    let config = tessera_net::remote::RemoteConfig::new(
        "tessera://127.0.0.1:1",
        "account",
        tessera_net::remote::Password::new("pw".to_owned()),
    )
    .expect("config");
    let mut b = tessera_net::remote::RemoteBackend::new(config).expect("backend");
    // Straight to the backend: the session would refuse the command in Idle already.
    assert!(matches!(
        b.submit(
            Instant::now(),
            Command::Account(AccountCommand::ChooseWorld { world: WorldId(1) })
        ),
        Err(NetError::InvalidCommand(_))
    ));
}
