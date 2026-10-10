// SPDX-License-Identifier: GPL-3.0-or-later
//! The windowed path of `--scenario` without a window (`docs/specs/scenario-feed.md`
//! §12.4): the client's world session plays a scenario in real time, enters the scenario's
//! character, and its events reach the existing handlers: chat lines for the chat window
//! and the overlay, actors in the entity table, entity events for the walker's figures;
//! the HUD line follows the scenario's state.

use std::sync::Arc;
use std::time::{Duration, Instant};

use tessera_client::net::World;
use tessera_net::sandbox::SandboxConfig;
use tessera_net::scenario::{EmptyResolver, Scenario};
use tessera_net::{EntityKind, Event, SessionState};

const SCENARIO: &str = "tessera-scenario 1
title \"Window test\"
tick 50ms
player \"Tamsin\"
character \"Orrin\"
enter \"Orrin\"
actor $host kind=npc name=\"Host\" pos=0,2,0
at 0s spawn $host
+0s system \"Doors open.\"
+0.2s say $host \"Welcome.\"
+0.2s despawn $host
";

fn world(text: &str) -> World {
    let sc = Scenario::parse(text.as_bytes()).expect("parses");
    let mut resolved = sc.resolve(&mut EmptyResolver).expect("resolves");
    resolved.set_plays_input(false);
    let config = SandboxConfig {
        scenario: Some(Arc::new(resolved)),
        ..SandboxConfig::default()
    };
    World::sandbox_with(Instant::now(), config).expect("session")
}

#[test]
fn a_scenario_plays_through_the_world_session() {
    let mut w = world(SCENARIO);
    w.set_character(Some("Orrin".into()));
    w.capture_entity_events(true);
    let start = Instant::now();
    let mut chat = Vec::new();
    let mut hud = Vec::new();
    let mut events = Vec::new();
    let mut saw_host = false;
    let mut line = None;
    for k in 0..400u64 {
        let now = start + Duration::from_millis(10 * k);
        w.update(now);
        chat.extend(w.take_chat());
        w.drain_hud_chat(&mut hud);
        w.drain_entity_events(&mut events, usize::MAX);
        saw_host |= w
            .entities()
            .values()
            .any(|e| e.name == "Host" && e.kind == EntityKind::NonPlayer);
        line = w.scenario_line(now);
        if line.as_deref().is_some_and(|l| l.contains("finished")) {
            break;
        }
    }
    assert_eq!(w.state(), SessionState::InWorld);
    let player = w.player().expect("in the world");
    assert_eq!(
        w.entities().get(&player).map(|e| e.name.as_str()),
        Some("Orrin"),
        "the scenario's `enter` character is entered"
    );
    assert!(saw_host, "the actor reached the entity table");
    assert!(
        !w.entities().values().any(|e| e.name == "Host"),
        "and left it again"
    );
    let texts: Vec<(&str, &str)> = chat
        .iter()
        .map(|l| (l.sender.as_str(), l.text.as_str()))
        .collect();
    assert_eq!(texts, [("", "Doors open."), ("Host", "Welcome.")]);
    assert_eq!(chat[0].chat_type, 8, "system lines are Log lines");
    assert_eq!(hud.len(), 2, "the overlay gets the same lines");
    assert!(
        events
            .iter()
            .any(|e| matches!(e, Event::EntityDespawned { .. })),
        "entity events reach the walker's queue"
    );
    let line = line.expect("a HUD line while a scenario plays");
    assert!(line.starts_with("Scenario finished (complete)"), "{line}");
}

#[test]
fn a_world_without_a_scenario_has_no_scenario_line() {
    let mut w = World::sandbox(Instant::now()).expect("session");
    w.update(Instant::now());
    assert_eq!(w.scenario_line(Instant::now()), None);
    assert!(w.scenario_status().is_none());
}

#[test]
fn an_unknown_enter_character_ends_the_session_with_a_notice() {
    let mut w = world("tessera-scenario 1\nplayer \"Tamsin\"\n");
    w.set_character(Some("Nobody".into()));
    let start = Instant::now();
    let mut chat = Vec::new();
    for k in 0..50u64 {
        w.update(start + Duration::from_millis(10 * k));
        chat.extend(w.take_chat());
    }
    assert!(w.player().is_none());
    assert!(chat.iter().any(|l| l.text.contains("Tamsin")), "{chat:?}");
    let line = w.scenario_line(start).expect("a scenario line");
    assert!(
        line.starts_with("Scenario stopped (disconnected)"),
        "a closed session never shows as running: {line}"
    );
}
