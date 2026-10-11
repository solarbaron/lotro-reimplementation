// SPDX-License-Identifier: GPL-3.0-or-later
//! The "P4 social" revision of `docs/specs/scenario-feed.md` (decision 0035): Vector H
//! (§14.11), the P4 hostile rows and the plain-sandbox cases S7–S8 (§14.12), the limits of
//! §11.1, the overlay subset (§12.5.2) and the session rules of §9.5 for the new variants.
#![cfg(feature = "scenario")]

use std::sync::Arc;
use std::time::{Duration, Instant};

use tessera_net::domain::{SocialCommand, SocialEvent, Vital};
use tessera_net::sandbox::{SandboxBackend, SandboxConfig};
use tessera_net::scenario::{EmptyResolver, HeadlessDriver, Scenario, ScenarioErrorKind as K};
use tessera_net::{Backend, Command, EntityId, Event, NetError, Session};

fn run(text: &str) -> String {
    let scenario = Scenario::parse(text.as_bytes()).expect("parse");
    let resolved = Arc::new(scenario.resolve(&mut EmptyResolver).expect("resolve"));
    let config = SandboxConfig {
        scenario: Some(Arc::clone(&resolved)),
        ..SandboxConfig::default()
    };
    let session = Session::new(SandboxBackend::new(config).expect("sandbox"));
    let mut driver = HeadlessDriver::new(session, resolved, Some(Vec::new()));
    let _ = driver.run();
    String::from_utf8(driver.into_log().expect("log")).expect("utf-8")
}

fn check(text: &str, expected: &str) {
    let log = run(text);
    if log != expected {
        let diff: Vec<String> = log
            .lines()
            .zip(expected.lines().chain(std::iter::repeat("<missing>")))
            .enumerate()
            .filter(|(_, (a, b))| a != b)
            .take(5)
            .map(|(i, (a, b))| format!("line {}:\n  got:  {a}\n  want: {b}", i + 1))
            .collect();
        panic!("log differs:\n{}\n--- full log ---\n{log}", diff.join("\n"));
    }
}

// --- §14.11 Vector H ---------------------------------------------------------------------

const VECTOR_H: &str = r#"tessera-scenario 1
title "Vector H"
tick 100ms
player "Tam"
actor $brin kind=player name="Brin" pos=3,0,0 facing=270
at 0s spawn $brin
+0s fellowship player $brin "Cael" leader=player
+0s fellow-vital "Cael" health 300/500
+0s vital $brin health 800/800
+0.2s perform $brin "Reel" sync
+0s perform "Cael" "Reel" sync
at 1s perform-withdraw "Cael"
+0s perform "Cael" "Jig" sync
at 1.5s perform-start by="Cael" lead=1s
on play-sync
+0s say $brin "Ready when you are."
end
on play-start
+0s fellow-vital "Cael" power 50/100
end
input
at 0.5s play-sync "Reel"
at 2s play-start
end
"#;

const LOG_H: &str = r#"tessera-scenario-log 1
install none
tick 100
title "Vector H"
pre > Connect
pre Connected characters=["Tam"]
pre > EnterWorld character="Tam"
0.000 WorldEntered entity=player name="Tam" at=0.000,0.000,0.000 facing=90.00
0.000 LocationReport entity=player at=0:0.000,0.000,0.000 facing=90.00 anchor=none cause=entered
0.000 EntitySpawned entity=$brin kind=player name="Brin" at=3.000,0.000,0.000 facing=270.00
0.000 FellowshipChanged members=[{name="Tam" entity=player leader=yes},{name="Brin" entity=$brin leader=no},{name="Cael" entity=none leader=no}]
0.000 FellowVitalChanged member="Cael" vital=health current=300 max=500
0.000 VitalChanged entity=$brin vital=health current=800 max=800
0.200 PerformanceReady member="Brin" entity=$brin tune="Reel"
0.200 PerformanceReady member="Cael" entity=none tune="Reel"
0.500 > ReadyPerformance tune="Reel"
0.500 ChatMessage channel=say sender="Brin" text="Ready when you are."
1.000 PerformanceWithdrawn member="Cael"
1.000 PerformanceReady member="Cael" entity=none tune="Jig"
1.500 PerformanceSyncStart by="Cael" at_ms=2500 lead_ms=1000
2.000 > StartPerformance
2.000 PerformanceSyncStart by="Tam" at_ms=2700 lead_ms=700
2.000 FellowVitalChanged member="Cael" vital=power current=50 max=100
2.000 end reason=complete skipped=0 input-skipped=0 dropped=0 spilled=0
"#;

