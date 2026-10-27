// SPDX-License-Identifier: GPL-3.0-or-later
//! The way from the world back to character selection (`Command::LeaveWorld`,
//! `Event::ReturnedToCharacterSelect`; `official-messages-play.md` §7.2, §8) and the
//! client's loading signal (`Command::WorldLoaded`, §4.5 item 4): the session rules with a
//! scripted backend, and the sandbox's answer.

use std::sync::{Arc, Mutex};
use std::time::Instant;

use tessera_net::sandbox::{SandboxBackend, SandboxConfig};
use tessera_net::{
    Backend, CharacterSummary, Command, Event, NetError, Session, SessionState, Transform, Vec3,
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

fn names(list: &[&str]) -> Vec<CharacterSummary> {
    list.iter()
        .map(|n| CharacterSummary {
            name: (*n).to_owned(),
        })
        .collect()
}

fn returned(list: &[&str]) -> Event {
    Event::ReturnedToCharacterSelect {
        characters: names(list),
    }
}

/// A scripted session in the world as entity 7.
fn scripted_in_world(t: Instant) -> (Scripted, Session) {
    let b = Scripted::default();
    let mut s = Session::new(b.clone());
    s.send(t, Command::Connect).expect("connect");
    b.push(Event::Connected {
        characters: names(&["Aldo"]),
    });
    let _ = s.poll(t);
    s.send(
        t,
        Command::EnterWorld {
            character: "Aldo".into(),
        },
    )
    .expect("enter");
    b.push(Event::WorldEntered {
        player: tessera_net::EntityId(7),
        name: "Aldo".into(),
        transform: Transform::new(Vec3::ZERO, 0.0),
    });
    let _ = s.poll(t);
    assert_eq!(s.state(), SessionState::InWorld);
    (b, s)
}

#[test]
fn the_return_goes_from_the_world_to_character_selection() {
    let t = Instant::now();
    let (b, mut s) = scripted_in_world(t);
    s.send(t, Command::WorldLoaded).expect("loaded");
    s.send(t, Command::LeaveWorld).expect("leave");
    assert_eq!(s.state(), SessionState::InWorld, "until the answer");
    b.push(returned(&["Aldo", "Bera\u{7}"]));
    let ev = s.poll(t);
    assert_eq!(s.state(), SessionState::Connected);
    assert_eq!(s.player(), None);
    let Some(Event::ReturnedToCharacterSelect { characters }) = ev.first() else {
        panic!("{ev:?}");
    };
    assert_eq!(
        characters,
        &names(&["Aldo"]),
        "invalid names are dropped as in Connected"
    );
    // Entity events now make no sense; world commands are refused; entering again works.
    b.push(Event::EntityDespawned {
        id: tessera_net::EntityId(7),
    });
    assert!(s.poll(t).is_empty());
    assert!(matches!(
        s.send(t, Command::LeaveWorld),
        Err(NetError::InvalidState { .. })
    ));
    assert!(matches!(
        s.send(t, Command::WorldLoaded),
        Err(NetError::InvalidState { .. })
    ));
    s.send(
        t,
        Command::EnterWorld {
            character: "Aldo".into(),
        },
    )
    .expect("enter again");
    let got = b.got.lock().expect("got").clone();
    assert!(got.contains(&Command::WorldLoaded) && got.contains(&Command::LeaveWorld));
}

#[test]
fn a_return_outside_the_world_is_dropped() {
    let t = Instant::now();
    let b = Scripted::default();
    let mut s = Session::new(b.clone());
    s.send(t, Command::Connect).expect("connect");
    b.push(returned(&["Aldo"]));
    assert!(s.poll(t).is_empty(), "not while connecting");
    assert_eq!(s.state(), SessionState::Connecting);
    b.push(Event::Connected {
        characters: names(&["Aldo"]),
    });
    let _ = s.poll(t);
    b.push(returned(&["Aldo"]));
    assert!(s.poll(t).is_empty(), "not at character selection");
    // Overtaken by Disconnect: stale, dropped.
    let (b, mut s) = scripted_in_world(t);
    s.send(t, Command::Disconnect).expect("disconnect");
    b.push(returned(&["Aldo"]));
    assert!(s.poll(t).is_empty());
}

#[test]
fn the_sandbox_leaves_the_world_and_enters_again() {
    let t = Instant::now();
    let config = SandboxConfig {
        characters: vec!["Alda".into(), "Brenna".into()],
        wanderers: 2,
        ..SandboxConfig::default()
    };
    let mut s = Session::new(SandboxBackend::new(config).expect("sandbox"));
    s.send(t, Command::Connect).expect("connect");
    let _ = s.poll(t);
    s.send(
        t,
        Command::EnterWorld {
            character: "Brenna".into(),
        },
    )
    .expect("enter");
    let ev = s.poll(t);
    assert!(ev.iter().any(|e| matches!(e, Event::WorldEntered { .. })));
    assert_eq!(s.state(), SessionState::InWorld);
    s.send(t, Command::WorldLoaded)
        .expect("the sandbox ignores it");
    assert!(
        s.poll(t)
            .iter()
            .all(|e| !matches!(e, Event::ReturnedToCharacterSelect { .. }))
    );
    s.send(t, Command::LeaveWorld).expect("leave");
    let ev = s.poll(t);
    let despawned = ev
        .iter()
        .filter(|e| matches!(e, Event::EntityDespawned { .. }))
        .count();
    assert_eq!(despawned, 3, "two wanderers and the player: {ev:?}");
    assert!(ev.iter().any(|e| matches!(e, Event::StaticsCleared)));
    let Some(Event::ReturnedToCharacterSelect { characters }) = ev.last() else {
        panic!("{ev:?}");
    };
    assert_eq!(characters, &names(&["Alda", "Brenna"]));
    assert_eq!(s.state(), SessionState::Connected);
    assert_eq!(s.player(), None);
    // Another character enters the same session.
    s.send(
        t,
        Command::EnterWorld {
            character: "Alda".into(),
        },
    )
    .expect("enter again");
    let ev = s.poll(t);
    assert!(
        ev.iter()
            .any(|e| matches!(e, Event::WorldEntered { name, .. } if name == "Alda")),
        "{ev:?}"
    );
    assert_eq!(s.state(), SessionState::InWorld);
}

#[cfg(feature = "scenario")]
#[test]
fn a_scenario_refuses_to_leave_the_world() {
    use tessera_net::scenario::{EmptyResolver, Scenario};
    let text = "tessera-scenario 1\ntick 100ms\nplayer \"Testa\"\n";
    let resolved = Scenario::parse(text.as_bytes())
        .expect("parse")
        .resolve(&mut EmptyResolver)
        .expect("resolve");
    let config = SandboxConfig {
        scenario: Some(Arc::new(resolved)),
        ..SandboxConfig::default()
    };
    let mut s = Session::new(SandboxBackend::new(config).expect("sandbox"));
    let t = Instant::now();
    s.send(t, Command::Connect).expect("connect");
    let _ = s.poll(t);
    s.send(
        t,
        Command::EnterWorld {
            character: "Testa".into(),
        },
    )
    .expect("enter");
    let _ = s.poll(t);
    assert_eq!(s.state(), SessionState::InWorld);
    assert!(matches!(
        s.send(t, Command::LeaveWorld),
        Err(NetError::InvalidCommand(_))
    ));
    assert_eq!(s.state(), SessionState::InWorld);
}

/// A world source whose named places never resolve (a teleport stays pending).
#[derive(Debug)]
struct NeverResolves;

impl tessera_net::sandbox::world::WorldSource for NeverResolves {
    fn resolve_place(
        &mut self,
        _name: &str,
    ) -> tessera_net::sandbox::world::Lookup<tessera_net::sandbox::world::ResolvedPlace> {
        tessera_net::sandbox::world::Lookup::Pending
    }

    fn static_entities(
        &mut self,
        _block: tessera_net::Landblock,
        _limit: usize,
        _out: &mut Vec<tessera_net::sandbox::world::StaticEntity>,
    ) -> tessera_net::sandbox::world::Lookup<()> {
        tessera_net::sandbox::world::Lookup::Ready(())
    }
}

#[test]
fn leaving_the_world_answers_a_pending_teleport_first() {
    let t = Instant::now();
    let mut s = Session::new(
        SandboxBackend::with_world(SandboxConfig::default(), NeverResolves).expect("sandbox"),
    );
    s.send(t, Command::Connect).expect("connect");
    let ev = s.poll(t);
    let Some(Event::Connected { characters }) = ev.first() else {
        panic!("{ev:?}");
    };
    let character = characters.first().expect("a character").name.clone();
    s.send(t, Command::EnterWorld { character }).expect("enter");
    let _ = s.poll(t);
    assert_eq!(s.state(), SessionState::InWorld);
    s.send(
        t,
        Command::Teleport {
            destination: tessera_net::Destination::Named("somewhere".into()),
        },
    )
    .expect("teleport");
    let _ = s.poll(t);
    s.send(t, Command::LeaveWorld).expect("leave");
    let ev = s.poll(t);
    let cancel = ev.iter().position(
        |e| matches!(e, Event::ChatMessage { text, .. } if text.starts_with("Teleport cancelled")),
    );
    let back = ev
        .iter()
        .position(|e| matches!(e, Event::ReturnedToCharacterSelect { .. }));
    assert!(
        matches!((cancel, back), (Some(c), Some(b)) if c < b),
        "the cancellation line comes before the return: {ev:?}"
    );
    assert_eq!(s.state(), SessionState::Connected);
}
