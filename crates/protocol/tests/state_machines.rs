// SPDX-License-Identifier: GPL-3.0-or-later
//! The client and server state machines driven against each other in memory: every
//! transmit is encoded to wire bytes and fed to the other side.

use tessera_protocol::client::{ClientConfig, ClientEvent, ClientSession, ClientState, CloseCause};
use tessera_protocol::codes::{AppErrorCode, ChatRejectedCode, LoginFailedCode};
use tessera_protocol::frame::{StreamKind, encode_datagram, frame_to_vec};
use tessera_protocol::message::*;
use tessera_protocol::movement::{MoveVerdict, MovementLimits, MovementValidator};
use tessera_protocol::server::{ServerEvent, ServerParams, ServerSession, ServerState};
use tessera_protocol::session::{
    ConnectionEnd, PeerDatagrams, StreamEnd, StreamRef, Transmit, Violation,
};
use tessera_protocol::types::*;
use tessera_protocol::value::{Property, Value, pid};
use tessera_protocol::{Channel, Message};

const MS: u64 = 1_000;
const S: u64 = 1_000_000;

struct Pair {
    client: ClientSession,
    server: ServerSession,
    client_events: Vec<ClientEvent>,
    server_events: Vec<ServerEvent>,
    client_closed: Option<(AppErrorCode, String)>,
    server_closed: Option<(AppErrorCode, String)>,
    client_violation: Option<Violation>,
    server_violation: Option<Violation>,
    now: u64,
    greeting_events: (Vec<ClientEvent>, Vec<ServerEvent>),
}

fn params() -> ServerParams {
    ServerParams {
        caps: ServerCaps::REGISTRATION
            | ServerCaps::CHARACTER_CREATION
            | ServerCaps::APPEARANCE_CHANGE,
        server_name: "Test Shard".into(),
        motd: "welcome".into(),
        ..ServerParams::default()
    }
}

impl Pair {
    fn new(config: ClientConfig, params: ServerParams) -> Self {
        let mut p = Self {
            client: ClientSession::new(config),
            server: ServerSession::new(params, 0),
            client_events: Vec::new(),
            server_events: Vec::new(),
            client_closed: None,
            server_closed: None,
            client_violation: None,
            server_violation: None,
            now: 0,
            greeting_events: (Vec::new(), Vec::new()),
        };
        p.client.connect(0).unwrap();
        p.server
            .handshake_complete(0, PeerDatagrams::MaxFrameSize(65_535))
            .unwrap();
        p.client
            .handshake_complete(0, PeerDatagrams::MaxPayload(1_162))
            .unwrap();
        p.pump();
        p.greeting_events = (p.take_client_events(), p.take_server_events());
        p
    }

    /// Moves every transmit across until both sides are quiet.
    fn pump(&mut self) {
        loop {
            let mut moved = false;
            while let Some(t) = self.client.poll_transmit() {
                moved = true;
                match t {
                    Transmit::Message(m) => {
                        let r = if m.channel() == Channel::Datagram {
                            let b = encode_datagram(&m).unwrap();
                            self.server
                                .on_datagram(&b, self.now)
                                .map(|e| e.into_iter().collect())
                        } else {
                            let b = frame_to_vec(&m).unwrap();
                            self.server.on_control_data(&b, self.now)
                        };
                        match r {
                            Ok(events) => self.server_events.extend(events),
                            Err(v) => self.server_violation = Some(v),
                        }
                    }
                    Transmit::Close { code, reason } => {
                        self.client_closed = Some((code, reason.clone()));
                        if let Some(e) = self
                            .server
                            .connection_closed(ConnectionEnd::PeerClosed { code, reason })
                        {
                            self.server_events.push(e);
                        }
                    }
                    Transmit::FinishControl | Transmit::OpenStream(_) => {}
                }
            }
            while let Some(t) = self.server.poll_transmit() {
                moved = true;
                match t {
                    Transmit::Message(m) => {
                        let r = match m.channel() {
                            Channel::Datagram => {
                                let b = encode_datagram(&m).unwrap();
                                self.client
                                    .on_datagram(&b, self.now)
                                    .map(|e| e.into_iter().collect())
                            }
                            Channel::Control => {
                                let b = frame_to_vec(&m).unwrap();
                                self.client.on_control_data(&b, self.now)
                            }
                            Channel::World => {
                                let b = frame_to_vec(&m).unwrap();
                                self.client.on_stream_data(StreamKind::World, &b, self.now)
                            }
                            Channel::Chat => {
                                let b = frame_to_vec(&m).unwrap();
                                self.client.on_stream_data(StreamKind::Chat, &b, self.now)
                            }
                        };
                        match r {
                            Ok(events) => self.client_events.extend(events),
                            Err(v) => self.client_violation = Some(v),
                        }
                    }
                    Transmit::OpenStream(kind) => {
                        if let Err(v) = self.client.on_uni_stream(kind.to_byte()) {
                            self.client_violation = Some(v);
                        }
                    }
                    Transmit::Close { code, reason } => {
                        self.server_closed = Some((code, reason.clone()));
                        if let Some(e) = self
                            .client
                            .connection_closed(ConnectionEnd::PeerClosed { code, reason })
                        {
                            self.client_events.push(e);
                        }
                    }
                    Transmit::FinishControl => {}
                }
            }
            if !moved {
                break;
            }
        }
    }

    fn take_server_events(&mut self) -> Vec<ServerEvent> {
        std::mem::take(&mut self.server_events)
    }

    fn take_client_events(&mut self) -> Vec<ClientEvent> {
        std::mem::take(&mut self.client_events)
    }

