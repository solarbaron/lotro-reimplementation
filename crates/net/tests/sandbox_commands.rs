// SPDX-License-Identifier: GPL-3.0-or-later
//! The sandbox backend's answers to the commands that replaced the remote side channels
//! (`tessera-protocol.md` Appendix A implementer OPEN): `Register`, `Stance`, `Emote` and
//! `SetAppearance`.

use std::time::{Duration, Instant};

use tessera_net::sandbox::{SandboxBackend, SandboxConfig};
use tessera_net::{
    Appearance, Body, Command, EntityId, EntityState, Event, Session, SessionState, Stance,
    Transform, Vec3,
};

fn session() -> Session {
    Session::new(
        SandboxBackend::new(SandboxConfig {
            characters: vec!["Alda".into()],
            spawn: Transform::new(Vec3::new(10.0, 20.0, 1.0), 0.0),
            wanderers: 0,
            ..SandboxConfig::default()
        })
        .unwrap(),
    )
}

/// A session in the world as Alda, and the player's entity.
fn in_world(t0: Instant) -> (Session, EntityId) {
    let mut s = session();
    s.send(t0, Command::Connect).unwrap();
    let _ = s.poll(t0);
    s.send(
        t0,
        Command::EnterWorld {
            character: "Alda".into(),
        },
    )
    .unwrap();
    let player = s
        .poll(t0)
        .iter()
        .find_map(|e| match e {
            Event::WorldEntered { player, .. } => Some(*player),
            _ => None,
        })
        .expect("entered");
    (s, player)
}

/// The `EntityStateChanged` states for `id` in `events`.
fn states(events: &[Event], id: EntityId) -> Vec<EntityState> {
    events
        .iter()
        .filter_map(|e| match e {
            Event::EntityStateChanged { id: i, state } if *i == id => Some(*state),
            _ => None,
        })
        .collect()
}

#[test]
fn register_connects_like_connect() {
    let t0 = Instant::now();
    let mut s = session();
    s.send(t0, Command::Register).unwrap();
    assert_eq!(s.state(), SessionState::Connecting);
    let ev = s.poll(t0);
    assert!(
        ev.iter()
            .any(|e| matches!(e, Event::Connected { characters }
            if characters.iter().any(|c| c.name == "Alda"))),
        "{ev:?}"
    );
    assert_eq!(s.state(), SessionState::Connected);
}

#[test]
fn stance_and_emote_change_the_player_state_once() {
    let t0 = Instant::now();
    let (mut s, player) = in_world(t0);
    let t1 = t0 + Duration::from_millis(10);
    s.send(
        t1,
        Command::Stance {
            stance: Stance::Ready,
        },
    )
    .unwrap();
    let first = states(&s.poll(t1), player);
    assert_eq!(first.len(), 1, "{first:?}");
    assert_eq!(first[0].stance, Stance::Ready);
    assert!(first[0].emote.is_none());
    // The same stance again changes nothing, so no event.
    s.send(
        t1,
        Command::Stance {
            stance: Stance::Ready,
        },
    )
    .unwrap();
    assert!(states(&s.poll(t1), player).is_empty());

    let t2 = t0 + Duration::from_millis(500);
    s.send(
        t2,
        Command::Emote {
            anim: 5,
            looping: true,
        },
    )
    .unwrap();
    let emote = states(&s.poll(t2), player);
    assert_eq!(emote.len(), 1, "{emote:?}");
    let e = emote[0].emote.expect("an emote plays");
    assert_eq!((e.anim, e.looping), (5, true));
    // `started` is the time since world entry (the sandbox's server clock).
    assert_eq!(e.started, 500_000);
    assert_eq!(emote[0].stance, Stance::Ready, "the stance is kept");

    // Anim 0 stops the emote.
    s.send(
        t2,
        Command::Emote {
            anim: 0,
            looping: false,
        },
    )
    .unwrap();
    let stopped = states(&s.poll(t2), player);
    assert_eq!(stopped.len(), 1, "{stopped:?}");
    assert!(stopped[0].emote.is_none());
}

#[test]
fn set_appearance_answers_for_the_player() {
    let t0 = Instant::now();
    let (mut s, player) = in_world(t0);
    let wanted = Appearance::new(Body::Character {
        species: 23,
        sex: 1,
    });
    s.send(
        t0,
        Command::SetAppearance {
            appearance: Box::new(wanted.clone()),
        },
    )
    .unwrap();
    let ev = s.poll(t0);
    assert!(
        ev.iter()
            .any(|e| matches!(e, Event::AppearanceChanged { id, appearance }
            if *id == player && **appearance == wanted)),
        "{ev:?}"
    );
}