#[test]
fn vector_h_fellow_out_of_view_sync_start_player_commands() {
    check(VECTOR_H, LOG_H);
}

#[test]
fn vector_h_is_deterministic() {
    assert_eq!(run(VECTOR_H), run(VECTOR_H));
}

// --- §14.12 hostile rows -------------------------------------------------------------------

const H: &str = "tessera-scenario 1\n";

fn err(rest: &str) -> (K, u32, u32) {
    match Scenario::parse(format!("{H}{rest}").as_bytes()) {
        Ok(_) => panic!("accepted: {rest:?}"),
        Err(e) => (e.kind, e.line, e.column),
    }
}

fn overlay_err(rest: &str) -> (K, u32, u32) {
    let s = Scenario::parse(format!("{H}{rest}").as_bytes()).expect("a valid plain scenario");
    let e = s.check_overlay().expect_err("outside the overlay subset");
    (e.kind, e.line, e.column)
}

#[test]
fn hostile_rows_of_section_14_12() {
    assert_eq!(
        err("at 0s perform-start lead=11s\n"),
        (K::OutOfRange, 2, 21)
    );
    assert_eq!(
        err("at 0s perform-start by=$ghost\n"),
        (K::UndefinedBinding, 2, 21)
    );
    assert_eq!(
        err("at 0s perform \"Ann\" \"Tune\"\n"),
        (K::UnexpectedToken, 2, 15)
    );
    assert_eq!(
        err("at 0s perform \"Ann\" \"Tune\" sync delay=1s\n"),
        (K::UnexpectedToken, 2, 33)
    );
    assert_eq!(
        err("at 0s fellow-vital \"Ann\" health 5/4\n"),
        (K::OutOfRange, 2, 33)
    );
    assert_eq!(
        err("at 0s fellow-vital player health 1/1\n"),
        (K::UnexpectedToken, 2, 20)
    );
    assert_eq!(
        overlay_err("at 0s perform-start\n"),
        (K::NotAllowedInOverlay, 2, 1)
    );
    assert_eq!(
        overlay_err("on play-start\nend\n"),
        (K::NotAllowedInOverlay, 2, 1)
    );
}