    fn login(&mut self) {
        self.client
            .login(
                Credentials {
                    account: "alice".into(),
                    password: "correct horse".into(),
                },
                self.now,
            )
            .unwrap();
        self.pump();
        let ev = self.take_server_events();
        assert!(
            matches!(
                &ev[..],
                [ServerEvent::Login {
                    register: false,
                    ..
                }]
            ),
            "{ev:?}"
        );
        self.server
            .accept_login(
                "alice".into(),
                vec![CharacterSummary {
                    name: "Aldith".into(),
                    body: Body::Character {
                        species: 23,
                        sex: 1,
                    },
                }],
                self.now,
            )
            .unwrap();
        self.pump();
    }

    fn enter(&mut self) {
        self.client.enter_world("Aldith".into(), self.now).unwrap();
        self.pump();
        let ev = self.take_server_events();
        assert!(matches!(&ev[..], [ServerEvent::EnterWorld { name }] if name == "Aldith"));
        self.server.enter_world(world_enter()).unwrap();
        self.server
            .send(
                Message::WorldClock(WorldClock {
                    server_time: 5 * S,
                    game_clock: 43_200.0,
                    rate: 1.0,
                }),
                self.now,
            )
            .unwrap();
        self.server
            .send(
                Message::EntityUpdate(EntityUpdate {
                    id: 42,
                    changes: vec![Property::new(pid::RUN_SPEED, Value::Float32(6.0))],
                }),
                self.now,
            )
            .unwrap();
        self.pump();
    }
}

fn world_enter() -> WorldEnter {
    WorldEnter {
        player: 42,
        name: "Aldith".into(),
        instance: 1,
        epoch: 1,
        server_time: 5 * S,
        pose: Pose {
            location: Location::new(1, 20_250.5, 22_500.25, 401.0),
            heading: 0.0,
            anchor: None,
        },
        appearance: Appearance::new(Body::Character {
            species: 23,
            sex: 1,
        }),
    }
}

fn bram(id: u64, region: u16) -> EntityCreate {
    EntityCreate {
        id,
        kind: EntityKind::Player,
        name: "Bram".into(),
        class: None,
        server_time: 6 * S,
        pose: Pose {
            location: Location::new(region, 20_260.0, 22_500.0, 400.5),
            heading: 3.0,
            anchor: None,
        },
        velocity: Vec3::new(2.0, 0.0, 0.0),
        motion: Motion::new(Gait::Walking, false),
        appearance: Appearance::new(Body::Character {
            species: 81,
            sex: 2,
        }),
        properties: vec![Property::new(pid::STANCE, Value::Enum(0))],
    }
}

#[test]
fn happy_path() {
    let mut p = Pair::new(ClientConfig::default(), params());
    assert_eq!(p.client.state(), ClientState::Authenticating);
    assert_eq!(p.server.state(), ServerState::AwaitLogin);
    let (ev, sev) = std::mem::take(&mut p.greeting_events);
    assert!(matches!(&ev[..], [ClientEvent::HelloAccepted(a)] if a.server_name == "Test Shard"));
    assert!(matches!(&sev[..], [ServerEvent::Greeted(_)]));

    p.login();
    assert_eq!(p.client.state(), ClientState::CharacterSelect);
    assert!(p.client.may_read(StreamKind::Chat));
    assert!(!p.client.may_read(StreamKind::World));
    let ev = p.take_client_events();
    assert!(
        matches!(&ev[..], [ClientEvent::LoggedIn { account, characters }]
        if account == "alice" && characters.len() == 1)
    );

    // Character creation.
    p.client
        .create_character(
            "Bram".into(),
            Appearance::new(Body::Character {
                species: 81,
                sex: 2,
            }),
        )
        .unwrap();
    p.pump();
    assert!(
        matches!(&p.take_server_events()[..], [ServerEvent::CharacterCreate(c)] if c.name == "Bram")
    );
    p.server.character_created(vec![]).unwrap();
    p.pump();
    assert!(matches!(
        &p.take_client_events()[..],
        [ClientEvent::CharacterList(_)]
    ));

    p.enter();
    assert_eq!(p.client.state(), ClientState::InWorld);
    assert_eq!(p.server.state(), ServerState::InWorld);
    let ev = p.take_client_events();
    assert!(
        matches!(
            &ev[..],
            [
                ClientEvent::WorldEntered(_),
                ClientEvent::WorldClock(_),
                ClientEvent::EntityUpdated(_)
            ]
        ),
        "{ev:?}"
    );

    // Entities, motion, staleness.
    p.server
        .send(Message::EntityCreate(bram(43, 1)), p.now)
        .unwrap();
    p.pump();
    assert!(matches!(&p.take_client_events()[..], [ClientEvent::EntityCreated(c)] if c.id == 43));
    let entry = MotionEntry {
        id: 43,
        position: Vec3::new(20_262.0, 22_500.0, 400.5),
        heading: 3.0,
        velocity: Vec3::new(2.0, 0.0, 0.0),
        motion: Motion::new(Gait::Walking, false),
    };
    let unknown = MotionEntry { id: 99, ..entry };
    for t in [7 * S, 6 * S, 7 * S] {
        p.server
            .send(
                Message::MotionBatch(MotionBatch {
                    server_time: t,
                    entries: vec![entry, unknown],
                }),
                p.now,
            )
            .unwrap();
    }
    p.pump();
    let ev = p.take_client_events();
    assert_eq!(ev.len(), 1, "{ev:?}");
    assert!(matches!(&ev[0], ClientEvent::Motion { entries, .. } if entries.len() == 1));

    // Movement through the validator.
    let mut validator =
        MovementValidator::new(MovementLimits::default(), 1, world_enter().pose, 5 * S);
    p.client
        .move_input(
            Location::new(1, 20_252.0, 22_500.25, 401.0),
            0.0,
            Vec3::new(6.0, 0.0, 0.0),
            Motion::new(Gait::Running, false),
            p.now,
        )
        .unwrap();
    p.pump();
    match &p.take_server_events()[..] {
        [ServerEvent::Move(m)] => {
            assert_eq!(validator.check(m, 5 * S + 66 * MS), MoveVerdict::Accepted)
        }
        other => panic!("{other:?}"),
    }

    // Chat: the server echoes the line.
    p.client.chat(ChatChannel::Say, "hello".into()).unwrap();
    p.pump();
    match &p.take_server_events()[..] {
        [ServerEvent::Chat(c)] => assert_eq!(c.text, "hello"),
        other => panic!("{other:?}"),
    }
    p.server
        .send(
            Message::ChatMessage(ChatMessage {
                server_time: 7 * S,
                channel: ChatChannel::Say,
                echo: true,
                sender_entity: 42,
                sender: "Aldith".into(),
                text: "hello".into(),
            }),
            p.now,
        )
        .unwrap();
    p.pump();
    assert!(matches!(&p.take_client_events()[..], [ClientEvent::Chat(c)] if c.echo));

    // Teleport to another region: InterestReset, then the report.
    let id = p
        .client
        .teleport(Destination::Named {
            name: "origin".into(),
        })
        .unwrap();
    p.pump();
    assert!(
        matches!(&p.take_server_events()[..], [ServerEvent::Teleport(t)] if t.request_id == id)
    );
    p.server.send(Message::InterestReset, p.now).unwrap();
    p.server
        .send(
            Message::LocationReport(LocationReport {
                cause: LocationCause::Teleported,
                request_id: id,
                epoch: 2,
                server_time: 8 * S,
                pose: Pose {
                    location: Location::new(7, 100.0, 100.0, 0.0),
                    heading: 0.0,
                    anchor: None,
                },
            }),
            p.now,
        )
        .unwrap();
    p.pump();
    let ev = p.take_client_events();
    assert!(
        matches!(&ev[..], [ClientEvent::InterestReset(ids), ClientEvent::Location(_)] if ids == &[43]),
        "{ev:?}"
    );
    assert_eq!(p.client.region(), Some(7));
    assert_eq!(p.client.epoch(), Some(2));
    assert!(p.client_violation.is_none());

    // Ping both ways.
    let nonce = p.client.ping(p.now).unwrap();
    p.now += 30 * MS;
    p.pump();
    assert!(
        matches!(&p.take_client_events()[..], [ClientEvent::Pong { nonce: n, .. }] if *n == nonce)
    );
    p.server.ping(p.now).unwrap();
    p.pump();
    assert!(matches!(
        &p.take_server_events()[..],
        [ServerEvent::Pong { .. }]
    ));

    // Disconnect.
    p.client.disconnect(AppErrorCode::NoError, "bye", p.now);
    assert_eq!(p.client.state(), ClientState::Closing);
    p.pump();
    assert!(matches!(
        &p.take_server_events()[..],
        [ServerEvent::Closed {
            code: AppErrorCode::NoError,
            client: true
        }]
    ));
    assert_eq!(p.server.state(), ServerState::Closed);
    assert_eq!(p.client.state(), ClientState::Closed);
    assert!(p.client_violation.is_none() && p.server_violation.is_none());
}

#[test]
fn time_sync_through_sessions() {
    let mut p = Pair::new(ClientConfig::default(), params());
    p.now = 10 * S;
    assert!(p.client.time_request_due(p.now));
    p.client.time_request(p.now).unwrap();
    p.pump();
    let ev = p.take_client_events();
    assert!(
        matches!(&ev[..], [ClientEvent::TimeSample(s)] if s.delay == 0 && s.offset == 0),
        "{ev:?}"
    );
    assert_eq!(p.client.server_time(p.now), Some(10 * S));
    // At most four requests per second are answered.
    for _ in 0..6 {
        p.client.time_request(p.now).unwrap();
    }
    p.pump();
    // The first request of this second was already answered: three more.
    assert_eq!(p.take_client_events().len(), 3);
    assert_eq!(p.server.dropped(), 3);
}

#[test]
fn three_login_failures_disconnect() {
    let mut p = Pair::new(ClientConfig::default(), params());
    for i in 0..3 {
        p.client
            .login(
                Credentials {
                    account: "alice".into(),
                    password: "wrong".into(),
                },
                p.now,
            )
            .unwrap();
        p.pump();
        assert!(matches!(
            &p.take_server_events()[..],
            [ServerEvent::Login { .. }]
        ));
        p.server
            .reject_login(LoginFailedCode::BadCredentials, 0, "", p.now)
            .unwrap();
        p.pump();
        let ev = p.take_client_events();
        if i < 2 {
            assert!(matches!(&ev[..], [ClientEvent::LoginFailed(_)]));
        } else {
            assert!(
                matches!(
                    &ev[..],
                    [
                        ClientEvent::LoginFailed(_),
                        ClientEvent::Closed(CloseCause::ServerDisconnect {
                            code: AppErrorCode::AuthFailed,
                            ..
                        })
                    ]
                ),
                "{ev:?}"
            );
        }
    }
    assert_eq!(p.client.state(), ClientState::Closed);
}

#[test]
fn invalid_account_and_empty_password_are_answered_by_the_session() {
    let mut p = Pair::new(ClientConfig::default(), params());
    p.client
        .login(
            Credentials {
                account: "not valid!".into(),
                password: "x".into(),
            },
            0,
        )
        .unwrap();
    p.pump();
    assert!(p.take_server_events().is_empty());
    assert!(matches!(&p.take_client_events()[..],
        [ClientEvent::LoginFailed(f)] if f.code == LoginFailedCode::InvalidAccountName));
    p.client
        .login(
            Credentials {
                account: "alice".into(),
                password: String::new(),
            },
            0,
        )
        .unwrap();
    p.pump();
    assert!(matches!(&p.take_client_events()[..],
        [ClientEvent::LoginFailed(f)] if f.code == LoginFailedCode::BadCredentials));
}

#[test]
fn unsupported_version() {
    let mut params = params();
    params.min_minor = 1;
    params.max_minor = 2;
    let p = Pair::new(ClientConfig::default(), params);
    assert_eq!(p.client.state(), ClientState::Closed);
    assert!(matches!(
        p.client.close_cause(),
        Some(CloseCause::ServerDisconnect {
            code: AppErrorCode::UnsupportedVersion,
            ..
        })
    ));
}