#[test]
fn further_static_errors_of_the_p4_forms() {
    // Lead bounds: 10 s is the largest, 0 s the smallest.
    assert!(Scenario::parse(format!("{H}at 0s perform-start lead=10s\n").as_bytes()).is_ok());
    assert!(Scenario::parse(format!("{H}at 0s perform-start lead=0s\n").as_bytes()).is_ok());
    // Tunes: 256 bytes at most (§11.1), empty refused.
    let long = "a".repeat(257);
    assert_eq!(
        err(&format!("at 0s perform \"Ann\" \"{long}\" sync\n")),
        (K::TooLong, 2, 21)
    );
    assert_eq!(
        err("input\nat 0s play-sync \"\"\nend\n"),
        (K::OutOfRange, 3, 17)
    );
    assert_eq!(
        err(&format!("input\nat 0s play-sync \"{long}\"\nend\n")),
        (K::TooLong, 3, 17)
    );
    assert_eq!(
        err("input\nat 0s play-sync Reel\nend\n"),
        (K::UnexpectedToken, 3, 17)
    );
    assert_eq!(
        err("input\nat 0s play-start now\nend\n"),
        (K::UnexpectedToken, 3, 18)
    );
    // `sync` after an extra argument, and arguments after `sync`.
    assert_eq!(
        err("at 0s perform \"Ann\" \"Tune\" sync x\n"),
        (K::UnexpectedToken, 2, 33)
    );
    // `perform-withdraw` needs one member; `fellow-vital` a vital.
    assert_eq!(err("at 0s perform-withdraw\n"), (K::MissingArgument, 2, 1));
    assert_eq!(
        err("at 0s fellow-vital \"Ann\" morale 1/2\n"),
        (K::UnexpectedToken, 2, 26)
    );
    // Patterns: `tune=` is a STRING of at most 256 bytes.
    assert_eq!(
        err("on play-sync tune=Reel\nend\n"),
        (K::UnexpectedToken, 2, 14)
    );
    assert_eq!(err("on disband x\nend\n"), (K::UnexpectedToken, 2, 12));
    // A raw TAB in a tune (the one control character the lexer keeps) is refused
    // statically, not only skipped at run time by `Session` (§9.4; §11.1 OPEN).
    assert_eq!(
        err("input\nat 0s play-sync \"a\tb\"\nend\n"),
        (K::OutOfRange, 3, 17)
    );
    assert_eq!(
        err("on play-sync tune=\"a\tb\"\nend\n"),
        (K::OutOfRange, 2, 14)
    );
    // Overlay runs: a P4 step inside an `on` of the subset, and the input forms.
    assert_eq!(
        overlay_err("at 0s fellow-vital \"Ann\" health 1/2\n"),
        (K::NotAllowedInOverlay, 2, 1)
    );
    assert_eq!(
        overlay_err("input\nat 0s play-start\nend\n"),
        (K::NotAllowedInOverlay, 3, 1)
    );
    assert_eq!(
        overlay_err("on disband\nend\n"),
        (K::NotAllowedInOverlay, 2, 1)
    );
}

// --- §5.7, §6.7, §11.1: waiting parts at run time -----------------------------------------

/// 64 distinct members wait; a 65th is skipped (counted); a repeat of a waiting member
/// replaces its part; a start empties them; a withdrawal with nothing waiting is skipped.
#[test]
fn waiting_parts_are_bounded_and_emptied() {
    let mut text = String::from("tessera-scenario 1\ntick 100ms\nplayer \"Tam\"\n");
    for i in 0..65 {
        text.push_str(&format!("at 0s perform \"M{i}\" \"T\" sync\n"));
    }
    text.push_str("at 0s perform \"M0\" \"U\" sync\n");
    text.push_str("at 0.1s perform-start\n");
    text.push_str("at 0.2s perform-withdraw \"M0\"\n");
    text.push_str("at 0.2s perform-withdraw player\n");
    let log = run(&text);
    assert_eq!(log.matches("PerformanceReady").count(), 65, "{log}");
    assert!(!log.contains("member=\"M64\""), "{log}");
    assert!(log.contains("0.000 PerformanceReady member=\"M0\" entity=none tune=\"U\""));
    assert!(log.contains("0.100 PerformanceSyncStart by=none at_ms=800 lead_ms=700"));
    assert!(!log.contains("PerformanceWithdrawn"), "{log}");
    assert!(log.contains("skipped=3"), "{log}");
}

/// `ReadyPerformance` records the player's part (no event); `perform-withdraw player`
/// then withdraws it. Under `defaults off` the command records nothing (§5.7).
#[test]
fn ready_performance_records_the_players_part() {
    let text = "tessera-scenario 1\ntick 100ms\nplayer \"Tam\"\n\
                at 0.5s perform-withdraw player\n\
                input\nat 0.1s play-sync \"Reel\"\nend\n";
    let log = run(text);
    assert!(
        log.contains("0.100 > ReadyPerformance tune=\"Reel\""),
        "{log}"
    );
    assert!(
        log.contains("0.500 PerformanceWithdrawn member=\"Tam\""),
        "{log}"
    );
    let off = text.replace("tick 100ms\n", "tick 100ms\ndefaults off\n");
    let log = run(&off);
    assert!(!log.contains("PerformanceWithdrawn"), "{log}");
    assert!(log.contains("skipped=1"), "{log}");
}