#[test]
fn datagrams_required() {
    let mut s = ServerSession::new(params(), 0);
    assert_eq!(
        s.handshake_complete(0, PeerDatagrams::Absent),
        Ok(Some(ServerEvent::Closed {
            code: AppErrorCode::DatagramsRequired,
            client: false
        }))
    );
    assert!(matches!(
        s.poll_transmit(),
        Some(Transmit::Close {
            code: AppErrorCode::DatagramsRequired,
            ..
        })
    ));
    let mut c = ClientSession::new(ClientConfig::default());
    c.connect(0).unwrap();
    assert_eq!(
        c.handshake_complete(0, PeerDatagrams::MaxFrameSize(1_199)),
        Ok(Some(ClientEvent::Closed(CloseCause::Violation(
            Violation::DatagramsRequired
        ))))
    );
    assert_eq!(c.state(), ClientState::Closed);
    // A usable payload of 1,100 or more (what quinn reports at a 1,200-byte MTU) is fine;
    // less is not.
    for (payload, ok) in [(1_162, true), (1_100, true), (1_099, false)] {
        let mut c = ClientSession::new(ClientConfig::default());
        c.connect(0).unwrap();
        let r = c.handshake_complete(0, PeerDatagrams::MaxPayload(payload));
        assert_eq!(r.unwrap().is_none(), ok, "{payload}");
        let mut s = ServerSession::new(params(), 0);
        let r = s.handshake_complete(0, PeerDatagrams::MaxPayload(payload));
        assert_eq!(r.unwrap().is_none(), ok, "{payload}");
    }
}

#[test]
fn client_violations_close_the_server() {
    let mut p = Pair::new(ClientConfig::default(), params());
    // EnterWorld before LoginOk: bypass the client's own checks.
    let bytes = frame_to_vec(&Message::EnterWorld(EnterWorld {
        name: "Aldith".into(),
    }))
    .unwrap();
    let r = p.server.on_control_data(&bytes, 0);
    assert!(matches!(r, Err(Violation::UnexpectedMessage { .. })));
    assert!(matches!(
        p.server.poll_transmit(),
        Some(Transmit::Close {
            code: AppErrorCode::ProtocolViolation,
            ..
        })
    ));
    assert_eq!(p.server.state(), ServerState::Closed);

    // A second Hello.
    let mut p = Pair::new(ClientConfig::default(), params());
    let hello = frame_to_vec(&Message::Hello(Hello {
        minor_max: 0,
        client_caps: 0,
        max_entities: 64,
        client_name: String::new(),
    }))
    .unwrap();
    assert!(p.server.on_control_data(&hello, 0).is_err());

    // Two outstanding logins.
    let mut p = Pair::new(ClientConfig::default(), params());
    let login = frame_to_vec(&Message::Login(Credentials {
        account: "alice".into(),
        password: "x".into(),
    }))
    .unwrap();
    let two = [login.clone(), login].concat();
    assert!(matches!(
        p.server.on_control_data(&two, 0),
        Err(Violation::RequestOutstanding(MessageType::Login))
    ));

    // Register without the capability.
    let mut params = params();
    params.caps = ServerCaps::NONE;
    let mut p = Pair::new(ClientConfig::default(), params);
    let reg = frame_to_vec(&Message::Register(Credentials {
        account: "bob".into(),
        password: "x".into(),
    }))
    .unwrap();
    assert!(matches!(
        p.server.on_control_data(&reg, 0),
        Err(Violation::CapabilityMissing(MessageType::Register))
    ));
}

#[test]
fn server_violations_close_the_client() {
    // Region change without InterestReset.
    let mut p = Pair::new(ClientConfig::default(), params());
    p.login();
    p.enter();
    p.server
        .send(Message::EntityCreate(bram(43, 1)), 0)
        .unwrap();
    p.server
        .send(
            Message::LocationReport(LocationReport {
                cause: LocationCause::Teleported,
                request_id: 0,
                epoch: 2,
                server_time: 8 * S,
                pose: Pose {
                    location: Location::new(9, 0.0, 0.0, 0.0),
                    heading: 0.0,
                    anchor: None,
                },
            }),
            0,
        )
        .unwrap();
    p.pump();
    assert!(matches!(p.client_violation, Some(Violation::World(_))));
    assert_eq!(p.client.state(), ClientState::Closed);
    assert_eq!(
        p.client_closed.as_ref().map(|c| c.0),
        Some(AppErrorCode::ProtocolViolation)
    );

    // Wrong epoch on a correction; EntityCreate for the player; destroy of the player.
    for bad in [
        Message::LocationReport(LocationReport {
            cause: LocationCause::Corrected,
            request_id: 0,
            epoch: 5,
            server_time: 0,
            pose: world_enter().pose,
        }),
        Message::EntityCreate(bram(42, 1)),
        Message::EntityCreate(bram(44, 3)),
        Message::EntityDestroy(EntityDestroy {
            id: 42,
            reason: DestroyReason::Removed,
        }),
    ] {
        let mut p = Pair::new(ClientConfig::default(), params());
        p.login();
        p.enter();
        p.server.send(bad, 0).unwrap();
        p.pump();
        assert!(p.client_violation.is_some());
    }

    // Stale cases are dropped, not fatal (§13.2).
    let mut p = Pair::new(ClientConfig::default(), params());
    p.login();
    p.enter();
    p.take_client_events();
    p.server
        .send(
            Message::EntityUpdate(EntityUpdate {
                id: 77,
                changes: vec![Property::new(pid::STANCE, Value::Enum(1))],
            }),
            0,
        )
        .unwrap();
    p.server
        .send(
            Message::EntityDestroy(EntityDestroy {
                id: 77,
                reason: DestroyReason::Removed,
            }),
            0,
        )
        .unwrap();
    p.server
        .send(Message::EntityCreate(bram(43, 1)), 0)
        .unwrap();
    p.server
        .send(Message::EntityCreate(bram(43, 1)), 0)
        .unwrap();
    p.pump();
    assert!(p.client_violation.is_none());
    assert_eq!(p.take_client_events().len(), 1);
    assert_eq!(p.client.dropped(), 3);
}

#[test]
fn entity_budget_is_enforced() {
    let config = ClientConfig {
        max_entities: 64,
        ..ClientConfig::default()
    };
    let mut p = Pair::new(config, params());
    p.login();
    p.enter();
    assert_eq!(p.server.entity_budget(), 64);
    for id in 100..164 {
        p.server
            .send(Message::EntityCreate(bram(id, 1)), 0)
            .unwrap();
    }
    p.pump();
    assert!(p.client_violation.is_none());
    p.server
        .send(Message::EntityCreate(bram(500, 1)), 0)
        .unwrap();
    p.pump();
    assert!(p.client_violation.is_some());
}

#[test]
fn chat_rate_limit_and_flooding() {
    let mut p = Pair::new(ClientConfig::default(), params());
    p.login();
    p.enter();
    p.take_client_events();
    for _ in 0..5 {
        p.client.chat(ChatChannel::Yell, "x".into()).unwrap();
    }
    p.pump();
    assert_eq!(p.take_server_events().len(), 5);
    p.client.chat(ChatChannel::Yell, "x".into()).unwrap();
    p.pump();
    assert!(matches!(&p.take_client_events()[..],
        [ClientEvent::ChatRejected(f)] if f.code == ChatRejectedCode::RateLimited));
    // Invalid text from the client gets INVALID_TEXT (after the token check).
    p.now = 2 * S;
    p.client.chat(ChatChannel::Say, "a\u{7}b".into()).unwrap();
    p.pump();
    assert!(matches!(&p.take_client_events()[..],
        [ClientEvent::ChatRejected(f)] if f.code == ChatRejectedCode::InvalidText));
    for _ in 0..18 {
        p.client.chat(ChatChannel::Yell, "x".into()).unwrap();
    }
    p.pump();
    assert!(matches!(
        p.client.close_cause(),
        Some(CloseCause::ServerDisconnect {
            code: AppErrorCode::Flooding,
            ..
        })
    ));
}

#[test]
fn control_flood_disconnects() {
    let mut p = Pair::new(ClientConfig::default(), params());
    p.login();
    p.enter();
    for _ in 0..251 {
        if p.client.query_location().is_err() {
            break;
        }
    }
    p.pump();
    assert!(matches!(p.server_violation, Some(Violation::Flooding)));
    assert!(matches!(
        p.client.close_cause(),
        Some(CloseCause::ServerDisconnect {
            code: AppErrorCode::Flooding,
            ..
        })
    ));
}

#[test]
fn timeouts() {
    // Server: no Hello within 10 s and nothing on the control stream, so no stream exists
    // for a Disconnect: closed at once.
    let mut s = ServerSession::new(params(), 0);
    s.handshake_complete(0, PeerDatagrams::MaxFrameSize(65_535))
        .unwrap();
    assert_eq!(s.poll_timeout(), Some(10 * S));
    assert!(s.handle_timeout(9 * S).is_none());
    assert!(matches!(
        s.handle_timeout(10 * S),
        Some(ServerEvent::Closed {
            code: AppErrorCode::AuthTimeout,
            ..
        })
    ));
    assert!(matches!(
        s.poll_transmit(),
        Some(Transmit::Close {
            code: AppErrorCode::AuthTimeout,
            ..
        })
    ));
    assert!(s.poll_transmit().is_none());
    assert_eq!(s.state(), ServerState::Closed);
    assert_eq!(s.connection_closed(ConnectionEnd::Local), None);

    // Server: part of a Hello arrived, but not all of it within 10 s: Disconnect.
    let mut s = ServerSession::new(params(), 0);
    s.handshake_complete(0, PeerDatagrams::MaxFrameSize(65_535))
        .unwrap();
    assert_eq!(s.on_control_data(&[9, 0], S), Ok(vec![]));
    assert!(matches!(
        s.handle_timeout(10 * S),
        Some(ServerEvent::Closed {
            code: AppErrorCode::AuthTimeout,
            ..
        })
    ));
    assert!(
        matches!(s.poll_transmit(), Some(Transmit::Message(Message::Disconnect(d))) if d.code == AppErrorCode::AuthTimeout)
    );
    assert_eq!(s.state(), ServerState::Closing);
    // Linger, then close.
    assert_eq!(s.poll_timeout(), Some(12 * S));
    while s.poll_transmit().is_some() {}
    s.handle_timeout(12 * S);
    assert!(matches!(
        s.poll_transmit(),
        Some(Transmit::Close {
            code: AppErrorCode::AuthTimeout,
            ..
        })
    ));

    // Server: no login within 60 s of HelloAck.
    let mut p = Pair::new(ClientConfig::default(), params());
    assert_eq!(p.server.poll_timeout(), Some(60 * S));
    p.server.handle_timeout(60 * S);
    p.pump();
    assert!(matches!(
        p.client.close_cause(),
        Some(CloseCause::ServerDisconnect {
            code: AppErrorCode::AuthTimeout,
            ..
        })
    ));

    // Server: 15 min idle in CharacterSelect.
    let mut p = Pair::new(ClientConfig::default(), params());
    p.login();
    assert_eq!(p.server.poll_timeout(), Some(15 * 60 * S));
    p.now = S;
    p.client.ping(p.now).unwrap();
    p.pump();
    assert_eq!(p.server.poll_timeout(), Some(S + 15 * 60 * S));
    p.server.handle_timeout(S + 15 * 60 * S);
    p.pump();
    assert!(matches!(
        p.client.close_cause(),
        Some(CloseCause::ServerDisconnect {
            code: AppErrorCode::Idle,
            ..
        })
    ));

    // Client: no HelloAck within 10 s.
    let mut c = ClientSession::new(ClientConfig::default());
    c.connect(0).unwrap();
    c.handshake_complete(S, PeerDatagrams::MaxFrameSize(65_535))
        .unwrap();
    assert_eq!(c.poll_timeout(), Some(11 * S));
    assert!(matches!(
        c.handle_timeout(11 * S),
        Some(ClientEvent::Closed(CloseCause::Failure(
            tessera_protocol::codes::ClientFailure::Timeout
        )))
    ));
}