/// The patterns: `play-sync tune=` matches the exact tune only; `disband` fires on
/// `DisbandFellowship`, which has no generic answer.
#[test]
fn patterns_match_the_new_commands() {
    let text = "tessera-scenario 1\ntick 100ms\nplayer \"Tam\"\n\
                on play-sync tune=\"Jig\"\n+0s system \"jig\"\nend\n\
                on disband\n+0s fellowship none\nend\n\
                input\nat 0.1s play-sync \"Reel\"\nat 0.2s play-sync \"Jig\"\n\
                at 0.3s disband\nend\n";
    let log = run(text);
    assert!(!log.contains("0.100 ChatMessage"), "{log}");
    assert!(
        log.contains("0.200 ChatMessage channel=system sender=none text=\"jig\""),
        "{log}"
    );
    assert!(log.contains("0.300 > DisbandFellowship"), "{log}");
    assert!(log.contains("0.300 FellowshipChanged members=[]"), "{log}");
}

/// A `fellow-vital` with a resource vital, and `perform-start by=player`.
#[test]
fn fellow_vital_resource_and_start_by_entity() {
    let text = "tessera-scenario 1\ntick 100ms\nplayer \"Tam\"\n\
                at 0s fellow-vital \"Cael\" resource=\"Fervour\" 2/5\n\
                +0s perform player \"Reel\" sync\n\
                at 0.3s perform-start by=player lead=0s\n";
    let log = run(text);
    assert!(
        log.contains(
            "FellowVitalChanged member=\"Cael\" vital=resource:\"Fervour\" current=2 max=5"
        ),
        "{log}"
    );
    assert!(log.contains("PerformanceReady member=\"Tam\" entity=player tune=\"Reel\""));
    assert!(log.contains("0.300 PerformanceSyncStart by=\"Tam\" at_ms=300 lead_ms=0"));
}

// --- §9.8 and §14.12 S7–S8: the plain sandbox ------------------------------------------------

fn plain_entered() -> (Session, Instant) {
    let config = SandboxConfig {
        characters: vec!["Testa".into()],
        wanderers: 0,
        latency: Duration::ZERO,
        ..SandboxConfig::default()
    };
    let mut s = Session::new(SandboxBackend::new(config).expect("sandbox"));
    let t0 = Instant::now();
    s.send(t0, Command::Connect).unwrap();
    let _ = s.poll(t0);
    s.send(
        t0,
        Command::EnterWorld {
            character: "Testa".into(),
        },
    )
    .unwrap();
    let ev = s.poll(t0);
    assert!(
        ev.iter().any(|e| matches!(e, Event::WorldEntered { .. })),
        "{ev:?}"
    );
    (s, t0)
}

#[test]
fn s7_plain_sandbox_answers_start_performance() {
    let (mut s, t0) = plain_entered();
    let t = t0 + Duration::from_micros(1_234_500);
    s.send(t, Command::Social(SocialCommand::StartPerformance))
        .unwrap();
    let ev = s.poll(t);
    let starts: Vec<_> = ev
        .iter()
        .filter_map(|e| match e {
            Event::Social(SocialEvent::PerformanceSyncStart { by, at_ms, lead_ms }) => {
                Some((by.clone(), *at_ms, *lead_ms))
            }
            _ => None,
        })
        .collect();
    assert_eq!(starts, vec![(Some("Testa".to_owned()), 1_934, 700)]);
}

#[test]
fn s8_plain_sandbox_ignores_the_other_new_commands() {
    let (mut s, t0) = plain_entered();
    for c in [
        Command::Social(SocialCommand::ReadyPerformance {
            tune: "Reel".into(),
        }),
        Command::Social(SocialCommand::DisbandFellowship),
        Command::Social(SocialCommand::JoinChannel {
            name: "Lore".into(),
        }),
    ] {
        s.send(t0, c).unwrap();
    }
    let ev = s.poll(t0);
    assert!(
        !ev.iter()
            .any(|e| matches!(e, Event::Social(_) | Event::ChatMessage { .. })),
        "{ev:?}"
    );
}