#[test]
fn usage_errors_are_not_fatal() {
    let mut p = Pair::new(ClientConfig::default(), params());
    assert!(p.client.chat(ChatChannel::Say, "x".into()).is_err());
    assert!(p.client.enter_world("Aldith".into(), 0).is_err());
    assert!(p.client.chat(ChatChannel::System, "x".into()).is_err());
    assert_eq!(p.client.state(), ClientState::Authenticating);
    p.login();
    assert!(p.client.chat(ChatChannel::System, "x".into()).is_err());
    assert!(p.server.send(Message::InterestReset, 0).is_err());
    assert!(p.client.ping(0).is_ok());
    assert!(p.client.ping(0).is_err());
}

// --- Regression tests of the protocol review ---

fn time_reply_datagram(client_time: u64) -> Vec<u8> {
    encode_datagram(&Message::TimeReply(TimeReply {
        client_time,
        server_receive: 9 * S,
        server_send: 9 * S,
        tick: 0,
    }))
    .unwrap()
}

#[test]
fn unsolicited_and_duplicate_time_replies_are_dropped() {
    let mut p = Pair::new(ClientConfig::default(), params());
    p.now = 5 * S;
    // Never requested.
    assert_eq!(
        p.client.on_datagram(&time_reply_datagram(5), p.now),
        Ok(None)
    );
    assert_eq!(p.client.clock().offset(), None);
    // Requested at 5 s: a made-up t0 is dropped, the real one counts once.
    p.client.time_request(p.now).unwrap();
    while p.client.poll_transmit().is_some() {}
    let at = p.now + 50 * MS;
    assert_eq!(
        p.client.on_datagram(&time_reply_datagram(p.now + 1), at),
        Ok(None)
    );
    assert!(matches!(
        p.client.on_datagram(&time_reply_datagram(p.now), at),
        Ok(Some(ClientEvent::TimeSample(_)))
    ));
    assert_eq!(
        p.client.on_datagram(&time_reply_datagram(p.now), at),
        Ok(None)
    );
    assert_eq!(p.client.dropped(), 3);
    assert_eq!(p.client.state(), ClientState::Authenticating);
}

#[test]
fn stream_end_outside_closing_is_a_violation() {
    // Client: the server FINs the world stream while InWorld.
    let mut p = Pair::new(ClientConfig::default(), params());
    p.login();
    p.enter();
    assert!(matches!(
        p.client
            .on_stream_end(StreamRef::Uni(StreamKind::World), StreamEnd::Finished),
        Err(Violation::Stream(_))
    ));
    assert_eq!(p.client.state(), ClientState::Closed);
    assert!(matches!(
        p.client.poll_transmit(),
        Some(Transmit::Close {
            code: AppErrorCode::ProtocolViolation,
            ..
        })
    ));

    // Client: a FIN in the middle of a control frame is a truncated frame.
    let mut p = Pair::new(ClientConfig::default(), params());
    let ping = frame_to_vec(&Message::Ping { nonce: 1 }).unwrap();
    assert_eq!(p.client.on_control_data(&ping[..5], 0), Ok(vec![]));
    assert_eq!(
        p.client
            .on_stream_end(StreamRef::Control, StreamEnd::Finished),
        Err(Violation::Decode(
            tessera_protocol::ProtocolError::Truncated
        ))
    );

    // Client: reset and an untyped stream; then nothing more once Closed.
    for (stream, end) in [
        (StreamRef::Control, StreamEnd::Reset),
        (StreamRef::Control, StreamEnd::Stopped),
        (StreamRef::Untyped, StreamEnd::Finished),
    ] {
        let mut p = Pair::new(ClientConfig::default(), params());
        assert!(p.client.on_stream_end(stream, end).is_err());
        while p.client.poll_transmit().is_some() {}
        assert_eq!(p.client.on_stream_end(stream, end), Ok(()));
        assert!(p.client.poll_transmit().is_none());
    }

    // Client: expected while Closing (after its own Disconnect).
    let mut p = Pair::new(ClientConfig::default(), params());
    p.client.disconnect(AppErrorCode::NoError, "bye", 0);
    assert_eq!(
        p.client
            .on_stream_end(StreamRef::Control, StreamEnd::Finished),
        Ok(())
    );
    assert_eq!(p.client.state(), ClientState::Closing);

    // Server: the client FINs its control stream mid-frame while InWorld.
    let mut p = Pair::new(ClientConfig::default(), params());
    p.login();
    p.enter();
    assert_eq!(p.server.on_control_data(&ping[..3], 0), Ok(vec![]));
    assert_eq!(
        p.server
            .on_stream_end(StreamRef::Control, StreamEnd::Finished),
        Err(Violation::Decode(
            tessera_protocol::ProtocolError::Truncated
        ))
    );
    assert_eq!(p.server.state(), ServerState::Closed);
    // Server: a clean FIN, STOP_SENDING on the world stream, a second control stream.
    let mut p = Pair::new(ClientConfig::default(), params());
    assert!(matches!(
        p.server
            .on_stream_end(StreamRef::Control, StreamEnd::Finished),
        Err(Violation::Stream(_))
    ));
    let mut p = Pair::new(ClientConfig::default(), params());
    p.login();
    p.enter();
    assert!(
        p.server
            .on_stream_end(StreamRef::Uni(StreamKind::World), StreamEnd::Stopped)
            .is_err()
    );
    let mut p = Pair::new(ClientConfig::default(), params());
    assert!(matches!(
        p.server.on_extra_stream(),
        Err(Violation::Stream(_))
    ));
    // Server: expected while Closing.
    let mut p = Pair::new(ClientConfig::default(), params());
    p.server.disconnect(AppErrorCode::Kicked, "", 0);
    assert_eq!(
        p.server
            .on_stream_end(StreamRef::Control, StreamEnd::Finished),
        Ok(())
    );
    assert_eq!(p.server.on_extra_stream(), Ok(()));
    assert_eq!(p.server.state(), ServerState::Closing);
}

#[test]
fn rate_limited_login_waits_for_retry_after() {
    let mut p = Pair::new(ClientConfig::default(), params());
    let creds = || Credentials {
        account: "alice".into(),
        password: "pw".into(),
    };
    p.now = 20;
    p.client.login(creds(), p.now).unwrap();
    p.pump();
    p.server
        .reject_login(LoginFailedCode::RateLimited, 30, "", p.now)
        .unwrap();
    p.pump();
    assert_eq!(p.client.login_retry_at(), Some(20 + 30 * S));
    assert_eq!(
        p.client.login(creds(), p.now + 1),
        Err(tessera_protocol::session::UsageError::RateLimited)
    );
    assert_eq!(
        p.client.register(creds(), 20 + 30 * S - 1),
        Err(tessera_protocol::session::UsageError::RateLimited)
    );
    assert!(p.client.poll_transmit().is_none());
    assert_eq!(p.client.login(creds(), 20 + 30 * S), Ok(()));
}

#[test]
fn invalid_teleport_still_reaches_the_server_application() {
    let mut p = Pair::new(ClientConfig::default(), params());
    p.login();
    p.enter();
    p.take_server_events();
    p.take_client_events();
    let first = p
        .client
        .teleport(Destination::Named { name: "far".into() })
        .unwrap();
    assert_eq!(p.client.pending_teleport(), Some(first));
    p.pump();
    assert!(
        matches!(&p.take_server_events()[..], [ServerEvent::Teleport(t)] if t.request_id == first)
    );
    let second = p
        .client
        .teleport(Destination::Named { name: "   ".into() })
        .unwrap();
    assert_eq!(p.client.pending_teleport(), Some(second));
    p.pump();
    // The application learns of the superseding request and cancels the first one.
    assert!(matches!(
        &p.take_server_events()[..],
        [ServerEvent::TeleportInvalid(t)] if t.request_id == second
    ));
    p.server
        .send(
            Message::TeleportRefused(TeleportRefused {
                request_id: first,
                code: tessera_protocol::codes::TeleportRefusedCode::Cancelled,
                text: String::new(),
            }),
            p.now,
        )
        .unwrap();
    p.pump();
    let ev = p.take_client_events();
    assert_eq!(ev.len(), 2, "{ev:?}");
    assert_eq!(p.client.pending_teleport(), None);
}

#[test]
fn requested_report_between_reset_and_teleport_is_allowed() {
    let mut p = Pair::new(ClientConfig::default(), params());
    p.login();
    p.enter();
    let id = p
        .client
        .teleport(Destination::Named { name: "far".into() })
        .unwrap();
    let q = p.client.query_location().unwrap();
    p.pump();
    p.server.send(Message::InterestReset, 0).unwrap();
    p.server
        .send(
            Message::LocationReport(LocationReport {
                cause: LocationCause::Requested,
                request_id: q,
                epoch: 1,
                server_time: 7 * S,
                pose: world_enter().pose,
            }),
            0,
        )
        .unwrap();
    p.server
        .send(
            Message::LocationReport(LocationReport {
                cause: LocationCause::Teleported,
                request_id: id,
                epoch: 2,
                server_time: 8 * S,
                pose: Pose {
                    location: Location::new(7, 100.0, 100.0, 0.0),
                    heading: 0.0,
                    anchor: None,
                },
            }),
            0,
        )
        .unwrap();
    p.pump();
    assert!(p.client_violation.is_none(), "{:?}", p.client_violation);
    assert_eq!(p.client.region(), Some(7));
    // The reset was used up by that teleport: another region change needs a new one.
    p.server
        .send(
            Message::LocationReport(LocationReport {
                cause: LocationCause::Teleported,
                request_id: 0,
                epoch: 3,
                server_time: 9 * S,
                pose: Pose {
                    location: Location::new(8, 100.0, 100.0, 0.0),
                    heading: 0.0,
                    anchor: None,
                },
            }),
            0,
        )
        .unwrap();
    p.pump();
    assert!(matches!(p.client_violation, Some(Violation::World(_))));
}

#[test]
fn closed_client_ignores_further_input() {
    let mut p = Pair::new(ClientConfig::default(), params());
    assert!(p.client.on_control_data(&[0, 0, 0, 0], 0).is_err());
    assert!(matches!(
        p.client.poll_transmit(),
        Some(Transmit::Close { .. })
    ));
    assert!(p.client.poll_transmit().is_none());
    assert_eq!(p.client.on_datagram(&[0xff, 0xff], 0), Ok(None));
    assert_eq!(p.client.on_control_data(&[1, 2, 3, 4, 5], 0), Ok(vec![]));
    assert_eq!(p.client.on_uni_stream(0x00), Ok(None));
    assert_eq!(
        p.client.on_stream_data(StreamKind::World, &[0; 8], 0),
        Ok(vec![])
    );
    assert!(p.client.poll_transmit().is_none());
    assert_eq!(p.client.state(), ClientState::Closed);

    // Closing: a bad datagram does not replace the queued graceful close.
    let mut p = Pair::new(ClientConfig::default(), params());
    p.client.disconnect(AppErrorCode::NoError, "bye", 0);
    assert_eq!(p.client.on_datagram(&[0xff, 0xff], 0), Ok(None));
    assert_eq!(p.client.on_control_data(&[0, 0, 0, 0], 0), Ok(vec![]));
    assert!(matches!(
        p.client.poll_transmit(),
        Some(Transmit::Message(Message::Disconnect(_)))
    ));
    assert_eq!(p.client.poll_transmit(), Some(Transmit::FinishControl));
    assert!(p.client.poll_transmit().is_none());
    assert_eq!(p.client.state(), ClientState::Closing);
}