// --- §9.5: session rules -----------------------------------------------------------------

#[test]
fn ready_performance_tune_is_validated() {
    let (mut s, t0) = plain_entered();
    for tune in [String::new(), "a".repeat(257), "a\nb".to_owned()] {
        assert!(matches!(
            s.send(
                t0,
                Command::Social(SocialCommand::ReadyPerformance { tune })
            ),
            Err(NetError::InvalidCommand(_))
        ));
    }
    assert!(
        s.send(
            t0,
            Command::Social(SocialCommand::ReadyPerformance {
                tune: "a".repeat(256)
            })
        )
        .is_ok()
    );
}

/// A backend that delivers scripted events: `early` with `Connected`, `late` after
/// `WorldEntered`.
#[derive(Debug, Default)]
struct Raw {
    early: Vec<Event>,
    late: Vec<Event>,
    queue: Vec<Event>,
}

impl Backend for Raw {
    fn name(&self) -> &str {
        "raw"
    }

    fn submit(&mut self, _now: Instant, command: Command) -> Result<(), NetError> {
        match command {
            Command::Connect => {
                self.queue.push(Event::Connected {
                    characters: vec![tessera_net::CharacterSummary { name: "Tam".into() }],
                });
                self.queue.append(&mut self.early);
            }
            Command::EnterWorld { character } => {
                self.queue.push(Event::WorldEntered {
                    player: EntityId(1),
                    name: character,
                    transform: tessera_net::Transform::new(tessera_net::Vec3::default(), 0.0),
                });
                self.queue.append(&mut self.late);
            }
            _ => {}
        }
        Ok(())
    }

    fn poll(&mut self, _now: Instant, out: &mut Vec<Event>) {
        out.append(&mut self.queue);
    }
}

#[test]
fn inbound_p4_events_are_cleaned_and_checked() {
    let now = Instant::now();
    let raw = Raw {
        // Before world entry the events follow the entity rule: dropped.
        early: vec![Event::Social(SocialEvent::PerformanceWithdrawn {
            member: "Early".into(),
        })],
        late: vec![
            Event::Social(SocialEvent::FellowVitalChanged {
                member: "Cael".into(),
                vital: Vital::Health,
                current: f32::NAN,
                max: 1.0,
            }),
            Event::Social(SocialEvent::FellowVitalChanged {
                member: "x".repeat(100),
                vital: Vital::Power,
                current: 1.0,
                max: f32::INFINITY,
            }),
            Event::Social(SocialEvent::PerformanceReady {
                member: format!("A\u{7}{}", "b".repeat(100)),
                entity: None,
                tune: "t".repeat(400),
            }),
            Event::Social(SocialEvent::PerformanceSyncStart {
                by: Some("c".repeat(70)),
                at_ms: u64::MAX,
                lead_ms: u32::MAX,
            }),
        ],
        queue: Vec::new(),
    };
    let mut s = Session::new(raw);
    s.send(now, Command::Connect).unwrap();
    let ev = s.poll(now);
    assert!(!ev.iter().any(|e| matches!(e, Event::Social(_))), "{ev:?}");
    s.send(
        now,
        Command::EnterWorld {
            character: "Tam".into(),
        },
    )
    .unwrap();
    let ev = s.poll(now);
    let social: Vec<_> = ev
        .iter()
        .filter_map(|e| match e {
            Event::Social(s) => Some(s.clone()),
            _ => None,
        })
        .collect();
    assert_eq!(social.len(), 2, "non-finite vitals are dropped: {social:?}");
    match &social[0] {
        SocialEvent::PerformanceReady { member, tune, .. } => {
            assert!(member.len() <= 64 && !member.chars().any(char::is_control));
            assert_eq!(tune.len(), 256);
        }
        other => panic!("{other:?}"),
    }
    match &social[1] {
        SocialEvent::PerformanceSyncStart { by: Some(by), .. } => assert_eq!(by.len(), 64),
        other => panic!("{other:?}"),
    }
}