#[test]
fn simultaneous_disconnect_closes_with_no_error() {
    let mut p = Pair::new(ClientConfig::default(), params());
    p.login();
    let ev = p.server.disconnect(AppErrorCode::Kicked, "", 0);
    assert!(matches!(
        ev,
        Some(ServerEvent::Closed {
            code: AppErrorCode::Kicked,
            client: false
        })
    ));
    assert_eq!(p.server.disconnect(AppErrorCode::Kicked, "", 0), None);
    while p.server.poll_transmit().is_some() {}
    let bytes = frame_to_vec(&Message::Disconnect(Disconnect {
        code: AppErrorCode::NoError,
        text: String::new(),
    }))
    .unwrap();
    // Already reported, so no second Closed event, but the close is at once with NO_ERROR.
    assert_eq!(p.server.on_control_data(&bytes, 0), Ok(vec![]));
    assert_eq!(p.server.state(), ServerState::Closed);
    assert!(matches!(
        p.server.poll_transmit(),
        Some(Transmit::Close {
            code: AppErrorCode::NoError,
            ..
        })
    ));
}

#[test]
fn every_session_end_is_reported_once() {
    // UNSUPPORTED_VERSION after Hello.
    let mut prm = params();
    prm.min_minor = 1;
    prm.max_minor = 1;
    let p = Pair::new(ClientConfig::default(), prm);
    let closed = |ev: &[ServerEvent]| {
        ev.iter()
            .filter(|e| matches!(e, ServerEvent::Closed { .. }))
            .count()
    };
    assert!(matches!(
        &p.greeting_events.1[..],
        [ServerEvent::Closed {
            code: AppErrorCode::UnsupportedVersion,
            client: false
        }]
    ));

    // Chat flooding inside ChatSend handling.
    let mut p = Pair::new(ClientConfig::default(), params());
    p.login();
    p.enter();
    p.take_server_events();
    for _ in 0..100 {
        if p.client.chat(ChatChannel::Say, "x".into()).is_err() {
            break;
        }
        p.pump();
    }
    let ev = p.take_server_events();
    assert_eq!(closed(&ev), 1, "{ev:?}");
    assert!(ev.iter().any(|e| matches!(
        e,
        ServerEvent::Closed {
            code: AppErrorCode::Flooding,
            ..
        }
    )));

    // reject_login with SERVER_FULL.
    let mut p = Pair::new(ClientConfig::default(), params());
    p.client
        .login(
            Credentials {
                account: "alice".into(),
                password: "pw".into(),
            },
            0,
        )
        .unwrap();
    p.pump();
    let ev = p
        .server
        .reject_login(LoginFailedCode::ServerFull, 0, "", 0)
        .unwrap();
    assert_eq!(
        ev,
        Some(ServerEvent::Closed {
            code: AppErrorCode::ServerFull,
            client: false
        })
    );
    p.pump();
    assert_eq!(closed(&p.take_server_events()), 0);

    // A peer close without Disconnect is reported by connection_closed, once.
    let mut s = ServerSession::new(params(), 0);
    s.handshake_complete(0, PeerDatagrams::MaxFrameSize(1_200))
        .unwrap();
    assert_eq!(
        s.connection_closed(ConnectionEnd::IdleTimeout),
        Some(ServerEvent::Closed {
            code: AppErrorCode::NoError,
            client: false
        })
    );
    assert_eq!(s.connection_closed(ConnectionEnd::IdleTimeout), None);
}

#[test]
fn client_close_causes() {
    let fresh = || {
        let mut c = ClientSession::new(ClientConfig::default());
        c.connect(0).unwrap();
        c
    };
    // The peer's reason phrase is sanitised and cut to 64 bytes.
    let mut c = fresh();
    let reason = format!("bad\u{7}\u{1b}[31m{}", "x".repeat(200));
    let ev = c.connection_closed(ConnectionEnd::PeerClosed {
        code: AppErrorCode::Kicked,
        reason,
    });
    let Some(ClientEvent::Closed(CloseCause::PeerClosed { reason, .. })) = ev else {
        panic!("{ev:?}");
    };
    assert!(reason.len() <= 64);
    assert!(!reason.chars().any(char::is_control), "{reason:?}");
    // Transport failures and idle timeouts are told apart.
    let mut c = fresh();
    assert_eq!(
        c.connection_closed(ConnectionEnd::Transport("socket gone".into())),
        Some(ClientEvent::Closed(CloseCause::Transport(
            "socket gone".into()
        )))
    );
    let mut c = fresh();
    assert_eq!(
        c.connection_closed(ConnectionEnd::IdleTimeout),
        Some(ClientEvent::Closed(CloseCause::Failure(
            tessera_protocol::codes::ClientFailure::Timeout
        )))
    );
    assert_eq!(c.connection_closed(ConnectionEnd::IdleTimeout), None);
}

#[test]
fn failed_encode_leaves_the_buffer_unchanged() {
    let mut appearance = Appearance::new(Body::Character { species: 1, sex: 1 });
    appearance.equipment.push(Equipped {
        item: 7,
        dye: Some(2.0),
        secondary_dye: None,
        stowed: false,
    });
    let mut out = vec![0xAA];
    assert!(
        Message::AppearanceSet(appearance)
            .encode_body(&mut out)
            .is_err()
    );
    assert_eq!(out, vec![0xAA]);
    let mut out = vec![0xBB];
    let bad = Value::Array(vec![Value::Float32(1.0), Value::Float32(f32::NAN)]);
    assert!(bad.encode(&mut out, false).is_err());
    assert_eq!(out, vec![0xBB]);
}