// --- §12.5.2, decision 0035 D3: overlay runs send every Social command to the server ------

/// A server double that records the commands it receives and refuses the Social ones
/// except the channel commands, as the test-server backend does at minor 2.
#[derive(Debug, Default)]
struct Server {
    queue: Vec<Event>,
    received: std::sync::Arc<std::sync::Mutex<Vec<&'static str>>>,
}

impl Backend for Server {
    fn name(&self) -> &str {
        "server double"
    }

    fn submit(&mut self, _now: Instant, command: Command) -> Result<(), NetError> {
        self.received.lock().unwrap().push(command.name());
        match command {
            Command::Connect => self.queue.push(Event::Connected {
                characters: vec![tessera_net::CharacterSummary {
                    name: "Aldith".into(),
                }],
            }),
            Command::EnterWorld { character } => self.queue.push(Event::WorldEntered {
                player: EntityId(42),
                name: character,
                transform: tessera_net::Transform::new(tessera_net::Vec3::default(), 0.0),
            }),
            Command::Social(SocialCommand::JoinChannel { name }) => {
                self.queue
                    .push(Event::Social(SocialEvent::ChannelJoined { channel: name }));
            }
            Command::Social(_) => return Err(NetError::InvalidCommand("refused")),
            _ => {}
        }
        Ok(())
    }

    fn poll(&mut self, _now: Instant, out: &mut Vec<Event>) {
        out.append(&mut self.queue);
    }
}

impl tessera_net::scenario::OverlayServer for Server {
    fn appearance_change(&self) -> bool {
        false
    }

    fn send_appearance(&mut self, _appearance: &tessera_net::Appearance) -> Result<(), NetError> {
        Ok(())
    }
}

#[test]
fn overlay_social_commands_go_to_the_server() {
    let scenario = Scenario::parse(
        b"tessera-scenario 1\ntick 100ms\nselect $g = did 0x70001234 all\nitem $s $g[0]\nat 0s give $s\n",
    )
    .unwrap();
    scenario.check_overlay().unwrap();
    let resolved = Arc::new(scenario.resolve(&mut EmptyResolver).unwrap());
    let server = Server::default();
    let received = std::sync::Arc::clone(&server.received);
    let mut s = Session::new(tessera_net::scenario::OverlayBackend::new(server, resolved));
    let t0 = Instant::now();
    s.send(t0, Command::Connect).unwrap();
    let _ = s.poll(t0);
    s.send(
        t0,
        Command::EnterWorld {
            character: "Aldith".into(),
        },
    )
    .unwrap();
    let _ = s.poll(t0);
    s.send(
        t0,
        Command::Social(SocialCommand::JoinChannel {
            name: "Lore".into(),
        }),
    )
    .unwrap();
    for c in [
        Command::Social(SocialCommand::StartPerformance),
        Command::Social(SocialCommand::ReadyPerformance {
            tune: "Reel".into(),
        }),
        Command::Social(SocialCommand::DisbandFellowship),
    ] {
        assert!(matches!(s.send(t0, c), Err(NetError::InvalidCommand(_))));
    }
    let mut ev = Vec::new();
    for i in 0..5 {
        s.poll_into(t0 + Duration::from_millis(100 * i), &mut ev);
    }
    // The server's answer arrives; the feed answers none of the Social commands.
    assert!(ev.iter().any(|e| matches!(
        e,
        Event::Social(SocialEvent::ChannelJoined { channel }) if channel == "Lore"
    )));
    assert!(
        !ev.iter()
            .any(|e| matches!(e, Event::Social(SocialEvent::PerformanceSyncStart { .. }))),
        "{ev:?}"
    );
    assert_eq!(
        *received.lock().unwrap(),
        vec![
            "Connect",
            "EnterWorld",
            "JoinChannel",
            "StartPerformance",
            "ReadyPerformance",
            "DisbandFellowship"
        ]
    );
}
